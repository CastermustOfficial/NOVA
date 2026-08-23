//! Il riconoscimento della parola di risveglio.
//!
//! I casi qui non sono inventati: sono trascrizioni vere prodotte da whisper
//! mentre l'utente chiamava NOVA. «No, va» al posto di «Nova» e' successo, e
//! senza il confronto senza spazi la chiamata veniva scartata.

use nova_core::caps_voce::dopo_il_risveglio;

#[test]
fn riconosce_le_forme_vere() {
    assert_eq!(dopo_il_risveglio("Nova, che ore sono?", "nova").as_deref(), Some("che ore sono"));
    assert_eq!(dopo_il_risveglio("nova apri i progetti", "nova").as_deref(), Some("apri i progetti"));
    assert_eq!(dopo_il_risveglio("NOVA.", "nova").as_deref(), Some(""));
    // come l'ha scritta whisper davvero, stasera
    assert_eq!(
        dopo_il_risveglio("No, va sarebbe molto bello se mi rispondessi", "nova").as_deref(),
        Some("sarebbe molto bello se mi rispondessi")
    );
}

#[test]
fn tollera_le_lettere_di_troppo_davanti() {
    // Anche questa e' vera: whisper cerca una parola italiana che suoni come
    // «Nova» e scrive «Innova».
    assert_eq!(
        dopo_il_risveglio("Innova, chi e orisono?", "nova").as_deref(),
        Some("chi e orisono")
    );
    assert_eq!(dopo_il_risveglio("Anova apri i progetti", "nova").as_deref(), Some("apri i progetti"));
    // Due lettere e' il confine: da tre in su non si passa.
    assert!(dopo_il_risveglio("rinnova l'abbonamento", "nova").is_none());
    assert!(dopo_il_risveglio("si rinnova da solo", "nova").is_none());
    // E il resto della frase non deve poter arrivare al nome per caso.
    assert!(dopo_il_risveglio("questa cosa e nova per me", "nova").is_none());
}

#[test]
fn non_si_risveglia_per_caso() {
    assert!(dopo_il_risveglio("parlami di Nova", "nova").is_none());
    assert!(dopo_il_risveglio("che ore sono", "nova").is_none());
    assert!(dopo_il_risveglio("no che va bene cosi", "nova").is_none());
    assert!(dopo_il_risveglio("", "nova").is_none());
    assert!(dopo_il_risveglio("nova", "").is_none());
    // Un nome corto non tollera niente: con «ada» anche «lada» o «rada»
    // diventerebbero un risveglio, e ci si sveglierebbe di continuo.
    assert!(dopo_il_risveglio("lada accendi la luce", "ada").is_none());
}

#[test]
fn tollera_la_punteggiatura_di_whisper() {
    for forma in ["Nova!", "Nova...", "«Nova»", "  nova  "] {
        assert!(
            dopo_il_risveglio(&format!("{forma} accendi la luce"), "nova").is_some(),
            "non riconosciuta: {forma}"
        );
    }
}
