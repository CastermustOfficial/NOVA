//! Il banco di confronto per i giudizi: stessi logit, stesse risposte.
//!
//! Si legge da stdin, si scrive su stdout, e il Python confronta. I numeri
//! passano da un esponenziale e da una divisione, quindi il confronto non puo'
//! essere sull'uguaglianza secca: la tolleranza la dichiara il Python, e qui si
//! scrive tutto quello che si sa.

use std::io::Read;

use nova_giudizio::candidati::{candidati, testo_della_domanda};
use nova_giudizio::domanda::{Ancora, Opzione};
use nova_giudizio::giudica::giudica;
use nova_giudizio::probabilita::{concentrazione, morbido, senza_prioria, statistiche};
use nova_giudizio::{Domanda, Giudizio, Politica, Risposta};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct PoliticaIn {
    #[serde(default = "si")]
    puo_astenersi: bool,
    #[serde(default = "meta")]
    massimo_indisponibile: f64,
    #[serde(default)]
    minimo_in_testa: f64,
}

fn si() -> bool {
    true
}
fn meta() -> f64 {
    0.5
}

impl From<PoliticaIn> for Politica {
    fn from(p: PoliticaIn) -> Politica {
        Politica {
            puo_astenersi: p.puo_astenersi,
            massimo_indisponibile: p.massimo_indisponibile,
            minimo_in_testa: p.minimo_in_testa,
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "tipo")]
enum DomandaIn {
    #[serde(rename = "booleana")]
    Booleana {
        istruzioni: String,
        vero: String,
        falso: String,
        politica: PoliticaIn,
    },
    #[serde(rename = "scelta")]
    Scelta {
        istruzioni: String,
        opzioni: Vec<(String, String)>,
        politica: PoliticaIn,
    },
    #[serde(rename = "punteggio")]
    Punteggio {
        istruzioni: String,
        livelli: Vec<String>,
        politica: PoliticaIn,
    },
    #[serde(rename = "numerica")]
    Numerica {
        istruzioni: String,
        unita: String,
        ancore: Vec<(f64, String)>,
        politica: PoliticaIn,
    },
}

impl From<DomandaIn> for Domanda {
    fn from(d: DomandaIn) -> Domanda {
        match d {
            DomandaIn::Booleana {
                istruzioni,
                vero,
                falso,
                politica,
            } => Domanda::Booleana {
                istruzioni,
                descrizione_vero: vero,
                descrizione_falso: falso,
                politica: politica.into(),
            },
            DomandaIn::Scelta {
                istruzioni,
                opzioni,
                politica,
            } => Domanda::Scelta {
                istruzioni,
                opzioni: opzioni
                    .into_iter()
                    .map(|(id, descrizione)| Opzione { id, descrizione })
                    .collect(),
                politica: politica.into(),
            },
            DomandaIn::Punteggio {
                istruzioni,
                livelli,
                politica,
            } => Domanda::Punteggio {
                istruzioni,
                livelli,
                politica: politica.into(),
            },
            DomandaIn::Numerica {
                istruzioni,
                unita,
                ancore,
                politica,
            } => Domanda::Numerica {
                istruzioni,
                unita,
                ancore: ancore
                    .into_iter()
                    .map(|(valore, descrizione)| Ancora {
                        valore,
                        descrizione,
                    })
                    .collect(),
                politica: politica.into(),
            },
        }
    }
}

#[derive(Deserialize)]
struct Caso {
    domanda: DomandaIn,
    logit: Vec<f64>,
    #[serde(default = "uno")]
    temperatura: f64,
    /// Se c'e', si toglie prima di leggere.
    #[serde(default)]
    priorita: Option<Vec<f64>>,
}

fn uno() -> f64 {
    1.0
}

#[derive(Deserialize)]
struct Dentro {
    casi: Vec<Caso>,
    /// Vettori sciolti, per provare la sola aritmetica.
    #[serde(default)]
    morbidi: Vec<(Vec<f64>, f64)>,
    #[serde(default)]
    priorita: Vec<(Vec<f64>, Vec<f64>)>,
    #[serde(default)]
    medie: Vec<(Vec<f64>, Vec<f64>)>,
    #[serde(default)]
    concentrazioni: Vec<Vec<f64>>,
}

#[derive(Serialize)]
struct EsitoFuori {
    /// `risposto | non_basta | fuori_scala | incerto`
    come: String,
    /// La risposta ridotta a numeri e stringhe, quando c'e'.
    valore: Option<f64>,
    scelta: Option<String>,
    normalizzato: Option<f64>,
    probabilita_vero: Option<f64>,
    /// (id, probabilita') nell'ordine delle lettere.
    probabilita: Vec<(String, f64)>,
    indisponibile: f64,
    in_testa: f64,
    concentrazione: f64,
    statistiche: Option<[f64; 5]>,
    racconto: String,
    /// Il testo che andrebbe al modello: cambia le risposte, quindi si confronta.
    testo: String,
    /// Le descrizioni dei candidati, nell'ordine delle lettere.
    candidati: Vec<(String, String)>,
}

#[derive(Serialize)]
struct Fuori {
    casi: Vec<Result<EsitoFuori, String>>,
    morbidi: Vec<Result<Vec<f64>, String>>,
    priorita: Vec<Result<Vec<f64>, String>>,
    medie: Vec<Result<[f64; 5], String>>,
    concentrazioni: Vec<f64>,
}

fn un_caso(caso: Caso) -> Result<EsitoFuori, String> {
    let domanda: Domanda = caso.domanda.into();
    let logit = match &caso.priorita {
        Some(p) => senza_prioria(&caso.logit, p)?,
        None => caso.logit.clone(),
    };
    let e = giudica(&domanda, &logit, caso.temperatura)?;
    let (come, valore, scelta, normalizzato, probabilita_vero) = match &e.giudizio {
        Giudizio::Risposto(Risposta::Booleana {
            valore,
            probabilita_vero,
        }) => (
            "risposto",
            Some(f64::from(u8::from(*valore))),
            None,
            None,
            Some(*probabilita_vero),
        ),
        Giudizio::Risposto(Risposta::Scelta { id }) => {
            ("risposto", None, Some(id.clone()), None, None)
        }
        Giudizio::Risposto(Risposta::Punteggio {
            punti,
            normalizzato,
        }) => ("risposto", Some(*punti), None, Some(*normalizzato), None),
        Giudizio::Risposto(Risposta::Numerica { valore, .. }) => {
            ("risposto", Some(*valore), None, None, None)
        }
        Giudizio::NonBasta { .. } => ("non_basta", None, None, None, None),
        Giudizio::FuoriScala { .. } => ("fuori_scala", None, None, None, None),
        Giudizio::Incerto { .. } => ("incerto", None, None, None, None),
    };
    Ok(EsitoFuori {
        come: come.to_string(),
        valore,
        scelta,
        normalizzato,
        probabilita_vero,
        probabilita: e.probabilita.clone(),
        indisponibile: e.indisponibile,
        in_testa: e.in_testa,
        concentrazione: e.concentrazione,
        statistiche: e
            .statistiche
            .as_ref()
            .map(|s| [s.media, s.scarto, s.mediana, s.decimo, s.novantesimo]),
        racconto: e.giudizio.come_si_racconta(),
        testo: testo_della_domanda(&domanda),
        candidati: candidati(&domanda)
            .into_iter()
            .map(|c| (c.id, c.descrizione))
            .collect(),
    })
}

fn main() {
    let mut testo = String::new();
    if std::io::stdin().read_to_string(&mut testo).is_err() {
        eprintln!("non ho potuto leggere da stdin");
        std::process::exit(2);
    }
    let dentro: Dentro = match serde_json::from_str(&testo) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("il JSON in ingresso non si legge: {e}");
            std::process::exit(2);
        }
    };
    let fuori = Fuori {
        casi: dentro.casi.into_iter().map(un_caso).collect(),
        morbidi: dentro.morbidi.iter().map(|(l, t)| morbido(l, *t)).collect(),
        priorita: dentro
            .priorita
            .iter()
            .map(|(l, p)| senza_prioria(l, p))
            .collect(),
        medie: dentro
            .medie
            .iter()
            .map(|(v, p)| {
                statistiche(v, p).map(|s| [s.media, s.scarto, s.mediana, s.decimo, s.novantesimo])
            })
            .collect(),
        concentrazioni: dentro
            .concentrazioni
            .iter()
            .map(|p| concentrazione(p))
            .collect(),
    };
    match serde_json::to_string(&fuori) {
        Ok(s) => println!("{s}"),
        Err(e) => {
            eprintln!("non ho potuto scrivere l'esito: {e}");
            std::process::exit(2);
        }
    }
}
