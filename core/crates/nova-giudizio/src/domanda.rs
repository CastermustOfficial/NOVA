//! Le quattro forme di domanda, e la politica che decide quando ci si ferma.

use crate::lettere::MASSIMI_CANDIDATI;

/// Un valore rappresentativo su una scala numerica.
///
/// **Non e' un intervallo statistico**: e' un punto con scritto accanto cosa
/// vuol dire. La media pesata sulle ancore resta dentro la prima e l'ultima,
/// e questo va detto a chi legge un numero che sembra una misura.
#[derive(Debug, Clone, PartialEq)]
pub struct Ancora {
    pub valore: f64,
    pub descrizione: String,
}

/// Quando una risposta si accetta e quando ci si ferma.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Politica {
    /// Se offrire al modello la via d'uscita «non lo so».
    ///
    /// Accesa di default, ed e' una scelta: senza, un modello che non ha
    /// l'informazione **sceglie comunque**, e sceglie con sicurezza.
    pub puo_astenersi: bool,
    /// Oltre questa quota di probabilita' sulle opzioni non-risposta, ci si
    /// ferma anche se la prima opzione valida e' in testa.
    pub massimo_indisponibile: f64,
    /// Sotto questa probabilita' la prima opzione non basta. Zero di default:
    /// e' una soglia che va scelta sui propri dati, e un valore inventato qui
    /// rifiuterebbe risposte buone senza che nessuno l'abbia chiesto.
    pub minimo_in_testa: f64,
}

impl Default for Politica {
    fn default() -> Politica {
        Politica {
            puo_astenersi: true,
            massimo_indisponibile: 0.5,
            minimo_in_testa: 0.0,
        }
    }
}

impl Politica {
    /// Una politica che non si astiene mai. Serve dove la domanda ha gia' una
    /// risposta «non si sa» fra le opzioni vere.
    pub fn senza_astensione() -> Politica {
        Politica {
            puo_astenersi: false,
            ..Politica::default()
        }
    }

    fn valida(&self) -> Result<(), String> {
        for (nome, v) in [
            ("massimo_indisponibile", self.massimo_indisponibile),
            ("minimo_in_testa", self.minimo_in_testa),
        ] {
            if !v.is_finite() || !(0.0..=1.0).contains(&v) {
                return Err(format!("{nome} deve stare fra 0 e 1"));
            }
        }
        if self.massimo_indisponibile <= 0.0 {
            return Err("massimo_indisponibile a zero rifiuterebbe ogni risposta".into());
        }
        Ok(())
    }
}

/// Un'opzione di una domanda a scelta.
#[derive(Debug, Clone, PartialEq)]
pub struct Opzione {
    pub id: String,
    pub descrizione: String,
}

/// Cosa si sta chiedendo.
///
/// Le quattro forme non sono zucchero sintattico sopra «scelta»: cambiano cosa
/// si legge dalla distribuzione. Una scelta guarda chi e' in testa; un
/// punteggio e un numero fanno una **media pesata**, che e' un'altra cosa e usa
/// tutta la distribuzione invece del solo massimo.
#[derive(Debug, Clone, PartialEq)]
pub enum Domanda {
    Booleana {
        istruzioni: String,
        descrizione_vero: String,
        descrizione_falso: String,
        politica: Politica,
    },
    Scelta {
        istruzioni: String,
        opzioni: Vec<Opzione>,
        politica: Politica,
    },
    Punteggio {
        istruzioni: String,
        /// Dal basso all'alto. L'ordine e' il significato.
        livelli: Vec<String>,
        politica: Politica,
    },
    Numerica {
        istruzioni: String,
        unita: String,
        /// Strettamente crescenti.
        ancore: Vec<Ancora>,
        politica: Politica,
    },
}

/// Il testo di default del ramo vero, quando chi chiede non lo scrive.
pub const VERO_PREDEFINITO: &str = "Yes. The evidence supports an affirmative answer.";
/// E quello del ramo falso.
pub const FALSO_PREDEFINITO: &str = "No. The evidence supports a negative answer.";

impl Domanda {
    pub fn istruzioni(&self) -> &str {
        match self {
            Domanda::Booleana { istruzioni, .. }
            | Domanda::Scelta { istruzioni, .. }
            | Domanda::Punteggio { istruzioni, .. }
            | Domanda::Numerica { istruzioni, .. } => istruzioni,
        }
    }

    pub fn politica(&self) -> Politica {
        match self {
            Domanda::Booleana { politica, .. }
            | Domanda::Scelta { politica, .. }
            | Domanda::Punteggio { politica, .. }
            | Domanda::Numerica { politica, .. } => *politica,
        }
    }

    /// Il nome della forma, per i registri e per il banco.
    pub fn tipo(&self) -> &'static str {
        match self {
            Domanda::Booleana { .. } => "booleana",
            Domanda::Scelta { .. } => "scelta",
            Domanda::Punteggio { .. } => "punteggio",
            Domanda::Numerica { .. } => "numerica",
        }
    }

    /// Cio' che rende la domanda ponibile: abbastanza opzioni, non troppe,
    /// ancore crescenti, politica sensata.
    ///
    /// Si controlla **qui**, non dove si compone il prompt: una domanda
    /// impossibile scoperta dopo aver prefillato lo stato e' un errore che
    /// arriva tardi, quando il lavoro caro e' gia' stato fatto.
    pub fn valida(&self) -> Result<(), String> {
        self.politica().valida()?;
        if self.istruzioni().trim().is_empty() {
            return Err("una domanda senza istruzioni non chiede niente".into());
        }
        let riservati = usize::from(self.politica().puo_astenersi)
            + if matches!(self, Domanda::Numerica { .. }) {
                2
            } else {
                0
            };
        let quanti = match self {
            Domanda::Booleana { .. } => 2,
            Domanda::Scelta { opzioni, .. } => {
                if opzioni.iter().any(|o| o.id.trim().is_empty()) {
                    return Err("ogni opzione ha bisogno di un identificativo".into());
                }
                let mut visti: Vec<&str> = opzioni.iter().map(|o| o.id.as_str()).collect();
                visti.sort_unstable();
                let quante = visti.len();
                visti.dedup();
                if visti.len() != quante {
                    return Err("due opzioni con lo stesso identificativo".into());
                }
                opzioni.len()
            }
            Domanda::Punteggio { livelli, .. } => {
                let mut visti: Vec<&str> = livelli.iter().map(String::as_str).collect();
                visti.sort_unstable();
                let quanti = visti.len();
                visti.dedup();
                if visti.len() != quanti {
                    return Err("due livelli con la stessa descrizione".into());
                }
                livelli.len()
            }
            Domanda::Numerica { ancore, unita, .. } => {
                if unita.trim().is_empty() {
                    return Err("un numero senza unita' non si legge".into());
                }
                if ancore
                    .windows(2)
                    .any(|c| !(c[0].valore < c[1].valore) || !c[1].valore.is_finite())
                {
                    return Err("le ancore devono essere strettamente crescenti e finite".into());
                }
                if ancore.first().is_some_and(|a| !a.valore.is_finite()) {
                    return Err("le ancore devono essere strettamente crescenti e finite".into());
                }
                ancore.len()
            }
        };
        if quanti < 2 {
            return Err("servono almeno due candidati".into());
        }
        if quanti + riservati > MASSIMI_CANDIDATI {
            return Err(format!(
                "ci stanno al massimo {} voci: {MASSIMI_CANDIDATI} lettere, {riservati} riservate",
                MASSIMI_CANDIDATI - riservati
            ));
        }
        Ok(())
    }
}
