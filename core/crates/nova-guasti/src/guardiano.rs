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

/// Cosa si toglie ai bordi di un token prima di guardarlo: la punteggiatura,
/// da D408 le parentesi, le virgolette e le caporali, da D409 il punto di
/// domanda, quello esclamativo e l'asterisco del grassetto. Senza, «token
/// appresi o «gist»» e «token prima e dopo (legenda» sembravano valori — un
/// simbolo e sei caratteri — e un piano che parla di token non usciva dal PC.
/// Un valore vero fra parentesi o virgolette resta un valore, tolti i bordi.
pub const BORDI: &str = "\"'`.,;:)=([]{}<>«»“”‘’?!*";

/// Le chiavi il cui valore puo' essere un numero e basta: un PIN, un codice
/// usa e getta, una password di sole cifre (D408). Dopo le altre — token,
/// secret, api key — un numero da solo e' una misura.
pub const CHIAVI_A_NUMERO: &[&str] = &[
    "password", "passwd", "pwd", "pin", "otp",
    "parola d'ordine", "parola di ordine", "parola d ordine",
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
fn sembra_un_valore(valore: &str, chiave_numerica: bool) -> bool {
    let quanti = valore.chars().count();
    // Un numero da solo (anche coi separatori: «1.532», «0,85») e' un
    // segreto solo dopo le chiavi che si scrivono a cifre (D408): «i token
    // passano da 1532 a 980» e' una misura, non un PIN. Lunghissimo, resta
    // sospetto comunque.
    if !chiave_numerica && e_un_numero(valore) {
        return quanti >= 16;
    }
    // Un PIN e' corto per costruzione: quattro cifre sono gia' il segreto
    // intero, e la regola generale sulla lunghezza lo lascerebbe passare.
    if !valore.is_empty() && valore.chars().all(|c| c.is_ascii_digit()) && (4..=19).contains(&quanti)
    {
        return true;
    }
    // Un nome di codice fatto di parole («llama.cpp», «api_key») non e' un
    // valore; con una cifra in un pezzo («abc_def123») si' (D409). Dopo una
    // password no: «Correct-Horse-Battery» e' una password fatta di parole.
    if !chiave_numerica && e_un_nome(valore) {
        return false;
    }
    // Una parola, anche con l'apostrofo dell'elisione («l'argomento»), non
    // e' un valore per i simboli che ha (D409).
    let parola = e_una_parola(valore);
    if quanti >= 6 && !parola {
        return true;
    }
    // Lunga, e' sospetta; ma una parola lunga scritta come una parola
    // («ridimensionamento», «Contemporaneamente») no: lo e' se ha maiuscole
    // in mezzo, come una password fatta di parole attaccate (D409).
    quanti >= 16 && !(parola && valore.chars().skip(1).all(|c| !c.is_uppercase()))
}

/// I separatori dei nomi di codice: «llama.cpp», «api_key», «a::b», percorsi.
pub const SEPARATORI_DI_NOME: &str = "_.:/\\-%";

/// Parole unite da [`SEPARATORI_DI_NOME`], ognuna scritta come una parola
/// (minuscola, con la maiuscola iniziale, o tutta maiuscola): un nome di
/// codice o un percorso, non un segreto. Un pezzo con una cifra, o con le
/// maiuscole sparse come una chiave a caso, non e' una parola.
fn e_un_nome(valore: &str) -> bool {
    if !valore.chars().any(|c| SEPARATORI_DI_NOME.contains(c)) {
        return false;
    }
    let parti: Vec<&str> = valore
        .split(|c| SEPARATORI_DI_NOME.contains(c))
        .filter(|p| !p.is_empty())
        .collect();
    !parti.is_empty()
        && parti.iter().all(|p| {
            e_una_parola(p)
                && (p.chars().skip(1).all(|c| !c.is_uppercase()) || p.chars().all(|c| !c.is_lowercase()))
        })
}

/// Lettere, con al piu' l'apostrofo dell'elisione fra due lettere.
fn e_una_parola(valore: &str) -> bool {
    let c: Vec<char> = valore.chars().collect();
    !c.is_empty()
        && c.iter().enumerate().all(|(i, x)| {
            x.is_alphabetic()
                || ((*x == '\'' || *x == '’')
                    && i > 0
                    && i + 1 < c.len()
                    && c[i - 1].is_alphabetic()
                    && c[i + 1].is_alphabetic())
        })
}

/// Cifre, con al piu' i separatori di un numero scritto: punto, virgola,
/// apostrofo, trattino basso.
fn e_un_numero(valore: &str) -> bool {
    valore.chars().any(|c| c.is_ascii_digit())
        && valore.chars().all(|c| c.is_ascii_digit() || ".,'_".contains(c))
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
    valore_dopo_la_chiave(testo, minuscolo).is_some()
}

/// Come [`chiave_poi_valore`], ma dice **quale** valore: serve a coprirlo
/// ([`copri`]).
fn valore_dopo_la_chiave(testo: &str, minuscolo: &[char]) -> Option<String> {
    let originale: Vec<char> = testo.chars().collect();
    for i in 0..minuscolo.len() {
        let Some(fine) = chiave_qui(minuscolo, i, CHIAVI_A_VOCE) else {
            continue;
        };
        let chiave: String = minuscolo[i..fine].iter().collect();
        let numerica = CHIAVI_A_NUMERO.contains(&chiave.as_str());
        let coda: String = originale[fine.min(originale.len())..].iter().collect();
        let mut visti = 0usize;
        for grezzo in coda.split_whitespace() {
            let valore = grezzo.trim_matches(|c| BORDI.contains(c));
            if valore.is_empty() || PONTI.contains(&valore.to_lowercase().as_str()) {
                continue;
            }
            if sembra_un_valore(valore, numerica) {
                return Some(valore.to_string());
            }
            visti += 1;
            if visti >= QUANTI_TOKEN {
                break;
            }
        }
    }
    None
}

/// Cosa resta al posto di un pezzo coperto da [`copri`].
pub const COPERTO: &str = "[nascosto dal guardiano]";

/// Quanti pezzi [`copri`] copre al massimo, prima di arrendersi.
const PEZZI_MASSIMI: usize = 200;

/// Il testo con coperto quello che sembra una credenziale, e i pezzi
/// coperti; `None` se resta qualcosa che non si sa coprire (D417).
///
/// Il guardiano non si allenta: una consegna di S0ex1 che scriveva «token
/// RAG: 98,77%» non partiva per il giudice, e il 13 ottobre meta' delle
/// consegne e' rimasta senza voto. Gio: «meglio un falso positivo», ma le
/// consegne vanno viste tutte. Quindi si copre il pezzo e si manda il resto:
/// il giudice vede tutto tranne quello, e quello non esce dal PC.
///
/// Prima le forme note (`chiavi::senza_chiavi`, la mano larga: prefissi,
/// chiavi private, token, carte), poi uno alla volta i valori dopo una
/// chiave. Una frase segreta fatta di parole («seed phrase» e poi parole)
/// non si sa coprire con sicurezza: allora `None`, e il testo non parte.
pub fn copri(testo: &str) -> Option<(String, Vec<String>)> {
    copri_con(testo, &[])
}

/// Come [`copri`], lasciando in chiaro i pezzi che l'utente ha detto non
/// segreti (D417): la prima volta il guardiano li copre, Gio li guarda, e da
/// li' in poi il giudice li vede.
///
/// Un non segreto si mette da parte con un segnaposto fatto di sole lettere
/// minuscole, che il guardiano non prende per un valore; dopo, torna al suo
/// posto. Una chiave dalla forma nota resta coperta anche se e' nell'elenco.
pub fn copri_con(testo: &str, non_segreti: &[String]) -> Option<(String, Vec<String>)> {
    let mut messi: Vec<(String, &str)> = Vec::new();
    let mut t = testo.to_string();
    let senza_forma = |p: &&String| {
        !p.trim().is_empty() && che_forma(p).is_none() && crate::chiavi::senza_chiavi(p) == **p
    };
    for (i, pezzo) in non_segreti.iter().filter(senza_forma).enumerate() {
        if t.contains(pezzo.as_str()) {
            let segnaposto = format!("nonsegreto{}", lettere(i));
            t = t.replace(pezzo.as_str(), &segnaposto);
            messi.push((segnaposto, pezzo.as_str()));
        }
    }
    let (mut coperto, pezzi) = copri_tutto(&t)?;
    for (segnaposto, pezzo) in messi.iter().rev() {
        coperto = coperto.replace(segnaposto.as_str(), pezzo);
    }
    Some((coperto, pezzi))
}

/// Un numero scritto in lettere minuscole: 0 «a», 25 «z», 26 «ba».
fn lettere(mut n: usize) -> String {
    let mut t = Vec::new();
    loop {
        t.push((b'a' + (n % 26) as u8) as char);
        n /= 26;
        if n == 0 {
            break;
        }
    }
    t.iter().rev().collect()
}

fn copri_tutto(testo: &str) -> Option<(String, Vec<String>)> {
    // Le forme note si coprono e basta: sono chiavi vere, non pezzi da far
    // rivedere a nessuno. Fra i pezzi vanno i valori dopo una chiave.
    let mut t = crate::chiavi::senza_chiavi(testo);
    let mut pezzi = Vec::new();
    for _ in 0..PEZZI_MASSIMI {
        if perche_non_si_salva(&t).is_none() {
            return Some((t, pezzi));
        }
        if che_forma(&t).is_some() {
            return None;
        }
        let minuscolo: Vec<char> = t.to_lowercase().chars().collect();
        let valore = valore_dopo_la_chiave(&t, &minuscolo)?;
        t = t.replace(&valore, COPERTO);
        pezzi.push(valore);
    }
    None
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

/// Gli strumenti che mostrano **cosa c'e' aperto adesso**, non **com'e' fatto
/// il PC**.
///
/// E' la stessa domanda del resto di questo modulo — cosa non entra in
/// memoria — posta pero' sulla **provenienza** invece che sulla forma. Il
/// guardiano guarda il testo e riconosce una chiave, un numero di carta, una
/// password; qui non c'e' niente da riconoscere: il titolo di una finestra e'
/// una stringa qualunque, e proprio per questo passerebbe.
///
/// Leggerli serve ad agire. **Ricordarli** scriverebbe nel vault i titoli
/// delle schede aperte e dei documenti su cui si sta lavorando, in chiaro e
/// per sempre — e un vault e' una cartella che la gente sincronizza.
pub const GUARDANO_LO_SCHERMO: [&str; 6] = [
    "ui.windows",
    "ui.tree",
    "ui.find",
    "finestre",
    "albero_finestra",
    "screenshot",
];

/// Se un turno che ha usato questi strumenti va ricordato **coperto**.
///
/// Basta uno: se in mezzo a dieci chiamate una sola ha guardato lo schermo,
/// il turno porta dentro cio' che c'era sullo schermo.
pub fn riservato_per_provenienza(strumenti: &[String]) -> bool {
    strumenti.iter().any(|s| GUARDANO_LO_SCHERMO.contains(&s.as_str()))
}

/// Se questo testo contiene qualcosa che non deve essere ricordato.
pub fn e_riservato(testo: &str) -> bool {
    perche_non_si_salva(testo).is_some()
}

// ------------------------------------------- l'etichetta senza il valore

/// Le parole che, nel **nome** di un campo, dicono che il contenuto e' un
/// segreto anche se preso da solo non lo sembra.
///
/// Serve a chi ha l'etichetta e il valore in due posti diversi. Il registro
/// delle azioni scrive «scritto in #password» in un campo e «Tramonto2026!»
/// in un altro: guardati uno per volta non sono niente — il secondo e' una
/// parola con dentro un anno — e guardati insieme sono una credenziale.
///
/// Si cercano dentro il testo **senza separatori**, cosi' `api key`,
/// `api_key`, `api-key` e `apikey` sono la stessa etichetta. Il prezzo e' che
/// ogni tanto si riconosce un'etichetta che non c'era; il guadagno e' che non
/// se ne perde una scritta in un modo che non avevo previsto, e in questo
/// mestiere le due cose non si pagano uguale.
pub const ETICHETTE_DI_SEGRETO: [&str; 17] = [
    "password",
    "passwd",
    "pwd",
    "passphrase",
    "token",
    "secret",
    "segreto",
    "bearer",
    "authorization",
    "credenziale",
    "credenziali",
    "api key",
    "private key",
    "seed phrase",
    "chiave api",
    "chiave privata",
    "chiave segreta",
];

/// Le etichette che si cercano **intere**.
///
/// `pin` e `otp` sono tre lettere che capitano dentro parole qualunque —
/// «spingere», «pinza», «optare» — e cercarle come le altre vorrebbe dire
/// buttare i dettagli di mezzo registro. Sono anche le uniche due per cui la
/// parte Python chiede il confine di parola.
pub const ETICHETTE_INTERE: [&str; 2] = ["pin", "otp"];

/// Anche `parola d'ordine` si dice in piu' modi, e l'apostrofo sparisce con
/// gli altri separatori: chi si fa dettare non sempre lo mette.
fn senza_separatori(t: &str) -> String {
    t.chars()
        .filter(|c| !matches!(c, ' ' | '\t' | '_' | '-' | '\'' | '\u{2019}'))
        .collect()
}

fn e_di_parola(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn parola_intera(dove: &str, cosa: &str) -> bool {
    let mut da = 0;
    while let Some(i) = dove[da..].find(cosa) {
        let inizio = da + i;
        let fine = inizio + cosa.len();
        let prima = dove[..inizio].chars().next_back();
        let dopo = dove[fine..].chars().next();
        if !prima.is_some_and(e_di_parola) && !dopo.is_some_and(e_di_parola) {
            return true;
        }
        da = inizio + 1;
    }
    false
}

/// Se questo testo e' il **nome** di qualcosa che contiene un segreto.
pub fn etichetta_di_segreto(testo: &str) -> bool {
    let basso = testo.to_lowercase();
    let compatto = senza_separatori(&basso);
    if ETICHETTE_DI_SEGRETO
        .iter()
        .any(|e| compatto.contains(&senza_separatori(e)))
    {
        return true;
    }
    // «parola d'ordine», «parola di ordine», «parola d ordine»: la stessa
    // cosa detta in tre modi, e senza separatori due di quei tre coincidono.
    if compatto.contains("paroladordine") || compatto.contains("paroladiordine") {
        return true;
    }
    ETICHETTE_INTERE.iter().any(|p| parola_intera(&basso, p))
}

#[cfg(test)]
mod prove_etichette {
    use super::*;

    #[test]
    fn il_nome_del_campo_basta() {
        for t in [
            "password",
            "scritto in #password",
            "API_KEY",
            "api-key",
            "apikey",
            "la Chiave Privata",
            "seed phrase",
            "parola d'ordine",
            "parola di ordine",
            "token di accesso",
        ] {
            assert!(
                etichetta_di_segreto(t),
                "«{t}» doveva annunciare un segreto"
            );
        }
    }

    #[test]
    fn e_le_parole_di_tre_lettere_si_cercano_intere() {
        assert!(etichetta_di_segreto("il PIN della carta"));
        assert!(etichetta_di_segreto("otp"));
        // Senza il confine di parola questi butterebbero i dettagli di mezzo
        // registro.
        assert!(!etichetta_di_segreto("spingere il pulsante"));
        assert!(!etichetta_di_segreto("una pinza"));
        assert!(!etichetta_di_segreto("optare per l'altro"));
    }

    #[test]
    fn una_riga_qualunque_non_annuncia_niente() {
        for t in [
            "premuto «Invia»",
            "aperto il PDF",
            "eseguito un comando",
            "",
        ] {
            assert!(
                !etichetta_di_segreto(t),
                "«{t}» non annuncia nessun segreto"
            );
        }
    }
}

#[cfg(test)]
mod prove_copri {
    use super::*;

    #[test]
    fn si_copre_il_pezzo_e_il_resto_si_legge() {
        // D417: le consegne di S0ex1 rimaste senza voto il 13 ottobre.
        let t = "- *Asimmetria token RAG*: 98,77% di input per prompt da 4.000 token.";
        assert!(perche_non_si_salva(t).is_some(), "il guardiano non si e' allentato");
        let (coperto, pezzi) = copri(t).unwrap();
        assert!(perche_non_si_salva(&coperto).is_none(), "{coperto}");
        assert!(coperto.contains(COPERTO) && coperto.contains("Asimmetria token RAG"), "{coperto}");
        assert!(!coperto.contains("98,77%"), "{coperto}");
        assert_eq!(pezzi, ["98,77%"]);
    }

    #[test]
    fn un_segreto_vero_non_esce_mai() {
        for (t, segreto) in [
            ("la password del wifi e' Tramonto2026, non dirla", "Tramonto2026"),
            ("Authorization: Bearer abcDEF123456ghiJKL789", "abcDEF123456ghiJKL789"),
            ("il pin del bancomat e' 4821", "4821"),
        ] {
            // Che lo copra la mano larga delle chiavi o il valore dopo la
            // chiave, il segreto non c'e' piu'.
            let (coperto, _) = copri(t).unwrap();
            assert!(!coperto.contains(segreto), "{t} -> {coperto}");
            assert!(perche_non_si_salva(&coperto).is_none(), "{coperto}");
        }
        // Una chiave dalla forma nota si copre senza finire fra i pezzi.
        let chiave = format!("la chiave e' {}", ["sk-ant-", "api03-", "AbCdEfGhIjKlMnOpQrSt0123456789"].concat());
        let (coperto, _) = copri(&chiave).unwrap();
        assert!(!coperto.contains("AbCdEfGhIjKlMnOpQrSt0123456789"), "{coperto}");
        assert!(perche_non_si_salva(&coperto).is_none(), "{coperto}");
        // Una frase segreta fatta di parole non si sa coprire: non parte.
        assert!(copri("la seed phrase e' gatto albero fiume").is_none());
        // Un testo pulito resta com'e'.
        assert_eq!(copri("niente di segreto qui"), Some(("niente di segreto qui".into(), vec![])));
    }

    #[test]
    fn un_non_segreto_detto_dall_utente_resta_in_chiaro() {
        let t = "token RAG: 98,77% e 99,69% di input; la password e' Tramonto2026";
        let lista = vec!["98,77%".to_string()];
        let (coperto, pezzi) = copri_con(t, &lista).unwrap();
        assert!(coperto.contains("98,77%"), "{coperto}");
        assert!(!coperto.contains("99,69%") && !coperto.contains("Tramonto2026"), "{coperto}");
        assert!(!pezzi.contains(&"98,77%".to_string()), "{pezzi:?}");
        // Una chiave dalla forma nota resta coperta anche nell'elenco.
        let chiave = ["sk-ant-", "api03-", "AbCdEfGhIjKlMnOpQrSt0123456789"].concat();
        let (coperto, _) = copri_con(&format!("la chiave e' {chiave}"), &[chiave.clone()]).unwrap();
        assert!(!coperto.contains("AbCdEfGhIjKlMnOpQrSt0123456789"), "{coperto}");
        assert_eq!(lettere(0), "a");
        assert_eq!(lettere(27), "bb");
    }
}

#[cfg(test)]
mod prove_bordi {
    use super::*;

    /// Il piano di S0ex1 (12 ottobre): parla di token, e il legale non poteva
    /// guardarlo perche' «(legenda» e ««gist»» sembravano valori (D408).
    #[test]
    fn una_parola_fra_parentesi_o_caporali_non_e_un_valore() {
        for t in [
            "potatura dei token e riassunto), token appresi o «gist», codifiche adatte",
            "token prima e dopo (legenda inclusa) con ogni tokenizer",
            "i token [stimati] del corpus",
            "il token “scaduto” ieri",
        ] {
            assert_eq!(perche_non_si_salva(t), None, "{t}");
        }
    }

    #[test]
    fn un_valore_fra_parentesi_o_caporali_resta_un_valore() {
        for t in [
            "la password (Tramonto2026) e' sul foglio",
            "il token e' «9f8e7d6c5b4a3210»",
            "secret: “Girasole99!”",
            "pin [4829]",
        ] {
            assert_eq!(perche_non_si_salva(t), Some("una credenziale in chiaro"), "{t}");
        }
    }
}

#[cfg(test)]
mod prove_numeri {
    use super::*;

    /// Dopo «token» un numero e' una misura; dopo pin, otp e password resta
    /// un segreto (D408).
    #[test]
    fn un_numero_e_un_segreto_solo_dopo_le_chiavi_a_cifre() {
        for t in [
            "i token passano da 1532 a 980",
            "la finestra e' di 128.000 token, e i token in ingresso 1.532,5",
            "il secret ha 2048 bit",
        ] {
            assert_eq!(perche_non_si_salva(t), None, "{t}");
        }
        for t in [
            "la password e' 12345678",
            "otp 482913",
            "il pin del bancomat e' 4829",
            "la parola d'ordine e' 2026",
        ] {
            assert_eq!(perche_non_si_salva(t), Some("una credenziale in chiaro"), "{t}");
        }
        // Lunghissimo resta sospetto: qui lo prende gia' la forma della carta.
        assert!(perche_non_si_salva("token 1234567890123456").is_some());
        assert!(e_un_numero("1.532,5") && e_un_numero("128_000") && !e_un_numero("9f8e") && !e_un_numero(".,"));
    }
}

#[cfg(test)]
mod prove_parole {
    use super::*;

    /// Il piano 3 di S0ex1 (D409): l'elisione, una parola lunga, una domanda.
    #[test]
    fn le_parole_restano_parole() {
        for t in [
            "bit per token, il costo della legenda e l'argomento",
            "token, quanto la preelaborazione del modello (ridimensionamento, ricampionamento)",
            "per token, invece di contarle con un sostituto?",
            "il token dell'utente e' Contemporaneamente scaduto",
        ] {
            assert_eq!(perche_non_si_salva(t), None, "{t}");
        }
        for t in [
            "il token e' CorrectHorseBatteryStaple",
            "password: Tramonto2026?",
            "token: abc'12def",
        ] {
            assert_eq!(perche_non_si_salva(t), Some("una credenziale in chiaro"), "{t}");
        }
        assert!(e_una_parola("l'argomento") && e_una_parola("dell’utente"));
        for t in [
            "token stabili che llama.cpp",
            "token. Adesso `strumenti_in_http::DAL_PYTHON`",
            "credenziali e configurazione vivono in `%APPDATA%\\NOVA`.",
            "le credenziali **verrà** chiesta",
        ] {
            assert_eq!(perche_non_si_salva(t), None, "{t}");
        }
        for t in [
            "token: abc_def123",
            "il token e' AbCdEf-GhIjKl-MnOpQr",
            "secret: alfa/beta9",
            "password = Correct-Horse-Battery-Stapl",
        ] {
            assert!(perche_non_si_salva(t).is_some(), "{t}");
        }
        assert!(e_un_nome("llama.cpp") && e_un_nome("api_key") && e_un_nome("DAL_PYTHON"));
        assert!(!e_un_nome("abc_def123") && !e_un_nome("AbCdEf-Gh") && !e_un_nome("parola"));
        assert!(!e_una_parola("'argomento") && !e_una_parola("l''a") && !e_una_parola("a1"));
    }
}
