//! Il banco degli strumenti: lo schema che va al modello, e la riga che
//! legge l'utente.
//!
//! Il confronto sullo schema e' **carattere per carattere**, e non e'
//! pignoleria: quel testo e' il prompt su cui il modello sceglie quale
//! strumento usare. Una parola diversa in una descrizione e' un
//! comportamento diverso, e non c'e' nessun tipo che se ne accorga.

use nova_strumenti::guardie::{Autonomia, Guardie};
use nova_strumenti::{anteprima, schema, Argomenti, Rischio, STRUMENTI};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct Dentro {
    /// (nome dello strumento, argomenti) di cui si vuole l'anteprima.
    #[serde(default)]
    anteprime: Vec<(String, BTreeMap<String, serde_json::Value>)>,
    /// Le regole con cui provare le guardie.
    #[serde(default)]
    protetti: Vec<String>,
    #[serde(default)]
    radici: Vec<String>,
    #[serde(default)]
    vietati: Vec<String>,
    #[serde(default)]
    autonomia: String,
    /// I percorsi da sottoporre alla guardia di scrittura.
    /// (percorso, dove porta) — la destinazione e' opzionale.
    #[serde(default)]
    scritture: Vec<(String, Option<String>)>,
    /// I comandi da sottoporre alla guardia dei comandi.
    #[serde(default)]
    comandi: Vec<String>,
}

#[derive(Serialize)]
struct Fuori {
    /// nome -> schema JSON, come stringa: si confronta il testo, non
    /// l'albero, perche' e' il testo che finisce nel prompt.
    schemi: Vec<(String, String)>,
    dichiarazioni: Vec<Dichiarazione>,
    anteprime: Vec<String>,
    /// Per ogni percorso: niente se si puo' scrivere, il messaggio se no.
    scritture: Vec<Option<String>>,
    comandi: Vec<Option<String>>,
    /// Se serve chiedere, per ognuno dei tre rischi.
    permessi: Vec<bool>,
    motivi_incomprensibili: Vec<String>,
}

#[derive(Serialize)]
struct Dichiarazione {
    nome: String,
    rischio: u8,
    categoria: String,
    obbligatori: Vec<String>,
    parametri: Vec<String>,
}

/// Gli argomenti come arrivano dal JSON.
struct DaJson(BTreeMap<String, serde_json::Value>);

impl Argomenti for DaJson {
    fn campo(&self, nome: &str) -> Option<String> {
        self.0.get(nome).map(|v| match v {
            // Il testo di un JSON non ha le virgolette attorno: `str()` di
            // Python su una stringa non le mette, e qui si racconta la stessa
            // cosa allo stesso modo.
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Null => "None".into(),
            serde_json::Value::Bool(b) => if *b { "True".into() } else { "False".into() },
            altro => altro.to_string(),
        })
    }
    fn campi(&self) -> Vec<String> {
        self.0.keys().cloned().collect()
    }
    fn acceso(&self, nome: &str) -> bool {
        match self.0.get(nome) {
            None | Some(serde_json::Value::Null) => false,
            Some(serde_json::Value::Bool(b)) => *b,
            Some(serde_json::Value::String(s)) => !s.is_empty(),
            Some(serde_json::Value::Number(n)) => n.as_f64().unwrap_or(0.0) != 0.0,
            Some(serde_json::Value::Array(a)) => !a.is_empty(),
            Some(serde_json::Value::Object(o)) => !o.is_empty(),
        }
    }
}

fn main() {
    let mut testo = String::new();
    if std::io::Read::read_to_string(&mut std::io::stdin(), &mut testo).is_err() {
        eprintln!("non riesco a leggere stdin");
        std::process::exit(1);
    }
    let testo = testo.strip_prefix('\u{feff}').unwrap_or(&testo);
    let d: Dentro = match serde_json::from_str(testo) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("ingresso illeggibile: {e}");
            std::process::exit(2);
        }
    };
    let fuori = Fuori {
        schemi: STRUMENTI.iter().map(|s| (s.nome.to_string(), schema(s))).collect(),
        dichiarazioni: STRUMENTI
            .iter()
            .map(|s| Dichiarazione {
                nome: s.nome.to_string(),
                rischio: s.rischio.numero(),
                categoria: s.categoria.to_string(),
                obbligatori: s.obbligatori.iter().map(|x| x.to_string()).collect(),
                parametri: s.parametri.iter().map(|p| p.nome.to_string()).collect(),
            })
            .collect(),
        anteprime: d
            .anteprime
            .into_iter()
            .map(|(nome, args)| anteprima(&nome, &DaJson(args)))
            .collect(),
        scritture: {
            let g = Guardie::nuove(&d.protetti, &d.radici, &d.vietati,
                                   Autonomia::dal_nome(&d.autonomia));
            d.scritture.iter()
                .map(|(p, r)| g.puo_scrivere(p, r.as_deref()).err().map(|e| e.messaggio()))
                .collect()
        },
        comandi: {
            let g = Guardie::nuove(&d.protetti, &d.radici, &d.vietati,
                                   Autonomia::dal_nome(&d.autonomia));
            d.comandi.iter()
                .map(|c| g.comando_permesso(c).err().map(|e| e.messaggio()))
                .collect()
        },
        permessi: {
            let g = Guardie::nuove(&d.protetti, &d.radici, &d.vietati,
                                   Autonomia::dal_nome(&d.autonomia));
            [Rischio::Innocuo, Rischio::Modifica, Rischio::Pericoloso]
                .iter().map(|r| g.serve_permesso(*r)).collect()
        },
        motivi_incomprensibili: Guardie::nuove(&d.protetti, &d.radici, &d.vietati,
                                               Autonomia::dal_nome(&d.autonomia))
            .motivi_incomprensibili,
    };
    println!("{}", serde_json::to_string(&fuori).unwrap());
}
