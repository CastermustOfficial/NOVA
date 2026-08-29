"""NOVA che ripara se stessa, ma su un banco.

Il giro e' sempre lo stesso, e nessun passo si salta:

    ripara_apri      -> una copia di NOVA in una cartella a parte, con dentro
                        esattamente il codice che sta girando, e la misura di
                        quali prove passano **adesso**
    (si lavora li' dentro con read_file / write_file, sul percorso restituito)
    ripara_verifica  -> rimisura e confronta con la partenza
    ripara_applica   -> solo se il confronto regge; mette da parte gli
                        originali prima di scrivere
    ripara_butta     -> smonta il banco

La regola del confronto non e' «tutte le prove verdi»: e' **nessuna prova che
era verde diventa rossa**. La differenza conta davvero, perche' in questo
progetto una prova e' rossa da prima (`test_voce.py`, una falsa segnalazione su
una chiave di prova); con la regola severa nessuna riparazione passerebbe mai,
e l'unica via d'uscita sarebbe spegnere il controllo - cioe' peggio di niente.

Perche' il banco e non direttamente il codice vero: perche' il modo piu' rapido
di rompere un assistente in maniera irreparabile e' lasciare che si ripari da
solo mentre e' rotto. Sul banco puo' sbagliare quante volte vuole.
"""
from __future__ import annotations

from .base import Risk, ToolError, tool


def _banco():
    from .. import banco
    return banco


@tool(
    "ripara_apri",
    "Apre un banco di prova: una copia di NOVA in una cartella a parte, con "
    "dentro il codice che sta girando adesso. Restituisce il percorso in cui "
    "lavorare e quali prove passano di partenza. Da usare prima di toccare "
    "qualunque file del progetto NOVA: sul banco si puo' sbagliare senza "
    "conseguenze. Ci mette una ventina di secondi, perche' misura la partenza.",
    {"motivo": {"type": "string",
                "description": "Cosa si sta cercando di riparare, in una riga"}},
    Risk.MODERATE, required=["motivo"], category="sistema",
    preview=lambda a: f"Apre un banco di prova per: {a.get('motivo', '')}",
)
def ripara_apri(motivo: str) -> str:
    b = _banco()
    s = b.apri(motivo)
    p = s["partenza"]
    righe = [
        f"banco {s['id']} aperto in {s['cartella']}",
        f"partenza: {len(p['verdi'])} prove verdi"
        + (f", rosse gia' adesso: {', '.join(p['rosse'])}" if p["rosse"] else ""),
        "Lavora sui file dentro quella cartella, poi chiama ripara_verifica.",
    ]
    return "\n".join(righe)


@tool(
    "ripara_verifica",
    "Rimisura le prove nel banco e le confronta con la partenza. Dice se la "
    "modifica regge, quali prove sono diventate rosse, quali si sono riparate "
    "e quali file sono stati toccati.",
    {"banco": {"type": "string", "description": "L'identificativo del banco"}},
    Risk.MODERATE, required=["banco"], category="sistema",
    preview=lambda a: f"Esegue le prove nel banco {a.get('banco', '')}",
)
def ripara_verifica(banco: str) -> str:
    b = _banco()
    try:
        v = b.verifica(banco)
    except RuntimeError as e:
        raise ToolError(str(e))

    righe = ["regge: " + ("si'" if v["regge"] else "no")]
    if v["file_toccati"]:
        righe.append("file toccati: " + ", ".join(v["file_toccati"]))
    else:
        righe.append("nel banco non e' cambiato niente")
    if v["regressioni"]:
        righe.append("diventate rosse: " + ", ".join(v["regressioni"]))
    if v["riparate"]:
        righe.append("riparate: " + ", ".join(v["riparate"]))
    if v["sparite"]:
        righe.append("prove sparite (non vale): " + ", ".join(v["sparite"]))
    if v["fuori_perimetro"]:
        righe.append("fuori perimetro: " + ", ".join(v["fuori_perimetro"]))
    if v["regge"] and v["file_toccati"]:
        righe.append("Si puo' applicare con ripara_applica.")
    return "\n".join(righe)


@tool(
    "ripara_applica",
    "Porta nel programma vero la modifica che nel banco ha retto. Rifiuta se "
    "la verifica non e' stata fatta o se non regge. Mette da parte gli "
    "originali: si torna indietro con riparazione_annulla.",
    {"banco": {"type": "string", "description": "L'identificativo del banco"}},
    # DANGEROUS e non MODERATE di proposito: qui NOVA riscrive se stessa. Con
    # «conferma sempre» e «solo rischiose» la domanda deve arrivare; in
    # «autonomo» passa, ed e' accettabile perche' e' stata provata e si annulla.
    Risk.DANGEROUS, required=["banco"], category="sistema",
    preview=lambda a: f"Scrive nel codice di NOVA quanto provato nel banco {a.get('banco', '')}",
)
def ripara_applica(banco: str) -> str:
    b = _banco()
    try:
        r = b.applica(banco)
    except RuntimeError as e:
        raise ToolError(str(e))
    righe = [f"riparazione {r['id']} applicata: " + ", ".join(r["file"])]
    if r["verdetto"]["riparate"]:
        righe.append("prove riparate: " + ", ".join(r["verdetto"]["riparate"]))
    righe.append("Per tornare indietro: riparazione_annulla con "
                 f"riparazione={r['id']}.")
    # Il codice nuovo entra in vigore al prossimo avvio: questo processo ha
    # gia' in memoria i moduli di prima. Dirlo evita di cercare per mezz'ora
    # perche' la correzione «non ha avuto effetto».
    righe.append("Il codice nuovo vale dal prossimo avvio di NOVA.")
    return "\n".join(righe)


@tool(
    "ripara_butta",
    "Smonta il banco di prova. Le riparazioni gia' applicate restano.",
    {"banco": {"type": "string", "description": "L'identificativo del banco"}},
    Risk.MODERATE, required=["banco"], category="sistema",
    preview=lambda a: f"Smonta il banco {a.get('banco', '')}",
)
def ripara_butta(banco: str) -> str:
    _banco().butta(banco)
    return f"banco {banco} smontato"


@tool(
    "riparazioni_elenco",
    "Cosa NOVA ha cambiato di se stessa, dalla piu' recente, e cosa e' gia' "
    "stato annullato.",
    {}, Risk.SAFE, category="sistema",
)
def riparazioni_elenco() -> str:
    import datetime
    elenco = _banco().riparazioni()
    if not elenco:
        return "nessuna riparazione registrata"
    righe = []
    for r in elenco[:20]:
        quando = datetime.datetime.fromtimestamp(r["quando"]).strftime("%d/%m %H:%M")
        stato = " (annullata)" if r["annullata"] else ""
        righe.append(f"{r['id']}  {quando}  {r['motivo'] or 'senza motivo'}"
                     f"  [{', '.join(r['file'])}]{stato}")
    return "\n".join(righe)


@tool(
    "riparazione_annulla",
    "Rimette il codice com'era prima di una riparazione. Funziona anche a "
    "distanza di giorni: gli originali sono su disco.",
    {"riparazione": {"type": "string",
                     "description": "L'identificativo dato da ripara_applica"}},
    Risk.MODERATE, required=["riparazione"], category="sistema",
    preview=lambda a: f"Rimette il codice com'era prima della riparazione {a.get('riparazione', '')}",
)
def riparazione_annulla(riparazione: str) -> str:
    try:
        r = _banco().annulla(riparazione)
    except RuntimeError as e:
        raise ToolError(str(e))
    pezzi = []
    if r["rimessi"]:
        pezzi.append("rimessi com'erano: " + ", ".join(r["rimessi"]))
    if r["rimossi"]:
        pezzi.append("tolti (non c'erano prima): " + ", ".join(r["rimossi"]))
    pezzi.append("Vale dal prossimo avvio di NOVA.")
    return "\n".join(pezzi) if pezzi else "non c'era niente da rimettere"
