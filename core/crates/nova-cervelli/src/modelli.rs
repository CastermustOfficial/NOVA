//! Il modello piu' recente di una famiglia, invece di un nome scritto una
//! volta per sempre (D377).
//!
//! I gradini di fabbrica dicevano `claude-sonnet-5` e `claude-opus-5`, scritti
//! per esteso perche' su Claude Code 2.0.42 gli alias erano rimasti indietro.
//! Un nome per esteso invecchia nello stesso modo, solo piu' piano: il 7
//! ottobre c'erano Sonnet 5.5 e Opus 5.5, e NOVA chiamava ancora i 5.
//!
//! Adesso un modello si puo' scrivere come `ultimo:<famiglia>`, e NOVA
//! sceglie il piu' recente di quella famiglia che parte davvero:
//!
//! - **Claude Code** non ha un elenco dei modelli. Si chiede alla CLI a quale
//!   modello porta l'alias della famiglia (`--model sonnet`), e poi si
//!   provano i nomi successivi: una prova con un nome che non esiste costa
//!   zero e torna in un paio di secondi (misurato: errore 404, 0 $). Gli
//!   alias da soli non bastano: su Claude Code 2.1.292 `sonnet` porta ancora
//!   a `claude-sonnet-5`, e `claude-sonnet-5-5` esiste e risponde.
//! - **Le CLI che hanno un elenco** (Antigravity: `agy models`) lo dicono da
//!   sole: si legge l'elenco e si sceglie il nome piu' recente che ha la
//!   forma della famiglia, per esempio `gemini-*-pro-high`.
//!
//! Qui ci sono solo le regole, senza processi: chi lancia le prove e tiene
//! il catalogo e' `nova_core::modelli`.

use serde_json::Value;

/// Il prefisso di un modello che si sceglie da solo.
pub const PREFISSO: &str = "ultimo:";

/// La famiglia, se il modello e' scritto come `ultimo:<famiglia>`.
pub fn famiglia(modello: &str) -> Option<&str> {
    modello
        .trim()
        .strip_prefix(PREFISSO)
        .map(str::trim)
        .filter(|f| !f.is_empty())
}

/// I nomi che erano i predefiniti di fabbrica dei gradini prima del D377,
/// con la famiglia che li sostituisce. Chi li ha nel file non li ha scelti:
/// li ha trovati li', perche' il pannello salva la configurazione intera.
/// Si leggono come `ultimo:<famiglia>`; chi vuole davvero quel modello lo
/// scrive in un'altra forma (`claude-opus-5[1m]`, o un alias). Il prezzo e'
/// che un utente che aveva scelto proprio quel nome, uguale al predefinito,
/// sale di versione: non si distingue da chi non l'ha mai toccato.
pub const VECCHI_PREDEFINITI: [(&str, &str); 2] =
    [("claude-sonnet-5", "sonnet"), ("claude-opus-5", "opus")];

/// La famiglia di Claude da scegliere da sola, se il modello la chiede:
/// `ultimo:<famiglia>`, o un vecchio predefinito di fabbrica.
pub fn famiglia_claude(modello: &str) -> Option<String> {
    if let Some(f) = famiglia(modello) {
        return Some(f.to_string());
    }
    VECCHI_PREDEFINITI
        .iter()
        .find(|(v, _)| *v == modello.trim())
        .map(|(_, f)| f.to_string())
}

/// Cosa si passa a Claude Code quando il catalogo non ha ancora un nome: la
/// famiglia stessa, che e' un alias della CLI e parte sempre. Un modello
/// scritto per esteso resta com'e'.
pub fn alias_claude(modello: &str) -> String {
    famiglia_claude(modello).unwrap_or_else(|| modello.to_string())
}

/// I numeri della versione in un pezzo di nome: `5-5` e' `[5, 5]`, `3.1` e'
/// `[3, 1]`. `None` se dentro c'e' qualcosa che non e' un numero o un
/// separatore: `5-5-high` non e' una versione.
pub fn versione(pezzo: &str) -> Option<Vec<u64>> {
    if pezzo.is_empty() {
        return None;
    }
    let mut fuori = Vec::new();
    for p in pezzo.split(['-', '.']) {
        if p.is_empty() || !p.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        fuori.push(p.parse().ok()?);
    }
    Some(fuori)
}

/// Il nome piu' recente dell'elenco che ha la forma della famiglia.
///
/// La forma ha un solo `*`, al posto della versione: `gemini-*-pro-high`
/// prende `gemini-3.1-pro-high` e non `gemini-3.8-flash-high`, e nemmeno
/// `gemini-3.1-pro-highest`. Senza `*` la forma e' un nome intero, e vale
/// solo se c'e' nell'elenco.
pub fn scegli(forma: &str, elenco: &[String]) -> Option<String> {
    let Some((prima, dopo)) = forma.split_once('*') else {
        return elenco.iter().find(|n| n.as_str() == forma).cloned();
    };
    elenco
        .iter()
        .filter_map(|n| {
            let resto = n.strip_prefix(prima)?.strip_suffix(dopo)?;
            Some((versione(resto)?, n))
        })
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, n)| n.clone())
}

/// I nomi in un elenco come lo stampa una CLI: il primo pezzo di ogni riga,
/// se ha la forma di un nome di modello (lettere minuscole, cifre, `-`, `.`,
/// e almeno una cifra). Le righe di contorno, come «Fetching available
/// models...», non lo hanno.
pub fn elenco(testo: &str) -> Vec<String> {
    testo
        .lines()
        .filter_map(|r| r.split_whitespace().next())
        .filter(|n| {
            n.chars().any(|c| c.is_ascii_digit())
                && n.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.')
                && n.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        })
        .map(str::to_string)
        .collect()
}

/// Il nome di Claude senza la data in coda: `claude-haiku-4-5-20251001` e'
/// `claude-haiku-4-5`. Anche `[1m]`, che Claude Code aggiunge al nome del
/// modello col contesto lungo, va via.
pub fn senza_data(nome: &str) -> String {
    let n = nome.split('[').next().unwrap_or(nome);
    match n.rsplit_once('-') {
        Some((testa, coda)) if coda.len() == 8 && coda.chars().all(|c| c.is_ascii_digit()) => {
            testa.to_string()
        }
        _ => n.to_string(),
    }
}

/// La versione di un nome di Claude della famiglia: `claude-opus-5-5` e'
/// `[5, 5]`, `claude-opus-5` e' `[5]`.
pub fn versione_claude(famiglia: &str, nome: &str) -> Option<Vec<u64>> {
    let n = senza_data(nome);
    versione(n.strip_prefix(&format!("claude-{famiglia}-"))?)
}

/// I nomi da provare dopo questo, dal piu' vicino.
///
/// Anthropic non numera in fila: dopo 4 sono venuti 4.1 e 4.5, dopo 5 sono
/// venuti 5.1 (Fable) e 5.5. Quindi si provano tutte le minori che restano
/// fino a 9, poi la maggiore dopo e le sue minori. Sono al piu' venti prove,
/// e quelle che non esistono non costano niente.
pub fn successori(famiglia: &str, nome: &str) -> Vec<String> {
    let Some(v) = versione_claude(famiglia, nome) else {
        return Vec::new();
    };
    let maggiore = v[0];
    let minore = v.get(1).copied().unwrap_or(0);
    let mut fuori: Vec<String> = ((minore + 1)..=9)
        .map(|m| format!("claude-{famiglia}-{maggiore}-{m}"))
        .collect();
    fuori.push(format!("claude-{famiglia}-{}", maggiore + 1));
    fuori.extend((1..=9).map(|m| format!("claude-{famiglia}-{}-{m}", maggiore + 1)));
    fuori
}

/// Com'e' andata una prova di Claude Code (`--output-format json`).
#[derive(Debug, Clone, PartialEq)]
pub enum Prova {
    /// Ha risposto; il nome e' quello che la CLI dice di aver usato.
    Va(String),
    /// Il modello non c'e', o non e' per questo account.
    NonEsiste,
    /// C'e', ma questa versione di Claude Code non lo sa usare: serve quella
    /// scritta qui, o una piu' nuova.
    ServeAggiornare(String),
    /// Qualunque altra cosa: la si racconta e non si conclude niente.
    Altro(String),
}

/// La prova letta dall'uscita di Claude Code.
///
/// Il nome del modello usato si prende da `modelUsage`, saltando quelli
/// di contorno: Claude Code chiama Haiku per i titoli, e senza filtrare per
/// famiglia una prova di Opus direbbe Haiku.
pub fn leggi_prova(famiglia: &str, uscita: &str) -> Prova {
    let Some(inizio) = uscita.find('{') else {
        return Prova::Altro(uscita.chars().take(200).collect());
    };
    let mut flusso = serde_json::Deserializer::from_str(&uscita[inizio..]).into_iter::<Value>();
    let Some(Ok(j)) = flusso.next() else {
        return Prova::Altro(uscita.chars().take(200).collect());
    };
    let testo = j.get("result").and_then(Value::as_str).unwrap_or("");
    if j.get("is_error").and_then(Value::as_bool) == Some(true) {
        if j.get("api_error_status").and_then(Value::as_i64) == Some(404)
            || testo.contains("may not exist")
        {
            return Prova::NonEsiste;
        }
        if let Some(dopo) = testo.split("version ").nth(1) {
            if testo.contains("does not support this model") {
                let v: String = dopo
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == '.')
                    .collect();
                return Prova::ServeAggiornare(v.trim_end_matches('.').to_string());
            }
        }
        return Prova::Altro(testo.chars().take(200).collect());
    }
    let usati = j
        .get("modelUsage")
        .and_then(Value::as_object)
        .map(|o| o.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    match usati
        .iter()
        .find(|n| n.contains(&format!("-{famiglia}-")) || n.ends_with(&format!("-{famiglia}")))
    {
        Some(n) => Prova::Va(senza_data(n)),
        None => Prova::Altro(format!("ha risposto, ma con {usati:?}")),
    }
}

/// La riga di comando di una CLI senza il modello, quando il modello non
/// c'e': `--model {model}` sparisce intero, invece di diventare `--model ""`,
/// che a una CLI chiede un modello senza nome.
///
/// Toglie l'argomento `{model}` e, se quello prima e' un'opzione (comincia
/// con `-`), anche quella. Un `{model}` dentro un argomento piu' lungo
/// (`--nome={model}`) diventa vuoto come prima: li' non c'e' una coppia da
/// togliere.
pub fn senza_modello(args: &[String]) -> Vec<String> {
    let mut fuori: Vec<String> = Vec::new();
    for a in args {
        if a == "{model}" {
            if fuori.last().is_some_and(|p| p.starts_with('-')) {
                fuori.pop();
            }
            continue;
        }
        fuori.push(a.replace("{model}", ""));
    }
    fuori
}

#[cfg(test)]
mod prove {
    use super::*;

    fn v(x: &[&str]) -> Vec<String> {
        x.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn la_famiglia_si_legge_solo_col_prefisso() {
        assert_eq!(famiglia("ultimo:opus"), Some("opus"));
        assert_eq!(
            famiglia(" ultimo: gemini-*-pro-high "),
            Some("gemini-*-pro-high")
        );
        assert_eq!(famiglia("claude-opus-5"), None);
        assert_eq!(famiglia("ultimo:"), None);
        assert_eq!(alias_claude("ultimo:sonnet"), "sonnet");
        assert_eq!(alias_claude("claude-opus-5-5"), "claude-opus-5-5");
    }

    #[test]
    fn i_vecchi_predefiniti_si_leggono_come_la_loro_famiglia() {
        assert_eq!(
            famiglia_claude("claude-sonnet-5").as_deref(),
            Some("sonnet")
        );
        assert_eq!(famiglia_claude(" claude-opus-5 ").as_deref(), Some("opus"));
        assert_eq!(alias_claude("claude-opus-5"), "opus");
        // Gli altri nomi interi restano quelli scelti.
        assert_eq!(famiglia_claude("claude-opus-5[1m]"), None);
        assert_eq!(famiglia_claude("claude-fable-5"), None);
        assert_eq!(famiglia_claude("ultimo:fable").as_deref(), Some("fable"));
    }

    #[test]
    fn la_versione_e_fatta_solo_di_numeri() {
        assert_eq!(versione("5-5"), Some(vec![5, 5]));
        assert_eq!(versione("3.1"), Some(vec![3, 1]));
        assert_eq!(versione("5-5-high"), None);
        assert_eq!(versione(""), None);
        assert_eq!(versione("5--5"), None);
    }

    #[test]
    fn si_sceglie_il_piu_recente_con_la_forma_giusta() {
        // L'elenco di `agy models` del 6 ottobre, abbreviato.
        let e = v(&[
            "gemini-3.8-flash-high",
            "gemini-3.7-flash-high",
            "gemini-3.1-pro-high",
            "gemini-3.1-pro-low",
            "gemini-2.5-pro-high",
            "claude-opus-5-5-high",
            "gemini-3.1-pro-highest",
        ]);
        assert_eq!(
            scegli("gemini-*-pro-high", &e).as_deref(),
            Some("gemini-3.1-pro-high")
        );
        assert_eq!(
            scegli("gemini-*-flash-high", &e).as_deref(),
            Some("gemini-3.8-flash-high")
        );
        assert_eq!(
            scegli("claude-opus-*-high", &e).as_deref(),
            Some("claude-opus-5-5-high")
        );
        assert_eq!(scegli("gemini-*-ultra", &e), None);
        // Senza asterisco vale solo il nome intero.
        assert_eq!(
            scegli("gemini-3.1-pro-low", &e).as_deref(),
            Some("gemini-3.1-pro-low")
        );
        assert_eq!(scegli("gemini-9-pro-low", &e), None);
        // 3.10 viene dopo 3.9: si confrontano numeri, non testo.
        let e = v(&["gemini-3.9-pro-high", "gemini-3.10-pro-high"]);
        assert_eq!(
            scegli("gemini-*-pro-high", &e).as_deref(),
            Some("gemini-3.10-pro-high")
        );
    }

    #[test]
    fn l_elenco_tiene_solo_i_nomi() {
        let t = "Fetching available models...\ngemini-3.8-flash-high\tGemini 3.8 Flash (High)\n\
                 claude-opus-5-5-low\tClaude Opus 5.5 (Low)\ngpt-oss-120b-medium\tGPT-OSS 120B\n\n";
        assert_eq!(
            elenco(t),
            v(&[
                "gemini-3.8-flash-high",
                "claude-opus-5-5-low",
                "gpt-oss-120b-medium"
            ])
        );
    }

    #[test]
    fn la_data_e_il_contesto_lungo_non_fanno_parte_del_nome() {
        assert_eq!(senza_data("claude-haiku-4-5-20251001"), "claude-haiku-4-5");
        assert_eq!(senza_data("claude-opus-5[1m]"), "claude-opus-5");
        assert_eq!(senza_data("claude-opus-5-5"), "claude-opus-5-5");
        assert_eq!(
            versione_claude("haiku", "claude-haiku-4-5-20251001"),
            Some(vec![4, 5])
        );
        assert_eq!(versione_claude("opus", "claude-sonnet-5"), None);
    }

    #[test]
    fn i_successori_coprono_le_minori_e_la_maggiore_dopo() {
        let s = successori("sonnet", "claude-sonnet-5");
        assert_eq!(s[0], "claude-sonnet-5-1");
        assert!(s.contains(&"claude-sonnet-5-5".to_string()));
        assert!(s.contains(&"claude-sonnet-6".to_string()));
        assert_eq!(s.last().unwrap(), "claude-sonnet-6-9");
        let s = successori("opus", "claude-opus-5-5");
        assert_eq!(s[0], "claude-opus-5-6");
        assert!(!s.contains(&"claude-opus-5-5".to_string()));
        assert!(successori("opus", "claude-sonnet-5").is_empty());
    }

    #[test]
    fn le_prove_si_leggono_come_le_scrive_claude_code() {
        // Le uscite vere del 7 ottobre, accorciate.
        let va = r#"{"is_error":false,"result":"ok","modelUsage":{"claude-haiku-4-5-20251001":{},"claude-opus-5-5":{}}}
Client.listTools() called but server does not advertise tools capability"#;
        assert_eq!(leggi_prova("opus", va), Prova::Va("claude-opus-5-5".into()));
        let lungo = r#"{"is_error":false,"result":"ok","modelUsage":{"claude-opus-5[1m]":{}}}"#;
        assert_eq!(
            leggi_prova("opus", lungo),
            Prova::Va("claude-opus-5".into())
        );
        let no = r#"{"is_error":true,"api_error_status":404,"result":"There's an issue with the selected model (claude-sonnet-5-1). It may not exist or you may not have access to it."}"#;
        assert_eq!(leggi_prova("sonnet", no), Prova::NonEsiste);
        let vecchia = r#"{"is_error":true,"api_error_status":null,"result":"API Error: 400 Claude Code 2.1.237 does not support this model; version 2.1.280 or newer is required. Run 'claude update'."}"#;
        assert_eq!(
            leggi_prova("opus", vecchia),
            Prova::ServeAggiornare("2.1.280".into())
        );
        assert!(matches!(leggi_prova("opus", "boh"), Prova::Altro(_)));
        let solo_haiku = r#"{"is_error":false,"modelUsage":{"claude-haiku-4-5-20251001":{}}}"#;
        assert!(matches!(leggi_prova("opus", solo_haiku), Prova::Altro(_)));
    }

    #[test]
    fn senza_modello_sparisce_la_coppia_intera() {
        assert_eq!(
            senza_modello(&v(&["--x", "--model", "{model}", "-p"])),
            v(&["--x", "-p"])
        );
        assert_eq!(senza_modello(&v(&["{model}", "-p"])), v(&["-p"]));
        assert_eq!(senza_modello(&v(&["--nome={model}"])), v(&["--nome="]));
    }
}
