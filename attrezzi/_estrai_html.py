# -*- coding: utf-8 -*-
"""Come sono nate in Rust le regole che leggono l'HTML.

Due famiglie di dichiarazioni, e nessuna delle due si puo' ricopiare a mano.

**Le entita' HTML.** `html.unescape` conosce duemiladuecento nomi: `&amp;`,
ma anche `&hellip;`, `&rsquo;`, `&mdash;` — quelli che nei titoli delle
pagine ci sono davvero. Una tabella scritta a mano ne conterrebbe venti, e le
altre resterebbero visibili come `&hellip;` dentro il titolo che NOVA mostra:
non un errore, un titolo sbagliato. Peggio ancora, nessuno se ne accorgerebbe
guardando i test, perche' i test li scrive chi ha scritto la tabella e
useranno le stesse venti (D113).

**Le espressioni regolari.** Quelle di `nova/html_a_testo.py` e i due
raschiatori di `nova/tools/web.py` sono il confine fra «testo della pagina» e
«codice della pagina». Si estraggono dal sorgente — le prime dagli oggetti
compilati, le seconde dall'albero sintattico, perche' vivono dentro le
funzioni — e non si riscrivono (D112).

Scrive `core/crates/nova-browser/src/entita.rs` e
`core/crates/nova-browser/src/regole.rs`. Si verifica da solo: rilegge cio'
che ha scritto e lo riconfronta con Python.
"""
import ast
import html
import html.entities
import io
import re
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

DOVE = RADICE / "core" / "crates" / "nova-browser" / "src"


def rust(t: str) -> str:
    """Un letterale Rust che non dipende da come il file viene letto."""
    fuori = ['"']
    for c in t:
        if c == '"':
            fuori.append('\\"')
        elif c == "\\":
            fuori.append("\\\\")
        elif c == "\n":
            fuori.append("\\n")
        elif c == "\r":
            fuori.append("\\r")
        elif c == "\t":
            fuori.append("\\t")
        elif " " <= c <= "~":
            fuori.append(c)
        else:
            fuori.append("\\u{%x}" % ord(c))
    fuori.append('"')
    return "".join(fuori)


# ------------------------------------------------------------------ entita'
nomi = sorted(html.entities.html5.items())
sbagliati = sorted(html._invalid_charrefs.items())
vietati = sorted(html._invalid_codepoints)

righe = [
    """//! I nomi delle entita' HTML, come li conosce `html.unescape`.
//!
//! **Generato da `_estrai_html.py`. Non si scrive a mano.** Sono i nomi che
//! lo standard HTML5 definisce: duemiladuecento, non venti. Una tabella
//! parziale non darebbe un errore — lascerebbe `&hellip;` dentro il titolo
//! che NOVA mostra, e nessuno se ne accorgerebbe (D113).
//!
//! I nomi ci sono in due forme, `amp` e `amp;`, perche' cosi' sono nello
//! standard: senza punto e virgola valgono solo per un elenco chiuso, e chi
//! scioglie deve poter distinguere.
//!
//! Ordinati: si cercano per bisezione.

/// Nome dell'entita' (senza `&`) e cosa vuol dire.
pub static NOMI: [(&str, &str); %d] = [""" % len(nomi)
]
for k, v in nomi:
    righe.append("    (%s, %s)," % (rust(k), rust(v)))
righe.append("];\n")

righe.append(
    """/// I numeri che lo standard dice di **non** leggere come tali.
///
/// Sono i posti dove Windows-1252 metteva le virgolette curve e il trattino
/// lungo. Le pagine li scrivono ancora, e `&#146;` va letto come apostrofo,
/// non come il carattere di controllo numero 146.
pub static SBAGLIATI: [(u32, &str); %d] = [""" % len(sbagliati))
for k, v in sbagliati:
    righe.append("    (%d, %s)," % (k, rust(v)))
righe.append("];\n")

righe.append(
    """/// I numeri che non danno niente: si sciolgono nel nulla.
pub static VIETATI: [u32; %d] = [""" % len(vietati))
for n in vietati:
    righe.append("    %d," % n)
righe.append("];")

(DOVE / "entita.rs").write_text("\n".join(righe) + "\n", encoding="utf-8", newline="\n")

# ----------------------------------------------------------- le espressioni
sorgente = io.open(RADICE / "nova" / "html_a_testo.py", encoding="utf-8").read()
spazio: dict = {}
exec(compile(sorgente, "html_a_testo.py", "exec"), spazio)

DA_MODULO = ["_INVISIBILE", "_A_CAPO", "_TAG", "_SPAZI", "_VUOTE", "_TITOLO"]

# I raschiatori di DuckDuckGo: due espressioni dichiarate accanto alla
# funzione, e una che vive ancora dentro `_ddg_lite`. Si prendono tutte e tre
# dall'albero sintattico, che e' l'unico modo di leggerle senza importare il
# modulo (che tirerebbe dentro `requests`).
web = ast.parse(io.open(RADICE / "nova" / "tools" / "web.py", encoding="utf-8").read())
DA_WEB = {}
for n in web.body:
    if (isinstance(n, ast.Assign) and isinstance(n.value, ast.Call)
            and isinstance(n.value.func, ast.Attribute)
            and n.value.func.attr == "compile"
            and getattr(n.value.func.value, "id", "") == "re"):
        DA_WEB[n.targets[0].id] = ast.literal_eval(n.value.args[0])
for f in ast.walk(web):
    if isinstance(f, ast.FunctionDef) and f.name == "_ddg_lite":
        for c in ast.walk(f):
            if (isinstance(c, ast.Call) and isinstance(c.func, ast.Attribute)
                    and c.func.attr == "findall"
                    and getattr(c.func.value, "id", "") == "re"):
                DA_WEB["_ddg_lite"] = ast.literal_eval(c.args[0])
                break
DAL_WEB = ["_RISULTATO", "_RIASSUNTO", "_ddg_lite"]
mancano = [n for n in DAL_WEB if n not in DA_WEB]
if mancano:
    print("non ho ritrovato le espressioni di:", ", ".join(mancano))
    sys.exit(1)

# `_INVISIBILE` usa un riferimento all'indietro (`</\1>`), che il motore di
# Rust non ha. Si espande nelle alternative che il gruppo elenca — e non a
# mano: i nomi si leggono dall'espressione stessa, e il file scritto tiene
# anche l'originale, cosi' una prova puo' controllare che l'elenco sia lo
# stesso.
originale = spazio["_INVISIBILE"].pattern
gruppo = re.search(r"<\((.*?)\)\[", originale).group(1)
TAG = gruppo.split("|")
espanso = "(?is)" + "|".join(f"<{t}[^>]*>.*?</{t}>" for t in TAG)

PERCHE = {
    "_INVISIBILE": ("Quello che sta dentro non e' testo della pagina: e' codice, stile,\n"
                    "/// o roba che il browser non mostra."),
    "_A_CAPO": ("I tag che, chiudendosi, mandano a capo. Senza, un elenco di dieci\n"
                "/// voci diventa una riga sola e il modello non vede piu' dove finisce\n"
                "/// una."),
    "_TAG": "Tutto il resto dei tag.",
    "_SPAZI": ("Gli spazi che si schiacciano. `\\xa0` e' lo spazio unificatore: sulle\n"
               "/// pagine c'e' dappertutto, e lasciarlo vuol dire mettere nel contesto\n"
               "/// del modello un carattere che sembra uno spazio e non lo e'."),
    "_VUOTE": "Tre a capo o piu' diventano due.",
    "_TITOLO": "Il titolo dichiarato dalla pagina.",
    "_RISULTATO": ("Un risultato dentro la pagina «html» di DuckDuckGo: il\n"
                   "/// collegamento che porta il titolo. Il riassunto e' un'altra\n"
                   "/// espressione apposta, perche' dentro la stessa scavalcava il\n"
                   "/// risultato successivo e se lo portava via (D181)."),
    "_RIASSUNTO": ("Il riassunto, che vale solo **dentro la finestra** di un\n"
                   "/// risultato: fuori di li' e' di un altro."),
    "_ddg_lite": "I risultati dentro la pagina «lite», che ha un'altra forma.",
}

pezzi = ["""//! Le regole che dicono cosa, in una pagina, e' testo.
//!
//! **Generato da `_estrai_html.py`, poi mantenuto a mano.** Sono le stesse
//! espressioni regolari che gira Python — estratte, non riscritte (D112):
//! quelle di `nova/html_a_testo.py` dagli oggetti gia' compilati, quelle dei
//! due raschiatori dall'albero sintattico di `nova/tools/web.py`, perche'
//! vivono dentro le funzioni.
//!
//! Un carattere di differenza qui non da' un errore: da' il testo di un altro
//! pezzo di pagina.
"""]
for nome in DA_MODULO:
    p = spazio[nome].pattern
    pezzi.append("/// %s\npub const %s: &str = %s;" % (PERCHE[nome], nome.lstrip("_"), rust(p)))

pezzi.append(
    "/// `INVISIBILE` senza il riferimento all'indietro, che il motore di Rust\n"
    "/// non ha: le stesse alternative, scritte per esteso.\n"
    "pub const INVISIBILE_ESPANSO: &str = %s;" % rust(espanso))
pezzi.append(
    "/// I nomi dei tag che `INVISIBILE` elenca. Una prova controlla che\n"
    "/// `INVISIBILE_ESPANSO` li contenga tutti e nessun altro.\n"
    "pub static INVISIBILI: [&str; %d] = [%s];"
    % (len(TAG), ", ".join(rust(t) for t in TAG)))

pezzi.append(
    "/// Come `html.unescape` riconosce un riferimento: per nome, per numero\n"
    "/// decimale, per numero esadecimale — e il punto e virgola e' facoltativo.\n"
    "/// Estratta da `html._charref`, non riscritta.\n"
    "pub const CHARREF: &str = %s;" % rust(html._charref.pattern))

for nome in DAL_WEB:
    # Python passa i flag a parte; in Rust si scrivono dentro l'espressione.
    pezzi.append("/// %s\npub const %s: &str = %s;"
                 % (PERCHE[nome], nome.lstrip("_").upper(), rust("(?is)" + DA_WEB[nome])))

# Il pezzo di indirizzo che dice «questo e' un rimbalzo»: non e'
# un'espressione, ma e' una dichiarazione, e vale la stessa regola.
for n in web.body:
    if isinstance(n, ast.Assign) and getattr(n.targets[0], "id", "") == "_RIMBALZO":
        RIMBALZO = ast.literal_eval(n.value)
        break
else:
    print("non ho ritrovato _RIMBALZO")
    sys.exit(1)
pezzi.append("/// Il pezzo di indirizzo che dice «questo e' un rimbalzo».\n"
             "pub const RIMBALZO: &str = %s;" % rust(RIMBALZO))

(DOVE / "regole.rs").write_text("\n\n".join(pezzi) + "\n", encoding="utf-8")

# ------------------------------------------------------------- si ricontrolla
guai = []
riletto = (DOVE / "entita.rs").read_text(encoding="utf-8")
if riletto.count("\n    (") != len(nomi) + len(sbagliati):
    guai.append("le coppie scritte non sono quelle contate")
for k, v in [nomi[0], nomi[len(nomi) // 2], nomi[-1]]:
    if "(%s, %s)," % (rust(k), rust(v)) not in riletto:
        guai.append(f"entita' {k!r} non ritrovata")

riletto = (DOVE / "regole.rs").read_text(encoding="utf-8")
for nome in DA_MODULO:
    if "pub const %s: &str = %s;" % (nome.lstrip("_"), rust(spazio[nome].pattern)) not in riletto:
        guai.append(f"{nome}: non ritrovata identica nel file scritto")
for nome in DAL_WEB:
    atteso = rust("(?is)" + DA_WEB[nome])
    if "pub const %s: &str = %s;" % (nome.lstrip("_").upper(), atteso) not in riletto:
        guai.append(f"{nome}: non ritrovata identica nel file scritto")
if "pub const CHARREF: &str = %s;" % rust(html._charref.pattern) not in riletto:
    guai.append("CHARREF: non ritrovata identica nel file scritto")
# L'espansione deve dire la stessa cosa dell'originale su del testo vero.
prova = ("<p>a<script x>via</script>b<STYLE>c</STYLE>d<head><title>t</title></head>"
         "<svg><path/></svg><template>z</template><noscript>n</noscript>e")
if re.sub(espanso.replace("(?is)", ""), " ", prova, flags=re.I | re.S) != \
        spazio["_INVISIBILE"].sub(" ", prova):
    guai.append("l'espansione di INVISIBILE non dice la stessa cosa dell'originale")

if guai:
    print("NON scritto bene:")
    for g in guai:
        print("  -", g)
    sys.exit(1)

print(f"scritto {DOVE / 'entita.rs'}")
print(f"  {len(nomi)} nomi, {len(sbagliati)} numeri sbagliati, {len(vietati)} vietati")
print(f"scritto {DOVE / 'regole.rs'}")
for nome in DA_MODULO:
    print(f"  {nome.lstrip('_'):16s} {len(spazio[nome].pattern):4d} caratteri")
for nome in DAL_WEB:
    print(f"  {nome.lstrip('_').upper():16s} {len(DA_WEB[nome]):4d} caratteri")
print(f"  {'CHARREF':16s} {len(html._charref.pattern):4d} caratteri")
