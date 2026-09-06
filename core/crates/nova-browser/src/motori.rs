//! Leggere i risultati di un motore di ricerca **senza browser**.
//!
//! E' la strada di ripiego: quella buona apre una pagina vera, perche' i
//! motori interrogati con una richiesta secca rispondono con una pagina
//! anti-bot. Ma quando il browser non c'e' o non parte, questo e' cio' che
//! resta, ed e' meglio di niente.
//!
//! Qui non si fa nessuna richiesta: si legge l'HTML che qualcun altro ha
//! scaricato. Raschiare l'HTML di un motore e' fragile per natura — cambiano
//! le classi e smette di funzionare — e proprio per questo va scritto in modo
//! che quando smette **si veda**: zero risultati, non risultati sbagliati.
//! E' gia' successo una volta, ed e' il motivo per cui `web_search` prova
//! prima col browser: i raschiatori trovavano zero risultati e NOVA diceva
//! «motore non raggiungibile», che era falso.
//!
//! **Il rimbalzo.** DuckDuckGo non da' l'indirizzo del sito: da' un proprio
//! indirizzo che ci rimanda, con quello vero dentro un parametro. Se non lo
//! si sbroglia, NOVA riporta l'indirizzo del motore e chi legge non sa dove
//! sta andando — e chi ci clicca passa da un terzo senza saperlo.

use std::sync::OnceLock;

use regex::Regex;

use crate::regole;
use crate::testo::{a_testo, scioglie};

/// Un risultato di ricerca.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Risultato {
    pub titolo: String,
    pub url: String,
    pub riassunto: String,
}

/// Quanto si tiene di un titolo e di un riassunto.
pub const MAX_TITOLO: usize = 200;
pub const MAX_RIASSUNTO: usize = 400;

pub use crate::regole::RIMBALZO;

fn primi(t: &str, quanti: usize) -> String {
    t.chars().take(quanti).collect()
}

fn risultato() -> &'static Regex {
    static Q: OnceLock<Regex> = OnceLock::new();
    Q.get_or_init(|| Regex::new(regole::RISULTATO).expect("regola scritta male"))
}

fn riassunto() -> &'static Regex {
    static Q: OnceLock<Regex> = OnceLock::new();
    Q.get_or_init(|| Regex::new(regole::RIASSUNTO).expect("regola scritta male"))
}

fn ddg_lite() -> &'static Regex {
    static Q: OnceLock<Regex> = OnceLock::new();
    Q.get_or_init(|| Regex::new(regole::DDG_LITE).expect("regola scritta male"))
}

/// `urllib.parse.unquote`: `%XX` torna il byte che era.
///
/// Si decodifica in byte e poi si legge come UTF-8, non carattere per
/// carattere: una lettera accentata dentro un indirizzo e' **due** `%XX`, e
/// scioglierli uno per volta darebbe due caratteri sbagliati invece di uno
/// giusto. Cio' che non e' UTF-8 valido diventa il carattere di sostituzione,
/// come fa Python con `errors="replace"`.
///
/// I pezzi non ASCII passano intatti: Python spezza la stringa nei suoi tratti
/// ASCII e scioglie solo quelli, e la differenza si vede su un indirizzo che
/// contiene gia' una lettera accentata scritta per esteso.
pub fn per_cento(t: &str) -> String {
    let mut fuori = String::with_capacity(t.len());
    let mut tratto = String::new();
    let mut ascii = true;
    for c in t.chars() {
        if c.is_ascii() != ascii {
            if ascii {
                fuori.push_str(&un_tratto(&tratto));
            } else {
                fuori.push_str(&tratto);
            }
            tratto.clear();
            ascii = c.is_ascii();
        }
        tratto.push(c);
    }
    if ascii {
        fuori.push_str(&un_tratto(&tratto));
    } else {
        fuori.push_str(&tratto);
    }
    fuori
}

fn un_tratto(t: &str) -> String {
    let b = t.as_bytes();
    let mut byte: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(x) = u8::from_str_radix(&t[i + 1..i + 3], 16) {
                byte.push(x);
                i += 3;
                continue;
            }
        }
        byte.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&byte).into_owned()
}

/// `urlsplit(...).query`: cio' che sta fra il primo `?` e il primo `#`.
///
/// Il frammento si taglia **prima** della domanda, come fa Python: un `?`
/// dentro il frammento non apre una domanda.
pub fn domanda_di(url: &str) -> &str {
    let senza_frammento = url.split('#').next().unwrap_or("");
    match senza_frammento.split_once('?') {
        Some((_, q)) => q,
        None => "",
    }
}

/// `parse_qs`: i valori di un parametro, come li legge Python.
///
/// I valori vuoti si saltano (`keep_blank_values` e' falso), il `+` diventa
/// uno spazio, e il resto si scioglie dai `%XX`.
pub fn valori_di(domanda: &str, nome: &str) -> Vec<String> {
    let mut fuori = Vec::new();
    for pezzo in domanda.split('&') {
        if pezzo.is_empty() {
            continue;
        }
        let Some((n, v)) = pezzo.split_once('=') else {
            continue;
        };
        if v.is_empty() {
            continue;
        }
        if per_cento(&n.replace('+', " ")) == nome {
            fuori.push(per_cento(&v.replace('+', " ")));
        }
    }
    fuori
}

/// L'indirizzo vero dietro un rimbalzo di DuckDuckGo.
///
/// Torna l'indirizzo com'era se non e' un rimbalzo, o se il parametro non
/// c'e': chi chiama non deve doversi chiedere quale dei casi ha in mano.
///
/// Il valore si scioglie **due volte** — una dentro `parse_qs`, una dopo —
/// perche' cosi' fa Python. Non e' un caso di studio: un indirizzo che
/// contiene un `%2520` esce diverso a seconda di quante volte lo si scioglie.
pub fn senza_rimbalzo(url: &str) -> String {
    if !url.contains(RIMBALZO) {
        return url.to_string();
    }
    match valori_di(domanda_di(url), "uddg").first() {
        Some(v) => per_cento(v),
        None => url.to_string(),
    }
}

/// I risultati dentro la pagina «html» di DuckDuckGo.
///
/// Il riassunto e' **facoltativo**: un risultato senza riassunto e' un
/// risultato, e scartarlo vorrebbe dire perdere proprio quelli che il motore
/// non ha saputo riassumere.
///
/// E si cerca **nella finestra** che va da questo risultato al prossimo. La
/// versione con tutto in un'espressione sola c'e' stata, e faceva una cosa
/// che nessuno avrebbe notato leggendola: un risultato senza riassunto si
/// prendeva quello del risultato dopo, **e si portava via anche quel
/// risultato**. Un elenco piu' corto, e un riassunto attaccato all'indirizzo
/// sbagliato (D181).
pub fn da_html(pagina: &str, quanti: usize) -> Vec<Risultato> {
    let trovati: Vec<_> = risultato().captures_iter(pagina).collect();
    let mut fuori = Vec::new();
    for (k, m) in trovati.iter().enumerate() {
        let dopo = m.get(0).map_or(0, |x| x.end());
        let fine = trovati
            .get(k + 1)
            .and_then(|n| n.get(0))
            .map_or(pagina.len(), |x| x.start());
        let finestra = if dopo <= fine { &pagina[dopo..fine] } else { "" };
        let riass = riassunto()
            .captures(finestra)
            .and_then(|c| c.get(1).map(|x| x.as_str().to_string()))
            .unwrap_or_default();
        let url = scioglie(m.get(1).map_or("", |x| x.as_str()));
        fuori.push(Risultato {
            titolo: primi(&a_testo(m.get(2).map_or("", |x| x.as_str())), MAX_TITOLO),
            url: senza_rimbalzo(&url),
            riassunto: primi(&a_testo(&riass), MAX_RIASSUNTO),
        });
        if fuori.len() >= quanti {
            break;
        }
    }
    fuori
}

/// I risultati dentro la pagina «lite», che ha un'altra forma.
pub fn da_lite(pagina: &str, quanti: usize) -> Vec<Risultato> {
    ddg_lite()
        .captures_iter(pagina)
        .take(quanti)
        .map(|m| Risultato {
            titolo: primi(&a_testo(m.get(2).map_or("", |x| x.as_str())), MAX_TITOLO),
            url: scioglie(m.get(1).map_or("", |x| x.as_str())),
            riassunto: String::new(),
        })
        .collect()
}

/// Come si racconta un risultato a chi legge.
pub fn riga(i: usize, titolo: &str, url: &str, riassunto: &str) -> String {
    let mut fuori = format!("{i}. {titolo}\n   {url}");
    if !riassunto.is_empty() {
        fuori.push_str(&format!("\n   {riassunto}"));
    }
    fuori
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn il_rimbalzo_si_sbroglia() {
        let u = "//duckduckgo.com/l/?uddg=https%3A%2F%2Fesempio.it%2Fpagina&rut=x";
        assert_eq!(senza_rimbalzo(u), "https://esempio.it/pagina");
        // Un indirizzo normale torna com'era: chi chiama non deve chiedersi
        // quale dei due casi ha in mano.
        assert_eq!(senza_rimbalzo("https://esempio.it"), "https://esempio.it");
        // Il pezzo c'e' ma il valore e' vuoto: si tiene quello che c'era,
        // invece di restituire una stringa vuota che nessuno saprebbe leggere.
        assert_eq!(senza_rimbalzo("//duckduckgo.com/l/?uddg="),
                   "//duckduckgo.com/l/?uddg=");
    }

    #[test]
    fn una_lettera_accentata_nellindirizzo_resta_una() {
        // `%C3%A9` sono due byte di **una** lettera: scioglierli uno per
        // volta darebbe due caratteri sbagliati.
        assert_eq!(per_cento("perch%C3%A9"), "perché");
        assert_eq!(per_cento("niente da fare"), "niente da fare");
        assert_eq!(per_cento("%ZZ"), "%ZZ");
        assert_eq!(per_cento("a%"), "a%");
    }

    #[test]
    fn si_scioglie_due_volte_perche_cosi_fa_python() {
        // `%2520` e' `%20` scritto una volta di piu': una sola sciolta
        // darebbe `%20`, due danno lo spazio. Non e' un dettaglio inventato,
        // e' cio' che succede passando da `parse_qs` e poi da `unquote`.
        let u = "//duckduckgo.com/l/?uddg=https%3A%2F%2Fx.it%2Fa%2520b";
        assert_eq!(senza_rimbalzo(u), "https://x.it/a b");
    }

    #[test]
    fn i_risultati_si_leggono_dalla_pagina_html() {
        // `r##"..."##`: dentro c'e' un `href="#"`, e con un cancelletto solo
        // la stringa si chiuderebbe li'.
        let p = r##"
        <div><a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Funo.it">
        Primo <b>sito</b></a>
        <a class="result__snippet" href="#">Il riassunto del primo</a></div>
        <div><a class="result__a" href="https://due.it">Secondo</a></div>
        "##;
        let r = da_html(p, 10);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].titolo, "Primo sito");
        assert_eq!(r[0].url, "https://uno.it");
        assert_eq!(r[0].riassunto, "Il riassunto del primo");
        assert_eq!(r[1].titolo, "Secondo");
        assert_eq!(r[1].riassunto, "");
    }

    #[test]
    fn un_riassunto_non_si_prende_da_un_altro_risultato() {
        // Il caso che la pagina vera contiene sempre: uno senza riassunto,
        // e subito dopo uno che ce l'ha. Prima il primo se lo prendeva, e si
        // portava via anche il secondo (D181).
        let p = concat!(
            r#"<a class="result__a" href="https://uno.it">Uno</a>"#,
            r#"<a class="result__a" href="https://due.it">Due</a>"#,
            r#"<a class="result__snippet">di Due</a>"#,
        );
        let r = da_html(p, 10);
        assert_eq!(r.len(), 2, "nessuno dei due si perde");
        assert_eq!(r[0].riassunto, "");
        assert_eq!(r[1].riassunto, "di Due");
    }

    #[test]
    fn quando_la_pagina_cambia_forma_non_si_inventa_niente() {
        // La domanda che conta su un raschiatore: se smette di funzionare,
        // si vede? Si': zero risultati, non risultati sbagliati.
        assert!(da_html("<html><body>niente di che</body></html>", 10).is_empty());
        assert!(da_lite("", 10).is_empty());
    }

    #[test]
    fn il_tetto_si_rispetta() {
        let uno = r#"<a class="result__a" href="https://x.it">X</a>"#;
        let p = uno.repeat(20);
        assert_eq!(da_html(&p, 6).len(), 6);
        assert_eq!(da_html(&p, 0).len(), 1, "zero non e' un tetto: e' il primo giro");
    }

    #[test]
    fn la_pagina_lite_vuole_indirizzi_veri() {
        let p = r#"<a href="https://uno.it" class="result-link">Uno</a>
                   <a href="/interno" class="result-link">Interno</a>"#;
        let r = da_lite(p, 10);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].url, "https://uno.it");
    }

    #[test]
    fn la_riga_salta_il_riassunto_quando_non_ce() {
        assert_eq!(riga(1, "T", "u", "r"), "1. T\n   u\n   r");
        assert_eq!(riga(2, "T", "u", ""), "2. T\n   u");
    }
}
