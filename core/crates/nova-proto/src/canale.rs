//! Aprire il canale verso il demone, dalla parte di chi chiede.
//!
//! Su Windows il canale e' una named pipe, e una named pipe ha un'abitudine
//! che il socket unix non ha: dopo ogni connessione il demone deve creare
//! l'istanza per la prossima, e chi arriva in quell'istante trova «tutte le
//! istanze della pipe sono impegnate» (`ERROR_PIPE_BUSY`, 231). Windows
//! prescrive al client di aspettare e riprovare. I tre client Rust — `nova`,
//! e il guscio due volte — aprivano la pipe una volta sola, e di due
//! richieste ravvicinate la seconda falliva come se il demone fosse spento.
//! L'ha mostrato `test_demone_compiti.py` sul PC di sviluppo (GPU RTX 4060 Ti, 16 GB di VRAM; 32 GB di RAM DDR5; scheda madre Gigabyte B650 EAGLE AX; CPU Ryzen 5 7600X), dove le prove del
//! demone girano su Windows; in CI girano su Linux, e non si vedeva.
//!
//! Si riprova ogni dieci millisecondi per un secondo al massimo, come il
//! client Python (`nova/core_client.py`). Il secondo si misura con
//! l'orologio, non contando i tentativi: su Windows una pausa di dieci
//! millisecondi ne dura quasi sedici, la risoluzione del timer di sistema, e
//! cento pause facevano un secondo e mezzo (misurato: 1,56 s). Una pipe che
//! non c'e' e' un altro errore, e non si riprova: il demone e' spento, e
//! aspettare non lo accende.

use std::time::Duration;

/// `ERROR_PIPE_BUSY`: tutte le istanze della pipe sono impegnate.
pub const PIPE_OCCUPATA: i32 = 231;

/// Per quanto si riprova al massimo, e quanto si aspetta fra un tentativo e
/// l'altro.
pub const ATTESA_PIPE: Duration = Duration::from_secs(1);
pub const PAUSA_PIPE: Duration = Duration::from_millis(10);

/// Un errore per cui vale la pena riprovare: la pipe c'e', ma e' occupata.
pub fn da_riprovare(e: &std::io::Error) -> bool {
    e.raw_os_error() == Some(PIPE_OCCUPATA)
}

/// Apre il canale verso il demone, riprovando finche' la pipe e' occupata.
#[cfg(windows)]
pub async fn apri(
    endpoint: &str,
) -> std::io::Result<tokio::net::windows::named_pipe::NamedPipeClient> {
    use tokio::net::windows::named_pipe::ClientOptions;
    let inizio = std::time::Instant::now();
    loop {
        match ClientOptions::new().open(endpoint) {
            Ok(c) => return Ok(c),
            Err(e) if da_riprovare(&e) && inizio.elapsed() < ATTESA_PIPE => {
                tokio::time::sleep(PAUSA_PIPE).await;
            }
            Err(e) => return Err(e),
        }
    }
}

/// Apre il canale verso il demone. Un socket unix non ha istanze da
/// aspettare: le connessioni in arrivo fanno la fila da sole.
#[cfg(not(windows))]
pub async fn apri(endpoint: &str) -> std::io::Result<tokio::net::UnixStream> {
    tokio::net::UnixStream::connect(endpoint).await
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn si_riprova_solo_la_pipe_occupata() {
        assert!(da_riprovare(&std::io::Error::from_raw_os_error(
            PIPE_OCCUPATA
        )));
        // La pipe che non c'e' (ERROR_FILE_NOT_FOUND, 2): il demone e' spento.
        assert!(!da_riprovare(&std::io::Error::from_raw_os_error(2)));
        assert!(!da_riprovare(&std::io::Error::other("altro")));
    }

    /// Una sola istanza, gia' presa: il client trova la pipe occupata, e
    /// quando il server crea la seconda istanza entra. Senza la riprova,
    /// `open` torna 231 subito.
    #[cfg(windows)]
    #[tokio::test]
    async fn una_pipe_occupata_si_aspetta() {
        use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};
        let nome = format!(r"\\.\pipe\nova-prova-occupata-{}", std::process::id());
        let primo = ServerOptions::new()
            .first_pipe_instance(true)
            .create(&nome)
            .unwrap();
        let _preso = ClientOptions::new().open(&nome).unwrap();
        primo.connect().await.unwrap();
        // Adesso l'unica istanza e' presa.
        let subito = ClientOptions::new().open(&nome).unwrap_err();
        assert!(da_riprovare(&subito), "{subito:?}");
        let nome2 = nome.clone();
        let dopo = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let s = ServerOptions::new().create(&nome2).unwrap();
            s.connect().await.unwrap();
            s
        });
        let c = apri(&nome).await;
        assert!(c.is_ok(), "{:?}", c.err());
        let _ = dopo.await.unwrap();
    }

    /// Una pipe sempre occupata si aspetta un secondo, non di piu'. Contando
    /// cento pause da dieci millisecondi si aspettava 1,56 s: su Windows una
    /// pausa ne dura quasi sedici.
    #[cfg(windows)]
    #[tokio::test]
    async fn una_pipe_sempre_occupata_si_aspetta_un_secondo() {
        use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};
        let nome = format!(
            r"\\.\pipe\nova-prova-sempre-occupata-{}",
            std::process::id()
        );
        let server = ServerOptions::new()
            .first_pipe_instance(true)
            .create(&nome)
            .unwrap();
        let _preso = ClientOptions::new().open(&nome).unwrap();
        server.connect().await.unwrap();
        let inizio = std::time::Instant::now();
        let e = apri(&nome).await.unwrap_err();
        let durata = inizio.elapsed();
        assert!(da_riprovare(&e), "{e:?}");
        assert!(durata >= ATTESA_PIPE, "{durata:?}");
        assert!(durata < Duration::from_millis(1400), "{durata:?}");
    }

    /// Una pipe che non esiste non si aspetta: si risponde subito.
    #[cfg(windows)]
    #[tokio::test]
    async fn una_pipe_che_non_c_e_non_si_aspetta() {
        let inizio = std::time::Instant::now();
        let e = apri(r"\\.\pipe\nova-questa-pipe-non-esiste")
            .await
            .unwrap_err();
        assert!(!da_riprovare(&e));
        assert!(
            inizio.elapsed() < Duration::from_millis(500),
            "{:?}",
            inizio.elapsed()
        );
    }
}
