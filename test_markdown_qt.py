# -*- coding: utf-8 -*-
"""Il giro Markdown -> documento -> Markdown non deve perdere niente.

Esiste per un difetto misurato di Qt: `setMarkdown` legge bene il grassetto -
il documento lo contiene davvero, verificato carattere per carattere - ma
`toMarkdown` lo butta, in tutti e tre i dialetti che offre. Scrivi in
grassetto, salvi, il grassetto non c'e' piu': una perdita silenziosa a ogni
salvataggio, cioe' la cosa peggiore che un editor possa fare.

La prova che conta e' quella della doppia andata: si legge, si riscrive, si
rilegge e si riscrive ancora. Se il secondo giro e' identico al primo, il
formato e' stabile e il documento non degrada un pezzetto per volta a ogni
salvataggio - che e' il modo in cui questi editor rovinano i file di
qualcuno senza che nessuno se ne accorga subito.
"""
import os
import sys

os.environ["QT_QPA_PLATFORM"] = "offscreen"
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

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


try:
    from PyQt6.QtGui import QTextDocument
    from PyQt6.QtWidgets import QApplication
except Exception as e:                                        # noqa: BLE001
    print(f"PyQt6 non disponibile ({type(e).__name__}): salto")
    sys.exit(0)

from nova.markdown_qt import da_documento   # noqa: E402

app = QApplication.instance() or QApplication([])

ORIGINALE = """# Stato dell'arte

Un paragrafo normale con **grassetto**, *corsivo* e `codice`.

## Sottotitolo

- primo punto
- secondo punto con **enfasi**
- terzo

1. uno
2. due

> Una citazione, che serve per le note.

Un altro paragrafo con un [collegamento](https://esempio.it) dentro.

| Ruolo | Nome |
|---|---|
| P | Meret |
| D | Buongiorno |

```
un blocco di codice
su due righe
```

Ultima riga.
"""


def giro(testo: str) -> str:
    d = QTextDocument()
    d.setMarkdown(testo)
    return da_documento(d)


primo = giro(ORIGINALE)

print("\n1. quello che Qt buttava")
controlla("il grassetto sopravvive", "**grassetto**" in primo, primo[:200])
controlla("il corsivo sopravvive", "*corsivo*" in primo)
controlla("l'enfasi dentro un elenco sopravvive", "**enfasi**" in primo)
controlla("e il metodo di Qt lo perderebbe ancora",
          "**grassetto**" not in QTextDocument_toMarkdown()
          if (QTextDocument_toMarkdown := (
              lambda: (lambda d: (d.setMarkdown(ORIGINALE), d.toMarkdown())[1])(
                  QTextDocument()))) else False)

print("\n2. e tutto il resto resta")
for nome, pezzo in [("titolo di primo livello", "# Stato dell'arte"),
                    ("titolo di secondo livello", "## Sottotitolo"),
                    ("codice in riga", "`codice`"),
                    ("elenco puntato", "- primo punto"),
                    ("elenco numerato", "1. uno"),
                    ("citazione", "> Una citazione"),
                    ("collegamento", "](https://esempio.it)"),
                    ("tabella", "| Ruolo | Nome |"),
                    ("riga della tabella", "| P | Meret |"),
                    ("blocco di codice", "```"),
                    ("contenuto del blocco", "un blocco di codice")]:
    controlla(nome, pezzo in primo, primo[:400] if pezzo not in primo else "")

print("\n3. il formato e' stabile: due giri danno lo stesso risultato")
secondo = giro(primo)
controlla("il secondo giro e' identico al primo", secondo == primo,
          "\n--- primo ---\n" + primo + "\n--- secondo ---\n" + secondo)
terzo = giro(secondo)
controlla("e il terzo pure", terzo == secondo)

print("\n4. i casi che rompono i marcatori")
for testo, atteso, perche in [
    ("Testo con **grassetto** finale.", "**grassetto**", "in mezzo alla riga"),
    ("**Tutta la riga in grassetto.**", "**Tutta la riga in grassetto.**",
     "riga intera"),
    ("Una parola *sola*.", "*sola*", "corsivo singolo"),
]:
    r = giro(testo)
    controlla(f"{perche}", atteso in r, repr(r))

vuoto = giro("")
controlla("un documento vuoto non esplode", isinstance(vuoto, str), repr(vuoto))

print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
