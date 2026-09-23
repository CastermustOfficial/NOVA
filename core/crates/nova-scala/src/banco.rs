//! Il banco di confronto per la scala.
//!
//! Qui il confronto pesa piu' che negli altri: gli altri banchi confrontano
//! un ordinamento o un conteggio, questo confronta **cosa esce dal PC**. Una
//! divergenza fra le due parti non e' un suggerimento sbagliato: e' un
//! compito che prende la porta quando doveva restare in casa, o che resta in
//! casa quando l'utente si aspettava aiuto.

use std::io::Read;

use nova_scala::{
    durata_pausa, e_in_casa, gradino_minimo, host_di, indice, parola_presente, ripieghi, scala,
    successivo, utilizzabile, Categoria, Configurazione, Gradino, Pause,
};
use std::collections::BTreeMap;

use nova_scala::delega::{
    allega, delega, leggi_come_python, secondo_parere, stato, strumento_delega, Cervelli, Detto,
    Guasto, Registro, Richiesta, Traccia,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

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
    /// I nomi delle CLI dichiarate in `brains.cli`, com'e' scritta la chiave.
    #[serde(default)]
    cli_dichiarate: Vec<String>,
    /// I nomi di cervello di cui si vuole sapere la specie.
    #[serde(default)]
    specie: Vec<String>,
    #[serde(default)]
    pause_chieste: Vec<i64>,
    /// Indirizzi di cui si vuole sapere se sono in casa.
    #[serde(default)]
    indirizzi: Vec<String>,
    /// Configurazioni intere, come le ha scritte l'utente: si legge la scala.
    #[serde(default)]
    configurazioni: Vec<Value>,
    /// Scenari di delega, giocati passo per passo coi cervelli finti.
    #[serde(default)]
    scenari: Vec<Scenario>,
}

/// Un copione: la configurazione, chi fa spendere, cosa risponde ognuno, e
/// i passi.
#[derive(Deserialize)]
struct Scenario {
    config: Value,
    #[serde(default)]
    a_consumo: Vec<String>,
    /// Gradino -> risposte in ordine: {"ok", "costo", "durata"},
    /// {"limite": secondi} o {"errore": testo}. Finite quelle, risponde
    /// «risposta di <gradino>».
    #[serde(default)]
    copione: BTreeMap<String, Vec<Value>>,
    orologio: f64,
    passi: Vec<Value>,
}

/// I cervelli finti del banco: rispondono dal copione, e si ricordano cosa
/// e' stato chiesto a chi.
struct Finti {
    a_consumo: Vec<String>,
    copione: BTreeMap<String, Vec<Value>>,
    spenti: BTreeMap<String, String>,
    orologio: f64,
    righe: Vec<String>,
    chiesti: Vec<(String, String)>,
}

impl Cervelli for Finti {
    fn a_consumo(&self, g: &Gradino) -> bool {
        self.a_consumo.contains(&g.nome)
    }
    fn chiedi(&mut self, g: &Gradino, prompt: &str) -> Result<Detto, Guasto> {
        if let Some(p) = self.spenti.get(&g.nome) {
            return Err(Guasto::NonPronto(p.clone()));
        }
        self.chiesti.push((g.nome.clone(), prompt.to_string()));
        let coda = self.copione.entry(g.nome.clone()).or_default();
        if coda.is_empty() {
            return Ok(Detto {
                testo: format!("risposta di {}", g.nome),
                ..Default::default()
            });
        }
        let r = coda.remove(0);
        if let Some(s) = r.get("limite").and_then(Value::as_i64) {
            return Err(Guasto::Limite { riprova_fra_s: s });
        }
        if let Some(e) = r.get("errore").and_then(Value::as_str) {
            return Err(Guasto::Altro(e.to_string()));
        }
        Ok(Detto {
            testo: r
                .get("ok")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            costo_usd: r.get("costo").and_then(Value::as_f64).unwrap_or(0.0),
            durata_ms: r.get("durata").and_then(Value::as_i64).unwrap_or(0),
        })
    }
    fn ora(&self) -> f64 {
        self.orologio
    }
    fn annota(&mut self, riga: &str) {
        self.righe.push(riga.to_string());
    }
}

fn testo_di(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

fn file_di(v: &Value) -> Vec<String> {
    v.get("file")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn traccia_json(t: &Traccia) -> Value {
    json!([
        t.da,
        t.a,
        t.motivo,
        t.compito,
        t.esito,
        t.costo_usd,
        t.durata_ms
    ])
}

fn gioca(sc: &Scenario) -> Value {
    let routing = nova_scala::routing_effettivo(&sc.config);
    let cfg = nova_scala::da_routing(&routing);
    let mut reg = Registro::default();
    let mut c = Finti {
        a_consumo: sc.a_consumo.clone(),
        copione: sc.copione.clone(),
        spenti: BTreeMap::new(),
        orologio: sc.orologio,
        righe: Vec::new(),
        chiesti: Vec::new(),
    };
    let mut risultati = Vec::new();
    for p in &sc.passi {
        let r = match testo_di(p, "tipo").as_str() {
            "delega" => {
                let file = file_di(p);
                let contesto = allega(&testo_di(p, "contesto"), &file, &leggi_come_python);
                match strumento_delega(
                    &cfg,
                    &mut reg,
                    &mut c,
                    &testo_di(p, "a"),
                    &testo_di(p, "compito"),
                    &testo_di(p, "motivo"),
                    &contesto,
                    file.len() as i64,
                ) {
                    Ok(t) => json!({"Ok": t}),
                    Err(e) => json!({"Err": e}),
                }
            }
            "grezza" => {
                let r = Richiesta {
                    a: testo_di(p, "a"),
                    compito: testo_di(p, "compito"),
                    motivo: testo_di(p, "motivo"),
                    da: testo_di(p, "da"),
                    contesto: testo_di(p, "contesto"),
                    allegati: p.get("allegati").and_then(Value::as_i64).unwrap_or(0),
                    salta_regola: p
                        .get("salta_regola")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                };
                match delega(&cfg, &mut reg, &mut c, &r, false) {
                    Ok(t) => json!({"Ok": traccia_json(&t)}),
                    Err(e) => json!({"Err": e}),
                }
            }
            "parere" => {
                let file = file_di(p);
                let contesto = allega("", &file, &leggi_come_python);
                let primo = p.get("primo").and_then(Value::as_str).unwrap_or("standard");
                let secondo = p
                    .get("secondo")
                    .and_then(Value::as_str)
                    .unwrap_or("alternativo");
                json!(secondo_parere(
                    &cfg,
                    &mut reg,
                    &mut c,
                    &testo_di(p, "domanda"),
                    primo,
                    secondo,
                    &contesto,
                    file.len() as i64,
                ))
            }
            "stato" => {
                let spenti = c.spenti.clone();
                let pronto = |g: &Gradino| match spenti.get(&g.nome) {
                    Some(p) => (false, p.clone()),
                    None => (true, String::new()),
                };
                stato(&cfg, &routing, &reg, &c, &pronto)
            }
            "avanza" => {
                c.orologio += p.get("secondi").and_then(Value::as_f64).unwrap_or(0.0);
                Value::Null
            }
            "spegni" => {
                c.spenti
                    .insert(testo_di(p, "gradino"), testo_di(p, "perche"));
                Value::Null
            }
            "accendi" => {
                c.spenti.remove(&testo_di(p, "gradino"));
                Value::Null
            }
            altro => json!({"Err": format!("passo sconosciuto: {altro}")}),
        };
        risultati.push(r);
    }
    let nomi: Vec<String> = cfg.tiers.iter().map(|t| t.nome.clone()).collect();
    json!({
        "risultati": risultati,
        "righe": c.righe,
        "chiesti": c.chiesti,
        "speso": reg.speso_usd,
        "prenotato": reg.prenotato_usd,
        "storico": reg.storico.iter().map(traccia_json).collect::<Vec<_>>(),
        "pause": nomi.iter().map(|n| (n.clone(), reg.pausa_residua(n, c.orologio)))
            .collect::<BTreeMap<_, _>>(),
    })
}

/// La scala letta da una configurazione, in una forma che si confronta.
fn letta(cfg: &Value) -> Value {
    let c = nova_scala::da_routing(&nova_scala::routing_effettivo(cfg));
    json!({
        "tiers": c.tiers.iter().map(|t| json!([t.nome, t.brain, t.model, t.descrizione,
                                                t.locale, t.a_pagamento])).collect::<Vec<_>>(),
        "scala": scala(&c),
        "escalation_automatica": c.escalation_automatica,
        "solo_locale": c.solo_locale,
        "tetto": c.tetto_usd_sessione,
        "stima": nova_scala::stima(&c),
        "ripiego_su_limite": c.ripiego_su_limite,
        "categorie": c.categorie.iter().map(|k| json!([k.nome, k.attiva, k.gradino_minimo,
                                                      k.parole, k.min_file, k.descrizione]))
            .collect::<Vec<_>>(),
    })
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
    /// Per ogni nome: «locale», «api», «claude» o «cli».
    specie: Vec<String>,
    configurazioni: Vec<Value>,
    predefinito: &'static str,
    scenari: Vec<Value>,
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
        ripiego_su_limite: true,
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
            .map(|c| utilizzabile(&cfg, &c.nome, &d.pause, c.speso_usd, c.prenotato_usd, &paga))
            .collect(),
        parole: d
            .parole
            .iter()
            .map(|(p, t)| parola_presente(&p.to_lowercase(), &t.to_lowercase()))
            .collect(),
        durate: d.pause_chieste.iter().map(|s| durata_pausa(*s)).collect(),
        host: d.indirizzi.iter().map(|u| host_di(u)).collect(),
        in_casa: d.indirizzi.iter().map(|u| e_in_casa(u)).collect(),
        specie: d
            .specie
            .iter()
            .map(|n| {
                match nova_scala::specie_di(n, &d.cli_dichiarate) {
                    nova_scala::Specie::Locale => "locale",
                    nova_scala::Specie::Api => "api",
                    nova_scala::Specie::Claude => "claude",
                    nova_scala::Specie::Cli => "cli",
                }
                .to_string()
            })
            .collect(),
        configurazioni: d.configurazioni.iter().map(letta).collect(),
        predefinito: nova_scala::predefinito::ROUTING_PREDEFINITO,
        scenari: d.scenari.iter().map(gioca).collect(),
    };

    match serde_json::to_string(&fuori) {
        Ok(s) => println!("{s}"),
        Err(e) => {
            eprintln!("non ho potuto scrivere l'uscita: {e}");
            std::process::exit(2);
        }
    }
}
