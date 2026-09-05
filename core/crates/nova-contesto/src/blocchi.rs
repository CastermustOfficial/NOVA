//! Quello che si attacca **in coda alla domanda**, e perche' proprio li'.
//!
//! Sta in `nova-contesto` per la ragione che lo ha fatto nascere, che e' una
//! ragione di finestra e non di contenuto.
//!
//! Prima questi testi finivano nel messaggio di sistema, riscritto a ogni
//! turno. Due difetti in uno.
//!
//! **Funzionale**: i cervelli agentici il prompt di sistema lo ricevono solo
//! all'apertura della sessione, quindi dal secondo turno in poi il contesto
//! veniva calcolato e buttato. NOVA faceva la ricerca sul grafo e non la
//! leggeva.
//!
//! **Di costo**: il messaggio di sistema e' la prima regione di token su cui
//! un fornitore tiene la cache. Cambiarlo a ogni turno — e cambiava, perche'
//! il contesto dipende dalla domanda — invalida tutto il prefisso: ogni turno
//! rielaborava l'intera conversazione da capo. In coda invece si aggiunge e
//! basta, e il prefisso resta valido.
//!
//! E' la stessa aritmetica del fondo nel taglio dei messaggi: non cambia cosa
//! il modello legge, cambia quanto spesso si butta via la cache.

/// Cio' che la memoria ha trovato, da mettere in coda alla domanda.
///
/// Il testo attorno non e' decorazione: dice al modello **come** leggerlo —
/// prima di misurare, senza ripeterlo all'utente come una novita', e con il
/// permesso esplicito di correggerlo. Senza quella cornice il contesto
/// diventa un blocco di fatti che il modello ripete a pappagallo.
///
/// Contesto vuoto vuol dire niente blocco: una cornice attorno al nulla
/// costa token e dice al modello che la memoria e' vuota, che e' una cosa
/// diversa dal non averla interrogata.
pub fn memoria(contesto: &str) -> String {
    if contesto.is_empty() {
        return String::new();
    }
    format!(
        "\n\n<memoria>\n\
         Quello che gia' sai, dalla tua memoria a grafo. Guardalo prima di \
         misurare o cercare, e non ripeterlo all'utente come se fosse una \
         novita'. Se scopri che qualcosa qui e' superato, correggilo con \
         kb_note o kb_forget.\n\n{contesto}\n</memoria>"
    )
}

/// Il richiamo all'identita', per i soli cervelli agentici.
///
/// Chi non e' agentico riceve il prompt di sistema intero a ogni chiamata e
/// non ha bisogno di essere richiamato all'ordine.
pub fn identita(agentico: bool) -> &'static str {
    if agentico {
        crate::testi::PROMEMORIA
    } else {
        ""
    }
}

/// Il messaggio dell'utente con attaccato tutto il resto.
///
/// **L'ordine conta**, ed e' l'unica cosa che questa funzione decide: prima
/// cio' che hai chiesto, poi cio' che NOVA sa, poi cio' che ha gia' fatto,
/// poi come deve rispondere. L'istruzione resta l'ultima cosa letta, che e'
/// il posto in cui i modelli la seguono di piu'.
///
/// La `postilla` e' un'istruzione per il cervello e basta: non entra nella
/// ricerca in memoria e non viene imparata. Serve alla voce, che a ogni turno
/// deve ricordare al cervello di rispondere come si parla — e che non puo'
/// metterlo nel prompt di sistema, visto che quello si passa solo quando la
/// sessione si apre.
pub fn domanda(
    testo: &str,
    memoria: &str,
    procedure: &str,
    identita: &str,
    postilla: &str,
) -> String {
    format!("{testo}{memoria}{procedure}{identita}{postilla}")
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn niente_memoria_niente_cornice() {
        assert_eq!(memoria(""), "");
    }

    #[test]
    fn la_cornice_dice_come_leggerlo() {
        let b = memoria("gio usa Rust");
        assert!(b.starts_with("\n\n<memoria>\n"));
        assert!(b.ends_with("\n</memoria>"));
        assert!(b.contains("gio usa Rust"));
        assert!(b.contains("kb_forget"), "manca il permesso di correggere");
    }

    #[test]
    fn lordine_e_quello_e_non_un_altro() {
        assert_eq!(domanda("chiesto", "-sa", "-fatto", "-chi", "-come"),
                   "chiesto-sa-fatto-chi-come");
        // Senza niente attorno resta esattamente la domanda: nessuna riga
        // aggiunta, nessuno spazio.
        assert_eq!(domanda("chiesto", "", "", "", ""), "chiesto");
    }

    #[test]
    fn lidentita_solo_a_chi_serve() {
        assert_eq!(identita(false), "");
        assert!(identita(true).contains("<sei_nova>"));
    }
}
