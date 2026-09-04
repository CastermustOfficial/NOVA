//! Il vault su disco: leggere, accorgersi di cosa e' cambiato, non perdere
//! nessuno per strada.
//!
//! E' il primo pezzo del cantiere che incontra davvero il filesystem, e per
//! questo il filesystem sta **dietro un tratto**. Non e' astrazione per il
//! gusto: la parte difficile di questo modulo non e' leggere un file, e' la
//! macchina a stati che decide chi ricaricare, chi dimenticare e chi
//! rivendica quale slug. Quella macchina si prova con un disco finto, in un
//! millisecondo e senza toccare niente; con un disco vero si proverebbero
//! tre casi su venti e si spererebbe.
//!
//! **I percorsi sono relativi alla radice, con la barra normale.** In Python
//! l'identita' di un file passa da `Path.resolve()`, che segue i punti di
//! reinnesto: e' esattamente cio' che D56 dice di non fare. La domanda vera
//! e' «e' questo il file, dentro questa cartella?», e a quella si risponde
//! **coi nomi**. Un percorso relativo normalizzato *e'* gia' la risposta, e
//! non costa nessuna chiamata al sistema.

use std::collections::BTreeMap;

use crate::{slug, Nodo};

/// Quanto si aspetta prima di riguardare il disco, in secondi.
///
/// Serve perche' il controllo sta sul percorso critico di ogni messaggio: su
/// un vault da 139 note vuol dire fare lo stat di 139 file, misurati in
/// **ventidue millisecondi su ventisei** del costo della memoria per turno.
///
/// Il valore e' un compromesso dichiarato: chi corregge una nota in Obsidian
/// vuole che NOVA se ne accorga, ma «subito» ed «entro due secondi» sono la
/// stessa cosa per una persona, mentre rileggere la cartella una volta o a
/// ogni frase non lo sono affatto.
pub const ATTESA_RILETTURA_S: f64 = 2.0;

/// Se sia ora di riguardare il disco.
///
/// L'orologio sta fuori, come in `a_markdown`: una funzione che se lo legge
/// da sola non si prova due volte con lo stesso risultato.
pub fn ora_di_riguardare(ultima_lettura: f64, adesso: f64, forza: bool) -> bool {
    // `forza` e' per chi ha appena scritto e vuole rileggere quel che ha
    // scritto: li' aspettare due secondi e' un difetto, non un risparmio.
    forza || (adesso - ultima_lettura) >= ATTESA_RILETTURA_S
}

/// Quanto basta per dire «questo file e' cambiato».
///
/// Data **e** dimensione. Su NTFS il timestamp avanza a scatti di circa
/// quindici millisecondi: due scritture dentro lo stesso scatto sarebbero
/// indistinguibili, e la seconda non verrebbe mai riletta.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Impronta {
    pub quando: f64,
    pub quanto: i64,
}

/// Il disco, ridotto a cio' che serve al vault.
///
/// Tre domande, tutte in percorsi **relativi alla radice** con la barra
/// normale: chi implementa questo tratto e' l'unico che deve sapere com'e'
/// fatto un percorso su questo sistema operativo.
pub trait Disco {
    /// I file del vault, in ordine stabile. L'ordine conta: quando due file
    /// si contendono uno slug vince chi arriva prima, e «prima» dev'essere la
    /// stessa cosa a ogni avvio.
    fn elenca(&self) -> Vec<String>;
    fn impronta(&self, dove: &str) -> Option<Impronta>;
    fn leggi(&self, dove: &str) -> Option<String>;
}

/// L'identita' di un file dentro il vault.
///
/// Minuscolo perche' Windows non distingue le maiuscole nei nomi; ma la
/// scelta e' del chiamante, non di questa funzione, e per questo la
/// distinzione e' un parametro: su un filesystem che distingue,
/// `A.md` e `a.md` sono due file, e appiattirli vorrebbe dire farne sparire
/// uno in silenzio.
pub fn chiave(percorso_relativo: &str, distingue_maiuscole: bool) -> String {
    let normale = percorso_relativo.replace('\\', "/");
    let normale = normale.trim_start_matches("./");
    if distingue_maiuscole {
        normale.to_string()
    } else {
        normale.to_lowercase()
    }
}

/// Se un file va letto come nodo.
///
/// Si saltano i nomi che cominciano per trattino basso — `_INDICE.md` lo
/// scrive NOVA e rileggerlo vorrebbe dire trasformare l'indice in un nodo che
/// finisce nell'indice — e tutto cio' che sta sotto una cartella col punto
/// davanti, che e' dove vivono l'audit e le cose di servizio.
pub fn da_leggere(percorso_relativo: &str) -> bool {
    let normale = percorso_relativo.replace('\\', "/");
    if !normale.to_lowercase().ends_with(".md") {
        return false;
    }
    let pezzi: Vec<&str> = normale.split('/').filter(|p| !p.is_empty()).collect();
    let Some((file, cartelle)) = pezzi.split_last() else {
        return false;
    };
    if cartelle.iter().any(|c| c.starts_with('.')) {
        return false;
    }
    !file.starts_with('_') && !file.starts_with('.')
}

/// Cosa e' successo all'ultimo giro. Serve a chi guarda, non al vault.
#[derive(Debug, Default, PartialEq)]
pub struct Cambiamenti {
    pub letti: Vec<String>,
    pub spariti: Vec<String>,
}

/// I nodi del vault e chi li tiene.
#[derive(Debug, Default)]
pub struct Deposito {
    nodi: BTreeMap<String, Nodo>,
    impronte: BTreeMap<String, Impronta>,
    /// chiave del file -> slug che ci ha trovato dentro. Vuoto se quel file
    /// ha perso una contesa.
    proprietario: BTreeMap<String, String>,
    /// slug -> chiave del file che lo contiene.
    casa: BTreeMap<String, String>,
    /// slug conteso -> i file che se lo contendono, in ordine.
    pub collisioni: BTreeMap<String, Vec<String>>,
    /// chiave -> il percorso come si scrive. La chiave e' minuscola su
    /// Windows, e in un messaggio all'utente il nome va reso come l'ha
    /// scritto lui.
    originale: BTreeMap<String, String>,
    distingue_maiuscole: bool,
}

impl Deposito {
    /// `distingue_maiuscole` va messo a `true` sui filesystem che lo fanno
    /// (Linux, macOS con APFS sensibile). Su Windows resta `false`.
    pub fn nuovo(distingue_maiuscole: bool) -> Self {
        Deposito { distingue_maiuscole, ..Default::default() }
    }

    pub fn quanti(&self) -> usize {
        self.nodi.len()
    }

    pub fn prendi(&self, slug: &str) -> Option<&Nodo> {
        self.nodi.get(slug)
    }

    pub fn tutti(&self) -> impl Iterator<Item = &Nodo> {
        self.nodi.values()
    }

    /// Dove sta il file di un nodo, relativo alla radice.
    pub fn dove(&self, slug: &str) -> Option<&str> {
        let k = self.casa.get(slug)?;
        self.originale.get(k).map(|s| s.as_str())
    }

    /// Rilegge tutto da capo, buttando via quello che sapeva.
    pub fn ricarica(&mut self, disco: &dyn Disco) -> usize {
        self.nodi.clear();
        self.impronte.clear();
        self.proprietario.clear();
        self.casa.clear();
        self.collisioni.clear();
        self.originale.clear();
        for dove in disco.elenca() {
            if da_leggere(&dove) {
                self.carica(disco, &dove);
            }
        }
        self.nodi.len()
    }

    /// Rilegge solo cio' che qualcuno ha toccato fuori da NOVA.
    pub fn aggiorna(&mut self, disco: &dyn Disco) -> Cambiamenti {
        let mut esito = Cambiamenti::default();
        let mut visti: Vec<String> = Vec::new();
        for dove in disco.elenca() {
            if !da_leggere(&dove) {
                continue;
            }
            let k = chiave(&dove, self.distingue_maiuscole);
            visti.push(k.clone());
            if self.impronte.get(&k) != disco.impronta(&dove).as_ref() {
                self.carica(disco, &dove);
                esito.letti.push(dove);
            }
        }
        let spariti: Vec<String> = self
            .impronte
            .keys()
            .filter(|k| !visti.contains(k))
            .cloned()
            .collect();
        let mut da_riaprire: Vec<String> = Vec::new();
        for k in spariti {
            self.impronte.remove(&k);
            self.originale.remove(&k);
            let slug = self.proprietario.remove(&k).unwrap_or_default();
            // Il nodo se ne va solo se **nessun altro file** lo rivendica:
            // altrimenti cancellare un doppione porterebbe via anche
            // l'originale.
            if !slug.is_empty() && !self.proprietario.values().any(|s| *s == slug) {
                self.nodi.remove(&slug);
                self.casa.remove(&slug);
                esito.spariti.push(slug.clone());
            }
            if self.collisioni.contains_key(&slug) {
                da_riaprire.push(slug);
            }
            if let Some(conteso) = self.slug_conteso(&k) {
                da_riaprire.push(conteso);
            }
        }
        // Se sparisce uno dei due file in lite, la contesa va **riaperta**:
        // altrimenti chi aveva perso resta invisibile pur essendo rimasto
        // l'unico, ed e' di nuovo un nodo che sparisce senza dirlo.
        da_riaprire.sort();
        da_riaprire.dedup();
        for slug in da_riaprire {
            for dove in self.collisioni.remove(&slug).unwrap_or_default() {
                if disco.impronta(&dove).is_none() {
                    continue;
                }
                let k = chiave(&dove, self.distingue_maiuscole);
                self.impronte.remove(&k);
                self.carica(disco, &dove);
                esito.letti.push(dove);
            }
        }
        esito.letti.sort();
        esito.letti.dedup();
        esito.spariti.sort();
        esito
    }

    fn slug_conteso(&self, k: &str) -> Option<String> {
        for (slug, percorsi) in &self.collisioni {
            for dove in percorsi {
                if chiave(dove, self.distingue_maiuscole) == k {
                    return Some(slug.clone());
                }
            }
        }
        None
    }

    /// Legge un file e ci mette dentro un nodo. Se lo slug e' gia' occupato
    /// da un **altro** file, la cosa viene registrata come contesa invece di
    /// far sparire uno dei due.
    pub fn carica(&mut self, disco: &dyn Disco, dove: &str) {
        let Some(testo) = disco.leggi(dove) else {
            return;
        };
        let k = chiave(dove, self.distingue_maiuscole);
        let radice_nome = nome_senza_estensione(dove);
        let mut nodo = Nodo::da_markdown(&testo, &radice_nome);
        // Lo slug passa comunque dal filtro: un file scritto a mano
        // («Progetto Nova.md») resterebbe altrimenti irraggiungibile, perche'
        // chi lo cerca slugifica sempre.
        nodo.slug = slug(if nodo.slug.is_empty() { &radice_nome } else { &nodo.slug });
        let Some(impronta) = disco.impronta(dove) else {
            return;
        };
        if let Some(casa_attuale) = self.casa.get(&nodo.slug) {
            if casa_attuale != &k {
                // Due file diversi rivendicano lo stesso slug. Vince chi e'
                // arrivato prima — l'ordine e' stabile — ma la cosa si dice,
                // invece di far sparire un nodo in silenzio.
                let mut in_lite = vec![
                    self.originale.get(casa_attuale).cloned()
                        .unwrap_or_else(|| casa_attuale.clone()),
                    dove.to_string(),
                ];
                in_lite.sort();
                in_lite.dedup();
                self.collisioni.insert(nodo.slug.clone(), in_lite);
                // L'impronta si segna lo stesso: senza, questo file verrebbe
                // riletto a ogni giro per scoprire ogni volta che ha perso.
                self.impronte.insert(k.clone(), impronta);
                self.originale.insert(k.clone(), dove.to_string());
                self.proprietario.insert(k, String::new());
                return;
            }
        }
        self.casa.insert(nodo.slug.clone(), k.clone());
        self.proprietario.insert(k.clone(), nodo.slug.clone());
        self.originale.insert(k.clone(), dove.to_string());
        self.impronte.insert(k, impronta);
        self.nodi.insert(nodo.slug.clone(), nodo);
    }

    /// Segna che un file e' stato scritto da NOVA, cosi' il giro dopo non lo
    /// rilegge credendo che l'abbia toccato l'utente.
    pub fn segna_scritto(&mut self, dove: &str, impronta: Impronta, slug: &str) {
        let k = chiave(dove, self.distingue_maiuscole);
        self.casa.insert(slug.to_string(), k.clone());
        self.proprietario.insert(k.clone(), slug.to_string());
        self.impronte.insert(k, impronta);
    }

    pub fn metti(&mut self, nodo: Nodo) {
        self.nodi.insert(nodo.slug.clone(), nodo);
    }
}

/// Il nome del file senza cartelle e senza `.md`, che e' il ripiego per il
/// titolo e per lo slug quando il frontmatter non li dice.
pub fn nome_senza_estensione(percorso: &str) -> String {
    let normale = percorso.replace('\\', "/");
    let ultimo = normale.rsplit('/').next().unwrap_or(&normale);
    match ultimo.rfind('.') {
        Some(i) if i > 0 => ultimo[..i].to_string(),
        _ => ultimo.to_string(),
    }
}

#[cfg(test)]
mod prove {
    use super::*;
    use std::cell::RefCell;

    /// Un disco finto: una cartella in memoria. E' tutto il motivo per cui il
    /// disco sta dietro un tratto — qui si scrivono in tre righe i casi che
    /// su un disco vero costerebbero mezz'ora e non sarebbero ripetibili.
    #[derive(Default)]
    struct DiscoFinto {
        file: RefCell<Vec<(String, String, Impronta)>>,
    }

    impl DiscoFinto {
        fn con(elenco: &[(&str, &str)]) -> Self {
            let d = DiscoFinto::default();
            for (dove, testo) in elenco {
                d.metti(dove, testo, 1.0);
            }
            d
        }
        fn metti(&self, dove: &str, testo: &str, quando: f64) {
            let imp = Impronta { quando, quanto: testo.len() as i64 };
            let mut f = self.file.borrow_mut();
            if let Some(v) = f.iter_mut().find(|(d, _, _)| d == dove) {
                *v = (dove.to_string(), testo.to_string(), imp);
            } else {
                f.push((dove.to_string(), testo.to_string(), imp));
            }
        }
        fn togli(&self, dove: &str) {
            self.file.borrow_mut().retain(|(d, _, _)| d != dove);
        }
    }

    impl Disco for DiscoFinto {
        fn elenca(&self) -> Vec<String> {
            let mut v: Vec<String> = self.file.borrow().iter().map(|(d, _, _)| d.clone()).collect();
            v.sort();
            v
        }
        fn impronta(&self, dove: &str) -> Option<Impronta> {
            self.file.borrow().iter().find(|(d, _, _)| d == dove).map(|(_, _, i)| *i)
        }
        fn leggi(&self, dove: &str) -> Option<String> {
            self.file.borrow().iter().find(|(d, _, _)| d == dove).map(|(_, t, _)| t.clone())
        }
    }

    fn nota(titolo: &str) -> String {
        format!("---\ntitle: {titolo}\ntipo: fatto\n---\n\nCorpo di {titolo}.\n")
    }

    #[test]
    fn si_leggono_solo_i_file_che_sono_nodi() {
        assert!(da_leggere("02-persone/persona-anna.md"));
        assert!(!da_leggere("_INDICE.md"), "l'indice diventerebbe un nodo dell'indice");
        assert!(!da_leggere(".nova/audit.jsonl"));
        assert!(!da_leggere(".nova/qualcosa.md"), "le cartelle col punto sono di servizio");
        assert!(!da_leggere("02-persone/nota.txt"));
        assert!(!da_leggere("02-persone/_bozza.md"));
        assert!(da_leggere("02-persone\\persona-anna.md"), "anche con la barra di Windows");
    }

    #[test]
    fn la_chiave_e_sui_nomi_e_non_su_dove_portano() {
        // D56: la domanda e' «e' questo file, dentro questa cartella?», e a
        // quella si risponde coi nomi. Niente resolve, niente syscall.
        assert_eq!(chiave("02-persone\\Anna.md", false), "02-persone/anna.md");
        assert_eq!(chiave("./02-persone/Anna.md", true), "02-persone/Anna.md");
        // Su un filesystem che distingue, due file restano due file.
        assert_ne!(chiave("A.md", true), chiave("a.md", true));
        assert_eq!(chiave("A.md", false), chiave("a.md", false));
    }

    #[test]
    fn si_riguarda_il_disco_ogni_due_secondi_ma_subito_se_lo_si_chiede() {
        assert!(!ora_di_riguardare(100.0, 101.0, false));
        assert!(ora_di_riguardare(100.0, 102.0, false));
        assert!(ora_di_riguardare(100.0, 100.1, true), "chi ha appena scritto non aspetta");
    }

    #[test]
    fn si_legge_il_vault_e_lo_si_ritrova_per_slug() {
        let d = DiscoFinto::con(&[
            ("02-persone/persona-anna.md", &nota("Anna")),
            ("06-fatti/fatto-caffe.md", &nota("Caffe")),
            ("_INDICE.md", "# indice"),
            (".nova/audit.jsonl", "{}"),
        ]);
        let mut v = Deposito::nuovo(false);
        assert_eq!(v.ricarica(&d), 2, "l'indice o l'audit sono entrati come nodi");
        assert!(v.prendi("persona-anna").is_some());
        assert_eq!(v.dove("persona-anna"), Some("02-persone/persona-anna.md"));
    }

    #[test]
    fn un_file_scritto_a_mano_resta_raggiungibile() {
        // «Progetto Nova.md» senza frontmatter: chi lo cerca slugifica sempre,
        // quindi lo slug deve passare dal filtro o il nodo e' invisibile.
        let d = DiscoFinto::con(&[("03-progetti/Progetto Nova.md", "Solo del testo.")]);
        let mut v = Deposito::nuovo(false);
        v.ricarica(&d);
        assert!(v.prendi("progetto-nova").is_some(), "{:?}", v.tutti().map(|n| &n.slug).collect::<Vec<_>>());
    }

    #[test]
    fn si_rilegge_solo_cio_che_e_cambiato() {
        let d = DiscoFinto::con(&[
            ("a.md", &nota("A")),
            ("b.md", &nota("B")),
        ]);
        let mut v = Deposito::nuovo(false);
        v.ricarica(&d);
        assert_eq!(v.aggiorna(&d), Cambiamenti::default(), "ha riletto senza motivo");
        d.metti("b.md", &nota("B corretta a mano"), 2.0);
        let c = v.aggiorna(&d);
        assert_eq!(c.letti, vec!["b.md"]);
        assert!(c.spariti.is_empty());
    }

    #[test]
    fn un_file_cancellato_porta_via_il_suo_nodo() {
        let d = DiscoFinto::con(&[("a.md", &nota("A")), ("b.md", &nota("B"))]);
        let mut v = Deposito::nuovo(false);
        v.ricarica(&d);
        d.togli("b.md");
        let c = v.aggiorna(&d);
        assert_eq!(c.spariti, vec!["b"]);
        assert_eq!(v.quanti(), 1);
        assert!(v.prendi("b").is_none());
    }

    #[test]
    fn due_file_che_si_contendono_uno_slug_non_ne_fanno_sparire_uno_in_silenzio() {
        // La contesa nasce dal **nome del file**, non dal frontmatter: lo
        // slug lo da' il nome, sempre. Due `doppio.md` in due cartelle
        // diverse sono un nodo solo per chi cerca, e due file per il disco.
        let corpo = &nota("Doppio");
        let d = DiscoFinto::con(&[("a/doppio.md", corpo), ("b/doppio.md", corpo)]);
        let mut v = Deposito::nuovo(false);
        v.ricarica(&d);
        assert_eq!(v.quanti(), 1);
        assert!(v.collisioni.contains_key("doppio"), "la lite non e' stata registrata");
        assert_eq!(v.collisioni["doppio"].len(), 2);
    }

    #[test]
    fn e_se_uno_dei_due_sparisce_laltro_torna_visibile() {
        // Il difetto che questa prova insegue: chi aveva perso la contesa
        // resta invisibile anche quando e' rimasto l'unico. Un nodo che
        // sparisce senza che nessuno lo dica.
        let corpo = &nota("Doppio");
        let d = DiscoFinto::con(&[("a/doppio.md", corpo), ("b/doppio.md", corpo)]);
        let mut v = Deposito::nuovo(false);
        v.ricarica(&d);
        assert_eq!(v.dove("doppio"), Some("a/doppio.md"));

        d.togli("a/doppio.md");
        v.aggiorna(&d);
        assert!(v.prendi("doppio").is_some(), "il superstite e' rimasto invisibile");
        assert_eq!(v.dove("doppio"), Some("b/doppio.md"));
        assert!(v.collisioni.is_empty(), "la lite e' finita ma risulta ancora aperta");
    }

    #[test]
    fn un_file_che_ha_perso_non_si_rilegge_a_ogni_giro() {
        // Senza segnare l'impronta del perdente, ogni `aggiorna` lo
        // rileggerebbe per riscoprire ogni volta che ha perso: su un vault
        // con un doppione, lavoro inutile a ogni messaggio.
        let corpo = &nota("Doppio");
        let d = DiscoFinto::con(&[("a/doppio.md", corpo), ("b/doppio.md", corpo)]);
        let mut v = Deposito::nuovo(false);
        v.ricarica(&d);
        assert_eq!(v.aggiorna(&d), Cambiamenti::default());
    }

    #[test]
    fn il_nome_del_file_e_il_ripiego_per_il_titolo() {
        assert_eq!(nome_senza_estensione("02-persone/persona-anna.md"), "persona-anna");
        assert_eq!(nome_senza_estensione("a\\b\\Nota Lunga.md"), "Nota Lunga");
        assert_eq!(nome_senza_estensione("senza-estensione"), "senza-estensione");
        assert_eq!(nome_senza_estensione(".nascosto"), ".nascosto");
    }
}
