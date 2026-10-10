//! Il ricercatore nel demone: il piano, i passi, il revisore, il rapporto (D383).
//!
//! Il secondo passo dei Dot (D381, `docs/dots.md`). Un Dot col mestiere di
//! ricercatore non fa turni finche' il modello risponde, come un Dot
//! generico: lavora a passi.
//!
//! 1. **Il piano**, col cervello piu' grande della scala: i passi, e per
//!    ognuno il cervello che lo fa. Per ora sceglie lui (Gio, 8 ottobre); poi
//!    scegliera' AR, il Dot che sceglie i modelli.
//! 2. **I passi**, ognuno un turno nella conversazione del Dot, che parte dal
//!    gradino assegnato (`agente::turno_in`, `parti_da`).
//! 3. **La revisione**: il cervello grande giudica i passi fatti dai cervelli
//!    piu' piccoli. Uno scarso si rifa' un gradino sopra quello che l'ha
//!    fatto, fino al cervello grande, i cui passi non si rivedono: non c'e'
//!    nessuno sopra a cui salire. Un cervello piccolo che non risponde fa
//!    salire allo stesso modo.
//! 4. **Il rapporto**: la risposta dell'ultimo passo, «scrivi», in
//!    `rapporti/<compito>.md`, con in coda le fonti controllate contro gli
//!    indirizzi che il Dot ha letto davvero con gli strumenti; e una nota nel
//!    suo vault, perche' la prossima ricerca sappia cosa ha gia' trovato.
//!
//! **Ogni scelta del cervello si registra**, con com'e' andata: nel diario
//! del Dot e in `decisioni.jsonl` (`cervello_per_passo`). Sono gli esempi su
//! cui si addestrera' CLM a scegliere.
//!
//! I testi e le regole — come si chiede il piano e come si legge, come si
//! legge un giudizio, come si controllano le fonti — stanno in
//! `nova_dot::ricerca`, dove si provano senza demone.

use std::sync::{Arc, Mutex};

use nova_ciclo::Fine;
use nova_dot as d;
use nova_dot::ricerca as r;
use serde_json::{json, Value};

use crate::agente::{Chi, EsecutoreDemone, Svolto};
use crate::server::Server;
use crate::sessione::Sessione;

fn adesso() -> String {
    crate::decisioni::adesso()
}

/// Un compito in corso, con tutto quello che serve ai suoi turni.
struct Lavoro<'a> {
    server: &'a Arc<Server>,
    c: &'a d::Cartella,
    nome: String,
    compito: &'a d::Compito,
    cfg: Value,
    s: Sessione,
    esecutore: EsecutoreDemone,
    sessione: String,
    /// I nomi dei gradini, dal piu' piccolo al piu' grande, come erano
    /// quando il compito e' cominciato.
    scala: Vec<String>,
    /// I gradini che leggono con strumenti loro, e non con quelli di NOVA:
    /// Claude Code e le CLI. Quello che leggono NOVA non lo vede.
    ciechi: Vec<String>,
    /// Se una parte del lavoro l'ha fatta uno di loro: allora un indirizzo
    /// che NOVA non ha visto non e' «non visto», e' da verificare.
    cieco: bool,
}

/// Com'e' finito un passo.
enum Uscita {
    Fatto {
        testo: String,
        cervello: String,
        esito: &'static str,
        secondi: f64,
        strumenti: Vec<String>,
    },
    Rotto {
        errore: String,
        cervello: String,
        secondi: f64,
        strumenti: Vec<String>,
    },
    Fermato,
}

impl Lavoro<'_> {
    fn cima(&self) -> String {
        self.scala.last().cloned().unwrap_or_default()
    }

    /// Dove sta un gradino nella scala. Uno che non c'e' piu' conta come il
    /// piu' alto: non si rivede e non si sale, perche' non si sa da dove.
    fn indice(&self, cervello: &str) -> usize {
        self.scala
            .iter()
            .position(|g| g == cervello)
            .unwrap_or(self.scala.len().saturating_sub(1))
    }

    fn diario(&self, campi: Value) {
        let mut riga = json!({ "quando": adesso(), "compito": self.compito.id });
        if let (Some(r), Value::Object(c)) = (riga.as_object_mut(), campi) {
            r.extend(c);
        }
        if let Err(e) = self.c.diario(&riga) {
            tracing::warn!(dot = %self.nome, errore = %e, "diario non scritto");
        }
    }

    fn evento(&self, campi: Value) {
        let mut e = json!({ "dot": self.nome, "id": self.compito.id });
        if let (Some(o), Value::Object(c)) = (e.as_object_mut(), campi) {
            o.extend(c);
        }
        self.server.ctx.bus.emit("dot.passo", e);
    }

    /// Un turno nella conversazione del Dot, a partire da `cervello`. La
    /// conversazione si salva dopo, comunque sia andata.
    async fn turno(&mut self, testo: &str, cervello: &str, per: &str) -> Result<Svolto, String> {
        let esito = crate::agente::turno_in(
            self.server,
            &self.cfg,
            &mut self.s,
            testo,
            &self.sessione,
            false,
            "",
            &self.esecutore,
            cervello,
        )
        .await;
        crate::dot::salva(self.c, &self.nome, &self.s);
        let svolto = esito.map_err(|e| e.to_string())?;
        // Il revisore giudica, non legge per il rapporto: chi legge sono il
        // piano e i passi.
        if per != "revisione" && self.ciechi.contains(&svolto.cervello) {
            self.cieco = true;
        }
        self.diario(json!({
            "tipo": "turno",
            "per": per,
            "cervello_chiesto": cervello,
            "cervello": svolto.cervello,
            "esito": crate::agente::esito_di(&svolto.fine).0,
            "strumenti": svolto.strumenti_usati,
            "secondi": (svolto.durata * 10.0).round() / 10.0,
        }));
        Ok(svolto)
    }

    /// Un passo: un turno, e altri se il modello finisce i giri di strumenti,
    /// fino a [`r::TURNI_PER_PASSO`].
    async fn passo(
        &mut self,
        numero: usize,
        quanti: usize,
        passo: &r::Passo,
        fatti: &[r::Fatto],
        cervello: &str,
        rifatto: Option<(&str, &str)>,
    ) -> Uscita {
        let mut testo = r::richiesta_del_passo(numero, quanti, passo, fatti, rifatto);
        let mut dove = cervello.to_string();
        let mut secondi = 0.0;
        let mut strumenti: Vec<String> = Vec::new();
        let mut ultimo = String::new();
        for _ in 0..r::TURNI_PER_PASSO {
            let svolto = match self.turno(&testo, &dove, "passo").await {
                Ok(s) => s,
                Err(errore) => {
                    return Uscita::Rotto {
                        errore,
                        cervello: dove,
                        secondi,
                        strumenti,
                    }
                }
            };
            secondi += svolto.durata;
            strumenti.extend(svolto.strumenti_usati.iter().cloned());
            if !svolto.cervello.is_empty() {
                // Se il turno e' salito da solo, si continua da li'.
                dove = svolto.cervello.clone();
            }
            match svolto.fine {
                Fine::Risposto(t) => {
                    return Uscita::Fatto {
                        testo: t,
                        cervello: dove,
                        esito: "risposto",
                        secondi,
                        strumenti,
                    }
                }
                Fine::PassiFiniti(t) => {
                    ultimo = t;
                    testo = r::continua_il_passo(numero, passo);
                }
                Fine::Fermato => return Uscita::Fermato,
                Fine::Rotto(errore) => {
                    return Uscita::Rotto {
                        errore,
                        cervello: dove,
                        secondi,
                        strumenti,
                    }
                }
            }
        }
        Uscita::Fatto {
            testo: ultimo,
            cervello: dove,
            esito: "passi_finiti",
            secondi,
            strumenti,
        }
    }

    /// Il giudizio del cervello grande su un passo. `None` se nel frattempo
    /// qualcuno ha fermato tutto.
    async fn rivedi(
        &mut self,
        numero: usize,
        quanti: usize,
        passo: &r::Passo,
        cervello: &str,
        risultato: &str,
    ) -> Option<r::Giudizio> {
        let chiesta = r::richiesta_della_revisione(numero, quanti, passo, cervello, risultato);
        let cima = self.cima();
        let detto = match self.turno(&chiesta, &cima, "revisione").await {
            Ok(s) => match s.fine {
                Fine::Risposto(t) | Fine::PassiFiniti(t) => Ok(t),
                Fine::Fermato => return None,
                Fine::Rotto(e) => Err(e),
            },
            Err(e) => Err(e),
        };
        let giudizio = match &detto {
            Ok(t) => r::leggi_giudizio(t),
            Err(_) => r::Giudizio::Illeggibile,
        };
        if giudizio == r::Giudizio::Illeggibile {
            // Si tiene il passo, e si scrive perche': un giudizio che non si
            // legge non fa salire, ma chi rilegge il diario deve saperlo.
            self.diario(json!({
                "tipo": "revisione_illeggibile",
                "passo": numero,
                "detto": match &detto {
                    Ok(t) => r::tagliato(t, 300),
                    Err(e) => format!("il revisore non ha risposto: {e}"),
                },
            }));
        }
        Some(giudizio)
    }

    /// Registra la scelta del cervello per un passo, con com'e' andata: nel
    /// diario sempre, in `decisioni.jsonl` se il registro e' acceso.
    #[allow(clippy::too_many_arguments)]
    fn registra(
        &self,
        numero: usize,
        passo: &r::Passo,
        scelto: &str,
        scelto_da: r::SceltoDa,
        arrivato: &str,
        esito: &str,
        giudizio: &str,
        strumenti: &[String],
        secondi: f64,
    ) {
        let riga = crate::decisioni::riga_cervello_per_passo(
            &adesso(),
            &crate::decisioni::CervelloPerPasso {
                dot: &self.nome,
                compito: self.compito.id,
                passo: numero,
                genere: passo.genere.nome(),
                richiesta: &passo.cosa,
                scala: &self.scala,
                scelto,
                scelto_da: scelto_da.nome(),
                arrivato,
                esito,
                giudizio,
                strumenti,
                secondi,
            },
        );
        crate::decisioni::annota(&self.cfg, &riga);
        self.diario(riga);
    }
}

/// Fa un compito da ricercatore. Torna lo stato con cui si chiude e la frase
/// che lo dice. In `usato` lascia il cervello dell'ultimo passo fatto: quello
/// che ha scritto il rapporto, che il capo giudica (D401).
pub async fn lavora(
    server: &Arc<Server>,
    c: &d::Cartella,
    dot: &d::Dot,
    compito: &d::Compito,
    usato: &mut String,
) -> (d::Stato, String) {
    let cfg = nova_configurazione::dove::leggi();
    let gradini = crate::agente::scala_di(&cfg);
    let scala: Vec<String> = gradini.iter().map(|g| g.nome().to_string()).collect();
    let ciechi: Vec<String> = gradini
        .iter()
        .filter(|g| g.indirizzo().is_none())
        .map(|g| g.nome().to_string())
        .collect();
    let viste = Arc::new(Mutex::new(Vec::new()));
    let mut l = Lavoro {
        server,
        c,
        nome: dot.nome.clone(),
        compito,
        s: crate::dot::conversazione_di(c, dot, &cfg),
        esecutore: EsecutoreDemone {
            server: server.clone(),
            chi: Chi::Dot(dot.nome.clone()),
            viste: Some(viste.clone()),
            sola_lettura: false,
        },
        sessione: format!("dot:{}", dot.nome),
        scala,
        ciechi,
        cieco: false,
        cfg,
    };
    let cima = l.cima();
    // Il cervello che AR ha scelto per tutto il compito (D397): fa il piano e
    // ogni passo, e il revisore puo' ancora far salire un passo scarso. Uno
    // che non e' piu' nella scala non vale: decide il piano, come prima.
    let di_ar = (!compito.cervello.is_empty())
        .then(|| l.scala.iter().find(|g| **g == compito.cervello).cloned())
        .flatten();
    if !compito.cervello.is_empty() && di_ar.is_none() {
        l.diario(json!({
            "tipo": "nota",
            "nota": format!("«{}», il cervello scelto da AR, non e' piu' nella scala: decide il piano", compito.cervello),
        }));
    }

    // 1. Il piano, col cervello grande, o con quello di AR.
    let mut chiesta = r::richiesta_del_piano(&compito.testo, &l.scala);
    if compito.riprese > 0 {
        chiesta.push_str(d::RIPRESO);
    }
    let chi_pianifica = di_ar.clone().unwrap_or_else(|| cima.clone());
    let detto = match l.turno(&chiesta, &chi_pianifica, "piano").await {
        Ok(s) => match s.fine {
            Fine::Risposto(t) | Fine::PassiFiniti(t) => t,
            Fine::Fermato => return (d::Stato::Fermato, "fermato col «fermati» di Nova".into()),
            Fine::Rotto(e) => return (d::Stato::Fallito, format!("il piano non si e' fatto: {e}")),
        },
        Err(e) => return (d::Stato::Fallito, format!("il piano non si e' fatto: {e}")),
    };
    let piano = r::piano(&detto, &l.scala);
    l.diario(json!({
        "tipo": "piano",
        "letto": piano.letto,
        "note": piano.note,
        "passi": piano.passi,
    }));
    let quanti = piano.passi.len();
    l.evento(json!({ "stato": "piano", "passi": quanti }));

    // 2. I passi, ognuno col suo cervello, e la revisione.
    let mut fatti: Vec<r::Fatto> = Vec::new();
    for (i, passo) in piano.passi.iter().enumerate() {
        let numero = i + 1;
        let (mut cervello, mut scelto_da) = match &di_ar {
            Some(g) => (g.clone(), r::SceltoDa::Ar),
            None => (passo.cervello.clone(), passo.scelto_da),
        };
        let mut rifatto: Option<(String, String)> = None;
        loop {
            l.evento(
                json!({ "passo": numero, "di": quanti, "cervello": cervello, "stato": "comincia" }),
            );
            let uscita = l
                .passo(
                    numero,
                    quanti,
                    passo,
                    &fatti,
                    &cervello,
                    rifatto.as_ref().map(|(a, b)| (a.as_str(), b.as_str())),
                )
                .await;
            let (testo, arrivato, esito, secondi, strumenti, rotto) = match uscita {
                Uscita::Fermato => {
                    return (d::Stato::Fermato, "fermato col «fermati» di Nova".into())
                }
                Uscita::Fatto {
                    testo,
                    cervello,
                    esito,
                    secondi,
                    strumenti,
                } => (testo, cervello, esito, secondi, strumenti, None),
                Uscita::Rotto {
                    errore,
                    cervello,
                    secondi,
                    strumenti,
                } => (
                    String::new(),
                    cervello,
                    "rotto",
                    secondi,
                    strumenti,
                    Some(errore),
                ),
            };
            let sotto_la_cima = l.indice(&arrivato) + 1 < l.scala.len();
            // Il giudizio, e il motivo per salire se c'e'.
            let (giudizio, motivo) = if let Some(e) = &rotto {
                ("rotto", Some(format!("il cervello non ha risposto: {e}")))
            } else if sotto_la_cima {
                match l.rivedi(numero, quanti, passo, &arrivato, &testo).await {
                    None => return (d::Stato::Fermato, "fermato col «fermati» di Nova".into()),
                    Some(r::Giudizio::Buono) => ("buono", None),
                    Some(r::Giudizio::Illeggibile) => ("illeggibile", None),
                    Some(r::Giudizio::Scarso(m)) => ("scarso", Some(m)),
                }
            } else {
                ("senza_revisione", None)
            };
            l.registra(
                numero, passo, &cervello, scelto_da, &arrivato, esito, giudizio, &strumenti,
                secondi,
            );
            if let Some(motivo) = motivo {
                if sotto_la_cima {
                    let su = l.scala[l.indice(&arrivato) + 1].clone();
                    l.diario(json!({
                        "tipo": "salita",
                        "passo": numero,
                        "da": arrivato,
                        "a": su,
                        "motivo": motivo,
                    }));
                    l.evento(json!({ "passo": numero, "di": quanti, "cervello": su, "stato": "sale", "da": arrivato }));
                    rifatto = Some((arrivato, motivo));
                    cervello = su;
                    scelto_da = r::SceltoDa::Salita;
                    continue;
                }
                if let Some(e) = rotto {
                    return (
                        d::Stato::Fallito,
                        format!("il passo {numero} non si e' fatto, nemmeno col cervello piu' grande: {e}"),
                    );
                }
            }
            l.evento(
                json!({ "passo": numero, "di": quanti, "cervello": arrivato, "stato": "fatto" }),
            );
            usato.clone_from(&arrivato);
            fatti.push(r::Fatto {
                numero,
                genere: passo.genere,
                testo,
            });
            break;
        }
    }

    // 3. Il rapporto: la risposta dell'ultimo passo, con le fonti controllate.
    let corpo = fatti.last().map(|f| f.testo.clone()).unwrap_or_default();
    if corpo.trim().is_empty() {
        return (
            d::Stato::Fallito,
            "l'ultimo passo non ha scritto niente: non c'e' un rapporto da consegnare".into(),
        );
    }
    let lette = viste.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let fonti = r::controlla_fonti(&corpo, &lette, !l.cieco);
    let percorso = match c.salva_rapporto(compito.id, &r::rapporto_su_disco(&corpo, &fonti)) {
        Ok(p) => p,
        Err(e) => return (d::Stato::Fallito, format!("il rapporto non si scrive: {e}")),
    };
    let dove = percorso.display().to_string();
    l.diario(json!({ "tipo": "rapporto", "file": dove, "fonti": fonti, "lette": lette.len() }));
    nel_vault(&l, &corpo, &dove).await;
    server.ctx.bus.emit(
        "dot.rapporto",
        json!({
            "dot": l.nome,
            "id": compito.id,
            "file": dove,
            "fonti": fonti.len(),
            "viste": fonti.iter().filter(|f| f.controllo == r::Controllo::Vista).count(),
            "da_verificare": fonti.iter().filter(|f| f.controllo == r::Controllo::DaVerificare).count(),
        }),
    );
    (d::Stato::Fatto, r::esito(&dove, &fonti))
}

/// Una nota nel vault del Dot per ogni rapporto: il titolo, l'inizio, dove
/// sta il resto. La prossima ricerca la trova nella sua memoria. Se la
/// memoria e' spenta, o la nota non si scrive, si dice nel diario e il
/// rapporto resta consegnato.
async fn nel_vault(l: &Lavoro<'_>, corpo: &str, dove: &str) {
    let titolo = r::titolo(corpo).unwrap_or_else(|| r::tagliato(&l.compito.testo, 60));
    let inizio: String = corpo
        .lines()
        .filter(|riga| !riga.trim_start().starts_with("# "))
        .collect::<Vec<_>>()
        .join("\n");
    let mut nodo = nova_nodi::Nodo {
        title: format!("Rapporto {}: {titolo}", l.compito.id),
        body: format!(
            "Il compito: {}\n\n{}\n\nIl rapporto intero: {dove}",
            r::tagliato(&l.compito.testo, 300),
            r::tagliato(&inizio, 800),
        ),
        tipo: "rapporto".into(),
        tags: vec!["rapporto".into()],
        confidenza: 0.9,
        origine: nova_nodi::ORIGINE_AUTO.to_string(),
        ..Default::default()
    };
    nodo.slug = nova_nodi::slug::slug(&nodo.title);
    let esito = match crate::dot::memoria_di(l.server, &l.nome, &l.cfg) {
        Ok((m, cfg)) => tokio::task::spawn_blocking(move || m.salva(&cfg, nodo, false))
            .await
            .map_err(|e| e.to_string())
            .and_then(|r| r),
        Err(e) => Err(e),
    };
    match esito {
        Ok(n) => l.diario(json!({ "tipo": "vault", "slug": n.slug })),
        Err(e) => l.diario(json!({ "tipo": "vault", "errore": e })),
    }
}
