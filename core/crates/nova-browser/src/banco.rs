//! Il banco del browser: il codice che parte verso la pagina dev'essere lo
//! stesso, carattere per carattere.
//!
//! Qui il confronto letterale non e' pignoleria: quel testo lo esegue un
//! interprete che non e' nostro, su una pagina dove l'utente e' gia'
//! autenticato. Una virgoletta di differenza non e' un formato diverso, e' un
//! confine che non c'e' piu'.

use std::io::Read;

use nova_browser::motori::{self, Risultato};
use nova_browser::testo;
use nova_browser::{
    clicca, clicca_testo, dentro, errore_di_pagina, incolla, leggi, per_testo, risultati,
    scheda, scrivi, tabella, trova, valuta_params, NessunaScheda, Scheda,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize)]
struct SchedaIn {
    #[serde(default)]
    id: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    titolo: String,
}

#[derive(Deserialize)]
struct CasoScheda {
    #[serde(default)]
    elenco: Vec<SchedaIn>,
    #[serde(default)]
    quale: String,
}

#[derive(Deserialize)]
struct Dentro {
    /// Valori da mettere dentro il JavaScript.
    #[serde(default)]
    valori: Vec<String>,
    /// (selettore, quanti)
    #[serde(default)]
    trova: Vec<(String, i64)>,
    /// (testo, selettore, quanti, esatto)
    #[serde(default)]
    per_testo: Vec<(String, String, i64, bool)>,
    #[serde(default)]
    clicca: Vec<String>,
    /// (testo, selettore)
    #[serde(default)]
    clicca_testo: Vec<(String, String)>,
    /// (selettore, testo)
    #[serde(default)]
    scrivi: Vec<(String, String)>,
    /// (testo, selettore)
    #[serde(default)]
    incolla: Vec<(String, String)>,
    /// (selettore, righe, caratteri_cella)
    #[serde(default)]
    tabella: Vec<(String, i64, i64)>,
    #[serde(default)]
    leggi: Vec<i64>,
    #[serde(default)]
    schede: Vec<CasoScheda>,
    #[serde(default)]
    risposte: Vec<Value>,
    #[serde(default)]
    espressioni: Vec<String>,
    /// (caratteri, quanti) per il copione dei risultati.
    #[serde(default)]
    risultati: Vec<(i64, i64)>,
    /// Pagine HTML di DuckDuckGo da raschiare, con quanti risultati tenere.
    #[serde(default)]
    ddg_html: Vec<(String, usize)>,
    #[serde(default)]
    ddg_lite: Vec<(String, usize)>,
    /// Indirizzi di cui sbrogliare il rimbalzo.
    #[serde(default)]
    rimbalzi: Vec<String>,
    /// Testi da sciogliere dalle entita' HTML.
    #[serde(default)]
    entita: Vec<String>,
    /// (indice, titolo, url, riassunto) da raccontare.
    #[serde(default)]
    righe: Vec<(usize, String, String, String)>,
    /// Pagine HTML da ridurre a testo.
    #[serde(default)]
    pagine: Vec<String>,
    /// (pagina, quanto del titolo tenere).
    #[serde(default)]
    titoli: Vec<(String, usize)>,
}

#[derive(Serialize)]
struct Fuori {
    valori: Vec<String>,
    trova: Vec<String>,
    per_testo: Vec<String>,
    clicca: Vec<String>,
    clicca_testo: Vec<String>,
    scrivi: Vec<String>,
    incolla: Vec<String>,
    tabella: Vec<String>,
    leggi: Vec<String>,
    /// L'identificativo scelto, oppure il motivo per cui non si e' scelto.
    schede: Vec<Result<String, String>>,
    errori: Vec<Option<String>>,
    params: Vec<Value>,
    risultati: Vec<String>,
    ddg_html: Vec<Vec<(String, String, String)>>,
    ddg_lite: Vec<Vec<(String, String, String)>>,
    rimbalzi: Vec<String>,
    entita: Vec<String>,
    righe: Vec<String>,
    pagine: Vec<String>,
    titoli: Vec<String>,
}

fn come_tre(r: &[Risultato]) -> Vec<(String, String, String)> {
    r.iter()
        .map(|x| (x.titolo.clone(), x.url.clone(), x.riassunto.clone()))
        .collect()
}

fn main() {
    let mut testo = String::new();
    if std::io::stdin().read_to_string(&mut testo).is_err() {
        eprintln!("non ho potuto leggere l'entrata");
        std::process::exit(2);
    }
    let d: Dentro = match serde_json::from_str(&testo) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("l'entrata non e' JSON valido: {e}");
            std::process::exit(2);
        }
    };

    let fuori = Fuori {
        valori: d.valori.iter().map(|v| dentro(v)).collect(),
        trova: d.trova.iter().map(|(s, q)| trova(s, *q)).collect(),
        per_testo: d
            .per_testo
            .iter()
            .map(|(t, s, q, e)| per_testo(t, s, *q, *e))
            .collect(),
        clicca: d.clicca.iter().map(|s| clicca(s)).collect(),
        clicca_testo: d.clicca_testo.iter().map(|(t, s)| clicca_testo(t, s)).collect(),
        scrivi: d.scrivi.iter().map(|(s, t)| scrivi(s, t)).collect(),
        incolla: d.incolla.iter().map(|(t, s)| incolla(t, s)).collect(),
        tabella: d.tabella.iter().map(|(s, r, c)| tabella(s, *r, *c)).collect(),
        leggi: d.leggi.iter().map(|c| leggi(*c)).collect(),
        schede: d
            .schede
            .iter()
            .map(|c| {
                let e: Vec<Scheda> = c
                    .elenco
                    .iter()
                    .map(|s| Scheda {
                        id: s.id.clone(),
                        url: s.url.clone(),
                        titolo: s.titolo.clone(),
                    })
                    .collect();
                match scheda(&e, &c.quale) {
                    Ok(t) => Ok(t.id.clone()),
                    Err(NessunaScheda::NonCeNeSono) => Err("nessuna scheda aperta".into()),
                    Err(NessunaScheda::NessunaCosi { quale, quante }) => {
                        Err(format!("nessuna scheda «{quale}» fra le {quante} aperte"))
                    }
                }
            })
            .collect(),
        errori: d.risposte.iter().map(errore_di_pagina).collect(),
        params: d.espressioni.iter().map(|e| valuta_params(e)).collect(),
        risultati: d.risultati.iter().map(|(c, q)| risultati(*c, *q)).collect(),
        ddg_html: d
            .ddg_html
            .iter()
            .map(|(p, q)| come_tre(&motori::da_html(p, *q)))
            .collect(),
        ddg_lite: d
            .ddg_lite
            .iter()
            .map(|(p, q)| come_tre(&motori::da_lite(p, *q)))
            .collect(),
        rimbalzi: d.rimbalzi.iter().map(|u| motori::senza_rimbalzo(u)).collect(),
        entita: d.entita.iter().map(|t| testo::scioglie(t)).collect(),
        righe: d
            .righe
            .iter()
            .map(|(i, t, u, r)| motori::riga(*i, t, u, r))
            .collect(),
        pagine: d.pagine.iter().map(|p| testo::a_testo(p)).collect(),
        titoli: d.titoli.iter().map(|(p, m)| testo::titolo_di(p, *m)).collect(),
    };

    match serde_json::to_string(&fuori) {
        Ok(s) => println!("{s}"),
        Err(e) => {
            eprintln!("non ho potuto scrivere l'uscita: {e}");
            std::process::exit(2);
        }
    }
}
