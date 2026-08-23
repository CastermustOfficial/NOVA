"""Il vault: cartella di file .md, indice in memoria, relazioni bidirezionali.

Fonte di verita' unica = i file sul disco. Nessun database: puoi aprire la
cartella in Obsidian, modificare a mano, e NOVA ricarica.
"""
from __future__ import annotations

import json
import os
import re
import threading
from datetime import datetime
from pathlib import Path

from .riservatezza import perche_non_si_salva
from .schema import (ORIGINE_AUTO, ORIGINE_SCANSIONE, ORIGINE_UTENTE,
                     STATUS_ARCHIVIATO, STATUS_ATTIVO, Node, slugify)

# Un nodo cresce per accodamento a ogni upsert: senza un tetto, i nodi
# frequentati diventano muri di testo che poi il retrieval tronca comunque.
MAX_CORPO = 4000
# Quanto puo' diventare grande audit.jsonl prima di essere ruotato.
MAX_AUDIT_BYTE = 2 * 1024 * 1024

# Chi ha visto la cosa piu' da vicino: l'utente batte una scansione del PC,
# che batte una deduzione fatta osservando una conversazione.
PESO_ORIGINE = {ORIGINE_UTENTE: 3, ORIGINE_SCANSIONE: 2, ORIGINE_AUTO: 1}
# Tipi che possono assorbire o essere assorbiti da chiunque: sono il
# contenitore generico in cui finisce quello che NOVA impara da sola.
TIPI_GENERICI = {"fatto", "nota", ""}

SOTTOCARTELLE = {
    "profilo": "01-profilo",
    "preferenza": "01-profilo",
    "persona": "02-persone",
    "progetto": "03-progetti",
    "app": "04-ambiente",
    "luogo": "04-ambiente",
    "abitudine": "05-abitudini",
    "fatto": "06-fatti",
    "nota": "06-fatti",
    "hub": "",
}


class Vault:
    """Carica, cerca, scrive e collega i nodi della KB."""

    def __init__(self, root: str | Path):
        self.root = Path(root)
        self.root.mkdir(parents=True, exist_ok=True)
        try:
            self._radice = self.root.resolve()
        except OSError:
            self._radice = self.root
        self.audit_path = self.root / ".nova" / "audit.jsonl"
        self.audit_path.parent.mkdir(parents=True, exist_ok=True)
        self._nodes: dict[str, Node] = {}
        # Indicizzati per percorso, non per slug: due file con lo stesso nome
        # base in cartelle diverse hanno mtime diversi, e una chiave sola per
        # entrambi li faceva ricaricare (e ri-embeddare) a ogni ricerca.
        self._mtimes: dict[str, tuple[float, int]] = {}
        self._proprietario: dict[str, str] = {}   # percorso -> slug caricato
        self.collisioni: dict[str, list[str]] = {}  # slug -> file che se lo contendono
        self._lock = threading.RLock()
        self.reload()

    # -- caricamento ---------------------------------------------------
    def _chiave(self, f: Path) -> str:
        """Identita' di un file dentro il vault, stabile fra sistemi.

        La radice si risolve una volta in __init__: qui girava due resolve()
        per file a ogni ricerca. E si usa normcase, non lower: su un
        filesystem che distingue le maiuscole «A.md» e «a.md» sono due file.
        """
        try:
            rel = Path(f).resolve().relative_to(self._radice)
        except (ValueError, OSError):
            rel = Path(f)
        return os.path.normcase(str(rel).replace("\\", "/"))

    @staticmethod
    def _impronta(f: Path) -> tuple[float, int]:
        """mtime *e* dimensione: su NTFS il timestamp avanza a scatti di ~15 ms,
        e due scritture nello stesso scatto sarebbero indistinguibili."""
        try:
            st = f.stat()
            return (st.st_mtime, st.st_size)
        except OSError:
            return (0.0, -1)

    def _file_md(self):
        """I .md del vault, in ordine stabile. Salta _INDICE e le cartelle punto."""
        for f in sorted(self.root.rglob("*.md")):
            try:
                rel = f.relative_to(self.root)
            except ValueError:
                continue
            if f.stem.startswith("_") or any(p.startswith(".") for p in rel.parts):
                continue
            yield f

    def reload(self) -> int:
        with self._lock:
            self._nodes.clear()
            self._mtimes.clear()
            self._proprietario.clear()
            self.collisioni.clear()
            for f in self._file_md():
                self._carica_file(f)
            return len(self._nodes)

    def refresh_if_changed(self) -> None:
        """Ricarica solo i file toccati fuori da NOVA (es. modificati in Obsidian)."""
        with self._lock:
            visti: set[str] = set()
            for f in self._file_md():
                chiave = self._chiave(f)
                visti.add(chiave)
                if self._mtimes.get(chiave) != self._impronta(f):
                    self._carica_file(f)
            da_riaprire: set[str] = set()
            for chiave in list(self._mtimes):
                if chiave in visti:
                    continue
                self._mtimes.pop(chiave, None)
                slug = self._proprietario.pop(chiave, "")
                # il nodo se ne va solo se nessun altro file lo rivendica
                if slug and not any(s == slug for s in self._proprietario.values()):
                    self._nodes.pop(slug, None)
                conteso = self._slug_conteso(chiave)
                for x in (slug, conteso):
                    if x and x in self.collisioni:
                        da_riaprire.add(x)
            # Se sparisce uno dei due file in lite, la contesa va riaperta:
            # altrimenti chi aveva perso resta invisibile pur essendo rimasto
            # l'unico, ed e' di nuovo un nodo che sparisce in silenzio.
            for slug in da_riaprire:
                for percorso in self.collisioni.pop(slug, []):
                    f = Path(percorso)
                    if not f.exists():
                        continue
                    self._mtimes.pop(self._chiave(f), None)
                    self._carica_file(f)

    def _slug_conteso(self, chiave: str) -> str:
        """Lo slug la cui collisione coinvolge questo file, se c'e'."""
        for slug, percorsi in self.collisioni.items():
            for percorso in percorsi:
                if self._chiave(Path(percorso)) == chiave:
                    return slug
        return ""

    def _carica_file(self, f: Path) -> None:
        try:
            testo = f.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            return
        chiave = self._chiave(f)
        # f.stem resta il ripiego per il titolo, ma lo slug passa da slugify:
        # get() e le relazioni slugificano sempre, e un file scritto a mano
        # («Progetto Nova.md») restava altrimenti irraggiungibile.
        node = Node.from_markdown(testo, f.stem, f)
        node.slug = slugify(node.slug or f.stem)
        impronta = self._impronta(f)
        occupante = self._nodes.get(node.slug)
        if (occupante is not None and occupante.path is not None
                and self._chiave(occupante.path) != chiave):
            # Due file diversi rivendicano lo stesso slug. Vince chi e' arrivato
            # prima (l'ordine e' stabile), ma la cosa va detta invece di far
            # sparire un nodo in silenzio.
            self.collisioni[node.slug] = sorted(
                {str(occupante.path), str(f)})
            self._mtimes[chiave] = impronta   # niente ricarica a vuoto ogni giro
            self._proprietario[chiave] = ""
            return
        self._nodes[node.slug] = node
        self._mtimes[chiave] = impronta
        self._proprietario[chiave] = node.slug

    def _rileggi_se_cambiato(self, node: Node | None) -> Node | None:
        """Rilegge un nodo dal disco se qualcuno l'ha toccato fuori da NOVA.

        Senza questo, un nodo modificato a mano in Obsidian veniva sovrascritto
        dalla copia stantia in memoria al primo upsert: la modifica dell'utente
        spariva senza lasciare traccia nemmeno nell'audit.
        """
        if node is None or node.path is None:
            return node
        if not node.path.exists():
            return node
        chiave = self._chiave(node.path)
        if self._mtimes.get(chiave) == self._impronta(node.path):
            return node
        self._carica_file(node.path)
        return self._nodes.get(node.slug) or node

    # -- lettura -------------------------------------------------------
    # Tutte sotto lock: MemoryWriter scrive da un thread di sfondo dopo quasi
    # ogni scambio, e iterare su _nodes mentre qualcuno lo modifica sollevava
    # «dictionary changed size during iteration» in mezzo a una ricerca.
    def __len__(self) -> int:
        with self._lock:
            return len(self._nodes)

    def all(self, includi_archiviati: bool = False) -> list[Node]:
        with self._lock:
            return [n for n in self._nodes.values()
                    if includi_archiviati or n.status != STATUS_ARCHIVIATO]

    def get(self, slug: str) -> Node | None:
        with self._lock:
            return self._nodes.get(slugify(slug))

    def per_titolo(self, titolo: str) -> Node | None:
        t = titolo.strip().lower()
        with self._lock:
            for n in self._nodes.values():
                if n.title.strip().lower() == t:
                    return n
            return self._nodes.get(slugify(titolo))

    def vicini(self, slug: str) -> list[Node]:
        """Nodi collegati in entrambe le direzioni (il grafo e' non orientato)."""
        slug = slugify(slug)
        out: dict[str, Node] = {}
        with self._lock:
            node = self._nodes.get(slug)
            if node:
                for r in node.tutte_le_relazioni():
                    v = self._nodes.get(r)
                    if v and v.status != STATUS_ARCHIVIATO:
                        out[v.slug] = v
            for altro in self._nodes.values():
                if altro.status == STATUS_ARCHIVIATO or altro.slug == slug:
                    continue
                if slug in altro.tutte_le_relazioni():
                    out[altro.slug] = altro
        return list(out.values())

    # -- scrittura -----------------------------------------------------
    def percorso_per(self, node: Node) -> Path:
        sotto = SOTTOCARTELLE.get(node.tipo, "06-fatti")
        cartella = self.root / sotto if sotto else self.root
        cartella.mkdir(parents=True, exist_ok=True)
        return cartella / f"{node.slug}.md"

    def upsert(self, node: Node, unisci: bool = True) -> Node:
        """Crea o aggiorna un nodo. Se esiste e `unisci`, fonde corpo e metadati.

        E' l'unica porta da cui si scrive nel vault: ci passano
        l'apprendimento automatico, `kb_note` e il seeding. Per questo il
        controllo sui segreti sta qui e non nel giudizio di chi chiama —
        chiudere una porta sola vuol dire chiuderla davvero.
        """
        motivo = perche_non_si_salva(f"{node.title}\n{node.body}")
        if motivo is not None:
            # Il rifiuto non riporta il valore: un messaggio d'errore finisce
            # nei log, e un log che ripete la credenziale appena rifiutata non
            # ha protetto niente.
            raise ValueError(
                f"non salvo questo nodo: contiene {motivo}. Quello che entra nel "
                f"vault viene riletto in ogni conversazione futura, comprese quelle "
                f"in cui leggo testo scritto da altri — una credenziale li' dentro "
                f"e' esposta per sempre. Se serve usarla, chiedila al momento."
            )
        with self._lock:
            node.slug = slugify(node.slug or node.title)
            esistente = self._nodes.get(node.slug)
            if esistente is None:
                esistente = self._stesso_nodo(node)
            # chi c'era gia' va riletto dal disco: potrebbe averlo appena
            # modificato l'utente in Obsidian
            esistente = self._rileggi_se_cambiato(esistente)
            if esistente is None:
                # Il file puo' esistere su disco senza essere ancora
                # nell'indice: MemoryWriter scrive da un thread di sfondo e
                # seed.py fa una raffica di upsert all'avvio, entrambi senza
                # passare da refresh_if_changed. Scriverci sopra in blocco
                # cancellerebbe quello che c'e' gia', senza nemmeno un merge.
                atteso = self.percorso_per(node)
                if atteso.exists():
                    self._carica_file(atteso)
                    esistente = self._nodes.get(node.slug)
            # Il controllo sui tipi stava solo dentro _stesso_nodo, cioe' sul
            # ramo fuzzy. Ma l'estrattore genera slug = slugify(titolo): la
            # persona «Marco» e il progetto «Marco» hanno entrambi slug
            # «marco», colpivano il ramo diretto e si fondevano lo stesso.
            if esistente is not None and not _tipi_compatibili(node.tipo, esistente.tipo):
                esistente = None
                node.slug = self._slug_libero(node)
            slug_richiesto = node.slug
            if esistente is not None:
                node.slug = esistente.slug
                if slug_richiesto != node.slug:
                    self._rinomina_relazioni(slug_richiesto, node.slug)
            if esistente and unisci:
                node = _fondi(esistente, node)
            if esistente and esistente.path and esistente.path.exists():
                percorso = esistente.path
            else:
                percorso = self.percorso_per(node)
            percorso.parent.mkdir(parents=True, exist_ok=True)
            percorso.write_text(node.to_markdown(), encoding="utf-8")
            node.path = percorso
            self._nodes[node.slug] = node
            chiave = self._chiave(percorso)
            self._mtimes[chiave] = self._impronta(percorso)
            self._proprietario[chiave] = node.slug
            self._collega_reciproco(node)
            self.audit("upsert", node.slug, {"tipo": node.tipo, "origine": node.origine})
            return node

    def _stesso_nodo(self, node: Node) -> Node | None:
        """Riconosce un nodo gia' presente sotto un altro slug.

        Il modello scrive 'knowledge-lab', la scansione aveva scritto
        'progetto-knowledge-lab': senza questo controllo la KB si sdoppia.
        """
        titolo = node.title.strip().lower()
        nudo = _senza_prefisso(node.slug)
        for altro in self._nodes.values():
            if altro.slug == node.slug:
                continue
            # Senza questo controllo la persona «Marco» e il progetto «Marco»
            # finivano nello stesso file, con un tipo solo e nella cartella
            # sbagliata. Un «fatto» invece puo' confluire in qualunque cosa:
            # e' il contenitore generico di quello che NOVA impara da sola.
            if not _tipi_compatibili(node.tipo, altro.tipo):
                continue
            if altro.title.strip().lower() == titolo:
                return altro
            if nudo and _senza_prefisso(altro.slug) == nudo:
                return altro
        return None

    def _slug_libero(self, node: Node) -> str:
        """Uno slug che non calpesti un nodo di tipo incompatibile."""
        base = slugify(f"{node.tipo}-{node.slug}") if node.tipo else node.slug
        candidato = base
        n = 2
        while candidato in self._nodes and not _tipi_compatibili(
                node.tipo, self._nodes[candidato].tipo):
            candidato = f"{base}-{n}"
            n += 1
        return candidato

    def _rinomina_relazioni(self, vecchio: str, nuovo: str) -> None:
        """Dopo una fusione, gli archi che puntavano al vecchio slug vanno spostati.

        Prima restavano appesi: `vicini()` li scartava senza dire niente e il
        grafo perdeva un arco a ogni deduplicazione, mentre `statistiche()`
        continuava a contarli.
        """
        if not vecchio or vecchio == nuovo:
            return
        for candidato in list(self._nodes.values()):
            if vecchio not in candidato.tutte_le_relazioni():
                continue
            # anche qui la copia in memoria puo' essere stantia: riscriverla
            # cancellerebbe quello che l'utente ha appena messo in Obsidian
            altro = self._rileggi_se_cambiato(candidato) or candidato
            if vecchio not in altro.tutte_le_relazioni():
                continue
            altro.relazioni = list(dict.fromkeys(
                nuovo if r == vecchio else r for r in altro.relazioni
                if r != altro.slug))
            # I wikilink nel corpo sono archi a tutti gli effetti: li seguono
            # _collega_reciproco, vicini() e statistiche(). Rinominare solo il
            # frontmatter lasciava appeso «Vedi [[vecchio-slug]]» per sempre.
            altro.body = _rinomina_wikilink(altro.body, vecchio, nuovo)
            if altro.path and altro.path.exists():
                altro.path.write_text(altro.to_markdown(), encoding="utf-8")
                self._segna_scritto(altro)

    def _collega_reciproco(self, node: Node) -> None:
        """Se A dice di essere collegato a B, B deve saperlo: grafo navigabile."""
        for slug in node.tutte_le_relazioni():
            # anche qui: riscrivere un terzo nodo da una copia stantia
            # cancellerebbe le modifiche fatte a mano su un file che nessuno
            # aveva chiesto di toccare
            altro = self._rileggi_se_cambiato(self._nodes.get(slug))
            if not altro or node.slug in altro.tutte_le_relazioni():
                continue
            altro.relazioni.append(node.slug)
            if altro.path and altro.path.exists():
                altro.path.write_text(altro.to_markdown(), encoding="utf-8")
                chiave = self._chiave(altro.path)
                self._mtimes[chiave] = self._impronta(altro.path)
                self._proprietario[chiave] = altro.slug

    def archivia(self, slug: str, motivo: str = "") -> bool:
        with self._lock:
            node = self._rileggi_se_cambiato(self._nodes.get(slugify(slug)))
            if not node:
                return False
            gia_archiviato = node.status == STATUS_ARCHIVIATO
            node.status = STATUS_ARCHIVIATO
            # archiviare due volte non deve accodare due volte la stessa riga
            if motivo and not gia_archiviato:
                node.body += f"\n\n> Archiviato il {datetime.now():%d/%m/%Y}: {motivo}"
            if node.path:
                node.path.write_text(node.to_markdown(), encoding="utf-8")
                self._segna_scritto(node)
            self.audit("archivia", node.slug, {"motivo": motivo})
            return True

    def riattiva(self, slug: str) -> bool:
        with self._lock:
            node = self._rileggi_se_cambiato(self._nodes.get(slugify(slug)))
            if not node:
                return False
            node.status = STATUS_ATTIVO
            if node.path:
                node.path.write_text(node.to_markdown(), encoding="utf-8")
                self._segna_scritto(node)
            self.audit("riattiva", node.slug, {})
            return True

    def _segna_scritto(self, node: Node) -> None:
        """Aggiorna l'mtime dopo una scrittura nostra: non e' una modifica esterna."""
        if node.path is None:
            return
        chiave = self._chiave(node.path)
        self._mtimes[chiave] = self._impronta(node.path)
        self._proprietario[chiave] = node.slug

    # -- audit ---------------------------------------------------------
    def audit(self, azione: str, slug: str, dettaglio: dict | None = None) -> None:
        riga = {
            "quando": datetime.now().isoformat(timespec="seconds"),
            "azione": azione,
            "nodo": slug,
            "dettaglio": dettaglio or {},
        }
        try:
            self._ruota_audit()
            with open(self.audit_path, "a", encoding="utf-8") as f:
                f.write(json.dumps(riga, ensure_ascii=False) + "\n")
        except OSError:
            pass

    def _ruota_audit(self) -> None:
        """Un file solo di storico, poi si ricomincia.

        Ogni upsert e ogni ricerca scrivono una riga: senza rotazione il file
        cresceva per sempre, e nessuno lo potava da nessuna parte.
        """
        try:
            if self.audit_path.stat().st_size < MAX_AUDIT_BYTE:
                return
        except OSError:
            return
        precedente = self.audit_path.with_suffix(".1.jsonl")
        try:
            precedente.unlink(missing_ok=True)
            self.audit_path.replace(precedente)
        except OSError:
            pass

    # -- manutenzione --------------------------------------------------
    def statistiche(self) -> dict:
        per_tipo: dict[str, int] = {}
        per_origine: dict[str, int] = {}
        for n in self.all():
            per_tipo[n.tipo] = per_tipo.get(n.tipo, 0) + 1
            per_origine[n.origine] = per_origine.get(n.origine, 0) + 1
        archiviati = len(self.all(True)) - len(self.all())
        # solo gli archi che puntano a un nodo che esiste davvero: contare
        # anche quelli pendenti faceva sembrare il grafo piu' ricco di com'e'
        with self._lock:
            presenti = set(self._nodes)
        collegamenti = sum(
            len([r for r in n.tutte_le_relazioni() if r in presenti])
            for n in self.all())
        pendenti = sum(
            len([r for r in n.tutte_le_relazioni() if r not in presenti])
            for n in self.all())
        orfani = [n.slug for n in self.all() if not self.vicini(n.slug)]
        return {
            "nodi_attivi": len(self.all()),
            "archiviati": archiviati,
            "collegamenti": collegamenti,
            "collegamenti_pendenti": pendenti,
            "per_tipo": per_tipo,
            "per_origine": per_origine,
            "orfani": orfani[:20],
            # due file che rivendicano lo stesso slug: uno dei due e' invisibile
            "collisioni": {k: v for k, v in list(self.collisioni.items())[:20]},
            "vault": str(self.root),
        }

    def scrivi_indice(self) -> Path:
        """Hub di navigazione, come i _COMMUNITY_ di graphify ma per tipo."""
        righe = ["---", "title: Indice della conoscenza", "tipo: hub",
                 "tags: [indice]", "relazioni: []", "area: Generale",
                 "status: attivo", "origine: scansione", "confidenza: 1.0",
                 "riferimenti: []", f"creato: {datetime.now():%Y-%m-%d}",
                 f"aggiornato: {datetime.now():%Y-%m-%d}", "---", "",
                 "Mappa di tutto quello che NOVA sa. Generato automaticamente.", ""]
        per_tipo: dict[str, list[Node]] = {}
        for n in sorted(self.all(), key=lambda x: x.title.lower()):
            per_tipo.setdefault(n.tipo, []).append(n)
        for tipo in sorted(per_tipo):
            righe.append(f"## {tipo}")
            righe.append("")
            for n in per_tipo[tipo]:
                marchio = "" if n.origine == ORIGINE_UTENTE else f"  _{n.origine}_"
                righe.append(f"- [[{n.slug}|{n.title}]]{marchio}")
            righe.append("")
        p = self.root / "_INDICE.md"
        p.write_text("\n".join(righe), encoding="utf-8")
        return p


WIKILINK = re.compile(r"\[\[([^\]|]+)(\|[^\]]*)?\]\]")


def _rinomina_wikilink(corpo: str, vecchio: str, nuovo: str) -> str:
    def sostituisci(m: "re.Match") -> str:
        if slugify(m.group(1)) != vecchio:
            return m.group(0)
        return f"[[{nuovo}{m.group(2) or ''}]]"
    return WIKILINK.sub(sostituisci, corpo or "")


PREFISSI = ("progetto-", "persona-", "app-", "luogo-", "nodo-")


def _senza_prefisso(slug: str) -> str:
    for pre in PREFISSI:
        if slug.startswith(pre):
            return slug[len(pre):]
    return slug


def _tipi_compatibili(a: str, b: str) -> bool:
    return a == b or a in TIPI_GENERICI or b in TIPI_GENERICI


def _tipo_piu_specifico(vecchio: str, nuovo: str) -> str:
    """Un «fatto» che confluisce in una persona non la trasforma in un fatto.

    L'estrattore mette sempre un tipo (default «fatto»), quindi «nuovo.tipo or
    vecchio.tipo» sceglieva sempre il nuovo: la prima annotazione generica su
    Anna declassava «persona-anna» a «fatto», e il file restava in 02-persone
    mentre l'indice diceva 06-fatti.
    """
    if nuovo and nuovo not in TIPI_GENERICI:
        return nuovo
    if vecchio and vecchio not in TIPI_GENERICI:
        return vecchio
    return nuovo or vecchio


def _limita_corpo(corpo: str, massimo: int = MAX_CORPO) -> str:
    """Tiene la definizione originale e le annotazioni piu' recenti.

    Le vecchie si perdono, ma si perdevano comunque: il retrieval tronca a
    950 caratteri, quindi oltre un certo punto stavano solo occupando disco e
    rallentando ogni ricerca.

    Anche il primo blocco ha una quota. Senza, un nodo il cui primo paragrafo
    da solo superava il tetto restava congelato per sempre: c'era spazio per
    la testa e per nient'altro, quindi ogni fatto nuovo veniva scartato in
    silenzio a ogni upsert successivo. Il fatto piu' recente entra sempre,
    tagliato se serve.
    """
    corpo = corpo.strip()
    if len(corpo) <= massimo:
        return corpo
    blocchi = [b.strip() for b in corpo.split("\n\n") if b.strip()]
    if not blocchi:
        return corpo[:massimo]
    quota_testa = max(120, massimo // 3)
    testa = blocchi[0]
    if len(testa) > quota_testa:
        testa = testa[:quota_testa].rstrip() + " [...]"
    if len(blocchi) == 1:
        return testa
    avviso = "> [{} annotazioni piu' vecchie rimosse]"
    disponibile = massimo - len(testa) - len(avviso.format(len(blocchi))) - 4
    if disponibile <= 0:
        return testa
    coda: list[str] = []
    usati = 0
    for i, b in enumerate(reversed(blocchi[1:])):
        spazio = disponibile - usati - 2
        if spazio <= 0:
            break
        if len(b) > spazio:
            if i:                       # non e' il piu' recente: si rinuncia
                break
            b = b[:max(0, spazio - 6)].rstrip() + " [...]"
        coda.append(b)
        usati += len(b) + 2
    coda.reverse()
    persi = len(blocchi) - 1 - len(coda)
    mezzo = [avviso.format(persi)] if persi else []
    return "\n\n".join([testa, *mezzo, *coda])


def _fondi(vecchio: Node, nuovo: Node) -> Node:
    """Aggiorna un nodo esistente senza perdere quello che c'era."""
    corpo_nuovo = (nuovo.body or "").strip()
    corpo_vecchio = (vecchio.body or "").strip()
    ripetuto = bool(corpo_nuovo) and corpo_nuovo in corpo_vecchio
    if corpo_nuovo and not ripetuto:
        corpo = f"{corpo_vecchio}\n\n{corpo_nuovo}".strip() if corpo_vecchio else corpo_nuovo
    else:
        corpo = corpo_vecchio
    corpo = _limita_corpo(corpo)
    # Confermata, non riformulata: alzare la confidenza ogni volta che il
    # testo e' *diverso* premiava la variazione lessicale, cioe' esattamente
    # il caso in cui NOVA non ha imparato niente di nuovo. Ora sale quando lo
    # stesso fatto torna identico da una seconda osservazione.
    confidenza = max(vecchio.confidenza, nuovo.confidenza)
    if ripetuto:
        confidenza = min(1.0, confidenza + 0.05)
    origine = max((vecchio.origine, nuovo.origine),
                  key=lambda o: PESO_ORIGINE.get(o, 0))
    return Node(
        slug=vecchio.slug,
        title=nuovo.title or vecchio.title,
        body=corpo,
        tipo=_tipo_piu_specifico(vecchio.tipo, nuovo.tipo),
        tags=list(dict.fromkeys([*vecchio.tags, *nuovo.tags])),
        relazioni=list(dict.fromkeys([*vecchio.relazioni, *nuovo.relazioni])),
        area=nuovo.area or vecchio.area,
        status=nuovo.status or vecchio.status,
        origine=origine,
        confidenza=confidenza,
        riferimenti=list(dict.fromkeys([*vecchio.riferimenti, *nuovo.riferimenti])),
        creato=vecchio.creato,
        path=vecchio.path,
    )
