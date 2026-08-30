//! Il banco di confronto per il registro: stesse righe, stesse risposte.
//!
//! Qui il confronto pesa piu' che negli altri due banchi. Le ricette e il
//! BM25 sbagliano un ordinamento; questo sbaglia una candidatura che non si
//! ritrova piu', e su quella promessa - «cio' che non si annulla, si annota»
//! - NOVA ci sta in piedi.

use std::io::Read;

use nova_registro::{cerca, data_italiana, giorno, racconta, riassunto, Filtro, Riga};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct RigaIn {
    #[serde(default)]
    quando: String,
    #[serde(default)]
    tipo: String,
    #[serde(default)]
    azione: String,
    #[serde(default)]
    dove: String,
    #[serde(default)]
    dettagli: String,
    #[serde(default)]
    esito: String,
}

#[derive(Deserialize)]
struct FiltroIn {
    #[serde(default)]
    testo: String,
    #[serde(default)]
    tipo: String,
    #[serde(default)]
    esito: String,
    #[serde(default)]
    non_prima_di: String,
    #[serde(default)]
    quante: usize,
}

#[derive(Deserialize)]
struct Dentro {
    righe: Vec<RigaIn>,
    filtri: Vec<FiltroIn>,
    /// La data di oggi, in AAAA-MM-GG: dentro non c'e' un orologio, cosi' il
    /// confronto non dipende da quando lo si esegue.
    oggi: String,
    #[serde(default)]
    date: Vec<String>,
}

#[derive(Serialize)]
struct Fuori {
    /// Per ogni filtro, gli indici delle righe che rispondono.
    trovate: Vec<Vec<usize>>,
    /// Per ogni filtro, il racconto.
    racconti: Vec<String>,
    riassunto: (usize, Vec<(String, usize)>, String, String),
    giorni: Vec<String>,
    italiane: Vec<String>,
}

fn main() {
    let mut testo = String::new();
    if std::io::stdin().read_to_string(&mut testo).is_err() {
        eprintln!("non ho potuto leggere da stdin");
        std::process::exit(2);
    }
    let dentro: Dentro = match serde_json::from_str(&testo) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("il JSON in ingresso non si legge: {e}");
            std::process::exit(2);
        }
    };
    let righe: Vec<Riga> = dentro
        .righe
        .into_iter()
        .map(|r| Riga {
            quando: r.quando, tipo: r.tipo, azione: r.azione,
            dove: r.dove, dettagli: r.dettagli, esito: r.esito,
        })
        .collect();

    let mut trovate = Vec::new();
    let mut racconti = Vec::new();
    for f in &dentro.filtri {
        let filtro = Filtro {
            testo: f.testo.clone(), tipo: f.tipo.clone(), esito: f.esito.clone(),
            non_prima_di: f.non_prima_di.clone(), quante: f.quante,
        };
        let scelte = cerca(&righe, &filtro);
        trovate.push(
            scelte.iter()
                .map(|r| righe.iter().position(|x| x == *r).unwrap_or(usize::MAX))
                .collect::<Vec<usize>>());
        racconti.push(racconta(&scelte, &dentro.oggi));
    }
    let r = riassunto(&righe);
    let fuori = Fuori {
        trovate,
        racconti,
        riassunto: (r.quante, r.tipi, r.prima, r.ultima),
        giorni: dentro.date.iter().map(|d| giorno(d, &dentro.oggi)).collect(),
        italiane: dentro.date.iter()
            .map(|d| data_italiana(&d.chars().take(10).collect::<String>()))
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
