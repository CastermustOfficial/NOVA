//! La pulizia del microfono (D446): via il rumore, e silenzio vero quando
//! nessuno parla.
//!
//! Il 19 novembre, in una prova con Gemini Live, la trascrizione ha scritto
//! «¿Qué» e «saber» mentre Gio non parlava, e NOVA si e' interrotta due volte
//! a meta' frase per scusarsi. Il microfono andava al server com'era, col
//! guadagno automatico che porta un sussurro verso il mezzo volume: il
//! rumore della stanza, gonfiato fino a venti volte, arrivava come una voce.
//! Il riconoscimento del parlato del server lo prendeva per l'utente che
//! parla sopra, e la trascrizione ci trovava delle parole.
//!
//! Due pezzi, uno dopo l'altro:
//!
//! - **il rumore si toglie** con RNNoise (Xiph), nella riscrittura in Rust
//!   `nnnoiseless`: una rete piccola, coi pesi nel binario, che lavora su
//!   pezzi da 10 ms a 48 kHz e per ognuno dice anche quanto e' probabile che
//!   ci sia una voce;
//! - **il cancello** ([`Cancello`]) usa quella probabilita': finche' nessuno
//!   parla escono zeri, cioe' silenzio vero. Si apre un attimo prima della
//!   voce (i pezzi aspettano [`ANTICIPO`] prima di uscire, cosi' l'attacco
//!   della parola non si perde) e resta aperto [`TENUTA`] dopo l'ultima
//!   voce, perche' la coda di una parola e le pause fra le parole non sono
//!   silenzio.
//!
//! Il cancello e' una regola pura, e si prova senza audio. La rete si prova
//! col rumore: una voce vera in una prova non ci sta, e le misure con la
//! voce stanno in `analisi/`.

use std::collections::VecDeque;

use nnnoiseless::DenoiseState;

/// La frequenza a cui lavora RNNoise, e nessun'altra.
pub const FREQUENZA: u32 = 48_000;

/// Un pezzo di RNNoise: 480 campioni, 10 ms.
pub const PEZZO: usize = DenoiseState::FRAME_SIZE;

/// Quanti pezzi (da 10 ms) aspettano prima di uscire: se intanto arriva una
/// voce, escono anche loro. E' il ritardo che la pulizia aggiunge.
pub const ANTICIPO: usize = 4;

/// Per quanti pezzi il cancello resta aperto dopo l'ultima voce.
pub const TENUTA: usize = 40;

/// La probabilita' di voce che apre il cancello.
pub const APRE: f32 = 0.75;

/// La probabilita' di voce che lo tiene aperto, una volta aperto.
pub const TIENE: f32 = 0.4;

/// Quanti pezzi di fila sopra [`APRE`] servono per aprire: un colpo solo
/// (un tasto, un clic) non e' una voce.
pub const DI_FILA: usize = 3;

/// Il cancello: dalla probabilita' di voce di ogni pezzo, in ordine, dice
/// se il pezzo di [`ANTICIPO`] pezzi prima esce o diventa silenzio.
#[derive(Debug, Clone, Default)]
pub struct Cancello {
    /// Quanti pezzi sono arrivati.
    arrivati: u64,
    /// L'ultimo pezzo con una voce, se c'e' stato.
    ultima_voce: Option<u64>,
    /// Quanti pezzi di fila sopra [`APRE`], adesso.
    di_fila: usize,
}

impl Cancello {
    /// Arriva la probabilita' di un pezzo. Torna `None` finche' non ci sono
    /// abbastanza pezzi in attesa; poi, per il pezzo di [`ANTICIPO`] pezzi
    /// prima, se esce.
    pub fn arriva(&mut self, probabilita: f32) -> Option<bool> {
        let t = self.arrivati;
        self.arrivati += 1;
        let p = if probabilita.is_finite() {
            probabilita
        } else {
            0.0
        };
        let aperto = self.ultima_voce.is_some_and(|v| t - v <= TENUTA as u64);
        if p >= APRE {
            self.di_fila += 1;
        } else {
            self.di_fila = 0;
        }
        if self.di_fila >= DI_FILA {
            // I pezzi di fila prima di questo escono con l'anticipo, che e'
            // piu' lungo: l'attacco della parola c'e' tutto.
            self.ultima_voce = Some(t);
        } else if aperto && p >= TIENE {
            self.ultima_voce = Some(t);
        }
        // Il pezzo che esce adesso e' quello di ANTICIPO pezzi fa.
        let d = t.checked_sub(ANTICIPO as u64)?;
        // L'ultima voce e' al piu' `t`, cioe' dentro l'anticipo: basta che
        // non sia piu' vecchia della tenuta.
        Some(self.ultima_voce.is_some_and(|v| v + TENUTA as u64 >= d))
    }
}

/// Quanto ha fatto la pulizia, per il registro e per le misure.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Conto {
    /// I pezzi da 10 ms usciti.
    pub pezzi: u64,
    /// Di questi, quelli usciti con la voce: gli altri sono silenzio.
    pub aperti: u64,
}

/// Quello che la rete ha visto in un pezzo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Visto {
    /// Quanto e' probabile che ci sia una voce, da 0 a 1.
    pub probabilita: f32,
    /// Quanto e' forte il pezzo com'e' arrivato (la radice della media dei
    /// quadrati, nella scala fra 0 e 1).
    pub dentro: f32,
    /// Quanto e' forte dopo che la rete ha tolto il rumore.
    pub pulito: f32,
}

/// Quanto e' forte un pezzo: la radice della media dei quadrati.
fn forza(pezzo: &[f32]) -> f32 {
    (pezzo.iter().map(|c| c * c).sum::<f32>() / pezzo.len().max(1) as f32).sqrt()
}

/// Il microfono pulito: campioni a 48 kHz in entrata, in virgola fra -1 e 1,
/// e in uscita gli stessi campioni senza rumore, [`ANTICIPO`] pezzi dopo, a
/// zero dove nessuno parla.
pub struct Pulitore {
    rete: Box<DenoiseState<'static>>,
    cancello: Cancello,
    /// I campioni arrivati che non fanno ancora un pezzo intero.
    resto: Vec<f32>,
    /// I pezzi puliti che aspettano il cancello.
    attesa: VecDeque<[f32; PEZZO]>,
    /// Il primo pezzo della rete ha un attacco sporco: esce a zero.
    primo: bool,
    conto: Conto,
}

impl Default for Pulitore {
    fn default() -> Self {
        Self::nuovo()
    }
}

impl Pulitore {
    pub fn nuovo() -> Pulitore {
        Pulitore {
            rete: DenoiseState::new(),
            cancello: Cancello::default(),
            resto: Vec::with_capacity(PEZZO),
            attesa: VecDeque::with_capacity(ANTICIPO + 1),
            primo: true,
            conto: Conto::default(),
        }
    }

    /// Quanto ha fatto finora.
    pub fn conto(&self) -> Conto {
        self.conto
    }

    /// Pulisce i campioni che arrivano e torna quelli pronti: un numero
    /// intero di pezzi, in ritardo di [`ANTICIPO`] pezzi. I campioni che non
    /// fanno un pezzo aspettano la chiamata dopo.
    pub fn pulisci(&mut self, campioni: &[f32]) -> Vec<f32> {
        self.pulisci_e_guarda(campioni, |_| {})
    }

    /// Come [`Pulitore::pulisci`], e a `guarda` arriva quello che la rete ha
    /// visto in ogni pezzo: serve alle misure.
    pub fn pulisci_e_guarda(
        &mut self,
        campioni: &[f32],
        mut guarda: impl FnMut(Visto),
    ) -> Vec<f32> {
        self.resto.extend_from_slice(campioni);
        let interi = self.resto.len() / PEZZO;
        let mut fuori = Vec::with_capacity(interi * PEZZO);
        let mut dentro = [0.0f32; PEZZO];
        for pezzo in self.resto.chunks_exact(PEZZO) {
            // RNNoise vuole i campioni nella scala dei 16 bit.
            for (d, c) in dentro.iter_mut().zip(pezzo) {
                *d = if c.is_finite() {
                    c.clamp(-1.0, 1.0) * 32767.0
                } else {
                    0.0
                };
            }
            let mut pulito = [0.0f32; PEZZO];
            let probabilita = self.rete.process_frame(&mut pulito, &dentro);
            if std::mem::take(&mut self.primo) {
                pulito = [0.0; PEZZO];
            }
            guarda(Visto {
                probabilita,
                dentro: forza(&dentro) / 32767.0,
                pulito: forza(&pulito) / 32767.0,
            });
            self.attesa.push_back(pulito);
            let Some(esce) = self.cancello.arriva(probabilita) else {
                continue;
            };
            let Some(pezzo) = self.attesa.pop_front() else {
                continue;
            };
            self.conto.pezzi += 1;
            if esce {
                self.conto.aperti += 1;
                fuori.extend(pezzo.iter().map(|c| (c / 32767.0).clamp(-1.0, 1.0)));
            } else {
                fuori.extend(std::iter::repeat(0.0).take(PEZZO));
            }
        }
        self.resto.drain(..interi * PEZZO);
        fuori
    }
}

/// Il microfono della voce dal vivo, dal pezzo che arriva dalla scheda a
/// quello che va al server: pulito, se la pulizia e' accesa, e poi col
/// guadagno automatico.
///
/// Il guadagno viene **dopo** il cancello, e ricorda solo i pezzi con la
/// voce: prima veniva per primo e su tutto, e nel silenzio gonfiava il
/// rumore della stanza fino a venti volte. La rete riconosce la voce allo
/// stesso modo a ogni volume (misurato da un picco di 0,5 a uno di 0,001),
/// quindi non le serve che il guadagno venga prima.
pub struct Ingresso {
    pulitore: Option<Pulitore>,
    /// La frequenza a cui escono i pezzi.
    uscita: u32,
    picco: f32,
}

impl Ingresso {
    /// `pulisci` accende la pulizia; `uscita` e' la frequenza che vuole chi
    /// riceve. Con la pulizia l'uscita e' a 16 kHz, e nessun'altra.
    pub fn nuovo(pulisci: bool, uscita: u32) -> Ingresso {
        Ingresso {
            pulitore: (pulisci && uscita == FREQUENZA / 3).then(Pulitore::nuovo),
            uscita,
            picco: 0.0,
        }
    }

    /// Se la pulizia e' accesa.
    pub fn pulisce(&self) -> bool {
        self.pulitore.is_some()
    }

    /// La frequenza a cui chiedere i pezzi al microfono: quella di RNNoise
    /// con la pulizia, se no direttamente quella di uscita.
    pub fn frequenza(&self) -> u32 {
        if self.pulisce() {
            FREQUENZA
        } else {
            self.uscita
        }
    }

    /// Quanto ha fatto la pulizia, se e' accesa.
    pub fn conto(&self) -> Option<Conto> {
        self.pulitore.as_ref().map(Pulitore::conto)
    }

    /// Un pezzo del microfono, alla frequenza di [`Ingresso::frequenza`]:
    /// torna quello da mandare, alla frequenza di uscita. `regola` e' la
    /// regola del guadagno, dal picco ricordato (`nova_live::guadagno`).
    pub fn passa(&mut self, pezzo: Vec<f32>, regola: impl Fn(f32) -> f32) -> Vec<f32> {
        let mut fuori = match &mut self.pulitore {
            Some(p) => a_16k(&p.pulisci(&pezzo)),
            None => pezzo,
        };
        // Un pezzo di solo silenzio non si gonfia, e non fa dimenticare il
        // volume della voce.
        if !self.pulisce() || fuori.iter().any(|c| *c != 0.0) {
            self.picco = crate::dal_vivo::con_guadagno(&mut fuori, self.picco, regola);
        }
        fuori
    }
}

/// Da 48 kHz a 16 kHz, quelli che vuole la voce dal vivo: la media di ogni
/// terna, che fa anche da filtro. Un resto in fondo, che non fa una terna,
/// si butta: i pezzi del [`Pulitore`] sono multipli di tre.
pub fn a_16k(campioni: &[f32]) -> Vec<f32> {
    campioni
        .chunks_exact(3)
        .map(|t| (t[0] + t[1] + t[2]) / 3.0)
        .collect()
}

#[cfg(test)]
mod prove {
    use super::*;

    /// Le uscite del cancello per una fila di probabilita': `true` dove il
    /// pezzo esce. Con l'anticipo, l'uscita `i` e' del pezzo `i`.
    fn uscite(probabilita: &[f32]) -> Vec<bool> {
        let mut c = Cancello::default();
        let mut fuori: Vec<bool> = probabilita.iter().filter_map(|p| c.arriva(*p)).collect();
        // I pezzi ancora in attesa escono con altro silenzio dietro.
        while fuori.len() < probabilita.len() {
            if let Some(x) = c.arriva(0.0) {
                fuori.push(x);
            }
        }
        fuori
    }

    #[test]
    fn nel_silenzio_il_cancello_resta_chiuso() {
        assert!(uscite(&[0.0; 200]).iter().all(|x| !x));
        assert!(
            uscite(&[0.3; 200]).iter().all(|x| !x),
            "sotto la soglia non e' una voce"
        );
        assert!(
            uscite(&[TIENE + 0.1; 200]).iter().all(|x| !x),
            "la soglia bassa tiene aperto, non apre"
        );
        assert!(uscite(&[f32::NAN; 50]).iter().all(|x| !x));
    }

    /// Un tasto o un clic: uno o due pezzi alti, e poi niente.
    #[test]
    fn un_colpo_solo_non_apre() {
        let mut p = vec![0.0f32; 100];
        p[40] = 0.99;
        p[41] = 0.95;
        assert!(uscite(&p).iter().all(|x| !x));
        // Colpi staccati, anche tanti: mai tre di fila.
        let colpi: Vec<f32> = (0..300)
            .map(|i| if i % 5 < 2 { 0.9 } else { 0.1 })
            .collect();
        assert!(uscite(&colpi).iter().all(|x| !x));
    }

    #[test]
    fn la_voce_apre_un_attimo_prima_e_tiene_dopo() {
        let mut p = vec![0.0f32; 300];
        for x in &mut p[100..150] {
            *x = 0.95;
        }
        let u = uscite(&p);
        // La voce si riconosce al terzo pezzo di fila, il 102: con l'anticipo
        // escono anche i pezzi da 98, cioe' l'attacco intero e un po' prima.
        let primo = u.iter().position(|x| *x).unwrap();
        assert_eq!(primo, 100 + DI_FILA - 1 - ANTICIPO);
        assert!(primo <= 100, "l'attacco della parola non si perde");
        // L'ultima voce e' il pezzo 149: resta aperto per la tenuta.
        let ultimo = u.iter().rposition(|x| *x).unwrap();
        assert_eq!(ultimo, 149 + TENUTA);
        assert!(
            u[primo..=ultimo].iter().all(|x| *x),
            "in mezzo non si chiude"
        );
    }

    /// Una pausa fra due parole piu' corta della tenuta non chiude; e la
    /// coda di una parola, sopra la soglia bassa, tiene aperto.
    #[test]
    fn le_pause_corte_e_le_code_non_chiudono() {
        let mut p = vec![0.0f32; 400];
        for x in &mut p[50..80] {
            *x = 0.9;
        }
        for x in &mut p[80..140] {
            *x = TIENE + 0.05; // la coda
        }
        for x in &mut p[140 + TENUTA - 5..200 + TENUTA] {
            *x = 0.9; // riparte prima che la tenuta finisca
        }
        let u = uscite(&p);
        let primo = u.iter().position(|x| *x).unwrap();
        let ultimo = u.iter().rposition(|x| *x).unwrap();
        assert!(
            u[primo..=ultimo].iter().all(|x| *x),
            "un pezzo solo, senza buchi"
        );
        assert_eq!(ultimo, 199 + TENUTA + TENUTA);
        // Dopo una pausa lunga si richiude, e la soglia bassa non riapre.
        let mut q = vec![0.0f32; 400];
        for x in &mut q[10..40] {
            *x = 0.9;
        }
        for x in &mut q[200..260] {
            *x = TIENE + 0.05;
        }
        assert!(uscite(&q)[40 + TENUTA + 1..].iter().all(|x| !x));
    }

    /// Un rumore che non e' una voce: uniforme, da un generatore con un seme
    /// fisso, a un volume che il guadagno automatico porterebbe al server.
    fn rumore(campioni: usize, ampiezza: f32) -> Vec<f32> {
        let mut seme: u32 = 0x1234_5678;
        (0..campioni)
            .map(|_| {
                seme = seme.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                ((seme >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0) * ampiezza
            })
            .collect()
    }

    #[test]
    fn il_rumore_esce_come_silenzio() {
        let mut p = Pulitore::nuovo();
        let dentro = rumore(FREQUENZA as usize * 3, 0.05);
        let mut fuori = Vec::new();
        // A pezzi da 100 ms, come il microfono.
        for pezzo in dentro.chunks(4_800) {
            fuori.extend(p.pulisci(pezzo));
        }
        assert_eq!(
            fuori.len(),
            dentro.len() - ANTICIPO * PEZZO,
            "in ritardo dell'anticipo"
        );
        assert!(
            fuori.iter().all(|c| *c == 0.0),
            "tre secondi di fruscio, e nemmeno un campione"
        );
        assert_eq!(
            p.conto(),
            Conto {
                pezzi: 300 - ANTICIPO as u64,
                aperti: 0
            }
        );
    }

    #[test]
    fn i_campioni_che_non_fanno_un_pezzo_aspettano() {
        let mut p = Pulitore::nuovo();
        assert!(p.pulisci(&[0.0; 100]).is_empty());
        assert!(p.pulisci(&[0.0; 379]).is_empty(), "479: manca un campione");
        // Col campione che manca il primo pezzo c'e', ma aspetta il cancello.
        assert!(p.pulisci(&[0.0; 1]).is_empty());
        let fuori = p.pulisci(&vec![0.0; PEZZO * ANTICIPO + 7]);
        assert_eq!(
            fuori.len(),
            PEZZO,
            "esce il primo, e sette campioni aspettano"
        );
        assert!(p.pulisci(&[f32::NAN, f32::INFINITY, 2.0]).is_empty());
    }

    /// Il difetto del 19 novembre: il rumore, col guadagno, arrivava al
    /// server come una voce. Con la pulizia esce zero; senza, com'era.
    #[test]
    fn il_guadagno_non_gonfia_piu_il_rumore() {
        let regola = |picco: f32| {
            if picco > 0.0 {
                (0.5 / picco).clamp(1.0, 20.0)
            } else {
                1.0
            }
        };
        let fruscio = rumore(FREQUENZA as usize * 2, 0.004);
        let mut pulito = Ingresso::nuovo(true, 16_000);
        assert!(pulito.pulisce());
        assert_eq!(pulito.frequenza(), FREQUENZA);
        let mut fuori = Vec::new();
        for pezzo in fruscio.chunks(4_800) {
            fuori.extend(pulito.passa(pezzo.to_vec(), regola));
        }
        assert_eq!(
            fuori.len(),
            (fruscio.len() - ANTICIPO * PEZZO) / 3,
            "a 16 kHz"
        );
        assert!(fuori.iter().all(|c| *c == 0.0));
        assert_eq!(pulito.conto().map(|c| c.aperti), Some(0));
        // Spenta, il microfono arriva gia' a 16 kHz e passa com'era prima: il
        // fruscio a 0,004 esce venti volte piu' forte.
        let mut grezzo = Ingresso::nuovo(false, 16_000);
        assert!(!grezzo.pulisce() && grezzo.conto().is_none());
        assert_eq!(grezzo.frequenza(), 16_000);
        let uscito = grezzo.passa(rumore(1_600, 0.004), regola);
        let picco = uscito.iter().fold(0.0f32, |m, c| m.max(c.abs()));
        assert!(picco > 0.07 && picco <= 0.081, "{picco}");
        // A un'altra frequenza di uscita la pulizia non si accende.
        assert!(!Ingresso::nuovo(true, 24_000).pulisce());
    }

    #[test]
    fn da_48_a_16_si_fa_la_media_delle_terne() {
        assert_eq!(
            a_16k(&[0.25, 0.5, 0.75, -1.0, -1.0, -1.0, 0.5]),
            [0.5, -1.0]
        );
        assert_eq!(
            a_16k(&vec![0.25; 4_800]).len(),
            1_600,
            "100 ms restano 100 ms"
        );
        assert!(a_16k(&[]).is_empty());
    }
}
