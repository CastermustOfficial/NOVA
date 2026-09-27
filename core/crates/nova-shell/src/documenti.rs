//! I documenti dell'harness che non sono testo: il PDF con le pagine vere,
//! il Word paragrafo per paragrafo, l'HTML disegnato, le immagini.
//!
//! Quarta fase di `docs/harness.md` (D341). Qui c'e' la meta' Rust:
//!
//! - **i file alla pagina.** La finestra li chiede per indirizzo, come una
//!   pagina web chiede un'immagine: `novafile://` e' un protocollo del
//!   guscio che serve i file di disco. Non tutti: solo quelli dentro le
//!   cartelle che l'harness ha aperto, e solo quelli di un tipo che la
//!   pagina sa mostrare. Un indirizzo e non una chiamata perche' cosi' un
//!   HTML trova da se' il suo foglio di stile e le sue immagini, accanto;
//! - **il Word**, letto nel suo ordine e riscritto con la chirurgia di
//!   `nova-docx` (D276): si tocca solo il paragrafo cambiato.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

use serde_json::{json, Value};

/// Le cartelle da cui il protocollo puo' servire file.
static CONSENTITE: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// Il nome del protocollo. Su Windows la finestra lo chiama
/// `http://novafile.localhost/...`, altrove `novafile://localhost/...`.
pub const PROTOCOLLO_DEI_FILE: &str = "novafile";

/// Il tipo di un file, per chi lo riceve. Solo quelli che una pagina sa
/// mostrare: il resto non si serve.
pub fn tipo_di(percorso: &Path) -> Option<&'static str> {
    let est = percorso
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    Some(match est.as_str() {
        "pdf" => "application/pdf",
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "svg" => "image/svg+xml",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "txt" | "md" => "text/plain; charset=utf-8",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        _ => return None,
    })
}

/// Da un indirizzo del protocollo al percorso sul disco.
///
/// Il percorso viaggia a pezzi codificati: `C%3A/Users/x/a.pdf` su Windows,
/// `/home/x/a.pdf` altrove (l'indirizzo e' `novafile://localhost//home/...`).
pub fn percorso_da(indirizzo_path: &str) -> PathBuf {
    let senza = indirizzo_path.strip_prefix('/').unwrap_or(indirizzo_path);
    PathBuf::from(sciogli(senza))
}

/// La codifica `%xx` degli indirizzi, tolta. I byte si rimettono insieme
/// prima di diventare testo: una lettera accentata sono due `%xx` di fila.
fn sciogli(s: &str) -> String {
    let b = s.as_bytes();
    let mut fuori = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let cifra = |c: u8| (c as char).to_digit(16);
            if let (Some(a), Some(z)) = (cifra(b[i + 1]), cifra(b[i + 2])) {
                fuori.push((a * 16 + z) as u8);
                i += 3;
                continue;
            }
        }
        fuori.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&fuori).into_owned()
}

/// Se questo file si puo' servire: sta dentro una cartella aperta
/// dall'harness. Si confronta il percorso **vero**, dopo aver sciolto i
/// `..` e i collegamenti: `cartella/../../segreti` non e' dentro `cartella`.
pub fn consentito(file: &Path, cartelle: &[PathBuf]) -> bool {
    let Ok(vero) = std::fs::canonicalize(file) else {
        return false;
    };
    cartelle.iter().any(|c| vero.starts_with(c))
}

/// La cartella di questo percorso entra fra quelle servibili.
#[tauri::command]
pub fn harness_consenti(percorso: String) -> Result<(), String> {
    let p = PathBuf::from(&percorso);
    let cartella = if p.is_dir() {
        p
    } else {
        p.parent().map(Path::to_path_buf).unwrap_or_default()
    };
    let vera =
        std::fs::canonicalize(&cartella).map_err(|e| format!("{}: {e}", cartella.display()))?;
    let mut c = CONSENTITE.lock().unwrap_or_else(|e| e.into_inner());
    if !c.contains(&vera) {
        c.push(vera);
    }
    Ok(())
}

/// Risponde a una richiesta del protocollo.
pub fn rispondi(richiesta: &tauri::http::Request<Vec<u8>>) -> tauri::http::Response<Vec<u8>> {
    let risposta = |stato: u16, tipo: &str, corpo: Vec<u8>| {
        tauri::http::Response::builder()
            .status(stato)
            .header("Content-Type", tipo)
            .header("Access-Control-Allow-Origin", "*")
            .body(corpo)
            .unwrap_or_default()
    };
    let file = percorso_da(richiesta.uri().path());
    let Some(tipo) = tipo_di(&file) else {
        return risposta(415, "text/plain", b"tipo di file non servito".to_vec());
    };
    let cartelle = CONSENTITE.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if !consentito(&file, &cartelle) {
        return risposta(403, "text/plain", b"fuori dalle cartelle aperte".to_vec());
    }
    match std::fs::read(&file) {
        Ok(dati) => risposta(200, tipo, dati),
        Err(_) => risposta(404, "text/plain", b"non c'e'".to_vec()),
    }
}

// ------------------------------------------------------------ il Word

fn modificato_il(p: &Path) -> Option<f64> {
    std::fs::metadata(p)
        .ok()?
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_millis() as f64)
}

fn in_json(p: &nova_docx::scrittura::Pezzo) -> Value {
    use nova_docx::scrittura::Pezzo;
    match p {
        Pezzo::Paragrafo {
            indice,
            testo,
            stile,
        } => {
            json!({ "tipo": "p", "indice": indice, "testo": testo, "stile": stile })
        }
        Pezzo::Tabella { indice, righe } => {
            json!({ "tipo": "t", "indice": indice, "righe": righe })
        }
    }
}

pub fn leggi_docx(f: &Path) -> Result<Value, String> {
    let xml = nova_docx::leggi_parte(f, nova_docx::DOCUMENTO)?;
    let pezzi: Vec<Value> = nova_docx::scrittura::corpo(&xml)
        .iter()
        .map(in_json)
        .collect();
    Ok(json!({ "pezzi": pezzi, "modificato": modificato_il(f) }))
}

/// Le modifiche come le manda la pagina: `{indice, testo}`,
/// `{indice, togli: true}`, `{dopo: indice|null, testi: [...]}`.
pub fn cambi_da(v: &[Value]) -> Result<Vec<nova_docx::scrittura::Cambio>, String> {
    use nova_docx::scrittura::Cambio;
    v.iter()
        .map(|c| {
            let indice = c.get("indice").and_then(Value::as_u64).map(|n| n as usize);
            if let Some(testi) = c.get("testi").and_then(Value::as_array) {
                return Ok(Cambio::Dopo {
                    indice: c.get("dopo").and_then(Value::as_u64).map(|n| n as usize),
                    testi: testi
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect(),
                });
            }
            let Some(indice) = indice else {
                return Err(format!("una modifica senza paragrafo: {c}"));
            };
            if c.get("togli").and_then(Value::as_bool) == Some(true) {
                return Ok(Cambio::Togli { indice });
            }
            match c.get("testo").and_then(Value::as_str) {
                Some(t) => Ok(Cambio::Testo {
                    indice,
                    testo: t.to_string(),
                }),
                None => Err(format!("una modifica senza testo: {c}")),
            }
        })
        .collect()
}

pub fn salva_docx(f: &Path, cambi: &[Value], atteso: Option<f64>) -> Result<Value, String> {
    if let (Some(a), Some(ora)) = (atteso, modificato_il(f)) {
        if (ora - a).abs() > 1.0 {
            return Err(format!(
                "{} {} e' stato cambiato sul disco dopo che l'hai aperto",
                crate::harness::CAMBIATO_SOTTO,
                f.display()
            ));
        }
    }
    let cambi = cambi_da(cambi)?;
    let xml = nova_docx::leggi_parte(f, nova_docx::DOCUMENTO)?;
    let (nuovo, quante) = nova_docx::scrittura::applica(&xml, &cambi)?;
    if quante > 0 {
        nova_docx::riscrivi_parte(f, nova_docx::DOCUMENTO, &nuovo)?;
    }
    Ok(json!({ "modificato": modificato_il(f), "quante": quante }))
}

#[tauri::command]
pub async fn harness_docx(percorso: String) -> Result<Value, String> {
    tokio::task::spawn_blocking(move || leggi_docx(Path::new(&percorso)))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn harness_docx_salva(
    percorso: String,
    cambi: Vec<Value>,
    atteso: Option<f64>,
) -> Result<Value, String> {
    tokio::task::spawn_blocking(move || salva_docx(Path::new(&percorso), &cambi, atteso))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn i_tipi_serviti_sono_quelli_che_una_pagina_mostra() {
        assert_eq!(tipo_di(Path::new("a.PDF")), Some("application/pdf"));
        assert_eq!(
            tipo_di(Path::new("x/stile.css")),
            Some("text/css; charset=utf-8")
        );
        assert_eq!(tipo_di(Path::new("chiave.key")), None);
        assert_eq!(tipo_di(Path::new("senza")), None);
        assert_eq!(tipo_di(Path::new("programma.exe")), None);
    }

    #[test]
    fn l_indirizzo_torna_percorso_accenti_compresi() {
        assert_eq!(
            percorso_da("/D%3A/Studio/perch%C3%A9.pdf"),
            PathBuf::from("D:/Studio/perché.pdf")
        );
        assert_eq!(
            percorso_da("//srv/sito/a%20b.html"),
            PathBuf::from("/srv/sito/a b.html")
        );
        assert_eq!(
            percorso_da("/x%zz.pdf"),
            PathBuf::from("x%zz.pdf"),
            "un % rotto resta com'e'"
        );
        assert_eq!(percorso_da("/fine%2"), PathBuf::from("fine%2"));
        assert_eq!(
            percorso_da("/%è"),
            PathBuf::from("%è"),
            "un % prima di una lettera larga non rompe niente"
        );
    }

    #[test]
    fn si_serve_solo_dentro_le_cartelle_aperte() {
        let d = std::env::temp_dir().join(format!("nova-documenti-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("progetto/sotto")).unwrap();
        std::fs::create_dir_all(d.join("altro")).unwrap();
        std::fs::write(d.join("progetto/sotto/a.pdf"), "x").unwrap();
        std::fs::write(d.join("altro/b.pdf"), "x").unwrap();
        let aperte = vec![std::fs::canonicalize(d.join("progetto")).unwrap()];
        assert!(consentito(&d.join("progetto/sotto/a.pdf"), &aperte));
        assert!(!consentito(&d.join("altro/b.pdf"), &aperte));
        assert!(
            !consentito(&d.join("progetto/../altro/b.pdf"), &aperte),
            "i .. non scappano"
        );
        assert!(!consentito(&d.join("progetto/manca.pdf"), &aperte));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn le_modifiche_della_pagina_diventano_cambi() {
        use nova_docx::scrittura::Cambio;
        let c = cambi_da(&[
            json!({"indice": 2, "testo": "ciao"}),
            json!({"indice": 3, "togli": true}),
            json!({"dopo": 1, "testi": ["a", "b"]}),
            json!({"dopo": null, "testi": ["in testa"]}),
        ])
        .unwrap();
        assert_eq!(
            c[0],
            Cambio::Testo {
                indice: 2,
                testo: "ciao".into()
            }
        );
        assert_eq!(c[1], Cambio::Togli { indice: 3 });
        assert_eq!(
            c[2],
            Cambio::Dopo {
                indice: Some(1),
                testi: vec!["a".into(), "b".into()]
            }
        );
        assert_eq!(
            c[3],
            Cambio::Dopo {
                indice: None,
                testi: vec!["in testa".into()]
            }
        );
        assert!(cambi_da(&[json!({"testo": "x"})]).is_err());
        assert!(cambi_da(&[json!({"indice": 1})]).is_err());
    }
}
