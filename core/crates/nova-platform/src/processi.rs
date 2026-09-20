//! I processi: elencarli, e chiuderne **uno**.
//!
//! **Perche' «uno» e' scritto in grassetto.** Dall'altra parte
//! `close_application` compone un modello di ricerca di PowerShell:
//!
//! ```text
//! Get-Process | Where-Object {$_.ProcessName -like '*{name}*' ...}
//! ```
//!
//! Il nome dell'utente finisce dentro un `-like`, cioe' dentro un linguaggio
//! di modelli. Misurato su questa macchina il 5 settembre: `name` uguale a
//! `*`, a `?` oppure a `[a-z]` seleziona **292 processi** — tutti quelli che
//! ci sono. Con `force` acceso vuol dire `Stop-Process -Force` su tutto il
//! sistema, servizi compresi, da un argomento di un carattere.
//!
//! Lo strumento e' marcato pericoloso, quindi una persona approva. Ma
//! l'anteprima che quella persona legge dice «Termina FORZATAMENTE '*'», e
//! non c'e' niente, in quella riga, che le faccia capire che sta per
//! spegnere duecentonovantadue processi.
//!
//! Il rimedio non e' scrivere un modello piu' prudente: e' **togliere il
//! modello**. Qui la selezione e l'azione sono due cose separate — si elenca,
//! si guarda cosa combacia, e si chiude un pid alla volta. Un pid non ha
//! caratteri jolly.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Processo {
    pub pid: u32,
    pub nome: String,
    /// Memoria di lavoro in byte. Zero quando il sistema non la lascia
    /// leggere — succede per i processi di sistema, e non e' un errore.
    pub memoria_byte: u64,
}

#[cfg(windows)]
mod imp {
    use super::Processo;
    use anyhow::{anyhow, Result};
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows::Win32::System::Threading::{
        OpenProcess, TerminateProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
        PROCESS_VM_READ,
    };

    fn memoria(pid: u32) -> u64 {
        unsafe {
            let Ok(h) = OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ,
                false,
                pid,
            ) else {
                return 0;
            };
            let mut c = PROCESS_MEMORY_COUNTERS::default();
            let ok = GetProcessMemoryInfo(
                h,
                &mut c,
                std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            );
            let _ = CloseHandle(h);
            if ok.is_ok() {
                c.WorkingSetSize as u64
            } else {
                0
            }
        }
    }

    pub fn elenca() -> Result<Vec<Processo>> {
        unsafe {
            let istantanea = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
                .map_err(|e| anyhow!("non riesco a fotografare i processi: {e}"))?;
            let mut voce = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };
            let mut fuori = Vec::new();
            if Process32FirstW(istantanea, &mut voce).is_ok() {
                loop {
                    let fine = voce
                        .szExeFile
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(voce.szExeFile.len());
                    let nome = String::from_utf16_lossy(&voce.szExeFile[..fine]);
                    // Il pid 0 e' il processo inattivo del sistema: esiste
                    // nella fotografia e non e' un programma.
                    if voce.th32ProcessID != 0 {
                        fuori.push(Processo {
                            pid: voce.th32ProcessID,
                            nome,
                            memoria_byte: memoria(voce.th32ProcessID),
                        });
                    }
                    if Process32NextW(istantanea, &mut voce).is_err() {
                        break;
                    }
                }
            }
            let _ = CloseHandle(istantanea);
            Ok(fuori)
        }
    }

    /// Chiude **un** processo, per pid.
    ///
    /// Non prende un nome e non prende un modello: prende il numero di un
    /// processo. Chi ha scelto quel numero l'ha scelto guardando un elenco.
    ///
    /// `forza` e' la differenza fra chiedere e imporre: senza, si mandano le
    /// finestre a chiudersi e il programma puo' chiedere se salvare; con,
    /// `TerminateProcess` lo ferma dov'e' ed eventuali lavori non salvati
    /// sono persi. E' anche il motivo per cui i due casi non si confondono
    /// mai in una sola chiamata.
    pub fn chiudi(pid: u32, forza: bool) -> Result<()> {
        if pid == 0 {
            return Err(anyhow!("il pid 0 non e' un programma"));
        }
        if !forza {
            let quante = super::chiedi_di_chiudere(pid)?;
            if quante == 0 {
                return Err(anyhow!(
                    "il processo {pid} non ha finestre a cui chiedere di chiudersi: \
                     per fermarlo serve «force»"
                ));
            }
            return Ok(());
        }
        unsafe {
            let h = OpenProcess(PROCESS_TERMINATE, false, pid)
                .map_err(|e| anyhow!("non posso fermare il processo {pid}: {e}"))?;
            let esito = TerminateProcess(h, 1);
            let _ = CloseHandle(h);
            esito.map_err(|e| anyhow!("non sono riuscito a fermare il processo {pid}: {e}"))
        }
    }
}

#[cfg(windows)]
/// Manda `WM_CLOSE` a tutte le finestre di questo processo, e dice a quante.
///
/// E' quello che fa `CloseMainWindow`, ma su **tutte** le finestre e non solo
/// sulla principale: un programma con tre finestre aperte, chiuso con la sola
/// principale, resta li' con le altre due.
fn chiedi_di_chiudere(pid: u32) -> anyhow::Result<usize> {
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};
    let finestre = crate::finestre::elenca()?;
    let mut quante = 0;
    for f in finestre.iter().filter(|f| f.pid == pid) {
        let h = HWND(f.handle as *mut std::ffi::c_void);
        // `PostMessage` e non `SendMessage`: il secondo aspetta che il
        // programma risponda, e un programma che mostra «vuoi salvare?»
        // non risponde finche' qualcuno non clicca. Chi chiude non deve
        // restare appeso a una finestra di dialogo.
        if unsafe { PostMessageW(Some(h), WM_CLOSE, WPARAM(0), LPARAM(0)) }.is_ok() {
            quante += 1;
        }
    }
    Ok(quante)
}

#[cfg(all(not(windows), unix))]
mod imp {
    pub use crate::processi_unix::{chiudi, elenca};
}

#[cfg(all(not(windows), not(unix)))]
mod imp {
    use super::Processo;
    use anyhow::{bail, Result};

    pub fn elenca() -> Result<Vec<Processo>> {
        bail!("l'elenco dei processi qui si chiede in un altro modo")
    }
    pub fn chiudi(_pid: u32, _forza: bool) -> Result<()> {
        bail!("chiudere un processo qui si fa in un altro modo")
    }
}

pub use imp::{chiudi, elenca};

/// Quali processi corrispondono a questo testo, **come sottostringa**.
///
/// Non e' un modello: `*` cerca un processo che si chiami davvero con un
/// asterisco dentro, e non ne trova nessuno. E' il punto di tutto il modulo.
pub fn corrispondenti<'a>(processi: &'a [Processo], testo: &str) -> Vec<&'a Processo> {
    let t = testo.to_lowercase();
    processi
        .iter()
        .filter(|p| p.nome.to_lowercase().contains(&t))
        .collect()
}

#[cfg(test)]
mod prove {
    use super::{corrispondenti, Processo};

    fn finti() -> Vec<Processo> {
        [
            "chrome.exe",
            "notepad.exe",
            "explorer.exe",
            "Chrome Helper.exe",
        ]
        .iter()
        .enumerate()
        .map(|(i, n)| Processo {
            pid: i as u32 + 10,
            nome: n.to_string(),
            memoria_byte: 0,
        })
        .collect()
    }

    #[test]
    fn si_cerca_una_sottostringa_senza_guardare_le_maiuscole() {
        let p = finti();
        let trovati = corrispondenti(&p, "chrome");
        assert_eq!(trovati.len(), 2, "chrome.exe e Chrome Helper.exe");
    }

    #[test]
    fn l_asterisco_si_cerca_alla_lettera() {
        // La prova che vale piu' di tutte le altre di questo modulo. Con la
        // strada di prima, `*` selezionava 292 processi su questa macchina —
        // cioe' tutti — e con «force» sarebbe stata la macchina giu' da un
        // argomento di un carattere.
        //
        // «Alla lettera» e non «non trova niente»: la differenza si e' vista
        // subito, perche' una finestra aperta si chiamava «*napoli difesa» —
        // l'asterisco del file non salvato — e quella l'asterisco ce l'ha per
        // davvero. Uno invece di 292 e' la risposta giusta; zero sarebbe
        // stata un'altra bugia.
        let p = finti();
        assert!(
            corrispondenti(&p, "*").is_empty(),
            "qui nessuno ha l'asterisco nel nome"
        );
        assert!(corrispondenti(&p, "?").is_empty());
        assert!(corrispondenti(&p, "[a-z]").is_empty());

        let con_asterisco = vec![Processo {
            pid: 1,
            nome: "*strano.exe".into(),
            memoria_byte: 0,
        }];
        assert_eq!(
            corrispondenti(&con_asterisco, "*").len(),
            1,
            "chi ha l'asterisco nel nome si trova cercando un asterisco"
        );
    }

    #[test]
    fn un_testo_vuoto_prende_tutto_e_deve_essere_chi_chiama_a_impedirlo() {
        // `contains("")` e' vero per ogni stringa: e' l'aritmetica delle
        // sottostringhe, non un difetto. Sta scritto qui perche' chi usa
        // questa funzione per **chiudere** deve rifiutare il testo vuoto
        // prima di arrivarci, e chi la usa per **elencare** no.
        let p = finti();
        assert_eq!(corrispondenti(&p, "").len(), 4);
    }
}

/// Avviare un programma come lo avvia il menu Start.
///
/// `ShellExecuteExW` e non `CreateProcess`: e' quello che sa risolvere le
/// «App Paths» del registro (`chrome` senza percorso), le associazioni di
/// file, e gli URI di sistema come `ms-settings:`. Dall'altra parte lo faceva
/// `Start-Process`, che e' un rivestimento su questa stessa chiamata — con
/// dentro il solito guaio delle virgolette: `-FilePath '{target}'` si rompe
/// su un percorso che contiene un apostrofo, e ce ne sono (D130).
#[cfg(windows)]
pub fn avvia(bersaglio: &str, argomenti: &str) -> anyhow::Result<()> {
    use anyhow::anyhow;
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW};
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    fn larga(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }
    let verbo = larga("open");
    let file = larga(bersaglio);
    let args = larga(argomenti);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        // Senza questo, la chiamata puo' tornare prima che il sistema abbia
        // davvero avviato qualcosa, e un errore arriverebbe quando non c'e'
        // piu' nessuno ad ascoltarlo.
        fMask: SEE_MASK_NOASYNC,
        lpVerb: PCWSTR(verbo.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: if argomenti.is_empty() {
            PCWSTR::null()
        } else {
            PCWSTR(args.as_ptr())
        },
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    unsafe { ShellExecuteExW(&mut info) }
        .map_err(|e| anyhow!("non riesco ad avviare «{bersaglio}»: {e}"))
}

#[cfg(all(not(windows), unix))]
pub use crate::processi_unix::avvia;

#[cfg(all(not(windows), not(unix)))]
pub fn avvia(_bersaglio: &str, _argomenti: &str) -> anyhow::Result<()> {
    anyhow::bail!("avviare un programma qui si fa in un altro modo")
}
