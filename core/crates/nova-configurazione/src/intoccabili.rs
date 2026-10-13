//! I file di NOVA che solo NOVA scrive (D410).
//!
//! Il 12 ottobre un Dot su Antigravity, che lavora con le mani sue
//! (`--dangerously-skip-permissions`), ha riscritto `config.json` per
//! eseguire un compito del piano: ha tolto Claude dalla scala, spento la
//! voce e azzerato il tetto. Nessuno strumento di NOVA era passato di li':
//! una CLI agentica scrive dove vuole. Senza Claude non c'era piu' un
//! giudice, e il progetto si e' fermato.
//!
//! **Chi scrive questi file per conto di NOVA ne lascia una copia buona**,
//! in `copie-buone/` accanto a `config.json`, con lo stesso percorso. Il
//! demone confronta i file con le loro copie mentre un Dot lavora e quando
//! finisce un compito ([`controlla`]): un file diverso dalla sua copia l'ha
//! scritto qualcun altro, e torna com'era. La versione estranea non si butta:
//! va in `copie-buone/estranei/`, con l'ora nel nome.
//!
//! **La copia si scrive prima del file** ([`prima_di_scrivere`]). Se il
//! controllo cade proprio fra le due scritture, trova il file vecchio e la
//! copia nuova: nello stesso processo non succede, perche' chi scrive
//! ([`scrivi_da_nova`]) e chi controlla si danno il turno; da un altro
//! processo (il pannello, il Python) il controllo riguarda dopo un attimo
//! ([`RIGUARDA`]), e un file che nel frattempo e' diventato uguale alla copia
//! era una scrittura di NOVA a meta'. La prova del demone l'ha preso in
//! fallo il 13 ottobre: l'indice dei progetti scritto dall'APM mentre
//! lavorava l'Architetto, e l'Architetto accusato.
//!
//! **Con nessun Dot al lavoro, quello che c'e' si adotta** ([`adotta`]): un
//! file cambiato mentre i Dot erano fermi l'ha cambiato l'utente, a mano o
//! con un altro programma, ed e' suo. Il demone adotta all'accensione e ogni
//! volta che un compito comincia senza altri Dot al lavoro.
//!
//! Non e' una serratura: un programma che sa di questa cartella puo'
//! riscrivere anche la copia. E' la rete sotto chi sbaglia in buona fede,
//! come quel Dot, che eseguiva un compito scritto male.

use std::path::{Path, PathBuf};

/// La cartella delle copie buone, accanto a `config.json`.
pub const COPIE: &str = "copie-buone";

/// Dentro le copie: le versioni estranee messe da parte.
pub const ESTRANEI: &str = "estranei";

/// I file sorvegliati che stanno direttamente nella cartella di NOVA.
pub const FILE_DI_NOVA: [&str; 2] = ["config.json", "progetti.json"];

/// La cartella dei Dot, e il file che dice chi e' ciascuno.
pub const CARTELLA_DOT: &str = "dots";
pub const FILE_DOT: &str = "dot.json";

/// Chi scrive per NOVA e chi controlla si danno il turno, nello stesso
/// processo.
static TURNO: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Quanto aspetta il controllo prima di riguardare un file diverso dalla
/// copia: una scrittura di NOVA da un altro processo, a meta' fra la copia e
/// il file, nel frattempo e' finita.
pub const RIGUARDA: std::time::Duration = std::time::Duration::from_millis(300);

/// Un file sorvegliato che e' cambiato senza NOVA.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toccato {
    /// Il percorso dalla cartella di NOVA, con `/`: `config.json`,
    /// `dots/ar/dot.json`.
    pub file: String,
    /// Dove e' finita la versione estranea, se il file c'era.
    pub estraneo: Option<PathBuf>,
    /// Il file e' tornato com'era (o e' stato tolto, se non c'era).
    pub rimesso: bool,
    /// Perche' non e' tornato, se non e' tornato.
    pub errore: String,
}

/// Il percorso di `f` dalla cartella di NOVA, con `/`. `None` se `f` non sta
/// dentro.
fn relativo(nova: &Path, f: &Path) -> Option<String> {
    let r = f.strip_prefix(nova).ok()?;
    let parti: Vec<String> = r
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    (!parti.is_empty()).then(|| parti.join("/"))
}

/// Dove sta la copia buona di `f`.
pub fn copia_di(nova: &Path, f: &Path) -> Option<PathBuf> {
    let r = relativo(nova, f)?;
    let mut p = nova.join(COPIE);
    for parte in r.split('/') {
        p.push(parte);
    }
    Some(p)
}

/// Scrive un file per intero: prima di fianco, poi al suo posto.
fn scrivi_intero(p: &Path, contenuto: &[u8]) -> Result<(), String> {
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let nuovo = p.with_extension("intoccabile-nuovo");
    std::fs::write(&nuovo, contenuto).map_err(|e| format!("{}: {e}", nuovo.display()))?;
    std::fs::rename(&nuovo, p).map_err(|e| format!("{}: {e}", p.display()))
}

/// NOVA sta per scrivere `contenuto` in `f`: prima ne lascia la copia buona.
/// Un file fuori dalla cartella di NOVA non ha copia, e non e' un errore.
pub fn prima_di_scrivere(nova: &Path, f: &Path, contenuto: &[u8]) -> Result<(), String> {
    match copia_di(nova, f) {
        Some(c) => scrivi_intero(&c, contenuto),
        None => Ok(()),
    }
}

/// NOVA scrive `contenuto` in `f`, nello stesso processo del controllo:
/// la copia, poi il file con `scrivi`, senza che il controllo passi in
/// mezzo.
pub fn scrivi_da_nova(
    nova: &Path,
    f: &Path,
    contenuto: &[u8],
    scrivi: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let _turno = TURNO.lock().unwrap_or_else(|e| e.into_inner());
    prima_di_scrivere(nova, f, contenuto)?;
    scrivi()
}

/// NOVA ha tolto `f` (un Dot licenziato): via anche la copia, se no il
/// controllo lo rimetterebbe.
pub fn tolto_da_nova(nova: &Path, f: &Path) -> Result<(), String> {
    let Some(c) = copia_di(nova, f) else {
        return Ok(());
    };
    match std::fs::remove_file(&c) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("{}: {e}", c.display())),
    }
}

/// I file sorvegliati: quelli che ci sono e quelli che hanno una copia. Un
/// `dot.json` che c'e' senza copia e' un Dot nato fuori da NOVA; una copia
/// senza file e' un Dot sparito: si guardano tutti e due.
pub fn sorvegliati(nova: &Path) -> Vec<PathBuf> {
    let mut tutti: Vec<PathBuf> = FILE_DI_NOVA.iter().map(|n| nova.join(n)).collect();
    for base in [nova.join(CARTELLA_DOT), nova.join(COPIE).join(CARTELLA_DOT)] {
        let Ok(voci) = std::fs::read_dir(&base) else {
            continue;
        };
        for v in voci.flatten() {
            if v.path().join(FILE_DOT).is_file() {
                tutti.push(nova.join(CARTELLA_DOT).join(v.file_name()).join(FILE_DOT));
            }
        }
    }
    tutti.sort();
    tutti.dedup();
    tutti
}

fn leggi(p: &Path) -> Option<Vec<u8>> {
    std::fs::read(p).ok()
}

/// Rende le copie uguali ai file: quello che c'e' e' buono. Torna i file la
/// cui copia e' cambiata.
pub fn adotta(nova: &Path) -> Vec<String> {
    let _turno = TURNO.lock().unwrap_or_else(|e| e.into_inner());
    let mut adottati = Vec::new();
    for f in sorvegliati(nova) {
        let (Some(c), Some(r)) = (copia_di(nova, &f), relativo(nova, &f)) else {
            continue;
        };
        let (attuale, copia) = (leggi(&f), leggi(&c));
        if attuale == copia {
            continue;
        }
        let fatto = match &attuale {
            Some(b) => scrivi_intero(&c, b),
            None => std::fs::remove_file(&c).map_err(|e| e.to_string()),
        };
        match fatto {
            Ok(()) => adottati.push(r),
            Err(e) => eprintln!("[intoccabili] la copia di {r} non si aggiorna: {e}"),
        }
    }
    adottati
}

/// Un nome di file da un'ora: niente `:`, che Windows non vuole.
fn per_nome(quando: &str) -> String {
    quando
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' })
        .collect()
}

/// Confronta i file con le copie e rimette com'erano quelli cambiati senza
/// NOVA. `quando` finisce nel nome delle versioni estranee.
///
/// - un file diverso dalla copia: la versione estranea va da parte, la copia
///   torna al suo posto;
/// - un `dot.json` che c'e' senza copia: va da parte, e il posto resta
///   vuoto;
/// - `config.json` o `progetti.json` senza copia: si adotta, mai si toglie;
/// - una copia senza file: il file torna.
pub fn controlla(nova: &Path, quando: &str) -> Vec<Toccato> {
    controlla_con(nova, quando, RIGUARDA)
}

/// Come [`controlla`], col tempo per riguardare scelto: le prove non
/// aspettano.
pub fn controlla_con(nova: &Path, quando: &str, riguarda: std::time::Duration) -> Vec<Toccato> {
    let _turno = TURNO.lock().unwrap_or_else(|e| e.into_inner());
    let diversi = |tutti: Vec<PathBuf>| -> Vec<PathBuf> {
        tutti
            .into_iter()
            .filter(|f| copia_di(nova, f).is_some_and(|c| leggi(f) != leggi(&c)))
            .collect()
    };
    let mut sospetti = diversi(sorvegliati(nova));
    // Una scrittura di NOVA da un altro processo, fra la copia e il file:
    // dopo un attimo e' finita, e il file e' uguale alla copia.
    if !sospetti.is_empty() && !riguarda.is_zero() {
        std::thread::sleep(riguarda);
        sospetti = diversi(sospetti);
    }
    let mut toccati = Vec::new();
    for f in sospetti {
        let (Some(c), Some(r)) = (copia_di(nova, &f), relativo(nova, &f)) else {
            continue;
        };
        let (attuale, copia) = (leggi(&f), leggi(&c));
        // Ne' file ne' copia: un `progetti.json` che non c'e' ancora.
        if attuale == copia {
            continue;
        }
        // La configurazione senza copia non si toglie mai: la copia manca
        // perche' qualcuno ha svuotato `copie-buone/`, non perche' il file
        // sia estraneo. Si adotta.
        if copia.is_none() && FILE_DI_NOVA.contains(&r.as_str()) {
            if let Some(b) = &attuale {
                if let Err(e) = scrivi_intero(&c, b) {
                    eprintln!("[intoccabili] la copia di {r} non si scrive: {e}");
                }
            }
            continue;
        }
        let mut t = Toccato {
            file: r.clone(),
            estraneo: None,
            rimesso: false,
            errore: String::new(),
        };
        if let Some(b) = &attuale {
            let mut dove = nova.join(COPIE).join(ESTRANEI);
            for parte in r.split('/') {
                dove.push(parte);
            }
            let nome = format!(
                "{}.{}",
                dove.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                per_nome(quando)
            );
            dove.set_file_name(nome);
            match scrivi_intero(&dove, b) {
                Ok(()) => t.estraneo = Some(dove),
                Err(e) => t.errore = format!("la versione estranea non si mette da parte: {e}"),
            }
        }
        // Senza la versione estranea al sicuro non si sovrascrive niente: si
        // dice e basta.
        if t.errore.is_empty() {
            let fatto = match &copia {
                Some(b) => scrivi_intero(&f, b),
                None => std::fs::remove_file(&f).map_err(|e| format!("{}: {e}", f.display())),
            };
            match fatto {
                Ok(()) => t.rimesso = true,
                Err(e) => t.errore = e,
            }
        }
        toccati.push(t);
    }
    toccati
}

#[cfg(test)]
mod prove {
    use super::*;

    fn nova(nome: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("nova-intoccabili-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(p.join(CARTELLA_DOT).join("ar")).unwrap();
        p
    }

    fn scrivi_da_nova(n: &Path, f: &Path, testo: &str) {
        prima_di_scrivere(n, f, testo.as_bytes()).unwrap();
        std::fs::write(f, testo).unwrap();
    }

    #[test]
    fn la_copia_sta_accanto_con_lo_stesso_percorso() {
        let n = Path::new("/x/NOVA");
        assert_eq!(copia_di(n, &n.join("config.json")), Some(n.join(COPIE).join("config.json")));
        assert_eq!(
            copia_di(n, &n.join("dots").join("ar").join("dot.json")),
            Some(n.join(COPIE).join("dots").join("ar").join("dot.json"))
        );
        assert_eq!(copia_di(n, Path::new("/altrove/config.json")), None, "fuori da NOVA niente copia");
        assert_eq!(copia_di(n, n), None);
    }

    #[test]
    fn quel_che_scrive_nova_non_si_tocca() {
        let n = nova("nova");
        let cfg = n.join("config.json");
        scrivi_da_nova(&n, &cfg, "{\"a\": 1}");
        let dot = n.join(CARTELLA_DOT).join("ar").join(FILE_DOT);
        scrivi_da_nova(&n, &dot, "{\"nome\": \"ar\"}");
        assert!(controlla(&n, "t").is_empty());
        assert_eq!(std::fs::read_to_string(&cfg).unwrap(), "{\"a\": 1}");
        let _ = std::fs::remove_dir_all(&n);
    }

    #[test]
    fn un_file_cambiato_da_fuori_torna_e_l_estraneo_resta_da_parte() {
        let n = nova("fuori");
        let cfg = n.join("config.json");
        scrivi_da_nova(&n, &cfg, "{\"difficile\": \"claude\"}");
        std::fs::write(&cfg, "{\"difficile\": \"gemini\"}").unwrap();
        let t = controlla(&n, "2026-10-12T22:58:18");
        assert_eq!(t.len(), 1, "{t:?}");
        assert_eq!(t[0].file, "config.json");
        assert!(t[0].rimesso && t[0].errore.is_empty(), "{t:?}");
        assert_eq!(std::fs::read_to_string(&cfg).unwrap(), "{\"difficile\": \"claude\"}");
        let estraneo = t[0].estraneo.clone().unwrap();
        assert_eq!(
            estraneo,
            n.join(COPIE).join(ESTRANEI).join("config.json.2026-10-12T22-58-18"),
            "senza «:», che Windows non vuole"
        );
        assert_eq!(std::fs::read_to_string(estraneo).unwrap(), "{\"difficile\": \"gemini\"}");
        assert!(controlla(&n, "t").is_empty(), "rimesso, e' di nuovo a posto");
        let _ = std::fs::remove_dir_all(&n);
    }

    #[test]
    fn un_dot_nato_fuori_si_toglie_e_uno_sparito_torna() {
        let n = nova("dot");
        let ar = n.join(CARTELLA_DOT).join("ar").join(FILE_DOT);
        scrivi_da_nova(&n, &ar, "{\"nome\": \"ar\"}");
        std::fs::remove_file(&ar).unwrap();
        let finto = n.join(CARTELLA_DOT).join("finto");
        std::fs::create_dir_all(&finto).unwrap();
        std::fs::write(finto.join(FILE_DOT), "{\"nome\": \"finto\"}").unwrap();
        let t = controlla(&n, "t");
        let file: Vec<&str> = t.iter().map(|x| x.file.as_str()).collect();
        assert_eq!(file, ["dots/ar/dot.json", "dots/finto/dot.json"], "{t:?}");
        assert!(t.iter().all(|x| x.rimesso), "{t:?}");
        assert_eq!(std::fs::read_to_string(&ar).unwrap(), "{\"nome\": \"ar\"}", "lo sparito torna");
        assert!(t[0].estraneo.is_none(), "non c'era niente da mettere da parte");
        assert!(!finto.join(FILE_DOT).exists(), "il nato fuori non e' un Dot");
        assert!(t[1].estraneo.as_ref().is_some_and(|p| p.is_file()), "ma non si butta");
        let _ = std::fs::remove_dir_all(&n);
    }

    #[test]
    fn senza_dot_al_lavoro_quel_che_c_e_si_adotta() {
        let n = nova("adotta");
        let cfg = n.join("config.json");
        scrivi_da_nova(&n, &cfg, "{\"a\": 1}");
        std::fs::write(&cfg, "{\"a\": 2}").unwrap();
        std::fs::write(n.join("progetti.json"), "{}").unwrap();
        assert_eq!(adotta(&n), ["config.json", "progetti.json"]);
        assert!(controlla(&n, "t").is_empty(), "adottato, non e' piu' estraneo");
        assert_eq!(std::fs::read_to_string(&cfg).unwrap(), "{\"a\": 2}", "e resta com'e'");
        assert!(adotta(&n).is_empty(), "la seconda volta non c'e' niente da adottare");
        let _ = std::fs::remove_dir_all(&n);
    }

    #[test]
    fn un_dot_licenziato_non_torna() {
        let n = nova("licenziato");
        let ar = n.join(CARTELLA_DOT).join("ar").join(FILE_DOT);
        scrivi_da_nova(&n, &ar, "{\"nome\": \"ar\"}");
        std::fs::remove_dir_all(n.join(CARTELLA_DOT).join("ar")).unwrap();
        tolto_da_nova(&n, &ar).unwrap();
        tolto_da_nova(&n, &ar).unwrap();
        assert!(controlla(&n, "t").is_empty());
        assert!(!ar.exists());
        let _ = std::fs::remove_dir_all(&n);
    }

    #[test]
    fn una_scrittura_di_nova_a_meta_non_e_un_estraneo() {
        // Da un altro processo (il pannello, il Python) la copia e' scritta e
        // il file non ancora: il controllo riguarda, e intanto e' finita.
        let n = nova("a-meta");
        let cfg = n.join("config.json");
        super::scrivi_da_nova(&n, &cfg, b"{\"a\": 1}", || {
            std::fs::write(&cfg, "{\"a\": 1}").map_err(|e| e.to_string())
        })
        .unwrap();
        prima_di_scrivere(&n, &cfg, b"{\"a\": 2}").unwrap();
        let f = cfg.clone();
        let altro = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(50));
            std::fs::write(&f, "{\"a\": 2}").unwrap();
        });
        let t = controlla_con(&n, "t", std::time::Duration::from_millis(500));
        altro.join().unwrap();
        assert!(t.is_empty(), "{t:?}");
        assert_eq!(std::fs::read_to_string(&cfg).unwrap(), "{\"a\": 2}");
        assert!(estranei_vuoti(&n));
        // Senza riguardare, la stessa scrittura a meta' sembrerebbe estranea:
        // e' questo che la pausa evita.
        prima_di_scrivere(&n, &cfg, b"{\"a\": 3}").unwrap();
        assert_eq!(controlla_con(&n, "t", std::time::Duration::ZERO).len(), 1);
        let _ = std::fs::remove_dir_all(&n);
    }

    fn estranei_vuoti(n: &Path) -> bool {
        !n.join(COPIE).join(ESTRANEI).exists()
    }

    #[test]
    fn la_configurazione_senza_copia_si_adotta_e_non_si_toglie() {
        let n = nova("senza-copia");
        let cfg = n.join("config.json");
        std::fs::write(&cfg, "{\"a\": 1}").unwrap();
        assert!(controlla(&n, "t").is_empty());
        assert_eq!(std::fs::read_to_string(&cfg).unwrap(), "{\"a\": 1}");
        assert_eq!(
            std::fs::read_to_string(copia_di(&n, &cfg).unwrap()).unwrap(),
            "{\"a\": 1}",
            "e da adesso ha la sua copia"
        );
        let _ = std::fs::remove_dir_all(&n);
    }
}
