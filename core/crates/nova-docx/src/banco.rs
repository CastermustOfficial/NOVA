//! Il banco: si legge un `.docx` vero e lo si modifica, e il Python dice
//! cosa e' rimasto.
//!
//! Le prove di unita' girano su XML scritto a mano, cioe' su un documento
//! che ho immaginato io. Questo gira su un `.docx` fatto da Word (o da
//! `python-docx`, che scrive le stesse parti), ed e' l'unico modo di sapere
//! se la chirurgia regge su un documento vero — che e' la domanda a cui D237
//! ha risposto una volta e che va tenuta vera.
use nova_docx::*;
use std::path::Path;

fn stringa(d: &serde_json_leggero::Valore, chiave: &str) -> String {
    d.stringa(chiave)
}

/// Un JSON minimo: qui serve leggere quattro campi e scriverne pochi, e una
/// dipendenza in piu' su un crate che non la usa per altro non si aggiunge.
mod serde_json_leggero {
    pub struct Valore(pub String);

    impl Valore {
        /// Il valore di stringa di una chiave, da una riga JSON piatta.
        pub fn stringa(&self, chiave: &str) -> String {
            let ago = format!("\"{chiave}\"");
            let Some(i) = self.0.find(&ago) else {
                return String::new();
            };
            let resto = &self.0[i + ago.len()..];
            let Some(due) = resto.find(':') else {
                return String::new();
            };
            let dopo = resto[due + 1..].trim_start();
            if !dopo.starts_with('"') {
                return String::new();
            }
            let mut fuori = String::new();
            let mut scappa = false;
            for c in dopo[1..].chars() {
                if scappa {
                    fuori.push(match c {
                        'n' => '\n',
                        't' => '\t',
                        altro => altro,
                    });
                    scappa = false;
                } else if c == '\\' {
                    scappa = true;
                } else if c == '"' {
                    break;
                } else {
                    fuori.push(c);
                }
            }
            fuori
        }
    }
}

fn protetto(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
}

fn elenco(v: &[String]) -> String {
    v.iter()
        .map(|x| format!("\"{}\"", protetto(x)))
        .collect::<Vec<_>>()
        .join(",")
}

fn main() {
    let mut tutto = String::new();
    if std::io::Read::read_to_string(&mut std::io::stdin(), &mut tutto).is_err() {
        eprintln!("non riesco a leggere stdin");
        std::process::exit(1);
    }
    for riga in tutto.lines() {
        let riga = riga.trim();
        if riga.is_empty() {
            continue;
        }
        let d = serde_json_leggero::Valore(riga.to_string());
        let tipo = stringa(&d, "tipo");
        let file = stringa(&d, "file");
        let xml = match leggi_parte(Path::new(&file), DOCUMENTO) {
            Ok(x) => x,
            Err(e) => {
                println!("{{\"errore\":\"{}\"}}", protetto(&e));
                continue;
            }
        };
        match tipo.as_str() {
            "leggi" => {
                let p = paragrafi(&xml);
                let testi: Vec<String> = p.iter().map(|d| testo_di(d.dentro(&xml))).collect();
                let stili: Vec<String> = p.iter().map(|d| stile_di(d.dentro(&xml))).collect();
                let mut righe_tabella: Vec<String> = Vec::new();
                for t in tabelle(&xml) {
                    let dentro = t.dentro(&xml);
                    for r in elementi(dentro, "w:tr") {
                        let riga_xml = r.dentro(dentro);
                        let celle: Vec<String> = elementi(riga_xml, "w:tc")
                            .iter()
                            .map(|c| testo_di(c.dentro(riga_xml)))
                            .collect();
                        righe_tabella.push(celle.join(" | "));
                    }
                }
                println!(
                    "{{\"paragrafi\":[{}],\"stili\":[{}],\"tabella\":[{}]}}",
                    elenco(&testi),
                    elenco(&stili),
                    elenco(&righe_tabella)
                );
            }
            "sostituisci" => {
                let quale: usize = stringa(&d, "quale").parse().unwrap_or(0);
                let nuovo = stringa(&d, "nuovo");
                let p = paragrafi(&xml);
                if quale >= p.len() {
                    println!("{{\"errore\":\"il paragrafo {quale} non c'e'\"}}");
                    continue;
                }
                let rifatto = riscrivi_testo(p[quale].dentro(&xml), &nuovo);
                let tutto_xml = sostituisci(&xml, p[quale], &rifatto);
                match riscrivi_parte(Path::new(&file), DOCUMENTO, &tutto_xml) {
                    Ok(quante) => println!("{{\"parti\":{quante}}}"),
                    Err(e) => println!("{{\"errore\":\"{}\"}}", protetto(&e)),
                }
            }
            altro => println!("{{\"errore\":\"non so fare «{}»\"}}", protetto(altro)),
        }
    }
}
