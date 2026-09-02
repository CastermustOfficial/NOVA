//! `nova-catalogo`: il verdetto in JSON, per chi non parla Python.
//!
//! Non e' un banco da prova soltanto: e' il **binario che l'installatore
//! chiamera' al posto di `python -m nova.catalogo`. Legge da standard input
//! `{famiglia, vram_gb, ram_gb, catalogo}` e scrive una riga di JSON, la
//! stessa forma di prima, cosi' `install.ps1` cambia una riga e non un
//! ragionamento.
//!
//! Il senso di portarlo e' tutto qui: di questo conto l'installatore non deve
//! avere una copia sua — due copie della stessa regola sono due regole
//! destinate a divergere — ma non deve nemmeno aver bisogno di Python per
//! farlo, perche' gira **prima** che le dipendenze esistano.

use nova_catalogo::{soglia, verdetto, Famiglia};

#[derive(serde::Deserialize)]
struct Domanda {
    #[serde(default)]
    famiglia: Famiglia,
    #[serde(default)]
    vram_gb: f64,
    #[serde(default)]
    ram_gb: f64,
    #[serde(default)]
    catalogo: serde_json::Value,
}

fn main() {
    let mut dentro = String::new();
    if std::io::Read::read_to_string(&mut std::io::stdin(), &mut dentro).is_err() {
        eprintln!("non riesco a leggere stdin");
        std::process::exit(1);
    }
    // Il BOM. Chi ci parla e' PowerShell, e PowerShell lo mette: un
    // `Set-Content -Encoding UTF8` su Windows PowerShell 5.1 scrive tre byte
    // davanti al primo `{`, e serde si ferma con «expected value at line 1
    // column 1» — che e' vero e non aiuta nessuno. Non e' sciatteria di chi
    // chiama: e' l'ambiente in cui questo binario deve funzionare, ed e'
    // compito suo incontrarlo li'. Lo stesso inciampo era gia' costato tempo
    // dalla parte Python, dove i sorgenti si leggono con `utf-8-sig`.
    let dentro = dentro.strip_prefix('\u{feff}').unwrap_or(&dentro);

    // Una riga sola (l'installatore) oppure molte (il confronto col Python).
    let righe: Vec<&str> = dentro.lines().filter(|r| !r.trim().is_empty()).collect();
    if righe.is_empty() {
        // Nessuna domanda non e' un errore da traceback: e' un verdetto
        // prudente. Chi installa non deve vedere un guasto perche' una
        // rilevazione a monte e' andata a vuoto.
        println!(
            "{}",
            serde_json::json!({
                "si_scarica": false,
                "motivo": "Non ho capito la domanda sul modello.",
                "suggerimento": "Configura il cervello dopo, dalle impostazioni di NOVA.",
                "in_cpu": false,
            })
        );
        return;
    }
    for riga in righe {
        // `unwrap_or_default()` qui era un difetto, e di quelli brutti: una
        // domanda illeggibile diventava una famiglia vuota, e la famiglia
        // vuota produceva un verdetto perfettamente formato — «legge almeno
        // 0.0 GB per token, non te lo faccio scaricare» — che sembra una
        // risposta e non lo e'. Uno strumento che riesce senza consegnare
        // niente e' peggio di uno che manca, perche' produce fiducia mal
        // riposta: l'installatore avrebbe rifiutato ogni modello e detto
        // all'utente una ragione inventata.
        let d: Domanda = match serde_json::from_str(riga) {
            Ok(d) => d,
            Err(e) => {
                println!(
                    "{}",
                    serde_json::json!({
                        "si_scarica": false,
                        "motivo": format!("Non ho capito la domanda sul modello: {e}"),
                        "suggerimento":
                            "Configura il cervello dopo, dalle impostazioni di NOVA.",
                        "in_cpu": false,
                    })
                );
                continue;
            }
        };
        let s = if d.catalogo.is_null() { None } else { Some(soglia(&d.catalogo)) };
        let v = verdetto(&d.famiglia, d.vram_gb, s, d.ram_gb);
        println!("{}", serde_json::to_string(&v).unwrap());
    }
}
