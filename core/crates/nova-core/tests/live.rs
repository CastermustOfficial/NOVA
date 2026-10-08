//! Gemini Live, contro un server finto (D386).
//!
//! Il server parla il protocollo vero (websocket, i messaggi di
//! `BidiGenerateContent`) e segue un copione: apre la sessione, saluta, chiede
//! una cosa a NOVA, viene interrotto, avverte che sta per chiudere, e alla
//! fine riceve «chiudi». La conversazione deve fare tutto quello che serve a
//! ogni passo: mandare il microfono, suonare la voce, rispondere alla
//! funzione con quello che dice NOVA, buttare la coda, riprendere con la
//! maniglia su un collegamento nuovo, e chiudere con «fine».

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use nova_core::bus::Bus;
use nova_core::live::{conversa, scelte, Bocca, Mani, Scelte};
use nova_live::Chiusura;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::Message;

const CHIAVE: &str = "CHIAVE-SEGRETA-123";

#[derive(Default)]
struct BoccaFinta {
    campioni: AtomicUsize,
    zittita: AtomicUsize,
    /// Fino a quando ha ancora voce da suonare.
    parla_fino: Mutex<Option<std::time::Instant>>,
}

impl Bocca for BoccaFinta {
    fn suona(&self, pcm: &[i16]) {
        self.campioni.fetch_add(pcm.len(), Ordering::SeqCst);
    }
    fn zitta(&self) {
        self.zittita.fetch_add(1, Ordering::SeqCst);
    }
    fn in_coda_ms(&self) -> u64 {
        match *self.parla_fino.lock().unwrap() {
            Some(t) => t
                .saturating_duration_since(std::time::Instant::now())
                .as_millis() as u64,
            None => 0,
        }
    }
}

#[derive(Default)]
struct ManiFinte {
    chieste: Mutex<Vec<String>>,
    fermate: AtomicUsize,
}

#[async_trait]
impl Mani for ManiFinte {
    async fn chiedi(&self, richiesta: &str) -> Result<String, String> {
        self.chieste.lock().unwrap().push(richiesta.to_string());
        // Un turno di NOVA non e' istantaneo: intanto la voce continua.
        tokio::time::sleep(Duration::from_millis(200)).await;
        Ok("Nella cartella ci sono tre file.".into())
    }
    fn ferma(&self) -> usize {
        self.fermate.fetch_add(1, Ordering::SeqCst);
        2
    }
}

fn le_scelte() -> Scelte {
    scelte(
        &json!({ "voice": { "conversazione": "gemini_live", "live_api_key": CHIAVE,
                            "live_voce": "Puck" },
                 "ui": { "lingua": "it" } }),
        &|_| None,
    )
}

/// Quello che il server finto ha visto, per i controlli alla fine.
#[derive(Default, Debug)]
struct Visto {
    indirizzi: Vec<String>,
    aperture: Vec<Value>,
    testi: Vec<Value>,
    pezzi_di_microfono: usize,
    risposte: Vec<Value>,
}

type Ws = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;

async fn accetta(ascolto: &TcpListener, visto: &Arc<Mutex<Visto>>) -> Ws {
    let (tcp, _) = ascolto.accept().await.unwrap();
    let v = visto.clone();
    tokio_tungstenite::accept_hdr_async(tcp, move |req: &Request, resp: Response| {
        v.lock().unwrap().indirizzi.push(req.uri().to_string());
        Ok(resp)
    })
    .await
    .unwrap()
}

/// Il prossimo messaggio del client che non e' microfono; il microfono lo
/// conta e basta.
async fn prossimo(ws: &mut Ws, visto: &Arc<Mutex<Visto>>) -> Value {
    loop {
        let m = tokio::time::timeout(Duration::from_secs(10), ws.next())
            .await
            .expect("il client tace da dieci secondi")
            .expect("il client ha chiuso")
            .unwrap();
        let Message::Text(t) = m else { continue };
        let v: Value = serde_json::from_str(&t).unwrap();
        if v.get("realtimeInput").is_some() {
            visto.lock().unwrap().pezzi_di_microfono += 1;
            continue;
        }
        return v;
    }
}

async fn manda(ws: &mut Ws, v: Value) {
    ws.send(Message::Text(v.to_string().into())).await.unwrap();
}

/// Voce finta: 2400 campioni (100 ms a 24 kHz) di PCM in base64.
fn voce() -> Value {
    // Il base64 lo fa la stessa funzione che codifica il microfono.
    let dati = nova_live::ingresso_audio(&[257i16; 2400])["realtimeInput"]["audio"]["data"].clone();
    json!({ "serverContent": { "modelTurn": { "parts": [
        { "inlineData": { "mimeType": "audio/pcm;rate=24000", "data": dati } }
    ] } } })
}

async fn copione(ascolto: TcpListener, visto: Arc<Mutex<Visto>>) {
    // --- primo collegamento ---
    let mut ws = accetta(&ascolto, &visto).await;
    let apertura = prossimo(&mut ws, &visto).await;
    visto.lock().unwrap().aperture.push(apertura);
    manda(&mut ws, json!({ "setupComplete": {} })).await;
    // Il primo testo: l'utente ha detto solo il nome, il modello saluta.
    let t = prossimo(&mut ws, &visto).await;
    visto.lock().unwrap().testi.push(t);
    manda(
        &mut ws,
        json!({ "sessionResumptionUpdate": { "resumable": true, "newHandle": "maniglia-1" } }),
    )
    .await;
    manda(&mut ws, voce()).await;
    manda(
        &mut ws,
        json!({ "serverContent": { "outputTranscription": { "text": "Ciao, " } } }),
    )
    .await;
    manda(
        &mut ws,
        json!({ "serverContent": { "outputTranscription": { "text": "dimmi." } } }),
    )
    .await;
    manda(
        &mut ws,
        json!({ "serverContent": { "turnComplete": true } }),
    )
    .await;
    // Si aspetta un po' di microfono: deve arrivare da solo.
    tokio::time::sleep(Duration::from_millis(400)).await;
    // L'utente chiede una cosa del PC: il modello la passa a NOVA.
    manda(
        &mut ws,
        json!({ "serverContent": { "inputTranscription": { "text": "che file ci sono?" } } }),
    )
    .await;
    manda(&mut ws, json!({ "toolCall": { "functionCalls": [
        { "id": "c1", "name": "chiedi_a_nova", "args": { "richiesta": "Che file ci sono nella cartella Progetti?" } }
    ] } })).await;
    manda(&mut ws, voce()).await;
    manda(&mut ws, json!({ "serverContent": { "interrupted": true } })).await;
    let r = prossimo(&mut ws, &visto).await;
    visto.lock().unwrap().risposte.push(r);
    // Il server sta per chiudere: il client deve passare a un altro
    // collegamento con la maniglia.
    manda(&mut ws, json!({ "goAway": { "timeLeft": "5s" } })).await;

    // --- secondo collegamento, ripreso ---
    let mut ws2 = accetta(&ascolto, &visto).await;
    let apertura = prossimo(&mut ws2, &visto).await;
    visto.lock().unwrap().aperture.push(apertura);
    manda(&mut ws2, json!({ "setupComplete": {} })).await;
    manda(
        &mut ws2,
        json!({ "toolCall": { "functionCalls": [
        { "id": "c2", "name": "ferma_tutto", "args": {} }
    ] } }),
    )
    .await;
    let r = prossimo(&mut ws2, &visto).await;
    visto.lock().unwrap().risposte.push(r);
    manda(
        &mut ws2,
        json!({ "toolCall": { "functionCalls": [
        { "id": "c3", "name": "chiudi_conversazione", "args": { "come": "fine" } }
    ] } }),
    )
    .await;
    let r = prossimo(&mut ws2, &visto).await;
    visto.lock().unwrap().risposte.push(r);
    manda(&mut ws2, voce()).await;
    // Il client chiude da se': si legge fino alla fine.
    while let Ok(Some(Ok(m))) = tokio::time::timeout(Duration::from_secs(10), ws2.next()).await {
        if matches!(m, Message::Close(_)) {
            break;
        }
    }
}

/// Un microfono finto: un pezzo di silenzio ogni 100 ms, finche' qualcuno
/// ascolta.
fn microfono() -> mpsc::Receiver<Vec<i16>> {
    let (tx, rx) = mpsc::channel(64);
    tokio::spawn(async move {
        while tx.send(vec![0i16; 1600]).await.is_ok() {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });
    rx
}

#[tokio::test]
async fn una_conversazione_intera_contro_il_server_finto() {
    let ascolto = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let porta = ascolto.local_addr().unwrap().port();
    // Una variabile sola per tutto il file: questa prova e' l'unica che la
    // tocca, e le due parti girano una dopo l'altra.
    std::env::set_var(
        "NOVA_LIVE_INDIRIZZO",
        format!("ws://127.0.0.1:{porta}/live"),
    );

    let visto = Arc::new(Mutex::new(Visto::default()));
    let server = tokio::spawn(copione(ascolto, visto.clone()));
    let bus = Bus::new();
    let mut eventi = bus.subscribe();
    let bocca = Arc::new(BoccaFinta::default());
    let mani = Arc::new(ManiFinte::default());
    let mut aperta = false;
    let esito = tokio::time::timeout(
        Duration::from_secs(30),
        conversa(
            &le_scelte(),
            microfono(),
            bocca.clone(),
            mani.clone(),
            bus.clone(),
            None,
            false,
            &mut aperta,
        ),
    )
    .await
    .expect("la conversazione non e' finita entro 30 secondi");
    server.await.unwrap();

    assert_eq!(
        esito.unwrap(),
        Chiusura::Fine,
        "«chiudi» con «fine» chiude con fine"
    );
    assert!(aperta);
    let v = visto.lock().unwrap();

    // La chiave va nell'indirizzo, e solo li'.
    assert_eq!(v.indirizzi.len(), 2, "{:?}", v.indirizzi);
    assert!(
        v.indirizzi[0].ends_with(&format!("/live?key={CHIAVE}")),
        "{:?}",
        v.indirizzi
    );

    // L'apertura porta la voce scelta, il modello, le tre funzioni, e la
    // seconda la maniglia del server.
    let a = &v.aperture[0]["setup"];
    assert_eq!(
        a["generationConfig"]["speechConfig"]["voiceConfig"]["prebuiltVoiceConfig"]["voiceName"],
        "Puck"
    );
    assert_eq!(a["model"], format!("models/{}", nova_live::MODELLO_LIVE));
    let funzioni: Vec<&str> = a["tools"][0]["functionDeclarations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        funzioni,
        ["chiedi_a_nova", "chiudi_conversazione", "ferma_tutto"]
    );
    assert!(a["sessionResumption"].get("handle").is_none());
    assert_eq!(
        v.aperture[1]["setup"]["sessionResumption"]["handle"],
        "maniglia-1"
    );
    assert!(
        !v.aperture[0].to_string().contains(CHIAVE),
        "la chiave non va nel messaggio"
    );

    // Detto solo il nome: il primo testo chiede un saluto, e non e' una
    // frase dell'utente.
    let primo = v.testi[0]["clientContent"]["turns"][0]["parts"][0]["text"]
        .as_str()
        .unwrap();
    assert_eq!(primo, nova_live::appena_chiamata(false));

    // Il microfono e' arrivato al server.
    assert!(v.pezzi_di_microfono >= 2, "pezzi: {}", v.pezzi_di_microfono);

    // La funzione ha avuto la risposta di NOVA, con il suo id e senza fermare
    // la voce.
    let r = &v.risposte[0]["toolResponse"]["functionResponses"][0];
    assert_eq!(
        (r["id"].as_str(), r["name"].as_str()),
        (Some("c1"), Some("chiedi_a_nova"))
    );
    assert_eq!(
        r["response"]["risposta"],
        "Nella cartella ci sono tre file."
    );
    assert_eq!(r["scheduling"], "WHEN_IDLE");
    assert_eq!(
        mani.chieste.lock().unwrap().as_slice(),
        ["Che file ci sono nella cartella Progetti?"]
    );
    // «ferma tutto» ferma davvero, e lo dice.
    let r = &v.risposte[1]["toolResponse"]["functionResponses"][0];
    assert_eq!(
        (r["id"].as_str(), r["response"]["fermate"].as_u64()),
        (Some("c2"), Some(2))
    );
    assert_eq!(mani.fermate.load(Ordering::SeqCst), 1);
    let r = &v.risposte[2]["toolResponse"]["functionResponses"][0];
    assert_eq!(
        (r["id"].as_str(), r["response"]["ok"].as_bool()),
        (Some("c3"), Some(true))
    );

    // La voce e' stata suonata (tre pezzi da 2400), e l'interruzione ha
    // buttato la coda.
    assert_eq!(bocca.campioni.load(Ordering::SeqCst), 3 * 2400);
    assert_eq!(bocca.zittita.load(Ordering::SeqCst), 1);

    // Le trascrizioni sono diventate un evento per la chat, a turno finito.
    let mut turni = Vec::new();
    let mut chiesto = Vec::new();
    while let Ok(e) = eventi.try_recv() {
        match e.topic.as_str() {
            "voce.live.turno" => turni.push(e.data),
            "voce.live.chiede" => chiesto.push(e.data),
            _ => {}
        }
    }
    assert_eq!(turni, vec![json!({ "utente": "", "nova": "Ciao, dimmi." })]);
    assert_eq!(
        chiesto,
        vec![json!({ "richiesta": "Che file ci sono nella cartella Progetti?" })]
    );
    drop(v);

    // --- seconda parte: un server che rifiuta prima di aprire ---
    let ascolto = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let porta = ascolto.local_addr().unwrap().port();
    std::env::set_var(
        "NOVA_LIVE_INDIRIZZO",
        format!("ws://127.0.0.1:{porta}/live"),
    );
    let rifiuta = tokio::spawn(async move {
        let (tcp, _) = ascolto.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
        let _ = ws.next().await;
        let _ = ws
            .send(Message::Text(
                json!({ "error": { "message": "API key not valid" } })
                    .to_string()
                    .into(),
            ))
            .await;
    });
    let mut aperta = false;
    let esito = conversa(
        &le_scelte(),
        microfono(),
        Arc::new(BoccaFinta::default()),
        Arc::new(ManiFinte::default()),
        Bus::new(),
        Some("che ore sono".into()),
        false,
        &mut aperta,
    )
    .await;
    rifiuta.await.unwrap();
    let errore = esito.unwrap_err().to_string();
    assert!(!aperta, "l'errore e' arrivato prima di aprire");
    assert!(errore.contains("API key not valid"), "{errore}");
    assert!(
        !errore.contains(CHIAVE),
        "la chiave non esce in un errore: {errore}"
    );

    // --- terza parte: «Nova, che ore sono», e poi una pausa ---
    // La frase detta col nome va al modello e nella chat come dell'utente.
    let ascolto = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let porta = ascolto.local_addr().unwrap().port();
    std::env::set_var(
        "NOVA_LIVE_INDIRIZZO",
        format!("ws://127.0.0.1:{porta}/live"),
    );
    let visto = Arc::new(Mutex::new(Visto::default()));
    let v2 = visto.clone();
    let breve = tokio::spawn(async move {
        let mut ws = accetta(&ascolto, &v2).await;
        let _ = prossimo(&mut ws, &v2).await;
        manda(&mut ws, json!({ "setupComplete": {} })).await;
        let t = prossimo(&mut ws, &v2).await;
        v2.lock().unwrap().testi.push(t);
        manda(
            &mut ws,
            json!({ "serverContent": { "outputTranscription": { "text": "Sono le tre." } } }),
        )
        .await;
        manda(
            &mut ws,
            json!({ "serverContent": { "turnComplete": true } }),
        )
        .await;
        manda(
            &mut ws,
            json!({ "toolCall": { "functionCalls": [
            { "id": "p1", "name": "chiudi_conversazione", "args": { "come": "pausa" } }
        ] } }),
        )
        .await;
        let _ = prossimo(&mut ws, &v2).await;
        while let Ok(Some(Ok(m))) = tokio::time::timeout(Duration::from_secs(10), ws.next()).await {
            if matches!(m, Message::Close(_)) {
                break;
            }
        }
    });
    let bus = Bus::new();
    let mut eventi = bus.subscribe();
    let mut aperta = false;
    let esito = conversa(
        &le_scelte(),
        microfono(),
        Arc::new(BoccaFinta::default()),
        Arc::new(ManiFinte::default()),
        bus,
        Some("  che ore sono ".into()),
        false,
        &mut aperta,
    )
    .await;
    breve.await.unwrap();
    assert_eq!(esito.unwrap(), Chiusura::Pausa);
    let testo =
        visto.lock().unwrap().testi[0]["clientContent"]["turns"][0]["parts"][0]["text"].clone();
    assert_eq!(testo, "  che ore sono ");
    let mut turni = Vec::new();
    while let Ok(e) = eventi.try_recv() {
        if e.topic == "voce.live.turno" {
            turni.push(e.data);
        }
    }
    assert_eq!(
        turni,
        vec![json!({ "utente": "che ore sono", "nova": "Sono le tre." })]
    );

    // --- quarta parte: mentre NOVA parla il microfono non va, e riparte
    // solo 300 ms dopo che la voce e' finita di suonare (con le casse) ---
    let ascolto = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let porta = ascolto.local_addr().unwrap().port();
    std::env::set_var(
        "NOVA_LIVE_INDIRIZZO",
        format!("ws://127.0.0.1:{porta}/live"),
    );
    let arrivi = Arc::new(Mutex::new(Vec::<std::time::Instant>::new()));
    let a2 = arrivi.clone();
    let ascolta = tokio::spawn(async move {
        let visto = Arc::new(Mutex::new(Visto::default()));
        let mut ws = accetta(&ascolto, &visto).await;
        let _ = prossimo(&mut ws, &visto).await;
        manda(&mut ws, json!({ "setupComplete": {} })).await;
        let fine = std::time::Instant::now() + Duration::from_millis(2000);
        while let Ok(Some(Ok(m))) = tokio::time::timeout_at(fine.into(), ws.next()).await {
            if let Message::Text(t) = m {
                if t.contains("realtimeInput") {
                    a2.lock().unwrap().push(std::time::Instant::now());
                }
            }
        }
        manda(
            &mut ws,
            json!({ "toolCall": { "functionCalls": [
            { "id": "f1", "name": "chiudi_conversazione", "args": { "come": "fine" } }
        ] } }),
        )
        .await;
        while let Ok(Some(Ok(m))) = tokio::time::timeout(Duration::from_secs(10), ws.next()).await {
            if matches!(m, Message::Close(_)) {
                break;
            }
        }
    });
    let bocca = Arc::new(BoccaFinta::default());
    let tace_da = std::time::Instant::now() + Duration::from_millis(800);
    *bocca.parla_fino.lock().unwrap() = Some(tace_da);
    let mut aperta = false;
    let esito = conversa(
        &le_scelte(),
        microfono(),
        bocca,
        Arc::new(ManiFinte::default()),
        Bus::new(),
        None,
        false,
        &mut aperta,
    )
    .await;
    ascolta.await.unwrap();
    assert_eq!(esito.unwrap(), Chiusura::Fine);
    let arrivi = arrivi.lock().unwrap().clone();
    assert!(
        arrivi.len() >= 3,
        "il microfono e' ripartito: {}",
        arrivi.len()
    );
    // Il microfono guarda la bocca ogni 100 ms: l'ultima volta che l'ha vista
    // parlare e' al piu' 100 ms prima della fine, quindi il primo pezzo arriva
    // almeno 200 ms dopo. Senza la coda arriverebbe entro 100.
    let primo = arrivi[0].saturating_duration_since(tace_da);
    assert!(
        arrivi[0] > tace_da && primo >= Duration::from_millis(150),
        "il primo pezzo e' arrivato {:?} dopo la fine della voce",
        arrivi[0].checked_duration_since(tace_da)
    );

    // --- quinta parte: nessuno risponde all'indirizzo ---
    let libero = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let porta = libero.local_addr().unwrap().port();
    drop(libero);
    std::env::set_var(
        "NOVA_LIVE_INDIRIZZO",
        format!("ws://127.0.0.1:{porta}/live"),
    );
    let mut aperta = false;
    let errore = conversa(
        &le_scelte(),
        microfono(),
        Arc::new(BoccaFinta::default()),
        Arc::new(ManiFinte::default()),
        Bus::new(),
        None,
        false,
        &mut aperta,
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(!aperta && errore.contains("non si collega"), "{errore}");
    assert!(!errore.contains(CHIAVE), "{errore}");
}

/// Con «solo sul PC» acceso non si apre niente: la voce non esce.
#[tokio::test]
async fn solo_sul_pc_non_si_collega_nemmeno() {
    let mut s = le_scelte();
    s.solo_locale = true;
    let mut aperta = false;
    let errore = conversa(
        &s,
        mpsc::channel(1).1,
        Arc::new(BoccaFinta::default()),
        Arc::new(ManiFinte::default()),
        Bus::new(),
        None,
        false,
        &mut aperta,
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(errore.contains("solo sul PC"), "{errore}");
}
