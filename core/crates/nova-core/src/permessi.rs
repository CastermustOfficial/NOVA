//! Quando un modello deve chiedere prima di agire.
//!
//! Le capacita' del demone si chiamano da tre porte, e solo due di queste
//! sono un modello:
//!
//! - **il turno del demone** (`agente/turno`), col cervello locale o un
//!   fornitore: chiama gli strumenti per conto suo;
//! - **la porta MCP** (`tools/call`), da cui passa Claude Code — e l'intero
//!   server `nova-core` sta nell'elenco degli strumenti che Claude usa senza
//!   chiedere (`--allowedTools`), quindi il suo sportello dei permessi per
//!   queste capacita' non scatta mai;
//! - **la porta diretta** (`capabilities/call`): il guscio, la riga di
//!   comando, i bottoni. Li' chi chiama e' la persona, e chiederle il
//!   permesso di fare quello che ha appena chiesto non protegge niente.
//!
//! Prima di questo modulo le prime due non chiedevano mai niente. Il Python
//! chiedeva nel suo turno (`needs_approval`, con il livello di autonomia
//! scelto nel pannello); il demone, che quel livello lo scriveva nel suo
//! stato, non lo guardava. Un `shell.exec` voluto dal modello partiva e
//! basta (D333).
//!
//! La regola e' quella del Python — [`Autonomia::chiede`], confrontata da un
//! banco — e il livello e' quello del `config.json` di NOVA, cioe' quello
//! che l'utente ha scelto nel pannello, riletto a ogni chiamata: cambiarlo
//! deve valere adesso.

use nova_proto::Risk;
use nova_strumenti::guardie::Autonomia;
use nova_strumenti::Rischio;
use serde_json::Value;

use crate::capability::{Capability, Ctx};

/// Le capacita' che usa solo la persona. Un modello che le chiama si sente
/// dire di no, e non le vede nemmeno nell'elenco.
///
/// `approvazione.rispondi` e' il bottone «consenti»: un modello che potesse
/// chiamarlo si approverebbe da solo le richieste che fa. Stava nell'elenco
/// di Claude Code, e il server `nova-core` e' fra quelli che Claude usa senza
/// chiedere.
pub const SOLO_PER_LA_PERSONA: [&str; 1] = ["approvazione.rispondi"];

/// Il testo che il modello legge quando la persona ha detto di no. E' quello
/// del Python, parola per parola.
pub const RIFIUTATA: &str = "AZIONE RIFIUTATA dall'utente. Non ripeterla: chiedi come \
                             procedere oppure proponi un'alternativa.";

/// Quando nessuno ha risposto: le parole dello sportello di Claude.
pub const SENZA_RISPOSTA: &str = "l'utente non ha risposto: considera l'azione non \
                                  autorizzata e spiega cosa avresti fatto invece di riprovare";

/// Se questa capacita' la puo' chiamare un modello.
pub fn per_un_modello(nome: &str) -> bool {
    !SOLO_PER_LA_PERSONA.contains(&nome)
}

/// Il rischio del demone nelle parole della regola.
pub fn rischio(r: Risk) -> Rischio {
    match r {
        Risk::Safe => Rischio::Innocuo,
        Risk::Moderate => Rischio::Modifica,
        Risk::Dangerous => Rischio::Pericoloso,
    }
}

/// Il livello di autonomia che l'utente ha scelto: `safety.autonomy` nel
/// `config.json` di NOVA. Un valore che non si capisce e' «chiedi se
/// rischioso», mai «fai pure».
pub fn autonomia(cfg: &Value) -> Autonomia {
    Autonomia::dal_nome(
        cfg.get("safety")
            .and_then(|s| s.get("autonomy"))
            .and_then(Value::as_str)
            .unwrap_or(""),
    )
}

/// Se prima di questa capacita' si chiede. Le capacita' dello sportello non
/// chiedono mai — chiedere il permesso di chiedere un permesso non finisce —
/// e nemmeno il freno: fermarsi deve funzionare sempre.
pub fn si_chiede(cap: &dyn Capability, a: Autonomia) -> bool {
    let i = cap.info();
    if i.category == "approvazione" || i.name == "azione.ferma" || i.name == "azione.stato" {
        return false;
    }
    a.chiede(rischio(i.risk))
}

/// La frase che la persona legge: cosa succede se dice di si'.
///
/// E' l'anteprima della capacita', quella di `prova=true`, se sa darla; se
/// no, la descrizione con gli argomenti. Mai il solo nome: `shell.exec` non
/// dice a nessuno cosa sta per succedere.
pub async fn in_chiaro(cap: &dyn Capability, args: &Value, ctx: &Ctx) -> String {
    if let Some(Ok(v)) = cap.anteprima(args.clone(), ctx).await {
        let detto = racconta(&v);
        if !detto.trim().is_empty() {
            return detto;
        }
    }
    let argomenti: String = args.to_string().chars().take(400).collect();
    format!("{}\n{argomenti}", cap.info().description)
}

/// Un'anteprima come la legge una persona.
///
/// Le anteprime non hanno tutte la stessa forma: quella di `fs.delete` dice
/// «manderei nel Cestino» in `farei` e il percorso a parte, quella di
/// `shell.exec` mette la riga in `farei_girare`. Prendere solo `farei`
/// voleva dire chiedere «posso mandare nel Cestino?» senza dire **cosa**.
/// Si tiene tutto tranne cio' che parla al modello (`nota`) o al protocollo.
pub fn racconta(v: &Value) -> String {
    const TESTA: [&str; 2] = ["farei", "farei_girare"];
    const FUORI: [&str; 5] = ["annullabile", "nota", "prova", "eseguito", "farei"];
    let corto = |x: &Value| -> String {
        let t = match x {
            Value::String(s) => s.clone(),
            altro => altro.to_string(),
        };
        let mut c: String = t.chars().take(300).collect();
        if t.chars().count() > 300 {
            c.push('…');
        }
        c
    };
    let Some(o) = v.as_object() else {
        return corto(v);
    };
    let mut righe = Vec::new();
    for k in TESTA {
        if let Some(x) = o.get(k).filter(|x| !x.is_null()) {
            righe.push(corto(x));
        }
    }
    for (k, x) in o {
        if FUORI.contains(&k.as_str()) || TESTA.contains(&k.as_str()) || x.is_null() {
            continue;
        }
        if x.as_str().is_some_and(|s| s.trim().is_empty()) {
            continue;
        }
        righe.push(format!("{k}: {}", corto(x)));
    }
    righe.join("\n")
}

/// Il permesso per una chiamata voluta da un modello.
///
/// `Ok` vuol dire «vai». `Err` porta il testo da restituire al modello al
/// posto del risultato: un rifiuto, una scadenza, o una capacita' che non
/// fa per lui.
pub async fn chiedi_per_un_modello(
    cap: &dyn Capability,
    args: &Value,
    ctx: &Ctx,
) -> Result<(), String> {
    let nome = cap.info().name;
    if !per_un_modello(&nome) {
        return Err(format!(
            "«{nome}» la usa la persona, non un modello: le richieste di permesso le decide lei."
        ));
    }
    let a = autonomia(&nova_configurazione::dove::leggi());
    if !si_chiede(cap, a) {
        return Ok(());
    }
    let dettaglio = in_chiaro(cap, args, ctx).await;
    let esito = crate::caps_approvazione::chiedi_e_aspetta(
        ctx,
        nome,
        dettaglio,
        cap.info().risk.as_str().to_string(),
        "utente".into(),
        // Quanto si aspetta la persona: come per lo sportello di Claude
        // Code, che e' la stessa persona davanti allo stesso bottone.
        crate::caps_approvazione::ATTESA_CLAUDE_S,
    )
    .await
    .map_err(|e| format!("non ho potuto chiedere il permesso: {e}"))?;
    match esito.get("esito").and_then(Value::as_str) {
        Some("consentito") => Ok(()),
        Some("scaduto") => Err(SENZA_RISPOSTA.to_string()),
        // Un esito che non si riconosce e' un no, come dallo sportello di
        // Claude: un guasto non deve mai diventare un permesso.
        _ => {
            let motivo = esito.get("motivo").and_then(Value::as_str).unwrap_or("");
            if motivo.trim().is_empty() {
                Err(RIFIUTATA.to_string())
            } else {
                Err(format!("{RIFIUTATA} Motivo: {motivo}"))
            }
        }
    }
}

#[cfg(test)]
mod prove {
    use super::*;
    use serde_json::json;

    #[test]
    fn il_livello_e_quello_del_pannello() {
        assert_eq!(autonomia(&json!({})), Autonomia::ChiediSeRischioso);
        assert_eq!(
            autonomia(&json!({"safety": {"autonomy": "autonomous"}})),
            Autonomia::Tutto
        );
        assert_eq!(
            autonomia(&json!({"safety": {"autonomy": "always_ask"}})),
            Autonomia::Chiedi
        );
        assert_eq!(
            autonomia(&json!({"safety": {"autonomy": "fai pure"}})),
            Autonomia::ChiediSeRischioso
        );
    }

    struct Finta(&'static str, &'static str, Risk);

    #[async_trait::async_trait]
    impl Capability for Finta {
        fn info(&self) -> nova_proto::CapabilityInfo {
            nova_proto::CapabilityInfo {
                name: self.0.into(),
                description: String::new(),
                risk: self.2,
                category: self.1.into(),
                schema: json!({}),
            }
        }
        async fn call(&self, _a: Value, _c: &Ctx) -> anyhow::Result<Value> {
            Ok(Value::Null)
        }
    }

    #[test]
    fn lo_sportello_e_il_freno_non_chiedono_mai() {
        // Anche con «conferma sempre»: chiedere il permesso di chiedere un
        // permesso non finisce, e fermarsi deve funzionare sempre.
        let sempre = Autonomia::Chiedi;
        assert!(!si_chiede(
            &Finta("approvazione.claude", "approvazione", Risk::Safe),
            sempre
        ));
        assert!(!si_chiede(
            &Finta("azione.ferma", "azione", Risk::Moderate),
            sempre
        ));
        assert!(si_chiede(&Finta("sys.info", "sistema", Risk::Safe), sempre));
        let prudente = Autonomia::ChiediSeRischioso;
        assert!(si_chiede(
            &Finta("shell.exec", "shell", Risk::Dangerous),
            prudente
        ));
        assert!(!si_chiede(
            &Finta("fs.write", "file", Risk::Moderate),
            prudente
        ));
    }

    #[test]
    fn l_anteprima_dice_anche_cosa() {
        let cestino = json!({"farei": "manderei nel Cestino, da dove si recupera",
                             "path": "C:\\x.txt", "annullabile": false, "nota": "per il modello"});
        assert_eq!(
            racconta(&cestino),
            "manderei nel Cestino, da dove si recupera\npath: C:\\x.txt"
        );
        let riga = json!({"farei_girare": "shutdown /s", "dove": "C:\\", "shell": "powershell",
                          "nota": "leggila", "annullabile": false});
        assert_eq!(
            racconta(&riga),
            "shutdown /s\ndove: C:\\\nshell: powershell"
        );
    }

    #[test]
    fn il_bottone_consenti_non_e_per_i_modelli() {
        assert!(!per_un_modello("approvazione.rispondi"));
        assert!(per_un_modello("approvazione.claude"));
        assert!(per_un_modello("shell.exec"));
    }
}
