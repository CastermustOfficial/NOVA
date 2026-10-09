//! I Dot di NOVA: chi sono, la loro coda, la conversazione che resta (D382).
//!
//! Un Dot e' un collega con un nome che porta a termine un compito da solo
//! (D381, `docs/dots.md`). Qui c'e' la parte che non fa turni: com'e' fatta
//! la sua cartella, come si legge la coda dei compiti, cosa si fa dopo un
//! riavvio. I turni li fa `nova_core::dot`, con gli strumenti del demone.
//!
//! La cartella di un Dot:
//!
//! ```text
//! <cartella di NOVA>/dots/<nome>/
//!   dot.json            chi e': nome, ruolo, quando e' nato
//!   compiti.jsonl       la coda, come diario di passaggi di stato
//!   conversazione.json  i messaggi, per riprendere dopo un riavvio
//!   diario.jsonl        cosa ha fatto, passo per passo
//!   vault/              la sua memoria, nello stesso formato di quella di NOVA
//!   rapporti/           quello che consegna: il ricercatore, un .md per compito
//!   file.jsonl          i file che ha toccato, per l'harness (D391, [`vista`])
//!   inviati.jsonl       i messaggi che ha mandato (D391)
//! ```
//!
//! Il ricercatore (D383) ha le sue regole in [`ricerca`]: il piano, i passi,
//! il revisore, le fonti controllate.
//!
//! **La coda e' un diario, non una tabella.** Ogni riga di `compiti.jsonl`
//! dice che un compito e' passato a uno stato. Lo stato di adesso si ottiene
//! rileggendo le righe in ordine ([`compiti`]): una riga scritta a meta' da
//! un demone che si spegne si salta, e quelle prima restano vere. Riscrivere
//! una tabella intera a ogni passaggio vorrebbe dire poterla perdere tutta.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod consegna;
pub mod custode;
pub mod gruppi;
pub mod ricerca;
pub mod vista;

/// I nomi che un Dot non puo' avere: la cartella dei gruppi e Nova.
pub const NOMI_PRESI: [&str; 2] = [gruppi::CARTELLA, DA_NOVA];

/// Chi scrive, quando scrive Nova; e a chi scrive un Dot che vuole Nova.
pub const DA_NOVA: &str = consegna::DA_NOVA;

/// Quante volte un compito puo' andare in attesa (D388). Un capo che a ogni
/// ripresa affida un pezzo nuovo non finirebbe mai: dopo l'ultima attesa il
/// compito si chiude con quello che ha, e lo dice.
pub const ATTESE_MASSIME: u32 = 5;

/// Quanto della posta entra nella domanda di un compito. Il resto si dice,
/// e resta da leggere al compito dopo.
pub const POSTA_IN_DOMANDA: usize = 8_000;

/// Quanto puo' essere lungo il nome di un Dot.
pub const NOME_MASSIMO: usize = 32;

/// Quante volte un compito interrotto da un riavvio si riprende. Un compito
/// che fa cadere il demone ogni volta non deve farlo cadere per sempre.
pub const RIPRESE_MASSIME: u32 = 2;

/// Quanti turni puo' fare un compito. Un turno finisce quando il modello
/// risponde o quando ha usato tutti i suoi passi; nel secondo caso si va
/// avanti, fino a qui.
pub const TURNI_PER_COMPITO: usize = 6;

/// Quanti messaggi tiene la conversazione salvata. Il turno taglia gia' quel
/// che manda al modello (D361); questo e' il tetto del file.
pub const MESSAGGI_MASSIMI: usize = 400;

/// Chi e' un Dot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dot {
    pub nome: String,
    /// Il ruolo, scritto da chi lo crea: e' la parte del prompt che lo fa
    /// essere lui.
    pub ruolo: String,
    /// Quando e' nato, come lo scrive il demone.
    pub nato: String,
    /// Come lavora. Un Dot nato prima del D383 non ce l'ha scritto, ed e'
    /// generico: lavorava cosi'.
    #[serde(default)]
    pub mestiere: Mestiere,
    /// Il suo capo, se ne ha uno (D388): un altro Dot, che gli puo' affidare
    /// pezzi del suo lavoro. Vuoto: risponde a Nova e all'utente.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub capo: String,
}

/// Come lavora un Dot.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mestiere {
    /// Un turno dopo l'altro nella sua conversazione, finche' risponde (D382).
    #[default]
    Generico,
    /// Il piano col cervello piu' grande, i passi col cervello assegnato, il
    /// revisore che fa salire, il rapporto con le fonti (D383).
    Ricercatore,
    /// Decide i permessi degli altri Dot, sempre e solo quello (D384). Lo fa
    /// nascere NOVA, ed e' uno solo: [`custode::NOME_CUSTODE`].
    Custode,
}

impl Mestiere {
    /// Il mestiere da come lo scrive chi crea un Dot. Vuoto vuol dire
    /// generico; una parola che non e' un mestiere si rifiuta, invece di far
    /// nascere un Dot che lavora in un altro modo da quello chiesto.
    pub fn da(testo: &str) -> Result<Mestiere, String> {
        match testo.trim() {
            "" | "generico" => Ok(Mestiere::Generico),
            "ricercatore" => Ok(Mestiere::Ricercatore),
            "custode" => Err(format!(
                "il custode dei permessi lo fa nascere NOVA, ed e' uno solo: «{}»",
                custode::NOME_CUSTODE
            )),
            altro => Err(format!(
                "«{altro}» non e' un mestiere: i mestieri sono «generico» e «ricercatore»"
            )),
        }
    }
}

/// A che punto e' un compito.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stato {
    /// In coda.
    Affidato,
    /// Lo sta facendo.
    InCorso,
    /// Aspetta che i sottoposti consegnino i pezzi che ha affidato loro
    /// (D388). Intanto il Dot fa gli altri compiti; quando tutti hanno
    /// consegnato, riprende.
    InAttesa,
    /// Finito, con un esito.
    Fatto,
    /// Finito male: l'esito dice perche'.
    Fallito,
    /// Fermato da qualcuno.
    Fermato,
    /// Interrotto da troppi riavvii.
    Interrotto,
}

impl Stato {
    /// Uno stato da cui non si torna.
    pub fn chiuso(self) -> bool {
        matches!(self, Stato::Fatto | Stato::Fallito | Stato::Fermato | Stato::Interrotto)
    }
}

/// Una riga di `compiti.jsonl`: un compito passa a uno stato.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evento {
    pub id: u64,
    pub stato: Stato,
    pub quando: String,
    /// Il compito, nella riga con cui entra in coda.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub testo: String,
    /// Chi l'ha affidato: `utente`, `nova`, un orario.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub da: String,
    /// Com'e' andata, o perche' si riprende.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub esito: String,
    /// Nella riga con cui entra in coda: il compito del capo di cui questo e'
    /// un pezzo (D388). Quando si chiude, l'esito torna li'.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub padre: Option<Rif>,
}

/// Un compito di un Dot: chi e quale.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rif {
    pub dot: String,
    pub id: u64,
}

/// Una riga di `compiti.jsonl` che non cambia lo stato di un compito ma ne
/// segue la squadra (D388): un pezzo affidato a un sottoposto, o il pezzo
/// consegnato. Non ha `stato`, e cosi' non si scambia per un passaggio.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Nota {
    pub id: u64,
    pub quando: String,
    pub nota: Squadra,
}

/// Cosa dice una [`Nota`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum Squadra {
    /// Il Dot ha affidato un pezzo del compito a un sottoposto.
    Affidato { dot: String, compito: u64 },
    /// Il sottoposto ha chiuso il pezzo.
    Consegnato {
        dot: String,
        compito: u64,
        stato: Stato,
        esito: String,
    },
}

/// Un pezzo di lavoro affidato a un sottoposto, com'e' adesso.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Sottocompito {
    pub dot: String,
    pub id: u64,
    pub stato: Stato,
    pub esito: String,
    /// Il capo ha gia' avuto l'esito, riprendendo il compito.
    pub letto: bool,
}

/// Un compito, com'e' adesso.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Compito {
    pub id: u64,
    pub testo: String,
    pub da: String,
    pub stato: Stato,
    pub affidato: String,
    pub iniziato: String,
    pub finito: String,
    pub esito: String,
    /// Quante volte e' stato ripreso dopo un riavvio.
    pub riprese: u32,
    /// Se e' un pezzo del compito di un capo, quale.
    pub padre: Option<Rif>,
    /// I pezzi affidati ai sottoposti, e com'e' andata.
    pub attende: Vec<Sottocompito>,
    /// Quante volte e' andato in attesa.
    pub attese: u32,
}

/// Il nome di un Dot, se va bene: minuscole, cifre e trattini, non vuoto,
/// che comincia con una lettera o una cifra. Diventa il nome di una
/// cartella, quindi niente punti, barre o spazi.
pub fn nome_valido(nome: &str) -> Result<String, String> {
    let n = nome.trim();
    if n.is_empty() {
        return Err("un Dot ha bisogno di un nome".into());
    }
    if n.chars().count() > NOME_MASSIMO {
        return Err(format!("il nome di un Dot sta in {NOME_MASSIMO} caratteri"));
    }
    if !n.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        return Err(format!(
            "«{n}»: il nome di un Dot e' fatto di lettere minuscole, cifre e trattini"
        ));
    }
    if n.starts_with('-') {
        return Err(format!("«{n}»: il nome di un Dot comincia con una lettera o una cifra"));
    }
    // Due nomi presi (D388): `dots/gruppi/` e' la cartella dei gruppi, e
    // «nova» e' a chi un Dot scrive per parlare con Nova.
    if NOMI_PRESI.contains(&n) {
        return Err(format!("«{n}» non puo' essere il nome di un Dot: e' gia' preso"));
    }
    Ok(n.to_string())
}

/// La coda di adesso, dalle righe di `compiti.jsonl`.
///
/// Una riga che non si legge si salta. Un passaggio per un compito che non e'
/// mai entrato in coda si salta. Un compito chiuso resta chiuso: una riga
/// dopo la chiusura non lo riapre.
pub fn compiti(righe: &str) -> Vec<Compito> {
    let mut fuori: Vec<Compito> = Vec::new();
    for riga in righe.lines() {
        let Ok(e) = serde_json::from_str::<Evento>(riga) else {
            if let Ok(n) = serde_json::from_str::<Nota>(riga) {
                if let Some(c) = fuori.iter_mut().find(|c| c.id == n.id) {
                    annota_la_squadra(c, n.nota);
                }
            }
            continue;
        };
        match fuori.iter_mut().find(|c| c.id == e.id) {
            None => {
                if e.stato == Stato::Affidato {
                    fuori.push(Compito {
                        id: e.id,
                        testo: e.testo,
                        da: e.da,
                        stato: Stato::Affidato,
                        affidato: e.quando,
                        iniziato: String::new(),
                        finito: String::new(),
                        esito: String::new(),
                        riprese: 0,
                        padre: e.padre,
                        attende: Vec::new(),
                        attese: 0,
                    });
                }
            }
            Some(c) => {
                if c.stato.chiuso() {
                    continue;
                }
                match e.stato {
                    Stato::Affidato => {
                        // Rimesso in coda: succede solo a un compito che era
                        // in corso quando il demone si e' spento.
                        if c.stato == Stato::InCorso {
                            c.riprese += 1;
                            c.stato = Stato::Affidato;
                        }
                    }
                    Stato::InCorso => {
                        // Ripreso dopo l'attesa: gli esiti consegnati fin qui
                        // il Dot li riceve adesso.
                        if c.stato == Stato::InAttesa {
                            for s in c.attende.iter_mut().filter(|s| s.stato.chiuso()) {
                                s.letto = true;
                            }
                        }
                        c.stato = Stato::InCorso;
                        if c.iniziato.is_empty() {
                            c.iniziato = e.quando;
                        }
                    }
                    Stato::InAttesa => {
                        c.stato = Stato::InAttesa;
                        c.attese += 1;
                    }
                    chiuso => {
                        c.stato = chiuso;
                        c.finito = e.quando;
                        c.esito = e.esito;
                    }
                }
            }
        }
    }
    fuori
}

/// Una nota sulla squadra di un compito.
fn annota_la_squadra(c: &mut Compito, n: Squadra) {
    match n {
        Squadra::Affidato { dot, compito } => {
            if !c.attende.iter().any(|s| s.dot == dot && s.id == compito) {
                c.attende.push(Sottocompito {
                    dot,
                    id: compito,
                    stato: Stato::Affidato,
                    esito: String::new(),
                    letto: false,
                });
            }
        }
        Squadra::Consegnato { dot, compito, stato, esito } => {
            if let Some(s) = c.attende.iter_mut().find(|s| s.dot == dot && s.id == compito) {
                if !s.stato.chiuso() {
                    s.stato = stato;
                    s.esito = esito;
                }
            }
        }
    }
}

/// Un compito in attesa i cui sottoposti hanno consegnato tutti: si
/// riprende.
pub fn pronto(c: &Compito) -> bool {
    c.stato == Stato::InAttesa && c.attende.iter().all(|s| s.stato.chiuso())
}

/// Se, finiti i turni, il compito va in attesa invece di chiudersi: c'e' un
/// pezzo affidato di cui il Dot non ha ancora avuto l'esito, e non ha gia'
/// aspettato [`ATTESE_MASSIME`] volte.
pub fn da_aspettare(c: &Compito) -> bool {
    c.attese < ATTESE_MASSIME && c.attende.iter().any(|s| !s.letto)
}

/// Cosa si aggiunge all'esito di un compito che si chiude con dei pezzi
/// ancora aperti, perche' ha gia' aspettato troppe volte.
pub fn senza_aspettare_oltre(c: &Compito) -> Option<String> {
    let aperti: Vec<String> = c
        .attende
        .iter()
        .filter(|s| !s.letto)
        .map(|s| format!("{} n. {}", s.dot, s.id))
        .collect();
    (c.attese >= ATTESE_MASSIME && !aperti.is_empty()).then(|| {
        format!(
            "(Chiuso dopo {ATTESE_MASSIME} attese: i pezzi ancora aperti, {}, non tornano qui.)",
            aperti.join(", ")
        )
    })
}

/// Il prossimo compito da fare: prima uno in attesa che si puo' riprendere,
/// perche' si finisce quel che si e' cominciato; se no il piu' vecchio in
/// coda.
pub fn prossimo(coda: &[Compito]) -> Option<&Compito> {
    coda.iter()
        .filter(|c| pronto(c))
        .min_by_key(|c| c.id)
        .or_else(|| coda.iter().filter(|c| c.stato == Stato::Affidato).min_by_key(|c| c.id))
}

/// Il numero del prossimo compito.
pub fn prossimo_id(coda: &[Compito]) -> u64 {
    coda.iter().map(|c| c.id).max().unwrap_or(0) + 1
}

/// Cosa scrivere all'accensione: i compiti rimasti in corso tornano in coda,
/// o si chiudono come interrotti se sono gia' stati ripresi troppe volte.
pub fn alla_ripartenza(coda: &[Compito], quando: &str) -> Vec<Evento> {
    coda.iter()
        .filter(|c| c.stato == Stato::InCorso)
        .map(|c| {
            if c.riprese < RIPRESE_MASSIME {
                Evento {
                    id: c.id,
                    stato: Stato::Affidato,
                    quando: quando.to_string(),
                    testo: String::new(),
                    da: String::new(),
                    esito: "ripreso dopo un riavvio".into(),
                    padre: None,
                }
            } else {
                Evento {
                    id: c.id,
                    stato: Stato::Interrotto,
                    quando: quando.to_string(),
                    testo: String::new(),
                    da: String::new(),
                    esito: format!(
                        "interrotto: il demone si e' spento {} volte mentre lo faceva",
                        c.riprese + 1
                    ),
                    padre: None,
                }
            }
        })
        .collect()
}

/// Il prompt di sistema di un Dot: quello di NOVA, e in coda chi e' lui.
///
/// Quello di NOVA resta davanti perche' ha le regole degli strumenti; in coda
/// si dice che in questa conversazione non e' Nova, e come lavora un Dot.
pub fn prompt(dot: &Dot, base: &str) -> String {
    let generico = format!(
        "{base}\n\n## Chi sei in questa conversazione\n\
         Non sei Nova: sei {nome}, un Dot di NOVA, un collega che porta a termine \
         un compito da solo. Il tuo ruolo: {ruolo}\n\
         Lavori mentre l'utente fa altro: non chiedere conferme e non fermarti a \
         meta' per chiedere come procedere, decidi tu. L'utente ti puo' scrivere \
         come un collega o come un direttore, e ti puo' fermare. Quando hai \
         finito, rispondi con il risultato del compito.",
        nome = dot.nome,
        ruolo = dot.ruolo.trim(),
    );
    match dot.mestiere {
        Mestiere::Generico => generico,
        Mestiere::Ricercatore => format!("{generico}\n{}", ricerca::PROMPT),
        Mestiere::Custode => format!("{generico}\n{}", custode::PROMPT),
    }
}

/// Il messaggio con cui comincia un compito.
pub fn domanda(c: &Compito) -> String {
    let da = if c.da.trim().is_empty() { "utente" } else { c.da.trim() };
    format!("Compito n. {} (affidato da {da}):\n{}", c.id, c.testo.trim())
}

/// Quello che si aggiunge alla prima domanda di un compito ripreso dopo un
/// riavvio: chi lo riprende deve saperlo.
pub const RIPRESO: &str =
    "\n\n(Ripreso dopo un riavvio: se l'avevi gia' cominciato, continua da dove eri.)";

/// Come si chiama uno stato, come lo scrive la coda.
pub fn nome_stato(s: Stato) -> &'static str {
    match s {
        Stato::Affidato => "affidato",
        Stato::InCorso => "in corso",
        Stato::InAttesa => "in attesa",
        Stato::Fatto => "fatto",
        Stato::Fallito => "fallito",
        Stato::Fermato => "fermato",
        Stato::Interrotto => "interrotto",
    }
}

/// La squadra di un Dot, in coda alla domanda di ogni compito (D388): a chi
/// risponde e a chi puo' affidare. Vuota se non ha ne' capo ne' sottoposti.
pub fn squadra(capo: &str, sottoposti: &[String]) -> String {
    let mut righe = Vec::new();
    if !capo.trim().is_empty() {
        righe.push(format!("Il tuo capo e' {}.", capo.trim()));
    }
    if !sottoposti.is_empty() {
        righe.push(format!(
            "I tuoi sottoposti: {}. A loro puoi affidare pezzi di questo compito con \
             dot.affida: il compito aspetta che consegnino, e riprende con i loro esiti. \
             Se serve, con loro puoi fare un gruppo (dot.gruppo).",
            sottoposti.join(", ")
        ));
    }
    if righe.is_empty() {
        String::new()
    } else {
        format!("\n\n({})", righe.join(" "))
    }
}

/// La domanda con cui riprende un compito in attesa: gli esiti dei
/// sottoposti che non ha ancora avuto.
pub fn ripresa(c: &Compito) -> String {
    let mut t = format!("Compito n. {}: i tuoi sottoposti hanno consegnato.\n", c.id);
    for s in c.attende.iter().filter(|s| !s.letto && s.stato.chiuso()) {
        t.push_str(&format!(
            "- {}, compito n. {} ({}): {}\n",
            s.dot,
            s.id,
            nome_stato(s.stato),
            s.esito.trim()
        ));
    }
    t.push_str(&format!(
        "Con questo, finisci il compito n. {}: {}",
        c.id,
        consegna::in_breve(&c.testo, consegna::COMPITO_IN_BREVE)
    ));
    t
}

/// Un messaggio nella posta di un Dot (D388).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Messaggio {
    /// Il numero, nella posta di chi lo riceve.
    pub n: u64,
    /// Chi scrive: un Dot, o `nova`.
    pub da: String,
    /// A chi era scritto: il Dot, o `gruppo:<nome>`.
    pub a: String,
    pub testo: String,
    pub quando: String,
}

/// La domanda di un compito con in coda la posta arrivata prima. I messaggi
/// si leggono cosi', al compito dopo: e' la scelta di Gio (D388). Oltre
/// [`POSTA_IN_DOMANDA`] caratteri ci si ferma, e lo si dice; torna anche
/// fino a che numero si e' letto.
pub fn con_la_posta(domanda: &str, posta: &[Messaggio]) -> (String, Option<u64>) {
    if posta.is_empty() {
        return (domanda.to_string(), None);
    }
    let mut t = format!("{domanda}\n\nMessaggi arrivati prima di questo compito:");
    let mut letto = None;
    let mut usati = 0usize;
    for m in posta {
        // A piu' Dot insieme (D392): si dice a chi, cosi' chi risponde sa a
        // chi scrivere per rispondere a tutti.
        let dove = match m.a.strip_prefix("gruppo:") {
            Some(g) => format!(" nel gruppo «{g}»"),
            None if m.a.contains(',') => format!(" a {} insieme", m.a.replace(',', ", ")),
            None => String::new(),
        };
        let riga = format!("\n- da {}{dove}: {}", m.da, m.testo.trim());
        if usati > 0 && usati + riga.chars().count() > POSTA_IN_DOMANDA {
            t.push_str("\n(Ci sono altri messaggi: li leggerai al prossimo compito.)");
            break;
        }
        usati += riga.chars().count();
        t.push_str(&riga);
        letto = Some(m.n);
    }
    (t, letto)
}

/// Il messaggio con cui si va avanti quando un turno ha finito i passi.
pub fn continua(c: &Compito) -> String {
    format!("Continua il compito n. {} da dove sei arrivato.", c.id)
}

/// La conversazione salvata.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Conversazione {
    /// Tutti i messaggi, quello di sistema compreso.
    pub messaggi: Vec<Value>,
    /// La sessione di Claude Code che tiene il filo, se c'e'.
    #[serde(default)]
    pub claude: String,
    /// Quante deleghe ha fatto: il conto non torna indietro (D218).
    #[serde(default)]
    pub deleghe: u32,
}

/// Taglia la conversazione al tetto, tenendo il messaggio di sistema e
/// tagliando davanti a una domanda dell'utente: tagliare in mezzo lascerebbe
/// la risposta di uno strumento senza la chiamata che l'ha chiesta.
pub fn pota(messaggi: &mut Vec<Value>, massimo: usize) {
    if messaggi.len() <= massimo || messaggi.is_empty() {
        return;
    }
    let ha_sistema = messaggi[0].get("role").and_then(Value::as_str) == Some("system");
    let primo = if ha_sistema { 1 } else { 0 };
    let da_togliere = messaggi.len() - massimo;
    let taglio = (primo + da_togliere..messaggi.len())
        .find(|&i| messaggi[i].get("role").and_then(Value::as_str) == Some("user"));
    if let Some(t) = taglio {
        messaggi.drain(primo..t);
    }
}

/// La cartella di un Dot.
#[derive(Debug, Clone)]
pub struct Cartella {
    pub radice: PathBuf,
}

pub(crate) fn scrivi_intero(p: &Path, testo: &str) -> Result<(), String> {
    let nuovo = p.with_extension("nuovo");
    std::fs::write(&nuovo, testo).map_err(|e| format!("{}: {e}", nuovo.display()))?;
    std::fs::rename(&nuovo, p).map_err(|e| format!("{}: {e}", p.display()))
}

/// Chi scrive nelle code e nei diari, uno alla volta. Il ciclo di un Dot e
/// chi gli affida un compito scrivono nello stesso file da due fili: senza
/// questo, su Windows due righe si sono mescolate e la coda ha perso dei
/// passaggi (`test_demone_dot.py`).
pub(crate) static SCRITTURA: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(crate) fn aggiungi_riga(p: &Path, riga: &str) -> Result<(), String> {
    let _turno = SCRITTURA.lock().unwrap_or_else(|e| e.into_inner());
    aggiungi_riga_gia_in_turno(p, riga)
}

/// La riga intera, a capo compreso, in una scrittura sola: `writeln!` su un
/// file ne fa due, e fra le due puo' passare un altro.
pub(crate) fn aggiungi_riga_gia_in_turno(p: &Path, riga: &str) -> Result<(), String> {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(p)
        .map_err(|e| format!("{}: {e}", p.display()))?;
    f.write_all(format!("{riga}\n").as_bytes())
        .map_err(|e| format!("{}: {e}", p.display()))
}

impl Cartella {
    /// La cartella del Dot con quel nome, sotto `base` (`dots/`).
    pub fn di(base: &Path, nome: &str) -> Result<Cartella, String> {
        Ok(Cartella {
            radice: base.join(nome_valido(nome)?),
        })
    }

    /// Se il Dot c'e'.
    pub fn esiste(&self) -> bool {
        self.radice.join("dot.json").is_file()
    }

    /// Fa nascere il Dot. Un Dot che c'e' gia' non si riscrive: avrebbe
    /// perso il ruolo che qualcuno gli aveva dato.
    pub fn crea(&self, dot: &Dot) -> Result<(), String> {
        if self.esiste() {
            return Err(format!("il Dot «{}» c'e' gia'", dot.nome));
        }
        std::fs::create_dir_all(&self.radice).map_err(|e| format!("{}: {e}", self.radice.display()))?;
        self.prepara()?;
        let testo = serde_json::to_string_pretty(dot).map_err(|e| e.to_string())? + "\n";
        scrivi_intero(&self.radice.join("dot.json"), &testo)
    }

    /// Le cartelle che un Dot si porta dietro: il vault e i rapporti. Si
    /// creano alla nascita, e all'accensione per i Dot nati prima che ci
    /// fossero (D382).
    pub fn prepara(&self) -> Result<(), String> {
        for p in [self.vault(), self.radice.join("rapporti")] {
            std::fs::create_dir_all(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        }
        Ok(())
    }

    /// Il suo vault: la sua memoria, nello stesso formato di quella di NOVA.
    pub fn vault(&self) -> PathBuf {
        self.radice.join("vault")
    }

    /// Dove sta il rapporto di un compito.
    pub fn rapporto(&self, id: u64) -> PathBuf {
        self.radice.join("rapporti").join(format!("{id}.md"))
    }

    /// Scrive il rapporto di un compito, tutto insieme: chi lo apre trova
    /// quello di prima o quello nuovo, mai mezzo.
    pub fn salva_rapporto(&self, id: u64, testo: &str) -> Result<PathBuf, String> {
        let p = self.rapporto(id);
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        scrivi_intero(&p, testo)?;
        Ok(p)
    }

    /// Chi e'.
    pub fn dot(&self) -> Result<Dot, String> {
        let p = self.radice.join("dot.json");
        let t = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        serde_json::from_str(t.trim_start_matches('\u{feff}')).map_err(|e| format!("{}: {e}", p.display()))
    }

    /// La coda di adesso.
    pub fn compiti(&self) -> Vec<Compito> {
        compiti(&std::fs::read_to_string(self.radice.join("compiti.jsonl")).unwrap_or_default())
    }

    /// Mette in coda un compito nuovo e ne torna il numero. Leggere la coda
    /// per il numero e scrivere la riga si fanno insieme: due compiti affidati
    /// nello stesso istante non prendono lo stesso numero.
    pub fn affida(&self, testo: &str, da: &str, quando: &str) -> Result<u64, String> {
        self.affida_per(testo, da, quando, None)
    }

    /// Come [`Cartella::affida`], per un pezzo del compito `padre` di un capo
    /// (D388): l'esito, quando il pezzo si chiude, tornera' li'.
    pub fn affida_per(
        &self,
        testo: &str,
        da: &str,
        quando: &str,
        padre: Option<Rif>,
    ) -> Result<u64, String> {
        let _turno = SCRITTURA.lock().unwrap_or_else(|e| e.into_inner());
        let id = prossimo_id(&self.compiti());
        let e = Evento {
            id,
            stato: Stato::Affidato,
            quando: quando.to_string(),
            testo: testo.to_string(),
            da: da.to_string(),
            esito: String::new(),
            padre,
        };
        let riga = serde_json::to_string(&e).map_err(|e| e.to_string())?;
        aggiungi_riga_gia_in_turno(&self.radice.join("compiti.jsonl"), &riga)?;
        Ok(id)
    }

    /// Scrive un passaggio di stato.
    pub fn annota(&self, e: &Evento) -> Result<(), String> {
        let riga = serde_json::to_string(e).map_err(|e| e.to_string())?;
        aggiungi_riga(&self.radice.join("compiti.jsonl"), &riga)
    }

    /// Scrive una nota sulla squadra di un compito (D388).
    pub fn annota_squadra(&self, n: &Nota) -> Result<(), String> {
        let riga = serde_json::to_string(n).map_err(|e| e.to_string())?;
        aggiungi_riga(&self.radice.join("compiti.jsonl"), &riga)
    }

    /// Mette un messaggio nella posta del Dot e ne torna il numero.
    pub fn imbuca(&self, da: &str, a: &str, testo: &str, quando: &str) -> Result<u64, String> {
        let _turno = SCRITTURA.lock().unwrap_or_else(|e| e.into_inner());
        let n = self.posta().iter().map(|m| m.n).max().unwrap_or(0) + 1;
        let m = Messaggio {
            n,
            da: da.to_string(),
            a: a.to_string(),
            testo: testo.to_string(),
            quando: quando.to_string(),
        };
        let riga = serde_json::to_string(&m).map_err(|e| e.to_string())?;
        aggiungi_riga_gia_in_turno(&self.radice.join("posta.jsonl"), &riga)?;
        Ok(n)
    }

    /// Tutta la posta, in ordine. Una riga che non si legge si salta.
    pub fn posta(&self) -> Vec<Messaggio> {
        std::fs::read_to_string(self.radice.join("posta.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|r| serde_json::from_str(r).ok())
            .collect()
    }

    /// Fino a che numero il Dot ha letto la posta.
    pub fn letta_fino_a(&self) -> u64 {
        std::fs::read_to_string(self.radice.join("posta_letta"))
            .ok()
            .and_then(|t| t.trim().parse().ok())
            .unwrap_or(0)
    }

    /// I messaggi che il Dot non ha ancora letto.
    pub fn non_letta(&self) -> Vec<Messaggio> {
        let fino = self.letta_fino_a();
        self.posta().into_iter().filter(|m| m.n > fino).collect()
    }

    /// Segna letta la posta fino a `n`. Non torna mai indietro.
    pub fn segna_letta(&self, n: u64) -> Result<(), String> {
        let _turno = SCRITTURA.lock().unwrap_or_else(|e| e.into_inner());
        if n <= self.letta_fino_a() {
            return Ok(());
        }
        scrivi_intero(&self.radice.join("posta_letta"), &format!("{n}\n"))
    }

    /// Una riga del diario: cosa ha fatto.
    pub fn diario(&self, riga: &Value) -> Result<(), String> {
        aggiungi_riga(&self.radice.join("diario.jsonl"), &riga.to_string())
    }

    /// La conversazione salvata, se c'e' e si legge.
    pub fn conversazione(&self) -> Option<Conversazione> {
        let t = std::fs::read_to_string(self.radice.join("conversazione.json")).ok()?;
        serde_json::from_str(&t).ok()
    }

    /// Salva la conversazione, tagliata al tetto, tutta insieme: chi si
    /// spegne a meta' lascia quella di prima, non mezza.
    pub fn salva_conversazione(&self, c: &Conversazione) -> Result<(), String> {
        let mut c = c.clone();
        pota(&mut c.messaggi, MESSAGGI_MASSIMI);
        let testo = serde_json::to_string(&c).map_err(|e| e.to_string())?;
        scrivi_intero(&self.radice.join("conversazione.json"), &testo)
    }
}

/// I nomi dei Dot che ci sono sotto `base`, in ordine.
pub fn elenco(base: &Path) -> Vec<String> {
    let Ok(voci) = std::fs::read_dir(base) else {
        return Vec::new();
    };
    let mut nomi: Vec<String> = voci
        .flatten()
        .filter(|v| v.path().join("dot.json").is_file())
        .filter_map(|v| v.file_name().to_str().map(str::to_string))
        .filter(|n| nome_valido(n).as_deref() == Ok(n.as_str()))
        .collect();
    nomi.sort();
    nomi
}

#[cfg(test)]
mod prove {
    use super::*;
    use serde_json::json;

    fn ev(id: u64, stato: Stato, quando: &str) -> String {
        serde_json::to_string(&Evento {
            id,
            stato,
            quando: quando.into(),
            testo: if stato == Stato::Affidato { format!("compito {id}") } else { String::new() },
            da: if stato == Stato::Affidato { "utente".into() } else { String::new() },
            esito: if stato.chiuso() { format!("esito {id}") } else { String::new() },
            padre: None,
        })
        .unwrap()
    }

    #[test]
    fn i_nomi_sono_nomi_di_cartella() {
        assert_eq!(nome_valido(" ricercatore ").unwrap(), "ricercatore");
        assert_eq!(nome_valido("dot-2").unwrap(), "dot-2");
        for brutto in ["", "Ricercatore", "a b", "../x", "a.b", "-a", "è"] {
            assert!(nome_valido(brutto).is_err(), "{brutto:?}");
        }
        assert!(nome_valido(&"a".repeat(NOME_MASSIMO + 1)).is_err());
        assert!(nome_valido(&"a".repeat(NOME_MASSIMO)).is_ok());
    }

    #[test]
    fn la_coda_si_rilegge_dal_diario() {
        let righe = [
            ev(1, Stato::Affidato, "t1"),
            ev(2, Stato::Affidato, "t2"),
            ev(1, Stato::InCorso, "t3"),
            "{mezza riga".to_string(),
            ev(1, Stato::Fatto, "t4"),
            ev(9, Stato::InCorso, "t5"),
        ]
        .join("\n");
        let c = compiti(&righe);
        assert_eq!(c.len(), 2, "il 9 non e' mai entrato in coda");
        assert_eq!(c[0].stato, Stato::Fatto);
        assert_eq!((c[0].iniziato.as_str(), c[0].finito.as_str()), ("t3", "t4"));
        assert_eq!(c[0].esito, "esito 1");
        assert_eq!(prossimo(&c).map(|c| c.id), Some(2));
        assert_eq!(prossimo_id(&c), 3);
        assert_eq!(prossimo_id(&[]), 1);
    }

    #[test]
    fn un_compito_chiuso_resta_chiuso() {
        let righe = [
            ev(1, Stato::Affidato, "t1"),
            ev(1, Stato::Fermato, "t2"),
            ev(1, Stato::InCorso, "t3"),
            ev(1, Stato::Affidato, "t4"),
        ]
        .join("\n");
        let c = compiti(&righe);
        assert_eq!(c[0].stato, Stato::Fermato);
        assert!(prossimo(&c).is_none());
    }

    #[test]
    fn dopo_un_riavvio_si_riprende_ma_non_per_sempre() {
        let mut righe = vec![ev(1, Stato::Affidato, "t1"), ev(1, Stato::InCorso, "t2")];
        for giro in 0..=RIPRESE_MASSIME {
            let c = compiti(&righe.join("\n"));
            let ev = alla_ripartenza(&c, &format!("r{giro}"));
            assert_eq!(ev.len(), 1);
            righe.push(serde_json::to_string(&ev[0]).unwrap());
            if giro < RIPRESE_MASSIME {
                assert_eq!(ev[0].stato, Stato::Affidato, "giro {giro}");
                let c = compiti(&righe.join("\n"));
                assert_eq!(c[0].riprese, giro + 1);
                assert_eq!(prossimo(&c).map(|c| c.id), Some(1));
                righe.push(super::prove::ev(1, Stato::InCorso, "di nuovo"));
            } else {
                assert_eq!(ev[0].stato, Stato::Interrotto);
                let c = compiti(&righe.join("\n"));
                assert_eq!(c[0].stato, Stato::Interrotto);
                assert!(c[0].esito.contains("3 volte"), "{}", c[0].esito);
            }
        }
        // Un compito in coda o finito non si tocca alla ripartenza.
        let c = compiti(&[ev(1, Stato::Affidato, "t"), ev(2, Stato::Affidato, "t"), ev(2, Stato::Fatto, "t")].join("\n"));
        assert!(alla_ripartenza(&c, "r").is_empty());
    }

    #[test]
    fn la_conversazione_si_taglia_davanti_a_una_domanda() {
        let mut m = vec![json!({"role": "system", "content": "s"})];
        for i in 0..5 {
            m.push(json!({"role": "user", "content": format!("d{i}")}));
            m.push(json!({"role": "assistant", "content": "", "tool_calls": [{"id": format!("c{i}")}]}));
            m.push(json!({"role": "tool", "tool_call_id": format!("c{i}"), "content": "r"}));
            m.push(json!({"role": "assistant", "content": format!("a{i}")}));
        }
        pota(&mut m, 10);
        assert_eq!(m[0]["role"], "system");
        assert_eq!(m[1]["role"], "user", "si comincia da una domanda");
        assert!(m.len() <= 10, "{}", m.len());
        assert_eq!(m.last().unwrap()["content"], "a4");
        // Sotto il tetto non si tocca niente.
        let mut corta = vec![json!({"role": "system"}), json!({"role": "user"})];
        pota(&mut corta, 10);
        assert_eq!(corta.len(), 2);
    }

    #[test]
    fn il_prompt_dice_chi_e_e_le_domande_dicono_quale_compito() {
        let d = Dot { nome: "ricercatore".into(), ruolo: "Cerchi e riassumi.".into(), nato: "t".into(), mestiere: Mestiere::Generico, capo: String::new() };
        let p = prompt(&d, "BASE");
        assert!(p.starts_with("BASE\n\n"));
        assert!(p.contains("sei ricercatore, un Dot di NOVA") && p.contains("Cerchi e riassumi."));
        let c = &compiti(&ev(7, Stato::Affidato, "t"))[0];
        assert_eq!(domanda(c), "Compito n. 7 (affidato da utente):\ncompito 7");
        assert!(continua(c).contains("n. 7"));
        assert!(
            !p.contains(ricerca::PROMPT),
            "un Dot generico non e' un ricercatore"
        );
        let r = Dot {
            mestiere: Mestiere::Ricercatore,
            ..d
        };
        assert!(prompt(&r, "BASE").ends_with(ricerca::PROMPT));
    }

    #[test]
    fn il_mestiere_si_sceglie_e_quello_di_prima_resta_generico() {
        assert_eq!(Mestiere::da(" ").unwrap(), Mestiere::Generico);
        assert_eq!(Mestiere::da("generico").unwrap(), Mestiere::Generico);
        assert_eq!(
            Mestiere::da(" ricercatore ").unwrap(),
            Mestiere::Ricercatore
        );
        assert!(Mestiere::da("Ricercatore")
            .unwrap_err()
            .contains("non e' un mestiere"));
        // Il custode non si sceglie: lo fa nascere NOVA, ed e' uno solo.
        assert!(Mestiere::da("custode").unwrap_err().contains("uno solo"));
        let c: Dot =
            serde_json::from_str(r#"{"nome":"custode","ruolo":"r","nato":"t","mestiere":"custode"}"#)
                .unwrap();
        assert_eq!(c.mestiere, Mestiere::Custode);
        // Un dot.json scritto dal D382, senza mestiere, si legge ancora.
        let vecchio: Dot = serde_json::from_str(r#"{"nome":"a","ruolo":"r","nato":"t"}"#).unwrap();
        assert_eq!(vecchio.mestiere, Mestiere::Generico);
        let nuovo = serde_json::to_value(Dot {
            mestiere: Mestiere::Ricercatore,
            ..vecchio
        })
        .unwrap();
        assert_eq!(nuovo["mestiere"], "ricercatore");
    }

    #[test]
    fn due_fili_che_scrivono_insieme_non_perdono_righe() {
        let base = std::env::temp_dir().join(format!("nova-dot-fili-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let c = Cartella::di(&base, "fili").unwrap();
        c.crea(&Dot { nome: "fili".into(), ruolo: "r".into(), nato: "t".into(), mestiere: Mestiere::Generico, capo: String::new() }).unwrap();
        let fili: Vec<_> = (0..8)
            .map(|f| {
                let c = c.clone();
                std::thread::spawn(move || {
                    for i in 0..50 {
                        let id = c.affida(&format!("compito {f}-{i} {}", "x".repeat(300)), "prova", "t").unwrap();
                        c.annota(&Evento {
                            id,
                            stato: Stato::InCorso,
                            quando: "t".into(),
                            testo: String::new(),
                            da: String::new(),
                            esito: String::new(),
                            padre: None,
                        })
                        .unwrap();
                    }
                })
            })
            .collect();
        for f in fili {
            f.join().unwrap();
        }
        let coda = c.compiti();
        assert_eq!(coda.len(), 400, "nessun compito perso");
        let mut id: Vec<u64> = coda.iter().map(|c| c.id).collect();
        id.dedup();
        assert_eq!(id, (1..=400).collect::<Vec<u64>>(), "nessun numero doppio");
        assert!(coda.iter().all(|c| c.stato == Stato::InCorso), "nessun passaggio perso");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn la_cartella_si_crea_si_rilegge_e_non_si_riscrive() {
        let base = std::env::temp_dir().join(format!("nova-dot-prova-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let c = Cartella::di(&base, "ricercatore").unwrap();
        assert!(Cartella::di(&base, "../fuori").is_err());
        let d = Dot { nome: "ricercatore".into(), ruolo: "r".into(), nato: "t".into(), mestiere: Mestiere::Generico, capo: String::new() };
        c.crea(&d).unwrap();
        assert_eq!(c.dot().unwrap(), d);
        assert!(c.vault().is_dir() && base.join("ricercatore").join("rapporti").is_dir());
        let r = c.salva_rapporto(3, "# Rapporto\n").unwrap();
        assert_eq!(r, c.rapporto(3));
        assert_eq!(std::fs::read_to_string(&r).unwrap(), "# Rapporto\n");
        assert!(c.crea(&d).is_err(), "un Dot che c'e' non si riscrive");
        c.annota(&serde_json::from_str(&ev(1, Stato::Affidato, "t")).unwrap()).unwrap();
        assert_eq!(c.compiti().len(), 1);
        let conv = Conversazione { messaggi: vec![json!({"role": "system", "content": "s"})], claude: "x".into(), deleghe: 2 };
        c.salva_conversazione(&conv).unwrap();
        assert_eq!(c.conversazione().unwrap(), conv);
        c.diario(&json!({"tipo": "prova"})).unwrap();
        std::fs::create_dir_all(base.join("Non-Valido")).unwrap();
        std::fs::write(base.join("Non-Valido").join("dot.json"), "{}").unwrap();
        std::fs::create_dir_all(base.join("vuota")).unwrap();
        assert_eq!(elenco(&base), vec!["ricercatore".to_string()]);
        let _ = std::fs::remove_dir_all(&base);
    }
}

/// La squadra (D388): i pezzi affidati ai sottoposti, l'attesa, la posta.
#[cfg(test)]
mod prove_squadra {
    use super::*;

    fn riga<T: Serialize>(x: &T) -> String {
        serde_json::to_string(x).unwrap()
    }

    fn ev(id: u64, stato: Stato) -> Evento {
        Evento {
            id,
            stato,
            quando: format!("t-{id}-{stato:?}"),
            testo: if stato == Stato::Affidato { format!("compito {id}") } else { String::new() },
            da: if stato == Stato::Affidato { "nova".into() } else { String::new() },
            esito: String::new(),
            padre: None,
        }
    }

    fn nota(id: u64, n: Squadra) -> String {
        riga(&Nota { id, quando: "t".into(), nota: n })
    }

    fn affidato(dot: &str, compito: u64) -> Squadra {
        Squadra::Affidato { dot: dot.into(), compito }
    }

    fn consegnato(dot: &str, compito: u64, esito: &str) -> Squadra {
        Squadra::Consegnato { dot: dot.into(), compito, stato: Stato::Fatto, esito: esito.into() }
    }

    #[test]
    fn il_capo_aspetta_tutti_e_poi_riprende_con_gli_esiti_non_ancora_letti() {
        let mut righe = vec![
            riga(&ev(1, Stato::Affidato)),
            riga(&ev(2, Stato::Affidato)),
            riga(&ev(1, Stato::InCorso)),
            nota(1, affidato("uno", 4)),
            nota(1, affidato("due", 7)),
            nota(1, affidato("due", 7)), // la stessa nota due volte conta una
            riga(&ev(1, Stato::InAttesa)),
        ];
        let c = compiti(&righe.join("\n"));
        assert_eq!(c[0].stato, Stato::InAttesa);
        assert_eq!(c[0].attende.len(), 2);
        assert!(da_aspettare(&c[0]) && !pronto(&c[0]));
        // In attesa, il Dot fa l'altro compito.
        assert_eq!(prossimo(&c).map(|x| x.id), Some(2));

        righe.push(nota(1, consegnato("uno", 4, "A fatto")));
        let c = compiti(&righe.join("\n"));
        assert!(!pronto(&c[0]), "manca ancora due");

        righe.push(nota(1, consegnato("due", 7, "B fatto")));
        // Una consegna ripetuta non cambia un pezzo gia' chiuso.
        righe.push(nota(1, Squadra::Consegnato { dot: "due".into(), compito: 7, stato: Stato::Fallito, esito: "x".into() }));
        let c = compiti(&righe.join("\n"));
        assert!(pronto(&c[0]));
        assert_eq!(prossimo(&c).map(|x| x.id), Some(1), "chi riprende passa davanti al compito nuovo");
        let r = ripresa(&c[0]);
        assert_eq!(
            r,
            "Compito n. 1: i tuoi sottoposti hanno consegnato.\n\
             - uno, compito n. 4 (fatto): A fatto\n\
             - due, compito n. 7 (fatto): B fatto\n\
             Con questo, finisci il compito n. 1: compito 1"
        );

        // Ripreso: gli esiti sono letti, e a fine turno non c'e' piu' da
        // aspettare.
        righe.push(riga(&ev(1, Stato::InCorso)));
        let c = compiti(&righe.join("\n"));
        assert!(c[0].attende.iter().all(|s| s.letto));
        assert!(!da_aspettare(&c[0]));
        // Un pezzo nuovo affidato dopo la ripresa si aspetta di nuovo.
        righe.push(nota(1, affidato("uno", 5)));
        let c = compiti(&righe.join("\n"));
        assert!(da_aspettare(&c[0]));
        assert_eq!(ripresa(&c[0]).matches("- ").count(), 0, "uno#5 non e' ancora consegnato");
    }

    #[test]
    fn un_pezzo_sa_di_chi_e_e_la_nota_non_si_scambia_per_un_passaggio() {
        let mut e = ev(3, Stato::Affidato);
        e.padre = Some(Rif { dot: "capo".into(), id: 1 });
        let c = compiti(&riga(&e));
        assert_eq!(c[0].padre, Some(Rif { dot: "capo".into(), id: 1 }));
        // Una riga di prima del D388 si legge ancora, senza padre.
        let vecchia = r#"{"id":1,"stato":"affidato","quando":"t","testo":"x","da":"utente"}"#;
        assert_eq!(compiti(vecchia)[0].padre, None);
        // La nota non ha lo stato: letta come passaggio non passa.
        assert!(serde_json::from_str::<Evento>(&nota(1, affidato("uno", 2))).is_err());
        // Una nota per un compito che non c'e' si salta.
        assert!(compiti(&nota(9, affidato("uno", 2))).is_empty());
    }

    #[test]
    fn dopo_un_riavvio_chi_aspetta_resta_in_attesa() {
        let righe = [riga(&ev(1, Stato::Affidato)), riga(&ev(1, Stato::InCorso)), riga(&ev(1, Stato::InAttesa))]
            .join("\n");
        assert!(alla_ripartenza(&compiti(&righe), "t").is_empty());
        assert!(!Stato::InAttesa.chiuso());
    }

    #[test]
    fn la_squadra_si_dice_nella_domanda_solo_se_c_e() {
        assert_eq!(squadra("", &[]), "");
        assert_eq!(squadra(" capo ", &[]), "\n\n(Il tuo capo e' capo.)");
        let s = squadra("", &["uno".into(), "due".into()]);
        assert!(s.starts_with("\n\n(I tuoi sottoposti: uno, due. A loro puoi affidare"), "{s}");
        assert!(squadra("capo", &["uno".into()]).contains("Il tuo capo e' capo. I tuoi sottoposti: uno."));
    }

    #[test]
    fn la_posta_si_legge_col_compito_e_non_torna_indietro() {
        let d = std::env::temp_dir().join(format!("nova-posta-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&d);
        let c = Cartella::di(&d, "uno").unwrap();
        c.crea(&Dot { nome: "uno".into(), ruolo: "r".into(), nato: "t".into(), mestiere: Mestiere::Generico, capo: "capo".into() })
            .unwrap();
        assert_eq!(c.dot().unwrap().capo, "capo");
        assert!(c.non_letta().is_empty());
        assert_eq!(c.imbuca("nova", "uno", "Ricorda il formato", "t1").unwrap(), 1);
        assert_eq!(c.imbuca("due", "gruppo:squadra", "Ci sono", "t2").unwrap(), 2);
        assert_eq!(c.imbuca("tre", "due,uno", "A tutti e due", "t3").unwrap(), 3);
        let posta = c.non_letta();
        let (domanda, letto) = con_la_posta("Compito n. 1:\nfai", &posta);
        assert_eq!(
            domanda,
            "Compito n. 1:\nfai\n\nMessaggi arrivati prima di questo compito:\n\
             - da nova: Ricorda il formato\n- da due nel gruppo «squadra»: Ci sono\n\
             - da tre a due, uno insieme: A tutti e due"
        );
        assert_eq!(letto, Some(3));
        c.segna_letta(3).unwrap();
        assert!(c.non_letta().is_empty());
        c.segna_letta(1).unwrap();
        assert_eq!(c.letta_fino_a(), 3, "segnare meno non torna indietro");
        assert_eq!(con_la_posta("d", &[]), ("d".to_string(), None));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn troppa_posta_si_ferma_e_lo_dice() {
        let lunghi: Vec<Messaggio> = (1..=5)
            .map(|n| Messaggio { n, da: "nova".into(), a: "x".into(), testo: "m".repeat(3_000), quando: "t".into() })
            .collect();
        let (t, letto) = con_la_posta("d", &lunghi);
        assert_eq!(letto, Some(2), "8.000 caratteri ne tengono due da 3.000");
        assert!(t.ends_with("(Ci sono altri messaggi: li leggerai al prossimo compito.)"));
        // Un messaggio solo, anche lungo, entra sempre: se no non si leggerebbe mai.
        let uno = vec![Messaggio { n: 1, da: "nova".into(), a: "x".into(), testo: "m".repeat(9_000), quando: "t".into() }];
        assert_eq!(con_la_posta("d", &uno).1, Some(1));
    }

    #[test]
    fn i_nomi_presi_non_sono_nomi_di_dot() {
        for preso in NOMI_PRESI {
            assert!(nome_valido(preso).unwrap_err().contains("gia' preso"), "{preso}");
        }
        assert_eq!(NOMI_PRESI, ["gruppi", "nova"]);
    }
}

#[cfg(test)]
mod prove_attese {
    use super::*;

    /// Un capo che a ogni ripresa affida un pezzo nuovo non gira per sempre.
    #[test]
    fn dopo_le_attese_massime_il_compito_si_chiude_e_lo_dice() {
        let ev = |stato| {
            serde_json::to_string(&Evento {
                id: 1,
                stato,
                quando: "t".into(),
                testo: if stato == Stato::Affidato { "gira".into() } else { String::new() },
                da: "nova".into(),
                esito: String::new(),
                padre: None,
            })
            .unwrap()
        };
        let nota = |n: Squadra| serde_json::to_string(&Nota { id: 1, quando: "t".into(), nota: n }).unwrap();
        let mut righe = vec![ev(Stato::Affidato), ev(Stato::InCorso)];
        for giro in 1..=ATTESE_MASSIME as u64 {
            righe.push(nota(Squadra::Affidato { dot: "uno".into(), compito: giro }));
            let c = compiti(&righe.join("\n"));
            assert!(da_aspettare(&c[0]), "giro {giro}");
            assert!(senza_aspettare_oltre(&c[0]).is_none());
            righe.push(ev(Stato::InAttesa));
            righe.push(nota(Squadra::Consegnato { dot: "uno".into(), compito: giro, stato: Stato::Fatto, esito: "x".into() }));
            righe.push(ev(Stato::InCorso));
        }
        righe.push(nota(Squadra::Affidato { dot: "uno".into(), compito: 99 }));
        let c = compiti(&righe.join("\n"));
        assert_eq!(c[0].attese, ATTESE_MASSIME);
        assert!(!da_aspettare(&c[0]), "dopo cinque attese non aspetta piu'");
        assert_eq!(
            senza_aspettare_oltre(&c[0]).as_deref(),
            Some("(Chiuso dopo 5 attese: i pezzi ancora aperti, uno n. 99, non tornano qui.)")
        );
    }
}
