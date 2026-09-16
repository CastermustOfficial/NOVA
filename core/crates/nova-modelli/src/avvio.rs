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

// ---------------------------------------------------------------- il giro
//
// Fin qui c'e' *come* si accende il modello. Questa parte e' *cosa si fa
// quando non si accende*, ed era l'unico pezzo del giro che non stava da
// nessuna parte: viveva dentro due `if` di `runtime.py`, scritti in due
// posti, e uno dei due diceva una cosa falsa.
//
//     if not OOM(err) and not auto_tune: break
//
// Letto con calma: si esce dal giro solo se **tutte e due** sono vere. Con
// `auto_tune` acceso — che e' il valore di fabbrica — un errore che non
// c'entra niente con la memoria (un flag rifiutato, un GGUF rotto, la porta
// occupata) non ferma niente: si riprova tutta la scala, e a ogni gradino
// NOVA scrive «Memoria insufficiente: riprovo con meno layer». Sei volte una
// frase che non e' stata verificata nemmeno una.
//
// Dire la causa sbagliata con sicurezza e' peggio che non dirla: manda a
// cercare dove non c'e' niente, e la volta dopo quel messaggio non lo legge
// piu' nessuno.

/// Com'e' finito un tentativo di accendere il modello.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Esito {
    /// Ha risposto sulla sua porta: e' acceso.
    Pronto,
    /// Il processo e' uscito da solo, prima di rispondere.
    Morto,
    /// E' ancora vivo, ma non ha risposto entro l'attesa.
    Scaduto,
}

/// Cosa si fa dopo un tentativo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Prossimo {
    Acceso,
    /// Si scende di un gradino. Dentro, il motivo **vero**.
    Riprova(String),
    /// Non si riprova. Dentro, cosa si dice a chi guarda.
    Arrenditi(String),
}

/// La riga con cui il processo ha detto di avere un problema, se c'e'.
///
/// Una parola sola, `error`, e dichiarata come tale: **non** un elenco di
/// frasi indovinate. Dei sei segni di memoria finita qui sopra si sa da dove
/// vengono; di un catalogo di «cause note» si saprebbe solo che sembrava
/// ragionevole, e sarebbe la terza volta in questo progetto che un elenco
/// immaginato copre il caso che capita davvero.
///
/// Se un giorno llama.cpp dicesse il motivo senza scrivere `error`, NOVA lo
/// tratterebbe come una morte muta e riproverebbe con meno layer: un
/// tentativo in piu', non una diagnosi falsa. Dei due modi di sbagliare e'
/// quello che costa meno.
pub fn ha_parlato(coda: &str) -> Option<String> {
    coda.lines()
        .rev()
        .map(str::trim)
        .find(|r| !r.is_empty() && r.to_lowercase().contains("error"))
        .map(|r| r.to_string())
}

/// Cosa fare dopo un tentativo, e **perche'**.
///
/// La regola, in una riga: la scala cura la memoria, quindi si scende solo
/// quando la memoria c'entra — o quando il processo e' morto senza dire
/// niente, che e' esattamente cio' che fa una memoria video che finisce di
/// colpo: il sistema chiude il processo e non gli lascia il tempo di
/// scrivere.
///
/// `Scaduto` senza una parola vale come una morte muta, e non per simmetria:
/// e' il caso della memoria **condivisa**, che non da' nessun errore, accetta
/// tutto e va dieci volte piu' piano. Li' l'unico rimedio e' meno layer.
pub fn dopo_un_tentativo(
    esito: Esito,
    coda: &str,
    auto: bool,
    altri_gradini: bool,
) -> Prossimo {
    if esito == Esito::Pronto {
        return Prossimo::Acceso;
    }
    let detto = ha_parlato(coda);
    let memoria = e_memoria_finita(coda);

    let motivo = match (&detto, esito) {
        (Some(r), _) => accorcia(r),
        (None, Esito::Morto) => "e' uscito subito senza dire perche'".to_string(),
        (None, _) => "non ha risposto entro l'attesa".to_string(),
    };

    if !auto {
        return Prossimo::Arrenditi(motivo);
    }
    if memoria {
        return Prossimo::Riprova("la memoria video non basta".to_string());
    }
    if detto.is_some() {
        // Ha parlato, e non ha parlato di memoria: scendere di un gradino non
        // cura niente. Si dice cosa ha detto, subito, invece di far passare
        // sei tentativi prima di arrivarci.
        return Prossimo::Arrenditi(motivo);
    }
    if !altri_gradini {
        return Prossimo::Arrenditi(motivo);
    }
    Prossimo::Riprova("non ha detto perche', e la memoria che finisce di colpo fa proprio questo".to_string())
}

/// Quanto tempo si aspetta questo tentativo.
///
/// Il primo puo' essere lento davvero: un modello da decine di gigabyte, un
/// disco lento, la cache del sistema fredda. Dal secondo in poi quel file il
/// sistema ce l'ha gia' in mano — se non risponde entro due minuti non e'
/// lentezza, e' un guasto, e aspettarne dieci vuol dire solo che NOVA sta
/// zitta un'ora prima di dire che non ce l'ha fatta.
pub const ATTESA_DOPO_IL_PRIMO_S: u64 = 120;

pub fn attesa_del_tentativo(primo: bool, configurata_s: u64) -> u64 {
    if primo {
        configurata_s
    } else {
        std::cmp::min(configurata_s, ATTESA_DOPO_IL_PRIMO_S)
    }
}

/// Quanto di una riga di registro si mostra: una riga di llama.cpp arriva
/// anche a mille caratteri, e in un pannello diventa un muro.
const QUANTO_DEL_MOTIVO: usize = 200;

fn accorcia(riga: &str) -> String {
    let quanti = riga.chars().count();
    if quanti <= QUANTO_DEL_MOTIVO {
        return riga.to_string();
    }
    let testa: String = riga.chars().take(QUANTO_DEL_MOTIVO).collect();
    format!("{testa}… (+{} caratteri)", quanti - QUANTO_DEL_MOTIVO)
}

#[cfg(test)]
mod prove_del_giro {
    use super::*;

    fn dopo(e: Esito, coda: &str) -> Prossimo {
        dopo_un_tentativo(e, coda, true, true)
    }

    #[test]
    fn pronto_e_pronto_e_basta() {
        assert_eq!(dopo(Esito::Pronto, "qualunque cosa"), Prossimo::Acceso);
        // Anche senza scala e senza auto: se ha risposto, ha risposto.
        assert_eq!(
            dopo_un_tentativo(Esito::Pronto, "error: boh", false, false),
            Prossimo::Acceso
        );
    }

    #[test]
    fn la_memoria_finita_fa_scendere_di_un_gradino() {
        for segno in SEGNI_DI_MEMORIA_FINITA {
            let coda = format!("ggml_backend_cuda: {segno}");
            match dopo(Esito::Morto, &coda) {
                Prossimo::Riprova(p) => assert!(p.contains("memoria"), "{p}"),
                altro => panic!("{segno}: {altro:?}"),
            }
        }
    }

    #[test]
    fn una_morte_muta_vale_come_memoria() {
        // Il caso che la scala esiste per curare e che non lascia tracce: il
        // sistema chiude il processo e non gli da' il tempo di scrivere.
        let coda = "llama_model_loader: loaded meta data\nllm_load_tensors: offloading 30 layers";
        match dopo(Esito::Morto, coda) {
            Prossimo::Riprova(p) => assert!(p.contains("non ha detto perche'"), "{p}"),
            altro => panic!("{altro:?}"),
        }
    }

    #[test]
    fn una_morte_parlante_ferma_il_giro_e_riporta_cosa_ha_detto() {
        // Il difetto vero: qui prima si percorreva tutta la scala dicendo
        // «memoria insufficiente» a ogni gradino, per un flag rifiutato.
        let coda = "error: unknown argument: --flash-attn-tipo";
        match dopo(Esito::Morto, coda) {
            Prossimo::Arrenditi(p) => assert!(p.contains("unknown argument"), "{p}"),
            altro => panic!("scendere di un gradino non cura un flag: {altro:?}"),
        }
    }

    #[test]
    fn ma_la_memoria_vince_anche_se_e_scritta_come_errore() {
        // «error: ... out of memory» parla **e** parla di memoria: e' il
        // caso in cui la scala serve, e l'ordine dei controlli conta.
        let coda = "error: failed to allocate buffer: out of memory";
        match dopo(Esito::Morto, coda) {
            Prossimo::Riprova(p) => assert!(p.contains("memoria"), "{p}"),
            altro => panic!("{altro:?}"),
        }
    }

    #[test]
    fn scaduto_e_muto_e_la_memoria_condivisa() {
        // Vivo, zitto, e dieci volte piu' piano: la memoria condivisa non
        // da' errori, accetta tutto. L'unico rimedio e' meno layer.
        match dopo(Esito::Scaduto, "llm_load_tensors: offloading 40 layers") {
            Prossimo::Riprova(p) => assert!(p.contains("non ha detto perche'"), "{p}"),
            altro => panic!("{altro:?}"),
        }
    }

    #[test]
    fn senza_auto_non_si_scende_mai_e_si_dice_cosa_e_successo() {
        let coda = "error: failed to allocate buffer: out of memory";
        match dopo_un_tentativo(Esito::Morto, coda, false, true) {
            Prossimo::Arrenditi(p) => assert!(p.contains("out of memory"), "{p}"),
            altro => panic!("{altro:?}"),
        }
        // E senza niente da leggere si dice almeno **cosa** e' successo.
        match dopo_un_tentativo(Esito::Scaduto, "", false, true) {
            Prossimo::Arrenditi(p) => assert!(p.contains("non ha risposto"), "{p}"),
            altro => panic!("{altro:?}"),
        }
    }

    #[test]
    fn finita_la_scala_ci_si_ferma() {
        match dopo_un_tentativo(Esito::Morto, "", true, false) {
            Prossimo::Arrenditi(p) => assert!(p.contains("senza dire perche'"), "{p}"),
            altro => panic!("non c'e' piu' niente sotto: {altro:?}"),
        }
    }

    #[test]
    fn dellerrore_si_prende_lultima_riga_non_la_prima() {
        // llama.cpp stampa centinaia di righe prima di morire: la prima e' un
        // banner, l'ultima e' il motivo.
        let coda = "error: this is an old warning\nload: ok\nerror: cannot open model file";
        assert_eq!(
            ha_parlato(coda).as_deref(),
            Some("error: cannot open model file")
        );
    }

    #[test]
    fn una_riga_lunghissima_non_diventa_un_muro() {
        let lunga = format!("error: {}", "x".repeat(500));
        match dopo(Esito::Morto, &lunga) {
            Prossimo::Arrenditi(p) => {
                assert!(p.chars().count() < 260, "{}", p.chars().count());
                assert!(p.contains("caratteri"), "deve dire che c'e' dell'altro");
            }
            altro => panic!("{altro:?}"),
        }
    }

    #[test]
    fn la_seconda_attesa_e_molto_piu_corta_della_prima() {
        assert_eq!(attesa_del_tentativo(true, 600), 600);
        assert_eq!(attesa_del_tentativo(false, 600), ATTESA_DOPO_IL_PRIMO_S);
        // Ma non si allunga mai un'attesa gia' corta.
        assert_eq!(attesa_del_tentativo(false, 30), 30);
    }
}
