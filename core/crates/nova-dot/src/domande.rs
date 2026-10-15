//! Le domande dei Dot mentre lavorano (D419).
//!
//! Deciso con Gio il 14 ottobre: «la comunicazione si dimostra fondamentale
//! per non rifare le cose 100 volte». Quel giorno la fase 1 di S0ex1 si e'
//! fermata su un parere del legale che nessuno aveva chiesto: il compito
//! 1.25 lo richiedeva, il Dot che lo faceva non aveva modo di chiederlo, e
//! la prima volta se l'e' scritto da solo. E i Dot che lavorano con una CLI
//! qualunque non hanno gli strumenti di NOVA: la chat del progetto del D416
//! (`dot.scrivi`) per loro non c'era.
//!
//! Adesso un Dot, in qualunque turno di un compito di un progetto, puo'
//! chiudere la risposta con delle righe che NOVA legge, con qualunque
//! cervello:
//!
//! - `CHIEDO A <chi>: <domanda>` — al legale, all'APM o a un collega della
//!   squadra. Quel turno non e' la consegna: NOVA porta la domanda, e la
//!   risposta torna al Dot nello stesso compito. Al massimo
//!   [`GIRI_PER_COMPITO`] volte.
//! - `ALLA CHAT: <messaggio>` — nella chat del progetto, senza aspettare:
//!   gli altri lo leggono al loro prossimo compito, e lo legge Gio.
//!
//! Dal D421 una domanda a un collega e' una conversazione fra loro due: va
//! nella sua posta, e risponde lui, con una riga `RISPONDO A <chi>: ...`,
//! fra un turno e l'altro del suo lavoro o, se e' libero, subito. Dal D423
//! un Dot che trova la scheda video occupata scrive `ASPETTO LA SCHEDA
//! VIDEO: <il programma>`, e riprende quando quel programma e' chiuso.
//!
//! Qui ci sono le parole e come si leggono; chi fa rispondere e chi scrive
//! nella chat e' `nova_core::domande`.

use crate::azienda;

/// Come comincia la riga di una domanda.
pub const CHIEDO: &str = "CHIEDO A";

/// Come comincia la riga di un messaggio alla chat del progetto.
pub const ALLA_CHAT: &str = "ALLA CHAT";

/// Come comincia la risposta a un collega che ha chiesto (D421).
pub const RISPONDO: &str = "RISPONDO A";

/// La riga di chi aspetta che la scheda video sia libera (D423).
pub const SCHEDA: &str = "ASPETTO LA SCHEDA VIDEO";

/// Quante volte, in un compito, un Dot puo' fermarsi a chiedere.
pub const GIRI_PER_COMPITO: usize = 3;

/// Quante domande per volta: oltre si risponde alle prime, e lo si dice.
pub const DOMANDE_PER_GIRO: usize = 3;

/// Quanto di una risposta torna al Dot.
pub const RISPOSTA_AL_DOT: usize = 6_000;

/// Quanti gettoni ha chi risponde.
pub const GETTONI: u32 = 1_200;

/// Il segno in testa alla domanda a chi risponde.
pub const SEGNO: &str = "[domanda di un Dot]";

/// Una domanda di un Dot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Domanda {
    /// A chi, come l'ha scritto il Dot: minuscolo, senza virgolette.
    pub a: String,
    pub testo: String,
}

/// Quello che un Dot ha detto in un turno, diviso: le domande, le risposte
/// ai colleghi, i messaggi alla chat, se aspetta la scheda video, e il
/// resto, che e' la consegna se non ci sono domande.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Detto {
    pub domande: Vec<Domanda>,
    /// Le risposte ai colleghi (D421): `a` e' chi aveva chiesto.
    pub risposte: Vec<Domanda>,
    pub alla_chat: Vec<String>,
    /// Aspetta la scheda video (D423): il programma che la occupa, o vuoto
    /// se non l'ha detto.
    pub scheda: Option<String>,
    pub resto: String,
}

/// La riga, senza quello che il Markdown ci mette intorno: `**`, un trattino
/// d'elenco, una citazione.
fn pulita(riga: &str) -> String {
    riga.replace("**", "")
        .replace('`', "")
        .trim()
        .trim_start_matches(['-', '*', '>'])
        .trim()
        .to_string()
}

/// Se la riga comincia con `segno` (maiuscole o no), quello che c'e' dopo.
fn dopo<'a>(riga: &'a str, segno: &str) -> Option<&'a str> {
    let testa = riga.get(..segno.len())?;
    testa
        .eq_ignore_ascii_case(segno)
        .then(|| &riga[segno.len()..])
}

/// Se la riga apre una domanda (`CHIEDO A`) o una risposta (`RISPONDO A`):
/// a chi, e la prima riga (che puo' essere vuota, se il testo comincia
/// sotto).
fn apre(p: &str, segno: &str) -> Option<(String, String)> {
    let x = dopo(p, segno)?;
    // «CHIEDO A legale: …», «CHIEDO AL legale: …», «CHIEDO ALL'APM: …»
    let x = x
        .strip_prefix("LL'")
        .or_else(|| x.strip_prefix("ll'"))
        .or_else(|| x.strip_prefix('L').or_else(|| x.strip_prefix('l')))
        .unwrap_or(x);
    let (a, domanda) = x.split_once(':')?;
    let a = a
        .trim()
        .trim_matches(['«', '»', '"', '\'', '<', '>'])
        .trim()
        .to_lowercase();
    (!a.is_empty()).then(|| (a, domanda.trim().to_string()))
}

/// Se la riga dice che aspetta la scheda video: il programma che la occupa,
/// o vuoto.
fn aspetta_scheda(p: &str) -> Option<String> {
    let x = dopo(p, SCHEDA)?.trim();
    let nome = x.strip_prefix(':').unwrap_or(x).trim();
    Some(
        nome.trim_matches(['«', '»', '"', '\'', '.'])
            .trim()
            .to_string(),
    )
}

/// Se la riga e' un messaggio alla chat, il messaggio.
fn alla_chat(p: &str) -> Option<String> {
    let m = dopo(p, ALLA_CHAT)?.trim_start().strip_prefix(':')?.trim();
    (!m.is_empty()).then(|| m.to_string())
}

/// Divide quello che il Dot ha detto.
///
/// Una domanda comincia con la riga `CHIEDO A <chi>:` e continua sulle righe
/// sotto, fino alla domanda dopo o a un'altra riga per NOVA (D420): il 14
/// ottobre il testo dei termini di Antigravity, mandato al legale su piu'
/// righe, e' arrivato tagliato alla prima. Lo stesso una risposta a un
/// collega, `RISPONDO A <chi>:` (D421). Un messaggio alla chat e la riga
/// della scheda video sono una riga sola. Prima della prima domanda o
/// risposta, le righe dentro un blocco di codice non contano: una consegna
/// che spiega il formato non e' una domanda.
pub fn dividi(t: &str) -> Detto {
    let mut d = Detto::default();
    let mut resto = Vec::new();
    let mut in_codice = false;
    // Quello che si sta leggendo: se e' una risposta, a chi, la riga
    // com'era, e le righe.
    type Aperta = Option<(bool, String, String, Vec<String>)>;
    let mut aperta: Aperta = None;
    let chiudi = |aperta: &mut Aperta, d: &mut Detto, resto: &mut Vec<String>| {
        if let Some((risposta, a, com_era, righe)) = aperta.take() {
            let testo = righe.join("\n").trim().to_string();
            if testo.is_empty() {
                // Una domanda senza niente dentro non e' una domanda.
                resto.push(com_era);
            } else if risposta {
                d.risposte.push(Domanda { a, testo });
            } else {
                d.domande.push(Domanda { a, testo });
            }
        }
    };
    for riga in t.lines() {
        let codice = riga.trim_start().starts_with("```");
        if !in_codice {
            let p = pulita(riga);
            let nuova = apre(&p, CHIEDO)
                .map(|x| (false, x))
                .or_else(|| apre(&p, RISPONDO).map(|x| (true, x)));
            if let Some((risposta, (a, prima))) = nuova {
                chiudi(&mut aperta, &mut d, &mut resto);
                aperta = Some((risposta, a, riga.to_string(), vec![prima]));
                continue;
            }
            if let Some(m) = alla_chat(&p) {
                chiudi(&mut aperta, &mut d, &mut resto);
                d.alla_chat.push(m);
                continue;
            }
            if let Some(nome) = aspetta_scheda(&p) {
                chiudi(&mut aperta, &mut d, &mut resto);
                d.scheda = Some(nome);
                continue;
            }
        }
        if codice {
            in_codice = !in_codice;
        }
        match aperta.as_mut() {
            Some((_, _, _, righe)) => righe.push(riga.to_string()),
            None => resto.push(riga.to_string()),
        }
    }
    chiudi(&mut aperta, &mut d, &mut resto);
    d.resto = resto.join("\n").trim().to_string();
    d
}

/// A chi va una domanda.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Chi {
    Legale,
    Apm,
    /// Un collega della squadra del progetto.
    Collega(String),
}

/// A chi va una domanda, fra quelli a cui si puo' chiedere: il legale,
/// l'APM, e la squadra del progetto senza chi chiede. Il nome sbagliato
/// torna un errore che dice a chi si puo'.
pub fn chi(a: &str, chiede: &str, squadra: &[String]) -> Result<Chi, String> {
    let a = a
        .trim()
        .trim_start_matches("il ")
        .trim_start_matches("l'")
        .trim();
    if a == azienda::NOME_LEGALE {
        return Ok(Chi::Legale);
    }
    if a == azienda::NOME_APM {
        return Ok(Chi::Apm);
    }
    if a == chiede {
        return Err("non puoi chiedere a te stesso".into());
    }
    if squadra.iter().any(|s| s == a) {
        return Ok(Chi::Collega(a.to_string()));
    }
    Err(format!(
        "«{a}» non e' fra quelli a cui puoi chiedere: {}",
        a_chi_si_chiede(chiede, squadra)
    ))
}

/// A chi si puo' chiedere, detto in una riga.
pub fn a_chi_si_chiede(chiede: &str, squadra: &[String]) -> String {
    let colleghi: Vec<&str> = squadra
        .iter()
        .map(String::as_str)
        .filter(|s| *s != chiede && *s != azienda::NOME_LEGALE && *s != azienda::NOME_APM)
        .collect();
    if colleghi.is_empty() {
        format!("{}, {}", azienda::NOME_LEGALE, azienda::NOME_APM)
    } else {
        format!(
            "{}, {}, {}",
            azienda::NOME_LEGALE,
            azienda::NOME_APM,
            colleghi.join(", ")
        )
    }
}

/// La regola per chiedere e per scrivere alla chat, in coda al testo di un
/// compito di un progetto.
pub fn regola(chiede: &str, squadra: &[String]) -> String {
    format!(
        "\n\nSe per andare avanti ti serve una risposta che non hai (un parere del legale, una \
         decisione dell'APM, un chiarimento da un collega), non inventarla e non scriverla tu al \
         posto suo: chiedila. Chiudi la risposta con una riga per domanda, cosi': \
         `{CHIEDO} <chi>: <la domanda>`, dove <chi> e' uno fra {a_chi}; la domanda puo' \
         continuare sulle righe sotto, fino alla domanda dopo, e le domande vanno in fondo. \
         Quel turno non e' la consegna: NOVA porta la domanda, la risposta ti torna in questo stesso compito, e \
         allora continui. Puoi farlo al massimo {GIRI_PER_COMPITO} volte per compito. Per dire \
         qualcosa a tutta la squadra senza aspettare, scrivi una riga `{ALLA_CHAT}: <il \
         messaggio>`: va nella chat del progetto, gli altri la leggono al loro prossimo \
         compito, e la legge Gio. Se ti serve la scheda video e la occupa un altro programma, \
         non consegnare a vuoto: scrivi una riga `{SCHEDA}: {COME_SI_ASPETTA}`, col PID se lo \
         vedi, e NOVA ti fa riprendere quando quel programma e' chiuso.",
        a_chi = a_chi_si_chiede(chiede, squadra),
    )
}

/// Taglia un testo a `massimo` caratteri, e lo dice.
pub fn tagliato(t: &str, massimo: usize) -> String {
    let t = t.trim();
    if t.chars().count() <= massimo {
        return t.to_string();
    }
    let corto: String = t.chars().take(massimo).collect();
    format!("{}… (tagliato)", corto.trim_end())
}

/// Quello che serve a chi risponde, oltre alla domanda.
pub struct Contesto<'a> {
    pub progetto: &'a str,
    /// Cosa vuole l'utente.
    pub richiesta: &'a str,
    /// Chi chiede, e a quale compito lavora.
    pub chiede: &'a str,
    pub compito: &'a str,
}

/// La domanda a chi risponde. `chi_e` dice chi e' (il legale, l'APM, un
/// collega col suo ruolo), `sa` quello che sa del progetto: lo stato per
/// l'APM, le sue consegne per un collega.
pub fn domanda_a(chi: &Chi, chi_e: &str, sa: &str, x: &Contesto, domanda: &str) -> String {
    let come = match chi {
        Chi::Legale => {
            "Rispondi da legale: un parere chiaro sulla domanda, con la norma o la licenza che \
             conta. Nel dubbio lo dici, e dici cosa servirebbe per toglierlo. Comincia con una \
             riga `ESITO: ok`, `ESITO: problemi` o `ESITO: non so`, poi il parere."
        }
        Chi::Apm => {
            "Rispondi da APM: decidi se la decisione e' tua. Se e' dell'utente (il budget, le \
             scelte inventive, una spesa) dillo, e non decidere al posto suo."
        }
        Chi::Collega(_) => {
            "Rispondi da collega, con quello che sai dalle tue consegne. Se non lo sai, dillo, \
             e di' dove guarderesti: non inventare."
        }
    };
    let sa = if sa.trim().is_empty() {
        String::new()
    } else {
        format!("\n\n{}", sa.trim())
    };
    format!(
        "{SEGNO} {chi_e}, nell'azienda dei Dot, nel progetto «{progetto}».\n\n\
         Cosa vuole l'utente:\n{richiesta}{sa}\n\n\
         «{chiede}» lavora a questo compito:\n{compito}\n\n\
         E ti chiede:\n{domanda}\n\n{come} Non hai i suoi file, e non ti servono: rispondi alla \
         domanda, in poche righe.",
        progetto = x.progetto,
        richiesta = tagliato(x.richiesta, 2_000),
        chiede = x.chiede,
        compito = tagliato(x.compito, 3_000),
        domanda = domanda.trim(),
    )
}

/// Il messaggio con cui il Dot riprende dopo le risposte. `rimaste` e'
/// quante volte puo' ancora chiedere.
pub fn risposte(r: &[(Domanda, String)], rimaste: usize) -> String {
    let mut t = String::from("Le risposte alle tue domande:");
    for (d, risposta) in r {
        t.push_str(&format!(
            "\n\n- A {}: «{}»\n  Risposta: {}",
            d.a,
            d.testo.trim(),
            tagliato(risposta, RISPOSTA_AL_DOT).replace('\n', "\n  ")
        ));
    }
    t.push_str("\n\nAdesso continua il compito con queste risposte.");
    t.push_str(&if rimaste == 0 {
        " Non puoi piu' chiedere in questo compito: quello che manca ancora scrivilo nella \
         consegna, in «Cosa non so»."
            .to_string()
    } else {
        format!(" Se ti serve, puoi chiedere ancora {rimaste} volte.")
    });
    t
}

/// Quando ha chiesto anche se non poteva piu': si va avanti senza.
pub const BASTA: &str = "Non puoi piu' chiedere in questo compito. Consegna con quello che hai, e \
quello che manca scrivilo in «Cosa non so».";

/// Le domande dei colleghi che aspettano la risposta di un Dot (D421), in
/// coda al messaggio di un suo turno: chi chiede, e cosa. Vuoto se non ce
/// ne sono. Si ripetono a ogni turno finche' non risponde: chi chiede sta
/// aspettando.
pub fn domande_per_te(domande: &[(String, String)]) -> String {
    if domande.is_empty() {
        return String::new();
    }
    let mut t = String::from("\n\nDomande dei colleghi, che aspettano la tua risposta:");
    for (da, q) in domande {
        t.push_str(&format!(
            "\n- da {da}: «{}»",
            tagliato(q, RISPOSTA_AL_DOT).replace('\n', "\n  ")
        ));
    }
    t.push_str(&format!(
        "\nRispondi a ognuno con una riga `{RISPONDO} <chi>: <la risposta>`, anche su piu' \
         righe, con quello che sai e che vedi nei tuoi file. Le risposte vanno in fondo, dopo \
         tutto il resto: una risposta continua fino alla riga per NOVA dopo, o alla fine. Poi \
         continua quello che stavi facendo."
    ));
    t
}

/// Il messaggio con cui si chiede a un Dot libero di rispondere ai colleghi
/// (D421): non ha un compito, e loro aspettano.
pub fn rispondi_ai_colleghi(domande: &[(String, String)]) -> String {
    format!(
        "Non hai compiti adesso, ma dei colleghi ti hanno fatto una domanda.{}",
        domande_per_te(domande)
    )
}

/// Quando un Dot ha risposto ai colleghi e basta, senza andare avanti: si
/// continua (D421).
pub const HAI_RISPOSTO: &str = "Hai risposto ai colleghi. Adesso continua il tuo compito.";

/// Quante volte, in un compito, si aspetta la scheda video (D423).
pub const ATTESE_DELLA_SCHEDA: usize = 3;

/// Come si scrive chi occupa la scheda video, dopo `ASPETTO LA SCHEDA
/// VIDEO:` (D425).
pub const COME_SI_ASPETTA: &str = "<il programma che la occupa> (PID <il numero>)";

/// Chi occupa la scheda video, come l'ha scritto il Dot (D425): il nome del
/// programma e i PID, se li ha scritti.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Occupata {
    pub programma: String,
    pub pid: Vec<u32>,
}

/// Legge chi occupa la scheda video da quello che il Dot ha scritto dopo
/// `ASPETTO LA SCHEDA VIDEO:` (D425): «python.exe (PID 13008, 17312)»,
/// «PID 13008», «League of Legends.exe». Il 15 ottobre un Dot ha scritto
/// solo «python.exe», e sul PC erano accesi altri quattro Python che con la
/// scheda non c'entravano: col PID si aspetta proprio quello.
pub fn chi_occupa(detto: &str) -> Occupata {
    // Minuscole ASCII: gli indici restano quelli del testo.
    let basso = detto.to_ascii_lowercase();
    let b = basso.as_bytes();
    let mut pid: Vec<u32> = Vec::new();
    let mut nome_fino = detto.find('(').unwrap_or(detto.len());
    let mut da = 0;
    while let Some(i) = basso[da..].find("pid").map(|i| da + i) {
        let mut j = i + 3;
        da = j;
        if (i > 0 && b[i - 1].is_ascii_alphanumeric())
            || (j < b.len() && b[j].is_ascii_alphanumeric())
        {
            continue;
        }
        nome_fino = nome_fino.min(i);
        loop {
            while j < b.len() && matches!(b[j], b' ' | b':' | b'=' | b'#' | b',' | b';') {
                j += 1;
            }
            let k = j;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            if j > k {
                if let Ok(n) = basso[k..j].parse::<u32>() {
                    if !pid.contains(&n) {
                        pid.push(n);
                    }
                }
                continue;
            }
            if basso[j..].starts_with("e ") || basso[j..].starts_with("and ") {
                j += if basso[j..].starts_with('e') { 2 } else { 4 };
                continue;
            }
            break;
        }
        da = j;
    }
    let programma = detto[..nome_fino]
        .trim()
        .trim_end_matches([',', ';', ':', '-', '(', ' '])
        .trim_matches(['«', '»', '"', '\'', '`', '.', ' '])
        .trim()
        .to_string();
    Occupata { programma, pid }
}

/// «1 minuto», «60 minuti».
fn in_minuti(n: u64) -> String {
    if n == 1 {
        "1 minuto".to_string()
    } else {
        format!("{n} minuti")
    }
}

/// Chi occupa la scheda, a parole: «python.exe» (PID 13008), il programma
/// col PID 13008, «gioco».
pub fn descrivi(o: &Occupata) -> String {
    let pid: Vec<String> = o.pid.iter().map(u32::to_string).collect();
    match (o.programma.is_empty(), pid.is_empty()) {
        (false, true) => format!("«{}»", o.programma),
        (false, false) => format!("«{}» (PID {})", o.programma, pid.join(", ")),
        (true, false) => format!("il programma col PID {}", pid.join(", ")),
        (true, true) => "il programma".to_string(),
    }
}

/// Il messaggio con cui si riprende quando il programma che occupava la
/// scheda video e' chiuso (D423).
pub fn scheda_libera(o: &Occupata) -> String {
    format!(
        "{} non e' piu' acceso: la scheda video e' libera. Riguardala, e continua il compito.",
        descrivi(o)
    )
}

/// Il messaggio con cui si riprende quando il programma nominato senza PID
/// e' ancora acceso dopo un'ora (D425): con lo stesso nome possono esserci
/// programmi che con la scheda non c'entrano, e la riguarda il Dot.
pub fn scheda_forse_altro(o: &Occupata, minuti: u64) -> String {
    format!(
        "{} e' ancora acceso dopo {}, ma con questo nome possono esserci anche \
         programmi che con la scheda video non c'entrano. Riguarda se la scheda e' davvero \
         occupata: se e' libera, continua il compito; se no, scrivi di nuovo `{SCHEDA}: \
         {COME_SI_ASPETTA}`, col PID di chi la occupa.",
        descrivi(o),
        in_minuti(minuti)
    )
}

/// Il messaggio con cui si riprende dopo un'attesa senza sapere cosa
/// occupava la scheda (D423): il Dot non l'ha detto, o NOVA non sa vedere i
/// programmi accesi.
pub fn scheda_da_riguardare(minuti: u64) -> String {
    let passati = if minuti == 1 {
        "E' passato 1 minuto".to_string()
    } else {
        format!("Sono passati {minuti} minuti")
    };
    format!(
        "{passati}. Riguarda se la scheda video e' libera: se si', continua il compito; se no, \
         scrivi di nuovo `{SCHEDA}: {COME_SI_ASPETTA}`."
    )
}

/// Il messaggio con cui si riprende quando la scheda e' rimasta occupata
/// oltre l'attesa massima (D423): il compito va com'e'.
pub fn scheda_ancora_occupata(o: &Occupata, ore: u64) -> String {
    format!(
        "{} e' ancora acceso dopo {ore} ore. Non aspettare oltre: consegna quello che hai, e \
         scrivi in «Cosa non so» che la misura non e' fatta perche' la scheda era occupata.",
        descrivi(o)
    )
}

/// Se un programma acceso e' quello che il Dot ha nominato: lo stesso nome,
/// maiuscole o no, con o senza `.exe`.
pub fn stesso_programma(acceso: &str, nominato: &str) -> bool {
    let n = |t: &str| {
        let t = t.trim().to_lowercase();
        t.strip_suffix(".exe").unwrap_or(&t).trim().to_string()
    };
    let (a, b) = (n(acceso), n(nominato));
    !b.is_empty() && a == b
}

/// Quello che torna a chi ha chiesto a un collega che non ha risposto in
/// tempo (D421): risponde NOVA dalle sue consegne, e lo dice.
pub fn non_ha_risposto(collega: &str, minuti: u64, ripiego: &str) -> String {
    let quanto = in_minuti(minuti);
    format!(
        "(«{collega}» non ha risposto entro {quanto}: risponde NOVA dalle sue consegne.) {}",
        ripiego.trim()
    )
}

/// Quello che torna a chi ha chiesto a un collega che sta aspettando una
/// risposta proprio da lui (D421): non si aspettano a vicenda, e risponde
/// NOVA dalle consegne del collega.
pub fn aspetta_te(collega: &str, ripiego: &str) -> String {
    format!(
        "(«{collega}» sta aspettando una tua risposta, e non puo' risponderti adesso: risponde \
         NOVA dalle sue consegne.) {}",
        ripiego.trim()
    )
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn le_righe_si_leggono_e_il_resto_resta() {
        let t = "Ho fatto la prima parte.\n\
                 **CHIEDO A legale:** il corpus Gutenberg si puo' usare per il banco?\n\
                 - CHIEDO ALL'APM: chi decide il budget?\n\
                 chiedo al «dati»: dove sono i conteggi di 1.7?\n\
                 ALLA CHAT: ho salvato le fonti in materiale/fonti\n\
                 Fine.";
        let d = dividi(t);
        assert_eq!(
            d.domande,
            vec![
                Domanda {
                    a: "legale".into(),
                    testo: "il corpus Gutenberg si puo' usare per il banco?".into()
                },
                Domanda {
                    a: "apm".into(),
                    testo: "chi decide il budget?".into()
                },
                Domanda {
                    a: "dati".into(),
                    testo: "dove sono i conteggi di 1.7?".into()
                },
            ]
        );
        assert_eq!(
            d.alla_chat,
            vec!["ho salvato le fonti in materiale/fonti".to_string()]
        );
        assert_eq!(d.resto, "Ho fatto la prima parte.\nFine.");
    }

    #[test]
    fn nel_codice_e_senza_domanda_non_e_una_domanda() {
        let t = "Il formato:\n```\nCHIEDO A legale: esempio\nALLA CHAT: esempio\n```\n\
                 CHIEDO A : vuota\nALLA CHAT:\nChiedo a tutti di leggere.\nCHIEDO A legale:";
        let d = dividi(t);
        assert!(d.domande.is_empty(), "{:?}", d.domande);
        assert!(d.alla_chat.is_empty());
        assert_eq!(d.resto, t, "anche la domanda vuota in fondo resta com'era");
    }

    #[test]
    fn una_domanda_continua_sulle_righe_sotto() {
        // D420: il 14 ottobre i termini mandati al legale su piu' righe sono
        // arrivati tagliati alla prima.
        let t = "Premessa.\n\
                 CHIEDO A legale: ecco i termini, alla lettera:\n\
                 ```\nYou may not use the Services\nALLA CHAT: dentro il codice non conta\n```\n\
                 Si applicano ad Antigravity?\n\
                 CHIEDO A apm:\nchi decide il budget?\n\
                 ALLA CHAT: fatto\n\
                 Fine.";
        let d = dividi(t);
        assert_eq!(
            d.domande,
            vec![
                Domanda {
                    a: "legale".into(),
                    testo: "ecco i termini, alla lettera:\n```\nYou may not use the Services\n\
                            ALLA CHAT: dentro il codice non conta\n```\nSi applicano ad Antigravity?"
                        .into()
                },
                Domanda {
                    a: "apm".into(),
                    testo: "chi decide il budget?".into()
                },
            ]
        );
        assert_eq!(d.alla_chat, vec!["fatto".to_string()]);
        assert_eq!(d.resto, "Premessa.\nFine.");
    }

    #[test]
    fn le_risposte_ai_colleghi_e_la_scheda_video() {
        // D421: chi ha ricevuto una domanda risponde con RISPONDO A, anche su
        // piu' righe, e continua il suo lavoro.
        let t = "Ho misurato.\n\
                 RISPONDO A dati: le misure di 1.1\nstanno in materiale/misure.csv\n\
                 **Rispondo a** «qualita»: si', ripetile\n\
                 ALLA CHAT: misure pronte\n\
                 Continuo.";
        let d = dividi(t);
        assert_eq!(
            d.risposte,
            vec![
                Domanda {
                    a: "dati".into(),
                    testo: "le misure di 1.1\nstanno in materiale/misure.csv".into()
                },
                Domanda {
                    a: "qualita".into(),
                    testo: "si', ripetile".into()
                },
            ]
        );
        assert!(d.domande.is_empty());
        assert_eq!(d.alla_chat, vec!["misure pronte".to_string()]);
        assert_eq!(d.resto, "Ho misurato.\nContinuo.");
        // D423: la scheda video, col programma o senza.
        let d = dividi("La GPU e' occupata.\nASPETTO LA SCHEDA VIDEO: League of Legends.exe");
        assert_eq!(d.scheda, Some("League of Legends.exe".into()));
        assert_eq!(d.resto, "La GPU e' occupata.");
        assert_eq!(
            dividi("- **aspetto la scheda video**").scheda,
            Some(String::new())
        );
        assert_eq!(
            dividi("Aspetto la scheda video: «gioco».").scheda,
            Some("gioco".into())
        );
        assert_eq!(dividi("Niente da aspettare.").scheda, None);
        let r = regola("dati", &[]);
        assert!(
            r.contains("`ASPETTO LA SCHEDA VIDEO: <il programma che la occupa> (PID <il numero>)`, col PID"),
            "{r}"
        );
    }

    #[test]
    fn come_si_risponde_a_un_collega() {
        assert_eq!(domande_per_te(&[]), "");
        let t = domande_per_te(&[
            ("dati".into(), "dove sono\nle misure?".into()),
            ("ricerca".into(), "quale fonte?".into()),
        ]);
        assert!(
            t.starts_with(
                "\n\nDomande dei colleghi, che aspettano la tua risposta:\n\
                           - da dati: «dove sono\n  le misure?»\n- da ricerca: «quale fonte?»"
            ),
            "{t}"
        );
        assert!(t.contains("`RISPONDO A <chi>: <la risposta>`"), "{t}");
        assert!(
            t.contains("Le risposte vanno in fondo, dopo tutto il resto"),
            "{t}"
        );
        // Quello che insegna, dividi lo legge.
        assert_eq!(dividi("RISPONDO A dati: si'").risposte.len(), 1);
        let libero = rispondi_ai_colleghi(&[("dati".into(), "dove?".into())]);
        assert!(
            libero.starts_with(
                "Non hai compiti adesso, ma dei colleghi ti hanno fatto una domanda.\n\nDomande"
            ),
            "{libero}"
        );
        assert!(non_ha_risposto("qualita", 1, "x").contains("entro 1 minuto:"));
        assert!(
            aspetta_te("dati", " forse ").starts_with("(«dati» sta aspettando una tua risposta")
        );
        assert!(aspetta_te("dati", " forse ").ends_with("dalle sue consegne.) forse"));
        let n = non_ha_risposto("qualita", 30, " stanno in materiale ");
        assert_eq!(
            n,
            "(«qualita» non ha risposto entro 30 minuti: risponde NOVA dalle sue consegne.) stanno in materiale"
        );
    }

    #[test]
    fn la_scheda_video() {
        assert!(stesso_programma(
            "League of Legends.exe",
            "league of legends"
        ));
        assert!(stesso_programma("giocofinto", "GiocoFinto.exe"));
        assert!(
            !stesso_programma("League of Legends.exe", "League"),
            "il nome intero, non un pezzo"
        );
        assert!(
            !stesso_programma("chrome.exe", ""),
            "senza nome non combacia niente"
        );
        let gioco = chi_occupa("gioco.exe");
        assert!(scheda_libera(&gioco).starts_with("«gioco.exe» non e' piu' acceso"));
        assert!(
            scheda_da_riguardare(15).contains("Sono passati 15 minuti")
                && scheda_da_riguardare(15).contains(
                    "`ASPETTO LA SCHEDA VIDEO: <il programma che la occupa> (PID <il numero>)`"
                )
        );
        assert!(scheda_ancora_occupata(&gioco, 6).contains("dopo 6 ore"));
        assert!(scheda_da_riguardare(1).starts_with("E' passato 1 minuto. Riguarda"));
        assert!(scheda_forse_altro(&gioco, 1).contains("acceso dopo 1 minuto, ma"));
    }

    #[test]
    fn chi_occupa_la_scheda_col_pid() {
        // D425: il 15 ottobre un Dot ha scritto «python.exe», e altri quattro
        // Python accesi l'hanno fatto aspettare per niente.
        let o = |p: &str, pid: &[u32]| Occupata {
            programma: p.into(),
            pid: pid.to_vec(),
        };
        assert_eq!(
            chi_occupa("python.exe (PID 13008, 17312)"),
            o("python.exe", &[13008, 17312])
        );
        assert_eq!(
            chi_occupa("python.exe, pid: 13008 e PID 17312"),
            o("python.exe", &[13008, 17312])
        );
        assert_eq!(chi_occupa("PID 13008"), o("", &[13008]));
        assert_eq!(chi_occupa("pid=42 and 43"), o("", &[42, 43]));
        assert_eq!(
            chi_occupa("«League of Legends.exe»."),
            o("League of Legends.exe", &[])
        );
        assert_eq!(
            chi_occupa("python.exe (il banco di 1.34)"),
            o("python.exe", &[])
        );
        assert_eq!(
            chi_occupa("rapid.exe"),
            o("rapid.exe", &[]),
            "«pid» in una parola no"
        );
        assert_eq!(chi_occupa(""), o("", &[]));
        assert_eq!(
            descrivi(&chi_occupa("python.exe (PID 13008, 17312)")),
            "«python.exe» (PID 13008, 17312)"
        );
        assert_eq!(descrivi(&chi_occupa("PID 7")), "il programma col PID 7");
        let f = scheda_forse_altro(&chi_occupa("python.exe"), 60);
        assert!(
            f.starts_with("«python.exe» e' ancora acceso dopo 60 minuti")
                && f.contains("col PID di chi la occupa"),
            "{f}"
        );
    }

    #[test]
    fn si_chiede_solo_a_chi_c_e() {
        let squadra = vec!["dati".to_string(), "qualita".to_string()];
        assert_eq!(chi("legale", "dati", &squadra), Ok(Chi::Legale));
        assert_eq!(chi("il legale", "dati", &squadra), Ok(Chi::Legale));
        assert_eq!(chi("apm", "dati", &squadra), Ok(Chi::Apm));
        assert_eq!(
            chi("qualita", "dati", &squadra),
            Ok(Chi::Collega("qualita".into()))
        );
        assert!(chi("dati", "dati", &squadra)
            .unwrap_err()
            .contains("te stesso"));
        let e = chi("gio", "dati", &squadra).unwrap_err();
        assert!(e.contains("legale, apm, qualita"), "{e}");
        assert_eq!(a_chi_si_chiede("dati", &[]), "legale, apm");
    }

    #[test]
    fn la_regola_dice_come_e_a_chi() {
        let r = regola("dati", &["dati".into(), "qualita".into()]);
        assert!(r.contains("`CHIEDO A <chi>: <la domanda>`"), "{r}");
        assert!(r.contains("legale, apm, qualita"), "{r}");
        assert!(r.contains("`ALLA CHAT: <il messaggio>`"), "{r}");
        assert!(
            r.contains(&format!("al massimo {GIRI_PER_COMPITO} volte")),
            "{r}"
        );
        // Quello che la regola insegna, dividi lo legge.
        let d = dividi("CHIEDO A qualita: dove sono le misure?\nALLA CHAT: fatto");
        assert_eq!(d.domande.len(), 1);
        assert_eq!(d.alla_chat.len(), 1);
    }

    #[test]
    fn la_domanda_e_le_risposte() {
        let x = Contesto {
            progetto: "s0ex1",
            richiesta: "spazi compressi",
            chiede: "dati",
            compito: "1.25 completare il banco",
        };
        let q = domanda_a(
            &Chi::Legale,
            "Sei il legale",
            "",
            &x,
            "posso scaricare Qwen2-VL?",
        );
        assert!(q.starts_with(SEGNO), "{q}");
        assert!(
            q.contains("«dati» lavora a questo compito:\n1.25 completare il banco"),
            "{q}"
        );
        assert!(q.contains("posso scaricare Qwen2-VL?"), "{q}");
        assert!(q.contains("ESITO: ok"), "{q}");
        let q = domanda_a(&Chi::Apm, "Sei l'APM", "Fase 1 ferma", &x, "chi decide?");
        assert!(
            q.contains("Fase 1 ferma") && q.contains("non decidere al posto suo"),
            "{q}"
        );
        let d = Domanda {
            a: "legale".into(),
            testo: "si puo'?".into(),
        };
        let r = risposte(
            &[(d.clone(), "ESITO: ok\nsi', con licenza Apache".into())],
            2,
        );
        assert!(
            r.contains("- A legale: «si puo'?»\n  Risposta: ESITO: ok\n  si', con licenza Apache"),
            "{r}"
        );
        assert!(r.contains("ancora 2 volte"), "{r}");
        assert!(risposte(&[(d.clone(), "x".into())], 0).contains("Non puoi piu' chiedere"));
        let lunga = "a".repeat(RISPOSTA_AL_DOT + 10);
        assert!(risposte(&[(d, lunga)], 1).contains("… (tagliato)"));
    }
}
