//! Cosa NOVA puo' fare senza chiedere, e cosa non puo' fare affatto.
//!
//! Tre domande, e nessuna delle tre e' una formalita': dove si puo'
//! scrivere, quali comandi non si eseguono mai, e quando ci si ferma a
//! chiedere il permesso.
//!
//! La prima delle tre, in Python, era sbagliata in **tre modi insieme**, e
//! tutti e tre erano la stessa lezione gia' scritta (D56): confrontava un
//! percorso passato da `resolve()` con dei percorsi protetti non risolti,
//! cioe' due spazi diversi; attaccava una barra rovescia a mano, quindi fuori
//! da Windows non scattava mai; e per le cartelle autorizzate confrontava
//! senza separatore, cosi' autorizzare `C:\dati` autorizzava anche
//! `C:\dati-altrui`. L'ultimo l'ho dimostrato con due cartelle e cinque
//! righe.

use regex::RegexBuilder;

/// Quanto NOVA puo' fare da sola.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Autonomia {
    /// Non chiede mai. Chi la sceglie sa cosa sta scegliendo.
    Tutto,
    /// Chiede per tutto, anche per leggere un file.
    Chiedi,
    /// Chiede solo per cio' che e' distruttivo o arbitrario.
    ChiediSeRischioso,
}

impl Autonomia {
    /// I nomi esatti che stanno nella configurazione.
    ///
    /// Sono questi tre e non altri: `always_ask` non e' `ask_all`. Avevo
    /// scritto il secondo, e il ripiego prudente lo nascondeva — chi aveva
    /// chiesto «conferma sempre» si ritrovava «conferma solo se rischioso»,
    /// senza un errore da nessuna parte. **Un ripiego indulgente maschera un
    /// valore sbagliato**, e piu' e' ragionevole meglio lo maschera.
    pub const NOMI: &'static [(&'static str, Autonomia)] = &[
        ("autonomous", Autonomia::Tutto),
        ("always_ask", Autonomia::Chiedi),
        ("ask_risky", Autonomia::ChiediSeRischioso),
    ];

    /// Il nome capito, oppure `None`: chi chiama decide cosa farne, e cosi'
    /// puo' accorgersi che non l'ha capito.
    pub fn capisci(s: &str) -> Option<Autonomia> {
        Self::NOMI.iter().find(|(n, _)| *n == s).map(|(_, a)| *a)
    }

    /// Il nome, col ripiego piu' prudente dei tre che abbiano senso: una
    /// configurazione illeggibile non deve diventare «fai pure».
    pub fn dal_nome(s: &str) -> Autonomia {
        Self::capisci(s).unwrap_or(Autonomia::ChiediSeRischioso)
    }
}

/// «Questo percorso sta dentro quella cartella?», risposta **sui nomi**.
///
/// Non su dove portano: `resolve()` segue i punti di reinnesto, e ci sono
/// sistemi dove lo stesso file risponde da due posti. La domanda vera e' «se
/// cancello questa cartella sparisce anche questo file?», e chi cancella una
/// cartella cancella i nomi che ci stanno sotto.
///
/// Il separatore in fondo e' obbligatorio: senza, `NOVA-vecchio` risulta
/// dentro `NOVA`.
pub fn dentro(figlio: &str, cartella: &str) -> bool {
    let a = normalizza(figlio);
    let b = normalizza(cartella);
    if b.is_empty() {
        return false;
    }
    if a == b {
        return true;
    }
    let b_senza = b.trim_end_matches(['/', '\\']);
    a.starts_with(&format!("{b_senza}/"))
}

/// Un percorso in forma confrontabile: separatori uniformi, `.` e `..`
/// risolti **a nome**, maiuscole appiattite.
///
/// Le maiuscole si appiattiscono sempre. E' la scelta prudente e non e'
/// gratis: su un filesystem che le distingue, `Dati` e `dati` sono due
/// cartelle e qui risultano la stessa. In una guardia si sbaglia verso il no,
/// e il no qui vuol dire «considerala protetta».
pub fn normalizza(p: &str) -> String {
    let uniforme = p.replace('\\', "/");
    let mut pezzi: Vec<&str> = Vec::new();
    for pezzo in uniforme.split('/') {
        match pezzo {
            "" | "." => {}
            ".." => {
                pezzi.pop();
            }
            altro => pezzi.push(altro),
        }
    }
    let radice = if uniforme.starts_with('/') { "/" } else { "" };
    format!("{radice}{}", pezzi.join("/")).to_lowercase()
}

/// Una stringa come la scriverebbe il `repr` di Python.
///
/// Apici singoli, e **le barre rovesce raddoppiate**. Il raddoppio e' un
/// artefatto di Python che trapela in un messaggio che leggono il modello e,
/// attraverso lui, l'utente: `C:\\dati` invece di `C:\dati`. Si riproduce
/// perche' due meta' che dicono la stessa cosa con due punteggiature diverse
/// sono due voci — e perche' un messaggio d'errore e' un contratto quanto un
/// formato di file. Se un giorno si pulisce, si pulisce da tutte e due le
/// parti insieme.
fn come_repr(s: &str) -> String {
    let mut fuori = String::with_capacity(s.len() + 2);
    fuori.push('\'');
    for c in s.chars() {
        match c {
            '\\' => fuori.push_str("\\\\"),
            '\'' => fuori.push_str("\\'"),
            '\n' => fuori.push_str("\\n"),
            '\t' => fuori.push_str("\\t"),
            c => fuori.push(c),
        }
    }
    fuori.push('\'');
    fuori
}

/// Le regole di sicurezza, gia' compilate.
pub struct Guardie {
    protetti: Vec<String>,
    radici: Vec<String>,
    vietati: Vec<regex::Regex>,
    /// I motivi che non si e' riusciti a compilare. **Non si buttano in
    /// silenzio**: un motivo vietato che sparisce e' un buco nella lista dei
    /// divieti, e chi l'aveva scritto crede di essere protetto. Il Python
    /// qui fa `except re.error: continue` e non lo dice a nessuno.
    pub motivi_incomprensibili: Vec<String>,
    pub autonomia: Autonomia,
}

/// Cosa impedisce di fare una cosa. Il messaggio e' quello che legge il
/// modello, e attraverso lui l'utente.
#[derive(Debug, PartialEq)]
pub enum Divieto {
    PercorsoProtetto(String),
    FuoriDalleCartelle(String),
    ComandoBloccato(String),
}

impl Divieto {
    pub fn messaggio(&self) -> String {
        match self {
            Divieto::PercorsoProtetto(p) => format!(
                "percorso protetto: {p}. Modificalo manualmente se necessario."
            ),
            Divieto::FuoriDalleCartelle(r) => format!(
                "scrittura non consentita fuori dalle cartelle autorizzate: {r}"
            ),
            Divieto::ComandoBloccato(m) => {
                format!("comando bloccato dalle regole di sicurezza (pattern: {m})")
            }
        }
    }
}

impl Guardie {
    pub fn nuove(
        protetti: &[String],
        radici: &[String],
        vietati: &[String],
        autonomia: Autonomia,
    ) -> Guardie {
        let mut compilati = Vec::new();
        let mut incomprensibili = Vec::new();
        for m in vietati {
            // `case_insensitive` come il `re.IGNORECASE` del Python: i comandi
            // si scrivono come capita, e «DISKPART» e' `diskpart`.
            match RegexBuilder::new(m).case_insensitive(true).build() {
                Ok(r) => compilati.push(r),
                Err(_) => incomprensibili.push(m.clone()),
            }
        }
        Guardie {
            protetti: protetti.to_vec(),
            radici: radici.to_vec(),
            vietati: compilati,
            motivi_incomprensibili: incomprensibili,
            autonomia,
        }
    }

    /// Se NOVA puo' scrivere qui.
    pub fn puo_scrivere(&self, percorso: &str) -> Result<(), Divieto> {
        for prot in &self.protetti {
            if dentro(percorso, prot) {
                return Err(Divieto::PercorsoProtetto(percorso.to_string()));
            }
        }
        // Nessuna radice dichiarata vuol dire «tutto il disco tranne i
        // protetti»: e' la configurazione predefinita, e restringerla di
        // nascosto renderebbe NOVA inutile senza dirlo.
        if !self.radici.is_empty() && !self.radici.iter().any(|r| dentro(percorso, r)) {
            let elenco: Vec<String> = self.radici.iter().map(|r| come_repr(r)).collect();
            return Err(Divieto::FuoriDalleCartelle(format!("[{}]", elenco.join(", "))));
        }
        Ok(())
    }

    /// Se questo comando si puo' eseguire.
    pub fn comando_permesso(&self, comando: &str) -> Result<(), Divieto> {
        for (r, testo) in self.vietati.iter().zip(self.motivi_compilati()) {
            if r.is_match(comando) {
                return Err(Divieto::ComandoBloccato(testo));
            }
        }
        Ok(())
    }

    fn motivi_compilati(&self) -> Vec<String> {
        self.vietati.iter().map(|r| r.as_str().to_string()).collect()
    }

    /// Se prima di fare questa cosa bisogna chiedere.
    pub fn serve_permesso(&self, rischio: crate::Rischio) -> bool {
        match self.autonomia {
            Autonomia::Tutto => false,
            Autonomia::Chiedi => true,
            Autonomia::ChiediSeRischioso => rischio >= crate::Rischio::Pericoloso,
        }
    }
}

#[cfg(test)]
mod prove {
    use super::*;
    use crate::Rischio;

    #[test]
    fn una_cartella_autorizzata_non_ne_autorizza_unaltra_che_le_somiglia() {
        // Il difetto dimostrato il 4 settembre: `write_roots = [C:\dati]`
        // lasciava scrivere in `C:\dati-altrui`.
        assert!(dentro(r"C:\dati\mio.txt", r"C:\dati"));
        assert!(dentro(r"C:\dati\sotto\mio.txt", r"C:\dati"));
        assert!(!dentro(r"C:\dati-altrui\tuo.txt", r"C:\dati"));
        assert!(!dentro(r"C:\datix\tuo.txt", r"C:\dati"));
    }

    #[test]
    fn la_domanda_si_fa_sui_nomi_e_funziona_con_tutte_e_due_le_barre() {
        assert!(dentro("C:/dati/mio.txt", r"C:\dati"));
        assert!(dentro("/home/gio/dati/x", "/home/gio/dati"));
        assert!(dentro(r"C:\dati\.\mio.txt", r"C:\dati"));
        // Un `..` si scioglie **a nome**, senza chiedere niente al disco.
        assert!(!dentro(r"C:\dati\..\fuori.txt", r"C:\dati"));
        assert!(dentro(r"C:\dati\sotto\..\mio.txt", r"C:\dati"));
    }

    #[test]
    fn una_cartella_e_dentro_se_stessa() {
        assert!(dentro(r"C:\dati", r"C:\dati"));
        assert!(dentro(r"C:\dati\", r"C:\dati"));
    }

    #[test]
    fn senza_radici_si_scrive_ovunque_tranne_nei_protetti() {
        let g = Guardie::nuove(
            &[r"C:\Windows".into()],
            &[],
            &[],
            Autonomia::ChiediSeRischioso,
        );
        assert!(g.puo_scrivere(r"C:\Users\gio\nota.txt").is_ok());
        assert!(g.puo_scrivere(r"C:\Windows\system32\x.dll").is_err());
        // E `C:\Windows-mio` non e' dentro `C:\Windows`.
        assert!(g.puo_scrivere(r"C:\Windows-mio\x.txt").is_ok());
    }

    #[test]
    fn i_comandi_vietati_si_riconoscono_comunque_scritti() {
        let g = Guardie::nuove(
            &[],
            &[],
            &[r"\bdiskpart\b".into(), r"\bformat\s+[a-z]:".into()],
            Autonomia::Tutto,
        );
        assert!(g.comando_permesso("dir /w").is_ok());
        assert!(g.comando_permesso("DISKPART /s script.txt").is_err());
        assert!(g.comando_permesso("format c:").is_err());
        // «formattazione» non e' «format c:»
        assert!(g.comando_permesso("spiega la formattazione del disco").is_ok());
    }

    #[test]
    fn un_motivo_che_non_si_capisce_non_sparisce_in_silenzio() {
        // Il Python fa `except re.error: continue`: il divieto svanisce e chi
        // l'aveva scritto crede di essere protetto. Qui si dice.
        let g = Guardie::nuove(&[], &[], &[r"(?<=x)y".into(), r"\bdiskpart\b".into()],
                               Autonomia::Tutto);
        assert_eq!(g.motivi_incomprensibili.len(), 1, "{:?}", g.motivi_incomprensibili);
        assert!(g.comando_permesso("diskpart").is_err(), "gli altri devono valere");
    }

    #[test]
    fn il_permesso_dipende_da_quanta_autonomia_si_e_data() {
        let con = |a| Guardie::nuove(&[], &[], &[], a);
        assert!(!con(Autonomia::Tutto).serve_permesso(Rischio::Pericoloso));
        assert!(con(Autonomia::Chiedi).serve_permesso(Rischio::Innocuo));
        let prudente = con(Autonomia::ChiediSeRischioso);
        assert!(!prudente.serve_permesso(Rischio::Innocuo));
        assert!(!prudente.serve_permesso(Rischio::Modifica));
        assert!(prudente.serve_permesso(Rischio::Pericoloso));
    }

    #[test]
    fn unautonomia_che_non_si_capisce_non_diventa_fai_pure() {
        assert_eq!(Autonomia::dal_nome("boh"), Autonomia::ChiediSeRischioso);
        assert_eq!(Autonomia::dal_nome(""), Autonomia::ChiediSeRischioso);
    }

    #[test]
    fn i_tre_nomi_veri_si_capiscono_tutti() {
        // Avevo scritto `ask_all` invece di `always_ask`, e il ripiego
        // prudente lo nascondeva: chi chiedeva «conferma sempre» otteneva
        // «conferma solo se rischioso». Questa prova esiste perche' un nome
        // sbagliato non possa piu' passare per una scelta.
        assert_eq!(Autonomia::capisci("autonomous"), Some(Autonomia::Tutto));
        assert_eq!(Autonomia::capisci("always_ask"), Some(Autonomia::Chiedi));
        assert_eq!(Autonomia::capisci("ask_risky"), Some(Autonomia::ChiediSeRischioso));
        assert_eq!(Autonomia::capisci("ask_all"), None);
    }
}
