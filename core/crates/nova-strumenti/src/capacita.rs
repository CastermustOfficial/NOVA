//! Cosa NOVA chiede al sistema operativo, detto **con le parole di NOVA**.
//!
//! Questi tratti li dichiara chi ne ha bisogno, non chi li implementa, e non
//! e' un dettaglio di organizzazione: se fosse la piattaforma a dichiarare le
//! sue capacita' e gli strumenti ad adattarsi, sarebbero i verbi di Windows a
//! decidere la forma di NOVA. Qui c'e' scritto «copia questo testo negli
//! appunti», non «chiama `SetClipboardData`» — e chi sa farlo si fa avanti.
//!
//! **Perche' non una shell.** Dall'altra parte gli appunti *erano*
//! `Get-Clipboard`, il volume *era* `SendKeys`, e l'elenco delle finestre
//! aperte *e'* ancora `Get-Process` con un `ConvertTo-Csv` in coda. Contate
//! con un analizzatore di sintassi — non a memoria, vedi D136 — le funzioni
//! che passano da una shell sono ventiquattro, di cui tredici strumenti
//! esposti al modello. Vuol dire un processo da avviare, una shell che
//! interpreta e una stringa da comporre per ogni gesto — e vuol dire che
//! quelle capacita' **non esistono** dove PowerShell manca o e' bloccato da
//! una policy. Non degradano: spariscono. Vedi D130.
//!
//! Ogni tratto ha un'implementazione che non sa fare niente e **lo dice**: una
//! capacita' che manca in silenzio e' peggio di una che manca.

/// Gli appunti di sistema.
pub trait Appunti {
    /// Cosa c'e' scritto adesso, o `None` se non c'e' testo.
    fn leggi(&self) -> Result<Option<String>, String>;
    /// Ci mette questo testo.
    fn scrivi(&self, testo: &str) -> Result<(), String>;
}

/// Le notifiche che compaiono in un angolo dello schermo.
pub trait Notifiche {
    /// Consegna la notifica e torna: **non** aspetta che sparisca.
    ///
    /// La distinzione e' il difetto che questo tratto esiste per non
    /// ripetere. Il fumetto dell'area di notifica muore insieme a chi possiede
    /// l'icona, quindi qualcuno deve restare li' per tutta la sua durata — e
    /// nel Python quel qualcuno era NOVA, ferma nove secondi (misurati) a
    /// guardare un fumetto che sta gia' guardando l'utente. Chi implementa
    /// questo metodo si organizza da solo per aspettare altrove.
    ///
    /// `Ok` vuol dire «consegnata», non «vista»: l'unico giudice di «e'
    /// comparsa?» e' la persona davanti allo schermo, e non ha un'API.
    fn mostra(&self, titolo: &str, messaggio: &str) -> Result<(), String>;
}

/// Il volume di sistema.
pub trait Audio {
    /// Da 0 a 100, e se e' muto.
    fn stato(&self) -> Result<(u8, bool), String>;
    fn imposta(&self, livello: u8) -> Result<(), String>;
    fn muto(&self, muto: bool) -> Result<(), String>;
}

/// Com'e' fatta la macchina su cui NOVA gira.
///
/// E' la capacita' che il modello chiede piu' spesso all'inizio di una
/// conversazione, quando vuole sapere dove si trova, ed era la piu' cara di
/// tutte: 1.543 millisecondi misurati, piu' di tutte le altre messe insieme,
/// perche' dall'altra parte era una query WMI dentro una shell.
pub trait Macchina {
    /// I numeri, non il racconto: a scriverli per una persona ci pensa
    /// [`informazioni`], e a leggerli per un conto ci pensa chi li riceve.
    fn com_e_fatta(&self) -> Result<crate::sistema::Macchina, String>;
}

/// La finestra che ha il fuoco, per quel che serve a chi preme i tasti.
///
/// L'identificativo c'e' perche' e' l'unica cosa che si puo' **ricontrollare**:
/// il titolo cambia da solo (un documento che si salva, una scheda che
/// carica), il processo e' uguale per dieci finestre. Il titolo e il processo
/// ci sono perche' sono l'unica cosa che una persona sa riconoscere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finestra {
    pub handle: i64,
    pub titolo: String,
    pub processo: String,
}

/// La tastiera, cioe' la sola capacita' che **non ha un bersaglio**.
///
/// Tutte le altre dicono a chi parlano: il file ha un percorso, la notifica
/// va all'utente, gli appunti sono uno solo. I tasti no: il sistema li
/// consegna a chi ha il fuoco *in quel millisecondo*, e fra l'approvazione di
/// una persona e il momento in cui partono il fuoco puo' essere cambiato
/// (D143). Per questo i due metodi che agiscono vogliono **dove**, e chi li
/// implementa si impegna a ricontrollarlo e a fermarsi se e' cambiato.
pub trait Tastiera {
    /// Chi ha il fuoco adesso, o `None` se non ce l'ha nessuno.
    fn davanti(&self) -> Result<Option<Finestra>, String>;
    /// Scrive il testo, ma solo finche' il fuoco resta su `dove`.
    fn scrivi_dentro(&self, testo: &str, dove: i64) -> Result<(), String>;
    /// Preme la combinazione, ma solo se il fuoco e' ancora su `dove`.
    fn premi_dentro(&self, tasti: &str, dove: i64) -> Result<(), String>;
}

/// Nessuno che sappia fare queste cose. Non e' un ripiego silenzioso: ogni
/// metodo dice **perche'**, cosi' chi legge la risposta capisce che manca il
/// sistema e non che ha sbagliato lui.
pub struct NienteSistema;

fn manca(cosa: &str) -> String {
    format!(
        "{cosa} non e' disponibile su questo sistema. \
         Non e' un errore della richiesta: e' una capacita' che qui non c'e'."
    )
}

impl Appunti for NienteSistema {
    fn leggi(&self) -> Result<Option<String>, String> {
        Err(manca("leggere gli appunti"))
    }
    fn scrivi(&self, _testo: &str) -> Result<(), String> {
        Err(manca("scrivere negli appunti"))
    }
}

impl Notifiche for NienteSistema {
    fn mostra(&self, _titolo: &str, _messaggio: &str) -> Result<(), String> {
        Err(manca("mostrare una notifica"))
    }
}

impl Tastiera for NienteSistema {
    fn davanti(&self) -> Result<Option<Finestra>, String> {
        Err(manca("sapere chi ha il fuoco"))
    }
    fn scrivi_dentro(&self, _testo: &str, _dove: i64) -> Result<(), String> {
        Err(manca("premere i tasti"))
    }
    fn premi_dentro(&self, _tasti: &str, _dove: i64) -> Result<(), String> {
        Err(manca("premere i tasti"))
    }
}

impl Macchina for NienteSistema {
    fn com_e_fatta(&self) -> Result<crate::sistema::Macchina, String> {
        Err(manca("leggere com'e' fatto il PC"))
    }
}

impl Audio for NienteSistema {
    fn stato(&self) -> Result<(u8, bool), String> {
        Err(manca("leggere il volume"))
    }
    fn imposta(&self, _livello: u8) -> Result<(), String> {
        Err(manca("cambiare il volume"))
    }
    fn muto(&self, _muto: bool) -> Result<(), String> {
        Err(manca("silenziare l'audio"))
    }
}

// ------------------------------------------------------------- i corpi
/// Legge gli appunti e lo racconta al modello.
pub fn leggi_appunti(a: &dyn Appunti) -> Result<String, String> {
    match a.leggi()? {
        Some(t) if !t.is_empty() => Ok(t),
        // «(appunti vuoti)» e non una stringa vuota: una risposta vuota il
        // modello non sa distinguerla da uno strumento che non ha funzionato.
        _ => Ok("(appunti vuoti)".into()),
    }
}

/// Ci mette un testo, e dice quanto.
pub fn scrivi_appunti(a: &dyn Appunti, testo: &str) -> Result<String, String> {
    a.scrivi(testo)?;
    Ok(format!("Copiati {} caratteri negli appunti.", testo.chars().count()))
}

/// Mostra una notifica.
pub fn notifica(n: &dyn Notifiche, titolo: &str, messaggio: &str) -> Result<String, String> {
    n.mostra(titolo, messaggio)?;
    Ok(format!("Notifica mostrata: {messaggio}"))
}

/// Cambia il volume, o lo silenzia, e dice com'e' rimasto.
pub fn volume(a: &dyn Audio, livello: Option<i64>, muto: Option<bool>) -> Result<String, String> {
    if livello.is_none() && muto.is_none() {
        return Err("serve 'level' o 'mute'".into());
    }
    if let Some(m) = muto {
        a.muto(m)?;
    }
    if let Some(l) = livello {
        a.imposta(l.clamp(0, 100) as u8)?;
    }
    let (adesso, e_muto) = a.stato()?;
    Ok(format!("Volume: {adesso}% (muto={})", if e_muto { "True" } else { "False" }))
}

/// Cosa si risponde quando davanti non c'e' nessuno.
///
/// `strumento` e' il nome con cui **quella meta'** di NOVA porta davanti una
/// finestra: nel Python e' `focus_window`, nel demone `ui.focus`. Nominare
/// uno strumento che chi legge non ha sarebbe un consiglio che non si puo'
/// seguire.
pub fn nessuno_davanti(strumento: &str) -> String {
    format!(
        "in questo momento nessuna finestra ha il fuoco: non premo niente, \
         perche' non saprei dove andrebbe a finire. Porta davanti la finestra \
         giusta con «{strumento}» e riprova."
    )
}

/// Digita un testo nella finestra che ha il fuoco, **nominandola**.
///
/// Tre regole, ognuna con una ragione:
///
/// 1. **si guarda prima chi c'e'**, e se non c'e' nessuno non si preme
///    niente: non si saprebbe dove va a finire;
/// 2. **si scrive legati a quella finestra**, e se il fuoco si sposta chi
///    implementa si ferma: fra il controllo e l'invio c'e' sempre un «fra»;
/// 3. **la risposta dice quale**. «Digitati 42 caratteri nella finestra
///    attiva» e' vero e inutile: non permette a nessuno — ne' al modello ne'
///    all'utente — di accorgersi che il testo e' andato altrove.
///
/// E un guasto a meta' **resta un guasto**. Non si riprova per altra strada:
/// una parte del testo puo' essere gia' arrivata, e ripeterlo per intero
/// vorrebbe dire scriverlo due volte — la seconda nella finestra che nel
/// frattempo ha preso il fuoco, cioe' in quella sbagliata per definizione.
pub fn digita(t: &dyn Tastiera, testo: &str, strumento_fuoco: &str) -> Result<String, String> {
    let w = t
        .davanti()?
        .ok_or_else(|| nessuno_davanti(strumento_fuoco))?;
    t.scrivi_dentro(testo, w.handle)?;
    Ok(format!(
        "Digitati {} caratteri in «{}» ({}).",
        testo.chars().count(),
        w.titolo,
        w.processo
    ))
}

/// Preme una combinazione nella finestra che ha il fuoco, nominandola.
///
/// Le stesse tre regole di [`digita`].
pub fn premi(t: &dyn Tastiera, tasti: &str, strumento_fuoco: &str) -> Result<String, String> {
    let w = t
        .davanti()?
        .ok_or_else(|| nessuno_davanti(strumento_fuoco))?;
    t.premi_dentro(tasti, w.handle)?;
    Ok(format!(
        "Inviata la combinazione: {tasti} in «{}» ({}).",
        w.titolo, w.processo
    ))
}

/// Aggiunge a un'anteprima **quale** finestra riceverebbe i tasti.
///
/// E' l'unica cosa con cui chi approva puo' decidere. «Digita nella finestra
/// attiva: ciao» e' identica se davanti c'e' il blocco note o il documento su
/// cui l'utente stava lavorando, e il risultato e' un'altra cosa (D143).
pub fn con_la_finestra(testa: &str, davanti: Option<&Finestra>) -> String {
    match davanti {
        None => format!("{testa} — non riesco a dire quale sia"),
        Some(w) => format!("{testa}\n  La finestra e': «{}» ({})", w.titolo, w.processo),
    }
}

/// La testa dell'anteprima di [`digita`]: il testo, fino a duecento caratteri.
pub fn anteprima_digita(testo: &str) -> String {
    format!(
        "Digita nella finestra che ha il fuoco: {}",
        testo.chars().take(200).collect::<String>()
    )
}

/// La testa dell'anteprima di [`premi`].
pub fn anteprima_tasti(tasti: &str) -> String {
    format!("Preme i tasti {tasti} nella finestra che ha il fuoco")
}

/// Com'e' fatto il PC, letto e raccontato.
pub fn informazioni(m: &dyn Macchina) -> Result<String, String> {
    Ok(crate::sistema::racconta(&m.com_e_fatta()?))
}

#[cfg(test)]
mod prove {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    struct AppuntiFinti(RefCell<Option<String>>);

    impl Appunti for AppuntiFinti {
        fn leggi(&self) -> Result<Option<String>, String> {
            Ok(self.0.borrow().clone())
        }
        fn scrivi(&self, testo: &str) -> Result<(), String> {
            *self.0.borrow_mut() = Some(testo.to_string());
            Ok(())
        }
    }

    #[test]
    fn gli_appunti_vuoti_lo_dicono() {
        // Una stringa vuota il modello non la distingue da uno strumento che
        // non ha funzionato.
        let a = AppuntiFinti::default();
        assert_eq!(leggi_appunti(&a).unwrap(), "(appunti vuoti)");
        a.scrivi("").unwrap();
        assert_eq!(leggi_appunti(&a).unwrap(), "(appunti vuoti)");
    }

    #[test]
    fn si_scrive_e_si_rilegge() {
        let a = AppuntiFinti::default();
        assert_eq!(scrivi_appunti(&a, "ciao").unwrap(),
                   "Copiati 4 caratteri negli appunti.");
        assert_eq!(leggi_appunti(&a).unwrap(), "ciao");
    }

    #[test]
    fn i_caratteri_si_contano_come_li_conta_una_persona() {
        // «perche'» con l'accento e' sette caratteri, non otto byte.
        let a = AppuntiFinti::default();
        assert_eq!(scrivi_appunti(&a, "perch\u{e9}").unwrap(),
                   "Copiati 6 caratteri negli appunti.");
    }

    #[test]
    fn dove_non_ce_il_sistema_lo_si_dice_invece_di_tacere() {
        let n = NienteSistema;
        let e = leggi_appunti(&n).unwrap_err();
        assert!(e.contains("non e' un errore della richiesta")
                || e.contains("Non e' un errore della richiesta"), "{e}");
        assert!(volume(&n, Some(50), None).is_err());
        assert!(informazioni(&n).unwrap_err().contains("com'e' fatto il PC"));
    }

    /// Una tastiera finta che puo' **cambiare il fuoco sotto i piedi**.
    ///
    /// E' il caso per cui la guardia esiste, e su una macchina di
    /// compilazione non si riproduce: serve una finta che lo faccia apposta.
    struct TastieraFinta {
        davanti: Option<Finestra>,
        /// Se vero, il fuoco «si sposta» fra lo sguardo e l'invio.
        scappa: bool,
        premuto: RefCell<Vec<(String, i64)>>,
    }

    impl TastieraFinta {
        fn con(davanti: Option<Finestra>, scappa: bool) -> Self {
            TastieraFinta {
                davanti,
                scappa,
                premuto: RefCell::new(Vec::new()),
            }
        }
    }

    impl Tastiera for TastieraFinta {
        fn davanti(&self) -> Result<Option<Finestra>, String> {
            Ok(self.davanti.clone())
        }
        fn scrivi_dentro(&self, testo: &str, dove: i64) -> Result<(), String> {
            if self.scappa {
                return Err("il fuoco e' passato a «Altro» (altro.exe) mentre scrivevo".into());
            }
            self.premuto.borrow_mut().push((testo.to_string(), dove));
            Ok(())
        }
        fn premi_dentro(&self, tasti: &str, dove: i64) -> Result<(), String> {
            if self.scappa {
                return Err("non scrivo niente: il fuoco e' su «Altro» (altro.exe)".into());
            }
            self.premuto.borrow_mut().push((tasti.to_string(), dove));
            Ok(())
        }
    }

    fn blocco() -> Finestra {
        Finestra {
            handle: 42,
            titolo: "Senza titolo - Blocco note".into(),
            processo: "notepad.exe".into(),
        }
    }

    #[test]
    fn senza_nessuno_davanti_non_si_preme_niente() {
        let t = TastieraFinta::con(None, false);
        let e = digita(&t, "ciao", "ui.focus").unwrap_err();
        assert!(
            e.contains("non premo niente") && e.contains("«ui.focus»"),
            "{e}"
        );
        assert!(premi(&t, "ctrl+s", "ui.focus").is_err());
        assert!(t.premuto.borrow().is_empty(), "ha premuto lo stesso");
    }

    #[test]
    fn si_scrive_legati_alla_finestra_guardata_e_la_si_nomina() {
        let t = TastieraFinta::con(Some(blocco()), false);
        let r = digita(&t, "perch\u{e9}", "ui.focus").unwrap();
        assert_eq!(
            r,
            "Digitati 6 caratteri in «Senza titolo - Blocco note» (notepad.exe)."
        );
        assert_eq!(
            t.premuto.borrow()[0].1,
            42,
            "il bersaglio e' quello guardato"
        );
        let r = premi(&t, "ctrl+s", "ui.focus").unwrap();
        assert!(
            r.ends_with("in «Senza titolo - Blocco note» (notepad.exe)."),
            "{r}"
        );
    }

    #[test]
    fn se_il_fuoco_scappa_a_meta_e_un_guasto_non_un_fatto() {
        // Il difetto che c'era dall'altra parte: un guasto a meta' veniva
        // preso per «il binario non c'e'» e si ripiegava su un'altra
        // strada, che riscriveva **tutto** il testo nella finestra che
        // intanto aveva preso il fuoco.
        let t = TastieraFinta::con(Some(blocco()), true);
        let e = digita(&t, "ciao", "ui.focus").unwrap_err();
        assert!(e.contains("mentre scrivevo"), "il motivo va detto: {e}");
        assert!(premi(&t, "ctrl+s", "ui.focus").is_err());
    }

    #[test]
    fn lanteprima_dice_quale_finestra_o_che_non_lo_sa() {
        let testa = anteprima_digita("ciao");
        assert_eq!(
            con_la_finestra(&testa, Some(&blocco())),
            "Digita nella finestra che ha il fuoco: ciao\n  \
             La finestra e': «Senza titolo - Blocco note» (notepad.exe)"
        );
        assert!(con_la_finestra(&testa, None).ends_with("— non riesco a dire quale sia"));
        assert_eq!(anteprima_digita(&"x".repeat(500)).chars().count(), 39 + 200);
    }

    #[test]
    fn il_volume_vuole_sapere_cosa_fare() {
        let n = NienteSistema;
        assert_eq!(volume(&n, None, None).unwrap_err(), "serve 'level' o 'mute'");
    }
}
