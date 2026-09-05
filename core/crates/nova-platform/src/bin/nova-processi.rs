//! I processi: elencarli, o chiuderne uno per pid.
//!
//! ```text
//! nova-processi                 elenca tutto, in JSON
//! nova-processi --chiudi 1234   chiede alle finestre di quel processo di chiudersi
//! nova-processi --chiudi 1234 --forza   lo ferma dov'e'
//! nova-processi --avvia chrome          lo avvia come farebbe il menu Start
//! nova-processi --avvia chrome "--x"    con degli argomenti
//! ```
//!
//! **Non c'e' un modo di chiudere per nome, ed e' voluto.** Dall'altra parte
//! si passa un testo che finisce dentro un `-like` di PowerShell: misurato,
//! `*` selezionava 292 processi. Qui si elenca, si guarda, e si chiude un pid
//! alla volta — un numero non ha caratteri jolly (D141).

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.first().map(String::as_str) == Some("--chiudi") {
        let Some(pid) = a.get(1).and_then(|s| s.parse::<u32>().ok()) else {
            eprintln!("«--chiudi» vuole un pid, cioe' un numero");
            std::process::exit(2);
        };
        let forza = a.iter().any(|x| x == "--forza");
        if let Err(e) = nova_platform::processi::chiudi(pid, forza) {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return;
    }
    if a.first().map(String::as_str) == Some("--avvia") {
        let Some(bersaglio) = a.get(1) else {
            eprintln!("«--avvia» vuole il nome o il percorso di un programma");
            std::process::exit(2);
        };
        // Gli argomenti del programma stanno in **un** parametro e non in
        // coda: cosi' nessuno deve indovinare dove finiscono i nostri e dove
        // cominciano i suoi.
        let argomenti = a.get(2).cloned().unwrap_or_default();
        if let Err(e) = nova_platform::processi::avvia(bersaglio, &argomenti) {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return;
    }
    match nova_platform::processi::elenca() {
        Ok(p) => match serde_json::to_string(&p) {
            Ok(j) => println!("{j}"),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        },
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
