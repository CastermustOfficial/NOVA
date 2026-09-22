//! Il vault, dal lato del demone: cosa NOVA sa gia' di chi le sta parlando.
//!
//! La memoria di NOVA e' una cartella di `.md` che si apre in Obsidian, un
//! nodo per file, con le relazioni scritte dentro. Il giudizio — chi entra
//! nel contesto, in che ordine, e perche' — sta tutto nei crate portati dal
//! Python e confrontati da un banco: `nova-nodi` per la forma su disco,
//! `nova-memoria` per BM25, l'embedding di casa, la fusione e la scelta.
//!
//! Qui c'e' quello che quei crate non fanno apposta: **aprire la cartella**,
//! tenerla in memoria fra un turno e l'altro, e rileggere solo i file che
//! sono cambiati.
//!
//! Il vault e' lo stesso del Python, per la terza volta nello stesso
//! progetto e per la stessa ragione del registro delle azioni e delle
//! procedure: due memorie sono due NOVA che sanno cose diverse della stessa
//! persona, e chi parla non ha modo di sapere con quale delle due.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use nova_memoria::scelta::{scegli, Candidato, Via};
use nova_memoria::{vettore::vettore, Bm25, Nodo as NodoIndice, Pezzo};
use nova_nodi::deposito::{Deposito, DiscoScrivibile, GuardianoDeiSegreti};
use nova_nodi::disco_vero::Cartella;
use nova_nodi::Nodo;
use serde_json::Value;

/// Quanti ricordi si consegnano, se la configurazione non lo dice.
pub const QUANTI: usize = 5;

/// Sotto questa confidenza un nodo non entra, a meno che la domanda non lo
/// nomini per nome.
pub const CONFIDENZA_MINIMA: f64 = 0.25;

/// Il vault aperto, con l'indice gia' costruito.
///
/// Sta dentro un `Mutex` e non dentro la sessione perche' il vault e' **uno
/// solo** per computer: due conversazioni aperte insieme leggono la stessa
/// memoria, e tenerne due copie vorrebbe dire rileggere la cartella due
/// volte per sapere la stessa cosa.
#[derive(Default)]
pub struct Memoria {
    dentro: Mutex<Option<Aperta>>,
}

/// Un nodo come lo legge chi ha cercato.
#[derive(Debug, Clone)]
pub struct Trovato {
    pub slug: String,
    pub titolo: String,
    pub tipo: String,
    pub confidenza: f64,
    pub corpo: String,
    pub relazioni: Vec<String>,
    /// Come ci si e' arrivati: `esatto`, `fusione`, `grafo`.
    pub via: String,
}

struct Aperta {
    radice: PathBuf,
    deposito: Deposito,
    indice: Bm25,
    /// slug -> vettore del nodo. Si ricalcola quando il nodo cambia.
    vettori: BTreeMap<String, Vec<f64>>,
}

/// Dove sta il vault, secondo la configurazione di NOVA.
///
/// `kb.vault_path` se c'e'; se no la cartella `vault` accanto al progetto,
/// che e' dove la mette l'installatore. La stessa regola di `kb_setup`.
pub fn percorso(cfg: &Value, radice_progetto: &Path) -> PathBuf {
    let scritto = cfg
        .get("kb")
        .and_then(|k| k.get("vault_path"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if scritto.is_empty() {
        radice_progetto.join("vault")
    } else {
        PathBuf::from(scritto)
    }
}

/// La radice del progetto, vista dal demone.
///
/// L'eseguibile sta in `bin/` o in `core/target/<profilo>/`: si sale finche'
/// non si trova una cartella che contiene `nova/`, che e' il segno del
/// progetto. Se non si trova niente, la cartella di lavoro — che e' quel che
/// faceva il Python prima di avere un'installazione vera.
pub fn radice_progetto() -> PathBuf {
    let mut p = std::env::current_exe().unwrap_or_default();
    while p.pop() {
        if p.join("nova").join("agent.py").is_file() {
            return p;
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

impl Memoria {
    /// Apre il vault se serve, e rilegge cio' che e' cambiato.
    ///
    /// Rileggere **solo i file cambiati** non e' un'ottimizzazione: un vault
    /// di mille note riletto a ogni turno sono mille file aperti mentre
    /// qualcuno aspetta una risposta.
    fn prepara(&self, radice: &Path) {
        let mut dentro = self.dentro.lock().unwrap();
        let cartella = Cartella::nuova(radice);
        let da_capo = match dentro.as_ref() {
            Some(a) => a.radice != radice,
            None => true,
        };
        if da_capo {
            let mut deposito = Deposito::nuovo(cfg_distingue_maiuscole());
            deposito.ricarica(&cartella);
            let mut a = Aperta {
                radice: radice.to_path_buf(),
                deposito,
                indice: Bm25::nuovo(),
                vettori: BTreeMap::new(),
            };
            reindicizza(&mut a);
            *dentro = Some(a);
            return;
        }
        let a = dentro.as_mut().expect("c'e'");
        let cambiati = a.deposito.aggiorna(&cartella);
        if !cambiati.letti.is_empty() || !cambiati.spariti.is_empty() {
            reindicizza(a);
        }
    }

    /// Cio' che la memoria ha da dire su questa domanda, gia' impaginato.
    ///
    /// Stringa vuota se non c'e' niente: una cornice attorno al nulla costa
    /// token e dice al modello che la memoria e' vuota, che e' un'altra cosa
    /// dal non averla interrogata.
    pub fn contesto_per(&self, domanda: &str, cfg: &Value) -> String {
        let radice = percorso(cfg, &radice_progetto());
        if !radice.is_dir() {
            return String::new();
        }
        self.prepara(&radice);
        let dentro = self.dentro.lock().unwrap();
        let Some(a) = dentro.as_ref() else {
            return String::new();
        };
        if a.deposito.quanti() == 0 {
            return String::new();
        }
        let quanti = cfg
            .get("kb")
            .and_then(|k| k.get("top_k"))
            .and_then(Value::as_u64)
            .unwrap_or(QUANTI as u64) as usize;
        let massimo = cfg
            .get("kb")
            .and_then(|k| k.get("max_context_chars"))
            .and_then(Value::as_u64)
            .unwrap_or(nova_memoria::MAX_CONTESTO as u64) as usize;
        let confidenza = cfg
            .get("kb")
            .and_then(|k| k.get("min_confidence"))
            .and_then(Value::as_f64)
            .unwrap_or(CONFIDENZA_MINIMA);

        let candidati: BTreeMap<String, Candidato> = a
            .deposito
            .attivi()
            .map(|n| {
                (
                    n.slug.clone(),
                    Candidato {
                        slug: n.slug.clone(),
                        titolo: n.title.clone(),
                        tag: n.tags.clone(),
                        tipo: n.tipo.clone(),
                        confidenza: n.confidenza,
                        aggiornato: n.aggiornato.clone(),
                    },
                )
            })
            .collect();
        let sparsi: BTreeMap<String, f64> = a.indice.cerca(domanda);
        let v_domanda = vettore(domanda);
        let densi: BTreeMap<String, f64> = a
            .vettori
            .iter()
            .filter_map(|(slug, v)| {
                let s = nova_memoria::coseno(&v_domanda, v);
                // La stessa soglia del Python: sotto, e' rumore
                // dell'hashing, non somiglianza.
                (s > 0.05).then(|| (slug.clone(), s))
            })
            .collect();
        let vicini = |slug: &str| -> Vec<String> {
            a.deposito
                .vicini(slug)
                .into_iter()
                .map(|n| n.slug.clone())
                .collect()
        };
        let (scelti, _resoconto) = scegli(
            domanda, &candidati, &sparsi, &densi, &vicini, quanti, confidenza, true,
        );
        let pezzi: Vec<Pezzo> = scelti
            .iter()
            .filter_map(|s| a.deposito.prendi(&s.slug))
            .map(|n| Pezzo {
                titolo: n.title.clone(),
                tipo: n.tipo.clone(),
                confidenza: n.confidenza,
                corpo: n.body.clone(),
            })
            .collect();
        let _ = Via::Esatto; // il perche' di ogni scelta serve all'audit, non qui
        nova_memoria::come_contesto(&pezzi, massimo)
    }

    /// Cosa la memoria sa di questa domanda, un nodo per riga.
    ///
    /// E' la stessa ricerca di [`Memoria::contesto_per`] — stesso indice,
    /// stessa fusione, stessa scelta — ma qui i nodi tornano interi invece che
    /// impaginati: chi chiede e' il modello, che vuole leggerli, non il
    /// compositore del prompt, che vuole un blocco della misura giusta.
    pub fn cerca(&self, domanda: &str, quanti: usize, cfg: &Value) -> Vec<Trovato> {
        let radice = percorso(cfg, &radice_progetto());
        if !radice.is_dir() {
            return Vec::new();
        }
        self.prepara(&radice);
        let dentro = self.dentro.lock().unwrap();
        let Some(a) = dentro.as_ref() else {
            return Vec::new();
        };
        // Senza soglia di confidenza e senza espansione del grafo: chi cerca a
        // mano vuole vedere cosa c'e', non il sottoinsieme che entrerebbe in un
        // prompt. La soglia serve a non sprecare contesto, e qui non c'e'
        // contesto da sprecare.
        let (scelti, _) = scegli(
            domanda,
            &self.candidati(a),
            &a.indice.cerca(domanda),
            &self.densi(a, domanda),
            &|slug: &str| {
                a.deposito
                    .vicini(slug)
                    .into_iter()
                    .map(|n| n.slug.clone())
                    .collect()
            },
            quanti.clamp(1, 12),
            0.0,
            false,
        );
        scelti
            .iter()
            .filter_map(|s| a.deposito.prendi(&s.slug).map(|n| (s, n)))
            .map(|(s, n)| Trovato {
                slug: n.slug.clone(),
                titolo: n.title.clone(),
                tipo: n.tipo.clone(),
                confidenza: n.confidenza,
                corpo: n.body.clone(),
                relazioni: n.tutte_le_relazioni(),
                via: match s.via {
                    Via::Esatto => "esatto",
                    Via::Fusione => "fusione",
                    Via::Grafo => "grafo",
                }
                .to_string(),
            })
            .collect()
    }

    fn candidati(&self, a: &Aperta) -> BTreeMap<String, Candidato> {
        a.deposito
            .attivi()
            .map(|n| {
                (
                    n.slug.clone(),
                    Candidato {
                        slug: n.slug.clone(),
                        titolo: n.title.clone(),
                        tag: n.tags.clone(),
                        tipo: n.tipo.clone(),
                        confidenza: n.confidenza,
                        aggiornato: n.aggiornato.clone(),
                    },
                )
            })
            .collect()
    }

    fn densi(&self, a: &Aperta, domanda: &str) -> BTreeMap<String, f64> {
        let v = vettore(domanda);
        a.vettori
            .iter()
            .filter_map(|(slug, x)| {
                let s = nova_memoria::coseno(&v, x);
                (s > 0.05).then(|| (slug.clone(), s))
            })
            .collect()
    }

    /// Scrive un nodo nel vault, e reindicizza.
    ///
    /// Il guardiano dei segreti sta **dentro la porta**, non nel giudizio di
    /// chi chiama (D106, D110): quel che entra qui viene riletto in ogni
    /// conversazione futura, comprese quelle in cui NOVA legge testo scritto
    /// da altri, e una credenziale li' dentro e' esposta per sempre.
    pub fn salva(&self, cfg: &Value, nodo: Nodo, unisci: bool) -> Result<Nodo, String> {
        self.con_il_disco(cfg, |a, cartella, oggi, _| {
            let salvato = a
                .deposito
                .salva(cartella, &GuardianoDeiSegreti, nodo, unisci, &oggi)?;
            Ok(salvato)
        })
    }

    /// Archivia un nodo: non entra piu' nelle risposte, il file resta.
    pub fn archivia(&self, cfg: &Value, chi: &str, motivo: &str) -> Result<bool, String> {
        let motivo = motivo.to_string();
        self.con_il_disco(cfg, move |a, cartella, oggi, italiano| {
            let Some(nodo) = a.deposito.per_titolo(chi).map(|n| n.slug.clone()) else {
                return Ok(false);
            };
            Ok(a.deposito
                .archivia(cartella, &nodo, &motivo, &oggi, &italiano))
        })
    }

    /// Collega due nodi. Il grafo non e' orientato: il legame vale nei due
    /// versi, e si scrive su uno solo perche' `vicini` guarda da tutt'e due.
    pub fn collega(&self, cfg: &Value, da: &str, a_chi: &str) -> Result<(String, String), String> {
        self.con_il_disco(cfg, |a, cartella, oggi, _| {
            let Some(primo) = a.deposito.per_titolo(da).cloned() else {
                return Err(format!("il nodo «{da}» non c'e'"));
            };
            let Some(secondo) = a.deposito.per_titolo(a_chi).cloned() else {
                return Err(format!("il nodo «{a_chi}» non c'e'"));
            };
            if primo.slug == secondo.slug {
                return Err("un nodo non si collega a se stesso".into());
            }
            let mut nuovo = primo.clone();
            if !nuovo.tutte_le_relazioni().contains(&secondo.slug) {
                nuovo.relazioni.push(secondo.slug.clone());
            }
            a.deposito
                .salva(cartella, &GuardianoDeiSegreti, nuovo, false, &oggi)?;
            Ok((primo.slug, secondo.slug))
        })
    }

    /// I nodi direttamente collegati a uno, per esplorare il grafo.
    pub fn vicini(&self, cfg: &Value, chi: &str) -> Option<(String, String, Vec<Trovato>)> {
        let radice = percorso(cfg, &radice_progetto());
        if !radice.is_dir() {
            return None;
        }
        self.prepara(&radice);
        let dentro = self.dentro.lock().unwrap();
        let a = dentro.as_ref()?;
        let nodo = a.deposito.per_titolo(chi)?;
        let vicini = a
            .deposito
            .vicini(&nodo.slug)
            .into_iter()
            .map(|n| Trovato {
                slug: n.slug.clone(),
                titolo: n.title.clone(),
                tipo: n.tipo.clone(),
                confidenza: n.confidenza,
                corpo: String::new(),
                relazioni: Vec::new(),
                via: "grafo".into(),
            })
            .collect();
        Some((nodo.slug.clone(), nodo.title.clone(), vicini))
    }

    /// Lo stato della memoria: quanti nodi, di che tipo, quanti legami.
    pub fn statistiche(&self, cfg: &Value) -> Option<nova_nodi::deposito::Statistiche> {
        let radice = percorso(cfg, &radice_progetto());
        if !radice.is_dir() {
            return None;
        }
        self.prepara(&radice);
        let dentro = self.dentro.lock().unwrap();
        Some(dentro.as_ref()?.deposito.statistiche())
    }

    /// Il giro comune a tutto cio' che **scrive**: apri, fai, riscrivi
    /// l'indice, reindicizza.
    ///
    /// Sta in un posto solo perche' le tre cose dopo sono facili da
    /// dimenticare, e dimenticarne una lascia la memoria che risponde con
    /// quel che sapeva prima — cioe' un difetto che si vede un turno dopo, e
    /// altrove.
    fn con_il_disco<T>(
        &self,
        cfg: &Value,
        fai: impl FnOnce(&mut Aperta, &Cartella, String, String) -> Result<T, String>,
    ) -> Result<T, String> {
        let radice = percorso(cfg, &radice_progetto());
        if !radice.is_dir() {
            return Err(format!(
                "la memoria non c'e': «{}» non e' una cartella. Controlla `kb.vault_path` nella configurazione.",
                radice.display()
            ));
        }
        self.prepara(&radice);
        let mut dentro = self.dentro.lock().unwrap();
        let a = dentro.as_mut().ok_or("la memoria non si e' aperta")?;
        let cartella = Cartella::nuova(&radice);
        // La stessa data che scrive il registro delle azioni: una memoria
        // datata in un fuso e un registro in un altro raccontano due giornate
        // diverse dello stesso pomeriggio.
        let oggi = crate::registro::oggi();
        let italiano = nova_registro::data_italiana(&oggi);
        let esito = fai(a, &cartella, oggi.clone(), italiano)?;
        let indice = a.deposito.indice(&oggi);
        let _ = DiscoScrivibile::scrivi(&cartella, Deposito::dove_va_lindice(), &indice);
        reindicizza(a);
        Ok(esito)
    }

    /// Quanti nodi sono in memoria adesso. Zero anche quando non e' mai
    /// stata aperta: e' cio' che si vuole sapere davvero.
    pub fn quanti(&self) -> usize {
        self.dentro
            .lock()
            .unwrap()
            .as_ref()
            .map_or(0, |a| a.deposito.quanti())
    }
}

/// Su questo sistema due file che differiscono solo per le maiuscole sono lo
/// stesso file?
fn cfg_distingue_maiuscole() -> bool {
    !cfg!(windows) && !cfg!(target_os = "macos")
}

fn reindicizza(a: &mut Aperta) {
    let nodi: Vec<NodoIndice> = a
        .deposito
        .attivi()
        .map(|n| NodoIndice {
            slug: n.slug.clone(),
            titolo: n.title.clone(),
            tag: n.tags.clone(),
            corpo: n.body.clone(),
        })
        .collect();
    a.indice = Bm25::nuovo();
    a.indice.indicizza(&nodi);
    a.vettori = nodi
        .iter()
        .map(|n| (n.slug.clone(), vettore(&nova_memoria::testo_pesato(n))))
        .collect();
}

#[cfg(test)]
mod prove {
    use super::*;
    use serde_json::json;

    fn vault(nome: &str, note: &[(&str, &str)]) -> PathBuf {
        let p = std::env::temp_dir().join(format!("nova-vault-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        for (file, testo) in note {
            std::fs::write(p.join(file), testo).unwrap();
        }
        p
    }

    fn nota(titolo: &str, tipo: &str, confidenza: f64, tag: &str, corpo: &str) -> String {
        format!(
            "---\ntitle: {titolo}\ntipo: {tipo}\nconfidenza: {confidenza}\ntags: {tag}\n\
             aggiornato: 2026-09-01\n---\n\n{corpo}\n"
        )
    }

    #[test]
    fn un_vault_che_non_ce_non_da_contesto() {
        let m = Memoria::default();
        let cfg = json!({ "kb": { "vault_path": "/questo/vault/non/esiste" } });
        assert_eq!(m.contesto_per("posta", &cfg), "");
        assert_eq!(m.quanti(), 0);
    }

    #[test]
    fn quel_che_si_sa_torna_nel_contesto() {
        let radice = vault(
            "posta",
            &[
                (
                    "posta.md",
                    &nota(
                        "Come guardo la posta",
                        "abitudine",
                        0.9,
                        "posta",
                        "Ogni mattina apro Gmail e leggo le non lette.",
                    ),
                ),
                (
                    "carbonara.md",
                    &nota(
                        "Carbonara",
                        "fatto",
                        0.8,
                        "cucina",
                        "Guanciale, uovo, pecorino.",
                    ),
                ),
            ],
        );
        let m = Memoria::default();
        let cfg = json!({ "kb": { "vault_path": radice.to_string_lossy() } });
        let contesto = m.contesto_per("come guardo la posta", &cfg);
        assert!(contesto.contains("Come guardo la posta"), "{contesto}");
        assert!(contesto.contains("Gmail"), "{contesto}");
        assert!(contesto.starts_with("### "), "{contesto}");
        assert!(contesto.contains("confidenza 0.9"), "{contesto}");
        let _ = std::fs::remove_dir_all(&radice);
    }

    #[test]
    fn una_nota_aggiunta_dopo_si_vede_senza_riavviare() {
        // Il vault e' una cartella che l'utente apre in Obsidian: se una nota
        // scritta a mano si vedesse solo al riavvio del demone, la memoria
        // sarebbe quella di ieri.
        let radice = vault(
            "nuova",
            &[(
                "uno.md",
                &nota("Il computer", "fatto", 0.9, "pc", "RTX 4060 Ti."),
            )],
        );
        let m = Memoria::default();
        let cfg = json!({ "kb": { "vault_path": radice.to_string_lossy() } });
        assert!(m.contesto_per("computer", &cfg).contains("RTX"));
        std::fs::write(
            radice.join("due.md"),
            nota(
                "La stampante",
                "fatto",
                0.9,
                "stampante",
                "E' una Brother in corridoio.",
            ),
        )
        .unwrap();
        let dopo = m.contesto_per("stampante", &cfg);
        assert!(dopo.contains("Brother"), "{dopo}");
        let _ = std::fs::remove_dir_all(&radice);
    }

    #[test]
    fn il_tetto_dei_caratteri_si_rispetta() {
        let lunga = "parola ".repeat(2000);
        let radice = vault(
            "tetto",
            &[
                ("a.md", &nota("Prima", "fatto", 0.9, "prova", &lunga)),
                ("b.md", &nota("Seconda", "fatto", 0.9, "prova", &lunga)),
                ("c.md", &nota("Terza", "fatto", 0.9, "prova", &lunga)),
            ],
        );
        let m = Memoria::default();
        let cfg = json!({
            "kb": { "vault_path": radice.to_string_lossy(), "max_context_chars": 1200 }
        });
        let contesto = m.contesto_per("parola", &cfg);
        assert!(
            contesto.chars().count() <= 1200,
            "{}",
            contesto.chars().count()
        );
        assert!(!contesto.is_empty(), "un tetto non e' un divieto");
        let _ = std::fs::remove_dir_all(&radice);
    }

    #[test]
    fn la_cartella_del_vault_si_legge_dalla_configurazione() {
        let p = percorso(
            &json!({ "kb": { "vault_path": "/altrove/note" } }),
            Path::new("/x"),
        );
        assert_eq!(p, PathBuf::from("/altrove/note"));
        let d = percorso(&json!({}), Path::new("/x"));
        assert_eq!(d, PathBuf::from("/x").join("vault"));
    }
}
