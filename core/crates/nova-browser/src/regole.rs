//! Le regole che dicono cosa, in una pagina, e' testo.
//!
//! **Generato da `_estrai_html.py`, poi mantenuto a mano.** Sono le stesse
//! espressioni regolari che gira Python — estratte, non riscritte (D112):
//! quelle di `nova/html_a_testo.py` dagli oggetti gia' compilati, quelle dei
//! due raschiatori dall'albero sintattico di `nova/tools/web.py`, perche'
//! vivono dentro le funzioni.
//!
//! Un carattere di differenza qui non da' un errore: da' il testo di un altro
//! pezzo di pagina.


/// Quello che sta dentro non e' testo della pagina: e' codice, stile,
/// o roba che il browser non mostra.
pub const INVISIBILE: &str = "(?is)<(script|style|noscript|template|svg|head)[^>]*>.*?</\\1>";

/// I tag che, chiudendosi, mandano a capo. Senza, un elenco di dieci
/// voci diventa una riga sola e il modello non vede piu' dove finisce
/// una.
pub const A_CAPO: &str = "(?i)<(br\\s*/?|/p|/div|/li|/h[1-6]|/tr|/ul|/ol)[^>]*>";

/// Tutto il resto dei tag.
pub const TAG: &str = "(?s)<[^>]+>";

/// Gli spazi che si schiacciano. `\xa0` e' lo spazio unificatore: sulle
/// pagine c'e' dappertutto, e lasciarlo vuol dire mettere nel contesto
/// del modello un carattere che sembra uno spazio e non lo e'.
pub const SPAZI: &str = "[ \\t\\r\\f\\v\\xa0]+";

/// Tre a capo o piu' diventano due.
pub const VUOTE: &str = "\\n{3,}";

/// Il titolo dichiarato dalla pagina.
pub const TITOLO: &str = "(?is)<title[^>]*>(.*?)</title>";

/// `INVISIBILE` senza il riferimento all'indietro, che il motore di Rust
/// non ha: le stesse alternative, scritte per esteso.
pub const INVISIBILE_ESPANSO: &str = "(?is)<script[^>]*>.*?</script>|<style[^>]*>.*?</style>|<noscript[^>]*>.*?</noscript>|<template[^>]*>.*?</template>|<svg[^>]*>.*?</svg>|<head[^>]*>.*?</head>";

/// I nomi dei tag che `INVISIBILE` elenca. Una prova controlla che
/// `INVISIBILE_ESPANSO` li contenga tutti e nessun altro.
pub static INVISIBILI: [&str; 6] = ["script", "style", "noscript", "template", "svg", "head"];

/// Come `html.unescape` riconosce un riferimento: per nome, per numero
/// decimale, per numero esadecimale — e il punto e virgola e' facoltativo.
/// Estratta da `html._charref`, non riscritta.
pub const CHARREF: &str = "&(#[0-9]+;?|#[xX][0-9a-fA-F]+;?|[^\\t\\n\\f <&#;]{1,32};?)";

/// Un risultato dentro la pagina «html» di DuckDuckGo: il
/// collegamento che porta il titolo. Il riassunto e' un'altra
/// espressione apposta, perche' dentro la stessa scavalcava il
/// risultato successivo e se lo portava via (D181).
pub const RISULTATO: &str = "(?is)<a[^>]+class=\"[^\"]*result__a[^\"]*\"[^>]+href=\"([^\"]+)\"[^>]*>(.*?)</a>";

/// Il riassunto, che vale solo **dentro la finestra** di un
/// risultato: fuori di li' e' di un altro.
pub const RIASSUNTO: &str = "(?is)<a[^>]+class=\"[^\"]*result__snippet[^\"]*\"[^>]*>(.*?)</a>";

/// I risultati dentro la pagina «lite», che ha un'altra forma.
pub const DDG_LITE: &str = "(?is)<a[^>]+href=\"(https?://[^\"]+)\"[^>]*class=\"result-link\"[^>]*>(.*?)</a>";

/// Il pezzo di indirizzo che dice «questo e' un rimbalzo».
pub const RIMBALZO: &str = "duckduckgo.com/l/?uddg=";
