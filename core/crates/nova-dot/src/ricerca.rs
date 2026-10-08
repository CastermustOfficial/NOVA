//! Il ricercatore: il piano, i passi, il revisore, le fonti (D383).
//!
//! Il primo Dot con un mestiere (D381, `docs/dots.md`). Riceve una domanda e
//! consegna un rapporto in Markdown con le fonti. Lavora cosi':
//!
//! 1. il cervello **piu' grande** della scala fa il piano: i passi, e per
//!    ognuno il cervello che lo fa ([`richiesta_del_piano`], [`piano`]);
//! 2. ogni passo e' un turno col cervello assegnato ([`richiesta_del_passo`]);
//! 3. il cervello grande **rivede** i passi fatti dai cervelli piu' piccoli
//!    ([`richiesta_della_revisione`], [`leggi_giudizio`]): uno scarso si rifa'
//!    un gradino piu' su;
//! 4. l'ultimo passo scrive il rapporto, e ogni indirizzo che cita si
//!    controlla contro quello che il Dot ha **letto davvero** con gli
//!    strumenti ([`controlla_fonti`], [`rapporto_su_disco`]).
//!
//! Qui ci sono solo testi e regole: i turni li fa `nova_core::ricercatore`.
//!
//! **Ogni richiesta porta con se' quello che serve.** I cervelli della scala
//! non vedono tutti la stessa conversazione: un cervello in HTTP riceve i
//! messaggi, Claude Code riceve solo l'ultima domanda e la sua sessione. Il
//! revisore deve leggere il risultato che giudica, e chi scrive il rapporto
//! deve sapere cosa hanno trovato i passi prima: per questo la richiesta li
//! contiene, invece di contare su una memoria che un cervello su due non ha.

use std::collections::BTreeSet;

use serde::Serialize;
use serde_json::Value;

/// Quanti passi al massimo ha un piano. Un piano piu' lungo si taglia: un
/// ricercatore che fa venti passi per una domanda ha sbagliato il piano.
pub const PASSI_MASSIMI: usize = 8;

/// Quanti turni puo' fare un passo. Un turno finisce quando il modello
/// risponde o quando ha usato tutti i suoi giri di strumenti; nel secondo
/// caso il passo va avanti, fino a qui.
pub const TURNI_PER_PASSO: usize = 3;

/// Quanto del risultato di un passo fatto entra nelle richieste dei passi
/// dopo. Il resto e' nella conversazione, per chi la vede.
pub const RISULTATO_IN_RICHIESTA: usize = 1500;

/// Quanto del risultato da giudicare entra nella richiesta al revisore.
pub const RISULTATO_DA_RIVEDERE: usize = 4000;

/// Quello che il prompt di un ricercatore dice in piu' di quello di un Dot.
pub const PROMPT: &str = "Sei un ricercatore: lavori a passi. Prima fai il piano, \
poi i passi uno alla volta, e alla fine consegni un rapporto in Markdown con le \
fonti. Cita solo pagine che hai letto davvero con gli strumenti in questo \
compito: ogni indirizzo del rapporto si controlla contro quello che hai letto.";

/// Che tipo di passo e'.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Genere {
    Cerca,
    Leggi,
    Confronta,
    Scrivi,
    /// Un tipo che il piano ha scritto e che non e' fra i quattro: il passo
    /// si fa lo stesso, e si dice che tipo era.
    Altro,
}

impl Genere {
    /// Il tipo da come lo scrive il piano.
    pub fn da(testo: &str) -> Genere {
        match testo.trim().to_lowercase().as_str() {
            "cerca" | "cercare" => Genere::Cerca,
            "leggi" | "leggere" => Genere::Leggi,
            "confronta" | "confrontare" => Genere::Confronta,
            "scrivi" | "scrivere" => Genere::Scrivi,
            _ => Genere::Altro,
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            Genere::Cerca => "cerca",
            Genere::Leggi => "leggi",
            Genere::Confronta => "confronta",
            Genere::Scrivi => "scrivi",
            Genere::Altro => "altro",
        }
    }
}

/// Chi ha scelto il cervello di un passo. E' quello che si registra per
/// insegnare a scegliere: una scelta del piano, un ripiego, una salita del
/// revisore sono tre cose diverse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SceltoDa {
    /// Il cervello grande, nel piano.
    Piano,
    /// Nessuno: il piano non si leggeva, o nominava un cervello che non c'e',
    /// e il passo va al cervello grande.
    Ripiego,
    /// Il revisore: il passo era scarso e si rifa' un gradino piu' su.
    Salita,
}

impl SceltoDa {
    pub fn nome(self) -> &'static str {
        match self {
            SceltoDa::Piano => "piano",
            SceltoDa::Ripiego => "ripiego",
            SceltoDa::Salita => "salita",
        }
    }
}

/// Un passo del piano.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Passo {
    pub genere: Genere,
    /// Cosa fare, come l'ha scritto il piano.
    pub cosa: String,
    /// Il nome del gradino che lo fa.
    pub cervello: String,
    pub scelto_da: SceltoDa,
    /// Perche' un ripiego, se lo e'.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub perche: String,
}

/// Il piano letto.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Piano {
    pub passi: Vec<Passo>,
    /// Vero se il piano viene dal cervello grande; falso se non si leggeva e
    /// questo e' il piano di ripiego.
    pub letto: bool,
    /// Cosa si e' corretto, o perche' non si leggeva.
    pub note: Vec<String>,
}

/// La domanda al cervello grande per il piano.
///
/// La scala si scrive dal piu' piccolo al piu' grande, con i nomi veri dei
/// gradini: sono quelli che il piano deve usare, e un nome inventato fa
/// ripiegare sul cervello grande.
pub fn richiesta_del_piano(compito: &str, scala: &[String]) -> String {
    let elenco: Vec<String> = scala
        .iter()
        .enumerate()
        .map(|(i, n)| format!("{}. {n}", i + 1))
        .collect();
    let esempio = scala.first().map(String::as_str).unwrap_or("");
    format!(
        "[piano] Prima di cominciare il compito, fai il piano.\n\
         Il compito: {compito}\n\n\
         Dividilo in passi, al massimo {PASSI_MASSIMI}. Ogni passo ha un tipo (cerca, \
         leggi, confronta o scrivi), cosa fare, e il cervello che lo fa. L'ultimo passo e' \
         «scrivi»: il rapporto in Markdown, con una sezione «## Fonti» che elenca gli \
         indirizzi delle pagine lette davvero.\n\n\
         I cervelli, dal piu' piccolo al piu' grande:\n{}\n\
         A ogni passo dai il piu' piccolo che lo sa fare bene: un revisore controlla i \
         passi dei cervelli piu' piccoli, e uno fatto male si rifa' un gradino piu' su.\n\n\
         Rispondi solo con il JSON, cosi':\n\
         {{\"passi\": [{{\"tipo\": \"cerca\", \"cosa\": \"...\", \"cervello\": \"{esempio}\"}}]}}",
        elenco.join("\n"),
        compito = compito.trim(),
    )
}

/// Il JSON dentro un testo: dalla prima graffa aperta all'ultima chiusa. I
/// modelli lo mettono fra tre apici, o con una frase davanti.
fn oggetto_dentro(testo: &str) -> Option<&str> {
    let inizio = testo.find('{')?;
    let fine = testo.rfind('}')?;
    (fine > inizio).then(|| &testo[inizio..=fine])
}

/// Il nome di un gradino come lo scrive la scala, se il piano ne ha scritto
/// uno che c'e'. Le maiuscole non contano: «Grande» e' «grande».
fn nella_scala(nome: &str, scala: &[String]) -> Option<String> {
    let n = nome.trim();
    scala
        .iter()
        .find(|g| g.as_str() == n)
        .or_else(|| scala.iter().find(|g| g.to_lowercase() == n.to_lowercase()))
        .cloned()
}

/// Legge il piano scritto dal cervello grande.
///
/// Un passo senza «cosa» si salta. Un cervello che non e' nella scala fa
/// ripiegare quel passo sul cervello grande, e si dice. Un piano piu' lungo
/// di [`PASSI_MASSIMI`] si taglia; uno che non finisce con «scrivi» ne
/// riceve uno, col cervello grande: senza, non c'e' un rapporto da
/// consegnare.
pub fn leggi_piano(testo: &str, scala: &[String]) -> Result<Piano, String> {
    let cima = scala
        .last()
        .cloned()
        .ok_or("la scala dei cervelli e' vuota")?;
    let json = oggetto_dentro(testo).ok_or("nel piano non c'e' un oggetto JSON")?;
    let v: Value =
        serde_json::from_str(json).map_err(|e| format!("il JSON del piano non si legge: {e}"))?;
    let voci = v
        .get("passi")
        .and_then(Value::as_array)
        .ok_or("nel piano non c'e' l'elenco «passi»")?;
    let mut note = Vec::new();
    let mut passi: Vec<Passo> = Vec::new();
    for voce in voci {
        let campo = |k: &str| voce.get(k).and_then(Value::as_str).unwrap_or("").trim();
        let cosa = campo("cosa");
        if cosa.is_empty() {
            note.push("un passo senza «cosa» si e' saltato".to_string());
            continue;
        }
        let scritto = campo("cervello");
        let (cervello, scelto_da, perche) = match nella_scala(scritto, scala) {
            Some(n) => (n, SceltoDa::Piano, String::new()),
            None => (
                cima.clone(),
                SceltoDa::Ripiego,
                format!("il cervello «{scritto}» non e' nella scala"),
            ),
        };
        passi.push(Passo {
            genere: Genere::da(campo("tipo")),
            cosa: cosa.to_string(),
            cervello,
            scelto_da,
            perche,
        });
    }
    if passi.is_empty() {
        return Err("il piano non ha passi".into());
    }
    if passi.len() > PASSI_MASSIMI {
        note.push(format!(
            "il piano aveva {} passi: tenuti i primi {PASSI_MASSIMI}",
            passi.len()
        ));
        passi.truncate(PASSI_MASSIMI);
    }
    if passi.last().map(|p| p.genere) != Some(Genere::Scrivi) {
        if passi.len() == PASSI_MASSIMI {
            passi.pop();
        }
        note.push("il piano non finiva con «scrivi»: aggiunto, col cervello grande".into());
        passi.push(Passo {
            genere: Genere::Scrivi,
            cosa: "Scrivi il rapporto con quello che hai trovato.".into(),
            cervello: cima,
            scelto_da: SceltoDa::Ripiego,
            perche: "il piano non finiva con «scrivi»".into(),
        });
    }
    Ok(Piano {
        passi,
        letto: true,
        note,
    })
}

/// Il piano: quello del cervello grande se si legge, se no quello di
/// ripiego, tutto col cervello grande. Il perche' resta nelle note.
pub fn piano(testo: &str, scala: &[String]) -> Piano {
    match leggi_piano(testo, scala) {
        Ok(p) => p,
        Err(perche) => {
            let cima = scala.last().cloned().unwrap_or_default();
            let passo = |genere, cosa: &str| Passo {
                genere,
                cosa: cosa.to_string(),
                cervello: cima.clone(),
                scelto_da: SceltoDa::Ripiego,
                perche: perche.clone(),
            };
            Piano {
                passi: vec![
                    passo(Genere::Cerca, "Cerca le fonti sul compito."),
                    passo(Genere::Leggi, "Leggi le fonti migliori che hai trovato."),
                    passo(
                        Genere::Scrivi,
                        "Scrivi il rapporto con quello che hai trovato.",
                    ),
                ],
                letto: false,
                note: vec![perche.clone()],
            }
        }
    }
}

/// Un testo tagliato a `quanti` caratteri, e il taglio si dichiara: chi
/// legge deve sapere che c'e' altro, non credere di aver letto tutto.
pub fn tagliato(testo: &str, quanti: usize) -> String {
    let t = testo.trim();
    if t.chars().count() <= quanti {
        return t.to_string();
    }
    let mut fuori: String = t.chars().take(quanti).collect();
    fuori.push_str(" [...]");
    fuori
}

/// Un passo gia' fatto, per le richieste dei passi dopo.
#[derive(Debug, Clone, PartialEq)]
pub struct Fatto {
    pub numero: usize,
    pub genere: Genere,
    pub testo: String,
}

/// La domanda per un passo.
///
/// `numero` parte da uno. `rifatto` dice chi l'aveva fatto prima e cosa ha
/// detto il revisore, quando il passo si rifa' un gradino piu' su.
pub fn richiesta_del_passo(
    numero: usize,
    quanti: usize,
    passo: &Passo,
    fatti: &[Fatto],
    rifatto: Option<(&str, &str)>,
) -> String {
    let mut t = format!(
        "[passo {numero} di {quanti}: {}] {}",
        passo.genere.nome(),
        passo.cosa.trim()
    );
    if passo.genere == Genere::Scrivi {
        t.push_str(
            "\nScrivi il rapporto intero in Markdown: comincia con un titolo «# ...» e chiudi \
             con «## Fonti», un indirizzo per riga, solo pagine che hai letto davvero in \
             questo compito. La tua risposta e' il rapporto che si consegna.",
        );
    }
    if let Some((prima, motivo)) = rifatto {
        t.push_str(&format!(
            "\nLo rifai tu: la prima volta l'ha fatto «{prima}», e il revisore ha detto: {}",
            motivo.trim()
        ));
    }
    if !fatti.is_empty() {
        t.push_str("\n\nQuello che e' venuto fuori dai passi fatti finora:");
        for f in fatti {
            t.push_str(&format!(
                "\n\n### Passo {} ({})\n{}",
                f.numero,
                f.genere.nome(),
                tagliato(&f.testo, RISULTATO_IN_RICHIESTA)
            ));
        }
    }
    t
}

/// Come continua un passo che ha finito i giri di strumenti.
pub fn continua_il_passo(numero: usize, passo: &Passo) -> String {
    format!(
        "Continua il passo {numero} ({}) da dove sei arrivato, e quando hai finito \
         rispondi con il risultato.",
        passo.genere.nome()
    )
}

/// La domanda al revisore. Il risultato da giudicare ci sta dentro: il
/// revisore puo' essere un cervello che la conversazione non la vede.
pub fn richiesta_della_revisione(
    numero: usize,
    quanti: usize,
    passo: &Passo,
    cervello: &str,
    risultato: &str,
) -> String {
    format!(
        "[revisione del passo {numero} di {quanti}] Il passo era: {} - {}\n\
         L'ha fatto «{cervello}», e ha dato questo:\n<<<\n{}\n>>>\n\
         Giudica se basta per andare avanti nel compito. Rispondi con una parola in \
         testa, BUONO o SCARSO, e dopo SCARSO il perche' in una riga.",
        passo.genere.nome(),
        passo.cosa.trim(),
        tagliato(risultato, RISULTATO_DA_RIVEDERE),
    )
}

/// Cosa ha detto il revisore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Giudizio {
    Buono,
    /// Con il perche'.
    Scarso(String),
    /// Non ha cominciato ne' con BUONO ne' con SCARSO. Il passo si tiene: un
    /// giudizio che non si legge non e' un giudizio, e salire costa.
    Illeggibile,
}

impl Giudizio {
    pub fn nome(&self) -> &'static str {
        match self {
            Giudizio::Buono => "buono",
            Giudizio::Scarso(_) => "scarso",
            Giudizio::Illeggibile => "illeggibile",
        }
    }
}

/// Legge il giudizio: la prima parola, senza badare a maiuscole, asterischi
/// o virgolette davanti.
pub fn leggi_giudizio(testo: &str) -> Giudizio {
    let t = testo.trim_start_matches(|c: char| !c.is_alphanumeric());
    let parola: String = t.chars().take_while(|c| c.is_alphabetic()).collect();
    let resto = t[parola.len()..]
        .trim_start_matches(|c: char| !c.is_alphanumeric())
        .trim();
    match parola.to_uppercase().as_str() {
        "BUONO" => Giudizio::Buono,
        "SCARSO" => Giudizio::Scarso(if resto.is_empty() {
            "il revisore non ha detto perche'".to_string()
        } else {
            tagliato(resto.lines().next().unwrap_or(resto), 400)
        }),
        _ => Giudizio::Illeggibile,
    }
}

/// Gli indirizzi web in un testo, in ordine e senza doppioni.
///
/// Un indirizzo finisce al primo spazio o a un carattere che in un testo lo
/// chiude: parentesi, virgolette, la barra rovescia di un JSON. La
/// punteggiatura in coda («vedi https://x.org.») non ne fa parte.
pub fn indirizzi(testo: &str) -> Vec<String> {
    let mut fuori: Vec<String> = Vec::new();
    let mut resto = testo;
    while let Some(i) = ["https://", "http://"]
        .iter()
        .filter_map(|p| resto.find(p))
        .min()
    {
        let dopo = &resto[i..];
        let fine = dopo
            .find(|c: char| {
                c.is_whitespace()
                    || matches!(c, '<' | '>' | '"' | '\'' | '`' | ')' | ']' | '|' | '\\')
            })
            .unwrap_or(dopo.len());
        let u = dopo[..fine].trim_end_matches(['.', ',', ';', ':', '!', '?']);
        if u.len() > "https://".len() && !fuori.iter().any(|x| x == u) {
            fuori.push(u.to_string());
        }
        resto = &dopo[fine.max(1)..];
    }
    fuori
}

/// Un indirizzo nella forma in cui si confronta: schema e dominio in
/// minuscolo, senza il frammento dopo «#» e senza la barra in fondo.
pub fn normale(url: &str) -> String {
    let senza_frammento = url.split('#').next().unwrap_or(url);
    let (schema, resto) = senza_frammento
        .split_once("://")
        .unwrap_or(("", senza_frammento));
    let (dominio, percorso) = match resto.find('/') {
        Some(i) => (&resto[..i], &resto[i..]),
        None => (resto, ""),
    };
    let mut n = format!(
        "{}://{}{}",
        schema.to_lowercase(),
        dominio.to_lowercase(),
        percorso
    );
    while n.ends_with('/') {
        n.pop();
    }
    n
}

/// Com'e' andato il controllo di una fonte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Controllo {
    /// L'indirizzo e' passato da uno strumento di NOVA in questo compito.
    Vista,
    /// Non e' passato da nessuno strumento, e tutto quello che il Dot ha
    /// letto l'ha letto con gli strumenti di NOVA: nessuno l'ha letta.
    NonVista,
    /// Non e' passato dagli strumenti di NOVA, ma una parte del lavoro l'ha
    /// fatta un cervello che legge con strumenti suoi (Claude Code, una CLI):
    /// NOVA non vede cosa ha letto, e non puo' dire ne' si' ne' no.
    DaVerificare,
}

/// Una fonte citata nel rapporto, e se il Dot l'aveva letta davvero.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Fonte {
    pub indirizzo: String,
    pub controllo: Controllo,
}

/// Ogni indirizzo del rapporto, con scritto se era fra quelli visti: gli
/// indirizzi comparsi in una chiamata a uno strumento di NOVA andata a buon
/// fine, negli argomenti o nel risultato.
///
/// `tutto_visto` dice se tutto il lavoro di lettura e' passato dagli
/// strumenti di NOVA. Quando no, un indirizzo che NOVA non ha visto non e'
/// «non visto»: e' da verificare. Dire «non vista» di una pagina che Claude
/// Code ha letto davvero sarebbe un'accusa falsa.
pub fn controlla_fonti(rapporto: &str, viste: &[String], tutto_visto: bool) -> Vec<Fonte> {
    let viste: BTreeSet<String> = viste.iter().map(|u| normale(u)).collect();
    let mut gia: BTreeSet<String> = BTreeSet::new();
    indirizzi(rapporto)
        .into_iter()
        .filter(|u| gia.insert(normale(u)))
        .map(|u| Fonte {
            controllo: if viste.contains(&normale(&u)) {
                Controllo::Vista
            } else if tutto_visto {
                Controllo::NonVista
            } else {
                Controllo::DaVerificare
            },
            indirizzo: u,
        })
        .collect()
}

/// «1 vista», «3 viste».
fn quante(n: usize, una: &str, tante: &str) -> String {
    format!("{n} {}", if n == 1 { una } else { tante })
}

fn conta(fonti: &[Fonte], c: Controllo) -> usize {
    fonti.iter().filter(|f| f.controllo == c).count()
}

/// Il rapporto come si scrive su disco: quello del modello, e in coda il
/// controllo delle fonti, scritto da NOVA e non dal modello.
pub fn rapporto_su_disco(corpo: &str, fonti: &[Fonte]) -> String {
    let mut t = corpo.trim_end().to_string();
    t.push_str("\n\n---\n\n## Le fonti, controllate da NOVA\n\n");
    if fonti.is_empty() {
        t.push_str("Il rapporto non cita nessun indirizzo.\n");
        return t;
    }
    t.push_str(
        "Un indirizzo e' «visto» se e' comparso in quello che il Dot ha letto davvero con \
         gli strumenti di NOVA, in questo compito.",
    );
    if conta(fonti, Controllo::DaVerificare) > 0 {
        t.push_str(
            " Una parte del lavoro l'ha fatta un cervello che legge con strumenti suoi \
             (Claude Code o una CLI): quello che ha letto NOVA non lo vede, e un indirizzo \
             che non ha visto resta da verificare.",
        );
    }
    t.push_str("\n\n");
    for f in fonti {
        let come = match f.controllo {
            Controllo::Vista => "vista",
            Controllo::NonVista => "**non vista**",
            Controllo::DaVerificare => "da verificare",
        };
        t.push_str(&format!("- {come}: {}\n", f.indirizzo));
    }
    t
}

/// La frase con cui si chiude un compito: dove sta il rapporto, e come
/// stanno le sue fonti.
pub fn esito(percorso: &str, fonti: &[Fonte]) -> String {
    if fonti.is_empty() {
        return format!("Rapporto in {percorso}. Non cita nessuna fonte.");
    }
    let mut t = format!(
        "Rapporto in {percorso}. {}: {}",
        quante(fonti.len(), "fonte citata", "fonti citate"),
        quante(conta(fonti, Controllo::Vista), "vista", "viste"),
    );
    let non = conta(fonti, Controllo::NonVista);
    if non > 0 {
        t.push_str(&format!(
            ", {} nelle letture del Dot",
            quante(non, "non vista", "non viste")
        ));
    }
    let dubbie = conta(fonti, Controllo::DaVerificare);
    if dubbie > 0 {
        t.push_str(&format!(", {dubbie} da verificare"));
    }
    t.push('.');
    t
}

/// Il titolo del rapporto: la prima riga «# ...», se c'e'.
pub fn titolo(corpo: &str) -> Option<String> {
    corpo
        .lines()
        .map(str::trim)
        .find(|r| r.starts_with("# "))
        .map(|r| r.trim_start_matches("# ").trim().to_string())
        .filter(|t| !t.is_empty())
}

#[cfg(test)]
mod prove {
    use super::*;

    fn scala() -> Vec<String> {
        vec![
            "piccolo".to_string(),
            "medio".to_string(),
            "grande".to_string(),
        ]
    }

    #[test]
    fn la_richiesta_del_piano_nomina_i_cervelli_veri_in_ordine() {
        let r = richiesta_del_piano(" le supernove ", &scala());
        assert!(r.starts_with("[piano] "));
        assert!(r.contains("Il compito: le supernove\n"));
        assert!(r.contains("1. piccolo\n2. medio\n3. grande\n"));
        assert!(r.contains("\"cervello\": \"piccolo\""));
        assert!(r.contains(&format!("al massimo {PASSI_MASSIMI}")));
    }

    #[test]
    fn il_piano_si_legge_anche_fra_tre_apici() {
        let t = "Ecco il piano:\n```json\n{\"passi\": [\
            {\"tipo\": \"cerca\", \"cosa\": \"trova\", \"cervello\": \"piccolo\"},\
            {\"tipo\": \"Leggi\", \"cosa\": \"leggi\", \"cervello\": \"Medio\"},\
            {\"tipo\": \"scrivi\", \"cosa\": \"scrivi\", \"cervello\": \"grande\"}]}\n```";
        let p = leggi_piano(t, &scala()).unwrap();
        assert!(p.letto && p.note.is_empty(), "{:?}", p.note);
        let g: Vec<_> = p
            .passi
            .iter()
            .map(|x| (x.genere, x.cervello.as_str(), x.scelto_da))
            .collect();
        assert_eq!(
            g,
            [
                (Genere::Cerca, "piccolo", SceltoDa::Piano),
                (Genere::Leggi, "medio", SceltoDa::Piano),
                (Genere::Scrivi, "grande", SceltoDa::Piano),
            ]
        );
    }

    #[test]
    fn un_cervello_che_non_c_e_ripiega_sul_grande_e_lo_dice() {
        let t = r#"{"passi": [{"tipo": "cerca", "cosa": "trova", "cervello": "gpt-9"},
                              {"tipo": "boh", "cosa": "pensa", "cervello": "piccolo"},
                              {"tipo": "scrivi", "cosa": " ", "cervello": "piccolo"},
                              {"tipo": "scrivi", "cosa": "scrivi", "cervello": "piccolo"}]}"#;
        let p = leggi_piano(t, &scala()).unwrap();
        assert_eq!(p.passi.len(), 3, "il passo senza «cosa» si salta");
        assert_eq!(p.passi[0].cervello, "grande");
        assert_eq!(p.passi[0].scelto_da, SceltoDa::Ripiego);
        assert!(p.passi[0].perche.contains("«gpt-9»"));
        assert_eq!(p.passi[1].genere, Genere::Altro);
        assert!(p.note.iter().any(|n| n.contains("senza «cosa»")));
    }

    #[test]
    fn un_piano_finisce_sempre_con_scrivi_e_non_supera_il_tetto() {
        let passo = r#"{"tipo": "cerca", "cosa": "c", "cervello": "piccolo"}"#;
        let lungo = format!(
            "{{\"passi\": [{}]}}",
            vec![passo; PASSI_MASSIMI + 3].join(",")
        );
        let p = leggi_piano(&lungo, &scala()).unwrap();
        assert_eq!(p.passi.len(), PASSI_MASSIMI);
        let ultimo = p.passi.last().unwrap();
        assert_eq!(
            (ultimo.genere, ultimo.cervello.as_str(), ultimo.scelto_da),
            (Genere::Scrivi, "grande", SceltoDa::Ripiego)
        );
        assert!(p.note.iter().any(|n| n.contains("tenuti i primi")));
        let corto = format!("{{\"passi\": [{passo}]}}");
        let p = leggi_piano(&corto, &scala()).unwrap();
        assert_eq!(p.passi.len(), 2);
        assert_eq!(p.passi[1].genere, Genere::Scrivi);
    }

    #[test]
    fn un_piano_che_non_si_legge_diventa_quello_di_ripiego_col_grande() {
        for brutto in [
            "niente JSON qui",
            "{rotto",
            "{\"altro\": 1}",
            "{\"passi\": []}",
        ] {
            assert!(leggi_piano(brutto, &scala()).is_err(), "{brutto}");
            let p = piano(brutto, &scala());
            assert!(!p.letto);
            assert_eq!(p.passi.len(), 3);
            assert!(p
                .passi
                .iter()
                .all(|x| x.cervello == "grande" && x.scelto_da == SceltoDa::Ripiego));
            assert_eq!(p.passi[2].genere, Genere::Scrivi);
            assert_eq!(p.note.len(), 1);
        }
        assert!(leggi_piano("{\"passi\": []}", &[])
            .unwrap_err()
            .contains("vuota"));
    }

    #[test]
    fn la_richiesta_di_un_passo_porta_quello_che_serve() {
        let p = Passo {
            genere: Genere::Scrivi,
            cosa: "scrivi".into(),
            cervello: "piccolo".into(),
            scelto_da: SceltoDa::Piano,
            perche: String::new(),
        };
        let fatti = [Fatto {
            numero: 1,
            genere: Genere::Cerca,
            testo: "x".repeat(RISULTATO_IN_RICHIESTA + 10),
        }];
        let r = richiesta_del_passo(2, 2, &p, &fatti, Some(("piccolo", "troppo vago")));
        assert!(r.starts_with("[passo 2 di 2: scrivi] scrivi\n"));
        assert!(r.contains("## Fonti"));
        assert!(r.contains("l'ha fatto «piccolo», e il revisore ha detto: troppo vago"));
        assert!(r.contains("### Passo 1 (cerca)\n"));
        assert!(r.ends_with(" [...]"), "il taglio si dichiara");
        let solo = richiesta_del_passo(
            1,
            2,
            &Passo {
                genere: Genere::Cerca,
                ..p
            },
            &[],
            None,
        );
        assert_eq!(solo, "[passo 1 di 2: cerca] scrivi");
    }

    #[test]
    fn il_revisore_legge_il_risultato_nella_richiesta() {
        let p = Passo {
            genere: Genere::Leggi,
            cosa: "leggi la fonte".into(),
            cervello: "piccolo".into(),
            scelto_da: SceltoDa::Piano,
            perche: String::new(),
        };
        let r = richiesta_della_revisione(2, 3, &p, "piccolo", "  il risultato  ");
        assert!(
            r.starts_with("[revisione del passo 2 di 3] Il passo era: leggi - leggi la fonte\n")
        );
        assert!(r.contains("L'ha fatto «piccolo»"));
        assert!(r.contains("<<<\nil risultato\n>>>"));
    }

    #[test]
    fn il_giudizio_e_la_prima_parola() {
        assert_eq!(leggi_giudizio("BUONO"), Giudizio::Buono);
        assert_eq!(leggi_giudizio("  **Buono**. Va bene."), Giudizio::Buono);
        assert_eq!(
            leggi_giudizio("SCARSO: non cita nessuna pagina\naltro"),
            Giudizio::Scarso("non cita nessuna pagina".into())
        );
        assert_eq!(
            leggi_giudizio("scarso"),
            Giudizio::Scarso("il revisore non ha detto perche'".into())
        );
        assert_eq!(leggi_giudizio("Direi che e' BUONO"), Giudizio::Illeggibile);
        assert_eq!(leggi_giudizio(""), Giudizio::Illeggibile);
        assert_eq!(leggi_giudizio("BUONISSIMO"), Giudizio::Illeggibile);
    }

    #[test]
    fn gli_indirizzi_si_trovano_dove_stanno_davvero() {
        let t = "Vedi https://esempio.org/a. E [qui](https://Esempio.org/b/) e \"http://x.it/c?d=1\", \
                 poi {\"url\":\"https://j.son/p\\\"} https://esempio.org/a di nuovo; e https:// da solo.";
        assert_eq!(
            indirizzi(t),
            [
                "https://esempio.org/a",
                "https://Esempio.org/b/",
                "http://x.it/c?d=1",
                "https://j.son/p"
            ]
        );
        assert!(indirizzi("niente qui").is_empty());
    }

    #[test]
    fn un_indirizzo_si_confronta_nella_forma_normale() {
        assert_eq!(
            normale("HTTPS://Esempio.ORG/Pagina/#sezione"),
            "https://esempio.org/Pagina"
        );
        assert_eq!(normale("https://esempio.org/"), "https://esempio.org");
        assert_eq!(normale("https://esempio.org"), "https://esempio.org");
    }

    #[test]
    fn le_fonti_si_controllano_contro_quello_che_si_e_letto() {
        let rapporto = "# Supernove\nTesto.\n## Fonti\n- https://esempio.org/supernove/\n\
                        - https://inventata.example/x\n- https://ESEMPIO.org/supernove";
        let viste = vec!["https://esempio.org/supernove".to_string()];
        let f = controlla_fonti(rapporto, &viste, true);
        assert_eq!(
            f,
            [
                Fonte {
                    indirizzo: "https://esempio.org/supernove/".into(),
                    controllo: Controllo::Vista
                },
                Fonte {
                    indirizzo: "https://inventata.example/x".into(),
                    controllo: Controllo::NonVista
                },
            ],
            "la stessa pagina scritta due volte conta una volta"
        );
        let su_disco = rapporto_su_disco(rapporto, &f);
        assert!(su_disco.starts_with(rapporto));
        assert!(su_disco.contains("## Le fonti, controllate da NOVA"));
        assert!(su_disco.contains("- vista: https://esempio.org/supernove/\n"));
        assert!(su_disco.contains("- **non vista**: https://inventata.example/x\n"));
        assert!(!su_disco.contains("strumenti suoi"));
        assert_eq!(
            esito("r.md", &f),
            "Rapporto in r.md. 2 fonti citate: 1 vista, 1 non vista nelle letture del Dot."
        );
        let tutte = controlla_fonti("https://esempio.org/supernove", &viste, true);
        assert_eq!(
            esito("r.md", &tutte),
            "Rapporto in r.md. 1 fonte citata: 1 vista."
        );
        assert_eq!(
            esito("r.md", &[]),
            "Rapporto in r.md. Non cita nessuna fonte."
        );
        assert!(rapporto_su_disco("x", &[]).contains("non cita nessun indirizzo"));
    }

    #[test]
    fn quello_che_legge_un_cervello_con_strumenti_suoi_resta_da_verificare() {
        let viste = vec!["https://esempio.org/a".to_string()];
        let f = controlla_fonti("https://esempio.org/a https://altro.org/b", &viste, false);
        assert_eq!(
            f.iter().map(|x| x.controllo).collect::<Vec<_>>(),
            [Controllo::Vista, Controllo::DaVerificare],
            "quello che NOVA ha visto resta visto"
        );
        let su_disco = rapporto_su_disco("x", &f);
        assert!(su_disco.contains("- da verificare: https://altro.org/b\n"));
        assert!(su_disco.contains("strumenti suoi"));
        assert!(!su_disco.contains("non vista"));
        assert_eq!(
            esito("r.md", &f),
            "Rapporto in r.md. 2 fonti citate: 1 vista, 1 da verificare."
        );
        assert_eq!(
            serde_json::to_value(&f[1]).unwrap()["controllo"],
            "da_verificare"
        );
    }

    #[test]
    fn il_titolo_e_la_prima_riga_con_un_cancelletto() {
        assert_eq!(
            titolo("intro\n# Le supernove \n## Fonti").as_deref(),
            Some("Le supernove")
        );
        assert_eq!(titolo("## Solo sezioni"), None);
        assert_eq!(titolo("# "), None);
        assert_eq!(tagliato("abc", 2), "ab [...]");
        assert_eq!(tagliato(" abc ", 3), "abc");
    }
}
