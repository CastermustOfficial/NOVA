//! Due modi di non farcela, e cosa si fa in ciascuno.
//!
//! **Sbattere contro un muro**: N chiamate di fila che falliscono. Qui si sale
//! di gradino, cioe' si passa il compito a un modello piu' capace. E' la
//! decisione piu' cara che NOVA prende da sola — un gradino piu' su spesso
//! vuol dire che il compito **esce dal PC** — quindi va presa con dei numeri,
//! non con un'impressione.
//!
//! **Girare a vuoto**: la stessa chiamata, con gli stessi argomenti, piu' e
//! piu' volte, e magari riuscendo ogni volta. Qui non c'e' niente da far
//! salire: c'e' da far **notare**. La differenza e' importante e sta nel
//! codice, non nei commenti: la ripetizione produce un promemoria, mai un
//! divieto. La decisione — riprovare diversamente, cercare altrove, o
//! concludere — resta al modello, e una ripetizione legittima non viene
//! bloccata da niente.
//!
//! ## Cosa sta fuori, e perche'
//!
//! Gli argomenti di una chiamata arrivano qui **gia' resi in testo**. Non e'
//! pigrizia: renderli sarebbe serializzazione, non decisione, e due
//! serializzatori diversi — `json.dumps` di Python e `serde_json` — scrivono
//! lo stesso oggetto con spaziature diverse. Inseguire l'uguaglianza a byte
//! di una stringa che non esce mai dal processo sarebbe lavoro speso per
//! niente, e sarebbe anche il posto sbagliato dove metterlo. Qui si decide
//! *se* e *cosa* dire; chi chiama porta il testo.

/// Le manopole del routing, con i valori di fabbrica di `routing_predefinito()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Manopole {
    pub automatica: bool,
    pub fallimenti_prima_di_salire: u32,
    pub passi_prima_di_salire: u32,
    pub salite_massime: u32,
}

impl Default for Manopole {
    fn default() -> Self {
        Self {
            automatica: true,
            fallimenti_prima_di_salire: 2,
            passi_prima_di_salire: 4,
            salite_massime: 2,
        }
    }
}

/// Se e' il momento di passare la palla a un gradino piu' alto.
///
/// `passi` e' il numero di giri fatti finora: e' il secondo sintomo, quello
/// che fa davvero il modello locale — continuare a chiamare strumenti senza
/// mai arrivare a una risposta. Un `passi_prima_di_salire` a zero lo spegne,
/// e va spento e non «messo molto alto», se no la soglia esiste comunque.
pub fn serve_salire(m: &Manopole, fallimenti: u32, salite: u32, passi: u32) -> bool {
    if !m.automatica {
        return false;
    }
    if salite >= m.salite_massime {
        return false;
    }
    if fallimenti >= m.fallimenti_prima_di_salire {
        return true;
    }
    m.passi_prima_di_salire != 0 && passi >= m.passi_prima_di_salire
}

/// Alla terza, alla quinta e all'ottava di fila si dice qualcosa. In mezzo no.
///
/// Non a ogni ripetizione: un promemoria a ogni giro diventa rumore, e il
/// rumore si impara a saltare. Tre soglie e poi silenzio.
pub const SOGLIE_RIPETIZIONE: [u32; 3] = [3, 5, 8];

/// Gli strumenti di servizio non spezzano la catena.
///
/// Se contassero, basterebbe un `get_datetime` in mezzo per ripulire un ciclo
/// e renderlo invisibile — cioe' il modo piu' facile di girare a vuoto senza
/// che nessuno lo dica.
pub const TRASPARENTI: [&str; 3] = ["get_datetime", "kb_stats", "modelli"];

pub fn trasparente(nome: &str) -> bool {
    TRASPARENTI.contains(&nome)
}

/// Il promemoria per la `n`-esima ripetizione di fila, se ne va detto uno.
///
/// `breve` sono gia' gli argomenti resi in testo e accorciati da chi chiama.
pub fn promemoria(n: u32, nome: &str, breve: &str) -> Option<String> {
    if !SOGLIE_RIPETIZIONE.contains(&n) {
        return None;
    }
    if n == SOGLIE_RIPETIZIONE[0] {
        return Some(format!(
            "\n\n[nota di sistema] Hai chiamato {n} volte di fila la stessa \
             cosa con gli stessi argomenti. Rileggi il risultato che hai \
             gia': se non ti sta dando quello che cerchi, cambia strada o \
             concludi con quello che sai."
        ));
    }
    Some(format!(
        "\n\n[nota di sistema] «{nome}» con gli stessi argomenti per la \
         {n}ª volta di fila ({breve}). Continuare a ripeterla non cambiera' \
         il risultato. Rileggi cosa ti ha gia' risposto, poi prova un \
         approccio diverso oppure rispondi all'utente con quello che hai."
    ))
}

/// Quante volte di fila e' arrivata la stessa identica chiamata.
#[derive(Debug, Default, Clone)]
pub struct Contatore {
    ultima: Option<String>,
    quante: u32,
}

impl Contatore {
    pub fn nuovo() -> Self {
        Self::default()
    }

    /// L'impronta e' nome piu' argomenti resi: due chiamate con gli stessi
    /// argomenti in ordine diverso devono avere la stessa impronta, e questo
    /// e' compito di chi rende gli argomenti (in Python, `sort_keys=True`).
    pub fn impronta(nome: &str, argomenti_resi: &str) -> String {
        format!("{nome}\u{0}{argomenti_resi}")
    }

    /// Guarda una chiamata e dice cosa eventualmente ricordare al modello.
    ///
    /// Uno strumento trasparente esce **prima** di toccare il contatore: non
    /// azzera la catena e non la fa nemmeno avanzare.
    pub fn guarda(&mut self, nome: &str, argomenti_resi: &str, breve: &str) -> Option<String> {
        if trasparente(nome) {
            return None;
        }
        let impronta = Self::impronta(nome, argomenti_resi);
        if self.ultima.as_deref() == Some(impronta.as_str()) {
            self.quante += 1;
        } else {
            self.ultima = Some(impronta);
            self.quante = 1;
        }
        promemoria(self.quante, nome, breve)
    }

    pub fn quante(&self) -> u32 {
        self.quante
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn si_sale_dopo_i_fallimenti_di_fila() {
        let m = Manopole::default();
        assert!(!serve_salire(&m, 0, 0, 0));
        assert!(!serve_salire(&m, 1, 0, 0));
        assert!(serve_salire(&m, 2, 0, 0), "due fallimenti bastano");
    }

    #[test]
    fn si_sale_anche_solo_girando_a_vuoto() {
        let m = Manopole::default();
        assert!(!serve_salire(&m, 0, 0, 3));
        assert!(serve_salire(&m, 0, 0, 4), "quattro passi senza risposta");
    }

    #[test]
    fn ma_non_oltre_le_salite_massime() {
        let m = Manopole::default();
        assert!(serve_salire(&m, 9, 1, 9));
        assert!(!serve_salire(&m, 9, 2, 9), "due salite sono il tetto");
    }

    #[test]
    fn spenta_vuol_dire_spenta() {
        let m = Manopole { automatica: false, ..Manopole::default() };
        assert!(!serve_salire(&m, 99, 0, 99));
    }

    #[test]
    fn zero_passi_spegne_la_soglia_dei_passi_non_la_accende() {
        // Il caso che rovina un `>=` scritto senza pensarci: con la soglia a
        // zero, «passi >= 0» sarebbe sempre vero e si salirebbe al primo giro.
        let m = Manopole { passi_prima_di_salire: 0, ..Manopole::default() };
        assert!(!serve_salire(&m, 0, 0, 0));
        assert!(!serve_salire(&m, 0, 0, 100));
        assert!(serve_salire(&m, 2, 0, 0), "i fallimenti contano ancora");
    }

    #[test]
    fn si_parla_alla_terza_alla_quinta_e_allottava() {
        for n in 0..=10 {
            let detto = promemoria(n, "list_directory", "{}").is_some();
            assert_eq!(detto, [3, 5, 8].contains(&n), "n = {n}");
        }
    }

    #[test]
    fn la_catena_conta_solo_le_chiamate_identiche() {
        let mut c = Contatore::nuovo();
        assert!(c.guarda("list_directory", "{\"path\":\"C\"}", "…").is_none());
        assert!(c.guarda("list_directory", "{\"path\":\"C\"}", "…").is_none());
        assert!(c.guarda("list_directory", "{\"path\":\"C\"}", "…").is_some(), "terza");
        // argomenti diversi: si riparte da uno
        assert!(c.guarda("list_directory", "{\"path\":\"D\"}", "…").is_none());
        assert_eq!(c.quante(), 1);
    }

    #[test]
    fn uno_strumento_di_servizio_non_ripulisce_il_ciclo() {
        let mut c = Contatore::nuovo();
        c.guarda("list_directory", "a", "…");
        c.guarda("list_directory", "a", "…");
        // in mezzo passa un tool trasparente: non deve azzerare niente
        assert!(c.guarda("get_datetime", "{}", "…").is_none());
        assert_eq!(c.quante(), 2, "il contatore non si e' mosso");
        assert!(c.guarda("list_directory", "a", "…").is_some(), "e la terza arriva");
    }

    #[test]
    fn dopo_lottava_si_tace() {
        let mut c = Contatore::nuovo();
        let mut detti = 0;
        for _ in 0..15 {
            if c.guarda("read_file", "x", "…").is_some() {
                detti += 1;
            }
        }
        assert_eq!(detti, 3, "tre promemoria in quindici giri, poi silenzio");
    }
}
