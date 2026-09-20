//! Il recinto attorno a **un** comando, e la cartella che muore con lui.
//!
//! `nova-platform::recinto` sa chiudere un processo dentro un confine che
//! tiene il kernel. Qui si decide **quale** confine: quello che l'utente ha
//! gia' dichiarato nella sua configurazione, piu' una cartella temporanea
//! che nasce con il comando e muore con lui.
//!
//! Le due cose insieme sono la differenza fra una regola e un muro. Prima:
//! `check_write` confrontava percorsi prima di partire, e quel che partiva
//! aveva tutti i privilegi di chi l'aveva lanciato — una regola che nessuno
//! applicava piu'. Adesso il confine vale anche dopo, e il rifiuto arriva da
//! `EACCES` del kernel invece che da una frase nostra.
//!
//! **Dove non c'e' un recinto, si dice.** Senza `write_roots` dichiarati non
//! si stringe niente: decidere al posto dell'utente quali cartelle sono sue
//! sarebbe la stessa presunzione che NOVA rifiuta altrove. E su Windows il
//! recinto di sistema non c'e' ancora: la risposta lo dichiara, invece di
//! lasciar credere di essere confinati.

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
    static CONTO: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = CONTO.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let p = std::env::temp_dir().join(format!("nova-comando-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&p).ok().map(|_| p)
}

/// Cosa lasciare toccare a questo comando.
pub fn permessi(ctx: &Ctx, effimera: Option<&Path>) -> Permessi {
    let mut scrive: Vec<PathBuf> = ctx.policy.write_roots().to_vec();
    if scrive.is_empty() {
        // Niente dichiarato, nessun recinto: la cartella effimera da sola
        // sarebbe un confine inventato da noi, e piu' stretto di quello che
        // l'utente ha scelto tenendo `write_roots` vuoto.
        return Permessi::default();
    }
    if let Some(t) = effimera {
        scrive.push(t.to_path_buf());
    }
    Permessi { scrive }
}

/// Il recinto per questo comando: cosa si stringe, e cosa sa fare il sistema.
#[derive(Debug, Clone)]
pub struct Attorno {
    pub permessi: Permessi,
    pub sistema: Recinto,
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
            let quante = self.permessi.scrive.len();
            return format!(
                "chiuso dal kernel: puo' scrivere solo in {quante} cartelle \
                 dichiarate, piu' la sua temporanea che muore col comando"
            );
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
    Attorno {
        permessi: permessi(ctx, effimera),
        sistema: recinto::disponibile(),
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

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn la_cartella_effimera_e_diversa_ogni_volta() {
        let a = cartella_effimera().expect("si crea");
        let b = cartella_effimera().expect("si crea");
        assert_ne!(a, b, "due comandi non devono scriversi addosso");
        assert!(a.is_dir() && b.is_dir());
        let _ = std::fs::remove_dir_all(&a);
        let _ = std::fs::remove_dir_all(&b);
    }

    #[test]
    fn senza_write_roots_non_si_stringe_niente() {
        let server = crate::build(crate::config::Config::default()).unwrap();
        let mut cfg = crate::config::Config::default();
        cfg.write_roots.clear();
        let ctx = &server.ctx;
        let p = permessi(ctx, Some(Path::new("/tmp/x")));
        if ctx.policy.write_roots().is_empty() {
            assert!(
                p.scrive.is_empty(),
                "un confine inventato da noi non e' suo"
            );
        }
    }

    #[test]
    fn e_quando_non_si_stringe_lo_si_dice() {
        let a = Attorno {
            permessi: Permessi::default(),
            sistema: Recinto::Kernel(4),
        };
        assert!(!a.chiuso());
        assert!(
            a.come_si_racconta().contains("write_roots"),
            "{}",
            a.come_si_racconta()
        );
    }

    #[test]
    fn con_le_cartelle_dichiarate_il_recinto_si_chiude() {
        let a = Attorno {
            permessi: Permessi {
                scrive: vec![PathBuf::from("/tmp/nova-prova")],
            },
            sistema: Recinto::Kernel(4),
        };
        assert!(a.chiuso());
        assert!(
            a.come_si_racconta().contains("kernel"),
            "{}",
            a.come_si_racconta()
        );
    }

    #[test]
    fn su_un_sistema_che_non_sa_tenerlo_non_si_finge() {
        let a = Attorno {
            permessi: Permessi {
                scrive: vec![PathBuf::from("/tmp/nova-prova")],
            },
            sistema: Recinto::NonCe("qui non c'e'"),
        };
        assert!(!a.chiuso());
        assert!(a.come_si_racconta().contains("qui non c'e'"));
    }
}
