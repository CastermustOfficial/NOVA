//! # nova-platform
//!
//! Il sistema operativo dietro un'astrazione.
//!
//! La capacita' che serve a NOVA per «usare davvero il PC senza guardarlo»
//! esiste su tutti e tre i sistemi, con tre nomi diversi, perche' ovunque la
//! legge impone un'API di accessibilita':
//!
//! | | Windows | macOS | Linux |
//! |---|---|---|---|
//! | albero dei controlli | UI Automation | Accessibility API | AT-SPI2 |
//!
//! Qui c'e' il trait comune e i tipi neutri. I backend stanno accanto, uno per
//! sistema. Non pixel: **oggetti** — pulsanti, campi, voci di menu, celle —
//! con nome, ruolo, valore e stato.

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[cfg(windows)]
mod windows_uia;

#[cfg(not(windows))]
mod non_implementato;

/// Una finestra di primo livello.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    /// Identificatore opaco della finestra (HWND su Windows).
    pub handle: i64,
    pub title: String,
    pub process: String,
    pub pid: u32,
}

/// Un nodo dell'albero dei controlli.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UiNode {
    /// Percorso di indici dalla radice della finestra: `[0, 3, 1]`.
    /// E' l'indirizzo con cui si torna a questo elemento in una chiamata
    /// successiva, senza tenere aperto niente fra una e l'altra.
    pub path: Vec<u32>,
    pub name: String,
    /// Ruolo normalizzato: `button`, `edit`, `text`, `list`, `menuitem`, ...
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automation_id: Option<String>,
    pub enabled: bool,
    /// Cosa si puo' fare: `invoke`, `set_value`, `toggle`, `select`, `expand`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<String>,
    /// x, y, larghezza, altezza. Utile all'utente, non al modello.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounds: Option<[i32; 4]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<UiNode>,
}

/// Come si sceglie una finestra: per handle o per pezzo di titolo.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WindowSel {
    Handle(i64),
    Title(String),
}

/// Filtro di ricerca dentro una finestra. I campi vuoti non filtrano.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UiQuery {
    /// Sottostringa del nome, senza distinzione fra maiuscole e minuscole.
    #[serde(default)]
    pub name: String,
    /// Ruolo esatto: `button`, `edit`, ...
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub automation_id: String,
    /// Solo elementi su cui si puo' agire.
    #[serde(default)]
    pub actionable: bool,
}

impl UiQuery {
    pub fn is_empty(&self) -> bool {
        self.name.is_empty()
            && self.role.is_empty()
            && self.automation_id.is_empty()
            && !self.actionable
    }

    pub fn matches(&self, n: &UiNode) -> bool {
        if !self.name.is_empty()
            && !n.name.to_lowercase().contains(&self.name.to_lowercase())
        {
            return false;
        }
        if !self.role.is_empty() && !n.role.eq_ignore_ascii_case(&self.role) {
            return false;
        }
        if !self.automation_id.is_empty()
            && n.automation_id.as_deref().unwrap_or("") != self.automation_id
        {
            return false;
        }
        if self.actionable && n.actions.is_empty() {
            return false;
        }
        true
    }
}

/// L'indirizzo di un elemento: finestra piu' percorso.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementRef {
    pub window: WindowSel,
    #[serde(default)]
    pub path: Vec<u32>,
}

/// L'albero dei controlli del sistema, qualunque sistema sia.
pub trait UiTree: Send + Sync {
    /// Nome del backend, per diagnostica: `uia`, `atspi`, `ax`.
    fn backend(&self) -> &'static str;

    /// Finestre di primo livello visibili.
    fn windows(&self) -> Result<Vec<WindowInfo>>;

    /// Albero dei controlli di una finestra, fino a `depth` livelli.
    fn tree(&self, window: &WindowSel, depth: usize) -> Result<UiNode>;

    /// Elementi che corrispondono al filtro, in ordine di profondita'.
    fn find(&self, window: &WindowSel, query: &UiQuery, limit: usize) -> Result<Vec<UiNode>>;

    /// Preme un pulsante, sceglie una voce di menu, attiva un collegamento.
    fn invoke(&self, target: &ElementRef) -> Result<()>;

    /// Scrive dentro un campo di testo, senza passare dalla tastiera.
    fn set_value(&self, target: &ElementRef, text: &str) -> Result<()>;

    /// Porta il fuoco su un elemento.
    fn focus(&self, target: &ElementRef) -> Result<()>;
}

/// Costruisce il backend giusto per questo sistema.
pub fn backend() -> Result<Box<dyn UiTree>> {
    #[cfg(windows)]
    {
        Ok(Box::new(windows_uia::Uia::new()?))
    }
    #[cfg(not(windows))]
    {
        Ok(Box::new(non_implementato::NonImplementato))
    }
}

/// Appiattisce un albero in una lista, mantenendo i percorsi.
pub fn appiattisci(radice: &UiNode) -> Vec<UiNode> {
    let mut fuori = Vec::new();
    let mut da_fare = vec![radice.clone()];
    while let Some(mut n) = da_fare.pop() {
        let figli = std::mem::take(&mut n.children);
        fuori.push(n);
        for f in figli.into_iter().rev() {
            da_fare.push(f);
        }
    }
    fuori
}
