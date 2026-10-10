//! Il piano di sviluppo dell'Architetto (D400).
//!
//! Deciso con Gio il 10 ottobre, sulla bozza, prima del codice
//! (`docs/dots.md`, «L'Architetto»):
//!
//! - **l'Architetto e' un Dot che legge e basta**: apre i file del progetto e
//!   la sua memoria con gli strumenti di [`STRUMENTI_DELL_ARCHITETTO`], e non
//!   tocca niente. Il cervello e' quello di AR: il piu' grande che risponde a
//!   un indirizzo, perche' chi ha mani sue (Claude Code, le CLI) potrebbe
//!   scrivere;
//! - **non chiede prima a Ricerca e Commerciale**: se servono lo stato
//!   dell'arte o il mercato, li mette nel piano, come compiti di una prima
//!   fase. Cosi' la spesa resta dentro il piano che l'utente approva;
//! - **fasi con i compiti gia' assegnati**: ogni compito dice chi lo fa (un
//!   reparto, o un ruolo che AR trovera') e da quali compiti dipende. Un capo
//!   puo' ancora dividere un suo compito;
//! - **rivede** quando lo chiede l'utente, e da solo quando una fase fallisce
//!   o la revisione la boccia, al piu' [`REVISIONI_PER_FASE`] volte per fase;
//!   alla terza decide l'utente. Ogni versione resta.
//!
//! **Il piano e' in Markdown, non in JSON** (Gio, 10 ottobre): costa meno
//! token, e lo legge anche una persona. Il formato e' fisso, e qui c'e' chi
//! lo legge ([`leggi`], con tutti i controlli) e chi lo riscrive in forma
//! pulita ([`scrivi`]): quello che va su disco e' sempre la forma pulita, e
//! si rilegge identico.
//!
//! I piani stanno nella cartella del progetto (D402,
//! [`crate::progetto`]): `progetti/<progetto>/piani/piano-<versione>.md`.
//! Col D400 stavano nella cartella dell'Architetto.

use std::path::{Path, PathBuf};

use crate::azienda::{self, Posto};
use crate::{Compito, Mestiere, Stato};

/// Gli strumenti dell'Architetto, e solo quelli: leggere i file del
/// progetto (anche i documenti), cercarci dentro, la sua memoria, l'ora.
/// Il web no: lo stato dell'arte e' della ricerca. Ogni altro strumento,
/// anche innocuo, si rifiuta (`nova_core::permessi::per_un_dot`).
pub const STRUMENTI_DELL_ARCHITETTO: [&str; 8] = [
    "documenti.leggi",
    "fs.grep",
    "fs.list",
    "fs.read",
    "fs.search",
    "fs.stat",
    "kb.cerca",
    "sys.ora",
];

/// Quanti turni ha l'Architetto per leggere il progetto e scrivere il
/// piano. Un turno finisce quando il cervello risponde o finisce i passi.
pub const TURNI_PER_PIANO: usize = 6;

/// Quante volte gli si chiede di riscrivere un piano che non si legge.
pub const CORREZIONI: usize = 2;

/// Quante volte una fase si rivede da sola, dopo che e' fallita o la
/// revisione l'ha bocciata. Alla terza decide l'utente. Una revisione
/// chiesta dall'utente fa ripartire il conto.
pub const REVISIONI_PER_FASE: u32 = 2;

/// Quanti compiti puo' avere un piano. Oltre, non e' un piano: e' un elenco
/// che nessuno potra' seguire.
pub const COMPITI_MASSIMI: usize = 200;

/// Quanto puo' essere lungo il nome di un progetto.
pub const PROGETTO_MASSIMO: usize = 48;

/// Il segno in testa a ogni domanda all'Architetto.
pub const SEGNO: &str = "[piano di sviluppo]";

/// La cartella dei piani, dentro quella del progetto.
pub const CARTELLA: &str = "piani";

/// Chi fa un compito del piano.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Chi {
    /// Un reparto che c'e' sempre (D395) e prende compiti.
    Reparto(String),
    /// Un ruolo che manca: lo trova AR, riprendendo un Dot libero che fa al
    /// caso o assumendone uno (D397).
    Assumi { ruolo: String, mestiere: Mestiere },
}

/// Un compito del piano.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Incarico {
    /// `<fase>.<numero>`, unico nel piano.
    pub id: String,
    pub cosa: String,
    pub chi: Chi,
    /// I compiti da cui dipende: quelli senza dipendenze possono partire
    /// insieme.
    pub dopo: Vec<String>,
}

/// Una fase del piano.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fase {
    /// Da 1, una dopo l'altra.
    pub n: u32,
    pub nome: String,
    /// Cosa produce.
    pub consegna: String,
    /// Come si sa che e' finita.
    pub fatta_quando: String,
    pub incarichi: Vec<Incarico>,
}

/// Un piano di sviluppo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piano {
    pub progetto: String,
    /// Una riga: cosa esiste alla fine che oggi non c'e'.
    pub obiettivo: String,
    /// Da 1. La scrive NOVA, non il cervello.
    pub versione: u32,
    /// La cartella dei file del progetto, se c'e'. La scrive NOVA.
    pub cartella: String,
    /// Dalla seconda versione: cosa cambia rispetto a prima, e perche'.
    pub cambia: Vec<String>,
    pub fasi: Vec<Fase>,
    pub rischi: Vec<String>,
    /// Quello che l'Architetto non sa decidere da solo: l'APM le mostra
    /// all'utente insieme al via.
    pub domande: Vec<String>,
}

impl Piano {
    /// Tutti i compiti, in ordine.
    pub fn incarichi(&self) -> impl Iterator<Item = &Incarico> {
        self.fasi.iter().flat_map(|f| f.incarichi.iter())
    }

    /// Quanti compiti vanno a chi manca: li trova AR.
    pub fn da_assumere(&self) -> usize {
        self.incarichi()
            .filter(|i| matches!(i.chi, Chi::Assumi { .. }))
            .count()
    }
}

/// I reparti a cui il piano puo' dare un compito: i posti fissi che
/// prendono compiti. Il legale no: lo chiama l'APM (D395).
pub fn reparti() -> Vec<&'static Posto> {
    azienda::POSTI
        .iter()
        .filter(|p| p.mestiere.prende_compiti())
        .collect()
}

/// Il nome di un progetto, se va bene: come quello di un Dot, minuscole,
/// cifre e trattini, perche' diventa il nome di una cartella.
pub fn nome_del_progetto(nome: &str) -> Result<String, String> {
    let n = nome.trim();
    if n.is_empty() {
        return Err("un progetto ha bisogno di un nome".into());
    }
    if n.chars().count() > PROGETTO_MASSIMO {
        return Err(format!(
            "il nome di un progetto sta in {PROGETTO_MASSIMO} caratteri"
        ));
    }
    if !n
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(format!(
            "«{n}»: il nome di un progetto e' fatto di lettere minuscole, cifre e trattini"
        ));
    }
    if n.starts_with('-') {
        return Err(format!(
            "«{n}»: il nome di un progetto comincia con una lettera o una cifra"
        ));
    }
    Ok(n.to_string())
}

// ------------------------------------------------------------ la richiesta

/// Perche' si chiede un piano.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Perche {
    /// Lo chiede l'utente: un piano nuovo, o una versione con quel che vuole
    /// cambiare.
    Utente,
    /// La fase con quel numero non e' andata: fallita, o bocciata dalla
    /// revisione. Si rivede da sola, al piu' [`REVISIONI_PER_FASE`] volte.
    Fase(u32),
}

/// Cosa si chiede all'Architetto. Viaggia nel testo del compito, in coda
/// all'Architetto: la prima riga e' per chi guarda la coda, le altre per
/// NOVA ([`Richiesta::dal_compito`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Richiesta {
    pub progetto: String,
    /// La cartella dei file del progetto, o vuoto.
    pub cartella: String,
    pub perche: Perche,
    /// La richiesta dell'utente, cosa cambiare, o perche' la fase non e'
    /// andata.
    pub testo: String,
}

impl Richiesta {
    /// Il testo del compito.
    pub fn nel_compito(&self) -> String {
        let titolo = match self.perche {
            Perche::Utente => format!("Piano di sviluppo di «{}»", self.progetto),
            Perche::Fase(n) => {
                format!(
                    "Rivedere il piano di «{}»: la fase {n} non e' andata",
                    self.progetto
                )
            }
        };
        let mut t = format!("{titolo}\nprogetto: {}\n", self.progetto);
        if !self.cartella.trim().is_empty() {
            t.push_str(&format!("cartella: {}\n", self.cartella.trim()));
        }
        if let Perche::Fase(n) = self.perche {
            t.push_str(&format!("fase: {n}\n"));
        }
        t.push('\n');
        t.push_str(self.testo.trim());
        t
    }

    /// La richiesta dal testo di un compito. La prima riga si salta: e' per
    /// chi guarda la coda.
    pub fn dal_compito(testo: &str) -> Result<Richiesta, String> {
        let mut righe = testo.lines().skip(1);
        let mut progetto = None;
        let mut cartella = String::new();
        let mut perche = Perche::Utente;
        for riga in righe.by_ref() {
            if riga.trim().is_empty() {
                break;
            }
            let (chiave, valore) = riga
                .split_once(':')
                .ok_or_else(|| format!("«{riga}» non e' una riga della richiesta"))?;
            let valore = valore.trim();
            match chiave.trim() {
                "progetto" => progetto = Some(nome_del_progetto(valore)?),
                "cartella" => cartella = valore.to_string(),
                "fase" => {
                    let n: u32 = valore
                        .parse()
                        .map_err(|_| format!("«{valore}» non e' il numero di una fase"))?;
                    if n == 0 {
                        return Err("le fasi si contano da 1".into());
                    }
                    perche = Perche::Fase(n);
                }
                altro => return Err(format!("«{altro}» non e' una voce della richiesta")),
            }
        }
        let progetto = progetto.ok_or("la richiesta non dice di che progetto e'")?;
        let testo = righe.collect::<Vec<_>>().join("\n").trim().to_string();
        if testo.is_empty() {
            return Err("la richiesta e' vuota".into());
        }
        Ok(Richiesta {
            progetto,
            cartella,
            perche,
            testo,
        })
    }
}

/// Quante revisioni automatiche ha gia' avuto, o ha in coda, la fase `fase`
/// del piano di `progetto`, nella coda dell'Architetto. Si contano da dopo
/// l'ultimo piano chiesto dall'utente e fatto: quando l'utente rivede il
/// piano, il conto riparte. Un compito fallito, fermato o interrotto non ha
/// dato un piano, e non conta.
pub fn revisioni_automatiche(coda: &[Compito], progetto: &str, fase: u32) -> u32 {
    let di = |c: &Compito| {
        Richiesta::dal_compito(&c.testo)
            .ok()
            .filter(|r| r.progetto == progetto)
    };
    let da = coda
        .iter()
        .filter(|c| c.stato == Stato::Fatto)
        .filter(|c| di(c).is_some_and(|r| r.perche == Perche::Utente))
        .map(|c| c.id)
        .max()
        .unwrap_or(0);
    coda.iter()
        .filter(|c| c.id > da)
        .filter(|c| {
            matches!(
                c.stato,
                Stato::Affidato | Stato::InCorso | Stato::InAttesa | Stato::Fatto
            )
        })
        .filter(|c| di(c).is_some_and(|r| r.perche == Perche::Fase(fase)))
        .count() as u32
}

// --------------------------------------------------- le parole al cervello

/// Il formato del piano, come lo legge [`leggi`]. Sta nel prompt di sistema
/// dell'Architetto, che i fornitori tengono in cache: non si ripete a ogni
/// domanda.
pub fn prompt() -> String {
    let reparti: Vec<&str> = reparti().iter().map(|p| p.nome).collect();
    format!(
        "{}\n\
         Leggi, non scrivi: i tuoi strumenti servono a leggere i file del progetto e la tua \
         memoria. Lo stato dell'arte e il mercato non li cerchi tu: se servono, mettili nel \
         piano, come compiti della ricerca e del commerciale in una prima fase.\n\
         Il piano si scrive cosi', in Markdown e nient'altro:\n\
         # Piano: <progetto>\n\
         Obiettivo: <una riga: cosa esiste alla fine che oggi non c'e'>\n\
         ## Fase 1: <nome>\n\
         Consegna: <cosa produce>\n\
         Fatta quando: <come si sa che e' finita>\n\
         - 1.1 [<chi>] <cosa fare>\n\
         - 1.2 [<chi>] <cosa fare> (dopo 1.1)\n\
         ## Rischi\n\
         - <un rischio>\n\
         ## Domande per te\n\
         - <quello che non sai decidere da solo>\n\
         <chi> e' un reparto ({}), oppure «assumi: <ruolo>» per chi manca, o «assumi \
         ricercatore: <ruolo>» per un ricercatore: lo trova AR. Il legale non prende compiti: \
         lo chiama l'APM. Ogni compito ha un id <fase>.<numero>; «(dopo ...)» dice da quali \
         compiti dipende, e quelli senza dipendenze partono insieme.",
        azienda::PROMPT_ARCHITETTO,
        reparti.join(", "),
    )
}

/// La domanda all'Architetto per la versione `versione` del piano.
/// `precedente` e' il piano di adesso, se c'e'.
pub fn domanda(r: &Richiesta, versione: u32, precedente: Option<&str>) -> String {
    let mut t = format!("{SEGNO} Progetto «{}», versione {versione}.\n", r.progetto);
    if r.cartella.trim().is_empty() {
        t.push_str("Il progetto non ha una cartella: pianifica dalla richiesta.\n");
    } else {
        t.push_str(&format!(
            "I file del progetto sono in «{}»: leggili prima di pianificare.\n",
            r.cartella.trim()
        ));
    }
    t.push('\n');
    match (r.perche, precedente) {
        (Perche::Fase(n), _) => t.push_str(&format!(
            "La fase {n} non e' andata: e' fallita, o la revisione l'ha bocciata.\n{}\n\
             Correggi il piano da li': le fasi gia' fatte restano.",
            r.testo.trim()
        )),
        (Perche::Utente, Some(_)) => t.push_str(&format!(
            "Cosa cambiare, lo chiede l'utente:\n{}",
            r.testo.trim()
        )),
        (Perche::Utente, None) => t.push_str(&format!("La richiesta:\n{}", r.testo.trim())),
    }
    if let Some(p) = precedente {
        t.push_str(&format!(
            "\n\nIl piano di adesso, versione {}:\n{}",
            versione.saturating_sub(1),
            p.trim()
        ));
    }
    t.push_str("\n\nRispondi solo col piano, nel formato del tuo ruolo.");
    if versione > 1 {
        t.push_str(&format!(
            " Subito dopo l'obiettivo metti «## Cosa cambia»: cosa cambia rispetto alla \
             versione {}, e perche'.",
            versione - 1
        ));
    }
    t
}

/// Quando il cervello finisce i passi di un turno senza scrivere il piano.
pub const CONTINUA: &str = "[piano di sviluppo] Hai finito i passi di questo turno. Se hai \
letto abbastanza scrivi il piano, nel formato; se no continua a leggere, e poi scrivilo.";

/// Quando il piano non si legge: cosa non va, per riscriverlo.
pub fn correggi(errori: &[String]) -> String {
    format!(
        "{SEGNO} Il piano non si legge:\n- {}\nRiscrivilo intero, nel formato, corretto.",
        errori.join("\n- ")
    )
}

// ---------------------------------------------------------- leggere il piano

/// Dove si e' arrivati leggendo.
enum Dove {
    Prima,
    Testa,
    Fase,
    Cambia,
    Rischi,
    Domande,
    Fuori,
}

/// Una riga senza i segni del grassetto, che i modelli mettono sulle voci.
fn pulita(riga: &str) -> String {
    riga.replace("**", "").trim().to_string()
}

/// Il valore di una voce `Chiave: valore`, se la riga e' quella voce.
fn voce(riga: &str, chiave: &str) -> Option<String> {
    let (k, v) = riga.split_once(':')?;
    k.trim()
        .eq_ignore_ascii_case(chiave)
        .then(|| v.trim().to_string())
}

/// Il contenuto di una riga di elenco, se lo e'.
fn elenco(riga: &str) -> Option<&str> {
    riga.strip_prefix("- ")
        .or_else(|| riga.strip_prefix("* "))
        .map(str::trim)
}

/// `Fase 2: nome`, `Fase 2 - nome`, `Fase 2. nome`.
fn titolo_di_fase(t: &str) -> Option<(u32, String)> {
    let resto = t
        .get(..4)
        .filter(|p| p.eq_ignore_ascii_case("fase"))
        .map(|_| &t[4..])?;
    let resto = resto.trim_start();
    let cifre: String = resto.chars().take_while(char::is_ascii_digit).collect();
    let n: u32 = cifre.parse().ok()?;
    let nome = resto[cifre.len()..]
        .trim_start_matches(|c: char| c.is_whitespace() || ":.-—–".contains(c))
        .trim()
        .to_string();
    Some((n, nome))
}

/// Chi fa un compito, da quello che c'e' fra le quadre.
fn leggi_chi(t: &str) -> Result<Chi, String> {
    let t = t.trim();
    let basso = t.to_lowercase();
    if let Some(resto) = basso.strip_prefix("assumi") {
        let Some((come, _)) = resto.split_once(':') else {
            return Err(format!("«{t}»: per chi manca si scrive «assumi: <ruolo>»"));
        };
        let mestiere = match come.trim() {
            "" | "generico" => Mestiere::Generico,
            "ricercatore" => Mestiere::Ricercatore,
            altro => {
                return Err(format!(
                    "«{altro}» non e' un mestiere da assumere: «assumi: <ruolo>» o «assumi \
                     ricercatore: <ruolo>»"
                ))
            }
        };
        // Il ruolo si tiene com'e' scritto, maiuscole comprese.
        let ruolo = t[t.find(':').map_or(t.len(), |i| i + 1)..]
            .trim()
            .to_string();
        if ruolo.is_empty() {
            return Err("a chi si assume serve un ruolo: «assumi: <ruolo>»".into());
        }
        return Ok(Chi::Assumi { ruolo, mestiere });
    }
    if let Some(p) = reparti().into_iter().find(|p| p.nome == basso) {
        return Ok(Chi::Reparto(p.nome.to_string()));
    }
    if let Some((chi, cosa)) = azienda::posto(&basso).and_then(|p| p.mestiere.a_parte()) {
        return Err(format!("«{basso}»: {chi} non prende compiti, {cosa}"));
    }
    let nomi: Vec<&str> = reparti().iter().map(|p| p.nome).collect();
    Err(format!(
        "«{t}» non e' un reparto: i reparti sono {}; per chi manca «assumi: <ruolo>»",
        nomi.join(", ")
    ))
}

/// Un compito, dalla riga di elenco: `1.2 [chi] cosa (dopo 1.1)`.
fn leggi_incarico(t: &str) -> Result<Incarico, String> {
    let (id, resto) = t.split_once(char::is_whitespace).unwrap_or((t, ""));
    let id = id.trim_end_matches([':', '.']).to_string();
    let (a, b) = id.split_once('.').unwrap_or(("", ""));
    if a.parse::<u32>().is_err() || b.parse::<u32>().is_err() {
        return Err(format!(
            "«{t}»: un compito comincia col suo id, <fase>.<numero>"
        ));
    }
    let resto = resto.trim();
    let Some(dentro) = resto.strip_prefix('[') else {
        return Err(format!(
            "compito {id}: dopo l'id viene chi lo fa, fra quadre"
        ));
    };
    let Some((chi, cosa)) = dentro.split_once(']') else {
        return Err(format!(
            "compito {id}: la quadra di chi lo fa non si chiude"
        ));
    };
    let chi = leggi_chi(chi).map_err(|e| format!("compito {id}: {e}"))?;
    let mut cosa = cosa.trim().to_string();
    let mut dopo = Vec::new();
    if cosa.ends_with(')') {
        if let Some(i) = cosa.to_lowercase().rfind("(dopo ") {
            let dentro = cosa[i + "(dopo ".len()..cosa.len() - 1].to_string();
            for d in dentro.split(',').flat_map(|x| x.split(" e ")) {
                let d = d.trim();
                if !d.is_empty() {
                    dopo.push(d.to_string());
                }
            }
            cosa = cosa[..i].trim().to_string();
        }
    }
    if cosa.is_empty() {
        return Err(format!("compito {id}: non dice cosa fare"));
    }
    Ok(Incarico {
        id,
        cosa,
        chi,
        dopo,
    })
}

/// Le dipendenze fanno un giro? Torna il giro, se c'e'.
fn giro(piano: &Piano) -> Option<Vec<String>> {
    fn visita<'a>(
        id: &'a str,
        dopo: &std::collections::HashMap<&'a str, &'a [String]>,
        stato: &mut std::collections::HashMap<&'a str, u8>,
        strada: &mut Vec<&'a str>,
    ) -> Option<Vec<String>> {
        match stato.get(id) {
            Some(2) => return None,
            Some(1) => {
                let da = strada.iter().position(|x| *x == id).unwrap_or(0);
                let mut g: Vec<String> = strada[da..].iter().map(|x| x.to_string()).collect();
                g.push(id.to_string());
                return Some(g);
            }
            _ => {}
        }
        stato.insert(id, 1);
        strada.push(id);
        for d in dopo.get(id).copied().unwrap_or_default() {
            if dopo.contains_key(d.as_str()) {
                if let Some(g) = visita(d, dopo, stato, strada) {
                    return Some(g);
                }
            }
        }
        strada.pop();
        stato.insert(id, 2);
        None
    }
    let dopo: std::collections::HashMap<&str, &[String]> = piano
        .incarichi()
        .map(|i| (i.id.as_str(), i.dopo.as_slice()))
        .collect();
    let mut stato = std::collections::HashMap::new();
    for i in piano.incarichi() {
        if let Some(g) = visita(&i.id, &dopo, &mut stato, &mut Vec::new()) {
            return Some(g);
        }
    }
    None
}

/// Legge la versione `versione` di un piano. Torna il piano e le note su
/// quel che si e' lasciato fuori, o tutti gli errori che lo rendono
/// illeggibile: chi l'ha scritto li riceve tutti insieme, e lo corregge in
/// una volta sola ([`correggi`]).
///
/// La versione e la cartella le scrive NOVA: una riga `Versione:` si salta,
/// e la versione e' quella chiesta.
pub fn leggi(testo: &str, versione: u32) -> Result<(Piano, Vec<String>), Vec<String>> {
    let mut p = Piano {
        progetto: String::new(),
        obiettivo: String::new(),
        versione,
        cartella: String::new(),
        cambia: Vec::new(),
        fasi: Vec::new(),
        rischi: Vec::new(),
        domande: Vec::new(),
    };
    let mut errori: Vec<String> = Vec::new();
    let mut note: Vec<String> = Vec::new();
    let mut titolo = false;
    let mut dove = Dove::Prima;
    let mut prima = false;
    let mut fuori = 0usize;
    for grezza in testo.lines() {
        // I recinti del codice, che i modelli mettono attorno al Markdown.
        if grezza.trim_start().starts_with("```") {
            continue;
        }
        let riga = pulita(grezza);
        if riga.is_empty() {
            continue;
        }
        if let Some(t) = riga.strip_prefix("# ") {
            if titolo {
                fuori += 1;
                continue;
            }
            titolo = true;
            let t = t.trim();
            let nome = t
                .get(..5)
                .filter(|x| x.eq_ignore_ascii_case("piano"))
                .map(|_| {
                    t[5..].trim_start_matches(|c: char| c.is_whitespace() || ":-—–".contains(c))
                });
            match nome {
                Some(n) => p.progetto = n.trim().to_string(),
                None => errori.push(format!(
                    "il titolo e' «# {t}»: si scrive «# Piano: <progetto>»"
                )),
            }
            dove = Dove::Testa;
            continue;
        }
        if matches!(dove, Dove::Prima) {
            prima = true;
            continue;
        }
        if let Some(t) = riga.strip_prefix("## ") {
            let t = t.trim();
            let basso = t.to_lowercase();
            if let Some((n, nome)) = titolo_di_fase(t) {
                if nome.is_empty() {
                    errori.push(format!("la fase {n} non ha un nome: «## Fase {n}: <nome>»"));
                }
                p.fasi.push(Fase {
                    n,
                    nome,
                    consegna: String::new(),
                    fatta_quando: String::new(),
                    incarichi: Vec::new(),
                });
                dove = Dove::Fase;
            } else if basso.starts_with("cosa cambia") {
                dove = Dove::Cambia;
            } else if basso.starts_with("rischi") {
                dove = Dove::Rischi;
            } else if basso.starts_with("domande") {
                dove = Dove::Domande;
            } else {
                note.push(format!(
                    "la sezione «{t}» non fa parte del piano: e' rimasta fuori"
                ));
                dove = Dove::Fuori;
            }
            continue;
        }
        match dove {
            Dove::Prima => {}
            Dove::Testa => {
                if let Some(v) = voce(&riga, "obiettivo") {
                    p.obiettivo = v;
                } else if let Some(v) = voce(&riga, "cartella") {
                    p.cartella = v;
                } else if voce(&riga, "versione").is_none() {
                    fuori += 1;
                }
            }
            Dove::Fase => {
                let Some(f) = p.fasi.last_mut() else { continue };
                if let Some(v) = voce(&riga, "consegna") {
                    f.consegna = v;
                } else if let Some(v) = voce(&riga, "fatta quando") {
                    f.fatta_quando = v;
                } else if let Some(t) = elenco(&riga) {
                    match leggi_incarico(t) {
                        Ok(i) => f.incarichi.push(i),
                        Err(e) => errori.push(format!("fase {}, {e}", f.n)),
                    }
                } else {
                    fuori += 1;
                }
            }
            Dove::Cambia | Dove::Rischi | Dove::Domande => {
                let lista = match dove {
                    Dove::Cambia => &mut p.cambia,
                    Dove::Rischi => &mut p.rischi,
                    _ => &mut p.domande,
                };
                match elenco(&riga) {
                    Some(t) if !t.is_empty() => lista.push(t.to_string()),
                    _ => fuori += 1,
                }
            }
            Dove::Fuori => {}
        }
    }
    if prima {
        note.push("c'era del testo prima del piano: e' rimasto fuori".into());
    }
    if fuori > 0 {
        note.push(format!(
            "{} fuori dal formato, rimast{} fuori",
            quanti(fuori, "riga", "righe"),
            if fuori == 1 { "a" } else { "e" }
        ));
    }

    // I controlli.
    if !titolo {
        errori.push("manca il titolo: la prima riga del piano e' «# Piano: <progetto>»".into());
    }
    if p.obiettivo.is_empty() {
        errori.push("manca l'obiettivo: «Obiettivo: <una riga>», sotto il titolo".into());
    }
    if p.fasi.is_empty() {
        errori.push("non c'e' nessuna fase: «## Fase 1: <nome>»".into());
    }
    let mut visti = std::collections::HashSet::new();
    for (i, f) in p.fasi.iter().enumerate() {
        let atteso = i as u32 + 1;
        if f.n != atteso {
            errori.push(format!(
                "le fasi si numerano da 1, una dopo l'altra: qui c'e' la fase {} dove va la {atteso}",
                f.n
            ));
        }
        if f.consegna.is_empty() {
            errori.push(format!(
                "la fase {} non dice cosa produce: «Consegna: ...»",
                f.n
            ));
        }
        if f.fatta_quando.is_empty() {
            errori.push(format!(
                "la fase {} non dice come si sa che e' finita: «Fatta quando: ...»",
                f.n
            ));
        }
        if f.incarichi.is_empty() {
            errori.push(format!(
                "la fase {} non ha compiti: «- {}.1 [<chi>] <cosa>»",
                f.n, f.n
            ));
        }
        for x in &f.incarichi {
            if x.id.split('.').next() != Some(f.n.to_string().as_str()) {
                errori.push(format!(
                    "il compito {} sta nella fase {}: il suo id comincia con {}.",
                    x.id, f.n, f.n
                ));
            }
            if !visti.insert(x.id.clone()) {
                errori.push(format!("il compito {} c'e' due volte", x.id));
            }
        }
    }
    for x in p.incarichi() {
        for d in &x.dopo {
            if *d == x.id {
                errori.push(format!(
                    "il compito {} non puo' venire dopo se stesso",
                    x.id
                ));
            } else if !visti.contains(d) {
                errori.push(format!("il compito {} viene dopo {d}, che non c'e'", x.id));
            }
        }
    }
    if errori.is_empty() {
        if let Some(g) = giro(&p) {
            errori.push(format!("le dipendenze fanno un giro: {}", g.join(" → ")));
        }
    }
    let quanti_compiti = p.incarichi().count();
    if quanti_compiti > COMPITI_MASSIMI {
        errori.push(format!(
            "{quanti_compiti} compiti sono troppi: un piano ne ha al piu' {COMPITI_MASSIMI}"
        ));
    }
    if versione > 1 && p.cambia.is_empty() {
        errori.push(format!(
            "e' la versione {versione}: scrivi cosa cambia rispetto alla {}, e perche', in \
             «## Cosa cambia»",
            versione - 1
        ));
    }
    if errori.is_empty() {
        Ok((p, note))
    } else {
        Err(errori)
    }
}

// ---------------------------------------------------------- scrivere il piano

fn chi_scritto(c: &Chi) -> String {
    match c {
        Chi::Reparto(r) => r.clone(),
        Chi::Assumi {
            ruolo,
            mestiere: Mestiere::Ricercatore,
        } => {
            format!("assumi ricercatore: {ruolo}")
        }
        Chi::Assumi { ruolo, .. } => format!("assumi: {ruolo}"),
    }
}

/// Il piano in forma pulita: e' quello che va su disco, e [`leggi`] lo
/// rilegge identico.
pub fn scrivi(p: &Piano) -> String {
    let mut t = format!("# Piano: {}\n\nVersione: {}\n", p.progetto, p.versione);
    t.push_str(&format!("Obiettivo: {}\n", p.obiettivo));
    if !p.cartella.is_empty() {
        t.push_str(&format!("Cartella: {}\n", p.cartella));
    }
    let lista = |t: &mut String, titolo: &str, voci: &[String]| {
        if !voci.is_empty() {
            t.push_str(&format!("\n## {titolo}\n"));
            for v in voci {
                t.push_str(&format!("- {v}\n"));
            }
        }
    };
    lista(&mut t, "Cosa cambia", &p.cambia);
    for f in &p.fasi {
        t.push_str(&format!(
            "\n## Fase {}: {}\nConsegna: {}\nFatta quando: {}\n",
            f.n, f.nome, f.consegna, f.fatta_quando
        ));
        for i in &f.incarichi {
            t.push_str(&format!("- {} [{}] {}", i.id, chi_scritto(&i.chi), i.cosa));
            if !i.dopo.is_empty() {
                t.push_str(&format!(" (dopo {})", i.dopo.join(", ")));
            }
            t.push('\n');
        }
    }
    lista(&mut t, "Rischi", &p.rischi);
    lista(&mut t, "Domande per te", &p.domande);
    t
}

/// `1 fase`, `3 fasi`.
pub fn quanti(n: usize, uno: &str, tanti: &str) -> String {
    format!("{n} {}", if n == 1 { uno } else { tanti })
}

/// Com'e' finito un piano, per la chat: quanto e' grande, e le domande per
/// l'utente, che deve rispondere.
pub fn esito(p: &Piano, file: &str) -> String {
    let mut t = format!(
        "Il piano di «{}», versione {}: {}, {}",
        p.progetto,
        p.versione,
        quanti(p.fasi.len(), "fase", "fasi"),
        quanti(p.incarichi().count(), "compito", "compiti"),
    );
    match p.da_assumere() {
        0 => t.push_str(", tutti ai reparti."),
        1 => t.push_str(", 1 per chi manca: lo trova AR."),
        n => t.push_str(&format!(", {n} per chi manca: li trova AR.")),
    }
    if !p.domande.is_empty() {
        t.push_str("\nDomande per te:");
        for d in &p.domande {
            t.push_str(&format!("\n- {d}"));
        }
    }
    t.push_str(&format!("\nIl piano: {file}"));
    t
}

// ------------------------------------------------------------- su disco

/// La cartella dei piani di un progetto: `nova` e' la cartella di NOVA.
pub fn cartella_del_progetto(nova: &Path, progetto: &str) -> PathBuf {
    crate::progetto::cartella(nova, progetto).join(CARTELLA)
}

/// Il file di una versione.
pub fn file(cartella: &Path, versione: u32) -> PathBuf {
    cartella.join(format!("piano-{versione}.md"))
}

/// Le versioni che ci sono, in ordine.
pub fn versioni(cartella: &Path) -> Vec<u32> {
    let mut v: Vec<u32> = std::fs::read_dir(cartella)
        .map(|it| {
            it.filter_map(|e| e.ok())
                .filter_map(|e| {
                    let n = e.file_name().to_string_lossy().to_string();
                    n.strip_prefix("piano-")?.strip_suffix(".md")?.parse().ok()
                })
                .filter(|n| *n > 0)
                .collect()
        })
        .unwrap_or_default();
    v.sort_unstable();
    v
}

/// Scrive una versione nuova del piano. Una versione che c'e' gia' non si
/// riscrive: ogni versione resta.
pub fn salva(cartella: &Path, p: &Piano) -> Result<PathBuf, String> {
    std::fs::create_dir_all(cartella).map_err(|e| format!("{}: {e}", cartella.display()))?;
    let f = file(cartella, p.versione);
    if f.exists() {
        return Err(format!(
            "la versione {} del piano c'e' gia': non si riscrive",
            p.versione
        ));
    }
    crate::scrivi_intero(&f, &scrivi(p))?;
    Ok(f)
}

#[cfg(test)]
mod prove {
    use super::*;

    const BUONO: &str = "Ecco il piano.\n\n```markdown\n# Piano: spazi-compressi\n\
        **Obiettivo:** un codificatore che allarga il contesto\n\n\
        ## Fase 1: Stato dell'arte\n\
        Consegna: un rapporto con le fonti\n\
        Fatta quando: la revisione lo approva\n\
        - 1.1 [ricerca] cercare i lavori sulla compressione\n\
        - 1.2 [revisione] rivedere il rapporto (dopo 1.1)\n\
        - 1.3 [commerciale] chi altro lo fa\n\n\
        ## Fase 2 - Prototipo\n\
        Consegna: il codificatore\n\
        Fatta quando: le prove passano\n\
        - 2.1 [assumi: programmatore Rust, compressione] scrivere il codificatore (dopo 1.2, 1.3)\n\
        - 2.2 [Assumi Ricercatore: esperto di Fourier] le trasformate (dopo 1.1)\n\
        - 2.3 [qualita] provarlo (dopo 2.1 e 2.2)\n\n\
        ## Rischi\n- non si comprime abbastanza\n\n\
        ## Domande per te\n* quanto puoi spendere?\n```\n";

    #[test]
    fn un_piano_buono_si_legge_tutto() {
        let (p, note) = leggi(BUONO, 1).unwrap();
        assert_eq!(p.progetto, "spazi-compressi");
        assert_eq!(p.obiettivo, "un codificatore che allarga il contesto");
        assert_eq!(p.fasi.len(), 2);
        assert_eq!(p.fasi[1].nome, "Prototipo");
        assert_eq!(p.incarichi().count(), 6);
        assert_eq!(p.da_assumere(), 2);
        let f2 = &p.fasi[1].incarichi;
        assert_eq!(
            f2[0].chi,
            Chi::Assumi {
                ruolo: "programmatore Rust, compressione".into(),
                mestiere: Mestiere::Generico
            }
        );
        assert_eq!(
            f2[1].chi,
            Chi::Assumi {
                ruolo: "esperto di Fourier".into(),
                mestiere: Mestiere::Ricercatore
            }
        );
        assert_eq!(f2[0].dopo, ["1.2", "1.3"]);
        assert_eq!(f2[2].dopo, ["2.1", "2.2"], "anche con la «e»");
        assert_eq!(f2[2].chi, Chi::Reparto("qualita".into()));
        assert_eq!(p.rischi, ["non si comprime abbastanza"]);
        assert_eq!(p.domande, ["quanto puoi spendere?"]);
        assert_eq!(note, ["c'era del testo prima del piano: e' rimasto fuori"]);
    }

    #[test]
    fn la_forma_pulita_si_rilegge_identica() {
        let (mut p, _) = leggi(BUONO, 1).unwrap();
        p.cartella = "C:\\progetti\\spazi".into();
        let scritto = scrivi(&p);
        let (di_nuovo, note) = leggi(&scritto, 1).unwrap();
        assert_eq!(di_nuovo, p);
        assert!(note.is_empty(), "{note:?}");
        assert!(
            scritto.starts_with("# Piano: spazi-compressi\n\nVersione: 1\n"),
            "{scritto}"
        );
        assert!(scritto.contains(
            "- 2.2 [assumi ricercatore: esperto di Fourier] le trasformate (dopo 1.1)\n"
        ));
    }

    #[test]
    fn gli_errori_arrivano_tutti_insieme() {
        let t = "# Piano: x\n## Fase 2: a\n- 1.1 [legale] guardare le norme\n\
                 - 2.1 [marketing] vendere (dopo 9.9)\n- 2.1 [assumi] boh\n- xx [ricerca] niente\n";
        let e = leggi(t, 1).unwrap_err();
        let tutto = e.join("\n");
        for atteso in [
            "manca l'obiettivo",
            "qui c'e' la fase 2 dove va la 1",
            "la fase 2 non dice cosa produce",
            "la fase 2 non dice come si sa che e' finita",
            "«legale»: il legale non prende compiti",
            "«marketing» non e' un reparto: i reparti sono commerciale, ricerca",
            "per chi manca si scrive «assumi: <ruolo>»",
            "un compito comincia col suo id",
        ] {
            assert!(tutto.contains(atteso), "manca «{atteso}» in:\n{tutto}");
        }
        assert!(leggi("niente di niente", 1)
            .unwrap_err()
            .iter()
            .any(|e| e.contains("manca il titolo")));
    }

    #[test]
    fn le_dipendenze_si_controllano() {
        let base = "# Piano: x\nObiettivo: o\n## Fase 1: a\nConsegna: c\nFatta quando: f\n";
        let e = leggi(&format!("{base}- 1.1 [ricerca] a (dopo 1.1)\n"), 1).unwrap_err();
        assert!(e[0].contains("non puo' venire dopo se stesso"), "{e:?}");
        let e = leggi(&format!("{base}- 1.1 [ricerca] a (dopo 1.3)\n"), 1).unwrap_err();
        assert!(e[0].contains("viene dopo 1.3, che non c'e'"), "{e:?}");
        let e = leggi(
            &format!("{base}- 1.1 [ricerca] a (dopo 1.3)\n- 1.2 [dati] b (dopo 1.1)\n- 1.3 [dati] c (dopo 1.2)\n"),
            1,
        )
        .unwrap_err();
        assert_eq!(e, ["le dipendenze fanno un giro: 1.1 → 1.3 → 1.2 → 1.1"]);
        let e = leggi(
            &format!("{base}- 2.1 [ricerca] a\n- 1.2 [dati] b\n- 1.2 [dati] c\n"),
            1,
        )
        .unwrap_err();
        assert!(
            e.iter()
                .any(|x| x.contains("il compito 2.1 sta nella fase 1")),
            "{e:?}"
        );
        assert!(
            e.iter()
                .any(|x| x.contains("il compito 1.2 c'e' due volte")),
            "{e:?}"
        );
    }

    #[test]
    fn dalla_seconda_versione_si_dice_cosa_cambia() {
        let e = leggi(BUONO, 2).unwrap_err();
        assert_eq!(e.len(), 1);
        assert!(
            e[0].contains("e' la versione 2: scrivi cosa cambia rispetto alla 1"),
            "{e:?}"
        );
        let con = BUONO.replace(
            "## Fase 1",
            "## Cosa cambia\n- la fase 2 chiede un ricercatore\n\n## Fase 1",
        );
        let (p, _) = leggi(&con, 2).unwrap();
        assert_eq!(p.versione, 2);
        assert_eq!(p.cambia, ["la fase 2 chiede un ricercatore"]);
    }

    #[test]
    fn la_versione_scritta_dal_cervello_non_conta_e_le_sezioni_estranee_si_dicono() {
        let t = BUONO
            .replace("**Obiettivo:**", "Versione: 7\nObiettivo:")
            .replace("## Rischi", "## Budget\n- 100 euro\n\n## Rischi");
        let (p, note) = leggi(&t, 1).unwrap();
        assert_eq!(p.versione, 1);
        assert!(
            note.iter()
                .any(|n| n == "la sezione «Budget» non fa parte del piano: e' rimasta fuori"),
            "{note:?}"
        );
    }

    #[test]
    fn la_richiesta_va_e_torna_dal_compito() {
        for r in [
            Richiesta {
                progetto: "spazi".into(),
                cartella: "C:\\a b\\c".into(),
                perche: Perche::Utente,
                testo: "fai il piano\nsu due righe".into(),
            },
            Richiesta {
                progetto: "spazi".into(),
                cartella: String::new(),
                perche: Perche::Fase(2),
                testo: "la revisione l'ha bocciata: mancano le fonti".into(),
            },
        ] {
            let t = r.nel_compito();
            assert_eq!(Richiesta::dal_compito(&t).unwrap(), r, "{t}");
        }
        let t = Richiesta {
            progetto: "p".into(),
            cartella: String::new(),
            perche: Perche::Fase(3),
            testo: "x".into(),
        }
        .nel_compito();
        assert_eq!(
            t,
            "Rivedere il piano di «p»: la fase 3 non e' andata\nprogetto: p\nfase: 3\n\nx"
        );
        assert!(
            Richiesta::dal_compito("cerca le fonti").is_err(),
            "un compito qualunque non e' una richiesta"
        );
        assert!(Richiesta::dal_compito("t\nprogetto: P\n\nx").is_err());
        assert!(Richiesta::dal_compito("t\nprogetto: p\n\n").is_err());
        assert!(Richiesta::dal_compito("t\nprogetto: p\nfase: 0\n\nx").is_err());
    }

    fn compito(id: u64, stato: Stato, r: &Richiesta) -> Compito {
        Compito {
            id,
            testo: r.nel_compito(),
            da: "nova".into(),
            stato,
            affidato: String::new(),
            iniziato: String::new(),
            finito: String::new(),
            esito: String::new(),
            riprese: 0,
            padre: None,
            attende: Vec::new(),
            attese: 0,
            cervello: String::new(),
        }
    }

    #[test]
    fn le_revisioni_automatiche_si_contano_fino_a_quella_dell_utente() {
        let r = |perche| Richiesta {
            progetto: "p".into(),
            cartella: String::new(),
            perche,
            testo: "x".into(),
        };
        let altro = Richiesta {
            progetto: "q".into(),
            ..r(Perche::Fase(2))
        };
        let mut coda = vec![
            compito(1, Stato::Fatto, &r(Perche::Utente)),
            compito(2, Stato::Fatto, &r(Perche::Fase(2))),
            compito(3, Stato::Fallito, &r(Perche::Fase(2))),
            compito(4, Stato::Affidato, &r(Perche::Fase(2))),
            compito(5, Stato::Fatto, &r(Perche::Fase(1))),
            compito(6, Stato::Fatto, &altro),
        ];
        assert_eq!(
            revisioni_automatiche(&coda, "p", 2),
            2,
            "la fallita non conta, quella in coda si'"
        );
        assert_eq!(revisioni_automatiche(&coda, "p", 1), 1);
        assert_eq!(revisioni_automatiche(&coda, "q", 2), 1);
        coda.push(compito(7, Stato::Fallito, &r(Perche::Utente)));
        assert_eq!(
            revisioni_automatiche(&coda, "p", 2),
            2,
            "una revisione dell'utente fallita non azzera"
        );
        coda.push(compito(8, Stato::Fatto, &r(Perche::Utente)));
        assert_eq!(
            revisioni_automatiche(&coda, "p", 2),
            0,
            "quella dell'utente fatta si'"
        );
    }

    #[test]
    fn la_domanda_dice_cosa_serve() {
        let r = Richiesta {
            progetto: "p".into(),
            cartella: "/tmp/p".into(),
            perche: Perche::Utente,
            testo: "un compressore".into(),
        };
        let d = domanda(&r, 1, None);
        assert!(d.starts_with("[piano di sviluppo] Progetto «p», versione 1.\nI file del progetto sono in «/tmp/p»"), "{d}");
        assert!(
            d.contains("La richiesta:\nun compressore") && !d.contains("Cosa cambia"),
            "{d}"
        );
        assert!(
            !d.contains("[piano]"),
            "il segno del piano del ricercatore e' un altro: {d}"
        );
        let d = domanda(
            &Richiesta {
                perche: Perche::Fase(2),
                cartella: String::new(),
                ..r
            },
            3,
            Some("# Piano: p"),
        );
        assert!(
            d.contains("non ha una cartella") && d.contains("La fase 2 non e' andata"),
            "{d}"
        );
        assert!(
            d.contains("Il piano di adesso, versione 2:\n# Piano: p")
                && d.contains("rispetto alla versione 2"),
            "{d}"
        );
    }

    #[test]
    fn il_prompt_nomina_tutti_i_reparti_e_solo_quelli() {
        let p = prompt();
        assert!(p.starts_with(azienda::PROMPT_ARCHITETTO), "{p}");
        let nomi: Vec<&str> = reparti().iter().map(|x| x.nome).collect();
        assert_eq!(
            nomi,
            [
                "commerciale",
                "ricerca",
                "revisione",
                "scrittura",
                "dati",
                "qualita",
                "amministrazione"
            ]
        );
        assert!(p.contains(&format!("({})", nomi.join(", "))), "{p}");
        for x in &azienda::POSTI {
            assert_eq!(
                leggi_chi(x.nome).is_ok(),
                x.mestiere.prende_compiti(),
                "{}",
                x.nome
            );
        }
    }

    #[test]
    fn i_nomi_dei_progetti_sono_nomi_di_cartella() {
        assert_eq!(
            nome_del_progetto(" spazi-compressi ").as_deref(),
            Ok("spazi-compressi")
        );
        for no in [
            "",
            "Spazi",
            "a b",
            "../x",
            "-a",
            &"a".repeat(PROGETTO_MASSIMO + 1),
        ] {
            assert!(nome_del_progetto(no).is_err(), "{no}");
        }
    }

    #[test]
    fn le_versioni_restano() {
        let d = std::env::temp_dir().join(format!("nova-piano-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        assert!(versioni(&d).is_empty());
        let (mut p, _) = leggi(BUONO, 1).unwrap();
        let f = salva(&d, &p).unwrap();
        assert_eq!(f, file(&d, 1));
        assert!(salva(&d, &p).unwrap_err().contains("non si riscrive"));
        p.versione = 2;
        p.cambia = vec!["niente".into()];
        salva(&d, &p).unwrap();
        std::fs::write(d.join("piano-x.md"), "").unwrap();
        assert_eq!(versioni(&d), [1, 2]);
        let (letto, _) = leggi(&std::fs::read_to_string(file(&d, 2)).unwrap(), 2).unwrap();
        assert_eq!(letto, p);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn l_esito_dice_quanto_e_grande_e_le_domande() {
        let (p, _) = leggi(BUONO, 1).unwrap();
        assert_eq!(
            esito(&p, "/x/piano-1.md"),
            "Il piano di «spazi-compressi», versione 1: 2 fasi, 6 compiti, 2 per chi manca: li trova AR.\n\
             Domande per te:\n- quanto puoi spendere?\nIl piano: /x/piano-1.md"
        );
        assert_eq!(quanti(1, "fase", "fasi"), "1 fase");
    }

    #[test]
    fn gli_strumenti_sono_in_ordine_e_leggono_e_basta() {
        let mut ordinati = STRUMENTI_DELL_ARCHITETTO;
        ordinati.sort_unstable();
        assert_eq!(ordinati, STRUMENTI_DELL_ARCHITETTO);
        for s in STRUMENTI_DELL_ARCHITETTO {
            assert!(
                !["write", "edit", "delete", "move", "copy", "mkdir", "affida", "scrivi", "nota"]
                    .iter()
                    .any(|v| s.contains(v)),
                "{s}"
            );
        }
    }
}
