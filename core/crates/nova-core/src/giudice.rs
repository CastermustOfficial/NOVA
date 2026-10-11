//! I giudici senza mani (D405).
//!
//! I giudici dell'azienda dei Dot leggono testo scritto da altri: AR, i
//! controlli dell'APM, il voto del capo, il custode, e l'Architetto, che
//! legge i file del progetto. Chi legge testo scritto da altri non deve
//! avere mani (D384), e fino a D405 l'unico cervello senza mani era quello
//! che risponde a un indirizzo. Chi aveva in scala solo Claude Code e CLI,
//! come Gio, aveva i Dot fermi al primo giudizio.
//!
//! Deciso con Gio l'11 ottobre, sulla bozza «Giudici senza mani»:
//!
//! 1. **Prima un cervello a indirizzo**, se la scala ne ha uno: il piu'
//!    grande, come prima. Chi ha un'API la usa come oggi.
//! 2. **Altrimenti Claude Code senza mani**, col modello del suo gradino
//!    com'e': niente strumenti, niente server MCP, niente CLAUDE.md,
//!    permessi negati (`nova_cervelli::claude::argomenti_senza_mani`). E non
//!    ci si fida delle opzioni: la riga d'apertura dice con che strumenti e'
//!    partito, e se non sono zero **la risposta non vale**.
//! 3. **L'Architetto legge**: con Claude ha `Read`, `Glob` e `Grep`, e parte
//!    dalla cartella del progetto. Con `dontAsk` un file fuori si nega da
//!    solo (provato sul PC di sviluppo).
//! 4. **Le altre CLI no.** Antigravity (`agy` 1.3.3) un modo di togliere gli
//!    strumenti non ce l'ha: provato senza `--dangerously-skip-permissions`,
//!    con `--mode plan` e con `--sandbox`, ha scritto un file ogni volta.

use serde_json::Value;

use crate::mondo::{Claude, Gradino};

/// Perche' un giudizio non si fa, quando la scala non ha un giudice.
pub const SENZA_GIUDICE: &str = "nella scala non c'e' un giudice senza mani: ne' un cervello che \
     risponde a un indirizzo, ne' Claude Code. Le altre CLI hanno mani loro, e non giudicano";

/// Chi giudica: un cervello a indirizzo, o Claude Code senza mani.
#[derive(Debug, Clone)]
pub enum Giudice {
    Indirizzo(Gradino),
    Claude { nome: String, come: Box<Claude> },
}

impl Giudice {
    /// Il giudice di questa scala: il piu' grande che risponde a un
    /// indirizzo e che `salta` non esclude; se non ce n'e', il gradino Claude
    /// piu' grande. Le altre CLI non giudicano mai.
    pub fn dalla_scala(gradini: &[Gradino], salta: impl Fn(&Gradino) -> bool) -> Option<Giudice> {
        if let Some(g) = gradini
            .iter()
            .rev()
            .find(|g| g.indirizzo().is_some() && !salta(g))
        {
            return Some(Giudice::Indirizzo(g.clone()));
        }
        gradini.iter().rev().find_map(|g| match g {
            Gradino::Claude { nome, come } => Some(Giudice::Claude {
                nome: nome.clone(),
                come: come.clone(),
            }),
            _ => None,
        })
    }

    /// Il nome del gradino: va nel registro come «chi ha deciso».
    pub fn nome(&self) -> &str {
        match self {
            Giudice::Indirizzo(g) => g.nome(),
            Giudice::Claude { nome, .. } => nome,
        }
    }

    /// Se la domanda resta sul PC. Claude Code no: la domanda va ad Anthropic.
    pub fn in_casa(&self) -> bool {
        match self {
            Giudice::Indirizzo(g) => g.indirizzo().is_some_and(|(_, _, _, casa)| casa),
            Giudice::Claude { .. } => false,
        }
    }

    /// Una domanda sola, senza strumenti. Torna il testo della risposta, o
    /// perche' non c'e'.
    pub async fn chiedi(&self, domanda: &str, gettoni: u32) -> Result<String, String> {
        match self {
            Giudice::Indirizzo(g) => {
                crate::agente::chiedi_con(std::slice::from_ref(g), domanda, gettoni)
                    .await
                    .ok_or_else(|| format!("«{}» non ha risposto", g.nome()))
            }
            Giudice::Claude { come, .. } => {
                claude_senza_mani(come, domanda, &cartella_del_giudice(), false).await
            }
        }
    }
}

/// Da dove parte un Claude giudice: una cartella vuota di NOVA, non quella
/// dell'utente. Senza strumenti non leggerebbe niente comunque; cosi' non
/// c'e' niente da leggere nemmeno se un giorno un'opzione cambiasse.
pub fn cartella_del_giudice() -> String {
    let c = crate::mondo::cartella_nova().join("giudice");
    let _ = std::fs::create_dir_all(&c);
    c.to_string_lossy().into_owned()
}

/// Claude Code senza mani, una volta. `lettore` gli da' i tre strumenti per
/// leggere, dentro `cartella` e basta; senza, nemmeno quelli e un passo solo.
pub async fn claude_senza_mani(
    c: &Claude,
    domanda: &str,
    cartella: &str,
    lettore: bool,
) -> Result<String, String> {
    use nova_cervelli::claude;
    let eseguibile = nova_cervelli::cerca::dove_e_claude(&c.d.binario);
    if let Some(perche) = claude::perche_non_pronto(
        &eseguibile,
        std::path::Path::new(&eseguibile).exists(),
        nova_cervelli::cerca::credenziali().is_some(),
    ) {
        return Err(perche);
    }
    let turni = if lettore { c.d.max_turns } else { 1 };
    let args = claude::argomenti_senza_mani(
        &eseguibile,
        &crate::modelli::per_claude(&c.d.modello),
        lettore,
        turni,
    );
    // La domanda su stdin: su Windows `claude` puo' essere un file batch, e
    // un a capo in un argomento chiude la riga (vedi `lancia_claude`).
    let u = crate::processo::lancia(&args, Some(domanda), cartella, c.d.secondi)
        .await
        .map_err(|g| match g {
            crate::processo::Guaio::Troppo => {
                format!("Claude Code non ha risposto entro {}s", c.d.secondi)
            }
            crate::processo::Guaio::Muto(e) => format!("Claude Code non e' partito: {e}"),
        })?;
    if nova_pitone::senza_bianchi(&u.stdout).is_empty() {
        return Err(claude::senza_uscita(&u.stderr, args.join(" ").chars().count()));
    }
    let f = claude::leggi_flusso(&u.stdout)?;
    claude::senza_mani_davvero(&f, lettore)?;
    let mut dati = f.risultato;
    let testo = dati.get("result").and_then(Value::as_str).unwrap_or("").to_string();
    if dati.get("is_error").and_then(Value::as_bool).unwrap_or(false) {
        if nova_guasti::cervelli::e_limite_uso(&testo) {
            return Err(format!(
                "Claude Code ha esaurito la quota: {}",
                testo.chars().take(300).collect::<String>()
            ));
        }
        dati["_stderr"] = Value::String(u.stderr);
        return Err(nova_guasti::cervelli::perche_errore(&dati, turni));
    }
    Ok(nova_pitone::senza_bianchi(&testo).to_string())
}

#[cfg(test)]
mod prove {
    use super::*;
    use crate::mondo::Come;
    use nova_scala::Specie;

    fn indirizzo(nome: &str, casa: bool) -> Gradino {
        Gradino::nuovo(nome, Specie::Locale, "http://x", nome, Vec::new(), casa, Come::Niente)
    }
    fn claude(nome: &str, modello: &str) -> Gradino {
        let mut c = Claude::default();
        c.d.modello = modello.into();
        Gradino::nuovo(nome, Specie::Claude, "", "", Vec::new(), false, Come::Claude(&c))
    }
    fn cli(nome: &str) -> Gradino {
        Gradino::nuovo(nome, Specie::Cli, "", "", Vec::new(), false, Come::Niente)
    }

    /// La scala di Gio: Antigravity, Antigravity, Claude. Giudica Claude, col
    /// modello del suo gradino.
    #[test]
    fn senza_indirizzi_giudica_claude_col_suo_modello() {
        let s = [cli("orchestra"), cli("rapido"), claude("difficile", "ultimo:opus")];
        let g = Giudice::dalla_scala(&s, |_| false).expect("Claude c'e'");
        assert_eq!(g.nome(), "difficile");
        assert!(!g.in_casa(), "la domanda va ad Anthropic");
        let Giudice::Claude { come, .. } = g else { panic!("{g:?}") };
        assert_eq!(come.d.modello, "ultimo:opus");
    }

    #[test]
    fn un_indirizzo_viene_prima_anche_se_claude_e_piu_in_alto() {
        let s = [indirizzo("piccolo", true), indirizzo("medio", false), claude("grande", "opus")];
        let g = Giudice::dalla_scala(&s, |_| false).unwrap();
        assert!(matches!(&g, Giudice::Indirizzo(_)) && g.nome() == "medio", "{g:?}");
        assert!(!g.in_casa());
        // Quello che `salta` esclude non giudica: il custode salta il
        // modello di casa, che ha gia' detto la sua.
        let g = Giudice::dalla_scala(&s, |g| g.nome() == "medio").unwrap();
        assert_eq!(g.nome(), "piccolo");
        assert!(g.in_casa());
        let g = Giudice::dalla_scala(&s, |g| g.nome() != "grande").unwrap();
        assert!(matches!(g, Giudice::Claude { .. }), "saltati tutti, resta Claude");
    }

    #[test]
    fn le_altre_cli_non_giudicano_mai() {
        assert!(Giudice::dalla_scala(&[cli("antigravity"), cli("codex")], |_| false).is_none());
        assert!(Giudice::dalla_scala(&[], |_| false).is_none());
        let s = [claude("piccolo", "haiku"), cli("x"), claude("grande", "opus")];
        assert_eq!(Giudice::dalla_scala(&s, |_| false).unwrap().nome(), "grande", "il piu' grande");
    }
}
