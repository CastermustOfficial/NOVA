//! Dove vive un nodo: in che cartella, e sotto che nome.
//!
//! Il disco non c'e'. Qui si decide il **percorso relativo** e lo slug; a
//! creare la cartella e a scriverci dentro pensa chi ha il disco, che e' la
//! parte da scrivere contro il trait di `nova-platform`. La divisione e' la
//! stessa di `nova-memoria`, dove i nodi arrivano gia' letti.
//!
//! Le cartelle numerate non sono un vezzo: il vault si apre in Obsidian, e
//! nell'albero a sinistra un utente vede `01-profilo`, `02-persone`,
//! `03-progetti`… in quell'ordine. E' l'unica parte di NOVA la cui interfaccia
//! e' un file manager.

use std::collections::HashMap;

use crate::fusione::tipi_compatibili;
use crate::slug;

/// A ogni tipo la sua cartella. Chi non e' in tabella finisce fra i fatti, che
/// e' il contenitore generico: meglio un nodo nel posto banale che un nodo in
/// una cartella inventata sul momento.
pub fn sottocartella(tipo: &str) -> &'static str {
    match tipo {
        "profilo" | "preferenza" => "01-profilo",
        "persona" => "02-persone",
        "progetto" => "03-progetti",
        "app" | "luogo" => "04-ambiente",
        "abitudine" => "05-abitudini",
        "fatto" | "nota" => "06-fatti",
        // L'hub sta in cima, fuori da tutte le cartelle: e' la porta del
        // vault, ed e' la prima cosa che si vede aprendolo.
        "hub" => "",
        _ => "06-fatti",
    }
}

/// Il percorso del file, relativo alla radice del vault.
///
/// Torna i pezzi separati invece di una stringa con le barre: le barre le
/// mette chi conosce il sistema operativo, e sono l'unica cosa che cambia fra
/// Windows e il resto.
pub fn percorso_relativo(tipo: &str, slug: &str) -> Vec<String> {
    let sotto = sottocartella(tipo);
    let file = format!("{slug}.md");
    if sotto.is_empty() {
        vec![file]
    } else {
        vec![sotto.to_string(), file]
    }
}

/// Uno slug che non calpesti un nodo di tipo incompatibile.
///
/// Il nome porta il tipo davanti — `persona-anna`, non `anna` — e la ragione
/// e' che due cose diverse possono chiamarsi uguale: la **persona** Anna e il
/// **progetto** Anna sono due nodi, e senza prefisso il secondo scriverebbe
/// sopra il primo.
///
/// Se anche col prefisso il posto e' occupato da qualcosa di **incompatibile**
/// si aggiunge un numero. Incompatibile e non «diverso»: un «fatto» e una
/// «persona» convivono benissimo nello stesso nodo — il fatto confluisce nella
/// persona — ed e' proprio quello che deve succedere quando NOVA impara
/// qualcosa di nuovo su Anna. Se qui si scrivesse `!=` invece di
/// `tipi_compatibili`, ogni annotazione automatica creerebbe `persona-anna-2`,
/// `persona-anna-3`, e la memoria si sbriciolerebbe in copie che non si
/// parlano.
pub fn slug_libero(tipo: &str, slug_di_partenza: &str, esistenti: &HashMap<String, String>) -> String {
    let base = if tipo.is_empty() {
        slug_di_partenza.to_string()
    } else {
        slug(&format!("{tipo}-{slug_di_partenza}"))
    };
    let occupato = |c: &str| {
        esistenti
            .get(c)
            .map(|altro| !tipi_compatibili(tipo, altro))
            .unwrap_or(false)
    };
    if !occupato(&base) {
        return base;
    }
    let mut n = 2;
    loop {
        let candidato = format!("{base}-{n}");
        if !occupato(&candidato) {
            return candidato;
        }
        n += 1;
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    fn vault(coppie: &[(&str, &str)]) -> HashMap<String, String> {
        coppie.iter().map(|(s, t)| (s.to_string(), t.to_string())).collect()
    }

    #[test]
    fn ogni_tipo_ha_la_sua_cartella_e_lignoto_finisce_fra_i_fatti() {
        assert_eq!(sottocartella("persona"), "02-persone");
        assert_eq!(sottocartella("preferenza"), "01-profilo");
        assert_eq!(sottocartella("luogo"), "04-ambiente");
        assert_eq!(sottocartella("qualcosa-di-nuovo"), "06-fatti");
        assert_eq!(sottocartella(""), "06-fatti");
    }

    #[test]
    fn lhub_sta_in_cima_fuori_dalle_cartelle() {
        assert_eq!(sottocartella("hub"), "");
        assert_eq!(percorso_relativo("hub", "indice"), vec!["indice.md"]);
    }

    #[test]
    fn il_percorso_e_a_pezzi_non_una_stringa_con_le_barre() {
        assert_eq!(
            percorso_relativo("persona", "persona-anna"),
            vec!["02-persone", "persona-anna.md"]
        );
    }

    #[test]
    fn il_tipo_va_davanti_al_nome() {
        let v = vault(&[]);
        assert_eq!(slug_libero("persona", "anna", &v), "persona-anna");
        assert_eq!(slug_libero("", "anna", &v), "anna");
    }

    #[test]
    fn un_fatto_confluisce_nella_persona_invece_di_sdoppiarla() {
        // Il caso che conta: NOVA impara qualcosa su Anna. Se qui si
        // numerasse, ogni annotazione automatica creerebbe un nodo nuovo e la
        // memoria si sbriciolerebbe in copie che non si parlano.
        let v = vault(&[("persona-anna", "persona")]);
        assert_eq!(slug_libero("fatto", "persona-anna", &v), "fatto-persona-anna");
        // e lo stesso nome con lo stesso tipo resta lo stesso nodo
        assert_eq!(slug_libero("persona", "anna", &v), "persona-anna");
    }

    #[test]
    fn due_cose_incompatibili_con_lo_stesso_nome_non_si_calpestano() {
        let v = vault(&[("persona-anna", "persona")]);
        // Un progetto che si chiama Anna: `slug` normalizza a persona-anna?
        // No — il prefisso e' il suo, «progetto-anna», e non c'e' collisione.
        assert_eq!(slug_libero("progetto", "anna", &v), "progetto-anna");
    }

    #[test]
    fn quando_il_posto_e_occupato_davvero_si_numera() {
        let v = vault(&[("persona-anna", "progetto")]);
        assert_eq!(slug_libero("persona", "anna", &v), "persona-anna-2");
        let v2 = vault(&[("persona-anna", "progetto"), ("persona-anna-2", "app")]);
        assert_eq!(slug_libero("persona", "anna", &v2), "persona-anna-3");
    }

    #[test]
    fn il_nome_passa_comunque_dal_filtro_degli_accenti() {
        let v = vault(&[]);
        assert_eq!(slug_libero("persona", "Niccolò", &v), "persona-niccolo");
    }
}
