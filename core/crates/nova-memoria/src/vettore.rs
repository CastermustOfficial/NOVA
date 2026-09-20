//! L'embedding di casa: nessun modello, nessuna rete, sempre lo stesso.
//!
//! E' il «hashing trick» del Python, portato riga per riga. Non capisce i
//! sinonimi come un modello vero — «guarda se ho posta» e «controlla le mail»
//! restano due cose diverse — ma e' deterministico, istantaneo e non fa mai
//! fallire l'avvio, ed e' il predefinito da sempre.
//!
//! **Perche' identico e non «equivalente».** Il vettore da solo non si legge:
//! si legge il suo coseno con gli altri, e quello decide **quali ricordi
//! entrano nel contesto**. Due embedding che ordinano in modo diverso sono
//! due NOVA che si ricordano cose diverse della stessa persona, e la
//! differenza non si vede mai — si vede solo una risposta un po' peggiore,
//! ogni tanto, senza nessuno che possa dire perche'.
//!
//! Per questo l'md5: non serve a niente di crittografico, serve a cadere
//! nella **stessa casella** del Python.

use md5::{Digest, Md5};

/// Quante caselle ha il vettore. Le stesse 384 del Python: cambiarle vuol
/// dire che nessun vettore scritto prima e' piu' confrontabile.
pub const DIMENSIONI: usize = 384;

/// Quanto pesa ciascuna forma della parola.
///
/// La parola intera conta uno; il suo prefisso di quattro lettere sei
/// decimi — e' cio' che fa somigliare «controlla» a «controllo»; la coppia
/// con la parola prima quattro decimi, che e' l'unico pezzo di contesto che
/// un hash puo' avere.
pub const GRADI: [f64; 3] = [1.0, 0.6, 0.4];

/// La casella in cui cade una chiave.
///
/// Il Python prende le prime otto cifre esadecimali dell'md5 e le legge come
/// un numero: sono i **primi quattro byte**, in ordine di scrittura.
fn casella(chiave: &str) -> usize {
    let mut h = Md5::new();
    h.update(chiave.as_bytes());
    let d = h.finalize();
    let primi = u32::from_be_bytes([d[0], d[1], d[2], d[3]]) as usize;
    primi % DIMENSIONI
}

/// I primi `quante` **caratteri** di una parola.
///
/// Caratteri e non byte: `t[:4]` di Python su «città» da' «citt», e su una
/// fetta di byte darebbe quattro byte, cioe' tre lettere e mezza.
fn prefisso(t: &str, quante: usize) -> String {
    t.chars().take(quante).collect()
}

/// Il vettore di un testo, gia' normalizzato.
pub fn vettore(testo: &str) -> Vec<f64> {
    let mut v = vec![0.0f64; DIMENSIONI];
    let tok = crate::tokenizza(testo);
    for (i, t) in tok.iter().enumerate() {
        let coppia = if i > 0 {
            format!("{}_{}", tok[i - 1], t)
        } else {
            t.clone()
        };
        let chiavi = [t.clone(), prefisso(t, 4), coppia];
        for (grado, chiave) in GRADI.iter().zip(chiavi.iter()) {
            v[casella(chiave)] += grado;
        }
    }
    // `norma or 1.0` del Python: un testo senza parole esce tutto zeri
    // invece di far dividere per zero.
    let norma = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    let norma = if norma == 0.0 { 1.0 } else { norma };
    v.iter().map(|x| x / norma).collect()
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn un_vettore_e_lungo_quanto_dice() {
        assert_eq!(vettore("una frase qualunque").len(), DIMENSIONI);
    }

    #[test]
    fn ed_e_normalizzato() {
        let v = vettore("controlla la posta");
        let lunghezza: f64 = v.iter().map(|x| x * x).sum::<f64>().sqrt();
        assert!((lunghezza - 1.0).abs() < 1e-9, "{lunghezza}");
    }

    #[test]
    fn un_testo_senza_parole_non_divide_per_zero() {
        let v = vettore("...");
        assert_eq!(v.len(), DIMENSIONI);
        assert!(v.iter().all(|x| *x == 0.0));
    }

    #[test]
    fn lo_stesso_testo_da_lo_stesso_vettore() {
        assert_eq!(vettore("che ore sono"), vettore("che ore sono"));
    }

    #[test]
    fn due_frasi_che_condividono_le_parole_si_somigliano_di_piu() {
        let a = vettore("controlla la posta di lavoro");
        let b = vettore("controlla la posta personale");
        let c = vettore("accendi le luci del salotto");
        assert!(
            crate::coseno(&a, &b) > crate::coseno(&a, &c),
            "{} vs {}",
            crate::coseno(&a, &b),
            crate::coseno(&a, &c)
        );
    }

    #[test]
    fn e_il_prefisso_avvicina_le_forme_della_stessa_parola() {
        // «controlla» e «controllo» non sono la stessa parola per l'indice,
        // e il prefisso di quattro lettere e' l'unica cosa che le avvicina.
        assert!(crate::coseno(&vettore("controlla"), &vettore("controllo")) > 0.0);
    }

    #[test]
    fn la_casella_e_quella_che_calcola_il_python() {
        // `int(hashlib.md5(b"posta").hexdigest()[:8], 16) % 384`, calcolato
        // col Python e scritto qui: e' l'unico modo di accorgersi se un
        // giorno questa meta' cadesse in un'altra casella.
        assert_eq!(casella("posta"), 0x70e5_1d48_usize % DIMENSIONI);
        assert_eq!(casella("posta"), 328);
    }
}
