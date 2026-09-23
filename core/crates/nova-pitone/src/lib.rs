//! Le abitudini di Python che il porto in Rust deve rispettare.
//!
//! Non sono utilita' generiche: sono i punti in cui la libreria standard di
//! Python e quella di Rust rispondono **diversamente** alla stessa domanda.
//! Ognuna di queste differenze e' costata almeno un banco rosso.
//!
//! Stanno qui, e non nel primo posto che ne ha avuto bisogno, per la
//! seconda occorrenza (D62): `nova-ricette` conta le righe della risposta di
//! un modello, `nova-browser` conta quelle del testo di una pagina. La stessa
//! domanda in due posti diventa, prima o poi, due risposte diverse (D73).

/// Un carattere che Python considera bianco.
///
/// `str.isspace()` dice `True` anche su `\x1c`-`\x1f` — i separatori di
/// file, gruppo, record e unita' — che `char::is_whitespace` di Rust non
/// considera bianchi. Chi ripulisce una riga con `trim()` invece che con
/// [`senza_bianchi`] se li ritrova dentro il testo.
pub fn e_bianco(c: char) -> bool {
    c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}')
}

/// `str.strip()`: via i bianchi da tutte e due le parti, quelli di Python.
pub fn senza_bianchi(t: &str) -> &str {
    t.trim_matches(e_bianco)
}

/// Un carattere su cui `str.splitlines()` va a capo.
pub fn e_a_capo(c: char) -> bool {
    matches!(
        c,
        '\n' | '\r' | '\u{b}' | '\u{c}' | '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{85}'
            | '\u{2028}' | '\u{2029}'
    )
}

/// Le righe come le separa Python.
///
/// `str.splitlines()` non taglia solo su `\n`: taglia anche su `\r`, `\r\n`,
/// `\v`, `\f`, i separatori `\x1c`-`\x1e`, `\x85`, e su `\u{2028}`/`\u{2029}`.
/// `str::lines()` di Rust taglia **solo** su `\n`. Non e' pedanteria: qui si
/// legge il testo che ha scritto un modello, o quello di una pagina, e ne'
/// l'uno ne' l'altra sono sotto il nostro controllo — se le righe si contano
/// diversamente, «risposta troppo corta» scatta da una parte e non
/// dall'altra.
pub fn righe(testo: &str) -> Vec<String> {
    let mut fuori: Vec<String> = Vec::new();
    let mut corrente = String::new();
    let caratteri: Vec<char> = testo.chars().collect();
    let mut i = 0;
    while i < caratteri.len() {
        let c = caratteri[i];
        if e_a_capo(c) {
            fuori.push(std::mem::take(&mut corrente));
            // `\r\n` e' un a capo solo.
            if c == '\r' && i + 1 < caratteri.len() && caratteri[i + 1] == '\n' {
                i += 1;
            }
        } else {
            corrente.push(c);
        }
        i += 1;
    }
    if !corrente.is_empty() {
        fuori.push(corrente);
    }
    fuori
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn si_va_a_capo_dove_ci_va_python() {
        assert_eq!(righe("a\nb"), vec!["a", "b"]);
        assert_eq!(righe("a\r\nb"), vec!["a", "b"]);
        // Questi `lines()` di Rust non li vede.
        assert_eq!(righe("a\u{b}b").len(), 2);
        assert_eq!(righe("a\u{2028}b").len(), 2);
        assert_eq!(righe("a\u{85}b").len(), 2);
        // L'ultima riga senza a capo c'e' lo stesso; quella dopo l'ultimo a
        // capo, se e' vuota, no.
        assert_eq!(righe("a\n"), vec!["a"]);
        assert_eq!(righe(""), Vec::<String>::new());
    }

    #[test]
    fn i_separatori_di_unita_sono_bianchi_per_python() {
        assert!(e_bianco('\u{1f}'));
        assert!(!'\u{1f}'.is_whitespace(), "se un giorno lo diventa, questo va tolto");
        assert_eq!(senza_bianchi(" \u{1c}a b\u{1f} "), "a b");
        // Lo spazio a larghezza zero non e' bianco ne' di qua ne' di la'.
        assert_eq!(senza_bianchi("\u{200b}a"), "\u{200b}a");
    }
}

// ------------------------------------------------------------- il JSON

use serde_json::Value;

/// `json.dumps(x, ensure_ascii=False)`: separatori `, ` e `: `.
///
/// `serde_json::to_string` scrive `{"a":1}`, Python `{"a": 1}`. Non cambia
/// cosa vuol dire — chi lo rilegge ottiene la stessa cosa — e cambia il
/// **testo**, che e' quello che finisce nel registro delle azioni e nel
/// prompt di chi riceve una risposta MCP. Dove le due meta' scrivono sullo
/// stesso file, o dove un banco confronta carattere per carattere, i due
/// spazi sono la differenza fra «identiche» e «si somigliano».
pub fn json_come_python(v: &Value) -> String {
    let mut fuori = String::new();
    scrivi(v, &mut fuori);
    fuori
}

/// `json.dumps(x, ensure_ascii=False, indent=n)`: una voce per riga.
///
/// Con `indent` Python cambia anche il separatore fra le voci — `,` senza
/// spazio, perche' dopo c'e' l'a capo — e lascia `{}` e `[]` su una riga
/// quando sono vuoti. E' il formato di ogni file che NOVA scrive per essere
/// riletto anche da un occhio umano (le procedure, i manifesti) e di cio' che
/// `fetch_url` mostra quando la pagina e' JSON.
pub fn json_come_python_rientrato(v: &Value, rientro: usize) -> String {
    let mut fuori = String::new();
    scrivi_rientrato(v, rientro, 0, &mut fuori);
    fuori
}

fn scrivi(v: &Value, dentro: &mut String) {
    match v {
        Value::Object(o) => {
            dentro.push('{');
            for (i, (k, val)) in o.iter().enumerate() {
                if i > 0 {
                    dentro.push_str(", ");
                }
                dentro.push_str(&Value::String(k.clone()).to_string());
                dentro.push_str(": ");
                scrivi(val, dentro);
            }
            dentro.push('}');
        }
        Value::Array(a) => {
            dentro.push('[');
            for (i, val) in a.iter().enumerate() {
                if i > 0 {
                    dentro.push_str(", ");
                }
                scrivi(val, dentro);
            }
            dentro.push(']');
        }
        altro => dentro.push_str(&foglia(altro)),
    }
}

fn scrivi_rientrato(v: &Value, rientro: usize, livello: usize, dentro: &mut String) {
    let a_capo = |dentro: &mut String, livello: usize| {
        dentro.push('\n');
        dentro.push_str(&" ".repeat(rientro * livello));
    };
    match v {
        Value::Object(o) if !o.is_empty() => {
            dentro.push('{');
            for (i, (k, val)) in o.iter().enumerate() {
                if i > 0 {
                    dentro.push(',');
                }
                a_capo(dentro, livello + 1);
                dentro.push_str(&Value::String(k.clone()).to_string());
                dentro.push_str(": ");
                scrivi_rientrato(val, rientro, livello + 1, dentro);
            }
            a_capo(dentro, livello);
            dentro.push('}');
        }
        Value::Array(a) if !a.is_empty() => {
            dentro.push('[');
            for (i, val) in a.iter().enumerate() {
                if i > 0 {
                    dentro.push(',');
                }
                a_capo(dentro, livello + 1);
                scrivi_rientrato(val, rientro, livello + 1, dentro);
            }
            a_capo(dentro, livello);
            dentro.push(']');
        }
        altro => dentro.push_str(&foglia(altro)),
    }
}

/// Un valore che non contiene altri valori.
///
/// I numeri con la virgola passano da [`float_come_python`]: `serde_json`
/// scrive `1e100` dove Python scrive `1e+100`.
fn foglia(v: &Value) -> String {
    match v {
        Value::Number(n) if n.is_f64() => {
            n.as_f64().map_or_else(|| n.to_string(), float_come_python)
        }
        altro => altro.to_string(),
    }
}

/// `repr(x)` di un `float` Python.
///
/// Le cifre sono le stesse di Rust — tutte e due scrivono le **piu' corte**
/// che, rilette, danno lo stesso numero — e cambia solo dove si mette la
/// virgola. Python passa alla forma con l'esponente sotto `1e-4` e da
/// `1e16` in su, scrive sempre il segno dell'esponente e almeno due cifre
/// (`1e-05`), e un numero intero porta sempre il suo `.0`.
///
/// `NaN` e gli infiniti si scrivono come li scrive `json.dumps`, che non e'
/// come li scrive `repr`: qui servono al JSON.
pub fn float_come_python(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    let scientifica = format!("{x:e}");
    let (segno, resto) = match scientifica.strip_prefix('-') {
        Some(r) => ("-", r),
        None => ("", scientifica.as_str()),
    };
    let (mantissa, esponente) = resto.split_once('e').unwrap_or((resto, "0"));
    let esponente: i32 = esponente.parse().unwrap_or(0);
    let cifre: String = mantissa.chars().filter(|c| *c != '.').collect();
    let quante = cifre.len() as i32;
    if (-4..16).contains(&esponente) {
        if esponente < 0 {
            let zeri = "0".repeat((-esponente - 1) as usize);
            return format!("{segno}0.{zeri}{cifre}");
        }
        let intere = (esponente + 1) as usize;
        if quante <= esponente + 1 {
            let zeri = "0".repeat(intere - cifre.len());
            return format!("{segno}{cifre}{zeri}.0");
        }
        let (prima, dopo) = cifre.split_at(intere);
        return format!("{segno}{prima}.{dopo}");
    }
    let mantissa = if cifre.len() > 1 {
        format!("{}.{}", &cifre[..1], &cifre[1..])
    } else {
        cifre
    };
    let verso = if esponente < 0 { '-' } else { '+' };
    format!("{segno}{mantissa}e{verso}{:02}", esponente.abs())
}

#[cfg(test)]
mod prove_json {
    use super::*;
    use serde_json::json;

    #[test]
    fn i_separatori_sono_quelli_di_python() {
        assert_eq!(
            json_come_python(&json!({"a": 1, "b": [1, 2]})),
            r#"{"a": 1, "b": [1, 2]}"#
        );
    }

    #[test]
    fn gli_accenti_restano_accenti() {
        // `ensure_ascii=False`: «perche'» non diventa \u00e9.
        assert_eq!(json_come_python(&json!("perché")), "\"perché\"");
    }

    #[test]
    fn e_quel_che_va_protetto_resta_protetto() {
        assert_eq!(json_come_python(&json!("a\nb")), "\"a\\nb\"");
        assert_eq!(
            json_come_python(&json!("dice \"ciao\"")),
            "\"dice \\\"ciao\\\"\""
        );
    }

    #[test]
    fn i_numeri_con_la_virgola_si_scrivono_come_repr() {
        for (x, atteso) in [
            (1.5, "1.5"),
            (3.0, "3.0"),
            (-0.0, "-0.0"),
            (0.1, "0.1"),
            (1e15, "1000000000000000.0"),
            (1e16, "1e+16"),
            (1.5e16, "1.5e+16"),
            (0.0001, "0.0001"),
            (0.00001, "1e-05"),
            (1.5e-7, "1.5e-07"),
            (1e100, "1e+100"),
            (123.456, "123.456"),
            (-2.5e-300, "-2.5e-300"),
        ] {
            assert_eq!(float_come_python(x), atteso, "{x}");
        }
    }

    #[test]
    fn rientrato_come_indent() {
        let v = json!({"a": 1, "b": [1, {"c": []}], "d": {}, "e": 2.0});
        assert_eq!(
            json_come_python_rientrato(&v, 1),
            "{\n \"a\": 1,\n \"b\": [\n  1,\n  {\n   \"c\": []\n  }\n ],\n \"d\": {},\n \"e\": 2.0\n}"
        );
        assert_eq!(json_come_python_rientrato(&json!([]), 1), "[]");
        assert_eq!(json_come_python_rientrato(&json!("x"), 2), "\"x\"");
    }
}
