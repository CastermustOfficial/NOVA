//! Prima di compilare il guscio: l'editor dell'harness, e poi Tauri.
//!
//! **Monaco non sta nel repository** (D336). Sono venticinque megabyte di
//! JavaScript ridotto, che cambiano tutti a ogni versione: tenerli in git
//! vorrebbe dire un repository che ingrassa di dieci mega a ogni
//! aggiornamento dell'editor, per un file che nessuno legge. Qui si scarica
//! **una versione fissata**, si controlla l'impronta che il registro npm
//! pubblica per quella versione, e si tiene solo la parte che una pagina
//! carica davvero — misurata aprendo l'editor e guardando cosa chiede.
//!
//! Senza rete non si ferma niente: il guscio si compila lo stesso, e
//! l'harness dice che l'editor manca invece di non aprirsi. Una compilazione
//! che fallisce perche' manca un editor sarebbe il guasto sbagliato nel posto
//! sbagliato — l'orb e la voce non c'entrano.

use std::io::Read;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use sha2::Digest as _;

/// La versione di Monaco, e l'impronta con cui npm la pubblica
/// (`npm view monaco-editor@0.57.0 dist.integrity`).
const MONACO: &str = "0.57.0";
const IMPRONTA: &str =
    "sha512-5BkI9KGoqrNvBGUe15/QlZq3OooZ8WLg1AxTpaqHRCP3HNpzPPZKE2EDz8M7c+VRmCeUw1Brp4cx/PWm3kI/5A==";

fn main() {
    let dove = PathBuf::from("ui").join("vendor").join("monaco");
    let segno = dove.join("VERSIONE");
    println!("cargo:rerun-if-changed={}", segno.display());
    if std::fs::read_to_string(&segno).is_ok_and(|v| v.trim() == MONACO) {
        // gia' qui
    } else if let Err(e) = porta_monaco(&dove) {
        println!(
            "cargo:warning=l'editor dell'harness (Monaco {MONACO}) non e' stato scaricato: {e}. \
             Il guscio funziona lo stesso; per avere l'editor ricompila con la rete."
        );
    }
    tauri_build::build()
}

/// Il pacchetto: dal registro npm, o da un file gia' scaricato indicato con
/// `NOVA_MONACO_TGZ` — per chi compila senza rete, o dietro un proxy che
/// il client HTTP non riconosce. L'impronta si controlla lo stesso.
fn pacchetto() -> Result<Vec<u8>, String> {
    println!("cargo:rerun-if-env-changed=NOVA_MONACO_TGZ");
    if let Some(f) = std::env::var_os("NOVA_MONACO_TGZ") {
        return std::fs::read(&f).map_err(|e| format!("leggendo {}: {e}", Path::new(&f).display()));
    }
    let url = format!("https://registry.npmjs.org/monaco-editor/-/monaco-editor-{MONACO}.tgz");
    let risposta = ureq::get(&url)
        .timeout(std::time::Duration::from_secs(180))
        .call()
        .map_err(|e| {
            format!("scaricando {url}: {e} (senza rete: NOVA_MONACO_TGZ=<pacchetto .tgz>)")
        })?;
    let mut compresso = Vec::new();
    risposta
        .into_reader()
        .take(80 << 20)
        .read_to_end(&mut compresso)
        .map_err(|e| format!("leggendo {url}: {e}"))?;
    Ok(compresso)
}

fn porta_monaco(dove: &Path) -> Result<(), String> {
    let compresso = pacchetto()?;

    let impronta = format!(
        "sha512-{}",
        base64::engine::general_purpose::STANDARD.encode(sha2::Sha512::digest(&compresso))
    );
    if impronta != IMPRONTA {
        return Err(format!(
            "l'impronta non torna: attesa {IMPRONTA}, arrivata {impronta}"
        ));
    }

    let mut tar = Vec::new();
    flate2::read::GzDecoder::new(&compresso[..])
        .read_to_end(&mut tar)
        .map_err(|e| format!("decomprimendo: {e}"))?;

    // Si scrive accanto e si sposta alla fine: un'interruzione a meta' non
    // deve lasciare un editor mezzo copiato con scritto sopra «fatto».
    let provvisoria = dove.with_extension("nuova");
    let _ = std::fs::remove_dir_all(&provvisoria);
    let mut quanti = 0;
    for (nome, dati) in voci_tar(&tar)? {
        let Some(relativo) = serve(&nome) else {
            continue;
        };
        let f = provvisoria.join(relativo);
        if let Some(d) = f.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        std::fs::write(&f, dati).map_err(|e| format!("scrivendo {}: {e}", f.display()))?;
        quanti += 1;
    }
    if quanti == 0 {
        return Err("nel pacchetto non c'era niente di cio' che serve".into());
    }
    std::fs::write(provvisoria.join("VERSIONE"), format!("{MONACO}\n"))
        .map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(dove);
    std::fs::rename(&provvisoria, dove).map_err(|e| e.to_string())?;
    Ok(())
}

/// Dove va un file del pacchetto, o `None` se non serve.
///
/// Si tiene `min/vs` — l'editor come lo carica una pagina — tranne:
/// - `language/`: il sorgente dei servizi di lingua, che l'editor non chiede
///   (li chiede gia' impacchettati, da `assets/`);
/// - le traduzioni dell'interfaccia di Monaco, tranne l'italiano;
/// - le dichiarazioni di tipo e le mappe, che servono a chi sviluppa Monaco.
fn serve(nome: &str) -> Option<String> {
    let dentro = nome.strip_prefix("package/min/")?;
    if !dentro.starts_with("vs/")
        || dentro.starts_with("vs/language/")
        || dentro.ends_with(".d.ts")
        || dentro.ends_with(".map")
        || (dentro.starts_with("vs/nls/lang/") && dentro != "vs/nls/lang/it.js")
        || dentro.contains("..")
    {
        return None;
    }
    Some(dentro.to_string())
}

/// Le voci di un archivio tar: nome e contenuto dei file normali.
///
/// Scritto qui invece di prendere un crate: il pacchetto npm e' un tar
/// semplice, e le poche righe sotto sono tutto quel che serve per leggerlo.
/// Le intestazioni «pax» (`x`, `g`) portano al massimo un nome lungo, e si
/// rispettano.
fn voci_tar(tar: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut voci = Vec::new();
    let mut i = 0;
    let mut nome_lungo: Option<String> = None;
    while i + 512 <= tar.len() {
        let testa = &tar[i..i + 512];
        if testa.iter().all(|&b| b == 0) {
            break;
        }
        let campo = |da: usize, a: usize| {
            let c = &testa[da..a];
            let fine = c.iter().position(|&b| b == 0).unwrap_or(c.len());
            String::from_utf8_lossy(&c[..fine]).to_string()
        };
        let dimensione = usize::from_str_radix(campo(124, 136).trim(), 8)
            .map_err(|e| format!("archivio rovinato alla posizione {i}: {e}"))?;
        let tipo = testa[156];
        let inizio = i + 512;
        let fine = inizio + dimensione;
        if fine > tar.len() {
            return Err("archivio troncato".into());
        }
        let dati = &tar[inizio..fine];
        match tipo {
            b'x' => {
                // «NN path=...\n»: si cerca solo il nome.
                let testo = String::from_utf8_lossy(dati);
                nome_lungo = testo
                    .lines()
                    .find_map(|r| r.split_once(" path=").map(|(_, p)| p.to_string()));
            }
            b'0' | 0 => {
                let prefisso = campo(345, 500);
                let nome = nome_lungo.take().unwrap_or_else(|| {
                    if prefisso.is_empty() {
                        campo(0, 100)
                    } else {
                        format!("{prefisso}/{}", campo(0, 100))
                    }
                });
                voci.push((nome, dati.to_vec()));
            }
            _ => nome_lungo = None,
        }
        i = inizio + dimensione.div_ceil(512) * 512;
    }
    Ok(voci)
}
