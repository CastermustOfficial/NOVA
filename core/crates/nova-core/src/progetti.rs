//! Dove stanno i progetti dei Dot (D407).
//!
//! Un progetto sta in `Documenti\Dots_Project\<nome>\`, o nella cartella
//! che ha indicato chi l'ha chiesto, con la struttura di
//! `nova_dot::struttura`. Nella cartella di NOVA resta solo l'indice: il
//! nome e la cartella di ognuno. Senza l'indice, un progetto in una
//! cartella scelta dall'utente non si ritroverebbe.
//!
//! Qui si guarda il disco: dov'e' Documenti, cosa dice l'indice, e lo
//! spostamento dei progetti di prima del D407.

use std::path::{Path, PathBuf};

use nova_dot::progetto as pr;
use nova_dot::struttura as st;

fn nova() -> PathBuf {
    crate::mondo::cartella_nova()
}

fn casa() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_default()
}

/// La cartella Documenti dell'utente: `Documents` (su Windows e' il nome
/// vero anche quando Esplora la mostra come «Documenti»), o `Documenti` se
/// c'e' solo quella. Se non c'e' nessuna delle due si usa `Documents`, che
/// nasce col primo progetto.
pub fn documenti() -> PathBuf {
    let c = casa();
    let inglese = c.join("Documents");
    if inglese.is_dir() {
        return inglese;
    }
    let italiano = c.join("Documenti");
    if italiano.is_dir() {
        return italiano;
    }
    inglese
}

/// La cartella di un progetto: quella dell'indice, o quella di serie.
pub fn radice(nome: &str) -> PathBuf {
    match st::progetti(&nova()).get(nome) {
        Some(r) => PathBuf::from(r),
        None => st::radice_predefinita(&documenti(), nome),
    }
}

/// Se il progetto e' gia' nell'indice.
pub fn conosciuto(nome: &str) -> bool {
    st::progetti(&nova()).contains_key(nome)
}

/// La cartella di un progetto, pronta: nell'indice, con le cartelle e i
/// file fissi. `cartella` vuota vuol dire quella di serie, in Documenti; un
/// progetto gia' nell'indice tiene la sua.
pub fn assicura(nome: &str, cartella: &str) -> Result<PathBuf, String> {
    let r = if conosciuto(nome) {
        radice(nome)
    } else if cartella.trim().is_empty() {
        st::radice_predefinita(&documenti(), nome)
    } else {
        PathBuf::from(cartella.trim())
    };
    // Una cartella, un progetto: due progetti nella stessa cartella si
    // scriverebbero sopra la cronologia e i piani.
    if let Some((altro, _)) = st::progetti(&nova())
        .into_iter()
        .find(|(n, x)| n != nome && Path::new(x) == r.as_path())
    {
        return Err(format!(
            "in «{}» c'e' gia' il progetto «{altro}»: ognuno ha la sua cartella",
            r.display()
        ));
    }
    st::prepara(&r, nome)?;
    st::registra(&nova(), nome, &r)?;
    Ok(r)
}

/// I progetti, in ordine di nome.
pub fn elenco() -> Vec<String> {
    pr::elenco(&nova())
}

/// Sposta i progetti di prima del D407 dalla cartella di NOVA alla loro:
/// la cronologia, il racconto e i piani; poi la struttura. Un progetto la
/// cui cartella nuova ha gia' una cronologia resta dov'era, e lo si dice
/// nel log: non si sovrascrive niente.
pub fn sposta_i_vecchi() {
    let nova = nova();
    for nome in pr::vecchi(&nova) {
        let da = pr::cartella_vecchia(&nova, &nome);
        let cartella = pr::Progetto::da(&nome, &pr::leggi(&da))
            .map(|p| p.cartella)
            .filter(|c| Path::new(c).is_dir())
            .unwrap_or_default();
        let a = if cartella.is_empty() {
            st::radice_predefinita(&documenti(), &nome)
        } else {
            PathBuf::from(cartella)
        };
        match sposta(&da, &a, &nome) {
            Ok(()) => tracing::info!(progetto = %nome, a = %a.display(), "progetto spostato (D407)"),
            Err(e) => tracing::warn!(progetto = %nome, errore = %e, "il progetto resta dov'era"),
        }
    }
    // La cartella di prima, se e' rimasta vuota: `remove_dir` non toglie una
    // cartella che ha ancora qualcosa dentro.
    let _ = std::fs::remove_dir(nova.join(pr::CARTELLA));
}

fn sposta(da: &Path, a: &Path, nome: &str) -> Result<(), String> {
    if a.join("progetto.jsonl").exists() {
        return Err(format!("in {} c'e' gia' una cronologia", a.display()));
    }
    st::prepara(a, nome)?;
    let copia = |x: &Path, y: &Path| {
        std::fs::copy(x, y)
            .map(|_| ())
            .map_err(|e| format!("{} -> {}: {e}", x.display(), y.display()))
    };
    for f in ["progetto.jsonl", "progetto.md"] {
        if da.join(f).is_file() {
            copia(&da.join(f), &a.join(f))?;
        }
    }
    let piani = da.join(pr::CARTELLA_PIANI_VECCHIA);
    if let Ok(it) = std::fs::read_dir(&piani) {
        for e in it.flatten() {
            if e.path().is_file() {
                copia(&e.path(), &a.join(st::PIANO).join(e.file_name()))?;
            }
        }
    }
    st::registra(&crate::mondo::cartella_nova(), nome, a)?;
    // Copiato tutto e registrato: solo adesso si toglie la cartella vecchia.
    std::fs::remove_dir_all(da).map_err(|e| format!("{}: {e}", da.display()))
}
