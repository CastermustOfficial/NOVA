//! Lanciare un cervello che e' un **programma**, non un indirizzo.
//!
//! Meta' della scala di NOVA non sta dietro a un URL: sono CLI agentiche che
//! si avviano, ricevono un prompt e stampano una risposta (D219). Da qui in
//! poi il mondo e' pieno di spigoli che non c'entrano niente col pensare —
//! il PATH, una finestra nera che si apre, un processo che non finisce — e
//! stanno tutti in questo file, sottile apposta.
//!
//! Cio' che si puo' decidere senza una macchina non sta qui: come si compone
//! il prompt, com'e' fatta la riga di comando, cosa vuol dire un'uscita
//! vuota, stanno in [`nova_cervelli::cli`] e si provano da soli.

#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;

/// Cosa ha lasciato un programma quando e' finito.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Uscita {
    pub stdout: String,
    pub stderr: String,
    pub codice: Option<i32>,
}

/// Il programma vero dietro un nome, cercato nel PATH.
///
/// Un percorso assoluto si prende per quello che e' **solo se esiste**: un
/// assoluto sbagliato tornato com'e' diventa un «file non trovato» del
/// sistema operativo, cioe' lo stesso guasto raccontato peggio.
///
/// L'ordine dei suffissi lo decide [`nova_cervelli::cli::candidati`]: su
/// Windows npm installa un `.cmd`, ed e' quello che si puo' eseguire.
pub fn trova(binario: &str) -> String {
    let b = binario.trim();
    if b.is_empty() {
        return String::new();
    }
    if Path::new(b).is_absolute() {
        return if Path::new(b).is_file() {
            b.to_string()
        } else {
            String::new()
        };
    }
    let Some(path) = std::env::var_os("PATH") else {
        return String::new();
    };
    for cartella in std::env::split_paths(&path) {
        for nome in nova_cervelli::cli::candidati(b) {
            let quale = cartella.join(&nome);
            if quale.is_file() {
                return quale.to_string_lossy().to_string();
            }
        }
    }
    String::new()
}

/// La cartella da cui si lancia: quella dichiarata, o quella dell'utente.
fn da_dove(cartella: &str) -> Option<std::path::PathBuf> {
    if !cartella.trim().is_empty() {
        return Some(std::path::PathBuf::from(cartella.trim()));
    }
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)
}

/// Lancia il programma e aspetta che finisca, non oltre `secondi`.
///
/// Il primo argomento e' l'eseguibile: e' la forma che torna
/// [`nova_cervelli::cli::argomenti`], e tenerla vuol dire che la riga che si
/// legge nel diario e' la riga che e' partita.
///
/// **Il processo si ammazza se sfora.** Lasciarlo vivere dopo aver smesso di
/// aspettarlo lo lascerebbe ad agire sul computer dell'utente mentre NOVA
/// racconta di aver rinunciato: due padroni e nessuno che guarda.
pub async fn lancia(
    args: &[String],
    su_stdin: Option<&str>,
    cartella: &str,
    secondi: u64,
) -> Result<Uscita, Guaio> {
    let Some((eseguibile, resto)) = args.split_first() else {
        return Ok(Uscita::default());
    };
    let mut c = tokio::process::Command::new(eseguibile);
    c.args(resto);
    if let Some(dove) = da_dove(cartella) {
        c.current_dir(dove);
    }
    // Una CLI agentica che scrive in casa dell'utente non deve mandare piu'
    // di quanto serve a chi l'ha fatta: e' la stessa riga che il Python
    // mette per Claude Code, e vale per tutte.
    c.env("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1");
    c.stdin(if su_stdin.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    })
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        // `claude.cmd` e' un file batch: senza questo ogni turno di NOVA
        // apriva un rettangolo nero sullo schermo dell'utente.
        const SENZA_FINESTRA: u32 = 0x0800_0000;
        c.creation_flags(SENZA_FINESTRA);
    }
    let mut figlio = c.spawn().map_err(|e| Guaio::Muto(e.to_string()))?;
    if let Some(testo) = su_stdin {
        if let Some(mut dentro) = figlio.stdin.take() {
            // Un tubo chiuso dall'altra parte non e' un guasto di NOVA: vuol
            // dire che il programma ha gia' deciso di non leggere, e cio'
            // che ha da dire sta su stdout o stderr. Si prosegue e si
            // guarda li'.
            let _ = dentro.write_all(testo.as_bytes()).await;
            let _ = dentro.shutdown().await;
        }
    }
    let atteso = tokio::time::timeout(
        std::time::Duration::from_secs(secondi.max(1)),
        figlio.wait_with_output(),
    )
    .await;
    let finito = match atteso {
        Err(_) => return Err(Guaio::Troppo),
        Ok(r) => r.map_err(|e| Guaio::Muto(e.to_string()))?,
    };
    Ok(Uscita {
        stdout: String::from_utf8_lossy(&finito.stdout).to_string(),
        stderr: String::from_utf8_lossy(&finito.stderr).to_string(),
        codice: finito.status.code(),
    })
}

/// Perche' non c'e' un'uscita da leggere.
///
/// Sono due cose diverse e si raccontano diverse: «ci ha messo troppo» e'
/// un'attesa finita, e chi chiama sa quanto ha aspettato e come si chiama
/// quel cervello; «non e' proprio partito» e' un guasto e il sistema
/// operativo lo ha gia' spiegato.
#[derive(Debug, Clone, PartialEq)]
pub enum Guaio {
    /// Ci ha messo troppo, ed e' stato fermato.
    Troppo,
    /// Non e' partito affatto, e il sistema operativo dice perche'.
    Muto(String),
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn un_assoluto_che_non_esiste_non_e_un_eseguibile() {
        assert_eq!(trova("/non/esiste/affatto/gemini"), "");
        assert_eq!(trova("  "), "");
    }

    #[test]
    fn nel_path_si_trova_quel_che_ce() {
        // `sh` c'e' su ogni macchina che compila questo, Windows compreso
        // quando la prova gira in CI su un runner Unix; se un giorno non ci
        // fosse, questa prova direbbe il falso invece di fallire, quindi si
        // guarda anche il contrario.
        let trovato = trova("sh");
        if cfg!(unix) {
            assert!(
                trovato.ends_with("sh"),
                "«sh» dovrebbe essere nel PATH: {trovato:?}"
            );
            assert!(Path::new(&trovato).is_absolute());
        }
        assert_eq!(trova("non-esiste-questo-programma-qui"), "");
    }

    #[tokio::test]
    async fn il_prompt_arriva_su_stdin_e_luscita_torna_indietro() {
        if !cfg!(unix) {
            return;
        }
        let args = vec!["/bin/cat".to_string()];
        let u = lancia(&args, Some("ciao NOVA"), "", 20).await.unwrap();
        assert_eq!(u.stdout, "ciao NOVA");
        assert_eq!(u.codice, Some(0));
    }

    #[tokio::test]
    async fn chi_ci_mette_troppo_viene_fermato() {
        if !cfg!(unix) {
            return;
        }
        let args = vec!["/bin/sh".into(), "-c".into(), "sleep 30".into()];
        let quando = std::time::Instant::now();
        assert_eq!(lancia(&args, None, "", 1).await, Err(Guaio::Troppo));
        assert!(
            quando.elapsed().as_secs() < 10,
            "non ha smesso di aspettare"
        );
    }

    #[tokio::test]
    async fn un_programma_che_non_ce_lo_dice_subito() {
        let args = vec!["non-esiste-questo-programma-qui".to_string()];
        assert!(matches!(
            lancia(&args, None, "", 5).await,
            Err(Guaio::Muto(_))
        ));
    }
}
