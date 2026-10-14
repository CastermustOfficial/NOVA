//! Le domande dei Dot mentre lavorano (D419).
//!
//! Deciso con Gio il 14 ottobre: «la comunicazione si dimostra fondamentale
//! per non rifare le cose 100 volte». Quel giorno la fase 1 di S0ex1 si e'
//! fermata su un parere del legale che nessuno aveva chiesto: il compito
//! 1.25 lo richiedeva, il Dot che lo faceva non aveva modo di chiederlo, e
//! la prima volta se l'e' scritto da solo. E i Dot che lavorano con una CLI
//! qualunque non hanno gli strumenti di NOVA: la chat del progetto del D416
//! (`dot.scrivi`) per loro non c'era.
//!
//! Adesso un Dot, in qualunque turno di un compito di un progetto, puo'
//! chiudere la risposta con delle righe che NOVA legge, con qualunque
//! cervello:
//!
//! - `CHIEDO A <chi>: <domanda>` — al legale, all'APM o a un collega della
//!   squadra. Quel turno non e' la consegna: NOVA porta la domanda, e la
//!   risposta torna al Dot nello stesso compito. Al massimo
//!   [`GIRI_PER_COMPITO`] volte.
//! - `ALLA CHAT: <messaggio>` — nella chat del progetto, senza aspettare:
//!   gli altri lo leggono al loro prossimo compito, e lo legge Gio.
//!
//! Qui ci sono le parole e come si leggono; chi fa rispondere e chi scrive
//! nella chat e' `nova_core::domande`.

use crate::azienda;

/// Come comincia la riga di una domanda.
pub const CHIEDO: &str = "CHIEDO A";

/// Come comincia la riga di un messaggio alla chat del progetto.
pub const ALLA_CHAT: &str = "ALLA CHAT";

/// Quante volte, in un compito, un Dot puo' fermarsi a chiedere.
pub const GIRI_PER_COMPITO: usize = 3;

/// Quante domande per volta: oltre si risponde alle prime, e lo si dice.
pub const DOMANDE_PER_GIRO: usize = 3;

/// Quanto di una risposta torna al Dot.
pub const RISPOSTA_AL_DOT: usize = 6_000;

/// Quanti gettoni ha chi risponde.
pub const GETTONI: u32 = 1_200;

/// Il segno in testa alla domanda a chi risponde.
pub const SEGNO: &str = "[domanda di un Dot]";

/// Una domanda di un Dot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Domanda {
    /// A chi, come l'ha scritto il Dot: minuscolo, senza virgolette.
    pub a: String,
    pub testo: String,
}

/// Quello che un Dot ha detto in un turno, diviso: le domande, i messaggi
/// alla chat, e il resto, che e' la consegna se non ci sono domande.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Detto {
    pub domande: Vec<Domanda>,
    pub alla_chat: Vec<String>,
    pub resto: String,
}

/// La riga, senza quello che il Markdown ci mette intorno: `**`, un trattino
/// d'elenco, una citazione.
fn pulita(riga: &str) -> String {
    riga.replace("**", "")
        .replace('`', "")
        .trim()
        .trim_start_matches(['-', '*', '>'])
        .trim()
        .to_string()
}

/// Se la riga comincia con `segno` (maiuscole o no), quello che c'e' dopo.
fn dopo<'a>(riga: &'a str, segno: &str) -> Option<&'a str> {
    let testa = riga.get(..segno.len())?;
    testa
        .eq_ignore_ascii_case(segno)
        .then(|| &riga[segno.len()..])
}

/// Divide quello che il Dot ha detto. Le righe dentro un blocco di codice
/// non contano: una consegna che spiega il formato non e' una domanda.
pub fn dividi(t: &str) -> Detto {
    let mut d = Detto::default();
    let mut resto = Vec::new();
    let mut in_codice = false;
    for riga in t.lines() {
        if riga.trim_start().starts_with("```") {
            in_codice = !in_codice;
            resto.push(riga);
            continue;
        }
        if in_codice {
            resto.push(riga);
            continue;
        }
        let p = pulita(riga);
        if let Some(x) = dopo(&p, CHIEDO) {
            // «CHIEDO A legale: …», «CHIEDO AL legale: …», «CHIEDO ALL'APM: …»
            let x = x
                .strip_prefix("LL'")
                .or_else(|| x.strip_prefix("ll'"))
                .or_else(|| x.strip_prefix('L').or_else(|| x.strip_prefix('l')))
                .unwrap_or(x);
            if let Some((a, domanda)) = x.split_once(':') {
                let a = a
                    .trim()
                    .trim_matches(['«', '»', '"', '\'', '<', '>'])
                    .trim()
                    .to_lowercase();
                let domanda = domanda.trim();
                if !a.is_empty() && !domanda.is_empty() {
                    d.domande.push(Domanda {
                        a,
                        testo: domanda.to_string(),
                    });
                    continue;
                }
            }
        }
        if let Some(x) = dopo(&p, ALLA_CHAT) {
            if let Some(m) = x.trim_start().strip_prefix(':') {
                if !m.trim().is_empty() {
                    d.alla_chat.push(m.trim().to_string());
                    continue;
                }
            }
        }
        resto.push(riga);
    }
    d.resto = resto.join("\n").trim().to_string();
    d
}

/// A chi va una domanda.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Chi {
    Legale,
    Apm,
    /// Un collega della squadra del progetto.
    Collega(String),
}

/// A chi va una domanda, fra quelli a cui si puo' chiedere: il legale,
/// l'APM, e la squadra del progetto senza chi chiede. Il nome sbagliato
/// torna un errore che dice a chi si puo'.
pub fn chi(a: &str, chiede: &str, squadra: &[String]) -> Result<Chi, String> {
    let a = a
        .trim()
        .trim_start_matches("il ")
        .trim_start_matches("l'")
        .trim();
    if a == azienda::NOME_LEGALE {
        return Ok(Chi::Legale);
    }
    if a == azienda::NOME_APM {
        return Ok(Chi::Apm);
    }
    if a == chiede {
        return Err("non puoi chiedere a te stesso".into());
    }
    if squadra.iter().any(|s| s == a) {
        return Ok(Chi::Collega(a.to_string()));
    }
    Err(format!(
        "«{a}» non e' fra quelli a cui puoi chiedere: {}",
        a_chi_si_chiede(chiede, squadra)
    ))
}

/// A chi si puo' chiedere, detto in una riga.
pub fn a_chi_si_chiede(chiede: &str, squadra: &[String]) -> String {
    let colleghi: Vec<&str> = squadra
        .iter()
        .map(String::as_str)
        .filter(|s| *s != chiede && *s != azienda::NOME_LEGALE && *s != azienda::NOME_APM)
        .collect();
    if colleghi.is_empty() {
        format!("{}, {}", azienda::NOME_LEGALE, azienda::NOME_APM)
    } else {
        format!(
            "{}, {}, {}",
            azienda::NOME_LEGALE,
            azienda::NOME_APM,
            colleghi.join(", ")
        )
    }
}

/// La regola per chiedere e per scrivere alla chat, in coda al testo di un
/// compito di un progetto.
pub fn regola(chiede: &str, squadra: &[String]) -> String {
    format!(
        "\n\nSe per andare avanti ti serve una risposta che non hai (un parere del legale, una \
         decisione dell'APM, un chiarimento da un collega), non inventarla e non scriverla tu al \
         posto suo: chiedila. Chiudi la risposta con una riga per domanda, cosi': \
         `{CHIEDO} <chi>: <la domanda>`, dove <chi> e' uno fra {a_chi}. Quel turno non e' la \
         consegna: NOVA porta la domanda, la risposta ti torna in questo stesso compito, e \
         allora continui. Puoi farlo al massimo {GIRI_PER_COMPITO} volte per compito. Per dire \
         qualcosa a tutta la squadra senza aspettare, scrivi una riga `{ALLA_CHAT}: <il \
         messaggio>`: va nella chat del progetto, gli altri la leggono al loro prossimo \
         compito, e la legge Gio.",
        a_chi = a_chi_si_chiede(chiede, squadra),
    )
}

/// Taglia un testo a `massimo` caratteri, e lo dice.
pub fn tagliato(t: &str, massimo: usize) -> String {
    let t = t.trim();
    if t.chars().count() <= massimo {
        return t.to_string();
    }
    let corto: String = t.chars().take(massimo).collect();
    format!("{}… (tagliato)", corto.trim_end())
}

/// Quello che serve a chi risponde, oltre alla domanda.
pub struct Contesto<'a> {
    pub progetto: &'a str,
    /// Cosa vuole l'utente.
    pub richiesta: &'a str,
    /// Chi chiede, e a quale compito lavora.
    pub chiede: &'a str,
    pub compito: &'a str,
}

/// La domanda a chi risponde. `chi_e` dice chi e' (il legale, l'APM, un
/// collega col suo ruolo), `sa` quello che sa del progetto: lo stato per
/// l'APM, le sue consegne per un collega.
pub fn domanda_a(chi: &Chi, chi_e: &str, sa: &str, x: &Contesto, domanda: &str) -> String {
    let come = match chi {
        Chi::Legale => {
            "Rispondi da legale: un parere chiaro sulla domanda, con la norma o la licenza che \
             conta. Nel dubbio lo dici, e dici cosa servirebbe per toglierlo. Comincia con una \
             riga `ESITO: ok`, `ESITO: problemi` o `ESITO: non so`, poi il parere."
        }
        Chi::Apm => {
            "Rispondi da APM: decidi se la decisione e' tua. Se e' dell'utente (il budget, le \
             scelte inventive, una spesa) dillo, e non decidere al posto suo."
        }
        Chi::Collega(_) => {
            "Rispondi da collega, con quello che sai dalle tue consegne. Se non lo sai, dillo, \
             e di' dove guarderesti: non inventare."
        }
    };
    let sa = if sa.trim().is_empty() {
        String::new()
    } else {
        format!("\n\n{}", sa.trim())
    };
    format!(
        "{SEGNO} {chi_e}, nell'azienda dei Dot, nel progetto «{progetto}».\n\n\
         Cosa vuole l'utente:\n{richiesta}{sa}\n\n\
         «{chiede}» lavora a questo compito:\n{compito}\n\n\
         E ti chiede:\n{domanda}\n\n{come} Non hai i suoi file, e non ti servono: rispondi alla \
         domanda, in poche righe.",
        progetto = x.progetto,
        richiesta = tagliato(x.richiesta, 2_000),
        chiede = x.chiede,
        compito = tagliato(x.compito, 3_000),
        domanda = domanda.trim(),
    )
}

/// Il messaggio con cui il Dot riprende dopo le risposte. `rimaste` e'
/// quante volte puo' ancora chiedere.
pub fn risposte(r: &[(Domanda, String)], rimaste: usize) -> String {
    let mut t = String::from("Le risposte alle tue domande:");
    for (d, risposta) in r {
        t.push_str(&format!(
            "\n\n- A {}: «{}»\n  Risposta: {}",
            d.a,
            d.testo.trim(),
            tagliato(risposta, RISPOSTA_AL_DOT).replace('\n', "\n  ")
        ));
    }
    t.push_str("\n\nAdesso continua il compito con queste risposte.");
    t.push_str(&if rimaste == 0 {
        " Non puoi piu' chiedere in questo compito: quello che manca ancora scrivilo nella \
         consegna, in «Cosa non so»."
            .to_string()
    } else {
        format!(" Se ti serve, puoi chiedere ancora {rimaste} volte.")
    });
    t
}

/// Quando ha chiesto anche se non poteva piu': si va avanti senza.
pub const BASTA: &str = "Non puoi piu' chiedere in questo compito. Consegna con quello che hai, e \
quello che manca scrivilo in «Cosa non so».";

/// La domanda e la risposta nella chat del progetto, perche' le vedano gli
/// altri e Gio.
pub fn in_chat(compito: &str, d: &Domanda, risposta: &str) -> String {
    format!(
        "Per il compito {compito} ho chiesto a {a}: «{q}»\nRisposta: {r}",
        a = d.a,
        q = d.testo.trim(),
        r = tagliato(risposta, 2_000),
    )
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn le_righe_si_leggono_e_il_resto_resta() {
        let t = "Ho fatto la prima parte.\n\
                 **CHIEDO A legale:** il corpus Gutenberg si puo' usare per il banco?\n\
                 - CHIEDO ALL'APM: chi decide il budget?\n\
                 chiedo al «dati»: dove sono i conteggi di 1.7?\n\
                 ALLA CHAT: ho salvato le fonti in materiale/fonti\n\
                 Fine.";
        let d = dividi(t);
        assert_eq!(
            d.domande,
            vec![
                Domanda {
                    a: "legale".into(),
                    testo: "il corpus Gutenberg si puo' usare per il banco?".into()
                },
                Domanda {
                    a: "apm".into(),
                    testo: "chi decide il budget?".into()
                },
                Domanda {
                    a: "dati".into(),
                    testo: "dove sono i conteggi di 1.7?".into()
                },
            ]
        );
        assert_eq!(
            d.alla_chat,
            vec!["ho salvato le fonti in materiale/fonti".to_string()]
        );
        assert_eq!(d.resto, "Ho fatto la prima parte.\nFine.");
    }

    #[test]
    fn nel_codice_e_senza_domanda_non_e_una_domanda() {
        let t = "Il formato:\n```\nCHIEDO A legale: esempio\nALLA CHAT: esempio\n```\n\
                 CHIEDO A legale:\nCHIEDO A : vuota\nALLA CHAT:\nChiedo a tutti di leggere.";
        let d = dividi(t);
        assert!(d.domande.is_empty(), "{:?}", d.domande);
        assert!(d.alla_chat.is_empty());
        assert_eq!(d.resto, t);
    }

    #[test]
    fn si_chiede_solo_a_chi_c_e() {
        let squadra = vec!["dati".to_string(), "qualita".to_string()];
        assert_eq!(chi("legale", "dati", &squadra), Ok(Chi::Legale));
        assert_eq!(chi("il legale", "dati", &squadra), Ok(Chi::Legale));
        assert_eq!(chi("apm", "dati", &squadra), Ok(Chi::Apm));
        assert_eq!(
            chi("qualita", "dati", &squadra),
            Ok(Chi::Collega("qualita".into()))
        );
        assert!(chi("dati", "dati", &squadra)
            .unwrap_err()
            .contains("te stesso"));
        let e = chi("gio", "dati", &squadra).unwrap_err();
        assert!(e.contains("legale, apm, qualita"), "{e}");
        assert_eq!(a_chi_si_chiede("dati", &[]), "legale, apm");
    }

    #[test]
    fn la_regola_dice_come_e_a_chi() {
        let r = regola("dati", &["dati".into(), "qualita".into()]);
        assert!(r.contains("`CHIEDO A <chi>: <la domanda>`"), "{r}");
        assert!(r.contains("legale, apm, qualita"), "{r}");
        assert!(r.contains("`ALLA CHAT: <il messaggio>`"), "{r}");
        assert!(
            r.contains(&format!("al massimo {GIRI_PER_COMPITO} volte")),
            "{r}"
        );
        // Quello che la regola insegna, dividi lo legge.
        let d = dividi("CHIEDO A qualita: dove sono le misure?\nALLA CHAT: fatto");
        assert_eq!(d.domande.len(), 1);
        assert_eq!(d.alla_chat.len(), 1);
    }

    #[test]
    fn la_domanda_e_le_risposte() {
        let x = Contesto {
            progetto: "s0ex1",
            richiesta: "spazi compressi",
            chiede: "dati",
            compito: "1.25 completare il banco",
        };
        let q = domanda_a(
            &Chi::Legale,
            "Sei il legale",
            "",
            &x,
            "posso scaricare Qwen2-VL?",
        );
        assert!(q.starts_with(SEGNO), "{q}");
        assert!(
            q.contains("«dati» lavora a questo compito:\n1.25 completare il banco"),
            "{q}"
        );
        assert!(q.contains("posso scaricare Qwen2-VL?"), "{q}");
        assert!(q.contains("ESITO: ok"), "{q}");
        let q = domanda_a(&Chi::Apm, "Sei l'APM", "Fase 1 ferma", &x, "chi decide?");
        assert!(
            q.contains("Fase 1 ferma") && q.contains("non decidere al posto suo"),
            "{q}"
        );
        let d = Domanda {
            a: "legale".into(),
            testo: "si puo'?".into(),
        };
        let r = risposte(
            &[(d.clone(), "ESITO: ok\nsi', con licenza Apache".into())],
            2,
        );
        assert!(
            r.contains("- A legale: «si puo'?»\n  Risposta: ESITO: ok\n  si', con licenza Apache"),
            "{r}"
        );
        assert!(r.contains("ancora 2 volte"), "{r}");
        assert!(risposte(&[(d.clone(), "x".into())], 0).contains("Non puoi piu' chiedere"));
        let c = in_chat("1.25", &d, "ESITO: ok");
        assert_eq!(
            c,
            "Per il compito 1.25 ho chiesto a legale: «si puo'?»\nRisposta: ESITO: ok"
        );
        let lunga = "a".repeat(RISPOSTA_AL_DOT + 10);
        assert!(risposte(&[(d, lunga)], 1).contains("… (tagliato)"));
    }
}
