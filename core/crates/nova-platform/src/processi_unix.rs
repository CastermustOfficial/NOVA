//! I processi fuori da Windows: elencarli, chiuderne **uno**, avviarne uno.
//!
//! Il grassetto su «uno» e' lo stesso dell'altra meta', e qui c'e' un modo in
//! piu' di sbagliarlo che Windows non ha. `kill` prende un intero con segno,
//! e sotto lo zero quell'intero non e' un processo:
//!
//! - `kill(0, ...)` lo manda a **tutto il gruppo** di chi chiama;
//! - `kill(-1, ...)` a **ogni processo** che l'utente puo' segnalare;
//! - `kill(-n, ...)` a tutto il gruppo `n`.
//!
//! Il tipo qui e' un `u32`, quindi i negativi non arrivano — ma lo zero si'.
//! Uno zero puo' venire da un numero non letto, da un campo vuoto, da un
//! `parse` andato male con un `unwrap_or(0)`: e il risultato sarebbe NOVA che
//! spegne tutta la sua sessione per un campo vuoto. Si rifiuta, e si dice
//! perche' (D258).

#![cfg(unix)]

use crate::processi::Processo;
use anyhow::{anyhow, bail, Result};

/// I pid che non si segnalano mai, e cosa sono davvero.
///
/// Lo zero non e' un processo: e' «tutto il mio gruppo». L'uno e' il primo
/// processo del sistema — su una macchina e' `init`, dentro un contenitore e'
/// il programma che tiene in piedi il contenitore stesso: fermarlo vuol dire
/// spegnere tutto, compresa NOVA che sta eseguendo il comando.
pub fn perche_non_si_tocca(pid: u32) -> Option<&'static str> {
    match pid {
        0 => Some(
            "il pid 0 non e' un processo: e' «tutto il gruppo di chi chiede». \
             Fermarlo vorrebbe dire fermare anche me, e tutto quello che ho \
             avviato. Se volevi un processo preciso, cercalo per nome ed \
             elencalo prima",
        ),
        1 => Some(
            "il pid 1 e' il primo processo del sistema: su una macchina e' \
             quello che tiene acceso tutto il resto, dentro un contenitore e' \
             il contenitore stesso. Fermarlo non chiude un programma, spegne \
             la macchina",
        ),
        _ => None,
    }
}

/// Il nome e la memoria di un processo, letti da `/proc/<pid>`.
///
/// `comm` e non la riga di comando: la riga di comando di un processo e'
/// **testo scelto da chi l'ha avviato**, e qui dentro finisce in un elenco
/// che poi qualcuno legge per decidere cosa chiudere. Una riga di comando
/// con dentro `chrome.exe` non deve poter far sembrare `chrome` un processo
/// che non e'.
pub fn da_proc(comm: &str, statm: &str, pagina: u64) -> (String, u64) {
    let nome = comm.trim().to_string();
    // `statm` e' una riga di numeri: il secondo e' quanto sta in memoria
    // davvero, in pagine.
    let residenti: u64 = statm
        .split_whitespace()
        .nth(1)
        .and_then(|x| x.parse().ok())
        .unwrap_or(0);
    (nome, residenti * pagina)
}

/// Una riga di `ps -axo pid=,rss=,comm=`, come la da' macOS.
///
/// Il nome puo' contenere spazi — «Google Chrome Helper» — quindi i primi
/// due campi si prendono da sinistra e **tutto il resto e' il nome**.
/// Dividere per spazi e prendere il terzo campo tronca proprio i nomi che
/// una persona riconosce.
pub fn da_riga_ps(riga: &str) -> Option<Processo> {
    // Non `splitn(3, char::is_whitespace)`: `ps` allinea le colonne con piu'
    // spazi, e quello spezza sul **primo** spazio di ogni gruppo lasciando
    // campi vuoti in mezzo. I due numeri si staccano a mano, e tutto quel
    // che resta e' il nome.
    let resto = riga.trim_start();
    let (numero, resto) = resto.split_once(char::is_whitespace)?;
    let pid: u32 = numero.parse().ok()?;
    let resto = resto.trim_start();
    let (kb, nome) = match resto.split_once(char::is_whitespace) {
        Some((forse, dopo)) => match forse.parse::<u64>() {
            Ok(k) => (k, dopo.trim()),
            // Il secondo campo non era un numero: `rss` mancava, e quello
            // era gia' l'inizio del nome.
            Err(_) => (0, resto),
        },
        None => (0, resto),
    };
    // Il percorso completo non serve a chi legge un elenco per decidere:
    // «Google Chrome Helper» si riconosce, `/Applications/...` no.
    let nome = nome.rsplit('/').next().unwrap_or(nome).trim().to_string();
    if nome.is_empty() {
        return None;
    }
    Some(Processo {
        pid,
        nome,
        memoria_byte: kb * 1024,
    })
}

fn pagina() -> u64 {
    let n = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if n > 0 {
        n as u64
    } else {
        4096
    }
}

fn da_proc_fs() -> Result<Vec<Processo>> {
    let pagina = pagina();
    let mut fuori = Vec::new();
    for voce in std::fs::read_dir("/proc")?.flatten() {
        let nome_cartella = voce.file_name();
        let Some(testo) = nome_cartella.to_str() else {
            continue;
        };
        let Ok(pid) = testo.parse::<u32>() else {
            continue;
        };
        // Un processo puo' finire fra il momento in cui compare nell'elenco
        // e quello in cui lo si legge: non e' un errore, e' la vita normale
        // di una fotografia. Si salta e si va avanti.
        let Ok(comm) = std::fs::read_to_string(voce.path().join("comm")) else {
            continue;
        };
        let statm = std::fs::read_to_string(voce.path().join("statm")).unwrap_or_default();
        let (nome, memoria_byte) = da_proc(&comm, &statm, pagina);
        if nome.is_empty() {
            continue;
        }
        fuori.push(Processo {
            pid,
            nome,
            memoria_byte,
        });
    }
    fuori.sort_by_key(|p| p.pid);
    Ok(fuori)
}

fn da_ps() -> Result<Vec<Processo>> {
    let uscita = std::process::Command::new("ps")
        .args(["-axo", "pid=,rss=,comm="])
        .output()
        .map_err(|e| anyhow!("non riesco a chiedere l'elenco dei processi: {e}"))?;
    if !uscita.status.success() {
        bail!("l'elenco dei processi non si e' fatto leggere");
    }
    Ok(String::from_utf8_lossy(&uscita.stdout)
        .lines()
        .filter_map(da_riga_ps)
        .collect())
}

/// La fotografia di chi c'e' adesso.
pub fn elenca() -> Result<Vec<Processo>> {
    if std::path::Path::new("/proc/self/comm").exists() {
        return da_proc_fs();
    }
    da_ps()
}

/// Ferma **un** processo.
///
/// Senza `forza` si manda `SIGTERM`, che e' «per favore chiudi»: il programma
/// lo puo' intercettare, salvare quello che stava facendo e uscire da se'.
/// Con `forza` si manda `SIGKILL`, che non si intercetta e non lascia salvare
/// niente — e' l'equivalente di staccare la spina a quel solo programma.
pub fn chiudi(pid: u32, forza: bool) -> Result<()> {
    if let Some(perche) = perche_non_si_tocca(pid) {
        bail!("{perche}");
    }
    let segnale = if forza { libc::SIGKILL } else { libc::SIGTERM };
    let esito = unsafe { libc::kill(pid as libc::pid_t, segnale) };
    if esito == 0 {
        return Ok(());
    }
    let e = std::io::Error::last_os_error();
    match e.raw_os_error() {
        Some(libc::ESRCH) => bail!("il processo {pid} non c'e' (piu')"),
        Some(libc::EPERM) => bail!(
            "il processo {pid} non e' tuo: fermarlo richiede i permessi di \
             amministratore, e non me li prendo da sola"
        ),
        _ => bail!("non sono riuscita a fermare il processo {pid}: {e}"),
    }
}

/// Avviare un programma come lo avvierebbe chi fa doppio clic.
///
/// `xdg-open` su Linux e `open` su macOS sanno risolvere un documento, un URL
/// e un'applicazione allo stesso modo del gestore di file. Gli argomenti
/// vanno come **argomenti**, uno per uno: non c'e' nessuna riga di comando da
/// comporre, quindi non c'e' niente da proteggere e niente che si rompa su un
/// apostrofo (D130).
pub fn avvia(bersaglio: &str, argomenti: &str) -> Result<()> {
    if bersaglio.trim().is_empty() {
        bail!("non mi hai detto cosa avviare");
    }
    let argomenti: Vec<&str> = argomenti.split_whitespace().collect();
    let apri = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    // Con argomenti si avvia il programma direttamente: `xdg-open` e `open`
    // aprono **un** oggetto e non sanno cosa farsene di un secondo.
    let (programma, tutti): (&str, Vec<&str>) = if argomenti.is_empty() {
        (apri, vec![bersaglio])
    } else {
        (bersaglio, argomenti)
    };
    std::process::Command::new(programma)
        .args(&tutti)
        .spawn()
        .map(|_| ())
        .map_err(|e| {
            if programma == apri {
                anyhow!(
                    "non riesco ad avviare «{bersaglio}»: su questo sistema manca \
                     `{apri}`, che e' quello che apre le cose al posto mio ({e})"
                )
            } else {
                anyhow!("non riesco ad avviare «{bersaglio}»: {e}")
            }
        })
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn lo_zero_e_luno_non_si_toccano() {
        // Lo zero e' il modo di spegnere tutto con un campo vuoto: e'
        // l'equivalente Unix dei duecentonovantadue processi selezionati da
        // un asterisco che questo modulo racconta.
        assert!(perche_non_si_tocca(0).unwrap().contains("tutto il gruppo"));
        assert!(perche_non_si_tocca(1).unwrap().contains("primo processo"));
        assert!(perche_non_si_tocca(2).is_none());
        assert!(perche_non_si_tocca(31_337).is_none());
        // E `chiudi` non ci arriva nemmeno, alla chiamata di sistema.
        assert!(chiudi(0, true).is_err());
        assert!(chiudi(1, true).is_err());
    }

    #[test]
    fn da_proc_legge_il_nome_e_la_memoria() {
        let (nome, byte) = da_proc("firefox\n", "123456 65536 1000 1 0 1 0\n", 4096);
        assert_eq!(nome, "firefox");
        assert_eq!(byte, 65536 * 4096);
        // Un `statm` vuoto vuol dire zero, non un errore: succede per i
        // processi del kernel, e non e' un guasto.
        assert_eq!(da_proc("kthreadd\n", "", 4096), ("kthreadd".to_string(), 0));
    }

    #[test]
    fn una_riga_di_ps_col_nome_che_ha_gli_spazi() {
        // «Google Chrome Helper» e' un nome vero, e troncarlo al primo
        // spazio lo rende irriconoscibile proprio a chi deve decidere.
        let p = da_riga_ps("  612  84320 /Applications/Google Chrome Helper").unwrap();
        assert_eq!(p.pid, 612);
        assert_eq!(p.memoria_byte, 84320 * 1024);
        assert_eq!(p.nome, "Google Chrome Helper");
        let p = da_riga_ps("1 2048 /sbin/launchd").unwrap();
        assert_eq!((p.pid, p.nome.as_str()), (1, "launchd"));
        assert!(da_riga_ps("").is_none());
        assert!(da_riga_ps("non un numero 123 x").is_none());
    }

    #[test]
    fn avviare_niente_non_avvia_niente() {
        assert!(avvia("", "").is_err());
        assert!(avvia("   ", "").is_err());
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn e_su_questa_macchina_i_processi_ci_sono() {
        let p = elenca().unwrap();
        assert!(!p.is_empty(), "almeno se stessa la vede");
        let mio = std::process::id();
        assert!(p.iter().any(|x| x.pid == mio), "non vede nemmeno se stessa");
        assert!(
            p.iter().any(|x| x.memoria_byte > 0),
            "nessuno occupa memoria?"
        );
        // E la ricerca per sottostringa funziona su quello che c'e' davvero.
        let miei = crate::processi::corrispondenti(&p, "");
        assert_eq!(miei.len(), p.len());
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn fermare_un_processo_che_non_c_e_lo_dice() {
        // Un pid altissimo non esiste: la risposta deve distinguere «non
        // c'e'» da «non e' tuo», perche' si risolvono in modi opposti.
        let e = chiudi(4_000_000, false).unwrap_err().to_string();
        assert!(e.contains("non c'e'"), "{e}");
    }
}
