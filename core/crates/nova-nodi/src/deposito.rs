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

use crate::posto;
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

    /// Un nodo per slug.
    ///
    /// Il nome passa dal filtro prima di cercare, come dall'altra parte: chi
    /// chiede puo' avere in mano «Progetto Nova» invece di «progetto-nova» —
    /// e' cosi' che lo scrive il modello — e senza il filtro non troverebbe
    /// niente, senza nemmeno un errore.
    pub fn prendi(&self, slug_cercato: &str) -> Option<&Nodo> {
        self.nodi.get(&slug(slug_cercato))
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
        // Anche il nome come si scrive: `dove()` legge di qui, e dimenticarlo
        // faceva risultare senza casa un nodo appena scritto.
        self.originale.insert(k.clone(), dove.to_string());
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

/// Un disco su cui si puo' anche scrivere.
///
/// **Il contratto e' che la scrittura non possa restare a meta'**: di fianco
/// e poi si rinomina. Non e' un dettaglio di chi implementa, e' scritto qui
/// perche' e' la ragione per cui questo tratto esiste separato da `Disco`.
///
/// Il difetto che lo motiva e' stato pagato: il Python scriveva le note con
/// una `write_text`, cioe' apri-tronca-scrivi, e ci passa il thread che
/// impara dopo quasi ogni scambio. Un'interruzione fra il tronca e lo scrivi
/// lascia una nota **vuota** — che alla ricerca dopo c'e' ancora e non dice
/// piu' niente, senza un errore da nessuna parte.
pub trait DiscoScrivibile: Disco {
    /// Scrive il testo e torna l'impronta di quel che e' finito su disco.
    /// L'impronta serve subito: senza, il giro dopo il file risulterebbe
    /// «cambiato da fuori» e verrebbe riletto per niente.
    fn scrivi(&self, dove: &str, testo: &str) -> Result<Impronta, String>;
}

/// Chi decide se un testo puo' entrare in memoria.
///
/// Sta dietro un tratto e non dentro una funzione perche' `salva` **deve**
/// chiederglielo: e' l'unica porta da cui si scrive nel vault, ci passano
/// l'apprendimento automatico, le note scritte a mano e il seeding, e
/// chiudere una porta sola vuol dire chiuderla davvero. Se il controllo
/// stesse nel giudizio di chi chiama, basterebbe un chiamante distratto.
///
/// Torna il **nome** di cio' che ha trovato, mai il valore: un messaggio di
/// rifiuto finisce nei registri, e un registro che ripete la credenziale
/// appena rifiutata non ha protetto niente.
pub trait Guardiano {
    fn perche_non_si_salva(&self, testo: &str) -> Option<String>;
}

/// Il guardiano vero: quello che sa che aspetto ha un segreto.
///
/// Delega a `nova-guasti`, dove la domanda «questa cosa e' un segreto?» si fa
/// per tutti — chi maschera i messaggi d'errore e chi difende la memoria.
/// Erano due elenchi separati e sapevano cose diverse: uno conosceva il
/// `Bearer`, l'altro le chiavi AWS, e ognuno passava le proprie prove mentre
/// un segreto usciva dalla parte che non lo conosceva.
pub struct GuardianoDeiSegreti;

impl Guardiano for GuardianoDeiSegreti {
    fn perche_non_si_salva(&self, testo: &str) -> Option<String> {
        nova_guasti::guardiano::perche_non_si_salva(testo).map(|s| s.to_string())
    }
}

/// Un guardiano che lascia passare tutto. Esiste per le prove e per i casi in
/// cui il testo e' gia' stato controllato a monte; non e' il predefinito
/// apposta, perche' scriverlo deve costare una riga visibile.
pub struct NessunControllo;

impl Guardiano for NessunControllo {
    fn perche_non_si_salva(&self, _testo: &str) -> Option<String> {
        None
    }
}

impl Deposito {
    /// Riconosce un nodo gia' presente sotto un altro slug.
    ///
    /// Il modello scrive «knowledge-lab», la scansione aveva scritto
    /// «progetto-knowledge-lab»: senza questo controllo la memoria si sdoppia
    /// e le due meta' non si parlano piu'.
    ///
    /// Il controllo sui tipi non e' un di piu': senza, la persona «Marco» e
    /// il progetto «Marco» finivano nello stesso file, con un tipo solo e
    /// nella cartella sbagliata.
    pub fn stesso_nodo(&self, nodo: &Nodo) -> Option<String> {
        let titolo = nodo.title.trim().to_lowercase();
        let nudo = crate::fusione::senza_prefisso(&nodo.slug);
        for altro in self.nodi.values() {
            if altro.slug == nodo.slug {
                continue;
            }
            if !crate::fusione::tipi_compatibili(&nodo.tipo, &altro.tipo) {
                continue;
            }
            if !titolo.is_empty() && altro.title.trim().to_lowercase() == titolo {
                return Some(altro.slug.clone());
            }
            if !nudo.is_empty() && crate::fusione::senza_prefisso(&altro.slug) == nudo {
                return Some(altro.slug.clone());
            }
        }
        None
    }

    /// I tipi di tutti i nodi, per chi deve chiedere uno slug libero.
    fn tipi(&self) -> std::collections::HashMap<String, String> {
        self.nodi
            .iter()
            .map(|(s, n)| (s.clone(), n.tipo.clone()))
            .collect()
    }

    /// Scrive un nodo nel vault: l'unica porta.
    ///
    /// Nell'ordine: si chiede al guardiano; si cerca chi c'e' gia' (per slug,
    /// poi per somiglianza); lo si rilegge dal disco perche' potrebbe averlo
    /// appena corretto l'utente in Obsidian; si controllano i tipi; si fonde;
    /// si scrive dove il file gia' sta, o dove il tipo dice che deve stare.
    pub fn salva(
        &mut self,
        disco: &dyn DiscoScrivibile,
        guardiano: &dyn Guardiano,
        mut nodo: Nodo,
        unisci: bool,
        oggi: &str,
    ) -> Result<Nodo, String> {
        if let Some(motivo) = guardiano.perche_non_si_salva(&format!(
            "{}\n{}",
            nodo.title, nodo.body
        )) {
            // Il rifiuto dice cosa ha trovato, non cosa ha letto.
            return Err(format!(
                "non salvo questo nodo: contiene {motivo}. Quello che entra nel \
                 vault viene riletto in ogni conversazione futura, comprese quelle \
                 in cui leggo testo scritto da altri — una credenziale li' dentro \
                 e' esposta per sempre. Se serve usarla, chiedila al momento."
            ));
        }
        nodo.slug = slug(if nodo.slug.is_empty() { &nodo.title } else { &nodo.slug });

        let mut chi_cera = if self.nodi.contains_key(&nodo.slug) {
            Some(nodo.slug.clone())
        } else {
            self.stesso_nodo(&nodo)
        };

        // Il file puo' esistere su disco senza essere ancora nell'indice: chi
        // impara scrive da un thread di sfondo e il seeding fa una raffica di
        // salvataggi, entrambi senza passare da un giro di aggiornamento.
        // Scriverci sopra in blocco cancellerebbe quel che c'e' gia'.
        if chi_cera.is_none() {
            let atteso = posto::percorso_relativo(&nodo.tipo, &nodo.slug).join("/");
            if disco.impronta(&atteso).is_some() {
                self.carica(disco, &atteso);
                if self.nodi.contains_key(&nodo.slug) {
                    chi_cera = Some(nodo.slug.clone());
                }
            }
        }

        // Il controllo sui tipi vale anche sul ramo diretto: chi estrae genera
        // slug dal titolo, quindi la persona «Marco» e il progetto «Marco»
        // hanno lo stesso slug e si fonderebbero senza passare dal ramo per
        // somiglianza.
        if let Some(slug_altro) = &chi_cera {
            let compatibili = self
                .nodi
                .get(slug_altro)
                .map(|a| crate::fusione::tipi_compatibili(&nodo.tipo, &a.tipo))
                .unwrap_or(false);
            if !compatibili {
                chi_cera = None;
                nodo.slug = posto::slug_libero(&nodo.tipo, &nodo.slug, &self.tipi());
            }
        }

        let slug_richiesto = nodo.slug.clone();
        let mut rinominato: Option<(String, String)> = None;
        if let Some(slug_altro) = &chi_cera {
            nodo.slug = slug_altro.clone();
            if slug_richiesto != nodo.slug {
                rinominato = Some((slug_richiesto, nodo.slug.clone()));
            }
        }

        if unisci {
            if let Some(vecchio) = chi_cera.as_ref().and_then(|s| self.nodi.get(s)) {
                nodo = crate::fusione::fondi(vecchio, &nodo);
            }
        }

        // Dove va: se il file c'e' gia' resta dov'e' — spostarlo vorrebbe
        // dire che chi l'aveva aperto in Obsidian se lo vede sparire.
        let dove = match chi_cera.as_ref().and_then(|s| self.dove(s)) {
            Some(d) => d.to_string(),
            None => posto::percorso_relativo(&nodo.tipo, &nodo.slug).join("/"),
        };
        let impronta = disco.scrivi(&dove, &nodo.a_markdown(oggi))?;
        let slug_finale = nodo.slug.clone();
        self.segna_scritto(&dove, impronta, &slug_finale);
        self.nodi.insert(slug_finale.clone(), nodo.clone());
        if let Some((vecchio, nuovo)) = rinominato {
            self.rinomina_relazioni(disco, &vecchio, &nuovo, oggi);
        }
        self.collega_reciproco(disco, &nodo, oggi);
        Ok(nodo)
    }

    /// Se A dice di essere collegato a B, B deve saperlo: il grafo si
    /// naviga in tutte e due i versi o non si naviga.
    ///
    /// Si accoda senza riordinare, come dall'altra parte: l'ordine delle
    /// relazioni e' quello in cui sono nate, e riordinarlo riscriverebbe il
    /// file di tutti i nodi collegati con una modifica che nessuno ha
    /// chiesto.
    fn collega_reciproco(&mut self, disco: &dyn DiscoScrivibile, nodo: &Nodo, oggi: &str) {
        for slug_altro in nodo.tutte_le_relazioni() {
            let Some(altro) = self.nodi.get(&slug_altro) else {
                continue;
            };
            if altro.tutte_le_relazioni().contains(&nodo.slug) {
                continue;
            }
            let Some(dove) = self.dove(&slug_altro).map(|d| d.to_string()) else {
                continue;
            };
            let Some(altro) = self.nodi.get_mut(&slug_altro) else {
                continue;
            };
            altro.relazioni.push(nodo.slug.clone());
            let testo = altro.a_markdown(oggi);
            if let Ok(impronta) = disco.scrivi(&dove, &testo) {
                self.segna_scritto(&dove, impronta, &slug_altro);
            }
        }
    }

    /// Dopo una fusione, gli archi che puntavano al vecchio slug vanno
    /// spostati. Prima restavano appesi: chi navigava il grafo li scartava
    /// senza dire niente, e il grafo perdeva un arco a ogni deduplicazione
    /// mentre le statistiche continuavano a contarli.
    fn rinomina_relazioni(
        &mut self,
        disco: &dyn DiscoScrivibile,
        vecchio: &str,
        nuovo: &str,
        oggi: &str,
    ) {
        if vecchio.is_empty() || vecchio == nuovo {
            return;
        }
        let da_toccare: Vec<String> = self
            .nodi
            .values()
            .filter(|n| n.tutte_le_relazioni().iter().any(|r| r == vecchio))
            .map(|n| n.slug.clone())
            .collect();
        for slug_altro in da_toccare {
            let Some(altro) = self.nodi.get_mut(&slug_altro) else {
                continue;
            };
            // `dict.fromkeys` del Python: toglie i doppioni **conservando
            // l'ordine**. Ordinare alfabeticamente sarebbe piu' comodo e
            // riscriverebbe il file di tutti i nodi collegati, con una
            // modifica che l'utente non ha chiesto e che vedrebbe comparire
            // in Obsidian. Si scartano anche gli archi verso se stessi: dopo
            // una fusione «vecchio» puo' essere diventato proprio questo
            // nodo.
            let mut viste: Vec<String> = Vec::new();
            for r in &altro.relazioni {
                let r = if r == vecchio { nuovo.to_string() } else { r.clone() };
                if r != slug_altro && !viste.contains(&r) {
                    viste.push(r);
                }
            }
            altro.relazioni = viste;
            // I wikilink nel corpo contano quanto le relazioni dichiarate:
            // rinominare solo il frontmatter lascia «Vedi [[vecchio]]» appeso
            // per sempre.
            altro.body = crate::fusione::rinomina_wikilink(&altro.body, vecchio, nuovo);
            let testo = altro.a_markdown(oggi);
            let Some(dove) = self.dove(&slug_altro).map(|d| d.to_string()) else {
                continue;
            };
            if let Ok(impronta) = disco.scrivi(&dove, &testo) {
                self.segna_scritto(&dove, impronta, &slug_altro);
            }
        }
    }
}

/// Lo stato di un nodo archiviato. Sta qui e non fra le costanti del nodo
/// perche' e' il vault a deciderlo.
pub const STATO_ARCHIVIATO: &str = "archiviato";
pub const STATO_ATTIVO: &str = "attivo";

/// Quanto puo' crescere il registro delle azioni prima che si ricominci.
pub const MAX_REGISTRO_BYTE: u64 = 2 * 1024 * 1024;

/// Il conto di cosa c'e' nel vault.
#[derive(Debug, Default, PartialEq)]
pub struct Statistiche {
    pub nodi_attivi: usize,
    pub archiviati: usize,
    pub collegamenti: usize,
    pub collegamenti_pendenti: usize,
    pub per_tipo: BTreeMap<String, usize>,
    pub per_origine: BTreeMap<String, usize>,
    pub orfani: Vec<String>,
    pub collisioni: BTreeMap<String, Vec<String>>,
}

impl Deposito {
    /// I nodi vivi. Gli archiviati ci sono ancora — il file non si cancella —
    /// ma non rispondono a una ricerca ne' contano nelle statistiche.
    pub fn attivi(&self) -> impl Iterator<Item = &Nodo> {
        self.nodi.values().filter(|n| n.status != STATO_ARCHIVIATO)
    }

    /// Un nodo per titolo, e in mancanza per slug.
    ///
    /// Il titolo prima dello slug perche' chi parla dice «il progetto Nova»,
    /// non «progetto-nova»: e' l'unico punto in cui la memoria si lascia
    /// interrogare come la si nomina a voce.
    pub fn per_titolo(&self, titolo: &str) -> Option<&Nodo> {
        let t = titolo.trim().to_lowercase();
        self.nodi
            .values()
            .find(|n| n.title.trim().to_lowercase() == t)
            .or_else(|| self.nodi.get(&slug(titolo)))
    }

    /// I nodi collegati, **in tutti e due i versi**: il grafo non e'
    /// orientato, e un nodo che qualcun altro nomina e' un suo vicino anche
    /// se lui non lo sa.
    pub fn vicini(&self, slug_cercato: &str) -> Vec<&Nodo> {
        let s = slug(slug_cercato);
        let mut fuori: BTreeMap<String, &Nodo> = BTreeMap::new();
        if let Some(nodo) = self.nodi.get(&s) {
            for r in nodo.tutte_le_relazioni() {
                if let Some(v) = self.nodi.get(&r) {
                    if v.status != STATO_ARCHIVIATO {
                        fuori.insert(v.slug.clone(), v);
                    }
                }
            }
        }
        for altro in self.nodi.values() {
            if altro.status == STATO_ARCHIVIATO || altro.slug == s {
                continue;
            }
            if altro.tutte_le_relazioni().contains(&s) {
                fuori.insert(altro.slug.clone(), altro);
            }
        }
        fuori.into_values().collect()
    }

    /// Mette un nodo da parte senza cancellarlo.
    ///
    /// `oggi_italiano` e' la data come la scrive un italiano — `04/09/2026` —
    /// e arriva da fuori come tutte le date: una funzione che legge
    /// l'orologio non si prova due volte con lo stesso risultato.
    pub fn archivia(
        &mut self,
        disco: &dyn DiscoScrivibile,
        slug_cercato: &str,
        motivo: &str,
        oggi: &str,
        oggi_italiano: &str,
    ) -> bool {
        let s = slug(slug_cercato);
        let Some(nodo) = self.nodi.get_mut(&s) else {
            return false;
        };
        let gia_archiviato = nodo.status == STATO_ARCHIVIATO;
        nodo.status = STATO_ARCHIVIATO.to_string();
        // Archiviare due volte non deve accodare due volte la stessa riga.
        if !motivo.is_empty() && !gia_archiviato {
            nodo.body
                .push_str(&format!("\n\n> Archiviato il {oggi_italiano}: {motivo}"));
        }
        self.riscrivi(disco, &s, oggi)
    }

    /// Rimette in circolo un nodo archiviato.
    pub fn riattiva(&mut self, disco: &dyn DiscoScrivibile, slug_cercato: &str, oggi: &str) -> bool {
        let s = slug(slug_cercato);
        let Some(nodo) = self.nodi.get_mut(&s) else {
            return false;
        };
        nodo.status = STATO_ATTIVO.to_string();
        self.riscrivi(disco, &s, oggi)
    }

    fn riscrivi(&mut self, disco: &dyn DiscoScrivibile, slug_nodo: &str, oggi: &str) -> bool {
        let Some(dove) = self.dove(slug_nodo).map(|d| d.to_string()) else {
            // Un nodo senza file e' un nodo che vive solo in memoria: c'e'
            // poco da riscrivere, ma la modifica in memoria e' avvenuta.
            return true;
        };
        let Some(nodo) = self.nodi.get(slug_nodo) else {
            return false;
        };
        let testo = nodo.a_markdown(oggi);
        match disco.scrivi(&dove, &testo) {
            Ok(impronta) => {
                self.segna_scritto(&dove, impronta, slug_nodo);
                true
            }
            Err(_) => false,
        }
    }

    /// Cosa c'e' nel vault, in numeri.
    pub fn statistiche(&self) -> Statistiche {
        let mut per_tipo: BTreeMap<String, usize> = BTreeMap::new();
        let mut per_origine: BTreeMap<String, usize> = BTreeMap::new();
        for n in self.attivi() {
            *per_tipo.entry(n.tipo.clone()).or_insert(0) += 1;
            *per_origine.entry(n.origine.clone()).or_insert(0) += 1;
        }
        let attivi: Vec<&Nodo> = self.attivi().collect();
        // Solo gli archi che puntano a un nodo che esiste **davvero**:
        // contare anche quelli pendenti faceva sembrare il grafo piu' ricco
        // di com'e', e nascondeva proprio i legami rotti che valeva la pena
        // vedere.
        let mut collegamenti = 0;
        let mut pendenti = 0;
        for n in &attivi {
            for r in n.tutte_le_relazioni() {
                if self.nodi.contains_key(&r) {
                    collegamenti += 1;
                } else {
                    pendenti += 1;
                }
            }
        }
        let orfani: Vec<String> = attivi
            .iter()
            .filter(|n| self.vicini(&n.slug).is_empty())
            .map(|n| n.slug.clone())
            .take(20)
            .collect();
        Statistiche {
            nodi_attivi: attivi.len(),
            archiviati: self.nodi.len() - attivi.len(),
            collegamenti,
            collegamenti_pendenti: pendenti,
            per_tipo,
            per_origine,
            orfani,
            collisioni: self.collisioni.iter().take(20).map(|(k, v)| (k.clone(), v.clone())).collect(),
        }
    }

    /// Il testo dell'indice: la mappa di tutto quello che NOVA sa.
    ///
    /// E' un `hub`, quindi sta in cima al vault e non in una sottocartella —
    /// e' la prima cosa che si vede aprendo la cartella in Obsidian.
    pub fn indice(&self, oggi: &str) -> String {
        let mut righe: Vec<String> = vec![
            "---".into(),
            "title: Indice della conoscenza".into(),
            "tipo: hub".into(),
            "tags: [indice]".into(),
            "relazioni: []".into(),
            "area: Generale".into(),
            "status: attivo".into(),
            "origine: scansione".into(),
            "confidenza: 1.0".into(),
            "riferimenti: []".into(),
            format!("creato: {oggi}"),
            format!("aggiornato: {oggi}"),
            "---".into(),
            "".into(),
            "Mappa di tutto quello che NOVA sa. Generato automaticamente.".into(),
            "".into(),
        ];
        let mut per_tipo: BTreeMap<String, Vec<&Nodo>> = BTreeMap::new();
        let mut ordinati: Vec<&Nodo> = self.attivi().collect();
        ordinati.sort_by_key(|n| n.title.to_lowercase());
        for n in ordinati {
            per_tipo.entry(n.tipo.clone()).or_default().push(n);
        }
        for (tipo, nodi) in per_tipo {
            righe.push(format!("## {tipo}"));
            righe.push(String::new());
            for n in nodi {
                // Il marchio dice da dove viene: quello che ha detto l'utente
                // non si distingue da quello che NOVA ha dedotto, se non si
                // scrive.
                let marchio = if n.origine == "utente" {
                    String::new()
                } else {
                    format!("  _{}_", n.origine)
                };
                righe.push(format!("- [[{}|{}]]{}", n.slug, n.title, marchio));
            }
            righe.push(String::new());
        }
        righe.join("\n")
    }

    /// Dove va scritto l'indice.
    pub fn dove_va_lindice() -> &'static str {
        "_INDICE.md"
    }
}

/// Se il registro delle azioni va ruotato.
///
/// Un file solo di storico, poi si ricomincia. Ogni salvataggio e ogni
/// ricerca scrivono una riga: senza rotazione il file cresce per sempre, e
/// non c'e' nessuno che lo poti.
pub fn ora_di_ruotare(quanto_e_grosso: u64) -> bool {
    quanto_e_grosso >= MAX_REGISTRO_BYTE
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

    // -- il disco finto, versione che sa anche scrivere -----------------
    impl DiscoScrivibile for DiscoFinto {
        fn scrivi(&self, dove: &str, testo: &str) -> Result<Impronta, String> {
            // Il disco finto e' atomico per costruzione: o la voce c'e' con
            // il contenuto nuovo, o c'e' con quello vecchio. E' il contratto
            // che il disco vero mantiene scrivendo di fianco e rinominando.
            self.metti(dove, testo, 9.0);
            self.impronta(dove).ok_or_else(|| "sparito".to_string())
        }
    }

    /// Un guardiano che rifiuta cio' che ha una cifra attaccata a «password».
    /// Non e' quello vero — quello e' un pezzo suo — ma basta a provare che
    /// `salva` **chiede** prima di scrivere.
    struct GuardianoFinto;

    impl Guardiano for GuardianoFinto {
        fn perche_non_si_salva(&self, testo: &str) -> Option<String> {
            if testo.to_lowercase().contains("password: tramonto2026") {
                Some("una credenziale in chiaro".into())
            } else {
                None
            }
        }
    }

    fn nodo(slug: &str, titolo: &str, tipo: &str, corpo: &str) -> Nodo {
        Nodo {
            slug: slug.into(),
            title: titolo.into(),
            body: corpo.into(),
            tipo: tipo.into(),
            ..Default::default()
        }
    }

    const OGGI: &str = "2026-09-04";

    #[test]
    fn un_nodo_nuovo_finisce_nella_cartella_del_suo_tipo() {
        let d = DiscoFinto::default();
        let mut v = Deposito::nuovo(false);
        let n = v
            .salva(&d, &NessunControllo, nodo("anna", "Anna", "persona", "x"), true, OGGI)
            .unwrap();
        assert_eq!(n.slug, "anna");
        assert_eq!(v.dove("anna"), Some("02-persone/anna.md"));
        assert!(d.leggi("02-persone/anna.md").unwrap().contains("title: Anna"));
    }

    #[test]
    fn il_guardiano_viene_chiesto_e_il_rifiuto_non_ripete_il_segreto() {
        let d = DiscoFinto::default();
        let mut v = Deposito::nuovo(false);
        let esito = v.salva(
            &d,
            &GuardianoFinto,
            nodo("wifi", "Wifi", "fatto", "password: Tramonto2026"),
            true,
            OGGI,
        );
        let motivo = esito.unwrap_err();
        assert!(motivo.contains("una credenziale in chiaro"), "{motivo}");
        assert!(
            !motivo.contains("Tramonto2026"),
            "il rifiuto ha ripetuto la credenziale che stava rifiutando: {motivo}"
        );
        assert_eq!(v.quanti(), 0, "l'ha salvato lo stesso");
        assert!(d.elenca().is_empty(), "ha scritto un file rifiutato");
    }

    #[test]
    fn un_fatto_su_anna_confluisce_in_anna_invece_di_sdoppiarla() {
        // Il caso che tiene insieme la memoria. Se qui nascesse un secondo
        // file, ogni annotazione automatica ne creerebbe un altro e i ricordi
        // su Anna smetterebbero di parlarsi.
        let d = DiscoFinto::default();
        let mut v = Deposito::nuovo(false);
        v.salva(&d, &NessunControllo, nodo("anna", "Anna", "persona", "Anna e' una collega."), true, OGGI)
            .unwrap();
        let n = v
            .salva(&d, &NessunControllo, nodo("anna", "Anna", "fatto", "Anna beve caffe'."), true, OGGI)
            .unwrap();
        assert_eq!(v.quanti(), 1, "la memoria si e' sdoppiata");
        assert_eq!(n.tipo, "persona", "il fatto ha declassato la persona");
        assert!(n.body.contains("collega") && n.body.contains("caffe"), "{}", n.body);
        assert_eq!(d.elenca().len(), 1, "due file per lo stesso nodo");
    }

    #[test]
    fn una_persona_e_un_progetto_con_lo_stesso_nome_restano_due_nodi() {
        // Chi estrae genera lo slug dal titolo: la persona «Marco» e il
        // progetto «Marco» arrivano tutti e due come slug «marco», e senza il
        // controllo sui tipi finirebbero nello stesso file, con un tipo solo
        // e nella cartella sbagliata.
        let d = DiscoFinto::default();
        let mut v = Deposito::nuovo(false);
        v.salva(&d, &NessunControllo, nodo("marco", "Marco", "persona", "un collega"), true, OGGI)
            .unwrap();
        let p = v
            .salva(&d, &NessunControllo, nodo("marco", "Marco", "progetto", "un lavoro"), true, OGGI)
            .unwrap();
        assert_eq!(v.quanti(), 2, "si sono fusi");
        assert_ne!(p.slug, "marco");
        assert!(v.dove(&p.slug).unwrap().starts_with("03-progetti/"), "{:?}", v.dove(&p.slug));
    }

    #[test]
    fn un_nodo_riconosciuto_sotto_un_altro_slug_non_ne_crea_un_secondo() {
        // Il modello scrive «knowledge-lab», la scansione aveva scritto
        // «progetto-knowledge-lab».
        let d = DiscoFinto::default();
        let mut v = Deposito::nuovo(false);
        v.salva(&d, &NessunControllo,
                nodo("progetto-knowledge-lab", "Knowledge Lab", "progetto", "il primo"),
                true, OGGI).unwrap();
        let n = v.salva(&d, &NessunControllo,
                        nodo("knowledge-lab", "Knowledge Lab", "progetto", "una nota nuova"),
                        true, OGGI).unwrap();
        assert_eq!(n.slug, "progetto-knowledge-lab");
        assert_eq!(v.quanti(), 1);
        assert_eq!(d.elenca().len(), 1);
    }

    #[test]
    fn un_file_gia_su_disco_non_viene_cancellato_da_un_salvataggio_alla_cieca() {
        // Chi impara scrive da un thread di sfondo, senza passare da un giro
        // di aggiornamento: il file c'e' ma l'indice non lo sa ancora.
        // Scriverci sopra in blocco butterebbe via quel che c'era.
        let d = DiscoFinto::default();
        d.metti("02-persone/anna.md",
                "---\ntitle: Anna\ntipo: persona\n---\n\nCosa importante scritta prima.\n", 1.0);
        let mut v = Deposito::nuovo(false);
        let n = v.salva(&d, &NessunControllo,
                        nodo("anna", "Anna", "persona", "Un fatto nuovo."), true, OGGI).unwrap();
        assert!(n.body.contains("Cosa importante scritta prima"),
                "il corpo di prima e' sparito: {}", n.body);
        assert!(n.body.contains("Un fatto nuovo"), "{}", n.body);
    }

    #[test]
    fn un_nodo_che_cambia_slug_si_porta_dietro_gli_archi_che_lo_puntavano() {
        // Prima gli archi restavano appesi: chi navigava il grafo li scartava
        // in silenzio, e il grafo perdeva un arco a ogni deduplicazione.
        let d = DiscoFinto::default();
        let mut v = Deposito::nuovo(false);
        v.salva(&d, &NessunControllo,
                nodo("progetto-nova", "Nova", "progetto", "il progetto"), true, OGGI).unwrap();
        let mut chi_punta = nodo("gio", "Gio", "persona", "Vedi [[nova]] per il resto.");
        chi_punta.relazioni = vec!["nova".into()];
        v.salva(&d, &NessunControllo, chi_punta, true, OGGI).unwrap();
        // «nova» ora viene riconosciuto come «progetto-nova»
        v.salva(&d, &NessunControllo,
                nodo("nova", "Nova", "progetto", "una nota nuova"), true, OGGI).unwrap();
        let gio = v.prendi("gio").unwrap();
        assert!(gio.relazioni.contains(&"progetto-nova".to_string()),
                "l'arco e' rimasto appeso: {:?}", gio.relazioni);
        assert!(gio.body.contains("[[progetto-nova]]"),
                "il wikilink nel corpo e' rimasto appeso: {}", gio.body);
    }

    #[test]
    fn quel_che_si_e_appena_scritto_non_risulta_cambiato_da_fuori() {
        // Senza segnare l'impronta, il giro dopo NOVA rileggerebbe da disco
        // ogni file che ha appena scritto lei.
        let d = DiscoFinto::default();
        let mut v = Deposito::nuovo(false);
        v.salva(&d, &NessunControllo, nodo("anna", "Anna", "persona", "x"), true, OGGI).unwrap();
        assert_eq!(v.aggiorna(&d), Cambiamenti::default());
    }

    #[test]
    fn il_guardiano_vero_e_attaccato_al_vault() {
        // Non basta che il tratto esista: se nessuno gli attacca il guardiano
        // vero, la porta e' aperta e sembra chiusa.
        let d = DiscoFinto::default();
        let mut v = Deposito::nuovo(false);
        let esito = v.salva(
            &d,
            &GuardianoDeiSegreti,
            nodo("wifi", "Wifi di casa", "fatto", "la password e' Tramonto2026"),
            true,
            OGGI,
        );
        assert!(esito.is_err(), "il guardiano vero ha lasciato passare una password");
        assert!(d.elenca().is_empty());

        // E un ricordo legittimo deve poter entrare: un guardiano che rifiuta
        // tutto non protegge, cancella la memoria.
        let ok = v.salva(
            &d,
            &GuardianoDeiSegreti,
            nodo("gio", "Gio", "persona", "Gio lavora meglio la mattina presto."),
            true,
            OGGI,
        );
        assert!(ok.is_ok(), "{:?}", ok.err());
    }

    #[test]
    fn si_cerca_un_nodo_anche_col_nome_come_lo_scrive_il_modello() {
        let d = DiscoFinto::con(&[("03-progetti/progetto-nova.md", &nota("Nova"))]);
        let mut v = Deposito::nuovo(false);
        v.ricarica(&d);
        assert!(v.prendi("progetto-nova").is_some());
        assert!(v.prendi("Progetto Nova").is_some(), "il filtro non e' stato applicato");
        assert!(v.prendi("mai-visto").is_none());
    }
}
