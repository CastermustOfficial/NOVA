# -*- coding: utf-8 -*-
"""Il ponte fra la pagina e il guscio regge da tutte e due le parti.

`invoke('nome')` e' l'unico modo che l'interfaccia ha di chiedere qualcosa al
Rust, ed e' una stringa: se il nome non esiste, o se un argomento si chiama
diversamente da come lo vuole la funzione, **non succede niente**. Nessun
errore rosso, nessuna riga di log: un bottone che si preme e non fa niente,
che e' il difetto piu' difficile da attribuire — sembra rotto il programma,
non il collegamento.

E' la stessa classe di D189, un piano piu' in la': la' erano nomi di
strumenti scritti a mano, qui sono nomi di comandi. La cura e' la stessa —
cercarli tutti e confrontarli con l'elenco vero, invece di fidarsi.

Tre direzioni, e tutte e tre sono difetti veri:

- la pagina chiama un comando che non e' registrato (il bottone morto);
- un comando e' registrato e non lo chiama nessuno (o e' codice morto, o e'
  una funzione che si e' persa per strada e nessuno se n'e' accorto);
- la pagina passa un argomento con un nome che la funzione non ha. Tauri
  accetta il camelCase per i parametri in snake_case, quindi il confronto va
  fatto sulle due forme, non su una.
"""
import re
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

SRC = RADICE / "core" / "crates" / "nova-shell" / "src"
UI = RADICE / "core" / "crates" / "nova-shell" / "ui"

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


#: Comandi registrati che nessuna pagina chiama, col perche'. Restare qui e'
#: una dichiarazione, non una scappatoia (D148) - e i due motivi buoni sono
#: molto diversi fra loro: uno e' «lo chiama il Rust», l'altro e' «non lo
#: chiama ancora nessuno, e lo sappiamo».
SENZA_CHIAMATE: dict[str, str] = {
    "mostra_chat":
        "la chiama il Rust, non la pagina: `bus.rs` la usa quando il demone "
        "manda qualcosa da mostrare. Registrata perche' e' la stessa cosa "
        "che farebbe un bottone.",
    "stato_orb":
        "aggancio dichiarato e non ancora collegato: il commento nel codice "
        "dice «la chiamera' il demone quando NOVA pensa, ascolta o parla», e "
        "finche' resta cosi' l'orb non cambia aspetto. Non e' codice morto "
        "per errore, e' un lavoro a meta' che qui resta visibile.",
}


def a_serpente(s: str) -> str:
    """`inCorso` -> `in_corso`. Tauri accetta tutte e due le forme."""
    return re.sub(r"([A-Z])", lambda m: "_" + m.group(1).lower(), s)


# -- cosa espone il guscio ------------------------------------------------
main = (SRC / "main.rs").read_text(encoding="utf-8-sig")
blocco = re.search(r"generate_handler!\[(.*?)\]", main, re.S)
registrati: set[str] = set()
if blocco:
    for voce in blocco.group(1).split(","):
        voce = re.sub(r"//.*", "", voce).strip()
        if voce:
            registrati.add(voce.split("::")[-1])

controlla("i comandi registrati si leggono", len(registrati) > 10,
          f"trovati {len(registrati)}: l'estrattore non estrae piu' niente")

# -- e con che argomenti --------------------------------------------------
#: nome del comando -> i parametri che accetta (senza quelli del guscio).
DEL_GUSCIO = {"app", "window", "webview", "state", "handle"}
firme: dict[str, set[str]] = {}
for f in sorted(SRC.rglob("*.rs")):
    testo = f.read_text(encoding="utf-8-sig")
    for m in re.finditer(r"#\[tauri::command\][^\n]*\n(?:[^\n]*\n)?\s*(?:pub\s+)?(?:async\s+)?fn\s+(\w+)\s*\(([^)]*)\)", testo):
        nome, dentro = m.group(1), m.group(2)
        p = set()
        for pezzo in dentro.split(","):
            pezzo = pezzo.strip()
            if not pezzo or pezzo.startswith("&") or pezzo.startswith("self"):
                continue
            nome_par = pezzo.split(":")[0].strip()
            if nome_par and nome_par not in DEL_GUSCIO:
                p.add(nome_par)
        firme[nome] = p

controlla("le firme dei comandi si leggono",
          len(firme) >= len(registrati),
          f"{len(firme)} firme per {len(registrati)} comandi registrati: "
          f"mancano {sorted(registrati - set(firme))}")

# -- cosa chiede la pagina ------------------------------------------------
chiamati: dict[str, set[str]] = {}
visti_bene: set[tuple] = set()
tutti_gli_invoke: list[tuple] = []
pagine = sorted(list(UI.glob("*.html")) + list(UI.glob("*.js")))
controlla("le pagine ci sono", len(pagine) >= 4, f"trovate {len(pagine)}")

for f in pagine:
    testo = f.read_text(encoding="utf-8-sig")
    # `invoke?.(...)` con l'optional chaining e' la forma piu' usata nelle
    # pagine, e una regex che chiede `invoke(` non la vede. Il cercatore che
    # non trova non e' innocuo: dichiara morti dei comandi vivi, e un
    # controllo che accusa il falso e' un controllo che si smette di leggere.
    for m in re.finditer(r"invoke(?:\?\.)?\s*\(\s*['\"]([\w:]+)['\"]\s*(,\s*\{([^{}]*)\})?", testo):
        nome = m.group(1)
        argomenti = chiamati.setdefault(nome, set())
        if m.group(3):
            for chiave in re.finditer(r"(\w+)\s*:", m.group(3)):
                argomenti.add(chiave.group(1))
        visti_bene.add((f.name, m.start()))
    # E per sapere se il cercatore ha perso qualcosa non ci si affida al
    # cercatore: si contano **tutte** le volte che compare la parola, e si
    # pretende che ognuna sia stata riconosciuta. Una forma nuova diventa
    # rossa qui invece di sparire in silenzio.
    for m in re.finditer(r"\binvoke\b\s*(?:\?\.)?\s*\(", testo):
        tutti_gli_invoke.append((f.name, m.start(), testo[m.start():m.start() + 70]))

print(f"\n== {len(chiamati)} comandi chiamati dalle pagine ==")
controlla("le pagine chiamano qualcosa", len(chiamati) > 5,
          f"trovati {len(chiamati)}: il cercatore non cerca piu' niente")

persi = [x for x in tutti_gli_invoke if (x[0], x[1]) not in visti_bene]
controlla("il cercatore non si perde nessuna chiamata", not persi,
          "queste sono scritte in una forma che non riconosce, e finirebbero "
          "per far dichiarare morto un comando vivo: "
          + "; ".join(f"{f}: {t.strip()}" for f, _, t in persi[:4]))

for nome in sorted(chiamati):
    controlla(f"invoke('{nome}') esiste davvero", nome in registrati,
              "nessun comando con questo nome e' registrato in main.rs: "
              "il bottone che lo chiama non fa niente, in silenzio")

print("\n== e gli argomenti si chiamano come li vuole il Rust ==")
for nome, argomenti in sorted(chiamati.items()):
    if nome not in firme:
        continue
    for a in sorted(argomenti):
        controlla(f"{nome}({a})", a in firme[nome] or a_serpente(a) in firme[nome],
                  f"la funzione accetta {sorted(firme[nome]) or 'nessun argomento'}: "
                  "un argomento che non c'e' arriva come «manca un parametro» "
                  "e la chiamata fallisce senza che nessuno lo veda")

print("\n== nessun comando dimenticato ==")
for nome in sorted(registrati):
    if nome in chiamati or nome in SENZA_CHIAMATE:
        passati += 1
        continue
    falliti.append(f"{nome} non lo chiama nessuno")
    print(f"  [NO ] {nome} non lo chiama nessuno  "
          "o e' codice morto, o e' una funzione che si e' persa: se lo "
          "chiama il Rust, o se e' un aggancio non ancora collegato, va "
          "messo in SENZA_CHIAMATE col perche'")

for nome, motivo in sorted(SENZA_CHIAMATE.items()):
    controlla(f"{nome} e' ancora registrato", nome in registrati,
              "dichiarato qui, ma non e' piu' un comando registrato")
    controlla(f"{nome} non e' tornato in uso", nome not in chiamati,
              "adesso la pagina lo chiama: la dichiarazione e' scaduta")
    controlla(f"{nome} ha un motivo vero", len(motivo) > 40,
              "un motivo di tre parole non e' un motivo")

print(f"\n{passati} passate, {len(falliti)} fallite")
for n in falliti:
    print(f"  - {n}")
sys.exit(1 if falliti else 0)
