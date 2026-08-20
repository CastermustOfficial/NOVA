//! Segnaposto per i sistemi il cui backend non e' ancora scritto.
//!
//! Fallisce con un messaggio che dice *quale* API andra' usata, invece di
//! restituire un albero vuoto e far credere che la finestra non abbia
//! controlli.

use anyhow::{bail, Result};

use crate::{ElementRef, UiNode, UiQuery, UiTree, WindowInfo, WindowSel};

pub struct NonImplementato;

#[cfg(target_os = "macos")]
const API: &str = "Accessibility API (AXUIElement)";
#[cfg(target_os = "linux")]
const API: &str = "AT-SPI2 (via D-Bus)";
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
const API: &str = "l'API di accessibilita' di questo sistema";

fn manca<T>() -> Result<T> {
    bail!(
        "backend dell'albero di accessibilita' non ancora implementato per {}: \
         va scritto sopra {API}",
        std::env::consts::OS
    )
}

impl UiTree for NonImplementato {
    fn backend(&self) -> &'static str {
        "non-implementato"
    }

    fn windows(&self) -> Result<Vec<WindowInfo>> {
        manca()
    }

    fn tree(&self, _window: &WindowSel, _depth: usize) -> Result<UiNode> {
        manca()
    }

    fn find(&self, _window: &WindowSel, _query: &UiQuery, _limit: usize) -> Result<Vec<UiNode>> {
        manca()
    }

    fn invoke(&self, _target: &ElementRef) -> Result<()> {
        manca()
    }

    fn set_value(&self, _target: &ElementRef, _text: &str) -> Result<()> {
        manca()
    }

    fn focus(&self, _target: &ElementRef) -> Result<()> {
        manca()
    }
}
