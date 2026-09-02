# -*- coding: utf-8 -*-
"""COM-11. Il modello locale che non vede, e cosa succede se lo si ignora.

`llama-server` avviato senza `--mmproj` non e' un modello a cui manca una
funzione comoda: e' un modello che **rifiuta** ogni richiesta con dentro
un'immagine. Misurato il 2 settembre, Gemma 4 26B-A4B su questa macchina,
server acceso senza proiettore, una chiamata OpenAI con un `image_url`:

    HTTP 500
    {"error":{"code":500,
              "message":"image input is not supported - hint: if this is
                         unexpected, you may need to provide the mmproj",
              "type":"server_error"}}

Tre difetti, non uno.

**Il primo e' una bugia.** `spiega_http` mandava ogni 5xx a «il problema e'
dall'altra parte, non tua: di solito passa da solo». Questo non passa da
solo: e' un file che non e' stato scaricato, e chi aspetta aspetta per
sempre.

**Il secondo e' un muro.** Il messaggio con l'immagine resta in
conversazione, quindi il turno dopo la rimanda, e fallisce uguale. Non e' un
turno perso: e' una conversazione che non riparte piu' finche' non la si
butta.

**Il terzo e' che nessuno chiedeva prima.** `runtime` sapeva gia' se il
proiettore c'era — lo cercava per decidere se passare `--mmproj` — ma chi
allegava le figure non glielo domandava.

Questa prova non accende nessun server: prova le tre decisioni.
"""
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from nova.agent import Agent                                  # noqa: E402
from nova.brains.base import LimiteUso                        # noqa: E402
from nova.guasti import senza_vista, spiega_http              # noqa: E402
from nova.runtime import proiettore_accanto                   # noqa: E402

passati = 0
falliti: list[str] = []


def controlla(nome, condizione, dettaglio=""):
    global passati
    if condizione:
        passati += 1
        print(f"  [ok ] {nome}")
    else:
        falliti.append(nome + (f" — {dettaglio}" if dettaglio else ""))
        print(f"  [NO ] {nome}" + (f" — {dettaglio}" if dettaglio else ""))


# Il corpo vero, copiato dalla risposta misurata e non riscritto a memoria.
CORPO_VERO = ('{"error":{"code":500,"message":"image input is not supported '
              '- hint: if this is unexpected, you may need to provide the '
              'mmproj","type":"server_error"}}')


print("\n=== Riconoscere il rifiuto ===")
controlla("il corpo misurato viene riconosciuto", senza_vista(CORPO_VERO))
controlla("basta la parola mmproj", senza_vista("please provide the MMPROJ"))
controlla("basta la frase, se un giorno il suggerimento sparisce",
          senza_vista("image input is not supported"))
controlla("un 500 qualunque non viene scambiato per questo",
          not senza_vista('{"error":{"message":"internal server error"}}'))
controlla("il vuoto non e' cecita'", not senza_vista(""))
controlla("e None nemmeno", not senza_vista(None))


print("\n=== Non dire che passa da solo ===")
detto = spiega_http(500, CORPO_VERO, "Modello locale")
# Attenzione al sottotesto: la frase giusta *contiene* «passa da solo»,
# preceduto da un «Non». Cercare la sottostringa nuda faceva fallire la prova
# proprio sulla risposta corretta — un test che boccia la cura e' peggio di
# nessun test.
controlla("non promette che si aggiusta da se'",
          "di solito passa da solo" not in detto.lower(), detto)
controlla("anzi, dice l'opposto", "non passa da solo" in detto.lower(), detto)
controlla("nomina il proiettore, che e' la cosa da scaricare",
          "mmproj" in detto.lower(), detto)
controlla("dice che il resto funziona", "resto" in detto.lower(), detto)
generico = spiega_http(500, "internal server error", "Il fornitore")
controlla("un 5xx vero resta un 5xx vero", "passa da solo" in generico,
          generico)


print("\n=== Chiedere prima, invece di scoprirlo dopo ===")
import tempfile                                               # noqa: E402
with tempfile.TemporaryDirectory() as tmp:
    cieco = Path(tmp) / "cieco"
    cieco.mkdir()
    (cieco / "modello-Q4.gguf").write_bytes(b"x")
    vedente = Path(tmp) / "vedente"
    vedente.mkdir()
    (vedente / "modello-Q4.gguf").write_bytes(b"x")
    (vedente / "mmproj-F16.gguf").write_bytes(b"x")

    controlla("senza proiettore accanto: nessuno",
              proiettore_accanto(str(cieco / "modello-Q4.gguf")) is None)
    controlla("col proiettore accanto: lo trova",
              proiettore_accanto(str(vedente / "modello-Q4.gguf")) is not None)
    controlla("un percorso che non esiste non fa esplodere niente",
              proiettore_accanto(str(Path(tmp) / "niente" / "x.gguf")) is None)
    controlla("e nemmeno il vuoto", proiettore_accanto("") is None)

    class FintoCfg:
        """La sola parte di configurazione che questa domanda tocca."""

        def __init__(self, attivo, modello):
            self.brains = type("B", (), {"active": attivo, "visione": True})()
            self.server = type("S", (), {"model_path": modello})()

    class FintoAgent(Agent):
        """Un Agent vero, senza cervello: si eredita, non si ricopia."""

        def __init__(self, cfg):                              # noqa: D107
            self.cfg = cfg
            self.messages = []

    controlla("il modello locale senza proiettore non vede",
              not FintoAgent(FintoCfg("locale",
                                      str(cieco / "modello-Q4.gguf")))
              ._vede_il_cervello())
    controlla("col proiettore vede",
              FintoAgent(FintoCfg("locale",
                                  str(vedente / "modello-Q4.gguf")))
              ._vede_il_cervello())
    controlla("un'API vede comunque, il proiettore e' affare di llama.cpp",
              FintoAgent(FintoCfg("api", str(cieco / "modello-Q4.gguf")))
              ._vede_il_cervello())

    print("\n=== La figura che non si manda ===")
    figura = cieco / "schermata.png"
    figura.write_bytes(b"non e' un PNG vero, e non serve che lo sia")
    a = FintoAgent(FintoCfg("locale", str(cieco / "modello-Q4.gguf")))
    a._consegna_immagini(f"Schermata salvata in {figura}")
    controlla("un messaggio arriva lo stesso", len(a.messages) == 1,
              f"{len(a.messages)} messaggi")
    solo = a.messages[0]["content"] if a.messages else ""
    controlla("e non contiene nessuna immagine",
              isinstance(solo, str) and "base64" not in solo)
    controlla("dice al modello di non fingere di aver guardato",
              "non dire di averla guardata" in str(solo).lower(), str(solo))


print("\n=== Sfilare la figura invece di murare la conversazione ===")
IMMAGINE = {"type": "image_url",
            "image_url": {"url": "data:image/jpeg;base64,QUJD"}}


def conversazione():
    return [
        {"role": "system", "content": "sistema"},
        {"role": "user", "content": "guarda lo schermo"},
        {"role": "user", "content": [{"type": "text", "text": "[immagine: x.png]"},
                                     IMMAGINE]},
    ]


a = Agent.__new__(Agent)
a.messages = conversazione()
controlla("con l'errore giusto le sfila",
          a._sfila_le_immagini(RuntimeError(spiega_http(500, CORPO_VERO))))
controlla("l'immagine se n'e' andata",
          all("base64" not in str(m.get("content")) for m in a.messages))
controlla("il testo e' rimasto",
          "[immagine: x.png]" in str(a.messages[-1]["content"]),
          str(a.messages[-1]["content"]))
controlla("e la conversazione ha ancora tutti i suoi messaggi",
          len(a.messages) == 3, f"{len(a.messages)}")
controlla("una seconda volta non c'e' piu' niente da sfilare",
          not a._sfila_le_immagini(RuntimeError(CORPO_VERO)))

b = Agent.__new__(Agent)
b.messages = conversazione()
controlla("un errore che parla d'altro non tocca niente",
          not b._sfila_le_immagini(RuntimeError("il disco e' pieno")))
controlla("e infatti l'immagine e' ancora li'",
          any("base64" in str(m.get("content")) for m in b.messages))

c = Agent.__new__(Agent)
c.messages = [{"role": "user", "content": "nessuna figura qui"}]
controlla("senza figure torna falso, cosi' chi chiama rilancia invece di "
          "riprovare uguale",
          not c._sfila_le_immagini(RuntimeError(CORPO_VERO)))


print("\n=== L'ordine dei rami, che e' dove sbagliavo ===")
# `LimiteUso` eredita da `RuntimeError`. Il ramo nuovo, se sta sopra quello
# della quota, se lo mangia: il gradino non va piu' in pausa e il ripiego non
# parte mai. Non e' un'ipotesi: e' come l'avevo scritto la prima volta.
controlla("la quota finita e' un RuntimeError, quindi l'ordine conta",
          issubclass(LimiteUso, RuntimeError))
import inspect                                                # noqa: E402
sorgente = inspect.getsource(Agent._giro)
controlla("nel codice il ramo della quota viene prima di quello generico",
          sorgente.index("except LimiteUso")
          < sorgente.index("except RuntimeError"))
d = Agent.__new__(Agent)
d.messages = conversazione()
controlla("e una quota finita non somiglia a un cervello cieco",
          not d._sfila_le_immagini(
              LimiteUso("la quota e' finita per adesso.")))

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
