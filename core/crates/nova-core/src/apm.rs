//! L'APM nel demone: porta un progetto dall'inizio alla fine (D402).
//!
//! L'ottavo passo dell'azienda dei Dot (D395, `docs/dots.md`). Deciso con
//! Gio il 10 ottobre, sulla bozza: l'APM lavora **a regole fisse**, e il
//! cervello grande serve ai controlli e al resoconto.
//!
//! 1. **Nasce**: Nova, su richiesta dell'utente, gli passa un progetto
//!    (`dot.progetto`). Uno alla volta: un altro aspetta in coda.
//! 2. **Il piano**: lo chiede all'Architetto (D400).
//! 3. **Il legale all'inizio**, sul piano.
//! 4. **La squadra**: per ogni ruolo «assumi» del piano chiede ad AR
//!    (D397), che riprende o assume, col cervello. I reparti ci sono gia'.
//! 5. **Il via**: scrive a Nova piano, squadra, tetto, dubbi del legale e
//!    domande dell'Architetto, e aspetta che l'utente lo dia (`dot.via`).
//! 6. **Il lavoro**: fase dopo fase, affida ogni compito appena quelli da
//!    cui dipende sono fatti. Il capo giudica ogni consegna (D401).
//! 7. **La fase**: chiusi i compiti, la revisione la guarda contro il suo
//!    «fatta quando», e poi il legale. Non fatta: l'Architetto la rivede, al
//!    piu' due volte; poi decide l'utente.
//! 8. **Il rilascio**: il legale guarda tutto, e Nova riceve il resoconto.
//!
//! **Il tetto**: contano solo i cervelli a consumo, come per il tetto della
//! sessione: ogni turno a consumo costa la stima di una delega
//! (`costo_stimato_delega`). Al 90% l'APM si ferma e chiede.
//!
//! **Si ferma** quando lo ferma l'utente (`dot.ferma_progetto`), quando il
//! tetto sta per finire, e quando qualcosa va deciso da lui: il legale ha
//! dei dubbi, un controllo non si e' potuto fare, una fase non va. Riparte
//! con `dot.via`. Ogni passo sta in `progetti/<nome>/progetto.jsonl`, e il
//! demone che si riaccende riprende da li'.

use std::path::PathBuf;
use std::sync::Arc;

use nova_decisioni::Decisione;
use nova_dot as d;
use nova_dot::piano;
use nova_dot::progetto::{self as pr, DoveSta, Evento, Progetto};
use serde_json::{json, Value};

use crate::server::Server;

/// Ogni quanto l'APM riguarda il progetto anche se nessuno lo sveglia.
const RIGUARDA: std::time::Duration = std::time::Duration::from_secs(3);

/// Quanti gettoni hanno il legale e la revisione: poche righe. Il
/// resoconto ne ha di piu'.
const GETTONI_CONTROLLO: u32 = 400;
const GETTONI_RESOCONTO: u32 = 1_500;

fn adesso() -> String {
    crate::decisioni::adesso()
}

fn nova() -> PathBuf {
    crate::mondo::cartella_nova()
}

fn cartella(nome: &str) -> PathBuf {
    pr::cartella(&nova(), nome)
}

/// Il progetto com'e' adesso, se c'e'.
pub fn leggi(nome: &str) -> Option<Progetto> {
    Progetto::da(nome, &pr::leggi(&cartella(nome)))
}

/// Il piano su cui lavora un progetto, e il suo file.
fn piano_di(p: &Progetto) -> Option<(piano::Piano, String)> {
    if p.versione == 0 {
        return None;
    }
    let f = piano::file(&piano::cartella_del_progetto(&nova(), &p.nome), p.versione);
    let t = std::fs::read_to_string(&f).ok()?;
    let (pi, _) = piano::leggi(&t, p.versione).ok()?;
    Some((pi, f.display().to_string()))
}

/// Un passo nel diario del progetto, e il racconto riscritto.
fn annota(nome: &str, e: Evento) {
    let dir = cartella(nome);
    if let Err(x) = pr::annota(&dir, &e) {
        tracing::warn!(progetto = nome, errore = %x, "il diario del progetto non si scrive");
        return;
    }
    if let Some(p) = leggi(nome) {
        let piano = piano_di(&p).map(|(x, _)| x);
        if let Err(x) = pr::scrivi_racconto(&dir, &pr::racconto(&p, piano.as_ref())) {
            tracing::warn!(progetto = nome, errore = %x, "il racconto del progetto non si scrive");
        }
    }
}

/// L'APM scrive a Nova: arriva in chat.
fn a_nova(server: &Arc<Server>, testo: &str) {
    if let Err(e) = crate::dot::scrivi(server, d::azienda::NOME_APM, d::DA_NOVA, testo) {
        tracing::warn!(errore = %e, "l'APM non riesce a scrivere a Nova");
    }
}

/// Si ferma, e lo dice a Nova.
fn ferma_e_di(server: &Arc<Server>, nome: &str, perche: &str) {
    annota(
        nome,
        Evento::Fermo {
            quando: adesso(),
            perche: perche.to_string(),
        },
    );
    a_nova(
        server,
        &format!("Il progetto «{nome}» e' fermo: {perche}. Ripartira' quando me lo dirai."),
    );
}

/// Sveglia l'APM: un compito di un progetto si e' chiuso, o e' arrivato il
/// via.
pub fn sveglia(server: &Arc<Server>) {
    server.dots.apm.notify_one();
}

fn apm_c_e() -> Result<(), String> {
    let c = d::Cartella::di(&crate::dot::base(), d::azienda::NOME_APM)?;
    if c.dot()
        .is_ok_and(|x| x.fisso && x.mestiere == d::Mestiere::Apm)
    {
        Ok(())
    } else {
        Err("l'APM non c'e': nasce coi Dot accesi, insieme al resto dell'azienda".into())
    }
}

/// Il progetto su cui si lavora: il primo nato fra quelli aperti. Gli
/// altri aperti aspettano in coda.
fn attivo() -> Option<Progetto> {
    pr::elenco(&nova())
        .iter()
        .filter_map(|n| leggi(n))
        .filter(Progetto::aperto)
        .min_by(|a, b| a.numero.cmp(&b.numero).then(a.nome.cmp(&b.nome)))
}

/// Com'e' un progetto per chi guarda: in coda se non e' quello attivo.
fn come_sta(p: &Progetto, attivo: Option<&Progetto>) -> DoveSta {
    if p.aperto() && attivo.is_some_and(|a| a.nome != p.nome) {
        DoveSta::InCoda
    } else {
        p.dove
    }
}

/// Nova passa un progetto all'APM (`dot.progetto`).
pub fn crea(
    server: &Arc<Server>,
    nome: &str,
    richiesta: &str,
    cartella_file: &str,
) -> Result<Value, String> {
    crate::dot_accesi::se_spenti()?;
    apm_c_e()?;
    let nome = piano::nome_del_progetto(nome)?;
    if richiesta.trim().is_empty() {
        return Err("all'APM serve sapere cosa si vuole fare".into());
    }
    let cartella_file = cartella_file.trim();
    if !cartella_file.is_empty() && !std::path::Path::new(cartella_file).is_dir() {
        return Err(format!(
            "«{cartella_file}» non e' una cartella: i file del progetto devono esserci"
        ));
    }
    if leggi(&nome).is_some() {
        return Err(format!("il progetto «{nome}» c'e' gia'"));
    }
    let numero = pr::elenco(&nova())
        .iter()
        .filter_map(|n| leggi(n))
        .map(|p| p.numero)
        .max()
        .unwrap_or(0)
        + 1;
    annota(
        &nome,
        Evento::Creato {
            quando: adesso(),
            numero,
            richiesta: richiesta.trim().to_string(),
            cartella: cartella_file.to_string(),
        },
    );
    sveglia(server);
    let attivo = attivo();
    let in_coda = attivo.as_ref().is_some_and(|a| a.nome != nome);
    Ok(json!({
        "progetto": nome,
        "in_coda": in_coda,
        "nota": if in_coda {
            "c'e' gia' un progetto in corso: questo aspetta che finisca"
        } else {
            "l'APM chiede il piano all'Architetto; quando ha piano e squadra ti chiede il via in chat"
        },
    }))
}

/// L'utente da' il via (`dot.via`), o fa ripartire un progetto fermo. Il
/// tetto, se c'e', sostituisce quello proposto.
pub fn via(server: &Arc<Server>, nome: &str, tetto: Option<f64>) -> Result<Value, String> {
    crate::dot_accesi::se_spenti()?;
    let p =
        leggi(nome.trim()).ok_or_else(|| format!("non c'e' nessun progetto «{}»", nome.trim()))?;
    if let Some(t) = tetto {
        if !(t.is_finite() && t >= 0.0) {
            return Err("il tetto e' un numero di dollari, da zero in su".into());
        }
    }
    let e = match p.dove {
        DoveSta::AspettaIlVia => Evento::Via {
            quando: adesso(),
            tetto: tetto.unwrap_or(p.tetto),
        },
        DoveSta::Fermo => Evento::Ripreso {
            quando: adesso(),
            tetto: tetto.unwrap_or(p.tetto),
        },
        DoveSta::Finito => return Err(format!("il progetto «{}» e' finito", p.nome)),
        _ => {
            return Err(format!(
                "il progetto «{}» non aspetta il via: si sta preparando, o e' gia' in corso",
                p.nome
            ))
        }
    };
    let ripreso = matches!(e, Evento::Ripreso { .. });
    annota(&p.nome, e);
    sveglia(server);
    let p = leggi(&p.nome).unwrap_or(p);
    Ok(json!({ "progetto": p.nome, "ripreso": ripreso, "tetto": p.tetto, "speso": p.speso }))
}

/// L'utente ferma un progetto (`dot.ferma_progetto`): i Dot che ci
/// lavorano si fermano, e i loro compiti si rifaranno alla ripartenza.
pub fn ferma(server: &Arc<Server>, nome: &str, perche: &str) -> Result<Value, String> {
    let p =
        leggi(nome.trim()).ok_or_else(|| format!("non c'e' nessun progetto «{}»", nome.trim()))?;
    if matches!(p.dove, DoveSta::Fermo | DoveSta::Finito) {
        return Err(format!("il progetto «{}» e' gia' fermo o finito", p.nome));
    }
    let perche = if perche.trim().is_empty() {
        "fermato dall'utente".to_string()
    } else {
        format!("fermato dall'utente: {}", perche.trim())
    };
    annota(
        &p.nome,
        Evento::Fermo {
            quando: adesso(),
            perche,
        },
    );
    let mut fermati = Vec::new();
    for l in p.lavori.values().filter(|l| l.stato.is_none()) {
        if crate::dot::compito_in_corso(server, &l.dot) == Some(l.compito)
            && crate::dot::ferma(server, &l.dot).is_ok()
        {
            fermati.push(l.dot.clone());
        }
    }
    Ok(json!({ "progetto": p.nome, "fermo": true, "dot_fermati": fermati }))
}

/// I progetti, o uno solo col suo racconto (`dot.progetti`).
pub fn stato(nome: &str) -> Result<Value, String> {
    let attivo = attivo();
    if nome.trim().is_empty() {
        let tutti: Vec<Value> = pr::elenco(&nova())
            .iter()
            .filter_map(|n| leggi(n))
            .map(|p| {
                json!({
                    "progetto": p.nome,
                    "sta": come_sta(&p, attivo.as_ref()),
                    "versione": p.versione,
                    "tetto": p.tetto,
                    "speso": p.speso,
                    "creato": p.creato,
                })
            })
            .collect();
        return Ok(json!({ "progetti": tutti }));
    }
    let p =
        leggi(nome.trim()).ok_or_else(|| format!("non c'e' nessun progetto «{}»", nome.trim()))?;
    let pi = piano_di(&p);
    Ok(json!({
        "progetto": p.nome,
        "sta": come_sta(&p, attivo.as_ref()),
        "perche_fermo": p.perche_fermo,
        "piano": pi.as_ref().map(|(_, f)| f.clone()),
        "racconto": pr::racconto(&p, pi.as_ref().map(|(x, _)| x)),
    }))
}

// ------------------------------------------------------------- il giro

/// Accende il giro dell'APM: a ogni sveglia, e ogni [`RIGUARDA`], porta
/// avanti il progetto attivo finche' c'e' qualcosa da fare.
pub fn avvia(server: &Arc<Server>) {
    let s = server.clone();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = s.dots.apm.notified() => {}
                _ = tokio::time::sleep(RIGUARDA) => {}
            }
            if !crate::dot_accesi::adesso().accesi || apm_c_e().is_err() {
                continue;
            }
            // Un passo alla volta, finche' ce n'e'.
            for _ in 0..64 {
                let Some(p) = attivo() else { break };
                if !passo(&s, &p).await {
                    break;
                }
            }
        }
    });
}

/// Un passo del progetto attivo. `true` se ha fatto qualcosa: allora se ne
/// fa subito un altro.
async fn passo(server: &Arc<Server>, p: &Progetto) -> bool {
    match p.dove {
        DoveSta::Prepara => prepara(server, p).await,
        DoveSta::InCorso => lavora(server, p).await,
        DoveSta::InCoda | DoveSta::AspettaIlVia | DoveSta::Fermo | DoveSta::Finito => false,
    }
}

/// Piano, legale dell'inizio, squadra, proposta.
async fn prepara(server: &Arc<Server>, p: &Progetto) -> bool {
    if p.versione == 0 && p.piano_chiesto.is_none() {
        match crate::architetto::chiedi(
            server,
            d::azienda::NOME_APM,
            &p.nome,
            &p.richiesta,
            &p.cartella,
            None,
        ) {
            Ok(v) => {
                let id = v.get("compito").and_then(Value::as_u64).unwrap_or(0);
                annota(
                    &p.nome,
                    Evento::PianoChiesto {
                        quando: adesso(),
                        compito: id,
                        fase: None,
                    },
                );
            }
            Err(e) => ferma_e_di(
                server,
                &p.nome,
                &format!("il piano non si e' potuto chiedere: {e}"),
            ),
        }
        return true;
    }
    if p.piano_chiesto.is_some() {
        return piano_pronto(server, p);
    }
    let Some((pi, file)) = piano_di(p) else {
        ferma_e_di(
            server,
            &p.nome,
            &format!("la versione {} del piano non si legge", p.versione),
        );
        return true;
    };
    if p.legale_inizio.is_none() {
        let materia = format!("Il piano:\n{}", piano::scrivi(&pi));
        return legale(server, p, "all'inizio", &materia, None, false).await;
    }
    if trova_la_squadra(server, p, &pi).await {
        return true;
    }
    if !p.proposto {
        let tetto = pr::tetto_proposto(&pi, ce_consumo());
        a_nova(server, &pr::proposta(p, &pi, &file, tetto));
        annota(
            &p.nome,
            Evento::Proposto {
                quando: adesso(),
                tetto,
            },
        );
        return true;
    }
    false
}

/// Il piano chiesto all'Architetto e' pronto? Se si', lo si prende; se il
/// compito e' andato male, ci si ferma.
fn piano_pronto(server: &Arc<Server>, p: &Progetto) -> bool {
    let Some((id, fase)) = p.piano_chiesto else {
        return false;
    };
    let Ok(c) = d::Cartella::di(&crate::dot::base(), d::azienda::NOME_ARCHITETTO) else {
        return false;
    };
    let Some(compito) = c.compiti().into_iter().find(|x| x.id == id) else {
        ferma_e_di(
            server,
            &p.nome,
            &format!("il compito n. {id} dell'Architetto non c'e'"),
        );
        return true;
    };
    match compito.stato {
        d::Stato::Fatto => {
            spesa(&p.nome, &c, id, "il piano");
            let versione = piano::versioni(&piano::cartella_del_progetto(&nova(), &p.nome))
                .last()
                .copied()
                .unwrap_or(0);
            annota(
                &p.nome,
                Evento::PianoPronto {
                    quando: adesso(),
                    versione,
                    da_fase: fase,
                },
            );
            true
        }
        s if s.chiuso() => {
            ferma_e_di(
                server,
                &p.nome,
                &format!("il piano non si e' fatto: {}", pr::in_breve(&compito.esito)),
            );
            true
        }
        _ => false,
    }
}

/// Per ogni ruolo del piano senza un Dot, chiede ad AR. `true` se ne ha
/// chiesto uno.
async fn trova_la_squadra(server: &Arc<Server>, p: &Progetto, pi: &piano::Piano) -> bool {
    let Some((ruolo, mestiere)) = p.ruoli_da_trovare(pi).into_iter().next() else {
        return false;
    };
    let bisogno = if mestiere == d::Mestiere::Ricercatore {
        format!("{ruolo} (un ricercatore), per il progetto «{}»", p.nome)
    } else {
        format!("{ruolo}, per il progetto «{}»", p.nome)
    };
    match crate::risorse::assumi(server, &bisogno, "", "").await {
        Ok(v) => match v.get("dot").and_then(Value::as_str) {
            Some(dot) => annota(
                &p.nome,
                Evento::Squadra {
                    quando: adesso(),
                    ruolo,
                    dot: dot.to_string(),
                },
            ),
            None => ferma_e_di(
                server,
                &p.nome,
                &format!("AR non ha detto chi fa «{ruolo}»"),
            ),
        },
        Err(e) => ferma_e_di(
            server,
            &p.nome,
            &format!("AR non ha trovato chi fa «{ruolo}»: {e}"),
        ),
    }
    true
}

/// Il lavoro, fase dopo fase.
async fn lavora(server: &Arc<Server>, p: &Progetto) -> bool {
    if p.piano_chiesto.is_some() {
        return piano_pronto(server, p);
    }
    // Un piano piu' nuovo su disco: l'ha rivisto l'utente, con Nova. Si
    // lavora su quello, dalla fase di adesso.
    let ultima = piano::versioni(&piano::cartella_del_progetto(&nova(), &p.nome))
        .last()
        .copied()
        .unwrap_or(0);
    if ultima > p.versione {
        let da_fase = piano_di(p).and_then(|(pi, _)| p.fase_di_adesso(&pi).map(|f| f.n));
        annota(
            &p.nome,
            Evento::PianoPronto {
                quando: adesso(),
                versione: ultima,
                da_fase,
            },
        );
        return true;
    }
    let Some((pi, _)) = piano_di(p) else {
        ferma_e_di(
            server,
            &p.nome,
            &format!("la versione {} del piano non si legge", p.versione),
        );
        return true;
    };
    if trova_la_squadra(server, p, &pi).await {
        return true;
    }
    if pr::tetto_toccato(p.speso, p.tetto) {
        ferma_e_di(
            server,
            &p.nome,
            &format!(
                "la spesa, {:.2} $, e' arrivata al {:.0}% del tetto di {:.2} $: dimmi se allargarlo",
                p.speso,
                pr::SOGLIA_DEL_TETTO * 100.0,
                p.tetto
            ),
        );
        return true;
    }
    // I compiti chiusi nel frattempo.
    let mut fatto = false;
    for (id, l) in p.lavori.iter().filter(|(_, l)| l.stato.is_none()) {
        let Ok(c) = d::Cartella::di(&crate::dot::base(), &l.dot) else {
            continue;
        };
        let Some(x) = c.compiti().into_iter().find(|x| x.id == l.compito) else {
            continue;
        };
        if x.stato.chiuso() {
            spesa(&p.nome, &c, l.compito, &format!("il compito {id}"));
            annota(
                &p.nome,
                Evento::Chiuso {
                    quando: adesso(),
                    incarico: id.clone(),
                    stato: x.stato,
                    esito: pr::in_breve(&x.esito),
                },
            );
            fatto = true;
        }
    }
    if fatto {
        return true;
    }
    // Il legale dopo ogni fase fatta (Gio, 10 ottobre).
    if let Some(f) = pi
        .fasi
        .iter()
        .find(|f| p.fasi.get(&f.n) == Some(&true) && !p.legale_fasi.contains_key(&f.n))
    {
        let materia = format!(
            "La fase {} «{}», e quello che ha consegnato:\n{}",
            f.n,
            f.nome,
            p.consegne(f)
        );
        return legale(
            server,
            p,
            &format!("dopo la fase {}", f.n),
            &materia,
            Some(f.n),
            false,
        )
        .await;
    }
    let Some(f) = p.fase_di_adesso(&pi) else {
        return rilascio(server, p, &pi).await;
    };
    if p.fasi.get(&f.n) == Some(&false) {
        // Non e' andata, e la revisione dell'Architetto non e' partita:
        // si chiede (o si ferma, se ne ha avute troppe).
        return rivedi_la_fase(server, p, f.n, "la fase non e' andata").await;
    }
    // I compiti pronti.
    let pronti = p.pronti_in(f);
    if !pronti.is_empty() {
        for i in pronti {
            let Some(dot) = p.chi_fa(&i.chi) else {
                continue;
            };
            let testo = pr::testo_del_compito(p, &pi, f, i);
            match crate::dot::affida_con(server, &dot, &testo, d::azienda::NOME_APM, None, "") {
                Ok(compito) => annota(
                    &p.nome,
                    Evento::Affidato {
                        quando: adesso(),
                        incarico: i.id.clone(),
                        dot,
                        compito,
                    },
                ),
                Err(e) => {
                    ferma_e_di(
                        server,
                        &p.nome,
                        &format!("il compito {} non si affida a «{dot}»: {e}", i.id),
                    );
                    return true;
                }
            }
        }
        return true;
    }
    if !p.fase_chiusa(f) {
        return false;
    }
    // La fase e' chiusa: si giudica.
    let male = p.andati_male(f);
    if !male.is_empty() {
        let perche = format!("compiti non andati: {}", male.join("; "));
        annota(
            &p.nome,
            Evento::Fase {
                quando: adesso(),
                n: f.n,
                fatta: false,
                perche: perche.clone(),
            },
        );
        return rivedi_la_fase(server, p, f.n, &perche).await;
    }
    let domanda = pr::domanda_revisione(p, f);
    let Some(detto) =
        chiedi_al_grande(server, &p.nome, "la revisione", &domanda, GETTONI_CONTROLLO).await
    else {
        return true;
    };
    match pr::leggi_revisione(&detto) {
        Some((true, perche)) => {
            annota(
                &p.nome,
                Evento::Fase {
                    quando: adesso(),
                    n: f.n,
                    fatta: true,
                    perche,
                },
            );
        }
        Some((false, perche)) => {
            annota(
                &p.nome,
                Evento::Fase {
                    quando: adesso(),
                    n: f.n,
                    fatta: false,
                    perche: perche.clone(),
                },
            );
            return rivedi_la_fase(server, p, f.n, &perche).await;
        }
        None => ferma_e_di(
            server,
            &p.nome,
            &format!(
                "la revisione della fase {} non si legge: «{}»",
                f.n,
                pr::in_breve(&detto)
            ),
        ),
    }
    true
}

/// Una fase non e' andata: l'Architetto la rivede, se non l'ha gia' fatto
/// troppe volte.
async fn rivedi_la_fase(server: &Arc<Server>, p: &Progetto, n: u32, perche: &str) -> bool {
    match crate::architetto::chiedi(server, d::azienda::NOME_APM, &p.nome, perche, "", Some(n)) {
        Ok(v) => {
            let id = v.get("compito").and_then(Value::as_u64).unwrap_or(0);
            annota(
                &p.nome,
                Evento::PianoChiesto {
                    quando: adesso(),
                    compito: id,
                    fase: Some(n),
                },
            );
        }
        Err(e) => ferma_e_di(
            server,
            &p.nome,
            &format!("la fase {n} non e' andata ({perche}), e {e}"),
        ),
    }
    true
}

/// Tutte le fasi fatte: il legale, il resoconto, la fine.
async fn rilascio(server: &Arc<Server>, p: &Progetto, pi: &piano::Piano) -> bool {
    if p.legale_rilascio.is_none() {
        let tutte: Vec<String> = pi
            .fasi
            .iter()
            .map(|f| format!("Fase {} «{}»:\n{}", f.n, f.nome, p.consegne(f)))
            .collect();
        let materia = format!(
            "Tutto quello che il progetto consegna:\n{}",
            tutte.join("\n\n")
        );
        return legale(server, p, "prima del rilascio", &materia, None, true).await;
    }
    let domanda = pr::domanda_resoconto(p, pi);
    let resoconto = match chiedi_al_grande(
        server,
        &p.nome,
        "il resoconto",
        &domanda,
        GETTONI_RESOCONTO,
    )
    .await
    {
        Some(t) if !t.trim().is_empty() => t.trim().to_string(),
        // Un resoconto si scrive anche senza cervello: le fasi e le
        // consegne, cosi' come sono.
        _ => pr::resoconto_semplice(p, pi),
    };
    annota(
        &p.nome,
        Evento::Finito {
            quando: adesso(),
            resoconto: resoconto.clone(),
        },
    );
    a_nova(
        server,
        &format!("Il progetto «{}» e' finito.\n\n{resoconto}", p.nome),
    );
    sveglia(server);
    true
}

/// Il legale guarda. Con dei dubbi all'inizio si va avanti, e li vede
/// l'utente nel via; dopo una fase o prima del rilascio ci si ferma.
async fn legale(
    server: &Arc<Server>,
    p: &Progetto,
    quando: &str,
    materia: &str,
    fase: Option<u32>,
    rilascio: bool,
) -> bool {
    let domanda = pr::domanda_legale(p, quando, materia);
    let Some(detto) =
        chiedi_al_grande(server, &p.nome, "il legale", &domanda, GETTONI_CONTROLLO).await
    else {
        return true;
    };
    let Some((ok, note)) = pr::leggi_legale(&detto) else {
        ferma_e_di(
            server,
            &p.nome,
            &format!(
                "la risposta del legale {quando} non si legge: «{}»",
                pr::in_breve(&detto)
            ),
        );
        return true;
    };
    annota(
        &p.nome,
        Evento::Legale {
            quando: adesso(),
            fase,
            rilascio,
            ok,
            note: note.clone(),
        },
    );
    let inizio = fase.is_none() && !rilascio;
    if !ok && !inizio {
        ferma_e_di(
            server,
            &p.nome,
            &format!("il legale ha dei dubbi {quando}: {}", note.join("; ")),
        );
    }
    true
}

/// Una domanda al cervello piu' grande che risponde a un indirizzo. Se non
/// si puo', il progetto si ferma e lo dice: un controllo che non c'e' non e'
/// un si'.
async fn chiedi_al_grande(
    server: &Arc<Server>,
    nome: &str,
    chi: &str,
    domanda: &str,
    gettoni: u32,
) -> Option<String> {
    let cfg = nova_configurazione::dove::leggi();
    let Some(grande) = crate::agente::scala_di(&cfg)
        .into_iter()
        .rev()
        .find(|g| g.indirizzo().is_some())
    else {
        ferma_e_di(
            server,
            nome,
            &format!("{chi} non ha un cervello a cui chiedere: nella scala non ce n'e' uno che risponde a un indirizzo"),
        );
        return None;
    };
    let in_casa = grande.indirizzo().is_some_and(|(_, _, _, casa)| casa);
    let solo_locale = crate::dalla_configurazione::scala(&cfg).solo_locale;
    if let Err(e) = crate::custode::si_puo_chiedere_per(
        Decisione::ControlloDelProgetto,
        chi,
        in_casa,
        domanda,
        solo_locale,
    ) {
        ferma_e_di(server, nome, &format!("{chi} non ha potuto guardare: {e}"));
        return None;
    }
    let Some(detto) =
        crate::agente::chiedi_con(std::slice::from_ref(&grande), domanda, gettoni).await
    else {
        ferma_e_di(
            server,
            nome,
            &format!("{chi} non ha risposto: «{}» non risponde", grande.nome()),
        );
        return None;
    };
    if a_consumo(&cfg, grande.nome()) {
        annota(
            nome,
            Evento::Spesa {
                quando: adesso(),
                usd: stima(&cfg),
                per: chi.to_string(),
            },
        );
    }
    Some(detto)
}

// ------------------------------------------------------------- la spesa

/// La stima di una domanda a consumo, dal pannello.
fn stima(cfg: &Value) -> f64 {
    nova_scala::stima(&crate::dalla_configurazione::scala(cfg))
}

/// Se il gradino con quel nome e' a consumo.
fn a_consumo(cfg: &Value, gradino: &str) -> bool {
    let conf = crate::dalla_configurazione::scala(cfg);
    let recapiti = crate::dalla_configurazione::recapiti(cfg, &|n| std::env::var(n).ok());
    conf.gradino(gradino)
        .is_some_and(|g| crate::caps_cervelli::a_consumo(g, &recapiti))
}

/// Se nella scala c'e' almeno un cervello a consumo: senza, il tetto e'
/// zero.
fn ce_consumo() -> bool {
    let cfg = nova_configurazione::dove::leggi();
    let conf = crate::dalla_configurazione::scala(&cfg);
    let recapiti = crate::dalla_configurazione::recapiti(&cfg, &|n| std::env::var(n).ok());
    conf.tiers
        .iter()
        .any(|g| crate::caps_cervelli::a_consumo(g, &recapiti))
}

/// Quanto e' costato un compito di un Dot: i suoi turni e i voti del capo
/// sui cervelli a consumo, alla stima di una delega. Si annota se non e'
/// zero.
fn spesa(nome: &str, c: &d::Cartella, compito: u64, per: &str) {
    let cfg = nova_configurazione::dove::leggi();
    let diario = std::fs::read_to_string(c.radice.join("diario.jsonl")).unwrap_or_default();
    let mut quante = 0usize;
    for r in diario
        .lines()
        .filter_map(|x| serde_json::from_str::<Value>(x).ok())
    {
        if r.get("compito").and_then(Value::as_u64) != Some(compito) {
            continue;
        }
        let cervello = match r.get("tipo").and_then(Value::as_str) {
            Some("turno") => r.get("cervello"),
            Some("voto") => r.get("deciso_da"),
            _ => None,
        };
        if cervello
            .and_then(Value::as_str)
            .is_some_and(|g| a_consumo(&cfg, g))
        {
            quante += 1;
        }
    }
    if quante > 0 {
        let usd = ((quante as f64 * stima(&cfg)) * 10_000.0).round() / 10_000.0;
        annota(
            nome,
            Evento::Spesa {
                quando: adesso(),
                usd,
                per: per.to_string(),
            },
        );
    }
}
