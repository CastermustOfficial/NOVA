//! «Dove sono i miei dati?», dalla parte del demone (D352).
//!
//! Gemello di `posti()` e `tutto()` in `nova/dati.py`. Le regole — come si
//! scrive una misura, il racconto, il rendiconto per chi disinstalla — stanno
//! in `nova-dati`, gemellate da un banco; qui c'e' l'elenco dei posti, e ogni
//! percorso lo dice **il modulo che ci scrive** quando nel demone ce n'e'
//! uno. Quelli che scriveva solo la meta' Python (i guasti, il browser delle
//! ricerche, il filo di Claude Code, il diario delle procedure) si compongono
//! qui accanto agli altri: il file resta sul disco anche quando chi lo
//! scriveva non c'e' piu', e una mappa che tace su un posto sembra completa.

use std::path::{Path, PathBuf};

use nova_dati::{Misura, Posto};
use serde_json::{json, Value};

/// La cartella di NOVA come la intende il Python: `%APPDATA%\NOVA`, o
/// `~/.config/NOVA`.
pub fn base() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(|b| PathBuf::from(b).join("NOVA"))
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default()
                .join(".config")
                .join("NOVA")
        })
}

fn casa() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_default()
}

/// Tutto quello che NOVA scrive, in ordine di quanto e' delicato.
pub fn posti(cfg: &Value) -> Vec<Posto> {
    let b = base();
    let segreti = crate::segreti::percorso().unwrap_or_else(|_| b.join("segreti.dat"));
    vec![
        Posto::nuovo(
            "Le credenziali",
            segreti,
            "NOVA non sa piu' entrare da nessuna parte, e le password vanno rimesse una per \
             una. Cifrato con DPAPI: senza il tuo account Windows non lo apre nessuno, nemmeno tu.",
        )
        .delicato(),
        Posto::nuovo(
            "Il fascicolo (CV, esperienze, testi tuoi)",
            crate::fascicolo::cartella(cfg),
            "NOVA torna a non sapere niente di te, e quando scrive a nome tuo deve richiedere \
             tutto. Sono file tuoi: stanno in Documenti apposta, per poterli aprire e correggere \
             a mano.",
        )
        .delicato(),
        Posto::nuovo(
            "La memoria a grafo",
            crate::memoria::percorso(cfg, &crate::memoria::radice_progetto()),
            "NOVA dimentica quello che ha imparato su di te e sul PC. Sono file .md leggibili: \
             si aprono in Obsidian, o in un editor qualunque.",
        ),
        Posto::nuovo(
            "Il registro delle azioni",
            crate::registro::percorso(),
            "Si perde la traccia di cosa NOVA ha fatto e non si puo' annullare. Non cambia \
             niente di come funziona; cambia cosa puoi rivedere.",
        ),
        Posto::nuovo(
            "Le procedure imparate",
            crate::ricette::percorso(),
            "NOVA rifa' da capo le strade che aveva gia' trovato: torna a funzionare, ci mette \
             solo di piu'.",
        ),
        Posto::nuovo(
            "Le automazioni che si e' scritta",
            crate::automazioni::cartella(),
            "Spariscono gli strumenti che NOVA si e' costruita da sola. Se le riservono, se le \
             riscrive.",
        ),
        Posto::nuovo(
            "La configurazione",
            nova_configurazione::dove::percorso(),
            "NOVA riparte come appena installata: si rifa' la scelta del cervello e dei \
             permessi. Qui dentro puo' esserci una chiave API, se ne hai messa una.",
        )
        .delicato(),
        Posto::nuovo(
            "Le voci del recinto sulle cartelle",
            b.join("recinto.json"),
            "NOVA non sa piu' su quali cartelle ha scritto i permessi del recinto di Windows, e \
             quelle voci restano dove sono: si tolgono con `novad --recinto --togli`. Su Linux il \
             file non esiste.",
        )
        .delicato(),
        Posto::nuovo(
            "Il controllo delle cartelle di terzi",
            b.join("recinto-controllo.json"),
            "Niente: si rifa' da solo all'avvio del demone. Dice quali cartelle di terzi il \
             contenitore di Windows puo' scrivere, e quando e' stato fatto il controllo.",
        ),
        Posto::nuovo(
            "I guasti",
            b.join("guasti.jsonl"),
            "Si perde il racconto di cosa e' andato storto. Serve solo a chi ripara: \
             cancellarlo non rompe niente.",
        ),
        Posto::nuovo(
            "L'harness",
            crate::caps_harness::base(),
            "Si chiudono i documenti aperti e si perdono le proposte in attesa. I documenti veri \
             non si toccano.",
        ),
        Posto::nuovo(
            "Le cose in programma",
            crate::pianificate::percorso(),
            "NOVA smette di fare da sola le cose ricorrenti. Le attivita' di Windows restano: si \
             tolgono con «install.ps1 -Disinstalla».",
        ),
        Posto::nuovo(
            "Gli avvisi gia' dati",
            crate::pianificate::avvisi_percorso(),
            "Si perde la traccia di cosa NOVA ti ha gia' segnalato. Non cambia niente di come \
             funziona: al massimo ti ridice una cosa che avevi gia' letto.",
        ),
        Posto::nuovo(
            "I log di avvio",
            crate::mondo::cartella_nova().join("logs"),
            "Si perde il racconto delle accensioni. Serve a chi ripara quando NOVA non parte: \
             cancellarlo non rompe niente.",
        ),
        Posto::nuovo(
            "Il browser di NOVA (cookie e sessioni aperte)",
            crate::caps_web::profilo(),
            "NOVA deve rifare l'accesso a tutti i siti su cui lavora per te. Non tocca il tuo \
             browser: questo e' un profilo suo, separato apposta.",
        )
        .delicato(),
        Posto::nuovo(
            "Il browser delle ricerche",
            b.join("browser-cerca"),
            "Niente: si ricrea alla prima ricerca. E' un profilo separato da quello di lavoro \
             proprio per non mescolare le due cose.",
        ),
        Posto::nuovo(
            "Le schermate che ha scattato",
            nova_strumenti::schermo::cartella(&casa()),
            "Si perdono le immagini che NOVA ha catturato dello schermo. Sono fotografie di cio' \
             che stavi guardando: se le cancelli non si rompe niente.",
        )
        .delicato(),
        Posto::nuovo(
            "Il filo della conversazione",
            b.join("sessione.json"),
            "La prossima frase apre una conversazione nuova invece di continuare quella di \
             prima. Dentro c'e' solo un identificativo, non il testo.",
        ),
        Posto::nuovo(
            "Il prompt di sistema passato al cervello",
            b.join("prompt_sistema.txt"),
            "Niente: si riscrive al primo turno. Esiste perche' su Windows la riga di comando \
             non regge ottomila caratteri.",
        ),
        Posto::nuovo(
            "Il diario delle procedure imparate",
            b.join("procedure.log"),
            "Si perde la traccia di quando NOVA ha imparato cosa. Le procedure restano: quelle \
             stanno in ricette.json.",
        ),
    ]
}

/// Il GGUF configurato: non e' roba di NOVA, ed e' la cosa piu' pesante.
/// Non si cancella, ma si dice dov'e' e quanto pesa.
pub fn il_modello(cfg: &Value) -> Option<Posto> {
    let p = cfg.pointer("/server/model_path").and_then(Value::as_str)?.trim();
    if p.is_empty() || !Path::new(p).is_file() {
        return None;
    }
    Some(Posto::nuovo(
        "Il modello scaricato",
        p,
        "NOVA non ha piu' un cervello locale finche' non ne scarichi un altro. Non e' un file \
         di NOVA: se usi anche LM Studio o llama.cpp, e' lo stesso che usano loro.",
    ))
}

/// Quanto pesa, guardando il disco.
pub fn misura(p: &Path) -> Misura {
    fn giu(d: &Path, byte: &mut u64, file: &mut u64) {
        for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
            let p = e.path();
            match e.file_type() {
                Ok(t) if t.is_dir() => giu(&p, byte, file),
                Ok(t) if t.is_file() => {
                    *byte += e.metadata().map(|m| m.len()).unwrap_or(0);
                    *file += 1;
                }
                _ => {}
            }
        }
    }
    if !p.exists() {
        return Misura::default();
    }
    if p.is_file() {
        return Misura { esiste: true, byte: std::fs::metadata(p).map(|m| m.len()).unwrap_or(0), quanti_file: 1 };
    }
    let (mut byte, mut file) = (0, 0);
    giu(p, &mut byte, &mut file);
    Misura { esiste: true, byte, quanti_file: file }
}

/// Tutto, modello compreso, misurato.
pub fn tutto(cfg: &Value) -> Vec<(Posto, Misura)> {
    posti(cfg)
        .into_iter()
        .chain(il_modello(cfg))
        .map(|p| {
            let m = misura(&p.dove);
            (p, m)
        })
        .collect()
}

/// L'elenco, in una forma che si legge.
pub fn racconto(cfg: &Value) -> String {
    nova_dati::racconta(&tutto(cfg), &base(), true)
}

/// L'inventario in JSON, per chi disinstalla: `novad --dati --json`.
pub fn rendiconto(cfg: &Value) -> Value {
    let b = base();
    let (voci, totale) = nova_dati::rendiconto(&tutto(cfg), &b);
    json!({
        "base": b.to_string_lossy(),
        "voci": voci.iter().map(|v| json!({
            "che_cos_e": v.che_cos_e, "dove": v.dove, "byte": v.byte, "misura": v.misura,
            "delicato": v.delicato, "va_via_con_la_cartella": v.va_via_con_la_cartella,
            "se_lo_cancelli": v.se_lo_cancelli,
        })).collect::<Vec<_>>(),
        "totale": totale,
    })
}
