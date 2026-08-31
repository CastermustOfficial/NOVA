# -*- coding: utf-8 -*-
"""I guasti in Rust devono dire le stesse parole, e coprire le stesse chiavi.

Settimo pezzo. E' quello che serve a tutti gli altri: quando il Python sara'
andato via, qualunque parte di NOVA che debba dire «non ci sono riuscito» deve
poterlo dire come lo dice NOVA, non come lo dice il sistema operativo.

Due confronti, e il secondo pesa piu' del primo.

Le **frasi** devono essere identiche, o NOVA parla con due voci a seconda di
quale meta' di se stessa sta rispondendo.

Il **mascheramento delle chiavi** deve essere identico o piu' largo. Il caso
e' vero e succedeva: il fornitore, quando la chiave e' sbagliata, la rimanda
indietro dentro il proprio errore - «Incorrect API key provided: sk-...» - e
da li' finiva in chat e nel registro (D29). Una divergenza in cui il Rust
copre qualcosa in piu' e' un fastidio; una in cui copre qualcosa in meno e'
una chiave che esce, e non se ne accorge nessuno finche' non e' tardi. Percio'
qui non si pretende l'uguaglianza: si pretende che **niente di segreto
sopravviva da nessuna delle due parti**.

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

NOME = "banco-guasti.exe" if os.name == "nt" else "banco-guasti"
BINARIO = RADICE / "core" / "target" / "release" / NOME
if not BINARIO.is_file():
    print("Il banco Rust non e' costruito per questo sistema. Per averlo:")
    print("  cd core && .\\x.cmd build --release -p nova-guasti "
          "--features banco --bin banco-guasti")
    sys.exit(2)

from nova import guasti as py                                # noqa: E402

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
                       capture_output=True, text=True, encoding="utf-8", timeout=120)
    if p.returncode != 0:
        raise RuntimeError(f"banco uscito {p.returncode}: {p.stderr[:400]}")
    return json.loads(p.stdout)


# =======================================================================
print("\n=== Le frasi ===")

CASI = [
    ("non_trovato", FileNotFoundError(2, "x", "llama-server.exe"),
     {"tipo": "non_trovato", "nome": "llama-server.exe"}),
    ("cartella", IsADirectoryError(21, "x", "Documenti"),
     {"tipo": "cartella", "nome": "Documenti"}),
    ("permesso", PermissionError(13, "x", "relazione.docx"),
     {"tipo": "permesso", "nome": "relazione.docx"}),
    ("connessione rifiutata", ConnectionRefusedError(),
     {"tipo": "nessuna_risposta"}),
    ("tempo scaduto", TimeoutError(), {"tipo": "troppo_tempo"}),
    ("memoria finita", MemoryError(), {"tipo": "memoria"}),
    ("avvitato", RecursionError(), {"tipo": "avvitato"}),
    ("libreria nota", ModuleNotFoundError("fitz", name="fitz"),
     {"tipo": "libreria", "nome": "fitz"}),
    ("libreria qualunque", ModuleNotFoundError("requests", name="requests"),
     {"tipo": "libreria", "nome": "requests"}),
    ("libreria senza nome", ModuleNotFoundError("boh"),
     {"tipo": "libreria", "nome": ""}),
    ("errore qualunque", ValueError("il numero non torna"),
     {"tipo": "altro", "testo": "il numero non torna"}),
    ("errore muto", ValueError(""), {"tipo": "altro", "testo": ""}),
]

for cosa in ("", "Aprendo il documento"):
    dentro = {"guasti": [{**c[2], "cosa": cosa} for c in CASI]}
    frasi = rust(dentro)["frasi"]
    diversi = []
    for (etichetta, ecc, _), atteso in zip(CASI, frasi):
        mio = py.spiega(ecc, cosa)
        if mio != atteso:
            diversi.append((etichetta, mio, atteso))
    controlla(f"{len(CASI)} guasti, con cosa={cosa!r}", not diversi,
              f"\n    primo scarto: {diversi[0] if diversi else ''}")

# Gli errori di sistema con il numero di Windows.
SISTEMA = [5, 32, 112, 1225, 9999]
dentro = {"guasti": [{"tipo": "sistema", "winerror": n, "testo": "boh"}
                     for n in SISTEMA]}
frasi = rust(dentro)["frasi"]
diversi = []
for n, atteso in zip(SISTEMA, frasi):
    e = OSError("boh")
    e.winerror = n
    e.strerror = "boh"
    if py.spiega(e, "") != atteso:
        diversi.append((n, py.spiega(e, ""), atteso))
controlla("gli errori di Windows, numero per numero", not diversi,
          f"\n    scarti: {diversi[:2]}")

print("\n=== Il modello spento non e' la rete giu' ===")
URL = [("http://127.0.0.1:8080", True), ("https://api.esempio.it/v1", False)]
fuori = rust({"irraggiungibili": [list(u) for u in URL]})["irraggiungibili"]
diversi = [(u, py.spiega_irraggiungibile(u, c), a)
           for (u, c), a in zip(URL, fuori)
           if py.spiega_irraggiungibile(u, c) != a]
controlla("le due frasi sono identiche", not diversi, str(diversi[:1]))

print("\n=== I pacchetti ===")
MODULI = ["fitz", "docx", "PIL", "cv2", "yaml", "requests", "una_cosa_mia"]
fuori = rust({"moduli": MODULI})["pacchetti"]
diversi = [(m, py._PACCHETTO.get(m, m), a) for m, a in zip(MODULI, fuori)
           if py._PACCHETTO.get(m, m) != a]
controlla("il nome da installare e' lo stesso", not diversi, str(diversi[:2]))

print("\n=== Le chiavi, che e' la parte che conta ===")

SEGRETI = [
    "sk-abcd1234efgh5678ijkl",
    "gsk_abcd1234efgh5678ijkl",
    "xai-abcd1234efgh5678ijkl",
    "AIzaAbcd1234efgh5678ijkl",
    "abcdefghijklmnop1234567890",
]
TESTI = [
    "Incorrect API key provided: sk-abcd1234efgh5678ijkl",
    "errore: gsk_abcd1234efgh5678ijkl rifiutata",
    "chiave xai-abcd1234efgh5678ijkl scaduta",
    "AIzaAbcd1234efgh5678ijkl non valida",
    'api_key: abcdefghijklmnop1234567890',
    '{"access_token":"abcdefghijklmnop1234567890"}',
    "Authorization: Bearer abcdefghijklmnop1234567890",
    "secret = abcdefghijklmnop1234567890",
    "prima sk-abcd1234efgh5678ijkl poi gsk_abcd1234efgh5678ijkl",
    "perché la chiave sk-abcd1234efgh5678ijkl è sbagliata",
    # E i casi che NON devono essere coperti, o i messaggi diventano illeggibili.
    "la chiave non e' stata accettata",
    "token non valido",
    "ask-me di nuovo",
    "key: 1234",
    "",
]

fuori = rust({"da_mascherare": TESTI})["mascherati"]
print("  quello che esce, per ciascuno dei due:")
scoperti_py, scoperti_rs, illeggibili = [], [], []
for testo, rs in zip(TESTI, fuori):
    pyt = py.senza_chiavi(testo)
    for s in SEGRETI:
        if s in testo:
            if s in pyt:
                scoperti_py.append((testo, s))
            if s in rs:
                scoperti_rs.append((testo, s))
    # I testi innocui devono restare leggibili da tutte e due le parti.
    if not any(s in testo for s in SEGRETI) and testo:
        if pyt != testo:
            illeggibili.append(("py", testo, pyt))
        if rs != testo:
            illeggibili.append(("rs", testo, rs))

controlla("nessun segreto sopravvive dal lato Python", not scoperti_py,
          str(scoperti_py[:2]))
controlla("nessun segreto sopravvive dal lato Rust", not scoperti_rs,
          str(scoperti_rs[:2]))
controlla("e i messaggi innocui restano leggibili", not illeggibili,
          str(illeggibili[:2]))

# La direzione conta: coprire di piu' e' un fastidio, coprire di meno e' una
# chiave che esce. Si controlla che il Rust non copra MENO.
meno = []
for testo, rs in zip(TESTI, fuori):
    pyt = py.senza_chiavi(testo)
    coperti_py = pyt.count("[chiave]")
    coperti_rs = rs.count("[chiave]")
    if coperti_rs < coperti_py:
        meno.append((testo, pyt, rs))
controlla("il Rust non copre mai meno del Python", not meno, str(meno[:2]))

# E una prova che non guarda le due implementazioni ma il risultato: qualunque
# cosa somigli a una chiave, dopo, non c'e' piu'.
print("\n=== La prova che non si fida di nessuno dei due ===")
import re                                                    # noqa: E402
SOSPETTO = re.compile(r"(sk-|gsk_|xai-|AIza)[A-Za-z0-9_\-]{8,}")
rimasti = [(t, x) for t, x in zip(TESTI, fuori) if SOSPETTO.search(x)]
rimasti += [(t, py.senza_chiavi(t)) for t in TESTI if SOSPETTO.search(py.senza_chiavi(t))]
controlla("dopo il mascheramento non resta niente che somigli a una chiave",
          not rimasti, str(rimasti[:2]))

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
