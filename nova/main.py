"""Punto di ingresso di NOVA."""
from __future__ import annotations

import argparse
import sys

from .config import CONFIG_PATH, Config
from .setup_wizard import autoconfigure


def _prepare_config(reconfigure: bool = False) -> Config:
    cfg = Config.load()
    notes = autoconfigure(cfg, force=reconfigure)
    cfg.save()
    for n in notes:
        print("[setup]", n)
    return cfg


def run_gui(cfg: Config) -> int:
    from PyQt6.QtWidgets import QApplication
    app = QApplication(sys.argv)
    app.setApplicationName("NOVA")
    app.setQuitOnLastWindowClosed(False)

    from .ui.main_window import MainWindow
    win = MainWindow(cfg)
    if not cfg.ui.start_minimized:
        win.show()
    return app.exec()


def run_cli(cfg: Config, once: str | None = None, no_server: bool = False) -> int:
    """Modalita' testuale: utile per test e diagnostica."""
    from .agent import Agent, AgentCallbacks
    from .runtime import LlamaServer

    server = LlamaServer(cfg, on_log=lambda m: None)
    if not no_server:
        print("[nova] avvio del modello...", flush=True)
        server.start(wait=True)
        print(f"[nova] pronto su {cfg.base_url} "
              f"[{server.accelerator}, ngl={server.gpu_layers}]", flush=True)

    auto_yes = cfg.safety.autonomy == "autonomous"

    def ask(name, args, desc, risk):
        if auto_yes:
            return True
        print(f"\n[conferma richiesta] {desc}")
        return input("Consentire? [s/N] ").strip().lower() in ("s", "si", "y", "yes")

    from .kb_setup import collega_memoria, esegui_seed_se_serve, prepara_kb
    vault, kb_engine = prepara_kb(cfg, log=lambda m: print("[nova]", m, flush=True))
    esegui_seed_se_serve(cfg, vault, kb_engine, log=lambda m: print("[kb]", m, flush=True))

    agent = Agent(cfg, kb_engine=kb_engine, vault=vault, callbacks=AgentCallbacks(
        on_status=lambda s: None,
        on_assistant=lambda t: print(f"\nNOVA: {t}", flush=True),
        on_tool_start=lambda n, a, d: print(f"\n  -> {d}", flush=True),
        on_tool_result=lambda n, r, ok: print(
            f"  <- {'OK' if ok else 'ERR'}: {r[:400]}", flush=True),
        ask_approval=ask,
        on_delega=lambda a, motivo, costo: print(
            f"\n  ~> delega a «{a}»: {motivo}"
            + (f"  ({costo:.4f} $)" if costo else ""), flush=True),
    ))
    memoria = collega_memoria(agent, vault, cfg,
                    on_learn=lambda nodi: print(
                        "\n[kb] imparato: " + ", ".join(n.title for n in nodi), flush=True))
    agent.detect_model()
    print(f"[nova] modello: {agent.model_name}", flush=True)

    try:
        if once:
            agent.send(once)
            if memoria is not None:
                print("[kb] attendo la scrittura in memoria...", flush=True)
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
            agent.send(text)
    finally:
        if not no_server:
            server.stop()
    return 0


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="nova", description="Assistente digitale locale")
    ap.add_argument("--cli", action="store_true", help="modalita' testuale invece della GUI")
    ap.add_argument("--ask", metavar="TESTO", help="esegue una singola richiesta e termina")
    ap.add_argument("--reconfigure", action="store_true", help="ririleva modello e runtime")
    ap.add_argument("--no-server", action="store_true",
                    help="non avviare llama-server (usa un server gia' attivo)")
    ap.add_argument("--config", action="store_true", help="stampa il percorso di configurazione")
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
    args = ap.parse_args(argv)

    if args.config:
        print(CONFIG_PATH)
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
        from .brains import BRAINS, crea_brain
        for nome in BRAINS:
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
                       no_server=args.no_server or cfg.brains.active != "locale")
    return run_gui(cfg)


if __name__ == "__main__":
    raise SystemExit(main())
