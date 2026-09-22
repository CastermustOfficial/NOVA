# -*- coding: utf-8 -*-
"""Ogni crate o lo esegue qualcuno, o dichiara perche' no.

Trentacinque crate, e una parte non la raggiunge nessuno degli eseguibili che
NOVA consegna: sono decisioni portate dal Python, provate, con il loro banco
gemello verde, e che non gira nessuna. Codice cosi' non e' neutro. Invecchia
come tutto il resto, ma invecchia **in silenzio**, perche' il modo in cui ci
si accorge che una cosa non va piu' bene e' usarla.

Non e' un difetto averne: un porto si scrive prima di attaccarlo, e
attaccarlo e' un lavoro suo. Il difetto e' non sapere quali sono. Questa
prova li conta, e pretende che ognuno stia in uno di due stati:

1. **raggiunto** da almeno uno dei binari di `core/binari.json`, seguendo le
   dipendenze dichiarate nei `Cargo.toml`;
2. **dichiarato qui sotto**, con scritto cosa manca per attaccarlo.

Un crate nuovo che non e' in nessuno dei due fa diventare rossa la suite. E
un crate dichiarato qui che nel frattempo e' stato attaccato la fa diventare
rossa lo stesso: il filo si accorcia, e l'elenco deve accorciarsi con lui.

Il conto si fa da **tutti** i binari, non da tre. La prima volta l'ho fatto
dal demone e dalla riga di comando, e ne ho dichiarati morti due che li
chiamava l'installatore (`dove_ho_sbagliato.md`). L'elenco dei binari esiste
gia' ed e' uno solo: `core/binari.json`.

Non esegue niente: legge i `Cargo.toml`.
"""
import json
import re
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
CRATES = RADICE / "core" / "crates"
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

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


# Cosa manca a ciascuno per essere attaccato. Non «e' morto»: e' la riga da
# cancellare il giorno in cui qualcuno lo esegue.
SCOLLEGATI: dict[str, str] = {
    "nova-giudizio": "la meta' pura delle decisioni tipizzate: c'e' tutta e si prova "
                     "tutta, ma nessuno gliele chiede ancora. Si attacca quando esiste la "
                     "meta' che parla con llama-server, cioe' chi i logit delle lettere li "
                     "va a prendere",
    "nova-browser": "il browser guidato da dentro: gira ancora tutto in Python "
                    "(nova/browser.py), e attaccarlo vuol dire spostare il "
                    "pilota, non aggiungerne un secondo",
    "nova-cdp": "la websocket verso il browser: serve a nova-browser, e si "
                "attacca insieme a lui",
    "nova-componenti": "le regole per procurarsi i pezzi mancanti. Scaricare e "
                       "scompattare li fa nova/componenti.py: due scaricatori "
                       "sarebbero due modi di lasciare mezzo file sul disco",
    "nova-decisioni": "il censimento di CANT-12 — quale materiale puo' uscire "
                      "dal PC. Attaccarlo e' CANT-12, che e' aperto apposta",
    "nova-docx": "modificare un .docx senza spogliarlo: lo strumento che lo "
                 "chiama e' in Python",
    "nova-fogli": "i fogli di calcolo. Manca la decisione sul motore di "
                  "ricalcolo (il banco l'ha misurata, la scelta e' di Gio)",
    "nova-harness": "il documento fatto a pezzi: e' la meta' che non si vede "
                    "degli strumenti harness_*, che stanno in Python",
    "nova-mcp-cliente": "NOVA che usa un server MCP di qualcun altro: non c'e' "
                        "ancora il posto da cui si configurano quei server",
    "nova-pianificazione": "«quando tocca di nuovo». Oggi a far ripartire le "
                           "attivita' e' l'Utilita' di pianificazione di "
                           "Windows, che sopravvive al riavvio: attaccarlo qui "
                           "vuol dire decidere chi dei due comanda",
}

# ----------------------------------------------------------------- lettura
ELENCO = json.loads((RADICE / "core" / "binari.json").read_text(encoding="utf-8-sig"))
BINARI = [e["nome"] for e in ELENCO["eseguibili"]]

dipendenze: dict[str, list[str]] = {}
di_chi_e: dict[str, str] = {}       # nome del binario -> crate che lo dichiara
for toml in sorted(CRATES.glob("*/Cargo.toml")):
    testo = toml.read_text(encoding="utf-8")
    crate = re.search(r'^\s*name\s*=\s*"([^"]+)"', testo, re.M).group(1)
    dipendenze[crate] = sorted(set(re.findall(r'^(nova-[a-z-]+)\s*=', testo, re.M)))
    for b in re.findall(r"\[\[bin\]\](.*?)(?=\n\[|\Z)", testo, re.S):
        m = re.search(r'name\s*=\s*"([^"]+)"', b)
        if m:
            di_chi_e.setdefault(m.group(1), crate)
    if (toml.parent / "src" / "main.rs").is_file():
        di_chi_e.setdefault(crate, crate)

raggiunti: set[str] = set()


def segui(crate: str) -> None:
    if crate in raggiunti:
        return
    raggiunti.add(crate)
    for d in dipendenze.get(crate, []):
        segui(d)


print("\n=== Chi esegue cosa ===")
senza_crate = [b for b in BINARI if b not in di_chi_e]
controlla("ogni binario consegnato viene da un crate del workspace",
          not senza_crate, " | ".join(senza_crate))
for b in BINARI:
    if b in di_chi_e:
        segui(di_chi_e[b])

print(f"  {len(raggiunti)} crate raggiunti dai {len(BINARI)} binari, "
      f"{len(dipendenze) - len(raggiunti)} no")

print("\n=== E chi non lo esegue nessuno lo dice ===")
scollegati = sorted(set(dipendenze) - raggiunti)
muti = [c for c in scollegati if c not in SCOLLEGATI]
controlla("nessun crate scollegato senza una riga che dica cosa gli manca",
          not muti,
          " | ".join(muti) + "  <- aggiungilo a SCOLLEGATI, con scritto "
          "perche' non lo esegue ancora nessuno" if muti else "")

gia_attaccati = sorted(c for c in SCOLLEGATI if c in raggiunti)
controlla("e nessuno dichiarato scollegato e' gia' attaccato",
          not gia_attaccati,
          " | ".join(gia_attaccati) + "  <- attaccato: togli la riga da "
          "SCOLLEGATI" if gia_attaccati else "")

inesistenti = sorted(c for c in SCOLLEGATI if c not in dipendenze)
controlla("e nessuna riga parla di un crate che non esiste piu'",
          not inesistenti, " | ".join(inesistenti))

controlla("ogni motivo dice qualcosa, non «TODO»",
          all(len(m) > 30 and "TODO" not in m for m in SCOLLEGATI.values()),
          "un motivo vuoto e' una riga che non si rileggera' mai")

if scollegati:
    print("\n  restano da attaccare, in ordine:")
    for c in scollegati:
        print(f"    - {c}: {SCOLLEGATI.get(c, '?')}")

print(f"\n{passati} passati, {len(falliti)} falliti")
sys.exit(1 if falliti else 0)
