# NOVA - assistente digitale locale per Windows

NOVA fa girare un modello GGUF **in locale** (llama.cpp, avviato e gestito
dall'app stessa: LM Studio non serve) e gli da' **mani vere** sul PC:
filesystem, applicazioni, finestre, PowerShell e web.

**Nessuna visione.** NOVA non guarda lo schermo: agisce tramite API di
sistema e comandi, che e' piu' veloce, deterministico e non consuma contesto
in immagini.

## Avvio rapido

```powershell
cd C:\Users\giova\NOVA
.\install.ps1                     # dipendenze + configurazione + avvio automatico
.\install.ps1 -WithCudaRuntime    # come sopra, scaricando anche llama.cpp CUDA
.\get_cuda_runtime.ps1            # solo il runtime CUDA

python -m nova                    # interfaccia grafica
python -m nova --cli              # modalita' testuale
python -m nova --ask "elenca i file sul desktop"
python -m nova --list-tools       # tutti i tool e il loro livello di rischio
python -m nova --reconfigure      # ririleva modello e runtime
```

## Architettura

```
run_nova.pyw          avvio silenzioso (usato dall'autostart)
install.ps1           installazione, runtime CUDA, avvio automatico, collegamento
nova/
  main.py             entrypoint, GUI o CLI
  config.py           configurazione persistente (%APPDATA%\NOVA\config.json)
  setup_wizard.py     rilevamento automatico di modello GGUF e runtime
  runtime.py          avvia/sorveglia/spegne llama-server.exe (+ auto-tuning GPU)
  agent.py            ciclo agente: modello <-> tool, sicurezza, approvazioni
  tools/
    base.py           registry, schemi OpenAI, livelli di rischio
    files.py          leggere, scrivere, cercare, spostare, aprire
    apps.py           avviare app, elencare/focalizzare/chiudere finestre
    shell.py          PowerShell, CMD, Python
    web.py            ricerca web, lettura pagine, apertura nel browser
    system.py         appunti, tasti, volume, notifiche, promemoria, info PC
  ui/main_window.py   finestra chat + registro azioni + tray + hotkey
  voice/              fase 2: STT (faster-whisper) e TTS (SAPI)
```

## Livelli di autonomia

Impostabili al volo dal menu in alto a destra (o in `config.json`):

| Livello | Comportamento |
|---|---|
| `always_ask` | conferma per **ogni** azione, anche le sole letture |
| `ask_risky` | conferma solo per le azioni `DANGEROUS` (shell, delete, chiusura app, tasti) |
| `autonomous` | nessuna conferma, tutto tracciato nel registro azioni |

Ogni tool e' classificato `SAFE` / `MODERATE` / `DANGEROUS`. Oltre
all'autonomia valgono sempre due guardie non aggirabili dal modello:

- `safety.protected_paths` - percorsi mai scrivibili (Windows, Program Files, ...)
- `safety.forbidden_command_patterns` - regex di comandi bloccati (format, diskpart, ...)
- `safety.write_roots` - se valorizzato, le scritture sono confinate a quelle cartelle

## Runtime del modello

All'avvio NOVA cerca un `llama-server.exe`, in quest'ordine:

1. `NOVA\runtime\` (build CUDA scaricata da `get_cuda_runtime.ps1`)
2. i backend gia' presenti in `%USERPROFILE%\.lmstudio\extensions\backends`
3. `LLAMA_CPP_HOME`

Poi lancia il server come processo figlio e lo spegne alla chiusura. Se il
modello non entra in VRAM, `auto_tune_gpu_layers` riprova da solo scalando i
layer offloadati finche' non parte.

## Aggiungere un tool

```python
from nova.tools.base import Risk, tool

@tool(
    "invia_email",
    "Invia una email tramite Outlook.",
    {"to": {"type": "string", "description": "Destinatario"},
     "subject": {"type": "string", "description": "Oggetto"},
     "body": {"type": "string", "description": "Testo"}},
    Risk.DANGEROUS, category="mail",
    preview=lambda a: f"Invia email a {a['to']}: {a['subject']}",
)
def invia_email(to: str, subject: str, body: str) -> str:
    ...
    return "Email inviata."
```

Importa il modulo in `nova/tools/__init__.py` e il modello lo vede subito.

## Fase 2 - comandi vocali

`nova/voice/` e' gia' predisposto: `stt.py` (faster-whisper, push-to-talk o
wake word) e `tts.py` (SAPI di Windows, zero dipendenze). Per attivarli:

```powershell
pip install faster-whisper sounddevice
```

poi `voice.enabled = true` in `config.json`.

## Prestazioni e tuning

NOVA calcola da sola quanti layer entrano in VRAM (`nova/gguf.py` legge i
metadati del modello, `estimate_gpu_layers` li confronta con la VRAM libera).
Serve perche' su Windows, quando la VRAM finisce, il driver NVIDIA ripiega in
silenzio sulla memoria condivisa: il modello parte lo stesso ma va ~10x piu'
lento.

Misure su RTX 4060 Ti 16 GB con Qwen3.8-27B Q4_K_M (15,7 GB):

| Configurazione | Layer su GPU | Generazione |
|---|---|---|
| CUDA, `-ngl 99` (VRAM saturata) | 65 | ~2 t/s, prompt 40 t/s |
| Vulkan, auto | 56 | ~8 t/s |
| CUDA, auto (stima VRAM) | 53 | ~7-9 t/s |

Un 27B a Q4 su 16 GB non ci sta interamente: circa 12 layer restano sulla CPU
ed e' quello il collo di bottiglia. Per andare molto piu' veloci ci sono due
strade, entrambe a un cambio di riga in `config.json`:

- un quant piu' piccolo dello stesso modello (Q3_K_M ~12,5 GB entra tutto in
  VRAM: 3-4x piu' veloce, qualita' leggermente inferiore);
- un modello piu' piccolo (8-14B) come "cervello rapido" per i comandi
  quotidiani, tenendo il 27B per i compiti complessi.

Per forzare un valore a mano: `server.n_gpu_layers` in `config.json`
(qualsiasi valore < 99 disattiva la stima automatica).

---

## Memoria: knowledge base a grafo

NOVA ha una memoria a lungo termine che sopravvive alle sessioni: un **vault
markdown in `NOVA\vault`, apribile in Obsidian cosi' com'e'** (frontmatter +
`[[wikilink]]`, quindi la vista a grafo di Obsidian funziona senza plugin).

La pipeline di retrieval e' il porting Python di
`knowledge-lab/backend/src/retrival`:

```
query
  1. bypass codice esatto      slug o tag identico -> boost
  2a. sparse  (BM25)           titolo x2.5, tag x2.0
  2b. dense   (embedding)      similarita' coseno
  3. RRF fusion (k=60)         un solo ordinamento
  4. filtro                    PRIMA del taglio a top-K, mai a valle
  5. espansione grafo 1-hop    i vicini dei migliori, ri-filtrati
  6. taglio a top-K
  7. audit                     vault\.nova\audit.jsonl
```

### Struttura

```
nova/kb/
  schema.py     nodo + frontmatter (parser proprio, come nodeLoader.ts)
  store.py      vault su disco, indice, relazioni bidirezionali, dedup, audit
  retrieval.py  BM25 + embedder + RRF + espansione grafo + KBEngine
  memory.py     apprendimento automatico dalle conversazioni
  seed.py       mappatura iniziale del PC
nova/kb_setup.py  regia: vault + motore + memoria
nova/tools/kb.py  i 6 tool con cui il modello usa la memoria
```

### Il vault

```
vault/
  _INDICE.md          hub di navigazione, rigenerato a ogni scrittura
  01-profilo/         profilo utente, preferenze
  02-persone/         collaboratori (dedotti dai co-autori git)
  03-progetti/        un nodo per repo o cartella di lavoro
  04-ambiente/        hardware, app installate, modelli, runtime
  05-abitudini/
  06-fatti/           tutto il resto
  .nova/audit.jsonl   ogni ricerca e ogni scrittura, con timestamp
```

Ogni nodo porta `origine` (`scansione` | `auto` | `utente`) e `confidenza`:
si distingue sempre cio' che NOVA ha dedotto da cio' che le hai detto tu.
Un fatto confermato una seconda volta alza la propria confidenza; `utente`
vince sempre su `auto`.

### Come impara

- **Seed**: alla prima esecuzione mappa profilo, progetti, ambiente e persone.
- **Automatico**: dopo ogni scambio un thread in background estrae i fatti
  *durevoli* (preferenze, progetti, persone, decisioni) e li scrive. Non
  memorizza richieste una tantum, output di comandi o orari.
- **Esplicito**: i tool `kb_note`, `kb_link`, `kb_forget` quando dici
  "ricordati che...".
- **Iniezione**: prima di ogni turno i nodi rilevanti finiscono nel prompt di
  sistema, cosi' NOVA non ti richiede cose che gia' sa.

### Tool esposti al modello

| Tool | Cosa fa |
|---|---|
| `kb_search` | cerca nella memoria (pipeline ibrida completa) |
| `kb_note` | crea o aggiorna un nodo |
| `kb_link` | collega due nodi (grafo non orientato) |
| `kb_neighbors` | esplora i collegamenti di un nodo |
| `kb_forget` | archivia un nodo superato (il file resta sul disco) |
| `kb_stats` | nodi, tipi, collegamenti, nodi isolati |

### Da riga di comando

```powershell
python -m nova --kb "orario di lavoro"    # interroga la memoria
python -m nova --kb-stats                 # stato della KB
python -m nova --seed-kb                  # ri-mappa il PC (idempotente)
```

### Configurazione (`kb` in config.json)

| Chiave | Default | Cosa fa |
|---|---|---|
| `enabled` | `true` | attiva la memoria |
| `vault_path` | `NOVA\vault` | dove vivono i nodi |
| `auto_seed` | `true` | mappatura iniziale del PC |
| `auto_learn` | `true` | scrittura automatica dopo ogni scambio |
| `inject_context` | `true` | iniezione del contesto prima del turno |
| `top_k` | `5` | quanti nodi entrano nel prompt |
| `min_confidence` | `0.25` | sotto questa soglia un nodo non viene usato |
| `embedder` | `hash` | `hash` (offline) oppure `llama` |
| `embedder_url` | `:8421` | secondo llama-server con un modello di embedding |

Con `embedder: "llama"` NOVA usa un vero modello di embedding servito su
un'altra porta (es. `nomic-embed-text`), guadagnando sulle riformulazioni.
Se non risponde, ricade da sola sull'embedder locale: la KB non si rompe mai
per colpa di un server spento.

## Nota sul ragionamento

Qwen3.5 e' un modello *thinking*: lasciato libero produce 1000+ token di
ragionamento per turno, che a 7 t/s significa due minuti di attesa. Per questo
il server parte con `--reasoning-budget 512`. Alzalo in
`server.extra_args` se preferisci risposte piu' ragionate e piu' lente,
mettilo a `0` per disattivare del tutto il ragionamento.
