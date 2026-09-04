//! Come si racconta la memoria di NOVA al modello.
//!
//! Il vault sa gia' cercare, salvare e collegare — sta in `nova-nodi` e
//! `nova-memoria`. Qui c'e' l'ultimo pezzo, quello che sembra il piu'
//! innocuo: **come si scrive un ricordo** perche' il modello lo legga.
//!
//! Non e' innocuo. Un nodo raccontato senza il tipo, o senza la confidenza,
//! e' un nodo che il modello tratta come una certezza; uno raccontato senza
//! i collegamenti e' un nodo isolato in una memoria che invece e' un grafo. E
//! un corpo tagliato senza dirlo e' peggio di un corpo tagliato: il modello
//! crede di aver letto tutto.

/// Quanto corpo di un nodo entra in un risultato di ricerca.
pub const MAX_CORPO_TROVATO: usize = 1400;

/// Quanti collegamenti si nominano per ogni nodo trovato.
pub const MAX_RELAZIONI: usize = 5;

/// Un nodo, per quel poco che serve a raccontarlo.
pub struct Trovato<'a> {
    pub slug: &'a str,
    pub titolo: &'a str,
    pub tipo: &'a str,
    pub corpo: &'a str,
    pub confidenza: f64,
    pub via: &'a str,
    pub relazioni: Vec<String>,
}

/// Un nodo trovato, come lo legge il modello.
///
/// Il corpo va **rientrato di due spazi**: senza, un nodo di dieci righe si
/// confonde col nodo dopo, e il modello non vede piu' dove finisce un
/// ricordo e ne comincia un altro.
pub fn racconta(t: &Trovato) -> String {
    let mut corpo = t.corpo.trim().to_string();
    if corpo.chars().count() > MAX_CORPO_TROVATO {
        // Il taglio si **dichiara**: un corpo accorciato in silenzio fa
        // credere al modello di aver letto tutto.
        corpo = corpo.chars().take(MAX_CORPO_TROVATO).collect::<String>() + " [...]";
    }
    let corpo = corpo.replace('\n', "\n  ");
    let rel = if t.relazioni.is_empty() {
        "-".to_string()
    } else {
        t.relazioni
            .iter()
            .take(MAX_RELAZIONI)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "[{}] {}  ({}, conf {}, via {})\n  {}\n  collegato a: {}",
        t.slug, t.titolo, t.tipo, due_decimali(t.confidenza), t.via, corpo, rel
    )
}

/// Piu' nodi trovati, separati da una riga vuota.
pub fn racconta_tutti(trovati: &[Trovato]) -> String {
    trovati.iter().map(racconta).collect::<Vec<_>>().join("\n\n")
}

/// Quando non c'e' niente, si dice **anche cosa fare**.
///
/// «Nessun risultato» chiude il discorso; questa frase lo apre: se il modello
/// ha appena imparato qualcosa, sa dove metterlo.
pub fn niente_in_memoria(domanda: &str) -> String {
    format!(
        "Nessun nodo in memoria per '{domanda}'. \
         Se impari qualcosa, salvalo con kb_note."
    )
}

/// I vicini di un nodo.
pub fn racconta_vicini(
    slug: &str,
    titolo: &str,
    vicini: &[(String, String, String)],
) -> String {
    if vicini.is_empty() {
        return format!("[{slug}] '{titolo}' non ha collegamenti.");
    }
    let mut righe = vec![format!("[{slug}] {titolo} -> {} collegamenti:", vicini.len())];
    for (s, t, tipo) in vicini {
        righe.push(format!("  [{s}] {t} ({tipo})"));
    }
    righe.join("\n")
}

/// Due decimali, come `f"{x:.2f}"` di Python.
///
/// Si usa il formattatore invece di fare il conto a mano, e la ragione l'ha
/// trovata il banco: `0.955` in binario e' **poco meno** di 0.955
/// (`0.95499999999999996`), quindi Python scrive `0.95`. Moltiplicando per
/// cento il prodotto arriva esattamente a `95.5` e l'arrotondamento al pari
/// da' `0.96`. L'errore non e' nell'arrotondamento: e' nella moltiplicazione,
/// che il formattatore non fa — lui guarda il valore vero del numero.
///
/// E' il contrario di cio' che ho fatto per la misura di un file, dove il
/// conto a mano va bene: li' i valori sono interi divisi per potenze di 1024,
/// che in binario sono esatti, e non c'e' nessun errore da introdurre.
/// Le due scelte sembrano incoerenti e non lo sono: dipende da che numeri
/// passano di li'.
fn due_decimali(x: f64) -> String {
    format!("{x:.2}")
}

#[cfg(test)]
mod prove {
    use super::*;

    fn t<'a>(corpo: &'a str, rel: &[&str]) -> Trovato<'a> {
        Trovato {
            slug: "persona-anna",
            titolo: "Anna",
            tipo: "persona",
            corpo,
            confidenza: 0.7,
            via: "fusione",
            relazioni: rel.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn un_ricordo_si_racconta_col_tipo_e_con_quanto_crederci() {
        let r = racconta(&t("Anna e' una collega.", &["progetto-nova"]));
        assert!(r.starts_with("[persona-anna] Anna  (persona, conf 0.70, via fusione)"), "{r}");
        assert!(r.contains("\n  Anna e' una collega."));
        assert!(r.ends_with("collegato a: progetto-nova"));
    }

    #[test]
    fn un_nodo_senza_collegamenti_lo_dice_con_un_trattino() {
        let r = racconta(&t("x", &[]));
        assert!(r.ends_with("collegato a: -"), "{r}");
    }

    #[test]
    fn il_corpo_si_rientra_o_i_ricordi_si_confondono() {
        // Senza il rientro, un nodo di dieci righe si attacca al successivo e
        // il modello non vede piu' dove finisce un ricordo.
        let r = racconta(&t("riga uno\nriga due", &[]));
        assert!(r.contains("\n  riga uno\n  riga due"), "{r}");
    }

    #[test]
    fn un_corpo_tagliato_lo_dichiara() {
        let lungo = "x".repeat(MAX_CORPO_TROVATO + 50);
        let r = racconta(&t(&lungo, &[]));
        assert!(r.contains(" [...]"), "un taglio taciuto fa credere di aver letto tutto");
    }

    #[test]
    fn di_collegamenti_se_ne_nominano_cinque() {
        let molti: Vec<&str> = vec!["a", "b", "c", "d", "e", "f", "g"];
        let r = racconta(&t("x", &molti));
        assert!(r.ends_with("collegato a: a, b, c, d, e"), "{r}");
    }

    #[test]
    fn la_confidenza_si_scrive_con_due_decimali_al_pari() {
        assert_eq!(due_decimali(0.7), "0.70");
        assert_eq!(due_decimali(1.0), "1.00");
        assert_eq!(due_decimali(0.125), "0.12", "a meta' si arrotonda al pari");
        assert_eq!(due_decimali(0.135), "0.14");
        assert_eq!(due_decimali(0.0), "0.00");
    }

    #[test]
    fn quando_non_ce_niente_si_dice_anche_cosa_fare() {
        let r = niente_in_memoria("gatto");
        assert!(r.contains("kb_note"), "«nessun risultato» chiude il discorso: {r}");
    }

    #[test]
    fn i_vicini_si_contano_e_si_elencano() {
        let v = vec![("a".into(), "Alfa".into(), "fatto".into())];
        let r = racconta_vicini("x", "Ics", &v);
        assert_eq!(r, "[x] Ics -> 1 collegamenti:\n  [a] Alfa (fatto)");
        assert_eq!(racconta_vicini("x", "Ics", &[]), "[x] 'Ics' non ha collegamenti.");
    }
}
