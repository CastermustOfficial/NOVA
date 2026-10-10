//! L'Architetto nel demone: il piano di sviluppo di un progetto (D400).
//!
//! Il settimo passo dell'azienda dei Dot (D395, `docs/dots.md`). Deciso con
//! Gio il 10 ottobre, sulla bozza:
//!
//! 1. **Chi chiede un piano** (Nova con `dot.pianifica`, poi l'APM) lo mette
//!    in coda all'Architetto ([`chiedi`]). L'Architetto non prende compiti a
//!    mano: la sua coda la riempie NOVA, col testo di
//!    [`nova_dot::piano::Richiesta`].
//! 2. **L'Architetto legge e basta**: un turno nella sua conversazione col
//!    cervello di AR, il piu' grande che risponde a un indirizzo, e solo gli
//!    strumenti per leggere (`EsecutoreDemone::sola_lettura`). Legge i file
//!    del progetto, se ha una cartella, e la sua memoria.
//! 3. **Risponde col piano in Markdown**, nel formato di `nova_dot::piano`.
//!    Un piano che non si legge torna indietro con tutti gli errori, al piu'
//!    [`p::CORREZIONI`] volte.
//! 4. **Ogni versione resta**: `piani/<progetto>/piano-<n>.md` nella sua
//!    cartella, in forma pulita, e una riga `piano` in `decisioni.jsonl` e
//!    nel suo diario.
//! 5. **Si consegna a Nova**: in chat arrivano quanto e' grande il piano, le
//!    domande per l'utente e dove sta il file.
//!
//! **Rivede** quando lo chiede l'utente, e da solo quando una fase fallisce
//! o la revisione la boccia: al piu' [`p::REVISIONI_PER_FASE`] volte per
//! fase, poi decide l'utente, e una sua revisione fa ripartire il conto.
//! Finche' l'APM non c'e', le fasi non girano: la revisione per una fase la
//! chiede Nova, e l'Architetto si prova da solo.

use std::sync::Arc;

use nova_ciclo::Fine;
use nova_dot as d;
use nova_dot::piano as p;
use serde_json::{json, Value};

use crate::agente::{Chi, EsecutoreDemone};
use crate::server::Server;

/// La cartella dei piani di un progetto.
fn cartella_del_progetto(progetto: &str) -> Result<std::path::PathBuf, String> {
    let c = d::Cartella::di(&crate::dot::base(), d::azienda::NOME_ARCHITETTO)?;
    Ok(p::cartella_del_progetto(&c.radice, progetto))
}

/// Chiede un piano all'Architetto: lo mette in coda, e torna subito. Il
/// piano arriva in chat quando e' pronto.
///
/// `fase`, se c'e', vuol dire che quella fase non e' andata e `testo` dice
/// perche': si rivede da sola, al piu' [`p::REVISIONI_PER_FASE`] volte.
/// `cartella`, se c'e', e' dove stanno i file del progetto: deve esistere.
pub fn chiedi(
    server: &Arc<Server>,
    progetto: &str,
    testo: &str,
    cartella: &str,
    fase: Option<u32>,
) -> Result<Value, String> {
    crate::dot_accesi::se_spenti()?;
    let progetto = p::nome_del_progetto(progetto)?;
    if testo.trim().is_empty() {
        return Err(match fase {
            None => "all'Architetto serve sapere cosa si vuole fare".into(),
            Some(n) => format!("all'Architetto serve sapere perche' la fase {n} non e' andata"),
        });
    }
    let cartella = cartella.trim();
    if !cartella.is_empty() && !std::path::Path::new(cartella).is_dir() {
        return Err(format!(
            "«{cartella}» non e' una cartella: i file del progetto devono esserci"
        ));
    }
    let dir = cartella_del_progetto(&progetto)?;
    let fatte = p::versioni(&dir);
    let perche = match fase {
        None => p::Perche::Utente,
        Some(0) => return Err("le fasi si contano da 1".into()),
        Some(n) => {
            let Some(ultima) = fatte.last().copied() else {
                return Err(format!(
                    "«{progetto}» non ha ancora un piano: prima il piano, poi le fasi"
                ));
            };
            let testo_ultimo = std::fs::read_to_string(p::file(&dir, ultima))
                .map_err(|e| format!("il piano di «{progetto}» non si legge: {e}"))?;
            let (piano, _) = p::leggi(&testo_ultimo, ultima).map_err(|e| {
                format!(
                    "la versione {ultima} del piano di «{progetto}» non si legge: {}",
                    e.join("; ")
                )
            })?;
            if !piano.fasi.iter().any(|f| f.n == n) {
                return Err(format!(
                    "il piano di «{progetto}», versione {ultima}, non ha una fase {n}"
                ));
            }
            let c = d::Cartella::di(&crate::dot::base(), d::azienda::NOME_ARCHITETTO)?;
            let gia = p::revisioni_automatiche(&c.compiti(), &progetto, n);
            if gia >= p::REVISIONI_PER_FASE {
                return Err(format!(
                    "la fase {n} del piano di «{progetto}» e' gia' tornata all'Architetto {gia} \
                     volte da sola: ora decide l'utente. Diglielo, e chiedigli cosa cambiare."
                ));
            }
            p::Perche::Fase(n)
        }
    };
    let r = p::Richiesta {
        progetto: progetto.clone(),
        cartella: cartella.to_string(),
        perche,
        testo: testo.trim().to_string(),
    };
    let id = crate::dot::affida_all_architetto(server, &r.nel_compito())?;
    Ok(json!({
        "architetto": d::azienda::NOME_ARCHITETTO,
        "compito": id,
        "progetto": progetto,
        "nota": "l'Architetto ci lavora: il piano arriva in chat quando e' pronto",
    }))
}

/// Un piano in corso, con tutto quello che serve ai suoi turni.
struct Lavoro<'a> {
    server: &'a Arc<Server>,
    c: &'a d::Cartella,
    dot: &'a d::Dot,
    compito: &'a d::Compito,
    r: p::Richiesta,
    versione: u32,
    cfg: Value,
    s: crate::sessione::Sessione,
    esecutore: EsecutoreDemone,
    sessione: String,
    /// Il cervello da cui parte ogni turno: quello di AR.
    cima: String,
    /// Chi ha risposto all'ultimo turno.
    cervello: String,
    strumenti: Vec<String>,
    secondi: f64,
    correzioni: usize,
}

/// Come si chiude un compito che non arriva al piano: lo stato, l'esito per
/// il registro, e perche'.
type Chiuso = (d::Stato, &'static str, String);

impl Lavoro<'_> {
    fn diario(&self, campi: Value) {
        let mut riga = json!({ "quando": crate::decisioni::adesso(), "compito": self.compito.id });
        if let (Some(r), Value::Object(c)) = (riga.as_object_mut(), campi) {
            r.extend(c);
        }
        if let Err(e) = self.c.diario(&riga) {
            tracing::warn!(dot = %self.dot.nome, errore = %e, "il diario dell'Architetto non si scrive");
        }
    }

    /// Un turno, e il testo con cui ha risposto: `None` se ha finito i passi
    /// senza rispondere.
    async fn turno(&mut self, testo: &str) -> Result<Option<String>, Chiuso> {
        let svolto = crate::agente::turno_in(
            self.server,
            &self.cfg,
            &mut self.s,
            testo,
            &self.sessione,
            false,
            "",
            &self.esecutore,
            &self.cima,
        )
        .await;
        crate::dot::salva(self.c, &self.dot.nome, &self.s);
        let svolto = svolto.map_err(|e| (d::Stato::Fallito, "rotto", e.to_string()))?;
        self.secondi += svolto.durata;
        self.strumenti
            .extend(svolto.strumenti_usati.iter().cloned());
        if !svolto.cervello.is_empty() {
            self.cervello = svolto.cervello.clone();
        }
        self.diario(json!({
            "tipo": "turno",
            "esito": crate::agente::esito_di(&svolto.fine).0,
            "cervello": svolto.cervello,
            "strumenti": svolto.strumenti_usati,
            "secondi": (svolto.durata * 10.0).round() / 10.0,
        }));
        match svolto.fine {
            Fine::Risposto(t) => Ok(Some(t)),
            Fine::PassiFiniti(_) => Ok(None),
            Fine::Fermato => Err((
                d::Stato::Fermato,
                "fermato",
                "fermato col «fermati» di Nova".into(),
            )),
            Fine::Rotto(e) => Err((d::Stato::Fallito, "rotto", e)),
        }
    }

    /// La riga `piano` in `decisioni.jsonl`, e nel diario.
    fn registra(&self, esito: &str, piano: Option<&p::Piano>, file: &str) {
        let fase = match self.r.perche {
            p::Perche::Fase(n) => Some(n),
            p::Perche::Utente => None,
        };
        let riga = crate::decisioni::riga_piano(
            &crate::decisioni::adesso(),
            &crate::decisioni::PianoFatto {
                progetto: &self.r.progetto,
                versione: self.versione,
                perche: if fase.is_some() { "fase" } else { "utente" },
                fase,
                richiesta: &self.r.testo,
                cervello: &self.cervello,
                esito,
                fasi: piano.map_or(0, |x| x.fasi.len()),
                compiti: piano.map_or(0, |x| x.incarichi().count()),
                da_assumere: piano.map_or(0, p::Piano::da_assumere),
                domande: piano.map_or(0, |x| x.domande.len()),
                correzioni: self.correzioni,
                strumenti: &self.strumenti,
                secondi: self.secondi,
                file,
            },
        );
        crate::decisioni::annota(&self.cfg, &riga);
        self.diario(riga);
    }

    /// Chiude il compito senza un piano, e lo registra.
    fn chiudi(&self, (stato, esito, perche): Chiuso) -> (d::Stato, String) {
        self.registra(esito, None, "");
        (stato, format!("il piano non si e' fatto: {perche}"))
    }
}

/// Fa un piano: la richiesta sta nel testo del compito. Torna lo stato con
/// cui si chiude il compito e la frase che lo dice.
pub async fn lavora(
    server: &Arc<Server>,
    c: &d::Cartella,
    dot: &d::Dot,
    compito: &d::Compito,
) -> (d::Stato, String) {
    let mut r = match p::Richiesta::dal_compito(&compito.testo) {
        Ok(r) => r,
        Err(e) => return (d::Stato::Fallito, format!("la richiesta non si legge: {e}")),
    };
    let dir = p::cartella_del_progetto(&c.radice, &r.progetto);
    let ultima = p::versioni(&dir).last().copied().unwrap_or(0);
    let precedente = if ultima == 0 {
        None
    } else {
        match std::fs::read_to_string(p::file(&dir, ultima)) {
            Ok(t) => Some(t),
            Err(e) => {
                return (
                    d::Stato::Fallito,
                    format!("il piano di adesso non si legge: {e}"),
                )
            }
        }
    };
    if matches!(r.perche, p::Perche::Fase(_)) && precedente.is_none() {
        return (
            d::Stato::Fallito,
            format!("«{}» non ha un piano da rivedere", r.progetto),
        );
    }
    // Una revisione senza cartella tiene quella del piano di adesso: va
    // nella domanda, perche' l'Architetto rilegga i file, e nel piano nuovo.
    if r.cartella.trim().is_empty() {
        r.cartella = precedente
            .as_deref()
            .and_then(|t| p::leggi(t, ultima).ok())
            .map(|(x, _)| x.cartella)
            .unwrap_or_default();
    }
    let cfg = nova_configurazione::dove::leggi();
    // Il cervello di AR (D397): il piu' grande che risponde a un indirizzo.
    // Il turno di chi legge e basta non usa gli altri.
    let Some(cima) = crate::agente::scala_di(&cfg)
        .iter()
        .rev()
        .find(|g| g.indirizzo().is_some())
        .map(|g| g.nome().to_string())
    else {
        return (
            d::Stato::Fallito,
            "nella scala non c'e' un cervello che risponde a un indirizzo: l'Architetto non usa \
             Claude Code ne' le CLI, che hanno mani loro"
                .into(),
        );
    };
    let mut l = Lavoro {
        server,
        c,
        dot,
        compito,
        versione: ultima + 1,
        s: crate::dot::conversazione_di(c, dot, &cfg),
        esecutore: EsecutoreDemone {
            server: server.clone(),
            chi: Chi::Dot(dot.nome.clone()),
            viste: None,
            sola_lettura: true,
        },
        sessione: format!("dot:{}", dot.nome),
        cima,
        cervello: String::new(),
        strumenti: Vec::new(),
        secondi: 0.0,
        correzioni: 0,
        cfg,
        r,
    };

    // 1. Legge quel che gli serve, e scrive il piano.
    let mut domanda = p::domanda(&l.r, l.versione, precedente.as_deref());
    if compito.riprese > 0 {
        domanda.push_str(d::RIPRESO);
    }
    let mut scritto = None;
    for giro in 0..p::TURNI_PER_PIANO {
        let testo = if giro == 0 {
            domanda.as_str()
        } else {
            p::CONTINUA
        };
        match l.turno(testo).await {
            Ok(Some(t)) => {
                scritto = Some(t);
                break;
            }
            Ok(None) => {}
            Err(chiuso) => return l.chiudi(chiuso),
        }
    }
    let Some(mut scritto) = scritto else {
        return l.chiudi((
            d::Stato::Fallito,
            "rotto",
            format!(
                "l'Architetto non l'ha scritto in {} turni",
                p::TURNI_PER_PIANO
            ),
        ));
    };

    // 2. Il piano si legge, o torna indietro con tutti gli errori.
    let (mut piano, mut note) = loop {
        match p::leggi(&scritto, l.versione) {
            Ok(x) => break x,
            Err(errori) => {
                l.diario(json!({ "tipo": "piano_illeggibile", "errori": errori }));
                if l.correzioni == p::CORREZIONI {
                    l.registra("illeggibile", None, "");
                    return (
                        d::Stato::Fallito,
                        format!(
                            "il piano non si legge nemmeno dopo {}: {}",
                            p::quanti(p::CORREZIONI, "correzione", "correzioni"),
                            errori.join("; ")
                        ),
                    );
                }
                l.correzioni += 1;
                match l.turno(&p::correggi(&errori)).await {
                    Ok(Some(t)) => scritto = t,
                    // Ha usato i passi senza riscriverlo: resta quello di
                    // prima, e la correzione e' persa.
                    Ok(None) => {}
                    Err(chiuso) => return l.chiudi(chiuso),
                }
            }
        }
    };

    // 3. Il progetto e la cartella sono quelli chiesti, non quelli scritti.
    if piano.progetto != l.r.progetto {
        note.push(format!(
            "il titolo diceva «{}»: il progetto e' «{}»",
            piano.progetto, l.r.progetto
        ));
        piano.progetto = l.r.progetto.clone();
    }
    piano.cartella = l.r.cartella.trim().to_string();
    let file = match p::salva(&dir, &piano) {
        Ok(f) => f.display().to_string(),
        Err(e) => {
            l.registra("rotto", Some(&piano), "");
            return (d::Stato::Fallito, format!("il piano non si scrive: {e}"));
        }
    };
    if !note.is_empty() {
        l.diario(json!({ "tipo": "nota", "note": note }));
    }
    l.registra("fatto", Some(&piano), &file);
    server.ctx.bus.emit(
        "dot.piano",
        json!({
            "dot": dot.nome,
            "id": compito.id,
            "progetto": piano.progetto,
            "versione": piano.versione,
            "file": file,
        }),
    );
    (d::Stato::Fatto, p::esito(&piano, &file))
}
