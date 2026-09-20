//! Quali giudizi NOVA da', su che materia, e cosa puo' uscire dal PC.
//!
//! NOVA e' piena di punti in cui **decide**, e quasi tutti decidono con liste
//! di parole, soglie e espressioni regolari: quale cervello serve, se una
//! frase e' un fatto da ricordare, se un risultato e' pertinente, quanto e'
//! rischiosa una chiamata. Ognuna di quelle e' un giudizio travestito da
//! conto, e restano cosi' non per pigrizia ma perche' l'alternativa costava
//! un giro di modello per ogni domandina.
//!
//! Questa cassetta non decide niente. Fa una cosa sola e la fa presto: dice
//! **di che materia e' fatta ciascuna decisione**, e quindi cosa si puo'
//! mandare fuori dal PC a farsi giudicare e cosa no.
//!
//! ## Perche' sta in un crate e non in un documento
//!
//! Perche' un confine scritto in un documento non lo controlla nessuno.
//! Quello che c'e' qui si puo' provare, e soprattutto: cio' che non deve
//! uscire **non si puo' costruire** nella forma che esce (vedi [`Fuori`]).
//! E' la stessa lezione del gradino che e' un processo e non un indirizzo —
//! costruire la cosa sbagliata deve essere impossibile, non tardivo.
//!
//! ## Cosa non c'e' qui, di proposito
//!
//! Il giudizio. Le euristiche di oggi stanno dove sono sempre state —
//! `nova_scala` per la scala, `nova_guasti` per le spiegazioni, il vault per
//! la memoria — e questa cassetta non ne tiene una seconda copia. Sapere
//! **cosa** si decide e **dove** si puo' decidere sono due domande diverse da
//! «come si decide», e mescolarle vorrebbe dire una terza copia di regole che
//! ne hanno gia' due.

/// Di cosa e' fatta una decisione.
///
/// Non e' una tassonomia per il gusto di averne una: e' l'unica cosa che
/// serve per rispondere alla domanda che conta, cioe' se quel materiale puo'
/// attraversare la porta di casa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Materia {
    /// Il testo di cio' che l'utente ha chiesto.
    Compito,
    /// Un messaggio d'errore. Ci finiscono dentro percorsi e nomi di file,
    /// ed e' il motivo per cui non e' materia neutra quanto sembra.
    Guasto,
    /// Una riga di conversazione fra l'utente e NOVA.
    Scambio,
    /// Un pezzo di cio' che NOVA ha imparato: il vault.
    Memoria,
    /// Un comando o una chiamata a uno strumento, cosi' com'e' scritta.
    Chiamata,
    /// Credenziali, chiavi, forme riservate.
    ///
    /// **Questa non esce mai**, e non e' una preferenza da configurare.
    /// Chiedere a qualcun altro «questa e' una chiave?» vuol dire mandargli
    /// la chiave: il costo e' esattamente il difetto che si voleva evitare, e
    /// nessuna qualita' di risposta lo ripaga.
    Segreto,
}

impl Materia {
    /// Se questa materia puo' essere giudicata fuori dal PC.
    pub fn puo_uscire(self) -> bool {
        !matches!(self, Materia::Segreto)
    }

    /// Come si chiama, per chi legge un registro o un pannello.
    pub fn nome(self) -> &'static str {
        match self {
            Materia::Compito => "compito",
            Materia::Guasto => "guasto",
            Materia::Scambio => "scambio",
            Materia::Memoria => "memoria",
            Materia::Chiamata => "chiamata",
            Materia::Segreto => "segreto",
        }
    }
}

/// Le decisioni che NOVA prende, e su che materia.
///
/// L'elenco e' il censimento di CANT-12 messo in codice. Sta qui e non in una
/// tabella perche' una tabella non si puo' provare: da qui si puo' chiedere,
/// per ognuna, se il suo materiale puo' uscire — e una decisione nuova che
/// non dichiara la sua materia non compila.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decisione {
    /// Quale gradino della scala serve per questo compito.
    /// Oggi: liste di parole per categoria (`nova_scala::gradino_minimo`).
    QualeCervello,
    /// In questo scambio c'e' un fatto durevole da ricordare?
    /// Oggi: `len(testo) >= 25 caratteri`.
    ValeLaPenaRicordare,
    /// Quanto e' pertinente questo pezzo di memoria a questa domanda?
    /// Oggi: somiglianza fra vettori piu' parole in comune.
    QuantoCentra,
    /// Questa cosa che si e' appena fatta vale come procedura da riusare?
    /// Oggi: ha impiegato piu' di otto secondi.
    ValeLaPenaSalvare,
    /// Quanto e' rischiosa **questa** chiamata, non lo strumento in generale.
    /// Oggi: il rischio e' dichiarato una volta per strumento, e
    /// `run_command("ls")` e `run_command("rm -rf /")` hanno lo stesso.
    QuantoRischia,
    /// Come si spiega questo errore a una persona.
    /// Oggi: corrispondenze su stringhe d'errore (`nova_guasti::spiega`).
    ComeSiSpiega,
    /// In questo testo c'e' una credenziale?
    /// Oggi: prefissi noti e espressioni regolari.
    CeUnSegreto,
}

impl Decisione {
    /// Di che materia e' fatta.
    pub fn materia(self) -> Materia {
        match self {
            Decisione::QualeCervello => Materia::Compito,
            Decisione::ValeLaPenaRicordare => Materia::Scambio,
            Decisione::QuantoCentra => Materia::Memoria,
            Decisione::ValeLaPenaSalvare => Materia::Scambio,
            Decisione::QuantoRischia => Materia::Chiamata,
            Decisione::ComeSiSpiega => Materia::Guasto,
            Decisione::CeUnSegreto => Materia::Segreto,
        }
    }

    /// Tutte, per chi deve mostrarle o provarle tutte.
    ///
    /// Scritto a mano e non derivato: aggiungerne una qui e' una riga, e
    /// dimenticarsene fa diventare rossa una prova. Una lista che si genera
    /// da sola non avrebbe fatto la stessa cosa.
    pub const TUTTE: [Decisione; 7] = [
        Decisione::QualeCervello,
        Decisione::ValeLaPenaRicordare,
        Decisione::QuantoCentra,
        Decisione::ValeLaPenaSalvare,
        Decisione::QuantoRischia,
        Decisione::ComeSiSpiega,
        Decisione::CeUnSegreto,
    ];
}

/// Materiale pronto a uscire dal PC.
///
/// **Si costruisce solo se la materia lo consente.** E' tutto il punto di
/// questa cassetta: chi vuole mandare qualcosa a farsi giudicare fuori deve
/// passare di qui, e di qui un [`Materia::Segreto`] non passa. Non c'e' un
/// controllo da ricordarsi di fare piu' avanti — piu' avanti non si arriva.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fuori {
    materia: Materia,
    testo: String,
}

impl Fuori {
    /// Prepara del materiale per un giudizio fuori casa.
    ///
    /// Torna `None` se quella materia non esce, oppure se **l'utente ha
    /// chiesto che non esca niente**: `solo_locale` non e' una preferenza fra
    /// le altre, e chiederlo a valle vorrebbe dire che qualcuno prima o poi
    /// si dimentica di chiederlo.
    pub fn prepara(d: Decisione, testo: &str, solo_in_casa: bool) -> Option<Fuori> {
        if solo_in_casa || !d.materia().puo_uscire() {
            return None;
        }
        Some(Fuori { materia: d.materia(), testo: testo.to_string() })
    }

    pub fn materia(&self) -> Materia {
        self.materia
    }

    pub fn testo(&self) -> &str {
        &self.testo
    }
}

/// Dove si e' deciso, per chi tiene il registro.
///
/// Un giudizio dato fuori casa e uno dato in casa sono due fatti diversi, e
/// chi legge il registro deve poterli distinguere senza indovinare.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dove {
    InCasa,
    Fuori,
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn un_segreto_non_esce_mai() {
        // Non «non dovrebbe»: non si puo' costruire la cosa che esce.
        assert!(Fuori::prepara(Decisione::CeUnSegreto, "sk-abc123", false).is_none());
        assert!(Fuori::prepara(Decisione::CeUnSegreto, "sk-abc123", true).is_none());
        assert!(!Materia::Segreto.puo_uscire());
    }

    #[test]
    fn e_nemmeno_se_qualcuno_cambia_idea_a_valle() {
        // La domanda si fa **una volta**, qui. Se `Fuori` si potesse
        // costruire e poi controllare, il controllo sarebbe una riga da
        // ricordarsi in ogni posto che manda qualcosa.
        for d in Decisione::TUTTE {
            let f = Fuori::prepara(d, "x", false);
            assert_eq!(
                f.is_some(),
                d.materia().puo_uscire(),
                "{:?} non rispetta la sua materia",
                d,
            );
        }
    }

    #[test]
    fn con_solo_locale_non_esce_niente() {
        // Non solo i segreti: **niente**. E' la promessa che l'utente ha
        // chiesto accendendo quell'interruttore.
        for d in Decisione::TUTTE {
            assert!(
                Fuori::prepara(d, "x", true).is_none(),
                "{:?} esce lo stesso con solo_locale acceso",
                d,
            );
        }
    }

    #[test]
    fn quel_che_esce_esce_intero() {
        let f = Fuori::prepara(Decisione::QualeCervello, "apri il PDF e riassumilo", false)
            .expect("il compito puo' uscire");
        assert_eq!(f.testo(), "apri il PDF e riassumilo");
        assert_eq!(f.materia(), Materia::Compito);
    }

    #[test]
    fn ogni_decisione_dichiara_la_sua_materia() {
        // Una decisione nuova senza materia non compila; una decisione nuova
        // che si dimentica di entrare in TUTTE la fa diventare rossa.
        assert_eq!(Decisione::TUTTE.len(), 7, "aggiungine una e aggiorna il conto");
        let mut viste: Vec<&str> = Decisione::TUTTE.iter().map(|d| d.materia().nome()).collect();
        viste.sort_unstable();
        viste.dedup();
        assert!(viste.len() >= 4, "tutte le decisioni parlano della stessa materia?");
    }

    #[test]
    fn le_materie_si_chiamano_tutte_in_modo_diverso() {
        let tutte = [
            Materia::Compito, Materia::Guasto, Materia::Scambio,
            Materia::Memoria, Materia::Chiamata, Materia::Segreto,
        ];
        let mut nomi: Vec<&str> = tutte.iter().map(|m| m.nome()).collect();
        nomi.sort_unstable();
        let quanti = nomi.len();
        nomi.dedup();
        assert_eq!(nomi.len(), quanti, "due materie con lo stesso nome nel registro");
    }

    #[test]
    fn solo_una_materia_resta_in_casa() {
        // Se un domani ne restassero due, e' una scelta da fare apposta e non
        // una riga da cambiare distrattamente.
        let in_casa: Vec<&str> = [
            Materia::Compito, Materia::Guasto, Materia::Scambio,
            Materia::Memoria, Materia::Chiamata, Materia::Segreto,
        ]
        .iter()
        .filter(|m| !m.puo_uscire())
        .map(|m| m.nome())
        .collect();
        assert_eq!(in_casa, ["segreto"]);
    }
}
