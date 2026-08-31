//! Quello che assomiglia a una chiave non esce di qui.
//!
//! E' il pezzo con la conseguenza piu' brutta di tutto il crate, e non e' un
//! caso teorico: succedeva. Il fornitore, quando la chiave e' sbagliata, la
//! rimanda indietro **dentro il proprio messaggio d'errore** - «Incorrect API
//! key provided: sk-...» - e da li' finiva in chat e nel registro. Si copre
//! prima di guardare cosa c'e' scritto, non dopo (D29).
//!
//! Il mascheramento e' volutamente **largo**: meglio coprire una stringa
//! innocua che lasciarne passare una vera. Un `[chiave]` di troppo dentro un
//! messaggio d'errore costa una parola; una chiave di meno costa un account.
//!
//! Qui non c'e' una libreria di espressioni regolari, e non serve: le forme
//! da riconoscere sono poche e fisse, e scritte a mano si vede cosa fanno.

/// I prefissi che i fornitori mettono davanti alle loro chiavi.
///
/// Sono marchi di fabbrica, non convenzioni: `sk-` e' OpenAI e chi ne copia
/// il dialetto, `gsk_` e' Groq, `xai-` e' xAI, `AIza` e' Google.
pub const PREFISSI: &[&str] = &["sk-", "gsk_", "xai-", "AIza"];

/// Quanto deve essere lunga la parte dopo il prefisso perche' sia una chiave
/// e non una parola che comincia per caso allo stesso modo.
pub const MINIMO_DOPO_PREFISSO: usize = 8;

/// Le parole che, seguite da un separatore e da un valore lungo, dicono che
/// quel valore e' un segreto anche se non ha un prefisso noto.
///
/// `bearer` e `authorization` sono arrivate tardi, e la storia merita una
/// riga: `Authorization: Bearer <token>` e' il modo piu' comune in cui una
/// chiave finisce dentro un messaggio d'errore o una richiesta registrata, e
/// non era coperto **ne' qui ne' nella versione Python**. Le due
/// implementazioni erano d'accordo, quindi un confronto fra loro non poteva
/// accorgersene: l'ha trovato una prova che invece di chiedere «dicono la
/// stessa cosa?» chiede «e' rimasto qualcosa di segreto?».
pub const PAROLE_SPIA: &[&str] = &[
    "key",
    "token",
    "secret",
    "bearer",
    "authorization",
    "password",
    "passwd",
];

/// Quanto deve essere lungo il valore dopo una parola spia.
pub const MINIMO_DOPO_SPIA: usize = 16;

/// Cosa si scrive al posto di quello che si copre.
pub const COPERTA: &str = "[chiave]";

fn di_chiave(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

fn separatore(c: char) -> bool {
    c == '"' || c == '\'' || c == ':' || c == '=' || c.is_whitespace()
}

/// Il testo senza quello che assomiglia a una chiave.
pub fn senza_chiavi(testo: &str) -> String {
    if testo.is_empty() {
        return String::new();
    }
    let b: Vec<char> = testo.chars().collect();
    let mut fuori = String::with_capacity(testo.len());
    let mut i = 0usize;

    while i < b.len() {
        if let Some(fine) = da_prefisso(&b, i) {
            fuori.push_str(COPERTA);
            i = fine;
            continue;
        }
        if let Some((inizio_valore, fine)) = da_parola_spia(&b, i) {
            // La parola spia e il separatore restano: e' il **valore** che si
            // copre. «api_key: [chiave]» si legge; «[chiave]» da solo no.
            for c in &b[i..inizio_valore] {
                fuori.push(*c);
            }
            fuori.push_str(COPERTA);
            i = fine;
            continue;
        }
        fuori.push(b[i]);
        i += 1;
    }
    fuori
}

/// Una chiave che comincia con un prefisso noto: torna dove finisce.
fn da_prefisso(b: &[char], i: usize) -> Option<usize> {
    // Solo a inizio parola: «ask-me» non contiene una chiave.
    if i > 0 && di_chiave(b[i - 1]) {
        return None;
    }
    for p in PREFISSI {
        let pc: Vec<char> = p.chars().collect();
        if i + pc.len() > b.len() || b[i..i + pc.len()] != pc[..] {
            continue;
        }
        let mut fine = i + pc.len();
        while fine < b.len() && di_chiave(b[fine]) {
            fine += 1;
        }
        if fine - (i + pc.len()) >= MINIMO_DOPO_PREFISSO {
            return Some(fine);
        }
    }
    None
}

/// `api_key = "abcdef..."`: torna (dove comincia il valore, dove finisce).
fn da_parola_spia(b: &[char], i: usize) -> Option<(usize, usize)> {
    if i > 0 && di_chiave(b[i - 1]) {
        return None;
    }
    // Si legge la parola intera, poi si guarda se **finisce** con una spia:
    // cosi' prende «api_key» e «access_token» oltre a «key» e «token».
    let mut fine_parola = i;
    while fine_parola < b.len() && di_chiave(b[fine_parola]) {
        fine_parola += 1;
    }
    if fine_parola == i {
        return None;
    }
    let parola: String = b[i..fine_parola].iter().collect::<String>().to_lowercase();
    if !PAROLE_SPIA.iter().any(|s| parola.ends_with(s)) {
        return None;
    }
    // Fra la parola e il valore ci stanno separatori: virgolette, due punti,
    // uguale, spazi. Non piu' di quattro, o si finirebbe per coprire la
    // parola dopo in una frase qualunque.
    let mut j = fine_parola;
    let mut quanti = 0;
    while j < b.len() && separatore(b[j]) && quanti < 4 {
        j += 1;
        quanti += 1;
    }
    if quanti == 0 {
        return None;
    }
    let inizio = j;
    while j < b.len() && di_chiave(b[j]) {
        j += 1;
    }
    if j - inizio >= MINIMO_DOPO_SPIA {
        Some((inizio, j))
    } else {
        None
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn la_chiave_del_fornitore_non_esce() {
        // Il caso vero: il fornitore rimanda indietro la chiave dentro il
        // proprio errore, e da li' finiva in chat e nel registro.
        let d = senza_chiavi("Incorrect API key provided: sk-abcd1234efgh5678ijkl");
        assert!(!d.contains("sk-abcd"), "{d}");
        assert!(d.contains(COPERTA), "{d}");
    }

    #[test]
    fn tutti_i_prefissi_conosciuti() {
        for (t, _) in [
            ("sk-abcdefgh12345678", ()),
            ("gsk_abcdefgh12345678", ()),
            ("xai-abcdefgh12345678", ()),
            ("AIzaAbcdEfgh12345678", ()),
        ] {
            let d = senza_chiavi(&format!("errore: {t} rifiutata"));
            assert!(!d.contains(t), "e' passata: {t} -> {d}");
        }
    }

    #[test]
    fn una_parola_che_comincia_uguale_non_e_una_chiave() {
        // «sk-» dentro un'altra parola, e un prefisso troppo corto per essere
        // una chiave: coprirli renderebbe illeggibili messaggi innocui.
        assert_eq!(senza_chiavi("ask-me"), "ask-me");
        assert_eq!(senza_chiavi("sk-ab"), "sk-ab");
    }

    #[test]
    fn il_valore_dopo_una_parola_spia_si_copre_ma_la_parola_resta() {
        let d = senza_chiavi("api_key: abcdefghijklmnop1234");
        assert!(d.starts_with("api_key: "), "{d}");
        assert!(!d.contains("abcdefgh"), "{d}");
        let d = senza_chiavi("{\"access_token\":\"abcdefghijklmnop1234\"}");
        assert!(!d.contains("abcdefgh"), "{d}");
        assert!(d.contains("access_token"), "{d}");
    }

    #[test]
    fn lintestazione_http_che_porta_la_chiave() {
        // `Authorization: Bearer <token>` e' il modo piu' comune in cui una
        // chiave finisce in un messaggio d'errore, e per un po' non l'ha
        // coperto nessuna delle due implementazioni.
        let d = senza_chiavi("Authorization: Bearer abcdefghijklmnop1234567890");
        assert!(!d.contains("abcdefgh"), "{d}");
        let d = senza_chiavi("password=abcdefghijklmnop1234567890");
        assert!(!d.contains("abcdefgh"), "{d}");
    }

    #[test]
    fn una_parola_spia_senza_un_valore_lungo_non_copre_la_frase_dopo() {
        // «la chiave non e' stata accettata» non deve diventare
        // «la [chiave]»: il valore dopo dev'essere lungo e compatto.
        let d = senza_chiavi("token non valido");
        assert_eq!(d, "token non valido");
        let d = senza_chiavi("key: 1234");
        assert_eq!(d, "key: 1234");
    }

    #[test]
    fn il_vuoto_resta_vuoto() {
        assert_eq!(senza_chiavi(""), "");
    }

    #[test]
    fn due_chiavi_nella_stessa_riga_spariscono_tutte_e_due() {
        let d = senza_chiavi("prima sk-aaaaaaaaaaaa poi gsk_bbbbbbbbbbbb");
        assert!(!d.contains("sk-aaaa") && !d.contains("gsk_bbbb"), "{d}");
        assert_eq!(d.matches(COPERTA).count(), 2, "{d}");
    }

    #[test]
    fn gli_accenti_non_spezzano_niente() {
        // Il testo intorno e' italiano: se si indicizzasse a byte invece che a
        // caratteri, una lettera accentata prima di una chiave farebbe
        // esplodere il taglio.
        let d = senza_chiavi("perché la chiave sk-abcdefgh12345678 è sbagliata");
        assert!(d.starts_with("perché la chiave "), "{d}");
        assert!(d.ends_with(" è sbagliata"), "{d}");
    }
}
