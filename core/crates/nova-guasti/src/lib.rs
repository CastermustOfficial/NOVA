//! # nova-guasti
//!
//! Un guasto detto in italiano.
//!
//! E' il pezzo che serve a tutti gli altri: quando il Python sara' andato via,
//! qualunque parte di NOVA che debba dire «non ci sono riuscito» deve poterlo
//! dire come lo dice NOVA, e non come lo dice il sistema operativo.
//!
//! La regola sta scritta in architettura (D28) e vale qui piu' che altrove:
//! **il nome di una classe non e' un messaggio**. «PermissionError: [Errno
//! 13]» dice a chi legge solo che il programma e' rotto. Il dettaglio tecnico
//! non sparisce, cambia posto: va nel file dei guasti, dove serve a chi
//! ripara.
//!
//! E c'e' un pezzo che non e' cortesia ma sicurezza: [`chiavi::senza_chiavi`].
//! Il fornitore, quando la chiave e' sbagliata, la rimanda indietro dentro il
//! proprio errore, e da li' finiva in chat e nel registro.

pub mod chiavi;

// Cosa non entra in memoria. Domanda diversa da «cosa non finisce nei log»,
// e con un costo dello sbaglio diverso: le forme sono le stesse, la mano no.
pub mod guardiano;
// Un codice HTTP detto in italiano. Sta qui e non nel client del modello
// perche' la stessa risposta la ricevono tutti i cervelli a pagamento, e una
// spiegazione per fornitore vorrebbe dire cinque spiegazioni che divergono.
pub mod http;

pub use chiavi::senza_chiavi;

/// Cosa e' andato storto, nei termini in cui lo sa il chiamante.
///
/// Non e' una gerarchia di eccezioni: e' l'elenco delle cose che a NOVA
/// capitano davvero, ognuna con una frase sua. Quando non e' nessuna di
/// queste si dice il messaggio del sistema, mai il nome del suo tipo.
#[derive(Debug, Clone, PartialEq)]
pub enum Guasto<'a> {
    NonTrovato { nome: &'a str },
    EUnaCartella { nome: &'a str },
    PermessoNegato { nome: &'a str },
    NessunaRisposta,
    TroppoTempo,
    NonETesto { nome: &'a str },
    JsonRotto { riga: u32, colonna: u32 },
    MancaUnaLibreria { nome: &'a str },
    MemoriaFinita,
    DiscoPieno,
    Avvitato,
    /// Un errore del sistema operativo, col suo numero di Windows se c'e'.
    DalSistema { winerror: u32, testo: &'a str },
    /// Tutto il resto: si riporta il messaggio, non il tipo.
    Altro { messaggio: &'a str },
}

/// Gli errori di Windows che hanno una frase migliore del loro numero.
pub const WINERROR: &[(u32, &str)] = &[
    (5, "Windows non me lo lascia fare (accesso negato)."),
    (32, "Il file e' aperto in un altro programma."),
    (112, "Non c'e' piu' spazio sul disco."),
    (1225, "Il computer dall'altra parte ha rifiutato la connessione."),
];

/// Quello che si importa non e' quello che si installa.
///
/// Mandare qualcuno a installare «fitz» lo manda a installare un pacchetto
/// sbagliato che esiste davvero, ed e' peggio di non dirgli niente.
pub const PACCHETTO: &[(&str, &str)] = &[
    ("fitz", "PyMuPDF"),
    ("docx", "python-docx"),
    ("PIL", "pillow"),
    ("websocket", "websocket-client"),
    ("yaml", "PyYAML"),
    ("cv2", "opencv-python"),
    ("sounddevice", "sounddevice"),
    ("faster_whisper", "faster-whisper"),
    ("pygments", "Pygments"),
];

pub fn pacchetto_di(modulo: &str) -> &str {
    PACCHETTO
        .iter()
        .find(|(m, _)| *m == modulo)
        .map(|(_, p)| *p)
        .unwrap_or(modulo)
}

fn winerror_noto(n: u32) -> Option<&'static str> {
    WINERROR.iter().find(|(k, _)| *k == n).map(|(_, t)| *t)
}

/// Il guasto in una frase. `cosa` e' quello che si stava facendo.
pub fn spiega(g: &Guasto, cosa: &str) -> String {
    let premessa = if cosa.is_empty() {
        String::new()
    } else {
        format!("{cosa}: ")
    };
    let corpo = match g {
        // Il caso piu' frequente non e' un file: e' un programma che
        // l'installatore dava per presente.
        Guasto::NonTrovato { nome } if !nome.is_empty() => format!("non trovo «{nome}»."),
        Guasto::NonTrovato { .. } => "non trovo quello che cercavo.".to_string(),
        Guasto::EUnaCartella { nome } => format!("«{nome}» e' una cartella, non un file."),
        Guasto::PermessoNegato { nome } => {
            let chi = if nome.is_empty() {
                "permesso negato: ".to_string()
            } else {
                format!("non posso toccare «{nome}»: ")
            };
            chi + "di solito e' aperto in un altro programma, oppure sta in una \
                   cartella che Windows protegge."
        }
        Guasto::NessunaRisposta => "non risponde nessuno dall'altra parte. Se e' il \
                                    modello, probabilmente e' spento."
            .to_string(),
        Guasto::TroppoTempo => "ci ha messo troppo e ho smesso di aspettare.".to_string(),
        Guasto::NonETesto { nome } => format!(
            "«{nome}» non e' testo, o e' scritto in una codifica che non riconosco."
        ),
        Guasto::JsonRotto { riga, colonna } => {
            format!("il file non e' JSON valido (riga {riga}, colonna {colonna}).")
        }
        Guasto::MancaUnaLibreria { nome } if nome.is_empty() => {
            "manca una libreria, e non so dire quale.".to_string()
        }
        Guasto::MancaUnaLibreria { nome } => format!(
            "manca «{nome}». Si installa con «pip install {}».",
            pacchetto_di(nome)
        ),
        Guasto::MemoriaFinita => "e' finita la memoria.".to_string(),
        Guasto::DiscoPieno => "non c'e' piu' spazio sul disco.".to_string(),
        Guasto::Avvitato => "mi sono avvitato su me stesso e mi sono fermato.".to_string(),
        Guasto::DalSistema { winerror, testo } => match winerror_noto(*winerror) {
            Some(t) => t.to_string(),
            None => format!("il sistema ha detto di no ({testo})."),
        },
        Guasto::Altro { messaggio } => {
            let m = messaggio.trim();
            if m.is_empty() {
                "qualcosa e' andato storto e non so dire cosa.".to_string()
            } else {
                m.to_string()
            }
        }
    };
    // Anche qui: quello che esce non contiene chiavi. Un messaggio di sistema
    // puo' portarsi dietro un URL con dentro un token.
    senza_chiavi(&(premessa + &corpo))
}

/// Nessuno risponde all'altro capo. Le cure sono due, molto diverse.
pub fn spiega_irraggiungibile(url: &str, in_casa: bool) -> String {
    if in_casa {
        format!(
            "il modello locale non risponde su {url}. Di solito vuol dire che \
             non e' acceso: si riaccende dalle impostazioni, alla voce \
             Cervello, oppure si passa a un altro cervello."
        )
    } else {
        format!(
            "non riesco a raggiungere {url}. O e' giu' il fornitore, o questo \
             PC in questo momento non e' in rete."
        )
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn il_nome_della_classe_non_compare_mai() {
        // La guardia che conta: nessuna di queste frasi deve somigliare a
        // «PermissionError» o «[Errno 13]».
        for g in [
            Guasto::NonTrovato { nome: "x.txt" },
            Guasto::PermessoNegato { nome: "y.txt" },
            Guasto::TroppoTempo,
            Guasto::DalSistema { winerror: 5, testo: "" },
            Guasto::Altro { messaggio: "" },
        ] {
            let t = spiega(&g, "Aprendo il file");
            assert!(!t.contains("Error"), "{t}");
            assert!(!t.contains("Errno"), "{t}");
            assert!(t.starts_with("Aprendo il file: "), "{t}");
        }
    }

    #[test]
    fn senza_cosa_non_c_e_la_premessa() {
        assert_eq!(spiega(&Guasto::MemoriaFinita, ""), "e' finita la memoria.");
    }

    #[test]
    fn si_manda_a_installare_il_pacchetto_giusto() {
        // «pip install fitz» installa un pacchetto sbagliato che esiste
        // davvero: mandarci qualcuno e' peggio che non dirgli niente.
        let t = spiega(&Guasto::MancaUnaLibreria { nome: "fitz" }, "");
        assert!(t.contains("pip install PyMuPDF"), "{t}");
        // Un modulo che non sta nella tabella si installa col suo nome.
        let t = spiega(&Guasto::MancaUnaLibreria { nome: "requests" }, "");
        assert!(t.contains("pip install requests"), "{t}");
    }

    #[test]
    fn un_errore_di_windows_conosciuto_diventa_una_frase() {
        let t = spiega(&Guasto::DalSistema { winerror: 32, testo: "" }, "");
        assert!(t.contains("aperto in un altro programma"), "{t}");
        // E uno sconosciuto riporta cosa ha detto il sistema, non il numero
        // da solo.
        let t = spiega(&Guasto::DalSistema { winerror: 9999, testo: "boh" }, "");
        assert!(t.contains("boh"), "{t}");
    }

    #[test]
    fn nemmeno_un_guasto_lascia_uscire_una_chiave() {
        // Un messaggio di sistema puo' portarsi dietro un URL con un token.
        let t = spiega(
            &Guasto::Altro {
                messaggio: "chiamata a https://x/v1?api_key=abcdefghijklmnop1234 fallita",
            },
            "",
        );
        assert!(!t.contains("abcdefgh"), "{t}");
    }

    #[test]
    fn il_modello_spento_e_la_rete_giu_sono_due_frasi_diverse() {
        let a = spiega_irraggiungibile("http://127.0.0.1:8080", true);
        let b = spiega_irraggiungibile("https://api.esempio.it", false);
        assert!(a.contains("non e' acceso"), "{a}");
        assert!(b.contains("non e' in rete"), "{b}");
        assert_ne!(a, b);
    }
}
