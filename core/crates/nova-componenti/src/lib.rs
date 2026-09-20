//! Cosa serve a ogni funzione di NOVA, cosa c'e' gia', e con che disciplina
//! si procura quel che manca.
//!
//! Il difetto che questo pezzo chiude: il pannello sapeva **scegliere** e non
//! sapeva **procurare**. Chi passava la voce a Kokoro senza averne i file
//! vedeva il menu cambiare e la voce restare muta, e l'unico posto capace di
//! scaricare qualcosa era l'installatore — cioe' ogni ripensamento costava
//! una reinstallazione.
//!
//! ## Le tre regole dello scaricare
//!
//! Valgono per tutti i pezzi, e ognuna e' scritta per un modo di sbagliare:
//!
//! **Si scarica di fianco, non sopra.** Il file arriva come `.parte` e prende
//! il suo nome solo a scaricamento finito. Una connessione che cade lascia
//! spazzatura riconoscibile, non un file valido a meta' che il programma
//! caricherebbe volentieri.
//!
//! **Si dichiara quanto pesa prima di cominciare**, perche' 800 MB su una
//! connessione lenta sono una decisione, non un dettaglio.
//!
//! **Si puo' fermare**, e fermarsi non deve lasciare niente a meta'.
//!
//! ## Cosa c'e' qui, e cosa no
//!
//! Qui ci sono le **regole** e il catalogo; la rete, lo zip e il disco stanno
//! a chi li ha. E' la stessa divisione di `nova_dati`: quel che si puo'
//! provare senza una connessione sta dove si prova.

pub mod catalogo;
pub use catalogo::catalogo;

use std::path::{Path, PathBuf};

/// Quanto si legge per volta. Con blocchi troppo piccoli si parla troppo, con
/// blocchi troppo grossi la barra sta ferma e sembra piantata.
pub const BLOCCO: usize = 1024 * 256;

/// Come si chiama un file mentre sta arrivando.
///
/// L'estensione si **aggiunge**, non si sostituisce: `modello.onnx` diventa
/// `modello.onnx.parte` e non `modello.parte`. Sostituirla vorrebbe dire che
/// due pezzi diversi con lo stesso nome e estensioni diverse finirebbero a
/// litigarsi lo stesso file temporaneo.
pub fn in_arrivo(dove: &Path) -> PathBuf {
    let mut nome = dove.as_os_str().to_os_string();
    nome.push(".parte");
    PathBuf::from(nome)
}

/// Se si deve dire qualcosa, arrivati a questa percentuale.
///
/// Un evento per blocco sono migliaia di righe su un file da mezzo giga. Si
/// parla quando la percentuale cambia, e nient'altro: la barra si muove
/// cento volte e non diecimila.
pub fn deve_parlare(percento: u8, ultima: Option<u8>) -> bool {
    Some(percento) != ultima
}

/// A che punto e', in percentuale.
///
/// Senza un totale la percentuale non esiste: si torna zero invece di
/// inventarne una. Una barra che avanza su un totale sconosciuto e' una
/// bugia che si scopre quando arriva al 100% e il file continua a crescere.
pub fn percento(fatto: u64, totale: u64) -> u8 {
    if totale == 0 {
        return 0;
    }
    ((fatto.min(totale) * 100) / totale) as u8
}

/// Un `.parte` rimasto da un tentativo andato male **non si riprende**.
///
/// Non sappiamo se il server servisse lo stesso file — una versione nuova con
/// lo stesso nome capita — e riprendere sbagliato produce un archivio
/// corrotto che **sembra intero**, cioe' il guasto peggiore di tutti: si
/// scopre al caricamento, lontano da qui.
pub const SI_RIPRENDE: bool = false;

/// Di che tipo e' un pezzo da procurare.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tipo {
    /// Un file, preso com'e'.
    File { url: String },
    /// Una copia da dentro il progetto: non si scarica niente.
    Copia { da: PathBuf },
    /// Uno zip da cui si prendono le dll che contengono una parola.
    ZipDll { url: String, filtro: String },
    /// Uno zip che si **appiattisce**: i rilasci mettono tutto in una
    /// sottocartella, e sperare che si chiami sempre allo stesso modo e' un
    /// modo di rompersi a ogni versione nuova.
    ZipPiatto { url: String },
    /// L'msi ufficiale di espeak-ng, aperto senza installarlo.
    MsiEspeak,
}

/// Un pezzo di un componente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pezzo {
    pub tipo: Tipo,
    /// Dove finisce: un file, o la cartella in cui si estrae.
    pub dove: PathBuf,
    /// Il file la cui esistenza dice che il pezzo c'e'. Per uno zip non e'
    /// `dove`, che e' una cartella e c'e' sempre.
    pub prova: Option<PathBuf>,
    /// File che valgono altrettanto, se gia' sul disco.
    ///
    /// Chi ha gia' `ggml-small.bin` non deve scaricare `ggml-base.bin` solo
    /// perche' il catalogo ne nomina un altro: sono mezzo giga per niente.
    pub vale_anche: Vec<String>,
}

impl Pezzo {
    /// Il file da guardare per sapere se c'e'.
    pub fn da_guardare(&self) -> &Path {
        self.prova.as_deref().unwrap_or(&self.dove)
    }

    /// Tutti i percorsi che soddisfano questo pezzo, in ordine.
    pub fn basta_uno_di(&self) -> Vec<PathBuf> {
        let primo = self.da_guardare().to_path_buf();
        let cartella = primo.parent().map(Path::to_path_buf).unwrap_or_default();
        let mut tutti = vec![primo];
        tutti.extend(self.vale_anche.iter().map(|a| cartella.join(a)));
        tutti
    }

    /// Se c'e', chiesto a chi sa guardare il disco.
    pub fn presente(&self, esiste: &dyn Fn(&Path) -> bool) -> bool {
        self.basta_uno_di().iter().any(|p| esiste(p))
    }
}

/// Una cosa che NOVA sa fare solo se ha i suoi pezzi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Componente {
    pub nome: String,
    pub titolo: String,
    /// A cosa serve, detto a chi non sa cos'e'.
    pub serve_a: String,
    /// **Cosa succede senza.** E' il campo che decide se uno scarica o no, e
    /// l'unico che una persona legge davvero.
    pub senza: String,
    pub mb: u32,
    /// Vuota se non c'e' niente da dire. espeak-ng e' GPLv3 e non viaggia
    /// dentro un progetto MIT: si scarica dalla sua fonte, e si dice.
    pub licenza: String,
    pub pezzi: Vec<Pezzo>,
}

/// Cosa c'e' e cosa manca, senza toccare la rete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stato {
    pub nome: String,
    pub titolo: String,
    pub serve_a: String,
    pub senza: String,
    pub licenza: String,
    pub mb: u32,
    pub presente: bool,
    pub mancano: usize,
    pub totale: usize,
}

/// Lo stato di un componente.
pub fn stato_di(c: &Componente, esiste: &dyn Fn(&Path) -> bool) -> Stato {
    let mancano = c.pezzi.iter().filter(|p| !p.presente(esiste)).count();
    Stato {
        nome: c.nome.clone(),
        titolo: c.titolo.clone(),
        serve_a: c.serve_a.clone(),
        senza: c.senza.clone(),
        licenza: c.licenza.clone(),
        mb: c.mb,
        presente: mancano == 0,
        mancano,
        totale: c.pezzi.len(),
    }
}

/// Quanto resta da scaricare, in megabyte.
///
/// Si conta **solo cio' che manca**: dire «350 MB» a chi ne ha gia' due terzi
/// e' il modo di far rinunciare qualcuno che avrebbe finito in un minuto.
pub fn quanto_manca(componenti: &[Componente], esiste: &dyn Fn(&Path) -> bool) -> u32 {
    componenti
        .iter()
        .filter(|c| c.pezzi.iter().any(|p| !p.presente(esiste)))
        .map(|c| c.mb)
        .sum()
}

#[cfg(test)]
mod prove {
    use super::*;
    use std::collections::HashSet;

    fn pezzo(dove: &str, prova: Option<&str>, vale_anche: &[&str]) -> Pezzo {
        Pezzo {
            tipo: Tipo::File { url: "https://esempio.it/x".into() },
            dove: PathBuf::from(dove),
            prova: prova.map(PathBuf::from),
            vale_anche: vale_anche.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Un finto disco su cui ci sono questi file e basta.
    ///
    /// I separatori si normalizzano, e non e' pignoleria di stile: `join`
    /// su Windows mette il backslash, e un elenco scritto con le barre in
    /// avanti non riconosceva piu' niente. Questa prova era verde qui e
    /// rossa sulla CI, ed e' lo stesso inciampo che aveva gia' preso le
    /// prove di `nova-cartelle` (D209): il codice va su tutti e due i
    /// sistemi, le **prove** erano scritte per uno solo.
    fn cosa_c_e(elenco: &[&str]) -> impl Fn(&Path) -> bool {
        fn dritto(s: &str) -> String {
            s.replace('\\', "/")
        }
        let s: HashSet<String> = elenco.iter().map(|x| dritto(x)).collect();
        move |p: &Path| s.contains(&dritto(&p.display().to_string()))
    }

    #[test]
    fn il_parte_si_aggiunge_non_sostituisce() {
        // `modello.onnx.parte`, non `modello.parte`: due pezzi con lo stesso
        // nome e estensioni diverse si litigherebbero lo stesso temporaneo.
        assert_eq!(
            in_arrivo(Path::new("/voce/kokoro-v1.0.onnx")),
            PathBuf::from("/voce/kokoro-v1.0.onnx.parte")
        );
        assert_eq!(in_arrivo(Path::new("/x/senza")), PathBuf::from("/x/senza.parte"));
    }

    #[test]
    fn un_parte_rimasto_non_si_riprende() {
        // Non sappiamo se il server servisse lo stesso file, e riprendere
        // sbagliato produce un archivio corrotto che **sembra intero**.
        assert!(!SI_RIPRENDE);
    }

    #[test]
    fn si_parla_quando_la_percentuale_cambia() {
        assert!(deve_parlare(0, None), "la prima volta si parla sempre");
        assert!(!deve_parlare(7, Some(7)));
        assert!(deve_parlare(8, Some(7)));
    }

    #[test]
    fn senza_un_totale_la_percentuale_non_si_inventa() {
        // Una barra che avanza su un totale sconosciuto e' una bugia che si
        // scopre quando arriva al 100% e il file continua a crescere.
        assert_eq!(percento(5_000, 0), 0);
        assert_eq!(percento(0, 100), 0);
        assert_eq!(percento(50, 100), 50);
        assert_eq!(percento(100, 100), 100);
    }

    #[test]
    fn la_percentuale_non_supera_cento() {
        // Capita: il `Content-Length` dichiarato e' minore di quel che arriva.
        assert_eq!(percento(200, 100), 100);
    }

    #[test]
    fn per_uno_zip_si_guarda_la_prova_non_la_cartella() {
        // `dove` per uno zip e' una cartella, e una cartella c'e' sempre:
        // guardare quella vorrebbe dire dire «c'e'» a chi non ha niente.
        let p = Pezzo {
            tipo: Tipo::ZipPiatto { url: "https://x/w.zip".into() },
            dove: PathBuf::from("/ascolto"),
            prova: Some(PathBuf::from("/ascolto/whisper-cli.exe")),
            vale_anche: vec![],
        };
        assert_eq!(p.da_guardare(), Path::new("/ascolto/whisper-cli.exe"));
        assert!(!p.presente(&cosa_c_e(&["/ascolto"])));
        assert!(p.presente(&cosa_c_e(&["/ascolto/whisper-cli.exe"])));
    }

    #[test]
    fn un_file_equivalente_gia_sul_disco_conta() {
        // Chi ha gia' `ggml-small.bin` non deve scaricarne un altro solo
        // perche' il catalogo ne nomina uno diverso: e' mezzo giga per niente.
        let p = pezzo("/ascolto/ggml-base.bin", None,
                      &["ggml-small.bin", "ggml-medium.bin", "ggml-tiny.bin"]);
        assert!(p.presente(&cosa_c_e(&["/ascolto/ggml-small.bin"])));
        assert!(p.presente(&cosa_c_e(&["/ascolto/ggml-base.bin"])));
        assert!(!p.presente(&cosa_c_e(&["/ascolto/ggml-altro.bin"])));
    }

    #[test]
    fn gli_equivalenti_si_cercano_nella_stessa_cartella() {
        let p = pezzo("/ascolto/ggml-base.bin", None, &["ggml-small.bin"]);
        assert_eq!(
            p.basta_uno_di(),
            vec![
                PathBuf::from("/ascolto/ggml-base.bin"),
                PathBuf::from("/ascolto/ggml-small.bin"),
            ]
        );
    }

    fn voce() -> Componente {
        Componente {
            nome: "voce_locale".into(),
            titolo: "Voce locale (Kokoro)".into(),
            serve_a: "Far parlare NOVA senza che l'audio esca dal PC".into(),
            senza: "NOVA scrive ma non parla".into(),
            mb: 350,
            licenza: String::new(),
            pezzi: vec![
                pezzo("/voce/kokoro.onnx", None, &[]),
                pezzo("/voce/voices.bin", None, &[]),
            ],
        }
    }

    #[test]
    fn lo_stato_conta_i_pezzi_che_mancano() {
        let c = voce();
        let mezzo = stato_di(&c, &cosa_c_e(&["/voce/kokoro.onnx"]));
        assert!(!mezzo.presente);
        assert_eq!(mezzo.mancano, 1);
        assert_eq!(mezzo.totale, 2);
        let tutto = stato_di(&c, &cosa_c_e(&["/voce/kokoro.onnx", "/voce/voices.bin"]));
        assert!(tutto.presente);
        assert_eq!(tutto.mancano, 0);
    }

    #[test]
    fn lo_stato_porta_con_se_cosa_succede_senza() {
        // E' il campo che decide se uno scarica o no, e l'unico che una
        // persona legge davvero. Perderlo per strada vuol dire un pannello
        // che dice «manca» senza dire cosa cambia.
        let s = stato_di(&voce(), &cosa_c_e(&[]));
        assert_eq!(s.senza, "NOVA scrive ma non parla");
        assert_eq!(s.serve_a, "Far parlare NOVA senza che l'audio esca dal PC");
    }

    #[test]
    fn si_conta_solo_quel_che_manca() {
        // Dire «350 MB» a chi ne ha gia' due terzi e' il modo di far
        // rinunciare qualcuno che avrebbe finito in un minuto.
        let mut altro = voce();
        altro.nome = "ascolto".into();
        altro.mb = 420;
        let tutti = vec![voce(), altro];
        assert_eq!(quanto_manca(&tutti, &cosa_c_e(&[])), 770);
        assert_eq!(
            quanto_manca(&tutti, &cosa_c_e(&["/voce/kokoro.onnx", "/voce/voices.bin"])),
            0,
            "se ci sono tutti, non manca niente",
        );
    }

    #[test]
    fn una_copia_dal_progetto_non_e_uno_scaricamento() {
        // `vocab.json` sta gia' dentro il progetto: chiederlo alla rete
        // sarebbe scaricare una cosa che si ha in mano.
        let p = Pezzo {
            tipo: Tipo::Copia { da: PathBuf::from("/progetto/vocab.json") },
            dove: PathBuf::from("/voce/vocab.json"),
            prova: None,
            vale_anche: vec![],
        };
        assert!(matches!(p.tipo, Tipo::Copia { .. }));
        assert!(p.presente(&cosa_c_e(&["/voce/vocab.json"])));
    }
}
