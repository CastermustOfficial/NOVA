//! Trovare i modelli GGUF che l'utente ha gia' sul disco.
//!
//! Il presupposto e' che l'utente **non** abbia LM Studio. Chi scarica da
//! HuggingFace a mano se lo ritrova in Download, sul Desktop, in `D:\AI`, in
//! una cartella che si chiama come gli pare. Per questo ci sono due modi di
//! cercare:
//!
//! - **lo sguardo veloce**: i posti dove i modelli finiscono di solito, con un
//!   tetto di tempo. Se non trova niente si salta e non si insiste;
//! - **la ricerca vera**: tutti i dischi fissi. Costa minuti, quindi non la si
//!   fa mai a sorpresa: la si fa quando qualcuno l'ha chiesta.
//!
//! Le radici arrivano da fuori. Questo modulo non sa cos'e' un disco - sa
//! percorrere un albero: e' cio' che lo rende provabile su qualsiasi sistema
//! con una cartella finta, invece che solo sulla macchina di chi lo scrive.

use crate::gguf;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Sotto questa soglia non c'e' un modello di linguaggio: c'e' un pezzo di
/// qualcos'altro, o uno scaricamento a meta'.
pub const MINIMO_BYTE: u64 = 100 * 1024 * 1024;

/// Quanto in fondo scendere nei posti noti. Le gerarchie dei gestori di
/// modelli sono `editore/repository/file.gguf`: quattro livelli bastano e
/// avanzano, e impediscono a una cartella Download disordinata di diventare
/// una voragine.
pub const PROFONDITA: usize = 4;

/// Sui dischi interi si scende di piu': un modello puo' stare in
/// `D:\roba\ia\modelli\vecchi\qwen\...` e nessuna profondita' e' quella
/// giusta. Otto e' il compromesso fra trovarlo e finire dentro un albero di
/// sorgenti.
pub const PROFONDITA_OVUNQUE: usize = 8;

/// Cartelle in cui non c'e' mai un modello e che costano care da attraversare.
pub const DA_SALTARE: &[&str] = &[
    "node_modules",
    "__pycache__",
    ".git",
    ".svn",
    "venv",
    ".venv",
    "$recycle.bin",
    "system volume information",
    "windows",
    "temp",
    "tmp",
    "appdata\\locallow",
    "onedrivetemp",
];

/// Preferenze di scelta automatica (parola chiave -> punteggio). Non e' un
/// giudizio sulla qualita' assoluta: e' quale modello va d'accordo con NOVA.
pub const PREFERITI: &[(&str, i64)] = &[
    ("qwen3.8", 100),
    ("qwen3", 90),
    ("qwen", 80),
    ("glm", 40),
    ("gemma", 30),
];

/// Un modello trovato sul disco.
#[derive(Debug, Clone, PartialEq)]
pub struct Trovato {
    pub percorso: String,
    pub nome: String,
    pub cartella: String,
    pub byte: u64,
    /// I gigabyte con un decimale, come li legge una persona.
    pub gb: f64,
    /// Il file che da' la vista al modello, se sta nella stessa cartella.
    pub proiettore: String,
}

/// Com'e' andata la ricerca. Serve a distinguere «non hai modelli» da «non ho
/// fatto in tempo a guardare»: sono due frasi diverse da dire all'utente, e
/// dirne una per l'altra lo manda a cercare il problema dalla parte sbagliata.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Resoconto {
    pub troncato: bool,
    pub secondi: f64,
}

/// Come cercare.
#[derive(Debug, Clone)]
pub struct Come {
    pub radici: Vec<PathBuf>,
    pub profondita: usize,
    pub secondi: f64,
    pub minimo: u64,
    /// Aprire ogni candidato e controllare che sia un GGUF **e che sia
    /// intero**. I primi quattro byte da soli non bastano: uno scaricamento
    /// interrotto ce li ha tutti.
    pub verifica: bool,
}

impl Default for Come {
    fn default() -> Self {
        Come {
            radici: Vec::new(),
            profondita: PROFONDITA,
            secondi: 20.0,
            minimo: MINIMO_BYTE,
            verifica: true,
        }
    }
}

/// I posti dove i modelli finiscono davvero, non solo quelli di LM Studio.
///
/// `casa` e `locale` si passano invece di leggerli qui: e' l'unico modo di
/// provare questa funzione senza dipendere da com'e' fatta la macchina di chi
/// esegue la prova.
pub fn cartelle_note(casa: &Path, locale: Option<&Path>, progetto: &Path) -> Vec<PathBuf> {
    let mut c: Vec<PathBuf> = vec![
        casa.join(".lmstudio").join("models"),
        casa.join(".cache").join("lm-studio").join("models"),
        // scaricato con huggingface-cli
        casa.join(".cache").join("huggingface").join("hub"),
        casa.join(".jan").join("models"),
        casa.join("jan").join("models"),
        // blob senza estensione, ma capita
        casa.join(".ollama").join("models"),
        casa.join("models"),
        casa.join("Downloads"),
        casa.join("Desktop"),
        casa.join("Documents"),
        // dove scarica NOVA
        progetto.join("runtime").join("modelli"),
    ];
    if let Some(l) = locale {
        c.push(l.join("nomic.ai").join("GPT4All"));
        c.push(l.join("llama.cpp"));
    }
    c.into_iter().filter(|p| p.is_dir()).collect()
}

fn da_saltare(nome: &str) -> bool {
    let n = nome.to_lowercase();
    DA_SALTARE.iter().any(|s| *s == n)
}

fn punteggio(percorso: &str, byte: u64) -> (i64, u64) {
    let n = percorso.to_lowercase();
    let mut p = 0;
    for (parola, punti) in PREFERITI {
        if n.contains(parola) {
            p = p.max(*punti);
        }
    }
    (p, byte)
}

/// Il file che da' la vista al modello, se sta nella stessa cartella.
///
/// llama.cpp lo carica solo se glielo si passa. Trovarlo e non dirlo
/// vorrebbe dire lasciare NOVA cieca avendo la vista a un metro.
fn proiettore(percorso: &Path) -> String {
    let dir = match percorso.parent() {
        Some(d) => d,
        None => return String::new(),
    };
    let voci = match fs::read_dir(dir) {
        Ok(v) => v,
        Err(_) => return String::new(),
    };
    // Le cartelle non hanno un ordine garantito: senza ordinare, due
    // proiettori nella stessa cartella darebbero una risposta diversa a ogni
    // esecuzione. Meglio arbitrario e stabile che arbitrario e basta.
    let mut candidati: Vec<PathBuf> = voci
        .flatten()
        .map(|v| v.path())
        .filter(|p| {
            let n = p
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            n.contains("mmproj") && n.ends_with(".gguf")
        })
        .collect();
    candidati.sort();
    candidati
        .first()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Discesa iterativa con tetto di profondita' e di tempo.
///
/// La pila e' esplicita e a ogni giro si guarda l'orologio. Una discesa
/// ricorsiva su un disco intero puo' restare in una cartella per minuti senza
/// che nessuno possa fermarla; qui si puo' sempre smettere.
fn cammina(radice: &Path, profondita: usize, scadenza: Instant, minimo: u64) -> Vec<PathBuf> {
    let mut fuori = Vec::new();
    let mut pila = vec![(radice.to_path_buf(), 0usize)];
    while let Some((cartella, livello)) = pila.pop() {
        if Instant::now() > scadenza {
            return fuori;
        }
        let voci = match fs::read_dir(&cartella) {
            Ok(v) => v,
            Err(_) => continue,
        };
        for v in voci.flatten() {
            let p = v.path();
            let nome = v.file_name().to_string_lossy().to_lowercase();
            // `symlink_metadata`: un collegamento che punta alla propria
            // cartella madre farebbe girare la discesa a vuoto per sempre.
            let meta = match fs::symlink_metadata(&p) {
                Ok(m) => m,
                Err(_) => continue,
            };
            if meta.is_dir() {
                if livello < profondita && !da_saltare(&nome) {
                    pila.push((p, livello + 1));
                }
            } else if meta.is_file() && nome.ends_with(".gguf") {
                if nome.contains("mmproj") {
                    continue; // e' la vista, non il cervello
                }
                if meta.len() >= minimo {
                    fuori.push(p);
                }
            }
        }
    }
    fuori
}

/// I modelli trovati, dal piu' adatto al meno adatto.
///
/// `secondi` e' un tetto, non una stima: scaduto quello si torna con quello
/// che si e' visto finora. Meglio un elenco parziale di un installatore fermo
/// - ma allora bisogna dirlo, ed e' quello che fa il resoconto.
pub fn trova(come: &Come) -> (Vec<Trovato>, Resoconto) {
    let inizio = Instant::now();
    let scadenza = inizio + Duration::from_secs_f64(come.secondi.max(1.0));

    let mut visti: Vec<(String, Trovato)> = Vec::new();
    let mut chiavi: std::collections::HashSet<String> = std::collections::HashSet::new();

    for radice in &come.radici {
        for f in cammina(radice, come.profondita, scadenza, come.minimo) {
            let chiave = f.to_string_lossy().to_lowercase();
            if !chiavi.insert(chiave.clone()) {
                continue;
            }
            if come.verifica && !gguf::utilizzabile(&f) {
                continue;
            }
            let byte = match fs::metadata(&f) {
                Ok(m) => m.len(),
                Err(_) => continue,
            };
            visti.push((
                chiave,
                Trovato {
                    percorso: f.to_string_lossy().into_owned(),
                    nome: f
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    cartella: f
                        .parent()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    byte,
                    gb: arrotonda1(byte as f64 / (1024f64 * 1024.0 * 1024.0)),
                    proiettore: proiettore(&f),
                },
            ));
        }
    }

    let mut fuori: Vec<Trovato> = visti.into_iter().map(|(_, t)| t).collect();
    // Decrescente per punteggio, poi per dimensione. A parita' di entrambi si
    // ordina per percorso: due file identici in due cartelle diverse devono
    // uscire sempre nello stesso ordine, o il «modello consigliato» cambia da
    // un avvio all'altro senza che nulla sia cambiato.
    fuori.sort_by(|a, b| {
        punteggio(&b.percorso, b.byte)
            .cmp(&punteggio(&a.percorso, a.byte))
            .then_with(|| a.percorso.cmp(&b.percorso))
    });

    let passati = inizio.elapsed().as_secs_f64();
    (
        fuori,
        Resoconto {
            troncato: Instant::now() > scadenza,
            secondi: arrotonda1(passati),
        },
    )
}

fn arrotonda1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

/// L'esito del controllo su un file indicato a mano.
#[derive(Debug, Clone, PartialEq)]
pub struct Verifica {
    pub ok: bool,
    pub percorso: String,
    pub motivo: String,
    pub nome: String,
    pub cartella: String,
    pub byte: u64,
    pub gb: f64,
    pub proiettore: String,
}

/// Un solo file, quello che l'utente ha indicato a mano.
///
/// Le virgolette si tolgono perche' Windows le mette: «Copia come percorso»
/// nel menu contestuale produce `"C:\...\modello.gguf"`, virgolette comprese,
/// e un utente che incolla quello ha fatto tutto giusto.
pub fn verifica_file(indicato: &str) -> Verifica {
    let pulito = indicato.trim().trim_matches('"').trim_matches('\'');
    let p = PathBuf::from(pulito);
    let vuota = |motivo: &str| Verifica {
        ok: false,
        percorso: p.to_string_lossy().into_owned(),
        motivo: motivo.to_string(),
        nome: String::new(),
        cartella: String::new(),
        byte: 0,
        gb: 0.0,
        proiettore: String::new(),
    };
    let meta = match fs::metadata(&p) {
        Ok(m) => m,
        Err(_) => return vuota("non esiste"),
    };
    if meta.is_dir() {
        return vuota("e' una cartella, non un file");
    }
    if !gguf::e_gguf(&p) {
        return vuota(
            "non e' un file GGUF: i primi byte non tornano \
             (succede con le pagine di errore salvate col nome giusto \
             o con i file rinominati)",
        );
    }
    // Un file a meta' ha l'intestazione giusta e i tensori no. Va detto qui,
    // adesso, e non fra un minuto sotto forma di llama.cpp che muore.
    match gguf::misura(&p) {
        Ok(m) if !m.completo() => {
            let mancano = m.byte_minimi.saturating_sub(m.byte) / (1024 * 1024);
            return vuota(&format!(
                "e' un GGUF ma non e' finito di scaricare: \
                 mancano almeno {mancano} MB dei suoi {} tensori",
                m.tensori
            ));
        }
        Err(e) => return vuota(&format!("e' un GGUF ma non si legge: {e}")),
        _ => {}
    }
    let byte = meta.len();
    Verifica {
        ok: true,
        percorso: p.to_string_lossy().into_owned(),
        motivo: String::new(),
        nome: p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        cartella: p
            .parent()
            .map(|d| d.to_string_lossy().into_owned())
            .unwrap_or_default(),
        byte,
        gb: arrotonda1(byte as f64 / (1024f64 * 1024.0 * 1024.0)),
        proiettore: proiettore(&p),
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn il_punteggio_premia_qwen_ma_non_alla_cieca() {
        // Piu' specifico vince: «qwen3.8» batte «qwen3» batte «qwen».
        assert!(punteggio("/x/Qwen3.8-27B.gguf", 1) > punteggio("/x/Qwen3-14B.gguf", 1));
        assert!(punteggio("/x/qwen2.gguf", 1) > punteggio("/x/glm-4.gguf", 1));
        // A parita' di parola, il piu' grande vince: e' quasi sempre la
        // quantizzazione migliore dello stesso modello.
        assert!(punteggio("/x/qwen.gguf", 200) > punteggio("/x/qwen.gguf", 100));
        // Uno sconosciuto non sparisce: vale zero, e resta in fondo.
        assert_eq!(punteggio("/x/mistral.gguf", 5).0, 0);
    }

    #[test]
    fn le_cartelle_care_si_saltano() {
        assert!(da_saltare("node_modules"));
        assert!(da_saltare("NODE_MODULES"));
        assert!(da_saltare("$Recycle.Bin"));
        assert!(!da_saltare("models"));
        assert!(!da_saltare("Downloads"));
    }

    #[test]
    fn un_percorso_fra_virgolette_e_lo_stesso_percorso() {
        let a = verifica_file("  \"/non/esiste/x.gguf\"  ");
        let b = verifica_file("/non/esiste/x.gguf");
        assert_eq!(a.percorso, b.percorso);
        assert!(!a.ok && a.motivo == "non esiste");
    }

    #[test]
    fn niente_radici_niente_giro() {
        let (m, r) = trova(&Come::default());
        assert!(m.is_empty());
        assert!(!r.troncato);
    }
}
