# -*- coding: utf-8 -*-
"""Cercare e leggere il web senza aprire una finestra.

Il difetto da cui nasce: NOVA apriva google.com nel proprio browser per
cercare - scheda, cookie, pagina dei risultati, click - quattro chiamate per
una. E la ricerca doveva essere di NOVA, non di Claude Code: chi la fa
ragionare con Gemini o col modello locale non ha `WebSearch`.

La prova tocca la rete: se non c'e', lo dice e non finge di aver provato.
"""
import sys
import time
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from nova import cerca  # noqa: E402

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


print("\n1. rifiuta quello che non puo' fare, invece di provarci")
r = cerca.prendi("mail.google.com")
controlla("un indirizzo senza schema viene rifiutato",
          not r.get("ok") and "http" in (r.get("motivo") or ""), str(r))
r = cerca.cerca("   ")
controlla("una domanda vuota viene rifiutata", not r.get("ok"), str(r))

print("\n2. da HTML a testo")
grezzo = ("<html><head><title>Prova &amp; C.</title><style>p{color:red}</style>"
          "</head><body><script>var x=1</script><h1>Titolo</h1>"
          "<p>Prima riga</p><p>Seconda &egrave; qui</p></body></html>")
t = cerca._testo(grezzo)
controlla("lo script non finisce nel testo", "var x" not in t, t[:60])
controlla("nemmeno lo stile", "color:red" not in t)
controlla("le entita' sono sciolte", "è qui" in t, t[:80])
controlla("le righe restano separate", "Prima riga" in t and "Seconda" in t)

print("\n3. prendere una pagina vera (serve rete)")
r = cerca.prendi("https://example.com", caratteri=2000)
if not r.get("ok") and "NameResolution" in (r.get("motivo") or "") \
        or (not r.get("ok") and "Connection" in (r.get("motivo") or "")):
    print("      niente rete: salto questa parte e la successiva")
else:
    controlla("la pagina arriva", r.get("ok"), str(r.get("motivo")))
    controlla("con un titolo", bool(r.get("titolo")), str(r.get("titolo")))
    controlla("e con del testo dentro", len(r.get("testo") or "") > 50)
    controlla("senza tag HTML nel testo", "<" not in (r.get("testo") or ""))

    print("\n4. cercare, senza che compaia niente sullo schermo")
    t0 = time.time()
    d = cerca.cerca("listone fantacalcio ruoli", quanti=6)
    ms = (time.time() - t0) * 1000
    controlla("la ricerca risponde", d.get("ok"), str(d.get("motivo")))
    ris = d.get("risultati") or []
    controlla("con piu' di un risultato", len(ris) >= 3, f"{len(ris)}")
    controlla("ogni risultato ha un indirizzo vero, non del motore",
              all(x.get("url", "").startswith("http")
                  and "bing.com" not in x.get("url", "") for x in ris),
              str([x.get("url", "")[:40] for x in ris[:3]]))
    controlla("e un titolo", all(x.get("titolo") for x in ris))
    print(f"       {ms:.0f} ms, {len(ris)} risultati")

    print("\n5. il browser da ricerca e' separato da quello di lavoro")
    controlla("porta diversa da quella del browser di lavoro",
              cerca.PORTA != 9222, str(cerca.PORTA))
    controlla("profilo diverso",
              cerca.profilo().name != "browser", str(cerca.profilo()))
    from nova import browser
    controlla("resta acceso per la prossima ricerca", browser.acceso(cerca.PORTA))
    # E non deve aver lasciato schede aperte ad accumularsi.
    schede = [s for s in browser.schede(cerca.PORTA) if s.get("type") == "page"
              and "bing.com" in (s.get("url") or "")]
    controlla("non lascia schede di ricerca aperte", not schede, str(len(schede)))

print(f"\n{passati}/{passati + len(falliti)} passati")
for f in falliti:
    print("  FALLITO:", f)
sys.exit(1 if falliti else 0)
