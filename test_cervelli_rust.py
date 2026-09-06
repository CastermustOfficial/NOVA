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

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
