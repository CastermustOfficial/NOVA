//! # nova-ciclo
//!
//! Il turno dell'agente: chiedi, esegui, rileggi, ripeti.
//!
//! E' il pezzo che decide **quando** si fa cosa, e non fa nessuna delle cose.
//! Chiedere a un cervello vuol dire parlare in rete; eseguire uno strumento
//! vuol dire toccare il PC; salire di gradino vuol dire mandare il compito
//! fuori di qui. Tutto questo sta dietro al tratto [`Mondo`], e il turno non
//! sa cosa ci sia dall'altra parte.
//!
//! Non e' un'astrazione per bellezza. Le regole di questo giro sono quelle che
//! si vedono meno e costano di piu' — dopo quanti fallimenti si cambia
//! cervello, se una chiamata negata dall'utente conta come un errore del
//! modello, cosa si consegna quando i passi finiscono — e provarle contro un
//! cervello vero vorrebbe dire non provarle mai: servirebbe un modello che
//! fallisce tre volte a comando. Con la cucitura si prova tutto in
//! millesimi, su qualunque sistema, senza rete.
//!
//! Le decisioni vere non sono nemmeno qui: stanno in `nova-salita`, gia'
//! confrontate col Python da un banco. Qui c'e' l'ordine in cui si chiedono.

use async_trait::async_trait;
use nova_salita::{passi_finiti, serve_salire, Manopole as ManopoleSalita};

/// Una chiamata a uno strumento, come l'ha chiesta il modello.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chiamata {
    pub nome: String,
    /// Gli argomenti gia' resi in testo: renderli e' serializzazione, non
    /// decisione, e due serializzatori scrivono lo stesso oggetto in modo
    /// diverso (la stessa ragione per cui `nova-salita` li vuole cosi').
    pub argomenti: String,
}

/// Cosa ha risposto il cervello.
#[derive(Debug, Clone, Default)]
pub struct Risposta {
    pub contenuto: String,
    pub chiamate: Vec<Chiamata>,
}

/// Com'e' finita una chiamata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Andata {
    Riuscita,
    Fallita,
    /// L'utente ha detto di no.
    ///
    /// **Non e' un fallimento del modello: e' una scelta di chi comanda.**
    /// Contarla fra i fallimenti vorrebbe dire far salire di gradino — cioe'
    /// mandare il compito fuori dal PC, che e' la decisione piu' cara che
    /// NOVA prende da sola — perche' l'utente ha detto di no tre volte.
    Negata,
}

/// Il mondo fuori dal turno. L'unica cosa che tocca qualcosa.
#[async_trait]
pub trait Mondo: Send {
    /// Chiede al cervello. La conversazione la tiene chi implementa.
    async fn chiedi(&mut self) -> Result<Risposta, String>;
    /// Esegue una chiamata, ne mette il risultato in conversazione, e dice
    /// com'e' andata. Se e' andata male, l'errore da ricordare.
    async fn esegui(&mut self, c: &Chiamata) -> (Andata, String);
    /// Passa il compito a un cervello piu' capace, dati gli errori recenti.
    async fn sali(&mut self, errori: &[String]);
    /// Mette una nota in conversazione: sta girando in tondo.
    fn annota(&mut self, nota: &str);
    /// Consegna all'utente cio' che il modello ha appena scritto.
    fn consegna(&mut self, testo: &str);
    /// Se qualcuno ha chiesto di fermarsi.
    fn fermato(&self) -> bool {
        false
    }
}

/// Le manopole del turno.
#[derive(Debug, Clone)]
pub struct Manopole {
    pub passi_massimi: u32,
    pub salita: ManopoleSalita,
}

/// Come e' finito il turno.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fine {
    /// Ha risposto. Dentro, la risposta.
    Risposto(String),
    /// I passi sono finiti. Dentro, cosa si dice - che comprende quel che il
    /// modello aveva gia' scritto (D217).
    PassiFiniti(String),
    /// Qualcuno ha chiesto di fermarsi.
    Fermato,
    /// Il cervello non ha risposto affatto.
    Rotto(String),
}

/// Un turno intero.
pub async fn turno(m: &mut dyn Mondo, mano: &Manopole) -> Fine {
    let mut ultimo = String::new();
    let mut fallimenti = 0u32;
    let mut salite = 0u32;
    let mut errori: Vec<String> = Vec::new();
    let mut ripetizioni = nova_salita::Contatore::nuovo();

    for passo in 0..mano.passi_massimi {
        if m.fermato() {
            return Fine::Fermato;
        }
        let risposta = match m.chiedi().await {
            Ok(r) => r,
            Err(e) => return Fine::Rotto(e),
        };
        if !risposta.contenuto.trim().is_empty() {
            ultimo = risposta.contenuto.clone();
            m.consegna(&risposta.contenuto);
        }
        if risposta.chiamate.is_empty() {
            return Fine::Risposto(ultimo);
        }
        for c in &risposta.chiamate {
            if m.fermato() {
                return Fine::Fermato;
            }
            let (andata, errore) = m.esegui(c).await;
            match andata {
                // Una negata azzera come una riuscita: e' una risposta
                // dell'utente, non un muro contro cui il modello ha sbattuto.
                Andata::Riuscita | Andata::Negata => {
                    fallimenti = 0;
                    errori.clear();
                }
                Andata::Fallita => {
                    fallimenti += 1;
                    errori.push(errore);
                }
            }
            // Il promemoria arriva **anche** su una negata: un modello che
            // martella una cosa vietata e' esattamente il giro da spezzare.
            if let Some(nota) = ripetizioni.guarda(&c.nome, &c.argomenti, &c.argomenti) {
                m.annota(&nota);
            }
        }
        // `passo + 1`: i passi fatti, non l'indice. Con l'indice la soglia
        // scatta un giro dopo, e con la soglia a uno non scatterebbe mai.
        if serve_salire(&mano.salita, fallimenti, salite, passo + 1) {
            salite += 1;
            fallimenti = 0;
            m.sali(&errori).await;
            errori.clear();
        }
    }
    Fine::PassiFiniti(passi_finiti(mano.passi_massimi, &ultimo))
}

#[cfg(test)]
mod prove {
    use super::*;
    use async_trait::async_trait;

    /// Un mondo che risponde cio' che gli si dice, e si ricorda cosa e'
    /// successo. Nessun cervello, nessuno strumento, nessuna rete.
    #[derive(Default)]
    struct Finto {
        risposte: Vec<Risposta>,
        esiti: Vec<(Andata, String)>,
        consegnato: Vec<String>,
        annotato: Vec<String>,
        salite: Vec<Vec<String>>,
        eseguite: Vec<String>,
        fermo_dopo: Option<usize>,
        chiesto: usize,
    }

    impl Finto {
        fn che_dice(risposte: Vec<Risposta>) -> Self {
            Finto { risposte, ..Default::default() }
        }
    }

    #[async_trait]
impl Mondo for Finto {
        async fn chiedi(&mut self) -> Result<Risposta, String> {
            self.chiesto += 1;
            if self.risposte.is_empty() {
                return Err("il finto ha finito le risposte".into());
            }
            Ok(self.risposte.remove(0))
        }
        async fn esegui(&mut self, c: &Chiamata) -> (Andata, String) {
            self.eseguite.push(c.nome.clone());
            if self.esiti.is_empty() {
                (Andata::Riuscita, String::new())
            } else {
                self.esiti.remove(0)
            }
        }
        async fn sali(&mut self, errori: &[String]) {
            self.salite.push(errori.to_vec());
        }
        fn annota(&mut self, nota: &str) {
            self.annotato.push(nota.to_string());
        }
        fn consegna(&mut self, testo: &str) {
            self.consegnato.push(testo.to_string());
        }
        fn fermato(&self) -> bool {
            matches!(self.fermo_dopo, Some(n) if self.chiesto > n)
        }
    }

    fn chiama(nome: &str) -> Chiamata {
        Chiamata { nome: nome.into(), argomenti: "{}".into() }
    }

    fn parla(testo: &str, chiamate: &[&str]) -> Risposta {
        Risposta {
            contenuto: testo.into(),
            chiamate: chiamate.iter().map(|n| chiama(n)).collect(),
        }
    }

    fn mano(passi: u32) -> Manopole {
        Manopole {
            passi_massimi: passi,
            salita: ManopoleSalita {
                automatica: true,
                fallimenti_prima_di_salire: 2,
                passi_prima_di_salire: 0,
                salite_massime: 1,
            },
        }
    }

    #[tokio::test]
    async fn una_risposta_senza_strumenti_chiude_il_turno() {
        let mut f = Finto::che_dice(vec![parla("ecco qua", &[])]);
        assert_eq!(turno(&mut f, &mano(12)).await, Fine::Risposto("ecco qua".into()));
        assert_eq!(f.chiesto, 1, "non deve chiedere due volte");
        assert_eq!(f.consegnato, vec!["ecco qua"]);
    }

    #[tokio::test]
    async fn con_gli_strumenti_si_rilegge_e_si_va_avanti() {
        let mut f = Finto::che_dice(vec![
            parla("ora guardo", &["leggi"]),
            parla("trovato", &[]),
        ]);
        assert_eq!(turno(&mut f, &mano(12)).await, Fine::Risposto("trovato".into()));
        assert_eq!(f.eseguite, vec!["leggi"]);
        assert_eq!(f.consegnato, vec!["ora guardo", "trovato"]);
    }

    #[tokio::test]
    async fn due_fallimenti_di_fila_fanno_salire_di_gradino() {
        let mut f = Finto::che_dice(vec![
            parla("", &["a"]),
            parla("", &["b"]),
            parla("fatto", &[]),
        ]);
        f.esiti = vec![
            (Andata::Fallita, "non trovo il file".into()),
            (Andata::Fallita, "nemmeno questo".into()),
        ];
        turno(&mut f, &mano(12)).await;
        assert_eq!(f.salite.len(), 1, "doveva salire una volta");
        assert_eq!(f.salite[0], vec!["non trovo il file", "nemmeno questo"],
                   "e portarsi dietro gli errori che l'hanno deciso");
    }

    #[tokio::test]
    async fn una_riuscita_in_mezzo_azzera_il_conto() {
        let mut f = Finto::che_dice(vec![
            parla("", &["a"]),
            parla("", &["b"]),
            parla("", &["c"]),
            parla("fine", &[]),
        ]);
        f.esiti = vec![
            (Andata::Fallita, "uno".into()),
            (Andata::Riuscita, String::new()),
            (Andata::Fallita, "due".into()),
        ];
        turno(&mut f, &mano(12)).await;
        assert!(f.salite.is_empty(), "non sono due di fila: {:?}", f.salite);
    }

    #[tokio::test]
    async fn una_chiamata_negata_non_e_un_fallimento_del_modello() {
        // E' la decisione piu' cara che NOVA prende da sola: mandare il
        // compito fuori dal PC perche' l'utente ha detto di no due volte
        // sarebbe esattamente il contrario di ascoltarlo.
        let mut f = Finto::che_dice(vec![
            parla("", &["cancella"]),
            parla("", &["cancella"]),
            parla("va bene", &[]),
        ]);
        f.esiti = vec![
            (Andata::Negata, String::new()),
            (Andata::Negata, String::new()),
        ];
        turno(&mut f, &mano(12)).await;
        assert!(f.salite.is_empty(), "un no non fa salire: {:?}", f.salite);
    }

    #[tokio::test]
    async fn e_pero_il_giro_lo_vede_lo_stesso() {
        // Tre volte la stessa chiamata, negata ogni volta: martellare una
        // cosa vietata e' il giro da spezzare per eccellenza.
        let mut f = Finto::che_dice(vec![
            parla("", &["cancella"]),
            parla("", &["cancella"]),
            parla("", &["cancella"]),
            parla("ok", &[]),
        ]);
        f.esiti = vec![(Andata::Negata, String::new()); 3];
        turno(&mut f, &mano(12)).await;
        assert_eq!(f.annotato.len(), 1, "alla terza si dice: {:?}", f.annotato);
        assert!(f.annotato[0].contains("volte di fila"), "{:?}", f.annotato);
    }

    #[tokio::test]
    async fn finiti_i_passi_si_consegna_quel_che_ce() {
        let mut f = Finto::che_dice(vec![parla("un pezzo", &["a"]); 3]);
        match turno(&mut f, &mano(3)).await {
            Fine::PassiFiniti(d) => {
                assert!(d.starts_with("un pezzo"), "{d}");
                assert!(d.contains("3 passaggi"), "{d}");
            }
            altro => panic!("{altro:?}"),
        }
        assert_eq!(f.chiesto, 3, "ne ha fatti esattamente tre");
    }

    #[tokio::test]
    async fn fermarsi_si_puo_prima_di_toccare_qualunque_cosa() {
        // Il caso che conta: si e' chiesto di fermarsi mentre il cervello
        // stava rispondendo. Le chiamate che quella risposta portava non si
        // eseguono affatto - fermarsi dopo averle fatte sarebbe fermarsi a
        // cose fatte.
        let mut f = Finto::che_dice(vec![parla("", &["a", "b", "c"]); 4]);
        f.fermo_dopo = Some(0);
        assert_eq!(turno(&mut f, &mano(12)).await, Fine::Fermato);
        assert!(f.eseguite.is_empty(), "non doveva eseguirne nessuna: {:?}", f.eseguite);
    }

    #[tokio::test]
    async fn e_ci_si_ferma_anche_in_mezzo_a_una_riga_gia_cominciata() {
        let mut f = Finto::che_dice(vec![parla("", &["a", "b", "c"]); 4]);
        f.fermo_dopo = Some(1);
        assert_eq!(turno(&mut f, &mano(12)).await, Fine::Fermato);
        assert_eq!(f.eseguite, vec!["a", "b", "c"],
                   "il primo giro va fino in fondo, il secondo non comincia");
    }

    #[tokio::test]
    async fn un_cervello_che_non_risponde_non_diventa_una_risposta_vuota() {
        let mut f = Finto::che_dice(vec![]);
        match turno(&mut f, &mano(12)).await {
            Fine::Rotto(e) => assert!(e.contains("finito le risposte"), "{e}"),
            altro => panic!("un guasto non e' una risposta: {altro:?}"),
        }
    }

    #[tokio::test]
    async fn il_contenuto_vuoto_non_cancella_quel_che_aveva_gia_detto() {
        // Un passo che chiama strumenti senza scrivere niente non deve
        // cancellare la frase del passo prima: e' quella che si consegna se
        // poi i passi finiscono.
        let mut f = Finto::che_dice(vec![
            parla("ho trovato tre cose", &["a"]),
            parla("", &["b"]),
            parla("   ", &["c"]),
        ]);
        match turno(&mut f, &mano(3)).await {
            Fine::PassiFiniti(d) => assert!(d.starts_with("ho trovato tre cose"), "{d}"),
            altro => panic!("{altro:?}"),
        }
    }

    #[tokio::test]
    async fn la_soglia_dei_passi_conta_i_passi_fatti_non_lindice() {
        // Sembra pignoleria e non lo e': con l'indice, una soglia di **un**
        // passo non scatterebbe mai al primo giro - e una soglia di uno vuol
        // dire proprio «dopo il primo».
        let mut mano = mano(1);
        mano.salita.passi_prima_di_salire = 1;
        let mut f = Finto::che_dice(vec![parla("", &["a"])]);
        turno(&mut f, &mano).await;
        assert_eq!(f.salite.len(), 1,
                   "un passo fatto e' un passo, non zero");
    }

    #[tokio::test]
    async fn si_sale_una_volta_sola_se_una_sola_e_concessa() {
        let mut f = Finto::che_dice(vec![parla("", &["a"]); 9]);
        f.esiti = vec![(Andata::Fallita, "male".into()); 9];
        turno(&mut f, &mano(9)).await;
        assert_eq!(f.salite.len(), 1, "salite_massime e' 1: {:?}", f.salite.len());
    }
}
