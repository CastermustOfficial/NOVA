//! Accendere il modello locale: il **giro**, non la decisione.
//!
//! La decisione — cosa si fa quando non parte, e quanto si aspetta — sta in
//! `nova_modelli::avvio`, provata da undici prove e confrontata col Python da
//! un banco (D213, D214). Qui c'e' solo cio' che quella decisione non puo'
//! fare da sola: avviare un processo, chiedergli se e' vivo, leggerne il
//! registro, spegnerlo.
//!
//! Sta nel demone e non nell'interfaccia perche' il demone **possiede** i
//! processi lunghi: se la finestra muore il modello resta caricato. Finora il
//! giro lo guidava Python — costruiva gli argomenti, chiedeva `proc.spawn`,
//! aspettava, rileggeva i registri, decideva. Il demone faceva da braccio
//! senza sapere cosa stava facendo, e chi voleva accendere il modello doveva
//! riscrivere tutta quella sequenza. Adesso si chiede una volta sola.
//!
//! Il giro e' separato da **chi prova**: `Tentativo` e' l'unica cosa che
//! tocca il mondo, e il resto si prova con un tentativo finto che risponde
//! cio' che si vuole. Senza quella cucitura, provare «tre gradini e poi si
//! arrende» vorrebbe dire avere una scheda video che finisce la memoria a
//! comando.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use nova_modelli::avvio::{self, Esito, Prossimo};

use crate::supervisor::{ChildSpec, Supervisor};

/// Il nome con cui il modello locale vive fra i processi del demone.
///
/// Uno solo, e scritto qui: chi lo ferma e chi ne legge il registro devono
/// dire lo stesso nome di chi lo ha avviato.
pub const NOME: &str = "modello";

/// Quante righe di registro si guardano per capire perche' non e' partito.
///
/// llama.cpp ne stampa centinaia all'avvio; il motivo, quando c'e', sta in
/// fondo.
pub const RIGHE_DA_LEGGERE: usize = 200;

/// Cosa si vuole accendere.
pub struct Richiesta {
    pub binario: String,
    pub impostazioni: avvio::Impostazioni,
    /// I gradini da provare, dal piu' ambizioso al piu' prudente.
    pub scala: Vec<i64>,
    pub proiettore: Option<String>,
    /// Se si puo' scendere di gradino. Falso = un tentativo solo.
    pub auto: bool,
    /// L'attesa del **primo** tentativo. Gli altri la accorciano da se'.
    pub attesa_s: u64,
}

/// Com'e' andata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Acceso {
    pub ngl: i64,
    /// Quanti tentativi sono serviti prima di questo. Zero = al primo colpo.
    pub tentativi: usize,
}

/// Chi prova davvero ad accendere un gradino.
///
/// L'unica cosa di questo modulo che tocca il mondo. Il giro qui sotto non sa
/// se dietro c'e' un processo vero o una finzione, ed e' precisamente cio'
/// che lo rende provabile.
#[async_trait]
pub trait Tentativo: Send + Sync {
    /// Avvia con questo numero di layer e aspetta. Torna com'e' finita e il
    /// registro da leggere.
    async fn prova(&self, ngl: i64, attesa_s: u64) -> (Esito, String);
    /// Spegne cio' che il tentativo ha lasciato in piedi.
    async fn spegni(&self);
}

/// Il giro: si scende di gradino finche' la regola dice di scendere.
///
/// Non decide niente da se'. Ogni volta che un tentativo finisce male chiede
/// a `dopo_un_tentativo`, che e' la stessa funzione che risponde al Python.
pub async fn accendi(r: &Richiesta, chi: &dyn Tentativo) -> Result<Acceso> {
    if r.scala.is_empty() {
        return Err(anyhow!("nessun gradino da provare: la scala e' vuota"));
    }
    let ultimo_indice = r.scala.len() - 1;
    let mut perche = String::new();
    for (i, ngl) in r.scala.iter().enumerate() {
        let attesa = avvio::attesa_del_tentativo(i == 0, r.attesa_s);
        let (esito, coda) = chi.prova(*ngl, attesa).await;
        if esito == Esito::Pronto {
            return Ok(Acceso { ngl: *ngl, tentativi: i });
        }
        // Spento **prima** di decidere: se si scende di un gradino, quello di
        // sopra non deve restare a tenersi la memoria che serve al prossimo.
        chi.spegni().await;
        match avvio::dopo_un_tentativo(esito, &coda, r.auto, i < ultimo_indice) {
            Prossimo::Acceso => return Ok(Acceso { ngl: *ngl, tentativi: i }),
            Prossimo::Arrenditi(p) => {
                return Err(anyhow!("il modello non e' partito: {p}"));
            }
            Prossimo::Riprova(p) => {
                tracing::info!(ngl, motivo = %p, "scendo di un gradino");
                perche = p;
            }
        }
    }
    // La scala e' finita senza che nessuno si arrendesse: capita solo se
    // l'ultimo gradino ha detto «riprova», e sotto non c'e' piu' niente.
    Err(anyhow!("il modello non e' partito, e la scala e' finita: {perche}"))
}

/// Il tentativo vero: il supervisore del demone avvia, e si chiede al server
/// se e' pronto.
pub struct ColSupervisore<'a> {
    pub sup: &'a Arc<Supervisor>,
    pub binario: String,
    pub impostazioni: avvio::Impostazioni,
    pub proiettore: Option<String>,
}

#[async_trait]
impl Tentativo for ColSupervisore<'_> {
    async fn prova(&self, ngl: i64, attesa_s: u64) -> (Esito, String) {
        let pro = self.proiettore.as_ref().map(Path::new);
        let riga = avvio::argomenti(
            Path::new(&self.binario),
            &self.impostazioni,
            ngl,
            pro,
        );
        let cartella = Path::new(&self.binario)
            .parent()
            .map(|p| p.display().to_string());
        let spec = ChildSpec {
            name: NOME.to_string(),
            program: riga[0].clone(),
            args: riga[1..].to_vec(),
            cwd: cartella,
            // Niente riavvio automatico: qui il riavvio lo decide la scala,
            // e due meccanismi che rialzano lo stesso processo litigherebbero
            // — il supervisore lo rimetterebbe su con i layer che avevano
            // appena fallito.
            restart: false,
            capture_output: true,
        };
        if let Err(e) = self.sup.spawn(spec).await {
            return (Esito::Morto, format!("error: non e' partito affatto: {e}"));
        }
        let esito = self.aspetta(attesa_s).await;
        let coda = self.sup.logs(NOME, RIGHE_DA_LEGGERE).await.join("\n");
        (esito, coda)
    }

    async fn spegni(&self) {
        let _ = self.sup.stop(NOME).await;
    }
}

impl ColSupervisore<'_> {
    fn indirizzo(&self) -> String {
        let host = if self.impostazioni.host.is_empty() {
            "127.0.0.1"
        } else {
            &self.impostazioni.host
        };
        // `0.0.0.0` vuol dire «ascolto ovunque», non e' un posto a cui
        // chiedere: a se stessi si bussa da 127.0.0.1.
        let host = if host == "0.0.0.0" { "127.0.0.1" } else { host };
        format!("http://{host}:{}/health", self.impostazioni.porta)
    }

    async fn aspetta(&self, attesa_s: u64) -> Esito {
        let fine = Instant::now() + Duration::from_secs(attesa_s);
        let url = self.indirizzo();
        while Instant::now() < fine {
            if !self.e_vivo().await {
                return Esito::Morto;
            }
            if salute(&url).await {
                return Esito::Pronto;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        Esito::Scaduto
    }

    async fn e_vivo(&self) -> bool {
        self.sup
            .status()
            .await
            .iter()
            .any(|c| c.name == NOME && c.running)
    }
}

/// Se il server risponde sulla sua porta.
///
/// Con `ureq`, che e' il cliente HTTP di casa — `nova-voce` e `nova-cervelli`
/// parlano gia' con quello, e un secondo cliente sarebbe un secondo
/// comportamento da conoscere. E' bloccante, quindi gira dove il blocco non
/// ferma nessuno.
pub async fn salute(url: &str) -> bool {
    let url = url.to_string();
    tokio::task::spawn_blocking(move || {
        ureq::get(&url)
            .timeout(Duration::from_millis(1500))
            .call()
            .map(|r| r.status() == 200)
            .unwrap_or(false)
    })
    .await
    .unwrap_or(false)
}

#[cfg(test)]
mod prove {
    use super::*;
    use std::sync::Mutex;

    /// Un tentativo che risponde cio' che gli si dice, e si ricorda cosa gli
    /// e' stato chiesto.
    struct Finto {
        risposte: Mutex<Vec<(Esito, String)>>,
        chiesti: Mutex<Vec<(i64, u64)>>,
        spegnimenti: Mutex<usize>,
    }

    impl Finto {
        fn con(risposte: Vec<(Esito, &str)>) -> Self {
            Finto {
                risposte: Mutex::new(
                    risposte.into_iter().map(|(e, c)| (e, c.to_string())).collect(),
                ),
                chiesti: Mutex::new(Vec::new()),
                spegnimenti: Mutex::new(0),
            }
        }
    }

    #[async_trait]
    impl Tentativo for Finto {
        async fn prova(&self, ngl: i64, attesa_s: u64) -> (Esito, String) {
            self.chiesti.lock().unwrap().push((ngl, attesa_s));
            let mut r = self.risposte.lock().unwrap();
            if r.is_empty() {
                (Esito::Scaduto, String::new())
            } else {
                r.remove(0)
            }
        }
        async fn spegni(&self) {
            *self.spegnimenti.lock().unwrap() += 1;
        }
    }

    fn richiesta(scala: Vec<i64>, auto: bool) -> Richiesta {
        Richiesta {
            binario: "llama-server".into(),
            impostazioni: avvio::Impostazioni::default(),
            scala,
            proiettore: None,
            auto,
            attesa_s: 600,
        }
    }

    #[tokio::test]
    async fn al_primo_colpo_non_si_spegne_niente() {
        let f = Finto::con(vec![(Esito::Pronto, "")]);
        let a = accendi(&richiesta(vec![30, 24, 18], true), &f).await.unwrap();
        assert_eq!(a, Acceso { ngl: 30, tentativi: 0 });
        assert_eq!(*f.spegnimenti.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn la_memoria_finita_fa_scendere_e_dice_con_quale_gradino_ce_lha_fatta() {
        let f = Finto::con(vec![
            (Esito::Morto, "ggml: out of memory"),
            (Esito::Morto, "ggml: out of memory"),
            (Esito::Pronto, ""),
        ]);
        let a = accendi(&richiesta(vec![30, 24, 18], true), &f).await.unwrap();
        assert_eq!(a, Acceso { ngl: 18, tentativi: 2 });
        // Due gradini falliti, due spegnimenti: chi non ce l'ha fatta non
        // resta acceso a tenersi la memoria del prossimo.
        assert_eq!(*f.spegnimenti.lock().unwrap(), 2);
    }

    #[tokio::test]
    async fn un_errore_che_non_e_memoria_ferma_tutto_al_primo_tentativo() {
        // Il difetto che CANT-4 e' andato a curare: qui prima si sarebbero
        // provati tutti e sei i gradini, dicendo «memoria» ogni volta.
        let f = Finto::con(vec![
            (Esito::Morto, "error: unknown argument: --pippo"),
            (Esito::Pronto, ""),
        ]);
        let e = accendi(&richiesta(vec![30, 24, 18], true), &f).await.unwrap_err();
        assert!(e.to_string().contains("unknown argument"), "{e}");
        assert!(!e.to_string().to_lowercase().contains("memoria"), "{e}");
        assert_eq!(f.chiesti.lock().unwrap().len(), 1, "ha provato piu' di una volta");
    }

    #[tokio::test]
    async fn la_prima_attesa_e_lunga_e_le_altre_no() {
        let f = Finto::con(vec![
            (Esito::Morto, ""),
            (Esito::Morto, ""),
            (Esito::Pronto, ""),
        ]);
        accendi(&richiesta(vec![30, 24, 18], true), &f).await.unwrap();
        let chiesti = f.chiesti.lock().unwrap().clone();
        assert_eq!(chiesti[0], (30, 600));
        assert_eq!(chiesti[1], (24, avvio::ATTESA_DOPO_IL_PRIMO_S));
        assert_eq!(chiesti[2], (18, avvio::ATTESA_DOPO_IL_PRIMO_S));
    }

    #[tokio::test]
    async fn senza_auto_si_prova_una_volta_sola() {
        let f = Finto::con(vec![(Esito::Morto, "ggml: out of memory")]);
        let e = accendi(&richiesta(vec![30, 24, 18], false), &f).await.unwrap_err();
        assert_eq!(f.chiesti.lock().unwrap().len(), 1, "{e}");
    }

    #[tokio::test]
    async fn finita_la_scala_si_dice_che_e_finita() {
        let f = Finto::con(vec![(Esito::Morto, ""), (Esito::Morto, "")]);
        let e = accendi(&richiesta(vec![6, 0], true), &f).await.unwrap_err();
        assert!(e.to_string().contains("non e' partito"), "{e}");
        assert_eq!(f.chiesti.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn una_scala_vuota_non_e_un_giro_riuscito() {
        let f = Finto::con(vec![]);
        let e = accendi(&richiesta(vec![], true), &f).await.unwrap_err();
        assert!(e.to_string().contains("scala e' vuota"), "{e}");
    }
}
