# -*- coding: utf-8 -*-
"""Il client del demone riprova quando la pipe e' occupata (D368).

Su Windows, subito dopo una connessione, il demone ha bisogno di un istante
per mettersi in ascolto della prossima. Chi arriva in quell'istante trova
«tutte le istanze occupate» (ERROR_PIPE_BUSY), che Python riporta come errno
22, e Windows prescrive di aspettare e riprovare. Misurato sul demone vero: due
client aperti uno dietro l'altro cadevano sempre (0 su 10), dopo un
millisecondo mai.

Questa prova non accende il demone: guarda la regola, con una `open` finta e
una piattaforma finta, cosi' gira dappertutto e non dipende dal caso. Il caso
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


def connetti_con(sequenza, piattaforma="win32"):
    """Esegue `connect()` con una `open` che risponde come dice `sequenza`.

    Torna (client o l'errore, quante volte ha aperto, quante pause ha fatto).
    """
    aperture, pause = [], []

    def open_finta(percorso, modo, buffering=0):
        aperture.append(percorso)
        esito = sequenza[min(len(aperture) - 1, len(sequenza) - 1)]
        if isinstance(esito, BaseException):
            raise esito
        return FileFinto()

    originali = {k: getattr(core_client, k, None) for k in ("open", "sys", "time")}
    core_client.open = open_finta
    core_client.sys = types.SimpleNamespace(platform=piattaforma)
    core_client.time = types.SimpleNamespace(sleep=pause.append)
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
    return esito, len(aperture), pause


occupata = OSError(errno.EINVAL, "Invalid argument")

print("1. la pipe occupata si aspetta e si riprova")
esito, aperture, pause = connetti_con([occupata, occupata, "ok"])
controlla("dopo due volte occupata, la terza si apre", not isinstance(esito, OSError), repr(esito))
controlla("ha aperto tre volte", aperture == 3, str(aperture))
controlla("e ha aspettato fra un tentativo e l'altro, e non di piu'", pause == [0.01, 0.01], str(pause))

print("\n2. una pipe che non c'e' non si riprova")
esito, aperture, pause = connetti_con([FileNotFoundError(errno.ENOENT, "non c'e'")])
controlla("l'errore passa com'e'", isinstance(esito, FileNotFoundError), repr(esito))
controlla("al primo colpo, senza pause", aperture == 1 and pause == [], f"{aperture} aperture, pause {pause}")

print("\n3. una pipe sempre occupata non fa aspettare per sempre")
esito, aperture, pause = connetti_con([occupata])
controlla("alla fine l'errore arriva", isinstance(esito, OSError) and esito.errno == errno.EINVAL, repr(esito))
controlla("dopo cento tentativi", aperture == 100, str(aperture))
controlla("per un secondo al massimo", sum(pause) <= 1.0 + 1e-9, f"{sum(pause):.2f} s")

print("\n4. fuori da Windows non cambia niente")
esito, aperture, pause = connetti_con([occupata], piattaforma="linux")
controlla("non passa dalla pipe", aperture == 0 and pause == [], f"{aperture} aperture")

print(f"\n{passati}/{passati + len(falliti)} passati")
for f in falliti:
    print("  FALLITO:", f)
sys.exit(1 if falliti else 0)
