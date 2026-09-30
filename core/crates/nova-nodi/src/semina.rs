//! La prima mappatura del PC: i nodi che NOVA scrive da sola la prima volta.
//!
//! E' il gemello di `nova/kb/seed.py`, con le scelte fatte con Gio per il
//! demone (D365). Si semina il profilo, le preferenze, l'ambiente, le
//! applicazioni e i progetti. **Le persone no**: il Python le deduceva dai
//! commit dei repository, e cosi' scriveva nel vault nomi ed email di chi
//! lavora con l'utente, gente che a NOVA non ha mai detto niente. E il
//! profilo tiene il nome git ma non l'email, per la stessa ragione rivolta
//! all'utente: un indirizzo non serve a lavorare, e nel vault resta per
//! sempre.
//!
//! Qui ci sono solo le regole: cosa guardare, cosa saltare, come si scrive
//! ogni nodo. Il disco, git e le informazioni di sistema li mette il demone,
//! in `nova_core::semina`, e i nodi entrano nel vault dall'unica porta,
//! [`crate::deposito::Deposito::salva`], col guardiano dei segreti davanti.

use crate::{slug, Nodo, ORIGINE_SCANSIONE};

/// Le cartelle della casa dell'utente dove si cercano progetti.
pub const CARTELLE_PROGETTI: [&str; 7] = [
    "Desktop", "Documents", "Documenti", "progettoX", "source", "repos", "dev",
];

/// Le cartelle in cui non si entra: dipendenze, costruzioni, dati di sistema.
/// Anche quelle che cominciano col punto, che non stanno qui (vedi
/// [`da_saltare`]).
pub const IGNORA: [&str; 9] = [
    "node_modules", "__pycache__", ".venv", "venv", "dist", "build", ".git", "AppData",
    "OneDrive",
];

/// I file che fanno di una cartella un progetto, anche senza `.git`.
pub const MARCATORI: [&str; 7] = [
    "package.json", "pyproject.toml", "requirements.txt", "Cargo.toml", "go.mod",
    "README.md", "index.html",
];

/// Dove si cerca la riga che dice cos'e' un progetto, in quest'ordine.
pub const LEGGIMI: [&str; 3] = ["README.md", "readme.md", "README.MD"];

/// Il marcatore, e l'etichetta che ne viene.
pub const TAG_DEI_MARCATORI: [(&str, &str); 5] = [
    ("package.json", "node"),
    ("pyproject.toml", "python"),
    ("requirements.txt", "python"),
    ("Cargo.toml", "rust"),
    ("go.mod", "go"),
];

/// Le applicazioni che non contano per l'automazione: pezzi di sistema,
/// aggiornamenti, librerie. Si confrontano in minuscolo, come pezzi di nome.
pub const RUMORE_APP: [&str; 8] = [
    "redistributable", "runtime", "update for", "driver", "sdk", "microsoft visual c++",
    "hotfix", "language pack",
];

/// Quanti progetti al massimo: oltre, la mappa smette di essere una mappa.
pub const MAX_PROGETTI: usize = 40;

/// Quanti livelli sotto ogni cartella di [`CARTELLE_PROGETTI`].
pub const PROFONDITA_PROGETTI: usize = 2;

/// Quante applicazioni al massimo nel nodo.
pub const MAX_APP: usize = 120;

/// Dove sta il segno che la semina e' gia' stata fatta, dentro il vault.
/// E' lo stesso file del Python: chi ha seminato con l'uno non risemina con
/// l'altro.
pub const MARCATORE: &str = ".nova/seed.json";

/// Una cartella in cui non si entra.
pub fn da_saltare(nome: &str) -> bool {
    IGNORA.contains(&nome) || nome.starts_with('.')
}

/// La prima riga che dice qualcosa, in un README.
///
/// E' `_prima_riga_readme` del Python: le righe come le separa Python, senza
/// i bianchi e senza i `#` del titolo in testa, e la prima piu' lunga di
/// dodici caratteri, tagliata a duecento. Le righe corte sono quasi sempre
/// il nome del progetto, che nel nodo c'e' gia'.
pub fn prima_riga_readme(testo: &str) -> String {
    for riga in nova_pitone::righe(testo) {
        let riga = nova_pitone::senza_bianchi(nova_pitone::senza_bianchi(&riga).trim_start_matches('#'));
        if riga.chars().count() > 12 {
            return riga.chars().take(200).collect();
        }
    }
    String::new()
}

/// Un progetto trovato sul disco, com'e' prima di diventare un nodo.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Progetto {
    pub percorso: String,
    pub nome: String,
    pub git: bool,
    pub remote: String,
    pub marcatori: Vec<String>,
    pub readme: String,
}

/// Il nodo di un progetto: `nodo_progetto` del Python.
pub fn nodo_progetto(p: &Progetto) -> Nodo {
    let mut corpo = vec![format!("- **Cartella**: `{}`", p.percorso)];
    if !p.remote.is_empty() {
        corpo.push(format!("- **Repository**: {}", p.remote));
    }
    if !p.marcatori.is_empty() {
        corpo.push(format!("- **Stack rilevato**: {}", p.marcatori.join(", ")));
    }
    if !p.readme.is_empty() {
        corpo.push(format!("\n{}", p.readme));
    }
    let mut tags = vec!["progetto".to_string()];
    if p.git {
        tags.push("git".into());
    }
    for (m, t) in TAG_DEI_MARCATORI {
        if p.marcatori.iter().any(|x| x == m) && !tags.iter().any(|x| x == t) {
            tags.push(t.to_string());
        }
    }
    tags.truncate(4);
    let mut riferimenti = vec![p.percorso.clone()];
    if !p.remote.is_empty() {
        riferimenti.push(p.remote.clone());
    }
    Nodo {
        slug: slug(&format!("progetto {}", p.nome)),
        title: p.nome.clone(),
        body: corpo.join("\n"),
        tipo: "progetto".into(),
        tags,
        relazioni: vec!["profilo-utente".into()],
        riferimenti,
        origine: ORIGINE_SCANSIONE.into(),
        confidenza: 0.9,
        ..Nodo::default()
    }
}

/// L'indirizzo di un repository senza chi e con che chiave ci si entra.
///
/// `https://anna:ghp_...@github.com/anna/x.git` diventa
/// `https://github.com/anna/x.git`. Il Python lo scriveva com'era: il
/// guardiano dei segreti avrebbe rifiutato il nodo, e la semina si fermava
/// li'. Qui il nodo entra, e la chiave no. Gli indirizzi come
/// `git@github.com:anna/x.git` non hanno una chiave dentro, e restano.
pub fn remote_senza_credenziali(url: &str) -> String {
    let Some((schema, resto)) = url.split_once("://") else {
        return url.to_string();
    };
    let fine_host = resto.find(['/', '?', '#']).unwrap_or(resto.len());
    match resto[..fine_host].rfind('@') {
        Some(chiocciola) => format!("{schema}://{}", &resto[chiocciola + 1..]),
        None => url.to_string(),
    }
}

/// Un nome che l'installatore non ha mai riempito, come `${{arpDisplayName}}`.
/// E' `_segnaposto` del Python: non e' un'applicazione, e nel vault
/// diventerebbe un ricordo sbagliato a cui il modello crede.
pub fn segnaposto(nome: &str) -> bool {
    nome.contains("${") || nome.contains("{{") || nome.starts_with("@{")
}

/// Le applicazioni che contano, nell'ordine in cui arrivano.
pub fn app_rilevanti(installate: &[String]) -> Vec<String> {
    installate
        .iter()
        .filter(|n| {
            let basso = n.to_lowercase();
            !RUMORE_APP.iter().any(|r| basso.contains(r)) && !segnaposto(n)
        })
        .cloned()
        .collect()
}

/// Il nodo delle applicazioni, se ce n'e' qualcuna che conta.
pub fn nodo_app(installate: &[String]) -> Option<Nodo> {
    let nomi = app_rilevanti(installate);
    if nomi.is_empty() {
        return None;
    }
    let mut corpo = vec![
        "Applicazioni installate rilevanti (rilevate dal registro):".to_string(),
        String::new(),
    ];
    corpo.extend(nomi.iter().take(MAX_APP).map(|n| format!("- {n}")));
    Some(Nodo {
        slug: "app-installate".into(),
        title: "Applicazioni installate".into(),
        body: corpo.join("\n"),
        tipo: "app".into(),
        tags: vec!["app".into(), "software".into()],
        relazioni: vec!["ambiente-tecnico".into()],
        origine: ORIGINE_SCANSIONE.into(),
        confidenza: 0.85,
        ..Nodo::default()
    })
}

/// Chi e' l'utente, per il nodo del profilo.
#[derive(Debug, Clone, Default)]
pub struct Profilo {
    pub utente: String,
    pub pc: String,
    pub casa: String,
    pub sistema: String,
    /// `git config --global user.name`, vuoto se non c'e'.
    pub nome_git: String,
}

/// Il nodo del profilo.
///
/// Diverso dal Python in due punti, e apposta (D365): non c'e' l'email di
/// git, e l'utente non e' «Utente Windows», perche' il demone gira anche su
/// Mac e Linux.
pub fn nodo_profilo(p: &Profilo) -> Nodo {
    let mut corpo = vec![
        format!("- **Utente**: `{}` su `{}`", p.utente, p.pc),
        format!("- **Cartella home**: `{}`", p.casa),
    ];
    if !p.sistema.is_empty() {
        corpo.push(format!("- **Sistema**: {}", p.sistema));
    }
    if !p.nome_git.is_empty() {
        corpo.push(format!("- **Identita' git**: {}", p.nome_git));
    }
    corpo.push(String::new());
    corpo.push("Vedi anche [[preferenze-di-lavoro]] e [[ambiente-tecnico]].".into());
    let chi = if p.nome_git.is_empty() { &p.utente } else { &p.nome_git };
    Nodo {
        slug: "profilo-utente".into(),
        title: format!("Profilo di {chi}"),
        body: corpo.join("\n"),
        tipo: "profilo".into(),
        tags: vec!["profilo".into(), "identita".into()],
        relazioni: vec!["preferenze-di-lavoro".into(), "ambiente-tecnico".into()],
        origine: ORIGINE_SCANSIONE.into(),
        confidenza: 0.95,
        ..Nodo::default()
    }
}

/// Il nodo delle preferenze. La lingua e' quella della configurazione,
/// scritta per nome (`italiano`, `inglese`): il Python scriveva sempre
/// «italiano», anche a chi aveva scelto un'altra lingua.
pub fn nodo_preferenze(lingua: &str) -> Nodo {
    Nodo {
        slug: "preferenze-di-lavoro".into(),
        title: "Preferenze di lavoro".into(),
        body: format!(
            "Come NOVA deve comportarsi. Questo nodo si arricchisce da solo \
             man mano che emergono preferenze nelle conversazioni.\n\n\
             - **Lingua**: {lingua}\n\
             - **Stile risposte**: brevi e concrete, niente preamboli\n\
             - **Azioni**: eseguire invece di spiegare come si farebbe\n"
        ),
        tipo: "preferenza".into(),
        tags: vec!["preferenze".into(), "stile".into()],
        relazioni: vec!["profilo-utente".into()],
        origine: ORIGINE_SCANSIONE.into(),
        confidenza: 0.8,
        ..Nodo::default()
    }
}

/// Com'e' fatto il PC, per il nodo dell'ambiente.
#[derive(Debug, Clone, Default)]
pub struct Ambiente {
    /// Vuoto se le informazioni di sistema non si sono potute leggere.
    pub sistema: String,
    pub build: u32,
    pub cpu: String,
    pub gpu: String,
    pub ram_byte: u64,
    /// `server.model_path` e `server.binary` della configurazione.
    pub modello: String,
    pub runtime: String,
}

/// Il nodo dell'ambiente: `nodo_ambiente` del Python senza la riga della
/// versione di Python, che nel demone non c'e' (D365).
///
/// La build si scrive solo se c'e': fuori da Windows le informazioni di
/// sistema la danno a zero, e «Ubuntu 24.04 (build 0)» dice una cosa falsa.
pub fn nodo_ambiente(a: &Ambiente) -> Nodo {
    let mut corpo = Vec::new();
    if !a.sistema.is_empty() {
        corpo.push(if a.build == 0 {
            format!("- **Sistema**: {}", a.sistema)
        } else {
            format!("- **Sistema**: {} (build {})", a.sistema, a.build)
        });
    }
    corpo.push(format!("- **CPU**: {}", if a.cpu.is_empty() { "n/d" } else { &a.cpu }));
    corpo.push(format!("- **GPU**: {}", if a.gpu.is_empty() { "n/d" } else { &a.gpu }));
    corpo.push(format!(
        "- **RAM**: {}",
        if a.ram_byte == 0 { "?".to_string() } else { nova_dati::pesa(a.ram_byte) }
    ));
    if !a.modello.is_empty() {
        corpo.push(format!("- **Modello locale**: `{}`", a.modello));
    }
    if !a.runtime.is_empty() {
        corpo.push(format!("- **Runtime llama.cpp**: `{}`", a.runtime));
    }
    Nodo {
        slug: "ambiente-tecnico".into(),
        title: "Ambiente tecnico del PC".into(),
        body: corpo.join("\n"),
        tipo: "app".into(),
        tags: vec!["hardware".into(), "ambiente".into()],
        relazioni: vec!["profilo-utente".into()],
        origine: ORIGINE_SCANSIONE.into(),
        confidenza: 0.95,
        ..Nodo::default()
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn un_progetto_diventa_il_suo_nodo() {
        let p = Progetto {
            percorso: "C:/Users/utente/Documents/NOVA".into(),
            nome: "NOVA".into(),
            git: true,
            remote: "https://github.com/anna/NOVA.git".into(),
            marcatori: vec!["requirements.txt".into(), "README.md".into()],
            readme: "L'assistente che vive nel PC".into(),
        };
        let n = nodo_progetto(&p);
        assert_eq!(n.slug, "progetto-nova");
        assert_eq!(n.title, "NOVA");
        assert_eq!(
            n.body,
            "- **Cartella**: `C:/Users/utente/Documents/NOVA`\n\
             - **Repository**: https://github.com/anna/NOVA.git\n\
             - **Stack rilevato**: requirements.txt, README.md\n\
             \nL'assistente che vive nel PC"
        );
        assert_eq!(n.tags, ["progetto", "git", "python"]);
        assert_eq!(n.riferimenti, ["C:/Users/utente/Documents/NOVA", "https://github.com/anna/NOVA.git"]);
        assert_eq!(n.tipo, "progetto");
        assert_eq!(n.origine, "scansione");
    }

    #[test]
    fn le_etichette_sono_al_massimo_quattro() {
        let p = Progetto {
            nome: "tutto".into(),
            git: true,
            marcatori: MARCATORI.iter().map(|m| m.to_string()).collect(),
            ..Progetto::default()
        };
        assert_eq!(nodo_progetto(&p).tags, ["progetto", "git", "node", "python"]);
    }

    #[test]
    fn la_riga_del_readme_salta_titoli_e_righe_corte() {
        assert_eq!(prima_riga_readme("# NOVA\n\n## L'assistente del PC, in locale\n"),
                   "L'assistente del PC, in locale");
        assert_eq!(prima_riga_readme("corta\r\naltra corta"), "");
        assert_eq!(prima_riga_readme(&"x".repeat(300)).chars().count(), 200);
    }

    #[test]
    fn dal_remote_si_toglie_la_chiave() {
        assert_eq!(remote_senza_credenziali("https://anna:ghp_abc@github.com/anna/x.git"),
                   "https://github.com/anna/x.git");
        assert_eq!(remote_senza_credenziali("https://anna@bitbucket.org/anna/x.git"),
                   "https://bitbucket.org/anna/x.git");
        assert_eq!(remote_senza_credenziali("git@github.com:anna/x.git"), "git@github.com:anna/x.git");
        // Una chiocciola dopo l'host non e' una credenziale.
        assert_eq!(remote_senza_credenziali("https://example.com/a@b"), "https://example.com/a@b");
    }

    #[test]
    fn le_app_di_sistema_e_i_segnaposto_restano_fuori() {
        let installate: Vec<String> = [
            "Microsoft Visual C++ 2015 Redistributable", "Steam", "${{arpDisplayName}}",
            "NVIDIA Graphics Driver", "Visual Studio Code",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let n = nodo_app(&installate).unwrap();
        assert_eq!(n.body, "Applicazioni installate rilevanti (rilevate dal registro):\n\n\
                            - Steam\n- Visual Studio Code");
        assert!(nodo_app(&["Windows SDK".to_string()]).is_none());
    }

    #[test]
    fn il_profilo_non_tiene_l_email() {
        let n = nodo_profilo(&Profilo {
            utente: "anna".into(),
            pc: "portatile".into(),
            casa: "C:\\Users\\utente".into(),
            sistema: "Windows 11 Pro".into(),
            nome_git: "Anna Prova".into(),
        });
        assert_eq!(n.title, "Profilo di Anna Prova");
        assert_eq!(
            n.body,
            "- **Utente**: `anna` su `portatile`\n\
             - **Cartella home**: `C:\\Users\\utente`\n\
             - **Sistema**: Windows 11 Pro\n\
             - **Identita' git**: Anna Prova\n\
             \n\
             Vedi anche [[preferenze-di-lavoro]] e [[ambiente-tecnico]]."
        );
        assert!(!n.body.contains('@'));
        let senza_git = nodo_profilo(&Profilo { utente: "anna".into(), ..Profilo::default() });
        assert_eq!(senza_git.title, "Profilo di anna");
    }

    #[test]
    fn l_ambiente_dice_n_d_quando_non_sa() {
        let n = nodo_ambiente(&Ambiente::default());
        assert_eq!(n.body, "- **CPU**: n/d\n- **GPU**: n/d\n- **RAM**: ?");
    }

    #[test]
    fn la_build_si_scrive_solo_se_c_e() {
        let windows = Ambiente { sistema: "Windows 11 Pro".into(), build: 26100, ..Ambiente::default() };
        assert!(nodo_ambiente(&windows).body.starts_with("- **Sistema**: Windows 11 Pro (build 26100)\n"));
        let linux = Ambiente { sistema: "Ubuntu 24.04.4 LTS".into(), ..Ambiente::default() };
        assert!(nodo_ambiente(&linux).body.starts_with("- **Sistema**: Ubuntu 24.04.4 LTS\n"));
    }

    #[test]
    fn si_salta_quel_che_non_e_un_progetto() {
        assert!(da_saltare("node_modules"));
        assert!(da_saltare(".cache"));
        assert!(!da_saltare("NOVA"));
    }
}
