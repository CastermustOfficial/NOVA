//! Il banco di confronto per la scala.
//!
//! Qui il confronto pesa piu' che negli altri: gli altri banchi confrontano
//! un ordinamento o un conteggio, questo confronta **cosa esce dal PC**. Una
//! divergenza fra le due parti non e' un suggerimento sbagliato: e' un
//! compito che prende la porta quando doveva restare in casa, o che resta in
//! casa quando l'utente si aspettava aiuto.

use std::io::Read;

use nova_scala::{
    durata_pausa, e_in_casa, gradino_minimo, host_di, indice, parola_presente, ripieghi,
    scala, successivo, utilizzabile, Categoria, Configurazione, Gradino, Pause,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct GradinoIn {
    nome: String,
    #[serde(default)]
    brain: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    descrizione: String,
    #[serde(default)]
    locale: bool,
    #[serde(default)]
    a_pagamento: bool,
}

#[derive(Deserialize)]
struct CategoriaIn {
    #[serde(default)]
    nome: String,
    #[serde(default = "vero")]
    attiva: bool,
    #[serde(default)]
    gradino_minimo: String,
    #[serde(default)]
    parole: Vec<String>,
    #[serde(default)]
    min_file: i64,
    #[serde(default)]
    descrizione: String,
}

fn vero() -> bool {
    true
}

#[derive(Deserialize)]
struct CasoUtile {
    nome: String,
    #[serde(default)]
    speso_usd: f64,
    #[serde(default)]
    prenotato_usd: f64,
}

#[derive(Deserialize)]
struct Dentro {
    #[serde(default)]
    tiers: Vec<GradinoIn>,
    #[serde(default)]
    scala: Vec<String>,
    #[serde(default = "vero")]
    escalation_automatica: bool,
    #[serde(default)]
    solo_locale: bool,
    #[serde(default)]
    categorie: Vec<CategoriaIn>,
    #[serde(default)]
    tetto_usd_sessione: f64,
    #[serde(default)]
    costo_stimato_delega: f64,
    /// Gradino -> secondi di pausa che mancano. Si passa da fuori: dentro non
    /// c'e' un orologio, cosi' il confronto non dipende da quando lo si fa.
    #[serde(default)]
    pause: Pause,
    /// I compiti su cui calcolare il gradino minimo, col numero di allegati.
    #[serde(default)]
    compiti: Vec<(String, i64)>,
    #[serde(default)]
    da_sostituire: Vec<String>,
    #[serde(default)]
    utili: Vec<CasoUtile>,
    /// I gradini che fanno spendere: si dichiara, non si costruisce.
    #[serde(default)]
    a_consumo: Vec<String>,
    #[serde(default)]
    parole: Vec<(String, String)>,
    #[serde(default)]
    pause_chieste: Vec<i64>,
    /// Indirizzi di cui si vuole sapere se sono in casa.
    #[serde(default)]
    indirizzi: Vec<String>,
}

#[derive(Serialize)]
struct Fuori {
    scala: Vec<String>,
    successivi: Vec<Option<String>>,
    indici: Vec<i64>,
    gradini_minimi: Vec<(String, String)>,
    ripieghi: Vec<Vec<String>>,
    utilizzabili: Vec<bool>,
    parole: Vec<bool>,
    durate: Vec<i64>,
    host: Vec<String>,
    in_casa: Vec<bool>,
}

fn main() {
    let mut testo = String::new();
    if std::io::stdin().read_to_string(&mut testo).is_err() {
        eprintln!("non ho potuto leggere l'ingresso");
        std::process::exit(2);
    }
    let d: Dentro = match serde_json::from_str(&testo) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("ingresso illeggibile: {e}");
            std::process::exit(2);
        }
    };

    let cfg = Configurazione {
        tiers: d
            .tiers
            .iter()
            .map(|t| Gradino {
                nome: t.nome.clone(),
                brain: t.brain.clone(),
                model: t.model.clone(),
                descrizione: t.descrizione.clone(),
                locale: t.locale,
                a_pagamento: t.a_pagamento,
            })
            .collect(),
        scala_dichiarata: d.scala.clone(),
        escalation_automatica: d.escalation_automatica,
        solo_locale: d.solo_locale,
        categorie: d
            .categorie
            .iter()
            .map(|c| Categoria {
                nome: c.nome.clone(),
                attiva: c.attiva,
                gradino_minimo: c.gradino_minimo.clone(),
                parole: c.parole.clone(),
                min_file: c.min_file,
                descrizione: c.descrizione.clone(),
            })
            .collect(),
        tetto_usd_sessione: d.tetto_usd_sessione,
        costo_stimato_delega: d.costo_stimato_delega,
    };

    let paga = |t: &Gradino| d.a_consumo.iter().any(|n| *n == t.nome);
    let nomi: Vec<String> = cfg.tiers.iter().map(|t| t.nome.clone()).collect();

    let fuori = Fuori {
        scala: scala(&cfg),
        successivi: nomi.iter().map(|n| successivo(&cfg, n)).collect(),
        indici: nomi.iter().map(|n| indice(&cfg, n)).collect(),
        gradini_minimi: d
            .compiti
            .iter()
            .map(|(c, a)| gradino_minimo(&cfg, c, *a))
            .collect(),
        ripieghi: d
            .da_sostituire
            .iter()
            .map(|n| ripieghi(&cfg, n, &d.pause))
            .collect(),
        utilizzabili: d
            .utili
            .iter()
            .map(|c| {
                utilizzabile(&cfg, &c.nome, &d.pause, c.speso_usd, c.prenotato_usd, &paga)
            })
            .collect(),
        parole: d
            .parole
            .iter()
            .map(|(p, t)| parola_presente(&p.to_lowercase(), &t.to_lowercase()))
            .collect(),
        durate: d.pause_chieste.iter().map(|s| durata_pausa(*s)).collect(),
        host: d.indirizzi.iter().map(|u| host_di(u)).collect(),
        in_casa: d.indirizzi.iter().map(|u| e_in_casa(u)).collect(),
    };

    match serde_json::to_string(&fuori) {
        Ok(s) => println!("{s}"),
        Err(e) => {
            eprintln!("non ho potuto scrivere l'uscita: {e}");
            std::process::exit(2);
        }
    }
}
