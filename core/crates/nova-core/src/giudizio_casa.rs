//! La meta' di `nova-giudizio` che parla con llama-server (CANT-12).
//!
//! `nova-giudizio` sa fare di una domanda un elenco di candidati, uno per
//! lettera, e di quei logit un giudizio. Qui si chiedono i logit al modello di
//! casa, con le tre cose che `misure/banco_giudizio_llama.py` ha misurato su
//! quattro modelli prima di scriverci sopra (D371):
//!
//! - **il ragionamento si chiude prima della risposta**
//!   (`chat_template_kwargs: {"enable_thinking": false}`): con il ragionamento
//!   aperto il primo token e' quello che apre il pensiero, e le lettere fra i
//!   primi cinque hanno in media al piu' 0,001;
//! - **ogni lettera e' un token solo**, e lo si chiede a `/tokenize` prima di
//!   fidarsi, una volta per server;
//! - **`cache_prompt` si usa**: su 384 confronti, quattro modelli, non ha
//!   cambiato nessuna decisione.
//!
//! Tutto quello che non va finisce in un `Err`, e chi chiama lo tratta come un
//! giudizio che non c'e': si astiene, cioe' chiede. Un server che non e'
//! llama-server (LM Studio, Ollama) non ha `/apply-template` ne' `n_probs`, e
//! li' il giudizio non si puo' dare: si dice, non si indovina.

use nova_giudizio::{candidati, giudica, lettera, Domanda, Esito};
use serde_json::{json, Value};
use std::time::Duration;

/// Quanti token piu' probabili chiedere. Ventisei sono le lettere possibili:
/// con trentadue ci stanno tutte anche se il modello ne mette davanti qualche
/// altro.
pub const N_PROBS: usize = 32;

/// Sotto questa massa, fra i token letti, le lettere non sono la risposta del
/// modello. Misurata sui quattro modelli (D371): con il ragionamento chiuso la
/// piu' bassa e' stata 0,738 (Qwen3-8B), con il ragionamento aperto la media
/// non ha passato 0,001. La soglia sta in mezzo, piu' vicina a chi risponde.
pub const MASSA_MINIMA: f64 = 0.5;

/// Il sistema, uguale per ogni domanda: e' il prefisso che la cache riusa.
pub const SISTEMA: &str = "Sei il giudice di NOVA, un assistente che vive nel PC dell'utente. \
     Rispondi a ogni domanda con una sola lettera maiuscola, senza spiegare.";

/// Quanto si aspetta una risposta del server. Un passo solo, ma sopra un
/// prompt che la prima volta va letto tutto.
const ATTESA: Duration = Duration::from_secs(120);

/// I logit letti, e quanto ci si puo' fidare.
#[derive(Debug, Clone, PartialEq)]
pub struct Lettura {
    /// Un logit per candidato, nell'ordine delle lettere.
    pub logit: Vec<f64>,
    /// La probabilita' che le lettere della domanda hanno insieme, fra i token
    /// che il server ha restituito.
    pub massa: f64,
    /// Quante lettere non erano fra i token restituiti. Per loro il logit e'
    /// il piu' basso fra quelli letti: un tetto, non una misura. Le fa pesare
    /// di piu' di quanto pesano, quindi il giudizio puo' solo diventare meno
    /// sicuro, mai di piu'.
    pub stimate: usize,
}

/// Il corpo di `/apply-template`: lo stato, poi la domanda con le lettere, e
/// il ragionamento chiuso.
pub fn corpo_template(stato: &str, domanda: &Domanda) -> Value {
    let utente = format!(
        "{}{}",
        stato.trim(),
        nova_giudizio::candidati::testo_della_domanda(domanda)
    );
    json!({
        "messages": [
            { "role": "system", "content": SISTEMA },
            { "role": "user", "content": utente.trim_start() },
        ],
        "chat_template_kwargs": { "enable_thinking": false },
    })
}

/// Il corpo di `/completion`: un token solo, le probabilita' dei primi
/// [`N_PROBS`], la cache del prefisso.
pub fn corpo_completion(prompt: &str) -> Value {
    json!({
        "prompt": prompt,
        "n_predict": 1,
        "temperature": 0,
        "n_probs": N_PROBS,
        "cache_prompt": true,
    })
}

/// Dalla risposta di `/completion` ai logit delle prime `quanti` lettere.
///
/// Un token conta per una lettera se, tolti gli spazi, e' esattamente quella
/// lettera: «A» e « A» sono due token dello stesso significato, e le loro
/// probabilita' si sommano. Torna i logaritmi delle probabilita': differiscono
/// dai logit per una costante, che `morbido` toglie da sola.
pub fn leggi_lettere(risposta: &Value, quanti: usize) -> Result<Lettura, String> {
    let primi = risposta
        .get("completion_probabilities")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .and_then(|p| p.get("top_logprobs"))
        .and_then(Value::as_array)
        .filter(|a| !a.is_empty())
        .ok_or("il server non ha restituito le probabilita' dei token (n_probs)")?;
    let mut somme = vec![0.0_f64; quanti];
    let mut viste = vec![false; quanti];
    let mut minimo = f64::INFINITY;
    for t in primi {
        let lp = t
            .get("logprob")
            .and_then(Value::as_f64)
            .ok_or("un token senza logprob")?;
        if !lp.is_finite() {
            continue;
        }
        minimo = minimo.min(lp);
        let testo = t.get("token").and_then(Value::as_str).unwrap_or("").trim();
        let mut c = testo.chars();
        if let (Some(l), None) = (c.next(), c.next()) {
            if let Some(i) = nova_giudizio::posizione(l).filter(|i| *i < quanti) {
                somme[i] += lp.exp();
                viste[i] = true;
            }
        }
    }
    if !minimo.is_finite() {
        return Err("nessuna probabilita' utilizzabile fra i token restituiti".into());
    }
    let massa: f64 = somme.iter().sum();
    if massa < MASSA_MINIMA {
        return Err(format!(
            "le lettere hanno solo {massa:.3} fra i token piu' probabili: il modello non sta \
             rispondendo con una lettera (ragionamento aperto, o un modello che non segue la domanda)"
        ));
    }
    let stimate = viste.iter().filter(|v| !**v).count();
    let logit = somme
        .iter()
        .zip(&viste)
        .map(|(s, v)| if *v { s.ln() } else { minimo })
        .collect();
    Ok(Lettura {
        logit,
        massa,
        stimate,
    })
}

/// Le lettere delle prime `quanti` posizioni, e ognuna dev'essere un token.
pub fn lettere_un_token(
    quanti: usize,
    tokenizza: &dyn Fn(&str) -> Result<usize, String>,
) -> Result<(), String> {
    for i in 0..quanti {
        let l = lettera(i).ok_or("piu' candidati che lettere")?;
        for forma in [l.to_string(), format!(" {l}")] {
            let n = tokenizza(&forma)?;
            if n != 1 {
                return Err(format!(
                    "per questo modello «{forma}» e' di {n} token: le lettere non si leggono \
                     con un passo solo"
                ));
            }
        }
    }
    Ok(())
}

fn chiedi(base: &str, percorso: &str, corpo: &Value) -> Result<Value, String> {
    let url = format!("{}{percorso}", base.trim_end_matches('/'));
    match ureq::post(&url)
        .timeout(ATTESA)
        .set("Content-Type", "application/json")
        .send_string(&corpo.to_string())
    {
        Ok(r) => r
            .into_string()
            .map_err(|e| format!("{percorso}: {e}"))
            .and_then(|t| {
                serde_json::from_str(&t)
                    .map_err(|e| format!("{percorso}: risposta non leggibile: {e}"))
            }),
        Err(ureq::Error::Status(codice, _)) => Err(format!(
            "{percorso} ha risposto {codice}: il server non e' un llama-server che lo conosce"
        )),
        Err(e) => Err(format!("{percorso}: {e}")),
    }
}

/// La domanda al modello di casa, all'indirizzo `base` (per esempio
/// `http://127.0.0.1:8420`), e il giudizio che ne viene.
///
/// Bloccante: chi sta in un contesto asincrono lo chiama da
/// `spawn_blocking`.
pub fn giudica_in_casa(
    base: &str,
    stato: &str,
    domanda: &Domanda,
    temperatura: f64,
) -> Result<(Esito, Lettura), String> {
    domanda.valida()?;
    let quanti = candidati(domanda).len();
    lettere_un_token(quanti, &|testo| {
        let r = chiedi(
            base,
            "/tokenize",
            &json!({ "content": testo, "add_special": false }),
        )?;
        r.get("tokens")
            .and_then(Value::as_array)
            .map(Vec::len)
            .ok_or_else(|| "/tokenize: niente token nella risposta".to_string())
    })?;
    let modello = chiedi(base, "/apply-template", &corpo_template(stato, domanda))?;
    let prompt = modello
        .get("prompt")
        .and_then(Value::as_str)
        .ok_or("/apply-template: niente prompt nella risposta")?;
    let risposta = chiedi(base, "/completion", &corpo_completion(prompt))?;
    let lettura = leggi_lettere(&risposta, quanti)?;
    let esito = giudica(domanda, &lettura.logit, temperatura)?;
    Ok((esito, lettura))
}

// ------------------------------------------------------- quale cervello

/// Il testo dell'opzione che non fa salire niente.
pub const NESSUNA_CATEGORIA: &str = "nessuna di queste: un compito che il modello di casa puo' \
     fare da solo";
/// Il suo identificativo, che nessuna categoria della configurazione puo' avere.
pub const NESSUNA: &str = "__nessuna__";

/// La domanda di `QualeCervello` (il primo punto del censimento di CANT-12):
/// questo compito rientra in una delle categorie che fanno salire di gradino?
///
/// Le opzioni sono le categorie attive della configurazione, con la loro
/// descrizione, e in fondo «nessuna». E' la stessa decisione che oggi prende
/// `nova_scala::gradino_minimo` con le liste di parole; qui la prende il
/// modello, e un banco le confronta (`misure/banco_quale_cervello.py`).
pub fn domanda_quale_cervello(categorie: &[(String, String)]) -> Domanda {
    let mut opzioni: Vec<nova_giudizio::Opzione> = categorie
        .iter()
        .map(|(id, descrizione)| nova_giudizio::Opzione {
            id: id.clone(),
            descrizione: descrizione.clone(),
        })
        .collect();
    opzioni.push(nova_giudizio::Opzione {
        id: NESSUNA.into(),
        descrizione: NESSUNA_CATEGORIA.into(),
    });
    Domanda::Scelta {
        istruzioni: "Di che tipo e' questo compito? Scegli la categoria che lo descrive meglio."
            .into(),
        opzioni,
        politica: nova_giudizio::Politica::default(),
    }
}

/// Lo stato della domanda: il compito, e quanti file ci sono allegati.
/// Solo il compito, come `gradino_minimo`: il contenuto dei file allegati
/// farebbe scattare una categoria su una parola dentro un commento.
pub fn stato_del_compito(compito: &str, allegati: i64) -> String {
    format!("Compito: {}\nFile allegati: {allegati}", compito.trim())
}

#[cfg(test)]
mod prove {
    use super::*;
    use nova_giudizio::{Giudizio, Opzione, Politica, Risposta};
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    fn scelta() -> Domanda {
        Domanda::Scelta {
            istruzioni: "Qual e' la capitale d'Italia?".into(),
            opzioni: ["Milano", "Roma", "Napoli"]
                .iter()
                .map(|x| Opzione {
                    id: x.to_lowercase(),
                    descrizione: x.to_string(),
                })
                .collect(),
            politica: Politica::default(),
        }
    }

    fn probabilita(voci: &[(&str, f64)]) -> Value {
        json!({ "completion_probabilities": [{ "top_logprobs":
            voci.iter().map(|(t, p)| json!({ "token": t, "logprob": p.ln() })).collect::<Vec<_>>()
        }]})
    }

    #[test]
    fn il_ragionamento_si_chiude_e_la_cache_si_usa() {
        let t = corpo_template("Lo stato.", &scelta());
        assert_eq!(t["chat_template_kwargs"]["enable_thinking"], json!(false));
        let utente = t["messages"][1]["content"].as_str().unwrap();
        assert!(
            utente.starts_with("Lo stato.\n\nQuestion: Qual e' la capitale"),
            "{utente}"
        );
        assert!(utente.contains("B. Roma"), "{utente}");
        let c = corpo_completion("p");
        assert_eq!(c["n_predict"], json!(1));
        assert_eq!(c["temperature"], json!(0));
        assert_eq!(c["cache_prompt"], json!(true));
        assert_eq!(c["n_probs"], json!(N_PROBS));
    }

    #[test]
    fn le_lettere_si_leggono_e_le_forme_con_lo_spazio_si_sommano() {
        let r = probabilita(&[
            ("B", 0.6),
            (" B", 0.2),
            ("A", 0.1),
            ("\n", 0.05),
            ("C", 0.04),
        ]);
        let l = leggi_lettere(&r, 4).unwrap();
        assert!((l.massa - 0.94).abs() < 1e-9, "{l:?}");
        assert!((l.logit[1] - 0.8_f64.ln()).abs() < 1e-9);
        assert!((l.logit[0] - 0.1_f64.ln()).abs() < 1e-9);
        // La quarta lettera non c'e': prende il logit piu' basso letto, che e'
        // un tetto e non una misura.
        assert_eq!(l.stimate, 1);
        assert!((l.logit[3] - 0.04_f64.ln()).abs() < 1e-9);
    }

    #[test]
    fn una_lettera_dopo_l_ultima_della_domanda_non_conta() {
        let r = probabilita(&[("A", 0.5), ("D", 0.45)]);
        let l = leggi_lettere(&r, 3).unwrap();
        assert!((l.massa - 0.5).abs() < 1e-9, "{l:?}");
    }

    #[test]
    fn senza_lettere_non_c_e_giudizio() {
        // Come Gemma con il ragionamento aperto: il primo token apre il pensiero.
        let r = probabilita(&[("<|channel>", 0.999), ("A", 0.0005)]);
        let e = leggi_lettere(&r, 3).unwrap_err();
        assert!(e.contains("non sta rispondendo con una lettera"), "{e}");
        let e = leggi_lettere(&json!({ "content": "A" }), 3).unwrap_err();
        assert!(e.contains("n_probs"), "{e}");
    }

    #[test]
    fn una_lettera_di_due_token_ferma_tutto() {
        let conta = |t: &str| Ok(if t == " C" { 2 } else { 1 });
        let e = lettere_un_token(3, &conta).unwrap_err();
        assert!(e.contains("« C» e' di 2 token"), "{e}");
        assert_eq!(lettere_un_token(2, &conta), Ok(()));
    }

    /// I percorsi chiesti, con i loro corpi.
    type Visti = Arc<Mutex<Vec<(String, Value)>>>;

    /// Un llama-server finto: risponde ai tre percorsi e si segna i corpi.
    fn server_finto(prob_b: f64) -> (String, Visti) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", l.local_addr().unwrap());
        let visti = Arc::new(Mutex::new(Vec::new()));
        let v = visti.clone();
        std::thread::spawn(move || {
            for s in l.incoming().flatten() {
                let mut r = BufReader::new(s.try_clone().unwrap());
                let mut prima = String::new();
                r.read_line(&mut prima).unwrap();
                let percorso = prima.split_whitespace().nth(1).unwrap_or("").to_string();
                let mut lunghezza = 0;
                loop {
                    let mut riga = String::new();
                    r.read_line(&mut riga).unwrap();
                    if riga.trim().is_empty() {
                        break;
                    }
                    if let Some(x) = riga.to_ascii_lowercase().strip_prefix("content-length:") {
                        lunghezza = x.trim().parse().unwrap_or(0);
                    }
                }
                let mut corpo = vec![0; lunghezza];
                r.read_exact(&mut corpo).unwrap();
                let corpo: Value = serde_json::from_slice(&corpo).unwrap_or(Value::Null);
                v.lock().unwrap().push((percorso.clone(), corpo));
                let risposta = match percorso.as_str() {
                    "/tokenize" => json!({ "tokens": [42] }),
                    "/apply-template" => json!({ "prompt": "<sistema e domanda>" }),
                    "/completion" => probabilita(&[
                        ("B", prob_b),
                        ("A", (1.0 - prob_b) / 2.0),
                        ("C", (1.0 - prob_b) / 2.0),
                    ]),
                    _ => Value::Null,
                }
                .to_string();
                let mut s = s;
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    risposta.len(),
                    risposta
                );
            }
        });
        (base, visti)
    }

    #[test]
    fn dal_server_al_giudizio() {
        let (base, visti) = server_finto(0.9);
        let (esito, lettura) = giudica_in_casa(&base, "Siamo in Italia.", &scelta(), 1.0).unwrap();
        assert_eq!(
            esito.giudizio,
            Giudizio::Risposto(Risposta::Scelta { id: "roma".into() })
        );
        // Tre opzioni piu' «non lo so»: quattro lettere, e la quarta non c'era.
        assert_eq!(lettura.logit.len(), 4);
        assert_eq!(lettura.stimate, 1);
        let visti = visti.lock().unwrap();
        let percorsi: Vec<&str> = visti.iter().map(|(p, _)| p.as_str()).collect();
        // Le quattro lettere, ciascuna con e senza spazio, poi il resto.
        assert_eq!(percorsi.iter().filter(|p| **p == "/tokenize").count(), 8);
        assert_eq!(&percorsi[8..], ["/apply-template", "/completion"]);
        assert_eq!(
            visti[8].1["chat_template_kwargs"]["enable_thinking"],
            json!(false)
        );
        assert_eq!(visti[9].1["prompt"], json!("<sistema e domanda>"));
    }

    #[test]
    fn una_lettera_mancante_fa_il_giudizio_meno_sicuro() {
        // B al 40%, A e C al 30%, e «non lo so» (la D) non restituita: prende
        // il tetto, 0,3, e la B scende da 0,4 a 0,4 / 1,3. Un tetto puo' solo
        // togliere sicurezza, mai aggiungerne.
        let (base, _) = server_finto(0.4);
        let (esito, lettura) = giudica_in_casa(&base, "", &scelta(), 1.0).unwrap();
        assert_eq!(lettura.stimate, 1);
        assert!((esito.in_testa - 0.4 / 1.3).abs() < 1e-9, "{esito:?}");
        assert!((esito.indisponibile - 0.3 / 1.3).abs() < 1e-9, "{esito:?}");
    }

    /// Con un llama-server vero: `NOVA_GIUDIZIO_URL=http://127.0.0.1:8499
    /// cargo test -p nova-core --lib giudizio_casa -- --ignored --nocapture`.
    /// Stampa ogni giudizio; pretende solo che la strada funzioni e che la
    /// capitale d'Italia sia Roma.
    #[test]
    #[ignore = "vuole un llama-server acceso, in NOVA_GIUDIZIO_URL"]
    fn con_un_modello_vero() {
        let base = std::env::var("NOVA_GIUDIZIO_URL").expect("NOVA_GIUDIZIO_URL");
        let domande = [
            (
                "Qual e' la capitale d'Italia?",
                vec!["Milano", "Napoli", "Roma", "Torino"],
            ),
            (
                "Una mail che chiede la password dell'utente e':",
                vec!["normale", "sospetta", "urgente", "da inoltrare"],
            ),
            (
                "Il comando 'format C:' su Windows...",
                vec!["apre un file", "stampa", "elenca file", "cancella il disco"],
            ),
        ];
        for (d, o) in domande {
            let domanda = Domanda::Scelta {
                istruzioni: d.into(),
                opzioni: o
                    .iter()
                    .map(|x| Opzione {
                        id: x.to_string(),
                        descrizione: x.to_string(),
                    })
                    .collect(),
                politica: Politica::default(),
            };
            let (esito, lettura) = giudica_in_casa(&base, "", &domanda, 1.0).unwrap();
            println!(
                "{d} -> {} (in testa {:.3}, massa {:.3}, stimate {})",
                esito.giudizio.come_si_racconta(),
                esito.in_testa,
                lettura.massa,
                lettura.stimate
            );
            if d.contains("capitale") {
                assert_eq!(
                    esito.giudizio,
                    Giudizio::Risposto(Risposta::Scelta { id: "Roma".into() })
                );
            }
        }
    }

    #[test]
    fn la_domanda_del_cervello_ha_le_categorie_e_in_fondo_nessuna() {
        let d = domanda_quale_cervello(&[
            ("architettura".into(), "decisione di architettura".into()),
            ("perdita_dati".into(), "rischio di perdita di dati".into()),
        ]);
        let c = candidati(&d);
        // Due categorie, «nessuna», e la via d'uscita «non lo so».
        assert_eq!(
            c.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(),
            [
                "architettura",
                "perdita_dati",
                NESSUNA,
                nova_giudizio::ABBASTANZA
            ]
        );
        let testo = corpo_template(&stato_del_compito("progetta lo schema", 0), &d);
        let utente = testo["messages"][1]["content"].as_str().unwrap();
        assert!(
            utente.starts_with("Compito: progetta lo schema\nFile allegati: 0\n\nQuestion:"),
            "{utente}"
        );
        assert!(utente.contains("C. nessuna di queste"), "{utente}");
    }

    #[test]
    fn un_server_che_non_e_llama_server_si_dice() {
        let e = giudica_in_casa("http://127.0.0.1:9", "", &scelta(), 1.0).unwrap_err();
        assert!(e.contains("/tokenize"), "{e}");
    }
}
