//! Fonemi con espeak-ng, caricato a runtime.
//!
//! La DLL sta nel runtime di NOVA insieme ai suoi dati: nessun pacchetto
//! Python, nessuna installazione di sistema. Si carica con `libloading`
//! invece di collegarla al binario perche' la voce e' opzionale — chi non la
//! usa non deve nemmeno avere il file.
//!
//! espeak-ng tiene stato globale di processo: due fonemizzazioni in
//! parallelo tornano fonemi corrotti. Da qui il mutex, che non e' pigrizia.
//! (Il pacchetto Python di riferimento fa la stessa cosa, per lo stesso
//! motivo.)

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use anyhow::{anyhow, Context, Result};
use libloading::{Library, Symbol};

/// L'audio non ci serve: si vogliono solo i fonemi.
const USCITA_SINCRONA: c_int = 0x02;
/// Il testo che passiamo e' UTF-8.
const TESTO_UTF8: c_int = 1;
/// Alfabeto fonetico internazionale, senza separatori fra i fonemi.
const FONEMI_IPA: c_int = 0x02;

type FnInizializza = unsafe extern "C" fn(c_int, c_int, *const c_char, c_int) -> c_int;
type FnVoce = unsafe extern "C" fn(*const c_char) -> c_int;
type FnFonemi = unsafe extern "C" fn(*mut *const c_void, c_int, c_int) -> *const c_char;

pub struct Espeak {
    _libreria: Library,
    inizializza: FnInizializza,
    imposta_voce: FnVoce,
    in_fonemi: FnFonemi,
}

#[derive(Default)]
struct Stato {
    voce_corrente: String,
}

/// Il lucchetto su espeak, uno per tutto il processo.
///
/// espeak-ng tiene il proprio stato in variabili globali: la lingua
/// impostata, i buffer di lavoro, il puntatore che avanza nel testo. Un lock
/// dentro `Espeak` sembra corretto e non lo e' — due istanze hanno due
/// lucchetti diversi e finiscono nella stessa libreria insieme, che e'
/// esattamente il caso che non deve succedere.
///
/// Non e' teoria: il banco di prova, che apre due istanze in due test
/// paralleli, cadeva con una corruzione di heap (0xC0000374). Passava a
/// giorni alterni, che e' il modo peggiore di essere rotti.
fn lucchetto() -> &'static Mutex<Stato> {
    static S: OnceLock<Mutex<Stato>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Stato::default()))
}

impl Espeak {
    /// `dll` e' il file della libreria, `dati` la cartella espeak-ng-data.
    pub fn apri(dll: &Path, dati: &Path) -> Result<Self> {
        if !dll.exists() {
            return Err(anyhow!("espeak-ng non trovato in {}", dll.display()));
        }
        if !dati.exists() {
            return Err(anyhow!("dati di espeak-ng non trovati in {}", dati.display()));
        }
        let libreria = unsafe { Library::new(dll) }
            .with_context(|| format!("caricamento di {}", dll.display()))?;
        unsafe {
            let inizializza: Symbol<FnInizializza> = libreria
                .get(b"espeak_Initialize\0")
                .context("espeak_Initialize assente")?;
            let imposta_voce: Symbol<FnVoce> = libreria
                .get(b"espeak_SetVoiceByName\0")
                .context("espeak_SetVoiceByName assente")?;
            let in_fonemi: Symbol<FnFonemi> = libreria
                .get(b"espeak_TextToPhonemes\0")
                .context("espeak_TextToPhonemes assente")?;
            let inizializza = *inizializza;
            let imposta_voce = *imposta_voce;
            let in_fonemi = *in_fonemi;

            // espeak vuole la cartella *madre* di espeak-ng-data
            let radice = dati
                .parent()
                .ok_or_else(|| anyhow!("percorso dei dati senza cartella madre"))?;
            let percorso = CString::new(radice.to_string_lossy().as_bytes())?;
            // Anche l'inizializzazione tocca lo stato globale: se un'altra
            // istanza sta fonemizzando in questo momento, si aspetta.
            let _guardia = lucchetto()
                .lock()
                .map_err(|_| anyhow!("stato di espeak avvelenato"))?;
            let frequenza = inizializza(USCITA_SINCRONA, 0, percorso.as_ptr(), 0);
            if frequenza < 0 {
                return Err(anyhow!(
                    "espeak_Initialize ha risposto {frequenza} (dati in {})",
                    radice.display()
                ));
            }
            drop(libreria.get::<*const c_void>(b"espeak_Initialize\0").ok());
            Ok(Self {
                _libreria: libreria,
                inizializza,
                imposta_voce,
                in_fonemi,
            })
        }
    }

    /// Testo -> IPA, per una lingua espeak («it», «en-us», ...).
    pub fn fonemi(&self, testo: &str, lingua: &str) -> Result<String> {
        let testo = testo.trim();
        if testo.is_empty() {
            return Ok(String::new());
        }
        let mut stato = lucchetto()
            .lock()
            .map_err(|_| anyhow!("stato di espeak avvelenato"))?;
        if stato.voce_corrente != lingua {
            let nome = CString::new(lingua)?;
            let esito = unsafe { (self.imposta_voce)(nome.as_ptr()) };
            if esito != 0 {
                return Err(anyhow!("espeak non conosce la lingua «{lingua}»"));
            }
            stato.voce_corrente = lingua.to_string();
        }

        let sorgente = CString::new(testo)?;
        let mut puntatore = sorgente.as_ptr() as *const c_void;
        let mut fuori = String::new();
        // espeak restituisce una frase per chiamata e fa avanzare il
        // puntatore: si cicla finche' non arriva a fine testo.
        let mut giri = 0;
        while !puntatore.is_null() {
            giri += 1;
            if giri > 512 {
                return Err(anyhow!("espeak non arriva alla fine del testo"));
            }
            let grezzo = unsafe { (self.in_fonemi)(&mut puntatore, TESTO_UTF8, FONEMI_IPA) };
            if grezzo.is_null() {
                break;
            }
            let pezzo = unsafe { CStr::from_ptr(grezzo) }.to_string_lossy();
            fuori.push_str(pezzo.trim());
        }
        drop(stato);
        Ok(fuori.trim().to_string())
    }

    /// Ping: la libreria risponde?
    pub fn viva(&self) -> bool {
        let _ = self.inizializza;
        self.fonemi("a", "it").map(|f| !f.is_empty()).unwrap_or(false)
    }
}

unsafe impl Send for Espeak {}
unsafe impl Sync for Espeak {}
