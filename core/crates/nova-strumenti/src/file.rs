//! Come si racconta un file al modello.
//!
//! Il disco sta fuori: qui c'e' solo la parte che decide **cosa legge il
//! modello** — l'ordine di un elenco, la misura di un file, quante righe
//! entrano, come si scrive una riga trovata. E' la parte che sembra
//! cosmetica e non lo e': quel testo e' cio' su cui il modello decide il
//! passo dopo, e una misura scritta in un altro modo o un troncamento a un
//! carattere diverso sono un contesto diverso.

/// Quanti elementi di una cartella si elencano prima di dire «e altri N».
pub const MAX_ELEMENTI: usize = 300;

/// Quanti caratteri di un file entrano in una lettura.
pub const MAX_CARATTERI_LETTI: usize = 40000;

/// Quanto puo' essere lunga una riga trovata da una ricerca nel testo.
pub const MAX_RIGA_TROVATA: usize = 200;

/// La misura di un file come la legge una persona.
///
/// Si sale di unita' solo finche' si divide: sotto i 1024 byte restano byte,
/// e non si scrive «0 KB» per un file di dieci righe.
pub fn misura(byte: u64) -> String {
    let mut quanto = byte as f64;
    let mut unita = "B";
    for u in ["KB", "MB", "GB"] {
        if quanto >= 1024.0 {
            quanto /= 1024.0;
            unita = u;
        } else {
            break;
        }
    }
    format!("{}", arrotonda(quanto)) + " " + unita
}

/// Arrotonda all'intero **come Python**, cioe' a meta' verso il pari.
///
/// `format!("{:.0}", x)` in Rust e `f"{x:.0f}"` in Python arrotondano tutti e
/// due al pari, ma la strada e' diversa e su qualche valore i due si
/// separano; qui l'arrotondamento e' esplicito, cosi' non dipende da come
/// due librerie diverse formattano.
fn arrotonda(x: f64) -> i64 {
    let giu = x.floor();
    let resto = x - giu;
    let giu = giu as i64;
    if resto > 0.5 {
        giu + 1
    } else if resto < 0.5 {
        giu
    } else if giu % 2 == 0 {
        giu
    } else {
        giu + 1
    }
}

/// La riga che descrive una cartella dentro un elenco.
pub fn riga_cartella(nome: &str, quando: &str) -> String {
    format!("[DIR ] {nome}/  ({quando})")
}

/// La riga che descrive un file dentro un elenco.
pub fn riga_file(nome: &str, byte: u64, quando: &str) -> String {
    format!("[FILE] {nome}  {}  ({quando})", misura(byte))
}

/// La riga di un elemento che non si riesce a guardare.
///
/// Non si salta: una cartella che elenca nove file su dieci senza dire
/// niente e' peggio di una che dice «questo non l'ho potuto guardare».
pub fn riga_illeggibile(nome: &str, perche: &str) -> String {
    format!("[????] {nome}  (non accessibile: {perche})")
}

/// La chiave con cui si ordina un elenco: prima le cartelle, poi per nome
/// senza distinguere le maiuscole.
pub fn chiave_ordine(nome: &str, e_cartella: bool) -> (bool, String) {
    (!e_cartella, nome.to_lowercase())
}

/// Quale fetta di righe si legge, e come si dice quale si e' letta.
///
/// `offset` conta da uno, e uno zero vale come uno: il modello scrive
/// `offset: 0` piu' spesso di quanto si creda, e rispondergli con un errore
/// invece che con la prima riga e' pignoleria che costa un giro.
pub fn fetta(quante_righe: usize, offset: i64, limite: i64) -> (usize, usize) {
    let inizio = (offset.max(1) - 1).max(0) as usize;
    let fine = if limite > 0 {
        inizio + limite as usize
    } else {
        quante_righe
    };
    (inizio, fine)
}

/// L'intestazione della lettura: dove si e' letto e quanto.
pub fn intestazione(percorso: &str, inizio: usize, fine: usize, quante: usize) -> String {
    format!("{percorso} (righe {}-{} di {quante}):", inizio + 1, fine.min(quante))
}

/// Il testo letto, tagliato se non ci sta.
pub fn taglia(testo: &str, quante_righe: usize) -> String {
    if testo.chars().count() <= MAX_CARATTERI_LETTI {
        return testo.to_string();
    }
    let corto: String = testo.chars().take(MAX_CARATTERI_LETTI).collect();
    format!("{corto}\n... [troncato, file di {quante_righe} righe]")
}

/// Il modello di ricerca, allargato a cio' che intendeva chi l'ha scritto.
///
/// Chi cerca «fattura» intende «i file che si chiamano qualcosa-fattura», non
/// «il file chiamato esattamente fattura»; e chi scrive `*.pdf` intende
/// dappertutto, non solo qui. Le due regole non si sovrappongono: la seconda
/// vale solo per chi un carattere jolly ce l'ha gia' messo.
pub fn modello_di_ricerca(dato: &str) -> String {
    if !dato.contains('*') && !dato.contains('?') {
        format!("**/*{dato}*")
    } else if !dato.starts_with("**") {
        format!("**/{dato}")
    } else {
        dato.to_string()
    }
}

/// La riga di una ricerca nel testo: dove, e cosa c'era scritto.
pub fn riga_trovata(percorso: &str, numero: usize, riga: &str) -> String {
    let corta: String = riga.trim().chars().take(MAX_RIGA_TROVATA).collect();
    format!("{percorso}:{numero}: {corta}")
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn le_misure_si_leggono_come_le_direbbe_una_persona() {
        assert_eq!(misura(0), "0 B");
        assert_eq!(misura(500), "500 B");
        assert_eq!(misura(1023), "1023 B");
        assert_eq!(misura(1024), "1 KB");
        assert_eq!(misura(1536), "2 KB", "1,5 KB arrotonda al pari");
        assert_eq!(misura(2048), "2 KB");
        assert_eq!(misura(1024 * 1024), "1 MB");
        assert_eq!(misura(1024 * 1024 * 1024), "1 GB");
        // Oltre il gigabyte non si sale: si dicono tanti GB, come il Python.
        assert!(misura(3 * 1024 * 1024 * 1024 * 1024).ends_with(" GB"));
    }

    #[test]
    fn un_file_piccolo_non_diventa_zero_kilobyte() {
        // Il difetto che la scala evita: dividere sempre farebbe scrivere
        // «0 KB» per un file di dieci righe, e chi legge crede sia vuoto.
        assert_eq!(misura(10), "10 B");
    }

    #[test]
    fn le_cartelle_vengono_prima_e_poi_conta_il_nome() {
        let mut v = vec![
            chiave_ordine("zeta.txt", false),
            chiave_ordine("Alfa", true),
            chiave_ordine("beta.txt", false),
            chiave_ordine("Zulu", true),
        ];
        v.sort();
        let nomi: Vec<&str> = v.iter().map(|(_, n)| n.as_str()).collect();
        assert_eq!(nomi, vec!["alfa", "zulu", "beta.txt", "zeta.txt"]);
    }

    #[test]
    fn la_fetta_di_righe_parte_da_uno_e_lo_zero_vale_uno() {
        assert_eq!(fetta(100, 1, 0), (0, 100));
        assert_eq!(fetta(100, 0, 0), (0, 100), "uno zero non e' un errore");
        assert_eq!(fetta(100, -5, 0), (0, 100));
        assert_eq!(fetta(100, 10, 5), (9, 14));
        assert_eq!(
            fetta(3, 1, 999),
            (0, 999),
            "il limite non si accorcia qui: a tagliarlo\nci pensa chi ha le righe in mano"
        );
    }

    #[test]
    fn lintestazione_non_promette_righe_che_non_ci_sono() {
        assert_eq!(intestazione("x.txt", 0, 1000, 3), "x.txt (righe 1-3 di 3):");
        assert_eq!(intestazione("x.txt", 9, 14, 100), "x.txt (righe 10-14 di 100):");
    }

    #[test]
    fn un_file_lungo_si_taglia_e_lo_dice() {
        let corto = "riga\n".repeat(3);
        assert_eq!(taglia(&corto, 3), corto);
        let lungo = "x".repeat(MAX_CARATTERI_LETTI + 10);
        let t = taglia(&lungo, 7);
        assert!(t.contains("[troncato, file di 7 righe]"));
        assert!(t.starts_with(&"x".repeat(100)));
    }

    #[test]
    fn chi_cerca_una_parola_intende_cercarla_dappertutto() {
        assert_eq!(modello_di_ricerca("fattura"), "**/*fattura*");
        assert_eq!(modello_di_ricerca("*.pdf"), "**/*.pdf");
        assert_eq!(modello_di_ricerca("**/*.pdf"), "**/*.pdf");
        assert_eq!(modello_di_ricerca("nota?.txt"), "**/nota?.txt");
    }

    #[test]
    fn una_riga_trovata_si_pota_ai_bordi_e_in_fondo() {
        assert_eq!(riga_trovata("a.py", 3, "   ciao   "), "a.py:3: ciao");
        let lunga = "y".repeat(500);
        let r = riga_trovata("a.py", 1, &lunga);
        assert_eq!(r.chars().count(), "a.py:1: ".chars().count() + MAX_RIGA_TROVATA);
    }
}
