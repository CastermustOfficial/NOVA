//! Il banco di confronto per la ricerca dei modelli.
//!
//! Qui il confronto ha una forma diversa dagli altri tre banchi. Ricette,
//! memoria e registro lavorano su dati che si possono inventare; questo
//! lavora su **file veri sul disco vero**, e la stessa cartella vista dai due
//! lati deve dare lo stesso elenco nello stesso ordine, con gli stessi
//! byte e lo stesso proiettore accanto.
//!
//! Non si legge la VRAM ne' si chiama nvidia-smi: i numeri di partenza
//! arrivano da fuori, cosi' il confronto e' ripetibile e non dipende da cosa
//! stava facendo la scheda video in quel momento.

use std::io::Read;
use std::path::PathBuf;

use nova_modelli::gguf;
use nova_modelli::motore::{motori, Motore};
use nova_modelli::strati::strati_su_gpu;
use nova_modelli::trova::{cartelle_note, trova, verifica_file, Come};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct CasoStrati {
    byte_modello: u64,
    n_strati: u32,
    vram_libera_mb: u64,
    ctx: u32,
    riserva_mb: f64,
    kv_tipo: String,
}

#[derive(Deserialize)]
struct Dentro {
    /// Le radici da percorrere. Le passa chi chiama: dentro non c'e' nessuna
    /// idea di cosa sia un disco.
    #[serde(default)]
    radici: Vec<String>,
    #[serde(default = "quattro")]
    profondita: usize,
    #[serde(default = "venti")]
    secondi: f64,
    #[serde(default)]
    minimo: u64,
    #[serde(default = "vero")]
    verifica: bool,
    /// File da controllare uno per uno, come li indicherebbe l'utente a mano.
    #[serde(default)]
    indicati: Vec<String>,
    /// File di cui leggere la forma.
    #[serde(default)]
    forme: Vec<String>,
    /// File di cui misurare la completezza.
    #[serde(default)]
    misure: Vec<String>,
    /// Casi di calcolo degli strati.
    #[serde(default)]
    strati: Vec<CasoStrati>,
    /// Le radici in cui cercare llama-server.
    #[serde(default)]
    motori: Vec<String>,
    /// La cartella del progetto: quello che sta li' vince su tutto.
    #[serde(default)]
    in_casa: String,
    /// Per provare `cartelle_note` senza dipendere da com'e' fatta questa casa.
    #[serde(default)]
    casa: String,
    #[serde(default)]
    locale: String,
    #[serde(default)]
    progetto: String,
}

fn quattro() -> usize {
    4
}
fn venti() -> f64 {
    20.0
}
fn vero() -> bool {
    true
}

#[derive(Serialize)]
struct ModelloFuori {
    percorso: String,
    nome: String,
    cartella: String,
    byte: u64,
    gb: f64,
    proiettore: String,
}

#[derive(Serialize)]
struct VerificaFuori {
    ok: bool,
    percorso: String,
    motivo: String,
    nome: String,
    cartella: String,
    byte: u64,
    gb: f64,
    proiettore: String,
}

#[derive(Serialize)]
struct FormaFuori {
    arch: String,
    nome: String,
    n_strati: u32,
    n_ctx_train: u32,
    n_embd: u32,
}

#[derive(Serialize)]
struct MisuraFuori {
    ok: bool,
    byte: u64,
    byte_minimi: u64,
    tensori: u64,
    completo: bool,
}

#[derive(Serialize)]
struct MotoreFuori {
    percorso: String,
    etichetta: String,
    acceleratore: String,
    priorita: i32,
    versione: [u32; 3],
}

#[derive(Serialize)]
struct Fuori {
    modelli: Vec<ModelloFuori>,
    /// Se il tetto di tempo e' scaduto. Il numero di secondi non si confronta:
    /// e' l'unica cosa che cambia legittimamente fra due esecuzioni.
    troncato: bool,
    indicati: Vec<VerificaFuori>,
    forme: Vec<FormaFuori>,
    strati: Vec<u32>,
    motori: Vec<MotoreFuori>,
    misure: Vec<MisuraFuori>,
    note: Vec<String>,
}

fn main() {
    let mut testo = String::new();
    if std::io::stdin().read_to_string(&mut testo).is_err() {
        eprintln!("non ho potuto leggere l'ingresso");
        std::process::exit(2);
    }
    let dentro: Dentro = match serde_json::from_str(&testo) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("ingresso illeggibile: {e}");
            std::process::exit(2);
        }
    };

    let mut radici: Vec<PathBuf> = dentro.radici.iter().map(PathBuf::from).collect();
    if !dentro.casa.is_empty() {
        let locale = if dentro.locale.is_empty() {
            None
        } else {
            Some(PathBuf::from(&dentro.locale))
        };
        radici.extend(cartelle_note(
            &PathBuf::from(&dentro.casa),
            locale.as_deref(),
            &PathBuf::from(&dentro.progetto),
        ));
    }

    let come = Come {
        radici,
        profondita: dentro.profondita,
        secondi: dentro.secondi,
        minimo: if dentro.minimo == 0 {
            nova_modelli::MINIMO_BYTE
        } else {
            dentro.minimo
        },
        verifica: dentro.verifica,
    };
    let (trovati, resoconto) = trova(&come);

    let fuori = Fuori {
        modelli: trovati
            .into_iter()
            .map(|m| ModelloFuori {
                percorso: m.percorso,
                nome: m.nome,
                cartella: m.cartella,
                byte: m.byte,
                gb: m.gb,
                proiettore: m.proiettore,
            })
            .collect(),
        troncato: resoconto.troncato,
        indicati: dentro
            .indicati
            .iter()
            .map(|s| {
                let v = verifica_file(s);
                VerificaFuori {
                    ok: v.ok,
                    percorso: v.percorso,
                    motivo: v.motivo,
                    nome: v.nome,
                    cartella: v.cartella,
                    byte: v.byte,
                    gb: v.gb,
                    proiettore: v.proiettore,
                }
            })
            .collect(),
        forme: dentro
            .forme
            .iter()
            .map(|s| {
                let f = gguf::forma(&PathBuf::from(s));
                FormaFuori {
                    arch: f.arch,
                    nome: f.nome,
                    n_strati: f.n_strati,
                    n_ctx_train: f.n_ctx_train,
                    n_embd: f.n_embd,
                }
            })
            .collect(),
        strati: dentro
            .strati
            .iter()
            .map(|c| {
                strati_su_gpu(
                    c.byte_modello,
                    c.n_strati,
                    c.vram_libera_mb,
                    c.ctx,
                    c.riserva_mb,
                    &c.kv_tipo,
                )
            })
            .collect(),
        motori: {
            let radici: Vec<PathBuf> = dentro.motori.iter().map(PathBuf::from).collect();
            let casa = if dentro.in_casa.is_empty() {
                None
            } else {
                Some(PathBuf::from(&dentro.in_casa))
            };
            motori(&radici, casa.as_deref())
                .into_iter()
                .map(|m: Motore| MotoreFuori {
                    percorso: m.percorso.to_string_lossy().into_owned(),
                    etichetta: m.etichetta,
                    acceleratore: m.acceleratore.nome().to_string(),
                    priorita: m.priorita,
                    versione: [m.versione.0, m.versione.1, m.versione.2],
                })
                .collect()
        },
        misure: dentro
            .misure
            .iter()
            .map(|s| match gguf::misura(&PathBuf::from(s)) {
                Ok(m) => MisuraFuori {
                    ok: true,
                    byte: m.byte,
                    byte_minimi: m.byte_minimi,
                    tensori: m.tensori,
                    completo: m.completo(),
                },
                Err(_) => MisuraFuori {
                    ok: false,
                    byte: 0,
                    byte_minimi: 0,
                    tensori: 0,
                    completo: false,
                },
            })
            .collect(),
        note: Vec::new(),
    };

    match serde_json::to_string(&fuori) {
        Ok(s) => println!("{s}"),
        Err(e) => {
            eprintln!("non ho potuto scrivere l'uscita: {e}");
            std::process::exit(2);
        }
    }
}
