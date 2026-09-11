# -*- coding: utf-8 -*-
"""Cosa NOVA dice a un cervello che vive fuori, e che dev'essere lo stesso.

CANT-3. Un cervello esterno riceve tre cose, e in tutte e tre sbagliare **non
da' un errore**:

1. una **riga di comando** — dove l'ordine degli argomenti decide se il
   cervello avra' le sue capacita' o no, e dove un elenco di permessi
   spezzato in piu' argomenti si perde per strada;
2. un **prompt di sistema** — l'identita', le istruzioni operative, e la
   memoria;
3. un **payload JSON** — dove una chiave in piu' manda in errore un'API e una
   in meno fa girare il modello locale con dei valori che non sono quelli
   scelti.

Piu' le decisioni piccole attorno: quanto aspettare quando il fornitore dice
«riprova», quali intestazioni si mandano, come si racconta lo stato.

Si confrontano contro il **Python vero**, chiamandone le funzioni con degli
oggetti finti al posto di quelli che toccano il disco: cosi' quello che si
misura e' il codice che gira, non una sua copia.

Esce 2 se il banco non e' costruito.
"""
import json
import os
import subprocess
import sys
from pathlib import Path
from types import SimpleNamespace

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "banco-cervelli.exe" if os.name == "nt" else "banco-cervelli"
BINARIO = RADICE / "core" / "target" / "release" / NOME
if not BINARIO.is_file():
    print("Il banco Rust non e' costruito per questo sistema. Per averlo:")
    print("  cd core && .\\x.cmd build --release -p nova-cervelli "
          "--features banco --bin banco-cervelli")
    sys.exit(2)

from nova.brains import claude_cli, cli_generic, openai_compat  # noqa: E402

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


def rust(dentro: dict) -> dict:
    p = subprocess.run([str(BINARIO)], input=json.dumps(dentro, ensure_ascii=False),
                       capture_output=True, text=True, encoding="utf-8",
                       errors="replace", timeout=120)
    if p.returncode != 0:
        print(f"  il banco Rust si e' fermato: {p.stderr.strip()[:300]}")
        sys.exit(1)
    return json.loads(p.stdout)


def finto(**campi):
    return SimpleNamespace(**campi)


def cfg_di(autonomia="ask_risky"):
    return finto(safety=finto(autonomy=autonomia))


# ------------------------------------------------------------------ scenari
SISTEMA_LUNGO = "riga\n" * 60
CLAUDE = [
    # Sessione nuova, senza MCP: il prompt viaggia in linea.
    dict(eseguibile="claude.cmd", model="sonnet", autonomia="ask_risky",
         sistema="il prompt"),
    # Sessione nuova con MCP: le opzioni MCP devono venire prima del prompt.
    dict(eseguibile="claude.cmd", model="sonnet", autonomia="ask_risky",
         mcp_config="C:\\Users\\x\\mcp.json", sistema=SISTEMA_LUNGO),
    # Mani libere: niente sportello dei permessi.
    dict(eseguibile="claude.cmd", model="opus", autonomia="autonomous",
         mcp_config="C:\\mcp.json", sistema="x"),
    # Conferma sempre.
    dict(eseguibile="claude.cmd", model="sonnet", autonomia="always_ask",
         mcp_config="C:\\mcp.json", sistema="x"),
    # Un livello che non esiste: non deve dare le mani libere.
    dict(eseguibile="claude.cmd", model="sonnet", autonomia="mai_visto",
         sistema="x"),
    # Sessione ripresa: niente prompt di sistema.
    dict(eseguibile="claude.cmd", model="sonnet", autonomia="ask_risky",
         session_id="s-123", mcp_config="C:\\mcp.json", sistema="x"),
    # Con tetto ai turni e argomenti aggiunti dall'utente.
    dict(eseguibile="claude.cmd", model="sonnet", autonomia="ask_risky",
         max_turns=12, extra_args=["--verbose", "--foo"], sistema="x"),
    # Zero turni e' «nessun tetto», non «zero turni».
    dict(eseguibile="claude.cmd", model="sonnet", autonomia="ask_risky",
         max_turns=0, sistema="x"),
]

MESSAGGI = [
    [],
    [{"role": "user", "contenuto": "x"}],
    [{"role": "system", "content": "regola una"},
     {"role": "user", "content": "domanda"},
     {"role": "system", "content": "regola due"},
     {"role": "assistant", "content": "risposta"},
     {"role": "user", "content": "seconda domanda"}],
    [{"role": "system", "content": "   "}, {"role": "user", "content": "solo io"}],
    [{"role": "assistant", "content": "nessun utente"}],
]

PROMPT = [
    dict(messaggi=MESSAGGI[2], utente="gio", home="C:\\Users\\gio", vault="", con_mcp=False),
    dict(messaggi=MESSAGGI[2], utente="gio", home="C:\\Users\\gio",
         vault="C:\\Users\\gio\\NOVA\\vault", con_mcp=True),
    dict(messaggi=MESSAGGI[2], utente="gio", home="C:\\Users\\gio",
         vault="C:\\Users\\gio\\NOVA\\vault", con_mcp=False),
    dict(messaggi=MESSAGGI[3], utente="perché", home="C:\\città", vault="", con_mcp=False),
    dict(messaggi=[], utente="gio", home="/home/gio", vault="/v", con_mcp=True),
]

STATO_CLAUDE = [
    ["sonnet", "abbonamento", "Max", 1.2349],
    ["sonnet", "abbonamento", "", 0.0],
    ["sonnet", "abbonamento", "Pro", 0.0],
    ["opus", "chiave", "", 0.45678],
    ["opus", "chiave", "", 0.0],
    # Il mezzo esatto: `.2f` di Python arrotonda al pari, non per eccesso.
    ["sonnet", "abbonamento", "Max", 0.125],
]

AUTONOMIE = ["always_ask", "ask_risky", "autonomous", "mai_visto", ""]

PAYLOAD = [
    dict(model="m", messaggi=MESSAGGI[2], tools=[], temperature=0.7, top_p=0.9,
         max_tokens=2048, top_k=None),
    dict(model="", messaggi=MESSAGGI[2], tools=[{"type": "function",
                                                 "function": {"name": "x"}}],
         temperature=0.7, top_p=0.9, max_tokens=2048, top_k=None),
    dict(model="qwen", messaggi=MESSAGGI[2], tools=[], temperature=0.0, top_p=1.0,
         max_tokens=1, top_k=40),
    dict(model="m", messaggi=[{"role": "user", "content": "perché è così \U0001f9ea"}],
         tools=[], temperature=0.7, top_p=0.9, max_tokens=10, top_k=None),
]

SEMPLICI = [["m", "domanda", 600], ["", "perché", 1], ["m", "con \"virgolette\"", 900]]

ATTESE = [None, "120", "1", "99999", "", "Wed, 21 Oct 2026 07:28:00 GMT", "60.7", "-5"]
CHIAVI = ["", "sk-abc", "   "]
STATO_LOCALE = ["", "C:\\modelli\\qwen.gguf", "qwen.gguf", "C:\\"]

CLI_PROMPT = [
    dict(messaggi=MESSAGGI[2], contesto_kb=""),
    dict(messaggi=MESSAGGI[2], contesto_kb="abita a Napoli"),
    dict(messaggi=[{"role": "user", "content": f"d{i}"} for i in range(9)],
         contesto_kb=""),
    dict(messaggi=[{"role": "user", "content": "  "},
                   {"role": "user", "content": "c'e'"}], contesto_kb=""),
    dict(messaggi=[], contesto_kb="solo il contesto"),
]

CLI_ARGOMENTI = [
    ["/bin/gemini", ["--model", "{model}", "--approval-mode", "yolo"], "gemini-2.5-pro"],
    ["g.cmd", ["--nome={model}", "{model}{model}"], "m"],
    ["g", [], ""],
]
CANDIDATI = ["gemini", "", "claude"]

# I corpi che i fornitori mandano indietro. Quelli storti contano piu' di
# quelli buoni: una risposta letta male non da' un errore, da' **una risposta
# vuota** — e per chi guarda NOVA e' indistinguibile da un modello che non ha
# saputo rispondere.
RISPOSTE = [
    {"choices": [{"message": {"content": "ecco la risposta"}}],
     "usage": {"prompt_tokens": 120, "completion_tokens": 40}},
    # una lista di scelte vuota: capita, ed e' il caso in cui «la prima»
    # senza pensarci si porta dietro un errore
    {"choices": []},
    {},
    {"choices": [{}]},
    {"choices": [{"message": None}]},
    # il ragionamento, nei tre modi in cui arriva
    {"choices": [{"message": {"content": "r", "reasoning_content": "penso"}}]},
    {"choices": [{"message": {"content": "r", "reasoning": "penso"}}]},
    {"choices": [{"message": {"content": "<think>ci penso</think>ecco"}}]},
    # un `<think>` che il modello non ha chiuso, perche' si e' interrotto
    {"choices": [{"message": {"content": "prima <think>e poi si ferma"}}]},
    # tutti e due insieme
    {"choices": [{"message": {"content": "<think>a</think>b",
                              "reasoning_content": "fuori"}}]},
    # le chiamate di strumento
    {"choices": [{"message": {"content": "", "tool_calls": [
        {"id": "1", "function": {"name": "read_file",
                                 "arguments": "{\"path\": \"x\"}"}}]}}]},
    # i token contati in modi strani
    {"usage": {"prompt_tokens": "12", "completion_tokens": 7.9}},
    {"usage": {"prompt_tokens": None, "completion_tokens": None}},
    {"usage": None},
    # accenti ed emoji, che e' dove il confronto si rompe se si sbaglia
    {"choices": [{"message": {"content": "perché la città è così \U0001f9ea"}}]},
    # Tutti e due i nomi del ragionamento **insieme**, con valori diversi:
    # e' l'unico caso in cui si vede quale dei due vince.
    {"choices": [{"message": {"content": "r", "reasoning_content": "primo",
                              "reasoning": "secondo"}}]},
    # Il primo vuoto: in Python `"" or x` da' x, e senza quel dettaglio il
    # ragionamento sparirebbe invece di prendere l'altro nome.
    {"choices": [{"message": {"content": "r", "reasoning_content": "",
                              "reasoning": "quello buono"}}]},
    # Piu' di una scelta: NOVA ne chiede sempre una sola, ma se un fornitore
    # ne manda due la prima e' la prima.
    {"choices": [{"message": {"content": "questa"}},
                 {"message": {"content": "non questa"}}]},
]


BUONA = '{"choices":[{"message":{"content":"ecco"}}]}'

# Il giro dei tentativi. Gli scenari che contano sono quelli storti: qui si
# decide se un guasto passeggero costa un turno o una conversazione, e se
# una quota finita si racconta come «riprova» o come «non ha funzionato» —
# che sono due cose diverse per il router, non due frasi.
def muto(quanti):
    return [{"muto": "connessione"} for _ in range(quanti)]


def risponde(codice=200, corpo=BUONA, riprova=None):
    return {"codice": codice, "corpo": corpo, "riprova_fra": riprova}


GIRI = [
    # buona al primo colpo
    dict(tappe=[risponde()], base_url="http://127.0.0.1:8080",
         etichetta="Modello locale", in_casa=True),
    # due silenzi e poi risponde: si riprova, e si aspetta in mezzo
    dict(tappe=muto(2) + [risponde()], base_url="https://api.esempio.it",
         etichetta="API esterna", in_casa=False),
    # muto per tutti e tre i tentativi, in casa
    dict(tappe=muto(3), base_url="http://127.0.0.1:8080",
         etichetta="Modello locale", in_casa=True),
    # muto per tutti e tre, fuori: la cura e' un'altra
    dict(tappe=muto(3), base_url="https://api.esempio.it",
         etichetta="API esterna", in_casa=False),
    # quota finita, col tempo dichiarato dal fornitore
    dict(tappe=[risponde(429, '{"error":"rate limited"}', "120")],
         base_url="https://api.esempio.it", etichetta="API esterna", in_casa=False),
    # quota finita senza `Retry-After`
    dict(tappe=[risponde(402, '{"error":"insufficient credits"}')],
         base_url="https://api.esempio.it", etichetta="API esterna", in_casa=False),
    # `Retry-After` fuori dai limiti, e uno che non e' un numero
    dict(tappe=[risponde(429, "{}", "5")], base_url="https://x.it",
         etichetta="API esterna", in_casa=False),
    dict(tappe=[risponde(429, "{}", "Wed, 21 Oct 2026 07:28:00 GMT")],
         base_url="https://x.it", etichetta="API esterna", in_casa=False),
    # una richiesta sbagliata: non si riprova, rimandarla uguale non la
    # raddrizza
    dict(tappe=[risponde(400, '{"error":{"message":"model not found"}}'),
                risponde()],
         base_url="https://api.esempio.it", etichetta="API esterna", in_casa=False),
    # il fornitore che sta male
    dict(tappe=[risponde(503, "service unavailable")], base_url="https://x.it",
         etichetta="API esterna", in_casa=False),
    # una chiave sbagliata
    dict(tappe=[risponde(401, '{"error":{"message":"invalid api key"}}')],
         base_url="https://x.it", etichetta="API esterna", in_casa=False),
    # Un corpo lunghissimo. Il taglio a 600 si vede solo dove il messaggio
    # del fornitore viene **riportato**, cioe' su un 400 con un `error`
    # dentro: tagliato, quel JSON non si legge piu' e resta la frase secca.
    # Tagliare in un punto diverso vuol dire dire una cosa diversa.
    dict(tappe=[risponde(400, json.dumps({"error": {"message": "x" * 900}}))],
         base_url="https://x.it", etichetta="API esterna", in_casa=False),
    # E uno con gli accenti, che e' dove tagliare a byte spezza una lettera.
    dict(tappe=[risponde(400, json.dumps({"error": {"message": "però " * 200}},
                                         ensure_ascii=False))],
         base_url="https://x.it", etichetta="API esterna", in_casa=False),
    # un silenzio, poi una quota finita
    dict(tappe=muto(1) + [risponde(429, "{}", "60")], base_url="https://x.it",
         etichetta="API esterna", in_casa=False),
]


def _msg(lista):
    return [{"ruolo": m.get("role", ""), "contenuto": m.get("content", "")}
            for m in lista]


fuori = rust({
    "claude": [{**c, "extra_args": c.get("extra_args", []),
                "file_prompt": c.get("file_prompt", "")} for c in CLAUDE],
    "prompt": [{**p, "messaggi": _msg(p["messaggi"])} for p in PROMPT],
    "ultimo_utente": [_msg(m) for m in MESSAGGI],
    "stato_claude": STATO_CLAUDE,
    "autonomie": AUTONOMIE,
    "payload": [{**p, "messaggi": _msg(p["messaggi"])} for p in PAYLOAD],
    "semplici": SEMPLICI,
    "attese": ATTESE,
    "chiavi": CHIAVI,
    "stato_locale": STATO_LOCALE,
    "cli_prompt": [{**c, "messaggi": _msg(c["messaggi"])} for c in CLI_PROMPT],
    "cli_argomenti": CLI_ARGOMENTI,
    "candidati": CANDIDATI,
    "risposte": RISPOSTE,
    "giri": GIRI,
})

print("\n1. la riga di comando di Claude Code")
diverse = []
for c, ru in zip(CLAUDE, fuori["claude"]):
    b = finto(eseguibile=c["eseguibile"], model=c["model"],
              cfg=cfg_di(c["autonomia"]), max_turns=c.get("max_turns", 0),
              session_id=c.get("session_id", ""), mcp_config=c.get("mcp_config", ""),
              extra_args=c.get("extra_args", []))
    py = claude_cli.ClaudeCodeBrain._argomenti(b, c["sistema"], usa_file=False)
    if ru != py:
        primo = next((k for k, (a, b2) in enumerate(zip(ru, py)) if a != b2),
                     min(len(ru), len(py)))
        diverse.append(f"{c['autonomia']}: a {primo}: {str(ru[primo:primo+2])[:90]} "
                       f"vs {str(py[primo:primo+2])[:90]}")
controlla(f"le {len(CLAUDE)} righe di comando coincidono", not diverse,
          " | ".join(diverse[:2]))

# L'ordine e' il punto, e va provato in quanto tale: su Windows `claude` e'
# un file batch, e cmd.exe rianalizza la riga. Un argomento con degli a capo
# la chiude li'.
con_mcp = next(i for i, c in enumerate(CLAUDE)
               if c.get("mcp_config") and "\n" in c["sistema"])
riga = fuori["claude"][con_mcp]
controlla("le opzioni MCP stanno prima del prompt di sistema",
          riga.index("--mcp-config") < riga.index("--append-system-prompt"),
          str(riga))
controlla("il banco ha un prompt di sistema con degli a capo",
          "\n" in CLAUDE[con_mcp]["sistema"],
          "senza, il guaio di cmd.exe non e' provato")
k = riga.index("--allowedTools")
controlla("gli strumenti permessi sono un argomento solo",
          riga[k + 2].startswith("--"),
          f"dopo l'elenco c'e' {riga[k + 2]!r}, che il CLI non legge come permesso")
controlla("e sono trentatre'", len(riga[k + 1].split(",")) == 33,
          str(len(riga[k + 1].split(","))))

print("\n2. il prompt di sistema")
import getpass  # noqa: E402


def py_prompt(p):
    """Il Python vero, con l'utente e la cartella di casa messi da noi:
    sono le due cose che cambiano da macchina a macchina, e il banco deve
    dare lo stesso risultato su una macchina che non e' questa."""
    b = finto(vault_path=p["vault"], mcp_config="x" if p["con_mcp"] else "")
    vero_utente, vero_path = getpass.getuser, claude_cli.Path
    try:
        getpass.getuser = lambda: p["utente"]
        claude_cli.Path = type("FintoPath", (), {"home": staticmethod(lambda: p["home"])})
        return claude_cli.ClaudeCodeBrain._system_prompt(b, p["messaggi"])
    finally:
        getpass.getuser = vero_utente
        claude_cli.Path = vero_path


diverse = []
for p, ru in zip(PROMPT, fuori["prompt"]):
    py = py_prompt(p)
    if ru != py:
        primo = next((k for k, (a, b2) in enumerate(zip(ru, py)) if a != b2),
                     min(len(ru), len(py)))
        diverse.append(f"a {primo}: {ru[primo:primo+60]!r} vs {py[primo:primo+60]!r}")
controlla(f"i {len(PROMPT)} prompt di sistema coincidono", not diverse,
          " | ".join(diverse[:1]))
controlla("il banco ha un caso col vault e uno senza",
          any(p["vault"] for p in PROMPT) and any(not p["vault"] for p in PROMPT))
controlla("e uno con l'MCP e uno senza",
          len({p["con_mcp"] for p in PROMPT if p["vault"]}) == 2,
          "senza, il suggerimento sbagliato non si vede")

print("\n3. le decisioni piccole attorno a Claude Code")
diverse = []
for (m, t, d, c), ru in zip(STATO_CLAUDE, fuori["stato_claude"]):
    vero = claude_cli.tipo_accesso
    try:
        claude_cli.tipo_accesso = lambda: (t, d)
        py = claude_cli.ClaudeCodeBrain.descrizione_stato(
            finto(model=m, costo_sessione=c))
    finally:
        claude_cli.tipo_accesso = vero
    if ru != py:
        diverse.append(f"{m}/{t}/{d}/{c}: rust {ru!r} vs python {py!r}")
controlla(f"le {len(STATO_CLAUDE)} descrizioni di stato coincidono", not diverse,
          " | ".join(diverse[:2]))

diverse = [f"{a!r}: rust {ru!r} vs python {claude_cli.PERMESSI.get(a, 'acceptEdits')!r}"
           for a, ru in zip(AUTONOMIE, fuori["autonomie"])
           if ru != claude_cli.PERMESSI.get(a, "acceptEdits")]
controlla(f"i {len(AUTONOMIE)} livelli di autonomia si traducono uguale", not diverse,
          " | ".join(diverse[:2]))
controlla("un livello che non si capisce non da' le mani libere",
          fuori["autonomie"][AUTONOMIE.index("mai_visto")] != "bypassPermissions")

diverse = [f"{i}" for (m, ru) in zip(MESSAGGI, fuori["ultimo_utente"])
           for i in [0] if ru != claude_cli.ClaudeCodeBrain._ultimo_utente(m)]
controlla(f"l'ultimo messaggio dell'utente e' lo stesso in tutti e {len(MESSAGGI)} i casi",
          not diverse, str(diverse[:2]))

print("\n4. il payload del dialetto OpenAI")


def py_payload(c):
    b = finto(model=c["model"])
    cfg = finto(model=finto(temperature=c["temperature"], top_p=c["top_p"],
                            max_tokens=c["max_tokens"], top_k=c["top_k"]))
    p = openai_compat.OpenAICompatBrain._payload(b, c["messaggi"], c["tools"], cfg)
    if c["top_k"] is not None:
        p["top_k"] = cfg.model.top_k
    return json.dumps(p, separators=(",", ":"), ensure_ascii=False)


diverse = []
for c, ru in zip(PAYLOAD, fuori["payload"]):
    py = py_payload(c)
    if ru != py:
        primo = next((k for k, (a, b2) in enumerate(zip(ru, py)) if a != b2),
                     min(len(ru), len(py)))
        diverse.append(f"a {primo}: {ru[primo:primo+60]!r} vs {py[primo:primo+60]!r}")
controlla(f"i {len(PAYLOAD)} payload coincidono, chiave per chiave e in ordine",
          not diverse, " | ".join(diverse[:1]))
controlla("il banco ha un payload con strumenti e uno senza",
          any(c["tools"] for c in PAYLOAD) and any(not c["tools"] for c in PAYLOAD))
controlla("e uno col top_k, che e' solo del modello in casa",
          any(c["top_k"] is not None for c in PAYLOAD),
          "mandarlo a un'API e' un 400")


def py_semplice(model, prompt, max_tokens):
    visto = {}
    b = openai_compat.OpenAICompatBrain.__new__(openai_compat.OpenAICompatBrain)
    b.model = model
    b._post = lambda p: (visto.update(p=p), {"choices": [{"message": {}}]})[1]
    openai_compat.OpenAICompatBrain.semplice(b, prompt, max_tokens)
    return json.dumps(visto["p"], separators=(",", ":"), ensure_ascii=False)


diverse = [f"{m}: rust {ru!r} vs python {py_semplice(m, p, t)!r}"
           for (m, p, t), ru in zip(SEMPLICI, fuori["semplici"])
           if ru != py_semplice(m, p, t)]
controlla(f"i {len(SEMPLICI)} payload della domanda secca coincidono", not diverse,
          " | ".join(diverse[:1]))

print("\n5. quanto aspettare, e cosa si manda in testa")


def py_attesa(valore):
    r = finto(headers={"Retry-After": valore} if valore is not None else {})
    return openai_compat.OpenAICompatBrain._quanto_aspettare(None, r)


diverse = [f"{v!r}: rust {ru} vs python {py_attesa(v)}"
           for v, ru in zip(ATTESE, fuori["attese"]) if ru != py_attesa(v)]
controlla(f"le {len(ATTESE)} attese coincidono", not diverse, " | ".join(diverse[:2]))
controlla("il banco ha un Retry-After che non e' un numero",
          any(v and not v.replace(".", "").replace("-", "").isdigit() for v in ATTESE),
          "e' la forma che la specifica permette, ed e' quella che rompe")


def py_headers(k):
    return list(openai_compat.OpenAICompatBrain._headers(finto(api_key=k)).items())


diverse = [f"{k!r}" for k, ru in zip(CHIAVI, fuori["chiavi"])
           if [tuple(x) for x in ru] != py_headers(k)]
controlla(f"le {len(CHIAVI)} intestazioni coincidono", not diverse, str(diverse))

diverse = [f"{m!r}: rust {ru!r} vs python {openai_compat.LocalBrain.descrizione_stato(finto(model=m))!r}"
           for m, ru in zip(STATO_LOCALE, fuori["stato_locale"])
           if ru != openai_compat.LocalBrain.descrizione_stato(finto(model=m))]
controlla(f"i {len(STATO_LOCALE)} stati del modello locale coincidono", not diverse,
          " | ".join(diverse[:2]))

print("\n6. la CLI generica")
diverse = []
for c, ru in zip(CLI_PROMPT, fuori["cli_prompt"]):
    py = cli_generic.CliBrain._prompt_completo(
        finto(kb_context=c["contesto_kb"]), c["messaggi"])
    if ru != py:
        diverse.append(f"rust {ru[:70]!r} vs python {py[:70]!r}")
controlla(f"i {len(CLI_PROMPT)} prompt completi coincidono", not diverse,
          " | ".join(diverse[:1]))
controlla("il banco ha piu' scambi di quelli che si riscrivono",
          any(len(c["messaggi"]) > 6 for c in CLI_PROMPT),
          "senza, il taglio agli ultimi sei non e' provato")

diverse = []
for (e, a, m), ru in zip(CLI_ARGOMENTI, fuori["cli_argomenti"]):
    py = cli_generic.CliBrain._argomenti(
        finto(_eseguibile=e, spec={"args": a}, model=m))
    if ru != py:
        diverse.append(f"rust {ru} vs python {py}")
controlla(f"le {len(CLI_ARGOMENTI)} righe di comando coincidono", not diverse,
          " | ".join(diverse[:1]))

# L'ordine in cui si cercano i nomi nel PATH non si legge dal risultato: si
# guarda **cosa ha chiesto**. Su Windows npm installa un `.cmd`, ed e' quello
# che si puo' eseguire.
import shutil  # noqa: E402

diverse = []
for b, ru in zip(CANDIDATI, fuori["candidati"]):
    chiesti: list[str] = []
    vero = shutil.which
    try:
        shutil.which = lambda n: chiesti.append(n)
        cli_generic.CliBrain._trova(b)
    finally:
        shutil.which = vero
    if ru != chiesti:
        diverse.append(f"{b!r}: rust {ru} vs python {chiesti}")
controlla(f"i {len(CANDIDATI)} nomi si cercano nello stesso ordine", not diverse,
          " | ".join(diverse[:2]))
controlla("il .cmd si cerca per primo",
          fuori["candidati"][0][0].endswith(".cmd"),
          "su Windows npm installa quello, e il nome nudo e' uno script")

print("\n7. e cosa ha detto il modello, letto dalla risposta del fornitore")


def py_risposta(corpo):
    """Il `chat` vero, con al posto della rete il corpo gia' pronto."""
    b = openai_compat.OpenAICompatBrain.__new__(openai_compat.OpenAICompatBrain)
    b.model = "m"
    b._post = lambda p: corpo
    cfg = finto(model=finto(temperature=0.7, top_p=0.9, max_tokens=1))
    r = openai_compat.OpenAICompatBrain.chat(b, [], [], cfg)
    return {"contenuto": r.contenuto, "ragionamento": r.ragionamento,
            "tool_calls": r.tool_calls, "token_input": r.token_input,
            "token_output": r.token_output}


diverse = []
for i, (corpo, ru) in enumerate(zip(RISPOSTE, fuori["risposte"])):
    py = py_risposta(corpo)
    if ru != py:
        diverse.append(f"caso {i}: rust {ru} vs python {py}")
controlla(f"le {len(RISPOSTE)} risposte si leggono uguale", not diverse,
          " | ".join(diverse[:2]))
controlla("il banco ha una lista di scelte vuota",
          any(r.get("choices") == [] for r in RISPOSTE),
          "senza, «la prima di zero» non e' provata")
controlla("e un <think> che il modello non ha chiuso",
          any("<think>" in str(r) and "</think>" not in str(r) for r in RISPOSTE),
          "senza, il troncone di ragionamento finisce nella risposta")
controlla("e dei token contati in un modo che Python accetta a fatica",
          any(isinstance((r.get("usage") or {}).get("prompt_tokens"), str)
              for r in RISPOSTE))

print("\n8. il giro dei tentativi: cosa si riprova, e cosa si dice")
import requests                                              # noqa: E402


class FintaRisposta:
    def __init__(self, t):
        self.status_code = t["codice"]
        self.text = t["corpo"]
        self.headers = ({"Retry-After": t["riprova_fra"]}
                        if t.get("riprova_fra") is not None else {})

    def json(self):
        return json.loads(self.text)


class FintaSessione:
    """La rete, sostituita da un copione. Cosi' quello che si misura e' la
    politica dei tentativi, non se il PC e' in rete."""

    def __init__(self, tappe):
        self.tappe = list(tappe)

    def post(self, *a, **k):
        if not self.tappe:
            raise requests.ConnectionError("copione finito")
        t = self.tappe.pop(0)
        if t.get("muto"):
            raise (requests.Timeout if t["muto"] == "scaduto"
                   else requests.ConnectionError)("muto")
        return FintaRisposta(t)


def py_giro(g):
    from nova.brains.base import LimiteUso
    b = openai_compat.OpenAICompatBrain.__new__(openai_compat.OpenAICompatBrain)
    b.base_url = g["base_url"].rstrip("/")
    b.api_key = ""
    b.model = "m"
    b.timeout_lettura = 1
    b.etichetta = g["etichetta"]
    b._sessione = FintaSessione(g["tappe"])
    attese = []
    vero_sleep = openai_compat.time.sleep
    openai_compat.time.sleep = lambda s: attese.append(s)
    try:
        dati = openai_compat.OpenAICompatBrain._post(b, {})
        msg = (dati.get("choices") or [{}])[0].get("message") or {}
        return {"esito": "ok", "messaggio": "", "riprova_fra_s": 0,
                "contenuto": (msg.get("content") or "").strip(), "attese": attese}
    except LimiteUso as e:
        return {"esito": "limite", "messaggio": str(e),
                "riprova_fra_s": e.riprova_fra_s, "contenuto": "", "attese": attese}
    except RuntimeError as e:
        # Python non distingue: sono tutti e due `RuntimeError`. La
        # differenza la fa **quale** testo, e quello si confronta.
        testo = str(e)
        dentro = "non risponde su" in testo or "non riesco a raggiungere" in testo
        return {"esito": "irraggiungibile" if dentro else "fornitore",
                "messaggio": testo, "riprova_fra_s": 0, "contenuto": "",
                "attese": attese}
    finally:
        openai_compat.time.sleep = vero_sleep


diverse = []
for i, (g, ru) in enumerate(zip(GIRI, fuori["giri"])):
    py = py_giro(json.loads(json.dumps(g)))
    if ru != py:
        primi = [k for k in py if ru.get(k) != py[k]]
        diverse.append(f"giro {i} ({g['etichetta']}, {len(g['tappe'])} tappe): "
                       f"{primi} rust {[ru.get(k) for k in primi]} vs "
                       f"python {[py[k] for k in primi]}")
controlla(f"i {len(GIRI)} giri di tentativi finiscono allo stesso modo",
          not diverse, " | ".join(diverse[:2])[:400])
controlla("il banco ha un giro che si riprende dopo due silenzi",
          any(sum(1 for t in g["tappe"] if t.get("muto")) == 2
              and not g["tappe"][-1].get("muto") for g in GIRI),
          "senza, «si riprova» non e' provato")
controlla("e uno che non risponde mai, dentro e fuori casa",
          len({g["in_casa"] for g in GIRI
               if all(t.get("muto") for t in g["tappe"])}) == 2,
          "il silenzio si racconta in due modi: e' spento, oppure non sei in rete")
controlla("e una quota finita, che non e' un errore del compito",
          any(r["esito"] == "limite" for r in fuori["giri"]),
          "se arriva come errore qualunque, il ripiego non parte mai")
# Dopo l'ultimo tentativo non si aspetta: la risposta e' gia' decisa. Col
# modello locale spento erano quindici secondi di attese su ogni domanda,
# prima del messaggio che dice di riaccenderlo — e li ha trovati il banco,
# perche' li avevo tolti dal Rust senza toglierli dal Python (D191).
mai = next(g for g in GIRI if all(t.get("muto") for t in g["tappe"]))
attese_mai = fuori["giri"][GIRI.index(mai)]["attese"]
controlla("dopo l'ultimo tentativo non si aspetta per niente",
          len(attese_mai) == len(mai["tappe"]) - 1, str(attese_mai))
controlla("e un 400, che non si riprova",
          any(len(g["tappe"]) > 1 and g["tappe"][0].get("codice") == 400
              for g in GIRI),
          "rimandare uguale una richiesta sbagliata non la raddrizza")


print("\n9. e come si paga Claude Code, che non e' contabilita'")

#: (chiave nell'ambiente, contenuto di .credentials.json oppure None)
#:
#: `None` vuol dire «il file non c'e' o non si legge»: e' un «non lo so», non
#: un «non e' abbonato». Le due frasi mandano a controllare cose diverse — la
#: prima l'accesso, la seconda il portafoglio.
ACCESSI = [
    ("sk-ant-qualcosa", {"claudeAiOauth": {"subscriptionType": "max"}}),
    ("", {"claudeAiOauth": {"subscriptionType": "max",
                            "rateLimitTier": "default_claude_max_5x"}}),
    ("", {"claudeAiOauth": {"subscriptionType": "pro"}}),
    ("", {"claudeAiOauth": {"subscriptionType": "pro", "rateLimitTier": ""}}),
    ("", {"claudeAiOauth": {"subscriptionType": "pro", "rateLimitTier": None}}),
    ("", {"claudeAiOauth": {}}),
    ("", {"claudeAiOauth": None}),
    ("", {}),
    ("", None),
    # Le trappole vere: in Python sono **falsi** anche lo zero e il booleano,
    # e un porto che guardasse solo `null` direbbe «abbonamento 0».
    ("", {"claudeAiOauth": {"subscriptionType": 0}}),
    ("", {"claudeAiOauth": {"subscriptionType": False}}),
    ("", {"claudeAiOauth": {"subscriptionType": True}}),
    ("", {"claudeAiOauth": {"subscriptionType": ""}}),
    # `.strip()` di Python toglie anche i separatori di unita', che
    # `char::is_whitespace` non considera bianchi (D182).
    ("", {"claudeAiOauth": {"subscriptionType": "  max  "}}),
    ("", {"claudeAiOauth": {"subscriptionType": "\x1fmax\x1f"}}),
    ("", {"claudeAiOauth": {"subscriptionType": "max",
                            "rateLimitTier": "default_claude_default_claude_x"}}),
    # Il prefisso **in mezzo**: Python lo toglie ovunque, e un porto che
    # tagliasse solo la testa resterebbe verde su tutti i casi qui sopra —
    # anche su quello ripetuto, perche' `trim_start_matches` di Rust ripete.
    ("", {"claudeAiOauth": {"subscriptionType": "max",
                            "rateLimitTier": "max_default_claude_5x"}}),
]

#: I `%APPDATA%` da cui ricavare il ripiego di npm.
#:
#: Fra questi **non** c'e' un percorso in stile Unix («/home/x/.config»): su
#: Windows `%APPDATA%` non e' mai fatto cosi', e le due librerie standard lo
#: normalizzano diversamente — Python lo riscrive con le barre rovesce, Rust
#: lo lascia com'e'. Confrontare la normalizzazione di due librerie su un
#: valore che quella piattaforma non produce e' un rosso che non corrisponde a
#: nessun difetto, e i rossi che non corrispondono a niente insegnano a non
#: leggere i rossi.
RIPIEGHI = [
    r"C:\Users\x\AppData\Roaming",
    "",                                  # variabile assente: viene relativo
    "C:\\Users\\Gio Rossi\\AppData\\Roaming",   # gli spazi ci sono davvero
    "C:\\Users\\x\\AppData\\Roaming\\",  # con la barra in fondo
    r"\\server\condivisa\AppData",       # profilo in rete: capita in azienda
]

fuori2 = rust({
    "accessi": [[k, c] for k, c in ACCESSI],
    "ripieghi": RIPIEGHI,
})


def py_accesso(chiave, credenziali):
    """Il `tipo_accesso` vero, con l'ambiente e la casa spostati sotto."""
    import tempfile
    vero_env = os.environ.get("ANTHROPIC_API_KEY")
    vera_home = claude_cli.Path.home
    with tempfile.TemporaryDirectory() as tmp:
        casa = Path(tmp)
        (casa / ".claude").mkdir()
        if credenziali is not None:
            (casa / ".claude" / ".credentials.json").write_text(
                json.dumps(credenziali), encoding="utf-8")
        try:
            if chiave:
                os.environ["ANTHROPIC_API_KEY"] = chiave
            else:
                os.environ.pop("ANTHROPIC_API_KEY", None)
            claude_cli.Path.home = staticmethod(lambda: casa)
            return list(claude_cli.tipo_accesso())
        finally:
            claude_cli.Path.home = vera_home
            os.environ.pop("ANTHROPIC_API_KEY", None)
            if vero_env is not None:
                os.environ["ANTHROPIC_API_KEY"] = vero_env


diverse = []
for (chiave, cred), ru in zip(ACCESSI, fuori2["accessi"]):
    py = py_accesso(chiave, cred)
    if list(ru) != py:
        diverse.append(f"{cred!r}: rust {ru} vs python {py}")
controlla(f"i {len(ACCESSI)} modi di pagare si leggono uguali", not diverse,
          " | ".join(diverse[:2]))
controlla("un abbonamento non si chiama «a consumo»",
          any(r[0] == "abbonamento" for r in fuori2["accessi"]),
          "col dollaro riportato che e' l'equivalente API, chiamarlo consumo "
          "vuol dire mostrare una spesa che nessuno paga")
controlla("«non lo so» esiste, e non e' «non e' abbonato»",
          any(r[0] == "sconosciuto" for r in fuori2["accessi"]))
controlla("uno zero non diventa un abbonamento",
          fuori2["accessi"][ACCESSI.index(("", {"claudeAiOauth": {"subscriptionType": 0}}))][0]
          == "sconosciuto")

# Dove npm mette Claude quando il PATH non lo sa.
diverse = []
for appdata, ru in zip(RIPIEGHI, fuori2["ripieghi"]):
    py = str(Path(appdata) / "npm" / "claude.cmd")
    if ru != py:
        diverse.append(f"{appdata!r}: rust {ru!r} vs python {py!r}")
controlla("il ripiego di npm e' lo stesso percorso", not diverse,
          " | ".join(diverse[:2]))
controlla("i nomi di Claude si cercano nello stesso ordine",
          fuori2["candidati_claude"] == ["claude.cmd", "claude.exe", "claude"],
          f"rust {fuori2['candidati_claude']}: su Windows npm installa il .cmd, "
          "e cercare l'.exe per primo vuol dire non trovarlo dove c'e'")


print("\n10. e perche' una CLI non e' pronta, lo dicono con le stesse parole")

#: (binario, nome) di CLI che non si trovano nel PATH.
#:
#: Nessuno confrontava questa frase, e ci e' voluto un caso vero per
#: accorgersene: «Installalo» e basta e' la cura **sbagliata** quando il
#: programma e' installato e NOVA sta guardando un PATH vecchio. Un processo
#: eredita le variabili d'ambiente da chi lo ha avviato, quindi una CLI
#: installata mentre NOVA gira resta invisibile finche' NOVA non riparte.
CLI_NON_PRONTE = [
    ("agy", "antigravity"),
    ("codex", "codex"),
    ("qwen", "Qwen Code"),
    # I caratteri che rompono le virgolette: il nome lo sceglie l'utente.
    ("mio-agente", "il «mio» agente"),
]

fuori3 = rust({"cli_non_pronte": [[b, n] for b, n in CLI_NON_PRONTE]})

from nova.brains.cli_generic import motivo_non_pronto          # noqa: E402

diverse = []
for (binario, nome), ru in zip(CLI_NON_PRONTE, fuori3["cli_non_pronte"]):
    py = motivo_non_pronto(binario, nome)
    if ru != py:
        diverse.append(f"{binario}: rust {ru!r} vs python {py!r}")
controlla(f"le {len(CLI_NON_PRONTE)} frasi coincidono", not diverse,
          " | ".join(diverse[:1]))
controlla("e dicono di riavviare NOVA, non di reinstallare e basta",
          all("riavvia NOVA" in r for r in fuori3["cli_non_pronte"]),
          "una CLI installata mentre NOVA gira e' invisibile finche' non "
          "riparte, e «installalo» manda a rifare una cosa gia' fatta (D193)")
controlla("ma dicono anche cosa fare se davvero non c'e'",
          all("installalo" in r.lower() for r in fuori3["cli_non_pronte"]),
          "il caso normale resta il piu' probabile: non va tolto")

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
