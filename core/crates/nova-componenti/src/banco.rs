//! Il banco: il catalogo e le regole, per il confronto col Python.
//!
//! Il catalogo si confronta **voce per voce**, percorsi compresi: sono le due
//! meta' della stessa scelta, e una versione aggiornata da una parte sola e'
//! un componente che il pannello dice presente e il demone non trova.
use nova_componenti::{catalogo, deve_parlare, in_arrivo, percento, quanto_manca, stato_di, Tipo};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
#[serde(tag = "tipo")]
enum Domanda {
    #[serde(rename = "catalogo")]
    Catalogo { radice: String, runtime: String },
    #[serde(rename = "stato")]
    Stato { radice: String, runtime: String, ci_sono: Vec<String> },
    #[serde(rename = "manca")]
    Manca { radice: String, runtime: String, ci_sono: Vec<String> },
    #[serde(rename = "in_arrivo")]
    InArrivo { dove: String },
    #[serde(rename = "percento")]
    Percento { fatto: u64, totale: u64 },
    #[serde(rename = "parlare")]
    Parlare { percento: u8, ultima: Option<u8> },
}

#[derive(Serialize)]
struct Risposta {
    #[serde(skip_serializing_if = "Option::is_none")]
    voci: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    testo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    numero: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    si: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    errore: Option<String>,
}

fn vuota() -> Risposta {
    Risposta { voci: None, testo: None, numero: None, si: None, errore: None }
}

fn tipo_nome(t: &Tipo) -> &'static str {
    match t {
        Tipo::File { .. } => "file",
        Tipo::Copia { .. } => "copia",
        Tipo::ZipDll { .. } => "zip_dll",
        Tipo::ZipPiatto { .. } => "zip_piatto",
        Tipo::MsiEspeak => "msi_espeak",
    }
}

fn url_di(t: &Tipo) -> String {
    match t {
        Tipo::File { url } | Tipo::ZipDll { url, .. } | Tipo::ZipPiatto { url } => url.clone(),
        Tipo::Copia { da } => da.display().to_string(),
        Tipo::MsiEspeak => String::new(),
    }
}

fn main() {
    let mut dentro = String::new();
    if std::io::Read::read_to_string(&mut std::io::stdin(), &mut dentro).is_err() {
        eprintln!("non riesco a leggere stdin");
        std::process::exit(1);
    }
    for riga in dentro.lines() {
        let riga = riga.trim();
        if riga.is_empty() {
            continue;
        }
        let r = match serde_json::from_str::<Domanda>(riga) {
            Err(e) => Risposta { errore: Some(format!("domanda illeggibile: {e}")), ..vuota() },
            Ok(Domanda::Catalogo { radice, runtime }) => {
                let c = catalogo(Path::new(&radice), Path::new(&runtime));
                Risposta {
                    voci: Some(serde_json::json!(c
                        .iter()
                        .map(|x| serde_json::json!({
                            "nome": x.nome, "titolo": x.titolo, "serve_a": x.serve_a,
                            "senza": x.senza, "mb": x.mb, "licenza": x.licenza,
                            "pezzi": x.pezzi.iter().map(|p| serde_json::json!({
                                "tipo": tipo_nome(&p.tipo),
                                "url": url_di(&p.tipo),
                                "filtro": match &p.tipo {
                                    Tipo::ZipDll { filtro, .. } => filtro.clone(),
                                    _ => String::new(),
                                },
                                "dove": p.dove.display().to_string(),
                                "prova": p.prova.as_ref()
                                    .map(|x| x.display().to_string()).unwrap_or_default(),
                                "vale_anche": p.vale_anche,
                            })).collect::<Vec<_>>(),
                        }))
                        .collect::<Vec<_>>())),
                    ..vuota()
                }
            }
            Ok(Domanda::Stato { radice, runtime, ci_sono }) => {
                let presenti: HashSet<PathBuf> = ci_sono.iter().map(PathBuf::from).collect();
                let esiste = move |p: &Path| presenti.contains(p);
                let c = catalogo(Path::new(&radice), Path::new(&runtime));
                Risposta {
                    voci: Some(serde_json::json!(c
                        .iter()
                        .map(|x| {
                            let s = stato_di(x, &esiste);
                            serde_json::json!({
                                "nome": s.nome, "titolo": s.titolo, "serve_a": s.serve_a,
                                "senza": s.senza, "licenza": s.licenza, "mb": s.mb,
                                "presente": s.presente, "mancano": s.mancano,
                                "totale": s.totale,
                            })
                        })
                        .collect::<Vec<_>>())),
                    ..vuota()
                }
            }
            Ok(Domanda::Manca { radice, runtime, ci_sono }) => {
                let presenti: HashSet<PathBuf> = ci_sono.iter().map(PathBuf::from).collect();
                let esiste = move |p: &Path| presenti.contains(p);
                let c = catalogo(Path::new(&radice), Path::new(&runtime));
                Risposta { numero: Some(quanto_manca(&c, &esiste) as u64), ..vuota() }
            }
            Ok(Domanda::InArrivo { dove }) => Risposta {
                testo: Some(in_arrivo(Path::new(&dove)).display().to_string()),
                ..vuota()
            },
            Ok(Domanda::Percento { fatto, totale }) => {
                Risposta { numero: Some(percento(fatto, totale) as u64), ..vuota() }
            }
            Ok(Domanda::Parlare { percento, ultima }) => {
                Risposta { si: Some(deve_parlare(percento, ultima)), ..vuota() }
            }
        };
        println!("{}", serde_json::to_string(&r).unwrap());
    }
}
