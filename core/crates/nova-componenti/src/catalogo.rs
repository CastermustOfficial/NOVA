//! Il catalogo: cosa NOVA sa fare in piu' se ha i suoi pezzi.
//!
//! I percorsi non sono qui dentro murati: si compongono su due cartelle che
//! passa chi chiama, come in `nova_dati`. Cosi' il catalogo dice **cosa**
//! serve, e dove vada a finire lo sa chi ci scrive.
//!
//! Il gemello e' `catalogo()` in `nova/componenti.py`, e un banco li confronta
//! voce per voce. Non e' generato da un estrattore come le guardie, e vale la
//! pena dire perche': le guardie sono liste di stringhe, questo e' un albero
//! con dentro percorsi che le due parti compongono in modo diverso. Un
//! estrattore avrebbe dovuto sapere come si compongono, cioe' sapere i
//! percorsi — che e' esattamente cio' che non deve fare.

use crate::{Componente, Pezzo, Tipo};
use std::path::Path;

const KOKORO: &str =
    "https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0";
const ONNX: &str = "https://github.com/microsoft/onnxruntime/releases/download/v1.20.1/\
onnxruntime-win-x64-1.20.1.zip";
const WHISPER: &str = "https://github.com/ggml-org/whisper.cpp/releases/download/v1.8.4/\
whisper-cublas-12.4.0-bin-x64.zip";
const GGML: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/";

/// Il catalogo, composto sulle cartelle di questa installazione.
///
/// `radice` e' la cartella del progetto (serve per la copia di `vocab.json`,
/// che sta gia' dentro e non si scarica), `runtime` quella dove finiscono i
/// pezzi.
pub fn catalogo(radice: &Path, runtime: &Path) -> Vec<Componente> {
    let voce = runtime.join("voce");
    let ascolto = runtime.join("ascolto");
    vec![
        Componente {
            nome: "voce_locale".into(),
            titolo: "Voce locale (Kokoro)".into(),
            serve_a: "Far parlare NOVA senza che l'audio esca dal PC".into(),
            senza: "NOVA scrive ma non parla, a meno di usare ElevenLabs o la voce di Windows"
                .into(),
            mb: 350,
            licenza: String::new(),
            pezzi: vec![
                Pezzo {
                    tipo: Tipo::File { url: format!("{KOKORO}/kokoro-v1.0.onnx") },
                    dove: voce.join("kokoro-v1.0.onnx"),
                    prova: None,
                    vale_anche: vec![],
                },
                Pezzo {
                    tipo: Tipo::File { url: format!("{KOKORO}/voices-v1.0.bin") },
                    dove: voce.join("voices-v1.0.bin"),
                    prova: None,
                    vale_anche: vec![],
                },
                Pezzo {
                    tipo: Tipo::Copia {
                        da: radice.join("core/crates/nova-voce/src/vocab.json"),
                    },
                    dove: voce.join("vocab.json"),
                    prova: None,
                    vale_anche: vec![],
                },
            ],
        },
        Componente {
            nome: "onnx".into(),
            titolo: "ONNX Runtime".into(),
            serve_a: "Eseguire Kokoro: il crate lo carica a runtime, non e' collegato dentro"
                .into(),
            senza: "La voce locale non parte nemmeno con i suoi modelli al posto giusto".into(),
            mb: 60,
            licenza: String::new(),
            pezzi: vec![Pezzo {
                tipo: Tipo::ZipDll { url: ONNX.into(), filtro: "onnxruntime".into() },
                dove: voce.clone(),
                prova: Some(voce.join("onnxruntime.dll")),
                vale_anche: vec![],
            }],
        },
        Componente {
            nome: "espeak".into(),
            titolo: "espeak-ng (fonemi)".into(),
            serve_a: "Trasformare il testo in fonemi: senza, Kokoro non sa cosa pronunciare"
                .into(),
            senza: "NOVA capisce quello che dici ma non risponde a voce".into(),
            mb: 12,
            licenza: "GPLv3 - non ridistribuito, si scarica dalla fonte ufficiale".into(),
            pezzi: vec![Pezzo {
                tipo: Tipo::MsiEspeak,
                dove: voce.clone(),
                prova: Some(voce.join("espeak-ng.dll")),
                vale_anche: vec![],
            }],
        },
        Componente {
            nome: "ascolto_locale".into(),
            titolo: "Ascolto locale (whisper.cpp)".into(),
            serve_a: "Trascrivere quello che dici senza mandare l'audio a nessuno".into(),
            senza: "L'ascolto passa da ElevenLabs, quindi la tua voce esce dal PC".into(),
            mb: 420,
            licenza: String::new(),
            pezzi: vec![
                Pezzo {
                    tipo: Tipo::ZipPiatto { url: WHISPER.into() },
                    dove: ascolto.clone(),
                    prova: Some(ascolto.join("whisper-cli.exe")),
                    vale_anche: vec![],
                },
                Pezzo {
                    tipo: Tipo::File { url: format!("{GGML}ggml-base.bin") },
                    dove: ascolto.join("ggml-base.bin"),
                    prova: None,
                    // Il demone accetta small, base, medium o tiny, in
                    // quest'ordine: chi ne ha gia' uno non deve scaricarne un
                    // altro solo perche' il catalogo ne nomina uno diverso.
                    vale_anche: vec![
                        "ggml-small.bin".into(),
                        "ggml-medium.bin".into(),
                        "ggml-tiny.bin".into(),
                    ],
                },
            ],
        },
    ]
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn ogni_componente_dice_cosa_succede_senza() {
        // E' il campo che decide se uno scarica o no. Uno vuoto vuol dire un
        // pannello che dice «manca» e non dice cosa cambia.
        for c in catalogo(Path::new("/progetto"), Path::new("/progetto/runtime")) {
            assert!(!c.senza.trim().is_empty(), "{} non dice cosa succede senza", c.nome);
            assert!(!c.serve_a.trim().is_empty(), "{} non dice a cosa serve", c.nome);
            assert!(c.mb > 0, "{} non dice quanto pesa", c.nome);
            assert!(!c.pezzi.is_empty(), "{} non ha pezzi", c.nome);
        }
    }

    #[test]
    fn i_nomi_sono_tutti_diversi() {
        let c = catalogo(Path::new("/p"), Path::new("/p/runtime"));
        let mut nomi: Vec<&str> = c.iter().map(|x| x.nome.as_str()).collect();
        nomi.sort_unstable();
        let quanti = nomi.len();
        nomi.dedup();
        assert_eq!(nomi.len(), quanti, "due componenti con lo stesso nome");
    }

    #[test]
    fn chi_e_di_un_altro_dice_di_chi_e() {
        // espeak-ng e' GPLv3 e non viaggia dentro un progetto MIT.
        let c = catalogo(Path::new("/p"), Path::new("/p/runtime"));
        let e = c.iter().find(|x| x.nome == "espeak").unwrap();
        assert!(e.licenza.contains("GPLv3"), "{}", e.licenza);
        assert!(e.licenza.contains("non ridistribuito"), "{}", e.licenza);
    }

    #[test]
    fn gli_zip_hanno_una_prova_che_non_e_la_cartella() {
        // Per uno zip `dove` e' una cartella, e una cartella c'e' sempre.
        for c in catalogo(Path::new("/p"), Path::new("/p/runtime")) {
            for p in &c.pezzi {
                if matches!(p.tipo, Tipo::ZipDll { .. } | Tipo::ZipPiatto { .. } | Tipo::MsiEspeak)
                {
                    assert!(
                        p.prova.is_some(),
                        "{}: un pezzo da estrarre senza prova si dice sempre presente",
                        c.nome
                    );
                }
            }
        }
    }

    #[test]
    fn i_percorsi_si_compongono_su_quel_che_passa_chi_chiama() {
        let c = catalogo(Path::new("/progetto"), Path::new("/altrove/runtime"));
        let voce = &c[0];
        assert!(voce.pezzi[0].dove.starts_with("/altrove/runtime/voce"));
        // E la copia viene da dentro il progetto, non dal runtime.
        assert!(matches!(&voce.pezzi[2].tipo, Tipo::Copia { da } if da.starts_with("/progetto")));
    }
}
