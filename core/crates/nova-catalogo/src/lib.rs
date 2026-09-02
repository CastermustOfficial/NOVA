//! Quale modello ha senso su questa macchina, e quale non si scarica affatto.
//!
//! Porta in Rust `nova/catalogo.py`. E' il pezzo con il criterio d'ordine piu'
//! chiaro di tutto il cantiere: lo chiama **l'installatore**, e l'installatore
//! gira prima che le dipendenze del progetto esistano. In Python era un
//! modulo di sola libreria standard proprio per questo; in Rust il problema
//! sparisce, perche' e' un binario e non ha dipendenze da installare.
//!
//! ## Il numero che decide
//!
//! Non i gigabyte del file, e nemmeno i parametri totali. Generare un token,
//! a una richiesta per volta, non e' un lavoro di calcolo: e' **leggere i
//! pesi dalla memoria**. La velocita' e' `banda / byte letti per token`, e i
//! byte letti sono i parametri che si **accendono** per quanti bit ciascuno
//! occupa — quindi la quantizzazione conta quanto l'architettura, e un denso
//! a un bit puo' finire nella stessa categoria di un MoE a tre.
//!
//! Le due misure da cui esce la soglia, stessa macchina, zero layer su GPU:
//!
//! ```text
//! Qwen3.8 27B Q4_K_M (denso)     15,7 GB/token   1,8 tok/s
//! Gemma 4 26B-A4B Q3 (MoE)       ~3,7 GB/token   7,6 tok/s
//! ```
//!
//! La stessa banda implicita (~28 GB/s) spiega tutte e due le righe: il
//! modello di costo e' quello giusto, non una spiegazione costruita su un
//! numero solo.
//!
//! ## Perche' esiste
//!
//! **La forma giusta di dire «non farlo» e' non offrirlo.** L'installatore,
//! quando la VRAM non si leggeva, scaricava comunque la variante piu' leggera
//! e ci scriveva accanto «andra' piano». Per un denso da 27B non e' «piu'
//! piano»: sono tredici gigabyte per un programma che si apre una volta e mai
//! piu'. Un avvertimento piu' grosso non ripara niente — chi installa clicca
//! avanti, e ha ragione, perche' gli abbiamo appena detto che si puo' fare.
//!
//! Ma «senza GPU mai» sarebbe sbagliato quanto il contrario: un modello
//! leggero da leggere va benissimo sul processore, e sette token al secondo
//! sono piu' veloci di quanto legga una persona. Quindi non si guarda se c'e'
//! una scheda video: si guarda il numero.

use serde::{Deserialize, Serialize};

/// Sotto questa soglia si sta sopra i sette token al secondo su una macchina
/// come quella su cui e' stata misurata (banda ~28 GB/s, DDR4 a due canali).
/// E' una stima onesta, non una legge: sta in un posto solo perche' quando
/// arriveranno altre misure si cambi qui — o meglio, in `models.json`.
pub const SOGLIA_GB_PER_TOKEN: f64 = 5.0;

/// Quanto del file si rilegge a ogni token quando il modello e' denso: tutto.
pub const DENSO: f64 = 1.0;

/// Quanti gigabyte lasciare al sistema quando il modello gira sul processore.
///
/// Sul processore il file non sta in VRAM: sta in RAM, tutto, e accanto ci
/// devono stare Windows, il browser e NOVA stessa. Senza questo margine un
/// modello «abbastanza veloce» entra sulla carta e in pratica manda la
/// macchina a paginare su disco — un altro modo di essere lentissimi, e
/// stavolta con la ventola accesa.
pub const MARGINE_RAM_GB: f64 = 4.0;

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Variante {
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub gb: f64,
    #[serde(default)]
    pub vram_gb: f64,
    #[serde(default)]
    pub qualita: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Famiglia {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub nome: String,
    /// La parte del file che si rilegge a ogni token. E' un dato della
    /// **famiglia**, non della variante: dipende dall'architettura, non da
    /// quanto si e' compressa.
    #[serde(default)]
    pub frazione_letta: Option<f64>,
    #[serde(default)]
    pub varianti: Vec<Variante>,
}

/// Chi non dichiara la frazione e' trattato come denso — il caso peggiore, ed
/// e' la direzione giusta in cui sbagliare.
pub fn frazione_letta(f: &Famiglia) -> f64 {
    match f.frazione_letta {
        Some(v) if v > 0.0 && v <= 1.0 => v,
        _ => DENSO,
    }
}

/// Arrotonda a due decimali come `round(x, 2)` di Python: a meta' si va al
/// pari, non sempre in su. Non e' pedanteria — e' l'unico modo perche' il
/// numero che finisce nella frase mostrata all'utente sia lo stesso nelle due
/// implementazioni, e il banco lo confronta cifra per cifra.
fn arrotonda2(x: f64) -> f64 {
    let scalato = x * 100.0;
    let giu = scalato.floor();
    let resto = scalato - giu;
    let n = if (resto - 0.5).abs() < f64::EPSILON * scalato.abs().max(1.0) {
        // esattamente a meta': si sceglie il pari
        if (giu as i64) % 2 == 0 { giu } else { giu + 1.0 }
    } else if resto > 0.5 {
        giu + 1.0
    } else {
        giu
    };
    n / 100.0
}

/// I gigabyte che si leggono per produrre un token.
pub fn gb_per_token(f: &Famiglia, v: &Variante) -> f64 {
    arrotonda2(v.gb * frazione_letta(f))
}

/// La soglia, dal catalogo se c'e' scritta.
///
/// Sta in `models.json` per la stessa ragione per cui ci stanno i modelli: non
/// e' codice, e' un dato. Quando arriveranno misure da altre macchine si
/// cambia il file, non si fa una release.
pub fn soglia(catalogo: &serde_json::Value) -> f64 {
    catalogo
        .get("regole_di_adattamento")
        .and_then(|r| r.get("soglia_gb_per_token"))
        .and_then(|v| v.as_f64())
        .filter(|v| *v > 0.0)
        .unwrap_or(SOGLIA_GB_PER_TOKEN)
}

/// La variante piu' grande che sta in VRAM, o nessuna.
///
/// Si sceglie la piu' grande che **entra**, non la piu' grande che si riesce a
/// caricare: se non ci sta tutta, llama.cpp mette una parte dei layer in RAM e
/// funziona lo stesso, dieci volte piu' piano, senza dire niente.
pub fn piu_grande_che_entra<'a>(f: &'a Famiglia, vram_gb: f64) -> Option<&'a Variante> {
    if vram_gb == 0.0 {
        return None;
    }
    f.varianti
        .iter()
        .filter(|v| v.vram_gb <= vram_gb)
        .fold(None, |scelta: Option<&Variante>, v| match scelta {
            Some(s) if s.gb >= v.gb => Some(s),
            _ => Some(v),
        })
}

/// Le varianti utilizzabili sul solo processore, dalla piu' grande.
///
/// Due vincoli, non uno: la **velocita'** (i byte letti per token) e lo
/// **spazio** (il file intero, che sul processore sta in RAM insieme a tutto
/// il resto). `ram_gb` a zero vuol dire «non lo so»: si guarda solo la
/// velocita'.
pub fn varianti_senza_gpu<'a>(
    f: &'a Famiglia,
    soglia_gb: Option<f64>,
    ram_gb: f64,
) -> Vec<&'a Variante> {
    let s = soglia_gb.unwrap_or(SOGLIA_GB_PER_TOKEN);
    let mut ok: Vec<&Variante> = f
        .varianti
        .iter()
        .filter(|v| gb_per_token(f, v) <= s)
        .filter(|v| ram_gb == 0.0 || v.gb <= ram_gb - MARGINE_RAM_GB)
        .collect();
    // Dalla piu' grande. `sort_by` e' stabile, come `sorted` di Python: a
    // parita' di peso l'ordine del catalogo si conserva, e con esso quale
    // variante viene proposta.
    ok.sort_by(|a, b| b.gb.partial_cmp(&a.gb).unwrap_or(std::cmp::Ordering::Equal));
    ok
}

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct Verdetto {
    pub si_scarica: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gb: Option<f64>,
    pub motivo: String,
    pub suggerimento: String,
    /// `true` quando il modello girera' sul processore: va detto **prima**.
    pub in_cpu: bool,
}

/// Se offrire lo scaricamento, quale variante, e cosa dire.
pub fn verdetto(f: &Famiglia, vram_gb: f64, soglia_gb: Option<f64>, ram_gb: f64) -> Verdetto {
    if let Some(v) = piu_grande_che_entra(f, vram_gb) {
        return Verdetto {
            si_scarica: true,
            file: Some(v.file.clone()),
            gb: Some(v.gb),
            ..Default::default()
        };
    }

    // Niente scheda video utilizzabile, o niente che ci stia dentro: decide il
    // numero, non la presenza della GPU.
    let leggere = varianti_senza_gpu(f, soglia_gb, ram_gb);
    if let Some(v) = leggere.first() {
        return Verdetto {
            si_scarica: true,
            file: Some(v.file.clone()),
            gb: Some(v.gb),
            in_cpu: true,
            motivo: format!(
                "Girera' sul processore: legge circa {:.1} GB per token, \
                 quindi qualche token al secondo. Si usa, ma non e' veloce.",
                gb_per_token(f, v)
            ),
            suggerimento: String::new(),
        };
    }

    let minimo = f
        .varianti
        .iter()
        .map(|v| gb_per_token(f, v))
        .fold(f64::INFINITY, f64::min);
    let minimo = if minimo.is_finite() { minimo } else { 0.0 };
    let nome = if !f.nome.is_empty() {
        f.nome.clone()
    } else if !f.id.is_empty() {
        f.id.clone()
    } else {
        "questo modello".to_string()
    };

    // Due rifiuti diversi meritano due frasi diverse: chi ha poca RAM puo'
    // comprarne, chi ha un modello troppo denso no.
    if minimo <= soglia_gb.unwrap_or(SOGLIA_GB_PER_TOKEN) && ram_gb != 0.0 {
        let piccolo = f
            .varianti
            .iter()
            .map(|v| v.gb)
            .fold(f64::INFINITY, f64::min);
        let piccolo = if piccolo.is_finite() { piccolo } else { 0.0 };
        return Verdetto {
            si_scarica: false,
            motivo: format!(
                "{nome} sarebbe abbastanza veloce sul processore, ma il file \
                 piu' piccolo pesa {piccolo:.0} GB e sul processore il modello \
                 sta in RAM, tutto: con {ram_gb:.0} GB non ci sta insieme al \
                 resto. Non te lo faccio scaricare."
            ),
            suggerimento: "Serve una variante piu' compressa della stessa \
                           famiglia, oppure un abbonamento che hai gia' o una \
                           chiave API: NOVA funziona lo stesso."
                .to_string(),
            ..Default::default()
        };
    }

    Verdetto {
        si_scarica: false,
        motivo: format!(
            "{nome} legge almeno {minimo:.1} GB per ogni token che scrive, e \
             senza scheda video quella lettura la fa la memoria di sistema: \
             verrebbe meno di un token al secondo. Non te lo faccio scaricare: \
             sono gigabyte per un programma che poi non apriresti."
        ),
        suggerimento: "Puoi provare piu' avanti, dalle impostazioni, con un \
                       modello che di parametri ne accende pochi - quelli \
                       vanno bene anche senza scheda video. Nel frattempo NOVA \
                       funziona con un abbonamento che hai gia' o con una \
                       chiave API."
            .to_string(),
        ..Default::default()
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    fn v(file: &str, gb: f64, vram_gb: f64) -> Variante {
        Variante { file: file.into(), gb, vram_gb, qualita: String::new() }
    }

    fn denso() -> Famiglia {
        Famiglia {
            id: "qwen".into(),
            nome: "Qwen3.8 27B".into(),
            frazione_letta: Some(1.0),
            varianti: vec![v("grande.gguf", 25.3, 28.0), v("medio.gguf", 15.7, 18.0)],
        }
    }

    fn moe() -> Famiglia {
        Famiglia {
            id: "gemma".into(),
            nome: "Gemma 4 26B-A4B".into(),
            frazione_letta: Some(0.15),
            varianti: vec![v("g-q4.gguf", 16.0, 18.0), v("g-q3.gguf", 10.6, 12.0)],
        }
    }

    #[test]
    fn con_la_vram_si_prende_la_piu_grande_che_entra() {
        let f = denso();
        assert_eq!(piu_grande_che_entra(&f, 28.0).unwrap().file, "grande.gguf");
        assert_eq!(piu_grande_che_entra(&f, 18.0).unwrap().file, "medio.gguf");
        assert!(piu_grande_che_entra(&f, 8.0).is_none(), "niente ci sta");
        assert!(piu_grande_che_entra(&f, 0.0).is_none(), "zero = non lo so");
    }

    #[test]
    fn un_denso_da_27b_non_si_offre_senza_scheda() {
        // E' il caso per cui questo modulo esiste: 15,7 GB per token.
        let d = verdetto(&denso(), 0.0, None, 32.0);
        assert!(!d.si_scarica, "{}", d.motivo);
        assert!(d.motivo.contains("legge almeno"), "{}", d.motivo);
    }

    #[test]
    fn un_moe_leggero_si_offre_anche_senza_scheda() {
        let d = verdetto(&moe(), 0.0, None, 32.0);
        assert!(d.si_scarica, "{}", d.motivo);
        assert!(d.in_cpu, "e va detto prima");
        assert_eq!(d.file.as_deref(), Some("g-q4.gguf"), "la piu' grande che passa");
    }

    #[test]
    fn abbastanza_veloce_ma_troppo_grande_e_un_rifiuto_diverso() {
        // 16 GB di file con 12 GB di RAM: veloce da leggere, ma non ci sta.
        let d = verdetto(&moe(), 0.0, None, 12.0);
        assert!(!d.si_scarica);
        assert!(d.motivo.contains("sarebbe abbastanza veloce"), "{}", d.motivo);
        assert!(d.motivo.contains("non ci sta insieme al resto"), "{}", d.motivo);
    }

    #[test]
    fn ram_a_zero_vuol_dire_non_lo_so_e_si_guarda_solo_la_velocita() {
        // La famiglia si lega a una variabile: `varianti_senza_gpu` torna
        // riferimenti dentro di lei, e passarle un temporaneo li farebbe
        // morire prima di essere letti.
        let f = moe();
        let senza = varianti_senza_gpu(&f, None, 0.0);
        assert_eq!(senza.len(), 2, "nessun vincolo di spazio");
        // Con 12 GB di RAM il tetto e' 12 - 4 = 8, e **nessuna** delle due
        // varianti (16,0 e 10,6 GB) ci sta. Qui la prima versione di questa
        // prova diceva 1: avevo guardato solo la variante grande e dato per
        // scontato che la piccola passasse. A sbagliare era l'attesa.
        assert_eq!(varianti_senza_gpu(&f, None, 12.0).len(), 0);
        // Con 16 il tetto e' 12: passa la piccola e non la grande. E' il
        // valore che fa vedere il confine, invece di scartare tutto.
        let al_confine = varianti_senza_gpu(&f, None, 16.0);
        assert_eq!(al_confine.len(), 1, "10,6 ci sta, 16,0 no");
        assert_eq!(al_confine[0].file, "g-q3.gguf");
    }

    #[test]
    fn chi_non_dichiara_la_frazione_e_trattato_come_denso() {
        let mut f = moe();
        f.frazione_letta = None;
        assert_eq!(frazione_letta(&f), DENSO);
        // e cosi' non si offre piu' senza scheda: e' il verso giusto in cui
        // sbagliare quando un dato manca.
        assert!(!verdetto(&f, 0.0, None, 64.0).si_scarica);
    }

    #[test]
    fn una_frazione_assurda_non_viene_creduta() {
        for cattiva in [0.0, -1.0, 1.5, 99.0] {
            let mut f = moe();
            f.frazione_letta = Some(cattiva);
            assert_eq!(frazione_letta(&f), DENSO, "frazione {cattiva}");
        }
    }

    #[test]
    fn la_soglia_si_legge_dal_catalogo_se_ce() {
        let c: serde_json::Value =
            serde_json::from_str(r#"{"regole_di_adattamento":{"soglia_gb_per_token":3.0}}"#)
                .unwrap();
        assert_eq!(soglia(&c), 3.0);
        // e i valori che non hanno senso non si credono
        let brutto: serde_json::Value =
            serde_json::from_str(r#"{"regole_di_adattamento":{"soglia_gb_per_token":0}}"#).unwrap();
        assert_eq!(soglia(&brutto), SOGLIA_GB_PER_TOKEN);
        assert_eq!(soglia(&serde_json::json!({})), SOGLIA_GB_PER_TOKEN);
    }

    #[test]
    fn la_vram_vince_sempre_sul_processore() {
        // Con una scheda capiente non si finisce mai nel ramo «in_cpu», nemmeno
        // per un denso: e' la GPU a leggere, e legge in fretta.
        let d = verdetto(&denso(), 28.0, None, 8.0);
        assert!(d.si_scarica && !d.in_cpu);
        assert_eq!(d.file.as_deref(), Some("grande.gguf"));
    }
}
