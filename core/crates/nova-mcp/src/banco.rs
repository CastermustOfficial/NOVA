//! Il banco del protocollo: le due meta' devono rispondere la stessa busta.
//!
//! Qui il confronto e' letterale e va bene cosi': il protocollo **e'** il
//! testo che passa sul tubo. Una chiave in piu', una in meno, un `id` di tipo
//! diverso — e chi sta dall'altra parte non e' un modello che si arrangia,
//! e' un programma che si pianta.

use std::io::Read;

use nova_mcp::{
    allega, gestisci, in_chiaro, rischio, risposta_permesso, Allegato, Esito, Strumenti,
    PROTOCOLLO, STRUMENTI_JSON, VERSIONI_NOTE,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Strumenti finti guidati dal caso: chi esiste, chi esplode.
struct Finti {
    esistenti: Vec<String>,
    esplodono: Vec<String>,
}

impl Strumenti for Finti {
    fn chiama(&self, nome: &str, argomenti: &Value) -> Result<String, String> {
        if self.esplodono.iter().any(|x| x == nome) {
            return Err(format!("{nome} non ce l'ha fatta"));
        }
        Ok(format!(
            "{nome} ha risposto con {}",
            nova_mcp::json_come_python(argomenti)
        ))
    }
    fn esiste(&self, nome: &str) -> bool {
        self.esistenti.iter().any(|x| x == nome)
    }
}

#[derive(Deserialize)]
struct CasoRichiesta {
    richiesta: Value,
    #[serde(default)]
    esistenti: Vec<String>,
    #[serde(default)]
    esplodono: Vec<String>,
}

#[derive(Deserialize)]
struct AllegatoIn {
    percorso: String,
    #[serde(default)]
    testo: Option<String>,
    #[serde(default)]
    perche: Option<String>,
}

#[derive(Deserialize)]
struct CasoAllegato {
    #[serde(default)]
    contesto: String,
    #[serde(default)]
    file: Vec<AllegatoIn>,
}

#[derive(Deserialize)]
struct Dentro {
    #[serde(default)]
    richieste: Vec<CasoRichiesta>,
    /// (strumento, argomenti) da giudicare e da raccontare.
    #[serde(default)]
    permessi: Vec<(String, Value)>,
    #[serde(default)]
    allegati: Vec<CasoAllegato>,
    /// (esito, motivo, argomenti) della richiesta di conferma.
    #[serde(default)]
    permessi_chiesti: Vec<(String, String, Value)>,
}

#[derive(Serialize)]
struct Fuori {
    /// `null` quando non si deve rispondere: e' un caso, non un vuoto.
    risposte: Vec<Option<Value>>,
    rischi: Vec<String>,
    domande: Vec<String>,
    allegati: Vec<String>,
    /// Le dichiarazioni cosi' come le vede il compilatore: si confrontano con
    /// il Python carattere per carattere, se no il banco confronta il Rust
    /// con se stesso.
    strumenti: String,
    risposte_permesso: Vec<String>,
    /// Le versioni del protocollo che questa meta' dice di sapere parlare.
    /// Si confrontano con quelle del Python: sono generate da li', e una
    /// cosa generata resta uguale solo finche' qualcuno lo verifica.
    versioni: Vec<String>,
    protocollo: String,
}

fn main() {
    let mut testo = String::new();
    if std::io::stdin().read_to_string(&mut testo).is_err() {
        eprintln!("non ho potuto leggere l'entrata");
        std::process::exit(2);
    }
    let d: Dentro = match serde_json::from_str(&testo) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("l'entrata non e' JSON valido: {e}");
            std::process::exit(2);
        }
    };

    let fuori = Fuori {
        risposte: d
            .richieste
            .iter()
            .map(|c| {
                let f = Finti {
                    esistenti: c.esistenti.clone(),
                    esplodono: c.esplodono.clone(),
                };
                gestisci(&c.richiesta, &f)
            })
            .collect(),
        rischi: d
            .permessi
            .iter()
            .map(|(s, a)| rischio(s, a).to_string())
            .collect(),
        domande: d.permessi.iter().map(|(s, a)| in_chiaro(s, a)).collect(),
        allegati: d
            .allegati
            .iter()
            .map(|c| {
                let file: Vec<Allegato> = c
                    .file
                    .iter()
                    .map(|f| match (&f.testo, &f.perche) {
                        (Some(t), _) => Allegato::Letto {
                            percorso: f.percorso.clone(),
                            testo: t.clone(),
                        },
                        (None, Some(p)) => Allegato::Illeggibile {
                            percorso: f.percorso.clone(),
                            perche: p.clone(),
                        },
                        _ => Allegato::Letto {
                            percorso: f.percorso.clone(),
                            testo: String::new(),
                        },
                    })
                    .collect();
                allega(&c.contesto, &file)
            })
            .collect(),
        strumenti: STRUMENTI_JSON.to_string(),
        risposte_permesso: d
            .permessi_chiesti
            .iter()
            .map(|(quale, motivo, argomenti)| {
                let e = match quale.as_str() {
                    "senza_demone" => Esito::SenzaDemone { perche: motivo },
                    "non_chiesto" => Esito::NonChiesto { perche: motivo },
                    "consentito" => Esito::Consentito,
                    "scaduto" => Esito::Scaduto,
                    _ => Esito::Negato { motivo },
                };
                risposta_permesso(&e, argomenti)
            })
            .collect(),
        versioni: VERSIONI_NOTE.iter().map(|v| v.to_string()).collect(),
        protocollo: PROTOCOLLO.to_string(),
    };

    match serde_json::to_string(&fuori) {
        Ok(s) => println!("{s}"),
        Err(e) => {
            eprintln!("non ho potuto scrivere l'uscita: {e}");
            std::process::exit(2);
        }
    }
}
