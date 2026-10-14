//! La pagella dei Dot nel demone: il capo giudica, AR decide (D401).
//!
//! Deciso con Gio il 10 ottobre. Quando un Dot che prende compiti consegna,
//! il suo capo (o l'APM, se non ne ha uno) giudica la consegna col cervello
//! piu' grande che risponde a un indirizzo, come il custode e AR: chi legge
//! il lavoro di un altro non deve avere mani. Se la scala non ne ha uno,
//! Claude Code senza mani (D405, `crate::giudice`). Un voto basso fa rifare il
//! compito un gradino piu' su (`nova_core::dot`), e dopo ogni voto AR guarda
//! la pagella e decide se il Dot scende, sale o va detto a Nova.
//!
//! Le regole e le parole stanno in `nova_dot::pagella`, dove si provano senza
//! demone. Se la domanda dovrebbe uscire dal PC si chiede prima a
//! `nova_decisioni` (`GiudizioDelCapo`), e una credenziale dentro non esce.
//! Un capo che non risponde, o un voto che non si legge, non giudica: il
//! compito resta com'e', e lo si scrive nel diario del Dot.

use std::sync::Arc;

use nova_decisioni::Decisione;
use nova_dot as d;
use nova_dot::pagella as p;
use serde_json::{json, Value};

use crate::server::Server;

/// Una riga nel diario del Dot.
fn diario(c: &d::Cartella, compito: u64, campi: Value) {
    let mut riga = json!({ "quando": crate::decisioni::adesso(), "compito": compito });
    if let (Some(r), Value::Object(x)) = (riga.as_object_mut(), campi) {
        r.extend(x);
    }
    if let Err(e) = c.diario(&riga) {
        tracing::warn!(errore = %e, "il diario del Dot non si scrive");
    }
}

/// I nomi della scala di adesso, dal piu' piccolo al piu' grande.
pub fn scala(cfg: &Value) -> Vec<String> {
    crate::agente::scala_di(cfg)
        .iter()
        .map(|g| g.nome().to_string())
        .collect()
}

/// Il capo giudica una consegna. `usato` e' il cervello che l'ha fatta;
/// `rifatto`, se era gia' un rifacimento dopo una bocciatura. Torna il voto,
/// o `None` se non si e' potuto giudicare.
pub async fn giudica(
    server: &Arc<Server>,
    c: &d::Cartella,
    dot: &d::Dot,
    compito: &d::Compito,
    esito: &str,
    usato: &str,
    rifatto: bool,
) -> Option<p::Giudizio> {
    let giudice = if dot.capo.trim().is_empty() {
        d::azienda::NOME_APM
    } else {
        dot.capo.trim()
    };
    let cfg = nova_configurazione::dove::leggi();
    let non_giudicato = |perche: String| {
        diario(
            c,
            compito.id,
            json!({ "tipo": "non_giudicato", "giudice": giudice, "perche": perche }),
        );
    };
    // Un cervello a indirizzo, o Claude Code senza mani (D405).
    let Some(grande) = crate::giudice::Giudice::dalla_scala(&crate::agente::scala_di(&cfg), |_| false)
    else {
        non_giudicato(crate::giudice::SENZA_GIUDICE.into());
        return None;
    };
    let mut consegna = esito.trim().to_string();
    if let Ok(r) = std::fs::read_to_string(c.rapporto(compito.id)) {
        consegna = format!("{consegna}\n\nIl rapporto:\n{}", r.trim());
    }
    // Le domande che ha fatto lavorando, come le ha portate NOVA (D420).
    let consegna = p::con_le_domande_del_dot(&consegna, &crate::domande::fatte(&dot.nome, compito));
    let in_casa = grande.in_casa();
    let solo_locale = crate::dalla_configurazione::scala(&cfg).solo_locale;
    // La consegna intera, o quanto ci sta nella finestra del modello di
    // casa (D419).
    let massimo = p::consegna_in_domanda(
        in_casa,
        crate::dalla_configurazione::modello_di_casa(&cfg).contesto,
    );
    let base = p::domanda(giudice, dot, &compito.testo, &consegna, massimo);
    // Il capo puo' chiedere al Dot prima di votare (D419): chi ha i file e'
    // lui, e un capo che suppone boccia per niente.
    let mut scambi: Vec<(String, String)> = Vec::new();
    let g = loop {
        let ancora = scambi.len() < p::DOMANDE_DEL_CAPO;
        let domanda = p::con_le_risposte(&base, &scambi, ancora);
        // Il pezzo che sembra un segreto si copre, e il resto si giudica (D417).
        let domanda = match crate::custode::da_chiedere(
            Decisione::GiudizioDelCapo,
            giudice,
            in_casa,
            &domanda,
            solo_locale,
        ) {
            Ok(d) => {
                if !d.coperti.is_empty() {
                    crate::custode::annota_coperti(
                        giudice,
                        &format!("il giudizio del compito {} di {}", compito.id, dot.nome),
                        &d.coperti,
                    );
                    diario(c, compito.id, json!({ "tipo": "coperti", "quanti": d.coperti.len() }));
                }
                d.domanda
            }
            Err(e) => {
                non_giudicato(e);
                return None;
            }
        };
        let detto = match grande.chiedi(&domanda, p::GETTONI).await {
            Ok(d) => d,
            Err(e) => {
                non_giudicato(e);
                return None;
            }
        };
        if let Some(g) = p::leggi(&detto) {
            break g;
        }
        let Some(q) = p::leggi_domanda(&detto).filter(|_| ancora) else {
            let corto: String = detto.chars().take(300).collect();
            non_giudicato(format!("il voto non si legge: «{corto}»"));
            return None;
        };
        let risposta = match crate::dot::rispondi(
            server,
            c,
            &dot.nome,
            compito.id,
            &p::al_dot(giudice, &q),
            usato,
        )
        .await
        {
            Ok(r) => r,
            Err(e) => format!("(il Dot non ha potuto rispondere: {e})"),
        };
        diario(
            c,
            compito.id,
            json!({
                "tipo": "domanda_del_capo",
                "giudice": giudice,
                "domanda": q,
                "risposta": d::domande::tagliato(&risposta, 1_000),
            }),
        );
        server.ctx.bus.emit(
            "dot.domanda_del_capo",
            json!({ "dot": dot.nome, "id": compito.id, "giudice": giudice, "domanda": q }),
        );
        scambi.push((q, risposta));
    };

    let quando = crate::decisioni::adesso();
    let riga = p::Riga::Voto {
        quando: quando.clone(),
        compito: compito.id,
        cervello: usato.to_string(),
        voto: g.voto,
        perche: g.perche.clone(),
        giudice: giudice.to_string(),
        rifatto,
    };
    if let Err(e) = c.annota_pagella(&riga) {
        tracing::warn!(dot = %dot.nome, errore = %e, "la pagella non si scrive");
    }
    crate::decisioni::annota(
        &cfg,
        &crate::decisioni::riga_voto(
            &quando, &dot.nome, compito.id, usato, g.voto, &g.perche, giudice, rifatto,
        ),
    );
    diario(
        c,
        compito.id,
        json!({
            "tipo": "voto",
            "voto": g.voto,
            "perche": g.perche,
            "cervello": usato,
            "giudice": giudice,
            "deciso_da": grande.nome(),
            "rifatto": rifatto,
        }),
    );
    server.ctx.bus.emit(
        "dot.voto",
        json!({
            "dot": dot.nome,
            "id": compito.id,
            "voto": g.voto,
            "bocciato": g.bocciato(),
            "cervello": usato,
            "giudice": giudice,
        }),
    );
    decide_ar(server, c, usato, &cfg);
    Some(g)
}

/// AR guarda la pagella dopo un voto, e decide: il Dot scende o sale di un
/// gradino, o lo si dice a Nova. Senza AR non decide nessuno, e i voti
/// restano nella pagella.
fn decide_ar(server: &Arc<Server>, c: &d::Cartella, usato: &str, cfg: &Value) {
    let base = crate::dot::base();
    let c_ar = d::Cartella::di(&base, d::azienda::NOME_AR);
    if !c_ar.as_ref().is_ok_and(|x| {
        x.dot()
            .is_ok_and(|a| a.fisso && a.mestiere == d::Mestiere::Ar)
    }) {
        return;
    }
    let Ok(dot) = c.dot() else { return };
    let attuale = if dot.cervello.is_empty() {
        usato.to_string()
    } else {
        dot.cervello.clone()
    };
    let scala = scala(cfg);
    let (decisione, perche) = p::decidi(&c.pagella(), &attuale, &scala);
    let a = match &decisione {
        p::Decisione::Tieni => return,
        p::Decisione::Scendi(a) | p::Decisione::Sali(a) => {
            if let Err(e) = crate::dot::cambia_cervello(&dot.nome, a, &perche) {
                tracing::warn!(dot = %dot.nome, errore = %e, "AR non riesce a cambiargli il cervello");
                return;
            }
            server.ctx.bus.emit(
                "dot.cervello",
                json!({ "dot": dot.nome, "da": attuale, "a": a, "perche": perche }),
            );
            a.clone()
        }
        p::Decisione::Segnala => {
            let detto = format!(
                "«{}» lavora gia' col cervello piu' grande, «{attuale}», e il suo capo lo boccia \
                 ancora: {perche}. Va deciso se tenerlo cosi', cambiargli il lavoro o licenziarlo.",
                dot.nome
            );
            if let Err(e) = crate::dot::scrivi(server, d::azienda::NOME_AR, d::DA_NOVA, &detto) {
                tracing::warn!(dot = %dot.nome, errore = %e, "AR non riesce a dirlo a Nova");
            }
            String::new()
        }
    };
    let riga = crate::decisioni::riga_cervello_ar(
        &crate::decisioni::adesso(),
        &dot.nome,
        &attuale,
        decisione.nome(),
        &a,
        &perche,
    );
    crate::decisioni::annota(cfg, &riga);
    if let Ok(c_ar) = c_ar {
        if let Err(e) = c_ar.diario(&riga) {
            tracing::warn!(errore = %e, "il diario di AR non si scrive");
        }
    }
}
