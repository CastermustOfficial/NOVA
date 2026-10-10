//! AR, le risorse dell'azienda dei Dot: chi lavora, e con che cervello (D397).
//!
//! Deciso con Gio il 9 ottobre 2026 (`docs/dots.md`, «Come un'azienda»).
//! Quando serve un Dot, Nova non lo fa nascere da se': chiede ad AR. AR
//! guarda i Dot che prendono compiti e **riprende** quello che fa al caso, se
//! e' libero; se nessuno fa al caso, ne **assume** uno nuovo. E per il
//! compito sceglie il **cervello di tutto il compito**: il Dot lo usa dal
//! primo all'ultimo passo, e il revisore puo' ancora far rifare un passo
//! scarso un gradino piu' su (si registra come salita: e' il segnale che AR
//! aveva scelto basso).
//!
//! AR decide **col cervello piu' grande**, con una domanda sola e senza
//! strumenti, come il custode quando il modello di casa non sa (D384). Qui ci
//! sono la domanda e come si legge la risposta; chi chiede e' `nova_core::risorse`.

use serde::Serialize;

use crate::ricerca::{nella_scala, oggetto_dentro, tagliato};
use crate::{Compito, Dot, Mestiere};

/// Quanto del ruolo di ogni Dot entra nella domanda.
pub const RUOLO_MASSIMO: usize = 200;

/// Quanto del bisogno e del compito entra nella domanda.
pub const TESTO_MASSIMO: usize = 2_000;

/// Quanti Dot entrano nella domanda: i piu' liberi e con piu' lavoro fatto
/// prima. Oltre, la domanda diventa un elenco che nessuno legge.
pub const CANDIDATI_MASSIMI: usize = 40;

/// Un Dot che AR puo' riprendere.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Candidato {
    pub nome: String,
    pub ruolo: String,
    pub mestiere: Mestiere,
    /// Senza compiti in coda, in corso o in attesa.
    pub libero: bool,
    pub fatti: usize,
    pub falliti: usize,
}

/// Cosa ha scelto AR.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "scelta", rename_all = "snake_case")]
pub enum Scelta {
    /// Un Dot che c'e' gia'.
    Riprendi { dot: String, cervello: String, perche: String },
    /// Un Dot nuovo.
    Assumi {
        nome: String,
        ruolo: String,
        mestiere: Mestiere,
        cervello: String,
        perche: String,
    },
}

impl Scelta {
    /// Il Dot scelto, nuovo o ripreso.
    pub fn dot(&self) -> &str {
        match self {
            Scelta::Riprendi { dot, .. } => dot,
            Scelta::Assumi { nome, .. } => nome,
        }
    }

    /// Il cervello scelto per il compito.
    pub fn cervello(&self) -> &str {
        match self {
            Scelta::Riprendi { cervello, .. } | Scelta::Assumi { cervello, .. } => cervello,
        }
    }
}

/// I candidati nell'ordine in cui AR li legge: prima i liberi, poi chi ha
/// fatto piu' compiti; e al massimo [`CANDIDATI_MASSIMI`].
pub fn in_ordine(mut c: Vec<Candidato>) -> Vec<Candidato> {
    c.sort_by(|a, b| b.libero.cmp(&a.libero).then(b.fatti.cmp(&a.fatti)).then(a.nome.cmp(&b.nome)));
    c.truncate(CANDIDATI_MASSIMI);
    c
}

/// La domanda al cervello grande. `scala` va dal cervello piu' piccolo al
/// piu' grande, con i nomi veri dei gradini.
pub fn richiesta(bisogno: &str, compito: &str, candidati: &[Candidato], scala: &[String]) -> String {
    let elenco = if candidati.is_empty() {
        "(nessuno)".to_string()
    } else {
        candidati
            .iter()
            .map(|c| {
                format!(
                    "- {} ({}, {}, {} compiti fatti, {} falliti): {}",
                    c.nome,
                    nome_mestiere(c.mestiere),
                    if c.libero { "libero" } else { "occupato" },
                    c.fatti,
                    c.falliti,
                    tagliato(&c.ruolo, RUOLO_MASSIMO),
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let compito = if compito.trim().is_empty() {
        "(nessuno per ora)".to_string()
    } else {
        tagliato(compito, TESTO_MASSIMO)
    };
    let esempio = scala.first().map(String::as_str).unwrap_or("");
    format!(
        "[risorse] Sei AR, le risorse dell'azienda dei Dot di NOVA: scegli chi fa un lavoro e \
         con che cervello.\n\n\
         Cosa serve: {}\n\
         Il compito: {compito}\n\
         I Dot che ci sono e prendono compiti:\n{elenco}\n\
         I cervelli, dal piu' piccolo al piu' grande: {}\n\n\
         Riprendi un Dot che c'e' se fa al caso ed e' libero; assumine uno nuovo solo se nessuno \
         fa al caso. Per il compito scegli il cervello piu' piccolo che lo sa fare bene: un \
         revisore puo' farlo rifare piu' su, ma costa.\n\
         Rispondi solo con un oggetto JSON, uno di questi due:\n\
         {{\"scelta\": \"riprendi\", \"dot\": \"nome\", \"cervello\": \"{esempio}\", \"perche\": \"una riga\"}}\n\
         {{\"scelta\": \"assumi\", \"nome\": \"minuscole-e-trattini\", \"ruolo\": \"chi e', cosa fa\", \
         \"mestiere\": \"generico o ricercatore\", \"cervello\": \"{esempio}\", \"perche\": \"una riga\"}}",
        tagliato(bisogno, TESTO_MASSIMO),
        scala.join(", "),
    )
}

fn nome_mestiere(m: Mestiere) -> &'static str {
    match m {
        Mestiere::Ricercatore => "ricercatore",
        _ => "generico",
    }
}

/// Legge la risposta del cervello grande. Torna la scelta e le note su cosa
/// si e' corretto; `Err` se non si legge, o se sceglie un Dot che non c'e'.
///
/// Un cervello che non e' nella scala diventa il piu' grande, e si dice; un
/// mestiere che non e' «generico» o «ricercatore» diventa generico, e si
/// dice. Un nome nuovo che e' gia' di un candidato e' quel candidato.
pub fn leggi_scelta(
    testo: &str,
    candidati: &[Candidato],
    scala: &[String],
) -> Result<(Scelta, Vec<String>), String> {
    let cima = scala.last().cloned().ok_or("la scala dei cervelli e' vuota")?;
    let oggetto = oggetto_dentro(testo).ok_or("la risposta di AR non ha un oggetto JSON")?;
    let v: serde_json::Value =
        serde_json::from_str(oggetto).map_err(|e| format!("la risposta di AR non si legge: {e}"))?;
    let campo = |n: &str| v.get(n).and_then(|x| x.as_str()).unwrap_or("").trim().to_string();
    let mut note = Vec::new();
    let cervello = {
        let scritto = campo("cervello");
        match nella_scala(&scritto, scala) {
            Some(g) => g,
            None => {
                note.push(format!(
                    "«{scritto}» non e' nella scala: il compito va al cervello piu' grande, «{cima}»"
                ));
                cima.clone()
            }
        }
    };
    let perche = campo("perche");
    let candidato = |nome: &str| {
        candidati
            .iter()
            .find(|c| c.nome == nome || c.nome.to_lowercase() == nome.to_lowercase())
            .map(|c| c.nome.clone())
    };
    match campo("scelta").to_lowercase().as_str() {
        "riprendi" => {
            let scritto = campo("dot");
            let dot = candidato(&scritto)
                .ok_or_else(|| format!("AR ha scelto «{scritto}», che non e' fra i Dot che prendono compiti"))?;
            Ok((Scelta::Riprendi { dot, cervello, perche }, note))
        }
        "assumi" => {
            let nome = crate::nome_valido(&campo("nome").to_lowercase())
                .map_err(|e| format!("AR ha scelto un nome che non va: {e}"))?;
            if let Some(dot) = candidato(&nome) {
                note.push(format!("«{dot}» c'e' gia': lo riprende invece di assumerlo"));
                return Ok((Scelta::Riprendi { dot, cervello, perche }, note));
            }
            let ruolo = campo("ruolo");
            if ruolo.is_empty() {
                return Err("AR vuole assumere un Dot senza dirgli chi e'".into());
            }
            let scritto = campo("mestiere").to_lowercase();
            let mestiere = match Mestiere::da(&scritto) {
                Ok(m @ (Mestiere::Generico | Mestiere::Ricercatore)) => m,
                _ => {
                    note.push(format!("«{scritto}» non e' un mestiere da assumere: generico"));
                    Mestiere::Generico
                }
            };
            Ok((Scelta::Assumi { nome, ruolo, mestiere, cervello, perche }, note))
        }
        altro => Err(format!("AR ha risposto «{altro}»: ne' riprendi ne' assumi")),
    }
}

/// Dopo quanti giorni senza lavoro AR licenzia un Dot che ha assunto lui.
pub const GIORNI_DA_FERMO: i64 = 30;

/// L'ultima volta che un Dot ha fatto qualcosa: quando e' nato, o quando un
/// suo compito e' entrato in coda, e' cominciato o e' finito. Le date sono
/// quelle del demone, ISO locali, e si confrontano come testo.
pub fn ultima_attivita(dot: &Dot, coda: &[Compito]) -> String {
    coda.iter()
        .flat_map(|c| [&c.affidato, &c.iniziato, &c.finito])
        .chain(std::iter::once(&dot.nato))
        .filter(|q| !q.trim().is_empty())
        .max()
        .cloned()
        .unwrap_or_default()
}

/// Se un Dot si puo' licenziare: non un posto fisso, non chi non prende
/// compiti (il custode, la direzione, il legale), non un capo con dei
/// sottoposti, e non chi ha ancora un compito da finire.
pub fn si_puo_licenziare(dot: &Dot, coda: &[Compito], sottoposti: usize) -> Result<(), String> {
    if dot.fisso {
        return Err(format!("«{}» e' un posto fisso dell'azienda: non si licenzia", dot.nome));
    }
    if let Some((chi, _)) = dot.mestiere.a_parte() {
        return Err(format!("{chi} non si licenzia"));
    }
    if sottoposti > 0 {
        return Err(format!(
            "«{}» e' il capo di {sottoposti} Dot: prima si spostano loro",
            dot.nome
        ));
    }
    let aperti = coda.iter().filter(|c| !c.stato.chiuso()).count();
    if aperti > 0 {
        return Err(format!("«{}» ha ancora {aperti} compiti da finire", dot.nome));
    }
    Ok(())
}

/// Se AR licenzia questo Dot da solo: l'ha assunto lui, si puo' licenziare,
/// e non fa niente da prima di `soglia` (trenta giorni fa).
pub fn da_licenziare(dot: &Dot, coda: &[Compito], sottoposti: usize, soglia: &str) -> bool {
    dot.assunto
        && si_puo_licenziare(dot, coda, sottoposti).is_ok()
        && ultima_attivita(dot, coda).as_str() < soglia
}

#[cfg(test)]
mod prove {
    use super::*;

    fn scala() -> Vec<String> {
        vec!["piccolo".into(), "medio".into(), "grande".into()]
    }

    fn cand(nome: &str, libero: bool, fatti: usize) -> Candidato {
        Candidato {
            nome: nome.into(),
            ruolo: format!("ruolo di {nome}"),
            mestiere: Mestiere::Generico,
            libero,
            fatti,
            falliti: 0,
        }
    }

    #[test]
    fn la_domanda_dice_chi_c_e_i_cervelli_e_come_rispondere() {
        let c = vec![cand("ricerca", true, 3)];
        let r = richiesta("uno che legga i paper", "riassumi questo", &c, &scala());
        assert!(r.starts_with("[risorse] Sei AR"));
        assert!(r.contains("Cosa serve: uno che legga i paper"));
        assert!(r.contains("- ricerca (generico, libero, 3 compiti fatti, 0 falliti): ruolo di ricerca"));
        assert!(r.contains("dal piu' piccolo al piu' grande: piccolo, medio, grande"));
        assert!(r.contains(r#""cervello": "piccolo""#));
        let senza = richiesta("x", " ", &[], &scala());
        assert!(senza.contains("Il compito: (nessuno per ora)") && senza.contains("(nessuno)"));
    }

    #[test]
    fn prima_i_liberi_poi_chi_ha_lavorato_di_piu() {
        let o = in_ordine(vec![cand("a", false, 9), cand("b", true, 1), cand("c", true, 5)]);
        let nomi: Vec<&str> = o.iter().map(|c| c.nome.as_str()).collect();
        assert_eq!(nomi, ["c", "b", "a"]);
        let tanti = in_ordine((0..60).map(|i| cand(&format!("d{i}"), true, 0)).collect());
        assert_eq!(tanti.len(), CANDIDATI_MASSIMI);
    }

    #[test]
    fn riprendere_vuol_dire_uno_che_c_e() {
        let c = vec![cand("ricerca", true, 3)];
        let (s, note) = leggi_scelta(
            r#"Ecco: {"scelta": "riprendi", "dot": "Ricerca", "cervello": "Medio", "perche": "sa farlo"}"#,
            &c,
            &scala(),
        )
        .unwrap();
        assert_eq!(
            s,
            Scelta::Riprendi { dot: "ricerca".into(), cervello: "medio".into(), perche: "sa farlo".into() }
        );
        assert!(note.is_empty());
        let e = leggi_scelta(r#"{"scelta": "riprendi", "dot": "nessuno", "cervello": "medio"}"#, &c, &scala())
            .unwrap_err();
        assert!(e.contains("«nessuno»"), "{e}");
    }

    #[test]
    fn assumere_vuole_un_nome_che_va_e_un_ruolo() {
        let c = vec![cand("ricerca", true, 3)];
        let (s, note) = leggi_scelta(
            r#"{"scelta": "assumi", "nome": "Lettore-Paper", "ruolo": "Leggi i paper.",
                "mestiere": "ricercatore", "cervello": "grande", "perche": "manca"}"#,
            &c,
            &scala(),
        )
        .unwrap();
        assert_eq!(s.dot(), "lettore-paper");
        assert_eq!(s.cervello(), "grande");
        assert!(matches!(s, Scelta::Assumi { mestiere: Mestiere::Ricercatore, .. }));
        assert!(note.is_empty());
        // Un nome gia' preso da un candidato e' quel candidato.
        let (s, note) = leggi_scelta(
            r#"{"scelta": "assumi", "nome": "ricerca", "ruolo": "r", "cervello": "medio"}"#,
            &c,
            &scala(),
        )
        .unwrap();
        assert!(matches!(s, Scelta::Riprendi { .. }) && note[0].contains("c'e' gia'"));
        assert!(leggi_scelta(r#"{"scelta": "assumi", "nome": "a/b", "ruolo": "r"}"#, &c, &scala())
            .unwrap_err()
            .contains("nome"));
        assert!(leggi_scelta(r#"{"scelta": "assumi", "nome": "x", "ruolo": " "}"#, &c, &scala())
            .unwrap_err()
            .contains("chi e'"));
    }

    #[test]
    fn quello_che_non_va_si_corregge_e_si_dice() {
        let (s, note) = leggi_scelta(
            r#"{"scelta": "assumi", "nome": "x", "ruolo": "r", "mestiere": "custode", "cervello": "enorme"}"#,
            &[],
            &scala(),
        )
        .unwrap();
        assert_eq!(s.cervello(), "grande", "un cervello che non c'e' e' il piu' grande");
        assert!(matches!(s, Scelta::Assumi { mestiere: Mestiere::Generico, .. }));
        assert_eq!(note.len(), 2, "{note:?}");
        assert!(leggi_scelta("non so", &[], &scala()).unwrap_err().contains("JSON"));
        assert!(leggi_scelta(r#"{"scelta": "boh"}"#, &[], &scala()).unwrap_err().contains("«boh»"));
        assert!(leggi_scelta("{}", &[], &[]).unwrap_err().contains("vuota"));
    }

    fn assunto(nato: &str) -> Dot {
        Dot {
            nome: "lettore".into(),
            ruolo: "r".into(),
            nato: nato.into(),
            mestiere: Mestiere::Generico,
            capo: String::new(),
            fisso: false,
            assunto: true,
        }
    }

    fn fatto(affidato: &str, finito: &str) -> Compito {
        let mut c = crate::compiti(&format!(
            "{{\"id\":1,\"stato\":\"affidato\",\"quando\":\"{affidato}\",\"testo\":\"t\"}}"
        ))
        .remove(0);
        if !finito.is_empty() {
            c.stato = crate::Stato::Fatto;
            c.finito = finito.into();
        }
        c
    }

    #[test]
    fn l_ultima_attivita_e_la_data_piu_recente() {
        let d = assunto("2026-08-01T10:00:00");
        assert_eq!(ultima_attivita(&d, &[]), "2026-08-01T10:00:00");
        let c = fatto("2026-08-20T09:00:00", "2026-08-21T11:00:00");
        assert_eq!(ultima_attivita(&d, &[c]), "2026-08-21T11:00:00");
    }

    #[test]
    fn ar_licenzia_solo_chi_ha_assunto_fermo_da_prima_della_soglia() {
        let soglia = "2026-09-09T12:00:00";
        let vecchio = assunto("2026-08-01T10:00:00");
        assert!(da_licenziare(&vecchio, &[], 0, soglia));
        assert!(!da_licenziare(&assunto("2026-09-20T10:00:00"), &[], 0, soglia), "nato da poco");
        let lavora = fatto("2026-08-02T10:00:00", "2026-09-30T10:00:00");
        assert!(!da_licenziare(&vecchio, &[lavora], 0, soglia), "ha lavorato da poco");
        assert!(!da_licenziare(&vecchio, &[], 1, soglia), "e' un capo");
        let in_coda = fatto("2026-08-02T10:00:00", "");
        assert!(!da_licenziare(&vecchio, &[in_coda.clone()], 0, soglia), "ha un compito in coda");
        let dell_utente = Dot { assunto: false, ..vecchio.clone() };
        assert!(!da_licenziare(&dell_utente, &[], 0, soglia), "AR licenzia solo chi ha assunto lui");
        // L'utente si', ma non i posti fissi ne' chi non prende compiti.
        assert!(si_puo_licenziare(&dell_utente, &[], 0).is_ok());
        let fisso = Dot { fisso: true, ..vecchio.clone() };
        assert!(si_puo_licenziare(&fisso, &[], 0).unwrap_err().contains("posto fisso"));
        let custode = Dot { mestiere: Mestiere::Custode, ..dell_utente.clone() };
        assert!(si_puo_licenziare(&custode, &[], 0).unwrap_err().contains("non si licenzia"));
        assert!(si_puo_licenziare(&vecchio, &[in_coda], 0).unwrap_err().contains("da finire"));
        assert!(si_puo_licenziare(&vecchio, &[], 2).unwrap_err().contains("capo di 2"));
    }
}
