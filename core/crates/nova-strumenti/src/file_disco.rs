//! Gli strumenti sui file, quelli che il disco lo toccano davvero.
//!
//! Ogni corpo prende le **guardie** come primo argomento, e non e' una
//! comodita': un permesso che si puo' dimenticare si dimentica, e qui non si
//! puo' scrivere uno strumento che scrive senza aver chiesto. E' la stessa
//! ragione per cui il guardiano dei segreti e' dentro la porta del vault e
//! non nel giudizio di chi chiama (D106, D110).
//!
//! Cio' che il disco non sa fare da solo — mandare nel Cestino, aprire un
//! file con l'applicazione giusta — sta dietro un tratto: sono le uniche due
//! cose di questo modulo che cambiano da sistema a sistema.

use std::path::{Path, PathBuf};

use crate::file::{
    chiave_ordine, fetta, intestazione, modello_di_ricerca, riga_cartella, riga_file,
    riga_illeggibile, riga_trovata, taglia, MAX_ELEMENTI,
};
use crate::guardie::{Divieto, Guardie};

/// Quanto puo' pesare un file perche' valga la pena cercarci dentro.
pub const MAX_BYTE_DA_SETACCIARE: u64 = 5_000_000;

/// L'errore che torna al modello. E' una stringa perche' e' una stringa che
/// il modello legge: non c'e' niente da distinguere a valle.
pub type Esito = Result<String, String>;

/// Le due cose che il disco non sa fare da solo.
pub trait Sistema {
    /// Manda nel Cestino. Il Cestino e' l'annullamento che il sistema regala
    /// gia': un file che ci finisce si recupera con due clic, uno cancellato
    /// davvero no.
    fn nel_cestino(&self, percorso: &Path) -> bool;
    /// Apre con l'applicazione predefinita.
    fn apri(&self, percorso: &Path) -> Result<(), String>;
}

/// Un sistema che non sa fare niente delle due. Non e' un ripiego silenzioso:
/// chi lo usa se lo sente dire.
pub struct SenzaSistema;

impl Sistema for SenzaSistema {
    fn nel_cestino(&self, _percorso: &Path) -> bool {
        false
    }
    fn apri(&self, _percorso: &Path) -> Result<(), String> {
        Err("aprire un file con l'applicazione predefinita non e' \
             implementato su questo sistema".into())
    }
}

/// Il percorso come lo intende l'utente, e come lo vede il disco.
///
/// Torna tutti e due: quello **lessicale** — le variabili sciolte, i `.` e i
/// `..` risolti a nome — e quello vero dopo aver seguito i collegamenti. Il
/// primo e' quello che si scrive nei messaggi, il secondo serve alle guardie
/// (D118), e chi ha solo il primo non puo' sapere che una giunzione lo porta
/// altrove.
pub struct Percorso {
    pub scritto: PathBuf,
    pub risolto: Option<PathBuf>,
}

impl Percorso {
    pub fn nuovo(dato: &str) -> Result<Percorso, String> {
        if dato.trim().is_empty() {
            return Err("percorso vuoto".into());
        }
        let sciolto = sciogli(dato);
        let assoluto = if Path::new(&sciolto).is_absolute() {
            PathBuf::from(&sciolto)
        } else {
            std::env::current_dir()
                .map(|c| c.join(&sciolto))
                .unwrap_or_else(|_| PathBuf::from(&sciolto))
        };
        let risolto = std::fs::canonicalize(&assoluto).ok().map(pulisci_prefisso);
        // Il percorso che si mostra e' quello risolto quando c'e', come fa il
        // Python: e' quello che l'utente ritrova se lo incolla altrove.
        let scritto = risolto.clone().unwrap_or(assoluto);
        Ok(Percorso { scritto, risolto })
    }

    fn testo(&self) -> String {
        self.scritto.display().to_string()
    }

    fn controlla(&self, g: &Guardie) -> Result<(), String> {
        let risolto = self.risolto.as_ref().map(|p| p.display().to_string());
        g.puo_scrivere(&self.testo(), risolto.as_deref())
            .map_err(|e: Divieto| e.messaggio())
    }
}

/// Scioglie `~` e le variabili d'ambiente, come fa il Python.
fn sciogli(dato: &str) -> String {
    let mut s = dato.to_string();
    if let Some(resto) = s.strip_prefix('~') {
        if let Some(casa) = casa() {
            s = format!("{}{}", casa.display(), resto);
        }
    }
    // `%NOME%` su Windows, `$NOME` altrove: si sciolgono tutte e due, perche'
    // il modello scrive quello che ha visto scritto da qualche parte.
    let mut fuori = String::with_capacity(s.len());
    let b: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < b.len() {
        if b[i] == '%' {
            if let Some(fine) = (i + 1..b.len()).find(|j| b[*j] == '%') {
                let nome: String = b[i + 1..fine].iter().collect();
                if let Ok(v) = std::env::var(&nome) {
                    fuori.push_str(&v);
                    i = fine + 1;
                    continue;
                }
            }
        }
        if b[i] == '$' {
            let fine = (i + 1..b.len())
                .find(|j| !(b[*j].is_alphanumeric() || b[*j] == '_'))
                .unwrap_or(b.len());
            let nome: String = b[i + 1..fine].iter().collect();
            if !nome.is_empty() {
                if let Ok(v) = std::env::var(&nome) {
                    fuori.push_str(&v);
                    i = fine;
                    continue;
                }
            }
        }
        fuori.push(b[i]);
        i += 1;
    }
    fuori
}

/// La cartella dell'utente: `USERPROFILE` su Windows, `HOME` altrove.
pub fn casa() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

/// Toglie il prefisso `\\?\` che `canonicalize` mette su Windows.
///
/// E' vero, e' corretto, ed e' illeggibile: un messaggio che dice
/// `\\?\C:\Users\utente\nota.txt` fa sembrare rotto qualcosa che funziona.
fn pulisci_prefisso(p: PathBuf) -> PathBuf {
    let s = p.display().to_string();
    match s.strip_prefix(r"\\?\") {
        Some(pulito) => PathBuf::from(pulito),
        None => p,
    }
}

fn quando(m: &std::fs::Metadata, fuso: &dyn crate::data::Fuso) -> String {
    let secondi = m
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    crate::data::locale_con(secondi, fuso)
}

// ---------------------------------------------------------------- elencare
/// Elenca file e sottocartelle: la cosa che si fa per orientarsi.
pub fn elenca(cartella: &str, modello: &str, anche_nascosti: bool,
              fuso: &dyn crate::data::Fuso) -> Esito {
    let d = Percorso::nuovo(cartella)?;
    let dove = &d.scritto;
    if !dove.exists() {
        return Err(format!("la cartella {} non esiste", d.testo()));
    }
    if !dove.is_dir() {
        return Err(format!("{} non e' una cartella", d.testo()));
    }
    let modello = if modello.is_empty() { "*" } else { modello };
    let quale = glob::Pattern::new(modello)
        .map_err(|e| format!("modello non valido '{modello}': {e}"))?;
    let mut voci: Vec<(bool, String, String)> = Vec::new();
    let Ok(lettura) = std::fs::read_dir(dove) else {
        return Err(format!("non riesco a leggere {}", d.testo()));
    };
    for voce in lettura.flatten() {
        let nome = voce.file_name().to_string_lossy().to_string();
        if !quale.matches(&nome) {
            continue;
        }
        if !anche_nascosti && nome.starts_with('.') {
            continue;
        }
        let e_cartella = voce.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let k = chiave_ordine(&nome, e_cartella);
        voci.push((k.0, k.1, nome));
    }
    if voci.is_empty() {
        return Ok(format!(
            "{}: nessun elemento corrispondente a '{modello}'.",
            d.testo()
        ));
    }
    voci.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
    let quanti = voci.len();
    let mut righe = vec![format!("{}  ({quanti} elementi)", d.testo())];
    for (_, _, nome) in voci.iter().take(MAX_ELEMENTI) {
        righe.push(descrivi(&dove.join(nome), nome, fuso));
    }
    if quanti > MAX_ELEMENTI {
        righe.push(format!("... e altri {} elementi", quanti - MAX_ELEMENTI));
    }
    Ok(righe.join("\n"))
}

fn descrivi(percorso: &Path, nome: &str, fuso: &dyn crate::data::Fuso) -> String {
    match std::fs::metadata(percorso) {
        Err(e) => riga_illeggibile(nome, &e.to_string()),
        Ok(m) if m.is_dir() => riga_cartella(nome, &quando(&m, fuso)),
        Ok(m) => riga_file(nome, m.len(), &quando(&m, fuso)),
    }
}

// ---------------------------------------------------------------- leggere
/// Legge un file di testo, o dice perche' non si puo'.
pub fn leggi(percorso: &str, offset: i64, limite: i64) -> Esito {
    let f = Percorso::nuovo(percorso)?;
    if !f.scritto.exists() {
        return Err(format!("il file {} non esiste", f.testo()));
    }
    if f.scritto.is_dir() {
        return Err(format!("{} e' una cartella, usa list_directory", f.testo()));
    }
    let Ok(grezzo) = std::fs::read(&f.scritto) else {
        return Err(format!("non riesco a leggere {}", f.testo()));
    };
    let testo = match String::from_utf8(grezzo.clone()) {
        Ok(t) => t,
        // Un file che non e' UTF-8 non e' per forza binario: sui PC italiani
        // e' spesso cp1252, e rifiutarlo vorrebbe dire non saper leggere
        // meta' dei file di testo che esistono su questa macchina.
        Err(_) => match da_cp1252(&grezzo) {
            Some(t) => t,
            None => {
                return Ok(format!(
                    "{} non e' un file di testo ({} byte).",
                    f.testo(),
                    grezzo.len()
                ))
            }
        },
    };
    let righe: Vec<&str> = testo.lines().collect();
    let (inizio, fine) = fetta(righe.len(), offset, limite);
    let pezzo = righe
        .get(inizio.min(righe.len())..fine.min(righe.len()))
        .unwrap_or(&[])
        .join("\n");
    Ok(format!(
        "{}\n{}",
        intestazione(&f.testo(), inizio, fine, righe.len()),
        taglia(&pezzo, righe.len())
    ))
}

/// I byte letti come cp1252, se ci stanno tutti.
fn da_cp1252(b: &[u8]) -> Option<String> {
    let mut fuori = String::with_capacity(b.len());
    for x in b {
        // I byte 0x81, 0x8D, 0x8F, 0x90, 0x9D non esistono in cp1252: un file
        // che li contiene non e' cp1252, e leggerlo comunque vorrebbe dire
        // inventare dei caratteri.
        if matches!(x, 0x81 | 0x8d | 0x8f | 0x90 | 0x9d) {
            return None;
        }
        fuori.push(CP1252[*x as usize]);
    }
    Some(fuori)
}

/// I 32 caratteri in cui cp1252 si scosta da latin-1. Il resto coincide col
/// punto di codice Unicode.
const CP1252: [char; 256] = {
    let mut t = ['\0'; 256];
    let mut i = 0;
    while i < 256 {
        t[i] = i as u8 as char;
        i += 1;
    }
    t[0x80] = '\u{20ac}'; t[0x82] = '\u{201a}'; t[0x83] = '\u{0192}';
    t[0x84] = '\u{201e}'; t[0x85] = '\u{2026}'; t[0x86] = '\u{2020}';
    t[0x87] = '\u{2021}'; t[0x88] = '\u{02c6}'; t[0x89] = '\u{2030}';
    t[0x8a] = '\u{0160}'; t[0x8b] = '\u{2039}'; t[0x8c] = '\u{0152}';
    t[0x8e] = '\u{017d}'; t[0x91] = '\u{2018}'; t[0x92] = '\u{2019}';
    t[0x93] = '\u{201c}'; t[0x94] = '\u{201d}'; t[0x95] = '\u{2022}';
    t[0x96] = '\u{2013}'; t[0x97] = '\u{2014}'; t[0x98] = '\u{02dc}';
    t[0x99] = '\u{2122}'; t[0x9a] = '\u{0161}'; t[0x9b] = '\u{203a}';
    t[0x9c] = '\u{0153}'; t[0x9e] = '\u{017e}'; t[0x9f] = '\u{0178}';
    t
};

// --------------------------------------------------------------- scrivere
/// Scrive un file, **senza poterlo lasciare a meta'**.
pub fn scrivi(g: &Guardie, percorso: &str, contenuto: &str, in_coda: bool) -> Esito {
    let f = Percorso::nuovo(percorso)?;
    f.controlla(g)?;
    let esisteva = f.scritto.exists();
    if let Some(cartella) = f.scritto.parent() {
        std::fs::create_dir_all(cartella).map_err(|e| e.to_string())?;
    }
    if in_coda {
        // Accodare non tronca niente: qui il file di prima non e' in pericolo.
        use std::io::Write;
        let mut fh = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&f.scritto)
            .map_err(|e| e.to_string())?;
        fh.write_all(contenuto.as_bytes()).map_err(|e| e.to_string())?;
    } else {
        di_fianco(&f.scritto, contenuto)?;
    }
    let quanti = std::fs::metadata(&f.scritto).map(|m| m.len()).unwrap_or(0);
    let verbo = if esisteva { "aggiornato" } else { "creato" };
    Ok(format!("File {verbo}: {} ({quanti} byte)", f.testo()))
}

/// Sostituisce un pezzo esatto dentro un file.
pub fn modifica(g: &Guardie, percorso: &str, vecchio: &str, nuovo: &str, tutte: bool) -> Esito {
    let f = Percorso::nuovo(percorso)?;
    f.controlla(g)?;
    if !f.scritto.exists() {
        return Err(format!("il file {} non esiste", f.testo()));
    }
    let testo = std::fs::read_to_string(&f.scritto).map_err(|e| e.to_string())?;
    let quante = testo.matches(vecchio).count();
    if quante == 0 {
        return Err("testo da sostituire non trovato; rileggi il file".into());
    }
    if quante > 1 && !tutte {
        return Err(format!(
            "'old_text' compare {quante} volte: rendilo univoco o usa replace_all"
        ));
    }
    let fatto = if tutte {
        testo.replace(vecchio, nuovo)
    } else {
        testo.replacen(vecchio, nuovo, 1)
    };
    di_fianco(&f.scritto, &fatto)?;
    let quante = if tutte { quante } else { 1 };
    Ok(format!("Modificato {} ({quante} sostituzioni)", f.testo()))
}

/// Di fianco e poi si rinomina: se ci si mette qualcosa di mezzo, il file di
/// prima e' ancora quello di prima. La lezione costata un file di prova il 3
/// settembre, e che riguardava le note di chi usa NOVA (D102).
fn di_fianco(dove: &Path, testo: &str) -> Result<(), String> {
    let nome = format!(
        "{}.parte-{}",
        dove.file_name().and_then(|n| n.to_str()).unwrap_or("file"),
        std::process::id()
    );
    let tmp = dove.with_file_name(nome);
    let esito = (|| -> std::io::Result<()> {
        use std::io::Write;
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(testo.as_bytes())?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, dove)
    })();
    if let Err(e) = esito {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.to_string());
    }
    Ok(())
}

/// Crea una cartella, e non si lamenta se c'era gia'.
pub fn crea_cartella(g: &Guardie, percorso: &str) -> Esito {
    let d = Percorso::nuovo(percorso)?;
    d.controlla(g)?;
    std::fs::create_dir_all(&d.scritto).map_err(|e| e.to_string())?;
    // «creata» anche quando c'era gia': e' quello che dice il Python, e
    // la differenza fra «creata» e «pronta» il modello la legge.
    Ok(format!("Cartella creata: {}", d.testo()))
}

// ------------------------------------------------------- spostare, copiare
/// Sposta o rinomina.
///
/// Il sistema serve per una ragione sola, e non e' un dettaglio: **chi
/// chiede di spostare non ha chiesto di distruggere cio' che c'era**. Con
/// `sovrascrivi` la destinazione che esiste gia' va nel Cestino prima che
/// sopra ci arrivi l'altra, cosi' se era la cosa sbagliata si recupera.
///
/// Questa funzione qui non ce l'aveva, e faceva `rename` sopra: il file di
/// destinazione spariva per sempre, senza che niente lo dicesse. Dall'altra
/// parte, in Python, la regola c'era da sempre. Il banco non se n'e'
/// accorto perche' provava solo il caso **senza** `sovrascrivi`, dove le
/// due meta' si comportano uguale (vedi `docs/dove_ho_sbagliato.md`).
pub fn sposta(g: &Guardie, sistema: &dyn Sistema, da: &str, a: &str, sovrascrivi: bool) -> Esito {
    let s = Percorso::nuovo(da)?;
    let d = Percorso::nuovo(a)?;
    // Tutte e due: si perde un file tanto dalla parte da cui parte quanto da
    // quella dove arriva.
    s.controlla(g)?;
    d.controlla(g)?;
    if !s.scritto.exists() {
        return Err(format!("{} non esiste", s.testo()));
    }
    if d.scritto.exists() {
        if !sovrascrivi {
            return Err(format!(
                "{} esiste gia'; usa overwrite=true per sovrascrivere",
                d.testo()
            ));
        }
        if !sistema.nel_cestino(&d.scritto) {
            return Err(format!(
                "{} esiste e non riesco a metterlo nel Cestino: mi fermo invece \
                 di cancellarlo per sempre. Spostalo o eliminalo tu, poi riprova.",
                d.testo()
            ));
        }
    }
    if let Some(cartella) = d.scritto.parent() {
        std::fs::create_dir_all(cartella).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&s.scritto, &d.scritto).map_err(|e| e.to_string())?;
    Ok(format!("Spostato: {} -> {}", s.testo(), d.testo()))
}

pub fn copia(g: &Guardie, da: &str, a: &str) -> Esito {
    let s = Percorso::nuovo(da)?;
    let d = Percorso::nuovo(a)?;
    d.controlla(g)?;
    if !s.scritto.exists() {
        return Err(format!("{} non esiste", s.testo()));
    }
    if let Some(cartella) = d.scritto.parent() {
        std::fs::create_dir_all(cartella).map_err(|e| e.to_string())?;
    }
    if s.scritto.is_dir() {
        copia_cartella(&s.scritto, &d.scritto)?;
    } else {
        std::fs::copy(&s.scritto, &d.scritto).map_err(|e| e.to_string())?;
    }
    Ok(format!("Copiato: {} -> {}", s.testo(), d.testo()))
}

fn copia_cartella(da: &Path, a: &Path) -> Result<(), String> {
    std::fs::create_dir_all(a).map_err(|e| e.to_string())?;
    for voce in std::fs::read_dir(da).map_err(|e| e.to_string())?.flatten() {
        let sotto = a.join(voce.file_name());
        if voce.path().is_dir() {
            copia_cartella(&voce.path(), &sotto)?;
        } else {
            std::fs::copy(voce.path(), &sotto).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Cancella. Nel Cestino, se non si chiede espressamente il contrario.
pub fn cancella(g: &Guardie, sistema: &dyn Sistema, percorso: &str, per_sempre: bool) -> Esito {
    let t = Percorso::nuovo(percorso)?;
    t.controlla(g)?;
    if !t.scritto.exists() {
        return Err(format!("{} non esiste", t.testo()));
    }
    if !per_sempre {
        if sistema.nel_cestino(&t.scritto) {
            return Ok(format!("Spostato nel Cestino: {}", t.testo()));
        }
        return Err(
            "impossibile usare il Cestino. Se serve cancellare davvero, \
             chiedimelo con permanent=true: e' un'azione che non si annulla."
                .into(),
        );
    }
    if t.scritto.is_dir() {
        std::fs::remove_dir_all(&t.scritto).map_err(|e| e.to_string())?;
    } else {
        std::fs::remove_file(&t.scritto).map_err(|e| e.to_string())?;
    }
    Ok(format!("Eliminato definitivamente: {}", t.testo()))
}

// ---------------------------------------------------------------- cercare
pub fn cerca_file(radice: &str, modello: &str, massimo: usize) -> Esito {
    let d = Percorso::nuovo(radice)?;
    if !d.scritto.is_dir() {
        return Err(format!("{} non e' una cartella", d.testo()));
    }
    let allargato = modello_di_ricerca(modello);
    let quale = glob::Pattern::new(&allargato)
        .map_err(|e| format!("errore durante la ricerca: {e}"))?;
    let mut fuori: Vec<String> = Vec::new();
    let massimo = massimo.max(1);
    cammina(&d.scritto, &d.scritto, &quale, massimo, &mut fuori);
    if fuori.is_empty() {
        return Ok(format!("Nessun risultato per '{allargato}' in {}.", d.testo()));
    }
    Ok(fuori.join("\n"))
}

fn cammina(radice: &Path, dove: &Path, quale: &glob::Pattern, massimo: usize, fuori: &mut Vec<String>) {
    if fuori.len() >= massimo {
        return;
    }
    let Ok(lettura) = std::fs::read_dir(dove) else {
        return;
    };
    let mut voci: Vec<_> = lettura.flatten().collect();
    voci.sort_by_key(|v| v.file_name());
    for voce in voci {
        if fuori.len() >= massimo {
            return;
        }
        let p = voce.path();
        if let Ok(rel) = p.strip_prefix(radice) {
            let rel = rel.to_string_lossy().replace('\\', "/");
            if quale.matches(&rel) {
                fuori.push(p.display().to_string());
            }
        }
        if p.is_dir() {
            cammina(radice, &p, quale, massimo, fuori);
        }
    }
}

pub fn cerca_nei_file(radice: &str, testo: &str, modello: &str, massimo: usize) -> Esito {
    let d = Percorso::nuovo(radice)?;
    let modello = if modello.is_empty() { "**/*" } else { modello };
    let quale = glob::Pattern::new(modello)
        .map_err(|e| format!("modello non valido '{modello}': {e}"))?;
    let cercato = testo.to_lowercase();
    let mut trovate: Vec<String> = Vec::new();
    let pieno = setaccia(&d.scritto, &d.scritto, &quale, &cercato, massimo, &mut trovate);
    if pieno {
        return Ok(trovate.join("\n") + "\n[limite risultati raggiunto]");
    }
    if trovate.is_empty() {
        return Ok(format!("Nessuna occorrenza di '{testo}' in {}.", d.testo()));
    }
    Ok(trovate.join("\n"))
}

fn setaccia(
    radice: &Path,
    dove: &Path,
    quale: &glob::Pattern,
    cercato: &str,
    massimo: usize,
    trovate: &mut Vec<String>,
) -> bool {
    let Ok(lettura) = std::fs::read_dir(dove) else {
        return false;
    };
    let mut voci: Vec<_> = lettura.flatten().collect();
    voci.sort_by_key(|v| v.file_name());
    for voce in voci {
        let p = voce.path();
        if p.is_dir() {
            if setaccia(radice, &p, quale, cercato, massimo, trovate) {
                return true;
            }
            continue;
        }
        let Ok(rel) = p.strip_prefix(radice) else { continue };
        let rel = rel.to_string_lossy().replace('\\', "/");
        if !quale.matches(&rel) {
            continue;
        }
        // Un file enorme non si legge riga per riga per cercarci dentro: e'
        // il modo migliore per bloccare NOVA su un log da due gigabyte.
        if std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0) > MAX_BYTE_DA_SETACCIARE {
            continue;
        }
        let Ok(grezzo) = std::fs::read(&p) else { continue };
        let contenuto = String::from_utf8_lossy(&grezzo);
        for (i, riga) in contenuto.lines().enumerate() {
            if riga.to_lowercase().contains(cercato) {
                trovate.push(riga_trovata(&p.display().to_string(), i + 1, riga));
                if trovate.len() >= massimo {
                    return true;
                }
            }
        }
    }
    false
}

/// Cosa si sa di un percorso, senza aprirlo.
pub fn informazioni(percorso: &str, fuso: &dyn crate::data::Fuso)
    -> Result<Vec<(String, String)>, String> {
    let p = Percorso::nuovo(percorso)?;
    let mut fuori = vec![("percorso".to_string(), p.testo())];
    let Ok(m) = std::fs::metadata(&p.scritto) else {
        fuori.push(("esiste".into(), "false".into()));
        return Ok(fuori);
    };
    fuori.push(("esiste".into(), "true".into()));
    fuori.push(("tipo".into(), if m.is_dir() { "cartella" } else { "file" }.into()));
    fuori.push(("byte".into(), m.len().to_string()));
    fuori.push(("misura".into(), crate::file::misura(m.len())));
    fuori.push(("modificato".into(), quando(&m, fuso)));
    Ok(fuori)
}

/// Apre con l'applicazione predefinita.
pub fn apri(sistema: &dyn Sistema, percorso: &str) -> Esito {
    let p = Percorso::nuovo(percorso)?;
    if !p.scritto.exists() {
        return Err(format!("{} non esiste", p.testo()));
    }
    sistema.apri(&p.scritto)?;
    Ok(format!("Aperto: {}", p.testo()))
}

/// Le cartelle che l'utente chiama per nome, nelle due lingue in cui
/// Windows le puo' aver create.
pub const CARTELLE_NOTE: [&str; 10] = [
    "Desktop",
    "Downloads",
    "Documents",
    "Documenti",
    "Pictures",
    "Immagini",
    "Music",
    "Musica",
    "Videos",
    "Video",
];

/// `known_folders`: la casa, le cartelle note **che ci sono davvero**, e le
/// due variabili che il modello chiede piu' spesso.
///
/// Le chiavi sono in minuscolo e `temp` e `appdata` ci sono sempre, anche
/// vuote: e' la forma che il Python ha sempre dato, e un modello che ha
/// imparato a leggere `appdata` non deve scoprire che a volte manca.
pub fn cartelle_note(casa: &Path, temp: &str, appdata: &str) -> Vec<(String, String)> {
    let mut fuori = vec![("home".to_string(), casa.display().to_string())];
    for nome in CARTELLE_NOTE {
        let p = casa.join(nome);
        if p.exists() {
            fuori.push((nome.to_lowercase(), p.display().to_string()));
        }
    }
    fuori.push(("temp".into(), temp.into()));
    fuori.push(("appdata".into(), appdata.into()));
    fuori
}
