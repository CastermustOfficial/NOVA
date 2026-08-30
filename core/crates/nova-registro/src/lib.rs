//! Il registro delle azioni che non si annullano: cercarlo e raccontarlo.
//!
//! Terzo pezzo portato dal Python. I primi due erano aritmetica; questo e'
//! il primo che tocca la promessa su cui NOVA sta in piedi — «cio' che non si
//! annulla, si annota» — e quindi il primo dove una differenza fra le due
//! versioni non sarebbe un dettaglio di prestazioni: sarebbe una candidatura
//! che non si ritrova piu'.
//!
//! Anche qui il disco resta fuori. Le righe arrivano gia' lette, e la parte
//! che le legge e le scrive e' quella che parla col sistema: va scritta
//! contro il trait di `nova-platform` quando ci si arriva. Cercare e
//! raccontare no, quelli sono testo.
//!
//! Le regole sono quelle del Python fino all'ultima: le parole si cercano
//! **tutte**, in qualunque campo e in qualunque ordine, senza accenti e senza
//! maiuscole. Chi cerca «societa» deve trovare «Societa'», e chi scrive di
//! fretta non mette le maiuscole.

use std::collections::HashMap;

/// Una riga del registro, ridotta a cio' su cui si cerca.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Riga {
    /// ISO 8601, come la scrive il Python: `2026-08-30T14:18:05`.
    pub quando: String,
    pub tipo: String,
    pub azione: String,
    pub dove: String,
    pub dettagli: String,
    pub esito: String,
}

impl Riga {
    /// Tutto il testo su cui si cerca, in un pezzo solo.
    fn cercabile(&self) -> String {
        format!("{} {} {} {} {}",
                self.azione, self.dove, self.dettagli, self.esito, self.tipo)
    }
}

/// Toglie accenti e maiuscole. Le stesse lettere del Python: quelle che
/// compaiono in italiano e nelle lingue vicine.
pub fn piatto(s: &str) -> String {
    s.chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            'ý' | 'ÿ' => 'y',
            altro => altro,
        })
        .collect()
}

/// Cosa si sta cercando.
#[derive(Debug, Clone, Default)]
pub struct Filtro {
    pub testo: String,
    pub tipo: String,
    pub esito: String,
    /// 0 = tutte. Le righe sono ordinate dalla piu' recente, e il taglio si
    /// fa confrontando la data in testa: e' testo ISO, quindi l'ordine
    /// alfabetico e' l'ordine cronologico.
    pub non_prima_di: String,
    pub quante: usize,
}

/// Le righe che rispondono al filtro, nell'ordine in cui arrivano.
///
/// Chi chiama passa le righe gia' ordinate dalla piu' recente, come fa
/// `leggi()` dalla parte Python.
pub fn cerca<'a>(righe: &'a [Riga], f: &Filtro) -> Vec<&'a Riga> {
    let parole: Vec<String> = piatto(&f.testo)
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    let mut fuori = Vec::new();
    for r in righe {
        if !f.tipo.is_empty() && r.tipo != f.tipo {
            continue;
        }
        if !f.esito.is_empty() && !r.esito.contains(&f.esito) {
            continue;
        }
        if !f.non_prima_di.is_empty() && r.quando.as_str() < f.non_prima_di.as_str() {
            continue;
        }
        if !parole.is_empty() {
            let dentro = piatto(&r.cercabile());
            if !parole.iter().all(|p| dentro.contains(p.as_str())) {
                continue;
            }
        }
        fuori.push(r);
        if f.quante > 0 && fuori.len() >= f.quante {
            break;
        }
    }
    fuori
}

/// Quante azioni, di che tipo, da quando a quando.
#[derive(Debug, Default, PartialEq)]
pub struct Riassunto {
    pub quante: usize,
    /// tipo -> conteggio, dal piu' frequente.
    pub tipi: Vec<(String, usize)>,
    pub prima: String,
    pub ultima: String,
}

pub fn riassunto(righe: &[Riga]) -> Riassunto {
    if righe.is_empty() {
        return Riassunto::default();
    }
    let mut conteggi: HashMap<&str, usize> = HashMap::new();
    for r in righe {
        let t = if r.tipo.is_empty() { "?" } else { r.tipo.as_str() };
        *conteggi.entry(t).or_insert(0) += 1;
    }
    let mut tipi: Vec<(String, usize)> =
        conteggi.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    // Dal piu' frequente; a pari merito in ordine alfabetico, se no due
    // esecuzioni della stessa cosa danno due elenchi diversi.
    tipi.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    Riassunto {
        quante: righe.len(),
        tipi,
        // Le righe arrivano dalla piu' recente: la prima e' l'ultima in
        // ordine di tempo, e viceversa.
        prima: righe[righe.len() - 1].quando.chars().take(10).collect(),
        ultima: righe[0].quando.chars().take(10).collect(),
    }
}

/// `2026-08-30` diventa `30/08/2026`. Una data ISO non e' una data che si
/// legge: e' un formato per le macchine, e questo lo leggono le persone.
pub fn data_italiana(iso: &str) -> String {
    let p: Vec<&str> = iso.splitn(3, '-').collect();
    if p.len() == 3 && p[0].len() == 4 {
        format!("{}/{}/{}", p[2], p[1], p[0])
    } else {
        iso.to_string()
    }
}

/// «oggi», «ieri», oppure la data. Un timestamp non e' un giorno.
///
/// `oggi` si passa da fuori (formato `AAAA-MM-GG`): dentro non c'e' un
/// orologio, cosi' la funzione e' provabile senza aspettare domani.
pub fn giorno(iso: &str, oggi: &str) -> String {
    let g: String = iso.chars().take(10).collect();
    if g == oggi {
        return "oggi".into();
    }
    if g == ieri(oggi) {
        return "ieri".into();
    }
    data_italiana(&g)
}

/// Il giorno prima, in aritmetica civile: niente fusi, niente ore.
fn ieri(oggi: &str) -> String {
    let p: Vec<&str> = oggi.splitn(3, '-').collect();
    if p.len() != 3 {
        return String::new();
    }
    let (Ok(a), Ok(m), Ok(g)) = (p[0].parse::<i32>(), p[1].parse::<u32>(), p[2].parse::<u32>())
    else {
        return String::new();
    };
    if g > 1 {
        return format!("{a:04}-{m:02}-{:02}", g - 1);
    }
    let (a2, m2) = if m > 1 { (a, m - 1) } else { (a - 1, 12) };
    format!("{a2:04}-{m2:02}-{:02}", giorni_del_mese(a2, m2))
}

fn giorni_del_mese(anno: i32, mese: u32) -> u32 {
    match mese {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        // Bisestile secondo il calendario gregoriano, non «divisibile per
        // quattro»: il 1900 non lo era e il 2000 si'.
        2 if (anno % 4 == 0 && anno % 100 != 0) || anno % 400 == 0 => 29,
        2 => 28,
        _ => 30,
    }
}

/// Le righe in una forma che si legge, raggruppate per giorno.
///
/// La domanda a cui questo risponde e' «cosa hai fatto ieri», non «cosa hai
/// fatto alla riga 47».
pub fn racconta(righe: &[&Riga], oggi: &str) -> String {
    if righe.is_empty() {
        return "Nessuna azione registrata.".into();
    }
    let mut fuori = vec![format!("{} azioni, dalla piu' recente:", righe.len())];
    let mut giorno_scritto = String::new();
    for r in righe {
        let g = giorno(&r.quando, oggi);
        if g != giorno_scritto {
            fuori.push(format!("\n— {g} —"));
            giorno_scritto = g;
        }
        let ora: String = r.quando.chars().skip(11).take(5).collect();
        let mut pezzi = vec![format!("{ora}  [{}]  {}", r.tipo, r.azione)];
        if !r.dove.is_empty() {
            pezzi.push(format!("          su: {}", r.dove));
        }
        if !r.dettagli.is_empty() {
            pezzi.push(format!("          {}", r.dettagli));
        }
        if !r.esito.is_empty() {
            pezzi.push(format!("          esito: {}", r.esito));
        }
        fuori.push(pezzi.join("\n"));
    }
    fuori.join("\n")
}

#[cfg(test)]
mod prove {
    use super::*;

    fn riga(quando: &str, tipo: &str, azione: &str, dove: &str, dettagli: &str) -> Riga {
        Riga {
            quando: quando.into(), tipo: tipo.into(), azione: azione.into(),
            dove: dove.into(), dettagli: dettagli.into(), esito: "ok".into(),
        }
    }

    fn archivio() -> Vec<Riga> {
        vec![
            riga("2026-08-30T09:10:00", "posta", "mandata una mail",
                 "mario@esempio.it", "riepilogo riunione"),
            riga("2026-08-29T18:00:00", "documento", "modificato un documento",
                 "relazione.docx", "3 modifiche"),
            riga("2026-08-06T11:30:00", "browser", "inviata candidatura",
                 "https://lavoro.it/44", "Società Rossi S.p.A."),
        ]
    }

    #[test]
    fn si_cerca_senza_accenti_e_senza_maiuscole() {
        let a = archivio();
        let f = |t: &str| Filtro { testo: t.into(), ..Default::default() };
        assert_eq!(cerca(&a, &f("societa")).len(), 1);
        assert_eq!(cerca(&a, &f("ROSSI")).len(), 1);
        // Tutte le parole, in qualunque ordine.
        assert_eq!(cerca(&a, &f("rossi candidatura")).len(), 1);
        assert_eq!(cerca(&a, &f("candidatura rossi")).len(), 1);
        assert_eq!(cerca(&a, &f("candidatura verdi")).len(), 0);
        // Anche dentro l'indirizzo.
        assert_eq!(cerca(&a, &f("lavoro.it")).len(), 1);
    }

    #[test]
    fn si_restringe_per_tipo_e_per_data() {
        let a = archivio();
        assert_eq!(cerca(&a, &Filtro { tipo: "posta".into(), ..Default::default() }).len(), 1);
        assert_eq!(
            cerca(&a, &Filtro { non_prima_di: "2026-08-29".into(), ..Default::default() }).len(),
            2);
    }

    #[test]
    fn i_giorni_hanno_un_nome() {
        assert_eq!(giorno("2026-08-30T09:10:00", "2026-08-30"), "oggi");
        assert_eq!(giorno("2026-08-29T18:00:00", "2026-08-30"), "ieri");
        assert_eq!(giorno("2026-08-06T11:30:00", "2026-08-30"), "06/08/2026");
        // I bordi: primo del mese, primo dell'anno, e il bisestile.
        assert_eq!(giorno("2026-07-31T00:00:00", "2026-08-01"), "ieri");
        assert_eq!(giorno("2025-12-31T00:00:00", "2026-01-01"), "ieri");
        assert_eq!(giorno("2024-02-29T00:00:00", "2024-03-01"), "ieri");
        assert_eq!(giorno("1900-02-28T00:00:00", "1900-03-01"), "ieri");
    }

    #[test]
    fn il_riassunto_conta_e_ordina() {
        let r = riassunto(&archivio());
        assert_eq!(r.quante, 3);
        assert_eq!(r.prima, "2026-08-06");
        assert_eq!(r.ultima, "2026-08-30");
        // A pari merito, ordine alfabetico: se no due esecuzioni danno due
        // elenchi diversi.
        assert_eq!(r.tipi[0].0, "browser");
    }
}
