# -*- coding: utf-8 -*-
"""«Dove sono i miei dati?» deve rispondere identico in Rust.

Non e' una domanda di comodo: e' quella che decide se qualcuno lascia
installato un programma che gli legge la posta e gli tiene le password. E
l'elenco lo leggono **tre** posti diversi — il racconto in chat, il
rendiconto che l'installatore stampa mentre disinstalla, e il pannello. Tre
letture dello stesso file che dicono tre misure diverse non sono un dettaglio
estetico: sono tre programmi che sembrano non essersi parlati.

I percorsi non si confrontano, e apposta: quelli li dice il modulo che ci
scrive dentro, e una mappa che ne tiene una copia prima o poi indica un file
che non c'e' (D188). Qui si confronta cio' che dalle due parti e' lo stesso
ragionamento: come si scrive una misura, come si mette insieme il racconto,
cosa finisce nel rendiconto, e la domanda «sta dentro questa cartella?».

Esce 2 se il banco non e' costruito.
"""
import json
import os
import subprocess
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "banco-dati.exe" if os.name == "nt" else "banco-dati"
BINARIO = RADICE / "core" / "target" / "release" / NOME
if not BINARIO.is_file():
    print("Il banco Rust non e' costruito per questo sistema. Per averlo:")
    print("  cd core && cargo build --release -p nova-dati "
          "--features banco --bin banco-dati")
    sys.exit(2)

from nova import dati                                        # noqa: E402
from nova.percorsi import dentro as py_dentro                 # noqa: E402

passati = 0
falliti = []


def controlla(nome, ok, dettaglio=""):
    global passati
    if ok:
        passati += 1
        print(f"  [ok ] {nome}")
    else:
        falliti.append(nome)
        print(f"  [NO ] {nome}" + (f"  -- {dettaglio}" if dettaglio else ""))


def chiedi(domande):
    testo = "\n".join(json.dumps(d, ensure_ascii=False) for d in domande)
    p = subprocess.run([str(BINARIO)], input=testo, capture_output=True,
                       text=True, encoding="utf-8", timeout=120)
    if p.returncode != 0:
        print("il banco e' uscito male:", p.returncode, p.stderr[:400])
        sys.exit(1)
    return [json.loads(r) for r in p.stdout.splitlines() if r.strip()]


# -- le misure -------------------------------------------------------------
MISURE = [0, 1, 2, 511, 1023, 1024, 1025, 1535, 1536, 1537, 2048, 10_000,
          1_048_575, 1_048_576, 1_048_577, 1_572_864, 5_000_000,
          1_073_741_823, 1_073_741_824, 2_000_000_000, 17_179_869_184,
          999_999_999_999]
# E una manciata di numeri intorno a ogni soglia, che e' dove si sbaglia.
for soglia in (1024, 1024 * 1024, 1024 * 1024 * 1024):
    MISURE += [soglia - 2, soglia - 1, soglia, soglia + 1, soglia + 2,
               soglia + soglia // 2]

casi = [{"tipo": "pesa", "byte": b} for b in MISURE]

# -- «sta dentro questa cartella?» ----------------------------------------
DENTRO = [
    ("/casa/NOVA/vault", "/casa/NOVA"),
    ("/casa/NOVA", "/casa/NOVA"),
    ("/casa/NOVA-vecchio/x", "/casa/NOVA"),    # il buco: NON deve passare
    ("/casa", "/casa/NOVA"),
    ("/altrove/x", "/casa/NOVA"),
    ("/casa/NOVA/a/b/c.txt", "/casa/NOVA"),
]
casi += [{"tipo": "dentro", "quale": a, "cartella": b} for a, b in DENTRO]

# -- il racconto e il rendiconto ------------------------------------------
POSTI = [
    {"che_cos_e": "Le credenziali", "dove": "/casa/NOVA/segreti.dat",
     "se_lo_cancelli": "NOVA non sa piu' entrare da nessuna parte.",
     "delicato": True, "esiste": True, "byte": 128, "quanti_file": 1},
    {"che_cos_e": "La memoria a grafo", "dove": "/casa/NOVA/vault",
     "se_lo_cancelli": "NOVA dimentica quello che ha imparato su di te.",
     "esiste": True, "byte": 1_572_864, "quanti_file": 87},
    {"che_cos_e": "Il fascicolo", "dove": "/utente/Documenti/NOVA/fascicolo",
     "se_lo_cancelli": "NOVA torna a non sapere niente di te.",
     "delicato": True, "esiste": True, "byte": 40_000, "quanti_file": 3},
    {"che_cos_e": "Le cose in programma", "dove": "/casa/NOVA/programma.json",
     "se_lo_cancelli": "NOVA smette di fare da sola le cose ricorrenti.",
     "esiste": False, "byte": 0, "quanti_file": 0},
]
for solo in (True, False):
    casi.append({"tipo": "racconta", "posti": POSTI, "base": "/casa/NOVA",
                 "solo_esistenti": solo})
casi.append({"tipo": "racconta", "posti": [], "base": "/casa/NOVA"})
casi.append({"tipo": "racconta",
             "posti": [dict(POSTI[3])], "base": "/casa/NOVA"})
casi.append({"tipo": "rendiconto", "posti": POSTI, "base": "/casa/NOVA"})

risposte = chiedi(casi)
print(f"=== {len(casi)} casi, una testa contro l'altra ===")


def py_racconta(posti, base, solo_esistenti):
    """Il racconto del Python, sugli stessi dati e senza toccare il disco."""
    class Finto:
        def __init__(self, v):
            self.che_cos_e = v["che_cos_e"]
            self.dove = Path(v["dove"])
            self.se_lo_cancelli = v["se_lo_cancelli"]
            self.delicato = v.get("delicato", False)
            self._esiste = v.get("esiste", False)
            self._byte = v.get("byte", 0)
            self._file = v.get("quanti_file", 0)
        esiste = property(lambda s: s._esiste)
        byte = property(lambda s: s._byte)
        quanti_file = property(lambda s: s._file)

    finti = [Finto(v) for v in posti]
    righe = ["Dove NOVA tiene le tue cose:\n"]
    totale = visti = 0
    for p in finti:
        if solo_esistenti and not p.esiste:
            continue
        visti += 1
        totale += p.byte
        misura = dati.pesa(p.byte) + (f", {p.quanti_file} file"
                                      if p.quanti_file > 1 else "")
        segno = "  (delicato)" if p.delicato else ""
        righe.append(f"{p.che_cos_e}{segno}")
        righe.append(f"    {p.dove}")
        righe.append(f"    {misura if p.esiste else 'non ancora creato'}")
        righe.append(f"    se lo cancelli: {p.se_lo_cancelli}")
        righe.append("")
    if not visti:
        return ("NOVA non ha ancora scritto niente: la prima volta che la usi "
                f"crea la sua cartella in {base}")
    righe.append(f"In tutto {dati.pesa(totale)}.")
    righe.append("")
    righe.append("Niente di questo esce dal PC finche' il cervello e' quello "
                 "locale. Con «claude» o «api» escono la domanda, il contesto "
                 "della conversazione e i nodi di memoria pertinenti — mai le "
                 "credenziali, che il modello non vede in nessun caso.")
    return "\n".join(righe)


diversi = []
for domanda, rust in zip(casi, risposte):
    if rust.get("errore"):
        diversi.append((domanda, rust, "il banco non ha capito la domanda"))
        continue
    if domanda["tipo"] == "pesa":
        mio = dati.pesa(domanda["byte"])
        if mio != rust.get("misura"):
            diversi.append((domanda["byte"], rust.get("misura"), mio))
    elif domanda["tipo"] == "dentro":
        mio = py_dentro(Path(domanda["quale"]), Path(domanda["cartella"]))
        if bool(mio) != rust.get("dentro"):
            diversi.append((domanda, rust.get("dentro"), mio))
    elif domanda["tipo"] == "racconta":
        mio = py_racconta(domanda["posti"], domanda["base"],
                          domanda.get("solo_esistenti", True))
        if mio != rust.get("testo"):
            diversi.append(("racconto", rust.get("testo"), mio))
    else:
        voci = []
        for v in domanda["posti"]:
            if not v.get("esiste"):
                continue
            voci.append({
                "che_cos_e": v["che_cos_e"], "dove": v["dove"],
                "byte": v["byte"], "misura": dati.pesa(v["byte"]),
                "delicato": v.get("delicato", False),
                "va_via_con_la_cartella": bool(
                    py_dentro(Path(v["dove"]), Path(domanda["base"]))),
                "se_lo_cancelli": v["se_lo_cancelli"],
            })
        if voci != rust.get("voci"):
            diversi.append(("rendiconto", rust.get("voci"), voci))
        atteso = dati.pesa(sum(v["byte"] for v in voci))
        if atteso != rust.get("totale"):
            diversi.append(("totale", rust.get("totale"), atteso))

controlla("le due teste dicono le stesse cose", not diversi,
          f"{len(diversi)} casi diversi")
for a, b, c in diversi[:3]:
    print(f"       caso:    {str(a)[:100]}")
    print(f"       rust:    {str(b)[:180]}")
    print(f"       python:  {str(c)[:180]}")

# -- e quel che ci si aspetta, scritto a mano -----------------------------
print("=== e quel che ci si aspetta, scritto a mano ===")
controlla("mille e cinquecentotrentasei byte sono due kilobyte",
          dati.pesa(1536) == "2 kB", dati.pesa(1536))
controlla("e mille e cinquecentotrentacinque sono uno",
          dati.pesa(1535) == "1 kB", dati.pesa(1535))
detto = py_racconta(POSTI, "/casa/NOVA", True)
controlla("il delicato si vede nel racconto", "(delicato)" in detto)
controlla("e il racconto finisce dicendo cosa esce dal PC",
          detto.rstrip().endswith("in nessun caso."))
controlla("un posto che non c'e' non entra nel racconto",
          "Le cose in programma" not in detto)
controlla("ma entra se si chiede tutto",
          "Le cose in programma" in py_racconta(POSTI, "/casa/NOVA", False))
controlla("il fascicolo non se ne va con la cartella di NOVA",
          not py_dentro(Path("/utente/Documenti/NOVA/fascicolo"),
                        Path("/casa/NOVA")),
          "sta in Documenti apposta, per poterlo aprire a mano")

print()
print(f"{passati} passati, {len(falliti)} falliti")
for n in falliti:
    print("  -", n)
sys.exit(1 if falliti else 0)
