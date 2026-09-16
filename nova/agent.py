"""Il cervello di NOVA: dialogo con il modello e ciclo di esecuzione dei tool.

Il modello vero e proprio sta dietro l'astrazione `brains`: puo' essere il
GGUF locale, Claude Code CLI o un'API esterna, e si cambia a caldo senza
perdere la conversazione.
"""
from __future__ import annotations

import getpass
import json
import re
import threading
import time
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Callable

from .brains import crea_brain
from .config import AUTONOMY_ASK_ALL, AUTONOMY_FULL, Config
from .percorsi import dentro_comunque
from .tools import REGISTRY, Risk, ToolError, openai_schema, run_tool


from .brains.base import LimiteUso


#: I tre segnaposto che il prompt di sistema conosce.
SEGNAPOSTO = ("{user}", "{now}", "{home}")


def sostituisci_segnaposto(modello: str, utente: str, adesso: str, casa: str) -> str:
    """I tre segnaposto, e nient'altro.

    Prima si usava `str.format`, e `str.format` non guarda i tre segnaposto:
    guarda **tutte** le graffe. Un prompt di sistema personalizzato che
    contenga un esempio JSON — `{"a": 1}` — o una graffa vuota faceva saltare
    la composizione con un `KeyError` grezzo, e la composizione avviene dentro
    `Agent.__init__`: NOVA non partiva, e quello che si leggeva era
    `KeyError: '"a"'`. Misurato con quattro prompt, non immaginato.

    Chi scrive un prompt di sistema ci mette esempi, e gli esempi hanno le
    graffe. Quindi non si formatta: si sostituisce, e tutto il resto resta
    scritto com'e'.
    """
    return (modello
            .replace("{user}", utente)
            .replace("{now}", adesso)
            .replace("{home}", casa))


def componi_domanda(testo: str, memoria: str = "", procedure: str = "",
                    identita: str = "", postilla: str = "") -> str:
    """Il messaggio dell'utente con attaccato tutto il resto.

    **L'ordine conta**, ed e' l'unica cosa che questa funzione decide: prima
    cio' che hai chiesto, poi cio' che NOVA sa, poi cio' che ha gia' fatto,
    poi come deve rispondere. L'istruzione resta l'ultima cosa letta, che e'
    il posto in cui i modelli la seguono di piu'.
    """
    return testo + memoria + procedure + identita + postilla


def componi_prompt(modello: str, utente: str, adesso: str, casa: str,
                   lingua: str = "it") -> str:
    """Il messaggio di sistema completo.

    Le regole operative si aggiungono **sempre**, anche a un prompt
    personalizzato: sono il minimo perche' NOVA sappia cosa puo' fare. Non si
    ripetono solo se il prompt le contiene davvero, e per saperlo si cerca una
    marca che vive dentro le regole stesse. Prima si cercava una frase del
    prompt predefinito, che nel frattempo si e' separata dalle regole: chi
    installava NOVA da zero si ritrovava senza quattordicimila caratteri di
    istruzioni, e non lo diceva nessuno.

    La lingua non si traduce: si **dice**. Tradurre il prompt vorrebbe dire
    mantenere undici copie di un testo che cambia a ogni funzione nuova, e
    vederle divergere.
    """
    from .config import INIZIO_REGOLE, REGOLE_OPERATIVE
    from .lingue import clausola
    base = sostituisci_segnaposto(modello, utente, adesso, casa)
    if INIZIO_REGOLE not in base:
        base += REGOLE_OPERATIVE
    return base + clausola(lingua)


def _traccia_turno(verso: str, cervello: str, secondi: float, esito: str) -> None:
    """Una riga quando un turno comincia, e una quando finisce.

    Il 10 settembre NOVA ha lavorato per due ore su una sola domanda - un
    modello da 27 miliardi di parametri a 3,3 token al secondo - e in quelle
    due ore non ha scritto **niente** da nessuna parte. Chi guardava vedeva
    un programma fermo, e non c'era modo di distinguerlo da un programma
    piantato: sono due cose diverse e vogliono due reazioni diverse.

    Il contenuto della domanda **non** entra qui, e non e' una dimenticanza:
    questo file sta accanto al programma e per D52 puo' finire sincronizzato
    col cloud. Per capire un blocco servono l'ora, il cervello e la durata;
    cosa e' stato chiesto non serve, e sarebbe l'unica cosa che non si puo'
    piu' togliere da li'.

    Silenziosa come tutta la diagnostica: un guasto nel diario non deve poter
    fermare la risposta.
    """
    try:
        import datetime
        import os as _os
        from .rotazione import accoda
        f = Path(__file__).resolve().parent.parent / "avvio.log"
        corpo = f"pid={_os.getpid()} TURNO {verso} cervello={cervello}"
        if verso == "fine":
            corpo += f" durata={secondi:.1f}s esito={esito}"
        accoda(f, f"{datetime.datetime.now():%d/%m %H:%M:%S} {corpo}", corpo)
    except Exception:
        pass


class Denied(Exception):
    """L'utente ha rifiutato l'azione."""


class Cancelled(Exception):
    """L'utente ha interrotto il turno."""


# ---------------------------------------------------------------- sicurezza
class SafetyContext:
    """Applica le regole di sicurezza prima che un tool tocchi il sistema."""

    def __init__(self, cfg: Config):
        self.cfg = cfg

    def guard_write(self, path: Path) -> None:
        r"""Se NOVA puo' scrivere qui.

        La domanda «sta dentro quella cartella?» si fa con
        `percorsi.dentro_comunque`, che guarda il nome **e** dove porta: sui
        soli nomi una giunzione aggira la protezione (misurato), e sulla sola
        destinazione NOVA non puo' scrivere in casa propria sotto un
        pacchetto MSIX (misurato anche quello). Questa guardia la sbagliava in tre
        modi insieme: confrontava un percorso risolto con dei protetti **non**
        risolti — due spazi diversi, e sotto un punto di reinnesto la
        protezione spariva in silenzio; attaccava una barra rovescia a mano,
        quindi fuori da Windows non scattava mai; e per le cartelle
        autorizzate confrontava senza separatore, cosi' autorizzare `C:\dati`
        autorizzava anche `C:\dati-altrui`.
        """
        for prot in self.cfg.safety.protected_paths:
            if dentro_comunque(path, prot):
                raise ToolError(
                    f"percorso protetto: {path}. Modificalo manualmente se necessario."
                )
        roots = self.cfg.safety.write_roots
        if roots and not any(dentro_comunque(path, r) for r in roots):
            raise ToolError(
                f"scrittura non consentita fuori dalle cartelle autorizzate: {roots}"
            )

    def guard_command(self, command: str) -> None:
        for pat in self.cfg.safety.forbidden_command_patterns:
            try:
                if re.search(pat, command, re.IGNORECASE):
                    raise ToolError(
                        f"comando bloccato dalle regole di sicurezza (pattern: {pat})"
                    )
            except re.error:
                continue

    def needs_approval(self, risk: Risk) -> bool:
        mode = self.cfg.safety.autonomy
        if mode == AUTONOMY_FULL:
            return False
        if mode == AUTONOMY_ASK_ALL:
            return True
        return risk >= Risk.DANGEROUS  # ask_risky


# ---------------------------------------------------------------- callbacks
@dataclass
class AgentCallbacks:
    """Ganci verso la UI. Ogni callback e' opzionale."""
    on_status: Callable[[str], None] = lambda s: None
    on_reasoning: Callable[[str], None] = lambda s: None
    on_assistant: Callable[[str], None] = lambda s: None
    on_tool_start: Callable[[str, dict, str], None] = lambda n, a, d: None
    on_tool_result: Callable[[str, str, bool], None] = lambda n, r, ok: None
    # deve restituire True/False; bloccante finche' l'utente decide
    ask_approval: Callable[[str, dict, str, Risk], bool] = lambda n, a, d, r: True
    on_brain: Callable[[str], None] = lambda s: None
    on_delega: Callable[[str, str, float], None] = lambda a, motivo, costo: None


# ---------------------------------------------------------------- agente
class Agent:
    def __init__(self, cfg: Config, callbacks: AgentCallbacks | None = None,
                 kb_engine=None, memory=None, vault=None, brain=None, router=None):
        self.strumenti_del_turno: set[str] = set()
        # La catena delle ripetizioni vale dentro un turno: una domanda nuova
        # ricomincia da capo.
        self._ultima_impronta = ""
        self._quante_ripetute = 0
        self._storia_giro: list[str] = []
        self._nomi_giro: list[str] = []
        self._giro_detto: tuple[str, int] | None = None
        self.cfg = cfg
        self.cb = callbacks or AgentCallbacks()
        self.safety = SafetyContext(cfg)
        self.kb = kb_engine
        self.vault = vault
        self.memory = memory
        self._ultima_impronta = ""
        self._quante_ripetute = 0
        self._storia_giro: list[str] = []
        self._nomi_giro: list[str] = []
        self._giro_detto: tuple[str, int] | None = None
        self.messages: list[dict] = []
        self.cancel_event = threading.Event()
        self._mem_idx: int | None = None
        self._system_base = ""
        self.brain = brain or crea_brain(cfg.brains.active, cfg, vault)
        self.router = router or self._crea_router()
        # Costruire un agente non e' iniziare una conversazione nuova.
        #
        # La distinzione sembra sottile e invece e' tutto: il guscio grafico
        # avvia un processo per messaggio, quindi «costruzione» capita a ogni
        # frase. Azzerare qui il filo del discorso significava che NOVA
        # rispondeva «non ho contesto su cosa intendi» a una domanda che
        # seguiva la sua stessa risposta di trenta secondi prima.
        self.reset(nuova_conversazione=False)

    def _crea_router(self):
        """Il router che sa quali modelli esistono e quanto si e' speso."""
        if not (self.cfg.brains.routing or {}).get("abilitato", True):
            return None
        from .routing import Router
        from .tools import deleghe
        # Anche le righe del router passano dal battito: sono cose che
        # succedono durante l'attesa, e devono restare vive come le altre.
        r = Router(self.cfg, self.vault, log=lambda m: self._stato(m))
        deleghe.collega(r)
        return r

    # -- cervello ------------------------------------------------------
    @property
    def model_name(self) -> str:
        return self.brain.descrizione_stato()

    def detect_model(self) -> str:
        rileva = getattr(self.brain, "rileva_modello", None)
        if callable(rileva):
            rileva()
        return self.brain.descrizione_stato()

    def cambia_brain(self, nome: str) -> str:
        """Sostituisce il cervello senza perdere la conversazione."""
        self.brain = crea_brain(nome, self.cfg, self.vault)
        self.cfg.brains.active = nome
        pronto, motivo = self.brain.disponibile()
        self.detect_model()
        stato = self.brain.descrizione_stato() if pronto else f"NON disponibile - {motivo}"
        self.cb.on_brain(stato)
        return stato

    def llm_semplice(self, prompt: str, max_tokens: int = 600) -> str:
        """Chiamata secca senza tool: la usa il modulo di memoria."""
        return self.brain.semplice(prompt, max_tokens)

    # -- conversazione ------------------------------------------------
    def system_prompt(self) -> str:
        try:
            user = getpass.getuser()
        except Exception:
            user = "utente"
        return componi_prompt(
            self.cfg.system_prompt,
            utente=user,
            adesso=datetime.now().strftime("%A %d/%m/%Y %H:%M"),
            casa=str(Path.home()),
            lingua=getattr(self.cfg.ui, "lingua", "it"),
        )

    def reset(self, nuova_conversazione: bool = True) -> None:
        """Ripulisce la trascrizione in memoria.

        Con `nuova_conversazione` taglia anche il filo che il cervello tiene
        per conto suo — la sessione di Claude Code, che sopravvive al
        processo. Senza, si prepara soltanto questa istanza.
        """
        self._system_base = self.system_prompt()
        self.messages = [{"role": "system", "content": self._system_base}]
        self._mem_idx = None
        if nuova_conversazione:
            self.brain.reset()

    #: Quanti caratteri vale un token, per stimare senza tokenizzatore.
    #:
    #: Misurato due volte su questa macchina, con prompt veri: 3,88 su una
    #: conversazione in italiano e 4,37 su del testo ripetitivo letto da un
    #: file. Si tiene il numero **piu' basso** dei due, anzi un filo sotto:
    #: sbagliare per eccesso di token vuol dire tagliare un po' presto, che si
    #: nota appena; sbagliare per difetto vuol dire sfondare il contesto, e
    #: quello e' un errore in faccia all'utente.
    CARATTERI_PER_TOKEN = 3.5

    #: Quanto lasciare libero per la risposta. Il contesto non serve solo a
    #: leggere: il modello ci scrive dentro.
    RISERVA_RISPOSTA_TOKEN = 1024

    #: Sopra questo numero di messaggi si taglia.
    TETTO_MESSAGGI = 60
    #: E si scende fino a questo. La distanza fra i due e' il punto: senza,
    #: si taglia a ogni turno.
    FONDO_MESSAGGI = 40

    def _spazio_per_la_conversazione(self, tools: list) -> int:
        """Quanti token restano alla conversazione, tolto tutto il resto.

        Il conto non e' un dettaglio contabile, e' la scoperta che ha fatto
        nascere questa funzione. Su questa macchina, con il contesto a 16.384:

            messaggio di sistema     ~5.200 token
            schemi dei sessanta tool ~6.900 token   (il 42% del contesto)
            riserva per la risposta   1.024 token
            ------------------------------------
            resta alla conversazione ~3.300 token   (il 20%)

        Il prefisso fisso si mangia i tre quarti del contesto, e quello che
        avanza e' molto meno di quanto sessanta messaggi possano pesare. Da
        qui il taglio a token.

        Se il cervello non e' quello locale il contesto non lo decide questa
        configurazione: si torna zero, cioe' «non lo so», e vale solo il
        taglio a messaggi.
        """
        if getattr(self.brain, "agentico", False):
            return 0
        try:
            ctx = int(getattr(self.cfg.server, "ctx_size", 0) or 0)
        except Exception:                                   # noqa: BLE001
            return 0
        if ctx <= 0:
            return 0
        sistema = str(self.messages[0].get("content") or "") if self.messages else ""
        fissi = self.stima_token(sistema) + self.RISERVA_RISPOSTA_TOKEN
        if tools:
            try:
                fissi += self.stima_token(json.dumps(tools, ensure_ascii=False))
            except Exception:                               # noqa: BLE001
                pass
        return max(0, ctx - fissi)

    @classmethod
    def stima_token(cls, testo: str) -> int:
        """Quanti token vale un testo, senza tokenizzatore.

        Serve una stima e non una misura: il tokenizzatore vero sta nel
        modello, cambia con il modello, e chiederglielo costerebbe un giro di
        rete per ogni messaggio a ogni turno solo per decidere se tagliare.
        """
        return int(len(testo or "") / cls.CARATTERI_PER_TOKEN) + 1

    def _token_dei(self, messaggi: list[dict]) -> int:
        return sum(self.stima_token(str(m.get("content") or "")) for m in messaggi)

    def trim_history(self, max_messages: int | None = None,
                     fondo: int | None = None,
                     token_disponibili: int = 0) -> None:
        """Accorcia la conversazione, ma di rado.

        Il taglio butta cio' che sta **subito dopo il messaggio di sistema**,
        e quello e' il posto peggiore: la cache del prefisso di llama.cpp vale
        finche' i token in testa sono gli stessi, quindi spostare la seconda
        riga invalida tutto il resto e si rielabora l'intera conversazione.

        Prima si tagliava fino a `tetto - 1`, cioe' si tornava esattamente sul
        filo. Il turno dopo aggiungeva due messaggi, si superava di nuovo, e si
        tagliava di nuovo: **dal trentesimo turno in poi si tagliava a ogni
        turno**, quindi la cache non si riformava mai piu' e ogni risposta
        pagava il prompt da capo. Non si rompeva niente, non lo diceva
        nessuno: la conversazione diventava lenta e restava lenta.

        Misurato con `banco_taglio.py` su Gemma 4 26B-A4B, conversazione da
        ottantuno messaggi, 15.379 token di prefisso:

            a caldo, prefisso intatto           175 ms
            dopo il taglio di prima           1.771 ms
            e il turno seguente               1.731 ms   <- non guarisce
            col fondo, dopo il taglio         1.217 ms
            e il turno seguente                 226 ms   <- guarito

        Scendere fino a un fondo non cambia **cosa** si butta: cambia quanto
        spesso. Si taglia una volta ogni dieci turni invece che a ogni turno, e
        nei nove in mezzo il prefisso resta valido. Il prezzo e' che quando si
        taglia si butta di piu' in un colpo solo, ed e' un prezzo che si paga
        volentieri: la memoria vera di NOVA non e' questa finestra, e' il
        vault.
        """
        tetto = self.TETTO_MESSAGGI if max_messages is None else max_messages
        giu = self.FONDO_MESSAGGI if fondo is None else fondo
        # Un fondo troppo vicino al tetto riporta al difetto di prima senza
        # dirlo, e «un turno di distanza» non basta: con `tetto - 2` si
        # taglierebbe a turni alterni invece che a ogni turno, che e' meta'
        # del difetto e non la sua assenza. La distanza minima e' un quarto
        # del tetto, cioe' una decina di turni di respiro.
        giu = max(2, min(giu, tetto * 3 // 4))
        if len(self.messages) <= tetto:
            # Sotto la soglia dei messaggi non si taglia per numero - ma il
            # taglio a token va fatto lo stesso, ed e' proprio questo il caso
            # che conta: dodici scambi con dentro il contenuto di un file sono
            # venticinque messaggi, quindi passano di qui, e sono centomila
            # token. La prima versione di questa funzione metteva il taglio a
            # token dopo questo `return`, cioe' non lo eseguiva mai nel solo
            # caso per cui era stato scritto.
            self._taglia_a_token(token_disponibili)
            return
        head = self.messages[:1]
        tail = self.messages[-(giu - 1):]
        # Una risposta di tool senza la chiamata che l'ha prodotta non e'
        # leggibile da nessun modello: si scarta finche' la coda non comincia
        # da qualcosa di sensato.
        while tail and tail[0].get("role") == "tool":
            tail.pop(0)
        self.messages = head + tail
        self._taglia_a_token(token_disponibili)

    def _taglia_a_token(self, disponibili: int) -> None:
        """E poi il taglio che conta davvero: quello sui token.

        La finestra si contava **in messaggi** e il limite del modello e' **in
        token**: due unita' diverse che non si parlavano. Sessanta messaggi
        possono essere trecento token o centomila, e bastano dodici scambi con
        dentro il contenuto di un file per arrivare a 102.953 token contro i
        16.384 del contesto - misurato, non immaginato. Il taglio a messaggi
        non scattava nemmeno: erano ventiquattro messaggi.

        Quello che arrivava all'utente era un JSON in inglese con dentro
        «exceeds the available context size».

        `disponibili` e' quanto resta al netto del prefisso fisso - il
        messaggio di sistema e gli schemi dei tool, che su questa macchina
        sono gia' i tre quarti del contesto - e della riserva per la risposta.
        Zero vuol dire «non lo so», e allora non si tocca niente: meglio il
        taglio a messaggi da solo che uno inventato.
        """
        # `< 2` e non `<= 2`: con esattamente due messaggi - sistema piu' una
        # risposta enorme - non c'e' niente da **togliere**, ma c'e' ancora da
        # **accorciare**, ed e' il caso che ha fatto scrivere l'accorciamento.
        # La prima versione usciva qui e lo lasciava passare intero.
        if disponibili <= 0 or len(self.messages) < 2:
            return
        head, coda = self.messages[:1], self.messages[1:]
        if self._token_dei(coda) <= disponibili:
            return
        # Stessa idea del fondo: si scende sotto la soglia, non ci si ferma
        # sopra, o si ritaglia a ogni turno e la cache non si riforma mai.
        obiettivo = int(disponibili * 0.75)
        while coda and self._token_dei(coda) > obiettivo:
            coda.pop(0)
        while coda and coda[0].get("role") == "tool":
            coda.pop(0)
        # Non si resta mai senza l'ultimo scambio: una conversazione vuota non
        # e' una conversazione accorciata, e' una amnesia.
        if not coda:
            coda = list(self.messages[-1:])
        # E se cio' che resta non ci sta **comunque**, vuol dire che un solo
        # messaggio e' piu' grande di tutto lo spazio: il contenuto di un file
        # letto, una pagina web intera. Buttarlo vorrebbe dire perdere proprio
        # la cosa di cui l'utente ha chiesto conto; tenerlo intero vuol dire
        # sfondare il contesto. Si accorcia, e lo si dice nel testo - un
        # taglio dichiarato il modello lo capisce, uno silenzioso gli fa
        # credere che il file finisca li'.
        if coda and self._token_dei(coda) > disponibili:
            coda = self._accorcia_il_piu_grosso(coda, obiettivo)
        self.messages = head + coda

    #: Sotto questa lunghezza un messaggio non si accorcia piu': quel che
    #: resta e' gia' solo l'inizio e la fine, e continuare vorrebbe dire
    #: toglierne il senso invece che il peso.
    MINIMO_ACCORCIABILE = 400

    def _accorcia_il_piu_grosso(self, coda: list[dict], obiettivo: int) -> list[dict]:
        """Accorcia i messaggi piu' grossi finche' la coda non ci sta.

        Si tiene l'inizio e la fine: l'inizio dice cos'era, la fine spesso
        porta la conclusione, ed e' il mezzo che si puo' perdere.

        **La lunghezza da togliere si calcola, non si indovina.** La prima
        versione tagliava a una misura fissa - millecinquecento caratteri in
        testa e altrettanti in coda - e non terminava: la scritta che dichiara
        il taglio e' lunga quanto i caratteri che alla seconda passata
        restavano da togliere, quindi il testo si accorciava di ottanta
        caratteri e ricresceva di ottanta, per sempre. Un ciclo che
        «ovviamente» finisce e non finisce. Adesso a ogni passata si punta
        alla lunghezza che serve, e si esce se non si e' guadagnato niente:
        due condizioni invece di una, perche' una si e' gia' vista sbagliare.
        """
        fuori = [dict(m) for m in coda]
        for _ in range(len(fuori) + 8):        # tetto: mai un ciclo aperto
            eccesso = self._token_dei(fuori) - obiettivo
            if eccesso <= 0:
                return fuori
            i = max(range(len(fuori)),
                    key=lambda k: len(str(fuori[k].get("content") or "")))
            testo = str(fuori[i].get("content") or "")
            if len(testo) <= self.MINIMO_ACCORCIABILE:
                break        # non c'e' piu' niente di grosso da accorciare
            # Quanto deve diventare lungo, piu' un margine per la scritta.
            da_togliere = int(eccesso * self.CARATTERI_PER_TOKEN) + 200
            voluta = max(self.MINIMO_ACCORCIABILE, len(testo) - da_togliere)
            meta = max(60, voluta // 2)
            tolti = len(testo) - meta * 2
            nuovo = (
                testo[:meta]
                + f"\n\n[...tagliati {tolti} caratteri perche' non ci stavano"
                  " nella memoria del modello...]\n\n"
                + testo[-meta:]
            )
            if len(nuovo) >= len(testo):
                break        # non si guadagna niente: si smette
            fuori[i]["content"] = nuovo
        return fuori

    # -- memoria nel prompt --------------------------------------------
    def _contesto_kb(self, user_text: str) -> str:
        if not self.kb or not self.cfg.kb.inject_context:
            return ""
        try:
            return self.kb.contesto_per(user_text, top_k=self.cfg.kb.top_k)
        except Exception:
            return ""

    def _blocco_memoria(self, user_text: str) -> str:
        """Cio' che la memoria ha trovato, da mettere in CODA alla domanda.

        Prima finiva nel messaggio di sistema, riscritto a ogni turno. Due
        difetti in uno.

        Funzionale: i cervelli agentici il prompt di sistema lo ricevono solo
        all'apertura della sessione, quindi dal secondo turno in poi il
        contesto veniva calcolato e buttato. NOVA faceva la ricerca sul grafo
        e non la leggeva.

        E di costo: il messaggio di sistema e' la prima regione di token su
        cui un fornitore tiene la cache. Cambiarlo a ogni turno — e cambiava,
        perche' il contesto dipende dalla domanda — invalida tutto il
        prefisso: ogni turno rielaborava l'intera conversazione da capo. In
        coda invece si aggiunge e basta, e il prefisso resta valido.
        """
        contesto = self._contesto_kb(user_text)
        if getattr(self.brain, "agentico", False):
            # Il cervello agentico se lo attacca da solo alla domanda: lui la
            # conversazione la tiene per conto suo, e noi gli passiamo un
            # messaggio per volta.
            self.brain.kb_context = contesto
            return ""
        if not contesto:
            return ""
        return (
            "\n\n<memoria>\n"
            "Quello che gia' sai, dalla tua memoria a grafo. Guardalo prima di "
            "misurare o cercare, e non ripeterlo all'utente come se fosse una "
            "novita'. Se scopri che qualcosa qui e' superato, correggilo con "
            "kb_note o kb_forget.\n\n"
            + contesto
            + "\n</memoria>"
        )

    # -- fallback per modelli che scrivono i tool call nel testo ------
    @staticmethod
    def _parse_inline_tool_calls(text: str) -> list[dict]:
        calls: list[dict] = []
        for m in re.finditer(r"<tool_call>\s*(\{.*?\})\s*</tool_call>", text, re.S):
            try:
                obj = json.loads(m.group(1))
                name = obj.get("name") or obj.get("tool")
                args = obj.get("arguments") or obj.get("parameters") or {}
                if name:
                    calls.append({
                        "id": f"inline_{len(calls)}",
                        "type": "function",
                        "function": {"name": name,
                                     "arguments": args if isinstance(args, str)
                                     else json.dumps(args, ensure_ascii=False)},
                    })
            except json.JSONDecodeError:
                continue
        return calls

    # -- ciclo principale ---------------------------------------------
    def send(self, user_text: str, postilla: str = "") -> str:
        """Un turno di conversazione.

        La `postilla` e' un'istruzione attaccata al messaggio per il cervello
        e basta: non entra nella ricerca in memoria e non viene imparata. Serve
        alla voce, che a ogni turno deve ricordare al cervello di rispondere
        come si parla — e che non puo' metterlo nel prompt di sistema, visto
        che quello si passa solo quando la sessione si apre.
        """
        self.cancel_event.clear()
        memoria = self._blocco_memoria(user_text)
        procedure = self._blocco_procedure(user_text)
        chi_sei = self._promemoria_identita()
        self.messages.append(
            {"role": "user",
             "content": componi_domanda(user_text, memoria, procedure,
                                        chi_sei, postilla)})
        _inizio_turno = time.time()
        # Quali strumenti ha usato *questo* turno: serve a decidere se cio' che
        # e' passato di qui puo' finire in memoria.
        self.strumenti_del_turno: set[str] = set()
        # La catena delle ripetizioni vale dentro un turno: una domanda nuova
        # ricomincia da capo.
        self._ultima_impronta = ""
        self._quante_ripetute = 0
        self._storia_giro: list[str] = []
        self._nomi_giro: list[str] = []
        self._giro_detto: tuple[str, int] | None = None
        agentico = getattr(self.brain, "agentico", False)
        tools = [] if agentico else openai_schema()
        self.trim_history(token_disponibili=self._spazio_per_la_conversazione(tools))
        # da dove si sale: cambia a ogni escalation, altrimenti la seconda
        # ridelegherebbe allo stesso gradino della prima
        gradino = (self.cfg.brains.routing or {}).get("orchestratore", "locale")

        from .attesa import Battito
        self._battito = Battito(self.cb.on_status)
        _traccia_turno("inizio", self.cfg.brains.active, 0.0, "")
        esito = "interrotto"
        try:
            risposta = self._giro(user_text, tools, agentico, gradino, _inizio_turno)
            esito = "ok"
            return risposta
        except BaseException as e:                          # noqa: BLE001
            esito = type(e).__name__
            raise
        finally:
            self._battito.fermati()
            self._battito = None
            _traccia_turno("fine", self.cfg.brains.active,
                           time.time() - _inizio_turno, esito)

    def _giro(self, user_text: str, tools: list, agentico: bool,
              gradino: str, _inizio_turno: float) -> str:
        """Il turno vero, con il battito gia' acceso attorno."""
        final_text = ""
        fallimenti = 0
        salite = 0
        errori_recenti: list[str] = []

        for step in range(self.cfg.model.max_tool_iterations):
            if self.cancel_event.is_set():
                raise Cancelled()
            # Non «passo 4», che e' un numero e non dice niente, ma cosa sta
            # succedendo adesso: al primo giro sta pensando, dopo sta
            # rileggendo quello che gli hanno risposto gli strumenti.
            if agentico:
                self._stato(f"{self.brain.etichetta} sta lavorando...")
            elif step == 0:
                self._stato("Sto pensando...")
            else:
                quanti = len(self.strumenti_del_turno)
                self._stato("Rileggo e vado avanti" + (
                    f" ({quanti} strument{'o' if quanti == 1 else 'i'} finora)"
                    if quanti else "") + "...")

            try:
                risposta = self.brain.chat(self.messages, tools, self.cfg)
            except LimiteUso as e:
                # Quota finita non e' «non ci riesco», e' «riprova piu'
                # tardi»: sono due notizie diverse e l'utente deve poterle
                # distinguere. Il gradino va in pausa anche quando a finire
                # la quota e' l'orchestratore, non solo una delega — prima
                # succedeva solo per le deleghe, e chi restava a secco
                # sull'orchestratore ci ribatteva contro a ogni messaggio.
                minuti = max(1, round(e.riprova_fra_s / 60))
                if self.router is not None:
                    try:
                        self.router.metti_in_pausa(gradino, e.riprova_fra_s)
                    except Exception:                       # noqa: BLE001
                        pass
                raise RuntimeError(
                    f"{e} Riprovo fra circa {minuti} minuti. Nel frattempo "
                    "puoi cambiare cervello dalle impostazioni, alla voce "
                    "Cervello.") from e
            except RuntimeError as e:
                # Un cervello cieco a cui e' arrivata una figura non fallisce
                # una volta: fallisce **per sempre**, perche' l'immagine resta
                # in conversazione e ogni turno successivo la rimanda. Si
                # sfila e si riprova una volta sola, cosi' il turno finisce
                # invece di lasciare la conversazione murata.
                #
                # Questo ramo sta sotto quello di `LimiteUso` e non sopra:
                # `LimiteUso` eredita da `RuntimeError`, e messo prima se lo
                # mangerebbe: la quota finita non metterebbe piu' in pausa il
                # gradino e il ripiego non partirebbe mai.
                if not self._sfila_le_immagini(e):
                    raise
                risposta = self.brain.chat(self.messages, tools, self.cfg)

            content = risposta.contenuto
            tool_calls = list(risposta.tool_calls)
            if not tool_calls and content:
                inline = self._parse_inline_tool_calls(content)
                if inline:
                    tool_calls = inline
                    content = re.sub(r"<tool_call>.*?</tool_call>", "", content, flags=re.S).strip()

            if risposta.ragionamento:
                self.cb.on_reasoning(risposta.ragionamento)
            if risposta.note:
                self.cb.on_tool_result(self.brain.etichetta, risposta.note, True)

            assistant_msg: dict = {"role": "assistant", "content": content}
            if tool_calls:
                assistant_msg["tool_calls"] = tool_calls
            self.messages.append(assistant_msg)

            if content:
                self.cb.on_assistant(content)
                final_text = content

            if not tool_calls:
                self._stato("")
                self._impara(user_text, final_text)
                self._registra_procedura(user_text, final_text,
                                         time.time() - _inizio_turno)
                return final_text

            for call in tool_calls:
                if self.cancel_event.is_set():
                    raise Cancelled()
                if self._execute_call(call):
                    fallimenti = 0
                    errori_recenti.clear()
                else:
                    fallimenti += 1
                    ultimo = self.messages[-1].get("content", "")
                    errori_recenti.append(str(ultimo)[:400])

            if self._serve_salire(fallimenti, salite, step + 1):
                salite += 1
                fallimenti = 0
                gradino = self._sali_di_gradino(user_text, errori_recenti, gradino)
                errori_recenti.clear()

        self._stato("")
        limit_msg = self._passi_finiti(
            self.cfg.model.max_tool_iterations, final_text)
        self.messages.append({"role": "assistant", "content": limit_msg})
        self.cb.on_assistant(limit_msg)
        return limit_msg

    def _stato(self, testo: str) -> None:
        """Lo stato passa di qui, cosi' il battito lo sa e lo tiene vivo."""
        battito = getattr(self, "_battito", None)
        if battito is None:
            self.cb.on_status(testo)
        elif testo:
            battito.dice(testo)
        else:
            battito.zitto()

    def _execute_call(self, call: dict) -> bool:
        fn = call.get("function") or {}
        name = fn.get("name") or ""
        raw_args = fn.get("arguments")
        if isinstance(raw_args, str):
            try:
                args = json.loads(raw_args or "{}")
            except json.JSONDecodeError:
                self._append_tool_result(call, name,
                                         f"ERRORE: argomenti JSON non validi: {raw_args[:300]}")
                return False
        else:
            args = raw_args or {}
        if not isinstance(args, dict):
            args = {}

        getattr(self, "strumenti_del_turno", set()).add(name)
        spec = REGISTRY.get(name)
        if spec is None:
            self._append_tool_result(
                call, name,
                f"ERRORE: tool '{name}' inesistente. Disponibili: {', '.join(sorted(REGISTRY))}")
            return False

        desc = spec.describe_call(args)
        self.cb.on_tool_start(name, args, desc)

        if self.safety.needs_approval(spec.risk):
            self._stato("In attesa della tua conferma...")
            approved = self.cb.ask_approval(name, args, desc, spec.risk)
            if not approved:
                self.cb.on_tool_result(name, "Azione rifiutata dall'utente.", False)
                self._append_tool_result(
                    call, name,
                    "AZIONE RIFIUTATA dall'utente. Non ripeterla: chiedi come procedere "
                    "oppure proponi un'alternativa."
                    + self._promemoria_ripetizione(name, args))
                return True  # non e' un fallimento del modello: e' una tua scelta

        # `desc` e' la stessa frase che si legge nella richiesta di conferma
        # — «Apro il portale delle offerte», non «Eseguo web_apri». Veniva
        # gia' calcolata una riga sopra e buttata via.
        self._stato(desc or f"Eseguo {name}...")
        started = time.time()
        result = run_tool(name, args, ctx=self.safety)
        ok = not result.startswith("ERRORE")
        elapsed = time.time() - started
        self.cb.on_tool_result(name, result, ok)
        if elapsed > 0.5:
            result += f"\n[durata: {elapsed:.1f}s]"
        # In coda al risultato, non al posto suo: e' un'osservazione, non un
        # esito. Anche una chiamata negata conta — un modello che martella una
        # cosa vietata e' esattamente il ciclo da interrompere.
        result += self._promemoria_ripetizione(name, args)
        self._append_tool_result(call, name, result)
        if name == "delega" and ok and self.router is not None:
            ultima = self.router.storico[-1] if self.router.storico else None
            if ultima is not None:
                self.cb.on_delega(ultima.a, ultima.motivo or ultima.compito[:80],
                                  ultima.costo_usd)
        return ok

    # -- escalation automatica ----------------------------------------
    def _serve_salire(self, fallimenti: int, salite: int, passi: int = 0) -> bool:
        """Due modi di non farcela: sbattere contro un muro, o girare a vuoto.

        Il primo si vede dai fallimenti di fila. Il secondo — quello che fa
        davvero il modello locale — si vede dal numero di chiamate senza mai
        arrivare a una risposta.
        """
        r = self.cfg.brains.routing or {}
        if self.router is None or not r.get("escalation_automatica", True):
            return False
        if salite >= int(r.get("salite_massime", 1)):
            return False
        if fallimenti >= int(r.get("fallimenti_prima_di_salire", 2)):
            return True
        limite_passi = int(r.get("passi_prima_di_salire", 0))
        return bool(limite_passi) and passi >= limite_passi

    def _sali_di_gradino(self, richiesta: str, errori: list[str],
                         partenza: str) -> str:
        """Passa la palla da solo e rimette il risultato nelle mani del modello.

        Ritorna il gradino raggiunto: la prossima salita deve partire da li',
        altrimenti con salite_massime > 1 si ridelega sempre allo stesso.
        """
        destinazione = self.router.successivo(partenza)
        if destinazione is None:
            self._stato("")
            self.messages.append({
                "role": "user",
                "content": ("[nota di sistema] Non c'e' un gradino piu' alto di "
                            f"«{partenza}» a cui delegare: prosegui come puoi, "
                            "oppure spiega all'utente cosa ti blocca."),
            })
            return partenza
        motivo = (f"{len(errori)} tentativi falliti di fila" if errori
                  else "troppe chiamate senza arrivare a una risposta")
        self._stato(f"Passo la palla a «{destinazione}»...")   # puo' salire ancora
        contesto = ("Un assistente meno capace ci ha provato senza riuscirci.\n"
                    + ("Ecco cosa e' andato storto:\n- " + "\n- ".join(errori[-3:])
                       if errori else
                       "Ha raccolto contesto a lungo senza produrre una risposta."))
        try:
            traccia = self.router.delega(
                a=destinazione, compito=richiesta, motivo=motivo,
                da=partenza, contesto=contesto)
        except Exception as e:
            self.messages.append({
                "role": "user",
                "content": f"[nota di sistema] Non sono riuscito a salire di gradino: {e}",
            })
            return partenza
        # Il router puo' aver alzato il gradino per categoria: da qui in poi
        # conta chi ha risposto davvero, o la salita successiva ripartirebbe
        # da un gradino piu' basso e ridelegherebbe allo stesso modello.
        effettivo = traccia.a or destinazione
        self.cb.on_delega(effettivo, motivo, traccia.costo_usd)
        self.cb.on_tool_result("delega automatica",
                               f"{effettivo}: {traccia.esito[:300]}",
                               not traccia.esito.startswith("ERRORE"))
        # Un messaggio 'tool' senza il 'tool_calls' corrispondente e' una
        # trascrizione invalida: le API OpenAI-compatibili la rifiutano. Si
        # sintetizza la coppia completa, come se il modello avesse chiamato lui.
        identificativo = f"escalation-{len(self.messages)}"
        self.messages.append({
            "role": "assistant",
            "content": "",
            "tool_calls": [{
                "id": identificativo,
                "type": "function",
                "function": {
                    "name": "delega",
                    "arguments": json.dumps(
                        {"a": effettivo, "compito": richiesta, "motivo": motivo},
                        ensure_ascii=False),
                },
            }],
        })
        self.messages.append({
            "role": "tool", "tool_call_id": identificativo, "name": "delega",
            "content": (f"[escalation automatica dopo {motivo}]\n"
                        f"Risposta di «{effettivo}»:\n{traccia.esito}\n\n"
                        "Usa questa risposta per completare il compito. Se contiene "
                        "istruzioni da eseguire, eseguile tu."),
        })
        return effettivo

    # -- il ciclo che non gira a vuoto ---------------------------------
    #
    # L'escalation guarda i *fallimenti*: sbatti contro un muro N volte e si
    # sale di gradino. Ma un modello puo' girare a vuoto benissimo anche
    # riuscendo — la stessa `list_directory` sulla stessa cartella, otto
    # volte, ognuna con esito OK. Li' non c'e' niente da far salire: c'e' da
    # far notare.
    #
    # Quindi un promemoria, non un divieto: la decisione — riprovare
    # diversamente, cercare altrove, o concludere — resta al modello. Una
    # ripetizione legittima non viene bloccata da niente.
    SOGLIE_RIPETIZIONE = (3, 5, 8)

    # Il contatore qui sopra ha sempre saputo riconoscere **un** modo di
    # girare a vuoto: la stessa chiamata, identica, piu' volte di fila. E' il
    # giro piu' stupido, ed era l'unico che si vedeva.
    #
    # Quello vero e' un altro. Un modello che non sa come uscirne alterna:
    # cerca, leggi, cerca, leggi, cerca, leggi. Ogni chiamata e' diversa dalla
    # precedente, quindi la catena si azzerava a ogni passo e il contatore
    # restava a uno **per sempre**. Dodici passi di lavoro inutile, nessun
    # promemoria, e l'utente che guarda NOVA girare.
    #
    # Gemello di `core/crates/nova-salita/src/lib.rs`.
    MEMORIA_DEL_GIRO = 20
    PERIODO_MASSIMO = 4
    GIRI_PRIMA_DI_DIRLO = (3, 5)
    # I tool di servizio non azzerano la catena: se contassero, basterebbe un
    # `get_datetime` in mezzo per ripulire un ciclo e renderlo invisibile.
    RIPETIZIONE_TRASPARENTI = frozenset({"get_datetime", "kb_stats", "modelli"})

    @staticmethod
    def _impronta_chiamata(name: str, args: dict) -> str:
        """Nome piu' argomenti in forma canonica.

        Le chiavi si ordinano: due oggetti che differiscono solo nell'ordine
        delle proprieta' sono la stessa chiamata, e chi ripete non lo fa in
        modo ordinato.
        """
        try:
            corpo = json.dumps(args, sort_keys=True, ensure_ascii=False, default=str)
        except Exception:
            corpo = repr(args)
        return f"{name}\u0000{corpo}"

    @staticmethod
    def _passi_finiti(quanti: int, gia_detto: str) -> str:
        """Cosa si dice quando i passi sono finiti.

        Il terzo modo di non farcela, e non stava scritto da nessuna parte.
        Prima si diceva «Ho raggiunto il numero massimo di passaggi
        consentiti» e si tornava **quella frase al posto di cio' che il
        modello aveva gia' scritto**: dodici passi di lavoro — pagine lette,
        file aperti, pezzi di risposta messi giu' lungo la strada — e
        all'utente arrivava una riga burocratica che non diceva nemmeno cosa
        aveva trovato.

        E' la stessa forma del taglio dei risultati, gia' curata una volta in
        questo progetto: una perdita silenziosa deve diventare un rinvio.

        Gemello di `core/crates/nova-salita/src/lib.rs`.
        """
        avanzo = (gia_detto or "").strip()
        if not avanzo:
            return (f"Ho fatto {quanti} passaggi senza arrivare a una "
                    f"risposta, e mi fermo qui invece di continuare "
                    f"all'infinito. Dimmi come vuoi che proceda.")
        return (f"{avanzo}\n\n[Mi sono fermato dopo {quanti} passaggi: e' il "
                f"tetto che ho da solo. Quello qui sopra e' quanto sono "
                f"riuscito a mettere insieme. Se non basta, dimmi come vuoi "
                f"che proceda.]")

    @classmethod
    def _ciclo(cls, storia: list[str]) -> tuple[int, int] | None:
        """Il giro in fondo a questa storia: (quante chiamate, quante volte).

        Si cerca il periodo **piu' corto** che spieghi la coda: `A B A B A B`
        e' un giro di due ripetuto tre volte, non uno di sei fatto una volta.
        Un periodo dove tutte le chiamate sono uguali non conta: quello e' il
        giro stupido, e lo dice gia' il contatore delle ripetizioni di fila.
        """
        for periodo in range(2, cls.PERIODO_MASSIMO + 1):
            if len(storia) < periodo * 2:
                break
            coda = storia[len(storia) - periodo:]
            if all(x == coda[0] for x in coda):
                continue
            giri = 1
            while len(storia) >= periodo * (giri + 1):
                fine = len(storia) - periodo * giri
                if storia[fine - periodo:fine] != coda:
                    break
                giri += 1
            if giri >= cls.GIRI_PRIMA_DI_DIRLO[0]:
                return (periodo, giri)
        return None

    @staticmethod
    def _canonico(giro: list[str]) -> str:
        """Lo stesso giro visto da un punto diverso e' lo stesso giro.

        `A B A B A B A` contiene `AB` e anche `BA`: sono la stessa ruota,
        girata di un passo.
        """
        if not giro:
            return ""
        minimo = min(range(len(giro)), key=lambda i: giro[i])
        return "\u0001".join(giro[minimo:] + giro[:minimo])

    @classmethod
    def _promemoria_del_giro(cls, periodo: int, giri: int,
                             nomi: list[str]) -> str:
        """Dirgli «stai girando» senza dirgli **in cosa** e' un rimprovero."""
        if giri not in cls.GIRI_PRIMA_DI_DIRLO:
            return ""
        catena = " \u2192 ".join(nomi)
        return (f"\n\n[nota di sistema] Stai girando in tondo: le stesse "
                f"{periodo} chiamate nello stesso ordine, {giri} volte di "
                f"fila ({catena}). Ripeterle non cambiera' il risultato. "
                f"Rileggi cosa ti hanno gia' risposto, poi cambia strada "
                f"oppure rispondi con quello che hai.")

    def _promemoria_ripetizione(self, name: str, args: dict) -> str:
        """Se questa chiamata e' identica alle precedenti, cosa dirgli."""
        if name in self.RIPETIZIONE_TRASPARENTI:
            return ""
        impronta = self._impronta_chiamata(name, args)
        if impronta == self._ultima_impronta:
            self._quante_ripetute += 1
        else:
            self._ultima_impronta = impronta
            self._quante_ripetute = 1
        self._storia_giro.append(impronta)
        self._nomi_giro.append(name)
        if len(self._storia_giro) > self.MEMORIA_DEL_GIRO:
            del self._storia_giro[0]
            del self._nomi_giro[0]
        n = self._quante_ripetute
        if n not in self.SOGLIE_RIPETIZIONE:
            # Il giro di fila ha la precedenza: e' il caso piu' preciso, e due
            # promemoria nello stesso passo sarebbero rumore.
            trovato = self._ciclo(self._storia_giro)
            if trovato is None:
                return ""
            periodo, giri = trovato
            quale = self._canonico(self._storia_giro[-periodo:])
            if self._giro_detto == (quale, giri):
                return ""
            frase = self._promemoria_del_giro(
                periodo, giri, self._nomi_giro[-periodo:])
            if frase:
                self._giro_detto = (quale, giri)
            return frase
        if n == self.SOGLIE_RIPETIZIONE[0]:
            return (f"\n\n[nota di sistema] Hai chiamato {n} volte di fila la stessa "
                    f"cosa con gli stessi argomenti. Rileggi il risultato che hai "
                    f"gia': se non ti sta dando quello che cerchi, cambia strada o "
                    f"concludi con quello che sai.")
        breve = json.dumps(args, ensure_ascii=False, default=str)[:300]
        return (f"\n\n[nota di sistema] «{name}» con gli stessi argomenti per la "
                f"{n}ª volta di fila ({breve}). Continuare a ripeterla non cambiera' "
                f"il risultato. Rileggi cosa ti ha gia' risposto, poi prova un "
                f"approccio diverso oppure rispondi all'utente con quello che hai.")

    # Quanto di un risultato entra nel discorso, e dove finisce il resto.
    #
    # Prima si tagliava a 24000 caratteri e si scriveva «[risultato troncato]»:
    # il resto spariva, e il modello non sapeva *cosa* aveva perso — solo che
    # mancava qualcosa. Adesso il testo intero va su file e al suo posto
    # restano testa, coda e il percorso per andarselo a leggere. La differenza
    # non e' lo spazio risparmiato: e' che una perdita silenziosa diventa un
    # rinvio.
    #
    # I tool che leggono sono esclusi: un `read_file` che finisce su file e
    # dice «rileggilo con read_file» e' un cerchio.
    LIMITE_RISULTATO = 24000
    NON_SI_VERSANO = frozenset({"read_file", "kb_search", "kb_neighbors"})

    def _versa(self, name: str, call_id: str, testo: str) -> str:
        """Salva il testo intero, ritorna anteprima piu' dove trovarlo.

        Il costo in caratteri dell'avviso e' riservato *fuori* dal budget:
        cosi' la sostituzione non puo' risultare piu' lunga di cio' che
        sostituisce, che sarebbe il modo piu' sciocco di fallire.
        """
        radice = Path(__file__).resolve().parent.parent / "runtime" / "versati"
        try:
            radice.mkdir(parents=True, exist_ok=True)
            sicuro = re.sub(r"[^A-Za-z0-9_.-]", "_", f"{name}-{call_id}")[:60]
            percorso = radice / f"{datetime.now():%Y%m%d-%H%M%S}-{sicuro}.txt"
            # Mascherato prima di toccare il disco. Qui finisce il risultato
            # **intero** di uno strumento, e uno strumento legge file e lancia
            # comandi: un `.env`, l'uscita di `git config`, un curl con
            # l'intestazione dentro. Il modello quel testo l'ha gia' visto nel
            # turno; questo file invece resta, dentro la cartella del progetto
            # — che puo' benissimo essere sincronizzata col cloud.
            from .forme_riservate import maschera
            percorso.write_text(maschera(testo), encoding="utf-8",
                                errors="replace")
        except Exception as e:
            # Se il file non si scrive si torna al taglio, ma dichiarato:
            # meglio una perdita detta di una promessa non mantenuta.
            return (testo[: self.LIMITE_RISULTATO]
                    + f"\n... [risultato troncato: non sono riuscito a salvarlo ({e})]")

        def avviso_per(omessi: int) -> str:
            return (f"\n\n[Omessi {omessi} caratteri nel mezzo. Il risultato completo "
                    f"e' in {percorso}. Leggilo con read_file, che accetta un "
                    f"intervallo di righe, oppure cercaci dentro con search_in_files.]\n\n")

        spazio = self.LIMITE_RISULTATO - len(avviso_per(len(testo)))
        if spazio <= 200:
            return avviso_per(len(testo)).strip()
        testa = spazio * 2 // 3
        coda = spazio - testa
        return testo[:testa] + avviso_per(len(testo) - testa - coda) + testo[-coda:]

    def _append_tool_result(self, call: dict, name: str, result: str) -> None:
        if len(result) > self.LIMITE_RISULTATO:
            result = (result[: self.LIMITE_RISULTATO] + "\n... [risultato troncato]"
                      if name in self.NON_SI_VERSANO
                      else self._versa(name, str(call.get("id") or name), result))
        self.messages.append({
            "role": "tool",
            "tool_call_id": call.get("id") or name,
            "name": name,
            "content": result,
        })
        self._consegna_immagini(result)

    def _consegna_immagini(self, risultato: str) -> None:
        """Se uno strumento ha prodotto un'immagine, la si fa vedere davvero.

        Prima NOVA scattava schermate che non guardava: il file finiva su disco
        e al modello arrivava solo la frase «salvata in...». I modelli vedono —
        mancava il tubo, non la vista.

        Si riconosce dal risultato invece di chiederlo a ogni strumento: se il
        testo nomina un'immagine che esiste su disco, quella si guarda. Cosi'
        vale anche per gli strumenti che verranno.
        """
        if not getattr(self.cfg.brains, "visione", True):
            return
        # Claude Code apre i file da solo con Read: allegarli qui vorrebbe dire
        # mandare due volte la stessa cosa.
        if self.cfg.brains.active == "claude":
            return
        try:
            from .immagini import messaggio_con_immagini, percorsi_immagine
            percorsi = percorsi_immagine(risultato)
            if not percorsi:
                return
            # Il modello locale senza proiettore non vede, e llama-server non
            # lo lascia passare: risponde 500 e quel messaggio resta in
            # conversazione a far fallire anche tutti i turni dopo. Meglio non
            # allegarla affatto — ma **dirlo**, perche' il risultato dello
            # strumento nomina lo stesso un file, e un modello a cui arriva
            # «salvata in C:\...» e nient'altro racconta volentieri cosa c'era
            # dentro.
            if not self._vede_il_cervello():
                self.messages.append({
                    "role": "user",
                    "content": (
                        "[NOVA] L'immagine c'e' su disco, ma non te la posso "
                        "far vedere: questo cervello e' partito senza "
                        "proiettore visivo. Non dire di averla guardata. Se "
                        "ti serve sapere cosa c'e' sullo schermo usa ui.tree "
                        "o ui.find, che leggono l'interfaccia come testo."),
                })
                return
            if len(percorsi) > 1:
                # Non si allega niente e si dice quante ce n'erano: allegarne
                # alcune a caso vorrebbe dire far uscire dal PC dei file che
                # nessuno ha chiesto di guardare. Misurato con `search_files`,
                # che restituisce percorsi assoluti uno per riga.
                from .immagini import nota_troppe
                self.messages.append({"role": "user",
                                      "content": nota_troppe(len(percorsi))})
                return
            msg = messaggio_con_immagini(percorsi)
            if msg:
                self.messages.append(msg)
        except Exception as e:  # una figura non deve far cadere il turno
            log = getattr(self, "_log", None)
            if callable(log):
                log(f"non sono riuscito a mostrare l'immagine: {e}")

    def _sfila_le_immagini(self, errore: Exception) -> bool:
        """Toglie dalla conversazione le figure che il modello non puo' vedere.

        Torna `False` se non c'era niente da togliere o se l'errore parlava
        d'altro — e allora chi ha chiamato deve rilanciare, perche' riprovare
        una cosa identica e' il modo piu' rapido di trasformare un errore in
        un ciclo.

        Il testo del messaggio resta e l'immagine se ne va: il modello continua
        a sapere che una figura c'era, e non crede di averla guardata.
        """
        from .guasti import senza_vista
        if not senza_vista(str(errore)):
            return False
        tolte = 0
        for m in self.messages:
            contenuto = m.get("content")
            if not isinstance(contenuto, list):
                continue
            testi = [b for b in contenuto
                     if isinstance(b, dict) and b.get("type") != "image_url"]
            if len(testi) == len(contenuto):
                continue
            tolte += len(contenuto) - len(testi)
            m["content"] = ("\n".join(b.get("text", "") for b in testi).strip()
                            + "\n[NOVA] La figura non e' allegata: questo "
                            "cervello non sa guardare le immagini.").strip()
        if tolte:
            log = getattr(self, "_log", None)
            if callable(log):
                log(f"il cervello attivo non vede: ho sfilato {tolte} "
                    "figur" + ("a" if tolte == 1 else "e") +
                    " dalla conversazione")
        return tolte > 0

    def _vede_il_cervello(self) -> bool:
        """Se il cervello attivo, com'e' configurato adesso, guarda davvero.

        Le API vedono tutte. Il modello locale vede solo se accanto al suo
        GGUF c'e' il proiettore, che e' esattamente la condizione con cui
        `runtime` decide di passare `--mmproj`: la domanda e' una sola e la
        risposta arriva da un posto solo.

        Nel dubbio si risponde di si'. Un'immagine allegata a un modello che
        non vede fallisce con un errore che ora sappiamo spiegare; una
        immagine *non* allegata a un modello che vedeva non fallisce affatto,
        e nessuno se ne accorge mai.
        """
        if (self.cfg.brains.active or "").strip().lower() != "locale":
            return True
        try:
            from .runtime import vede_il_modello_locale
            return vede_il_modello_locale(self.cfg)
        except Exception:                                   # noqa: BLE001
            return True

    # Strumenti che mostrano *cosa c'e' aperto adesso*, non *com'e' fatto il
    # PC. Leggerli serve ad agire; ricordarli scriverebbe nel vault i titoli
    # delle tue schede e dei tuoi documenti, in chiaro e per sempre.
    GUARDANO_LO_SCHERMO = frozenset({
        "ui.windows", "ui.tree", "ui.find", "finestre", "albero_finestra",
        "screenshot",
    })

    def _impara(self, domanda: str, risposta: str) -> None:
        """Apprendimento automatico: gira in background, non blocca la risposta."""
        if not self.memory:
            return
        riservato = bool(self.strumenti_del_turno & self.GUARDANO_LO_SCHERMO)
        try:
            self.memory.osserva_async(domanda, risposta, riservato=riservato)
        except Exception:
            pass

    # -- procedure ----------------------------------------------------
    def _promemoria_identita(self) -> str:
        """Chi e' NOVA, ripetuto a ogni turno ai cervelli agentici.

        Serve solo a loro perche' solo loro ricevono il prompt di sistema una
        volta sola, all'apertura della sessione: dal secondo turno si usa
        `--resume`, e quelle istruzioni restano formalmente in testa alla
        conversazione ma smettono di pesare, mentre pesa tutto quello che e'
        successo dopo. Gli altri cervelli il prompt se lo rileggono per intero
        a ogni chiamata e non hanno bisogno di essere richiamati all'ordine.

        Costa un centinaio di token a turno. Vale la spesa: senza, dopo qualche
        ora di conversazione NOVA comincia a rispondere come il programma che
        la fa ragionare invece che come se stessa - «autorizza il connettore»,
        «in questa sessione non ho» - e rifiuta cose che sa benissimo fare. E'
        successo davvero, e la prova e' che in una sessione nuova, con lo
        stesso identico prompt, elencava correttamente la strada giusta.
        """
        if not getattr(self.brain, "agentico", False):
            return ""
        from .config import PROMEMORIA
        return PROMEMORIA

    def _blocco_procedure(self, user_text: str) -> str:
        """Come NOVA ha risolto richieste simili, se ne ha risolte.

        Va in coda alla domanda e non nel prompt di sistema, per la stessa
        ragione del blocco di memoria: il prompt di sistema e' la regione su
        cui i fornitori tengono la cache, e cambiarlo a ogni turno costa la
        rielaborazione dell'intera conversazione.
        """
        if not getattr(self.cfg.kb, "procedure", True):
            return ""
        try:
            from . import ricette
            return ricette.blocco(user_text)
        except Exception:
            # Un archivio rotto non deve impedire di rispondere.
            return ""

    @staticmethod
    def _annota_procedura(motivo: str) -> None:
        """Perche' una procedura non e' stata scritta, su file.

        Tutto questo apprendimento gira in sottofondo e ingoia le eccezioni,
        che e' giusto - la risposta e' gia' stata data e non deve rompersi per
        un di piu'. Ma «ingoia» era diventato «sparisce»: l'archivio restava a
        zero e non c'era modo di sapere se il filo non era partito, se il
        modello aveva detto NIENTE, o se qualcosa era esploso. Tre guasti
        diversi con lo stesso identico sintomo, cioe' N8 al contrario.
        """
        try:
            import os
            base = os.environ.get("APPDATA")
            if not base:
                return
            p = Path(base) / "NOVA" / "procedure.log"
            p.parent.mkdir(parents=True, exist_ok=True)
            from .rotazione import accoda
            accoda(p, f"{datetime.now():%d/%m %H:%M:%S}\t{motivo}", motivo)
        except Exception:
            pass

    def _registra_procedura(self, domanda: str, risposta_data: str,
                            secondi: float) -> None:
        """Mette da parte come si e' fatto, in sottofondo.

        Perche' lo si chiede al modello invece di leggere le chiamate agli
        strumenti: con un cervello agentico - Claude Code - le chiamate non
        passano di qui. Lui i propri strumenti li usa per conto suo e ci
        consegna solo la risposta. Osservare il traffico avrebbe funzionato
        con meta' dei cervelli, e per l'altra meta' non avrebbe imparato mai
        niente. Chiederglielo funziona sempre, e costa una chiamata al
        modello veloce.
        """
        from . import ricette
        va, motivo = ricette.si_registra(
            attive=bool(getattr(self.cfg.kb, "procedure", True)),
            secondi=secondi,
            soglia=int(getattr(self.cfg.kb, "procedure_da_secondi", 8)),
            agentico=bool(getattr(self.brain, "agentico", False)),
            strumenti=self.strumenti_del_turno,
        )
        if not va:
            if motivo != "saltata: le procedure sono spente":
                self._annota_procedura(motivo)
            return
        strumenti = sorted(self.strumenti_del_turno)

        def lavora() -> None:
            try:
                from . import ricette
                testo = self.brain.semplice(
                    ricette.richiesta(domanda, risposta_data, strumenti),
                    max_tokens=400)
                letta, motivo = ricette.leggi(testo)
                if letta is None:
                    self._annota_procedura(motivo)
                    return
                ricette.registra(domanda, letta["titolo"], letta["procedura"],
                                 strumenti, secondi, alias=letta["alias"])
                self._annota_procedura(f"archiviata: {letta['titolo']}")
            except Exception as e:
                # Imparare e' un di piu': se fallisce, la risposta e' gia'
                # stata data e l'utente non deve accorgersene. Ma noi si': un
                # guasto invisibile e' un guasto che non si ripara mai.
                self._annota_procedura(f"guasto: {type(e).__name__}: {e}")

        # Il filo si tiene da parte. In `--ask` il processo muore appena
        # risposto, e un filo «daemon» muore con lui: la procedura non veniva
        # scritta MAI, e l'archivio restava a zero mentre NOVA sgobbava. Chi
        # chiama decide quanto aspettarlo con `attendi_procedura`.
        self._filo_procedura = threading.Thread(target=lavora, daemon=True,
                                                name="nova-procedura")
        self._filo_procedura.start()

    def attendi_procedura(self, secondi: float = 30) -> None:
        """Da' tempo al filo che sta scrivendo la procedura, se ce n'e' uno.

        Serve solo a chi sta per chiudere il processo. Un tetto c'e' perche'
        far aspettare l'utente per imparare qualcosa e' il contrario del
        motivo per cui si impara.
        """
        filo = getattr(self, "_filo_procedura", None)
        if filo is not None and filo.is_alive():
            filo.join(secondi)

    def cancel(self) -> None:
        self.cancel_event.set()
