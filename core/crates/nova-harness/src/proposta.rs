//! Una proposta di NOVA come si guarda nell'harness: il file com'e', il file
//! come sarebbe, e quante righe cambiano.
//!
//! La proposta la scrive lo strumento `harness_proponi` in un file suo,
//! `proposta-<impronta>.json`, accanto alle sessioni: un elenco di modifiche
//! a dei blocchi, ognuna con dentro cosa c'era quando e' stata proposta.
//! Qui la si trasforma nel testo intero che ne verrebbe fuori, con
//! [`crate::modifica::rifai`] — la **stessa** funzione che applica — cosi'
//! il confronto che si mostra e' quello che poi si scrive.
//!
//! Due cose che l'applicazione del Python non tiene e che qui si tengono,
//! perche' chi le perde cambia un file in punti che nessuno ha toccato:
//! gli a capo di Windows e l'ultima riga senza a capo.

use serde_json::Value;

use crate::modifica::{rifai, Azione, Marche, Pronta, Saltata};

/// Le modifiche scritte in una proposta, lette come [`Pronta`].
///
/// Una modifica che non si capisce — un'azione sconosciuta, un blocco
/// mancante — si salta: la proposta l'ha gia' controllata chi l'ha scritta,
/// e una voce illeggibile qui vuol dire un file toccato a mano. Il conto di
/// quelle saltate torna, perche' chi guarda deve saperlo.
pub fn modifiche(proposta: &Value) -> (Vec<Pronta>, usize) {
    let mut pronte = Vec::new();
    let mut illeggibili = 0;
    for m in proposta
        .get("modifiche")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
    {
        let testo_di = |k: &str| m.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let numero = |k: &str| m.get(k).and_then(Value::as_u64).map(|n| n as u32);
        let azione = Azione::da(&testo_di("azione"));
        let blocco = testo_di("blocco");
        match azione {
            Some(azione) if !blocco.is_empty() => pronte.push(Pronta {
                azione,
                blocco,
                testo: testo_di("testo"),
                prima: testo_di("prima"),
                righe: numero("righe"),
                pagina: numero("pagina"),
            }),
            _ => illeggibili += 1,
        }
    }
    (pronte, illeggibili)
}

/// Le righe di un testo, contate come le conta un editor: sui `\n`, senza
/// la riga vuota dopo l'ultimo a capo, e senza il `\r` di Windows in coda
/// (lo stesso conto di `harness.righe_di` in Python, D274).
pub fn righe(testo: &str) -> Vec<String> {
    if testo.is_empty() {
        return Vec::new();
    }
    let mut v: Vec<String> = testo
        .split('\n')
        .map(|r| r.strip_suffix('\r').unwrap_or(r).to_string())
        .collect();
    if v.last().is_some_and(String::is_empty) {
        v.pop();
    }
    v
}

/// Il testo come sarebbe con le modifiche, e quelle che non si possono fare.
///
/// Gli a capo restano quelli del file: se c'era un `\r\n`, le righe nuove
/// lo prendono. L'ultima riga resta com'era — con o senza a capo — se
/// nessuna modifica la tocca; se il file e' vuoto o perde l'ultima riga, si
/// chiude con un a capo come fa il Python.
pub fn proposto(originale: &str, pronte: &[Pronta]) -> (String, usize, Vec<Saltata>) {
    let prima = righe(originale);
    let (dopo, fatte, saltate) = rifai(&prima, pronte, &Marche::default());
    let a_capo = if originale.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut testo = dopo.join(a_capo);
    let chiuso = originale.is_empty() || originale.ends_with('\n');
    let ultima_uguale = !dopo.is_empty() && dopo.last() == prima.last();
    if chiuso || !ultima_uguale {
        testo.push_str(a_capo);
    }
    (testo, fatte, saltate)
}

/// Oltre questo prodotto di righe il confronto esatto costa troppo, e si
/// conta alla grossa: il conto serve a dire «+3 −1», non a disegnare niente.
pub const CONFRONTO_MAX: usize = 4_000_000;

/// Quante righe arrivano e quante se ne vanno, da un testo all'altro.
///
/// Si tolgono prima le righe uguali in testa e in coda, che in una proposta
/// sono quasi tutto il file; il resto si confronta con la sottosequenza
/// comune piu' lunga.
pub fn quante_cambiano(prima: &str, dopo: &str) -> (usize, usize) {
    let a = righe(prima);
    let b = righe(dopo);
    let testa = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let resto_a = &a[testa..];
    let resto_b = &b[testa..];
    let coda = resto_a
        .iter()
        .rev()
        .zip(resto_b.iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let a = &resto_a[..resto_a.len() - coda];
    let b = &resto_b[..resto_b.len() - coda];
    if a.len().saturating_mul(b.len()) > CONFRONTO_MAX {
        return (b.len(), a.len());
    }
    let mut riga = vec![0usize; b.len() + 1];
    for x in a {
        let mut diagonale = 0;
        for (j, y) in b.iter().enumerate() {
            let sopra = riga[j + 1];
            riga[j + 1] = if x == y {
                diagonale + 1
            } else {
                sopra.max(riga[j])
            };
            diagonale = sopra;
        }
    }
    let comuni = riga[b.len()];
    (b.len() - comuni, a.len() - comuni)
}

#[cfg(test)]
mod prove {
    use super::*;
    use serde_json::json;

    fn una(azione: &str, blocco: &str, testo: &str, prima: &str) -> Value {
        json!({"azione": azione, "blocco": blocco, "testo": testo, "prima": prima, "righe": 1})
    }

    #[test]
    fn le_modifiche_si_leggono_e_le_illeggibili_si_contano() {
        let p = json!({"modifiche": [
            una("sostituisci", "r1", "b2", "b"),
            {"azione": "vola", "blocco": "r0"},
            {"azione": "elimina"},
            {"azione": "Elimina", "blocco": "r2", "prima": "c", "righe": 1, "pagina": 3},
        ]});
        let (m, male) = modifiche(&p);
        assert_eq!(male, 2);
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].azione, Azione::Sostituisci);
        assert_eq!(m[0].testo, "b2");
        assert_eq!(m[0].righe, Some(1));
        assert_eq!(m[1].azione, Azione::Elimina);
        assert_eq!(m[1].pagina, Some(3));
        assert_eq!(modifiche(&json!({})).0.len(), 0);
    }

    #[test]
    fn le_righe_si_contano_come_in_un_editor() {
        assert_eq!(righe(""), Vec::<String>::new());
        assert_eq!(righe("a\nb\n"), ["a", "b"]);
        assert_eq!(righe("a\r\nb"), ["a", "b"]);
        assert_eq!(righe("a\n\n"), ["a", ""]);
        assert_eq!(
            righe("a\x0cb\n"),
            ["a\x0cb"],
            "il salto pagina non e' un a capo"
        );
    }

    #[test]
    fn gli_a_capo_di_windows_restano() {
        let (m, _) = modifiche(&json!({"modifiche": [una("sostituisci", "r1", "B", "b")]}));
        let (t, fatte, saltate) = proposto("a\r\nb\r\nc\r\n", &m);
        assert_eq!(t, "a\r\nB\r\nc\r\n");
        assert_eq!((fatte, saltate.len()), (1, 0));
    }

    #[test]
    fn l_ultima_riga_senza_a_capo_resta_senza() {
        let (m, _) = modifiche(&json!({"modifiche": [una("sostituisci", "r0", "A", "a")]}));
        assert_eq!(proposto("a\nb", &m).0, "A\nb");
        assert_eq!(proposto("a\nb\n", &m).0, "A\nb\n");
        // Se si tocca proprio l'ultima, si chiude come il Python.
        let (m, _) = modifiche(&json!({"modifiche": [una("sostituisci", "r1", "B", "b")]}));
        assert_eq!(proposto("a\nb", &m).0, "a\nB\n");
    }

    #[test]
    fn un_file_cambiato_sotto_dice_perche() {
        let (m, _) = modifiche(&json!({"modifiche": [una("sostituisci", "r1", "B", "x")]}));
        let (t, fatte, saltate) = proposto("a\nb\n", &m);
        assert_eq!(t, "a\nb\n");
        assert_eq!(fatte, 0);
        assert!(saltate[0].perche.contains("cambiato"), "{:?}", saltate);
    }

    #[test]
    fn il_conto_delle_righe() {
        assert_eq!(quante_cambiano("a\nb\nc\n", "a\nb\nc\n"), (0, 0));
        assert_eq!(quante_cambiano("a\nb\nc\n", "a\nB\nc\n"), (1, 1));
        assert_eq!(quante_cambiano("a\nc\n", "a\nb\nc\n"), (1, 0));
        assert_eq!(quante_cambiano("a\nb\nc\n", "a\nc\n"), (0, 1));
        assert_eq!(quante_cambiano("", "x\ny\n"), (2, 0));
        assert_eq!(quante_cambiano("x\ny\nz\n", "y\nx\nz\n"), (1, 1));
        assert_eq!(quante_cambiano("a\r\nb\r\n", "a\nb\n"), (0, 0));
        // In mezzo, una riga in comune fra due cambiate.
        assert_eq!(
            quante_cambiano("1\n2\n3\n4\n5\n", "1\nX\n3\nY\n5\n"),
            (2, 2)
        );
        // Due righe comuni di fila in mezzo a righe cambiate.
        assert_eq!(quante_cambiano("x\na\nb\ny\n", "a\nb\nz\n"), (1, 2));
    }

    #[test]
    fn oltre_il_tetto_si_conta_alla_grossa_anche_se_le_righe_sono_le_stesse() {
        // Esatto farebbe (2, 2): ma sono 2101 righe per parte da confrontare,
        // e il conto grosso le da' tutte per cambiate.
        let comuni: String = (0..2099).map(|i| format!("c{i}\n")).collect();
        let a = format!("A\n{comuni}Z\n");
        let b = format!("B\n{comuni}Y\n");
        assert_eq!(quante_cambiano(&a, &b), (2101, 2101));
        let poche: String = (0..100).map(|i| format!("c{i}\n")).collect();
        assert_eq!(
            quante_cambiano(&format!("A\n{poche}Z\n"), &format!("B\n{poche}Y\n")),
            (2, 2)
        );
    }

    #[test]
    fn oltre_il_tetto_si_conta_alla_grossa() {
        let a: String = (0..2100).map(|i| format!("a{i}\n")).collect();
        let b: String = (0..2100).map(|i| format!("b{i}\n")).collect();
        assert_eq!(quante_cambiano(&a, &b), (2100, 2100));
    }
}
