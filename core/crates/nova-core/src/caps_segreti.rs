//! Le credenziali, esposte come capacità.
//!
//! Cinque porte, e la differenza fra loro è tutta in *dove finisce il valore*:
//!
//! - `segreti.elenco`, `segreti.ordine` restituiscono **giudizi**, mai valori.
//!   Girano liberamente: sono ciò che permette a NOVA di dire «ce l'ho»,
//!   «quella è debole», «quella la usi in quattro posti».
//! - `segreti.salva` e `segreti.genera` scrivono. La seconda non fa nemmeno
//!   vedere cosa ha generato: chi la chiama riceve il nome, non la password.
//! - `segreti.leggi` è l'unica che tira fuori il valore, ed è marcata
//!   pericolosa perché lo è: da lì in poi il segreto è nel contesto di chi ha
//!   chiesto, e nel contesto ci resta.
//!
//! Per usare una credenziale non serve leggerla: `ui.set_text` accetta
//! `segreto`, e il valore va dall'archivio dentro al campo senza passare dal
//! modello. È il percorso normale; `segreti.leggi` è per quando sei tu a
//! volerla sapere.

use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use nova_proto::{CapabilityInfo, Risk};
use serde_json::{json, Value};

use crate::capability::{arg_bool, arg_str, arg_str_opt, arg_u64, schema, Capability, Ctx, Registry};
use crate::segreti::{self, Voce};

pub fn register(reg: &mut Registry) {
    reg.add(Arc::new(ElencoCap));
    reg.add(Arc::new(LeggiCap));
    reg.add(Arc::new(SalvaCap));
    reg.add(Arc::new(GeneraCap));
    reg.add(Arc::new(DimenticaCap));
    reg.add(Arc::new(OrdineCap));
    reg.add(Arc::new(ImportaCap));
}

struct ElencoCap;

#[async_trait]
impl Capability for ElencoCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "segreti.elenco".into(),
            description: "L'inventario delle credenziali salvate: nome, servizio, utente, \
                          indirizzo, da quanto non cambia, quanto e' robusta, se e' \
                          ripetuta altrove. NON restituisce le password — per usarne \
                          una passa «segreto» a ui.set_text, per vederla usa \
                          segreti.leggi."
                .into(),
            risk: Risk::Safe,
            category: "segreti".into(),
            schema: schema(&[
                ("cerca", "string", "Filtra per nome, servizio, utente o categoria", false),
            ]),
        }
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let ago = arg_str_opt(&args, "cerca").unwrap_or_default().to_lowercase();
        let tutte = tokio::task::spawn_blocking(segreti::elenco).await??;
        let scelte: Vec<_> = tutte
            .into_iter()
            .filter(|s| {
                ago.is_empty()
                    || [&s.nome, &s.servizio, &s.utente, &s.categoria, &s.url]
                        .iter()
                        .any(|c| c.to_lowercase().contains(&ago))
            })
            .collect();
        Ok(json!({ "quante": scelte.len(), "credenziali": scelte }))
    }
}

struct LeggiCap;

#[async_trait]
impl Capability for LeggiCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "segreti.leggi".into(),
            description: "Tira fuori il valore di una credenziale. Da qui in poi il \
                          segreto e' nel contesto della conversazione e ci resta: usala \
                          solo quando e' l'utente a volerla sapere. Per COMPILARE un \
                          campo non serve — passa «segreto» a ui.set_text e il valore \
                          non passa da te."
                .into(),
            risk: Risk::Dangerous,
            category: "segreti".into(),
            schema: schema(&[("nome", "string", "Il nome della credenziale", true)]),
        }
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let nome = arg_str(&args, "nome")?;
        let valore = tokio::task::spawn_blocking(move || segreti::leggi(&nome)).await??;
        Ok(json!({ "valore": valore }))
    }
}

struct SalvaCap;

#[async_trait]
impl Capability for SalvaCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "segreti.salva".into(),
            description: "Salva o aggiorna una credenziale. Il valore vuoto lascia \
                          quello di prima, cosi' si correggono i metadati — servizio, \
                          utente, categoria — senza dover ridire la password."
                .into(),
            risk: Risk::Moderate,
            category: "segreti".into(),
            schema: schema(&[
                ("nome", "string", "Come richiamarla, es. «gmail.giova»", true),
                ("valore", "string", "La password; vuoto = non toccarla", false),
                ("servizio", "string", "Gmail, Amazon, la banca...", false),
                ("utente", "string", "L'indirizzo o il nome utente", false),
                ("url", "string", "Dove si usa", false),
                ("categoria", "string", "posta, banca, lavoro, svago...", false),
                ("note", "string", "Quello che serve ricordare e non e' segreto", false),
            ]),
        }
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let v = Voce {
            nome: arg_str(&args, "nome")?,
            valore: arg_str_opt(&args, "valore").unwrap_or_default(),
            servizio: arg_str_opt(&args, "servizio").unwrap_or_default(),
            utente: arg_str_opt(&args, "utente").unwrap_or_default(),
            url: arg_str_opt(&args, "url").unwrap_or_default(),
            categoria: arg_str_opt(&args, "categoria").unwrap_or_default(),
            note: arg_str_opt(&args, "note").unwrap_or_default(),
            ..Default::default()
        };
        let s = tokio::task::spawn_blocking(move || segreti::salva_voce(v)).await??;
        Ok(json!({ "salvata": s.nome, "robustezza": s.robustezza, "scheda": s }))
    }
}

struct GeneraCap;

#[async_trait]
impl Capability for GeneraCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "segreti.genera".into(),
            description: "Genera una password nuova e la salva sotto il nome dato. Non \
                          la restituisce: la generi, la usi con ui.set_text, e non e' \
                          mai passata da nessuna conversazione. E' il modo giusto di \
                          crearne una."
                .into(),
            risk: Risk::Moderate,
            category: "segreti".into(),
            schema: schema(&[
                ("nome", "string", "Come richiamarla", true),
                ("lunghezza", "integer", "Predefinita 20; minimo 12", false),
                ("servizio", "string", "Per che cosa", false),
                ("utente", "string", "Con quale utente", false),
                ("url", "string", "Dove si usa", false),
                ("categoria", "string", "posta, banca, lavoro...", false),
                ("mostra", "boolean", "true per riceverla comunque (evita, se puoi)", false),
            ]),
        }
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let lunghezza = arg_u64(&args, "lunghezza", 20) as usize;
        let mostra = arg_bool(&args, "mostra", false);
        let nome = arg_str(&args, "nome")?;
        let servizio = arg_str_opt(&args, "servizio").unwrap_or_default();
        let utente = arg_str_opt(&args, "utente").unwrap_or_default();
        let url = arg_str_opt(&args, "url").unwrap_or_default();
        let categoria = arg_str_opt(&args, "categoria").unwrap_or_default();

        let (scheda, valore) = tokio::task::spawn_blocking(move || -> Result<_> {
            let valore = segreti::genera(lunghezza)?;
            let s = segreti::salva_voce(Voce {
                nome,
                valore: valore.clone(),
                servizio,
                utente,
                url,
                categoria,
                ..Default::default()
            })?;
            Ok((s, valore))
        })
        .await??;

        let mut fuori = json!({
            "creata": scheda.nome,
            "robustezza": scheda.robustezza,
            "lunghezza": scheda.lunghezza,
            "nota": "usala passando «segreto» a ui.set_text: cosi' non passa da qui",
        });
        if mostra {
            fuori["valore"] = json!(valore);
        }
        Ok(fuori)
    }
}

struct DimenticaCap;

#[async_trait]
impl Capability for DimenticaCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "segreti.dimentica".into(),
            description: "Toglie una credenziale dall'archivio. Non si torna indietro.".into(),
            risk: Risk::Dangerous,
            category: "segreti".into(),
            schema: schema(&[("nome", "string", "Quale", true)]),
        }
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let nome = arg_str(&args, "nome")?;
        let tolta = tokio::task::spawn_blocking(move || segreti::dimentica(&nome)).await??;
        Ok(json!({ "tolta": tolta }))
    }
}

/// Il mestiere che un archivio di password non fa mai: dire cosa non va.
struct OrdineCap;

#[async_trait]
impl Capability for OrdineCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "segreti.ordine".into(),
            description: "Cosa non va nell'archivio: password ripetute su piu' servizi, \
                          deboli, vecchie, o voci a cui manca l'utente o l'indirizzo. \
                          Nessun valore nella risposta — solo dove intervenire."
                .into(),
            risk: Risk::Safe,
            category: "segreti".into(),
            schema: schema(&[
                ("vecchia_dopo_giorni", "integer", "Oltre quanti giorni e' «vecchia» (predefinito 365)", false),
            ]),
        }
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let soglia = arg_u64(&args, "vecchia_dopo_giorni", 365);
        let tutte = tokio::task::spawn_blocking(segreti::elenco).await??;
        let nome = |s: &segreti::Scheda| s.nome.clone();

        let ripetute: Vec<String> = tutte.iter().filter(|s| s.ripetuta_in > 0).map(nome).collect();
        let deboli: Vec<String> = tutte.iter().filter(|s| s.robustezza <= 1).map(nome).collect();
        let vecchie: Vec<String> = tutte
            .iter()
            .filter(|s| s.aggiornato > 0 && s.giorni_fa > soglia)
            .map(nome)
            .collect();
        let incomplete: Vec<String> = tutte
            .iter()
            .filter(|s| s.utente.is_empty() || s.servizio.is_empty())
            .map(nome)
            .collect();

        Ok(json!({
            "quante": tutte.len(),
            "ripetute_su_piu_servizi": ripetute,
            "deboli": deboli,
            "vecchie": vecchie,
            "senza_utente_o_servizio": incomplete,
            "tutto_a_posto": ripetute.is_empty() && deboli.is_empty()
                && vecchie.is_empty() && incomplete.is_empty(),
        }))
    }
}

// ------------------------------------------------------------ importare

/// Legge un file di credenziali scritto a mano e lo versa nell'archivio.
///
/// Il file di partenza è quello che quasi tutti hanno da qualche parte: un
/// blocco note con dentro servizi, indirizzi e password, in un formato che non
/// è un formato. Il lavoro sporco lo fa **qui**, nel demone: il contenuto non
/// passa da Python, non passa dal cervello, non passa da nessuna
/// conversazione. Ciò che esce da questa funzione sono conteggi, nomi di
/// servizio e utenti mascherati — mai un valore.
///
/// `prova` è acceso di proposito: prima si guarda cosa avrebbe capito, poi si
/// scrive. Un importatore che indovina male e scrive comunque è peggio del
/// file di partenza, perché sembra ordinato.
struct ImportaCap;

/// Una riga capita: cosa ci abbiamo letto dentro.
struct Letta {
    servizio: String,
    utente: String,
    valore: String,
    url: String,
    note: String,
}

/// Nasconde tutto tranne il primo carattere e il dominio: `g***@gmail.com`.
/// Serve per far vedere *che cosa* è stato riconosciuto senza dirlo.
fn maschera(u: &str) -> String {
    match u.split_once('@') {
        Some((testa, coda)) => {
            let primo: String = testa.chars().take(1).collect();
            format!("{primo}***@{coda}")
        }
        None => {
            let primo: String = u.chars().take(1).collect();
            format!("{primo}***")
        }
    }
}

/// Il servizio dedotto dal dominio, quando nessuno l'ha scritto.
fn servizio_da(u: &str, url: &str) -> String {
    // Dall'indirizzo del sito, se c'e'; altrimenti dal dominio della mail.
    let senza_schema = url.rsplit("://").next().unwrap_or("");
    let da_url: String = senza_schema.split('/').next().unwrap_or("").to_string();
    let dominio = if da_url.is_empty() {
        u.split_once('@').map(|(_, d)| d.to_string()).unwrap_or_default()
    } else {
        da_url
    };
    let nucleo = dominio
        .trim_start_matches("www.")
        .split('.')
        .next()
        .unwrap_or("")
        .to_string();
    if nucleo.is_empty() {
        return String::new();
    }
    // Iniziale maiuscola: «gmail» -> «Gmail». Serve solo a farlo leggere bene.
    let mut c = nucleo.chars();
    match c.next() {
        Some(primo) => {
            let testa: String = primo.to_uppercase().collect();
            testa + c.as_str()
        }
        None => nucleo,
    }
}

/// Legge il file a **blocchi**, non a righe.
///
/// La forma che la gente scrive davvero è questa: righe vuote a separare, e
/// dentro ogni blocco il nome del servizio, l'indirizzo, la password — in
/// quest'ordine, senza etichette. Il primo tentativo leggeva riga per riga
/// cercando `chiave: valore`, e su un file reale ha riconosciuto **1 voce su
/// 107 righe**: le intestazioni contenevano due punti («Posta del lavoro:») e
/// venivano scambiate per campi, così ogni blocco si chiudeva senza password e
/// veniva buttato.
///
/// Quindi si guarda la posizione, che è l'informazione che c'è davvero:
/// l'indirizzo fa da perno, ciò che sta sopra è il servizio, ciò che sta sotto
/// è il segreto. Le etichette esplicite restano gestite, perché quando ci sono
/// sono più affidabili della posizione.
fn interpreta(testo: &str) -> (Vec<Letta>, Vec<usize>) {
    let mut lette = Vec::new();
    let mut non_capiti = Vec::new();

    // I blocchi: gruppi di righe non vuote, con il numero della prima riga.
    let mut blocchi: Vec<(usize, Vec<&str>)> = Vec::new();
    let mut corrente: Vec<&str> = Vec::new();
    let mut inizio = 1usize;
    for (i, riga) in testo.lines().enumerate() {
        let r = riga.trim();
        if r.is_empty() {
            if !corrente.is_empty() {
                blocchi.push((inizio, std::mem::take(&mut corrente)));
            }
            inizio = i + 2;
        } else {
            if corrente.is_empty() {
                inizio = i + 1;
            }
            corrente.push(r);
        }
    }
    if !corrente.is_empty() {
        blocchi.push((inizio, corrente));
    }

    for (riga_iniziale, blocco) in blocchi {
        match leggi_blocco(&blocco) {
            Some(l) => lette.push(l),
            None => non_capiti.push(riga_iniziale),
        }
    }
    (lette, non_capiti)
}

/// L'indirizzo dentro una riga, se c'è.
fn indirizzo(riga: &str) -> Option<String> {
    riga.split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '|'))
        .map(|t| t.trim_matches(|c: char| !c.is_alphanumeric() && c != '@' && c != '.' && c != '_' && c != '-' && c != '+'))
        .find(|t| {
            let Some((testa, coda)) = t.split_once('@') else { return false };
            !testa.is_empty() && coda.contains('.') && coda.len() >= 4
        })
        .map(str::to_string)
}

/// `etichetta: valore`, ma solo quando l'etichetta è davvero un'etichetta.
fn etichetta(riga: &str) -> Option<(String, String)> {
    let (k, v) = riga.split_once(':')?;
    let chiave = k.trim().to_lowercase();
    let valore = v.trim();
    let sensata = !chiave.is_empty()
        && chiave.len() <= 12
        && chiave.split_whitespace().count() == 1
        && chiave.chars().all(|c| c.is_alphabetic());
    (sensata && !valore.is_empty()).then(|| (chiave, valore.to_string()))
}

fn leggi_blocco(righe: &[&str]) -> Option<Letta> {
    // Strada 1: etichette esplicite. Quando ci sono, comandano loro.
    let mut campi = std::collections::HashMap::new();
    for r in righe {
        if let Some((k, v)) = etichetta(r) {
            campi.entry(k).or_insert(v);
        }
    }
    let prendi = |chiavi: &[&str]| -> String {
        chiavi.iter().find_map(|k| campi.get(*k).cloned()).unwrap_or_default()
    };
    let per_etichetta = prendi(&["password", "pass", "pwd", "psw", "chiave"]);
    if !per_etichetta.is_empty() {
        let utente = prendi(&["mail", "email", "utente", "user", "username", "account"]);
        let url = prendi(&["url", "sito", "link", "indirizzo"]);
        let servizio = {
            let s = prendi(&["servizio", "nome"]);
            if !s.is_empty() {
                s
            } else {
                // L'intestazione del blocco: la prima riga che non è
                // un'etichetta e non è un indirizzo.
                righe
                    .iter()
                    .find(|r| etichetta(r).is_none() && indirizzo(r).is_none())
                    .map(|r| ripulisci_titolo(r))
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| servizio_da(&utente, &url))
            }
        };
        return Some(Letta { servizio, utente, valore: per_etichetta, url, note: String::new() });
    }

    // Strada 2: la posizione. L'indirizzo è il perno.
    let Some(perno) = righe.iter().position(|r| indirizzo(r).is_some()) else {
        // Nessun indirizzo: capita per i servizi dove si entra con un nome
        // utente. Con due o tre righe la lettura piu' probabile e' la piu'
        // semplice — nome del servizio, poi l'utente, poi il segreto.
        if righe.len() < 2 || righe.len() > 3 {
            return None;
        }
        let valore = righe[righe.len() - 1].trim().to_string();
        if valore.chars().count() < 6 {
            return None;
        }
        return Some(Letta {
            servizio: ripulisci_titolo(righe[0]),
            utente: if righe.len() == 3 { righe[1].trim().to_string() } else { String::new() },
            valore,
            url: String::new(),
            note: String::new(),
        });
    };
    let utente = indirizzo(righe[perno])?;

    // Sulla stessa riga può esserci già la password: il pezzo più lungo che
    // non è l'indirizzo.
    let sulla_riga = righe[perno]
        .split_whitespace()
        .map(|t| t.trim_matches(|c: char| matches!(c, '/' | '|' | ',' | ';')))
        .filter(|t| t.len() >= 4 && !t.contains('@'))
        .max_by_key(|t| t.len())
        .map(str::to_string);

    // Altrimenti la prima riga sotto che non sia un altro indirizzo: capita di
    // avere la mail di recupero subito dopo quella principale.
    let sotto = righe[perno + 1..]
        .iter()
        .find(|r| indirizzo(r).is_none())
        .map(|r| r.trim().to_string());

    // C'e' anche chi scrive la password *sopra* l'indirizzo. Se sotto non
    // c'e' niente si guarda la riga precedente — ma solo se piu' su resta
    // ancora una riga da usare come nome del servizio, altrimenti si
    // scambierebbe l'intestazione per un segreto.
    let sopra = (perno >= 2)
        .then(|| righe[perno - 1].trim().to_string())
        .filter(|r| indirizzo(r).is_none() && etichetta(r).is_none());

    let valore = sulla_riga.or(sotto).or(sopra)?;
    if valore.chars().count() < 3 {
        return None;
    }

    // Sopra l'indirizzo c'è il nome del servizio, se qualcuno l'ha scritto.
    let servizio = righe[..perno]
        .iter()
        .rev()
        .filter(|r| r.trim() != valore)
        .map(|r| ripulisci_titolo(r))
        .find(|s| !s.is_empty())
        .unwrap_or_else(|| servizio_da(&utente, ""));

    // Ciò che avanza sotto la password è una nota, non un segreto: si conta e
    // basta, senza portarselo dietro.
    let avanzo = righe[perno + 1..]
        .iter()
        .filter(|r| r.trim() != valore)
        .count();
    let note = if avanzo > 0 {
        format!("nel file c'erano altre {avanzo} righe in questo blocco")
    } else {
        String::new()
    };

    Some(Letta { servizio, utente, valore, url: String::new(), note })
}

/// Un'intestazione ripulita: «Posta del lavoro:» -> «Posta del lavoro».
fn ripulisci_titolo(r: &str) -> String {
    let t = r.trim().trim_end_matches(':').trim();
    // Una riga lunga o piena di simboli non è il nome di un servizio.
    if t.is_empty() || t.chars().count() > 40 || t.split_whitespace().count() > 5 {
        return String::new();
    }
    t.to_string()
}

#[async_trait]
impl Capability for ImportaCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "segreti.importa".into(),
            description: "Legge un file di credenziali scritto a mano e lo versa \
                          nell'archivio cifrato. Di base fa una PROVA: dice cosa avrebbe \
                          capito — servizi e utenti mascherati, mai le password — e non \
                          scrive niente. Con «prova» a falso importa davvero. Il \
                          contenuto del file non esce mai dal demone."
                .into(),
            risk: Risk::Moderate,
            category: "segreti".into(),
            schema: schema(&[
                ("percorso", "string", "Il file da leggere", true),
                ("prova", "boolean", "true (predefinito) guarda e basta; false importa", false),
                ("prefisso", "string", "Anteposto ai nomi, es. «vecchie» -> vecchie.gmail", false),
            ]),
        }
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let percorso = arg_str(&args, "percorso")?;
        let prova = arg_bool(&args, "prova", true);
        let prefisso = arg_str_opt(&args, "prefisso").unwrap_or_default();

        let esito = tokio::task::spawn_blocking(move || -> Result<Value> {
            let grezzo = std::fs::read(&percorso)
                .map_err(|e| anyhow!("non riesco a leggere «{percorso}»: {e}"))?;
            let testo = String::from_utf8_lossy(&grezzo);
            let (lette, non_capite) = interpreta(&testo);

            let mut riconosciute = Vec::new();
            let mut scritte = 0usize;
            let mut nomi_usati = std::collections::HashSet::new();
            // Un file scritto a mano quasi sempre contiene la stessa cosa due
            // volte: si copia un blocco per aggiornarlo e poi non si cancella
            // il vecchio. Importarli entrambi vorrebbe dire portare il
            // disordine dentro l'archivio che serve a toglierlo.
            let mut gia_viste = std::collections::HashSet::new();
            let mut doppioni = 0usize;

            for l in &lette {
                if !gia_viste.insert((
                    l.servizio.to_lowercase(),
                    l.utente.to_lowercase(),
                    l.valore.clone(),
                )) {
                    doppioni += 1;
                    continue;
                }
                let base = if l.servizio.is_empty() { "sconosciuto".to_string() }
                           else { l.servizio.to_lowercase().replace(' ', "-") };
                let mut nome = if prefisso.is_empty() { base.clone() }
                               else { format!("{}.{}", prefisso.trim(), base) };
                let mut n = 2;
                while !nomi_usati.insert(nome.clone()) {
                    nome = format!("{nome}-{n}");
                    n += 1;
                }
                riconosciute.push(json!({
                    "nome": nome,
                    "servizio": l.servizio,
                    "utente": maschera(&l.utente),
                    "robustezza": crate::segreti::robustezza(&l.valore),
                    "caratteri": l.valore.chars().count(),
                }));
                if !prova {
                    crate::segreti::salva_voce(crate::segreti::Voce {
                        nome,
                        valore: l.valore.clone(),
                        servizio: l.servizio.clone(),
                        utente: l.utente.clone(),
                        url: l.url.clone(),
                        categoria: "importata".into(),
                        note: if l.note.is_empty() {
                            "importata da un file scritto a mano".into()
                        } else {
                            format!("importata da un file scritto a mano; {}", l.note)
                        },
                        ..Default::default()
                    })?;
                    scritte += 1;
                }
            }
            Ok(json!({
                "prova": prova,
                "righe_nel_file": testo.lines().count(),
                "riconosciute": riconosciute.len(),
                "doppioni_saltati": doppioni,
                "scritte": scritte,
                "credenziali": riconosciute,
                "righe_non_capite": non_capite,
                "nota": if prova {
                    "niente e' stato scritto: richiama con prova=false per importare"
                } else {
                    "importate; ora il file in chiaro andrebbe tolto di mezzo"
                },
            }))
        })
        .await??;
        Ok(esito)
    }
}

/// Il valore di una credenziale, per chi deve *usarla* senza vederla.
///
/// La chiama `ui.set_text` quando riceve `segreto` invece di `text`: il valore
/// esce dall'archivio, entra nel campo, e non compare da nessuna parte in
/// mezzo. È il motivo per cui l'archivio non indebolisce niente — un'iniezione
/// perfetta non può far uscire ciò che non è mai entrato.
pub fn valore_per_uso(nome: &str) -> Result<String> {
    segreti::leggi(nome).map_err(|e| anyhow!("{e}"))
}
