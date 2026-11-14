//! La prova di un compito di codice (D445).
//!
//! Deciso con Gio il 13 e il 14 novembre: il piano lo fa il modello forte,
//! il codice lo scrive quello piu' debole. Perche' renda, il piano del
//! forte deve essere fine, e ogni compito di codice deve avere una prova
//! che passa o non passa: il 25 ottobre il revisore ha bocciato 10 passi su
//! 15 fatti da «rapido», e ogni bocciatura si paga tre volte (il passo, la
//! revisione, il rifacimento col cervello grande).
//!
//! - **Nel piano** l'Architetto segna un compito di codice con
//!   `(prova: <cosa deve passare>)`, e lo tiene piccolo: un file o una
//!   funzione, che un cervello piccolo fa in un turno.
//! - **Nel compito** la prova arriva con la riga [`DEL_COMPITO`] e la sua
//!   regola: il Dot scrive la prova, la lancia lui, e chiude la consegna con
//!   una riga `PROVA: passata` o `PROVA: fallita`.
//! - **NOVA non lancia niente** (Gio, il 14 novembre: la prova la fa girare
//!   solo il Dot): legge la riga. Senza riga, o con «fallita», il compito non
//!   e' fatto: si rifa' una volta sullo stesso gradino, poi una volta dal
//!   gradino sopra, e se la prova non passa nemmeno li' il compito risulta
//!   non andato. Il voto del capo resta, sulla consegna con la prova passata.
//!
//! Qui stanno le parole e come si leggono; il giro sta in `nova_core::dot`.

/// Come comincia, in una consegna, la riga che dice com'e' andata la prova.
pub const SEGNO: &str = "PROVA:";

/// Come comincia, nel testo di un compito, la riga con la sua prova. Sta a
/// inizio riga: le consegne dei compiti prima, citate nel testo, sono
/// rientrate, e non la possono imitare.
pub const DEL_COMPITO: &str = "PROVA DEL COMPITO:";

/// Come comincia il segno della prova nella riga di un compito del piano:
/// `(prova: <cosa deve passare>)`.
pub const NEL_PIANO: &str = "(prova:";

/// Quante volte un compito si rifa' perche' la prova non e' passata: la
/// prima sullo stesso gradino, la seconda dal gradino sopra.
pub const RIFACIMENTI: usize = 2;

/// Come comincia l'esito di un compito la cui prova non e' passata.
pub const NON_PASSATA: &str = "la prova del compito non e' passata";

/// Quanto dell'ultima consegna resta nell'esito di un compito non andato.
const CONSEGNA_NELL_ESITO: usize = 1_500;

/// La prova nel testo di un compito, con la sua regola.
pub fn regola(prova: &str) -> String {
    format!(
        "\n{DEL_COMPITO} {}\n\
         Questo compito e' fatto quando la sua prova passa. Scrivila come una prova che si \
         lancia con un comando, lanciala tu, e chiudi la consegna con una riga da sola, fuori \
         dai blocchi di codice e senza apici, che comincia con `{SEGNO} passata` oppure con \
         `{SEGNO} fallita`; sotto, il comando e le ultime righe della sua uscita. Non scrivere \
         «passata» per una prova che non hai lanciato o che non passa: se fallisce, dillo, e \
         di' dove.",
        prova.trim()
    )
}

/// La prova che il testo di un compito chiede, se ne chiede una: la riga
/// [`DEL_COMPITO`], a inizio riga.
pub fn chiesta(testo: &str) -> Option<&str> {
    testo
        .lines()
        .find_map(|r| r.strip_prefix(DEL_COMPITO))
        .map(str::trim)
        .filter(|p| !p.is_empty())
}

/// Com'e' andata la prova, secondo la consegna.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Esito {
    Passata,
    /// Il Dot dice che non passa: quello che ha scritto dopo «fallita».
    Fallita(String),
    /// La consegna non lo dice: la riga non c'e', o non si legge.
    NonDetta,
}

impl Esito {
    /// Perche' la prova non vale come passata, per il Dot che rifa' e per il
    /// diario.
    pub fn perche(&self) -> String {
        match self {
            Esito::Passata => String::new(),
            Esito::Fallita(d) if d.is_empty() => "La consegna dice che la prova e' fallita.".into(),
            Esito::Fallita(d) => format!("La consegna dice che la prova e' fallita: {d}"),
            Esito::NonDetta => format!(
                "La consegna non ha la riga `{SEGNO} passata` o `{SEGNO} fallita`, da sola, \
                 fuori dai blocchi di codice e senza apici."
            ),
        }
    }
}

/// Legge com'e' andata la prova dalla consegna: l'ultima riga che comincia
/// con [`SEGNO`], fuori dai blocchi di codice. Una riga citata (`>`, o fra
/// apici inversi) e' testo, come le altre righe per NOVA (D432). Una riga
/// che dice tutte e due le cose, come la regola ricopiata, non dice niente.
pub fn letta(consegna: &str) -> Esito {
    let mut esito = Esito::NonDetta;
    let mut in_codice = false;
    for riga in consegna.lines() {
        if riga.trim_start().starts_with("```") {
            in_codice = !in_codice;
            continue;
        }
        if in_codice || crate::domande::citata(riga) {
            continue;
        }
        let pulita = crate::domande::pulita(riga);
        let Some(dopo) = crate::domande::dopo(&pulita, SEGNO) else {
            continue;
        };
        let detto = dopo.trim();
        let minuscolo = detto.to_lowercase();
        let passata = minuscolo.contains("passata") && !minuscolo.contains("non passata");
        let fallita = minuscolo.contains("fallita") || minuscolo.contains("non passata");
        esito = match (passata, fallita) {
            (true, false) if minuscolo.starts_with("passata") => Esito::Passata,
            (false, true) => {
                let resto = ["fallita", "non passata"]
                    .iter()
                    .find_map(|p| minuscolo.strip_prefix(p).and_then(|_| detto.get(p.len()..)))
                    .unwrap_or(detto);
                Esito::Fallita(
                    resto
                        .trim_start_matches([':', '.', ',', ';', '-', '—', ' '])
                        .trim()
                        .chars()
                        .take(300)
                        .collect(),
                )
            }
            // Una riga che non si legge non cambia quello che si e' letto.
            _ => continue,
        };
    }
    esito
}

/// Il testo con cui si rifa' un compito la cui prova non e' passata.
pub fn rifai(compito: &str, perche: &str) -> String {
    format!(
        "{}\n\n(Da rifare: la prova del compito non e' passata. {} Quello che hai fatto e' sul \
         disco: correggi, rilancia la prova, e chiudi con la riga `{SEGNO} passata` o `{SEGNO} \
         fallita`.)",
        compito.trim(),
        perche.trim()
    )
}

/// L'esito di un compito la cui prova non e' passata: perche', e l'ultima
/// consegna, che resta da leggere.
pub fn non_passata(perche: &str, consegna: &str) -> String {
    let c = consegna.trim();
    let corta: String = c.chars().take(CONSEGNA_NELL_ESITO).collect();
    let taglio = if c.chars().count() > CONSEGNA_NELL_ESITO {
        "… (tagliato)"
    } else {
        ""
    };
    format!(
        "{NON_PASSATA}. {}\n\nL'ultima consegna:\n{corta}{taglio}",
        perche.trim()
    )
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn la_prova_del_compito_si_trova_solo_a_inizio_riga() {
        let t = format!(
            "Progetto «p», fase 1. Compito 1.2 del piano:\nscrivere il lettore{}\n\n\
             Cio' che hanno consegnato i compiti prima di questo:\n- 1.1 (dati): x\n  \
             {DEL_COMPITO} un'altra, citata",
            regola(" `pytest prove/test_lettore.py` passa ")
        );
        assert_eq!(chiesta(&t), Some("`pytest prove/test_lettore.py` passa"));
        assert!(t.contains("chiudi la consegna con una riga da sola"));
        assert_eq!(chiesta("un compito qualunque"), None);
        assert_eq!(
            chiesta(&format!("x\n  {DEL_COMPITO} rientrata")),
            None,
            "rientrata e' citata"
        );
        assert_eq!(
            chiesta(&format!("x\n{DEL_COMPITO}   ")),
            None,
            "vuota non e' una prova"
        );
    }

    #[test]
    fn la_riga_della_prova_si_legge() {
        assert_eq!(
            letta("Fatto.\nPROVA: passata\n`pytest -q`: 3 passed"),
            Esito::Passata
        );
        assert_eq!(
            letta("- **PROVA: Passata** — pytest"),
            Esito::Passata,
            "grassetto ed elenco"
        );
        assert_eq!(letta("prova: passata."), Esito::Passata, "maiuscole o no");
        assert_eq!(
            letta("PROVA: fallita: test_vuoto, riga 12"),
            Esito::Fallita("test_vuoto, riga 12".into())
        );
        assert_eq!(letta("PROVA: non passata"), Esito::Fallita(String::new()));
        assert_eq!(letta("Ho scritto il codice."), Esito::NonDetta);
        assert_eq!(letta("PROVA: forse"), Esito::NonDetta);
        assert_eq!(
            letta("PROVA: passata\nProva: `pytest -q`, 3 passed"),
            Esito::Passata,
            "il comando sotto"
        );
        // Vale l'ultima: prima fallita, poi corretta e passata; e il contrario.
        assert_eq!(
            letta("PROVA: fallita\nCorretto.\nPROVA: passata"),
            Esito::Passata
        );
        assert!(matches!(
            letta("PROVA: passata\nPoi ho rotto.\nPROVA: fallita"),
            Esito::Fallita(_)
        ));
    }

    /// D432: una riga citata o in un blocco di codice e' testo. E la regola
    /// ricopiata, che dice tutte e due le parole, non dice com'e' andata.
    #[test]
    fn una_riga_citata_o_ricopiata_non_vale() {
        assert_eq!(letta("> PROVA: passata"), Esito::NonDetta);
        assert_eq!(letta("- `PROVA: passata`"), Esito::NonDetta);
        assert_eq!(letta("```\nPROVA: passata\n```"), Esito::NonDetta);
        assert_eq!(
            letta("PROVA: passata oppure PROVA: fallita"),
            Esito::NonDetta
        );
        assert_eq!(
            letta("La riga `PROVA: passata` la scrivo dopo."),
            Esito::NonDetta
        );
        assert_eq!(
            letta("```\nlog\n```\nPROVA: passata"),
            Esito::Passata,
            "dopo il blocco vale"
        );
    }

    #[test]
    fn le_parole_del_rifacimento_e_dell_esito() {
        let r = rifai(" il compito \n", &Esito::NonDetta.perche());
        assert!(
            r.starts_with("il compito\n\n(Da rifare: la prova del compito non e' passata. La consegna non ha la riga")
                && r.ends_with("fallita`.)"),
            "{r}"
        );
        assert_eq!(
            Esito::Fallita("riga 3".into()).perche(),
            "La consegna dice che la prova e' fallita: riga 3"
        );
        let e = non_passata("Perche'.", &"x".repeat(2_000));
        assert!(e.starts_with(
            "la prova del compito non e' passata. Perche'.\n\nL'ultima consegna:\nxxx"
        ));
        assert!(e.ends_with("… (tagliato)"));
        assert!(!non_passata("P.", "corta").contains("tagliato"));
    }
}
