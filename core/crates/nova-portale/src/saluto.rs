//! Il saluto: la prima riga del canale, che dice chi e' il dispositivo.
//!
//! Prima di ogni altra cosa il dispositivo manda **una riga JSON**:
//!
//! - `{"fai":"abbina","codice":"…","nome":"…"}` la prima volta, col codice
//!   dell'invito: il PC risponde `{"ok":true,"id":"…","gettone":"…"}`;
//! - `{"fai":"entra","id":"…","gettone":"…"}` tutte le altre: `{"ok":true,"id":"…"}`.
//!
//! Dopo un `ok` il canale porta le righe del demone. Dopo un no il PC
//! risponde `{"ok":false,"errore":"…"}` e chiude; prima di rispondere
//! aspetta [`PAUSA_DOPO_UN_NO`], e chi non saluta entro [`TEMPO`] o manda
//! una riga piu' lunga di [`RIGA_MASSIMA`] e' fuori: chi tira a indovinare
//! va piano, e non tiene occupato il PC.

use crate::abbinamento::{Banco, Dispositivi, Dispositivo, Esito};
use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Quanto puo' essere lunga la riga del saluto.
pub const RIGA_MASSIMA: usize = 2048;

/// Entro quanto il dispositivo deve salutare.
pub const TEMPO: Duration = Duration::from_secs(10);

/// Quanto aspetta il PC prima di dire di no.
pub const PAUSA_DOPO_UN_NO: Duration = Duration::from_secs(1);

/// Cosa si dice a chi presenta un codice che non vale.
pub const CODICE_NON_BUONO: &str =
    "il codice non vale: e' sbagliato, scaduto o gia' usato. Chiedine uno nuovo dal pannello di NOVA";

/// Cosa si dice a chi non e' abbinato, o non lo e' piu'.
pub const NON_ABBINATO: &str = "questo dispositivo non e' abbinato a NOVA, o e' stato dimenticato";

/// Cosa si dice a chi non saluta come si deve.
pub const SALUTO_NON_LETTO: &str = "il saluto non si legge";

/// La richiesta del dispositivo.
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "fai", rename_all = "lowercase")]
pub enum Richiesta {
    Abbina {
        codice: String,
        #[serde(default)]
        nome: String,
    },
    Entra {
        id: String,
        gettone: String,
    },
}

/// Cio' che decide chi entra: il codice in corso e i dispositivi abbinati.
pub struct Soglia {
    pub banco: Mutex<Banco>,
    pub dispositivi: Mutex<Dispositivi>,
}

fn adesso() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Legge una riga, un byte alla volta: quello che viene dopo l'a capo e' del
/// demone, e non si deve mangiare.
pub async fn leggi_riga<S: AsyncRead + Unpin>(canale: &mut S) -> Result<String> {
    let mut riga = Vec::new();
    loop {
        let b = canale.read_u8().await.context("il canale si e' chiuso")?;
        if b == b'\n' {
            break;
        }
        if riga.len() >= RIGA_MASSIMA {
            bail!("riga troppo lunga");
        }
        riga.push(b);
    }
    String::from_utf8(riga).map_err(|_| anyhow!("la riga non e' testo"))
}

async fn scrivi_riga<S: AsyncWrite + Unpin>(canale: &mut S, v: &Value) -> Result<()> {
    canale.write_all(format!("{v}\n").as_bytes()).await?;
    canale.flush().await?;
    Ok(())
}

impl Soglia {
    /// Decide una richiesta. Torna il dispositivo e la risposta da dargli.
    fn decidi(&self, richiesta: Richiesta) -> Result<(Dispositivo, Value), &'static str> {
        match richiesta {
            Richiesta::Abbina { codice, nome } => {
                let esito = self.banco.lock().unwrap().prova(&codice, Instant::now());
                if esito != Esito::Giusto {
                    return Err(CODICE_NON_BUONO);
                }
                let (d, gettone) = self
                    .dispositivi
                    .lock()
                    .unwrap()
                    .aggiungi(&nome, adesso())
                    .map_err(|_| "il PC non riesce a salvare l'abbinamento")?;
                let risposta = json!({ "ok": true, "id": d.id, "gettone": gettone });
                Ok((d, risposta))
            }
            Richiesta::Entra { id, gettone } => {
                let d = self
                    .dispositivi
                    .lock()
                    .unwrap()
                    .riconosci(&id, &gettone, adesso())
                    .ok_or(NON_ABBINATO)?;
                let risposta = json!({ "ok": true, "id": d.id });
                Ok((d, risposta))
            }
        }
    }

    /// La meta' del PC: aspetta il saluto, decide, risponde. Torna il
    /// dispositivo che e' entrato, o perche' non e' entrato nessuno.
    pub async fn accogli<S>(&self, canale: &mut S) -> Result<Dispositivo, String>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let deciso = match tokio::time::timeout(TEMPO, leggi_riga(canale)).await {
            Err(_) => return Err("nessun saluto in tempo".into()),
            Ok(Err(e)) => return Err(e.to_string()),
            Ok(Ok(riga)) => match serde_json::from_str::<Richiesta>(&riga) {
                Ok(richiesta) => self.decidi(richiesta),
                Err(_) => Err(SALUTO_NON_LETTO),
            },
        };
        match deciso {
            Ok((d, risposta)) => {
                scrivi_riga(canale, &risposta)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(d)
            }
            Err(perche) => {
                tokio::time::sleep(PAUSA_DOPO_UN_NO).await;
                let _ = scrivi_riga(canale, &json!({ "ok": false, "errore": perche })).await;
                let _ = canale.shutdown().await;
                Err(perche.to_string())
            }
        }
    }
}

/// Legge la risposta del PC al saluto.
async fn risposta<S: AsyncRead + Unpin>(canale: &mut S) -> Result<Value> {
    let riga = tokio::time::timeout(TEMPO + PAUSA_DOPO_UN_NO, leggi_riga(canale))
        .await
        .map_err(|_| anyhow!("il PC non risponde al saluto"))??;
    let v: Value = serde_json::from_str(&riga).context("la risposta del PC non si legge")?;
    if v.get("ok").and_then(Value::as_bool) != Some(true) {
        bail!(
            "{}",
            v.get("errore")
                .and_then(Value::as_str)
                .unwrap_or("il PC ha detto di no")
        );
    }
    Ok(v)
}

/// La meta' del dispositivo, la prima volta: presenta il codice dell'invito.
/// Torna il suo id e il suo gettone, da tenere.
pub async fn abbinati<S>(canale: &mut S, codice: &str, nome: &str) -> Result<(String, String)>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    scrivi_riga(
        canale,
        &json!({ "fai": "abbina", "codice": codice, "nome": nome }),
    )
    .await?;
    let v = risposta(canale).await?;
    let campo = |c: &str| {
        v.get(c)
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| anyhow!("nella risposta del PC manca «{c}»"))
    };
    Ok((campo("id")?, campo("gettone")?))
}

/// La meta' del dispositivo, le altre volte: si presenta col suo gettone.
pub async fn entra<S>(canale: &mut S, id: &str, gettone: &str) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    scrivi_riga(
        canale,
        &json!({ "fai": "entra", "id": id, "gettone": gettone }),
    )
    .await?;
    risposta(canale).await.map(|_| ())
}

#[cfg(test)]
mod prove {
    use super::*;
    use crate::identita::Identita;
    use crate::tls;
    use tokio::io::AsyncBufReadExt;
    use tokio::net::TcpListener;

    fn soglia(nome: &str) -> (std::sync::Arc<Soglia>, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("nova-portale-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let s = Soglia {
            banco: Mutex::new(Banco::default()),
            dispositivi: Mutex::new(Dispositivi::apri(&dir.join("dispositivi.json")).unwrap()),
        };
        (std::sync::Arc::new(s), dir)
    }

    /// Un PC finto: accetta, saluta, e a chi entra fa l'eco di una riga.
    async fn pc(identita: &Identita, soglia: std::sync::Arc<Soglia>) -> u16 {
        let ascolto = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let porta = ascolto.local_addr().unwrap().port();
        let accettatore = tls::accettatore(identita).unwrap();
        tokio::spawn(async move {
            loop {
                let (tcp, _) = ascolto.accept().await.unwrap();
                let (accettatore, soglia) = (accettatore.clone(), soglia.clone());
                tokio::spawn(async move {
                    let Ok(mut canale) = accettatore.accept(tcp).await else {
                        return;
                    };
                    if soglia.accogli(&mut canale).await.is_ok() {
                        let mut righe = tokio::io::BufReader::new(canale);
                        let mut riga = String::new();
                        if righe.read_line(&mut riga).await.is_ok() {
                            let _ = righe
                                .get_mut()
                                .write_all(format!("eco {riga}").as_bytes())
                                .await;
                        }
                    }
                });
            }
        });
        porta
    }

    #[test]
    fn il_saluto_si_legge() {
        assert_eq!(
            serde_json::from_str::<Richiesta>(r#"{"fai":"abbina","codice":"c"}"#).unwrap(),
            Richiesta::Abbina {
                codice: "c".into(),
                nome: String::new()
            }
        );
        assert_eq!(
            serde_json::from_str::<Richiesta>(r#"{"fai":"entra","id":"i","gettone":"g"}"#).unwrap(),
            Richiesta::Entra {
                id: "i".into(),
                gettone: "g".into()
            }
        );
        assert!(serde_json::from_str::<Richiesta>(r#"{"fai":"entra","id":"i"}"#).is_err());
        assert!(serde_json::from_str::<Richiesta>(r#"{"method":"daemon/status"}"#).is_err());
    }

    #[tokio::test]
    async fn ci_si_abbina_col_codice_e_poi_si_entra_col_gettone() {
        let identita = Identita::nuova().unwrap();
        let (soglia, dir) = soglia("saluto");
        let porta = pc(&identita, soglia.clone()).await;
        let codice = soglia.banco.lock().unwrap().nuovo_codice(Instant::now());

        let mut c = tls::collega("127.0.0.1", porta, &identita.impronta())
            .await
            .unwrap();
        let (id, gettone) = abbinati(&mut c, &codice, "Tab A8").await.unwrap();
        // Abbinato, il canale e' gia' quello del demone.
        c.write_all(b"ciao\n").await.unwrap();
        assert_eq!(leggi_riga(&mut c).await.unwrap(), "eco ciao");
        assert_eq!(
            soglia.dispositivi.lock().unwrap().elenco()[0].nome,
            "Tab A8"
        );

        // Il codice si e' consumato: un secondo dispositivo non si abbina.
        let mut c = tls::collega("127.0.0.1", porta, &identita.impronta())
            .await
            .unwrap();
        let no = abbinati(&mut c, &codice, "altro")
            .await
            .unwrap_err()
            .to_string();
        assert_eq!(no, CODICE_NON_BUONO);

        // Col gettone si entra; con un altro no.
        let mut c = tls::collega("127.0.0.1", porta, &identita.impronta())
            .await
            .unwrap();
        entra(&mut c, &id, &gettone).await.unwrap();
        c.write_all(b"di nuovo\n").await.unwrap();
        assert_eq!(leggi_riga(&mut c).await.unwrap(), "eco di nuovo");
        let mut c = tls::collega("127.0.0.1", porta, &identita.impronta())
            .await
            .unwrap();
        let prima = Instant::now();
        let no = entra(&mut c, &id, "un altro gettone")
            .await
            .unwrap_err()
            .to_string();
        assert_eq!(no, NON_ABBINATO);
        assert!(prima.elapsed() >= PAUSA_DOPO_UN_NO, "il no arriva piano");

        // Dimenticato: il suo gettone non apre piu'.
        assert!(soglia.dispositivi.lock().unwrap().dimentica(&id).unwrap());
        let mut c = tls::collega("127.0.0.1", porta, &identita.impronta())
            .await
            .unwrap();
        assert_eq!(
            entra(&mut c, &id, &gettone).await.unwrap_err().to_string(),
            NON_ABBINATO
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn chi_non_saluta_come_si_deve_resta_fuori() {
        let identita = Identita::nuova().unwrap();
        let (soglia, dir) = soglia("fuori");
        let porta = pc(&identita, soglia).await;
        // Una riga del demone al posto del saluto: non passa.
        let mut c = tls::collega("127.0.0.1", porta, &identita.impronta())
            .await
            .unwrap();
        c.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"daemon/status\"}\n")
            .await
            .unwrap();
        let v: Value = serde_json::from_str(&leggi_riga(&mut c).await.unwrap()).unwrap();
        assert_eq!(v, json!({ "ok": false, "errore": SALUTO_NON_LETTO }));
        assert!(leggi_riga(&mut c).await.is_err(), "e il canale si chiude");
        // Una riga senza fine: il PC non la aspetta per sempre.
        let mut c = tls::collega("127.0.0.1", porta, &identita.impronta())
            .await
            .unwrap();
        c.write_all(&vec![b'x'; RIGA_MASSIMA + 10]).await.unwrap();
        assert!(leggi_riga(&mut c).await.is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn il_dispositivo_parla_solo_col_pc_dell_invito() {
        let (vero, finto) = (Identita::nuova().unwrap(), Identita::nuova().unwrap());
        let (soglia, dir) = soglia("impronta");
        // Risponde un altro PC, col suo certificato: il dispositivo ha
        // l'impronta di quello vero, e non ci parla.
        let porta = pc(&finto, soglia).await;
        let e = tls::collega("127.0.0.1", porta, &vero.impronta())
            .await
            .unwrap_err();
        assert!(
            format!("{e:#}").contains("non e' il PC dell'invito"),
            "{e:#}"
        );
        assert!(tls::collega("127.0.0.1", porta, &finto.impronta())
            .await
            .is_ok());
        let _ = std::fs::remove_dir_all(dir);
    }
}
