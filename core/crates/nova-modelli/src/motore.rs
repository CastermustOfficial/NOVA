//! Trovare llama-server, e capire con che cosa e' stato costruito.
//!
//! L'altra meta' di «cosa serve prima del primo avvio»: un modello non basta,
//! ci vuole il motore che lo fa girare, e non tutti i motori sono uguali.
//! Quello compilato con CUDA usa la scheda video; quello CPU no, e la
//! differenza fra i due e' un fattore dieci che nessuno annuncia.
//!
//! Come per i modelli, le radici da percorrere arrivano da fuori: qui non si
//! sa cosa sia una cartella di LM Studio ne' dove stia la casa dell'utente.
//! Serve a poterlo provare con un albero finto, che e' l'unico modo di
//! provare i casi che contano senza costruirli davvero su questo disco.
//!
//! ## Tre difetti che il porto ha fatto uscire
//!
//! Il codice Python da cui viene questo modulo aveva tre trappole, tutte
//! della stessa famiglia: **una stringa usata al posto di una struttura**.
//! Sono corrette in tutte e due le parti, perche' il Python e' quello che
//! gira oggi.
//!
//! 1. La priorita' assoluta ai binari del progetto si decideva con
//!    `str(exe).startswith(radice/"runtime")`. Un prefisso non e' un
//!    percorso: `runtime-vecchio` e `runtime_backup` passavano il controllo e
//!    si prendevano la precedenza sul motore buono. E' lo stesso errore del
//!    `bin/` nel `.gitignore`, che valeva per qualunque cartella chiamata
//!    cosi'.
//! 2. L'acceleratore si indovinava con `"cuda" in percorso_intero`. Chi ha la
//!    cartella utente dentro `C:\cuda\...`, o un disco montato `/mnt/cuda`,
//!    si vedeva classificare come CUDA anche il motore CPU. Qui si guardano i
//!    **componenti** del percorso, non la stringa.
//! 3. La versione si cercava in tutto il percorso. `C:\v1.2.3\llama\...`
//!    dava la versione della cartella del nonno. Qui si guarda solo il nome
//!    della cartella del binario, che e' dove i nomi la mettono davvero
//!    (`llama.cpp-win-x86_64-vulkan-avx2-2.31.2`).

use std::fs;
use std::path::{Path, PathBuf};

/// Il nome dell'eseguibile su questo sistema.
///
/// Il Python cercava `llama-server.exe` e basta: su Linux e su macOS il file
/// si chiama senza estensione, quindi non trovava mai niente e NOVA
/// concludeva che non ci fosse un motore. Un'altra promessa che si sarebbe
/// rotta sulla macchina di qualcun altro.
pub const NOME_SERVER: &str = if cfg!(windows) {
    "llama-server.exe"
} else {
    "llama-server"
};

/// Come si chiamano le librerie condivise qui.
pub const ESTENSIONE_LIBRERIA: &str = if cfg!(windows) {
    ".dll"
} else if cfg!(target_os = "macos") {
    ".dylib"
} else {
    ".so"
};

/// Quanto in fondo cercare dentro una radice. Le cartelle dei motori sono
/// `radice/nome-versione/llama-server`: due livelli bastano, e impediscono a
/// un `LLAMA_CPP_HOME` puntato per sbaglio sulla home di diventare una
/// scansione del disco.
pub const PROFONDITA: usize = 3;

/// Con che cosa e' stato costruito il motore, e quanto conviene.
///
/// Il numero e' un ordine di preferenza, non un voto: piu' basso, prima.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Acceleratore {
    Cuda,
    Vulkan,
    Rocm,
    Cpu,
}

impl Acceleratore {
    pub fn nome(&self) -> &'static str {
        match self {
            Acceleratore::Cuda => "cuda",
            Acceleratore::Vulkan => "vulkan",
            Acceleratore::Rocm => "rocm",
            Acceleratore::Cpu => "cpu",
        }
    }

    pub fn priorita(&self) -> i32 {
        match self {
            Acceleratore::Cuda => 0,
            Acceleratore::Vulkan => 1,
            Acceleratore::Rocm => 2,
            Acceleratore::Cpu => 3,
        }
    }
}

/// Un motore trovato sul disco.
#[derive(Debug, Clone, PartialEq)]
pub struct Motore {
    pub percorso: PathBuf,
    /// Il nome della cartella che lo contiene: e' quello che una persona
    /// riconosce in un elenco.
    pub etichetta: String,
    pub acceleratore: Acceleratore,
    pub priorita: i32,
    /// La versione letta dal nome della cartella, `(0,0,0)` se non c'e'.
    pub versione: (u32, u32, u32),
}

/// Quanto si guadagna a stare dentro la cartella del progetto.
///
/// E' un salto, non un pollice: il binario che NOVA si e' scaricata da sola
/// deve battere qualunque cosa ci sia in giro, anche un CUDA piu' recente,
/// perche' e' l'unico di cui conosciamo la provenienza.
pub const SCONTO_IN_CASA: i32 = 10;

/// La versione dentro un nome di cartella: `...-vulkan-avx2-2.31.2` -> (2,31,2).
///
/// Si guarda **solo** il nome della cartella del binario. Cercarla in tutto
/// il percorso vuol dire raccogliere il numero di versione di una cartella
/// qualunque che sta piu' in alto.
pub fn versione_da(nome: &str) -> (u32, u32, u32) {
    let cifre: Vec<&str> = nome
        .split(|c: char| !c.is_ascii_digit() && c != '.')
        .filter(|p| !p.is_empty())
        .collect();
    // Si prende l'ultima terna: i nomi mettono la versione in coda, dopo
    // l'architettura e le estensioni del processore.
    for pezzo in cifre.iter().rev() {
        let n: Vec<&str> = pezzo.split('.').filter(|x| !x.is_empty()).collect();
        if n.len() >= 3 {
            if let (Ok(a), Ok(b), Ok(c)) = (n[0].parse(), n[1].parse(), n[2].parse()) {
                return (a, b, c);
            }
        }
    }
    (0, 0, 0)
}

/// I componenti del percorso, minuscoli, spezzati anche sui trattini.
///
/// `llama.cpp-win-x86_64-nvidia-cuda12-avx2-1.65.0` diventa fra gli altri
/// `nvidia` e `cuda12`. E' la differenza fra riconoscere un nome e trovare
/// tre lettere dentro una stringa qualunque.
fn parole_del_percorso(p: &Path) -> Vec<String> {
    p.components()
        .filter_map(|c| c.as_os_str().to_str())
        .flat_map(|s| {
            s.to_lowercase()
                .split(|c: char| !c.is_ascii_alphanumeric())
                .filter(|x| !x.is_empty())
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
        })
        .collect()
}

fn ha_parola(parole: &[String], radice: &str) -> bool {
    // `cuda` deve trovare anche `cuda12` — le versioni si attaccano al nome —
    // ma non `cudale`, che e' il cognome di qualcuno dentro il percorso della
    // sua cartella utente. La regola e' quindi: la parola intera, oppure la
    // parola seguita **da sole cifre**.
    //
    // La prima versione di questa funzione si fermava a `starts_with`, e la
    // prova qui sotto l'ha bocciata subito: era la stessa forma dei tre
    // difetti che questo modulo e' venuto a correggere, riscritta mentre li
    // correggevo.
    parole.iter().any(|w| {
        w == radice
            || (w.starts_with(radice) && w[radice.len()..].chars().all(|c| c.is_ascii_digit()))
    })
}

/// Con che cosa e' stato costruito, guardando prima le librerie che gli
/// stanno accanto e poi il nome.
///
/// Le librerie vengono prima perche' sono la prova: il nome e' quello che
/// qualcuno ha scritto, `ggml-cuda.dll` e' quello che c'e' davvero.
pub fn acceleratore_di(percorso: &Path) -> Acceleratore {
    let mut librerie: Vec<String> = Vec::new();
    if let Some(dir) = percorso.parent() {
        if let Ok(voci) = fs::read_dir(dir) {
            for v in voci.flatten() {
                let n = v.file_name().to_string_lossy().to_lowercase();
                if n.ends_with(ESTENSIONE_LIBRERIA) {
                    librerie.push(n);
                }
            }
        }
    }
    let ha = |gambo: &str| librerie.iter().any(|l| l.contains(gambo));
    let parole = parole_del_percorso(percorso);

    if ha("ggml-cuda") || ha_parola(&parole, "cuda") {
        return Acceleratore::Cuda;
    }
    if ha("ggml-vulkan") || ha_parola(&parole, "vulkan") {
        return Acceleratore::Vulkan;
    }
    if ha("ggml-hip") || ha_parola(&parole, "rocm") || ha_parola(&parole, "hip") {
        return Acceleratore::Rocm;
    }
    Acceleratore::Cpu
}

/// Se `figlio` sta davvero dentro `padre`.
///
/// Non `starts_with` sulla stringa: `runtime-vecchio` comincia per `runtime`
/// e non ci sta dentro. Il confronto e' sui componenti, che e' quello che
/// «dentro» vuol dire.
pub fn dentro(figlio: &Path, padre: &Path) -> bool {
    figlio.starts_with(padre)
}

/// Discesa con tetto di profondita', in cerca dell'eseguibile.
fn cerca(radice: &Path, profondita: usize, fuori: &mut Vec<PathBuf>) {
    let mut pila = vec![(radice.to_path_buf(), 0usize)];
    while let Some((dir, livello)) = pila.pop() {
        let voci = match fs::read_dir(&dir) {
            Ok(v) => v,
            Err(_) => continue,
        };
        for v in voci.flatten() {
            let p = v.path();
            let meta = match fs::symlink_metadata(&p) {
                Ok(m) => m,
                Err(_) => continue,
            };
            if meta.is_dir() {
                if livello < profondita {
                    pila.push((p, livello + 1));
                }
            } else if v.file_name().to_string_lossy().eq_ignore_ascii_case(NOME_SERVER) {
                fuori.push(p);
            }
        }
    }
}

/// Tutti i motori utilizzabili, il migliore per primo.
///
/// `in_casa` e' la cartella del progetto: quello che sta li' dentro vince su
/// tutto, perche' e' l'unico di cui si conosce la provenienza.
pub fn motori(radici: &[PathBuf], in_casa: Option<&Path>) -> Vec<Motore> {
    let mut trovati: Vec<PathBuf> = Vec::new();
    for r in radici {
        cerca(r, PROFONDITA, &mut trovati);
    }

    let mut visti: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut fuori: Vec<Motore> = Vec::new();
    for p in trovati {
        // Su Windows lo stesso file puo' arrivare scritto con maiuscole
        // diverse da due radici diverse: sarebbero due voci per un file solo.
        if !visti.insert(p.to_string_lossy().to_lowercase()) {
            continue;
        }
        let acceleratore = acceleratore_di(&p);
        let etichetta = p
            .parent()
            .and_then(|d| d.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut priorita = acceleratore.priorita();
        if let Some(casa) = in_casa {
            if dentro(&p, casa) {
                priorita -= SCONTO_IN_CASA;
            }
        }
        fuori.push(Motore {
            versione: versione_da(&etichetta),
            percorso: p,
            etichetta,
            acceleratore,
            priorita,
        });
    }

    // Priorita' crescente, poi versione decrescente, poi percorso: due motori
    // pari devono uscire sempre nello stesso ordine, o «il migliore» cambia
    // da un avvio all'altro senza che nulla sia cambiato.
    fuori.sort_by(|a, b| {
        a.priorita
            .cmp(&b.priorita)
            .then_with(|| b.versione.cmp(&a.versione))
            .then_with(|| a.percorso.cmp(&b.percorso))
    });
    fuori
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn la_versione_si_legge_in_coda_al_nome() {
        assert_eq!(versione_da("llama.cpp-win-x86_64-vulkan-avx2-2.31.2"), (2, 31, 2));
        assert_eq!(versione_da("llama.cpp-win-x86_64-nvidia-cuda12-avx2-1.63.1"), (1, 63, 1));
        assert_eq!(versione_da("runtime"), (0, 0, 0));
    }

    #[test]
    fn le_versioni_si_ordinano_da_numeri_non_da_lettere() {
        // Il caso che c'e' davvero su questa macchina: 2.8.0, 2.28.2, 2.31.2.
        // Per una stringa «2.8.0» viene dopo «2.31.2»; per dei numeri no.
        let mut v = [
            versione_da("x-2.8.0"),
            versione_da("x-2.31.2"),
            versione_da("x-2.28.2"),
        ];
        v.sort();
        assert_eq!(v, [(2, 8, 0), (2, 28, 2), (2, 31, 2)]);
    }

    #[test]
    fn il_numero_del_processore_non_e_una_versione() {
        // `x86_64` e `avx2` sono pieni di cifre e non sono versioni.
        assert_eq!(versione_da("llama.cpp-win-x86_64-avx2"), (0, 0, 0));
    }

    #[test]
    fn cuda_e_una_parola_non_tre_lettere() {
        // Chi si chiama Cudale e tiene i motori in casa sua non ha una
        // scheda NVIDIA per questo.
        let parole = parole_del_percorso(Path::new("/home/cudale/motori/cpu/llama-server"));
        assert!(!ha_parola(&parole, "cuda"), "{parole:?}");
        // Ma `cuda12` e' CUDA: la versione si attacca al nome.
        let parole =
            parole_del_percorso(Path::new("/x/llama.cpp-win-nvidia-cuda12-avx2/llama-server"));
        assert!(ha_parola(&parole, "cuda"), "{parole:?}");
        // E `cuda` da solo pure.
        let parole = parole_del_percorso(Path::new("/x/llama-cuda/llama-server"));
        assert!(ha_parola(&parole, "cuda"), "{parole:?}");
    }

    #[test]
    fn runtime_vecchio_non_e_dentro_runtime() {
        // Il difetto trovato portando: `startswith` su stringa diceva di si',
        // e un binario di scorta si prendeva la precedenza assoluta.
        let casa = Path::new("/p/NOVA/runtime");
        assert!(dentro(Path::new("/p/NOVA/runtime/llama-server"), casa));
        assert!(!dentro(Path::new("/p/NOVA/runtime-vecchio/llama-server"), casa));
        assert!(!dentro(Path::new("/p/NOVA/runtime_backup/llama-server"), casa));
        assert!(!dentro(Path::new("/p/NOVA/runtime2/llama-server"), casa));
    }

    #[test]
    fn senza_radici_nessun_motore() {
        assert!(motori(&[], None).is_empty());
    }
}
