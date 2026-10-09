//! Il custode dei permessi nel demone (D384).
//!
//! Un Dot ha l'autonomia piena: all'utente non chiede niente. Quando Nova
//! chiederebbe il permesso, secondo l'autonomia del pannello
//! ([`crate::permessi::si_chiede`]), un Dot lo chiede al custode: un Dot che
//! fa questo e basta (`nova_dot::custode`). Le strade da cui arriva la
//! domanda sono tre, e passano tutte da [`decidi`]:
//!
//! - il turno di un Dot, quando il modello chiama uno strumento di NOVA
//!   (`agente::EsecutoreDemone`);
//! - Claude Code lanciato per un Dot, che chiama gli strumenti di NOVA via MCP
//!   su un collegamento legato al Dot (`server`, `mcp/per_conto_di`);
//! - lo stesso Claude Code quando vuole usare uno strumento suo e chiede allo
//!   sportello (`approvazione.claude`).
//!
//! **Chi decide.** Prima il modello di casa, con una domanda si'/no letta
//! dalle lettere (`giudizio_casa`): privato e gratis. Se il modello di casa
//! non c'e', non si accende o non sa decidere, il cervello piu' grande della
//! scala che risponde a un indirizzo, con una domanda sola e senza strumenti.
//! Claude Code e le CLI no: un giudice che legge testo scritto da altri — gli
//! argomenti di un Dot, una pagina letta — non deve avere mani. Se la domanda
//! dovrebbe uscire dal PC si chiede prima a `nova_decisioni` se puo', e una
//! credenziale dentro la domanda non esce mai.
//!
//! **Nel dubbio, no.** Nessun cervello a cui chiedere, una risposta che non
//! si legge, una domanda che non puo' uscire: e' un no, col perche'. Ogni
//! decisione si scrive nel diario del custode e in `decisioni.jsonl`
//! (`permesso_dot`), ed esce come evento `dot.permesso`.

use std::sync::Arc;

use nova_decisioni::{Decisione, Fuori};
use nova_dot::custode as c;
use nova_giudizio::{Giudizio, Risposta};
use serde_json::{json, Value};

use crate::server::Server;

/// Com'e' andata una domanda al custode.
#[derive(Debug, Clone, PartialEq)]
pub struct Decisa {
    pub consentito: bool,
    /// `casa`, `grande` o `nessuno`.
    pub chi: &'static str,
    /// Com'e' andata la domanda, per il registro.
    pub come: String,
    /// Perche' no, per il Dot.
    pub motivo: String,
    pub probabilita_vero: Option<f64>,
}

fn no(chi: &'static str, come: String, motivo: String) -> Decisa {
    Decisa {
        consentito: false,
        chi,
        come,
        motivo,
        probabilita_vero: None,
    }
}

/// Se la domanda puo' andare a un cervello fuori dal PC. Una credenziale
/// dentro non esce mai; con «solo sul PC» acceso non esce niente
/// (`nova_decisioni::Fuori`). Un cervello sul PC non e' fuori.
pub fn si_puo_chiedere(in_casa: bool, stato: &str, solo_locale: bool) -> Result<(), String> {
    si_puo_chiedere_per(Decisione::PermessoDiUnDot, "il custode", in_casa, stato, solo_locale)
}

/// Come [`si_puo_chiedere`], per un'altra decisione e un altro che chiede:
/// AR, quando sceglie chi lavora (D397).
pub fn si_puo_chiedere_per(
    decisione: Decisione,
    chi: &str,
    in_casa: bool,
    stato: &str,
    solo_locale: bool,
) -> Result<(), String> {
    if in_casa {
        return Ok(());
    }
    if let Some(cosa) = nova_guasti::guardiano::perche_non_si_salva(stato) {
        return Err(format!(
            "nella domanda c'e' {cosa}, e una credenziale non esce dal PC"
        ));
    }
    if Fuori::prepara(decisione, stato, solo_locale).is_none() {
        return Err(format!("con «solo sul PC» acceso {chi} non chiede fuori"));
    }
    Ok(())
}

/// Chiede al custode se il Dot `dot` puo' fare un'azione. `Ok` vuol dire
/// «vai»; `Err` porta il testo che il Dot legge al posto del risultato.
pub async fn decidi(
    server: &Arc<Server>,
    dot: &str,
    strumento: &str,
    rischio: &str,
    dettaglio: &str,
) -> Result<(), String> {
    let cfg = nova_configurazione::dove::leggi();
    let (ruolo, compito) = crate::dot::ruolo_e_compito(server, dot);
    let stato = c::stato(dot, &ruolo, &compito, strumento, rischio, dettaglio);
    let d = giudica(server, &cfg, &stato).await;
    let riga = crate::decisioni::riga_permesso_dot(
        &crate::decisioni::adesso(),
        dot,
        strumento,
        rischio,
        &stato,
        d.consentito,
        d.chi,
        &d.come,
        d.probabilita_vero,
    );
    crate::decisioni::annota(&cfg, &riga);
    crate::dot::diario_del_custode(&riga);
    server.ctx.bus.emit(
        "dot.permesso",
        json!({
            "dot": dot,
            "strumento": strumento,
            "consentito": d.consentito,
            "chi": d.chi,
            "motivo": d.motivo,
        }),
    );
    if d.consentito {
        Ok(())
    } else {
        Err(c::negata(&d.motivo))
    }
}

/// Il giudizio: il modello di casa, poi il cervello grande, poi no.
async fn giudica(server: &Arc<Server>, cfg: &Value, stato: &str) -> Decisa {
    let gradini = crate::agente::scala_di(cfg);
    let recapiti = crate::dalla_configurazione::recapiti(cfg, &|n| std::env::var(n).ok());
    let mut note: Vec<String> = Vec::new();

    if gradini
        .iter()
        .any(|g| crate::modello_locale::e_il_modello_di_casa(g, &recapiti))
    {
        match crate::modello_locale::assicura(server, cfg, &recapiti.locale_url).await {
            Err(e) => note.push(format!("il modello di casa non si accende: {e}")),
            Ok(()) => {
                let base = recapiti.locale_url.clone();
                let s = stato.to_string();
                let esito = tokio::task::spawn_blocking(move || {
                    crate::giudizio_casa::giudica_in_casa(&base, &s, &c::domanda(), 1.0)
                })
                .await
                .map_err(|e| e.to_string())
                .and_then(|r| r);
                match esito {
                    Ok((e, _)) => match e.giudizio {
                        Giudizio::Risposto(Risposta::Booleana {
                            valore,
                            probabilita_vero,
                        }) => {
                            return Decisa {
                                consentito: valore,
                                chi: "casa",
                                come: "risposto".into(),
                                motivo: if valore {
                                    String::new()
                                } else {
                                    "secondo il modello di casa non serve al compito, o fa \
                                     danni che l'utente non ha chiesto"
                                        .into()
                                },
                                probabilita_vero: Some(probabilita_vero),
                            }
                        }
                        altro => note.push(format!(
                            "il modello di casa non sa decidere ({})",
                            altro.come_si_racconta()
                        )),
                    },
                    Err(e) => note.push(format!("il modello di casa non ha giudicato: {e}")),
                }
            }
        }
    }

    // Il cervello piu' grande che risponde a un indirizzo e non e' il modello
    // di casa: quello ha gia' detto quello che sapeva.
    let grande = gradini
        .iter()
        .rev()
        .find(|g| {
            g.indirizzo().is_some() && !crate::modello_locale::e_il_modello_di_casa(g, &recapiti)
        })
        .cloned();
    let Some(grande) = grande else {
        note.push("non c'e' un cervello senza mani a cui chiedere".into());
        let come = note.join("; ");
        return no(
            "nessuno",
            come.clone(),
            format!("il custode non ha potuto decidere: {come}"),
        );
    };
    let in_casa = grande.indirizzo().is_some_and(|(_, _, _, casa)| casa);
    let solo_locale = crate::dalla_configurazione::scala(cfg).solo_locale;
    if let Err(perche) = si_puo_chiedere(in_casa, stato, solo_locale) {
        note.push(perche);
        let come = note.join("; ");
        return no(
            "nessuno",
            come.clone(),
            format!("il custode non ha potuto decidere: {come}"),
        );
    }
    let detto = crate::agente::chiedi_con(
        std::slice::from_ref(&grande),
        &c::richiesta_al_grande(stato),
        400,
    )
    .await;
    note.push(format!("ha deciso «{}»", grande.nome()));
    match detto.as_deref().map(c::leggi_verdetto) {
        Some(Some(c::Verdetto::Consenti)) => Decisa {
            consentito: true,
            chi: "grande",
            come: note.join("; "),
            motivo: String::new(),
            probabilita_vero: None,
        },
        Some(Some(c::Verdetto::Nega(motivo))) => no("grande", note.join("; "), motivo),
        Some(None) => no(
            "grande",
            format!("{}; risposta che non si legge", note.join("; ")),
            "la risposta del cervello grande non si legge, e nel dubbio no".into(),
        ),
        None => no(
            "grande",
            format!("{}; nessuna risposta", note.join("; ")),
            format!("«{}» non ha risposto, e nel dubbio no", grande.nome()),
        ),
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn sul_pc_si_chiede_sempre_fuori_solo_se_si_puo() {
        assert!(si_puo_chiedere(true, "scrivi la chiave sk-xyz", true).is_ok());
        assert!(si_puo_chiedere(false, "scrivi appunti.txt", false).is_ok());
        let solo = si_puo_chiedere(false, "scrivi appunti.txt", true).unwrap_err();
        assert!(solo.contains("solo sul PC"), "{solo}");
        let chiave = format!(
            "scrivi {}AbCdEfGhIjKlMnOpQrStUvWxYz0123456789",
            ["sk", "ant", "api03-"].join("-")
        );
        let e = si_puo_chiedere(false, &chiave, false).unwrap_err();
        assert!(e.contains("non esce dal PC"), "{e}");
        assert!(
            !e.contains("AbCdEfGhIjKl"),
            "il perche' non ripete la chiave: {e}"
        );
    }
}
