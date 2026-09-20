//! Il Cestino fuori da Windows: la specifica freedesktop, e `~/.Trash`.
//!
//! La premessa e' la stessa dell'altra meta' — **prima la reversibilita',
//! poi il permesso** — ma qui c'e' una cosa in piu' da dire, perche' la
//! tentazione e' forte e sbagliata: **non si copia e poi si cancella**.
//!
//! Il Cestino e' un `rename`, cioe' un'operazione che riesce o non riesce.
//! Copiare un file su un altro disco e poi cancellare l'originale e' una
//! sequenza di due operazioni, e fra le due c'e' un momento in cui un disco
//! pieno, un cavo staccato o un processo ucciso lasciano **meta' file** e
//! nessun originale. Quando il `rename` non si puo' fare — il file sta su un
//! altro disco, e su quel disco non c'e' modo di aprire un cestino — la
//! risposta e' «qui l'unica alternativa e' distruggere», e va **detta**,
//! non fatta. E' la stessa frase che la meta' Windows scrive per i dischi di
//! rete, ed e' la stessa ragione.
//!
//! L'altra cosa che questa specifica insegna, e che vale la pena di aver
//! letto: il file `.trashinfo` si scrive **prima** di spostare il file, e si
//! scrive con `O_EXCL`. Non e' pedanteria — e' cosi' che si prenota un nome
//! senza un lucchetto: due programmi che buttano nel Cestino due file con lo
//! stesso nome nello stesso istante non possono vincere tutti e due, perche'
//! il secondo `O_EXCL` fallisce. Scrivendo prima il file e poi l'informazione
//! si perderebbe la corsa e si sovrascriverebbe il file di qualcun altro.

#![cfg(unix)]

use anyhow::{anyhow, bail, Result};
use std::fs;
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

/// Quanti nomi si provano prima di arrendersi.
///
/// Non e' un tetto di comodo: se in un Cestino ci sono gia' mille file
/// chiamati `appunti.txt`, il problema non e' il nome — e' un Cestino che
/// non svuota nessuno da anni, e continuare a contare non aiuta.
pub const NOMI_DA_PROVARE: u32 = 1000;

/// I caratteri che in un `Path=` di `.trashinfo` non si proteggono.
///
/// La specifica dice «come in un URL», con la barra lasciata stare: senza
/// quell'eccezione il percorso diventerebbe illeggibile e nessun gestore di
/// file saprebbe piu' rimettere il file al suo posto.
fn si_lascia_stare(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.' | b'~')
}

/// Il percorso come lo vuole `.trashinfo`.
pub fn percorso_protetto(percorso: &str) -> String {
    let mut fuori = String::with_capacity(percorso.len());
    for b in percorso.as_bytes() {
        if si_lascia_stare(*b) {
            fuori.push(*b as char);
        } else {
            fuori.push_str(&format!("%{b:02X}"));
        }
    }
    fuori
}

/// Il contenuto del file `.trashinfo`.
///
/// La data e' locale e senza fuso, come dice la specifica. Non e' una scelta
/// nostra e non e' bella, ma un gestore di file che legge una data in un
/// formato suo non la mostra affatto.
pub fn informazione(percorso: &str, quando: &str) -> String {
    format!(
        "[Trash Info]\nPath={}\nDeletionDate={}\n",
        percorso_protetto(percorso),
        quando
    )
}

/// Il nome da provare al tentativo numero `n`.
///
/// `appunti.txt`, poi `appunti.1.txt`, `appunti.2.txt`: il numero va **prima**
/// dell'estensione, se no il file nel Cestino non si apre piu' con un doppio
/// clic — e uno che nel Cestino non si apre non si riconosce, e non si
/// recupera.
pub fn nome_numero(nome: &str, n: u32) -> String {
    if n == 0 {
        return nome.to_string();
    }
    match nome.rsplit_once('.') {
        // `.nascosto` non ha estensione: il punto e' il primo carattere.
        Some((base, est)) if !base.is_empty() => format!("{base}.{n}.{est}"),
        _ => format!("{nome}.{n}"),
    }
}

/// Dove sta il Cestino di casa.
pub fn cestino_di_casa(casa: &Path, xdg_data: Option<&Path>) -> PathBuf {
    match xdg_data {
        Some(d) if d.is_absolute() => d.join("Trash"),
        _ => casa.join(".local/share/Trash"),
    }
}

/// Su macOS il Cestino e' uno solo e sta li'.
pub fn cestino_mac(casa: &Path) -> PathBuf {
    casa.join(".Trash")
}

/// Se questo sistema tiene le informazioni di ripristino di fianco al file.
///
/// Su Linux si', ed e' la specifica freedesktop. Su macOS no: il Cestino e'
/// una cartella e basta, e «Rimetti a posto» si appoggia a un registro del
/// Finder che non e' nostro. Il file si recupera lo stesso — trascinandolo
/// fuori — e questo va detto a chi chiede, invece di lasciargli credere che
/// il tasto funzionera'.
pub const RIMETTE_A_POSTO_DA_SE: bool = cfg!(not(target_os = "macos"));

/// La frase per chi butta un file che sta su un altro disco.
pub fn niente_cestino_qui(percorso: &str) -> String {
    format!(
        "«{percorso}» sta su un disco che non ha un Cestino, e da qui non se ne \
         puo' aprire uno. Spostarlo vorrebbe dire copiarlo e poi cancellare \
         l'originale, e fra le due cose c'e' un momento in cui si perde tutto: \
         non lo faccio. Se vuoi cancellarlo davvero, dimmelo e lo cancello — \
         ma non si disfa."
    )
}

fn casa() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| anyhow!("non so dove sia la tua cartella personale: HOME non e' impostata"))
}

fn chi_sono() -> u32 {
    // Senza libc: il proprietario di `/proc/self` su Linux e' l'utente
    // effettivo, e su macOS ci pensa `HOME`, che appartiene a lui.
    if let Ok(m) = fs::metadata("/proc/self") {
        return m.uid();
    }
    casa()
        .ok()
        .and_then(|c| fs::metadata(c).ok())
        .map(|m| m.uid())
        .unwrap_or(0)
}

/// Il percorso assoluto senza seguire l'ultimo collegamento.
///
/// `canonicalize` lo seguirebbe, e buttare nel Cestino un collegamento
/// finirebbe per buttarci il file a cui punta: due cose molto diverse, e la
/// seconda e' quella che nessuno ha chiesto.
fn assoluto(percorso: &Path) -> Result<PathBuf> {
    let nome = percorso
        .file_name()
        .ok_or_else(|| anyhow!("«{}» non e' un nome di file", percorso.display()))?;
    let genitore = percorso.parent().filter(|p| !p.as_os_str().is_empty());
    let base = match genitore {
        Some(g) => g
            .canonicalize()
            .map_err(|e| anyhow!("non riesco a raggiungere «{}»: {e}", g.display()))?,
        None => std::env::current_dir()?,
    };
    Ok(base.join(nome))
}

/// Il cestino di primo livello del disco su cui sta questo file, se si puo'.
///
/// La specifica ne prevede due: `$radice/.Trash/$uid`, che deve gia' esserci
/// ed essere appiccicosa (lo sticky bit: chiunque scrive, nessuno cancella
/// la roba degli altri), e `$radice/.Trash-$uid`, che si puo' creare.
fn cestino_del_disco(radice: &Path) -> Option<PathBuf> {
    let uid = chi_sono();
    let comune = radice.join(".Trash");
    if let Ok(m) = fs::symlink_metadata(&comune) {
        // Un collegamento al posto della cartella comune e' il modo classico
        // di farsi spostare i file altrove: la specifica dice di rifiutarlo.
        let appiccicosa = m.mode() & 0o1000 != 0;
        if m.is_dir() && appiccicosa && !m.file_type().is_symlink() {
            let mio = comune.join(uid.to_string());
            if fs::create_dir_all(mio.join("files")).is_ok()
                && fs::create_dir_all(mio.join("info")).is_ok()
            {
                return Some(mio);
            }
        }
    }
    let mio = radice.join(format!(".Trash-{uid}"));
    if fs::create_dir_all(mio.join("files")).is_ok() && fs::create_dir_all(mio.join("info")).is_ok()
    {
        return Some(mio);
    }
    None
}

/// Dove comincia il disco su cui sta questo percorso.
fn radice_del_disco(percorso: &Path) -> Option<PathBuf> {
    let suo = fs::metadata(percorso).ok()?.dev();
    let mut qui = percorso.to_path_buf();
    loop {
        let sopra = qui.parent()?;
        match fs::metadata(sopra) {
            Ok(m) if m.dev() == suo => qui = sopra.to_path_buf(),
            // Appena il disco cambia, quello di prima era il punto d'innesto.
            _ => return Some(qui),
        }
    }
}

fn adesso() -> String {
    // Senza `chrono`: la data locale in formato ISO la sa gia' dire `date`,
    // e questo file lo legge un gestore di file, non un ciclo stretto.
    if let Ok(u) = std::process::Command::new("date")
        .args(["+%Y-%m-%dT%H:%M:%S"])
        .output()
    {
        if u.status.success() {
            let t = String::from_utf8_lossy(&u.stdout).trim().to_string();
            if !t.is_empty() {
                return t;
            }
        }
    }
    // Se non c'e' nemmeno `date`, meglio una data sbagliata che nessun
    // Cestino: il campo e' informativo, il recupero dipende da `Path=`.
    "1970-01-01T00:00:00".to_string()
}

/// Mette il file nel cestino indicato, secondo la specifica freedesktop.
fn dentro_un_cestino(cestino: &Path, da: &Path) -> Result<PathBuf> {
    let nome = da
        .file_name()
        .ok_or_else(|| anyhow!("«{}» non ha un nome", da.display()))?
        .to_string_lossy()
        .to_string();
    fs::create_dir_all(cestino.join("files"))?;
    fs::create_dir_all(cestino.join("info"))?;
    let quando = adesso();
    for n in 0..NOMI_DA_PROVARE {
        let tentativo = nome_numero(&nome, n);
        let info = cestino.join("info").join(format!("{tentativo}.trashinfo"));
        // `create_new`: e' questo che prenota il nome. Se un altro programma
        // sta buttando nel Cestino un file che si chiama uguale, uno dei due
        // fallisce qui invece che sovrascrivere il file dell'altro.
        let mut f = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&info)
        {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => bail!("non riesco a scrivere nel Cestino: {e}"),
        };
        f.write_all(informazione(&da.to_string_lossy(), &quando).as_bytes())?;
        drop(f);
        let dove = cestino.join("files").join(&tentativo);
        match fs::rename(da, &dove) {
            Ok(()) => return Ok(dove),
            Err(e) => {
                // Il nome prenotato si libera: lasciarlo li' farebbe crescere
                // `info/` di file che non descrivono niente, e un Cestino che
                // si riempie di schede senza file confonde chi lo apre.
                let _ = fs::remove_file(&info);
                return Err(anyhow::Error::new(e));
            }
        }
    }
    bail!("nel Cestino ci sono gia' {NOMI_DA_PROVARE} file chiamati «{nome}»")
}

/// Manda un file o una cartella nel Cestino.
pub fn butta(percorso: &str) -> Result<()> {
    let da = assoluto(Path::new(percorso))?;
    if fs::symlink_metadata(&da).is_err() {
        bail!("«{percorso}» non c'e'");
    }
    let casa = casa()?;

    if cfg!(target_os = "macos") {
        let cestino = cestino_mac(&casa);
        fs::create_dir_all(&cestino)?;
        let nome = da
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        for n in 0..NOMI_DA_PROVARE {
            let dove = cestino.join(nome_numero(&nome, n));
            if dove.exists() {
                continue;
            }
            return match fs::rename(&da, &dove) {
                Ok(()) => Ok(()),
                Err(e) if e.raw_os_error() == Some(18) => bail!("{}", niente_cestino_qui(percorso)),
                Err(e) => Err(anyhow!("«{percorso}» non si riesce a cestinare: {e}")),
            };
        }
        bail!("nel Cestino ci sono gia' {NOMI_DA_PROVARE} file chiamati «{nome}»");
    }

    let cestino = cestino_di_casa(
        &casa,
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .as_deref(),
    );
    match dentro_un_cestino(&cestino, &da) {
        Ok(_) => return Ok(()),
        Err(e) => {
            // EXDEV: il file sta su un altro disco. Non e' un guasto, e' la
            // condizione che la specifica prevede — e l'unica in cui si
            // cerca un cestino sull'altro disco.
            let altro_disco = e
                .downcast_ref::<std::io::Error>()
                .and_then(|x| x.raw_os_error())
                == Some(18);
            if !altro_disco {
                return Err(e);
            }
        }
    }
    let radice =
        radice_del_disco(&da).ok_or_else(|| anyhow!("{}", niente_cestino_qui(percorso)))?;
    let suo =
        cestino_del_disco(&radice).ok_or_else(|| anyhow!("{}", niente_cestino_qui(percorso)))?;
    dentro_un_cestino(&suo, &da)
        .map(|_| ())
        .map_err(|e| anyhow!("«{percorso}» non si riesce a cestinare: {e}"))
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn il_percorso_si_protegge_come_vuole_la_specifica() {
        // La barra resta: senza, nessun gestore di file saprebbe rimettere
        // il file al suo posto.
        assert_eq!(percorso_protetto("/casa/note.txt"), "/casa/note.txt");
        assert_eq!(
            percorso_protetto("/casa/L'anno scorso"),
            "/casa/L%27anno%20scorso"
        );
        assert_eq!(percorso_protetto("/casa/citt\u{e0}"), "/casa/citt%C3%A0");
        assert_eq!(percorso_protetto("/a/b%c"), "/a/b%25c");
        assert_eq!(percorso_protetto("/a/~b-c_d.e"), "/a/~b-c_d.e");
    }

    #[test]
    fn linformazione_ha_le_tre_righe_che_servono() {
        let i = informazione("/casa/L'anno scorso", "2026-09-20T10:00:00");
        assert!(i.starts_with("[Trash Info]\n"), "{i}");
        assert!(i.contains("Path=/casa/L%27anno%20scorso\n"), "{i}");
        assert!(i.ends_with("DeletionDate=2026-09-20T10:00:00\n"), "{i}");
    }

    #[test]
    fn il_numero_va_prima_dellestensione() {
        // `appunti.1.txt` e non `appunti.txt.1`: uno che nel Cestino non si
        // apre con un doppio clic non si riconosce, e non si recupera.
        assert_eq!(nome_numero("appunti.txt", 0), "appunti.txt");
        assert_eq!(nome_numero("appunti.txt", 1), "appunti.1.txt");
        assert_eq!(nome_numero("archivio.tar.gz", 2), "archivio.tar.2.gz");
        assert_eq!(nome_numero("senzaestensione", 3), "senzaestensione.3");
        // Un file nascosto non ha estensione: il punto e' il primo carattere.
        assert_eq!(nome_numero(".bashrc", 1), ".bashrc.1");
        assert_eq!(nome_numero("cartella", 0), "cartella");
    }

    #[test]
    fn il_cestino_di_casa_ascolta_xdg_ma_non_a_qualunque_prezzo() {
        let casa = Path::new("/casa/gio");
        assert_eq!(
            cestino_di_casa(casa, None),
            PathBuf::from("/casa/gio/.local/share/Trash")
        );
        assert_eq!(
            cestino_di_casa(casa, Some(Path::new("/dati"))),
            PathBuf::from("/dati/Trash")
        );
        // Un XDG_DATA_HOME relativo la specifica dice di ignorarlo: seguirlo
        // vorrebbe dire un Cestino che cambia posto con la cartella corrente.
        assert_eq!(
            cestino_di_casa(casa, Some(Path::new("dati"))),
            PathBuf::from("/casa/gio/.local/share/Trash")
        );
    }

    #[test]
    fn chi_non_puo_cestinare_lo_sente_dire_con_cosa_succederebbe() {
        let m = niente_cestino_qui("/media/chiavetta/foto.jpg");
        assert!(m.contains("/media/chiavetta/foto.jpg"), "{m}");
        assert!(m.contains("non lo faccio"), "{m}");
        assert!(m.contains("non si disfa"), "{m}");
    }

    // ---------------------------------------------------------- sul disco

    fn banco(chi: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("nova-cestino-{}-{chi}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn un_file_buttato_e_ancora_li_dentro() {
        let d = banco("uno");
        let cestino = d.join("Cestino");
        let f = d.join("note.txt");
        fs::write(&f, "ciao").unwrap();
        let dove = dentro_un_cestino(&cestino, &f).unwrap();
        assert!(!f.exists(), "l'originale non c'e' piu'");
        assert_eq!(
            fs::read_to_string(&dove).unwrap(),
            "ciao",
            "il file c'e' tutto"
        );
        let info = cestino.join("info/note.txt.trashinfo");
        let testo = fs::read_to_string(&info).unwrap();
        assert!(
            testo.contains(&percorso_protetto(&f.to_string_lossy())),
            "{testo}"
        );
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn due_file_con_lo_stesso_nome_non_si_sovrascrivono() {
        let d = banco("due");
        let cestino = d.join("Cestino");
        for (dove, cosa) in [("a", "primo"), ("b", "secondo")] {
            let sotto = d.join(dove);
            fs::create_dir_all(&sotto).unwrap();
            let f = sotto.join("note.txt");
            fs::write(&f, cosa).unwrap();
            dentro_un_cestino(&cestino, &f).unwrap();
        }
        let dentro: Vec<String> = fs::read_dir(cestino.join("files"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(dentro.len(), 2, "{dentro:?}");
        assert!(dentro.contains(&"note.txt".to_string()), "{dentro:?}");
        assert!(dentro.contains(&"note.1.txt".to_string()), "{dentro:?}");
        // E le due schede dicono due posti diversi: e' cosi' che si sa
        // quale rimettere dove.
        let uno = fs::read_to_string(cestino.join("info/note.txt.trashinfo")).unwrap();
        let due = fs::read_to_string(cestino.join("info/note.1.txt.trashinfo")).unwrap();
        assert_ne!(uno, due);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn anche_una_cartella_piena_ci_va_intera() {
        let d = banco("cartella");
        let cestino = d.join("Cestino");
        let c = d.join("progetto");
        fs::create_dir_all(c.join("dentro")).unwrap();
        fs::write(c.join("dentro/x.txt"), "roba").unwrap();
        let dove = dentro_un_cestino(&cestino, &c).unwrap();
        assert_eq!(
            fs::read_to_string(dove.join("dentro/x.txt")).unwrap(),
            "roba"
        );
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn un_nome_con_un_apostrofo_dentro_non_e_un_problema() {
        // E' il difetto che la meta' Windows racconta (D130): li' il
        // percorso finiva dentro una stringa PowerShell. Qui non si compone
        // nessuna stringa, e questa prova serve a tenerlo vero.
        let d = banco("apostrofo");
        let cestino = d.join("Cestino");
        let f = d.join("L'anno scorso & altro.txt");
        fs::write(&f, "x").unwrap();
        let dove = dentro_un_cestino(&cestino, &f).unwrap();
        assert!(dove.exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn se_il_file_non_si_sposta_non_resta_una_scheda_orfana() {
        let d = banco("orfana");
        let cestino = d.join("Cestino");
        let e = dentro_un_cestino(&cestino, &d.join("non-ci-sono.txt")).unwrap_err();
        assert!(e.to_string().len() > 5, "{e}");
        let schede: Vec<_> = fs::read_dir(cestino.join("info")).unwrap().collect();
        assert!(
            schede.is_empty(),
            "una scheda senza file confonde chi apre il Cestino"
        );
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn un_collegamento_si_butta_per_quello_che_e() {
        // `canonicalize` seguirebbe il collegamento e finirebbe per buttare
        // nel Cestino il file a cui punta: due cose molto diverse, e la
        // seconda non l'ha chiesta nessuno.
        let d = banco("collegamento");
        let vero = d.join("vero.txt");
        fs::write(&vero, "importante").unwrap();
        let link = d.join("scorciatoia.txt");
        std::os::unix::fs::symlink(&vero, &link).unwrap();
        let visto = assoluto(&link).unwrap();
        assert!(visto.ends_with("scorciatoia.txt"), "{}", visto.display());
        let _ = fs::remove_dir_all(&d);
    }
}
