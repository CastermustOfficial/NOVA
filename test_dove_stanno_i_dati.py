# -*- coding: utf-8 -*-
"""La mappa dei dati non deve tacere su niente, ne' inventare niente.

`dati_dove` risponde a «dove stanno le mie cose». E' una domanda a cui NOVA
deve saper rispondere per intero, perche' e' quella che si fa chi vuole
salvarsi i dati, portarli via, o cancellarli — e una mappa incompleta e'
peggio di nessuna mappa: **sembra** completa.

Aveva due difetti insieme, e nessuno dei due dava errore:

- diceva che le cose in programma stanno in `pianificate.json`, e nessuno
  scriveva un file con quel nome. L'elenco mostra solo cio' che esiste,
  quindi quella voce non compariva mai: chi chiedeva dove stanno i suoi dati
  non sentiva parlare delle cose che NOVA fa da sola;
- non nominava `avvisi.jsonl` ne' la cartella dei log, che invece ci sono.

Questa prova non guarda l'elenco: guarda **il codice**, cerca ogni percorso
che NOVA si costruisce dentro la propria cartella, e pretende che la mappa lo
copra. E al contrario, che la mappa non prometta un posto che nessuno scrive.
"""
import ast
import io
import re
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from nova.dati import posti                                   # noqa: E402

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


#: Percorsi che NOVA scrive e che la mappa non elenca **apposta**, col
#: perche'. Restare qui e' una dichiarazione, non una scappatoia.
FUORI_DALLA_MAPPA = {
    "avvio.log": "sta accanto al programma, non fra i dati dell'utente: e' il "
                 "diario di chi ripara, e si cancella con la copia di NOVA.",
}


def coda(n) -> str | None:
    """L'ultimo pezzo di un'espressione di percorso.

    `Path(b) / "NOVA" if b else Path.home() / ".config" / "NOVA"` finisce con
    «NOVA» da tutte e due le parti: e' la radice. `radice / "azioni.jsonl"`
    finisce con il nome del file.
    """
    if isinstance(n, ast.IfExp):
        return coda(n.body)
    if isinstance(n, ast.BinOp) and isinstance(n.op, ast.Div):
        return coda(n.right) or coda(n.left)
    if isinstance(n, ast.Constant) and isinstance(n.value, str):
        return n.value
    return None


def dove_scrive(sorgente: str) -> set[str]:
    """I nomi che questo modulo si costruisce **dentro** la cartella di NOVA.

    Non si cerca `_base()`: ogni modulo se la ricalcola a modo suo, con un
    nome di variabile diverso — `cartella`, `radice`, `base`. Si cerca la
    **forma**: qualunque cosa finisca con «NOVA» e' la radice, e cio' che le
    si attacca dopo e' un posto in cui NOVA scrive.

    Un modulo la cui radice e' gia' una sottocartella (l'harness) non
    contribuisce coi suoi file: contribuisce con la sottocartella, che e'
    quello che la mappa deve elencare.
    """
    albero = ast.parse(sorgente)
    radici: set[str] = {"APP_DIR"}
    sotto: set[str] = set()
    for n in ast.walk(albero):
        if isinstance(n, (ast.Assign, ast.AnnAssign)):
            bersagli = n.targets if isinstance(n, ast.Assign) else [n.target]
            nome = getattr(bersagli[0], "id", None)
            if nome and n.value is not None and coda(n.value) == "NOVA":
                radici.add(nome)
        if isinstance(n, ast.FunctionDef):
            for r in ast.walk(n):
                if isinstance(r, ast.Return) and r.value is not None:
                    if coda(r.value) == "NOVA":
                        radici.add(n.name)

    def e_radice(x) -> bool:
        if isinstance(x, ast.Name):
            return x.id in radici
        if isinstance(x, ast.Call):
            return getattr(x.func, "id", getattr(x.func, "attr", "")) in radici
        return coda(x) == "NOVA"

    for n in ast.walk(albero):
        if (isinstance(n, ast.BinOp) and isinstance(n.op, ast.Div)
                and isinstance(n.right, ast.Constant)
                and isinstance(n.right.value, str)
                and "{" not in n.right.value
                and e_radice(n.left)):
            sotto.add(n.right.value)
    return sotto


print("\n1. dove NOVA scrive, secondo il codice")
scritti: dict[str, list[str]] = {}
for f in sorted((RADICE / "nova").rglob("*.py")):
    for nome in dove_scrive(io.open(f, encoding="utf-8-sig").read()):
        scritti.setdefault(nome, []).append(f.name)

for nome in sorted(scritti):
    print(f"     {nome:24s} {sorted(set(scritti[nome]))}")
controlla("il codice dichiara almeno otto posti", len(scritti) >= 8,
          str(len(scritti)))

print("\n2. la mappa li copre tutti")
mappa = {p.dove.name for p in posti()}
muti = sorted(n for n in scritti
              if n not in mappa and n not in FUORI_DALLA_MAPPA)
controlla("nessun posto in cui NOVA scrive resta fuori dalla mappa", not muti,
          f"{muti}  <- o si aggiunge a `dati.posti()`, o si dichiara in "
          "FUORI_DALLA_MAPPA con scritto perche'")

print("\n3. e non ne promette nessuno che non esista")
# La voce sbagliata non dava errore: l'elenco mostra solo cio' che esiste,
# quindi un percorso che nessuno scrive **spariva**, e chi leggeva credeva
# che quel dato non ci fosse.
sorgenti = "\n".join(io.open(f, encoding="utf-8-sig").read()
                     for f in (RADICE / "nova").rglob("*.py")
                     if f.name != "dati.py")
sorgenti += "\n".join(io.open(f, encoding="utf-8", errors="replace").read()
                      for f in (RADICE / "core" / "crates").rglob("*.rs"))
inventati = []
for p in posti():
    nome = p.dove.name
    if nome in scritti or f'"{nome}"' in sorgenti:
        continue
    inventati.append(nome)
# Il vault e il fascicolo hanno un nome scelto dall'utente: non si cercano
# per nome, si cercano per la funzione che li calcola.
inventati = [n for n in inventati
             if not any(x in sorgenti for x in ("percorso_vault", "def cartella"))
             or n not in {posti()[1].dove.name, posti()[2].dove.name}]
controlla("ogni posto della mappa lo scrive davvero qualcuno", not inventati,
          f"{inventati}  <- la mappa lo promette e nessuno lo crea: sparisce "
          "dall'elenco e chi legge crede che quel dato non ci sia")

print("\n4. e dice cosa si perde, per ognuno")
vuoti = [p.che_cos_e for p in posti()
         if not p.se_lo_cancelli.strip() or len(p.se_lo_cancelli) < 40]
controlla("ogni voce spiega cosa succede se si cancella", not vuoti, str(vuoti))
delicati = [p.che_cos_e for p in posti() if p.delicato]
controlla("e almeno le credenziali e la configurazione sono marcate delicate",
          len(delicati) >= 3, str(delicati))

print("\n5. e non ricalcola un percorso che un altro modulo sa gia' dire")
# La riparazione vera non e' stata aggiungere le voci mancanti: e' stato
# smettere di **ricalcolare** i percorsi qui dentro. Un percorso scritto due
# volte e' un percorso che prima o poi diverge — e qui divergere vuol dire
# indicare a chi cerca i propri dati un file che non esiste. Fuori da Windows
# le due copie erano gia' divergenti in tre punti, e nessuno lo vedeva perche'
# NOVA gira su Windows, dove per caso coincidono.
#
# Chi resta scritto a mano ha un motivo: nessun modulo Python sa dirlo.
A_MANO_CON_MOTIVO = {
    "segreti.dat": "lo scrive il demone, che e' in Rust: di qua non c'e' "
                   "nessuna funzione da chiamare.",
    "procedure.log": "lo scrive `agent` senza passare da una funzione sua.",
    "vault": "non e' la voce della mappa — quella la dice `percorso_vault` — "
             "e' il ripiego per quando la configurazione non si legge. Un "
             "posto detto male e' meglio di un elenco che non parte.",
}
sorgente_dati = io.open(RADICE / "nova" / "dati.py", encoding="utf-8-sig").read()
a_mano = set(re.findall(r'\bb\s*/\s*"([^"]+)"', sorgente_dati))
doppioni = sorted(n for n in a_mano
                  if n not in A_MANO_CON_MOTIVO and n in scritti)
controlla("nessun percorso ricalcolato qui che un altro modulo gia' calcola",
          not doppioni,
          f"{doppioni}  <- chiedilo al modulo che lo scrive, invece di "
          "rifare il conto")
avanzati = sorted(n for n in a_mano if n not in A_MANO_CON_MOTIVO
                  and n not in scritti)
controlla("e quelli scritti a mano sono solo quelli dichiarati", not avanzati,
          f"{avanzati}  <- aggiungilo ad A_MANO_CON_MOTIVO con scritto perche'")

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
