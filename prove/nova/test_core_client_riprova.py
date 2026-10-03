# -*- coding: utf-8 -*-
"""Il client del demone riprova quando la pipe e' occupata (D368).

Su Windows, subito dopo una connessione, il demone ha bisogno di un istante
per mettersi in ascolto della prossima. Chi arriva in quell'istante trova
«tutte le istanze occupate» (ERROR_PIPE_BUSY), che Python riporta come errno
22, e Windows prescrive di aspettare e riprovare. Misurato sul demone vero: due
client aperti uno dietro l'altro cadevano sempre (0 su 10), dopo un
millisecondo mai.

Questa prova non accende il demone: guarda la regola, con una `open` finta, una
piattaforma finta e un orologio finto, cosi' gira dappertutto e non dipende dal
caso. L'attesa massima e' un secondo d'orologio, anche quando una pausa dura
piu' di quanto chiesto, come su Windows; e quando finisce l'errore dice cosa e'
successo, non solo «Invalid argument». Il caso
vero lo guardano `test_demone_permessi.py` e `test_demone_claude.py`, che
tenevano due connessioni insieme e cadevano per questo.
"""
import errno
import sys
import types
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from nova import core_client                                      # noqa: E402

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


class FileFinto:
    def close(self):
        pass


def connetti_con(sequenza, piattaforma="win32", dura=None):
    """Esegue `connect()` con una `open` che risponde come dice `sequenza`.

    Ogni pausa manda avanti l'orologio finto di quanto chiesto, o di `dura`
    se e' data: una pausa che dura piu' del chiesto, come su Windows.
    Torna (client o l'errore, quante volte ha aperto, le pause chieste, il
    tempo passato sull'orologio).
    """
    aperture, pause, orologio = [], [], [0.0]

    def pausa(secondi):
        pause.append(secondi)
        orologio[0] += secondi if dura is None else dura

    def open_finta(percorso, modo, buffering=0):
        aperture.append(percorso)
        esito = sequenza[min(len(aperture) - 1, len(sequenza) - 1)]
        if isinstance(esito, BaseException):
            raise esito
        return FileFinto()

    originali = {k: getattr(core_client, k, None) for k in ("open", "sys", "time")}
    core_client.open = open_finta
    core_client.sys = types.SimpleNamespace(platform=piattaforma)
    core_client.time = types.SimpleNamespace(sleep=pausa, monotonic=lambda: orologio[0])
    try:
        try:
            esito = core_client.CoreClient(r"\\.\pipe\finta").connect()
        except Exception as e:                                   # noqa: BLE001
            esito = e
    finally:
        for k, v in originali.items():
            if v is None:
                delattr(core_client, k)
            else:
                setattr(core_client, k, v)
    return esito, len(aperture), pause, orologio[0]


occupata = OSError(errno.EINVAL, "Invalid argument")

print("1. la pipe occupata si aspetta e si riprova")
esito, aperture, pause, _ = connetti_con([occupata, occupata, "ok"])
controlla("dopo due volte occupata, la terza si apre", not isinstance(esito, OSError), repr(esito))
controlla("ha aperto tre volte", aperture == 3, str(aperture))
controlla("e ha aspettato fra un tentativo e l'altro, e non di piu'", pause == [0.01, 0.01], str(pause))

print("\n2. una pipe che non c'e' non si riprova")
esito, aperture, pause, _ = connetti_con([FileNotFoundError(errno.ENOENT, "non c'e'")])
controlla("l'errore passa com'e'", isinstance(esito, FileNotFoundError), repr(esito))
controlla("al primo colpo, senza pause", aperture == 1 and pause == [], f"{aperture} aperture, pause {pause}")

print("\n3. una pipe sempre occupata non fa aspettare per sempre")
esito, aperture, pause, passato = connetti_con([occupata])
controlla("alla fine l'errore arriva", isinstance(esito, OSError) and esito.errno == errno.EINVAL, repr(esito))
controlla("dopo un secondo, non prima", 1.0 <= passato < 1.0 + 0.01 + 1e-9, f"{passato:.3f} s")
controlla("e l'errore dice cosa e' successo, e dove",
          "occupata per 1 s" in str(esito) and getattr(esito, "filename", None) == r"\\.\pipe\finta",
          repr(esito))
# Su Windows una pausa di dieci millisecondi ne dura quasi sedici: contando
# cento pause si aspettava un secondo e mezzo.
esito, aperture, pause, passato = connetti_con([occupata], dura=0.0156)
controlla("anche con pause che durano piu' del chiesto, un secondo",
          1.0 <= passato < 1.0 + 0.0156 + 1e-9, f"{passato:.3f} s, {aperture} aperture")

print("\n4. fuori da Windows non cambia niente")
esito, aperture, pause, _ = connetti_con([occupata], piattaforma="linux")
controlla("non passa dalla pipe", aperture == 0 and pause == [], f"{aperture} aperture")

print(f"\n{passati}/{passati + len(falliti)} passati")
for f in falliti:
    print("  FALLITO:", f)
sys.exit(1 if falliti else 0)
