//! Un agente esterno pilotato per riga di comando.
//!
//! Claude Code ha un modulo suo perche' ha sessioni, permessi e MCP. Qui c'e'
//! il caso generale: un binario che prende un prompt e restituisce testo, e
//! che si aggiunge **dalla configurazione**, senza scrivere codice nuovo.
//!
//! Non ha sessione: la continuita' gliela da' NOVA, riscrivendogli ogni volta
//! gli ultimi scambi.

use crate::{istruzioni, Messaggio};
use serde_json::Value;

/// Quanti scambi si riscrivono a una CLI che non ha sessione.
pub const ULTIMI_SCAMBI: usize = 6;

/// Quanto si aspetta una CLI, se la dichiarazione non lo dice.
///
/// Dieci minuti sembrano tanti e non lo sono: una CLI agentica **agisce**,
/// e un tetto stretto non protegge da niente — taglia a meta' un lavoro
/// gia' cominciato sul computer dell'utente.
pub const SECONDI_PREDEFINITI: u64 = 600;

/// Com'e' dichiarata una CLI in `brains.cli`.
///
/// E' cio' che serve per **lanciarla**, e non una copia del pezzo di file:
/// i valori di ripiego si applicano qui, una volta sola, invece che a ogni
/// domanda fatta al dizionario. Chi la lancia non deve sapere che
/// `"prompt"` puo' mancare, ne' che quando manca vuol dire stdin.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Dichiarata {
    /// Il nome sotto cui e' scritta: e' anche cio' che si dice all'utente
    /// quando gli si chiede di toglierla.
    pub nome: String,
    pub etichetta: String,
    pub binario: String,
    pub args: Vec<String>,
    pub modello: String,
    /// Il prompt va su stdin (vero) o in coda alla riga di comando (falso).
    pub su_stdin: bool,
    pub secondi: u64,
    /// Da dove la si lancia. Vuoto vuol dire «la cartella dell'utente», e
    /// quale sia non e' una cosa che si sa senza guardare la macchina.
    pub cartella: String,
    /// Se ogni chiamata fa spendere davvero lo sa l'utente, non NOVA.
    pub a_consumo: bool,
}

/// Come Python scrive un nome con l'iniziale grande.
///
/// `str.capitalize()` **abbassa il resto**: «GLM» diventa «Glm». Non e' una
/// bellezza, ma e' cio' che l'utente vede gia' oggi nel pannello, e due
/// etichette diverse per lo stesso cervello sono due cervelli finche' non
/// si guarda meglio.
pub fn iniziale_grande(nome: &str) -> String {
    let mut c = nome.chars();
    match c.next() {
        None => String::new(),
        Some(primo) => primo.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
    }
}

/// La dichiarazione letta dal pezzo di configurazione, coi ripieghi applicati.
///
/// Un pezzo che non c'e' (`Value::Null`) non e' un errore: e' una CLI che si
/// chiama come il suo binario e non vuole argomenti. E' il caso di chi
/// scrive `"cli": {"gemini": {}}`, ed e' anche cio' che tiene in piedi un
/// gradino il cui `brain` non e' piu' dichiarato — lanciare `gemini` e
/// sbagliare e' piu' onesto che far finta che il gradino non esista.
pub fn dichiarata(nome: &str, spec: &Value) -> Dichiarata {
    let testo = |k: &str| {
        spec.get(k)
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string()
    };
    let binario = {
        let b = testo("binary");
        if b.is_empty() {
            nome.trim().to_string()
        } else {
            b
        }
    };
    let etichetta = {
        let e = testo("etichetta");
        if e.is_empty() {
            iniziale_grande(nome.trim())
        } else {
            e
        }
    };
    Dichiarata {
        nome: nome.trim().to_string(),
        etichetta,
        binario,
        args: spec
            .get("args")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|x| match x {
                        Value::String(s) => s.clone(),
                        altro => altro.to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        modello: testo("model"),
        // Il valore di ripiego e' stdin, ed e' quello giusto: una riga di
        // comando ha un tetto — 8191 caratteri su Windows — e un prompt lo
        // supera senza avvisare.
        su_stdin: {
            let p = testo("prompt").to_lowercase();
            p.is_empty() || p == "stdin"
        },
        secondi: spec
            .get("timeout")
            .and_then(Value::as_u64)
            .filter(|s| *s > 0)
            .unwrap_or(SECONDI_PREDEFINITI),
        cartella: testo("cwd"),
        a_consumo: spec
            .get("a_consumo")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

/// I nomi da provare nel PATH, in quest'ordine.
///
/// Su Windows npm installa un `.cmd`: e' quello che si puo' eseguire, e va
/// cercato **per primo** — cercare prima il nome nudo trova il file dello
/// script, che Windows non sa avviare.
pub fn candidati(binario: &str) -> Vec<String> {
    if binario.is_empty() {
        return Vec::new();
    }
    [".cmd", ".exe", ""]
        .iter()
        .map(|s| format!("{binario}{s}"))
        .collect()
}

/// Il prompt intero per una CLI senza sessione.
pub fn prompt_completo(messaggi: &[Messaggio], contesto_kb: &str) -> String {
    let mut pezzi: Vec<String> = Vec::new();
    let sistema = istruzioni(messaggi);
    if !sistema.is_empty() {
        pezzi.push(sistema);
    }
    if !contesto_kb.is_empty() {
        pezzi.push(format!("Quello che sai gia' dell'utente:\n{contesto_kb}"));
    }
    let scambi: Vec<&Messaggio> = messaggi
        .iter()
        .filter(|m| m.ruolo == "user" || m.ruolo == "assistant")
        .collect();
    let da = scambi.len().saturating_sub(ULTIMI_SCAMBI);
    for m in &scambi[da..] {
        let chi = if m.ruolo == "user" { "UTENTE" } else { "ASSISTENTE" };
        let testo = m.contenuto.trim();
        if !testo.is_empty() {
            pezzi.push(format!("{chi}: {testo}"));
        }
    }
    pezzi.join("\n\n")
}

/// La riga di comando, col nome del modello sostituito dove serve.
pub fn argomenti(eseguibile: &str, args: &[String], model: &str) -> Vec<String> {
    let mut fuori = vec![eseguibile.to_string()];
    fuori.extend(args.iter().map(|a| a.replace("{model}", model)));
    fuori
}

/// Perche' questa CLI non e' pronta, se non lo e'.
pub fn perche_non_pronto(eseguibile: &str, binario: &str, nome: &str) -> Option<String> {
    if eseguibile.is_empty() {
        // «Installalo» da solo e' fuorviante in un caso che capita spesso: il
        // programma **e' installato**, e si sta guardando un PATH vecchio. Un
        // processo eredita le variabili d'ambiente da chi lo ha avviato,
        // quindi una CLI installata mentre NOVA gira resta invisibile finche'
        // NOVA non riparte - e il messaggio manda a reinstallare una cosa che
        // c'e' gia' (D193). Capitato l'11 settembre con Antigravity CLI:
        // installata alle 13:34, NOVA in piedi dalle 13:14.
        return Some(format!(
            "«{binario}» non trovato nel PATH. Se l'hai appena installato, \
             riavvia NOVA: eredita il PATH da quando e' partita. Altrimenti \
             installalo, oppure togli «{nome}» da brains.cli."
        ));
    }
    None
}

/// Cosa ha detto la CLI, o perche' non si puo' dire che abbia risposto.
///
/// Uscita vuota vuol dire **guasto**, non risposta vuota: una CLI che non
/// ha niente da dire scrive comunque qualcosa. E allora l'unica riga che
/// spiega qualcosa sta su stderr, e va riportata — buttarla via e' il modo
/// in cui si finisce con un messaggio d'errore che non dice niente.
pub fn cosa_ha_detto(etichetta: &str, stdout: &str, stderr: &str) -> Result<String, String> {
    let uscita = nova_pitone::senza_bianchi(stdout).to_string();
    if !uscita.is_empty() {
        return Ok(uscita);
    }
    let e = nova_pitone::senza_bianchi(stderr);
    let e: String = e.chars().take(400).collect();
    Err(format!("{etichetta} non ha prodotto output. {e}"))
}

/// Quando la CLI non risponde entro il suo tempo.
pub fn non_ha_risposto(etichetta: &str, secondi: u64) -> String {
    format!("{etichetta} non ha risposto entro {secondi}s")
}

/// Come si racconta lo stato di questa CLI.
pub fn descrizione_stato(etichetta: &str, model: &str) -> String {
    format!(
        "{etichetta}: {}",
        if model.is_empty() { "modello predefinito" } else { model }
    )
}

#[cfg(test)]
mod prove {
    use super::*;

    fn m(r: &str, c: &str) -> Messaggio {
        Messaggio { ruolo: r.into(), contenuto: c.into() }
    }

    #[test]
    fn su_windows_il_cmd_si_cerca_per_primo() {
        assert_eq!(candidati("gemini"), vec!["gemini.cmd", "gemini.exe", "gemini"]);
        assert!(candidati("").is_empty());
    }

    #[test]
    fn si_riscrivono_gli_ultimi_scambi_non_tutti() {
        let mut m9: Vec<Messaggio> = Vec::new();
        for i in 0..9 {
            m9.push(m("user", &format!("d{i}")));
        }
        let p = prompt_completo(&m9, "");
        assert!(!p.contains("d2"), "il terzultimo giro non c'entra piu'");
        assert!(p.contains("d8") && p.contains("d3"));
    }

    #[test]
    fn i_messaggi_vuoti_non_diventano_righe_vuote() {
        let p = prompt_completo(&[m("user", "  "), m("user", "c'e'")], "");
        assert_eq!(p, "UTENTE: c'e'");
    }

    #[test]
    fn una_dichiarazione_vuota_e_una_cli_che_si_chiama_come_il_suo_binario() {
        let d = dichiarata("gemini", &serde_json::Value::Null);
        assert_eq!(d.binario, "gemini");
        assert_eq!(d.etichetta, "Gemini");
        assert!(
            d.su_stdin,
            "il ripiego e' stdin: la riga di comando ha un tetto"
        );
        assert_eq!(d.secondi, SECONDI_PREDEFINITI);
        assert!(d.args.is_empty() && !d.a_consumo);
    }

    #[test]
    fn la_dichiarazione_si_legge_tutta() {
        let d = dichiarata(
            "glm",
            &serde_json::json!({
                "binary": "glm-cli", "args": ["--model", "{model}"],
                "model": "glm-4.6", "prompt": "argomento", "timeout": 30,
                "cwd": "C:\\lavoro", "a_consumo": true, "etichetta": "GLM"
            }),
        );
        assert_eq!(d.binario, "glm-cli");
        assert_eq!(
            d.etichetta, "GLM",
            "l'etichetta scritta vince su quella dedotta"
        );
        assert_eq!(d.args, vec!["--model", "{model}"]);
        assert!(!d.su_stdin && d.a_consumo);
        assert_eq!((d.secondi, d.cartella.as_str()), (30, "C:\\lavoro"));
    }

    #[test]
    fn un_tempo_di_zero_non_e_un_tempo() {
        // Zero in Python e' un valore falso, e li' `spec.get("timeout", 600)`
        // lo terrebbe: un tetto di zero secondi e' una CLI che non parte mai.
        assert_eq!(
            dichiarata("x", &serde_json::json!({"timeout": 0})).secondi,
            SECONDI_PREDEFINITI
        );
    }

    #[test]
    fn luscita_vuota_e_un_guasto_e_si_racconta_con_lo_stderr() {
        assert_eq!(
            cosa_ha_detto("Gemini", "  ciao  ", "rumore"),
            Ok("ciao".into())
        );
        let e = cosa_ha_detto("Gemini", " \n ", "ENOENT: manca la chiave").unwrap_err();
        assert!(
            e.contains("non ha prodotto output") && e.contains("manca la chiave"),
            "{e}"
        );
    }

    #[test]
    fn il_modello_si_sostituisce_ovunque_lo_chieda_la_configurazione() {
        let a = argomenti(
            "/bin/gemini",
            &["--model".into(), "{model}".into(), "--nome={model}".into()],
            "gemini-2.5-pro",
        );
        assert_eq!(a, vec!["/bin/gemini", "--model", "gemini-2.5-pro",
                           "--nome=gemini-2.5-pro"]);
    }
}
