# -*- coding: utf-8 -*-
"""Il portale: un altro dispositivo parla col NOVA del PC, solo se abbinato (D448).

Deciso con Gio il 27 novembre: un tablet che e' «solo NOVA», e il lavoro lo
fa il PC. NOVA ha le mani sul PC, quindi chi entra dalla rete lo comanda: la
porta e' spenta di serie, il canale e' cifrato col certificato del PC
riconosciuto dall'impronta, e si entra solo con un codice che vale una volta
o col gettone avuto all'abbinamento.

Qui il dispositivo e' questo script, che parla TLS con la libreria di Python:
un cliente scritto da un'altra mano rispetto al demone. Si guarda che:

0. di serie il portale e' spento, e spento non da' inviti;
1. acceso dal pannello ascolta, e dice la sua impronta;
2. chi non saluta, o saluta con un codice o un gettone non suoi, resta fuori;
3. col codice dell'invito ci si abbina, e sullo stesso canale il demone
   risponde come alla finestra del PC; il codice vale una volta sola;
4. col gettone si rientra; sul disco del PC il gettone non c'e';
5. un modello non puo' abbinare nessuno: il portale non e' fra le capacita';
6. riacceso il demone il portale si riaccende da solo, con la stessa
   impronta, e il dispositivo abbinato rientra;
7. un dispositivo dimenticato mentre e' collegato esce subito e non rientra;
8. spento il portale chi era collegato esce, e la porta e' chiusa; i
   dispositivi abbinati si vedono e si dimenticano anche da spento;
9. spento e riacceso di fila la porta si riapre; un indirizzo su cui
   ascoltare scritto male non diventa «tutte le reti».

Esce 2 — «qui non si puo' provare» — se il demone non e' costruito.
"""
import base64
import hashlib
import json
import os
import socket
import ssl
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


casa = Path(tempfile.mkdtemp(prefix="nova-portale-"))
cartella_nova = casa / "NOVA"
(cartella_nova / "copie-buone").mkdir(parents=True, exist_ok=True)


def porta_libera() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


PORTA = porta_libera()


def scrivi_config(acceso, ascolta="127.0.0.1"):
    """La configurazione come la scrive il pannello, con la copia buona.

    Si ascolta solo su questa macchina: la prova non apre niente verso la
    rete, e il firewall di Windows non ha da chiedere.
    """
    cfg = {"kb": {"enabled": False}}
    if acceso is not None:
        cfg["portale"] = {"acceso": acceso, "porta": PORTA, "ascolta": ascolta,
                          "indirizzi": ["pc.di-prova.example"]}
    testo = json.dumps(cfg, ensure_ascii=False, indent=2)
    (cartella_nova / "config.json").write_text(testo, encoding="utf-8")
    (cartella_nova / "copie-buone" / "config.json").write_text(testo, encoding="utf-8")


scrivi_config(None)

endpoint = (rf"\\.\pipe\nova-portale-{os.getpid()}" if os.name == "nt"
            else str(casa / "nova.sock"))
ambiente = dict(os.environ)
ambiente.update({"APPDATA": str(casa), "HOME": str(casa), "USERPROFILE": str(casa),
                 "XDG_CONFIG_HOME": str(casa), "XDG_RUNTIME_DIR": str(casa)})
for k in ("http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY"):
    ambiente.pop(k, None)


def rpc(metodo, **params):
    with CoreClient(endpoint, timeout=60) as c:
        return c.request(metodo, params)


def errore_di(metodo, **params) -> str:
    try:
        rpc(metodo, **params)
    except Exception as e:  # noqa: BLE001
        return str(e)
    return ""


def accendi():
    p = subprocess.Popen([str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
                         env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    scadenza = time.time() + 20
    while time.time() < scadenza and p.poll() is None:
        if CoreClient.disponibile(endpoint):
            return p
        time.sleep(0.3)
    print("il demone non ha risposto:", (p.stderr.read() or b"").decode("utf-8", "replace")[-400:])
    p.kill()
    sys.exit(2)


def aspetta_che(condizione, secondi=20):
    fine = time.time() + secondi
    while time.time() < fine:
        if condizione():
            return True
        time.sleep(0.2)
    return condizione()


class Dispositivo:
    """Il dispositivo: TLS 1.3 verso il portale, e righe JSON."""

    def __init__(self):
        ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        # Nessuna autorita' firma il certificato del PC: si riconosce
        # dall'impronta, che qui si calcola e si confronta a mano.
        ctx.check_hostname = False
        ctx.verify_mode = ssl.CERT_NONE
        ctx.minimum_version = ssl.TLSVersion.TLSv1_3
        self.s = ctx.wrap_socket(socket.create_connection(("127.0.0.1", PORTA), timeout=15))
        der = self.s.getpeercert(binary_form=True)
        self.impronta = base64.urlsafe_b64encode(hashlib.sha256(der).digest()).rstrip(b"=").decode()
        self.resto = b""

    def manda(self, oggetto):
        self.s.sendall((json.dumps(oggetto) + "\n").encode("utf-8"))

    def riga(self, secondi=15):
        """La prossima riga, o None se il canale si e' chiuso."""
        self.s.settimeout(secondi)
        while b"\n" not in self.resto:
            try:
                pezzo = self.s.recv(65536)
            except (ssl.SSLError, ConnectionError, OSError) as e:
                if isinstance(e, socket.timeout):
                    raise
                return None
            if not pezzo:
                return None
            self.resto += pezzo
        riga, self.resto = self.resto.split(b"\n", 1)
        return json.loads(riga)

    def chiedi(self, metodo, params=None, ident=1):
        self.manda({"jsonrpc": "2.0", "id": ident, "method": metodo, "params": params or {}})
        while True:
            r = self.riga()
            if r is None or r.get("id") == ident:
                return r

    def chiuso(self, secondi=10) -> bool:
        """Il PC ha chiuso il canale? Le righe che arrivano prima non contano."""
        fine = time.time() + secondi
        while time.time() < fine:
            try:
                if self.riga(max(0.1, fine - time.time())) is None:
                    return True
            except socket.timeout:
                return False
        return False

    def chiudi(self):
        try:
            self.s.close()
        except OSError:
            pass


def porta_chiusa() -> bool:
    try:
        socket.create_connection(("127.0.0.1", PORTA), timeout=3).close()
    except OSError:
        return True
    return False


processo = accendi()
try:
    print("\n0. di serie il portale e' spento")
    stato = rpc("portale/stato")
    controlla("spento, e non in ascolto",
              stato.get("acceso") is False and stato.get("in_ascolto") is False, json.dumps(stato))
    controlla("spento non da' inviti", "il portale e' spento" in errore_di("portale/abbina"),
              errore_di("portale/abbina"))
    controlla("e la porta e' chiusa", porta_chiusa())

    print("\n1. acceso dal pannello, ascolta")
    scrivi_config(True)
    stato = rpc("portale/applica")
    controlla("in ascolto sulla porta del pannello",
              stato.get("in_ascolto") is True and stato.get("porta") == PORTA, json.dumps(stato))
    impronta = stato.get("impronta", "")
    controlla("dice la sua impronta", len(impronta) == 43, impronta)
    controlla("nessun dispositivo, per ora", stato.get("dispositivi") == [], json.dumps(stato))

    print("\n2. chi non saluta, o non e' abbinato, resta fuori")
    d = Dispositivo()
    controlla("il certificato e' quello dell'impronta", d.impronta == impronta, d.impronta)
    d.manda({"jsonrpc": "2.0", "id": 1, "method": "daemon/status"})
    r = d.riga()
    controlla("una richiesta al demone senza saluto: no",
              r is not None and r.get("ok") is False and "result" not in r, json.dumps(r))
    controlla("e il canale si chiude", d.chiuso())
    d.chiudi()
    d = Dispositivo()
    inizio = time.time()
    d.manda({"fai": "entra", "id": "nessuno", "gettone": "inventato"})
    r = d.riga()
    controlla("un gettone inventato: no, e non subito",
              r is not None and r.get("ok") is False and "non e' abbinato" in r.get("errore", "")
              and time.time() - inizio >= 1.0, f"{json.dumps(r)} in {time.time() - inizio:.2f}s")
    d.chiudi()
    d = Dispositivo()
    d.manda({"fai": "abbina", "codice": "tirato-a-indovinare", "nome": "intruso"})
    r = d.riga()
    controlla("un codice inventato: no",
              r is not None and r.get("ok") is False and "il codice non vale" in r.get("errore", ""),
              json.dumps(r))
    d.chiudi()

    print("\n3. col codice dell'invito ci si abbina")
    invito = rpc("portale/abbina")
    riga = invito.get("invito", "")
    campi = dict(p.split("=", 1) for p in riga.split("?", 1)[-1].split("&"))
    controlla("l'invito dice dove, quale porta, quale impronta, e ha un codice",
              riga.startswith("nova://abbina?v=1&") and "pc.di-prova.example" in campi.get("a", "")
              and campi.get("p") == str(PORTA) and campi.get("i") == impronta
              and len(campi.get("c", "")) == 22, riga)
    controlla("col suo QR, e quanto vale",
              "<svg" in invito.get("qr", "") and invito.get("vale_s") == 300, str(invito)[:200])
    controlla("lo stato dice che c'e' un codice in corso",
              0 < rpc("portale/stato").get("codice_scade_fra_s", 0) <= 300)
    d = Dispositivo()
    d.manda({"fai": "abbina", "codice": campi["c"], "nome": "Tab A8"})
    r = d.riga()
    controlla("abbinato: un id e un gettone",
              r is not None and r.get("ok") is True and r.get("id") and len(r.get("gettone", "")) == 43,
              json.dumps(r))
    ident, gettone = r["id"], r["gettone"]
    r = d.chiedi("daemon/status")
    controlla("sullo stesso canale il demone risponde come alla finestra",
              r is not None and "uptime_s" in json.dumps(r.get("result", {})), json.dumps(r)[:300])
    stato = rpc("portale/stato")
    controlla("il pannello lo vede, collegato",
              [(x["nome"], x["collegato"]) for x in stato["dispositivi"]] == [("Tab A8", True)],
              json.dumps(stato["dispositivi"]))
    controlla("il codice si e' consumato", "codice_scade_fra_s" not in stato, json.dumps(stato))
    d2 = Dispositivo()
    d2.manda({"fai": "abbina", "codice": campi["c"], "nome": "secondo"})
    r = d2.riga()
    controlla("lo stesso codice una seconda volta: no",
              r is not None and r.get("ok") is False, json.dumps(r))
    d2.chiudi()
    d.chiudi()
    controlla("staccato, il pannello lo vede scollegato",
              aspetta_che(lambda: rpc("portale/stato")["dispositivi"][0]["collegato"] is False))

    print("\n4. col gettone si rientra")
    d = Dispositivo()
    d.manda({"fai": "entra", "id": ident, "gettone": gettone})
    r = d.riga()
    controlla("entrato", r == {"ok": True, "id": ident}, json.dumps(r))
    r = d.chiedi("capabilities/list")
    capacita = [c["name"] for c in (r or {}).get("result", {}).get("capabilities", [])]
    controlla("e ha le capacita' del demone", len(capacita) > 20, str(len(capacita)))
    d.chiudi()
    sul_disco = (cartella_nova / "portale" / "dispositivi.json").read_text(encoding="utf-8")
    controlla("sul disco del PC il gettone non c'e': c'e' la sua impronta",
              gettone not in sul_disco and hashlib.sha256(gettone.encode()).hexdigest() in sul_disco)

    print("\n5. un modello non puo' abbinare nessuno")
    controlla("il portale non e' fra le capacita'",
              not [c for c in capacita if "portale" in c], str([c for c in capacita if "portale" in c]))
    strumenti = [t["name"] for t in rpc("tools/list").get("tools", [])]
    controlla("ne' fra gli strumenti dei modelli",
              len(strumenti) > 20 and not [t for t in strumenti if "portale" in t])
    controlla("e chiesto come capacita' non esiste",
              "sconosciuta" in errore_di("capabilities/call", name="portale.abbina", args={}))

    print("\n6. riacceso il demone, il portale si riaccende da solo")
    processo.kill()
    processo.wait(10)
    if os.name != "nt":
        Path(endpoint).unlink(missing_ok=True)
    controlla("a demone spento la porta e' chiusa", aspetta_che(porta_chiusa))
    processo = accendi()
    controlla("in ascolto senza che nessuno lo chieda",
              aspetta_che(lambda: rpc("portale/stato").get("in_ascolto") is True))
    controlla("con la stessa impronta", rpc("portale/stato").get("impronta") == impronta)
    d = Dispositivo()
    d.manda({"fai": "entra", "id": ident, "gettone": gettone})
    controlla("e il dispositivo abbinato rientra", d.riga() == {"ok": True, "id": ident})

    print("\n7. dimenticato mentre e' collegato: esce subito e non rientra")
    r = d.chiedi("events/subscribe", {"topics": ["*"]})
    controlla("era iscritto agli eventi", r is not None and r.get("result", {}).get("ok") is True,
              json.dumps(r))
    r = rpc("portale/dimentica", id=ident)
    controlla("dimenticato", r == {"dimenticato": True}, json.dumps(r))
    controlla("il suo canale si chiude", d.chiuso())
    d.chiudi()
    controlla("il pannello non lo ha piu'", rpc("portale/stato")["dispositivi"] == [])
    controlla("dimenticarlo due volte non fa niente",
              rpc("portale/dimentica", id=ident) == {"dimenticato": False})
    d = Dispositivo()
    d.manda({"fai": "entra", "id": ident, "gettone": gettone})
    r = d.riga()
    controlla("e col suo gettone non rientra",
              r is not None and r.get("ok") is False and "non e' abbinato" in r.get("errore", ""),
              json.dumps(r))
    d.chiudi()

    print("\n8. spento il portale, chi era collegato esce e la porta e' chiusa")
    codice = dict(p.split("=", 1) for p in rpc("portale/abbina")["invito"].split("&"))["c"]
    d = Dispositivo()
    d.manda({"fai": "abbina", "codice": codice, "nome": "ultimo"})
    r = d.riga()
    controlla("un altro dispositivo, abbinato e collegato", r is not None and r.get("ok") is True,
              json.dumps(r))
    scrivi_config(False)
    stato = rpc("portale/applica")
    controlla("spento", stato.get("acceso") is False and stato.get("in_ascolto") is False,
              json.dumps(stato))
    controlla("chi era collegato esce", d.chiuso())
    d.chiudi()
    controlla("e la porta e' chiusa", aspetta_che(porta_chiusa))
    ultimo = r["id"]
    controlla("da spento il dispositivo abbinato si vede ancora, scollegato",
              [(x["id"], x["collegato"]) for x in stato["dispositivi"]] == [(ultimo, False)],
              json.dumps(stato))
    controlla("e si dimentica anche da spento",
              rpc("portale/dimentica", id=ultimo) == {"dimenticato": True}
              and rpc("portale/stato")["dispositivi"] == [])

    print("\n9. spento e riacceso di fila; un indirizzo scritto male")
    for giro in range(3):
        scrivi_config(True)
        acceso = rpc("portale/applica")
        scrivi_config(False)
        spento = rpc("portale/applica")
        if not (acceso.get("in_ascolto") is True and "errore" not in acceso
                and spento.get("in_ascolto") is False):
            break
    controlla("tre volte di fila la porta si riapre e si richiude",
              acceso.get("in_ascolto") is True and "errore" not in acceso
              and spento.get("in_ascolto") is False, f"giro {giro}: {json.dumps(acceso)}")
    scrivi_config(True, ascolta="tutte")
    stato = rpc("portale/applica")
    controlla("un indirizzo scritto male: il portale non parte, e dice perche'",
              stato.get("in_ascolto") is False and "non e' un indirizzo" in stato.get("errore", ""),
              json.dumps(stato))
    controlla("e la porta resta chiusa", porta_chiusa())
    scrivi_config(True)
    stato = rpc("portale/applica")
    controlla("corretto, parte e l'errore sparisce",
              stato.get("in_ascolto") is True and "errore" not in stato, json.dumps(stato))
    controlla("e il dispositivo dimenticato da spento non c'e' piu'", stato.get("dispositivi") == [])

    print("\n10. il microfono di un dispositivo diventa un orecchio (D455)")
    # Un decimo di secondo di silenzio a 16 kHz, come lo manda il tablet.
    PEZZO = {"pcm": base64.b64encode(b"\0\0" * 1600).decode()}

    def evento_orecchio(chi, secondi=15):
        """Il prossimo evento «voce.orecchio» che arriva a questo dispositivo."""
        fine = time.time() + secondi
        while time.time() < fine:
            try:
                r = chi.riga(max(0.1, fine - time.time()))
            except socket.timeout:
                return None
            if r is None:
                return None
            if r.get("method") == "event" and r["params"].get("topic") == "voce.orecchio":
                return r["params"]["data"]
        return None

    codice = dict(p.split("=", 1) for p in rpc("portale/abbina")["invito"].split("&"))["c"]
    d = Dispositivo()
    d.manda({"fai": "abbina", "codice": codice, "nome": "tablet che ascolta"})
    r = d.riga()
    controlla("un dispositivo abbinato e collegato", r is not None and r.get("ok") is True, json.dumps(r))
    ident, gettone = r["id"], r["gettone"]
    # Un secondo collegamento dello stesso dispositivo, che guarda e basta.
    occhio = Dispositivo()
    occhio.manda({"fai": "entra", "id": ident, "gettone": gettone})
    controlla("e un secondo collegamento che guarda", (occhio.riga() or {}).get("ok") is True)
    for chi in (d, occhio):
        chi.chiedi("events/subscribe", {"topics": ["voce.orecchio"]})
    for _ in range(3):
        d.manda({"jsonrpc": "2.0", "method": "voce/pezzo", "params": PEZZO})
    e = evento_orecchio(d)
    controlla("al primo pezzo nasce il suo orecchio, e si dice",
              e == {"id": ident, "nome": "tablet che ascolta", "aperto": True}, json.dumps(e))
    controlla("lo sa anche chi guarda", evento_orecchio(occhio) == e)
    d.manda({"jsonrpc": "2.0", "method": "voce/pezzo", "params": {"pcm": "non sono campioni"}})
    d.manda({"jsonrpc": "2.0", "method": "voce/pezzo", "params": {}})
    r = d.chiedi("ping", ident=7)
    controlla("un pezzo che non si legge si butta, e il canale resta",
              r is not None and "pong" in r.get("result", {}), json.dumps(r))
    controlla("dalla pipe del PC nessuno manda un microfono: il metodo non c'e'",
              "metodo sconosciuto: voce/pezzo" in errore_di("voce/pezzo", **PEZZO),
              errore_di("voce/pezzo", **PEZZO))
    d.chiudi()
    e = evento_orecchio(occhio)
    controlla("quando il dispositivo esce il suo orecchio si chiude",
              e == {"id": ident, "nome": "tablet che ascolta", "aperto": False}, json.dumps(e))
    occhio.chiudi()
    controlla("e si dimentica come gli altri",
              rpc("portale/dimentica", id=ident) == {"dimenticato": True})
finally:
    processo.kill()

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
if falliti:
    print("::error::test_demone_portale: " + "; ".join(falliti))
sys.exit(1 if falliti else 0)
