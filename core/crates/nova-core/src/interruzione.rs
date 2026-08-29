//! Fermare cio' che NOVA sta facendo.
//!
//! Non e' un freno: e' un acceleratore. Senza un modo di dire «basta», ogni
//! azione va giustificata *prima* di partire — e un agente che deve avere
//! ragione al primo colpo non prova mai niente. Con l'interruzione, tentare
//! costa poco, e questo lo rende molto piu' capace, non meno.
//!
//! «Ferma» significa **fermare l'azione in corso**, non chiudere il
//! programma. Dopo un'interruzione NOVA e' viva e ascolta.
//!
//! ```text
//!   ferma()  ->  generazione += 1  ->  campanello
//!                                          |
//!                       ogni azione in corso lo sente e molla
//! ```
//!
//! Il meccanismo e' una *generazione*, non una bandiera: una bandiera va
//! rimessa a posto, e finche' e' alzata interrompe anche cio' che parte dopo.
//! Chi comincia un'azione prende il numero corrente; se il numero cambia,
//! quell'azione e' stata interrotta. Chi parte dopo prende gia' il numero
//! nuovo e non ne risente.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::OnceLock;

use serde_json::json;
use tokio::sync::Notify;

pub struct Stato {
    generazione: AtomicU64,
    /// Quante azioni interrompibili sono in volo adesso.
    in_corso: AtomicUsize,
    /// Quante ne sono state interrotte da quando il demone e' acceso.
    interrotte: AtomicU64,
    campanello: Notify,
}

fn stato() -> &'static Stato {
    static S: OnceLock<Stato> = OnceLock::new();
    S.get_or_init(|| Stato {
        generazione: AtomicU64::new(0),
        in_corso: AtomicUsize::new(0),
        interrotte: AtomicU64::new(0),
        campanello: Notify::new(),
    })
}

/// Il biglietto che un'azione prende quando comincia.
#[derive(Debug, Clone, Copy)]
pub struct Gettone {
    nato_a: u64,
}

impl Gettone {
    pub fn annullato(&self) -> bool {
        stato().generazione.load(Ordering::SeqCst) != self.nato_a
    }

    /// Si risolve quando questa azione viene interrotta. Se lo e' gia',
    /// ritorna subito: chi arriva tardi non deve restare appeso.
    pub async fn attendi(&self) {
        loop {
            if self.annullato() {
                return;
            }
            // `notified()` va costruito PRIMA di ricontrollare, altrimenti si
            // perde il segnale che arriva nel mezzo (risveglio perduto).
            let avviso = stato().campanello.notified();
            tokio::pin!(avviso);
            avviso.as_mut().enable();
            if self.annullato() {
                return;
            }
            avviso.await;
        }
    }
}

pub fn gettone() -> Gettone {
    Gettone {
        nato_a: stato().generazione.load(Ordering::SeqCst),
    }
}

/// Ferma tutto cio' che e' in corso. Ritorna quante azioni erano in volo.
pub fn ferma(bus: &crate::bus::Bus) -> usize {
    let s = stato();
    let quante = s.in_corso.load(Ordering::SeqCst);
    s.generazione.fetch_add(1, Ordering::SeqCst);
    s.campanello.notify_waiters();
    bus.emit("azione.interrotta", json!({ "quante": quante }));
    // Un'interruzione a vuoto non e' un errore: capita di premere «ferma»
    // mentre l'ultima cosa stava gia' finendo. Ma non si annuncia come se
    // avesse fermato qualcosa.
    if quante > 0 {
        bus.emit("stato.cambiato", json!({ "stato": crate::risveglio::stato_a_riposo() }));
        tracing::info!(quante, "interrotto");
    }
    quante
}

pub fn quante_in_corso() -> usize {
    stato().in_corso.load(Ordering::SeqCst)
}

pub fn quante_interrotte() -> u64 {
    stato().interrotte.load(Ordering::SeqCst)
}

/// Esegue qualcosa rendendolo interrompibile.
///
/// Se arriva un «ferma», il futuro viene lasciato cadere e si torna con un
/// errore che lo dice. Nota onesta sui limiti: lasciar cadere un futuro non
/// ferma un lavoro gia' partito su un altro thread (`spawn_blocking`) ne'
/// uccide un processo figlio. Restituisce il controllo a chi ha chiesto, che
/// e' cio' che conta per chi sta guardando; il resto lo fanno le singole
/// capacita' che sanno cosa hanno avviato.
pub async fn interrompibile<F, T>(lavoro: F) -> anyhow::Result<T>
where
    F: std::future::Future<Output = anyhow::Result<T>>,
{
    let g = gettone();
    let s = stato();
    s.in_corso.fetch_add(1, Ordering::SeqCst);
    let esito = tokio::select! {
        biased;
        _ = g.attendi() => {
            s.interrotte.fetch_add(1, Ordering::SeqCst);
            Err(anyhow::anyhow!("interrotto su richiesta"))
        }
        r = lavoro => r,
    };
    s.in_corso.fetch_sub(1, Ordering::SeqCst);
    esito
}

#[cfg(test)]
mod prove {
    use super::*;

    fn bus_finto() -> crate::bus::Bus {
        crate::bus::Bus::new()
    }

    #[tokio::test]
    async fn cio_che_finisce_in_tempo_non_viene_toccato() {
        let esito = interrompibile(async { Ok::<_, anyhow::Error>(7) }).await;
        assert_eq!(esito.unwrap(), 7);
    }

    #[tokio::test]
    async fn un_lavoro_lungo_si_ferma() {
        let bus = bus_finto();
        let b = bus.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            ferma(&b);
        });
        let esito = interrompibile(async {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            Ok::<_, anyhow::Error>(1)
        })
        .await;
        assert!(esito.is_err(), "doveva essere interrotto");
    }

    #[tokio::test]
    async fn chi_parte_dopo_non_eredita_l_interruzione() {
        let bus = bus_finto();
        ferma(&bus);
        // Questa nasce dopo: la generazione e' gia' quella nuova.
        let esito = interrompibile(async {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            Ok::<_, anyhow::Error>(3)
        })
        .await;
        assert_eq!(esito.unwrap(), 3, "una bandiera avrebbe rotto anche questa");
    }

    #[tokio::test]
    async fn l_esito_passa_intatto() {
        // Il contatore «in corso» e' globale — c'e' un solo NOVA e un solo
        // pulsante «ferma» — e i test girano in parallelo: fra il «prima» e il
        // «dopo» un'altra prova puo' cominciare o finire. Verificarlo qui
        // produce un test che fallisce a caso, che e' peggio di non averlo.
        // Cio' che si puo' verificare davvero e' che avvolgere un lavoro non
        // ne alteri l'esito, ne' quando riesce ne' quando fallisce.
        let bene = interrompibile(async { Ok::<_, anyhow::Error>("intatto") }).await;
        assert_eq!(bene.unwrap(), "intatto");

        let male = interrompibile(async { Err::<(), _>(anyhow::anyhow!("caduta vera")) }).await;
        assert!(male.unwrap_err().to_string().contains("caduta vera"),
                "l'errore originale non deve essere sostituito da «interrotto»");
    }
}