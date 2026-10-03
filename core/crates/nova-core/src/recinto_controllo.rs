//! Il controllo delle cartelle di terzi che il contenitore puo' scrivere (D367).
//!
//! Su Windows il contenitore ha il controllo d'accesso due volte, e Everyone
//! non gli basta. ALL APPLICATION PACKAGES si': le cartelle di terzi che le
//! sono aperte — su una macchina vera, quattro, fra driver e Segnalazione
//! errori di Windows — un comando confinato le puo' scrivere.
//!
//! **Non si possono chiudere solo per il contenitore** (vedi
//! `nova_platform::recinto::windows`): un divieto intestato a lui non lo
//! ferma, e senza ALL APPLICATION PACKAGES PowerShell non parte. Toccare i
//! permessi di quelle cartelle per tutte le app di Windows non e' nostro. Resta
//! una cosa sola, e va fatta bene: **rilevarle, dirlo, e rifarlo** — un
//! aggiornamento del produttore le cambia.
//!
//! Il controllo percorre tutti i dischi fissi che salvano i permessi (NTFS,
//! ReFS), senza limite di livelli,
//! a priorita' bassa. Si fa un minuto dopo l'avvio del demone, se manca o ha
//! piu' di un giorno, e si rinnova da solo. Si ferma quando il demone si chiude. Il risultato sta in `recinto-controllo.json`,
//! accanto a `core.json`, e **ogni comando confinato lo racconta**, con la
//! data: chi legge deve sapere quanto e' vecchio quel che gli si dice.
//!
//! Dove non c'e' il contenitore (Linux) non c'e' niente da controllare.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

/// Il demone si sta chiudendo: il controllo in corso si ferma alla prossima
/// cartella. Senza, il filo bloccante che percorre i dischi — minuti — lo
/// aspetterebbe il runtime, e il demone non si spegnerebbe.
static FERMO: AtomicBool = AtomicBool::new(false);

/// Chiede al controllo in corso di fermarsi, e a quelli futuri di non partire.
/// Lo chiama `novad` quando smette di ascoltare.
pub fn ferma() {
    FERMO.store(true, Ordering::SeqCst);
}

/// Quanto aspetta il primo controllo dopo l'avvio del demone: un minuto, cosi'
/// un demone di vita breve non lancia mai una scansione dei dischi, e quello
/// che dura non la fa mentre il PC si sta ancora avviando.
/// `NOVA_RECINTO_CONTROLLO_RITARDO_S` lo cambia — le prove lo mettono a zero
/// per vedere cosa succede se il demone si chiude durante la scansione.
// Il controllo periodico esiste solo su Windows: altrove nessuno la chiama.
#[cfg_attr(not(windows), allow(dead_code))]
fn ritardo() -> std::time::Duration {
    let secondi = std::env::var("NOVA_RECINTO_CONTROLLO_RITARDO_S")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(60);
    std::time::Duration::from_secs(secondi)
}

/// Il risultato di un controllo.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Rapporto {
    /// Quando e' finito, in secondi da epoca Unix.
    pub quando: u64,
    pub durata_s: u64,
    /// I dischi percorsi: quelli che salvano i permessi (NTFS, ReFS).
    pub dischi: Vec<String>,
    /// I dischi fissi dove i permessi non esistono (FAT, exFAT...): il
    /// contenitore vi scrive ovunque.
    #[serde(default)]
    pub dischi_senza_permessi: Vec<String>,
    pub cartelle: u64,
    /// Non coperte: permessi illeggibili.
    #[serde(default)]
    pub non_leggibili: u64,
    /// Non coperte: elenco illeggibile, quindi il sottoalbero non si e' visto.
    #[serde(default)]
    pub non_elencabili: u64,
    /// Fermato prima della fine.
    #[serde(default)]
    pub interrotto: bool,
    /// Le cartelle che il contenitore puo' scrivere perche' aperte a tutti i
    /// pacchetti.
    #[serde(default)]
    pub aperte: Vec<String>,
}

/// Dove sta il rapporto: accanto a `core.json`.
pub fn percorso() -> PathBuf {
    crate::config::Config::path().with_file_name("recinto-controllo.json")
}

/// L'ultimo rapporto. `None` se non c'e', o non si legge: in quel caso si dice
/// «non ancora fatto» e si rifa', non si finge un esito.
pub fn ultimo() -> Option<Rapporto> {
    serde_json::from_slice(&std::fs::read(percorso()).ok()?).ok()
}

#[cfg(windows)]
fn salva_in(dove: &std::path::Path, r: &Rapporto) -> Result<(), String> {
    if let Some(su) = dove.parent() {
        std::fs::create_dir_all(su).map_err(|e| format!("non creo {}: {e}", su.display()))?;
    }
    let testo = serde_json::to_vec_pretty(r).map_err(|e| format!("non compongo il rapporto: {e}"))?;
    let provvisorio = dove.with_extension("json.nuovo");
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&provvisorio)
            .map_err(|e| format!("non scrivo {}: {e}", provvisorio.display()))?;
        f.write_all(&testo)
            .and_then(|_| f.sync_all())
            .map_err(|e| format!("non scrivo {}: {e}", provvisorio.display()))?;
    }
    std::fs::rename(&provvisorio, &dove).map_err(|e| format!("non sostituisco {}: {e}", dove.display()))
}

/// Adesso, in secondi da epoca Unix.
pub fn adesso() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Una data in UTC, `AAAA-MM-GG HH:MM`. Senza librerie di date: la formula e'
/// quella civile standard (giorni dall'epoca, ere da 400 anni), e le prove la
/// controllano su date note, anni bisestili compresi.
pub fn data_utc(secondi: u64) -> String {
    let giorni = (secondi / 86_400) as i64;
    let resto = secondi % 86_400;
    let z = giorni + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let giorno = doy - (153 * mp + 2) / 5 + 1;
    let mese = if mp < 10 { mp + 3 } else { mp - 9 };
    let anno = if mese <= 2 { y + 1 } else { y };
    format!("{anno:04}-{mese:02}-{giorno:02} {:02}:{:02}", resto / 3600, (resto % 3600) / 60)
}

/// Quel che un comando confinato racconta del controllo. Pura: la prova la
/// guarda con rapporti inventati.
pub fn racconto_da(r: Option<&Rapporto>, adesso: u64) -> String {
    let Some(r) = r else {
        return "controllo delle cartelle di terzi: non ancora fatto (parte all'avvio del \
                demone, o con `novad --recinto --controlla`)"
            .to_string();
    };
    let mut s = format!(
        "controllo delle cartelle di terzi del {} UTC su {} cartelle: ",
        data_utc(r.quando),
        r.cartelle
    );
    if r.aperte.is_empty() {
        s.push_str("nessuna scrivibile dal contenitore");
    } else {
        let primi: Vec<&str> = r.aperte.iter().take(5).map(String::as_str).collect();
        s.push_str(&format!(
            "{} che il contenitore puo' scrivere perche' aperte a tutti i pacchetti, e che \
             NOVA non puo' chiudere: {}",
            r.aperte.len(),
            primi.join(", ")
        ));
        if r.aperte.len() > 5 {
            s.push_str(&format!(" e altre {}", r.aperte.len() - 5));
        }
    }
    if !r.dischi_senza_permessi.is_empty() {
        s.push_str(&format!(
            "; dischi senza permessi, dove il contenitore scrive ovunque: {}",
            r.dischi_senza_permessi.join(", ")
        ));
    }
    if r.non_leggibili + r.non_elencabili > 0 {
        s.push_str(&format!(
            "; non coperte: {} con permessi illeggibili, {} non elencabili",
            r.non_leggibili, r.non_elencabili
        ));
    }
    if r.interrotto {
        s.push_str("; controllo interrotto");
    }
    if adesso.saturating_sub(r.quando) > 7 * 86_400 {
        s.push_str("; piu' vecchio di 7 giorni");
    }
    s
}

/// La riga di adesso, letta dal disco.
pub fn racconto_riga() -> String {
    racconto_da(ultimo().as_ref(), adesso())
}

/// Percorre tutti i dischi fissi e salva il rapporto. `avanzamento` riceve una
/// riga ogni tanto. Un solo controllo alla volta, fra tutti i processi di NOVA.
#[cfg(windows)]
pub fn esegui(avanzamento: &dyn Fn(&str)) -> Result<Rapporto, String> {
    esegui_in(&percorso(), &FERMO, avanzamento)
}

/// Come `esegui`, con il posto del rapporto e la bandiera che lo ferma. Un
/// controllo fermato **non salva**: un rapporto a meta' cancellerebbe quello
/// completo di prima, e direbbe di aver guardato dischi che non ha finito.
#[cfg(windows)]
pub fn esegui_in(
    dove: &std::path::Path,
    annulla: &AtomicBool,
    avanzamento: &dyn Fn(&str),
) -> Result<Rapporto, String> {
    use nova_platform::recinto::windows::{
        assicura_profilo, cerca_cartelle_aperte, dischi_fissi, priorita_bassa, sonda,
        BloccoFraProcessi,
    };
    use std::cell::Cell;

    if annulla.load(Ordering::SeqCst) {
        return Err("controllo fermato prima di cominciare".to_string());
    }
    let _blocco = BloccoFraProcessi::prova("NOVA-recinto-controllo")
        .ok_or_else(|| "un altro controllo e' gia' in corso".to_string())?;
    // Minuti di scansione non devono rubare il PC a chi lo sta usando.
    struct Piano;
    impl Drop for Piano {
        fn drop(&mut self) {
            priorita_bassa(false);
        }
    }
    priorita_bassa(true);
    let _piano = Piano;

    let chi = assicura_profilo()?;
    let so = sonda(&chi)?;
    let (ntfs, altri) = dischi_fissi();
    let inizio = std::time::Instant::now();
    let ogni = Cell::new(0u64);
    let mut r = Rapporto::default();
    for d in &ntfs {
        r.dischi.push(d.display().to_string());
        let s = cerca_cartelle_aperte(d, &so, annulla, &|n| {
            ogni.set(ogni.get() + 1);
            if ogni.get() % 10 == 0 {
                avanzamento(&format!("{}  {n} cartelle", d.display()));
            }
        });
        r.cartelle += s.cartelle;
        r.non_leggibili += s.non_leggibili;
        r.non_elencabili += s.non_elencabili;
        r.interrotto |= s.interrotta;
        r.aperte.extend(s.aperte.iter().map(|p| p.display().to_string()));
    }
    r.dischi_senza_permessi = altri
        .iter()
        .map(|(p, formato)| format!("{} ({formato})", p.display()))
        .collect();
    r.aperte.sort();
    r.aperte.dedup();
    r.durata_s = inizio.elapsed().as_secs();
    r.quando = adesso();
    if r.interrotto {
        return Err("controllo fermato a meta': il rapporto di prima resta com'e'".to_string());
    }
    salva_in(dove, &r)?;
    Ok(r)
}

#[cfg(not(windows))]
pub fn esegui(_avanzamento: &dyn Fn(&str)) -> Result<Rapporto, String> {
    Err("il controllo delle cartelle di terzi esiste solo su Windows".to_string())
}

/// Fa partire il controllo in background: dopo `ritardo()` se manca o ha piu'
/// di un giorno, e poi si rinnova da solo. Si chiama una volta, all'avvio del demone,
/// e solo se c'e' un recinto (`write_roots`): scansionare i dischi di chi non
/// usa il contenitore sarebbe lavoro per niente.
pub fn avvia_periodico() {
    #[cfg(windows)]
    {
        static GIA: AtomicBool = AtomicBool::new(false);
        if GIA.swap(true, Ordering::SeqCst) {
            return;
        }
        tokio::spawn(async {
            tokio::time::sleep(ritardo()).await;
            while !FERMO.load(Ordering::SeqCst) {
                let da_rifare = match ultimo() {
                    None => true,
                    Some(r) => adesso().saturating_sub(r.quando) > 24 * 3600,
                };
                if da_rifare {
                    match tokio::task::spawn_blocking(|| esegui(&|_| {})).await {
                        Ok(Ok(r)) => tracing::info!(
                            cartelle = r.cartelle,
                            aperte = r.aperte.len(),
                            secondi = r.durata_s,
                            "controllo delle cartelle di terzi fatto"
                        ),
                        Ok(Err(e)) => tracing::warn!(errore = %e, "controllo delle cartelle di terzi non fatto"),
                        Err(e) => tracing::warn!(errore = %e, "controllo delle cartelle di terzi non fatto"),
                    }
                }
                tokio::time::sleep(std::time::Duration::from_secs(6 * 3600)).await;
            }
        });
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    /// Date note: l'epoca, una data qualunque, e i due 29 febbraio che
    /// sbagliano le formule scritte di fretta (2000 e' bisestile, 1900 no).
    #[test]
    fn le_date_in_utc_sono_quelle_giuste() {
        assert_eq!(data_utc(0), "1970-01-01 00:00");
        assert_eq!(data_utc(1_700_000_000), "2023-11-14 22:13");
        assert_eq!(data_utc(951_782_400), "2000-02-29 00:00");
        assert_eq!(data_utc(1_709_164_800), "2024-02-29 00:00");
        assert_eq!(data_utc(1_709_251_199), "2024-02-29 23:59");
        assert_eq!(data_utc(1_709_251_200), "2024-03-01 00:00");
        assert_eq!(data_utc(4_102_444_799), "2099-12-31 23:59");
    }

    fn rapporto() -> Rapporto {
        Rapporto {
            quando: 1_700_000_000,
            durata_s: 230,
            dischi: vec!["C:\\".into(), "D:\\".into()],
            cartelle: 733_898,
            ..Default::default()
        }
    }

    /// Un controllo fermato prima di cominciare non tocca il rapporto di prima:
    /// ne' lo riscrive vuoto, ne' lo cancella.
    #[cfg(windows)]
    #[test]
    fn un_controllo_fermato_non_riscrive_il_rapporto_di_prima() {
        let dove = std::env::temp_dir().join(format!("nova-controllo-fermo-{}.json", std::process::id()));
        let prima = rapporto();
        std::fs::write(&dove, serde_json::to_vec(&prima).unwrap()).unwrap();
        let fermo = AtomicBool::new(true);
        let esito = esegui_in(&dove, &fermo, &|_| {});
        let dopo: Option<Rapporto> = std::fs::read(&dove).ok().and_then(|b| serde_json::from_slice(&b).ok());
        let _ = std::fs::remove_file(&dove);
        assert!(esito.is_err(), "un controllo fermato non e' un controllo fatto");
        assert_eq!(dopo, Some(prima), "il rapporto di prima e' stato toccato");
    }

    #[test]
    fn senza_rapporto_si_dice_che_non_e_stato_fatto() {
        let r = racconto_da(None, 1_700_000_000);
        assert!(r.contains("non ancora fatto"), "{r}");
        assert!(r.contains("--controlla"), "{r}");
    }

    #[test]
    fn il_racconto_porta_la_data_e_il_numero_di_cartelle() {
        let r = racconto_da(Some(&rapporto()), 1_700_000_100);
        assert!(r.contains("2023-11-14 22:13 UTC"), "{r}");
        assert!(r.contains("733898 cartelle"), "{r}");
        assert!(r.contains("nessuna scrivibile dal contenitore"), "{r}");
        assert!(!r.contains("piu' vecchio"), "{r}");
    }

    /// Le cartelle aperte si elencano — le prime cinque, poi quante altre — e
    /// si dice che NOVA non le puo' chiudere: e' la verita', e chi legge deve
    /// poterne tenere conto.
    #[test]
    fn le_cartelle_aperte_si_elencano_e_si_dice_che_non_si_chiudono() {
        let mut r = rapporto();
        r.aperte = (1..=7).map(|n| format!("C:\\terzi\\{n}")).collect();
        let t = racconto_da(Some(&r), r.quando);
        assert!(t.contains("7 che il contenitore puo' scrivere"), "{t}");
        assert!(t.contains("NOVA non puo' chiudere"), "{t}");
        assert!(t.contains("C:\\terzi\\1") && t.contains("C:\\terzi\\5"), "{t}");
        assert!(!t.contains("C:\\terzi\\6"), "{t}");
        assert!(t.contains("e altre 2"), "{t}");
    }

    /// Dove i permessi non esistono, il contenitore non e' confinato affatto:
    /// si dice, e non si lascia credere il contrario.
    #[test]
    fn i_dischi_senza_permessi_si_dichiarano() {
        let mut r = rapporto();
        r.dischi_senza_permessi = vec!["E:\\ (exFAT)".into()];
        let t = racconto_da(Some(&r), r.quando);
        assert!(t.contains("E:\\ (exFAT)") && t.contains("scrive ovunque"), "{t}");
    }

    #[test]
    fn quel_che_non_si_e_visto_si_dice() {
        let mut r = rapporto();
        r.non_leggibili = 609;
        r.non_elencabili = 617;
        r.interrotto = true;
        let t = racconto_da(Some(&r), r.quando);
        assert!(t.contains("609 con permessi illeggibili, 617 non elencabili"), "{t}");
        assert!(t.contains("controllo interrotto"), "{t}");
    }

    #[test]
    fn un_rapporto_vecchio_lo_dice() {
        let r = rapporto();
        let t = racconto_da(Some(&r), r.quando + 8 * 86_400);
        assert!(t.contains("piu' vecchio di 7 giorni"), "{t}");
    }

    /// Un rapporto scritto da una versione precedente, senza i campi nuovi, si
    /// legge ancora.
    #[test]
    fn un_rapporto_con_meno_campi_si_legge() {
        let vecchio = r#"{"quando":1,"durata_s":2,"dischi":["C:\\"],"cartelle":3}"#;
        let r: Rapporto = serde_json::from_str(vecchio).unwrap();
        assert!(r.aperte.is_empty() && !r.interrotto);
    }
}
