//! Una notifica, dalla riga di comando.
//!
//! ```text
//! nova-notifica "Titolo" "Messaggio"       mostra per 8 secondi
//! nova-notifica "Titolo" "Messaggio" 3000  mostra per 3 secondi
//! nova-notifica --da-file promemoria.txt    titolo sulla prima riga, poi il testo
//! ```
//!
//! Questo programma **aspetta**, e deve aspettare: il fumetto dell'area di
//! notifica muore insieme a chi possiede l'icona. Il punto e' che ad
//! aspettare sia lui e non NOVA, che lo lancia e se ne va (D130, D134).

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.is_empty() {
        eprintln!("uso: nova-notifica <titolo> <messaggio> [millisecondi]");
        eprintln!("     nova-notifica --da-file <percorso>");
        std::process::exit(2);
    }
    // Il testo da un file, e non dalla riga di comando.
    //
    // Serve ai promemoria dell'Utilita' di pianificazione: li' il comando
    // finisce dentro un XML, e prima ancora dentro la riga di comando che
    // Windows ricostruisce all'orario giusto. Ogni passaggio e' un posto dove
    // una virgoletta nel messaggio dell'utente puo' rompere tutto — e nel
    // vecchio `create_reminder` lo rompeva **sempre**, anche senza virgolette
    // (D146). Con un file, nella riga di comando finisce solo un percorso che
    // abbiamo scritto noi.
    //
    // Prima riga il titolo, il resto il messaggio.
    let (titolo, messaggio, durata_file);
    if a[0] == "--da-file" {
        let Some(percorso) = a.get(1) else {
            eprintln!("«--da-file» vuole un percorso");
            std::process::exit(2);
        };
        let dentro = match std::fs::read_to_string(percorso) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("non riesco a leggere «{percorso}»: {e}");
                std::process::exit(1);
            }
        };
        let mut righe = dentro.splitn(2, '\n');
        titolo = righe.next().unwrap_or("NOVA").trim_end_matches('\r').to_string();
        messaggio = righe.next().unwrap_or("").trim_end_matches('\n').to_string();
        durata_file = a.get(2).and_then(|s| s.parse::<u32>().ok());
        let durata = durata_file.unwrap_or(20_000).clamp(1000, 30_000);
        if let Err(e) = nova_platform::notifiche::mostra(&titolo, &messaggio, durata) {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return;
    }
    let titolo = a[0].as_str();
    let messaggio = a.get(1).map(String::as_str).unwrap_or("");
    let durata = a
        .get(2)
        .and_then(|s| s.parse::<u32>().ok())
        // Otto secondi e' quanto durava prima: si cambia il costo, non
        // l'aspetto. Il minimo di Windows e' comunque piu' alto.
        .unwrap_or(8000)
        .clamp(1000, 30_000);
    if let Err(e) = nova_platform::notifiche::mostra(titolo, messaggio, durata) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
