# nova-core

Il demone di NOVA, in Rust. Un processo che **vive nel sistema** invece di una
applicazione che apri: possiede il bus di eventi, il registro delle capacita',
i processi lunghi (llama-server per primo) e il server RPC locale.

Le interfacce — la finestra PyQt, la CLI, la voce, Claude Code — diventano
client sottili: possono morire e ripartire senza fermare NOVA.

## Perche' non un OS

Il vincolo vero e' la portabilita', non il ring 0. Le capacita' che servono
esistono gia' in ogni sistema operativo, esposte in userspace:

| Serve per | Windows | macOS | Linux |
|---|---|---|---|
| Controllare qualsiasi app | UI Automation | Accessibility API | AT-SPI2 |
| Osservare tutto il sistema | ETW | EndpointSecurity | eBPF |
| Annullare cio' che si e' fatto | VSS | snapshot APFS | overlayfs / btrfs |
| Canale locale | named pipe | socket unix | socket unix |

Ogni riga e' la stessa capacita' con tre nomi. Il demone e' costruito attorno
a questa forma: **un trait, tre backend**. L'ultima riga e' gia' implementata
(`server.rs`); le altre tre sono i prossimi strati.

## Struttura

```
core/
  x.cmd                   ambiente di build (attiva vcvars + rustup del progetto)
  smoke.ps1               prova end-to-end: policy, eventi, supervisione
  crates/
    nova-proto/           tipi del protocollo: JSON-RPC, capacita', eventi
    nova-core/            il motore
      bus.rs              broadcast di eventi; i lenti perdono, il demone non rallenta
      capability.rs       registro + trait Capability + schemi JSON
      caps.rs             le capacita' native
      supervisor.rs       processi figli: avvio, riavvio, output come eventi
      policy.rs           divieti non negoziabili (percorsi, comandi)
      config.rs           %APPDATA%\NOVA\core.json
      server.rs           named pipe / socket unix, JSON-RPC + alias MCP
    novad/                il demone
    nova-cli/             il client da riga di comando (`nova`)
```

## Build

Rust e' installato ma fuori dal PATH, e il linker MSVC va attivato: ci pensa
`x.cmd`, che imposta l'ambiente e inoltra tutto a cargo.

```powershell
.\x build --release
.\x test
powershell -ExecutionPolicy Bypass -File smoke.ps1
```

## Uso

```powershell
.\target\release\novad.exe                     # avvia il demone
nova status                                    # stato
nova caps                                      # capacita' disponibili
nova call sys.info
nova call fs.list path=C:\Users hidden=true
nova call service.start name=llama-server      # il modello lo possiede il demone
nova watch "proc.*" "cap.*"                    # eventi in tempo reale
nova shutdown
```

Gli argomenti si passano come `chiave=valore` (il valore e' interpretato come
JSON se possibile), oppure come oggetto JSON completo, oppure da stdin con
`--stdin`. E' una concessione a PowerShell, che maltratta le virgolette quando
chiama un eseguibile nativo.

## Capacita' native

| Capacita' | Rischio | Cosa fa |
|---|---|---|
| `daemon.status` | safe | versione, uptime, figli |
| `sys.info` | safe | OS, architettura, host, utente, core |
| `fs.list` `fs.read` `fs.stat` | safe | lettura del filesystem |
| `fs.write` | moderate | scrittura, soggetta alla policy |
| `shell.exec` | dangerous | PowerShell su Windows, `sh` altrove |
| `proc.spawn` `proc.stop` `proc.list` | dangerous/safe | processi supervisionati |
| `service.list` `service.start` | safe/moderate | servizi da configurazione |
| `bus.publish` | moderate | inietta un evento nel sistema |

Sono poche di proposito. La regola del progetto e' che NOVA non deve avere un
tool per ogni cosa: deve avere **poche primitive universali** — shell,
filesystem, processi — piu' la capacita' di estendersi.

## Policy: cosa sta nel demone e cosa no

Distinzione che conta:

- L'**approvazione** (chiedere all'utente) sta nel client, che ha una faccia.
- I **divieti** stanno qui, applicati dentro il processo che esegue davvero
  l'operazione. Nessun modello e nessun client possono aggirarli.

I divieti sui comandi guardano la *posizione*, non la sottostringa: `diskpart`
a inizio comando e' bloccato, `Get-Date -Format o` no. Una guardia che blocca
i comandi innocui viene disattivata dopo tre giorni, e allora tanto vale non
averla.

## Eventi

Topic gerarchici, sottoscrizione con `*` e `prefisso.*`:

```
daemon.started      daemon.heartbeat
proc.started        proc.output       proc.exited     proc.restarting
cap.called          fs.written        shell.executed
```

L'output di ogni processo supervisionato diventa `proc.output`: i log di
llama-server sono osservabili in tempo reale da qualunque client, invece di
finire in un file che nessuno legge.

## Protocollo

JSON-RPC 2.0 a righe. Metodi: `initialize`, `ping`, `capabilities/list`,
`capabilities/call`, `events/subscribe`, `events/unsubscribe`,
`daemon/status`, `daemon/shutdown`.

Ci sono anche gli alias **MCP** `tools/list` e `tools/call`: con
`nova mcp` (ponte stdio) Claude Code si collega al demone e vede le capacita'
come suoi tool, senza scrivere un adattatore.

Da Python: `nova/core_client.py`.

```python
from nova.core_client import CoreClient
with CoreClient() as c:
    print(c.status())
    c.call("fs.write", path=r"C:\tmp\x.txt", content="ciao")
    c.subscribe("proc.*")
    for ev in c.events():
        print(ev["topic"], ev["data"])
```

## Prossimi strati

1. `nova-platform`: trait `UiTree` con backend UIA / AX / AT-SPI — controllare
   qualunque applicazione senza visione.
2. Osservazione: ETW / EndpointSecurity / eBPF dietro un trait `Observer`.
3. Snapshot: VSS / APFS / overlayfs dietro un trait `Snapshot`, per poter
   osare e tornare indietro.
4. Migrazione progressiva di tool e KB dal lato Python al demone.

---

## nova-platform: usare le applicazioni senza guardarle

Il primo dei tre strati previsti e' scritto: `crates/nova-platform` espone il
trait **`UiTree`**, con il backend Windows su UI Automation.

Non pixel: **oggetti**. L'albero di accessibilita' restituisce pulsanti, campi,
voci di menu e celle con nome, ruolo, valore e stato — e ogni applicazione che
rispetta l'accessibilita' lo espone, cioe' quasi tutte, per obbligo di legge.

### Come si indirizza un elemento

Con un **percorso di indici** dalla radice della finestra: `[1,1,1,6,7]`.

Niente stato tenuto aperto fra una chiamata e l'altra, niente puntatori COM da
custodire: `ui.find` restituisce i percorsi, `ui.click` li risolve rifacendo la
discesa. Se l'interfaccia e' cambiata nel frattempo il percorso non risolve e
l'errore lo dice — molto meglio di premere il pulsante sbagliato.

### Capacita'

| Capacita' | Rischio | Cosa fa |
|---|---|---|
| `ui.windows` | safe | finestre aperte con titolo, processo, handle |
| `ui.tree` | safe | albero dei controlli di una finestra |
| `ui.find` | safe | cerca per nome, ruolo, automation id |
| `ui.focus` | moderate | porta il fuoco su un elemento |
| `ui.click` | dangerous | preme, sceglie, spunta, seleziona |
| `ui.set_text` | dangerous | scrive in un campo senza simulare la tastiera |

```powershell
nova call ui.windows
nova call ui.tree window="Blocco note" depth=3
nova call ui.find window=Calcolatrice name=Sette role=button
nova call ui.click window=Calcolatrice "path=[1,1,1,6,7]"
nova call ui.set_text window="appunti" "path=[0,0]" text="ciao"
```

### Prove fatte

Blocco note: `ui.set_text` sul nodo `document [0,0]`, poi riletto dall'albero —
`value: "Scritto da NOVA senza toccare la tastiera."`, e la barra di stato
dell'applicazione confermava «42 caratteri».

Calcolatrice: quattro `ui.click` su `Sette`, `Più`, `Cinque`, `Uguale`, e il
risultato riletto dai nodi di testo:

```
1,1,1,0   L'espressione è 7 + 5=
1,1,1,1   Lo schermo è 12
```

Nessun mouse, nessuno screenshot, nessun pixel.

### COM in un mondo async

UIA e' COM: gli oggetti non sono `Send` e l'apartment va inizializzato una
volta sola. Tutto il lavoro vive quindi in **un thread dedicato** che possiede
l'apartment e la `IUIAutomation`; il resto del demone gli parla per messaggi.
Fuori si vede un backend normale, `Send + Sync`, tenuto in un `Arc` dentro
tokio, e le capacita' lo chiamano da `spawn_blocking`.

Se il backend non parte, il demone parte lo stesso: si perdono le `ui.*` e lo
dice nei log. Sugli altri sistemi c'e' un backend segnaposto che fallisce
nominando l'API da usare (AT-SPI2, AXUIElement) invece di restituire un albero
vuoto facendo credere che la finestra non abbia controlli.

### Limiti onesti

- I ruoli sono quelli di UIA, non quelli che uno si aspetta: l'editor del Blocco
  note e' `document`, non `edit`. Conviene guardare l'albero prima di cercare.
- Il campo `actions` di ogni nodo e' **dedotto dal ruolo**, non verificato:
  interrogare i pattern di ogni nodo costerebbe un giro COM per nodo. La verita'
  si scopre al momento dell'azione, che fallisce con un messaggio chiaro.
- Giochi, applicazioni disegnate su canvas e vecchia roba Java non espongono
  niente. Li' l'albero e' vuoto e servirebbe la visione: e' il ~5% dei casi.
- L'albero si ferma a 200 figli per nodo: certe liste ne hanno decine di
  migliaia e un albero che non finisce mai non serve a nessuno.
