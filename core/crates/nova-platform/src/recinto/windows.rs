//! Il recinto di Windows: un contenitore (AppContainer) con il profilo
//! `nova.recinto`, e un job object che tiene insieme il comando e i suoi figli.
//!
//! Windows non ha Landlock, ma un AppContainer gli somiglia abbastanza da
//! servire: il processo gira con l'identita' del contenitore, e ogni controllo
//! d'accesso si fa **due volte** — una con i permessi dell'utente, una con
//! quelli del contenitore — e passa solo se passano entrambe. Il contenitore
//! non ha voci su niente finche' NOVA non gliele scrive: sulle cartelle
//! dichiarate (leggere, scrivere, cancellare), sugli strumenti (solo leggere
//! ed eseguire) e su poche antenate (solo la cartella, perche' PowerShell si
//! posizioni). Del resto del profilo non legge niente; delle antenate vede i
//! nomi.
//!
//! L'identita' e' il SID dell'AppContainer (`S-1-15-2-...`), derivato dal nome
//! `nova.recinto` **in UTF-16**: non e' un utente, non ha password, non compare
//! in nessun elenco, e ha lo stesso valore su ogni PC. NOVA lo scrive come voce
//! di permesso sulle cartelle, e lo toglie quando una cartella esce
//! dall'elenco e alla disinstallazione (`novad --recinto --togli`).
//!
//! Strade scartate (D367): un token ristretto in scrittura (`WRITE_RESTRICTED`),
//! che con Everyone fra i SID di restrizione lasciava scrivibili le cartelle
//! aperte a Everyone e non copriva la cancellazione; e il livello di integrita'
//! basso, che avrebbe reso le cartelle dichiarate scrivibili a **ogni**
//! processo a integrita' bassa del PC.

use super::Permessi;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use windows::core::{BOOL, HRESULT, PCWSTR, PWSTR};
use windows::Win32::Foundation::{
    CloseHandle, LocalFree, SetHandleInformation, ERROR_ALREADY_EXISTS, ERROR_NOT_FOUND, HANDLE,
    HANDLE_FLAGS, HANDLE_FLAG_INHERIT, HLOCAL, WAIT_OBJECT_0,
};
use windows::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSidToSidW, GetNamedSecurityInfoW, SetEntriesInAclW,
    SetNamedSecurityInfoW, EXPLICIT_ACCESS_W, GRANT_ACCESS, NO_MULTIPLE_TRUSTEE, REVOKE_ACCESS,
    SE_FILE_OBJECT, TRUSTEE_IS_SID, TRUSTEE_IS_UNKNOWN, TRUSTEE_W,
};
use windows::Win32::Security::Isolation::{
    CreateAppContainerProfile, DeleteAppContainerProfile, DeriveAppContainerSidFromAppContainerName,
};
use windows::Win32::Security::{
    AccessCheck, CreateRestrictedToken, DeleteAce, DuplicateTokenEx, EqualSid, FreeSid, GetAce,
    SecurityImpersonation,
    TokenImpersonation, ACCESS_ALLOWED_ACE, ACE_FLAGS, ACE_HEADER, ACL, DACL_SECURITY_INFORMATION,
    GENERIC_MAPPING, GROUP_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION, PRIVILEGE_SET,
    PSECURITY_DESCRIPTOR, PSID, SECURITY_ATTRIBUTES, SECURITY_CAPABILITIES, SID_AND_ATTRIBUTES,
    SUB_CONTAINERS_AND_OBJECTS_INHERIT, TOKEN_ASSIGN_PRIMARY, TOKEN_DUPLICATE, TOKEN_IMPERSONATE,
    TOKEN_QUERY, DISABLE_MAX_PRIVILEGE, LUA_TOKEN,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, DELETE, FILE_ALL_ACCESS, FILE_FLAGS_AND_ATTRIBUTES, FILE_GENERIC_EXECUTE,
    FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, TerminateJobObject,
};
use windows::Win32::System::Pipes::CreatePipe;
use windows::Win32::System::Threading::{
    CreateProcessAsUserW, CreateProcessW, DeleteProcThreadAttributeList, GetExitCodeProcess,
    InitializeProcThreadAttributeList, OpenProcessToken, ResumeThread, TerminateProcess,
    UpdateProcThreadAttribute, WaitForSingleObject, CREATE_NO_WINDOW, CREATE_SUSPENDED,
    CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT, LPPROC_THREAD_ATTRIBUTE_LIST,
    PROCESS_CREATION_FLAGS, PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
    PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES, STARTF_USESTDHANDLES, STARTUPINFOEXW,
    STARTUPINFOW,
};

use windows::Win32::Security::{
    GetSecurityDescriptorControl, InitializeSecurityDescriptor, SetKernelObjectSecurity,
    SetSecurityDescriptorControl, SetSecurityDescriptorDacl, SECURITY_DESCRIPTOR_CONTROL,
};
use windows::Win32::Storage::FileSystem::{
    FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_DELETE, READ_CONTROL, WRITE_DAC,
};

use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION};
use windows::Win32::Storage::FileSystem::GetDriveTypeW;
use windows::Win32::System::Threading::GetCurrentProcess;
use windows::Win32::UI::Shell::{
    ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
use windows::Win32::Foundation::{WAIT_ABANDONED, WAIT_TIMEOUT};
use windows::Win32::System::Threading::{CreateMutexW, ReleaseMutex};

use windows::Win32::Storage::FileSystem::{GetLogicalDrives, GetVolumeInformationW};
use windows::Win32::System::Threading::{
    GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL, THREAD_PRIORITY_NORMAL,
};

/// Il nome del profilo AppContainer da cui si deriva l'identita' di NOVA.
/// Cambiarlo cambia il SID, e le voci gia' scritte sulle cartelle
/// resterebbero intestate a nessuno.
pub const NOME_PROFILO: &str = "nova.recinto";

/// «Modifica» di Windows: leggere, scrivere, eseguire e cancellare. Non
/// «Controllo completo»: un comando non deve poter cambiare i permessi
/// della cartella, nemmeno dentro il recinto.
const MODIFICA: u32 = FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0 | FILE_GENERIC_EXECUTE.0 | DELETE.0;

/// Leggere ed eseguire, mai scrivere.
const LEGGI_ESEGUI: u32 = FILE_GENERIC_READ.0 | FILE_GENERIC_EXECUTE.0;

const ACCESS_ALLOWED_ACE_TYPE: u8 = 0;
/// `OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE`: la voce vale anche per quel
/// che sta dentro.
const EREDITA: u8 = 0x01 | 0x02;
const SOLO_EREDITATA: u8 = 0x08; // INHERIT_ONLY_ACE

/// La capability che apre la rete verso l'esterno. Senza, un AppContainer non
/// ha ne' DNS ne' internet; il loopback resta chiuso comunque (verificato).
const INTERNET_CLIENT: &str = "S-1-15-3-1";
const SE_GROUP_ENABLED: u32 = 0x0000_0004;

fn errore(cosa: &str) -> String {
    format!("{cosa}: {}", std::io::Error::last_os_error())
}

fn largo(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn largo_percorso(p: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    p.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
}

/// Un SID posseduto, liberato con `LocalFree`.
pub struct Identita {
    sid: PSID,
}

unsafe impl Send for Identita {}
unsafe impl Sync for Identita {}

impl Drop for Identita {
    fn drop(&mut self) {
        unsafe {
            let _ = LocalFree(Some(HLOCAL(self.sid.0)));
        }
    }
}

impl Identita {
    /// L'identita' di NOVA: il SID dell'AppContainer `nova.recinto`. Si
    /// **deriva** dal nome, senza creare niente: e' lo stesso su ogni PC.
    ///
    /// Il nome va passato come UTF-16. Passato come ANSI — e' successo in un
    /// laboratorio — Windows registra un profilo con sei caratteri senza
    /// senso, e il SID e' un altro.
    pub fn di_nova() -> Result<Identita, String> {
        let nome = largo(NOME_PROFILO);
        let sid = unsafe { DeriveAppContainerSidFromAppContainerName(PCWSTR(nome.as_ptr())) }
            .map_err(|e| format!("non derivo l'identita' di NOVA: {e}"))?;
        Identita::copia_e_libera(sid)
    }

    /// Un SID che il sistema ha allocato (da liberare con `FreeSid`) diventa
    /// un SID nostro, liberato con `LocalFree`.
    fn copia_e_libera(sid: PSID) -> Result<Identita, String> {
        let mut p = PWSTR::null();
        let convertito = unsafe { ConvertSidToStringSidW(sid, &mut p) };
        if convertito.is_err() {
            unsafe {
                FreeSid(sid);
            }
            return Err(errore("non leggo il SID del contenitore"));
        }
        let testo = unsafe { p.to_string() }.unwrap_or_default();
        unsafe {
            let _ = LocalFree(Some(HLOCAL(p.0 as *mut c_void)));
            FreeSid(sid);
        }
        Identita::da_stringa(&testo)
    }

    /// Un'identita' da un SID scritto: serve a `recinto.json`, che tiene
    /// l'identita' con cui sono state scritte le voci.
    pub fn da_stringa(testo: &str) -> Result<Identita, String> {
        let largo_testo = largo(testo);
        let mut sid = PSID::default();
        unsafe {
            ConvertStringSidToSidW(PCWSTR(largo_testo.as_ptr()), &mut sid)
                .map_err(|e| format!("non e' un SID: {testo}: {e}"))?;
        }
        Ok(Identita { sid })
    }

    /// `S-1-15-2-...`: com'e' scritta sulle cartelle e in `recinto.json`.
    pub fn stringa(&self) -> String {
        let mut p = PWSTR::null();
        unsafe {
            if ConvertSidToStringSidW(self.sid, &mut p).is_err() {
                return String::new();
            }
            let s = p.to_string().unwrap_or_default();
            let _ = LocalFree(Some(HLOCAL(p.0 as *mut c_void)));
            s
        }
    }

    fn trustee(&self) -> TRUSTEE_W {
        TRUSTEE_W {
            pMultipleTrustee: std::ptr::null_mut(),
            MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_UNKNOWN,
            ptstrName: PWSTR(self.sid.0 as *mut u16),
        }
    }
}

/// Registra il profilo del contenitore, se non c'e' gia', e torna la sua
/// identita'. **Lascia traccia sul PC** — una cartella in
/// `%LOCALAPPDATA%\Packages` e una voce nel registro — e quella traccia si
/// toglie con `elimina_profilo`: lo fa `novad --recinto --togli`.
pub fn assicura_profilo() -> Result<Identita, String> {
    let nome = largo(NOME_PROFILO);
    let descrizione = largo("Recinto dei comandi di NOVA");
    let esito = unsafe {
        CreateAppContainerProfile(
            PCWSTR(nome.as_ptr()),
            PCWSTR(nome.as_ptr()),
            PCWSTR(descrizione.as_ptr()),
            None,
        )
    };
    match esito {
        Ok(sid) => Identita::copia_e_libera(sid),
        Err(e) if e.code() == HRESULT::from_win32(ERROR_ALREADY_EXISTS.0) => Identita::di_nova(),
        Err(e) => Err(format!("non registro il profilo del recinto: {e}")),
    }
}

/// Toglie il profilo del contenitore e la sua cartella. Se non c'e', va bene.
pub fn elimina_profilo() -> Result<(), String> {
    let nome = largo(NOME_PROFILO);
    match unsafe { DeleteAppContainerProfile(PCWSTR(nome.as_ptr())) } {
        Ok(()) => Ok(()),
        Err(e) if e.code() == HRESULT::from_win32(ERROR_NOT_FOUND.0) => Ok(()),
        Err(e) => Err(format!("non tolgo il profilo del recinto: {e}")),
    }
}

/// La cartella temporanea che Windows da' al contenitore. `TEMP` e `TMP`
/// dei comandi puntano **sempre** qui, qualunque valore si passi: e' una
/// sola, condivisa e non muore col comando. Si svuota a fine comando.
pub fn temporanea_del_profilo() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(|l| PathBuf::from(l).join("Packages").join(NOME_PROFILO).join("AC").join("Temp"))
}

/// Il DACL di una cartella, letto adesso. `descrittore` va liberato.
struct Dacl {
    acl: *mut ACL,
    descrittore: PSECURITY_DESCRIPTOR,
}

impl Drop for Dacl {
    fn drop(&mut self) {
        unsafe {
            let _ = LocalFree(Some(HLOCAL(self.descrittore.0)));
        }
    }
}

fn leggi_dacl(dove: &Path) -> Result<Dacl, String> {
    let nome = largo_percorso(dove);
    let mut acl: *mut ACL = std::ptr::null_mut();
    let mut sd = PSECURITY_DESCRIPTOR::default();
    let e = unsafe {
        GetNamedSecurityInfoW(
            PCWSTR(nome.as_ptr()),
            SE_FILE_OBJECT,
            // Proprietario e gruppo servono ad `AccessCheck`, che sul solo
            // DACL rifiuta il descrittore.
            DACL_SECURITY_INFORMATION | OWNER_SECURITY_INFORMATION | GROUP_SECURITY_INFORMATION,
            None,
            None,
            Some(&mut acl as *mut *mut ACL),
            None,
            &mut sd,
        )
    };
    if e.is_err() {
        return Err(format!(
            "non leggo i permessi di {}: {}",
            dove.display(),
            std::io::Error::from_raw_os_error(e.0 as i32)
        ));
    }
    Ok(Dacl {
        acl,
        descrittore: sd,
    })
}

fn scrivi_dacl(dove: &Path, acl: *const ACL) -> Result<(), String> {
    let nome = largo_percorso(dove);
    let e = unsafe {
        SetNamedSecurityInfoW(
            PCWSTR(nome.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(acl),
            None,
        )
    };
    if e.is_err() {
        return Err(format!(
            "non scrivo i permessi di {}: {}",
            dove.display(),
            std::io::Error::from_raw_os_error(e.0 as i32)
        ));
    }
    Ok(())
}

/// Scrive il DACL su **quella cartella soltanto**, senza ripassare i figli.
///
/// `SetNamedSecurityInfoW` dopo aver scritto ripercorre tutto il sottoalbero
/// per aggiornare l'ereditarieta'. Su una cartella di progetto serve — i file
/// che ci sono devono prendere la voce — ma per un'antenata come
/// `C:\Users\<nome>` vuol dire ripassare l'intero profilo: la prova
/// end-to-end e' rimasta due minuti e mezzo al 100% di CPU, e la revoca
/// avrebbe rifatto lo stesso giro. Questa e' la chiamata del kernel, senza
/// il giro: un permesso che non si eredita non ha niente da propagare.
///
/// Il controllo del descrittore — DACL protetto, DACL gestito
/// dall'ereditarieta' automatica — si ricopia dall'originale, perche' senza
/// il sistema tratterebbe come esplicite anche le voci ereditate.
fn scrivi_dacl_locale(dove: &Path, acl: *const ACL, originale: &Dacl) -> Result<(), String> {
    let nome = largo_percorso(dove);
    let h = unsafe {
        CreateFileW(
            PCWSTR(nome.as_ptr()),
            WRITE_DAC.0 | READ_CONTROL.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            None,
        )
    }
    .map_err(|e| format!("non apro {} per scrivere i permessi: {e}", dove.display()))?;
    let h = Chiuso(h);
    let mut memoria = vec![0u64; 16];
    let sd = PSECURITY_DESCRIPTOR(memoria.as_mut_ptr() as *mut c_void);
    unsafe {
        InitializeSecurityDescriptor(sd, 1)
            .map_err(|e| format!("non preparo il descrittore: {e}"))?;
        SetSecurityDescriptorDacl(sd, true, Some(acl), false)
            .map_err(|e| format!("non metto i permessi nel descrittore: {e}"))?;
        let mut controllo = 0u16;
        let mut revisione = 0u32;
        if GetSecurityDescriptorControl(originale.descrittore, &mut controllo, &mut revisione).is_ok() {
            // SE_DACL_AUTO_INHERITED | SE_DACL_PROTECTED
            let interesse = SECURITY_DESCRIPTOR_CONTROL(0x0400 | 0x1000);
            let _ = SetSecurityDescriptorControl(
                sd,
                interesse,
                SECURITY_DESCRIPTOR_CONTROL(controllo & interesse.0),
            );
        }
        SetKernelObjectSecurity(h.0, DACL_SECURITY_INFORMATION, sd)
            .map_err(|e| format!("non scrivo i permessi di {}: {e}", dove.display()))?;
    }
    Ok(())
}

/// Perche' una cartella sta fra quelle del recinto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Genere {
    /// Dichiarata in `write_roots`: leggere, scrivere, cancellare, ereditato.
    Scrive,
    /// Uno strumento da far partire (python, node, cargo): solo leggere ed
    /// eseguire, ereditato. Mai scrittura.
    Legge,
    /// Una cartella sopra una radice dichiarata per scrivere: la prima sotto la
    /// radice del disco, il genitore o il nonno (vedi `per_posizionarsi_sotto`).
    /// PowerShell non riesce a posizionarsi in una cartella se il contenitore
    /// non puo' leggere queste (provato: servono lettura ed esecuzione, non
    /// basta l'attraversamento). Solo la cartella, **non** il suo contenuto: se
    /// ne vedono i nomi.
    Antenata,
}

impl Genere {
    pub fn maschera(self) -> u32 {
        match self {
            Genere::Scrive => MODIFICA,
            Genere::Legge | Genere::Antenata => LEGGI_ESEGUI,
        }
    }

    fn eredita(self) -> u8 {
        match self {
            Genere::Antenata => 0,
            _ => EREDITA,
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            Genere::Scrive => "scrive",
            Genere::Legge => "legge",
            Genere::Antenata => "antenata",
        }
    }

    pub fn da_nome(testo: &str) -> Option<Genere> {
        match testo {
            "scrive" => Some(Genere::Scrive),
            "legge" => Some(Genere::Legge),
            "antenata" => Some(Genere::Antenata),
            _ => None,
        }
    }
}

/// Se la cartella ha gia' una voce **esplicita** per l'identita' che copre
/// quel genere. Una voce di scrittura copre anche la lettura.
pub fn ha_permesso(dove: &Path, chi: &Identita, genere: Genere) -> Result<bool, String> {
    let d = leggi_dacl(dove)?;
    if d.acl.is_null() {
        return Ok(false);
    }
    let quante = unsafe { (*d.acl).AceCount };
    for i in 0..quante as u32 {
        let mut ace: *mut c_void = std::ptr::null_mut();
        if unsafe { GetAce(d.acl, i, &mut ace) }.is_err() {
            continue;
        }
        let testa = unsafe { *(ace as *const ACE_HEADER) };
        if testa.AceType != ACCESS_ALLOWED_ACE_TYPE || testa.AceFlags & SOLO_EREDITATA != 0 {
            continue;
        }
        let voce = unsafe { &*(ace as *const ACCESS_ALLOWED_ACE) };
        let sid = PSID(&voce.SidStart as *const u32 as *mut c_void);
        if unsafe { EqualSid(sid, chi.sid) }.is_err() {
            continue;
        }
        let copre = (voce.Mask & genere.maschera()) == genere.maschera();
        let eredita_ok = genere.eredita() == 0 || (testa.AceFlags & EREDITA) == EREDITA;
        if copre && eredita_ok {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Scrive sulla cartella la voce per l'identita'.
///
/// Per le cartelle in cui il contenitore scrive o da cui legge strumenti la
/// voce si **eredita**, e Windows la propaga ai file che ci sono: su una
/// cartella grande ci mette il tempo che ci mette. Per un'antenata no: e'
/// la cartella sola, e si scrive senza ripassare i figli (vedi
/// `scrivi_dacl_locale`).
pub fn concedi(dove: &Path, chi: &Identita, genere: Genere) -> Result<(), String> {
    if ha_permesso(dove, chi, genere)? {
        return Ok(());
    }
    let d = leggi_dacl(dove)?;
    let voce = EXPLICIT_ACCESS_W {
        grfAccessPermissions: genere.maschera(),
        grfAccessMode: GRANT_ACCESS,
        grfInheritance: ACE_FLAGS(genere.eredita() as u32),
        Trustee: chi.trustee(),
    };
    let mut nuovo: *mut ACL = std::ptr::null_mut();
    let e = unsafe {
        SetEntriesInAclW(
            Some(&[voce]),
            if d.acl.is_null() { None } else { Some(d.acl as *const ACL) },
            &mut nuovo,
        )
    };
    if e.is_err() {
        return Err(format!(
            "non compongo i permessi di {}: {}",
            dove.display(),
            std::io::Error::from_raw_os_error(e.0 as i32)
        ));
    }
    let esito = if genere == Genere::Antenata {
        scrivi_dacl_locale(dove, nuovo, &d)
    } else {
        scrivi_dacl(dove, nuovo)
    };
    unsafe {
        let _ = LocalFree(Some(HLOCAL(nuovo as *mut c_void)));
    }
    esito
}

/// Toglie dalla cartella **ogni** voce esplicita intestata all'identita',
/// di qualunque genere, e Windows toglie da solo anche le copie ereditate
/// sotto. Una voce del contenitore e' nostra per definizione: il SID non e'
/// di nessun altro.
pub fn revoca(dove: &Path, chi: &Identita) -> Result<(), String> {
    revoca_con(dove, chi, false)
}

/// Come `revoca`, ma senza ripassare i figli: per le cartelle che portano
/// solo una voce di antenata, che non si eredita e non ha copie sotto.
pub fn revoca_locale(dove: &Path, chi: &Identita) -> Result<(), String> {
    revoca_con(dove, chi, true)
}

fn revoca_con(dove: &Path, chi: &Identita, locale: bool) -> Result<(), String> {
    let d = leggi_dacl(dove)?;
    if d.acl.is_null() {
        return Ok(());
    }
    let voce = EXPLICIT_ACCESS_W {
        grfAccessPermissions: 0,
        grfAccessMode: REVOKE_ACCESS,
        grfInheritance: SUB_CONTAINERS_AND_OBJECTS_INHERIT,
        Trustee: chi.trustee(),
    };
    let mut nuovo: *mut ACL = std::ptr::null_mut();
    let e = unsafe { SetEntriesInAclW(Some(&[voce]), Some(d.acl as *const ACL), &mut nuovo) };
    if e.is_err() {
        return Err(format!(
            "non compongo i permessi di {}: {}",
            dove.display(),
            std::io::Error::from_raw_os_error(e.0 as i32)
        ));
    }
    let esito = if locale {
        scrivi_dacl_locale(dove, nuovo, &d)
    } else {
        scrivi_dacl(dove, nuovo)
    };
    unsafe {
        let _ = LocalFree(Some(HLOCAL(nuovo as *mut c_void)));
    }
    esito
}

/// Toglie **solo** la voce di quel genere — maschera e flag esatti — e lascia
/// le altre voci del contenitore sulla stessa cartella.
pub fn togli_genere(dove: &Path, chi: &Identita, genere: Genere) -> Result<(), String> {
    let d = leggi_dacl(dove)?;
    if d.acl.is_null() {
        return Ok(());
    }
    let quante = unsafe { (*d.acl).AceCount } as u32;
    let mut tolta = false;
    for i in (0..quante).rev() {
        let mut ace: *mut c_void = std::ptr::null_mut();
        if unsafe { GetAce(d.acl, i, &mut ace) }.is_err() {
            continue;
        }
        let testa = unsafe { *(ace as *const ACE_HEADER) };
        if testa.AceType != ACCESS_ALLOWED_ACE_TYPE
            || testa.AceFlags & 0x10 != 0
            || testa.AceFlags & EREDITA != genere.eredita()
        {
            continue;
        }
        let voce = unsafe { &*(ace as *const ACCESS_ALLOWED_ACE) };
        let sid = PSID(&voce.SidStart as *const u32 as *mut c_void);
        if unsafe { EqualSid(sid, chi.sid) }.is_ok() && voce.Mask == genere.maschera() {
            unsafe { DeleteAce(d.acl, i) }
                .map_err(|e| format!("non tolgo la voce da {}: {e}", dove.display()))?;
            tolta = true;
            break;
        }
    }
    if tolta {
        if genere == Genere::Antenata {
            scrivi_dacl_locale(dove, d.acl, &d)?;
        } else {
            scrivi_dacl(dove, d.acl)?;
        }
    }
    Ok(())
}

/// Un token del contenitore, buono per `AccessCheck`: il controllo "il
/// contenitore ci arriva?" lo fa Windows, con le sue regole — quelle degli
/// AppContainer, che valutano anche ALL APPLICATION PACKAGES — e non una
/// nostra stima.
pub struct Sonda {
    token: Chiuso,
}

/// Il token si prende da un processo del contenitore che non parte mai: si
/// crea sospeso, se ne duplica il token, si ferma.
pub fn sonda(chi: &Identita) -> Result<Sonda, String> {
    let base = token_di_base(true)?;
    let capacita = Capacita::nuova(chi, false)?;
    let mut attributi = ListaAttributi::nuova(1)?;
    attributi.imposta(
        PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,
        &capacita.sc as *const SECURITY_CAPABILITIES as *const c_void,
        std::mem::size_of::<SECURITY_CAPABILITIES>(),
    )?;
    let radice = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
    let programma = PathBuf::from(radice).join("System32").join("cmd.exe");
    let programma_largo = largo_percorso(&programma);
    let mut riga = largo("cmd.exe /c exit");
    let si = STARTUPINFOEXW {
        StartupInfo: STARTUPINFOW {
            cb: std::mem::size_of::<STARTUPINFOEXW>() as u32,
            ..Default::default()
        },
        lpAttributeList: attributi.lista,
    };
    let mut pi = PROCESS_INFORMATION::default();
    unsafe {
        crea_processo(
            base.as_ref(),
            PCWSTR(programma_largo.as_ptr()),
            &mut riga,
            false,
            CREATE_SUSPENDED | CREATE_NO_WINDOW | EXTENDED_STARTUPINFO_PRESENT,
            None,
            PCWSTR::null(),
            &si.StartupInfo,
            &mut pi,
        )
        .map_err(|e| format!("non avvio la sonda del contenitore: {e}"))?;
    }
    let processo = Chiuso(pi.hProcess);
    let _filo = Chiuso(pi.hThread);
    let mut originale = HANDLE::default();
    let aperto = unsafe { OpenProcessToken(processo.0, TOKEN_QUERY | TOKEN_DUPLICATE, &mut originale) };
    let originale = Chiuso(originale);
    let mut copia = HANDLE::default();
    let duplicato = aperto.and_then(|_| unsafe {
        DuplicateTokenEx(
            originale.0,
            TOKEN_QUERY | TOKEN_IMPERSONATE,
            None,
            SecurityImpersonation,
            TokenImpersonation,
            &mut copia,
        )
    });
    unsafe {
        let _ = TerminateProcess(processo.0, 0);
    }
    duplicato.map_err(|e| format!("non ricavo il token del contenitore: {e}"))?;
    Ok(Sonda { token: Chiuso(copia) })
}

/// Se il contenitore ottiene `maschera` su quella cartella, cosi' com'e'
/// adesso. E' lo stesso controllo che fara' il kernel quando il comando
/// aprira' un file.
pub fn puo(dove: &Path, sonda: &Sonda, maschera: u32) -> Result<bool, String> {
    let d = leggi_dacl(dove)?;
    let mappa = GENERIC_MAPPING {
        GenericRead: FILE_GENERIC_READ.0,
        GenericWrite: FILE_GENERIC_WRITE.0,
        GenericExecute: FILE_GENERIC_EXECUTE.0,
        GenericAll: FILE_ALL_ACCESS.0,
    };
    let mut privilegi = vec![0u8; 256];
    let mut lunghezza = privilegi.len() as u32;
    let mut concesso = 0u32;
    let mut esito = BOOL(0);
    unsafe {
        AccessCheck(
            d.descrittore,
            sonda.token.0,
            maschera,
            &mappa,
            Some(privilegi.as_mut_ptr() as *mut PRIVILEGE_SET),
            &mut lunghezza,
            &mut concesso,
            &mut esito,
        )
        .map_err(|e| format!("controllo d'accesso su {}: {e}", dove.display()))?;
    }
    Ok(esito.as_bool() && (concesso & maschera) == maschera)
}

/// Prepara una cartella perche' il contenitore faccia quel che il genere
/// dice, e torna `true` se ha **scritto** una voce adesso. Se alla fine il
/// controllo di Windows dice che il contenitore non ci arriva — una cartella
/// che nemmeno l'utente puo' modificare — toglie quel che ha scritto e
/// rifiuta: un comando che parte «dentro le cartelle dichiarate» e poi non ci
/// scrive sarebbe una promessa falsa.
///
/// Per le antenate e per le cartelle di strumenti e' diverso: se il contenitore
/// legge gia' quella cartella — `Git`, che sta in Program Files — non si tocca
/// niente.
pub fn prepara_cartella(
    dove: &Path,
    chi: &Identita,
    genere: Genere,
    sonda: &Sonda,
) -> Result<bool, String> {
    if matches!(genere, Genere::Antenata | Genere::Legge) && puo(dove, sonda, genere.maschera())? {
        return Ok(false);
    }
    let c_era = ha_permesso(dove, chi, genere)?;
    if !c_era {
        concedi(dove, chi, genere)?;
    }
    if !puo(dove, sonda, genere.maschera())? {
        if !c_era {
            let _ = togli_genere(dove, chi, genere);
        }
        return Err(format!(
            "la cartella {} non diventa accessibile dentro il recinto ({}): \
             il comando non parte{}",
            dove.display(),
            genere.nome(),
            nota_se_elevato()
        ));
    }
    Ok(!c_era)
}

/// La **prima cartella sotto la radice del disco** (`C:\Users` per
/// `C:\Users\utente\NOVA`): una delle due che PowerShell vuole leggere per
/// posizionarsi (vedi `per_posizionarsi_sotto`).
///
/// `None` se `dove` e' gia' la prima cartella, o la radice stessa.
pub fn prima_componente(dove: &Path) -> Option<PathBuf> {
    dove.ancestors()
        .find(|p| p.parent().is_some_and(|su| su.parent().is_none()))
        .filter(|p| *p != dove && p.exists())
        .map(Path::to_path_buf)
}

/// Le cartelle che il contenitore deve poter leggere perche' PowerShell si
/// posizioni in `radice` **o in una sua sottocartella diretta**.
///
/// La regola e' stata misurata con una catena `R\a\b\c\dentro`, provando
/// le combinazioni: servono **due** cartelle, la prima sotto la radice del
/// disco (`R`) **e il nonno** di quella in cui ci si posiziona (`b`). Il
/// genitore (`c`) e il bisnonno (`a`) no; la prima da sola, il nonno da solo,
/// o la prima col genitore, non bastano. Per posizionarsi in `radice` il nonno
/// e' quello di `radice`; per una sua sottocartella diretta e' il genitore di
/// `radice`; piu' sotto, il nonno cade dentro `radice` e prende la voce che si
/// eredita. Percio' qui: la prima componente, il genitore e il nonno di
/// `radice`, senza la radice del disco (che non si puo' preparare e, finora,
/// non e' servita).
///
/// La prima viene per prima: chi prepara si ferma li' se non ci riesce.
pub fn per_posizionarsi_sotto(radice: &Path) -> Vec<PathBuf> {
    let genitore = radice.parent().map(Path::to_path_buf);
    let nonno = genitore.as_deref().and_then(Path::parent).map(Path::to_path_buf);
    let mut v: Vec<PathBuf> = Vec::new();
    for p in [prima_componente(radice), genitore, nonno].into_iter().flatten() {
        let e_la_radice_del_disco = p.parent().is_none();
        if p != radice && !e_la_radice_del_disco && p.exists() && !v.contains(&p) {
            v.push(p);
        }
    }
    v
}

/// Un handle che si chiude da solo.
struct Chiuso(HANDLE);

// Un handle e' un numero che il kernel capisce da qualunque filo: e' il
// puntatore dentro `HANDLE` a far dubitare il compilatore, non Windows.
unsafe impl Send for Chiuso {}

impl Drop for Chiuso {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

/// Il job object attorno a un comando. Non uccide alla chiusura: un
/// comando finito bene puo' aver avviato un programma che deve restare
/// aperto. Uccide quando lo si **ferma** — interruzione o scadenza — e
/// allora cadono tutti insieme, figli e nipoti.
pub struct Job {
    h: HANDLE,
    fermato: AtomicBool,
}

unsafe impl Send for Job {}
unsafe impl Sync for Job {}

impl Job {
    fn nuovo() -> Result<Job, String> {
        let h = unsafe { CreateJobObjectW(None, PCWSTR::null()) }
            .map_err(|e| format!("non creo il job object: {e}"))?;
        Ok(Job {
            h,
            fermato: AtomicBool::new(false),
        })
    }

    /// Ferma tutto quello che vive nel job. Si puo' chiamare piu' volte.
    pub fn ferma(&self) {
        if !self.fermato.swap(true, Ordering::SeqCst) {
            unsafe {
                let _ = TerminateJobObject(self.h, 1);
            }
        }
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.h);
        }
    }
}

/// Cosa far partire.
#[derive(Debug, Clone)]
pub struct Comando {
    /// Il programma: un percorso, o un nome da cercare nel `PATH`.
    pub programma: String,
    pub argomenti: Vec<String>,
    pub cartella: Option<PathBuf>,
    /// Variabili d'ambiente da sovrascrivere o aggiungere a quelle del demone.
    pub ambiente: Vec<(String, String)>,
}

/// Com'e' andata.
#[derive(Debug, Clone, Default)]
pub struct Esito {
    /// Il codice d'uscita; nessuno se il comando e' stato fermato.
    pub codice: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    /// Fermato dalla scadenza, non finito da solo.
    pub scaduto: bool,
}

/// La riga di comando come la rilegge il C runtime di Windows: le stesse
/// regole della libreria standard di Rust, che questo modulo sostituisce
/// perche' `std::process` non sa far partire un processo con un token.
fn quota(riga: &mut Vec<u16>, arg: &str) {
    let tra_virgolette = arg.is_empty() || arg.contains(' ') || arg.contains('\t');
    if tra_virgolette {
        riga.push(b'"' as u16);
    }
    let mut barre = 0usize;
    for c in arg.encode_utf16() {
        if c == b'\\' as u16 {
            barre += 1;
        } else {
            if c == b'"' as u16 {
                riga.extend(std::iter::repeat(b'\\' as u16).take(barre + 1));
            }
            barre = 0;
        }
        riga.push(c);
    }
    if tra_virgolette {
        riga.extend(std::iter::repeat(b'\\' as u16).take(barre));
        riga.push(b'"' as u16);
    }
}

/// Dove sta il programma. `CreateProcess` cercherebbe da solo, ma
/// comincerebbe dalla cartella del demone: si cerca nel `PATH`, come fa
/// la libreria standard.
fn trova(programma: &str) -> Result<PathBuf, String> {
    let p = Path::new(programma);
    if p.components().count() > 1 {
        return if p.is_file() {
            Ok(p.to_path_buf())
        } else {
            Err(format!("il programma non c'e': {programma}"))
        };
    }
    let mut nomi = vec![programma.to_string()];
    if !programma.to_ascii_lowercase().ends_with(".exe") {
        nomi.push(format!("{programma}.exe"));
    }
    for cartella in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        for n in &nomi {
            let c = cartella.join(n);
            if c.is_file() {
                return Ok(c);
            }
        }
    }
    Err(format!("il programma non si trova nel PATH: {programma}"))
}

/// Il blocco d'ambiente: quello del demone, con le sovrascritture. I
/// nomi non distinguono le maiuscole.
fn ambiente(sovra: &[(String, String)]) -> Vec<u16> {
    let mut voci: Vec<(String, String)> = std::env::vars().collect();
    for (k, v) in sovra {
        let ku = k.to_uppercase();
        voci.retain(|(x, _)| x.to_uppercase() != ku);
        voci.push((k.clone(), v.clone()));
    }
    voci.sort_by_key(|(k, _)| k.to_uppercase());
    let mut blocco = Vec::new();
    for (k, v) in voci {
        blocco.extend(format!("{k}={v}").encode_utf16());
        blocco.push(0);
    }
    blocco.push(0);
    blocco
}

/// Un tubo: il capo che si da' al figlio e' ereditabile, il nostro no.
fn tubo() -> Result<(Chiuso, Chiuso), String> {
    let attr = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: true.into(),
    };
    let mut lettura = HANDLE::default();
    let mut scrittura = HANDLE::default();
    unsafe {
        CreatePipe(&mut lettura, &mut scrittura, Some(&attr as *const SECURITY_ATTRIBUTES), 0)
            .map_err(|e| format!("non creo il tubo: {e}"))?;
        SetHandleInformation(lettura, HANDLE_FLAG_INHERIT.0, HANDLE_FLAGS(0))
            .map_err(|e| format!("non tolgo l'ereditarieta' al tubo: {e}"))?;
    }
    Ok((Chiuso(lettura), Chiuso(scrittura)))
}

fn nul() -> Result<Chiuso, String> {
    let attr = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: true.into(),
    };
    let nome = largo("NUL");
    let h = unsafe {
        CreateFileW(
            PCWSTR(nome.as_ptr()),
            FILE_GENERIC_READ.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            Some(&attr as *const SECURITY_ATTRIBUTES),
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
    }
    .map_err(|e| format!("non apro NUL: {e}"))?;
    Ok(Chiuso(h))
}

fn leggi_tutto(h: Chiuso) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        use std::io::Read;
        use std::os::windows::io::FromRawHandle;
        let mut f = unsafe { std::fs::File::from_raw_handle(h.0 .0) };
        std::mem::forget(h);
        let mut dati = Vec::new();
        let _ = f.read_to_end(&mut dati);
        dati
    })
}

/// Il comando in corsa: si aspetta con `attendi`, si ferma dal job.
pub struct InCorsa {
    pub job: Arc<Job>,
    processo: Chiuso,
    stdout: std::thread::JoinHandle<Vec<u8>>,
    stderr: std::thread::JoinHandle<Vec<u8>>,
}

// ---------------------------------------------------------------------------
// Il passo con privilegi di amministratore (D367)
//
// Per posizionarsi in una cartella PowerShell vuole che il contenitore legga
// la prima cartella sotto la radice del disco. Per un progetto nel profilo e'
// `C:\Users`, e il DACL di `C:\Users` non lo scrive un utente senza
// amministratore. Lo stesso per gli strumenti installati per tutti gli utenti
// (`C:\Python313`). Questo e' l'**unico** punto in cui NOVA chiede piu'
// privilegi di quelli dell'utente, ed e' costruito per restare minuscolo:
//
// - fa **due** cose, e solo di lettura: scrive o toglie la voce di antenata
//   del contenitore su una prima cartella sotto la radice di un disco fisso, e
//   la voce di sola lettura ed esecuzione su una cartella di strumenti. Mai
//   scrittura;
// - l'identita' non si passa: la deriva lui dal nome. Chiunque lo invochi, con
//   qualunque argomento, al peggio apre in lettura una cartella a un
//   contenitore che solo NOVA usa — e il contenitore ottiene sempre
//   l'intersezione fra la voce e i diritti dell'utente, quindi non legge
//   niente che l'utente non legga gia';
// - il percorso e' validato in modo stretto (`e_una_prima_componente`,
//   `e_una_cartella_di_strumenti`);
// - non parte mai da solo: lo lancia un comando esplicito dell'utente, e la
//   conferma la chiede Windows (UAC).
//
// Restano due limiti. Quello di ogni elevazione: Windows mostra il nome
// dell'eseguibile, non gli argomenti, e un eseguibile in una cartella che
// l'utente stesso puo' scrivere si puo' sostituire; installato in una cartella
// protetta (Program Files) il limite cade, in una cartella di lavoro no, ed e'
// detto. E una finestra fra il controllo del percorso e la scrittura: chi puo'
// creare cartelle nella radice del disco potrebbe scambiare la cartella con una
// giunzione nel mezzo. L'effetto massimo e' la stessa voce di lettura su un'altra
// cartella, che per l'intersezione di sopra non da' al contenitore niente di
// piu' di quel che l'utente legge gia'.
// ---------------------------------------------------------------------------

/// Un blocco che vale fra **processi**: il demone e `novad --recinto` scrivono
/// lo stesso elenco, e un `Mutex` di Rust vede solo i fili del proprio
/// processo. Un mutex con nome di Windows li vede tutti. Il mutex e'
/// rientrante per lo stesso filo: chi lo ha gia' puo' riprenderlo.
///
/// **Elevato e non elevato.** Un mutex creato da un processo elevato
/// potrebbe non aprirsi da uno non elevato: il gruppo Amministratori, nel
/// token normale, serve solo a negare. Misurato, con la stessa chiamata di
/// qui (`CreateMutexW` senza descrittore) in tutti e due i versi, su Windows 11
/// Pro 10.0.26200 con UAC attivo e un utente amministratore: si apre. Handle
/// valido, errore 183 («esisteva gia'»), e l'attesa a zero risponde «occupato»
/// finche' lo tiene l'altro. Non misurato: elevare con le credenziali di un
/// **altro** account amministratore, dove il mutex apparterrebbe a un altro
/// utente; li' il descrittore predefinito potrebbe negare l'accesso, e `prendi`
/// fallirebbe con «non creo il blocco».
pub struct BloccoFraProcessi(HANDLE);

unsafe impl Send for BloccoFraProcessi {}

impl BloccoFraProcessi {
    /// Come `prendi`, ma senza aspettare: `None` se lo tiene qualcun altro.
    /// Serve a chi non deve mai fare due volte la stessa cosa lunga insieme.
    pub fn prova(nome: &str) -> Option<BloccoFraProcessi> {
        let n = largo(&format!("Local\\{nome}"));
        let h = unsafe { CreateMutexW(None, false, PCWSTR(n.as_ptr())) }.ok()?;
        let attesa = unsafe { WaitForSingleObject(h, 0) };
        if attesa == WAIT_OBJECT_0 || attesa == WAIT_ABANDONED {
            Some(BloccoFraProcessi(h))
        } else {
            unsafe {
                let _ = CloseHandle(h);
            }
            None
        }
    }

    pub fn prendi(nome: &str) -> Result<BloccoFraProcessi, String> {
        let n = largo(&format!("Local\\{nome}"));
        let h = unsafe { CreateMutexW(None, false, PCWSTR(n.as_ptr())) }
            .map_err(|e| format!("non creo il blocco {nome}: {e}"))?;
        let attesa = unsafe { WaitForSingleObject(h, 60_000) };
        if attesa == WAIT_OBJECT_0 || attesa == WAIT_ABANDONED {
            Ok(BloccoFraProcessi(h))
        } else {
            unsafe {
                let _ = CloseHandle(h);
            }
            Err(format!(
                "un altro processo di NOVA tiene {nome} da piu' di un minuto: non scrivo l'elenco del recinto"
            ))
        }
    }
}

impl Drop for BloccoFraProcessi {
    fn drop(&mut self) {
        unsafe {
            let _ = ReleaseMutex(self.0);
            let _ = CloseHandle(self.0);
        }
    }
}

/// Se il token `token` e' elevato; `None` se Windows non lo dice.
fn elevazione_di(token: HANDLE) -> Option<bool> {
    let mut el = TOKEN_ELEVATION::default();
    let mut restituiti = 0u32;
    unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut el as *mut TOKEN_ELEVATION as *mut c_void),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut restituiti,
        )
    }
    .ok()?;
    Some(el.TokenIsElevated != 0)
}

/// Se questo processo gira elevato; `None` se Windows non lo dice.
pub fn stato_elevazione() -> Option<bool> {
    let mut token = HANDLE::default();
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }.ok()?;
    let token = Chiuso(token);
    elevazione_di(token.0)
}

/// Se questo processo gira elevato, cioe' con i privilegi di amministratore.
pub fn e_elevato() -> bool {
    stato_elevazione() == Some(true)
}

/// Il token da cui nascono la sonda e i comandi **confinati**: mai quello di
/// un amministratore.
///
/// Un figlio creato senza un token esplicito eredita quello del demone, e se il
/// demone e' stato avviato da amministratore il comando nasce con i gruppi di
/// amministrazione abilitati: l'AppContainer controlla l'accesso **due volte**
/// — con il token e con il contenitore — e un amministratore supera la prima in
/// qualunque cartella che conceda qualcosa ai contenitori (GHSA-38cw-xfm5-xq9f).
/// Misurato: il comando dice `IsInRole(Administrator) = False` ma ha
/// `BUILTIN\Administrators` abilitato, e scrive dove un utente normale non
/// scrive. La sonda deve avere lo stesso token del comando: se lei fosse
/// amministratore, «il contenitore ci arriva?» direbbe di si' a quello che il
/// comando vero non puo' fare.
///
/// `None` se il comando non e' confinato (senza recinto non c'e' nessun confine
/// da proteggere: gira come il demone, com'e' sempre stato) o se il demone non
/// e' elevato. Altrimenti un token senza poteri; e se non si riesce a farlo
/// — o se Windows non dice se il demone e' elevato, e nel dubbio lo e' — **il
/// comando non parte**, e lo dice.
fn token_di_base(confinato: bool) -> Result<Option<Chiuso>, String> {
    if !confinato || stato_elevazione() == Some(false) {
        return Ok(None);
    }
    senza_poteri_di_amministratore().map(Some)
}

/// Una riga in piu' per chi legge un rifiuto da un demone elevato: il comando
/// gira senza i poteri dell'amministratore, quindi una cartella che solo gli
/// Amministratori possiedono o scrivono non gli serve, e senza questa riga il
/// rifiuto sembrerebbe un guasto del recinto.
fn nota_se_elevato() -> String {
    if stato_elevazione() == Some(true) {
        ". Il demone gira da amministratore ma il comando no: se la cartella \
         appartiene agli Amministratori (creata da un processo elevato) o la \
         scrivono solo loro, al comando non basta"
            .to_string()
    } else {
        String::new()
    }
}

/// Perche' il comando non parte quando il demone e' elevato e i poteri non si
/// tolgono. Dice «amministratore» e dice che il comando non parte: chi legge
/// deve capire che e' una scelta, non un guasto.
fn rifiuto_per_demone_elevato(perche: &str) -> String {
    format!(
        "il demone gira da amministratore e non riesco a togliere i poteri al comando \
         ({perche}): non lo lancio. Un comando di NOVA non riceve mai i poteri \
         dell'amministratore"
    )
}

/// Il token di questo processo privato dei poteri dell'amministratore: i
/// gruppi di amministrazione tolti, i privilegi tolti tranne quello di
/// attraversare le cartelle, integrita' media. E' quello che l'UAC dava al
/// processo prima di elevarlo, ricavato dal token del demone.
fn senza_poteri_di_amministratore() -> Result<Chiuso, String> {
    let rifiuto = |perche: String| rifiuto_per_demone_elevato(&perche);
    let mut mio = HANDLE::default();
    unsafe {
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_QUERY | TOKEN_DUPLICATE | TOKEN_ASSIGN_PRIMARY,
            &mut mio,
        )
    }
    .map_err(|e| rifiuto(format!("non apro il token del demone: {e}")))?;
    let mio = Chiuso(mio);
    let mut ridotto = HANDLE::default();
    unsafe { CreateRestrictedToken(mio.0, DISABLE_MAX_PRIVILEGE | LUA_TOKEN, None, None, None, &mut ridotto) }
        .map_err(|e| rifiuto(format!("non creo il token ridotto: {e}")))?;
    let ridotto = Chiuso(ridotto);
    // Non ci si fida di averlo fatto: si chiede al token appena nato.
    if elevazione_di(ridotto.0) != Some(false) {
        return Err(rifiuto("il token ridotto risulta ancora elevato".to_string()));
    }
    Ok(ridotto)
}

// kernel32, senza una feature in piu' del crate `windows`.
extern "system" {
    fn GetConsoleProcessList(lista: *mut u32, quanti: u32) -> u32;
    fn FreeConsole() -> i32;
    fn AllocConsole() -> i32;
    fn GetConsoleWindow() -> *mut c_void;
}

/// Come si crea un processo confinato: con una console **nuova** (`CREATE_NO_WINDOW`),
/// o con quella del demone, che il figlio eredita.
///
/// Con il demone elevato e il comando senza poteri la console nuova non va:
/// il figlio muore all'avvio con `0xC0000142`, con o senza contenitore (misurato
/// su Windows 11 10.0.26200). Resta la console ereditata, e per quella
/// `console_privata` si assicura che sia solo del demone.
fn flag_di_creazione(console_ereditata: bool) -> PROCESS_CREATION_FLAGS {
    let base = CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT | EXTENDED_STARTUPINFO_PRESENT;
    if console_ereditata {
        base
    } else {
        base | CREATE_NO_WINDOW
    }
}

/// Fa in modo che la console del demone sia **solo sua**, e nascosta.
///
/// Un comando che eredita una console puo' leggerne e consumarne l'input anche
/// dentro il contenitore (misurato: un tasto messo in coda da un altro processo
/// viene visto e consumato; non puo' invece scriverci ne' leggerne lo schermo).
/// Se il demone e' stato lanciato da un terminale elevato, quella console e'
/// del terminale e un comando potrebbe rubare i tasti battuti li'. Quindi: se
/// al demone e' agganciato solo lui, la console e' gia' sua; altrimenti — ne
/// ha una condivisa, o nessuna — la lascia e se ne da' una nuova, nascosta. Lo
/// si fa una volta sola, prima che esista un figlio che la condivida.
fn console_privata() -> Result<(), String> {
    static ESITO: std::sync::OnceLock<Result<(), String>> = std::sync::OnceLock::new();
    ESITO
        .get_or_init(|| {
            let mut elenco = [0u32; 2];
            if unsafe { GetConsoleProcessList(elenco.as_mut_ptr(), 2) } == 1 {
                return Ok(());
            }
            unsafe { FreeConsole() };
            if unsafe { AllocConsole() } == 0 {
                return Err(rifiuto_per_demone_elevato("non riesco a dargli una console tutta sua"));
            }
            let finestra = unsafe { GetConsoleWindow() };
            if !finestra.is_null() {
                let _ = unsafe { ShowWindow(HWND(finestra), SW_HIDE) };
            }
            Ok(())
        })
        .clone()
}

/// `CreateProcess` dal token dato, o da quello di questo processo se `token`
/// e' `None`. E' l'unico punto in cui si crea un processo del contenitore, e
/// vale per la sonda come per i comandi.
#[allow(clippy::too_many_arguments)]
unsafe fn crea_processo(
    token: Option<&Chiuso>,
    programma: PCWSTR,
    riga: &mut [u16],
    ereditare: bool,
    flag: PROCESS_CREATION_FLAGS,
    ambiente: Option<*const c_void>,
    cartella: PCWSTR,
    si: &STARTUPINFOW,
    pi: &mut PROCESS_INFORMATION,
) -> windows::core::Result<()> {
    let riga = Some(PWSTR(riga.as_mut_ptr()));
    match token {
        Some(t) => CreateProcessAsUserW(
            Some(t.0), programma, riga, None, None, ereditare, flag, ambiente, cartella, si, pi,
        ),
        None => CreateProcessW(programma, riga, None, None, ereditare, flag, ambiente, cartella, si, pi),
    }
}

/// Se `p` e' una cartella su cui il passo privilegiato accetta di lavorare:
/// la **prima sotto la radice di un disco fisso**, e nient'altro.
///
/// Si rifiuta tutto quel che non ha esattamente la forma `X:\nome`: percorsi
/// relativi, `..`, prefissi `\\?\`, percorsi di rete, cartelle piu' in basso,
/// la radice stessa, file, giunzioni e collegamenti simbolici, dischi non
/// fissi. Una giunzione, in particolare, farebbe scrivere il permesso altrove
/// da quello che il percorso dice.
pub fn e_una_prima_componente(p: &Path) -> Result<(), String> {
    use std::os::windows::fs::MetadataExt;
    use std::path::{Component, Prefix};
    let componenti: Vec<Component> = p.components().collect();
    let lettera = match componenti.as_slice() {
        [Component::Prefix(pre), Component::RootDir, Component::Normal(_)] => match pre.kind() {
            Prefix::Disk(l) => l,
            _ => return Err(format!("{} non e' un percorso di un disco locale", p.display())),
        },
        _ => {
            return Err(format!(
                "{} non e' la prima cartella sotto la radice di un disco",
                p.display()
            ))
        }
    };
    let meta = std::fs::symlink_metadata(p).map_err(|e| format!("{}: {e}", p.display()))?;
    if !meta.is_dir() {
        return Err(format!("{} non e' una cartella", p.display()));
    }
    if meta.file_attributes() & 0x0400 != 0 {
        return Err(format!("{} e' una giunzione o un collegamento: non la tocco", p.display()));
    }
    let radice = largo(&format!("{}:\\", lettera as char));
    const DRIVE_FIXED: u32 = 3;
    if unsafe { GetDriveTypeW(PCWSTR(radice.as_ptr())) } != DRIVE_FIXED {
        return Err(format!("{}: il disco non e' un disco fisso", p.display()));
    }
    Ok(())
}

/// Il passo privilegiato, apertura: scrive la voce di antenata del contenitore
/// sulla prima cartella `p`. Non crea il profilo: l'identita' si deriva e
/// basta, perche' un profilo creato da un processo elevato avrebbe permessi
/// suoi.
pub fn apri_prima_componente(p: &Path) -> Result<(), String> {
    e_una_prima_componente(p)?;
    let chi = Identita::di_nova()?;
    concedi(p, &chi, Genere::Antenata)
}

/// Il passo privilegiato, chiusura. Una cartella che non c'e' piu' non ha voci
/// da togliere.
pub fn chiudi_prima_componente(p: &Path) -> Result<(), String> {
    if !p.exists() {
        return Ok(());
    }
    e_una_prima_componente(p)?;
    let chi = Identita::di_nova()?;
    togli_genere(p, &chi, Genere::Antenata)
}

/// Se `p` e' una cartella di strumenti su cui il passo privilegiato accetta di
/// lavorare. A differenza della prima componente, qui puo' stare a qualunque
/// profondita': `C:\Python313`, `C:\Program Files\nodejs`. Si rifiutano la
/// radice del disco, i percorsi che non sono `X:\nome\...` puliti (`..`,
/// `\\?\`, di rete; un `.` in mezzo non cambia dove si va), i file, le
/// giunzioni e i dischi non fissi.
///
/// Il passo concede **solo lettura ed esecuzione**, ereditata, mai scrittura, e
/// il contenitore ottiene sempre l'intersezione fra la voce e i diritti
/// dell'utente: non legge niente che l'utente non legga gia'.
pub fn e_una_cartella_di_strumenti(p: &Path) -> Result<(), String> {
    use std::os::windows::fs::MetadataExt;
    use std::path::{Component, Prefix};
    let mut lettera = None;
    let mut radice = false;
    let mut normali = 0usize;
    for componente in p.components() {
        match componente {
            Component::Prefix(pre) => match pre.kind() {
                Prefix::Disk(l) => lettera = Some(l),
                _ => return Err(format!("{} non e' un percorso di un disco locale", p.display())),
            },
            Component::RootDir => radice = true,
            Component::Normal(_) => normali += 1,
            _ => return Err(format!("{} contiene . o ..: non la tocco", p.display())),
        }
    }
    let Some(lettera) = lettera.filter(|_| radice && normali >= 1) else {
        return Err(format!("{} non e' una cartella sotto la radice di un disco", p.display()));
    };
    let meta = std::fs::symlink_metadata(p).map_err(|e| format!("{}: {e}", p.display()))?;
    if !meta.is_dir() {
        return Err(format!("{} non e' una cartella", p.display()));
    }
    if meta.file_attributes() & 0x0400 != 0 {
        return Err(format!("{} e' una giunzione o un collegamento: non la tocco", p.display()));
    }
    let disco = largo(&format!("{}:\\", lettera as char));
    const DRIVE_FIXED: u32 = 3;
    if unsafe { GetDriveTypeW(PCWSTR(disco.as_ptr())) } != DRIVE_FIXED {
        return Err(format!("{}: il disco non e' un disco fisso", p.display()));
    }
    Ok(())
}

/// Il passo privilegiato, apertura delle cartelle di strumenti: lettura ed
/// esecuzione, ereditata. Come per la prima componente, non crea il profilo.
pub fn apri_cartella_di_strumenti(p: &Path) -> Result<(), String> {
    e_una_cartella_di_strumenti(p)?;
    let chi = Identita::di_nova()?;
    concedi(p, &chi, Genere::Legge)
}

/// Il passo privilegiato, chiusura. Una cartella che non c'e' piu' non ha voci
/// da togliere.
pub fn chiudi_cartella_di_strumenti(p: &Path) -> Result<(), String> {
    if !p.exists() {
        return Ok(());
    }
    e_una_cartella_di_strumenti(p)?;
    let chi = Identita::di_nova()?;
    togli_genere(p, &chi, Genere::Legge)
}

/// Com'e' andato il passo da amministratore.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassoElevato {
    /// E' finito, con questo codice d'uscita.
    Finito(i32),
    /// Il tempo e' finito e il passo no. **Non e' un rifiuto**: elevato, da qui
    /// non lo si puo' fermare, e se la persona conferma piu' tardi parte e
    /// scrive. Chi ha annotato l'intenzione la deve tenere.
    Scaduto,
}

/// Rilancia **questo stesso eseguibile** con gli argomenti dati, chiedendo i
/// privilegi di amministratore, e aspetta. Windows mostra la richiesta
/// all'utente: se la rifiuta, torna un errore e non e' cambiato niente.
pub fn esegui_elevato(argomenti: &[String]) -> Result<PassoElevato, String> {
    let exe = std::env::current_exe().map_err(|e| format!("non so chi sono: {e}"))?;
    let exe_largo = largo_percorso(&exe);
    let mut riga: Vec<u16> = Vec::new();
    for (i, a) in argomenti.iter().enumerate() {
        if i > 0 {
            riga.push(b' ' as u16);
        }
        quota(&mut riga, a);
    }
    riga.push(0);
    let verbo = largo("runas");
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
        lpVerb: PCWSTR(verbo.as_ptr()),
        lpFile: PCWSTR(exe_largo.as_ptr()),
        lpParameters: PCWSTR(riga.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    unsafe { ShellExecuteExW(&mut info) }.map_err(|e| {
        format!("la richiesta di amministratore non e' partita o e' stata rifiutata: {e}")
    })?;
    let processo = Chiuso(info.hProcess);
    match unsafe { WaitForSingleObject(processo.0, 180_000) } {
        WAIT_OBJECT_0 => {}
        WAIT_TIMEOUT => return Ok(PassoElevato::Scaduto),
        _ => return Err(errore("non aspetto il passo da amministratore")),
    }
    let mut codice = 0u32;
    unsafe { GetExitCodeProcess(processo.0, &mut codice) }
        .map_err(|e| format!("non leggo l'esito del passo da amministratore: {e}"))?;
    Ok(PassoElevato::Finito(codice as i32))
}

// ---------------------------------------------------------------------------
// Le cartelle di terzi aperte a tutti i pacchetti (D367)
//
// Il contenitore ha il controllo d'accesso due volte: serve una voce per
// l'utente **e** una per il contenitore. Everyone non conta per il
// contenitore (provato: una cartella aperta a Everyone resta chiusa), ma
// ALL APPLICATION PACKAGES si': su una macchina vera sono comparse quattro
// cartelle cosi', fra driver e Segnalazione errori di Windows, che un comando
// confinato poteva scrivere.
//
// **Non si possono chiudere solo per il contenitore.** E' stato provato: un
// divieto intestato al suo SID, o a una capability che ha solo lui, non lo
// ferma — scrive lo stesso, con qualunque maschera; e togliere ALL
// APPLICATION PACKAGES dal suo token (LPAC) fa non partire PowerShell.
// L'unico rimedio sarebbe toccare i permessi di quelle cartelle per tutte le
// app di Windows, e non e' nostro farlo. Resta rilevarle e dirlo, e rifarlo:
// un aggiornamento del produttore le cambia.
// ---------------------------------------------------------------------------

/// Tutto cio' che modifica una cartella: scrivere dati, aggiungere in coda,
/// attributi estesi, cancellare un figlio, attributi, cancellare, cambiare i
/// permessi, cambiare il proprietario.
const SCRITTURA: u32 = 0x0000_0002
    | 0x0000_0004
    | 0x0000_0010
    | 0x0000_0040
    | 0x0000_0100
    | 0x0001_0000
    | 0x0004_0000
    | 0x0008_0000;

/// Cosa ha trovato una scansione.
#[derive(Debug, Clone, Default)]
pub struct Scansione {
    /// Quante cartelle ha esaminato.
    pub cartelle: u64,
    /// Cartelle di cui non si sono potuti leggere i permessi: non coperte.
    pub non_leggibili: u64,
    /// Cartelle di cui non si e' potuto leggere l'elenco: i loro sottoalberi
    /// non sono coperti.
    pub non_elencabili: u64,
    /// Le cartelle che il contenitore puo' scrivere perche' sono aperte ad ALL
    /// APPLICATION PACKAGES, e che nessuna voce nostra giustifica.
    pub aperte: Vec<PathBuf>,
    /// Fermata prima della fine.
    pub interrotta: bool,
}

/// Se ha una voce concessa per uno di questi SID con un diritto di scrittura.
fn aperta_a(d: &Dacl, sids: &[Identita]) -> bool {
    if d.acl.is_null() {
        return false;
    }
    let quante = unsafe { (*d.acl).AceCount };
    for i in 0..quante as u32 {
        let mut ace: *mut c_void = std::ptr::null_mut();
        if unsafe { GetAce(d.acl, i, &mut ace) }.is_err() {
            continue;
        }
        let testa = unsafe { *(ace as *const ACE_HEADER) };
        if testa.AceType != ACCESS_ALLOWED_ACE_TYPE || testa.AceFlags & SOLO_EREDITATA != 0 {
            continue;
        }
        let voce = unsafe { &*(ace as *const ACCESS_ALLOWED_ACE) };
        // GENERIC_WRITE e GENERIC_ALL contano: nei descrittori non sempre sono mappati.
        if voce.Mask & (SCRITTURA | 0x4000_0000 | 0x1000_0000) == 0 {
            continue;
        }
        let sid = PSID(&voce.SidStart as *const u32 as *mut c_void);
        if sids.iter().any(|s| unsafe { EqualSid(sid, s.sid) }.is_ok()) {
            return true;
        }
    }
    false
}

/// Percorre **tutto** `radice`, senza limite di livelli, e dice quali
/// cartelle il contenitore puo' scrivere perche' aperte ad ALL APPLICATION
/// PACKAGES. La voce ereditata in una cartella e' la stessa che c'e' sopra:
/// si guarda comunque ogni cartella, perche' e' li' che il contenitore scrive.
///
/// Il sospetto lo da' l'ispezione delle voci (veloce); la conferma, per le
/// poche sospette, la da' `AccessCheck` col token del contenitore — la stessa
/// regola del kernel, non una nostra stima. Le giunzioni non si seguono.
pub fn cerca_cartelle_aperte(
    radice: &Path,
    sonda: &Sonda,
    annulla: &AtomicBool,
    avanzamento: &dyn Fn(u64),
) -> Scansione {
    use std::os::windows::fs::MetadataExt;
    let mut s = Scansione::default();
    let sids: Vec<Identita> = ["S-1-15-2-1", "S-1-15-2-2", INTERNET_CLIENT]
        .iter()
        .filter_map(|t| Identita::da_stringa(t).ok())
        .collect();
    let mut pila: Vec<PathBuf> = vec![radice.to_path_buf()];
    while let Some(p) = pila.pop() {
        if annulla.load(Ordering::Relaxed) {
            s.interrotta = true;
            break;
        }
        s.cartelle += 1;
        if s.cartelle % 5000 == 0 {
            avanzamento(s.cartelle);
        }
        match leggi_dacl(&p) {
            Err(_) => s.non_leggibili += 1,
            Ok(d) => {
                if aperta_a(&d, &sids)
                    && [0x2u32, 0x4, 0x40]
                        .iter()
                        .any(|m| puo(&p, sonda, *m).unwrap_or(false))
                {
                    s.aperte.push(p.clone());
                }
            }
        }
        match std::fs::read_dir(&p) {
            Err(_) => s.non_elencabili += 1,
            Ok(figli) => {
                for f in figli.flatten() {
                    if let Ok(m) = f.metadata() {
                        if m.is_dir() && m.file_attributes() & 0x0400 == 0 {
                            pila.push(f.path());
                        }
                    }
                }
            }
        }
    }
    s
}

/// `FILE_PERSISTENT_ACLS`: il volume salva i permessi. Il criterio e' questo
/// flag, che Windows riporta per il volume, e non il nome del formato: NTFS e
/// ReFS (un Dev Drive di Windows 11 e' ReFS) i permessi li hanno, FAT ed exFAT no.
fn ha_permessi_salvati(flag_del_volume: u32) -> bool {
    flag_del_volume & 0x0000_0008 != 0
}

/// I dischi fissi del PC: prima quelli che salvano i permessi (NTFS, ReFS),
/// poi gli altri (FAT, exFAT...), dove il contenitore **non e' confinato
/// affatto** e va detto.
pub fn dischi_fissi() -> (Vec<PathBuf>, Vec<(PathBuf, String)>) {
    let mut ntfs = Vec::new();
    let mut altri = Vec::new();
    let maschera = unsafe { GetLogicalDrives() };
    for i in 0..26u32 {
        if maschera & (1 << i) == 0 {
            continue;
        }
        let radice = format!("{}:\\", (b'A' + i as u8) as char);
        let largo_radice = largo(&radice);
        const DRIVE_FIXED: u32 = 3;
        if unsafe { GetDriveTypeW(PCWSTR(largo_radice.as_ptr())) } != DRIVE_FIXED {
            continue;
        }
        let mut nome = [0u16; 32];
        let mut flag = 0u32;
        let letto = unsafe {
            GetVolumeInformationW(
                PCWSTR(largo_radice.as_ptr()),
                None,
                None,
                None,
                Some(&mut flag as *mut u32),
                Some(&mut nome),
            )
        };
        if letto.is_err() {
            continue;
        }
        let fine = nome.iter().position(|c| *c == 0).unwrap_or(nome.len());
        let formato = String::from_utf16_lossy(&nome[..fine]);
        if ha_permessi_salvati(flag) {
            ntfs.push(PathBuf::from(radice));
        } else {
            altri.push((PathBuf::from(radice), formato));
        }
    }
    (ntfs, altri)
}

/// Abbassa (o rialza) la priorita' del filo corrente: una scansione di tutti i
/// dischi dura minuti e non deve rubare il PC a chi lo sta usando.
pub fn priorita_bassa(bassa: bool) {
    unsafe {
        let _ = SetThreadPriority(
            GetCurrentThread(),
            if bassa { THREAD_PRIORITY_BELOW_NORMAL } else { THREAD_PRIORITY_NORMAL },
        );
    }
}

/// Le capability del contenitore, con tutto quello a cui puntano: devono
/// vivere finche' il processo non e' partito.
struct Capacita {
    _contenitore: Identita,
    _sids: Vec<Identita>,
    _voci: Vec<SID_AND_ATTRIBUTES>,
    sc: SECURITY_CAPABILITIES,
}

impl Capacita {
    fn nuova(chi: &Identita, rete: bool) -> Result<Capacita, String> {
        let contenitore = Identita::da_stringa(&chi.stringa())?;
        let mut sids = Vec::new();
        let mut voci: Vec<SID_AND_ATTRIBUTES> = Vec::new();
        if rete {
            let internet = Identita::da_stringa(INTERNET_CLIENT)?;
            voci.push(SID_AND_ATTRIBUTES {
                Sid: internet.sid,
                Attributes: SE_GROUP_ENABLED,
            });
            sids.push(internet);
        }
        let sc = SECURITY_CAPABILITIES {
            AppContainerSid: contenitore.sid,
            Capabilities: if voci.is_empty() {
                std::ptr::null_mut()
            } else {
                voci.as_mut_ptr()
            },
            CapabilityCount: voci.len() as u32,
            Reserved: 0,
        };
        Ok(Capacita {
            _contenitore: contenitore,
            _sids: sids,
            _voci: voci,
            sc,
        })
    }
}

/// L'elenco degli attributi di un processo, con la sua memoria.
struct ListaAttributi {
    _memoria: Vec<u64>,
    lista: LPPROC_THREAD_ATTRIBUTE_LIST,
}

impl ListaAttributi {
    fn nuova(quanti: u32) -> Result<ListaAttributi, String> {
        let mut lunghezza = 0usize;
        unsafe {
            let _ = InitializeProcThreadAttributeList(None, quanti, None, &mut lunghezza);
        }
        if lunghezza == 0 {
            return Err(errore("non so quanto e' grande l'elenco degli attributi"));
        }
        let mut memoria = vec![0u64; lunghezza.div_ceil(8)];
        let lista = LPPROC_THREAD_ATTRIBUTE_LIST(memoria.as_mut_ptr() as *mut c_void);
        unsafe {
            InitializeProcThreadAttributeList(Some(lista), quanti, None, &mut lunghezza)
                .map_err(|e| format!("non preparo gli attributi del processo: {e}"))?;
        }
        Ok(ListaAttributi {
            _memoria: memoria,
            lista,
        })
    }

    fn imposta(&mut self, attributo: u32, valore: *const c_void, dimensione: usize) -> Result<(), String> {
        unsafe {
            UpdateProcThreadAttribute(
                self.lista,
                0,
                attributo as usize,
                Some(valore),
                dimensione,
                None,
                None,
            )
        }
        .map_err(|e| format!("non imposto un attributo del processo: {e}"))
    }
}

impl Drop for ListaAttributi {
    fn drop(&mut self) {
        unsafe { DeleteProcThreadAttributeList(self.lista) };
    }
}

/// Fa partire il comando, dentro un job e — se ci sono permessi — dentro il
/// contenitore. Torna appena il processo e' partito.
///
/// **Qui non si toccano i permessi delle cartelle**: vanno preparate prima,
/// con `prepara_cartella`, perche' chi le prepara le deve anche annotare. Se
/// non lo sono, il comando parte lo stesso e il sistema gli nega l'accesso.
pub fn lancia(c: &Comando, permessi: Option<&Permessi>) -> Result<InCorsa, String> {
    let programma = trova(&c.programma)?;
    // Prima di tutto il resto: se il demone e' elevato e i poteri non si tolgono,
    // il comando non parte e non si apre niente.
    let base = token_di_base(permessi.is_some())?;
    // Senza poteri da un demone elevato la console nuova non va: si eredita quella
    // del demone, che prima si rende solo sua.
    let console_ereditata = base.is_some();
    if console_ereditata {
        console_privata()?;
    }
    let capacita = match permessi {
        Some(p) => {
            let chi = assicura_profilo()?;
            Some(Capacita::nuova(&chi, !p.senza_rete)?)
        }
        None => None,
    };
    let job = Arc::new(Job::nuovo()?);

    let (out_l, out_s) = tubo()?;
    let (err_l, err_s) = tubo()?;
    let dentro = nul()?;

    let mut riga: Vec<u16> = Vec::new();
    quota(&mut riga, &programma.to_string_lossy());
    for a in &c.argomenti {
        riga.push(b' ' as u16);
        quota(&mut riga, a);
    }
    riga.push(0);
    let programma_largo = largo_percorso(&programma);
    let cartella = c.cartella.as_ref().map(|d| largo_percorso(d));
    let blocco = ambiente(&c.ambiente);

    // Solo i tre handle standard passano al figlio, nominati uno per
    // uno: senza questo elenco un comando erediterebbe ogni handle
    // ereditabile del demone, compresi i tubi di un altro comando in
    // corso, che non si chiuderebbero piu'.
    let handles = [dentro.0, out_s.0, err_s.0];
    let mut attributi = ListaAttributi::nuova(if capacita.is_some() { 2 } else { 1 })?;
    if let Some(cap) = &capacita {
        attributi.imposta(
            PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,
            &cap.sc as *const SECURITY_CAPABILITIES as *const c_void,
            std::mem::size_of::<SECURITY_CAPABILITIES>(),
        )?;
    }
    attributi.imposta(
        PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
        handles.as_ptr() as *const c_void,
        std::mem::size_of_val(&handles),
    )?;

    let si = STARTUPINFOEXW {
        StartupInfo: STARTUPINFOW {
            cb: std::mem::size_of::<STARTUPINFOEXW>() as u32,
            dwFlags: STARTF_USESTDHANDLES,
            hStdInput: dentro.0,
            hStdOutput: out_s.0,
            hStdError: err_s.0,
            ..Default::default()
        },
        lpAttributeList: attributi.lista,
    };
    let mut pi = PROCESS_INFORMATION::default();
    // `CREATE_NO_WINDOW`: una console nuova e invisibile. Con
    // `DETACHED_PROCESS`, cioe' senza console, PowerShell non parte.
    let flag = flag_di_creazione(console_ereditata);
    unsafe {
        crea_processo(
            base.as_ref(),
            PCWSTR(programma_largo.as_ptr()),
            &mut riga,
            true,
            flag,
            Some(blocco.as_ptr() as *const c_void),
            cartella
                .as_ref()
                .map(|c| PCWSTR(c.as_ptr()))
                .unwrap_or(PCWSTR::null()),
            &si.StartupInfo,
            &mut pi,
        )
    }
    .map_err(|e| format!("il comando non e' partito: {e}"))?;
    let processo = Chiuso(pi.hProcess);
    let filo = Chiuso(pi.hThread);

    // Nel job **prima** di farlo correre: un processo assegnato dopo
    // aver gia' fatto un figlio lascerebbe quel figlio fuori.
    let assegnato = unsafe { AssignProcessToJobObject(job.h, processo.0) };
    if let Err(e) = assegnato {
        job.ferma();
        unsafe {
            let _ = TerminateProcess(processo.0, 1);
        }
        return Err(format!("non metto il comando nel job object: {e}"));
    }
    if unsafe { ResumeThread(filo.0) } == u32::MAX {
        job.ferma();
        return Err(errore("il comando non riparte"));
    }
    drop(filo);
    // I capi del figlio si chiudono qui: se restassero aperti nel demone,
    // i lettori non vedrebbero mai la fine.
    drop(out_s);
    drop(err_s);
    drop(dentro);

    Ok(InCorsa {
        job,
        processo,
        stdout: leggi_tutto(out_l),
        stderr: leggi_tutto(err_l),
    })
}

impl InCorsa {
    /// Aspetta che finisca, entro la scadenza. Scaduto, ferma il job:
    /// cade il comando con tutto quello che ha avviato.
    pub fn attendi(self, scadenza: Duration) -> Esito {
        let inizio = Instant::now();
        let ms = scadenza.as_millis().min(u32::MAX as u128 - 1) as u32;
        let finito = unsafe { WaitForSingleObject(self.processo.0, ms) } == WAIT_OBJECT_0;
        let mut scaduto = !finito;
        if !finito {
            self.job.ferma();
        }
        // I tubi si chiudono quando si chiude l'ultimo che li tiene: un
        // programma avviato dal comando e lasciato in piedi li terrebbe
        // aperti. Si aspetta quel che resta della scadenza, poi si ferma
        // il job, come nel caso scaduto. Fermato il job, i lettori
        // finiscono in un attimo: gli si lascia un respiro per raccogliere
        // quel che il comando aveva scritto prima di cadere.
        let respiro = Duration::from_secs(3);
        let lettori = (self.stdout, self.stderr);
        // Un comando finito a un soffio dalla scadenza non deve risultare scaduto
        // perche' ai lettori non resta il tempo di svuotare i tubi.
        let resta = if scaduto {
            respiro
        } else {
            scadenza.saturating_sub(inizio.elapsed()).max(Duration::from_millis(500))
        };
        let pronti = lettori_finiti(&lettori, resta) || {
            scaduto = true;
            self.job.ferma();
            lettori_finiti(&lettori, respiro)
        };
        // Un lettore che non finisce nemmeno a job fermato tiene un tubo
        // che nessuno chiudera': non lo si aspetta, si lascia andare.
        let (stdout, stderr) = if pronti {
            (
                lettori.0.join().unwrap_or_default(),
                lettori.1.join().unwrap_or_default(),
            )
        } else {
            (Vec::new(), Vec::new())
        };
        let mut codice = 0u32;
        let letto = unsafe { GetExitCodeProcess(self.processo.0, &mut codice) }.is_ok();
        Esito {
            codice: if scaduto || !letto {
                None
            } else {
                Some(codice as i32)
            },
            stdout,
            stderr,
            scaduto,
        }
    }
}

type Lettori = (std::thread::JoinHandle<Vec<u8>>, std::thread::JoinHandle<Vec<u8>>);

/// Se i due lettori hanno finito entro il tempo dato.
fn lettori_finiti(lettori: &Lettori, entro: Duration) -> bool {
    let scadenza = Instant::now() + entro;
    while !lettori.0.is_finished() || !lettori.1.is_finished() {
        if Instant::now() >= scadenza {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    true
}

/// Fa partire il comando e aspetta la fine: `lancia` piu' `attendi`.
pub fn esegui(c: &Comando, permessi: Option<&Permessi>, scadenza: Duration) -> Result<Esito, String> {
    Ok(lancia(c, permessi)?.attendi(scadenza))
}

#[cfg(test)]
mod prove {
    use super::*;
    use std::sync::Mutex;

    /// Le prove toccano il profilo del contenitore, che e' uno solo per
    /// utente: una alla volta.
    static SERIALE: Mutex<()> = Mutex::new(());

    fn seriale() -> std::sync::MutexGuard<'static, ()> {
        SERIALE.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// `icacls` come lo userebbe una persona: per costruire cartelle con i
    /// permessi che si trovano in giro, e per leggerli dopo.
    fn icacls(args: &[&str]) -> String {
        let o = std::process::Command::new("icacls").args(args).output().expect("icacls");
        String::from_utf8_lossy(&o.stdout).into_owned()
    }

    fn nuova_cartella(nome: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("nova-win-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Toglie le voci del contenitore e la cartella.
    fn butta(d: &Path, chi: &Identita) {
        let _ = revoca(d, chi);
        let s = d.display().to_string();
        let u = format!("{}:(OI)(CI)F", std::env::var("USERNAME").unwrap_or_default());
        icacls(&[&s, "/grant", &u, "/T", "/C", "/Q"]);
        let _ = std::fs::remove_dir_all(d);
    }

    /// Toglie la cartella anche se la prova cade a meta': senza, una prova
    /// fallita lascia una cartella nella radice del disco.
    struct Pulizia(PathBuf);

    impl Drop for Pulizia {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn ps(script: &str, permessi: Option<&Permessi>, cartella: Option<PathBuf>) -> Esito {
        let comando = Comando {
            programma: "powershell".into(),
            argomenti: vec![
                "-NoProfile".into(),
                "-NonInteractive".into(),
                "-ExecutionPolicy".into(),
                "Bypass".into(),
                "-Command".into(),
                script.into(),
            ],
            cartella,
            ambiente: vec![],
        };
        esegui(&comando, permessi, Duration::from_secs(90)).expect("il comando parte")
    }

    fn uscita(e: &Esito) -> String {
        format!(
            "codice {:?}\nstdout: {}\nstderr: {}",
            e.codice,
            String::from_utf8_lossy(&e.stdout),
            String::from_utf8_lossy(&e.stderr)
        )
    }

    /// Quante volte piu' veloce deve essere la voce che non ripassa i figli.
    const SOGLIA: u32 = 5;

    /// Un disco ha i permessi se Windows lo dice, non se si chiama NTFS: ReFS
    /// (un Dev Drive) li ha, e prima veniva trattato come un disco senza.
    #[test]
    fn windows_il_criterio_dei_dischi_e_il_flag_dei_permessi_non_il_nome() {
        const CASE_PRESERVED: u32 = 0x0000_0002;
        const UNICODE: u32 = 0x0000_0004;
        const PERSISTENT_ACLS: u32 = 0x0000_0008;
        assert!(ha_permessi_salvati(CASE_PRESERVED | UNICODE | PERSISTENT_ACLS), "NTFS e ReFS portano il flag");
        assert!(!ha_permessi_salvati(CASE_PRESERVED | UNICODE), "FAT ed exFAT no");
        assert!(!ha_permessi_salvati(0));
    }

    /// Il token da cui nascono sonda e comandi: quello del demone se non e'
    /// elevato, uno senza poteri se lo e'. Vale nei due modi, e la CI di
    /// Windows gira da amministratore.
    #[test]
    fn windows_il_token_di_base_non_e_mai_di_amministratore() {
        // Senza recinto non c'e' confine da proteggere: il token e' quello del demone.
        assert!(token_di_base(false).unwrap().is_none(), "un comando non confinato non cambia token");
        let base = token_di_base(true).expect("il token di base si ottiene");
        match stato_elevazione() {
            Some(false) => assert!(base.is_none(), "un demone normale non cambia il suo token"),
            _ => {
                let t = base.expect("un demone elevato deve dare un token senza poteri");
                assert_eq!(elevazione_di(t.0), Some(false), "il token di base e' ancora elevato");
            }
        }
    }

    /// Un comando di NOVA non riceve mai i poteri dell'amministratore, nemmeno
    /// se il demone li ha. La cartella «fortino» la scrivono solo gli
    /// Amministratori, il sistema e i contenitori (ALL APPLICATION PACKAGES), non
    /// l'utente: ci riesce solo un comando che ha i poteri dell'amministratore.
    /// Da utente normale non c'e' niente da provare: la CI di Windows, che gira
    /// da amministratore, e' la guardia.
    #[test]
    fn windows_un_comando_non_riceve_i_poteri_dell_amministratore() {
        if !e_elevato() {
            eprintln!("saltata: la prova riguarda un demone elevato");
            return;
        }
        let _s = seriale();
        let chi = assicura_profilo().unwrap();
        let so = sonda(&chi).unwrap();
        let base = nuova_cartella("elevato");
        let (lavoro, fortino) = (base.join("lavoro"), base.join("fortino"));
        std::fs::create_dir_all(&lavoro).unwrap();
        std::fs::create_dir_all(&fortino).unwrap();
        let f = fortino.display().to_string();
        // `/inheritance:r` converte le voci ereditate in esplicite e non le toglie:
        // dentro `%TEMP%` l'utente ci resterebbe, e un comando senza poteri ci
        // scriverebbe lo stesso. Si toglie a mano, e poi si controlla.
        let utente = std::env::var("USERNAME").unwrap_or_default();
        icacls(&[&f, "/inheritance:r"]);
        icacls(&[&f, "/remove:g", &utente]);
        icacls(&[&f, "/remove:g", "*S-1-3-4"]);
        icacls(&[
            &f,
            "/grant:r",
            "*S-1-5-32-544:(OI)(CI)F",
            "/grant:r",
            "*S-1-15-2-1:(OI)(CI)F",
            "/grant:r",
            "*S-1-5-18:(OI)(CI)F",
        ]);
        // Le voci, senza il percorso: il percorso della cartella contiene il nome dell'utente.
        let voci = icacls(&[&f]).replace(&f, "");
        assert!(
            !voci.to_lowercase().contains(&format!("\\{}:", utente.to_lowercase())),
            "il fortino non e' un fortino: l'utente ci puo' ancora scrivere\n{voci}"
        );
        assert!(prepara_cartella(&lavoro, &chi, Genere::Scrive, &so).unwrap());
        let script = format!(
            "{T}{}",
            r#"
T 'scrive-lavoro' { Set-Content -LiteralPath '__L__\f.txt' -Value ok -ErrorAction Stop }
T 'scrive-fortino' { Set-Content -LiteralPath '__F__\f.txt' -Value ok -ErrorAction Stop }
"#
        )
        .replace("__L__", &lavoro.display().to_string())
        .replace("__F__", &fortino.display().to_string());
        let permessi = Permessi { scrive: vec![lavoro.clone()], legge: vec![], senza_rete: false };
        let esito = ps(&script, Some(&permessi), None);
        let o = String::from_utf8_lossy(&esito.stdout).to_string();
        let scritto_nel_fortino = fortino.join("f.txt").exists();
        let scritto_nel_lavoro = lavoro.join("f.txt").is_file();
        butta(&lavoro, &chi);
        let _ = std::fs::remove_dir_all(&base);
        assert!(o.contains("scrive-lavoro=SI"), "i comandi devono girare davvero\n{}", uscita(&esito));
        assert!(scritto_nel_lavoro, "il comando non ha scritto in write_roots");
        assert!(
            o.contains("scrive-fortino=NO") && !scritto_nel_fortino,
            "un comando di NOVA ha scritto dove solo un amministratore scrive\n{}",
            uscita(&esito)
        );
    }

    /// Se il demone e' elevato e i poteri non si possono togliere, il comando
    /// non parte e il rifiuto dice perche'. Far fallire `CreateRestrictedToken`
    /// da fuori non si puo': si prova il testo che il codice usa.
    #[test]
    fn windows_il_rifiuto_per_un_demone_elevato_nomina_l_amministratore() {
        let testo = rifiuto_per_demone_elevato("non creo il token ridotto: errore di prova");
        assert!(testo.to_lowercase().contains("amministratore"), "{testo}");
        assert!(testo.contains("non lo lancio"), "{testo}");
        assert!(testo.contains("errore di prova"), "dice anche il perche': {testo}");
    }

    /// Con il demone elevato il comando eredita la console (una nuova non parte); in
    /// ogni altro caso se ne crea una nuova, come sempre.
    #[test]
    fn windows_la_console_si_eredita_solo_per_un_demone_elevato() {
        assert_ne!(flag_di_creazione(false).0 & CREATE_NO_WINDOW.0, 0, "console nuova");
        assert_eq!(flag_di_creazione(true).0 & CREATE_NO_WINDOW.0, 0, "console ereditata");
        for f in [flag_di_creazione(false), flag_di_creazione(true)] {
            assert_ne!(f.0 & CREATE_SUSPENDED.0, 0, "parte sempre sospeso: va nel job prima di correre");
            assert_ne!(f.0 & EXTENDED_STARTUPINFO_PRESENT.0, 0);
        }
    }

    /// Dopo `console_privata`, alla console del processo e' agganciato solo il
    /// processo stesso: nessun terminale la condivide, quindi un comando che la
    /// eredita non ha tasti altrui da leggere (un comando confinato che eredita
    /// una console puo' leggere e consumarne l'input; misurato). Vale solo da
    /// elevato, che e' quando la console si eredita. Si prende il blocco delle
    /// prove: un comando in corso, agganciato alla stessa console, falserebbe il
    /// conteggio.
    #[test]
    fn windows_la_console_del_demone_elevato_e_solo_sua() {
        if !e_elevato() {
            eprintln!("saltata: la console si eredita solo con un demone elevato");
            return;
        }
        let _s = seriale();
        console_privata().expect("una console tutta sua");
        let mut elenco = [0u32; 8];
        let agganciati = unsafe { GetConsoleProcessList(elenco.as_mut_ptr(), 8) };
        // Se non e' solo lui, la prova dice chi altro c'e': senza i nomi, «2 processi» non si capisce.
        let altri: Vec<String> = elenco
            .iter()
            .take(agganciati.min(8) as usize)
            .filter(|p| **p != std::process::id())
            .map(|p| {
                std::process::Command::new("tasklist")
                    .args(["/FO", "CSV", "/NH", "/FI", &format!("PID eq {p}")])
                    .output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    .unwrap_or_default()
            })
            .collect();
        assert_eq!(
            agganciati, 1,
            "alla console del demone sono agganciati {agganciati} processi (devono essere solo lui): {elenco:?}; gli altri: {altri:?}"
        );
        assert_eq!(elenco[0], std::process::id(), "l'unico agganciato non e' il demone");
    }

    /// Se il DACL della cartella e' protetto, cioe' non eredita dal padre.
    fn protetto(d: &Path) -> bool {
        let dacl = leggi_dacl(d).unwrap();
        let (mut controllo, mut revisione) = (0u16, 0u32);
        unsafe { GetSecurityDescriptorControl(dacl.descrittore, &mut controllo, &mut revisione) }.unwrap();
        controllo & 0x1000 != 0 // SE_DACL_PROTECTED
    }

    /// Una cartella con l'ereditarieta' disattivata (`icacls /inheritance:r`) e'
    /// una scelta di chi l'ha fatta: scriverci o togliere la voce del
    /// contenitore non deve riattivarla, perche' riattivata comincerebbe a
    /// ereditare dal padre e avrebbe permessi **piu' larghi** di prima. E il
    /// contrario: una cartella che eredita non deve diventare protetta.
    #[test]
    fn windows_una_voce_non_cambia_la_protezione_dell_ereditarieta() {
        let _s = seriale();
        let chi = assicura_profilo().unwrap();
        let utente = std::env::var("USERNAME").unwrap_or_default();
        for protetta in [true, false] {
            for genere in [Genere::Scrive, Genere::Legge, Genere::Antenata] {
                let d = nuova_cartella(&format!("prot-{protetta}-{}", genere.nome()));
                let s = d.display().to_string();
                if protetta {
                    icacls(&[&s, "/inheritance:r"]);
                    icacls(&[&s, "/grant", &format!("{utente}:(OI)(CI)F")]);
                }
                assert_eq!(protetto(&d), protetta, "la cartella di partenza non e' come dovrebbe");
                concedi(&d, &chi, genere).unwrap();
                assert!(ha_permesso(&d, &chi, genere).unwrap());
                assert_eq!(
                    protetto(&d),
                    protetta,
                    "dopo `concedi` ({}) la protezione e' cambiata",
                    genere.nome()
                );
                if genere == Genere::Antenata {
                    revoca_locale(&d, &chi).unwrap();
                } else {
                    revoca(&d, &chi).unwrap();
                }
                assert_eq!(
                    protetto(&d),
                    protetta,
                    "dopo la revoca ({}) la protezione e' cambiata",
                    genere.nome()
                );
                assert!(!ha_permesso(&d, &chi, genere).unwrap(), "la voce e' rimasta");
                butta(&d, &chi);
            }
        }
    }

    const T: &str = "function T($n,[scriptblock]$b){ try { & $b; \"$n=SI\" } catch { \"$n=NO\" } }; ";

    /// Il SID di `nova.recinto` derivato dal nome in UTF-16. Passato in ANSI,
    /// Windows registra un profilo di sei caratteri senza senso e il SID e'
    /// un altro: e' successo, e questa prova lo avrebbe preso.
    #[test]
    fn windows_l_identita_e_quella_del_nome_giusto() {
        assert_eq!(
            Identita::di_nova().unwrap().stringa(),
            "S-1-15-2-3068841503-1630141745-868897104-1338301600-1924873177-1596986457-3595769742"
        );
    }

    /// Il profilo si registra e si toglie, e non lascia la cartella.
    #[test]
    fn windows_il_profilo_si_registra_e_si_toglie() {
        let _s = seriale();
        let chi = assicura_profilo().unwrap();
        assert_eq!(chi.stringa(), Identita::di_nova().unwrap().stringa());
        let cartella = temporanea_del_profilo().unwrap().parent().unwrap().parent().unwrap().to_path_buf();
        assert!(cartella.is_dir(), "il profilo non ha una cartella: {}", cartella.display());
        assert_eq!(cartella.file_name().unwrap(), NOME_PROFILO);
        elimina_profilo().unwrap();
        assert!(!cartella.exists(), "la cartella del profilo e' rimasta");
        elimina_profilo().expect("toglierlo due volte non e' un errore");
        assicura_profilo().unwrap();
    }

    /// Il cuore: dentro si scrive; fuori non si crea, non si sovrascrive, non
    /// si cancella e — a differenza del token ristretto — non si legge; le
    /// cartelle di strumenti si leggono e non si toccano.
    #[test]
    fn windows_rifiuta_quello_che_il_recinto_non_consente() {
        let _s = seriale();
        let chi = assicura_profilo().unwrap();
        let so = sonda(&chi).unwrap();
        let base = nuova_cartella("rifiuta");
        let (dentro, fuori, strumenti) = (base.join("dentro"), base.join("fuori"), base.join("strumenti"));
        for d in [&dentro, &fuori, &strumenti] {
            std::fs::create_dir_all(d).unwrap();
        }
        std::fs::write(fuori.join("da_modificare.txt"), "originale").unwrap();
        std::fs::write(fuori.join("da_cancellare.txt"), "resto").unwrap();
        std::fs::write(fuori.join("segreto.txt"), "non si legge").unwrap();
        std::fs::write(strumenti.join("strumento.txt"), "si legge").unwrap();
        assert!(prepara_cartella(&dentro, &chi, Genere::Scrive, &so).unwrap());
        assert!(prepara_cartella(&strumenti, &chi, Genere::Legge, &so).unwrap());

        let script = format!(
            "{T}{}",
            r#"
T 'scrive-dentro' { Set-Content -LiteralPath '__D__\f.txt' -Value ok -ErrorAction Stop }
T 'crea-fuori' { Set-Content -LiteralPath '__F__\nuovo.txt' -Value x -ErrorAction Stop }
T 'sovrascrive-fuori' { Set-Content -LiteralPath '__F__\da_modificare.txt' -Value cambiato -ErrorAction Stop }
T 'cancella-fuori' { Remove-Item -LiteralPath '__F__\da_cancellare.txt' -ErrorAction Stop }
T 'legge-fuori' { [IO.File]::ReadAllText('__F__\segreto.txt') | Out-Null }
T 'legge-strumenti' { [IO.File]::ReadAllText('__S__\strumento.txt') | Out-Null }
T 'scrive-strumenti' { Set-Content -LiteralPath '__S__\x.txt' -Value x -ErrorAction Stop }
T 'cancella-strumenti' { Remove-Item -LiteralPath '__S__\strumento.txt' -ErrorAction Stop }
"#
        )
        .replace("__D__", &dentro.display().to_string())
        .replace("__F__", &fuori.display().to_string())
        .replace("__S__", &strumenti.display().to_string());
        let permessi = Permessi {
            scrive: vec![dentro.clone()],
            legge: vec![strumenti.clone()],
            senza_rete: false,
        };
        let esito = ps(&script, Some(&permessi), None);
        let o = String::from_utf8_lossy(&esito.stdout).to_string();
        for (voce, atteso) in [
            ("scrive-dentro", "SI"),
            ("crea-fuori", "NO"),
            ("sovrascrive-fuori", "NO"),
            ("cancella-fuori", "NO"),
            ("legge-fuori", "NO"),
            ("legge-strumenti", "SI"),
            ("scrive-strumenti", "NO"),
            ("cancella-strumenti", "NO"),
        ] {
            assert!(o.contains(&format!("{voce}={atteso}")), "{voce} doveva essere {atteso}\n{}", uscita(&esito));
        }
        // E il sistema operativo lo conferma, non solo quel che il comando dice.
        assert!(dentro.join("f.txt").is_file());
        assert!(!fuori.join("nuovo.txt").exists());
        assert_eq!(std::fs::read_to_string(fuori.join("da_modificare.txt")).unwrap(), "originale");
        assert!(fuori.join("da_cancellare.txt").is_file(), "fuori dal recinto ha cancellato");
        assert!(!strumenti.join("x.txt").exists());
        assert!(strumenti.join("strumento.txt").is_file(), "ha cancellato uno strumento");
        butta(&dentro, &chi);
        butta(&strumenti, &chi);
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Le cartelle di terzi scrivibili da tutti: Everyone non basta al
    /// contenitore (chiuso), ALL APPLICATION PACKAGES si' (aperto). La sonda
    /// lo vede come lo vede la scrittura vera: e' il metodo del controllo
    /// periodico.
    #[test]
    fn windows_la_sonda_vede_le_cartelle_aperte_a_tutti() {
        let _s = seriale();
        let chi = assicura_profilo().unwrap();
        let so = sonda(&chi).unwrap();
        let base = nuova_cartella("aperte");
        let (everyone, pacchetti, normale) =
            (base.join("everyone"), base.join("pacchetti"), base.join("normale"));
        for d in [&everyone, &pacchetti, &normale] {
            std::fs::create_dir_all(d).unwrap();
        }
        icacls(&[&everyone.display().to_string(), "/grant", "*S-1-1-0:(OI)(CI)F"]);
        icacls(&[&pacchetti.display().to_string(), "/grant", "*S-1-15-2-1:(OI)(CI)F"]);

        let script = format!(
            "{T}{}",
            r#"
T 'everyone' { Set-Content -LiteralPath '__E__\f.txt' -Value x -ErrorAction Stop }
T 'pacchetti' { Set-Content -LiteralPath '__P__\f.txt' -Value x -ErrorAction Stop }
T 'normale' { Set-Content -LiteralPath '__N__\f.txt' -Value x -ErrorAction Stop }
"#
        )
        .replace("__E__", &everyone.display().to_string())
        .replace("__P__", &pacchetti.display().to_string())
        .replace("__N__", &normale.display().to_string());
        let permessi = Permessi::default();
        let esito = ps(&script, Some(&permessi), None);
        let o = String::from_utf8_lossy(&esito.stdout).to_string();
        for (voce, cartella, scritta) in [
            ("everyone", &everyone, false),
            ("pacchetti", &pacchetti, true),
            ("normale", &normale, false),
        ] {
            assert_eq!(cartella.join("f.txt").is_file(), scritta, "{voce}: la scrittura vera\n{}", uscita(&esito));
            assert_eq!(
                puo(cartella, &so, MODIFICA).unwrap(),
                scritta,
                "{voce}: la sonda non dice quel che fa la scrittura vera\n{o}"
            );
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Il proprietario di `d` e' l'utente di questo processo? Si confrontano i
    /// SID e non i nomi: i nomi cambiano con la lingua e col dominio
    /// (`MACCHINA\utente`), e leggerli dal testo di PowerShell non dice perche'
    /// fallisce quando fallisce.
    fn il_proprietario_e_l_utente(d: &Path) -> bool {
        use windows::core::BOOL;
        use windows::Win32::Security::{GetSecurityDescriptorOwner, TokenUser, TOKEN_USER};
        let dacl = leggi_dacl(d).expect("leggo il proprietario");
        let mut proprietario = PSID::default();
        let mut predefinito = BOOL::default();
        unsafe { GetSecurityDescriptorOwner(dacl.descrittore, &mut proprietario, &mut predefinito) }
            .expect("il proprietario della cartella");
        let mut token = HANDLE::default();
        unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }.expect("il token");
        let token = Chiuso(token);
        let mut serve = 0u32;
        let _ = unsafe { GetTokenInformation(token.0, TokenUser, None, 0, &mut serve) };
        // u64 per l'allineamento: sotto c'e' una struttura con puntatori.
        let mut buf = vec![0u64; (serve as usize).div_ceil(8).max(1)];
        unsafe {
            GetTokenInformation(token.0, TokenUser, Some(buf.as_mut_ptr() as *mut c_void), serve, &mut serve)
        }
        .expect("l'utente del token");
        let utente = unsafe { &*(buf.as_ptr() as *const TOKEN_USER) };
        unsafe { EqualSid(proprietario, utente.User.Sid) }.is_ok()
    }

    /// La cartella che concede solo a SYSTEM e a «DIRITTI PROPRIETARIO» (OWNER
    /// RIGHTS, S-1-3-4): quella che crea `mkdtemp` di Python 3.13. Senza la voce
    /// dell'utente, dunque a chi la possiede e a nessun altro.
    fn solo_sistema_e_proprietario(s: &str, utente: &str) {
        icacls(&[s, "/inheritance:r"]);
        icacls(&[s, "/remove:g", utente]);
        icacls(&[s, "/grant:r", "*S-1-5-18:(OI)(CI)F"]);
        icacls(&[s, "/grant:r", "*S-1-3-4:(OI)(CI)F"]);
        let voci = icacls(&[s]).replace(s, "").to_lowercase();
        assert!(!voci.contains(&format!("\\{}:", utente.to_lowercase())), "l'utente c'e' gia'\n{voci}");
    }

    /// Le cartelle che concedono solo a «DIRITTI PROPRIETARIO» — quelle di
    /// `mkdtemp` di Python 3.13 — erano il caso che col token ristretto
    /// obbligava a scrivere anche una voce per l'utente. Col contenitore
    /// funzionano con la sola voce di NOVA, **se le possiede l'utente**.
    ///
    /// Chi le possiede dipende da chi le ha create: un processo normale ne e' il
    /// proprietario, uno elevato no (la possiede il gruppo Amministratori:
    /// misurato). Qui si prova a dare la cartella all'utente, e si guarda il
    /// risultato per SID. Se non ci si riesce — succede sull'agente di GitHub,
    /// dove si e' amministratori senza UAC — vale l'altra meta' del
    /// comportamento, che e' la stessa di
    /// `windows_una_cartella_degli_amministratori_non_si_prepara_senza_i_loro_poteri`.
    #[test]
    fn windows_cartella_dei_soli_diritti_del_proprietario() {
        let _s = seriale();
        let chi = assicura_profilo().unwrap();
        let so = sonda(&chi).unwrap();
        let d = nuova_cartella("proprietario");
        let s = d.display().to_string();
        let utente = std::env::var("USERNAME").unwrap();
        let assegnazione = icacls(&[&s, "/setowner", &utente]);
        let dell_utente = il_proprietario_e_l_utente(&d);
        solo_sistema_e_proprietario(&s, &utente);
        if !dell_utente {
            eprintln!("non riesco a dare la cartella all'utente ({}): si prova il rifiuto", assegnazione.trim());
            let errore = prepara_cartella(&d, &chi, Genere::Scrive, &so)
                .expect_err("una cartella che l'utente non possiede non si prepara per un comando senza poteri");
            assert!(errore.contains("non diventa accessibile"), "{errore}");
            if stato_elevazione() == Some(true) {
                assert!(errore.to_lowercase().contains("amministratore"), "il rifiuto non nomina l'amministratore: {errore}");
            }
            butta(&d, &chi);
            return;
        }
        prepara_cartella(&d, &chi, Genere::Scrive, &so).expect("si prepara con la sola voce di NOVA");
        let esito = ps(
            &format!("Set-Content -LiteralPath '{}' -Value ok", d.join("f.txt").display()),
            Some(&Permessi { scrive: vec![d.clone()], ..Default::default() }),
            None,
        );
        assert!(d.join("f.txt").is_file(), "dentro non si scrive\n{}", uscita(&esito));
        let voci = icacls(&[&s]).replace(&s, "").to_lowercase();
        assert!(!voci.contains(&format!("\\{}:", utente.to_lowercase())), "e' comparsa una voce dell'utente");
        butta(&d, &chi);
    }

    /// Una cartella degli Amministratori — la crea un processo elevato — non
    /// si prepara per un comando che non ha i loro poteri, e il rifiuto dice che
    /// il demone e' elevato e il comando no. Non dipende da `/setowner`: parte
    /// dal proprietario che la cartella ha davvero, e se l'ambiente la da' gia'
    /// all'utente (UAC acceso, processo normale) la prova non ha niente da dire.
    #[test]
    fn windows_una_cartella_degli_amministratori_non_si_prepara_senza_i_loro_poteri() {
        if !e_elevato() {
            eprintln!("saltata: da utente normale le cartelle sono dell'utente");
            return;
        }
        let _s = seriale();
        let chi = assicura_profilo().unwrap();
        let so = sonda(&chi).unwrap();
        let d = nuova_cartella("degli-amministratori");
        if il_proprietario_e_l_utente(&d) {
            eprintln!("saltata: in questo ambiente le cartelle di un processo elevato sono gia' dell'utente");
            butta(&d, &chi);
            return;
        }
        let s = d.display().to_string();
        solo_sistema_e_proprietario(&s, &std::env::var("USERNAME").unwrap());
        let errore = prepara_cartella(&d, &chi, Genere::Scrive, &so)
            .expect_err("un comando senza i poteri dell'amministratore non scrive in una cartella degli Amministratori");
        assert!(errore.contains("non diventa accessibile"), "{errore}");
        assert!(
            errore.to_lowercase().contains("amministratore"),
            "il rifiuto non dice che il demone e' elevato e il comando no: {errore}"
        );
        butta(&d, &chi);
    }

    /// Una cartella che nemmeno l'utente puo' modificare: NOVA non se la
    /// prende, ritira quel che ha scritto e rifiuta.
    #[test]
    fn windows_senza_modifica_dell_utente_il_comando_non_parte() {
        let _s = seriale();
        let chi = assicura_profilo().unwrap();
        let so = sonda(&chi).unwrap();
        let d = nuova_cartella("sola-lettura");
        let s = d.display().to_string();
        icacls(&[&s, "/inheritance:r"]);
        icacls(&[&s, "/remove:g", &std::env::var("USERNAME").unwrap()]);
        icacls(&[&s, "/grant:r", "*S-1-5-18:(OI)(CI)F"]);
        icacls(&[&s, "/grant:r", "*S-1-3-4:(OI)(CI)(R,WDAC)"]);
        let errore = prepara_cartella(&d, &chi, Genere::Scrive, &so).expect_err("doveva rifiutare");
        assert!(errore.contains("non parte"), "{errore}");
        assert!(!ha_permesso(&d, &chi, Genere::Scrive).unwrap(), "la voce di NOVA non e' stata ritirata");
        butta(&d, &chi);
    }

    /// PowerShell si posiziona in una cartella solo se il contenitore legge
    /// la **prima cartella sotto la radice del disco**. Senza, `Set-Location`
    /// dice «Accesso negato» e i percorsi relativi finiscono dove capita.
    #[test]
    fn windows_le_antenate_servono_per_la_cartella_di_lavoro() {
        let _s = seriale();
        let chi = assicura_profilo().unwrap();
        let so = sonda(&chi).unwrap();
        let radice = PathBuf::from(format!(
            "{}\\nova-prova-antenate-{}",
            std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()),
            std::process::id()
        ));
        if std::fs::create_dir_all(radice.join("a").join("b").join("dentro")).is_err() {
            eprintln!("saltata: non posso creare cartelle nella radice del disco");
            return;
        }
        let _pulizia = Pulizia(radice.clone());
        let dentro = radice.join("a").join("b").join("dentro");
        prepara_cartella(&dentro, &chi, Genere::Scrive, &so).unwrap();
        let script = format!(
            "Set-Location -LiteralPath '{}'; Set-Content -Path rel.txt -Value x",
            dentro.display()
        );
        let permessi = Permessi { scrive: vec![dentro.clone()], ..Default::default() };

        ps(&script, Some(&permessi), None);
        assert!(!dentro.join("rel.txt").exists(), "si e' posizionato senza le antenate");

        // La prima sotto la radice del disco, da sola, non basta: serve anche il
        // nonno (qui `a`). Il genitore (`b`) no.
        let da_preparare = per_posizionarsi_sotto(&dentro);
        assert_eq!(da_preparare.first(), Some(&radice), "{da_preparare:?}");
        assert_eq!(da_preparare.len(), 3, "{da_preparare:?}");
        prepara_cartella(&da_preparare[0], &chi, Genere::Antenata, &so).unwrap();
        ps(&script, Some(&permessi), None);
        assert!(!dentro.join("rel.txt").exists(), "la prima da sola e' bastata");
        for a in &da_preparare[1..] {
            prepara_cartella(a, &chi, Genere::Antenata, &so).unwrap();
        }
        let esito = ps(&script, Some(&permessi), None);
        assert!(dentro.join("rel.txt").is_file(), "con le antenate non si posiziona\n{}", uscita(&esito));
        let _ = std::fs::remove_dir_all(&radice);
    }

    /// Quali cartelle servono a PowerShell per posizionarsi sotto una radice.
    #[test]
    fn windows_per_posizionarsi_sotto() {
        let r = PathBuf::from(r"C:\Windows\System32\drivers");
        assert_eq!(
            per_posizionarsi_sotto(&r),
            vec![PathBuf::from(r"C:\Windows"), PathBuf::from(r"C:\Windows\System32")],
            "la prima, il genitore; il nonno e' la prima di nuovo"
        );
        assert!(per_posizionarsi_sotto(Path::new(r"C:\Windows")).is_empty(), "e' gia' la prima");
        assert!(per_posizionarsi_sotto(Path::new(r"C:\")).is_empty(), "e' la radice");
    }

    /// Quale cartella serve a PowerShell per posizionarsi.
    #[test]
    fn windows_la_prima_componente() {
        assert_eq!(
            prima_componente(Path::new(r"C:\Users\utente\NOVA")),
            Some(PathBuf::from(r"C:\Users"))
        );
        assert_eq!(prima_componente(Path::new(r"C:\Windows")), None, "e' gia' la prima");
        assert_eq!(prima_componente(Path::new(r"C:\")), None, "e' la radice");
    }

    /// Le voci di generi diversi sulla stessa cartella: togliere un genere
    /// lascia gli altri; la revoca li toglie tutti, anche quella senza
    /// ereditarieta'.
    #[test]
    fn windows_togli_genere_lascia_le_altre_voci() {
        let _s = seriale();
        let chi = assicura_profilo().unwrap();
        let d = nuova_cartella("generi");
        let s = d.display().to_string();
        let voci = |s: &str| icacls(&[s]).matches(&chi.stringa()).count();

        concedi(&d, &chi, Genere::Antenata).unwrap();
        concedi(&d, &chi, Genere::Scrive).unwrap();
        assert_eq!(voci(&s), 2, "{}", icacls(&[&s]));
        togli_genere(&d, &chi, Genere::Antenata).unwrap();
        assert_eq!(voci(&s), 1, "{}", icacls(&[&s]));
        assert!(ha_permesso(&d, &chi, Genere::Scrive).unwrap());

        concedi(&d, &chi, Genere::Legge).unwrap();
        revoca(&d, &chi).unwrap();
        assert_eq!(voci(&s), 0, "{}", icacls(&[&s]));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Fermare ferma anche i nipoti: un comando che avvia un altro processo e
    /// non finisce, fermato per scadenza, non lascia niente in piedi. E' il
    /// job object, e vale anche senza permessi.
    #[test]
    fn windows_fermare_ferma_anche_i_nipoti() {
        let _s = seriale();
        let comando = Comando {
            programma: "powershell".into(),
            argomenti: vec![
                "-NoProfile".into(),
                "-NonInteractive".into(),
                "-Command".into(),
                "$n = Start-Process powershell -ArgumentList '-NoProfile','-Command','Start-Sleep 60' -PassThru; Write-Output $n.Id; Start-Sleep 60".into(),
            ],
            cartella: None,
            ambiente: vec![],
        };
        let inizio = Instant::now();
        let esito = esegui(&comando, None, Duration::from_secs(6)).expect("parte");
        assert!(esito.scaduto, "doveva scadere: {esito:?}");
        assert!(esito.codice.is_none());
        assert!(inizio.elapsed() < Duration::from_secs(30), "la scadenza non ha fermato niente");
        let pid: u32 = String::from_utf8_lossy(&esito.stdout)
            .trim()
            .parse()
            .expect("il figlio ha stampato il pid del nipote");
        std::thread::sleep(Duration::from_millis(500));
        let vivo = unsafe {
            windows::Win32::System::Threading::OpenProcess(
                windows::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION,
                false,
                pid,
            )
        };
        if let Ok(h) = vivo {
            let mut codice = 0u32;
            let ancora = unsafe { GetExitCodeProcess(h, &mut codice) }.is_ok() && codice == 259; // STILL_ACTIVE
            unsafe {
                let _ = CloseHandle(h);
            }
            assert!(!ancora, "il nipote {pid} e' ancora vivo: il job non l'ha preso");
        }
    }

    /// Senza permessi non c'e' contenitore: e' il comando di prima, con in
    /// piu' solo il job object.
    #[test]
    fn windows_senza_permessi_non_si_stringe_niente() {
        let _s = seriale();
        let base = nuova_cartella("libero");
        let comando = Comando {
            programma: "powershell".into(),
            argomenti: vec![
                "-NoProfile".into(),
                "-Command".into(),
                format!("Set-Content -Path '{}' -Value 'x'; Write-Output \"$env:NOVA_PROVA\"", base.join("f.txt").display()),
            ],
            cartella: Some(base.clone()),
            ambiente: vec![("NOVA_PROVA".into(), "vale".into())],
        };
        let esito = esegui(&comando, None, Duration::from_secs(60)).expect("parte");
        assert_eq!(esito.codice, Some(0), "{}", uscita(&esito));
        assert!(base.join("f.txt").is_file());
        assert_eq!(String::from_utf8_lossy(&esito.stdout).trim(), "vale");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// La rete: spenta quando lo si chiede. Accesa, solo se l'host stesso ha
    /// internet: altrimenti la prova non puo' dire niente e lo dice.
    #[test]
    fn windows_la_rete_si_puo_spegnere() {
        let _s = seriale();
        assicura_profilo().unwrap();
        let prova = "try { $c = New-Object Net.Sockets.TcpClient; $a = $c.BeginConnect('1.1.1.1',443,$null,$null); if ($a.AsyncWaitHandle.WaitOne(4000)) { $c.EndConnect($a); 'RETE=SI' } else { 'RETE=NO' } } catch { 'RETE=NO' }";
        let spenta = ps(prova, Some(&Permessi { senza_rete: true, ..Default::default() }), None);
        assert!(String::from_utf8_lossy(&spenta.stdout).contains("RETE=NO"), "{}", uscita(&spenta));

        let addr: std::net::SocketAddr = "1.1.1.1:443".parse().unwrap();
        if std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(4)).is_err() {
            eprintln!("saltata la parte con la rete: l'host non ha internet");
            return;
        }
        let accesa = ps(prova, Some(&Permessi::default()), None);
        assert!(String::from_utf8_lossy(&accesa.stdout).contains("RETE=SI"), "{}", uscita(&accesa));
    }

    /// Una voce di antenata non ripassa i figli. Con `SetNamedSecurityInfoW`
    /// scrivere un permesso su `C:\Users\<nome>` ripercorreva l'intero
    /// profilo: la prova end-to-end e' rimasta due minuti e mezzo al 100% di
    /// CPU, e nessuna prova su una cartella piccola se n'era accorta. Qui la
    /// differenza si misura: la stessa cartella con 6.000 file, una voce che
    /// si eredita (e deve ripassarli) contro una che non si eredita.
    #[test]
    fn windows_l_antenata_non_ripassa_i_figli() {
        let _s = seriale();
        let chi = assicura_profilo().unwrap();
        let d = nuova_cartella("figli");
        let _pulizia = Pulizia(d.clone());
        for i in 0..60 {
            let sotto = d.join(format!("s{i}"));
            std::fs::create_dir_all(&sotto).unwrap();
            for k in 0..100 {
                std::fs::write(sotto.join(format!("f{k}.txt")), "x").unwrap();
            }
        }
        let inizio = Instant::now();
        concedi(&d, &chi, Genere::Antenata).unwrap();
        let antenata = inizio.elapsed();
        let inizio = Instant::now();
        revoca_locale(&d, &chi).unwrap();
        let revoca_locale_t = inizio.elapsed();
        assert!(!ha_permesso(&d, &chi, Genere::Antenata).unwrap(), "la revoca locale non ha tolto la voce");

        let inizio = Instant::now();
        concedi(&d, &chi, Genere::Scrive).unwrap();
        let scrive = inizio.elapsed();
        eprintln!("antenata {antenata:?}, revoca locale {revoca_locale_t:?}, scrive {scrive:?}");

        // I figli ripassati: solo la voce che si eredita arriva ai file.
        let un_file = d.join("s7").join("f7.txt");
        assert!(
            icacls(&[&un_file.display().to_string()]).contains(&chi.stringa()),
            "la voce ereditata non e' arrivata ai file"
        );
        assert!(
            antenata * SOGLIA < scrive && revoca_locale_t * SOGLIA < scrive,
            "la voce di antenata costa come quella ereditata: antenata {antenata:?}, \
             revoca locale {revoca_locale_t:?}, scrive {scrive:?}"
        );
        butta(&d, &chi);
    }

    /// Il passo privilegiato accetta una cosa sola: la prima cartella sotto la
    /// radice di un disco fisso. Tutto il resto — percorsi che portano
    /// altrove, o che ingannano il confronto — si rifiuta.
    #[test]
    fn windows_il_passo_privilegiato_accetta_una_cosa_sola() {
        e_una_prima_componente(Path::new(r"C:\Windows")).expect("la prima cartella e' valida");
        for no in [
            r"C:\Windows\System32",
            r"C:\",
            r"Windows",
            r"C:\Windows\..\Users",
            r"\\?\C:\Windows",
            r"\\server\condivisione\cartella",
            r"C:\nova-non-esiste-di-sicuro",
            r"C:\Windows\notepad.exe",
        ] {
            assert!(e_una_prima_componente(Path::new(no)).is_err(), "doveva rifiutare {no}");
        }
    }

    /// Una giunzione fa scrivere il permesso altrove da quello che il
    /// percorso dice: si rifiuta.
    #[test]
    fn windows_il_passo_privilegiato_rifiuta_le_giunzioni() {
        let _s = seriale();
        let giunzione = PathBuf::from(format!(
            "{}\\nova-prova-giunzione-{}",
            std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()),
            std::process::id()
        ));
        let bersaglio = nuova_cartella("giunzione-bersaglio");
        let fatta = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(&giunzione)
            .arg(&bersaglio)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !fatta {
            eprintln!("saltata: non posso creare giunzioni nella radice del disco");
            let _ = std::fs::remove_dir_all(&bersaglio);
            return;
        }
        let esito = e_una_prima_componente(&giunzione);
        let _ = std::fs::remove_dir(&giunzione);
        let _ = std::fs::remove_dir_all(&bersaglio);
        let errore = esito.expect_err("doveva rifiutare la giunzione");
        assert!(errore.contains("giunzione"), "{errore}");
    }

    /// Apertura e chiusura, sulla logica e non sull'elevazione: una cartella
    /// mia sulla radice del disco, che il mio utente puo' scrivere.
    #[test]
    fn windows_il_passo_privilegiato_apre_e_chiude() {
        let _s = seriale();
        let chi = assicura_profilo().unwrap();
        let d = PathBuf::from(format!(
            "{}\\nova-prova-privilegiato-{}",
            std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()),
            std::process::id()
        ));
        if std::fs::create_dir_all(&d).is_err() {
            eprintln!("saltata: non posso creare cartelle nella radice del disco");
            return;
        }
        let _pulizia = Pulizia(d.clone());
        apri_prima_componente(&d).unwrap();
        assert!(ha_permesso(&d, &chi, Genere::Antenata).unwrap());
        apri_prima_componente(&d).expect("aprire due volte non e' un errore");
        chiudi_prima_componente(&d).unwrap();
        assert!(!ha_permesso(&d, &chi, Genere::Antenata).unwrap());
        // E il passo non scrive altrove: su una cartella piu' in basso rifiuta.
        let sotto = d.join("sotto");
        std::fs::create_dir_all(&sotto).unwrap();
        assert!(apri_prima_componente(&sotto).is_err());
        assert!(!ha_permesso(&sotto, &chi, Genere::Antenata).unwrap());
        // Chiudere una cartella che non c'e' piu' va bene.
        chiudi_prima_componente(Path::new(r"C:\nova-non-esiste-di-sicuro")).unwrap();
    }

    /// Dire se si e' elevati non deve rompersi, in un senso o nell'altro.
    #[test]
    fn windows_si_sa_se_si_e_elevati() {
        let _ = e_elevato();
    }

    /// Le cartelle di strumenti stanno a qualunque profondita', ma non sono
    /// tutto: la radice, i percorsi sporchi, i file e le giunzioni si rifiutano.
    #[test]
    fn windows_il_passo_privilegiato_accetta_le_cartelle_di_strumenti() {
        e_una_cartella_di_strumenti(Path::new(r"C:\Windows")).expect("una cartella a un livello");
        e_una_cartella_di_strumenti(Path::new(r"C:\Windows\System32\drivers")).expect("una piu' in basso");
        for no in [
            r"C:\",
            r"Windows",
            r"C:\Windows\..\Users",
            r"\\?\C:\Windows",
            r"\\server\condivisione\cartella",
            r"C:\nova-non-esiste-di-sicuro",
            r"C:\Windows\notepad.exe",
        ] {
            assert!(e_una_cartella_di_strumenti(Path::new(no)).is_err(), "doveva rifiutare {no}");
        }
        // Un `.` in mezzo lo elimina `Path::components()`, e non cambia dove si
        // va: `C:\Windows\.\System32` e' `C:\Windows\System32`. Il `..` no.
        e_una_cartella_di_strumenti(Path::new(r"C:\Windows\.\System32")).expect("il punto e' innocuo");
    }

    /// Lettura ed esecuzione, mai scrittura: la cartella di strumenti si
    /// legge, e il contenitore non ci puo' scrivere. Sulla logica, non
    /// sull'elevazione: una cartella mia che l'utente puo' scrivere.
    #[test]
    fn windows_le_cartelle_di_strumenti_si_aprono_in_sola_lettura() {
        let _s = seriale();
        let chi = assicura_profilo().unwrap();
        let so = sonda(&chi).unwrap();
        let d = nuova_cartella("strumenti-privilegiati");
        let _pulizia = Pulizia(d.clone());
        std::fs::write(d.join("strumento.exe"), "x").unwrap();
        assert!(!puo(&d, &so, LEGGI_ESEGUI).unwrap(), "in partenza il contenitore non la legge");

        apri_cartella_di_strumenti(&d).unwrap();
        assert!(ha_permesso(&d, &chi, Genere::Legge).unwrap());
        assert!(!ha_permesso(&d, &chi, Genere::Scrive).unwrap(), "e' comparsa una voce di scrittura");
        let so = sonda(&chi).unwrap();
        assert!(puo(&d, &so, LEGGI_ESEGUI).unwrap(), "dopo, il contenitore la legge");
        assert!(!puo(&d, &so, MODIFICA).unwrap(), "e non la scrive");
        assert!(
            icacls(&[&d.join("strumento.exe").display().to_string()]).contains(&chi.stringa()),
            "i file che ci sono non hanno preso la voce"
        );

        apri_cartella_di_strumenti(&d).expect("aprire due volte non e' un errore");
        chiudi_cartella_di_strumenti(&d).unwrap();
        assert!(!ha_permesso(&d, &chi, Genere::Legge).unwrap());
        assert!(!icacls(&[&d.join("strumento.exe").display().to_string()]).contains(&chi.stringa()));
    }

    /// Il controllo trova le cartelle aperte ad ALL APPLICATION PACKAGES, a
    /// qualunque profondita', e solo quelle: non quelle aperte a Everyone (il
    /// contenitore non le scrive) ne' le normali.
    #[test]
    fn windows_il_controllo_trova_le_cartelle_aperte_ai_pacchetti() {
        let _s = seriale();
        let chi = assicura_profilo().unwrap();
        let so = sonda(&chi).unwrap();
        let base = nuova_cartella("controllo");
        let _pulizia = Pulizia(base.clone());
        let (pacchetti, annidata, everyone, normale) = (
            base.join("pacchetti"),
            base.join("fondo").join("piu").join("giu"),
            base.join("everyone"),
            base.join("normale"),
        );
        for d in [&pacchetti, &annidata, &everyone, &normale] {
            std::fs::create_dir_all(d).unwrap();
        }
        icacls(&[&pacchetti.display().to_string(), "/grant", "*S-1-15-2-1:(OI)(CI)F"]);
        icacls(&[&annidata.display().to_string(), "/grant", "*S-1-15-2-1:(OI)(CI)F"]);
        icacls(&[&everyone.display().to_string(), "/grant", "*S-1-1-0:(OI)(CI)F"]);

        let niente = AtomicBool::new(false);
        let s = cerca_cartelle_aperte(&base, &so, &niente, &|_| {});
        assert!(s.cartelle >= 7, "ha esaminato {} cartelle", s.cartelle);
        assert!(!s.interrotta);
        let mut trovate: Vec<String> = s.aperte.iter().map(|p| p.display().to_string().to_lowercase()).collect();
        trovate.sort();
        let mut attese: Vec<String> =
            [&pacchetti, &annidata].iter().map(|p| p.display().to_string().to_lowercase()).collect();
        attese.sort();
        assert_eq!(trovate, attese, "{:?}", s.aperte);
        // E non e' solo un sospetto: la scrittura vera lo conferma.
        ps(
            &format!(
                "Set-Content -LiteralPath '{}' -Value x -ErrorAction SilentlyContinue; \
                 Set-Content -LiteralPath '{}' -Value x -ErrorAction SilentlyContinue",
                pacchetti.join("c.txt").display(),
                everyone.join("c.txt").display()
            ),
            Some(&Permessi::default()),
            None,
        );
        assert!(pacchetti.join("c.txt").is_file(), "la cartella aperta ai pacchetti non e' scrivibile");
        assert!(!everyone.join("c.txt").exists(), "quella aperta a Everyone e' scrivibile");
    }

    /// **Prova-allarme.** Un divieto intestato al contenitore NON lo ferma:
    /// Windows non applica i divieti nella valutazione del contenitore, con
    /// nessuna maschera. Per questo le cartelle di terzi aperte a tutti i
    /// pacchetti si rilevano e si dichiarano invece di chiuderle. Se questa
    /// prova cade, Windows e' cambiato: la scansione puo' tornare a chiudere.
    #[test]
    fn windows_un_divieto_per_il_contenitore_non_ferma_la_scrittura() {
        let _s = seriale();
        let chi = assicura_profilo().unwrap();
        let base = nuova_cartella("divieto-inutile");
        let _pulizia = Pulizia(base.clone());
        let d = base.join("pacchetti");
        std::fs::create_dir_all(&d).unwrap();
        icacls(&[&d.display().to_string(), "/grant", "*S-1-15-2-1:(OI)(CI)F"]);
        icacls(&[&d.display().to_string(), "/deny", &format!("*{}:(OI)(CI)F", chi.stringa())]);
        ps(
            &format!("Set-Content -LiteralPath '{}' -Value x -ErrorAction SilentlyContinue", d.join("c.txt").display()),
            Some(&Permessi::default()),
            None,
        );
        assert!(
            d.join("c.txt").is_file(),
            "il divieto ha fermato il contenitore: Windows e' cambiato, vedi D367"
        );
    }

    /// I dischi fissi: quello di sistema c'e', ed e' NTFS.
    #[test]
    fn windows_i_dischi_fissi() {
        let (ntfs, _altri) = dischi_fissi();
        let sistema = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()).to_lowercase();
        assert!(
            ntfs.iter().any(|d| d.display().to_string().to_lowercase().starts_with(&sistema)),
            "{ntfs:?}"
        );
    }

    /// Un fermo a meta' non lascia una scansione che si spaccia per finita.
    #[test]
    fn windows_una_scansione_fermata_lo_dice() {
        let _s = seriale();
        let chi = assicura_profilo().unwrap();
        let so = sonda(&chi).unwrap();
        let fermo = AtomicBool::new(true);
        let s = cerca_cartelle_aperte(Path::new(r"C:\Windows"), &so, &fermo, &|_| {});
        assert!(s.interrotta);
        assert_eq!(s.cartelle, 0);
    }
}
