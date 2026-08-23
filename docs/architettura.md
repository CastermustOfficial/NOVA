# NOVA — Documento di architettura

> Stato: bozza di lavoro. Raccoglie le decisioni prese finora e il perché.
> Quando una decisione cambia, si riscrive qui la voce e si annota il motivo:
> un documento che registra solo l'esito e non la ragione invecchia male,
> perché nessuno sa più quali vincoli si possono rimettere in discussione.

## 0. Cosa è NOVA

Un assistente che sta sul PC dell'utente e **fa le cose**, non solo le spiega.
L'immagine di riferimento è un tecnico competente seduto accanto: guarda,
capisce, agisce, e continua a lavorare mentre tu lavori.

Tre proprietà che lo distinguono da una chat:

- **Agisce.** Apre programmi, compila moduli, scrive file, esegue comandi.
- **Non ruba il posto.** Lavora in una finestra sua, sull'albero di
  accessibilità e non su mouse e tastiera, quindi non interrompe l'operatore.
- **Vede lo stato reale.** Legge il PC — finestre aperte, hardware, servizi —
  invece di indovinare.

## 1. Principi

**P1 — Conta la conseguenza, non il gesto.** Un'azione non è compiuta perché è
stato premuto un pulsante. Verificare sul modulo appena compilato non vale:
vale controllare l'effetto.

**P2 — «Non posso» va qualificato.** Quasi mai è vero in assoluto. Le forme
oneste sono «non posso da qui», «non posso adesso», «non posso senza il tuo
permesso» — e ognuna dice all'utente cosa fare dopo. Un «non posso» secco
dove esisteva una strada è un difetto, non prudenza.

**P3 — Il confine è una manopola dell'utente.** NOVA non decide da sé quanto
può osare. Di fabbrica parte cauta; l'utente la apre quanto vuole.

**P4 — Degradare con grazia.** Ogni funzione che dipende da un permesso, da
un componente o da hardware specifico deve poter mancare senza rompere il
resto. Si rileva la capacità, si offre solo ciò che esiste.

**P5 — Dichiarare il costo.** Quando NOVA chiede qualcosa, dice cosa costa in
chiaro. Nessuna richiesta va presentata come un vantaggio se è una rinuncia.

## 2. Il cervello

NOVA non è legata a un modello. Il router sceglie il gradino; il gradino è
configurazione, non codice.

### 2.1 Le tre vie

| Via | Chi la usa | Stato |
|---|---|---|
| **Chiave API a consumo** | chi vuole la qualità massima e sa cosa paga | **predefinita** |
| **Modello locale** | chi vuole gratuito, offline, privato | consigliata a chi ha l'hardware |
| **CLI di un abbonamento** | utenti avanzati | opt-in, avvisata |

### 2.2 Perché la chiave API è la predefinita

È l'unico canale **esplicitamente previsto** dai fornitori per un uso
programmatico. L'utente sa cosa paga e quanto.

### 2.3 Perché la CLI di abbonamento non è la predefinita

Gli abbonamenti consumer (Claude, ChatGPT e simili) sono pensati per l'uso
interattivo dell'abbonato. Usare la loro CLI come motore di un'applicazione
terza è, per la maggior parte dei termini di servizio, fuori perimetro.

Il rischio non lo corre il progetto: lo corre l'utente, che può vedersi
limitare o sospendere l'account. Un progetto che fa sospendere l'abbonamento
a chi lo installa perde la fiducia una volta sola.

Resta disponibile, per chi sa cosa sta facendo, con l'avviso in chiaro.

### 2.4 Il modello locale e il manifest

«Il migliore in commercio» cambia ogni mese. **Non può stare nel codice**,
o ogni modello nuovo diventa una release.

Sta in `models.json`: nome esatto, taglia, quantizzazione, VRAM e RAM
richieste, fonte del download, qualità dichiarata e data dell'ultima
revisione. Il codice sceglie *come* scaricare e avviare; il manifest dice
*cosa*. Aggiornare la classifica è modificare un file di dati — e NOVA può
rileggerlo da sola.

Il flusso: controllo dei requisiti, proposta del modello migliore che quella
macchina regge davvero, **dichiarazione onesta della qualità attesa**,
download, verifica dell'hash.

Sulla dichiarazione di qualità: dire «più lento e meno capace dei modelli a
pagamento, ma sufficiente per X e Y» costruisce fiducia. Promettere parità
la distrugge al primo confronto.
## 3. Distribuzione e fiducia

Il problema: un eseguibile non firmato che automatizza l'interfaccia, lancia
processi e conserva credenziali **verrà** segnalato da SmartScreen e da
Defender. È il muro più sottovalutato dei progetti di questo tipo: senza
risposta, metà degli utenti non arriva al primo avvio.

### 3.1 Cosa si fa

1. **Firmare il codice.** Un certificato EV dà fiducia SmartScreen da subito;
   uno OV la costruisce col tempo. È la voce di bilancio numero uno.
2. **Segnalare i falsi positivi** ai fornitori di antivirus. Gratis, previsto,
   funziona.
3. **Distribuire via winget**, che ha reputazione propria.
4. **Build riproducibili e hash pubblicati.** È il vantaggio dell'open source:
   chiunque può verificare che il binario corrisponda al sorgente.

### 3.2 Cosa non si fa

**L'installer non chiede mai un'esclusione antivirus.** Un'app che come primo
atto chiede di essere tolta dai controlli è indistinguibile da un malware, e
l'utente fa bene a chiudere tutto. All'installazione la fiducia è zero: si
guadagna, non si chiede in prestito.

Questo **non** vieta di chiedere un permesso più avanti — vedi 4.3.

## 4. Permessi

### 4.1 Il valore sta nella descrizione

Predefinito: **conferma sempre**.

Ma una richiesta generica è peggio di nessuna richiesta: dopo tre giorni di
«consentire operazione su file?» si clicca sì a occhi chiusi, e il presidio
è evaporato mentre sembra ancora in piedi.

- No: «Consentire operazione su file?»
- Sì: «Sto per eliminare 4 file in Download: [elenco]. Procedo?»

Il bottone non protegge nessuno. Protegge la frase.

### 4.2 Il permesso più stretto che sblocca la funzione

Quando serve un permesso di sistema, si chiede **il più mirato possibile**.

Su Windows le cose che bloccano davvero l'automazione legittima non sono le
scansioni antivirus, ma l'**accesso alle cartelle controllate** e le regole
**ASR**. Entrambe hanno liste di app consentite per singola applicazione.

«Lasciami scrivere in Documenti» è più onesto, più mirato e molto più facile
da concedere di «escludimi dai controlli».

### 4.3 Il momento conta più della richiesta

La stessa domanda è legittima o sospetta a seconda di quando arriva.

- **All'installazione**: no. Fiducia zero, nessun contesto.
- **Dopo, per una funzione precisa e nominata**: accettabile, se opt-in, se
  dichiara il costo (P5) e se il rifiuto lascia il resto funzionante (P4).

Forma corretta della richiesta:

> «Per fare X devo essere esclusa dai controlli di Windows. Vuol dire che
> Windows non mi controllerà più. Se preferisci di no, X non lo faccio e
> tutto il resto continua a funzionare uguale.»

Mai chiamarla «protezione aggiuntiva»: un'esclusione toglie controlli, non
li aggiunge. Presentare una rinuncia come un vantaggio è la frase che un
domani viene citata contro il progetto.

### 4.4 Nota sul fare da antivirus

Le esclusioni non sono lo strumento con cui si costruisce un prodotto di
sicurezza. La strada vera è registrarsi nel **Centro sicurezza di Windows** e
usare **AMSI**. È una strada da fornitore riconosciuto, oggi fuori scala.
Annotata perché la porta è quella, non l'esclusione.
## 5. Hardware e BIOS

### 5.1 Il livello sempre disponibile: diagnosi, guida, verifica

Funziona su **ogni** macchina, senza permessi speciali e a rischio zero.
NOVA legge scheda madre, BIOS, RAM, CPU; riconosce le configurazioni
sbagliate; **guida passo per passo** con istruzioni cucite su quella scheda;
e dopo il riavvio **verifica** che sia andata.

Esempio reale, dalla macchina di sviluppo: RAM DDR5 a 4800 MT/s su Ryzen
7600X, cioè il ripiego JEDEC — EXPO disattivato, banda lasciata sul tavolo.
Rilevabile in una query, correggibile in trenta secondi, invisibile a chi
non sa dove guardare.

Questo è il 95% del valore, ed è quello che fa l'esperto seduto accanto: non
ti sfila la tastiera, ti dice cosa c'è che non va e controlla dopo.

### 5.2 Il livello opt-in: scrittura diretta

**Perché non può essere la strada principale**

- Gli offset della variabile di setup cambiano **per ogni scheda e per ogni
  versione di BIOS**. Non esiste mappa universale.
- La firma dell'applicazione UEFI richiede una chiave che in open source
  **non si può distribuire**: ogni utente dovrebbe generarla e registrarla
  nel proprio BIOS.
- Un errore non produce un file corrotto: produce **un PC che non si avvia**,
  recuperabile solo aprendo il case. Su una macchina altrui è inaccettabile
  come comportamento predefinito.
- Il premio è piccolo: un utente normale tocca il BIOS forse quattro volte
  nella vita del PC (EXPO, virtualizzazione, TPM, ordine di boot).

**Come funzionerebbe**

Applicazione UEFI sulla partizione EFI, lanciata con `BootNext`. Gira in fase
DXE/BDS — lo stesso contesto del setup del BIOS — quindi senza le protezioni
SMM che bloccano le scritture a sistema operativo avviato.

Architettura obbligata: **cervello sopra, mani sotto.** Nella fase firmware
non esistono Python, modello o rete: ci sono poche centinaia di KB di C.
NOVA ragiona dove c'è un OS, lascia un'istruzione precisa, riavvia, l'agente
minuscolo esegue, si torna su, NOVA verifica. È come lavora un tecnico: non
pensa dentro il BIOS, decide fuori ed entra a eseguire.

**Reti di sicurezza**

- `BootNext` è **monouso**: il firmware la consuma e la cancella. Se l'agente
  fallisce, il riavvio successivo torna a Windows da solo.
- Dump completo della variabile **prima** di scriverla, quindi ripristinabile.
- AM5 ha il fallback automatico sul training della memoria.
- Ultimo gradino, dichiarato in anticipo: reset del CMOS a mano.

**Rilevamento di capacità** (P4)

All'avvio NOVA guarda scheda madre, versione BIOS, stato di Secure Boot, e
cerca se esiste una mappa IFR per quella combinazione. In base a cosa trova
offre la via automatica o quella guidata. Le mappe crescono nel tempo,
contribuite dagli utenti: chi ha una scheda non coperta la genera e la manda
al repo.

Il predefinito resta **sempre** la guida. L'automatico è opt-in, con l'avviso
in chiaro e la procedura di recupero detta prima, non dopo.

**Primo passo, a rischio zero**: scaricare il file del BIOS pubblicato dal
produttore, estrarne l'IFR con UEFITool, ottenere la mappa. È solo parsing di
un file — non tocca la macchina — e dice se la strada esiste prima di
scrivere una riga di codice UEFI.

### 5.3 Livelli scartati

| Livello | Perché no |
|---|---|
| Driver kernel (ring 0) | HVCI attivo blocca i driver non firmati. Ed è «sotto» come privilegi, non come tempo. |
| Hypervisor sotto l'OS | Su Windows moderno il posto è **già occupato** da Hyper-V/VBS. Liberarlo significa spegnere proprio ciò che protegge le credenziali. |
| Flash del firmware modificato | Nessun potere in più sulle impostazioni, e l'errore non si recupera col reset del CMOS: serve un programmatore hardware. |

## 6. Decisioni registrate

| # | Decisione | Motivo |
|---|---|---|
| D1 | Chiave API come via predefinita | unico canale esplicitamente previsto per uso programmatico |
| D2 | CLI di abbonamento come opt-in avvisato | il rischio di sospensione ricade sull'utente |
| D3 | Catalogo modelli in `models.json`, non nel codice | «il migliore» cambia ogni mese |
| D4 | Firma del codice invece di esclusioni antivirus | l'esclusione all'installazione è il pattern del malware |
| D5 | Conferma sempre come predefinito | il confine è una manopola dell'utente (P3) |
| D6 | BIOS: guida sempre, scrittura opt-in con rilevamento capacità | il rischio è un PC che non si avvia; il premio è piccolo |
| D7 | Permesso più stretto invece di esclusione totale | più onesto, più facile da concedere |

## 7. Non-obiettivi

- Non si spegne VBS/Secure Boot per comodità.
- Non si chiedono esclusioni antivirus durante l'installazione.
- Non si promette parità fra modello locale e modelli a pagamento.
- Non si supporta la scrittura BIOS su schede senza mappa verificata.

## 8. Questioni aperte

- Costo e tipo del certificato di firma: chi lo intesta in un progetto open
  source, e chi lo rinnova.
- Se il modello locale debba essere proposto per primo a chi ha l'hardware
  adatto, o restare la seconda scelta.
- Formato e verifica delle mappe IFR contribuite: come si accetta il
  contributo di uno sconosciuto per una cosa che, se sbagliata, non fa
  avviare un PC.
- Come si comporta il «conferma sempre» durante un lavoro lungo in
  co-working, quando l'utente non sta guardando.