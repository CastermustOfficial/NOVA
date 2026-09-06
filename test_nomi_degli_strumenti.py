# -*- coding: utf-8 -*-
"""Ogni strumento nominato deve esistere, e col nome giusto.

E' la classe di difetti piu' silenziosa che NOVA abbia avuto. Un nome
sbagliato in un elenco di permessi, o dentro il prompt, **non da' nessun
errore**: da' un cervello a cui manca una capacita' e che non sa dire perche'.

E' gia' successo due volte. Senza `Read` fra i permessi, NOVA scattava
schermate che non poteva guardare — `Read` e' anche cio' che apre le
immagini, e il file finiva su disco senza che il modello lo vedesse mai. E i
nomi delle capacita' del demone hanno l'**underscore** una volta esposti via
MCP (`ui_find`), mentre dentro il demone si chiamano col punto (`ui.find`):
cercarli col punto non trova niente, e il prompt lo dice apposta.

Qui si confrontano tre elenchi di nomi *scritti* con tre elenchi di nomi
*veri*:

- `--allowedTools`, l'elenco dei permessi passato a Claude Code;
- il prompt di sistema, che insegna al cervello quali strumenti ha;
- il suggerimento sulla memoria, che ne nomina altri.

Contro: gli strumenti del server MCP di NOVA, le capacita' del demone, e i
nativi di Claude Code — questi ultimi dichiarati qui, perche' non sono nostri
e non c'e' nessun posto da cui leggerli.
"""
import ast
import io
import re
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
CRATES = RADICE / "core" / "crates"
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from nova.config import REGOLE_OPERATIVE                      # noqa: E402
from nova.mcp_kb import STRUMENTI                             # noqa: E402

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


#: I nativi di Claude Code. Non sono nostri: non c'e' nessun elenco da cui
#: leggerli, quindi stanno scritti qui col loro perche'.
#:
#: «L'unico posto in cui un nome di strumento e' ricopiato a mano» era una
#: frase che avevo scritto e che era **falsa**: gli stessi nomi stanno anche
#: dentro `mcp_kb._rischio`, che classifica le richieste di permesso che
#: arrivano da Claude Code. Adesso il confronto c'e' (sezione 6), cosi' la
#: frase e' vera perche' e' controllata, non perche' l'ho detta.
#: `Task` e `TodoWrite` li avevo messi qui a memoria, e la prova al contrario
#: — quella che cerca i nomi che **nessuno** nomina — li ha tolti subito:
#: nessuno li permette e nessuno li classifica. Un elenco di riferimento che
#: cresce a intuizione smette di essere un riferimento.
NATIVI = {"Read", "Glob", "Grep", "WebSearch", "WebFetch", "Write", "Edit",
          "Bash", "MultiEdit", "NotebookRead", "NotebookEdit"}

# ----------------------------------------------------------- i nomi veri
NOSTRI = {s["name"] for s in STRUMENTI}

CAPACITA = set()
for f in sorted((CRATES / "nova-core" / "src").glob("caps*.rs")):
    CAPACITA |= set(re.findall(r'name:\s*"([a-z_]+\.[a-z_]+)"',
                               f.read_text(encoding="utf-8", errors="replace")))

print("\n1. gli elenchi di nomi veri non sono vuoti")
controlla(f"il server MCP di NOVA espone {len(NOSTRI)} strumenti", len(NOSTRI) > 20)
controlla(f"il demone dichiara {len(CAPACITA)} capacita'", len(CAPACITA) > 30)


def esiste(nome: str) -> bool:
    """Se questo nome, scritto come si scrive in un permesso, esiste."""
    if nome in NATIVI:
        return True
    pezzi = nome.split("__")
    if len(pezzi) == 2:
        # «mcp__nova-core» senza terzo pezzo: e' tutto il server.
        return pezzi[0] == "mcp" and pezzi[1] in ("nova", "nova-core")
    if len(pezzi) != 3 or pezzi[0] != "mcp":
        return False
    server, strumento = pezzi[1], pezzi[2]
    if server == "nova":
        return strumento in NOSTRI
    if server == "nova-core":
        # Esposti via MCP col trattino basso, dichiarati col punto.
        return strumento.replace("_", ".", 1) in CAPACITA
    return False


# ------------------------------------------------------ i nomi scritti
def costante_rust(percorso: Path, nome: str) -> str:
    testo = percorso.read_text(encoding="utf-8", errors="replace")
    m = re.search(rf'pub const {nome}: &str = "((?:[^"\\]|\\.)*)";', testo)
    if not m:
        return ""
    return m.group(1).replace("\\n", "\n").replace('\\"', '"').replace("\\\\", "\\")


DICH = CRATES / "nova-cervelli" / "src" / "dichiarazioni.rs"
PERMESSI = [x for x in costante_rust(DICH, "STRUMENTI_PERMESSI").split(",") if x]
HINT = costante_rust(DICH, "HINT_MCP")
SPORTELLO = costante_rust(DICH, "SPORTELLO_PERMESSI")

print(f"\n2. i {len(PERMESSI)} nomi dell'elenco dei permessi")
fantasmi = [n for n in PERMESSI if not esiste(n)]
controlla("ognuno di loro esiste davvero", not fantasmi,
          f"{fantasmi}  <- un nome sbagliato qui non da' errore: da' una "
          "capacita' che manca e nessuno sa perche'")
controlla("l'elenco non ha doppioni", len(PERMESSI) == len(set(PERMESSI)),
          str([n for n in PERMESSI if PERMESSI.count(n) > 1]))
controlla("nessun nome ha spazi attorno", all(n == n.strip() for n in PERMESSI),
          str([n for n in PERMESSI if n != n.strip()]))
controlla("c'e' `Read`, che e' anche cio' che apre le immagini",
          "Read" in PERMESSI,
          "senza, NOVA scatta schermate che non puo' guardare")
controlla("e lo sportello dei permessi", SPORTELLO in PERMESSI,
          f"{SPORTELLO!r}: senza, chiedere un permesso non arriva a nessuno")

print("\n3. i nomi scritti nel prompt e nei suggerimenti")
SCRITTI: dict[str, list[str]] = {}
for dove, testo in [("le regole operative", REGOLE_OPERATIVE),
                    ("il suggerimento sulla memoria", HINT)]:
    for n in re.findall(r"mcp__[A-Za-z0-9_.-]+", testo):
        n = n.rstrip(".,;:)»`")
        SCRITTI.setdefault(n, []).append(dove)
controlla(f"il prompt nomina {len(SCRITTI)} strumenti", len(SCRITTI) >= 10,
          str(len(SCRITTI)))
fantasmi = sorted(f"{n} (in {SCRITTI[n][0]})" for n in SCRITTI if not esiste(n))
controlla("e ognuno di loro esiste", not fantasmi,
          f"{fantasmi}  <- il modello prova a chiamarlo e non lo trova")

print("\n4. e cio' che il prompt insegna, il cervello ha il permesso di usarlo")
# Insegnare uno strumento che poi e' vietato e' peggio che non insegnarlo:
# il modello ci prova, si vede rifiutare, e non ha modo di capire che il
# problema non e' la sua richiesta.
def permesso(n: str) -> bool:
    if n in PERMESSI:
        return True
    server = "__".join(n.split("__")[:2])
    return server in PERMESSI


vietati = sorted(n for n in SCRITTI if not permesso(n))
controlla("nessuno strumento insegnato e' fuori dai permessi", not vietati,
          f"{vietati}  <- il modello ci prova e si vede rifiutare, senza "
          "capire che non e' colpa della sua richiesta")

print("\n5. e i nomi del demone hanno il trattino basso, non il punto")
# La regola che il prompt dichiara apposta, controllata invece che
# raccomandata: col punto non si trova niente.
col_punto = sorted(n for n in list(SCRITTI) + PERMESSI
                   if n.startswith("mcp__nova-core__") and "." in n)
controlla("nessun nome del demone e' scritto col punto", not col_punto,
          str(col_punto))
# La regola non basta che valga: chi la deve seguire e' un modello, e la
# segue solo se gliela si dice. Puo' stare nelle regole operative o nel
# suggerimento sulla memoria — sono tutti e due testi che legge.
avvisato = any(x in (REGOLE_OPERATIVE + HINT)
               for x in ("underscore", "trattino basso", "col punto"))
controlla("e il prompt lo dice a chi legge", avvisato,
          "la regola vale solo se chi la deve seguire la legge")

print("\n6. e l'altro elenco di nativi dice le stesse cose di questo")
# `mcp_kb._rischio` classifica le richieste di permesso che arrivano **da**
# Claude Code, e per farlo nomina i suoi strumenti nativi. E' un secondo
# elenco scritto a mano, ed e' esattamente la forma di difetto che oggi ha
# gia' morso tre volte: due elenchi separati sanno cose diverse (D113).
albero = ast.parse(io.open(RADICE / "nova" / "mcp_kb.py", encoding="utf-8-sig").read())
DA_RISCHIO: set[str] = set()
for f in ast.walk(albero):
    if isinstance(f, ast.FunctionDef) and f.name == "_rischio":
        for c in ast.walk(f):
            if isinstance(c, (ast.Tuple, ast.List, ast.Set)):
                for e in c.elts:
                    if isinstance(e, ast.Constant) and isinstance(e.value, str):
                        DA_RISCHIO.add(e.value)
controlla(f"il classificatore dei permessi nomina {len(DA_RISCHIO)} nativi",
          len(DA_RISCHIO) >= 5, str(sorted(DA_RISCHIO)))
sconosciuti = sorted(DA_RISCHIO - NATIVI)
controlla("e ognuno di loro e' fra quelli che questa prova conosce",
          not sconosciuti,
          f"{sconosciuti}  <- due elenchi separati sanno cose diverse: o e' "
          "un nativo e va in NATIVI, o non lo e' e li' non ci va")
# E al contrario: NATIVI non deve diventare una lista dei desideri.
mai_usati = sorted(n for n in NATIVI
                   if n not in DA_RISCHIO and n not in PERMESSI)
controlla("e questa prova non conosce nativi che non nomina nessuno",
          not mai_usati,
          f"{mai_usati}  <- nessuno li permette e nessuno li classifica: "
          "toglierli, o scoprire chi doveva nominarli")

print("\n7. e ogni strumento dichiarato dal server ha chi lo esegue")
# Un nome nell'elenco `tools/list` senza un gestore dietro e' uno strumento
# che il programma dall'altra parte vede, chiama, e si sente rispondere
# «sconosciuto». Un gestore senza dichiarazione e' l'opposto: una cosa che
# NOVA sa fare e che nessuno le chiedera' mai, perche' non l'ha detto.
alb = ast.parse(io.open(RADICE / "nova" / "mcp_kb.py", encoding="utf-8-sig").read())
GESTITI: list[str] = []
for n in ast.walk(alb):
    if (isinstance(n, ast.Dict) and n.keys
            and all(isinstance(k, ast.Constant) and isinstance(k.value, str)
                    for k in n.keys)
            and all(isinstance(v, ast.Attribute)
                    and getattr(v.value, "id", "") == "self" for v in n.values)):
        GESTITI = [k.value for k in n.keys]
controlla(f"il server smista {len(GESTITI)} nomi", len(GESTITI) > 20,
          "non ho ritrovato la tabella di smistamento")
orfani = sorted(NOSTRI - set(GESTITI))
controlla("ogni strumento dichiarato ha chi lo esegue", not orfani,
          f"{orfani}  <- l'altro programma lo vede, lo chiama, e si sente "
          "rispondere «sconosciuto»")
muti = sorted(set(GESTITI) - NOSTRI)
controlla("e ogni gestore e' dichiarato", not muti,
          f"{muti}  <- NOVA lo sa fare e nessuno glielo chiedera' mai")

senza_descrizione = sorted(s["name"] for s in STRUMENTI
                           if len((s.get("description") or "").strip()) < 20)
controlla("e ognuno dice a cosa serve", not senza_descrizione,
          f"{senza_descrizione}  <- il modello sceglie gli strumenti leggendo "
          "questo, non il nome")
senza_schema = sorted(s["name"] for s in STRUMENTI if not s.get("inputSchema"))
controlla("e come si chiama", not senza_schema, str(senza_schema))

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
