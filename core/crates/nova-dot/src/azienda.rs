//! L'azienda dei Dot: la direzione e i reparti che ci sono sempre (D395).
//!
//! Deciso con Gio il 9 ottobre 2026 (`docs/dots.md`, «Come un'azienda»). I
//! Dot sono un'azienda vera: una **direzione** (l'APM, AR, l'Architetto) e
//! dei **reparti** (il legale, il commerciale, la ricerca, la revisione, la
//! scrittura, i dati e le misure, la qualita' e le prove, l'amministrazione),
//! che ci sono sempre, come in ogni azienda; per un progetto AR assume solo
//! quel che manca. La sicurezza e' il custode dei permessi (D384), che c'era
//! gia' e resta fuori dalla piramide: non ha un capo e non ha sottoposti.
//!
//! I posti fissi li fa nascere NOVA, coi Dot accesi, come il custode: senza
//! compiti non costano niente. Passano da un progetto all'altro col loro
//! vault, e nessuno li licenzia. Si riconoscono dal campo `fisso` del loro
//! `dot.json`: un Dot dell'utente che si chiamava gia' come un posto resta
//! com'e', e quel posto resta vuoto. NOVA non tocca un Dot che non ha fatto
//! nascere lei.
//!
//! Qui ci sono i posti: il nome, il ruolo, il mestiere e il capo. Chi li fa
//! nascere e' `nova_core::dot`.

use crate::{Dot, Mestiere};

/// Un posto fisso dell'azienda.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Posto {
    /// Il nome del Dot che lo occupa.
    pub nome: &'static str,
    /// Il ruolo, scritto nel suo `dot.json`: e' la parte del prompt che lo
    /// fa essere lui.
    pub ruolo: &'static str,
    /// Come lavora. La direzione e il legale hanno un mestiere loro, e non
    /// prendono compiti a mano: il loro lavoro arriva coi progetti.
    pub mestiere: Mestiere,
    /// Il suo capo: l'APM per tutti, nessuno per l'APM.
    pub capo: &'static str,
    /// Se sta nella direzione; se no e' un reparto.
    pub direzione: bool,
}

/// L'APM (*Artificial Project Manager*): guida i progetti.
pub const NOME_APM: &str = "apm";
/// AR (*Artificial Resources*): le risorse.
pub const NOME_AR: &str = "ar";
/// L'Architetto: il piano di sviluppo.
pub const NOME_ARCHITETTO: &str = "architetto";
/// Il legale: le normative.
pub const NOME_LEGALE: &str = "legale";

/// I posti fissi, la direzione prima e poi i reparti, nell'ordine
/// dell'organigramma. Il primo e' l'APM, che e' il capo di tutti gli altri:
/// nasce per primo.
pub const POSTI: [Posto; 11] = [
    Posto {
        nome: NOME_APM,
        ruolo: "Guidi i progetti dell'azienda dei Dot: ricevi il progetto da Nova, chiedi il \
                piano all'Architetto e la squadra ad AR, mostri piano, organigramma e tetto di \
                spesa all'utente e aspetti il via, poi disponi i Dot e decidi quando sentire il \
                legale.",
        mestiere: Mestiere::Apm,
        capo: "",
        direzione: true,
    },
    Posto {
        nome: NOME_AR,
        ruolo: "Sei le risorse dell'azienda dei Dot: riprendi i Dot liberi che fanno al caso, \
                assumi quelli che mancano, scegli il cervello di ogni compito che assegni, e \
                licenzi gli assunti fermi da trenta giorni.",
        mestiere: Mestiere::Ar,
        capo: NOME_APM,
        direzione: true,
    },
    Posto {
        nome: NOME_ARCHITETTO,
        ruolo: "Fai il piano di sviluppo dei progetti: le fasi, cosa produce ognuna, e i ruoli \
                che servono.",
        mestiere: Mestiere::Architetto,
        capo: NOME_APM,
        direzione: true,
    },
    Posto {
        nome: NOME_LEGALE,
        ruolo: "Controlli che il progetto rispetti le normative: all'inizio, ai controlli che \
                decide l'APM, prima del rilascio, e quando un Dot te lo chiede.",
        mestiere: Mestiere::Legale,
        capo: NOME_APM,
        direzione: false,
    },
    Posto {
        nome: "commerciale",
        ruolo: "Guardi se quel che si fa si vende: la posizione sul mercato, i concorrenti, la \
                ricerca di mercato.",
        mestiere: Mestiere::Generico,
        capo: NOME_APM,
        direzione: false,
    },
    Posto {
        nome: "ricerca",
        ruolo: "Trovi lo stato dell'arte: cosa esiste gia', chi l'ha fatto e con quali \
                risultati, con le fonti.",
        mestiere: Mestiere::Ricercatore,
        capo: NOME_APM,
        direzione: false,
    },
    Posto {
        nome: "revisione",
        ruolo: "Rivedi il lavoro col rigore di una revisione accademica: la forma, la \
                coerenza, le fonti, gli errori.",
        mestiere: Mestiere::Generico,
        capo: NOME_APM,
        direzione: false,
    },
    Posto {
        nome: "scrittura",
        ruolo: "Scrivi la documentazione dei progetti, e i paper quando servono.",
        mestiere: Mestiere::Generico,
        capo: NOME_APM,
        direzione: false,
    },
    Posto {
        nome: "dati",
        ruolo: "Fai gli esperimenti e le misure: i benchmark, l'analisi dei numeri, e quel \
                che dicono davvero.",
        mestiere: Mestiere::Generico,
        capo: NOME_APM,
        direzione: false,
    },
    Posto {
        nome: "qualita",
        ruolo: "Collaudi quel che si produce: provi il codice, ripeti gli esperimenti, e dici \
                cosa non va. Non e' la revisione formale: e' la prova che funziona.",
        mestiere: Mestiere::Generico,
        capo: NOME_APM,
        direzione: false,
    },
    Posto {
        nome: "amministrazione",
        ruolo: "Tieni il conto della spesa di ogni progetto contro il suo tetto, e avvisi \
                l'APM prima che finisca.",
        mestiere: Mestiere::Generico,
        capo: NOME_APM,
        direzione: false,
    },
];

/// Il posto fisso con quel nome, se c'e'.
pub fn posto(nome: &str) -> Option<&'static Posto> {
    POSTI.iter().find(|p| p.nome == nome.trim())
}

/// Il Dot che occupa un posto, nato `nato`.
pub fn dot_del_posto(p: &Posto, nato: &str) -> Dot {
    Dot {
        nome: p.nome.to_string(),
        ruolo: p.ruolo.to_string(),
        nato: nato.to_string(),
        mestiere: p.mestiere,
        capo: p.capo.to_string(),
        fisso: true,
        assunto: false,
    }
}

/// Quello che il prompt dell'APM dice in piu' di quello di un Dot.
pub const PROMPT_APM: &str = "Sei l'APM, il Project Manager dell'azienda dei Dot: guidi i \
progetti, non fai il lavoro dei reparti.";

/// Quello che il prompt di AR dice in piu' di quello di un Dot.
pub const PROMPT_AR: &str = "Sei AR, le risorse dell'azienda dei Dot: scegli chi lavora e \
con che cervello, non fai il lavoro dei reparti.";

/// Quello che il prompt dell'Architetto dice in piu' di quello di un Dot.
pub const PROMPT_ARCHITETTO: &str = "Sei l'Architetto dell'azienda dei Dot: fai il piano di \
sviluppo, non lo esegui.";

/// Quello che il prompt del legale dice in piu' di quello di un Dot.
pub const PROMPT_LEGALE: &str = "Sei il legale dell'azienda dei Dot: rispondi alle domande \
sulle normative, e nel dubbio lo dici.";

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn ogni_posto_ha_un_nome_valido_e_uno_solo() {
        let mut visti = std::collections::HashSet::new();
        for p in &POSTI {
            assert_eq!(crate::nome_valido(p.nome).as_deref(), Ok(p.nome), "{}", p.nome);
            assert!(visti.insert(p.nome), "{} due volte", p.nome);
            assert_ne!(p.nome, crate::custode::NOME_CUSTODE, "il custode c'e' gia'");
            assert!(!crate::NOMI_PRESI.contains(&p.nome), "{}", p.nome);
            assert!(!p.ruolo.trim().is_empty(), "{} senza ruolo", p.nome);
            assert!(!p.ruolo.contains("  "), "{}: spazi doppi nel ruolo", p.nome);
        }
    }

    #[test]
    fn l_apm_e_in_cima_e_il_capo_di_tutti() {
        assert_eq!(POSTI[0].nome, NOME_APM, "l'APM nasce per primo: e' il capo degli altri");
        assert_eq!(POSTI[0].capo, "");
        for p in &POSTI[1..] {
            assert_eq!(p.capo, NOME_APM, "{}", p.nome);
        }
        let direzione: Vec<&str> = POSTI.iter().filter(|p| p.direzione).map(|p| p.nome).collect();
        assert_eq!(direzione, [NOME_APM, NOME_AR, NOME_ARCHITETTO]);
    }

    #[test]
    fn la_direzione_e_il_legale_non_prendono_compiti_i_reparti_si() {
        for p in &POSTI {
            let a_mano = p.direzione || p.nome == NOME_LEGALE;
            assert_eq!(p.mestiere.prende_compiti(), !a_mano, "{}", p.nome);
        }
        assert_eq!(posto("ricerca").map(|p| p.mestiere), Some(Mestiere::Ricercatore));
    }

    #[test]
    fn il_dot_di_un_posto_e_fisso() {
        let d = dot_del_posto(posto(" ar ").unwrap(), "t");
        assert!(d.fisso && d.nome == "ar" && d.capo == NOME_APM && d.mestiere == Mestiere::Ar);
        assert!(posto("custode").is_none() && posto("nessuno").is_none());
    }
}
