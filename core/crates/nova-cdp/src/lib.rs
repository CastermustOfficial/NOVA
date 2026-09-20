//! Parlare al browser nella sua lingua.
//!
//! Il protocollo si chiama CDP e viaggia su una websocket. Questa cassetta
//! non sa cosa chiedere — i copioni e le regole stanno in `nova_browser`, che
//! resta puro — sa solo **come** chiedere: trovare le schede, aprire una
//! connessione, mandare una domanda e riconoscere la propria risposta in
//! mezzo a quelle che non si sono chieste.
//!
//! ## L'origine e' obbligatoria, ed e' esatta
//!
//! Da Chrome 111 la connessione al CDP viene **rifiutata** se l'origine non
//! e' una di quelle dichiarate all'avvio. NOVA il browser lo avvia da se' con
//! `--remote-allow-origins=http://127.0.0.1`, quindi quella e' l'unica
//! origine che passa — e va mandata: misurato contro un Chrome 141 avviato
//! esattamente come lo avvia NOVA,
//!
//! ```text
//! con Origin: http://127.0.0.1  -> risponde
//! senza Origin                  -> 403 Forbidden
//! ```
//!
//! Sbagliarla in una delle due direzioni non da' un errore che si capisce:
//! da' un browser che non si comanda.
//!
//! ## Perche' bloccante
//!
//! Perche' e' la forma del problema, non una scorciatoia. Il Python dice di
//! se': «questo modulo viene chiamato da processi che nascono e muoiono a
//! ogni richiesta: una connessione che sopravvive al processo non esiste».
//! Dove servono piu' domande legate fra loro c'e' la [`Sessione`], perche'
//! `objectId` e domini abilitati **vivono nella connessione**: chiedere un
//! elemento su una e usarlo su un'altra da' «Could not find object with
//! given id», ed e' il modo in cui si passa un pomeriggio a incolpare il
//! selettore.

use serde_json::{json, Value};
use std::net::TcpStream;
use std::time::{Duration, Instant};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::WebSocket;

/// L'origine che NOVA dichiara al browser quando lo avvia, e che quindi deve
/// mandare quando gli parla. Le due cose sono la stessa scelta scritta in due
/// posti, ed e' il genere di coppia che si scolla in silenzio.
pub const ORIGINE: &str = "http://127.0.0.1";

/// Quanto si aspetta una risposta, se nessuno lo dice.
pub const ATTESA_S: u64 = 20;

/// Una scheda vista dal protocollo: quel che serve per sceglierla e per
/// attaccarcisi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scheda {
    pub id: String,
    pub tipo: String,
    pub titolo: String,
    pub url: String,
    /// L'indirizzo della websocket. Senza questo non ci si parla.
    pub ws: String,
}

impl Scheda {
    /// Da una voce di `/json/list`.
    ///
    /// Una scheda **senza** `webSocketDebuggerUrl` non e' una scheda a cui si
    /// possa parlare: capita per quelle gia' occupate da un altro debugger.
    /// Torna `None` invece di costruirne una che poi non si connette.
    pub fn da(v: &Value) -> Option<Scheda> {
        let ws = v.get("webSocketDebuggerUrl")?.as_str()?;
        if ws.is_empty() {
            return None;
        }
        Some(Scheda {
            id: v.get("id").and_then(Value::as_str).unwrap_or("").to_string(),
            tipo: v.get("type").and_then(Value::as_str).unwrap_or("").to_string(),
            titolo: v.get("title").and_then(Value::as_str).unwrap_or("").to_string(),
            url: v.get("url").and_then(Value::as_str).unwrap_or("").to_string(),
            ws: ws.to_string(),
        })
    }

    /// Se e' una pagina, e non un lavoratore o un'estensione.
    pub fn e_una_pagina(&self) -> bool {
        self.tipo == "page"
    }
}

/// Le schede, lette dall'elenco che il browser pubblica.
///
/// Si legge dal testo e non dalla rete, cosi' si puo' provare: chi ha in mano
/// la connessione passa quel che ha ricevuto.
pub fn schede_da(elenco: &str) -> Result<Vec<Scheda>, String> {
    let v: Value = serde_json::from_str(elenco)
        .map_err(|e| format!("l'elenco delle schede non e' JSON: {e}"))?;
    let a = v.as_array().ok_or_else(|| {
        "l'elenco delle schede non e' una lista: forse e' un errore del browser".to_string()
    })?;
    Ok(a.iter().filter_map(Scheda::da).collect())
}

/// Una connessione aperta, per piu' domande di fila.
pub struct Sessione {
    ws: WebSocket<MaybeTlsStream<TcpStream>>,
    attesa: Duration,
    n: u64,
}

impl Sessione {
    /// Si attacca a una scheda.
    pub fn apri(scheda: &Scheda, attesa: Duration) -> Result<Sessione, String> {
        let richiesta = richiesta_di_aggancio(&scheda.ws)?;
        let (ws, _) = tungstenite::connect(richiesta).map_err(|e| motivo(&e.to_string()))?;
        Ok(Sessione { ws, attesa, n: 0 })
    }

    /// Una domanda, e la **propria** risposta.
    ///
    /// Il browser manda anche eventi che nessuno ha chiesto: si scorre finche'
    /// non torna quello con il proprio numero. Prendere il primo messaggio che
    /// arriva vorrebbe dire, ogni tanto, leggere la risposta a una domanda che
    /// non si e' fatta.
    pub fn chiama(&mut self, metodo: &str, params: Value) -> Result<Value, String> {
        self.n += 1;
        let mio = self.n;
        let domanda = json!({ "id": mio, "method": metodo, "params": params });
        self.ws
            .send(tungstenite::Message::Text(domanda.to_string().into()))
            .map_err(|e| motivo(&e.to_string()))?;

        let scadenza = Instant::now() + self.attesa;
        while Instant::now() < scadenza {
            let messaggio = self.ws.read().map_err(|e| motivo(&e.to_string()))?;
            let tungstenite::Message::Text(t) = messaggio else {
                continue;
            };
            let v: Value = match serde_json::from_str(&t) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if v.get("id").and_then(Value::as_u64) != Some(mio) {
                continue; // un evento, o la risposta a un'altra domanda
            }
            if let Some(e) = v.get("error") {
                return Err(errore_del_browser(e));
            }
            return Ok(v.get("result").cloned().unwrap_or(Value::Null));
        }
        Err(format!(
            "il browser non ha risposto a {metodo} entro {}s",
            self.attesa.as_secs()
        ))
    }

    pub fn chiudi(mut self) {
        let _ = self.ws.close(None);
    }
}

/// La richiesta di aggancio, con l'origine che serve.
fn richiesta_di_aggancio(url: &str) -> Result<tungstenite::http::Request<()>, String> {
    let senza = url.strip_prefix("ws://").unwrap_or(url);
    let ospite = senza.split('/').next().unwrap_or("127.0.0.1");
    tungstenite::http::Request::builder()
        .uri(url)
        .header("Host", ospite)
        .header("Origin", ORIGINE)
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header(
            "Sec-WebSocket-Key",
            tungstenite::handshake::client::generate_key(),
        )
        .body(())
        .map_err(|e| format!("non riesco a comporre la richiesta al browser: {e}"))
}

/// Cosa ha detto il browser quando ha detto di no.
fn errore_del_browser(e: &Value) -> String {
    e.get("message")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| e.to_string())
}

/// Un guasto della connessione, in italiano.
///
/// **Il 403 ha un nome.** E' l'unico che capita per una ragione che si puo'
/// spiegare — il browser e' stato avviato senza l'origine che NOVA gli
/// manda — e senza questa riga si legge «Http(Response { status: 403 ... })»,
/// che manda a cercare dalla parte sbagliata.
pub fn motivo(grezzo: &str) -> String {
    if grezzo.contains("403") {
        return "il browser ha rifiutato la connessione: e' stato avviato senza \
                «--remote-allow-origins=http://127.0.0.1». Chiudilo e lascia che \
                lo riapra NOVA."
            .to_string();
    }
    // Due scritture, e servono tutte e due: il sistema dice «Connection
    // refused», il `Debug` di Rust dice «ConnectionRefused». Cercarne una
    // sola funziona finche' non arriva l'altra, ed e' arrivata subito.
    let senza_spazi = grezzo.replace(' ', "").to_lowercase();
    if senza_spazi.contains("connectionrefused") {
        return "sulla porta del browser non risponde nessuno: probabilmente non \
                e' acceso."
            .to_string();
    }
    format!("il browser non risponde: {grezzo}")
}

#[cfg(test)]
mod prove {
    use super::*;

    const ELENCO: &str = r#"[
      {"id":"A1","type":"page","title":"Una pagina","url":"https://esempio.it",
       "webSocketDebuggerUrl":"ws://127.0.0.1:9222/devtools/page/A1"},
      {"id":"B2","type":"service_worker","title":"un lavoratore","url":"chrome://x",
       "webSocketDebuggerUrl":"ws://127.0.0.1:9222/devtools/worker/B2"},
      {"id":"C3","type":"page","title":"occupata da un altro debugger","url":"https://x.it"}
    ]"#;

    #[test]
    fn si_leggono_le_schede_che_hanno_un_indirizzo() {
        let s = schede_da(ELENCO).unwrap();
        // La terza non ha `webSocketDebuggerUrl`: succede quando un altro
        // debugger e' gia' attaccato. Costruirla vorrebbe dire scoprirlo al
        // momento di parlarle.
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].id, "A1");
        assert!(s[0].e_una_pagina());
        assert!(!s[1].e_una_pagina(), "un service worker non e' una pagina");
    }

    #[test]
    fn un_elenco_che_non_e_una_lista_lo_dice() {
        let e = schede_da(r#"{"errore":"qualcosa"}"#).unwrap_err();
        assert!(e.contains("non e' una lista"), "{e}");
        let e = schede_da("non sono json").unwrap_err();
        assert!(e.contains("non e' JSON"), "{e}");
    }

    #[test]
    fn lorigine_si_manda_sempre_ed_e_quella() {
        // Da Chrome 111 senza questa riga si prende 403. Misurato contro un
        // Chrome 141 avviato come lo avvia NOVA.
        let r = richiesta_di_aggancio("ws://127.0.0.1:9222/devtools/page/A1").unwrap();
        assert_eq!(r.headers().get("Origin").unwrap(), ORIGINE);
        assert_eq!(r.headers().get("Host").unwrap(), "127.0.0.1:9222");
        assert_eq!(ORIGINE, "http://127.0.0.1");
    }

    #[test]
    fn lospite_viene_dallindirizzo_non_da_una_costante() {
        // Una porta diversa e' il caso normale: il browser delle ricerche sta
        // su un'altra. Un `Host` murato li' funzionerebbe su una e non
        // sull'altra.
        let r = richiesta_di_aggancio("ws://127.0.0.1:9223/devtools/page/Z").unwrap();
        assert_eq!(r.headers().get("Host").unwrap(), "127.0.0.1:9223");
    }

    #[test]
    fn il_403_si_spiega_invece_di_mostrare_la_risposta_grezza() {
        let m = motivo("Http(Response { status: 403, version: HTTP/1.1 })");
        assert!(m.contains("remote-allow-origins"), "{m}");
        assert!(!m.contains("Response {"), "non si mostra la risposta grezza: {m}");
    }

    #[test]
    fn e_una_porta_chiusa_pure() {
        let m = motivo("Io(Os { code: 111, kind: ConnectionRefused })");
        assert!(m.contains("non e' acceso"), "{m}");
    }

    #[test]
    fn un_guasto_che_non_si_riconosce_si_riporta_intero() {
        // Meglio una frase con dentro il testo del sistema che una frase
        // generica: chi ripara ha bisogno di quello che c'era scritto.
        let m = motivo("qualcosa di mai visto");
        assert!(m.contains("qualcosa di mai visto"), "{m}");
    }

    #[test]
    fn lerrore_del_browser_e_la_sua_frase() {
        let e = json!({"code": -32000, "message": "Could not find object with given id"});
        assert_eq!(errore_del_browser(&e), "Could not find object with given id");
        // E se non c'e' un messaggio, tutto quel che c'e'.
        let senza = json!({"code": -32000});
        assert!(errore_del_browser(&senza).contains("-32000"));
    }
}
