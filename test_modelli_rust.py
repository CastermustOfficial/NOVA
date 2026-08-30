# -*- coding: utf-8 -*-
"""La ricerca dei modelli in Rust deve trovare quello che trova Python.

Quarto pezzo portato, e quello con la ragione piu' forte per esistere. Il
modulo Python porta scritto in testa che e' «di sola libreria standard, di
proposito: viene eseguito dall'installatore prima che le dipendenze del
progetto siano garantite». E' una circolarita' ammessa in un commento: per
decidere quale modello serve, oggi bisogna gia' avere Python installato. Il
binario Rust la scioglie — cerca, legge e calcola su una macchina appena
accesa.

Il confronto e' su tre cose insieme:

- **quali file** vengono trovati in una cartella finta, **in che ordine**, e
  con quali byte, gigabyte e proiettore accanto;
- **la forma letta dal GGUF**: architettura, strati, contesto — su file finti
  costruiti qui e, se ce n'e' uno vero sul disco, anche su quello;
- **il calcolo degli strati su GPU**, cifra per cifra, su una griglia di casi.

La cartella e' finta di proposito. Un confronto fatto sui modelli veri di
questa macchina proverebbe che i due codici sono d'accordo *qui*; una cartella
costruita apposta prova che sono d'accordo sui casi che contano — il file
rinominato, lo scaricamento a meta', il proiettore accanto al modello, le due
copie dello stesso file in due posti.

Esce 2 — «qui non si puo' provare» — se il binario non e' costruito.
"""
import json
import os
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "banco-modelli.exe" if os.name == "nt" else "banco-modelli"
BINARIO = RADICE / "core" / "target" / "release" / NOME
if not BINARIO.is_file():
    print("Il banco Rust non e' costruito per questo sistema. Per averlo:")
    print("  cd core && .\\x.cmd build --release -p nova-modelli "
          "--features banco --bin banco-modelli")
    sys.exit(2)

from nova import modelli_trova as py            # noqa: E402
from nova.gguf import misura, model_shape       # noqa: E402
from nova.runtime import PESO_KV                # noqa: E402

# Un'asimmetria che il porting ha fatto uscire allo scoperto, e che vale la
# pena scrivere: `trova(extra=[...])` in Python **aggiunge** le cartelle note
# a quelle indicate — non esiste un modo di dire «guarda solo qui». Su questa
# macchina significa che il lato Python trovava anche i sei modelli veri di
# LM Studio mentre il lato Rust vedeva solo la cartella finta, e i due elenchi
# non potevano coincidere. In Rust le radici sono un dato che si passa; in
# Python sono cablate dentro la funzione. Qui si zittiscono per la durata
# della prova, ma la differenza resta da sanare quando il Python andra' via.
py.cartelle_note = lambda: []

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


# --- Un GGUF finto, ma con l'intestazione vera -------------------------
# I primi quattro byte e la tabella dei metadati sono quelli veri: e' cio' che
# i due lettori leggono davvero. Il corpo e' riempimento, perche' nessuno dei
# due legge i tensori.

def _str(s: bytes) -> bytes:
    return struct.pack("<Q", len(s)) + s


def gguf_finto(percorso: Path, arch: str, strati: int, ctx: int, embd: int,
               nome: str = "prova", byte_totali: int = 0,
               con_vocabolario: bool = True, tensori: int = 3,
               taglia_a: float = 1.0) -> None:
    """Un GGUF finto con la tabella dei tensori vera.

    `taglia_a` sotto 1 lo tronca: e' lo scaricamento a meta', che ha
    l'intestazione giusta e i dati no.
    """
    kv = [
        (b"general.architecture", 8, _str(arch.encode())),
        (b"general.name", 8, _str(nome.encode())),
        (f"{arch}.block_count".encode(), 4, struct.pack("<I", strati)),
        (f"{arch}.context_length".encode(), 4, struct.pack("<I", ctx)),
        (f"{arch}.embedding_length".encode(), 4, struct.pack("<I", embd)),
    ]
    if con_vocabolario:
        # Il pezzo che costa: un vettore di stringhe che nessuno dei due deve
        # tenere in memoria. Piccolo qui, ma la strada percorsa e' la stessa.
        voci = b"".join(_str(f"tok{i}".encode()) for i in range(500))
        kv.append((b"tokenizer.ggml.tokens", 9,
                   struct.pack("<I", 8) + struct.pack("<Q", 500) + voci))
    corpo = b"GGUF" + struct.pack("<I", 3) + struct.pack("<Q", tensori)
    corpo += struct.pack("<Q", len(kv))
    for chiave, tipo, val in kv:
        corpo += _str(chiave) + struct.pack("<I", tipo) + val
    # La tabella dei tensori: nome, dimensioni, tipo, scostamento. E' su
    # questa che si capisce se il file arriva fin dove dice di arrivare.
    intero = max(byte_totali, len(corpo) + 4096)
    passo = (intero - len(corpo)) // (tensori + 1)
    for i in range(tensori):
        corpo += _str(f"blk.{i}.weight".encode())
        corpo += struct.pack("<I", 2) + struct.pack("<Q", 64) + struct.pack("<Q", 64)
        corpo += struct.pack("<I", 0)                 # F32
        corpo += struct.pack("<Q", i * passo)         # scostamento
    dati = max(0, intero - len(corpo))
    corpo += b"\0" * int(dati * taglia_a)
    percorso.write_bytes(corpo)


def rust(dentro: dict) -> dict:
    r = subprocess.run([str(BINARIO)], input=json.dumps(dentro),
                       capture_output=True, text=True, encoding="utf-8",
                       timeout=240)
    if r.returncode != 0:
        raise RuntimeError(f"banco uscito {r.returncode}: {r.stderr[:400]}")
    return json.loads(r.stdout)


# =======================================================================
print("\n=== La cartella finta ===")

MINIMO = 20_000        # sopra la briciola, sotto il piu' piccolo dei finti

with tempfile.TemporaryDirectory(prefix="nova-modelli-") as tmp:
    base = Path(tmp)
    (base / "unsloth" / "Qwen3.8-27B-GGUF").mkdir(parents=True)
    (base / "unsloth" / "gemma-4-26B-GGUF").mkdir(parents=True)
    (base / "Downloads").mkdir()
    (base / "node_modules" / "roba").mkdir(parents=True)
    (base / "vuota").mkdir()

    q = base / "unsloth" / "Qwen3.8-27B-GGUF" / "Qwen3.8-27B-UD-Q4_K_XL.gguf"
    gguf_finto(q, "qwen3", 64, 262144, 5120, "Qwen3.8 27B", byte_totali=90_000)

    # Il proiettore accanto: si trova, non si elenca come modello.
    gguf_finto(base / "unsloth" / "Qwen3.8-27B-GGUF" / "mmproj-F16.gguf",
               "clip", 24, 0, 1152, "proiettore", byte_totali=20_000)

    g = base / "unsloth" / "gemma-4-26B-GGUF" / "gemma-4-26B-A4B-it-UD-IQ4_NL.gguf"
    gguf_finto(g, "gemma3", 62, 131072, 4608, "Gemma 4 26B", byte_totali=70_000)

    # Uno sconosciuto: vale zero di punteggio, ma non sparisce.
    m = base / "Downloads" / "mistral-7b.gguf"
    gguf_finto(m, "llama", 32, 32768, 4096, "Mistral", byte_totali=50_000)

    # Un file rinominato: estensione giusta, primi byte sbagliati. Deve
    # sparire con la verifica accesa e comparire con la verifica spenta.
    bugiardo = base / "Downloads" / "sedicente.gguf"
    bugiardo.write_bytes(b"<!DOCTYPE html>" + b"\0" * 50_000)

    # Sotto la soglia: non e' un modello, e' un pezzo di qualcosa.
    piccolo = base / "Downloads" / "briciola.gguf"
    gguf_finto(piccolo, "llama", 4, 512, 128, "briciola")

    # Dentro node_modules: non ci si entra nemmeno.
    gguf_finto(base / "node_modules" / "roba" / "qwen-nascosto.gguf",
               "qwen3", 8, 4096, 512, "nascosto", byte_totali=60_000)

    comune = {"radici": [str(base)], "profondita": 8, "secondi": 60.0,
              "minimo": MINIMO}

    fuori = rust({**comune, "verifica": True})
    attesi = py.trova(extra=[base], secondi=60.0, minimo=MINIMO, verifica=True)

    def snello(m):
        return (m["percorso"].lower(), m["nome"], m["byte"],
                round(m["gb"], 1), m["proiettore"].lower())

    a = [snello(x) for x in attesi]
    b = [snello(x) for x in fuori["modelli"]]
    controlla("stesso elenco, stesso ordine", a == b, f"\n    py={a}\n    rs={b}")
    # Tre su sette file .gguf nella cartella: il proiettore, il rinominato,
    # la briciola sotto soglia e quello dentro node_modules non sono modelli.
    controlla("tre modelli su sette file", len(b) == 3, f"trovati {len(b)}")

    nomi = {x[1] for x in b}
    controlla("il proiettore non e' un modello", "mmproj-F16.gguf" not in nomi)
    controlla("il file rinominato sparisce", "sedicente.gguf" not in nomi)
    controlla("sotto soglia non entra", "briciola.gguf" not in nomi)
    controlla("node_modules non si attraversa", "qwen-nascosto.gguf" not in nomi)
    controlla("qwen viene per primo", b[0][1].lower().startswith("qwen3.8"))
    controlla("gemma prima di mistral",
              [x[1] for x in b].index(g.name) < [x[1] for x in b].index(m.name))

    proi = {x[1]: x[4] for x in b}
    controlla("il proiettore sta accanto al modello",
              proi[q.name].endswith("mmproj-f16.gguf"), proi[q.name])
    controlla("chi non ce l'ha ha la stringa vuota", proi[m.name] == "")

    # --- verifica spenta: entra anche il bugiardo ----------------------
    fuori2 = rust({**comune, "verifica": False})
    attesi2 = py.trova(extra=[base], secondi=60.0, minimo=MINIMO, verifica=False)
    a2 = [snello(x) for x in attesi2]
    b2 = [snello(x) for x in fuori2["modelli"]]
    controlla("senza verifica, stesso elenco", a2 == b2, f"\n    py={a2}\n    rs={b2}")
    controlla("senza verifica il bugiardo entra",
              any(x[1] == "sedicente.gguf" for x in b2))

    # --- profondita': con 1 livello non si arriva ai modelli -----------
    poco = rust({**comune, "profondita": 1})
    poco_py = py.trova(extra=[base], secondi=60.0, minimo=MINIMO)
    controlla("la profondita' e' un tetto vero",
              len(poco["modelli"]) < len(poco_py),
              f"a un livello: {len(poco['modelli'])}")

    # --- lo stesso file da due radici si conta una volta ---------------
    doppio = rust({**comune, "radici": [str(base), str(base)]})
    controlla("due radici uguali non raddoppiano l'elenco",
              len(doppio["modelli"]) == len(fuori["modelli"]))

    # --- la forma letta dal GGUF ---------------------------------------
    print("\n=== La forma dei modelli ===")
    percorsi = [str(q), str(g), str(m), str(bugiardo), str(base / "non-c-e.gguf")]
    forme = rust({"radici": [], "forme": percorsi})["forme"]
    for percorso, letta in zip(percorsi, forme):
        att = model_shape(percorso)
        nome_breve = Path(percorso).name
        uguale = (
            letta["arch"] == att.get("arch", "")
            and letta["nome"] == att.get("name", "")
            and letta["n_strati"] == int(att.get("n_layers") or 0)
            and letta["n_ctx_train"] == int(att.get("n_ctx_train") or 0)
            and letta["n_embd"] == int(att.get("n_embd") or 0)
        )
        controlla(f"forma di {nome_breve}", uguale, f"\n    py={att}\n    rs={letta}")

    controlla("un file rotto da' una forma vuota, non un errore",
              forme[3]["arch"] == "" and forme[3]["n_strati"] == 0)
    controlla("un file assente da' una forma vuota",
              forme[4]["arch"] == "" and forme[4]["n_strati"] == 0)

    # --- il file indicato a mano ---------------------------------------
    print("\n=== Il file indicato a mano ===")
    indicati = [
        str(q),                       # buono
        f'"{q}"',                     # con le virgolette di «Copia come percorso»
        f"  {q}  ",                   # con gli spazi dell'incollato
        str(bugiardo),                # rinominato
        str(base / "Downloads"),      # una cartella
        str(base / "non-c-e.gguf"),   # non esiste
    ]
    v_rs = rust({"radici": [], "indicati": indicati})["indicati"]
    for indicato, r in zip(indicati, v_rs):
        att = py.verifica_file(indicato)
        uguale = bool(att.get("ok")) == r["ok"] and att.get("motivo", "") == r["motivo"]
        if att.get("ok"):
            uguale = uguale and att["byte"] == r["byte"] and round(att["gb"], 1) == round(r["gb"], 1)
        controlla(f"verifica di {indicato[:44]!r}", uguale,
                  f"\n    py={att}\n    rs={r}")

    controlla("le virgolette non cambiano la risposta",
              v_rs[0]["percorso"] == v_rs[1]["percorso"] == v_rs[2]["percorso"])

# =======================================================================
print("\n=== Lo scaricamento a meta' ===")

# Il difetto trovato su un file vero: l'intestazione GGUF sta all'inizio, e
# uno scaricamento interrotto ce l'ha tutta. Per mesi in tre punti c'era
# scritto che i quattro byte bastavano a riconoscerlo. Non bastano.
with tempfile.TemporaryDirectory(prefix="nova-meta-") as tmp:
    base = Path(tmp)
    sano = base / "sano.gguf"
    meta = base / "a-meta.gguf"
    gguf_finto(sano, "qwen3", 64, 8192, 512, byte_totali=200_000)
    gguf_finto(meta, "qwen3", 64, 8192, 512, byte_totali=200_000, taglia_a=0.4)

    controlla("i quattro byte dicono si' a entrambi",
              py._e_gguf(sano) and py._e_gguf(meta))

    for f, atteso in ((sano, True), (meta, False)):
        m_py = misura(f)
        m_rs = rust({"radici": [], "misure": [str(f)]})["misure"][0]
        controlla(f"misura di {f.name}: py e rs d'accordo",
                  m_py["byte"] == m_rs["byte"]
                  and m_py["byte_minimi"] == m_rs["byte_minimi"]
                  and m_py["tensori"] == m_rs["tensori"]
                  and m_py["completo"] == m_rs["completo"],
                  f"\n    py={m_py}\n    rs={m_rs}")
        controlla(f"{f.name} completo={atteso}", m_py["completo"] is atteso)

    # E la ricerca non lo deve offrire.
    comune2 = {"radici": [str(base)], "profondita": 4, "secondi": 30.0,
               "minimo": 1000}
    rs_el = rust({**comune2, "verifica": True})["modelli"]
    py_el = py.trova(extra=[base], secondi=30.0, minimo=1000, verifica=True)
    controlla("la ricerca non offre il file a meta'",
              [x["nome"] for x in rs_el] == ["sano.gguf"],
              str([x["nome"] for x in rs_el]))
    controlla("e Python fa lo stesso",
              [x["nome"] for x in py_el] == [x["nome"] for x in rs_el],
              f"py={[x['nome'] for x in py_el]}")

    # E chi lo indica a mano deve sentirsi dire perche'.
    v_py = py.verifica_file(str(meta))
    v_rs = rust({"radici": [], "indicati": [str(meta)]})["indicati"][0]
    controlla("il motivo e' lo stesso, parola per parola",
              v_py["motivo"] == v_rs["motivo"] and not v_py["ok"] and not v_rs["ok"],
              f"\n    py={v_py.get('motivo')!r}\n    rs={v_rs['motivo']!r}")
    controlla("e dice quanto manca, non solo che manca",
              "MB" in v_rs["motivo"] and "tensori" in v_rs["motivo"], v_rs["motivo"])


# =======================================================================
print("\n=== Gli strati su GPU ===")

# Una griglia che passa per i punti che il banco ha misurato su questa
# macchina: 53 strati e 60 strati sono due velocita' diverse del cinquanta per
# cento, e la differenza fra le due la decide questo calcolo.
CASI = []
for byte_modello in (4 * 1024**3, 16 * 1024**3, 25 * 1024**3, 60 * 1024**3):
    for n_strati in (0, 32, 62, 64, 80):
        for vram in (0, 2000, 8000, 16376, 24564, 80000):
            for ctx in (512, 8192, 32768, 131072):
                for kv in ("f16", "q8_0", "q4_0", "inventato"):
                    CASI.append({
                        "byte_modello": byte_modello, "n_strati": n_strati,
                        "vram_libera_mb": vram, "ctx": ctx,
                        "riserva_mb": 900.0, "kv_tipo": kv,
                    })


def strati_py(c):
    """La stessa aritmetica di estimate_gpu_layers, senza il disco e la GPU.

    Non si chiama la funzione vera perche' quella legge la dimensione dal
    filesystem e la VRAM da nvidia-smi: due cose che qui si passano da fuori
    apposta, per non confrontare due esecuzioni della stessa macchina in due
    momenti diversi.
    """
    size_mb = c["byte_modello"] / (1024 * 1024)
    n_layers = c["n_strati"]
    free = c["vram_libera_mb"]
    if not n_layers or not free or not c["byte_modello"]:
        return 0
    kv_mb = max(256, c["ctx"] * 0.05) * PESO_KV.get(c["kv_tipo"], 1.0)
    budget = free * 0.96 - c["riserva_mb"] - kv_mb
    per_layer = size_mb / (n_layers + 1)
    if budget <= per_layer:
        return 0
    return max(0, min(n_layers, int(budget // per_layer)))


rs_strati = rust({"radici": [], "strati": CASI})["strati"]
diversi = [(c, strati_py(c), r) for c, r in zip(CASI, rs_strati) if strati_py(c) != r]
controlla(f"{len(CASI)} casi di calcolo, cifra per cifra", not diversi,
          f"\n    primo scarto: {diversi[0] if diversi else ''}")

# I punti misurati sul banco di questa macchina, come promemoria vivo.
mio = {"byte_modello": 15_662_000_000, "n_strati": 64, "vram_libera_mb": 15_800,
       "ctx": 8192, "riserva_mb": 900.0, "kv_tipo": "f16"}
mio_q8 = {**mio, "kv_tipo": "q8_0"}
a, b = strati_py(mio), strati_py(mio_q8)
controlla("la cache a q8_0 regala strati su questa macchina", b > a, f"f16={a} q8={b}")
controlla("Rust dice lo stesso",
          rust({"radici": [], "strati": [mio, mio_q8]})["strati"] == [a, b])

# =======================================================================
print("\n=== I posti dove si guarda ===")
with tempfile.TemporaryDirectory(prefix="nova-casa-") as tmp:
    casa = Path(tmp) / "casa"
    prog = Path(tmp) / "progetto"
    for p in [casa / ".lmstudio" / "models", casa / "Downloads",
              casa / "Desktop", prog / "runtime" / "modelli"]:
        p.mkdir(parents=True)
    (casa / "una-cartella-che-non-conta").mkdir()

    note = rust({"radici": [], "casa": str(casa), "progetto": str(prog),
                 "secondi": 30.0, "minimo": MINIMO})
    # Non ci sono modelli: quel che conta e' che non sia esploso e che abbia
    # guardato solo dove doveva.
    controlla("le cartelle note si visitano senza inventare modelli",
              note["modelli"] == [] and not note["troncato"])

# =======================================================================
print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
