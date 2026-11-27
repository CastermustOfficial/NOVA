//! Cosa mostra uno schermo per un evento del demone.
//!
//! Il demone pubblica su un bus cio' che gli succede. Chi ha uno schermo — la
//! finestra sul PC, e dal D449 il tablet che entra dal portale — ne ricava
//! tre cose: lo stato dell'orb, la riga «sto facendo...», e le righe da
//! scrivere nella chat. Stavano nel guscio (`nova-shell/src/bus.rs`); stanno
//! qui perche' due schermi che le riscrivono ciascuno per conto suo
//! finirebbero per mostrare due NOVA diverse.

use serde_json::Value;

/// Da evento del demone a stato dell'orb. Cio' che non e' qui non tocca
/// l'orb: uno stato che lampeggia per ogni cosa non comunica piu' niente.
pub fn stato_da_evento(topic: &str, dati: &Value) -> Option<&'static str> {
    match topic {
        "stato.cambiato" => match dati.get("stato").and_then(|s| s.as_str()) {
            Some("parlo") => Some("parlo"),
            Some("ascolto") => Some("ascolto"),
            Some("penso") => Some("penso"),
            Some("agisco") => Some("agisco"),
            Some("quiete") => Some("quiete"),
            _ => None,
        },
        // Il supervisor si e' arreso: e' la cosa piu' importante che possa
        // succedere senza che nessuno guardi. Prima finiva in una riga di log
        // dentro la finestra PyQt, cioe' quella che di solito e' chiusa.
        "proc.gave_up" => Some("allarme"),
        "approvazione.richiesta" => Some("chiedo"),
        // Il turno dentro il demone. Finche' girava in un processo a parte
        // questi due non esistevano e l'orb li imparava da `stato.cambiato`;
        // adesso che il turno e' qui dentro, l'orb li sente da chi li vive.
        "agente.stato" => match dati.get("fase").and_then(|f| f.as_str()) {
            // Accendere il modello di casa e' gia' lavoro: chi guarda l'orb
            // deve vedere che NOVA sta facendo qualcosa, non che dorme (D358).
            Some("penso") | Some("modello") => Some("penso"),
            Some("finito") => Some("quiete"),
            _ => None,
        },
        "agente.strumento" => match dati.get("stato").and_then(|s| s.as_str()) {
            Some("inizio") => Some("agisco"),
            // A strumento finito si torna a pensare, non a quiete: il turno
            // non e' finito, sta rileggendo quello che gli e' tornato.
            Some("fine") => Some("penso"),
            _ => None,
        },
        "approvazione.decisa" | "approvazione.scaduta" => Some("quiete"),
        // Sveglia vuol dire che l'orecchio e' aperto sul serio: chi guarda
        // l'orb deve poter capire da li' se puo' parlare senza dire il nome.
        "voce.fase" => match dati.get("fase").and_then(|f| f.as_str()) {
            Some("sveglia") => Some("ascolto"),
            Some("dormiente") | Some("in_pausa") => Some("spento"),
            _ => None,
        },
        _ => None,
    }
}

/// Le righe che il demone porta nella chat da solo, come (chi, testo).
///
/// - Gemini Live (D386): le trascrizioni arrivano a turno finito; un errore
///   si scrive perche' a voce si e' detto solo che Live non risponde, e il
///   perche' (la chiave, la rete) serve leggerlo.
/// - Un Dot che ha finito un compito di Nova (D387): la riga la prepara il
///   demone, con l'esito; a voce, se serve, la dice lui.
/// - Un Dot che scrive a Nova (D388): finche' non c'e' l'harness, la chat e'
///   dove l'utente legge i messaggi dei Dot.
pub fn righe_per_la_chat(topic: &str, dati: &Value) -> Vec<(&'static str, String)> {
    let campo = |n: &str| {
        dati.get(n)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string()
    };
    match topic {
        "voce.live.turno" => [("utente", campo("utente")), ("nova", campo("nova"))]
            .into_iter()
            .filter(|(_, t)| !t.is_empty())
            .collect(),
        "voce.live.errore" if !campo("errore").is_empty() => {
            let come = if dati.get("aperta").and_then(|v| v.as_bool()) == Some(true) {
                "La linea con Gemini Live e' caduta"
            } else {
                "Gemini Live non parte"
            };
            vec![(
                "nova",
                format!("{come} ({}): continuo con la mia voce.", campo("errore")),
            )]
        }
        "dot.consegna" if !campo("chat").is_empty() => vec![("nova", campo("chat"))],
        "dot.messaggio" if campo("a") == "nova" && !campo("testo").is_empty() => {
            vec![(
                "nova",
                format!("Messaggio da {}: {}", campo("da"), campo("testo")),
            )]
        }
        _ => Vec::new(),
    }
}

/// Da evento del demone a riga di stato accanto all'orb.
///
/// E' la stessa cosa che la meta' Python scrive su stderr marcata: «Sto
/// pensando...», «Apro il portale delle offerte». Di uno strumento si
/// preferisce la descrizione al nome — `fs.write` e' per il modello, «Scrive
/// un file» e' per chi legge — e quando la descrizione non c'e' si dice il
/// nome, che e' meglio di niente.
pub fn passo_da_evento(topic: &str, dati: &Value) -> Option<String> {
    match topic {
        "agente.stato" => match dati.get("fase").and_then(|f| f.as_str()) {
            Some("penso") => Some("Sto pensando...".to_string()),
            // Il primo messaggio aspetta che il modello si carichi, e sono
            // decine di secondi: senza questa riga sembrerebbe NOVA bloccata.
            Some("modello") => {
                Some("Accendo il modello locale: la prima volta ci vuole un po'...".to_string())
            }
            Some("finito") => Some(String::new()),
            _ => None,
        },
        "agente.strumento" if dati.get("stato").and_then(|s| s.as_str()) == Some("inizio") => {
            let nome = dati.get("nome").and_then(|n| n.as_str()).unwrap_or("");
            let desc = dati
                .get("descrizione")
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .trim();
            if desc.is_empty() {
                Some(format!("Eseguo {nome}..."))
            } else {
                Some(format!("{desc}..."))
            }
        }
        // `agente.imparato` **non** e' un passo, ed e' una tentazione:
        // arriva a turno finito, quando la risposta e' gia' sullo schermo.
        // Scriverlo accanto all'orb vorrebbe dire riaccendere una riga di
        // stato che nessuno spegnera' piu', perche' dopo non succede altro.
        _ => None,
    }
}

/// Gli argomenti del bus a cui si iscrive uno schermo che non e' la finestra
/// del PC (D449): quello che muove l'orb e quello che si legge in chat. I
/// file che cambiano e il portale stesso riguardano il PC.
pub const ARGOMENTI_DI_UNO_SCHERMO: [&str; 7] = [
    "stato.*",
    "approvazione.*",
    "proc.*",
    "voce.*",
    "ui.chat",
    "azione.*",
    "agente.*",
];

/// Cosa si manda alla pagina di un altro schermo per un evento del demone
/// (D449): il canale, con gli stessi nomi che usa il guscio sul PC, e il suo
/// contenuto. In fondo c'e' sempre l'evento com'e' arrivato, per chi lo vuole
/// leggere da se'.
pub fn per_la_pagina(topic: &str, dati: &Value) -> Vec<(&'static str, Value)> {
    use serde_json::json;
    let mut messaggi = Vec::new();
    if let Some(stato) = stato_da_evento(topic, dati) {
        messaggi.push(("nova://stato", json!({ "stato": stato })));
    }
    if topic == "voce.fase" {
        messaggi.push(("nova://fase", dati.clone()));
    }
    if let Some(testo) = passo_da_evento(topic, dati) {
        messaggi.push(("nova://passo", json!({ "testo": testo })));
    }
    if topic == "ui.chat" {
        let detto = dati
            .get("messaggio")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .trim();
        if !detto.is_empty() {
            messaggi.push(("nova://voce", json!({ "da": "nova", "testo": detto })));
        }
    }
    for (da, testo) in righe_per_la_chat(topic, dati) {
        messaggi.push(("nova://voce", json!({ "da": da, "testo": testo })));
    }
    messaggi.push(("nova://evento", json!({ "topic": topic, "dati": dati })));
    messaggi
}

#[cfg(test)]
mod prove {
    use super::*;
    use serde_json::json;

    #[test]
    fn gemini_live_scrive_nella_chat_i_due_versi_e_gli_errori() {
        let turno = json!({ "utente": " che ore sono ", "nova": "Sono le tre." });
        assert_eq!(
            righe_per_la_chat("voce.live.turno", &turno),
            vec![
                ("utente", "che ore sono".to_string()),
                ("nova", "Sono le tre.".to_string())
            ]
        );
        // Un saluto del modello senza niente detto dall'utente: una riga sola.
        let solo_nova = json!({ "utente": "", "nova": "Ciao!" });
        assert_eq!(
            righe_per_la_chat("voce.live.turno", &solo_nova),
            vec![("nova", "Ciao!".to_string())]
        );
        let errore = json!({ "errore": "manca la chiave", "aperta": false });
        assert_eq!(
            righe_per_la_chat("voce.live.errore", &errore),
            vec![(
                "nova",
                "Gemini Live non parte (manca la chiave): continuo con la mia voce.".to_string()
            )]
        );
        let caduta = json!({ "errore": "rete assente", "aperta": true });
        assert_eq!(
            righe_per_la_chat("voce.live.errore", &caduta),
            vec![(
                "nova",
                "La linea con Gemini Live e' caduta (rete assente): continuo con la mia voce."
                    .to_string()
            )]
        );
        assert!(righe_per_la_chat("voce.live.errore", &json!({})).is_empty());
        assert!(righe_per_la_chat("voce.comando", &turno).is_empty());
    }

    #[test]
    fn la_consegna_di_un_dot_va_nella_chat_come_detta_da_nova() {
        let d = json!({ "dot": "ricercatore", "id": 3, "stato": "fatto",
                        "chat": "ricercatore ha finito.\n\nRapporto in x.md.", "voce": "corta" });
        assert_eq!(
            righe_per_la_chat("dot.consegna", &d),
            vec![(
                "nova",
                "ricercatore ha finito.\n\nRapporto in x.md.".to_string()
            )]
        );
        // Senza parole non c'e' niente da scrivere; e il passaggio di stato
        // di un compito, da solo, nella chat non va.
        assert!(righe_per_la_chat("dot.consegna", &json!({ "chat": " " })).is_empty());
        assert!(righe_per_la_chat("dot.compito", &d).is_empty());
    }

    #[test]
    fn un_dot_che_scrive_a_nova_si_legge_in_chat_gli_altri_messaggi_no() {
        let a_nova = json!({ "da": "uno", "a": "nova", "testo": " Ho finito la bozza. " });
        assert_eq!(
            righe_per_la_chat("dot.messaggio", &a_nova),
            vec![("nova", "Messaggio da uno: Ho finito la bozza.".to_string())]
        );
        let fra_loro = json!({ "da": "uno", "a": "due", "testo": "ciao" });
        assert!(righe_per_la_chat("dot.messaggio", &fra_loro).is_empty());
        assert!(righe_per_la_chat(
            "dot.messaggio",
            &json!({ "da": "uno", "a": "nova", "testo": "" })
        )
        .is_empty());
    }

    #[test]
    fn il_turno_del_demone_muove_l_orb() {
        let f = |fase: &str| stato_da_evento("agente.stato", &json!({ "fase": fase }));
        assert_eq!(f("penso"), Some("penso"));
        assert_eq!(f("finito"), Some("quiete"));
        assert_eq!(f("boh"), None);
        let s = |stato: &str| stato_da_evento("agente.strumento", &json!({ "stato": stato }));
        assert_eq!(s("inizio"), Some("agisco"));
        // Finito uno strumento il turno non e' finito: sta rileggendo.
        assert_eq!(s("fine"), Some("penso"));
    }

    #[test]
    fn di_uno_strumento_si_legge_la_frase_non_il_nome() {
        let d = json!({ "nome": "fs.write", "stato": "inizio", "descrizione": "Scrive un file" });
        assert_eq!(
            passo_da_evento("agente.strumento", &d),
            Some("Scrive un file...".to_string())
        );
    }

    /// Senza descrizione si dice il nome: peggio della frase, molto meglio
    /// di una riga vuota mentre NOVA sta facendo qualcosa.
    #[test]
    fn senza_frase_si_dice_il_nome() {
        let d = json!({ "nome": "fs.write", "stato": "inizio" });
        assert_eq!(
            passo_da_evento("agente.strumento", &d),
            Some("Eseguo fs.write...".to_string())
        );
        let vuota = json!({ "nome": "fs.write", "stato": "inizio", "descrizione": "   " });
        assert_eq!(
            passo_da_evento("agente.strumento", &vuota),
            Some("Eseguo fs.write...".to_string())
        );
    }

    #[test]
    fn a_strumento_finito_non_si_scrive_niente() {
        let d = json!({ "nome": "fs.write", "stato": "fine", "ok": true });
        assert_eq!(passo_da_evento("agente.strumento", &d), None);
    }

    /// A turno finito la riga si **spegne**. Un passo che resta acceso
    /// racconta una cosa che non sta piu' succedendo.
    #[test]
    fn a_turno_finito_la_riga_si_spegne() {
        assert_eq!(
            passo_da_evento("agente.stato", &json!({ "fase": "finito" })),
            Some(String::new())
        );
        assert_eq!(
            passo_da_evento("agente.stato", &json!({ "fase": "penso" })),
            Some("Sto pensando...".to_string())
        );
    }

    /// Cio' che si impara arriva quando la risposta e' gia' letta: se
    /// diventasse un passo, resterebbe acceso per sempre.
    #[test]
    fn quello_che_impara_non_e_un_passo() {
        assert_eq!(
            passo_da_evento("agente.imparato", &json!({ "procedura": "x" })),
            None
        );
    }

    /// D449: un altro schermo riceve gli stessi canali della finestra del PC.
    #[test]
    fn per_la_pagina_di_un_altro_schermo_i_canali_sono_quelli_del_guscio() {
        let canali = |topic: &str, dati: Value| -> Vec<(&'static str, Value)> {
            per_la_pagina(topic, &dati)
        };
        // Lo stato dell'orb e la riga «sto facendo», e in fondo l'evento.
        assert_eq!(
            canali("agente.stato", json!({ "fase": "penso" })),
            vec![
                ("nova://stato", json!({ "stato": "penso" })),
                ("nova://passo", json!({ "testo": "Sto pensando..." })),
                (
                    "nova://evento",
                    json!({ "topic": "agente.stato", "dati": { "fase": "penso" } })
                ),
            ]
        );
        // La fase della voce muove l'orb e va anche da sola.
        let fase = canali("voce.fase", json!({ "fase": "sveglia" }));
        assert_eq!(fase[0], ("nova://stato", json!({ "stato": "ascolto" })));
        assert_eq!(fase[1], ("nova://fase", json!({ "fase": "sveglia" })));
        // Quello che NOVA scrive in chat, e i due versi di Gemini Live.
        assert_eq!(
            canali("ui.chat", json!({ "messaggio": " Fatto. ", "apri": true }))[0],
            ("nova://voce", json!({ "da": "nova", "testo": "Fatto." }))
        );
        assert_eq!(
            canali("ui.chat", json!({ "messaggio": "  " })).len(),
            1,
            "vuoto: solo l'evento"
        );
        let live = canali(
            "voce.live.turno",
            json!({ "utente": "ciao", "nova": "eccomi" }),
        );
        assert_eq!(
            live[0],
            ("nova://voce", json!({ "da": "utente", "testo": "ciao" }))
        );
        assert_eq!(
            live[1],
            ("nova://voce", json!({ "da": "nova", "testo": "eccomi" }))
        );
        // Un evento che non mostra niente arriva lo stesso com'e'.
        assert_eq!(
            canali("cap.called", json!({ "name": "x" })),
            vec![(
                "nova://evento",
                json!({ "topic": "cap.called", "dati": { "name": "x" } })
            )]
        );
        // Lo schermo si iscrive a quello che questi canali leggono.
        for argomento in [
            "stato.cambiato",
            "agente.stato",
            "voce.live.turno",
            "ui.chat",
        ] {
            assert!(
                ARGOMENTI_DI_UNO_SCHERMO
                    .iter()
                    .any(|p| crate::topic_matches(p, argomento)),
                "{argomento}"
            );
        }
    }
}
