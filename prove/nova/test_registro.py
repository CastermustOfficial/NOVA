# -*- coding: utf-8 -*-
"""Il registro delle azioni che non si annullano.

Non e' un freno - l'utente resta responsabile di quello che chiede - ma la
responsabilita' ha bisogno di visibilita': si risponde solo di quello che si
puo' vedere. Se NOVA manda tre candidature mentre l'utente guarda altrove,
senza registro non resta traccia di cosa e' partito e a chi.

Il controllo che conta piu' di tutti e' l'ultimo: un registro e' esattamente
il posto dove una password finirebbe scritta per sempre.
"""
import json
import os
import sys
import stat
import tempfile
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

# Il registro scrive sotto %APPDATA%: si dirotta su una cartella temporanea
# PRIMA di importarlo, o la prova sporca il registro vero dell'utente.
finto = Path(tempfile.mkdtemp(prefix="nova_reg_"))
os.environ["APPDATA"] = str(finto)

from nova import registro  # noqa: E402
from nova import mcp_kb    # noqa: E402

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


print("\n1. scrive e rilegge")
controlla("all'inizio e' vuoto", registro.leggi() == [])
registro.annota("inviata candidatura", dove="acme.com",
                dettagli="posizione: sviluppatore", tipo="dichiarata")
registro.annota("premuto «Invia»", dove="https://acme.com/careers")
righe = registro.leggi()
controlla("due righe", len(righe) == 2, str(len(righe)))
controlla("la piu' recente per prima",
          righe[0]["azione"].startswith("premuto"), righe[0]["azione"])
controlla("con data, tipo e luogo",
          all(k in righe[0] for k in ("quando", "tipo", "dove")))
controlla("il tipo dichiarato si distingue da quello automatico",
          righe[1]["tipo"] == "dichiarata" and righe[0]["tipo"] == "browser",
          f"{righe[1]['tipo']} / {righe[0]['tipo']}")

print("\n2. si legge senza decodificare niente")
t = registro.racconta()
controlla("il racconto nomina l'azione", "inviata candidatura" in t)
controlla("e dice dove", "acme.com" in t)

print("\n3. non si mette mai di traverso")
# Un registro che solleva impedisce di lavorare, e verrebbe tolto di mezzo.
# La cartella si rende non scrivibile per un attimo, e **si rimette com'era**.
#
# Prima non si rimetteva. Su Windows non si notava, perche' li' `chmod` non
# fa niente: la cartella restava scrivibile e il resto della prova girava.
# Su Linux e macOS invece restava a 0555 fino alla fine, e la sezione 7 - la
# potatura - non poteva piu' rinominare niente. Rossa li', verde qui, e
# sembrava un difetto della potatura: ci sono volute tre sonde per scoprire
# che era questa riga, cinquanta righe piu' su, a lasciare il mondo storto.
#
# La regola che ne esce vale oltre questa prova: una prova che cambia i
# permessi, l'ambiente o una cartella la rimette a posto **sempre**, anche
# quando fallisce, o il difetto si presenta altrove e con un'altra faccia.
cartella = registro.percorso().parent
modo_di_prima = cartella.stat().st_mode
if os.name != "nt":
    cartella.chmod(0o555)
try:
    try:
        registro.annota("x" * 5000, dove="y" * 5000, dettagli="z" * 5000)
        ok = True
    except Exception:                                       # noqa: BLE001
        ok = False
finally:
    if os.name != "nt":
        cartella.chmod(stat.S_IMODE(modo_di_prima))
controlla("un'annotazione enorme non solleva", ok)
controlla("e la cartella e' tornata scrivibile",
          os.access(cartella, os.W_OK),
          "una prova che lascia il mondo storto rompe quelle dopo, e lontano")
ultima = registro.leggi(1)[0]
controlla("e viene comunque troncata", len(ultima["dettagli"]) <= registro.TESTO_MAX,
          str(len(ultima["dettagli"])))

print("\n4. la finestra temporale")
controlla("con ore=0 si vede tutto", len(registro.leggi(ore=0)) >= 3)
controlla("con una finestra larga si vede lo stesso",
          len(registro.leggi(ore=24)) >= 3)

print("\n5. gli strumenti del browser annotano da soli")


class FintoBrowser:
    def clicca(self, selettore="", scheda="", testo=""):
        return {"ok": True, "su": "ACCETTO TUTTO"}

    def scrivi(self, selettore, testo, scheda=""):
        return {"ok": True}


s = mcp_kb.ServerKB.__new__(mcp_kb.ServerKB)
s._browser = lambda: FintoBrowser()
s._dove_sono = lambda scheda: "https://esempio.it/modulo"

prima = len(registro.leggi(999))
s.web_click(testo="ACCETTO", scheda="x")
dopo = registro.leggi(999)
controlla("un click lascia una riga", len(dopo) == prima + 1)
controlla("con l'indirizzo della pagina",
          dopo[0].get("dove") == "https://esempio.it/modulo", str(dopo[0]))

print("\n6. il valore di una credenziale non entra MAI nel registro")


class FintoCore:
    def __enter__(self):
        return self

    def __exit__(self, *a):
        return False

    def call(self, metodo, params):
        return {"valore": "SuperSegreta123!"}


import nova.core_client as cc  # noqa: E402
cc.CoreClient = lambda *a, **k: FintoCore()

s.web_scrivi(selettore="#password", segreto="gmail", scheda="x")
riga = registro.leggi(1)[0]
tutto = json.dumps(registro.leggi(999), ensure_ascii=False)
controlla("l'uso della credenziale e' annotato",
          "gmail" in riga.get("azione", ""), str(riga))
controlla("il tipo dice che e' una credenziale",
          riga.get("tipo") == "credenziale", str(riga.get("tipo")))
controlla("IL VALORE NON C'E' DA NESSUNA PARTE",
          "SuperSegreta123!" not in tutto)

print("\n7. non cresce all'infinito")
registro.BYTE_MAX = 2000
for i in range(60):
    registro.annota(f"azione numero {i}", dettagli="x" * 200)
f = registro.percorso()
storico_f = f.with_suffix(".1.jsonl")
# Prima si controllava solo che il file fosse **sotto** i 40.000 byte, e un
# file vuoto lo e': se `annota` avesse smesso di scrivere del tutto, questa
# riga sarebbe rimasta verde e la prova avrebbe detto che il registro «resta
# sotto controllo» mentre non conteneva niente. Adesso si chiede anche che
# abbia scritto.
controlla("il file resta sotto controllo", 0 < f.stat().st_size <= 40000,
          f"{f.stat().st_size} byte")
def perche_non_ruota() -> str:
    """Cosa risponde la potatura, chiesto direttamente.

    Scritta per una rossa che si vedeva solo su un agente, dove il file non
    lo si puo' aprire: com'e' fatto, quale tetto e' in vigore, cosa risponde
    `ruota_se_serve` chiamata a mano, e se in quella cartella rinominare si
    puo'. E' stata l'ultima domanda a dare la risposta - `PermissionError`,
    perche' cinquanta righe piu' su la prova stessa aveva tolto il permesso
    di scrittura e non lo aveva rimesso. Resta qui: costa niente quando e'
    verde, e la prossima volta la risposta arriva al primo giro.
    """
    import shutil as _shutil
    import stat as _stat
    from nova.rotazione import MAX_BYTE, ruota_se_serve
    pezzi = []
    try:
        st = f.stat()
        pezzi.append(f"S_ISREG={_stat.S_ISREG(st.st_mode)} size={st.st_size} "
                     f"tetto={registro.BYTE_MAX} (di fabbrica {MAX_BYTE})")
        pezzi.append(f"a mano -> {ruota_se_serve(f, registro.BYTE_MAX)}, "
                     f"storico={storico_f.exists()}")
    except Exception as e:                                  # noqa: BLE001
        pezzi.append(f"non si riesce nemmeno a chiederlo: {type(e).__name__}: {e}")
    # Se dice di no con un file normale piu' grosso del tetto, l'unica strada
    # rimasta dentro la funzione e' che il rinominare fallisca. Lo si prova su
    # una copia, per non rovinare quel che resta da controllare.
    copia = f.with_name("prova-di-rinomina.tmp")
    meta = f.with_name("prova-di-rinomina.1.tmp")
    try:
        copia.unlink(missing_ok=True)
        meta.unlink(missing_ok=True)
        _shutil.copyfile(f, copia)
        copia.replace(meta)
        pezzi.append("rinominare li' dentro si puo'")
        meta.unlink(missing_ok=True)
    except Exception as e:                                  # noqa: BLE001
        pezzi.append(f"rinominare NO: {type(e).__name__}: {e}")
    return " | ".join(pezzi)


# Quando questa e' rossa serve sapere **cosa** c'era: su un agente il file
# non lo si puo' aprire, e senza queste due righe resta solo «non esiste».
dentro = ", ".join(f"{x.name} ({x.stat().st_size}b)"
                   for x in sorted(f.parent.iterdir()))
controlla("e il vecchio e' messo da parte, non buttato",
          storico_f.exists(),
          f"in {f.parent}: {dentro} -- {perche_non_ruota()}")

# `cerca` esiste per «cosa ho mandato a quella societa'?» tre settimane dopo.
# Se `leggi` guardasse solo il file vivo, il giorno della potatura quella
# domanda comincerebbe a rispondere «niente»: nessun errore, nessuna riga di
# log, e nessun modo di capire perche'.
storico = storico_f.read_text(encoding="utf-8").splitlines() if storico_f.exists() else []
primo = json.loads(storico[0])["azione"] if storico else None
azioni = [x.get("azione") for x in registro.leggi(quante=10000)]
controlla("cio' che sta nello storico si legge ancora",
          primo is not None and primo in azioni,
          "non c'e' nessuno storico da rileggere" if primo is None
          else f"«{primo}» sparita dopo la potatura")
controlla("e l'elenco resta dal piu' recente al piu' vecchio",
          bool(azioni) and azioni[-1] == primo,
          f"in fondo c'e' «{azioni[-1]}» invece della prima riga mai scritta: "
          "i due file si leggono nell'ordine sbagliato")

print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
