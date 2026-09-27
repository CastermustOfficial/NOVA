//! Le automazioni: quello che NOVA ha imparato, diventato uno strumento vero.
//!
//! Il **meccanismo** e' in Rust, gli **script** restano in Python (deciso con
//! Gio il 26 settembre, D346): Python sul PC e' la lingua in cui NOVA
//! scrive, non qualcosa da cui dipende per funzionare. Senza Python NOVA va,
//! e le automazioni dicono che manca.
//!
//! Gemello di `nova/automazioni.py`, file per file: la stessa cartella, lo
//! stesso guscio intorno al corpo che scrive il modello, lo stesso manifesto
//! accanto. Un'automazione creata da una meta' si esegue dall'altra.
//!
//! Le regole che vengono dal Python e restano: si **collauda prima di
//! salvare** (se la prova non gira, l'automazione non nasce); ogni
//! esecuzione gira in un processo a parte con un tetto di tempo; nessun
//! filtro sul contenuto del codice — le difese sono il rischio dichiarato,
//! il codice che resta leggibile, e il processo che si puo' fermare.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Map, Value};

/// Quanto puo' durare un'automazione prima che la si consideri piantata.
pub const ATTESA_AUTOMAZIONE_S: u64 = 120;

/// Il guscio, byte per byte quello del Python: il modello scrive solo il
/// corpo di `esegui`, e il contratto — parametri in, testo fuori, errori
/// riportati — lo garantisce questo testo, che non cambia.
const GUSCIO: &str = r#"# -*- coding: utf-8 -*-
"""@@TITOLO@@

@@DESCRIZIONE@@

Scritta da NOVA il @@QUANDO@@. Il corpo di `esegui` e' generato; tutto il resto
e' il guscio standard delle automazioni (nova/automazioni.py).
"""
import io
import json
import sys

sys.path.insert(0, r"@@RADICE@@")


def esegui(@@FIRMA@@):
@@CORPO@@


if __name__ == "__main__":
    try:
        grezzo = sys.stdin.read()
        parametri = json.loads(grezzo) if grezzo.strip() else {}
    except ValueError as e:
        print(json.dumps({"ok": False, "errore": f"parametri illeggibili: {e}"}))
        raise SystemExit(2)
    try:
        esito = esegui(**parametri)
        print(json.dumps({"ok": True, "risultato": str(esito)}, ensure_ascii=False))
    except Exception as e:
        import traceback
        print(json.dumps({"ok": False, "errore": f"{type(e).__name__}: {e}",
                          "dove": traceback.format_exc(limit=3)}, ensure_ascii=False))
        raise SystemExit(1)
"#;

/// Dove stanno: accanto a `config.json`, come nel Python.
pub fn cartella() -> PathBuf {
    crate::mondo::cartella_nova().join("automazioni")
}

fn file(nome: &str) -> PathBuf {
    cartella().join(format!("{nome}.py"))
}

fn manifesto(nome: &str) -> PathBuf {
    cartella().join(format!("{nome}.json"))
}

/// Il Python con cui girano gli script.
pub fn python() -> String {
    std::env::var("NOVA_PYTHON")
        .unwrap_or_else(|_| if cfg!(windows) { "python" } else { "python3" }.to_string())
}

/// `^[a-z][a-z0-9_]{2,39}$`.
pub fn nome_valido(n: &str) -> bool {
    let b = n.as_bytes();
    (3..=40).contains(&b.len())
        && b[0].is_ascii_lowercase()
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'_')
}

/// La firma di `esegui`: tutti i parametri opzionali, `None` di riposo.
pub fn firma(parametri: &Map<String, Value>) -> String {
    parametri
        .keys()
        .map(|k| format!("{k}=None"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Il corpo rientrato di quattro spazi, tolto il rientro comune.
pub fn rientra(corpo: &str) -> String {
    let testo = corpo.replace("\r\n", "\n");
    let righe: Vec<&str> = testo.split('\n').collect();
    let utili: Vec<&&str> = righe.iter().filter(|r| !r.trim().is_empty()).collect();
    if utili.is_empty() {
        return "    return 'automazione vuota'".into();
    }
    let rientro = |r: &str| r.chars().count() - r.trim_start().chars().count();
    let comune = utili.iter().map(|r| rientro(r)).min().unwrap_or(0);
    righe
        .iter()
        .map(|r| {
            if r.trim().is_empty() {
                String::new()
            } else {
                format!("    {}", r.chars().skip(comune).collect::<String>())
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Il file dell'automazione, com'e' sul disco.
pub fn guscio(
    titolo: &str,
    descrizione: &str,
    quando: &str,
    radice: &str,
    firma: &str,
    corpo: &str,
) -> String {
    GUSCIO
        .replace("@@TITOLO@@", titolo)
        .replace("@@DESCRIZIONE@@", descrizione)
        .replace("@@QUANDO@@", quando)
        .replace("@@RADICE@@", radice)
        .replace("@@FIRMA@@", firma)
        .replace("@@CORPO@@", corpo)
}

fn oggi() -> String {
    let t = nova_platform::orologio::adesso();
    let d = nova_calendario::da_istante(t, nova_platform::fuso_secondi(t));
    format!("{:02}/{:02}/{:04}", d.giorno, d.mese, d.anno)
}

fn secondi_adesso() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}

fn arrotonda2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

fn leggi_json(p: &Path) -> Option<Value> {
    let t = std::fs::read_to_string(p).ok()?;
    serde_json::from_str(t.trim_start_matches('\u{feff}')).ok()
}

fn scrivi_manifesto(nome: &str, m: &Value) -> std::io::Result<()> {
    std::fs::create_dir_all(cartella())?;
    std::fs::write(
        manifesto(nome),
        nova_pitone::json_come_python_rientrato(m, 1),
    )
}

/// Le automazioni che esistono, dalla piu' usata.
pub fn elenco() -> Vec<Value> {
    let Ok(dentro) = std::fs::read_dir(cartella()) else {
        return Vec::new();
    };
    let mut fuori: Vec<Value> = dentro
        .filter_map(Result::ok)
        .map(|d| d.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .filter_map(|p| leggi_json(&p))
        .filter(|d| {
            let n = d.get("nome").and_then(Value::as_str).unwrap_or("");
            !n.is_empty() && file(n).is_file()
        })
        .collect();
    let chiave = |d: &Value| {
        (
            d.get("esecuzioni").and_then(Value::as_f64).unwrap_or(0.0),
            d.get("creata").and_then(Value::as_f64).unwrap_or(0.0),
        )
    };
    fuori.sort_by(|a, b| {
        chiave(b)
            .partial_cmp(&chiave(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    fuori
}

pub fn leggi(nome: &str) -> Option<Value> {
    if !nome_valido(nome) {
        return None;
    }
    leggi_json(&manifesto(nome))
}

pub fn codice(nome: &str) -> String {
    if !nome_valido(nome) {
        return String::new();
    }
    std::fs::read_to_string(file(nome)).unwrap_or_default()
}

/// Esegue uno script in un processo a parte, con i parametri su stdin.
///
/// A parte e non dentro il demone: un'automazione che va in ciclo o si
/// prende la memoria non deve poter portarsi via NOVA.
pub async fn lancia(percorso: &Path, parametri: &Value, attesa: u64) -> Value {
    let inizio = std::time::Instant::now();
    let radice = crate::memoria::radice_progetto();
    let args = vec![python(), percorso.to_string_lossy().to_string()];
    let dentro = nova_pitone::json_come_python(parametri);
    let esito = crate::processo::lancia_con(
        &args,
        Some(&dentro),
        &radice.to_string_lossy(),
        attesa,
        &[("PYTHONIOENCODING", "utf-8")],
    )
    .await;
    let secondi = arrotonda2(inizio.elapsed().as_secs_f64());
    let u = match esito {
        Ok(u) => u,
        Err(crate::processo::Guaio::Troppo) => {
            return json!({ "ok": false, "errore": format!("non e' finita entro {attesa} secondi"),
                "secondi": attesa });
        }
        Err(crate::processo::Guaio::Muto(e)) => {
            return json!({ "ok": false, "secondi": secondi,
                "errore": format!("Python non parte ({e}): le automazioni sono script Python, \
                                   e senza Python non girano") });
        }
    };
    let uscita = u.stdout.trim();
    if uscita.is_empty() {
        let coda: String = {
            let e = u.stderr.trim();
            let n = e.chars().count();
            e.chars().skip(n.saturating_sub(300)).collect()
        };
        return json!({ "ok": false, "secondi": secondi, "errore": format!("nessuna uscita. {coda}") });
    }
    let ultima = uscita.lines().last().unwrap_or("");
    match serde_json::from_str::<Value>(ultima) {
        Ok(Value::Object(mut d)) => {
            d.insert("secondi".into(), json!(secondi));
            Value::Object(d)
        }
        _ => json!({ "ok": false, "secondi": secondi,
            "errore": format!("uscita non interpretabile: {}", uscita.chars().take(300).collect::<String>()) }),
    }
}

/// Se il codice compila: `None`, o `(riga, messaggio)` come li dice Python.
async fn compila(percorso: &Path) -> Result<Option<(i64, String)>, String> {
    const CONTROLLO: &str = "import sys, json\n\
        s = open(sys.argv[1], encoding='utf-8').read()\n\
        try:\n    compile(s, sys.argv[1], 'exec'); print(json.dumps({'ok': True}))\n\
        except SyntaxError as e:\n    print(json.dumps({'ok': False, 'riga': e.lineno, 'msg': e.msg}))\n";
    let args = vec![
        python(),
        "-c".into(),
        CONTROLLO.into(),
        percorso.to_string_lossy().to_string(),
    ];
    let u = crate::processo::lancia_con(&args, None, "", 60, &[("PYTHONIOENCODING", "utf-8")])
        .await
        .map_err(|e| match e {
            crate::processo::Guaio::Muto(m) => format!(
                "Python non parte ({m}): le automazioni sono script Python, e senza Python non \
                 si scrivono"
            ),
            crate::processo::Guaio::Troppo => "il controllo del codice non e' finito".into(),
        })?;
    let v: Value =
        serde_json::from_str(u.stdout.trim().lines().last().unwrap_or("")).map_err(|_| {
            format!(
                "il controllo del codice non ha risposto: {}",
                u.stderr.trim()
            )
        })?;
    if v["ok"] == json!(true) {
        return Ok(None);
    }
    Ok(Some((
        v["riga"].as_i64().unwrap_or(0),
        v["msg"].as_str().unwrap_or("").to_string(),
    )))
}

/// Cosa chiede chi crea un'automazione.
pub struct Nuova<'a> {
    pub nome: &'a str,
    pub titolo: &'a str,
    pub descrizione: &'a str,
    pub corpo: &'a str,
    pub parametri: Map<String, Value>,
    pub prova: Map<String, Value>,
    pub rischio: &'a str,
    pub da_procedura: &'a str,
}

fn scrivi_file(n: &Nuova, nome: &str, dove: &Path) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dove)?;
    let testo = guscio(
        if n.titolo.is_empty() { nome } else { n.titolo },
        n.descrizione,
        &oggi(),
        &crate::memoria::radice_progetto().to_string_lossy(),
        &firma(&n.parametri),
        &rientra(n.corpo),
    );
    let p = dove.join(format!("{nome}.py"));
    std::fs::write(&p, testo)?;
    Ok(p)
}

/// Scrive l'automazione, la prova, e la salva **solo se la prova gira**.
pub async fn crea(n: Nuova<'_>) -> Result<Value, String> {
    let nome = n.nome.trim().to_lowercase();
    if !nome_valido(&nome) {
        return Err(
            "il nome va da 3 a 40 caratteri, minuscole, cifre e _, e comincia con una lettera"
                .into(),
        );
    }
    if n.corpo.trim().is_empty() {
        return Err("serve il corpo della funzione".into());
    }
    let appoggio = cartella().join("_prova");
    let percorso = scrivi_file(&n, &nome, &appoggio).map_err(|e| e.to_string())?;
    if let Some((riga, msg)) = compila(&percorso).await.inspect_err(|_| {
        let _ = std::fs::remove_file(&percorso);
    })? {
        let _ = std::fs::remove_file(&percorso);
        return Err(format!("il codice non compila: riga {riga}: {msg}"));
    }
    let esito = lancia(&percorso, &Value::Object(n.prova.clone()), ATTESA_AUTOMAZIONE_S).await;
    if esito["ok"] != json!(true) {
        let _ = std::fs::remove_file(&percorso);
        let errore: String = nova_pitone::str_di(esito.get("errore"))
            .chars()
            .take(400)
            .collect();
        let errore = if esito.get("errore").is_none() {
            String::new()
        } else {
            errore
        };
        return Err(format!(
            "la prova non e' andata a buon fine, non la salvo. {errore}"
        ));
    }
    let definitivo = scrivi_file(&n, &nome, &cartella()).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&percorso);
    let rischio = if ["safe", "moderate", "dangerous"].contains(&n.rischio) {
        n.rischio
    } else {
        "dangerous"
    };
    let m = json!({
        "nome": nome,
        "titolo": if n.titolo.is_empty() { nome.as_str() } else { n.titolo },
        "descrizione": n.descrizione,
        "parametri": n.parametri,
        "rischio": rischio,
        "creata": secondi_adesso(),
        "da_procedura": n.da_procedura,
        "esecuzioni": 0,
        "fallimenti": 0,
        "ultimo_uso": 0,
        "secondi": esito.get("secondi").cloned().unwrap_or(json!(0)),
        "prova": n.prova,
    });
    scrivi_manifesto(&nome, &m).map_err(|e| e.to_string())?;
    let mut fuori = m;
    fuori["esito_prova"] = esito.get("risultato").cloned().unwrap_or(json!(""));
    fuori["percorso"] = json!(definitivo.to_string_lossy());
    Ok(fuori)
}

/// Fa girare un'automazione e tiene il conto di come e' andata.
pub async fn esegui(nome: &str, parametri: &Value) -> Result<Value, String> {
    let Some(mut m) = leggi(nome) else {
        return Err(format!("non esiste nessuna automazione «{nome}»"));
    };
    let p = file(nome);
    if !p.is_file() {
        return Err(format!("il file di «{nome}» non c'e' piu'"));
    }
    let esito = lancia(&p, parametri, ATTESA_AUTOMAZIONE_S).await;
    let conta = |m: &Value, k: &str| m.get(k).and_then(Value::as_i64).unwrap_or(0);
    m["esecuzioni"] = json!(conta(&m, "esecuzioni") + 1);
    if esito["ok"] != json!(true) {
        m["fallimenti"] = json!(conta(&m, "fallimenti") + 1);
    }
    m["ultimo_uso"] = json!(secondi_adesso());
    if let Some(s) = esito
        .get("secondi")
        .and_then(Value::as_f64)
        .filter(|s| *s != 0.0)
    {
        let prima = m
            .get("secondi")
            .and_then(Value::as_f64)
            .filter(|x| *x != 0.0)
            .unwrap_or(s);
        m["secondi"] = json!(arrotonda2((prima + s) / 2.0));
    }
    let _ = scrivi_manifesto(nome, &m);
    Ok(esito)
}

pub fn elimina(nome: &str) -> bool {
    if !nome_valido(nome) {
        return false;
    }
    let mut trovata = false;
    for p in [file(nome), manifesto(nome)] {
        if p.exists() && std::fs::remove_file(&p).is_ok() {
            trovata = true;
        }
    }
    trovata
}

/// L'automazione nata da una certa procedura, se c'e'.
pub fn per_procedura(id: &str) -> Option<Value> {
    elenco()
        .into_iter()
        .find(|a| a.get("da_procedura").and_then(Value::as_str) == Some(id))
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn i_nomi() {
        for si in ["abc", "controlla_posta", "a12", &"a".repeat(40)] {
            assert!(nome_valido(si), "{si}");
        }
        for no in [
            "ab",
            "1abc",
            "Abc",
            "a-b",
            "àbc",
            "",
            &"a".repeat(41),
            "../x",
        ] {
            assert!(!nome_valido(no), "{no}");
        }
    }

    #[test]
    fn il_corpo_si_rientra_di_quattro() {
        assert_eq!(
            rientra("    x = 1\r\n    return x"),
            "    x = 1\n    return x"
        );
        assert_eq!(
            rientra("if a:\n    b\n\nreturn c"),
            "    if a:\n        b\n\n    return c"
        );
        assert_eq!(rientra("  \n\n"), "    return 'automazione vuota'");
    }

    #[test]
    fn la_firma_e_il_guscio() {
        let p: Map<String, Value> =
            serde_json::from_str(r#"{"quante": {"type": "integer"}, "da": {}}"#).unwrap();
        assert_eq!(firma(&p), "quante=None, da=None");
        let g = guscio("T", "D", "27/09/2026", "C:\\NOVA", "x=None", "    return x");
        assert!(g.contains("def esegui(x=None):\n    return x\n\n\nif __name__"));
        assert!(g.contains("sys.path.insert(0, r\"C:\\NOVA\")"));
        assert!(g.contains("parametri = json.loads(grezzo) if grezzo.strip() else {}"));
    }
}
