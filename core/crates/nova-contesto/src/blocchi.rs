//! Quello che si attacca **in coda alla domanda**, e perche' proprio li'.
//!
//! Sta in `nova-contesto` per la ragione che lo ha fatto nascere, che e' una
//! ragione di finestra e non di contenuto.
//!
//! Prima questi testi finivano nel messaggio di sistema, riscritto a ogni
//! turno. Due difetti in uno.
//!
//! **Funzionale**: i cervelli agentici il prompt di sistema lo ricevono solo
//! all'apertura della sessione, quindi dal secondo turno in poi il contesto
//! veniva calcolato e buttato. NOVA faceva la ricerca sul grafo e non la
//! leggeva.
//!
//! **Di costo**: il messaggio di sistema e' la prima regione di token su cui
//! un fornitore tiene la cache. Cambiarlo a ogni turno — e cambiava, perche'
//! il contesto dipende dalla domanda — invalida tutto il prefisso: ogni turno
//! rielaborava l'intera conversazione da capo. In coda invece si aggiunge e
//! basta, e il prefisso resta valido.
//!
//! E' la stessa aritmetica del fondo nel taglio dei messaggi: non cambia cosa
//! il modello legge, cambia quanto spesso si butta via la cache.

use crate::Messaggio;

/// Cio' che la memoria ha trovato, da mettere in coda alla domanda.
///
/// Il testo attorno non e' decorazione: dice al modello **come** leggerlo —
/// prima di misurare, senza ripeterlo all'utente come una novita', e con il
/// permesso esplicito di correggerlo. Senza quella cornice il contesto
/// diventa un blocco di fatti che il modello ripete a pappagallo.
///
/// Contesto vuoto vuol dire niente blocco: una cornice attorno al nulla
/// costa token e dice al modello che la memoria e' vuota, che e' una cosa
/// diversa dal non averla interrogata.
pub fn memoria(contesto: &str) -> String {
    if contesto.is_empty() {
        return String::new();
    }
    format!(
        "\n\n<memoria>\n\
         Quello che gia' sai, dalla tua memoria a grafo. Guardalo prima di \
         misurare o cercare, e non ripeterlo all'utente come se fosse una \
         novita'. Se scopri che qualcosa qui e' superato, correggilo con \
         kb_note o kb_forget.\n\n{contesto}\n</memoria>"
    )
}

/// Il richiamo all'identita', per i soli cervelli agentici.
///
/// Chi non e' agentico riceve il prompt di sistema intero a ogni chiamata e
/// non ha bisogno di essere richiamato all'ordine.
pub fn identita(agentico: bool) -> &'static str {
    if agentico {
        crate::testi::PROMEMORIA
    } else {
        ""
    }
}

/// Il messaggio dell'utente con attaccato tutto il resto.
///
/// **L'ordine conta**, ed e' l'unica cosa che questa funzione decide: prima
/// cio' che hai chiesto, poi cio' che NOVA sa, poi cio' che ha gia' fatto,
/// poi come deve rispondere. L'istruzione resta l'ultima cosa letta, che e'
/// il posto in cui i modelli la seguono di piu'.
///
/// La `postilla` e' un'istruzione per il cervello e basta: non entra nella
/// ricerca in memoria e non viene imparata. Serve alla voce, che a ogni turno
/// deve ricordare al cervello di rispondere come si parla — e che non puo'
/// metterlo nel prompt di sistema, visto che quello si passa solo quando la
/// sessione si apre.
pub fn domanda(
    testo: &str,
    memoria: &str,
    procedure: &str,
    identita: &str,
    postilla: &str,
) -> String {
    format!("{testo}{memoria}{procedure}{identita}{postilla}")
}

/// Separa il ragionamento dalla risposta.
///
/// Molti modelli il ragionamento lo scrivono dentro `<think>...</think>` nel
/// testo, altri in un campo a parte. Le due cose vanno tenute separate perche'
/// l'utente legge la risposta: un ragionamento che finisce nel contenuto e' il
/// modello che si contraddice a voce alta davanti a chi ha chiesto qualcosa.
///
/// **Anche un `<think>` mai chiuso.** Se il modello si interrompe a meta' del
/// ragionamento, senza il secondo taglio quel troncone finirebbe intero nella
/// risposta — ed e' il caso in cui si vede di piu', perche' non finisce con
/// una frase compiuta.
///
/// Ritorna `(contenuto, ragionamento)`, tutti e due ripuliti ai bordi.
pub fn separa_ragionamento(contenuto: &str, ragionamento_a_parte: &str) -> (String, String) {
    let (pulito, trovati) = togli_think(contenuto);
    let mut ragionamento = ragionamento_a_parte.to_string();
    if !trovati.is_empty() {
        ragionamento = format!("{ragionamento}\n{}", trovati.join("\n"))
            .trim()
            .to_string();
    }
    let pulito = togli_think_aperto(&pulito);
    (pulito.trim().to_string(), ragionamento.trim().to_string())
}

/// I `<think>...</think>` chiusi: cosa resta, e cosa c'era dentro.
///
/// Scritto a mano invece che con un'espressione regolare, come le chiamate nel
/// testo (D154): la regola e' che il tag si riconosce **senza guardare le
/// maiuscole** e che il punto prende anche gli a capo, e scriverla costringe a
/// dirlo.
fn togli_think(testo: &str) -> (String, Vec<String>) {
    const APRE: &str = "<think>";
    const CHIUDE: &str = "</think>";
    let minuscolo = testo.to_lowercase();
    let caratteri: Vec<char> = testo.chars().collect();
    let piccoli: Vec<char> = minuscolo.chars().collect();
    let apre: Vec<char> = APRE.chars().collect();
    let chiude: Vec<char> = CHIUDE.chars().collect();
    let mut fuori = String::new();
    let mut dentro: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i < caratteri.len() {
        if piccoli[i..].starts_with(&apre) {
            if let Some(fine) = trova(&piccoli, &chiude, i + apre.len()) {
                dentro.push(caratteri[i + apre.len()..fine].iter().collect());
                i = fine + chiude.len();
                continue;
            }
        }
        fuori.push(caratteri[i]);
        i += 1;
    }
    (fuori, dentro)
}

/// Un `<think>` mai chiuso porta via tutto quello che viene dopo.
fn togli_think_aperto(testo: &str) -> String {
    let minuscolo = testo.to_lowercase();
    match minuscolo.find("<think>") {
        Some(i) => testo.chars().take(minuscolo[..i].chars().count()).collect(),
        None => testo.to_string(),
    }
}

fn trova(dove: &[char], cosa: &[char], da: usize) -> Option<usize> {
    if cosa.is_empty() || dove.len() < cosa.len() || da > dove.len() - cosa.len() {
        return None;
    }
    (da..=dove.len() - cosa.len()).find(|&i| &dove[i..i + cosa.len()] == cosa)
}

/// Un solo messaggio di sistema, in testa.
///
/// Molti template di chat ne pretendono uno solo: mandarne due vuol dire che
/// il secondo viene ignorato, o peggio che il template si rompe e il modello
/// riceve una conversazione senza istruzioni.
pub fn un_solo_sistema(messaggi: &[Messaggio]) -> Vec<Messaggio> {
    let sistema: Vec<&Messaggio> = messaggi.iter().filter(|m| m.ruolo == "system").collect();
    let resto: Vec<Messaggio> = messaggi
        .iter()
        .filter(|m| m.ruolo != "system")
        .cloned()
        .collect();
    if sistema.is_empty() {
        return resto;
    }
    let testa = Messaggio::nuovo(
        "system",
        sistema
            .iter()
            .map(|m| m.contenuto.as_str())
            .collect::<Vec<&str>>()
            .join("\n\n")
            .trim(),
    );
    let mut fuori = vec![testa];
    fuori.extend(resto);
    fuori
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn niente_memoria_niente_cornice() {
        assert_eq!(memoria(""), "");
    }

    #[test]
    fn la_cornice_dice_come_leggerlo() {
        let b = memoria("gio usa Rust");
        assert!(b.starts_with("\n\n<memoria>\n"));
        assert!(b.ends_with("\n</memoria>"));
        assert!(b.contains("gio usa Rust"));
        assert!(b.contains("kb_forget"), "manca il permesso di correggere");
    }

    #[test]
    fn il_ragionamento_non_finisce_nella_risposta() {
        let (c, r) = separa_ragionamento("<think>ci penso</think>Ecco la risposta.", "");
        assert_eq!(c, "Ecco la risposta.");
        assert_eq!(r, "ci penso");
    }

    #[test]
    fn il_tag_si_riconosce_anche_in_maiuscolo_e_su_piu_righe() {
        let (c, r) = separa_ragionamento("<THINK>a\ncapo</Think> risposta", "");
        assert_eq!(c, "risposta");
        assert_eq!(r, "a\ncapo");
    }

    #[test]
    fn un_think_mai_chiuso_porta_via_tutto_quello_che_segue() {
        // E' il caso che si vede di piu': il modello si interrompe a meta'
        // del ragionamento e senza questo taglio il troncone finirebbe nella
        // risposta, senza nemmeno una frase compiuta in fondo.
        let (c, r) = separa_ragionamento("Ecco. <think>sto ancora pensando e poi", "");
        assert_eq!(c, "Ecco.");
        assert_eq!(r, "");
    }

    #[test]
    fn il_campo_a_parte_e_i_tag_si_sommano() {
        let (_, r) = separa_ragionamento("<think>due</think>x", "uno");
        assert_eq!(r, "uno\ndue");
    }

    #[test]
    fn senza_ragionamento_non_si_tocca_niente() {
        let (c, r) = separa_ragionamento("  risposta secca  ", "");
        assert_eq!(c, "risposta secca");
        assert_eq!(r, "");
    }

    #[test]
    fn i_messaggi_di_sistema_diventano_uno() {
        let m = vec![
            Messaggio::nuovo("system", "primo"),
            Messaggio::nuovo("user", "ciao"),
            Messaggio::nuovo("system", "secondo"),
        ];
        let f = un_solo_sistema(&m);
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].ruolo, "system");
        assert_eq!(f[0].contenuto, "primo\n\nsecondo");
        assert_eq!(f[1].ruolo, "user");
    }

    #[test]
    fn senza_sistema_resta_tutto_com_era() {
        let m = vec![Messaggio::nuovo("user", "ciao")];
        assert_eq!(un_solo_sistema(&m), m);
    }

    #[test]
    fn lordine_e_quello_e_non_un_altro() {
        assert_eq!(domanda("chiesto", "-sa", "-fatto", "-chi", "-come"),
                   "chiesto-sa-fatto-chi-come");
        // Senza niente attorno resta esattamente la domanda: nessuna riga
        // aggiunta, nessuno spazio.
        assert_eq!(domanda("chiesto", "", "", "", ""), "chiesto");
    }

    #[test]
    fn lidentita_solo_a_chi_serve() {
        assert_eq!(identita(false), "");
        assert!(identita(true).contains("<sei_nova>"));
    }
}
