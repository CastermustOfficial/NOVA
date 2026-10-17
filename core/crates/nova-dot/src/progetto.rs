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
//! Dal D407 stanno nella cartella del progetto, `Documenti\Dots_Project\
//! <nome>\` o quella indicata, con il resto della sua struttura
//! ([`crate::struttura`]); nella cartella di NOVA resta l'indice. Prima del
//! D407 stavano in `<cartella di NOVA>/progetti/<nome>/` ([`cartella_vecchia`]),
//! coi piani in `piani/`: il demone li sposta alla prima accensione.
//!
//! Qui ci sono le regole e le parole; chi le fa girare e' `nova_core::apm`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::piano::{self, Chi, Piano};
use crate::{Mestiere, Stato};

/// Dove stavano i progetti prima del D407, dentro la cartella di NOVA.
pub const CARTELLA: &str = "progetti";
/// Dove stavano i piani prima del D407, dentro la cartella del progetto.
pub const CARTELLA_PIANI_VECCHIA: &str = "piani";

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

/// La cartella di un progetto prima del D407, dentro quella di NOVA.
pub fn cartella_vecchia(nova: &Path, nome: &str) -> PathBuf {
    nova.join(CARTELLA).join(nome)
}

/// I progetti rimasti dove stavano prima del D407: da spostare.
pub fn vecchi(nova: &Path) -> Vec<String> {
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

/// I progetti che ci sono, in ordine di nome: quelli dell'indice la cui
/// cartella ha ancora la cronologia.
pub fn elenco(nova: &Path) -> Vec<String> {
    crate::struttura::progetti(nova)
        .into_iter()
        .filter(|(_, r)| Path::new(r).join("progetto.jsonl").is_file())
        .map(|(n, _)| n)
        .collect()
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
    /// una revisione per una fase che non e' andata. `da_fase` e' da dove si
    /// rifa' col piano nuovo (vuoto: dalla `fase`); `cambio`, se lo chiede
    /// l'utente in corsa (D403).
    PianoChiesto {
        quando: String,
        compito: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fase: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        da_fase: Option<u32>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        cambio: bool,
        /// La stessa domanda di prima, rifatta perche' l'utente ha fatto
        /// ripartire il progetto (D411): un cambio arrivato nel frattempo
        /// non c'e' dentro, e resta da chiedere.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        di_nuovo: bool,
    },
    /// Il piano e' pronto, in quella versione. Se era una revisione, i
    /// compiti da quella fase in poi si rifanno col piano nuovo, fuori che
    /// quelli fatti che il piano nuovo lascia uguali (`tenuti`).
    PianoPronto {
        quando: String,
        versione: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        da_fase: Option<u32>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tenuti: Vec<String>,
    },
    /// L'utente vuole cambiare qualcosa (D403, `dot.cambia_progetto`).
    /// L'APM lascia finire i compiti in corso, non ne affida di nuovi, e
    /// chiede all'Architetto di rivedere il piano dalla fase di adesso.
    Cambio { quando: String, cosa: String },
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
    /// Un Dot, mentre lavorava a un compito, ha chiesto qualcosa al legale,
    /// all'APM o a un collega, e questa e' la risposta (D419). Non cambia lo
    /// stato del progetto: resta scritto, perche' un parere chiesto e avuto
    /// si ritrova.
    Domanda {
        quando: String,
        da: String,
        a: String,
        incarico: String,
        domanda: String,
        risposta: String,
    },
}

/// Quanti aiuti puo' avere chi fa un ruolo del piano (D422), oltre a lui:
/// piu' Dot insieme finiscono prima, ma consumano le quote insieme.
pub const AIUTI_PER_RUOLO: usize = 2;

/// Chi fa un compito pronto (D422).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChiLibero {
    /// Chi lo fa di solito, ed e' libero.
    Lui(String),
    /// Un suo aiuto, libero.
    Aiuto(String),
    /// Un aiuto da trovare con AR, per questo ruolo; `di` e' chi lo fa di
    /// solito.
    DaTrovare { ruolo: String, di: String },
    /// Sono tutti occupati: il compito va in coda a chi lo fa di solito.
    InCoda(String),
}

/// Il ruolo di chi fa un compito: il reparto, o il ruolo da assumere.
pub fn ruolo_di(chi: &Chi) -> String {
    match chi {
        Chi::Reparto(r) => r.clone(),
        Chi::Assumi { ruolo, .. } => ruolo.clone(),
    }
}

/// Come si chiama nella squadra il `k`-esimo aiuto di un ruolo.
pub fn ruolo_dell_aiuto(ruolo: &str, k: usize) -> String {
    format!("{ruolo} (aiuto {k})")
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
    /// Il compito dell'Architetto che si aspetta, se c'e', e da che fase si
    /// rifa' col piano nuovo.
    pub piano_chiesto: Option<(u64, Option<u32>)>,
    /// Il piano che si aspetta l'ha chiesto l'utente in corsa (D403).
    pub piano_per_cambio: bool,
    /// L'utente ha fatto ripartire il progetto mentre si aspettava un piano
    /// (D411): se il compito dell'Architetto e' andato male, si richiede,
    /// invece di fermarsi di nuovo per lo stesso motivo.
    pub piano_da_richiedere: bool,
    /// Il cambio chiesto dall'utente e non ancora passato all'Architetto.
    pub cambio: Option<String>,
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
            piano_per_cambio: false,
            piano_da_richiedere: false,
            cambio: None,
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
                Evento::PianoChiesto {
                    compito,
                    fase,
                    da_fase,
                    cambio,
                    di_nuovo,
                    ..
                } => {
                    p.piano_chiesto = Some((*compito, da_fase.or(*fase)));
                    p.piano_per_cambio = *cambio;
                    p.piano_da_richiedere = false;
                    if *cambio && !*di_nuovo {
                        p.cambio = None;
                    }
                }
                Evento::PianoPronto {
                    versione,
                    da_fase,
                    tenuti,
                    ..
                } => {
                    p.piano_chiesto = None;
                    p.piano_da_richiedere = false;
                    p.versione = *versione;
                    if let Some(n) = da_fase {
                        // La revisione rifa' la fase e quelle dopo: i loro
                        // compiti e i loro esiti non valgono piu', fuori che
                        // quelli fatti che il piano nuovo lascia uguali.
                        let prefisso =
                            |id: &str| id.split('.').next().and_then(|x| x.parse::<u32>().ok());
                        p.lavori.retain(|id, l| {
                            prefisso(id).is_some_and(|f| f < *n)
                                || (tenuti.contains(id) && l.stato == Some(Stato::Fatto))
                        });
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
                Evento::Cambio { cosa, .. } => {
                    // Due cambi prima che l'Architetto ne veda uno: li vede
                    // tutti e due.
                    p.cambio = Some(match p.cambio.take() {
                        Some(prima) => format!("{prima}\n{cosa}"),
                        None => cosa.clone(),
                    });
                    // Prima del via si riprepara: il legale riguarda il
                    // piano nuovo, e la proposta si rifa'.
                    if !col_via {
                        p.legale_inizio = None;
                        p.proposto = false;
                        if p.dove == DoveSta::AspettaIlVia {
                            p.dove = DoveSta::Prepara;
                        }
                    }
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
                    // Un piano chiesto e non arrivato (D411): se il compito
                    // dell'Architetto e' andato male, l'APM lo richiede.
                    p.piano_da_richiedere = p.piano_chiesto.is_some();
                    p.dove = if col_via {
                        DoveSta::InCorso
                    } else {
                        DoveSta::Prepara
                    };
                    // Ripreso prima del via: si riprepara, e si ripropone.
                    if !col_via {
                        p.proposto = false;
                    }
                    // Un paper non fatto si riaffida (D407): era per lui che
                    // il progetto si era fermato.
                    if p.lavori.get(crate::struttura::INCARICO_PAPER).is_some_and(|l| {
                        l.stato.is_some_and(|s| s != Stato::Fatto)
                    }) {
                        p.lavori.remove(crate::struttura::INCARICO_PAPER);
                    }
                }
                Evento::Domanda { .. } => {}
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

    /// I Dot che hanno un compito aperto nel progetto.
    pub fn occupati(&self) -> BTreeSet<String> {
        self.lavori
            .values()
            .filter(|l| l.stato.is_none())
            .map(|l| l.dot.clone())
            .collect()
    }

    /// Chi fa un compito pronto, se chi lo fa di solito e' occupato (D422):
    /// lui, se e' libero; se no un suo aiuto libero; se no un aiuto da
    /// trovare, fino a [`AIUTI_PER_RUOLO`]; se no lui, e il compito aspetta
    /// in coda. `occupati` sono i Dot che hanno un compito aperto, compresi
    /// quelli appena affidati.
    pub fn chi_e_libero(&self, chi: &Chi, occupati: &BTreeSet<String>) -> Option<ChiLibero> {
        let primo = self.chi_fa(chi)?;
        if !occupati.contains(&primo) {
            return Some(ChiLibero::Lui(primo));
        }
        let base = ruolo_di(chi);
        let aiuti: Vec<&String> = (1..=AIUTI_PER_RUOLO)
            .filter_map(|k| self.squadra.get(&ruolo_dell_aiuto(&base, k)))
            .collect();
        if let Some(a) = aiuti.iter().find(|a| !occupati.contains(**a)) {
            return Some(ChiLibero::Aiuto((*a).clone()));
        }
        if aiuti.len() < AIUTI_PER_RUOLO {
            return Some(ChiLibero::DaTrovare {
                ruolo: ruolo_dell_aiuto(&base, aiuti.len() + 1),
                di: primo,
            });
        }
        Some(ChiLibero::InCoda(primo))
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

    /// Se un compito che usa la scheda video e' aperto (D424, D426).
    pub fn scheda_aperta(&self, piano: &Piano) -> bool {
        piano
            .incarichi()
            .any(|i| i.scheda && self.lavori.get(&i.id).is_some_and(|l| l.stato.is_none()))
    }

    /// I compiti pronti che si affidano adesso (D424): tutti, tranne quelli
    /// che usano la scheda video quando ce n'e' gia' uno aperto; e di quelli,
    /// uno alla volta. Il 15 ottobre 1.33 e 1.34 di S0ex1, due misure, sono
    /// andati insieme sulla stessa scheda.
    pub fn da_affidare_in<'a>(
        &self,
        piano: &Piano,
        f: &'a piano::Fase,
    ) -> Vec<&'a piano::Incarico> {
        let mut scheda = self.scheda_aperta(piano);
        self.pronti_in(f)
            .into_iter()
            .filter(|i| {
                if !i.scheda {
                    return true;
                }
                if scheda {
                    return false;
                }
                scheda = true;
                true
            })
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

/// Quello che sa chi ha un compito sulla scheda video (D424).
pub const SCHEDA_TUA: &str = "\nQuesto compito usa la scheda video: finche' non hai finito, NOVA \
non fa partire altri compiti che la usano, e CLM, il suo giudice, la lascia libera (D426). Se la \
trovi occupata lo stesso, guarda chi la occupa e scrivilo con `ASPETTO LA SCHEDA VIDEO`.";

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
    if i.scheda {
        t.push_str(SCHEDA_TUA);
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

/// La squadra del progetto e la sua chat, in coda al testo di un compito
/// (D416): chi c'e', chi lavora adesso e a cosa, e dove ci si parla.
pub fn squadra_e_chat(p: &Progetto, chi: &str, gruppo: Option<&str>) -> String {
    let mut squadra: Vec<&str> = p.lavori.values().map(|l| l.dot.as_str()).collect();
    squadra.push(chi);
    squadra.sort_unstable();
    squadra.dedup();
    let adesso: Vec<String> = p
        .lavori
        .iter()
        .filter(|(_, l)| l.stato.is_none() && l.dot != chi)
        .map(|(incarico, l)| format!("{} ({incarico})", l.dot))
        .collect();
    let mut t = format!("\n\nLa squadra del progetto: {}.", squadra.join(", "));
    t.push_str(&if adesso.is_empty() {
        " Adesso non lavora nessun altro.".to_string()
    } else {
        format!(" Adesso lavorano: {}.", adesso.join(", "))
    });
    if let Some(g) = gruppo {
        t.push_str(&format!(
            " La chat del progetto e' il gruppo «{g}»: se hai trovato qualcosa che serve agli \
             altri, scrivilo li', con una riga `{chat}: <il messaggio>` (o con dot.scrivi a \
             «gruppo:{g}», se hai gli strumenti di NOVA). Gli altri lo leggono al loro prossimo \
             compito, e lo legge Gio. Le consegne degli altri sono nella cartella del progetto, \
             e l'indice le elenca: leggi quelle che ti servono.",
            chat = crate::domande::ALLA_CHAT,
        ));
    }
    // Chiedere mentre si lavora (D419): al legale, all'APM, a un collega.
    let squadra: Vec<String> = squadra.iter().map(|s| (*s).to_string()).collect();
    t.push_str(&crate::domande::regola(chi, &squadra));
    t
}

/// Il progetto e il compito del piano di un compito, dal suo testo: quello
/// che [`testo_del_compito`] scrive in testa. `None` per un compito che non
/// e' di un progetto.
pub fn progetto_dal_testo(testo: &str) -> Option<(String, String)> {
    let dopo = testo.trim_start().strip_prefix("Progetto «")?;
    let (nome, resto) = dopo.split_once('»')?;
    let (_, compito) = resto.split_once(". Compito ")?;
    let (incarico, _) = compito.split_once(" del piano:")?;
    let (nome, incarico) = (nome.trim(), incarico.trim());
    (!nome.is_empty() && !incarico.is_empty() && !incarico.contains('\n'))
        .then(|| (nome.to_string(), incarico.to_string()))
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

/// I compiti fatti che un piano nuovo lascia uguali, da `da_fase` in poi:
/// stesso id, stessa cosa, stesso chi. Non si rifanno (D403).
pub fn tenuti(p: &Progetto, vecchio: &Piano, nuovo: &Piano, da_fase: u32) -> Vec<String> {
    let fase_di = |id: &str| id.split('.').next().and_then(|x| x.parse::<u32>().ok());
    p.lavori
        .iter()
        .filter(|(id, l)| fase_di(id).is_some_and(|f| f >= da_fase) && l.stato == Some(Stato::Fatto))
        .filter(|(id, _)| {
            let prima = vecchio.incarichi().find(|i| i.id == **id);
            let dopo = nuovo.incarichi().find(|i| i.id == **id);
            matches!((prima, dopo), (Some(a), Some(b)) if a.cosa == b.cosa && a.chi == b.chi)
        })
        .map(|(id, _)| id.clone())
        .collect()
}

/// I compiti fatti, da `da_fase` in poi, che il piano nuovo segna «(fatto)»
/// (D431): l'Architetto dice che non vanno rifatti, anche se una fase non e'
/// andata o se ne ha cambiato il testo. Un compito segnato che non e' fatto
/// si affida come gli altri.
pub fn segnati_fatti(p: &Progetto, nuovo: &Piano, da_fase: u32) -> Vec<String> {
    let fase_di = |id: &str| id.split('.').next().and_then(|x| x.parse::<u32>().ok());
    p.lavori
        .iter()
        .filter(|(id, l)| {
            fase_di(id).is_some_and(|f| f >= da_fase) && l.stato == Some(Stato::Fatto)
        })
        .filter(|(id, _)| nuovo.incarichi().any(|i| i.id == **id && i.fatto))
        .map(|(id, _)| id.clone())
        .collect()
}

/// Quanto costerebbe, alla stima del tetto, quel che resta da fare col
/// piano nuovo: i compiti da `da_fase` in poi, meno quelli tenuti.
pub fn costo_del_resto(nuovo: &Piano, da_fase: u32, tenuti: &[String]) -> f64 {
    let quanti = nuovo
        .fasi
        .iter()
        .filter(|f| f.n >= da_fase)
        .flat_map(|f| f.incarichi.iter())
        .filter(|i| !tenuti.contains(&i.id))
        .count();
    ((quanti as f64 * TETTO_PER_COMPITO) * 100.0).round() / 100.0
}

/// Il testo per l'Architetto quando l'utente chiede un cambio: cosa
/// cambiare e, a lavoro iniziato, da che fase (quelle prima restano).
pub fn testo_del_cambio(cosa: &str, da_fase: Option<u32>) -> String {
    match da_fase {
        Some(n) if n > 1 => format!(
            "{}\nIl lavoro e' alla fase {n}: le fasi prima restano come sono, si cambia da li'.",
            cosa.trim()
        ),
        _ => cosa.trim().to_string(),
    }
}

/// Se il resto del lavoro, col piano nuovo, passa il tetto: allora si
/// chiede il via con un tetto nuovo. Senza tetto non si chiede.
pub fn serve_un_tetto_nuovo(p: &Progetto, serve: f64) -> bool {
    p.tetto > 0.0 && p.speso + serve > p.tetto
}

/// Cosa dire quando il piano cambia in corsa: cosa cambia, e se si riparte
/// o serve un tetto nuovo.
pub fn dopo_il_cambio(p: &Progetto, nuovo: &Piano, file: &str, serve: f64) -> String {
    let mut t = format!(
        "Il piano di «{}» e' cambiato, versione {}. E' in {file}.",
        p.nome, nuovo.versione
    );
    if !nuovo.cambia.is_empty() {
        t.push_str("\nCosa cambia:");
        for c in &nuovo.cambia {
            t.push_str(&format!("\n- {c}"));
        }
    }
    // Le domande del piano nuovo (D410): prima si perdevano, e quelle di un
    // piano cambiato in corsa non le leggeva nessuno.
    if !nuovo.domande.is_empty() {
        t.push_str("\nLe domande dell'Architetto:");
        for d in &nuovo.domande {
            t.push_str(&format!("\n- {d}"));
        }
    }
    if serve_un_tetto_nuovo(p, serve) {
        t.push_str(&format!(
            "\nPer finire servono circa {serve:.2} $, e col gia' speso ({:.2} $) si passa il tetto di {:.2} $: mi fermo, e aspetto il via con un tetto nuovo.",
            p.speso, p.tetto
        ));
    } else {
        t.push_str("\nRiparto.");
    }
    t
}

/// La frase detta a voce quando un progetto finisce (D403): corta, senza
/// percorsi. Il resoconto e' in chat.
pub fn frase_di_fine(nome: &str) -> String {
    format!("Il progetto {nome} e' finito: il resoconto e' in chat.")
}

/// Le righe che Nova ha nel prompt coi Dot accesi (D403): davanti a un
/// lavoro grosso propone di darlo ai Dot. Corte, perche' le legge anche il
/// modello di casa, che ha il contesto contato: col modello di casa al
/// posto di `dot.progetto` c'e' la delega a Claude con gli strumenti di
/// NOVA, se nella scala c'e' Claude.
pub fn per_nova(con_claude: bool) -> String {
    let senza = if con_claude {
        "cervelli_delega con dot: true"
    } else {
        "di' che serve Claude"
    };
    format!(
        "## Progetti dei Dot\nUn lavoro grosso (molti passi, ricerca, codice) proponi di darlo \
         ai Dot; al si', dot_progetto, o se non c'e' {senza}."
    )
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
                da_fase: None,
                cambio: false,
                di_nuovo: false,
            },
            Evento::PianoPronto {
                quando: t(),
                versione: 1,
                da_fase: None,
                tenuti: Vec::new(),
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
    fn un_compito_dice_la_squadra_chi_lavora_e_dove_ci_si_parla() {
        // D416: i Dot di S0ex1 non sapevano degli altri.
        let mut p = Progetto::da("s0ex1", &base()).unwrap();
        p.lavori.clear();
        for (incarico, dot, stato) in [
            ("1.1", "ricerca", None),
            ("1.4", "dati", Some(Stato::Fatto)),
            ("1.6", "dati", None),
        ] {
            p.lavori.insert(
                incarico.into(),
                Lavoro { dot: dot.into(), compito: 1, stato, esito: String::new() },
            );
        }
        let t = squadra_e_chat(&p, "qualita", Some("s0ex1"));
        assert!(t.contains("La squadra del progetto: dati, qualita, ricerca."), "{t}");
        assert!(t.contains("Adesso lavorano: ricerca (1.1), dati (1.6)."), "{t}");
        assert!(t.contains("dot.scrivi a «gruppo:s0ex1»") && t.contains("lo legge Gio"), "{t}");
        // D419: anche chi lavora con una CLI senza gli strumenti di NOVA
        // scrive alla chat, e chiede mentre lavora.
        assert!(t.contains("con una riga `ALLA CHAT: <il messaggio>`"), "{t}");
        assert!(t.contains("`CHIEDO A <chi>: <la domanda>`, dove <chi> e' uno fra legale, apm, dati, ricerca, utente"), "{t}");
        p.lavori.clear();
        let solo = squadra_e_chat(&p, "dati", None);
        assert!(solo.contains("Adesso non lavora nessun altro.") && !solo.contains("gruppo"), "{solo}");
        assert!(solo.contains("uno fra legale, apm, utente; la domanda puo' continuare"), "{solo}");
    }

    #[test]
    fn la_scheda_video_un_compito_alla_volta() {
        // D424: il 15 ottobre 1.33 e 1.34 di S0ex1 hanno misurato insieme.
        let pi = piano::leggi(
            "# Piano: p\nObiettivo: o\n## Fase 1: Banco\nConsegna: c\nFatta quando: f\n\
             - 1.1 [dati] misurare A (scheda video)\n- 1.2 [dati] misurare B (scheda video)\n\
             - 1.3 [ricerca] leggere\n- 1.4 [qualita] ripetere A (scheda video) (dopo 1.1)\n",
            1,
        )
        .unwrap()
        .0;
        let f = &pi.fasi[0];
        let ids = |v: Vec<&piano::Incarico>| v.iter().map(|i| i.id.clone()).collect::<Vec<_>>();
        let mut p = Progetto::da("p", &base()).unwrap();
        p.lavori.clear();
        assert_eq!(
            ids(p.da_affidare_in(&pi, f)),
            ["1.1", "1.3"],
            "delle due misure pronte ne parte una"
        );
        let lavoro = |stato| Lavoro {
            dot: "dati".into(),
            compito: 1,
            stato,
            esito: String::new(),
        };
        assert!(!p.scheda_aperta(&pi));
        p.lavori.insert("1.1".into(), lavoro(None));
        assert!(p.scheda_aperta(&pi), "D426: la scheda e' di una misura");
        assert_eq!(
            ids(p.da_affidare_in(&pi, f)),
            ["1.3"],
            "con una misura aperta le altre aspettano"
        );
        assert_eq!(ids(p.pronti_in(f)), ["1.2", "1.3"], "ma restano pronte");
        p.lavori.insert("1.1".into(), lavoro(Some(Stato::Fatto)));
        assert_eq!(
            ids(p.da_affidare_in(&pi, f)),
            ["1.2", "1.3"],
            "chiusa la prima, parte la seconda, e la ripetizione aspetta il suo turno"
        );
        // Chi ha il compito lo sa.
        let t = testo_del_compito(&p, &pi, f, &f.incarichi[1]);
        assert!(t.contains("Questo compito usa la scheda video"), "{t}");
        assert!(!testo_del_compito(&p, &pi, f, &f.incarichi[2]).contains("scheda video"));
    }

    #[test]
    fn un_compito_pronto_va_a_chi_e_libero() {
        // D422: il 14 ottobre nove compiti di S0ex1 sono andati uno dopo
        // l'altro, e due di «ricerca» si sono messi in fila.
        let mut p = Progetto::da("s0ex1", &base()).unwrap();
        p.lavori.clear();
        p.squadra.clear();
        let reparto = Chi::Reparto("ricerca".into());
        let nessuno = BTreeSet::new();
        assert_eq!(
            p.chi_e_libero(&reparto, &nessuno),
            Some(ChiLibero::Lui("ricerca".into()))
        );
        let mut occ: BTreeSet<String> = ["ricerca".to_string()].into();
        assert_eq!(
            p.chi_e_libero(&reparto, &occ),
            Some(ChiLibero::DaTrovare {
                ruolo: "ricerca (aiuto 1)".into(),
                di: "ricerca".into()
            })
        );
        p.squadra
            .insert("ricerca (aiuto 1)".into(), "ricercatore-2".into());
        assert_eq!(
            p.chi_e_libero(&reparto, &occ),
            Some(ChiLibero::Aiuto("ricercatore-2".into()))
        );
        occ.insert("ricercatore-2".into());
        assert_eq!(
            p.chi_e_libero(&reparto, &occ),
            Some(ChiLibero::DaTrovare {
                ruolo: "ricerca (aiuto 2)".into(),
                di: "ricerca".into()
            })
        );
        p.squadra
            .insert("ricerca (aiuto 2)".into(), "ricercatore-3".into());
        occ.insert("ricercatore-3".into());
        assert_eq!(
            p.chi_e_libero(&reparto, &occ),
            Some(ChiLibero::InCoda("ricerca".into())),
            "al massimo due aiuti"
        );
        // Un ruolo da assumere senza nessuno non ha chi lo fa.
        let assumi = Chi::Assumi {
            ruolo: "sviluppatore".into(),
            mestiere: Mestiere::Generico,
        };
        assert_eq!(p.chi_e_libero(&assumi, &nessuno), None);
        p.squadra.insert("sviluppatore".into(), "sviluppo".into());
        assert_eq!(
            p.chi_e_libero(&assumi, &nessuno),
            Some(ChiLibero::Lui("sviluppo".into()))
        );
        // Chi ha un compito aperto e' occupato.
        p.lavori.insert(
            "1.1".into(),
            Lavoro {
                dot: "sviluppo".into(),
                compito: 3,
                stato: None,
                esito: String::new(),
            },
        );
        p.lavori.insert(
            "1.2".into(),
            Lavoro {
                dot: "dati".into(),
                compito: 4,
                stato: Some(Stato::Fatto),
                esito: String::new(),
            },
        );
        assert_eq!(p.occupati(), ["sviluppo".to_string()].into());
    }

    #[test]
    fn dal_testo_del_compito_si_sa_di_che_progetto_e() {
        let p = Progetto::da("s0ex1", &base()).unwrap();
        let pi = piano();
        let mut i = pi.fasi[0].incarichi[1].clone();
        // Un «. Compito … del piano:» dentro il testo del compito non inganna.
        i.cosa = format!("{}. Compito 9.9 del piano: no", i.cosa);
        let testo = testo_del_compito(&p, &pi, &pi.fasi[0], &i);
        assert_eq!(progetto_dal_testo(&testo), Some(("s0ex1".into(), i.id.clone())), "{testo}");
        assert_eq!(progetto_dal_testo("Compito qualunque"), None);
        assert_eq!(progetto_dal_testo("Progetto «»: niente"), None);
        // Una riga del diario che non cambia niente si rilegge.
        let mut e = base();
        let prima = Progetto::da("s0ex1", &e).unwrap();
        e.push(Evento::Domanda {
            quando: t(), da: "dati".into(), a: "legale".into(), incarico: "1.25".into(),
            domanda: "si puo'?".into(), risposta: "ESITO: ok".into(),
        });
        let riga = serde_json::to_string(e.last().unwrap()).unwrap();
        assert!(riga.starts_with("{\"tipo\":\"domanda\""), "{riga}");
        assert_eq!(eventi(&riga), vec![e.last().unwrap().clone()]);
        assert_eq!(Progetto::da("s0ex1", &e).unwrap(), prima);
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
                da_fase: None,
                cambio: false,
                di_nuovo: false,
            },
            Evento::PianoPronto {
                quando: t(),
                versione: 2,
                da_fase: Some(2),
                tenuti: vec!["2.1".into()],
            },
        ]);
        let p = Progetto::da("p", &e).unwrap();
        assert_eq!(p.versione, 2);
        assert_eq!(p.piano_chiesto, None);
        assert_eq!(
            p.lavori.keys().collect::<Vec<_>>(),
            ["1.1", "1.2"],
            "2.1 era tenuto, ma non era fatto: si rifa'"
        );
        assert_eq!(p.fasi.get(&1), Some(&true));
        assert_eq!(p.fasi.get(&2), None);
    }

    #[test]
    fn un_cambio_in_corso_tiene_i_compiti_uguali_e_prima_del_via_riprepara() {
        let mut e = base();
        e.extend([
            Evento::Via { quando: t(), tetto: 0.0 },
            affidato("1.1", "ricerca", 4),
            chiuso("1.1", Stato::Fatto),
            affidato("1.2", "revisione", 5),
            Evento::Cambio { quando: t(), cosa: "niente prototipo".into() },
        ]);
        let p = Progetto::da("p", &e).unwrap();
        assert_eq!((p.dove, p.cambio.as_deref()), (DoveSta::InCorso, Some("niente prototipo")));
        let mut due = e.clone();
        due.push(Evento::Cambio { quando: t(), cosa: "e niente Huffman".into() });
        assert_eq!(
            Progetto::da("p", &due).unwrap().cambio.as_deref(),
            Some("niente prototipo\ne niente Huffman"),
            "due cambi insieme: l'Architetto li vede tutti e due"
        );
        e.extend([
            chiuso("1.2", Stato::Fatto),
            Evento::PianoChiesto { quando: t(), compito: 8, fase: None, da_fase: Some(1), cambio: true, di_nuovo: false },
        ]);
        let p = Progetto::da("p", &e).unwrap();
        assert_eq!((p.cambio.as_deref(), p.piano_chiesto, p.piano_per_cambio), (None, Some((8, Some(1))), true));
        e.push(Evento::PianoPronto { quando: t(), versione: 2, da_fase: Some(1), tenuti: vec!["1.1".into()] });
        let p = Progetto::da("p", &e).unwrap();
        assert_eq!(p.lavori.keys().collect::<Vec<_>>(), ["1.1"], "1.1 e' uguale e fatto: resta");

        let mut prima = base();
        prima.extend([
            Evento::Legale { quando: t(), fase: None, rilascio: false, ok: true, note: vec![] },
            Evento::Proposto { quando: t(), tetto: 1.0 },
            Evento::Cambio { quando: t(), cosa: "piu' corto".into() },
        ]);
        let p = Progetto::da("p", &prima).unwrap();
        assert_eq!((p.dove, p.proposto, p.legale_inizio), (DoveSta::Prepara, false, None));
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

    /// D431: il 17 ottobre l'Architetto scriveva in prosa che 2.1, 2.2, 2.7
    /// e 2.9 di S0ex1 erano gia' fatti; il piano nuovo dopo una fase non
    /// andata li ha fatti rifare, e la consegna di 2.1 e' stata sovrascritta.
    #[test]
    fn un_piano_nuovo_tiene_i_fatti_segnati() {
        let mut e = base();
        e.extend([
            Evento::Via {
                quando: t(),
                tetto: 2.0,
            },
            affidato("1.1", "ricerca", 4),
            chiuso("1.1", Stato::Fatto),
            affidato("1.2", "revisione", 5),
            chiuso("1.2", Stato::Fatto),
            affidato("2.2", "programmatore", 6),
            chiuso("2.2", Stato::Fallito),
        ]);
        let p = Progetto::da("p", &e).unwrap();
        let nuovo = piano::leggi(
            &PIANO
                .replace(
                    "- 1.1 [ricerca] cercare\n",
                    "- 1.1 [ricerca] cercare meglio (fatto)\n",
                )
                .replace(
                    "- 2.2 [assumi: programmatore Rust] documentare\n",
                    "- 2.2 [assumi: programmatore Rust] documentare (fatto)\n",
                )
                .replace(
                    "Obiettivo: o\n",
                    "Obiettivo: o\n## Cosa cambia\n- 1.1 e' fatto\n",
                ),
            2,
        )
        .unwrap()
        .0;
        assert_eq!(
            segnati_fatti(&p, &nuovo, 1),
            ["1.1"],
            "1.1 e' segnato e fatto, anche col testo cambiato; 1.2 non e' segnato; 2.2 e' segnato \
             ma non e' fatto"
        );
        assert!(
            segnati_fatti(&p, &nuovo, 2).is_empty(),
            "le fasi prima non si contano"
        );
        e.push(Evento::PianoPronto {
            quando: t(),
            versione: 2,
            da_fase: Some(1),
            tenuti: vec!["1.1".into()],
        });
        let dopo = Progetto::da("p", &e).unwrap();
        assert_eq!(dopo.lavori.keys().collect::<Vec<_>>(), ["1.1"]);
        assert_eq!(
            dopo.pronti_in(&nuovo.fasi[0])
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            ["1.2"],
            "1.1 non si riaffida, 1.2 si'"
        );
    }

    #[test]
    fn un_piano_nuovo_tiene_i_fatti_uguali_e_dice_quanto_costa_il_resto() {
        let mut e = base();
        e.extend([
            Evento::Via { quando: t(), tetto: 2.0 },
            affidato("1.1", "ricerca", 4),
            chiuso("1.1", Stato::Fatto),
            affidato("1.2", "revisione", 5),
            chiuso("1.2", Stato::Fatto),
            Evento::Spesa { quando: t(), usd: 1.5, per: "x".into() },
        ]);
        let p = Progetto::da("p", &e).unwrap();
        let nuovo_testo = PIANO.replace("- 1.2 [revisione] rivedere (dopo 1.1)", "- 1.2 [revisione] rivedere bene (dopo 1.1)")
            .replace("Obiettivo: o\n", "Obiettivo: o\n## Cosa cambia\n- 1.2 piu' severa\n");
        let nuovo = piano::leggi(&nuovo_testo, 2).unwrap().0;
        let t = tenuti(&p, &piano(), &nuovo, 1);
        assert_eq!(t, ["1.1"], "1.2 e' cambiato: si rifa'");
        assert!(
            segnati_fatti(&p, &nuovo, 1).is_empty(),
            "nessuno e' segnato"
        );
        assert_eq!(
            costo_del_resto(&nuovo, 1, &t),
            1.2,
            "quattro compiti da rifare"
        );
        let m = dopo_il_cambio(&p, &nuovo, "/x/piano-2.md", 1.2);
        assert!(m.starts_with("Il piano di «p» e' cambiato, versione 2. E' in /x/piano-2.md.\nCosa cambia:\n- 1.2 piu' severa"), "{m}");
        assert!(m.ends_with("si passa il tetto di 2.00 $: mi fermo, e aspetto il via con un tetto nuovo."), "{m}");
        assert!(dopo_il_cambio(&p, &nuovo, "f", 0.3).ends_with("\nRiparto."));
        assert!(!m.contains("domande"), "senza domande non se ne parla: {m}");
        // D410: le domande del piano nuovo arrivano in chat anche dopo un
        // cambio, non solo con la proposta.
        let con_domande = piano::leggi(&format!("{nuovo_testo}## Domande per te\n- chi paga il piano business?\n"), 2)
            .unwrap()
            .0;
        let m = dopo_il_cambio(&p, &con_domande, "f", 0.3);
        assert!(
            m.contains("\n- 1.2 piu' severa\nLe domande dell'Architetto:\n- chi paga il piano business?\nRiparto."),
            "{m}"
        );
        assert!(serve_un_tetto_nuovo(&p, 0.51) && !serve_un_tetto_nuovo(&p, 0.5), "1.5 gia' spesi, tetto 2");
        let senza = Progetto { tetto: 0.0, ..p.clone() };
        assert!(!serve_un_tetto_nuovo(&senza, 100.0), "senza tetto non si chiede");
        assert!(dopo_il_cambio(&senza, &nuovo, "f", 100.0).ends_with("\nRiparto."));
        assert_eq!(testo_del_cambio(" niente Huffman ", Some(1)), "niente Huffman");
        assert_eq!(testo_del_cambio("niente Huffman", None), "niente Huffman");
        assert_eq!(
            testo_del_cambio("niente Huffman", Some(2)),
            "niente Huffman\nIl lavoro e' alla fase 2: le fasi prima restano come sono, si cambia da li'."
        );
        assert_eq!(frase_di_fine("spazi"), "Il progetto spazi e' finito: il resoconto e' in chat.");
    }

    #[test]
    fn le_righe_per_nova_sono_corte_e_dicono_cosa_fare() {
        let con = per_nova(true);
        assert!(con.starts_with("## Progetti dei Dot\n") && con.contains("cervelli_delega con dot: true"), "{con}");
        assert!(per_nova(false).contains("di' che serve Claude"));
        assert!(con.chars().count() < 220, "{} caratteri: le legge anche il modello di casa", con.chars().count());
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
    fn un_piano_non_arrivato_si_richiede_quando_il_progetto_riparte() {
        // D411: S0ex1 si e' fermato perche' il piano di revisione non si e'
        // fatto (mancava il giudice); rimesso il giudice, col via si
        // fermava di nuovo per lo stesso motivo.
        let q = String::from("2026-10-13T10:00:00");
        let mut e = vec![
            Evento::Creato { quando: q.clone(), numero: 1, richiesta: "x".into(), cartella: String::new() },
            Evento::Via { quando: q.clone(), tetto: 0.0 },
            Evento::PianoChiesto { quando: q.clone(), compito: 15, fase: Some(1), da_fase: None, cambio: false, di_nuovo: false },
            Evento::Fermo { quando: q.clone(), perche: "il piano non si e' fatto".into() },
        ];
        assert!(!Progetto::da("p", &e).unwrap().piano_da_richiedere, "fermo: si aspetta il via");
        e.push(Evento::Ripreso { quando: q.clone(), tetto: 0.0 });
        let p = Progetto::da("p", &e).unwrap();
        assert!(p.piano_da_richiedere && p.piano_chiesto == Some((15, Some(1))), "{p:?}");
        e.push(Evento::PianoChiesto { quando: q.clone(), compito: 16, fase: Some(1), da_fase: None, cambio: false, di_nuovo: false });
        let p = Progetto::da("p", &e).unwrap();
        assert!(!p.piano_da_richiedere && p.piano_chiesto == Some((16, Some(1))), "richiesto: una volta sola");
        // Rifatta per un cambio, la domanda di prima non porta via un cambio
        // arrivato nel frattempo: quello e' ancora da chiedere.
        let mut col_cambio = e.clone();
        col_cambio.push(Evento::Cambio { quando: q.clone(), cosa: "Claude resta".into() });
        col_cambio.push(Evento::PianoChiesto { quando: q.clone(), compito: 17, fase: None, da_fase: Some(1), cambio: true, di_nuovo: true });
        assert_eq!(Progetto::da("p", &col_cambio).unwrap().cambio.as_deref(), Some("Claude resta"));
        e.push(Evento::PianoPronto { quando: q.clone(), versione: 2, da_fase: Some(1), tenuti: vec![] });
        assert!(!Progetto::da("p", &e).unwrap().piano_da_richiedere);
        // Ripartito senza un piano in attesa: niente da richiedere.
        e.push(Evento::Fermo { quando: q.clone(), perche: "fermato dall'utente".into() });
        e.push(Evento::Ripreso { quando: q, tetto: 0.0 });
        assert!(!Progetto::da("p", &e).unwrap().piano_da_richiedere);
    }

    #[test]
    fn un_paper_non_fatto_si_riaffida_quando_il_progetto_riparte() {
        let q = String::from("2026-10-12T10:00:00");
        let paper = crate::struttura::INCARICO_PAPER.to_string();
        let mut e = vec![
            Evento::Creato { quando: q.clone(), numero: 1, richiesta: "x".into(), cartella: String::new() },
            Evento::Via { quando: q.clone(), tetto: 0.0 },
            Evento::Affidato { quando: q.clone(), incarico: paper.clone(), dot: "scrittura".into(), compito: 9 },
            Evento::Chiuso { quando: q.clone(), incarico: paper.clone(), stato: Stato::Fallito, esito: "no".into() },
            Evento::Fermo { quando: q.clone(), perche: "il paper non si e' fatto".into() },
        ];
        assert!(Progetto::da("p", &e).unwrap().lavori.contains_key(&paper));
        e.push(Evento::Ripreso { quando: q.clone(), tetto: 0.0 });
        assert!(!Progetto::da("p", &e).unwrap().lavori.contains_key(&paper), "si riaffida");
        // Fatto, resta.
        e[3] = Evento::Chiuso { quando: q, incarico: paper.clone(), stato: Stato::Fatto, esito: "ok".into() };
        assert!(Progetto::da("p", &e).unwrap().lavori.contains_key(&paper));
    }

    #[test]
    fn i_progetti_si_elencano_dall_indice_e_i_vecchi_dalla_cartella() {
        let d = std::env::temp_dir().join(format!("nova-progetti-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        assert!(elenco(&d).is_empty() && vecchi(&d).is_empty());
        for n in ["b", "a", "vuoto"] {
            let r = d.join("Dots_Project").join(n);
            std::fs::create_dir_all(&r).unwrap();
            if n != "vuoto" {
                std::fs::write(r.join("progetto.jsonl"), "").unwrap();
            }
            crate::struttura::registra(&d, n, &r).unwrap();
        }
        assert_eq!(elenco(&d), ["a", "b"], "senza cronologia non e' un progetto");
        std::fs::create_dir_all(cartella_vecchia(&d, "vecchio")).unwrap();
        std::fs::write(cartella_vecchia(&d, "vecchio").join("progetto.jsonl"), "").unwrap();
        std::fs::create_dir_all(cartella_vecchia(&d, "niente")).unwrap();
        assert_eq!(vecchi(&d), ["vecchio"]);
        let _ = std::fs::remove_dir_all(&d);
    }
}
