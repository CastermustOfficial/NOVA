//! Cosa c'e' aperto nella finestra dell'harness, detto a NOVA.
//!
//! La chat dell'harness sta accanto ai file: chi scrive «spiegami questo»
//! intende il pezzo che ha selezionato, e chi scrive «qui c'e' un errore»
//! intende il file che ha davanti. Un cervello che non lo sa risponde a una
//! domanda diversa da quella fatta.
//!
//! Il contesto va **in coda alla domanda**, come la postilla della voce: e'
//! un'istruzione per il cervello, non qualcosa che l'utente ha detto. Non
//! entra nella ricerca in memoria e non si impara.
//!
//! Il contesto arriva dalla pagina cosi':
//!
//! ```json
//! { "cartella": "C:\\progetto",
//!   "attivo": { "percorso": "C:\\progetto\\src\\main.rs", "riga": 40, "modificato": true },
//!   "aperti": ["C:\\progetto\\src\\main.rs", "C:\\progetto\\README.md"],
//!   "selezione": { "da": 12, "a": 18, "testo": "..." } }
//! ```
//!
//! Tutto e' facoltativo: una finestra senza niente di aperto non aggiunge
//! niente alla domanda.

use serde_json::Value;

/// Quanti caratteri di selezione arrivano al cervello.
///
/// Una selezione e' una cosa che si indica, non un file da incollare: chi
/// seleziona trecento righe e chiede «cosa fa» fa prima a dire il file, e il
/// cervello il file lo sa leggere da se'.
pub const SELEZIONE_MAX: usize = 8_000;

/// Quanti altri file aperti si nominano.
pub const APERTI_MAX: usize = 12;

fn testo<'a>(v: &'a Value, chiave: &str) -> &'a str {
    v.get(chiave).and_then(Value::as_str).unwrap_or("").trim()
}

fn numero(v: &Value, chiave: &str) -> Option<u64> {
    v.get(chiave).and_then(Value::as_u64).filter(|n| *n > 0)
}

/// Il percorso come lo si dice: relativo alla cartella, se ci sta dentro.
///
/// Si confronta senza guardare le maiuscole e con le due barre uguali,
/// perche' su Windows `C:\Progetto` e `c:/progetto` sono la stessa cartella.
pub fn come_si_dice(percorso: &str, cartella: &str) -> String {
    let normale = |s: &str| s.replace('\\', "/").to_lowercase();
    let c = normale(cartella.trim_end_matches(['/', '\\']));
    let p = normale(percorso);
    if !c.is_empty() && p.starts_with(&format!("{c}/")) {
        let taglio = cartella.trim_end_matches(['/', '\\']).chars().count() + 1;
        return percorso
            .chars()
            .skip(taglio)
            .collect::<String>()
            .replace('\\', "/");
    }
    percorso.to_string()
}

/// Il nome del file, senza cartelle.
pub fn nome(percorso: &str) -> &str {
    percorso.rsplit(['/', '\\']).next().unwrap_or(percorso)
}

/// Un recinto per il codice che non si chiude dentro al codice: un apice
/// in piu' della fila piu' lunga che il testo contiene, e mai meno di tre.
pub fn recinto(dentro: &str) -> String {
    let mut piu_lunga = 0;
    let mut fila = 0;
    for c in dentro.chars() {
        if c == '`' {
            fila += 1;
            piu_lunga = piu_lunga.max(fila);
        } else {
            fila = 0;
        }
    }
    "`".repeat((piu_lunga + 1).max(3))
}

/// La postilla da attaccare alla domanda. Vuota se non c'e' niente di aperto.
pub fn postilla(contesto: &Value) -> String {
    let cartella = testo(contesto, "cartella");
    let attivo = contesto.get("attivo").cloned().unwrap_or(Value::Null);
    let percorso = testo(&attivo, "percorso");
    let aperti: Vec<&str> = contesto
        .get("aperti")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(str::trim).collect())
        .unwrap_or_default();
    let altri: Vec<&str> = aperti
        .iter()
        .copied()
        .filter(|p| !p.is_empty() && *p != percorso)
        .collect();
    if cartella.is_empty() && percorso.is_empty() && altri.is_empty() {
        return String::new();
    }

    let mut righe = vec![
        "<harness>".to_string(),
        "Questa domanda arriva dall'harness: l'utente ha dei file aperti accanto alla chat."
            .to_string(),
    ];
    if !cartella.is_empty() {
        righe.push(format!("Cartella del progetto: {cartella}"));
    }
    if !percorso.is_empty() {
        let mut r = format!("In primo piano: {}", come_si_dice(percorso, cartella));
        if let Some(n) = numero(&attivo, "riga") {
            r.push_str(&format!(", riga {n}"));
        }
        if attivo.get("modificato").and_then(Value::as_bool) == Some(true) {
            r.push_str(" (con modifiche non ancora salvate: su disco c'e' la versione di prima)");
        }
        righe.push(r);
    }
    if !altri.is_empty() {
        let mut nomi: Vec<String> = altri
            .iter()
            .take(APERTI_MAX)
            .map(|p| come_si_dice(p, cartella))
            .collect();
        if altri.len() > APERTI_MAX {
            nomi.push(format!("e altri {}", altri.len() - APERTI_MAX));
        }
        righe.push(format!("Aperti anche: {}", nomi.join(", ")));
    }

    let selezione = contesto.get("selezione").cloned().unwrap_or(Value::Null);
    let scelto = selezione.get("testo").and_then(Value::as_str).unwrap_or("");
    if !scelto.trim().is_empty() && !percorso.is_empty() {
        let dove = match (numero(&selezione, "da"), numero(&selezione, "a")) {
            (Some(da), Some(a)) if a > da => format!("le righe {da}-{a} di {}", nome(percorso)),
            (Some(da), _) => format!("un pezzo della riga {da} di {}", nome(percorso)),
            _ => format!("un pezzo di {}", nome(percorso)),
        };
        let quanti = scelto.chars().count();
        let mostrato: String = scelto.chars().take(SELEZIONE_MAX).collect();
        let r = recinto(&mostrato);
        righe.push(format!("Ha selezionato {dove}:"));
        righe.push(format!("{r}\n{mostrato}\n{r}"));
        if quanti > SELEZIONE_MAX {
            righe.push(format!(
                "(la selezione e' tagliata: sono {quanti} caratteri, qui ce ne sono {SELEZIONE_MAX})"
            ));
        }
        righe.push(
            "Se la domanda dice «questo», «qui» o «la selezione», parla di quel pezzo.".to_string(),
        );
    }
    righe.push(
        "Quando indichi un punto preciso scrivi file:riga (per esempio main.rs:40): \
         nell'harness diventa un collegamento che porta li'."
            .to_string(),
    );
    righe.push("</harness>".to_string());
    format!("\n\n{}", righe.join("\n"))
}

#[cfg(test)]
mod prove {
    use super::*;
    use serde_json::json;

    #[test]
    fn senza_niente_di_aperto_non_si_aggiunge_niente() {
        assert_eq!(postilla(&json!({})), "");
        assert_eq!(postilla(&json!({"aperti": [], "attivo": {}})), "");
        assert_eq!(postilla(&Value::Null), "");
    }

    #[test]
    fn il_file_davanti_si_dice_relativo_alla_cartella() {
        let p = postilla(&json!({
            "cartella": "C:\\Progetto",
            "attivo": {"percorso": "c:\\progetto\\src\\main.rs", "riga": 40},
            "aperti": ["c:\\progetto\\src\\main.rs", "D:\\altro\\note.md"],
        }));
        assert!(p.starts_with("\n\n<harness>\n"), "{p}");
        assert!(p.ends_with("\n</harness>"), "{p}");
        assert!(p.contains("In primo piano: src/main.rs, riga 40\n"), "{p}");
        assert!(p.contains("Aperti anche: D:\\altro\\note.md\n"), "{p}");
        assert!(p.contains("Cartella del progetto: C:\\Progetto\n"), "{p}");
        assert!(!p.contains("selezionato"), "{p}");
        assert!(!p.contains("non ancora salvate"), "{p}");
    }

    #[test]
    fn la_riga_zero_e_le_modifiche_assenti_non_si_dicono() {
        let p = postilla(&json!({"attivo": {"percorso": "/a/b.rs", "riga": 0, "modificato": false}}));
        assert!(p.contains("In primo piano: /a/b.rs\n"), "{p}");
    }

    #[test]
    fn anche_gli_altri_aperti_si_dicono_relativi() {
        let p = postilla(&json!({"cartella": "/p", "aperti": ["/p/src/a.rs"]}));
        assert!(p.contains("Aperti anche: src/a.rs\n"), "{p}");
    }

    #[test]
    fn le_modifiche_non_salvate_si_dicono() {
        let p = postilla(&json!({"attivo": {"percorso": "/a/b.rs", "modificato": true}}));
        assert!(
            p.contains("In primo piano: /a/b.rs (con modifiche non ancora salvate"),
            "{p}"
        );
        assert!(!p.contains(", riga"), "{p}");
    }

    #[test]
    fn la_selezione_va_nel_recinto_con_le_righe() {
        let p = postilla(&json!({
            "attivo": {"percorso": "/a/main.rs"},
            "selezione": {"da": 12, "a": 18, "testo": "let x = 1;"},
        }));
        assert!(
            p.contains("Ha selezionato le righe 12-18 di main.rs:\n```\nlet x = 1;\n```\n"),
            "{p}"
        );
        assert!(p.contains("«questo»"), "{p}");
        let una = postilla(&json!({
            "attivo": {"percorso": "/a/main.rs"},
            "selezione": {"da": 7, "a": 7, "testo": "x"},
        }));
        assert!(una.contains("un pezzo della riga 7 di main.rs"), "{una}");
        let senza = postilla(&json!({
            "attivo": {"percorso": "/a/main.rs"},
            "selezione": {"testo": "x"},
        }));
        assert!(senza.contains("un pezzo di main.rs"), "{senza}");
    }

    #[test]
    fn una_selezione_vuota_o_senza_file_non_conta() {
        let p = postilla(&json!({
            "attivo": {"percorso": "/a/main.rs"},
            "selezione": {"da": 1, "a": 2, "testo": "   "},
        }));
        assert!(!p.contains("selezionato"), "{p}");
        let q = postilla(&json!({"cartella": "/a", "selezione": {"testo": "x"}}));
        assert!(!q.contains("selezionato"), "{q}");
    }

    #[test]
    fn la_selezione_lunga_si_taglia_e_lo_dice() {
        let lunga = "a".repeat(SELEZIONE_MAX + 5);
        let p = postilla(&json!({"attivo": {"percorso": "/x.txt"}, "selezione": {"testo": lunga}}));
        assert!(
            p.contains(&format!("\n{}\n", "a".repeat(SELEZIONE_MAX))),
            "tagliata al tetto"
        );
        assert!(!p.contains(&"a".repeat(SELEZIONE_MAX + 1)));
        assert!(
            p.contains(&format!("sono {} caratteri", SELEZIONE_MAX + 5)),
            "{}",
            &p[p.len() - 300..]
        );
        let giusta = "b".repeat(SELEZIONE_MAX);
        let q =
            postilla(&json!({"attivo": {"percorso": "/x.txt"}, "selezione": {"testo": giusta}}));
        assert!(!q.contains("tagliata"));
    }

    #[test]
    fn gli_altri_aperti_si_contano_oltre_il_tetto() {
        let aperti: Vec<String> = (0..APERTI_MAX + 3).map(|i| format!("/p/f{i}.rs")).collect();
        let p = postilla(&json!({"cartella": "/p", "aperti": aperti}));
        assert!(
            p.contains(&format!("f{}.rs, e altri 3\n", APERTI_MAX - 1)),
            "{p}"
        );
        assert!(!p.contains(&format!("f{}.rs", APERTI_MAX)), "{p}");
        let giusti: Vec<String> = (0..APERTI_MAX).map(|i| format!("/p/f{i}.rs")).collect();
        let q = postilla(&json!({"cartella": "/p", "aperti": giusti}));
        assert!(!q.contains("e altri"), "{q}");
    }

    #[test]
    fn il_recinto_non_si_chiude_dentro_il_codice() {
        assert_eq!(recinto("niente"), "```");
        assert_eq!(recinto("a ``` b"), "````");
        assert_eq!(recinto("`` e `````"), "``````");
        assert_eq!(recinto("` `"), "```");
    }

    #[test]
    fn fuori_dalla_cartella_il_percorso_resta_intero() {
        assert_eq!(come_si_dice("/p/a/b.rs", "/p"), "a/b.rs");
        assert_eq!(come_si_dice("/p/a/b.rs", "/p/"), "a/b.rs");
        assert_eq!(come_si_dice("/pq/b.rs", "/p"), "/pq/b.rs");
        assert_eq!(come_si_dice("/p/b.rs", ""), "/p/b.rs");
        assert_eq!(come_si_dice("C:\\P\\è\\x.md", "c:\\p"), "è/x.md");
        assert_eq!(nome("C:\\a\\b.rs"), "b.rs");
        assert_eq!(nome("b.rs"), "b.rs");
    }
}
