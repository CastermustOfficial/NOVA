# -*- coding: utf-8 -*-
"""La finestra dell'harness segue il registro, e non riceve ordini.

E' la proprieta' su cui vale la pena insistere: fra NOVA e la finestra non
c'e' niente da tenere in vita. La finestra legge un file e si adegua, quindi
si puo' chiudere, riaprire, o non aprire affatto, senza che il lavoro cambi.
Un collegamento vivo fra i due processi sarebbe stata la scelta ovvia, e
sarebbe stata la solita cosa che si rompe quando uno dei due muore.

Gira senza schermo (`QT_QPA_PLATFORM=offscreen`) e non fa comparire niente:
una prova che apre una finestra sul monitor di chi lavora e' esattamente il
difetto che questo progetto evita da giorni.
"""
import os
import sys
import tempfile
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

os.environ["QT_QPA_PLATFORM"] = "offscreen"
finto = Path(tempfile.mkdtemp(prefix="nova_fin_"))
os.environ["APPDATA"] = str(finto)

passati = 0
falliti: list[str] = []


def _blocchi(doc):
    b = doc.begin()
    while b.isValid():
        yield b
        b = b.next()


def controlla(nome, condizione, dettaglio=""):
    global passati
    if condizione:
        passati += 1
        print(f"  [ok ] {nome}")
    else:
        falliti.append(nome)
        print(f"  [NO ] {nome}  {dettaglio}")


try:
    from PyQt6.QtWidgets import QApplication
    from PyQt6.QtGui import QTextCursor
    from PyQt6.QtCore import Qt
except Exception as e:                                        # noqa: BLE001
    print(f"PyQt6 non disponibile ({type(e).__name__}): salto la prova")
    sys.exit(0)

from nova import harness                # noqa: E402
from nova import harness_finestra       # noqa: E402

# La finestra vera non si apre: qui si prova il meccanismo, non il vetro.
accensioni = []
harness_finestra.apri_se_serve = lambda *a, **k: (
    accensioni.append(1) or {"viva": False, "accesa_adesso": False,
                             "motivo": "non in una prova"})

import docx                             # noqa: E402
d = docx.Document()
for t in ["Stato dell'arte dei modelli aperti",
          "La quantizzazione a quattro bit riduce la memoria del settanta "
          "per cento.",
          "Le licenze restano il punto piu' controverso del settore.",
          "Engram indicizza una memoria statica con gli n-grammi."]:
    d.add_paragraph(t)
doc = finto / "ricerca.docx"
d.save(str(doc))

print("\n1. aprire accende la finestra, ma non e' obbligatorio che ci riesca")
r = harness.apri(str(doc))
controlla("il documento si apre", r.get("ok"), str(r.get("motivo")))
controlla("e si e' provato ad accendere la finestra", accensioni == [1])
controlla("e chi apre riceve il motivo, non un booleano",
          isinstance(r.get("finestra"), dict) and "motivo" in r["finestra"],
          str(r.get("finestra")))

print("\n1b. le due meta' ci sono")
app0 = QApplication.instance() or QApplication(sys.argv[:1])
prova = harness_finestra.costruisci(app0)
controlla("c'e' il documento a sinistra", hasattr(prova, "testo"))
controlla("e la conversazione a destra",
          hasattr(prova, "dialogo") and hasattr(prova, "campo"))
controlla("con un bottone per mandare", prova.bottone.text() == "Invia")
controlla("il tema e' quello di NOVA, non uno inventato",
          harness_finestra.BRACE == "#e8734a"
          and harness_finestra.FONDO == "#0a0908")
del prova

app = QApplication.instance() or QApplication(sys.argv[:1])
w = harness_finestra.costruisci(app)

print("\n2. la finestra si accorge da sola di cosa c'e'")
w.guarda()
controlla("mostra il nome del documento", "ricerca.docx" in w.intestazione.text())
controlla("e quanti blocchi sono", "4 blocchi" in w.intestazione.text(),
          w.intestazione.text())
html = w.testo.toHtml()
for pezzo in ("quantizzazione", "licenze", "Engram"):
    controlla(f"il testo disegnato contiene «{pezzo}»", pezzo in html)
controlla("ogni blocco ha un ancoraggio per lo scorrimento",
          html.count("name=") >= 4, str(html.count("name=")))

print("\n3. una ricerca sposta l'evidenziazione, senza che nessuno la avvisi")
harness.cerca("quanto risparmia la quantizzazione")
w.guarda()
primo = list(w._evidenziati)
controlla("la finestra ha cambiato evidenziazione da sola", bool(primo), str(primo))
controlla("ed e' il blocco giusto",
          "settanta" in [b["testo"] for b in harness._stato()["blocchi"]
                         if b["id"] == primo[0]][0], str(primo))

harness.cerca("licenze controverse")
w.guarda()
controlla("una seconda ricerca la sposta ancora",
          w._evidenziati and w._evidenziati != primo, str(w._evidenziati))

print("\n4. niente collegamenti vivi da tenere in piedi")
# Si butta via la finestra e se ne fa un'altra: deve ritrovare tutto, perche'
# lo stato non e' mai stato dentro di lei.
del w
w2 = harness_finestra.costruisci(app)
w2.guarda()
controlla("una finestra nuova ritrova il documento",
          "ricerca.docx" in w2.intestazione.text())
controlla("e ritrova anche cosa era evidenziato", bool(w2._evidenziati),
          str(w2._evidenziati))

print("\n5. e quando si chiude, lo dice invece di restare ferma")
harness.chiudi()
w2.guarda()
controlla("torna a dire che non c'e' niente",
          "Nessun documento" in w2.intestazione.text(), w2.intestazione.text())

print("\n6. un markdown non si legge soltanto: si scrive")
sorgente = finto_doc = Path(tempfile.mkdtemp(prefix="nova_md_")) / "appunti.md"
sorgente.write_text(
    "# Titolo\n\nUn paragrafo con **grassetto** e *corsivo*.\n\n"
    "- primo\n- secondo\n", encoding="utf-8")
harness.apri(str(sorgente))
w3 = harness_finestra.costruisci(app)
w3.guarda()
controlla("il foglio prende il posto della lettura", w3.modi.currentIndex() == 1,
          str(w3.modi.currentIndex()))
controlla("i ferri compaiono solo qui", not w3.barraFerri.isHidden())
controlla("il titolo e' un titolo, non testo con un cancelletto",
          "# Titolo" not in w3.editor.toPlainText()
          and "Titolo" in w3.editor.toPlainText(),
          w3.editor.toPlainText()[:60])
controlla("l'intestazione dice che si puo' modificare",
          "modificabile" in w3.intestazione.text(), w3.intestazione.text())
controlla("aprendo non risulta sporco", w3._sporco is False)

print("\n7. si salva quello che si vede, e resta com'era")
c = w3.editor.textCursor()
c.movePosition(QTextCursor.MoveOperation.End)
w3.editor.setTextCursor(c)
w3.editor.insertPlainText(" Aggiunta a mano.")
controlla("scrivere sporca il foglio", w3._sporco is True)
controlla("e lo dice", "non salvato" in w3.salvato.text(), w3.salvato.text())
w3.salva()
scritto = sorgente.read_text(encoding="utf-8")
controlla("il salvataggio va a buon fine", w3._sporco is False
          and w3.salvato.text() == "salvato", w3.salvato.text())
controlla("la copia di prima resta accanto",
          sorgente.with_suffix(".md.prima").exists())
controlla("il grassetto sopravvive al giro", "**grassetto**" in scritto,
          scritto[:120])
controlla("il corsivo pure", "*corsivo*" in scritto, scritto[:120])
controlla("il titolo torna titolo", scritto.lstrip().startswith("# Titolo"),
          scritto[:40])
controlla("l'elenco resta un elenco", "- primo" in scritto, scritto)
controlla("e c'e' quello che ho aggiunto", "Aggiunta a mano." in scritto)

# Riaprire e risalvare non deve spostare una virgola: e' la prova che il
# formato e' un punto fermo e non una lenta deriva a ogni passaggio.
w3.editor.document().setMarkdown(scritto)
from nova.markdown_qt import da_documento                     # noqa: E402
controlla("un secondo giro non cambia piu' niente",
          da_documento(w3.editor.document()) == scritto,
          repr(da_documento(w3.editor.document())[:80]))

print("\n8. il lavoro non salvato non lo cancella nessuno")
w3.editor.insertPlainText(" In corso.")
harness.cerca("primo")
w3.guarda()
controlla("una ricerca non ributta dentro il file dal disco",
          "In corso." in w3.editor.toPlainText())
controlla("e il foglio resta sporco", w3._sporco is True)

print("\n9. i PDF e i Word restano in lettura")
harness.apri(str(doc))
w3.guarda()
controlla("un docx torna alla meta' che si legge",
          w3.modi.currentIndex() == 0, str(w3.modi.currentIndex()))
controlla("e i ferri spariscono", w3.barraFerri.isHidden())
controlla("non c'e' piu' niente da salvare", w3._file_modificabile == "")

print("\n10. il documento si ingrandisce, e la misura se la ricorda")
partenza = w3.editor.maximumWidth()
w3.ingrandisci(+1)
controlla("ingrandire allarga il foglio", w3.editor.maximumWidth() > partenza,
          f"{partenza} -> {w3.editor.maximumWidth()}")
controlla("e lo dice in percentuale", "%" in w3.etichettaZoom.text()
          and w3.etichettaZoom.text() != "100%", w3.etichettaZoom.text())
w3.ingrandisci(-1)
controlla("rimpicciolire torna indietro",
          w3.editor.maximumWidth() == partenza, str(w3.editor.maximumWidth()))
w3.ingrandisci(+1)
w3.ingrandisci(+1)
grande = w3._zoom
w4 = harness_finestra.costruisci(app)
controlla("una finestra nuova riapre alla misura di prima",
          abs(w4._zoom - grande) < 0.001, f"{w4._zoom} vs {grande}")
w4.ingrandisci(0)
controlla("e si torna a cento con un colpo solo", w4._zoom == 1.0)
controlla("piu' grande di cosi' non va", (
    [w4.ingrandisci(+1) for _ in range(20)] and w4._zoom <= 2.8), str(w4._zoom))
w4.ingrandisci(0)

print("\n11. NOVA propone, e non applica")
from nova import harness_modifica as modifica                 # noqa: E402
harness.apri(str(sorgente))
w5 = harness_finestra.costruisci(app)
w5.guarda()
controlla("senza proposte il riquadro non c'e'",
          w5.riquadroProposta.isHidden())
prima_su_disco = sorgente.read_text(encoding="utf-8")
bersaglio = harness._stato()["blocchi"][-1]["id"]
modifica.proponi([{"blocco": bersaglio, "azione": "sostituisci",
                   "testo": "- secondo, rifatto meglio"}],
                 motivo="lo hai chiesto tu")
w5.guarda()
controlla("la proposta compare da sola", not w5.riquadroProposta.isHidden())
controlla("dice quante sono e perche'",
          "1 modifica" in w5.titoloProposta.text()
          and "lo hai chiesto tu" in w5.titoloProposta.text(),
          w5.titoloProposta.text())
diff = w5.diffProposta.toPlainText()
controlla("si vede cosa c'era", "secondo" in diff, diff[:80])
controlla("e cosa ci sarebbe", "rifatto meglio" in diff, diff[:120])
controlla("ma sul disco non e' cambiato niente",
          sorgente.read_text(encoding="utf-8") == prima_su_disco)

print("\n12. il bottone e' dell'utente, e scartare non lascia niente")
w5.scartaProposta()
controlla("scartare fa sparire il riquadro", w5.riquadroProposta.isHidden())
controlla("e il file resta com'era",
          sorgente.read_text(encoding="utf-8") == prima_su_disco)

modifica.proponi([{"blocco": harness._stato()["blocchi"][-1]["id"],
                   "azione": "sostituisci",
                   "testo": "- secondo, rifatto meglio"}])
w5.guarda()
w5.applicaProposta()
controlla("applicare scrive davvero",
          "rifatto meglio" in sorgente.read_text(encoding="utf-8"),
          sorgente.read_text(encoding="utf-8"))
controlla("e il riquadro se ne va", w5.riquadroProposta.isHidden())
controlla("il foglio mostra il testo nuovo",
          "rifatto meglio" in w5.editor.toPlainText(),
          w5.editor.toPlainText()[-90:])

print("\n13. l'anteprima sta dentro il testo, e si puo' ritoccare")
sorgente.write_text(
    "# Titolo\n\nUn paragrafo con **grassetto** e *corsivo*.\n\n"
    "Una frase da correggere.\n", encoding="utf-8")
harness.apri(str(sorgente))
w6 = harness_finestra.costruisci(app)
w6.guarda()
intatto = sorgente.read_text(encoding="utf-8")
da_cambiare = [b for b in harness._stato()["blocchi"]
               if "da correggere" in b["testo"]][0]["id"]
modifica.proponi([{"blocco": da_cambiare, "azione": "sostituisci",
                   "testo": "Una frase corretta."}], motivo="prova")
w6.guarda()
visto = w6.editor.toPlainText()
controlla("nel foglio c'e' quello che ci sarebbe", "Una frase corretta." in visto,
          visto[-90:])
controlla("e anche quello che se ne andrebbe", "Una frase da correggere." in visto)
controlla("le marche non si vedono",
          "\u241e" not in visto and "\u241f" not in visto, repr(visto[-60:]))
controlla("il grassetto del resto e' rimasto",
          "grassetto" in visto and "**" not in visto, visto[:80])
controlla("sul disco ancora niente",
          sorgente.read_text(encoding="utf-8") == intatto)
controlla("il foglio non risulta sporco per colpa dell'anteprima",
          w6._sporco is False)

# Il colore non e' decorazione: e' come il documento sa cosa buttare dopo.
segnati = {}
b = w6.editor.document().begin()
while b.isValid():
    q = b.blockFormat().intProperty(harness_finestra.ANTEPRIMA)
    if q:
        segnati.setdefault(q, []).append(b.text())
    b = b.next()
controlla("il nuovo e' segnato come nuovo",
          any("corretta" in x for x in segnati.get(1, [])), str(segnati))
controlla("e il vecchio come vecchio",
          any("da correggere" in x for x in segnati.get(2, [])), str(segnati))

print("\n14. si applica quello che si e' visto, anche se l'ho ritoccato io")
c = w6.editor.textCursor()
trovato = w6.editor.find("Una frase corretta.")
controlla("l'anteprima si raggiunge per scriverci", trovato)
w6.editor.insertPlainText("Una frase corretta a mano.")
w6.applicaProposta()
finale = sorgente.read_text(encoding="utf-8")
controlla("sul disco finisce il mio ritocco",
          "Una frase corretta a mano." in finale, finale)
controlla("la riga vecchia non c'e'", "da correggere" not in finale, finale)
controlla("e nemmeno quella proposta cosi' com'era",
          "Una frase corretta.\n" not in finale, repr(finale))
controlla("il resto del documento e' intatto",
          "**grassetto**" in finale and finale.startswith("# Titolo"),
          finale[:70])
controlla("le marche non finiscono nel file",
          "\u241e" not in finale and "\u241f" not in finale)
controlla("la proposta e' chiusa", modifica.proposta() is None)
controlla("e il riquadro se n'e' andato", w6.riquadroProposta.isHidden())
resta = [b.blockFormat().intProperty(harness_finestra.ANTEPRIMA)
         for b in _blocchi(w6.editor.document())]
controlla("nel foglio non resta niente di segnato", not any(resta), str(resta))

print("\n15. scartare toglie l'anteprima e lascia il documento com'era")
prima_di_tutto = sorgente.read_text(encoding="utf-8")
modifica.proponi([{"blocco": harness._stato()["blocchi"][0]["id"],
                   "azione": "sostituisci", "testo": "# Titolo diverso"}])
w6.guarda()
controlla("l'anteprima c'e'", "Titolo diverso" in w6.editor.toPlainText())
w6.scartaProposta()
rimasto = w6.editor.toPlainText()
controlla("scartare la toglie dal foglio", "Titolo diverso" not in rimasto,
          rimasto[:70])
controlla("e rimette il titolo vero", rimasto.lstrip().startswith("Titolo"),
          rimasto[:40])
controlla("il file non e' mai stato toccato",
          sorgente.read_text(encoding="utf-8") == prima_di_tutto)
resta2 = [b.blockFormat().intProperty(harness_finestra.ANTEPRIMA)
          for b in _blocchi(w6.editor.document())]
controlla("e non resta niente di segnato", not any(resta2), str(resta2))

print("\n16. una proposta non si perde chiudendo la finestra")
modifica.proponi([{"blocco": harness._stato()["blocchi"][0]["id"],
                   "azione": "sostituisci", "testo": "# Titolo proposto"}],
                 motivo="deve sopravvivere")
# Riaprire lo stesso documento crea una sessione nuova: e' li' che prima la
# proposta spariva in silenzio.
vecchia = harness._stato()["sessione"]
harness.apri(str(sorgente))
controlla("la sessione e' davvero un'altra",
          harness._stato()["sessione"] != vecchia)
controlla("ma la proposta e' ancora sua",
          (modifica.proposta() or {}).get("motivo") == "deve sopravvivere",
          str(modifica.proposta()))
w7 = harness_finestra.costruisci(app)
w7.guarda()
controlla("e una finestra nuova la ritrova", not w7.riquadroProposta.isHidden())
controlla("con l'anteprima gia' dentro il testo",
          "Titolo proposto" in w7.editor.toPlainText(),
          w7.editor.toPlainText()[:60])
w7.scartaProposta()
controlla("e si scarta lo stesso", modifica.proposta() is None)

print("\n17. un .txt torna sul disco esattamente com'era")
# Un file di testo non e' Markdown. Passarlo per il convertitore lo rovina -
# ogni riga diventa un paragrafo, con una riga vuota in mezzo - e su del
# codice incollato dentro un .txt il danno e' totale e silenzioso.
piano = Path(tempfile.mkdtemp(prefix="nova_txt_")) / "appunti.txt"
crudo = ("def somma(a, b):\n"
         "    # due piu' due\n"
         "    return a + b\n"
         "\n"
         "- non e' un elenco\n"
         "# non e' un titolo\n")
piano.write_text(crudo, encoding="utf-8")
harness.apri(str(piano))
w8 = harness_finestra.costruisci(app)
w8.guarda()
controlla("il .txt si apre nel foglio", w8.modi.currentIndex() == 1)
controlla("e lo si legge tale e quale",
          w8.editor.toPlainText().rstrip("\n") == crudo.rstrip("\n"),
          repr(w8.editor.toPlainText()))
w8._sporco = True
w8.salva()
tornato = piano.read_text(encoding="utf-8")
controlla("e torna sul disco identico", tornato == crudo, repr(tornato))
controlla("niente righe vuote infilate in mezzo",
          "\n\n    # due" not in tornato, repr(tornato))
controlla("il trattino non e' diventato un elenco",
          "- non e' un elenco" in tornato, repr(tornato))
controlla("e il cancelletto non e' diventato un titolo",
          "# non e' un titolo" in tornato, repr(tornato))

print("\n18. il codice si apre, e si salva senza essere riscritto")
sorgente_py = piano.with_name("modulo.py")
codice = ("def somma(a, b):\n"
          "    # *non* e' corsivo\n"
          "    return a + b\n"
          "\n"
          "\n"
          "class Cosa:\n"
          "    pass\n")
sorgente_py.write_text(codice, encoding="utf-8")
r = harness.apri(str(sorgente_py))
controlla("un .py si apre", r.get("ok"), str(r.get("motivo")))
bpy = harness._stato()["blocchi"]
controlla("tagliato per righe", all(b.get("righe") == 1 for b in bpy),
          str(bpy[:1]))
controlla("e l'indentazione e' nel blocco, non persa",
          any(b["testo"].startswith("    return") for b in bpy),
          str([b["testo"] for b in bpy]))
wpy = harness_finestra.costruisci(app)
wpy.guarda()
controlla("si apre nel foglio, da scrivere", wpy.modi.currentIndex() == 1)
controlla("con il sorgente esatto",
          wpy.editor.toPlainText().rstrip("\n") == codice.rstrip("\n"),
          repr(wpy.editor.toPlainText()))
wpy._sporco = True
wpy.salva()
controlla("e torna sul disco identico",
          sorgente_py.read_text(encoding="utf-8") == codice,
          repr(sorgente_py.read_text(encoding="utf-8")))
controlla("gli asterischi non sono diventati corsivo",
          "*non*" in sorgente_py.read_text(encoding="utf-8"))

# L'aria fra i paragrafi su un sorgente e' un disastro: una riga vuota
# disegnata fra ogni riga vera, e una funzione non ci sta piu' nello schermo.
margini = [b.blockFormat().bottomMargin() for b in _blocchi(
    wpy.editor.document())]
controlla("nessuna aria infilata fra le righe di codice",
          not any(margini), str(margini))
controlla("e il codice non va a capo da solo",
          wpy.editor.lineWrapMode().name == "NoWrap",
          str(wpy.editor.lineWrapMode()))

# Tornando a un documento vero, la tipografia deve tornare quella di prima.
harness.apri(str(sorgente))
wpy.guarda()
margini_md = [b.blockFormat().bottomMargin() for b in _blocchi(
    wpy.editor.document())]
controlla("su un documento l'aria torna", any(margini_md), str(margini_md))
controlla("e le righe tornano ad andare a capo",
          wpy.editor.lineWrapMode().name == "WidgetWidth",
          str(wpy.editor.lineWrapMode()))

print("\n19. un artifact si guarda reso, e si modifica nel sorgente")
pagina_html = Path(tempfile.mkdtemp(prefix="nova_art_")) / "cruscotto.html"
fonte = ("<!doctype html><html><head><style>\n"
         "body{display:grid;place-items:center;background:#111;color:#eee}\n"
         "</style></head><body>\n"
         "<h1 id='t'>Cruscotto</h1>\n"
         "<script>document.getElementById('t').textContent='Cruscotto vivo'</script>\n"
         "</body></html>\n")
pagina_html.write_text(fonte, encoding="utf-8")
r = harness.apri(str(pagina_html))
controlla("un .html si apre", r.get("ok"), str(r.get("motivo")))
bl = harness._stato()["blocchi"]
controlla("e i blocchi sono le righe del sorgente",
          any("<h1" in b["testo"] for b in bl),
          str([b["testo"][:30] for b in bl][:4]))
# Il codice non ha righe vuote dove finisce il senso: tagliarlo per
# paragrafi darebbe un blocco solo, e l'unica modifica proponibile sarebbe
# «riscrivi tutto il file».
controlla("ce n'e' uno per riga, non uno per tutto il file",
          len(bl) >= 6, f"{len(bl)} blocchi")
controlla("e ognuno dichiara di occupare una riga",
          all(b.get("righe") == 1 for b in bl), str(bl[:1]))

w9 = harness_finestra.costruisci(app)
w9.guarda()
c_e_motore = w9.pagina is not None
if c_e_motore:
    controlla("si apre sulla pagina disegnata, non sul codice",
              w9.modi.currentIndex() == 2, str(w9.modi.currentIndex()))
    controlla("e l'intestazione lo dice", "resa" in w9.intestazione.text(),
              w9.intestazione.text())
    controlla("c'e' il bottone per andare al codice",
              not w9.bottoneSorgente.isHidden()
              and w9.bottoneSorgente.text() == "Sorgente",
              w9.bottoneSorgente.text())

    print("\n20. il codice e' a un click, ed e' li' che si scrive")
    w9.scambiaVista()
    controlla("il bottone porta al foglio", w9.modi.currentIndex() == 1,
              str(w9.modi.currentIndex()))
    controlla("e il foglio ha il sorgente vero, tag compresi",
              "<script>" in w9.editor.toPlainText(),
              w9.editor.toPlainText()[:60])
    controlla("i ferri del Markdown spariscono su un sorgente",
              all(b.isHidden() for b in w9.ferriMd))
    controlla("il bottone adesso riporta indietro",
              w9.bottoneSorgente.text() == "Anteprima",
              w9.bottoneSorgente.text())

    print("\n21. e salvarlo non lo riscrive in Markdown")
    c = w9.editor.textCursor()
    w9.editor.find("Cruscotto vivo")
    w9.editor.insertPlainText("Cruscotto acceso")
    w9.salva()
    salvato = pagina_html.read_text(encoding="utf-8")
    controlla("il ritocco c'e'", "Cruscotto acceso" in salvato, salvato[-160:])
    controlla("il doctype e' sopravvissuto",
              salvato.lstrip().startswith("<!doctype html>"), salvato[:40])
    controlla("lo <style> pure", "<style>" in salvato)
    controlla("e non ci sono righe vuote infilate in mezzo",
              "\n\nbody{" not in salvato, repr(salvato[:200]))
    controlla("la copia di prima e' accanto",
              pagina_html.with_suffix(".html.prima").exists())

    print("\n22. una proposta su un artifact si legge nel codice")
    w9._sorgente_aperto = False
    w9._firma = None
    w9.guarda()
    controlla("si e' tornati alla pagina", w9.modi.currentIndex() == 2)
    riga_h1 = [b["id"] for b in harness._stato()["blocchi"]
               if "<h1" in b["testo"]][0]
    modifica.proponi([{"blocco": riga_h1, "azione": "sostituisci",
                       "testo": "<h1 id='t'>Cruscotto nuovo</h1>"}],
                     motivo="prova sull artifact")
    w9.guarda()
    controlla("una differenza non si mostra su una pagina disegnata: "
              "si passa al codice", w9.modi.currentIndex() == 1,
              str(w9.modi.currentIndex()))
    controlla("con l'anteprima dentro il sorgente",
              "Cruscotto nuovo" in w9.editor.toPlainText())
    w9.scartaProposta()
    controlla("scartare non tocca il file",
              "Cruscotto nuovo" not in pagina_html.read_text(encoding="utf-8"))
else:
    print("  (PyQt6-WebEngine non c'e': salto la parte resa)")
    controlla("senza motore si cade sul sorgente, non su una finta anteprima",
              w9.modi.currentIndex() == 1, str(w9.modi.currentIndex()))

print("\n23. su un sorgente si cambia una riga, non mezzo file")
p2 = Path(tempfile.mkdtemp(prefix="nova_htm_")) / "p.html"
fitto = ("<h1>uno</h1>\n"
         "<p>due</p>\n"
         "<p>tre</p>\n"
         "<p>quattro</p>\n")
p2.write_text(fitto, encoding="utf-8")
harness.apri(str(p2))
b_due = [b["id"] for b in harness._stato()["blocchi"] if "due" in b["testo"]][0]
modifica.proponi([{"blocco": b_due, "azione": "sostituisci",
                   "testo": "<p>DUE</p>"}])
modifica.applica()
finito = p2.read_text(encoding="utf-8")
controlla("la riga cambiata e' cambiata", "<p>DUE</p>" in finito, finito)
controlla("e le altre no",
          "<h1>uno</h1>" in finito and "<p>tre</p>" in finito
          and "<p>quattro</p>" in finito, finito)
controlla("il file ha ancora quattro righe",
          len([r for r in finito.splitlines() if r.strip()]) == 4, repr(finito))

# Togliere una riga di codice non deve portarsi via quella dopo: nei
# documenti a paragrafi la riga seguente e' vuota, qui e' altro codice.
harness.apri(str(p2))
b_tre = [b["id"] for b in harness._stato()["blocchi"] if "tre" in b["testo"]][0]
modifica.proponi([{"blocco": b_tre, "azione": "elimina"}])
modifica.applica()
dopo_tolta = p2.read_text(encoding="utf-8")
controlla("eliminare toglie solo quella riga",
          "<p>tre</p>" not in dopo_tolta, dopo_tolta)
controlla("e non si porta dietro la riga dopo",
          "<p>quattro</p>" in dopo_tolta, dopo_tolta)

print("\n24. una cartella si apre come progetto")
prog = Path(tempfile.mkdtemp(prefix="nova_prog_"))
(prog / "src").mkdir()
(prog / "index.html").write_text(
    "<h1>ciao</h1>\n<script src='src/app.js'></script>\n", encoding="utf-8")
(prog / "src" / "app.js").write_text(
    "function saluta(){\n  return 'ciao';\n}\n", encoding="utf-8")
(prog / "README.md").write_text("# Progetto\n\nUna prova.\n", encoding="utf-8")
(prog / "node_modules").mkdir()
(prog / "node_modules" / "roba.js").write_text("x", encoding="utf-8")
(prog / "logo.bin").write_bytes(b"\x00\x01\x02")

r = harness.apri(str(prog))
controlla("la cartella si apre", r.get("ok"), str(r.get("motivo")))
controlla("e dice quanti file ha", r.get("file_nel_progetto", 0) == 3,
          str(r.get("file_nel_progetto")))
s = harness._stato()
controlla("node_modules non entra nell'albero",
          not any("node_modules" in x for x in s["albero"]), str(s["albero"]))
controlla("e nemmeno un binario",
          not any(x.endswith(".bin") for x in s["albero"]), str(s["albero"]))
controlla("si parte da quello che si guarda, non dal primo in ordine",
          s["nome"] == "index.html", s["nome"])

wp = harness_finestra.costruisci(app)
wp.guarda()
controlla("la colonna dei file compare", not wp.colonnaFile.isHidden())
controlla("con dentro tutti i file", len(wp._voci) == 3, str(list(wp._voci)))
controlla("e il titolo del progetto", prog.name in wp.titoloProgetto.text(),
          wp.titoloProgetto.text())
controlla("il file aperto e' quello selezionato",
          wp.alberoFile.currentItem().data(0, Qt.ItemDataRole.UserRole)
          == "index.html",
          str(wp.alberoFile.currentItem().text(0)))

# La colonna mostra la forma della cartella, non un elenco di percorsi: un
# file dentro src sta sotto un ramo src, e si chiama app.js, non src/app.js.
controlla("le cartelle sono rami, non prefissi nel nome",
          wp._voci["src/app.js"].text(0) == "app.js",
          wp._voci["src/app.js"].text(0))
controlla("con la cartella sopra di loro",
          wp._voci["src/app.js"].parent() is not None
          and wp._voci["src/app.js"].parent().text(0) == "src",
          str(wp._voci["src/app.js"].parent()))
controlla("i file di primo livello non hanno un ramo sopra",
          wp._voci["index.html"].parent() is None)
# Un progetto tutto espanso sarebbe di nuovo un elenco: si apre solo la
# strada che porta al file aperto.
controlla("la cartella che non porta a niente di aperto resta chiusa",
          not wp._voci["src/app.js"].parent().isExpanded())

print("\n25. cliccare un file lo apre, e NOVA lo sa")
voce = wp._voci["src/app.js"]
wp.apriDallAlbero(voce)
controlla("si e' aperto il file cliccato",
          harness._stato()["nome"] == "app.js", harness._stato()["nome"])
# Non e' un fatto della finestra: e' scritto nello stato, quindi NOVA lo
# vede senza che nessuno glielo dica.
controlla("ed e' scritto nello stato, non solo sullo schermo",
          harness._stato()["file"].endswith("app.js"))
controlla("il progetto non e' sparito passando da un file all'altro",
          harness._stato().get("radice") == str(prog.resolve()),
          str(harness._stato().get("radice")))
controlla("la colonna e' ancora li'", not wp.colonnaFile.isHidden())
controlla("e la cartella del file aperto adesso e' aperta",
          wp._voci["src/app.js"].parent().isExpanded())
controlla("l'albero non e' stato ricostruito da capo",
          wp.alberoFile.currentItem() is wp._voci["src/app.js"])
controlla("e il foglio mostra il js", "function saluta" in wp.editor.toPlainText(),
          wp.editor.toPlainText()[:50])

print("\n26. una proposta su un file del progetto")
b_js = [b["id"] for b in harness._stato()["blocchi"]
        if "return" in b["testo"]][0]
modifica.proponi([{"blocco": b_js, "azione": "sostituisci",
                   "testo": "  return 'ciao a tutti';"}], motivo="prova")
wp.guarda()
controlla("l'anteprima e' nel sorgente del file giusto",
          "ciao a tutti" in wp.editor.toPlainText(),
          wp.editor.toPlainText()[:80])
wp.applicaProposta()
finito_js = (prog / "src" / "app.js").read_text(encoding="utf-8")
controlla("applicata sul file del progetto", "ciao a tutti" in finito_js,
          finito_js)
controlla("e le altre righe sono intatte",
          finito_js.startswith("function saluta(){"), finito_js)

print("\n27. il codice si vede, non solo si legge")
from nova import evidenzia                                    # noqa: E402
controlla("Pygments c'e'", evidenzia.disponibile())
# I linguaggi non li riconosciamo noi: scriverli a mano voleva dire
# sbagliarli su Rust, su Svelte e su tutto quello che non avevamo previsto.
for nome, atteso in [("a.py", "Python"), ("a.js", "JavaScript"),
                     ("a.html", "HTML"), ("a.rs", "Rust"),
                     ("a.toml", "TOML"), ("a.ps1", "PowerShell"),
                     ("a.sql", "SQL")]:
    lex = evidenzia.lessico(nome)
    controlla(f"{nome} lo riconosce", lex is not None and atteso in lex.name,
              str(lex))
controlla("un'estensione sconosciuta non e' un guasto",
          evidenzia.lessico("a.qwertyuiop") is None)

harness.apri(str(sorgente_py))
wc = harness_finestra.costruisci(app)
wc.guarda()
doc = wc.editor.document()
def _colore(frase, dentro):
    for b in _blocchi(doc):
        if frase not in b.text():
            continue
        i = b.text().index(dentro)
        for f in b.layout().formats():
            if f.start <= i < f.start + f.length:
                return f.format.foreground().color().name()
    return ""
controlla("le parole chiave sono blu come il pensiero",
          _colore("def somma", "def") == evidenzia.CHIAVE,
          _colore("def somma", "def"))
controlla("il nome definito e' brace",
          _colore("def somma", "somma") == evidenzia.DEFINITO,
          _colore("def somma", "somma"))
controlla("il commento e' spento",
          _colore("# *non*", "#") == evidenzia.COMMENTO,
          _colore("# *non*", "#"))
controlla("il fondo del codice e' scuro",
          evidenzia.FONDO_CODICE in wc.editor.styleSheet(),
          wc.editor.styleSheet()[:80])
controlla("i numeri di riga ci sono", not wc.numeriRiga.isHidden())

print("\n28. e un documento resta una pagina")
harness.apri(str(sorgente))
wc.guarda()
controlla("il foglio torna bianco", "#fbfaf8" in wc.editor.styleSheet(),
          wc.editor.styleSheet()[:80])
controlla("e i numeri di riga spariscono", wc.numeriRiga.isHidden())
colorati = [f for b in _blocchi(wc.editor.document())
            for f in b.layout().formats()
            if f.format.foreground().color().name() == evidenzia.CHIAVE]
controlla("niente colori da codice su un documento", not colorati,
          str(len(colorati)))

print("\n29. colorare non e' modificare")
# rehighlight() fa scattare textChanged: segnarlo come lavoro da salvare
# faceva credere al foglio di avere roba da perdere, e da li' si rifiutava
# di ricaricarsi - il danno vero non era la scritta, era il rifiuto.
harness.apri(str(sorgente_py))
wc.guarda()
controlla("aprendo un sorgente non risulta sporco", wc._sporco is False)
wc.evidenziatore._rileggi()
controlla("e ricolorarlo non lo sporca", wc._sporco is False)
# Qt segnala un cambio di formato come «N caratteri via, N dentro», non
# come zero: la regola dei zero copre l'evidenziatore, e le impaginazioni
# restano coperte dalla guardia di caricamento. Qui si prova la strada che
# un utente percorre davvero, cioe' lo zoom.
wc.ingrandisci(+1)
controlla("nemmeno ingrandire, che rifa' tutta l'impaginazione",
          wc._sporco is False)
wc.ingrandisci(0)
wc.editor.insertPlainText("x")
controlla("ma scrivere si'", wc._sporco is True)

print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
