//! Backend Windows: UI Automation.
//!
//! UIA e' COM, e COM non ama i thread altrui: gli oggetti non sono `Send`, e
//! l'apartment va inizializzato una volta sola. Quindi tutto il lavoro vive in
//! **un thread dedicato** che possiede l'apartment e la `IUIAutomation`, e il
//! resto del mondo gli parla per messaggi. Fuori si vede un backend normale,
//! `Send + Sync`, che si puo' tenere in un `Arc` dentro un runtime async.

use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use windows::core::{Interface, BOOL, BSTR};
use windows::Win32::Foundation::{HWND, LPARAM, MAX_PATH, RECT};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationInvokePattern,
    IUIAutomationLegacyIAccessiblePattern, IUIAutomationSelectionItemPattern,
    IUIAutomationTogglePattern, IUIAutomationValuePattern, TreeScope_Children,
    UIA_InvokePatternId, UIA_LegacyIAccessiblePatternId, UIA_SelectionItemPatternId,
    TreeScope_Subtree, UIA_ControlTypePropertyId, UIA_TogglePatternId, UIA_ValuePatternId,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
};

use crate::{ElementRef, UiNode, UiQuery, UiTree, WindowInfo, WindowSel};

/// Quanti figli guardare per nodo: alcune liste ne hanno decine di migliaia e
/// un albero che non finisce mai non serve a nessuno.
const MAX_FIGLI: usize = 200;

/// Quanti elementi guardare al massimo durante una ricerca.
///
/// `FindAll` su una pagina come Gmail restituisce decine di migliaia di nodi;
/// leggere il nome di ognuno e' una chiamata che attraversa i processi. Il
/// tetto e' la differenza fra una ricerca che risponde e una che sembra
/// appesa. Quando lo si tocca, si dice.
const MAX_ESAMINATI: i32 = 6000;

/// Oltre questo, si smette di aspettare.
///
/// Una chiamata UI Automation entra nel message loop del processo bersaglio:
/// se quello e' appeso, la chiamata non torna mai. Senza un limite, l'unico
/// thread worker resta ostaggio e *ogni* richiesta successiva — su qualunque
/// finestra, da qualunque client — muore in coda dietro di lei.
const TIMEOUT_CHIAMATA: Duration = Duration::from_secs(20);

// ------------------------------------------------------------- messaggi

type Esito<T> = Sender<Result<T, String>>;

enum Cmd {
    Windows(Esito<Vec<WindowInfo>>),
    Tree(WindowSel, usize, Esito<UiNode>),
    Find(WindowSel, UiQuery, usize, Esito<Vec<UiNode>>),
    Invoke(ElementRef, Esito<()>),
    SetValue(ElementRef, String, Esito<()>),
    Focus(ElementRef, Esito<()>),
}

pub struct Uia {
    /// `None` quando il worker precedente e' rimasto bloccato: la prossima
    /// richiesta ne accende uno nuovo invece di ereditarne il blocco.
    tx: Mutex<Option<Sender<Cmd>>>,
}

impl Uia {
    pub fn new() -> Result<Self> {
        let (tx, rx) = channel::<Cmd>();
        let (pronto_tx, pronto_rx) = channel::<Result<(), String>>();

        std::thread::Builder::new()
            .name("nova-uia".into())
            .spawn(move || unsafe {
                // apartment multithread: niente pompa messaggi da gestire
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                let automation: IUIAutomation =
                    match CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) {
                        Ok(a) => a,
                        Err(e) => {
                            let _ = pronto_tx.send(Err(format!(
                                "UI Automation non disponibile: {e}"
                            )));
                            return;
                        }
                    };
                let _ = pronto_tx.send(Ok(()));
                servi(&automation, rx);
            })
            .map_err(|e| anyhow!("impossibile avviare il thread UIA: {e}"))?;

        match pronto_rx.recv() {
            Ok(Ok(())) => Ok(Self { tx: Mutex::new(Some(tx)) }),
            Ok(Err(e)) => bail!(e),
            Err(_) => bail!("il thread UIA e' morto durante l'avvio"),
        }
    }

    /// Accende un thread UIA nuovo. Il vecchio, se e' bloccato in COM, non si
    /// puo' uccidere: restera' li' finche' l'applicazione non risponde, poi
    /// trovera' il canale chiuso e uscira' da solo.
    fn rimpiazza_worker(&self) -> Result<Sender<Cmd>> {
        let (tx, rx) = channel::<Cmd>();
        let (pronto_tx, pronto_rx) = channel::<Result<(), String>>();
        std::thread::Builder::new()
            .name("nova-uia".into())
            .spawn(move || unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                let automation: IUIAutomation =
                    match CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) {
                        Ok(a) => a,
                        Err(e) => {
                            let _ = pronto_tx.send(Err(format!("{e}")));
                            return;
                        }
                    };
                let _ = pronto_tx.send(Ok(()));
                servi(&automation, rx);
            })
            .map_err(|e| anyhow!("impossibile riavviare il thread UIA: {e}"))?;
        match pronto_rx.recv() {
            Ok(Ok(())) => Ok(tx),
            Ok(Err(e)) => bail!("UI Automation non disponibile: {e}"),
            Err(_) => bail!("il thread UIA e' morto durante il riavvio"),
        }
    }

    fn invia<T>(&self, costruisci: impl FnOnce(Esito<T>) -> Cmd) -> Result<T> {
        let (tx, rx) = channel::<Result<T, String>>();
        {
            let mut guardia = self
                .tx
                .lock()
                .map_err(|_| anyhow!("canale UIA avvelenato"))?;
            if guardia.is_none() {
                *guardia = Some(self.rimpiazza_worker()?);
            }
            let mittente = guardia.as_ref().expect("appena creato");
            if mittente.send(costruisci(tx)).is_err() {
                // il worker e' morto: uno nuovo e la richiesta si ripete
                *guardia = None;
                bail!("il thread UIA non risponde piu': riprova");
            }
        }
        match rx.recv_timeout(TIMEOUT_CHIAMATA) {
            Ok(esito) => esito.map_err(|e| anyhow!(e)),
            Err(RecvTimeoutError::Timeout) => {
                // Il worker e' ostaggio di un'applicazione che non risponde.
                // Si lascia andare e la prossima richiesta ne accende un altro:
                // meglio perdere un thread che perdere tutto il sottosistema.
                if let Ok(mut guardia) = self.tx.lock() {
                    *guardia = None;
                }
                bail!(
                    "l'applicazione non ha risposto entro {}s: probabilmente e' bloccata. \
                     Il canale UIA e' stato rigenerato, puoi riprovare.",
                    TIMEOUT_CHIAMATA.as_secs()
                )
            }
            Err(RecvTimeoutError::Disconnected) => {
                if let Ok(mut guardia) = self.tx.lock() {
                    *guardia = None;
                }
                bail!("il thread UIA e' morto durante la richiesta")
            }
        }
    }
}

impl UiTree for Uia {
    fn backend(&self) -> &'static str {
        "uia"
    }

    fn windows(&self) -> Result<Vec<WindowInfo>> {
        self.invia(Cmd::Windows)
    }

    fn tree(&self, window: &WindowSel, depth: usize) -> Result<UiNode> {
        let w = window.clone();
        self.invia(move |tx| Cmd::Tree(w, depth, tx))
    }

    fn find(&self, window: &WindowSel, query: &UiQuery, limit: usize) -> Result<Vec<UiNode>> {
        let (w, q) = (window.clone(), query.clone());
        self.invia(move |tx| Cmd::Find(w, q, limit, tx))
    }

    fn invoke(&self, target: &ElementRef) -> Result<()> {
        let t = target.clone();
        self.invia(move |tx| Cmd::Invoke(t, tx))
    }

    fn set_value(&self, target: &ElementRef, text: &str) -> Result<()> {
        let (t, s) = (target.clone(), text.to_string());
        self.invia(move |tx| Cmd::SetValue(t, s, tx))
    }

    fn focus(&self, target: &ElementRef) -> Result<()> {
        let t = target.clone();
        self.invia(move |tx| Cmd::Focus(t, tx))
    }
}

// ------------------------------------------------------- ciclo del thread

unsafe fn servi(automation: &IUIAutomation, rx: std::sync::mpsc::Receiver<Cmd>) {
    while let Ok(cmd) = rx.recv() {
        match cmd {
            Cmd::Windows(tx) => {
                let _ = tx.send(elenca_finestre().map_err(|e| e.to_string()));
            }
            Cmd::Tree(sel, depth, tx) => {
                let esito = (|| -> Result<UiNode> {
                    let el = elemento_finestra(automation, &sel)?;
                    Ok(costruisci_albero(automation, &el, Vec::new(), depth))
                })();
                let _ = tx.send(esito.map_err(|e| e.to_string()));
            }
            Cmd::Find(sel, query, limit, tx) => {
                let esito = (|| -> Result<Vec<UiNode>> {
                    let el = elemento_finestra(automation, &sel)?;
                    cerca(automation, &el, &query, limit)
                })();
                let _ = tx.send(esito.map_err(|e| e.to_string()));
            }
            Cmd::Invoke(target, tx) => {
                let _ = tx.send(agisci(automation, &target).map_err(|e| e.to_string()));
            }
            Cmd::SetValue(target, testo, tx) => {
                let _ = tx.send(scrivi(automation, &target, &testo).map_err(|e| e.to_string()));
            }
            Cmd::Focus(target, tx) => {
                let esito = (|| -> Result<()> {
                    let el = risolvi(automation, &target)?;
                    el.SetFocus()
                        .map_err(|e| anyhow!("impossibile dare il fuoco: {e}"))
                })();
                let _ = tx.send(esito.map_err(|e| e.to_string()));
            }
        }
    }
}

// ------------------------------------------------------------- finestre

unsafe extern "system" fn raccogli(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let elenco = &mut *(lparam.0 as *mut Vec<HWND>);
    if IsWindowVisible(hwnd).as_bool() && GetWindowTextLengthW(hwnd) > 0 {
        elenco.push(hwnd);
    }
    BOOL(1)
}

unsafe fn elenca_finestre() -> Result<Vec<WindowInfo>> {
    let mut handles: Vec<HWND> = Vec::new();
    EnumWindows(
        Some(raccogli),
        LPARAM(&mut handles as *mut Vec<HWND> as isize),
    )
    .map_err(|e| anyhow!("EnumWindows fallita: {e}"))?;

    let mut fuori = Vec::with_capacity(handles.len());
    for h in handles {
        let mut buf = [0u16; 512];
        let n = GetWindowTextW(h, &mut buf);
        let titolo = String::from_utf16_lossy(&buf[..n.max(0) as usize]);
        if titolo.trim().is_empty() {
            continue;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(h, Some(&mut pid));
        fuori.push(WindowInfo {
            handle: h.0 as i64,
            title: titolo,
            process: nome_processo(pid),
            pid,
        });
    }
    Ok(fuori)
}

unsafe fn nome_processo(pid: u32) -> String {
    if pid == 0 {
        return String::new();
    }
    let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
        return String::new();
    };
    let mut buf = [0u16; MAX_PATH as usize];
    let mut n = buf.len() as u32;
    let esito = QueryFullProcessImageNameW(
        handle,
        PROCESS_NAME_FORMAT(0),
        windows::core::PWSTR(buf.as_mut_ptr()),
        &mut n,
    );
    let _ = windows::Win32::Foundation::CloseHandle(handle);
    if esito.is_err() {
        return String::new();
    }
    let intero = String::from_utf16_lossy(&buf[..n as usize]);
    intero
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(&intero)
        .to_string()
}

// -------------------------------------------------------------- elementi

unsafe fn elemento_finestra(
    automation: &IUIAutomation,
    sel: &WindowSel,
) -> Result<IUIAutomationElement> {
    let hwnd = match sel {
        WindowSel::Handle(h) => HWND(*h as *mut core::ffi::c_void),
        WindowSel::Title(t) => {
            let finestre = elenca_finestre()?;
            let t_min = t.to_lowercase();
            let trovata = finestre
                .iter()
                .find(|w| w.title.to_lowercase().contains(&t_min))
                .ok_or_else(|| {
                    anyhow!(
                        "nessuna finestra con «{t}» nel titolo. Aperte: {}",
                        finestre
                            .iter()
                            .map(|w| w.title.as_str())
                            .take(12)
                            .collect::<Vec<_>>()
                            .join(" | ")
                    )
                })?;
            HWND(trovata.handle as *mut core::ffi::c_void)
        }
    };
    automation
        .ElementFromHandle(hwnd)
        .map_err(|e| anyhow!("la finestra non espone un albero di accessibilita': {e}"))
}

/// Cerca dentro tutta la finestra, chiedendolo a UI Automation.
///
/// La versione di prima costruiva l'albero fino a una profondita' fissa e poi
/// filtrava. Su una pagina semplice bastava; su Gmail no — il nodo `document`
/// sta all'undicesimo livello e i pulsanti veri stanno molto piu' sotto, quindi
/// la ricerca trovava solo la cornice del browser e restituiva «niente» a una
/// domanda a cui la risposta c'era. Alzare il limite non era la soluzione: con
/// duecento figli per nodo, ogni livello in piu' moltiplica il lavoro, e ogni
/// figlio e' una chiamata che attraversa i processi.
///
/// `FindAll(TreeScope_Subtree)` sposta la camminata **dentro** il motore di
/// UI Automation: una chiamata sola, nessun limite di profondita', e il
/// filtro grosso (il ruolo) lo applica lui.
///
/// Resta da ricostruire il `path`, che e' il modo in cui NOVA indirizza un
/// elemento fra una chiamata e l'altra: `FindAll` restituisce elementi, non
/// indirizzi. Si risale ai genitori contando la posizione fra i fratelli — ma
/// solo per i pochi elementi che passano il filtro, non per tutti.
unsafe fn cerca(
    automation: &IUIAutomation,
    radice: &IUIAutomationElement,
    query: &UiQuery,
    limit: usize,
) -> Result<Vec<UiNode>> {
    // Il ruolo, se c'e', lo filtra il motore: e' cio' che evita di leggere il
    // nome di diecimila elementi per trovarne tre.
    let condizione = match codice_ruolo(&query.role) {
        Some(codice) => {
            let v: VARIANT = codice.into();
            automation.CreatePropertyCondition(UIA_ControlTypePropertyId, &v)?
        }
        None => automation.CreateTrueCondition()?,
    };

    let trovati = radice
        .FindAll(TreeScope_Subtree, &condizione)
        .map_err(|e| anyhow!("ricerca fallita: {e}"))?;
    let quanti = trovati.Length().unwrap_or(0);

    let mut fuori = Vec::new();
    let mut guardati = 0i32;
    for i in 0..quanti.min(MAX_ESAMINATI) {
        guardati += 1;
        let Ok(el) = trovati.GetElement(i) else { continue };
        // Il nodo si costruisce senza percorso: calcolarlo per tutti sarebbe
        // il costo che stiamo evitando.
        let mut n = nodo(&el, Vec::new());
        if !query.matches(&n) {
            continue;
        }
        n.path = percorso_di(automation, radice, &el);
        fuori.push(n);
        if fuori.len() >= limit.max(1) {
            break;
        }
    }
    tracing::debug!(quanti, guardati, trovati = fuori.len(), "ricerca");
    Ok(fuori)
}

/// Da elemento a percorso di indici, risalendo i genitori.
///
/// Vuoto se la risalita non arriva alla radice: meglio un percorso assente,
/// che chi legge vede, di uno inventato che porta altrove.
unsafe fn percorso_di(
    automation: &IUIAutomation,
    radice: &IUIAutomationElement,
    el: &IUIAutomationElement,
) -> Vec<u32> {
    let Ok(walker) = automation.ControlViewWalker() else {
        return Vec::new();
    };
    let mut indici: Vec<u32> = Vec::new();
    let mut corrente = el.clone();
    for _ in 0..64 {
        if automation
            .CompareElements(&corrente, radice)
            .map(|b| b.as_bool())
            .unwrap_or(false)
        {
            indici.reverse();
            return indici;
        }
        let Ok(padre) = walker.GetParentElement(&corrente) else {
            return Vec::new();
        };
        let fratelli = figli(automation, &padre);
        let Some(posto) = fratelli.iter().position(|f| {
            automation
                .CompareElements(f, &corrente)
                .map(|b| b.as_bool())
                .unwrap_or(false)
        }) else {
            // Oltre MAX_FIGLI il fratello esiste ma non l'abbiamo contato:
            // dirlo con un percorso vuoto e' piu' onesto che indovinare.
            return Vec::new();
        };
        indici.push(posto as u32);
        corrente = padre;
    }
    Vec::new()
}

unsafe fn figli(
    automation: &IUIAutomation,
    el: &IUIAutomationElement,
) -> Vec<IUIAutomationElement> {
    let Ok(condizione) = automation.CreateTrueCondition() else {
        return Vec::new();
    };
    let Ok(trovati) = el.FindAll(TreeScope_Children, &condizione) else {
        return Vec::new();
    };
    let quanti = trovati.Length().unwrap_or(0).min(MAX_FIGLI as i32);
    let mut fuori = Vec::with_capacity(quanti as usize);
    for i in 0..quanti {
        if let Ok(f) = trovati.GetElement(i) {
            fuori.push(f);
        }
    }
    fuori
}

unsafe fn testo(b: windows::core::Result<BSTR>) -> String {
    b.map(|s| s.to_string()).unwrap_or_default()
}

unsafe fn nodo(el: &IUIAutomationElement, path: Vec<u32>) -> UiNode {
    let control_type = el.CurrentControlType().map(|c| c.0).unwrap_or(0);
    let role = ruolo(control_type);
    let rect: RECT = el.CurrentBoundingRectangle().unwrap_or_default();
    let automation_id = testo(el.CurrentAutomationId());
    UiNode {
        path,
        name: testo(el.CurrentName()),
        actions: azioni_probabili(&role),
        role,
        value: valore(el),
        automation_id: if automation_id.is_empty() { None } else { Some(automation_id) },
        enabled: el.CurrentIsEnabled().map(|b| b.as_bool()).unwrap_or(true),
        bounds: if rect.right > rect.left {
            Some([rect.left, rect.top, rect.right - rect.left, rect.bottom - rect.top])
        } else {
            None
        },
        children: Vec::new(),
    }
}

unsafe fn valore(el: &IUIAutomationElement) -> Option<String> {
    let pattern = el.GetCurrentPattern(UIA_ValuePatternId).ok()?;
    let value: IUIAutomationValuePattern = pattern.cast().ok()?;
    let v = testo(value.CurrentValue());
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

unsafe fn costruisci_albero(
    automation: &IUIAutomation,
    el: &IUIAutomationElement,
    path: Vec<u32>,
    depth: usize,
) -> UiNode {
    let mut n = nodo(el, path.clone());
    if depth == 0 {
        return n;
    }
    for (i, f) in figli(automation, el).into_iter().enumerate() {
        let mut p = path.clone();
        p.push(i as u32);
        n.children.push(costruisci_albero(automation, &f, p, depth - 1));
    }
    n
}

unsafe fn risolvi(
    automation: &IUIAutomation,
    target: &ElementRef,
) -> Result<IUIAutomationElement> {
    let mut corrente = elemento_finestra(automation, &target.window)?;
    for (livello, indice) in target.path.iter().enumerate() {
        let f = figli(automation, &corrente);
        let scelto = f.get(*indice as usize).ok_or_else(|| {
            anyhow!(
                "percorso non valido: al livello {livello} l'indice {indice} non esiste \
                 (ci sono {} figli). L'interfaccia e' cambiata: rifai ui.find.",
                f.len()
            )
        })?;
        corrente = scelto.clone();
    }
    Ok(corrente)
}

// -------------------------------------------------------------- azioni

unsafe fn agisci(automation: &IUIAutomation, target: &ElementRef) -> Result<()> {
    let el = risolvi(automation, target)?;
    let nome = testo(el.CurrentName());

    if let Ok(p) = el.GetCurrentPattern(UIA_InvokePatternId) {
        if let Ok(invoke) = p.cast::<IUIAutomationInvokePattern>() {
            return invoke
                .Invoke()
                .map_err(|e| anyhow!("«{nome}» non ha accettato il comando: {e}"));
        }
    }
    if let Ok(p) = el.GetCurrentPattern(UIA_TogglePatternId) {
        if let Ok(toggle) = p.cast::<IUIAutomationTogglePattern>() {
            return toggle
                .Toggle()
                .map_err(|e| anyhow!("«{nome}» non si e' lasciato commutare: {e}"));
        }
    }
    if let Ok(p) = el.GetCurrentPattern(UIA_SelectionItemPatternId) {
        if let Ok(sel) = p.cast::<IUIAutomationSelectionItemPattern>() {
            return sel
                .Select()
                .map_err(|e| anyhow!("«{nome}» non si e' lasciato selezionare: {e}"));
        }
    }
    if let Ok(p) = el.GetCurrentPattern(UIA_LegacyIAccessiblePatternId) {
        if let Ok(legacy) = p.cast::<IUIAutomationLegacyIAccessiblePattern>() {
            return legacy
                .DoDefaultAction()
                .map_err(|e| anyhow!("«{nome}» non ha un'azione predefinita: {e}"));
        }
    }
    bail!("«{nome}» non espone nessuna azione: non e' un elemento cliccabile")
}

unsafe fn scrivi(automation: &IUIAutomation, target: &ElementRef, testo_nuovo: &str) -> Result<()> {
    let el = risolvi(automation, target)?;
    let nome = testo(el.CurrentName());
    let pattern = el
        .GetCurrentPattern(UIA_ValuePatternId)
        .map_err(|e| anyhow!("«{nome}» non e' un campo scrivibile: {e}"))?;
    let value: IUIAutomationValuePattern = pattern
        .cast()
        .map_err(|e| anyhow!("«{nome}» non e' un campo scrivibile: {e}"))?;
    if value.CurrentIsReadOnly().map(|b| b.as_bool()).unwrap_or(false) {
        bail!("«{nome}» e' in sola lettura");
    }
    value
        .SetValue(&BSTR::from(testo_nuovo))
        .map_err(|e| anyhow!("«{nome}» ha rifiutato il testo: {e}"))
}

// ------------------------------------------------------------- tassonomia

/// Da identificatore UIA a nome comprensibile. I numeri sono stabili
/// dall'epoca di Windows 7: usarli evita di dipendere dai nomi delle costanti,
/// che cambiano fra le versioni di windows-rs.
fn ruolo(control_type: i32) -> String {
    match control_type {
        50000 => "button",
        50001 => "calendar",
        50002 => "checkbox",
        50003 => "combobox",
        50004 => "edit",
        50005 => "hyperlink",
        50006 => "image",
        50007 => "listitem",
        50008 => "list",
        50009 => "menu",
        50010 => "menubar",
        50011 => "menuitem",
        50012 => "progressbar",
        50013 => "radiobutton",
        50014 => "scrollbar",
        50015 => "slider",
        50016 => "spinner",
        50017 => "statusbar",
        50018 => "tab",
        50019 => "tabitem",
        50020 => "text",
        50021 => "toolbar",
        50022 => "tooltip",
        50023 => "tree",
        50024 => "treeitem",
        50025 => "custom",
        50026 => "group",
        50027 => "thumb",
        50028 => "datagrid",
        50029 => "dataitem",
        50030 => "document",
        50031 => "splitbutton",
        50032 => "window",
        50033 => "pane",
        50034 => "header",
        50035 => "headeritem",
        50036 => "table",
        50037 => "titlebar",
        50038 => "separator",
        50039 => "semanticzoom",
        50040 => "appbar",
        _ => "unknown",
    }
    .to_string()
}

/// Da nome di ruolo a identificatore UIA. Serve a far filtrare il motore
/// invece di leggere il nome di ogni nodo per poi scartarlo.
fn codice_ruolo(nome: &str) -> Option<i32> {
    if nome.trim().is_empty() {
        return None;
    }
    let n = nome.trim().to_lowercase();
    (50000..=50040).find(|c| ruolo(*c) == n)
}

/// Cosa *probabilmente* si puo' fare, dedotto dal ruolo.
///
/// E' un'indicazione, non una promessa: interrogare i pattern di ogni nodo
/// costerebbe un giro COM per nodo e renderebbe l'albero lentissimo. La verita'
/// si scopre al momento dell'azione, che fallisce con un messaggio chiaro.
fn azioni_probabili(role: &str) -> Vec<String> {
    let a: &[&str] = match role {
        "button" | "hyperlink" | "menuitem" | "splitbutton" => &["invoke"],
        "checkbox" | "radiobutton" => &["toggle"],
        "edit" | "document" => &["set_value"],
        "combobox" => &["set_value", "expand"],
        "listitem" | "treeitem" | "tabitem" | "dataitem" => &["select"],
        _ => &[],
    };
    a.iter().map(|s| s.to_string()).collect()
}
