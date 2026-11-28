//! Il dispositivo dalla sua parte (D449): tenere l'abbinamento, trovare il
//! PC, chiedere e sentire.
//!
//! - **[`Abbinamento`]** e' quello che il dispositivo tiene sul suo disco
//!   dopo essersi abbinato: dove trovare il PC, la sua impronta, il proprio
//!   id e il gettone. Il gettone e' un segreto: il file lo legge solo
//!   l'utente.
//! - **[`raggiungi`]** prova gli indirizzi dell'invito in ordine e si ferma
//!   al primo che risponde: quello della rete di casa quando si e' a casa,
//!   quello della rete privata quando si e' fuori.
//! - **[`Collegamento`]** e' il canale dopo il saluto: una richiesta ha il
//!   suo numero e aspetta la risposta con quel numero, e gli eventi del
//!   demone arrivano a parte. Quando il canale si chiude, chi aspettava una
//!   risposta lo viene a sapere invece di aspettare per sempre.

use crate::invito::Invito;
use crate::saluto;
use crate::tls::{self, Canale};
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot, watch};

/// Quanti eventi possono stare in coda prima che il dispositivo li legga.
/// Oltre, i piu' nuovi si perdono: uno schermo lento non ferma il canale.
pub const EVENTI_IN_CODA: usize = 256;

/// Cosa si dice a chi aspettava una risposta quando il canale si chiude.
pub const CANALE_CHIUSO: &str = "il collegamento col PC si e' chiuso";

/// Quello che il dispositivo tiene dopo essersi abbinato.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Abbinamento {
    pub indirizzi: Vec<String>,
    pub porta: u16,
    pub impronta: String,
    pub id: String,
    pub gettone: String,
}

impl Abbinamento {
    /// Quello salvato, se c'e'. Un file che non si legge e' un errore: il
    /// dispositivo deve dire che va abbinato di nuovo, non far finta di
    /// niente.
    pub fn leggi(percorso: &Path) -> Result<Option<Self>> {
        match std::fs::read_to_string(percorso) {
            Ok(testo) => serde_json::from_str(&testo)
                .map(Some)
                .with_context(|| format!("{} non si legge", percorso.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).with_context(|| format!("{} non si apre", percorso.display())),
        }
    }

    /// Lo salva, leggibile solo dall'utente: dentro c'e' il gettone.
    pub fn salva(&self, percorso: &Path) -> Result<()> {
        use std::io::Write;
        if let Some(dir) = percorso.parent() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("non riesco a creare {}", dir.display()))?;
        }
        let accanto = percorso.with_extension("nuovo");
        let mut come = std::fs::OpenOptions::new();
        come.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            come.mode(0o600);
        }
        come.open(&accanto)
            .and_then(|mut f| f.write_all(serde_json::to_string_pretty(self)?.as_bytes()))
            .and_then(|()| std::fs::rename(&accanto, percorso))
            .with_context(|| format!("non riesco a scrivere {}", percorso.display()))
    }

    /// Lo toglie: il dispositivo torna da abbinare.
    pub fn dimentica(percorso: &Path) -> Result<()> {
        match std::fs::remove_file(percorso) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => {
                Err(e).with_context(|| format!("non riesco a togliere {}", percorso.display()))
            }
        }
    }
}

/// Si collega al primo indirizzo che risponde col certificato del PC. Torna
/// il canale e l'indirizzo che ha risposto; se non risponde nessuno, perche'
/// ciascuno no.
pub async fn raggiungi(
    indirizzi: &[String],
    porta: u16,
    impronta: &str,
) -> Result<(Canale, String)> {
    let mut no = Vec::new();
    for indirizzo in indirizzi {
        match tls::collega(indirizzo, porta, impronta).await {
            Ok(canale) => return Ok((canale, indirizzo.clone())),
            Err(e) => no.push(format!("{e:#}")),
        }
    }
    if no.is_empty() {
        bail!("non c'e' nessun indirizzo a cui cercare il PC");
    }
    bail!("il PC non si trova: {}", no.join("; "))
}

/// La prima volta: dall'invito all'abbinamento. Il canale che torna e' gia'
/// dentro.
pub async fn abbina(invito: &Invito, nome: &str) -> Result<(Abbinamento, Canale)> {
    let (mut canale, _) = raggiungi(&invito.indirizzi, invito.porta, &invito.impronta).await?;
    let (id, gettone) = saluto::abbinati(&mut canale, &invito.codice, nome).await?;
    let abbinamento = Abbinamento {
        indirizzi: invito.indirizzi.clone(),
        porta: invito.porta,
        impronta: invito.impronta.clone(),
        id,
        gettone,
    };
    Ok((abbinamento, canale))
}

/// Le altre volte: col proprio gettone.
pub async fn entra(a: &Abbinamento) -> Result<Canale> {
    let (mut canale, _) = raggiungi(&a.indirizzi, a.porta, &a.impronta).await?;
    saluto::entra(&mut canale, &a.id, &a.gettone).await?;
    Ok(canale)
}

/// Un evento del demone.
#[derive(Debug, Clone, PartialEq)]
pub struct Evento {
    pub topic: String,
    pub dati: Value,
}

type Attese = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>>;

/// Il canale dopo il saluto.
pub struct Collegamento {
    scrivi: mpsc::Sender<String>,
    attese: Attese,
    prossimo: AtomicU64,
    chiuso: watch::Receiver<bool>,
}

impl Collegamento {
    /// Prende un canale gia' dentro e lo tiene: torna il collegamento e la
    /// coda degli eventi, che finisce quando il canale si chiude.
    pub fn avvia<S>(canale: S) -> (Arc<Self>, mpsc::Receiver<Evento>)
    where
        S: AsyncRead + AsyncWrite + Send + 'static,
    {
        let (lettore, mut scrittore) = tokio::io::split(canale);
        let (scrivi, mut da_scrivere) = mpsc::channel::<String>(64);
        let (eventi_tx, eventi) = mpsc::channel(EVENTI_IN_CODA);
        let (chiudi, chiuso) = watch::channel(false);
        let attese: Attese = Arc::default();

        tokio::spawn(async move {
            while let Some(riga) = da_scrivere.recv().await {
                if scrittore.write_all(riga.as_bytes()).await.is_err()
                    || scrittore.write_all(b"\n").await.is_err()
                    || scrittore.flush().await.is_err()
                {
                    break;
                }
            }
            let _ = scrittore.shutdown().await;
        });

        let attese_l = attese.clone();
        tokio::spawn(async move {
            let mut righe = BufReader::new(lettore).lines();
            while let Ok(Some(riga)) = righe.next_line().await {
                let Ok(v) = serde_json::from_str::<Value>(&riga) else {
                    continue;
                };
                if v.get("method").and_then(Value::as_str) == Some("event") {
                    let p = v.get("params");
                    let evento = Evento {
                        topic: p
                            .and_then(|p| p.get("topic"))
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        dati: p.and_then(|p| p.get("data")).cloned().unwrap_or(json!({})),
                    };
                    // Coda piena: l'evento si perde, il canale va avanti.
                    let _ = eventi_tx.try_send(evento);
                    continue;
                }
                let Some(id) = v.get("id").and_then(Value::as_u64) else {
                    continue;
                };
                if let Some(chi) = attese_l.lock().unwrap().remove(&id) {
                    let esito = match v.get("error") {
                        Some(e) => Err(e
                            .get("message")
                            .and_then(Value::as_str)
                            .unwrap_or("il PC ha risposto con un errore")
                            .to_string()),
                        None => Ok(v.get("result").cloned().unwrap_or(Value::Null)),
                    };
                    let _ = chi.send(esito);
                }
            }
            // Chiuso: chi aspettava lo viene a sapere.
            for (_, chi) in attese_l.lock().unwrap().drain() {
                let _ = chi.send(Err(CANALE_CHIUSO.to_string()));
            }
            let _ = chiudi.send(true);
        });

        let collegamento = Self {
            scrivi,
            attese,
            prossimo: AtomicU64::new(1),
            chiuso,
        };
        (Arc::new(collegamento), eventi)
    }

    /// Chiede una cosa al demone e aspetta la risposta, al massimo `attesa`.
    pub async fn chiedi(&self, metodo: &str, params: Value, attesa: Duration) -> Result<Value> {
        if *self.chiuso.borrow() {
            bail!(CANALE_CHIUSO);
        }
        let id = self.prossimo.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.attese.lock().unwrap().insert(id, tx);
        let riga = json!({ "jsonrpc": "2.0", "id": id, "method": metodo, "params": params });
        if self.scrivi.send(riga.to_string()).await.is_err() {
            self.attese.lock().unwrap().remove(&id);
            bail!(CANALE_CHIUSO);
        }
        match tokio::time::timeout(attesa, rx).await {
            Ok(Ok(esito)) => esito.map_err(|e| anyhow!(e)),
            Ok(Err(_)) => bail!(CANALE_CHIUSO),
            Err(_) => {
                self.attese.lock().unwrap().remove(&id);
                bail!(
                    "il PC non ha risposto a «{metodo}» entro {} s",
                    attesa.as_secs()
                )
            }
        }
    }

    /// Manda una cosa al demone senza aspettare risposta (D455): cosi'
    /// viaggia il microfono, dieci pezzi al secondo. Non aspetta nemmeno il
    /// posto in coda: se il canale e' indietro il pezzo si perde, che per una
    /// voce dal vivo e' meglio che arrivare tardi. Dice se e' partito.
    pub fn manda(&self, metodo: &str, params: Value) -> bool {
        if *self.chiuso.borrow() {
            return false;
        }
        let riga = json!({ "jsonrpc": "2.0", "method": metodo, "params": params });
        self.scrivi.try_send(riga.to_string()).is_ok()
    }

    /// Il canale si e' chiuso: non partira' piu' niente.
    pub fn e_chiuso(&self) -> bool {
        *self.chiuso.borrow()
    }

    /// Aspetta che il canale si chiuda.
    pub async fn finito(&self) {
        let mut chiuso = self.chiuso.clone();
        let _ = chiuso.wait_for(|c| *c).await;
    }
}

#[cfg(test)]
mod prove {
    use super::*;
    use crate::abbinamento::{Banco, Dispositivi};
    use crate::identita::Identita;
    use crate::saluto::Soglia;
    use std::time::Instant;
    use tokio::net::TcpListener;

    const POCO: Duration = Duration::from_secs(10);

    /// Un PC finto che dopo il saluto parla come il demone quanto basta:
    /// «eco» risponde coi parametri, «due» risponde prima alla seconda
    /// richiesta, «evento» manda un evento, «errore» risponde con un errore,
    /// «muto» non risponde, «chiudi» chiude. Di ogni notifica (una riga
    /// senza numero) tiene il conto, e «notifiche» lo dice con l'ultima.
    async fn pc(nome: &str) -> (Identita, Arc<Soglia>, u16, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("nova-cliente-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let identita = Identita::nuova().unwrap();
        let soglia = Arc::new(Soglia {
            banco: Mutex::new(Banco::default()),
            dispositivi: Mutex::new(Dispositivi::apri(&dir.join("dispositivi.json")).unwrap()),
        });
        let ascolto = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let porta = ascolto.local_addr().unwrap().port();
        let (accettatore, soglia_a) = (tls::accettatore(&identita).unwrap(), soglia.clone());
        tokio::spawn(async move {
            loop {
                let (tcp, _) = ascolto.accept().await.unwrap();
                let (accettatore, soglia) = (accettatore.clone(), soglia_a.clone());
                tokio::spawn(async move {
                    let Ok(mut canale) = accettatore.accept(tcp).await else {
                        return;
                    };
                    if soglia.accogli(&mut canale).await.is_err() {
                        return;
                    }
                    let (lettore, mut scrittore) = tokio::io::split(canale);
                    let mut righe = BufReader::new(lettore).lines();
                    let mut tenuta: Option<Value> = None;
                    let mut notifiche: Vec<Value> = Vec::new();
                    while let Ok(Some(riga)) = righe.next_line().await {
                        let v: Value = serde_json::from_str(&riga).unwrap();
                        if v.get("id").is_none() {
                            notifiche.push(v);
                            continue;
                        }
                        let risposta =
                            |r: Value| format!("{}\n", json!({ "id": v["id"], "result": r }));
                        let fuori = match v["method"].as_str().unwrap() {
                            "eco" => risposta(v["params"].clone()),
                            // La prima «due» si tiene; alla seconda si
                            // risponde prima a lei e poi alla prima.
                            "due" => match tenuta.take() {
                                None => {
                                    tenuta = Some(v.clone());
                                    continue;
                                }
                                Some(prima) => format!(
                                    "{}{}\n",
                                    risposta(v["params"].clone()),
                                    json!({ "id": prima["id"], "result": prima["params"] })
                                ),
                            },
                            "evento" => format!(
                                "{}\n{}",
                                json!({ "method": "event", "params": { "topic": "stato.cambiato", "data": { "stato": "penso" } } }),
                                risposta(json!({ "ok": true }))
                            ),
                            "errore" => format!(
                                "{}\n",
                                json!({ "id": v["id"], "error": { "code": -32001, "message": "non si puo'" } })
                            ),
                            "muto" => continue,
                            "notifiche" => risposta(json!({
                                "quante": notifiche.len(),
                                "ultima": notifiche.last(),
                            })),
                            _ => break,
                        };
                        if scrittore.write_all(fuori.as_bytes()).await.is_err() {
                            break;
                        }
                    }
                });
            }
        });
        (identita, soglia, porta, dir)
    }

    fn invito(identita: &Identita, soglia: &Soglia, porta: u16, indirizzi: &[&str]) -> Invito {
        Invito {
            indirizzi: indirizzi.iter().map(|s| s.to_string()).collect(),
            porta,
            impronta: identita.impronta(),
            codice: soglia.banco.lock().unwrap().nuovo_codice(Instant::now()),
        }
    }

    #[test]
    fn l_abbinamento_si_salva_e_si_rilegge() {
        let dir = std::env::temp_dir().join(format!("nova-cliente-file-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let file = dir.join("abbinamento.json");
        assert_eq!(Abbinamento::leggi(&file).unwrap(), None, "mai abbinato");
        let a = Abbinamento {
            indirizzi: vec!["192.168.1.5".into(), "pc.rete.ts.net".into()],
            porta: 47_810,
            impronta: "impronta".into(),
            id: "id".into(),
            gettone: "segreto".into(),
        };
        a.salva(&file).unwrap();
        assert_eq!(Abbinamento::leggi(&file).unwrap(), Some(a));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let modo = std::fs::metadata(&file).unwrap().permissions().mode();
            assert_eq!(modo & 0o777, 0o600, "dentro c'e' il gettone");
        }
        std::fs::write(&file, "{ rotto").unwrap();
        assert!(
            Abbinamento::leggi(&file).is_err(),
            "rotto non e' «mai abbinato»"
        );
        Abbinamento::dimentica(&file).unwrap();
        Abbinamento::dimentica(&file).unwrap();
        assert_eq!(Abbinamento::leggi(&file).unwrap(), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn ci_si_abbina_dall_invito_e_si_rientra_dall_abbinamento() {
        let (identita, soglia, porta, dir) = pc("abbina").await;
        // Il primo indirizzo non risponde: si passa al secondo.
        let chiusa = TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        assert!(tls::collega("127.0.0.1", chiusa, &identita.impronta())
            .await
            .is_err());
        let i = invito(
            &identita,
            &soglia,
            porta,
            &["localhost.invalid", "127.0.0.1"],
        );
        let (a, canale) = abbina(&i, "Tab A8").await.unwrap();
        assert_eq!(a.indirizzi, i.indirizzi);
        assert_eq!(
            (a.porta, a.impronta.as_str()),
            (porta, identita.impronta().as_str())
        );
        let (c, _eventi) = Collegamento::avvia(canale);
        assert_eq!(
            c.chiedi("eco", json!({ "a": 1 }), POCO).await.unwrap(),
            json!({ "a": 1 })
        );

        let (c, _eventi) = Collegamento::avvia(entra(&a).await.unwrap());
        assert_eq!(
            c.chiedi("eco", json!("di nuovo"), POCO).await.unwrap(),
            json!("di nuovo")
        );

        // Col gettone di un altro non si entra, e l'errore e' quello del PC.
        let mut falso = a.clone();
        falso.gettone = "un altro".into();
        let e = entra(&falso).await.err().unwrap().to_string();
        assert_eq!(e, saluto::NON_ABBINATO);
        // Nessun indirizzo buono: si dice perche'.
        let mut perso = a.clone();
        perso.indirizzi = vec!["localhost.invalid".into()];
        let e = entra(&perso).await.err().unwrap().to_string();
        assert!(e.starts_with("il PC non si trova: "), "{e}");
        perso.indirizzi.clear();
        assert!(entra(&perso)
            .await
            .err()
            .unwrap()
            .to_string()
            .contains("nessun indirizzo"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn ogni_risposta_torna_a_chi_l_ha_chiesta_e_gli_eventi_arrivano_a_parte() {
        let (identita, soglia, porta, dir) = pc("richieste").await;
        let i = invito(&identita, &soglia, porta, &["127.0.0.1"]);
        let (_, canale) = abbina(&i, "").await.unwrap();
        let (c, mut eventi) = Collegamento::avvia(canale);

        // Due richieste insieme, e il PC risponde prima alla seconda.
        let (c1, c2) = (c.clone(), c.clone());
        let prima = tokio::spawn(async move { c1.chiedi("due", json!("prima"), POCO).await });
        tokio::time::sleep(Duration::from_millis(100)).await;
        let seconda = tokio::spawn(async move { c2.chiedi("due", json!("seconda"), POCO).await });
        assert_eq!(seconda.await.unwrap().unwrap(), json!("seconda"));
        assert_eq!(prima.await.unwrap().unwrap(), json!("prima"));

        assert_eq!(
            c.chiedi("evento", json!({}), POCO).await.unwrap(),
            json!({ "ok": true })
        );
        let e = eventi.recv().await.unwrap();
        assert_eq!(
            e,
            Evento {
                topic: "stato.cambiato".into(),
                dati: json!({ "stato": "penso" })
            }
        );

        let e = c
            .chiedi("errore", json!({}), POCO)
            .await
            .unwrap_err()
            .to_string();
        assert_eq!(e, "non si puo'");
        let e = c
            .chiedi("muto", json!({}), Duration::from_millis(300))
            .await
            .unwrap_err();
        assert!(e.to_string().contains("non ha risposto a «muto»"), "{e}");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// D455: il microfono viaggia a notifiche, senza risposta e in ordine.
    #[tokio::test]
    async fn una_notifica_parte_senza_aspettare_risposta() {
        let (identita, soglia, porta, dir) = pc("notifiche").await;
        let i = invito(&identita, &soglia, porta, &["127.0.0.1"]);
        let (_, canale) = abbina(&i, "").await.unwrap();
        let (c, _eventi) = Collegamento::avvia(canale);
        assert!(!c.e_chiuso());
        for n in 0..10i16 {
            assert!(c.manda(crate::voce::METODO_DEL_PEZZO, crate::voce::pezzo(&[n, -n])));
        }
        // La richiesta parte dopo le notifiche, sullo stesso canale: quando
        // torna, il PC le ha lette tutte.
        let v = c.chiedi("notifiche", json!({}), POCO).await.unwrap();
        assert_eq!(v["quante"], 10);
        assert_eq!(v["ultima"]["method"], "voce/pezzo");
        assert!(
            v["ultima"].get("id").is_none(),
            "una notifica non ha numero"
        );
        assert_eq!(
            crate::voce::leggi_il_pezzo(&v["ultima"]["params"]).unwrap(),
            vec![9, -9]
        );
        // A canale chiuso non parte, e lo dice.
        let _ = c.chiedi("chiudi", json!({}), POCO).await;
        tokio::time::timeout(POCO, c.finito()).await.unwrap();
        assert!(!c.manda("voce/pezzo", json!({})));
        assert!(c.e_chiuso());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn quando_il_canale_si_chiude_chi_aspettava_lo_viene_a_sapere() {
        let (identita, soglia, porta, dir) = pc("chiuso").await;
        let i = invito(&identita, &soglia, porta, &["127.0.0.1"]);
        let (_, canale) = abbina(&i, "").await.unwrap();
        let (c, mut eventi) = Collegamento::avvia(canale);
        let c1 = c.clone();
        let in_attesa = tokio::spawn(async move { c1.chiedi("muto", json!({}), POCO).await });
        tokio::time::sleep(Duration::from_millis(100)).await;
        let e = c
            .chiedi("chiudi", json!({}), POCO)
            .await
            .unwrap_err()
            .to_string();
        assert_eq!(e, CANALE_CHIUSO);
        assert_eq!(
            in_attesa.await.unwrap().unwrap_err().to_string(),
            CANALE_CHIUSO
        );
        tokio::time::timeout(POCO, c.finito())
            .await
            .expect("il collegamento sa di essere chiuso");
        assert!(
            eventi.recv().await.is_none(),
            "e la coda degli eventi finisce"
        );
        let e = c
            .chiedi("eco", json!({}), POCO)
            .await
            .unwrap_err()
            .to_string();
        assert_eq!(e, CANALE_CHIUSO, "dopo non si chiede piu'");
        let _ = std::fs::remove_dir_all(dir);
    }
}
