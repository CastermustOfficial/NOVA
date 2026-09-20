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

// Governare la scena — spostare, mandare dietro, sapere che schermi ci sono —
// non e' leggere l'albero: sta in un modulo suo, e non passa da COM.
// Che dischi ci sono: stessa famiglia di domanda, stessa forma di risposta.
// Chi cerca i modelli sul disco chiede qui, e poi non parla piu' col sistema.
pub mod dischi;

// Quanta memoria video c'e' davvero: la domanda da cui dipende quanti
// strati del modello vanno sulla scheda, e finora l'unica a cui
// rispondeva un programma di NVIDIA.
pub mod gpu;

// Gli appunti, chiamati direttamente: niente processo, niente shell, niente
// stringa da comporre. Vedi D130.
pub mod appunti;
/// Appunti, volume e notifiche su Linux e macOS. Li tiene l'ambiente
/// grafico, e l'ambiente grafico e' un programma esterno: si provano gli
/// strumenti noti in ordine, e se non ce n'e' nessuno si dice **quali** si
/// sono cercati invece di dire «non disponibile» (D193).
#[cfg(unix)]
pub mod scrivania_unix;

// Il volume, chiesto a Core Audio invece che simulato a colpi di tasto.
pub mod audio;

// Le notifiche: il fumetto resta quello, ma ad aspettare che finisca non e'
// piu' NOVA.
pub mod notifiche;

// Com'e' fatto il PC. Era la capacita' piu' cara di tutte: 1.543 ms.
pub mod sistema;
/// La meta' Unix: `/proc` e `/sys` su Linux, `sysctl` e `sw_vers` su macOS.
/// Le regole di lettura stanno separate da quel che apre i file, perche' un
/// Mac da qui non si puo' provare e il testo che risponde si' (D209).
#[cfg(unix)]
pub mod sistema_unix;

// Il registro, letto in un posto solo: `sistema` ne aveva gia' una copia
// privata, e alla seconda occorrenza si mette in comune (D62).
pub mod registro;

// Che applicazioni sono installate: gli stessi tre rami di registro che
// leggeva PowerShell, letti direttamente.
pub mod applicazioni;

// I processi. Qui la selezione e l'azione sono separate di proposito: si
// elenca, si guarda, e si chiude **un pid** — non un modello di ricerca
// (D141).
pub mod processi;
/// La meta' Unix: `/proc` su Linux, `ps` su macOS, e `kill` — che ha un modo
/// di spegnere tutto che Windows non ha, e va rifiutato per nome (D258).
#[cfg(unix)]
pub mod processi_unix;

// La tastiera. Qui la regola non e' tecnica: non si preme un tasto senza aver
// prima guardato chi ha il fuoco, e la risposta nomina la finestra.
pub mod tastiera;

// Il Cestino: cancellare in un modo che si puo' disfare. Premessa N2 —
// prima la reversibilita', poi il permesso.
pub mod cestino;
/// La meta' Unix del Cestino: la specifica freedesktop su Linux, `~/.Trash`
/// su macOS. Sta in un modulo suo perche' ha una regola in piu' da
/// spiegare, e e' quella che protegge i dati di qualcuno: **non si copia e
/// poi si cancella**.
#[cfg(unix)]
pub mod cestino_unix;

// Se i byte di un file stanno qui o nel cloud. Serve prima di leggere un
// modello da dodici gigabyte che una cartella sincronizzata puo' aver
// «liberato» lasciando un segnaposto al suo posto.
pub mod nuvola;
pub use nuvola::segnaposto;

pub mod finestre;
pub use finestre::{schermi, schermo_di, schermo_di_lavoro, sposta, Posa, Schermo};

// Che ore sono per chi guarda l'orologio. Il fuso e' una domanda di sistema,
// come tutte quelle che qui dentro hanno una risposta diversa per ogni OS, e
// serve al registro delle azioni: quel file lo scrivono tutte e due le meta'
// di NOVA, e una delle due scrive l'ora di casa.
pub mod orologio;
pub use orologio::fuso_secondi;

// Il recinto: cosa un processo figlio puo' toccare, deciso dal kernel invece
// che da una nostra stringa. E' l'unica guardia che vale anche **dopo** che
// il comando e' partito.
pub mod recinto;

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
        if !self.name.is_empty() && !n.name.to_lowercase().contains(&self.name.to_lowercase()) {
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
