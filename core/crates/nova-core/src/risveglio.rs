//! Il microfono sempre aperto, e la conversazione che ne segue.
//!
//! ```text
//!   DORMIENTE   il microfono e' aperto ma si cerca solo il nome
//!       | «Nova»
//!       v
//!    SVEGLIA    tutto quello che dici va al cervello, senza ripetere il nome.
//!       |       Fra una frase e l'altra bastano due secondi di silenzio.
//!       |
//!       +-- il cervello dice «pausa»  -> IN PAUSA: la conversazione resta,
//!       |                                l'orecchio si chiude. «Nova» riapre.
//!       +-- il cervello dice «fine»   -> DORMIENTE: si chiude e si dimentica.
//! ```
//!
//! Due scelte che vengono da come si parla davvero:
//!
//! Il nome serve **una volta sola**. Doverlo ripetere a ogni frase non e' una
//! conversazione, e' un citofono.
//!
//! E la frase di chiusura non e' in un elenco qui dentro. «Okay basta cosi'»,
//! «stop», «dacci un taglio» sono infinite, e una lista di parole le sbaglia
//! sempre. Lo decide il cervello, che l'intenzione la capisce.
//!
//! Quello che si dice mentre e' DORMIENTE e non contiene il nome viene
//! buttato senza lasciare traccia: un microfono aperto tutto il giorno non
//! deve diventare un registro di quello che si dice in casa.

use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use serde_json::json;

use crate::bus::Bus;

pub const DORMIENTE: u8 = 0;
pub const SVEGLIA: u8 = 1;
pub const IN_PAUSA: u8 = 2;

pub fn nome_fase(f: u8) -> &'static str {
    match f {
        SVEGLIA => "sveglia",
        IN_PAUSA => "in_pausa",
        _ => "dormiente",
    }
}

pub struct Stato {
    pub acceso: AtomicBool,
    /// NOVA sta parlando: non deve ascoltare se stessa.
    pub bocca_aperta: AtomicBool,
    /// Quante volte NOVA ha aperto bocca. Vedi `apri_bocca`.
    pub turni_di_parola: AtomicU64,
    pub fase: AtomicU8,
}

pub fn stato() -> &'static Arc<Stato> {
    static S: OnceLock<Arc<Stato>> = OnceLock::new();
    S.get_or_init(|| {
        Arc::new(Stato {
            acceso: AtomicBool::new(false),
            bocca_aperta: AtomicBool::new(false),
            turni_di_parola: AtomicU64::new(0),
            fase: AtomicU8::new(DORMIENTE),
        })
    })
}

/// NOVA comincia a parlare.
///
/// La bandierina da sola non basta. Il ciclo la guarda *prima* di registrare,
/// ma la registrazione dura secondi: se NOVA comincia a parlare a meta', quel
/// pezzo di audio contiene la sua voce e finisce a whisper. In conversazione
/// aperta questo non e' un fastidio, e' un anello — NOVA si sente, risponde a
/// se stessa, e va avanti da sola.
///
/// Il contatore serve a chi stava gia' registrando: se e' cambiato mentre
/// registrava, quell'audio si butta senza guardarlo.
pub fn apri_bocca() {
    let s = stato();
    s.turni_di_parola.fetch_add(1, Ordering::SeqCst);
    s.bocca_aperta.store(true, Ordering::SeqCst);
}

pub fn chiudi_bocca() {
    stato().bocca_aperta.store(false, Ordering::SeqCst);
}

pub fn in_ascolto() -> bool {
    stato().acceso.load(Ordering::SeqCst)
}

pub fn fase() -> u8 {
    stato().fase.load(Ordering::SeqCst)
}

/// Lo stato dell'orb quando NOVA smette di parlare o d'agire: verde se
/// la conversazione e' aperta (tocca all'utente), grigio se e' andata a
/// dormire. Prima si tornava sempre a 'quiete' e l'orb sembrava inattivo
/// anche mentre ascoltava: non si capiva quando parlare.
pub fn stato_a_riposo() -> &'static str {
    if fase() == SVEGLIA { "ascolto" } else { "spento" }
}

pub fn ferma() {
    let s = stato();
    s.acceso.store(false, Ordering::SeqCst);
    s.fase.store(DORMIENTE, Ordering::SeqCst);
}

/// La cambia chi riceve la risposta del cervello.
pub fn imposta_fase(bus: &Bus, nuova: u8) {
    let s = stato();
    let prima = s.fase.swap(nuova, Ordering::SeqCst);
    if prima == nuova {
        return;
    }
    bus.emit("voce.fase", json!({ "fase": nome_fase(nuova) }));
    bus.emit(
        "stato.cambiato",
        json!({ "stato": if nuova == SVEGLIA { "ascolto" } else { "spento" } }),
    );
}

/// Quanto silenzio chiude una frase.
///
/// Piu' lungo in conversazione: chi parla si ferma a pensare, e tagliargli la
/// frase a meta' del ragionamento e' il difetto piu' fastidioso di un
/// assistente vocale.
fn silenzio_per(fase: u8) -> f32 {
    if fase == SVEGLIA {
        2.5
    } else {
        1.2
    }
}

/// Accende il ciclo. Ritorna false se girava gia'.
pub fn avvia(
    bus: Bus,
    microfono: Option<String>,
    cartella_ascolto: std::path::PathBuf,
    parola: String,
) -> bool {
    let s = stato();
    if s.acceso.swap(true, Ordering::SeqCst) {
        return false;
    }
    s.fase.store(DORMIENTE, Ordering::SeqCst);
    tokio::spawn(async move {
        tracing::info!(parola = %parola, "risveglio acceso");
        bus.emit("risveglio.acceso", json!({ "parola": parola }));
        let s = stato();
        let mut errori_di_fila = 0u32;
        // Il microfono muto e' uno stato, non un guasto: si dice una volta e
        // si continua a guardare, piu' di rado.
        let mut muto = false;
        // Rete di sicurezza: in conversazione aperta, dopo un lungo
        // silenzio non si resta svegli all'infinito. Non e' un elenco di
        // parole di chiusura: e' solo 'se non parli piu', mi metto da
        // parte'. Il 'fine' resta una decisione del cervello.
        let mut ultimo_attivo = std::time::Instant::now();
        const INATTIVITA: std::time::Duration = std::time::Duration::from_secs(60);

        while s.acceso.load(Ordering::SeqCst) {
            if s.bocca_aperta.load(Ordering::SeqCst) {
                ultimo_attivo = std::time::Instant::now();
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                continue;
            }
            let fase_ora = s.fase.load(Ordering::SeqCst);
            if fase_ora == SVEGLIA && ultimo_attivo.elapsed() >= INATTIVITA {
                // Silenzio prolungato: si mette in pausa da sola. La
                // conversazione resta, il nome la riapre con un 'riprendiamo'.
                tracing::info!("silenzio prolungato: in pausa");
                imposta_fase(&bus, IN_PAUSA);
                continue;
            }
            let mic = microfono.clone();
            let cartella = cartella_ascolto.clone();
            let glossario = parola.clone();
            let silenzio = silenzio_per(fase_ora);
            let parole_prima = s.turni_di_parola.load(Ordering::SeqCst);

            let esito = tokio::task::spawn_blocking(move || -> anyhow::Result<Option<(String, f32)>> {
                let a = nova_voce::ascolta_con_attesa(mic.as_deref(), 8.0, 20.0, silenzio, 16_000)?;
                if !a.ha_parlato || a.campioni.is_empty() {
                    tracing::debug!(picco = a.picco, "giro a vuoto");
                    return Ok(None);
                }
                let mut t = nova_voce::Trascrittore::nuovo(&cartella, "it")?;
                t.glossario = vec![glossario];
                Ok(Some((t.trascrivi(&a.campioni, a.frequenza)?, a.picco)))
            })
            .await;

            // Se NOVA ha parlato mentre questo pezzo veniva registrato, il
            // pezzo contiene la sua voce: si butta senza trascriverlo.
            if s.turni_di_parola.load(Ordering::SeqCst) != parole_prima
                || s.bocca_aperta.load(Ordering::SeqCst)
            {
                tracing::debug!("scartato: NOVA ha parlato mentre ascoltava");
                continue;
            }

            match esito {
                Ok(Ok(Some((testo, picco)))) => {
                    errori_di_fila = 0;
                    ultimo_attivo = std::time::Instant::now();
                    if muto {
                        muto = false;
                        bus.emit("voce.muto", json!({ "muto": false }));
                    }
                    if testo.trim().is_empty() {
                        continue;
                    }
                    // Il testo solo a «debug»: a livello normale si vede che
                    // c'e' stato del parlato, non cosa diceva.
                    tracing::info!(picco, caratteri = testo.chars().count(),
                                   fase = nome_fase(fase_ora), "parlato");
                    tracing::debug!(testo = %testo, "trascritto");

                    match fase_ora {
                        SVEGLIA => {
                            // Conversazione aperta: tutto va al cervello, il
                            // nome non serve piu'.
                            bus.emit("voce.comando", json!({ "testo": testo, "primo": false }));
                        }
                        _ => match crate::caps_voce::dopo_il_risveglio(&testo, &parola) {
                            Some(comando) => {
                                tracing::info!("risvegliata");
                                imposta_fase(&bus, SVEGLIA);
                                bus.emit(
                                    "voce.risveglio",
                                    json!({
                                        "testo": testo,
                                        "comando": comando,
                                        "riprende": fase_ora == IN_PAUSA,
                                    }),
                                );
                                // Una parola, subito: chi ha chiamato deve
                                // sapere di essere stato sentito senza dover
                                // guardare lo schermo.
                                let saluto = if fase_ora == IN_PAUSA {
                                    // Anche qui una frase, non una parola: e'
                                    // la prima cosa che il sintetizzatore
                                    // legge, e su una parola sola sbaglia
                                    // lingua.
                                    "Riprendiamo pure.".to_string()
                                } else {
                                    crate::caps_voce::saluto_di_risveglio()
                                };
                                crate::caps_voce::annuncia(bus.clone(), &saluto).await;
                                // Se dopo il nome c'era gia' altro, e' gia' un
                                // comando: «Nova, che ore sono» non deve
                                // costringere a ripetere la domanda.
                                if !comando.trim().is_empty() {
                                    bus.emit(
                                        "voce.comando",
                                        json!({ "testo": comando, "primo": true }),
                                    );
                                }
                            }
                            None => tracing::debug!("non diretto a NOVA, scartato"),
                        },
                    }
                }
                Ok(Ok(None)) => {
                    errori_di_fila = 0;
                    if muto {
                        // Il braccetto e' stato abbassato: si riprende senza
                        // che nessuno debba riavviare niente.
                        muto = false;
                        tracing::info!("il microfono e' tornato a sentire");
                        bus.emit("voce.muto", json!({ "muto": false }));
                    }
                }
                Ok(Err(e)) => {
                    if let Some(m) = e.downcast_ref::<nova_voce::MicrofonoMuto>() {
                        // Non conta come errore: spegnere il risveglio qui
                        // vorrebbe dire che chi abbassa il braccetto e chiama
                        // NOVA non ottiene niente, e non puo' sapere perche'.
                        if !muto {
                            muto = true;
                            tracing::warn!(errore = %m, "il microfono non consegna niente");
                            bus.emit(
                                "voce.muto",
                                json!({ "muto": true, "microfono": m.microfono,
                                        "motivo": format!("{m}") }),
                            );
                        }
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                        continue;
                    }
                    errori_di_fila += 1;
                    tracing::warn!(errore = %e, "ascolto fallito");
                    if errori_di_fila >= 5 {
                        bus.emit("risveglio.spento", json!({ "motivo": format!("{e}") }));
                        s.acceso.store(false, Ordering::SeqCst);
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
                Err(e) => {
                    tracing::warn!(errore = %e, "compito di ascolto interrotto");
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        }
        let s = stato();
        s.acceso.store(false, Ordering::SeqCst);
        s.fase.store(DORMIENTE, Ordering::SeqCst);
        bus.emit("risveglio.spento", json!({ "motivo": "fermato" }));
        bus.emit("stato.cambiato", json!({ "stato": "quiete" }));
        tracing::info!("risveglio spento");
    });
    true
}
