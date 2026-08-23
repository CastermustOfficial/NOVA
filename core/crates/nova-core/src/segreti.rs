//! L'archivio delle credenziali: cifrato, ordinato, e fuori dal contesto.
//!
//! NOVA è il PC, quindi le password le deve avere. La domanda non è *se*, ma
//! **dove vive il valore** — perché il contesto del modello è l'unico canale
//! da cui un segreto può davvero uscire: ciò che entra nel prompt viene
//! riletto in ogni conversazione futura, comprese quelle in cui NOVA sta
//! leggendo una pagina web scritta da qualcun altro.
//!
//! Quindi la separazione non è fra ciò che NOVA sa e ciò che non sa. È fra:
//!
//! - **l'inventario** — nomi, servizi, utenti, quando è cambiata, quanto è
//!   robusta, se è ripetuta altrove. Sta in chiaro, gira liberamente nel
//!   prompt, ed è ciò che permette a NOVA di dire «ce l'ho» e di tenere
//!   ordine;
//! - **il valore** — che si va a prendere quando serve, passa dall'archivio
//!   al campo di testo, e non attraversa mai il modello.
//!
//! Cifratura: DPAPI, legata all'account Windows dell'utente. Non c'è una
//! password madre da ricordare — che è il punto: NOVA deve poter fare le cose
//! anche quando l'utente non è alla tastiera. Il prezzo, detto chiaro: chi ha
//! la sessione Windows sbloccata ha anche l'archivio. Su un sistema in cui
//! NOVA può già fare qualunque cosa, quella non è una porta in più.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};

/// Una credenziale, con tutto ciò che serve a ritrovarla e a giudicarla.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Voce {
    /// Il nome con cui la si chiama: `gmail.giova`. È l'unico identificatore.
    pub nome: String,
    #[serde(default)]
    pub servizio: String,
    #[serde(default)]
    pub utente: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub categoria: String,
    #[serde(default)]
    pub note: String,
    /// Secondi dall'epoca. Un formato leggibile lo fa chi mostra.
    #[serde(default)]
    pub creato: u64,
    #[serde(default)]
    pub aggiornato: u64,
    /// Il segreto. Non esce mai da questo modulo se non su richiesta esplicita.
    #[serde(default)]
    pub valore: String,
}

/// La stessa voce senza il valore: questa può stare ovunque.
#[derive(Debug, Clone, Serialize)]
pub struct Scheda {
    pub nome: String,
    pub servizio: String,
    pub utente: String,
    pub url: String,
    pub categoria: String,
    pub note: String,
    pub creato: u64,
    pub aggiornato: u64,
    /// Quanti giorni fa è stata cambiata l'ultima volta.
    pub giorni_fa: u64,
    /// Da 0 a 4. Non è il valore, è un giudizio sul valore.
    pub robustezza: u8,
    pub lunghezza: usize,
    /// Quante altre voci usano lo stesso identico segreto.
    pub ripetuta_in: usize,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Archivio {
    #[serde(default = "versione_corrente")]
    versione: u32,
    #[serde(default)]
    voci: Vec<Voce>,
}

fn versione_corrente() -> u32 {
    1
}

fn adesso() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Dove sta l'archivio. Accanto alla configurazione, non nel vault e non nel
/// repository: il vault finisce nel prompt e il repository finisce su GitHub.
pub fn percorso() -> Result<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| anyhow!("APPDATA non definita"))?
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .ok_or_else(|| anyhow!("HOME non definita"))?
    };
    Ok(base.join("NOVA").join("segreti.dat"))
}

// ------------------------------------------------------------- cifratura

#[cfg(windows)]
mod cassa {
    use anyhow::{anyhow, Result};
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN,
    };

    /// Entropia aggiuntiva: senza, qualunque programma che gira come questo
    /// utente può decifrare l'archivio chiamando DPAPI a vuoto. Con, deve
    /// almeno sapere questa stringa — che sta nel binario, quindi non è un
    /// segreto: è un chiavistello, non una serratura.
    const SALE: &[u8] = b"nova.segreti.v1";

    fn blob(dati: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB {
            cbData: dati.len() as u32,
            pbData: dati.as_ptr() as *mut u8,
        }
    }

    unsafe fn raccogli(b: &CRYPT_INTEGER_BLOB) -> Vec<u8> {
        let v = std::slice::from_raw_parts(b.pbData, b.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(b.pbData as *mut _)));
        v
    }

    pub fn cifra(chiaro: &[u8]) -> Result<Vec<u8>> {
        unsafe {
            let mut dentro = blob(chiaro);
            let mut sale = blob(SALE);
            let mut fuori = CRYPT_INTEGER_BLOB::default();
            CryptProtectData(
                &mut dentro,
                None,
                Some(&mut sale),
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut fuori,
            )
            .map_err(|e| anyhow!("CryptProtectData fallita: {e}"))?;
            Ok(raccogli(&fuori))
        }
    }

    pub fn decifra(cifrato: &[u8]) -> Result<Vec<u8>> {
        unsafe {
            let mut dentro = blob(cifrato);
            let mut sale = blob(SALE);
            let mut fuori = CRYPT_INTEGER_BLOB::default();
            CryptUnprotectData(
                &mut dentro,
                None,
                Some(&mut sale),
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut fuori,
            )
            .map_err(|e| {
                anyhow!(
                    "CryptUnprotectData fallita: {e}. L'archivio e' legato all'account \
                     Windows che l'ha creato: copiato altrove, o su un altro utente, \
                     non si apre — ed e' cio' che deve fare."
                )
            })?;
            Ok(raccogli(&fuori))
        }
    }
}

#[cfg(not(windows))]
mod cassa {
    use anyhow::{bail, Result};
    pub fn cifra(_c: &[u8]) -> Result<Vec<u8>> {
        bail!("archivio delle credenziali non ancora implementato per {}: \
               va scritto sopra il portachiavi di sistema", std::env::consts::OS)
    }
    pub fn decifra(_c: &[u8]) -> Result<Vec<u8>> {
        bail!("archivio delle credenziali non ancora implementato per {}", std::env::consts::OS)
    }
}

// --------------------------------------------------------------- lettura

fn carica() -> Result<Archivio> {
    let p = percorso()?;
    if !p.exists() {
        return Ok(Archivio { versione: versione_corrente(), voci: Vec::new() });
    }
    let cifrato = std::fs::read(&p)?;
    if cifrato.is_empty() {
        return Ok(Archivio { versione: versione_corrente(), voci: Vec::new() });
    }
    let chiaro = cassa::decifra(&cifrato)?;
    Ok(serde_json::from_slice(&chiaro)?)
}

fn salva(a: &Archivio) -> Result<()> {
    let p = percorso()?;
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    let chiaro = serde_json::to_vec(a)?;
    let cifrato = cassa::cifra(&chiaro)?;
    // Scrittura atomica: un'interruzione a metà non deve lasciare un archivio
    // troncato, che sarebbe indistinguibile da un archivio perduto.
    let temporaneo = p.with_extension("dat.nuovo");
    std::fs::write(&temporaneo, &cifrato)?;
    std::fs::rename(&temporaneo, &p)?;
    Ok(())
}

// ------------------------------------------------------------- giudizio

/// Da 0 a 4. Non misura l'entropia con precisione: distingue «questa la
/// indovina chiunque» da «questa no», che è la distinzione che serve a
/// mettere in ordine.
pub fn robustezza(v: &str) -> u8 {
    let n = v.chars().count();
    if n == 0 {
        return 0;
    }
    let mut famiglie = 0u8;
    if v.chars().any(|c| c.is_ascii_lowercase()) { famiglie += 1; }
    if v.chars().any(|c| c.is_ascii_uppercase()) { famiglie += 1; }
    if v.chars().any(|c| c.is_ascii_digit()) { famiglie += 1; }
    if v.chars().any(|c| !c.is_alphanumeric()) { famiglie += 1; }
    let per_lunghezza = match n {
        0..=7 => 0,
        8..=11 => 1,
        12..=15 => 2,
        16..=19 => 3,
        _ => 4,
    };
    let per_varieta = famiglie.saturating_sub(1);
    per_lunghezza.min(per_varieta.max(if n >= 20 { 3 } else { 0 })).min(4)
}

fn scheda(v: &Voce, quante_volte: &HashMap<&str, usize>) -> Scheda {
    let ora = adesso();
    Scheda {
        nome: v.nome.clone(),
        servizio: v.servizio.clone(),
        utente: v.utente.clone(),
        url: v.url.clone(),
        categoria: v.categoria.clone(),
        note: v.note.clone(),
        creato: v.creato,
        aggiornato: v.aggiornato,
        giorni_fa: ora.saturating_sub(v.aggiornato) / 86_400,
        robustezza: robustezza(&v.valore),
        lunghezza: v.valore.chars().count(),
        ripetuta_in: quante_volte
            .get(v.valore.as_str())
            .copied()
            .unwrap_or(1)
            .saturating_sub(1),
    }
}

// -------------------------------------------------------------- pubblico

/// L'inventario, senza valori. Può stare nel prompt, può stare in un log.
pub fn elenco() -> Result<Vec<Scheda>> {
    let a = carica()?;
    let mut quante: HashMap<&str, usize> = HashMap::new();
    for v in &a.voci {
        if !v.valore.is_empty() {
            *quante.entry(v.valore.as_str()).or_insert(0) += 1;
        }
    }
    let mut fuori: Vec<Scheda> = a.voci.iter().map(|v| scheda(v, &quante)).collect();
    fuori.sort_by(|x, y| x.nome.cmp(&y.nome));
    Ok(fuori)
}

/// Il valore. L'unica funzione che lo restituisce.
pub fn leggi(nome: &str) -> Result<String> {
    let a = carica()?;
    a.voci
        .iter()
        .find(|v| v.nome.eq_ignore_ascii_case(nome))
        .map(|v| v.valore.clone())
        .ok_or_else(|| anyhow!("nessuna credenziale si chiama «{nome}»"))
}

pub fn esiste(nome: &str) -> bool {
    carica()
        .map(|a| a.voci.iter().any(|v| v.nome.eq_ignore_ascii_case(nome)))
        .unwrap_or(false)
}

/// Crea o aggiorna. Il valore vuoto lascia quello di prima: così si possono
/// correggere i metadati senza dover ridire la password.
pub fn salva_voce(mut nuova: Voce) -> Result<Scheda> {
    if nuova.nome.trim().is_empty() {
        bail!("una credenziale senza nome non si ritrova: serve un nome");
    }
    nuova.nome = nuova.nome.trim().to_lowercase();
    let mut a = carica()?;
    let ora = adesso();
    match a.voci.iter_mut().find(|v| v.nome == nuova.nome) {
        Some(vecchia) => {
            if !nuova.valore.is_empty() && nuova.valore != vecchia.valore {
                vecchia.valore = nuova.valore;
                vecchia.aggiornato = ora;
            }
            for (dove, nuovo) in [
                (&mut vecchia.servizio, nuova.servizio),
                (&mut vecchia.utente, nuova.utente),
                (&mut vecchia.url, nuova.url),
                (&mut vecchia.categoria, nuova.categoria),
                (&mut vecchia.note, nuova.note),
            ] {
                if !nuovo.is_empty() {
                    *dove = nuovo;
                }
            }
        }
        None => {
            nuova.creato = ora;
            nuova.aggiornato = ora;
            a.voci.push(nuova.clone());
        }
    }
    salva(&a)?;
    let quante = HashMap::new();
    let v = a
        .voci
        .iter()
        .find(|v| v.nome == nuova.nome)
        .cloned()
        .unwrap_or_default();
    Ok(scheda(&v, &quante))
}

pub fn dimentica(nome: &str) -> Result<bool> {
    let mut a = carica()?;
    let prima = a.voci.len();
    a.voci.retain(|v| !v.nome.eq_ignore_ascii_case(nome));
    let tolta = a.voci.len() != prima;
    if tolta {
        salva(&a)?;
    }
    Ok(tolta)
}

/// Una password nuova, generata qui. Non passa dal modello: lui riceve solo
/// il nome con cui richiamarla.
pub fn genera(lunghezza: usize) -> Result<String> {
    const ALFABETO: &[u8] =
        b"abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789!@#$%&*-_=+?";
    let n = lunghezza.clamp(12, 128);
    let mut byte = vec![0u8; n * 2];
    riempi_a_caso(&mut byte)?;
    Ok(byte
        .chunks(2)
        .take(n)
        .map(|c| {
            let i = ((c[0] as usize) << 8 | c[1] as usize) % ALFABETO.len();
            ALFABETO[i] as char
        })
        .collect())
}

#[cfg(windows)]
fn riempi_a_caso(buf: &mut [u8]) -> Result<()> {
    use windows::Win32::Security::Cryptography::{
        BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG,
    };
    unsafe {
        let s = BCryptGenRandom(None, buf, BCRYPT_USE_SYSTEM_PREFERRED_RNG);
        if s.is_err() {
            bail!("BCryptGenRandom fallita: {s:?}");
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn riempi_a_caso(buf: &mut [u8]) -> Result<()> {
    use std::io::Read;
    std::fs::File::open("/dev/urandom")?.read_exact(buf)?;
    Ok(())
}
