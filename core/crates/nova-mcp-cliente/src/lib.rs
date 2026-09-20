//! NOVA che **usa** un server MCP di qualcun altro.
//!
//! `nova-mcp` dice di se': «il protocollo con cui NOVA si apre a un altro
//! programma». E' vero, ed e' meta'. Questa e' l'altra: ogni volta che una
//! cosa esiste gia' come server MCP — pilotare Excel, un gestionale, un
//! servizio interno — la scelta era fra riscriverla e rinunciarci, quando la
//! terza strada e' parlarci.
//!
//! ## Perche' questo non e' uno strumento, e' un cancello
//!
//! Un server MCP descrive i propri strumenti **con parole sue**, e quelle
//! parole finiscono nel prompt di NOVA. Chi apre questa porta senza decidere
//! prima chi puo' entrare sta dando a un estraneo il permesso di scrivere
//! dentro la testa di NOVA — e non «in teoria»: la descrizione di uno
//! strumento e' testo libero, arriva da un processo che non e' nostro, e il
//! modello la legge come legge tutto il resto.
//!
//! Da qui le cinque regole di questo crate. Nessuna e' una precauzione
//! generica: ognuna chiude un modo preciso di entrare.
//!
//! 1. **Un server si dichiara, non si scopre.** NOVA non va a cercare server
//!    MCP sul PC. Un server trovato da solo e' uno sconosciuto con il
//!    permesso di scrivere nel prompt, e nessuno gliel'ha dato (D263).
//! 2. **Le parole di un estraneo si citano, non si obbediscono.** La
//!    descrizione arriva marcata per quello che e' — testo di qualcun altro —
//!    e cio' che dentro somiglia a un ordine si **dice**, non si toglie di
//!    nascosto (D264).
//! 3. **I nomi portano davanti quello del server.** Un server che dichiara
//!    uno strumento chiamato `Bash` non deve poter coprire il `Bash` di NOVA
//!    (D265).
//! 4. **Uno strumento altrui non e' mai «sicuro».** La tabella dei rischi di
//!    `nova-mcp` conosce gli strumenti di NOVA. Di quelli di un altro non sa
//!    niente, e cio' che un estraneo dice di se' non e' una prova (D266).
//! 5. **Quel che entra ha una misura.** Descrizioni e risposte finiscono nel
//!    contesto: senza un tetto, un server puo' riempirlo da solo (D267).

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

/// La versione del protocollo che NOVA parla, dalle due parti.
pub use nova_mcp::PROTOCOLLO;

/// Quanti caratteri di descrizione di uno strumento altrui entrano nel
/// prompt. Trentatre' strumenti di NOVA ci stanno in poche migliaia di
/// caratteri; un server che ne mandasse uno da centomila non sarebbe
/// piu' utile, sarebbe **solo** il contesto di NOVA.
pub const QUANTO_PUO_DIRE: usize = 2000;

/// Quanti caratteri di risposta si leggono da una chiamata.
pub const QUANTO_PUO_RISPONDERE: usize = 20_000;

/// Quanti strumenti si accettano da un server solo.
pub const QUANTI_STRUMENTI: usize = 100;

/// Cosa si mette fra il nome del server e quello dello strumento.
pub const SEPARATORE: &str = "__";

/// Quanto si aspetta una risposta prima di considerare il server muto.
///
/// Si chiama cosi' e non `ATTESA_S` perche' quel nome e' gia' l'attesa di
/// `nova-cdp` verso Chrome: sono due protocolli diversi e due numeri
/// diversi, e chi cerca il nome deve trovare quello che intende (D225).
///
/// Senza un tetto, un server che non risponde non e' un errore: e' NOVA che
/// si ferma per sempre, in silenzio, e chi guarda vede solo che «non fa
/// niente». Trenta secondi sono larghi per un programma locale e corti per
/// una persona che aspetta.
pub const ATTESA_RISPOSTA_S: u64 = 30;

/// Come si dice a chi legge che quel che segue non e' roba di NOVA.
///
/// I nomi sono lunghi apposta: `APERTURA` e `CHIUSURA` da soli esistevano
/// gia' in un altro crate a significare un'altra cosa, e chi cerca un nome
/// in tutto il progetto deve trovarne uno (D225).
pub const CITAZIONE_APRE: &str = "[descrizione fornita dal server MCP";
pub const CITAZIONE_CHIUDE: &str = "[fine della descrizione fornita da fuori]";

/// Un server dichiarato. **Dichiarato**: NOVA non ne cerca nessuno.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dichiarato {
    pub nome: String,
    pub comando: String,
    pub argomenti: Vec<String>,
    pub cartella: Option<String>,
}

/// Se un nome di server si puo' usare.
///
/// Lettere, cifre, trattino e trattino basso. Niente punti, niente spazi,
/// niente separatore: il nome finisce **davanti** a quello di ogni strumento,
/// e un nome che contiene il separatore renderebbe impossibile dire dove
/// finisce il server e comincia lo strumento.
pub fn nome_valido(nome: &str) -> bool {
    !nome.is_empty()
        && nome.len() <= 40
        && !nome.contains(SEPARATORE)
        && nome
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// `posta__invia`: il nome con cui NOVA vede uno strumento altrui.
pub fn nome_completo(server: &str, strumento: &str) -> String {
    format!("{server}{SEPARATORE}{strumento}")
}

/// Da quale server viene, e come si chiama davvero.
pub fn da_nome_completo(completo: &str) -> Option<(&str, &str)> {
    let (server, strumento) = completo.split_once(SEPARATORE)?;
    if server.is_empty() || strumento.is_empty() {
        return None;
    }
    Some((server, strumento))
}

/// Il campo della configurazione in cui i server si dichiarano.
pub const DOVE_SI_DICHIARANO: &str = "mcp_esterni";

/// I server dichiarati nella configurazione, e le voci che non vanno.
///
/// **Dichiarati**: questa funzione legge un campo, non guarda il PC. Non c'e'
/// nessuna scoperta automatica e non ce ne sara': un server trovato da solo
/// e' uno sconosciuto con il permesso di scrivere nel prompt (D263).
///
/// Una voce sbagliata non porta giu' le altre, e non sparisce: finisce
/// nell'elenco dei rifiuti col motivo, perche' un server che l'utente ha
/// scritto e che NOVA ignora in silenzio e' il modo piu' sicuro di far
/// perdere mezz'ora a qualcuno.
pub fn dichiarati_da(config: &Value) -> (Vec<Dichiarato>, Vec<String>) {
    let vuoto = Vec::new();
    let voci = config
        .get(DOVE_SI_DICHIARANO)
        .and_then(Value::as_array)
        .unwrap_or(&vuoto);
    let mut buoni: Vec<Dichiarato> = Vec::new();
    let mut rifiutati = Vec::new();
    for v in voci {
        let nome = v.get("nome").and_then(Value::as_str).unwrap_or("").trim();
        let comando = v
            .get("comando")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if !nome_valido(nome) {
            rifiutati.push(format!(
                "«{nome}» non e' un nome di server che si possa usare: servono \
                 lettere, cifre, trattini o trattini bassi, e non piu' di quaranta"
            ));
            continue;
        }
        if comando.is_empty() {
            rifiutati.push(format!("«{nome}» non dice quale programma avviare"));
            continue;
        }
        if buoni.iter().any(|d| d.nome == nome) {
            // Due server con lo stesso nome vorrebbero dire due strumenti con
            // lo stesso nome completo, cioe' NOVA che ne chiama uno a caso.
            rifiutati.push(format!("«{nome}» e' dichiarato due volte: tengo il primo"));
            continue;
        }
        buoni.push(Dichiarato {
            nome: nome.to_string(),
            comando: comando.to_string(),
            argomenti: v
                .get("argomenti")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
            cartella: v
                .get("cartella")
                .and_then(Value::as_str)
                .filter(|x| !x.trim().is_empty())
                .map(str::to_string),
        });
    }
    (buoni, rifiutati)
}

/// Le forme che, dentro il testo di un estraneo, non descrivono uno strumento
/// ma danno un ordine a chi legge.
///
/// Non e' un antivirus e non pretende di esserlo: e' un elenco di cose che
/// in una descrizione di uno strumento **non hanno motivo di esserci**. Serve
/// a far comparire una riga davanti a una persona, non a decidere da sola.
pub const FRASI_CHE_COMANDANO: [&str; 16] = [
    "ignore previous",
    "ignore all previous",
    "disregard previous",
    "ignora le istruzioni",
    "ignora tutto",
    "system:",
    "assistant:",
    "<|im_start|>",
    "<|im_end|>",
    "you must always",
    "devi sempre",
    "do not tell the user",
    "non dirlo all'utente",
    "senza chiedere conferma",
    "without asking the user",
    "new instructions",
];

/// Cosa, in questo testo, somiglia a un ordine invece che a una descrizione.
///
/// Si **dice**, non si toglie. Togliere di nascosto vorrebbe dire che
/// l'attacco riesce a met&agrave; — la frase sparisce, ma nessuno sa che
/// qualcuno ci ha provato — e che uno strumento onesto che nomina una di
/// queste parole diventa incomprensibile senza spiegazione.
pub fn sospetti(testo: &str) -> Vec<&'static str> {
    let basso = testo.to_lowercase();
    FRASI_CHE_COMANDANO
        .iter()
        .copied()
        .filter(|f| basso.contains(f))
        .collect()
}

/// Taglia a `quanti` caratteri **veri**, non byte, e lo dice.
pub fn accorcia(testo: &str, quanti: usize) -> String {
    if testo.chars().count() <= quanti {
        return testo.to_string();
    }
    let preso: String = testo.chars().take(quanti).collect();
    format!("{preso}\n[...tagliato a {quanti} caratteri]")
}

/// La descrizione di uno strumento altrui, come la legge NOVA.
///
/// Le due righe intorno non sono decorazione: sono l'unica cosa che
/// distingue, dentro un prompt, quello che NOVA sa da quello che le ha detto
/// un processo di qualcun altro.
pub fn citato(server: &str, testo: &str) -> String {
    format!(
        "{CITAZIONE_APRE} «{server}», non da NOVA]\n{}\n{CITAZIONE_CHIUDE}",
        accorcia(testo, QUANTO_PUO_DIRE)
    )
}

/// Quanto pesa chiamare uno strumento che non e' di NOVA.
///
/// Mai «safe». `nova_mcp::rischio` sa cosa fanno gli strumenti di NOVA
/// perche' li ha scritti NOVA; di quelli di un altro sa solo il nome che si
/// sono dati. Si parte da «moderate» e si sale se negli argomenti c'e'
/// qualcosa di pesante — quella parte vale per chiunque, perche' guarda i
/// valori e non chi li manda.
pub fn rischio(argomenti: &Value) -> &'static str {
    match nova_mcp::rischio("uno-strumento-di-un-altro", argomenti) {
        "dangerous" => "dangerous",
        _ => "moderate",
    }
}

/// Uno strumento altrui, dopo il cancello.
#[derive(Debug, Clone, PartialEq)]
pub struct Strumento {
    /// Come si chiama a casa sua.
    pub suo: String,
    /// Come lo vede NOVA: `server__strumento`.
    pub completo: String,
    /// Gia' citata e gia' accorciata.
    pub descrizione: String,
    pub schema: Value,
    /// Cosa, nel testo che e' arrivato, somiglia a un ordine.
    pub sospetti: Vec<&'static str>,
}

/// Il cancello: cosa succede a una dichiarazione che arriva da fuori.
pub fn passa_il_cancello(server: &str, grezzo: &Value) -> Result<Strumento, String> {
    let suo = grezzo
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if suo.is_empty() {
        return Err("uno strumento senza nome".to_string());
    }
    if suo.len() > 80 {
        return Err(format!(
            "«{}...» ha un nome troppo lungo",
            &suo[..40.min(suo.len())]
        ));
    }
    let testo = grezzo
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let schema = grezzo
        .get("inputSchema")
        .cloned()
        // Uno strumento senza schema non e' un errore: vuol dire «non prendo
        // argomenti». Inventarne uno vuoto e' cio' che fa la specifica.
        .unwrap_or_else(|| json!({"type": "object", "properties": {}}));
    // I sospetti si cercano su **tutto** quel che arriva, non solo sulla
    // descrizione: anche i nomi dei campi dello schema finiscono nel prompt.
    let tutto = format!("{suo}\n{testo}\n{schema}");
    Ok(Strumento {
        completo: nome_completo(server, &suo),
        suo,
        descrizione: citato(server, testo),
        schema,
        sospetti: sospetti(&tutto),
    })
}

/// Gli strumenti di un server, passati dal cancello.
///
/// Uno strumento che non passa non ferma gli altri: un server con venti
/// strumenti buoni e uno senza nome resta utile per venti. Chi non passa
/// finisce nell'elenco dei rifiutati, che si mostra invece di sparire.
pub fn tutti(server: &str, elenco: &[Value]) -> (Vec<Strumento>, Vec<String>) {
    let mut buoni = Vec::new();
    let mut rifiutati = Vec::new();
    for g in elenco.iter().take(QUANTI_STRUMENTI) {
        match passa_il_cancello(server, g) {
            Ok(s) => buoni.push(s),
            Err(e) => rifiutati.push(e),
        }
    }
    if elenco.len() > QUANTI_STRUMENTI {
        rifiutati.push(format!(
            "«{server}» ne ha dichiarati {}: ne guardo i primi {QUANTI_STRUMENTI}",
            elenco.len()
        ));
    }
    (buoni, rifiutati)
}

// --------------------------------------------------------------- il tubo

/// Una busta JSON-RPC con un `id`: a questa si aspetta una risposta.
pub fn domanda(id: u64, metodo: &str, parametri: Value) -> String {
    json!({"jsonrpc": "2.0", "id": id, "method": metodo, "params": parametri}).to_string()
}

/// Una busta senza `id`: e' una **notifica**, e a una notifica non si
/// risponde mai. Aspettare una risposta vorrebbe dire restare fermi.
pub fn avviso(metodo: &str, parametri: Value) -> String {
    json!({"jsonrpc": "2.0", "method": metodo, "params": parametri}).to_string()
}

/// Cosa c'e' dentro una risposta, o perche' non c'e' niente.
pub fn risultato(risposta: &Value) -> Result<Value, String> {
    if let Some(e) = risposta.get("error") {
        let codice = e.get("code").and_then(Value::as_i64).unwrap_or(0);
        let messaggio = e
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("nessuna spiegazione");
        return Err(format!(
            "il server ha risposto con un errore ({codice}): {messaggio}"
        ));
    }
    risposta
        .get("result")
        .cloned()
        .ok_or_else(|| "la risposta non ha ne' un risultato ne' un errore".to_string())
}

/// Il testo che una chiamata ha prodotto.
///
/// La risposta di `tools/call` e' un elenco di pezzi con un tipo. Si prende
/// quel che e' testo; di quel che non lo e' — un'immagine, un dato binario —
/// si dice **che c'e'** invece di far finta di niente: una chiamata che
/// risponde un'immagine e che qui sembra vuota manderebbe NOVA a riprovare
/// all'infinito.
pub fn testo_del_risultato(r: &Value) -> String {
    let vuoto = Vec::new();
    let pezzi = r.get("content").and_then(Value::as_array).unwrap_or(&vuoto);
    let mut fuori: Vec<String> = Vec::new();
    for p in pezzi {
        match p.get("type").and_then(Value::as_str).unwrap_or("") {
            "text" => fuori.push(
                p.get("text")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            ),
            altro => fuori.push(format!("[un pezzo di tipo «{altro}», che non e' testo]")),
        }
    }
    accorcia(&fuori.join("\n"), QUANTO_PUO_RISPONDERE)
}

/// Se lo strumento dice di non esserci riuscito.
///
/// Nel protocollo MCP un errore **dello strumento** non e' un errore del
/// protocollo: arriva come una risposta riuscita con `isError` acceso, con
/// dentro il motivo. E' una distinzione giusta — «la chiamata non si e'
/// potuta fare» e «la chiamata si e' fatta e ha detto di no» sono due cose
/// diverse — ma va **guardata**, o tutte e due diventano «riuscito».
///
/// Il banco contro un server vero ha trovato proprio questo: chiamare uno
/// strumento che non esiste tornava `Ok`, perche' il server risponde con un
/// risultato che dice «non lo conosco». NOVA lo avrebbe letto come una
/// chiamata andata bene con dentro una frase strana (D268).
pub fn non_ce_l_ha_fatta(r: &Value) -> bool {
    r.get("isError").and_then(Value::as_bool).unwrap_or(false)
}

/// Un server acceso, col suo tubo aperto.
pub struct Collegamento {
    pub server: String,
    figlio: Child,
    dentro: ChildStdin,
    /// Le righe arrivano da un filo a parte. Non e' architettura per il
    /// gusto di farla: leggere direttamente da una pipe non si puo'
    /// interrompere, e un server che non risponde bloccherebbe NOVA per
    /// sempre invece che per `ATTESA_RISPOSTA_S` secondi.
    righe: Receiver<String>,
    contatore: u64,
}

impl Collegamento {
    /// Accende il server dichiarato.
    ///
    /// Il comando e gli argomenti vanno come **argomenti**, non come una riga
    /// da interpretare: non c'e' nessuna shell in mezzo, quindi non c'e'
    /// niente da proteggere (D130).
    pub fn apri(d: &Dichiarato) -> Result<Collegamento, String> {
        if !nome_valido(&d.nome) {
            return Err(format!(
                "«{}» non e' un nome di server che si possa usare: servono lettere, \
                 cifre, trattini o trattini bassi, e non piu' di quaranta",
                d.nome
            ));
        }
        let mut c = Command::new(&d.comando);
        c.args(&d.argomenti)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // Gli errori del server restano suoi: molti scrivono righe di
            // diagnostica su stderr, e mescolarle alle risposte vorrebbe
            // dire non riuscire piu' a leggere ne' le une ne' le altre.
            .stderr(Stdio::null());
        if let Some(dove) = &d.cartella {
            c.current_dir(dove);
        }
        let mut figlio = c
            .spawn()
            .map_err(|e| format!("non riesco ad avviare «{}» ({}): {e}", d.nome, d.comando))?;
        let dentro = figlio.stdin.take().ok_or("il server non accetta niente")?;
        let uscita = figlio.stdout.take().ok_or("il server non dice niente")?;
        let (manda, righe) = mpsc::channel();
        std::thread::spawn(move || {
            for riga in BufReader::new(uscita).lines() {
                let Ok(riga) = riga else { break };
                if manda.send(riga).is_err() {
                    break; // dall'altra parte non ascolta piu' nessuno
                }
            }
        });
        Ok(Collegamento {
            server: d.nome.clone(),
            figlio,
            dentro,
            righe,
            contatore: 0,
        })
    }

    fn scrivi(&mut self, riga: &str) -> Result<(), String> {
        self.dentro
            .write_all(riga.as_bytes())
            .and_then(|_| self.dentro.write_all(b"\n"))
            .and_then(|_| self.dentro.flush())
            .map_err(|e| format!("«{}» non ascolta piu': {e}", self.server))
    }

    pub(crate) fn chiedi(&mut self, metodo: &str, parametri: Value) -> Result<Value, String> {
        self.contatore += 1;
        let mio = self.contatore;
        self.scrivi(&domanda(mio, metodo, parametri))?;
        let scade = Instant::now() + Duration::from_secs(ATTESA_RISPOSTA_S);
        loop {
            let quanto = scade.saturating_duration_since(Instant::now());
            let riga = match self.righe.recv_timeout(quanto) {
                Ok(r) => r,
                Err(RecvTimeoutError::Timeout) => {
                    return Err(format!(
                        "«{}» non ha risposto a «{metodo}» entro {ATTESA_RISPOSTA_S} secondi",
                        self.server
                    ))
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(format!(
                        "«{}» ha chiuso senza rispondere a «{metodo}»",
                        self.server
                    ))
                }
            };
            let riga = riga.trim();
            if riga.is_empty() {
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(riga) else {
                // Una riga che non e' JSON e' diagnostica finita nel posto
                // sbagliato: si salta. Fermarsi qui vorrebbe dire che un
                // server chiacchierone non si puo' usare affatto.
                continue;
            };
            // Una risposta con un altro `id` e' di una domanda di prima;
            // una senza `id` e' una notifica del server, e non e' la
            // risposta a niente.
            match v.get("id").and_then(Value::as_u64) {
                Some(suo) if suo == mio => return risultato(&v),
                _ => continue,
            }
        }
    }

    /// Il saluto: si dice chi si e', e si conferma di aver capito.
    pub fn saluta(&mut self) -> Result<Value, String> {
        let r = self.chiedi(
            "initialize",
            json!({
                "protocolVersion": PROTOCOLLO,
                "capabilities": {},
                "clientInfo": {"name": nova_mcp::NOME, "version": nova_mcp::VERSIONE}
            }),
        )?;
        // Senza questa notifica molti server non rispondono a niente altro.
        self.scrivi(&avviso("notifications/initialized", json!({})))?;
        Ok(r)
    }

    /// Cosa sa fare, passato dal cancello.
    pub fn strumenti(&mut self) -> Result<(Vec<Strumento>, Vec<String>), String> {
        let r = self.chiedi("tools/list", json!({}))?;
        let elenco = r
            .get("tools")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        Ok(tutti(&self.server.clone(), &elenco))
    }

    /// Chiama uno strumento. Il nome e' **il suo**, senza il prefisso.
    ///
    /// Torna `Err` in tutti e due i modi di non farcela — il protocollo che
    /// rifiuta, e lo strumento che dice di no — ma il messaggio li
    /// distingue, perche' si risolvono in modi diversi: al primo si guarda
    /// il server, al secondo si guardano gli argomenti.
    pub fn chiama(&mut self, strumento: &str, argomenti: &Value) -> Result<String, String> {
        let r = self.chiedi(
            "tools/call",
            json!({"name": strumento, "arguments": argomenti}),
        )?;
        let testo = testo_del_risultato(&r);
        if non_ce_l_ha_fatta(&r) {
            return Err(format!(
                "«{strumento}» dice di non esserci riuscito: {testo}"
            ));
        }
        Ok(testo)
    }

    /// Spegne il server.
    ///
    /// Si chiude prima lo stdin: e' cosi' che un processo che legge righe
    /// capisce che ha finito, e quasi tutti escono da se'. Chi non esce lo si
    /// ferma — un server MCP lasciato acceso e' un processo che resta sul PC
    /// di qualcuno dopo che NOVA ha chiuso.
    pub fn chiudi(mut self) {
        drop(self.dentro);
        let _ = self.figlio.wait();
    }
}

#[cfg(test)]
mod prove;
