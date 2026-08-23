# NOVA

**Un esperto seduto accanto a te, dentro il tuo PC.**

NOVA non e' una chat che da' consigli: apre programmi, compila moduli, scrive
file, esegue comandi. E lo fa **senza rubarti il posto** — lavora in una
finestra sua, agendo sull'albero di accessibilita' invece che su mouse e
tastiera, cosi' puoi continuare a lavorare mentre lei fa il suo pezzo.

[![ci](https://github.com/CastermustOfficial/NOVA/actions/workflows/ci.yml/badge.svg)](https://github.com/CastermustOfficial/NOVA/actions/workflows/ci.yml)
[![licenza: MIT](https://img.shields.io/badge/licenza-MIT-blue.svg)](LICENSE)

> **Stato: alpha.** Funziona sulla macchina di chi la sviluppa. Se la provi,
> aspettati spigoli — e aprine una issue, che e' il modo piu' utile di aiutare.

## Cosa sa fare

- **Agisce sul sistema**: file, applicazioni, finestre, PowerShell, web.
- **Non ti interrompe**: usa l'accessibilita', non input sintetici. Puo'
  operare su una finestra in secondo piano mentre tu scrivi in un'altra.
- **Ti ascolta**: chiamala per nome e parla; capisce da sola quando la
  conversazione e' finita.
- **Ricorda**: una memoria a grafo di cio' che impara sul tuo PC e sul tuo
  lavoro. Resta sul tuo disco.
- **Custodisce le credenziali**: archivio cifrato con DPAPI, cosi' puo'
  compilare un accesso senza che la password passi mai dal modello.
- **Vede lo stato reale** dell'hardware: ti dice se la RAM va piu' piano di
  quanto potrebbe, invece di farti indovinare.

## Installazione

Serve **Python 3.10+**. Non serve ne' Rust ne' Visual Studio: il core arriva
gia' compilato.

```powershell
git clone https://github.com/CastermustOfficial/NOVA.git
cd NOVA
.\install.ps1
```

| Opzione | Cosa fa |
|---|---|
| `.\install.ps1` | installa tutto e configura l'avvio automatico |
| `.\install.ps1 -ConCuda` | scarica anche llama.cpp CUDA, per il modello locale |
| `.\install.ps1 -DaSorgente` | compila il core invece di scaricarlo (serve Rust + MSVC) |
| `.\install.ps1 -SenzaAvvioAuto` | non parte all'accensione |
| `.\install.ps1 -Disinstalla` | toglie avvio automatico e collegamento |

Poi avvia NOVA dal collegamento sul Desktop: comparira' un orb in un angolo
dello schermo. Cliccalo per scrivere, oppure chiamala per nome.

## Il cervello: tre strade

NOVA non e' legata a un modello. Scegli tu chi ragiona:

| Strada | Per chi | Nota |
|---|---|---|
| **Chiave API** | qualita' massima, si paga a consumo | **consigliata** |
| **Modello locale** | gratuito, offline, privato | serve una GPU decente |
| **CLI di un abbonamento** | utenti avanzati | vedi l'avvertenza sotto |

> **Avvertenza sugli abbonamenti.** Usare la CLI di un abbonamento consumer
> come motore di un'applicazione terza e' fuori dai termini di servizio della
> maggior parte dei fornitori, e il rischio ricade sul tuo account. NOVA
> supporta questa strada perche' e' comoda, ma non e' quella predefinita e non
> te la consiglia.

Il catalogo dei modelli locali sta in [`models.json`](models.json): e' un
dato, non codice, cosi' aggiornare la classifica non richiede una release.

## Permessi

NOVA parte con **«conferma sempre»**: chiede il permesso prima di ogni azione
che tocca il sistema, e la richiesta dice *cosa* sta per fare, non un generico
«consentire operazione?». Puoi allentare il vincolo quando ti fidi — e'
una manopola tua, non una decisione sua.

Quello che resta sul tuo disco e non esce mai: memoria, credenziali,
configurazione. Vivono in `%APPDATA%\NOVA`.

## Documentazione

- [Documento di architettura](docs/architettura.md) — le decisioni prese e il
  perche', comprese quelle scartate.
- [Come contribuire](CONTRIBUTING.md)

## Per chi sviluppa

```powershell
.\build.ps1              # compila il core Rust (release)
.\build.ps1 -Test        # esegue i test Rust
python -m pytest -q       # esegue i test Python
```

Il resto di questo documento e' la documentazione tecnica di dettaglio.

---

## Architettura

```
bin/nova-shell.exe    l'orb e le finestre (avviato all'accensione)
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

---

## Tre cervelli intercambiabili

Quello che *pensa* sta dietro l'astrazione `nova/brains`. Si cambia a caldo
dal menu **Cervello** in alto, senza perdere la conversazione ne' la memoria.

| Cervello | Cos'e' | Agentico |
|---|---|---|
| `locale` | il GGUF servito da llama-server sul tuo PC | no |
| `claude` | Claude Code CLI in headless | si' |
| `api` | qualunque endpoint OpenAI-compatibile | no |

**Agentico** e' la differenza che conta. `locale` e `api` *propongono* tool
call e NOVA li esegue applicando guardie e livelli di autonomia. `claude` ha
mani proprie: NOVA gli fa da tramite, gli passa il contesto e la memoria, e
riporta cosa ha fatto, in quanti turni e quanto e' costato.

```powershell
python -m nova --brains                    # chi c'e' e chi e' pronto
python -m nova --brain claude              # cambia e avvia
python -m nova --brain claude --ask "..."  # una richiesta sola
```

### Claude Code come cervello

Serve `npm install -g @anthropic-ai/claude-code` e un `claude` gia'
autenticato. NOVA:

- lo lancia in headless (`-p --output-format json`), prompt via stdin
- mantiene la sessione fra un turno e l'altro con `--resume <session_id>`
- traduce i **tuoi** livelli di autonomia nei suoi permessi:

  | Autonomia NOVA | `--permission-mode` |
  |---|---|
  | Conferma sempre | `plan` (analizza e propone, non tocca nulla) |
  | Conferma azioni rischiose | `acceptEdits` |
  | Autonomo | `bypassPermissions` |

- gli espone la memoria a grafo come **server MCP** (`nova/mcp_kb.py`), quindi
  Claude usa `mcp__nova__kb_search` e `mcp__nova__kb_note`: stessa pipeline di
  retrieval del modello locale, stesso formato dei nodi. Se l'MCP non parte,
  ricade sulla lettura diretta dei file .md del vault.
- riporta costo e token di ogni turno nel registro azioni.

Attenzione a `brains.claude_model`: l'alias `opus` su CLI datate punta a
`claude-opus-4-1`, che e' stato ritirato e risponde 404. Il default e'
`sonnet`, che funziona.

### API esterna

`brains.api_base_url` + `brains.api_model` + una chiave (in `brains.api_key`
oppure nella variabile d'ambiente indicata da `brains.api_key_env`). Va con
OpenAI, OpenRouter, Groq, Together e chiunque parli lo stesso dialetto. Usa il
ciclo di tool di NOVA, quindi guardie e autonomia restano identiche.

### Cosa esce dal PC

Con `locale` niente, mai. Con `claude` e `api` escono la richiesta, il
contesto della conversazione e i nodi di memoria pertinenti al messaggio: e'
la scelta che rende quei cervelli utili, ma va fatta sapendo cosa comporta.
Il selettore e' li' apposta: per il lavoro sensibile torna su `locale`.

---

## Il modello appartiene al demone

Da quando c'e' `core/` (vedi `core/README.md`), NOVA non genera piu' llama-server
come processo figlio: lo affida a **nova-core**, che lo supervisiona.

```
llama-server pid 2760 -> padre: novad
```

Conseguenze pratiche:

| | prima | adesso |
|---|---|---|
| Chiudi la finestra | il modello si scarica | resta caricato |
| Riapri NOVA | ~2 minuti di caricamento | **2 secondi** |
| Il server cade | resta giu' | il demone lo rialza |
| Log del modello | file che nessuno legge | eventi `proc.output` sul bus, piu' buffer circolare |

All'avvio NOVA prova nella sequenza: *il demone possiede gia' il modello?* →
lo **adotta** (recupera anche con quanti `-ngl` era partito, chiedendolo al
demone); *c'e' qualcosa sulla porta?* → lo riusa; altrimenti chiede a nova-core
di avviarlo, e solo se il demone manca ricade sul vecchio processo figlio. Se
`core/` non e' compilato NOVA funziona esattamente come prima.

Chiavi in `server` di `config.json`:

| Chiave | Default | Cosa fa |
|---|---|---|
| `use_daemon` | `true` | affida il modello a nova-core |
| `daemon_autostart` | `true` | accende nova-core se non gira |
| `stop_model_on_exit` | `false` | chiudere NOVA **non** scarica il modello |

La finestra si sottoscrive a `proc.*` e mostra nel registro azioni i log del
modello presi dal bus, non piu' da un processo che possiede lei.

---

## Chi risponde a cosa: il router

Il modello locale **orchestra**. È gratis, è privato, sta già in VRAM, e per
capire cosa vuoi e chiamare i tool giusti basta e avanza. Quando il compito lo
supera non ci prova lo stesso: passa la palla e riprende in mano il risultato.

I gradini sono in `brains.routing.tiers` di `config.json`, in ordine di
potenza. Quelli predefiniti:

| Gradino | Cervello | Modello | Quando |
|---|---|---|---|
| `locale` | GGUF sul PC | Qwen3.8-27B | orchestrazione e compiti semplici |
| `standard` | Claude Code | `sonnet` | il cavallo da lavoro |
| `difficile` | Claude Code | `claude-opus-4-5-20251101` | quando il compito lo merita |
| `alternativo` | Gemini CLI | `gemini-2.5-pro` | seconda opinione |

```powershell
python -m nova --modelli     # gradini, stato, speso / tetto
```

### Come passa la palla

Tre strade, in ordine di intelligenza:

1. **`delega`** — il modello sceglie. Scrive il compito per intero (chi lo
   riceve non vede la conversazione) e passa i **percorsi** dei file in `file`:
   li allega NOVA, gratis. Poi riprende lui con la risposta.
2. **Escalation automatica** — se NOVA sbaglia due volte di fila, o fa sei
   chiamate senza arrivare a una risposta, sale di gradino da sola e infila il
   risultato nella conversazione. Sono due modi diversi di non farcela:
   sbattere contro un muro, e girare a vuoto.
3. **`secondo_parere`** — la stessa domanda a due gradini, per confrontare.

### Guardie

| Chiave (`brains.routing`) | Default | Cosa fa |
|---|---|---|
| `orchestratore` | `locale` | chi guida la conversazione |
| `escalation_automatica` | `true` | sale da sola quando serve |
| `fallimenti_prima_di_salire` | `2` | tentativi andati male |
| `passi_prima_di_salire` | `6` | chiamate senza risposta |
| `salite_massime` | `1` | quante volte per turno |
| `tetto_usd_sessione` | `5.0` | oltre, le deleghe a pagamento si fermano |
| `solo_locale` | `false` | `true` = niente esce dal PC, punto |

### Aggiungere un modello senza scrivere codice

Le CLI agentiche esterne si dichiarano in `brains.cli`; poi si citano in un
gradino. `{model}` viene sostituito.

```json
"cli": {
  "deepseek": {
    "etichetta": "DeepSeek",
    "binary": "deepseek",
    "args": ["--model", "{model}"],
    "model": "deepseek-reasoner",
    "prompt": "stdin"
  }
}
```

### Numeri misurati

| | tempo | costo |
|---|---|---|
| `standard` (Sonnet), domanda secca | 7,1 s | 0,016 $ |
| `alternativo` (Gemini), domanda secca | 21,7 s | 0 $ |
| `difficile` (Opus), review di un file da 300 righe | 112,7 s | **0,89 $** |

Opus costa: con il tetto a 5 $ ci stanno cinque review come quella. È il motivo
per cui l'orchestratore è il locale e non lui.

### Cosa ha insegnato la prova

Alla prima versione il modello locale **non delegava**: davanti a «critica
architetturale severa di questo file» ha fatto dieci chiamate di tool per
raccogliere contesto senza mai passare la palla. Due correzioni:

- il prompt ora elenca i casi concreti in cui delegare *subito* (giudicare
  codice, progettare, ragionamenti lunghi, molti file insieme) invece di dire
  genericamente «se ti supera»;
- l'escalation automatica guarda anche il numero di passi, non solo i
  fallimenti — perché girare a vuoto è l'altro modo di non farcela.

Dopo le correzioni, con la stessa richiesta: legge il file, annuncia
«ora delego la critica a un modello più capace», sceglie **`difficile`** da
solo e motiva — *«richiede ragionamento fine su race condition tokio e
correctness concorrente; supera le mie possibilità di analisi affidabile»* —
allega i file e riprende il controllo con la risposta.

## Lo screenshot è un accessorio

C'è un tool `screenshot`, e serve per le domande sull'aspetto delle cose
(«che ne pensi di questa interfaccia?»). **Non è una fondamenta**: per *agire*
su un'applicazione NOVA usa l'albero di accessibilità, che è preciso,
istantaneo e non costa niente. Dare la vista a un modello per fargli premere
un pulsante è lento e caro; averla per esprimere un giudizio è un di più.

### Abbonamento, non spesa

NOVA legge `~/.claude/.credentials.json` e riconosce il tipo di accesso. Su
questo PC:

```
accesso: ('abbonamento', 'max_5x')
```

Con un abbonamento il `total_cost_usd` che Claude Code riporta è un
**equivalente API**: dice quanto pesa una richiesta, non quanto hai speso. Il
tetto in dollari quindi **non si applica** ai gradini coperti da abbonamento —
si applica solo a chi paga a token (`brain: "api"`, oppure una CLI dichiarata
con `"a_consumo": true`).

```
orchestratore: locale   nessun gradino a consumo:
0.0 $ è l'equivalente API, non una spesa

* locale       locale   predefinito                locale       pronto
  standard     claude   sonnet                     abbonamento  pronto
  difficile    claude   claude-opus-4-5-...        abbonamento  pronto
  alternativo  gemini   gemini-2.5-pro             incluso      pronto
```

### Quando finisce la quota

Con l'abbonamento il vincolo vero non sono i soldi, sono i **limiti d'uso**. È
una cosa diversa da un errore: non vuol dire «non ci riesco», vuol dire
«riprova più tardi». NOVA la tratta come tale:

1. riconosce il messaggio di quota esaurita (`usage limit`, `rate limit`, 429, …)
   e solleva `LimiteUso`, non un errore generico;
2. mette **quel gradino in pausa** per il tempo indicato;
3. **ripiega su un altro fornitore** — non su un altro modello dello stesso,
   perché il limite è sul conto, non sul modello — e in ultima istanza torna
   sul locale.

```
«difficile» in pausa per 30 minuti: quota esaurita
«difficile» è a quota: ripiego su «alternativo»
esito finale: da «alternativo»
motivo: prova (ripiego: «difficile» a quota)
```

Si disattiva con `ripiego_su_limite: false`, se preferisci che si fermi e te lo
dica invece di cambiare modello da solo.
