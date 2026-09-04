//! Il banco: una riga JSON per domanda, per il confronto col Python.

use nova_nodi::fusione;
use nova_nodi::deposito::{Deposito, Disco, DiscoScrivibile, Impronta, NessunControllo};
use nova_nodi::posto;
use std::collections::{BTreeMap, HashMap};
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
    #[serde(rename = "fondi")]
    Fondi {
        #[serde(default)]
        vecchio: NodoJson,
        #[serde(default)]
        nuovo: NodoJson,
    },
    #[serde(rename = "limita")]
    Limita { testo: String, massimo: usize },
    #[serde(rename = "tipo")]
    TipoPiuSpecifico { vecchio: String, nuovo: String },
    #[serde(rename = "wikilink")]
    Wikilink { corpo: String, vecchio: String, nuovo: String },
    #[serde(rename = "prefisso")]
    Prefisso { testo: String },
    // Il campo si chiama `tipo_nodo` e non `tipo` perche' `tipo` e' gia'
    // l'etichetta che sceglie la domanda: due campi con lo stesso nome e
    // serde legge la domanda sbagliata senza dire niente.
    #[serde(rename = "cartella")]
    Cartella { tipo_nodo: String },
    #[serde(rename = "percorso")]
    Percorso { tipo_nodo: String, slug: String },
    /// Un vault raccontato passo per passo: a ogni passo il disco e' quello
    /// che dice la mappa, e si guarda cosa ne pensa il deposito.
    #[serde(rename = "vault")]
    Vault {
        #[serde(default)]
        passi: Vec<BTreeMap<String, String>>,
        #[serde(default)]
        distingue_maiuscole: bool,
    },
    /// Una sequenza di salvataggi su un vault che parte da un certo stato.
    /// Alla fine si guarda cosa c'e' su disco, file per file.
    #[serde(rename = "salva")]
    Salva {
        #[serde(default)]
        partenza: BTreeMap<String, String>,
        #[serde(default)]
        nodi: Vec<NodoJson>,
        oggi: String,
    },
    #[serde(rename = "slug_libero")]
    SlugLibero {
        tipo_nodo: String,
        slug: String,
        #[serde(default)]
        esistenti: HashMap<String, String>,
    },
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
    testo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pezzi: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    passi: Option<Vec<StatoVault>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    disco: Option<Vec<(String, String)>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rifiuti: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    errore: Option<String>,
}

#[derive(Serialize)]
struct StatoVault {
    slug: Vec<String>,
    dove: Vec<(String, String)>,
    titoli: Vec<(String, String)>,
    collisioni: Vec<(String, Vec<String>)>,
}

/// Il disco del banco: una cartella descritta in JSON. L'impronta e' la
/// lunghezza del testo piu' il testo stesso, cosi' due contenuti diversi
/// hanno impronte diverse senza bisogno di un orologio.
struct DiscoFinto {
    file: std::cell::RefCell<BTreeMap<String, String>>,
}

impl Disco for DiscoFinto {
    fn elenca(&self) -> Vec<String> {
        self.file.borrow().keys().cloned().collect()
    }
    fn impronta(&self, dove: &str) -> Option<Impronta> {
        self.file.borrow().get(dove).map(|t| Impronta {
            quando: impronta_del_testo(t),
            quanto: t.len() as i64,
        })
    }
    fn leggi(&self, dove: &str) -> Option<String> {
        self.file.borrow().get(dove).cloned()
    }
}

impl DiscoScrivibile for DiscoFinto {
    fn scrivi(&self, dove: &str, testo: &str) -> Result<Impronta, String> {
        self.file.borrow_mut().insert(dove.to_string(), testo.to_string());
        self.impronta(dove).ok_or_else(|| "sparito".to_string())
    }
}

fn impronta_del_testo(t: &str) -> f64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in t.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    (h % 1_000_000) as f64
}

fn vuota() -> Risposta {
    Risposta { slug: None, markdown: None, nodo: None, relazioni: None,
               lista: None, frontmatter: None, corpo: None, testo: None,
               pezzi: None, passi: None, disco: None, rifiuti: None,
               errore: None }
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
            Ok(Domanda::Fondi { vecchio, nuovo }) => {
                let v: Nodo = vecchio.into();
                let n: Nodo = nuovo.into();
                Risposta { nodo: Some(fusione::fondi(&v, &n).into()), ..vuota() }
            }
            Ok(Domanda::Limita { testo, massimo }) => Risposta {
                testo: Some(fusione::limita_corpo(&testo, massimo)),
                ..vuota()
            },
            Ok(Domanda::TipoPiuSpecifico { vecchio, nuovo }) => Risposta {
                testo: Some(fusione::tipo_piu_specifico(&vecchio, &nuovo).to_string()),
                ..vuota()
            },
            Ok(Domanda::Wikilink { corpo, vecchio, nuovo }) => Risposta {
                testo: Some(fusione::rinomina_wikilink(&corpo, &vecchio, &nuovo)),
                ..vuota()
            },
            Ok(Domanda::Prefisso { testo }) => Risposta {
                testo: Some(fusione::senza_prefisso(&testo)),
                ..vuota()
            },
            Ok(Domanda::Cartella { tipo_nodo }) => Risposta {
                testo: Some(posto::sottocartella(&tipo_nodo).to_string()),
                ..vuota()
            },
            Ok(Domanda::Percorso { tipo_nodo, slug }) => Risposta {
                pezzi: Some(posto::percorso_relativo(&tipo_nodo, &slug)),
                ..vuota()
            },
            Ok(Domanda::Vault { passi, distingue_maiuscole }) => {
                let mut v = Deposito::nuovo(distingue_maiuscole);
                let mut fuori = Vec::new();
                for (i, mappa) in passi.iter().enumerate() {
                    let disco = DiscoFinto { file: std::cell::RefCell::new(mappa.clone()) };
                    // Il primo passo e' un'apertura del vault, i successivi
                    // sono cio' che l'utente ha combinato in Obsidian mentre
                    // NOVA era accesa.
                    if i == 0 {
                        v.ricarica(&disco);
                    } else {
                        v.aggiorna(&disco);
                    }
                    fuori.push(StatoVault {
                        slug: v.tutti().map(|n| n.slug.clone()).collect(),
                        dove: v
                            .tutti()
                            .map(|n| (n.slug.clone(), v.dove(&n.slug).unwrap_or("").to_string()))
                            .collect(),
                        titoli: v
                            .tutti()
                            .map(|n| (n.slug.clone(), n.title.clone()))
                            .collect(),
                        collisioni: v.collisioni.clone().into_iter().collect(),
                    });
                }
                Risposta { passi: Some(fuori), ..vuota() }
            }
            Ok(Domanda::Salva { partenza, nodi, oggi }) => {
                let disco = DiscoFinto { file: std::cell::RefCell::new(partenza) };
                let mut v = Deposito::nuovo(false);
                v.ricarica(&disco);
                let mut rifiuti = Vec::new();
                for j in nodi {
                    let n: Nodo = j.into();
                    if let Err(motivo) = v.salva(&disco, &NessunControllo, n, true, &oggi) {
                        rifiuti.push(motivo);
                    }
                }
                let fuori: Vec<(String, String)> =
                    disco.file.borrow().iter().map(|(k, t)| (k.clone(), t.clone())).collect();
                Risposta { disco: Some(fuori), rifiuti: Some(rifiuti), ..vuota() }
            }
            Ok(Domanda::SlugLibero { tipo_nodo, slug, esistenti }) => Risposta {
                slug: Some(posto::slug_libero(&tipo_nodo, &slug, &esistenti)),
                ..vuota()
            },
        };
        println!("{}", serde_json::to_string(&r).unwrap());
    }
}
