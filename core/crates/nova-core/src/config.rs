//! Configurazione del demone. JSON, accanto a quella di NOVA lato Python.
//!
//! **Le regole di lettura non sono scritte qui**: sono quelle di
//! `nova-configurazione`, le stesse che valgono per il `config.json` di
//! NOVA. Prima erano due, e quella di qui era piu' povera in tre modi che
//! costavano caro:
//!
//! - un campo solo scritto male faceva fallire la lettura di **tutto** il
//!   file, e si tornava ai predefiniti — cioe' `write_roots` vuoto, cioe'
//!   nessun confinamento delle scritture, senza che nessuno lo dicesse
//!   (D285);
//! - `protected_paths` si lasciava sostituire da un file salvato, mentre
//!   `forbidden_commands` si univa gia' ai predefiniti: due guardie nello
//!   stesso file con due comportamenti diversi (D185);
//! - l'unico avviso che diceva «ho ignorato la tua configurazione» era un
//!   `tracing::warn!` emesso **prima** che il logger esistesse, quindi non
//!   lo leggeva nessuno, mai (D286).

use std::path::{Path, PathBuf};

use nova_configurazione::{Guardie, Regole};
use nova_strumenti::predefiniti;
use serde::{Deserialize, Serialize};

/// Le liste del demone che si uniscono ai predefiniti invece di farsi
/// sostituire. Stanno in cima e non dentro una sezione, a differenza di
/// quelle di NOVA — la regola e' la stessa, i nomi no.
pub const GUARDIE: [&str; 2] = ["protected_paths", "forbidden_commands"];

/// I campi in cui il vuoto non e' una scelta ma una perdita.
///
/// Si chiama cosi' e non `NON_SI_SVUOTA` perche' quel nome e' gia' quello di
/// `nova-configurazione`, dove vuol dire il prompt di sistema di NOVA: due
/// cose diverse con lo stesso nome sono due cose che prima o poi qualcuno
/// confonde (D225).
///
/// `write_roots` **non** sta qui: vuoto vuol dire «non confinare», ed e' una
/// scelta che qualcuno puo' fare davvero. Gli altri tre no — un demone senza
/// endpoint non si raggiunge, senza autonomia non sa cosa chiedere, senza
/// livello di log non scrive niente.
pub const SENZA_QUESTI_NON_PARTE: [&str; 3] = ["endpoint", "autonomy", "log_level"];

/// Come si legge il file del demone.
pub const REGOLE: Regole<'static> = Regole {
    guardie: Guardie {
        sezione: None,
        campi: &GUARDIE,
    },
    non_si_svuota: &SENZA_QUESTI_NON_PARTE,
    // Acceso, e qui e' la differenza che conta rispetto a NOVA in Python:
    // questa configurazione finisce dentro una struttura tipata, e un campo
    // di un tipo sbagliato la farebbe fallire tutta.
    tipi_fermi: true,
};

/// Livelli di autonomia: gli stessi nomi usati da NOVA in Python.
///
/// Non riscritti: vengono da `nova_strumenti::predefiniti`, che li estrae da
/// `nova/config.py`. Erano tre stringhe dichiarate qui a mano, ed era il
/// posto in cui prima o poi una delle due copie sarebbe cambiata da sola.
pub const AUTONOMY_ASK_ALL: &str = predefiniti::LIVELLI[0];
pub const AUTONOMY_ASK_RISKY: &str = predefiniti::LIVELLI[1];
pub const AUTONOMY_FULL: &str = predefiniti::LIVELLI[2];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Named pipe su Windows, socket unix altrove.
    pub endpoint: String,
    pub autonomy: String,
    /// Percorsi mai scrivibili, qualunque cosa dica il modello.
    pub protected_paths: Vec<String>,
    /// Se valorizzato, le scritture sono confinate qui dentro.
    pub write_roots: Vec<String>,
    /// Sottostringhe vietate nei comandi di shell.
    pub forbidden_commands: Vec<String>,
    pub shell_timeout_s: u64,
    /// Processi da avviare all'accensione del demone.
    pub services: Vec<ServiceSpec>,
    pub log_level: String,

    /// Vuoto se il file si e' letto. Altrimenti **perche'** e' stato
    /// ignorato. Non si serializza: e' diagnostica, e un file salvato non
    /// deve poter raccontare al demone di aver avuto un errore che non ha
    /// avuto.
    #[serde(skip)]
    pub errore_caricamento: String,
    /// I campi del file che non si sono potuti usare, e sono rimasti di
    /// fabbrica.
    #[serde(skip)]
    pub campi_ignorati: Vec<String>,
    /// Le guardie di fabbrica rimesse nell'elenco, per campo. Aggiungere
    /// qualcosa alla configurazione di qualcuno senza dirlo e' l'altro modo
    /// di sbagliare.
    #[serde(skip)]
    pub guardie_aggiunte: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServiceSpec {
    pub name: String,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub autostart: bool,
    pub restart: bool,
    pub capture_output: bool,
}

impl Default for ServiceSpec {
    fn default() -> Self {
        Self {
            name: String::new(),
            program: String::new(),
            args: Vec::new(),
            cwd: String::new(),
            autostart: false,
            restart: true,
            capture_output: true,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            endpoint: nova_proto::endpoint_default(),
            autonomy: predefiniti::AUTONOMIA_PREDEFINITA.to_string(),
            protected_paths: default_protected(),
            write_roots: Vec::new(),
            forbidden_commands: predefiniti::COMANDI_VIETATI
                .iter()
                .map(|s| s.to_string())
                .collect(),
            shell_timeout_s: predefiniti::SHELL_TIMEOUT_S,
            services: Vec::new(),
            log_level: "info".into(),
            errore_caricamento: String::new(),
            campi_ignorati: Vec::new(),
            guardie_aggiunte: Vec::new(),
        }
    }
}

#[cfg(windows)]
fn default_protected() -> Vec<String> {
    predefiniti::PERCORSI_PROTETTI
        .iter()
        .map(|s| s.to_string())
        .collect()
}

#[cfg(not(windows))]
fn default_protected() -> Vec<String> {
    predefiniti::PERCORSI_PROTETTI_UNIX
        .iter()
        .map(|s| s.to_string())
        .collect()
}

impl Config {
    pub fn path() -> PathBuf {
        let base = if cfg!(windows) {
            std::env::var("APPDATA").unwrap_or_else(|_| ".".into())
        } else {
            std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
                format!("{home}/.config")
            })
        };
        PathBuf::from(base).join("NOVA").join("core.json")
    }

    pub fn load() -> Self {
        Self::leggi(&Self::path())
    }

    /// La configurazione, letta da un file preciso.
    ///
    /// Non stampa e non registra niente: quel che e' successo torna nei
    /// campi, e lo dice chi ha un posto dove dirlo. L'avviso che c'era prima
    /// usciva da qui, e usciva **prima che il logger esistesse** (D286).
    pub fn leggi(p: &Path) -> Self {
        let predefinito = match serde_json::to_value(Config::default()) {
            Ok(v) => v,
            Err(_) => return Config::default(),
        };
        let testo = match std::fs::read_to_string(p) {
            Ok(t) => t,
            // Il file non c'e': e' il caso normale della prima accensione.
            Err(_) => return Config::default(),
        };
        let lettura = nova_configurazione::leggi_con(&predefinito, &testo, &REGOLE);
        let mut cfg: Config = serde_json::from_value(lettura.config).unwrap_or_default();
        cfg.errore_caricamento = lettura.errore;
        cfg.campi_ignorati = lettura.rapporto.ignorate;
        cfg.guardie_aggiunte = lettura
            .rapporto
            .aggiunte
            .iter()
            .map(|a| format!("{}: {}", a.campo, a.voci.join(", ")))
            .collect();
        cfg
    }

    /// Cosa c'e' da dire a una persona su com'e' andata la lettura.
    ///
    /// Vuoto quando non c'e' niente da dire, che e' il caso normale.
    pub fn da_raccontare(&self) -> Vec<String> {
        let mut righe = Vec::new();
        if !self.errore_caricamento.is_empty() {
            righe.push(format!(
                "la configurazione in {} non si e' letta ({}): vado avanti coi \
                 valori di fabbrica e **non la riscrivo**, cosi' resta com'e' \
                 e la puoi correggere",
                Self::path().display(),
                self.errore_caricamento
            ));
        }
        for campo in &self.campi_ignorati {
            righe.push(format!(
                "il campo «{campo}» nella configurazione non ha la forma giusta: \
                 per quello ho tenuto il valore di fabbrica, il resto del file l'ho letto"
            ));
        }
        for aggiunta in &self.guardie_aggiunte {
            righe.push(format!(
                "ho rimesso nelle guardie quelle di fabbrica che mancavano — {aggiunta}"
            ));
        }
        righe
    }

    /// Scrive la configurazione.
    ///
    /// **Si rifiuta se il file non si era letto.** In quel caso `self` sono i
    /// predefiniti, e salvarli vorrebbe dire cancellare la configurazione di
    /// qualcuno — con dentro i percorsi che aveva protetto — per una virgola
    /// di troppo, e cancellarla proprio mentre gli si sta dicendo che c'e' un
    /// problema (D250, la stessa cosa dall'altra parte).
    pub fn save(&self) -> anyhow::Result<PathBuf> {
        if !self.errore_caricamento.is_empty() {
            anyhow::bail!(
                "non riscrivo la configurazione: quella che c'e' non si e' letta \
                 ({}), e sovrascriverla vorrebbe dire perderla. Correggi il file \
                 oppure spostalo, e riprova",
                self.errore_caricamento
            );
        }
        let p = Self::path();
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&p, serde_json::to_string_pretty(self)?)?;
        Ok(p)
    }

    pub fn log_dir() -> PathBuf {
        Self::path()
            .parent()
            .map(|d| d.join("logs"))
            .unwrap_or_else(|| PathBuf::from("logs"))
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    fn in_un_file(contenuto: &str) -> (tempo::Cartella, Config) {
        let c = tempo::Cartella::nuova();
        let f = c.0.join("core.json");
        std::fs::write(&f, contenuto).unwrap();
        let cfg = Config::leggi(&f);
        (c, cfg)
    }

    /// Una cartella che si cancella da se'.
    mod tempo {
        pub struct Cartella(pub std::path::PathBuf);
        impl Cartella {
            pub fn nuova() -> Cartella {
                let p = std::env::temp_dir().join(format!(
                    "nova-core-config-{}-{:?}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                ));
                std::fs::create_dir_all(&p).unwrap();
                Cartella(p)
            }
        }
        impl Drop for Cartella {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    #[test]
    fn un_campo_storto_non_porta_via_tutto_il_file() {
        // E' il difetto che questo filo ha chiuso. Prima: `from_str` falliva
        // sull'intero file, si tornava ai predefiniti, e `write_roots`
        // tornava vuoto — cioe' il confinamento delle scritture spariva, in
        // silenzio, per un campo che non c'entrava niente.
        let (_c, cfg) = in_un_file(
            r#"{"shell_timeout_s": "ciao", "write_roots": ["D:\\lavoro"],
                "endpoint": "\\\\.\\pipe\\mio"}"#,
        );
        assert_eq!(cfg.write_roots, vec!["D:\\lavoro"], "il confinamento resta");
        assert_eq!(
            cfg.endpoint, r"\\.\pipe\mio",
            "e il resto del file si legge"
        );
        assert_eq!(
            cfg.shell_timeout_s,
            predefiniti::SHELL_TIMEOUT_S,
            "quello storto no"
        );
        assert_eq!(cfg.campi_ignorati, vec!["shell_timeout_s".to_string()]);
        assert!(
            cfg.errore_caricamento.is_empty(),
            "il file non e' «illeggibile»"
        );
        // E chi legge lo viene a sapere.
        assert!(cfg
            .da_raccontare()
            .iter()
            .any(|r| r.contains("shell_timeout_s")));
    }

    #[test]
    fn e_nemmeno_un_nulla_o_un_decimale_dove_va_un_intero() {
        // Tutti e due farebbero fallire la conversione di **tutta** la
        // configurazione: `serde` rifiuta un `42.0` dove va un intero, e un
        // `null` dove va un numero. Scartare il campo e dirlo e' meglio che
        // perdere il file.
        let (_c, cfg) = in_un_file(r#"{"shell_timeout_s": null, "log_level": "debug"}"#);
        assert_eq!(cfg.shell_timeout_s, predefiniti::SHELL_TIMEOUT_S);
        assert_eq!(cfg.log_level, "debug", "il resto si legge");
        assert!(
            cfg.campi_ignorati.is_empty(),
            "un nulla non e' un errore da dire"
        );

        let (_c, cfg) = in_un_file(r#"{"shell_timeout_s": 42.0, "log_level": "debug"}"#);
        assert_eq!(cfg.shell_timeout_s, predefiniti::SHELL_TIMEOUT_S);
        assert_eq!(cfg.log_level, "debug", "il resto si legge");
        assert_eq!(cfg.campi_ignorati, vec!["shell_timeout_s".to_string()]);

        // E un intero normale entra, che e' il caso che deve continuare a
        // funzionare.
        let (_c, cfg) = in_un_file(r#"{"shell_timeout_s": 42}"#);
        assert_eq!(cfg.shell_timeout_s, 42);
    }

    #[test]
    fn le_guardie_di_fabbrica_tornano_dentro_tutte_e_due_le_liste() {
        // `forbidden_commands` si univa gia' (in `policy.rs`), `protected_paths`
        // no: due guardie nello stesso file con due comportamenti diversi.
        let (_c, cfg) = in_un_file(r#"{"protected_paths": ["D:\\mio"]}"#);
        assert_eq!(
            cfg.protected_paths[0], "D:\\mio",
            "quel che c'era viene prima"
        );
        for p in default_protected() {
            assert!(cfg.protected_paths.contains(&p), "manca {p}");
        }
        assert!(cfg
            .da_raccontare()
            .iter()
            .any(|r| r.contains("protected_paths")));
    }

    #[test]
    fn un_file_illeggibile_lo_dice_e_non_si_riscrive() {
        let (_c, cfg) = in_un_file("{questo non e' json");
        assert!(!cfg.errore_caricamento.is_empty());
        assert_eq!(
            cfg.endpoint,
            Config::default().endpoint,
            "si riparte da li'"
        );
        let racconto = cfg.da_raccontare();
        assert!(racconto[0].contains("non la riscrivo"), "{racconto:?}");
        // E `save` si rifiuta davvero, invece di cancellare quel che c'e'.
        let e = cfg.save().unwrap_err().to_string();
        assert!(e.contains("non riscrivo"), "{e}");
    }

    #[test]
    fn un_file_vuoto_non_e_un_file_rotto() {
        // Zero byte vuol dire «non c'e' scritto niente», non «e' rotto»: se
        // no il demone si lamenta per sempre di un file in cui non c'e'
        // niente da recuperare, e non lo riscrive mai.
        for vuoto in ["", "   \n", "\u{feff}"] {
            let (_c, cfg) = in_un_file(vuoto);
            assert!(cfg.errore_caricamento.is_empty(), "{vuoto:?}");
            assert!(cfg.da_raccontare().is_empty());
            assert_eq!(cfg.log_level, "info");
        }
    }

    #[test]
    fn un_endpoint_svuotato_non_vince_ma_write_roots_vuoto_si() {
        let (_c, cfg) = in_un_file(r#"{"endpoint": "", "log_level": "", "write_roots": []}"#);
        assert_eq!(
            cfg.endpoint,
            Config::default().endpoint,
            "senza, non si raggiunge"
        );
        assert_eq!(cfg.log_level, "info");
        assert!(cfg.write_roots.is_empty(), "vuoto qui e' una scelta vera");
    }

    #[test]
    fn e_quando_va_tutto_bene_non_si_dice_niente() {
        let (_c, cfg) = in_un_file(
            r#"{"log_level": "debug", "shell_timeout_s": 42,
                "protected_paths": []}"#,
        );
        assert_eq!(cfg.log_level, "debug");
        assert_eq!(cfg.shell_timeout_s, 42);
        // I predefiniti tornano dentro l'elenco svuotato, e **quello** si
        // dice: e' l'unica cosa che il demone ha cambiato di suo.
        assert_eq!(cfg.da_raccontare().len(), 1, "{:?}", cfg.da_raccontare());
        let (_c, cfg) = in_un_file(r#"{"log_level": "debug"}"#);
        assert!(cfg.da_raccontare().is_empty(), "{:?}", cfg.da_raccontare());
    }

    #[test]
    fn la_diagnostica_non_arriva_dal_file() {
        let (_c, cfg) = in_un_file(
            r#"{"errore_caricamento": "me lo sono inventato",
                "campi_ignorati": ["endpoint"]}"#,
        );
        assert!(cfg.errore_caricamento.is_empty());
        assert!(cfg.campi_ignorati.is_empty());
    }

    #[test]
    fn e_quel_che_si_salva_non_porta_la_diagnostica() {
        let mut cfg = Config::default();
        cfg.campi_ignorati = vec!["qualcosa".into()];
        let scritto = serde_json::to_string(&cfg).unwrap();
        assert!(!scritto.contains("campi_ignorati"), "{scritto}");
        assert!(!scritto.contains("errore_caricamento"), "{scritto}");
    }
}
