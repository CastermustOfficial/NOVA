//! Quando un Dot finisce un compito che gli ha dato Nova (D387).
//!
//! Deciso con Gio l'8 ottobre: Nova affida ai Dot senza chiedere il
//! permesso, e quando un Dot finisce un compito di Nova l'utente lo sa
//! **in chat e a voce**. Qui ci sono le parole: la riga per la chat, con
//! l'esito, e la frase da dire, corta e senza percorsi. Chi le consegna e'
//! `nova_core::dot`.

use crate::{Compito, Stato};

/// Chi affida, quando affida Nova: si scrive in `compiti.jsonl`, ed e' da li'
/// che si sa a chi consegnare.
pub const DA_NOVA: &str = "nova";

/// Quanto del compito si ripete nella chat e a voce.
pub const COMPITO_IN_BREVE: usize = 80;

/// Quanto dell'esito entra nella chat. Il resto sta nel rapporto o nello
/// stato del Dot: la chat e' un avviso, non il lavoro.
pub const ESITO_IN_CHAT: usize = 1_200;

/// Il compito l'ha dato Nova.
pub fn di_nova(c: &Compito) -> bool {
    c.da.trim() == DA_NOVA
}

/// Le parole di una consegna.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Avviso {
    /// La riga per la chat, con l'esito.
    pub chat: String,
    /// La frase da dire: corta, senza percorsi ne' esito.
    pub voce: String,
}

/// La prima riga del compito, al piu' `massimo` caratteri; se si taglia, lo
/// si dice con «…» (D129).
pub fn in_breve(testo: &str, massimo: usize) -> String {
    let riga = testo.trim().lines().next().unwrap_or("").trim();
    let mut corta: String = riga.chars().take(massimo).collect();
    let tagliata = riga.chars().count() > massimo || testo.trim().lines().count() > 1;
    if tagliata {
        corta = corta.trim_end().to_string();
        corta.push('…');
    }
    corta
}

/// L'esito per la chat: intero se ci sta, se no tagliato e detto.
fn esito_in_chat(esito: &str) -> String {
    let e = esito.trim();
    if e.chars().count() <= ESITO_IN_CHAT {
        return e.to_string();
    }
    let corto: String = e.chars().take(ESITO_IN_CHAT).collect();
    format!("{}… (il resto con dot.stato)", corto.trim_end())
}

/// Cosa dire quando un Dot chiude un compito di Nova. `None` se il compito
/// non e' chiuso: un avviso a meta' non c'e'.
pub fn avviso(dot: &str, compito: &Compito, stato: Stato, esito: &str) -> Option<Avviso> {
    if !stato.chiuso() {
        return None;
    }
    let breve = in_breve(&compito.testo, COMPITO_IN_BREVE);
    let esito = esito_in_chat(esito);
    let (chat, voce) = match stato {
        Stato::Fatto => (
            format!("{dot} ha finito il compito che gli avevo dato: «{breve}»."),
            format!("{dot} ha finito il compito che gli avevo dato."),
        ),
        Stato::Fallito => (
            format!("{dot} non e' riuscito a finire «{breve}»."),
            format!("{dot} non e' riuscito a finire il compito che gli avevo dato."),
        ),
        Stato::Fermato => (
            format!("{dot} si e' fermato su «{breve}»."),
            format!("{dot} si e' fermato."),
        ),
        Stato::Interrotto => (
            format!("{dot} ha lasciato «{breve}»: troppi riavvii a meta'."),
            format!("{dot} ha lasciato il compito che gli avevo dato."),
        ),
        Stato::Affidato | Stato::InCorso | Stato::InAttesa => return None,
    };
    let chat = if esito.is_empty() {
        chat
    } else {
        format!("{chat}\n\n{esito}")
    };
    Some(Avviso { chat, voce })
}

#[cfg(test)]
mod prove {
    use super::*;

    fn compito(testo: &str, da: &str) -> Compito {
        Compito {
            id: 3,
            testo: testo.into(),
            da: da.into(),
            stato: Stato::InCorso,
            affidato: String::new(),
            iniziato: String::new(),
            finito: String::new(),
            esito: String::new(),
            riprese: 0,
            padre: None,
            attende: Vec::new(),
            attese: 0,
            cervello: String::new(),
        }
    }

    #[test]
    fn si_avvisa_solo_per_i_compiti_di_nova() {
        assert!(di_nova(&compito("x", "nova")));
        assert!(di_nova(&compito("x", " nova ")));
        assert!(!di_nova(&compito("x", "utente")));
        assert!(!di_nova(&compito("x", "")));
    }

    #[test]
    fn fatto_la_chat_porta_l_esito_e_la_voce_no() {
        let a = avviso(
            "ricercatore",
            &compito("Cerca i prezzi dei biglietti per Lisbona", "nova"),
            Stato::Fatto,
            "Rapporto in C:\\NOVA\\dots\\ricercatore\\rapporti\\3.md. 2 fonti citate: 2 viste.",
        )
        .unwrap();
        assert_eq!(
            a.chat,
            "ricercatore ha finito il compito che gli avevo dato: «Cerca i prezzi dei biglietti \
             per Lisbona».\n\nRapporto in C:\\NOVA\\dots\\ricercatore\\rapporti\\3.md. 2 fonti \
             citate: 2 viste."
        );
        assert_eq!(
            a.voce,
            "ricercatore ha finito il compito che gli avevo dato."
        );
        assert!(!a.voce.contains("rapporti"));
    }

    #[test]
    fn ogni_stato_chiuso_ha_le_sue_parole_e_gli_aperti_nessuna() {
        let c = compito("scrivi la lettera", "nova");
        let f = avviso("a", &c, Stato::Fallito, "il cervello non risponde").unwrap();
        assert_eq!(
            f.chat,
            "a non e' riuscito a finire «scrivi la lettera».\n\nil cervello non risponde"
        );
        let s = avviso("a", &c, Stato::Fermato, "").unwrap();
        assert_eq!(
            (s.chat.as_str(), s.voce.as_str()),
            (
                "a si e' fermato su «scrivi la lettera».",
                "a si e' fermato."
            )
        );
        assert!(avviso("a", &c, Stato::Interrotto, "")
            .unwrap()
            .chat
            .contains("riavvii"));
        assert!(avviso("a", &c, Stato::Affidato, "").is_none());
        assert!(avviso("a", &c, Stato::InCorso, "").is_none());
        assert!(avviso("a", &c, Stato::InAttesa, "").is_none(), "in attesa non e' finito");
    }

    #[test]
    fn il_compito_e_l_esito_lunghi_si_tagliano_e_lo_si_dice() {
        assert_eq!(in_breve("  corto  ", 80), "corto");
        assert_eq!(in_breve("prima riga\nseconda", 80), "prima riga…");
        let lungo = "a".repeat(100);
        assert_eq!(in_breve(&lungo, 80), format!("{}…", "a".repeat(80)));
        let esito = "e".repeat(ESITO_IN_CHAT + 50);
        let a = avviso("d", &compito("x", "nova"), Stato::Fatto, &esito).unwrap();
        let parte = a.chat.split("\n\n").nth(1).unwrap();
        assert_eq!(
            parte,
            format!("{}… (il resto con dot.stato)", "e".repeat(ESITO_IN_CHAT))
        );
    }
}
