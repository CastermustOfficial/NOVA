"""Tool su file e cartelle."""
from __future__ import annotations

import os
import shutil
import subprocess
from datetime import datetime
from pathlib import Path

from .base import Risk, ToolError, tool

MAX_READ_CHARS = 40000


def _p(path: str) -> Path:
    if not path or not str(path).strip():
        raise ToolError("percorso vuoto")
    return Path(os.path.expandvars(os.path.expanduser(str(path)))).resolve()


def _fmt(entry: Path) -> str:
    try:
        st = entry.stat()
        when = datetime.fromtimestamp(st.st_mtime).strftime("%Y-%m-%d %H:%M")
        if entry.is_dir():
            return f"[DIR ] {entry.name}/  ({when})"
        size = st.st_size
        unit = "B"
        for u in ("KB", "MB", "GB"):
            if size >= 1024:
                size /= 1024
                unit = u
            else:
                break
        return f"[FILE] {entry.name}  {size:.0f} {unit}  ({when})"
    except OSError as e:
        return f"[????] {entry.name}  (non accessibile: {e})"


@tool(
    "list_directory",
    "Elenca file e sottocartelle di una cartella. Usalo per orientarti prima di agire.",
    {
        "path": {"type": "string", "description": "Percorso assoluto della cartella"},
        "pattern": {"type": "string", "description": "Filtro glob opzionale, es. *.pdf"},
        "show_hidden": {"type": "boolean", "description": "Includi elementi nascosti"},
    },
    Risk.SAFE, required=["path"], category="file",
    preview=lambda a: f"Elenca la cartella {a.get('path')}",
)
def list_directory(path: str, pattern: str = "*", show_hidden: bool = False) -> str:
    d = _p(path)
    if not d.exists():
        raise ToolError(f"la cartella {d} non esiste")
    if not d.is_dir():
        raise ToolError(f"{d} non e' una cartella")
    items = sorted(d.glob(pattern or "*"), key=lambda x: (not x.is_dir(), x.name.lower()))
    if not show_hidden:
        items = [i for i in items if not i.name.startswith(".")]
    if not items:
        return f"{d}: nessun elemento corrispondente a '{pattern}'."
    head = [f"{d}  ({len(items)} elementi)"]
    body = [_fmt(i) for i in items[:300]]
    if len(items) > 300:
        body.append(f"... e altri {len(items) - 300} elementi")
    return "\n".join(head + body)


@tool(
    "read_file",
    "Legge il contenuto testuale di un file. Usalo prima di modificarlo.",
    {
        "path": {"type": "string", "description": "Percorso assoluto del file"},
        "offset": {"type": "integer", "description": "Prima riga da leggere (1-based)"},
        "limit": {"type": "integer", "description": "Numero massimo di righe"},
    },
    Risk.SAFE, required=["path"], category="file",
    preview=lambda a: f"Legge il file {a.get('path')}",
)
def read_file(path: str, offset: int = 1, limit: int = 0) -> str:
    f = _p(path)
    if not f.exists():
        raise ToolError(f"il file {f} non esiste")
    if f.is_dir():
        raise ToolError(f"{f} e' una cartella, usa list_directory")
    try:
        text = f.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        try:
            text = f.read_text(encoding="cp1252")
        except Exception:
            return f"{f} non e' un file di testo ({f.stat().st_size} byte)."
    lines = text.splitlines()
    start = max(0, (offset or 1) - 1)
    end = start + limit if limit else len(lines)
    chunk = "\n".join(lines[start:end])
    if len(chunk) > MAX_READ_CHARS:
        chunk = chunk[:MAX_READ_CHARS] + f"\n... [troncato, file di {len(lines)} righe]"
    return f"{f} (righe {start + 1}-{min(end, len(lines))} di {len(lines)}):\n{chunk}"


@tool(
    "write_file",
    "Crea un file nuovo o riscrive completamente un file esistente.",
    {
        "path": {"type": "string", "description": "Percorso assoluto del file"},
        "content": {"type": "string", "description": "Contenuto completo da scrivere"},
        "append": {"type": "boolean", "description": "Aggiungi in coda invece di sovrascrivere"},
    },
    Risk.MODERATE, required=["path", "content"], category="file",
    preview=lambda a: (
        f"Scrive {len(a.get('content') or '')} caratteri in {a.get('path')}"
        + (" (in coda)" if a.get("append") else "")
    ),
)
def write_file(path: str, content: str, append: bool = False, ctx=None) -> str:
    f = _p(path)
    if ctx is not None:
        ctx.guard_write(f)
    f.parent.mkdir(parents=True, exist_ok=True)
    existed = f.exists()
    with open(f, "a" if append else "w", encoding="utf-8", newline="\n") as fh:
        fh.write(content)
    verb = "aggiornato" if existed else "creato"
    return f"File {verb}: {f} ({f.stat().st_size} byte)"


@tool(
    "edit_file",
    "Sostituisce una porzione esatta di testo dentro un file. Leggi prima il file.",
    {
        "path": {"type": "string", "description": "Percorso assoluto del file"},
        "old_text": {"type": "string", "description": "Testo esatto da sostituire"},
        "new_text": {"type": "string", "description": "Testo sostitutivo"},
        "replace_all": {"type": "boolean", "description": "Sostituisci tutte le occorrenze"},
    },
    Risk.MODERATE, required=["path", "old_text", "new_text"], category="file",
    preview=lambda a: f"Modifica {a.get('path')} sostituendo un blocco di testo",
)
def edit_file(path: str, old_text: str, new_text: str, replace_all: bool = False, ctx=None) -> str:
    f = _p(path)
    if ctx is not None:
        ctx.guard_write(f)
    if not f.exists():
        raise ToolError(f"il file {f} non esiste")
    text = f.read_text(encoding="utf-8")
    n = text.count(old_text)
    if n == 0:
        raise ToolError("testo da sostituire non trovato; rileggi il file")
    if n > 1 and not replace_all:
        raise ToolError(f"'old_text' compare {n} volte: rendilo univoco o usa replace_all")
    f.write_text(text.replace(old_text, new_text, -1 if replace_all else 1), encoding="utf-8")
    return f"Modificato {f} ({n if replace_all else 1} sostituzioni)"


@tool(
    "create_folder",
    "Crea una cartella (e le cartelle intermedie).",
    {"path": {"type": "string", "description": "Percorso assoluto della nuova cartella"}},
    Risk.MODERATE, category="file",
    preview=lambda a: f"Crea la cartella {a.get('path')}",
)
def create_folder(path: str, ctx=None) -> str:
    d = _p(path)
    if ctx is not None:
        ctx.guard_write(d)
    if d.exists():
        return f"La cartella {d} esiste gia'."
    d.mkdir(parents=True, exist_ok=True)
    return f"Cartella creata: {d}"


def _nel_cestino(t: Path) -> bool:
    """Manda nel Cestino invece di distruggere. True se ci e' riuscito.

    Serve perche' il Cestino e' l'annullamento che Windows regala gia': un file
    che ci finisce si recupera con due clic, uno cancellato davvero no. Vale la
    premessa N2 del progetto — prima la reversibilita', poi il permesso.
    """
    try:
        from send2trash import send2trash  # type: ignore
        send2trash(str(t))
        return True
    except Exception:
        pass
    try:
        ps = (
            "Add-Type -AssemblyName Microsoft.VisualBasic; "
            + ("[Microsoft.VisualBasic.FileIO.FileSystem]::DeleteDirectory("
               if t.is_dir() else
               "[Microsoft.VisualBasic.FileIO.FileSystem]::DeleteFile(")
            + f"'{t}','OnlyErrorDialogs','SendToRecycleBin')"
        )
        r = subprocess.run(["powershell", "-NoProfile", "-Command", ps],
                           capture_output=True, text=True, timeout=60)
        return r.returncode == 0
    except Exception:
        return False


@tool(
    "move_path",
    "Sposta o rinomina un file o una cartella.",
    {
        "source": {"type": "string", "description": "Percorso di origine"},
        "destination": {"type": "string", "description": "Percorso di destinazione"},
        "overwrite": {"type": "boolean", "description": "Sovrascrivi la destinazione"},
    },
    Risk.MODERATE, required=["source", "destination"], category="file",
    preview=lambda a: f"Sposta {a.get('source')} -> {a.get('destination')}",
)
def move_path(source: str, destination: str, overwrite: bool = False, ctx=None) -> str:
    s, d = _p(source), _p(destination)
    if ctx is not None:
        ctx.guard_write(s)
        ctx.guard_write(d)
    if not s.exists():
        raise ToolError(f"{s} non esiste")
    if d.exists() and not overwrite:
        raise ToolError(f"{d} esiste gia'; usa overwrite=true per sovrascrivere")
    if d.exists() and overwrite:
        # Chi chiede di spostare non ha chiesto di distruggere cio' che c'era.
        # La destinazione va nel Cestino: se era la cosa sbagliata si recupera.
        if not _nel_cestino(d):
            raise ToolError(
                f"{d} esiste e non riesco a metterlo nel Cestino: mi fermo invece "
                f"di cancellarlo per sempre. Spostalo o eliminalo tu, poi riprova."
            )
    d.parent.mkdir(parents=True, exist_ok=True)
    shutil.move(str(s), str(d))
    return f"Spostato: {s} -> {d}"


@tool(
    "copy_path",
    "Copia un file o una cartella.",
    {
        "source": {"type": "string", "description": "Percorso di origine"},
        "destination": {"type": "string", "description": "Percorso di destinazione"},
    },
    Risk.MODERATE, required=["source", "destination"], category="file",
    preview=lambda a: f"Copia {a.get('source')} -> {a.get('destination')}",
)
def copy_path(source: str, destination: str, ctx=None) -> str:
    s, d = _p(source), _p(destination)
    if ctx is not None:
        ctx.guard_write(d)
    if not s.exists():
        raise ToolError(f"{s} non esiste")
    if s.is_dir():
        shutil.copytree(s, d, dirs_exist_ok=True)
    else:
        d.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(s, d)
    return f"Copiato: {s} -> {d}"


@tool(
    "delete_path",
    "Sposta un file o una cartella nel Cestino (o elimina definitivamente se richiesto).",
    {
        "path": {"type": "string", "description": "Percorso da eliminare"},
        "permanent": {"type": "boolean", "description": "Elimina definitivamente invece del Cestino"},
    },
    Risk.DANGEROUS, required=["path"], category="file",
    preview=lambda a: (
        ("ELIMINA DEFINITIVAMENTE " if a.get("permanent") else "Sposta nel Cestino ")
        + str(a.get("path"))
    ),
)
def delete_path(path: str, permanent: bool = False, ctx=None) -> str:
    t = _p(path)
    if ctx is not None:
        ctx.guard_write(t)
    if not t.exists():
        raise ToolError(f"{t} non esiste")
    if not permanent:
        try:
            from send2trash import send2trash  # type: ignore
            send2trash(str(t))
            return f"Spostato nel Cestino: {t}"
        except ImportError:
            pass
        ps = (
            "Add-Type -AssemblyName Microsoft.VisualBasic; "
            + ("[Microsoft.VisualBasic.FileIO.FileSystem]::DeleteDirectory("
               if t.is_dir() else
               "[Microsoft.VisualBasic.FileIO.FileSystem]::DeleteFile(")
            + f"'{t}','OnlyErrorDialogs','SendToRecycleBin')"
        )
        r = subprocess.run(["powershell", "-NoProfile", "-Command", ps],
                           capture_output=True, text=True, timeout=60)
        if r.returncode == 0:
            return f"Spostato nel Cestino: {t}"
        raise ToolError(f"impossibile usare il Cestino: {r.stderr.strip()[:300]}")
    shutil.rmtree(t) if t.is_dir() else t.unlink()
    return f"Eliminato definitivamente: {t}"


@tool(
    "search_files",
    "Cerca file per nome (glob) dentro una cartella, ricorsivamente.",
    {
        "root": {"type": "string", "description": "Cartella da cui partire"},
        "pattern": {"type": "string", "description": "Glob, es. **/*.docx oppure fattura*"},
        "max_results": {"type": "integer", "description": "Numero massimo di risultati"},
    },
    Risk.SAFE, required=["root", "pattern"], category="file",
    preview=lambda a: f"Cerca '{a.get('pattern')}' in {a.get('root')}",
)
def search_files(root: str, pattern: str, max_results: int = 100) -> str:
    d = _p(root)
    if not d.is_dir():
        raise ToolError(f"{d} non e' una cartella")
    if "*" not in pattern and "?" not in pattern:
        pattern = f"**/*{pattern}*"
    elif not pattern.startswith("**"):
        pattern = f"**/{pattern}"
    out: list[str] = []
    try:
        for p in d.glob(pattern):
            out.append(str(p))
            if len(out) >= max(1, max_results):
                break
    except OSError as e:
        raise ToolError(f"errore durante la ricerca: {e}")
    return "\n".join(out) if out else f"Nessun risultato per '{pattern}' in {d}."


@tool(
    "search_in_files",
    "Cerca una stringa dentro i file di testo di una cartella (grep).",
    {
        "root": {"type": "string", "description": "Cartella da cui partire"},
        "query": {"type": "string", "description": "Testo da cercare"},
        "file_pattern": {"type": "string", "description": "Glob dei file da ispezionare, es. **/*.py"},
        "max_results": {"type": "integer", "description": "Numero massimo di righe trovate"},
    },
    Risk.SAFE, required=["root", "query"], category="file",
    preview=lambda a: f"Cerca il testo '{a.get('query')}' nei file di {a.get('root')}",
)
def search_in_files(root: str, query: str, file_pattern: str = "**/*", max_results: int = 60) -> str:
    d = _p(root)
    hits: list[str] = []
    q = query.lower()
    for p in d.glob(file_pattern):
        try:
            if not p.is_file() or p.stat().st_size > 5000000:
                continue
            for i, line in enumerate(p.read_text(encoding="utf-8", errors="ignore").splitlines(), 1):
                if q in line.lower():
                    hits.append(f"{p}:{i}: {line.strip()[:200]}")
                    if len(hits) >= max_results:
                        return "\n".join(hits) + "\n[limite risultati raggiunto]"
        except OSError:
            continue
    return "\n".join(hits) if hits else f"Nessuna occorrenza di '{query}' in {d}."


@tool(
    "open_path",
    "Apre un file o una cartella con l'applicazione predefinita di Windows (Esplora risorse, Word, ...).",
    {"path": {"type": "string", "description": "Percorso da aprire"}},
    Risk.MODERATE, category="file",
    preview=lambda a: f"Apre {a.get('path')} in Windows",
)
def open_path(path: str) -> str:
    t = _p(path)
    if not t.exists():
        raise ToolError(f"{t} non esiste")
    os.startfile(str(t))  # type: ignore[attr-defined]
    return f"Aperto: {t}"


@tool(
    "path_info",
    "Restituisce metadati di un file o cartella (esistenza, dimensione, date).",
    {"path": {"type": "string", "description": "Percorso da ispezionare"}},
    Risk.SAFE, category="file",
    preview=lambda a: f"Info su {a.get('path')}",
)
def path_info(path: str) -> dict:
    t = _p(path)
    if not t.exists():
        return {"path": str(t), "exists": False}
    st = t.stat()
    return {
        "path": str(t),
        "exists": True,
        "type": "directory" if t.is_dir() else "file",
        "size_bytes": st.st_size,
        "modified": datetime.fromtimestamp(st.st_mtime).isoformat(timespec="seconds"),
        "created": datetime.fromtimestamp(st.st_ctime).isoformat(timespec="seconds"),
    }


@tool(
    "known_folders",
    "Elenca i percorsi delle cartelle note dell'utente (Desktop, Download, Documenti, ...).",
    {},
    Risk.SAFE, required=[], category="file",
    preview=lambda a: "Elenca le cartelle note dell'utente",
)
def known_folders() -> dict:
    home = Path.home()
    out = {"home": str(home)}
    for name in ("Desktop", "Downloads", "Documents", "Documenti", "Pictures",
                 "Immagini", "Music", "Musica", "Videos", "Video"):
        p = home / name
        if p.exists():
            out[name.lower()] = str(p)
    out["temp"] = os.environ.get("TEMP", "")
    out["appdata"] = os.environ.get("APPDATA", "")
    return out
