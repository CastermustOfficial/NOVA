//! Appunti, volume e notifiche su Linux e macOS.
//!
//! Qui, a differenza del Cestino e dei processi, non c'e' una chiamata di
//! sistema da fare: queste tre cose le tiene l'ambiente grafico, e l'ambiente
//! grafico e' un programma esterno. Su macOS ce n'e' sempre uno solo e c'e'
//! sempre (`pbcopy`, `osascript`); su Linux dipende da cosa e' installato —
//! Wayland o X11, PipeWire o ALSA — e **non c'e' un modo di saperlo che non
//! sia provare**.
//!
//! Da qui la regola di questo modulo: si provano gli strumenti noti in
//! ordine, e se non ce n'e' nessuno l'errore **dice quali si sono cercati**.
//! «Gli appunti non sono disponibili» manda qualcuno a cercare un guasto;
//! «non trovo ne' `wl-copy` ne' `xclip` ne' `xsel`» gli dice cosa installare
//! (D193). E soprattutto: **non si finge**. Un volume che torna 50 sempre, o
//! degli appunti che tornano vuoti invece di dire che non si sono letti,
//! sono peggio di un errore — sono un errore che nessuno vede.

#![cfg(unix)]

use anyhow::{anyhow, bail, Result};
use std::io::Write;
use std::process::{Command, Stdio};

/// Chi sa leggere e scrivere gli appunti, in ordine di preferenza.
///
/// Wayland per primo: su una sessione Wayland `xclip` a volte c'e' lo stesso
/// (per Xwayland) e parla con gli appunti **sbagliati**, quelli delle
/// applicazioni X. Chi copia da Firefox su Wayland non ritroverebbe niente.
pub const APPUNTI: [(&str, &str); 3] = [
    ("wl-copy", "wl-paste"),
    ("xclip", "xclip"),
    ("xsel", "xsel"),
];

/// Chi sa il volume, in ordine.
pub const VOLUME: [&str; 2] = ["wpctl", "pactl"];

/// Chi sa mostrare una notifica, in ordine.
pub const NOTIFICHE: [&str; 2] = ["notify-send", "kdialog"];

/// La frase per quando non c'e' nessuno degli strumenti che servono.
///
/// Dice **cosa** manca e **a cosa serviva**, perche' l'elenco dei nomi da
/// solo non basta a chi non li ha mai sentiti.
pub fn nessuno_di(a_che_serve: &str, cercati: &[&str]) -> String {
    format!(
        "per {a_che_serve} su questo sistema serve uno fra {}, e non ne trovo \
         nessuno. Installane uno e riprova",
        cercati.join(", ")
    )
}

fn ce(programma: &str) -> bool {
    Command::new(programma)
        .arg("--help")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .status()
        .is_ok()
}

fn uscita(programma: &str, argomenti: &[&str]) -> Result<String> {
    let u = Command::new(programma)
        .args(argomenti)
        .output()
        .map_err(|e| anyhow!("«{programma}» non si e' avviato: {e}"))?;
    if !u.status.success() {
        bail!(
            "«{programma}» ha risposto male: {}",
            String::from_utf8_lossy(&u.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&u.stdout).to_string())
}

fn dentro(programma: &str, argomenti: &[&str], testo: &str) -> Result<()> {
    let mut figlio = Command::new(programma)
        .args(argomenti)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| anyhow!("«{programma}» non si e' avviato: {e}"))?;
    figlio
        .stdin
        .as_mut()
        .ok_or_else(|| anyhow!("«{programma}» non accetta niente da scrivere"))?
        .write_all(testo.as_bytes())?;
    // `wl-copy` resta in piedi finche' tiene gli appunti: aspettarlo vuol
    // dire non tornare mai. Si aspetta solo chi finisce.
    if programma == "wl-copy" {
        return Ok(());
    }
    let esito = figlio.wait()?;
    if !esito.success() {
        bail!("«{programma}» non ha accettato il testo");
    }
    Ok(())
}

// ------------------------------------------------------------- gli appunti

/// Cosa c'e' negli appunti. `None` vuol dire «ci sono, e sono vuoti».
pub fn appunti_leggi() -> Result<Option<String>> {
    if cfg!(target_os = "macos") {
        let t = uscita("pbpaste", &[])?;
        return Ok((!t.is_empty()).then_some(t));
    }
    for (scrive, legge) in APPUNTI {
        if !ce(scrive) {
            continue;
        }
        let t = if legge == "xclip" {
            uscita("xclip", &["-selection", "clipboard", "-o"])
        } else if legge == "xsel" {
            uscita("xsel", &["--clipboard", "--output"])
        } else {
            uscita(legge, &["--no-newline"])
        };
        // Gli appunti vuoti fanno uscire male sia `xclip` sia `wl-paste`:
        // e' «non c'e' niente», non «non ci sono riuscita».
        let t = t.unwrap_or_default();
        return Ok((!t.is_empty()).then_some(t));
    }
    let cercati: Vec<&str> = APPUNTI.iter().map(|(s, _)| *s).collect();
    bail!("{}", nessuno_di("leggere gli appunti", &cercati))
}

/// Mette del testo negli appunti.
pub fn appunti_scrivi(testo: &str) -> Result<()> {
    if cfg!(target_os = "macos") {
        return dentro("pbcopy", &[], testo);
    }
    for (scrive, _) in APPUNTI {
        if !ce(scrive) {
            continue;
        }
        return match scrive {
            "xclip" => dentro("xclip", &["-selection", "clipboard"], testo),
            "xsel" => dentro("xsel", &["--clipboard", "--input"], testo),
            _ => dentro(scrive, &[], testo),
        };
    }
    let cercati: Vec<&str> = APPUNTI.iter().map(|(s, _)| *s).collect();
    bail!("{}", nessuno_di("scrivere negli appunti", &cercati))
}

// ---------------------------------------------------------------- il volume

/// Il volume che `wpctl get-volume` racconta: `Volume: 0.45 [MUTED]`.
pub fn volume_da_wpctl(riga: &str) -> Option<(u8, bool)> {
    let dopo = riga.split_once(':')?.1;
    let numero = dopo.split_whitespace().next()?;
    let v: f32 = numero.parse().ok()?;
    let muto = riga.to_uppercase().contains("MUTED");
    Some(((v * 100.0).round().clamp(0.0, 100.0) as u8, muto))
}

/// Il volume che `pactl get-sink-volume @DEFAULT_SINK@` racconta.
///
/// La riga ha due canali — «front-left: 32768 / 50% / -18.06 dB, front-right:
/// ...» — e si prende il **primo**: mediarli darebbe un numero che nessuno
/// slider mostra, e riscriverlo poi rimetterebbe i due canali pari, cioe'
/// cancellerebbe il bilanciamento di chi l'aveva impostato.
///
/// Si cerca **il primo pezzo che finisce per `%`**, e non il secondo campo
/// fra le barre. Le due cose danno la stessa risposta sul formato di oggi, e
/// non su quello di ieri: `pactl` piu' vecchi scrivono «Volume: 0: 50% 1:
/// 50%», senza nessuna barra. Contare i campi vuol dire non leggere niente
/// su quelle macchine — e sono proprio le macchine dove uno si aspetta che
/// qualcosa non funzioni, quindi nessuno andrebbe a cercare qui.
pub fn volume_da_pactl(testo: &str, muto: &str) -> Option<(u8, bool)> {
    let percento = testo
        .split(['/', ' ', '\t', ','])
        .map(str::trim)
        .find(|p| p.ends_with('%') && p.len() > 1)?
        .trim_end_matches('%')
        .parse::<u32>()
        .ok()?;
    Some((percento.min(100) as u8, muto.to_lowercase().contains("yes")))
}

/// Quanto e' alto il volume, e se e' muto.
pub fn volume_stato() -> Result<(u8, bool)> {
    if cfg!(target_os = "macos") {
        let v = uscita(
            "osascript",
            &["-e", "output volume of (get volume settings)"],
        )?;
        let m = uscita(
            "osascript",
            &["-e", "output muted of (get volume settings)"],
        )?;
        let livello: u8 = v.trim().parse().unwrap_or(0);
        return Ok((livello.min(100), m.trim() == "true"));
    }
    if ce("wpctl") {
        if let Ok(t) = uscita("wpctl", &["get-volume", "@DEFAULT_AUDIO_SINK@"]) {
            if let Some(x) = volume_da_wpctl(&t) {
                return Ok(x);
            }
        }
    }
    if ce("pactl") {
        let v = uscita("pactl", &["get-sink-volume", "@DEFAULT_SINK@"])?;
        let m = uscita("pactl", &["get-sink-mute", "@DEFAULT_SINK@"]).unwrap_or_default();
        if let Some(x) = volume_da_pactl(&v, &m) {
            return Ok(x);
        }
    }
    bail!("{}", nessuno_di("leggere il volume", &VOLUME))
}

/// Mette il volume a questo livello.
pub fn volume_imposta(livello: u8) -> Result<()> {
    let livello = livello.min(100);
    if cfg!(target_os = "macos") {
        return uscita(
            "osascript",
            &["-e", &format!("set volume output volume {livello}")],
        )
        .map(|_| ());
    }
    if ce("wpctl") {
        return uscita(
            "wpctl",
            &[
                "set-volume",
                "@DEFAULT_AUDIO_SINK@",
                &format!("{}%", livello),
            ],
        )
        .map(|_| ());
    }
    if ce("pactl") {
        return uscita(
            "pactl",
            &["set-sink-volume", "@DEFAULT_SINK@", &format!("{livello}%")],
        )
        .map(|_| ());
    }
    bail!("{}", nessuno_di("cambiare il volume", &VOLUME))
}

/// Silenzia, o toglie il silenzio.
pub fn volume_muto(muto: bool) -> Result<()> {
    if cfg!(target_os = "macos") {
        let v = if muto { "true" } else { "false" };
        return uscita(
            "osascript",
            &["-e", &format!("set volume output muted {v}")],
        )
        .map(|_| ());
    }
    let acceso = if muto { "1" } else { "0" };
    if ce("wpctl") {
        return uscita("wpctl", &["set-mute", "@DEFAULT_AUDIO_SINK@", acceso]).map(|_| ());
    }
    if ce("pactl") {
        return uscita("pactl", &["set-sink-mute", "@DEFAULT_SINK@", acceso]).map(|_| ());
    }
    bail!("{}", nessuno_di("silenziare l'audio", &VOLUME))
}

// ------------------------------------------------------------- le notifiche

/// Mostra una notifica di sistema.
///
/// Il titolo e il messaggio vanno come **argomenti**, non dentro un copione:
/// su macOS `osascript` esegue AppleScript, cioe' un linguaggio, e un
/// messaggio con dentro una virgoletta lo romperebbe — lo stesso difetto che
/// l'altra meta' racconta per PowerShell (D130). Le virgolette e le barre si
/// proteggono, ed e' l'unico posto di questo modulo in cui si compone
/// qualcosa: la si compone sapendolo.
pub fn notifica(titolo: &str, messaggio: &str, millisecondi: u32) -> Result<()> {
    if cfg!(target_os = "macos") {
        let copione = format!(
            "display notification \"{}\" with title \"{}\"",
            per_applescript(messaggio),
            per_applescript(titolo)
        );
        return uscita("osascript", &["-e", &copione]).map(|_| ());
    }
    if ce("notify-send") {
        let durata = millisecondi.to_string();
        return uscita("notify-send", &["-t", &durata, titolo, messaggio]).map(|_| ());
    }
    if ce("kdialog") {
        let secondi = (millisecondi / 1000).max(1).to_string();
        return uscita(
            "kdialog",
            &["--title", titolo, "--passivepopup", messaggio, &secondi],
        )
        .map(|_| ());
    }
    bail!("{}", nessuno_di("mostrare una notifica", &NOTIFICHE))
}

/// Il testo come lo vuole una stringa AppleScript.
pub fn per_applescript(testo: &str) -> String {
    testo.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn chi_non_ha_gli_strumenti_sente_quali_cercavo() {
        let m = nessuno_di("leggere gli appunti", &["wl-copy", "xclip"]);
        assert!(m.contains("wl-copy, xclip"), "{m}");
        assert!(m.contains("Installane uno"), "{m}");
    }

    #[test]
    fn wayland_viene_prima_di_x11() {
        // Su una sessione Wayland `xclip` spesso c'e' lo stesso, per
        // Xwayland, e parla con gli appunti sbagliati: chi copia da Firefox
        // non ritroverebbe niente.
        assert_eq!(APPUNTI[0].0, "wl-copy");
    }

    #[test]
    fn il_volume_di_wpctl_si_legge() {
        assert_eq!(volume_da_wpctl("Volume: 0.45"), Some((45, false)));
        assert_eq!(volume_da_wpctl("Volume: 0.45 [MUTED]"), Some((45, true)));
        assert_eq!(volume_da_wpctl("Volume: 1.00"), Some((100, false)));
        // Oltre 1.0 si puo' andare, ma un volume al 150% su uno slider che
        // arriva a 100 non si puo' mostrare.
        assert_eq!(volume_da_wpctl("Volume: 1.50"), Some((100, false)));
        assert_eq!(volume_da_wpctl("Volume: 0.005"), Some((1, false)));
        assert_eq!(volume_da_wpctl("niente"), None);
    }

    #[test]
    fn e_quello_di_pactl_pure() {
        let t = "Volume: front-left: 32768 /  50% / -18.06 dB,   \
                 front-right: 16384 /  25% / -24.08 dB";
        // Il primo canale, non la media: mediarli e poi riscriverli
        // cancellerebbe il bilanciamento di chi l'aveva impostato.
        assert_eq!(volume_da_pactl(t, "Mute: no"), Some((50, false)));
        assert_eq!(volume_da_pactl(t, "Mute: yes"), Some((50, true)));
        assert_eq!(volume_da_pactl("boh", "Mute: no"), None);
        // E il formato vecchio, che di barre non ne ha nessuna. Contando i
        // campi qui non si legge niente — e sono proprio le macchine dove
        // nessuno andrebbe a cercare il motivo.
        assert_eq!(
            volume_da_pactl("Volume: 0: 37% 1: 37%", "Mute: no"),
            Some((37, false))
        );
        // Un volume oltre cento si mostra a cento: uno slider che arriva a
        // 100 non sa disegnare 150.
        let forte = "Volume: front-left: 98304 / 150% / 3.5 dB";
        assert_eq!(volume_da_pactl(forte, "Mute: no"), Some((100, false)));
        // «%» da solo non e' un numero.
        assert_eq!(volume_da_pactl("Volume: % ", "Mute: no"), None);
    }

    #[test]
    fn una_virgoletta_nel_messaggio_non_rompe_applescript() {
        // E' il difetto che l'altra meta' racconta per PowerShell (D130), e
        // qui e' l'unico posto in cui si compone davvero qualcosa.
        assert_eq!(per_applescript("dice \"ciao\""), "dice \\\"ciao\\\"");
        assert_eq!(per_applescript("C:\\x"), "C:\\\\x");
        assert_eq!(per_applescript("normale"), "normale");
    }
}
