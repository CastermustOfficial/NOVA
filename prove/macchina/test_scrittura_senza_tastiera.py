# -*- coding: utf-8 -*-
"""Scrivere dentro un campo **senza toccare la tastiera e senza il fuoco**.

E' la regola di NOVA detta da Gio, e vale piu' di una scelta tecnica:

    NOVA non si sovrappone a cio' che fa l'utente, lavora separatamente.

`type_text` la viola per costruzione. `SendInput` non ha un bersaglio: manda
al sistema, il sistema consegna a chi ha il fuoco, e chiunque sia li' in quel
momento se lo ritrova addosso. Si puo' renderla onesta — verificare il fuoco,
nominare la finestra, rifiutare se cambia — ma non si puo' renderla separata:
prendere la tastiera e' il suo modo di funzionare. Per questo resta «ultima
spiaggia».

La strada separata e' `ui.set_text`: parla all'**applicazione**, non alla
tastiera. Il testo arriva intero, non dipende da quale finestra ha il fuoco, e
mentre NOVA scrive li' l'utente puo' continuare a scrivere altrove.

Questa prova guarda proprio quello, ed e' la cosa che finora nessuna prova
guardava: che si scriva davvero in un campo mentre il fuoco e' **da un'altra
parte**. La cavia e' una Mappa caratteri aperta da noi — ha un campo di testo
vero e nessuno stato da perdere.

Esce 2 se il demone non risponde o se la cavia non espone il campo.
"""
from __future__ import annotations

import json
import subprocess
import sys
import time
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from nova import binari  # noqa: E402

PROC = binari.trova("nova-processi")
FIN = binari.trova("nova-finestre")
if PROC is None or FIN is None:
    print("Servono nova-processi e nova-finestre.")
    sys.exit(2)

from nova.core_client import CoreClient  # noqa: E402

passati = 0
falliti: list[str] = []
COBAIA = "charmap.exe"


def controlla(nome, condizione, dettaglio=""):
    global passati
    if condizione:
        passati += 1
        print(f"  [ok ] {nome}")
    else:
        falliti.append(nome + (f" — {dettaglio}" if dettaglio else ""))
        print(f"  [NO ] {nome}" + (f" — {dettaglio}" if dettaglio else ""))


def finestre() -> list[dict]:
    r = subprocess.run([str(FIN)], capture_output=True, text=True,
                       encoding="utf-8", timeout=20)
    return json.loads(r.stdout)


def mia() -> dict | None:
    return next((w for w in finestre() if w["process"].lower() == COBAIA), None)


def spegni():
    r = subprocess.run([str(PROC)], capture_output=True, text=True,
                       encoding="utf-8", timeout=30)
    for p in json.loads(r.stdout):
        if p["nome"].lower() == COBAIA:
            subprocess.run([str(PROC), "--chiudi", str(p["pid"]), "--forza"],
                           capture_output=True, timeout=15)


spegni()
try:
    with CoreClient(timeout=30) as c:
        # Chi c'era davanti **prima** di aprire la cavia: e' li' che il fuoco
        # deve tornare, ed e' anche la premessa della prova. Aprire un
        # programma gli da' il primo piano — giustamente, l'ha chiesto
        # qualcuno — quindi il fuoco va rimesso a posto a mano prima di
        # cominciare.
        def davanti_ora() -> dict:
            return json.loads(subprocess.run(
                [str(FIN), "--davanti"], capture_output=True, text=True,
                encoding="utf-8", timeout=15).stdout or "null") or {}

        altrove = davanti_ora()
        subprocess.run([str(PROC), "--avvia", COBAIA], capture_output=True, timeout=20)
        w = None
        for _ in range(20):
            w = mia()
            if w:
                break
            time.sleep(0.5)
        if w is None:
            print("La cavia non si e' aperta: niente da provare qui.")
            sys.exit(2)

        print("\n1. si trova il campo di testo parlando all'applicazione")
        # Gli argomenti sono piatti — «role», non «query.role». La prima
        # versione di questa prova li annidava, e `ui.find` ha risposto con
        # **tutto** l'albero: sembrava un filtro ignorato dal demone, ed era
        # una chiamata scritta male da me. Un filtro che non filtra e uno che
        # non gli e' mai arrivato si somigliano molto, visti dal chiamante.
        campi = c.call("ui.find", {"window": w["handle"], "role": "edit", "limit": 5})
        elenco = campi.get("elements") or []
        controlla("la Mappa caratteri espone un campo di testo", bool(elenco),
                  json.dumps(campi)[:200])
        controlla("e il filtro per ruolo filtra davvero",
                  all(e.get("role") == "edit" for e in elenco),
                  str([e.get("role") for e in elenco]))
        # Si sceglie per **cosa sa fare**, non per come si chiama: `set_value`
        # e' la risposta dell'applicazione a «si puo' scrivere qui dentro?».
        campo = next((e for e in elenco if "set_value" in (e.get("actions") or [])), None)
        controlla("e dichiara di essere scrivibile", campo is not None,
                  str([e.get("actions") for e in elenco]))
        if campo is None:
            print("Senza un campo scrivibile non c'e' niente da provare: mi fermo qui.")
            sys.exit(2)

        print("\n2. si scrive dentro mentre il fuoco e' altrove")
        # Il cuore della prova: si scrive in una finestra che **non** e'
        # davanti. Se servisse il fuoco, qui fallirebbe.
        #
        # La premessa pero' non e' garantita: Windows non lascia che un
        # programma qualunque sposti il primo piano, ed e' giusto cosi'. Se
        # non si riesce a mandare il fuoco altrove, questa prova non e'
        # falsa — e' **non eseguibile**, e lo dice invece di diventare rossa
        # per una ragione che non c'entra con cio' che verifica (D53).
        for candidata in [altrove] + [x for x in finestre() if x["handle"] != w["handle"]]:
            if not candidata.get("handle") or candidata["handle"] == w["handle"]:
                continue
            subprocess.run([str(FIN), "--avanti", str(candidata["handle"])],
                           capture_output=True, timeout=15)
            time.sleep(0.4)
            if davanti_ora().get("handle") != w["handle"]:
                break
        davanti = davanti_ora()
        if davanti.get("handle") == w["handle"]:
            print("       (non riesco a mandare il fuoco altrove: Windows non")
            print("        lascia spostare il primo piano da qui. La parte che conta")
            print("        di questa prova non e' eseguibile adesso.)")
            sys.exit(2 if not falliti else 1)
        controlla("il fuoco NON e' sulla cavia", True,
                  f"davanti c'e' «{davanti.get('title')}»")

        TESTO = "perché città però 😀"
        # Se la scrittura non riesce, la prova e' **rossa**, non «non
        # provabile»: da qui non si puo' distinguere «NOVA non ci riesce» da
        # «la cavia in questo momento non risponde», e chiamarla non provabile
        # sarebbe scegliere l'ipotesi comoda. Quello che si puo' fare e' non
        # far uscire un traceback: chi legge il banco deve capire cosa e'
        # successo senza aprire il codice (ATT-1).
        try:
            c.call("ui.set_text", {"window": w["handle"], "path": campo["path"],
                                   "text": TESTO})
            scritto = True
        except Exception as e:                                  # noqa: BLE001
            scritto = False
            controlla("si scrive nella finestra senza toccare la tastiera",
                      False,
                      f"la cavia ha rifiutato la scrittura: {e}. "
                      "Se davanti c'e' un gioco a schermo intero o la Mappa "
                      "caratteri e' stata chiusa a mano, rilancia la prova "
                      "con la scrivania libera prima di cercare il difetto "
                      "nel codice.")
        time.sleep(0.4)

        print("\n3. e il testo c'e' davvero, riletto dall'applicazione")
        riletti = c.call("ui.find", {"window": w["handle"], "role": "edit", "limit": 5})
        dopo = riletti.get("elements") or []
        valore = next((e.get("value") for e in dopo if e.get("path") == campo["path"]), None)
        # Il ritorno a capo in coda e' della Mappa caratteri, non di NOVA:
        # verificato leggendo il campo **prima** di scriverci, dove c'era gia'
        # un `\r` da solo. Toglierlo qui e' giusto; toglierlo dentro NOVA
        # sarebbe correggere il campo di qualcun altro.
        controlla("il campo contiene quello che gli e' stato scritto",
                  scritto and (valore or "").rstrip("\r\n") == TESTO,
                  repr(valore) if scritto else "non e' stato scritto niente")

        print("\n4. e il fuoco non si e' mosso")
        # E' la regola: NOVA lavora separatamente. Se scrivendo si fosse
        # portata avanti la finestra, avrebbe interrotto chi stava usando il
        # PC — che e' esattamente cio' che non deve fare.
        dopo_f = json.loads(subprocess.run(
            [str(FIN), "--davanti"], capture_output=True, text=True,
            encoding="utf-8", timeout=15).stdout or "null") or {}
        controlla("davanti c'e' ancora quello di prima",
                  dopo_f.get("handle") == davanti.get("handle"),
                  f"prima «{davanti.get('title')}», dopo «{dopo_f.get('title')}»")
finally:
    spegni()

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
