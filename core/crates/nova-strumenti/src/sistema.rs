//! Le parti degli strumenti di sistema che non chiedono niente al sistema.
//!
//! Sono poche righe e sono quelle in cui sbagliare non si vede: una
//! combinazione di tasti tradotta male non da' errore, **preme altri tasti**
//! — e li preme nella finestra che ha il fuoco, che e' quella dove l'utente
//! sta lavorando.

/// I giorni, in italiano e minuscoli, con **lunedi' a zero**.
///
/// Lunedi' a zero perche' e' cio' che usa Python (`datetime.weekday()`), e
/// una convenzione diversa qui sarebbe una differenza che non si vede finche'
/// non e' domenica.
pub const GIORNI: [&str; 7] = [
    "lunedi", "martedi", "mercoledi", "giovedi", "venerdi", "sabato", "domenica",
];

/// Data e ora come le dice NOVA quando gliele si chiede.
pub fn data_e_ora(secondi: u64, fuso: &dyn crate::data::Fuso) -> String {
    let d = nova_calendario::da_istante(secondi as i64, fuso.secondi_in(secondi));
    let g = nova_calendario::giorno_settimana(d.anno, d.mese, d.giorno) as usize;
    format!(
        "{} {:02}/{:02}/{:04} {:02}:{:02}:{:02}",
        GIORNI[g.min(6)], d.giorno, d.mese, d.anno, d.ora, d.minuto, d.secondo
    )
}

/// I modificatori, come li scrive `SendKeys`.
const MODIFICATORI: &[(&str, &str)] = &[
    ("ctrl", "^"), ("control", "^"), ("alt", "%"), ("shift", "+"),
];

/// I tasti che `SendKeys` chiama per nome.
const SPECIALI: &[(&str, &str)] = &[
    ("enter", "{ENTER}"), ("esc", "{ESC}"), ("escape", "{ESC}"),
    ("tab", "{TAB}"), ("space", " "), ("backspace", "{BACKSPACE}"),
    ("delete", "{DELETE}"), ("up", "{UP}"), ("down", "{DOWN}"),
    ("left", "{LEFT}"), ("right", "{RIGHT}"), ("home", "{HOME}"),
    ("end", "{END}"),
];

/// Traduce «ctrl+shift+esc» nella forma che capisce `SendKeys`.
///
/// E' il pezzo piu' pericoloso di tutto il modulo, e non perche' sia
/// difficile: una traduzione sbagliata **non da' errore**, preme altri tasti,
/// e li preme nella finestra che ha il fuoco — cioe' quella dove l'utente sta
/// lavorando in quel momento.
///
/// Una combinazione fatta di soli modificatori non e' una combinazione: se ne
/// esce con un errore invece che premendo qualcosa a caso.
pub fn traduci_tasti(tasti: &str) -> Result<String, String> {
    let pezzi: Vec<String> = tasti
        .split('+')
        .map(|p| p.trim().to_lowercase())
        .collect();
    let modificatore = |p: &String| MODIFICATORI.iter().find(|(n, _)| n == p).map(|(_, s)| *s);
    let mods: String = pezzi.iter().filter_map(modificatore).collect();
    let resto: Vec<&String> = pezzi.iter().filter(|p| modificatore(p).is_none()).collect();
    let Some(tasto) = resto.last() else {
        return Err(format!("combinazione non valida: {tasti}"));
    };
    if tasto.is_empty() {
        return Err(format!("combinazione non valida: {tasti}"));
    }
    let scritto = match SPECIALI.iter().find(|(n, _)| n == &tasto.as_str()) {
        Some((_, s)) => s.to_string(),
        None if tasto.chars().count() == 1 => tasto.to_string(),
        None => format!("{{{}}}", tasto.to_uppercase()),
    };
    Ok(format!("{mods}{scritto}"))
}

/// Quanti passi di volume servono per arrivare a questo livello.
///
/// **Questa non e' la strada con cui NOVA cambia il volume.** Il volume si
/// chiede a Core Audio: si legge, si scrive e si rilegge (vedi il tratto
/// `Audio` in `capacita.rs`). Qui c'e' la traduzione fedele del *ripiego*,
/// quello che resta dove il binario non c'e': azzera con cinquanta colpi di
/// tasto in giu' — piu' dei venticinque che bastano, per partire da zero
/// qualunque fosse il livello — e poi risale, due punti per colpo.
///
/// Resta perche' il ripiego resta, e perche' finche' esiste dev'essere
/// identico a quello del Python: due strade che fanno «quasi» la stessa cosa
/// sono peggio di una sola. Ma chi legge questo file deve sapere che il
/// numero che esce di qui non e' un volume — e' un conto di pressioni, e il
/// volume vero nessuno l'ha letto (D132).
pub fn passi_di_volume(livello: i64) -> i64 {
    let livello = livello.clamp(0, 100);
    // `round` di Python a meta' va al pari: 25 passi e mezzo diventano 26,
    // 24 e mezzo diventano 24.
    let meta = livello as f64 / 2.0;
    let giu = meta.floor();
    let resto = meta - giu;
    let giu = giu as i64;
    if resto > 0.5 {
        giu + 1
    } else if resto < 0.5 {
        giu
    } else if giu % 2 == 0 {
        giu
    } else {
        giu + 1
    }
}

/// Un disco, per quello che se ne dice a chi guarda.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Disco {
    /// `C:\`
    pub radice: String,
    pub totale_byte: u64,
    pub liberi_byte: u64,
}

/// La batteria, quando ce n'e' una.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Batteria {
    pub percentuale: Option<u8>,
    pub alla_corrente: bool,
    pub minuti_rimasti: Option<u32>,
}

/// Com'e' fatto il PC, **in numeri**.
///
/// Non e' una copia della struttura che legge il sistema, ed e' voluto: li'
/// c'e' quello che questo sistema operativo sa dire di se', qui c'e' quello
/// di cui NOVA parla. Il passaggio fra le due sta in un posto solo, e se un
/// giorno un campo cambia nome di la' il compilatore lo chiede qui invece di
/// lasciare una riga vuota nella risposta.
///
/// I numeri restano numeri fino all'ultimo momento. Prima arrivavano gia'
/// scritti e nella lingua dell'utente — «RAM_GB: 31,1» con la virgola e, tre
/// righe sotto, «72.5GB liberi» col punto, perche' passavano da due
/// formattatori diversi di PowerShell. Chi legge quella riga e' un modello
/// che ci deve fare un conto (D137).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Macchina {
    pub sistema: String,
    pub build: u32,
    pub pc: String,
    pub cpu: String,
    pub processori: u32,
    pub ram_totale_byte: u64,
    pub ram_libera_byte: u64,
    pub dischi: Vec<Disco>,
    /// `None` su un fisso: non e' un dato mancante, e' una batteria che non
    /// c'e'. Chi legge deve poterlo dire, e la riga lo dice.
    pub batteria: Option<Batteria>,
    pub acceso_da_secondi: u64,
}

/// Com'e' fatto il PC, raccontato a chi legge.
///
/// L'allineamento a nove caratteri della radice del disco non e' estetica: le
/// righe le legge un modello insieme a tutto il resto della conversazione, e
/// una colonna che balla e' una riga in piu' da interpretare.
pub fn racconta(d: &Macchina) -> String {
    let pesa = nova_dati::pesa;
    let mut righe = vec![
        format!("Sistema       : {} (build {})", d.sistema, d.build),
        format!("PC            : {}", d.pc),
        format!(
            "CPU           : {} ({} processori logici)",
            d.cpu, d.processori
        ),
        format!(
            "RAM           : {} liberi su {}",
            pesa(d.ram_libera_byte),
            pesa(d.ram_totale_byte)
        ),
    ];
    for disco in &d.dischi {
        righe.push(format!(
            "Disco {:<9}: {} liberi su {}",
            disco.radice,
            pesa(disco.liberi_byte),
            pesa(disco.totale_byte)
        ));
    }
    match &d.batteria {
        // Il silenzio direbbe «non lo so»; qui si sa, ed e' «non ce n'e' una».
        None => righe.push("Batteria      : nessuna (e' un fisso)".to_string()),
        Some(b) => {
            let mut pezzi: Vec<String> = Vec::new();
            if let Some(p) = b.percentuale {
                pezzi.push(format!("{p}%"));
            }
            pezzi.push(
                if b.alla_corrente {
                    "alla corrente"
                } else {
                    "a batteria"
                }
                .to_string(),
            );
            if let Some(m) = b.minuti_rimasti {
                pezzi.push(format!("~{m} minuti"));
            }
            righe.push(format!("Batteria      : {}", pezzi.join(", ")));
        }
    }
    let ore = d.acceso_da_secondi / 3600;
    let minuti = (d.acceso_da_secondi % 3600) / 60;
    righe.push(format!("Acceso da     : {ore}h {minuti}m"));
    righe.join("\n")
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn le_combinazioni_si_traducono_come_le_capisce_windows() {
        assert_eq!(traduci_tasti("ctrl+s").unwrap(), "^s");
        assert_eq!(traduci_tasti("ctrl+shift+esc").unwrap(), "^+{ESC}");
        assert_eq!(traduci_tasti("alt+tab").unwrap(), "%{TAB}");
        assert_eq!(traduci_tasti("f5").unwrap(), "{F5}");
        assert_eq!(traduci_tasti("a").unwrap(), "a");
        assert_eq!(traduci_tasti("CTRL+S").unwrap(), "^s");
        assert_eq!(traduci_tasti(" ctrl + s ").unwrap(), "^s");
    }

    #[test]
    fn una_combinazione_di_soli_modificatori_non_preme_niente_a_caso() {
        // E' il caso in cui sbagliare fa danno: senza questo controllo si
        // manderebbe il solo modificatore, o peggio una stringa vuota, alla
        // finestra dove l'utente sta lavorando.
        assert!(traduci_tasti("ctrl").is_err());
        assert!(traduci_tasti("ctrl+alt").is_err());
        assert!(traduci_tasti("").is_err());
        assert!(traduci_tasti("+").is_err());
    }

    #[test]
    fn i_giorni_partono_da_lunedi() {
        // Il 1970-01-01 era un giovedi'.
        assert_eq!(data_e_ora(0, &crate::data::FusoFisso(0)), "giovedi 01/01/1970 00:00:00");
    }

    fn fissa() -> Macchina {
        Macchina {
            sistema: "Windows 11 Pro".into(),
            build: 26200,
            pc: "IL-FISSO".into(),
            cpu: "AMD Ryzen 9".into(),
            processori: 32,
            ram_totale_byte: 34_359_738_368,
            ram_libera_byte: 8_589_934_592,
            dischi: vec![Disco {
                radice: "C:\\".into(),
                totale_byte: 1_000_000_000_000,
                liberi_byte: 500_000_000_000,
            }],
            batteria: None,
            acceso_da_secondi: 3600 * 5 + 60 * 7 + 42,
        }
    }

    #[test]
    fn di_un_fisso_si_dice_che_la_batteria_non_ce() {
        // Il silenzio direbbe «non lo so», e sono due cose diverse: la prima
        // manda a cercare un guasto, la seconda no.
        let t = racconta(&fissa());
        assert!(t.contains("Batteria      : nessuna (e' un fisso)"), "{t}");
        assert!(
            t.contains("Acceso da     : 5h 7m"),
            "i secondi non si dicono: {t}"
        );
    }

    #[test]
    fn della_batteria_si_dice_solo_quel_che_si_sa() {
        let mut m = fissa();
        m.batteria = Some(Batteria {
            percentuale: None,
            alla_corrente: false,
            minuti_rimasti: None,
        });
        assert!(racconta(&m).contains("Batteria      : a batteria"));
        m.batteria = Some(Batteria {
            percentuale: Some(62),
            alla_corrente: true,
            minuti_rimasti: Some(95),
        });
        assert!(racconta(&m).contains("Batteria      : 62%, alla corrente, ~95 minuti"));
    }

    #[test]
    fn i_dischi_stanno_in_colonna() {
        let mut m = fissa();
        m.dischi.push(Disco {
            radice: "D:\\".into(),
            totale_byte: 2048,
            liberi_byte: 1024,
        });
        let detto = racconta(&m);
        let righe: Vec<&str> = detto.lines().filter(|r| r.starts_with("Disco")).collect();
        assert_eq!(righe.len(), 2);
        let colonne: Vec<usize> = righe.iter().map(|r| r.find(':').unwrap()).collect();
        assert_eq!(colonne[0], colonne[1], "la colonna balla: {righe:?}");
    }

    #[test]
    fn il_volume_sale_a_passi_da_due() {
        assert_eq!(passi_di_volume(0), 0);
        assert_eq!(passi_di_volume(100), 50);
        assert_eq!(passi_di_volume(50), 25);
        assert_eq!(passi_di_volume(51), 26, "25,5 arrotonda al pari");
        assert_eq!(passi_di_volume(49), 24, "24,5 arrotonda al pari");
        assert_eq!(passi_di_volume(-10), 0, "fuori scala si torna dentro");
        assert_eq!(passi_di_volume(500), 50);
    }
}
