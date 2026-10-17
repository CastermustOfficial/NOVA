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

/// Le CLI agentiche che NOVA conosce da sola: il predefinito di
/// `brains.cli`, gemello di `cli_predefinite()` in `nova/routing.py` (una
/// prova confronta i due). Serve a chi scrive la configurazione da zero —
/// l'installatore, `nova configura` — che senza Python non ha piu' un
/// `Config` coi suoi predefiniti da salvare.
///
/// Il modello resta vuoto di proposito: ogni CLI ha il suo predefinito, che
/// cambia piu' spesso di quanto si aggiorni un file di configurazione.
pub fn predefinite() -> Value {
    serde_json::json!({
        "codex": {
            "etichetta": "Codex (OpenAI)",
            "binary": "codex",
            "args": ["exec", "--skip-git-repo-check"],
            "model": "",
            "prompt": "argomento",
            "timeout": 900,
            "a_consumo": false,
        },
        // Il sostituto di Gemini CLI per gli account personali, dal 18
        // giugno 2026. `--dangerously-skip-permissions`: una CLI agentica
        // agisce con le proprie mani, non passando dalle guardie di NOVA.
        // `--model {model}` sparisce senza modello; `agy models` da' l'elenco
        // da cui si sceglie un `ultimo:<forma>` (D377).
        "antigravity": {
            "etichetta": "Antigravity (Google)",
            "binary": "agy",
            "args": ["--dangerously-skip-permissions", "--model", "{model}", "-p"],
            "elenco_modelli": ["models"],
            "model": "",
            "prompt": "argomento",
            "timeout": 600,
            "a_consumo": false,
        },
        "gemini": {
            "etichetta": "Gemini (licenza enterprise o chiave API)",
            "binary": "gemini",
            "args": ["--approval-mode", "yolo", "-p"],
            "model": "",
            "prompt": "argomento",
            "timeout": 600,
            "a_consumo": false,
        },
        "qwen": {
            "etichetta": "Qwen Code",
            "binary": "qwen",
            "args": ["-p"],
            "model": "",
            "prompt": "argomento",
            "timeout": 600,
            "a_consumo": false,
        },
    })
}

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
    /// Gli argomenti che fanno stampare alla CLI i modelli che ha
    /// (`["models"]` per Antigravity). Vuoto: la CLI un elenco non lo da', e
    /// un `ultimo:<famiglia>` scritto per lei lascia scegliere a lei (D377).
    pub elenco_modelli: Vec<String>,
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
        elenco_modelli: spec
            .get("elenco_modelli")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
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
///
/// Senza modello, `--model {model}` sparisce intero invece di diventare
/// `--model ""` (D377): e' il caso di un `ultimo:<famiglia>` che la CLI
/// sceglie da sola, o di un elenco che non si e' ancora potuto leggere.
pub fn argomenti(eseguibile: &str, args: &[String], model: &str) -> Vec<String> {
    let mut fuori = vec![eseguibile.to_string()];
    if model.trim().is_empty() {
        fuori.extend(crate::modelli::senza_modello(args));
    } else {
        fuori.extend(args.iter().map(|a| a.replace("{model}", model)));
    }
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

/// Quando la CLI non risponde entro il suo tempo. Claude Code lo dice con
/// la stessa frase, e [`e_tempo_scaduto`] la riconosce.
pub fn non_ha_risposto(etichetta: &str, secondi: u64) -> String {
    format!("{etichetta}{SCADUTO}{secondi}s")
}

/// Il pezzo fisso della frase di [`non_ha_risposto`].
const SCADUTO: &str = " non ha risposto entro ";

/// Se un esito dice che una CLI, o Claude Code, e' andata oltre il suo
/// tempo: la frase di [`non_ha_risposto`], anche in mezzo ad altro testo.
/// Un compito dei Dot scaduto cosi' si riprova una volta (D410).
pub fn e_tempo_scaduto(testo: &str) -> bool {
    testo.split(SCADUTO).skip(1).any(|dopo| {
        let cifre = dopo.chars().take_while(char::is_ascii_digit).count();
        cifre > 0 && dopo[cifre..].starts_with('s')
    })
}

/// Il pezzo fisso della frase di [`quota_finita`].
const QUOTA: &str = " ha finito la quota, e riprende fra ";

/// La frase di una CLI che ha finito la quota (D414): chi, fra quanti
/// secondi riprende, e quello che ha detto lei. Non e' un compito andato
/// male: un compito dei Dot che la trova aspetta e si rifa'.
pub fn quota_finita(etichetta: &str, secondi: i64, dettaglio: &str) -> String {
    format!("{etichetta}{QUOTA}{secondi}s. {dettaglio}")
}

/// Chi ha finito la quota e fra quanti secondi torna, se l'esito e' la
/// frase di [`quota_finita`] (anche in mezzo ad altro testo, come «il piano
/// non si e' fatto: …»), o quella di Claude Code, che quando torna non lo
/// dice: allora `se_non_lo_dice`.
pub fn attesa_quota(testo: &str, se_non_lo_dice: i64) -> Option<(String, i64)> {
    // Il nome e' quel che sta prima della frase, dopo l'ultimo «: » o a capo.
    let chi = |prima: &str| {
        prima
            .rsplit([':', '\n'])
            .next()
            .unwrap_or(prima)
            .trim()
            .to_string()
    };
    let mut pezzi = testo.split(QUOTA);
    let mut prima = pezzi.next().unwrap_or("");
    for dopo in pezzi {
        let cifre = dopo.chars().take_while(char::is_ascii_digit).count();
        if cifre > 0 && dopo[cifre..].starts_with('s') {
            if let Ok(n) = dopo[..cifre].parse::<i64>() {
                return Some((chi(prima), n));
            }
        }
        prima = dopo;
    }
    let (prima, _) = testo.split_once(ESAURITA)?;
    Some((chi(prima), se_non_lo_dice))
}

/// Come lo dice Claude Code (`nova_core::mondo`, `nova/brains/claude_cli.py`).
const ESAURITA: &str = " ha esaurito la quota";

/// Quanto manca al ritorno della quota, come lo scrive la CLI: Antigravity
/// dice «Resets in 2h4m26s». `None` se non lo dice.
pub fn secondi_dichiarati(testo: &str) -> Option<i64> {
    let basso = testo.to_lowercase();
    let da = basso.find("resets in ")? + "resets in ".len();
    let mut totale = 0i64;
    let mut numero = String::new();
    let mut visto = false;
    for c in basso[da..].chars() {
        match c {
            '0'..='9' => numero.push(c),
            'h' | 'm' | 's' if !numero.is_empty() => {
                let n: i64 = numero.parse().ok()?;
                totale += n * match c {
                    'h' => 3600,
                    'm' => 60,
                    _ => 1,
                };
                numero.clear();
                visto = true;
            }
            _ => break,
        }
    }
    visto.then_some(totale)
}

/// Quanto manca al ritorno della quota quando la CLI dice l'ora, come Claude
/// Code: «You've hit your session limit · resets 5:10pm (Europe/Rome)» (D430).
/// `ora` sono i secondi passati dalla mezzanotte, nel fuso del PC: l'ora
/// detta e' quella del PC, e il fuso fra parentesi non si legge. Un'ora gia'
/// passata e' quella di domani. `None` se non dice un'ora, o se dice un
/// giorno («resets Oct 20, 5pm»), che si lascia all'attesa di serie.
pub fn secondi_fino_alle(testo: &str, ora: i64) -> Option<i64> {
    let basso = testo.to_lowercase();
    let mut resto = &basso[basso.find("resets ")? + "resets ".len()..];
    resto = resto.strip_prefix("at ").unwrap_or(resto);
    let cifre = |s: &str| s.chars().take_while(char::is_ascii_digit).count();
    let n = cifre(resto);
    if n == 0 || n > 2 {
        return None;
    }
    let mut ore: i64 = resto[..n].parse().ok()?;
    resto = &resto[n..];
    let mut minuti = 0;
    let con_i_minuti = resto.starts_with(':');
    if con_i_minuti {
        let m = cifre(&resto[1..]);
        if m != 2 {
            return None;
        }
        minuti = resto[1..3].parse().ok()?;
        resto = &resto[3..];
    }
    let resto = resto.trim_start();
    if resto.starts_with("am") || resto.starts_with("pm") {
        if !(1..=12).contains(&ore) {
            return None;
        }
        ore %= 12;
        if resto.starts_with("pm") {
            ore += 12;
        }
    } else if !con_i_minuti || ore > 23 {
        // «resets 5» non e' un'ora; «resets 17:10» si'.
        return None;
    }
    if minuti > 59 {
        return None;
    }
    let giorno = 24 * 3600;
    let mut fra = ore * 3600 + minuti * 60 - ora.rem_euclid(giorno);
    if fra <= 0 {
        fra += giorno;
    }
    Some(fra)
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

    #[test]
    fn la_quota_finita_si_riconosce_con_quando_torna() {
        // D414: Antigravity, il 13 ottobre alle 18:28.
        let detto = "Antigravity (Google) non ha prodotto output. error: Individual quota reached. \
                     Please upgrade your subscription to increase your limits. Resets in 2h4m26s.";
        assert_eq!(secondi_dichiarati(detto), Some(2 * 3600 + 4 * 60 + 26));
        assert_eq!(secondi_dichiarati("Resets in 45m."), Some(2700));
        assert_eq!(secondi_dichiarati("resets in 30s"), Some(30));
        assert_eq!(secondi_dichiarati("Individual quota reached."), None);
        assert_eq!(secondi_dichiarati("Resets in a while"), None);
        let f = quota_finita("Antigravity (Google)", 7466, detto);
        assert!(f.starts_with("Antigravity (Google) ha finito la quota, e riprende fra 7466s."), "{f}");
        let ag = Some(("Antigravity (Google)".to_string(), 7466));
        assert_eq!(attesa_quota(&f, 900), ag);
        assert_eq!(attesa_quota(&format!("il piano non si e' fatto: {f}"), 900), ag);
        assert_eq!(
            attesa_quota("Claude Code ha esaurito la quota: limite", 900),
            Some(("Claude Code".to_string(), 900))
        );
        assert_eq!(attesa_quota("Antigravity (Google) non ha prodotto output. boh", 900), None);
        assert_eq!(attesa_quota(&non_ha_risposto("Finta", 600), 900), None);
    }

    /// D430: Claude Code dice l'ora in cui torna, non fra quanto.
    #[test]
    fn la_quota_di_claude_torna_all_ora_detta() {
        let detto = "You've hit your session limit · resets 5:10pm (Europe/Rome)";
        let alle_15_55 = 15 * 3600 + 55 * 60;
        assert_eq!(secondi_fino_alle(detto, alle_15_55), Some(75 * 60));
        assert_eq!(
            secondi_fino_alle("resets 3pm", 14 * 3600 + 30 * 60 + 20),
            Some(29 * 60 + 40)
        );
        assert_eq!(secondi_fino_alle("Resets at 12am", 23 * 3600), Some(3600));
        assert_eq!(secondi_fino_alle("resets 12pm", 11 * 3600), Some(3600));
        assert_eq!(secondi_fino_alle("resets 17:10", 17 * 3600), Some(600));
        assert_eq!(secondi_fino_alle("resets 9 am", 8 * 3600), Some(3600));
        // Gia' passata: e' domani.
        assert_eq!(
            secondi_fino_alle(detto, 17 * 3600 + 20 * 60),
            Some(24 * 3600 - 10 * 60)
        );
        assert_eq!(
            secondi_fino_alle(detto, 17 * 3600 + 10 * 60),
            Some(24 * 3600)
        );
        // L'ora del giorno si prende anche da un istante intero.
        assert_eq!(
            secondi_fino_alle(detto, 86_400 * 3 + alle_15_55),
            Some(75 * 60)
        );
        assert_eq!(secondi_fino_alle("resets Oct 20, 5pm", 0), None);
        assert_eq!(secondi_fino_alle("resets 5", 0), None);
        assert_eq!(secondi_fino_alle("resets 13pm", 0), None);
        assert_eq!(secondi_fino_alle("resets 25:00", 0), None);
        assert_eq!(secondi_fino_alle("resets 5:7pm", 0), None);
        assert_eq!(secondi_fino_alle("Resets in 2h4m26s", 0), None);
        assert_eq!(secondi_fino_alle("session limit", 0), None);
        let f = quota_finita("Claude Code", 4500, detto);
        assert_eq!(
            attesa_quota(&f, 900),
            Some(("Claude Code".to_string(), 4500))
        );
    }

    #[test]
    fn il_tempo_scaduto_si_riconosce_e_niente_altro() {
        assert!(e_tempo_scaduto(&non_ha_risposto("Antigravity (Google)", 600)));
        assert!(e_tempo_scaduto(&non_ha_risposto("Claude Code", 900)));
        assert!(e_tempo_scaduto(&format!("primo: {}; secondo: rotto", non_ha_risposto("X", 3))));
        assert!(!e_tempo_scaduto("il modello locale non ha risposto entro l'attesa"));
        assert!(!e_tempo_scaduto("Finta non ha risposto entro s"));
        assert!(!e_tempo_scaduto("Finta non ha prodotto output."));
        assert!(!e_tempo_scaduto(""));
    }

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
    fn l_elenco_dei_modelli_si_legge_e_di_serie_non_c_e() {
        let d = dichiarata(
            "antigravity",
            &serde_json::json!({"elenco_modelli": ["models"]}),
        );
        assert_eq!(d.elenco_modelli, vec!["models"]);
        assert!(dichiarata("x", &serde_json::json!({}))
            .elenco_modelli
            .is_empty());
    }

    #[test]
    fn senza_modello_la_riga_non_chiede_un_modello_senza_nome() {
        let args: Vec<String> = ["--x", "--model", "{model}", "-p"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(argomenti("agy", &args, ""), vec!["agy", "--x", "-p"]);
        assert_eq!(argomenti("agy", &args, "  "), vec!["agy", "--x", "-p"]);
        assert_eq!(
            argomenti("agy", &args, "gemini-3.1-pro-high"),
            vec!["agy", "--x", "--model", "gemini-3.1-pro-high", "-p"]
        );
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
