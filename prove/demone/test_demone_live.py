# -*- coding: utf-8 -*-
"""Gemini Live nel demone: le voci del pannello e la prova di una (D386).

Il pannello chiede al demone l'elenco delle voci (`voce.live.voci`) e ne fa
sentire una (`voce.live.prova`): Gemini Live si presenta con quella voce, dal
vivo. Qui al posto di Google c'e' un server websocket finto, scritto a mano
sul protocollo (RFC 6455), che risponde come Live: apre la sessione, legge la
domanda e manda voce e trascrizione. Il demone lo trova con
`NOVA_LIVE_INDIRIZZO`.

Si prova anche quello che non deve succedere: la chiave non torna al
pannello e non finisce nei log; con «solo sul PC» acceso o senza chiave non
si apre niente.

Esce 2 — «qui non si puo' provare» — se il demone non e' costruito.
"""
import base64
import hashlib
import json
import os
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path
from urllib.parse import parse_qs, urlparse

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


CHIAVE = "CHIAVE-DI-PROVA-7f3a"

# --- il server websocket finto ------------------------------------------------

GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"


def leggi_esatti(s, n):
    dati = b""
    while len(dati) < n:
        pezzo = s.recv(n - len(dati))
        if not pezzo:
            raise ConnectionError("chiuso")
        dati += pezzo
    return dati


def leggi_messaggio(s):
    """Un messaggio del client: (opcode, dati). Il client maschera sempre."""
    testa = leggi_esatti(s, 2)
    opcode = testa[0] & 0x0F
    lunghezza = testa[1] & 0x7F
    if lunghezza == 126:
        lunghezza = struct.unpack(">H", leggi_esatti(s, 2))[0]
    elif lunghezza == 127:
        lunghezza = struct.unpack(">Q", leggi_esatti(s, 8))[0]
    maschera = leggi_esatti(s, 4) if testa[1] & 0x80 else b"\0\0\0\0"
    dati = bytes(b ^ maschera[i % 4] for i, b in enumerate(leggi_esatti(s, lunghezza)))
    return opcode, dati


def manda_testo(s, valore):
    dati = json.dumps(valore).encode("utf-8")
    if len(dati) < 126:
        testa = struct.pack(">BB", 0x81, len(dati))
    elif len(dati) < 65536:
        testa = struct.pack(">BBH", 0x81, 126, len(dati))
    else:
        testa = struct.pack(">BBQ", 0x81, 127, len(dati))
    s.sendall(testa + dati)


def prossimo_json(s):
    while True:
        opcode, dati = leggi_messaggio(s)
        if opcode == 0x8:
            raise ConnectionError("il client ha chiuso")
        if opcode in (0x1, 0x2):
            return json.loads(dati.decode("utf-8"))


#: Quello che il server ha visto, una voce per collegamento.
collegamenti: list[dict] = []
#: Quanti secondi di voce manda il server: mezzo secondo a 24 kHz.
CAMPIONI = 12000


def servi(s):
    visto = {}
    collegamenti.append(visto)
    try:
        richiesta = b""
        while b"\r\n\r\n" not in richiesta:
            pezzo = s.recv(4096)
            if not pezzo:
                return
            richiesta += pezzo
        righe = richiesta.decode("latin-1").split("\r\n")
        visto["percorso"] = righe[0].split(" ")[1]
        intestazioni = {r.split(":", 1)[0].strip().lower(): r.split(":", 1)[1].strip()
                        for r in righe[1:] if ":" in r}
        accetta = base64.b64encode(hashlib.sha1(
            (intestazioni["sec-websocket-key"] + GUID).encode()).digest()).decode()
        s.sendall(("HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\n"
                   "Connection: Upgrade\r\n"
                   f"Sec-WebSocket-Accept: {accetta}\r\n\r\n").encode())
        visto["apertura"] = prossimo_json(s)
        manda_testo(s, {"setupComplete": {}})
        visto["domanda"] = prossimo_json(s)
        voce = base64.b64encode(struct.pack(f"<{CAMPIONI}h", *([300] * CAMPIONI))).decode()
        nome = (visto["apertura"]["setup"]["generationConfig"]["speechConfig"]
                ["voiceConfig"]["prebuiltVoiceConfig"]["voiceName"])
        manda_testo(s, {"serverContent": {"modelTurn": {"parts": [
            {"inlineData": {"mimeType": "audio/pcm;rate=24000", "data": voce}}]}}})
        manda_testo(s, {"serverContent": {"outputTranscription": {"text": f"Ciao, sono {nome}."}}})
        manda_testo(s, {"serverContent": {"turnComplete": True}})
        while True:
            leggi_messaggio(s)
    except (ConnectionError, OSError, KeyError, ValueError):
        pass
    finally:
        s.close()


ascolto = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
ascolto.bind(("127.0.0.1", 0))
ascolto.listen(8)


def accetta_sempre():
    while True:
        try:
            c, _ = ascolto.accept()
        except OSError:
            return
        threading.Thread(target=servi, args=(c,), daemon=True).start()


threading.Thread(target=accetta_sempre, daemon=True).start()

# --- il demone ----------------------------------------------------------------

casa = tempfile.mkdtemp(prefix="nova-live-")
cartella_nova = Path(casa) / "NOVA"
cartella_nova.mkdir(parents=True, exist_ok=True)


def configura(chiave=CHIAVE, solo_locale=False, conversazione="gemini_live"):
    (cartella_nova / "config.json").write_text(json.dumps({
        "kb": {"enabled": False},
        "voice": {"conversazione": conversazione, "live_api_key": chiave,
                  "live_voce": "Charon"},
        "brains": {"routing": {"solo_locale": solo_locale}},
    }, ensure_ascii=False), encoding="utf-8")


configura()
endpoint = (rf"\\.\pipe\nova-live-{os.getpid()}" if os.name == "nt"
            else str(Path(casa) / "nova.sock"))
ambiente = dict(os.environ)
ambiente.update({"APPDATA": casa, "HOME": casa, "USERPROFILE": casa,
                 "XDG_CONFIG_HOME": casa, "XDG_RUNTIME_DIR": casa,
                 "NOVA_LIVE_INDIRIZZO": f"ws://127.0.0.1:{ascolto.getsockname()[1]}/live"})
# La chiave deve venire dal pannello: una dell'ambiente avrebbe la precedenza.
for k in ("GEMINI_API_KEY", "GOOGLE_API_KEY", "http_proxy", "HTTP_PROXY", "https_proxy",
          "HTTPS_PROXY", "all_proxy", "ALL_PROXY"):
    ambiente.pop(k, None)

registro = Path(casa) / "novad.log"
uscita = open(registro, "wb")
processo = subprocess.Popen([str(DEMONE), "--endpoint", endpoint, "--log", "debug"],
                            env=ambiente, stdout=uscita, stderr=subprocess.STDOUT)


def rpc(metodo, timeout=60, **params):
    with CoreClient(endpoint, timeout=timeout) as c:
        return c.request(metodo, params)


def capacita(nome, **args):
    return rpc("capabilities/call", name=nome, args=args)


def errore(nome, **args):
    try:
        capacita(nome, **args)
    except Exception as e:                                        # noqa: BLE001
        return str(e)
    return ""


try:
    scadenza = time.time() + 20
    while time.time() < scadenza and processo.poll() is None:
        if CoreClient.disponibile(endpoint):
            break
        time.sleep(0.3)
    else:
        print("  il demone non ha risposto:", registro.read_text(errors="replace")[-400:])
        processo.kill()
        sys.exit(2)

    print("\n1. il pannello chiede le voci")
    v = capacita("voce.live.voci")
    nomi = [x["nome"] for x in v.get("voci", [])]
    controlla("ci sono tutte e trenta, ognuna col suo carattere",
              len(nomi) == 30 and len(set(nomi)) == 30
              and all(x["it"] and x["en"] for x in v["voci"]), str(nomi))
    controlla("con quella scelta e quella di serie",
              v.get("scelta") == "Charon" and v.get("predefinita") == "Kore", str(v)[:200])
    controlla("e sa che la chiave viene dal pannello, senza dirla",
              v.get("chiave") == "pannello" and CHIAVE not in json.dumps(v), str(v)[:200])
    controlla("e che e' pronta", v.get("pronta") is True and v.get("perche_no") is None, str(v)[:200])

    print("\n2. «Ascolta»: la voce si presenta, dal vivo")
    r = capacita("voce.live.prova", voce="puck")
    c = collegamenti[-1] if collegamenti else {}
    setup = c.get("apertura", {}).get("setup", {})
    controlla("il server ha avuto la voce chiesta, scritta giusta",
              setup.get("generationConfig", {}).get("speechConfig", {}).get("voiceConfig", {})
              .get("prebuiltVoiceConfig", {}).get("voiceName") == "Puck", json.dumps(setup)[:300])
    controlla("e il modello di Live", setup.get("model") == "models/gemini-3.8-live",
              str(setup.get("model")))
    domanda = (c.get("domanda", {}).get("clientContent", {}).get("turns") or [{}])[0] \
        .get("parts", [{}])[0].get("text", "")
    controlla("le si chiede di presentarsi col suo nome", "Puck" in domanda, domanda)
    query = parse_qs(urlparse(c.get("percorso", "")).query)
    controlla("la chiave va nell'indirizzo", query.get("key") == [CHIAVE], c.get("percorso", ""))
    controlla("e non nei messaggi", CHIAVE not in json.dumps(c.get("apertura", {})))
    controlla("torna cosa ha detto e quanto ha parlato",
              r.get("voce") == "Puck" and r.get("detto") == "Ciao, sono Puck."
              and r.get("secondi") == 0.5 and isinstance(r.get("primo_audio_ms"), int), str(r))
    controlla("e se l'altoparlante non c'e' lo dice, invece di fallire",
              r.get("suonato") is True or bool(r.get("perche_non_suonato")), str(r))

    print("\n3. una voce che non c'e' diventa quella di serie")
    capacita("voce.live.prova", voce="Inesistente")
    nome = (collegamenti[-1].get("apertura", {}).get("setup", {}).get("generationConfig", {})
            .get("speechConfig", {}).get("voiceConfig", {}).get("prebuiltVoiceConfig", {})
            .get("voiceName"))
    controlla("Kore", nome == "Kore", str(nome))

    print("\n4. con «solo sul PC» la voce non esce")
    configura(solo_locale=True)
    prima = len(collegamenti)
    e = errore("voce.live.prova", voce="Puck")
    controlla("la prova dice di no, e perche'", "solo sul PC" in e, e)
    time.sleep(0.3)
    controlla("e non si e' collegata a niente", len(collegamenti) == prima, str(len(collegamenti)))
    v = capacita("voce.live.voci")
    controlla("le voci dicono che non e' pronta, col perche'",
              v.get("pronta") is False and "solo sul PC" in (v.get("perche_no") or ""), str(v)[:300])

    print("\n5. senza chiave non parte")
    configura(chiave="")
    e = errore("voce.live.prova", voce="Puck")
    controlla("la prova chiede la chiave", "manca la chiave" in e, e)
    v = capacita("voce.live.voci")
    controlla("e le voci dicono che non c'e'", v.get("chiave") == "" and v.get("pronta") is False,
              str(v)[:300])
    configura(conversazione="classica")
    v = capacita("voce.live.voci")
    controlla("con la conversazione classica non e' pronta, ma le voci si danno",
              v.get("pronta") is False and "classica" in (v.get("perche_no") or "")
              and len(v.get("voci", [])) == 30, str(v)[:300])

    print("\n6. un modello non le vede, e non le chiama")
    prima = len(collegamenti)
    configura()
    with CoreClient(endpoint, timeout=60) as cc:
        elenco = {t["name"] for t in cc.request("tools/list")["tools"]}
        chiamata = cc.request("tools/call", {"name": "voce_live_prova", "arguments": {"voce": "Puck"}})
    controlla("non stanno nell'elenco dei modelli",
              "voce_live_prova" not in elenco and "voce_live_voci" not in elenco
              and "voce_parla" in elenco, str(sorted(x for x in elenco if x.startswith("voce"))))
    controlla("e chiamarla lo stesso non apre niente",
              chiamata.get("isError") and "la usa la persona" in chiamata["content"][0]["text"]
              and len(collegamenti) == prima, repr(chiamata)[:200])
finally:
    if processo.poll() is None:
        processo.kill()
        processo.wait(10)
    uscita.close()
    ascolto.close()

log = registro.read_text(encoding="utf-8", errors="replace")
print("\n7. la chiave non finisce nei log")
controlla("il registro c'e' e ha parlato", len(log) > 0, str(len(log)))
controlla("e la chiave non c'e'", CHIAVE not in log, "")

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
if falliti:
    print("::error::test_demone_live: " + "; ".join(falliti))
sys.exit(1 if falliti else 0)
