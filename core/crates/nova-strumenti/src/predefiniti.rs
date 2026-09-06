//! Le guardie predefinite: cosa non si tocca, cosa non si esegue.
//!
//! **Generato da `_estrai_guardie.py`. Non si scrive a mano.** Erano due
//! elenchi — uno in `nova/config.py`, uno scritto a mano nella
//! configurazione del demone — e sapevano cose diverse (D185). Ora e' uno
//! solo, e sta in Python perche' e' li' che l'utente lo puo' cambiare.
//!
//! Qui ci sono i **valori**; il meccanismo che li applica sta in
//! [`crate::guardie`], e usa espressioni regolari senza distinzione fra
//! maiuscole e minuscole, come `re.IGNORECASE` di Python: i comandi si
//! scrivono come capita, e «DISKPART» e' `diskpart`.


/// I percorsi in cui NOVA non scrive mai, qualunque cosa dica il modello.
///
/// Sono quelli di Windows perche' NOVA in Python vive li'. Il demone gira
/// anche altrove e ne ha un secondo elenco per i sistemi Unix, che non ha un
/// gemello da confrontare: e' dichiarato accanto a questo, non nascosto
/// dentro la configurazione.
pub static PERCORSI_PROTETTI: [&str; 4] = [
    "C:\\Windows",
    "C:\\Program Files",
    "C:\\Program Files (x86)",
    "C:\\ProgramData\\Microsoft",
];

/// I comandi che non si eseguono, come espressioni regolari.
///
/// Non sono sottostringhe: `format ` come sottostringa blocca anche
/// `Get-Date -Format o`, mentre `\bformat\s+[a-z]:` blocca solo il comando
/// che formatta un disco. La differenza fra i due modi e' la differenza fra
/// una guardia che si puo' tenere accesa e una che si finisce per spegnere.
pub static COMANDI_VIETATI: [&str; 8] = [
    "\\bformat\\s+[a-z]:",
    "\\bvssadmin\\b.*\\bdelete\\b",
    "\\bbcdedit\\b",
    "\\bcipher\\s+/w",
    "\\bdiskpart\\b",
    "\\bwevtutil\\s+cl\\b",
    "\\bmkfs(\\.[a-z0-9]+)?\\b",
    "\\brm\\s+(?:-[a-z]+\\s+)*-[a-z]*(?:rf|fr)[a-z]*\\s+/",
];

/// L'equivalente per i sistemi Unix, dove gira solo il demone.
///
/// Non viene da Python — NOVA in Python e' di Windows — e quindi nessun
/// banco lo confronta con niente. E' dichiarato qui, accanto all'altro,
/// proprio perche' si veda che e' l'unico senza gemello.
pub static PERCORSI_PROTETTI_UNIX: [&str; 5] = [
    "/boot",
    "/etc",
    "/sys",
    "/proc",
    "/dev",
];

/// I tre livelli di autonomia, dal piu' prudente al piu' libero.
pub static LIVELLI: [&str; 3] = [
    "always_ask",
    "ask_risky",
    "autonomous",
];

/// Come si chiamano i tre livelli quando li legge una persona.
pub static ETICHETTE: [&str; 3] = [
    "Conferma sempre",
    "Conferma azioni rischiose",
    "Autonomo",
];

/// Il livello predefinito: si chiede per le azioni rischiose.
pub const AUTONOMIA_PREDEFINITA: &str = "ask_risky";

/// Quanto si aspetta un comando di shell, in secondi.
pub const SHELL_TIMEOUT_S: u64 = 120;
