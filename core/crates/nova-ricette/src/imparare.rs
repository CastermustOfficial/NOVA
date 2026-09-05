//! Come NOVA impara una procedura, e quando decide di non impararla.
//!
//! Il materiale glielo si **passa**, non glielo si chiede: `semplice()` e' una
//! chiamata isolata, senza sessione e senza memoria del turno appena finito.
//! Chiedergli «cosa hai fatto?» era chiedere a chi non c'era — rispondeva
//! NIENTE ogni volta, e l'archivio restava vuoto senza che nessun errore lo
//! dicesse.
//!
//! Qui non c'e' il modello e non c'e' il disco: c'e' il testo che gli si
//! manda, la decisione se valga la pena mandarlo, e la lettura di cio' che
//! risponde. Tre cose che decidono **cosa NOVA impara**, e che in Python
//! stavano dentro un metodo di novanta righe insieme alla rete e al filo.

/// Quanto della richiesta e della risposta si passa al modello.
///
/// Sono **caratteri**, non byte: in italiano non e' la stessa cosa, e tagliare
/// a byte spezzerebbe una lettera accentata a meta' (D152).
pub const QUANTA_DOMANDA: usize = 300;
pub const QUANTA_RISPOSTA: usize = 900;

/// Sotto questa lunghezza i passi non sono passi.
pub const MINIMO_PASSI: usize = 20;
/// E il titolo si taglia qui.
pub const MASSIMO_TITOLO: usize = 60;

/// Il testo con cui si chiede al modello di ricostruire la procedura.
///
/// E' un prompt, quindi e' codice: una parola diversa e' un comportamento
/// diverso, e qui il comportamento e' cosa NOVA impara. Estratto dal Python,
/// non ricopiato, e il banco lo confronta carattere per carattere (D158).
pub const RICHIESTA: &str = concat!(
    "Ecco uno scambio appena avvenuto fra un utente e un ",
    "assistente che ha le mani sul suo PC.\n\n",
    "RICHIESTA: \"{domanda}\"\n\n",
    "RISPOSTA DATA: \"{risposta}\"\n\n",
    "{strumenti}",
    "Ricostruisci da questo la procedura, perche' la ",
    "prossima volta si possa rifare senza cercare.\n",
    "- prima riga: un titolo di tre o quattro parole;\n",
    "- poi al massimo sei righe numerate, concrete: quali ",
    "strumenti, quali comandi, quali percorsi, in che ordine;\n",
    "- NON scrivere i risultati (numeri, nomi, contenuti ",
    "trovati): quelli cambiano. Solo i passi.\n",
    "- ultima riga, che comincia con «ALTRE PAROLE:»: sei o ",
    "sette modi DIVERSI in cui la stessa cosa si sarebbe ",
    "potuta chiedere, separati da virgola. Sinonimi veri, ",
    "anche in inglese e anche gergali - per «controlla la ",
    "posta»: inbox, email, messaggi, mail, casella, ",
    "corrispondenza. Servono a ritrovare questa procedura ",
    "quando la richiesta sara' scritta con altre parole.\n",
    "Se dallo scambio non si capisce nessuna procedura ripetibile, ",
    "rispondi soltanto: NIENTE",
);

fn primi(testo: &str, quanti: usize) -> String {
    testo.chars().take(quanti).collect()
}

/// Il prompt, con dentro lo scambio appena avvenuto.
pub fn richiesta(domanda: &str, risposta: &str, strumenti: &[String]) -> String {
    let quali = strumenti.join(", ");
    let blocco = if quali.is_empty() {
        String::new()
    } else {
        format!("STRUMENTI USATI: {quali}\n\n")
    };
    RICHIESTA
        .replace("{domanda}", &primi(domanda, QUANTA_DOMANDA))
        .replace("{risposta}", &primi(risposta, QUANTA_RISPOSTA))
        .replace("{strumenti}", &blocco)
}

/// Perche' questo turno non si impara.
///
/// Ogni motivo e' distinto apposta: sono i punti in cui NOVA smette di
/// imparare, e finiscono in `procedure.log`. Esistono perche' «l'archivio e'
/// vuoto» aveva tre cause diverse con lo stesso identico sintomo — il filo
/// non partito, il modello che diceva NIENTE, e qualcosa che esplodeva in
/// silenzio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NonSiRegistra {
    Spente,
    SottoLaSoglia { secondi: i64, soglia: i64 },
    NessunoStrumento,
}

/// Se questo turno vale la pena di essere imparato.
pub fn si_registra(
    attive: bool,
    secondi: f64,
    soglia: i64,
    agentico: bool,
    quanti_strumenti: usize,
) -> Result<(), NonSiRegistra> {
    if !attive {
        return Err(NonSiRegistra::Spente);
    }
    if secondi < soglia as f64 {
        // Come `f"{secondi:.0f}"` in Python: arrotondamento al pari.
        return Err(NonSiRegistra::SottoLaSoglia {
            secondi: arrotonda_al_pari(secondi),
            soglia,
        });
    }
    if !agentico && quanti_strumenti == 0 {
        return Err(NonSiRegistra::NessunoStrumento);
    }
    Ok(())
}

/// `f"{x:.0f}"` di Python: mezzo si arrotonda al **pari**, non per eccesso.
pub fn arrotonda_al_pari(x: f64) -> i64 {
    format!("{x:.0}").parse().unwrap_or(x as i64)
}

/// Perche' la risposta del modello non e' diventata una procedura.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NonSiLegge {
    NienteRisposta,
    DiceNiente,
    TroppoCorta { testo: String },
    PassiScarni { passi: String },
}

/// Una procedura ricostruita.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Letta {
    pub titolo: String,
    pub procedura: String,
    pub alias: Vec<String>,
}

/// Le righe come le separa Python.
///
/// `str.splitlines()` non taglia solo su `\n`: taglia anche su `\r`, `\r\n`,
/// `\v`, `\f`, i separatori `\x1c`-`\x1e`, `\x85`, e su `\u{2028}`/`\u{2029}`.
/// `str::lines()` di Rust taglia **solo** su `\n`. Non e' pedanteria: qui si
/// legge il testo che ha scritto un modello, e un modello puo' scrivere
/// qualunque cosa — se le righe si contano diversamente, «risposta troppo
/// corta» scatta da una parte e non dall'altra.
pub fn righe(testo: &str) -> Vec<String> {
    fn e_a_capo(c: char) -> bool {
        matches!(c, '\n' | '\r' | '\u{b}' | '\u{c}' | '\u{1c}' | '\u{1d}'
                    | '\u{1e}' | '\u{85}' | '\u{2028}' | '\u{2029}')
    }
    let mut fuori: Vec<String> = Vec::new();
    let mut corrente = String::new();
    let caratteri: Vec<char> = testo.chars().collect();
    let mut i = 0;
    while i < caratteri.len() {
        let c = caratteri[i];
        if e_a_capo(c) {
            fuori.push(std::mem::take(&mut corrente));
            // `\r\n` e' un a capo solo.
            if c == '\r' && i + 1 < caratteri.len() && caratteri[i + 1] == '\n' {
                i += 1;
            }
        } else {
            corrente.push(c);
        }
        i += 1;
    }
    if !corrente.is_empty() {
        fuori.push(corrente);
    }
    fuori
}

/// Quello che il modello ha risposto, letto come procedura.
pub fn leggi(testo: &str) -> Result<Letta, NonSiLegge> {
    let testo = testo.trim();
    if testo.is_empty() {
        return Err(NonSiLegge::NienteRisposta);
    }
    if testo.to_uppercase().starts_with("NIENTE") {
        return Err(NonSiLegge::DiceNiente);
    }
    let mut righe_utili: Vec<String> =
        righe(testo).into_iter().filter(|r| !r.trim().is_empty()).collect();
    if righe_utili.len() < 2 {
        return Err(NonSiLegge::TroppoCorta { testo: primi(testo, 80) });
    }
    let titolo: String = primi(
        righe_utili[0].trim_matches(|c| c == ' ' || c == '#' || c == '*' || c == '-').trim(),
        MASSIMO_TITOLO,
    );
    let mut alias: Vec<String> = Vec::new();
    for i in 0..righe_utili.len() {
        if righe_utili[i].trim().to_uppercase().starts_with("ALTRE PAROLE") {
            let dopo = righe_utili[i]
                .split_once(':')
                .map(|(_, d)| d.to_string())
                .unwrap_or_else(|| righe_utili[i].clone());
            alias = dopo
                .split(',')
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect();
            righe_utili.truncate(i);
            break;
        }
    }
    let procedura = righe_utili
        .iter()
        .skip(1)
        .cloned()
        .collect::<Vec<String>>()
        .join("\n")
        .trim()
        .to_string();
    if procedura.chars().count() < MINIMO_PASSI {
        return Err(NonSiLegge::PassiScarni { passi: primi(&procedura, 80) });
    }
    Ok(Letta { titolo, procedura, alias })
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn il_prompt_ha_i_tre_segnaposto_e_nessun_altro() {
        let p = richiesta("chiesto", "risposto", &["a".into(), "b".into()]);
        assert!(!p.contains("{domanda}") && !p.contains("{risposta}")
                && !p.contains("{strumenti}"));
        assert!(p.contains("STRUMENTI USATI: a, b\n\n"));
        let senza = richiesta("x", "y", &[]);
        assert!(!senza.contains("STRUMENTI USATI"));
    }

    #[test]
    fn il_materiale_si_taglia_a_caratteri() {
        let lunga = "à".repeat(500);
        let p = richiesta(&lunga, "", &[]);
        // 300 caratteri, non 300 byte.
        assert!(p.contains(&"à".repeat(300)));
        assert!(!p.contains(&"à".repeat(301)));
    }

    #[test]
    fn quando_non_si_registra() {
        assert_eq!(si_registra(false, 100.0, 8, true, 3), Err(NonSiRegistra::Spente));
        assert_eq!(si_registra(true, 3.0, 8, true, 3),
                   Err(NonSiRegistra::SottoLaSoglia { secondi: 3, soglia: 8 }));
        assert_eq!(si_registra(true, 100.0, 8, false, 0),
                   Err(NonSiRegistra::NessunoStrumento));
        // Un cervello agentico non passa di qui con gli strumenti: le sue
        // chiamate le fa per conto suo, quindi zero strumenti non vuol dire
        // che non abbia fatto niente.
        assert_eq!(si_registra(true, 100.0, 8, true, 0), Ok(()));
    }

    #[test]
    fn i_secondi_si_scrivono_come_li_scrive_python() {
        assert_eq!(arrotonda_al_pari(3.4), 3);
        assert_eq!(arrotonda_al_pari(3.5), 4);
        assert_eq!(arrotonda_al_pari(2.5), 2); // al pari, non per eccesso
        assert_eq!(arrotonda_al_pari(0.5), 0);
    }

    #[test]
    fn una_risposta_buona() {
        let r = leggi("Aprire il vault\n1. apri Obsidian\n2. cerca la nota\n\
                       ALTRE PAROLE: vault, note, obsidian").unwrap();
        assert_eq!(r.titolo, "Aprire il vault");
        assert_eq!(r.procedura, "1. apri Obsidian\n2. cerca la nota");
        assert_eq!(r.alias, vec!["vault", "note", "obsidian"]);
    }

    #[test]
    fn i_rifiuti_hanno_nomi_diversi() {
        assert_eq!(leggi(""), Err(NonSiLegge::NienteRisposta));
        assert_eq!(leggi("   \n  "), Err(NonSiLegge::NienteRisposta));
        assert_eq!(leggi("NIENTE"), Err(NonSiLegge::DiceNiente));
        assert_eq!(leggi("niente di ripetibile"), Err(NonSiLegge::DiceNiente));
        assert!(matches!(leggi("solo un titolo"), Err(NonSiLegge::TroppoCorta { .. })));
        assert!(matches!(leggi("Titolo\n1. x"), Err(NonSiLegge::PassiScarni { .. })));
    }

    #[test]
    fn il_titolo_si_ripulisce_e_si_taglia() {
        let r = leggi("## **Titolo con i fronzoli** --\n1. un passo abbastanza lungo\n\
                       2. un altro").unwrap();
        // `strip(" #*-")` toglie **tutti** quei caratteri dai bordi, asterischi
        // compresi: quindi il grassetto del markdown sparisce insieme ai cancelletti.
        assert_eq!(r.titolo, "Titolo con i fronzoli");
        let lungo = format!("{}\n1. un passo abbastanza lungo da bastare", "t".repeat(200));
        assert_eq!(leggi(&lungo).unwrap().titolo.chars().count(), MASSIMO_TITOLO);
    }

    #[test]
    fn le_righe_si_contano_come_in_python() {
        // `\u{2028}` e' un a capo per Python e non per Rust: senza questa
        // riga, «risposta troppo corta» scatterebbe da una parte sola.
        assert_eq!(righe("a\u{2028}b").len(), 2);
        assert_eq!(righe("a\r\nb").len(), 2);
        assert_eq!(righe("a\rb").len(), 2);
        assert_eq!(righe("a\u{b}b").len(), 2);
        assert_eq!(righe("a\nb\n").len(), 2);
        assert_eq!(righe("").len(), 0);
    }

    #[test]
    fn senza_altre_parole_gli_alias_restano_vuoti() {
        let r = leggi("Titolo\n1. un passo abbastanza lungo da contare").unwrap();
        assert!(r.alias.is_empty());
        assert_eq!(r.procedura, "1. un passo abbastanza lungo da contare");
    }
}
