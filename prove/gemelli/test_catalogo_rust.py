# -*- coding: utf-8 -*-
"""Il verdetto sul modello deve essere identico in Rust e in Python.

Decimo pezzo del cantiere, e quello con il criterio d'ordine piu' chiaro:
`catalogo.py` lo chiama **l'installatore**, che gira prima che le dipendenze
del progetto esistano. In Python era di sola libreria standard proprio per
questo; in Rust il problema sparisce, perche' e' un binario.

Cosa decide: se offrire lo scaricamento di un modello, quale variante, e cosa
dire mentre lo si fa. Sbagliarlo in un verso sono tredici gigabyte scaricati
per un programma che si apre una volta e mai piu'; sbagliarlo nell'altro e'
un rifiuto a qualcuno che poteva usarlo benissimo.

Si confronta anche il **testo** dei motivi, non solo la decisione: quelle
frasi le legge l'utente mentre installa, e sono la differenza fra «non te lo
faccio scaricare» e «non te lo faccio scaricare, ecco perche' e cosa puoi
fare invece».

Il catalogo vero (`models.json`) e' fra i casi, non solo dati inventati: e'
quello su cui gira davvero l'installatore.

Esce 2 se il binario non e' costruito.
"""
import itertools
import json
import os
import subprocess
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "nova-catalogo.exe" if os.name == "nt" else "nova-catalogo"
BINARIO = RADICE / "core" / "target" / "release" / NOME
if not BINARIO.is_file():
    print("Il binario Rust non e' costruito per questo sistema. Per averlo:")
    print("  cd core && cargo build --release -p nova-catalogo")
    sys.exit(2)

from nova import catalogo as py                               # noqa: E402

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


def rust(domande: list[dict]) -> list[dict]:
    dentro = "\n".join(json.dumps(d, ensure_ascii=False) for d in domande)
    p = subprocess.run([str(BINARIO)], input=dentro, capture_output=True,
                       text=True, encoding="utf-8", timeout=120)
    if p.returncode != 0:
        print("il binario e' uscito male:", p.stderr[:400])
        sys.exit(1)
    return [json.loads(r) for r in p.stdout.splitlines() if r.strip()]


def python(fam: dict, vram: float, ram: float, cat: dict | None) -> dict:
    v = py.verdetto(fam, vram, py.soglia(cat) if cat else None, ram)
    return {
        "si_scarica": v.si_scarica,
        "file": (v.variante or {}).get("file"),
        "gb": (v.variante or {}).get("gb"),
        "motivo": v.motivo,
        "suggerimento": v.suggerimento,
        "in_cpu": v.in_cpu,
    }


# --- le famiglie: quella vera piu' due estremi -----------------------------
CATALOGO = json.loads((RADICE / "models.json").read_text(encoding="utf-8"))
FAMIGLIE = list(CATALOGO["famiglie"])
FAMIGLIE.append({                     # un MoE leggerissimo
    "id": "bonsai", "nome": "Bonsai 27B", "frazione_letta": 0.11,
    "varianti": [{"file": "bonsai-q1.gguf", "gb": 3.8, "vram_gb": 5}],
})
FAMIGLIE.append({                     # senza frazione dichiarata: denso
    "id": "ignoto", "nome": "Modello Ignoto",
    "varianti": [{"file": "x.gguf", "gb": 9.0, "vram_gb": 10}],
})
FAMIGLIE.append({"id": "vuota", "nome": "Famiglia Vuota", "varianti": []})

VRAM = [0, 4, 6, 8, 12, 16, 24, 28, 48]
RAM = [0, 8, 12, 16, 32, 64]

print(f"\n=== Il verdetto, {len(FAMIGLIE)} famiglie x {len(VRAM)} VRAM x {len(RAM)} RAM ===")
domande, attese = [], []
for fam, vram, ram in itertools.product(FAMIGLIE, VRAM, RAM):
    domande.append({"famiglia": fam, "vram_gb": vram, "ram_gb": ram,
                    "catalogo": CATALOGO})
    attese.append(python(fam, vram, ram, CATALOGO))

risposte = rust(domande)
controlla("il binario risponde a tutte le domande",
          len(risposte) == len(domande), f"{len(risposte)} su {len(domande)}")

campi = ("si_scarica", "file", "gb", "in_cpu", "motivo", "suggerimento")
diverse = []
for d, r, p in zip(domande, risposte, attese):
    for c in campi:
        # il Rust omette file/gb quando non c'e' variante: None e assente
        # sono la stessa cosa, e vanno confrontati come tali
        a, b = r.get(c), p.get(c)
        if (a or None) != (b or None):
            diverse.append(
                f"{d['famiglia']['id']} vram={d['vram_gb']} ram={d['ram_gb']} "
                f"campo {c}: rust={a!r} python={b!r}")
            break
controlla(f"tutti i {len(domande)} verdetti coincidono, testo compreso",
          not diverse, " | ".join(diverse[:3]))


print("\n=== E i comportamenti attesi, decisi a mano ===")
# Non chiesti a nessuna delle due implementazioni: sono le regole per cui
# questo modulo e' stato scritto.
qwen = next(f for f in CATALOGO["famiglie"] if "Qwen" in (f.get("nome") or ""))
r_qwen_senza = rust([{"famiglia": qwen, "vram_gb": 0, "ram_gb": 32,
                      "catalogo": CATALOGO}])[0]
controlla("un denso da 27B non si offre senza scheda video",
          not r_qwen_senza["si_scarica"], r_qwen_senza.get("motivo", "")[:90])
controlla("e il rifiuto dice perché, non solo no",
          "legge almeno" in r_qwen_senza.get("motivo", "")
          and bool(r_qwen_senza.get("suggerimento")))

bonsai = FAMIGLIE[-3]
r_bonsai = rust([{"famiglia": bonsai, "vram_gb": 0, "ram_gb": 16,
                  "catalogo": CATALOGO}])[0]
controlla("un MoE leggero si offre anche senza scheda video",
          r_bonsai["si_scarica"] and r_bonsai["in_cpu"],
          r_bonsai.get("motivo", "")[:90])
controlla("e lo dice prima che girerà sul processore",
          "processore" in r_bonsai.get("motivo", ""))

r_vuota = rust([{"famiglia": FAMIGLIE[-1], "vram_gb": 0, "ram_gb": 32,
                 "catalogo": CATALOGO}])[0]
controlla("una famiglia senza varianti non fa esplodere niente",
          r_vuota["si_scarica"] is False)

r_niente = subprocess.run([str(BINARIO)], input="", capture_output=True,
                          text=True, encoding="utf-8", timeout=60)
controlla("e nemmeno una domanda vuota", r_niente.returncode == 0
          and json.loads(r_niente.stdout)["si_scarica"] is False,
          r_niente.stdout[:100])

print("\n=== La forma giusta di dire «non farlo» è non offrirlo ===")
# Il motivo per cui il modulo esiste: mai «si può fare ma andrà piano» su un
# modello che in pratica non si userebbe.
brutti = [r for r in risposte
          if r["si_scarica"] and not r.get("in_cpu")
          and "piano" in (r.get("motivo") or "")]
controlla("nessun verdetto dice «scaricalo, ma andrà piano»", not brutti,
          str(brutti[:1]))

print("\n=== Quello che ha trovato solo l'integrazione ===")
# Due difetti che nessuna prova di unita' avrebbe visto, perche' stanno nel
# punto in cui il binario incontra chi lo chiama — e chi lo chiama e'
# PowerShell.

# 1. Il BOM. `Set-Content -Encoding UTF8` su Windows PowerShell 5.1 mette tre
# byte davanti al primo `{`. serde si ferma con «expected value at line 1
# column 1»: vero, e inutile. Non e' sciatteria di chi chiama, e' l'ambiente
# in cui questo binario deve funzionare.
con_bom = "\ufeff" + json.dumps(
    {"famiglia": qwen, "vram_gb": 0, "ram_gb": 32, "catalogo": CATALOGO},
    ensure_ascii=False)
r = subprocess.run([str(BINARIO)], input=con_bom, capture_output=True,
                   text=True, encoding="utf-8", timeout=60)
d = json.loads(r.stdout) if r.stdout.strip() else {}
controlla("un ingresso col BOM viene capito lo stesso",
          "Qwen" in (d.get("motivo") or ""), (d.get("motivo") or "")[:90])

# 2. Una domanda illeggibile deve **dirlo**. Prima diventava una famiglia
# vuota, e la famiglia vuota produceva un verdetto perfettamente formato —
# «legge almeno 0.0 GB per token, non te lo faccio scaricare» — che sembra
# una risposta e non lo e'. Uno strumento che riesce senza consegnare niente
# e' peggio di uno che manca: l'installatore avrebbe rifiutato ogni modello
# dando all'utente una ragione inventata.
r = subprocess.run([str(BINARIO)], input="{non sono json}",
                   capture_output=True, text=True, encoding="utf-8", timeout=60)
d = json.loads(r.stdout)
controlla("una domanda illeggibile non diventa un verdetto inventato",
          not d["si_scarica"] and "Non ho capito" in d["motivo"], d["motivo"][:90])
controlla("e non pretende di aver misurato niente",
          "0.0 GB" not in d["motivo"], d["motivo"][:90])

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
