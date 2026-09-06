//! # nova-browser
//!
//! Il browser guidato **dal di dentro** invece che da fuori.
//!
//! Finora NOVA leggeva le pagine web dall'albero di accessibilita' di Windows,
//! cioe' quello che il browser espone ai lettori di schermo. Su
//! un'applicazione nativa e' la strada giusta; su una pagina web e' la strada
//! sbagliata, e si vedeva: ventiquattro turni e il menu File di Google Docs
//! ancora non era aperto. Chi apre «Ispeziona» ci arriva in tre secondi,
//! perche' `document.querySelector("#docs-file-menu")` sostituisce venti
//! chiamate.
//!
//! ## Cosa sta qui e cosa sta fuori
//!
//! Qui: **il testo esatto che il browser eseguira'**. Il JavaScript sta in
//! [`copioni`], estratto dal Python e non ricopiato; questo modulo lo compone
//! mettendoci dentro gli argomenti, costruisce il messaggio del DevTools
//! Protocol, e legge cosa e' andato storto.
//!
//! Fuori: la connessione WebSocket, l'avvio del browser, il profilo. Sono
//! processi e rete, e non e' quello che qui si sta decidendo.
//!
//! ## Il confine che conta
//!
//! Un selettore e un testo da scrivere sono **dati dell'utente** che finiscono
//! dentro **codice che il browser esegue** su una pagina dove l'utente e' gia'
//! autenticato. Non c'e' un posto in NOVA dove il confine fra argomento e
//! codice conti di piu'. Si passa da [`dentro`], che scrive un valore come lo
//! scriverebbe `json.dumps` di Python — virgolette, barre rovesce, caratteri
//! di controllo e **tutto cio' che non e' ASCII** — e non da una
//! concatenazione.

pub mod copioni;

use serde_json::{json, Value};

/// La porta su cui NOVA parla col proprio browser.
pub const PORTA: u16 = 9333;

/// Un valore dentro il JavaScript, scritto come lo scrive `json.dumps`.
///
/// **Ci si passa sempre**, anche per un selettore che «sicuramente non ha
/// virgolette»: e' il confine fra un argomento e del codice, e un confine che
/// vale solo per gli argomenti prevedibili non e' un confine.
///
/// Come Python, e non come `serde_json`: `json.dumps` senza
/// `ensure_ascii=False` scrive **`\u00e9`**, non `é`. Le due forme sono
/// tutte e due JavaScript valido e fanno la stessa cosa — ma il banco
/// confronta il testo, e un banco che accetta due scritture diverse smette di
/// accorgersi di tutto il resto.
pub fn dentro(valore: &str) -> String {
    let mut fuori = String::with_capacity(valore.len() + 2);
    fuori.push('"');
    for c in valore.chars() {
        match c {
            '"' => fuori.push_str("\\\""),
            '\\' => fuori.push_str("\\\\"),
            '\n' => fuori.push_str("\\n"),
            '\r' => fuori.push_str("\\r"),
            '\t' => fuori.push_str("\\t"),
            '\u{8}' => fuori.push_str("\\b"),
            '\u{c}' => fuori.push_str("\\f"),
            c if (c as u32) < 0x20 => fuori.push_str(&format!("\\u{:04x}", c as u32)),
            c if (c as u32) < 0x7f => fuori.push(c),
            c => {
                // Fuori dall'ASCII Python scrive sempre `\uXXXX`, e per i
                // caratteri oltre il piano base scrive **la coppia
                // surrogata** — che e' come JavaScript li tiene in memoria.
                let mut buf = [0u16; 2];
                for u in c.encode_utf16(&mut buf) {
                    fuori.push_str(&format!("\\u{u:04x}"));
                }
            }
        }
    }
    fuori.push('"');
    fuori
}

/// Un intero dentro il JavaScript. C'e' per non lasciare a chi chiama la
/// tentazione di scriverlo a mano accanto a un valore che invece va protetto.
pub fn numero(n: i64) -> String {
    n.to_string()
}

fn riempi(copione: &str, pezzi: &[String]) -> String {
    let mut fuori = String::new();
    let mut i = 0usize;
    let mut resto = copione;
    while let Some(p) = trova_segnaposto(resto) {
        fuori.push_str(&resto[..p.0]);
        fuori.push_str(pezzi.get(i).map(String::as_str).unwrap_or(""));
        resto = &resto[p.1..];
        i += 1;
    }
    fuori.push_str(resto);
    fuori
}

/// Il prossimo `%s` o `%d`: dove comincia e dove finisce.
fn trova_segnaposto(t: &str) -> Option<(usize, usize)> {
    let b = t.as_bytes();
    for i in 0..b.len().saturating_sub(1) {
        if b[i] == b'%' && (b[i + 1] == b's' || b[i + 1] == b'd') {
            return Some((i, i + 2));
        }
    }
    None
}

// --------------------------------------------------------------- i copioni

/// Gli elementi che corrispondono a un selettore CSS.
pub fn trova(selettore: &str, quanti: i64) -> String {
    riempi(copioni::TROVA, &[dentro(selettore), numero(quanti)])
}

/// Gli elementi che contengono un testo visibile.
pub fn per_testo(testo: &str, selettore: &str, quanti: i64, esatto: bool) -> String {
    riempi(
        copioni::PER_TESTO,
        &[
            dentro(testo),
            dentro(selettore),
            numero(quanti),
            // `esatto` non passa da `dentro`: e' un letterale booleano di
            // JavaScript, non una stringa. Scriverlo fra virgolette darebbe
            // `"false"`, che in JavaScript e' **vero**.
            if esatto { "true".into() } else { "false".into() },
        ],
    )
}

/// Preme su un elemento scelto per selettore.
pub fn clicca(selettore: &str) -> String {
    riempi(copioni::CLICCA, &[dentro(selettore)])
}

/// Preme sul primo elemento visibile che contiene quel testo.
pub fn clicca_testo(testo: &str, selettore: &str) -> String {
    riempi(copioni::CLICCA_TESTO, &[dentro(testo), dentro(selettore)])
}

/// Scrive in un campo.
pub fn scrivi(selettore: &str, testo: &str) -> String {
    riempi(copioni::SCRIVI, &[dentro(selettore), dentro(testo)])
}

/// Incolla un blocco intero.
pub fn incolla(testo: &str, selettore: &str) -> String {
    riempi(copioni::INCOLLA, &[dentro(testo), dentro(selettore)])
}

/// Una tabella come TSV.
pub fn tabella(selettore: &str, righe: i64, caratteri_cella: i64) -> String {
    riempi(
        copioni::TABELLA,
        &[dentro(selettore), numero(righe), numero(caratteri_cella)],
    )
}

/// Il testo della pagina.
pub fn leggi(caratteri: i64) -> String {
    riempi(copioni::LEGGI, &[numero(caratteri), numero(caratteri)])
}

// ------------------------------------------------------------------- CDP

/// I parametri di `Runtime.evaluate`.
///
/// `awaitPromise` c'e' perche' meta' delle cose utili in una pagina moderna
/// sono asincrone, e senza si otterrebbe un «Promise» invece del valore.
/// `userGesture` fa passare i click per gesti dell'utente — senza, meta' dei
/// menu non si apre — e permette `$$` come in console.
pub fn valuta_params(codice: &str) -> Value {
    json!({
        "expression": codice,
        "returnByValue": true,
        "awaitPromise": true,
        "userGesture": true,
    })
}

/// Cosa e' andato storto nella pagina, se e' andato storto qualcosa.
///
/// Il messaggio si taglia a quattrocento caratteri: uno stack trace di
/// JavaScript minificato e' lungo migliaia di caratteri e non dice niente in
/// piu' del suo inizio.
pub fn errore_di_pagina(risposta: &Value) -> Option<String> {
    let d = risposta.get("exceptionDetails")?;
    // Il Python scrive `if r.get("exceptionDetails"):`, e in Python un
    // oggetto **vuoto e' falso**: un `exceptionDetails: {}` non e' un errore,
    // e' un campo che non dice niente. Qui la stessa cosa va scritta a mano,
    // perche' in Rust «c'e' la chiave» e «la chiave dice qualcosa» sono due
    // domande diverse — ed e' esattamente il genere di differenza che fa
    // fallire un turno che invece era riuscito.
    let vuoto = match d {
        Value::Null => true,
        Value::Object(o) => o.is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::String(s) => s.is_empty(),
        Value::Bool(b) => !*b,
        Value::Number(n) => n.as_f64().map(|x| x == 0.0).unwrap_or(false),
    };
    if vuoto {
        return None;
    }
    let msg = d
        .get("exception")
        .and_then(|e| e.get("description"))
        .and_then(Value::as_str)
        .or_else(|| d.get("text").and_then(Value::as_str))
        .unwrap_or("errore nella pagina");
    Some(msg.chars().take(400).collect())
}

/// Il valore riportato da `Runtime.evaluate`.
pub fn valore_di(risposta: &Value) -> Option<&Value> {
    risposta.get("result").and_then(|r| r.get("value"))
}

// ---------------------------------------------------------------- schede

/// Una scheda del browser, ridotta a cio' che serve per sceglierla.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scheda {
    pub id: String,
    pub url: String,
    pub titolo: String,
}

/// Perche' non si e' potuto scegliere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NessunaScheda {
    NonCeNeSono,
    NessunaCosi { quale: String, quante: usize },
}

/// La scheda su cui lavorare.
///
/// **Per identificativo prima**, e non e' un ordine arbitrario: senza, si
/// apriva una scheda e se ne leggeva un'altra — e il risultato non era un
/// errore, era il **contenuto sbagliato**, che e' molto peggio. Un profilo
/// nuovo di Edge ha gia' due o tre schede sue (benvenuto, estensioni,
/// ricerca), e la prima della lista non e' quasi mai la tua.
pub fn scheda<'a>(elenco: &'a [Scheda], quale: &str) -> Result<&'a Scheda, NessunaScheda> {
    if elenco.is_empty() {
        return Err(NessunaScheda::NonCeNeSono);
    }
    if quale.is_empty() {
        return Ok(&elenco[0]);
    }
    if let Some(t) = elenco.iter().find(|t| t.id == quale) {
        return Ok(t);
    }
    let cercato = quale.to_lowercase();
    if let Some(t) = elenco
        .iter()
        .find(|t| format!("{}{}", t.url, t.titolo).to_lowercase().contains(&cercato))
    {
        return Ok(t);
    }
    Err(NessunaScheda::NessunaCosi {
        quale: quale.to_string(),
        quante: elenco.len(),
    })
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn un_argomento_non_puo_diventare_codice() {
        // Il caso che conta: un selettore che chiude la stringa e apre del
        // codice. Se `dentro` non facesse il suo lavoro, questo eseguirebbe
        // `alert(1)` nella pagina dell'utente.
        let cattivo = r#"a"); alert(1); ("#;
        let js = clicca(cattivo);
        // La domanda giusta non e' «c'e' dentro `alert(1)`» — c'e', ed e'
        // corretto che ci sia: e' il testo che l'utente ha scritto. La
        // domanda e' se la stringa si **chiude** li'. Senza escape il
        // JavaScript direbbe `querySelector("a"); alert(1); ("")`; con
        // l'escape la virgoletta e' preceduta da una barra e resta dentro.
        assert!(!js.contains("\"a\");"), "la stringa si e' chiusa: {js}");
        assert!(js.contains(r#""a\"); alert(1); (""#), "{js}");
    }

    #[test]
    fn si_scrive_come_lo_scrive_python() {
        assert_eq!(dentro("ciao"), "\"ciao\"");
        assert_eq!(dentro("con \"virgolette\""), "\"con \\\"virgolette\\\"\"");
        assert_eq!(dentro("barra\\rovescia"), "\"barra\\\\rovescia\"");
        assert_eq!(dentro("a\ncapo\te tab"), "\"a\\ncapo\\te tab\"");
        // Fuori dall'ASCII, `\uXXXX`: e' quello che fa `json.dumps` di
        // fabbrica, e il banco confronta il testo.
        assert_eq!(dentro("perché"), "\"perch\\u00e9\"");
        // Oltre il piano base, la coppia surrogata.
        assert_eq!(dentro("\u{1F9EA}"), "\"\\ud83e\\uddea\"");
        assert_eq!(dentro("\u{1}"), "\"\\u0001\"");
    }

    #[test]
    fn il_booleano_non_va_fra_virgolette() {
        // `"false"` in JavaScript e' **vero**: fra virgolette, «non esatto»
        // diventerebbe «esatto».
        assert!(per_testo("x", "", 5, false).contains("esatto = false"));
        assert!(per_testo("x", "", 5, true).contains("esatto = true"));
    }

    #[test]
    fn i_segnaposto_vengono_riempiti_tutti() {
        for js in [
            trova("#a", 20),
            per_testo("t", "#a", 5, true),
            clicca("#a"),
            clicca_testo("t", ""),
            scrivi("#a", "x"),
            incolla("x", "#a"),
            tabella("", 400, 120),
            leggi(6000),
        ] {
            assert!(!js.contains("%s"), "{js}");
            assert!(!js.contains("%d"), "{js}");
        }
    }

    #[test]
    fn gli_argomenti_finiscono_nellordine_giusto() {
        let js = scrivi("#campo", "il valore");
        let i = js.find("\"#campo\"").unwrap();
        let j = js.find("\"il valore\"").unwrap();
        assert!(i < j, "selettore prima, testo dopo");
    }

    #[test]
    fn la_scheda_si_sceglie_per_identificativo_prima() {
        let e = vec![
            Scheda { id: "A".into(), url: "https://benvenuto".into(), titolo: "Benvenuto".into() },
            Scheda { id: "B".into(), url: "https://esempio.it".into(), titolo: "Esempio".into() },
        ];
        // Vuoto: la prima.
        assert_eq!(scheda(&e, "").unwrap().id, "A");
        // Per identificativo, anche quando il testo direbbe un'altra.
        assert_eq!(scheda(&e, "B").unwrap().id, "B");
        // Per pezzo di indirizzo o titolo.
        assert_eq!(scheda(&e, "esempio").unwrap().id, "B");
        assert_eq!(scheda(&e, "BENVENUTO").unwrap().id, "A");
        assert_eq!(
            scheda(&e, "mai visto"),
            Err(NessunaScheda::NessunaCosi { quale: "mai visto".into(), quante: 2 })
        );
        assert_eq!(scheda(&[], ""), Err(NessunaScheda::NonCeNeSono));
    }

    #[test]
    fn lerrore_della_pagina_si_legge_e_si_taglia() {
        let r = json!({"exceptionDetails": {
            "exception": {"description": "x".repeat(1000)}}});
        assert_eq!(errore_di_pagina(&r).unwrap().chars().count(), 400);
        let senza_descrizione = json!({"exceptionDetails": {"text": "Uncaught"}});
        assert_eq!(errore_di_pagina(&senza_descrizione).unwrap(), "Uncaught");
        // Un `exceptionDetails` vuoto **non** e' un errore: in Python un
        // oggetto vuoto e' falso, e quel ramo non scatta.
        assert!(errore_di_pagina(&json!({"exceptionDetails": {}})).is_none());
        let senza_niente = json!({"exceptionDetails": {"exception": {}}});
        assert_eq!(errore_di_pagina(&senza_niente).unwrap(), "errore nella pagina");
        assert!(errore_di_pagina(&json!({"result": {"value": 1}})).is_none());
    }

    #[test]
    fn i_parametri_di_evaluate_hanno_le_quattro_chiavi() {
        let p = valuta_params("1+1");
        assert_eq!(p["expression"], "1+1");
        assert_eq!(p["returnByValue"], true);
        assert_eq!(p["awaitPromise"], true);
        assert_eq!(p["userGesture"], true);
    }
}
