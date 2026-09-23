//! Una pagina scaricata **senza browser**, e come la si racconta.
//!
//! E' il gemello di `nova/tools/web.py` per la parte che non ha bisogno di
//! un browser: `fetch_url`, i raschiatori di `web_search` e `open_in_browser`.
//! Qui non si fa nessuna richiesta — la rete la fa il demone — si decide
//! **cosa si chiede** e **cosa si dice** della risposta, che e' la parte che
//! un banco puo' confrontare col Python carattere per carattere.
//!
//! Due differenze sono volute, e sono scritte dove stanno:
//!
//! - [`decodifica`]: una pagina `text/html` senza `charset` si legge come
//!   UTF-8, non come Latin-1;
//! - [`nessun_risultato`]: se la ricerca fallisce si dice **perche'** per
//!   ciascun motore, invece di «non ho riconosciuto i risultati» anche quando
//!   il motore non ha risposto affatto.

use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

use crate::motori::{riga, Risultato};
use crate::regole;
use crate::testo::a_testo;

/// Il browser che NOVA dice di essere. Senza, DuckDuckGo risponde con una
/// pagina diversa, o con nessuna.
pub const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                      (KHTML, like Gecko) Chrome/126.0 Safari/537.36";

/// Quanto si aspetta una pagina.
pub const SECONDI: u64 = 25;

/// Le due porte di DuckDuckGo che si leggono senza JavaScript.
pub const DDG_HTML: &str = "https://html.duckduckgo.com/html/";
pub const DDG_LITE: &str = "https://lite.duckduckgo.com/lite/";

/// Dove porta `rete.apri` quando gli si chiede una ricerca e non un indirizzo.
pub const RICERCA_GOOGLE: &str = "https://www.google.com/search?q=";

/// Quanti caratteri di una pagina, se nessuno dice altro.
pub const CARATTERI_PREDEFINITI: i64 = 12_000;

/// Sotto questa soglia una pagina non si taglia: meno di cosi' non basta a
/// capire di cosa parla.
pub const CARATTERI_MINIMI: i64 = 500;

/// Quanti risultati, se nessuno dice altro, e al massimo.
pub const RISULTATI_PREDEFINITI: i64 = 6;
pub const RISULTATI_MASSIMI: i64 = 15;

/// Oltre questo si smette di leggere. Il Python non ha un limite; un demone
/// che si mette in memoria un file da un gigabyte perche' qualcuno gli ha
/// dato l'indirizzo sbagliato si', e la pagina poi si taglia comunque a
/// qualche migliaio di caratteri.
pub const BYTE_PAGINA_MASSIMI: u64 = 20 * 1024 * 1024;

/// `max(1, min(int(max_results or 6), 15))`.
///
/// Lo zero vale «quanti ne vuoi tu», come in Python, dove `0 or 6` e' 6.
pub fn quanti(chiesti: Option<i64>) -> usize {
    let n = match chiesti {
        None | Some(0) => RISULTATI_PREDEFINITI,
        Some(n) => n,
    };
    n.clamp(1, RISULTATI_MASSIMI) as usize
}

/// L'indirizzo da scaricare: chi scrive `esempio.it` intende `https://`.
pub fn con_schema(url: &str) -> String {
    let basso = url.to_lowercase();
    if basso.starts_with("http://") || basso.starts_with("https://") {
        url.to_string()
    } else {
        format!("https://{url}")
    }
}

/// Cosa apre `rete.apri`: l'indirizzo, o una ricerca Google.
///
/// Qui lo schema `file:` passa: aprire un file locale nel browser e' una
/// cosa che si chiede, e scaricarlo no — per quello c'e' `fs.read`.
pub fn da_aprire(url: &str, cerca: &str) -> Result<String, String> {
    if url.is_empty() && cerca.is_empty() {
        return Err("serve 'url' oppure 'search_query'".into());
    }
    if url.is_empty() {
        return Ok(format!("{RICERCA_GOOGLE}{}", cita(cerca)));
    }
    let basso = url.to_lowercase();
    if ["http://", "https://", "file:"]
        .iter()
        .any(|s| basso.starts_with(s))
    {
        Ok(url.to_string())
    } else {
        Ok(format!("https://{url}"))
    }
}

/// `urllib.parse.quote(testo)`: lettere, cifre, `_.-~` e `/` restano, il
/// resto diventa `%XX` byte per byte.
pub fn cita(testo: &str) -> String {
    let mut fuori = String::with_capacity(testo.len());
    for b in testo.bytes() {
        if b.is_ascii_alphanumeric() || b"_.-~/".contains(&b) {
            fuori.push(b as char);
        } else {
            fuori.push_str(&format!("%{b:02X}"));
        }
    }
    fuori
}

/// `s[:n]` di Python, contando i caratteri: un `n` negativo toglie dalla fine.
fn primi(s: &str, n: i64) -> String {
    let quanti = s.chars().count() as i64;
    let fino = if n < 0 {
        (quanti + n).max(0)
    } else {
        n.min(quanti)
    };
    s.chars().take(fino as usize).collect()
}

fn titolo() -> &'static Regex {
    static Q: OnceLock<Regex> = OnceLock::new();
    Q.get_or_init(|| Regex::new(regole::TITOLO).expect("regola scritta male"))
}

/// Cosa si dice di una pagina scaricata.
///
/// `finale` e' l'indirizzo **dopo** i rimandi: chi legge deve sapere dove e'
/// arrivato, non dove voleva andare. Se l'intestazione dice JSON e il corpo
/// lo e' davvero, si mostra il JSON rientrato; se dice JSON e non lo e', si
/// legge come una pagina qualunque — un server che mente sul tipo e' comune.
///
/// Il taglio del JSON usa `max_chars` cosi' com'e', e quello della pagina
/// no: e' cosi' anche in Python, dove un JSON chiesto con `max_chars = 0`
/// torna vuoto e una pagina con `0` torna di dodicimila caratteri.
pub fn pagina(finale: &str, tipo: &str, testo: &str, max_chars: Option<i64>) -> String {
    if tipo.contains("json") {
        let pulito = testo.strip_prefix('\u{feff}').unwrap_or(testo);
        if let Ok(v) = serde_json::from_str::<Value>(pulito) {
            let tutto = nova_pitone::json_come_python_rientrato(&v, 1);
            return primi(&tutto, max_chars.unwrap_or(CARATTERI_PREDEFINITI));
        }
    }
    let titolo = titolo()
        .captures(testo)
        .map(|c| a_testo(&c[1]))
        .unwrap_or_default();
    let mut corpo = a_testo(testo);
    let limite = match max_chars {
        None | Some(0) => CARATTERI_PREDEFINITI,
        Some(n) => n,
    }
    .max(CARATTERI_MINIMI);
    if corpo.chars().count() as i64 > limite {
        corpo = primi(&corpo, limite) + "\n... [pagina troncata]";
    }
    format!("URL: {finale}\nTITOLO: {titolo}\n\n{corpo}")
}

/// I risultati di una ricerca, uno per paragrafo.
pub fn elenco(risultati: &[Risultato]) -> String {
    risultati
        .iter()
        .enumerate()
        .map(|(i, r)| riga(i + 1, &r.titolo, &r.url, &r.riassunto))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Cosa si dice quando una ricerca non ha dato niente.
///
/// **Diverso dal Python, apposta.** Di la' ogni errore di rete si ingoia e il
/// messaggio dice sempre che i lettori «non hanno riconosciuto i risultati».
/// E' la stessa bugia che il Python stesso racconta di aver gia' pagato una
/// volta: un motore che non risponde e un lettore che non capisce la pagina
/// sono due guasti diversi, da cercare in due posti diversi. Qui ogni motore
/// dice il suo.
pub fn nessun_risultato(query: &str, perche: &[String]) -> String {
    format!(
        "non ho trovato niente per «{query}». Il browser guidato non e' ancora \
         collegato al demone; senza browser: {}. Prova ad aprire la ricerca \
         con rete.apri.",
        perche.join("; ")
    )
}

/// Il testo di una risposta, dai suoi byte.
///
/// **Diverso dal Python, apposta.** `requests`, quando l'intestazione dice
/// `text/html` e non dice il `charset`, legge la pagina come Latin-1 — lo
/// vuole una vecchia regola dell'HTTP — e ogni lettera accentata di una
/// pagina UTF-8 diventa due caratteri sbagliati. Oggi quasi tutte le pagine
/// sono UTF-8: qui si legge UTF-8, e si passa a Latin-1 solo quando e'
/// l'intestazione a chiederlo.
pub fn decodifica(byte: &[u8], tipo: &str) -> String {
    let charset = tipo
        .split(';')
        .skip(1)
        .filter_map(|p| p.split_once('='))
        .find(|(k, _)| k.trim().eq_ignore_ascii_case("charset"))
        .map(|(_, v)| v.trim().trim_matches('"').to_ascii_lowercase());
    match charset.as_deref() {
        Some("iso-8859-1" | "latin-1" | "latin1" | "iso8859-1" | "l1") => {
            byte.iter().map(|&b| b as char).collect()
        }
        _ => String::from_utf8_lossy(byte).into_owned(),
    }
}

/// Il proxy da usare per questo indirizzo, come lo sceglie `requests`.
///
/// `https_proxy` per `https://`, `http_proxy` per `http://`, minuscolo prima
/// di maiuscolo, `all_proxy` se non c'e' altro — e **niente proxy** per chi
/// sta in `no_proxy`. Il cliente HTTP di casa sa leggere il proxy
/// dall'ambiente ma non sa di `no_proxy`: preso cosi' com'e', mandava
/// attraverso il proxy anche una pagina su `localhost`.
///
/// Su Windows `requests` legge anche il proxy impostato nel sistema; qui no.
pub fn proxy_per(url: &str, ambiente: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let (schema, resto) = url.split_once("://")?;
    let schema = schema.to_ascii_lowercase();
    let autorita = resto.split(['/', '?', '#']).next().unwrap_or("");
    let ospite = autorita.rsplit('@').next().unwrap_or("");
    let ospite = if let Some(r) = ospite.strip_prefix('[') {
        r.split(']').next().unwrap_or("")
    } else {
        ospite.split(':').next().unwrap_or("")
    }
    .to_ascii_lowercase();
    let leggi = |nome: &str| {
        ambiente(nome)
            .or_else(|| ambiente(&nome.to_ascii_uppercase()))
            .filter(|v| !v.trim().is_empty())
    };
    if let Some(esclusi) = leggi("no_proxy") {
        for e in esclusi.split(',') {
            let e = e.trim();
            if e == "*" {
                return None;
            }
            let e = e.split(':').next().unwrap_or("").trim_start_matches('.');
            let e = e.to_ascii_lowercase();
            if !e.is_empty() && (ospite == e || ospite.ends_with(&format!(".{e}"))) {
                return None;
            }
        }
    }
    leggi(&format!("{schema}_proxy")).or_else(|| leggi("all_proxy"))
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn quanti_come_python() {
        assert_eq!(quanti(None), 6);
        assert_eq!(quanti(Some(0)), 6);
        assert_eq!(quanti(Some(-3)), 1);
        assert_eq!(quanti(Some(40)), 15);
        assert_eq!(quanti(Some(4)), 4);
    }

    #[test]
    fn il_json_si_taglia_come_una_fetta() {
        let t = r#"{"a": "bb"}"#;
        assert_eq!(pagina("u", "application/json", t, Some(0)), "");
        assert_eq!(
            pagina("u", "application/json", t, Some(-2)),
            "{\n \"a\": \"bb\""
        );
        // Dice JSON e non lo e': si legge come pagina.
        assert!(pagina("u", "application/json", "<p>no</p>", None).starts_with("URL: u\n"));
    }

    #[test]
    fn la_pagina_si_taglia_al_minimo_cinquecento() {
        let lunga = "x".repeat(900);
        let p = pagina("u", "text/html", &lunga, Some(10));
        assert!(p.ends_with(&format!("{}\n... [pagina troncata]", "x".repeat(500))));
    }

    #[test]
    fn il_proxy_non_serve_a_chi_sta_in_casa() {
        let amb = |k: &str| match k {
            "https_proxy" => Some("http://p:1".to_string()),
            "NO_PROXY" => Some("localhost,.interno.it".to_string()),
            _ => None,
        };
        assert_eq!(
            proxy_per("https://esempio.it/x", &amb),
            Some("http://p:1".into())
        );
        assert_eq!(proxy_per("https://localhost:8080/x", &amb), None);
        assert_eq!(proxy_per("https://a.interno.it", &amb), None);
        // http:// non ha un proxy suo e non c'e' all_proxy.
        assert_eq!(proxy_per("http://esempio.it", &amb), None);
    }

    #[test]
    fn la_codifica_la_decide_l_intestazione() {
        let utf8 = "perché".as_bytes();
        assert_eq!(decodifica(utf8, "text/html"), "perché");
        assert_eq!(
            decodifica(&[0x70, 0xe9], "text/html; charset=ISO-8859-1"),
            "pé"
        );
    }
}
