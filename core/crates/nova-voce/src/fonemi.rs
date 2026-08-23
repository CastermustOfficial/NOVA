//! Testo -> fonemi, punteggiatura compresa.
//!
//! espeak da solo non basta: butta via la punteggiatura, e Kokoro la usa per
//! le pause e l'intonazione. La libreria Python di riferimento (`phonemizer`)
//! risolve cosi': stacca i segni prima di fonemizzare e li rimette dopo, al
//! posto giusto. Qui e' riportato lo stesso algoritmo — non per fedelta'
//! cieca, ma perche' il banco di prova in `tests/` confronta l'uscita con
//! quella del riferimento frase per frase: se divergono, il Rust ha torto.

use std::collections::HashMap;

use anyhow::Result;

use crate::espeak::Espeak;

/// I segni che vengono staccati e rimessi. Stesso insieme del riferimento.
const SEGNI: &[char] = &[
    ';', ':', ',', '.', '!', '?', '¡', '¿', '—', '…', '"', '«', '»', '“', '”',
    '(', ')', '{', '}', '[', ']',
];

/// Dove stava un segno rispetto al testo che lo circonda.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Posto {
    Inizio,
    Fine,
    Mezzo,
    Solo,
}

struct Segno {
    testo: String,
    posto: Posto,
}

fn e_segno(c: char) -> bool {
    SEGNI.contains(&c)
}

/// I gruppi di punteggiatura, spazi attorno inclusi: `(\s*[segni]+\s*)+`.
///
/// Gli spazi fanno parte del gruppo apposta: «con «standard» e» deve
/// ridiventare «con standard e» con uno spazio solo, non con tre.
fn gruppi(caratteri: &[char]) -> Vec<(usize, usize)> {
    let n = caratteri.len();
    let mut fuori = Vec::new();
    let mut i = 0;
    while i < n {
        let mut j = i;
        let mut preso = false;
        loop {
            let mut k = j;
            while k < n && caratteri[k].is_whitespace() {
                k += 1;
            }
            if k < n && e_segno(caratteri[k]) {
                while k < n && e_segno(caratteri[k]) {
                    k += 1;
                }
                while k < n && caratteri[k].is_whitespace() {
                    k += 1;
                }
                j = k;
                preso = true;
            } else {
                break;
            }
        }
        if preso {
            fuori.push((i, j));
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    fuori
}

/// Stacca la punteggiatura: ritorna i pezzi da fonemizzare e i segni tolti.
fn stacca(riga: &str) -> (Vec<String>, Vec<Segno>) {
    let caratteri: Vec<char> = riga.chars().collect();
    let trovati = gruppi(&caratteri);
    if trovati.is_empty() {
        return (vec![riga.to_string()], Vec::new());
    }
    // la riga e' fatta solo di segni
    if trovati.len() == 1 && trovati[0] == (0, caratteri.len()) {
        return (
            Vec::new(),
            vec![Segno { testo: riga.to_string(), posto: Posto::Solo }],
        );
    }

    let mut segni = Vec::new();
    for (indice, &(inizio, fine)) in trovati.iter().enumerate() {
        let testo: String = caratteri[inizio..fine].iter().collect();
        let posto = if indice == 0 && riga.starts_with(&testo) {
            Posto::Inizio
        } else if indice == trovati.len() - 1 && riga.ends_with(&testo) {
            Posto::Fine
        } else {
            Posto::Mezzo
        };
        segni.push(Segno { testo, posto });
    }

    let mut pezzi = Vec::new();
    let mut resto = riga.to_string();
    for segno in &segni {
        match resto.split_once(segno.testo.as_str()) {
            Some((prima, dopo)) => {
                pezzi.push(prima.to_string());
                resto = dopo.to_string();
            }
            None => break,
        }
    }
    pezzi.push(resto);
    // i pezzi vuoti spariscono, i segni restano: e' cosi' anche nel riferimento
    (pezzi.into_iter().filter(|p| !p.is_empty()).collect(), segni)
}

/// Rimette i segni fra i pezzi fonemizzati.
fn rimetti(mut pezzi: Vec<String>, mut segni: Vec<Segno>) -> String {
    const SEPARATORE: &str = " ";
    let mut fuori: Vec<String> = Vec::new();
    let mut posizione = 0usize;

    while !pezzi.is_empty() || !segni.is_empty() {
        if segni.is_empty() {
            for mut riga in pezzi.drain(..) {
                if !riga.ends_with(SEPARATORE) {
                    riga.push_str(SEPARATORE);
                }
                fuori.push(riga);
            }
            continue;
        }
        if pezzi.is_empty() {
            fuori.push(segni.iter().map(|s| s.testo.as_str()).collect::<String>());
            segni.clear();
            continue;
        }
        // tutti i segni di una frase sola hanno indice 0: si consumano in
        // ordine finche' non si chiude un pezzo
        if posizione == 0 {
            let segno = segni.remove(0);
            let marchio = segno.testo.clone();
            if pezzi[0].ends_with(SEPARATORE) {
                let taglio = pezzi[0].len() - SEPARATORE.len();
                pezzi[0].truncate(taglio);
            }
            match segno.posto {
                Posto::Inizio => {
                    pezzi[0] = format!("{marchio}{}", pezzi[0]);
                }
                Posto::Fine => {
                    let coda = if marchio.ends_with(SEPARATORE) { "" } else { SEPARATORE };
                    fuori.push(format!("{}{marchio}{coda}", pezzi.remove(0)));
                    posizione += 1;
                }
                Posto::Solo => {
                    let coda = if marchio.ends_with(SEPARATORE) { "" } else { SEPARATORE };
                    fuori.push(format!("{marchio}{coda}"));
                    posizione += 1;
                }
                Posto::Mezzo => {
                    if pezzi.len() == 1 {
                        pezzi[0].push_str(&marchio);
                    } else {
                        let primo = pezzi.remove(0);
                        pezzi[0] = format!("{primo}{marchio}{}", pezzi[0]);
                    }
                }
            }
        } else {
            fuori.push(pezzi.remove(0));
            posizione += 1;
        }
    }
    fuori.join("")
}

/// Il fonemizzatore completo: testo -> fonemi filtrati sul vocabolario.
pub struct Fonemizzatore {
    espeak: Espeak,
    vocabolario: HashMap<char, i64>,
}

impl Fonemizzatore {
    pub fn nuovo(espeak: Espeak, vocabolario: HashMap<char, i64>) -> Self {
        Self { espeak, vocabolario }
    }

    /// Testo -> fonemi IPA, solo i caratteri che il modello conosce.
    pub fn fonemi(&self, testo: &str, lingua: &str) -> Result<String> {
        let testo = testo.trim();
        if testo.is_empty() {
            return Ok(String::new());
        }
        let (pezzi, segni) = stacca(testo);
        let mut fonemizzati = Vec::with_capacity(pezzi.len());
        for pezzo in &pezzi {
            fonemizzati.push(self.espeak.fonemi(pezzo, lingua)?);
        }
        let unito = rimetti(fonemizzati, segni);
        Ok(unito
            .chars()
            .filter(|c| self.vocabolario.contains_key(c))
            .collect::<String>()
            .trim()
            .to_string())
    }

    /// Fonemi -> identificativi per il modello.
    pub fn token(&self, fonemi: &str) -> Vec<i64> {
        fonemi
            .chars()
            .filter_map(|c| self.vocabolario.get(&c).copied())
            .collect()
    }
}
