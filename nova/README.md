# nova/ — la prima versione, in Python

***Italiano** · la mappa del NOVA di oggi sta nel [README](../README.md#architettura)*

NOVA e' nata qui. Fino a settembre 2026 il ciclo dell'agente, gli strumenti,
la memoria e i cervelli giravano in questa cartella, e il demone in Rust
teneva solo i processi lunghi. Poi, un pezzo alla volta, e' passato tutto nel
Rust: ogni pezzo con un banco gemello che confronta le due versioni sugli
stessi casi (`prove/gemelli/`), e solo quando il banco dava verde. Il racconto
e' in [`docs/verso_la_beta.md`](../docs/verso_la_beta.md).

Oggi questa cartella serve a due cose:

- e' **il termine di paragone** dei banchi: finche' un pezzo del Rust ha il suo
  gemello, e' qui che si guarda cosa doveva fare;
- si usa ancora dal terminale, con `python -m nova`, per chi preferisce.

NOVA non ne ha bisogno per funzionare: l'orb, il demone, l'installatore e la
disinstallazione non chiamano Python. Python sul PC resta la lingua in cui NOVA
scrive le sue automazioni (D346), e questo e' l'unico motivo per installarlo.

## La mappa

Com'era, quando questa era NOVA:

```
nova/
  main.py             entrypoint: accende l'orb, oppure la CLI
  config.py           configurazione persistente (%APPDATA%\NOVA\config.json)
  setup_wizard.py     rilevamento automatico di modello GGUF e runtime
  modelli_trova.py    dove cercare un .gguf che l'utente ha gia'
  catalogo.py         quale modello ha senso qui, e quale non si scarica
  cartelle.py         se una cartella la sincronizza qualcun altro col cloud
  componenti.py       cosa serve a ogni funzione, e come procurarlo
  runtime.py          avvia/sorveglia/spegne llama-server.exe (+ auto-tuning GPU)
  gguf.py             la forma di un GGUF: strati e contesto, per la stima della VRAM
  daemon.py           accende nova-core quando non gira
  core_client.py      il canale verso il demone: capacita', processi, eventi
  agent.py            ciclo agente: modello <-> tool, sicurezza, approvazioni
  routing.py          chi risponde a cosa: gradini, delega, ripiego sulla quota
  brains/
    base.py           cosa deve saper fare un cervello, e `LimiteUso`
    openai_compat.py  il modello locale e le API esterne: stesso dialetto
    claude_cli.py     Claude Code in headless, con la memoria come server MCP
    cli_generic.py    le altre CLI agentiche, dichiarate senza scrivere codice
  guasti.py           un guasto detto in italiano; il traceback va nel file
  attesa.py           l'attesa che si vede passare, senza fingere una percentuale
  dati.py             dove NOVA tiene le tue cose, e cosa succede se le cancelli
  processi.py         nessun processo di NOVA apre una finestra nera
  lingue.py           in che lingua risponde, e i nomi dell'interfaccia
  browser.py          pilota Edge o Chrome in CDP: incolla, tabelle, caricamenti
  cerca.py            ricerca web senza aprire un browser sullo schermo
  immagini.py         le schermate che il modello puo' guardare
  ricette.py          le procedure imparate, ritrovate anche con un refuso
  automazioni.py      il guscio degli strumenti che NOVA scrive da se'
  banco.py            la copia su cui si prova una riparazione
  registro.py         cio' che non si annulla, si annota - e si ricerca
  rotazione.py        nessun diario cresce per sempre: due MB, uno storico
  pianificazione.py   attivita' ricorrenti e sentinelle
  fascicolo.py        i fatti veri sull'utente: CV, esperienze, testi suoi
  fogli.py            i fogli di calcolo: riferimenti, celle, come si rende
  harness.py          documenti e progetti: aprire, cercare, indicare
  harness_modifica.py proporre modifiche, e applicarle solo su richiesta
  harness_prova.py    i test del progetto: si applica se non peggiora
  mcp_kb.py           gli strumenti esposti a un cervello agentico che il demone non ha ancora
  kb_setup.py         regia della memoria: vault, motore, apprendimento
  tools/
    base.py           registry, schemi OpenAI, livelli di rischio
    files.py          leggere, scrivere, cercare, spostare, aprire
    apps.py           avviare app, elencare/focalizzare/chiudere finestre
    shell.py          PowerShell, CMD, Python
    web.py            ricerca web, lettura pagine, apertura nel browser
    documenti.py      leggere dentro pdf, docx, fogli
    system.py         appunti, tasti, volume, notifiche, promemoria, info PC
    tempo.py          promemoria e cose in programma
    schermo.py        schermate, quando leggere il sistema non basta
    deleghe.py        passare la palla a un gradino piu' alto
    kb.py             i sei strumenti con cui il modello usa la memoria
    automazioni.py    strumenti che NOVA scrive da se'
    procedure.py      come ha risolto una richiesta, per rifarla
    riparazione.py    il banco: si ripara da sola senza rompersi
  kb/
    schema.py         nodo + frontmatter
    store.py          il vault su disco, indice, relazioni, audit
    retrieval.py      BM25 + embedder + RRF + espansione grafo
    memory.py         cosa imparare da uno scambio, e cosa no
    riservatezza.py   cosa non entra in memoria nemmeno se passa di li'
    seed.py           la prima mappatura del PC
  voice/
    stt.py            ascolto: whisper, push-to-talk o parola di richiamo
    tts.py            voce: SAPI di Windows, zero dipendenze
    audio.py          microfono e altoparlanti
    elevenlabs.py     voce e trascrizione via ElevenLabs
    openai_audio.py   voce e trascrizione via API compatibili
```
