//! Cosa si dice al modello dopo aver guidato il browser.
//!
//! Gemello dei metodi `web_*` di `ServerKB` in `nova/mcp_kb.py`: il testo
//! che torna a Claude Code — e adesso anche al cervello del demone — dopo
//! aver aperto, cercato, premuto, scritto, incollato, caricato. Qui non si
//! tocca il browser: entra quello che il copione ha riportato, esce la frase,
//! e se va scritta una riga nel registro delle azioni, quale.
//!
//! I valori arrivano come `Value` e si leggono **come li legge Python**:
//! `x.get("id")` vero o falso, `str(None)` che fa «None», un `!r` che sceglie
//! le virgolette dal contenuto. Il modello legge queste righe e ci sceglie la
//! mossa dopo, e sono il testo che il banco confronta.

use nova_pitone::{repr_stringa, str_di, vero};
use serde_json::Value;

/// Una riga da scrivere nel registro delle azioni.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotazione {
    pub azione: String,
    pub dettagli: String,
    /// «browser», salvo le credenziali.
    pub tipo: String,
}

fn nota(azione: String, dettagli: String) -> Annotazione {
    Annotazione {
        azione,
        dettagli,
        tipo: "browser".into(),
    }
}

/// `d.get(k, predefinito)` e poi `str()`: il predefinito vale solo se la
/// chiave **manca**, non se c'e' ed e' vuota.
fn con_predefinito(d: &Value, k: &str, predefinito: &str) -> String {
    match d.get(k) {
        None => predefinito.to_string(),
        v => str_di(v),
    }
}

/// `d.get(k) or altro`.
fn o_altrimenti(d: &Value, k: &str, altro: &str) -> String {
    let v = d.get(k);
    if vero(v) {
        str_di(v)
    } else {
        altro.to_string()
    }
}

fn errore(d: &Value) -> String {
    format!("ERRORE: {}", con_predefinito(d, "motivo", "non riuscito"))
}

/// Quando mancano tutti e due i modi di indicare un elemento.
pub fn manca_bersaglio(selettore: &str, testo: &str) -> Option<String> {
    if nova_pitone::senza_bianchi(selettore).is_empty()
        && nova_pitone::senza_bianchi(testo).is_empty()
    {
        Some("ERRORE: serve «selettore» oppure «testo»".into())
    } else {
        None
    }
}

/// `web_apri`: l'identificativo, perche' il modello lo passi agli altri.
pub fn apri(scheda: &Value, url_chiesto: &str) -> String {
    format!(
        "scheda {}\n{}\n{}\nPassa questo identificativo come «scheda» agli altri strumenti web.",
        str_di(scheda.get("id")),
        o_altrimenti(scheda, "titolo", ""),
        o_altrimenti(scheda, "url", url_chiesto),
    )
}

/// `web_trova`: un elemento per riga, con quello che serve per sceglierlo.
pub fn trova(elementi: &Value, selettore: &str, testo: &str) -> String {
    let vuoto = Vec::new();
    let e = elementi.as_array().unwrap_or(&vuoto);
    if e.is_empty() {
        let che = if nova_pitone::senza_bianchi(testo).is_empty() {
            format!("«{selettore}»")
        } else {
            format!("«{testo}»")
        };
        return format!("nessun elemento per {che}");
    }
    let mut righe = vec![format!("{} elementi:", e.len())];
    for x in e {
        let mut pezzi = vec![str_di(x.get("tag"))];
        if vero(x.get("id")) {
            pezzi.push(format!("#{}", str_di(x.get("id"))));
        }
        if vero(x.get("ruolo")) {
            pezzi.push(format!("role={}", str_di(x.get("ruolo"))));
        }
        if vero(x.get("etichetta")) {
            let e = str_di(x.get("etichetta"));
            pezzi.push(format!("aria-label={}", repr_stringa(&e)));
        }
        if !vero(x.get("visibile")) {
            pezzi.push("(non visibile)".into());
        }
        let coda = if vero(x.get("testo")) {
            format!("  «{}»", str_di(x.get("testo")))
        } else {
            String::new()
        };
        righe.push(format!("  {}{coda}", pezzi.join(" ")));
    }
    righe.join("\n")
}

/// `web_tabella`: la tabella gia' a tabulazioni, con quante righe mancano.
pub fn tabella(d: &Value, righe: i64) -> String {
    if !vero(d.get("ok")) {
        return errore(d);
    }
    let coda = if vero(d.get("tagliato")) {
        let tutte = d.get("righe").and_then(Value::as_i64).unwrap_or(0);
        format!(
            "\n[...altre {} righe: rifai con «righe» piu' alto]",
            tutte - righe
        )
    } else {
        String::new()
    };
    format!(
        "{}: {} righe x {} colonne\n{}{coda}",
        str_di(d.get("quale")),
        str_di(d.get("righe")),
        str_di(d.get("colonne")),
        o_altrimenti(d, "tsv", ""),
    )
}

/// `web_leggi`: titolo, indirizzo e il testo visibile.
pub fn leggi(d: &Value) -> String {
    let coda = if vero(d.get("tagliato")) {
        "\n[...tagliato]"
    } else {
        ""
    };
    format!(
        "{}\n{}\n\n{}{coda}",
        str_di(d.get("titolo")),
        str_di(d.get("url")),
        o_altrimenti(d, "testo", ""),
    )
}

/// `web_click`: cosa si e' premuto, e se c'erano altri col testo uguale.
pub fn click(r: &Value, selettore: &str, testo: &str) -> (String, Option<Annotazione>) {
    if !vero(r.get("ok")) {
        return (errore(r), None);
    }
    let su = if vero(r.get("su")) {
        str_di(r.get("su"))
    } else if !selettore.is_empty() {
        selettore.to_string()
    } else {
        testo.to_string()
    };
    let dettagli = if !selettore.is_empty() {
        format!("selettore: {selettore}")
    } else {
        format!("testo: {testo}")
    };
    let a = nota(format!("premuto «{su}»"), dettagli);
    let altri = if vero(r.get("altri")) {
        str_di(r.get("altri"))
    } else {
        String::new()
    };
    let n = if altri.is_empty() {
        String::new()
    } else {
        format!(" (altri {altri} con lo stesso testo)")
    };
    (
        format!("premuto: {}{n}", o_altrimenti(r, "su", selettore)),
        Some(a),
    )
}

/// `web_scrivi` con un testo.
pub fn scrivi(esito: &Value, selettore: &str, testo: &str) -> (String, Option<Annotazione>) {
    if !vero(esito.get("ok")) {
        return (errore(esito), None);
    }
    (
        format!("scritto in {selettore}"),
        Some(nota(format!("scritto in {selettore}"), testo.to_string())),
    )
}

/// `web_scrivi` con una credenziale: si dice il **nome**, mai il valore.
pub fn scrivi_segreto(
    esito: &Value,
    selettore: &str,
    segreto: &str,
) -> (String, Option<Annotazione>) {
    if !vero(esito.get("ok")) {
        return (errore(esito), None);
    }
    (
        format!("scritta la credenziale «{segreto}» in {selettore} (valore non mostrato)"),
        Some(Annotazione {
            azione: format!("usata la credenziale «{segreto}»"),
            dettagli: format!("scritta in {selettore}"),
            tipo: "credenziale".into(),
        }),
    )
}

/// Quando in archivio quella credenziale non c'e'.
pub fn segreto_assente(segreto: &str) -> String {
    format!("ERRORE: nessuna credenziale «{segreto}» in archivio")
}

/// Dopo il copione dell'incolla: cosa si fa.
#[derive(Debug, Clone, PartialEq)]
pub enum DopoIncolla {
    /// Si risponde con questo, cosi' com'e'.
    Fine(Value),
    /// La pagina non l'ha preso ma il campo accetta testo: si consegna con
    /// `Input.insertText`, e poi si risponde con questo.
    Inserisci(Value),
}

/// La scelta di `browser.incolla` dopo il primo tentativo.
///
/// Prima l'evento `paste`, che e' quello che aspettano le griglie; se la
/// pagina non l'ha preso e l'elemento accetta testo, `Input.insertText`. Se
/// non lo accetta **si dice**, invece di consegnare il testo al nulla e
/// tornare «fatto».
pub fn dopo_incolla(esito: &Value) -> DopoIncolla {
    if !vero(esito.get("ok")) {
        return DopoIncolla::Fine(esito.clone());
    }
    let con_come = |come: &str| {
        let mut o = serde_json::Map::new();
        o.insert("come".into(), Value::String(come.into()));
        if let Some(e) = esito.as_object() {
            for (k, v) in e {
                o.insert(k.clone(), v.clone());
            }
        }
        Value::Object(o)
    };
    if vero(esito.get("preso_dalla_pagina")) {
        return DopoIncolla::Fine(con_come("evento incolla"));
    }
    if !vero(esito.get("scrivibile")) {
        return DopoIncolla::Fine(serde_json::json!({
            "ok": false,
            "come": "evento incolla",
            "motivo": format!(
                "la pagina non ha preso l'incolla e l'elemento non accetta testo: serve un \
                 altro punto d'appoggio (elemento: {})",
                str_di(esito.get("su"))
            ),
        }));
    }
    DopoIncolla::Inserisci(con_come("insertText"))
}

/// `web_incolla`: quante righe e colonne sono entrate, e dove.
pub fn incolla(r: &Value, testo: &str) -> (String, Option<Annotazione>) {
    let ok = r.get("ok").map_or(true, |v| vero(Some(v)));
    let motivo = vero(r.get("motivo"));
    let a = if ok && !motivo {
        Some(nota(
            format!("incollate {} righe", testo.matches('\n').count() + 1),
            testo.to_string(),
        ))
    } else {
        None
    };
    if !ok || motivo {
        return (errore(r), a);
    }
    let righe = testo.matches('\n').count() + usize::from(!testo.ends_with('\n'));
    let colonne = if testo.is_empty() {
        0
    } else {
        let r = nova_pitone::righe(testo);
        if r.is_empty() {
            1
        } else {
            r.iter()
                .map(|x| x.matches('\t').count() + 1)
                .max()
                .unwrap_or(1)
        }
    };
    (
        format!(
            "incollate {righe} righe x {colonne} colonne in {} ({})",
            o_altrimenti(r, "su", "dove stava il fuoco"),
            str_di(r.get("come"))
        ),
        a,
    )
}

/// `web_carica`: quali file sono entrati nel campo.
pub fn carica(r: &Value, selettore: &str) -> (String, Option<Annotazione>) {
    if !vero(r.get("ok")) {
        return (errore(r), None);
    }
    let file: Vec<String> = r
        .get("file")
        .and_then(Value::as_array)
        .map(|a| a.iter().map(|x| str_di(Some(x))).collect())
        .unwrap_or_default();
    let elenco = file.join(", ");
    (
        format!("consegnati a {selettore}: {elenco}"),
        Some(nota(format!("consegnati file a {selettore}"), elenco)),
    )
}

/// Il campo scelto per `carica` e' un campo file? `tipo` e' quello che
/// riporta la pagina: `tag:type`.
pub fn non_e_un_campo_file(tipo: &str) -> Option<String> {
    if tipo == "input:file" {
        None
    } else {
        let che = if tipo.is_empty() { "?" } else { tipo };
        Some(format!("quel selettore non e' un campo file (e' {che})"))
    }
}

#[cfg(test)]
mod prove {
    use super::*;
    use serde_json::json;

    #[test]
    fn un_elemento_si_racconta_come_in_python() {
        let e = json!([{"tag": "button", "id": "ok", "ruolo": null, "etichetta": "l'invio",
                        "testo": "Invia", "visibile": false}]);
        assert_eq!(
            trova(&e, "button", ""),
            "1 elementi:\n  button #ok aria-label=\"l'invio\" (non visibile)  «Invia»"
        );
        assert_eq!(trova(&json!([]), "#x", " "), "nessun elemento per «#x»");
    }

    #[test]
    fn un_incolla_che_la_pagina_ignora_non_torna_fatto() {
        let e = json!({"ok": true, "preso_dalla_pagina": false, "scrivibile": false, "su": "div"});
        match dopo_incolla(&e) {
            DopoIncolla::Fine(v) => assert_eq!(v["ok"], false),
            altro => panic!("{altro:?}"),
        }
    }
}
