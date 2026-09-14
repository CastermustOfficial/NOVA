//! Le immagini che entrano nella conversazione, e quali no.
//!
//! I modelli sanno vedere. Quello che mancava non era la vista, era il
//! **tubo**: NOVA scattava una schermata, la salvava, e al modello arrivava
//! la frase «salvata in C:\...» — che lui leggeva convinto di aver guardato.
//! Uno strumento che riesce senza consegnare niente e' peggio di uno che
//! manca, perche' produce fiducia mal riposta.
//!
//! La regola era: se il risultato di uno strumento **nomina** un'immagine che
//! sta su disco, quella si guarda. Scritta cosi' vale anche per gli strumenti
//! che verranno, ed e' il motivo per cui era stata scritta cosi'.
//!
//! ## Perche' adesso se ne consegna al massimo una
//!
//! Misurato, non immaginato. `search_files` restituisce **percorsi assoluti,
//! uno per riga**. Chiedere «trova le foto del matrimonio» produceva un
//! risultato che nomina venti fotografie, e le prime due venivano
//! automaticamente convertite in base64 e allegate alla conversazione —
//! quindi, con un cervello a pagamento, **uscivano dal PC** al giro
//! successivo. Sotto una riga che diceva «questa e' la figura prodotta dallo
//! strumento», che per giunta non era vero: nessuno strumento le aveva
//! prodotte, una ricerca le aveva nominate.
//!
//! La differenza fra i due casi non e' nel percorso ne' nella cartella — un
//! utente puo' legittimamente dire «guarda questa foto sul desktop» — ma nel
//! **numero**. Uno strumento che produce un'immagine ne produce una; un
//! elenco ne nomina tante. Quindi: una sola immagine nominata si consegna,
//! molte si **dichiarano** e non si allegano (D129: un taglio si dice
//! sempre). Il modello sa che ci sono, sa quante sono, e puo' chiederne una.

/// Le estensioni che si riconoscono come immagine.
pub const ESTENSIONI: [&str; 6] = ["png", "jpg", "jpeg", "webp", "gif", "bmp"];

/// Il lato lungo massimo. Oltre non si guadagna leggibilita': si paga
/// contesto.
pub const LATO_MASSIMO: u32 = 1568;

/// Un'immagine piu' pesante di cosi', dopo la conversione, non si manda:
/// vuol dire che qualcosa non ha funzionato nel ridimensionamento.
pub const BYTE_MASSIMI: usize = 4_000_000;

fn ammesso(c: char) -> bool {
    // La classe del Python: `[^\s"'<>|?*\n\r]`. I due punti e la barra
    // rovescia ci **stanno dentro**, ed e' quello che permette a un percorso
    // Windows di essere un solo pezzo.
    !(c.is_whitespace() || c == '"' || c == '\'' || c == '<' || c == '>'
      || c == '|' || c == '?' || c == '*')
}

fn estensione_a(caratteri: &[char], da: usize) -> Option<usize> {
    for e in ESTENSIONI {
        let lung = e.len();
        if da + lung > caratteri.len() {
            continue;
        }
        let pezzo: String = caratteri[da..da + lung].iter().collect();
        if pezzo.to_lowercase() == e {
            return Some(lung);
        }
    }
    None
}

/// Quanto e' lungo l'attacco di un percorso a questa posizione: `C:\` o `/`.
fn attacco(caratteri: &[char], i: usize) -> Option<usize> {
    if caratteri[i] == '/' {
        return Some(1);
    }
    if i + 2 < caratteri.len()
        && caratteri[i].is_ascii_alphabetic()
        && caratteri[i + 1] == ':'
        && caratteri[i + 2] == '\\'
    {
        return Some(3);
    }
    None
}

/// I percorsi di immagine **nominati** in un testo.
///
/// Scritto a mano invece che con l'espressione regolare, per la stessa
/// ragione delle chiamate dentro il testo (D154): scriverla a mano costringe
/// a dire la regola ad alta voce. Qui la regola e': si parte da `C:\` o da
/// `/`, si prende **il meno possibile** — e' un `+?`, non un `+` — e ci si
/// ferma alla **prima** estensione di immagine che si incontra. Quindi in
/// `C:\a.png\b.png` il percorso e' `C:\a.png`, non tutta la riga. Il corpo
/// deve avere almeno un carattere, e non contiene spazi ne' virgolette: un
/// percorso con uno spazio dentro si spezza, ed e' un limite noto della
/// regola, non un difetto di questa scrittura.
pub fn nominate(testo: &str) -> Vec<String> {
    let caratteri: Vec<char> = testo.chars().collect();
    let mut fuori: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i < caratteri.len() {
        let Some(lungo) = attacco(&caratteri, i) else {
            i += 1;
            continue;
        };
        let inizio_corpo = i + lungo;
        let mut j = inizio_corpo;
        let mut trovato = None;
        // Il corpo deve avere almeno un carattere: `+?` e non `*?`.
        while j < caratteri.len() {
            if !ammesso(caratteri[j]) {
                break;
            }
            j += 1;
            if j > inizio_corpo && j < caratteri.len() && caratteri[j] == '.' {
                if let Some(lung_est) = estensione_a(&caratteri, j + 1) {
                    trovato = Some(j + 1 + lung_est);
                    break;
                }
            }
        }
        match trovato {
            Some(fine) => {
                fuori.push(caratteri[i..fine].iter().collect());
                i = fine;
            }
            None => i += 1,
        }
    }
    fuori
}

/// Cosa fare delle immagini nominate da un risultato.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Consegna {
    /// Nessuna immagine nominata: non si fa niente.
    Niente,
    /// Una sola: si allega.
    Una(String),
    /// Tante: non si allega niente, e **si dice**.
    Troppe(usize),
}

/// La decisione, e basta. Chi chiama ci mette il disco e la rete.
pub fn decidi(esistenti: &[String]) -> Consegna {
    match esistenti.len() {
        0 => Consegna::Niente,
        1 => Consegna::Una(esistenti[0].clone()),
        n => Consegna::Troppe(n),
    }
}

/// Il testo che accompagna l'immagine allegata.
///
/// Dice «nominata», non «prodotta»: la versione di prima diceva «questa e' la
/// figura prodotta dallo strumento» anche quando lo strumento si era limitato
/// a elencarla, e una frase falsa al modello vale quanto un dato falso.
pub fn testo_di_consegna(nome: &str) -> String {
    format!(
        "[immagine: {nome}] Questa e' l'immagine che lo strumento ha nominato. \
         Guardala e usa quello che ci vedi: e' la tua vista sullo schermo."
    )
}

/// Quando ne sono state nominate troppe: si dice quante, e che non si allegano.
pub fn nota_troppe(quante: usize) -> String {
    format!(
        "[NOVA] Il risultato nomina {quante} immagini. Non te le allego: \
         allegarne alcune a caso vorrebbe dire mandare fuori dal PC dei file \
         che nessuno ha chiesto di guardare. Se te ne serve una, chiedila per \
         nome e te la faccio vedere."
    )
}

/// Quando il cervello attivo non ha il proiettore visivo.
///
/// L'immagine non si allega — llama-server risponderebbe 500 e quel
/// messaggio resterebbe in conversazione a far fallire anche tutti i turni
/// dopo — ma **si dice**, perche' il risultato dello strumento nomina lo
/// stesso un file, e un modello a cui arriva «salvata in C:\...» e nient'altro
/// racconta volentieri cosa c'era dentro.
pub const SENZA_PROIETTORE: &str =
    "[NOVA] L'immagine c'e' su disco, ma non te la posso far vedere: questo \
     cervello e' partito senza proiettore visivo. Non dire di averla guardata. \
     Se ti serve sapere cosa c'e' sullo schermo usa ui.tree o ui.find, che \
     leggono l'interfaccia come testo.";

/// Quanto diventa grande un'immagine dopo il ridimensionamento.
///
/// Una schermata 4K in PNG diventa qualche megabyte di base64: riempirebbe
/// il contesto lasciando al modello lo spazio per guardare e non per
/// ragionare.
pub fn nuova_misura(larghezza: u32, altezza: u32, lato_massimo: u32) -> (u32, u32) {
    let lato = larghezza.max(altezza);
    if lato <= lato_massimo || lato == 0 {
        return (larghezza, altezza);
    }
    let fattore = f64::from(lato_massimo) / f64::from(lato);
    (
        std::cmp::max(1, (f64::from(larghezza) * fattore) as u32),
        std::cmp::max(1, (f64::from(altezza) * fattore) as u32),
    )
}

/// Un pezzo di un messaggio ricco: o testo, o un'immagine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Blocco {
    Testo(String),
    Immagine,
}

/// La riga che prende il posto delle figure sfilate.
pub const SFILATA: &str =
    "\n[NOVA] La figura non e' allegata: questo cervello non sa guardare le immagini.";

/// Toglie da un messaggio le figure che il modello non puo' vedere.
///
/// Torna `None` se non c'era niente da togliere: chi ha chiamato deve saperlo,
/// perche' rilanciare la stessa richiesta identica e' il modo piu' rapido di
/// trasformare un errore in un ciclo.
///
/// **Il testo resta e l'immagine se ne va.** Non si butta il messaggio
/// intero: il modello continua a sapere che una figura c'era, e non crede di
/// averla guardata. Un messaggio sparito e un messaggio senza figura sono due
/// cose diverse — la prima gli fa dimenticare che ha chiesto qualcosa.
pub fn sfila(blocchi: &[Blocco]) -> Option<String> {
    let quante = blocchi.iter().filter(|b| **b == Blocco::Immagine).count();
    if quante == 0 {
        return None;
    }
    let testi: Vec<&str> = blocchi
        .iter()
        .filter_map(|b| match b {
            Blocco::Testo(t) => Some(t.as_str()),
            Blocco::Immagine => None,
        })
        .collect();
    Some(format!("{}{SFILATA}", testi.join("\n").trim()).trim().to_string())
}

/// Come si racconta a chi guarda il registro quante ne sono state sfilate.
pub fn quante_sfilate(tolte: usize) -> String {
    let parola = if tolte == 1 { "figura" } else { "figure" };
    format!("il cervello attivo non vede: ho sfilato {tolte} {parola} dalla conversazione")
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn un_percorso_windows_e_uno_unix() {
        assert_eq!(nominate(r"salvata in C:\Users\utente\schermate\x.png"),
                   vec![r"C:\Users\utente\schermate\x.png"]);
        assert_eq!(nominate("vedi /home/utente/foto.JPEG"), vec!["/home/utente/foto.JPEG"]);
    }

    #[test]
    fn il_meno_possibile_e_non_il_piu_possibile() {
        // `+?`: ci si ferma alla prima estensione, non all'ultima.
        assert_eq!(nominate(r"C:\a.png\b.png"), vec![r"C:\a.png"]);
    }

    #[test]
    fn niente_dove_non_ce_niente() {
        assert!(nominate("").is_empty());
        assert!(nominate("nessun percorso qui, solo parole").is_empty());
        assert!(nominate("testo.png senza attacco").is_empty());
        assert!(nominate(r"C:\solo\una\cartella").is_empty());
    }

    #[test]
    fn qualunque_barra_e_un_attacco_e_va_detto() {
        // Non e' un difetto di questa scrittura: e' cosa fa la regola, e
        // conviene averlo scritto invece di scoprirlo. Una barra qualunque
        // apre un percorso, quindi un percorso relativo e un URL entrano
        // tutti e due — e poi cadono perche' su disco non esistono. Il
        // filtro vero e' l'esistenza del file, non la forma.
        assert_eq!(nominate("relativo/senza/attacco.png"), vec!["/senza/attacco.png"]);
        assert_eq!(nominate("http://x.it/a/b.png"), vec!["//x.it/a/b.png"]);
    }

    #[test]
    fn un_elenco_ne_nomina_tante() {
        let uscita = "C:\\f\\a.jpg\nC:\\f\\b.jpg\nC:\\f\\c.png";
        assert_eq!(nominate(uscita).len(), 3);
    }

    #[test]
    fn una_sola_si_consegna_tante_si_dichiarano() {
        assert_eq!(decidi(&[]), Consegna::Niente);
        assert_eq!(decidi(&["x.png".to_string()]), Consegna::Una("x.png".into()));
        assert_eq!(decidi(&["a".into(), "b".into(), "c".into()]), Consegna::Troppe(3));
        assert!(nota_troppe(3).contains('3'));
    }

    #[test]
    fn il_testo_non_dice_prodotta_quando_non_e_prodotta() {
        let t = testo_di_consegna("x.png");
        assert!(t.contains("ha nominato"));
        assert!(!t.contains("prodotta"));
    }

    #[test]
    fn sfilare_lascia_il_testo_e_lo_dice() {
        let m = vec![Blocco::Testo("ecco la schermata".into()), Blocco::Immagine];
        let fuori = sfila(&m).unwrap();
        assert!(fuori.starts_with("ecco la schermata"));
        assert!(fuori.contains("non sa guardare le immagini"));
    }

    #[test]
    fn senza_figure_non_si_tocca_niente() {
        assert_eq!(sfila(&[Blocco::Testo("solo testo".into())]), None);
        assert_eq!(sfila(&[]), None);
    }

    #[test]
    fn una_figura_da_sola_lascia_la_sola_riga() {
        let fuori = sfila(&[Blocco::Immagine]).unwrap();
        assert!(fuori.starts_with("[NOVA]"), "{fuori:?}");
        assert!(!fuori.starts_with('\n'));
    }

    #[test]
    fn il_plurale_e_giusto() {
        assert!(quante_sfilate(1).contains("1 figura "));
        assert!(quante_sfilate(2).contains("2 figure "));
        assert!(quante_sfilate(0).contains("0 figure "));
    }

    #[test]
    fn il_ridimensionamento_tiene_le_proporzioni() {
        assert_eq!(nuova_misura(800, 600, 1568), (800, 600));
        assert_eq!(nuova_misura(3840, 2160, 1568), (1568, 882));
        assert_eq!(nuova_misura(1, 20000, 1568), (1, 1568));
        assert_eq!(nuova_misura(0, 0, 1568), (0, 0));
    }
}
