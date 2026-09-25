//! Il browser di NOVA, guidato dal demone.
//!
//! Sono gli otto strumenti `web_*` del server MCP Python — `web_apri`,
//! `web_trova`, `web_leggi`, `web_click`, `web_scrivi`, `web_incolla`,
//! `web_carica`, `web_tabella` — qui `web.apri` e compagnia. Il modello li
//! vede con lo stesso nome (`web_apri`) e con la stessa descrizione, presa
//! dalle dichiarazioni estratte da Python: il prompt di NOVA li nomina cosi',
//! e adesso ci sono anche per il cervello del demone, che prima non aveva un
//! browser affatto.
//!
//! Chi fa cosa:
//!
//! - il JavaScript che gira nella pagina e il confine fra argomento e codice:
//!   `nova_browser` (copioni estratti da Python, `dentro`);
//! - la frase che torna al modello, la riga del registro, la scelta dopo un
//!   incolla: `nova_browser::racconti`, confrontati col Python da un banco;
//! - la connessione, le schede, la porta HTTP: `nova_cdp`;
//! - qui: avviare il browser col suo profilo, e mettere in fila i pezzi.
//!
//! **Il profilo e' di NOVA, non dell'utente.** Da Chrome 136 la porta di
//! debug e' vietata sul profilo predefinito, ed e' giusto cosi': NOVA agisce
//! come se stessa, con accessi suoi che si revocano senza toccare quelli
//! dell'utente.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use nova_browser::racconti::{self, Annotazione, DopoIncolla};
use nova_proto::{CapabilityInfo, Risk};
use serde_json::{json, Value};

use crate::capability::{arg_i64_opt, arg_str, arg_str_opt, Capability, Ctx, Registry};

/// La porta del browser di lavoro: quella di `nova_browser`, la stessa del
/// Python.
const PORTA: u16 = nova_browser::PORTA;

/// Quanto si aspetta che il browser apra la porta, e che una pagina finisca
/// di caricare: `ATTESA_AVVIO_S` del Python, e i suoi venti secondi di
/// `apri`.
const ATTESA_AVVIO: Duration = Duration::from_secs(20);
const ATTESA_PAGINA: Duration = Duration::from_secs(20);

/// Dove Windows mette Edge e Chrome, in quest'ordine: Edge prima, perche'
/// c'e' su ogni Windows.
const EDGE: [&str; 2] = [
    r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
    r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
];
const CHROME: [&str; 2] = [
    r"C:\Program Files\Google\Chrome\Application\chrome.exe",
    r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
];

pub fn register(reg: &mut Registry) {
    for (nome, rischio) in [
        ("web.apri", Risk::Moderate),
        ("web.trova", Risk::Safe),
        ("web.leggi", Risk::Safe),
        ("web.click", Risk::Moderate),
        ("web.scrivi", Risk::Moderate),
        ("web.incolla", Risk::Moderate),
        ("web.carica", Risk::Moderate),
        ("web.tabella", Risk::Safe),
    ] {
        reg.add(Arc::new(Web { nome, rischio }));
    }
}

// ------------------------------------------------------------ il browser

/// Il profilo del browser di NOVA: accanto a `config.json`, come in Python.
fn profilo() -> PathBuf {
    crate::mondo::cartella_nova().join("browser")
}

fn eseguibile() -> Result<String, String> {
    for p in EDGE.iter().chain(CHROME.iter()) {
        if std::path::Path::new(p).is_file() {
            return Ok(p.to_string());
        }
    }
    for nome in ["msedge", "chrome"] {
        let trovato = crate::processo::trova(nome);
        if !trovato.is_empty() {
            return Ok(trovato);
        }
    }
    // Chi legge non ha ne' Edge ne' Chrome, quindi non e' una macchina come
    // le altre: merita di sapere cosa smette di funzionare e cosa no.
    Err(
        "non trovo ne' Edge ne' Chrome, e senza uno dei due non posso guidare una \
         pagina: aprire, leggere, premere e scrivere sui siti restano fuori. Cercare sul \
         web funziona lo stesso, in modo piu' fragile. Se ne hai uno installato altrove, \
         dimmi dov'e'."
            .into(),
    )
}

/// Accende il browser di NOVA, o si attacca a quello gia' acceso.
fn avvia() -> Result<(), String> {
    if nova_cdp::versione(PORTA).is_some() {
        return Ok(());
    }
    let p = profilo();
    std::fs::create_dir_all(&p).map_err(|e| format!("non posso creare {}: {e}", p.display()))?;
    let mut c = std::process::Command::new(eseguibile()?);
    c.args([
        format!("--remote-debugging-port={PORTA}"),
        // Obbligatorio: sul profilo predefinito la porta e' vietata.
        format!("--user-data-dir={}", p.display()),
        // Solo l'interfaccia locale: la porta di debug e' una chiave di casa.
        format!("--remote-allow-origins={}", nova_cdp::ORIGINE),
        "--no-first-run".into(),
        "--no-default-browser-check".into(),
        "--new-window".into(),
        "about:blank".into(),
    ])
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW, come il Python
    }
    c.spawn()
        .map_err(|e| format!("non riesco ad avviare il browser: {e}"))?;
    let scadenza = Instant::now() + ATTESA_AVVIO;
    while Instant::now() < scadenza {
        if nova_cdp::versione(PORTA).is_some() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    Err(format!(
        "il browser non ha aperto la porta {PORTA} entro {}s",
        ATTESA_AVVIO.as_secs()
    ))
}

/// La scheda su cui lavorare: per identificativo prima, poi per un pezzo di
/// indirizzo o di titolo (`nova_browser::scheda`).
fn scheda(quale: &str) -> Result<nova_cdp::Scheda, String> {
    let tutte = nova_cdp::schede(PORTA)?;
    let ridotte: Vec<nova_browser::Scheda> = tutte
        .iter()
        .map(|t| nova_browser::Scheda {
            id: t.id.clone(),
            url: t.url.clone(),
            titolo: t.titolo.clone(),
        })
        .collect();
    match nova_browser::scheda(&ridotte, quale) {
        Ok(s) => Ok(tutte
            .into_iter()
            .find(|t| t.id == s.id)
            .expect("e' nell'elenco")),
        Err(nova_browser::NessunaScheda::NonCeNeSono) => Err("nessuna scheda aperta".into()),
        Err(nova_browser::NessunaScheda::NessunaCosi { quale, quante }) => {
            Err(format!("nessuna scheda «{quale}» fra le {quante} aperte"))
        }
    }
}

/// Esegue un copione in una scheda e ne riporta il valore.
fn valuta_in(t: &nova_cdp::Scheda, codice: &str) -> Result<Value, String> {
    let r = nova_cdp::chiedi(
        t,
        "Runtime.evaluate",
        nova_browser::valuta_params(codice),
        Duration::from_secs(nova_cdp::ATTESA_S),
    )?;
    if let Some(e) = nova_browser::errore_di_pagina(&r) {
        return Err(e);
    }
    Ok(nova_browser::valore_di(&r).cloned().unwrap_or(Value::Null))
}

fn valuta(quale: &str, codice: &str) -> Result<Value, String> {
    valuta_in(&scheda(quale)?, codice)
}

/// L'indirizzo della pagina su cui si agisce, per il registro: senza, il
/// registro direbbe «premuto #invia» senza dire su quale sito.
fn dove_sono(quale: &str) -> String {
    scheda(quale)
        .map(|t| t.url.chars().take(300).collect())
        .unwrap_or_default()
}

fn annota(a: Option<Annotazione>, dove: &str) {
    if let Some(a) = a {
        crate::registro::annota(&a.azione, dove, &a.dettagli, &a.tipo, "");
    }
}

/// `Path(x).expanduser()` di Python: solo la tilde in testa.
fn espandi_tilde(p: &str) -> PathBuf {
    let casa = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    match p.strip_prefix('~') {
        Some(resto) if resto.is_empty() || resto.starts_with(['/', '\\']) => {
            PathBuf::from(format!("{casa}{resto}"))
        }
        _ => PathBuf::from(p),
    }
}

// ------------------------------------------------------------ le azioni

fn apri(url: &str) -> Result<String, String> {
    avvia()?;
    let t = nova_cdp::nuova(PORTA, url)?;
    let id = t
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    // «Esiste la scheda» non vuol dire «c'e' quello che ti serve»: si aspetta
    // che la pagina abbia finito di caricare.
    //
    // E «complete» non basta: il documento vuoto con cui nasce ogni scheda
    // dice `complete` **prima** che la navigazione cominci. Fermarsi li'
    // voleva dire che le mosse dopo finivano su una pagina bianca — la prova
    // l'ha preso alla seconda esecuzione. Si aspetta anche di aver lasciato
    // `about:blank`, salvo che fosse proprio quella la pagina chiesta.
    let pronta = if url.trim_start().starts_with("about:") {
        "document.readyState === 'complete'"
    } else {
        "document.readyState === 'complete' && location.href !== 'about:blank'"
    };
    let scadenza = Instant::now() + ATTESA_PAGINA;
    while Instant::now() < scadenza {
        if let Ok(v) = valuta(&id, pronta) {
            if v.as_bool() == Some(true) {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(400));
    }
    // Titolo e indirizzo si rileggono **dopo** il caricamento. Il Python
    // riporta quelli della scheda appena creata, quando la pagina non c'e'
    // ancora: il titolo e' vuoto, e dopo un rinvio — il sito che manda alla
    // pagina di accesso — l'indirizzo e' quello chiesto e non quello dove si
    // e' davvero. Il modello legge questa risposta per decidere la mossa
    // dopo, e deve leggere dove sta.
    let caricata = scheda(&id).ok();
    let riassunto = json!({
        "id": t.get("id").cloned().unwrap_or(Value::Null),
        "url": caricata.as_ref().map(|x| json!(x.url))
            .unwrap_or_else(|| t.get("url").cloned().unwrap_or(Value::Null)),
        "titolo": caricata.as_ref().map(|x| json!(x.titolo))
            .unwrap_or_else(|| t.get("title").cloned().unwrap_or(Value::Null)),
    });
    Ok(racconti::apri(&riassunto, url))
}

fn trova(a: &Value) -> Result<String, String> {
    let selettore = arg_str_opt(a, "selettore").unwrap_or_default();
    let testo = arg_str_opt(a, "testo").unwrap_or_default();
    let quale = arg_str_opt(a, "scheda").unwrap_or_default();
    let quanti = arg_i64_opt(a, "quanti").unwrap_or(20);
    if let Some(e) = racconti::manca_bersaglio(&selettore, &testo) {
        return Ok(e);
    }
    let e = if !nova_pitone::senza_bianchi(&testo).is_empty() {
        // Mai esatto: `web_trova` del Python non lo passa, e lo strumento
        // cerca «quello che c'e' scritto», anche dentro una frase piu' lunga.
        let r = valuta(
            &quale,
            &nova_browser::per_testo(&testo, &selettore, quanti, false),
        )?;
        r.get("nodi").cloned().unwrap_or(Value::Null)
    } else {
        valuta(&quale, &nova_browser::trova(&selettore, quanti))?
    };
    Ok(racconti::trova(&e, &selettore, &testo))
}

fn leggi(a: &Value) -> Result<String, String> {
    let quale = arg_str_opt(a, "scheda").unwrap_or_default();
    let caratteri = arg_i64_opt(a, "caratteri").unwrap_or(6000);
    Ok(racconti::leggi(&valuta(
        &quale,
        &nova_browser::leggi(caratteri),
    )?))
}

fn tabella(a: &Value) -> Result<String, String> {
    let selettore = arg_str_opt(a, "selettore").unwrap_or_default();
    let quale = arg_str_opt(a, "scheda").unwrap_or_default();
    let righe = arg_i64_opt(a, "righe").unwrap_or(400);
    let d = valuta(&quale, &nova_browser::tabella(&selettore, righe, 120))?;
    Ok(racconti::tabella(&d, righe))
}

fn click(a: &Value) -> Result<String, String> {
    let selettore = arg_str_opt(a, "selettore").unwrap_or_default();
    let testo = arg_str_opt(a, "testo").unwrap_or_default();
    let quale = arg_str_opt(a, "scheda").unwrap_or_default();
    if let Some(e) = racconti::manca_bersaglio(&selettore, &testo) {
        return Ok(e);
    }
    let dove = dove_sono(&quale);
    let copione = if !nova_pitone::senza_bianchi(&testo).is_empty() {
        nova_browser::clicca_testo(&testo, &selettore)
    } else {
        nova_browser::clicca(&selettore)
    };
    let (detto, nota) = racconti::click(&valuta(&quale, &copione)?, &selettore, &testo);
    annota(nota, &dove);
    Ok(detto)
}

fn scrivi(a: &Value) -> Result<String, String> {
    let selettore = arg_str(a, "selettore").map_err(|e| e.to_string())?;
    let testo = arg_str_opt(a, "testo").unwrap_or_default();
    let segreto = arg_str_opt(a, "segreto").unwrap_or_default();
    let quale = arg_str_opt(a, "scheda").unwrap_or_default();
    if !nova_pitone::senza_bianchi(&segreto).is_empty() {
        // Il valore esce dall'archivio, entra nel campo, e muore qui: non
        // torna al modello e non finisce nel registro.
        let valore = crate::caps_segreti::valore_per_uso(nova_pitone::senza_bianchi(&segreto))
            .unwrap_or_default();
        if valore.is_empty() {
            return Ok(racconti::segreto_assente(&segreto));
        }
        let dove = dove_sono(&quale);
        let esito = valuta(&quale, &nova_browser::scrivi(&selettore, &valore));
        drop(valore);
        let (detto, nota) = racconti::scrivi_segreto(&esito?, &selettore, &segreto);
        annota(nota, &dove);
        return Ok(detto);
    }
    let dove = dove_sono(&quale);
    let (detto, nota) = racconti::scrivi(
        &valuta(&quale, &nova_browser::scrivi(&selettore, &testo))?,
        &selettore,
        &testo,
    );
    annota(nota, &dove);
    Ok(detto)
}

fn incolla(a: &Value) -> Result<String, String> {
    let testo = arg_str(a, "testo").map_err(|e| e.to_string())?;
    let selettore = arg_str_opt(a, "selettore").unwrap_or_default();
    let quale = arg_str_opt(a, "scheda").unwrap_or_default();
    let dove = dove_sono(&quale);
    let t = scheda(&quale)?;
    let esito = valuta_in(&t, &nova_browser::incolla(&testo, &selettore))?;
    let r = match racconti::dopo_incolla(&esito) {
        DopoIncolla::Fine(v) => v,
        DopoIncolla::Inserisci(v) => {
            nova_cdp::chiedi(
                &t,
                "Input.insertText",
                json!({ "text": testo }),
                Duration::from_secs(nova_cdp::ATTESA_S),
            )?;
            v
        }
    };
    let (detto, nota) = racconti::incolla(&r, &testo);
    annota(nota, &dove);
    Ok(detto)
}

fn carica(a: &Value) -> Result<String, String> {
    let selettore = arg_str(a, "selettore").map_err(|e| e.to_string())?;
    let quale = arg_str_opt(a, "scheda").unwrap_or_default();
    // Una stringa sola vale un elenco di uno, come in Python.
    let percorsi: Vec<String> = match a.get("percorsi") {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(v)) => v.iter().map(|x| nova_pitone::str_di(Some(x))).collect(),
        _ => Vec::new(),
    };
    let dove = dove_sono(&quale);
    let r = consegna(&selettore, &percorsi, &quale)?;
    let (detto, nota) = racconti::carica(&r, &selettore);
    annota(nota, &dove);
    Ok(detto)
}

/// I file dentro un campo di caricamento, senza finestre di dialogo: li
/// mette il browser. Tutto in una connessione sola, perche' l'`objectId`
/// dell'elemento vale solo dentro la sessione che l'ha chiesto.
fn consegna(selettore: &str, percorsi: &[String], quale: &str) -> Result<Value, String> {
    let mut veri = Vec::new();
    for x in percorsi {
        let f = espandi_tilde(x);
        if !f.is_file() {
            return Ok(
                json!({"ok": false, "motivo": format!("file inesistente: {}", f.display())}),
            );
        }
        let assoluto = std::fs::canonicalize(&f).unwrap_or(f);
        let s = assoluto.to_string_lossy().to_string();
        // Su Windows `canonicalize` scrive `\\?\C:\...`, che il browser non
        // capisce: il Python (`resolve`) scrive `C:\...`.
        veri.push(s.strip_prefix(r"\\?\").map(str::to_string).unwrap_or(s));
    }
    if veri.is_empty() {
        return Ok(json!({"ok": false, "motivo": "nessun file da caricare"}));
    }
    let t = scheda(quale)?;
    let mut s = nova_cdp::Sessione::apri(&t, Duration::from_secs(nova_cdp::ATTESA_S))?;
    let esito = (|| -> Result<Value, String> {
        s.chiama("DOM.enable", json!({}))?;
        let r = s.chiama(
            "Runtime.evaluate",
            json!({
                "expression": format!("document.querySelector({})", nova_browser::dentro(selettore)),
                "returnByValue": false,
            }),
        )?;
        let Some(oid) = r
            .pointer("/result/objectId")
            .and_then(Value::as_str)
            .map(str::to_string)
        else {
            return Ok(json!({"ok": false, "motivo": "nessun elemento per quel selettore"}));
        };
        let tipo = s.chiama(
            "Runtime.callFunctionOn",
            json!({
                "objectId": oid,
                "functionDeclaration":
                    "function(){return this.tagName.toLowerCase()+':'+(this.type||'')}",
                "returnByValue": true,
            }),
        )?;
        let che = tipo
            .pointer("/result/value")
            .and_then(Value::as_str)
            .unwrap_or("");
        if let Some(m) = racconti::non_e_un_campo_file(che) {
            return Ok(json!({"ok": false, "motivo": m}));
        }
        s.chiama(
            "DOM.setFileInputFiles",
            json!({"objectId": oid, "files": veri}),
        )?;
        let nomi: Vec<String> = veri
            .iter()
            .map(|x| {
                std::path::Path::new(x)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            })
            .collect();
        Ok(json!({"ok": true, "file": nomi}))
    })();
    s.chiudi();
    esito
}

// ------------------------------------------------------------ la capacita'

struct Web {
    nome: &'static str,
    rischio: Risk,
}

/// Descrizione e schema come li vede Claude Code dal server Python: estratti,
/// non riscritti. Il modello sceglie lo strumento su queste parole.
fn dichiarata(nome_mcp: &str) -> (String, Value) {
    let tutte: Value =
        serde_json::from_str(nova_mcp::dichiarazioni::STRUMENTI_JSON).unwrap_or(Value::Null);
    let d = tutte
        .as_array()
        .and_then(|a| a.iter().find(|x| x["name"] == nome_mcp))
        .cloned()
        .unwrap_or(Value::Null);
    (
        d["description"].as_str().unwrap_or("").to_string(),
        d.get("inputSchema")
            .cloned()
            .unwrap_or(json!({"type": "object"})),
    )
}

fn primo(a: &Value, k: &[&str]) -> String {
    k.iter()
        .find_map(|x| arg_str_opt(a, x).filter(|s| !s.trim().is_empty()))
        .unwrap_or_default()
}

#[async_trait]
impl Capability for Web {
    fn info(&self) -> CapabilityInfo {
        let (description, schema) = dichiarata(&self.nome.replace('.', "_"));
        CapabilityInfo {
            name: self.nome.into(),
            description,
            risk: self.rischio,
            category: "web".into(),
            schema,
        }
    }

    /// Cosa succederebbe, in una riga: e' anche la frase della richiesta di
    /// permesso, quando il livello di autonomia la chiede.
    async fn anteprima(&self, a: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        let dove = match primo(&a, &["scheda"]) {
            s if s.is_empty() => String::new(),
            s => format!(" (scheda {s})"),
        };
        let farei = match self.nome {
            "web.apri" => format!("Apro {} nel browser di NOVA", primo(&a, &["url"])),
            "web.trova" => format!(
                "Cerco «{}» nella pagina{dove}",
                primo(&a, &["testo", "selettore"])
            ),
            "web.leggi" => format!("Leggo la pagina{dove}"),
            "web.tabella" => format!("Leggo una tabella della pagina{dove}"),
            "web.click" => format!(
                "Premo «{}» nella pagina{dove}",
                primo(&a, &["testo", "selettore"])
            ),
            "web.scrivi" => match primo(&a, &["segreto"]) {
                s if !s.is_empty() => format!(
                    "Scrivo la credenziale «{s}» in {}{dove}",
                    primo(&a, &["selettore"])
                ),
                _ => format!(
                    "Scrivo in {}{dove}:\n{}",
                    primo(&a, &["selettore"]),
                    primo(&a, &["testo"])
                ),
            },
            "web.incolla" => {
                let t = primo(&a, &["testo"]);
                format!(
                    "Incollo {} righe in {}{dove}",
                    t.matches('\n').count() + 1,
                    match primo(&a, &["selettore"]) {
                        s if s.is_empty() => "dove sta il fuoco".to_string(),
                        s => s,
                    }
                )
            }
            "web.carica" => format!(
                "Consegno {} a {}{dove}",
                match a.get("percorsi") {
                    Some(Value::Array(v)) => v
                        .iter()
                        .map(|x| nova_pitone::str_di(Some(x)))
                        .collect::<Vec<_>>()
                        .join(", "),
                    Some(v) => nova_pitone::str_di(Some(v)),
                    None => String::new(),
                },
                primo(&a, &["selettore"])
            ),
            _ => return None,
        };
        Some(Ok(json!({ "farei": farei, "annullabile": false })))
    }

    async fn call(&self, a: Value, _ctx: &Ctx) -> Result<Value> {
        let nome = self.nome;
        let fatto = tokio::task::spawn_blocking(move || match nome {
            "web.apri" => arg_str(&a, "url")
                .map_err(|e| e.to_string())
                .and_then(|u| apri(&u)),
            "web.trova" => trova(&a),
            "web.leggi" => leggi(&a),
            "web.tabella" => tabella(&a),
            "web.click" => click(&a),
            "web.scrivi" => scrivi(&a),
            "web.incolla" => incolla(&a),
            "web.carica" => carica(&a),
            altro => Err(format!("«{altro}» non e' uno strumento del browser")),
        })
        .await
        .map_err(|e| anyhow!("il browser non ha risposto: {e}"))?;
        match fatto {
            // Un «ERRORE: ...» detto dal racconto e' gia' la frase per il
            // modello; qui diventa un fallimento, come per ogni capacita',
            // cosi' il turno lo conta e Claude lo vede come errore.
            Ok(t) if t.starts_with("ERRORE: ") => Err(anyhow!("{}", &t[8..])),
            Ok(t) => Ok(Value::String(t)),
            Err(e) => Err(anyhow!("{e}")),
        }
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn ogni_doppione_tolto_a_claude_c_e_davvero_nel_demone() {
        // Se l'elenco nomina uno strumento che il demone non ha, Claude lo
        // perde del tutto: non lo vede piu' dal Python e non lo trova qui.
        let mut reg = Registry::default();
        register(&mut reg);
        for s in nova_cervelli::claude::SPOSTATI_NEL_DEMONE {
            let nome = s.strip_prefix("mcp__nova__").expect("e' del server Python");
            assert!(reg.get(nome).is_some(), "{nome} non e' nel demone");
            assert!(
                !dichiarata(nome).0.is_empty(),
                "{nome} non ha la descrizione del Python"
            );
        }
    }
}
