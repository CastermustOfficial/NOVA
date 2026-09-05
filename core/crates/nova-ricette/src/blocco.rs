//! Il testo che le procedure mettono in bocca al modello.
//!
//! Sta qui e non in `nova-contesto` — dove sta il blocco della memoria —
//! perche' ha bisogno dei dati delle procedure, e un testo lontano dal suo
//! dato e' un testo che si aggiorna a meta' (D72). Il **dove** si attacca,
//! invece, e' una domanda di finestra, e la risposta e' la stessa: in coda
//! alla domanda, mai nel prompt di sistema, o si butta via la cache del
//! prefisso a ogni turno.
//!
//! **Il tono e' quello dell'appunto, non dell'ordine.** La procedura e' cio'
//! che ha funzionato l'altra volta, non cio' che va fatto adesso a scatola
//! chiusa: e il testo lo dice esplicitamente, due volte, perche' un modello a
//! cui si consegna una lista di passi la esegue.

/// Una procedura gia' scelta, pronta da raccontare.
///
/// Il punteggio arriva gia' calcolato e l'esistenza di un'automazione arriva
/// da fuori: l'archivio delle automazioni sta su disco, e questo modulo non
/// tocca il disco.
#[derive(Clone, Debug)]
pub struct Proposta {
    pub titolo: String,
    pub procedura: String,
    pub usata: i64,
    pub somiglianza: f64,
    /// Se da questa procedura e' gia' nata un'automazione. Se si', non si
    /// insiste a suggerirla.
    pub ha_automazione: bool,
}

/// Da quante ripetizioni conviene proporre di farne uno strumento.
///
/// Tre volte la stessa strada e' il momento in cui conviene asfaltarla: da
/// qui in poi ogni ripetizione costa un giro di modello per passo, e uno
/// script li farebbe tutti in un turno solo. Il numero nasce dal contatore
/// che c'e' gia', non da un'euristica inventata.
pub const DA_ASFALTARE: i64 = 3;

/// Un numero scritto come lo scrive Python.
///
/// `format!("{}", 1.0_f64)` in Rust da' `1`, `str(1.0)` in Python da' `1.0`.
/// Qui il numero finisce dentro il testo che il modello legge, quindi la
/// differenza non e' estetica: e' un carattere in piu' in cio' che il banco
/// confronta, e un giorno sarebbe stata una divergenza da cercare.
///
/// **Non arrotonda.** L'arrotondamento avviene prima, quando la proposta
/// viene costruita ([`arrotonda2`]), esattamente dove avviene nel Python: e'
/// `proponi` che scrive `round(s, 2)` nel dizionario, non il testo che lo
/// legge. Metterlo qui vorrebbe dire arrotondare due volte — e la prima
/// stesura lo faceva, e il banco l'ha detto subito con un caso a 0,125.
pub fn numero(x: f64) -> String {
    let s = format!("{x}");
    if s.contains('.') || s.contains('e') || s.contains("inf") || s.contains("NaN") {
        s
    } else {
        format!("{s}.0")
    }
}

/// `round(x, 2)` come lo fa Python.
///
/// Non `(x * 100).round() / 100`: quella moltiplicazione introduce un errore
/// che puo' far attraversare la mezza cifra al numero sbagliato. Python
/// scrive il valore binario **esatto** con due decimali, correttamente
/// arrotondato, e poi lo rilegge — che e' esattamente questo.
pub fn arrotonda2(x: f64) -> f64 {
    format!("{x:.2}").parse().unwrap_or(x)
}

/// Il testo da mettere in coda alla domanda, se c'e' qualcosa da dire.
pub fn blocco(trovate: &[Proposta]) -> String {
    if trovate.is_empty() {
        return String::new();
    }
    let mut righe: Vec<String> = vec![
        "\n\n<gia_fatto>".to_string(),
        "Richieste simili le hai gia' risolte. Queste sono PROPOSTE, \
         pescate per somiglianza di parole: possono anche non \
         c'entrare niente. Scarta senza pensarci quelle sbagliate - \
         il numero accanto dice quanto somigliano, non quanto sono \
         giuste."
            .to_string(),
    ];
    for r in trovate {
        let quante_volte = if r.usata > 1 {
            format!(", gia' fatto {} volte", r.usata)
        } else {
            String::new()
        };
        let titolo = if r.titolo.is_empty() { "senza titolo" } else { &r.titolo };
        righe.push(format!(
            "\n[{titolo}]  (somiglianza {}{quante_volte})",
            numero(r.somiglianza)
        ));
        righe.push(r.procedura.trim().to_string());
    }
    for r in trovate {
        if r.usata >= DA_ASFALTARE && !r.ha_automazione {
            righe.push(format!(
                "\nQuesta l'hai gia' fatta {} volte sempre allo \
                 stesso modo. Se i passi sono stabili, conviene trasformarla in \
                 un'automazione con automazione_crea: diventa uno strumento solo, \
                 e la prossima volta e' un turno invece di dieci. Se invece i \
                 passi cambiano ogni volta, lascia stare.",
                r.usata
            ));
            break;
        }
    }
    righe.push(
        "\nSe la richiesta e' la stessa, rifalla cosi' senza ricominciare a \
         cercare. Se qualcosa non torna piu' — un percorso cambiato, uno \
         strumento che non risponde — adattati e non insistere sulla vecchia \
         strada: quello che sai e' come e' andata l'altra volta, non come deve \
         andare oggi."
            .to_string(),
    );
    righe.push("</gia_fatto>".to_string());
    righe.join("\n")
}

#[cfg(test)]
mod prove {
    use super::*;

    fn p(titolo: &str, usata: i64, s: f64, auto: bool) -> Proposta {
        Proposta {
            titolo: titolo.to_string(),
            procedura: format!("  passi di {titolo}  "),
            usata,
            somiglianza: s,
            ha_automazione: auto,
        }
    }

    #[test]
    fn niente_procedure_niente_blocco() {
        assert_eq!(blocco(&[]), "");
    }

    #[test]
    fn i_numeri_si_scrivono_come_in_python() {
        assert_eq!(numero(1.0), "1.0");
        assert_eq!(numero(0.5), "0.5");
        assert_eq!(numero(0.0), "0.0");
        assert_eq!(numero(0.42), "0.42");
        assert_eq!(numero(0.4), "0.4");
        // Non arrotonda: chi scrive non e' chi decide quante cifre tenere.
        assert_eq!(numero(0.125), "0.125");
        assert_eq!(numero(0.3333333333), "0.3333333333");
    }

    #[test]
    fn larrotondamento_sta_dove_sta_in_python() {
        assert_eq!(arrotonda2(0.125), 0.12);
        assert_eq!(arrotonda2(0.4266), 0.43);
        assert_eq!(arrotonda2(1.0), 1.0);
        assert_eq!(numero(arrotonda2(1.0)), "1.0");
        assert_eq!(numero(arrotonda2(0.3333333333)), "0.33");
    }

    #[test]
    fn una_sola_volta_non_si_conta() {
        let b = blocco(&[p("aprire il vault", 1, 0.4, false)]);
        assert!(b.contains("(somiglianza 0.4)"), "{b}");
        assert!(!b.contains("gia' fatto"), "{b}");
    }

    #[test]
    fn i_passi_si_ripuliscono_ai_bordi() {
        let b = blocco(&[p("x", 1, 0.4, false)]);
        assert!(b.contains("\npassi di x\n"), "{b}");
    }

    #[test]
    fn tre_volte_si_propone_di_asfaltarla() {
        let b = blocco(&[p("x", 3, 0.4, false)]);
        assert!(b.contains("automazione_crea"));
        assert!(b.contains("gia' fatta 3 volte"));
    }

    #[test]
    fn ma_non_se_lautomazione_esiste_gia() {
        let b = blocco(&[p("x", 9, 0.4, true)]);
        assert!(!b.contains("automazione_crea"));
    }

    #[test]
    fn il_suggerimento_si_da_una_volta_sola() {
        let b = blocco(&[p("x", 3, 0.4, false), p("y", 5, 0.3, false)]);
        assert_eq!(b.matches("automazione_crea").count(), 1);
    }

    #[test]
    fn il_tono_resta_quello_dellappunto() {
        // Due volte, apposta: un modello a cui si consegna una lista di passi
        // la esegue, e queste sono proposte pescate per somiglianza.
        let b = blocco(&[p("x", 1, 0.4, false)]);
        assert!(b.contains("PROPOSTE"));
        assert!(b.contains("non come deve \nandare oggi") || b.contains("non come deve andare oggi"));
    }
}
