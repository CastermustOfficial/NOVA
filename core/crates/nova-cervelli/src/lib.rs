//! Cosa NOVA dice a un cervello che vive **fuori** da lei.
//!
//! Un cervello esterno riceve tre cose e nessuna delle tre e' testo libero:
//! una riga di comando, un prompt di sistema, un payload JSON. Sbagliare in
//! ognuna delle tre non da' un errore — da' un cervello che si comporta
//! diversamente e non sa dire perche'.
//!
//! - [`claude`]: Claude Code in modalita' headless. E' **agentico**: agisce
//!   coi propri strumenti, e NOVA gli fa da tramite.
//! - [`cli`]: qualunque altro binario agentico dichiarato in configurazione.
//! - [`openai`]: il dialetto OpenAI, che parlano sia il modello locale sia le
//!   API esterne. Non e' agentico: propone chiamate, le esegue NOVA.
//!
//! Le dichiarazioni — identita', permessi, elenco degli strumenti — stanno in
//! [`dichiarazioni`] e sono **generate** da `_estrai_cervelli.py` (D112).

pub mod accesso;
pub mod claude;
pub mod cli;
pub mod dichiarazioni;
pub mod openai;
pub mod rete;

/// Un messaggio della conversazione, come lo vedono i cervelli.
pub use nova_contesto::Messaggio;

/// Il testo dei messaggi di sistema, uno solo, separati da una riga vuota.
///
/// E' `"\n\n".join(...).strip()` di Python: la stessa forma in tre posti
/// diversi — il prompt di Claude Code, il prompt di una CLI, la testa di un
/// payload OpenAI — e quindi un posto solo.
pub fn istruzioni(messaggi: &[Messaggio]) -> String {
    messaggi
        .iter()
        .filter(|m| m.ruolo == "system")
        .map(|m| m.contenuto.clone())
        .collect::<Vec<_>>()
        .join("\n\n")
        .trim()
        .to_string()
}

/// `str.format` con dei segnaposto dichiarati, e nient'altro.
///
/// Non e' `format!`: i testi arrivano da `dichiarazioni`, e li' `{` e `}`
/// compaiono solo nei segnaposto. Se un giorno un testo ne contenesse uno
/// solo, Python solleverebbe un `KeyError` e qui non succederebbe niente —
/// e' la differenza che ha fatto D157, e una prova la dichiara.
pub fn riempi(testo: &str, valori: &[(&str, &str)]) -> String {
    let mut fuori = testo.to_string();
    for (nome, valore) in valori {
        fuori = fuori.replace(&format!("{{{nome}}}"), valore);
    }
    fuori
}
