# -*- coding: utf-8 -*-
"""I guasti che il banco del browser deve saper vedere.

Il giro sta in `_mutazioni.py`: qui ci sono solo i punti da rompere e cosa ci
si aspetta che succeda.
"""
import sys
from pathlib import Path

import _mutazioni

RADICE = Path(__file__).resolve().parents[1]
B = RADICE / "core" / "crates" / "nova-browser" / "src"

#: Ogni guasto e' `(nome, file, prima, dopo, atteso)`. `atteso` e' «rosso»
#: quasi sempre; e' «equivalente» quando il guasto **non cambia niente** e si
#: sa perche' — un mutante equivalente esiste, e dichiararlo e' meglio che
#: aggiungere una prova finta per farlo diventare rosso.
GUASTI = [
    ("il riassunto si cerca fino in fondo alla pagina",
     B / "motori.rs",
     'let finestra = if dopo <= fine { &pagina[dopo..fine] } else { "" };',
     "let finestra = &pagina[dopo..];", "rosso"),
    # Questo non cambia niente, e si sa perche': la finestra piu' larga
    # contiene in piu' soltanto il collegamento del titolo, che ha la classe
    # `result__a` e non `result__snippet`. Resta in elenco perche' il giorno
    # in cui le due classi si somigliassero, diventerebbe rosso.
    ("il riassunto si cerca dall'inizio del risultato invece che dalla fine",
     B / "motori.rs",
     "let dopo = m.get(0).map_or(0, |x| x.end());",
     "let dopo = m.get(0).map_or(0, |x| x.start());", "equivalente"),
    ("le righe si contano con lines() invece che con splitlines",
     B / "testo.rs",
     "let ripulite: Vec<String> = righe(&t)",
     "let ripulite: Vec<String> = t.lines().map(|x| x.to_string()).collect::<Vec<_>>()", "rosso"),
    ("si ripulisce con trim() invece che coi bianchi di Python",
     B / "testo.rs",
     ".map(|r| senza_bianchi(r).to_string())",
     ".map(|r| r.trim().to_string())", "rosso"),
    ("i numeri di Windows-1252 si leggono come tali",
     B / "testo.rs",
     "    if let Ok(i) = SBAGLIATI.binary_search_by(|(k, _)| k.cmp(&(n as u32))) {\n        return SBAGLIATI[i].1.to_string();\n    }\n",
     "", "rosso"),
    ("il punto e virgola del nome si butta via",
     B / "testo.rs",
     "NOMI.binary_search_by(|(k, _)| (*k).cmp(nome))",
     "NOMI.binary_search_by(|(k, _)| (*k).cmp(nome.trim_end_matches(';')))", "rosso"),
    ("l'indirizzo del rimbalzo si scioglie una volta sola",
     B / "motori.rs",
     "        Some(v) => per_cento(v),",
     "        Some(v) => v.clone(),", "rosso"),
    ("le entita' si sciolgono prima di togliere i tag",
     B / "testo.rs",
     "    let t = tag().replace_all(&t, NoExpand(\" \"));\n    let t = scioglie(&t);",
     "    let t = scioglie(&t);\n    let t = tag().replace_all(&t, NoExpand(\" \")).into_owned();", "rosso"),
    ("il titolo si taglia dove si taglia il riassunto",
     B / "motori.rs",
     "), MAX_TITOLO),",
     "), MAX_RIASSUNTO),", "rosso"),
]

sys.exit(_mutazioni.giro(GUASTI, "nova-browser", "banco-browser",
                         "test_browser_rust.py"))
