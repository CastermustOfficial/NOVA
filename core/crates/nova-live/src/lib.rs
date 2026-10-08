//! Gemini Live: la voce dal vivo di NOVA, il protocollo senza rete (D386).
//!
//! Nella conversazione classica la voce fa tre giri: il microfono diventa
//! testo (whisper), il testo va al cervello, la risposta torna voce
//! (Kokoro). I tempi morti e le interruzioni vengono da li'. Gemini Live
//! ascolta e parla da solo, in tempo reale: tiene la conversazione, e quando
//! serve fare qualcosa sul PC passa il lavoro a NOVA con una funzione
//! (`idea/`, «Una voce che parla davvero», provato il 7 ottobre).
//!
//! Qui ci sono i messaggi e le regole, tutti provabili senza rete:
//!
//! - il messaggio d'apertura ([`setup`]): il modello, la voce, le istruzioni,
//!   le funzioni di NOVA, la trascrizione nei due versi, la compressione del
//!   contesto e la ripresa della sessione;
//! - il microfono e la voce in PCM a 16 bit ([`ingresso_audio`],
//!   [`audio_da`]): 16 kHz in entrata, 24 kHz in uscita;
//! - cosa dice il server ([`leggi`]), e cosa gli si risponde quando chiama
//!   una funzione ([`risposta_funzione`]);
//! - le voci che si possono scegliere ([`VOCI`]).
//!
//! La websocket, il microfono e l'altoparlante stanno in `nova_core::live` e
//! in `nova_voce::dal_vivo`.
//!
//! La forma dei messaggi e' quella della Live API
//! (`ai.google.dev/api/live`, letta l'8 ottobre 2026); le funzioni non
//! bloccanti e la loro risposta «quando e' libero» sono quelle provate il 7
//! ottobre su `gemini-3.8-live`.

use base64::Engine;
use serde_json::{json, Value};

/// L'indirizzo della Live API. La chiave va in coda, come `?key=`: chi
/// scrive l'indirizzo in un log scrive la chiave, quindi non lo fa nessuno.
pub const INDIRIZZO: &str = "wss://generativelanguage.googleapis.com/ws/\
google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerateContent";

/// Il modello di serie: quello provato il 7 ottobre, stabile nell'elenco dei
/// modelli l'8 ottobre.
pub const MODELLO_LIVE: &str = "gemini-3.8-live";

/// La voce di serie. La Live API ne offre trenta (le stesse della sintesi
/// vocale di Gemini); questa e' quella degli esempi della documentazione.
pub const VOCE_PREDEFINITA: &str = "Kore";

/// Il microfono va mandato a 16 kHz.
pub const FREQUENZA_INGRESSO: u32 = 16_000;

/// La voce torna a 24 kHz.
pub const FREQUENZA_USCITA: u32 = 24_000;

/// Quanti millisecondi di microfono per messaggio.
pub const MS_PER_PEZZO: u32 = 100;

/// Le voci, col carattere che dice la documentazione: il nome, in italiano,
/// in inglese.
pub const VOCI: [(&str, &str, &str); 30] = [
    ("Zephyr", "luminosa", "bright"),
    ("Puck", "vivace", "upbeat"),
    ("Charon", "informativa", "informative"),
    ("Kore", "decisa", "firm"),
    ("Fenrir", "eccitabile", "excitable"),
    ("Leda", "giovanile", "youthful"),
    ("Orus", "decisa", "firm"),
    ("Aoede", "leggera", "breezy"),
    ("Callirrhoe", "rilassata", "easy-going"),
    ("Autonoe", "luminosa", "bright"),
    ("Enceladus", "soffiata", "breathy"),
    ("Iapetus", "chiara", "clear"),
    ("Umbriel", "rilassata", "easy-going"),
    ("Algieba", "morbida", "smooth"),
    ("Despina", "morbida", "smooth"),
    ("Erinome", "chiara", "clear"),
    ("Algenib", "roca", "gravelly"),
    ("Rasalgethi", "informativa", "informative"),
    ("Laomedeia", "vivace", "upbeat"),
    ("Achernar", "dolce", "soft"),
    ("Alnilam", "decisa", "firm"),
    ("Schedar", "uniforme", "even"),
    ("Gacrux", "matura", "mature"),
    ("Pulcherrima", "diretta", "forward"),
    ("Achird", "amichevole", "friendly"),
    ("Zubenelgenubi", "informale", "casual"),
    ("Vindemiatrix", "gentile", "gentle"),
    ("Sadachbia", "animata", "lively"),
    ("Sadaltager", "esperta", "knowledgeable"),
    ("Sulafat", "calda", "warm"),
];

/// La voce scelta, se e' una di quelle che ci sono; se no quella di serie.
/// Le maiuscole non contano: «kore» e' «Kore».
pub fn voce_valida(scelta: &str) -> &'static str {
    let s = scelta.trim();
    VOCI.iter()
        .find(|(n, _, _)| n.eq_ignore_ascii_case(s))
        .map(|(n, _, _)| *n)
        .unwrap_or(VOCE_PREDEFINITA)
}

/// Il nome di una lingua, per le istruzioni. I modelli audio nativi scelgono
/// la lingua da soli e non accettano un codice: gliela si dice a parole.
pub fn nome_lingua(codice: &str) -> &'static str {
    match codice.trim().to_lowercase().as_str() {
        "en" => "inglese",
        "es" => "spagnolo",
        "fr" => "francese",
        "de" => "tedesco",
        "pt" => "portoghese",
        _ => "italiano",
    }
}

/// Le istruzioni di sistema della voce.
pub fn istruzioni(lingua: &str) -> String {
    format!(
        "Sei Nova, l'assistente che vive nel PC dell'utente e gli parla a voce. Rispondi in \
         {}, breve e naturale, come in una conversazione. Per tutto quello che riguarda il PC \
         dell'utente, i suoi file, i programmi, la sua memoria, le ricerche, i documenti, i Dot \
         o un lavoro da fare, chiama {CHIEDI_A_NOVA} con la richiesta scritta per intero: NOVA \
         lo fa con i suoi strumenti. Mentre aspetti, di' in poche parole che ci stai lavorando; \
         quando arriva la risposta, raccontala in breve. Non inventare quello che non sai del PC \
         o della vita dell'utente: chiedilo a NOVA. Quando l'utente saluta o dice che ha finito, \
         chiama {CHIUDI} con «fine»; se dice di aspettare o che ti richiamera', con «pausa». Se \
         l'utente ti chiede di fermare quello che NOVA sta facendo, chiama {FERMA}.",
        nome_lingua(lingua),
    )
}

/// Cosa si scrive al modello appena aperta la sessione, quando dopo il nome
/// l'utente non ha detto altro: cosi' risponde subito con la sua voce, e chi
/// ha chiamato sa di essere stato sentito. Non e' una frase dell'utente, e
/// nella chat non compare.
pub fn appena_chiamata(riprende: bool) -> String {
    if riprende {
        "L'utente ti richiama dopo una pausa: digli in poche parole che riprendete, e ascolta."
            .to_string()
    } else {
        "L'utente ti ha appena chiamato per nome: rispondi con un saluto di poche parole, e \
         ascolta."
            .to_string()
    }
}

/// La funzione che passa il lavoro a NOVA.
pub const CHIEDI_A_NOVA: &str = "chiedi_a_nova";
/// La funzione che chiude la conversazione.
pub const CHIUDI: &str = "chiudi_conversazione";
/// La funzione che ferma quello che NOVA sta facendo.
pub const FERMA: &str = "ferma_tutto";

/// Le funzioni di NOVA, come le vede il modello vocale.
///
/// `chiedi_a_nova` **non blocca**: il modello continua a parlare mentre NOVA
/// lavora, e racconta la risposta quando arriva (provato il 7 ottobre).
pub fn funzioni() -> Value {
    json!([
        {
            "name": CHIEDI_A_NOVA,
            "description": "Passa a NOVA quello che c'e' da fare o da sapere sul PC dell'utente: \
                file, programmi, memoria, ricerche in rete, documenti, i Dot, qualunque lavoro. \
                Torna la risposta di NOVA.",
            "behavior": "NON_BLOCKING",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "richiesta": {
                        "type": "STRING",
                        "description": "La richiesta, scritta per intero, con tutto quello che \
                            ha detto l'utente e che serve"
                    }
                },
                "required": ["richiesta"]
            }
        },
        {
            "name": CHIUDI,
            "description": "Chiude la conversazione a voce: «fine» quando l'utente ha finito, \
                «pausa» quando ti richiamera'.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "come": { "type": "STRING", "enum": ["fine", "pausa"] }
                },
                "required": ["come"]
            }
        },
        {
            "name": FERMA,
            "description": "Ferma subito quello che NOVA sta facendo sul PC.",
            "parameters": { "type": "OBJECT", "properties": {} }
        }
    ])
}

/// Cosa serve per aprire una sessione.
#[derive(Debug, Clone, PartialEq)]
pub struct Apertura {
    pub modello: String,
    pub voce: String,
    pub istruzioni: String,
    pub funzioni: Value,
    /// L'ultima maniglia di ripresa ricevuta, per riprendere la stessa
    /// conversazione su un collegamento nuovo.
    pub ripresa: Option<String>,
}

/// Il nome del modello come lo vuole il server: `models/...`.
pub fn nome_modello(modello: &str) -> String {
    let m = modello.trim();
    let m = if m.is_empty() { MODELLO_LIVE } else { m };
    if m.starts_with("models/") {
        m.to_string()
    } else {
        format!("models/{m}")
    }
}

/// Il primo messaggio: si manda una volta, e si aspetta `setupComplete`.
///
/// La compressione del contesto e' accesa: senza, una conversazione solo
/// audio si ferma a 15 minuti. La ripresa pure: il collegamento dura circa
/// 10 minuti, e con la maniglia la conversazione continua su quello dopo.
pub fn setup(a: &Apertura) -> Value {
    let mut ripresa = json!({});
    if let Some(h) = a.ripresa.as_deref().filter(|h| !h.is_empty()) {
        ripresa["handle"] = json!(h);
    }
    json!({
        "setup": {
            "model": nome_modello(&a.modello),
            "generationConfig": {
                "responseModalities": ["AUDIO"],
                "speechConfig": {
                    "voiceConfig": { "prebuiltVoiceConfig": { "voiceName": voce_valida(&a.voce) } }
                }
            },
            "systemInstruction": { "parts": [{ "text": a.istruzioni }] },
            "tools": [{ "functionDeclarations": a.funzioni }],
            "inputAudioTranscription": {},
            "outputAudioTranscription": {},
            "contextWindowCompression": { "slidingWindow": {} },
            "sessionResumption": ripresa
        }
    })
}

/// Da campioni in virgola a PCM a 16 bit.
pub fn in_pcm16(campioni: &[f32]) -> Vec<i16> {
    campioni
        .iter()
        .map(|c| (c.clamp(-1.0, 1.0) * 32767.0).round() as i16)
        .collect()
}

/// Un pezzo di microfono, gia' a 16 kHz.
pub fn ingresso_audio(pcm: &[i16]) -> Value {
    let mut byte = Vec::with_capacity(pcm.len() * 2);
    for c in pcm {
        byte.extend_from_slice(&c.to_le_bytes());
    }
    json!({
        "realtimeInput": {
            "audio": {
                "data": base64::engine::general_purpose::STANDARD.encode(&byte),
                "mimeType": format!("audio/pcm;rate={FREQUENZA_INGRESSO}")
            }
        }
    })
}

/// Una frase scritta, come se l'avesse detta l'utente: serve a mandare cio'
/// che e' stato detto insieme al nome («Nova, che ore sono»), e alla prova
/// delle voci.
pub fn ingresso_testo(testo: &str) -> Value {
    json!({
        "clientContent": {
            "turns": [{ "role": "user", "parts": [{ "text": testo }] }],
            "turnComplete": true
        }
    })
}

/// I campioni di un pezzo di voce: PCM a 16 bit, little-endian, in base64.
/// Un byte dispari in fondo si butta.
pub fn audio_da(b64: &str) -> Result<Vec<i16>, String> {
    let byte = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .map_err(|e| format!("l'audio non e' base64: {e}"))?;
    Ok(byte
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect())
}

/// Una funzione chiamata dal modello.
#[derive(Debug, Clone, PartialEq)]
pub struct Chiamata {
    pub id: String,
    pub nome: String,
    pub argomenti: Value,
}

/// Cosa dice il server, un pezzo alla volta. Un messaggio solo puo' portarne
/// piu' d'uno: della voce, la trascrizione e la fine del turno.
#[derive(Debug, Clone, PartialEq)]
pub enum DalServer {
    /// La sessione e' aperta.
    Pronto,
    /// Un pezzo di voce, a 24 kHz.
    Voce(Vec<i16>),
    /// Cosa ha sentito dell'utente: un pezzo di trascrizione.
    Sentito(String),
    /// Cosa sta dicendo: un pezzo di trascrizione.
    Detto(String),
    /// L'utente ha parlato sopra: la voce in coda va buttata.
    Interrotto,
    /// Il modello ha finito di parlare, e tocca all'utente.
    TurnoFinito,
    Chiamate(Vec<Chiamata>),
    /// Funzioni chiamate prima e da lasciar perdere.
    Annullate(Vec<String>),
    /// Il collegamento sta per chiudersi, fra quanto lo dice il server.
    StaPerChiudere(String),
    /// La maniglia per riprendere la conversazione su un collegamento nuovo.
    Ripresa(String),
    /// Un errore detto dal server.
    Errore(String),
}

/// Legge un messaggio del server. Un messaggio che non si legge e' un errore
/// detto, non un silenzio.
pub fn leggi(testo: &str) -> Vec<DalServer> {
    let v: Value = match serde_json::from_str(testo) {
        Ok(v) => v,
        Err(e) => return vec![DalServer::Errore(format!("messaggio illeggibile: {e}"))],
    };
    let mut fuori = Vec::new();
    if v.get("setupComplete").is_some() {
        fuori.push(DalServer::Pronto);
    }
    if let Some(sc) = v.get("serverContent") {
        if let Some(parti) = sc
            .get("modelTurn")
            .and_then(|t| t.get("parts"))
            .and_then(Value::as_array)
        {
            for p in parti {
                let Some(d) = p.get("inlineData") else {
                    continue;
                };
                let mime = d.get("mimeType").and_then(Value::as_str).unwrap_or("");
                if !mime.starts_with("audio/") {
                    continue;
                }
                match audio_da(d.get("data").and_then(Value::as_str).unwrap_or("")) {
                    Ok(pcm) if !pcm.is_empty() => fuori.push(DalServer::Voce(pcm)),
                    Ok(_) => {}
                    Err(e) => fuori.push(DalServer::Errore(e)),
                }
            }
        }
        let trascritto = |k: &str| {
            sc.get(k)
                .and_then(|t| t.get("text"))
                .and_then(Value::as_str)
                .filter(|t| !t.is_empty())
                .map(str::to_string)
        };
        if let Some(t) = trascritto("inputTranscription") {
            fuori.push(DalServer::Sentito(t));
        }
        if let Some(t) = trascritto("outputTranscription") {
            fuori.push(DalServer::Detto(t));
        }
        if sc.get("interrupted").and_then(Value::as_bool) == Some(true) {
            fuori.push(DalServer::Interrotto);
        }
        if sc.get("turnComplete").and_then(Value::as_bool) == Some(true) {
            fuori.push(DalServer::TurnoFinito);
        }
    }
    if let Some(tc) = v.get("toolCall") {
        let chiamate: Vec<Chiamata> = tc
            .get("functionCalls")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|c| Chiamata {
                        id: c
                            .get("id")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        nome: c
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        argomenti: c.get("args").cloned().unwrap_or_else(|| json!({})),
                    })
                    .collect()
            })
            .unwrap_or_default();
        if !chiamate.is_empty() {
            fuori.push(DalServer::Chiamate(chiamate));
        }
    }
    if let Some(ids) = v
        .get("toolCallCancellation")
        .and_then(|t| t.get("ids"))
        .and_then(Value::as_array)
    {
        fuori.push(DalServer::Annullate(
            ids.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect(),
        ));
    }
    if let Some(g) = v.get("goAway") {
        fuori.push(DalServer::StaPerChiudere(
            g.get("timeLeft")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        ));
    }
    if let Some(r) = v.get("sessionResumptionUpdate") {
        if r.get("resumable").and_then(Value::as_bool) == Some(true) {
            if let Some(h) = r
                .get("newHandle")
                .and_then(Value::as_str)
                .filter(|h| !h.is_empty())
            {
                fuori.push(DalServer::Ripresa(h.to_string()));
            }
        }
    }
    if let Some(e) = v.get("error") {
        let m = e
            .get("message")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| e.to_string());
        fuori.push(DalServer::Errore(m));
    }
    fuori
}

/// La risposta a una funzione. `chiedi_a_nova` risponde «quando e' libero»:
/// il modello finisce la frase che sta dicendo e poi racconta.
pub fn risposta_funzione(c: &Chiamata, risultato: Value) -> Value {
    let mut r = json!({ "id": c.id, "name": c.nome, "response": risultato });
    if c.nome == CHIEDI_A_NOVA {
        r["scheduling"] = json!("WHEN_IDLE");
    }
    json!({ "toolResponse": { "functionResponses": [r] } })
}

/// Come finisce una conversazione.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chiusura {
    /// Si chiude e si dimentica: il nome la riapre da capo.
    Fine,
    /// L'orecchio si chiude, la conversazione resta: il nome la riapre.
    Pausa,
}

/// Come chiudere, da cio' che ha chiesto il modello. Una parola che non si
/// capisce e' una pausa: si puo' riprendere, e una fine sbagliata no.
pub fn chiusura(argomenti: &Value) -> Chiusura {
    match argomenti.get("come").and_then(Value::as_str).map(str::trim) {
        Some("fine") => Chiusura::Fine,
        _ => Chiusura::Pausa,
    }
}

/// La richiesta da passare a NOVA, se c'e'.
pub fn richiesta(argomenti: &Value) -> Option<String> {
    argomenti
        .get("richiesta")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .map(str::to_string)
}

/// Se il microfono va mandato adesso.
///
/// Con l'altoparlante il microfono sente la voce di NOVA, e il server la
/// prende per l'utente che parla sopra: NOVA si interrompe da sola. Senza
/// cuffie si manda solo quando NOVA tace da un po' (`TACE_DA_MS`); con le
/// cuffie (`interrompibile`) sempre, e l'utente la puo' interrompere.
pub fn manda_il_microfono(interrompibile: bool, parla: bool, tace_da_ms: u64) -> bool {
    interrompibile || (!parla && tace_da_ms >= TACE_DA_MS)
}

/// Quanto deve tacere NOVA prima che il microfono riparta, senza cuffie: la
/// coda della voce nella stanza e nel buffer della scheda.
pub const TACE_DA_MS: u64 = 300;

/// Guadagno automatico del microfono.
///
/// I microfoni consegnano volumi diversissimi: sulle cuffie del PC di
/// sviluppo una frase normale arriva a 0,008 (`nova_voce::audio`). Il
/// riconoscimento del parlato del server su un sussurro non sente. Il
/// guadagno porta il picco recente verso [`PICCO_OBIETTIVO`], fino a
/// [`GUADAGNO_MASSIMO`]: il silenzio di una stanza non si gonfia fino a
/// sembrare voce.
pub fn guadagno(picco_recente: f32) -> f32 {
    if !picco_recente.is_finite() || picco_recente <= 0.0 {
        return 1.0;
    }
    (PICCO_OBIETTIVO / picco_recente).clamp(1.0, GUADAGNO_MASSIMO)
}

/// Il picco a cui il guadagno porta la voce.
pub const PICCO_OBIETTIVO: f32 = 0.5;

/// Oltre questo il guadagno non va: 0,008 diventa 0,16.
pub const GUADAGNO_MASSIMO: f32 = 20.0;

#[cfg(test)]
mod prove {
    use super::*;

    fn apertura() -> Apertura {
        Apertura {
            modello: "gemini-3.8-live".into(),
            voce: "aoede".into(),
            istruzioni: istruzioni("it"),
            funzioni: funzioni(),
            ripresa: None,
        }
    }

    #[test]
    fn l_apertura_ha_la_voce_le_funzioni_e_la_ripresa() {
        let s = setup(&apertura());
        let x = &s["setup"];
        assert_eq!(x["model"], "models/gemini-3.8-live");
        assert_eq!(
            x["generationConfig"]["responseModalities"],
            json!(["AUDIO"])
        );
        assert_eq!(
            x["generationConfig"]["speechConfig"]["voiceConfig"]["prebuiltVoiceConfig"]
                ["voiceName"],
            "Aoede",
            "la voce scritta in minuscolo si riconosce"
        );
        assert!(x["systemInstruction"]["parts"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Rispondi in italiano"));
        let nomi: Vec<&str> = x["tools"][0]["functionDeclarations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["name"].as_str().unwrap())
            .collect();
        assert_eq!(nomi, [CHIEDI_A_NOVA, CHIUDI, FERMA]);
        assert_eq!(
            x["tools"][0]["functionDeclarations"][0]["behavior"],
            "NON_BLOCKING"
        );
        assert_eq!(x["inputAudioTranscription"], json!({}));
        assert_eq!(x["outputAudioTranscription"], json!({}));
        assert_eq!(
            x["contextWindowCompression"],
            json!({ "slidingWindow": {} })
        );
        assert_eq!(
            x["sessionResumption"],
            json!({}),
            "una sessione nuova non ha maniglia"
        );
        let ripresa = setup(&Apertura {
            ripresa: Some("m1".into()),
            ..apertura()
        });
        assert_eq!(
            ripresa["setup"]["sessionResumption"],
            json!({ "handle": "m1" })
        );
    }

    #[test]
    fn una_voce_che_non_c_e_diventa_quella_di_serie() {
        assert_eq!(voce_valida(" sulafat "), "Sulafat");
        assert_eq!(voce_valida("Robot"), VOCE_PREDEFINITA);
        assert_eq!(voce_valida(""), VOCE_PREDEFINITA);
        let mut nomi: Vec<&str> = VOCI.iter().map(|v| v.0).collect();
        nomi.sort_unstable();
        nomi.dedup();
        assert_eq!(nomi.len(), 30, "trenta voci, nessuna due volte");
        assert!(VOCI.iter().any(|v| v.0 == VOCE_PREDEFINITA));
    }

    #[test]
    fn il_modello_si_scrive_come_lo_vuole_il_server() {
        assert_eq!(nome_modello(""), "models/gemini-3.8-live");
        assert_eq!(nome_modello("models/altro"), "models/altro");
        assert_eq!(nome_modello(" altro "), "models/altro");
    }

    #[test]
    fn il_microfono_va_e_la_voce_torna_in_pcm_a_16_bit() {
        let pcm = in_pcm16(&[0.0, 1.0, -1.0, 2.0, 0.5]);
        assert_eq!(pcm, [0, 32767, -32767, 32767, 16384]);
        let m = ingresso_audio(&pcm);
        assert_eq!(
            m["realtimeInput"]["audio"]["mimeType"],
            "audio/pcm;rate=16000"
        );
        let b64 = m["realtimeInput"]["audio"]["data"].as_str().unwrap();
        assert_eq!(audio_da(b64).unwrap(), pcm, "andata e ritorno");
        assert!(audio_da("non e' base64!").is_err());
        // Un byte dispari in fondo si butta.
        assert_eq!(audio_da("AQID").unwrap(), [0x0201]);
    }

    #[test]
    fn si_legge_quello_che_dice_il_server() {
        assert_eq!(leggi(r#"{"setupComplete": {}}"#), [DalServer::Pronto]);
        let voce = ingresso_audio(&[1, 2, 3])["realtimeInput"]["audio"]["data"].clone();
        let m = json!({ "serverContent": {
            "modelTurn": { "parts": [
                { "inlineData": { "mimeType": "audio/pcm;rate=24000", "data": voce } },
                { "text": "un pensiero, non audio" }
            ]},
            "inputTranscription": { "text": "che ore" },
            "outputTranscription": { "text": "Sono le" },
            "turnComplete": true
        }});
        assert_eq!(
            leggi(&m.to_string()),
            [
                DalServer::Voce(vec![1, 2, 3]),
                DalServer::Sentito("che ore".into()),
                DalServer::Detto("Sono le".into()),
                DalServer::TurnoFinito,
            ]
        );
        assert_eq!(
            leggi(r#"{"serverContent": {"interrupted": true}}"#),
            [DalServer::Interrotto]
        );
        let c = leggi(
            r#"{"toolCall": {"functionCalls": [{"id": "f1", "name": "chiedi_a_nova",
                "args": {"richiesta": "apri il PDF"}}]}}"#,
        );
        assert_eq!(
            c,
            [DalServer::Chiamate(vec![Chiamata {
                id: "f1".into(),
                nome: CHIEDI_A_NOVA.into(),
                argomenti: json!({ "richiesta": "apri il PDF" }),
            }])]
        );
        assert_eq!(
            leggi(r#"{"toolCallCancellation": {"ids": ["f1"]}}"#),
            [DalServer::Annullate(vec!["f1".into()])]
        );
        assert_eq!(
            leggi(r#"{"goAway": {"timeLeft": "50s"}}"#),
            [DalServer::StaPerChiudere("50s".into())]
        );
        assert_eq!(
            leggi(r#"{"sessionResumptionUpdate": {"resumable": true, "newHandle": "m2"}}"#),
            [DalServer::Ripresa("m2".into())]
        );
        assert!(
            leggi(r#"{"sessionResumptionUpdate": {"resumable": false, "newHandle": "m3"}}"#)
                .is_empty()
        );
        assert!(matches!(&leggi("{rotto")[..], [DalServer::Errore(_)]));
        assert_eq!(
            leggi(r#"{"error": {"message": "chiave non valida"}}"#),
            [DalServer::Errore("chiave non valida".into())]
        );
        assert!(leggi(r#"{"usageMetadata": {"totalTokenCount": 9}}"#).is_empty());
    }

    #[test]
    fn la_risposta_a_nova_arriva_quando_e_libero() {
        let c = Chiamata {
            id: "f1".into(),
            nome: CHIEDI_A_NOVA.into(),
            argomenti: json!({}),
        };
        let r = risposta_funzione(&c, json!({ "risposta": "fatto" }));
        let f = &r["toolResponse"]["functionResponses"][0];
        assert_eq!(
            (f["id"].as_str(), f["name"].as_str()),
            (Some("f1"), Some(CHIEDI_A_NOVA))
        );
        assert_eq!(f["response"]["risposta"], "fatto");
        assert_eq!(f["scheduling"], "WHEN_IDLE");
        let chiudi = risposta_funzione(
            &Chiamata {
                nome: CHIUDI.into(),
                ..c
            },
            json!({}),
        );
        assert!(chiudi["toolResponse"]["functionResponses"][0]
            .get("scheduling")
            .is_none());
    }

    #[test]
    fn chiudere_e_chiedere_si_leggono_dagli_argomenti() {
        assert_eq!(chiusura(&json!({ "come": "fine" })), Chiusura::Fine);
        assert_eq!(chiusura(&json!({ "come": "pausa" })), Chiusura::Pausa);
        assert_eq!(
            chiusura(&json!({ "come": "boh" })),
            Chiusura::Pausa,
            "nel dubbio si riprende"
        );
        assert_eq!(
            richiesta(&json!({ "richiesta": " apri " })).as_deref(),
            Some("apri")
        );
        assert_eq!(richiesta(&json!({ "richiesta": "  " })), None);
        assert_eq!(richiesta(&json!({})), None);
    }

    #[test]
    fn senza_cuffie_il_microfono_aspetta_che_nova_taccia() {
        assert!(manda_il_microfono(true, true, 0), "con le cuffie sempre");
        assert!(!manda_il_microfono(false, true, 10_000), "mentre parla no");
        assert!(!manda_il_microfono(false, false, TACE_DA_MS - 1));
        assert!(manda_il_microfono(false, false, TACE_DA_MS));
    }

    #[test]
    fn il_guadagno_alza_il_sussurro_ma_non_il_silenzio() {
        assert_eq!(guadagno(0.008), GUADAGNO_MASSIMO);
        assert_eq!(guadagno(0.1), 5.0);
        assert_eq!(guadagno(0.9), 1.0, "una voce forte non si abbassa");
        assert_eq!(guadagno(0.0), 1.0);
        assert_eq!(guadagno(f32::NAN), 1.0);
    }

    #[test]
    fn le_istruzioni_nominano_le_funzioni_e_la_lingua() {
        let t = istruzioni("en");
        assert!(t.contains("Rispondi in inglese"));
        for f in [CHIEDI_A_NOVA, CHIUDI, FERMA] {
            assert!(t.contains(f), "{f}");
        }
        assert_eq!(nome_lingua("xx"), "italiano");
    }
}
