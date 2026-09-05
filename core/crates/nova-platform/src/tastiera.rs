//! Premere i tasti, sapendo dove finiscono.
//!
//! **La regola di questo modulo, prima di ogni dettaglio tecnico.**
//! `SendInput` non ha un bersaglio: manda al sistema, e il sistema consegna a
//! chi ha il fuoco *in quel millisecondo*. Non si scrive senza aver prima
//! guardato chi c'e' davanti, e non si risponde «scritto nella finestra
//! attiva» — si risponde con il **nome** di quella finestra. Chi legge una
//! risposta che non nomina il bersaglio non ha modo di accorgersi che il
//! testo e' andato altrove.
//!
//! L'ho imparato sbagliando: misurando la strada vecchia ho scritto una riga
//! di prova senza verificare il fuoco, e quella riga e' finita in una finestra
//! che non era la mia. La verifica sta qui perche' e' qui che serve, non nella
//! testa di chi chiama.
//!
//! **Perche' non `SendKeys`.** La strada di prima passa da
//! `[System.Windows.Forms.SendKeys]`, che e' un piccolo linguaggio: `{`, `}`,
//! `+`, `^`, `%`, `~`, `(`, `)` hanno un significato e vanno protetti a mano,
//! con un elenco di sostituzioni scritto nel Python. Un elenco di
//! sostituzioni scritto a mano e' l'inizio di ogni guaio di virgolette
//! (D130). Qui il testo si manda **come Unicode**: `KEYEVENTF_UNICODE` non
//! interpreta niente, quindi non c'e' niente da proteggere — e passano gli
//! accenti, le virgolette basse e le emoji, che `SendKeys` non sa mandare.

#[cfg(windows)]
mod imp {
    use anyhow::{anyhow, Result};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
        KEYEVENTF_UNICODE, VIRTUAL_KEY,
    };

    fn evento(unita: u16, su: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0),
                    wScan: unita,
                    dwFlags: if su {
                        KEYEVENTF_UNICODE | KEYEVENTF_KEYUP
                    } else {
                        KEYEVENTF_UNICODE
                    },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    fn tasto(vk: u16, su: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(vk),
                    wScan: 0,
                    dwFlags: if su { KEYEVENTF_KEYUP } else { Default::default() },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    fn manda(eventi: &[INPUT]) -> Result<()> {
        if eventi.is_empty() {
            return Ok(());
        }
        let quanti = unsafe { SendInput(eventi, std::mem::size_of::<INPUT>() as i32) };
        if quanti as usize != eventi.len() {
            return Err(anyhow!(
                "il sistema ha accettato {quanti} eventi su {}: succede quando una \
                 finestra con privilegi piu' alti ha il fuoco, e li' non si scrive",
                eventi.len()
            ));
        }
        Ok(())
    }

    /// Scrive un testo, carattere per carattere, senza interpretarlo.
    ///
    /// Le emoji e gli altri caratteri fuori dal piano base viaggiano come due
    /// unita' — la coppia surrogata — e vanno mandate tutte e due: mandarne
    /// una sola non scrive mezzo carattere, scrive un carattere sbagliato.
    pub fn scrivi(testo: &str) -> Result<()> {
        scrivi_dentro(testo, None)
    }

    /// Scrive, e se `dove` e' dato **ricontrolla il fuoco a ogni blocco**.
    ///
    /// Controllarlo una volta sola non basta, e l'ho imparato guardando una
    /// prova fallire: il fuoco era quello giusto quando ho chiesto, e a meta'
    /// scrittura un'altra applicazione se l'e' ripreso. Windows puo' cambiare
    /// il primo piano fra due chiamate, e fra il controllo e `SendInput` c'e'
    /// sempre un fra.
    ///
    /// Non si puo' rendere impossibile — non esiste un modo di dire «manda
    /// questi tasti **a questa finestra**» — ma si puo' ridurre a un blocco:
    /// al massimo trentadue caratteri finiscono dove non dovevano, invece di
    /// tutto il testo, e chi chiama lo viene a sapere invece di leggere
    /// «fatto».
    pub fn scrivi_dentro(testo: &str, dove: Option<i64>) -> Result<()> {
        for pezzo in testo.chars().collect::<Vec<_>>().chunks(32) {
            if let Some(atteso) = dove {
                match crate::finestre::davanti() {
                    Ok(Some(f)) if f.handle == atteso => {}
                    Ok(Some(f)) => {
                        return Err(anyhow!(
                            "il fuoco e' passato a «{}» ({}) mentre scrivevo: mi sono                              fermato, ma una parte del testo puo' esserci gia' finita",
                            f.title, f.process
                        ))
                    }
                    Ok(None) => {
                        return Err(anyhow!(
                            "nessuna finestra ha piu' il fuoco mentre scrivevo: mi sono                              fermato"
                        ))
                    }
                    Err(e) => return Err(anyhow!("non riesco a controllare il fuoco: {e}")),
                }
            }
            let mut eventi = Vec::with_capacity(pezzo.len() * 4);
            for c in pezzo {
                let mut buf = [0u16; 2];
                for u in c.encode_utf16(&mut buf) {
                    eventi.push(evento(*u, false));
                    eventi.push(evento(*u, true));
                }
            }
            manda(&eventi)?;
        }
        Ok(())
    }

    /// Preme una combinazione: i modificatori scendono, il tasto scende e
    /// risale, i modificatori risalgono **in ordine inverso**.
    ///
    /// L'ordine inverso non e' eleganza: rilasciare `ctrl` prima di `shift` in
    /// una combinazione a tre lascia a Windows un istante in cui e' premuto un
    /// altro accordo. E se qualcosa va storto a meta', i modificatori vanno
    /// rilasciati lo stesso — un `ctrl` rimasto giu' rende la tastiera
    /// dell'utente inutilizzabile finche' non lo ripreme lui.
    pub fn combinazione(modificatori: &[u16], tasto_finale: u16) -> Result<()> {
        let mut eventi = Vec::new();
        for m in modificatori {
            eventi.push(tasto(*m, false));
        }
        eventi.push(tasto(tasto_finale, false));
        eventi.push(tasto(tasto_finale, true));
        for m in modificatori.iter().rev() {
            eventi.push(tasto(*m, true));
        }
        let esito = manda(&eventi);
        if esito.is_err() {
            // Il rilascio di sicurezza. Se il blocco sopra e' passato a meta',
            // qui si rimettono su i modificatori uno per uno, ignorando gli
            // errori: peggio di non riuscire a premere una combinazione c'e'
            // solo lasciare la tastiera bloccata.
            for m in modificatori.iter().rev() {
                let _ = manda(&[tasto(*m, true)]);
            }
        }
        esito
    }
}

#[cfg(not(windows))]
mod imp {
    use anyhow::{bail, Result};

    pub fn scrivi(_testo: &str) -> Result<()> {
        bail!("premere i tasti qui si fa in un altro modo")
    }
    pub fn scrivi_dentro(_testo: &str, _dove: Option<i64>) -> Result<()> {
        bail!("premere i tasti qui si fa in un altro modo")
    }
    pub fn combinazione(_modificatori: &[u16], _tasto: u16) -> Result<()> {
        bail!("premere i tasti qui si fa in un altro modo")
    }
}

pub use imp::{combinazione, scrivi, scrivi_dentro};

// ------------------------------------------------------------ i nomi dei tasti

/// I modificatori, col loro codice virtuale.
pub const MODIFICATORI: &[(&str, u16)] = &[
    ("ctrl", 0x11), ("control", 0x11), ("alt", 0x12), ("shift", 0x10),
    ("win", 0x5B), ("windows", 0x5B),
];

/// I tasti che hanno un nome invece di una lettera.
pub const TASTI: &[(&str, u16)] = &[
    ("enter", 0x0D), ("invio", 0x0D), ("return", 0x0D),
    ("esc", 0x1B), ("escape", 0x1B),
    ("tab", 0x09), ("space", 0x20), ("spazio", 0x20),
    ("backspace", 0x08), ("delete", 0x2E), ("del", 0x2E), ("canc", 0x2E),
    ("up", 0x26), ("down", 0x28), ("left", 0x25), ("right", 0x27),
    ("home", 0x24), ("end", 0x23), ("pageup", 0x21), ("pagedown", 0x22),
    ("insert", 0x2D), ("f1", 0x70), ("f2", 0x71), ("f3", 0x72), ("f4", 0x73),
    ("f5", 0x74), ("f6", 0x75), ("f7", 0x76), ("f8", 0x77), ("f9", 0x78),
    ("f10", 0x79), ("f11", 0x7A), ("f12", 0x7B),
];

/// Traduce «ctrl+shift+esc» in modificatori e tasto finale.
///
/// E' il pezzo dove sbagliare non da' errore: una traduzione storta preme
/// altri tasti, e li preme dove l'utente sta lavorando. Percio' i casi
/// dubbi si **rifiutano** invece di indovinare.
pub fn capisci(tasti: &str) -> Result<(Vec<u16>, u16), String> {
    let pezzi: Vec<String> = tasti
        .split('+')
        .map(|p| p.trim().to_lowercase())
        .filter(|p| !p.is_empty())
        .collect();
    if pezzi.is_empty() {
        return Err("combinazione vuota".into());
    }
    let mut modificatori = Vec::new();
    let mut finale: Option<u16> = None;
    for p in &pezzi {
        if let Some((_, vk)) = MODIFICATORI.iter().find(|(n, _)| *n == p) {
            if !modificatori.contains(vk) {
                modificatori.push(*vk);
            }
            continue;
        }
        if finale.is_some() {
            return Err(format!(
                "«{tasti}» ha due tasti oltre ai modificatori: non so quale premere"
            ));
        }
        finale = Some(if let Some((_, vk)) = TASTI.iter().find(|(n, _)| *n == p) {
            *vk
        } else if p.chars().count() == 1 {
            let c = p.chars().next().unwrap().to_ascii_uppercase();
            // Solo lettere e cifre: gli altri simboli hanno codici che
            // dipendono dalla disposizione della tastiera, e su una tastiera
            // italiana premerebbero un carattere diverso da quello chiesto.
            if c.is_ascii_alphanumeric() {
                c as u16
            } else {
                return Err(format!(
                    "«{p}» non e' una lettera o una cifra: il suo tasto cambia con la \
                     disposizione della tastiera, e premerlo alla cieca scriverebbe \
                     un altro carattere"
                ));
            }
        } else {
            return Err(format!("non conosco il tasto «{p}»"));
        });
    }
    match finale {
        // Una combinazione di soli modificatori non e' una combinazione:
        // premerla vorrebbe dire tenere giu' ctrl e non fare niente.
        None => Err(format!("«{tasti}» sono solo modificatori: manca il tasto da premere")),
        Some(f) => Ok((modificatori, f)),
    }
}

#[cfg(test)]
mod prove {
    use super::capisci;

    #[test]
    fn le_combinazioni_normali_si_capiscono() {
        assert_eq!(capisci("ctrl+s").unwrap(), (vec![0x11], b'S' as u16));
        assert_eq!(capisci("ctrl+shift+esc").unwrap(), (vec![0x11, 0x10], 0x1B));
        // 0x74 e non 0x70: 0x70 e' F1. La prima versione di questa riga
        // diceva 0x70 perche' l'avevo scritta a memoria invece di leggerla
        // dalla tabella qui sopra — e con un tasto funzione premuto al posto
        // di un altro nessuno solleva niente, si apre solo la cosa sbagliata.
        assert_eq!(capisci("f5").unwrap(), (vec![], 0x74));
        assert_eq!(capisci("f1").unwrap(), (vec![], 0x70));
        assert_eq!(capisci("invio").unwrap(), (vec![], 0x0D));
    }

    #[test]
    fn gli_spazi_e_le_maiuscole_non_contano() {
        assert_eq!(capisci("  CTRL + S ").unwrap(), (vec![0x11], b'S' as u16));
    }

    #[test]
    fn i_modificatori_ripetuti_non_si_ripetono() {
        assert_eq!(capisci("ctrl+control+s").unwrap(), (vec![0x11], b'S' as u16));
    }

    #[test]
    fn i_casi_dubbi_si_rifiutano_invece_di_indovinare() {
        // Sbagliare qui non da' errore: preme un altro tasto, nella finestra
        // dove l'utente sta lavorando.
        assert!(capisci("ctrl").is_err(), "soli modificatori");
        assert!(capisci("ctrl+alt").is_err());
        assert!(capisci("").is_err());
        assert!(capisci("+").is_err());
        assert!(capisci("ctrl+a+b").is_err(), "due tasti finali");
        assert!(capisci("ctrl+pippo").is_err(), "tasto sconosciuto");
    }

    #[test]
    fn i_simboli_si_rifiutano_perche_dipendono_dalla_tastiera() {
        // Su una tastiera italiana il tasto che sulla americana fa «;» fa
        // tutt'altro. Premere il codice virtuale alla cieca scriverebbe un
        // carattere diverso da quello chiesto, e in silenzio.
        assert!(capisci("ctrl+;").is_err());
        assert!(capisci("ctrl+/").is_err());
        // Per scrivere un carattere c'e' `scrivi`, che non interpreta niente.
    }
}
