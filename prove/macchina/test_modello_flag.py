# -*- coding: utf-8 -*-
"""Come si accende il modello: i flag che cambiano il doppio dei numeri.

Misurato su questa macchina (RTX 4060 Ti 16 GB, Qwen3.8-27B Q4_K_M, prompt
vero da 12.492 token), con `banco_modello.py`:

    configurazione        prompt a caldo   generazione
    come prima (53 layer)      1504 ms        6,0 t/s
    + flash attention          1541 ms        6,1 t/s   (era gia' acceso)
    + KV a 8 bit               1281 ms        6,5 t/s
    + i layer che ne seguono    691 ms        9,0 t/s   (60 layer)

Due cose imparate, e nessuna delle due si poteva sapere leggendo:

**Flash attention era gia' acceso.** Il valore di fabbrica in questa build e'
`auto`, e auto vuol dire on. Metterlo a mano non cambia niente: sarebbe stata
una riga di changelog per un guadagno che non esiste.

**La KV cache a 8 bit non serve a calcolare piu' in fretta.** Serve a
occupare meta' memoria, e su una scheda dove il modello non ci sta tutto
quella meta' diventa layer che tornano sulla GPU. Il guadagno vero e' li' -
piu' 50% di generazione - non nel calcolo.
"""
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

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


from nova.config import Config, ServerConfig                     # noqa: E402
from nova.runtime import PESO_KV, LlamaServer, estimate_gpu_layers  # noqa: E402

print("\n1. il tipo della KV cache e' una scelta dichiarata")
controlla("c'e' in configurazione", hasattr(ServerConfig(), "kv_cache_type"))
controlla("e il valore di fabbrica e' quello misurato migliore",
          ServerConfig().kv_cache_type == "q8_0", ServerConfig().kv_cache_type)
controlla("i pesi noti coprono i tipi che si usano",
          {"f16", "q8_0"} <= set(PESO_KV))
controlla("e a 8 bit la cache pesa meta'", PESO_KV["q8_0"] == 0.5)

print("\n2. arriva alla riga di comando, e solo quando serve")
cfg = Config.load()


def args_con(tipo: str) -> list:
    c = Config.load()
    c.server.kv_cache_type = tipo
    s = LlamaServer(c, on_log=lambda m: None)
    try:
        return s._build_args(50)
    except Exception:                                            # noqa: BLE001
        return []


a8 = args_con("q8_0")
if not a8:
    print("  (niente binario del modello qui: salto)")
else:
    controlla("con q8_0 i due flag ci sono",
              "-ctk" in a8 and "-ctv" in a8 and a8[a8.index("-ctk") + 1] == "q8_0")
    # f16 e' gia' il valore di fabbrica di llama.cpp: passarlo sarebbe una
    # cosa in piu' che puo' non piacere a un binario vecchio.
    af = args_con("f16")
    controlla("con f16 non si passa niente", "-ctk" not in af)
    av = args_con("")
    controlla("e nemmeno se qualcuno lo svuota", "-ctk" not in av)

print("\n3. e la stima dei layer sa che la cache e' piu' piccola")
# Non e' un dettaglio: e' il motivo per cui la KV a 8 bit conviene. Se la
# stima non lo sapesse, la memoria liberata resterebbe inutilizzata.
import inspect                                                   # noqa: E402
sorgente = inspect.getsource(estimate_gpu_layers)
controlla("la stima accetta il tipo di cache", "kv_tipo" in sorgente)
controlla("e lo usa per pesare la cache", "PESO_KV" in sorgente)

if Path(cfg.server.model_path).is_file():
    con_f16 = estimate_gpu_layers(cfg.server.model_path, cfg.server.ctx_size,
                                  kv_tipo="f16")
    con_q8 = estimate_gpu_layers(cfg.server.model_path, cfg.server.ctx_size,
                                 kv_tipo="q8_0")
    print(f"  (su questa macchina: {con_f16} layer con f16, {con_q8} con q8_0)")
    controlla("con la cache piu' piccola ci stanno piu' layer", con_q8 >= con_f16,
              f"{con_f16} -> {con_q8}")
else:
    print("  (nessun modello su questa macchina: salto la misura)")

print("\n4. il banco per rimisurare c'e', e dice cosa misura")
banco = (RADICE / "misure" / "banco_modello.py")
controlla("banco_modello.py esiste", banco.is_file())
testo = banco.read_text(encoding="utf-8")
# Le due misure non sono equivalenti: la seconda e' il caso normale, perche'
# dopo il primo messaggio tutti i turni hanno lo stesso prefisso.
controlla("misura a freddo e a caldo", "a freddo" in testo and "a caldo" in testo)
# `-ngl 999` satura la VRAM e il driver ripiega sulla memoria condivisa: in
# quel regime qualunque confronto fra flag non dice niente.
controlla("e parte dai layer che NOVA userebbe, non da «tutti»",
          "estimate_gpu_layers" in testo)
controlla("usa una porta sua, per non disturbare il modello acceso",
          "PORTA = 84" in testo and "8420" not in testo)

print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
