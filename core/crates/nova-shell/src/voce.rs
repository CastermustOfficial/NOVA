//! Il giro che chiude la conversazione a voce.
//!
//! ```text
//!   demone            guscio                        demone
//!   voce.comando  ->  cervello (nova --ask)  ->  voce.parla
//!                            |
//!                            +-- [NOVA:FINE]  -> voce.fase dormiente
//!                            +-- [NOVA:PAUSA] -> voce.fase in_pausa
//! ```
//!
//! Sta nel guscio e non nel demone per un motivo solo: il cervello e' Python,
//! e chi sa dov'e' installato e' il guscio. Quando il ciclo dell'agente sara'
//! in Rust questo pezzo si sposta dentro al demone e sparisce un salto.
//!
//! Una domanda alla volta, in fila. Mentre NOVA pensa il microfono resta
//! aperto — e' giusto, si continua a parlare — ma le frasi che arrivano
//! aspettano il loro turno invece di far partire tre cervelli in parallelo.

use std::sync::OnceLock;

use serde_json::json;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

/// Quanto si legge ad alta voce prima di rimandare allo schermo.
///
/// Non e' una censura: e' che duemila caratteri letti sono quasi tre minuti
/// in cui non si puo' interrompere. Il nocciolo si dice, il resto si legge.
const LIMITE_PARLATO: usize = 1200;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Chiusura {
    Nessuna,
    Fine,
    Pausa,
}

/// Toglie il marcatore dalla risposta e dice quale c'era.
///
/// Tollerante di proposito: `[NOVA:FINE]`, `[nova: fine]`, `[ NOVA - FINE ]`
/// sono la stessa cosa. Un marcatore che funziona solo se scritto in un modo
/// e' un marcatore che ogni tanto non funziona, e quando non funziona la
/// conversazione non si chiude piu'.
pub fn separa(risposta: &str) -> (String, Chiusura) {
    let mut testo = String::with_capacity(risposta.len());
    let mut chiusura = Chiusura::Nessuna;
    let mut salta_fino_a = 0usize;
    for (i, c) in risposta.char_indices() {
        if i < salta_fino_a {
            continue;
        }
        if c == '[' {
            if let Some(rel) = risposta[i..].find(']') {
                if rel <= 24 {
                    let dentro: String = risposta[i + 1..i + rel]
                        .chars()
                        .filter(|c| c.is_alphanumeric())
                        .flat_map(char::to_lowercase)
                        .collect();
                    let visto = match dentro.as_str() {
                        "novafine" => Some(Chiusura::Fine),
                        "novapausa" => Some(Chiusura::Pausa),
                        "novaaperto" => Some(Chiusura::Nessuna),
                        _ => None,
                    };
                    if let Some(v) = visto {
                        chiusura = v;
                        salta_fino_a = i + rel + 1;
                        continue;
                    }
                }
            }
        }
        testo.push(c);
    }
    (testo.trim().to_string(), chiusura)
}

/// Da testo scritto a testo da dire.
///
/// Gli asterischi e i cancelletti letti ad alta voce sono rumore: il
/// sintetizzatore non li salta, li pronuncia. Il grassetto in una frase
/// parlata semplicemente non esiste.
pub fn per_la_voce(testo: &str) -> String {
    let mut fuori = String::with_capacity(testo.len());
    let mut in_recinto = false;
    for riga in testo.lines() {
        let t = riga.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            // Un blocco di codice non si legge: si guarda.
            in_recinto = !in_recinto;
            continue;
        }
        if in_recinto {
            continue;
        }
        let mut r = t.to_string();
        // Titoli, citazioni, elenchi: la struttura visiva non ha voce.
        r = r.trim_start_matches('#').trim_start_matches('>').to_string();
        let senza_punto = r.trim_start();
        if let Some(resto) = senza_punto
            .strip_prefix("- ")
            .or_else(|| senza_punto.strip_prefix("* "))
            .or_else(|| senza_punto.strip_prefix("+ "))
        {
            r = resto.to_string();
        }
        let r = togli_collegamenti(&r);
        let r: String = r.chars().filter(|c| !matches!(c, '*' | '`' | '_' | '#')).collect();
        let r = r.trim();
        if r.is_empty() {
            continue;
        }
        if !fuori.is_empty() {
            fuori.push(' ');
        }
        fuori.push_str(r);
    }
    fuori
}

/// `[etichetta](indirizzo)` -> `etichetta`. Un URL letto lettera per lettera
/// e' la cosa piu' inutile che una voce possa fare.
fn togli_collegamenti(riga: &str) -> String {
    let mut fuori = String::with_capacity(riga.len());
    let mut resto = riga;
    while let Some(apre) = resto.find('[') {
        let Some(chiude) = resto[apre..].find(']') else { break };
        let dopo = apre + chiude + 1;
        if !resto[dopo..].starts_with('(') {
            fuori.push_str(&resto[..dopo]);
            resto = &resto[dopo..];
            continue;
        }
        let Some(fine_url) = resto[dopo..].find(')') else { break };
        fuori.push_str(&resto[..apre]);
        fuori.push_str(&resto[apre + 1..apre + chiude]);
        resto = &resto[dopo + fine_url + 1..];
    }
    fuori.push_str(resto);
    fuori
}

/// Taglia sull'ultimo punto prima del limite, e lo dice.
pub fn accorcia(testo: &str) -> String {
    if testo.chars().count() <= LIMITE_PARLATO {
        return testo.to_string();
    }
    let limite = testo
        .char_indices()
        .nth(LIMITE_PARLATO)
        .map(|(i, _)| i)
        .unwrap_or(testo.len());
    let taglio = testo[..limite]
        .rfind(['.', '!', '?'])
        .map(|i| i + 1)
        .unwrap_or(limite);
    format!("{} Il resto e' nella chat.", testo[..taglio].trim())
}

static CODA: OnceLock<UnboundedSender<String>> = OnceLock::new();

/// Mette una frase in fila per il cervello. La chiama il bus.
pub fn manda(testo: String) {
    match CODA.get() {
        Some(tx) => {
            let _ = tx.send(testo);
        }
        None => tracing::warn!("comando vocale arrivato prima che la coda fosse pronta"),
    }
}

pub fn avvia(app: AppHandle) {
    let (tx, mut rx) = unbounded_channel::<String>();
    if CODA.set(tx).is_err() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        while let Some(testo) = rx.recv().await {
            let chiusura = un_giro(&app, testo).await;
            if chiusura != Chiusura::Nessuna {
                // Cio' che e' stato detto mentre NOVA rispondeva non vale
                // piu': la conversazione e' chiusa. Passarlo al cervello ora
                // vorrebbe dire riaprirla da sola, subito dopo averla chiusa.
                let mut buttate = 0;
                while rx.try_recv().is_ok() {
                    buttate += 1;
                }
                if buttate > 0 {
                    tracing::debug!(buttate, "frasi scartate dopo la chiusura");
                }
            }
        }
    });
}

async fn un_giro(app: &AppHandle, testo: String) -> Chiusura {
    crate::cronologia::aggiungi("utente", &testo);
    let _ = app.emit("nova://voce", json!({ "da": "utente", "testo": testo }));
    let _ = app.emit("nova://stato", json!({ "stato": "penso" }));

    let risposta = match crate::cervello::chiedi(testo, true).await {
        Ok(r) if !r.trim().is_empty() => r,
        Ok(_) => return Chiusura::Nessuna,
        Err(e) => {
            tracing::warn!(errore = %e, "il cervello non ha risposto");
            crate::cronologia::aggiungi("errore", &e);
            let _ = app.emit("nova://voce", json!({ "da": "errore", "testo": e }));
            // Tacere sarebbe la cosa peggiore: chi ha parlato resterebbe li'
            // a chiedersi se e' stato sentito o se il sistema e' morto.
            dillo(app, "Non ci sono riuscito.").await;
            return Chiusura::Nessuna;
        }
    };

    let (pulito, chiusura) = separa(&risposta);
    crate::cronologia::aggiungi("nova", &pulito);
    let _ = app.emit("nova://voce", json!({ "da": "nova", "testo": pulito }));
    dillo(app, &accorcia(&per_la_voce(&pulito))).await;

    match chiusura {
        Chiusura::Fine => {
            cambia_fase(app, "dormiente").await;
            // Chiudere vuol dire chiudere: la prossima volta che si dice il
            // nome si ricomincia, non si riprende. La differenza fra questo e
            // la pausa e' tutta qui.
            if let Err(e) = crate::cervello::dimentica() {
                tracing::warn!(errore = %e, "non riesco a chiudere la sessione");
            }
        }
        // In pausa il filo resta: «Nova, riprendiamo» deve ritrovare tutto.
        Chiusura::Pausa => cambia_fase(app, "in_pausa").await,
        Chiusura::Nessuna => {}
    }
    chiusura
}

async fn dillo(app: &AppHandle, testo: &str) {
    if testo.trim().is_empty() {
        return;
    }
    if let Err(e) = crate::demone::chiama(
        "voce.parla",
        json!({ "testo": testo, "aspetta": true }),
    )
    .await
    {
        tracing::warn!(errore = %e, "non riesco a far parlare NOVA");
        let _ = app.emit("nova://voce", json!({ "da": "errore", "testo": e.to_string() }));
    }
}

/// Il cambio di fase non si annuncia da qui: lo fa il demone, che emette
/// `voce.fase` sul bus e da li' torna a tutte le finestre. Annunciarlo anche
/// qui vorrebbe dire dirlo due volte, e una delle due sarebbe una bugia se la
/// chiamata fallisse.
async fn cambia_fase(_app: &AppHandle, fase: &str) {
    if let Err(e) = crate::demone::chiama("voce.fase", json!({ "fase": fase })).await {
        tracing::warn!(errore = %e, fase, "non riesco a cambiare fase");
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn il_marcatore_si_stacca() {
        let (t, c) = separa("Ciao, alla prossima. [NOVA:FINE]");
        assert_eq!(t, "Ciao, alla prossima.");
        assert_eq!(c, Chiusura::Fine);
    }

    #[test]
    fn scritto_storto_vale_lo_stesso() {
        let (t, c) = separa("Va bene, aspetto.\n[ nova : pausa ]");
        assert_eq!(t, "Va bene, aspetto.");
        assert_eq!(c, Chiusura::Pausa);
    }

    #[test]
    fn senza_marcatore_non_si_tocca_niente() {
        let (t, c) = separa("Sono le nove e venti.");
        assert_eq!(t, "Sono le nove e venti.");
        assert_eq!(c, Chiusura::Nessuna);
    }

    #[test]
    fn una_parentesi_qualunque_resta() {
        let (t, c) = separa("Il file si chiama [bozza] e sta sul desktop.");
        assert_eq!(t, "Il file si chiama [bozza] e sta sul desktop.");
        assert_eq!(c, Chiusura::Nessuna);
    }

    #[test]
    fn il_markdown_non_si_pronuncia() {
        let d = per_la_voce("## Titolo\n\n- **primo** punto\n- secondo\n\n```rust\nlet x = 1;\n```\n\nfine.");
        assert_eq!(d, "Titolo primo punto secondo fine.");
    }

    #[test]
    fn dei_collegamenti_si_legge_solo_il_nome() {
        assert_eq!(
            per_la_voce("Guarda [la guida](https://esempio.it/x?y=1)."),
            "Guarda la guida."
        );
    }

    #[test]
    fn le_risposte_lunghe_si_tagliano_su_una_frase() {
        let lunga = "Frase di prova. ".repeat(200);
        let corta = accorcia(&lunga);
        assert!(corta.chars().count() < lunga.chars().count());
        assert!(corta.ends_with("Il resto e' nella chat."));
        assert!(corta.contains("Frase di prova. Frase"));
    }
}
