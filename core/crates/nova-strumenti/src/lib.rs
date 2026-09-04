//! Le mani di NOVA sul PC: cosa sa fare, quanto costa sbagliarlo, e come si
//! racconta a chi guarda.
//!
//! Qui dentro non si **fa** niente. C'e' la dichiarazione di ogni strumento —
//! nome, descrizione, parametri, rischio — e le due cose che se ne ricavano:
//! lo schema JSON che va al modello, e la riga in italiano che l'utente legge
//! mentre NOVA lavora o prima di dare un permesso.
//!
//! **La descrizione e' codice, non prosa.** E' cio' su cui il modello sceglie
//! quale strumento usare, e una parola diversa cambia il comportamento senza
//! che nessun tipo se ne accorga: e' gia' successo di peggiorare `kb_note` da
//! 3 su 4 a 1 su 6 «rafforzandone» il testo. Percio' queste descrizioni non
//! sono state riscritte durante il porting — sono state **estratte** dal
//! registro Python, e il banco le confronta carattere per carattere.
//!
//! Lo schema costa: sessanta strumenti sono circa 7.600 token in ogni
//! richiesta, il 42% del contesto in certe configurazioni. Averlo qui come
//! dato invece che sparso in sessanta decoratori e' il primo passo per
//! poterlo potare con cognizione.

mod dichiarazioni;

// Cosa NOVA puo' fare senza chiedere, e cosa non puo' fare affatto.
pub mod guardie;

pub use dichiarazioni::STRUMENTI;

/// Quanto costa sbagliare questa azione.
///
/// Non e' un'etichetta descrittiva: guida i livelli di autonomia, cioe'
/// decide **cosa NOVA puo' fare senza chiedere**.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rischio {
    /// Sola lettura, nessun effetto: guardare una cartella, leggere l'ora.
    Innocuo = 0,
    /// Crea o modifica: scrive un file, apre un'applicazione.
    Modifica = 1,
    /// Distruttivo o arbitrario: cancella, esegue comandi, chiude processi.
    Pericoloso = 2,
}

impl Rischio {
    pub fn numero(&self) -> u8 {
        *self as u8
    }
}

/// Un parametro di uno strumento, come lo vede il modello.
#[derive(Debug, Clone, Copy)]
pub struct Parametro {
    pub nome: &'static str,
    pub tipo: &'static str,
    pub descrizione: &'static str,
    /// Per gli array: di che tipo sono gli elementi.
    pub elementi: Option<&'static str>,
}

/// Uno strumento dichiarato.
#[derive(Debug, Clone, Copy)]
pub struct Strumento {
    pub nome: &'static str,
    pub descrizione: &'static str,
    pub rischio: Rischio,
    pub categoria: &'static str,
    pub obbligatori: &'static [&'static str],
    pub parametri: &'static [Parametro],
    /// Il modello dell'anteprima, quando si puo' dire con un modello.
    /// Chi non ce l'ha se la fa scrivere a mano in `anteprima`.
    pub anteprima: Option<&'static str>,
}

/// Lo strumento con questo nome, se c'e'.
pub fn trova(nome: &str) -> Option<&'static Strumento> {
    STRUMENTI.iter().find(|s| s.nome == nome)
}

/// I nomi di tutti gli strumenti, in ordine.
pub fn nomi() -> Vec<&'static str> {
    STRUMENTI.iter().map(|s| s.nome).collect()
}

// ---------------------------------------------------------------- schema
/// Lo schema JSON di uno strumento, nel formato che i modelli si aspettano.
///
/// Si scrive a mano invece di passare da una libreria di serializzazione
/// perche' **l'ordine delle chiavi conta**: e' testo che finisce in un
/// prompt, e due ordini diversi sono due prompt diversi. Una mappa che
/// riordina alfabeticamente cambierebbe il contesto del modello senza che
/// nessuno se ne accorga.
pub fn schema(s: &Strumento) -> String {
    let mut fuori = String::with_capacity(512);
    fuori.push_str("{\"type\": \"function\", \"function\": {\"name\": ");
    virgolette(&mut fuori, s.nome);
    fuori.push_str(", \"description\": ");
    virgolette(&mut fuori, s.descrizione);
    fuori.push_str(", \"parameters\": {\"type\": \"object\", \"properties\": {");
    for (i, p) in s.parametri.iter().enumerate() {
        if i > 0 {
            fuori.push_str(", ");
        }
        virgolette(&mut fuori, p.nome);
        fuori.push_str(": {\"type\": ");
        virgolette(&mut fuori, p.tipo);
        // `items` prima di `description`, come nelle dichiarazioni Python.
        // L'ordine delle chiavi non e' un dettaglio di formato: e' testo che
        // finisce nel prompt, e due ordini sono due prompt.
        if let Some(e) = p.elementi {
            fuori.push_str(", \"items\": {\"type\": ");
            virgolette(&mut fuori, e);
            fuori.push('}');
        }
        fuori.push_str(", \"description\": ");
        virgolette(&mut fuori, p.descrizione);
        fuori.push('}');
    }
    fuori.push_str("}, \"required\": [");
    for (i, r) in s.obbligatori.iter().enumerate() {
        if i > 0 {
            fuori.push_str(", ");
        }
        virgolette(&mut fuori, r);
    }
    fuori.push_str("]}}}");
    fuori
}

/// Una stringa JSON, con le stesse fughe che usa Python: solo quelle
/// necessarie, e gli accenti lasciati come sono (`ensure_ascii=False`).
fn virgolette(fuori: &mut String, s: &str) {
    fuori.push('"');
    for c in s.chars() {
        match c {
            '"' => fuori.push_str("\\\""),
            '\\' => fuori.push_str("\\\\"),
            '\n' => fuori.push_str("\\n"),
            '\r' => fuori.push_str("\\r"),
            '\t' => fuori.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                fuori.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => fuori.push(c),
        }
    }
    fuori.push('"');
}

// ------------------------------------------------------------- anteprima
/// Cosa dire a chi guarda, prima di fare una cosa.
///
/// Finisce sotto gli occhi dell'utente in due momenti: lo stato che si legge
/// mentre NOVA lavora, e la riga della richiesta di conferma. Percio' e' in
/// italiano e non e' una chiamata di funzione: chi legge
/// `files_search({"query": "x"})` capisce solo che sta guardando dentro un
/// programma.
pub fn anteprima(nome: &str, args: &dyn Argomenti) -> String {
    if let Some(riga) = anteprima_a_mano(nome, args) {
        return riga;
    }
    if let Some(s) = trova(nome) {
        if let Some(modello) = s.anteprima {
            return rendi(modello, args);
        }
    }
    ripiego(nome, args)
}

/// Da dove si prendono i valori. E' un tratto e non una mappa perche' chi
/// chiama ha gia' i suoi argomenti in una forma sua — un JSON, una struttura
/// — e non deve copiarli in un'altra per farseli raccontare.
pub trait Argomenti {
    /// Il valore di un campo come testo, o `None` se non c'e'.
    fn campo(&self, nome: &str) -> Option<String>;
    /// I campi presenti, in ordine, per il ripiego.
    fn campi(&self) -> Vec<String>;
    /// Se il campo c'e' e non e' «falso» (vuoto, zero, `false`).
    fn acceso(&self, nome: &str) -> bool {
        match self.campo(nome) {
            None => false,
            Some(v) => !(v.is_empty() || v == "false" || v == "0" || v == "None"),
        }
    }
}

/// L'interprete del modello. Quattro forme, e non una di piu':
///
/// ```text
/// {campo}         il valore, «None» se manca — come lo scrive Python
/// {campo?}        il valore, niente se manca
/// {campo|testo}   il valore, «testo» se manca
/// {campo:180}     il valore tagliato
/// ```
///
/// «None» quando manca e' un'abitudine di Python, non una scelta: si copia
/// perche' l'anteprima e' una stringa che l'utente vede, e cambiarla adesso
/// vorrebbe dire due meta' che raccontano la stessa azione con parole diverse.
pub fn rendi(modello: &str, args: &dyn Argomenti) -> String {
    let mut fuori = String::with_capacity(modello.len() + 32);
    let b: Vec<char> = modello.chars().collect();
    let mut i = 0;
    while i < b.len() {
        if b[i] != '{' {
            fuori.push(b[i]);
            i += 1;
            continue;
        }
        let Some(fine) = (i + 1..b.len()).find(|j| b[*j] == '}') else {
            fuori.push(b[i]);
            i += 1;
            continue;
        };
        let dentro: String = b[i + 1..fine].iter().collect();
        i = fine + 1;
        if let Some((campo, quanti)) = dentro.split_once(':') {
            let v = args.campo(campo).unwrap_or_else(|| "None".into());
            let quanti: usize = quanti.parse().unwrap_or(v.chars().count());
            fuori.extend(v.chars().take(quanti));
        } else if let Some((campo, ripiego)) = dentro.split_once('|') {
            let v = args.campo(campo).unwrap_or_default();
            fuori.push_str(if v.is_empty() { ripiego } else { &v });
        } else if let Some(campo) = dentro.strip_suffix('?') {
            fuori.push_str(&args.campo(campo).unwrap_or_default());
        } else {
            fuori.push_str(&args.campo(&dentro).unwrap_or_else(|| "None".into()));
        }
    }
    fuori
}

/// La riga per uno strumento senza anteprima: sgraziata, ma in italiano.
fn ripiego(nome: &str, args: &dyn Argomenti) -> String {
    let pezzi: Vec<String> = args
        .campi()
        .iter()
        .filter_map(|k| {
            let v = args.campo(k)?;
            if v.is_empty() { None } else { Some(format!("{k}: {v}")) }
        })
        .collect();
    let coda = if pezzi.is_empty() {
        String::new()
    } else {
        let unito: String = pezzi.join(", ").chars().take(160).collect();
        format!(" \u{2014} {unito}")
    };
    format!("Uso \u{ab}{}\u{bb}{}", nome.replace('_', " "), coda)
}

/// Le anteprime che un modello non sa dire, perche' hanno un «se» dentro.
///
/// Sono quattordici, e sono a mano apposta: un linguaggio di modelli che cresce a
/// forza di casi speciali smette di essere una semplificazione e diventa un
/// secondo linguaggio da imparare.
fn anteprima_a_mano(nome: &str, a: &dyn Argomenti) -> Option<String> {
    let v = |k: &str| a.campo(k).unwrap_or_else(|| "None".into());
    let vuoto = |k: &str| a.campo(k).unwrap_or_default();
    Some(match nome {
        "close_application" => format!(
            "{}'{}'",
            if a.acceso("force") { "Termina FORZATAMENTE " } else { "Chiude " },
            v("name")
        ),
        "delega" => {
            let motivo = a.campo("motivo").filter(|s| !s.is_empty())
                .unwrap_or_else(|| v("compito"));
            let motivo: String = motivo.chars().take(200).collect();
            format!("Delega a \u{ab}{}\u{bb}: {}", v("a"), motivo)
        }
        "delete_path" => format!(
            "{}{}",
            if a.acceso("permanent") { "ELIMINA DEFINITIVAMENTE " } else { "Sposta nel Cestino " },
            v("path")
        ),
        "open_application" => format!(
            "Avvia l'applicazione '{}' {}",
            v("name"), vuoto("arguments")
        ).trim().to_string(),
        "open_in_browser" => {
            if a.acceso("url") {
                format!("Apre nel browser: {}", v("url"))
            } else {
                format!("Cerca su Google nel browser: {}", v("search_query"))
            }
        }
        "pianifica" => {
            let ripeti = a.campo("ripeti").filter(|s| !s.is_empty())
                .map(|r| format!(" ({r})")).unwrap_or_default();
            format!("Programma NOVA per {}{}: {}", v("quando"), ripeti, v("istruzione"))
        }
        "procedure_elenco" => {
            if a.acceso("cerca") {
                format!("Cerco fra le procedure imparate: \u{ab}{}\u{bb}", v("cerca"))
            } else {
                "Guardo le procedure che ho imparato".into()
            }
        }
        "run_cmd" => tagliato("CMD:\n", &vuoto("command")),
        "run_powershell" => tagliato("PowerShell:\n", &vuoto("command")),
        "run_python" => tagliato("Python:\n", &vuoto("code")),
        "screenshot" => {
            if a.acceso("finestra") {
                format!("Cattura la finestra \u{ab}{}\u{bb}", v("finestra"))
            } else {
                "Cattura tutto lo schermo".into()
            }
        }
        "set_volume" => {
            if a.acceso("mute") {
                "Silenzia l'audio".into()
            } else {
                format!("Imposta il volume a {}%", v("level"))
            }
        }
        "automazione_crea" => {
            let quando: String = vuoto("quando_usarla").chars().take(120).collect();
            format!("Scrive e collauda una nuova automazione \u{ab}{}\u{bb}: {}",
                    v("nome"), quando)
        }
        "write_file" => format!(
            "Scrive {} caratteri in {}{}",
            vuoto("content").chars().count(),
            v("path"),
            if a.acceso("append") { " (in coda)" } else { "" }
        ),
        _ => return None,
    })
}

fn tagliato(testa: &str, corpo: &str) -> String {
    let corto: String = corpo.chars().take(800).collect();
    format!("{testa}{corto}")
}
