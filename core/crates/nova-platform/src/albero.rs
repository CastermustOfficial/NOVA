//! L'albero di un processo: il figlio e tutti quelli che fa partire (D415).
//!
//! Un Dot lavora con una CLI che e' un albero di processi: su Windows
//! `claude.cmd` fa partire `node`, che fa partire `claude.exe`, che fa
//! partire il collegamento MCP di NOVA. `kill_on_drop` ferma solo il primo,
//! e il 13 ottobre, fermato il progetto, due `claude.exe` sono rimasti accesi
//! a lavorare per conto loro. Era il limite scritto in architettura,
//! «Nipoti orfani».
//!
//! **Chi tiene l'[`Albero`] tiene in vita tutto l'albero**: quando lo lascia,
//! perche' il processo e' finito o perche' il lavoro e' stato fermato, cadono
//! tutti insieme, figli e nipoti.
//!
//! - Su Windows e' un *job object* con `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`:
//!   quando si chiude l'ultima maniglia, Windows ferma ogni processo dentro.
//! - Su Linux e macOS e' un gruppo di processi: il figlio parte capo di un
//!   gruppo suo (`process_group(0)`, lo fa chi lo lancia), e alla fine il
//!   gruppo riceve `SIGKILL`.
//!
//! Resta un buco piccolo su Windows: il figlio si lega al job appena
//! partito, e un nipote nato in quel frattempo resta fuori.

/// L'albero di un processo: quando lo si lascia, cade tutto.
pub struct Albero {
    #[cfg(windows)]
    job: windows::Win32::Foundation::HANDLE,
    #[cfg(not(windows))]
    gruppo: i32,
}

#[cfg(windows)]
unsafe impl Send for Albero {}
#[cfg(windows)]
unsafe impl Sync for Albero {}

impl Albero {
    /// Lega al job l'albero del processo appena partito.
    #[cfg(windows)]
    pub fn lega(processo: std::os::windows::io::RawHandle) -> Result<Albero, String> {
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::{CloseHandle, HANDLE};
        use windows::Win32::System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
            SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };
        // SAFETY: si crea un job senza nome; la maniglia la tiene l'Albero, e
        // la chiude il suo Drop (o la riga qui sotto, se va male).
        let job = unsafe { CreateJobObjectW(None, PCWSTR::null()) }
            .map_err(|e| format!("non creo il job object: {e}"))?;
        let mut limiti = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limiti.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: `limiti` e' della classe che si dichiara, e vive fino alla
        // fine della chiamata; `processo` e' la maniglia di un figlio vivo.
        let fatto = unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                (&limiti as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
            .and_then(|()| AssignProcessToJobObject(job, HANDLE(processo)))
        };
        if let Err(e) = fatto {
            // SAFETY: la maniglia appena creata, chiusa una volta sola.
            unsafe {
                let _ = CloseHandle(job);
            }
            return Err(format!("non lego il processo al job object: {e}"));
        }
        Ok(Albero { job })
    }

    /// Prende il gruppo del processo appena partito, che deve esserne il capo
    /// (`process_group(0)` prima di farlo partire).
    #[cfg(not(windows))]
    pub fn lega(pid: u32) -> Albero {
        Albero { gruppo: pid as i32 }
    }
}

impl Drop for Albero {
    #[cfg(windows)]
    fn drop(&mut self) {
        // SAFETY: l'ultima maniglia del job: chiuderla ferma tutto quel che
        // c'e' dentro (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`).
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.job);
        }
    }

    #[cfg(not(windows))]
    fn drop(&mut self) {
        if self.gruppo > 1 {
            // SAFETY: un segnale a un gruppo; se non c'e' piu' nessuno, la
            // chiamata fallisce e basta.
            unsafe {
                libc::kill(-self.gruppo, libc::SIGKILL);
            }
        }
    }
}

#[cfg(all(test, not(windows)))]
mod prove {
    use super::*;
    use std::os::unix::process::CommandExt;

    fn vivo(pid: i32) -> bool {
        // Un processo finito resta un attimo come zombie: per noi e' morto.
        std::fs::read_to_string(format!("/proc/{pid}/stat"))
            .map(|s| !s.split_whitespace().nth(2).is_some_and(|stato| stato == "Z"))
            .unwrap_or(false)
    }

    #[test]
    fn lasciato_l_albero_cadono_anche_i_nipoti() {
        if !std::path::Path::new("/proc").is_dir() {
            return;
        }
        let mut figlio = std::process::Command::new("sh")
            .args(["-c", "sleep 60 & echo $!; wait"])
            .process_group(0)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let albero = Albero::lega(figlio.id());
        let mut riga = String::new();
        std::io::BufRead::read_line(
            &mut std::io::BufReader::new(figlio.stdout.take().unwrap()),
            &mut riga,
        )
        .unwrap();
        let nipote: i32 = riga.trim().parse().unwrap();
        assert!(vivo(nipote), "il nipote e' partito");
        drop(albero);
        let _ = figlio.wait();
        let fine = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while vivo(nipote) && std::time::Instant::now() < fine {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(!vivo(nipote), "il nipote e' rimasto acceso");
    }
}
