//! L'invito: quello che il dispositivo inquadra per abbinarsi.
//!
//! Una riga sola, `nova://abbina?v=1&a=<indirizzi>&p=<porta>&i=<impronta>&c=<codice>`,
//! che il pannello mostra come QR. Dentro c'e' tutto quello che serve per
//! trovare il PC, riconoscerlo e farsi riconoscere:
//!
//! - **gli indirizzi** a cui provare, in ordine: quello sulla rete di casa,
//!   e quelli che l'utente ha aggiunto per quando e' fuori;
//! - **l'impronta** del certificato del PC: il dispositivo parla solo con chi
//!   ha quel certificato;
//! - **il codice**, che vale una volta e per pochi minuti.
//!
//! Chi fotografa l'invito mentre e' valido si puo' abbinare: si mostra il
//! tempo che serve, e scade da solo.

use anyhow::{anyhow, bail, Result};

/// Come comincia un invito.
pub const INIZIO: &str = "nova://abbina?";

/// La versione dell'invito che questo codice scrive e legge.
pub const VERSIONE_DELL_INVITO: &str = "1";

/// Un invito ad abbinarsi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invito {
    pub indirizzi: Vec<String>,
    pub porta: u16,
    pub impronta: String,
    pub codice: String,
}

/// Un indirizzo che si puo' scrivere nell'invito: un nome o un numero, senza
/// niente che lo confonda col resto della riga.
pub fn indirizzo_buono(a: &str) -> bool {
    !a.is_empty()
        && a.len() <= 253
        && a.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '_'))
}

/// Impronte e codici sono in base64 da indirizzo.
fn da_indirizzo(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

impl Invito {
    /// La riga dell'invito. Gli indirizzi che non si possono scrivere si
    /// saltano.
    pub fn testo(&self) -> String {
        let indirizzi: Vec<&str> = self
            .indirizzi
            .iter()
            .map(String::as_str)
            .filter(|a| indirizzo_buono(a))
            .collect();
        format!(
            "{INIZIO}v={VERSIONE_DELL_INVITO}&a={}&p={}&i={}&c={}",
            indirizzi.join(","),
            self.porta,
            self.impronta,
            self.codice
        )
    }

    /// Legge un invito, e dice cosa non va in uno che non si legge.
    pub fn leggi(testo: &str) -> Result<Self> {
        let resto = testo
            .trim()
            .strip_prefix(INIZIO)
            .ok_or_else(|| anyhow!("non e' un invito di NOVA: comincia con «{INIZIO}»"))?;
        let (mut versione, mut indirizzi, mut porta, mut impronta, mut codice) =
            (None, None, None, None, None);
        for pezzo in resto.split('&') {
            match pezzo.split_once('=') {
                Some(("v", v)) => versione = Some(v),
                Some(("a", v)) => indirizzi = Some(v),
                Some(("p", v)) => porta = Some(v),
                Some(("i", v)) => impronta = Some(v),
                Some(("c", v)) => codice = Some(v),
                // Un campo che non si conosce e' di una versione dopo.
                _ => {}
            }
        }
        if versione != Some(VERSIONE_DELL_INVITO) {
            bail!("l'invito e' di una versione che questo NOVA non legge");
        }
        let indirizzi: Vec<String> = indirizzi
            .unwrap_or_default()
            .split(',')
            .filter(|a| indirizzo_buono(a))
            .map(str::to_string)
            .collect();
        if indirizzi.is_empty() {
            bail!("l'invito non dice dove trovare il PC");
        }
        let porta: u16 = porta
            .and_then(|p| p.parse().ok())
            .filter(|p| *p != 0)
            .ok_or_else(|| anyhow!("l'invito non dice la porta"))?;
        let impronta = impronta
            .filter(|i| da_indirizzo(i))
            .ok_or_else(|| anyhow!("l'invito non ha l'impronta del PC"))?;
        let codice = codice
            .filter(|c| da_indirizzo(c))
            .ok_or_else(|| anyhow!("l'invito non ha il codice"))?;
        Ok(Self {
            indirizzi,
            porta,
            impronta: impronta.to_string(),
            codice: codice.to_string(),
        })
    }

    /// L'invito come QR, in SVG: nero su bianco, col bordo che i lettori
    /// vogliono.
    pub fn svg(&self) -> Result<String> {
        let qr = qrcode::QrCode::new(self.testo().as_bytes())
            .map_err(|e| anyhow!("l'invito non entra in un QR: {e}"))?;
        Ok(qr
            .render::<qrcode::render::svg::Color<'_>>()
            .min_dimensions(240, 240)
            .quiet_zone(true)
            .build())
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    fn invito() -> Invito {
        Invito {
            indirizzi: vec![
                "192.168.1.5".into(),
                "pc.rete-mia.ts.net".into(),
                "fe80::1".into(),
            ],
            porta: 47_810,
            impronta: "A".repeat(43),
            codice: "b-_9".into(),
        }
    }

    #[test]
    fn un_invito_scritto_si_rilegge_uguale() {
        let i = invito();
        let riga = i.testo();
        assert_eq!(
            riga,
            format!(
                "nova://abbina?v=1&a=192.168.1.5,pc.rete-mia.ts.net,fe80::1&p=47810&i={}&c=b-_9",
                "A".repeat(43)
            )
        );
        assert_eq!(Invito::leggi(&format!("  {riga}\n")).unwrap(), i);
        // Un campo in piu', di una versione dopo, non disturba.
        assert_eq!(Invito::leggi(&format!("{riga}&z=nuovo")).unwrap(), i);
    }

    #[test]
    fn un_indirizzo_che_confonde_la_riga_non_ci_entra() {
        let mut i = invito();
        i.indirizzi = vec![
            "pc&c=mio".into(),
            "".into(),
            "10.0.0.2".into(),
            "a b".into(),
        ];
        assert_eq!(
            Invito::leggi(&i.testo()).unwrap().indirizzi,
            vec!["10.0.0.2"]
        );
    }

    #[test]
    fn un_invito_che_non_si_legge_dice_perche() {
        let errore = |t: &str| Invito::leggi(t).unwrap_err().to_string();
        assert!(errore("https://esempio.it").contains("non e' un invito di NOVA"));
        let buono = invito().testo();
        assert!(errore(&buono.replace("v=1", "v=2")).contains("versione"));
        assert!(
            errore(&buono.replace("a=192.168.1.5,pc.rete-mia.ts.net,fe80::1", "a="))
                .contains("dove trovare")
        );
        assert!(errore(&buono.replace("p=47810", "p=0")).contains("porta"));
        assert!(errore(&buono.replace("p=47810", "p=70000")).contains("porta"));
        assert!(errore(&buono.replace("&i=", "&x=")).contains("impronta"));
        assert!(errore(&buono.replace("c=b-_9", "c=b+9")).contains("codice"));
    }

    #[test]
    fn l_invito_diventa_un_qr() {
        let svg = invito().svg().unwrap();
        assert!(
            svg.starts_with("<?xml") || svg.starts_with("<svg"),
            "{}",
            &svg[..40]
        );
        assert!(svg.contains("<svg") && svg.contains("</svg>"));
    }
}
