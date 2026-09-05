//! Come si accende il modello locale, e cosa si fa quando non ci sta.
//!
//! Qui non si avvia nessun processo e non si legge nessun disco: c'e' la
//! **riga di comando** che llama-server riceve, la **scala di ripiego** dei
//! layer da mettere sulla scheda video, e il riconoscimento dell'unico errore
//! che vale la pena riprovare. Il processo lo avvia chi sa avviare processi.
//!
//! E' il pezzo dove le decisioni si vedono poco e costano molto: un flag in
//! meno vuol dire meta' della memoria sprecata, un flag in piu' vuol dire un
//! binario vecchio che non parte, e un numero di layer sbagliato vuol dire un
//! modello che gira dieci volte piu' piano senza dire niente.

use std::path::Path;

/// Come e' configurato il server locale.
///
/// I campi arrivano da fuori: qui non si legge nessun file di
/// configurazione.
#[derive(Debug, Clone, Default)]
pub struct Impostazioni {
    pub percorso_modello: String,
    pub host: String,
    pub porta: u16,
    pub contesto: i64,
    pub paralleli: i64,
    pub fili: i64,
    /// `f16` (il valore di fabbrica), `q8_0`, `q5_1`, `q4_0`.
    pub tipo_kv: String,
    pub argomenti_extra: Vec<String>,
}

/// La riga di comando con cui si avvia llama-server.
///
/// Due scelte che sembrano dettagli e non lo sono.
///
/// **La cache KV a 8 bit** e' meta' della memoria, e su una scheda dove il
/// modello non ci sta tutto quella meta' diventa layer sulla GPU. Ma non si
/// passa quando e' `f16`, che e' gia' il valore di fabbrica: un flag in meno
/// e' una cosa in meno che puo' non piacere a un binario vecchio.
///
/// **Il proiettore visivo** arriva da fuori e non si cerca qui, perche' la
/// stessa domanda — «questo modello vede?» — la fa anche chi decide *se
/// allegare una figura*. Se la rispondessero in due posti diversi, prima o
/// poi risponderebbero diverso, e la forma di quel disaccordo e' un'immagine
/// mandata a un modello cieco: 500, e il messaggio resta in conversazione a
/// far fallire anche tutti i turni dopo.
pub fn argomenti(
    binario: &Path,
    s: &Impostazioni,
    ngl: i64,
    proiettore: Option<&Path>,
) -> Vec<String> {
    let mut a: Vec<String> = vec![
        binario.display().to_string(),
        "-m".into(),
        s.percorso_modello.clone(),
        "--host".into(),
        s.host.clone(),
        "--port".into(),
        s.porta.to_string(),
        "-ngl".into(),
        ngl.to_string(),
        "-c".into(),
        s.contesto.to_string(),
        "-np".into(),
        s.paralleli.to_string(),
    ];
    if s.fili != 0 {
        a.push("-t".into());
        a.push(s.fili.to_string());
    }
    let tipo_kv = s.tipo_kv.trim();
    let tipo_kv = if tipo_kv.is_empty() { "f16" } else { tipo_kv };
    if tipo_kv != "f16" {
        a.push("-ctk".into());
        a.push(tipo_kv.to_string());
        a.push("-ctv".into());
        a.push(tipo_kv.to_string());
    }
    if let Some(p) = proiettore {
        a.push("--mmproj".into());
        a.push(p.display().to_string());
    }
    a.extend(s.argomenti_extra.iter().cloned());
    a
}

/// Di quanto si scende a ogni tentativo fallito per memoria.
pub const GRADINO: i64 = 6;

/// La scala dei tentativi: quanti layer mettere sulla scheda, in ordine.
///
/// Se l'utente ha scelto un numero suo, si prova quello e basta: non e'
/// compito di NOVA correggere una decisione presa a mano.
///
/// Altrimenti si parte da `stimato` e si scende di sei alla volta fino a
/// zero. **Da qui prima si partiva da 64 alla cieca**, e la scala di ripiego
/// non poteva correggere niente: scende a ogni errore di memoria, ma la
/// memoria condivisa **non da' errori** — accetta tutto e va dieci volte piu'
/// piano. Era un meccanismo di sicurezza che aspettava un'eccezione da
/// qualcosa che non ne solleva, cioe' nessun meccanismo di sicurezza.
///
/// `stimato` a zero vuol dire «sul processore»: lento di sicuro invece che
/// finto veloce, e lo si dice.
pub fn scala_dei_layer(base: i64, auto: bool, stimato: i64) -> Vec<i64> {
    if !auto {
        return vec![base];
    }
    // Un numero scelto a mano sotto 99 e' una scelta; 99 vuol dire «tutti»,
    // cioe' «decidi tu».
    let partenza = if base < 99 { base } else { stimato };
    let mut scala = vec![partenza];
    let mut ora = partenza;
    while ora > 0 {
        ora -= GRADINO;
        scala.push(std::cmp::max(ora, 0));
    }
    let mut visti = std::collections::BTreeSet::new();
    scala.into_iter().filter(|v| *v >= 0 && visti.insert(*v)).collect()
}

/// I modi in cui llama.cpp dice «non ci sta in memoria».
///
/// Sono sei perche' vengono da posti diversi — CUDA, Vulkan, l'allocatore di
/// ggml — e riconoscerne uno solo vorrebbe dire riprovare con meno layer in
/// un caso su sei e arrendersi negli altri cinque.
pub const SEGNI_DI_MEMORIA_FINITA: [&str; 6] = [
    "out of memory",
    "failed to allocate",
    "cudamalloc failed",
    "erroroutofdevicememory",
    "unable to allocate backend buffer",
    "insufficient memory",
];

/// Se questo pezzo di registro giustifica un altro tentativo con meno layer.
pub fn e_memoria_finita(testo: &str) -> bool {
    let t = testo.to_lowercase();
    SEGNI_DI_MEMORIA_FINITA.iter().any(|s| t.contains(s))
}

/// Il proiettore multimodale fra i nomi di file di una cartella.
///
/// I repository lo mettono accanto al GGUF e lo chiamano `mmproj-F16.gguf` o
/// giu' di li'. Chi non ce l'ha non e' un modello rotto: e' un modello che
/// **non vede**, il che va benissimo finche' nessuno gli manda una figura
/// fingendo che la guardi.
///
/// I nomi arrivano da fuori — il disco lo legge chi legge il disco — e si
/// confrontano **senza guardare le maiuscole**, perche' e' quello che fa
/// `Path.glob` su Windows, che e' dove NOVA gira.
pub fn proiettore(nomi: &[String]) -> Option<String> {
    let mut buoni: Vec<&String> = nomi
        .iter()
        .filter(|n| {
            let m = n.to_lowercase();
            m.contains("mmproj") && m.ends_with(".gguf")
        })
        .collect();
    // `sorted()` su percorsi Windows confronta le versioni minuscole.
    buoni.sort_by_key(|n| n.to_lowercase());
    buoni.first().map(|n| (*n).clone())
}

#[cfg(test)]
mod prove {
    use super::*;
    use std::path::PathBuf;

    fn base() -> Impostazioni {
        Impostazioni {
            percorso_modello: r"C:\m\modello.gguf".into(),
            host: "127.0.0.1".into(),
            porta: 8080,
            contesto: 16384,
            paralleli: 1,
            fili: 0,
            tipo_kv: "f16".into(),
            argomenti_extra: vec![],
        }
    }

    #[test]
    fn la_riga_minima() {
        let a = argomenti(&PathBuf::from("llama-server.exe"), &base(), 33, None);
        assert_eq!(
            a,
            vec!["llama-server.exe", "-m", r"C:\m\modello.gguf", "--host",
                 "127.0.0.1", "--port", "8080", "-ngl", "33", "-c", "16384",
                 "-np", "1"]
        );
    }

    #[test]
    fn la_cache_a_otto_bit_si_passa_solo_se_serve() {
        let mut s = base();
        s.tipo_kv = "q8_0".into();
        let a = argomenti(&PathBuf::from("x"), &s, 1, None);
        assert!(a.windows(2).any(|w| w == ["-ctk", "q8_0"]));
        assert!(a.windows(2).any(|w| w == ["-ctv", "q8_0"]));
        // f16 e' il valore di fabbrica: un flag in meno.
        assert!(!argomenti(&PathBuf::from("x"), &base(), 1, None)
            .iter()
            .any(|x| x == "-ctk"));
        // E vuoto vale f16.
        let mut vuoto = base();
        vuoto.tipo_kv = "  ".into();
        assert!(!argomenti(&PathBuf::from("x"), &vuoto, 1, None)
            .iter()
            .any(|x| x == "-ctk"));
    }

    #[test]
    fn il_proiettore_entra_prima_degli_extra() {
        let mut s = base();
        s.argomenti_extra = vec!["--flash-attn".into()];
        let a = argomenti(&PathBuf::from("x"), &s, 1, Some(Path::new(r"C:\m\mmproj.gguf")));
        let i = a.iter().position(|x| x == "--mmproj").unwrap();
        let j = a.iter().position(|x| x == "--flash-attn").unwrap();
        assert!(i < j, "{a:?}");
    }

    #[test]
    fn i_fili_solo_se_qualcuno_li_ha_chiesti() {
        let mut s = base();
        s.fili = 8;
        assert!(argomenti(&PathBuf::from("x"), &s, 1, None)
            .windows(2)
            .any(|w| w == ["-t", "8"]));
        assert!(!argomenti(&PathBuf::from("x"), &base(), 1, None)
            .iter()
            .any(|x| x == "-t"));
    }

    #[test]
    fn la_scala_scende_di_sei_e_finisce_a_zero() {
        assert_eq!(scala_dei_layer(99, true, 33), vec![33, 27, 21, 15, 9, 3, 0]);
        assert_eq!(scala_dei_layer(99, true, 6), vec![6, 0]);
        assert_eq!(scala_dei_layer(99, true, 0), vec![0]);
    }

    #[test]
    fn una_scelta_a_mano_si_rispetta() {
        assert_eq!(scala_dei_layer(20, false, 33), vec![20]);
        // Sotto 99 e' una scelta: si parte da li' e si scende comunque.
        assert_eq!(scala_dei_layer(12, true, 33), vec![12, 6, 0]);
    }

    #[test]
    fn la_memoria_finita_si_riconosce_da_sei_parti() {
        assert!(e_memoria_finita("ggml_backend_cuda_buffer_type_alloc: cudaMalloc failed"));
        assert!(e_memoria_finita("vk::Result::eErrorOutOfDeviceMemory"));
        assert!(e_memoria_finita("llama_init: unable to allocate backend buffer"));
        assert!(e_memoria_finita("Insufficient Memory"));
        assert!(!e_memoria_finita("modello caricato, tutto bene"));
    }

    #[test]
    fn e_due_forme_che_oggi_NON_si_riconoscono() {
        // Non e' un difetto di questa traduzione: e' cosa fa il Python, e
        // portarlo diverso vorrebbe dire che il banco non confronta piu'
        // niente (D153). Ma vale la pena averle scritte, perche' sono le due
        // in cui NOVA **non** riprova con meno layer e si arrende:
        //
        // - la costante Vulkan con i trattini bassi, che e' come la scrivono
        //   i livelli di validazione (dentro llama.cpp arriva nella forma di
        //   `vulkan.hpp`, che invece si riconosce);
        // - il messaggio di ggml quando l'allocazione fallisce senza usare
        //   nessuna delle sei parole.
        assert!(!e_memoria_finita("VK_ERROR_OUT_OF_DEVICE_MEMORY"));
        assert!(!e_memoria_finita(
            "ggml_vulkan: Device memory allocation of 123 bytes failed"));
    }

    #[test]
    fn il_proiettore_si_riconosce_senza_guardare_le_maiuscole() {
        let nomi: Vec<String> = ["modello.gguf", "MMPROJ-F16.GGUF", "note.txt"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(proiettore(&nomi), Some("MMPROJ-F16.GGUF".to_string()));
        assert_eq!(proiettore(&["modello.gguf".to_string()]), None);
        assert_eq!(proiettore(&[]), None);
    }

    #[test]
    fn fra_due_proiettori_vince_il_primo_in_ordine() {
        let nomi: Vec<String> = ["mmproj-Q8.gguf", "mmproj-F16.gguf"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(proiettore(&nomi), Some("mmproj-F16.gguf".to_string()));
    }
}
