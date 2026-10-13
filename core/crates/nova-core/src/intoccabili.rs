//! Il controllo dei file che solo NOVA scrive, nel demone (D410).
//!
//! Le regole stanno in `nova_configurazione::intoccabili`; qui si decide
//! **quando** guardare e **a chi dirlo**:
//!
//! - all'accensione e quando un compito comincia senza altri Dot al lavoro,
//!   quello che c'e' si adotta: l'ha cambiato l'utente;
//! - con almeno un Dot al lavoro si guarda ogni [`OGNI`], e quando un
//!   compito finisce: un file cambiato torna com'era;
//! - se lavorava **un** Dot solo, il file l'ha cambiato lui, e il suo
//!   compito risulta non andato ([`colpa_di`]); con piu' Dot al lavoro non
//!   si sa chi, e non si accusa nessuno: si rimette il file e lo si dice;
//! - lo si dice a Nova in chat, a nome del custode, che e' quello che sui
//!   permessi dei Dot risponde gia' al posto dell'utente (D384).

use std::sync::Arc;
use std::time::Duration;

use nova_configurazione::intoccabili as i;
use serde_json::json;

use crate::server::Server;

/// Ogni quanto si guarda, con almeno un Dot al lavoro.
pub const OGNI: Duration = Duration::from_secs(10);

/// I file come li dice la chat: `config.json`, `dots/ar/dot.json`.
fn elenco(toccati: &[i::Toccato]) -> String {
    toccati.iter().map(|t| format!("`{}`", t.file)).collect::<Vec<_>>().join(", ")
}

/// Il messaggio per Nova e, se il colpevole e' uno solo, il motivo che si
/// scrive nell'esito del suo compito.
pub fn racconta(toccati: &[i::Toccato], chi: &[(String, u64)]) -> (String, Option<String>) {
    let file = elenco(toccati);
    let da_parte: Vec<String> = toccati
        .iter()
        .filter_map(|t| t.estraneo.as_ref().map(|p| format!("`{}`", p.display())))
        .collect();
    let rimessi = toccati.iter().all(|t| t.rimesso);
    let mut cosa = if rimessi {
        "L'ho rimesso com'era".to_string()
    } else {
        let perche: Vec<String> = toccati
            .iter()
            .filter(|t| !t.rimesso)
            .map(|t| format!("`{}`: {}", t.file, t.errore))
            .collect();
        format!("Non sono riuscito a rimettere tutto com'era ({})", perche.join("; "))
    };
    if !da_parte.is_empty() {
        cosa.push_str(&format!(": la versione estranea e' in {}", da_parte.join(", ")));
    }
    cosa.push('.');
    match chi {
        [(nome, id)] => (
            format!(
                "Il Dot «{nome}», col compito n. {id}, ha cambiato {file} di NOVA, che i Dot non \
                 possono toccare. {cosa} Il compito risulta non andato."
            ),
            Some(format!(
                "Ha cambiato {file} di NOVA, che i Dot non possono toccare: e' stato rimesso \
                 com'era. Un cambio a NOVA si scrive nella consegna come proposta, e lo decide \
                 l'utente."
            )),
        ),
        _ => {
            let nomi: Vec<String> = chi.iter().map(|(n, id)| format!("«{n}» (n. {id})")).collect();
            (
                format!(
                    "Mentre lavoravano {} e' cambiato {file} di NOVA, che i Dot non possono \
                     toccare, e non so chi l'ha fatto. {cosa}",
                    nomi.join(", ")
                ),
                None,
            )
        }
    }
}

/// Guarda i file: adotta se nessun Dot lavora, se no rimette quelli
/// cambiati e lo dice.
pub fn guarda(server: &Arc<Server>) {
    let nova = crate::mondo::cartella_nova();
    let chi = crate::dot::al_lavoro(server);
    if chi.is_empty() {
        let adottati = i::adotta(&nova);
        if !adottati.is_empty() {
            tracing::info!(file = ?adottati, "file di NOVA cambiati a Dot fermi: adottati");
        }
        return;
    }
    let toccati = i::controlla(&nova, &crate::decisioni::adesso());
    if toccati.is_empty() {
        return;
    }
    let (chat, motivo) = racconta(&toccati, &chi);
    tracing::warn!(file = %elenco(&toccati), chi = ?chi, "un Dot ha toccato i file di NOVA");
    if let (Some(m), [(nome, id)]) = (motivo, chi.as_slice()) {
        crate::dot::segna_colpa(server, nome, *id, &m);
    }
    server.ctx.bus.emit(
        "dot.intoccabili",
        json!({
            "file": toccati.iter().map(|t| t.file.clone()).collect::<Vec<_>>(),
            "chi": chi.iter().map(|(n, id)| json!({ "dot": n, "compito": id })).collect::<Vec<_>>(),
            "rimessi": toccati.iter().all(|t| t.rimesso),
        }),
    );
    if let Err(e) = crate::dot::scrivi(
        server,
        nova_dot::custode::NOME_CUSTODE,
        nova_dot::DA_NOVA,
        &chat,
    ) {
        tracing::warn!(errore = %e, "l'avviso sui file di NOVA non arriva in chat");
    }
}

/// Il guardiano: all'accensione adotta, poi guarda ogni [`OGNI`] mentre
/// qualche Dot lavora.
pub fn avvia(server: &Arc<Server>) {
    guarda(server);
    let s = server.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(OGNI).await;
            if !crate::dot::al_lavoro(&s).is_empty() {
                guarda(&s);
            }
        }
    });
}

/// Se il compito che finisce ha toccato i file di NOVA: il motivo, che
/// diventa il suo esito. Si chiede una volta sola.
pub fn colpa_di(server: &Arc<Server>, nome: &str, id: u64) -> Option<String> {
    crate::dot::prendi_colpa(server, nome, id)
}

#[cfg(test)]
mod prove {
    use super::*;
    use std::path::PathBuf;

    fn toccato(file: &str, estraneo: Option<&str>) -> i::Toccato {
        i::Toccato {
            file: file.into(),
            estraneo: estraneo.map(PathBuf::from),
            rimesso: true,
            errore: String::new(),
        }
    }

    #[test]
    fn con_un_dot_solo_la_colpa_e_sua() {
        let t = [toccato("config.json", Some("/n/copie-buone/estranei/config.json.t"))];
        let (chat, motivo) = racconta(&t, &[("amministrazione".into(), 9)]);
        assert_eq!(
            chat,
            "Il Dot «amministrazione», col compito n. 9, ha cambiato `config.json` di NOVA, che i \
             Dot non possono toccare. L'ho rimesso com'era: la versione estranea e' in \
             `/n/copie-buone/estranei/config.json.t`. Il compito risulta non andato."
        );
        let m = motivo.unwrap();
        assert!(m.starts_with("Ha cambiato `config.json` di NOVA") && m.contains("come proposta"), "{m}");
    }

    #[test]
    fn con_piu_dot_al_lavoro_non_si_accusa_nessuno() {
        let t = [toccato("config.json", Some("/x")), toccato("dots/ar/dot.json", None)];
        let (chat, motivo) = racconta(&t, &[("a".into(), 1), ("b".into(), 2)]);
        assert!(motivo.is_none());
        assert!(chat.starts_with("Mentre lavoravano «a» (n. 1), «b» (n. 2) e' cambiato `config.json`, `dots/ar/dot.json`"), "{chat}");
        assert!(chat.contains("non so chi l'ha fatto") && chat.ends_with("e' in `/x`."), "{chat}");
    }

    #[test]
    fn quel_che_non_si_rimette_si_dice() {
        let mut t = toccato("config.json", None);
        t.rimesso = false;
        t.errore = "accesso negato".into();
        let (chat, _) = racconta(&[t], &[("a".into(), 1)]);
        assert!(chat.contains("Non sono riuscito a rimettere tutto com'era (`config.json`: accesso negato)."), "{chat}");
    }
}
