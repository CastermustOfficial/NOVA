//! Spostare e nascondere finestre, e sapere che schermi ci sono.
//!
//! Non passa da UI Automation: sono chiamate Win32 su un handle, e non hanno
//! bisogno dell'apartment COM del thread dedicato. Stanno qui e non nel trait
//! `UiTree` perche' non sono «leggere l'albero»: sono governare la scena.
//!
//! La regola che conta e' una sola, ed e' `SWP_NOACTIVATE`: si sposta e si
//! ridimensiona una finestra **senza darle il fuoco**. E' cio' che permette a
//! NOVA di sistemarsi la propria finestra mentre l'operatore sta scrivendo
//! altrove, senza fargli saltare il cursore da un'altra parte.

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Un monitor, come lo vede il sistema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schermo {
    /// Origine in coordinate virtuali: il secondo monitor puo' stare a x negative.
    pub x: i32,
    pub y: i32,
    pub larghezza: i32,
    pub altezza: i32,
    /// L'area utilizzabile, cioe' tolta la barra delle applicazioni. E' questa
    /// che serve per posare una finestra, non l'area totale.
    pub lavoro: [i32; 4],
    pub principale: bool,
}

/// Dove mettere una finestra, e se lasciarla dietro.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Posa {
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub larghezza: Option<i32>,
    pub altezza: Option<i32>,
    /// Mandarla in fondo alla pila. Senza, resta dov'era nell'ordine.
    pub dietro: bool,
}

#[cfg(windows)]
mod imp {
    use super::{Posa, Schermo};
    use anyhow::{anyhow, Result};
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM, RECT, TRUE};
    use windows::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        IsWindow, SetWindowPos, HWND_BOTTOM, MONITORINFOF_PRIMARY, SWP_NOACTIVATE, SWP_NOMOVE,
        SWP_NOSIZE, SWP_NOZORDER,
    };

    unsafe extern "system" fn raccogli(
        h: HMONITOR,
        _dc: HDC,
        _r: *mut RECT,
        dati: LPARAM,
    ) -> BOOL {
        let elenco = &mut *(dati.0 as *mut Vec<Schermo>);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(h, &mut info).as_bool() {
            let m = info.rcMonitor;
            let l = info.rcWork;
            elenco.push(Schermo {
                x: m.left,
                y: m.top,
                larghezza: m.right - m.left,
                altezza: m.bottom - m.top,
                lavoro: [l.left, l.top, l.right - l.left, l.bottom - l.top],
                principale: info.dwFlags & MONITORINFOF_PRIMARY != 0,
            });
        }
        TRUE
    }

    pub fn schermi() -> Result<Vec<Schermo>> {
        let mut elenco: Vec<Schermo> = Vec::new();
        unsafe {
            EnumDisplayMonitors(
                None,
                None,
                Some(raccogli),
                LPARAM(&mut elenco as *mut Vec<Schermo> as isize),
            )
            .ok()
            .map_err(|e| anyhow!("EnumDisplayMonitors fallita: {e}"))?;
        }
        // Il principale per primo: e' l'ordine che si aspetta chi legge.
        elenco.sort_by_key(|s| !s.principale);
        Ok(elenco)
    }

    pub fn sposta(handle: i64, posa: &Posa) -> Result<()> {
        let h = HWND(handle as *mut std::ffi::c_void);
        unsafe {
            if !IsWindow(Some(h)).as_bool() {
                return Err(anyhow!("la finestra {handle} non esiste piu'"));
            }
            // NOACTIVATE e' il punto di tutta questa funzione: si mette a posto
            // una finestra senza portarla davanti e senza toglierti il fuoco.
            let mut flag = SWP_NOACTIVATE;
            let muove = posa.x.is_some() && posa.y.is_some();
            let ridimensiona = posa.larghezza.is_some() && posa.altezza.is_some();
            if !muove {
                flag |= SWP_NOMOVE;
            }
            if !ridimensiona {
                flag |= SWP_NOSIZE;
            }
            if !posa.dietro {
                flag |= SWP_NOZORDER;
            }
            SetWindowPos(
                h,
                if posa.dietro { Some(HWND_BOTTOM) } else { None },
                posa.x.unwrap_or(0),
                posa.y.unwrap_or(0),
                posa.larghezza.unwrap_or(0),
                posa.altezza.unwrap_or(0),
                flag,
            )
            .map_err(|e| anyhow!("SetWindowPos fallita: {e}"))
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::{Posa, Schermo};
    use anyhow::{bail, Result};

    pub fn schermi() -> Result<Vec<Schermo>> {
        bail!("elenco degli schermi non ancora implementato per {}", std::env::consts::OS)
    }

    pub fn sposta(_handle: i64, _posa: &Posa) -> Result<()> {
        bail!("spostamento finestre non ancora implementato per {}", std::env::consts::OS)
    }
}

pub fn schermi() -> Result<Vec<Schermo>> {
    imp::schermi()
}

pub fn sposta(handle: i64, posa: &Posa) -> Result<()> {
    imp::sposta(handle, posa)
}

/// Lo schermo su cui cade il centro di un rettangolo, se ce n'e' uno.
pub fn schermo_di(x: i32, y: i32) -> Result<Option<Schermo>> {
    Ok(schermi()?
        .into_iter()
        .find(|s| x >= s.x && x < s.x + s.larghezza && y >= s.y && y < s.y + s.altezza))
}

/// Lo schermo dove NOVA dovrebbe lavorare: il secondario se c'e', altrimenti
/// il principale.
///
/// Con due monitor l'operatore puo' girarsi e guardare cosa sta combinando
/// senza cambiare contesto; con uno solo non resta che stare dietro.
pub fn schermo_di_lavoro() -> Result<Option<Schermo>> {
    let s = schermi()?;
    Ok(s.iter().find(|m| !m.principale).cloned().or_else(|| s.first().cloned()))
}

