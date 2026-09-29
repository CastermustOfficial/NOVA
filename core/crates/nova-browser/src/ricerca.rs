//! Cercare col browser **senza finestra**: il gemello di `nova/cerca.py`.
//!
//! I motori di ricerca, chiesti con una richiesta semplice, rispondono con una
//! pagina anti-bot: il Python l'aveva provato, e il 29 settembre il demone ha
//! fatto cinque ricerche su DuckDuckGo tornate tutte vuote (D362). Per questo
//! `web_search` del Python, e da D363 `rete.cerca` del demone, provano prima
//! con un browser vero, su una porta e un profilo suoi, senza finestra: non
//! compare sullo schermo, non ruba il fuoco e non tocca le schede su cui NOVA
//! sta lavorando. I lettori di DuckDuckGo restano come ripiego, per chi non ha
//! ne' Edge ne' Chrome.
//!
//! Qui c'e' quello che si decide: l'indirizzo della ricerca, come si accende
//! il browser, come si leggono i risultati. La rete e i processi li fa il
//! demone, in `nova_core::caps_rete`.

use serde_json::Value;

use crate::motori::Risultato;

/// Il motore, come `MOTORE` del Python senza il segnaposto.
pub const MOTORE: &str = "https://www.bing.com/search?q=";

/// Quanti secondi si aspetta che il browser apra la porta: `ATTESA_AVVIO_S`
/// del Python.
pub const ATTESA_AVVIO_S: u64 = 25;

/// Quanti secondi si aspetta che la pagina mostri dei risultati: l'`attesa`
/// di `cerca()` nel Python.
pub const ATTESA_RISULTATI_S: u64 = 12;

/// Ogni quanti millisecondi si guarda di nuovo la pagina, come il Python.
pub const PASSO_MS: u64 = 400;

/// Quanti caratteri di riassunto chiede il copione: il `200` del Python.
pub const CARATTERI_RIASSUNTO: i64 = 200;

/// Il nome della cartella del profilo, accanto a `config.json`.
pub const PROFILO: &str = "browser-cerca";

/// L'indirizzo della ricerca, con la domanda scritta come la scrive
/// `urllib.parse.quote_plus`.
pub fn indirizzo(domanda: &str) -> String {
    format!("{MOTORE}{}", quote_plus(domanda))
}

/// `urllib.parse.quote_plus`: lo spazio diventa `+`, lettere, cifre e `_.-~`
/// restano, tutto il resto diventa `%XX` dei suoi byte UTF-8, in maiuscolo.
pub fn quote_plus(t: &str) -> String {
    let mut fuori = String::with_capacity(t.len() * 3);
    for b in t.bytes() {
        match b {
            b' ' => fuori.push('+'),
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-' | b'~' => {
                fuori.push(b as char)
            }
            _ => fuori.push_str(&format!("%{b:02X}")),
        }
    }
    fuori
}

/// Gli argomenti con cui si accende il browser delle ricerche, nell'ordine
/// del Python.
///
/// `--headless=new` e' il punto di tutto: senza, una ricerca aprirebbe una
/// finestra sullo schermo di chi lavora.
pub fn argomenti(porta: u16, profilo: &str, origine: &str) -> Vec<String> {
    vec![
        format!("--remote-debugging-port={porta}"),
        format!("--user-data-dir={profilo}"),
        format!("--remote-allow-origins={origine}"),
        "--headless=new".into(),
        "--no-first-run".into(),
        "--no-default-browser-check".into(),
        "about:blank".into(),
    ]
}

/// Quanti risultati chiedere al copione: `max(1, min(quanti, 25))`.
pub fn da_chiedere(quanti: usize) -> i64 {
    quanti.clamp(1, 25) as i64
}

/// I risultati, da quello che il copione ha restituito.
///
/// Il copione torna `{quanti, risultati: [{titolo, url, testo}]}`, gia'
/// tagliati alla lunghezza del Python. Finche' `quanti` e' zero la pagina non
/// e' ancora pronta, o non ha risultati: si torna un elenco vuoto, e chi
/// chiama decide se aspettare ancora.
pub fn letti(valore: &Value, quanti: usize) -> Vec<Risultato> {
    if valore.get("quanti").and_then(Value::as_u64).unwrap_or(0) == 0 {
        return Vec::new();
    }
    let campo = |r: &Value, k: &str| r.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    valore
        .get("risultati")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .take(quanti)
                .map(|r| Risultato {
                    titolo: campo(r, "titolo"),
                    url: campo(r, "url"),
                    riassunto: campo(r, "testo"),
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod prove {
    use super::*;
    use serde_json::json;

    #[test]
    fn la_domanda_si_scrive_come_quote_plus() {
        assert_eq!(quote_plus("gatti neri"), "gatti+neri");
        assert_eq!(quote_plus("perché?"), "perch%C3%A9%3F");
        assert_eq!(quote_plus("a+b&c=d/e"), "a%2Bb%26c%3Dd%2Fe");
        assert_eq!(quote_plus("_.-~"), "_.-~");
        assert_eq!(
            indirizzo("llama.cpp flash attention"),
            "https://www.bing.com/search?q=llama.cpp+flash+attention"
        );
    }

    #[test]
    fn il_browser_delle_ricerche_parte_senza_finestra() {
        let a = argomenti(9223, "/casa/NOVA/browser-cerca", "http://127.0.0.1");
        assert_eq!(a[0], "--remote-debugging-port=9223");
        assert_eq!(a[1], "--user-data-dir=/casa/NOVA/browser-cerca");
        assert!(a.contains(&"--headless=new".to_string()));
        assert_eq!(a.last().map(String::as_str), Some("about:blank"));
    }

    #[test]
    fn si_chiedono_da_uno_a_venticinque_risultati() {
        assert_eq!(da_chiedere(0), 1);
        assert_eq!(da_chiedere(6), 6);
        assert_eq!(da_chiedere(99), 25);
    }

    #[test]
    fn i_risultati_si_leggono_solo_quando_ci_sono() {
        assert!(letti(&json!({"quanti": 0, "risultati": []}), 6).is_empty());
        assert!(letti(&Value::Null, 6).is_empty());
        let v = json!({"quanti": 3, "risultati": [
            {"titolo": "Uno", "url": "https://uno.it", "testo": "primo"},
            {"titolo": "Due", "url": "https://due.it", "testo": ""},
            {"titolo": "Tre", "url": "https://tre.it", "testo": "terzo"},
        ]});
        let r = letti(&v, 2);
        assert_eq!(r.len(), 2);
        assert_eq!(
            r[0],
            Risultato {
                titolo: "Uno".into(),
                url: "https://uno.it".into(),
                riassunto: "primo".into()
            }
        );
        assert_eq!(r[1].riassunto, "");
    }
}
