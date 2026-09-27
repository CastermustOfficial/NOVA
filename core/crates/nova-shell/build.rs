//! Prima di compilare il guscio: l'editor e il terminale dell'harness, e poi
//! Tauri.
//!
//! **Monaco e xterm.js non stanno nel repository** (D336). Monaco sono
//! venticinque megabyte di JavaScript ridotto, che cambiano tutti a ogni
//! versione: tenerli in git vorrebbe dire un repository che ingrassa di dieci
//! mega a ogni aggiornamento dell'editor, per un file che nessuno legge. Qui
//! si scarica **una versione fissata** di ciascun pacchetto, si controlla
//! l'impronta che il registro npm pubblica per quella versione, e si tiene
//! solo la parte che una pagina carica davvero — misurata aprendo la pagina
//! e guardando cosa chiede.
//!
//! Senza rete non si ferma niente: il guscio si compila lo stesso, e
//! l'harness dice cosa manca invece di non aprirsi. Una compilazione che
//! fallisce perche' manca un editor sarebbe il guasto sbagliato nel posto
//! sbagliato — l'orb e la voce non c'entrano. Chi compila senza rete, o
//! dietro un proxy che il client HTTP non riconosce, mette i `.tgz` in una
//! cartella e la indica con `NOVA_PACCHETTI`: l'impronta si controlla lo
//! stesso.

use std::io::Read;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use sha2::Digest as _;

/// Un pacchetto npm da portare nel guscio.
struct Pacchetto {
    /// Il nome sul registro, con l'ambito se c'e' (`@xterm/xterm`).
    nome: &'static str,
    versione: &'static str,
    /// `npm view <nome>@<versione> dist.integrity`.
    impronta: &'static str,
    /// Dove finisce, sotto `ui/vendor`.
    cartella: &'static str,
    /// Quali file tenere, e dove: `None` per quelli che non servono.
    serve: fn(&str) -> Option<String>,
}

const PACCHETTI: [Pacchetto; 4] = [
    Pacchetto {
        nome: "monaco-editor",
        versione: "0.57.0",
        impronta: "sha512-5BkI9KGoqrNvBGUe15/QlZq3OooZ8WLg1AxTpaqHRCP3HNpzPPZKE2EDz8M7c+VRmCeUw1Brp4cx/PWm3kI/5A==",
        cartella: "monaco",
        serve: serve_monaco,
    },
    Pacchetto {
        nome: "@xterm/xterm",
        versione: "6.0.0",
        impronta: "sha512-TQwDdQGtwwDt+2cgKDLn0IRaSxYu1tSUjgKarSDkUM0ZNiSRXFpjxEsvc/Zgc5kq5omJ+V0a8/kIM2WD3sMOYg==",
        cartella: "xterm",
        serve: serve_xterm,
    },
    Pacchetto {
        nome: "@xterm/addon-fit",
        versione: "0.11.0",
        impronta: "sha512-jYcgT6xtVYhnhgxh3QgYDnnNMYTcf8ElbxxFzX0IZo+vabQqSPAjC3c1wJrKB5E19VwQei89QCiZZP86DCPF7g==",
        cartella: "xterm-fit",
        serve: serve_xterm,
    },
    Pacchetto {
        nome: "pdfjs-dist",
        versione: "6.3.289",
        impronta: "sha512-ZHjSVpDa3D6izMq8/04lvkhkATUmL9px6ChPaXc1k6nU2Mrhlg1/7F0bdUqCwUjw3NsPTfPZsMDUU6ZIcRaeQw==",
        cartella: "pdfjs",
        serve: serve_pdfjs,
    },
];

fn main() {
    println!("cargo:rerun-if-env-changed=NOVA_PACCHETTI");
    for p in &PACCHETTI {
        let dove = PathBuf::from("ui").join("vendor").join(p.cartella);
        let segno = dove.join("VERSIONE");
        println!("cargo:rerun-if-changed={}", segno.display());
        if std::fs::read_to_string(&segno).is_ok_and(|v| v.trim() == p.versione) {
            continue;
        }
        if let Err(e) = porta(p, &dove) {
            println!(
                "cargo:warning={} {} non e' stato scaricato: {e}. Il guscio funziona lo stesso; \
                 per averlo ricompila con la rete, o indica i pacchetti con NOVA_PACCHETTI.",
                p.nome, p.versione
            );
        }
    }
    tauri_build::build()
}

/// Il nome del file che `npm pack` scrive per quel pacchetto.
fn nome_del_file(p: &Pacchetto) -> String {
    format!(
        "{}-{}.tgz",
        p.nome.trim_start_matches('@').replace('/', "-"),
        p.versione
    )
}

/// Il pacchetto: da `NOVA_PACCHETTI`, se c'e', o dal registro npm.
fn scarica(p: &Pacchetto) -> Result<Vec<u8>, String> {
    if let Some(d) = std::env::var_os("NOVA_PACCHETTI") {
        let f = Path::new(&d).join(nome_del_file(p));
        return std::fs::read(&f).map_err(|e| format!("leggendo {}: {e}", f.display()));
    }
    let breve = p.nome.rsplit('/').next().unwrap_or(p.nome);
    let url = format!(
        "https://registry.npmjs.org/{}/-/{breve}-{}.tgz",
        p.nome, p.versione
    );
    let risposta = ureq::get(&url)
        .timeout(std::time::Duration::from_secs(180))
        .call()
        .map_err(|e| format!("scaricando {url}: {e}"))?;
    let mut compresso = Vec::new();
    risposta
        .into_reader()
        .take(80 << 20)
        .read_to_end(&mut compresso)
        .map_err(|e| format!("leggendo {url}: {e}"))?;
    Ok(compresso)
}

fn porta(p: &Pacchetto, dove: &Path) -> Result<(), String> {
    let compresso = scarica(p)?;
    let impronta = format!(
        "sha512-{}",
        base64::engine::general_purpose::STANDARD.encode(sha2::Sha512::digest(&compresso))
    );
    if impronta != p.impronta {
        return Err(format!(
            "l'impronta non torna: attesa {}, arrivata {impronta}",
            p.impronta
        ));
    }

    let mut tar = Vec::new();
    flate2::read::GzDecoder::new(&compresso[..])
        .read_to_end(&mut tar)
        .map_err(|e| format!("decomprimendo: {e}"))?;

    // Si scrive accanto e si sposta alla fine: un'interruzione a meta' non
    // deve lasciare un pacchetto mezzo copiato con scritto sopra «fatto».
    let provvisoria = dove.with_extension("nuova");
    let _ = std::fs::remove_dir_all(&provvisoria);
    let mut quanti = 0;
    for (nome, dati) in voci_tar(&tar)? {
        let Some(relativo) = (p.serve)(&nome) else {
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
    std::fs::write(provvisoria.join("VERSIONE"), format!("{}\n", p.versione))
        .map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(dove);
    std::fs::rename(&provvisoria, dove).map_err(|e| e.to_string())?;
    Ok(())
}

/// Di Monaco si tiene `min/vs` — l'editor come lo carica una pagina — tranne:
/// - `language/`: il sorgente dei servizi di lingua, che l'editor non chiede
///   (li chiede gia' impacchettati, da `assets/`);
/// - le traduzioni dell'interfaccia di Monaco, tranne l'italiano;
/// - le dichiarazioni di tipo e le mappe, che servono a chi sviluppa Monaco.
fn serve_monaco(nome: &str) -> Option<String> {
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

/// Di pdf.js si tiene la libreria e il suo lavoratore, ridotti, e quel che
/// servono per disegnare i PDF veri: le tabelle dei caratteri orientali
/// (`cmaps`), i caratteri standard che un PDF puo' non contenere, i profili
/// di colore e i moduli `wasm` per le immagini JPEG 2000. Il visualizzatore
/// completo di Mozilla no: le pagine le disegna l'harness.
///
/// La libreria e' quella **legacy**, con dentro i rattoppi per i motori un
/// po' indietro: quella normale usa `Map.getOrInsertComputed`, che Chromium
/// 140 non ha ancora (misurato: le pagine restavano bianche), e la finestra
/// web di Windows si aggiorna quando vuole lei. Si mette dove la pagina la
/// cerca, in `build/`.
fn serve_pdfjs(nome: &str) -> Option<String> {
    let dentro = nome.strip_prefix("package/")?;
    if dentro.contains("..") || dentro.ends_with(".map") {
        return None;
    }
    match dentro {
        "legacy/build/pdf.min.mjs" => return Some("build/pdf.min.mjs".into()),
        "legacy/build/pdf.worker.min.mjs" => return Some("build/pdf.worker.min.mjs".into()),
        "web/pdf_viewer.css" | "LICENSE" => return Some(dentro.to_string()),
        _ => {}
    }
    ["cmaps/", "standard_fonts/", "wasm/", "iccs/"]
        .iter()
        .any(|c| dentro.starts_with(c))
        .then(|| dentro.to_string())
}

/// Di xterm.js e del suo adattatore si tiene lo script da caricare con un
/// `<script>` e il foglio di stile: niente moduli, niente mappe.
fn serve_xterm(nome: &str) -> Option<String> {
    let dentro = nome.strip_prefix("package/")?;
    let tenuto =
        (dentro.starts_with("lib/") && dentro.ends_with(".js") && !dentro.ends_with(".mjs"))
            || dentro == "css/xterm.css"
            || dentro == "LICENSE";
    (tenuto && !dentro.contains("..")).then(|| dentro.to_string())
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
