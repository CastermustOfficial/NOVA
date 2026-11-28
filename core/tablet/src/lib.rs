//! NOVA su un altro dispositivo (D449): lo schermo che entra dal portale.
//!
//! Deciso con Gio il 27 novembre: un tablet che e' «solo NOVA», una specie
//! di Alexa. Qui non c'e' nessun cervello: il lavoro lo fa il NOVA del PC, e
//! questo programma e' il suo secondo schermo.
//!
//! - **Si abbina** con l'invito del pannello (`nova://abbina?...`), che
//!   arriva incollato nella pagina o aperto come indirizzo. Un invito aperto
//!   da fuori non abbina da solo: riempie il campo, e chi ha il dispositivo
//!   in mano preme «Abbina». Un indirizzo lo puo' aprire qualunque
//!   programma, e abbinarsi a un PC sbagliato vorrebbe dire mostrare — e
//!   domani far sentire — a un altro.
//! - **Tiene il filo** col PC ([`tieni_il_filo`]): entra col suo gettone, si
//!   iscrive agli eventi, e quando il filo cade riprova, con calma, per
//!   sempre. Se il PC non lo riconosce piu' (e' stato dimenticato dal
//!   pannello) butta l'abbinamento e torna a chiedere un invito.
//! - **Gira alla pagina** quello che il demone racconta, con gli stessi
//!   canali che usa il guscio sul PC (`nova_proto::vista::per_la_pagina`):
//!   l'orb e la chat sono le stesse.
//!
//! Le regole del canale stanno in `nova-portale`, e li' si provano; qui c'e'
//! solo la colla con la finestra.

use nova_portale::cliente::{self, Abbinamento, Collegamento};
use nova_portale::invito::Invito;
use nova_portale::saluto::NON_ABBINATO;
use nova_proto::vista::{per_la_pagina, ARGOMENTI_DI_UNO_SCHERMO};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_deep_link::DeepLinkExt;
use tokio::sync::Notify;

/// Il canale dello stato dell'orb, lo stesso del guscio sul PC.
const CANALE_STATO: &str = "nova://stato";

/// La fase della voce che si da' per buona se il PC non la dice.
const FASE_A_RIPOSO: &str = "dormiente";

/// L'orb quando il PC non c'e': quasi fermo e in bianco e nero.
const ORB_SENZA_PC: &str = "spento";

/// Il canale su cui la pagina sente com'e' il filo col PC.
const CANALE_LINEA: &str = "nova://linea";

/// Il canale su cui la pagina riceve un invito aperto da fuori.
const CANALE_INVITO: &str = "nova://invito";

/// Quanto si aspetta fra un tentativo e l'altro di trovare il PC: da qui...
const PAUSA_CORTA: Duration = Duration::from_millis(500);

/// ...a qui, raddoppiando.
const PAUSA_LUNGA: Duration = Duration::from_secs(10);

/// Quanto si aspetta una risposta corta del PC.
const ATTESA_CORTA: Duration = Duration::from_secs(60);

/// Quanto si aspetta la risposta a una domanda: un turno di NOVA puo' usare
/// degli strumenti e durare a lungo.
const ATTESA_DEL_TURNO: Duration = Duration::from_secs(1800);

/// Cosa si dice a chi scrive mentre il PC non c'e'.
const SENZA_PC: &str = "NOVA non e' collegata al PC: riprovo da sola appena lo trovo.";

/// Lo stato del programma.
struct Tablet {
    /// Dove sta l'abbinamento, nella cartella dei dati del programma.
    file: PathBuf,
    collegamento: Mutex<Option<Arc<Collegamento>>>,
    /// Com'e' il filo, come lo legge la pagina.
    linea: Mutex<Value>,
    /// L'ultimo stato dell'orb mandato alla pagina: una pagina che si apre
    /// a filo gia' teso non ha sentito l'evento, e lo chiede.
    orb: Mutex<String>,
    /// L'invito aperto da fuori che la pagina non ha ancora letto.
    invito: Mutex<Option<String>>,
    /// Sveglia chi tiene il filo: c'e' un abbinamento nuovo.
    riprova: Notify,
}

impl Tablet {
    /// Cambia com'e' il filo e lo dice alla pagina.
    fn di(&self, app: &AppHandle, abbinato: bool, collegato: bool, errore: &str) {
        let linea = json!({ "abbinato": abbinato, "collegato": collegato, "errore": errore });
        *self.linea.lock().unwrap() = linea.clone();
        let _ = app.emit(CANALE_LINEA, self.linea_con_l_orb());
    }

    /// Il filo, con l'ultimo stato dell'orb.
    fn linea_con_l_orb(&self) -> Value {
        let mut linea = self.linea.lock().unwrap().clone();
        linea["orb"] = json!(*self.orb.lock().unwrap());
        linea
    }

    /// Manda alla pagina i messaggi di un evento, e tiene lo stato dell'orb.
    fn gira(&self, app: &AppHandle, topic: &str, dati: &Value) {
        for (canale, messaggio) in per_la_pagina(topic, dati) {
            if canale == CANALE_STATO {
                if let Some(stato) = messaggio.get("stato").and_then(Value::as_str) {
                    *self.orb.lock().unwrap() = stato.to_string();
                }
            }
            let _ = app.emit(canale, messaggio);
        }
    }
}

/// In che fase e' la voce sul PC, chiesta appena collegati: dormiente (in
/// attesa del nome) o sveglia. Da li' viene l'orb a riposo, come sul PC: il
/// D449 metteva «quiete» a ogni collegamento, che e' l'orb sveglio, e il 28
/// novembre Gio ha chiesto perche' a riposo l'orb fosse cosi' acceso.
async fn fase_della_voce(c: &Collegamento) -> String {
    c.chiedi(
        "capabilities/call",
        json!({ "name": "voce.fase", "args": {} }),
        ATTESA_CORTA,
    )
    .await
    .ok()
    .and_then(|r| r.get("fase").and_then(Value::as_str).map(str::to_string))
    // Se il PC non lo dice, a riposo: e' il caso di gran lunga piu' lungo.
    .unwrap_or_else(|| FASE_A_RIPOSO.to_string())
}

/// Tiene il filo col PC, per sempre.
async fn tieni_il_filo(app: AppHandle, t: Arc<Tablet>) {
    let mut pausa = PAUSA_CORTA;
    loop {
        let abbinamento = match Abbinamento::leggi(&t.file) {
            Ok(Some(a)) => a,
            // Mai abbinato, o un file che non si legge: serve un invito.
            Ok(None) => {
                let errore = t.linea.lock().unwrap()["errore"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let errore = if errore == NON_ABBINATO {
                    errore
                } else {
                    String::new()
                };
                t.di(&app, false, false, &errore);
                t.riprova.notified().await;
                continue;
            }
            Err(e) => {
                t.di(&app, false, false, &format!("{e:#}"));
                t.riprova.notified().await;
                continue;
            }
        };
        match cliente::entra(&abbinamento).await {
            Ok(canale) => {
                let (c, mut eventi) = Collegamento::avvia(canale);
                let iscritto = c
                    .chiedi(
                        "events/subscribe",
                        json!({ "topics": ARGOMENTI_DI_UNO_SCHERMO }),
                        ATTESA_CORTA,
                    )
                    .await;
                match iscritto {
                    Ok(_) => {
                        pausa = PAUSA_CORTA;
                        let fase = fase_della_voce(&c).await;
                        *t.collegamento.lock().unwrap() = Some(c);
                        t.gira(&app, "voce.fase", &json!({ "fase": fase }));
                        t.di(&app, true, true, "");
                        while let Some(evento) = eventi.recv().await {
                            t.gira(&app, &evento.topic, &evento.dati);
                        }
                        *t.collegamento.lock().unwrap() = None;
                        *t.orb.lock().unwrap() = ORB_SENZA_PC.to_string();
                        t.di(&app, true, false, "");
                    }
                    Err(e) => t.di(&app, true, false, &format!("{e:#}")),
                }
            }
            // Il PC c'e' e dice che questo dispositivo non e' dei suoi: il
            // gettone non serve piu', e riprovare non lo fara' servire.
            Err(e) if e.to_string() == NON_ABBINATO => {
                let _ = Abbinamento::dimentica(&t.file);
                t.di(&app, false, false, NON_ABBINATO);
                continue;
            }
            Err(e) => t.di(&app, true, false, &format!("{e:#}")),
        }
        tokio::select! {
            _ = tokio::time::sleep(pausa) => {}
            _ = t.riprova.notified() => {}
        }
        pausa = (pausa * 2).min(PAUSA_LUNGA);
    }
}

/// Com'e' il filo col PC, per la pagina che si apre.
#[tauri::command]
fn linea(t: State<'_, Arc<Tablet>>) -> Value {
    t.linea_con_l_orb()
}

/// L'invito aperto da fuori, se la pagina non l'ha ancora letto.
#[tauri::command]
fn invito_in_arrivo(t: State<'_, Arc<Tablet>>) -> Option<String> {
    t.invito.lock().unwrap().take()
}

/// Dove porta un invito, per farlo leggere a chi sta per abbinarsi.
#[tauri::command]
fn leggi_invito(invito: String) -> Result<Value, String> {
    let i = Invito::leggi(&invito).map_err(|e| e.to_string())?;
    Ok(json!({ "indirizzi": i.indirizzi, "porta": i.porta }))
}

/// Si abbina al PC dell'invito, e tiene l'abbinamento.
#[tauri::command]
async fn abbina(t: State<'_, Arc<Tablet>>, invito: String, nome: String) -> Result<(), String> {
    let invito = Invito::leggi(&invito).map_err(|e| e.to_string())?;
    let (abbinamento, _canale) = cliente::abbina(&invito, &nome)
        .await
        .map_err(|e| format!("{e:#}"))?;
    abbinamento.salva(&t.file).map_err(|e| format!("{e:#}"))?;
    t.riprova.notify_one();
    Ok(())
}

/// Una frase scritta sul dispositivo: la porta al NOVA del PC e torna con la
/// risposta.
#[tauri::command]
async fn parla(t: State<'_, Arc<Tablet>>, testo: String) -> Result<String, String> {
    let c = t.collegamento.lock().unwrap().clone().ok_or(SENZA_PC)?;
    let r = c
        .chiedi(
            "agente/turno",
            json!({ "testo": testo, "voce": false, "postilla": "" }),
            ATTESA_DEL_TURNO,
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(r.get("risposta")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string())
}

/// Un indirizzo aperto da fuori: se e' un invito lo si tiene per la pagina.
fn arriva(app: &AppHandle, t: &Tablet, indirizzo: &str) {
    if Invito::leggi(indirizzo).is_err() {
        return;
    }
    *t.invito.lock().unwrap() = Some(indirizzo.to_string());
    let _ = app.emit(CANALE_INVITO, indirizzo);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            let cartella = app.path().app_data_dir()?;
            let t = Arc::new(Tablet {
                file: cartella.join("abbinamento.json"),
                collegamento: Mutex::new(None),
                linea: Mutex::new(json!({ "abbinato": false, "collegato": false, "errore": "" })),
                orb: Mutex::new(ORB_SENZA_PC.to_string()),
                invito: Mutex::new(None),
                riprova: Notify::new(),
            });
            app.manage(t.clone());

            // L'invito con cui il programma e' stato aperto, e quelli che
            // arrivano mentre e' aperto.
            if let Ok(Some(indirizzi)) = app.deep_link().get_current() {
                for indirizzo in indirizzi {
                    arriva(app.handle(), &t, indirizzo.as_str());
                }
            }
            let (maniglia, per_gli_inviti) = (app.handle().clone(), t.clone());
            app.deep_link().on_open_url(move |evento| {
                for indirizzo in evento.urls() {
                    arriva(&maniglia, &per_gli_inviti, indirizzo.as_str());
                }
            });

            tauri::async_runtime::spawn(tieni_il_filo(app.handle().clone(), t));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            linea,
            invito_in_arrivo,
            leggi_invito,
            abbina,
            parla
        ])
        .run(tauri::generate_context!())
        .expect("NOVA non parte");
}
