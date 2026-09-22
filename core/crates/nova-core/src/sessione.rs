//! La conversazione, fra un turno e l'altro.
//!
//! Un turno e' una cosa che comincia e finisce: si compone la domanda, si
//! chiede, si eseguono strumenti, si risponde. Ma la conversazione no — e
//! quel che resta fra un turno e l'altro e' poco e va tenuto con cura,
//! perche' ognuna di quelle poche cose e' gia' stata sbagliata una volta.
//!
//! **Il prompt di sistema si compone una volta sola.** I fornitori tengono la
//! cache sulla prima regione della richiesta: se il messaggio numero zero
//! cambia di un carattere a ogni turno — un orologio, un contatore — la cache
//! non vale piu' e si rielabora l'intera conversazione a ogni risposta. Non
//! si rompe niente e diventa lento per sempre, che e' il difetto peggiore da
//! accorgersene. Qui il prompt arriva gia' fatto e si tiene com'e'.
//!
//! **Il conto delle deleghe non torna indietro.** L'identificativo di una
//! delega si ricavava dalla lunghezza della conversazione, che e' comodo
//! finche' qualcuno non la accorcia: dopo un taglio due deleghe diverse
//! ricevono lo stesso numero, e la seconda risponde alla prima (D218). Sta
//! qui, e sale e basta.
//!
//! **I gradini restano, il gradino no.** La scala dei cervelli e' della
//! conversazione; essere saliti sul terzo gradino e' di **questo** turno. Se
//! non si tornasse giu' a ogni turno, una domanda difficile manderebbe fuori
//! casa anche tutte quelle facili che vengono dopo.

use crate::mondo::{Gradino, Misure};
use serde_json::{json, Value};

/// Cio' che sopravvive a un turno.
pub struct Sessione {
    /// Il messaggio numero zero, gia' composto. Non si ricalcola.
    sistema: String,
    /// La conversazione, messaggio di sistema compreso.
    pub messaggi: Vec<Value>,
    /// I cervelli in ordine di potenza. Il primo e' quello da cui si parte.
    pub gradini: Vec<Gradino>,
    /// Entro quanto deve stare la conversazione.
    pub misure: Misure,
    /// Quante volte si e' delegato **da quando la sessione e' aperta**.
    pub deleghe: u32,
    /// La sessione di Claude Code che tiene il filo di questa conversazione.
    ///
    /// Claude Code la conversazione la tiene lui, e si riprende con
    /// `--resume`: qui si conserva solo il capo del filo. Vuoto vuol dire
    /// «aprine una», ed e' anche l'unico caso in cui gli si passa il prompt
    /// di sistema. Sta in memoria e non su disco: il demone resta acceso, e
    /// il problema per cui il Python la scriveva su un file — un processo per
    /// messaggio, e ogni frase una conversazione nuova — qui non c'e'.
    pub claude: String,
}

impl Sessione {
    /// Una conversazione nuova, con il suo prompt di sistema.
    pub fn nuova(sistema: &str, gradini: Vec<Gradino>) -> Sessione {
        Sessione {
            sistema: sistema.to_string(),
            messaggi: vec![json!({"role": "system", "content": sistema})],
            gradini,
            misure: Misure::default(),
            deleghe: 0,
            claude: String::new(),
        }
    }

    /// Il prompt di sistema, per chi deve contarne i token.
    pub fn sistema(&self) -> &str {
        &self.sistema
    }

    /// Ricomincia da capo, tenendo lo stesso prompt.
    ///
    /// Le deleghe **non** si azzerano: gli identificativi che hanno gia'
    /// viaggiato verso un fornitore non tornano liberi solo perche' qui si e'
    /// voltato pagina.
    pub fn ricomincia(&mut self) {
        self.messaggi = vec![json!({"role": "system", "content": self.sistema})];
        // «Ricomincia da capo» vale anche per Claude Code: senza questa riga
        // il bottone svuotava la conversazione di NOVA e Claude riprendeva la
        // sua, con tutto quello che si era detto prima.
        self.claude.clear();
    }

    /// Ricomincia con un prompt nuovo: cambia la configurazione, cambia il
    /// messaggio numero zero.
    pub fn ricomincia_con(&mut self, sistema: &str) {
        self.sistema = sistema.to_string();
        self.ricomincia();
    }

    /// Aggiunge la domanda dell'utente, gia' composta con cio' che le va in
    /// coda (`nova_contesto::blocchi::domanda`).
    ///
    /// Comporre qui dentro sarebbe comodo e sbagliato: il blocco di memoria
    /// vuole il grafo, quello delle procedure vuole le ricette, e una
    /// conversazione che per aggiungere una riga ha bisogno di mezzo
    /// programma non si prova senza mezzo programma.
    pub fn chiede(&mut self, domanda: &str) {
        self.messaggi
            .push(json!({"role": "user", "content": domanda}));
    }

    /// Quanti messaggi ci sono, prompt di sistema compreso.
    pub fn quanti(&self) -> usize {
        self.messaggi.len()
    }
}

#[cfg(test)]
mod prove {
    use super::*;
    use nova_scala::Specie;

    fn scala(quanti: usize) -> Vec<Gradino> {
        (0..quanti)
            .map(|i| Gradino::nuovo(&format!("g{i}"), Specie::Api, "http://x", "m", vec![], true, crate::mondo::Come::Niente))
            .collect()
    }

    #[test]
    fn si_apre_con_il_suo_messaggio_numero_zero() {
        let s = Sessione::nuova("sei NOVA", scala(2));
        assert_eq!(s.quanti(), 1);
        assert_eq!(s.messaggi[0]["role"], "system");
        assert_eq!(s.messaggi[0]["content"], "sei NOVA");
    }

    #[test]
    fn il_prompt_non_si_ricompone_mai() {
        // I fornitori tengono la cache sulla prima regione della richiesta:
        // un carattere diverso nel messaggio numero zero e si rielabora
        // l'intera conversazione. Qui il prompt arriva gia' fatto e resta
        // quello, identico, anche dopo dieci giri.
        let mut s = Sessione::nuova("sono le 14:32", scala(1));
        let zero = s.messaggi[0].clone();
        for i in 0..10 {
            s.chiede(&format!("domanda {i}"));
            s.ricomincia();
            assert_eq!(
                s.messaggi[0], zero,
                "il messaggio numero zero e' cambiato al giro {i}"
            );
        }
    }

    #[test]
    fn ricominciare_butta_la_conversazione_e_tiene_la_testa() {
        let mut s = Sessione::nuova("sistema", scala(1));
        s.chiede("uno");
        s.chiede("due");
        assert_eq!(s.quanti(), 3);
        s.ricomincia();
        assert_eq!(s.quanti(), 1);
        assert_eq!(s.messaggi[0]["content"], "sistema");
    }

    #[test]
    fn le_deleghe_non_tornano_indietro() {
        // L'identificativo di una delega si ricavava dalla lunghezza della
        // conversazione: comodo finche' qualcuno non la accorcia, e allora
        // due deleghe diverse ricevono lo stesso numero e la seconda
        // risponde alla prima (D218). Azzerarlo a ogni «ricomincia» sarebbe
        // lo stesso difetto con un'altra faccia.
        let mut s = Sessione::nuova("sistema", scala(3));
        s.deleghe = 7;
        s.ricomincia();
        assert_eq!(s.deleghe, 7, "le deleghe gia' partite non tornano libere");
        s.ricomincia_con("un altro sistema");
        assert_eq!(s.deleghe, 7);
    }

    #[test]
    fn cambiare_configurazione_cambia_la_testa() {
        let mut s = Sessione::nuova("prima", scala(1));
        s.chiede("qualcosa");
        s.ricomincia_con("dopo");
        assert_eq!(s.sistema(), "dopo");
        assert_eq!(s.messaggi[0]["content"], "dopo");
        assert_eq!(s.quanti(), 1);
    }

    #[test]
    fn la_domanda_si_riceve_gia_composta() {
        // Comporre qui dentro vorrebbe dire che per aggiungere una riga alla
        // conversazione serve il grafo e le ricette. La composizione sta in
        // `nova_contesto::blocchi::domanda`, e qui arriva il risultato.
        let mut s = Sessione::nuova("sistema", scala(1));
        let d = nova_contesto::blocchi::domanda(
            "che ore sono",
            &nova_contesto::blocchi::memoria("gio usa Rust"),
            "",
            nova_contesto::blocchi::identita(false),
            "",
        );
        s.chiede(&d);
        // Chi parla e' l'utente. Un secondo messaggio di sistema in mezzo
        // alla conversazione lo vietano meta' dei fornitori, e gli altri lo
        // accettano e lo trattano male: `nova_contesto::un_solo_sistema`
        // esiste per rimediare a questo, e qui non deve servire.
        assert_eq!(s.messaggi[1]["role"], "user");
        assert_eq!(
            s.messaggi.iter().filter(|m| m["role"] == "system").count(),
            1,
            "di messaggi di sistema ce n'e' uno solo, ed e' il primo",
        );
        let dentro = s.messaggi[1]["content"].as_str().unwrap();
        assert!(
            dentro.starts_with("che ore sono"),
            "la domanda resta la prima cosa"
        );
        assert!(dentro.contains("<memoria>"));
    }

    #[test]
    fn la_scala_e_della_conversazione() {
        let s = Sessione::nuova("sistema", scala(3));
        assert_eq!(s.gradini.len(), 3);
        assert_eq!(s.gradini[0].nome(), "g0");
    }
}
