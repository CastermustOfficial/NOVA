//! Spostare e nascondere finestre, e sapere che schermi ci sono.
//!
//! Non passa da UI Automation: sono chiamate Win32 su un handle, e non hanno
//! bisogno dell'apartment COM del thread dedicato. Stanno qui e non nel trait
//! `UiTree` perche' non sono «leggere l'albero»: sono governare la scena.
//!
//! Qui sta anche **l'elenco delle finestre aperte**. Stava dentro il backend
//! di UI Automation, e non era il suo posto: `EnumWindows` piu'
//! `GetWindowTextW` non toccano UIA, e chi voleva sapere che finestre ci sono
//! doveva far partire un thread COM e un'intera automazione per una domanda
//! che non ne ha bisogno. Non l'ho riscritta: l'ho spostata (D99).
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
        EnumWindows, GetClassNameW, GetWindowTextLengthW, GetWindowTextW,
        GetWindowThreadProcessId, IsWindow,
        IsWindowVisible, SetWindowPos, HWND_BOTTOM, MONITORINFOF_PRIMARY, SWP_NOACTIVATE,
        SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
    };
    use windows::Win32::Foundation::MAX_PATH;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use crate::WindowInfo;

    /// Le due classi di finestra che sono **lo sfondo del desktop**.
    ///
    /// `Progman` e' la finestra di Esplora risorse che disegna le icone del
    /// desktop, e ha per titolo «Program Manager»; `WorkerW` e' la sua gemella
    /// che compare quando lo sfondo e' animato. Sono visibili e hanno un
    /// titolo, quindi passano ogni altro filtro — ma nessuno che dica «che
    /// finestre ho aperte» intende quelle. Toglierle e' l'unica esclusione
    /// che questo elenco si permette, e sta scritta qui perche' un'esclusione
    /// taciuta e' un elenco che mente (D129).
    const SFONDO: [&str; 2] = ["Progman", "WorkerW"];

    /// Raccoglie gli handle delle finestre che una persona vedrebbe.
    ///
    /// Il filtro e' visibile **e** con un titolo, come prima, meno lo sfondo
    /// del desktop.
    unsafe extern "system" fn raccogli_finestre(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let elenco = &mut *(lparam.0 as *mut Vec<HWND>);
        if IsWindowVisible(hwnd).as_bool() && GetWindowTextLengthW(hwnd) > 0 {
            let mut classe = [0u16; 64];
            let n = GetClassNameW(hwnd, &mut classe);
            let nome = String::from_utf16_lossy(&classe[..n.max(0) as usize]);
            if !SFONDO.contains(&nome.as_str()) {
                elenco.push(hwnd);
            }
        }
        BOOL(1)
    }

    /// Il nome dell'eseguibile di un processo, senza il percorso.
    ///
    /// `PROCESS_QUERY_LIMITED_INFORMATION` e non `QUERY_INFORMATION`: il
    /// primo funziona anche sui processi di un altro livello di integrita',
    /// il secondo no. Con quello sbagliato l'elenco perderebbe in silenzio i
    /// nomi dei processi elevati.
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
        intero.rsplit(['\\', '/']).next().unwrap_or(&intero).to_string()
    }

    /// Le finestre di primo livello visibili, con titolo e processo.
    ///
    /// **L'ordine e' quello della pila**, dalla piu' in alto alla piu' in
    /// fondo: e' cosi' che `EnumWindows` le restituisce, ed e' un dato, non
    /// un caso. La strada di prima le ordinava per nome del processo e
    /// buttava via quell'informazione; chi chiede «che finestre ho aperte»
    /// quasi sempre intende quella davanti.
    pub fn elenca() -> Result<Vec<WindowInfo>> {
        unsafe {
            let mut handles: Vec<HWND> = Vec::new();
            EnumWindows(
                Some(raccogli_finestre),
                LPARAM(&mut handles as *mut Vec<HWND> as isize),
            )
            .map_err(|e| anyhow!("EnumWindows fallita: {e}"))?;

            let mut fuori = Vec::with_capacity(handles.len());
            for h in handles {
                // 512 e' quanto sta in un titolo che valga la pena leggere.
                // `GetWindowTextW` tronca da se' e non e' un problema: un
                // titolo piu' lungo di cosi' nessuno lo legge intero.
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
    }

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

    /// Sposta e ridimensiona senza dare il fuoco.
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

    pub fn elenca() -> Result<Vec<crate::WindowInfo>> {
        Ok(Vec::new())
    }

    pub fn sposta(_handle: i64, _posa: &Posa) -> Result<()> {
        bail!("spostamento finestre non ancora implementato per {}", std::env::consts::OS)
    }
}

pub fn schermi() -> Result<Vec<Schermo>> {
    imp::schermi()
}

/// Le finestre di primo livello visibili, con titolo, processo e pid.
///
/// Non passa da UI Automation: e' `EnumWindows` e basta. Chi vuole *guardare
/// dentro* una finestra chiede al trait `UiTree`; chi vuole solo sapere cosa
/// e' aperto chiede qui, e non paga un thread COM per farlo.
pub fn elenca() -> Result<Vec<crate::WindowInfo>> {
    imp::elenca()
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

