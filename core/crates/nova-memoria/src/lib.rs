//! La meta' sparsa del recupero dalla memoria: BM25 e fusione dei ranking.
//!
//! Secondo pezzo portato dal Python, e il primo che incontra il disco - o
//! meglio, che lo incontrerebbe: qui dentro il disco non c'e', perche' i
//! nodi arrivano gia' letti. E' una scelta, non una dimenticanza: la parte
//! che legge il vault e' la parte che parla con Windows, e va scritta contro
//! il trait di `nova-platform` quando ci si arriva. Il punteggio no: quello
//! e' aritmetica, e si porta subito.
//!
//! **Cosa non c'e'.** L'embedder locale, che assegna i secchielli con MD5:
//! riprodurlo vorrebbe dire portarsi dietro un'implementazione di MD5 per
//! ottenere gli stessi identici secchielli, e non e' il momento. Qui c'e' la
//! meta' sparsa, che e' quella che pesa di piu' - l'embedder predefinito e'
//! a hash e non sa che «guarda se ho posta» e «controlla le mail» sono la
//! stessa cosa.
//!
//! Come per le ricette: costanti e formule identiche al Python, e un banco
//! (`src/banco.rs`) che pretende gli stessi punteggi sulle stesse domande.

use std::collections::{HashMap, HashSet};

/// Quanto conta la frequenza di un termine prima di saturare.
pub const K1: f64 = 1.5;
/// Quanto si corregge per la lunghezza del documento.
pub const B: f64 = 0.75;
/// La costante della fusione. Grande apposta: quello che conta non e' il
/// punteggio ma il divario fra due posizioni vicine, e con k=60 quel divario
/// e' piccolo abbastanza da non far vincere un indizio su una prova.
pub const RRF_K: usize = 60;

/// Parole che non distinguono niente.
pub const STOPWORDS: &[&str] = &[
    "il", "lo", "la", "i", "gli", "le", "un", "uno", "una", "di", "a", "da",
    "in", "con", "su", "per", "tra", "fra", "e", "o", "ma", "che", "chi", "cui",
    "non", "come", "dove", "quando", "quale", "quali", "del", "della", "dei",
    "delle", "degli", "al", "alla", "ai", "alle", "nel", "nella", "sul", "sulla",
    "mi", "ti", "si", "ci", "vi", "ho", "hai", "ha", "sono", "sei", "e'", "il",
    "the", "of", "to", "and", "is", "in", "it", "for",
];

/// Un nodo della memoria, ridotto a cio' che serve per pesarlo.
#[derive(Debug, Clone, Default)]
pub struct Nodo {
    pub slug: String,
    pub titolo: String,
    pub tag: Vec<String>,
    pub corpo: String,
}

/// Le parole che l'indice conta.
///
/// Il criterio e' quello del Python: lettere ASCII, le accentate che
/// compaiono in italiano, cifre e trattino basso. Tutto il resto separa.
pub fn tokenizza(testo: &str) -> Vec<String> {
    fn dentro(c: char) -> bool {
        c.is_ascii_alphanumeric()
            || c == '_'
            || matches!(c, 'à' | 'è' | 'é' | 'ì' | 'ò' | 'ó' | 'ù' | 'ç'
                         | 'À' | 'È' | 'É' | 'Ì' | 'Ò' | 'Ó' | 'Ù' | 'Ç')
    }
    let ferme: HashSet<&str> = STOPWORDS.iter().copied().collect();
    let mut fuori = Vec::new();
    let mut corrente = String::new();
    for c in testo.chars() {
        if dentro(c) {
            corrente.push(c);
        } else if !corrente.is_empty() {
            fuori.push(std::mem::take(&mut corrente));
        }
    }
    if !corrente.is_empty() {
        fuori.push(corrente);
    }
    fuori
        .into_iter()
        .map(|t| t.to_lowercase())
        .filter(|t| !ferme.contains(t.as_str()))
        .collect()
}

/// Titolo x2.5, tag x2.0, poi il corpo.
///
/// Lo spazio in coda a ogni ripetizione serve: senza, l'ultimo tag della
/// prima copia si fonde col primo della seconda in un token inesistente, e
/// il peso doppio non viene applicato a nessuno dei due.
pub fn testo_pesato(n: &Nodo) -> String {
    format!("{} {} {}",
            format!("{} ", n.titolo).repeat(2),
            format!("{} ", n.tag.join(" ")).repeat(2),
            n.corpo)
}

/// Indice invertito su cui si misura BM25.
#[derive(Debug, Default)]
pub struct Bm25 {
    pub k1: f64,
    pub b: f64,
    df: HashMap<String, usize>,
    freq: HashMap<String, HashMap<String, usize>>,
    lunghezze: HashMap<String, usize>,
    postings: HashMap<String, HashSet<String>>,
    lunghezza_media: f64,
}

impl Bm25 {
    pub fn nuovo() -> Self {
        Self { k1: K1, b: B, ..Default::default() }
    }

    /// Rifa' l'indice sui nodi dati. Qui, a differenza del Python, si
    /// ricostruisce tutto: il Python tiene una firma per nodo per non
    /// ritoccare cio' che non e' cambiato, ed e' un'ottimizzazione che ha
    /// senso quando l'indice vive fra una ricerca e l'altra. Qui l'indice
    /// nasce e muore con la chiamata, quindi la firma non avrebbe niente da
    /// confrontare - e i punteggi non dipendono da lei.
    pub fn indicizza(&mut self, nodi: &[Nodo]) {
        self.df.clear();
        self.freq.clear();
        self.lunghezze.clear();
        self.postings.clear();
        for n in nodi {
            let tok = tokenizza(&testo_pesato(n));
            let mut conteggi: HashMap<String, usize> = HashMap::new();
            for t in &tok {
                *conteggi.entry(t.clone()).or_insert(0) += 1;
            }
            self.lunghezze.insert(n.slug.clone(), tok.len());
            for t in conteggi.keys() {
                *self.df.entry(t.clone()).or_insert(0) += 1;
                self.postings.entry(t.clone()).or_default().insert(n.slug.clone());
            }
            self.freq.insert(n.slug.clone(), conteggi);
        }
        let totale: usize = self.lunghezze.values().sum();
        self.lunghezza_media = if self.lunghezze.is_empty() {
            0.0
        } else {
            totale as f64 / self.lunghezze.len() as f64
        };
    }

    /// I punteggi dei nodi che contengono almeno un termine della domanda.
    pub fn cerca(&self, query: &str) -> HashMap<String, f64> {
        let termini: HashSet<String> = tokenizza(query).into_iter().collect();
        let mut punteggi: HashMap<String, f64> = HashMap::new();
        if termini.is_empty() || self.freq.is_empty() {
            return punteggi;
        }
        let n_doc = self.freq.len() as f64;
        for t in &termini {
            let df = match self.df.get(t) {
                Some(d) if *d > 0 => *d as f64,
                _ => continue,
            };
            let idf = (1.0 + (n_doc - df + 0.5) / (df + 0.5)).ln();
            let Some(slug_con_t) = self.postings.get(t) else { continue };
            for slug in slug_con_t {
                let f = self.freq.get(slug).and_then(|c| c.get(t)).copied().unwrap_or(0);
                if f == 0 {
                    continue;
                }
                let f = f as f64;
                let lunghezza = self.lunghezze.get(slug).copied().unwrap_or(0) as f64;
                let media = if self.lunghezza_media == 0.0 { 1.0 } else { self.lunghezza_media };
                let norm = 1.0 - self.b + self.b * (lunghezza / media);
                *punteggi.entry(slug.clone()).or_insert(0.0) +=
                    idf * (f * (self.k1 + 1.0)) / (f + self.k1 * norm);
            }
        }
        punteggi.retain(|_, v| *v > 0.0);
        punteggi
    }
}

/// Reciprocal Rank Fusion: unisce ranking eterogenei senza normalizzarli.
///
/// L'ordinamento dentro ogni ranking deve essere quello del Python, che usa
/// `sorted(..., reverse=True)` su `dict.items()`: stabile, quindi a parita'
/// di punteggio vince chi e' stato inserito prima. Qui i punteggi arrivano
/// gia' in ordine di inserimento, e si ordina in modo stabile.
pub fn rrf(ranking: &[Vec<(String, f64)>], k: usize) -> HashMap<String, f64> {
    let mut fusi: HashMap<String, f64> = HashMap::new();
    for punteggi in ranking {
        let mut ordinati: Vec<&(String, f64)> = punteggi.iter().collect();
        ordinati.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        for (posizione, (slug, _)) in ordinati.iter().enumerate() {
            *fusi.entry(slug.clone()).or_insert(0.0) += 1.0 / (k + posizione + 1) as f64;
        }
    }
    fusi
}

/// Coseno fra due vettori gia' normalizzati: il prodotto scalare.
pub fn coseno(a: &[f64], b: &[f64]) -> f64 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod prove {
    use super::*;

    fn nodo(slug: &str, titolo: &str, tag: &[&str], corpo: &str) -> Nodo {
        Nodo {
            slug: slug.into(),
            titolo: titolo.into(),
            tag: tag.iter().map(|s| s.to_string()).collect(),
            corpo: corpo.into(),
        }
    }

    #[test]
    fn le_parole_ferme_non_entrano() {
        assert_eq!(tokenizza("il gatto e il cane"), vec!["gatto", "cane"]);
        // «è» resta: nell'elenco delle parole ferme c'e' `e'` con
        // l'apostrofo, che e' un'altra stringa. Non e' un errore di
        // traduzione - il Python fa lo stesso - ma e' una cosa che il
        // porting ha fatto vedere: chi scrive «è» invece di «e'» si porta in
        // memoria un termine che non distingue niente.
        assert_eq!(tokenizza("Però l'ora è tarda"),
                   vec!["però", "l", "ora", "è", "tarda"]);
    }

    #[test]
    fn i_tag_non_si_fondono_fra_una_copia_e_l_altra() {
        // Senza lo spazio in coda, «posta»+«mail» diventava «postamail»: un
        // token che non esiste, e il peso doppio non lo prendeva nessuno.
        let n = nodo("x", "T", &["posta", "mail"], "corpo");
        let tok = tokenizza(&testo_pesato(&n));
        assert!(tok.iter().all(|t| t != "postamail"), "{tok:?}");
        assert_eq!(tok.iter().filter(|t| *t == "posta").count(), 2);
    }

    #[test]
    fn il_titolo_pesa_piu_del_corpo() {
        let mut i = Bm25::nuovo();
        i.indicizza(&[
            nodo("a", "carbonara", &[], "una pagina che parla d'altro"),
            nodo("b", "altro", &[], "carbonara"),
        ]);
        let p = i.cerca("carbonara");
        assert!(p["a"] > p["b"], "{p:?}");
    }

    #[test]
    fn la_fusione_premia_la_posizione_non_il_punteggio() {
        // Un ranking con un punteggio enorme non deve travolgere l'altro.
        let a = vec![("x".to_string(), 1000.0), ("y".to_string(), 1.0)];
        let b = vec![("y".to_string(), 0.9), ("x".to_string(), 0.8)];
        let f = rrf(&[a, b], RRF_K);
        assert!((f["x"] - f["y"]).abs() < 1e-9, "{f:?}");
    }
}
