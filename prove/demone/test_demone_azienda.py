# -*- coding: utf-8 -*-
"""L'azienda dei Dot: la direzione e i reparti nascono coi Dot accesi (D395).

Deciso con Gio il 9 ottobre: i Dot sono un'azienda vera. La direzione
(l'APM, AR, l'Architetto) e i reparti (il legale, il commerciale, la
ricerca, la revisione, la scrittura, i dati, la qualita', l'amministrazione)
ci sono sempre: li fa nascere NOVA coi Dot accesi, come il custode, e senza
compiti non costano niente. L'APM e' il capo di tutti. La direzione e il
legale non prendono compiti a mano e non leggono la posta: il loro lavoro
arriva coi progetti. I reparti si'.

Un Dot dell'utente che si chiamava gia' come un posto resta com'e', e quel
posto resta vuoto; se e' l'APM, non nasce nessuno, perche' tutti gli altri
l'avrebbero come capo.

Esce 2 — «qui non si puo' provare» — se il demone non e' costruito.
"""
import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "novad.exe" if os.name == "nt" else "novad"
DEMONE = next((p for p in (RADICE / "core" / "target" / "release" / NOME,
                           RADICE / "core" / "target" / "debug" / NOME)
               if p.is_file()), None)
if DEMONE is None:
    print("Il demone non e' costruito per questo sistema. Per averlo:")
    print("  cd core && cargo build --release -p novad")
    sys.exit(2)

from nova.core_client import CoreClient                           # noqa: E402

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


#: I posti fissi, come li scrive `nova_dot::azienda::POSTI`.
DIREZIONE = ["apm", "ar", "architetto"]
REPARTI = ["legale", "commerciale", "ricerca", "revisione", "scrittura", "dati", "qualita",
           "amministrazione"]
#: Chi non prende compiti a mano.
SU_CHIAMATA = {"apm", "ar", "architetto", "legale"}


class Demone:
    """Un demone con la sua casa, i Dot gia' scritti e l'interruttore."""

    def __init__(self, prefisso: str, dots_utente: dict[str, dict], accesi: str):
        self.casa = tempfile.mkdtemp(prefix=prefisso)
        self.nova = Path(self.casa) / "NOVA"
        for nome, dot in dots_utente.items():
            c = self.nova / "dots" / nome
            c.mkdir(parents=True, exist_ok=True)
            (c / "dot.json").write_text(json.dumps(dot, ensure_ascii=False), encoding="utf-8")
        self.nova.mkdir(parents=True, exist_ok=True)
        self.configura(accesi)
        self.endpoint = ("\\\\.\\pipe\\" + f"{prefisso}{os.getpid()}" if os.name == "nt"
                         else str(Path(self.casa) / "nova.sock"))
        self.processo = None

    def configura(self, accesi: str):
        cfg = {"kb": {"enabled": False}, "dots": {"accesi": accesi}}
        (self.nova / "config.json").write_text(json.dumps(cfg), encoding="utf-8")

    def accendi(self):
        ambiente = dict(os.environ)
        ambiente.update({"APPDATA": self.casa, "HOME": self.casa, "USERPROFILE": self.casa,
                         "XDG_CONFIG_HOME": self.casa, "XDG_RUNTIME_DIR": self.casa,
                         "NOVA_PROVA_VRAM_MIB": str(8 * 1024)})
        for k in ("http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY", "all_proxy",
                  "ALL_PROXY", "OPENAI_API_KEY"):
            ambiente.pop(k, None)
        self.processo = subprocess.Popen([str(DEMONE), "--endpoint", self.endpoint, "--log", "warn"],
                                         env=ambiente, stdout=subprocess.DEVNULL,
                                         stderr=subprocess.PIPE)
        scadenza = time.time() + 20
        while time.time() < scadenza and self.processo.poll() is None:
            if CoreClient.disponibile(self.endpoint):
                return
            time.sleep(0.3)
        print("  il demone non ha risposto:",
              (self.processo.stderr.read() or b"").decode("utf-8", "replace")[-400:])
        self.spegni()
        sys.exit(2)

    def spegni(self):
        if self.processo and self.processo.poll() is None:
            self.processo.kill()
            self.processo.wait(timeout=10)

    def capacita(self, cap, **args):
        with CoreClient(self.endpoint, timeout=60) as c:
            return c.request("capabilities/call", {"name": cap, "args": args})

    def errore(self, cap, **args):
        try:
            self.capacita(cap, **args)
        except Exception as e:                                    # noqa: BLE001
            return str(e)
        return ""

    def dots(self) -> dict[str, dict]:
        return {d["nome"]: d for d in self.capacita("dot.stato")["dots"]}

    def aspetta(self, nome: str, secondi: float = 20) -> bool:
        fine = time.time() + secondi
        while time.time() < fine:
            if nome in self.dots():
                return True
            time.sleep(0.3)
        return False


# Un Dot dell'utente che si chiama come un reparto, nato prima del D395.
MIO = {"nome": "ricerca", "ruolo": "Il mio ricercatore.", "nato": "2026-10-01T10:00:00"}
d = Demone("nova-azienda-", {"ricerca": MIO}, accesi="no")
d2 = None
try:
    d.accendi()

    print("\n1. coi Dot spenti l'azienda non nasce")
    controlla("ci sono solo il custode e il Dot dell'utente",
              set(d.dots()) == {"custode", "ricerca"}, str(sorted(d.dots())))

    print("\n2. accesi, nascono la direzione e i reparti, senza riavviare")
    d.configura("si")
    controlla("l'APM nasce entro pochi secondi", d.aspetta("apm"), str(sorted(d.dots())))
    time.sleep(1)
    tutti = d.dots()
    controlla("ci sono tutti i posti, e il custode",
              set(tutti) == {"custode", *DIREZIONE, *REPARTI}, str(sorted(tutti)))
    controlla("i posti nati da NOVA sono fissi",
              all(tutti[n].get("fisso") is True for n in DIREZIONE + REPARTI if n != "ricerca"),
              json.dumps({n: tutti[n].get("fisso") for n in tutti}))
    controlla("l'APM e' il capo di tutti, e non ha un capo",
              tutti["apm"]["capo"] == ""
              and all(tutti[n]["capo"] == "apm" for n in DIREZIONE + REPARTI
                      if n not in ("apm", "ricerca")),
              json.dumps({n: tutti[n]["capo"] for n in tutti}))
    controlla("la direzione e il legale non prendono compiti, i reparti si'",
              all(tutti[n]["prende_compiti"] is (n not in SU_CHIAMATA)
                  for n in DIREZIONE + REPARTI),
              json.dumps({n: tutti[n]["prende_compiti"] for n in tutti}))
    controlla("il Dot dell'utente resta suo: non e' fisso, ha il suo ruolo e nessun capo",
              tutti["ricerca"].get("fisso") is False and tutti["ricerca"]["ruolo"] == MIO["ruolo"]
              and tutti["ricerca"]["capo"] == "", json.dumps(tutti["ricerca"]))
    scritto = json.loads((d.nova / "dots" / "ricerca" / "dot.json").read_text(encoding="utf-8"))
    controlla("e il suo dot.json non e' stato toccato", scritto == MIO, str(scritto))

    print("\n3. nella vista: chi e' su chiamata, e cosa fa")
    v = {x["nome"]: x for x in d.capacita("dot.vista", nome="")["dots"]}
    controlla("l'APM e' su chiamata, e la vista dice perche'",
              v["apm"]["sta"] == "su_chiamata"
              and v["apm"]["a_parte"] == "L'APM non prende compiti e non legge la posta: "
                                         "guida i progetti che Nova gli passa.",
              json.dumps(v["apm"], ensure_ascii=False))
    controlla("il commerciale e' libero, e si puo' scrivergli",
              v["commerciale"]["sta"] == "libero" and v["commerciale"]["a_parte"] is None,
              json.dumps(v["commerciale"], ensure_ascii=False))
    controlla("il custode resta il custode", v["custode"]["sta"] == "custode")

    print("\n4. i posti non si rifanno a mano, e chi e' su chiamata dice di no")
    e = d.errore("dot.crea", nome="legale", ruolo="r")
    controlla("un Dot non nasce col nome di un posto", "posto fisso" in e, e)
    e = d.errore("dot.crea", nome="altro-ar", ruolo="r", mestiere="ar")
    controlla("ne' col suo mestiere", "posto fisso" in e and "uno solo" in e, e)
    e = d.errore("dot.crea", nome="sotto", ruolo="r", capo="apm")
    controlla("l'APM non ha sottoposti a mano", "l'APM non ha sottoposti" in e, e)
    e = d.errore("dot.affida", nome="apm", compito="fai qualcosa")
    controlla("non prende compiti", "l'APM non prende compiti" in e, e)
    e = d.errore("dot.scrivi", a="ar", testo="ciao")
    controlla("AR non legge la posta", "AR non legge la posta" in e, e)
    e = d.errore("dot.gruppo", nome="g", membri=["legale", "commerciale"])
    controlla("il legale non sta nei gruppi", "il legale non sta nei gruppi" in e, e)
    controlla("e il custode dice di no come prima",
              "il custode dei permessi non prende compiti" in d.errore("dot.affida", nome="custode",
                                                                        compito="x"))
    controlla("a un reparto invece si scrive",
              d.capacita("dot.scrivi", a="commerciale", testo="ciao").get("a") == "commerciale")

    print("\n5. a ogni accensione non si rifanno")
    nato = json.loads((d.nova / "dots" / "apm" / "dot.json").read_text(encoding="utf-8"))["nato"]
    d.spegni()
    d.accendi()
    dopo = d.dots()
    controlla("gli stessi Dot", set(dopo) == set(tutti), str(sorted(dopo)))
    rinato = json.loads((d.nova / "dots" / "apm" / "dot.json").read_text(encoding="utf-8"))["nato"]
    controlla("e l'APM e' quello di prima", rinato == nato, f"{nato} -> {rinato}")

    print("\n6. se l'APM e' un Dot dell'utente, non nasce nessuno")
    d2 = Demone("nova-azienda-apm-", {"apm": {"nome": "apm", "ruolo": "Il mio.", "nato": "t"}},
                accesi="si")
    d2.accendi()
    time.sleep(2)
    controlla("ci sono solo il custode e il suo apm", set(d2.dots()) == {"custode", "apm"},
              str(sorted(d2.dots())))
finally:
    d.spegni()
    if d2:
        d2.spegni()

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
