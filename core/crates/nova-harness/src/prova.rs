//! Il verificatore: si applica solo se i test restano verdi.
//!
//! E' il pezzo che trasforma l'harness da un buon posto per leggere a un
//! posto dove si puo' programmare. Senza, NOVA propone una modifica al codice
//! e l'utente deve fidarsi: leggere il diff e decidere. Va bene per tre
//! righe, non va bene per un file che non si conosce.
//!
//! Due decisioni, perche' la versione ingenua di questa cosa non funziona.
//!
//! **Verde dopo non basta.** Se i test erano gia' rossi prima, «verde dopo»
//! e' irraggiungibile e «rosso dopo» non dice niente: si starebbe rifiutando
//! una modifica buona per colpa di un guasto che c'era gia'. Quel che conta
//! e' il confronto — quali prove passavano prima, quali passano adesso
//! (D277).
//!
//! **Come si prova un progetto non si indovina, si riconosce.** Un
//! `Cargo.toml` dice `cargo test`; un `package.json` con uno script `test`
//! dice `npm test`; pytest si usa **solo se il progetto lo dichiara**, perche'
//! eseguirlo dove non c'e' vuol dire raccogliere file che non erano pensati
//! per lui. E quando non c'e' niente di tutto questo ma ci sono dei
//! `test_*.py` che finiscono con `sys.exit`, sono script che si eseguono e
//! basta — e' la convenzione di NOVA stessa, e vale per chiunque scriva i
//! test cosi'.

use std::path::{Path, PathBuf};

/// Oltre questo non si aspetta piu': una suite che non finisce e' un guasto
/// suo, non un motivo per lasciare l'utente fermo.
pub const ATTESA_PROVE_S: u64 = 300;

/// Quanto output si tiene di una prova caduta. La **coda**, non la testa:
/// l'errore sta in fondo.
pub const CODA_RIGHE: usize = 40;

/// Il codice di uscita che vuol dire «qui non si puo' provare».
///
/// Serve il demone, serve un browser, serve una macchina vera. Non e' un
/// fallimento, e chiamarlo rosso insegnerebbe a ignorare i rossi.
pub const NON_PROVABILE_QUI: i32 = 2;

/// Un modo di provare un progetto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Banco {
    pub nome: String,
    pub comando: Vec<String>,
    pub dove: PathBuf,
    /// Alcune suite si eseguono un file per volta (la convenzione di NOVA):
    /// li' `pezzi` sono i file, e il comando si ripete su ognuno.
    pub pezzi: Vec<String>,
}

impl Banco {
    pub fn nuovo(nome: &str, comando: &[&str], dove: &Path) -> Banco {
        Banco {
            nome: nome.to_string(),
            comando: comando.iter().map(|x| x.to_string()).collect(),
            dove: dove.to_path_buf(),
            pezzi: Vec::new(),
        }
    }

    pub fn descrizione(&self) -> String {
        let base = self.comando.join(" ");
        if self.pezzi.is_empty() {
            base
        } else {
            format!("{base} ({} file)", self.pezzi.len())
        }
    }

    /// La prima parola del nome: `cargo (core)` e' pur sempre `cargo`.
    pub fn famiglia(&self) -> &str {
        self.nome.split_whitespace().next().unwrap_or(&self.nome)
    }
}

/// Cosa c'e' sul disco, chiesto a chi sa guardarlo.
///
/// La **decisione** sta qui e si puo' provare senza un progetto vero;
/// guardare il disco e' un'operazione, e sta da un'altra parte. E' la stessa
/// separazione del taglio dei blocchi, per la stessa ragione.
#[derive(Debug, Clone, Default)]
pub struct Segni {
    pub ha_cargo: bool,
    /// Le sottocartelle che hanno un `Cargo.toml` loro, in ordine.
    pub cargo_sotto: Vec<String>,
    /// `package.json` c'e' **e** dichiara uno script `test`.
    pub npm_prova: bool,
    pub ha_go: bool,
    /// Il progetto dichiara pytest, in un modo o nell'altro.
    pub dichiara_pytest: bool,
    /// I `test_*.py` che sono script da eseguire, in ordine.
    pub script_soli: Vec<String>,
}

/// Le sottocartelle in cui si guarda se c'e' un secondo progetto Rust.
pub const SOTTO_RUST: [&str; 3] = ["core", "rust", "src-tauri"];

/// I file che, se ci sono, vogliono dire «questo progetto usa pytest».
pub const DICHIARANO_PYTEST: [&str; 3] = ["pytest.ini", "tox.ini", "setup.cfg"];

/// Come si prova questo progetto. Il primo della lista e' il piu' probabile.
pub fn scopri(radice: &Path, segni: &Segni, python: &str) -> Vec<Banco> {
    let mut banchi = Vec::new();
    if segni.ha_cargo {
        banchi.push(Banco::nuovo("cargo", &["cargo", "test"], radice));
    }
    for sotto in &segni.cargo_sotto {
        banchi.push(Banco::nuovo(
            &format!("cargo ({sotto})"),
            &["cargo", "test"],
            &radice.join(sotto),
        ));
    }
    if segni.npm_prova {
        banchi.push(Banco::nuovo("npm", &["npm", "test", "--silent"], radice));
    }
    if segni.ha_go {
        banchi.push(Banco::nuovo("go", &["go", "test", "./..."], radice));
    }
    if segni.dichiara_pytest {
        banchi.push(Banco::nuovo(
            "pytest",
            &[python, "-m", "pytest", "-q"],
            radice,
        ));
    }
    // La convenzione di NOVA, e di chiunque scriva i test come script. Non
    // si aggiunge dove pytest c'e' gia': sarebbero gli stessi file provati
    // due volte in due modi, e due verdetti sullo stesso file.
    if !segni.script_soli.is_empty() && !segni.dichiara_pytest {
        let mut b = Banco::nuovo("script", &[python], radice);
        b.pezzi = segni.script_soli.clone();
        banchi.push(b);
    }
    banchi
}

/// Lo script `test` dichiarato in un `package.json`.
///
/// Si legge il campo, non si indovina: un `package.json` senza script `test`
/// e' un progetto che i test non li ha, e `npm test` li' stampa un errore e
/// esce diverso da zero — cioe' un rosso che non e' un rosso.
pub fn script_di_test(pacchetto: &str) -> String {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(pacchetto.trim_start_matches('\u{feff}'))
    else {
        return String::new();
    };
    v.get("scripts")
        .and_then(|s| s.get("test"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string()
}

/// Se questo file e' uno script che si esegue, non un file che pytest
/// raccoglie.
///
/// Il segno e' l'uscita esplicita: un file scritto per pytest non ne ha
/// bisogno, perche' non viene mai eseguito da solo.
pub fn e_uno_script(contenuto: &str) -> bool {
    contenuto.contains("sys.exit(") || contenuto.contains("raise SystemExit")
}

/// Come e' andata una prova sola.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Andata {
    Passata,
    Caduta,
    NonProvabileQui,
}

/// Cosa vuol dire questo codice di uscita.
pub fn come_e_andata(codice: i32) -> Andata {
    match codice {
        0 => Andata::Passata,
        NON_PROVABILE_QUI => Andata::NonProvabileQui,
        _ => Andata::Caduta,
    }
}

/// La coda dell'uscita di una prova caduta.
pub fn coda(uscita: &str, righe: usize) -> String {
    let tutte: Vec<&str> = uscita.trim().lines().collect();
    let da = tutte.len().saturating_sub(righe);
    tutte[da..].join("\n")
}

/// Com'e' andato un giro di prove.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Esito {
    pub provabile: bool,
    pub banco: String,
    pub comando: String,
    pub passate: Vec<String>,
    pub cadute: Vec<String>,
    pub saltate: Vec<String>,
    /// Vuoto se si e' potuto provare.
    pub motivo: String,
}

impl Esito {
    pub fn ok(&self) -> bool {
        self.provabile && self.cadute.is_empty()
    }

    pub fn non_provabile(motivo: &str) -> Esito {
        Esito {
            provabile: false,
            motivo: motivo.to_string(),
            ..Esito::default()
        }
    }
}

/// Il verdetto del confronto fra due giri.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdetto {
    Peggio,
    Meglio,
    Uguale,
    Ignoto,
}

impl Verdetto {
    pub fn nome(self) -> &'static str {
        match self {
            Verdetto::Peggio => "peggio",
            Verdetto::Meglio => "meglio",
            Verdetto::Uguale => "uguale",
            Verdetto::Ignoto => "ignoto",
        }
    }
}

/// Cosa e' cambiato fra prima e dopo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Giudizio {
    pub verdetto: Verdetto,
    pub nuove_cadute: Vec<String>,
    pub guarite: Vec<String>,
    pub racconto: String,
}

/// Quante prove si nominano nel racconto prima di fermarsi.
pub const QUANTE_SI_NOMINANO: usize = 6;

/// Non «e' verde», ma «e' peggio di prima».
///
/// E' la differenza fra un verificatore che si puo' usare su un progetto vero
/// e uno che funziona solo se la suite era gia' tutta verde. Le cadute che
/// c'erano gia' non sono colpa della modifica, e rifiutare per quelle vuol
/// dire non poter mai applicare niente su un progetto vivo.
pub fn confronta(prima: &Esito, dopo: &Esito) -> Giudizio {
    if !dopo.provabile {
        return Giudizio {
            verdetto: Verdetto::Ignoto,
            nuove_cadute: Vec::new(),
            guarite: Vec::new(),
            racconto: if dopo.motivo.is_empty() {
                "non ho potuto provare".to_string()
            } else {
                dopo.motivo.clone()
            },
        };
    }
    // Se **prima** non si e' potuto provare, non si sa cosa cadeva gia': si
    // parte da «niente era rotto», che e' il verso prudente — cosi' una
    // caduta nuova si vede invece di essere perdonata.
    let gia_rotte: Vec<String> = if prima.provabile {
        prima.cadute.clone()
    } else {
        Vec::new()
    };
    let nuove: Vec<String> = dopo
        .cadute
        .iter()
        .filter(|x| !gia_rotte.contains(x))
        .cloned()
        .collect();
    let guarite: Vec<String> = gia_rotte
        .iter()
        .filter(|x| !dopo.cadute.contains(x))
        .cloned()
        .collect();
    if !nuove.is_empty() {
        let racconto = format!(
            "cade quello che prima passava: {}",
            nuove
                .iter()
                .take(QUANTE_SI_NOMINANO)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        );
        return Giudizio {
            verdetto: Verdetto::Peggio,
            nuove_cadute: nuove,
            guarite,
            racconto,
        };
    }
    if !guarite.is_empty() {
        let racconto = format!(
            "e ne ripara: {}",
            guarite
                .iter()
                .take(QUANTE_SI_NOMINANO)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        );
        return Giudizio {
            verdetto: Verdetto::Meglio,
            nuove_cadute: Vec::new(),
            guarite,
            racconto,
        };
    }
    Giudizio {
        verdetto: Verdetto::Uguale,
        nuove_cadute: Vec::new(),
        guarite: Vec::new(),
        racconto: if gia_rotte.is_empty() {
            "i test passano come prima".to_string()
        } else {
            format!("non peggiora niente ({} gia' rotti prima)", gia_rotte.len())
        },
    }
}

/// L'esito in una riga, per chi legge e non per chi conta.
pub fn racconta(esito: &Esito, durata_s: f64) -> String {
    if !esito.provabile {
        return if esito.motivo.is_empty() {
            "non provabile".to_string()
        } else {
            esito.motivo.clone()
        };
    }
    let mut pezzi = vec![format!("{} passate", esito.passate.len())];
    if !esito.cadute.is_empty() {
        pezzi.push(format!("{} cadute", esito.cadute.len()));
    }
    if !esito.saltate.is_empty() {
        pezzi.push(format!("{} non provabili qui", esito.saltate.len()));
    }
    // Un decimale sempre, anche quando e' zero: dall'altra parte il numero
    // e' un `float` arrotondato a uno, e `1.0` si stampa «1.0». Due meta'
    // che raccontano «1s» e «1.0s» sono due programmi.
    format!(
        "{} in {:.1}s ({})",
        pezzi.join(", "),
        durata_s,
        esito.comando
    )
}

/// Che famiglia di banco serve per un file di questa lingua.
///
/// Provare la suite Rust perche' si e' cambiata una riga di Python e' tempo
/// buttato, e su un progetto grosso e' tanto tempo.
pub const LINGUA: [(&str, &str); 8] = [
    (".go", "go"),
    (".js", "npm"),
    (".jsx", "npm"),
    (".rs", "cargo"),
    (".svelte", "npm"),
    (".ts", "npm"),
    (".tsx", "npm"),
    (".vue", "npm"),
];

/// Il `.py` ne vuole due, in ordine: pytest se c'e', se no gli script.
pub const PER_PYTHON: [&str; 2] = ["pytest", "script"];

/// Quali famiglie di banco vanno bene per questo file.
pub fn famiglie_per(estensione: &str) -> Vec<&'static str> {
    if estensione == ".py" {
        return PER_PYTHON.to_vec();
    }
    LINGUA
        .iter()
        .find(|(e, _)| *e == estensione)
        .map(|(_, f)| vec![*f])
        .unwrap_or_default()
}

/// Il banco giusto per quel file, o il primo che c'e'.
///
/// Un file di una lingua che questo progetto non prova, e un **documento**,
/// non tornano niente: far girare i test lo stesso vorrebbe dire dare un
/// verde che non parla di quel file, ed e' peggio di un «non so come si
/// prova» (D278).
pub fn scegli<'a>(banchi: &'a [Banco], estensione: &str) -> Option<&'a Banco> {
    if banchi.is_empty() {
        return None;
    }
    if estensione.is_empty() {
        return banchi.first();
    }
    for voluta in famiglie_per(estensione) {
        if let Some(b) = banchi.iter().find(|b| b.famiglia() == voluta) {
            return Some(b);
        }
    }
    None
}

#[cfg(test)]
mod prove;
