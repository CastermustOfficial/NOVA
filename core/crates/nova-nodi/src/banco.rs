//! Il banco: una riga JSON per domanda, per il confronto col Python.

use nova_nodi::{come_lista, dividi_frontmatter, slug, Nodo};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(tag = "tipo")]
enum Domanda {
    #[serde(rename = "slug")]
    Slug { testo: String },
    #[serde(rename = "scrivi")]
    Scrivi {
        #[serde(default)]
        nodo: NodoJson,
        oggi: String,
    },
    #[serde(rename = "leggi")]
    Leggi { testo: String, slug: String },
    #[serde(rename = "relazioni")]
    Relazioni {
        #[serde(default)]
        nodo: NodoJson,
    },
    #[serde(rename = "lista")]
    Lista { testo: String },
    #[serde(rename = "frontmatter")]
    Frontmatter { testo: String },
}

#[derive(Deserialize, Serialize, Default, Clone)]
struct NodoJson {
    #[serde(default)]
    slug: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    body: String,
    #[serde(default = "fatto")]
    tipo: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    relazioni: Vec<String>,
    #[serde(default = "generale")]
    area: String,
    #[serde(default = "attivo")]
    status: String,
    #[serde(default = "auto")]
    origine: String,
    #[serde(default = "sette")]
    confidenza: f64,
    #[serde(default)]
    riferimenti: Vec<String>,
    #[serde(default)]
    creato: String,
    #[serde(default)]
    aggiornato: String,
}

fn fatto() -> String { "fatto".into() }
fn generale() -> String { "Generale".into() }
fn attivo() -> String { "attivo".into() }
fn auto() -> String { "auto".into() }
fn sette() -> f64 { 0.7 }

impl From<NodoJson> for Nodo {
    fn from(j: NodoJson) -> Self {
        Nodo {
            slug: j.slug, title: j.title, body: j.body, tipo: j.tipo,
            tags: j.tags, relazioni: j.relazioni, area: j.area,
            status: j.status, origine: j.origine, confidenza: j.confidenza,
            riferimenti: j.riferimenti, creato: j.creato,
            aggiornato: j.aggiornato,
        }
    }
}

impl From<Nodo> for NodoJson {
    fn from(n: Nodo) -> Self {
        NodoJson {
            slug: n.slug, title: n.title, body: n.body, tipo: n.tipo,
            tags: n.tags, relazioni: n.relazioni, area: n.area,
            status: n.status, origine: n.origine, confidenza: n.confidenza,
            riferimenti: n.riferimenti, creato: n.creato,
            aggiornato: n.aggiornato,
        }
    }
}

#[derive(Serialize)]
struct Risposta {
    #[serde(skip_serializing_if = "Option::is_none")]
    slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    markdown: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nodo: Option<NodoJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    relazioni: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lista: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    frontmatter: Option<Vec<(String, String)>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    corpo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    errore: Option<String>,
}

fn vuota() -> Risposta {
    Risposta { slug: None, markdown: None, nodo: None, relazioni: None,
               lista: None, frontmatter: None, corpo: None, errore: None }
}

fn main() {
    let mut dentro = String::new();
    if std::io::Read::read_to_string(&mut std::io::stdin(), &mut dentro).is_err() {
        eprintln!("non riesco a leggere stdin");
        std::process::exit(1);
    }
    let dentro = dentro.strip_prefix('\u{feff}').unwrap_or(&dentro);
    for riga in dentro.lines().filter(|r| !r.trim().is_empty()) {
        let r = match serde_json::from_str::<Domanda>(riga) {
            // Una domanda illeggibile lo dice: un banco che risponde comunque
            // fa passare un confronto che non e' mai avvenuto.
            Err(e) => Risposta { errore: Some(format!("{e}")), ..vuota() },
            Ok(Domanda::Slug { testo }) => Risposta { slug: Some(slug(&testo)), ..vuota() },
            Ok(Domanda::Scrivi { nodo, oggi }) => {
                let n: Nodo = nodo.into();
                Risposta { markdown: Some(n.a_markdown(&oggi)), ..vuota() }
            }
            Ok(Domanda::Leggi { testo, slug }) => {
                let n = Nodo::da_markdown(&testo, &slug);
                Risposta { nodo: Some(n.into()), ..vuota() }
            }
            Ok(Domanda::Relazioni { nodo }) => {
                let n: Nodo = nodo.into();
                Risposta { relazioni: Some(n.tutte_le_relazioni()), ..vuota() }
            }
            Ok(Domanda::Lista { testo }) => {
                Risposta { lista: Some(come_lista(Some(&testo))), ..vuota() }
            }
            Ok(Domanda::Frontmatter { testo }) => {
                let (fm, corpo) = dividi_frontmatter(&testo);
                Risposta { frontmatter: Some(fm), corpo: Some(corpo), ..vuota() }
            }
        };
        println!("{}", serde_json::to_string(&r).unwrap());
    }
}
