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

// ------------------------------------------------------------- repr

/// Se Python considera stampabile questo carattere (`str.isprintable`).
///
/// Python lo decide sulla categoria Unicode: non stampabili sono i
/// controlli, i formati, i separatori di riga e di paragrafo, gli spazi
/// **tranne** lo spazio normale, l'uso privato e i non assegnati. La libreria
/// di Rust non porta le categorie, e qui ci sono quelle che si incontrano
/// davvero in una pagina web — lo spazio non divisibile per primo — piu'
/// l'uso privato. **I non assegnati no**: sono migliaia, cambiano a ogni
/// versione di Unicode, e in un'etichetta di una pagina non ci finiscono.
pub fn stampabile(c: char) -> bool {
    let n = c as u32;
    let non = matches!(n,
        // Cc
        0x00..=0x1f | 0x7f..=0x9f
        // Zs tranne lo spazio
        | 0xa0 | 0x1680 | 0x2000..=0x200a | 0x202f | 0x205f | 0x3000
        // Zl, Zp
        | 0x2028 | 0x2029
        // Cf
        | 0xad | 0x600..=0x605 | 0x61c | 0x6dd | 0x70f | 0x890..=0x891 | 0x8e2 | 0x180e
        | 0x200b..=0x200f | 0x202a..=0x202e | 0x2060..=0x2064 | 0x2066..=0x206f
        | 0xfeff | 0xfff9..=0xfffb | 0x110bd | 0x110cd | 0x13430..=0x1343f
        | 0x1bca0..=0x1bca3 | 0x1d173..=0x1d17a | 0xe0001 | 0xe0020..=0xe007f
        // Co
        | 0xe000..=0xf8ff | 0xf0000..=0xffffd | 0x100000..=0x10fffd
    );
    !non
}

/// `repr(s)` di Python per una stringa.
///
/// Le virgolette sono singole, **tranne** quando dentro c'e' un apice e
/// nessuna virgoletta doppia: allora sono doppie, e l'apice resta com'e'.
/// Serve dove un messaggio di Python mette un `!r` e il modello legge quel
/// testo: `aria-label="l'utente"` e `aria-label='l\'utente'` sono lo stesso
/// valore e due righe diverse.
pub fn repr_stringa(s: &str) -> String {
    let virgola = if s.contains('\'') && !s.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut fuori = String::with_capacity(s.len() + 2);
    fuori.push(virgola);
    for c in s.chars() {
        match c {
            '\\' => fuori.push_str("\\\\"),
            '\n' => fuori.push_str("\\n"),
            '\r' => fuori.push_str("\\r"),
            '\t' => fuori.push_str("\\t"),
            c if c == virgola => {
                fuori.push('\\');
                fuori.push(c);
            }
            c if stampabile(c) => fuori.push(c),
            c => {
                let n = c as u32;
                if n <= 0xff {
                    fuori.push_str(&format!("\\x{n:02x}"));
                } else if n <= 0xffff {
                    fuori.push_str(&format!("\\u{n:04x}"));
                } else {
                    fuori.push_str(&format!("\\U{n:08x}"));
                }
            }
        }
    }
    fuori.push(virgola);
    fuori
}

/// `str(x)` di Python per un valore arrivato da JSON: `None`, `True`,
/// `False`, il testo senza virgolette, i numeri come li scrive Python.
pub fn str_di(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => "None".into(),
        Some(Value::Bool(true)) => "True".into(),
        Some(Value::Bool(false)) => "False".into(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) if n.is_f64() => {
            n.as_f64().map_or_else(|| n.to_string(), float_come_python)
        }
        Some(altro) => json_come_python(altro),
    }
}

/// Se un valore e' «vero» per Python: `None`, `False`, zero e i vuoti no.
pub fn vero(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|x| x != 0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
    }
}

#[cfg(test)]
mod prove_repr {
    use super::*;

    #[test]
    fn le_virgolette_le_sceglie_il_contenuto() {
        assert_eq!(repr_stringa("ciao"), "'ciao'");
        assert_eq!(repr_stringa("l'utente"), "\"l'utente\"");
        assert_eq!(repr_stringa("l'\"a\""), "'l\\'\"a\"'");
        assert_eq!(repr_stringa("a\u{a0}b\tc"), "'a\\xa0b\\tc'");
        assert_eq!(repr_stringa("é\u{200b}"), "'é\\u200b'");
    }
}

// ------------------------------------------------------------- file

/// `Path(p).expanduser()`: la tilde in testa diventa la cartella
/// dell'utente, e nient'altro cambia.
///
/// Stava in tre posti — chi allega file a una delega, chi consegna file al
/// browser, chi legge un documento — scritta tre volte uguale. La tilde che
/// non sta in testa, o che e' seguita da un nome (`~anna`), resta com'e':
/// Python la risolverebbe verso la cartella di un altro utente, e qui non si
/// indovina.
pub fn espandi_utente(p: &str) -> String {
    let casa = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    match p.strip_prefix('~') {
        Some(resto) if resto.is_empty() || resto.starts_with(['/', '\\']) => {
            format!("{casa}{resto}")
        }
        _ => p.to_string(),
    }
}

/// `Path(p).read_text(encoding="utf-8", errors="replace")`.
///
/// In modo testo, quindi con gli a capo universali: `\r\n` e `\r` da solo
/// diventano `\n`. Chi riceve il testo di un file di Windows non deve
/// trovarsi un carattere in piu' a ogni riga.
pub fn leggi_testo(p: &std::path::Path) -> std::io::Result<String> {
    let b = std::fs::read(p)?;
    Ok(String::from_utf8_lossy(&b)
        .replace("\r\n", "\n")
        .replace('\r', "\n"))
}
