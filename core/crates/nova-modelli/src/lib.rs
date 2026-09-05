//! # nova-modelli
//!
//! Trovare i modelli, leggerne la forma, decidere quanto caricarne su GPU.
//!
//! E' il pezzo del cantiere con la ragione piu' forte per stare in Rust, e non
//! e' la velocita'. Il modulo Python equivalente porta scritto in testa che e'
//! «di sola libreria standard, di proposito: viene eseguito dall'installatore
//! prima che le dipendenze del progetto siano garantite». E' una circolarita'
//! ammessa in una riga di commento: per decidere quale modello serve, oggi
//! bisogna gia' avere Python. Qui non serve piu' niente - il binario cerca,
//! legge e calcola su una macchina appena accesa.
//!
//! Tre moduli, tre domande:
//!
//! - [`trova`] - quali file GGUF ci sono su questo disco, e quale conviene;
//! - [`gguf`] - com'e' fatto il modello dentro (quanti strati, che contesto);
//! - [`strati`] - quanti di quegli strati stanno davvero in VRAM;
//! - [`motore`] - quale llama-server c'e' su questo disco, e se usa la scheda.
//!
//! Niente dipendenze obbligatorie, e nemmeno una chiamata al sistema
//! operativo: le radici da percorrere si passano da fuori, e chi sa cosa sia
//! un disco fisso e' `nova-platform`. Non e' pignoleria - e' cio' che rende
//! questo codice provabile su Linux con una cartella finta invece che solo
//! sulla macchina di chi l'ha scritto.

pub mod gguf;
pub mod motore;
pub mod strati;
pub mod trova;

// Come si accende il modello locale: la riga di comando, la scala dei layer,
// e l'unico errore che vale la pena riprovare.
pub mod avvio;

pub use gguf::{e_gguf, forma, metadati, Forma, Valore};
pub use motore::{acceleratore_di, motori, Acceleratore, Motore};
pub use strati::{peso_kv, strati_su_gpu, PESO_KV, RISERVA_MB};
pub use trova::{
    cartelle_note, trova, verifica_file, Come, Resoconto, Trovato, Verifica, MINIMO_BYTE,
    PROFONDITA, PROFONDITA_OVUNQUE,
};
