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

use nova_calendario::giorni_del_mese;
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

// `giorni_del_mese` stava qui, privata. Adesso sta in `nova-calendario`,
// perche' serviva anche a `nova-pianificazione` e una seconda copia scritta a
// mano si disallinea sempre — questo progetto l'ha gia' imparato con l'elenco
// dei binari, le cartelle sincronizzate e i posti dei dati. Alla seconda
// occorrenza la si mette in comune, non alla quarta.

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

// ----------------------------------------------------- scrivere una riga

/// Quanto si tiene di ciascun campo. Gli stessi tagli della parte Python:
/// una riga che si allarga a piacere trasforma un registro in un deposito.
pub const MAX_AZIONE: usize = 200;
pub const MAX_DOVE: usize = 300;
pub const MAX_DETTAGLI: usize = 300;
pub const MAX_ESITO: usize = 200;

/// Cosa resta al posto dei dettagli quando il campo annuncia una credenziale.
pub const NON_REGISTRATO: &str = "[non registrato: il campo contiene una credenziale]";

/// Il tipo con cui si annota di default: la sorgente automatica sono gli
/// strumenti che cambiano il mondo attraverso il browser.
pub const TIPO_PREDEFINITO: &str = "browser";

/// Una riga da scrivere, prima che qualcuno la mascheri e la tagli.
#[derive(Debug, Clone, Default)]
pub struct Scritta<'a> {
    pub azione: &'a str,
    pub dove: &'a str,
    pub dettagli: &'a str,
    /// Vuoto vuol dire [`TIPO_PREDEFINITO`].
    pub tipo: &'a str,
    /// Vuoto vuol dire che la chiave `esito` non compare affatto.
    pub esito: &'a str,
}

/// I primi `quanti` **caratteri**, non byte.
///
/// Python taglia le stringhe per caratteri. Tagliare per byte qui vorrebbe
/// dire spezzare una lettera accentata a meta' e scrivere nel registro una
/// riga che non e' piu' testo valido — su un file che si apre il giorno in
/// cui si vuole sapere cosa e' successo.
fn primi(t: &str, quanti: usize) -> String {
    t.chars().take(quanti).collect()
}

/// I dettagli mascherati, o sostituiti del tutto se il campo li annuncia.
///
/// Il caso che il filtro per forme non prende: NOVA compila un modulo di
/// accesso e scrive «scritto in #password» nell'azione e «Tramonto2026!» nei
/// dettagli. Uno per volta non sono niente — il secondo e' una parola con
/// dentro un anno — e insieme sono una credenziale. Saper compilare un modulo
/// e' una cosa che NOVA deve fare; il prezzo e' che il registro di quelle
/// azioni non conserva cio' che ha scritto.
pub fn dettagli_sicuri(azione: &str, dove: &str, dettagli: &str) -> String {
    if nova_guasti::guardiano::etichetta_di_segreto(azione)
        || nova_guasti::guardiano::etichetta_di_segreto(dove)
    {
        return NON_REGISTRATO.to_string();
    }
    primi(&nova_guasti::chiavi::senza_chiavi(dettagli), MAX_DETTAGLI)
}

/// La riga JSON da mettere in coda al registro, `\n` escluso.
///
/// **Tutto quello che passa di qui passa dal filtro**, senza eccezioni: nel
/// registro finisce anche il testo che NOVA *scrive* nei campi, e ci
/// finiscono le righe di comando, che portano volentieri un
/// `Authorization: Bearer`. Si maschera qui e non nei posti che annotano, per
/// la stessa ragione per cui il vault si chiude su `upsert`: la porta e' una
/// sola, e un chiamante che si dimentica non e' un'ipotesi, e' una certezza.
///
/// `quando` si passa da fuori — e' l'unica cosa qui dentro che dipende
/// dall'orologio, e passandola si puo' provare quel che esce.
pub fn riga_da_scrivere(s: &Scritta, quando: &str) -> String {
    let tipo = if s.tipo.is_empty() {
        TIPO_PREDEFINITO
    } else {
        s.tipo
    };
    let mut riga = serde_json::Map::new();
    riga.insert("quando".into(), quando.into());
    riga.insert("tipo".into(), tipo.into());
    riga.insert(
        "azione".into(),
        primi(&nova_guasti::chiavi::senza_chiavi(s.azione), MAX_AZIONE).into(),
    );
    riga.insert(
        "dove".into(),
        primi(&nova_guasti::chiavi::senza_chiavi(s.dove), MAX_DOVE).into(),
    );
    riga.insert(
        "dettagli".into(),
        dettagli_sicuri(s.azione, s.dove, s.dettagli).into(),
    );
    if !s.esito.is_empty() {
        riga.insert(
            "esito".into(),
            primi(&nova_guasti::chiavi::senza_chiavi(s.esito), MAX_ESITO).into(),
        );
    }
    // `json.dumps(..., ensure_ascii=False)`, separatori compresi: la riga
    // **e'** il testo che resta sul disco, e il file lo scrivono tutte e due
    // le meta'. Due spazi di differenza non cambiano cosa vuol dire e
    // cambiano cosa c'e' scritto.
    nova_pitone::json_come_python(&serde_json::Value::Object(riga))
}

/// Una riga letta dal file, riportata alla forma su cui si cerca.
///
/// Una riga che non si legge non ferma la lettura delle altre: un registro
/// con dentro una riga storta e' comunque il registro, e chi lo apre lo apre
/// per sapere cosa e' successo, non per sapere che c'e' una riga storta.
pub fn riga_letta(testo: &str) -> Option<Riga> {
    let v: serde_json::Value = serde_json::from_str(testo).ok()?;
    let campo = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    Some(Riga {
        quando: campo("quando"),
        tipo: campo("tipo"),
        azione: campo("azione"),
        dove: campo("dove"),
        dettagli: campo("dettagli"),
        esito: campo("esito"),
    })
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
