//! `nova componenti`: procurare i pezzi che mancano, senza Python (D351).
//!
//! Il catalogo e le regole — cosa serve, dove va, quando un pezzo c'e',
//! come si chiama mentre arriva, quando si parla — stanno in
//! `nova-componenti`, confrontati col Python da un banco. Qui c'e' quello che
//! quel crate lascia apposta a qualcun altro: la rete, gli zip, il disco,
//! l'msi di espeak-ng. Lo usano l'installatore e il pannello, che fin qui
//! lanciavano `python -m nova.componenti` e quindi volevano Python per
//! scaricare una voce.
//!
//! Gli eventi sono quelli del Python, una riga JSON per evento, sciacquata
//! subito: chi legge dall'altra parte mostra l'avanzamento mentre succede.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use nova_componenti::{
    catalogo, deve_parlare, in_arrivo, percento, stato_di, Componente, Pezzo, Tipo, BLOCCO,
};

fn radice() -> PathBuf {
    nova_configurazione::dove::radice_progetto()
}

fn il_catalogo() -> Vec<Componente> {
    let r = radice();
    catalogo(&r, &r.join("runtime"))
}

/// `nova componenti elenco`: cosa c'e' e cosa manca, senza rete.
pub fn elenco() -> Value {
    let esiste = |p: &Path| p.exists();
    Value::Array(
        il_catalogo()
            .iter()
            .map(|c| {
                let s = stato_di(c, &esiste);
                json!({
                    "nome": s.nome, "titolo": s.titolo, "serve_a": s.serve_a,
                    "senza": s.senza, "licenza": s.licenza, "mb": s.mb,
                    "presente": s.presente, "mancano": s.mancano, "totale": s.totale,
                })
            })
            .collect(),
    )
}

/// Un indirizzo, passato dallo specchio se ce n'e' uno.
///
/// `NOVA_COMPONENTI_SPECCHIO=http://specchio.interno/` fa chiedere
/// `http://specchio.interno/<percorso>` invece di `https://github.com/<percorso>`:
/// serve a chi sta dietro una rete che GitHub non lo raggiunge, e alla prova,
/// che non puo' scaricare mezzo giga da internet a ogni giro.
fn specchio(url: &str) -> String {
    let Ok(base) = std::env::var("NOVA_COMPONENTI_SPECCHIO") else {
        return url.to_string();
    };
    let senza_schema = url.split_once("://").map_or(url, |(_, r)| r);
    let percorso = senza_schema.split_once('/').map_or("", |(_, p)| p);
    format!("{}/{percorso}", base.trim_end_matches('/'))
}

fn agente() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(60))
        .timeout_read(std::time::Duration::from_secs(60))
        .user_agent("nova")
        .build()
}

/// Scarica un file raccontando, e lo mette al suo nome solo a fine corsa.
/// Un `.parte` rimasto non si riprende: si toglie e si ricomincia.
fn scarica_file(url: &str, dove: &Path, parla: &mut dyn FnMut(Value)) -> Result<(), String> {
    if let Some(p) = dove.parent() {
        std::fs::create_dir_all(p).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    let parte = in_arrivo(dove);
    let _ = std::fs::remove_file(&parte);
    let esito = (|| -> Result<(), String> {
        let r = agente().get(&specchio(url)).call().map_err(|e| match e {
            ureq::Error::Status(codice, _) => format!("il server ha risposto {codice} per {url}"),
            altro => format!("non riesco a scaricare {url}: {altro}"),
        })?;
        let totale: u64 = r.header("Content-Length").and_then(|v| v.parse().ok()).unwrap_or(0);
        let nome = dove.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let mut f = std::fs::File::create(&parte).map_err(|e| format!("{}: {e}", parte.display()))?;
        let mut lettore = r.into_reader();
        let mut blocco = vec![0u8; BLOCCO];
        let (mut fatto, mut ultima) = (0u64, None);
        loop {
            let n = lettore.read(&mut blocco).map_err(|e| format!("lo scaricamento si e' interrotto: {e}"))?;
            if n == 0 {
                break;
            }
            f.write_all(&blocco[..n]).map_err(|e| format!("{}: {e}", parte.display()))?;
            fatto += n as u64;
            let perc = percento(fatto, totale);
            if deve_parlare(perc, ultima) {
                ultima = Some(perc);
                parla(json!({ "evento": "avanzamento", "percento": perc, "byte": fatto,
                              "totale": totale, "file": nome }));
            }
        }
        f.flush().map_err(|e| e.to_string())?;
        drop(f);
        std::fs::rename(&parte, dove).map_err(|e| format!("{}: {e}", dove.display()))
    })();
    if esito.is_err() {
        let _ = std::fs::remove_file(&parte);
    }
    esito
}

fn cartella_temporanea(prefisso: &str) -> Result<PathBuf, String> {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let d = std::env::temp_dir().join(format!("{prefisso}{}_{t}", std::process::id()));
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    Ok(d)
}

fn impronta(testo: &str) -> u64 {
    testo.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3))
}

/// Tutti i file sotto una cartella.
fn tutti_i_file(d: &Path, fuori: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            tutti_i_file(&p, fuori);
        } else {
            fuori.push(p);
        }
    }
}

fn copia_albero(da: &Path, a: &Path) -> Result<(), String> {
    std::fs::create_dir_all(a).map_err(|e| e.to_string())?;
    for e in std::fs::read_dir(da).map_err(|e| e.to_string())?.flatten() {
        let (p, q) = (e.path(), a.join(e.file_name()));
        if p.is_dir() {
            copia_albero(&p, &q)?;
        } else {
            std::fs::copy(&p, &q).map_err(|e| format!("{}: {e}", q.display()))?;
        }
    }
    Ok(())
}

fn estrai_zip(zip: &Path, dove: &Path) -> Result<(), String> {
    let f = std::fs::File::open(zip).map_err(|e| e.to_string())?;
    let mut a = zip::ZipArchive::new(f).map_err(|e| format!("l'archivio non si apre: {e}"))?;
    a.extract(dove).map_err(|e| format!("l'archivio non si estrae: {e}"))
}

fn procura(pezzo: &Pezzo, parla: &mut dyn FnMut(Value)) -> Result<(), String> {
    match &pezzo.tipo {
        Tipo::Copia { da } => {
            // Come il Python: se la sorgente non c'e', non c'e' niente da fare.
            if da.exists() {
                if let Some(p) = pezzo.dove.parent() {
                    std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
                }
                std::fs::copy(da, &pezzo.dove).map_err(|e| format!("{}: {e}", pezzo.dove.display()))?;
            }
            Ok(())
        }
        Tipo::File { url } => scarica_file(url, &pezzo.dove, parla),
        Tipo::ZipDll { url, .. } | Tipo::ZipPiatto { url } => {
            std::fs::create_dir_all(&pezzo.dove).map_err(|e| e.to_string())?;
            let zip = std::env::temp_dir().join(format!("nova_{:x}.zip", impronta(url)));
            scarica_file(url, &zip, parla)?;
            parla(json!({ "evento": "lavoro", "messaggio": "estraggo" }));
            let estratto = cartella_temporanea("nova_zip_")?;
            let esito = (|| -> Result<(), String> {
                estrai_zip(&zip, &estratto)?;
                let mut file = Vec::new();
                tutti_i_file(&estratto, &mut file);
                for f in file {
                    let nome = f.file_name().unwrap_or_default().to_string_lossy().to_string();
                    let prendi = match &pezzo.tipo {
                        Tipo::ZipDll { filtro, .. } => {
                            nome.to_lowercase().ends_with(".dll") && nome.to_lowercase().contains(filtro.as_str())
                        }
                        // I rilasci di whisper.cpp mettono tutto in una
                        // sottocartella: si appiattisce.
                        _ => true,
                    };
                    if prendi {
                        std::fs::copy(&f, pezzo.dove.join(&nome)).map_err(|e| format!("{nome}: {e}"))?;
                    }
                }
                Ok(())
            })();
            let _ = std::fs::remove_dir_all(&estratto);
            let _ = std::fs::remove_file(&zip);
            esito
        }
        Tipo::MsiEspeak => espeak(&pezzo.dove, parla),
    }
}

/// La dll e i dati di espeak-ng, presi dall'msi ufficiale senza installarlo:
/// `msiexec /a` apre l'archivio in una cartella e basta, niente registro e
/// niente voce in «Installazione applicazioni».
fn espeak(dove: &Path, parla: &mut dyn FnMut(Value)) -> Result<(), String> {
    std::fs::create_dir_all(dove).map_err(|e| e.to_string())?;
    let rilascio: Value = agente()
        .get("https://api.github.com/repos/espeak-ng/espeak-ng/releases/latest")
        .call()
        .map_err(|e| format!("non riesco a chiedere l'ultimo rilascio di espeak-ng: {e}"))?
        .into_json()
        .map_err(|e| e.to_string())?;
    let asset = rilascio["assets"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|a| a["name"].as_str().is_some_and(|n| n.to_lowercase().ends_with("x64.msi")))
        .ok_or("nessun pacchetto x64 nell'ultimo rilascio di espeak-ng")?;
    let nome = asset["name"].as_str().unwrap_or("espeak-ng.msi");
    let url = asset["browser_download_url"].as_str().ok_or("il rilascio non dice da dove scaricare")?;
    let msi = std::env::temp_dir().join(nome);
    scarica_file(url, &msi, parla)?;
    parla(json!({ "evento": "lavoro", "messaggio": "apro il pacchetto" }));
    let estratto = cartella_temporanea("nova_espeak_")?;
    let esito = (|| -> Result<(), String> {
        let s = std::process::Command::new("msiexec")
            .arg("/a")
            .arg(&msi)
            .arg("/qn")
            .arg(format!("TARGETDIR={}", estratto.display()))
            .status()
            .map_err(|e| format!("msiexec non parte: {e}"))?;
        if !s.success() {
            return Err(format!("msiexec e' uscito con {s}"));
        }
        let mut file = Vec::new();
        tutti_i_file(&estratto, &mut file);
        if let Some(dll) = file.iter().find(|f| f.file_name().is_some_and(|n| n == "espeak-ng.dll")) {
            std::fs::copy(dll, dove.join("espeak-ng.dll")).map_err(|e| e.to_string())?;
        }
        let dati = file
            .iter()
            .filter_map(|f| f.ancestors().find(|a| a.file_name().is_some_and(|n| n == "espeak-ng-data")))
            .next()
            .map(Path::to_path_buf);
        if let Some(d) = dati {
            copia_albero(&d, &dove.join("espeak-ng-data"))?;
        }
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&estratto);
    let _ = std::fs::remove_file(&msi);
    esito?;
    if !dove.join("espeak-ng.dll").exists() {
        return Err("espeak-ng.dll non e' uscita dal pacchetto".into());
    }
    Ok(())
}

fn con(e: Value, extra: &[(&str, Value)]) -> Value {
    let mut m: Map<String, Value> = e.as_object().cloned().unwrap_or_default();
    for (k, v) in extra {
        m.insert((*k).to_string(), v.clone());
    }
    Value::Object(m)
}

/// `nova componenti scarica <nome>`: procura i pezzi che mancano. Torna
/// falso se qualcosa e' andato storto (e l'ha gia' detto).
pub fn scarica(nome: &str, parla: &mut dyn FnMut(Value)) -> bool {
    let catalogo = il_catalogo();
    let Some(voluto) = catalogo.iter().find(|c| c.nome == nome) else {
        parla(json!({ "evento": "errore", "messaggio": format!("componente sconosciuto: {nome}") }));
        return false;
    };
    let esiste = |p: &Path| p.exists();
    let mancanti: Vec<&Pezzo> = voluto.pezzi.iter().filter(|p| !p.presente(&esiste)).collect();
    if mancanti.is_empty() {
        parla(json!({ "evento": "finito", "componente": nome, "messaggio": "c'era gia' tutto" }));
        return true;
    }
    let di = mancanti.len();
    parla(json!({ "evento": "inizio", "componente": nome, "pezzi": di, "mb": voluto.mb }));
    for (i, pezzo) in mancanti.iter().enumerate() {
        let i = i + 1;
        let mut racconta = |e: Value| {
            parla(con(e, &[("componente", json!(nome)), ("pezzo", json!(i)), ("di", json!(di))]))
        };
        if let Err(messaggio) = procura(pezzo, &mut racconta) {
            parla(json!({ "evento": "errore", "componente": nome, "pezzo": i, "messaggio": messaggio }));
            return false;
        }
        parla(json!({ "evento": "pezzo", "componente": nome, "pezzo": i, "di": di }));
    }
    parla(json!({ "evento": "finito", "componente": nome }));
    true
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn lo_specchio_tiene_il_percorso() {
        std::env::set_var("NOVA_COMPONENTI_SPECCHIO", "http://127.0.0.1:8/x/");
        assert_eq!(specchio("https://github.com/a/b.zip"), "http://127.0.0.1:8/x/a/b.zip");
        std::env::remove_var("NOVA_COMPONENTI_SPECCHIO");
        assert_eq!(specchio("https://github.com/a/b.zip"), "https://github.com/a/b.zip");
    }

    #[test]
    fn un_nome_sconosciuto_lo_dice() {
        let mut eventi = Vec::new();
        assert!(!scarica("boh", &mut |e| eventi.push(e)));
        assert_eq!(eventi[0]["evento"], "errore");
        assert!(eventi[0]["messaggio"].as_str().unwrap().contains("boh"));
    }

    #[test]
    fn l_elenco_ha_tutti_i_componenti() {
        let e = elenco();
        let nomi: Vec<&str> = e.as_array().unwrap().iter().filter_map(|c| c["nome"].as_str()).collect();
        assert_eq!(nomi, ["voce_locale", "onnx", "espeak", "ascolto_locale"]);
    }

    #[test]
    fn un_file_che_non_arriva_non_lascia_niente() {
        let d = std::env::temp_dir().join(format!("nova-comp-{}", std::process::id()));
        let dove = d.join("x.bin");
        let mut eventi = Vec::new();
        let r = scarica_file("http://127.0.0.1:9/niente", &dove, &mut |e| eventi.push(e));
        assert!(r.is_err());
        assert!(!dove.exists() && !in_arrivo(&dove).exists());
        let _ = std::fs::remove_dir_all(&d);
    }
}
