# -*- coding: utf-8 -*-
"""«Ogni lunedi' alle 9» deve dare lo stesso istante in Rust e in Python.

Ottavo pezzo del cantiere. E' della famiglia delle **decisioni**, come la
scala: non calcola qualcosa da mostrare, decide *quando* NOVA fa una cosa da
sola. Sbagliarlo di un giorno vuol dire un'attivita' che non parte, o che
parte a ripetizione.

Due modi di provarlo, e servono tutti e due.

Il primo e' il confronto fra le due implementazioni, caso per caso, cifra per
cifra. Trova le divergenze.

Il secondo e' un gruppo di **risultati attesi scritti a mano**, con la data
calcolata sul calendario e non chiesta a nessuno dei due. Serve perche' il
confronto da solo non basta: due implementazioni che concordano non sono due
implementazioni verificate, e un errore condiviso — copiato dal Python al
Rust senza accorgersene — passerebbe indisturbato. E' D51, imparata con
`Authorization: Bearer` che non era mascherato ne' di qua ne' di la'.

Il tempo si passa sempre da fuori, mai `datetime.now()`: cosi' la prova dice
la stessa cosa oggi, a Capodanno e il 29 febbraio.

Esce 2 se il banco non e' costruito.
"""
import json
import os
import subprocess
import sys
from datetime import datetime
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "banco-pianificazione.exe" if os.name == "nt" else "banco-pianificazione"
BINARIO = RADICE / "core" / "target" / "release" / NOME
if not BINARIO.is_file():
    print("Il banco Rust non e' costruito per questo sistema. Per averlo:")
    print("  cd core && cargo build --release -p nova-pianificazione "
          "--features banco --bin banco-pianificazione")
    sys.exit(2)

from nova.pianificazione import prossimo                      # noqa: E402

passati = 0
falliti: list[str] = []


def controlla(nome, condizione, dettaglio=""):
    global passati
    if condizione:
        passati += 1
        print(f"  [ok ] {nome}")
    else:
        falliti.append(nome + (f" — {dettaglio}" if dettaglio else ""))
        print(f"  [NO ] {nome}" + (f" — {dettaglio}" if dettaglio else ""))


def rust(casi: list[tuple[str, str]]) -> list[dict]:
    dentro = "\n".join(
        json.dumps({"quando": q, "da": d}, ensure_ascii=False) for q, d in casi)
    p = subprocess.run([str(BINARIO)], input=dentro, capture_output=True,
                       text=True, encoding="utf-8", timeout=120)
    if p.returncode != 0:
        print("il banco e' uscito male:", p.stderr[:400])
        sys.exit(1)
    return [json.loads(r) for r in p.stdout.splitlines() if r.strip()]


def python(quando: str, da_iso: str) -> dict:
    """La stessa domanda al Python, nella stessa forma.

    `prossimo` torna un timestamp; qui si ritorna alla data locale, che e' la
    forma su cui si puo' confrontare senza tirare dentro i fusi — che nel
    Rust, di proposito, non ci sono.
    """
    da = datetime.fromisoformat(da_iso)
    try:
        t = prossimo(quando, da)
    except ValueError as e:
        return {"ok": False, "errore": str(e)}
    return {"ok": True, "prossimo": datetime.fromtimestamp(t).isoformat()}


# ---------------------------------------------------------------------------
MOMENTI = [
    "2026-09-02T10:00:00",   # un mercoledi'
    "2026-09-02T08:59:00",
    "2026-09-02T09:00:00",   # esattamente sull'ora predefinita
    "2026-09-06T23:59:00",   # domenica sera
    "2026-12-31T23:30:00",   # scavalca l'anno
    "2024-02-28T23:30:00",   # scavalca il giorno bisestile
    "2026-02-28T10:00:00",   # e lo stesso giorno in un anno che non lo e'
]
FRASI = [
    "ogni giorno 08:00", "ogni giorno 18:30", "ogni giorno 00:00",
    "ogni lunedi 09:00", "ogni lunedì 09:00", "ogni martedi 07:15",
    "ogni mercoledi 10:00", "ogni giovedi", "ogni venerdi 23:59",
    "ogni sabato 06:00", "ogni domenica 12:00",
    "ogni 30 minuti", "ogni 1 minuti", "ogni 90 minuti",
    "ogni 2 ore", "ogni 12 ore", "ogni minuto", "ogni ora",
    "ogni giorno 8.30", "OGNI GIORNO 08:00", "  ogni giorno 08:00  ",
    "ogni giorno", "08:00",
    # e le frasi che devono fallire, allo stesso modo da tutte e due le parti
    "", "   ", "quando ti pare", "ogni giorno 99:99", "ogni giorno 24:00",
]

print(f"\n=== Le due implementazioni, {len(MOMENTI)} momenti x {len(FRASI)} frasi ===")
casi = [(f, m) for m in MOMENTI for f in FRASI]
risposte = rust(casi)
controlla("il banco risponde a tutte le domande",
          len(risposte) == len(casi), f"{len(risposte)} su {len(casi)}")

diverse = []
for (q, m), r in zip(casi, risposte):
    p = python(q, m)
    if r.get("ok") != p.get("ok"):
        diverse.append(f"«{q}» da {m}: rust ok={r.get('ok')} python ok={p.get('ok')}")
    elif r.get("ok") and r.get("prossimo") != p.get("prossimo"):
        diverse.append(f"«{q}» da {m}: rust {r.get('prossimo')} python {p.get('prossimo')}")
controlla(f"tutti i {len(casi)} casi danno lo stesso istante",
          not diverse, "; ".join(diverse[:4]))

print("\n=== E i risultati attesi, calcolati sul calendario ===")
# Scritti a mano guardando un calendario, non chiesti a nessuna delle due
# implementazioni: e' l'unica parte della prova che sopravviverebbe a un
# errore copiato dall'una all'altra.
# Il 2 settembre 2026 e' un mercoledi'.
ATTESI = [
    ("ogni giorno 08:00", "2026-09-02T10:00:00", "2026-09-03T08:00:00"),
    ("ogni giorno 18:30", "2026-09-02T10:00:00", "2026-09-02T18:30:00"),
    # oggi e' mercoledi': mercoledi' alle 8 e' passato, si va alla settimana dopo
    ("ogni mercoledi 08:00", "2026-09-02T10:00:00", "2026-09-09T08:00:00"),
    ("ogni mercoledi 18:00", "2026-09-02T10:00:00", "2026-09-02T18:00:00"),
    ("ogni lunedi 09:00", "2026-09-02T10:00:00", "2026-09-07T09:00:00"),
    ("ogni domenica 12:00", "2026-09-02T10:00:00", "2026-09-06T12:00:00"),
    # senza orario si usano le nove
    ("ogni giovedi", "2026-09-02T10:00:00", "2026-09-03T09:00:00"),
    # gli intervalli si contano da adesso, e scavallano l'anno
    ("ogni 30 minuti", "2026-12-31T23:30:00", "2027-01-01T00:00:00"),
    ("ogni 2 ore", "2026-12-31T23:30:00", "2027-01-01T01:30:00"),
    # il 2024 e' bisestile: dal 28 febbraio si passa per il 29
    ("ogni giorno 08:00", "2024-02-28T23:30:00", "2024-02-29T08:00:00"),
    # il 2026 no: dal 28 si va direttamente a marzo
    ("ogni giorno 08:00", "2026-02-28T10:00:00", "2026-03-01T08:00:00"),
]
sbagliati_rust, sbagliati_py = [], []
r_att = rust([(q, m) for q, m, _ in ATTESI])
for (q, m, atteso), r in zip(ATTESI, r_att):
    if r.get("prossimo") != atteso:
        sbagliati_rust.append(f"«{q}» da {m}: {r.get('prossimo')} invece di {atteso}")
    p = python(q, m)
    if p.get("prossimo") != atteso:
        sbagliati_py.append(f"«{q}» da {m}: {p.get('prossimo')} invece di {atteso}")
controlla("il Rust dà i risultati veri", not sbagliati_rust,
          "; ".join(sbagliati_rust[:3]))
controlla("e il Python pure", not sbagliati_py, "; ".join(sbagliati_py[:3]))

print("\n=== Il prossimo è sempre avanti ===")
# La proprietà che nessuno dei due deve rompere: un istante nel passato
# farebbe partire l'attività subito, e poi di nuovo, e poi di nuovo.
indietro = []
buone = [f for f in FRASI if f.strip() and "99:99" not in f
         and "24:00" not in f and "pare" not in f]
r_av = rust([(f, m) for m in MOMENTI for f in buone])
for (q, m), r in zip([(f, m) for m in MOMENTI for f in buone], r_av):
    if r.get("ok") and r.get("prossimo") <= m:
        indietro.append(f"«{q}» da {m} -> {r.get('prossimo')}")
controlla("nessuna frase valida guarda all'indietro", not indietro,
          "; ".join(indietro[:3]))

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
