//! Imparare da uno scambio: la parte di `nova/kb/memory.py` che non tocca
//! ne' il modello ne' il disco.
//!
//! Dopo un turno si chiede al modello quali fatti **durevoli** ci sono nello
//! scambio; qui c'e' tutto cio' che sta intorno a quella domanda: come si
//! scrive ([`richiesta`]), cosa gli si dice di sapere gia' ([`gia_noti`]),
//! come si legge la risposta ([`estrai_json`]) e cosa se ne tiene
//! ([`fatto_a_nodo`]). Chiamare il modello e scrivere nel vault lo fa il
//! demone, perche' e' li' che vivono il cervello e il deposito.
//!
//! Le regole sono quelle del Python voce per voce, e il banco le confronta:
//! due meta' che imparano cose diverse dallo stesso scambio scriverebbero
//! due memorie diverse della stessa persona.

use crate::{slug, Nodo, ORIGINE_AUTO};
use serde_json::Value;

/// La domanda al modulo di memoria. Identica a `PROMPT_ESTRAZIONE`, con le
/// graffe doppie di `str.format` gia' sciolte.
pub const PROMPT_ESTRAZIONE: &str = r#"Sei il modulo di memoria di NOVA. Leggi lo scambio qui sotto ed estrai
SOLO i fatti durevoli su {user} o sul suo ambiente di lavoro: preferenze, abitudini,
strumenti, progetti, persone, vincoli, decisioni prese.

NON estrarre:
- richieste una tantum ("apri il blocco note")
- risultati temporanei (elenchi di file, output di comandi, orari)
- cose che gia' sai (elencate sotto come "gia' in memoria")
- supposizioni: solo cio' che e' stato detto o dimostrato
- TITOLI di finestre, schede del browser, documenti o file aperti: dicono cosa
  {user} stava guardando in un certo momento, non chi e'. L'applicazione si
  puo' ricordare («usa Antigravity»), quello che c'e' dentro no.

Gia' in memoria{parziale}: {noti}

Scambio:
---
UTENTE: {utente}
NOVA: {assistente}
---

Rispondi SOLO con un array JSON, anche vuoto. Ogni elemento:
{"titolo": "breve, 2-6 parole", "tipo": "profilo|preferenza|progetto|app|persona|abitudine|fatto",
  "testo": "una o due frasi in italiano, autoconsistenti",
  "tags": ["max 4"], "relazioni": ["slug-di-nodi-gia-noti"], "confidenza": 0.5-0.95}

Se non c'e' nulla da imparare rispondi []."#;

/// Quanto di ciascuna meta' dello scambio arriva al modello.
pub const TAGLIO_SCAMBIO: usize = 2500;
/// Quanti caratteri di slug si elencano come gia' noti.
pub const NOTI_MAX_CARATTERI: usize = 1800;
/// Quanti fatti si tengono da una risposta sola.
pub const FATTI_MAX: usize = 6;
/// I gettoni concessi al modello per rispondere.
pub const GETTONI_ESTRAZIONE: u32 = 700;
/// Quanti scambi aspettano in fila: oltre, i piu' vecchi cedono il posto.
pub const CODA_MASSIMA: usize = 8;
/// Sotto questa lunghezza la domanda non ha niente da insegnare
/// (`kb.learn_min_chars`, se la configurazione non dice altro).
pub const MIN_CARATTERI: usize = 25;

/// Segni che un «fatto» e' in realta' la fotografia di uno schermo: il
/// titolo di una scheda, di un documento, di una finestra. Non parole
/// proibite: **forme**, cioe' come si presenta un titolo catturato invece
/// di un fatto raccontato.
pub const FORME_DI_TITOLO: [&str; 14] = [
    " - google chrome",
    " — google chrome",
    " - mozilla firefox",
    " - microsoft edge",
    " e altre ",
    " and other ",
    " - youtube",
    "scheda del browser",
    "schede aperte",
    "titolo della finestra",
    "finestra aperta",
    "finestre aperte",
    "ha aperto la scheda",
    "stava guardando",
];

/// Le estensioni che, seguite da un trattino e da un'applicazione, fanno
/// il titolo di una finestra: «bilancio.xlsx — Excel».
const ESTENSIONI_DI_FINESTRA: [&str; 9] =
    ["docx", "xlsx", "pptx", "pdf", "txt", "md", "png", "jpg", "mp4"];

/// Lo scambio vale la fatica? Come `osserva_async`: domanda abbastanza
/// lunga, e un turno che non ha guardato lo schermo — leggere le finestre
/// serve ad agire, ricordarle no, e un vault markdown non dimentica.
pub fn vale_la_pena(domanda: &str, min_caratteri: usize, riservato: bool) -> bool {
    domanda.trim().chars().count() >= min_caratteri && !riservato
}

/// La domanda completa per il modello.
pub fn richiesta(utente_nome: &str, noti: &str, parziale: &str, utente: &str, assistente: &str) -> String {
    let taglia = |s: &str| s.chars().take(TAGLIO_SCAMBIO).collect::<String>();
    // Un giro solo, come `str.format`: un segnaposto che compare **dentro**
    // lo scambio (qualcuno che scrive «{user}») resta com'e'.
    let mut fuori = String::with_capacity(PROMPT_ESTRAZIONE.len() + 6000);
    let mut resto = PROMPT_ESTRAZIONE;
    let voci: [(&str, String); 5] = [
        ("{user}", utente_nome.to_string()),
        ("{parziale}", parziale.to_string()),
        ("{noti}", noti.to_string()),
        ("{utente}", taglia(utente)),
        ("{assistente}", taglia(assistente)),
    ];
    while let Some(i) = resto.find('{') {
        fuori.push_str(&resto[..i]);
        let dopo = &resto[i..];
        match voci.iter().find(|(k, _)| dopo.starts_with(k)) {
            Some((k, v)) => {
                fuori.push_str(v);
                resto = &dopo[k.len()..];
            }
            None => {
                fuori.push('{');
                resto = &dopo[1..];
            }
        }
    }
    fuori.push_str(resto);
    fuori
}

/// Le parole di almeno tre lettere o cifre ASCII: `[a-z0-9]{3,}`.
fn parole(testo: &str) -> std::collections::BTreeSet<String> {
    let mut fuori = std::collections::BTreeSet::new();
    let mut corrente = String::new();
    for c in testo.chars().chain(std::iter::once(' ')) {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            corrente.push(c);
        } else {
            if corrente.len() >= 3 {
                fuori.insert(corrente.clone());
            }
            corrente.clear();
        }
    }
    fuori
}

/// Gli slug che contano per **questo** scambio, non i primi in ordine
/// alfabetico: oltre la sessantesima «a» il modello non sapeva piu' cosa
/// c'era gia' e ricreava nodi doppi. Torna `(elenco, parziale)`.
pub fn gia_noti<'a>(nodi: impl IntoIterator<Item = &'a Nodo>, scambio: &str) -> (String, String) {
    let nodi: Vec<&Nodo> = nodi.into_iter().collect();
    if nodi.is_empty() {
        return ("(niente)".into(), String::new());
    }
    let chiave = parole(&scambio.to_lowercase());
    let pertinenza = |n: &Nodo| -> usize {
        let testo = format!("{} {} {}", n.title, n.tags.join(" "), n.slug).to_lowercase();
        parole(&testo).intersection(&chiave).count()
    };
    let mut ordinati: Vec<(usize, &Nodo)> = nodi.iter().map(|n| (pertinenza(n), *n)).collect();
    ordinati.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.slug.cmp(&b.1.slug)));
    let mut scelti: Vec<&str> = Vec::new();
    let mut usati = 0;
    for (_, n) in &ordinati {
        let lungo = n.slug.chars().count();
        if usati + lungo + 2 > NOTI_MAX_CARATTERI {
            break;
        }
        scelti.push(&n.slug);
        usati += lungo + 2;
    }
    let mancanti = nodi.len() - scelti.len();
    let parziale = if mancanti > 0 {
        format!(" (i {} piu' pertinenti su {})", scelti.len(), nodi.len())
    } else {
        String::new()
    };
    scelti.sort();
    let elenco = if scelti.is_empty() {
        "(niente)".to_string()
    } else {
        scelti.join(", ")
    };
    (elenco, parziale)
}

/// Toglie i `<think>...</think>`, senza badare alle maiuscole.
fn senza_pensieri(testo: &str) -> String {
    let mut fuori = testo.to_string();
    loop {
        let basso = fuori.to_ascii_lowercase();
        let Some(i) = basso.find("<think>") else { break };
        let Some(j) = basso[i..].find("</think>") else { break };
        fuori.replace_range(i..i + j + "</think>".len(), "");
    }
    fuori
}

/// Toglie i recinti ```` ``` ```` a inizio e fine riga, come
/// `re.sub(r"^```(?:json)?|```$", "", t, flags=re.M)`.
fn senza_recinti(testo: &str) -> String {
    testo
        .split('\n')
        .map(|riga| {
            let mut r = riga;
            if let Some(dopo) = r.strip_prefix("```") {
                r = dopo.strip_prefix("json").unwrap_or(dopo);
            }
            r.strip_suffix("```").unwrap_or(r).to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Gli oggetti dell'array JSON nella risposta del modello, anche se ci ha
/// messo intorno pensieri, recinti o chiacchiere. Niente, se non ce n'e'.
pub fn estrai_json(testo: &str) -> Vec<serde_json::Map<String, Value>> {
    if testo.is_empty() {
        return Vec::new();
    }
    let t = senza_pensieri(testo);
    let t = senza_recinti(t.trim());
    let t = t.trim();
    let (Some(inizio), Some(fine)) = (t.find('['), t.rfind(']')) else {
        return Vec::new();
    };
    if fine <= inizio {
        return Vec::new();
    }
    match serde_json::from_str::<Value>(&t[inizio..=fine]) {
        Ok(Value::Array(v)) => v
            .into_iter()
            .filter_map(|d| match d {
                Value::Object(m) => Some(m),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn parola(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Sembra la cattura di quello che c'era sullo schermo?
pub fn e_una_finestra(titolo: &str, testo: &str) -> bool {
    let insieme = format!("{titolo} {testo}").to_lowercase();
    if FORME_DI_TITOLO.iter().any(|f| insieme.contains(f)) {
        return true;
    }
    // `[\w\-]+\.(docx|...)\s*[-—]\s*\w`: nome di file, trattino,
    // applicazione.
    let c: Vec<char> = insieme.chars().collect();
    for (i, &x) in c.iter().enumerate() {
        if x != '.' || i == 0 || !(parola(c[i - 1]) || c[i - 1] == '-') {
            continue;
        }
        for est in ESTENSIONI_DI_FINESTRA {
            let e: Vec<char> = est.chars().collect();
            let mut k = i + 1;
            if c.len() < k + e.len() || c[k..k + e.len()] != e[..] {
                continue;
            }
            k += e.len();
            while k < c.len() && c[k].is_whitespace() {
                k += 1;
            }
            if k >= c.len() || !(c[k] == '-' || c[k] == '—') {
                continue;
            }
            k += 1;
            while k < c.len() && c[k].is_whitespace() {
                k += 1;
            }
            if k < c.len() && parola(c[k]) {
                return true;
            }
        }
    }
    false
}

/// Falso come lo intende Python: `None`, `False`, zero, vuoto.
fn vuoto(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => true,
        Some(Value::Bool(b)) => !b,
        Some(Value::Number(n)) => n.as_f64() == Some(0.0),
        Some(Value::String(s)) => s.is_empty(),
        Some(Value::Array(a)) => a.is_empty(),
        Some(Value::Object(o)) => o.is_empty(),
    }
}

/// `str(v)` per i valori che un modello mette davvero in un JSON.
fn come_testo(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::Null => "None".into(),
        altro => altro.to_string(),
    }
}

fn campo(f: &serde_json::Map<String, Value>, nome: &str, ripiego: &str) -> String {
    let v = f.get(nome);
    if vuoto(v) {
        ripiego.to_string()
    } else {
        come_testo(v.expect("non vuoto"))
    }
}

/// Una riga sola, senza spazi doppi, al massimo 120 caratteri: il titolo
/// finisce grezzo nel frontmatter, e un a capo dentro ne perdeva meta'.
pub fn pulisci_titolo(v: &str) -> String {
    v.split(char::is_whitespace)
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(120)
        .collect()
}

fn come_lista(v: Option<&Value>) -> Vec<String> {
    if vuoto(v) {
        return Vec::new();
    }
    match v.expect("non vuoto") {
        Value::Array(a) => a.iter().map(come_testo).collect(),
        altro => vec![come_testo(altro)],
    }
}

/// Un fatto del modello diventa un nodo, o niente se non regge.
pub fn fatto_a_nodo(f: &serde_json::Map<String, Value>) -> Option<Nodo> {
    let titolo = pulisci_titolo(&campo(f, "titolo", ""));
    let testo = campo(f, "testo", "").trim().to_string();
    if titolo.is_empty() || testo.chars().count() < 10 {
        return None;
    }
    if e_una_finestra(&titolo, &testo) {
        // Ultima rete, sotto l'istruzione nel prompt: un modello che si
        // distrae non deve poter scrivere sul disco cosa avevi aperto.
        return None;
    }
    let confidenza = match f.get("confidenza") {
        None => 0.7,
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.7),
        Some(Value::String(s)) => s.trim().parse::<f64>().unwrap_or(0.7),
        Some(Value::Bool(b)) => f64::from(u8::from(*b)),
        Some(_) => 0.7,
    };
    // `max(0.3, min(0.95, nan))` in Python fa 0.95: `min` tiene il primo
    // quando il confronto e' falso.
    let confidenza = if confidenza.is_nan() { 0.95 } else { confidenza };
    Some(Nodo {
        slug: slug(&titolo),
        title: titolo,
        body: testo,
        tipo: campo(f, "tipo", "fatto"),
        tags: come_lista(f.get("tags"))
            .into_iter()
            .map(|t| t.trim().to_lowercase())
            .take(4)
            .collect(),
        relazioni: come_lista(f.get("relazioni"))
            .iter()
            .map(|r| slug(r))
            .take(5)
            .collect(),
        origine: ORIGINE_AUTO.to_string(),
        confidenza: confidenza.clamp(0.3, 0.95),
        ..Default::default()
    })
}

/// Dalla risposta del modello ai nodi da scrivere, al massimo
/// [`FATTI_MAX`] tentativi.
pub fn nodi_dalla_risposta(risposta: &str) -> Vec<Nodo> {
    estrai_json(risposta)
        .iter()
        .take(FATTI_MAX)
        .filter_map(fatto_a_nodo)
        .collect()
}

#[cfg(test)]
mod prove {
    use super::*;
    use serde_json::json;

    fn mappa(v: Value) -> serde_json::Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn la_risposta_si_legge_anche_col_contorno() {
        let r = "<THINK>boh [1]</think>\n```json\n[{\"titolo\": \"a\"}, 3]\n```";
        let v = estrai_json(r);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0]["titolo"], "a");
        assert!(estrai_json("niente").is_empty());
        assert!(estrai_json("] [").is_empty());
    }

    #[test]
    fn un_titolo_di_finestra_non_si_ricorda() {
        assert!(e_una_finestra("Bilancio", "bilancio.xlsx — Excel era aperto"));
        assert!(e_una_finestra("x", "YouTube - Google Chrome"));
        assert!(!e_una_finestra("Usa Excel", "Gio usa Excel per i conti di casa."));
        assert!(!e_una_finestra("file", "il file .md - x"));
    }

    #[test]
    fn un_fatto_diventa_un_nodo_come_in_python() {
        let n = fatto_a_nodo(&mappa(json!({
            "titolo": "  Editor\npreferito ",
            "testo": "Gio scrive codice con Antigravity.",
            "tags": "Editor",
            "relazioni": ["Gio Profilo"],
            "confidenza": "2",
        })))
        .unwrap();
        assert_eq!(n.title, "Editor preferito");
        assert_eq!(n.tipo, "fatto");
        assert_eq!(n.tags, vec!["editor"]);
        assert_eq!(n.relazioni, vec![slug("Gio Profilo")]);
        assert_eq!(n.confidenza, 0.95);
        assert_eq!(n.origine, ORIGINE_AUTO);
        assert!(fatto_a_nodo(&mappa(json!({"titolo": "x", "testo": "corto"}))).is_none());
    }

    #[test]
    fn i_noti_sono_i_piu_pertinenti() {
        let a = Nodo { slug: "aaa".into(), title: "Aaa".into(), ..Default::default() };
        let b = Nodo { slug: "excel-conti".into(), title: "Excel conti".into(), ..Default::default() };
        let (elenco, parziale) = gia_noti([&a, &b], "uso excel");
        assert_eq!(elenco, "aaa, excel-conti");
        assert_eq!(parziale, "");
        assert_eq!(gia_noti(std::iter::empty(), "x").0, "(niente)");
    }

    #[test]
    fn i_segnaposto_si_sostituiscono_una_volta_sola() {
        let r = richiesta("Gio", "(niente)", "", "scrivo {user}", "ok");
        assert!(r.contains("UTENTE: scrivo {user}"));
        assert!(r.contains("su Gio o sul suo"));
        assert!(r.contains("{\"titolo\": \"breve"));
    }

    #[test]
    fn la_domanda_corta_o_riservata_non_insegna() {
        assert!(!vale_la_pena("ciao", MIN_CARATTERI, false));
        assert!(vale_la_pena(&"a".repeat(30), MIN_CARATTERI, false));
        assert!(!vale_la_pena(&"a".repeat(30), MIN_CARATTERI, true));
    }
}
