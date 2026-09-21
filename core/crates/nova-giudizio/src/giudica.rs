//! Da una distribuzione a un giudizio tipizzato.
//!
//! E' l'unico posto dove si decide **quando ci si ferma**, e il perche' di ogni
//! soglia sta scritto accanto alla riga che la usa.

use crate::candidati::{candidati, Candidato, ABBASTANZA, FUORI_SOPRA, FUORI_SOTTO};
use crate::domanda::Domanda;
use crate::probabilita::{concentrazione, morbido, statistiche, Statistiche};
use crate::{Giudizio, Risposta};

/// Tutto quello che si e' letto da una distribuzione, giudizio compreso.
///
/// Il giudizio e' quello che si usa; il resto serve a chi deve **misurare**
/// se quel giudizio e' affidabile, e senza le probabilita' grezze non si puo'.
#[derive(Debug, Clone, PartialEq)]
pub struct Esito {
    pub giudizio: Giudizio,
    /// id del candidato -> probabilita', nell'ordine delle lettere.
    pub probabilita: Vec<(String, f64)>,
    /// Quanto e' andato alle vie d'uscita, tutte insieme.
    pub indisponibile: f64,
    /// La probabilita' della prima, qualunque essa sia.
    pub in_testa: f64,
    pub concentrazione: f64,
    /// Le statistiche della media pesata, per punteggi e numeri. Sono
    /// **condizionate alle sole opzioni valide**: un'astensione al 40% non
    /// deve trascinare la media verso lo zero.
    pub statistiche: Option<Statistiche>,
    /// La temperatura con cui si e' letta. Uno vuol dire «non calibrata».
    pub temperatura: f64,
}

impl Esito {
    /// Se le probabilita' sono state tarate su dati veri o no. E' una riga che
    /// va portata fino a chi legge: un numero calibrato e uno che non lo e'
    /// hanno la stessa faccia e un significato diverso.
    pub fn taratura(&self) -> &'static str {
        if self.temperatura == 1.0 {
            "non calibrata: e' il punteggio grezzo delle opzioni"
        } else {
            "scalata in temperatura: vale se la taratura e' stata verificata a parte"
        }
    }
}

/// Se a questo punto ci si ferma invece di rispondere.
///
/// E' una funzione a parte, e minuscola, per una ragione precisa: e' **il**
/// confine, e va fissato con numeri esatti. Dentro `giudica` le probabilita'
/// arrivano da un esponenziale e da una divisione, e sul confine quelle cadono
/// da una parte o dall'altra dell'ultimo bit a seconda della macchina — quindi
/// una prova che passi di li' non puo' inchiodare la semantica di `>=`. Qui si'.
///
/// Due condizioni, e la seconda non e' implicata dalla prima: con una sola via
/// d'uscita una quota oltre la meta' la mette gia' in testa da sola, ma con tre
/// — i numeri — possono sommare oltre la soglia restando ciascuna sotto a chi
/// guida. Quello non e' una risposta, e' un pareggio mal letto.
pub fn ci_si_ferma(prima_e_speciale: bool, indisponibile: f64, soglia: f64) -> bool {
    prima_e_speciale || indisponibile >= soglia
}

/// Legge i logit delle lettere e ne fa un giudizio.
///
/// I logit arrivano **nell'ordine dei candidati**, uno per lettera. Se sono di
/// numero diverso e' un errore e non un caso da indovinare: vorrebbe dire che
/// chi ha chiesto e chi ha letto avevano in mente due domande diverse.
pub fn giudica(domanda: &Domanda, logit: &[f64], temperatura: f64) -> Result<Esito, String> {
    domanda.valida()?;
    let scelte = candidati(domanda);
    if logit.len() != scelte.len() {
        return Err(format!(
            "{} logit per {} candidati: la domanda letta non e' quella posta",
            logit.len(),
            scelte.len()
        ));
    }
    let ps = morbido(logit, temperatura)?;
    let politica = domanda.politica();

    let quota = |id: &str| -> f64 {
        scelte
            .iter()
            .zip(&ps)
            .find(|(c, _)| c.id == id)
            .map(|(_, p)| *p)
            .unwrap_or(0.0)
    };
    let non_basta = quota(ABBASTANZA);
    let sotto = quota(FUORI_SOTTO);
    let sopra = quota(FUORI_SOPRA);
    let indisponibile = non_basta + sotto + sopra;

    let validi: Vec<(&Candidato, f64)> = scelte
        .iter()
        .zip(&ps)
        .filter(|(c, _)| !c.speciale())
        .map(|(c, p)| (c, *p))
        .collect();
    let disponibile: f64 = validi.iter().map(|(_, p)| p).sum();

    // La prima in assoluto, speciali comprese: e' la risposta del modello, non
    // quella che ci fa comodo.
    let (prima, in_testa) =
        scelte
            .iter()
            .zip(&ps)
            .fold((None::<&Candidato>, f64::NEG_INFINITY), |(c, m), (x, p)| {
                if *p > m {
                    (Some(x), *p)
                } else {
                    (c, m)
                }
            });
    let prima = prima.ok_or("nessun candidato")?;

    let base = Esito {
        giudizio: Giudizio::NonBasta { quanto: non_basta },
        probabilita: scelte
            .iter()
            .zip(&ps)
            .map(|(c, p)| (c.id.clone(), *p))
            .collect(),
        indisponibile,
        in_testa,
        concentrazione: concentrazione(&ps),
        statistiche: None,
        temperatura,
    };

    // Ci si ferma se ha vinto una via d'uscita, **oppure** se le vie d'uscita
    // insieme superano la soglia: una risposta che vince con il 40% mentre il
    // 45% dice «non lo so» non e' una risposta, e' un pareggio mal letto.
    if ci_si_ferma(
        prima.speciale(),
        indisponibile,
        politica.massimo_indisponibile,
    ) {
        // Fuori scala e informazione mancante sono due cose diverse e si
        // distinguono da chi ha preso piu' probabilita'.
        let giudizio = if sotto + sopra > non_basta {
            Giudizio::FuoriScala { sotto, sopra }
        } else {
            Giudizio::NonBasta { quanto: non_basta }
        };
        return Ok(Esito { giudizio, ..base });
    }
    if in_testa < politica.minimo_in_testa {
        return Ok(Esito {
            giudizio: Giudizio::Incerto { in_testa },
            ..base
        });
    }
    if !(disponibile > 0.0) {
        return Ok(Esito {
            giudizio: Giudizio::NonBasta { quanto: non_basta },
            ..base
        });
    }

    // Le medie si fanno **sulle sole opzioni valide, rinormalizzate**: cio' che
    // e' andato a «non lo so» non e' un valore basso, e' un'assenza.
    let condizionate: Vec<f64> = validi.iter().map(|(_, p)| p / disponibile).collect();
    let (giudizio, stat) = match domanda {
        Domanda::Scelta { .. } => (
            Giudizio::Risposto(Risposta::Scelta {
                id: prima.id.clone(),
            }),
            None,
        ),
        Domanda::Booleana { .. } => {
            let vero = validi
                .iter()
                .zip(&condizionate)
                .find(|((c, _), _)| c.id == "vero")
                .map(|(_, p)| *p)
                .unwrap_or(0.0);
            (
                Giudizio::Risposto(Risposta::Booleana {
                    valore: prima.id == "vero",
                    probabilita_vero: vero,
                }),
                None,
            )
        }
        Domanda::Punteggio { livelli, .. } => {
            let valori: Vec<f64> = validi.iter().filter_map(|(c, _)| c.valore).collect();
            let s = statistiche(&valori, &condizionate)?;
            let alto = (livelli.len() - 1) as f64;
            (
                Giudizio::Risposto(Risposta::Punteggio {
                    punti: s.media,
                    normalizzato: if alto > 0.0 { s.media / alto } else { 0.0 },
                }),
                Some(s),
            )
        }
        Domanda::Numerica { unita, .. } => {
            let valori: Vec<f64> = validi.iter().filter_map(|(c, _)| c.valore).collect();
            let s = statistiche(&valori, &condizionate)?;
            (
                Giudizio::Risposto(Risposta::Numerica {
                    valore: s.media,
                    unita: unita.clone(),
                }),
                Some(s),
            )
        }
    };
    Ok(Esito {
        giudizio,
        statistiche: stat,
        ..base
    })
}

#[cfg(test)]
mod prove {
    use super::*;
    use crate::domanda::{Ancora, Opzione, Politica};

    fn booleana(politica: Politica) -> Domanda {
        Domanda::Booleana {
            istruzioni: "E' urgente?".into(),
            descrizione_vero: crate::domanda::VERO_PREDEFINITO.into(),
            descrizione_falso: crate::domanda::FALSO_PREDEFINITO.into(),
            politica,
        }
    }

    fn numerica() -> Domanda {
        Domanda::Numerica {
            istruzioni: "Quanto?".into(),
            unita: "percent".into(),
            ancore: vec![
                Ancora {
                    valore: 0.0,
                    descrizione: "Vuoto".into(),
                },
                Ancora {
                    valore: 100.0,
                    descrizione: "Pieno".into(),
                },
            ],
            politica: Politica::default(),
        }
    }

    fn scelta() -> Domanda {
        Domanda::Scelta {
            istruzioni: "A chi tocca?".into(),
            opzioni: vec![
                Opzione {
                    id: "conti".into(),
                    descrizione: "Pagamenti".into(),
                },
                Opzione {
                    id: "tecnici".into(),
                    descrizione: "Guasti".into(),
                },
            ],
            politica: Politica::default(),
        }
    }

    #[test]
    fn un_booleano_si_legge_e_porta_la_probabilita_del_vero() {
        // falso, vero, non_basta
        let e = giudica(&booleana(Politica::default()), &[0.0, 5.0, -5.0], 1.0).unwrap();
        let Giudizio::Risposto(Risposta::Booleana {
            valore,
            probabilita_vero,
        }) = e.giudizio
        else {
            panic!("{:?}", e.giudizio)
        };
        assert!(valore);
        assert!(probabilita_vero > 0.99, "{probabilita_vero}");
    }

    /// «Non lo so» che vince non e' un falso: e' una domanda da rigirare.
    #[test]
    fn non_basta_non_diventa_un_no() {
        let e = giudica(&booleana(Politica::default()), &[0.0, 0.0, 9.0], 1.0).unwrap();
        assert!(
            matches!(e.giudizio, Giudizio::NonBasta { .. }),
            "{:?}",
            e.giudizio
        );
        assert!(e.giudizio.risposta().is_none());
    }

    /// E nemmeno un pareggio mal letto: se le vie d'uscita **insieme** passano
    /// la soglia ci si ferma anche quando una risposta e' in testa.
    ///
    /// Il caso si costruisce solo con piu' di una via d'uscita, ed e' una cosa
    /// che vale la pena sapere: con la sola astensione, una quota oltre la
    /// meta' la mette gia' in testa da sola, quindi la soglia non fa mai un
    /// lavoro suo. Lo fa sui numeri, dove le vie sono tre.
    #[test]
    fn tre_vie_d_uscita_insieme_fermano_una_risposta_in_testa() {
        let d = numerica();
        // ancora0 0.30, ancora1 0.0, sotto 0.25, sopra 0.25, non_basta 0.20
        let p: [f64; 5] = [0.30, 0.0001, 0.25, 0.25, 0.1999];
        let logit: Vec<f64> = p.iter().map(|x: &f64| x.ln()).collect();
        let e = giudica(&d, &logit, 1.0).unwrap();
        assert!(e.in_testa > 0.29 && e.in_testa < 0.31, "{}", e.in_testa);
        assert!(e.indisponibile > 0.69, "{}", e.indisponibile);
        assert!(e.giudizio.risposta().is_none(), "{:?}", e.giudizio);
        // Fra le tre, sotto+sopra pesano piu' dell'astensione: e' fuori scala.
        assert!(
            matches!(e.giudizio, Giudizio::FuoriScala { .. }),
            "{:?}",
            e.giudizio
        );
    }

    /// Il confine, con numeri esatti e senza passare da un esponenziale.
    ///
    /// Sul confine le probabilita' calcolate cadono di qua o di la' dell'ultimo
    /// bit a seconda della macchina, quindi una prova che le faccia calcolare
    /// non inchioda `>=`. Qui i numeri si scrivono, e la semantica e' fissata.
    #[test]
    fn sulla_soglia_esatta_ci_si_ferma() {
        assert!(ci_si_ferma(false, 0.5, 0.5), "sulla soglia ci si ferma");
        assert!(!ci_si_ferma(false, 0.25, 0.5), "sotto si risponde");
        assert!(ci_si_ferma(false, 0.75, 0.5));
        // Una via d'uscita in testa ferma comunque, qualunque sia la soglia.
        assert!(ci_si_ferma(true, 0.0, 1.0));
        assert!(!ci_si_ferma(false, 0.999, 1.0));
        assert!(ci_si_ferma(false, 1.0, 1.0));
    }

    #[test]
    fn fuori_scala_e_informazione_mancante_non_si_confondono() {
        let d = numerica();
        // ancore, sotto, sopra, non_basta
        let e = giudica(&d, &[0.0, 0.0, 6.0, 0.0, 0.0], 1.0).unwrap();
        assert!(
            matches!(e.giudizio, Giudizio::FuoriScala { .. }),
            "{:?}",
            e.giudizio
        );
        let f = giudica(&d, &[0.0, 0.0, 0.0, 0.0, 6.0], 1.0).unwrap();
        assert!(
            matches!(f.giudizio, Giudizio::NonBasta { .. }),
            "{:?}",
            f.giudizio
        );
    }

    /// La media non deve essere trascinata da cio' che non e' una risposta.
    #[test]
    fn la_media_ignora_cio_che_non_e_una_risposta() {
        let d = Domanda::Punteggio {
            istruzioni: "Quanto e' arrabbiato?".into(),
            livelli: vec!["Calmo".into(), "Seccato".into(), "Furioso".into()],
            politica: Politica::default(),
        };
        // tutta la probabilita' valida sul livello alto, un po' su non_basta
        let e = giudica(&d, &[-10.0, -10.0, 1.0, 0.5], 1.0).unwrap();
        let Giudizio::Risposto(Risposta::Punteggio {
            punti,
            normalizzato,
        }) = e.giudizio
        else {
            panic!("{:?}", e.giudizio)
        };
        assert!(punti > 1.99, "{punti}");
        assert!((normalizzato - punti / 2.0).abs() < 1e-12);
    }

    #[test]
    fn il_numero_di_logit_sbagliato_e_un_errore_non_un_indovinello() {
        assert!(giudica(&scelta(), &[1.0, 2.0], 1.0).is_err());
        assert!(giudica(&scelta(), &[1.0, 2.0, 3.0, 4.0], 1.0).is_err());
    }

    /// Un giudizio puo' aggiungere un divieto, mai toglierne uno.
    #[test]
    fn stringere_non_allenta_mai() {
        let innocuo = Giudizio::Risposto(Risposta::Booleana {
            valore: false,
            probabilita_vero: 0.01,
        });
        assert!(
            innocuo.stringe(true, |_| false),
            "una regola che vieta vince sempre"
        );
        assert!(!innocuo.stringe(false, |_| false));
        let pericoloso = Giudizio::Risposto(Risposta::Booleana {
            valore: true,
            probabilita_vero: 0.99,
        });
        assert!(pericoloso.stringe(false, |r| matches!(
            r,
            Risposta::Booleana { valore: true, .. }
        )));
        // E il dubbio stringe come un si'.
        assert!(Giudizio::NonBasta { quanto: 0.9 }.stringe(false, |_| false));
        assert!(Giudizio::Incerto { in_testa: 0.3 }.stringe(false, |_| false));
    }
}
