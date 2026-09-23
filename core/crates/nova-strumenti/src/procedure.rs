//! Vedere e dimenticare le procedure imparate.
//!
//! Gemello di `nova/tools/procedure.py`. Una procedura imparata male e'
//! peggio di una non imparata: viene proposta con la stessa sicurezza di una
//! buona, e manda NOVA sulla strada sbagliata prima ancora che ci pensi. Deve
//! poter sparire senza aprire un file a mano.
//!
//! **Si lavora sulle voci cosi' come stanno nel file**, non sulla
//! `Ricetta` di `nova-ricette`. La `Ricetta` e' fatta per ritrovare una
//! procedura, e tiene i campi che servono a quello: riscrivere l'archivio
//! passando da li' vorrebbe dire perdere ogni campo che non conosce, e
//! scrivere `12.0` dove Python aveva scritto `12`. Dimenticarne una deve
//! lasciare le altre **byte per byte** come le avrebbe lasciate Python.

use serde_json::Value;

use crate::data::Fuso;

/// Quante procedure si mostrano al massimo.
pub const MASSIMO_ELENCO: usize = 25;

/// Quante righe di ciascuna.
pub const RIGHE_PER_PROCEDURA: usize = 6;

/// `str(x)` di Python, per i valori che in una procedura ci possono stare.
///
/// Le liste e i dizionari Python li scriverebbe con gli apici singoli; qui
/// escono in JSON. In un archivio scritto da NOVA non ci sono, e se ci sono
/// l'archivio e' stato toccato a mano.
fn come_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::Number(n) if n.is_f64() => n
            .as_f64()
            .map_or_else(|| n.to_string(), nova_pitone::float_come_python),
        altro => altro.to_string(),
    }
}

fn testo<'a>(r: &'a Value, chiave: &str) -> &'a str {
    r.get(chiave).and_then(Value::as_str).unwrap_or("")
}

fn ultimo_uso(r: &Value) -> f64 {
    r.get("ultimo_uso").and_then(Value::as_f64).unwrap_or(0.0)
}

/// `procedure_elenco(cerca)`: le piu' recenti prima, filtrate su titolo e
/// domanda che le ha fatte nascere.
pub fn elenco(voci: &[Value], cerca: &str, fuso: &dyn Fuso) -> String {
    let mut ordinate: Vec<&Value> = voci.iter().filter(|v| v.is_object()).collect();
    // Stabile, come `sorted(..., reverse=True)`: a pari data resta l'ordine
    // del file.
    ordinate.sort_by(|a, b| {
        ultimo_uso(b)
            .partial_cmp(&ultimo_uso(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let filtro = nova_pitone::senza_bianchi(cerca).to_lowercase();
    if !filtro.is_empty() {
        ordinate.retain(|r| {
            format!("{} {}", testo(r, "titolo"), testo(r, "innesco"))
                .to_lowercase()
                .contains(&filtro)
        });
    }
    if ordinate.is_empty() {
        let mut detto = "nessuna procedura imparata".to_string();
        if !filtro.is_empty() {
            detto.push_str(&format!(" per «{cerca}»"));
        }
        return detto;
    }

    let mut righe = Vec::new();
    for r in ordinate.iter().take(MASSIMO_ELENCO) {
        let istante = ultimo_uso(r).floor().max(0.0) as u64;
        let d = nova_calendario::da_istante(istante as i64, fuso.secondi_in(istante));
        let quando = format!("{:02}/{:02} {:02}:{:02}", d.giorno, d.mese, d.ora, d.minuto);
        let titolo = r.get("titolo").map_or_else(|| "?".to_string(), come_str);
        let usata = r.get("usata").map_or_else(|| "1".to_string(), come_str);
        let secondi = r.get("secondi").map_or_else(|| "0".to_string(), come_str);
        let id = r.get("id").map(come_str).unwrap_or_default();
        righe.push(format!(
            "{id}  {titolo}  (usata {usata}x, ultima {quando}, la prima volta {secondi}s)"
        ));
        for riga in nova_pitone::righe(testo(r, "procedura"))
            .iter()
            .take(RIGHE_PER_PROCEDURA)
        {
            righe.push(format!("      {}", nova_pitone::senza_bianchi(riga)));
        }
    }
    righe.join("\n")
}

/// L'archivio senza la procedura `id`, o `None` se non c'era.
///
/// Si confronta con l'identificativo **ripulito dagli spazi**, come fa il
/// Python: chi lo copia da un elenco si porta dietro quello che c'e' intorno.
pub fn senza(voci: &[Value], id: &str) -> Option<Vec<Value>> {
    let id = nova_pitone::senza_bianchi(id);
    let restano: Vec<Value> = voci
        .iter()
        .filter(|v| v.get("id").and_then(Value::as_str) != Some(id))
        .cloned()
        .collect();
    (restano.len() != voci.len()).then_some(restano)
}

pub fn dimenticata(id: &str) -> String {
    format!("procedura {id} dimenticata")
}

pub fn non_trovata(id: &str) -> String {
    format!("non trovo nessuna procedura «{id}»")
}

#[cfg(test)]
mod prove {
    use super::*;
    use crate::data::FusoFisso;
    use serde_json::json;

    #[test]
    fn le_piu_recenti_prima_e_i_numeri_come_python() {
        let voci = vec![
            json!({"id": "a", "titolo": "vecchia", "ultimo_uso": 0, "usata": 2, "secondi": 12}),
            json!({"id": "b", "titolo": "nuova", "ultimo_uso": 86400.5, "secondi": 3.0,
                   "procedura": "  uno\npasso due  \n"}),
        ];
        let e = elenco(&voci, "", &FusoFisso(0));
        assert_eq!(
            e,
            "b  nuova  (usata 1x, ultima 02/01 00:00, la prima volta 3.0s)\n      uno\n      passo due\n\
             a  vecchia  (usata 2x, ultima 01/01 00:00, la prima volta 12s)"
        );
        assert_eq!(elenco(&voci, " NUOVA ", &FusoFisso(0)).lines().count(), 3);
        assert_eq!(
            elenco(&voci, "zz", &FusoFisso(0)),
            "nessuna procedura imparata per «zz»"
        );
        assert_eq!(
            elenco(&[], "  ", &FusoFisso(0)),
            "nessuna procedura imparata"
        );
    }

    #[test]
    fn dimenticare_toglie_solo_quella() {
        let voci = vec![json!({"id": "a", "x": 1}), json!({"id": "b"})];
        assert_eq!(senza(&voci, " a "), Some(vec![json!({"id": "b"})]));
        assert_eq!(senza(&voci, "c"), None);
    }
}
