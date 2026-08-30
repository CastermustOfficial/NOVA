//! Lettore minimale dei metadati GGUF.
//!
//! Serve una cosa sola: quanti strati ha il modello. Da li' si calcola quanti
//! ne stanno in VRAM, e da quel numero dipende se NOVA va a trenta token al
//! secondo o a tre. Il resto dei metadati si legge perche' costa niente
//! leggerlo mentre si e' li'.
//!
//! Due scelte meritano una riga.
//!
//! **Il vocabolario non si tiene.** In un GGUF moderno `tokenizer.ggml.tokens`
//! e' un vettore da centocinquantamila stringhe. Caricarlo per contare gli
//! strati vuol dire allocare decine di megabyte e buttarli: si legge, si conta
//! e si scarta, tenendo solo la lunghezza.
//!
//! **Non si crede all'intestazione sui numeri.** Un file troncato a meta'
//! scaricamento dichiara volentieri quattro miliardi di chiavi; se ci si
//! crede, si prova ad allocare quattro miliardi di voci e il processo muore
//! per esaurimento di memoria invece di dire «questo file e' incompleto».

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

/// Oltre questo un file non e' un GGUF con troppe chiavi: e' un file rotto.
/// I modelli veri stanno sotto il migliaio.
const MAX_CHIAVI: u64 = 1 << 20;
/// Una stringa di metadati piu' lunga di questo e' un numero letto storto.
const MAX_STRINGA: u64 = 1 << 24;
/// Un vettore piu' lungo di questo non lo si percorre nemmeno per contarlo.
const MAX_ARRAY: u64 = 1 << 28;

/// Un valore di metadati, ridotto a cio' che ci serve davvero.
#[derive(Debug, Clone, PartialEq)]
pub enum Valore {
    Intero(i64),
    Decimale(f64),
    Vero(bool),
    Testo(String),
    /// Un vettore: se ne tiene la lunghezza, non il contenuto.
    Vettore(usize),
}

impl Valore {
    /// Il valore come intero, se lo e'. I conteggi di strati sono `u32` nei
    /// file veri, ma qualcuno li scrive `u64`: si accettano entrambi.
    pub fn intero(&self) -> Option<i64> {
        match self {
            Valore::Intero(n) => Some(*n),
            _ => None,
        }
    }

    pub fn testo(&self) -> Option<&str> {
        match self {
            Valore::Testo(s) => Some(s),
            _ => None,
        }
    }
}

/// La forma di un modello: quel che basta per decidere come caricarlo.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Forma {
    pub arch: String,
    pub nome: String,
    /// Il numero di blocchi transformer. Zero vuol dire «non l'ho trovato»,
    /// e chi calcola gli strati deve trattarlo come «non so», non come zero.
    pub n_strati: u32,
    /// Il contesto con cui e' stato addestrato.
    pub n_ctx_train: u32,
    pub n_embd: u32,
}

fn leggi_esatto<R: Read>(r: &mut R, buf: &mut [u8]) -> Result<(), String> {
    r.read_exact(buf).map_err(|e| format!("file troncato: {e}"))
}

fn u32le<R: Read>(r: &mut R) -> Result<u32, String> {
    let mut b = [0u8; 4];
    leggi_esatto(r, &mut b)?;
    Ok(u32::from_le_bytes(b))
}

fn u64le<R: Read>(r: &mut R) -> Result<u64, String> {
    let mut b = [0u8; 8];
    leggi_esatto(r, &mut b)?;
    Ok(u64::from_le_bytes(b))
}

fn stringa<R: Read>(r: &mut R) -> Result<String, String> {
    let n = u64le(r)?;
    if n > MAX_STRINGA {
        return Err(format!("stringa di {n} byte: il file non e' quello che dice"));
    }
    let mut b = vec![0u8; n as usize];
    leggi_esatto(r, &mut b)?;
    // `from_utf8_lossy`: un byte storto in un nome non deve far fallire la
    // lettura di un modello che per il resto sta benissimo.
    Ok(String::from_utf8_lossy(&b).into_owned())
}

/// Legge un valore del tipo dato, e lo scarta se e' un vettore.
fn valore<R: Read + Seek>(r: &mut R, tipo: u32) -> Result<Valore, String> {
    Ok(match tipo {
        0 => Valore::Intero(leggi_n::<R, 1>(r)?[0] as i64),
        1 => Valore::Intero(leggi_n::<R, 1>(r)?[0] as i8 as i64),
        2 => Valore::Intero(u16::from_le_bytes(leggi_n::<R, 2>(r)?) as i64),
        3 => Valore::Intero(i16::from_le_bytes(leggi_n::<R, 2>(r)?) as i64),
        4 => Valore::Intero(u32::from_le_bytes(leggi_n::<R, 4>(r)?) as i64),
        5 => Valore::Intero(i32::from_le_bytes(leggi_n::<R, 4>(r)?) as i64),
        6 => Valore::Decimale(f32::from_le_bytes(leggi_n::<R, 4>(r)?) as f64),
        7 => Valore::Vero(leggi_n::<R, 1>(r)?[0] != 0),
        8 => Valore::Testo(stringa(r)?),
        9 => {
            let et = u32le(r)?;
            let n = u64le(r)?;
            if n > MAX_ARRAY {
                return Err(format!("vettore di {n} voci: il file non e' quello che dice"));
            }
            salta_vettore(r, et, n)?;
            Valore::Vettore(n as usize)
        }
        10 => Valore::Intero(u64::from_le_bytes(leggi_n::<R, 8>(r)?) as i64),
        11 => Valore::Intero(i64::from_le_bytes(leggi_n::<R, 8>(r)?)),
        12 => Valore::Decimale(f64::from_le_bytes(leggi_n::<R, 8>(r)?)),
        altro => return Err(format!("tipo GGUF sconosciuto: {altro}")),
    })
}

fn leggi_n<R: Read, const N: usize>(r: &mut R) -> Result<[u8; N], String> {
    let mut b = [0u8; N];
    leggi_esatto(r, &mut b)?;
    Ok(b)
}

/// Il costo in byte di un tipo a larghezza fissa, se ce l'ha.
fn larghezza(tipo: u32) -> Option<u64> {
    Some(match tipo {
        0 | 1 | 7 => 1,
        2 | 3 => 2,
        4 | 5 | 6 => 4,
        10 | 11 | 12 => 8,
        _ => return None,
    })
}

/// Scavalca un vettore senza materializzarlo.
///
/// E' qui che si guadagna il tempo: centocinquantamila stringhe di
/// vocabolario si attraversano leggendo la lunghezza e saltando, non
/// allocando. Per i tipi a larghezza fissa e' un salto solo.
fn salta_vettore<R: Read + Seek>(r: &mut R, tipo: u32, n: u64) -> Result<(), String> {
    if let Some(w) = larghezza(tipo) {
        r.seek(SeekFrom::Current((w * n) as i64))
            .map_err(|e| format!("file troncato: {e}"))?;
        return Ok(());
    }
    match tipo {
        8 => {
            for _ in 0..n {
                let l = u64le(r)?;
                if l > MAX_STRINGA {
                    return Err("stringa fuori misura dentro un vettore".into());
                }
                r.seek(SeekFrom::Current(l as i64))
                    .map_err(|e| format!("file troncato: {e}"))?;
            }
            Ok(())
        }
        // Vettori di vettori: esistono nella specifica, non nei modelli veri.
        9 => {
            for _ in 0..n {
                let et = u32le(r)?;
                let m = u64le(r)?;
                if m > MAX_ARRAY {
                    return Err("vettore annidato fuori misura".into());
                }
                salta_vettore(r, et, m)?;
            }
            Ok(())
        }
        altro => Err(format!("tipo GGUF sconosciuto dentro un vettore: {altro}")),
    }
}

/// I primi quattro byte di un GGUF sono `GGUF`.
///
/// L'estensione la mette chi rinomina; questi byte li mette chi ha scritto il
/// file. Uno scaricamento interrotto o una pagina di errore salvata col nome
/// giusto superano il primo controllo e non il secondo.
pub fn e_gguf(percorso: &Path) -> bool {
    let mut f = match File::open(percorso) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut b = [0u8; 4];
    f.read_exact(&mut b).is_ok() && &b == b"GGUF"
}

/// Tutti i metadati, in ordine di chiave.
pub fn metadati(percorso: &Path) -> Result<BTreeMap<String, Valore>, String> {
    let f = File::open(percorso).map_err(|e| format!("non si apre: {e}"))?;
    let mut r = BufReader::new(f);
    let mut magia = [0u8; 4];
    leggi_esatto(&mut r, &mut magia)?;
    if &magia != b"GGUF" {
        return Err("non e' un file GGUF".into());
    }
    let _versione = u32le(&mut r)?;
    let _tensori = u64le(&mut r)?;
    let n = u64le(&mut r)?;
    if n > MAX_CHIAVI {
        return Err(format!("{n} chiavi dichiarate: il file non e' quello che dice"));
    }
    let mut kv = BTreeMap::new();
    for _ in 0..n {
        let chiave = stringa(&mut r)?;
        let tipo = u32le(&mut r)?;
        let v = valore(&mut r, tipo)?;
        kv.insert(chiave, v);
    }
    Ok(kv)
}

/// La forma del modello. Un file illeggibile da' una forma vuota, non un
/// errore: chi chiama sta scegliendo fra dieci file e non deve fermarsi al
/// primo rotto.
pub fn forma(percorso: &Path) -> Forma {
    let kv = match metadati(percorso) {
        Ok(kv) => kv,
        Err(_) => return Forma::default(),
    };
    let arch = kv
        .get("general.architecture")
        .and_then(|v| v.testo())
        .unwrap_or("")
        .to_string();
    let nome = kv
        .get("general.name")
        .and_then(|v| v.testo())
        .unwrap_or("")
        .to_string();
    let leggi = |suffisso: &str| -> u32 {
        kv.get(&format!("{arch}.{suffisso}"))
            .and_then(|v| v.intero())
            .filter(|n| *n > 0)
            .unwrap_or(0) as u32
    };
    Forma {
        n_strati: leggi("block_count"),
        n_ctx_train: leggi("context_length"),
        n_embd: leggi("embedding_length"),
        arch,
        nome,
    }
}

#[cfg(test)]
mod prove {
    use super::*;
    use std::io::Cursor;

    /// Costruisce un GGUF finto in memoria, con le chiavi date.
    fn finto(chiavi: &[(&str, Valore)]) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(b"GGUF");
        b.extend_from_slice(&3u32.to_le_bytes());
        b.extend_from_slice(&0u64.to_le_bytes());
        b.extend_from_slice(&(chiavi.len() as u64).to_le_bytes());
        for (k, v) in chiavi {
            b.extend_from_slice(&(k.len() as u64).to_le_bytes());
            b.extend_from_slice(k.as_bytes());
            match v {
                Valore::Intero(n) => {
                    b.extend_from_slice(&4u32.to_le_bytes());
                    b.extend_from_slice(&(*n as u32).to_le_bytes());
                }
                Valore::Testo(s) => {
                    b.extend_from_slice(&8u32.to_le_bytes());
                    b.extend_from_slice(&(s.len() as u64).to_le_bytes());
                    b.extend_from_slice(s.as_bytes());
                }
                _ => unreachable!(),
            }
        }
        b
    }

    fn leggi_da(b: &[u8]) -> Result<BTreeMap<String, Valore>, String> {
        let mut r = Cursor::new(b);
        let mut magia = [0u8; 4];
        leggi_esatto(&mut r, &mut magia)?;
        assert_eq!(&magia, b"GGUF");
        u32le(&mut r)?;
        u64le(&mut r)?;
        let n = u64le(&mut r)?;
        let mut kv = BTreeMap::new();
        for _ in 0..n {
            let k = stringa(&mut r)?;
            let t = u32le(&mut r)?;
            kv.insert(k, valore(&mut r, t)?);
        }
        Ok(kv)
    }

    #[test]
    fn legge_le_chiavi() {
        let b = finto(&[
            ("general.architecture", Valore::Testo("qwen3".into())),
            ("qwen3.block_count", Valore::Intero(64)),
        ]);
        let kv = leggi_da(&b).unwrap();
        assert_eq!(kv["general.architecture"].testo(), Some("qwen3"));
        assert_eq!(kv["qwen3.block_count"].intero(), Some(64));
    }

    #[test]
    fn un_vettore_si_conta_e_non_si_tiene() {
        let mut b = Vec::new();
        b.extend_from_slice(&9u32.to_le_bytes()); // vettore
        b.extend_from_slice(&8u32.to_le_bytes()); // di stringhe
        b.extend_from_slice(&2u64.to_le_bytes());
        for s in ["ciao", "mondo"] {
            b.extend_from_slice(&(s.len() as u64).to_le_bytes());
            b.extend_from_slice(s.as_bytes());
        }
        let mut r = Cursor::new(&b[4..]);
        let v = valore(&mut r, 9).unwrap();
        assert_eq!(v, Valore::Vettore(2));
    }

    #[test]
    fn un_conteggio_assurdo_non_alloca() {
        // Un file troncato che dichiara quattro miliardi di chiavi deve dare
        // un errore, non provare a fare spazio per quattro miliardi di voci.
        let mut b = Vec::new();
        b.extend_from_slice(b"GGUF");
        b.extend_from_slice(&3u32.to_le_bytes());
        b.extend_from_slice(&0u64.to_le_bytes());
        b.extend_from_slice(&u64::MAX.to_le_bytes());
        let mut r = Cursor::new(&b[..]);
        let mut m = [0u8; 4];
        leggi_esatto(&mut r, &mut m).unwrap();
        u32le(&mut r).unwrap();
        u64le(&mut r).unwrap();
        let n = u64le(&mut r).unwrap();
        assert!(n > MAX_CHIAVI);
    }

    #[test]
    fn zero_strati_e_non_so_non_zero() {
        // block_count assente: la forma torna 0, e chi calcola deve leggerlo
        // come «non lo so», mai come «nessuno strato».
        assert_eq!(Forma::default().n_strati, 0);
    }
}
