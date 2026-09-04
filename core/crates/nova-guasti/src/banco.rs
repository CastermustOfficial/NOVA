//! Il banco di confronto per i guasti.
//!
//! Due cose si confrontano qui, e la seconda pesa piu' della prima.
//!
//! Le **frasi** devono essere identiche, o NOVA parla con due voci a seconda
//! di quale meta' di se stessa sta rispondendo.
//!
//! Il **mascheramento delle chiavi** deve essere identico o piu' largo. Una
//! divergenza in cui il Rust copre qualcosa in piu' e' un fastidio; una in cui
//! copre qualcosa in meno e' una chiave che esce, e non si accorge nessuno
//! finche' non e' troppo tardi.

use std::io::Read;

use nova_guasti::{senza_chiavi, spiega, spiega_irraggiungibile, Guasto};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct CasoGuasto {
    tipo: String,
    #[serde(default)]
    nome: String,
    #[serde(default)]
    cosa: String,
    #[serde(default)]
    winerror: u32,
    #[serde(default)]
    testo: String,
    #[serde(default)]
    riga: u32,
    #[serde(default)]
    colonna: u32,
}

#[derive(Deserialize)]
struct Dentro {
    #[serde(default)]
    guasti: Vec<CasoGuasto>,
    #[serde(default)]
    da_mascherare: Vec<String>,
    /// (url, in_casa)
    #[serde(default)]
    irraggiungibili: Vec<(String, bool)>,
    #[serde(default)]
    moduli: Vec<String>,
    /// Testi da sottoporre al guardiano del vault: entrano o no, e col nome
    /// di cosa ci si e' trovato dentro.
    #[serde(default)]
    da_giudicare: Vec<String>,
}

#[derive(Serialize)]
struct Fuori {
    frasi: Vec<String>,
    mascherati: Vec<String>,
    irraggiungibili: Vec<String>,
    pacchetti: Vec<String>,
    giudizi: Vec<Option<String>>,
}

fn main() {
    let mut testo = String::new();
    if std::io::stdin().read_to_string(&mut testo).is_err() {
        eprintln!("non ho potuto leggere l'ingresso");
        std::process::exit(2);
    }
    let d: Dentro = match serde_json::from_str(&testo) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("ingresso illeggibile: {e}");
            std::process::exit(2);
        }
    };

    let frasi = d
        .guasti
        .iter()
        .map(|c| {
            let g = match c.tipo.as_str() {
                "non_trovato" => Guasto::NonTrovato { nome: &c.nome },
                "cartella" => Guasto::EUnaCartella { nome: &c.nome },
                "permesso" => Guasto::PermessoNegato { nome: &c.nome },
                "nessuna_risposta" => Guasto::NessunaRisposta,
                "troppo_tempo" => Guasto::TroppoTempo,
                "non_e_testo" => Guasto::NonETesto { nome: &c.nome },
                "json" => Guasto::JsonRotto {
                    riga: c.riga,
                    colonna: c.colonna,
                },
                "libreria" => Guasto::MancaUnaLibreria { nome: &c.nome },
                "memoria" => Guasto::MemoriaFinita,
                "disco" => Guasto::DiscoPieno,
                "avvitato" => Guasto::Avvitato,
                "sistema" => Guasto::DalSistema {
                    winerror: c.winerror,
                    testo: &c.testo,
                },
                _ => Guasto::Altro {
                    messaggio: &c.testo,
                },
            };
            spiega(&g, &c.cosa)
        })
        .collect();

    let fuori = Fuori {
        frasi,
        mascherati: d.da_mascherare.iter().map(|t| senza_chiavi(t)).collect(),
        irraggiungibili: d
            .irraggiungibili
            .iter()
            .map(|(u, c)| spiega_irraggiungibile(u, *c))
            .collect(),
        pacchetti: d
            .moduli
            .iter()
            .map(|m| nova_guasti::pacchetto_di(m).to_string())
            .collect(),
        giudizi: d
            .da_giudicare
            .iter()
            .map(|t| {
                nova_guasti::guardiano::perche_non_si_salva(t).map(|s| s.to_string())
            })
            .collect(),
    };

    match serde_json::to_string(&fuori) {
        Ok(s) => println!("{s}"),
        Err(e) => {
            eprintln!("non ho potuto scrivere l'uscita: {e}");
            std::process::exit(2);
        }
    }
}
