"""Punto di ingresso di NOVA."""
from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path

from .config import CONFIG_PATH, Config
from .setup_wizard import autoconfigure


def _traccia_main() -> None:
    """Una riga quando main() parte. Serve a distinguere due ipotesi diverse:
    «il cervello non e' questo» e «il cervello e' questo ma non passa di qui»."""
    try:
        import datetime, os
        from pathlib import Path as _P
        f = _P(__file__).resolve().parent.parent / "avvio.log"
        with open(f, "a", encoding="utf-8") as fh:
            fh.write(f"{datetime.datetime.now():%d/%m %H:%M:%S} "
                     f"pid={os.getpid()} MAIN argv={' '.join(sys.argv[:3])!r}\n")
    except Exception:
        pass


def _prepare_config(reconfigure: bool = False) -> Config:
    _traccia_main()
    cfg = Config.load()
    notes = autoconfigure(cfg, force=reconfigure)
    cfg.save()
    for n in notes:
        print("[setup]", n)
    return cfg


def guscio() -> Path | None:
    """Dove sta l'orb. In `bin/` per chi installa, in `core/target/` per chi compila."""
    radice = Path(__file__).resolve().parent.parent
    nome = "nova-shell.exe" if os.name == "nt" else "nova-shell"
    for posto in (radice / "bin" / nome,
                  radice / "core" / "target" / "release" / nome):
        if posto.is_file():
            return posto
    return None


def avvia_orb() -> int:
    """L'interfaccia di NOVA e' l'orb, e ce n'e' una sola.

    Fino a poco fa ce n'erano due: l'orb, e una finestra PyQt che era la
    prima interfaccia di NOVA e che nessuno aveva mai spento. Due interfacce
    non sono una scelta in piu' per l'utente, sono due posti dove le cose si
    scollano: i menu del cervello erano solo in una, la fascia che dice cosa
    esce dal PC pure, e alla domanda «cosa vede uno appena installato» le
    risposte erano due. La seconda e' stata tolta.

    Qui non si aspetta: l'orb e' un processo che vive per conto suo, e questo
    comando ha finito quando lo ha acceso.
    """
    exe = guscio()
    if exe is None:
        print("Non trovo l'orb (nova-shell). Se hai installato NOVA con "
              "install.ps1 dovrebbe stare in bin\\; se la stai compilando, "
              "fallo con .\\build.ps1.\n"
              "Nel frattempo la puoi usare da qui: python -m nova --cli",
              flush=True)
        return 1
    from .processi import SENZA_FINESTRA
    try:
        subprocess.Popen([str(exe)], cwd=str(exe.parent.parent),
                         creationflags=SENZA_FINESTRA)
    except Exception as e:                                  # noqa: BLE001
        from .guasti import registra, spiega
        registra(e, dove="avvio orb")
        print(spiega(e, "Non sono riuscita ad avviare l'orb"), flush=True)
        return 1
    return 0


POSTILLA_VOCE = """

<voce>
Questa domanda arriva dal microfono e la tua risposta verra' letta ad alta voce.
Rispondi come si parla: frasi brevi, niente elenchi puntati, niente markdown,
niente blocchi di codice se non sono davvero il contenuto della risposta. Se
servirebbe una risposta lunga, di' a voce il nocciolo.

Ogni risposta a voce finisce SEMPRE, come ultima cosa e da sola su una riga, con
uno di questi tre marcatori. Non e' opzionale: e' parte del formato.
- [NOVA:APERTO]  la conversazione continua, resti in ascolto;
- [NOVA:PAUSA]   l'utente vuole sospendere e riprendere piu' tardi;
- [NOVA:FINE]    l'utente sta chiudendo, in qualunque modo lo dica.

Non e' una lista di parole da riconoscere: giudica l'intenzione dell'ultima
frase e scegli tu. Nel dubbio fra continuare e chiudere, un commiato (ciao,
a dopo, grazie e basta, va bene cosi') e' [NOVA:FINE]. Il marcatore va per
ultimo, da solo, e non si commenta mai.
</voce>"""


def run_cli(cfg: Config, once: str | None = None, no_server: bool = False,
            dalla_voce: bool = False) -> int:
    """Modalita' testuale: utile per test e diagnostica.

    Regola su cosa va dove: su **stdout** solo la risposta, su **stderr**
    tutto il resto — avanzamento, tool, deleghe, memoria. Serve a chi ci
    parla da fuori: il guscio grafico legge stdout e lo mette in una bolla,
    e finche' i log erano mescolati alla risposta l'utente si vedeva
    «[nova] KB: 55 nodi...» dentro il messaggio di NOVA.
    """
    import sys as _sys

    # Su Windows lo stdout rediretto prende la codifica locale (cp1252), e gli
    # accenti italiani arrivano a chi legge come byte non validi: «non è
    # malware» diventa «non � malware». Chi ci parla da fuori vuole UTF-8.
    for flusso in (_sys.stdout, _sys.stderr):
        try:
            flusso.reconfigure(encoding="utf-8", errors="replace")
        except Exception:
            pass

    def log(*pezzi):
        print(*pezzi, file=_sys.stderr, flush=True)
    from .agent import Agent, AgentCallbacks
    from .runtime import LlamaServer

    server = LlamaServer(cfg, on_log=lambda m: None)
    if not no_server:
        log("[nova] avvio del modello...")
        server.start(wait=True)
        log(f"[nova] pronto su {cfg.base_url} "
            f"[{server.accelerator}, ngl={server.gpu_layers}]")

    auto_yes = cfg.safety.autonomy == "autonomous"

    def ask(name, args, desc, risk):
        if auto_yes:
            return True
        log(f"\n[conferma richiesta] {desc}")
        return input("Consentire? [s/N] ").strip().lower() in ("s", "si", "y", "yes")

    from .kb_setup import collega_memoria, esegui_seed_se_serve, prepara_kb
    vault, kb_engine = prepara_kb(cfg, log=lambda m: log("[nova]", m))
    esegui_seed_se_serve(cfg, vault, kb_engine, log=lambda m: log("[kb]", m))

    agent = Agent(cfg, kb_engine=kb_engine, vault=vault, callbacks=AgentCallbacks(
        on_status=lambda s: None,
        # In --ask la risposta e' *tutto* stdout: chi legge da fuori non deve
        # dover togliere un prefisso per avere il testo.
        on_assistant=lambda t: print(t if once else f"\nNOVA: {t}", flush=True),
        on_tool_start=lambda n, a, d: log(f"  -> {d}"),
        on_tool_result=lambda n, r, ok: log(
            f"  <- {'OK' if ok else 'ERR'}: {r[:400]}"),
        ask_approval=ask,
        on_delega=lambda a, motivo, costo: log(
            f"  ~> delega a «{a}»: {motivo}"
            + (f"  ({costo:.4f} $)" if costo else "")),
    ))
    memoria = collega_memoria(agent, vault, cfg,
                    on_learn=lambda nodi: log(
                        "[kb] imparato: " + ", ".join(n.title for n in nodi)))
    agent.detect_model()
    log(f"[nova] modello: {agent.model_name}")

    try:
        if once:
            try:
                agent.send(once, postilla=POSTILLA_VOCE if dalla_voce else "")
            except KeyboardInterrupt:
                raise
            except Exception as e:
                # In `--ask` lo standard output E' la risposta: un tracciato
                # Python qui arriva all'utente dentro la bolla della chat,
                # pieno di percorsi di file e righe di libreria. Un cervello
                # che si ferma e' una notizia da dare, non un programma che si
                # sbriciola: si dice cosa e' successo, in italiano, e si esce
                # da fermi.
                motivo = str(e).strip() or f"{type(e).__name__} senza spiegazione"
                print(motivo, flush=True)
                log(f"[nova] il cervello si e' fermato: {type(e).__name__}: {e}")
                return 1
            # Prima la procedura, che e' breve: se il processo muore mentre
            # la scrive, quella fatica e' persa e la volta dopo NOVA
            # ricomincia a cercare da zero.
            agent.attendi_procedura(30)
            if memoria is not None:
                log("[kb] attendo la scrittura in memoria...")
                memoria.attendi(180)
            return 0
        while True:
            try:
                text = input("\nTu> ").strip()
            except (EOFError, KeyboardInterrupt):
                break
            if text.lower() in ("exit", "quit", "esci"):
                break
            if not text:
                continue
            try:
                agent.send(text)
            except KeyboardInterrupt:
                raise
            except Exception as e:
                # Alla tastiera vale lo stesso: un turno andato male non deve
                # portarsi via la conversazione.
                print(f"\nNOVA: {str(e).strip() or type(e).__name__}", flush=True)
    finally:
        if not no_server:
            server.stop()
    return 0


def _console_in_italiano() -> None:
    """La console di Windows non parla UTF-8 finche' non glielo si dice.

    Il valore predefinito qui e' la tabella codici 850, che non ha il trattino
    lungo ne' varie altre cose che NOVA scrive in italiano: l'utente vede dei
    punti interrogativi e pensa a un guasto, quando invece e' solo il terminale
    che non sa disegnare quel carattere. Due mosse: si dice a Windows che
    l'uscita e' UTF-8, e si dice a Python di scriverla cosi' - con
    errors="replace", perche' un accento che non si stampa non deve fermare un
    comando.
    """
    if os.name == "nt":
        try:
            import ctypes
            ctypes.windll.kernel32.SetConsoleOutputCP(65001)
        except Exception:                                   # noqa: BLE001
            pass
    for flusso in (sys.stdout, sys.stderr):
        try:
            flusso.reconfigure(encoding="utf-8", errors="replace")
        except Exception:                                   # noqa: BLE001
            pass


def main(argv: list[str] | None = None) -> int:
    _console_in_italiano()
    # La rete si stende per prima. Sotto pythonw non c'e' una console: un
    # errore non gestito qui dentro chiuderebbe NOVA senza lasciare niente,
    # ne' sullo schermo ne' su disco.
    from .guasti import installa
    installa()
    ap = argparse.ArgumentParser(prog="nova", description="Assistente digitale locale")
    ap.add_argument("--cli", action="store_true", help="modalita' testuale invece della GUI")
    ap.add_argument("--ask", metavar="TESTO", help="esegue una singola richiesta e termina")
    ap.add_argument("--voce", action="store_true",
                    help="la domanda arriva dal microfono: risposta parlata e "
                         "marcatori di chiusura")
    ap.add_argument("--reconfigure", action="store_true", help="ririleva modello e runtime")
    ap.add_argument("--no-server", action="store_true",
                    help="non avviare llama-server (usa un server gia' attivo)")
    ap.add_argument("--config", action="store_true", help="stampa il percorso di configurazione")
    ap.add_argument("--harness", action="store_true",
                    help="apre la finestra dell'harness e la tiene aggiornata")
    ap.add_argument("--pianificate", action="store_true",
                    help="esegue le automazioni pianificate che sono dovute")
    ap.add_argument("--list-tools", action="store_true", help="elenca i tool disponibili")
    ap.add_argument("--seed-kb", action="store_true",
                    help="ri-mappa il PC nella knowledge base e termina")
    ap.add_argument("--kb", metavar="QUERY", help="cerca nella knowledge base e termina")
    ap.add_argument("--kb-stats", action="store_true",
                    help="statistiche della knowledge base")
    ap.add_argument("--brain", choices=["locale", "claude", "api"],
                    help="quale cervello usare in questa esecuzione")
    ap.add_argument("--brains", action="store_true",
                    help="elenca i cervelli e il loro stato")
    ap.add_argument("--modelli", action="store_true",
                    help="mostra i gradini del router e quanto si e' speso")
    ap.add_argument("--registro", nargs="?", const="", metavar="PAROLE",
                    help="cosa NOVA ha fatto e non si annulla; con delle "
                         "parole, cerca fra le azioni")
    ap.add_argument("--giorni", type=float, default=0,
                    help="con --registro: solo gli ultimi N giorni")
    ap.add_argument("--dati", action="store_true",
                    help="dove NOVA tiene le tue cose, quanto pesano e cosa "
                         "succede se le cancelli")
    args = ap.parse_args(argv)

    if args.config:
        print(CONFIG_PATH)
        return 0

    if args.dati:
        # Come il registro: legge il disco e basta. Chi vuole sapere dove
        # stanno i suoi dati spesso lo vuole sapere *prima* di fidarsi
        # abbastanza da far partire il resto.
        from .dati import racconta
        print(racconta())
        return 0

    if args.registro is not None:
        # Non passa dalla configurazione ne' dal cervello: legge un file e lo
        # racconta. Deve funzionare anche quando NOVA non parte piu' - anzi,
        # soprattutto allora, perche' e' il momento in cui serve sapere cosa
        # aveva fatto.
        from .registro import cerca, racconta, riassunto
        if not args.registro and not args.giorni:
            print(riassunto())
            print()
        righe = cerca(testo=args.registro, giorni=args.giorni, quante=40)
        if args.registro and not righe:
            print(f"Niente che contenga «{args.registro}».")
            return 0
        print(racconta(righe=righe))
        return 0

    if args.harness:
        # Prima di tutto il resto: non serve ne' la configurazione ne' un
        # cervello, questa finestra legge un file e disegna.
        from .harness_finestra import avvia, segna_viva
        segna_viva()
        return avvia()

    if args.pianificate:
        # Il giro che chiama l'attivita' pianificata del sistema. Non passa da
        # _prepare_config ne' dal modello: esegue quello che tocca e tace.
        from .pianificazione import esegui_dovute
        for x in esegui_dovute():
            print(f"{x['nome']}: {x['esito']}"
                  + ("  (cambiato)" if x.get("cambiato") else ""))
        return 0

    if args.list_tools:
        from .tools import REGISTRY
        for name, t in sorted(REGISTRY.items()):
            print(f"{t.risk.name:<10} {name:<22} {t.description[:90]}")
        return 0

    cfg = _prepare_config(args.reconfigure)
    if args.brain:
        cfg.brains.active = args.brain
        cfg.save()

    if args.modelli:
        from .routing import Router
        stato = Router(cfg).stato()
        if stato["spesa_reale"]:
            print(f"orchestratore: {stato['orchestratore']}   "
                  f"speso: {stato['equivalente_usd']} $ / tetto {stato['tetto_usd']} $\n")
        else:
            print(f"orchestratore: {stato['orchestratore']}   "
                  f"nessun gradino a consumo: {stato['equivalente_usd']} $ "
                  f"e' l'equivalente API, non una spesa\n")
        for g in stato["gradini"]:
            segno = "*" if g["gradino"] == stato["orchestratore"] else " "
            dove = ("locale" if g["locale"]
                    else ("a consumo" if g["a_consumo"] else "incluso"))
            print(f"{segno} {g['gradino']:<12} {g['cervello']:<8} {g['modello']:<28} "
                  f"{dove:<12} {'pronto' if g['pronto'] else 'NON pronto'}")
            if g["descrizione"]:
                print(f"    {g['descrizione']}")
            if not g["pronto"] and g["nota"]:
                print(f"    ! {g['nota']}")
        return 0

    if args.brains:
        # elenco_brains, non BRAINS: le CLI configurate sono cervelli a tutti
        # gli effetti, e finche' questo elenco ne mostrava tre chi ne aveva
        # una installata non aveva modo di accorgersene.
        from .brains import crea_brain, elenco_brains
        for nome in elenco_brains(cfg):
            b = crea_brain(nome, cfg)
            pronto, motivo = b.disponibile()
            attivo = "*" if nome == cfg.brains.active else " "
            print(f"{attivo} {nome:<8} {'pronto' if pronto else 'non disponibile':<16} "
                  f"{b.descrizione_stato() if pronto else motivo}")
        return 0

    if args.seed_kb or args.kb or args.kb_stats:
        from .kb_setup import esegui_seed_se_serve, prepara_kb
        vault, engine = prepara_kb(cfg, log=lambda m: print("[nova]", m))
        if vault is None:
            print("Knowledge base disattivata in config.json (kb.enabled=false).")
            return 1
        if args.seed_kb:
            esegui_seed_se_serve(cfg, vault, engine, log=lambda m: print("[kb]", m), forza=True)
        if args.kb:
            ris = engine.cerca(args.kb, top_k=8)
            for h in ris.hits:
                corpo = h.node.body.strip().replace("\n", " ")[:220]
                print(f"\n[{h.node.slug}] {h.node.title}  ({h.node.tipo}, "
                      f"conf {h.node.confidenza:.2f}, via {h.via})\n  {corpo}")
            print(f"\n-- {ris.audit}")
        if args.kb_stats or args.seed_kb:
            import json as _json
            print(_json.dumps(vault.statistiche(), indent=2, ensure_ascii=False))
        return 0

    if args.no_server:
        cfg.server.autostart_model = False

    if cfg.brains.active != "locale":
        cfg.server.autostart_model = False

    if args.cli or args.ask:
        return run_cli(cfg, once=args.ask,
                       no_server=args.no_server or cfg.brains.active != "locale",
                       dalla_voce=args.voce)
    return avvia_orb()


if __name__ == "__main__":
    raise SystemExit(main())
