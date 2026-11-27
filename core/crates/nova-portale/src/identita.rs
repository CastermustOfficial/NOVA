//! Chi e' il PC per i dispositivi: un certificato fatto in casa.
//!
//! Non lo firma nessuno, e non serve: il dispositivo ne legge l'impronta nel
//! codice di abbinamento e accetta solo quello. Si fa una volta, e resta: se
//! cambiasse, ogni dispositivo abbinato smetterebbe di riconoscere il PC.

use anyhow::{Context, Result};
use base64::Engine;
use sha2::{Digest, Sha256};
use std::path::Path;

/// Il nome scritto nel certificato. Non e' un indirizzo: il dispositivo
/// guarda l'impronta, non il nome.
pub const NOME_NEL_CERTIFICATO: &str = "nova";

const CERTIFICATO: &str = "certificato.der";
const CHIAVE: &str = "chiave.der";

/// Il certificato del PC e la sua chiave privata, in DER.
#[derive(Clone)]
pub struct Identita {
    pub certificato: Vec<u8>,
    pub chiave: Vec<u8>,
}

/// L'impronta di un certificato: SHA-256 del DER, in base64 da indirizzo.
pub fn impronta_di(der: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(der))
}

impl Identita {
    /// Un certificato nuovo, con la sua chiave.
    pub fn nuova() -> Result<Self> {
        let fatto = rcgen::generate_simple_self_signed(vec![NOME_NEL_CERTIFICATO.to_string()])
            .context("il certificato del portale non si e' fatto")?;
        Ok(Self {
            certificato: fatto.cert.der().to_vec(),
            chiave: fatto.key_pair.serialize_der(),
        })
    }

    pub fn impronta(&self) -> String {
        impronta_di(&self.certificato)
    }

    /// Quello salvato nella cartella, o uno nuovo salvato li'. `true` se e'
    /// nuovo: chi era abbinato a quello di prima non riconosce piu' il PC.
    pub fn apri_o_crea(cartella: &Path) -> Result<(Self, bool)> {
        let (c, k) = (cartella.join(CERTIFICATO), cartella.join(CHIAVE));
        if let (Ok(certificato), Ok(chiave)) = (std::fs::read(&c), std::fs::read(&k)) {
            if !certificato.is_empty() && !chiave.is_empty() {
                return Ok((
                    Self {
                        certificato,
                        chiave,
                    },
                    false,
                ));
            }
        }
        let nuova = Self::nuova()?;
        std::fs::create_dir_all(cartella)
            .with_context(|| format!("non riesco a creare {}", cartella.display()))?;
        scrivi_riservato(&k, &nuova.chiave)?;
        std::fs::write(&c, &nuova.certificato)
            .with_context(|| format!("non riesco a scrivere {}", c.display()))?;
        Ok((nuova, true))
    }
}

/// La chiave privata la legge solo l'utente. Su Windows la cartella di NOVA
/// sta nel profilo dell'utente, che e' gia' solo suo.
fn scrivi_riservato(percorso: &Path, dati: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut come = std::fs::OpenOptions::new();
    come.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        come.mode(0o600);
    }
    come.open(percorso)
        .and_then(|mut f| f.write_all(dati))
        .with_context(|| format!("non riesco a scrivere {}", percorso.display()))
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn l_identita_si_fa_una_volta_e_resta() {
        let dir = std::env::temp_dir().join(format!("nova-portale-id-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (prima, nuova) = Identita::apri_o_crea(&dir).unwrap();
        assert!(nuova);
        let (dopo, nuova) = Identita::apri_o_crea(&dir).unwrap();
        assert!(!nuova, "la seconda volta e' quella salvata");
        assert_eq!(prima.impronta(), dopo.impronta());
        // SHA-256 in base64 da indirizzo, senza riempimento: 43 caratteri.
        assert_eq!(prima.impronta().len(), 43);
        assert!(prima
            .impronta()
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        assert_ne!(Identita::nuova().unwrap().impronta(), prima.impronta());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let modo = std::fs::metadata(dir.join(CHIAVE))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(modo & 0o777, 0o600, "la chiave la legge solo l'utente");
        }
        // Una chiave sparita: l'identita' si rifa', e lo si dice.
        std::fs::remove_file(dir.join(CHIAVE)).unwrap();
        let (rifatta, nuova) = Identita::apri_o_crea(&dir).unwrap();
        assert!(nuova);
        assert_ne!(rifatta.impronta(), prima.impronta());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
