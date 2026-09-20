//! NOVA scrive dentro il documento, ma non di nascosto.
//!
//! Fin qui l'harness sapeva leggere e indicare, non toccare. Toccare pero' e'
//! l'azione che non si annulla da se', quindi qui non esiste una funzione che
//! modifichi e basta: esiste una **proposta**, che non tocca niente, e
//! un'**applicazione**, che si chiede dopo aver visto cosa cambia. Una
//! proposta si puo' guardare, discutere e buttare senza conseguenze, e chi
//! applica sa sempre cosa sta applicando.
//!
//! **Lo stesso codice fa l'anteprima e l'applicazione**, e non per pigrizia:
//! un'anteprima calcolata a parte prima o poi mostra qualcosa di diverso da
//! quel che poi succede, ed e' il modo piu' sicuro di far perdere fiducia a
//! chi deve premere il bottone. Le marche — una per cio' che arriva, una per
//! cio' che se ne va — si attaccano in coda alle righe: il testo e' lo
//! stesso, e chi lo disegna sa cosa colorare.

use crate::{schiaccia, Blocco};

/// Cosa si puo' chiedere di fare a un blocco.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Azione {
    Sostituisci,
    Prima,
    Dopo,
    Elimina,
    Evidenzia,
    Nota,
}

/// Le azioni che si fanno su un documento di testo.
pub const SU_UN_TESTO: [&str; 4] = ["dopo", "elimina", "prima", "sostituisci"];

/// Le azioni che si fanno su un PDF.
///
/// Il testo di un PDF non si riscrive: un PDF non contiene paragrafi,
/// contiene lettere messe in un punto della pagina, e cambiarne una vuol dire
/// ridisegnare quel che c'e' intorno — con un risultato che si vede. Si
/// annota pero' per davvero, e l'annotazione resta nel file.
pub const SU_UN_PDF: [&str; 2] = ["evidenzia", "nota"];

/// Le azioni che non prendono un testo: sono gesti, non parole. Chiedere il
/// testo di un «elimina» sarebbe una domanda senza risposta possibile.
pub const SENZA_TESTO: [&str; 2] = ["elimina", "evidenzia"];

/// Quanto si mostra di un testo nell'anteprima.
pub const ESTRATTO: usize = 220;

impl Azione {
    pub fn da(nome: &str) -> Option<Azione> {
        match nome.trim().to_lowercase().as_str() {
            "sostituisci" => Some(Azione::Sostituisci),
            "prima" => Some(Azione::Prima),
            "dopo" => Some(Azione::Dopo),
            "elimina" => Some(Azione::Elimina),
            "evidenzia" => Some(Azione::Evidenzia),
            "nota" => Some(Azione::Nota),
            _ => None,
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            Azione::Sostituisci => "sostituisci",
            Azione::Prima => "prima",
            Azione::Dopo => "dopo",
            Azione::Elimina => "elimina",
            Azione::Evidenzia => "evidenzia",
            Azione::Nota => "nota",
        }
    }

    pub fn vuole_un_testo(self) -> bool {
        !SENZA_TESTO.contains(&self.nome())
    }
}

/// Quali azioni sono lecite su questo file.
pub fn lecite_per(estensione: &str) -> &'static [&'static str] {
    if estensione == ".pdf" {
        &SU_UN_PDF
    } else {
        &SU_UN_TESTO
    }
}

/// Se dentro questo file si sa riscrivere riga per riga.
///
/// Il codice sta qui perche' e' testo a righe come l'HTML: fin qui l'harness
/// sapeva *proporre* una modifica a un `.py` e non sapeva applicarla, e lo
/// diceva solo al momento del bottone.
pub fn si_riscrive(estensione: &str) -> bool {
    matches!(estensione, ".md" | ".markdown" | ".txt")
        || crate::CODICE.contains(&estensione)
        || crate::A_RIGHE_IN_PIU.contains(&estensione)
}

/// Una modifica chiesta, prima di essere controllata.
#[derive(Debug, Clone)]
pub struct Chiesta {
    pub azione: String,
    pub blocco: String,
    pub testo: String,
}

/// Una modifica controllata, con dentro cosa c'era.
#[derive(Debug, Clone, PartialEq)]
pub struct Pronta {
    pub azione: Azione,
    pub blocco: String,
    pub testo: String,
    /// Cosa c'era, quando la proposta e' stata fatta. Non e' per il racconto:
    /// e' cio' che permette di accorgersi che il file e' cambiato sotto.
    pub prima: String,
    pub righe: Option<u32>,
    pub pagina: Option<u32>,
}

/// Controlla le modifiche chieste contro il documento aperto.
///
/// Torna le modifiche pronte **oppure** tutti i motivi per cui non lo sono.
/// Tutti, non il primo: chi propone tre modifiche e ne sbaglia due deve
/// sapere quali due, o le scopre una alla volta.
pub fn controlla(
    chieste: &[Chiesta],
    blocchi: &[Blocco],
    estensione: &str,
) -> Result<Vec<Pronta>, Vec<String>> {
    if chieste.is_empty() {
        return Err(vec!["nessuna modifica da proporre".to_string()]);
    }
    let lecite = lecite_per(estensione);
    let mut pronte = Vec::new();
    let mut guai = Vec::new();
    for (n, c) in chieste.iter().enumerate() {
        let quale = n + 1;
        let nome = if c.azione.trim().is_empty() {
            "sostituisci"
        } else {
            c.azione.trim()
        };
        let azione = Azione::da(nome).filter(|a| lecite.contains(&a.nome()));
        let Some(azione) = azione else {
            guai.push(format!(
                "modifica {quale}: su un {estensione} si puo' fare {}, non «{}»",
                lecite.join(", "),
                nome.to_lowercase()
            ));
            continue;
        };
        let blocco = c.blocco.trim();
        let Some(b) = blocchi.iter().find(|b| b.id == blocco) else {
            guai.push(format!(
                "modifica {quale}: il blocco «{blocco}» non esiste in questo documento"
            ));
            continue;
        };
        if azione.vuole_un_testo() && c.testo.trim().is_empty() {
            guai.push(format!("modifica {quale}: manca il testo"));
            continue;
        }
        if estensione == ".docx"
            && matches!(azione, Azione::Prima | Azione::Dopo)
            && blocco.starts_with('t')
        {
            guai.push(format!(
                "modifica {quale}: dentro una tabella si sostituisce la riga, \
                 non se ne aggiungono"
            ));
            continue;
        }
        pronte.push(Pronta {
            azione,
            blocco: blocco.to_string(),
            testo: c.testo.clone(),
            prima: b.testo.clone(),
            righe: b.righe,
            pagina: b.pagina,
        });
    }
    if !guai.is_empty() {
        return Err(guai);
    }
    Ok(pronte)
}

/// Un testo accorciato per l'anteprima, con i puntini se e' stato accorciato.
pub fn corta(testo: &str, quanto: usize) -> String {
    let piatto = schiaccia(testo);
    if piatto.chars().count() <= quanto {
        return piatto;
    }
    let preso: String = piatto.chars().take(quanto.saturating_sub(1)).collect();
    format!("{preso}…")
}

/// Da quale riga comincia questo blocco. Solo i blocchi a righe ne hanno una.
pub fn inizio(blocco: &str) -> Option<usize> {
    blocco.strip_prefix('r')?.parse().ok()
}

/// Perche' una modifica non si e' potuta fare.
#[derive(Debug, Clone, PartialEq)]
pub struct Saltata {
    pub blocco: String,
    pub perche: String,
}

/// Le marche che si attaccano in coda alle righe, per l'anteprima.
#[derive(Debug, Clone, Default)]
pub struct Marche {
    pub nuovo: String,
    pub vecchio: String,
}

impl Marche {
    fn anteprima(&self) -> bool {
        !self.nuovo.is_empty() || !self.vecchio.is_empty()
    }
}

/// Dove finisce il blocco che comincia a `i`.
fn fine_del_blocco(righe: &[String], i: usize, quante: Option<u32>) -> usize {
    match quante {
        // Quante righe occupa il blocco lo sa il blocco.
        Some(q) if q > 0 => righe.len().min(i + q as usize),
        // Senza, si arriva fino alla riga vuota: e' un documento a
        // paragrafi. Nel codice il blocco e' una riga sola, e fermarsi alla
        // riga vuota si mangerebbe mezzo file.
        _ => {
            let mut fine = i;
            while fine < righe.len() && !righe[fine].trim().is_empty() {
                fine += 1;
            }
            fine
        }
    }
}

/// Se quel che c'e' adesso e' ancora quel che c'era quando si e' proposto.
///
/// **Questa e' la cosa che mancava.** La proposta si controlla contro i
/// blocchi letti quando il documento e' stato aperto, e si applica anche
/// mezz'ora dopo. Se in mezzo qualcuno ha toccato il file — l'utente, un
/// altro programma, un `git pull` — le righe a quel numero vogliono dire
/// un'altra cosa, e NOVA ci scriveva sopra **senza dire niente**. Qui il
/// confronto e' sul contenuto, non sulla data: una data cambia anche quando
/// il testo e' lo stesso, e il testo e' quel che conta (D273).
pub fn e_ancora_quello(righe: &[String], i: usize, fine: usize, prima: &str) -> bool {
    let adesso = schiaccia(&righe[i..fine].join(" "));
    adesso == schiaccia(prima)
}

/// Le righe come saranno. Con le marche, come si vedono prima.
///
/// Si lavora dal fondo verso l'alto: cosi' gli indici delle modifiche ancora
/// da fare restano quelli calcolati all'apertura.
pub fn rifai(
    righe: &[String],
    modifiche: &[Pronta],
    marche: &Marche,
) -> (Vec<String>, usize, Vec<Saltata>) {
    let mut righe: Vec<String> = righe.to_vec();
    let mut ordinate: Vec<&Pronta> = modifiche.iter().collect();
    // Ordinamento stabile: due modifiche sullo stesso blocco restano
    // nell'ordine in cui sono state chieste.
    ordinate.sort_by_key(|m| std::cmp::Reverse(inizio(&m.blocco).unwrap_or(0)));
    let anteprima = marche.anteprima();
    let mut fatte = 0usize;
    let mut saltate = Vec::new();
    for m in ordinate {
        let Some(i) = inizio(&m.blocco) else {
            saltate.push(Saltata {
                blocco: m.blocco.clone(),
                perche: format!("«{}» non e' un punto di un file a righe", m.blocco),
            });
            continue;
        };
        if i >= righe.len() {
            saltate.push(Saltata {
                blocco: m.blocco.clone(),
                perche: format!(
                    "il file adesso ha {} righe e «{}» non c'e' piu'",
                    righe.len(),
                    m.blocco
                ),
            });
            continue;
        }
        let fine = fine_del_blocco(&righe, i, m.righe);
        if !e_ancora_quello(&righe, i, fine, &m.prima) {
            saltate.push(Saltata {
                blocco: m.blocco.clone(),
                perche: format!(
                    "in «{}» adesso c'e' scritto un'altra cosa: il file e' \
                     cambiato da quando ho proposto, e non ci scrivo sopra",
                    m.blocco
                ),
            });
            continue;
        }
        let nuove: Vec<String> = if m.testo.is_empty() {
            Vec::new()
        } else {
            m.testo
                .lines()
                .map(|r| format!("{r}{}", marche.nuovo))
                .collect()
        };
        let vecchie: Vec<String> = righe[i..fine]
            .iter()
            .map(|r| format!("{r}{}", marche.vecchio))
            .collect();
        match m.azione {
            Azione::Sostituisci => {
                let mut dentro = if anteprima {
                    let mut v = vecchie.clone();
                    v.push(String::new());
                    v
                } else {
                    Vec::new()
                };
                dentro.extend(nuove);
                righe.splice(i..fine, dentro);
            }
            Azione::Elimina => {
                // La riga vuota dopo il blocco se ne va con lui, ma solo se
                // e' davvero vuota: nel codice, la riga dopo e' altro codice.
                let coda = if fine < righe.len() && righe[fine].trim().is_empty() {
                    fine + 1
                } else {
                    fine
                };
                let dentro: Vec<String> = if anteprima {
                    let mut v = vecchie.clone();
                    v.push(String::new());
                    v
                } else {
                    Vec::new()
                };
                righe.splice(i..coda, dentro);
            }
            Azione::Prima => {
                let mut dentro = nuove;
                dentro.push(String::new());
                righe.splice(i..i, dentro);
            }
            Azione::Dopo => {
                let mut dentro = vec![String::new()];
                dentro.extend(nuove);
                righe.splice(fine..fine, dentro);
            }
            // Evidenzia e Nota non toccano righe: sono del PDF.
            Azione::Evidenzia | Azione::Nota => {
                saltate.push(Saltata {
                    blocco: m.blocco.clone(),
                    perche: format!("«{}» non si fa dentro un file a righe", m.azione.nome()),
                });
                continue;
            }
        }
        fatte += 1;
    }
    (righe, fatte, saltate)
}

/// Il documento come sarebbe, con le marche su cio' che cambia.
pub fn anteprima(contenuto: &str, modifiche: &[Pronta], marche: &Marche) -> String {
    let righe: Vec<String> = contenuto.lines().map(str::to_string).collect();
    let (fuori, _, _) = rifai(&righe, modifiche, marche);
    fuori.join("\n")
}

/// Il documento come sara' davvero, e cosa non si e' potuto fare.
///
/// Il file finisce sempre con un a capo: un file di testo senza l'a capo in
/// fondo fa litigare meta' degli strumenti che lo leggono.
pub fn applicato(contenuto: &str, modifiche: &[Pronta]) -> (String, usize, Vec<Saltata>) {
    let righe: Vec<String> = contenuto.lines().map(str::to_string).collect();
    let (fuori, fatte, saltate) = rifai(&righe, modifiche, &Marche::default());
    let mut testo = fuori.join("\n");
    if !testo.ends_with('\n') {
        testo.push('\n');
    }
    (testo, fatte, saltate)
}

#[cfg(test)]
mod prove;
