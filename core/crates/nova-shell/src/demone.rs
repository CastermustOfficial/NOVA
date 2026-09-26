//! Il ponte fra le pagine e il demone.
//!
//! Le finestre non parlano direttamente col demone: passano di qui. Non e'
//! burocrazia — e' il punto dove si decide cosa una pagina puo' chiedere.
//! Il demone sa eseguire comandi di shell e scrivere file: esporlo tutto a
//! del JavaScript, anche il nostro, vorrebbe dire che un domani un errore in
//! una pagina diventa un problema di sistema.
//!
//! Quindi: elenco esplicito. Cio' che non e' scritto qui non si puo' chiamare.
//!
//! L'elenco vale per **le pagine**, non per il guscio: [`chiama`] lo
//! controlla, [`metodo`] no. Non e' una scappatoia — e' la stessa regola
//! guardata dal lato giusto. Chi passa da `chiama` e' del JavaScript che un
//! domani puo' rompersi; chi passa da `metodo` e' codice Rust di questo
//! binario. Se i due condividessero l'elenco, l'unico modo di far chiamare
//! `agente/turno` al guscio sarebbe aprirlo anche alle pagine.

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// Cio' che l'interfaccia ha davvero bisogno di chiedere.
const CONSENTITE: &[&str] = &[
    "daemon.status",
    "sys.info",
    "voce.stato",
    "voce.parla",
    "voce.ascolta",
    "voce.dispositivi",
    "voce.trascrivi",
    "voce.risveglio",
    "voce.fase",
    "azione.ferma",
    "azione.stato",
    "approvazione.attese",
    "approvazione.rispondi",
    // I bottoni dell'harness: guardare, accettare, buttare e provare le
    // proposte di NOVA (D339).
    "harness.proposte",
    "harness.applica",
    "harness.scarta",
    "harness.prova",
];

#[cfg(windows)]
async fn connetti(endpoint: &str) -> std::io::Result<tokio::net::windows::named_pipe::NamedPipeClient> {
    tokio::net::windows::named_pipe::ClientOptions::new().open(endpoint)
}

#[cfg(not(windows))]
async fn connetti(endpoint: &str) -> std::io::Result<tokio::net::UnixStream> {
    tokio::net::UnixStream::connect(endpoint).await
}

/// Accende il demone se non risponde, e aspetta che sia in piedi.
///
/// Un interruttore solo per tutto NOVA. Chi avvia l'orb non deve sapere che
/// dietro c'e' un secondo processo, e soprattutto non deve *ricordarselo*:
/// l'avvio automatico di Windows lancia una cosa sola, e se quella cosa non
/// tira su anche il demone, al riavvio del PC NOVA c'e' ma non sente e non
/// parla — che e' peggio di non esserci, perche' sembra rotta.
///
/// Se il demone e' gia' vivo non fa niente: due demoni sulla stessa pipe
/// sarebbero un guaio peggiore di nessun demone.
pub async fn assicura_avviato() -> Result<bool> {
    let endpoint = nova_proto::endpoint_default();
    if connetti(&endpoint).await.is_ok() {
        return Ok(false);
    }
    let exe = std::env::current_exe().context("non so dove sono")?;
    let novad = exe.with_file_name(if cfg!(windows) { "novad.exe" } else { "novad" });
    if !novad.exists() {
        return Err(anyhow!(
            "il demone non risponde e non trovo «{}» da avviare",
            novad.display()
        ));
    }

    // I flussi vanno su file: un processo staccato che scrive su handle
    // ereditati e' un processo di cui, quando qualcosa va storto, non resta
    // niente da leggere.
    let registro = crate::stato::radice().join("runtime");
    let _ = std::fs::create_dir_all(&registro);
    let apri = |nome: &str| {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(registro.join(nome))
            .map(std::process::Stdio::from)
            .unwrap_or_else(|_| std::process::Stdio::null())
    };
    crate::processo::comando(&novad.to_string_lossy())
        .stdout(apri("novad.out"))
        .stderr(apri("novad.err"))
        .stdin(std::process::Stdio::null())
        .spawn()
        .with_context(|| format!("avvio di {}", novad.display()))?;

    // Il demone apre la pipe dopo aver montato l'albero di accessibilita':
    // qualche secondo. Si aspetta lui invece di far fallire la prima cosa che
    // l'utente prova a fare.
    for _ in 0..60 {
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        if connetti(&endpoint).await.is_ok() {
            tracing::info!("demone avviato dal guscio");
            return Ok(true);
        }
    }
    Err(anyhow!("il demone e' stato avviato ma non risponde"))
}

pub async fn chiama(capacita: &str, args: Value) -> Result<Value> {
    if !CONSENTITE.contains(&capacita) {
        return Err(anyhow!(
            "«{capacita}» non e' fra le capacita' che l'interfaccia puo' chiedere"
        ));
    }
    metodo(
        "capabilities/call",
        json!({ "name": capacita, "args": args }),
        attesa_per(capacita),
    )
    .await
}

/// Quanto si aspetta una capacita' qualunque: sono cose corte, e se non
/// tornano e' perche' qualcosa si e' incastrato.
const ATTESA_CAPACITA: u64 = 120;

/// Tranne le prove di un progetto: una suite puo' metterci i suoi cinque
/// minuti (`nova_harness::prova::ATTESA_PROVE_S`), e «applica e prova» la
/// esegue due volte, prima e dopo. Un guscio che molla prima direbbe
/// «non risponde» mentre il demone sta ancora provando.
fn attesa_per(capacita: &str) -> u64 {
    match capacita {
        "harness.prova" | "harness.applica" => 2 * nova_harness::prova::ATTESA_PROVE_S + 60,
        _ => ATTESA_CAPACITA,
    }
}

/// Quanto si aspetta un turno intero. E' lo stesso quarto d'ora che dentro
/// il demone aspetta il modello (`nova_core::agente::ATTESA_RISPOSTA`): un
/// guscio che molla prima lascerebbe il turno a girare da solo, con gli
/// strumenti gia' partiti e nessuno a leggerne la risposta.
const ATTESA_TURNO: u64 = 1000;

/// Un metodo qualunque del demone, senza elenco di consentite.
///
/// La differenza con [`chiama`] non e' tecnica, e' **chi chiama**: qui ci
/// arriva solo codice Rust del guscio, che e' nostro; li' ci arriva del
/// JavaScript di una pagina, che un domani puo' rompersi o essere ingannato.
/// Per questo l'elenco sta di la' e non qui: metterlo in comune vorrebbe
/// dire o aprire le pagine ai metodi dell'agente, o chiudere all'agente le
/// cose che gli servono.
async fn metodo(nome: &str, params: Value, secondi: u64) -> Result<Value> {
    let endpoint = nova_proto::endpoint_default();
    let stream = match connetti(&endpoint).await {
        Ok(s) => s,
        Err(_) => {
            // Caduto o mai partito: si riprova una volta ad accenderlo invece
            // di restituire un errore che l'utente non sa come risolvere.
            assicura_avviato().await?;
            connetti(&endpoint)
                .await
                .map_err(|e| anyhow!("il demone non risponde ({e}). E' avviato?"))?
        }
    };
    let (lettore, mut scrittore) = tokio::io::split(stream);
    let richiesta = json!({
        "jsonrpc": "2.0", "id": 1, "method": nome, "params": params
    });
    scrittore.write_all(richiesta.to_string().as_bytes()).await?;
    scrittore.write_all(b"\n").await?;
    scrittore.flush().await?;

    let mut righe = BufReader::new(lettore).lines();
    let scadenza = std::time::Duration::from_secs(secondi);
    loop {
        // Un tetto di attesa c'e' comunque: senza, una connessione che resta
        // aperta e muta tiene il guscio appeso per sempre, e per chi guarda
        // e' identico a NOVA che sta pensando.
        let riga = match tokio::time::timeout(scadenza, righe.next_line()).await {
            Ok(r) => match r? {
                Some(l) => l,
                None => break,
            },
            Err(_) => return Err(anyhow!("il demone non ha risposto entro {secondi}s")),
        };
        let v: Value = match serde_json::from_str(&riga) {
            Ok(v) => v,
            Err(_) => continue,
        };
        // le notifiche del bus passano sulla stessa linea: si saltano
        if v.get("id").is_none() {
            continue;
        }
        if let Some(errore) = v.get("error") {
            let messaggio = errore
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("errore sconosciuto");
            return Err(anyhow!("{messaggio}"));
        }
        return Ok(v.get("result").cloned().unwrap_or(json!({})));
    }
    Err(anyhow!("nessuna risposta dal demone"))
}

/// Il demone sa fare un turno adesso?
///
/// Si chiede **prima** di mandare la domanda, e costa quanto un ping: legge
/// la configurazione e guarda com'e' fatto il primo gradino. La strada
/// alternativa — provare il turno e ripiegare se fallisce — sarebbe peggio
/// che inutile, perche' un turno che muore a meta' ha gia' eseguito degli
/// strumenti: rifarlo dall'altra parte li farebbe **due volte**.
pub async fn pronto_al_turno() -> Result<(bool, String)> {
    let r = metodo("agente/pronto", json!({}), ATTESA_CAPACITA).await?;
    Ok((
        r.get("pronto").and_then(Value::as_bool).unwrap_or(false),
        r.get("perche")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    ))
}

/// Un turno intero dentro il demone.
///
/// Gli avanzamenti non tornano da qui: viaggiano sul bus (`agente.*`) e li
/// raccoglie [`crate::bus`], che sulla sua connessione c'e' gia'. Farli
/// tornare anche di qua vorrebbe dire due strade per la stessa notizia, e
/// chi guarda l'orb vedrebbe ogni passo due volte quando un turno parte
/// dalla voce invece che dalla chat.
///
/// La voce e la chat sono **la stessa conversazione** (D307): cambia solo la
/// bandierina `voce`, che fa attaccare alla domanda la postilla di chi parla
/// al microfono. Prima qui si passava «voce» come *nome di sessione*: il
/// demone apriva una seconda conversazione per la voce, e la postilla non
/// arrivava mai — cioe' mai il marcatore con cui la voce capisce che il
/// discorso e' chiuso.
///
/// `postilla` e' il resto di cio' che va in coda alla domanda senza essere
/// detto dall'utente: oggi, cosa c'e' aperto nell'harness.
pub async fn turno(testo: &str, voce: bool, postilla: &str) -> Result<String> {
    let r = metodo("agente/turno", richiesta_turno(testo, voce, postilla), ATTESA_TURNO).await?;
    Ok(r.get("risposta")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string())
}

/// Cosa si manda al demone per un turno. Senza `sessione`: voce e chat
/// finiscono nella conversazione di sempre.
fn richiesta_turno(testo: &str, voce: bool, postilla: &str) -> Value {
    json!({ "testo": testo, "voce": voce, "postilla": postilla })
}

/// Taglia il filo del discorso dalla parte del demone.
pub async fn dimentica_sessione(sessione: &str) -> Result<()> {
    metodo(
        "agente/dimentica",
        json!({ "sessione": sessione }),
        ATTESA_CAPACITA,
    )
    .await
    .map(|_| ())
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn le_prove_si_aspettano_due_volte_e_il_resto_no() {
        assert_eq!(attesa_per("harness.applica"), 660);
        assert_eq!(attesa_per("harness.prova"), 660);
        assert_eq!(attesa_per("harness.proposte"), ATTESA_CAPACITA);
    }

    #[test]
    fn la_voce_e_una_bandierina_non_una_conversazione_a_parte() {
        let r = richiesta_turno("che ore sono", true, "");
        assert_eq!(r["voce"], true);
        assert!(r.get("sessione").is_none(), "{r}");
        let h = richiesta_turno("cosa fa?", false, "\n\n<harness>x</harness>");
        assert_eq!(h["voce"], false);
        assert_eq!(h["postilla"], "\n\n<harness>x</harness>");
        assert_eq!(h["testo"], "cosa fa?");
    }
}
