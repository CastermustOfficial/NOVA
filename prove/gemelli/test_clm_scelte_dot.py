# -*- coding: utf-8 -*-
"""Gli esempi per le scelte dei Dot, e le soglie che arrivano al demone (D404).

CLM impara il custode, e per AR chi lavora e il cervello. Qui, senza
modelli, si guarda il pezzo di `misure/clm_addestra.py` che li prepara:

- le domande vengono da `banco-testi-clm`, cioe' dalle funzioni del demone:
  lo stato e i candidati dell'addestramento sono quelli che il demone chiede;
- le scelte vere del registro entrano solo se non le ha fatte CLM, e un
  cervello che ha preso un voto sotto 6 insegna il gradino sopra;
- una situazione troppo simile al banco resta fuori;
- le soglie misurate finiscono in `teste.json`, e il demone le legge (le
  teste si caricano col banco Rust di `nova-clm`).

Esce 2 — «qui non si puo' provare» — se i banchi non sono costruiti o manca
numpy.
"""
import json
import os
import subprocess
import sys
import tempfile
import types
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.path.insert(0, str(RADICE / "misure"))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

ESE = ".exe" if os.name == "nt" else ""
TESTI = RADICE / "core" / "target" / "release" / f"banco-testi-clm{ESE}"
CLM = RADICE / "core" / "target" / "release" / f"banco-clm{ESE}"
if not TESTI.is_file():
    print("Il banco dei testi non e' costruito per questo sistema. Per averlo:")
    print("  cd core && cargo build --release -p nova-dot --features banco --bin banco-testi-clm")
    sys.exit(2)
if not CLM.is_file():
    print("Il banco Rust di CLM non e' costruito per questo sistema. Per averlo:")
    print("  cd core && cargo build --release -p nova-clm --features banco --bin banco-clm")
    sys.exit(2)
try:
    import numpy as np
except ImportError:
    print("manca numpy")
    sys.exit(2)

import banco_scelte_dot as bd                                     # noqa: E402
import clm_addestra as ca                                         # noqa: E402

passati = 0
falliti: list[str] = []


def controlla(nome, condizione, dettaglio=""):
    global passati
    if condizione:
        passati += 1
        print(f"  [ok ] {nome}")
    else:
        falliti.append(nome)
        print(f"  [NO ] {nome}  {dettaglio}")


def dot(nome, ruolo):
    return {"nome": nome, "ruolo": ruolo, "mestiere": "generico", "libero": True, "fatti": 1,
            "falliti": 0}


print("\n1. il banco: domande coi testi del demone")
casi = bd.domande()
per = {k: sum(1 for x, _, _ in casi if x == k) for k in ca.SCELTE_DOT}
controlla("venti casi per il custode, dodici per chi lavora, dieci per il cervello",
          per == {"custode": 20, "ar_chi": 12, "ar_cervello": 10}, str(per))
controlla("ogni risposta attesa e' fra i candidati del suo caso",
          all(at in d["ids"] and len(d["ids"]) == len(d["candidati"]) for _, d, at in casi))
controlla("lo stato del custode finisce con l'istruzione della domanda si'/no",
          casi[0][1]["stato"].endswith("niente cambiamenti al sistema che il compito non richiede.")
          and casi[0][1]["ids"] == ["si", "no"], casi[0][1]["stato"][-120:])

print("\n2. le scelte vere: solo quelle che non ha fatto CLM, e il voto corregge il cervello")
with tempfile.TemporaryDirectory() as d:
    d = Path(d)
    sit = {"bisogno": "uno che scriva", "compito": "scrivi la scheda", "scala": ["piccolo", "medio", "grande"],
           "candidati": [dot("scrivano", "Scrive."), dot("lettore", "Legge.")]}
    registro = d / "decisioni.jsonl"
    righe = [
        {"tipo": "permesso_dot", "chi": "casa", "consentito": True, "richiesta": "Il Dot: a. stato uno"},
        {"tipo": "permesso_dot", "chi": "grande", "consentito": False, "richiesta": "Il Dot: b. stato due"},
        {"tipo": "permesso_dot", "chi": "clm", "consentito": True, "richiesta": "Il Dot: c. di CLM"},
        {"tipo": "permesso_dot", "chi": "nessuno", "consentito": False, "richiesta": "Il Dot: d. nessuno"},
        {"tipo": "scelta_ar", "scelta": "riprendi", "dot": "scrivano", "cervello": "piccolo", "compito": 4,
         "chi_da": "grande", "cervello_da": "grande", "situazione": sit},
        {"tipo": "voto", "dot": "scrivano", "compito": 4, "voto": 5},
        {"tipo": "scelta_ar", "scelta": "assumi", "dot": "nuovo", "cervello": "medio", "compito": 7,
         "chi_da": "clm", "cervello_da": "grande", "situazione": sit},
        {"tipo": "scelta_ar", "scelta": "riprendi", "dot": "lettore", "cervello": "grande", "compito": None,
         "deciso_da": "grande", "situazione": sit},
        {"tipo": "scelta_ar", "scelta": "riprendi", "dot": "lettore", "cervello": "medio", "compito": 9,
         "situazione": None},
    ]
    registro.write_text("".join(json.dumps(r) + "\n" for r in righe), encoding="utf-8")
    (d / "addestra").mkdir()
    ca.comando_veri(types.SimpleNamespace(registro=registro, cartella=d))
    veri = ca.leggi_jsonl(d / "addestra" / "veri.jsonl")
    cus = [x for x in veri if x["scelta"] == "custode"]
    controlla("il custode: entrano le decisioni del modello di casa e del grande, non di CLM ne' di nessuno",
              [x["id"] for x in cus] == ["si", "no"]
              and cus[0]["domanda"]["stato"].startswith("Il Dot: a. stato uno\n\n"), str(cus)[:300])
    chi = [x["id"] for x in veri if x["scelta"] == "ar_chi"]
    controlla("chi lavora: le scelte del cervello grande, non quella di CLM", chi == ["scrivano", "lettore"],
              str(chi))
    cerv = [x["id"] for x in veri if x["scelta"] == "ar_cervello"]
    controlla("il cervello: un voto sotto 6 insegna il gradino sopra; senza compito non entra",
              cerv == ["medio", "medio"], str(cerv))
    controlla("le domande sono quelle del demone",
              all(x["domanda"]["ids"][0] == "scrivano" for x in veri if x["scelta"] == "ar_chi")
              and all(x["domanda"]["ids"] == ["piccolo", "medio", "grande"]
                      for x in veri if x["scelta"] == "ar_cervello"))

    print("\n3. gli esempi: il maestro e le scelte vere, senza i parenti del banco")
    simile = dict(bd.CASI_CUSTODE[0])
    del simile["atteso"]
    nuova = {"dot": "x", "ruolo": "r", "compito": "Ordina le ricevute del 2025 per mese",
             "strumento": "fs.move", "rischio": "moderate", "dettaglio": "sposterei 40 PDF in Ricevute/2025"}
    dom = ca.banco_testi({"cosa": "domande", "custode": [simile, nuova],
                          "ar": [{**sit, "bisogno": "un traduttore"}]})
    et = [{"scelta": "custode", **simile, "scelta_maestro": "si", "domanda": dom["custode"][0]["clm"]},
          {"scelta": "custode", **nuova, "scelta_maestro": "no", "domanda": dom["custode"][1]["clm"]},
          {"scelta": "ar", **sit, "bisogno": "un traduttore", "chi": "__assumi__", "cervello": "piccolo",
           "domanda_chi": dom["ar"][0]["chi"], "domanda_cervello": dom["ar"][0]["cervello"]}]
    ca.scrivi_jsonl(d / "addestra" / "situazioni_etichettate.jsonl", et)
    es = ca.esempi_dot(d)
    controlla("il caso simile al banco resta fuori, l'altro entra con la sua risposta",
              len(es["custode"]) == 1 + 2 and es["custode"][0]["giusto"] == 1
              and "Ricevute/2025" in es["custode"][0]["stato"], str([e["stato"][-60:] for e in es["custode"]]))
    controlla("chi lavora: «assumi» e' l'ultimo candidato",
              any(e["giusto"] == len(e["candidati"]) - 1 and e["stato"].startswith("Cosa serve: un traduttore")
                  for e in es["ar_chi"]), str(es["ar_chi"])[:300])
    controlla("e gli esempi veri ci sono anche loro",
              len(es["ar_chi"]) == 3 and len(es["ar_cervello"]) == 3, f"{len(es['ar_chi'])} {len(es['ar_cervello'])}")

print("\n4. la soglia: la piu' bassa al 95%, mai zero; i si' sbagliati si contano")
tutti_giusti = [("si", 0.6, "si")] * 19 + [("si", 0.55, "no")]
s_, righe_ = ca.soglia_da(tutti_giusti)
controlla("se tutto e' sopra il 95%, la soglia e' 0,5 e non zero", s_ == 0.5, str(s_))
controlla("e il si' sbagliato e' contato", righe_[1]["si_sbagliati"] == 1, str(righe_[1]))
misti = [("si", 0.97, "si")] * 19 + [("si", 0.96, "no")] + [("no", 0.7, "si")] * 10
# Fino a 0,7 decide anche i dieci sbagliati; da 0,75 restano 19 giusti su 20.
controlla("altrimenti la piu' bassa che arriva al 95%", ca.soglia_da(misti)[0] == 0.75,
          str(ca.soglia_da(misti)[0]))
controlla("e nessuna, se nessuna ci arriva",
          ca.soglia_da([("a", 0.99, "b"), ("a", 0.99, "a")])[0] is None)

print("\n5. le soglie misurate arrivano a teste.json, e il demone le legge")
with tempfile.TemporaryDirectory() as d:
    sorgente, dest = Path(d) / "npz", Path(d) / "rust"
    sorgente.mkdir()
    rng = np.random.default_rng(3)
    pesi = {}
    for lato in ("state_head", "action_head"):
        for n, (r, c) in {"inp": (4, 6), "hidden.0": (4, 4), "out": (3, 4)}.items():
            pesi[f"{lato}.{n}.weight"] = rng.standard_normal((r, c)).astype("f4")
            pesi[f"{lato}.{n}.bias"] = rng.standard_normal(r).astype("f4")
    np.savez(sorgente / "teste.npz", **pesi)
    (sorgente / "teste.json").write_text(json.dumps(
        {"cfg": {"depth": 3, "activation": "gelu", "layernorm": False, "residual": False}, "scala": 20.0}),
        encoding="utf-8")
    (sorgente / "soglia.json").write_text(json.dumps({"soglia": 0.8}), encoding="utf-8")
    (sorgente / "soglia-custode.json").write_text(json.dumps({"soglia": 0.9}), encoding="utf-8")
    (sorgente / "soglia-ar_chi.json").write_text(json.dumps({"soglia": None}), encoding="utf-8")
    ca.esporta(sorgente, dest)
    scritte = json.loads((dest / "teste.json").read_text(encoding="utf-8"))["soglie"]
    controlla("una soglia per scelta; senza una soglia al 95% la scelta non c'e'",
              scritte == {"quale_cervello": 0.8, "custode": 0.9}, str(scritte))
    solo = Path(d) / "solo"
    ca.esporta(sorgente, solo, ["quale_cervello"])
    controlla("e si possono esportare le soglie di alcune scelte soltanto",
              json.loads((solo / "teste.json").read_text(encoding="utf-8"))["soglie"] == {"quale_cervello": 0.8})
    r = subprocess.run([str(CLM)], capture_output=True, text=True, timeout=60,
                       input=json.dumps({"cartella": str(dest), "stati": [[0.1] * 6],
                                         "candidati": [[0.2] * 6, [0.3] * 6]}))
    controlla("le teste con le soglie si leggono in Rust", r.returncode == 0, r.stderr[-300:])
    scritte_male = json.loads((dest / "teste.json").read_text(encoding="utf-8"))
    scritte_male["soglie"]["custode"] = 1.5
    (dest / "teste.json").write_text(json.dumps(scritte_male), encoding="utf-8")
    r = subprocess.run([str(CLM)], capture_output=True, text=True, timeout=60,
                       input=json.dumps({"cartella": str(dest), "stati": [[0.1] * 6],
                                         "candidati": [[0.2] * 6]}))
    controlla("e una soglia fuori da 0-1 si rifiuta, e si dice", r.returncode != 0 and "va da 0 a 1" in r.stderr,
              r.stderr[-200:])

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
