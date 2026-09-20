//! Il banco: una riga JSON per caso, per il confronto col Python.
//!
//! Le misure si confrontano a carattere, e non e' pignoleria: lo stesso
//! numero deve leggersi uguale nel racconto, nel rendiconto del
//! disinstallatore e nel pannello, o sembrano tre programmi diversi che
//! dicono tre cose diverse sullo stesso file.
use nova_dati::{pesa, racconta, rendiconto, Misura, Posto};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(tag = "tipo")]
enum Domanda {
    #[serde(rename = "pesa")]
    Pesa { byte: u64 },
    #[serde(rename = "racconta")]
    Racconta {
        posti: Vec<VocePosto>,
        base: String,
        #[serde(default = "vero")]
        solo_esistenti: bool,
    },
    #[serde(rename = "dentro")]
    Dentro { quale: String, cartella: String },
    #[serde(rename = "rendiconto")]
    Rendiconto { posti: Vec<VocePosto>, base: String },
}

fn vero() -> bool {
    true
}

#[derive(Deserialize)]
struct VocePosto {
    che_cos_e: String,
    dove: String,
    se_lo_cancelli: String,
    #[serde(default)]
    delicato: bool,
    #[serde(default)]
    esiste: bool,
    #[serde(default)]
    byte: u64,
    #[serde(default)]
    quanti_file: u64,
}

impl VocePosto {
    fn in_due(&self) -> (Posto, Misura) {
        let mut p = Posto::nuovo(&self.che_cos_e, &self.dove, &self.se_lo_cancelli);
        if self.delicato {
            p = p.delicato();
        }
        (
            p,
            Misura { esiste: self.esiste, byte: self.byte, quanti_file: self.quanti_file },
        )
    }
}

#[derive(Serialize)]
struct Risposta {
    #[serde(skip_serializing_if = "Option::is_none")]
    misura: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    testo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dentro: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    voci: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    totale: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    errore: Option<String>,
}

fn vuota() -> Risposta {
    Risposta {
        misura: None,
        testo: None,
        dentro: None,
        voci: None,
        totale: None,
        errore: None,
    }
}

fn main() {
    let mut dentro_tutto = String::new();
    if std::io::Read::read_to_string(&mut std::io::stdin(), &mut dentro_tutto).is_err() {
        eprintln!("non riesco a leggere stdin");
        std::process::exit(1);
    }
    for riga in dentro_tutto.lines() {
        let riga = riga.trim();
        if riga.is_empty() {
            continue;
        }
        let r = match serde_json::from_str::<Domanda>(riga) {
            Err(e) => Risposta { errore: Some(format!("domanda illeggibile: {e}")), ..vuota() },
            Ok(Domanda::Pesa { byte }) => Risposta { misura: Some(pesa(byte)), ..vuota() },
            Ok(Domanda::Dentro { quale, cartella }) => Risposta {
                dentro: Some(nova_dati::sta_dentro(
                    std::path::Path::new(&quale),
                    std::path::Path::new(&cartella),
                )),
                ..vuota()
            },
            Ok(Domanda::Racconta { posti, base, solo_esistenti }) => {
                let p: Vec<(Posto, Misura)> = posti.iter().map(VocePosto::in_due).collect();
                Risposta {
                    testo: Some(racconta(&p, std::path::Path::new(&base), solo_esistenti)),
                    ..vuota()
                }
            }
            Ok(Domanda::Rendiconto { posti, base }) => {
                let p: Vec<(Posto, Misura)> = posti.iter().map(VocePosto::in_due).collect();
                let (voci, totale) = rendiconto(&p, std::path::Path::new(&base));
                Risposta {
                    voci: Some(
                        voci.iter()
                            .map(|v| {
                                serde_json::json!({
                                    "che_cos_e": v.che_cos_e,
                                    "dove": v.dove,
                                    "byte": v.byte,
                                    "misura": v.misura,
                                    "delicato": v.delicato,
                                    "va_via_con_la_cartella": v.va_via_con_la_cartella,
                                    "se_lo_cancelli": v.se_lo_cancelli,
                                })
                            })
                            .collect(),
                    ),
                    totale: Some(totale),
                    ..vuota()
                }
            }
        };
        println!("{}", serde_json::to_string(&r).unwrap());
    }
}
