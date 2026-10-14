//! Le domande dei Dot mentre lavorano, e la chat del progetto (D419).
//!
//! Le parole e come si leggono stanno in `nova_dot::domande`. Qui c'e' chi
//! fa il resto, dopo ogni turno di un compito di un progetto:
//!
//! - i messaggi `ALLA CHAT:` vanno nella chat del progetto, a nome del Dot;
//! - le domande `CHIEDO A <chi>:` vanno a chi risponde, e la risposta torna
//!   al Dot nello stesso compito. Rispondono il legale, l'APM o un collega
//!   della squadra, col cervello piu' grande e senza mani, come i controlli
//!   dell'APM: chi legge testo scritto da un altro non deve avere mani
//!   (D384). Un collega risponde dalle sue consegne nel progetto, non coi
//!   suoi file: la sua conversazione e' sua, e magari sta lavorando.
//!
//! Ogni domanda con la sua risposta resta scritta tre volte, e non e'
//! burocrazia: nel diario del progetto (`Evento::Domanda`), perche' un
//! parere chiesto e avuto si ritrova; nella chat del progetto, perche' lo
//! vedano gli altri e Gio; nel diario del Dot, coi suoi passi.

use std::sync::Arc;

use nova_dot as d;
use nova_dot::domande as dd;
use nova_dot::progetto::{self as pr, Evento, Progetto};
use serde_json::json;

use crate::server::Server;

/// Com'e' andato un turno, guardato dopo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dopo {
    /// Il turno e' la consegna: questo e' il testo, senza le righe per NOVA.
    Consegna(String),
    /// Il Dot ha chiesto: riprende con questo messaggio, che porta le
    /// risposte.
    Riprendi(String),
}

/// Guarda quello che il Dot ha detto in un turno del compito: scrive i
/// messaggi alla chat e porta le domande. `giri` conta quante volte ha gia'
/// chiesto in questo compito. Un compito che non e' di un progetto non ha
/// ne' chat ne' squadra: quello che ha detto e' la consegna, com'e'.
pub async fn dopo_il_turno(
    server: &Arc<Server>,
    c: &d::Cartella,
    nome: &str,
    compito: &d::Compito,
    detto: &str,
    giri: &mut usize,
) -> Dopo {
    let Some((progetto, incarico)) = pr::progetto_dal_testo(&compito.testo) else {
        return Dopo::Consegna(detto.to_string());
    };
    let x = dd::dividi(detto);
    for m in &x.alla_chat {
        alla_chat(server, c, nome, compito.id, &progetto, m);
    }
    if x.domande.is_empty() {
        return Dopo::Consegna(x.resto);
    }
    if *giri >= dd::GIRI_PER_COMPITO {
        // Ha gia' chiesto quanto poteva: se ha anche consegnato, vale la
        // consegna; se no gli si dice, una volta, di consegnare. Se chiede
        // ancora, quello che ha detto e' la consegna, com'e'.
        *giri += 1;
        diario(
            c,
            compito.id,
            json!({ "tipo": "domande_finite", "domande": x.domande.len() }),
        );
        return if !x.resto.trim().is_empty() {
            Dopo::Consegna(x.resto)
        } else if *giri == dd::GIRI_PER_COMPITO + 1 {
            Dopo::Riprendi(dd::BASTA.to_string())
        } else {
            Dopo::Consegna(detto.trim().to_string())
        };
    }
    *giri += 1;
    let p = crate::apm::leggi(&progetto);
    let mut risposte = Vec::new();
    for q in x.domande.iter().take(dd::DOMANDE_PER_GIRO) {
        let r = rispondi(p.as_ref(), &progetto, nome, compito, q).await;
        diario(
            c,
            compito.id,
            json!({
                "tipo": "domanda",
                "a": q.a,
                "domanda": q.testo,
                "risposta": dd::tagliato(&r, 1_000),
            }),
        );
        crate::apm::annota(
            &progetto,
            Evento::Domanda {
                quando: crate::decisioni::adesso(),
                da: nome.to_string(),
                a: q.a.clone(),
                incarico: incarico.clone(),
                domanda: q.testo.clone(),
                risposta: r.clone(),
            },
        );
        server.ctx.bus.emit(
            "dot.domanda",
            json!({ "dot": nome, "id": compito.id, "a": q.a, "domanda": q.testo }),
        );
        alla_chat(
            server,
            c,
            nome,
            compito.id,
            &progetto,
            &dd::in_chat(&incarico, q, &r),
        );
        risposte.push((q.clone(), r));
    }
    let mut seguito = dd::risposte(&risposte, dd::GIRI_PER_COMPITO - *giri);
    if x.domande.len() > dd::DOMANDE_PER_GIRO {
        seguito.push_str(&format!(
            " Avevi fatto {} domande: ho portato le prime {}, le altre richiedile se ti servono \
             ancora.",
            x.domande.len(),
            dd::DOMANDE_PER_GIRO
        ));
    }
    Dopo::Riprendi(seguito)
}

/// Le domande che un Dot ha fatto lavorando a un compito di un progetto, con
/// le risposte, dal diario del progetto: (a chi, la domanda, la risposta).
/// Le vede il capo che giudica (D420). Un compito rifatto le ha tutte, anche
/// quelle della volta prima: sono dello stesso compito del piano.
pub fn fatte(dot: &str, compito: &d::Compito) -> Vec<(String, String, String)> {
    let Some((progetto, incarico)) = pr::progetto_dal_testo(&compito.testo) else {
        return Vec::new();
    };
    pr::leggi(&crate::progetti::radice(&progetto))
        .into_iter()
        .filter_map(|e| match e {
            Evento::Domanda {
                da,
                a,
                incarico: i,
                domanda,
                risposta,
                ..
            } if da == dot && i == incarico => Some((a, domanda, risposta)),
            _ => None,
        })
        .collect()
}

/// Il messaggio di un turno con in coda la posta arrivata mentre il Dot
/// lavorava (D420), che da li' risulta letta. Senza posta nuova, il
/// messaggio com'e'.
pub fn con_la_posta_arrivata(c: &d::Cartella, compito: u64, messaggio: String) -> String {
    let posta = c.non_letta();
    let (t, letta) = d::con_la_posta_arrivata(&messaggio, &posta);
    let Some(n) = letta else {
        return messaggio;
    };
    if let Err(e) = c.segna_letta(n) {
        tracing::warn!(errore = %e, "la posta letta non si segna");
    }
    let quanti = posta.iter().filter(|m| m.n <= n).count();
    diario(c, compito, json!({ "tipo": "posta", "messaggi": quanti }));
    t
}

/// Una riga nel diario del Dot.
fn diario(c: &d::Cartella, compito: u64, campi: serde_json::Value) {
    let mut riga = json!({ "quando": crate::decisioni::adesso(), "compito": compito });
    if let (Some(r), serde_json::Value::Object(x)) = (riga.as_object_mut(), campi) {
        r.extend(x);
    }
    if let Err(e) = c.diario(&riga) {
        tracing::warn!(errore = %e, "il diario del Dot non si scrive");
    }
}

/// Un messaggio del Dot nella chat del progetto. Se non si scrive lo si dice
/// nel suo diario e si va avanti: il compito vale piu' del messaggio.
fn alla_chat(
    server: &Arc<Server>,
    c: &d::Cartella,
    nome: &str,
    compito: u64,
    progetto: &str,
    m: &str,
) {
    let scritto = crate::dot::nel_gruppo_del_progetto(progetto, nome)
        .ok_or_else(|| format!("la chat del progetto «{progetto}» non c'e'"))
        .and_then(|g| crate::dot::scrivi(server, nome, &format!("gruppo:{g}"), m));
    diario(
        c,
        compito,
        match scritto {
            Ok(_) => json!({ "tipo": "alla_chat", "testo": dd::tagliato(m, 1_000) }),
            Err(e) => json!({ "tipo": "alla_chat", "testo": dd::tagliato(m, 1_000), "errore": e }),
        },
    );
}

/// La squadra del progetto: chi ha un compito del piano, e chi fa un ruolo.
fn squadra(p: &Progetto) -> Vec<String> {
    let mut s: Vec<String> = p
        .lavori
        .values()
        .map(|l| l.dot.clone())
        .chain(p.squadra.values().cloned())
        .collect();
    s.sort();
    s.dedup();
    s
}

/// Chi e', per la domanda: il ruolo dal suo `dot.json`, se c'e'.
fn ruolo_di(nome: &str) -> String {
    d::Cartella::di(&crate::dot::base(), nome)
        .and_then(|c| c.dot())
        .map(|x| x.ruolo.trim().to_string())
        .unwrap_or_default()
}

/// La risposta a una domanda, o perche' non c'e': in tutti e due i casi e'
/// un testo che torna al Dot, perche' sappia come stanno le cose.
async fn rispondi(
    p: Option<&Progetto>,
    progetto: &str,
    chiede: &str,
    compito: &d::Compito,
    q: &dd::Domanda,
) -> String {
    let Some(p) = p else {
        return format!("(nessuna risposta: il progetto «{progetto}» non si legge)");
    };
    let chi = match dd::chi(&q.a, chiede, &squadra(p)) {
        Ok(x) => x,
        Err(e) => return format!("(nessuna risposta: {e})"),
    };
    let (chi_e, sa) = match &chi {
        dd::Chi::Legale => {
            let note = if p.note_legali.is_empty() {
                String::new()
            } else {
                format!(
                    "Le tue note sul progetto, finora:\n- {}",
                    p.note_legali.join("\n- ")
                )
            };
            ("Sei il legale".to_string(), note)
        }
        dd::Chi::Apm => (
            "Sei l'APM, che guida il progetto".to_string(),
            format!(
                "Il progetto adesso:\n{}",
                dd::tagliato(&pr::racconto(p, None), 6_000)
            ),
        ),
        dd::Chi::Collega(n) => {
            let ruolo = ruolo_di(n);
            let consegne: Vec<String> = p
                .lavori
                .iter()
                .filter(|(_, l)| &l.dot == n)
                .map(|(i, l)| {
                    let stato = l.stato.map_or("in corso", d::nome_stato);
                    format!("- {i} ({stato}): {}", pr::in_breve(&l.esito))
                })
                .collect();
            let sa = if consegne.is_empty() {
                "Nel progetto non hai ancora consegnato niente.".to_string()
            } else {
                format!("Le tue consegne nel progetto:\n{}", consegne.join("\n"))
            };
            let chi_e = if ruolo.is_empty() {
                format!("Sei «{n}»")
            } else {
                format!("Sei «{n}». Il tuo ruolo: {ruolo}")
            };
            (chi_e, dd::tagliato(&sa, 8_000))
        }
    };
    let x = dd::Contesto {
        progetto,
        richiesta: &p.richiesta,
        chiede,
        compito: &compito.testo,
    };
    let domanda = dd::domanda_a(&chi, &chi_e, &sa, &x, &q.testo);
    let per = match &chi {
        dd::Chi::Legale => d::azienda::NOME_LEGALE.to_string(),
        dd::Chi::Apm => d::azienda::NOME_APM.to_string(),
        dd::Chi::Collega(n) => n.clone(),
    };
    match crate::apm::chiedi_per_il_progetto(progetto, &per, &domanda, dd::GETTONI).await {
        Ok((r, _)) if !r.trim().is_empty() => r.trim().to_string(),
        Ok(_) => format!("(nessuna risposta: {per} non ha detto niente)"),
        Err(e) => format!("(nessuna risposta: {e})"),
    }
}
