# -*- coding: utf-8 -*-
"""`novad --consiglio`: la scala consigliata per quello che c'e' (D379).

Deciso con Gio il 7 ottobre: NOVA consiglia la scala per quello che l'utente
ha davvero (solo Gemini: Flash e poi Pro; solo Claude: Haiku e poi Opus;
tutti e due: Flash e poi Opus; il modello sul PC, se c'e', orchestra) e non
la applica mai da sola. Qui il demone gira in una casa finta, con un catalogo
dei modelli scritto a mano e un «modello» che e' un file qualunque, e si
guarda cosa stampa — e che il file della configurazione resti com'era.

Esce 2 — «qui non si puo' provare» — se il demone non e' costruito.
"""
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "novad.exe" if os.name == "nt" else "novad"
DEMONE = next((p for p in (RADICE / "core" / "target" / "release" / NOME,
                           RADICE / "core" / "target" / "debug" / NOME)
               if p.is_file()), None)
if DEMONE is None:
    print("Il demone non e' costruito per questo sistema. Per averlo:")
    print("  cd core && cargo build --release -p novad")
    sys.exit(2)

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


casa = tempfile.mkdtemp(prefix="nova-consiglio-")
ambiente = dict(os.environ)
ambiente.update({"APPDATA": casa, "HOME": casa, "USERPROFILE": casa,
                 "XDG_CONFIG_HOME": casa, "XDG_RUNTIME_DIR": casa})
cartella = Path(casa) / "NOVA"
cartella.mkdir()
modello = Path(casa) / "finto.gguf"
modello.write_bytes(b"GGUF")
CONFIG = {"server": {"model_path": str(modello)},
          "brains": {"routing": {"orchestratore": "rapido", "scala": ["rapido"],
                                 "tiers": {"rapido": {"brain": "claude",
                                                      "model": "ultimo:sonnet"}}}}}
(cartella / "config.json").write_text(json.dumps(CONFIG), encoding="utf-8")
prima = (cartella / "config.json").read_bytes()


def consiglio() -> dict:
    r = subprocess.run([str(DEMONE), "--consiglio"], capture_output=True, text=True,
                       encoding="utf-8", env=ambiente, timeout=60)
    if r.returncode != 0:
        return {"errore": r.stderr[-300:]}
    return json.loads(r.stdout)


print("\n1. senza catalogo non si sa ancora cosa c'e'")
c = consiglio()
controlla("lo dice", c.get("conosciuto") is False, str(c)[:200])
controlla("ma il modello sul PC lo vede gia'", c.get("disponibili", {}).get("locale") is True)

print("\n2. Claude Code e Antigravity: il modello sul PC, poi Flash, poi Opus")
(cartella / "modelli.json").write_text(json.dumps({
    "quando": 1, "claude": {"opus": "claude-opus-5-5"},
    "elenchi": {"agy": ["gemini-3.8-flash-high", "gemini-3.1-pro-high"]},
    "disponibili": {"claude": True, "antigravity": True}}), encoding="utf-8")
c = consiglio()
righe = [(g["nome"], g["brain"], g["scelto"]) for g in c.get("righe", [])]
controlla("la scala consigliata", righe == [
    ("locale", "locale", "finto.gguf"),
    ("rapido", "antigravity", "gemini-3.8-flash-high"),
    ("difficile", "claude", "claude-opus-5-5")], str(righe))
controlla("orchestra il modello sul PC", c["consigliata"]["orchestratore"] == "locale")
controlla("e non e' quella in uso", c.get("coincide") is False)
controlla("quella in uso si vede com'e'",
          [(g["nome"], g["scelto"]) for g in c.get("attuale", [])] == [("rapido", "sonnet")],
          str(c.get("attuale")))

print("\n3. solo Claude: Haiku e poi Opus")
(cartella / "modelli.json").write_text(json.dumps({
    "quando": 1, "disponibili": {"claude": True, "antigravity": False}}), encoding="utf-8")
modello.unlink()
c = consiglio()
controlla("senza il file il modello sul PC non c'e'", c["disponibili"]["locale"] is False)
controlla("Haiku e poi Opus, e orchestra il rapido",
          [(g["nome"], g["model"]) for g in c["righe"]]
          == [("rapido", "ultimo:haiku"), ("difficile", "ultimo:opus")]
          and c["consigliata"]["orchestratore"] == "rapido", str(c["righe"]))

print("\n4. consigliare non cambia niente")
controlla("il file della configurazione e' quello di prima",
          (cartella / "config.json").read_bytes() == prima)
# La copia la fa solo il pannello, quando si preme «Conferma» (D390).
controlla("e accanto non c'e' nessuna copia, perche' non si e' scritto",
          not (cartella / "config.json.prima-della-scala").exists()
          and not (cartella / "config.json.prima-del-consiglio").exists())

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
if falliti:
    print("::error::test_demone_consiglio: " + "; ".join(falliti))
sys.exit(1 if falliti else 0)
