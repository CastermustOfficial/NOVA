//! Il banco di confronto: le due versioni devono dire la stessa cosa.
//!
//! Un porting che «sembra giusto» non e' un porting: e' una riscrittura di
//! cui nessuno sa piu' se cambia qualcosa. Qui il Python scrive l'archivio e
//! le domande su un file, questo binario risponde con i propri punteggi, e
//! il Python confronta cifra per cifra. Finche' i due sono d'accordo su
//! ogni domanda, la traduzione regge; il giorno che divergono, si vede su
//! quale domanda e di quanto.
//!
//! Si legge da stdin e si scrive su stdout: nessun file da concordare,
//! nessun percorso da indovinare.

use std::io::Read;

use nova_ricette::blocco::{blocco, Proposta};
use nova_ricette::imparare;
use nova_ricette::{proponi, Ricetta};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct RicettaIn {
    #[serde(default)]
    id: String,
    #[serde(default)]
    parole: Vec<String>,
    #[serde(default)]
    parole_alias: Vec<String>,
    #[serde(default)]
    parole_passi: Vec<String>,
    #[serde(default)]
    usata: i64,
    #[serde(default)]
    ultimo_uso: f64,
    #[serde(default)]
    titolo: String,
    #[serde(default)]
    procedura: String,
    #[serde(default)]
    strumenti: Vec<String>,
    #[serde(default)]
    creata: f64,
    #[serde(default)]
    innesco: String,
    #[serde(default)]
    secondi: f64,
}

/// Una ricetta come esce dalla fusione o da una registrazione: si confronta
/// campo per campo, perche' un archivio giusto con un contatore sbagliato
/// propone la stessa cosa con la sicurezza sbagliata.
#[derive(Serialize)]
struct RicettaFuori {
    id: String,
    parole: Vec<String>,
    parole_alias: Vec<String>,
    parole_passi: Vec<String>,
    usata: i64,
    ultimo_uso: f64,
    titolo: String,
    procedura: String,
    strumenti: Vec<String>,
    creata: f64,
    innesco: String,
    secondi: f64,
}

#[derive(Deserialize)]
struct PropostaIn {
    #[serde(default)]
    titolo: String,
    #[serde(default)]
    procedura: String,
    #[serde(default)]
    usata: i64,
    #[serde(default)]
    somiglianza: f64,
    #[serde(default)]
    ha_automazione: bool,
}

/// Un archivio, e le procedure da registrarci sopra una dopo l'altra.
#[derive(Deserialize)]
struct ScenarioRegistra {
    #[serde(default)]
    archivio: Vec<RicettaIn>,
    #[serde(default)]
    passi: Vec<PassoRegistra>,
}

#[derive(Deserialize)]
struct PassoRegistra {
    #[serde(default)]
    domanda: String,
    #[serde(default)]
    titolo: String,
    #[serde(default)]
    procedura: String,
    #[serde(default)]
    strumenti: Vec<String>,
    #[serde(default)]
    secondi: f64,
    #[serde(default)]
    alias: Vec<String>,
    /// L'orologio si passa da fuori: qui dentro non ce n'e' uno.
    #[serde(default)]
    adesso: f64,
    /// E l'identificativo di una procedura nuova pure: il caso non si prova.
    #[serde(default)]
    id_nuovo: String,
}

#[derive(Deserialize)]
struct Dentro {
    #[serde(default)]
    ricette: Vec<RicettaIn>,
    #[serde(default)]
    domande: Vec<String>,
    #[serde(default = "quattro")]
    quante: usize,
    /// Gruppi di procedure gia' scelte, di cui si vuole il testo.
    #[serde(default)]
    blocchi: Vec<Vec<PropostaIn>>,
    /// Numeri da scrivere come li scriverebbe Python.
    #[serde(default)]
    numeri: Vec<f64>,
    /// Numeri da arrotondare come li arrotonderebbe Python.
    #[serde(default)]
    da_arrotondare: Vec<f64>,
    /// (domanda, risposta, strumenti) di cui si vuole il prompt.
    #[serde(default)]
    richieste: Vec<(String, String, Vec<String>)>,
    /// Archivi da fondere, ognuno con la sua soglia.
    #[serde(default)]
    fusioni: Vec<(Vec<RicettaIn>, f64)>,
    /// Registrazioni da fare in fila su un archivio di partenza: e' la meta'
    /// che **scrive**, e finora il banco guardava solo quella che legge.
    #[serde(default)]
    registrazioni: Vec<ScenarioRegistra>,
    /// (attive, secondi, soglia, agentico, quanti strumenti).
    #[serde(default)]
    decisioni: Vec<(bool, f64, i64, bool, usize)>,
    /// Risposte finte del modello, da leggere come procedura.
    #[serde(default)]
    risposte: Vec<String>,
}

fn quattro() -> usize {
    4
}

#[derive(Serialize)]
struct Esito {
    domanda: String,
    /// Indice nell'archivio e punteggio, nell'ordine in cui si propongono.
    scelte: Vec<(usize, f64)>,
}

#[derive(Serialize)]
struct Fuori {
    /// La forma storica dell'uscita, tenuta com'era: il banco delle domande
    /// c'era prima e non ha ragione di cambiare.
    proposte: Vec<Esito>,
    blocchi: Vec<String>,
    numeri: Vec<String>,
    arrotondati: Vec<f64>,
    richieste: Vec<String>,
    /// "" se si registra, altrimenti il motivo nella stessa forma del Python.
    decisioni: Vec<String>,
    /// null se non si e' letta niente, con a fianco il motivo.
    lette: Vec<(Option<(String, String, Vec<String>)>, String)>,
    righe: Vec<Vec<String>>,
    fusioni: Vec<Vec<RicettaFuori>>,
    /// Per ogni scenario: l'archivio com'e' rimasto.
    registrazioni: Vec<Vec<RicettaFuori>>,
    /// Le soglie, dette da questa parte.
    ///
    /// Non si vedono da nessuno degli scenari: il banco le passa da fuori,
    /// quindi una che cambiasse resterebbe verde. Sono numeri che decidono
    /// cosa si propone e cosa si butta, e vanno confrontati per conto loro.
    soglie: Vec<(String, f64)>,
}

fn come_ricetta(r: &RicettaIn) -> Ricetta {
    Ricetta {
        id: r.id.clone(),
        parole: r.parole.clone(),
        parole_alias: r.parole_alias.clone(),
        parole_passi: r.parole_passi.clone(),
        usata: r.usata,
        ultimo_uso: r.ultimo_uso,
        titolo: r.titolo.clone(),
        procedura: r.procedura.clone(),
        strumenti: r.strumenti.clone(),
        creata: r.creata,
        innesco: r.innesco.clone(),
        secondi: r.secondi,
    }
}

fn come_fuori(r: Ricetta) -> RicettaFuori {
    RicettaFuori {
        id: r.id,
        parole: r.parole,
        parole_alias: r.parole_alias,
        parole_passi: r.parole_passi,
        usata: r.usata,
        ultimo_uso: r.ultimo_uso,
        titolo: r.titolo,
        procedura: r.procedura,
        strumenti: r.strumenti,
        creata: r.creata,
        innesco: r.innesco,
        secondi: r.secondi,
    }
}

fn main() {
    let mut testo = String::new();
    if std::io::Read::read_to_string(&mut std::io::stdin(), &mut testo).is_err() {
        eprintln!("non ho potuto leggere da stdin");
        std::process::exit(2);
    }
    let _ = &mut testo as &mut dyn std::fmt::Write;
    let dentro: Dentro = match serde_json::from_str(&testo) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("il JSON in ingresso non si legge: {e}");
            std::process::exit(2);
        }
    };
    let elenco: Vec<Ricetta> = dentro
        .ricette
        .into_iter()
        .map(|r| Ricetta {
            id: r.id,
            parole: r.parole,
            parole_alias: r.parole_alias,
            parole_passi: r.parole_passi,
            usata: r.usata,
            ultimo_uso: r.ultimo_uso,
            titolo: r.titolo,
            procedura: r.procedura,
            strumenti: r.strumenti,
            creata: r.creata,
            innesco: r.innesco,
            secondi: r.secondi,
        })
        .collect();
    let fuori = Fuori {
        proposte: dentro
            .domande
            .into_iter()
            .map(|d| Esito {
                scelte: proponi(&elenco, &d, dentro.quante),
                domanda: d,
            })
            .collect(),
        blocchi: dentro
            .blocchi
            .iter()
            .map(|gruppo| {
                let proposte: Vec<Proposta> = gruppo
                    .iter()
                    .map(|p| Proposta {
                        titolo: p.titolo.clone(),
                        procedura: p.procedura.clone(),
                        usata: p.usata,
                        somiglianza: p.somiglianza,
                        ha_automazione: p.ha_automazione,
                    })
                    .collect();
                blocco(&proposte)
            })
            .collect(),
        numeri: dentro.numeri.iter().map(|x| nova_ricette::blocco::numero(*x)).collect(),
        arrotondati: dentro
            .da_arrotondare
            .iter()
            .map(|x| nova_ricette::blocco::arrotonda2(*x))
            .collect(),
        richieste: dentro
            .richieste
            .iter()
            .map(|(dm, r, s)| imparare::richiesta(dm, r, s))
            .collect(),
        decisioni: dentro
            .decisioni
            .iter()
            .map(|(a, sec, so, ag, n)| match imparare::si_registra(*a, *sec, *so, *ag, *n) {
                Ok(()) => String::new(),
                Err(imparare::NonSiRegistra::Spente) =>
                    "saltata: le procedure sono spente".to_string(),
                Err(imparare::NonSiRegistra::SottoLaSoglia { secondi, soglia }) =>
                    format!("saltata: {secondi}s sotto la soglia di {soglia}"),
                Err(imparare::NonSiRegistra::NessunoStrumento) =>
                    "saltata: nessuno strumento usato".to_string(),
            })
            .collect(),
        lette: dentro
            .risposte
            .iter()
            .map(|r| match imparare::leggi(r) {
                Ok(l) => (Some((l.titolo, l.procedura, l.alias)), String::new()),
                Err(imparare::NonSiLegge::NienteRisposta) =>
                    (None, "niente risposta".to_string()),
                Err(imparare::NonSiLegge::DiceNiente) =>
                    (None, "dice niente".to_string()),
                Err(imparare::NonSiLegge::TroppoCorta { testo }) =>
                    (None, format!("troppo corta|{testo}")),
                Err(imparare::NonSiLegge::PassiScarni { passi }) =>
                    (None, format!("passi scarni|{passi}")),
            })
            .collect(),
        righe: dentro.risposte.iter().map(|r| imparare::righe(r)).collect(),
        soglie: vec![
            ("SOGLIA".into(), nova_ricette::SOGLIA),
            ("SOGLIA_PAROLA".into(), nova_ricette::SOGLIA_PAROLA),
            ("SOGLIA_FUSIONE".into(), nova_ricette::SOGLIA_FUSIONE),
        ],
        fusioni: dentro
            .fusioni
            .iter()
            .map(|(a, soglia)| {
                let e: Vec<Ricetta> = a.iter().map(come_ricetta).collect();
                nova_ricette::unisci(&e, *soglia).into_iter().map(come_fuori).collect()
            })
            .collect(),
        registrazioni: dentro
            .registrazioni
            .iter()
            .map(|s| {
                let mut archivio: Vec<Ricetta> = s.archivio.iter().map(come_ricetta).collect();
                for p in &s.passi {
                    nova_ricette::registra(
                        &mut archivio,
                        &p.domanda,
                        &p.titolo,
                        &p.procedura,
                        &p.strumenti,
                        p.secondi,
                        &p.alias,
                        p.adesso,
                        &p.id_nuovo,
                    );
                }
                archivio.into_iter().map(come_fuori).collect()
            })
            .collect(),
    };
    match serde_json::to_string(&fuori) {
        Ok(s) => println!("{s}"),
        Err(e) => {
            eprintln!("non ho potuto scrivere l'esito: {e}");
            std::process::exit(2);
        }
    }
}

// `Read` importato sopra serve a `read_to_string` su stdin.
const _: fn(&mut std::io::Stdin, &mut String) -> std::io::Result<usize> =
    <std::io::Stdin as Read>::read_to_string;
