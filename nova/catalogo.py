"""Quale modello ha senso su questa macchina, e quale non si scarica affatto.

Un modulo di sola libreria standard, come `modelli_trova`: lo chiama
l'installatore, e l'installatore gira prima che le dipendenze esistano.

## Il numero che decide

Non i gigabyte del file, e nemmeno i parametri totali. Generare un token, a
una richiesta per volta, non e' un lavoro di calcolo: e' **leggere i pesi
dalla memoria**. La velocita' e' quindi `banda / byte letti per token`, e i
byte letti sono i parametri che si **accendono** moltiplicati per quanti bit
ciascuno occupa. La quantizzazione entra nel conto quanto l'architettura: un
denso a un bit e un MoE a tre bit possono finire nella stessa categoria.

Le due misure da cui esce la soglia, stessa macchina, stesso prompt, zero
layer sulla GPU:

    Qwen3.8 27B Q4_K_M (denso)     15,7 GB/token   1,8 tok/s
    Gemma 4 26B-A4B Q3 (MoE)       ~3,7 GB/token   7,6 tok/s

La stessa banda implicita (~28 GB/s) spiega tutte e due le righe, il che vuol
dire che il modello di costo e' quello giusto e non una spiegazione costruita
su un numero solo.

## Perche' esiste

L'installatore, quando la VRAM non si leggeva, scaricava comunque la variante
piu' leggera e ci scriveva accanto «su questa macchina andra' piano». Per un
denso da 27B non e' «piu' piano»: sono tredici gigabyte scaricati per un
programma che si apre una volta e mai piu'. Un avvertimento piu' grosso non
ripara niente - chi installa clicca avanti, e ha ragione, perche' gli abbiamo
appena detto che si puo' fare. **La forma giusta di dire «non farlo» e' non
offrirlo.**

Ma «senza GPU mai» sarebbe sbagliato quanto il contrario: un modello leggero
da leggere funziona benissimo sul processore, e sette token al secondo sono
piu' veloci di quanto legga una persona. Percio' qui non si guarda se c'e'
una scheda video: si guarda il numero.
"""
from __future__ import annotations

from dataclasses import dataclass
from typing import Any

# Sotto questa soglia si sta sopra i sette token al secondo su una macchina
# come quella su cui e' stata misurata (banda ~28 GB/s, DDR4 a due canali).
# Su una piu' lenta serve di meno, su una DDR5 si puo' osare di piu': e' una
# stima onesta, non una legge, e sta scritta in un posto solo perche' quando
# arriveranno altre misure si cambi qui.
SOGLIA_GB_PER_TOKEN = 5.0


def soglia(catalogo: dict | None = None) -> float:
    """La soglia, dal catalogo se c'e' scritta.

    Sta in `models.json` per la stessa ragione per cui ci stanno i modelli:
    non e' codice, e' un dato. Quando arriveranno misure da altre macchine si
    cambia il file, non si fa una release.
    """
    if catalogo:
        v = (catalogo.get("regole_di_adattamento") or {}).get("soglia_gb_per_token")
        if isinstance(v, (int, float)) and v > 0:
            return float(v)
    return SOGLIA_GB_PER_TOKEN

# Quanto del file si rilegge a ogni token quando il modello e' denso: tutto.
DENSO = 1.0

# Quanti gigabyte di RAM lasciare al sistema quando il modello gira sul
# processore. Sul processore il file non sta in VRAM: sta in RAM, tutto, e
# accanto ci devono stare Windows, il browser e NOVA stessa. Senza questo
# margine un modello «abbastanza veloce» entra sulla carta e in pratica manda
# la macchina a paginare su disco, che e' un altro modo di essere lentissimi -
# e stavolta con la ventola accesa.
MARGINE_RAM_GB = 4.0


def frazione_letta(famiglia: dict[str, Any]) -> float:
    """La parte del file che si rilegge a ogni token.

    E' un dato della famiglia, non della variante: dipende
    dall'architettura, non da quanto si e' compressa. Chi non la dichiara e'
    trattato come denso — che e' il caso peggiore, ed e' la direzione giusta
    in cui sbagliare.
    """
    v = famiglia.get("frazione_letta")
    if isinstance(v, (int, float)) and 0 < float(v) <= 1.0:
        return float(v)
    return DENSO


def gb_per_token(famiglia: dict[str, Any], variante: dict[str, Any]) -> float:
    """I gigabyte che si leggono per produrre un token."""
    return round(float(variante.get("gb") or 0.0) * frazione_letta(famiglia), 2)


def piu_grande_che_entra(famiglia: dict[str, Any], vram_gb: float):
    """La variante piu' grande che sta in VRAM, o `None`.

    E' la regola di sempre: si sceglie la piu' grande che **entra**, non la
    piu' grande che si riesce a caricare. Se non ci sta tutta, llama.cpp
    mette una parte dei layer in RAM e funziona lo stesso, dieci volte piu'
    piano, senza dire niente.
    """
    if not vram_gb:
        return None
    entrano = [v for v in famiglia.get("varianti", [])
               if float(v.get("vram_gb") or 0) <= float(vram_gb)]
    return max(entrano, key=lambda v: float(v.get("gb") or 0)) if entrano else None


def varianti_senza_gpu(famiglia: dict[str, Any],
                       soglia_gb: float | None = None,
                       ram_gb: float = 0.0) -> list[dict[str, Any]]:
    """Le varianti utilizzabili sul solo processore, dalla piu' grande.

    Due vincoli, non uno, e li ha trovati una prova che sbagliava per il
    motivo giusto:

    - **la velocita'**, che sono i byte letti per token;
    - **lo spazio**, che e' il file intero. Sulla GPU il file sta in VRAM;
      sul processore sta in RAM, e accanto ci devono stare il sistema, il
      browser e NOVA. Un modello abbastanza veloce ma troppo grande entra
      sulla carta e in pratica manda la macchina a paginare su disco - un
      altro modo di essere lentissimi, stavolta con la ventola accesa.

    `ram_gb` a zero vuol dire «non lo so»: si controlla solo la velocita'.
    """
    s = SOGLIA_GB_PER_TOKEN if soglia_gb is None else soglia_gb
    ok = []
    for v in famiglia.get("varianti", []):
        if gb_per_token(famiglia, v) > s:
            continue
        if ram_gb and float(v.get("gb") or 0) > float(ram_gb) - MARGINE_RAM_GB:
            continue
        ok.append(v)
    return sorted(ok, key=lambda v: float(v.get("gb") or 0), reverse=True)


@dataclass
class Verdetto:
    """Cosa fare, e cosa dire mentre lo si fa."""
    si_scarica: bool
    variante: dict[str, Any] | None = None
    motivo: str = ""
    suggerimento: str = ""
    #: `True` quando il modello girera' sul processore: va detto prima.
    in_cpu: bool = False


def verdetto(famiglia: dict[str, Any], vram_gb: float,
             soglia_gb: float | None = None,
             ram_gb: float = 0.0) -> Verdetto:
    """Se offrire lo scaricamento, quale variante, e cosa dire."""
    scelta = piu_grande_che_entra(famiglia, vram_gb)
    if scelta is not None:
        return Verdetto(si_scarica=True, variante=scelta)

    # Niente scheda video utilizzabile, o niente che ci stia dentro: decide
    # il numero, non la presenza della GPU.
    leggere = varianti_senza_gpu(famiglia, soglia_gb, ram_gb)
    if leggere:
        v = leggere[0]
        return Verdetto(
            si_scarica=True, variante=v, in_cpu=True,
            motivo=(f"Girera' sul processore: legge circa "
                    f"{gb_per_token(famiglia, v):.1f} GB per token, quindi "
                    f"qualche token al secondo. Si usa, ma non e' veloce."),
        )

    varianti = famiglia.get("varianti", [])
    minimo = min((gb_per_token(famiglia, v) for v in varianti), default=0.0)
    nome = famiglia.get("nome") or famiglia.get("id") or "questo modello"

    # Due rifiuti diversi meritano due frasi diverse: chi ha poca RAM puo'
    # comprarne, chi ha un modello troppo denso no.
    if minimo <= (SOGLIA_GB_PER_TOKEN if soglia_gb is None else soglia_gb) and ram_gb:
        piccolo = min((float(v.get("gb") or 0) for v in varianti), default=0.0)
        return Verdetto(
            si_scarica=False,
            motivo=(f"{nome} sarebbe abbastanza veloce sul processore, ma il "
                    f"file piu' piccolo pesa {piccolo:.0f} GB e sul processore "
                    f"il modello sta in RAM, tutto: con {ram_gb:.0f} GB non ci "
                    f"sta insieme al resto. Non te lo faccio scaricare."),
            suggerimento=("Serve una variante piu' compressa della stessa "
                          "famiglia, oppure un abbonamento che hai gia' o una "
                          "chiave API: NOVA funziona lo stesso."),
        )

    return Verdetto(
        si_scarica=False,
        motivo=(f"{nome} legge almeno {minimo:.1f} GB per ogni token che "
                f"scrive, e senza scheda video quella lettura la fa la "
                f"memoria di sistema: verrebbe meno di un token al secondo. "
                f"Non te lo faccio scaricare: sono gigabyte per un programma "
                f"che poi non apriresti."),
        suggerimento=("Puoi provare piu' avanti, dalle impostazioni, con un "
                      "modello che di parametri ne accende pochi - quelli "
                      "vanno bene anche senza scheda video. Nel frattempo "
                      "NOVA funziona con un abbonamento che hai gia' o con "
                      "una chiave API."),
    )


def _principale() -> int:
    """Il verdetto in JSON, per chi non parla Python.

    Legge da standard input `{famiglia, vram_gb, catalogo}` e scrive una riga
    di JSON. Lo usa `install.ps1`, che di questo conto non deve avere una
    copia sua: due copie della stessa regola sono due regole destinate a
    divergere - si aggiorna una e non l'altra, e nessuno se ne accorge finche'
    qualcuno non si ritrova sul disco tredici gigabyte che non gli servono.
    """
    import json
    import sys

    try:
        dentro = json.loads(sys.stdin.read() or "{}")
    except Exception:                                       # noqa: BLE001
        print(json.dumps({"si_scarica": False, "file": None,
                          "motivo": "Non ho capito la domanda sul modello.",
                          "suggerimento": "Configura il cervello dopo, "
                                          "dalle impostazioni di NOVA."},
                         ensure_ascii=False))
        return 0

    famiglia = dentro.get("famiglia") or {}
    v = verdetto(famiglia,
                 float(dentro.get("vram_gb") or 0),
                 soglia(dentro.get("catalogo")),
                 float(dentro.get("ram_gb") or 0))
    print(json.dumps({
        "si_scarica": v.si_scarica,
        "file": (v.variante or {}).get("file"),
        "gb": (v.variante or {}).get("gb"),
        "motivo": v.motivo,
        "suggerimento": v.suggerimento,
        "in_cpu": v.in_cpu,
    }, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(_principale())
