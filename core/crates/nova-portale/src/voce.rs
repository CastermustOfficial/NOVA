//! La voce nel canale (D455): il microfono del dispositivo che arriva al PC.
//!
//! Deciso con Gio il 28 novembre: il tablet e' una cosa a cui si parla, e
//! ascoltano tutti e due, il PC e il tablet. Riconoscere la frase di
//! risveglio e' un lavoro del PC (il tablet non ce la fa), quindi il
//! dispositivo manda il suo microfono cosi' com'e': pezzi da un decimo di
//! secondo, mono, a 16 kHz, a 16 bit. Li manda sempre, anche nel silenzio:
//! cosi' il PC ci applica la stessa regola che usa sul suo microfono (il
//! fondo della stanza misurato all'inizio, la soglia sopra il fondo), che e'
//! quella collaudata. Mandare solo quando c'e' suono vorrebbe una regola
//! seconda, sul dispositivo, proprio nel punto dove si rischia di perdere la
//! parola di risveglio: resta un'idea.
//!
//! Un pezzo viaggia come notifica, senza risposta: `voce/pezzo`, coi campioni
//! in base64. Sono 3200 byte ogni decimo di secondo, poco piu' di 4 kB in
//! base64: 256 kbit al secondo di campioni, circa 345 sul filo. Niente si
//! salva: il PC butta tutto quello che non contiene la frase di risveglio,
//! come fa col suo microfono.
//!
//! Qui ci sono la forma del pezzo, che i due capi devono leggere uguale, e
//! [`Pezzi`]: come il microfono del dispositivo diventa pezzi.

use base64::Engine;
use serde_json::{json, Value};

/// Il metodo con cui il dispositivo manda un pezzo del suo microfono.
pub const METODO_DEL_PEZZO: &str = "voce/pezzo";

/// La frequenza dei campioni che viaggiano: quella che vogliono sia chi
/// trascrive sia la conversazione dal vivo.
pub const FREQUENZA_DEL_MICROFONO: u32 = 16_000;

/// Quanti millisecondi di microfono in un pezzo.
pub const MS_DI_UN_PEZZO: u32 = 100;

/// Il pezzo piu' lungo che il PC accetta: due secondi. Un dispositivo che ne
/// manda di piu' lunghi non e' il nostro programma.
pub const CAMPIONI_IN_UN_PEZZO_AL_PIU: usize = FREQUENZA_DEL_MICROFONO as usize * 2;

/// I parametri di `voce/pezzo` per questi campioni.
pub fn pezzo(campioni: &[i16]) -> Value {
    let mut byte = Vec::with_capacity(campioni.len() * 2);
    for c in campioni {
        byte.extend_from_slice(&c.to_le_bytes());
    }
    json!({ "pcm": base64::engine::general_purpose::STANDARD.encode(byte) })
}

/// I campioni di un `voce/pezzo`, o perche' non si leggono.
pub fn leggi_il_pezzo(params: &Value) -> Result<Vec<i16>, String> {
    let testo = params
        .get("pcm")
        .and_then(Value::as_str)
        .ok_or("nel pezzo mancano i campioni")?;
    // Prima della decodifica: quattro caratteri ogni tre byte.
    if testo.len() > CAMPIONI_IN_UN_PEZZO_AL_PIU * 2 / 3 * 4 + 4 {
        return Err("il pezzo e' troppo lungo".into());
    }
    let byte = base64::engine::general_purpose::STANDARD
        .decode(testo)
        .map_err(|_| "i campioni del pezzo non si leggono")?;
    if byte.len() % 2 != 0 {
        return Err("i campioni del pezzo sono spezzati a meta'".into());
    }
    Ok(byte
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]))
        .collect())
}

/// Dal microfono del dispositivo ai pezzi che viaggiano.
///
/// Il microfono consegna quello che vuole lui: 48 kHz o 44,1, uno o due
/// canali, a blocchi di lunghezza qualunque. Qui diventano pezzi da
/// [`MS_DI_UN_PEZZO`] a [`FREQUENZA_DEL_MICROFONO`], mono: i canali si
/// mediano (per capire una voce un microfono stereo non serve) e la
/// frequenza si porta con un'interpolazione lineare che non ricomincia a
/// ogni blocco, se no a ogni giuntura ci sarebbe uno scatto.
pub struct Pezzi {
    canali: usize,
    /// Quanti campioni della sorgente per ogni campione che esce.
    passo: f64,
    /// I campioni mono della sorgente non ancora consumati.
    mono: Vec<f32>,
    /// Un blocco finito a meta' di un quadro: i canali che aspettano gli altri.
    a_meta: Vec<f32>,
    /// Dove cade il prossimo campione che esce, dentro `mono`.
    posizione: f64,
    /// I campioni usciti che non fanno ancora un pezzo.
    pronti: Vec<i16>,
}

impl Pezzi {
    /// Per un microfono a `frequenza` hertz e `canali` canali alternati.
    pub fn nuovo(frequenza: u32, canali: usize) -> Pezzi {
        Pezzi {
            canali: canali.max(1),
            passo: frequenza.max(1) as f64 / FREQUENZA_DEL_MICROFONO as f64,
            mono: Vec::new(),
            a_meta: Vec::new(),
            posizione: 0.0,
            pronti: Vec::new(),
        }
    }

    /// Un blocco del microfono, in virgola fra -1 e 1: tornano i pezzi che
    /// con questo blocco si sono riempiti, anche nessuno.
    pub fn versa(&mut self, blocco: &[f32]) -> Vec<Vec<i16>> {
        self.a_meta.extend_from_slice(blocco);
        let interi = self.a_meta.len() / self.canali * self.canali;
        for quadro in self.a_meta[..interi].chunks_exact(self.canali) {
            self.mono
                .push(quadro.iter().sum::<f32>() / self.canali as f32);
        }
        self.a_meta.drain(..interi);

        // Ogni campione che esce sta fra due della sorgente: servono tutti e
        // due, quindi l'ultimo della sorgente aspetta il blocco dopo.
        while (self.posizione.floor() as usize) + 1 < self.mono.len() {
            let sotto = self.posizione.floor() as usize;
            let frazione = (self.posizione - sotto as f64) as f32;
            let c = self.mono[sotto] * (1.0 - frazione) + self.mono[sotto + 1] * frazione;
            self.pronti
                .push((c.clamp(-1.0, 1.0) * 32767.0).round() as i16);
            self.posizione += self.passo;
        }
        let consumati = (self.posizione.floor() as usize).min(self.mono.len());
        self.mono.drain(..consumati);
        self.posizione -= consumati as f64;

        let lungo = (FREQUENZA_DEL_MICROFONO * MS_DI_UN_PEZZO / 1000) as usize;
        let mut fuori = Vec::new();
        while self.pronti.len() >= lungo {
            fuori.push(self.pronti.drain(..lungo).collect());
        }
        fuori
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    /// Un tono a `hz` hertz, a `frequenza`, su `canali` canali uguali.
    fn tono(hz: f32, frequenza: u32, canali: usize, campioni: usize) -> Vec<f32> {
        let mut v = Vec::with_capacity(campioni * canali);
        for n in 0..campioni {
            let c = (n as f32 * hz * std::f32::consts::TAU / frequenza as f32).sin() * 0.5;
            for _ in 0..canali {
                v.push(c);
            }
        }
        v
    }

    /// Quante volte il segnale passa da sotto a sopra lo zero.
    fn salite(pezzi: &[Vec<i16>]) -> usize {
        let tutti: Vec<i16> = pezzi.concat();
        tutti.windows(2).filter(|w| w[0] < 0 && w[1] >= 0).count()
    }

    #[test]
    fn un_secondo_di_microfono_fa_dieci_pezzi_qualunque_sia_il_microfono() {
        for (frequenza, canali) in [(48_000u32, 2usize), (44_100, 1), (16_000, 1), (8_000, 1)] {
            let mut p = Pezzi::nuovo(frequenza, canali);
            let pezzi = p.versa(&tono(
                200.0,
                frequenza,
                canali,
                frequenza as usize + frequenza as usize / 10,
            ));
            assert!(
                pezzi.len() >= 10,
                "{frequenza} Hz, {canali} canali: {} pezzi",
                pezzi.len()
            );
            assert!(pezzi.iter().all(|x| x.len() == 1600));
            // Il tono e' rimasto un tono a 200 Hz: in un secondo sale 200 volte.
            let salite = salite(&pezzi[..10]);
            assert!(
                (199..=201).contains(&salite),
                "{frequenza} Hz: {salite} salite"
            );
            // E forte com'era: meta' scala.
            let picco = pezzi
                .concat()
                .iter()
                .map(|c| c.unsigned_abs())
                .max()
                .unwrap();
            assert!(
                (15_500..=16_500).contains(&picco),
                "{frequenza} Hz: picco {picco}"
            );
        }
    }

    #[test]
    fn come_arrivano_i_blocchi_non_cambia_quello_che_esce() {
        let sorgente = tono(310.0, 48_000, 2, 48_000);
        let mut intero = Pezzi::nuovo(48_000, 2);
        let tutto_insieme = intero.versa(&sorgente);
        // A blocchi di lunghezze strane, anche dispari: un blocco puo'
        // finire a meta' di un quadro stereo.
        let mut a_pezzi = Pezzi::nuovo(48_000, 2);
        let mut un_po_alla_volta = Vec::new();
        let mut resto = &sorgente[..];
        for lungo in [1usize, 7, 480, 959, 4096, 3, 20_001].iter().cycle() {
            if resto.is_empty() {
                break;
            }
            let n = (*lungo).min(resto.len());
            un_po_alla_volta.extend(a_pezzi.versa(&resto[..n]));
            resto = &resto[n..];
        }
        assert_eq!(tutto_insieme.len(), 10);
        assert_eq!(un_po_alla_volta, tutto_insieme);
    }

    #[test]
    fn i_canali_si_mediano_e_il_troppo_forte_non_fa_il_giro() {
        // Sinistra a +1, destra a -1: la media e' silenzio.
        let mut p = Pezzi::nuovo(16_000, 2);
        let opposti: Vec<f32> = (0..3200 + 2).flat_map(|_| [1.0f32, -1.0]).collect();
        let pezzi = p.versa(&opposti);
        assert_eq!(pezzi.len(), 2);
        assert!(pezzi.concat().iter().all(|c| *c == 0));
        // Fuori scala: si ferma al massimo, non diventa negativo.
        let mut p = Pezzi::nuovo(16_000, 1);
        let pezzi = p.versa(&vec![3.5f32; 1602]);
        assert!(pezzi[0].iter().all(|c| *c == i16::MAX));
    }

    #[test]
    fn un_pezzo_torna_com_era() {
        let campioni: Vec<i16> = vec![0, 1, -1, 12_345, -12_345, i16::MAX, i16::MIN];
        assert_eq!(leggi_il_pezzo(&pezzo(&campioni)).unwrap(), campioni);
        assert_eq!(leggi_il_pezzo(&pezzo(&[])).unwrap(), Vec::<i16>::new());
    }

    #[test]
    fn un_decimo_di_secondo_sono_1600_campioni() {
        let quanti = (FREQUENZA_DEL_MICROFONO * MS_DI_UN_PEZZO / 1000) as usize;
        assert_eq!(quanti, 1600);
        let p = pezzo(&vec![7i16; quanti]);
        assert_eq!(leggi_il_pezzo(&p).unwrap().len(), quanti);
        // Poco piu' di 4 kB di testo per pezzo.
        assert_eq!(p["pcm"].as_str().unwrap().len(), 4268);
    }

    #[test]
    fn quello_che_non_e_un_pezzo_si_rifiuta() {
        assert!(leggi_il_pezzo(&json!({})).is_err());
        assert!(leggi_il_pezzo(&json!({ "pcm": 3 })).is_err());
        assert!(leggi_il_pezzo(&json!({ "pcm": "non e' base64!" })).is_err());
        // Tre byte: un campione e mezzo.
        assert!(leggi_il_pezzo(&json!({ "pcm": "AAAA" })).is_err());
    }

    #[test]
    fn un_pezzo_troppo_lungo_si_rifiuta_prima_di_leggerlo() {
        let al_limite = vec![1i16; CAMPIONI_IN_UN_PEZZO_AL_PIU];
        assert_eq!(
            leggi_il_pezzo(&pezzo(&al_limite)).unwrap().len(),
            CAMPIONI_IN_UN_PEZZO_AL_PIU
        );
        let troppo = vec![1i16; CAMPIONI_IN_UN_PEZZO_AL_PIU + 16];
        assert_eq!(
            leggi_il_pezzo(&pezzo(&troppo)).unwrap_err(),
            "il pezzo e' troppo lungo"
        );
    }
}
