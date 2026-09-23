//! Passare la palla: il giro della delega.
//!
//! Gemello di `Router.delega` in `nova/routing.py` e dei tre strumenti di
//! `nova/tools/deleghe.py`. Qui c'e' **tutto cio' che decide** — a chi si
//! sale per regola, chi e' consentito, quanto si prenota sul tetto di spesa,
//! chi e' in pausa e per quanto, su chi si ripiega e con che parole lo si
//! dice — e niente di cio' che agisce: chiedere davvero a un cervello, e
//! sapere se fa spendere, lo fa chi implementa [`Cervelli`]. Cosi' il giro
//! si confronta col Python vero con dei cervelli finti che rispondono da un
//! copione, e l'orologio e' quello che dice il copione.
//!
//! **Un ripiego non ripiega.** L'elenco dei sostituti lo fa chi ha fallito
//! per primo, e lo scorre tutto lui. Il Python ripiegava anche dai
//! sostituti, e con `solo_locale` acceso girava in tondo fino a
//! `RecursionError` (D331): corretto da tutte e due le parti.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::{
    durata_pausa, gradino_minimo, indice, ripieghi, scala, stima, utilizzabile, Configurazione,
    Gradino, Pause,
};

/// Traccia di una palla passata.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Traccia {
    pub da: String,
    pub a: String,
    pub motivo: String,
    pub compito: String,
    pub esito: String,
    pub costo_usd: f64,
    pub durata_ms: i64,
}

/// Cosa ha risposto un cervello.
#[derive(Debug, Clone, Default)]
pub struct Detto {
    pub testo: String,
    pub costo_usd: f64,
    pub durata_ms: i64,
}

/// Perche' un cervello non ha risposto.
#[derive(Debug, Clone, PartialEq)]
pub enum Guasto {
    /// Non e' pronto: manca il programma, manca la chiave, il server e' giu'.
    NonPronto(String),
    /// Quota finita. **Non e' un errore del compito**: il gradino va in
    /// pausa e si prova un altro fornitore.
    Limite { riprova_fra_s: i64 },
    /// Qualunque altra cosa.
    Altro(String),
}

/// Chi sa parlare davvero coi cervelli.
pub trait Cervelli {
    /// Se questo gradino fa spendere davvero (un abbonamento no).
    fn a_consumo(&self, g: &Gradino) -> bool;
    /// Chiede, con un messaggio solo. Costruire il cervello e controllare
    /// che sia pronto stanno qui dentro: sono effetti, non decisioni.
    fn chiedi(&mut self, g: &Gradino, prompt: &str) -> Result<Detto, Guasto>;
    /// Adesso, in secondi dal 1970.
    fn ora(&self) -> f64;
    /// Una riga per chi guarda cosa succede.
    fn annota(&mut self, _riga: &str) {}
}

/// Se questo gradino fa spendere: un gradino in casa mai, e senza
/// chiederlo a nessuno — e' `Router.a_consumo` del Python.
fn fa_spendere(c: &dyn Cervelli, g: &Gradino) -> bool {
    !g.locale && c.a_consumo(g)
}

/// Cio' che la delega si ricorda fra una chiamata e l'altra: quanto si e'
/// speso, quanto e' prenotato, chi e' in pausa, cosa e' successo.
#[derive(Debug, Clone, Default)]
pub struct Registro {
    /// Equivalente API: con l'abbonamento non e' una spesa.
    pub speso_usd: f64,
    /// Le stime delle deleghe in volo.
    pub prenotato_usd: f64,
    stime: BTreeMap<String, f64>,
    /// Gradino -> istante in cui si puo' riprovare.
    in_pausa: BTreeMap<String, f64>,
    pub storico: Vec<Traccia>,
}

impl Registro {
    /// Secondi che mancano prima di poter riprovare: `int(fine - ora)`.
    pub fn pausa_residua(&self, nome: &str, ora: f64) -> i64 {
        let fine = self.in_pausa.get(nome).copied().unwrap_or(0.0);
        ((fine - ora) as i64).max(0)
    }

    /// Le pause come le vogliono [`utilizzabile`] e [`ripieghi`].
    pub fn pause(&self, ora: f64) -> Pause {
        self.in_pausa
            .keys()
            .map(|n| (n.clone(), self.pausa_residua(n, ora)))
            .collect()
    }

    /// Mette in pausa, mai per meno di un minuto. Torna quanto.
    pub fn metti_in_pausa(&mut self, nome: &str, secondi: i64, ora: f64) -> i64 {
        let quanto = durata_pausa(secondi);
        self.in_pausa.insert(nome.to_string(), ora + quanto as f64);
        quanto
    }
}

/// Cosa si chiede.
#[derive(Debug, Clone, Default)]
pub struct Richiesta {
    pub a: String,
    pub compito: String,
    pub motivo: String,
    pub da: String,
    pub contesto: String,
    pub allegati: i64,
    pub salta_regola: bool,
}

/// `.strip()` di Python.
fn ripulito(s: &str) -> String {
    nova_pitone::senza_bianchi(s).to_string()
}

/// Se questo gradino si puo' usare adesso, e se no perche'.
///
/// Se lo si puo' usare e fa spendere, **prenota** la stima: verificare il
/// tetto e aggiornarlo in due momenti diversi lascia passare due deleghe
/// sopra il tetto. La stima conta anche questa delega: sapere di essere
/// sotto il tetto prima di partire non serve, serve sapere di esserci ancora
/// dopo.
fn consenti(
    cfg: &Configurazione,
    reg: &mut Registro,
    c: &dyn Cervelli,
    t: &Gradino,
) -> Result<(), String> {
    if cfg.solo_locale && !t.locale {
        return Err(format!(
            "«{}» manderebbe dati fuori dal PC, ma brains.routing.solo_locale e' attivo. \
             Disattivalo se vuoi usarlo.",
            t.nome
        ));
    }
    let fra = reg.pausa_residua(&t.nome, c.ora());
    if fra > 0 {
        return Err(format!(
            "«{}» ha esaurito la quota: riprovabile fra {} minuti.",
            t.nome,
            fra / 60
        ));
    }
    let tetto = cfg.tetto_usd_sessione;
    if tetto != 0.0 && fa_spendere(c, t) {
        let s = stima(cfg);
        let proiezione = reg.speso_usd + reg.prenotato_usd + s;
        if proiezione > tetto {
            return Err(format!(
                "tetto di spesa raggiunto ({:.2} $ spesi + {:.2} $ in corso + {:.2} $ \
                 stimati = {:.2} $, su {:.2} $). Alza brains.routing.tetto_usd_sessione \
                 oppure resta sul locale.",
                reg.speso_usd, reg.prenotato_usd, s, proiezione, tetto
            ));
        }
        reg.prenotato_usd += s;
        reg.stime.insert(t.nome.clone(), s);
    }
    Ok(())
}

fn si_puo(cfg: &Configurazione, reg: &Registro, c: &dyn Cervelli, nome: &str) -> bool {
    utilizzabile(
        cfg,
        nome,
        &reg.pause(c.ora()),
        reg.speso_usd,
        reg.prenotato_usd,
        &|g: &Gradino| fa_spendere(c, g),
    )
}

/// Il motivo di un sostituto si riscrive **anche nello storico**.
///
/// In Python la traccia nello storico e quella restituita sono lo stesso
/// oggetto, e cambiarne il motivo li cambia tutti e due. Qui sono due copie:
/// l'ultima dello storico e' proprio quella del sostituto, perche' un
/// sostituto non ripiega e la sua traccia e' l'ultima cosa che scrive.
fn ribattezza(reg: &mut Registro, rip: &Traccia) {
    if let Some(ultima) = reg.storico.last_mut() {
        ultima.motivo = rip.motivo.clone();
    }
}

/// Affida un sotto-compito a un gradino.
///
/// Torna la traccia — anche quando il cervello non ha risposto: allora
/// l'esito comincia con `ERRORE` — oppure un rifiuto, che e' il
/// `PermissionError` del Python: il gradino non e' consentito e nessun
/// sostituto lo era. Tutto finisce nello storico, anche i tentativi.
///
/// `ripiego` e' vero per i sostituti: loro non ripiegano (vedi in testa).
pub fn delega(
    cfg: &Configurazione,
    reg: &mut Registro,
    c: &mut dyn Cervelli,
    r: &Richiesta,
    ripiego: bool,
) -> Result<Traccia, String> {
    let mut a = r.a.clone();
    let mut motivo = r.motivo.clone();
    // Sui sostituti la regola non si riapplica: rialzare al gradino che ha
    // appena rifiutato rimanda la palla a chi l'ha respinta.
    let (minimo, categoria) = if r.salta_regola {
        (String::new(), String::new())
    } else {
        gradino_minimo(cfg, &r.compito, r.allegati)
    };
    // Se il minimo e' chi sta gia' chiedendo, salire sarebbe un ciclo.
    if !minimo.is_empty() && minimo != r.da && indice(cfg, &minimo) > indice(cfg, &a) {
        if si_puo(cfg, reg, c, &minimo) {
            c.annota(&format!(
                "«{categoria}»: sale da «{a}» a «{minimo}» per regola"
            ));
            motivo = ripulito(&format!(
                "{motivo} (gradino minimo «{minimo}»: {categoria})"
            ));
            a = minimo;
        } else {
            c.annota(&format!(
                "«{categoria}» vorrebbe «{minimo}», non utilizzabile: resto su «{a}»"
            ));
        }
    }

    let mut t = Traccia {
        da: r.da.clone(),
        a: a.clone(),
        motivo: motivo.clone(),
        compito: r.compito.clone(),
        ..Default::default()
    };
    let prompt = if r.contesto.is_empty() {
        r.compito.clone()
    } else {
        format!("{}\n\n---\n\n{}", r.contesto, r.compito)
    };
    let detto = if motivo.is_empty() {
        r.compito.chars().take(60).collect()
    } else {
        motivo.clone()
    };
    c.annota(&format!("delega a «{a}»: {detto}"));

    let sostituto = |cfg: &Configurazione,
                     reg: &mut Registro,
                     c: &mut dyn Cervelli,
                     alternativa: &str|
     -> Result<Traccia, String> {
        let r2 = Richiesta {
            a: alternativa.to_string(),
            allegati: 0,
            salta_regola: true,
            ..r.clone()
        };
        delega(cfg, reg, c, &r2, true)
    };

    let mut fuori: Option<Result<Traccia, String>> = None;
    match cfg.gradino(&a).cloned() {
        None => {
            let s = scala(cfg);
            let ci_sono = if s.is_empty() {
                "(nessuno)".to_string()
            } else {
                s.join(", ")
            };
            t.esito = format!("ERRORE: gradino «{a}» inesistente. Ci sono: {ci_sono}");
        }
        Some(g) => match consenti(cfg, reg, c, &g) {
            Err(perche) => {
                // Policy o tetto: ha senso provare un gradino consentito prima
                // di arrendersi, ma se non ce n'e' l'utente deve sapere il
                // perche'.
                t.esito = format!("ERRORE: {perche}");
                let candidati = if ripiego {
                    Vec::new()
                } else {
                    ripieghi(cfg, &a, &reg.pause(c.ora()))
                };
                for alternativa in candidati {
                    if let Ok(mut rip) = sostituto(cfg, reg, c, &alternativa) {
                        if !rip.esito.starts_with("ERRORE") {
                            rip.motivo =
                                ripulito(&format!("{motivo} (ripiego: «{a}» non consentito)"));
                            ribattezza(reg, &rip);
                            fuori = Some(Ok(rip));
                            break;
                        }
                    }
                }
                if fuori.is_none() {
                    fuori = Some(Err(perche));
                }
            }
            Ok(()) => match c.chiedi(&g, &prompt) {
                Ok(d) => {
                    t.esito = d.testo;
                    t.costo_usd = d.costo_usd;
                    t.durata_ms = d.durata_ms;
                }
                Err(Guasto::NonPronto(perche)) | Err(Guasto::Altro(perche)) => {
                    t.esito = format!("ERRORE: {perche}");
                }
                Err(Guasto::Limite { riprova_fra_s }) => {
                    let ora = c.ora();
                    let quanto = reg.metti_in_pausa(&a, riprova_fra_s, ora);
                    c.annota(&format!(
                        "«{a}» in pausa per {} minuti: quota esaurita",
                        quanto / 60
                    ));
                    t.esito = format!("ERRORE: quota esaurita su «{a}»");
                    if !ripiego && cfg.ripiego_su_limite {
                        for alternativa in ripieghi(cfg, &a, &reg.pause(c.ora())) {
                            c.annota(&format!("«{a}» e' a quota: ripiego su «{alternativa}»"));
                            match sostituto(cfg, reg, c, &alternativa) {
                                Err(e) => {
                                    c.annota(&format!("«{alternativa}» non utilizzabile: {e}"));
                                }
                                Ok(mut rip) => {
                                    if !rip.esito.starts_with("ERRORE") {
                                        rip.motivo =
                                            ripulito(&format!("{motivo} (ripiego: «{a}» a quota)"));
                                        ribattezza(reg, &rip);
                                        fuori = Some(Ok(rip));
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            },
        },
    }

    // Il `finally` del Python: il costo vero sostituisce la prenotazione, e
    // la traccia va nello storico comunque sia andata.
    reg.speso_usd += t.costo_usd;
    let s = reg.stime.remove(&a).unwrap_or(0.0);
    reg.prenotato_usd = (reg.prenotato_usd - s).max(0.0);
    reg.storico.push(t.clone());
    fuori.unwrap_or(Ok(t))
}

// ------------------------------------------------------------ gli strumenti

/// I file indicati, in coda al contesto.
///
/// Il modello passa i percorsi, non il contenuto: cosi' delegare costa una
/// riga invece di qualche migliaio di token generati a mano. La regola —
/// quanti file, quanti caratteri, come si taglia — e' quella di
/// [`nova_mcp::allega`], con in piu' la riga che dice che ci si e' fermati:
/// e' l'unica differenza fra le due funzioni del Python. `leggi` torna il
/// percorso come lo si mostra e il testo, o perche' non si e' letto.
pub fn allega(
    contesto: &str,
    file: &[String],
    leggi: &dyn Fn(&str) -> (String, Result<String, String>),
) -> String {
    let letti: Vec<nova_mcp::Allegato> = file
        .iter()
        .take(nova_mcp::MAX_FILE)
        .map(|p| match leggi(p) {
            (percorso, Ok(testo)) => nova_mcp::Allegato::Letto { percorso, testo },
            (percorso, Err(perche)) => nova_mcp::Allegato::Illeggibile { percorso, perche },
        })
        .collect();
    nova_mcp::allega_avvisando(contesto, &letti, true)
}

/// Come legge un allegato il Python: `Path(p).expanduser()` e
/// `read_text(errors="replace")`, che apre in modo testo — ogni `\r\n` e
/// ogni `\r` da solo diventano `\n`. Chi riceve un file di Windows non deve
/// trovarsi un carattere in piu' a ogni riga.
///
/// Torna il percorso come lo si mostra e il testo, o perche' non si e'
/// letto. Il perche' e' quello del sistema, e non ha le parole di Python
/// (`[Errno 2] No such file or directory`): e' l'unica cosa che qui non si
/// confronta.
pub fn leggi_come_python(p: &str) -> (String, Result<String, String>) {
    let casa = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    let esteso = match p.strip_prefix('~') {
        Some(resto) if resto.is_empty() || resto.starts_with(['/', '\\']) => {
            format!("{casa}{resto}")
        }
        _ => p.to_string(),
    };
    let letto = std::fs::read(&esteso)
        .map(|b| {
            String::from_utf8_lossy(&b)
                .replace("\r\n", "\n")
                .replace('\r', "\n")
        })
        .map_err(|e| e.to_string());
    (esteso, letto)
}

/// `delega` come la vede il modello: la risposta con un'intestazione che
/// dice chi ha risposto davvero, o l'errore da mostrargli.
///
/// `a` e' com'e' arrivato, spazi compresi: e' con quello che si confronta
/// chi ha risposto, e cosi' fa il Python.
pub fn strumento_delega(
    cfg: &Configurazione,
    reg: &mut Registro,
    c: &mut dyn Cervelli,
    a: &str,
    compito: &str,
    motivo: &str,
    contesto: &str,
    allegati: i64,
) -> Result<String, String> {
    let r = Richiesta {
        a: ripulito(a),
        compito: compito.to_string(),
        motivo: motivo.to_string(),
        da: "orchestratore".into(),
        contesto: contesto.to_string(),
        allegati,
        salta_regola: false,
    };
    let t = delega(cfg, reg, c, &r, false)?;
    let effettivo = if t.a.is_empty() { a } else { t.a.as_str() };
    if t.esito.starts_with("ERRORE") {
        let dopo: String = t.esito.chars().skip(7).collect();
        return Err(format!("«{effettivo}» non ha potuto rispondere: {dopo}"));
    }
    // Non «a»: il gradino puo' essere salito, e dirlo storto vorrebbe dire
    // mostrare all'utente un modello che non ha risposto.
    let mut testa = format!("[risposta da «{effettivo}»");
    if effettivo != a {
        testa.push_str(&format!(" (salito da «{a}»)"));
    }
    if t.costo_usd != 0.0 {
        testa.push_str(&format!(", {:.4} $", t.costo_usd));
    }
    if t.durata_ms != 0 {
        testa.push_str(&format!(", {:.1}s", t.durata_ms as f64 / 1000.0));
    }
    testa.push(']');
    Ok(format!("{testa}\n{}", t.esito))
}

/// I due gradini a cui chiedere un secondo parere.
///
/// La regola di salita si risolve qui, una volta: applicata dentro le due
/// deleghe farebbe collassare tutti e due i gradini sullo stesso modello, e
/// lo strumento esiste apposta per confrontarne due.
pub fn scelti_per_parere(
    cfg: &Configurazione,
    reg: &Registro,
    c: &dyn Cervelli,
    domanda: &str,
    allegati: i64,
    primo: &str,
    secondo: &str,
) -> [String; 2] {
    let (minimo, _) = gradino_minimo(cfg, domanda, allegati);
    let scegli = |g: &str| {
        if !minimo.is_empty()
            && indice(cfg, &minimo) > indice(cfg, g)
            && si_puo(cfg, reg, c, &minimo)
        {
            minimo.clone()
        } else {
            g.to_string()
        }
    };
    let mut s = [scegli(primo), scegli(secondo)];
    if s[0] == s[1] {
        // Due teste restano due.
        s[1] = secondo.to_string();
    }
    s
}

/// `secondo_parere`: la stessa domanda a due gradini, e le due risposte.
pub fn secondo_parere(
    cfg: &Configurazione,
    reg: &mut Registro,
    c: &mut dyn Cervelli,
    domanda: &str,
    primo: &str,
    secondo: &str,
    contesto: &str,
    allegati: i64,
) -> String {
    let scelti = scelti_per_parere(cfg, reg, c, domanda, allegati, primo, secondo);
    let mut pezzi = Vec::new();
    for (richiesto, gradino) in [primo, secondo].iter().zip(scelti.iter()) {
        let titolo = if gradino == richiesto {
            gradino.clone()
        } else {
            format!("{gradino} (salito da {richiesto})")
        };
        let r = Richiesta {
            a: gradino.clone(),
            compito: domanda.to_string(),
            motivo: "secondo parere".into(),
            da: "orchestratore".into(),
            contesto: contesto.to_string(),
            allegati: 0,
            salta_regola: true,
        };
        pezzi.push(match delega(cfg, reg, c, &r, false) {
            Ok(t) => format!("### {titolo}\n{}", t.esito),
            Err(e) => format!("### {titolo}\n(non disponibile: {e})"),
        });
    }
    pezzi.join("\n\n")
}

/// `round(x, 4)` di Python: arrotondato come si scrive, non come si calcola.
fn quattro_cifre(x: f64) -> f64 {
    format!("{x:.4}").parse().unwrap_or(x)
}

/// `modelli`: i gradini, il loro stato, quanto si e' speso e qual e' il tetto.
///
/// `pronto` dice, per ogni gradino, se il cervello e' pronto e se no perche':
/// chiederlo e' un effetto. `routing` e' la configurazione **come scritta**,
/// perche' due campi — l'orchestratore e il tetto — si mostrano cosi' come
/// sono.
pub fn stato(
    cfg: &Configurazione,
    routing: &Value,
    reg: &Registro,
    c: &dyn Cervelli,
    pronto: &dyn Fn(&Gradino) -> (bool, String),
) -> Value {
    let ora = c.ora();
    let mut gradini = Vec::new();
    let mut a_consumo_qualcuno = false;
    for t in &cfg.tiers {
        let (mut pronto_ora, mut motivo) = pronto(t);
        let pausa = reg.pausa_residua(&t.nome, ora);
        if pronto_ora && pausa != 0 {
            pronto_ora = false;
            motivo = format!("quota esaurita, riprovabile fra {} min", pausa / 60);
        }
        let consumo = fa_spendere(c, t);
        a_consumo_qualcuno |= consumo;
        gradini.push(json!({
            "gradino": t.nome,
            "cervello": t.brain,
            "modello": if t.model.is_empty() { "predefinito" } else { &t.model },
            "locale": t.locale,
            "a_consumo": consumo,
            "pronto": pronto_ora,
            "nota": if pronto_ora { String::new() } else { motivo },
            "descrizione": t.descrizione,
        }));
    }
    let mut salite = serde_json::Map::new();
    if let Some(Value::Object(o)) = routing.get("categorie_che_salgono") {
        for (nome, spec) in o {
            if spec.is_object()
                && spec
                    .get("attiva")
                    .map_or(true, crate::configurazione::vero_python)
            {
                salite.insert(
                    nome.clone(),
                    spec.get("gradino_minimo").cloned().unwrap_or(json!("")),
                );
            }
        }
    }
    json!({
        "gradini": gradini,
        "orchestratore": routing.get("orchestratore").cloned().unwrap_or(json!("locale")),
        "equivalente_usd": quattro_cifre(reg.speso_usd),
        "spesa_reale": a_consumo_qualcuno,
        "tetto_usd": if a_consumo_qualcuno {
            routing.get("tetto_usd_sessione").cloned().unwrap_or(Value::Null)
        } else {
            Value::Null
        },
        "deleghe": reg.storico.len(),
        "salite_automatiche": salite,
    })
}

#[cfg(test)]
mod prove {
    use super::*;

    /// Cervelli che rispondono da un copione.
    struct Copione {
        risposte: BTreeMap<String, Vec<Result<Detto, Guasto>>>,
        ora: f64,
        righe: Vec<String>,
    }

    impl Cervelli for Copione {
        fn a_consumo(&self, g: &Gradino) -> bool {
            g.a_pagamento
        }
        fn chiedi(&mut self, g: &Gradino, _p: &str) -> Result<Detto, Guasto> {
            let coda = self.risposte.entry(g.nome.clone()).or_default();
            if coda.is_empty() {
                return Ok(Detto {
                    testo: format!("risposta di {}", g.nome),
                    ..Default::default()
                });
            }
            coda.remove(0)
        }
        fn ora(&self) -> f64 {
            self.ora
        }
        fn annota(&mut self, riga: &str) {
            self.righe.push(riga.to_string());
        }
    }

    fn gradino(nome: &str, brain: &str, locale: bool) -> Gradino {
        Gradino {
            nome: nome.into(),
            brain: brain.into(),
            locale,
            a_pagamento: !locale,
            ..Default::default()
        }
    }

    fn cfg() -> Configurazione {
        Configurazione {
            tiers: vec![
                gradino("locale", "locale", true),
                gradino("standard", "claude", false),
                gradino("difficile", "claude", false),
                gradino("alternativo", "gemini", false),
            ],
            ripiego_su_limite: true,
            escalation_automatica: true,
            ..Default::default()
        }
    }

    #[test]
    fn un_ripiego_non_ripiega() {
        // Con solo_locale, «standard» e «alternativo» si rimandavano la palla.
        let mut c = cfg();
        c.solo_locale = true;
        let mut reg = Registro::default();
        let mut k = Copione {
            risposte: BTreeMap::new(),
            ora: 1000.0,
            righe: vec![],
        };
        let r = Richiesta {
            a: "standard".into(),
            compito: "ciao".into(),
            ..Default::default()
        };
        let t = delega(&c, &mut reg, &mut k, &r, false).unwrap();
        assert_eq!(t.a, "locale");
        assert_eq!(t.motivo, "(ripiego: «standard» non consentito)");
        let tentati: Vec<&str> = reg.storico.iter().map(|x| x.a.as_str()).collect();
        assert_eq!(tentati, ["alternativo", "locale", "standard"]);
    }

    #[test]
    fn a_quota_si_va_in_pausa_e_si_cambia_fornitore() {
        let c = cfg();
        let mut reg = Registro::default();
        let mut k = Copione {
            risposte: BTreeMap::from([(
                "standard".to_string(),
                vec![Err(Guasto::Limite { riprova_fra_s: 5 })],
            )]),
            ora: 1000.0,
            righe: vec![],
        };
        let r = Richiesta {
            a: "standard".into(),
            compito: "ciao".into(),
            ..Default::default()
        };
        let t = delega(&c, &mut reg, &mut k, &r, false).unwrap();
        // Non «difficile»: e' lo stesso conto di chi e' a quota.
        assert_eq!(t.a, "alternativo");
        assert_eq!(
            reg.pausa_residua("standard", 1000.0),
            60,
            "mai meno di un minuto"
        );
        assert!(k
            .righe
            .contains(&"«standard» in pausa per 1 minuti: quota esaurita".into()));
    }
}
