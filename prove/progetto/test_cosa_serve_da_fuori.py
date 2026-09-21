# -*- coding: utf-8 -*-
"""Quello che NOVA chiede al mondo fuori, e cosa dice quando non c'e'.

La prima delle cinque frasi del cancello della beta e' «qualcuno che non e'
l'autore l'ha installato, su una macchina che non e' quella». Questa prova
non puo' installare NOVA altrove, ma puo' fare l'unica cosa che qui si puo'
fare: **guardare cosa NOVA dice a chi non ha gia' tutto**.

Su questa macchina c'e' tutto. Ed e' esattamente il motivo per cui i tre
messaggi peggiori del progetto erano quelli che vede solo chi non ha Chrome,
non ha llama-server, non ha un modello: nessuno li aveva mai letti.

La regola di casa esiste gia', ed e' buona — «manca sounddevice: si installa
con pip install sounddevice», «Claude Code non trovato. Installalo con: npm
install -g ...», «Non trovo l'orb: se hai installato con install.ps1
dovrebbe stare in bin\\». Un messaggio che dice cosa manca **e cosa fare**.
Qui quella regola smette di essere una consuetudine e diventa un controllo.

Due cose si provano:

1. l'elenco di cio' che NOVA chiede al mondo fuori e' **dichiarato**, e ogni
   voce dice chi la usa e cosa succede senza;
2. ogni messaggio che annuncia un'assenza dice anche **cosa fare** — un
   comando, un percorso da indicare, o almeno cosa continua a funzionare.
"""
import ast
import io
import re
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
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


#: Cosa NOVA chiede al mondo fuori: il nome con cui compare nei messaggi, a
#: cosa serve, e cosa succede a chi non ce l'ha.
#:
#: Non e' documentazione: e' l'elenco su cui gira la prova qui sotto. Una
#: dipendenza nuova che non entra qui non viene controllata, ed e' per questo
#: che la seconda parte della prova va a cercare i messaggi che nominano
#: qualcosa che qui non c'e'.
DA_FUORI = {
    "pypdf": "leggere i PDF. Senza, gli altri formati si aprono lo stesso.",
    "python-docx": "leggere i .docx. Senza, gli altri formati si aprono lo stesso.",
    "openpyxl": "leggere i fogli di calcolo.",
    "mss": "catturare lo schermo.",
    "sounddevice": "sentire il microfono. Senza, NOVA legge e scrive ma non ascolta.",
    "faster-whisper": "trascrivere in casa, senza mandare l'audio fuori.",
    "PyQt6-WebEngine": "vedere una pagina resa nell'harness. Senza, resta il sorgente.",
    "Claude Code": "il cervello agentico. Senza, restano il locale e le API.",
    "Edge": "guidare una pagina web. Senza, la ricerca funziona lo stesso.",
    "Chrome": "guidare una pagina web. Senza, la ricerca funziona lo stesso.",
    "llama-server": "far girare il modello in casa. Senza, restano claude e le API.",
    "GGUF": "il file del cervello locale. Senza, restano claude e le API.",
    "nova-shell": "l'orb. Senza, NOVA si usa dalla riga di comando.",
    "ElevenLabs": "la voce e la trascrizione belle. Senza, c'e' quella di sistema.",
    "nvidia-smi": "sapere quanta VRAM c'e'. Senza, si tira a indovinare piu' basso.",
}

#: Come si riconosce che un messaggio dice **anche cosa fare**.
#:
#: O un comando da eseguire, o un posto dove mettere il percorso, o cosa
#: continua a funzionare lo stesso. Il terzo caso conta come cura: a chi ha
#: appena installato NOVA, sapere che non e' rotta e' meta' della risposta.
CURE = [
    r"pip install", r"npm install", r"install\.ps1", r"python -m nova",
    r"si installa con", r"[Ii]nstallal[oa]", r"si scarica", r"scaricat[oa]",
    r"si indica", r"dimmi dov", r"chiedimelo", r"esegui ", r"\bprova\b",
    r"funziona lo stesso", r"[Nn]el frattempo", r"restano", r"resta il",
    r"oppure", r"dovrebbe stare",
    # «normale se la scheda non e' NVIDIA» e' una cura: dice che non c'e'
    # niente da riparare, che a chi legge serve quanto un comando.
    r"normale se", r"non e' un problema", r"non serve a nessuno",
    # «uso la voce di sistema» e' una cura come un comando: dice cosa
    # succede invece, che e' cio' che chi legge deve sapere.
    r"\buso la\b", r"passo alla", r"si scende",
]

#: Messaggi che annunciano un'assenza **senza** cura, e con un motivo.
SENZA_CURA_CON_MOTIVO = {
    "manca una libreria, e non so dire quale.":
        "e' il ripiego di quando Python non dice il nome del modulo: non c'e' "
        "niente da suggerire, e inventarlo manderebbe a installare la cosa "
        "sbagliata. Il ramo sopra, quando il nome c'e', la cura ce l'ha.",
}


#: Come si riconosce che un messaggio **annuncia un'assenza**.
#:
#: Serve a separarlo da un messaggio che parla della stessa cosa per un
#: altro motivo: «Claude Code ha esaurito la quota» nomina Claude Code e non
#: dice che manca — li' non c'e' niente da installare.
ASSENZE = [r"non trov", r"\bmanca\b", r"\bmancano\b", r"non c'e'", r"non ce n'e'",
           r"non e' installat", r"non autenticat", r"non disponibil"]


def _docstring(alb) -> set[int]:
    """Le righe che sono docstring: prosa, non messaggi."""
    fuori = set()
    import ast as _a
    for n in _a.walk(alb):
        if isinstance(n, (_a.Module, _a.ClassDef, _a.FunctionDef, _a.AsyncFunctionDef)):
            corpo = getattr(n, "body", [])
            if (corpo and isinstance(corpo[0], _a.Expr)
                    and isinstance(corpo[0].value, _a.Constant)
                    and isinstance(corpo[0].value.value, str)):
                fuori.add(corpo[0].value.lineno)
    return fuori


def messaggi_di_assenza() -> list[tuple[str, int, str]]:
    """I testi che NOVA mostra quando qualcosa non c'e'.

    Si guardano **tutti** i testi, non solo quelli che si sollevano: alcune
    di queste cose non fanno fallire niente, lo dicono e basta — «senza
    PyQt6-WebEngine resta il sorgente» e' scritto, non lanciato. Guardare
    solo i `raise` vuol dire non vedere proprio i messaggi piu' gentili.

    E un messaggio interpolato si rimette **insieme** prima di leggerlo. Un
    `f"manca X: " "installa con pip install X"` per l'albero sintattico sono
    due pezzi: il primo dice che manca qualcosa e non dice cosa fare, il
    secondo dice cosa fare e non sembra un'assenza. Letti separati, un
    messaggio perfetto risulta rotto e uno rotto passa — l'ho visto fare tutte
    e due le cose nello stesso giro.
    """
    fuori = []
    for f in sorted(RADICE.joinpath("nova").rglob("*.py")):
        try:
            alb = ast.parse(io.open(f, encoding="utf-8-sig").read())
        except SyntaxError:
            continue
        prosa = _docstring(alb)
        interi = []
        dentro_un_intero = set()
        for n in ast.walk(alb):
            if not isinstance(n, ast.JoinedStr):
                continue
            pezzi = []
            for v in n.values:
                if isinstance(v, ast.Constant) and isinstance(v.value, str):
                    pezzi.append(v.value)
                    dentro_un_intero.add(id(v))
                else:
                    pezzi.append("{}")
            interi.append((n.lineno, "".join(pezzi)))
        for n in ast.walk(alb):
            if (isinstance(n, ast.Constant) and isinstance(n.value, str)
                    and id(n) not in dentro_un_intero and n.lineno not in prosa):
                interi.append((n.lineno, n.value))
        for riga, grezzo in interi:
            t = " ".join(grezzo.split())
            if not (12 < len(t) < 400):
                continue
            # «serve pypdf» e' un'assenza; «serve una conversazione piu'
            # corta» no. La differenza e' se dopo «serve» c'e' il nome di
            # una cosa che sta in DA_FUORI.
            e_assenza = any(re.search(a, t, re.IGNORECASE) for a in ASSENZE) or any(
                re.search(rf"\bserve\s+«?{re.escape(n)}", t, re.IGNORECASE)
                for n in DA_FUORI)
            if e_assenza:
                fuori.append((f.relative_to(RADICE).as_posix(), riga, t))
    return fuori


TUTTI = messaggi_di_assenza()

print("\n1. l'elenco di cio' che serve da fuori")
controlla(f"sono dichiarate {len(DA_FUORI)} cose", len(DA_FUORI) >= 12)
senza_perche = sorted(k for k, v in DA_FUORI.items() if len(v) < 20)
controlla("ognuna dice a cosa serve e cosa succede senza", not senza_perche,
          str(senza_perche))
controlla(f"e {len(TUTTI)} messaggi di assenza si leggono dal codice",
          len(TUTTI) > 40, str(len(TUTTI)))

print("\n2. chi annuncia un'assenza dice anche cosa fare")
# Solo i messaggi che parlano di **una cosa da fuori**: «manca il nome» e'
# un errore della richiesta, non dell'ambiente, e li' non c'e' niente da
# installare.
def parla_di_fuori(t: str) -> list[str]:
    """Tutte quelle che nomina, non la prima: «ne' Edge ne' Chrome» ne
    nomina due, e fermarsi alla prima lascia la seconda senza nessuno."""
    return [n for n in DA_FUORI
            if re.search(rf"\b{re.escape(n)}\b", t, re.IGNORECASE)]


def ha_una_cura(t: str) -> bool:
    return any(re.search(c, t) for c in CURE)


nudi = []
coperte = set()
for dove, riga, t in TUTTI:
    nomi = parla_di_fuori(t)
    if not nomi:
        continue
    coperte.update(nomi)
    if ha_una_cura(t) or t in SENZA_CURA_CON_MOTIVO:
        continue
    nudi.append(f"{dove}:{riga} «{t[:90]}»")
controlla("nessun messaggio dice solo cosa manca", not nudi,
          " | ".join(nudi[:3]) + "  <- dire cosa manca senza dire cosa fare "
          "lascia fermo chi legge" if nudi else "")

print("\n3. e l'elenco non invecchia in silenzio")
# Un pezzo dichiarato che nessun messaggio nomina piu' e' un pezzo che non
# serve piu', o un messaggio che e' stato riscritto e non lo nomina piu': in
# tutti e due i casi va guardato.
mai_nominate = sorted(set(DA_FUORI) - coperte)
controlla("ogni cosa dichiarata compare in almeno un messaggio", not mai_nominate,
          f"{mai_nominate}  <- o non serve piu', o il messaggio che la "
          "nominava e' stato riscritto")
controlla("e chi non ha la cura ha almeno il motivo",
          all(t in {x for _, _, x in TUTTI} for t in SENZA_CURA_CON_MOTIVO),
          "una deroga a un messaggio che non esiste piu' e' una deroga che "
          "nessuno rileggera'")

print("\n4. i tre casi che vede solo chi non ha gia' tutto")
# Sono i primi che incontra chi installa NOVA adesso, ed erano i tre senza
# cura: su questa macchina non li legge nessuno.
def cerca(pezzo: str) -> list[str]:
    return [t for _, _, t in TUTTI if pezzo.lower() in t.lower()]


for pezzo, cosa in (("Chrome", "guidare una pagina"),
                    ("llama-server", "il modello in casa"),
                    ("GGUF", "il cervello locale")):
    trovati = [t for t in cerca(pezzo) if ha_una_cura(t)]
    controlla(f"«{pezzo}» che manca dice cosa fare", bool(trovati),
              f"nessun messaggio su {cosa} spiega come rimediare")

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
