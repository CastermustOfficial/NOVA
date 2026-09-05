"""Agire nel tempo: NOVA che fa qualcosa piu' tardi, o tutti i giorni.

Prima esisteva solo `create_reminder`, che sembrava una pianificazione ma
programmava un **fumetto di notifica**: NOVA non poteva darsi appuntamento con
se stessa. Qui si pianifica un'azione vera, cioe' una domanda che NOVA si
porra' da sola all'ora giusta.

Il meccanismo e' l'Utilita' di pianificazione di Windows: sopravvive al
riavvio del PC, che e' esattamente cio' che serve a «tutti i lunedi' alle 9».
Un timer dentro il demone morirebbe col demone.
"""
from __future__ import annotations

import datetime
import subprocess
import sys
import tempfile
from pathlib import Path

from .. import attivita
from ..scrittura import scrivi
from .base import Risk, ToolError, tool

PREFISSO = "NOVA_Compito_"


def _radice() -> Path:
    # nova/tools/tempo.py -> nova/tools -> nova -> radice
    return Path(__file__).resolve().parent.parent.parent


def _quando(when: str) -> datetime.datetime:
    when = (when or "").strip()
    try:
        if len(when) <= 5:
            t = datetime.datetime.strptime(when, "%H:%M").time()
            dt = datetime.datetime.combine(datetime.date.today(), t)
            if dt < datetime.datetime.now():
                dt += datetime.timedelta(days=1)
            return dt
        return datetime.datetime.fromisoformat(when.replace("T", " "))
    except ValueError:
        raise ToolError("l'ora va scritta come 'HH:MM' oppure 'YYYY-MM-DD HH:MM'")


# I giorni della settimana stanno in `nova/attivita.py`, insieme al resto di
# cio' che sa parlare con l'Utilita' di pianificazione: qui c'erano le sigle a
# tre lettere di `schtasks /D`, che con l'XML non servono piu'.


@tool(
    "pianifica",
    "Fa in modo che NOVA esegua un'istruzione piu' tardi, o a ripetizione. "
    "Diverso da create_reminder, che mostra solo una notifica: qui NOVA agisce "
    "davvero. Sopravvive al riavvio del computer.",
    {
        "istruzione": {"type": "string",
                       "description": "Cosa dovra' fare NOVA, scritto come glielo diresti"},
        "quando": {"type": "string",
                   "description": "'HH:MM' oppure 'YYYY-MM-DD HH:MM'"},
        "ripeti": {"type": "string",
                   "description": "Vuoto = una volta sola. Oppure: 'ogni giorno', "
                                  "'ogni lunedi', 'ogni settimana', 'ogni mese'"},
        "nome": {"type": "string", "description": "Come chiamarlo, per ritrovarlo dopo"},
    },
    Risk.MODERATE, required=["istruzione", "quando"], category="sistema",
    preview=lambda a: (f"Programma NOVA per {a.get('quando')}"
                       f"{' (' + a['ripeti'] + ')' if a.get('ripeti') else ''}: "
                       f"{a.get('istruzione')}"),
)
def pianifica(istruzione: str, quando: str, ripeti: str = "", nome: str = "") -> str:
    istruzione = (istruzione or "").strip()
    if not istruzione:
        raise ToolError("serve l'istruzione: cosa deve fare NOVA")

    dt = _quando(quando)
    etichetta = (nome or istruzione)[:40].strip()
    pulita = "".join(c if c.isalnum() or c in " -_" else "_" for c in etichetta).strip()
    task = PREFISSO + (pulita or dt.strftime("%Y%m%d%H%M%S"))

    r = (ripeti or "").strip().lower()
    giorno = ""
    if not r:
        ogni = ""
    elif r in ("ogni giorno", "giornaliero", "ogni giorni"):
        ogni = "giorno"
    elif r in ("ogni settimana", "settimanale"):
        ogni = "settimana"
    elif r in ("ogni mese", "mensile"):
        ogni = "mese"
    elif r.startswith("ogni ") and r[5:].replace("'", "").strip() in attivita.GIORNI_XML:
        ogni, giorno = "settimana", r[5:].replace("'", "").strip()
    else:
        raise ToolError(
            f"non capisco «{ripeti}». Usa: vuoto, 'ogni giorno', 'ogni settimana', "
            "'ogni mese' oppure 'ogni lunedi' (o un altro giorno)."
        )

    # L'istruzione va in un file, e nella riga di comando finisce solo il suo
    # percorso.
    #
    # Prima ci finiva dentro, fra virgolette doppie — al punto che le
    # virgolette nell'istruzione erano **vietate**, e chi voleva far dire a
    # NOVA «cerca "casa in affitto"» non poteva. E non bastava: misurato l'8
    # settembre, «controlla l'agenda» veniva registrata come «controlla
    # l"agenda». Un apostrofo diventato virgoletta, e NOVA che si sarebbe
    # posta una domanda diversa da quella chiesta senza che niente lo
    # segnalasse (D149).
    cartella = Path(tempfile.gettempdir()) / "nova-compiti"
    cartella.mkdir(parents=True, exist_ok=True)
    file_domanda = cartella / f"{task}.txt"
    scrivi(file_domanda, istruzione)

    # pythonw: senza finestra nera che compare all'improvviso mentre si lavora.
    exe = sys.executable.replace("python.exe", "pythonw.exe")
    if not Path(exe).exists():
        exe = sys.executable

    try:
        attivita.crea(
            task, dt, exe, f'-m nova --ask-file "{file_domanda}"',
            descrizione=istruzione, ripeti=ogni, giorno=giorno,
            # Una richiesta a NOVA puo' voler dire chiamare un modello: mezz'ora
            # e' quanto ci si puo' mettere prima che valga la pena fermarsi.
            durata_massima="PT30M")
    except attivita.AttivitaFallita as e:
        raise ToolError(str(e)) from e

    quando_umano = (dt.strftime("%d/%m/%Y alle %H:%M") if not r
                    else f"{ripeti}, alle {dt.strftime('%H:%M')}")
    return (f"Programmato «{task}»: {quando_umano}.\n"
            f"NOVA fara': {istruzione}\n"
            f"Per toglierlo: pianifica_togli con nome={task}")


@tool(
    "pianifica_elenco",
    "Elenca le cose che NOVA si e' data da fare piu' tardi.",
    {}, Risk.SAFE, category="sistema",
    preview=lambda a: "Guardo cosa ho in programma",
)
def pianifica_elenco() -> str:
    res = subprocess.run(["schtasks", "/Query", "/FO", "LIST", "/V"],
                         capture_output=True, text=True, timeout=45,
                         encoding="utf-8", errors="replace")
    if res.returncode != 0:
        raise ToolError("non riesco a leggere le attivita' pianificate")
    blocchi = res.stdout.split("\n\n")
    nostri = [b for b in blocchi if PREFISSO in b]
    if not nostri:
        return "NOVA non ha niente in programma."
    fuori = []
    for b in nostri:
        nome = prossima = azione = ""
        for riga in b.splitlines():
            k = riga.split(":", 1)
            if len(k) != 2:
                continue
            chiave, valore = k[0].strip().lower(), k[1].strip()
            # I nomi dei campi cambiano con la lingua di Windows. Su una
            # macchina italiana sono «Prossima esecuzione» e «Attivita' da
            # eseguire», su una inglese «Next Run Time» e «Task To Run»: si
            # riconoscono per pezzi di parola invece che per stringa esatta,
            # altrimenti l'elenco esce vuoto senza dire perche'.
            if "nome attivit" in chiave or "taskname" in chiave:
                nome = valore.strip("\\")
            elif "prossima esecuzione" in chiave or "next run" in chiave:
                prossima = valore
            elif "da eseguire" in chiave or "task to run" in chiave:
                azione = valore
        istruzione = azione.split('--ask "')[-1].rstrip('"') if "--ask" in azione else azione
        fuori.append(f"- {nome}\n    quando: {prossima}\n    fara': {istruzione}")
    return "In programma:\n" + "\n".join(fuori)


@tool(
    "pianifica_togli",
    "Toglie una cosa programmata. Il nome si trova con pianifica_elenco.",
    {"nome": {"type": "string", "description": "Nome dell'attivita', es. NOVA_Compito_backup"}},
    Risk.MODERATE, required=["nome"], category="sistema",
    preview=lambda a: f"Toglie dal programma: {a.get('nome')}",
)
def pianifica_togli(nome: str) -> str:
    nome = (nome or "").strip()
    if not nome.startswith(PREFISSO):
        raise ToolError(
            f"posso togliere solo le attivita' che ha creato NOVA (iniziano con {PREFISSO}). "
            "Le altre sono di Windows o di altri programmi: toglile tu se sei sicuro."
        )
    res = subprocess.run(["schtasks", "/Delete", "/TN", nome, "/F"],
                         capture_output=True, text=True, timeout=45)
    if res.returncode != 0:
        raise ToolError((res.stderr or res.stdout).strip()[:300])
    return f"Tolto dal programma: {nome}"