//! Quando tocca di nuovo.
//!
//! Traduce le frasi che scrive una persona — «ogni giorno 08:00», «ogni
//! lunedi' 09:00», «ogni 30 minuti», «ogni ora» — nel prossimo istante in cui
//! l'attivita' va fatta.
//!
//! Due scelte che vengono dal Python e restano, perche' non sono dettagli.
//!
//! **Una frase che non si capisce e' un errore, non un valore di ripiego.**
//! Se «ogni martedhi» diventasse silenziosamente «fra un'ora», l'utente
//! avrebbe una pianificazione che parte quando non deve e non parte quando
//! deve, e non lo saprebbe mai. Meglio rifiutarla mentre la scrive.
//!
//! **Niente fusi orari.** Si lavora sull'ora che l'utente legge
//! sull'orologio, `DataOra` senza fuso, e si torna la stessa cosa. La
//! conversione in timestamp e' una domanda di piattaforma e sta fuori: dentro
//! resta la parte che non cambia mai, e che si puo' provare senza aspettare
//! le due di notte dell'ultima domenica di ottobre.

use nova_calendario::DataOra;

/// I nomi dei giorni, con lunedi' = 0 come in Python. Con e senza accento,
/// perche' chi scrive in fretta l'accento lo salta e non e' un errore suo.
pub const GIORNI: &[(&str, u32)] = &[
    ("lunedi", 0),
    ("lunedì", 0),
    ("martedi", 1),
    ("martedì", 1),
    ("mercoledi", 2),
    ("mercoledì", 2),
    ("giovedi", 3),
    ("giovedì", 3),
    ("venerdi", 4),
    ("venerdì", 4),
    ("sabato", 5),
    ("domenica", 6),
];

/// L'ora di default quando la frase dice un giorno ma non un orario.
pub const ORA_PREDEFINITA: (u32, u32) = (9, 0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errore {
    Vuoto,
    OrarioImpossibile { ora: u32, minuto: u32 },
    NonCapisco(String),
}

impl std::fmt::Display for Errore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Errore::Vuoto => write!(f, "manca il «quando»"),
            Errore::OrarioImpossibile { ora, minuto } => {
                write!(f, "orario impossibile: {ora}:{minuto:02}")
            }
            Errore::NonCapisco(q) => write!(
                f,
                "non capisco «{q}». Esempi: «ogni giorno 08:00», \
                 «ogni lunedi 09:00», «ogni 30 minuti», «ogni ora»"
            ),
        }
    }
}

impl std::error::Error for Errore {}

/// Il primo `HH:MM` o `HH.MM` che compare, come lo trova il Python.
///
/// Si accetta anche il punto perche' in italiano «alle 8.30» si scrive cosi'
/// almeno quanto «8:30».
fn orario(q: &str) -> Option<(u32, u32)> {
    let b: Vec<char> = q.chars().collect();
    let mut i = 0;
    while i < b.len() {
        if !b[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        // una o due cifre, poi il separatore, poi esattamente due cifre
        let inizio = i;
        let mut fine = i;
        while fine < b.len() && b[fine].is_ascii_digit() && fine - inizio < 2 {
            fine += 1;
        }
        if fine < b.len()
            && (b[fine] == ':' || b[fine] == '.')
            && fine + 2 < b.len()
            && b[fine + 1].is_ascii_digit()
            && b[fine + 2].is_ascii_digit()
        {
            let h: u32 = b[inizio..fine].iter().collect::<String>().parse().ok()?;
            let m: u32 = b[fine + 1..fine + 3].iter().collect::<String>().parse().ok()?;
            return Some((h, m));
        }
        // non era un orario: si riparte dopo le cifre appena guardate
        i = fine.max(inizio + 1);
    }
    None
}

/// «ogni 30 minuti» / «ogni 2 ore» -> (quanti, e' in minuti).
///
/// Il Python cerca `ogni\s+(\d+)\s*(minut|or)`: il prefisso, non la parola
/// intera, cosi' prende «minuto», «minuti», «ora», «ore». Qui si fa lo
/// stesso, perche' una differenza qui sarebbe una frase che funziona da una
/// parte e non dall'altra.
fn ogni_quanto(q: &str) -> Option<(i64, bool)> {
    let b: Vec<char> = q.chars().collect();
    let ogni: Vec<char> = "ogni".chars().collect();
    let mut i = 0;
    while i + ogni.len() <= b.len() {
        if b[i..i + ogni.len()] != ogni[..] {
            i += 1;
            continue;
        }
        let mut j = i + ogni.len();
        // \s+
        let spazi = j;
        while j < b.len() && b[j].is_whitespace() {
            j += 1;
        }
        if j == spazi {
            i += 1;
            continue;
        }
        // (\d+)
        let cifre = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j == cifre {
            i += 1;
            continue;
        }
        let n: i64 = b[cifre..j].iter().collect::<String>().parse().unwrap_or(1);
        // \s*
        while j < b.len() && b[j].is_whitespace() {
            j += 1;
        }
        let resto: String = b[j..].iter().collect();
        if resto.starts_with("minut") {
            return Some((n.max(1), true));
        }
        if resto.starts_with("or") {
            return Some((n.max(1), false));
        }
        i += 1;
    }
    None
}

/// Traduce la frase nel prossimo istante in cui tocca.
///
/// `da` e' il momento da cui si guarda avanti: si passa da fuori, cosi' la
/// funzione non ha un orologio dentro e si prova a qualunque ora.
pub fn prossimo(quando: &str, da: DataOra) -> Result<DataOra, Errore> {
    let q = quando.trim().to_lowercase();
    if q.is_empty() {
        return Err(Errore::Vuoto);
    }

    // Prima gli intervalli: «ogni 30 minuti» non ha un orario di riferimento,
    // si conta dal momento in cui si guarda.
    if let Some((n, minuti)) = ogni_quanto(&q) {
        return Ok(da.piu_minuti(if minuti { n } else { n * 60 }));
    }
    if q.split_whitespace().collect::<Vec<_>>() == ["ogni", "minuto"] {
        return Ok(da.piu_minuti(1));
    }
    if q.split_whitespace().collect::<Vec<_>>() == ["ogni", "ora"] {
        return Ok(da.piu_minuti(60));
    }

    let trovato = orario(&q);
    let (h, mi) = trovato.unwrap_or(ORA_PREDEFINITA);
    if h > 23 || mi > 59 {
        return Err(Errore::OrarioImpossibile { ora: h, minuto: mi });
    }

    let giorno = GIORNI
        .iter()
        .find(|(nome, _)| q.contains(nome))
        .map(|(_, n)| *n);

    let bersaglio = da.con_orario(h, mi);

    let Some(giorno) = giorno else {
        // Senza un giorno della settimana serve almeno un indizio che si
        // stia parlando di un orario ricorrente, se no la frase e' altro.
        if q.contains("giorno") || q.contains("ogni") || trovato.is_some() {
            return Ok(if bersaglio <= da {
                bersaglio.piu_giorni(1)
            } else {
                bersaglio
            });
        }
        return Err(Errore::NonCapisco(quando.trim().to_string()));
    };

    let mut avanti = (giorno as i64 - bersaglio.giorno_settimana() as i64).rem_euclid(7);
    if avanti == 0 && bersaglio <= da {
        avanti = 7;
    }
    Ok(bersaglio.piu_giorni(avanti))
}

#[cfg(test)]
mod prove {
    use super::*;

    fn d(s: &str) -> DataOra {
        DataOra::da_iso(s).unwrap()
    }

    #[test]
    fn gli_intervalli_si_contano_da_adesso() {
        let ora = d("2026-09-02T10:00:00");
        assert_eq!(prossimo("ogni 30 minuti", ora).unwrap().iso(), "2026-09-02T10:30:00");
        assert_eq!(prossimo("ogni 2 ore", ora).unwrap().iso(), "2026-09-02T12:00:00");
        assert_eq!(prossimo("ogni minuto", ora).unwrap().iso(), "2026-09-02T10:01:00");
        assert_eq!(prossimo("ogni ora", ora).unwrap().iso(), "2026-09-02T11:00:00");
    }

    #[test]
    fn ogni_giorno_salta_a_domani_se_e_gia_passato() {
        let ora = d("2026-09-02T10:00:00");
        assert_eq!(prossimo("ogni giorno 08:00", ora).unwrap().iso(), "2026-09-03T08:00:00");
        assert_eq!(prossimo("ogni giorno 18:30", ora).unwrap().iso(), "2026-09-02T18:30:00");
    }

    #[test]
    fn il_giorno_della_settimana_va_avanti_di_una_settimana_se_e_oggi_e_passato() {
        // Il 2 settembre 2026 e' mercoledi'.
        let ora = d("2026-09-02T10:00:00");
        assert_eq!(prossimo("ogni mercoledi 08:00", ora).unwrap().iso(), "2026-09-09T08:00:00");
        assert_eq!(prossimo("ogni mercoledi 18:00", ora).unwrap().iso(), "2026-09-02T18:00:00");
        assert_eq!(prossimo("ogni lunedi 09:00", ora).unwrap().iso(), "2026-09-07T09:00:00");
        assert_eq!(prossimo("ogni domenica 09:00", ora).unwrap().iso(), "2026-09-06T09:00:00");
    }

    #[test]
    fn laccento_non_cambia_niente() {
        let ora = d("2026-09-02T10:00:00");
        assert_eq!(
            prossimo("ogni lunedì 09:00", ora).unwrap(),
            prossimo("ogni lunedi 09:00", ora).unwrap()
        );
    }

    #[test]
    fn senza_orario_si_usano_le_nove() {
        let ora = d("2026-09-02T10:00:00");
        assert_eq!(prossimo("ogni giovedi", ora).unwrap().iso(), "2026-09-03T09:00:00");
    }

    #[test]
    fn una_frase_che_non_si_capisce_e_un_errore() {
        let ora = d("2026-09-02T10:00:00");
        assert_eq!(prossimo("", ora), Err(Errore::Vuoto));
        assert_eq!(prossimo("   ", ora), Err(Errore::Vuoto));
        assert!(matches!(prossimo("quando ti pare", ora), Err(Errore::NonCapisco(_))));
        assert_eq!(
            prossimo("ogni giorno 99:99", ora),
            Err(Errore::OrarioImpossibile { ora: 99, minuto: 99 })
        );
    }

    #[test]
    fn il_punto_vale_come_i_due_punti() {
        let ora = d("2026-09-02T10:00:00");
        assert_eq!(prossimo("ogni giorno 8.30", ora).unwrap().iso(), "2026-09-03T08:30:00");
    }

    #[test]
    fn il_prossimo_e_sempre_avanti() {
        // La proprieta' che conta davvero: qualunque frase valida, da
        // qualunque momento, deve dare un istante nel futuro. Se ne desse uno
        // nel passato l'attivita' partirebbe subito e in continuazione.
        let momenti = [
            "2026-09-02T00:00:00",
            "2026-09-02T08:59:59",
            "2026-09-02T09:00:00",
            "2026-12-31T23:59:00",
            "2024-02-28T23:30:00",
        ];
        let frasi = [
            "ogni giorno 09:00",
            "ogni lunedi 09:00",
            "ogni domenica 23:59",
            "ogni 30 minuti",
            "ogni ora",
            "ogni 1 minuti",
        ];
        for m in momenti {
            for f in frasi {
                let da = d(m);
                let p = prossimo(f, da).unwrap();
                assert!(p > da, "«{f}» da {m} ha dato {} che non e' avanti", p.iso());
            }
        }
    }
}
