//! Il banco di confronto per la memoria: stesse domande, stessi punteggi.
//!
//! Come quello delle ricette: si legge da stdin, si scrive su stdout, e il
//! Python confronta cifra per cifra. Qui pero' i numeri sono in virgola
//! mobile e passano da un logaritmo, quindi il confronto non puo' essere
//! sull'uguaglianza secca: si dichiara la tolleranza dalla parte del Python,
//! e qui si scrive tutto quello che si sa.

use std::io::Read;

use nova_memoria::scelta::{scegli, Candidato, Via};
use nova_memoria::{coseno, rrf, tokenizza, Bm25, Nodo, RRF_K};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct NodoIn {
    slug: String,
    #[serde(default)]
    titolo: String,
    #[serde(default)]
    tag: Vec<String>,
    #[serde(default)]
    corpo: String,
}

#[derive(Deserialize)]
struct Dentro {
    nodi: Vec<NodoIn>,
    domande: Vec<String>,
    /// Ranking gia' pronti da fondere, per provare la fusione da sola.
    #[serde(default)]
    fusioni: Vec<Vec<Vec<(String, f64)>>>,
    /// Coppie di vettori, per provare il coseno.
    #[serde(default)]
    vettori: Vec<(Vec<f64>, Vec<f64>)>,
    /// Corpi da tagliare per il contesto, col loro massimo.
    #[serde(default)]
    tagli: Vec<(String, usize)>,
    /// slug -> data di aggiornamento, per lo spareggio a pari merito.
    #[serde(default)]
    freschezza: std::collections::BTreeMap<String, String>,
    /// Gli scenari di **scelta**: chi entra nel contesto e in che ordine.
    ///
    /// Sono separati dal resto perche' chiedono piu' cose — la confidenza dei
    /// nodi, il tipo, i vicini — che al solo punteggio non servono. Il Python
    /// li prepara e manda anche i punteggi gia' calcolati: cosi' il confronto
    /// e' sulla **politica**, non sull'aritmetica che il banco verifica gia'
    /// altrove.
    #[serde(default)]
    scelte: Vec<ScenarioScelta>,
}

#[derive(Deserialize)]
struct ScenarioScelta {
    domanda: String,
    nodi: Vec<NodoScelta>,
    #[serde(default)]
    sparsi: std::collections::BTreeMap<String, f64>,
    #[serde(default)]
    densi: std::collections::BTreeMap<String, f64>,
    /// slug -> i suoi vicini, nell'ordine in cui il vault li restituisce.
    #[serde(default)]
    vicini: std::collections::BTreeMap<String, Vec<String>>,
    quanti: usize,
    confidenza_minima: f64,
    #[serde(default)]
    espandi_grafo: bool,
}

#[derive(Deserialize)]
struct NodoScelta {
    slug: String,
    #[serde(default)]
    titolo: String,
    #[serde(default)]
    tag: Vec<String>,
    #[serde(default)]
    tipo: String,
    confidenza: f64,
    #[serde(default)]
    aggiornato: String,
}

#[derive(Serialize)]
struct EsitoScelta {
    /// slug, punteggio e **perche'** e' entrato: le tre cose su cui le due
    /// meta' devono essere d'accordo.
    scelti: Vec<(String, f64, String)>,
    scartati_confidenza: usize,
    scartati_esempio: Vec<String>,
    espansi_da_grafo: usize,
}

#[derive(Serialize)]
struct Fuori {
    /// domanda -> (slug, punteggio), ordinati dal piu' alto.
    bm25: Vec<(String, Vec<(String, f64)>)>,
    /// domanda -> parole che l'indice conta.
    parole: Vec<(String, Vec<String>)>,
    fusioni: Vec<Vec<(String, f64)>>,
    coseni: Vec<f64>,
    tagli: Vec<String>,
    scelte: Vec<EsitoScelta>,
}

/// Lo stesso ordine della libreria, non uno riscritto qui.
///
/// Prima questa funzione riordinava per conto suo, e per questo il banco
/// passava mentre dentro `rrf` l'ordine dei pari merito era una lotteria: il
/// riordino in uscita la nascondeva. Adesso chiama `in_ordine`, cosi' se lo
/// spareggio cambia lo fa da tutte e due le parti o da nessuna.
fn ordina(
    m: std::collections::BTreeMap<String, f64>,
    freschezza: &std::collections::BTreeMap<String, String>,
) -> Vec<(String, f64)> {
    nova_memoria::in_ordine(&m, freschezza)
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
    let nodi: Vec<Nodo> = dentro
        .nodi
        .into_iter()
        .map(|n| Nodo { slug: n.slug, titolo: n.titolo, tag: n.tag, corpo: n.corpo })
        .collect();
    let mut indice = Bm25::nuovo();
    indice.indicizza(&nodi);

    let fuori = Fuori {
        bm25: dentro
            .domande
            .iter()
            .map(|d| (d.clone(), ordina(indice.cerca(d), &dentro.freschezza)))
            .collect(),
        parole: dentro
            .domande
            .iter()
            .map(|d| (d.clone(), tokenizza(d)))
            .collect(),
        fusioni: dentro
            .fusioni
            .iter()
            .map(|r| ordina(rrf(r, RRF_K, &dentro.freschezza), &dentro.freschezza))
            .collect(),
        coseni: dentro.vettori.iter().map(|(a, b)| coseno(a, b)).collect(),
        tagli: dentro
            .tagli
            .iter()
            .map(|(c, m)| nova_memoria::testa_e_coda(c, *m))
            .collect(),
        scelte: dentro
            .scelte
            .iter()
            .map(|s| {
                let nodi: std::collections::BTreeMap<String, Candidato> = s
                    .nodi
                    .iter()
                    .map(|n| {
                        (
                            n.slug.clone(),
                            Candidato {
                                slug: n.slug.clone(),
                                titolo: n.titolo.clone(),
                                tag: n.tag.clone(),
                                tipo: n.tipo.clone(),
                                confidenza: n.confidenza,
                                aggiornato: n.aggiornato.clone(),
                            },
                        )
                    })
                    .collect();
                let vicini = |slug: &str| -> Vec<String> {
                    s.vicini.get(slug).cloned().unwrap_or_default()
                };
                let (scelti, r) = scegli(
                    &s.domanda,
                    &nodi,
                    &s.sparsi,
                    &s.densi,
                    &vicini,
                    s.quanti,
                    s.confidenza_minima,
                    s.espandi_grafo,
                );
                EsitoScelta {
                    scelti: scelti
                        .into_iter()
                        .map(|x| {
                            let via = match x.via {
                                Via::Esatto => "esatto",
                                Via::Fusione => "fusione",
                                Via::Grafo => "grafo",
                            };
                            (x.slug, x.punteggio, via.to_string())
                        })
                        .collect(),
                    scartati_confidenza: r.scartati_confidenza,
                    scartati_esempio: r.scartati_esempio,
                    espansi_da_grafo: r.espansi_da_grafo,
                }
            })
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
