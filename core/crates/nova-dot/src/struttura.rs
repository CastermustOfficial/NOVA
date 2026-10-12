//! La cartella di un progetto dei Dot (D407).
//!
//! Deciso con Gio il 12 ottobre, sulla bozza «Dots_Project, la disposizione»:
//!
//! - **dove**: `Documenti\Dots_Project\<progetto>\`, creata se non c'e'.
//!   Una cartella indicata da chi chiede il progetto vince, e ha la stessa
//!   struttura; i file che ci sono gia' non si toccano;
//! - **piani e cronologia stanno nel progetto**, non piu' nella cartella di
//!   NOVA: niente doppioni. In NOVA resta solo l'indice dei progetti
//!   ([`INDICE_PROGETTI`]), il nome e la cartella di ognuno;
//! - **le cartelle le decide l'APM**: una per reparto, una per compito (il
//!   livello atomico), e le cinque del metodo (`piano`, `errori`, `idea`,
//!   `test`, `analisi`), come quelle di NOVA;
//! - **separato e chiaro**: ogni cosa ha il suo posto e il suo documento;
//!   solo il paper, alla fine, mette insieme le parti, e l'indice dice a
//!   tutti dove sta cosa e cosa fa. Ognuno lo aggiorna con le sue consegne;
//! - **metodo scientifico**: chi misura (ricerca, dati, qualita') compila
//!   l'esperimento, con quello che serve a rifarlo.
//!
//! ```text
//! <progetto>\
//!   INDICE.md            dove sta cosa e cosa fa
//!   progetto.md          com'e' adesso
//!   progetto.jsonl       la cronologia dell'APM
//!   piano\ToDo.md        cosa c'e' da fare, spuntato col compito che l'ha chiuso
//!   piano\piano-<n>.md   le versioni del piano dell'Architetto
//!   errori\errori.md     idea\idee.md     test\test.md     analisi\fase-<n>.md
//!   prodotto\            quello che si costruisce, un albero solo
//!   reparti\<reparto>\<id>-<cosa>\   compito.md, esperimento.md, consegna.md, materiale\
//!   paper.md             resoconto.md
//! ```
//!
//! Qui ci sono i nomi, i testi e le regole; chi crea le cartelle e scrive i
//! file al momento giusto e' `nova_core::apm`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::piano::{Chi, Fase, Incarico, Piano};
use crate::progetto::Progetto;
use crate::Stato;

/// La cartella dei progetti, dentro Documenti.
pub const DOTS_PROJECT: &str = "Dots_Project";
/// L'indice dei progetti, nella cartella di NOVA: nome e cartella di ognuno.
pub const INDICE_PROGETTI: &str = "progetti.json";

pub const INDICE: &str = "INDICE.md";
pub const PIANO: &str = "piano";
pub const TODO: &str = "ToDo.md";
pub const ERRORI: &str = "errori";
pub const IDEA: &str = "idea";
pub const TEST: &str = "test";
pub const ANALISI: &str = "analisi";
pub const PRODOTTO: &str = "prodotto";
pub const REPARTI: &str = "reparti";
pub const MATERIALE: &str = "materiale";
pub const PAPER: &str = "paper.md";
pub const RESOCONTO: &str = "resoconto.md";

/// I file delle cinque cartelle del metodo che hanno un file solo.
pub const FILE_ERRORI: &str = "errori.md";
pub const FILE_IDEE: &str = "idee.md";
pub const FILE_TEST: &str = "test.md";

/// I reparti che misurano: i loro compiti hanno l'esperimento.
pub const CON_ESPERIMENTO: [&str; 3] = ["ricerca", "dati", "qualita"];

/// L'incarico del paper, che l'APM affida alla scrittura alla fine.
pub const INCARICO_PAPER: &str = "paper";
/// Il reparto che scrive il paper.
pub const REPARTO_PAPER: &str = "scrittura";

/// Quanto e' lungo, al piu', il nome della cartella di un compito dopo l'id.
const NOME_COMPITO: usize = 40;

/// Le sezioni di una consegna, in ordine. Le ultime quattro l'APM le porta
/// nelle cartelle del metodo e nell'indice.
pub const SEZIONI: [&str; 7] = [
    "Cosa ho fatto",
    "Prove e numeri",
    "Cosa non so",
    "Idee",
    "Errori",
    "Test",
    "Indice",
];

/// La cartella di un progetto quando chi lo chiede non ne indica una.
pub fn radice_predefinita(documenti: &Path, nome: &str) -> PathBuf {
    documenti.join(DOTS_PROJECT).join(nome)
}

/// Un pezzo di nome di cartella: minuscole senza accenti, cifre e trattini.
pub fn slug(testo: &str, massimo: usize) -> String {
    let mut fuori = String::new();
    for c in testo.trim().chars() {
        let c = match c {
            'à' | 'á' | 'â' | 'ä' | 'À' | 'Á' => 'a',
            'è' | 'é' | 'ê' | 'ë' | 'È' | 'É' => 'e',
            'ì' | 'í' | 'î' | 'ï' | 'Ì' | 'Í' => 'i',
            'ò' | 'ó' | 'ô' | 'ö' | 'Ò' | 'Ó' => 'o',
            'ù' | 'ú' | 'û' | 'ü' | 'Ù' | 'Ú' => 'u',
            c => c.to_ascii_lowercase(),
        };
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            fuori.push(c);
        } else if !fuori.ends_with('-') && !fuori.is_empty() {
            fuori.push('-');
        }
    }
    // Troppo lungo: si taglia a una parola intera, non a meta'.
    if fuori.len() > massimo {
        let taglio = &fuori[..massimo];
        fuori = match taglio.rfind('-') {
            Some(k) if fuori.as_bytes()[massimo] != b'-' && k > 0 => taglio[..k].to_string(),
            _ => taglio.to_string(),
        };
    }
    let fuori = fuori.trim_end_matches('-').to_string();
    if fuori.is_empty() {
        "senza-nome".into()
    } else {
        fuori
    }
}

/// Il reparto di chi fa un compito: il reparto, o per un ruolo assunto da
/// AR un reparto col nome del ruolo.
pub fn reparto_di(chi: &Chi) -> String {
    match chi {
        Chi::Reparto(r) => slug(r, 40),
        Chi::Assumi { ruolo, .. } => slug(ruolo, 40),
    }
}

/// Se i compiti di chi lo fa hanno l'esperimento.
pub fn con_esperimento(chi: &Chi) -> bool {
    matches!(chi, Chi::Reparto(r) if CON_ESPERIMENTO.contains(&r.as_str()))
}

/// La cartella di un compito, relativa alla radice del progetto.
pub fn cartella_del_compito(i: &Incarico) -> PathBuf {
    PathBuf::from(REPARTI)
        .join(reparto_di(&i.chi))
        .join(format!("{}-{}", i.id, slug(&i.cosa, NOME_COMPITO)))
}

/// La cartella del paper, relativa alla radice.
pub fn cartella_del_paper() -> PathBuf {
    PathBuf::from(REPARTI).join(REPARTO_PAPER).join(INCARICO_PAPER)
}

/// Un percorso relativo scritto sempre con `/`, come lo legge l'indice.
pub fn relativo(p: &Path) -> String {
    p.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// L'indice, com'e' appena nato il progetto: le cartelle fisse.
pub fn indice_iniziale(nome: &str) -> String {
    format!(
        "# Indice: {nome}\n\n\
         Dove sta cosa, e cosa fa. «Le cartelle» le scrive l'APM; «I file» li aggiungono i Dot con le \
         loro consegne (sezione `## Indice`), e l'APM con i compiti che affida.\n\n\
         ## Le cartelle\n\
         - `{INDICE}` — questo file\n\
         - `progetto.md` — com'e' il progetto adesso\n\
         - `progetto.jsonl` — la cronologia dell'APM, un passo per riga\n\
         - `{PIANO}/{TODO}` — cosa c'e' da fare, in ordine, spuntato col compito che l'ha chiuso\n\
         - `{PIANO}/piano-<n>.md` — le versioni del piano dell'Architetto\n\
         - `{ERRORI}/{FILE_ERRORI}` — ogni errore: cosa, come ce ne siamo accorti, cosa si e' corretto, la regola\n\
         - `{IDEA}/{FILE_IDEE}` — ogni miglioria: cosa, perche', cosa costerebbe\n\
         - `{TEST}/{FILE_TEST}` — ogni prova: cosa controlla, come si lancia, cosa serve\n\
         - `{ANALISI}/fase-<n>.md` — ogni fase confermata dalla revisione: su quali prove, con quali numeri\n\
         - `{PRODOTTO}/` — quello che si costruisce, un albero solo\n\
         - `{REPARTI}/<reparto>/<id>-<cosa>/` — un compito: `compito.md`, `esperimento.md` (ricerca, dati, \
         qualita'), `consegna.md`, `{MATERIALE}/`\n\
         - `{PAPER}` — alla fine, il paper che mette insieme le parti\n\
         - `{RESOCONTO}` — alla fine, il resoconto per l'utente\n\n\
         ## I file\n"
    )
}

fn intestazione(titolo: &str, cosa: &str) -> String {
    format!("# {titolo}\n\n{cosa}\n")
}

/// Le cartelle e i file fissi di un progetto. Crea solo quello che manca:
/// in una cartella indicata dall'utente i file che c'erano restano.
pub fn prepara(radice: &Path, nome: &str) -> Result<(), String> {
    let crea = |p: PathBuf| std::fs::create_dir_all(&p).map_err(|e| format!("{}: {e}", p.display()));
    for d in [PIANO, ERRORI, IDEA, TEST, ANALISI, PRODOTTO, REPARTI] {
        crea(radice.join(d))?;
    }
    let file: [(PathBuf, String); 4] = [
        (radice.join(INDICE), indice_iniziale(nome)),
        (
            radice.join(ERRORI).join(FILE_ERRORI),
            intestazione(
                &format!("Errori: {nome}"),
                "Ogni errore: cosa e' successo, come ce ne siamo accorti, cosa si e' corretto, la regola \
                 che ne viene. Non si cancella niente.",
            ),
        ),
        (
            radice.join(IDEA).join(FILE_IDEE),
            intestazione(
                &format!("Idee: {nome}"),
                "Ogni miglioria, anche solo teorica: cosa, perche', cosa costerebbe.",
            ),
        ),
        (
            radice.join(TEST).join(FILE_TEST),
            intestazione(
                &format!("Test: {nome}"),
                "Ogni prova: cosa controlla, come si lancia, cosa serve per farla girare.",
            ),
        ),
    ];
    for (p, testo) in file {
        if !p.exists() {
            crate::scrivi_intero(&p, &testo)?;
        }
    }
    Ok(())
}

/// `compito.md`: cosa si chiede, a chi, quando e' fatto.
pub fn compito_md(progetto: &str, f: &Fase, i: &Incarico, dot: &str, numero: u64) -> String {
    let dopo = if i.dopo.is_empty() {
        "niente".to_string()
    } else {
        i.dopo.join(", ")
    };
    format!(
        "# Compito {id}: {cosa}\n\n\
         - Progetto: {progetto}\n\
         - Fase: {n} «{nome}» (consegna della fase: {consegna}; fatta quando: {quando})\n\
         - Reparto: {reparto}\n\
         - Chi: {dot}, compito n. {numero}\n\
         - Dipende da: {dopo}\n",
        id = i.id,
        cosa = i.cosa,
        n = f.n,
        nome = f.nome,
        consegna = f.consegna,
        quando = f.fatta_quando,
        reparto = reparto_di(&i.chi),
    )
}

/// `esperimento.md`, da compilare: il metodo scientifico.
pub const ESPERIMENTO_MD: &str = "# Esperimento\n\n\
## Ipotesi\nCosa ci si aspetta, in una frase che si possa smentire.\n\n\
## Metodo\nCome si misura: dati, strumenti, condizioni, quante ripetizioni.\n\n\
## Misure\nI numeri, ognuno col file in `materiale/` da cui viene.\n\n\
## Risultato\nCosa dicono le misure, senza interpretarle.\n\n\
## Conclusione\nL'ipotesi regge o no, e con che limiti. Un numero senza misura si scrive «(non verificato)».\n";

/// Le regole per il Dot che fa un compito: dove lavora e come consegna.
pub fn regole(radice: &Path, cartella: &Path, esperimento: bool) -> String {
    let dentro = radice.join(cartella);
    let mut t = format!(
        "\n\nDove lavori: «{}». Li' c'e' `compito.md`; quello che ti serve a rifare il lavoro (fonti, \
         dati grezzi, script) va in `{MATERIALE}/`. Quello che costruisci per il progetto va in «{}». \
         L'indice di tutto e' «{}».",
        dentro.display(),
        radice.join(PRODOTTO).display(),
        radice.join(INDICE).display(),
    );
    if esperimento {
        t.push_str(
            " E' un compito che misura: compila `esperimento.md` (ipotesi, metodo, misure, risultato, \
             conclusione).",
        );
    }
    t.push_str(&format!(
        "\n\nCome consegni: rispondi con la consegna in Markdown, con queste sezioni in quest'ordine, \
         anche vuote: {}. In «Prove e numeri» ogni numero ha la misura da cui viene, in `{MATERIALE}/`; \
         un numero senza misura scrivilo con «(non verificato)». Nelle ultime quattro una voce per riga, \
         che comincia con «- »: in «Idee» cosa, perche', cosa costerebbe; in «Errori» cosa e' successo, \
         come te ne sei accorto, cosa hai corretto, la regola; in «Test» cosa controlla, come si lancia, \
         cosa serve; in «Indice» `percorso` — cosa fa, per ogni file che hai creato o cambiato, col \
         percorso dalla cartella del progetto.",
        SEZIONI.map(|s| format!("«## {s}»")).join(", ")
    ));
    t
}

/// Le voci delle sezioni di una consegna che l'APM porta altrove.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sezioni {
    pub idee: Vec<String>,
    pub errori: Vec<String>,
    pub test: Vec<String>,
    /// `(percorso, cosa fa)`.
    pub indice: Vec<(String, String)>,
}

/// Legge le sezioni «Idee», «Errori», «Test» e «Indice» di una consegna:
/// le righe che cominciano con «- », sotto il titolo `##` giusto. Il resto
/// della consegna resta dov'e'.
pub fn sezioni(consegna: &str) -> Sezioni {
    let mut s = Sezioni::default();
    let mut dove = "";
    for riga in consegna.lines() {
        let r = riga.trim();
        if let Some(titolo) = r.strip_prefix("## ") {
            dove = match titolo.trim().to_lowercase().as_str() {
                "idee" => "idee",
                "errori" => "errori",
                "test" => "test",
                "indice" => "indice",
                _ => "",
            };
            continue;
        }
        if r.starts_with("# ") {
            dove = "";
            continue;
        }
        let Some(voce) = r.strip_prefix("- ").map(str::trim).filter(|v| !v.is_empty()) else {
            continue;
        };
        match dove {
            "idee" => s.idee.push(voce.to_string()),
            "errori" => s.errori.push(voce.to_string()),
            "test" => s.test.push(voce.to_string()),
            "indice" => {
                if let Some(v) = voce_d_indice(voce) {
                    s.indice.push(v);
                }
            }
            _ => {}
        }
    }
    s
}

/// «`percorso` — cosa fa»: il percorso fra apici inversi, poi un trattino
/// (lungo o corto) o due punti. Un percorso che esce dal progetto no.
fn voce_d_indice(voce: &str) -> Option<(String, String)> {
    let resto = voce.strip_prefix('`')?;
    let fine = resto.find('`')?;
    let percorso = resto[..fine].trim().replace('\\', "/");
    let cosa = resto[fine + 1..]
        .trim()
        .trim_start_matches(['—', '–', '-', ':'])
        .trim();
    let fuori = percorso.is_empty()
        || percorso.starts_with('/')
        || percorso.contains(':')
        || percorso.split('/').any(|p| p == "..");
    (!fuori && !cosa.is_empty()).then(|| (percorso.trim_start_matches("./").to_string(), cosa.to_string()))
}

/// `consegna.md`: chi, com'e' finito, e la consegna com'e'.
pub fn consegna_md(i: &Incarico, dot: &str, numero: u64, stato: Stato, esito: &str) -> String {
    format!(
        "# Consegna {id}: {cosa}\n\n- Chi: {dot}, compito n. {numero}\n- Stato: {stato}\n\n{esito}\n",
        id = i.id,
        cosa = i.cosa,
        stato = crate::nome_stato(stato),
        esito = esito.trim(),
    )
}

/// Una voce per `errori.md`, `idee.md` o `test.md`: quando, da dove, cosa.
pub fn voce(quando: &str, da: &str, testo: &str) -> String {
    format!("- {quando} · {da} · {}", testo.trim())
}

/// L'indice con le voci nuove: una voce con lo stesso percorso si
/// sostituisce, le altre si aggiungono. «I file» resta in ordine di
/// percorso; il resto del file non si tocca.
pub fn indice_aggiornato(indice: &str, voci: &[(String, String)], chi: &str) -> String {
    let segno = "## I file";
    let (testa, coda) = match indice.find(segno) {
        Some(k) => (&indice[..k], &indice[k + segno.len()..]),
        None => (indice, ""),
    };
    let mut tutte: BTreeMap<String, String> = BTreeMap::new();
    for riga in coda.lines() {
        if let Some((p, cosa)) = riga.trim().strip_prefix("- ").and_then(voce_d_indice) {
            tutte.insert(p, cosa);
        }
    }
    for (p, cosa) in voci {
        let cosa = if chi.is_empty() {
            cosa.clone()
        } else {
            format!("{cosa} ({chi})")
        };
        tutte.insert(p.clone(), cosa);
    }
    let mut t = format!("{}{segno}\n", testa.trim_end_matches('\n').to_string() + "\n\n");
    for (p, cosa) in tutte {
        t.push_str(&format!("- `{p}` — {cosa}\n"));
    }
    t
}

/// `piano/ToDo.md`: i compiti in ordine, spuntati col compito che li ha
/// chiusi, e quello che resta dopo l'ultima fase.
pub fn todo(p: &Progetto, piano: &Piano) -> String {
    let mut t = format!(
        "# ToDo: {}\n\nPiano versione {}. Un compito si spunta quando e' fatto, col compito del Dot che \
         l'ha chiuso.\n",
        p.nome, piano.versione
    );
    for f in &piano.fasi {
        let stato = match p.fasi.get(&f.n) {
            Some(true) => "fatta",
            Some(false) => "non andata",
            None if f.incarichi.iter().any(|i| p.lavori.contains_key(&i.id)) => "in corso",
            None => "da fare",
        };
        t.push_str(&format!("\n## Fase {}: {} ({stato})\n", f.n, f.nome));
        for i in &f.incarichi {
            let cartella = relativo(&cartella_del_compito(i));
            let (segno, chi) = match p.lavori.get(&i.id) {
                Some(l) if l.stato == Some(Stato::Fatto) => {
                    ("x", format!(" — fatto da {}, compito n. {}", l.dot, l.compito))
                }
                Some(l) => match l.stato {
                    Some(s) => (" ", format!(" — {} ({}, compito n. {})", crate::nome_stato(s), l.dot, l.compito)),
                    None => (" ", format!(" — in corso ({}, compito n. {})", l.dot, l.compito)),
                },
                None => (" ", String::new()),
            };
            t.push_str(&format!(
                "- [{segno}] {} [{}] {} — `{cartella}/`{chi}\n",
                i.id,
                reparto_di(&i.chi),
                i.cosa
            ));
        }
    }
    let paper = p.lavori.get(INCARICO_PAPER);
    let fatto = |b: bool| if b { "x" } else { " " };
    t.push_str(&format!(
        "\n## Alla fine\n\
         - [{}] il paper ({REPARTO_PAPER}) — `{PAPER}`\n\
         - [{}] il legale, prima del rilascio\n\
         - [{}] il resoconto — `{RESOCONTO}`\n",
        fatto(paper.is_some_and(|l| l.stato == Some(Stato::Fatto))),
        fatto(p.legale_rilascio.is_some()),
        fatto(!p.resoconto.is_empty()),
    ));
    t
}

/// `analisi/fase-<n>.md`: la fase confermata dalla revisione, coi compiti,
/// le consegne e gli esperimenti.
pub fn analisi_fase(p: &Progetto, f: &Fase, quando: &str, perche: &str) -> String {
    let mut t = format!(
        "# Analisi: fase {} «{}»\n\n\
         Confermata dalla revisione il {quando}: {}\n\n\
         - Consegna: {}\n- Fatta quando: {}\n\n## I compiti\n",
        f.n,
        f.nome,
        perche.trim(),
        f.consegna,
        f.fatta_quando
    );
    let mut esperimenti = Vec::new();
    for i in &f.incarichi {
        let c = relativo(&cartella_del_compito(i));
        match p.lavori.get(&i.id) {
            Some(l) => t.push_str(&format!(
                "- {} ({}, compito n. {}): {} — `{c}/consegna.md`\n",
                i.id, l.dot, l.compito, i.cosa
            )),
            None => t.push_str(&format!("- {}: {} — non affidato\n", i.id, i.cosa)),
        }
        if con_esperimento(&i.chi) {
            esperimenti.push(format!("- `{c}/esperimento.md`"));
        }
    }
    if !esperimenti.is_empty() {
        t.push_str(&format!("\n## Gli esperimenti\n{}\n", esperimenti.join("\n")));
    }
    t
}

/// Il compito del paper per la scrittura.
pub fn testo_del_paper(nome: &str, radice: &Path) -> String {
    format!(
        "Progetto «{nome}»: tutte le fasi sono fatte. Scrivi il paper del progetto: mette insieme quello \
         che dicono «{analisi}», i reparti e «{prodotto}», e per trovare le cose usa «{indice}». Ogni \
         risultato cita il file da cui viene. Il paper e' la tua consegna: l'APM lo mette in «{paper}».",
        analisi = radice.join(ANALISI).display(),
        prodotto = radice.join(PRODOTTO).display(),
        indice = radice.join(INDICE).display(),
        paper = radice.join(PAPER).display(),
    )
}

// ------------------------------------------------- l'indice dei progetti

/// I progetti che NOVA conosce: nome e cartella. Un file che non si legge
/// e' un indice vuoto.
pub fn progetti(nova: &Path) -> BTreeMap<String, String> {
    std::fs::read_to_string(nova.join(INDICE_PROGETTI))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// Scrive nell'indice dei progetti dove sta un progetto.
pub fn registra(nova: &Path, nome: &str, radice: &Path) -> Result<(), String> {
    let _turno = REGISTRO.lock().unwrap_or_else(|e| e.into_inner());
    let mut tutti = progetti(nova);
    tutti.insert(nome.to_string(), radice.display().to_string());
    std::fs::create_dir_all(nova).map_err(|e| format!("{}: {e}", nova.display()))?;
    let t = serde_json::to_string_pretty(&tutti).map_err(|e| e.to_string())?;
    crate::scrivi_intero(&nova.join(INDICE_PROGETTI), &t)
}

/// Chi scrive l'indice dei progetti, uno alla volta.
static REGISTRO: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod prove {
    use super::*;
    use crate::Mestiere;

    fn incarico(id: &str, cosa: &str, chi: Chi) -> Incarico {
        Incarico { id: id.into(), cosa: cosa.into(), chi, dopo: Vec::new() }
    }

    #[test]
    fn la_cartella_di_un_compito_sta_nel_suo_reparto() {
        let i = incarico("1.2", "Misurare la compressione: token, costo e perdita!", Chi::Reparto("dati".into()));
        assert_eq!(relativo(&cartella_del_compito(&i)), "reparti/dati/1.2-misurare-la-compressione-token-costo-e");
        let a = incarico(
            "2.1",
            "Scrivere il codice",
            Chi::Assumi { ruolo: "Programmatore Rust".into(), mestiere: Mestiere::Generico },
        );
        assert_eq!(relativo(&cartella_del_compito(&a)), "reparti/programmatore-rust/2.1-scrivere-il-codice");
        assert!(con_esperimento(&i.chi) && !con_esperimento(&a.chi));
        assert!(!con_esperimento(&Chi::Reparto("scrittura".into())));
        assert_eq!(slug("  Qualità è già perché  ", 40), "qualita-e-gia-perche");
        assert_eq!(slug("!!!", 40), "senza-nome");
    }

    #[test]
    fn prepara_crea_quello_che_manca_e_non_tocca_il_resto() {
        let d = std::env::temp_dir().join(format!("nova-struttura-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join(TEST)).unwrap();
        std::fs::write(d.join(TEST).join(FILE_TEST), "mio").unwrap();
        prepara(&d, "s0ex1").unwrap();
        for c in [PIANO, ERRORI, IDEA, TEST, ANALISI, PRODOTTO, REPARTI] {
            assert!(d.join(c).is_dir(), "{c}");
        }
        assert_eq!(std::fs::read_to_string(d.join(TEST).join(FILE_TEST)).unwrap(), "mio");
        assert!(std::fs::read_to_string(d.join(INDICE)).unwrap().starts_with("# Indice: s0ex1"));
        assert!(std::fs::read_to_string(d.join(ERRORI).join(FILE_ERRORI)).unwrap().contains("Non si cancella"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn le_sezioni_della_consegna_si_leggono() {
        let c = "## Cosa ho fatto\n- niente da portare via\n## Idee\n- usare zstd — piu' veloce — un giorno\n\
                 ## Errori\n- ho misurato due volte lo stesso file\n## Test\n- `test_x.py` controlla i token\n\
                 ## Indice\n- `prodotto/comprimi.py` — comprime un testo\n- `../fuori.txt` — no\n\
                 - `C:/x` — no\n- senza percorso\n- `materiale\\dati.csv`: i numeri\n## Altro\n- no";
        let s = sezioni(c);
        assert_eq!(s.idee, ["usare zstd — piu' veloce — un giorno"]);
        assert_eq!(s.errori, ["ho misurato due volte lo stesso file"]);
        assert_eq!(s.test, ["`test_x.py` controlla i token"]);
        assert_eq!(
            s.indice,
            [
                ("prodotto/comprimi.py".to_string(), "comprime un testo".to_string()),
                ("materiale/dati.csv".to_string(), "i numeri".to_string())
            ]
        );
        assert_eq!(sezioni("Consegna di dati."), Sezioni::default());
    }

    #[test]
    fn l_indice_si_aggiorna_senza_doppioni() {
        let i = indice_iniziale("p");
        let i = indice_aggiornato(&i, &[("b.md".into(), "il b".into()), ("a.md".into(), "il a".into())], "dati");
        let i = indice_aggiornato(&i, &[("b.md".into(), "il b, nuovo".into())], "qualita");
        let file = &i[i.find("## I file").unwrap()..];
        assert_eq!(file, "## I file\n- `a.md` — il a (dati)\n- `b.md` — il b, nuovo (qualita)\n");
        assert!(i.contains("## Le cartelle") && i.matches("## I file").count() == 1);
    }

    #[test]
    fn le_regole_dicono_dove_e_come() {
        let r = regole(Path::new("/p"), &PathBuf::from("reparti/dati/1.1-x"), true);
        assert!(r.contains("esperimento.md") && r.contains("«## Indice»") && r.contains("(non verificato)"));
        assert!(!regole(Path::new("/p"), &PathBuf::from("reparti/scrittura/1.1-x"), false).contains("esperimento.md"));
    }

    fn progetto_e_piano() -> (Progetto, Piano) {
        use crate::progetto::Evento;
        let testo = "# Piano: p\nObiettivo: un compressore\n\
                     ## Fase 1: Ricerca\nConsegna: un rapporto\nFatta quando: approvato\n\
                     - 1.1 [dati] cercare i dati\n- 1.2 [scrittura] scrivere il rapporto (dopo 1.1)\n";
        let (piano, _) = crate::piano::leggi(testo, 1).expect("il piano si legge");
        let q = String::from("2026-10-12T10:00:00");
        let eventi = vec![
            Evento::Creato { quando: q.clone(), numero: 1, richiesta: "x".into(), cartella: String::new() },
            Evento::PianoPronto { quando: q.clone(), versione: 1, da_fase: None, tenuti: Vec::new() },
            Evento::Via { quando: q.clone(), tetto: 0.0 },
            Evento::Affidato { quando: q.clone(), incarico: "1.1".into(), dot: "dati".into(), compito: 7 },
            Evento::Chiuso { quando: q.clone(), incarico: "1.1".into(), stato: Stato::Fatto, esito: "ok".into() },
            Evento::Affidato { quando: q.clone(), incarico: "1.2".into(), dot: "scrittura".into(), compito: 3 },
        ];
        (Progetto::da("p", &eventi).expect("progetto"), piano)
    }

    #[test]
    fn il_todo_spunta_col_compito_che_ha_chiuso() {
        let (p, piano) = progetto_e_piano();
        let t = todo(&p, &piano);
        assert!(t.contains("## Fase 1: Ricerca (in corso)"), "{t}");
        assert!(t.contains("- [x] 1.1 [dati] cercare i dati — `reparti/dati/1.1-cercare-i-dati/` — fatto da dati, compito n. 7"), "{t}");
        assert!(t.contains("- [ ] 1.2 [scrittura] scrivere il rapporto — `reparti/scrittura/1.2-scrivere-il-rapporto/` — in corso (scrittura, compito n. 3)"), "{t}");
        assert!(t.contains("- [ ] il paper (scrittura)") && t.contains("- [ ] il resoconto"), "{t}");
    }

    #[test]
    fn l_analisi_di_una_fase_ha_compiti_consegne_ed_esperimenti() {
        let (p, piano) = progetto_e_piano();
        let a = analisi_fase(&p, &piano.fasi[0], "2026-10-12T11:00:00", "va bene");
        assert!(a.starts_with("# Analisi: fase 1 «Ricerca»\n\nConfermata dalla revisione il 2026-10-12T11:00:00: va bene"), "{a}");
        assert!(a.contains("- 1.1 (dati, compito n. 7): cercare i dati — `reparti/dati/1.1-cercare-i-dati/consegna.md`"), "{a}");
        assert!(a.contains("## Gli esperimenti\n- `reparti/dati/1.1-cercare-i-dati/esperimento.md`"), "{a}");
        assert!(!a.contains("scrittura/1.2-scrivere-il-rapporto/esperimento.md"), "{a}");
    }

    #[test]
    fn l_indice_dei_progetti_si_scrive_e_si_rilegge() {
        let d = std::env::temp_dir().join(format!("nova-indice-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        assert!(progetti(&d).is_empty());
        registra(&d, "s0ex1", Path::new("/docs/Dots_Project/s0ex1")).unwrap();
        registra(&d, "altro", Path::new("/mio")).unwrap();
        let p = progetti(&d);
        assert_eq!(p.get("s0ex1").map(String::as_str), Some("/docs/Dots_Project/s0ex1"));
        assert_eq!(p.len(), 2);
        let _ = std::fs::remove_dir_all(&d);
    }
}
