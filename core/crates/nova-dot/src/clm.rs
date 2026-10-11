//! I testi di CLM per le scelte dei Dot (D404).
//!
//! Deciso con Gio l'11 ottobre 2026, sulla bozza: CLM impara tre scelte
//! dell'azienda dei Dot, oltre a «quale cervello» della delega (D378):
//!
//! - **il custode**: un permesso chiesto da un Dot si consente o si nega.
//!   CLM decide nei due versi, sopra la sua soglia;
//! - **AR, chi lavora**: riprendere un Dot che c'e' o assumerne uno nuovo.
//!   Il Dot nuovo (nome, ruolo, mestiere) lo scrive sempre il cervello
//!   grande: CLM sceglie, non scrive;
//! - **AR, il cervello**: il gradino piu' piccolo che fa bene il compito.
//!
//! CLM sceglie fra candidati scritti: lo stato e' la situazione con
//! un'istruzione in coda, i candidati sono le risposte possibili. Qui ci sono
//! **gli stessi testi** per l'addestramento e per il demone: le prove e
//! `misure/clm_addestra.py` li prendono da `banco-testi-clm`, che chiama
//! queste funzioni. Cio' che si impara e' cio' che si chiede.

use serde::Serialize;

use crate::custode;
use crate::ricerca::tagliato;
use crate::risorse::{self, Candidato, Scelta, TESTO_MASSIMO};
use crate::Mestiere;

/// Le scelte che CLM puo' imparare, coi nomi che usano le teste
/// (`teste.json`, `soglie`) e l'addestramento.
pub const QUALE_CERVELLO: &str = "quale_cervello";
pub const CUSTODE: &str = "custode";
pub const AR_CHI: &str = "ar_chi";
pub const AR_CERVELLO: &str = "ar_cervello";

/// Una domanda per CLM: lo stato, i candidati, e cosa vuol dire ognuno.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Domanda {
    pub stato: String,
    pub candidati: Vec<String>,
    /// Per ogni candidato, cosa si sceglie: `si`/`no` per il custode, il
    /// nome del Dot o [`ASSUMI_ID`] per chi lavora, il nome del gradino per
    /// il cervello.
    pub ids: Vec<String>,
}

// ------------------------------------------------------------ il custode

/// Il custode: la situazione e' quella che legge il modello di casa
/// ([`custode::stato`]), l'istruzione e le due risposte sono le sue
/// ([`custode::ISTRUZIONI`], [`custode::VERO`], [`custode::FALSO`]).
pub fn custode(stato: &str) -> Domanda {
    Domanda {
        stato: format!("{}\n\n{}", stato.trim(), custode::ISTRUZIONI),
        candidati: vec![custode::VERO.to_string(), custode::FALSO.to_string()],
        ids: vec!["si".into(), "no".into()],
    }
}

// ------------------------------------------------------------------- AR

/// L'id del candidato «assumi».
pub const ASSUMI_ID: &str = "__assumi__";

/// Il candidato «assumi»: nessuno di quelli che ci sono fa al caso.
pub const ASSUMI: &str = "Assumere un Dot nuovo: nessuno di quelli che ci sono fa al caso.";

/// L'istruzione per chi lavora, come la dice la domanda di AR.
pub const ISTRUZIONE_CHI: &str = "Chi fa questo lavoro? Un Dot che c'e', se fa al caso ed e' \
libero; uno nuovo solo se nessuno fa al caso.";

/// L'istruzione per il cervello, come la dice la domanda di AR.
pub const ISTRUZIONE_CERVELLO: &str = "Con che cervello? Il piu' piccolo che fa bene questo \
compito: un revisore puo' farlo rifare piu' su, ma costa.";

/// Il lavoro, come lo legge AR: cosa serve e il compito, tagliati come
/// nella sua domanda ([`risorse::richiesta`]).
fn lavoro(bisogno: &str, compito: &str) -> String {
    let compito = if compito.trim().is_empty() {
        "(nessuno per ora)".to_string()
    } else {
        tagliato(compito, TESTO_MASSIMO)
    };
    format!(
        "Cosa serve: {}\nIl compito: {compito}",
        tagliato(bisogno, TESTO_MASSIMO)
    )
}

/// Un Dot che c'e', come candidato: come lo elenca la domanda di AR.
pub fn riprendi(c: &Candidato) -> String {
    format!(
        "Riprendere {}, {} ({}, {} compiti fatti, {} falliti): {}",
        c.nome,
        if c.mestiere == Mestiere::Ricercatore {
            "ricercatore"
        } else {
            "generico"
        },
        if c.libero { "libero" } else { "occupato" },
        c.fatti,
        c.falliti,
        tagliato(&c.ruolo, risorse::RUOLO_MASSIMO),
    )
}

/// Chi lavora: i Dot che ci sono, nell'ordine in cui li legge AR, e in
/// fondo «assumi».
pub fn ar_chi(bisogno: &str, compito: &str, candidati: &[Candidato]) -> Domanda {
    let mut c: Vec<String> = candidati.iter().map(riprendi).collect();
    let mut ids: Vec<String> = candidati.iter().map(|x| x.nome.clone()).collect();
    c.push(ASSUMI.to_string());
    ids.push(ASSUMI_ID.to_string());
    Domanda {
        stato: format!("{}\n\n{ISTRUZIONE_CHI}", lavoro(bisogno, compito)),
        candidati: c,
        ids,
    }
}

/// Un gradino come candidato: il nome e il posto nella scala, che va dal
/// piu' piccolo al piu' grande.
pub fn gradino(nome: &str, posto: usize, quanti: usize) -> String {
    format!("Il cervello «{nome}», il {posto} di {quanti} dal piu' piccolo al piu' grande.")
}

/// Il cervello: i gradini della scala, dal piu' piccolo.
pub fn ar_cervello(bisogno: &str, compito: &str, scala: &[String]) -> Domanda {
    Domanda {
        stato: format!("{}\n\n{ISTRUZIONE_CERVELLO}", lavoro(bisogno, compito)),
        candidati: scala
            .iter()
            .enumerate()
            .map(|(i, n)| gradino(n, i + 1, scala.len()))
            .collect(),
        ids: scala.to_vec(),
    }
}

/// La riga che si aggiunge alla domanda di AR quando una parte della scelta
/// l'ha gia' fatta CLM: il cervello grande sceglie il resto.
pub fn gia_deciso(chi: Option<&str>, cervello: Option<&str>) -> String {
    let mut t = String::new();
    match chi {
        Some(ASSUMI_ID) => t.push_str(
            "\nSi e' gia' deciso che nessuno fa al caso: rispondi con «assumi», e scrivi il Dot nuovo.",
        ),
        Some(dot) => t.push_str(&format!(
            "\nSi e' gia' deciso che lo fa «{dot}»: rispondi con «riprendi» e quel Dot."
        )),
        None => {}
    }
    if let Some(g) = cervello {
        t.push_str(&format!("\nIl cervello e' gia' deciso: «{g}»."));
    }
    t
}

/// La scelta di AR quando CLM ne ha fatta una parte sopra la soglia: chi
/// lavora (`chi`, il nome di un Dot o [`ASSUMI_ID`]) e il cervello. Le parti
/// di CLM valgono; il resto e' del cervello grande. Torna la scelta e le
/// note su cosa si e' cambiato.
///
/// Un «assumi» di CLM non si impone: il Dot nuovo lo scrive il cervello
/// grande, e se quello riprende un Dot che c'e' vale la sua scelta, e si
/// dice.
pub fn con_clm(grande: Scelta, chi: Option<&str>, cervello: Option<&str>) -> (Scelta, Vec<String>) {
    let mut note = Vec::new();
    let mut s = match (chi, grande) {
        (
            Some(dot),
            Scelta::Riprendi {
                dot: suo,
                cervello: c,
                perche,
            },
        ) if dot != ASSUMI_ID => {
            if suo != dot {
                note.push(format!(
                    "CLM ha scelto «{dot}», il cervello grande «{suo}»: vale CLM"
                ));
            }
            Scelta::Riprendi {
                dot: dot.to_string(),
                cervello: c,
                perche,
            }
        }
        (
            Some(dot),
            Scelta::Assumi {
                nome,
                cervello: c,
                perche,
                ..
            },
        ) if dot != ASSUMI_ID => {
            note.push(format!(
                "CLM ha scelto «{dot}», il cervello grande voleva assumere «{nome}»: vale CLM"
            ));
            Scelta::Riprendi {
                dot: dot.to_string(),
                cervello: c,
                perche,
            }
        }
        (Some(_), g @ Scelta::Riprendi { .. }) => {
            note.push(format!(
                "CLM voleva assumere, ma il cervello grande ha ripreso «{}»: il Dot nuovo lo \
                 scrive lui, e vale la sua scelta",
                g.dot()
            ));
            g
        }
        (_, g) => g,
    };
    if let Some(g) = cervello {
        let c = match &mut s {
            Scelta::Riprendi { cervello, .. } | Scelta::Assumi { cervello, .. } => cervello,
        };
        if c != g {
            note.push(format!("il cervello l'ha scelto CLM: «{g}», non «{c}»"));
            *c = g.to_string();
        }
    }
    (s, note)
}

#[cfg(test)]
mod prove {
    use super::*;

    fn cand(nome: &str, libero: bool) -> Candidato {
        Candidato {
            nome: nome.into(),
            ruolo: "Cerca fonti e scrive rapporti.".into(),
            mestiere: Mestiere::Ricercatore,
            libero,
            fatti: 3,
            falliti: 1,
        }
    }

    #[test]
    fn il_custode_ha_le_parole_della_domanda_si_no() {
        let d = custode(" Un Dot vuole fare un'azione. ");
        assert_eq!(
            d.stato,
            format!("Un Dot vuole fare un'azione.\n\n{}", custode::ISTRUZIONI)
        );
        assert_eq!(d.candidati, [custode::VERO, custode::FALSO]);
        assert_eq!(d.ids, ["si", "no"]);
    }

    #[test]
    fn chi_lavora_elenca_i_dot_e_in_fondo_assumi() {
        let d = ar_chi(
            "un ricercatore",
            "",
            &[cand("lettore", true), cand("scrivano", false)],
        );
        assert_eq!(
            d.stato,
            format!(
                "Cosa serve: un ricercatore\nIl compito: (nessuno per ora)\n\n{ISTRUZIONE_CHI}"
            )
        );
        assert_eq!(
            d.candidati[0],
            "Riprendere lettore, ricercatore (libero, 3 compiti fatti, 1 falliti): Cerca fonti e scrive rapporti."
        );
        assert!(d.candidati[1].contains("(occupato,"));
        assert_eq!(d.candidati[2], ASSUMI);
        assert_eq!(d.ids, ["lettore", "scrivano", ASSUMI_ID]);
        assert_eq!(
            ar_chi("x", "", &[]).ids,
            [ASSUMI_ID],
            "senza Dot resta solo assumi"
        );
    }

    #[test]
    fn il_cervello_dice_il_posto_nella_scala() {
        let d = ar_cervello("x", "riassumi", &["piccolo".into(), "grande".into()]);
        assert!(d
            .stato
            .starts_with("Cosa serve: x\nIl compito: riassumi\n\nCon che cervello?"));
        assert_eq!(
            d.candidati,
            [
                "Il cervello «piccolo», il 1 di 2 dal piu' piccolo al piu' grande.",
                "Il cervello «grande», il 2 di 2 dal piu' piccolo al piu' grande."
            ]
        );
        assert_eq!(d.ids, ["piccolo", "grande"]);
    }

    fn riprendi_(dot: &str, c: &str) -> Scelta {
        Scelta::Riprendi {
            dot: dot.into(),
            cervello: c.into(),
            perche: "p".into(),
        }
    }

    fn assumi_(nome: &str, c: &str) -> Scelta {
        Scelta::Assumi {
            nome: nome.into(),
            ruolo: "r".into(),
            mestiere: Mestiere::Generico,
            cervello: c.into(),
            perche: "p".into(),
        }
    }

    #[test]
    fn le_parti_di_clm_valgono_il_resto_e_del_cervello_grande() {
        assert_eq!(
            con_clm(riprendi_("a", "medio"), None, None),
            (riprendi_("a", "medio"), vec![])
        );
        let (s, n) = con_clm(riprendi_("b", "medio"), Some("a"), None);
        assert_eq!(s, riprendi_("a", "medio"));
        assert!(n[0].contains("vale CLM"), "{n:?}");
        let (s, n) = con_clm(assumi_("nuovo", "grande"), Some("a"), Some("piccolo"));
        assert_eq!(s, riprendi_("a", "piccolo"));
        assert_eq!(n.len(), 2, "{n:?}");
        let (s, n) = con_clm(riprendi_("b", "medio"), Some(ASSUMI_ID), None);
        assert_eq!(s, riprendi_("b", "medio"), "un assumi di CLM non si impone");
        assert!(n[0].contains("vale la sua scelta"), "{n:?}");
        let (s, n) = con_clm(assumi_("nuovo", "grande"), Some(ASSUMI_ID), Some("medio"));
        assert_eq!(s, assumi_("nuovo", "medio"));
        assert_eq!(n, ["il cervello l'ha scelto CLM: «medio», non «grande»"]);
    }

    #[test]
    fn quel_che_ha_deciso_clm_lo_sa_il_cervello_grande() {
        assert_eq!(gia_deciso(None, None), "");
        assert!(gia_deciso(Some(ASSUMI_ID), None).contains("rispondi con «assumi»"));
        let t = gia_deciso(Some("lettore"), Some("medio"));
        assert!(
            t.contains("lo fa «lettore»") && t.ends_with("Il cervello e' gia' deciso: «medio»."),
            "{t}"
        );
    }
}
