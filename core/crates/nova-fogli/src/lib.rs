//! I fogli di calcolo: riferimenti, lettura, e scrittura senza spogliare.
//!
//! NOVA sapeva fare meta' della meta': leggeva il testo delle celle di un
//! `.xlsx`, in sola lettura, e non sapeva scrivere niente. Qui c'e' il resto,
//! e le regole che lo governano — che sono poche e tutte nate da un modo
//! preciso di perdere i dati di qualcuno.
//!
//! La libreria e' `umya-spreadsheet`, scelta **misurando** invece che
//! leggendo la documentazione: sul banco dei documenti regge il giro
//! leggi-tocca-riscrivi tenendo formule, formato percentuale, grassetti e il
//! secondo foglio (D239).

use std::path::{Path, PathBuf};

/// Quante righe di un foglio si leggono prima di fermarsi.
///
/// Non e' un limite di memoria: e' che il foglio lo legge un modello, e un
/// bilancio da diecimila righe versato in un prompt non e' un'informazione,
/// e' un contesto pieno e una risposta peggiore.
pub const RIGHE_MAX: usize = 500;

/// Quante cifre puo' avere un numero prima che scriverlo come numero lo
/// cambi. Oltre 2^53 un intero non ci sta piu' esatto in un `f64`, e un IBAN
/// o un numero d'ordine di sedici cifre tornerebbe indietro **diverso**.
pub const CIFRE_ESATTE: usize = 15;

/// Cosa c'e' dentro una cella quando ce la si scrive.
#[derive(Debug, Clone, PartialEq)]
pub enum Valore {
    Vuoto,
    Testo(String),
    Numero(f64),
    /// Senza l'uguale davanti: `SUM(A1:A2)`.
    Formula(String),
}

/// Una cella: `A1`, `$B$7`, `aa12`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Riferimento {
    /// Da 1. `A` e' 1, `Z` 26, `AA` 27.
    pub colonna: u32,
    /// Da 1, come la si vede nel foglio.
    pub riga: u32,
}

/// `A` -> 1, `Z` -> 26, `AA` -> 27. Maiuscole e minuscole sono la stessa cosa.
pub fn numero_di_colonna(lettere: &str) -> Option<u32> {
    if lettere.is_empty() || lettere.len() > 3 {
        return None; // oltre `XFD` non c'e' foglio
    }
    let mut n: u32 = 0;
    for c in lettere.chars() {
        let d = match c {
            'A'..='Z' => c as u32 - 'A' as u32 + 1,
            'a'..='z' => c as u32 - 'a' as u32 + 1,
            _ => return None,
        };
        n = n.checked_mul(26)?.checked_add(d)?;
    }
    Some(n)
}

/// 1 -> `A`, 27 -> `AA`. Lo zero non e' una colonna.
pub fn lettere_di_colonna(mut n: u32) -> String {
    if n == 0 {
        return String::new();
    }
    let mut fuori = Vec::new();
    while n > 0 {
        // Bijective base 26: il resto zero vuol dire `Z`, e il prestito va
        // tolto **prima** di dividere. Scritta come una base 26 normale
        // manda `Z` a `A@` e `AA` a `BA`.
        let resto = (n - 1) % 26;
        fuori.push((b'A' + resto as u8) as char);
        n = (n - 1) / 26;
    }
    fuori.iter().rev().collect()
}

impl Riferimento {
    pub fn nuovo(colonna: u32, riga: u32) -> Riferimento {
        Riferimento { colonna, riga }
    }

    /// Da come si scrive. I `$` si tollerano: chi copia un riferimento da
    /// Excel se li porta dietro, e rifiutarlo per quello sarebbe pedanteria
    /// pagata da chi incolla.
    pub fn da(testo: &str) -> Option<Riferimento> {
        let pulito: String = testo.trim().chars().filter(|c| *c != '$').collect();
        let taglio = pulito.find(|c: char| c.is_ascii_digit())?;
        let (lettere, cifre) = pulito.split_at(taglio);
        let colonna = numero_di_colonna(lettere)?;
        let riga: u32 = cifre.parse().ok()?;
        if riga == 0 {
            return None; // le righe partono da 1: `A0` non esiste
        }
        Some(Riferimento { colonna, riga })
    }

    /// Come si scrive: `A1`.
    pub fn scritto(&self) -> String {
        format!("{}{}", lettere_di_colonna(self.colonna), self.riga)
    }
}

/// Un rettangolo di celle: `A1:C10`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Area {
    pub da: Riferimento,
    pub a: Riferimento,
}

impl Area {
    /// Da come si scrive. `A1` da solo e' l'area di una cella sola, e
    /// `C10:A1` e' la stessa area di `A1:C10`: chi seleziona col mouse
    /// dal basso a destra scrive il secondo, e rifiutarglielo non
    /// protegge nessuno.
    pub fn da_testo(testo: &str) -> Option<Area> {
        let t = testo.trim();
        let (a, b) = match t.split_once(':') {
            Some((a, b)) => (Riferimento::da(a)?, Riferimento::da(b)?),
            None => {
                let uno = Riferimento::da(t)?;
                (uno, uno)
            }
        };
        Some(Area {
            da: Riferimento::nuovo(a.colonna.min(b.colonna), a.riga.min(b.riga)),
            a: Riferimento::nuovo(a.colonna.max(b.colonna), a.riga.max(b.riga)),
        })
    }

    pub fn scritta(&self) -> String {
        format!("{}:{}", self.da.scritto(), self.a.scritto())
    }

    pub fn quante_celle(&self) -> u64 {
        let colonne = (self.a.colonna - self.da.colonna + 1) as u64;
        let righe = (self.a.riga - self.da.riga + 1) as u64;
        colonne * righe
    }

    pub fn contiene(&self, r: &Riferimento) -> bool {
        r.colonna >= self.da.colonna
            && r.colonna <= self.a.colonna
            && r.riga >= self.da.riga
            && r.riga <= self.a.riga
    }
}

/// Cosa vuol dire, per una cella, il testo che ci si vuole mettere dentro.
///
/// La regola e' una sola e sta sotto tutte le altre: **un numero si scrive
/// come numero solo quando scriverlo come numero non lo cambia**. `007`
/// diventerebbe `7`, `+39 02 1234` diventerebbe un conto, e un numero
/// d'ordine di sedici cifre tornerebbe indietro arrotondato. Sono tutti e
/// tre danni silenziosi, e uno solo di essi — un totale che non somma
/// perche' i numeri sono testo — e' visibile. Fra un danno visibile e tre
/// invisibili si sceglie quello visibile.
///
/// `1.50` resta un numero: gli zeri in coda dopo la virgola sono un
/// **formato**, non un altro valore, e il formato della cella non lo tocca
/// nessuno.
pub fn interpreta(testo: &str) -> Valore {
    let t = testo.trim();
    if t.is_empty() {
        return Valore::Vuoto;
    }
    // L'apostrofo davanti e' la convenzione di Excel per «questo e' testo,
    // non discutere»: chi la scrive sa gia' cosa vuole.
    if let Some(resto) = t.strip_prefix('\'') {
        return Valore::Testo(resto.to_string());
    }
    if let Some(resto) = t.strip_prefix('=') {
        if !resto.trim().is_empty() {
            return Valore::Formula(resto.to_string());
        }
        return Valore::Testo(t.to_string());
    }
    match numero_onesto(t) {
        Some(n) => Valore::Numero(n),
        None => Valore::Testo(t.to_string()),
    }
}

/// Il numero che questo testo e', se scriverlo come numero non lo cambia.
fn numero_onesto(t: &str) -> Option<f64> {
    let senza_segno = t.strip_prefix('-').unwrap_or(t);
    if senza_segno.is_empty() {
        return None;
    }
    let (intera, decimale) = match senza_segno.split_once('.') {
        Some((a, b)) => (a, Some(b)),
        None => (senza_segno, None),
    };
    if intera.is_empty() || !intera.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    // `007` non e' sette: e' un codice, e scriverlo come numero lo accorcia.
    if intera.len() > 1 && intera.starts_with('0') {
        return None;
    }
    if let Some(d) = decimale {
        if d.is_empty() || !d.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
    }
    let cifre = intera.trim_start_matches('0').len() + decimale.map_or(0, str::len);
    if cifre > CIFRE_ESATTE {
        return None;
    }
    t.parse::<f64>().ok()
}

/// Come si rende un foglio in testo.
#[derive(Debug, Clone)]
pub struct Come {
    pub separatore: String,
    pub righe_max: usize,
    /// Se una riga in cui non c'e' niente si salta.
    pub salta_vuote: bool,
}

impl Default for Come {
    fn default() -> Come {
        Come {
            separatore: " | ".to_string(),
            righe_max: RIGHE_MAX,
            salta_vuote: true,
        }
    }
}

/// Il titolo di un foglio dentro il testo reso.
pub fn titolo(nome: &str) -> String {
    format!("--- foglio «{nome}» ---")
}

/// La riga che dice che ci si e' fermati.
pub fn troncato(righe_max: usize) -> String {
    format!("[...foglio troncato a {righe_max} righe]")
}

/// Un foglio, in testo.
pub fn rendi(nome: &str, righe: &[Vec<String>], come: &Come) -> String {
    let mut fuori: Vec<String> = Vec::new();
    for r in righe {
        if come.salta_vuote && !r.iter().any(|c| !c.trim().is_empty()) {
            continue;
        }
        fuori.push(r.join(&come.separatore));
        if fuori.len() > come.righe_max {
            fuori.push(troncato(come.righe_max));
            break;
        }
    }
    if fuori.is_empty() {
        return String::new();
    }
    format!("{}\n{}", titolo(nome), fuori.join("\n"))
}

// ---------------------------------------------------------------- il disco

/// Cosa c'era in una cella quando la si e' letta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Letta {
    /// Il valore in cache: quello che l'ultimo programma che ha aperto il
    /// file ci ha lasciato dentro.
    pub valore: String,
    /// La formula, se c'e', senza l'uguale davanti.
    pub formula: String,
}

/// Come si legge una cella che contiene un conto.
///
/// Un `.xlsx` porta due cose per ogni cella con una formula: la formula, e
/// il **risultato dell'ultima volta che qualcuno l'ha calcolata**. Nessuna
/// libreria — ne' `openpyxl` ne' `umya` — calcola niente: leggono la cache.
///
/// E la cache e' vuota in tutti i file generati da un programma invece che
/// da Excel. Cioe' NOVA, leggendo un foglio che aveva appena scritto lei,
/// vedeva **celle vuote** dove stanno i totali, e nessun modo di sapere che
/// c'era un conto: `data_only=True` restituisce `None`, e `None` diventa la
/// stringa vuota come una cella davvero vuota (D253).
///
/// Qui: se il risultato c'e', si dice il risultato. Se non c'e' e una
/// formula c'e', si dice la formula con l'uguale davanti — che e' brutto da
/// leggere e vero, invece che pulito e falso.
pub fn come_si_legge(c: &Letta) -> String {
    if !c.valore.is_empty() {
        return c.valore.clone();
    }
    if !c.formula.is_empty() {
        // `openpyxl` da' la formula con l'uguale davanti, `umya` senza. Un
        // solo uguale si toglie, non tutti: `==A1` e' una formula che
        // comincia per `=`, e mangiargliene due la cambia.
        return format!("={}", c.formula.strip_prefix('=').unwrap_or(&c.formula));
    }
    String::new()
}

/// Dove si scrive prima di mettere il file al suo posto.
///
/// `umya` riscrive **tutto** l'archivio: una scrittura interrotta a meta' —
/// la corrente, il disco pieno, l'antivirus — lascerebbe al posto del
/// bilancio di qualcuno uno zip troncato che ha ancora il nome giusto. Si
/// scrive di fianco e si rinomina, come per il vault e per la
/// configurazione.
pub fn di_fianco(dove: &Path) -> PathBuf {
    nova_componenti::in_arrivo(dove)
}

/// I nomi dei fogli, nell'ordine in cui stanno nel file.
pub fn nomi_dei_fogli(dove: &Path) -> Result<Vec<String>, String> {
    let libro = umya_spreadsheet::reader::xlsx::read(dove).map_err(|e| non_si_apre(dove, &e))?;
    Ok(libro
        .sheet_collection()
        .iter()
        .map(|f| f.name().to_string())
        .collect())
}

fn non_si_apre(dove: &Path, e: &impl std::fmt::Debug) -> String {
    format!("non riesco ad aprire {}: {e:?}", dove.display())
}

/// Il messaggio per chi chiede un foglio che non c'e'.
///
/// Dice **quali** ci sono, e non e' cortesia: senza, l'unico modo di
/// scoprirlo e' aprire il file in un altro programma, cioe' esattamente la
/// cosa che si stava chiedendo a NOVA di evitare.
pub fn foglio_che_non_ce(chiesto: &str, ci_sono: &[String]) -> String {
    format!(
        "in questo file non c'e' un foglio «{chiesto}». Ci sono: {}",
        ci_sono.join(", ")
    )
}

/// Il contenuto di un foglio (o di tutti), in testo.
pub fn leggi(dove: &Path, foglio: Option<&str>, come: &Come) -> Result<String, String> {
    let libro = umya_spreadsheet::reader::xlsx::read(dove).map_err(|e| non_si_apre(dove, &e))?;
    let nomi: Vec<String> = libro
        .sheet_collection()
        .iter()
        .map(|f| f.name().to_string())
        .collect();
    let da_leggere: Vec<String> = match foglio {
        Some(n) if !n.is_empty() => {
            if !nomi.iter().any(|x| x == n) {
                return Err(foglio_che_non_ce(n, &nomi));
            }
            vec![n.to_string()]
        }
        _ => nomi.clone(),
    };
    let mut pezzi = Vec::new();
    for nome in &da_leggere {
        let f = libro.sheet_by_name(nome).map_err(|e| format!("{e:?}"))?;
        let (colonne, righe_quante) = f.highest_column_and_row();
        let mut righe: Vec<Vec<String>> = Vec::new();
        for r in 1..=righe_quante {
            let mut riga = Vec::with_capacity(colonne as usize);
            for c in 1..=colonne {
                let letta = match f.cell((c, r)) {
                    Some(cella) => Letta {
                        valore: cella.value().to_string(),
                        formula: cella.formula().to_string(),
                    },
                    None => Letta {
                        valore: String::new(),
                        formula: String::new(),
                    },
                };
                riga.push(come_si_legge(&letta));
            }
            righe.push(riga);
            if righe.len() > come.righe_max {
                break;
            }
        }
        let reso = rendi(nome, &righe, come);
        if !reso.is_empty() {
            pezzi.push(reso);
        }
    }
    if pezzi.is_empty() {
        return Err(format!("{} e' vuoto", nome_di(dove)));
    }
    Ok(pezzi.join("\n\n"))
}

fn nome_di(dove: &Path) -> String {
    dove.file_name()
        .map(|x| x.to_string_lossy().to_string())
        .unwrap_or_else(|| dove.display().to_string())
}

/// Scrive delle celle in un foglio che c'e' gia', e lascia in pace il resto.
///
/// Il foglio deve esistere. Crearlo da se' quando il nome non si trova
/// vorrebbe dire che un nome scritto male — `Conti` invece di `conti` —
/// produce un secondo foglio vuoto accanto a quello vero invece di un
/// errore, e chi guarda il totale in fondo non vede niente di strano.
pub fn scrivi(dove: &Path, foglio: &str, celle: &[(Riferimento, Valore)]) -> Result<usize, String> {
    let mut libro =
        umya_spreadsheet::reader::xlsx::read(dove).map_err(|e| non_si_apre(dove, &e))?;
    let nomi: Vec<String> = libro
        .sheet_collection()
        .iter()
        .map(|f| f.name().to_string())
        .collect();
    if !nomi.iter().any(|x| x == foglio) {
        return Err(foglio_che_non_ce(foglio, &nomi));
    }
    {
        let f = libro
            .sheet_by_name_mut(foglio)
            .map_err(|e| format!("{e:?}"))?;
        for (r, v) in celle {
            let cella = f.cell_mut((r.colonna, r.riga));
            match v {
                Valore::Vuoto => {
                    cella.set_value("");
                }
                Valore::Testo(t) => {
                    cella.set_value_string(t.clone());
                }
                Valore::Numero(n) => {
                    cella.set_value_number(*n);
                }
                Valore::Formula(f) => {
                    cella.set_formula(f.clone());
                }
            }
        }
    }
    let provvisorio = di_fianco(dove);
    umya_spreadsheet::writer::xlsx::write(&libro, &provvisorio)
        .map_err(|e| format!("non riesco a scrivere {}: {e:?}", provvisorio.display()))?;
    std::fs::rename(&provvisorio, dove).map_err(|e| {
        let _ = std::fs::remove_file(&provvisorio);
        format!("non riesco a mettere {} al suo posto: {e}", nome_di(dove))
    })?;
    Ok(celle.len())
}

#[cfg(test)]
mod prove;
