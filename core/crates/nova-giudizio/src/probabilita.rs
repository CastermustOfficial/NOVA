//! Dai logit alle probabilita', e da una distribuzione a un numero.

/// Da logit a probabilita', con una temperatura.
///
/// Il massimo si sottrae prima di esponenziare: senza, un logit grande manda
/// `exp` all'infinito e tutta la distribuzione diventa `NaN`. E' il difetto
/// classico di questa funzione, e costa una riga non averlo.
///
/// La temperatura non e' un abbellimento: le probabilita' lette cosi' sono
/// **sistematicamente troppo sicure** — spesso 0.9999 — e una soglia messa
/// sopra numeri del genere non distingue niente. Si tara sui propri dati, mai
/// a occhio.
pub fn morbido(logit: &[f64], temperatura: f64) -> Result<Vec<f64>, String> {
    if !temperatura.is_finite() || temperatura <= 0.0 {
        return Err("la temperatura deve essere finita e positiva".into());
    }
    if logit.len() < 2 {
        return Err("servono almeno due logit".into());
    }
    if logit.iter().any(|x| !x.is_finite()) {
        return Err("un logit non finito non e' una risposta".into());
    }
    let massimo = logit.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let pesi: Vec<f64> = logit
        .iter()
        .map(|x| ((x - massimo) / temperatura).exp())
        .collect();
    let totale: f64 = pesi.iter().sum();
    if !(totale > 0.0) || !totale.is_finite() {
        return Err("la somma dei pesi non e' un numero utilizzabile".into());
    }
    Ok(pesi.iter().map(|x| x / totale).collect())
}

/// Toglie dai logit la preferenza che il modello ha per le lettere **prima di
/// sapere di cosa si parla**.
///
/// Chiedere la stessa domanda con un'evidenza priva di contenuto da' la
/// distribuzione a priori sulle lettere: se esce piatta, il modello non ha
/// preferenze e qui non cambia niente; se esce storta, quella pendenza sta
/// dentro **ogni** risposta e va tolta.
///
/// Per NOVA e' quasi gratis, ed e' il motivo per cui c'e': quella priorita' non
/// dipende dallo stato, solo dal testo della domanda. NOVA fa sempre le stesse
/// domande su stati che cambiano, quindi si misura una volta per forma di
/// domanda e non si rimisura piu'.
///
/// La sottrazione e' in spazio logaritmico — dividere le probabilita' — e il
/// risultato si rinormalizza da solo al passaggio successivo in [`morbido`].
pub fn senza_prioria(logit: &[f64], priorita: &[f64]) -> Result<Vec<f64>, String> {
    if logit.len() != priorita.len() {
        return Err("la priorita' ha una lunghezza diversa dai logit".into());
    }
    let prima = morbido(priorita, 1.0)?;
    let dopo: Vec<f64> = logit
        .iter()
        .zip(prima.iter())
        .map(|(x, p)| x - p.ln())
        .collect();
    if dopo.iter().any(|x| !x.is_finite()) {
        return Err("la correzione ha prodotto un valore non finito".into());
    }
    Ok(dopo)
}

/// Di quanto si e' disposti a credere che una somma cumulata abbia raggiunto
/// un quantile.
///
/// Sommare dieci volte un decimo non fa uno: fa 0.9999999999999999, e la somma
/// dei primi nove si ferma a 0.8999999999999999. Senza questo margine il
/// novantesimo percentile di una distribuzione piatta su dieci livelli salta di
/// **un'ancora intera** a seconda di come sono caduti gli ultimi bit — e gli
/// ultimi bit cambiano da macchina a macchina. E' successo: verde qui, rosso
/// sulla CI, con `p90` a 8 da una parte e 9 dall'altra.
///
/// Il margine non e' una tolleranza di confronto: e' la constatazione che una
/// cumulata di probabilita' porta con se' l'errore di dieci addizioni, e che un
/// quantile che cambia per quello non e' un quantile. Mille miliardesimi sono
/// enormemente piu' dell'errore accumulabile e enormemente meno di qualunque
/// differenza che significhi qualcosa.
pub const TOLLERANZA_CUMULATA: f64 = 1e-12;

/// Cosa si sa di un numero, letto da una distribuzione su valori discreti.
#[derive(Debug, Clone, PartialEq)]
pub struct Statistiche {
    pub media: f64,
    pub scarto: f64,
    pub mediana: f64,
    pub decimo: f64,
    pub novantesimo: f64,
}

/// La media pesata e la sua dispersione.
///
/// I quantili sono quelli della distribuzione **discreta sulle ancore**, non di
/// una curva continua: il decimo percentile e' la prima ancora la cui
/// probabilita' cumulata arriva a 0.1, non un valore interpolato. Dirlo
/// importa, perche' un `p10` che sembra un intervallo di confidenza non lo e'.
///
/// I valori devono arrivare **in ordine crescente**, come le ancore: i quantili
/// di una lista disordinata sarebbero numeri senza significato, e tacerlo
/// sarebbe il modo di ritrovarseli in un rapporto.
pub fn statistiche(valori: &[f64], probabilita: &[f64]) -> Result<Statistiche, String> {
    if valori.len() != probabilita.len() || valori.is_empty() {
        return Err("valori e probabilita' devono essere tanti e altrettanti".into());
    }
    if valori.windows(2).any(|c| c[0] > c[1]) {
        return Err("i valori devono arrivare in ordine crescente".into());
    }
    let media: f64 = valori
        .iter()
        .zip(probabilita)
        .map(|(v, p)| v * p)
        .sum::<f64>();
    let varianza: f64 = valori
        .iter()
        .zip(probabilita)
        .map(|(v, p)| p * (v - media).powi(2))
        .sum::<f64>();
    let quantile = |q: f64| -> f64 {
        let mut cumulata = 0.0;
        for (v, p) in valori.iter().zip(probabilita) {
            cumulata += p;
            if cumulata + TOLLERANZA_CUMULATA >= q {
                return *v;
            }
        }
        *valori.last().unwrap()
    };
    Ok(Statistiche {
        media,
        scarto: varianza.max(0.0).sqrt(),
        mediana: quantile(0.5),
        decimo: quantile(0.1),
        novantesimo: quantile(0.9),
    })
}

/// Quanto e' concentrata una distribuzione, fra 0 (piatta) e 1 (certa).
///
/// E' l'entropia normalizzata, girata. **Non e' la probabilita' di avere
/// ragione**: dice che forma ha la distribuzione, non se il modello ci ha
/// azzeccato. Le due cose si confondono facilmente e il nome aiuta a non farlo.
pub fn concentrazione(probabilita: &[f64]) -> f64 {
    let n = probabilita.len();
    if n < 2 {
        return 1.0;
    }
    let entropia: f64 = -probabilita
        .iter()
        .filter(|p| **p > 0.0)
        .map(|p| p * p.ln())
        .sum::<f64>();
    (1.0 - entropia / (n as f64).ln()).clamp(0.0, 1.0)
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn un_logit_enorme_non_fa_saltare_niente() {
        let p = morbido(&[1000.0, 0.0], 1.0).unwrap();
        assert!(p.iter().all(|x| x.is_finite()));
        assert!((p.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        assert!(p[0] > 0.999);
    }

    #[test]
    fn la_temperatura_appiattisce() {
        let fredda = morbido(&[2.0, 0.0], 0.5).unwrap();
        let calda = morbido(&[2.0, 0.0], 4.0).unwrap();
        assert!(fredda[0] > calda[0]);
        assert!(morbido(&[1.0, 0.0], 0.0).is_err());
        assert!(morbido(&[1.0, 0.0], f64::NAN).is_err());
    }

    /// Se la priorita' e' piatta non deve cambiare niente: e' il caso in cui
    /// il correttore non serve, e deve accorgersene da solo.
    #[test]
    fn una_priorita_piatta_non_sposta_le_probabilita() {
        let logit = [1.0, 2.0, 0.5];
        let prima = morbido(&logit, 1.0).unwrap();
        let corretti = senza_prioria(&logit, &[0.0, 0.0, 0.0]).unwrap();
        let dopo = morbido(&corretti, 1.0).unwrap();
        for (a, b) in prima.iter().zip(dopo.iter()) {
            assert!((a - b).abs() < 1e-12, "{prima:?} vs {dopo:?}");
        }
    }

    /// E se e' storta, la deve raddrizzare: una priorita' identica ai logit
    /// vuol dire che il modello risponde senza guardare l'evidenza, e la
    /// correzione deve lasciare una distribuzione piatta.
    #[test]
    fn una_priorita_uguale_ai_logit_lascia_il_piatto() {
        let logit = [3.0, 0.0, 1.0];
        let dopo = morbido(&senza_prioria(&logit, &logit).unwrap(), 1.0).unwrap();
        for p in &dopo {
            assert!((p - 1.0 / 3.0).abs() < 1e-12, "{dopo:?}");
        }
    }

    #[test]
    fn i_quantili_sono_quelli_delle_ancore() {
        let s = statistiche(&[0.0, 50.0, 100.0], &[0.2, 0.3, 0.5]).unwrap();
        assert!((s.media - 65.0).abs() < 1e-12);
        // La mediana e' la prima ancora la cui cumulata **arriva** a 0.5:
        // 0.2 + 0.3 = 0.5, quindi 50 e non 100. E' il punto in cui un
        // quantile discreto non somiglia a uno continuo.
        assert_eq!(s.decimo, 0.0);
        assert_eq!(s.mediana, 50.0);
        assert_eq!(s.novantesimo, 100.0);
    }

    /// Dieci decimi non fanno uno, e il quantile non deve accorgersene.
    ///
    /// E' il caso che ha fatto diventare rosso il banco sulla CI restando
    /// verde in locale: la somma dei primi nove decimi e' 0.8999999999999999,
    /// e senza margine il novantesimo percentile saltava dall'ottavo al nono
    /// livello a seconda della macchina.
    #[test]
    fn una_cumulata_che_arriva_per_un_pelo_conta_come_arrivata() {
        let valori: Vec<f64> = (0..10).map(|i| i as f64).collect();
        let ps = vec![0.1; 10];
        let somma_dei_primi_nove: f64 = ps[..9].iter().sum();
        assert!(
            somma_dei_primi_nove < 0.9,
            "il caso di prova non e' sul filo: {somma_dei_primi_nove}"
        );
        let s = statistiche(&valori, &ps).unwrap();
        assert_eq!(s.novantesimo, 8.0, "nove decimi arrivano a 0.9");
        assert_eq!(s.mediana, 4.0, "cinque decimi arrivano a 0.5");
        assert_eq!(s.decimo, 0.0);
    }

    /// Ma il margine non deve inghiottire un livello intero.
    #[test]
    fn il_margine_non_sposta_un_quantile_vero() {
        let s = statistiche(&[0.0, 1.0, 2.0], &[0.05, 0.5, 0.45]).unwrap();
        assert_eq!(s.decimo, 1.0, "0.05 non arriva a 0.1");
        assert_eq!(s.mediana, 1.0);
        assert_eq!(s.novantesimo, 2.0);
    }

    #[test]
    fn i_valori_disordinati_si_rifiutano() {
        assert!(statistiche(&[10.0, 0.0], &[0.5, 0.5]).is_err());
    }

    #[test]
    fn la_concentrazione_va_da_piatta_a_certa() {
        assert!(concentrazione(&[0.5, 0.5]) < 1e-12);
        assert!(concentrazione(&[1.0, 0.0]) > 1.0 - 1e-12);
    }
}
