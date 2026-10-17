//! Le domande dei Dot mentre lavorano, e la chat del progetto (D419).
//!
//! Le parole e come si leggono stanno in `nova_dot::domande`. Qui c'e' chi
//! fa il resto, dopo ogni turno di un compito di un progetto:
//!
//! - i messaggi `ALLA CHAT:` vanno nella chat del progetto, a nome del Dot;
//! - le domande `CHIEDO A <chi>:` vanno a chi risponde, e la risposta torna
//!   al Dot nello stesso compito. Il legale e l'APM rispondono col cervello
//!   piu' grande e senza mani, come i controlli dell'APM: chi legge testo
//!   scritto da un altro non deve avere mani (D384).
//! - **Un collega risponde lui** (D421): la domanda gli arriva fra un turno
//!   e l'altro del suo lavoro, o subito se e' libero, e lui risponde con
//!   `RISPONDO A <chi>:`, coi suoi file. Chi ha chiesto aspetta; se il
//!   collega non risponde in tempo, risponde NOVA dalle sue consegne, e lo
//!   dice. Due Dot che si chiederebbero a vicenda non si aspettano.
//! - `ASPETTO LA SCHEDA VIDEO: <programma>` ferma il compito finche' quel
//!   programma e' acceso (D423).
//!
//! Ogni domanda con la sua risposta resta scritta: nel diario del progetto
//! (`Evento::Domanda`), perche' un parere chiesto e avuto si ritrova; nella
//! conversazione fra chi chiede e chi risponde, che Gio vede nell'harness
//! (D421: prima finiva nella chat del gruppo, come un monologo); nel diario
//! del Dot, coi suoi passi.

use std::sync::Arc;
use std::time::Duration;

use nova_dot as d;
use nova_dot::domande as dd;
use nova_dot::progetto::{self as pr, Evento, Progetto};
use serde_json::json;

use crate::server::Server;

/// Una domanda fra colleghi che aspetta la risposta (D421): a chi e' fatta,
/// chi la fa, il testo, e da dove arriva la risposta a chi aspetta.
pub struct Aperta {
    pub a: String,
    pub da: String,
    pub testo: String,
    /// Quando e' stata fatta: la risposta, nella loro conversazione, viene
    /// dopo.
    quando: String,
    tx: Option<tokio::sync::oneshot::Sender<String>>,
}

/// L'ora di adesso, ma dopo `prima` (D421): nella conversazione fra due Dot
/// i messaggi si ordinano per ora, al secondo, e una risposta che arriva
/// nello stesso secondo della domanda le finirebbe davanti.
async fn dopo_di(prima: &str) -> String {
    loop {
        let ora = crate::decisioni::adesso();
        if ora.as_str() > prima {
            return ora;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// Quanto si aspetta la risposta di un collega, di serie (D421). Si cambia
/// con `dots.secondi_per_rispondere`.
pub const SECONDI_PER_RISPONDERE: u64 = 30 * 60;

/// Ogni quanto si guarda se il programma che occupa la scheda video e' ancora
/// acceso, di serie (D423). Si cambia con
/// `dots.secondi_fra_i_controlli_della_scheda`.
pub const SECONDI_FRA_I_CONTROLLI: u64 = 60;

/// Quanto si aspetta, al massimo, un programma nominato senza PID (D425),
/// prima di farla riguardare al Dot. Si cambia con
/// `dots.secondi_per_riguardare_la_scheda`.
pub const SECONDI_PER_RIGUARDARE_LA_SCHEDA: u64 = 60 * 60;

/// Quanti controlli si aspettano, quando non si sa cosa occupa la scheda,
/// prima di farla riguardare al Dot: un quarto d'ora coi valori di serie.
pub const CONTROLLI_SENZA_NOME: u64 = 15;

/// Quanti turni di un compito, al massimo, solo per rispondere ai colleghi.
pub const SOLO_RISPOSTE: usize = 3;

/// I turni di un compito che non contano fra quelli concessi: dopo le
/// risposte a una domanda, dopo l'attesa della scheda video, e quelli in cui
/// il Dot ha solo risposto ai colleghi.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Conto {
    pub domande: usize,
    pub scheda: usize,
    pub solo_risposte: usize,
    pub fuori: usize,
    /// Le risposte dell'utente avute nel compito, con la domanda (D429):
    /// una domanda uguale non gli si rifa', e i passi dopo le ricevono.
    pub dall_utente: Vec<(String, String)>,
}

impl Conto {
    /// Quanti turni fuori conto ci possono essere, al massimo.
    pub const MASSIMO_FUORI: usize =
        dd::GIRI_PER_COMPITO + 2 + dd::ATTESE_DELLA_SCHEDA + SOLO_RISPOSTE;
}

fn secondi_da_configurazione(chiave: &str, se_manca: u64) -> u64 {
    nova_configurazione::dove::leggi()
        .get("dots")
        .and_then(|x| x.get(chiave))
        .and_then(serde_json::Value::as_u64)
        .filter(|s| *s > 0)
        .unwrap_or(se_manca)
}

/// Le domande dei colleghi che aspettano la risposta di `nome`: chi chiede,
/// e cosa.
pub fn aspettano(server: &Arc<Server>, nome: &str) -> Vec<(String, String)> {
    server
        .dots
        .aperte
        .lock()
        .map(|v| {
            v.iter()
                .filter(|x| x.a == nome)
                .map(|x| (x.da.clone(), x.testo.clone()))
                .collect()
        })
        .unwrap_or_default()
}

/// Com'e' andato un turno, guardato dopo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dopo {
    /// Il turno e' la consegna: questo e' il testo, senza le righe per NOVA.
    Consegna(String),
    /// Il Dot ha chiesto: riprende con questo messaggio, che porta le
    /// risposte.
    Riprendi(String),
}

/// Guarda quello che il Dot ha detto in un turno del compito: porta le
/// risposte ai colleghi (D421), scrive i messaggi alla chat, porta le
/// domande, aspetta la scheda video (D423). `conto` tiene il conto di
/// queste cose nel compito. Un compito che non e' di un progetto non ha ne'
/// chat ne' squadra: quello che ha detto e' la consegna.
pub async fn dopo_il_turno(
    server: &Arc<Server>,
    c: &d::Cartella,
    nome: &str,
    compito: &d::Compito,
    detto: &str,
    conto: &mut Conto,
) -> Dopo {
    let x = dd::dividi(detto);
    for r in &x.risposte {
        risposta_al_collega(server, c, nome, compito.id, r).await;
    }
    let progetto = pr::progetto_dal_testo(&compito.testo);
    let Some((progetto, incarico)) = progetto else {
        if x.risposte.is_empty() && x.scheda.is_none() {
            return Dopo::Consegna(detto.to_string());
        }
        return Dopo::Consegna(x.resto);
    };
    for m in &x.alla_chat {
        alla_chat(server, c, nome, compito.id, &progetto, m);
    }
    // Le domande, e poi la scheda video: se ha chiesto e aspetta anche la
    // scheda, riprende con le risposte quando la scheda e' libera.
    let mut seguito: Vec<String> = Vec::new();
    if !x.domande.is_empty() {
        match domande(
            server, c, nome, compito, &progetto, &incarico, &x, detto, conto,
        )
        .await
        {
            Ok(t) => seguito.push(t),
            Err(consegna) => return Dopo::Consegna(consegna),
        }
    }
    if let Some(programma) = &x.scheda {
        if conto.scheda < dd::ATTESE_DELLA_SCHEDA {
            conto.scheda += 1;
            if seguito.is_empty() {
                conto.fuori += 1;
            }
            seguito.push(aspetta_la_scheda(server, c, nome, compito.id, programma).await);
        } else {
            diario(
                c,
                compito.id,
                json!({ "tipo": "scheda_finita", "programma": programma }),
            );
        }
    }
    if !seguito.is_empty() {
        return Dopo::Riprendi(seguito.join("\n\n"));
    }
    // Ha risposto ai colleghi e basta: si continua, qualche volta.
    if x.resto.trim().is_empty() && !x.risposte.is_empty() && conto.solo_risposte < SOLO_RISPOSTE {
        conto.solo_risposte += 1;
        conto.fuori += 1;
        return Dopo::Riprendi(dd::HAI_RISPOSTO.to_string());
    }
    Dopo::Consegna(x.resto)
}

/// Le domande di un turno: le porta, e torna il messaggio con le risposte.
/// `Err` con la consegna, se ha gia' chiesto quanto poteva e ha anche
/// consegnato.
#[allow(clippy::too_many_arguments)]
async fn domande(
    server: &Arc<Server>,
    c: &d::Cartella,
    nome: &str,
    compito: &d::Compito,
    progetto: &str,
    incarico: &str,
    x: &dd::Detto,
    detto: &str,
    conto: &mut Conto,
) -> Result<String, String> {
    if conto.domande >= dd::GIRI_PER_COMPITO {
        // Ha gia' chiesto quanto poteva: se ha anche consegnato, vale la
        // consegna; se no gli si dice, una volta, di consegnare. Se chiede
        // ancora, quello che ha detto e' la consegna, com'e'.
        conto.domande += 1;
        diario(
            c,
            compito.id,
            json!({ "tipo": "domande_finite", "domande": x.domande.len() }),
        );
        return if !x.resto.trim().is_empty() {
            Err(x.resto.clone())
        } else if conto.domande == dd::GIRI_PER_COMPITO + 1 {
            conto.fuori += 1;
            Ok(dd::BASTA.to_string())
        } else {
            Err(detto.trim().to_string())
        };
    }
    conto.domande += 1;
    conto.fuori += 1;
    let p = crate::apm::leggi(progetto);
    let mut risposte = Vec::new();
    for q in x.domande.iter().take(dd::DOMANDE_PER_GIRO) {
        // Una domanda all'utente a cui ha gia' risposto nel compito non gli
        // si rifa' (D429): torna la sua risposta di prima.
        let prima = matches!(dd::chi(&q.a, nome, &[]), Ok(dd::Chi::Utente))
            .then(|| {
                conto
                    .dall_utente
                    .iter()
                    .find(|(d, _)| dd::stessa_domanda(d, &q.testo))
                    .map(|(_, r)| r.clone())
            })
            .flatten();
        let gia = prima.is_some();
        let r = match prima {
            Some(prima) => dd::gia_risposta(&prima),
            None => {
                let (r, dall_utente) =
                    rispondi(server, c, p.as_ref(), progetto, nome, compito, q).await;
                if let Some(a) = dall_utente {
                    conto.dall_utente.push((q.testo.trim().to_string(), a));
                }
                r
            }
        };
        let mut riga = json!({
            "tipo": "domanda",
            "a": q.a,
            "domanda": q.testo,
            "risposta": dd::tagliato(&r, 1_000),
        });
        if gia {
            riga["gia_risposta"] = json!(true);
        }
        diario(c, compito.id, riga);
        crate::apm::annota(
            progetto,
            Evento::Domanda {
                quando: crate::decisioni::adesso(),
                da: nome.to_string(),
                a: q.a.clone(),
                incarico: incarico.to_string(),
                domanda: q.testo.clone(),
                risposta: r.clone(),
            },
        );
        server.ctx.bus.emit(
            "dot.domanda",
            json!({ "dot": nome, "id": compito.id, "a": q.a, "domanda": q.testo }),
        );
        risposte.push((q.clone(), r));
    }
    let mut seguito = dd::risposte(&risposte, dd::GIRI_PER_COMPITO - conto.domande);
    if x.domande.len() > dd::DOMANDE_PER_GIRO {
        seguito.push_str(&format!(
            " Avevi fatto {} domande: ho portato le prime {}, le altre richiedile se ti servono \
             ancora.",
            x.domande.len(),
            dd::DOMANDE_PER_GIRO
        ));
    }
    Ok(seguito)
}

/// Un Dot risponde a un collega che gli ha chiesto (D421). La risposta va
/// nella loro conversazione, e a chi aspetta; se non aspetta piu', perche'
/// e' passato troppo tempo, gli arriva come un messaggio, cosi' non si perde.
async fn risposta_al_collega(
    server: &Arc<Server>,
    c: &d::Cartella,
    nome: &str,
    compito: u64,
    r: &dd::Domanda,
) {
    let attesa = server.dots.aperte.lock().ok().and_then(|mut v| {
        let i = v.iter().position(|x| x.a == nome && x.da == r.a)?;
        Some(v.remove(i))
    });
    let quando_domanda = attesa
        .as_ref()
        .map(|x| x.quando.clone())
        .unwrap_or_default();
    let consegnata = match attesa.and_then(|mut x| x.tx.take()) {
        Some(tx) => {
            let quando = dopo_di(&quando_domanda).await;
            if let Err(e) = c.spedito(nome, &r.a, r.testo.trim(), &quando) {
                tracing::warn!(errore = %e, "la copia della risposta non si scrive");
            }
            tx.send(r.testo.trim().to_string()).is_ok()
        }
        None => false,
    };
    if !consegnata {
        if let Err(e) = crate::dot::scrivi(server, nome, &r.a, r.testo.trim()) {
            tracing::warn!(errore = %e, "la risposta al collega non arriva");
        }
    }
    diario(
        c,
        compito,
        json!({
            "tipo": "risposta",
            "a": r.a,
            "testo": dd::tagliato(&r.testo, 1_000),
            "aspettava": consegnata,
        }),
    );
    server
        .ctx
        .bus
        .emit("dot.risposta", json!({ "dot": nome, "a": r.a }));
}

/// Un Dot libero risponde ai colleghi che aspettano (D421): un turno nella
/// sua conversazione, coi suoi file. Se ha risposto a uno solo senza la
/// riga `RISPONDO A`, quello che ha detto e' la risposta.
pub async fn rispondi_da_libero(server: &Arc<Server>, c: &d::Cartella, nome: &str) {
    let in_attesa = aspettano(server, nome);
    if in_attesa.is_empty() {
        return;
    }
    let Ok(dot) = c.dot() else { return };
    let testo = dd::rispondi_ai_colleghi(&in_attesa);
    match crate::dot::rispondi(
        server,
        c,
        nome,
        0,
        &testo,
        &dot.cervello,
        "risposta ai colleghi",
    )
    .await
    {
        Ok(detto) => {
            let x = dd::dividi(&detto);
            if x.risposte.is_empty() && in_attesa.len() == 1 && !detto.trim().is_empty() {
                let r = dd::Domanda {
                    a: in_attesa[0].0.clone(),
                    testo: detto.trim().to_string(),
                };
                risposta_al_collega(server, c, nome, 0, &r).await;
            } else {
                for r in &x.risposte {
                    risposta_al_collega(server, c, nome, 0, r).await;
                }
            }
        }
        Err(e) => diario(c, 0, json!({ "tipo": "risposta", "errore": e })),
    }
}

/// Aspetta che il programma che occupa la scheda video si chiuda (D423), e
/// torna il messaggio con cui il Dot riprende. Coi PID (D425) si aspetta
/// proprio quei processi, fino all'attesa massima dei Dot; col solo nome al
/// massimo un'ora, poi la riguarda il Dot, perche' con lo stesso nome possono
/// esserci programmi che con la scheda non c'entrano. Non si sa cosa la
/// occupa, o NOVA non vede i programmi accesi: si aspetta un quarto d'ora, e
/// la riguarda il Dot.
async fn aspetta_la_scheda(
    server: &Arc<Server>,
    c: &d::Cartella,
    nome: &str,
    compito: u64,
    detto: &str,
) -> String {
    let o = dd::chi_occupa(detto);
    let ogni = secondi_da_configurazione(
        "secondi_fra_i_controlli_della_scheda",
        SECONDI_FRA_I_CONTROLLI,
    );
    diario(
        c,
        compito,
        json!({ "tipo": "aspetta_scheda", "programma": o.programma, "pid": o.pid }),
    );
    server.ctx.bus.emit(
        "dot.aspetta_scheda",
        json!({ "dot": nome, "id": compito, "programma": o.programma, "pid": o.pid }),
    );
    let acceso = || -> Option<bool> {
        let tutti = nova_platform::processi::elenca().ok()?;
        Some(if o.pid.is_empty() {
            tutti
                .iter()
                .any(|p| dd::stesso_programma(&p.nome, &o.programma))
        } else {
            tutti.iter().any(|p| o.pid.contains(&p.pid))
        })
    };
    if (o.programma.is_empty() && o.pid.is_empty()) || acceso().is_none() {
        let secondi = ogni * CONTROLLI_SENZA_NOME;
        tokio::time::sleep(Duration::from_secs(secondi)).await;
        diario(
            c,
            compito,
            json!({ "tipo": "scheda", "esito": "da_riguardare" }),
        );
        return dd::scheda_da_riguardare(secondi.div_ceil(60));
    }
    let limite = if o.pid.is_empty() {
        secondi_da_configurazione(
            "secondi_per_riguardare_la_scheda",
            SECONDI_PER_RIGUARDARE_LA_SCHEDA,
        ) as i64
    } else {
        crate::dot::ATTESA_MASSIMA_S
    };
    let fine = nova_platform::orologio::adesso() + limite;
    loop {
        if acceso() == Some(false) {
            diario(
                c,
                compito,
                json!({ "tipo": "scheda", "esito": "libera", "programma": o.programma, "pid": o.pid }),
            );
            return dd::scheda_libera(&o);
        }
        if nova_platform::orologio::adesso() >= fine {
            if o.pid.is_empty() {
                diario(
                    c,
                    compito,
                    json!({ "tipo": "scheda", "esito": "da_riguardare", "programma": o.programma }),
                );
                return dd::scheda_forse_altro(&o, (limite as u64).div_ceil(60));
            }
            diario(
                c,
                compito,
                json!({ "tipo": "scheda", "esito": "occupata", "programma": o.programma, "pid": o.pid }),
            );
            return dd::scheda_ancora_occupata(&o, (limite / 3600) as u64);
        }
        tokio::time::sleep(Duration::from_secs(ogni)).await;
    }
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
/// lavorava (D420), che da li' risulta letta, e le domande dei colleghi che
/// aspettano la sua risposta (D421). Senza niente di nuovo, il messaggio
/// com'e'.
pub fn con_la_posta_arrivata(
    server: &Arc<Server>,
    c: &d::Cartella,
    nome: &str,
    compito: u64,
    messaggio: String,
) -> String {
    let posta = c.non_letta();
    let (t, letta) = d::con_la_posta_arrivata(&messaggio, &posta);
    if let Some(n) = letta {
        if let Err(e) = c.segna_letta(n) {
            tracing::warn!(errore = %e, "la posta letta non si segna");
        }
        let quanti = posta.iter().filter(|m| m.n <= n).count();
        diario(c, compito, json!({ "tipo": "posta", "messaggi": quanti }));
    }
    format!("{t}{}", dd::domande_per_te(&aspettano(server, nome)))
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
/// un testo che torna al Dot, perche' sappia come stanno le cose. Un
/// collega risponde lui (D421); il legale e l'APM col cervello grande, e la
/// loro risposta resta nella conversazione con chi ha chiesto. Il secondo
/// valore e' la risposta dell'utente, se ha risposto lui (D429).
async fn rispondi(
    server: &Arc<Server>,
    c: &d::Cartella,
    p: Option<&Progetto>,
    progetto: &str,
    chiede: &str,
    compito: &d::Compito,
    q: &dd::Domanda,
) -> (String, Option<String>) {
    let Some(p) = p else {
        return (
            format!("(nessuna risposta: il progetto «{progetto}» non si legge)"),
            None,
        );
    };
    let chi = match dd::chi(&q.a, chiede, &squadra(p)) {
        Ok(x) => x,
        Err(e) => return (format!("(nessuna risposta: {e})"), None),
    };
    if let dd::Chi::Collega(n) = &chi {
        return (
            chiedi_al_collega(server, c, p, progetto, chiede, compito, n, q).await,
            None,
        );
    }
    if chi == dd::Chi::Utente {
        return chiedi_all_utente(server, c, progetto, chiede, compito, q).await;
    }
    let quando = crate::decisioni::adesso();
    let r = dal_cervello_grande(p, progetto, chiede, compito, q, &chi).await;
    // La conversazione fra chi chiede e chi risponde, per l'harness.
    let per = match &chi {
        dd::Chi::Apm => d::azienda::NOME_APM,
        _ => d::azienda::NOME_LEGALE,
    };
    let _ = c.spedito(chiede, per, q.testo.trim(), &quando);
    if let Ok(x) = d::Cartella::di(&crate::dot::base(), per) {
        if x.esiste() {
            let _ = x.spedito(per, chiede, &r, &dopo_di(&quando).await);
        }
    }
    (r, None)
}

/// Quanto si aspetta la risposta dell'utente, di serie (D427): l'attesa
/// massima dei Dot. Si cambia con `dots.secondi_per_l_utente`.
pub const SECONDI_PER_L_UTENTE: u64 = crate::dot::ATTESA_MASSIMA_S as u64;

/// Chiede all'utente, tramite Nova, e aspetta la sua risposta (D427). Il 15
/// ottobre i Dot di S0ex1 avevano bisogno di una decisione di Gio, e non
/// avevano modo di chiedergliela: la fase e' fallita tre volte. Torna il
/// testo per il Dot e, se l'utente ha risposto, la sua risposta (D429).
async fn chiedi_all_utente(
    server: &Arc<Server>,
    c: &d::Cartella,
    progetto: &str,
    chiede: &str,
    compito: &d::Compito,
    q: &dd::Domanda,
) -> (String, Option<String>) {
    let secondi = secondi_da_configurazione("secondi_per_l_utente", SECONDI_PER_L_UTENTE);
    let incarico = pr::progetto_dal_testo(&compito.testo)
        .map(|(_, i)| i)
        .unwrap_or_default();
    let (tx, rx) = tokio::sync::oneshot::channel();
    if let Ok(mut m) = server.dots.per_l_utente.lock() {
        m.insert(chiede.to_string(), tx);
    }
    let testo = dd::all_utente(chiede, progetto, &incarico, &q.testo);
    if let Err(e) = crate::dot::scrivi(server, chiede, d::DA_NOVA, &testo) {
        if let Ok(mut m) = server.dots.per_l_utente.lock() {
            m.remove(chiede);
        }
        return (
            format!("(nessuna risposta: la domanda non arriva all'utente: {e})"),
            None,
        );
    }
    diario(
        c,
        compito.id,
        json!({ "tipo": "all_utente", "domanda": dd::tagliato(&q.testo, 1_000) }),
    );
    // Con la voce accesa, glielo si dice anche (come la fine di un progetto).
    let cfg = crate::caps_voce::configurazione_utente().unwrap_or_else(|| json!({}));
    if crate::dot::si_dice_a_voce(&cfg, crate::live::in_corso()) {
        let bus = server.ctx.bus.clone();
        let frase = format!("{chiede} ha una domanda per te, sul progetto {progetto}.");
        tokio::spawn(async move { crate::caps_voce::annuncia(bus, &frase).await });
    }
    match tokio::time::timeout(Duration::from_secs(secondi), rx).await {
        Ok(Ok(r)) => {
            diario(c, compito.id, json!({ "tipo": "dall_utente" }));
            (r.clone(), Some(r))
        }
        _ => {
            if let Ok(mut m) = server.dots.per_l_utente.lock() {
                m.remove(chiede);
            }
            diario(
                c,
                compito.id,
                json!({ "tipo": "dall_utente", "esito": "non_ha_risposto" }),
            );
            (dd::utente_non_ha_risposto(secondi.div_ceil(60)), None)
        }
    }
}

/// Chiede a un collega, e aspetta la sua risposta (D421). Se il collega sta
/// aspettando una risposta da chi chiede, o non risponde in tempo, risponde
/// NOVA dalle sue consegne, e lo dice.
#[allow(clippy::too_many_arguments)]
async fn chiedi_al_collega(
    server: &Arc<Server>,
    c: &d::Cartella,
    p: &Progetto,
    progetto: &str,
    chiede: &str,
    compito: &d::Compito,
    collega: &str,
    q: &dd::Domanda,
) -> String {
    let secondi = secondi_da_configurazione("secondi_per_rispondere", SECONDI_PER_RISPONDERE);
    let minuti = secondi.div_ceil(60);
    let a_vicenda = server
        .dots
        .in_attesa
        .lock()
        .is_ok_and(|m| m.get(collega).is_some_and(|x| x == chiede));
    let chi = dd::Chi::Collega(collega.to_string());
    if a_vicenda {
        diario(
            c,
            compito.id,
            json!({ "tipo": "a_vicenda", "collega": collega }),
        );
        let r = dal_cervello_grande(p, progetto, chiede, compito, q, &chi).await;
        return dd::aspetta_te(collega, &r);
    }
    let quando = crate::decisioni::adesso();
    let _ = c.spedito(chiede, collega, q.testo.trim(), &quando);
    let (tx, rx) = tokio::sync::oneshot::channel();
    if let Ok(mut v) = server.dots.aperte.lock() {
        v.push(Aperta {
            a: collega.to_string(),
            da: chiede.to_string(),
            testo: q.testo.trim().to_string(),
            quando,
            tx: Some(tx),
        });
    }
    if let Ok(mut m) = server.dots.in_attesa.lock() {
        m.insert(chiede.to_string(), collega.to_string());
    }
    server.dots.sveglia(collega);
    let arrivata = tokio::time::timeout(Duration::from_secs(secondi), rx).await;
    if let Ok(mut m) = server.dots.in_attesa.lock() {
        m.remove(chiede);
    }
    match arrivata {
        Ok(Ok(r)) => r,
        _ => {
            // Non aspetta piu': la domanda si toglie, e se il collega
            // risponde dopo, la risposta arriva come un messaggio.
            if let Ok(mut v) = server.dots.aperte.lock() {
                v.retain(|x| !(x.a == collega && x.da == chiede));
            }
            let r = dal_cervello_grande(p, progetto, chiede, compito, q, &chi).await;
            dd::non_ha_risposto(collega, minuti, &r)
        }
    }
}

/// La risposta del cervello grande, senza mani, a nome di chi: il legale,
/// l'APM, o un collega dalle sue consegne.
async fn dal_cervello_grande(
    p: &Progetto,
    progetto: &str,
    chiede: &str,
    compito: &d::Compito,
    q: &dd::Domanda,
    chi: &dd::Chi,
) -> String {
    let (chi_e, sa) = match chi {
        // All'utente risponde l'utente (D427): qui non si arriva.
        dd::Chi::Utente => {
            return "(nessuna risposta: la domanda e' dell'utente, e risponde lui)".into()
        }
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
    let domanda = dd::domanda_a(chi, &chi_e, &sa, &x, &q.testo);
    let per = match chi {
        dd::Chi::Legale => d::azienda::NOME_LEGALE.to_string(),
        dd::Chi::Apm => d::azienda::NOME_APM.to_string(),
        dd::Chi::Collega(n) => n.clone(),
        dd::Chi::Utente => dd::UTENTE.to_string(),
    };
    match crate::apm::chiedi_per_il_progetto(progetto, &per, &domanda, dd::GETTONI).await {
        Ok((r, _)) if !r.trim().is_empty() => r.trim().to_string(),
        Ok(_) => format!("(nessuna risposta: {per} non ha detto niente)"),
        Err(e) => format!("(nessuna risposta: {e})"),
    }
}
