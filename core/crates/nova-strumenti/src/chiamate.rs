//! Cosa arriva dal modello, e cosa gli torna indietro.
//!
//! Due cose che si somigliano poco e stanno insieme per una ragione sola:
//! sono i due punti in cui il testo del modello e il mondo si toccano.
//!
//! **Da lui verso il PC**: certi modelli le chiamate agli strumenti le
//! scrivono nel testo invece che nel canale apposito, dentro `<tool_call>`.
//! Leggerle e' un ripiego, e va detto cosa significa: e' il punto in cui
//! della **prosa diventa un'azione**. Riconoscerne un sottoinsieme vuol dire
//! eseguire una cosa diversa da quella chiesta — o non eseguire niente
//! quando invece era stato chiesto.
//!
//! **Dal PC verso di lui**: un risultato troppo lungo non entra nel discorso.
//! Prima si tagliava a ventiquattromila caratteri con la scritta «risultato
//! troncato», e il resto spariva: il modello non sapeva *cosa* aveva perso,
//! solo che mancava qualcosa. Adesso il testo intero va su file e al suo
//! posto restano testa, coda e il percorso per andarselo a leggere. Non e'
//! spazio risparmiato: e' una perdita silenziosa diventata un rinvio.
//!
//! Qui non si scrive su disco e non si legge l'orologio: il percorso e la
//! data arrivano da fuori, come tutte le cose che dipendono dal mondo.

use serde_json::Value;

/// Quanto di un risultato entra nel discorso.
pub const LIMITE_RISULTATO: usize = 24_000;

/// Gli strumenti che **non** si versano su file: un `read_file` che finisce
/// su file e dice «rileggilo con read_file» e' un cerchio.
pub const NON_SI_VERSANO: [&str; 3] = ["read_file", "kb_search", "kb_neighbors"];

pub fn si_versa(nome: &str) -> bool {
    !NON_SI_VERSANO.contains(&nome)
}

/// Una chiamata trovata dentro il testo.
#[derive(Clone, Debug, PartialEq)]
pub struct ChiamataInline {
    pub id: String,
    pub tipo: String,
    /// Il nome come l'ha scritto il modello. Non e' detto che sia una
    /// stringa: si tiene il valore vero, perche' e' quello che il Python
    /// passa avanti.
    pub nome: Value,
    /// Gli argomenti gia' resi in stringa, come li vuole il canale dei tool.
    pub argomenti: String,
}

/// Vero se il valore e' «falso» secondo Python: `null`, `false`, `0`, `""`,
/// `[]`, `{}`.
///
/// Serve perche' il Python sceglie con `or`, e la differenza si vede: un
/// `"arguments": ""` non e' una stringa vuota da passare avanti, e' un campo
/// che **non conta**, e si guarda `parameters`. Con un semplice «c'e'/non
/// c'e'» si passerebbe una stringa vuota dove il Python passa `{}`.
fn falso_per_python(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::Bool(b) => !*b,
        Value::Number(n) => n.as_f64().map(|x| x == 0.0).unwrap_or(false),
        Value::String(s) => s.is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::Object(o) => o.is_empty(),
    }
}

fn primo_vero<'a>(o: &'a Value, chiavi: &[&str]) -> Option<&'a Value> {
    for k in chiavi {
        if let Some(v) = o.get(k) {
            if !falso_per_python(v) {
                return Some(v);
            }
        }
    }
    None
}

/// Rende un valore JSON **come lo renderebbe Python**.
///
/// `serde_json::to_string` scrive `{"a":1}`, `json.dumps` scrive `{"a": 1}`:
/// separatori diversi. Non e' cosmesi — questa stringa e' cio' che arriva
/// allo strumento come argomenti, e il banco la confronta carattere per
/// carattere. L'ordine delle chiavi si conserva perche' `serde_json` in
/// questo progetto e' compilato con `preserve_order` (lo stesso motivo per
/// cui la scala del routing non si riordina da sola).
pub fn come_python(v: &Value) -> String {
    let mut fuori = String::new();
    scrivi_come_python(v, &mut fuori);
    fuori
}

fn scrivi_come_python(v: &Value, dentro: &mut String) {
    match v {
        Value::Object(o) => {
            dentro.push('{');
            for (i, (k, val)) in o.iter().enumerate() {
                if i > 0 {
                    dentro.push_str(", ");
                }
                dentro.push_str(&Value::String(k.clone()).to_string());
                dentro.push_str(": ");
                scrivi_come_python(val, dentro);
            }
            dentro.push('}');
        }
        Value::Array(a) => {
            dentro.push('[');
            for (i, val) in a.iter().enumerate() {
                if i > 0 {
                    dentro.push_str(", ");
                }
                scrivi_come_python(val, dentro);
            }
            dentro.push(']');
        }
        altro => dentro.push_str(&altro.to_string()),
    }
}

/// Le chiamate scritte dentro il testo.
///
/// Il Python cerca `<tool_call>\s*(\{.*?\})\s*</tool_call>` con il punto che
/// prende anche gli a capo. Qui la stessa cosa a mano, e vale la pena dire
/// **perche' a mano** invece che con la stessa espressione: quella scrittura
/// dice una cosa precisa che si perde a leggerla in fretta — la graffa deve
/// essere la prima cosa non bianca dopo l'apertura, e la graffa di chiusura
/// l'ultima cosa non bianca prima della chiusura. Un `<tool_call>` seguito da
/// una parola e poi da un oggetto **non e' una chiamata**, e scriverlo a mano
/// costringe a dirlo.
///
/// Un blocco che non e' JSON valido si salta in silenzio, come in Python: e'
/// prosa che somigliava a una chiamata, non una chiamata rotta.
pub fn inline(testo: &str) -> Vec<ChiamataInline> {
    const APRE: &str = "<tool_call>";
    const CHIUDE: &str = "</tool_call>";
    let caratteri: Vec<char> = testo.chars().collect();
    let apre: Vec<char> = APRE.chars().collect();
    let chiude: Vec<char> = CHIUDE.chars().collect();

    let mut fuori: Vec<ChiamataInline> = Vec::new();
    let mut i = 0usize;
    while let Some(inizio) = trova(&caratteri, &apre, i) {
        let dopo_tag = inizio + apre.len();
        let mut j = dopo_tag;
        while j < caratteri.len() && caratteri[j].is_whitespace() {
            j += 1;
        }
        if j >= caratteri.len() || caratteri[j] != '{' {
            // La graffa non e' la prima cosa: qui non comincia una chiamata.
            i = dopo_tag;
            continue;
        }
        // La chiusura buona e' la prima preceduta (a meno di spazi) da una
        // graffa: e' quello che fa il `.*?` con l'ancora dopo.
        let mut cerca_da = j;
        let mut trovata: Option<(usize, usize)> = None;
        while let Some(fine_tag) = trova(&caratteri, &chiude, cerca_da) {
            let mut k = fine_tag;
            while k > j && caratteri[k - 1].is_whitespace() {
                k -= 1;
            }
            if k > j && caratteri[k - 1] == '}' {
                trovata = Some((k, fine_tag + chiude.len()));
                break;
            }
            cerca_da = fine_tag + chiude.len();
        }
        let Some((fine_json, ripresa)) = trovata else {
            break; // nessuna chiusura utile fino in fondo
        };
        let grezzo: String = caratteri[j..fine_json].iter().collect();
        i = ripresa;
        let Ok(obj) = serde_json::from_str::<Value>(&grezzo) else {
            continue; // non era JSON: si salta, come in Python
        };
        let Some(nome) = primo_vero(&obj, &["name", "tool"]) else {
            continue; // senza nome non c'e' niente da chiamare
        };
        let argomenti = match primo_vero(&obj, &["arguments", "parameters"]) {
            Some(Value::String(s)) => s.clone(),
            Some(altro) => come_python(altro),
            None => "{}".to_string(),
        };
        fuori.push(ChiamataInline {
            id: format!("inline_{}", fuori.len()),
            tipo: "function".to_string(),
            nome: nome.clone(),
            argomenti,
        });
    }
    fuori
}

fn trova(dove: &[char], cosa: &[char], da: usize) -> Option<usize> {
    if cosa.is_empty() || dove.len() < cosa.len() {
        return None;
    }
    (da..=dove.len() - cosa.len()).find(|&i| &dove[i..i + cosa.len()] == cosa)
}

// ------------------------------------------------------- e il ritorno

/// Il nome del file dove finisce un risultato troppo lungo.
///
/// `quando` arriva da fuori — un `20260905-152233` — perche' un nome di file
/// che dipende dall'orologio non si puo' provare due volte allo stesso modo.
pub fn nome_del_file(quando: &str, nome: &str, call_id: &str) -> String {
    let grezzo = format!("{nome}-{call_id}");
    let sicuro: String = grezzo
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-' { c } else { '_' })
        .take(60)
        .collect();
    format!("{quando}-{sicuro}.txt")
}

/// L'avviso che prende il posto del pezzo omesso.
pub fn avviso(omessi: usize, percorso: &str) -> String {
    format!(
        "\n\n[Omessi {omessi} caratteri nel mezzo. Il risultato completo e' in \
         {percorso}. Leggilo con read_file, che accetta un intervallo di righe, \
         oppure cercaci dentro con search_in_files.]\n\n"
    )
}

/// Testa, avviso, coda.
///
/// Il costo in caratteri dell'avviso si riserva **fuori** dal budget: cosi'
/// la sostituzione non puo' risultare piu' lunga di cio' che sostituisce, che
/// sarebbe il modo piu' sciocco di fallire.
pub fn versa(testo: &str, percorso: &str, limite: usize) -> String {
    let caratteri: Vec<char> = testo.chars().collect();
    let intero = caratteri.len();
    let costo = avviso(intero, percorso).chars().count();
    let spazio = limite as i64 - costo as i64;
    if spazio <= 200 {
        return avviso(intero, percorso).trim().to_string();
    }
    let spazio = spazio as usize;
    let testa = spazio * 2 / 3;
    let coda = spazio - testa;
    if intero <= testa + coda {
        return testo.to_string();
    }
    let davanti: String = caratteri[..testa].iter().collect();
    let dietro: String = caratteri[intero - coda..].iter().collect();
    format!("{davanti}{}{dietro}", avviso(intero - testa - coda, percorso))
}

/// Il taglio dichiarato, per gli strumenti che non si versano.
pub fn troncato(testo: &str, limite: usize) -> String {
    let caratteri: Vec<char> = testo.chars().collect();
    if caratteri.len() <= limite {
        return testo.to_string();
    }
    let davanti: String = caratteri[..limite].iter().collect();
    format!("{davanti}\n... [risultato troncato]")
}

/// Il taglio quando il file non si e' potuto scrivere: meglio una perdita
/// detta di una promessa non mantenuta.
pub fn troncato_perche(testo: &str, limite: usize, errore: &str) -> String {
    let caratteri: Vec<char> = testo.chars().collect();
    let quanti = std::cmp::min(limite, caratteri.len());
    let davanti: String = caratteri[..quanti].iter().collect();
    format!("{davanti}\n... [risultato troncato: non sono riuscito a salvarlo ({errore})]")
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn una_chiamata_semplice() {
        let c = inline("blabla <tool_call>{\"name\": \"read_file\", \"arguments\": {\"path\": \"a.txt\"}}</tool_call> fine");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].id, "inline_0");
        assert_eq!(c[0].nome, Value::String("read_file".into()));
        assert_eq!(c[0].argomenti, "{\"path\": \"a.txt\"}");
    }

    #[test]
    fn i_separatori_sono_quelli_di_python() {
        let c = inline("<tool_call>{\"name\":\"x\",\"arguments\":{\"a\":1,\"b\":[1,2]}}</tool_call>");
        assert_eq!(c[0].argomenti, "{\"a\": 1, \"b\": [1, 2]}");
    }

    #[test]
    fn lordine_delle_chiavi_non_si_riordina() {
        let c = inline("<tool_call>{\"name\":\"x\",\"arguments\":{\"z\":1,\"a\":2}}</tool_call>");
        assert_eq!(c[0].argomenti, "{\"z\": 1, \"a\": 2}");
    }

    #[test]
    fn gli_argomenti_vuoti_valgono_come_assenti() {
        // `or` in Python: "" e {} sono falsi e si passa oltre.
        let c = inline("<tool_call>{\"name\":\"x\",\"arguments\":\"\",\"parameters\":{\"a\":1}}</tool_call>");
        assert_eq!(c[0].argomenti, "{\"a\": 1}");
        let c2 = inline("<tool_call>{\"name\":\"x\",\"arguments\":{}}</tool_call>");
        assert_eq!(c2[0].argomenti, "{}");
    }

    #[test]
    fn una_stringa_di_argomenti_si_passa_com_e() {
        let c = inline("<tool_call>{\"name\":\"x\",\"arguments\":\"{gia' scritto}\"}</tool_call>");
        assert_eq!(c[0].argomenti, "{gia' scritto}");
    }

    #[test]
    fn tool_al_posto_di_name() {
        let c = inline("<tool_call>{\"tool\":\"y\"}</tool_call>");
        assert_eq!(c[0].nome, Value::String("y".into()));
        assert_eq!(c[0].argomenti, "{}");
    }

    #[test]
    fn senza_nome_non_e_una_chiamata() {
        assert!(inline("<tool_call>{\"arguments\":{\"a\":1}}</tool_call>").is_empty());
        assert!(inline("<tool_call>{\"name\":\"\"}</tool_call>").is_empty());
    }

    #[test]
    fn la_graffa_deve_venire_subito() {
        // Il Python vuole `\s*\{`: una parola in mezzo e non e' una chiamata.
        assert!(inline("<tool_call> ecco: {\"name\":\"x\"}</tool_call>").is_empty());
        // Con soli spazi e a capo invece si'.
        assert_eq!(inline("<tool_call>\n  {\"name\":\"x\"}\n</tool_call>").len(), 1);
    }

    #[test]
    fn json_rotto_si_salta_e_si_va_avanti() {
        let t = "<tool_call>{rotto}</tool_call><tool_call>{\"name\":\"x\"}</tool_call>";
        let c = inline(t);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].id, "inline_0", "l'indice conta le chiamate valide");
    }

    #[test]
    fn piu_chiamate_di_fila() {
        let t = "<tool_call>{\"name\":\"a\"}</tool_call> e poi <tool_call>{\"name\":\"b\"}</tool_call>";
        let c = inline(t);
        assert_eq!(c.len(), 2);
        assert_eq!(c[1].id, "inline_1");
    }

    #[test]
    fn gli_accenti_non_si_toccano() {
        let c = inline("<tool_call>{\"name\":\"x\",\"arguments\":{\"q\":\"perché città\"}}</tool_call>");
        assert_eq!(c[0].argomenti, "{\"q\": \"perché città\"}");
    }

    #[test]
    fn il_nome_del_file_e_sempre_scrivibile() {
        let n = nome_del_file("20260905-152233", "run_powershell", "call/1:2\\3");
        assert_eq!(n, "20260905-152233-run_powershell-call_1_2_3.txt");
        assert!(!n.contains('/') && !n.contains('\\') && !n.contains(':'));
    }

    #[test]
    fn il_nome_del_file_si_ferma_a_sessanta() {
        let n = nome_del_file("q", &"a".repeat(200), "b");
        // 60 caratteri di nome sicuro, piu' "q-" e ".txt"
        assert_eq!(n.chars().count(), 60 + 2 + 4);
    }

    #[test]
    fn versare_non_allunga_il_testo() {
        // Il margine di quaranta caratteri che c'era qui non serviva: il
        // costo dell'avviso si riserva sul numero piu' grande che l'avviso
        // potra' contenere, quindi la sostituzione **non puo'** superare il
        // limite. Lasciare uno spiraglio dove la garanzia e' netta vuol dire
        // che il giorno in cui si rompe la prova resta verde.
        let testo = "x".repeat(200_000);
        let fuori = versa(&testo, "C:\\r\\v\\a.txt", LIMITE_RISULTATO);
        assert!(fuori.chars().count() <= LIMITE_RISULTATO, "{}", fuori.chars().count());
        assert!(fuori.contains("Omessi "));
        assert!(fuori.contains("a.txt"));
    }

    #[test]
    fn e_non_lo_allunga_mai_a_nessuna_misura() {
        for limite in [300usize, 400, 1_000, 24_000] {
            for lungo in [limite + 1, limite + 50, limite * 2, limite * 10] {
                let fuori = versa(&"q".repeat(lungo), "C:\\r\\v\\a-b.txt", limite);
                assert!(fuori.chars().count() <= limite,
                        "limite {limite}, originale {lungo}: sono usciti {}",
                        fuori.chars().count());
            }
        }
    }

    #[test]
    fn della_testa_se_ne_tiene_il_doppio_della_coda() {
        // Non e' meta' e meta': di un risultato di strumento l'inizio porta
        // quasi sempre la forma - le intestazioni, lo schema, le prime righe
        // di una tabella - e la fine solo come e' andata a finire. Due terzi
        // e un terzo e' una **scelta**, e finora la difendeva soltanto il
        // banco contro il Python: cambiarla qui e li' nello stesso modo
        // sarebbe rimasto verde ovunque.
        let testo: String = (0..60_000).map(|i| char::from(b'a' + (i % 26) as u8)).collect();
        let fuori = versa(&testo, "C:\\r\\v\\a.txt", LIMITE_RISULTATO);
        let (testa, resto) = fuori.split_once("\n\n[Omessi ").unwrap();
        let coda = resto.split_once("]\n\n").unwrap().1;
        let rapporto = testa.chars().count() as f64 / coda.chars().count() as f64;
        assert!((1.9..2.1).contains(&rapporto),
                "testa {} coda {}: rapporto {rapporto:.2}, doveva essere 2",
                testa.chars().count(), coda.chars().count());
    }

    #[test]
    fn un_percorso_lunghissimo_lascia_solo_lavviso() {
        let testo = "x".repeat(200_000);
        let percorso = "C:\\".to_string() + &"cartellona\\".repeat(3_000) + "a.txt";
        let fuori = versa(&testo, &percorso, LIMITE_RISULTATO);
        assert!(fuori.starts_with("[Omessi "));
        assert!(!fuori.starts_with('\n'), "lo strip toglie gli a capo");
    }

    #[test]
    fn il_taglio_non_spezza_una_lettera_accentata() {
        let testo = "perché ".repeat(10_000);
        let fuori = versa(&testo, "a.txt", LIMITE_RISULTATO);
        assert!(fuori.contains("Omessi "));
        let t2 = troncato(&testo, 1_001);
        assert!(t2.ends_with("[risultato troncato]"));
    }
}
