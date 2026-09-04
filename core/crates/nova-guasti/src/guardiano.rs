//! Cosa **non** entra nella memoria di NOVA.
//!
//! E' una domanda diversa da quella di `chiavi`, che maschera i segreti nei
//! messaggi d'errore, e la differenza sta tutta nel costo dello sbaglio.
//! Coprire di troppo in un registro costa una parola illeggibile; rifiutare
//! di troppo costa **un ricordo che NOVA non avra' mai**, e l'utente non
//! capisce perche'. Percio' qui si guarda con la mano ferma dove la' si
//! guarda con la mano larga — ma le forme sono le stesse, dichiarate in un
//! posto solo (`chiavi::PREFISSI`): due elenchi separati sanno sempre cose
//! diverse, ed e' cosi' che una chiave AWS finiva in chiaro nel giornale dei
//! guasti mentre il vault la rifiutava.
//!
//! **Il motivo non contiene mai il valore.** Un rifiuto finisce in un
//! registro, e un registro che ripete la credenziale appena rifiutata non ha
//! protetto niente.

use crate::chiavi::{blocco_di_chiave, credenziali_in_indirizzo, di_chiave, jwt,
                    numero_di_carta, prefisso_qui};

/// Le parole che, seguite da un valore, annunciano una credenziale.
///
/// «parola d ordine» senza apostrofo non e' un refuso: NOVA si fa dettare, e
/// chi trascrive l'apostrofo non sempre lo mette. Una regola che vale solo
/// per chi scrive protegge meta' degli utenti.
pub const CHIAVI_A_VOCE: &[&str] = &[
    "password", "passwd", "pwd", "passphrase",
    "api key", "apikey", "api_key", "api-key", "chiave api",
    "secret", "segreto", "token", "bearer",
    "authorization", "autorizzazione",
    "credenziale", "credenziali",
    "access key", "access_key", "access-key",
    "client secret", "client_secret", "client-secret",
    "private key", "private_key", "private-key", "chiave privata",
    "pin", "otp", "seed phrase", "seedphrase",
    "parola d'ordine", "parola di ordine", "parola d ordine",
];

/// Le chiavi il cui valore e' fatto di **parole comuni**.
///
/// Una passphrase e una seed phrase sono per costruzione sei parole del
/// vocabolario, e il controllo sulla densita' — quello che distingue «la
/// password e' cambiata» da «la password e' Tramonto2026» — le lascerebbe
/// passare tutte.
///
/// Sono anche le due cose che non si possono cambiare dopo: una seed phrase
/// rubata svuota un portafoglio, e non c'e' nessun «reimposta». Qui l'errore
/// da evitare non e' bloccare una frase di troppo.
pub const CHIAVI_A_PAROLE: &[&str] = &[
    "passphrase", "seed phrase", "seedphrase",
    "frase di recupero", "recovery phrase",
    "parola d'ordine", "parola di ordine", "parola d ordine",
];

/// Token che sono solo il ponte fra la chiave e il valore: non si contano.
const PONTI: &[&str] = &[
    ":", "=", "\u{e8}", "e'", "e", "sono", "era", "sarebbe", "il", "la", "lo",
    "del", "della", "dello", "dei", "di", "d'", "mia", "mio", "un", "una",
];

/// Quanti token dopo la chiave si guardano.
///
/// Quattro: fra la parola e il valore ci sta spesso una precisazione — «la
/// password *del wifi* e' ...» — e piu' in la' e' un'altra frase, che presa
/// darebbe falsi allarmi.
const QUANTI_TOKEN: usize = 4;

/// Se questa stringa e' un segreto invece che una parola.
///
/// «la password e' cambiata», «il token e' scaduto» non sono segreti: sono
/// frasi. Cio' che distingue un valore vero e' la **densita'** — una cifra, un
/// simbolo — oppure una lunghezza che nessuna parola italiana normale
/// raggiunge. Meglio lasciar passare «segretissima» che rifiutare mezza
/// conversazione.
fn sembra_un_valore(valore: &str) -> bool {
    let quanti = valore.chars().count();
    // Un PIN e' corto per costruzione: quattro cifre sono gia' il segreto
    // intero, e la regola generale sulla lunghezza lo lascerebbe passare.
    if !valore.is_empty() && valore.chars().all(|c| c.is_ascii_digit()) && (4..=19).contains(&quanti)
    {
        return true;
    }
    if quanti >= 6 && !valore.chars().all(|c| c.is_alphabetic()) {
        return true;
    }
    quanti >= 16
}

/// Come si chiama il segreto che c'e' qui dentro, se ce n'e' uno.
///
/// L'ordine e' quello delle forme, non quello del testo: si cerca la prima
/// forma dell'elenco che compare da qualche parte, non la prima cosa che
/// compare nel testo. E' la stessa scelta del Python, e conta perche' il nome
/// finisce nel messaggio: chi legge deve sentirsi dire «una chiave AWS», che
/// e' una cosa precisa, e non «un numero di carta» perche' per caso c'erano
/// diciassette cifre piu' in su.
pub fn che_forma(testo: &str) -> Option<&'static str> {
    if testo.is_empty() {
        return None;
    }
    let b: Vec<char> = testo.chars().collect();
    // Prima i prefissi, nell'ordine in cui sono dichiarati.
    for p in crate::chiavi::PREFISSI {
        for i in 0..b.len() {
            if let Some((nome, _)) = prefisso_qui(&b, i, true) {
                if nome == p.nome {
                    return Some(nome);
                }
            }
        }
    }
    for i in 0..b.len() {
        if blocco_di_chiave(&b, i).is_some() {
            return Some("un blocco di chiave privata");
        }
    }
    for i in 0..b.len() {
        if jwt(&b, i).is_some() {
            return Some("un JSON Web Token");
        }
    }
    for i in 0..b.len() {
        if credenziali_in_indirizzo(&b, i).is_some() {
            return Some("credenziali dentro un indirizzo");
        }
    }
    for i in 0..b.len() {
        if numero_di_carta(&b, i).is_some() {
            return Some("un numero di carta");
        }
    }
    None
}

/// Dove finisce, nel testo minuscolo, la chiave che comincia a `i` — se ce
/// n'e' una fra quelle date.
fn chiave_qui(minuscolo: &[char], i: usize, elenco: &[&str]) -> Option<usize> {
    // A inizio parola: «passwordless» non annuncia niente, e nemmeno il
    // «pin» dentro «spingere».
    if i > 0 && di_chiave(minuscolo[i - 1]) {
        return None;
    }
    for k in elenco {
        let kc: Vec<char> = k.chars().collect();
        if i + kc.len() > minuscolo.len() || minuscolo[i..i + kc.len()] != kc[..] {
            continue;
        }
        let fine = i + kc.len();
        if fine < minuscolo.len() && di_chiave(minuscolo[fine]) {
            continue;
        }
        return Some(fine);
    }
    None
}

/// Una chiave «a parole» seguita da almeno due parole vere.
fn coppia_a_parole(minuscolo: &[char]) -> bool {
    for i in 0..minuscolo.len() {
        let Some(fine) = chiave_qui(minuscolo, i, CHIAVI_A_PAROLE) else {
            continue;
        };
        let coda: String = minuscolo[fine..].iter().collect();
        let parole: Vec<&str> = coda
            .split(|c: char| !(c.is_alphabetic() || c == '\''))
            .filter(|p| p.chars().count() >= 3)
            .take(2)
            .collect();
        // Due parole di almeno tre lettere dopo «seed phrase» sono gia' una
        // seed phrase: non si aspetta di vederne dodici.
        if parole.len() == 2 && parole.iter().all(|p| !PONTI.contains(p)) {
            return true;
        }
    }
    false
}

/// Una chiave, e poco dopo qualcosa che sembra un valore.
///
/// Si cerca la chiave, **poi** si guardano i token che seguono, uno per uno.
/// Non con una regola sola che leghi chiave e valore in un colpo: quella
/// trova la prima coppia e si ferma li'. In «la password del wifi e'
/// Tramonto2026» la prima coppia sarebbe «password ... wifi» — una parola
/// comune, che giustamente non fa scattare niente — e il segreto due parole
/// piu' in la' non verrebbe mai guardato. Stesso buco travestito da un altro
/// caso: «Authorization: Bearer eyJhb...» legava «authorization» a «Bearer»,
/// che e' innocuo, e il token restava fuori.
fn chiave_poi_valore(testo: &str, minuscolo: &[char]) -> bool {
    let originale: Vec<char> = testo.chars().collect();
    for i in 0..minuscolo.len() {
        let Some(fine) = chiave_qui(minuscolo, i, CHIAVI_A_VOCE) else {
            continue;
        };
        let coda: String = originale[fine.min(originale.len())..].iter().collect();
        let mut visti = 0usize;
        for grezzo in coda.split_whitespace() {
            let valore = grezzo.trim_matches(|c| "\"'`.,;:)=".contains(c));
            if valore.is_empty() || PONTI.contains(&valore.to_lowercase().as_str()) {
                continue;
            }
            if sembra_un_valore(valore) {
                return true;
            }
            visti += 1;
            if visti >= QUANTI_TOKEN {
                break;
            }
        }
    }
    false
}

/// Il motivo per cui questo testo non va in memoria, o `None` se puo'
/// entrare.
pub fn perche_non_si_salva(testo: &str) -> Option<&'static str> {
    if testo.is_empty() {
        return None;
    }
    if let Some(forma) = che_forma(testo) {
        return Some(forma);
    }
    let minuscolo: Vec<char> = testo.to_lowercase().chars().collect();
    if coppia_a_parole(&minuscolo) {
        return Some("una credenziale in chiaro");
    }
    if chiave_poi_valore(testo, &minuscolo) {
        return Some("una credenziale in chiaro");
    }
    None
}

/// Se questo testo contiene qualcosa che non deve essere ricordato.
pub fn e_riservato(testo: &str) -> bool {
    perche_non_si_salva(testo).is_some()
}
