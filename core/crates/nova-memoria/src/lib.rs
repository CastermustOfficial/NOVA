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

use std::collections::{BTreeMap, BTreeSet, HashSet};

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
///
/// Le mappe sono **ordinate** e non a dispersione, e non e' un dettaglio di
/// gusto. In Python erano `dict` e `set`: l'ordine di scorrimento dipendeva
/// dal seme dell'hash del processo, e siccome la fusione RRF trasforma la
/// posizione in punteggio e il taglio a `top_k` butta via l'ultimo, la stessa
/// domanda sulla stessa memoria restituiva ricordi diversi a riavvii diversi.
/// Misurato sul vault vero: 444 domande, tre in cui cambiava quale nodo
/// veniva ricordato. Qui l'ordine e' quello degli slug, sempre — anche la
/// somma dei contributi, che e' in virgola mobile e non e' associativa.
#[derive(Debug, Default)]
pub struct Bm25 {
    pub k1: f64,
    pub b: f64,
    df: BTreeMap<String, usize>,
    freq: BTreeMap<String, BTreeMap<String, usize>>,
    lunghezze: BTreeMap<String, usize>,
    postings: BTreeMap<String, BTreeSet<String>>,
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
            let mut conteggi: BTreeMap<String, usize> = BTreeMap::new();
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
    pub fn cerca(&self, query: &str) -> BTreeMap<String, f64> {
        let termini: BTreeSet<String> = tokenizza(query).into_iter().collect();
        let mut punteggi: BTreeMap<String, f64> = BTreeMap::new();
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

/// Punteggio decrescente, poi **freschezza**, poi slug.
///
/// E' l'unico posto dove si decide chi viene prima, e la stessa domanda
/// ricorre tre volte nel recupero. Prima lo spareggio era implicito — un
/// ordinamento stabile, cioe' «vince chi e' arrivato prima» — e riprodurre
/// fedelmente quel comportamento dal Python voleva dire riprodurre un
/// difetto: la', l'ordine di arrivo risaliva a due insiemi di stringhe e
/// quindi al seme dell'hash del processo; qui sarebbe risalito all'ordine in
/// cui il chiamante passa i ranking. In tutti e due i casi la risposta a «chi
/// viene prima?» era «dipende», e il taglio finale la trasformava in «questo
/// ricordo lo tengo, quest'altro no».
///
/// **A parita' esatta vince il nodo aggiornato piu' di recente.** I nodi
/// crescono per accodamento: uno toccato ieri e' quasi sempre piu' vivo di
/// uno fermo da mesi. La data e' `aggiornato` in forma `AAAA-MM-GG`, quindi
/// l'ordine alfabetico **e'** l'ordine cronologico, e la granularita' e' il
/// giorno: nello stesso giorno il pareggio resta e decide lo slug —
/// arbitrario, ma stabile e leggibile nell'audit.
pub fn in_ordine(
    punteggi: &BTreeMap<String, f64>,
    freschezza: &BTreeMap<String, String>,
) -> Vec<(String, f64)> {
    let mut voci: Vec<(String, f64)> = punteggi.iter().map(|(s, p)| (s.clone(), *p)).collect();
    let vuota = String::new();
    voci.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                let (fa, fb) = (
                    freschezza.get(&a.0).unwrap_or(&vuota),
                    freschezza.get(&b.0).unwrap_or(&vuota),
                );
                fb.cmp(fa)
            })
            .then_with(|| a.0.cmp(&b.0))
    });
    voci
}

/// Reciprocal Rank Fusion: unisce ranking eterogenei senza normalizzarli.
///
/// Lo spareggio va **qui dentro** e non solo sull'ordine finale: l'RRF
/// trasforma la posizione in punteggio, quindi due nodi a pari merito escono
/// di qui con punteggi gia' diversi, e un criterio applicato piu' a valle non
/// troverebbe piu' nessun pareggio da sciogliere.
pub fn rrf(
    ranking: &[Vec<(String, f64)>],
    k: usize,
    freschezza: &BTreeMap<String, String>,
) -> BTreeMap<String, f64> {
    let mut fusi: BTreeMap<String, f64> = BTreeMap::new();
    for punteggi in ranking {
        let come_mappa: BTreeMap<String, f64> = punteggi.iter().cloned().collect();
        for (posizione, (slug, _)) in in_ordine(&come_mappa, freschezza).iter().enumerate() {
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

/// I caratteri di corpo che un nodo puo' occupare nel contesto del modello.
pub const MAX_CORPO_NEL_CONTESTO: usize = 950;

/// Tiene l'inizio **e** la fine di un corpo lungo.
///
/// I nodi crescono per accodamento: tagliare solo la coda vuol dire tenere la
/// definizione originale e buttare via proprio i fatti piu' recenti, che sono
/// quasi sempre quelli che servono.
///
/// Il conto e' in **caratteri**, non in byte, perche' in Python `len()` conta
/// caratteri: su un corpo pieno di accenti un taglio a byte cadrebbe prima —
/// e potrebbe cadere in mezzo a una lettera.
pub fn testa_e_coda(corpo: &str, massimo: usize) -> String {
    let quanti = corpo.chars().count();
    if quanti <= massimo {
        return corpo.to_string();
    }
    let quanti_testa = (massimo as f64 * 0.6) as usize;
    let quanti_coda = massimo - quanti_testa;
    let testa: String = corpo.chars().take(quanti_testa).collect();
    let coda: String = corpo.chars().skip(quanti - quanti_coda).collect();
    format!("{}\n[...]\n{}", testa.trim_end(), coda.trim_start())
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
        let f = rrf(&[a, b], RRF_K, &BTreeMap::new());
        assert!((f["x"] - f["y"]).abs() < 1e-9, "{f:?}");
    }

    #[test]
    fn la_fusione_non_dipende_da_come_arrivano_i_ranking() {
        // Il difetto vero, in versione Rust: se lo spareggio fosse l'ordine di
        // inserimento, girare la lista d'ingresso cambierebbe la classifica.
        let avanti = vec![("a".to_string(), 1.0), ("b".to_string(), 1.0),
                          ("c".to_string(), 1.0)];
        let mut indietro = avanti.clone();
        indietro.reverse();
        assert_eq!(rrf(&[avanti], RRF_K, &BTreeMap::new()), rrf(&[indietro], RRF_K, &BTreeMap::new()));
    }

    #[test]
    fn a_parita_esatta_vince_lo_slug_e_non_il_caso() {
        let f = rrf(&[vec![("zeta".to_string(), 1.0), ("alfa".to_string(), 1.0)]], RRF_K,
                &BTreeMap::new());
        assert!(f["alfa"] > f["zeta"], "{f:?}");
    }

    #[test]
    fn i_punteggi_non_dipendono_dallordine_dei_nodi() {
        let n = |s: &str| Nodo {
            slug: s.into(),
            titolo: "progetto".into(),
            tag: vec![],
            corpo: "un progetto di prova".into(),
        };
        let avanti = [n("a"), n("b"), n("c")];
        let mut indietro = avanti.clone();
        indietro.reverse();
        let (mut x, mut y) = (Bm25::nuovo(), Bm25::nuovo());
        x.indicizza(&avanti);
        y.indicizza(&indietro);
        assert_eq!(x.cerca("progetto"), y.cerca("progetto"));
    }

    #[test]
    fn del_corpo_lungo_restano_la_testa_e_la_coda() {
        let corpo = format!("INIZIO{}FINE", "x".repeat(2000));
        let t = testa_e_coda(&corpo, 100);
        assert!(t.starts_with("INIZIO") && t.ends_with("FINE") && t.contains("[...]"), "{t}");
        assert_eq!(testa_e_coda("ciao", 950), "ciao");
    }

    #[test]
    fn il_taglio_conta_caratteri_e_non_byte() {
        // Con un conto in byte, 600 lettere accentate sarebbero 1200 e il
        // taglio cadrebbe a meta' di una lettera.
        let corpo = "è".repeat(600);
        let t = testa_e_coda(&corpo, 500);
        assert!(t.contains("[...]"));
        assert!(t.chars().all(|c| c == 'è' || "[.]\n".contains(c)), "{t}");
    }

    #[test]
    fn a_parita_esatta_vince_il_piu_fresco() {
        // Il criterio che conta davvero: fra due ricordi ugualmente
        // pertinenti, quello toccato piu' di recente.
        let mut date = BTreeMap::new();
        date.insert("alfa".to_string(), "2026-01-01".to_string());
        date.insert("zeta".to_string(), "2026-09-03".to_string());
        let pari = vec![("alfa".to_string(), 1.0), ("zeta".to_string(), 1.0)];
        let f = rrf(&[pari], RRF_K, &date);
        assert!(f["zeta"] > f["alfa"], "il vecchio ha battuto il fresco: {f:?}");
    }

    #[test]
    fn ma_la_freschezza_non_scavalca_il_punteggio() {
        // Un nodo aggiornato ieri e poco pertinente non deve passare davanti a
        // uno pertinente e vecchio: lo spareggio scioglie i pari merito, non
        // riscrive la classifica.
        let mut date = BTreeMap::new();
        date.insert("vecchio".to_string(), "2020-01-01".to_string());
        date.insert("fresco".to_string(), "2026-09-03".to_string());
        let r = vec![("vecchio".to_string(), 10.0), ("fresco".to_string(), 1.0)];
        let f = rrf(&[r], RRF_K, &date);
        assert!(f["vecchio"] > f["fresco"], "{f:?}");
    }

    #[test]
    fn senza_data_si_finisce_in_fondo_ma_in_ordine() {
        // Un nodo senza `aggiornato` non deve far saltare l'ordinamento: vale
        // come il piu' vecchio possibile, e fra due senza data decide lo slug.
        let mut date = BTreeMap::new();
        date.insert("con".to_string(), "2026-09-03".to_string());
        let pari: BTreeMap<String, f64> =
            [("con", 1.0), ("senza", 1.0), ("altro", 1.0)]
                .into_iter()
                .map(|(s, p)| (s.to_string(), p))
                .collect();
        let ordine: Vec<String> = in_ordine(&pari, &date).into_iter().map(|(s, _)| s).collect();
        assert_eq!(ordine, vec!["con", "altro", "senza"], "{ordine:?}");
    }
}
