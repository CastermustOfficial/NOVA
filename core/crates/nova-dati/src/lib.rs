//! «Dove sono i miei dati?»
//!
//! E' la domanda che decide se qualcuno lascia installato un programma che
//! gli legge la posta, gli tiene le password e sa cosa fa al computer.
//! Quattro colonne: **che cos'e'**, **dove sta**, **quanto pesa**, e **cosa
//! succede se lo cancelli**. L'ultima e' quella che nessuno scrive mai, ed e'
//! la sola che permetta a una persona di fare pulizia senza paura.
//!
//! ## Cosa c'e' qui, e cosa no
//!
//! **I percorsi non stanno qui.** Ogni percorso lo dice il modulo che ci
//! scrive dentro: una mappa che tiene la propria copia dei percorsi prima o
//! poi diverge, e qui divergere vuol dire indicare a chi cerca i suoi dati un
//! file che non c'e' — che e' esattamente com'e' cominciata (D188). Chi
//! chiama passa i posti che ha; questa cassetta li misura e li racconta.
//!
//! Qui c'e' quel che si puo' provare senza un disco: come si scrive una
//! misura perche' una persona la legga, come si mette insieme il racconto, e
//! la frase che chiude l'elenco — quella su cosa esce dal PC, che e' la
//! ragione per cui l'elenco esiste.

use std::path::{Path, PathBuf};

/// Una cosa che NOVA tiene sul disco.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Posto {
    pub che_cos_e: String,
    pub dove: PathBuf,
    /// Cosa succede se lo cancelli, detto a chi non sa cos'e'.
    pub se_lo_cancelli: String,
    /// Se dentro c'e' roba che riguarda **la persona**, non il programma.
    pub delicato: bool,
}

impl Posto {
    pub fn nuovo(che_cos_e: &str, dove: impl Into<PathBuf>, se_lo_cancelli: &str) -> Posto {
        Posto {
            che_cos_e: che_cos_e.to_string(),
            dove: dove.into(),
            se_lo_cancelli: se_lo_cancelli.to_string(),
            delicato: false,
        }
    }

    pub fn delicato(mut self) -> Posto {
        self.delicato = true;
        self
    }
}

/// Quanto occupa un posto, guardato sul disco.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Misura {
    pub esiste: bool,
    pub byte: u64,
    pub quanti_file: u64,
}

/// Una misura scritta perche' la legga una persona.
///
/// Le soglie e le cifre decimali sono quelle del Python, e non per pignoleria:
/// lo stesso numero deve leggersi uguale nel racconto, nel rendiconto del
/// disinstallatore e nel pannello, o sembrano tre programmi diversi che dicono
/// tre cose diverse sullo stesso file.
pub fn pesa(byte: u64) -> String {
    const K: f64 = 1024.0;
    let b = byte as f64;
    if byte < 1024 {
        return format!("{byte} B");
    }
    if byte < 1024 * 1024 {
        // Senza decimali, e **arrotondato**: 1536 byte sono «2 kB», non
        // «1 kB». E' quel che fa `{:.0f}` in Python, e discostarsene di un'
        // unita' basta a far divergere due elenchi che dicono lo stesso file.
        return format!("{} kB", (b / K).round() as u64);
    }
    if byte < 1024 * 1024 * 1024 {
        return format!("{:.1} MB", b / (K * K));
    }
    format!("{:.2} GB", b / (K * K * K))
}

/// Cosa si dice di un posto che non c'e' ancora.
pub const NON_CREATO: &str = "non ancora creato";

/// La frase che chiude l'elenco.
///
/// E' la ragione per cui l'elenco esiste: sapere **dove** stanno i propri dati
/// serve a poco se non si sa anche **se escono**. Sta qui e non in
/// un'interfaccia perche' chi la legge da PowerShell durante una
/// disinstallazione ha lo stesso diritto di leggerla di chi apre il pannello.
pub const CHIUSURA: &str = "Niente di questo esce dal PC finche' il cervello e' quello \
locale. Con «claude» o «api» escono la domanda, il contesto della conversazione e i \
nodi di memoria pertinenti — mai le credenziali, che il modello non vede in nessun caso.";

/// Cosa si dice quando NOVA non ha ancora scritto niente.
pub fn niente_ancora(base: &Path) -> String {
    format!(
        "NOVA non ha ancora scritto niente: la prima volta che la usi crea la \
         sua cartella in {}",
        base.display()
    )
}

/// L'elenco, in una forma che si legge.
///
/// `solo_esistenti` perche' un elenco di venti voci di cui quindici «non
/// ancora create» non e' una risposta alla domanda «dove sono i miei dati»:
/// e' un catalogo del programma.
pub fn racconta(posti: &[(Posto, Misura)], base: &Path, solo_esistenti: bool) -> String {
    let mut righe = vec!["Dove NOVA tiene le tue cose:".to_string(), String::new()];
    let mut totale: u64 = 0;
    let mut visti = 0;
    for (p, m) in posti {
        if solo_esistenti && !m.esiste {
            continue;
        }
        visti += 1;
        totale += m.byte;
        let misura = if m.esiste {
            let mut t = pesa(m.byte);
            if m.quanti_file > 1 {
                t.push_str(&format!(", {} file", m.quanti_file));
            }
            t
        } else {
            NON_CREATO.to_string()
        };
        let segno = if p.delicato { "  (delicato)" } else { "" };
        righe.push(format!("{}{}", p.che_cos_e, segno));
        righe.push(format!("    {}", p.dove.display()));
        righe.push(format!("    {misura}"));
        righe.push(format!("    se lo cancelli: {}", p.se_lo_cancelli));
        righe.push(String::new());
    }
    if visti == 0 {
        return niente_ancora(base);
    }
    righe.push(format!("In tutto {}.", pesa(totale)));
    righe.push(String::new());
    righe.push(CHIUSURA.to_string());
    righe.join("\n")
}

/// Una voce del rendiconto: quel che si dice a chi disinstalla.
///
/// Non esce nessun **contenuto**: solo dove, quanto pesa, e se sparisce
/// cancellando la cartella di NOVA.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Voce {
    pub che_cos_e: String,
    pub dove: String,
    pub byte: u64,
    pub misura: String,
    pub delicato: bool,
    /// Se sta dentro la cartella di NOVA, e quindi se ne va con lei.
    pub va_via_con_la_cartella: bool,
    pub se_lo_cancelli: String,
}

/// L'inventario, per chi disinstalla.
///
/// Viene dallo stesso elenco del racconto e non da una seconda lista scritta a
/// mano: le due risposte alla stessa domanda erano gia' divergute di quindici
/// gigabyte, perche' una aggiungeva il modello scaricato e l'altra no (D188).
pub fn rendiconto(posti: &[(Posto, Misura)], base: &Path) -> (Vec<Voce>, String) {
    let voci: Vec<Voce> = posti
        .iter()
        .filter(|(_, m)| m.esiste)
        .map(|(p, m)| Voce {
            che_cos_e: p.che_cos_e.clone(),
            dove: p.dove.display().to_string(),
            byte: m.byte,
            misura: pesa(m.byte),
            delicato: p.delicato,
            va_via_con_la_cartella: sta_dentro(&p.dove, base),
            se_lo_cancelli: p.se_lo_cancelli.clone(),
        })
        .collect();
    let totale = pesa(voci.iter().map(|v| v.byte).sum());
    (voci, totale)
}

/// Se un percorso sta dentro una cartella.
///
/// Per **componenti**, non per prefisso di testo: `C:\NOVA-vecchio` comincia
/// come `C:\NOVA` e non ci sta dentro. E' lo stesso inciampo per cui
/// autorizzare `C:\dati` autorizzava anche `C:\dati-altrui`.
pub fn sta_dentro(quale: &Path, cartella: &Path) -> bool {
    let mut a = quale.components();
    for pezzo in cartella.components() {
        match a.next() {
            Some(x) if x == pezzo => continue,
            _ => return false,
        }
    }
    true
}

#[cfg(test)]
mod prove {
    use super::*;

    fn posto(nome: &str, dove: &str, byte: u64, file: u64) -> (Posto, Misura) {
        (
            Posto::nuovo(nome, dove, "niente di grave"),
            Misura { esiste: byte > 0 || file > 0, byte, quanti_file: file },
        )
    }

    #[test]
    fn le_misure_si_leggono_come_in_python() {
        // I numeri vengono da `nova/dati.py`, chiesti a lui.
        for (b, atteso) in [
            (0u64, "0 B"), (1, "1 B"), (1023, "1023 B"),
            (1024, "1 kB"), (1536, "2 kB"), (1_048_575, "1024 kB"),
            (1_048_576, "1.0 MB"), (1_572_864, "1.5 MB"),
            (1_073_741_823, "1024.0 MB"), (1_073_741_824, "1.00 GB"),
            (17_179_869_184, "16.00 GB"),
        ] {
            assert_eq!(pesa(b), atteso, "{b} byte");
        }
    }

    #[test]
    fn milleduecentotrentasei_byte_sono_due_kilobyte() {
        // Arrotondato, non troncato. Un'unita' di differenza basta a far
        // dire due cose diverse sullo stesso file a due parti del programma.
        assert_eq!(pesa(1536), "2 kB");
        assert_eq!(pesa(1535), "1 kB");
    }

    #[test]
    fn il_racconto_dice_le_quattro_colonne() {
        let p = vec![posto("La memoria a grafo", "/casa/vault", 4096, 12)];
        let t = racconta(&p, Path::new("/casa"), true);
        assert!(t.contains("La memoria a grafo"), "{t}");
        assert!(t.contains("/casa/vault"), "{t}");
        assert!(t.contains("4 kB, 12 file"), "{t}");
        assert!(t.contains("se lo cancelli: niente di grave"), "{t}");
    }

    #[test]
    fn un_file_solo_non_si_conta_a_voce() {
        // «1 file» dopo la misura non aggiunge niente e fa rumore.
        let p = vec![posto("La configurazione", "/casa/config.json", 900, 1)];
        let t = racconta(&p, Path::new("/casa"), true);
        assert!(t.contains("900 B\n"), "{t}");
        assert!(!t.contains("1 file"), "{t}");
    }

    #[test]
    fn il_delicato_si_vede() {
        let mut p = posto("Le credenziali", "/casa/segreti.dat", 128, 1);
        p.0 = p.0.delicato();
        let t = racconta(&[p], Path::new("/casa"), true);
        assert!(t.contains("Le credenziali  (delicato)"), "{t}");
    }

    #[test]
    fn quando_non_c_e_niente_lo_dice_e_dice_dove_nascera() {
        let vuoti = vec![(
            Posto::nuovo("La memoria", "/casa/vault", "x"),
            Misura::default(),
        )];
        let t = racconta(&vuoti, Path::new("/casa/NOVA"), true);
        assert!(t.contains("non ha ancora scritto niente"), "{t}");
        assert!(t.contains("/casa/NOVA"), "deve dire dove nascera': {t}");
    }

    #[test]
    fn e_chiedendo_tutto_si_vedono_anche_i_posti_non_creati() {
        let vuoti = vec![(
            Posto::nuovo("La memoria", "/casa/vault", "x"),
            Misura::default(),
        )];
        let t = racconta(&vuoti, Path::new("/casa"), false);
        assert!(t.contains(NON_CREATO), "{t}");
    }

    #[test]
    fn il_racconto_finisce_dicendo_cosa_esce_dal_pc() {
        // E' la ragione per cui l'elenco esiste: sapere dove stanno i propri
        // dati serve a poco se non si sa se escono.
        let p = vec![posto("x", "/casa/x", 10, 1)];
        let t = racconta(&p, Path::new("/casa"), true);
        assert!(t.contains("mai le credenziali"), "{t}");
        assert!(t.trim_end().ends_with("in nessun caso."), "{t}");
    }

    #[test]
    fn il_totale_somma_solo_quel_che_c_e() {
        let p = vec![
            posto("uno", "/casa/a", 1024, 1),
            posto("due", "/casa/b", 1024, 1),
            (Posto::nuovo("tre", "/casa/c", "x"), Misura::default()),
        ];
        let t = racconta(&p, Path::new("/casa"), true);
        assert!(t.contains("In tutto 2 kB."), "{t}");
    }

    #[test]
    fn il_rendiconto_tace_sui_posti_che_non_ci_sono() {
        let p = vec![
            posto("c'e'", "/casa/a", 100, 1),
            (Posto::nuovo("non c'e'", "/casa/b", "x"), Misura::default()),
        ];
        let (voci, totale) = rendiconto(&p, Path::new("/casa"));
        assert_eq!(voci.len(), 1);
        assert_eq!(totale, "100 B");
    }

    #[test]
    fn il_rendiconto_dice_cosa_se_ne_va_con_la_cartella() {
        let p = vec![
            posto("dentro", "/casa/NOVA/vault", 10, 1),
            posto("fuori", "/utente/Documenti/fascicolo", 10, 1),
        ];
        let (voci, _) = rendiconto(&p, Path::new("/casa/NOVA"));
        assert!(voci[0].va_via_con_la_cartella);
        assert!(!voci[1].va_via_con_la_cartella, "il fascicolo sta in Documenti apposta");
    }

    #[test]
    fn dentro_si_guarda_a_pezzi_non_a_lettere() {
        // `/casa/NOVA-vecchio` comincia come `/casa/NOVA` e non ci sta
        // dentro. E' lo stesso inciampo di `C:\dati` che autorizzava
        // `C:\dati-altrui`.
        assert!(sta_dentro(Path::new("/casa/NOVA/vault"), Path::new("/casa/NOVA")));
        assert!(!sta_dentro(Path::new("/casa/NOVA-vecchio/x"), Path::new("/casa/NOVA")));
        assert!(sta_dentro(Path::new("/casa/NOVA"), Path::new("/casa/NOVA")));
        assert!(!sta_dentro(Path::new("/casa"), Path::new("/casa/NOVA")));
    }

    #[test]
    fn nel_rendiconto_non_finisce_mai_un_contenuto() {
        // La regola sta scritta nel Python: «non esce nessun contenuto, solo
        // dove, quanto pesa, e se sparisce». Qui si controlla che i campi
        // siano quei sette e non uno di piu'.
        let p = vec![posto("Le credenziali", "/casa/segreti.dat", 128, 1)];
        let (voci, _) = rendiconto(&p, Path::new("/casa"));
        let v = &voci[0];
        // Se qualcuno aggiunge un campo con dentro del contenuto, questa
        // riga non cambia da sola: cambia la struct, e allora va guardata.
        let Voce { che_cos_e, dove, byte, misura, delicato, va_via_con_la_cartella, se_lo_cancelli } = v;
        assert_eq!(che_cos_e, "Le credenziali");
        assert!(dove.ends_with("segreti.dat"));
        assert_eq!(*byte, 128);
        assert_eq!(misura, "128 B");
        assert!(!*delicato);
        assert!(*va_via_con_la_cartella);
        assert!(!se_lo_cancelli.is_empty());
    }
}
