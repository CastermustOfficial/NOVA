"""Seed iniziale della KB: NOVA ispeziona il PC e si costruisce la mappa di base.

Quattro strati: profilo e preferenze, progetti e cartelle, app e ambiente,
persone. Tutto marcato `origine: scansione` cosi' si distingue da cio' che
NOVA impara parlando con te.
"""
from __future__ import annotations

import getpass
import json
import os
import platform
import subprocess
from datetime import date
from pathlib import Path

from .schema import ORIGINE_SCANSIONE, Node, slugify
from .store import Vault

CARTELLE_PROGETTI = ["Desktop", "Documents", "Documenti", "progettoX", "source", "repos", "dev"]
IGNORA = {"node_modules", "__pycache__", ".venv", "venv", "dist", "build",
          ".git", "AppData", "OneDrive"}


def _ps(cmd: str, timeout: int = 60) -> str:
    try:
        r = subprocess.run(["powershell", "-NoProfile", "-NonInteractive", "-Command", cmd],
                           capture_output=True, text=True, timeout=timeout,
                           encoding="utf-8", errors="replace")
        return (r.stdout or "").strip()
    except Exception:
        return ""


# ------------------------------------------------------------------ 1. profilo
def nodo_profilo() -> Node:
    try:
        utente = getpass.getuser()
    except Exception:
        utente = "utente"
    nome_git = _ps("git config --global user.name")
    mail_git = _ps("git config --global user.email")
    corpo = [
        f"- **Utente Windows**: `{utente}` su `{platform.node()}`",
        f"- **Cartella home**: `{Path.home()}`",
        f"- **Sistema**: {platform.system()} {platform.release()}",
    ]
    if nome_git:
        corpo.append(f"- **Identita' git**: {nome_git} <{mail_git}>")
    corpo.append("")
    corpo.append("Vedi anche [[preferenze-di-lavoro]] e [[ambiente-tecnico]].")
    return Node(
        slug="profilo-utente",
        title=f"Profilo di {nome_git or utente}",
        body="\n".join(corpo),
        tipo="profilo",
        tags=["profilo", "identita"],
        relazioni=["preferenze-di-lavoro", "ambiente-tecnico"],
        origine=ORIGINE_SCANSIONE,
        confidenza=0.95,
    )


def nodo_preferenze() -> Node:
    return Node(
        slug="preferenze-di-lavoro",
        title="Preferenze di lavoro",
        body=(
            "Come NOVA deve comportarsi. Questo nodo si arricchisce da solo "
            "man mano che emergono preferenze nelle conversazioni.\n\n"
            "- **Lingua**: italiano\n"
            "- **Stile risposte**: brevi e concrete, niente preamboli\n"
            "- **Azioni**: eseguire invece di spiegare come si farebbe\n"
        ),
        tipo="preferenza",
        tags=["preferenze", "stile"],
        relazioni=["profilo-utente"],
        origine=ORIGINE_SCANSIONE,
        confidenza=0.8,
    )


# ------------------------------------------------------- 2. progetti e cartelle
def trova_progetti(max_progetti: int = 40) -> list[dict]:
    home = Path.home()
    trovati: dict[str, dict] = {}
    for nome in CARTELLE_PROGETTI:
        radice = home / nome
        if not radice.is_dir():
            continue
        for figlio in _sottocartelle(radice, profondita=2):
            git_dir = figlio / ".git"
            marcatori = [m for m in ("package.json", "pyproject.toml", "requirements.txt",
                                     "Cargo.toml", "go.mod", "README.md", "index.html")
                         if (figlio / m).exists()]
            if not git_dir.exists() and not marcatori:
                continue
            remote = ""
            if git_dir.exists():
                remote = _ps(f'git -C "{figlio}" remote get-url origin', timeout=20)
            trovati[str(figlio).lower()] = {
                "path": str(figlio),
                "nome": figlio.name,
                "git": bool(git_dir.exists()),
                "remote": remote,
                "marcatori": marcatori,
                "readme": _prima_riga_readme(figlio),
            }
            if len(trovati) >= max_progetti:
                return list(trovati.values())
    return list(trovati.values())


def _sottocartelle(radice: Path, profondita: int) -> list[Path]:
    out: list[Path] = []
    try:
        for p in radice.iterdir():
            if not p.is_dir() or p.name in IGNORA or p.name.startswith("."):
                continue
            out.append(p)
            if profondita > 1:
                out.extend(_sottocartelle(p, profondita - 1))
    except OSError:
        pass
    return out


def _prima_riga_readme(cartella: Path) -> str:
    for nome in ("README.md", "readme.md", "README.MD"):
        f = cartella / nome
        if f.exists():
            try:
                for riga in f.read_text(encoding="utf-8", errors="ignore").splitlines():
                    riga = riga.strip().lstrip("#").strip()
                    if len(riga) > 12:
                        return riga[:200]
            except OSError:
                pass
    return ""


def nodo_progetto(p: dict) -> Node:
    corpo = [f"- **Cartella**: `{p['path']}`"]
    if p["remote"]:
        corpo.append(f"- **Repository**: {p['remote']}")
    if p["marcatori"]:
        corpo.append(f"- **Stack rilevato**: {', '.join(p['marcatori'])}")
    if p["readme"]:
        corpo.append(f"\n{p['readme']}")
    tags = ["progetto"]
    if p["git"]:
        tags.append("git")
    for m, t in (("package.json", "node"), ("pyproject.toml", "python"),
                 ("requirements.txt", "python"), ("Cargo.toml", "rust"), ("go.mod", "go")):
        if m in p["marcatori"] and t not in tags:
            tags.append(t)
    return Node(
        slug=slugify("progetto " + p["nome"]),
        title=p["nome"],
        body="\n".join(corpo),
        tipo="progetto",
        tags=tags[:4],
        relazioni=["profilo-utente"],
        riferimenti=[p["path"]] + ([p["remote"]] if p["remote"] else []),
        origine=ORIGINE_SCANSIONE,
        confidenza=0.9,
    )


# -------------------------------------------------------- 3. app e ambiente
def nodo_ambiente(cfg_modello: str = "", cfg_runtime: str = "") -> Node:
    gpu = _ps("(Get-CimInstance Win32_VideoController | Select-Object -First 1).Name", 30)
    cpu = _ps("(Get-CimInstance Win32_Processor | Select-Object -First 1).Name", 30)
    ram = _ps("[math]::Round((Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory/1GB,1)", 30)
    corpo = [
        f"- **CPU**: {cpu or 'n/d'}",
        f"- **GPU**: {gpu or 'n/d'}",
        f"- **RAM**: {ram or '?'} GB",
        f"- **Python**: {platform.python_version()}",
    ]
    if cfg_modello:
        corpo.append(f"- **Modello locale**: `{cfg_modello}`")
    if cfg_runtime:
        corpo.append(f"- **Runtime llama.cpp**: `{cfg_runtime}`")
    return Node(
        slug="ambiente-tecnico",
        title="Ambiente tecnico del PC",
        body="\n".join(corpo),
        tipo="app",
        tags=["hardware", "ambiente"],
        relazioni=["profilo-utente"],
        origine=ORIGINE_SCANSIONE,
        confidenza=0.95,
    )


def nodi_app(limite: int = 25) -> list[Node]:
    """Solo le app che contano per l'automazione, non tutti i redistributable."""
    grezzo = _ps(
        "$k='HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*',"
        "'HKLM:\\SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*',"
        "'HKCU:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*';"
        "Get-ItemProperty $k -ErrorAction SilentlyContinue | Where-Object {$_.DisplayName} | "
        "Select-Object -Expand DisplayName | Sort-Object -Unique", 90)
    rumore = ("redistributable", "runtime", "update for", "driver", "sdk",
              "microsoft visual c++", "hotfix", "language pack")
    nomi = [n.strip() for n in grezzo.splitlines()
            if n.strip() and not any(r in n.lower() for r in rumore)]
    if not nomi:
        return []
    corpo = ["Applicazioni installate rilevanti (rilevate dal registro):", ""]
    corpo += [f"- {n}" for n in nomi[:120]]
    return [Node(
        slug="app-installate",
        title="Applicazioni installate",
        body="\n".join(corpo),
        tipo="app",
        tags=["app", "software"],
        relazioni=["ambiente-tecnico"],
        origine=ORIGINE_SCANSIONE,
        confidenza=0.85,
    )]


# ------------------------------------------------------------- 4. persone
def nodi_persone(progetti: list[dict]) -> list[Node]:
    """Deduce le persone dai co-autori git dei progetti trovati."""
    conteggio: dict[str, dict] = {}
    io = _ps("git config --global user.email").strip().lower()
    for p in progetti:
        if not p.get("git"):
            continue
        out = _ps(f'git -C "{p["path"]}" log --pretty=format:"%an|%ae" -n 200', 30)
        for riga in out.splitlines():
            if "|" not in riga:
                continue
            nome, _, mail = riga.partition("|")
            nome, mail = nome.strip(), mail.strip().lower()
            if not nome or mail == io or "noreply" in mail:
                continue
            voce = conteggio.setdefault(mail, {"nome": nome, "commit": 0, "progetti": set()})
            voce["commit"] += 1
            voce["progetti"].add(p["nome"])
    nodi: list[Node] = []
    for mail, v in sorted(conteggio.items(), key=lambda kv: -kv[1]["commit"])[:15]:
        progetti_str = ", ".join(f"[[{slugify('progetto ' + n)}|{n}]]" for n in sorted(v["progetti"]))
        nodi.append(Node(
            slug=slugify("persona " + v["nome"]),
            title=v["nome"],
            body=(f"- **Email**: {mail}\n"
                  f"- **Commit visti**: {v['commit']}\n"
                  f"- **Progetti in comune**: {progetti_str or 'n/d'}"),
            tipo="persona",
            tags=["persona", "collaboratore"],
            relazioni=[slugify("progetto " + n) for n in sorted(v["progetti"])][:5],
            riferimenti=[mail],
            origine=ORIGINE_SCANSIONE,
            confidenza=0.7,
        ))
    if not nodi:
        nodi.append(Node(
            slug="persone",
            title="Persone",
            body=("Nessun collaboratore dedotto dai repository. Questo nodo si "
                  "riempira' man mano che nomini persone nelle conversazioni."),
            tipo="persona",
            tags=["persone"],
            relazioni=["profilo-utente"],
            origine=ORIGINE_SCANSIONE,
            confidenza=0.5,
        ))
    return nodi


# ------------------------------------------------------------------ regia
def esegui_seed(vault: Vault, cfg_modello: str = "", cfg_runtime: str = "",
                log=print) -> dict:
    """Popola la KB da zero. Idempotente: rilanciarlo aggiorna senza duplicare."""
    creati = 0
    log("Profilo e preferenze...")
    for n in (nodo_profilo(), nodo_preferenze()):
        vault.upsert(n)
        creati += 1

    log("Ambiente e applicazioni...")
    vault.upsert(nodo_ambiente(cfg_modello, cfg_runtime))
    creati += 1
    for n in nodi_app():
        vault.upsert(n)
        creati += 1

    log("Progetti e cartelle di lavoro...")
    progetti = trova_progetti()
    for p in progetti:
        vault.upsert(nodo_progetto(p))
        creati += 1

    log("Persone...")
    for n in nodi_persone(progetti):
        vault.upsert(n)
        creati += 1

    vault.scrivi_indice()
    _scrivi_marcatore(vault)
    stat = vault.statistiche()
    log(f"Seed completato: {creati} nodi scritti, {stat['collegamenti']} collegamenti.")
    return stat


def _scrivi_marcatore(vault: Vault) -> None:
    p = vault.root / ".nova" / "seed.json"
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(json.dumps({"eseguito": date.today().isoformat()}), encoding="utf-8")


def seed_gia_fatto(vault: Vault) -> bool:
    return (vault.root / ".nova" / "seed.json").exists()
