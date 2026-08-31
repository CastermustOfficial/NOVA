# -*- coding: utf-8 -*-
"""Senza scheda video non si scarica un modello che non si potra' usare.

D41 e D42. L'installatore, quando la VRAM non si legge, scaricava comunque la
variante piu' leggera della famiglia consigliata e ci scriveva accanto «su
questa macchina andra' piano». Coi numeri misurati quella riga e' una bugia
gentile: per un denso da 27B non e' piu' piano, sono tredici gigabyte
scaricati per un programma che non si apre piu'. E chi installa clicca
avanti, perche' gli abbiamo appena detto che si puo' fare.

La regola non e' pero' «senza GPU mai», che sarebbe altrettanto sbagliata:
un modello leggero **da leggere** funziona benissimo sul processore. Cio' che
decide e' quanti byte si leggono per token — parametri che si accendono per i
bit che ciascuno occupa — e non i gigabyte del file ne' i parametri totali.

Le due misure da cui esce la soglia, stessa macchina, zero layer su GPU:

    Qwen3.8 27B Q4_K_M (denso)     15,7 GB/token   1,8 tok/s
    Gemma 4 26B-A4B Q3 (MoE)       ~3,7 GB/token   7,6 tok/s

Questa prova non esegue `install.ps1` — mai, per nessun motivo: quello script
agisce sul sistema, e una volta l'ho imparato nel modo peggiore. Prova la
funzione che decide, e poi controlla per lettura che l'installatore la usi.
"""
import json
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from nova.catalogo import (                                    # noqa: E402
    MARGINE_RAM_GB, SOGLIA_GB_PER_TOKEN, gb_per_token,
    piu_grande_che_entra, varianti_senza_gpu, verdetto,
)

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


CATALOGO = json.loads((RADICE / "models.json").read_text(encoding="utf-8-sig"))

# Due famiglie finte, che sono i due casi veri misurati.
DENSO = {
    "id": "finto-denso", "nome": "Denso 27B", "frazione_letta": 1.0,
    "varianti": [
        {"file": "d-q6.gguf", "gb": 25.3, "vram_gb": 28},
        {"file": "d-q4.gguf", "gb": 15.7, "vram_gb": 18},
        {"file": "d-q3.gguf", "gb": 12.0, "vram_gb": 14},
    ],
}
MOE = {
    "id": "finto-moe", "nome": "MoE 26B-A4B", "frazione_letta": 0.31,
    "varianti": [
        {"file": "m-q4.gguf", "gb": 15.0, "vram_gb": 17},
        {"file": "m-q3.gguf", "gb": 12.0, "vram_gb": 14},
    ],
}

print("\n=== I byte letti per token ===")
# Per un denso si legge tutto il file a ogni token.
controlla("un denso legge tutto il file",
          gb_per_token(DENSO, DENSO["varianti"][1]) == 15.7)
# Per un MoE solo la parte che si accende, piu' cio' che si rilegge sempre.
g = gb_per_token(MOE, MOE["varianti"][1])
controlla("un MoE legge una frazione", abs(g - 3.72) < 0.01, str(g))
controlla("e la frazione non e' quella dei parametri attivi",
          g > 12.0 * 0.15,
          "attenzione e strati condivisi si rileggono comunque a ogni token")

print("\n=== La soglia ===")
# Ricavata dalle due misure: 15,7 GB/token danno 1,8 tok/s, 3,7 ne danno 7,6.
controlla("il denso da 27B non passa",
          gb_per_token(DENSO, DENSO["varianti"][1]) > SOGLIA_GB_PER_TOKEN)
controlla("nemmeno la sua variante piu' leggera",
          gb_per_token(DENSO, DENSO["varianti"][-1]) > SOGLIA_GB_PER_TOKEN,
          "e' il caso che l'installatore scaricava lo stesso")
controlla("il MoE passa",
          gb_per_token(MOE, MOE["varianti"][1]) <= SOGLIA_GB_PER_TOKEN)

print("\n=== Cosa si offre, e cosa no ===")
v = verdetto(DENSO, vram_gb=0)
controlla("senza VRAM un denso non si offre", not v.si_scarica)
controlla("e il motivo si puo' dire a una persona",
          v.motivo and "token" in v.motivo.lower(), repr(v.motivo))
controlla("e non si resta senza una strada", bool(v.suggerimento), repr(v.suggerimento))

v = verdetto(MOE, vram_gb=0)
controlla("senza VRAM un MoE leggero si offre", v.si_scarica)
controlla("e si offre la piu' grande che regge",
          v.variante and v.variante["file"] == "m-q4.gguf",
          str(v.variante))
controlla("dicendo che girera' sul processore", v.in_cpu and "processore" in v.motivo,
          repr(v.motivo))

print("\n=== E deve anche starci, in RAM ===")
# Questa sezione l'ha trovata una prova che sbagliava per il motivo giusto.
# La velocita' non basta: sulla GPU il file sta in VRAM, sul processore sta in
# RAM — tutto, e accanto ci vanno il sistema, il browser e NOVA. Un modello
# abbastanza veloce ma troppo grande entra sulla carta e in pratica manda la
# macchina a paginare su disco.
controlla("con 32 GB di RAM la piu' grande va bene",
          verdetto(MOE, vram_gb=0, ram_gb=32).variante["file"] == "m-q4.gguf")
controlla("con 16 GB si scende a quella che ci sta",
          verdetto(MOE, vram_gb=0, ram_gb=16).variante["file"] == "m-q3.gguf",
          str(verdetto(MOE, vram_gb=0, ram_gb=16).variante))
stretto = verdetto(MOE, vram_gb=0, ram_gb=8)
controlla("con 8 GB non si offre niente", not stretto.si_scarica)
controlla("e il motivo parla di RAM, non di lentezza",
          "RAM" in stretto.motivo, repr(stretto.motivo))
controlla("il margine per il sistema e' dichiarato", MARGINE_RAM_GB > 0)
controlla("senza sapere la RAM si guarda solo la velocita'",
          len(varianti_senza_gpu(MOE)) == 2
          and len(varianti_senza_gpu(MOE, ram_gb=16)) == 1)

print("\n=== Con la scheda video non cambia niente ===")
v = verdetto(DENSO, vram_gb=24)
controlla("con 24 GB il denso si offre", v.si_scarica)
controlla("e si prende la piu' grande che ENTRA",
          v.variante and v.variante["file"] == "d-q4.gguf", str(v.variante))
controlla("la regola vecchia e' ancora quella",
          piu_grande_che_entra(DENSO, 14)["file"] == "d-q3.gguf")
controlla("e se non entra niente, niente",
          piu_grande_che_entra(DENSO, 4) is None)

print("\n=== Il catalogo vero ===")
for f in CATALOGO["famiglie"]:
    controlla(f"{f['id']}: dichiara la frazione letta",
              isinstance(f.get("frazione_letta"), (int, float))
              and 0 < f["frazione_letta"] <= 1.0,
              str(f.get("frazione_letta")))
    controlla(f"{f['id']}: ogni variante ha gb e vram_gb",
              all("gb" in v and "vram_gb" in v for v in f["varianti"]))

consigliate = [f for f in CATALOGO["famiglie"] if f.get("consigliata")]
controlla("c'e' una sola famiglia consigliata", len(consigliate) == 1,
          str([f["id"] for f in consigliate]))

# Il caso che ha fatto nascere tutto questo, sul catalogo vero.
if consigliate:
    v = verdetto(consigliate[0], vram_gb=0)
    print(f"  (senza GPU, {consigliate[0]['id']}: "
          f"{'si offre' if v.si_scarica else 'non si offre'})")

print("\n=== E l'installatore la usa davvero ===")
# Non si esegue install.ps1: si legge. Quello script agisce sul sistema.
ps = (RADICE / "install.ps1").read_text(encoding="utf-8-sig")
controlla("install.ps1 chiede il verdetto a nova.catalogo",
          "nova.catalogo" in ps or "Verdetto-Modello" in ps)
controlla("e non scarica piu' la piu' leggera per ripiego",
          "Scarico la variante piu' leggera" not in ps,
          "c'e' ancora la riga che scaricava comunque")

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
