//! Quello che l'harness mostra di un Dot (D391): i file che tocca, i
//! messaggi che manda, e cosa ha fatto passo per passo.
//!
//! Le scelte sulla bozza dei Dot nell'harness, il 9 ottobre: a sinistra
//! l'organigramma, i gruppi e **i file che i Dot stanno toccando**, come in
//! *Esplora*; al centro la scheda di un Dot, con la sua chat. Per la chat
//! servono anche i messaggi che il Dot **manda**: quelli che riceve stanno
//! gia' nella sua posta (D388).
//!
//! Tre file in piu' nella cartella del Dot:
//!
//! ```text
//! file.jsonl      un file toccato per riga: quando, compito, percorso, come, strumento
//! inviati.jsonl   i messaggi che ha mandato, come quelli della posta
//! ```
//!
//! e il diario (`diario.jsonl`), che c'era gia', si rilegge qui.
//!
//! Quali strumenti toccano un file, e con quale argomento, lo dice
//! [`TOCCANO`]. Un comando di shell puo' toccare file anche lui, ma quali non
//! si sa senza capire il comando: non si indovina.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{aggiungi_riga, Cartella, Messaggio, SCRITTURA};

/// Come un Dot ha toccato un file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Come {
    /// L'ha letto.
    Letto,
    /// L'ha scritto, creato o cambiato.
    Scritto,
    /// L'ha tolto di li': cancellato, o spostato altrove.
    Tolto,
}

/// Gli strumenti che toccano un file: il nome, l'argomento col percorso, e
/// come lo toccano. Uno spostamento toglie l'origine e scrive la
/// destinazione; una copia legge l'una e scrive l'altra.
pub const TOCCANO: [(&str, &str, Come); 9] = [
    ("documenti.leggi", "path", Come::Letto),
    ("fs.copy", "destination", Come::Scritto),
    ("fs.copy", "source", Come::Letto),
    ("fs.delete", "path", Come::Tolto),
    ("fs.edit", "path", Come::Scritto),
    ("fs.move", "destination", Come::Scritto),
    ("fs.move", "source", Come::Tolto),
    ("fs.read", "path", Come::Letto),
    ("fs.write", "path", Come::Scritto),
];

/// Quanti file toccati si mostrano al massimo, i piu' recenti.
pub const FILE_MOSTRATI: usize = 300;

/// Quante righe del diario si rileggono al massimo, le piu' recenti.
pub const PASSI_RILETTI: usize = 400;

/// I file che una chiamata tocca, e come. Il percorso e' quello scritto
/// negli argomenti, senza spazi attorno; uno vuoto non conta.
pub fn toccati(strumento: &str, argomenti: &Value) -> Vec<(String, Come)> {
    TOCCANO
        .iter()
        .filter(|(s, _, _)| *s == strumento)
        .filter_map(|(_, arg, come)| {
            let p = argomenti.get(*arg)?.as_str()?.trim();
            (!p.is_empty()).then(|| (p.to_string(), *come))
        })
        .collect()
}

/// Una riga di `file.jsonl`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tocco {
    pub quando: String,
    /// Il compito durante il quale l'ha toccato; zero se non ne faceva uno.
    #[serde(default)]
    pub compito: u64,
    pub percorso: String,
    pub come: Come,
    pub strumento: String,
}

impl Cartella {
    /// Scrive un file toccato.
    pub fn tocca(&self, t: &Tocco) -> Result<(), String> {
        let riga = serde_json::to_string(t).map_err(|e| e.to_string())?;
        aggiungi_riga(&self.radice.join("file.jsonl"), &riga)
    }

    /// I file toccati, in ordine. Una riga che non si legge si salta.
    pub fn tocchi(&self) -> Vec<Tocco> {
        righe(&self.radice.join("file.jsonl"))
    }

    /// Tiene una copia di un messaggio mandato. Il numero e' quello nella
    /// lista dei mandati, non nella posta di chi lo riceve.
    pub fn spedito(&self, da: &str, a: &str, testo: &str, quando: &str) -> Result<u64, String> {
        let _turno = SCRITTURA.lock().unwrap_or_else(|e| e.into_inner());
        let n = self.inviati().iter().map(|m| m.n).max().unwrap_or(0) + 1;
        let m = Messaggio {
            n,
            da: da.to_string(),
            a: a.to_string(),
            testo: testo.to_string(),
            quando: quando.to_string(),
        };
        let riga = serde_json::to_string(&m).map_err(|e| e.to_string())?;
        crate::aggiungi_riga_gia_in_turno(&self.radice.join("inviati.jsonl"), &riga)?;
        Ok(n)
    }

    /// I messaggi mandati, in ordine.
    pub fn inviati(&self) -> Vec<Messaggio> {
        righe(&self.radice.join("inviati.jsonl"))
    }

    /// Le ultime [`PASSI_RILETTI`] righe del diario.
    pub fn passi(&self) -> Vec<Value> {
        let tutte: Vec<Value> = righe(&self.radice.join("diario.jsonl"));
        let da = tutte.len().saturating_sub(PASSI_RILETTI);
        tutte[da..].to_vec()
    }
}

fn righe<T: serde::de::DeserializeOwned>(p: &std::path::Path) -> Vec<T> {
    std::fs::read_to_string(p)
        .unwrap_or_default()
        .lines()
        .filter_map(|r| serde_json::from_str(r).ok())
        .collect()
}

/// I file toccati da uno o piu' Dot, uno per percorso, dal piu' recente:
/// chi l'ha toccato, se e' stato letto, scritto o tolto, quante volte e
/// l'ultima. Al massimo [`FILE_MOSTRATI`]. Due tocchi nello stesso secondo
/// si ordinano come sono arrivati: `tocchi` e' in ordine di tempo.
pub fn riassumi(tocchi: &[(String, Tocco)]) -> Vec<Value> {
    struct Voce {
        percorso: String,
        dots: Vec<String>,
        letto: bool,
        scritto: bool,
        tolto: bool,
        volte: u64,
        ultimo: String,
        dopo: usize,
    }
    let mut voci: Vec<Voce> = Vec::new();
    for (n, (dot, t)) in tocchi.iter().enumerate() {
        let i = match voci.iter().position(|v| v.percorso == t.percorso) {
            Some(i) => i,
            None => {
                voci.push(Voce {
                    percorso: t.percorso.clone(),
                    dots: Vec::new(),
                    letto: false,
                    scritto: false,
                    tolto: false,
                    volte: 0,
                    ultimo: String::new(),
                    dopo: 0,
                });
                voci.len() - 1
            }
        };
        let v = &mut voci[i];
        if !v.dots.contains(dot) {
            v.dots.push(dot.clone());
        }
        match t.come {
            Come::Letto => v.letto = true,
            Come::Scritto => v.scritto = true,
            Come::Tolto => v.tolto = true,
        }
        v.volte += 1;
        if t.quando >= v.ultimo {
            v.ultimo = t.quando.clone();
            v.dopo = n;
        }
    }
    voci.sort_by(|a, b| b.ultimo.cmp(&a.ultimo).then(b.dopo.cmp(&a.dopo)));
    voci.truncate(FILE_MOSTRATI);
    voci.into_iter()
        .map(|mut v| {
            v.dots.sort();
            json!({
                "percorso": v.percorso,
                "dots": v.dots,
                "letto": v.letto,
                "scritto": v.scritto,
                "tolto": v.tolto,
                "volte": v.volte,
                "ultimo": v.ultimo,
            })
        })
        .collect()
}

/// Chi sta in una conversazione, da un messaggio (D392): chi scrive, se non
/// e' Nova, e chi riceve. `None` per un gruppo o per un messaggio a Nova:
/// quelli hanno la loro chat. Un insieme di un Dot solo e' la chat fra lui e
/// Nova; di due o piu', la chat «fra di loro».
pub fn partecipanti(da: &str, a: &str) -> Option<Vec<String>> {
    let a = a.trim();
    if a.starts_with("gruppo:") || a == crate::DA_NOVA {
        return None;
    }
    let mut chi: Vec<String> = a
        .split(',')
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_string)
        .collect();
    let da = da.trim();
    if da != crate::DA_NOVA && !da.is_empty() {
        chi.push(da.to_string());
    }
    chi.sort();
    chi.dedup();
    (!chi.is_empty()).then_some(chi)
}

/// Quanti messaggi di una chat «fra di loro» si rileggono al massimo.
pub const FRA_RILETTI: usize = 200;

/// Le chat «fra di loro», dalla posta di tutti i Dot: `posta` e' ogni
/// messaggio ricevuto, con chi l'ha ricevuto. Un messaggio a piu' Dot sta
/// nella posta di ognuno, e conta una volta sola. Ogni chat ha i suoi
/// partecipanti e i messaggi in ordine; le chat, dalla piu' recente.
pub fn fra_di_loro(posta: &[Messaggio]) -> Vec<(Vec<String>, Vec<Messaggio>)> {
    let mut chat: Vec<(Vec<String>, Vec<Messaggio>)> = Vec::new();
    for m in posta {
        let Some(chi) = partecipanti(&m.da, &m.a) else {
            continue;
        };
        if chi.len() < 2 {
            continue;
        }
        let i = match chat.iter().position(|(c, _)| *c == chi) {
            Some(i) => i,
            None => {
                chat.push((chi, Vec::new()));
                chat.len() - 1
            }
        };
        let gia = chat[i]
            .1
            .iter()
            .any(|x| x.da == m.da && x.testo == m.testo && x.quando == m.quando && x.a == m.a);
        if !gia {
            chat[i].1.push(m.clone());
        }
    }
    for (_, ms) in chat.iter_mut() {
        ms.sort_by(|a, b| a.quando.cmp(&b.quando));
        let da = ms.len().saturating_sub(FRA_RILETTI);
        ms.drain(..da);
    }
    let ultimo = |ms: &Vec<Messaggio>| ms.last().map(|m| m.quando.clone()).unwrap_or_default();
    chat.sort_by(|a, b| ultimo(&b.1).cmp(&ultimo(&a.1)).then(a.0.cmp(&b.0)));
    chat
}

/// Il gruppo dentro cui sta un gruppo «interno» (D392): quello piu' piccolo
/// che ne contiene tutti i membri e ne ha di piu'. A parita', il primo per
/// nome. `None` se non sta dentro nessuno.
pub fn dentro(g: &crate::gruppi::Gruppo, tutti: &[crate::gruppi::Gruppo]) -> Option<String> {
    tutti
        .iter()
        .filter(|o| o.nome != g.nome && o.membri.len() > g.membri.len())
        .filter(|o| g.membri.iter().all(|m| o.membri.contains(m)))
        .min_by(|a, b| {
            a.membri
                .len()
                .cmp(&b.membri.len())
                .then(a.nome.cmp(&b.nome))
        })
        .map(|o| o.nome.clone())
}

#[cfg(test)]
mod prove {
    use super::*;

    fn base() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "nova-dot-vista-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn tocco(percorso: &str, come: Come, quando: &str) -> Tocco {
        Tocco {
            quando: quando.into(),
            compito: 1,
            percorso: percorso.into(),
            come,
            strumento: "fs.read".into(),
        }
    }

    #[test]
    fn si_sa_quali_file_tocca_una_chiamata_e_come() {
        assert_eq!(
            toccati(
                "fs.write",
                &json!({ "path": " C:\\a.txt ", "content": "x" })
            ),
            [("C:\\a.txt".to_string(), Come::Scritto)]
        );
        assert_eq!(
            toccati("fs.move", &json!({ "source": "a", "destination": "b" })),
            [
                ("b".to_string(), Come::Scritto),
                ("a".to_string(), Come::Tolto)
            ]
        );
        assert_eq!(
            toccati("fs.copy", &json!({ "source": "a", "destination": "b" })),
            [
                ("b".to_string(), Come::Scritto),
                ("a".to_string(), Come::Letto)
            ]
        );
        assert!(
            toccati("fs.read", &json!({ "path": "  " })).is_empty(),
            "un percorso vuoto non conta"
        );
        assert!(toccati("fs.read", &json!({ "path": 3 })).is_empty());
        assert!(
            toccati("shell.exec", &json!({ "command": "del a.txt" })).is_empty(),
            "non si indovina"
        );
        assert!(
            toccati("fs.list", &json!({ "path": "C:\\" })).is_empty(),
            "una cartella non e' un file"
        );
    }

    #[test]
    fn ogni_strumento_dell_elenco_compare_in_ordine_e_una_volta_per_argomento() {
        let coppie: Vec<(&str, &str)> = TOCCANO.iter().map(|(s, a, _)| (*s, *a)).collect();
        let mut ordinate = coppie.clone();
        ordinate.sort();
        ordinate.dedup();
        assert_eq!(coppie, ordinate);
    }

    #[test]
    fn i_file_toccati_si_scrivono_e_si_riassumono_dal_piu_recente() {
        let c = Cartella::di(&base(), "uno").unwrap();
        std::fs::create_dir_all(&c.radice).unwrap();
        c.tocca(&tocco("a.txt", Come::Letto, "2026-10-09T10:00:00"))
            .unwrap();
        c.tocca(&tocco("b.txt", Come::Scritto, "2026-10-09T10:01:00"))
            .unwrap();
        c.tocca(&tocco("a.txt", Come::Scritto, "2026-10-09T10:02:00"))
            .unwrap();
        std::fs::write(c.radice.join("file.jsonl"), {
            let mut t = std::fs::read_to_string(c.radice.join("file.jsonl")).unwrap();
            t.push_str("{mezza riga\n");
            t
        })
        .unwrap();
        let tocchi = c.tocchi();
        assert_eq!(tocchi.len(), 3, "la riga a meta' si salta");
        let mut tutti: Vec<(String, Tocco)> =
            tocchi.into_iter().map(|t| ("uno".into(), t)).collect();
        tutti.push((
            "due".into(),
            tocco("a.txt", Come::Tolto, "2026-10-09T09:00:00"),
        ));
        let r = riassumi(&tutti);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0]["percorso"], "a.txt", "il piu' recente prima");
        assert_eq!(r[0]["dots"], json!(["due", "uno"]));
        assert_eq!(
            (
                r[0]["letto"].as_bool(),
                r[0]["scritto"].as_bool(),
                r[0]["tolto"].as_bool()
            ),
            (Some(true), Some(true), Some(true))
        );
        assert_eq!(r[0]["volte"], 3);
        assert_eq!(r[0]["ultimo"], "2026-10-09T10:02:00");
        assert_eq!(r[1]["percorso"], "b.txt");
        assert_eq!(r[1]["letto"], false);
    }

    #[test]
    fn nello_stesso_secondo_vale_l_ordine_in_cui_sono_arrivati() {
        let t = |p: &str| {
            (
                "uno".to_string(),
                tocco(p, Come::Letto, "2026-10-09T10:00:00"),
            )
        };
        let r = riassumi(&[t("b.txt"), t("a.txt")]);
        assert_eq!(
            r[0]["percorso"], "a.txt",
            "l'ultimo arrivato prima, non l'ordine dei nomi"
        );
        let r = riassumi(&[t("a.txt"), t("b.txt")]);
        assert_eq!(r[0]["percorso"], "b.txt");
    }

    #[test]
    fn i_file_mostrati_hanno_un_tetto() {
        let tanti: Vec<(String, Tocco)> = (0..FILE_MOSTRATI + 7)
            .map(|i| {
                (
                    "uno".into(),
                    tocco(&format!("f{i:04}"), Come::Letto, &format!("{i:06}")),
                )
            })
            .collect();
        let r = riassumi(&tanti);
        assert_eq!(r.len(), FILE_MOSTRATI);
        assert_eq!(r[0]["percorso"], format!("f{:04}", FILE_MOSTRATI + 6));
    }

    #[test]
    fn i_messaggi_mandati_si_tengono_numerati() {
        let c = Cartella::di(&base(), "uno").unwrap();
        std::fs::create_dir_all(&c.radice).unwrap();
        assert_eq!(c.spedito("uno", "nova", "ciao", "t1").unwrap(), 1);
        assert_eq!(
            c.spedito("uno", "gruppo:squadra", "ci sono", "t2").unwrap(),
            2
        );
        let m = c.inviati();
        assert_eq!(m.len(), 2);
        assert_eq!(
            (m[1].a.as_str(), m[1].testo.as_str(), m[1].da.as_str()),
            ("gruppo:squadra", "ci sono", "uno")
        );
    }

    fn msg(da: &str, a: &str, testo: &str, quando: &str) -> Messaggio {
        Messaggio {
            n: 1,
            da: da.into(),
            a: a.into(),
            testo: testo.into(),
            quando: quando.into(),
        }
    }

    #[test]
    fn chi_sta_in_una_conversazione() {
        let v = |x: &[&str]| Some(x.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(partecipanti("uno", "due"), v(&["due", "uno"]));
        assert_eq!(
            partecipanti("nova", "uno"),
            v(&["uno"]),
            "con Nova: la sua chat"
        );
        assert_eq!(partecipanti("nova", "uno,due"), v(&["due", "uno"]));
        assert_eq!(partecipanti("tre", "uno,due"), v(&["due", "tre", "uno"]));
        assert_eq!(partecipanti("uno", "gruppo:squadra"), None);
        assert_eq!(partecipanti("uno", "nova"), None);
    }

    #[test]
    fn le_chat_fra_di_loro_si_ricostruiscono_dalla_posta() {
        let posta = [
            msg("uno", "due", "ciao", "t1"),
            msg("due", "uno", "eccomi", "t3"),
            // A tutti e due: sta nella posta di ognuno, conta una volta.
            msg("nova", "due,uno", "brave", "t4"),
            msg("nova", "due,uno", "brave", "t4"),
            msg("tre", "due,uno", "posso?", "t2"),
            msg("nova", "uno", "solo a te", "t5"),
            msg("uno", "gruppo:squadra", "a tutti", "t6"),
        ];
        let c = fra_di_loro(&posta);
        assert_eq!(c.len(), 2, "{c:?}");
        assert_eq!(c[0].0, ["due", "uno"], "la piu' recente prima");
        let testi: Vec<&str> = c[0].1.iter().map(|m| m.testo.as_str()).collect();
        assert_eq!(testi, ["ciao", "eccomi", "brave"]);
        assert_eq!(c[1].0, ["due", "tre", "uno"]);
    }

    #[test]
    fn un_gruppo_interno_sta_dentro_il_piu_piccolo_che_lo_contiene() {
        let g = |nome: &str, membri: &[&str]| crate::gruppi::Gruppo {
            nome: nome.into(),
            membri: membri.iter().map(|s| s.to_string()).collect(),
            nato: "t".into(),
            da: String::new(),
        };
        let tutti = [
            g("azienda", &["a", "b", "c", "d"]),
            g("squadra", &["a", "b", "c"]),
            g("coppia", &["a", "b"]),
            g("altri", &["c", "e"]),
        ];
        assert_eq!(dentro(&tutti[2], &tutti).as_deref(), Some("squadra"));
        assert_eq!(dentro(&tutti[1], &tutti).as_deref(), Some("azienda"));
        assert_eq!(dentro(&tutti[0], &tutti), None);
        assert_eq!(dentro(&tutti[3], &tutti), None, "e non sta in nessuno");
    }

    #[test]
    fn il_diario_si_rilegge_dagli_ultimi() {
        let c = Cartella::di(&base(), "uno").unwrap();
        std::fs::create_dir_all(&c.radice).unwrap();
        for i in 0..PASSI_RILETTI + 3 {
            c.diario(&json!({ "i": i })).unwrap();
        }
        let p = c.passi();
        assert_eq!(p.len(), PASSI_RILETTI);
        assert_eq!(p[0]["i"], 3);
    }
}
