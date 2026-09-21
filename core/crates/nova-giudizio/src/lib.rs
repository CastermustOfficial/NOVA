//! Decisioni tipizzate lette dai logit, senza generare un solo token.
//!
//! `nova_decisioni` dice **di che materia** e' fatta ogni decisione di NOVA, e
//! nella sua prima pagina c'e' una frase che questa cassetta viene a smentire:
//! le euristiche di oggi — liste di parole, soglie, espressioni regolari —
//! «restano cosi' non per pigrizia ma perche' l'alternativa costava un giro di
//! modello per ogni domandina».
//!
//! Non costa piu' quel prezzo. Una domanda a scelta multipla in cui ogni
//! risposta e' **una lettera maiuscola** si risolve leggendo i logit di quelle
//! lettere dopo un forward pass solo: niente ciclo di decodifica, niente JSON
//! da riparare, niente testo da interpretare. Quel che torna non e' una
//! risposta da leggere, e' **una distribuzione**: quale opzione, e quanto
//! sicura.
//!
//! Qui dentro non c'e' nessun modello e nessuna rete. C'e' la meta' pura: come
//! una domanda diventa un elenco di candidati, come i logit diventano un
//! giudizio tipizzato, e cosa si ha il diritto di leggere da quel giudizio.
//! Chi parla con `llama-server` sta altrove, e questa parte si prova senza
//! scaricare un peso.
//!
//! ## L'idea non e' nostra
//!
//! Il modello di programmazione viene da **Jev** di TypeSafe («System One»:
//! stato non strutturato dentro, decisioni tipizzate con probabilita' fuori).
//! La dimostrazione che si puo' fare **in casa**, con pesi aperti, e' di
//! [Rizzo Flow](https://github.com/Rizzo-AI-Academy/rizzo-flow), a sua volta
//! ispirato a [SemIf](https://github.com/TheoLeeCJ/SemIf). Qui non c'e' codice
//! loro: c'e' la stessa idea, scritta per NOVA, con le scelte di NOVA — che
//! dove divergono lo dicono.
//!
//! ## Le tre scelte di NOVA
//!
//! **Un giudizio non ha un valore nullo.** Altrove il risultato e' un campo
//! `value` che vale `null` piu' uno `status` che spiega perche'. E' la forma
//! che consente a chi chiama di dimenticarsi del secondo: legge il primo, lo
//! trova vuoto, e ci mette un ripiego. Qui [`Giudizio`] e' un enum: il valore
//! **non si puo' leggere** senza aver trattato il caso in cui non c'e'. E' la
//! stessa regola del gradino che e' un processo e non un indirizzo — costruire
//! la cosa sbagliata deve essere impossibile, non tardivo.
//!
//! **Non basta vuol dire «chiedo», non vuol dire «no».** In un sistema che
//! agisce sul PC di qualcuno, «l'evidenza non mi dice» e «l'evidenza dice di
//! no» sono due risposte opposte, e confonderle e' il modo di fare una cosa
//! che nessuno aveva chiesto. [`Giudizio::NonBasta`] esiste per non poterle
//! confondere.
//!
//! **Un giudizio puo' solo stringere una guardia, mai allentarla.** Le guardie
//! di NOVA sono deterministiche e non negoziabili (D309). Un modello che dice
//! «questo comando e' innocuo» non deve poter aprire una porta che una regola
//! ha chiuso; uno che dice «questo e' pericoloso» deve poterne chiudere una che
//! era aperta. Vedi [`Giudizio::stringe`], che e' l'unico modo previsto di
//! trasformare un giudizio in un permesso.

pub mod candidati;
pub mod domanda;
pub mod giudica;
pub mod lettere;
pub mod probabilita;

pub use candidati::{candidati, Candidato, ABBASTANZA, FUORI_SOPRA, FUORI_SOTTO};
pub use domanda::{Ancora, Domanda, Opzione, Politica};
pub use giudica::{giudica, Esito};
pub use lettere::{lettera, posizione, LETTERE, MASSIMI_CANDIDATI};
pub use probabilita::{morbido, senza_prioria, statistiche, Statistiche};

/// Cosa si e' deciso, e cosa si ha il diritto di leggerne.
///
/// Le varianti diverse da [`Giudizio::Risposto`] non portano un valore, e non
/// e' una dimenticanza: e' il punto. Un `Option` accanto a uno `status` si
/// puo' guardare a meta'; un enum no.
#[derive(Debug, Clone, PartialEq)]
pub enum Giudizio {
    /// C'e' una risposta, e la politica la accetta.
    Risposto(Risposta),
    /// L'evidenza non dice abbastanza. **Non e' un no**: e' «chiedi».
    NonBasta { quanto: f64 },
    /// Il valore si conosce, ed e' fuori dalle ancore dichiarate.
    FuoriScala { sotto: f64, sopra: f64 },
    /// C'e' una preferenza, ma piu' bassa di quella che la politica esige.
    Incerto { in_testa: f64 },
}

/// La risposta vera, per tipo di domanda.
#[derive(Debug, Clone, PartialEq)]
pub enum Risposta {
    /// Vero o falso, con la probabilita' del vero fra le sole opzioni valide.
    Booleana { valore: bool, probabilita_vero: f64 },
    /// L'identificativo dell'opzione scelta.
    Scelta { id: String },
    /// Media pesata sui livelli, e la stessa media portata in [0, 1].
    Punteggio { punti: f64, normalizzato: f64 },
    /// Media pesata sulle ancore, con l'unita' dichiarata dalla domanda.
    Numerica { valore: f64, unita: String },
}

impl Giudizio {
    /// La risposta, se c'e'.
    pub fn risposta(&self) -> Option<&Risposta> {
        match self {
            Giudizio::Risposto(r) => Some(r),
            _ => None,
        }
    }

    /// Una riga che dice com'e' andata, per il registro e per chi guarda.
    pub fn come_si_racconta(&self) -> String {
        match self {
            Giudizio::Risposto(Risposta::Booleana { valore, .. }) => {
                format!("si', {}", if *valore { "vero" } else { "falso" })
            }
            Giudizio::Risposto(Risposta::Scelta { id }) => format!("scelta: {id}"),
            Giudizio::Risposto(Risposta::Punteggio { punti, .. }) => {
                format!("punteggio {punti:.2}")
            }
            Giudizio::Risposto(Risposta::Numerica { valore, unita }) => {
                format!("{valore:.3} {unita}")
            }
            Giudizio::NonBasta { quanto } => {
                format!(
                    "l'evidenza non basta ({:.0}% su «non lo so»)",
                    quanto * 100.0
                )
            }
            Giudizio::FuoriScala { sotto, sopra } => format!(
                "il valore e' fuori dalle ancore ({:.0}% sotto, {:.0}% sopra)",
                sotto * 100.0,
                sopra * 100.0
            ),
            Giudizio::Incerto { in_testa } => {
                format!(
                    "nessuna opzione convince ({:.0}% alla prima)",
                    in_testa * 100.0
                )
            }
        }
    }

    /// L'unico modo previsto di trasformare un giudizio in un permesso.
    ///
    /// `gia_vietato` e' la risposta della guardia deterministica. Il giudizio
    /// puo' **aggiungere** un divieto, mai toglierne uno: se la regola ha gia'
    /// detto no, qui esce no comunque sia andata la decisione.
    ///
    /// Anche il dubbio vieta. Un giudizio che non arriva a una risposta, in un
    /// punto in cui si sta decidendo se lasciar fare qualcosa, e' esattamente
    /// il momento in cui ci si ferma e si chiede: `NonBasta` e `Incerto`
    /// stringono come stringerebbe un si'.
    pub fn stringe(&self, gia_vietato: bool, pericoloso: impl FnOnce(&Risposta) -> bool) -> bool {
        if gia_vietato {
            return true;
        }
        match self {
            Giudizio::Risposto(r) => pericoloso(r),
            _ => true,
        }
    }
}
