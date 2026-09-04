//! Il banco degli strumenti: lo schema che va al modello, e la riga che
//! legge l'utente.
//!
//! Il confronto sullo schema e' **carattere per carattere**, e non e'
//! pignoleria: quel testo e' il prompt su cui il modello sceglie quale
//! strumento usare. Una parola diversa in una descrizione e' un
//! comportamento diverso, e non c'e' nessun tipo che se ne accorga.

use nova_strumenti::file;
use nova_strumenti::file_disco::{self, SenzaSistema};
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
    /// Misure di file da scrivere come le legge una persona.
    #[serde(default)]
    misure: Vec<u64>,
    /// (quante righe, offset, limite) per la fetta di una lettura.
    #[serde(default)]
    fette: Vec<(usize, i64, i64)>,
    /// Modelli di ricerca da allargare.
    #[serde(default)]
    ricerche: Vec<String>,
    /// (percorso, numero, riga) per una riga trovata.
    #[serde(default)]
    trovate: Vec<(String, usize, String)>,
    /// (nome, e_cartella) da ordinare.
    #[serde(default)]
    da_ordinare: Vec<(String, bool)>,
    /// Le operazioni sui file da eseguire davvero, in ordine.
    #[serde(default)]
    operazioni: Vec<Operazione>,
    /// Il fuso, in secondi. Arriva da fuori come tutte le cose che dipendono
    /// dal mondo: cosi' il banco non cambia risposta a marzo e a ottobre.
    #[serde(default)]
    fuso: i64,
}

#[derive(Deserialize)]
#[serde(tag = "che")]
enum Operazione {
    #[serde(rename = "elenca")]
    Elenca { dove: String, #[serde(default)] modello: String, #[serde(default)] nascosti: bool },
    #[serde(rename = "leggi")]
    Leggi { dove: String, #[serde(default)] offset: i64, #[serde(default)] limite: i64 },
    #[serde(rename = "scrivi")]
    Scrivi { dove: String, testo: String, #[serde(default)] in_coda: bool },
    #[serde(rename = "modifica")]
    Modifica { dove: String, vecchio: String, nuovo: String, #[serde(default)] tutte: bool },
    #[serde(rename = "cartella")]
    Cartella { dove: String },
    #[serde(rename = "sposta")]
    Sposta { da: String, a: String, #[serde(default)] sovrascrivi: bool },
    #[serde(rename = "copia")]
    Copia { da: String, a: String },
    #[serde(rename = "cancella")]
    Cancella { dove: String, #[serde(default)] per_sempre: bool },
    #[serde(rename = "cerca")]
    Cerca { dove: String, modello: String, #[serde(default = "cento")] massimo: usize },
    #[serde(rename = "setaccia")]
    Setaccia { dove: String, testo: String, #[serde(default)] modello: String,
               #[serde(default = "sessanta")] massimo: usize },
    #[serde(rename = "info")]
    Info { dove: String },
}

fn cento() -> usize { 100 }
fn sessanta() -> usize { 60 }

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
    misure: Vec<String>,
    fette: Vec<(usize, usize)>,
    ricerche: Vec<String>,
    trovate: Vec<String>,
    ordinati: Vec<String>,
    /// Per ogni operazione: quello che il modello leggerebbe, con davanti
    /// «ERRORE: » se e' andata storta — come fa `run_tool` dall'altra parte.
    operazioni: Vec<String>,
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
        misure: d.misure.iter().map(|b| file::misura(*b)).collect(),
        fette: d.fette.iter().map(|(q, o, l)| file::fetta(*q, *o, *l)).collect(),
        ricerche: d.ricerche.iter().map(|r| file::modello_di_ricerca(r)).collect(),
        trovate: d.trovate.iter()
            .map(|(p, n, r)| file::riga_trovata(p, *n, r)).collect(),
        ordinati: {
            let mut v: Vec<(String, bool, String)> = d.da_ordinare.iter()
                .map(|(n, c)| {
                    let k = file::chiave_ordine(n, *c);
                    (n.clone(), k.0, k.1)
                })
                .collect();
            v.sort_by(|a, b| (a.1, &a.2).cmp(&(b.1, &b.2)));
            v.into_iter().map(|(n, _, _)| n).collect()
        },
        operazioni: {
            let g = Guardie::nuove(&d.protetti, &d.radici, &d.vietati,
                                   Autonomia::dal_nome(&d.autonomia));
            let sistema = SenzaSistema;
            d.operazioni.iter().map(|op| {
                let esito = match op {
                    Operazione::Elenca { dove, modello, nascosti } =>
                        file_disco::elenca(dove, modello, *nascosti, d.fuso),
                    Operazione::Leggi { dove, offset, limite } =>
                        file_disco::leggi(dove, *offset, *limite),
                    Operazione::Scrivi { dove, testo, in_coda } =>
                        file_disco::scrivi(&g, dove, testo, *in_coda),
                    Operazione::Modifica { dove, vecchio, nuovo, tutte } =>
                        file_disco::modifica(&g, dove, vecchio, nuovo, *tutte),
                    Operazione::Cartella { dove } => file_disco::crea_cartella(&g, dove),
                    Operazione::Sposta { da, a, sovrascrivi } =>
                        file_disco::sposta(&g, da, a, *sovrascrivi),
                    Operazione::Copia { da, a } => file_disco::copia(&g, da, a),
                    Operazione::Cancella { dove, per_sempre } =>
                        file_disco::cancella(&g, &sistema, dove, *per_sempre),
                    Operazione::Cerca { dove, modello, massimo } =>
                        file_disco::cerca_file(dove, modello, *massimo),
                    Operazione::Setaccia { dove, testo, modello, massimo } =>
                        file_disco::cerca_nei_file(dove, testo, modello, *massimo),
                    Operazione::Info { dove } => file_disco::informazioni(dove, d.fuso)
                        .map(|v| v.iter().map(|(k, x)| format!("{k}: {x}"))
                             .collect::<Vec<_>>().join("\n")),
                };
                match esito {
                    Ok(t) => t,
                    Err(e) => format!("ERRORE: {e}"),
                }
            }).collect()
        },
    };
    println!("{}", serde_json::to_string(&fuori).unwrap());
}
