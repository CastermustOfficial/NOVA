//! Come si legge la configurazione di NOVA.
//!
//! Il file salvato si applica **sopra** i valori di fabbrica, e le regole di
//! quel «sopra» sono cinque. Nessuna e' una preferenza di stile: ognuna e'
//! scritta per una cosa che e' andata storta.
//!
//! **Le sezioni non si elencano a mano.** C'era un elenco con dentro i sette
//! nomi, piu' una riga a parte per il prompt di sistema, e `fascicolo` non
//! era ne' nell'uno ne' nell'altra: veniva **scritto** nel file e non letto
//! mai. Chi spostava il fascicolo vedeva NOVA continuare a usare quello
//! vecchio, senza un errore da nessuna parte (D229). Qui i campi sono quelli
//! che ha il valore di fabbrica: aggiungerne uno non richiede di ricordarsi
//! di questo posto.
//!
//! **Le chiavi che la fabbrica non conosce non entrano.** Un file salvato non
//! deve poter aggiungere campi a NOVA.
//!
//! **Un dizionario si fonde al primo livello.** Il salvato vince chiave per
//! chiave, ma quelle che non conosce — perche' aggiunte dopo — tornano di
//! fabbrica: altrimenti ogni configurazione vecchia perde le novita'. Solo al
//! primo livello, pero': dentro `tiers` comanda l'utente.
//!
//! **Le guardie si uniscono invece di essere sostituite.** Ovunque altro il
//! salvato vince, ed e' giusto: e' roba dell'utente. Li' no, e non perche'
//! NOVA sappia meglio — e' che quella e' l'unica lista che **cresce**, e una
//! lista che cresce piu' un file che vince danno un elenco congelato al
//! giorno in cui e' stato salvato. Ed e' gia' successo col prompt di sistema:
//! una configurazione di mesi prima ne conteneva la meta', e fra le righe
//! mancanti c'era quella che diceva a NOVA che poteva guardare la posta.
//!
//! **La diagnostica non arriva da fuori.** Un file salvato non deve poter
//! raccontare a NOVA di aver avuto un errore che non ha avuto.

pub mod dove;
pub use dove::{leggi_da, percorso};

use serde_json::{Map, Value};

/// I campi che non stanno nel file e non si leggono da li'.
pub const NON_SI_CARICANO: [&str; 3] =
    ["errore_caricamento", "guardie_aggiunte", "sezioni_ignorate"];

/// Le due liste in cui il salvato **non** vince da solo: si unisce.
pub const GUARDIE_CHE_SI_UNISCONO: [&str; 2] = ["forbidden_command_patterns", "protected_paths"];

/// La sezione in cui stanno le guardie.
pub const SEZIONE_GUARDIE: &str = "safety";

/// L'unico campo in cui il vuoto **non** vince: un prompt di sistema svuotato
/// da un salvataggio andato male lascerebbe NOVA senza istruzioni, e senza
/// niente da cui accorgersene.
pub const NON_SI_SVUOTA: &str = "system_prompt";

/// Dove stanno, in una configurazione, le liste che si uniscono.
///
/// Le **regole** di lettura sono le stesse per chiunque; i nomi dei campi no.
/// NOVA tiene le sue guardie dentro `safety`; il demone le tiene in cima e le
/// chiama in un altro modo. Scrivere due volte la stessa regola per due
/// schemi diversi sarebbe il modo di farle divergere — ed e' esattamente il
/// difetto che questo crate e' nato per chiudere (D185, D229).
#[derive(Debug, Clone, Copy)]
pub struct Guardie<'a> {
    /// La sezione che le contiene, oppure `None` se stanno al primo livello.
    pub sezione: Option<&'a str>,
    pub campi: &'a [&'a str],
}

impl Guardie<'static> {
    /// Quelle di NOVA, dentro `safety`.
    pub const DI_NOVA: Guardie<'static> = Guardie {
        sezione: Some(SEZIONE_GUARDIE),
        campi: &GUARDIE_CHE_SI_UNISCONO,
    };
}

/// Le regole con cui un salvato si applica sopra la fabbrica.
///
/// Sono le stesse per chiunque; cambiano i nomi dei campi e una cosa sola di
/// sostanza, `tipi_fermi`, che dipende da **chi legge**.
#[derive(Debug, Clone, Copy)]
pub struct Regole<'a> {
    pub guardie: Guardie<'a>,
    /// I campi in cui il vuoto **non** vince.
    pub non_si_svuota: &'a [&'a str],
    /// Se un valore di un tipo che la fabbrica non ha resta fuori.
    ///
    /// Per chi legge dentro una struttura tipata — il demone — dev'essere
    /// accesa: un `"shell_timeout_s": "ciao"` fa fallire la conversione di
    /// **tutta** la configurazione, e si torna ai predefiniti perdendo anche
    /// i campi scritti bene. Per NOVA in Python e' spenta, perche' li' un
    /// tipo sbagliato non fa fallire la lettura: fa fallire qualcosa dopo,
    /// lontano dalla causa. E' un difetto anche quello, ma e' un altro, e
    /// chiuderlo vuol dire cambiare tutte e due le meta' insieme — se no le
    /// due risposte divergono, che e' la cosa che questo crate esiste per
    /// impedire (D284).
    pub tipi_fermi: bool,
}

impl Regole<'static> {
    /// Quelle di NOVA in Python, che la prova gemella tiene ferme.
    pub const DI_NOVA: Regole<'static> = Regole {
        guardie: Guardie::DI_NOVA,
        non_si_svuota: &[NON_SI_SVUOTA],
        tipi_fermi: false,
    };
}

/// Che forma ha questo valore, per dire se due sono dello stesso tipo.
///
/// **Un intero e un decimale sono due forme diverse**, e non per pedanteria:
/// `serde` rifiuta un `42.0` dove va un intero, e quel rifiuto arriva dopo —
/// quando si converte **tutta** la configurazione, che e' esattamente il
/// modo di perderla che questo controllo esiste per chiudere. Meglio
/// scartare il campo e dirlo.
fn forma(v: &Value) -> u8 {
    match v {
        Value::Null => 0,
        Value::Bool(_) => 1,
        Value::Number(n) if n.is_f64() => 2,
        Value::Number(_) => 3,
        Value::String(_) => 4,
        Value::Array(_) => 5,
        Value::Object(_) => 6,
    }
}

/// Cosa NOVA ha rimesso nell'elenco delle guardie, e in che campo.
///
/// Aggiungere qualcosa alla configurazione di qualcuno **senza dirlo** e'
/// l'altro modo di sbagliare.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aggiunta {
    pub campo: String,
    pub voci: Vec<String>,
}

/// Cosa NOVA ha fatto al file di qualcun altro, per poterglielo dire.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rapporto {
    /// Le guardie di fabbrica rimesse nell'elenco.
    pub aggiunte: Vec<Aggiunta>,
    /// Le sezioni che nel file non erano oggetti, e sono rimaste di
    /// fabbrica. Nella parte Python questa riga non esisteva perche' non ci
    /// si arrivava: `"safety": "ciao"` faceva morire la lettura con un
    /// AttributeError, fuori da ogni riparo, e NOVA non partiva affatto.
    pub ignorate: Vec<String>,
}

/// Cos'e' uscito dalla lettura.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lettura {
    pub config: Value,
    pub rapporto: Rapporto,
    /// Vuoto se e' andata. Altrimenti perche' il file e' stato ignorato —
    /// e si riparte dai valori di fabbrica **dicendolo**: tornare ai
    /// predefiniti in silenzio vuol dire perdere cervello, gradini,
    /// autonomia e vault per un file salvato con la codifica sbagliata.
    pub errore: String,
}

/// Il salvato sopra i valori di fabbrica, con le regole di NOVA.
pub fn applica(predefinito: &Value, salvato: &Value) -> (Value, Rapporto) {
    applica_con(predefinito, salvato, &Regole::DI_NOVA)
}

/// Il salvato sopra i valori di fabbrica, dicendo dove guardare.
///
/// `non_si_svuota` sono i campi in cui il vuoto **non** vince. Per NOVA e' il
/// prompt di sistema; per un altro schema saranno altri — ma la ragione e'
/// sempre quella: un campo svuotato da un salvataggio andato male non e' una
/// scelta di nessuno, e lasciarlo vincere toglie qualcosa in silenzio.
pub fn applica_con(predefinito: &Value, salvato: &Value, regole: &Regole) -> (Value, Rapporto) {
    let mut fuori = predefinito.clone();
    let mut ignorate: Vec<String> = Vec::new();
    let (Some(dentro), Some(sopra)) = (fuori.as_object_mut(), salvato.as_object()) else {
        return (fuori, Rapporto::default());
    };
    // I campi sono quelli che ha la fabbrica. Un elenco scritto a mano e'
    // quello che ha fatto sparire `fascicolo`.
    let campi: Vec<String> = dentro.keys().cloned().collect();
    for campo in campi {
        if NON_SI_CARICANO.contains(&campo.as_str()) {
            continue;
        }
        let Some(valore) = sopra.get(&campo) else {
            continue;
        };
        let attuale = dentro.get(&campo).cloned().unwrap_or(Value::Null);
        if attuale.is_object() {
            if !valore.is_object() {
                // Una sezione che nel file non e' un oggetto resta di
                // fabbrica, e si annota: sostituirla con una stringa
                // toglierebbe a NOVA tutte le guardie in un colpo.
                if !valore.is_null() {
                    ignorate.push(campo.clone());
                }
                continue;
            }
            dentro.insert(campo, sezione(&attuale, valore));
        } else if regole.tipi_fermi && valore.is_null() && !attuale.is_null() {
            // Un nulla e' «non detto», non «detto male»: resta quel che c'e'
            // di fabbrica, e non si spaventa nessuno. Infilarlo davvero
            // dentro un campo tipato farebbe fallire la conversione di tutta
            // la configurazione — cioe' proprio quel che c'e' qui sotto.
            continue;
        } else if regole.tipi_fermi && !attuale.is_null() && forma(&attuale) != forma(valore) {
            // Un tipo che la fabbrica non ha non entra: e' lo stesso
            // difetto della sezione che non e' un oggetto, un piano piu'
            // giu'. Chi legge dentro una struttura tipata, senza questo,
            // perde **tutta** la configurazione per un campo solo.
            ignorate.push(campo.clone());
        } else if regole.non_si_svuota.contains(&campo.as_str()) {
            if !vuoto(valore) {
                dentro.insert(campo, valore.clone());
            }
        } else {
            dentro.insert(campo, valore.clone());
        }
    }
    let aggiunte = guardie_non_si_perdono(&mut fuori, predefinito, &regole.guardie);
    (fuori, Rapporto { aggiunte, ignorate })
}

/// Una sezione: il salvato vince chiave per chiave, ma solo per le chiavi che
/// la fabbrica conosce.
fn sezione(predefinito: &Value, salvato: &Value) -> Value {
    let mut fuori = predefinito.clone();
    let (Some(dentro), Some(sopra)) = (fuori.as_object_mut(), salvato.as_object()) else {
        return fuori;
    };
    for (k, v) in sopra {
        let Some(di_fabbrica) = dentro.get(k) else {
            continue; // una chiave che la sezione non conosce non ci entra
        };
        if di_fabbrica.is_object() && v.is_object() {
            // Un livello solo: dentro `tiers` comanda l'utente.
            let mut unito: Map<String, Value> =
                di_fabbrica.as_object().cloned().unwrap_or_default();
            for (k2, v2) in v.as_object().unwrap() {
                unito.insert(k2.clone(), v2.clone());
            }
            dentro.insert(k.clone(), Value::Object(unito));
        } else {
            dentro.insert(k.clone(), v.clone());
        }
    }
    Value::Object(dentro.clone())
}

/// Se un valore conta come «non detto».
fn vuoto(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::Object(o) => o.is_empty(),
        Value::Bool(b) => !b,
        Value::Number(n) => n.as_f64() == Some(0.0),
    }
}

/// I predefiniti delle guardie si **aggiungono**, non si lasciano sostituire.
fn guardie_non_si_perdono(
    config: &mut Value,
    fabbrica: &Value,
    guardie: &Guardie,
) -> Vec<Aggiunta> {
    let mut aggiunte = Vec::new();
    for campo in guardie.campi {
        let di_fabbrica: Vec<String> = match guardie.sezione {
            Some(s) => fabbrica.get(s).and_then(|x| x.get(campo)),
            None => fabbrica.get(campo),
        }
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
        let Some(suoi_val) = (match guardie.sezione {
            Some(s) => config.get_mut(s).and_then(|x| x.get_mut(campo)),
            None => config.get_mut(campo),
        }) else {
            continue;
        };
        let suoi: Vec<String> = suoi_val
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let mancanti: Vec<String> = di_fabbrica
            .into_iter()
            .filter(|x| !suoi.contains(x))
            .collect();
        if mancanti.is_empty() {
            continue;
        }
        let mut tutti = suoi;
        tutti.extend(mancanti.iter().cloned());
        *suoi_val = Value::Array(tutti.into_iter().map(Value::String).collect());
        aggiunte.push(Aggiunta {
            campo: campo.to_string(),
            voci: mancanti,
        });
    }
    aggiunte
}

/// Toglie le CLI messe a nulla.
///
/// La configurazione si fonde, non si sostituisce: dal pannello una chiave
/// non si puo' cancellare, si puo' solo mettere a nulla. Se quel nulla
/// restasse, il file si riempirebbe di lapidi e chi lo apre non capirebbe se
/// «gemini: null» vuol dire tolto o rotto. Alla prima lettura sparisce.
pub fn pulisci_cli(config: &mut Value) {
    let Some(voci) = config
        .get_mut("brains")
        .and_then(|b| b.get_mut("cli"))
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    let morte: Vec<String> = voci
        .iter()
        .filter(|(_, v)| vuoto(v))
        .map(|(k, _)| k.clone())
        .collect();
    for k in morte {
        voci.remove(&k);
    }
}

/// La configurazione, letta da quel che c'e' sul file, con le regole di NOVA.
pub fn leggi(predefinito: &Value, testo: &str) -> Lettura {
    leggi_con(predefinito, testo, &Regole::DI_NOVA)
}

/// La configurazione, letta con le regole che le si danno.
///
/// Il BOM si toglie: il Blocco note e PowerShell lo scrivono in testa, e un
/// parser che ci muore sopra fa perdere **tutta** la configurazione per una
/// codifica.
pub fn leggi_con(predefinito: &Value, testo: &str, regole: &Regole) -> Lettura {
    let pulito = testo.trim_start_matches('\u{feff}');
    if pulito.trim().is_empty() {
        return Lettura {
            config: predefinito.clone(),
            rapporto: Rapporto::default(),
            errore: String::new(),
        };
    }
    let salvato: Value = match serde_json::from_str(pulito) {
        Ok(v) => v,
        Err(e) => {
            return Lettura {
                config: predefinito.clone(),
                rapporto: Rapporto::default(),
                errore: format!("il file non si legge: {e}"),
            }
        }
    };
    if !salvato.is_object() {
        return Lettura {
            config: predefinito.clone(),
            rapporto: Rapporto::default(),
            errore: "il contenuto non e' un oggetto JSON".to_string(),
        };
    }
    let (mut config, rapporto) = applica_con(predefinito, &salvato, regole);
    // Su uno schema che non ha `brains` non fa niente: il costo di
    // chiamarla sempre e' zero, e una riga in meno da ricordarsi.
    pulisci_cli(&mut config);
    Lettura {
        config,
        rapporto,
        errore: String::new(),
    }
}

#[cfg(test)]
mod prove {
    use super::*;
    use serde_json::json;

    fn fabbrica() -> Value {
        json!({
            "server": { "host": "127.0.0.1", "port": 8080, "ctx_size": 16384 },
            "safety": {
                "autonomy": "ask_risky",
                "write_roots": [],
                "protected_paths": ["/etc", "/boot"],
                "forbidden_command_patterns": ["\\bdiskpart\\b", "\\bmkfs\\b"]
            },
            "brains": {
                "active": "locale",
                "cli": { "gemini": { "comando": "gemini" } },
                "routing": { "abilitato": true, "scala": ["locale", "standard"] }
            },
            "system_prompt": "sei NOVA, e sai fare un mucchio di cose",
            "fascicolo": "",
            "errore_caricamento": "",
            "guardie_aggiunte": {},
            "sezioni_ignorate": []
        })
    }

    #[test]
    fn il_fascicolo_si_legge_davvero() {
        // Il difetto vero: `save()` lo scriveva, la documentazione diceva
        // all'utente che si puo' spostare da li', e `load()` non lo leggeva.
        let (c, _) = applica(&fabbrica(), &json!({"fascicolo": "D:/i-miei-fatti"}));
        assert_eq!(c["fascicolo"], "D:/i-miei-fatti");
    }

    #[test]
    fn una_chiave_che_la_fabbrica_non_conosce_non_entra() {
        let (c, _) = applica(&fabbrica(), &json!({"roba_inventata": 1}));
        assert!(c.get("roba_inventata").is_none());
        let (c, _) = applica(&fabbrica(), &json!({"brains": {"inventata": 1}}));
        assert!(c["brains"].get("inventata").is_none());
    }

    #[test]
    fn la_diagnostica_non_arriva_da_fuori() {
        let (c, _) = applica(
            &fabbrica(),
            &json!({"errore_caricamento": "me lo sono inventato",
                    "guardie_aggiunte": {"x": ["y"]}}),
        );
        assert_eq!(c["errore_caricamento"], "");
        assert_eq!(c["guardie_aggiunte"], json!({}));
        // Anche l'elenco delle sezioni saltate: raccontare a NOVA di aver
        // ignorato una sezione che non ha ignorato e' lo stesso difetto.
        let (c, _) = applica(&fabbrica(), &json!({"sezioni_ignorate": ["safety"]}));
        assert_eq!(c["sezioni_ignorate"], json!([]));
    }

    #[test]
    fn il_salvato_vince_dentro_una_sezione() {
        let (c, _) = applica(&fabbrica(), &json!({"server": {"port": 9999}}));
        assert_eq!(c["server"]["port"], 9999);
        assert_eq!(c["server"]["host"], "127.0.0.1", "il resto resta");
    }

    #[test]
    fn un_dizionario_si_fonde_al_primo_livello() {
        // Le chiavi che il salvato non conosce tornano di fabbrica: se no
        // ogni configurazione vecchia perde le novita'.
        let (c, _) = applica(
            &fabbrica(),
            &json!({"brains": {"cli": {"mia": {"comando": "mia"}}}}),
        );
        assert!(c["brains"]["cli"]["mia"].is_object(), "la sua c'e'");
        assert!(
            c["brains"]["cli"]["gemini"].is_object(),
            "e quella di fabbrica torna"
        );
    }

    #[test]
    fn e_il_salvato_vince_sulle_chiavi_che_ha() {
        // Una fusione scritta al contrario lascerebbe le chiavi dell'utente
        // al loro posto con dentro i valori di fabbrica: il modo piu'
        // silenzioso di cancellare una scelta.
        let (c, _) = applica(
            &fabbrica(),
            &json!({"brains": {"cli": {"gemini": {"comando": "il-mio-gemini"}}}}),
        );
        assert_eq!(c["brains"]["cli"]["gemini"]["comando"], "il-mio-gemini");
    }

    #[test]
    fn ma_solo_al_primo_livello_dentro_tiers_comanda_lutente() {
        let (c, _) = applica(
            &fabbrica(),
            &json!({"brains": {"routing": {"scala": ["solo-locale"]}}}),
        );
        assert_eq!(c["brains"]["routing"]["scala"], json!(["solo-locale"]));
        assert_eq!(
            c["brains"]["routing"]["abilitato"], true,
            "il resto torna di fabbrica"
        );
    }

    #[test]
    fn le_guardie_si_uniscono_e_lo_dicono() {
        let (c, r) = applica(
            &fabbrica(),
            &json!({"safety": {"protected_paths": ["/mio"]}}),
        );
        let p = c["safety"]["protected_paths"].as_array().unwrap();
        assert!(p.iter().any(|x| x == "/mio"), "quel che c'era resta");
        assert!(p.iter().any(|x| x == "/etc"), "e i predefiniti tornano");
        assert!(p.iter().any(|x| x == "/boot"));
        assert_eq!(r.aggiunte.len(), 1);
        assert_eq!(r.aggiunte[0].campo, "protected_paths");
        assert_eq!(r.aggiunte[0].voci, vec!["/etc", "/boot"]);
    }

    #[test]
    fn quel_che_c_era_viene_prima_di_quel_che_torna() {
        // L'ordine non e' estetico: chi legge il file deve riconoscere le
        // proprie righe in cima invece di cercarle in mezzo.
        let (c, _) = applica(
            &fabbrica(),
            &json!({"safety": {"protected_paths": ["/mio"]}}),
        );
        assert_eq!(c["safety"]["protected_paths"][0], "/mio");
    }

    #[test]
    fn le_stesse_regole_valgono_per_uno_schema_diverso() {
        // Il demone tiene le sue guardie **in cima** e le chiama in un altro
        // modo. La regola e' la stessa; scriverla due volte sarebbe il modo
        // di farla divergere — che e' il difetto per cui questo crate esiste.
        let fabbrica = json!({
            "endpoint": "\\\\.\\pipe\\nova",
            "protected_paths": ["/etc", "/boot"],
            "forbidden_commands": ["diskpart", "mkfs"],
            "write_roots": [],
        });
        let regole = Regole {
            guardie: Guardie {
                sezione: None,
                campi: &["protected_paths", "forbidden_commands"],
            },
            non_si_svuota: &["endpoint"],
            tipi_fermi: true,
        };
        let (c, r) = applica_con(
            &fabbrica,
            &json!({"protected_paths": ["/mio"], "endpoint": ""}),
            &regole,
        );
        let p = c["protected_paths"].as_array().unwrap();
        assert_eq!(p[0], "/mio", "quel che c'era viene prima");
        assert!(p.iter().any(|x| x == "/etc"), "e i predefiniti tornano");
        assert_eq!(r.aggiunte.len(), 1);
        assert_eq!(r.aggiunte[0].campo, "protected_paths");
        // E un endpoint svuotato non vince: il demone resterebbe senza porta.
        assert_eq!(c["endpoint"], fabbrica["endpoint"]);
        // Mentre `write_roots` vuoto e' una scelta e vince.
        let (c, _) = applica_con(&fabbrica, &json!({"write_roots": []}), &regole);
        assert_eq!(c["write_roots"], json!([]));
        // E un campo di un tipo che la fabbrica non ha resta fuori, invece
        // di far fallire la conversione di tutta la configurazione.
        let (c, r) = applica_con(
            &fabbrica,
            &json!({"protected_paths": "non una lista", "endpoint": "\\\\.\\pipe\\mio"}),
            &regole,
        );
        assert_eq!(
            c["protected_paths"], fabbrica["protected_paths"],
            "resta di fabbrica"
        );
        assert_eq!(
            c["endpoint"], "\\\\.\\pipe\\mio",
            "e il resto del file si legge lo stesso"
        );
        assert_eq!(r.ignorate, vec!["protected_paths".to_string()]);
    }

    #[test]
    fn e_se_non_manca_niente_non_si_dice_niente() {
        let (_, r) = applica(
            &fabbrica(),
            &json!({"safety": {"protected_paths": ["/etc", "/boot", "/mio"]}}),
        );
        assert!(
            r.aggiunte.is_empty(),
            "non si annuncia un'aggiunta che non c'e' stata"
        );
    }

    #[test]
    fn un_prompt_svuotato_non_vince() {
        let (c, _) = applica(&fabbrica(), &json!({"system_prompt": ""}));
        assert!(c["system_prompt"].as_str().unwrap().len() > 10);
    }

    #[test]
    fn ma_uno_scritto_davvero_si() {
        let (c, _) = applica(&fabbrica(), &json!({"system_prompt": "sei un tostapane"}));
        assert_eq!(c["system_prompt"], "sei un tostapane");
    }

    #[test]
    fn un_falso_vince_perche_e_una_scelta() {
        // Il vuoto non vince **solo** per il prompt di sistema. Un
        // interruttore spento e' una scelta, e trattarlo come «non detto»
        // vorrebbe dire riaccenderlo a ogni avvio.
        let (c, _) = applica(
            &fabbrica(),
            &json!({"brains": {"routing": {"abilitato": false}}}),
        );
        assert_eq!(c["brains"]["routing"]["abilitato"], false);
        let (c, _) = applica(&fabbrica(), &json!({"server": {"port": 0}}));
        assert_eq!(c["server"]["port"], 0);
    }

    #[test]
    fn le_cli_messe_a_nulla_spariscono() {
        let mut c = json!({"brains": {"cli": {
            "gemini": Value::Null, "vera": {"comando": "x"}, "vuota": {}
        }}});
        pulisci_cli(&mut c);
        let cli = c["brains"]["cli"].as_object().unwrap();
        assert!(
            !cli.contains_key("gemini"),
            "una lapide non si distingue da un guasto"
        );
        assert!(!cli.contains_key("vuota"));
        assert!(cli.contains_key("vera"));
    }

    #[test]
    fn e_una_lapide_e_una_lapide_comunque_sia_scritta() {
        // Dal pannello una chiave svuotata non arriva sempre come `null`:
        // puo' arrivare come falso, zero, stringa vuota o lista vuota.
        let mut c = json!({"brains": {"cli": {
            "nulla": Value::Null, "falsa": false, "zero": 0,
            "stringa": "", "lista": [], "viva": {"comando": "x"}
        }}});
        pulisci_cli(&mut c);
        let cli = c["brains"]["cli"].as_object().unwrap();
        assert_eq!(cli.len(), 1, "resta solo quella viva: {cli:?}");
        assert!(cli.contains_key("viva"));
    }

    #[test]
    fn il_bom_non_fa_perdere_la_configurazione() {
        // Il Blocco note e PowerShell lo scrivono in testa. Un parser che ci
        // muore sopra fa perdere tutto per una codifica.
        let l = leggi(&fabbrica(), "\u{feff}{\"fascicolo\": \"D:/x\"}");
        assert_eq!(l.errore, "");
        assert_eq!(l.config["fascicolo"], "D:/x");
    }

    #[test]
    fn un_file_rotto_lo_dice_invece_di_tacere() {
        // Tornare ai predefiniti in silenzio vuol dire perdere cervello,
        // gradini, autonomia e vault per un file salvato male.
        let l = leggi(&fabbrica(), "{questo non e' json");
        assert!(!l.errore.is_empty());
        assert_eq!(
            l.config["brains"]["active"], "locale",
            "si riparte dalla fabbrica"
        );
        let l = leggi(&fabbrica(), "[1, 2, 3]");
        assert!(l.errore.contains("non e' un oggetto"), "{}", l.errore);
    }

    #[test]
    fn un_file_vuoto_non_e_un_errore() {
        let l = leggi(&fabbrica(), "   \n");
        assert_eq!(l.errore, "");
        assert_eq!(l.config, fabbrica());
    }

    #[test]
    fn leggere_toglie_anche_le_lapidi() {
        let l = leggi(&fabbrica(), r#"{"brains": {"cli": {"gemini": null}}}"#);
        assert!(l.config["brains"]["cli"].as_object().unwrap().is_empty());
    }

    #[test]
    fn una_sezione_che_non_e_un_oggetto_non_porta_via_le_guardie() {
        // Il difetto vero stava nella parte Python: «"safety": "ciao"» non
        // tornava ai predefiniti, alzava AttributeError fuori da ogni
        // riparo, e NOVA non partiva. Qui la sezione resta di fabbrica —
        // guardie comprese — e lo si dice.
        let (c, r) = applica(&fabbrica(), &json!({"safety": "ciao"}));
        assert_eq!(c["safety"], fabbrica()["safety"], "le guardie restano");
        assert_eq!(r.ignorate, vec!["safety".to_string()]);
    }

    #[test]
    fn e_vale_per_qualunque_forma_sbagliata() {
        for storta in [json!([1, 2]), json!(5), json!("x"), json!(true)] {
            let (c, r) = applica(&fabbrica(), &json!({ "server": storta }));
            assert_eq!(c["server"]["port"], 8080, "{storta}");
            assert_eq!(r.ignorate, vec!["server".to_string()], "{storta}");
        }
    }

    #[test]
    fn una_sezione_a_nulla_non_e_un_errore() {
        // `null` e' «non detto», non «detto male»: non vale la pena
        // spaventare qualcuno per una chiave lasciata vuota.
        let (c, r) = applica(&fabbrica(), &json!({"server": Value::Null}));
        assert_eq!(c["server"], fabbrica()["server"]);
        assert!(r.ignorate.is_empty());
    }

    #[test]
    fn e_il_resto_del_file_si_legge_lo_stesso() {
        // Una sezione storta non deve far buttare via anche quello che
        // nello stesso file era scritto bene.
        let l = leggi(&fabbrica(), r#"{"safety": "ciao", "fascicolo": "D:/x"}"#);
        assert_eq!(l.errore, "", "il file non e' «illeggibile», lo e' un pezzo");
        assert_eq!(l.config["fascicolo"], "D:/x");
        assert_eq!(l.rapporto.ignorate, vec!["safety".to_string()]);
    }
}
