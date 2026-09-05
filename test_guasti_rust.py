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

# Il corpus e' cresciuto, e non per completezza: per un difetto.
#
# Le due implementazioni avevano due elenchi di forme **diversi**, e questo
# confronto restava verde perche' chiedeva solo delle quattro forme che
# conoscevano tutte e due. Fuori dal corpus, il Python lasciava scoperta la
# firma del JWT e il Rust non copriva affatto le chiavi AWS, i token Slack e
# GitHub, i blocchi di chiave privata, le credenziali dentro un indirizzo e i
# numeri di carta. Un confronto vale quanto le domande che fa.
SEGRETI = [
    "sk-abcd1234efgh5678ijkl",
    "gsk_abcd1234efgh5678ijkl",
    "xai-abcd1234efgh5678ijkl",
    "AIzaAbcd1234efgh5678ijkl",
    "abcdefghijklmnop1234567890",
    "ghp_abcdefghijklmnopqrstuvwxyz123456",
    "xoxb-1234567890-abcdefghijkl",
    "AKIA1234567890ABCDEF",
    "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0In0.abcdef",
    "4111 1111 1111 1111",
    "4111-1111-1111-1111",
    # Anche questi sono segreti, e vanno dichiarati tali: cio' che non e' in
    # questo elenco la prova lo considera innocuo, e pretende che resti
    # leggibile. Dimenticarne uno qui vuol dire pretendere che un segreto
    # NON venga coperto — il contrario di quello che serve.
    "utente:segreto@example.com",
    "BEGIN RSA PRIVATE KEY",
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
    # Le forme che una sola delle due parti conosceva.
    "token ghp_abcdefghijklmnopqrstuvwxyz123456 rifiutato",
    "webhook xoxb-1234567890-abcdefghijkl non valido",
    "AKIA1234567890ABCDEF non autorizzata",
    "connessione a https://utente:segreto@example.com/db fallita",
    "-----BEGIN RSA PRIVATE KEY----- non leggibile",
    "jwt eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0In0.abcdef scaduto",
    "pagamento con 4111 1111 1111 1111 rifiutato",
    "pagamento con 4111-1111-1111-1111 rifiutato",
    # E i casi che NON devono essere coperti, o i messaggi diventano illeggibili.
    "la chiave non e' stata accettata",
    "token non valido",
    "ask-me di nuovo",
    "key: 1234",
    "connessione rifiutata su 127.0.0.1:8080",
    "HTTP 500 dal fornitore",
    "porta 8080 gia in uso",
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

print("\n6. il guardiano della memoria: cosa non entra nel vault")
# La domanda giusta non e' «le due meta' sono d'accordo?» — su questo si sono
# gia' trovate d'accordo nello sbagliare (D51). Le domande sono due:
#   e' rimasto fuori qualcosa che doveva essere rifiutato?
#   e' stato rifiutato qualcosa che NOVA doveva poter ricordare?
# La seconda pesa quanto la prima: un guardiano troppo severo non protegge,
# cancella la memoria.
from nova.kb.riservatezza import perche_non_si_salva as py_guardiano  # noqa: E402

DA_RIFIUTARE = [
    # forme che sono un segreto per come sono fatte
    ("chiave OpenAI", "la chiave e' sk-abcd1234efgh5678ijklmnop"),
    ("chiave Groq", "gsk_abcd1234efgh5678ijkl"),
    ("chiave xAI", "xai-abcd1234efgh5678ijkl"),
    ("chiave Google", "AIzaAbcd1234efgh5678ijkl"),
    ("token GitHub", "ghp_abcdefghijklmnopqrstuvwxyz123456"),
    ("token Slack", "xoxb-1234567890-abcdefghijkl"),
    ("chiave AWS", "AKIA1234567890ABCDEF"),
    ("chiave privata", "-----BEGIN RSA PRIVATE KEY-----"),
    ("jwt", "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0In0.abcdef"),
    ("credenziali in un indirizzo", "https://utente:segreto@example.com/db"),
    ("numero di carta", "la carta e' 4111 1111 1111 1111"),
    # e le forme che si riconoscono solo dall'etichetta
    ("password detta a voce", "la password del wifi e' Tramonto2026"),
    ("password con i due punti", "password: Tramonto2026!"),
    ("il pin, che e' corto per costruzione", "il pin del bancomat e' 4829"),
    ("bearer dentro un'intestazione",
     "Authorization: Bearer abcdefghijklmnop1234567890"),
    ("una seed phrase, che e' fatta di parole comuni",
     "la seed phrase e' cavallo batteria graffetta corretta"),
    ("passphrase", "passphrase: montagna azzurra lontana"),
    ("parola d'ordine dettata senza apostrofo",
     "la parola d ordine e' Girasole99"),
    ("chiave api con la precisazione in mezzo",
     "la api key del fornitore e' abcd1234efgh5678"),
    ("token con il valore due parole dopo",
     "il token di accesso e' 9f8e7d6c5b4a3210"),
]

DA_RICORDARE = [
    ("una frase sulla password, senza password",
     "Gio ha cambiato la password del wifi la settimana scorsa"),
    ("un token scaduto, detto e basta", "il token e' scaduto ieri"),
    ("una parola lunga che non e' un segreto",
     "la password e' segretissima secondo lui"),
    ("un fatto normale", "Gio lavora meglio la mattina presto"),
    ("un numero che non e' una carta", "il progetto ha 42 nodi"),
    ("una data", "la riunione e' il 04/09/2026"),
    ("un indirizzo senza credenziali", "https://example.com/pagina"),
    ("una parola che comincia come un prefisso", "ask-me di nuovo"),
    ("un nodo tecnico sul PC", "RTX 4060 Ti da 16 GB, 32 GB di RAM"),
    ("niente", ""),
]

TUTTI = [t for _, t in DA_RIFIUTARE] + [t for _, t in DA_RICORDARE]
giudizi = rust({"da_giudicare": TUTTI})["giudizi"]
suoi = dict(zip(TUTTI, giudizi))

passati_py, passati_rs = [], []
for nome, testo in DA_RIFIUTARE:
    if py_guardiano(testo) is None:
        passati_py.append(nome)
    if suoi[testo] is None:
        passati_rs.append(nome)
controlla("niente di segreto entra in memoria, dal lato Python",
          not passati_py, str(passati_py))
controlla("niente di segreto entra in memoria, dal lato Rust",
          not passati_rs, str(passati_rs))

rifiutati_py, rifiutati_rs = [], []
for nome, testo in DA_RICORDARE:
    if py_guardiano(testo) is not None:
        rifiutati_py.append((nome, py_guardiano(testo)))
    if suoi[testo] is not None:
        rifiutati_rs.append((nome, suoi[testo]))
controlla("e nessun ricordo legittimo viene buttato, dal lato Python",
          not rifiutati_py, str(rifiutati_py))
controlla("e nessun ricordo legittimo viene buttato, dal lato Rust",
          not rifiutati_rs, str(rifiutati_rs))

diverse = [f"{t[:40]!r}: rust {suoi[t]!r} vs python {py_guardiano(t)!r}"
           for t in TUTTI if suoi[t] != py_guardiano(t)]
controlla("e le due meta' danno lo stesso nome alla stessa cosa",
          not diverse, " | ".join(diverse[:3]))

# Il motivo non deve mai ripetere il valore: finisce in un registro.
ripetuti = [t[:40] for t in TUTTI
            if suoi[t] and any(pezzo in suoi[t] for pezzo in t.split() if len(pezzo) > 6)]
controlla("e il rifiuto non ripete mai quello che ha rifiutato",
          not ripetuti, str(ripetuti[:2]))

# --------------------------------------------------------------------------
# Un codice HTTP detto in italiano.
#
# E' il punto 7 dell'attrito: «non ci riesco» deve dire perche' e cosa fare.
# Ma c'e' anche una parte che non e' cortesia: il corpo della risposta puo'
# contenere la chiave rimandata indietro dal fornitore, e da li' finirebbe in
# chat e nel registro.
CHIAVE = "sk-proj-" + "A" * 40

CORPI = [
    "",
    "<html><body>502 Bad Gateway</body></html>",
    '{"codice": 12, "roba": [1, 2]}',
    '{"message": "modello sconosciuto"}',
    '{"error": {"message": "Incorrect API key provided: ' + CHIAVE + '"}}',
    '{"error": {"detail": "rate limit, riprova fra un minuto"}}',
    '{"error":{"error":{"error":{"error":"in fondo"}}}}',
    '{"error":{"error":{"error":{"error":{"error":"troppo in fondo"}}}}}',
    '{"message": "' + "a" * 500 + '"}',
    '{"error": {"message": "con accenti: perché la città è così"}}',
    '{"error":{"code":400,"message":"the request exceeds the available context '
    'size. try increasing the context size or enable context shift",'
    '"n_prompt_tokens":102953,"n_ctx":16384,"type":"exceed_context_size_error"}}',
    '{"error":"maximum context length"}',
    '{"error":{"n_prompt_tokens":"102953","n_ctx":"16384",'
    '"message":"context length exceeded"}}',
    '{"error":{"n_prompt_tokens":"tanti","n_ctx":16384,'
    '"message":"context length exceeded"}}',
    "image input is not supported - hint: if this is unexpected, you may need "
    "to provide the mmproj",
    '{"error": {"message": "MMPROJ missing"}}',
    '{"message": 42}',
    '{"message": null}',
    '{"message": ["a", "b"]}',
]

CODICI = [200, 400, 401, 402, 403, 404, 413, 429, 499, 500, 502, 503, 599]
HTTP = [(c, corpo, "Il fornitore") for c in CODICI for corpo in CORPI[:6]]
HTTP += [(400, CORPI[10], "Il fornitore"), (400, CORPI[11], "Il fornitore"),
         (400, CORPI[12], "Il fornitore"), (400, CORPI[13], "Il fornitore"),
         (500, CORPI[14], "llama-server"), (502, CORPI[15], "Il fornitore"),
         (401, CORPI[4], "OpenRouter")]
NUMERI = [0, 1, 12, 999, 1000, 1234, 16384, 102953, 1000000, -4321]

suo = rust({"http": [list(x) for x in HTTP], "corpi": CORPI, "numeri": NUMERI})

diverse = [f"{c} {corpo[:30]!r}: rust {r[:70]!r} vs python {py.spiega_http(c, corpo, dove)[:70]!r}"
           for (c, corpo, dove), r in zip(HTTP, suo["http"])
           if r != py.spiega_http(c, corpo, dove)]
controlla(f"le {len(HTTP)} spiegazioni HTTP sono identiche", not diverse,
          " | ".join(diverse[:2]))

diverse = [f"{c[:40]!r}: rust {r!r} vs python {py._motivo_del_fornitore(c)!r}"
           for c, r in zip(CORPI, suo["motivi"]) if r != py._motivo_del_fornitore(c)]
controlla(f"i {len(CORPI)} motivi estratti sono identici", not diverse,
          " | ".join(diverse[:2]))

diverse = [f"{c[:40]!r}" for c, r in zip(CORPI, suo["senza_vista"])
           if r != py.senza_vista(c)]
controlla("e il giudizio «questo modello non vede» pure", not diverse, str(diverse[:2]))

diverse = [f"{c[:40]!r}" for c, r in zip(CORPI, suo["contesto_sfondato"])
           if r != py._contesto_sfondato(c)]
controlla("e il giudizio «il contesto non basta» pure", not diverse, str(diverse[:2]))

diverse = [f"{c[:40]!r}: rust {tuple(r)} vs python {py._misure_del_contesto(c)}"
           for c, r in zip(CORPI, suo["misure"]) if tuple(r) != py._misure_del_contesto(c)]
controlla("e i due numeri del contesto pure", not diverse, " | ".join(diverse[:2]))

diverse = [f"{n}: rust {r!r} vs python {f'{n:,}'.replace(',', '.')!r}"
           for n, r in zip(NUMERI, suo["migliaia"])
           if r != f"{n:,}".replace(",", ".")]
controlla("e le migliaia si scrivono col punto, come in Python", not diverse,
          " | ".join(diverse[:2]))

# La domanda che conta piu' di tutte le altre messe insieme.
tutte = suo["http"] + suo["motivi"]
perde = [x[:60] for x in tutte if "sk-proj-" in x or "A" * 20 in x]
controlla("e la chiave non compare in NESSUNA delle frasi prodotte",
          not perde, str(perde[:2]))
controlla("il banco ha davvero un corpo con la chiave dentro",
          any(CHIAVE in c for c in CORPI),
          "senza, la verifica qui sopra non prova niente")

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
