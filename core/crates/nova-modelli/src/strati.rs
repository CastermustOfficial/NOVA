//! Quanti strati stanno davvero in VRAM.
//!
//! E' il calcolo che decide se NOVA va a trenta token al secondo o a tre, e
//! si fa **prima** di avviare llama.cpp perche' dopo e' tardi: quando la VRAM
//! finisce, il driver NVIDIA su Windows non fallisce - ripiega in silenzio
//! sulla memoria condivisa. Il modello parte, risponde, e va dieci volte piu'
//! piano senza che nessuno lo dica. Un guasto che non solleva e' peggio di
//! uno che solleva, e questo e' il caso da manuale.
//!
//! Il banco l'ha misurato su questa macchina: da 53 a 60 strati la
//! generazione cresce del cinquanta per cento, a 62 peggiora, a 64 crolla.
//! La curva ha un ginocchio e sta a ridosso del limite: per questo si calcola
//! invece di passare `-ngl 999` e sperare.

/// Quanto occupa la cache KV rispetto a `f16`, per tipo.
///
/// Non e' un dettaglio contabile: una cache dimezzata sono megabyte che
/// diventano strati, e gli strati sono la differenza fra le due velocita' di
/// cui sopra.
pub const PESO_KV: &[(&str, f64)] = &[
    ("f16", 1.0),
    ("bf16", 1.0),
    ("q8_0", 0.5),
    ("q5_1", 0.36),
    ("q4_0", 0.28),
];

/// Il margine da lasciare al desktop e al browser, in MiB.
pub const RISERVA_MB: f64 = 900.0;

/// Non si usa tutta la VRAM libera dichiarata: il 4% resta fuori. Fra la
/// lettura e l'avvio passano secondi, e in quei secondi il desktop puo'
/// prendersi qualche decina di megabyte.
pub const QUOTA_UTILIZZABILE: f64 = 0.96;

/// La cache KV non scende sotto questo, anche a contesto minuscolo: sotto ci
/// sono comunque i buffer di calcolo.
pub const KV_MINIMO_MB: f64 = 256.0;

/// MiB di cache KV per token di contesto, a `f16`. Stima prudente.
pub const KV_PER_TOKEN_MB: f64 = 0.05;

pub fn peso_kv(tipo: &str) -> f64 {
    PESO_KV
        .iter()
        .find(|(n, _)| *n == tipo)
        .map(|(_, p)| *p)
        .unwrap_or(1.0)
}

/// Quanti strati mettere su GPU.
///
/// Zero e' una risposta legittima e vuol dire «tutto in CPU»: e' lenta ma
/// funziona, mentre metterne troppi non e' lento - e' finto veloce.
///
/// `n_strati` a zero vuol dire «non ho letto la forma del modello»: in quel
/// caso non si indovina, si torna zero. Meglio partire piano di sicuro che
/// partire a caso.
pub fn strati_su_gpu(
    byte_modello: u64,
    n_strati: u32,
    vram_libera_mb: u64,
    ctx: u32,
    riserva_mb: f64,
    kv_tipo: &str,
) -> u32 {
    if n_strati == 0 || vram_libera_mb == 0 || byte_modello == 0 {
        return 0;
    }
    let mb_modello = byte_modello as f64 / (1024.0 * 1024.0);
    let kv_mb = (ctx as f64 * KV_PER_TOKEN_MB).max(KV_MINIMO_MB) * peso_kv(kv_tipo);
    let disponibile = vram_libera_mb as f64 * QUOTA_UTILIZZABILE - riserva_mb - kv_mb;
    // `n_strati + 1`: oltre ai blocchi c'e' lo strato di uscita, che pesa come
    // uno di loro e che llama.cpp mette su GPU insieme agli altri. Dividere
    // per il solo numero di blocchi sovrastima ogni strato e ne fa entrare
    // uno di troppo - proprio quello che si voleva evitare.
    let per_strato = mb_modello / (n_strati as f64 + 1.0);
    if disponibile <= per_strato {
        return 0;
    }
    let quanti = (disponibile / per_strato).floor();
    (quanti as u32).min(n_strati)
}

#[cfg(test)]
mod prove {
    use super::*;

    const GB: u64 = 1024 * 1024 * 1024;

    #[test]
    fn senza_forma_non_si_indovina() {
        assert_eq!(strati_su_gpu(16 * GB, 0, 24000, 8192, RISERVA_MB, "f16"), 0);
    }

    #[test]
    fn senza_vram_tutto_in_cpu() {
        assert_eq!(strati_su_gpu(16 * GB, 64, 0, 8192, RISERVA_MB, "f16"), 0);
    }

    #[test]
    fn non_si_supera_mai_il_numero_di_strati() {
        // Una scheda enorme non fa comparire strati che il modello non ha.
        let n = strati_su_gpu(4 * GB, 32, 80_000, 8192, RISERVA_MB, "f16");
        assert_eq!(n, 32);
    }

    #[test]
    fn la_cache_piu_leggera_regala_strati() {
        let f16 = strati_su_gpu(16 * GB, 64, 16_000, 131_072, RISERVA_MB, "f16");
        let q8 = strati_su_gpu(16 * GB, 64, 16_000, 131_072, RISERVA_MB, "q8_0");
        assert!(q8 > f16, "q8_0 {q8} non batte f16 {f16}");
    }

    #[test]
    fn un_tipo_sconosciuto_e_prudente() {
        // Se non si sa quanto pesa, si assume che pesi come f16: mai meno.
        assert_eq!(peso_kv("q3_k_xxl_inventato"), 1.0);
        assert_eq!(
            strati_su_gpu(16 * GB, 64, 16_000, 8192, RISERVA_MB, "boh"),
            strati_su_gpu(16 * GB, 64, 16_000, 8192, RISERVA_MB, "f16")
        );
    }

    #[test]
    fn un_modello_che_non_ci_sta_da_zero_non_un_numero_negativo() {
        assert_eq!(strati_su_gpu(60 * GB, 80, 2_000, 8192, RISERVA_MB, "f16"), 0);
    }
}
