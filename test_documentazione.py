# -*- coding: utf-8 -*-
"""La documentazione si scolla un pezzo per volta, non tutta insieme.

Nessuno decide di lasciare un modulo fuori dai documenti: si aggiunge un
file, si e' di fretta, e la riga nell'albero del README non si scrive. Sei
mesi dopo l'albero elenca venti moduli su trentacinque e non e' piu' una
mappa - e' un elenco parziale, che e' peggio di nessun elenco perche' chi lo
legge crede che sia completo.

Qui si controlla solo cio' che si puo' controllare da solo: che ogni modulo
sia nominato da qualche parte, che i documenti citati esistano davvero, e che
il diario non resti indietro rispetto al lavoro. La prosa resta
responsabilita' di chi scrive.
"""
import re
import subprocess
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
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


DOCS = RADICE / "docs"
README = (RADICE / "README.md").read_text(encoding="utf-8-sig")
README_EN = (RADICE / "README.en.md").read_text(encoding="utf-8-sig")
DIARIO = (DOCS / "diario.md").read_text(encoding="utf-8")
ARCH = (DOCS / "architettura.md").read_text(encoding="utf-8")
BETA = (DOCS / "verso_la_beta.md").read_text(encoding="utf-8")
TUTTO = README + DIARIO + ARCH + BETA

print("\n1. i documenti ci sono e si raggiungono")
for f in ["architettura.md", "diario.md", "verso_la_beta.md"]:
    controlla(f"docs/{f} esiste", (DOCS / f).is_file())
    controlla(f"e il README ci porta", f"docs/{f}" in README, "non e' linkato")
    controlla(f"anche in inglese", f"docs/{f}" in README_EN)

# Un link a un documento che non c'e' e' peggio di nessun link: promette una
# spiegazione e la nega nello stesso gesto.
for testo, nome in [(README, "README.md"), (README_EN, "README.en.md"),
                    (DIARIO, "diario.md"), (ARCH, "architettura.md"),
                    (BETA, "verso_la_beta.md")]:
    base = RADICE if nome.startswith("README") else DOCS
    rotti = []
    for dove in re.findall(r"\]\(([^)#:]+\.md)\)", testo):
        if not (base / dove).is_file() and not (RADICE / dove).is_file():
            rotti.append(dove)
    controlla(f"{nome}: nessun rimando a un documento che non c'e'",
              not rotti, str(rotti))

print("\n2. ogni modulo e' nominato da qualche parte")
# L'albero nel README e' la mappa; il diario e l'architettura sono gli altri
# posti buoni. Quello che non compare in nessuno dei tre non lo conosce
# nessuno tranne chi lo ha scritto.
# `__init__.py` e `__main__.py` non sono moduli da spiegare: uno dichiara un
# pacchetto, l'altro esiste perche' «python -m nova» funzioni.
SENZA_NOME = {"__init__.py", "__main__.py"}
moduli = sorted(f.relative_to(RADICE).as_posix()
                for f in (RADICE / "nova").rglob("*.py")
                if f.name not in SENZA_NOME)
muti = [m for m in moduli if Path(m).name not in TUTTO]
controlla(f"tutti e {len(moduli)} i moduli sono citati", not muti, str(muti))

print("\n3. e ogni modulo citato esiste")
# Il contrario, che e' l'altro modo di scollarsi: si cancella un file e la
# riga nell'albero resta. E' successo con ui/main_window.py.
citati = set(re.findall(r"\b([a-z_]+\.py)\b", README)) - SENZA_NOME
esistenti = {Path(m).name for m in moduli} | {
    f.name for f in RADICE.glob("*.py")}
fantasmi = sorted(c for c in citati if c not in esistenti)
controlla("il README non elenca file spariti", not fantasmi, str(fantasmi))

print("\n4. le decisioni sono numerate e non si ripetono")
numeri = re.findall(r"^\| (D\d+) \|", ARCH, re.M)
controlla("ci sono decisioni registrate", len(numeri) >= 20, str(len(numeri)))
doppi = sorted({n for n in numeri if numeri.count(n) > 1})
controlla("e nessuna ha lo stesso numero di un'altra", not doppi, str(doppi))

print("\n5. il diario non resta indietro")
# Un diario e' utile solo se lo si scrive quando si lavora. Se l'ultima
# voce e' piu' vecchia dell'ultimo lavoro sul codice, non e' un diario: e'
# un documento scritto una volta e poi dimenticato.
#
# Si guarda la data, non i moduli nuovi: il repo e' stato schiacciato in un
# commit solo, quindi per git «nato di recente» vale per tutto.
import datetime                                              # noqa: E402
MESI = {"gennaio": 1, "febbraio": 2, "marzo": 3, "aprile": 4, "maggio": 5,
        "giugno": 6, "luglio": 7, "agosto": 8, "settembre": 9,
        "ottobre": 10, "novembre": 11, "dicembre": 12}
date = []
for g, m, a in re.findall(r"^## (\d{1,2}) (\w+) (\d{4})", DIARIO, re.M):
    if m.lower() in MESI:
        date.append(datetime.date(int(a), MESI[m.lower()], int(g)))
controlla("il diario ha almeno una voce datata", bool(date), DIARIO[:80])
if date:
    try:
        quando = subprocess.run(
            ["git", "log", "-1", "--format=%cs", "--", "nova", "core", "install.ps1"],
            cwd=str(RADICE), capture_output=True, text=True, timeout=30).stdout.strip()
        ultimo_lavoro = datetime.date.fromisoformat(quando) if quando else None
    except Exception:                                        # noqa: BLE001
        ultimo_lavoro = None
    if ultimo_lavoro is None:
        print("  (niente git qui: salto)")
    else:
        scarto = (ultimo_lavoro - max(date)).days
        controlla("e l'ultima non e' piu' vecchia dell'ultimo lavoro",
                  scarto <= 7,
                  f"ultima voce {max(date)}, ultimo lavoro {ultimo_lavoro}")
        # E nemmeno piu' **nuova**. Questa non c'era, e per questo il diario
        # ha camminato tre giorni avanti all'orologio senza che niente lo
        # dicesse: le voci erano datate 9 settembre mentre l'ultimo lavoro
        # era del 6. Un diario avanti non e' un diario in ritardo di segno
        # opposto: e' un documento che afferma una cosa falsa su quando e'
        # successo cio' che racconta, ed e' l'unico documento che non ha modo
        # di smentirsi da solo.
        controlla("e nemmeno piu' nuova: un diario non puo' stare nel futuro",
                  max(date) <= ultimo_lavoro,
                  f"ultima voce {max(date)}, ultimo lavoro {ultimo_lavoro}")

print("\n6. e dice a cosa serve, cosi' nessuno lo confonde con git log")
controlla("spiega perche' non basta il registro delle modifiche",
          "git log" in DIARIO and "mentre" in DIARIO.lower())
controlla("e dice che ci vanno anche gli errori",
          "errori" in DIARIO.lower() or "errore" in DIARIO.lower())

print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
