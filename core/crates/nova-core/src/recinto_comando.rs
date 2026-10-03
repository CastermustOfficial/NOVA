//! Il recinto attorno a **un** comando, e la cartella che muore con lui.
//!
//! `nova-platform::recinto` sa chiudere un processo dentro un confine che
//! tiene il sistema: Landlock su Linux, un contenitore (AppContainer) e un job
//! object su Windows. Qui si decide **quale** confine: quello che l'utente ha
//! gia' dichiarato nella sua configurazione, piu' una cartella temporanea
//! che nasce con il comando e muore con lui.
//!
//! Le due cose insieme sono la differenza fra una regola e un muro. Prima:
//! `check_write` confrontava percorsi prima di partire, e quel che partiva
//! aveva tutti i privilegi di chi l'aveva lanciato — una regola che nessuno
//! applicava piu'. Adesso il confine vale anche dopo, e il rifiuto arriva dal
//! sistema — `EACCES` su Linux, «accesso negato» su Windows — invece che da
//! una frase nostra.
//!
//! **Dove non c'e' un recinto, si dice.** Senza `write_roots` dichiarati non
//! si stringe niente: decidere al posto dell'utente quali cartelle sono sue
//! sarebbe la stessa presunzione che NOVA rifiuta altrove. E dove il sistema
//! non sa tenerlo, o lo tiene solo in parte, la risposta lo dichiara invece
//! di lasciar credere di essere confinati: su Windows il contenitore non vede
//! il resto del profilo ne' il loopback, e puo' scrivere nelle poche cartelle
//! di terzi aperte a tutti i pacchetti (D367).

use nova_platform::recinto::{self, Permessi, Recinto};
use std::path::{Path, PathBuf};

use crate::capability::Ctx;

/// Una cartella scrivibile che vale per un comando solo.
///
/// Serve perche' un comando confinato che non ha **nessun** posto dove
/// appoggiare un file fallisce in modi che non somigliano a un problema di
/// permessi: un archivio che non si scompatta, un compilatore che non scrive
/// l'intermedio. Questa nasce adesso, sta dentro il recinto, e muore con il
/// comando — cosi' non diventa un deposito di residui di cui nessuno sa piu'
/// niente.
pub fn cartella_effimera() -> Option<PathBuf> {
    // Su Windows `TEMP` e `TMP` non si possono scegliere: il contenitore
    // ha la sua temporanea, e il sistema la impone qualunque valore si passi
    // (provato). Quella si svuota a fine comando; questa non servirebbe.
    if cfg!(windows) {
        return None;
    }
    static CONTO: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = CONTO.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let p = std::env::temp_dir().join(format!("nova-comando-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&p).ok().map(|_| p)
}

/// Cosa lasciare toccare a questo comando.
pub fn permessi(ctx: &Ctx, effimera: Option<&Path>) -> Permessi {
    permessi_da(&ctx.policy, effimera)
}

/// Come `permessi`, da una policy sola: serve anche all'avvio del demone,
/// dove non c'e' un comando.
pub fn permessi_da(policy: &crate::policy::Policy, effimera: Option<&Path>) -> Permessi {
    let mut scrive: Vec<PathBuf> = policy.write_roots().to_vec();
    if scrive.is_empty() {
        // Niente dichiarato, nessun recinto: la cartella effimera da sola
        // sarebbe un confine inventato da noi, e piu' stretto di quello che
        // l'utente ha scelto tenendo `write_roots` vuoto.
        return Permessi::default();
    }
    if let Some(t) = effimera {
        scrive.push(t.to_path_buf());
    }
    Permessi {
        scrive,
        legge: policy.tool_roots().to_vec(),
        senza_rete: policy.senza_rete(),
    }
}

/// Il recinto per questo comando: cosa si stringe, e cosa sa fare il sistema.
#[derive(Debug, Clone)]
pub struct Attorno {
    pub permessi: Permessi,
    pub sistema: Recinto,
    /// La cartella temporanea del comando, quando sta dentro i permessi.
    /// Si tiene a parte perche' non e' una cartella **dichiarata**: contarla
    /// fra quelle dell'utente farebbe dire al racconto un numero sbagliato.
    pub temporanea: Option<PathBuf>,
}

impl Attorno {
    /// Se il comando partira' davvero dentro un recinto.
    pub fn chiuso(&self) -> bool {
        self.sistema.ce() && recinto::serve(&self.permessi)
    }

    /// Una riga che dice com'e' andata, da mettere nella risposta.
    ///
    /// Si dichiara **anche quando non c'e'**: un confine che si crede di
    /// avere e non si ha e' peggio di un confine che manca.
    pub fn come_si_racconta(&self) -> String {
        if self.chiuso() {
            let dichiarate = self.permessi.scrive.len() - usize::from(self.temporanea.is_some());
            let cartelle = if dichiarate == 1 {
                "1 cartella dichiarata".to_string()
            } else {
                format!("{dichiarate} cartelle dichiarate")
            };
            return match self.sistema {
                Recinto::Contenitore => {
                    let strumenti = match self.permessi.legge.len() {
                        0 => "solo il sistema".to_string(),
                        1 => "il sistema e 1 cartella di strumenti".to_string(),
                        n => format!("il sistema e {n} cartelle di strumenti"),
                    };
                    let rete = if self.permessi.senza_rete { "spenta" } else { "accesa" };
                    let controllo = crate::recinto_controllo::racconto_riga();
                    format!(
                        "chiuso da Windows (contenitore): crea, modifica e cancella file \
                         solo in {cartelle}; legge {strumenti}, non il resto del profilo; \
                         rete {rete}; il loopback (localhost) e' chiuso, quindi un server \
                         di prova avviato in locale non e' raggiungibile dal comando; \
                         {controllo}"
                    )
                }
                _ => {
                    let coda = if self.temporanea.is_some() {
                        ", piu' la sua temporanea che muore col comando"
                    } else {
                        ""
                    };
                    let rete = if self.permessi.senza_rete {
                        "; la rete non si spegne su questo sistema, la richiesta e' ignorata"
                    } else {
                        ""
                    };
                    format!("chiuso dal kernel: puo' scrivere solo in {cartelle}{coda}{rete}")
                }
            };
        }
        if !recinto::serve(&self.permessi) {
            return "nessun recinto: in configurazione non ci sono write_roots, \
                    quindi il confine e' solo quello della policy"
                .to_string();
        }
        self.sistema.come_si_racconta()
    }
}

pub fn prepara(ctx: &Ctx, effimera: Option<&Path>) -> Attorno {
    let permessi = permessi(ctx, effimera);
    let temporanea = if permessi.scrive.is_empty() {
        None
    } else {
        effimera.map(Path::to_path_buf)
    };
    Attorno {
        permessi,
        sistema: recinto::disponibile(),
        temporanea,
    }
}

/// Attacca il recinto al comando che sta per partire.
///
/// Su unix si chiude **fra la fork e la exec**: la restrizione sopravvive
/// alla exec, quindi il programma parte gia' dentro, e il demone — che sta
/// nel processo padre — non si stringe niente addosso.
#[cfg(unix)]
pub fn applica(cmd: &mut tokio::process::Command, attorno: &Attorno) {
    if !attorno.chiuso() {
        return;
    }
    let permessi = attorno.permessi.clone();
    unsafe {
        use std::os::unix::process::CommandExt;
        cmd.as_std_mut()
            .pre_exec(move || recinto::chiudi(&permessi).map_err(std::io::Error::other));
    }
}

#[cfg(not(unix))]
pub fn applica(_cmd: &mut tokio::process::Command, _attorno: &Attorno) {}

/// Come e' andato un comando, indipendente dal modo in cui e' partito.
pub struct Uscita {
    /// Il codice d'uscita; `None` se il comando e' stato fermato.
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    /// Fermato dalla scadenza, non finito da solo.
    pub scaduto: bool,
    /// Cose che non vanno e non riguardano la sicurezza: per esempio una
    /// antenata che non si e' riusciti a preparare, e PowerShell non si
    /// posizionera' nella cartella di lavoro.
    pub avvisi: Vec<String>,
}

/// Quanti comandi confinati sono in corso: la temporanea del contenitore e'
/// una sola, e si svuota solo quando non ne resta nessuno.
#[cfg(windows)]
static IN_CORSO: std::sync::Mutex<usize> = std::sync::Mutex::new(0);

/// Un comando in corso, contato. Il conto e lo svuotamento della temporanea
/// stanno sotto lo stesso blocco: con un contatore atomico un comando che parte
/// fra «sono l'ultimo» e «svuoto» si vedrebbe cancellare i file da sotto. Si
/// decrementa nel `Drop`, perche' un turno interrotto lascia cadere il futuro
/// senza passare dal resto della funzione: senza, il conto non tornerebbe a zero
/// e la temporanea non si svuoterebbe piu'.
#[cfg(windows)]
struct ComandoContato {
    confinato: bool,
}

#[cfg(windows)]
impl ComandoContato {
    fn nuovo(confinato: bool) -> Self {
        *IN_CORSO.lock().unwrap_or_else(|e| e.into_inner()) += 1;
        ComandoContato { confinato }
    }
}

#[cfg(windows)]
impl Drop for ComandoContato {
    fn drop(&mut self) {
        let mut n = IN_CORSO.lock().unwrap_or_else(|e| e.into_inner());
        *n -= 1;
        if *n == 0 && self.confinato {
            svuota_temporanea_del_contenitore();
        }
    }
}

/// Svuota la temporanea del contenitore. Windows la impone come `TEMP` e
/// `TMP` di ogni comando confinato: e' una sola, condivisa e non muore col
/// comando, quindi la tocca chi l'ha usata. Quel che non si riesce a togliere
/// resta li' e si riprova al comando dopo.
#[cfg(windows)]
pub fn svuota_temporanea_del_contenitore() {
    let Some(t) = nova_platform::recinto::windows::temporanea_del_profilo() else {
        return;
    };
    svuota_cartella(&t);
}

/// Toglie tutto quel che c'e' dentro `t`, e non `t`. Un collegamento o una
/// giunzione si toglie **lui**, senza entrarci: la cartella e' scrivibile dal
/// contenitore, che puo' crearne uno verso qualunque posto (per crearlo non
/// serve poter accedere al bersaglio), e questo codice cancella con il token
/// dell'utente.
#[cfg(windows)]
pub fn svuota_cartella(t: &Path) {
    let Ok(dentro) = std::fs::read_dir(t) else {
        return;
    };
    for voce in dentro.flatten() {
        let p = voce.path();
        let Ok(tipo) = voce.file_type() else {
            continue;
        };
        let _ = if tipo.is_symlink() {
            std::fs::remove_dir(&p).or_else(|_| std::fs::remove_file(&p))
        } else if tipo.is_dir() {
            std::fs::remove_dir_all(&p)
        } else {
            std::fs::remove_file(&p)
        };
    }
}

/// Fa partire il comando e ne aspetta la fine, in un filo a parte.
///
/// **Se il futuro cade prima della fine** — un turno interrotto, una richiesta
/// annullata — il comando si ferma, con tutto quello che ha avviato. Su unix lo
/// fa `kill_on_drop`; qui il comando gira in un filo bloccante che nessuno
/// annulla, e senza una guardia arriverebbe in fondo da solo, di nascosto, dopo
/// che chi l'aveva chiesto se n'e' andato. La guardia ferma il job object.
/// Un comando finito per conto suo non si tocca: puo' aver avviato un programma
/// che deve restare aperto.
#[cfg(windows)]
pub async fn lancia_e_aspetta(
    comando: nova_platform::recinto::windows::Comando,
    permessi: Option<Permessi>,
    timeout: std::time::Duration,
) -> anyhow::Result<nova_platform::recinto::windows::Esito> {
    use nova_platform::recinto::windows::{self, Job};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    /// Quel che serve per fermare il comando da fuori del filo che lo aspetta.
    #[derive(Default)]
    struct Presa {
        job: Mutex<Option<Arc<Job>>>,
        annullato: AtomicBool,
    }

    struct Guardia {
        presa: Arc<Presa>,
        armata: bool,
    }

    impl Drop for Guardia {
        fn drop(&mut self) {
            if !self.armata {
                return;
            }
            // Prima la bandiera, poi il job: il filo fa l'inverso (prima il job,
            // poi guarda la bandiera), quindi almeno uno dei due vede l'altro.
            self.presa.annullato.store(true, Ordering::SeqCst);
            let job = self.presa.job.lock().unwrap_or_else(|e| e.into_inner()).clone();
            if let Some(j) = job {
                j.ferma();
            }
        }
    }

    let presa = Arc::new(Presa::default());
    let mut guardia = Guardia { presa: presa.clone(), armata: true };
    let nel_filo = presa.clone();
    let esito = tokio::task::spawn_blocking(move || {
        let in_corsa = windows::lancia(&comando, permessi.as_ref())?;
        *nel_filo.job.lock().unwrap_or_else(|e| e.into_inner()) = Some(in_corsa.job.clone());
        // Annullato mentre partiva: il futuro e' gia' caduto e nessuno guardera'
        // l'esito.
        if nel_filo.annullato.load(Ordering::SeqCst) {
            in_corsa.job.ferma();
        }
        Ok::<_, String>(in_corsa.attendi(timeout))
    })
    .await;
    guardia.armata = false;
    esito
        .map_err(|e| anyhow::anyhow!("il comando non si e' potuto avviare: {e}"))?
        .map_err(|e| anyhow::anyhow!("{e}"))
}

/// Su Windows il recinto non si mette fra la fork e la exec — quella fork non
/// c'e' — ma si costruisce **il processo** dentro un contenitore, con un job
/// object. Quel lancio lo fa `nova_platform::recinto::windows`, che pero' non
/// e' `tokio::process`: quando il recinto e' chiuso il comando passa di li',
/// altrimenti resta la strada di sempre. Il blocco vero e' in un thread a
/// parte perche' quelle chiamate di sistema aspettano; il turno intanto non
/// si ferma.
#[cfg(windows)]
pub async fn esegui_windows(
    programma: &str,
    argomenti: Vec<String>,
    cwd: Option<std::path::PathBuf>,
    ambiente: Vec<(String, String)>,
    attorno: &Attorno,
    timeout: std::time::Duration,
) -> anyhow::Result<Uscita> {
    use nova_platform::recinto::windows;

    // Prima l'elenco (D367): quel che non serve piu' perde le voci, quel che
    // serve si prepara e si annota. Se una cosa di sicurezza non riesce il
    // comando non parte — una voce rimasta dove non deve allargherebbe il
    // recinto. Senza recinto (`write_roots` vuoto) si prepara un elenco
    // vuoto, che toglie tutto quello che c'era.
    let da_preparare = if attorno.chiuso() { attorno.permessi.clone() } else { Permessi::default() };
    let avvisi = tokio::task::spawn_blocking(move || crate::recinto_registro::prepara(&da_preparare))
        .await
        .map_err(|e| anyhow::anyhow!("il recinto non si e' potuto preparare: {e}"))?
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let permessi = attorno.chiuso().then(|| attorno.permessi.clone());
    let confinato = permessi.is_some();
    let comando = windows::Comando {
        programma: programma.to_string(),
        argomenti,
        cartella: cwd,
        ambiente,
    };
    let esito = {
        let _conto = ComandoContato::nuovo(confinato);
        lancia_e_aspetta(comando, permessi, timeout).await
    }?;

    Ok(Uscita {
        code: esito.codice,
        stdout: esito.stdout,
        stderr: esito.stderr,
        scaduto: esito.scaduto,
        avvisi,
    })
}

#[cfg(test)]
mod prove {
    use super::*;
    use crate::config::Config;
    use crate::policy::Policy;

    #[cfg(not(windows))]
    #[test]
    fn la_cartella_effimera_e_diversa_ogni_volta() {
        let a = cartella_effimera().expect("si crea");
        let b = cartella_effimera().expect("si crea");
        assert_ne!(a, b, "due comandi non devono scriversi addosso");
        assert!(a.is_dir() && b.is_dir());
        let _ = std::fs::remove_dir_all(&a);
        let _ = std::fs::remove_dir_all(&b);
    }

    /// Interrompere il turno deve fermare il comando. Su unix lo fa
    /// `kill_on_drop`; su Windows il comando gira in un filo bloccante, che
    /// nessuno annulla se il futuro cade: senza una guardia il comando arriva in
    /// fondo da solo, di nascosto, dopo che chi l'aveva chiesto se n'e' andato.
    #[cfg(windows)]
    #[tokio::test]
    async fn interrompere_il_turno_ferma_il_comando() {
        use nova_platform::recinto::windows::Comando;
        let dir = std::env::temp_dir().join(format!("nova-interruzione-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let marcatore = dir.join("arrivato.txt");
        let script = format!(
            "Start-Sleep -Seconds 4; Set-Content -Path '{}' -Value x",
            marcatore.display()
        );
        let comando = Comando {
            programma: "powershell".into(),
            argomenti: ["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command"]
                .iter()
                .map(|s| s.to_string())
                .chain(std::iter::once(script))
                .collect(),
            cartella: None,
            ambiente: vec![],
        };
        // Un turno interrotto lascia cadere il futuro: qui, dopo un attimo.
        let esito = tokio::time::timeout(
            std::time::Duration::from_millis(1200),
            lancia_e_aspetta(comando, None, std::time::Duration::from_secs(60)),
        )
        .await;
        assert!(esito.is_err(), "il comando doveva essere ancora in corso");
        // Se il comando sopravvive scrive il file entro 4 secondi dall'avvio.
        tokio::time::sleep(std::time::Duration::from_secs(7)).await;
        let arrivato = marcatore.exists();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(!arrivato, "il comando e' arrivato in fondo dopo l'interruzione: nessuno l'ha fermato");
    }

    /// La temporanea del contenitore e' scrivibile dal contenitore, che puo'
    /// crearci una giunzione verso qualunque posto; e chi la svuota cancella con
    /// il token dell'utente. Si toglie la giunzione, mai quel che c'e' dietro.
    #[cfg(windows)]
    #[test]
    fn svuotare_la_temporanea_non_entra_nei_collegamenti() {
        let base = std::env::temp_dir().join(format!("nova-svuota-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let bersaglio = base.join("bersaglio");
        let temp = base.join("temp");
        std::fs::create_dir_all(&bersaglio).unwrap();
        std::fs::create_dir_all(temp.join("sotto")).unwrap();
        std::fs::write(bersaglio.join("prezioso.txt"), b"non toccare").unwrap();
        std::fs::write(temp.join("residuo.txt"), b"x").unwrap();
        std::fs::write(temp.join("sotto").join("altro.txt"), b"x").unwrap();
        let giunzione = temp.join("giunzione");
        let fatta = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(&giunzione)
            .arg(&bersaglio)
            .output()
            .unwrap();
        assert!(fatta.status.success(), "mklink /J: {}", String::from_utf8_lossy(&fatta.stdout));
        svuota_cartella(&temp);
        let resta: Vec<_> = std::fs::read_dir(&temp).unwrap().flatten().collect();
        let integro = bersaglio.join("prezioso.txt").exists();
        let _ = std::fs::remove_dir_all(&base);
        assert!(integro, "ha cancellato quel che sta dietro la giunzione");
        assert!(resta.is_empty(), "la temporanea non e' vuota: {resta:?}");
    }

    /// Su Windows la temporanea la impone il sistema: una cartella per
    /// comando sarebbe un residuo e basta.
    #[cfg(windows)]
    #[test]
    fn su_windows_non_c_e_una_cartella_per_comando() {
        assert!(cartella_effimera().is_none());
    }

    #[test]
    fn senza_write_roots_non_si_stringe_niente() {
        let mut cfg = Config::default();
        cfg.write_roots.clear();
        cfg.tool_roots = vec!["C:\\strumenti".into()];
        let policy = Policy::con_quelle_di_nova(&cfg, &serde_json::json!({}));
        let p = permessi_da(&policy, Some(Path::new("/tmp/x")));
        assert!(p.scrive.is_empty(), "un confine inventato da noi non e' suo");
        assert!(p.legge.is_empty(), "gli strumenti senza recinto non servono a niente");
    }

    /// `tool_roots` e `shell_senza_rete` della configurazione arrivano ai
    /// permessi del comando.
    #[test]
    fn gli_strumenti_e_la_rete_arrivano_ai_permessi() {
        let mut cfg = Config::default();
        cfg.write_roots = vec!["D:\\lavoro".into()];
        cfg.tool_roots = vec!["C:\\Python313".into(), "C:\\Program Files\\nodejs".into()];
        cfg.shell_senza_rete = true;
        let policy = Policy::con_quelle_di_nova(&cfg, &serde_json::json!({}));
        let p = permessi_da(&policy, None);
        assert_eq!(p.scrive, vec![PathBuf::from("D:\\lavoro")]);
        assert_eq!(
            p.legge,
            vec![PathBuf::from("C:\\Python313"), PathBuf::from("C:\\Program Files\\nodejs")]
        );
        assert!(p.senza_rete);
        // E il default e' rete accesa.
        let mut cfg = Config::default();
        cfg.write_roots = vec!["D:\\lavoro".into()];
        let policy = Policy::con_quelle_di_nova(&cfg, &serde_json::json!({}));
        assert!(!permessi_da(&policy, None).senza_rete);
    }

    #[test]
    fn e_quando_non_si_stringe_lo_si_dice() {
        let a = Attorno {
            permessi: Permessi::default(),
            sistema: Recinto::Landlock(4),
            temporanea: None,
        };
        assert!(!a.chiuso());
        assert!(
            a.come_si_racconta().contains("write_roots"),
            "{}",
            a.come_si_racconta()
        );
    }

    fn con_cartelle(sistema: Recinto) -> Attorno {
        Attorno {
            permessi: Permessi {
                scrive: vec![PathBuf::from("/tmp/nova-prova")],
                ..Default::default()
            },
            sistema,
            temporanea: None,
        }
    }

    #[test]
    fn con_le_cartelle_dichiarate_il_recinto_si_chiude() {
        let a = con_cartelle(Recinto::Landlock(4));
        assert!(a.chiuso());
        assert!(a.come_si_racconta().contains("kernel"), "{}", a.come_si_racconta());
    }

    /// La cartella temporanea non si conta fra quelle dichiarate: con una
    /// sola `write_roots` il racconto dice una, non due.
    #[test]
    fn la_temporanea_non_si_conta_fra_le_dichiarate() {
        let tmp = PathBuf::from("/tmp/nova-comando-0");
        let a = Attorno {
            permessi: Permessi {
                scrive: vec![PathBuf::from("/tmp/nova-prova"), tmp.clone()],
                ..Default::default()
            },
            sistema: Recinto::Landlock(4),
            temporanea: Some(tmp),
        };
        let r = a.come_si_racconta();
        assert!(r.contains("1 cartella dichiarata"), "{r}");
        assert!(r.contains("temporanea"), "{r}");
    }

    /// Su Windows il racconto dice chi tiene il recinto e **cosa il
    /// comando non vede**: il resto del profilo e il loopback. Chi lancia un
    /// server di prova in locale lo vedra' fallire, e deve potersi spiegare
    /// perche'.
    #[test]
    fn su_windows_si_dice_cosa_si_vede_e_cosa_no() {
        let r = con_cartelle(Recinto::Contenitore).come_si_racconta();
        assert!(r.contains("Windows"), "{r}");
        assert!(r.contains("solo il sistema"), "{r}");
        assert!(r.contains("non il resto del profilo"), "{r}");
        assert!(r.contains("rete accesa"), "{r}");
        assert!(r.contains("loopback"), "{r}");
        assert!(r.contains("server di prova"), "{r}");
        assert!(!r.contains("kernel"), "{r}");

        let mut a = con_cartelle(Recinto::Contenitore);
        a.permessi.legge = vec![PathBuf::from("C:\\Python313")];
        a.permessi.senza_rete = true;
        let r = a.come_si_racconta();
        assert!(r.contains("il sistema e 1 cartella di strumenti"), "{r}");
        assert!(r.contains("rete spenta"), "{r}");
    }

    /// Dove la rete non si spegne, la richiesta non finge di aver funzionato.
    #[test]
    fn su_linux_la_rete_non_si_spegne_e_lo_si_dice() {
        let mut a = con_cartelle(Recinto::Landlock(4));
        a.permessi.senza_rete = true;
        let r = a.come_si_racconta();
        assert!(r.contains("la rete non si spegne"), "{r}");
    }

    #[test]
    fn su_un_sistema_che_non_sa_tenerlo_non_si_finge() {
        let a = con_cartelle(Recinto::NonCe("qui non c'e'"));
        assert!(!a.chiuso());
        assert!(a.come_si_racconta().contains("qui non c'e'"));
    }
}
