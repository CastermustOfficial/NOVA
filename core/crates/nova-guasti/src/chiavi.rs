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
/// I prefissi che annunciano una chiave. Erano quattro, e il guardiano del
/// vault ne conosceva altri sei che qui uscivano **in chiaro** nel giornale
/// dei guasti: le chiavi AWS, i token GitHub e Slack. Due elenchi separati
/// sanno sempre cose diverse — vedi `nova/forme_riservate.py`, che dalla
/// parte Python li ha uniti in uno solo.
pub const PREFISSI: &[Prefisso] = &[
    Prefisso { inizio: "sk-", nome: "una chiave di servizio", minimo: 16 },
    Prefisso { inizio: "pk-", nome: "una chiave di servizio", minimo: 16 },
    Prefisso { inizio: "rk-", nome: "una chiave di servizio", minimo: 16 },
    Prefisso { inizio: "sk_", nome: "una chiave di servizio", minimo: 16 },
    Prefisso { inizio: "pk_", nome: "una chiave di servizio", minimo: 16 },
    Prefisso { inizio: "rk_", nome: "una chiave di servizio", minimo: 16 },
    Prefisso { inizio: "gsk_", nome: "una chiave Groq", minimo: 8 },
    Prefisso { inizio: "xai-", nome: "una chiave xAI", minimo: 8 },
    Prefisso { inizio: "AIza", nome: "una chiave Google", minimo: 8 },
    Prefisso { inizio: "ghp_", nome: "un token GitHub", minimo: 20 },
    Prefisso { inizio: "gho_", nome: "un token GitHub", minimo: 20 },
    Prefisso { inizio: "ghu_", nome: "un token GitHub", minimo: 20 },
    Prefisso { inizio: "ghs_", nome: "un token GitHub", minimo: 20 },
    Prefisso { inizio: "ghr_", nome: "un token GitHub", minimo: 20 },
    Prefisso { inizio: "xoxb-", nome: "un token Slack", minimo: 10 },
    Prefisso { inizio: "xoxa-", nome: "un token Slack", minimo: 10 },
    Prefisso { inizio: "xoxp-", nome: "un token Slack", minimo: 10 },
    Prefisso { inizio: "xoxr-", nome: "un token Slack", minimo: 10 },
    Prefisso { inizio: "xoxs-", nome: "un token Slack", minimo: 10 },
    Prefisso { inizio: "AKIA", nome: "una chiave AWS", minimo: 16 },
    Prefisso { inizio: "ASIA", nome: "una chiave AWS", minimo: 16 },
];

/// Un prefisso, come si chiama, e quanto deve essere lungo cio' che segue.
///
/// Il nome serve a chi **rifiuta** — il guardiano del vault dice cosa ha
/// trovato — e non a chi maschera, che copre e basta.
///
/// Il minimo e' due numeri diversi per una ragione, non per distrazione:
/// **mascherare e rifiutare non hanno lo stesso costo di errore.** Coprire di
/// troppo in un messaggio d'errore costa una parola illeggibile in un
/// registro; rifiutare di troppo costa un **ricordo che NOVA non avra' mai**,
/// e senza che l'utente capisca perche'. Quindi si maschera con la mano
/// larga (`MINIMO_DOPO_PREFISSO`, otto per tutti) e si rifiuta con la mano
/// ferma (il minimo dichiarato qui, che e' quello del fornitore).
pub struct Prefisso {
    pub inizio: &'static str,
    pub nome: &'static str,
    pub minimo: usize,
}

/// Quanto deve essere lunga la parte dopo il prefisso perche' **si copra**.
/// Vedi `Prefisso::minimo` per la soglia con cui invece si rifiuta.
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

pub(crate) fn di_chiave(c: char) -> bool {
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
        if let Some(fine) = da_forma(&b, i) {
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

/// Le forme che non hanno un prefisso ma si riconoscono lo stesso.
///
/// Sono i tre casi che il guardiano del vault prendeva e questo no: il blocco
/// di chiave privata, le credenziali infilate dentro un indirizzo, e il JSON
/// Web Token. Un messaggio d'errore con dentro `https://utente:parola@host`
/// finiva nel giornale dei guasti cosi' com'era.
fn da_forma(b: &[char], i: usize) -> Option<usize> {
    if let Some(f) = blocco_di_chiave(b, i) {
        return Some(f);
    }
    if let Some(f) = credenziali_in_indirizzo(b, i) {
        return Some(f);
    }
    if let Some(f) = jwt(b, i) {
        return Some(f);
    }
    numero_di_carta(b, i)
}

/// Da tredici a diciannove cifre, con o senza spazi e trattini in mezzo.
///
/// Il guardiano del vault lo prendeva e questo no: un numero di carta poteva
/// finire in chiaro nel giornale dei guasti. Trovato confrontando le due
/// implementazioni su un corpus che chiedeva «e' sopravvissuto qualcosa?»
/// invece di «siete d'accordo?».
pub(crate) fn numero_di_carta(b: &[char], i: usize) -> Option<usize> {
    if i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == '_') {
        return None;
    }
    if !b[i].is_ascii_digit() {
        return None;
    }
    let mut j = i;
    let mut cifre = 0usize;
    let mut ultima_cifra = i;
    while j < b.len() && cifre < 19 {
        if b[j].is_ascii_digit() {
            cifre += 1;
            ultima_cifra = j;
            j += 1;
            // un solo separatore fra una cifra e l'altra
            if j < b.len() && (b[j] == ' ' || b[j] == '-') {
                j += 1;
            }
        } else {
            break;
        }
    }
    if cifre < 13 {
        return None;
    }
    // Deve finire su una cifra, e dopo non puo' esserci altra roba di parola.
    let fine = ultima_cifra + 1;
    if fine < b.len() && (b[fine].is_ascii_alphanumeric() || b[fine] == '_') {
        return None;
    }
    Some(fine)
}

/// `-----BEGIN RSA PRIVATE KEY-----` e parenti.
pub(crate) fn blocco_di_chiave(b: &[char], i: usize) -> Option<usize> {
    if b[i] != '-' {
        return None;
    }
    let mut j = i;
    while j < b.len() && b[j] == '-' {
        j += 1;
    }
    if j - i < 3 {
        return None;
    }
    while j < b.len() && b[j].is_whitespace() {
        j += 1;
    }
    let resto: String = b[j..].iter().collect();
    if !resto.starts_with("BEGIN ") {
        return None;
    }
    // «BEGIN » piu' le parole maiuscole fino a «PRIVATE KEY».
    let dopo = &resto["BEGIN ".len()..];
    let fine_etichetta = dopo
        .char_indices()
        .take_while(|(_, c)| c.is_ascii_uppercase() || *c == ' ')
        .last()
        .map(|(k, c)| k + c.len_utf8())
        .unwrap_or(0);
    let etichetta = &dopo[..fine_etichetta];
    if !etichetta.contains("PRIVATE KEY") {
        return None;
    }
    let consumati = "BEGIN ".len() + etichetta.trim_end().len();
    Some(j + consumati)
}

/// `schema://utente:parola@host`: si copre fino alla chiocciola compresa.
pub(crate) fn credenziali_in_indirizzo(b: &[char], i: usize) -> Option<usize> {
    if i > 0 && di_chiave(b[i - 1]) {
        return None;
    }
    if !b[i].is_ascii_lowercase() {
        return None;
    }
    let mut j = i;
    while j < b.len()
        && (b[j].is_ascii_lowercase() || b[j].is_ascii_digit() || "+.-".contains(b[j]))
    {
        j += 1;
    }
    let resto: String = b[j..].iter().collect();
    if !resto.starts_with("://") {
        return None;
    }
    j += 3;
    // utente: niente spazi, niente / : @
    let inizio_utente = j;
    while j < b.len() && !b[j].is_whitespace() && !"/:@".contains(b[j]) {
        j += 1;
    }
    if j == inizio_utente || j >= b.len() || b[j] != ':' {
        return None;
    }
    j += 1;
    let inizio_parola = j;
    while j < b.len() && !b[j].is_whitespace() && !"/@".contains(b[j]) {
        j += 1;
    }
    if j == inizio_parola || j >= b.len() || b[j] != '@' {
        return None;
    }
    Some(j + 1)
}

/// `eyJ....` con due punti: intestazione, contenuto e firma.
pub(crate) fn jwt(b: &[char], i: usize) -> Option<usize> {
    if i > 0 && di_chiave(b[i - 1]) {
        return None;
    }
    let resto: String = b[i..].iter().collect();
    if !resto.starts_with("eyJ") {
        return None;
    }
    let di_jwt = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-';
    let mut j = i;
    let mut punti = 0;
    while j < b.len() && (di_jwt(b[j]) || b[j] == '.') {
        if b[j] == '.' {
            punti += 1;
        }
        j += 1;
    }
    if punti >= 2 && j - i >= 24 {
        Some(j)
    } else {
        None
    }
}

/// Una chiave che comincia con un prefisso noto: torna dove finisce.
fn da_prefisso(b: &[char], i: usize) -> Option<usize> {
    // Solo a inizio parola: «ask-me» non contiene una chiave.
    if i > 0 && di_chiave(b[i - 1]) {
        return None;
    }
    prefisso_qui(b, i, false).map(|(_, fine)| fine)
}

/// Il prefisso che comincia qui, se c'e': come si chiama e dove finisce.
///
/// `severo` sceglie la soglia: falso per mascherare (mano larga), vero per
/// rifiutare (la soglia del fornitore).
pub(crate) fn prefisso_qui(b: &[char], i: usize, severo: bool) -> Option<(&'static str, usize)> {
    if i > 0 && di_chiave(b[i - 1]) {
        return None;
    }
    for p in PREFISSI {
        let pc: Vec<char> = p.inizio.chars().collect();
        if i + pc.len() > b.len() || b[i..i + pc.len()] != pc[..] {
            continue;
        }
        let mut fine = i + pc.len();
        while fine < b.len() && di_chiave(b[fine]) {
            fine += 1;
        }
        let quanto = fine - (i + pc.len());
        let minimo = if severo { p.minimo } else { MINIMO_DOPO_PREFISSO };
        if quanto >= minimo {
            return Some((p.nome, fine));
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
