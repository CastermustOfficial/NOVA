# -*- coding: utf-8 -*-
"""Il demone fa un turno con un cervello che e' un **programma**, non un URL.

Meta' della scala di NOVA non sta dietro a un indirizzo: `claude`, `gemini`,
`glm` e chiunque altro sia dichiarato in `brains.cli` sono binari che si
lanciano, ricevono un prompt e stampano una risposta (D219). Finora il turno
del demone sapeva parlare solo in HTTP, e a un gradino di quella meta'
rispondeva «non so ancora farlo» — cioe' su una macchina la cui scala
comincia con una CLI il turno in Rust non partiva **mai**, e ogni messaggio
ripiegava sul Python.

Questa prova accende il demone vero con una scala che comincia con una CLI
finta, e guarda le quattro cose che non si vedono da fuori:

1. `agente/pronto` dice **si'**, ed e' quello che decide la strada prima di
   imboccarla: un ripiego dopo aver gia' eseguito degli strumenti li
   eseguirebbe due volte;
2. il prompt che la CLI riceve e' quello intero — istruzioni di sistema,
   domanda dell'utente, scambi di prima — perche' una CLI non ha sessione e
   la continuita' gliela da' NOVA;
3. il `{model}` della configurazione finisce davvero sulla riga di comando;
4. una CLI che nel PATH non c'e' lo dice **prima** di lanciarla, e lo dice
   nel modo giusto: «riavvia NOVA» prima di «installalo» (D193).

Esce 2 — «qui non si puo' provare» — se il demone non e' costruito.
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

NOME = "novad.exe" if os.name == "nt" else "novad"
DEMONE = next((p for p in (RADICE / "core" / "target" / "release" / NOME,
                           RADICE / "core" / "target" / "debug" / NOME)
               if p.is_file()), None)
if DEMONE is None:
    print("Il demone non e' costruito per questo sistema. Per averlo:")
    print("  cd core && cargo build --release --bin novad")
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


def turno(c, **cosa) -> str:
    """Il turno, o il guasto: qui contano tutti e due.

    Un cervello che non parte non e' una risposta triste, e' un errore del
    metodo — come un indirizzo irraggiungibile. Quello che si guarda e'
    **cosa dice**, e quindi qui torna in tutti e due i casi il testo che
    arriverebbe sotto gli occhi dell'utente.
    """
    try:
        return json.dumps(c.request("agente/turno", cosa), ensure_ascii=False)
    except CoreError as e:
        return str(e)

casa = tempfile.mkdtemp(prefix="nova-cli-")
(Path(casa) / "NOVA").mkdir(parents=True, exist_ok=True)

# ------------------------------------------------------- la CLI finta
#
# E' uno script Python lanciato con l'interprete che sta gia' girando, e il
# motivo e' che deve funzionare identica su Windows e altrove: un `.sh` col
# shebang non parte su Windows, un `.cmd` non parte altrove, e una prova che
# gira solo su meta' delle macchine non prova la meta' che conta.
#
# Scrive su un file quello che ha ricevuto: il prompt su stdin e la riga di
# comando. Sono le due cose che da fuori non si vedono e che, sbagliate, non
# danno un errore — danno una CLI che risponde peggio senza saper dire perche'.
TRACCIA = Path(casa) / "ricevuto.jsonl"
FINTA = Path(casa) / "finta_cli.py"
FINTA.write_text(
    "import json, sys\n"
    "prompt = sys.stdin.read()\n"
    "with open(r'" + str(TRACCIA) + "', 'a', encoding='utf-8') as f:\n"
    "    f.write(json.dumps({'argv': sys.argv[1:], 'prompt': prompt},\n"
    "                       ensure_ascii=False) + '\\n')\n"
    "if '--fai-finta-di-morire' in sys.argv:\n"
    "    sys.stderr.write('manca la chiave')\n"
    "    raise SystemExit(1)\n"
    "print('Ho risposto io, la CLI.')\n",
    encoding="utf-8")


def ricevuti() -> list[dict]:
    if not TRACCIA.is_file():
        return []
    return [json.loads(r) for r in TRACCIA.read_text(encoding="utf-8").splitlines() if r]


def configura(spec: dict, brain: str = "finta") -> None:
    (Path(casa) / "NOVA" / "config.json").write_text(json.dumps({
        "system_prompt": "Sei NOVA di prova. Utente: {user}.",
        # Una porta chiusa apposta: se il turno ripiegasse sull'HTTP invece
        # di lanciare il programma, questa prova lo vedrebbe come un guasto
        # di rete invece di passare per sbaglio.
        "server": {"host": "127.0.0.1", "port": 1},
        "model": {"max_tool_iterations": 4},
        "kb": {"vault_path": str(Path(casa) / "vault"), "procedure": False},
        "brains": {
            "active": brain,
            "cli": {"finta": spec},
            "routing": {
                "scala": ["primo"],
                "tiers": {"primo": {"brain": brain, "model": "modello-di-prova"}},
                "escalation_automatica": False,
            },
        },
    }, ensure_ascii=False), encoding="utf-8")


configura({"binary": sys.executable,
           "args": [str(FINTA), "--model", "{model}"],
           "etichetta": "Finta", "timeout": 60})

endpoint = (rf"\\.\pipe\nova-cli-{os.getpid()}" if os.name == "nt"
            else str(Path(casa) / "nova.sock"))
ambiente = dict(os.environ)
ambiente["APPDATA"] = casa
ambiente["HOME"] = casa
ambiente["USERPROFILE"] = casa
ambiente["XDG_CONFIG_HOME"] = casa
ambiente["XDG_RUNTIME_DIR"] = casa

processo = subprocess.Popen(
    [str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
    env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)

try:
    scadenza = time.time() + 20
    while time.time() < scadenza and processo.poll() is None:
        if CoreClient.disponibile(endpoint):
            break
        time.sleep(0.3)
    else:
        fine = (processo.stderr.read() or b"").decode("utf-8", "replace")[-400:]
        print(f"  il demone non ha risposto su {endpoint}: {fine}")
        processo.kill()
        sys.exit(2)

    print("\n1. con una CLI in cima, il turno in Rust si dichiara pronto")
    with CoreClient(endpoint, timeout=90) as c:
        p = c.request("agente/pronto", {})
    controlla("pronto", p.get("pronto") is True,
              json.dumps(p, ensure_ascii=False)[:200])
    controlla("e il gradino e' quello dichiarato", p.get("gradini") == ["primo"],
              str(p.get("gradini")))

    print("\n2. il turno lancia il programma e la sua uscita e' la risposta")
    with CoreClient(endpoint, timeout=90) as c:
        r = c.request("agente/turno", {"testo": "dimmi che ore sono"})
    controlla("il turno finisce con una risposta", r.get("esito") == "risposto",
              json.dumps(r, ensure_ascii=False)[:200])
    controlla("e la risposta e' quella che ha stampato la CLI",
              "Ho risposto io, la CLI." in (r.get("risposta") or ""),
              str(r.get("risposta"))[:150])
    controlla("la CLI e' stata lanciata una volta sola", len(ricevuti()) == 1,
              str(len(ricevuti())))

    print("\n3. il prompt che riceve e' quello intero: non ha sessione")
    primo = ricevuti()[0]
    controlla("ci sono le istruzioni di sistema",
              primo["prompt"].startswith("Sei NOVA di prova."),
              primo["prompt"][:120])
    controlla("con i segnaposto gia' sostituiti", "{user}" not in primo["prompt"],
              primo["prompt"][:120])
    controlla("e la domanda dell'utente, marcata come sua",
              "UTENTE: dimmi che ore sono" in primo["prompt"],
              primo["prompt"][-200:])

    print("\n4. il modello della configurazione arriva sulla riga di comando")
    controlla("il segnaposto e' stato sostituito",
              primo["argv"][-2:] == ["--model", "modello-di-prova"],
              str(primo["argv"]))
    controlla("e non e' rimasto {model} da nessuna parte",
              not any("{model}" in a for a in primo["argv"]), str(primo["argv"]))

    print("\n5. la conversazione resta, ed e' NOVA a ridargliela")
    with CoreClient(endpoint, timeout=90) as c:
        c.request("agente/turno", {"testo": "e invece adesso che giorno e'"})
    secondo = ricevuti()[1]
    controlla("nel secondo prompt c'e' anche il primo giro",
              "UTENTE: dimmi che ore sono" in secondo["prompt"],
              secondo["prompt"][-300:])
    controlla("e cosa aveva risposto la CLI, marcato come suo",
              "ASSISTENTE: Ho risposto io, la CLI." in secondo["prompt"],
              secondo["prompt"][-300:])

    print("\n6. un programma che non c'e' lo dice prima di lanciarlo")
    configura({"binary": "non-esiste-questo-programma-qui"})
    with CoreClient(endpoint, timeout=90) as c:
        p = c.request("agente/pronto", {})
        detto = turno(c, testo="ci provi lo stesso?", sessione="altra")
    controlla("non pronto", p.get("pronto") is False, json.dumps(p)[:200])
    # «Installalo» da solo manderebbe a reinstallare una cosa che c'e' gia':
    # un processo eredita il PATH da quando e' partito, e una CLI installata
    # mentre NOVA gira resta invisibile finche' NOVA non riparte (D193).
    controlla("e il motivo dice prima di riavviare, poi di installare",
              "riavvia NOVA" in (p.get("perche") or ""), str(p.get("perche")))
    controlla("il turno non finge di aver risposto",
              "non-esiste-questo-programma-qui" in detto, detto[:250])
    controlla("e dice anche da dove si toglie", "brains.cli" in detto, detto[:250])

    print("\n7. una CLI che non stampa niente e' un guasto, non una risposta vuota")
    configura({"binary": sys.executable,
               "args": [str(FINTA), "--fai-finta-di-morire"],
               "etichetta": "Finta"})
    with CoreClient(endpoint, timeout=90) as c:
        detto = turno(c, testo="e qui cosa succede", sessione="terza")
    controlla("il guasto arriva fino all'utente", "Finta" in detto, detto[:250])
    controlla("con dentro quel che la CLI ha scritto su stderr",
              "manca la chiave" in detto,
              "buttare stderr lascerebbe un errore che non dice niente: "
              + detto[:250])

finally:
    processo.terminate()
    try:
        processo.wait(timeout=10)
    except subprocess.TimeoutExpired:
        processo.kill()

print(f"\n{passati} controlli passati, {len(falliti)} falliti")
if falliti:
    print("::error::test_demone_cli: " + "; ".join(falliti))
    sys.exit(1)
sys.exit(0)
