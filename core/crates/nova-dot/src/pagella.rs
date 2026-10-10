//! La pagella dei Dot: il capo giudica, AR decide (D401).
//!
//! Deciso con Gio il 10 ottobre: un Dot nasce per il lavoro da fare, col
//! cervello che gli da' AR, e si giudica da quello che porta. Cosi' ogni
//! cervello finisce dove e' portato, e il piu' grande resta per dove serve
//! davvero: niente cannone per una mosca.
//!
//! - **Il capo giudica ogni consegna** col cervello piu' grande che risponde
//!   a un indirizzo: un voto da 1 a 10 e una riga di perche'. Un Dot senza
//!   capo lo giudica l'APM, il capo di tutti.
//! - **Sotto [`BOCCIATO_SOTTO`] il compito e' bocciato**, e si rifa' subito un
//!   gradino piu' su: all'utente arriva il lavoro rifatto. Sul gradino piu'
//!   alto non si sale: si tiene, e il voto resta.
//! - **AR guarda la pagella** dopo ogni voto ([`decidi`]): le ultime
//!   [`ULTIME`] consegne col cervello di adesso. Tutte da [`SCENDE_DA`] in su,
//!   il Dot scende di un gradino; [`BOCCIATE_PER_SALIRE`] o piu' bocciate,
//!   sale; gia' in cima e bocciato ancora, AR lo dice a Nova. Dopo un cambio
//!   si conta da capo.
//!
//! La pagella sta nella cartella del Dot (`pagella.jsonl`), una riga per
//! voto e una per cambio di cervello, e se ne va con lui in archivio. Qui ci
//! sono le regole e le parole; chi chiede il voto e chi cambia il cervello e'
//! `nova_core::pagella`.

use serde::{Deserialize, Serialize};

use crate::{Cartella, Dot};

/// Sotto questo voto la consegna e' bocciata, e si rifa' piu' su.
pub const BOCCIATO_SOTTO: u8 = 6;

/// Da questo voto in su la consegna e' cosi' buona che, se lo sono tutte le
/// ultime [`ULTIME`], il Dot prova un cervello piu' leggero.
pub const SCENDE_DA: u8 = 8;

/// Quante consegne guarda AR, le ultime col cervello di adesso.
pub const ULTIME: usize = 5;

/// Quante bocciature, fra le ultime [`ULTIME`], fanno salire il Dot.
pub const BOCCIATE_PER_SALIRE: usize = 2;

/// Quanto della consegna entra nella domanda al capo.
pub const CONSEGNA_IN_DOMANDA: usize = 6_000;

/// Quanti gettoni ha il capo per rispondere: due righe.
pub const GETTONI: u32 = 200;

/// Il segno in testa alla domanda al capo.
pub const SEGNO: &str = "[giudizio del capo]";

/// Una riga della pagella.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum Riga {
    /// Il voto del capo a una consegna.
    Voto {
        quando: String,
        compito: u64,
        /// Il cervello che l'ha fatta.
        cervello: String,
        voto: u8,
        perche: String,
        /// Chi ha giudicato: il capo, o l'APM.
        giudice: String,
        /// La consegna era gia' un rifacimento, dopo una bocciatura.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        rifatto: bool,
    },
    /// AR gli ha dato un cervello: assumendolo, riprendendolo, o guardando la
    /// pagella. Da qui si conta da capo.
    Cervello {
        quando: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        da: String,
        a: String,
        perche: String,
    },
}

impl Cartella {
    /// La pagella: i voti e i cambi di cervello, in ordine. Una riga che non
    /// si legge si salta.
    pub fn pagella(&self) -> Vec<Riga> {
        std::fs::read_to_string(self.radice.join("pagella.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|r| serde_json::from_str(r).ok())
            .collect()
    }

    /// Una riga in fondo alla pagella.
    pub fn annota_pagella(&self, riga: &Riga) -> Result<(), String> {
        let t = serde_json::to_string(riga).map_err(|e| e.to_string())?;
        crate::aggiungi_riga(&self.radice.join("pagella.jsonl"), &t)
    }
}

/// Il voto letto dalla risposta del capo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Giudizio {
    pub voto: u8,
    pub perche: String,
}

impl Giudizio {
    pub fn bocciato(&self) -> bool {
        self.voto < BOCCIATO_SOTTO
    }
}

/// Taglia un testo a `massimo` caratteri, e lo dice.
fn tagliato(t: &str, massimo: usize) -> String {
    let t = t.trim();
    if t.chars().count() <= massimo {
        return t.to_string();
    }
    let corto: String = t.chars().take(massimo).collect();
    format!("{}… (tagliato)", corto.trim_end())
}

/// La domanda al capo: chi giudica, chi ha lavorato, il compito, la
/// consegna, e come rispondere. Due righe in risposta: pochi gettoni.
pub fn domanda(giudice: &str, dot: &Dot, compito: &str, consegna: &str) -> String {
    format!(
        "{SEGNO} Sei «{giudice}», il capo di «{nome}» nell'azienda dei Dot. Il suo ruolo: {ruolo}\n\
         Giudica la sua consegna: se fa quello che il compito chiede, se e' corretta e \
         completa. Non rifarla.\n\n\
         Il compito:\n{compito}\n\n\
         La consegna:\n{consegna}\n\n\
         Rispondi con due righe e nient'altro:\n\
         VOTO: <da 1 a 10; sotto {BOCCIATO_SOTTO} si rifa'>\n\
         PERCHE: <una riga>",
        nome = dot.nome,
        ruolo = dot.ruolo.trim(),
        compito = tagliato(compito, CONSEGNA_IN_DOMANDA),
        consegna = tagliato(consegna, CONSEGNA_IN_DOMANDA),
    )
}

/// Il voto dalla risposta del capo: la riga `VOTO:` con un numero da 1 a 10,
/// e il perche', se c'e'. `None` se non si legge: allora non si giudica, e
/// nessuno sale per una risposta storta.
pub fn leggi(risposta: &str) -> Option<Giudizio> {
    let mut voto = None;
    let mut perche = String::new();
    for riga in risposta.lines() {
        let r = riga.replace("**", "");
        let r = r.trim().trim_start_matches(['-', '*']).trim();
        let Some((k, v)) = r.split_once(':') else {
            continue;
        };
        match k.trim().to_uppercase().as_str() {
            "VOTO" if voto.is_none() => {
                let cifre: String = v.trim().chars().take_while(char::is_ascii_digit).collect();
                // «7/10» si legge 7; «0» o «11» no.
                voto = cifre.parse::<u8>().ok().filter(|n| (1..=10).contains(n));
                voto?;
            }
            "PERCHE" | "PERCHÉ" | "PERCHE'" if perche.is_empty() => perche = v.trim().to_string(),
            _ => {}
        }
    }
    Some(Giudizio {
        voto: voto?,
        perche,
    })
}

/// Il testo con cui si rifa' un compito bocciato: il compito, e perche' il
/// capo l'ha bocciato.
pub fn rifai(compito: &str, g: &Giudizio) -> String {
    let perche = if g.perche.is_empty() {
        "non l'ha detto".to_string()
    } else {
        g.perche.clone()
    };
    format!(
        "{}\n\n(Da rifare: il tuo capo ha bocciato la consegna di prima con {} su 10. \
         Perche': {perche})",
        compito.trim(),
        g.voto
    )
}

/// Il gradino sopra `cervello` nella scala, se c'e'.
pub fn sopra(cervello: &str, scala: &[String]) -> Option<String> {
    let i = scala.iter().position(|g| g == cervello)?;
    scala.get(i + 1).cloned()
}

/// Il gradino sotto `cervello` nella scala, se c'e'.
pub fn sotto(cervello: &str, scala: &[String]) -> Option<String> {
    let i = scala.iter().position(|g| g == cervello)?;
    i.checked_sub(1).map(|j| scala[j].clone())
}

/// Cosa decide AR guardando la pagella.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decisione {
    /// Il cervello va bene com'e'.
    Tieni,
    /// Va bene sempre: prova un cervello piu' leggero.
    Scendi(String),
    /// Lo bocciano: un cervello piu' grande.
    Sali(String),
    /// E' gia' sul piu' grande, e lo bocciano ancora: lo si dice a Nova.
    Segnala,
}

impl Decisione {
    /// Come la scrive il registro.
    pub fn nome(&self) -> &'static str {
        match self {
            Decisione::Tieni => "tieni",
            Decisione::Scendi(_) => "scendi",
            Decisione::Sali(_) => "sali",
            Decisione::Segnala => "segnala",
        }
    }
}

/// I voti che contano per AR: gli ultimi [`ULTIME`] col cervello di adesso,
/// da dopo l'ultimo cambio di cervello.
pub fn ultimi(pagella: &[Riga], cervello: &str) -> Vec<u8> {
    let da = pagella
        .iter()
        .rposition(|r| matches!(r, Riga::Cervello { .. }))
        .map_or(0, |i| i + 1);
    let voti: Vec<u8> = pagella[da..]
        .iter()
        .filter_map(|r| match r {
            Riga::Voto {
                cervello: c, voto, ..
            } if c == cervello => Some(*voto),
            _ => None,
        })
        .collect();
    voti[voti.len().saturating_sub(ULTIME)..].to_vec()
}

/// Cosa decide AR dopo un voto, e perche'. `cervello` e' quello del Dot
/// adesso; uno che non e' nella scala non si sposta.
pub fn decidi(pagella: &[Riga], cervello: &str, scala: &[String]) -> (Decisione, String) {
    if !scala.iter().any(|g| g == cervello) {
        return (Decisione::Tieni, format!("«{cervello}» non e' nella scala"));
    }
    let voti = ultimi(pagella, cervello);
    let bocciate = voti.iter().filter(|v| **v < BOCCIATO_SOTTO).count();
    let elenco = voti
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    if bocciate >= BOCCIATE_PER_SALIRE {
        let perche = format!(
            "{bocciate} bocciate sulle ultime {} consegne con «{cervello}» ({elenco})",
            voti.len()
        );
        if let Some(su) = sopra(cervello, scala) {
            return (Decisione::Sali(su), perche);
        }
        // In cima non c'e' dove salire. Lo si dice quando arriva una
        // bocciatura, non a ogni voto buono che segue.
        if voti.last().is_some_and(|v| *v < BOCCIATO_SOTTO) {
            return (Decisione::Segnala, perche);
        }
        return (Decisione::Tieni, perche);
    }
    if voti.len() == ULTIME && voti.iter().all(|v| *v >= SCENDE_DA) {
        if let Some(giu) = sotto(cervello, scala) {
            return (
                Decisione::Scendi(giu),
                format!("le ultime {ULTIME} consegne con «{cervello}» tutte da {SCENDE_DA} in su ({elenco})"),
            );
        }
    }
    (Decisione::Tieni, String::new())
}

#[cfg(test)]
mod prove {
    use super::*;

    fn scala() -> Vec<String> {
        vec!["piccolo".into(), "medio".into(), "grande".into()]
    }

    fn voto(cervello: &str, v: u8) -> Riga {
        Riga::Voto {
            quando: "t".into(),
            compito: 1,
            cervello: cervello.into(),
            voto: v,
            perche: String::new(),
            giudice: "apm".into(),
            rifatto: false,
        }
    }

    fn cambio(a: &str) -> Riga {
        Riga::Cervello {
            quando: "t".into(),
            da: String::new(),
            a: a.into(),
            perche: "x".into(),
        }
    }

    #[test]
    fn il_voto_si_legge_anche_scritto_male() {
        assert_eq!(
            leggi("VOTO: 7\nPERCHE: manca una fonte"),
            Some(Giudizio {
                voto: 7,
                perche: "manca una fonte".into()
            })
        );
        assert_eq!(
            leggi("**Voto:** 9/10\n- Perché: completo").map(|g| (g.voto, g.perche)),
            Some((9, "completo".into()))
        );
        assert_eq!(leggi("Ecco.\nVOTO: 10").map(|g| g.voto), Some(10));
        for no in [
            "",
            "Va bene.",
            "VOTO: 0",
            "VOTO: 11",
            "VOTO: sette",
            "PERCHE: bello",
        ] {
            assert_eq!(leggi(no), None, "{no}");
        }
        assert!(leggi("VOTO: 5").unwrap().bocciato());
        assert!(!leggi("VOTO: 6").unwrap().bocciato());
    }

    #[test]
    fn la_domanda_dice_chi_cosa_e_come_rispondere() {
        let d = Dot {
            nome: "lettore".into(),
            ruolo: "Leggi e riassumi.".into(),
            nato: "t".into(),
            mestiere: crate::Mestiere::Generico,
            capo: "ricerca".into(),
            fisso: false,
            assunto: true,
            cervello: "piccolo".into(),
        };
        let q = domanda(
            "ricerca",
            &d,
            "riassumi il contratto",
            &"x".repeat(CONSEGNA_IN_DOMANDA + 10),
        );
        assert!(
            q.starts_with("[giudizio del capo] Sei «ricerca», il capo di «lettore»"),
            "{q}"
        );
        assert!(
            q.contains("Il suo ruolo: Leggi e riassumi.")
                && q.contains("Il compito:\nriassumi il contratto")
        );
        assert!(
            q.contains("… (tagliato)")
                && q.ends_with("VOTO: <da 1 a 10; sotto 6 si rifa'>\nPERCHE: <una riga>"),
            "{q}"
        );
        assert!(
            !q.contains("[piano]") && !q.contains("[risorse]"),
            "i segni degli altri sono altri"
        );
    }

    #[test]
    fn il_rifacimento_dice_perche() {
        let g = Giudizio {
            voto: 4,
            perche: "non ha le fonti".into(),
        };
        assert_eq!(
            rifai("cerca le fonti", &g),
            "cerca le fonti\n\n(Da rifare: il tuo capo ha bocciato la consegna di prima con 4 su 10. Perche': non ha le fonti)"
        );
        assert_eq!(sopra("piccolo", &scala()).as_deref(), Some("medio"));
        assert_eq!(sopra("grande", &scala()), None);
        assert_eq!(sotto("piccolo", &scala()), None);
        assert_eq!(sopra("altro", &scala()), None);
    }

    #[test]
    fn tutte_buone_scende_di_un_gradino() {
        let mut p: Vec<Riga> = (0..4).map(|_| voto("medio", 9)).collect();
        assert_eq!(
            decidi(&p, "medio", &scala()).0,
            Decisione::Tieni,
            "quattro non bastano"
        );
        p.push(voto("medio", 8));
        let (d, perche) = decidi(&p, "medio", &scala());
        assert_eq!(d, Decisione::Scendi("piccolo".into()));
        assert_eq!(
            perche,
            "le ultime 5 consegne con «medio» tutte da 8 in su (9, 9, 9, 9, 8)"
        );
        assert_eq!(
            decidi(
                &p.iter().map(|_| voto("piccolo", 9)).collect::<Vec<_>>(),
                "piccolo",
                &scala()
            )
            .0,
            Decisione::Tieni,
            "sotto il piu' piccolo non si scende"
        );
        p.push(voto("medio", 7));
        assert_eq!(
            decidi(&p, "medio", &scala()).0,
            Decisione::Tieni,
            "un 7 fra le ultime cinque"
        );
    }

    #[test]
    fn due_bocciate_sale_e_in_cima_si_segnala() {
        let p = vec![
            voto("piccolo", 9),
            voto("piccolo", 4),
            voto("piccolo", 8),
            voto("piccolo", 5),
        ];
        let (d, perche) = decidi(&p, "piccolo", &scala());
        assert_eq!(d, Decisione::Sali("medio".into()));
        assert_eq!(
            perche,
            "2 bocciate sulle ultime 4 consegne con «piccolo» (9, 4, 8, 5)"
        );
        let in_cima = vec![voto("grande", 3), voto("grande", 5)];
        assert_eq!(decidi(&in_cima, "grande", &scala()).0, Decisione::Segnala);
        let poi_buono = vec![voto("grande", 3), voto("grande", 5), voto("grande", 9)];
        assert_eq!(
            decidi(&poi_buono, "grande", &scala()).0,
            Decisione::Tieni,
            "si segnala alla bocciatura, non dopo"
        );
    }

    #[test]
    fn dopo_un_cambio_si_conta_da_capo_e_contano_solo_quelli_del_cervello() {
        let mut p = vec![voto("piccolo", 3), voto("piccolo", 4), cambio("medio")];
        assert_eq!(decidi(&p, "medio", &scala()).0, Decisione::Tieni);
        // Un rifacimento piu' su non conta per il cervello del Dot.
        p.extend([voto("medio", 9), voto("grande", 2), voto("medio", 9)]);
        assert_eq!(ultimi(&p, "medio"), [9, 9]);
        p.extend((0..6).map(|_| voto("medio", 10)));
        assert_eq!(
            ultimi(&p, "medio"),
            [10, 10, 10, 10, 10],
            "le ultime cinque"
        );
        assert_eq!(decidi(&p, "altro", &scala()).0, Decisione::Tieni);
    }

    #[test]
    fn la_pagella_va_e_torna_dal_disco() {
        let d = std::env::temp_dir().join(format!("nova-pagella-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let c = Cartella { radice: d.clone() };
        assert!(c.pagella().is_empty());
        c.annota_pagella(&voto("piccolo", 7)).unwrap();
        c.annota_pagella(&cambio("medio")).unwrap();
        std::fs::write(
            d.join("pagella.jsonl"),
            std::fs::read_to_string(d.join("pagella.jsonl")).unwrap() + "{rotta\n",
        )
        .unwrap();
        assert_eq!(c.pagella(), [voto("piccolo", 7), cambio("medio")]);
        let righe = std::fs::read_to_string(d.join("pagella.jsonl")).unwrap();
        assert!(righe.starts_with("{\"tipo\":\"voto\""), "{righe}");
        assert!(
            !righe.contains("rifatto") && !righe.contains("\"da\""),
            "i campi vuoti non si scrivono: {righe}"
        );
        let _ = std::fs::remove_dir_all(&d);
    }
}
