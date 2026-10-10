//! I progetti dell'azienda dei Dot, guidati dall'APM (D402).
//!
//! Deciso con Gio il 10 ottobre, sulla bozza, prima del codice:
//!
//! - **l'APM lavora a regole fisse**: le fasi in ordine, un compito affidato
//!   appena quelli da cui dipende sono fatti. Il cervello grande serve solo
//!   ai controlli (il legale, la revisione di una fase) e al resoconto;
//! - **il via lo dai in chat a Nova**, dopo aver visto piano, squadra, tetto
//!   e le domande dell'Architetto;
//! - **il legale guarda** all'inizio, **dopo ogni fase** e prima del
//!   rilascio;
//! - **un progetto alla volta**: un secondo aspetta in coda.
//!
//! Un progetto e' un diario, come la coda di un Dot: ogni riga di
//! `progetto.jsonl` e' un passo ([`Evento`]), e com'e' adesso si ottiene
//! rileggendole in ordine ([`Progetto::da`]). Un demone che si spegne a meta'
//! riprende da dove era. Accanto, `progetto.md` lo racconta a chi lo apre.
//!
//! ```text
//! <cartella di NOVA>/progetti/<nome>/
//!   progetto.jsonl   i passi dell'APM
//!   progetto.md      com'e' adesso, da leggere
//!   piani/           le versioni del piano dell'Architetto (D400)
//! ```
//!
//! Qui ci sono le regole e le parole; chi le fa girare e' `nova_core::apm`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::piano::{self, Chi, Piano};
use crate::{Mestiere, Stato};

/// La cartella dei progetti, dentro quella di NOVA.
pub const CARTELLA: &str = "progetti";

/// Quanto di una consegna entra in un testo per un altro (il compito dopo,
/// il legale, la revisione, il resoconto).
pub const CONSEGNA_IN_BREVE: usize = 2_000;

/// A quale parte del tetto l'APM si ferma e chiede se allargarlo.
pub const SOGLIA_DEL_TETTO: f64 = 0.9;

/// Quanti dollari si propongono di tetto per ogni compito del piano, se
/// nella scala c'e' un cervello a consumo: tre domande a consumo per
/// compito, alla stima di serie di una delega. L'utente lo cambia nel via.
pub const TETTO_PER_COMPITO: f64 = 0.3;

/// Il tetto piu' piccolo che si propone.
pub const TETTO_MINIMO: f64 = 1.0;

/// La cartella di un progetto.
pub fn cartella(nova: &Path, nome: &str) -> PathBuf {
    nova.join(CARTELLA).join(nome)
}

/// I progetti che ci sono, in ordine di nome.
pub fn elenco(nova: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(nova.join(CARTELLA))
        .map(|it| {
            it.filter_map(|e| e.ok())
                .filter(|e| e.path().join("progetto.jsonl").is_file())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// Un passo di un progetto: una riga di `progetto.jsonl`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum Evento {
    /// Nova l'ha passato all'APM. `numero` dice in che ordine sono nati i
    /// progetti: l'ora si scrive al secondo, e due possono nascere insieme.
    Creato {
        quando: String,
        #[serde(default)]
        numero: u64,
        richiesta: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        cartella: String,
    },
    /// Il piano e' chiesto all'Architetto: il suo compito, e la fase se e'
    /// una revisione per una fase che non e' andata.
    PianoChiesto {
        quando: String,
        compito: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fase: Option<u32>,
    },
    /// Il piano e' pronto, in quella versione. Se era una revisione per una
    /// fase, i compiti da quella fase in poi si rifanno col piano nuovo.
    PianoPronto {
        quando: String,
        versione: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        da_fase: Option<u32>,
    },
    /// Il legale ha guardato: all'inizio (`fase` vuota e non `rilascio`),
    /// dopo una fase, o prima del rilascio.
    Legale {
        quando: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fase: Option<u32>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        rilascio: bool,
        ok: bool,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        note: Vec<String>,
    },
    /// AR ha trovato chi fa un ruolo del piano.
    Squadra {
        quando: String,
        ruolo: String,
        dot: String,
    },
    /// L'APM ha mostrato tutto a Nova, e aspetta il via.
    Proposto { quando: String, tetto: f64 },
    /// Il via dell'utente, col tetto in dollari (zero: niente a consumo).
    Via { quando: String, tetto: f64 },
    /// Un compito del piano e' andato a un Dot.
    Affidato {
        quando: String,
        incarico: String,
        dot: String,
        compito: u64,
    },
    /// Il compito del Dot si e' chiuso.
    Chiuso {
        quando: String,
        incarico: String,
        stato: Stato,
        esito: String,
    },
    /// Una fase e' finita: fatta, o no e perche'.
    Fase {
        quando: String,
        n: u32,
        fatta: bool,
        perche: String,
    },
    /// Quanto e' costato un pezzo, in dollari, e cosa.
    Spesa {
        quando: String,
        usd: f64,
        per: String,
    },
    /// L'APM si e' fermato, e perche': lo decide l'utente.
    Fermo { quando: String, perche: String },
    /// L'utente l'ha fatto ripartire, col tetto nuovo.
    Ripreso { quando: String, tetto: f64 },
    /// Il progetto e' finito, col resoconto.
    Finito { quando: String, resoconto: String },
}

/// Dove sta un progetto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DoveSta {
    /// Aspetta che finisca quello prima.
    InCoda,
    /// Piano, legale dell'inizio, squadra.
    Prepara,
    /// Ha mostrato tutto, aspetta il via.
    AspettaIlVia,
    InCorso,
    Fermo,
    Finito,
}

/// Un compito del piano, com'e' adesso.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Lavoro {
    pub dot: String,
    pub compito: u64,
    /// `None` finche' il compito del Dot e' aperto.
    pub stato: Option<Stato>,
    pub esito: String,
}

/// Un progetto, com'e' adesso: le righe del suo diario rilette in ordine.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Progetto {
    pub nome: String,
    pub richiesta: String,
    pub cartella: String,
    pub creato: String,
    /// In che ordine e' nato: il primo aperto e' quello su cui si lavora.
    pub numero: u64,
    pub dove: DoveSta,
    /// La versione del piano su cui si lavora, o zero.
    pub versione: u32,
    /// Il compito dell'Architetto che si aspetta, se c'e'.
    pub piano_chiesto: Option<(u64, Option<u32>)>,
    /// I ruoli del piano, e chi li fa.
    pub squadra: BTreeMap<String, String>,
    pub tetto: f64,
    pub speso: f64,
    /// I compiti del piano affidati, per id.
    pub lavori: BTreeMap<String, Lavoro>,
    /// Le fasi finite: per numero, fatta o no.
    pub fasi: BTreeMap<u32, bool>,
    pub legale_inizio: Option<bool>,
    pub legale_fasi: BTreeMap<u32, bool>,
    pub legale_rilascio: Option<bool>,
    /// Le note del legale, le ultime.
    pub note_legali: Vec<String>,
    pub proposto: bool,
    pub perche_fermo: String,
    pub resoconto: String,
}

impl Progetto {
    /// Rilegge il diario di un progetto. Senza la riga `creato` non e' un
    /// progetto.
    pub fn da(nome: &str, eventi: &[Evento]) -> Option<Progetto> {
        let mut p = Progetto {
            nome: nome.to_string(),
            richiesta: String::new(),
            cartella: String::new(),
            creato: String::new(),
            numero: 0,
            dove: DoveSta::Prepara,
            versione: 0,
            piano_chiesto: None,
            squadra: BTreeMap::new(),
            tetto: 0.0,
            speso: 0.0,
            lavori: BTreeMap::new(),
            fasi: BTreeMap::new(),
            legale_inizio: None,
            legale_fasi: BTreeMap::new(),
            legale_rilascio: None,
            note_legali: Vec::new(),
            proposto: false,
            perche_fermo: String::new(),
            resoconto: String::new(),
        };
        let mut creato = false;
        // Prima del via, un fermo torna a preparare; dopo, a lavorare.
        let mut col_via = false;
        for e in eventi {
            match e {
                Evento::Creato {
                    quando,
                    numero,
                    richiesta,
                    cartella,
                } => {
                    creato = true;
                    p.creato.clone_from(quando);
                    p.numero = *numero;
                    p.richiesta.clone_from(richiesta);
                    p.cartella.clone_from(cartella);
                }
                Evento::PianoChiesto { compito, fase, .. } => {
                    p.piano_chiesto = Some((*compito, *fase))
                }
                Evento::PianoPronto {
                    versione, da_fase, ..
                } => {
                    p.piano_chiesto = None;
                    p.versione = *versione;
                    if let Some(n) = da_fase {
                        // La revisione rifa' la fase e quelle dopo: i loro
                        // compiti e i loro esiti non valgono piu'.
                        let prefisso =
                            |id: &str| id.split('.').next().and_then(|x| x.parse::<u32>().ok());
                        p.lavori
                            .retain(|id, _| prefisso(id).is_some_and(|f| f < *n));
                        p.fasi.retain(|f, _| f < n);
                        p.legale_fasi.retain(|f, _| f < n);
                    }
                }
                Evento::Legale {
                    fase,
                    rilascio,
                    ok,
                    note,
                    ..
                } => {
                    if *rilascio {
                        p.legale_rilascio = Some(*ok);
                    } else if let Some(n) = fase {
                        p.legale_fasi.insert(*n, *ok);
                    } else {
                        p.legale_inizio = Some(*ok);
                    }
                    p.note_legali.clone_from(note);
                }
                Evento::Squadra { ruolo, dot, .. } => {
                    p.squadra.insert(ruolo.clone(), dot.clone());
                }
                Evento::Proposto { tetto, .. } => {
                    p.proposto = true;
                    p.tetto = *tetto;
                    p.dove = DoveSta::AspettaIlVia;
                }
                Evento::Via { tetto, .. } => {
                    col_via = true;
                    p.tetto = *tetto;
                    p.dove = DoveSta::InCorso;
                }
                Evento::Affidato {
                    incarico,
                    dot,
                    compito,
                    ..
                } => {
                    p.lavori.insert(
                        incarico.clone(),
                        Lavoro {
                            dot: dot.clone(),
                            compito: *compito,
                            stato: None,
                            esito: String::new(),
                        },
                    );
                }
                Evento::Chiuso {
                    incarico,
                    stato,
                    esito,
                    ..
                } => {
                    // Fermato (dall'utente, o col progetto) o interrotto da
                    // troppi riavvii: non e' andato male, si rifa'.
                    if matches!(stato, Stato::Fermato | Stato::Interrotto) {
                        p.lavori.remove(incarico);
                    } else if let Some(l) = p.lavori.get_mut(incarico) {
                        l.stato = Some(*stato);
                        l.esito.clone_from(esito);
                    }
                }
                Evento::Fase { n, fatta, .. } => {
                    p.fasi.insert(*n, *fatta);
                }
                Evento::Spesa { usd, .. } => p.speso += usd,
                Evento::Fermo { perche, .. } => {
                    p.dove = DoveSta::Fermo;
                    p.perche_fermo.clone_from(perche);
                }
                Evento::Ripreso { tetto, .. } => {
                    p.tetto = *tetto;
                    p.perche_fermo.clear();
                    p.dove = if col_via {
                        DoveSta::InCorso
                    } else {
                        DoveSta::Prepara
                    };
                    // Ripreso prima del via: si riprepara, e si ripropone.
                    if !col_via {
                        p.proposto = false;
                    }
                }
                Evento::Finito { resoconto, .. } => {
                    p.dove = DoveSta::Finito;
                    p.resoconto.clone_from(resoconto);
                }
            }
        }
        creato.then_some(p)
    }

    /// Se il progetto e' ancora vivo: non finito.
    pub fn aperto(&self) -> bool {
        self.dove != DoveSta::Finito
    }

    /// I ruoli del piano che non hanno ancora un Dot, nell'ordine in cui
    /// compaiono, senza ripetizioni.
    pub fn ruoli_da_trovare(&self, piano: &Piano) -> Vec<(String, Mestiere)> {
        let mut fuori: Vec<(String, Mestiere)> = Vec::new();
        for i in piano.incarichi() {
            if let Chi::Assumi { ruolo, mestiere } = &i.chi {
                if !self.squadra.contains_key(ruolo) && !fuori.iter().any(|(r, _)| r == ruolo) {
                    fuori.push((ruolo.clone(), *mestiere));
                }
            }
        }
        fuori
    }

    /// Chi fa un compito del piano: il reparto, o chi AR ha trovato per il
    /// ruolo.
    pub fn chi_fa(&self, chi: &Chi) -> Option<String> {
        match chi {
            Chi::Reparto(r) => Some(r.clone()),
            Chi::Assumi { ruolo, .. } => self.squadra.get(ruolo).cloned(),
        }
    }

    /// La fase su cui si lavora: la prima non finita bene.
    pub fn fase_di_adesso<'a>(&self, piano: &'a Piano) -> Option<&'a piano::Fase> {
        piano
            .fasi
            .iter()
            .find(|f| self.fasi.get(&f.n) != Some(&true))
    }

    /// I compiti della fase di adesso che si possono affidare: non ancora
    /// affidati, e coi compiti da cui dipendono fatti.
    pub fn pronti<'a>(&self, piano: &'a Piano) -> Vec<&'a piano::Incarico> {
        self.fase_di_adesso(piano)
            .map(|f| self.pronti_in(f))
            .unwrap_or_default()
    }

    /// Come [`Progetto::pronti`], per una fase.
    pub fn pronti_in<'a>(&self, f: &'a piano::Fase) -> Vec<&'a piano::Incarico> {
        let fatto = |id: &str| {
            self.lavori
                .get(id)
                .is_some_and(|l| l.stato == Some(Stato::Fatto))
        };
        f.incarichi
            .iter()
            .filter(|i| !self.lavori.contains_key(&i.id))
            .filter(|i| i.dopo.iter().all(|d| fatto(d)))
            .collect()
    }

    /// Se in una fase non c'e' piu' niente da fare: nessun compito aperto, e
    /// nessuno da affidare. Quelli che restano aspettano un compito andato
    /// male, e non si potranno fare.
    pub fn fase_chiusa(&self, f: &piano::Fase) -> bool {
        let aperto = f
            .incarichi
            .iter()
            .any(|i| self.lavori.get(&i.id).is_some_and(|l| l.stato.is_none()));
        !aperto && self.pronti_in(f).is_empty()
    }

    /// I compiti della fase che non sono andati, col loro esito.
    pub fn andati_male(&self, f: &piano::Fase) -> Vec<String> {
        f.incarichi
            .iter()
            .filter_map(|i| {
                let l = self.lavori.get(&i.id)?;
                let s = l.stato?;
                (s != Stato::Fatto).then(|| {
                    format!(
                        "{} ({}, {}): {}",
                        i.id,
                        l.dot,
                        crate::nome_stato(s),
                        in_breve(&l.esito)
                    )
                })
            })
            .collect()
    }

    /// Le consegne della fase, una per compito, per il legale e la
    /// revisione.
    pub fn consegne(&self, f: &piano::Fase) -> String {
        f.incarichi
            .iter()
            .filter_map(|i| {
                let l = self.lavori.get(&i.id)?;
                Some(format!(
                    "- {} ({}): {}\n  {}",
                    i.id,
                    l.dot,
                    i.cosa,
                    in_breve(&l.esito).replace('\n', "\n  ")
                ))
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Le righe di un diario di progetto. Una che non si legge si salta.
pub fn eventi(testo: &str) -> Vec<Evento> {
    testo
        .lines()
        .filter_map(|r| serde_json::from_str(r).ok())
        .collect()
}

/// I passi di un progetto, dalla sua cartella.
pub fn leggi(dir: &Path) -> Vec<Evento> {
    eventi(&std::fs::read_to_string(dir.join("progetto.jsonl")).unwrap_or_default())
}

/// Un passo in fondo al diario del progetto.
pub fn annota(dir: &Path, e: &Evento) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|x| format!("{}: {x}", dir.display()))?;
    let t = serde_json::to_string(e).map_err(|x| x.to_string())?;
    crate::aggiungi_riga(&dir.join("progetto.jsonl"), &t)
}

/// Riscrive `progetto.md`, tutto insieme.
pub fn scrivi_racconto(dir: &Path, testo: &str) -> Result<(), String> {
    crate::scrivi_intero(&dir.join("progetto.md"), testo)
}

/// Un testo tagliato a [`CONSEGNA_IN_BREVE`] caratteri, e lo dice.
pub fn in_breve(t: &str) -> String {
    let t = t.trim();
    if t.chars().count() <= CONSEGNA_IN_BREVE {
        return t.to_string();
    }
    let corto: String = t.chars().take(CONSEGNA_IN_BREVE).collect();
    format!("{}… (tagliato)", corto.trim_end())
}

/// Il tetto che l'APM propone: [`TETTO_PER_COMPITO`] per compito, almeno
/// [`TETTO_MINIMO`]; zero se nella scala non c'e' niente a consumo.
pub fn tetto_proposto(piano: &Piano, ce_consumo: bool) -> f64 {
    if !ce_consumo {
        return 0.0;
    }
    let t = piano.incarichi().count() as f64 * TETTO_PER_COMPITO;
    (t.max(TETTO_MINIMO) * 100.0).round() / 100.0
}

/// Se la spesa ha toccato la soglia del tetto. Un tetto a zero non ferma:
/// vuol dire che non c'e' niente a consumo.
pub fn tetto_toccato(speso: f64, tetto: f64) -> bool {
    tetto > 0.0 && speso >= tetto * SOGLIA_DEL_TETTO
}

/// Il compito per un Dot: il progetto, la fase, cosa fare, e le consegne
/// dei compiti da cui dipende.
pub fn testo_del_compito(
    p: &Progetto,
    piano: &Piano,
    f: &piano::Fase,
    i: &piano::Incarico,
) -> String {
    let mut t = format!(
        "Progetto «{}», fase {} «{}» (consegna: {}). Compito {} del piano:\n{}",
        p.nome, f.n, f.nome, f.consegna, i.id, i.cosa
    );
    if !p.cartella.is_empty() {
        t.push_str(&format!("\nI file del progetto sono in «{}».", p.cartella));
    }
    let prima: Vec<String> = i
        .dopo
        .iter()
        .filter_map(|d| {
            let l = p.lavori.get(d)?;
            let cosa = piano
                .incarichi()
                .find(|x| x.id == *d)
                .map(|x| x.cosa.clone())
                .unwrap_or_default();
            Some(format!(
                "- {d} ({}): {cosa}\n  {}",
                l.dot,
                in_breve(&l.esito).replace('\n', "\n  ")
            ))
        })
        .collect();
    if !prima.is_empty() {
        t.push_str(&format!(
            "\n\nCio' che hanno consegnato i compiti prima di questo:\n{}",
            prima.join("\n")
        ));
    }
    t
}

/// Il messaggio con cui l'APM chiede il via: il piano, la squadra, il
/// tetto, il legale e le domande dell'Architetto.
pub fn proposta(p: &Progetto, piano: &Piano, file_piano: &str, tetto: f64) -> String {
    let mut t = format!(
        "Il progetto «{}» e' pronto a partire. Il piano, versione {}: {}, {}. E' in {file_piano}.",
        p.nome,
        piano.versione,
        piano::quanti(piano.fasi.len(), "fase", "fasi"),
        piano::quanti(piano.incarichi().count(), "compito", "compiti"),
    );
    let mut reparti: Vec<String> = piano
        .incarichi()
        .filter_map(|i| match &i.chi {
            Chi::Reparto(r) => Some(r.clone()),
            Chi::Assumi { .. } => None,
        })
        .collect();
    reparti.sort();
    reparti.dedup();
    t.push_str(&format!("\nLa squadra: i reparti {}", reparti.join(", ")));
    if p.squadra.is_empty() {
        t.push('.');
    } else {
        let assunti: Vec<String> = p
            .squadra
            .iter()
            .map(|(r, d)| format!("{d} ({r})"))
            .collect();
        t.push_str(&format!("; da AR: {}.", assunti.join(", ")));
    }
    if tetto > 0.0 {
        t.push_str(&format!(
            "\nIl tetto che propongo: {tetto:.2} $ per i cervelli a consumo; mi fermo al {:.0}% e ti chiedo.",
            SOGLIA_DEL_TETTO * 100.0
        ));
    } else {
        t.push_str("\nNella scala non ci sono cervelli a consumo: il progetto non costa niente.");
    }
    if p.legale_inizio == Some(false) {
        t.push_str("\nIl legale ha dei dubbi:");
        for n in &p.note_legali {
            t.push_str(&format!("\n- {n}"));
        }
    }
    if !piano.domande.is_empty() {
        t.push_str("\nLe domande dell'Architetto:");
        for d in &piano.domande {
            t.push_str(&format!("\n- {d}"));
        }
    }
    t.push_str("\nDammi il via, o dimmi cosa cambiare.");
    t
}

/// Il segno della domanda al legale.
pub const SEGNO_LEGALE: &str = "[legale]";

/// Il segno della domanda alla revisione, a fine fase.
pub const SEGNO_REVISIONE: &str = "[revisione di fase]";

/// Il segno della domanda per il resoconto.
pub const SEGNO_RESOCONTO: &str = "[resoconto]";

/// La domanda al legale. `quando` e' «all'inizio», «dopo la fase 2», «prima
/// del rilascio»; `materia` e' quel che guarda: il piano, o le consegne.
pub fn domanda_legale(p: &Progetto, quando: &str, materia: &str) -> String {
    format!(
        "{SEGNO_LEGALE} Sei il legale dell'azienda dei Dot. Guarda il progetto «{nome}» {quando}: \
         che rispetti le normative (dati personali, diritto d'autore e licenze, sicurezza, \
         contratti, e quel che vale per il suo campo). Nel dubbio lo dici.\n\n\
         Cosa vuole l'utente:\n{richiesta}\n\n{materia}\n\n\
         Rispondi cosi', e nient'altro:\n\
         ESITO: ok | problemi\n\
         - <un problema per riga, se ce ne sono>",
        nome = p.nome,
        richiesta = in_breve(&p.richiesta),
        materia = materia.trim(),
    )
}

/// La risposta del legale: ok o no, e i problemi. `None` se non si legge:
/// allora non si va avanti, perche' un controllo che non c'e' non e' un ok.
pub fn leggi_legale(t: &str) -> Option<(bool, Vec<String>)> {
    let mut esito = None;
    let mut note = Vec::new();
    for riga in t.lines() {
        let r = riga.replace("**", "");
        let r = r.trim();
        if let Some((k, v)) = r.split_once(':') {
            if k.trim().eq_ignore_ascii_case("esito") && esito.is_none() {
                let v = v.trim().to_lowercase();
                esito = if v.starts_with("ok") {
                    Some(true)
                } else if v.starts_with("problem") {
                    Some(false)
                } else {
                    return None;
                };
                continue;
            }
        }
        if let Some(n) = r.strip_prefix("- ").or_else(|| r.strip_prefix("* ")) {
            if !n.trim().is_empty() {
                note.push(n.trim().to_string());
            }
        }
    }
    let ok = esito?;
    if !ok && note.is_empty() {
        note.push("il legale ha visto problemi, ma non li ha scritti".into());
    }
    Some((ok, note))
}

/// La domanda alla revisione, a fine fase: la fase fatta come doveva?
pub fn domanda_revisione(p: &Progetto, f: &piano::Fase) -> String {
    format!(
        "{SEGNO_REVISIONE} Sei la revisione dell'azienda dei Dot. La fase {n} «{nome}» del \
         progetto «{progetto}» doveva consegnare: {consegna}. E' fatta quando: {quando}.\n\n\
         Le consegne dei compiti:\n{consegne}\n\n\
         Rispondi con due righe e nient'altro:\n\
         ESITO: fatta | non fatta\n\
         PERCHE: <una riga>",
        n = f.n,
        nome = f.nome,
        progetto = p.nome,
        consegna = f.consegna,
        quando = f.fatta_quando,
        consegne = p.consegne(f),
    )
}

/// La risposta della revisione: fatta o no, e perche'. `None` se non si
/// legge.
pub fn leggi_revisione(t: &str) -> Option<(bool, String)> {
    let mut fatta = None;
    let mut perche = String::new();
    for riga in t.lines() {
        let r = riga.replace("**", "");
        let Some((k, v)) = r.trim().split_once(':') else {
            continue;
        };
        match k.trim().to_uppercase().as_str() {
            "ESITO" if fatta.is_none() => {
                let v = v.trim().to_lowercase();
                fatta = if v.starts_with("non") {
                    Some(false)
                } else if v.starts_with("fatta") {
                    Some(true)
                } else {
                    return None;
                };
            }
            "PERCHE" | "PERCHÉ" | "PERCHE'" if perche.is_empty() => perche = v.trim().to_string(),
            _ => {}
        }
    }
    Some((fatta?, perche))
}

/// La domanda per il resoconto finale, che Nova legge all'utente.
pub fn domanda_resoconto(p: &Progetto, piano: &Piano) -> String {
    let fasi: Vec<String> = piano
        .fasi
        .iter()
        .map(|f| format!("Fase {} «{}»:\n{}", f.n, f.nome, p.consegne(f)))
        .collect();
    format!(
        "{SEGNO_RESOCONTO} Sei l'APM dell'azienda dei Dot. Il progetto «{nome}» e' finito. \
         Scrivi per l'utente un resoconto corto, in Markdown: cosa si e' fatto, cosa si e' \
         trovato, cosa resta aperto, dove sono le consegne.\n\n\
         Cosa voleva l'utente:\n{richiesta}\n\n{fasi}",
        nome = p.nome,
        richiesta = in_breve(&p.richiesta),
        fasi = fasi.join("\n\n"),
    )
}

/// Il resoconto senza un cervello: le fasi e le consegne, cosi' come sono.
pub fn resoconto_semplice(p: &Progetto, piano: &Piano) -> String {
    let mut t = format!("# Il progetto «{}» e' finito\n", p.nome);
    for f in &piano.fasi {
        t.push_str(&format!(
            "\n## Fase {}: {}\n{}\n",
            f.n,
            f.nome,
            p.consegne(f)
        ));
    }
    t
}

/// Com'e' il progetto, in Markdown, per chi apre `progetto.md`.
pub fn racconto(p: &Progetto, piano: Option<&Piano>) -> String {
    let dove = match p.dove {
        DoveSta::InCoda => "in coda: aspetta che finisca il progetto prima".to_string(),
        DoveSta::Prepara => "si prepara: piano, legale, squadra".to_string(),
        DoveSta::AspettaIlVia => "aspetta il via dell'utente".to_string(),
        DoveSta::InCorso => "in corso".to_string(),
        DoveSta::Fermo => format!("fermo: {}", p.perche_fermo),
        DoveSta::Finito => "finito".to_string(),
    };
    let mut t = format!(
        "# Progetto: {}\n\nNato: {}\nStato: {dove}\nPiano: versione {}\nTetto: {:.2} $\nSpeso: {:.2} $\n\n## Richiesta\n{}\n",
        p.nome, p.creato, p.versione, p.tetto, p.speso, p.richiesta.trim()
    );
    if !p.squadra.is_empty() {
        t.push_str("\n## Squadra trovata da AR\n");
        for (r, d) in &p.squadra {
            t.push_str(&format!("- {d}: {r}\n"));
        }
    }
    if let Some(piano) = piano {
        for f in &piano.fasi {
            let stato = match p.fasi.get(&f.n) {
                Some(true) => " (fatta)",
                Some(false) => " (non andata)",
                None => "",
            };
            t.push_str(&format!("\n## Fase {}: {}{stato}\n", f.n, f.nome));
            for i in &f.incarichi {
                let come = match p.lavori.get(&i.id) {
                    None => "da fare".to_string(),
                    Some(l) => match l.stato {
                        None => format!("{} ci lavora (compito n. {})", l.dot, l.compito),
                        Some(s) => format!("{}: {}", l.dot, crate::nome_stato(s)),
                    },
                };
                t.push_str(&format!("- {} {} — {come}\n", i.id, i.cosa));
            }
        }
    }
    if !p.note_legali.is_empty() {
        t.push_str("\n## Le ultime note del legale\n");
        for n in &p.note_legali {
            t.push_str(&format!("- {n}\n"));
        }
    }
    if !p.resoconto.is_empty() {
        t.push_str(&format!("\n## Resoconto\n{}\n", p.resoconto.trim()));
    }
    t
}

#[cfg(test)]
mod prove {
    use super::*;

    const PIANO: &str = "# Piano: p\nObiettivo: o\n\
        ## Fase 1: Stato dell'arte\nConsegna: un rapporto\nFatta quando: approvato\n\
        - 1.1 [ricerca] cercare\n- 1.2 [revisione] rivedere (dopo 1.1)\n\
        ## Fase 2: Prototipo\nConsegna: il codice\nFatta quando: le prove passano\n\
        - 2.1 [assumi: programmatore Rust] scrivere (dopo 1.2)\n\
        - 2.2 [assumi: programmatore Rust] documentare\n\
        - 2.3 [assumi ricercatore: esperto di brevetti] i brevetti\n";

    fn piano() -> Piano {
        piano::leggi(PIANO, 1).unwrap().0
    }

    fn t() -> String {
        "t".into()
    }

    fn base() -> Vec<Evento> {
        vec![
            Evento::Creato {
                quando: t(),
                numero: 1,
                richiesta: "un compressore".into(),
                cartella: String::new(),
            },
            Evento::PianoChiesto {
                quando: t(),
                compito: 1,
                fase: None,
            },
            Evento::PianoPronto {
                quando: t(),
                versione: 1,
                da_fase: None,
            },
        ]
    }

    fn chiuso(id: &str, s: Stato) -> Evento {
        Evento::Chiuso {
            quando: t(),
            incarico: id.into(),
            stato: s,
            esito: format!("esito di {id}"),
        }
    }

    fn affidato(id: &str, dot: &str, n: u64) -> Evento {
        Evento::Affidato {
            quando: t(),
            incarico: id.into(),
            dot: dot.into(),
            compito: n,
        }
    }

    #[test]
    fn senza_la_riga_creato_non_e_un_progetto() {
        assert!(Progetto::da("p", &[]).is_none());
        let p = Progetto::da("p", &base()).unwrap();
        assert_eq!(
            (p.dove, p.versione, p.piano_chiesto),
            (DoveSta::Prepara, 1, None)
        );
    }

    #[test]
    fn i_ruoli_si_trovano_una_volta_sola() {
        let mut e = base();
        let p = Progetto::da("p", &e).unwrap();
        assert_eq!(
            p.ruoli_da_trovare(&piano()),
            [
                ("programmatore Rust".to_string(), Mestiere::Generico),
                ("esperto di brevetti".to_string(), Mestiere::Ricercatore)
            ]
        );
        e.push(Evento::Squadra {
            quando: t(),
            ruolo: "programmatore Rust".into(),
            dot: "rustico".into(),
        });
        let p = Progetto::da("p", &e).unwrap();
        assert_eq!(p.ruoli_da_trovare(&piano()).len(), 1);
        assert_eq!(
            p.chi_fa(&Chi::Assumi {
                ruolo: "programmatore Rust".into(),
                mestiere: Mestiere::Generico
            })
            .as_deref(),
            Some("rustico")
        );
        assert_eq!(
            p.chi_fa(&Chi::Reparto("ricerca".into())).as_deref(),
            Some("ricerca")
        );
    }

    #[test]
    fn si_affida_fase_per_fase_quando_le_dipendenze_sono_fatte() {
        let mut e = base();
        e.push(Evento::Via {
            quando: t(),
            tetto: 3.0,
        });
        let p = Progetto::da("p", &e).unwrap();
        assert_eq!(p.dove, DoveSta::InCorso);
        let ids = |p: &Progetto| {
            p.pronti(&piano())
                .iter()
                .map(|i| i.id.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            ids(&p),
            ["1.1"],
            "1.2 aspetta 1.1, e la fase 2 aspetta la 1"
        );
        e.push(affidato("1.1", "ricerca", 4));
        assert!(
            ids(&Progetto::da("p", &e).unwrap()).is_empty(),
            "affidato non si riaffida"
        );
        e.push(chiuso("1.1", Stato::Fatto));
        assert_eq!(ids(&Progetto::da("p", &e).unwrap()), ["1.2"]);
        e.extend([affidato("1.2", "revisione", 5), chiuso("1.2", Stato::Fatto)]);
        let p = Progetto::da("p", &e).unwrap();
        assert!(p.fase_chiusa(&piano().fasi[0]));
        assert!(
            ids(&p).is_empty(),
            "la fase 1 e' chiusa ma non ancora giudicata"
        );
        e.push(Evento::Fase {
            quando: t(),
            n: 1,
            fatta: true,
            perche: String::new(),
        });
        let p = Progetto::da("p", &e).unwrap();
        assert_eq!(ids(&p), ["2.1", "2.2", "2.3"], "2.1 dipende da 1.2, fatto");
        assert!(p
            .consegne(&piano().fasi[0])
            .contains("- 1.1 (ricerca): cercare\n  esito di 1.1"));
    }

    #[test]
    fn un_compito_fermato_si_rifa() {
        let mut e = base();
        e.extend([
            Evento::Via {
                quando: t(),
                tetto: 0.0,
            },
            affidato("1.1", "ricerca", 4),
            chiuso("1.1", Stato::Fermato),
        ]);
        let p = Progetto::da("p", &e).unwrap();
        assert!(p.lavori.is_empty());
        assert_eq!(
            p.pronti(&piano())
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            ["1.1"]
        );
    }

    #[test]
    fn un_compito_andato_male_chiude_la_fase_e_si_dice() {
        let mut e = base();
        e.extend([
            Evento::Via {
                quando: t(),
                tetto: 0.0,
            },
            affidato("1.1", "ricerca", 4),
            chiuso("1.1", Stato::Fallito),
        ]);
        let p = Progetto::da("p", &e).unwrap();
        assert!(p.fase_chiusa(&piano().fasi[0]), "1.2 non si puo' piu' fare");
        assert_eq!(
            p.andati_male(&piano().fasi[0]),
            ["1.1 (ricerca, fallito): esito di 1.1"]
        );
    }

    #[test]
    fn una_revisione_per_fase_rifa_da_quella_fase() {
        let mut e = base();
        e.extend([
            Evento::Via {
                quando: t(),
                tetto: 0.0,
            },
            affidato("1.1", "ricerca", 4),
            chiuso("1.1", Stato::Fatto),
            affidato("1.2", "revisione", 5),
            chiuso("1.2", Stato::Fatto),
            Evento::Fase {
                quando: t(),
                n: 1,
                fatta: true,
                perche: String::new(),
            },
            affidato("2.1", "rustico", 6),
            chiuso("2.1", Stato::Fallito),
            Evento::Fase {
                quando: t(),
                n: 2,
                fatta: false,
                perche: "no".into(),
            },
            Evento::PianoChiesto {
                quando: t(),
                compito: 7,
                fase: Some(2),
            },
            Evento::PianoPronto {
                quando: t(),
                versione: 2,
                da_fase: Some(2),
            },
        ]);
        let p = Progetto::da("p", &e).unwrap();
        assert_eq!(p.versione, 2);
        assert_eq!(p.lavori.keys().collect::<Vec<_>>(), ["1.1", "1.2"]);
        assert_eq!(p.fasi.get(&1), Some(&true));
        assert_eq!(p.fasi.get(&2), None);
    }

    #[test]
    fn fermo_e_ripreso_tornano_dove_erano() {
        let mut e = base();
        e.push(Evento::Fermo {
            quando: t(),
            perche: "il piano non si e' fatto".into(),
        });
        assert_eq!(Progetto::da("p", &e).unwrap().dove, DoveSta::Fermo);
        e.push(Evento::Ripreso {
            quando: t(),
            tetto: 0.0,
        });
        assert_eq!(
            Progetto::da("p", &e).unwrap().dove,
            DoveSta::Prepara,
            "prima del via si riprepara"
        );
        e.extend([
            Evento::Proposto {
                quando: t(),
                tetto: 2.0,
            },
            Evento::Via {
                quando: t(),
                tetto: 2.0,
            },
        ]);
        e.push(Evento::Spesa {
            quando: t(),
            usd: 1.5,
            per: "x".into(),
        });
        e.push(Evento::Fermo {
            quando: t(),
            perche: "tetto".into(),
        });
        e.push(Evento::Ripreso {
            quando: t(),
            tetto: 5.0,
        });
        let p = Progetto::da("p", &e).unwrap();
        assert_eq!((p.dove, p.tetto, p.speso), (DoveSta::InCorso, 5.0, 1.5));
        e.push(Evento::Finito {
            quando: t(),
            resoconto: "fatto".into(),
        });
        assert!(!Progetto::da("p", &e).unwrap().aperto());
    }

    #[test]
    fn il_tetto_si_propone_e_si_tocca_al_novanta_per_cento() {
        assert_eq!(tetto_proposto(&piano(), true), 1.5, "cinque compiti");
        assert_eq!(tetto_proposto(&piano::leggi("# Piano: p\nObiettivo: o\n## Fase 1: a\nConsegna: c\nFatta quando: f\n- 1.1 [dati] x\n", 1).unwrap().0, true), 1.0);
        assert_eq!(tetto_proposto(&piano(), false), 0.0);
        assert!(!tetto_toccato(0.89, 1.0) && tetto_toccato(0.9, 1.0));
        assert!(
            !tetto_toccato(100.0, 0.0),
            "a zero non c'e' niente a consumo"
        );
    }

    #[test]
    fn il_legale_e_la_revisione_si_leggono() {
        assert_eq!(leggi_legale("ESITO: ok"), Some((true, vec![])));
        assert_eq!(
            leggi_legale("**Esito:** problemi\n- licenza GPL\n- dati personali"),
            Some((false, vec!["licenza GPL".into(), "dati personali".into()]))
        );
        assert_eq!(
            leggi_legale("ESITO: problemi").unwrap().1,
            ["il legale ha visto problemi, ma non li ha scritti"]
        );
        assert_eq!(leggi_legale("Va bene."), None);
        assert_eq!(leggi_legale("ESITO: boh"), None);
        assert_eq!(
            leggi_revisione("ESITO: fatta\nPERCHE: completo"),
            Some((true, "completo".into()))
        );
        assert_eq!(
            leggi_revisione("ESITO: non fatta\nPERCHE: mancano le fonti"),
            Some((false, "mancano le fonti".into()))
        );
        assert_eq!(leggi_revisione("ESITO: forse"), None);
        assert_eq!(leggi_revisione("Va bene."), None);
    }

    #[test]
    fn il_compito_porta_il_progetto_la_fase_e_cio_che_viene_prima() {
        let mut e = base();
        e[0] = Evento::Creato {
            quando: t(),
            numero: 1,
            richiesta: "r".into(),
            cartella: "/tmp/x".into(),
        };
        e.extend([affidato("1.1", "ricerca", 4), chiuso("1.1", Stato::Fatto)]);
        let p = Progetto::da("p", &e).unwrap();
        let pi = piano();
        let testo = testo_del_compito(&p, &pi, &pi.fasi[0], &pi.fasi[0].incarichi[1]);
        assert!(testo.starts_with("Progetto «p», fase 1 «Stato dell'arte» (consegna: un rapporto). Compito 1.2 del piano:\nrivedere"), "{testo}");
        assert!(testo.contains("I file del progetto sono in «/tmp/x»."));
        assert!(testo.contains("Cio' che hanno consegnato i compiti prima di questo:\n- 1.1 (ricerca): cercare\n  esito di 1.1"), "{testo}");
    }

    #[test]
    fn la_proposta_dice_tutto_quello_che_serve_per_il_via() {
        let mut e = base();
        e.push(Evento::Squadra {
            quando: t(),
            ruolo: "programmatore Rust".into(),
            dot: "rustico".into(),
        });
        e.push(Evento::Legale {
            quando: t(),
            fase: None,
            rilascio: false,
            ok: false,
            note: vec!["licenze".into()],
        });
        let p = Progetto::da("p", &e).unwrap();
        let mut pi = piano();
        pi.domande = vec!["quanto spendi?".into()];
        let t = proposta(&p, &pi, "/x/piano-1.md", 1.5);
        assert!(t.starts_with("Il progetto «p» e' pronto a partire. Il piano, versione 1: 2 fasi, 5 compiti. E' in /x/piano-1.md."), "{t}");
        assert!(
            t.contains(
                "La squadra: i reparti revisione, ricerca; da AR: rustico (programmatore Rust)."
            ),
            "{t}"
        );
        assert!(t.contains("Il tetto che propongo: 1.50 $ per i cervelli a consumo; mi fermo al 90% e ti chiedo."), "{t}");
        assert!(
            t.contains("Il legale ha dei dubbi:\n- licenze")
                && t.contains("Le domande dell'Architetto:\n- quanto spendi?"),
            "{t}"
        );
        assert!(proposta(&p, &pi, "f", 0.0).contains("non ci sono cervelli a consumo"));
    }

    #[test]
    fn il_diario_va_e_torna_e_il_racconto_si_legge() {
        let mut e = base();
        e.extend([
            Evento::Via {
                quando: t(),
                tetto: 1.0,
            },
            affidato("1.1", "ricerca", 4),
        ]);
        let righe: String = e
            .iter()
            .map(|x| serde_json::to_string(x).unwrap() + "\n")
            .collect();
        assert_eq!(eventi(&(righe.clone() + "{rotta\n")), e);
        assert!(
            righe
                .lines()
                .next()
                .unwrap()
                .starts_with("{\"tipo\":\"creato\""),
            "{righe}"
        );
        let p = Progetto::da("p", &e).unwrap();
        let r = racconto(&p, Some(&piano()));
        assert!(r.starts_with("# Progetto: p\n\nNato: t\nStato: in corso\nPiano: versione 1\nTetto: 1.00 $\nSpeso: 0.00 $\n"), "{r}");
        assert!(
            r.contains(
                "- 1.1 cercare — ricerca ci lavora (compito n. 4)\n- 1.2 rivedere — da fare\n"
            ),
            "{r}"
        );
    }

    #[test]
    fn i_progetti_si_elencano_dalla_cartella() {
        let d = std::env::temp_dir().join(format!("nova-progetti-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        assert!(elenco(&d).is_empty());
        for n in ["b", "a"] {
            std::fs::create_dir_all(cartella(&d, n)).unwrap();
            std::fs::write(cartella(&d, n).join("progetto.jsonl"), "").unwrap();
        }
        std::fs::create_dir_all(cartella(&d, "vuoto")).unwrap();
        assert_eq!(elenco(&d), ["a", "b"]);
        let _ = std::fs::remove_dir_all(&d);
    }
}
