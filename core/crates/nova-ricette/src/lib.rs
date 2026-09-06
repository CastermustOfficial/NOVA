//! Le ricette: ritrovare una procedura gia' imparata, anche con un refuso.
//!
//! Questo e' il primo pezzo di NOVA portato dal Python al Rust, ed e' stato
//! scelto per quello che *non* ha: nessuna finestra, nessuna chiamata a
//! Windows, nessuna rete, nessuna dipendenza. E' aritmetica su stringhe, con
//! una prova che gia' esisteva. Serve a rispondere con un numero invece che
//! con una stima alla domanda «quanto costa portare il resto».
//!
//! **Il porting non e' un'occasione per migliorare.** Le soglie, i pesi e le
//! guardie sono quelli di `nova/ricette.py` fino all'ultima cifra, comprese
//! le stranezze che sembrano sbagliate e non lo sono - la rarita' senza
//! logaritmo, il contenimento asimmetrico, i bordi `«»` sui trigrammi.
//! Cambiare qualcosa qui mentre si traduce vorrebbe dire non sapere piu' se
//! una differenza fra le due versioni e' un errore di traduzione o un
//! miglioramento voluto, ed e' il modo in cui un porting diventa una
//! riscrittura che nessuno sa piu' confrontare.
//!
//! Il confronto e' `src/banco.rs`, che rilegge lo stesso archivio e le stesse
//! domande e pretende gli stessi punteggi.

use std::collections::{HashMap, HashSet};

/// Sotto questa, la procedura non si propone. Larga apposta: una candidata
/// di troppo costa qualche centinaio di token, una mancata costa i dieci
/// turni che ci vogliono a rifare la strada da capo.
pub const SOGLIA: f64 = 0.30;
/// Quanto devono somigliarsi due parole per valere l'una per l'altra.
pub const SOGLIA_PAROLA: f64 = 0.5;

/// Parole che non distinguono niente: ci sono in ogni richiesta.
pub const VUOTE: &[&str] = &[
    "il", "lo", "la", "i", "gli", "le", "un", "uno", "una", "di", "a", "da",
    "in", "con", "su", "per", "tra", "fra", "e", "ed", "o", "che", "chi",
    "cosa", "come", "quando", "dove", "mi", "ti", "ci", "vi", "si", "me",
    "te", "se", "non", "piu", "puoi", "puo", "vorrei", "voglio", "dammi",
    "fammi", "per favore", "grazie", "ok", "adesso", "ora", "poi", "anche",
    "del", "della", "dei", "delle", "dal", "dalla", "al", "alla", "ai",
    "sul", "sulla", "nel", "nella", "questo", "questa", "quello", "quella",
    "the", "an", "of", "to", "for", "my", "please", "can", "you",
];

/// Sopra questa, due procedure sono la stessa cosa scritta due volte.
///
/// Molto piu' alta di [`SOGLIA`], e per una ragione opposta: li' si sceglie
/// cosa **mostrare**, e una candidata di troppo costa qualche centinaio di
/// token; qui si sceglie cosa **buttare**, e una fusione sbagliata perde una
/// procedura per sempre.
pub const SOGLIA_FUSIONE: f64 = 0.75;

/// Una procedura in archivio, ridotta a cio' che serve per ritrovarla.
///
/// I campi oltre le parole servono alla fusione: chi assorbe chi si decide
/// su `usata` e `ultimo_uso`, e cio' che si tiene — titolo e passi — viene
/// dall'ultima riuscita.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Ricetta {
    pub parole: Vec<String>,
    pub parole_alias: Vec<String>,
    pub parole_passi: Vec<String>,
    pub usata: i64,
    pub ultimo_uso: f64,
    pub titolo: String,
    pub procedura: String,
    pub strumenti: Vec<String>,
}

/// Toglie gli accenti come fa `unicodedata.normalize("NFKD", ...)` seguito
/// dallo scarto dei segni combinanti, per le lettere che compaiono davvero
/// in italiano e nelle lingue vicine.
///
/// Non e' una NFKD completa: farla per intero vorrebbe dire portarsi dietro
/// le tabelle Unicode per un guadagno che qui non esiste, perche' subito
/// dopo si tiene solo `[a-z0-9]` e tutto il resto cade comunque. Quello che
/// deve valere e' che una lettera accentata diventi la sua base invece di
/// sparire: senza, «però» darebbe «per» e non «pero».
fn senza_accenti(c: char) -> Option<char> {
    let minuscola = c.to_lowercase().next().unwrap_or(c);
    Some(match minuscola {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        'ñ' => 'n',
        'ý' | 'ÿ' => 'y',
        altro => altro,
    })
}

/// Parole utili: senza accenti, senza punteggiatura, senza le vuote.
pub fn parole(testo: &str) -> Vec<String> {
    let vuote: HashSet<&str> = VUOTE.iter().copied().collect();
    let piatto: String = testo.chars().filter_map(senza_accenti).collect();
    let mut fuori = Vec::new();
    let mut corrente = String::new();
    for c in piatto.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
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
        .filter(|p| p.chars().count() > 2 && !vuote.contains(p.as_str()))
        .collect()
}

/// Quanto vale una parola: poco se sta in tutte le procedure.
///
/// Rarita' senza logaritmo, di proposito: con un archivio da sessanta voci
/// il logaritmo appiattisce proprio la differenza che serve.
pub fn rarita(elenco: &[Ricetta]) -> HashMap<String, f64> {
    let mut quante: HashMap<&str, usize> = HashMap::new();
    for r in elenco {
        let mut viste: HashSet<&str> = HashSet::new();
        for p in r.parole.iter().chain(r.parole_alias.iter()) {
            viste.insert(p.as_str());
        }
        for p in viste {
            *quante.entry(p).or_insert(0) += 1;
        }
    }
    let totale = elenco.len().max(1) as f64;
    quante
        .into_iter()
        .map(|(p, n)| (p.to_string(), 1.0 + totale / (1.0 + n as f64)))
        .collect()
}

/// I pezzi di tre lettere di una parola, con i bordi segnati.
///
/// I bordi contano: senza, «ore» dentro «lavore» e «ore» da sola darebbero
/// gli stessi pezzi, e l'inizio di una parola e' proprio cio' che la
/// distingue.
pub fn trigrammi(p: &str) -> HashSet<String> {
    let s: Vec<char> = format!("\u{ab}{p}\u{bb}").chars().collect();
    if s.len() < 3 {
        return HashSet::new();
    }
    (0..=s.len() - 3)
        .map(|i| s[i..i + 3].iter().collect::<String>())
        .collect()
}

/// Coefficiente di Dice: due volte l'intersezione sul totale.
pub fn dado(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    2.0 * a.intersection(b).count() as f64 / (a.len() + b.len()) as f64
}

/// Due parole che valgono l'una per l'altra, senza un vero analizzatore.
///
/// Copre i due casi che si presentano subito e che l'uguaglianza secca
/// sbaglia entrambi - «email»/«mail» e «silenzia»/«silenzioso» - e poi i
/// refusi, con due guardie imparate sbagliando: stessa lettera iniziale
/// (senza, «ricetta» valeva «letta») e lunghezze che non differiscono di
/// piu' di uno (senza, «stazione» valeva «situazione»).
pub fn stessa_parola(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    // Le lunghezze si contano in caratteri, non in byte: `parole()` lascia
    // passare solo ASCII, ma questa funzione e' pubblica e chi la chiama
    // domani potrebbe non saperlo.
    let (na, nb) = (a.chars().count(), b.chars().count());
    if na >= 4 && nb >= 4 && (a.contains(b) || b.contains(a)) {
        return true;
    }
    if na >= 6 && nb >= 6 && a.chars().take(6).eq(b.chars().take(6)) {
        return true;
    }
    if na < 5 || nb < 5 {
        return false;
    }
    if a.chars().next() != b.chars().next() {
        return false;
    }
    if na.abs_diff(nb) > 1 {
        return false;
    }
    dado(&trigrammi(a), &trigrammi(b)) >= SOGLIA_PAROLA
}

/// Quanto della domanda e' coperto da questa procedura.
///
/// Contenimento **asimmetrico**, non coseno: una procedura ricca di dettagli
/// non deve perdere contro una povera solo perche' ha piu' parole.
pub fn somiglianza(chieste: &[String], r: &Ricetta, peso: &HashMap<String, f64>) -> f64 {
    let a: HashSet<&String> = chieste.iter().collect();
    let b: HashSet<&String> = r.parole.iter().collect();
    let alias: HashSet<&String> = r.parole_alias.iter().filter(|x| !b.contains(*x)).collect();
    let deboli: HashSet<&String> = r
        .parole_passi
        .iter()
        .filter(|x| !b.contains(*x) && !alias.contains(*x))
        .collect();
    if a.is_empty() || (b.is_empty() && alias.is_empty() && deboli.is_empty()) {
        return 0.0;
    }
    let mut su = 0.0;
    for p in &a {
        let w = peso.get(p.as_str()).copied().unwrap_or(1.0);
        if b.iter().any(|q| stessa_parola(p, q)) {
            su += w;
        } else if alias.iter().any(|q| stessa_parola(p, q)) {
            su += w * 0.85;
        } else if deboli.iter().any(|q| stessa_parola(p, q)) {
            su += w * 0.5;
        }
    }
    let giu: f64 = a
        .iter()
        .map(|p| peso.get(p.as_str()).copied().unwrap_or(1.0))
        .sum();
    if giu == 0.0 {
        0.0
    } else {
        su / giu
    }
}

/// Le procedure che somigliano alla richiesta, dalla piu' vicina.
/// Ritorna gli indici nell'elenco, con il punteggio.
pub fn proponi(elenco: &[Ricetta], domanda: &str, quante: usize) -> Vec<(usize, f64)> {
    if elenco.is_empty() {
        return Vec::new();
    }
    let chieste = parole(domanda);
    if chieste.is_empty() {
        return Vec::new();
    }
    let peso = rarita(elenco);
    let mut punteggi: Vec<(usize, f64)> = elenco
        .iter()
        .enumerate()
        .map(|(i, r)| (i, somiglianza(&chieste, r, &peso)))
        .filter(|(_, s)| *s >= SOGLIA)
        .collect();
    // Stesso ordinamento del Python: punteggio, poi quante volte e' servita.
    // `sort_by` e' stabile come `list.sort`, quindi a parita' di entrambi
    // vince chi viene prima nell'archivio, in tutte e due le versioni.
    punteggi.sort_by(|x, y| {
        let a = (y.1, elenco[y.0].usata);
        let b = (x.1, elenco[x.0].usata);
        a.0.partial_cmp(&b.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.1.cmp(&b.1))
    });
    punteggi.truncate(quante);
    punteggi
}

// Il testo che le procedure mettono in bocca al modello: sta accanto al suo
// dato, perche' un testo lontano dal dato si aggiorna a meta' (D72).
pub mod blocco;

// Come si impara una procedura: cosa si chiede al modello, quando non vale la
// pena chiederglielo, e come si legge quello che risponde.
pub mod imparare;

/// Fonde le procedure che sono la stessa cosa scritta due volte.
///
/// Serve perche' il difetto si vede nei numeri: ventotto procedure
/// archiviate e solo quattro usate piu' di una volta, con «Controllo posta
/// Gmail» e «Controllo ultime email Gmail» che si dividono il contatore.
/// Divise, nessuna delle due arriva alle tre volte che fanno scattare il
/// suggerimento dell'automazione: il gradino successivo non si presenta mai.
///
/// Chi resta e' la piu' usata; a parita', la piu' recente. I passi tenuti
/// sono quelli dell'**ultima riuscita**: se la strada e' cambiata, quella
/// buona e' l'ultima.
///
/// L'ordine di assorbimento vive solo qui dentro. Fuori si restituisce
/// l'archivio nell'ordine in cui stava, e non e' una gentilezza: `unisci`
/// gira a ogni registrazione, e un elenco che si rimescola da solo cambia in
/// silenzio anche **chi viene buttato** quando si supera il tetto, che taglia
/// in coda.
pub fn unisci(elenco: &[Ricetta], soglia: f64) -> Vec<Ricetta> {
    if elenco.len() < 2 {
        return elenco.to_vec();
    }
    let peso = rarita(elenco);

    // Si ordina per (usata, ultimo_uso) decrescente. `sort_by` e' stabile
    // come `list.sort` di Python, quindi a parita' di entrambi vince chi
    // veniva prima nell'archivio — di qua e di la'.
    let mut ordine: Vec<usize> = (0..elenco.len()).collect();
    ordine.sort_by(|x, y| {
        let a = (elenco[*y].usata, elenco[*y].ultimo_uso);
        let b = (elenco[*x].usata, elenco[*x].ultimo_uso);
        (a.0, a.1)
            .partial_cmp(&(b.0, b.1))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // `tenute` conserva la posizione originale accanto alla ricetta, cosi'
    // alla fine si rimette tutto dov'era.
    let mut tenute: Vec<(usize, Ricetta)> = Vec::new();
    for i in ordine {
        let r = &elenco[i];
        let gemella = tenute.iter().position(|(_, t)| {
            let d = somiglianza(&r.parole, t, &peso);
            let s = somiglianza(&t.parole, r, &peso);
            d.max(s) >= soglia
        });
        let Some(g) = gemella else {
            tenute.push((i, r.clone()));
            continue;
        };
        let t = &mut tenute[g].1;
        // I contatori si sommano: e' il punto di tutta la fusione.
        t.usata += r.usata;
        t.parole = unione(&t.parole, &r.parole);
        t.parole_passi = unione(&t.parole_passi, &r.parole_passi);
        t.parole_alias = unione(&t.parole_alias, &r.parole_alias);
        t.strumenti = unione(&t.strumenti, &r.strumenti);
        // Il testo, invece, non si somma: si tiene quello dell'ultima volta
        // che la strada ha funzionato.
        if r.ultimo_uso > t.ultimo_uso {
            t.procedura = r.procedura.clone();
            t.titolo = r.titolo.clone();
            t.ultimo_uso = r.ultimo_uso;
        }
    }
    tenute.sort_by_key(|(i, _)| *i);
    tenute.into_iter().map(|(_, r)| r).collect()
}

/// `sorted(set(a) | set(b))` di Python.
fn unione(a: &[String], b: &[String]) -> Vec<String> {
    let mut insieme: Vec<String> = a.iter().chain(b.iter()).cloned().collect();
    insieme.sort();
    insieme.dedup();
    insieme
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn le_parole_perdono_accenti_punteggiatura_e_vuote() {
        assert_eq!(parole("Guarda però la Posta!"), vec!["guarda", "pero", "posta"]);
        assert_eq!(parole("il e di a"), Vec::<String>::new());
        // Due lettere non bastano: «se», «tu», «pc» non distinguono.
        assert_eq!(parole("PC acceso"), vec!["acceso"]);
    }

    #[test]
    fn i_bordi_dei_trigrammi_contano() {
        assert!(trigrammi("ore").contains("\u{ab}or"));
        assert!(dado(&trigrammi("ore"), &trigrammi("lavore")) < 0.8);
    }

    #[test]
    fn le_guardie_imparate_sbagliando() {
        // Senza la guardia sull'iniziale, «ricetta» valeva «letta».
        assert!(!stessa_parola("ricetta", "letta"));
        // Senza quella sulla lunghezza, «stazione» valeva «situazione».
        assert!(!stessa_parola("stazione", "situazione"));
        // I refusi veri invece devono passare.
        assert!(stessa_parola("inbox", "inbo"));
        assert!(stessa_parola("email", "mail"));
        assert!(stessa_parola("silenzia", "silenzioso"));
    }

    #[test]
    fn la_rarita_non_ha_logaritmo() {
        let elenco = vec![
            Ricetta { parole: vec!["posta".into(), "fantacalcio".into()], ..Default::default() },
            Ricetta { parole: vec!["posta".into()], ..Default::default() },
        ];
        let r = rarita(&elenco);
        // «posta» sta in tutte e due, «fantacalcio» in una: 1+2/3 contro 1+2/2.
        assert!((r["posta"] - (1.0 + 2.0 / 3.0)).abs() < 1e-12);
        assert!((r["fantacalcio"] - 2.0).abs() < 1e-12);
    }

    fn con(parole: &[&str], usata: i64, ultimo: f64, titolo: &str) -> Ricetta {
        Ricetta {
            parole: parole.iter().map(|x| x.to_string()).collect(),
            usata,
            ultimo_uso: ultimo,
            titolo: titolo.into(),
            procedura: format!("passi di {titolo}"),
            ..Default::default()
        }
    }

    #[test]
    fn due_scritture_della_stessa_cosa_diventano_una() {
        // Il caso vero: «Controllo posta Gmail» e «Controllo ultime email
        // Gmail». Divise, nessuna delle due arriva alle tre volte che fanno
        // scattare il suggerimento dell'automazione.
        let a = con(&["controllo", "posta", "gmail"], 2, 100.0, "Controllo posta Gmail");
        let b = con(&["controllo", "posta", "gmail"], 1, 200.0, "Controllo ultime email");
        let f = unisci(&[a, b], SOGLIA_FUSIONE);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].usata, 3, "i contatori si sommano: e' il punto");
        // I passi tenuti sono quelli dell'ultima riuscita, non della piu' usata.
        assert_eq!(f[0].titolo, "Controllo ultime email");
        assert_eq!(f[0].ultimo_uso, 200.0);
    }

    #[test]
    fn due_cose_diverse_restano_due() {
        let a = con(&["controllo", "posta", "gmail"], 5, 100.0, "Posta");
        let b = con(&["ordina", "fatture", "cartella"], 5, 100.0, "Fatture");
        assert_eq!(unisci(&[a, b], SOGLIA_FUSIONE).len(), 2);
    }

    #[test]
    fn larchivio_torna_nellordine_in_cui_stava() {
        // Non e' una gentilezza: `unisci` gira a ogni registrazione, e chi
        // viene buttato quando si supera il tetto e' chi sta in coda. Un
        // elenco che si rimescola da solo cambia in silenzio anche quello.
        let a = con(&["alfa", "uno"], 1, 10.0, "A");
        let b = con(&["beta", "due"], 9, 90.0, "B");
        let c = con(&["gamma", "tre"], 5, 50.0, "C");
        let f = unisci(&[a, b, c], SOGLIA_FUSIONE);
        assert_eq!(f.iter().map(|r| r.titolo.as_str()).collect::<Vec<_>>(),
                   vec!["A", "B", "C"]);
    }

    #[test]
    fn assorbe_la_piu_usata_non_la_prima() {
        // Se assorbisse la prima dell'elenco, il titolo tenuto sarebbe quello
        // della meno usata e il conteggio finirebbe sotto la voce sbagliata.
        let debole = con(&["controllo", "posta", "gmail"], 1, 10.0, "Debole");
        let forte = con(&["controllo", "posta", "gmail"], 8, 20.0, "Forte");
        let f = unisci(&[debole, forte], SOGLIA_FUSIONE);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].titolo, "Forte");
        assert_eq!(f[0].usata, 9);
    }

    #[test]
    fn le_parole_si_sommano_ordinate_e_senza_doppioni() {
        let mut a = con(&["controllo", "posta", "gmail"], 2, 100.0, "A");
        a.strumenti = vec!["web_apri".into()];
        let mut b = con(&["controllo", "posta", "gmail"], 1, 50.0, "B");
        b.strumenti = vec!["web_leggi".into(), "web_apri".into()];
        b.parole.push("email".into());
        let f = unisci(&[a, b], SOGLIA_FUSIONE);
        assert_eq!(f[0].strumenti, vec!["web_apri", "web_leggi"]);
        assert_eq!(f[0].parole, vec!["controllo", "email", "gmail", "posta"]);
    }

    #[test]
    fn un_archivio_di_uno_o_di_zero_non_si_tocca() {
        assert!(unisci(&[], SOGLIA_FUSIONE).is_empty());
        let uno = vec![con(&["alfa"], 1, 1.0, "A")];
        assert_eq!(unisci(&uno, SOGLIA_FUSIONE), uno);
    }
}
