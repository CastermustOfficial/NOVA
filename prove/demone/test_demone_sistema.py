# -*- coding: utf-8 -*-
"""Gli appunti, il volume, le notifiche, l'ora e com'e' fatto il PC, dal demone.

Questa famiglia era **gia' collegata e non la chiedeva nessuno**. I tratti
stavano in `nova-strumenti::capacita` — «copia questo testo negli appunti»,
detto con le parole di NOVA e non con quelle di Windows (D130) — e
`nova-core::caps_sistema` li implementava tutti passando da `nova-platform`.
Poi quel modulo non veniva registrato: dal demone gli appunti, il volume e le
notifiche **non esistevano**, e una prova che diceva «i tratti hanno qualcuno
dietro» passava lo stesso, perche' guardava il collegamento e non la porta.

Quel che si prova qui e' la porta. Una capacita' implementata e non
registrata e' esattamente come una che manca, con in piu' il codice che fa
credere il contrario.

**Le risposte dipendono dalla macchina, e va bene.** Su un agente di
compilazione non c'e' una sessione interattiva: gli appunti non ci sono, la
scheda audio nemmeno. L'esito giusto li' e' «ha risposto», non «ha
funzionato» — chiedere il verde vorrebbe dire spegnere la prova sulle
macchine dove NOVA gira davvero (D53). Cio' che invece **non** dipende dalla
macchina — che la richiesta vuota si rifiuti, che un guasto dica di essere un
guasto, che l'ora abbia la forma giusta — si pretende sempre.

Esce 2 — «qui non si puo' provare» — se il demone non e' costruito.
"""
import json
import os
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "novad.exe" if os.name == "nt" else "novad"
DEMONE = next((p for p in (RADICE / "core" / "target" / "release" / NOME,
                           RADICE / "core" / "target" / "debug" / NOME)
               if p.is_file()), None)
if DEMONE is None:
    print("Il demone non e' costruito per questo sistema. Per averlo:")
    print("  cd core && cargo build --release -p novad")
    sys.exit(2)

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


from nova.core_client import CoreClient                           # noqa: E402

casa = tempfile.mkdtemp(prefix="nova-sistema-")
(Path(casa) / "NOVA").mkdir(parents=True, exist_ok=True)
(Path(casa) / "NOVA" / "config.json").write_text("{}", encoding="utf-8")

endpoint = (rf"\\.\pipe\nova-sistema-{os.getpid()}" if os.name == "nt"
            else str(Path(casa) / "nova.sock"))
ambiente = dict(os.environ)
ambiente["APPDATA"] = casa
ambiente["HOME"] = casa
ambiente["USERPROFILE"] = casa
ambiente["XDG_CONFIG_HOME"] = casa
ambiente["XDG_RUNTIME_DIR"] = casa

processo = subprocess.Popen(
    [str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
    env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)


def esito(c, nome, args=None):
    """Quel che il modello leggerebbe: la risposta, o il guasto.

    Qui contano tutti e due. Una capacita' che su questa macchina non puo'
    funzionare deve **dire** che non puo', e quel messaggio e' un risultato
    quanto l'altro: e' l'unica cosa che distingue «manca il sistema» da «hai
    sbagliato richiesta».
    """
    try:
        return c.call(nome, args or {}), None
    except Exception as e:                                        # noqa: BLE001
        return None, str(e)


try:
    scadenza = time.time() + 20
    pronto = False
    while time.time() < scadenza and processo.poll() is None:
        if CoreClient.disponibile(endpoint):
            pronto = True
            break
        time.sleep(0.3)
    if not pronto:
        fine = (processo.stderr.read() or b"").decode("utf-8", "replace")[-400:]
        print(f"  il demone non ha risposto su {endpoint}: {fine}")
        processo.kill()
        sys.exit(2)

    print("\n1. le capacita' sono registrate, non solo implementate")
    with CoreClient(endpoint, timeout=30) as c:
        tutte = {x["name"]: x for x in c.request("capabilities/list")["capabilities"]}
    attese = ["sys.ora", "sys.appunti_leggi", "sys.appunti_scrivi",
              "sys.volume", "sys.notifica", "sys.info", "sys.digita", "sys.tasti"]
    mancano = [n for n in attese if n not in tutte]
    controlla("le otto capacita' di sistema ci sono", not mancano, str(mancano))
    # I tasti non hanno un bersaglio: meno che «pericolose» vorrebbe dire
    # premerli senza chiedere.
    controlla("e le due della tastiera sono dichiarate pericolose",
              all(tutte.get(n, {}).get("risk") == "dangerous"
                  for n in ("sys.digita", "sys.tasti")),
              str([(n, tutte.get(n, {}).get("risk")) for n in ("sys.digita", "sys.tasti")]))
    # Scrivere negli appunti butta via quel che c'era: non e' «sicuro».
    controlla("e copiare negli appunti non e' dichiarato innocuo",
              tutte.get("sys.appunti_scrivi", {}).get("risk") != "safe",
              str(tutte.get("sys.appunti_scrivi")))

    print("\n2. l'ora ha la forma che ha nel Python")
    with CoreClient(endpoint, timeout=30) as c:
        ora, guasto = esito(c, "sys.ora")
    controlla("risponde", guasto is None, str(guasto))
    controlla("con giorno, data e ora nella forma di NOVA",
              isinstance(ora, str) and re.fullmatch(
                  r"(lunedi|martedi|mercoledi|giovedi|venerdi|sabato|domenica) "
                  r"\d{2}/\d{2}/\d{4} \d{2}:\d{2}:\d{2}", ora or ""),
              repr(ora))
    # E il giorno della settimana e' quello vero, non il primo dell'elenco:
    # una tabella scalata di uno non si vede finche' non e' domenica.
    from nova.tools.system import get_datetime                     # noqa: E402
    controlla("e il giorno della settimana e' quello che dice anche il Python",
              isinstance(ora, str) and ora.split()[0] == get_datetime().split()[0],
              f"{ora!r} vs {get_datetime()!r}")

    print("\n3. gli appunti passano dal sistema, e se non c'e' lo dicono")
    with CoreClient(endpoint, timeout=30) as c:
        scritto, g_scritto = esito(c, "sys.appunti_scrivi", {"text": "ciao NOVA"})
        letto, g_letto = esito(c, "sys.appunti_leggi")
        vuoto, g_vuoto = esito(c, "sys.appunti_scrivi", {})
    if g_scritto is None:
        controlla("copiare dice quanti caratteri", "9 caratteri" in str(scritto),
                  str(scritto))
        controlla("e rileggendoli si ritrova quel che si e' scritto",
                  letto == "ciao NOVA", repr(letto))
    else:
        # Su una macchina senza sessione grafica gli appunti non ci sono:
        # l'esito giusto e' un guasto che lo **dice**, non una stringa vuota
        # che il modello scambierebbe per «gli appunti erano vuoti».
        controlla("dove gli appunti non ci sono, il guasto lo dice",
                  len(g_scritto) > 10, repr(g_scritto))
        controlla("e anche leggerli fallisce invece di rispondere vuoto",
                  g_letto is not None or letto == "(appunti vuoti)",
                  f"{letto!r} / {g_letto!r}")
    # Questo invece non dipende dalla macchina.
    controlla("copiare senza testo si rifiuta prima di toccare il sistema",
              g_vuoto is not None and "text" in g_vuoto, repr(g_vuoto))

    print("\n4. il volume vuole sapere cosa fare")
    with CoreClient(endpoint, timeout=30) as c:
        niente, g_niente = esito(c, "sys.volume", {})
        _zero, g_zero = esito(c, "sys.volume", {"level": 0})
    controlla("senza ne' livello ne' muto si rifiuta",
              g_niente is not None and "serve 'level' o 'mute'" in g_niente,
              repr(g_niente))
    # Zero e' un volume, non un'assenza: se il demone lo confondesse con «non
    # lo ha chiesto» risponderebbe come sopra invece di provarci.
    controlla("mentre «zero» e' una richiesta, e ci prova",
              g_zero is None or "serve 'level' o 'mute'" not in g_zero,
              repr(g_zero))

    print("\n5. la notifica torna subito")
    with CoreClient(endpoint, timeout=30) as c:
        quando = time.time()
        _n, g_n = esito(c, "sys.notifica", {"message": "prova"})
        durata = time.time() - quando
        _senza, g_senza = esito(c, "sys.notifica", {})
    # Non si guarda se il fumetto compare — non c'e' modo, e qui non compare
    # comunque. Si guarda l'unica cosa che il tratto promette e che era il
    # difetto: che chi chiama non resti li'. Prima erano novemila millisecondi.
    controlla("chi la chiede non resta ad aspettare che sparisca", durata < 3,
              f"{durata:.1f}s")
    controlla("e senza messaggio si rifiuta",
              g_senza is not None and "message" in g_senza, repr(g_senza))

    print("\n6. com'e' fatto il PC, e non solo il nome del sistema")
    with CoreClient(endpoint, timeout=30) as c:
        pc, g_pc = esito(c, "sys.info")
    controlla("risponde", g_pc is None, str(g_pc))
    pc = pc or {}
    controlla("con quel che si sa comunque",
              all(k in pc for k in ("os", "arch", "user", "home", "cpus")),
              str(sorted(pc))[:200])
    if "macchina" in pc:
        controlla("e col racconto della macchina, non con numeri sciolti",
                  "Acceso da" in pc["macchina"] and "RAM" in pc["macchina"],
                  str(pc["macchina"])[:250])
        controlla("piu' i numeri, per chi ci deve fare un conto",
                  isinstance(pc.get("ram_totale_byte"), int)
                  and pc["ram_totale_byte"] > 0,
                  str(pc.get("ram_totale_byte")))
    else:
        # Se il sistema non lo sa dire, lo dice: una risposta piu' corta che
        # sembra completa e' peggio di un campo che spiega cosa manca.
        controlla("o, se il sistema non lo sa dire, spiega perche'",
                  isinstance(pc.get("macchina_non_letta"), str)
                  and len(pc["macchina_non_letta"]) > 10,
                  str(pc.get("macchina_non_letta")))

    print("\n7. la tastiera guarda chi c'e' davanti, e senza nessuno non preme")
    # Su questa macchina puo' non esserci nessuna finestra col fuoco — su un
    # agente di compilazione non c'e' nemmeno uno schermo. E' proprio il caso
    # da provare: la risposta giusta e' **non premere** e dirlo. Se invece
    # una finestra c'e', questa prova non ci scrive: non e' sua (vedi
    # `prove/macchina/test_tastiera.py`, che scrive solo in una finestra che
    # apre lei).
    with CoreClient(endpoint, timeout=30) as c:
        # «prova: true» e' l'anteprima: dice cosa succederebbe e non lo fa.
        chi, g_chi = esito(c, "sys.digita", {"text": "ciao", "prova": True})
    controlla("l'anteprima risponde, e non esegue",
              g_chi is None and (chi or {}).get("eseguito") is False, repr(g_chi or chi)[:200])
    farei = str((chi or {}).get("farei", ""))
    controlla("l'anteprima dice quale finestra, o che non sa dirlo",
              "La finestra e': «" in farei or "non riesco a dire quale sia" in farei,
              farei[:200])
    controlla("e dice che non si annulla",
              (chi or {}).get("annullabile") is False, str(chi)[:200])
    if (chi or {}).get("finestra") is None:
        with CoreClient(endpoint, timeout=30) as c:
            _d, g_d = esito(c, "sys.digita", {"text": "ciao", "delay_seconds": 0})
            _t, g_t = esito(c, "sys.tasti", {"keys": "ctrl+s"})
        controlla("senza nessuno davanti, digitare si rifiuta",
                  g_d is not None and "non premo niente" in g_d, repr(g_d))
        # «focus_window» e' il nome del Python: dal demone quello strumento
        # non esiste, e un consiglio che non si puo' seguire non e' un aiuto.
        controlla("e consiglia lo strumento che questa meta' ha davvero",
                  g_d is not None and "«ui.focus»" in g_d
                  and "focus_window" not in g_d, repr(g_d))
        controlla("e premere una combinazione pure", g_t is not None, repr(g_t))
    else:
        print("  (davanti c'e' una finestra che non e' nostra: non ci scrivo)")

finally:
    processo.terminate()
    try:
        processo.wait(timeout=10)
    except subprocess.TimeoutExpired:
        processo.kill()

print(f"\n{passati} controlli passati, {len(falliti)} falliti")
if falliti:
    print("::error::test_demone_sistema: " + "; ".join(falliti))
    sys.exit(1)
sys.exit(0)
