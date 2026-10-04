# -*- coding: utf-8 -*-
"""Cercare e leggere il web senza aprire una finestra.

Il difetto da cui nasce: NOVA apriva google.com nel proprio browser per
cercare - scheda, cookie, pagina dei risultati, click - quattro chiamate per
una. E la ricerca doveva essere di NOVA, non di Claude Code: chi la fa
ragionare con Gemini o col modello locale non ha `WebSearch`.

La prova tocca la rete: se non c'e', lo dice e non finge di aver provato. E
il motore di ricerca e' di qualcun altro — puo' strozzare, puo' rispondere
una pagina anti-bot, puo' essere giu'. Quando succede questa prova si
dichiara **non provabile** invece di rossa: dare la colpa a NOVA per una
cosa che NOVA non controlla e' il modo di rendere una suite inaffidabile, e
una suite che ogni tanto mente non la guarda piu' nessuno (D164).

Il tempo dichiarato non e' un permesso di essere lenta. Questa prova avvia un
Chrome vero e aspetta due volte la rete, e con la macchina occupata — un
`cargo test` di tutto lo spazio di lavoro accanto — i novanta secondi
predefiniti non le bastano: e' successo, e il banco l'ha segnata rossa mentre
da sola passava in tre secondi. Un rosso che dipende da cosa gira accanto non
dice niente sul codice (D156).
"""
# banco: attesa 240
import sys
import time
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
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

# Guidare il browser vuol dire parlargli in HTTP, e quella conversazione si
# rompe in tutti i modi in cui si rompe una conversazione. Ogni altra strada
# di questo modulo torna un motivo; questa sola sollevava, e chi la chiamava
# si ritrovava in mano uno stack di `requests`. Al modello uno stack non dice
# se riprovare, se cambiare strada o se dirlo all'utente. Una frase si'.
#
# La porta e' chiusa apposta, e qui nessun browser si accende: i due `avvia`,
# quello del browser delle ricerche e quello del browser di lavoro che
# `browser.apri` chiama da se', sono sostituiti da finti che non fanno niente.
# Fino al D364 `cerca.cerca` li chiamava veri. Dove Edge c'e', come sulla CI
# Windows e sul PC di sviluppo (GPU RTX 4060 Ti, 16 GB di VRAM; 32 GB di RAM DDR5; scheda madre Gigabyte B650 EAGLE AX; CPU Ryzen 5 7600X), si accendeva un Edge vero sulla porta 59999, col
# profilo delle ricerche, e restava acceso dopo la prova. La prova allora
# dipendeva da Bing, e la parte 4 non poteva piu' accendere il suo browser
# sulla 9223, perche' il profilo era gia' in uso.
avvia_vero, avvia_lavoro_vero = cerca.avvia, cerca.browser.avvia
apri_vero, valuta_vero, chiudi_vero = cerca.browser.apri, cerca.browser.valuta, cerca._chiudi
cerca.avvia = cerca.browser.avvia = lambda *_a, **_k: {}
try:
    try:
        r = cerca.cerca("qualcosa", porta=59999, attesa=1)
        esploso = ""
    except Exception as e:                                     # noqa: BLE001
        r, esploso = {}, f"{type(e).__name__}: {e}"
    controlla("con il browser irraggiungibile torna un motivo, non uno stack",
              not esploso and not r.get("ok")
              and (r.get("motivo") or "").startswith("non riesco a guidare il browser: "),
              esploso or str(r))

    # Anche leggere la pagina e' parlare col browser. Fino al D364 un errore
    # qui usciva come stack: la scheda si apre, e poi ogni lettura fallisce.
    chiuse: list[str] = []

    def valuta_rotta(*_a, **_k):
        raise RuntimeError("TypeError: document.body is null")

    cerca.browser.apri = lambda *_a, **_k: {"id": "s1"}
    cerca.browser.valuta = valuta_rotta
    cerca._chiudi = lambda sid, _porta: chiuse.append(sid)
    try:
        r = cerca.cerca("qualcosa", porta=59999, attesa=1)
        esploso = ""
    except Exception as e:                                     # noqa: BLE001
        r, esploso = {}, f"{type(e).__name__}: {e}"
    controlla("una pagina che non si lascia leggere e' un motivo, col suo errore",
              not esploso and not r.get("ok")
              and r.get("motivo") == ("il motore non ha dato risultati leggibili; "
                                      "l'ultima lettura della pagina ha dato: "
                                      "TypeError: document.body is null"),
              esploso or str(r))
    controlla("e la scheda si chiude lo stesso", chiuse == ["s1"], str(chiuse))
finally:
    cerca.avvia, cerca.browser.avvia = avvia_vero, avvia_lavoro_vero
    cerca.browser.apri, cerca.browser.valuta, cerca._chiudi = apri_vero, valuta_vero, chiudi_vero

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
    # Senza Edge ne' Chrome questa parte non si puo' provare: e' «non
    # provabile qui», non un fallimento. Prima moriva con un traceback, che
    # e' il modo di dire «questa suite gira su una macchina sola».
    from nova import browser as _b
    try:
        _b._eseguibile()
    except Exception as e:                                     # noqa: BLE001
        print(f"      niente browser ({e}): salto la ricerca vera")
        print(f"\n{passati}/{passati + len(falliti)} passati")
        for f in falliti:
            print("  FALLITO:", f)
        sys.exit(1 if falliti else 2)
    # Due volte, e poi si dichiara.
    #
    # Un motore di ricerca e' di qualcun altro: puo' rispondere una pagina
    # anti-bot, puo' strozzare chi chiede troppo in fretta, puo' essere giu'.
    # Quando succede, questa prova diceva **rossa** — cioe' dava la colpa a
    # NOVA per una cosa che NOVA non controlla, ed e' esattamente l'errore
    # che il banco ha smesso di fare quando ha imparato «non provabile»
    # (D164). Qui la distinzione non c'era ancora, e in una giornata di
    # lavoro l'ho vista sbagliare due volte.
    #
    # Cosa resta rosso: se NOVA non sa guidare il browser, o se il codice si
    # rompe. Quello si vede lo stesso, perche' fallisce in un altro modo.
    t0 = time.time()
    d = cerca.cerca("listone fantacalcio ruoli", quanti=6)
    if not (d.get("ok") and d.get("risultati")):
        time.sleep(3)
        d = cerca.cerca("listone fantacalcio ruoli", quanti=6)
    ms = (time.time() - t0) * 1000
    if not (d.get("ok") and d.get("risultati")):
        print(f"      il motore non ha dato risultati ({d.get('motivo') or 'nessun motivo'}):")
        print("      non e' una cosa che NOVA controlla, quindi qui non si prova.")
        print(f"\n{passati}/{passati + len(falliti)} passati")
        for f in falliti:
            print("  FALLITO:", f)
        sys.exit(1 if falliti else 2)
    controlla("la ricerca risponde", d.get("ok"), str(d.get("motivo")))
    ris = d.get("risultati") or []
    controlla("con piu' di un risultato", len(ris) >= 3, f"{len(ris)}")
    # «Del motore» vuol dire il rimbalzo che Bing mette al posto dell'indirizzo
    # (`bing.com/ck/a?...`), non qualunque pagina di bing.com: cercando «bing»,
    # i primi risultati veri sono `www.bing.com` e `get.bing.com`, e il 4 ottobre
    # la CI ha avuto risultati su Microsoft per una domanda sul fantacalcio. Se
    # cade, la prova dice quali indirizzi, interi.
    sbagliati = [x.get("url", "") for x in ris
                 if not x.get("url", "").startswith("http") or "bing.com/ck/" in x.get("url", "")]
    controlla("ogni risultato ha un indirizzo vero, non del motore", not sbagliati,
              str(sbagliati))
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
    # Chiudere una scheda non e' istantaneo: guardare una volta sola fa dire
    # «lasciata aperta» a chi ha la macchina lenta. In CI e' capitato su una
    # versione di Python su quattro - cioe' la differenza non era Python, era
    # il carico. Si aspetta che sparisca, e solo se resta e' una perdita.
    def schede_di_ricerca():
        return [s for s in browser.schede(cerca.PORTA)
                if s.get("type") == "page" and "bing.com" in (s.get("url") or "")]
    schede = schede_di_ricerca()
    scade = time.time() + 5
    while schede and time.time() < scade:
        time.sleep(0.5)
        schede = schede_di_ricerca()
    controlla("non lascia schede di ricerca aperte", not schede, str(len(schede)))

print(f"\n{passati}/{passati + len(falliti)} passati")
for f in falliti:
    print("  FALLITO:", f)
sys.exit(1 if falliti else 0)
