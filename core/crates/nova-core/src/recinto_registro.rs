//! L'elenco delle voci del recinto (D367).
//!
//! Su Windows il recinto e' un contenitore (AppContainer), e per funzionare
//! scrive voci di permesso **sulle cartelle dell'utente**: per scrivere nelle
//! cartelle dichiarate, per leggere gli strumenti elencati, e — solo la
//! cartella, non il contenuto — su tre cartelle sopra ogni cartella dichiarata
//! per scrivere (la prima sotto la radice del disco, il genitore e il nonno),
//! perche' PowerShell riesca a posizionarsi dentro. Gli strumenti non hanno
//! antenate: si eseguono per percorso, e quelle metterebbero in elenco le
//! cartelle vicine, che possono tenere segreti (`.cargo` accanto a
//! `.cargo\bin`). Piu' un profilo registrato nel sistema.
//! Nessuna di queste cose sta nei dati di NOVA: senza un elenco nessuno
//! saprebbe piu' toglierle, e una voce rimasta su una cartella che l'utente
//! non dichiara piu' **allargherebbe il recinto** oltre quello che ha scelto.
//!
//! L'elenco e' `recinto.json`, accanto a `core.json`. Ogni voce si annota
//! **prima** di scriverla sul disco: un'annotazione senza voce si toglie
//! senza danni, una voce senza annotazione resterebbe per sempre. Si allinea
//! all'avvio del demone e prima di ogni comando (quel che non e' piu'
//! dichiarato perde le voci), e `novad --recinto --togli` toglie tutto — e'
//! quello che chiama il disinstallatore.
//!
//! **Nel dubbio non si parte.** Un elenco illeggibile, una revoca che non
//! riesce, un'identita' diversa da quella registrata, una cartella
//! dichiarata che il contenitore non raggiunge: il comando si ferma e lo
//! dice, invece di lanciare dentro un recinto che non si sa piu' descrivere.
//! Fanno eccezione due cose, e nessuna e' di sicurezza: se la **prima
//! cartella sotto la radice** non si riesce a preparare — `C:\Users`, per un
//! progetto nel profilo, non si scrive senza amministratore — il comando parte
//! lo stesso, PowerShell non si posiziona nella cartella di lavoro, e la cosa
//! finisce negli avvisi della risposta; e se una **cartella di strumenti** non
//! si riesce a preparare, i comandi che la usano non partono ma gli altri si.
//! In tutti e due i casi il contenitore ha meno accesso del dichiarato, non di
//! piu'.
//!
//! Su Linux Landlock non scrive niente sulle cartelle: l'elenco resta vuoto
//! e le funzioni non hanno niente da fare.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use nova_platform::recinto::Permessi;
use serde::{Deserialize, Serialize};

/// Un comando alla volta tocca l'elenco: due comandi in parallelo che lo
/// riscrivessero insieme ne perderebbero una riga.
static LUCCHETTO: Mutex<()> = Mutex::new(());

/// Cosa c'e' in `recinto.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Registro {
    /// L'identita' scritta sulle cartelle, come `S-1-15-2-...`. Se un giorno
    /// cambiasse, le voci vecchie sarebbero intestate a qualcun altro: per
    /// questo si tiene, e si usa quella per toglierle.
    #[serde(default)]
    pub identita: String,
    /// NOVA ha registrato il profilo del contenitore nel sistema: va tolto.
    #[serde(default)]
    pub profilo: bool,
    #[serde(default)]
    pub cartelle: Vec<Voce>,
}

/// Una cartella toccata, e perche'.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Voce {
    pub percorso: PathBuf,
    /// `scrive`, `legge`, `antenata`: i nomi dei generi di
    /// `nova_platform::recinto::windows::Genere`.
    #[serde(default)]
    pub generi: Vec<String>,
    /// La voce l'ha scritta il passo da amministratore (`C:\\Users`, per un
    /// progetto nel profilo): si toglie solo da amministratore. Le voci scritte
    /// dall'utente non hanno bisogno di niente.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub elevata: bool,
}

/// Dove sta l'elenco: accanto a `core.json`.
pub fn percorso() -> PathBuf {
    crate::config::Config::path().with_file_name("recinto.json")
}

/// La stessa cartella scritta in due modi — maiuscole, barre, una barra in
/// fondo — e' la stessa cartella: su Windows i percorsi non distinguono.
fn chiave(p: &Path) -> String {
    let s = p.display().to_string().replace('/', "\\");
    let s = s.trim_end_matches('\\').to_string();
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s
    }
}

/// Se `p` sta dentro `radice` (o e' `radice`), **per componenti**: `C:\dati`
/// non contiene `C:\dati-altrui`.
fn dentro(p: &Path, radice: &Path) -> bool {
    let (p, r) = (format!("{}\\", chiave(p)), format!("{}\\", chiave(radice)));
    p.starts_with(&r)
}

fn leggi(dove: &Path) -> Result<Registro, String> {
    match std::fs::read(dove) {
        Ok(b) => serde_json::from_slice(&b).map_err(|e| {
            format!(
                "{} e' illeggibile ({e}): non so piu' su quali cartelle NOVA ha \
                 scritto le voci del recinto, e non lancio niente finche' non \
                 si sistema",
                dove.display()
            )
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Registro::default()),
        Err(e) => Err(format!("non leggo {}: {e}", dove.display())),
    }
}

/// Scrittura atomica: un file a parte, poi il cambio di nome. Un demone che
/// muore a meta' lascia l'elenco vecchio, non uno troncato. Senza voci e
/// senza profilo non c'e' niente da ricordare: il file non resta.
// Fuori da Windows la usano solo le prove: l'elenco non si scrive mai.
#[cfg_attr(not(windows), allow(dead_code))]
fn scrivi(dove: &Path, r: &Registro) -> Result<(), String> {
    if r.cartelle.is_empty() && !r.profilo {
        return match std::fs::remove_file(dove) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("non tolgo {}: {e}", dove.display())),
        };
    }
    if let Some(su) = dove.parent() {
        std::fs::create_dir_all(su).map_err(|e| format!("non creo {}: {e}", su.display()))?;
    }
    let testo = serde_json::to_vec_pretty(r).map_err(|e| format!("non compongo l'elenco: {e}"))?;
    let provvisorio = dove.with_extension("json.nuovo");
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&provvisorio)
            .map_err(|e| format!("non scrivo {}: {e}", provvisorio.display()))?;
        f.write_all(&testo)
            .and_then(|_| f.sync_all())
            .map_err(|e| format!("non scrivo {}: {e}", provvisorio.display()))?;
    }
    std::fs::rename(&provvisorio, dove).map_err(|e| format!("non sostituisco {}: {e}", dove.display()))
}

/// Le cartelle annotate, per chi chiede (`novad --recinto`).
pub fn elenco() -> Result<Registro, String> {
    let _g = LUCCHETTO.lock().unwrap_or_else(|e| e.into_inner());
    leggi(&percorso())
}

#[cfg(windows)]
mod sistema {
    use super::*;
    use nova_platform::recinto::windows::{
        assicura_profilo, elimina_profilo, ha_permesso, per_posizionarsi_sotto, prepara_cartella, puo,
        revoca, revoca_locale, sonda, Genere, Identita, Sonda,
    };

    use nova_platform::recinto::windows::{
        apri_cartella_di_strumenti, apri_prima_componente, chiudi_cartella_di_strumenti,
        chiudi_prima_componente, e_elevato, e_una_cartella_di_strumenti, e_una_prima_componente,
        esegui_elevato, prima_componente, BloccoFraProcessi, PassoElevato,
    };

    const BLOCCO: &str = "NOVA-recinto-registro";

    /// Quello che il contenitore deve poter fare, cartella per cartella:
    /// scrivere dove l'utente l'ha dichiarato, leggere gli strumenti, e
    /// leggere le cartelle che PowerShell vuole per posizionarsi (vedi
    /// `per_posizionarsi_sotto`).
    pub(super) fn desiderate(p: &Permessi) -> Vec<(PathBuf, Genere)> {
        fn aggiungi(v: &mut Vec<(PathBuf, Genere)>, percorso: &Path, g: Genere) {
            if !v.iter().any(|(q, h)| *h == g && chiave(q) == chiave(percorso)) {
                v.push((percorso.to_path_buf(), g));
            }
        }
        let mut v = Vec::new();
        for d in &p.scrive {
            aggiungi(&mut v, d, Genere::Scrive);
        }
        for d in &p.legge {
            aggiungi(&mut v, d, Genere::Legge);
        }
        // Le antenate servono a PowerShell per posizionarsi nella cartella di
        // **lavoro**, quindi si calcolano per le cartelle in cui si scrive. Uno
        // strumento si esegue per percorso assoluto: aprirne le antenate
        // metterebbe in elenco le cartelle vicine (`.cargo` accanto a
        // `.cargo\bin`, dove stanno le credenziali).
        let radici: Vec<PathBuf> = p.scrive.clone();
        for d in radici {
            for a in per_posizionarsi_sotto(&d) {
                aggiungi(&mut v, &a, Genere::Antenata);
            }
        }
        v
    }

    fn ha(v: &Voce, g: Genere) -> bool {
        v.generi.iter().any(|x| x == g.nome())
    }

    fn segna(r: &mut Registro, p: &Path, g: Genere) {
        let k = chiave(p);
        match r.cartelle.iter_mut().find(|v| chiave(&v.percorso) == k) {
            Some(v) => {
                if !ha(v, g) {
                    v.generi.push(g.nome().to_string());
                }
            }
            None => r.cartelle.push(Voce {
                percorso: p.to_path_buf(),
                generi: vec![g.nome().to_string()],
                elevata: false,
            }),
        }
    }

    fn desegna(r: &mut Registro, p: &Path, g: Genere) {
        let k = chiave(p);
        for v in r.cartelle.iter_mut().filter(|v| chiave(&v.percorso) == k) {
            v.generi.retain(|x| x != g.nome());
        }
        r.cartelle.retain(|v| !v.generi.is_empty());
    }

    fn identita_per(r: &Registro) -> Result<Identita, String> {
        if r.identita.is_empty() {
            Identita::di_nova()
        } else {
            Identita::da_stringa(&r.identita)
        }
    }

    /// Allinea l'elenco a quello che serve adesso e, se `concedere`, prepara
    /// quello che manca. Torna gli avvisi: cose non di sicurezza che non
    /// sono andate.
    pub fn prepara_in(file: &Path, permessi: &Permessi, concedere: bool) -> Result<Vec<String>, String> {
        let _b = BloccoFraProcessi::prendi(BLOCCO)?;
        let mut r = leggi(file)?;
        let chi_attuale = Identita::di_nova()?;
        if !r.cartelle.is_empty() || r.profilo {
            if r.identita.is_empty() {
                r.identita = chi_attuale.stringa();
            } else if r.identita != chi_attuale.stringa() {
                return Err(format!(
                    "{} parla di un'altra identita' ({}): togli prima le voci vecchie \
                     con `novad --recinto --togli`",
                    file.display(),
                    r.identita
                ));
            }
        }
        let voluto = desiderate(permessi);

        // 1) Quel che non serve piu'. Una cartella con anche un solo genere
        //    non piu' voluto perde **tutte** le voci del contenitore: dopo la
        //    fusione non si distinguono, e quelle ancora volute si riscrivono
        //    subito sotto.
        let mut errori = Vec::new();
        let mut avvisi_iniziali: Vec<String> = Vec::new();
        for v in r.cartelle.iter_mut() {
            let k = chiave(&v.percorso);
            let voluti: Vec<&str> = voluto
                .iter()
                .filter(|(q, _)| chiave(q) == k)
                .map(|(_, g)| g.nome())
                .collect();
            if v.generi.iter().all(|g| voluti.contains(&g.as_str())) {
                continue;
            }
            if v.elevata {
                // Aperta da amministratore, si chiude da amministratore. Se non
                // lo siamo la voce resta: e' la lettura di una cartella di primo
                // livello, non un'apertura in scrittura, e si dice.
                if !e_elevato() {
                    avvisi_iniziali.push(format!(
                        "la voce su {} non serve piu' ma l'ha scritta un amministratore: \
                         per toglierla, `novad --recinto --togli`",
                        v.percorso.display()
                    ));
                    continue;
                }
                let chiusa = (if ha(v, Genere::Antenata) {
                    chiudi_prima_componente(&v.percorso)
                } else {
                    Ok(())
                })
                .and_then(|_| {
                    if ha(v, Genere::Legge) {
                        chiudi_cartella_di_strumenti(&v.percorso)
                    } else {
                        Ok(())
                    }
                });
                if let Err(e) = chiusa {
                    errori.push(format!("{}: {e}", v.percorso.display()));
                    continue;
                }
                v.elevata = false;
            } else if v.percorso.exists() {
                // Una cartella che porta solo la voce di antenata non ha copie
                // sotto: revocarla ripassando i figli, su `C:\Users\<nome>`,
                // vorrebbe dire ripassare tutto il profilo.
                let solo_antenata = v.generi.iter().all(|g| g == Genere::Antenata.nome());
                let esito = if solo_antenata {
                    revoca_locale(&v.percorso, &chi_attuale)
                } else {
                    revoca(&v.percorso, &chi_attuale)
                };
                if let Err(e) = esito {
                    errori.push(format!("{}: {e}", v.percorso.display()));
                    continue;
                }
            }
            v.generi.clear();
        }
        r.cartelle.retain(|v| !v.generi.is_empty());
        scrivi(file, &r)?;
        if !errori.is_empty() {
            return Err(format!(
                "non ho potuto togliere le voci del recinto da cartelle che non \
                 servono piu', e resterebbero accessibili ai comandi: {}",
                errori.join("; ")
            ));
        }
        if !concedere || voluto.is_empty() {
            return Ok(avvisi_iniziali);
        }

        // 2) Quel che manca. Il profilo si segna prima di crearlo.
        if !r.profilo {
            r.profilo = true;
            r.identita = chi_attuale.stringa();
            scrivi(file, &r)?;
        }
        let chi = assicura_profilo()?;
        let mut so: Option<Sonda> = None;
        let mut avvisi = avvisi_iniziali;
        // Le altre cartelle di una catena non servono a niente se la prima non
        // si riesce a preparare — `C:\Users`, per un progetto nel profilo — e
        // preparle esporrebbe i nomi di `C:\Users\<nome>` senza alcun beneficio.
        let mut saltare: Vec<PathBuf> = Vec::new();
        for (percorso, genere) in &voluto {
            if !percorso.is_dir() {
                // Una cartella di strumenti scritta male — con una variabile
                // d'ambiente, che non si espande — non deve sparire in
                // silenzio: i comandi che la usano non partirebbero, e nessuno
                // saprebbe perche'.
                if *genere == Genere::Legge {
                    avvisi.push(format!(
                        "la cartella di strumenti {} non esiste: in core.json va scritta \
                         per intero, senza variabili d'ambiente",
                        percorso.display()
                    ));
                }
                continue;
            }
            if *genere == Genere::Antenata
                && saltare.iter().any(|s| chiave(s) == chiave(percorso))
            {
                continue;
            }
            let segnata = r
                .cartelle
                .iter()
                .find(|v| chiave(&v.percorso) == chiave(percorso))
                .map(|v| ha(v, *genere))
                .unwrap_or(false);
            // Permessi che non si leggono: «non so dirlo» vuol dire che si prova a
            // preparare, non che tutto si ferma.
            if segnata && ha_permesso(percorso, &chi, *genere).unwrap_or(false) {
                continue;
            }
            if so.is_none() {
                so = Some(sonda(&chi)?);
            }
            let sonda_ref = so.as_ref().unwrap();
            // Un'antenata o uno strumento che il contenitore legge gia' — `Git`,
            // in Program Files — non si segna e non si tocca: non e' una voce
            // nostra.
            // «Non so dirlo» — i permessi non si leggono nemmeno — vuol dire che si
            // prova a preparare, non che tutto si ferma.
            if matches!(genere, Genere::Antenata | Genere::Legge)
                && puo(percorso, sonda_ref, genere.maschera()).unwrap_or(false)
            {
                continue;
            }
            segna(&mut r, percorso, *genere);
            scrivi(file, &r)?;
            match prepara_cartella(percorso, &chi, *genere, sonda_ref) {
                Ok(true) | Ok(false) => {}
                Err(e) => {
                    desegna(&mut r, percorso, *genere);
                    scrivi(file, &r)?;
                    if *genere == Genere::Antenata {
                        for radice in permessi.scrive.iter() {
                            let catena = per_posizionarsi_sotto(radice);
                            if catena.first().is_some_and(|f| chiave(f) == chiave(percorso)) {
                                saltare.extend(catena.into_iter().skip(1));
                            }
                        }
                        avvisi.push(format!(
                            "il contenitore non puo' leggere {}: {e}. PowerShell non si \
                             posizionera' nelle cartelle sotto di essa: un comando con \
                             cartella di lavoro li' parte altrove, dove non puo' scrivere. \
                             Per aprirla una volta, da amministratore: `novad --recinto --prepara`",
                            percorso.display()
                        ));
                    } else if *genere == Genere::Legge {
                        // Meno accesso del dichiarato, non di piu': la direzione
                        // sicura. Un comando che non usa lo strumento non ne
                        // deve pagare il prezzo.
                        avvisi.push(format!(
                            "il contenitore non puo' leggere la cartella di strumenti {}: {e}. \
                             I comandi che la usano non partiranno. Per aprirla una volta, \
                             da amministratore: `novad --recinto --prepara`",
                            percorso.display()
                        ));
                    } else {
                        return Err(e);
                    }
                }
            }
        }
        Ok(avvisi)
    }

    /// Come `prepara_in`, e in piu' apre **da amministratore** quel che il
    /// contenitore non legge e l'utente non puo' preparare da solo: le prime
    /// cartelle sotto la radice (`C:\\Users`) e le cartelle di strumenti
    /// installate per tutti gli utenti (`C:\\Python313`). Si chiama solo da un
    /// comando esplicito: Windows chiede la conferma, e senza non cambia
    /// niente. Una richiesta sola per tutto.
    ///
    /// Il blocco fra processi **non** si tiene mentre si aspetta la conferma,
    /// che puo' durare minuti: i comandi del demone intanto aspetterebbero il
    /// blocco per un minuto e poi fallirebbero. Si prende per annotare
    /// l'intenzione e di nuovo per riconciliarla con quel che e' successo.
    pub fn prepara_con_privilegi_in(file: &Path, permessi: &Permessi) -> Result<Vec<String>, String> {
        let (chi, prime, strumenti) = {
            let _b = BloccoFraProcessi::prendi(BLOCCO)?;
            let avvisi = prepara_in(file, permessi, true)?;
            let chi = assicura_profilo()?;
            let so = sonda(&chi)?;
            let mut prime: Vec<PathBuf> = Vec::new();
            // La prima cartella sotto la radice serve a PowerShell per
            // posizionarsi nella cartella di lavoro: si guarda dove si scrive.
            for radice in permessi.scrive.iter() {
                let Some(prima) = prima_componente(radice) else { continue };
                if e_una_prima_componente(&prima).is_ok()
                    && !puo(&prima, &so, Genere::Antenata.maschera()).unwrap_or(false)
                    && !prime.iter().any(|d| chiave(d) == chiave(&prima))
                {
                    prime.push(prima);
                }
            }
            let mut strumenti: Vec<PathBuf> = Vec::new();
            for d in &permessi.legge {
                if d.is_dir()
                    && e_una_cartella_di_strumenti(d).is_ok()
                    && !puo(d, &so, Genere::Legge.maschera()).unwrap_or(false)
                    && !strumenti.iter().any(|s| chiave(s) == chiave(d))
                {
                    strumenti.push(d.clone());
                }
            }
            if prime.is_empty() && strumenti.is_empty() {
                return Ok(avvisi);
            }
            // L'intenzione si segna prima, come per ogni voce.
            let mut r = leggi(file)?;
            for (cartelle, genere) in [(&prime, Genere::Antenata), (&strumenti, Genere::Legge)] {
                for d in cartelle {
                    segna(&mut r, d, genere);
                    let k = chiave(d);
                    if let Some(v) = r.cartelle.iter_mut().find(|v| chiave(&v.percorso) == k) {
                        v.elevata = true;
                    }
                }
            }
            scrivi(file, &r)?;
            (chi, prime, strumenti)
        };
        let esito: Result<PassoElevato, String> = if e_elevato() {
            prime
                .iter()
                .try_for_each(|d| apri_prima_componente(d))
                .and_then(|_| strumenti.iter().try_for_each(|d| apri_cartella_di_strumenti(d)))
                .map(|_| PassoElevato::Finito(0))
        } else {
            let mut a: Vec<String> = Vec::new();
            if !prime.is_empty() {
                a.push("--recinto-apri".to_string());
                a.extend(prime.iter().map(|d| d.display().to_string()));
            }
            if !strumenti.is_empty() {
                a.push("--recinto-apri-legge".to_string());
                a.extend(strumenti.iter().map(|d| d.display().to_string()));
            }
            esegui_elevato(&a)
        };
        let (puo_ancora_arrivare, difetto) = match esito {
            Ok(PassoElevato::Finito(0)) => (false, None),
            Ok(PassoElevato::Finito(codice)) => {
                (false, Some(format!("il passo da amministratore e' finito con codice {codice}")))
            }
            Ok(PassoElevato::Scaduto) => (
                true,
                Some(
                    "il passo da amministratore non e' finito entro 3 minuti. Se la richiesta \
                     di Windows viene confermata piu' tardi le voci si scrivono, e sono gia' \
                     annotate: `novad --recinto` le mostra e `novad --recinto --togli` le toglie"
                        .to_string(),
                ),
            ),
            Err(e) => (false, Some(e)),
        };
        {
            let _b = BloccoFraProcessi::prendi(BLOCCO)?;
            riconcilia(file, &chi, &prime, &strumenti, puo_ancora_arrivare)?;
        }
        if let Some(d) = difetto {
            return Err(d);
        }
        // Non ci si fida del codice d'uscita: si chiede a Windows.
        let so = sonda(&chi)?;
        for d in prime.iter().chain(strumenti.iter()) {
            if !puo(d, &so, Genere::Antenata.maschera())? {
                return Err(format!("{} non risulta leggibile dopo il passo da amministratore", d.display()));
            }
        }
        // Adesso le prime ci sono: il resto della catena si puo' preparare.
        prepara_in(file, permessi, true)
    }

    /// Dopo il passo da amministratore, comunque sia andato: l'annotazione di
    /// una cartella resta **se e solo se la voce c'e' davvero**. Ritirarle tutte
    /// quando il passo e' finito male lascerebbe senza annotazione le voci che
    /// erano state scritte, e una voce senza annotazione resta per sempre. Se il
    /// passo e' solo scaduto (`puo_ancora_arrivare`) non si ritira niente: la
    /// persona puo' confermare dopo, e a quel punto la voce arriva.
    pub(super) fn riconcilia(
        file: &Path,
        chi: &Identita,
        prime: &[PathBuf],
        strumenti: &[PathBuf],
        puo_ancora_arrivare: bool,
    ) -> Result<(), String> {
        let mut r = leggi(file)?;
        for (cartelle, genere) in [(prime, Genere::Antenata), (strumenti, Genere::Legge)] {
            for d in cartelle {
                let scritta = ha_permesso(d, chi, genere).unwrap_or(false);
                if !scritta && !puo_ancora_arrivare {
                    desegna(&mut r, d, genere);
                }
            }
        }
        scrivi(file, &r)
    }

    /// Il lavoro del passo da amministratore, per `novad --recinto-apri`,
    /// `--recinto-chiudi` e le due varianti per gli strumenti. Torna il codice
    /// d'uscita: 0 se tutto e' andato, 1 se qualcosa e' stato rifiutato.
    pub fn passo_privilegiato(p: &super::PassoPrivilegiato) -> i32 {
        let mut codice = 0;
        let mut esegui = |cartelle: &[PathBuf], f: &dyn Fn(&Path) -> Result<(), String>| {
            for d in cartelle {
                if let Err(e) = f(d) {
                    eprintln!("{e}");
                    codice = 1;
                }
            }
        };
        esegui(&p.apri_prime, &|d| apri_prima_componente(d));
        esegui(&p.chiudi_prime, &|d| chiudi_prima_componente(d));
        esegui(&p.apri_strumenti, &|d| apri_cartella_di_strumenti(d));
        esegui(&p.chiudi_strumenti, &|d| chiudi_cartella_di_strumenti(d));
        codice
    }

    pub fn togli_tutto_in(file: &Path) -> Result<usize, String> {
        let _b = BloccoFraProcessi::prendi(BLOCCO)?;
        let mut r = leggi(file)?;
        let chi = identita_per(&r)?;
        let mut tolte = 0;
        let mut errori = Vec::new();
        let mut tenute = Vec::new();
        // Le voci scritte da amministratore si tolgono da amministratore, una
        // richiesta sola per tutte. Se l'utente la rifiuta restano, e si dice.
        let mut prime: Vec<String> = Vec::new();
        let mut strumenti: Vec<String> = Vec::new();
        for v in r.cartelle.iter().filter(|v| v.elevata && v.percorso.exists()) {
            let d = v.percorso.display().to_string();
            if ha(v, Genere::Antenata) {
                prime.push(d.clone());
            }
            if ha(v, Genere::Legge) {
                strumenti.push(d);
            }
        }
        if !prime.is_empty() || !strumenti.is_empty() {
            let esito = if e_elevato() {
                prime
                    .iter()
                    .try_for_each(|d| chiudi_prima_componente(Path::new(d)))
                    .and_then(|_| {
                        strumenti
                            .iter()
                            .try_for_each(|d| chiudi_cartella_di_strumenti(Path::new(d)))
                    })
                    .map(|_| PassoElevato::Finito(0))
            } else {
                let mut a: Vec<String> = Vec::new();
                if !prime.is_empty() {
                    a.push("--recinto-chiudi".to_string());
                    a.extend(prime.iter().cloned());
                }
                if !strumenti.is_empty() {
                    a.push("--recinto-chiudi-legge".to_string());
                    a.extend(strumenti.iter().cloned());
                }
                esegui_elevato(&a)
            };
            match esito {
                Ok(PassoElevato::Finito(0)) => {}
                Ok(PassoElevato::Finito(codice)) => {
                    errori.push(format!("il passo da amministratore e' finito con codice {codice}"))
                }
                Ok(PassoElevato::Scaduto) => errori.push(
                    "il passo da amministratore non e' finito entro 3 minuti: le voci che \
                     restano si tolgono rilanciando `novad --recinto --togli`"
                        .to_string(),
                ),
                Err(e) => errori.push(e),
            }
        }
        for v in std::mem::take(&mut r.cartelle) {
            if !v.percorso.exists() {
                tolte += 1;
                continue;
            }
            if v.elevata {
                let ancora = v
                    .generi
                    .iter()
                    .filter_map(|g| Genere::da_nome(g))
                    .any(|g| ha_permesso(&v.percorso, &chi, g).unwrap_or(true));
                if ancora {
                    errori.push(format!("{}: la voce scritta da amministratore c'e' ancora", v.percorso.display()));
                    tenute.push(v);
                } else {
                    tolte += 1;
                }
                continue;
            }
            let solo_antenata = v.generi.iter().all(|g| g == Genere::Antenata.nome());
            let esito = if solo_antenata {
                revoca_locale(&v.percorso, &chi)
            } else {
                revoca(&v.percorso, &chi)
            };
            match esito {
                Ok(()) => tolte += 1,
                Err(e) => {
                    errori.push(format!("{}: {e}", v.percorso.display()));
                    tenute.push(v);
                }
            }
        }
        r.cartelle = tenute;
        if errori.is_empty() {
            // Il profilo si toglie sempre, anche se l'elenco non lo ricorda:
            // il nome e' nostro, e un profilo rimasto e' una traccia sul PC.
            match elimina_profilo() {
                Ok(()) => r.profilo = false,
                Err(e) => errori.push(e),
            }
        }
        scrivi(file, &r)?;
        if errori.is_empty() {
            Ok(tolte)
        } else {
            Err(format!("voci del recinto non tolte: {}", errori.join("; ")))
        }
    }
}

/// Quel che serve a questo comando, e toglie quel che non serve piu'. Si
/// chiama prima di ogni comando confinato. Se torna un errore il comando non
/// parte; gli avvisi sono cose che non riguardano la sicurezza.
pub fn prepara(permessi: &Permessi) -> Result<Vec<String>, String> {
    #[cfg(windows)]
    {
        sistema::prepara_in(&percorso(), permessi, true)
    }
    #[cfg(not(windows))]
    {
        let _ = permessi;
        Ok(Vec::new())
    }
}

/// Come `prepara`, e in piu' apre da amministratore le prime cartelle che
/// l'utente non puo' preparare da solo. E' il comando esplicito dietro
/// `novad --recinto --prepara`: Windows chiede la conferma.
pub fn prepara_con_privilegi(permessi: &Permessi) -> Result<Vec<String>, String> {
    #[cfg(windows)]
    {
        sistema::prepara_con_privilegi_in(&percorso(), permessi)
    }
    #[cfg(not(windows))]
    {
        let _ = permessi;
        Ok(Vec::new())
    }
}

/// Cosa chiede un'invocazione del passo da amministratore. Tutti i campi
/// vuoti vuol dire che non e' un'invocazione del passo.
#[derive(Debug, Clone, Default)]
pub struct PassoPrivilegiato {
    /// Le prime cartelle sotto la radice di un disco da aprire (`C:\Users`).
    pub apri_prime: Vec<PathBuf>,
    pub chiudi_prime: Vec<PathBuf>,
    /// Le cartelle di strumenti da aprire in sola lettura (`C:\Python313`).
    pub apri_strumenti: Vec<PathBuf>,
    pub chiudi_strumenti: Vec<PathBuf>,
}

impl PassoPrivilegiato {
    pub fn e_vuoto(&self) -> bool {
        self.apri_prime.is_empty()
            && self.chiudi_prime.is_empty()
            && self.apri_strumenti.is_empty()
            && self.chiudi_strumenti.is_empty()
    }
}

/// Il passo da amministratore: `novad --recinto-apri|--recinto-chiudi|
/// --recinto-apri-legge|--recinto-chiudi-legge DIR...`. Torna il codice
/// d'uscita.
pub fn passo_privilegiato(p: &PassoPrivilegiato) -> i32 {
    #[cfg(windows)]
    {
        sistema::passo_privilegiato(p)
    }
    #[cfg(not(windows))]
    {
        let _ = p;
        0
    }
}

/// Toglie le voci delle cartelle che non sono piu' dichiarate, senza
/// scriverne di nuove. Si chiama all'avvio del demone.
pub fn allinea(permessi: &Permessi) -> Result<(), String> {
    #[cfg(windows)]
    {
        sistema::prepara_in(&percorso(), permessi, false).map(|_| ())
    }
    #[cfg(not(windows))]
    {
        let _ = permessi;
        Ok(())
    }
}

/// Toglie tutte le voci annotate, il profilo del contenitore e l'elenco.
/// Lo chiama `novad --recinto --togli`, e quello lo chiama il disinstallatore.
pub fn togli_tutto() -> Result<usize, String> {
    #[cfg(windows)]
    {
        sistema::togli_tutto_in(&percorso())
    }
    #[cfg(not(windows))]
    {
        Ok(0)
    }
}

/// Una cartella del `PATH` che il contenitore non legge, candidata per
/// `tool_roots`. **Non si concede mai da sola**: la sceglie l'utente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposta {
    pub percorso: PathBuf,
    /// Sta nel profilo dell'utente, dove vicino ci sono spesso segreti.
    pub nel_profilo: bool,
}

/// Dai percorsi gia' filtrati (esistono, il contenitore non li legge) alle
/// proposte: una volta sola ciascuno, nell'ordine del `PATH`, con il profilo
/// segnalato. Pura, per le prove.
pub fn proposte_da(candidati: &[PathBuf], profilo: Option<&Path>) -> Vec<Proposta> {
    let mut v: Vec<Proposta> = Vec::new();
    for c in candidati {
        if v.iter().any(|p| chiave(&p.percorso) == chiave(c)) {
            continue;
        }
        v.push(Proposta {
            percorso: c.clone(),
            nel_profilo: profilo.is_some_and(|u| dentro(c, u)),
        });
    }
    v
}

/// Le cartelle del `PATH` che il contenitore non legge. Su Windows soltanto.
pub fn proponi() -> Result<Vec<Proposta>, String> {
    #[cfg(windows)]
    {
        use nova_platform::recinto::windows::{assicura_profilo, puo, sonda, Genere};
        let chi = assicura_profilo()?;
        let so = sonda(&chi)?;
        let candidati: Vec<PathBuf> = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
            .filter(|p| p.is_absolute() && p.is_dir())
            .filter(|p| !puo(p, &so, Genere::Legge.maschera()).unwrap_or(false))
            .collect();
        let profilo = std::env::var_os("USERPROFILE").map(PathBuf::from);
        Ok(proposte_da(&candidati, profilo.as_deref()))
    }
    #[cfg(not(windows))]
    {
        Ok(Vec::new())
    }
}

/// Quel che `novad --recinto --proponi` stampa.
pub fn racconto_proposte(p: &[Proposta]) -> String {
    if p.is_empty() {
        return "nessuna cartella del PATH da proporre: il contenitore le legge tutte, \
                o non c'e' un contenitore su questo sistema"
            .to_string();
    }
    let mut s = String::from(
        "cartelle del PATH che il contenitore non legge (candidate per `tool_roots` in core.json):\n",
    );
    for x in p {
        s.push_str(&format!(
            "  {}{}\n",
            x.percorso.display(),
            if x.nel_profilo {
                "   <- sta nel tuo profilo: li' vicino ci sono spesso segreti; concedi \
                 questa cartella e non quella sopra"
            } else {
                ""
            }
        ));
    }
    if p.iter().any(|x| chiave(&x.percorso).ends_with("\\.cargo\\bin")) {
        s.push_str(
            "  nota: cargo e' un proxy di rustup; per farlo partire serve anche ~\\.rustup \
             (le toolchain, senza segreti). `.cargo` no: ci sono le credenziali.\n",
        );
    }
    s.push_str(
        "nessuna viene concessa da sola. Una cartella installata per tutti gli utenti \
         (C:\\Python313) si apre con `novad --recinto --prepara`, che chiede la conferma \
         di amministratore.",
    );
    s
}

/// Quel che `novad --recinto` stampa.
pub fn racconto() -> String {
    let r = match elenco() {
        Ok(r) => r,
        Err(e) => return e,
    };
    if r.cartelle.is_empty() && !r.profilo {
        return "nessuna cartella porta le voci del recinto di NOVA".to_string();
    }
    let profilo_utente = std::env::var_os("USERPROFILE").map(PathBuf::from);
    let mut s = format!(
        "voci del recinto di NOVA ({}), in {}:\n",
        if r.identita.is_empty() { "identita' non ancora scritta" } else { &r.identita },
        percorso().display()
    );
    for v in &r.cartelle {
        let nel_profilo = profilo_utente.as_deref().is_some_and(|u| dentro(&v.percorso, u));
        let sensibile = nel_profilo && v.generi.iter().any(|g| g != "scrive");
        s.push_str(&format!(
            "  {}  [{}]{}\n",
            v.percorso.display(),
            if v.elevata {
                format!("{}, aperta da amministratore", v.generi.join(", "))
            } else {
                v.generi.join(", ")
            },
            if sensibile {
                "  <- sta nel tuo profilo: li' vicino ci sono spesso segreti; qui il contenitore legge, non scrive"
            } else {
                ""
            }
        ));
    }
    if r.profilo {
        s.push_str("  profilo del contenitore registrato nel sistema (nova.recinto)\n");
    }
    s.push_str("per toglierle tutte: novad --recinto --togli");
    s
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn la_stessa_cartella_scritta_in_due_modi_e_la_stessa() {
        if cfg!(windows) {
            assert_eq!(chiave(Path::new(r"C:\Dati\Lavoro\")), chiave(Path::new("c:/dati/lavoro")));
        } else {
            assert_eq!(chiave(Path::new("/dati/lavoro/")), chiave(Path::new("/dati/lavoro")));
        }
    }

    /// `C:\dati` non contiene `C:\dati-altrui`: il difetto che il commento di
    /// `Policy` racconta come gia' corretto altrove.
    #[test]
    fn dentro_conta_per_componenti() {
        let (a, b) = if cfg!(windows) {
            (r"C:\dati-altrui\x", r"C:\dati")
        } else {
            ("/dati-altrui/x", "/dati")
        };
        assert!(!dentro(Path::new(a), Path::new(b)));
        assert!(dentro(Path::new(&format!("{b}/x")), Path::new(b)));
        assert!(dentro(Path::new(b), Path::new(b)));
    }

    #[test]
    fn un_elenco_illeggibile_ferma_invece_di_ripartire_da_zero() {
        let d = std::env::temp_dir().join(format!("nova-registro-rotto-{}.json", std::process::id()));
        std::fs::write(&d, b"{ non e' json").unwrap();
        let e = leggi(&d).expect_err("doveva fermarsi");
        assert!(e.contains("non lancio niente"), "{e}");
        // E il file non si tocca: dentro c'e' l'unica traccia delle voci.
        assert_eq!(std::fs::read(&d).unwrap(), b"{ non e' json");
        let _ = std::fs::remove_file(&d);
    }

    /// Un elenco scritto prima che esistesse `elevata` si legge ancora: il
    /// campo manca e vale falso. E il falso non si scrive, cosi' l'elenco di
    /// chi non usa il passo da amministratore non cambia.
    #[test]
    fn una_voce_senza_elevata_si_legge_e_non_la_scrive() {
        let vecchio = r#"{"identita":"S-1-15-2-1","profilo":true,"cartelle":[{"percorso":"C:\\x","generi":["scrive"]}]}"#;
        let r: Registro = serde_json::from_str(vecchio).unwrap();
        assert!(!r.cartelle[0].elevata);
        assert!(!serde_json::to_string(&r).unwrap().contains("elevata"));
        let mut r2 = r.clone();
        r2.cartelle[0].elevata = true;
        assert!(serde_json::to_string(&r2).unwrap().contains("\"elevata\":true"));
    }

    /// Le proposte: una volta sola ciascuna, nell'ordine del PATH, e quelle nel
    /// profilo si segnalano. Quella appena fuori no: `C:\Users\Public` non e'
    /// dentro `C:\Users\utente`.
    #[test]
    fn le_proposte_segnalano_il_profilo_e_non_si_ripetono() {
        let profilo = if cfg!(windows) { PathBuf::from(r"C:\Users\utente") } else { PathBuf::from("/home/utente") };
        let (cargo, python, vicino) = if cfg!(windows) {
            (
                PathBuf::from(r"C:\Users\utente\.cargo\bin"),
                PathBuf::from(r"C:\Python313"),
                PathBuf::from(r"C:\Users\Public\bin"),
            )
        } else {
            (
                PathBuf::from("/home/utente/.cargo/bin"),
                PathBuf::from("/opt/python"),
                PathBuf::from("/home/utente-altro/bin"),
            )
        };
        let p = proposte_da(&[cargo.clone(), python.clone(), cargo.clone(), vicino.clone()], Some(&profilo));
        assert_eq!(p.len(), 3, "{p:?}");
        assert_eq!(p[0], Proposta { percorso: cargo, nel_profilo: true });
        assert_eq!(p[1], Proposta { percorso: python, nel_profilo: false });
        assert_eq!(p[2], Proposta { percorso: vicino, nel_profilo: false }, "Public non e' dentro utente");
        let t = racconto_proposte(&p);
        assert!(t.contains("sta nel tuo profilo"), "{t}");
        assert!(t.contains("nessuna viene concessa da sola"), "{t}");
    }

    /// Per cargo si dice quel che serve davvero, e che `.cargo` no.
    #[test]
    fn per_cargo_si_dice_anche_rustup_e_non_cargo() {
        if !cfg!(windows) {
            return;
        }
        let p = proposte_da(&[PathBuf::from(r"C:\Users\utente\.cargo\bin")], Some(Path::new(r"C:\Users\utente")));
        let t = racconto_proposte(&p);
        assert!(t.contains(".rustup") && t.contains("`.cargo` no"), "{t}");
    }

    #[test]
    fn senza_proposte_si_dice() {
        assert!(racconto_proposte(&[]).contains("nessuna cartella del PATH da proporre"));
    }

    /// «Dove stanno i dati» dice dov'e' `recinto.json` dalla base, come per le
    /// altre voci della lista, e non dal modulo che lo scrive: i due conti
    /// devono dare lo stesso posto, o la mappa indicherebbe un file che non c'e'.
    #[cfg(windows)]
    #[test]
    fn la_mappa_dei_dati_e_il_modulo_che_scrive_dicono_lo_stesso_posto() {
        let b = crate::dati::base();
        assert_eq!(percorso(), b.join("recinto.json"));
        assert_eq!(crate::recinto_controllo::percorso(), b.join("recinto-controllo.json"));
    }

    #[test]
    fn senza_voci_e_senza_profilo_non_resta_un_file() {
        let d = std::env::temp_dir().join(format!("nova-registro-vuoto-{}.json", std::process::id()));
        std::fs::write(&d, b"{}").unwrap();
        scrivi(&d, &Registro::default()).unwrap();
        assert!(!d.exists());
    }

    /// Con il profilo registrato il file resta anche senza cartelle: e'
    /// l'unica traccia che c'e' qualcosa da togliere.
    #[test]
    fn con_il_profilo_ma_senza_cartelle_il_file_resta() {
        let d = std::env::temp_dir().join(format!("nova-registro-profilo-{}.json", std::process::id()));
        let r = Registro { profilo: true, ..Default::default() };
        scrivi(&d, &r).unwrap();
        assert_eq!(leggi(&d).unwrap(), r);
        let _ = std::fs::remove_file(&d);
    }

    #[cfg(windows)]
    mod windows {
        use super::super::sistema;
        use super::*;
        use nova_platform::recinto::windows::{
            assicura_profilo, concedi, ha_permesso, revoca_locale, Genere, Identita,
        };
        use std::sync::Mutex;

        /// Il profilo e' uno solo per utente: una prova alla volta.
        static SERIALE: Mutex<()> = Mutex::new(());

        /// Toglie la cartella anche se la prova cade a meta': senza, una prova
        /// fallita lascia una cartella nella radice del disco.
        struct Pulizia(PathBuf);

        impl Drop for Pulizia {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }

        /// Una radice mia sul disco di sistema, cosi' le antenate sono mie e
        /// non si toccano le cartelle vere del profilo.
        fn radice(nome: &str) -> Option<PathBuf> {
            let r = PathBuf::from(format!(
                "{}\\nova-prova-{nome}-{}",
                std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()),
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&r);
            std::fs::create_dir_all(&r).ok().map(|_| r)
        }

        fn permessi(scrive: &[&Path], legge: &[&Path]) -> Permessi {
            Permessi {
                scrive: scrive.iter().map(|p| p.to_path_buf()).collect(),
                legge: legge.iter().map(|p| p.to_path_buf()).collect(),
                senza_rete: false,
            }
        }

        /// Il ciclo intero: preparate e annotate, le cartelle portano la voce;
        /// quando una esce da `write_roots`, l'allineamento la toglie e la
        /// toglie dall'elenco; disinstallando, tutto — profilo compreso.
        #[test]
        fn windows_una_cartella_che_esce_da_write_roots_perde_le_voci() {
            let _s = SERIALE.lock().unwrap_or_else(|e| e.into_inner());
            let Some(base) = radice("registro") else {
                eprintln!("saltata: non posso creare cartelle nella radice del disco");
                return;
            };
            let _pulizia = Pulizia(base.clone());
            let (a, b) = (base.join("a"), base.join("b"));
            std::fs::create_dir_all(&a).unwrap();
            std::fs::create_dir_all(&b).unwrap();
            let elenco = base.join("recinto.json");
            let chi = Identita::di_nova().unwrap();

            sistema::prepara_in(&elenco, &permessi(&[&a, &b], &[]), true).unwrap();
            assert!(ha_permesso(&a, &chi, Genere::Scrive).unwrap());
            assert!(ha_permesso(&b, &chi, Genere::Scrive).unwrap());
            let r = leggi(&elenco).unwrap();
            assert!(r.profilo, "il profilo non e' stato segnato");
            assert_eq!(r.identita, chi.stringa());

            // `b` esce dalle cartelle dichiarate.
            sistema::prepara_in(&elenco, &permessi(&[&a], &[]), true).unwrap();
            assert!(ha_permesso(&a, &chi, Genere::Scrive).unwrap(), "a e' ancora dichiarata");
            assert!(!ha_permesso(&b, &chi, Genere::Scrive).unwrap(), "b non e' piu' dichiarata");
            assert!(
                !leggi(&elenco).unwrap().cartelle.iter().any(|v| chiave(&v.percorso) == chiave(&b)),
                "b e' rimasta nell'elenco"
            );

            // Disinstallando, tutto.
            sistema::togli_tutto_in(&elenco).unwrap();
            assert!(!ha_permesso(&a, &chi, Genere::Scrive).unwrap());
            assert!(!elenco.exists(), "l'elenco vuoto e' rimasto");
            let profilo = nova_platform::recinto::windows::temporanea_del_profilo()
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .to_path_buf();
            assert!(!profilo.exists(), "il profilo del contenitore e' rimasto: {}", profilo.display());
            let _ = std::fs::remove_dir_all(&base);
        }


    /// Uno strumento che non si riesce a preparare e' un avviso, perche' il
    /// contenitore ha meno accesso del dichiarato; una cartella di scrittura
    /// no, perche' un comando che parte "dentro le cartelle dichiarate" e poi
    /// non ci scrive sarebbe una promessa falsa. `System32\config` non la
    /// legge nemmeno l'utente: e' il caso di una cartella che non si prepara.
    #[test]
    fn windows_uno_strumento_che_non_si_prepara_e_un_avviso() {
        let _s = SERIALE.lock().unwrap_or_else(|e| e.into_inner());
        // Da amministratore — come sul runner della CI — `System32\config` si
        // legge e si prepara davvero: la prova non rifiuterebbe, e scriverebbe
        // un permesso in `System32`. Si salta.
        if nova_platform::recinto::windows::e_elevato() {
            eprintln!("saltata: gira da amministratore, e il rifiuto che aspetta non ci sarebbe");
            return;
        }
        let Some(base) = radice("strumenti") else {
            eprintln!("saltata: non posso creare cartelle nella radice del disco");
            return;
        };
        let _pulizia = Pulizia(base.clone());
        let elenco = base.join("recinto.json");
        let inaccessibile = PathBuf::from(format!(
            "{}\\Windows\\System32\\config",
            std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into())
        ));
        if !inaccessibile.is_dir() {
            eprintln!("saltata: manca {}", inaccessibile.display());
            return;
        }
        let avvisi = sistema::prepara_in(&elenco, &permessi(&[], &[&inaccessibile]), true)
            .expect("uno strumento non preparabile non blocca il comando");
        assert!(
            avvisi.iter().any(|a| a.contains("cartella di strumenti") && a.contains("--prepara")),
            "{avvisi:?}"
        );
        assert!(
            !leggi(&elenco).unwrap().cartelle.iter().any(|v| chiave(&v.percorso) == chiave(&inaccessibile)),
            "una voce che non si e' scritta e' rimasta nell'elenco"
        );
        let errore = sistema::prepara_in(&elenco, &permessi(&[&inaccessibile], &[]), true)
            .expect_err("una cartella di scrittura che non si prepara blocca");
        assert!(!errore.is_empty());
        sistema::togli_tutto_in(&elenco).unwrap();
    }

        /// Una cartella che passa da scrivibile a sola lettura perde la
        /// scrittura e tiene la lettura.
        #[test]
        fn windows_da_scrittura_a_sola_lettura_si_toglie_solo_la_scrittura() {
            let _s = SERIALE.lock().unwrap_or_else(|e| e.into_inner());
            let Some(base) = radice("generi") else {
                eprintln!("saltata: non posso creare cartelle nella radice del disco");
                return;
            };
            let _pulizia = Pulizia(base.clone());
            let d = base.join("strumenti");
            std::fs::create_dir_all(&d).unwrap();
            let elenco = base.join("recinto.json");
            let chi = assicura_profilo().unwrap();

            sistema::prepara_in(&elenco, &permessi(&[&d], &[]), true).unwrap();
            assert!(ha_permesso(&d, &chi, Genere::Scrive).unwrap());

            sistema::prepara_in(&elenco, &permessi(&[&base.join("altrove")], &[&d]), true).unwrap();
            assert!(ha_permesso(&d, &chi, Genere::Legge).unwrap(), "la lettura doveva restare");
            assert!(!ha_permesso(&d, &chi, Genere::Scrive).unwrap(), "la scrittura doveva sparire");
            let r = leggi(&elenco).unwrap();
            let voce = r.cartelle.iter().find(|v| chiave(&v.percorso) == chiave(&d)).unwrap();
            assert_eq!(voce.generi, vec!["legge".to_string()]);

            sistema::togli_tutto_in(&elenco).unwrap();
            let _ = std::fs::remove_dir_all(&base);
        }

        /// Le antenate servono a PowerShell per posizionarsi nella cartella di
        /// **lavoro**. Per uno strumento no: si esegue per percorso, e aprirne
        /// le antenate metterebbe in elenco `.cargo` accanto a `.cargo\bin`, la
        /// cartella con le credenziali.
        #[test]
        fn windows_le_antenate_si_calcolano_solo_per_dove_si_scrive() {
            let Some(base) = radice("antenate") else {
                eprintln!("saltata: non posso creare cartelle nella radice del disco");
                return;
            };
            let _pulizia = Pulizia(base.clone());
            let lavoro = base.join("a").join("b").join("lavoro");
            let strumenti = base.join("c").join(".cargo").join("bin");
            std::fs::create_dir_all(&lavoro).unwrap();
            std::fs::create_dir_all(&strumenti).unwrap();
            let voluto = sistema::desiderate(&permessi(&[&lavoro], &[&strumenti]));
            let antenate: Vec<PathBuf> = voluto
                .iter()
                .filter(|(_, g)| *g == Genere::Antenata)
                .map(|(p, _)| p.clone())
                .collect();
            let ce = |p: &Path| antenate.iter().any(|q| chiave(q) == chiave(p));
            assert!(ce(&base.join("a").join("b")), "il genitore della cartella di lavoro: {antenate:?}");
            assert!(ce(&base.join("a")), "il nonno della cartella di lavoro: {antenate:?}");
            assert!(
                !ce(&base.join("c").join(".cargo")),
                "`.cargo` e' comparsa fra le antenate di uno strumento: {antenate:?}"
            );
            assert!(!ce(&base.join("c")), "{antenate:?}");
            assert!(
                voluto.iter().any(|(p, g)| *g == Genere::Legge && chiave(p) == chiave(&strumenti)),
                "lo strumento stesso c'e' ancora"
            );
        }

        /// Dopo il passo da amministratore, comunque sia andato, l'annotazione
        /// resta solo dove la voce c'e' davvero. Ritirarle tutte quando il passo
        /// finisce male lascia le voci scritte senza annotazione, e quelle non
        /// le toglie piu' nessuno.
        #[test]
        fn windows_dopo_il_passo_si_tiene_l_annotazione_solo_di_quel_che_c_e() {
            let _s = SERIALE.lock().unwrap_or_else(|e| e.into_inner());
            let Some(base) = radice("riconcilia") else {
                eprintln!("saltata: non posso creare cartelle nella radice del disco");
                return;
            };
            let _pulizia = Pulizia(base.clone());
            let (a, b) = (base.join("a"), base.join("b"));
            std::fs::create_dir_all(&a).unwrap();
            std::fs::create_dir_all(&b).unwrap();
            let elenco = base.join("recinto.json");
            let chi = assicura_profilo().unwrap();
            let annota_entrambe = || {
                let r = Registro {
                    identita: chi.stringa(),
                    profilo: true,
                    cartelle: [&a, &b]
                        .iter()
                        .map(|d| Voce {
                            percorso: (*d).clone(),
                            generi: vec!["antenata".to_string()],
                            elevata: true,
                        })
                        .collect(),
                };
                scrivi(&elenco, &r).unwrap();
            };
            let annotata = |d: &Path| {
                leggi(&elenco).unwrap().cartelle.iter().any(|v| chiave(&v.percorso) == chiave(d))
            };
            let (pa, pb) = (vec![a.clone(), b.clone()], Vec::<PathBuf>::new());

            // Finito male dopo aver scritto la voce su `a` e non su `b`.
            concedi(&a, &chi, Genere::Antenata).unwrap();
            annota_entrambe();
            sistema::riconcilia(&elenco, &chi, &pa, &pb, false).unwrap();
            assert!(annotata(&a), "la voce su `a` c'e': l'annotazione non si ritira");
            assert!(!annotata(&b), "su `b` non e' stato scritto niente: l'annotazione si ritira");

            // Scaduto: la conferma puo' ancora arrivare, non si ritira niente.
            annota_entrambe();
            sistema::riconcilia(&elenco, &chi, &pa, &pb, true).unwrap();
            assert!(annotata(&a) && annotata(&b), "scaduto non vuol dire rifiutato");

            // Rifiutato: nessuna voce, si ritirano tutte.
            revoca_locale(&a, &chi).unwrap();
            annota_entrambe();
            sistema::riconcilia(&elenco, &chi, &pa, &pb, false).unwrap();
            assert!(!annotata(&a) && !annotata(&b), "nessuna voce, nessuna annotazione");

            sistema::togli_tutto_in(&elenco).unwrap();
        }
    }
}
