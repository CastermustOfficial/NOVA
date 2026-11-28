//! Gli orecchi lontani (D455): il microfono di un dispositivo del portale.
//!
//! Deciso con Gio il 28 novembre: sul tablet si parla, e ascoltano tutti e
//! due, il PC e il tablet. Il dispositivo manda il suo microfono a pezzi
//! (`nova_portale::voce`); qui i pezzi diventano un orecchio che il ciclo del
//! risveglio ascolta con la stessa regola del microfono del PC
//! (`nova_voce::ascolta_dai_raccolti`): chi trascrive, chi riconosce la frase
//! e chi parla col cervello non sa da dove arrivano i campioni.
//!
//! Un orecchio nasce al primo pezzo che arriva da un dispositivo e muore
//! quando il dispositivo si scollega. Di quello che sente non si tiene
//! niente: i campioni stanno in memoria finche' qualcuno li ascolta, al
//! massimo un minuto, e ogni ascolto riparte da zero.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use nova_portale::voce::FREQUENZA_DEL_MICROFONO;
use tokio::sync::mpsc;

/// Quanti secondi di campioni si tengono se nessuno sta ascoltando (NOVA
/// parla, o la conversazione ce l'ha un altro orecchio). Un ascolto dura al
/// massimo 28 secondi e comincia svuotando: il tetto non lo tocca mai.
pub const SECONDI_TENUTI: usize = 60;

/// Quanti pezzi possono aspettare la conversazione dal vivo: sei secondi.
/// Oltre, i piu' nuovi si perdono: una voce dal vivo in ritardo non serve.
const PEZZI_IN_ATTESA: usize = 64;

/// L'orecchio di un dispositivo.
pub struct Lontano {
    /// Chi e': l'identificativo del dispositivo abbinato.
    pub id: String,
    /// Come si chiama, per i registri e per chi guarda.
    pub nome: String,
    raccolti: Arc<Mutex<Vec<f32>>>,
    vivo: AtomicBool,
    /// La conversazione dal vivo, quando c'e': riceve gli stessi pezzi.
    rubinetto: Mutex<Option<mpsc::Sender<Vec<i16>>>>,
}

fn registro() -> &'static Mutex<HashMap<String, Arc<Lontano>>> {
    static R: OnceLock<Mutex<HashMap<String, Arc<Lontano>>>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(HashMap::new()))
}

/// E' arrivato un pezzo dal dispositivo `id`. Se e' il primo l'orecchio
/// nasce, e torna: chi chiama gli fa partire il ciclo del risveglio.
pub fn arriva(id: &str, nome: &str, campioni: &[i16]) -> Option<Arc<Lontano>> {
    let (orecchio, nuovo) = {
        let mut r = registro().lock().unwrap();
        match r.get(id) {
            Some(o) => (o.clone(), false),
            None => {
                let o = Arc::new(Lontano {
                    id: id.to_string(),
                    nome: nome.to_string(),
                    raccolti: Arc::new(Mutex::new(Vec::new())),
                    vivo: AtomicBool::new(true),
                    rubinetto: Mutex::new(None),
                });
                r.insert(id.to_string(), o.clone());
                (o, true)
            }
        }
    };
    orecchio.versa(campioni);
    nuovo.then_some(orecchio)
}

/// Il dispositivo si e' scollegato: il suo orecchio non c'e' piu'.
pub fn chiudi(id: &str) {
    if let Some(o) = registro().lock().unwrap().remove(id) {
        o.vivo.store(false, Ordering::SeqCst);
        o.rubinetto.lock().unwrap().take();
        o.raccolti.lock().unwrap().clear();
    }
}

/// Gli orecchi aperti adesso: chi sono e come si chiamano.
pub fn aperti() -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = registro()
        .lock()
        .unwrap()
        .values()
        .map(|o| (o.id.clone(), o.nome.clone()))
        .collect();
    v.sort();
    v
}

impl Lontano {
    /// Il dispositivo e' ancora collegato.
    pub fn vivo(&self) -> bool {
        self.vivo.load(Ordering::SeqCst)
    }

    fn versa(&self, campioni: &[i16]) {
        {
            let mut v = self.raccolti.lock().unwrap();
            v.extend(campioni.iter().map(|c| *c as f32 / 32768.0));
            let tetto = FREQUENZA_DEL_MICROFONO as usize * SECONDI_TENUTI;
            if v.len() > tetto {
                let di_troppo = v.len() - tetto;
                v.drain(..di_troppo);
            }
        }
        let mut r = self.rubinetto.lock().unwrap();
        if let Some(tx) = r.as_ref() {
            match tx.try_send(campioni.to_vec()) {
                Ok(()) | Err(mpsc::error::TrySendError::Full(_)) => {}
                // La conversazione e' finita: il rubinetto si chiude.
                Err(mpsc::error::TrySendError::Closed(_)) => *r = None,
            }
        }
    }

    /// Ascolta una frase da questo orecchio, come `ascolta_con_attesa` fa
    /// col microfono del PC. Blocca: va chiamata fuori dal giro asincrono.
    /// Comincia da adesso: quello che e' arrivato prima si butta.
    pub fn ascolta(
        &self,
        attesa_inizio_s: f32,
        massimo_s: f32,
        silenzio_s: f32,
        frequenza: u32,
    ) -> anyhow::Result<nova_voce::Ascolto> {
        self.raccolti.lock().unwrap().clear();
        nova_voce::ascolta_dai_raccolti(
            &self.raccolti,
            FREQUENZA_DEL_MICROFONO,
            &self.nome,
            attesa_inizio_s,
            massimo_s,
            silenzio_s,
            frequenza,
        )
    }

    /// I pezzi di questo orecchio da adesso in poi, per la conversazione dal
    /// vivo: a 16 kHz, come arrivano. Uno alla volta: chi lo chiede di nuovo
    /// chiude quello di prima.
    pub fn dal_vivo(&self) -> mpsc::Receiver<Vec<i16>> {
        let (tx, rx) = mpsc::channel(PEZZI_IN_ATTESA);
        *self.rubinetto.lock().unwrap() = Some(tx);
        rx
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    /// Un pezzo da 100 ms: silenzio col fruscio di una stanza, o un tono.
    fn pezzo(n: usize, forte: bool) -> Vec<i16> {
        (0..1600usize)
            .map(|k| {
                let t = (n * 1600 + k) as f32;
                let fruscio = (((n * 1600 + k) * 7919 % 13) as f32 - 6.0) * 4.0;
                let voce = if forte {
                    (t * 220.0 * std::f32::consts::TAU / 16_000.0).sin() * 6000.0
                } else {
                    0.0
                };
                (fruscio + voce) as i16
            })
            .collect()
    }

    #[test]
    fn l_orecchio_nasce_al_primo_pezzo_e_muore_quando_il_dispositivo_esce() {
        let id = "prova-nasce";
        assert!(!aperti().iter().any(|(i, _)| i == id));
        let o = arriva(id, "Tab A8", &pezzo(0, false)).expect("al primo pezzo nasce");
        assert_eq!((o.id.as_str(), o.nome.as_str()), (id, "Tab A8"));
        assert!(o.vivo());
        assert!(
            arriva(id, "Tab A8", &pezzo(1, false)).is_none(),
            "al secondo c'e' gia'"
        );
        assert!(aperti().contains(&(id.to_string(), "Tab A8".to_string())));
        chiudi(id);
        assert!(!o.vivo());
        assert!(!aperti().iter().any(|(i, _)| i == id));
        assert!(
            o.raccolti.lock().unwrap().is_empty(),
            "e di quello che ha sentito non resta niente"
        );
        // Ricollegato, e' un orecchio nuovo.
        assert!(arriva(id, "Tab A8", &pezzo(0, false)).is_some());
        chiudi(id);
    }

    #[test]
    fn senza_nessuno_che_ascolta_si_tiene_al_piu_un_minuto() {
        let id = "prova-tetto";
        let o = arriva(id, "tablet", &pezzo(0, false)).unwrap();
        for n in 1..(SECONDI_TENUTI * 10 + 50) {
            arriva(id, "tablet", &pezzo(n, false));
        }
        assert_eq!(o.raccolti.lock().unwrap().len(), 16_000 * SECONDI_TENUTI);
        chiudi(id);
    }

    /// La frase detta al tablet si ritaglia con la regola del microfono del
    /// PC, e quello che era arrivato prima dell'ascolto non c'entra.
    #[test]
    fn una_frase_detta_al_dispositivo_si_ascolta_come_dal_microfono_del_pc() {
        let id = "prova-frase";
        // Prima dell'ascolto: un urlo che non deve finire nella frase.
        let o = arriva(id, "tablet", &pezzo(0, true)).unwrap();
        let manda = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(300));
            // Un secondo di stanza, un secondo di voce, poi la stanza.
            for n in 0..50 {
                arriva(id, "tablet", &pezzo(n + 1, (10..20).contains(&n)));
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        });
        let a = o.ascolta(5.0, 10.0, 0.6, 16_000).unwrap();
        assert!(a.ha_parlato && a.fermato_dal_silenzio);
        assert_eq!(a.microfono, "tablet");
        let secondi = a.campioni.len() as f32 / 16_000.0;
        assert!((1.5..4.0).contains(&secondi), "{secondi} s");
        manda.join().unwrap();
        chiudi(id);
    }

    #[tokio::test]
    async fn la_conversazione_dal_vivo_riceve_i_pezzi_cosi_come_arrivano() {
        let id = "prova-dal-vivo";
        let o = arriva(id, "tablet", &pezzo(0, false)).unwrap();
        let mut rx = o.dal_vivo();
        arriva(id, "tablet", &pezzo(1, true));
        arriva(id, "tablet", &pezzo(2, false));
        assert_eq!(rx.recv().await.unwrap(), pezzo(1, true));
        assert_eq!(rx.recv().await.unwrap(), pezzo(2, false));
        // Chi lo chiede di nuovo chiude quello di prima.
        let mut nuovo = o.dal_vivo();
        arriva(id, "tablet", &pezzo(3, false));
        assert!(rx.recv().await.is_none());
        assert_eq!(nuovo.recv().await.unwrap(), pezzo(3, false));
        // Finita la conversazione i pezzi continuano ad arrivare senza danno.
        drop(nuovo);
        arriva(id, "tablet", &pezzo(4, false));
        arriva(id, "tablet", &pezzo(5, false));
        assert!(o.rubinetto.lock().unwrap().is_none());
        // E quando il dispositivo esce, chi ascoltava dal vivo lo sa.
        let mut ultimo = o.dal_vivo();
        chiudi(id);
        assert!(ultimo.recv().await.is_none());
    }
}
