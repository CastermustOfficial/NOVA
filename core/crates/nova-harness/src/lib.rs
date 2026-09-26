//! L'harness: il documento fatto a pezzi, e la ricerca che torna una
//! **posizione** invece di un'affermazione.
//!
//! La chat e' dove si parla; l'harness e' dove il lavoro sta. Qui c'e' la
//! meta' che non si vede: come un documento si divide in blocchi, quali file
//! di un progetto si guardano, e come si decide quale blocco risponde a una
//! domanda.
//!
//! **Perche' la posizione e' la funzione.** Un assistente allo studio che
//! dice «lo trovi a pagina 12, terzo blocco» non puo' bluffare: o quel blocco
//! contiene quella cosa o non la contiene. E' la stessa medicina del
//! fascicolo — i fatti vengono da un posto reale — applicata alla lettura
//! invece che alla scrittura. Da questo discende tutto il resto: i blocchi
//! esistono perche' servono a **indicare**, e un modo di dividere un
//! documento che non produce punti indicabili non serve a niente.
//!
//! Qui dentro non si apre nessun file. Un `.docx` e un `.pdf` si aprono con
//! le librerie scelte misurando (D237, D238), e quella e' un'altra cosa: il
//! taglio in blocchi e' una **decisione**, aprire un file e' un'operazione.
//! Tenerle separate e' quel che permette di provare la decisione senza avere
//! il file, e di cambiare libreria senza ridiscutere il taglio.

use serde_json::Value;

/// Quanto e' grande un file prima che non valga la pena aprirlo.
pub const FILE_MAX: u64 = 400_000;

/// Quanti file di un progetto entrano nell'albero.
pub const ALBERO_MAX: usize = 600;

/// Le estensioni che si tagliano **per righe** perche' sono codice.
///
/// Il codice non ha righe vuote dove finisce il senso: un HTML scritto
/// stretto sarebbe un blocco solo, e l'unica modifica proponibile sarebbe
/// «riscrivi tutto il file», cioe' nessuna modifica proponibile. Una riga per
/// blocco e' anche l'unita' con cui si legge un errore: file, riga.
pub const CODICE: [&str; 32] = [
    ".py",
    ".js",
    ".mjs",
    ".ts",
    ".jsx",
    ".tsx",
    ".css",
    ".json",
    ".yml",
    ".yaml",
    ".toml",
    ".ini",
    ".cfg",
    ".rs",
    ".go",
    ".java",
    ".c",
    ".h",
    ".cpp",
    ".hpp",
    ".cs",
    ".rb",
    ".php",
    ".sh",
    ".bat",
    ".ps1",
    ".sql",
    ".xml",
    ".svg",
    ".vue",
    ".svelte",
    ".gitignore",
];

/// Le estensioni che si tagliano per righe oltre al codice.
///
/// L'HTML non e' codice da eseguire ma si taglia per righe per la stessa
/// ragione: scritto stretto sarebbe un blocco solo.
pub const A_RIGHE_IN_PIU: [&str; 2] = [".html", ".htm"];

/// I documenti che non sono codice.
pub const DOCUMENTI: [&str; 4] = [".pdf", ".docx", ".txt", ".md"];

/// Cartelle che in un progetto non si guardano: sono il prodotto, non il
/// lavoro, e riempirebbero l'albero di roba che nessuno apre.
pub const NON_GUARDARE: [&str; 14] = [
    ".git",
    "node_modules",
    "__pycache__",
    ".venv",
    "venv",
    "dist",
    "build",
    ".mypy_cache",
    ".pytest_cache",
    ".idea",
    ".vscode",
    "target",
    ".next",
    "site-packages",
];

/// Da cosa si parte guardando un progetto: prima quello che si guarda, poi
/// quello che si legge, poi quello che si esegue.
pub const PRIMI: [&str; 9] = [
    "index.html",
    "README.md",
    "readme.md",
    "main.py",
    "app.py",
    "index.js",
    "main.js",
    "src/index.html",
    "src/main.py",
];

/// File senza estensione che si guardano lo stesso.
pub const SENZA_ESTENSIONE: [&str; 1] = ["Makefile"];

/// Come si taglia un file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Taglio {
    /// Una riga per blocco: e' codice.
    Righe,
    /// Un paragrafo per blocco, separati da righe vuote.
    Paragrafi,
    /// I paragrafi e le righe di tabella di un `.docx`.
    Docx,
    /// I blocchi di una pagina, col riquadro in cui stanno.
    Pdf,
    /// Non lo so aprire.
    Nessuno,
}

fn minuscola(s: &str) -> String {
    s.to_lowercase()
}

/// L'estensione di un nome di file, col punto, in minuscolo.
///
/// Un nome che **comincia** per punto non ha estensione: `.gitignore` si
/// chiama cosi', non e' un file senza nome con estensione `gitignore`.
pub fn estensione(nome: &str) -> String {
    let solo = nome.rsplit(['/', '\\']).next().unwrap_or(nome);
    // `Some(0)` e `None` finiscono tutti e due qui, ed e' voluto: un nome
    // che comincia per punto e' tutto estensione — `.gitignore` si chiama
    // cosi'. Scriverlo come un caso a parte sarebbe stato piu' esplicito e
    // non avrebbe cambiato niente: una mutazione che lo toglie sopravvive,
    // perche' `&solo[0..]` e' `solo`.
    match solo.rfind('.') {
        Some(i) if i > 0 => minuscola(&solo[i..]),
        _ => minuscola(solo),
    }
}

/// Come si taglia questo file.
pub fn taglio_di(nome: &str) -> Taglio {
    let est = estensione(nome);
    if est == ".docx" {
        return Taglio::Docx;
    }
    if est == ".pdf" {
        return Taglio::Pdf;
    }
    if CODICE.contains(&est.as_str()) || A_RIGHE_IN_PIU.contains(&est.as_str()) {
        return Taglio::Righe;
    }
    if est == ".txt" || est == ".md" {
        return Taglio::Paragrafi;
    }
    Taglio::Nessuno
}

/// Se questo file si puo' aprire affatto.
pub fn si_apre(nome: &str) -> bool {
    taglio_di(nome) != Taglio::Nessuno
        || SENZA_ESTENSIONE.contains(&nome.rsplit(['/', '\\']).next().unwrap_or(nome))
}

/// Cosa si dice a chi chiede un file che non si sa aprire.
pub fn non_so_aprire(nome: &str) -> String {
    let est = estensione(nome);
    let mut tutte: Vec<String> = DOCUMENTI
        .iter()
        .chain(CODICE.iter())
        .chain(A_RIGHE_IN_PIU.iter())
        .map(|x| x.to_string())
        .collect();
    tutte.sort();
    tutte.dedup();
    format!(
        "non so aprire un {}. So aprire: {}",
        if est.starts_with('.') && est.len() > 1 {
            est
        } else {
            "file senza estensione".to_string()
        },
        tutte.join(", ")
    )
}

/// Un pezzo di documento che si puo' **indicare**.
#[derive(Debug, Clone, PartialEq)]
pub struct Blocco {
    /// Come si chiama questo punto: `r12`, `p3b1`, `t0r2`.
    pub id: String,
    /// La pagina, dove esiste una pagina.
    pub pagina: Option<u32>,
    pub testo: String,
    pub stile: String,
    /// Dove sta nella pagina: senza questo non si puo' evidenziare, e senza
    /// evidenziare il profilo «studio» non ha ragione di esistere (D238).
    pub riquadro: Option<[f64; 4]>,
    /// Quante righe del file occupa, dove il file ha righe.
    pub righe: Option<u32>,
}

impl Blocco {
    fn nudo(id: String, testo: String) -> Blocco {
        Blocco {
            id,
            pagina: None,
            testo,
            stile: String::new(),
            riquadro: None,
            righe: None,
        }
    }
}

/// Il codice, una riga per blocco.
///
/// Le righe vuote non diventano blocchi — non c'e' niente da indicare — ma
/// il numero di riga resta quello vero: `r12` e' la dodicesima riga del file
/// contando da zero, anche se prima ce n'erano tre vuote. Rinumerare
/// vorrebbe dire che l'errore di un compilatore e il blocco dell'harness
/// parlano di due righe diverse.
pub fn per_righe(contenuto: &str) -> Vec<Blocco> {
    contenuto
        .lines()
        .enumerate()
        .filter(|(_, r)| !r.trim().is_empty())
        .map(|(i, r)| Blocco {
            righe: Some(1),
            ..Blocco::nudo(format!("r{i}"), r.trim_end().to_string())
        })
        .collect()
}

/// Il testo, un paragrafo per blocco.
///
/// Un paragrafo finisce dove c'e' una riga vuota. L'id e' la riga in cui il
/// paragrafo **comincia**, per la stessa ragione di sopra.
pub fn per_paragrafi(contenuto: &str) -> Vec<Blocco> {
    let righe: Vec<&str> = contenuto.lines().collect();
    let mut fuori = Vec::new();
    let mut raccolto: Vec<String> = Vec::new();
    let mut inizio = 0usize;
    for (i, r) in righe.iter().enumerate() {
        if !r.trim().is_empty() {
            if raccolto.is_empty() {
                inizio = i;
            }
            raccolto.push(r.trim().to_string());
        } else if !raccolto.is_empty() {
            fuori.push(Blocco {
                righe: Some((i - inizio) as u32),
                ..Blocco::nudo(format!("r{inizio}"), raccolto.join(" "))
            });
            raccolto.clear();
        }
    }
    if !raccolto.is_empty() {
        fuori.push(Blocco {
            righe: Some((righe.len() - inizio) as u32),
            ..Blocco::nudo(format!("r{inizio}"), raccolto.join(" "))
        });
    }
    fuori
}

/// Un paragrafo di `.docx`, come lo da' chi apre il file.
#[derive(Debug, Clone)]
pub struct Paragrafo {
    pub testo: String,
    pub stile: String,
}

/// Il `.docx`: prima i paragrafi, poi le righe delle tabelle.
///
/// Le tabelle **non** si saltano, e non e' un dettaglio: ignorarle vuol dire
/// leggere una fattura senza gli importi. Una riga di tabella e' un blocco,
/// con le celle separate da `|`.
pub fn per_docx(paragrafi: &[Paragrafo], tabelle: &[Vec<Vec<String>>]) -> Vec<Blocco> {
    let mut fuori = Vec::new();
    for (i, p) in paragrafi.iter().enumerate() {
        let t = p.testo.trim();
        if t.is_empty() {
            continue;
        }
        fuori.push(Blocco {
            stile: p.stile.clone(),
            ..Blocco::nudo(format!("p{i}"), t.to_string())
        });
    }
    for (ti, tab) in tabelle.iter().enumerate() {
        for (ri, riga) in tab.iter().enumerate() {
            let celle: Vec<String> = riga.iter().map(|c| c.trim().to_string()).collect();
            let t = celle.join(" | ");
            let t = t.trim_matches(|c: char| c == ' ' || c == '|');
            if t.is_empty() {
                continue;
            }
            fuori.push(Blocco {
                stile: "Tabella".to_string(),
                ..Blocco::nudo(format!("t{ti}r{ri}"), t.to_string())
            });
        }
    }
    fuori
}

/// Un blocco di PDF come lo da' chi apre il file: dove sta, e cosa dice.
#[derive(Debug, Clone)]
pub struct PezzoPdf {
    /// La pagina, contando da zero.
    pub pagina: u32,
    /// Il numero del blocco dentro la pagina.
    pub numero: u32,
    pub riquadro: [f64; 4],
    pub testo: String,
}

/// Il PDF: un blocco per pezzo, con il riquadro in cui sta.
///
/// Gli spazi si schiacciano: un PDF manda a capo dove finisce la riga
/// tipografica, non dove finisce la frase, e un testo pieno di ritorni a
/// capo non si cerca.
pub fn per_pdf(pezzi: &[PezzoPdf]) -> Vec<Blocco> {
    pezzi
        .iter()
        .filter_map(|p| {
            let testo = schiaccia(&p.testo);
            if testo.is_empty() {
                return None;
            }
            Some(Blocco {
                pagina: Some(p.pagina + 1),
                riquadro: Some([
                    arrotonda(p.riquadro[0]),
                    arrotonda(p.riquadro[1]),
                    arrotonda(p.riquadro[2]),
                    arrotonda(p.riquadro[3]),
                ]),
                ..Blocco::nudo(format!("p{}b{}", p.pagina, p.numero), testo)
            })
        })
        .collect()
}

/// Gli spazi bianchi di fila diventano uno solo, e i capi si tolgono.
pub fn schiaccia(testo: &str) -> String {
    testo.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Un decimale: il riquadro si confronta e si stampa, e due cifre in piu'
/// non aiutano nessuno a trovare un rettangolo su una pagina.
fn arrotonda(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

// ---------------------------------------------------------- il progetto

/// Se questo percorso relativo passa il filtro delle cartelle.
pub fn si_guarda(relativo: &str) -> bool {
    relativo
        .split(['/', '\\'])
        .all(|parte| !NON_GUARDARE.contains(&parte))
}

/// Un file del progetto, come lo vede chi guarda il disco.
#[derive(Debug, Clone)]
pub struct SulDisco {
    /// Relativo alla radice, con le barre in avanti.
    pub dove: String,
    pub byte: u64,
}

/// L'albero: i file del progetto, in ordine, senza quelli che nessuno apre.
///
/// Si ferma a `ALBERO_MAX` **dopo** aver filtrato, non prima: fermarsi prima
/// vorrebbe dire che una cartella `node_modules` da diecimila file mangia
/// tutto l'albero e il progetto vero non compare.
pub fn albero(file: &[SulDisco]) -> Vec<String> {
    let mut dentro: Vec<String> = file
        .iter()
        .filter(|f| si_guarda(&f.dove))
        .filter(|f| si_apre(&f.dove))
        .filter(|f| f.byte <= FILE_MAX)
        .map(|f| f.dove.replace('\\', "/"))
        .collect();
    dentro.sort();
    dentro.truncate(ALBERO_MAX);
    dentro
}

/// Da quale file si parte, guardando un progetto.
pub fn da_dove_si_parte(albero: &[String]) -> Option<&String> {
    PRIMI
        .iter()
        .find_map(|p| albero.iter().find(|x| x.as_str() == *p))
        .or_else(|| albero.first())
}

// ----------------------------------------------------------- la ricerca

/// Le parole utili di una domanda.
///
/// E' la **stessa** funzione delle ricette, e non per risparmiare righe:
/// cosi' la tolleranza ai refusi vale anche qui, e chi cerca «Calhanoglu»
/// scritto storto lo trova lo stesso.
pub fn parole(testo: &str) -> Vec<String> {
    nova_ricette::parole(testo)
}

/// Quanto un blocco risponde alla domanda: da 0 a 1.
///
/// E' la frazione delle parole chieste che il blocco contiene, con la stessa
/// uguaglianza delle ricette — contenimento, radice, trigrammi.
pub fn punteggio(chieste: &[String], testo: &str) -> f64 {
    let chieste = senza_doppie(chieste);
    if chieste.is_empty() {
        return 0.0;
    }
    let dentro = senza_doppie(&parole(testo));
    if dentro.is_empty() {
        return 0.0;
    }
    let presi = chieste
        .iter()
        .filter(|p| dentro.iter().any(|q| nova_ricette::stessa_parola(p, q)))
        .count();
    presi as f64 / chieste.len() as f64
}

fn senza_doppie(v: &[String]) -> Vec<String> {
    let mut fuori: Vec<String> = Vec::new();
    for x in v {
        if !fuori.contains(x) {
            fuori.push(x.clone());
        }
    }
    fuori
}

/// Un blocco trovato, e quanto risponde.
#[derive(Debug, Clone, PartialEq)]
pub struct Trovato {
    pub id: String,
    pub pagina: Option<u32>,
    pub quanto: f64,
    pub testo: String,
}

/// Quanto testo di un blocco trovato si riporta.
pub const QUANTO_SI_MOSTRA: usize = 300;

/// Dove sta, nel documento, quello che si sta chiedendo.
///
/// L'ordinamento e' **stabile**: a parita' di punteggio vince chi viene
/// prima nel documento. Non e' un dettaglio di implementazione — e' cio' che
/// rende ripetibile una risposta, e una risposta che cambia ordine fra due
/// domande uguali fa dubitare di tutto il resto.
pub fn cerca(blocchi: &[Blocco], domanda: &str, quanti: usize) -> Vec<Trovato> {
    let chieste = parole(domanda);
    if chieste.is_empty() {
        return Vec::new();
    }
    let mut punteggi: Vec<(f64, &Blocco)> = blocchi
        .iter()
        .map(|b| (punteggio(&chieste, &b.testo), b))
        .filter(|(p, _)| *p > 0.0)
        .collect();
    punteggi.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    punteggi
        .into_iter()
        .take(quanti.max(1))
        .map(|(p, b)| Trovato {
            id: b.id.clone(),
            pagina: b.pagina,
            quanto: (p * 100.0).round() / 100.0,
            testo: primi(&b.testo, QUANTO_SI_MOSTRA),
        })
        .collect()
}

/// I primi caratteri di un testo. A **caratteri**, non a byte.
pub fn primi(testo: &str, quanti: usize) -> String {
    testo.chars().take(quanti).collect()
}

/// I blocchi intorno a un punto: il contesto in cui quella cosa sta.
///
/// Torna gli indici, non i blocchi: chi chiama ne ha bisogno per dire alla
/// finestra cosa accendere, e una copia dei blocchi non servirebbe a quello.
pub fn intorno(blocchi: &[Blocco], a: &str, quanti: usize) -> Result<(usize, usize), String> {
    if a.is_empty() {
        return Ok((0, quanti.max(1).min(blocchi.len())));
    }
    let i = blocchi
        .iter()
        .position(|b| b.id == a)
        .ok_or_else(|| format!("nel documento non c'e' nessun «{a}»"))?;
    let da = i.saturating_sub(quanti);
    let fino = (i + quanti + 1).min(blocchi.len());
    Ok((da, fino))
}

/// Come si scrive un blocco quando lo si riporta a chi legge.
pub fn riga_di(b: &Blocco) -> String {
    match b.pagina {
        Some(p) => format!("[{}, pagina {p}] {}", b.id, b.testo),
        None => format!("[{}] {}", b.id, b.testo),
    }
}

/// Il testo di un pezzo di documento, fermandosi a `caratteri`.
///
/// Si conta **prima** di aggiungere: un blocco che sfora non entra a meta'.
/// Un blocco troncato e' un blocco che dice una cosa che il documento non
/// dice, ed e' proprio il contrario di quel che serve qui.
pub fn fino_a(blocchi: &[Blocco], caratteri: usize) -> (String, usize) {
    let mut pezzi: Vec<String> = Vec::new();
    let mut quanti = 0usize;
    for b in blocchi {
        let pezzo = riga_di(b);
        let lungo = pezzo.chars().count();
        if quanti + lungo > caratteri {
            break;
        }
        quanti += lungo;
        pezzi.push(pezzo);
    }
    let n = pezzi.len();
    (pezzi.join("\n\n"), n)
}

/// I blocchi come JSON, per chi li salva o li manda alla finestra.
pub fn in_json(b: &Blocco) -> Value {
    serde_json::json!({
        "id": b.id,
        "pagina": b.pagina,
        "testo": b.testo,
        "stile": b.stile,
        "riquadro": b.riquadro.map(|r| r.to_vec()),
        "righe": b.righe,
    })
}

/// Le proposte di modifica: quel che si puo' chiedere a un blocco, e come
/// un documento diventa quel che sara'.
/// Cosa c'e' aperto nella finestra, detto al cervello.
pub mod aperti;

pub mod modifica;

/// Il verificatore: come si prova un progetto, e cosa vuol dire che una
/// modifica non lo ha peggiorato.
pub mod prova;

#[cfg(test)]
mod prove;
