//! Il nodo della memoria, e la sua forma su disco.
//!
//! Un file `.md` per nodo, frontmatter in testa, relazioni esplicite. Il vault
//! si apre in Obsidian cosi' com'e', e i `[[wikilink]]` nel corpo sono nativi:
//! e' una scelta che si paga in rigidita' del formato e si riprende tutta in
//! fiducia, perche' l'utente puo' leggere e correggere a mano cio' che NOVA
//! ricorda di lui.
//!
//! Qui c'e' il **modello**: cosa e' un nodo, come si scrive e come si rilegge.
//! Non c'e' il disco — quello e' il pezzo che parla col sistema, e va scritto
//! contro il trait di `nova-platform` quando ci si arriva. E non c'e' il
//! guardiano: cio' che puo' entrare nel vault lo decide il filtro dei segreti,
//! che dalla parte Python vive in `nova/forme_riservate.py` ed e' il primo
//! posto a cui questa crate dovra' chiedere il permesso quando imparera' a
//! scrivere.
//!
//! **La data si passa da fuori.** `to_markdown` in Python chiama
//! `date.today()`; qui `oggi` e' un argomento. E' la stessa scelta di
//! `nova-calendario` e di `nova-pianificazione`: una funzione che legge
//! l'orologio non si puo' provare due volte con lo stesso risultato, e il
//! banco che la confronta col Python fallirebbe a mezzanotte.

pub mod slug;
pub use slug::slug;

// Cosa succede quando NOVA impara qualcosa su un fatto che sa gia'. E'
// la parte di `upsert` che non tocca il disco, ed e' quella dove la
// memoria si corrompe in silenzio se la regola e' sbagliata.
pub mod fusione;

pub const STATUS_ATTIVO: &str = "attivo";
pub const STATUS_ARCHIVIATO: &str = "archiviato";

pub const ORIGINE_UTENTE: &str = "utente";
pub const ORIGINE_AUTO: &str = "auto";
pub const ORIGINE_SCANSIONE: &str = "scansione";

/// I tipi previsti. Il modello puo' comunque inventarne altri, e va bene:
/// e' un'etichetta per ritrovarsi, non uno schema da far rispettare.
pub const TIPI: &[&str] = &[
    "profilo", "preferenza", "progetto", "app", "persona", "luogo",
    "abitudine", "fatto", "nota", "hub",
];

#[derive(Debug, Clone, PartialEq)]
pub struct Nodo {
    pub slug: String,
    pub title: String,
    pub body: String,
    pub tipo: String,
    pub tags: Vec<String>,
    pub relazioni: Vec<String>,
    pub area: String,
    pub status: String,
    pub origine: String,
    pub confidenza: f64,
    pub riferimenti: Vec<String>,
    pub creato: String,
    pub aggiornato: String,
}

impl Default for Nodo {
    fn default() -> Self {
        Self {
            slug: String::new(),
            title: String::new(),
            body: String::new(),
            tipo: "fatto".into(),
            tags: Vec::new(),
            relazioni: Vec::new(),
            area: "Generale".into(),
            status: STATUS_ATTIVO.into(),
            origine: ORIGINE_AUTO.into(),
            confidenza: 0.7,
            riferimenti: Vec::new(),
            creato: String::new(),
            aggiornato: String::new(),
        }
    }
}

impl Nodo {
    /// Il testo su cui lavorano BM25 e l'embedder.
    pub fn testo_indicizzabile(&self) -> String {
        format!("{}\n{}\n{}", self.title, self.tags.join(" "), self.body)
    }

    /// I `[[wikilink]]` scritti nel corpo, gia' ridotti a slug.
    pub fn wikilink_nel_corpo(&self) -> Vec<String> {
        let mut fuori = Vec::new();
        let b: Vec<char> = self.body.chars().collect();
        let mut i = 0;
        while i + 1 < b.len() {
            if b[i] == '[' && b[i + 1] == '[' {
                let inizio = i + 2;
                let mut j = inizio;
                while j < b.len() && b[j] != ']' && b[j] != '|' {
                    j += 1;
                }
                if j > inizio {
                    fuori.push(slug(&b[inizio..j].iter().collect::<String>()));
                }
                i = j;
            } else {
                i += 1;
            }
        }
        fuori
    }

    /// Le relazioni dichiarate piu' quelle scritte nel corpo, senza doppioni
    /// e senza se stesso. L'ordine e' quello di apparizione: le dichiarate
    /// prima, come in `dict.fromkeys`.
    pub fn tutte_le_relazioni(&self) -> Vec<String> {
        let mut viste = Vec::new();
        for r in self.relazioni.iter().cloned().chain(self.wikilink_nel_corpo()) {
            if !r.is_empty() && r != self.slug && !viste.contains(&r) {
                viste.push(r);
            }
        }
        viste
    }

    /// Il file, cosi' come va scritto. `oggi` in forma `AAAA-MM-GG`.
    pub fn a_markdown(&self, oggi: &str) -> String {
        let creato = if self.creato.is_empty() { oggi } else { &self.creato };
        let campi: Vec<(&str, String)> = vec![
            ("title", una_riga(&self.title)),
            ("tipo", una_riga(&self.tipo)),
            ("tags", lista(&self.tags)),
            ("relazioni", lista(&self.relazioni)),
            ("area", una_riga(&self.area)),
            ("status", una_riga(&self.status)),
            ("origine", una_riga(&self.origine)),
            ("confidenza", due_decimali(self.confidenza)),
            ("riferimenti", lista(&self.riferimenti)),
            ("creato", una_riga(creato)),
            ("aggiornato", una_riga(oggi)),
        ];
        let mut righe = vec!["---".to_string()];
        for (k, v) in campi {
            righe.push(format!("{k}: {v}"));
        }
        righe.push("---".into());
        righe.push(String::new());
        righe.push(self.body.trim().to_string());
        righe.push(String::new());
        righe.join("\n")
    }

    /// Rilegge un file. `slug` viene dal nome del file, non dal contenuto:
    /// e' il nome a dire chi e' il nodo, se no rinominare un file lo sdoppia.
    pub fn da_markdown(testo: &str, slug_del_file: &str) -> Nodo {
        let (fm, corpo) = dividi_frontmatter(testo);
        let prendi = |k: &str| fm.iter().find(|(c, _)| c == k).map(|(_, v)| v.clone());
        let o = |k: &str, se_vuoto: &str| {
            prendi(k).filter(|s| !s.is_empty()).unwrap_or_else(|| se_vuoto.to_string())
        };
        Nodo {
            slug: slug_del_file.to_string(),
            title: o("title", &slug_del_file.replace('-', " ")),
            body: corpo.trim().to_string(),
            tipo: o("tipo", "fatto"),
            tags: come_lista(prendi("tags").as_deref()),
            relazioni: come_lista(prendi("relazioni").as_deref())
                .iter()
                .map(|r| slug(r))
                .collect(),
            area: o("area", "Generale"),
            status: o("status", STATUS_ATTIVO),
            origine: o("origine", ORIGINE_AUTO),
            confidenza: come_numero(prendi("confidenza").as_deref(), 0.7),
            riferimenti: come_lista(prendi("riferimenti").as_deref()),
            creato: prendi("creato").unwrap_or_default(),
            aggiornato: prendi("aggiornato").unwrap_or_default(),
        }
    }
}

/// Un valore su una riga sola.
///
/// Il parser del frontmatter e' «una chiave per riga»: un a capo dentro un
/// valore produceva una riga senza «:», che veniva scartata in silenzio — e
/// parte del contenuto spariva alla prima riscrittura del file.
pub fn una_riga(v: &str) -> String {
    let mut fuori = String::with_capacity(v.len());
    let mut spazio = false;
    for c in v.chars() {
        if c.is_whitespace() {
            spazio = true;
        } else {
            if spazio && !fuori.is_empty() {
                fuori.push(' ');
            }
            spazio = false;
            fuori.push(c);
        }
    }
    fuori
}

fn lista(v: &[String]) -> String {
    let dentro: Vec<String> = v.iter().map(|x| una_riga(x).replace(',', " ")).collect();
    format!("[{}]", dentro.join(", "))
}

/// Due decimali, come `round(x, 2)` di Python: a meta' si va al pari.
fn due_decimali(x: f64) -> String {
    let s = x * 100.0;
    let giu = s.floor();
    let resto = s - giu;
    let n = if (resto - 0.5).abs() < 1e-9 {
        if (giu as i64) % 2 == 0 { giu } else { giu + 1.0 }
    } else if resto > 0.5 {
        giu + 1.0
    } else {
        giu
    } / 100.0;
    // Come lo stampa `str(float)` di Python: 0.9 resta «0.9», ma 1.0 resta
    // «1.0» e non diventa «1». Rust con `{}` toglie il decimale agli interi,
    // e quella differenza di un carattere basta a far divergere il file.
    if n.fract() == 0.0 {
        format!("{n:.1}")
    } else {
        format!("{n}")
    }
}

pub fn come_lista(v: Option<&str>) -> Vec<String> {
    let s = match v {
        None => return Vec::new(),
        Some(s) => s.trim(),
    };
    let dentro = s.strip_prefix('[').and_then(|x| x.strip_suffix(']')).unwrap_or(s);
    dentro
        .split(',')
        .map(|p| p.trim().trim_matches(|c| c == '\'' || c == '"').to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

pub fn come_numero(v: Option<&str>, se_no: f64) -> f64 {
    v.and_then(|s| s.trim().parse::<f64>().ok()).unwrap_or(se_no)
}

/// Parser minimale: `chiave: valore`, niente nidificazioni.
pub fn dividi_frontmatter(testo: &str) -> (Vec<(String, String)>, String) {
    if !testo.starts_with("---") {
        return (Vec::new(), testo.to_string());
    }
    let dopo = &testo[3..];
    let fine = match dopo.find("\n---") {
        None => return (Vec::new(), testo.to_string()),
        Some(k) => k,
    };
    let blocco = &dopo[..fine];
    let corpo = &dopo[fine + 4..];
    let mut fm = Vec::new();
    for riga in blocco.lines() {
        let riga = riga.trim_end();
        if riga.trim().is_empty() || riga.trim_start().starts_with('#') {
            continue;
        }
        if let Some(k) = riga.find(':') {
            fm.push((riga[..k].trim().to_string(), riga[k + 1..].trim().to_string()));
        }
    }
    (fm, corpo.to_string())
}

#[cfg(test)]
mod prove {
    use super::*;

    fn nodo() -> Nodo {
        Nodo {
            slug: "il-gatto-di-gio".into(),
            title: "Il gatto di Gio".into(),
            body: "Si chiama Ugo. Vive con [[gio]] e con [[il-cane|il cane]].".into(),
            tipo: "persona".into(),
            tags: vec!["gatto".into(), "casa".into()],
            relazioni: vec!["gio".into()],
            confidenza: 0.9,
            ..Default::default()
        }
    }

    #[test]
    fn il_giro_completo_torna_uguale() {
        let n = nodo();
        let testo = n.a_markdown("2026-09-03");
        let riletto = Nodo::da_markdown(&testo, &n.slug);
        assert_eq!(riletto.title, n.title);
        assert_eq!(riletto.body, n.body);
        assert_eq!(riletto.tags, n.tags);
        assert_eq!(riletto.tipo, n.tipo);
        assert_eq!(riletto.confidenza, n.confidenza);
    }

    #[test]
    fn i_wikilink_diventano_relazioni_senza_doppioni() {
        let r = nodo().tutte_le_relazioni();
        assert_eq!(r, vec!["gio", "il-cane"], "«gio» c'e' due volte e conta una");
    }

    #[test]
    fn un_nodo_non_e_in_relazione_con_se_stesso() {
        let mut n = nodo();
        n.body = "vedi [[il-gatto-di-gio]]".into();
        assert!(!n.tutte_le_relazioni().contains(&n.slug));
    }

    #[test]
    fn un_a_capo_dentro_un_valore_non_spezza_il_frontmatter() {
        // Il difetto vero: una riga senza «:» veniva scartata in silenzio e
        // meta' del titolo spariva alla prima riscrittura.
        let mut n = nodo();
        n.title = "Un titolo\ncon un a capo".into();
        let testo = n.a_markdown("2026-09-03");
        assert!(!testo.contains("Un titolo\ncon"), "l'a capo e' rimasto");
        let riletto = Nodo::da_markdown(&testo, &n.slug);
        assert_eq!(riletto.title, "Un titolo con un a capo");
    }

    #[test]
    fn una_virgola_dentro_un_elemento_non_ne_crea_due() {
        let mut n = nodo();
        n.tags = vec!["casa, mia".into()];
        let riletto = Nodo::da_markdown(&n.a_markdown("2026-09-03"), &n.slug);
        assert_eq!(riletto.tags.len(), 1, "{:?}", riletto.tags);
    }

    #[test]
    fn senza_frontmatter_il_testo_e_tutto_corpo() {
        let n = Nodo::da_markdown("solo del testo", "x");
        assert_eq!(n.body, "solo del testo");
        assert_eq!(n.tipo, "fatto", "e i valori di fabbrica reggono");
        assert_eq!(n.confidenza, 0.7);
    }

    #[test]
    fn la_data_di_creazione_si_conserva_e_quella_di_modifica_no() {
        let mut n = nodo();
        n.creato = "2026-01-01".into();
        let t = n.a_markdown("2026-09-03");
        assert!(t.contains("creato: 2026-01-01"), "{t}");
        assert!(t.contains("aggiornato: 2026-09-03"), "{t}");
    }
}
