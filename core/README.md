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
