//! Cosa NOVA si ricorda, e in che ordine.
//!
//! `nova-memoria` sapeva gia' **pesare** — BM25, la fusione dei ranking, lo
//! spareggio. Qui c'e' la parte che con quei pesi ci **decide**: chi entra nel
//! contesto del modello e chi resta fuori.
//!
//! E' la piu' delicata di tutta la memoria, per una ragione sola: cio' che
//! resta fuori non lascia traccia. Un nodo pesato male si vede — sta nel posto
//! sbagliato dell'elenco. Un nodo scartato no: il modello risponde come se non
//! esistesse, e nessuno ha modo di accorgersene.
//!
//! **Cosa non c'e', e non e' una dimenticanza.** Non c'e' l'embedder: i
//! punteggi «densi» arrivano gia' calcolati, come i «sparsi». La ragione e'
//! la stessa per cui non c'e' il disco (vedi il modulo principale): questa e'
//! la politica, e la politica e' aritmetica. Chi calcola i vettori e' un'altra
//! cosa, e su una macchina senza modello locale non esiste affatto.
//!
//! I sei passi sono quelli del Python, nello stesso ordine, con le stesse
//! costanti — e l'ordine conta: il filtro sulla confidenza sta **prima** del
//! taglio, non dopo. Filtrare a valle vuol dire che i nodi scartati si
//! mangiano i posti buoni e poi spariscono, e chi guarda vede un elenco corto
//! senza sapere perche'.

use std::collections::{BTreeMap, BTreeSet};

use crate::{in_ordine, rrf, tokenizza, RRF_K};

/// Il punteggio con cui entra un nodo chiamato **per nome**.
///
/// Mille, cioe' fuori scala rispetto all'RRF, che vive intorno a 1/61. Non e'
/// un peso: e' un «questo lo voleva lui», e deve stare davanti a qualunque
/// cosa il calcolo abbia trovato.
pub const BOOST_ESATTO: f64 = 1000.0;

/// Quanto vale al massimo un tag che compare nella domanda.
///
/// La differenza fra il primo e il quarto posto dell'RRF: abbastanza da
/// spostare qualcosa, non abbastanza da ribaltare la ricerca.
pub fn bonus_tag_max() -> f64 {
    1.0 / (RRF_K as f64 + 1.0) - 1.0 / (RRF_K as f64 + 4.0)
}

/// Il tetto al punteggio che un vicino puo' ereditare dalla sua sorgente.
///
/// Senza, il vicino di un nodo nominato per nome valeva 350 — un terzo di
/// mille — e spingeva fuori dal contesto tutto quello che la ricerca aveva
/// trovato davvero. Il tetto e' il primo posto dell'RRF: un vicino puo'
/// valere quanto un buon risultato, mai quanto una richiesta esplicita.
pub fn tetto_punteggio_grafo() -> f64 {
    1.0 / (RRF_K as f64 + 1.0)
}

/// Quanto si sconta un vicino rispetto alla sua sorgente.
pub const SCONTO_GRAFO: f64 = 0.35;

/// Un nodo, per quel che serve a **sceglierlo**.
#[derive(Debug, Clone, Default)]
pub struct Candidato {
    pub slug: String,
    pub titolo: String,
    pub tag: Vec<String>,
    pub tipo: String,
    pub confidenza: f64,
    /// La data dell'ultimo tocco, come stringa ordinabile. Serve solo allo
    /// spareggio: a pari punteggio vince il piu' recente.
    pub aggiornato: String,
}

/// Perche' un nodo e' finito nel contesto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    /// La domanda lo nominava.
    Esatto,
    /// L'ha trovato la ricerca.
    Fusione,
    /// E' vicino a qualcosa che la ricerca ha trovato.
    Grafo,
}

#[derive(Debug, Clone)]
pub struct Scelto {
    pub slug: String,
    pub punteggio: f64,
    pub via: Via,
}

/// Cio' che serve per raccontare **cosa e' stato lasciato fuori**.
#[derive(Debug, Clone, Default)]
pub struct Resoconto {
    pub scartati_confidenza: usize,
    pub scartati_esempio: Vec<String>,
    pub espansi_da_grafo: usize,
    pub nodi_in_memoria: usize,
}

/// Titolo o domanda ridotti a uno slug, **come lo riduce il Python**.
///
/// Serve a riconoscere che la domanda nomina un nodo. La riduzione toglie gli
/// accenti scomponendo i caratteri, poi tiene solo lettere e cifre ASCII: se
/// qui si divergesse dal Python, «parlami di Città» troverebbe il nodo da una
/// parte e non dall'altra.
pub fn come_slug(testo: &str) -> String {
    let mut fuori = String::new();
    let mut ultimo_trattino = false;
    for c in testo.chars() {
        let base = scomponi(c);
        if base.is_ascii_alphanumeric() {
            fuori.push(base.to_ascii_lowercase());
            ultimo_trattino = false;
        } else if !ultimo_trattino {
            fuori.push('-');
            ultimo_trattino = true;
        }
    }
    let ridotto = fuori.trim_matches('-');
    let tagliato: String = ridotto.chars().take(80).collect();
    if tagliato.is_empty() {
        "nodo".to_string()
    } else {
        tagliato
    }
}

/// Una lettera accentata ridotta alla sua base, quando ne ha una.
///
/// E' la parte di `unicodedata.normalize("NFKD")` che serve qui: il Python
/// scompone e butta i segni combinanti. Tenere una tabella invece di una
/// libreria di normalizzazione e' una scelta: copre le lettere che compaiono
/// nei titoli veri, e per tutto il resto il carattere diventa un trattino —
/// che e' esattamente cio' che farebbe anche il Python.
fn scomponi(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        'ñ' => 'n',
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'A',
        'È' | 'É' | 'Ê' | 'Ë' => 'E',
        'Ì' | 'Í' | 'Î' | 'Ï' => 'I',
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' => 'O',
        'Ù' | 'Ú' | 'Û' | 'Ü' => 'U',
        'Ç' => 'C',
        'Ñ' => 'N',
        altro => altro,
    }
}

/// Chi entra nel contesto, in che ordine, e cosa e' rimasto fuori.
///
/// `vicini` risponde «chi e' collegato a questo slug»: e' l'unica cosa che
/// questa funzione non sa fare da sola, e arriva da chi ha il grafo.
#[allow(clippy::too_many_arguments)]
pub fn scegli(
    domanda: &str,
    nodi: &BTreeMap<String, Candidato>,
    sparsi: &BTreeMap<String, f64>,
    densi: &BTreeMap<String, f64>,
    vicini: &dyn Fn(&str) -> Vec<String>,
    quanti: usize,
    confidenza_minima: f64,
    espandi_grafo: bool,
) -> (Vec<Scelto>, Resoconto) {
    let mut resoconto = Resoconto {
        nodi_in_memoria: nodi.len(),
        ..Default::default()
    };
    if nodi.is_empty() {
        return (Vec::new(), resoconto);
    }

    // 1. la domanda nomina un nodo? Allora e' quello, e salta anche il filtro.
    let q_slug = come_slug(domanda);
    let q_minuscola = domanda.trim().to_lowercase();
    let q_tok: BTreeSet<String> = tokenizza(domanda).into_iter().collect();
    let mut esatti: BTreeMap<String, f64> = BTreeMap::new();
    let mut bonus_tag: BTreeMap<String, f64> = BTreeMap::new();
    for (slug, n) in nodi {
        if *slug == q_slug || n.titolo.trim().to_lowercase() == q_minuscola {
            esatti.insert(slug.clone(), BOOST_ESATTO);
            continue;
        }
        if q_tok.is_empty() {
            continue;
        }
        let comuni = n
            .tag
            .iter()
            .filter(|t| q_tok.contains(&t.to_lowercase()))
            .count();
        if comuni > 0 {
            // Proporzionale a quanta parte della domanda copre: un tag su sei
            // parole conta poco, tre su quattro contano.
            bonus_tag.insert(
                slug.clone(),
                bonus_tag_max() * (comuni as f64 / q_tok.len() as f64),
            );
        }
    }

    // 2. fusione dei due ranking, con lo spareggio sulla freschezza.
    let freschezza: BTreeMap<String, String> = nodi
        .iter()
        .map(|(s, n)| (s.clone(), n.aggiornato.clone()))
        .collect();
    // `rrf` prende ranking gia' in forma di elenco: e' la stessa forma che
    // usa il Python, e non e' un dettaglio — un ranking e' un ordine, non una
    // mappa, e passarlo come mappa vorrebbe dire ricostruirlo due volte.
    let come_elenco = |m: &BTreeMap<String, f64>| -> Vec<(String, f64)> {
        m.iter().map(|(s, p)| (s.clone(), *p)).collect()
    };
    let mut fusi = rrf(&[come_elenco(sparsi), come_elenco(densi)], RRF_K, &freschezza);
    for (slug, boost) in &esatti {
        *fusi.entry(slug.clone()).or_insert(0.0) += boost;
    }
    for (slug, bonus) in &bonus_tag {
        *fusi.entry(slug.clone()).or_insert(0.0) += bonus;
    }

    // 3. il filtro **prima** del taglio, mai a valle.
    let mut candidati: BTreeMap<String, f64> = BTreeMap::new();
    let mut scartati: Vec<String> = Vec::new();
    for (slug, punteggio) in &fusi {
        let Some(n) = nodi.get(slug) else { continue };
        // Gli hub sono indici, non ricordi: nominano altri nodi e non dicono
        // niente da soli.
        if n.tipo == "hub" {
            continue;
        }
        if n.confidenza < confidenza_minima && !esatti.contains_key(slug) {
            // Chi e' stato chiamato per nome si vede comunque: chiedere
            // «parlami di X» e ricevere zero risultati perche' X e' poco
            // confidente non e' un filtro, e' una bugia.
            scartati.push(slug.clone());
            continue;
        }
        candidati.insert(slug.clone(), *punteggio);
    }
    scartati.sort();
    resoconto.scartati_confidenza = scartati.len();
    resoconto.scartati_esempio = scartati.into_iter().take(5).collect();

    let ordinati = in_ordine(&candidati, &freschezza);
    let mut scelti: Vec<Scelto> = ordinati
        .iter()
        .take(quanti)
        .map(|(slug, punteggio)| Scelto {
            slug: slug.clone(),
            punteggio: *punteggio,
            via: if esatti.contains_key(slug) { Via::Esatto } else { Via::Fusione },
        })
        .collect();

    // 4. un salto di grafo sui migliori, **a turno**.
    let massimo_grafo = std::cmp::max(2, quanti / 2);
    if espandi_grafo && !scelti.is_empty() {
        let mut gia: BTreeSet<String> = scelti.iter().map(|s| s.slug.clone()).collect();
        let sorgenti: Vec<Scelto> = scelti.iter().take(3).cloned().collect();
        let mut code: Vec<Vec<String>> =
            sorgenti.iter().map(|s| vicini(&s.slug)).collect();
        let mut aggiunti = 0usize;
        // A turno, un vicino per sorgente: prima il primo prendeva tutta la
        // quota e il secondo e il terzo non contribuivano mai.
        while aggiunti < massimo_grafo && code.iter().any(|c| !c.is_empty()) {
            let mut progresso = false;
            for (i, coda) in code.iter_mut().enumerate() {
                if aggiunti >= massimo_grafo {
                    break;
                }
                while !coda.is_empty() {
                    let vicino = coda.remove(0);
                    let Some(n) = nodi.get(&vicino) else { continue };
                    if gia.contains(&vicino) || n.confidenza < confidenza_minima {
                        continue;
                    }
                    let punteggio =
                        sorgenti[i].punteggio.min(tetto_punteggio_grafo()) * SCONTO_GRAFO;
                    scelti.push(Scelto { slug: vicino.clone(), punteggio, via: Via::Grafo });
                    gia.insert(vicino);
                    aggiunti += 1;
                    progresso = true;
                    break;
                }
            }
            if !progresso {
                break;
            }
        }
        resoconto.espansi_da_grafo = aggiunti;
    }

    // 5. riordino finale: i vicini entrano in coda con un punteggio ridotto,
    // e senza riordinare l'ordine mostrato non rispecchiava i punteggi.
    let punteggi: BTreeMap<String, f64> =
        scelti.iter().map(|s| (s.slug.clone(), s.punteggio)).collect();
    let ordine = in_ordine(&punteggi, &freschezza);
    let posto: BTreeMap<&str, usize> = ordine
        .iter()
        .enumerate()
        .map(|(i, (slug, _))| (slug.as_str(), i))
        .collect();
    scelti.sort_by_key(|s| posto.get(s.slug.as_str()).copied().unwrap_or(usize::MAX));
    scelti.truncate(quanti + massimo_grafo);
    (scelti, resoconto)
}

#[cfg(test)]
mod prove {
    use super::*;

    fn nodo(slug: &str, titolo: &str, conf: f64) -> Candidato {
        Candidato {
            slug: slug.into(),
            titolo: titolo.into(),
            tipo: "fatto".into(),
            confidenza: conf,
            aggiornato: "2026-01-01".into(),
            ..Default::default()
        }
    }

    fn memoria() -> BTreeMap<String, Candidato> {
        let mut m = BTreeMap::new();
        for (s, t, c) in [
            ("anna", "Anna", 0.9),
            ("progetto-nova", "Progetto Nova", 0.9),
            ("appunto-vago", "Appunto vago", 0.1),
            ("indice", "Indice", 0.9),
        ] {
            m.insert(s.to_string(), nodo(s, t, c));
        }
        m.get_mut("indice").unwrap().tipo = "hub".into();
        m
    }

    fn niente_vicini(_: &str) -> Vec<String> {
        Vec::new()
    }

    #[test]
    fn chi_e_nominato_per_nome_arriva_primo() {
        let m = memoria();
        let mut sparsi = BTreeMap::new();
        sparsi.insert("progetto-nova".to_string(), 9.0);
        let (scelti, _) = scegli("anna", &m, &sparsi, &BTreeMap::new(),
                                 &niente_vicini, 5, 0.25, false);
        assert_eq!(scelti[0].slug, "anna");
        assert_eq!(scelti[0].via, Via::Esatto);
    }

    #[test]
    fn chi_e_nominato_per_nome_salta_il_filtro_di_confidenza() {
        // Chiedere «parlami di X» e non ricevere niente perche' X e' poco
        // confidente non e' un filtro: e' una bugia.
        let mut m = memoria();
        m.insert("timido".into(), nodo("timido", "Timido", 0.05));
        let (scelti, r) = scegli("timido", &m, &BTreeMap::new(), &BTreeMap::new(),
                                 &niente_vicini, 5, 0.25, false);
        assert_eq!(scelti[0].slug, "timido");
        assert!(!r.scartati_esempio.contains(&"timido".to_string()));
    }

    #[test]
    fn i_poco_confidenti_si_scartano_e_si_conta_quanti() {
        let m = memoria();
        let mut sparsi = BTreeMap::new();
        sparsi.insert("appunto-vago".to_string(), 5.0);
        sparsi.insert("anna".to_string(), 5.0);
        let (scelti, r) = scegli("qualcosa", &m, &sparsi, &BTreeMap::new(),
                                 &niente_vicini, 5, 0.25, false);
        assert!(!scelti.iter().any(|s| s.slug == "appunto-vago"));
        assert_eq!(r.scartati_confidenza, 1);
        assert_eq!(r.scartati_esempio, vec!["appunto-vago".to_string()]);
    }

    #[test]
    fn gli_hub_non_sono_ricordi() {
        let m = memoria();
        let mut sparsi = BTreeMap::new();
        sparsi.insert("indice".to_string(), 99.0);
        let (scelti, r) = scegli("qualcosa", &m, &sparsi, &BTreeMap::new(),
                                 &niente_vicini, 5, 0.25, false);
        assert!(!scelti.iter().any(|s| s.slug == "indice"));
        // E non si contano fra gli scartati per confidenza: sono un'altra
        // cosa, e mescolarli renderebbe quel numero inutile.
        assert_eq!(r.scartati_confidenza, 0);
    }

    #[test]
    fn il_grafo_prende_a_turno_da_ogni_sorgente() {
        // Il difetto che questa regola ripara: il primo risultato si prendeva
        // tutta la quota e il secondo e il terzo non contribuivano mai.
        let mut m = memoria();
        for s in ["v1a", "v1b", "v2a"] {
            m.insert(s.into(), nodo(s, s, 0.9));
        }
        let mut sparsi = BTreeMap::new();
        sparsi.insert("anna".to_string(), 9.0);
        sparsi.insert("progetto-nova".to_string(), 8.0);
        let vicini = |s: &str| match s {
            "anna" => vec!["v1a".to_string(), "v1b".to_string()],
            "progetto-nova" => vec!["v2a".to_string()],
            _ => Vec::new(),
        };
        let (scelti, r) = scegli("qualcosa", &m, &sparsi, &BTreeMap::new(),
                                 &vicini, 4, 0.25, true);
        let da_grafo: Vec<&str> = scelti.iter()
            .filter(|s| s.via == Via::Grafo)
            .map(|s| s.slug.as_str())
            .collect();
        assert_eq!(r.espansi_da_grafo, 2);
        assert!(da_grafo.contains(&"v1a"), "il primo vicino della prima sorgente");
        assert!(da_grafo.contains(&"v2a"), "e uno della seconda, non due della prima");
    }

    #[test]
    fn un_vicino_non_vale_quanto_una_richiesta_esplicita() {
        // Senza il tetto, il vicino di un nodo nominato per nome valeva 350 e
        // spingeva fuori tutto quello che la ricerca aveva trovato.
        let mut m = memoria();
        m.insert("vicino".into(), nodo("vicino", "Vicino", 0.9));
        let vicini = |s: &str| if s == "anna" { vec!["vicino".to_string()] } else { Vec::new() };
        let (scelti, _) = scegli("anna", &m, &BTreeMap::new(), &BTreeMap::new(),
                                 &vicini, 4, 0.25, true);
        let v = scelti.iter().find(|s| s.slug == "vicino").unwrap();
        assert!(v.punteggio < 1.0, "il vicino vale {}, fuori scala", v.punteggio);
        assert_eq!(scelti[0].slug, "anna");
    }

    #[test]
    fn una_memoria_vuota_non_e_un_errore() {
        let (scelti, r) = scegli("qualsiasi", &BTreeMap::new(), &BTreeMap::new(),
                                 &BTreeMap::new(), &niente_vicini, 5, 0.25, true);
        assert!(scelti.is_empty());
        assert_eq!(r.nodi_in_memoria, 0);
    }

    #[test]
    fn lo_slug_toglie_gli_accenti_come_il_python() {
        assert_eq!(come_slug("Città"), "citta");
        assert_eq!(come_slug("Progetto Nova"), "progetto-nova");
        assert_eq!(come_slug("  già!  "), "gia");
        assert_eq!(come_slug("???"), "nodo");
        assert_eq!(come_slug(""), "nodo");
    }
}
