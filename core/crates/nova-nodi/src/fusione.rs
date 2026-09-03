//! Cosa succede quando NOVA impara qualcosa su un fatto che sa gia'.
//!
//! E' la parte di `Vault.upsert` che non tocca il disco, ed e' quella dove la
//! memoria si corrompe in silenzio se la regola e' sbagliata: nessuno se ne
//! accorge, perche' un nodo peggiorato ha lo stesso aspetto di un nodo giusto.
//!
//! I commenti qui sotto non sono spiegazioni: sono difetti gia' successi.

use crate::{slug, Nodo, ORIGINE_AUTO, ORIGINE_SCANSIONE, ORIGINE_UTENTE};

/// Quanto vale una fonte. Chi l'ha detto conta piu' di quante volte e' stato
/// detto: se l'utente lo ha affermato, l'osservazione automatica non lo
/// declassa.
pub fn peso_origine(o: &str) -> u8 {
    match o {
        ORIGINE_UTENTE => 3,
        ORIGINE_SCANSIONE => 2,
        ORIGINE_AUTO => 1,
        _ => 0,
    }
}

/// I tipi che possono assorbire o essere assorbiti da chiunque: il
/// contenitore generico in cui finisce quello che NOVA impara da sola.
pub const TIPI_GENERICI: [&str; 3] = ["fatto", "nota", ""];

pub fn generico(t: &str) -> bool {
    TIPI_GENERICI.contains(&t)
}

pub fn tipi_compatibili(a: &str, b: &str) -> bool {
    a == b || generico(a) || generico(b)
}

/// Un «fatto» che confluisce in una persona non la trasforma in un fatto.
///
/// L'estrattore mette **sempre** un tipo (di fabbrica «fatto»), quindi un
/// `nuovo.tipo or vecchio.tipo` sceglieva sempre il nuovo: la prima
/// annotazione generica su Anna declassava `persona-anna` a «fatto», e il
/// file restava in `02-persone` mentre l'indice diceva `06-fatti`.
pub fn tipo_piu_specifico<'a>(vecchio: &'a str, nuovo: &'a str) -> &'a str {
    if !nuovo.is_empty() && !generico(nuovo) {
        return nuovo;
    }
    if !vecchio.is_empty() && !generico(vecchio) {
        return vecchio;
    }
    if nuovo.is_empty() { vecchio } else { nuovo }
}

pub const MAX_CORPO: usize = 4000;

/// Tiene la definizione originale e le annotazioni piu' recenti.
///
/// Le vecchie si perdono, ma si perdevano comunque: il recupero tronca a 950
/// caratteri, quindi oltre un certo punto stavano solo occupando disco e
/// rallentando ogni ricerca.
///
/// Anche il **primo blocco** ha una quota. Senza, un nodo il cui primo
/// paragrafo da solo superava il tetto restava congelato per sempre: c'era
/// spazio per la testa e per nient'altro, quindi ogni fatto nuovo veniva
/// scartato in silenzio a ogni scrittura successiva. Il fatto piu' recente
/// entra sempre, tagliato se serve.
pub fn limita_corpo(corpo: &str, massimo: usize) -> String {
    let corpo = corpo.trim();
    if conta(corpo) <= massimo {
        return corpo.to_string();
    }
    let blocchi: Vec<&str> = corpo
        .split("\n\n")
        .map(|b| b.trim())
        .filter(|b| !b.is_empty())
        .collect();
    if blocchi.is_empty() {
        return taglia(corpo, massimo);
    }
    let quota_testa = std::cmp::max(120, massimo / 3);
    let mut testa = blocchi[0].to_string();
    if conta(&testa) > quota_testa {
        testa = format!("{} [...]", taglia(&testa, quota_testa).trim_end());
    }
    if blocchi.len() == 1 {
        return testa;
    }
    let avviso = |n: usize| format!("> [{n} annotazioni piu' vecchie rimosse]");
    let disponibile = massimo as i64
        - conta(&testa) as i64
        - conta(&avviso(blocchi.len())) as i64
        - 4;
    if disponibile <= 0 {
        return testa;
    }
    let disponibile = disponibile as usize;
    let mut coda: Vec<String> = Vec::new();
    let mut usati = 0usize;
    for (i, b) in blocchi[1..].iter().rev().enumerate() {
        let spazio = disponibile as i64 - usati as i64 - 2;
        if spazio <= 0 {
            break;
        }
        let spazio = spazio as usize;
        let mut b = (*b).to_string();
        if conta(&b) > spazio {
            if i > 0 {
                // non e' il piu' recente: si rinuncia invece di troncarlo
                break;
            }
            let quanto = spazio.saturating_sub(6);
            b = format!("{} [...]", taglia(&b, quanto).trim_end());
        }
        usati += conta(&b) + 2;
        coda.push(b);
    }
    coda.reverse();
    let persi = blocchi.len() - 1 - coda.len();
    let mut pezzi = vec![testa];
    if persi > 0 {
        pezzi.push(avviso(persi));
    }
    pezzi.extend(coda);
    pezzi.join("\n\n")
}

/// Python conta i **caratteri**, non i byte: con un accento dentro, `len()` in
/// Rust darebbe uno in piu' e i tagli cadrebbero in un altro punto.
fn conta(s: &str) -> usize {
    s.chars().count()
}

fn taglia(s: &str, quanti: usize) -> String {
    s.chars().take(quanti).collect()
}

/// Aggiorna un nodo esistente senza perdere quello che c'era.
pub fn fondi(vecchio: &Nodo, nuovo: &Nodo) -> Nodo {
    let corpo_nuovo = nuovo.body.trim();
    let corpo_vecchio = vecchio.body.trim();
    let ripetuto = !corpo_nuovo.is_empty() && corpo_vecchio.contains(corpo_nuovo);
    let corpo = if !corpo_nuovo.is_empty() && !ripetuto {
        if corpo_vecchio.is_empty() {
            corpo_nuovo.to_string()
        } else {
            format!("{corpo_vecchio}\n\n{corpo_nuovo}").trim().to_string()
        }
    } else {
        corpo_vecchio.to_string()
    };
    let corpo = limita_corpo(&corpo, MAX_CORPO);

    // Confermata, non riformulata. Alzare la confidenza ogni volta che il
    // testo e' *diverso* premiava la variazione lessicale — cioe' proprio il
    // caso in cui NOVA non ha imparato niente di nuovo. Sale quando lo stesso
    // fatto torna identico da una seconda osservazione.
    let mut confidenza = vecchio.confidenza.max(nuovo.confidenza);
    if ripetuto {
        confidenza = (confidenza + 0.05).min(1.0);
    }

    // A parita' di peso vince il vecchio, come `max` di Python su una coppia:
    // torna il primo dei due massimi.
    let origine = if peso_origine(&nuovo.origine) > peso_origine(&vecchio.origine) {
        nuovo.origine.clone()
    } else {
        vecchio.origine.clone()
    };

    Nodo {
        slug: vecchio.slug.clone(),
        title: se_vuoto(&nuovo.title, &vecchio.title),
        body: corpo,
        tipo: tipo_piu_specifico(&vecchio.tipo, &nuovo.tipo).to_string(),
        tags: unisci(&vecchio.tags, &nuovo.tags),
        relazioni: unisci(&vecchio.relazioni, &nuovo.relazioni),
        area: se_vuoto(&nuovo.area, &vecchio.area),
        status: se_vuoto(&nuovo.status, &vecchio.status),
        origine,
        confidenza,
        riferimenti: unisci(&vecchio.riferimenti, &nuovo.riferimenti),
        creato: vecchio.creato.clone(),
        aggiornato: vecchio.aggiornato.clone(),
    }
}

fn se_vuoto(preferito: &str, ripiego: &str) -> String {
    if preferito.is_empty() { ripiego.to_string() } else { preferito.to_string() }
}

/// Come `dict.fromkeys`: l'ordine di apparizione, senza doppioni.
fn unisci(a: &[String], b: &[String]) -> Vec<String> {
    let mut fuori: Vec<String> = Vec::new();
    for x in a.iter().chain(b.iter()) {
        if !fuori.contains(x) {
            fuori.push(x.clone());
        }
    }
    fuori
}

/// I prefissi che l'estrattore mette davanti agli slug per tipo.
pub const PREFISSI: [&str; 5] = ["progetto-", "persona-", "app-", "luogo-", "nodo-"];

pub fn senza_prefisso(s: &str) -> String {
    for p in PREFISSI {
        if let Some(resto) = s.strip_prefix(p) {
            return resto.to_string();
        }
    }
    s.to_string()
}

/// Rinomina i `[[wikilink]]` che puntano a `vecchio`, lasciando gli altri.
///
/// Si confronta lo **slug** del bersaglio, non il testo: `[[Il Gatto]]` e
/// `[[il-gatto]]` puntano allo stesso nodo, e rinominarne uno solo lascerebbe
/// un collegamento rotto che nessuno vede finche' non ci si clicca sopra.
pub fn rinomina_wikilink(corpo: &str, vecchio: &str, nuovo: &str) -> String {
    let b: Vec<char> = corpo.chars().collect();
    let mut fuori = String::with_capacity(corpo.len());
    let mut i = 0usize;
    while i < b.len() {
        if i + 1 < b.len() && b[i] == '[' && b[i + 1] == '[' {
            if let Some((bersaglio, etichetta, fine)) = wikilink(&b, i) {
                if slug(&bersaglio) == vecchio {
                    fuori.push_str(&format!("[[{nuovo}{etichetta}]]"));
                } else {
                    fuori.extend(&b[i..fine]);
                }
                i = fine;
                continue;
            }
        }
        fuori.push(b[i]);
        i += 1;
    }
    fuori
}

/// `[[bersaglio|etichetta]]` a partire da `i`: (bersaglio, «|etichetta» o
/// vuoto, dove finisce). `None` se non e' un wikilink chiuso.
fn wikilink(b: &[char], i: usize) -> Option<(String, String, usize)> {
    let inizio = i + 2;
    let mut j = inizio;
    while j < b.len() && b[j] != ']' && b[j] != '|' {
        j += 1;
    }
    if j >= b.len() || j == inizio {
        return None;
    }
    let bersaglio: String = b[inizio..j].iter().collect();
    let mut etichetta = String::new();
    if b[j] == '|' {
        let da = j;
        while j < b.len() && b[j] != ']' {
            j += 1;
        }
        if j >= b.len() {
            return None;
        }
        etichetta = b[da..j].iter().collect();
    }
    if j + 1 < b.len() && b[j] == ']' && b[j + 1] == ']' {
        Some((bersaglio, etichetta, j + 2))
    } else {
        None
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    fn n(body: &str, tipo: &str, conf: f64, origine: &str) -> Nodo {
        Nodo {
            slug: "x".into(), title: "X".into(), body: body.into(),
            tipo: tipo.into(), confidenza: conf, origine: origine.into(),
            ..Default::default()
        }
    }

    #[test]
    fn un_fatto_generico_non_declassa_una_persona() {
        assert_eq!(tipo_piu_specifico("persona", "fatto"), "persona");
        assert_eq!(tipo_piu_specifico("fatto", "persona"), "persona");
        assert_eq!(tipo_piu_specifico("fatto", "nota"), "nota");
    }

    #[test]
    fn il_corpo_nuovo_si_aggiunge_e_quello_ripetuto_no() {
        let v = n("Sa il francese.", "persona", 0.7, ORIGINE_AUTO);
        let aggiunto = fondi(&v, &n("Vive a Roma.", "fatto", 0.7, ORIGINE_AUTO));
        assert!(aggiunto.body.contains("Sa il francese."));
        assert!(aggiunto.body.contains("Vive a Roma."));

        let uguale = fondi(&v, &n("Sa il francese.", "fatto", 0.7, ORIGINE_AUTO));
        assert_eq!(uguale.body, "Sa il francese.", "niente doppioni");
    }

    #[test]
    fn la_conferma_alza_la_confidenza_la_riformulazione_no() {
        let v = n("Sa il francese.", "persona", 0.7, ORIGINE_AUTO);
        let confermato = fondi(&v, &n("Sa il francese.", "fatto", 0.7, ORIGINE_AUTO));
        assert!((confermato.confidenza - 0.75).abs() < 1e-9, "{}", confermato.confidenza);
        let riformulato = fondi(&v, &n("Conosce il francese.", "fatto", 0.7, ORIGINE_AUTO));
        assert!((riformulato.confidenza - 0.7).abs() < 1e-9,
                "premiare la variazione lessicale premia il non aver imparato niente");
    }

    #[test]
    fn losservazione_automatica_non_declassa_cio_che_ha_detto_lutente() {
        let v = n("x", "fatto", 0.9, ORIGINE_UTENTE);
        assert_eq!(fondi(&v, &n("y", "fatto", 0.5, ORIGINE_AUTO)).origine, ORIGINE_UTENTE);
        let a = n("x", "fatto", 0.5, ORIGINE_AUTO);
        assert_eq!(fondi(&a, &n("y", "fatto", 0.9, ORIGINE_UTENTE)).origine, ORIGINE_UTENTE);
    }

    #[test]
    fn il_fatto_piu_recente_entra_sempre_anche_se_la_testa_e_enorme() {
        // Il difetto: un primo paragrafo piu' lungo del tetto congelava il
        // nodo per sempre, e ogni fatto nuovo spariva in silenzio.
        let testa = "a".repeat(5000);
        let fuori = limita_corpo(&format!("{testa}\n\nfatto nuovo"), MAX_CORPO);
        assert!(fuori.contains("fatto nuovo"), "il fatto nuovo e' sparito");
        assert!(fuori.len() <= MAX_CORPO + 64);
    }

    #[test]
    fn i_wikilink_si_rinominano_per_slug_non_per_testo() {
        let c = "vedi [[Il Gatto]] e [[il-gatto|il gatto]] e [[altro]]";
        let f = rinomina_wikilink(c, "il-gatto", "ugo");
        assert_eq!(f, "vedi [[ugo]] e [[ugo|il gatto]] e [[altro]]");
    }

    #[test]
    fn un_wikilink_non_chiuso_resta_com_e() {
        assert_eq!(rinomina_wikilink("[[aperto", "aperto", "x"), "[[aperto");
    }

    #[test]
    fn i_prefissi_si_tolgono_solo_in_testa() {
        assert_eq!(senza_prefisso("persona-anna"), "anna");
        assert_eq!(senza_prefisso("anna-persona"), "anna-persona");
    }
}
