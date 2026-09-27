# -*- coding: utf-8 -*-
"""Un compito pianificato fa il suo turno senza Python (D345).

All'ora giusta l'Utilita' di pianificazione lancia `nova chiedi --accendi
--sessione compiti --da-file <file>`. Qui si prova quel giro dall'inizio,
senza aspettare nessun orario e senza Windows:

1. il demone e' spento, e `nova chiedi --accendi` lo accende;
2. la domanda arriva dal file intera — apostrofi, virgolette, accenti,
   a capo — che e' il motivo per cui sta in un file (D149);
3. la risposta del cervello torna su stdout;
4. il compito ha la sua conversazione, non entra nel filo della chat;
5. fuori da Windows gli strumenti del tempo dicono perche' non possono,
   invece di fallire con un errore del sistema.

Il cervello e' una CLI finta, come in test_demone_cli. Esce 2 se il demone o
la riga di comando non sono costruiti.
"""
import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

esegue = lambda n: n + ".exe" if os.name == "nt" else n
CARTELLA = RADICE / "core" / "target" / "release"
NOVA, DEMONE = CARTELLA / esegue("nova"), CARTELLA / esegue("novad")
if not (NOVA.is_file() and DEMONE.is_file()):
    print("Il demone o la riga di comando non sono costruiti. Per averli:")
    print("  cd core && cargo build --release -p novad -p nova-cli")
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


from nova.core_client import CoreClient, CoreError               # noqa: E402

casa = Path(tempfile.mkdtemp(prefix="nova-compiti-"))
(casa / "NOVA").mkdir(parents=True)
TRACCIA = casa / "ricevuto.jsonl"
FINTA = casa / "finta_cli.py"
FINTA.write_text(
    "import json, sys\n"
    "prompt = sys.stdin.read()\n"
    "with open(r'" + str(TRACCIA) + "', 'a', encoding='utf-8') as f:\n"
    "    f.write(json.dumps({'prompt': prompt}, ensure_ascii=False) + '\\n')\n"
    "print('Agenda controllata.')\n", encoding="utf-8")
(casa / "NOVA" / "config.json").write_text(json.dumps({
    "system_prompt": "Sei NOVA di prova.",
    "server": {"host": "127.0.0.1", "port": 1},
    "model": {"max_tool_iterations": 2},
    "kb": {"vault_path": str(casa / "vault"), "procedure": False},
    "brains": {
        "active": "finta",
        "cli": {"finta": {"binary": sys.executable, "args": [str(FINTA)],
                          "etichetta": "Finta", "timeout": 60}},
        "routing": {"scala": ["primo"], "tiers": {"primo": {"brain": "finta", "model": "x"}},
                    "escalation_automatica": False},
    },
}), encoding="utf-8")

endpoint = (rf"\\.\pipe\nova-compiti-{os.getpid()}" if os.name == "nt"
            else str(casa / "nova.sock"))
ambiente = dict(os.environ)
for k in ("APPDATA", "HOME", "USERPROFILE", "XDG_CONFIG_HOME", "XDG_RUNTIME_DIR"):
    ambiente[k] = str(casa)

domanda = "Controlla l'agenda di \"domani\"\ne dimmi cosa c'è."
file_domanda = casa / "compito.txt"
file_domanda.write_text(domanda, encoding="utf-8")

print("\n1. il demone e' spento, e il compito lo accende")
controlla("prima il demone non c'e'", not CoreClient.disponibile(endpoint))
try:
    r = subprocess.run([str(NOVA), "--endpoint", endpoint, "chiedi", "--accendi",
                        "--sessione", "compiti", "--da-file", str(file_domanda)],
                       env=ambiente, capture_output=True, text=True, encoding="utf-8",
                       errors="replace", timeout=120)
    controlla("il compito finisce bene", r.returncode == 0, (r.stderr or r.stdout)[-400:])
    controlla("e dopo il demone c'e'", CoreClient.disponibile(endpoint))

    print("\n2. la domanda arriva intera, dal file")
    prompt = "\n".join(x["prompt"] for x in
                       (json.loads(l) for l in TRACCIA.read_text(encoding="utf-8").splitlines() if l)) \
        if TRACCIA.is_file() else ""
    controlla("apostrofi, virgolette, accenti e a capo compresi", domanda in prompt, prompt[-300:])

    print("\n3. la risposta torna su stdout")
    controlla("e' quella del cervello", r.stdout.strip() == "Agenda controllata.", repr(r.stdout))

    with CoreClient(endpoint, timeout=60) as c:
        print("\n4. il compito ha la sua conversazione")
        aperte = c.request("agente/sessioni").get("aperte", [])
        controlla("«compiti», non quella della chat", "compiti" in aperte, str(aperte))

        print("\n5. gli strumenti del tempo")
        nomi = {t["name"] for t in c.request("tools/list")["tools"]}
        controlla("un modello li vede",
                  {"sys_promemoria", "sys_pianifica", "sys_pianificate", "sys_pianifica_togli"} <= nomi)
        try:
            c.call("sys.pianifica_togli", {"nome": "Aggiornamento di Windows"})
            controlla("non si toglie un'attivita' che non e' di NOVA", False, "l'ha tolta")
        except CoreError as e:
            controlla("non si toglie un'attivita' che non e' di NOVA", "solo le attivita'" in str(e), str(e))
        try:
            c.call("sys.promemoria", {"message": "x", "when": "2020-01-01 10:00"})
            controlla("un promemoria nel passato si rifiuta", False)
        except CoreError as e:
            controlla("un promemoria nel passato si rifiuta", "gia' passato" in str(e), str(e))
        if os.name != "nt":
            try:
                c.call("sys.pianifica", {"istruzione": "x", "quando": "10:00"})
                controlla("fuori da Windows lo dice", False)
            except CoreError as e:
                controlla("fuori da Windows lo dice", "solo su Windows" in str(e)
                          or "non e' costruita" in str(e), str(e))
finally:
    try:
        with CoreClient(endpoint, timeout=10) as c:
            c.request("daemon/shutdown")
    except Exception:
        pass

print(f"\n{passati} controlli passati, {len(falliti)} falliti")
if falliti:
    print("::error::test_demone_compiti: " + "; ".join(falliti))
    sys.exit(1)
sys.exit(0)
