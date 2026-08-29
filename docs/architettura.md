# NOVA — Documento di architettura

> Stato: documento vivo. Raccoglie le premesse, le decisioni prese e il perche'.
>
> Il documento ha due livelli, e vanno tenuti distinti. Le **premesse**
> (sezione 1) sono invarianti: se una scelta le viola, si cambia la scelta.
> Le **decisioni** (sezione 7) sono revocabili: se cambiano i vincoli si
> riscrivono, annotando il motivo. Un documento che registra solo l'esito e
> non la ragione invecchia male, perche' nessuno sa piu' cosa si puo'
> rimettere in discussione.

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

## 1. Premesse: cosa non barattiamo

Queste non sono linee guida, sono **invarianti**. Un principio si adatta al
contesto; una premessa no: se una scelta la viola, si cambia la scelta.

Ognuna e' scritta insieme a **cio' che staremmo barattando**, perche' una
premessa che nessuno ha mai avuto la tentazione di rompere non e' una
premessa: e' un'ovvieta'. Il valore sta nel riconoscere l'affare quando
arriva, e rifiutarlo lo stesso.

---

### N1 — NOVA non ha confini di capacita'

Se l'utente lo consente, NOVA puo' fare tutto cio' che l'utente stesso
potrebbe fare sul proprio computer. Non si costruiscono sandbox, non si
amputano capacita' per rendere qualcosa «piu' sicuro».

**Baratto rifiutato:** «con un ambiente isolato dormiremmo tranquilli». Si',
e NOVA diventerebbe un altro prodotto — uno dei tanti che consigliano invece
di fare. La sicurezza si ottiene con reversibilita' e osservabilita' (N2),
non togliendo poteri.

### N2 — Prima la reversibilita', poi il permesso

Ogni volta che si puo' sostituire una richiesta di conferma con un
annullamento, si sostituisce. Un'operazione reversibile non ha bisogno di
essere temuta.

**Baratto rifiutato:** «basta chiedere conferma, e' piu' semplice da
scrivere». Vero, ed e' anche il motivo per cui quasi tutti gli assistenti
restano timidi: senza rete di recupero, ogni azione va giustificata prima, e
un agente che deve avere ragione al primo colpo non prova mai niente.

### N3 — I dati dell'utente non lasciano la sua macchina

Memoria, credenziali e configurazione vivono in `%APPDATA%\NOVA`. Non nel
repository, non in telemetria, non in un servizio nostro. Se l'utente sceglie
un cervello o una voce remoti, quello che esce e' **solo** cio' che serve a
quella chiamata, e glielo si dice in chiaro.

**Baratto rifiutato:** «un po' di telemetria anonima aiuterebbe a capire i
guasti». Aiuterebbe noi. Il patto con chi installa un programma che vede
tutto il suo computer e' che quel programma non riferisca a nessuno.

### N4 — Il segreto non passa dal modello

Quando NOVA compila una password, il valore va dall'archivio cifrato al campo
di destinazione **senza entrare nel contesto del modello**. E' una proprieta'
dell'architettura, non una buona abitudine: la capacita' accetta il *nome*
della credenziale, non il suo valore.

**Baratto rifiutato:** «sarebbe molto piu' semplice passargliela e basta».
Si'. E ogni conversazione, ogni log, ogni riassunto in memoria diventerebbe un
posto dove quella password puo' riaffiorare.

### N5 — Conta la conseguenza, non il gesto

Un'azione non e' compiuta perche' e' stata invocata. Verificare sul modulo che
si e' appena compilato non vale: vale l'effetto. La verifica e' strutturale,
non un passo che si salta quando si va di fretta.

**Baratto rifiutato:** «verificare raddoppia i tempi». Talvolta e' vero. Ma un
sistema che dichiara successi che non ci sono e' peggio di uno lento: e' uno
di cui non ti puoi fidare, e a quel punto ricontrolli tutto a mano.

### N6 — NOVA non ruba il posto all'operatore

Si agisce sull'albero di accessibilita', non su mouse e tastiera sintetici.
Cosi' NOVA lavora su una finestra in secondo piano mentre l'utente scrive in
un'altra. Lo screenshot e l'input simulato restano il **ripiego** per cio' che
non espone struttura: non diventano mai il canale principale.

**Baratto rifiutato:** «guardare lo schermo e cliccare e' molto piu' facile da
implementare». E' vero, ed e' anche il modo piu' rapido per uccidere il
co-working: nel momento in cui NOVA muove il tuo cursore, non potete piu'
lavorare insieme.

### N7 — Si puo' sempre fermare

Ogni operazione lunga e' interrompibile. «Ferma» significa fermare **l'azione
in corso**, non chiudere il programma. L'interruzione non e' una funzione di
emergenza: e' cio' che rende accettabile l'autonomia alta.

**Baratto rifiutato:** «lo aggiungiamo dopo, tanto le operazioni sono brevi».
Le operazioni brevi diventano lunghe il giorno in cui arriva il pianificatore.
L'interruzione va prima, sempre.

### N8 — Nessuna morte silenziosa

Un componente che si arrende lo dice, e lo dice dove qualcuno guarda. Nessun
guasto puo' assomigliare alla quiete.

**Baratto rifiutato:** nessuno — questa l'abbiamo pagata. Il supervisor si
arrendeva per sempre dopo cinque riavvii, e l'unico ad ascoltare l'evento era
una finestra che il progetto rende deliberatamente chiudibile. Per giorni
«il modello non e' attivo» ha avuto la faccia di una cosa normale.

### N9 — Il confine e' una manopola dell'utente

NOVA non decide quanto puo' osare. Parte cauta e si apre quanto l'utente
vuole. I livelli di autonomia limitano **quanto processo decisionale si
delega**, mai quali capacita' esistono: lo stesso strumento e' disponibile a
ogni livello, cambia solo chi decide se usarlo.

**Baratto rifiutato:** «certe cose non dovrebbe proprio poterle fare». Quel
giudizio spetta a chi possiede il computer.

### N10 — Ogni richiesta dichiara il proprio costo

Quando NOVA chiede qualcosa, dice cosa costa. Una rinuncia non si presenta
mai come un vantaggio.

**Baratto rifiutato:** «se lo formuliamo meglio accettano piu' spesso». Si
chiama raggiro, e funziona una volta sola.

### N11 — Se la strada non cede se ne prende un'altra; se non c'e', si costruisce

Un comando che non risponde non e' un muro: e' un modo che non funziona. NOVA
non insiste, cambia. E quando nessuna strada esistente arriva all'obiettivo,
se ne fabbrica una — un indirizzo composto a mano, un file scritto e
riconsegnato, uno script, un'automazione nuova. La capacita' di costruire il
pezzo mancante e' parte del mestiere, non un caso limite.

Il pavimento resta N6: qualunque via si inventi deve funzionare in secondo
piano. Una strada nuova che si prende tastiera e mouse non e' una strada, e'
un'interruzione con un altro nome.

**Baratto rifiutato:** «insistere e' tenacia». Insistere e' l'unica forma di
pigrizia che sembra impegno, ed e' costosa in modo misurabile: in un lavoro
vero, quindici tentativi su un filtro che non ha mai ceduto, mentre cambiare
fonte — quello che alla fine ha risolto — ne e' costati due.

## 2. Principi operativi

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

## 3. Il cervello

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
## 4. Distribuzione e fiducia

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

## 5. Permessi

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
## 6. Hardware e BIOS

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

## 7. Decisioni registrate

| # | Decisione | Motivo |
|---|---|---|
| D1 | Chiave API come via predefinita | unico canale esplicitamente previsto per uso programmatico |
| D2 | CLI di abbonamento come opt-in avvisato | il rischio di sospensione ricade sull'utente |
| D3 | Catalogo modelli in `models.json`, non nel codice | «il migliore» cambia ogni mese |
| D4 | Firma del codice invece di esclusioni antivirus | l'esclusione all'installazione è il pattern del malware |
| D5 | Conferma sempre come predefinito | il confine è una manopola dell'utente (P3) |
| D6 | BIOS: guida sempre, scrittura opt-in con rilevamento capacità | il rischio è un PC che non si avvia; il premio è piccolo |
| D7 | Permesso più stretto invece di esclusione totale | più onesto, più facile da concedere |
| D8 | Catalogo modelli in `models.json`, scelto sulla VRAM reale | `AdapterRAM` mente sopra i 4 GB: si legge nvidia-smi o il registro |
| D9 | Reversibilita' prima del permesso (N2) | un'operazione annullabile non va temuta, e permette a NOVA di osare |
| D10 | Interruzione prima del pianificatore | senza stop, l'autonomia alta e' inaccettabile a ragione |
| D11 | Le premesse N1-N10 sono invarianti, non linee guida | senza un livello che non si negozia, ogni scelta scomoda erode il progetto |
| D12 | Il prompt di sistema non si traduce: gli si dice in che lingua rispondere. Si traducono i nomi e i titoli | il modello capisce un'istruzione in italiano e risponde in coreano; l'interfaccia no, li' non c'e' nessun modello in mezzo. Tradurre il prompt vorrebbe dire N copie di un testo che cambia a ogni funzione nuova |
| D15 | Il dizionario dell'interfaccia ha l'italiano come chiave, non un identificatore | con chiavi astratte una voce dimenticata lascia la chiave o il vuoto; con l'italiano lascia una frase di senso compiuto |
| D16 | Un solo posto sa cosa serve a ogni funzione e come procurarlo (`nova/componenti.py`) | quando l'unico programma capace di scaricare era l'installer, ogni ripensamento costava una reinstallazione |
| D17 | NOVA puo' riparare il proprio codice, ma solo passando da un banco: copia, prova, confronta, applica | il modo piu' rapido di rompere un assistente in maniera irreparabile e' lasciare che si ripari da solo mentre e' rotto |
| D18 | Il criterio non e' «tutte le prove verdi», e' «nessuna prova che era verde diventa rossa» | con una prova gia' rossa la regola severa non lascerebbe passare niente, mai, e l'unica via d'uscita sarebbe spegnere il controllo |
| D19 | Una prova sparita conta come regressione | cancellare il file che ti accusa fa tornare tutto verde |
| D20 | Si registra la **procedura**, mai la risposta | «hai tre mail nuove» e' vero per dieci minuti; «si apre mail.google.com e si leggono le non lette» vale per mesi |
| D21 | La procedura la scrive il modello, non la deduciamo dalle chiamate | con un cervello agentico le chiamate non passano da noi: osservarle avrebbe funzionato per meta' dei cervelli e per l'altra meta' mai |
| D22 | Riconoscimento lessicale pesato, non embedding | l'embedder predefinito e' a hash e non sa che «guarda se ho posta» e «controlla le mail» sono la stessa cosa: darebbe somiglianze a caso, e una procedura sbagliata proposta con sicurezza e' peggio di nessuna |
| D23 | La procedura si suggerisce, non si esegue da sola | un riconoscimento lessicale che facesse partire azioni prima o poi manderebbe la mail sbagliata alla persona sbagliata |
| D24 | Un'automazione si collauda **prima** di salvarla: se la prova non gira, non nasce | un'automazione rotta ma salvata verrebbe riproposta come funzionante, e la volta dopo il guasto sembrerebbe venire da un'altra parte |
| D25 | Nessun filtro sul contenuto del codice generato | sarebbe un recinto alle capacita' di NOVA (N1). Le difese sono altre: creazione rischiosa quindi visibile, codice leggibile e cancellabile, esecuzione in un processo fermabile |
| D26 | Il modello scrive solo il corpo; il guscio lo mette NOVA | il contratto - parametri in, testo fuori, errori riportati - lo deve garantire il programma, non la buona volonta' del codice generato |
| D27 | Le automazioni compaiono come strumenti normali (`auto_*`) | se il modello dovesse ricordarsi di chiamare un «esegui_automazione» generico, tornerebbe a essere una decisione, e le decisioni sono la parte che costa |
| D13 | Cercare i modelli, riconoscerli e pilotare le CLI sta in Python, non nell'installer | una seconda copia in PowerShell diverge in silenzio: si aggiunge una cartella a una sola delle due e nessuno se ne accorge |
| D14 | Chi ha gia' un modello, un abbonamento o un server acceso viene servito prima di chi deve scaricare | far scaricare 13 GB a chi li ha gia' e' il modo piu' rapido per far chiudere l'installer |

## 8. Roadmap

### 8.1 La riformulazione

NOVA non e' un assistente con dei permessi: e' **un runtime personale per
agenti con accesso diretto al sistema operativo**. La domanda che guida il
lavoro cambia di conseguenza.

Non piu': *«come impedisco a NOVA di fare cose pericolose?»*
Ma: *«come faccio in modo che NOVA possa fare qualunque cosa, sapendo
esattamente cosa sta facendo, senza rompersi e potendo tornare indietro?»*

Le capacita' restano costanti; varia quanto processo decisionale si delega
(N9). Percio' non si costruiscono guardie: si costruiscono **annullamento,
anteprima, interruzione e memoria dei fatti**.

### 8.2 Cosa esiste gia'

Va detto prima di pianificare, altrimenti si riscrive cio' che funziona.

| Pezzo | Stato | Dove |
|---|---|---|
| Registro capacita' con rischio dichiarato | **fatto** — 41 capacita': 22 safe, 12 moderate, 7 dangerous | `nova-core/src/caps*.rs` |
| Bus eventi con sottoscrizione per argomento | **fatto** | `nova-core/src/bus.rs` |
| Processi come oggetti persistenti | **fatto** — pid, stato, riavvii, log | `supervisor.rs` |
| Nessuna morte silenziosa del supervisor | **fatto** — quarantena, `proc.gave_up` -> orb rosso | `supervisor.rs`, `bus.rs` |
| Albero di accessibilita' come canale d'azione | **fatto** | `nova-platform/src/windows_uia.rs` |
| Verifica della conseguenza | **primitiva presente** — `ui.attendi` aspetta l'effetto | `caps_ui.rs` |
| Credenziali cifrate, valore fuori dal modello | **fatto** — parametro `segreto:` | `segreti.rs` |
| Approvazione con attesa, scadenza, campanello | **fatto** | `caps_approvazione.rs` |
| Livelli di autonomia | **presente ma statico** | `SafetyConfig.autonomy` |
| Interruzione di cio' che e' in corso | **fatto** — `azione.ferma`, ogni capacita' e' interrompibile | `interruzione.rs`, `server.rs` |
| Annullamento delle scritture su file | **fatto** — giornale su disco, sopravvive al riavvio | `giornale.rs`, `annulla.*` |
| Freno visibile nell'interfaccia | **fatto** — bottone quando lavora, Esc contestuale | `ui/index.html` |
| Anteprima (`prova=true`) come contratto del registro | **fatto** — chi non sa rispondere rifiuta, non esegue | `capability.rs`, `server.rs` |
| Leggere documenti (PDF, Word, Excel) | **fatto** | `tools/documenti.py` |
| Vista | **fatto su tutti i cervelli** — le immagini viaggiano nei messaggi | `immagini.py`, `agent.py`, `runtime.py` |
| Agire nel tempo, anche a ripetizione | **fatto** | `tools/tempo.py` |
| Accorgersi dei cambiamenti e reagire | **fatto** — sondaggio, non notifiche | `osserva.rs`, `bus.rs` |

Circa meta' di quella che una roadmap scritta da fuori chiamerebbe «Fase 1»
e' gia' in piedi. Il lavoro vero e' altrove.

### 8.3 I tre pezzi che vengono prima di tutto

Rendono ogni cosa successiva meno rischiosa da costruire, e nessuno dei tre
toglie a NOVA un solo potere.

**A. Annullamento (N2).** Ogni capacita' che modifica dichiara la propria
inversa; il demone tiene un giornale delle operazioni. Un'operazione su 400
file si annulla con un comando solo.

> Difetto trovato e corretto: `files.py:194`. In uno **spostamento** con
> `overwrite`, la destinazione veniva distrutta con `rmtree()`/`unlink()`: chi
> chiede di spostare non ha chiesto di cancellare cio' che c'era. Ora finisce
> nel Cestino, e se non ci riesce si ferma invece di distruggere.
>
> Nota di onesta': la prima stesura di questo documento parlava di **tre**
> percorsi distruttivi. Guardandoli davvero, due erano legittimi —
> `files.py:262` e' `delete_path(permanent=True)`, cioe' una cancellazione
> definitiva **richiesta**, e `shell.py:101` rimuove un file temporaneo creato
> dallo strumento stesso. Il difetto era uno solo. Un elenco di problemi piu'
> lungo del vero e' un modo di sembrare rigorosi.

Windows regala meta' del lavoro: cestino e punti di ripristino. Il guadagno
non e' la prudenza, e' che NOVA puo' **osare**.

**B. Anteprima universale.** `prova=true` esiste oggi in **una sola**
capacita', `segreti.importa` — ed e' la ragione per cui quell'importazione e'
andata bene: 20 credenziali mostrate prima di scrivere un byte, e in anteprima
si e' visto che una password stava finendo come *nome del servizio*. Quel
difetto l'ha trovato la modalita' prova, non una revisione.

Va promossa a contratto del registro. Il valore piu' grande non e' per
l'utente: e' che **NOVA puo' verificare il proprio piano prima di eseguirlo**.

**C. Interruzione (N7).** Non e' un freno, e' un acceleratore. Senza stop ogni
azione va giustificata prima: NOVA diventa timida, l'utente la tiene ad
autonomia bassa, e a quel punto non serve a niente. Con lo stop **tentare
costa poco**, e un agente che puo' tentare e' molto piu' capace di uno che
deve avere ragione al primo colpo.

### 8.4 Poi

**Task come oggetti di prima classe.** Oggi esiste una conversazione, non
esistono task: se la chat si chiude non prosegue niente, e «riprendi quello
che stavi facendo» non ha un referente. La macchina a stati non e'
contabilita': e' **la** funzionalita' che rende reale il co-working — NOVA sul
secondo schermo mentre l'operatore sta sul primo, e il controllo da telefono.
Il pezzo che la rende viva: un task bloccato deve poter **parcheggiarsi e
chiedere** invece di fallire.

**Registro eventi strutturato, che NOVA rilegge.** Non per il debug umano: per
la sua memoria. Oggi la memoria e' scritta dal modello che riassume se stesso,
ed e' per questo che la scheda del bug del supervisor diceva `status: attivo`
mentre meta' era gia' corretta. Un modello che si racconta e' una fonte
inaffidabile su se stesso. Una memoria episodica **derivata dal registro** e'
fatta di cose accadute, non di cose narrate — e abilita la domanda che nessun
agente sa rispondere onestamente: *«perche' hai aperto Edge?»*.

**Autonomia negoziabile a tempo.** Non `autonomia = 2`, ma *«per questo task
mi servirebbe autonomia 3 per dieci minuti, me la concedi?»*. L'infrastruttura
c'e' gia': l'autonomia diventa una risorsa che si concede, non un interruttore
dimenticato acceso.

**Pianificatore.** Obiettivo -> piano -> passo -> azione -> osservazione ->
verifica. **Dopo** A, B e C: un pianificatore ad autonomia alta senza
interruzione e senza annullamento e' precisamente lo scenario che fa paura, e
a ragione.

**Memoria procedurale, con cautela.** Si promuove a procedura solo cio' che ha
**superato una verifica di conseguenza**, non cio' che e' semplicemente
accaduto tre volte. Altrimenti si automatizzano i propri errori.

### 8.5 Cosa NON si fa adesso, e perche'

**Sub-agent.** Moltiplicano i modi di sbagliare prima che ne esista uno solo
per capire cosa e' andato storto. Vengono dopo il registro eventi.

**Percezione a schermate come canale principale.** Violerebbe N6: l'albero di
accessibilita' funziona su finestre in secondo piano, ed e' la ragione fisica
per cui NOVA non ruba il posto.

**Riscrivere l'Agent Python in Rust.** Il confine e' gia' giusto: Python
ragiona, Rust agisce. Si sposta quando sara' stabile, non per eleganza.

## 9. Non-obiettivi

- Non si spegne VBS/Secure Boot per comodità.
- Non si chiedono esclusioni antivirus durante l'installazione.
- Non si promette parità fra modello locale e modelli a pagamento.
- Non si supporta la scrittura BIOS su schede senza mappa verificata.

## 9-bis. Il banco: riparare se stessa senza rompersi

NOVA ha le mani sul proprio codice da sempre: vive in una cartella che sa
leggere e scrivere. Quello che mancava non era il permesso, era **un posto dove
provare** una correzione prima che diventi il programma in esecuzione.

Il giro e' in `nova/banco.py`, esposto al modello in `nova/tools/riparazione.py`:

1. **si copia** — un albero di lavoro git a parte, con dentro esattamente il
   codice che sta girando, comprese le modifiche non ancora committate e i file
   nuovi. Un banco che non contiene il difetto e' tempo perso;
2. **si misura la partenza** — quali prove passano *adesso*. Senza questo
   numero «le prove passano» non dimostra niente;
3. **si lavora li'** — con gli stessi strumenti di sempre, sul percorso del banco;
4. **si confronta** — nessuna prova verde diventata rossa, nessuna prova
   sparita, niente fuori dal perimetro;
5. **si applica** — mettendo da parte gli originali, e registrando la
   riparazione perche' si possa annullare anche a NOVA spenta.

### Perche' non contraddice N1

N1 dice che NOVA non ha confini di capacita': niente sandbox, niente
recinti. Il banco sembra il contrario e non lo e', perche' non limita cio' che
NOVA puo' fare **sul PC** — limita solo il momento in cui una modifica al suo
**stesso codice** diventa reale. E' la differenza fra sequestrare gli attrezzi
a un falegname e un falegname che prova un incastro su uno scarto prima di
tagliare la trave. Sul banco NOVA puo' fare qualunque cosa, comprese quelle che
la romperebbero: e' esattamente a questo che serve.

### Cosa il banco non promette

Le prove coprono cio' che coprono. Una modifica puo' passarle tutte e rompere
qualcosa che nessuno prova: il banco riduce il rischio, non lo annulla. Per
questo la rete vera non e' il verde delle prove — e' che gli originali restano
da parte e `riparazione_annulla` funziona a distanza di giorni, senza bisogno
che NOVA sia accesa o funzionante.

## 9-ter. Le procedure: non rifare la fatica due volte

Il costo di una richiesta la prima volta non e' il modello: e' l'**esplorazione**.
«Controlla le ultime mail» la prima volta significa provare una strada, scoprire
che non va, provarne un'altra, trovare quella giusta. La seconda volta, senza
memoria, si rifa' tutto da capo.

`nova/ricette.py` tiene le procedure; l'agente le legge prima e le scrive dopo.

**Cosa si registra.** I passi: «apro X, cerco Y, leggo Z». Mai il risultato:
una memoria che risponde con i dati di ieri e' peggio di una che non risponde.

**Chi la scrive.** Il modello stesso, a turno finito, con una chiamata al
cervello veloce in sottofondo. Non la si deduce osservando le chiamate agli
strumenti perche' con un cervello agentico quelle chiamate non passano di qui:
Claude Code usa i propri strumenti per conto suo e consegna solo la risposta.
Chiederglielo funziona con tutti i cervelli.

**Come si ritrova.** Confronto lessicale con pesatura per rarita', piu' due
regole di parentela fra parole (una contenuta nell'altra, oppure sei caratteri
iniziali uguali) che coprono «email»/«mail» e «silenzia»/«silenzioso». Non
embedding: quello predefinito e' a hash e non capisce il significato.

**Come si usa.** La procedura finisce in coda alla domanda, sotto gli occhi del
modello, con scritto esplicitamente che e' come e' andata l'altra volta e non
come deve andare oggi. Non parte niente da sola.

**Quando NON si registra.** Sotto gli otto secondi (non c'era fatica da
risparmiare), senza strumenti usati (era una conversazione), a interruttore
spento. Le procedure sono al massimo sessanta: oltre, un archivio diventa
rumore e il rumore fa proporre la strada sbagliata.

## 9-quater. Le automazioni: quando la procedura diventa uno strumento

Una procedura e' un appunto: il modello la legge e poi decide passo per passo,
e **ogni decisione e' un giro di modello**. Un'automazione toglie il modello di
mezzo per la parte meccanica.

Il giro: NOVA scrive il **corpo** di una funzione, `nova/automazioni.py` ci
mette attorno il guscio (lettura dei parametri, cattura degli errori, formato
dell'uscita), la esegue in una cartella d'appoggio con i parametri di prova, e
**solo se gira** la sposta fra le automazioni vere. Da quel momento compare
nell'elenco degli strumenti come `auto_<nome>`.

L'iniziativa nasce dai dati che ci sono gia': quando una procedura risulta
rifatta tre volte e non ha ancora un'automazione, il blocco che finisce sotto
gli occhi del modello glielo fa notare. Niente euristiche inventate, solo il
contatore.

### I tempi, misurati

| | tempo |
|---|---|
| eseguire un'automazione (dal registro) | **0,04-0,07 s** |
| turno di modello senza strumenti | ~3 s |
| **richiesta risolta da un'automazione** | **~8-12 s (2 turni)** |
| la stessa richiesta esplorando da zero | decine di secondi |

L'automazione in se' e' gratis. Il pavimento restano **due** turni di modello,
non uno: il primo per decidere di chiamarla, il secondo per raccontare il
risultato. Non si scende sotto senza cambiare qualcosa d'altro - vedi le
questioni aperte.

## 10. Questioni aperte

- **Il secondo turno.** Una richiesta risolta da un'automazione costa due giri
  di modello: uno per chiamarla, uno per commentare cio' che ha risposto.
  Misurato: 12,4 s per «quanto spazio ho sui dischi», di cui 0,07 s di lavoro
  vero. Per arrivare ai tre secondi servirebbe che l'uscita dell'automazione
  fosse **gia'** la risposta all'utente, restituita senza ripassare dal
  modello. E' fattibile - un campo «risposta diretta» nel manifesto - ma
  toglie al modello la possibilita' di accorgersi che il risultato non ha
  senso, e va deciso sapendo cosa si baratta.
- **Dove vanno i secondi** (misurato su questa macchina, cervello Claude Code):

  | | tempo |
  |---|---|
  | avviare Python e importare tutta NOVA | 0,25 s |
  | avviare Node / il CLI di Claude | 0,05-0,20 s |
  | eseguire uno strumento (il lavoro vero) | millisecondi |
  | **un turno di modello** | **~3 s** |
  | turno con una chiamata a strumento (2 turni) | 8-9,5 s |
  | preparare e consultare la memoria | ~0,5 s |

  Chiamare una funzione non costa niente: costa **decidere di chiamarla**, e
  ogni decisione e' un giro completo di modello. Da qui la regola pratica:
  ~3 secondi di base, ~3-5 secondi per ogni passo che richiede un pensiero.
  Le procedure imparate non rendono il passo piu' veloce - **tolgono passi**,
  ed e' li' che sta il guadagno: dieci turni di esplorazione contro due di
  esecuzione sono un minuto contro nove secondi.

- Per scendere sotto i tre secondi su una richiesta gia' nota bisognerebbe
  eseguire i passi **senza passare dal modello**, e chiamarlo una volta sola
  alla fine per formulare la risposta. E' realistico - il tetto sarebbe il
  singolo turno, cioe' i ~3 s misurati - ma apre la domanda vera: quando una
  procedura e' abbastanza collaudata da eseguirla alla cieca. Oggi la risposta
  e' mai, e il modo per cambiarla e' lo stesso schema del banco: N esecuzioni
  con lo stesso esito, anteprima, e annullamento gia' pronto.
- Il riconoscimento lessicale sbaglia in modo prevedibile: due richieste che
  usano parole diverse per la stessa cosa non si trovano ancora («posta
  elettronica» contro «inbox»). Il rimedio serio e' un embedder vero, che c'e'
  gia' come opzione (`kb.embedder = "llama"`) ma richiede un secondo server.
- Il banco prova quello che le prove provano. `test_voce.py` e' rosso da prima
  per una falsa segnalazione, e la copertura sul guscio Rust e sull'interfaccia
  e' zero: una riparazione che tocca `core/` o `ui/` passa la verifica senza
  che nessuno l'abbia davvero provata. Prima di allargare il banco a quelle
  parti serve almeno un `cargo check` dentro il confronto.
- Se una riparazione debba poter modificare `nova/banco.py` stesso. Oggi puo':
  il processo ha gia' i moduli in memoria, quindi non si taglia il ramo sotto i
  piedi in corsa, ma il giro successivo userebbe il codice nuovo per giudicare
  se stesso.
- La lingua e' fatta a meta' e si sa quale meta': l'interfaccia ha il
  dizionario italiano e inglese, le altre nove lingue offerte fanno rispondere
  NOVA nella lingua giusta ma lasciano i menu in italiano. La tendina lo dice
  invece di lasciarlo scoprire dopo.
- I componenti scaricabili sono quattro (voce, ONNX, espeak, ascolto). Il
  modello del cervello no: si sceglie ancora dall'installer o a mano. E' il
  prossimo pezzo che dovrebbe passare dal pannello.
- Se il primo avvio guidato debba stare dentro NOVA invece che
  nell'installer: un installer che fa dieci domande perde meta' della
  gente alla quarta, e le stesse domande poste dall'interfaccia si
  possono rifare.
- Costo e tipo del certificato di firma: chi lo intesta in un progetto open
  source, e chi lo rinnova.
- Se il modello locale debba essere proposto per primo a chi ha l'hardware
  adatto, o restare la seconda scelta.
- Formato e verifica delle mappe IFR contribuite: come si accetta il
  contributo di uno sconosciuto per una cosa che, se sbagliata, non fa
  avviare un PC.
- Come si comporta il «conferma sempre» durante un lavoro lungo in
  co-working, quando l'utente non sta guardando.

- Il giornale delle operazioni: quanto indietro tenerlo, e cosa fare delle
  operazioni non invertibili (un processo ucciso non si resuscita). Serve un
  modo onesto di dire «questo pezzo non si annulla» **prima** di eseguirlo.
- L'anteprima universale su capacita' che chiamano il mondo esterno: cosa
  vuol dire «prova» per una richiesta HTTP che ha effetti sul server altrui.
- Se l'autonomia concessa a tempo debba scadere sul tempo o sul task: dieci
  minuti passano anche mentre NOVA aspetta una risposta dell'utente.
- **Nipoti orfani.** `kill_on_drop` uccide il figlio diretto di `shell.exec`,
  non i suoi discendenti: un comando che ne avvia altri lascia processi vivi
  dopo un «ferma». La cura sono i **job object** di Windows — si assegna il
  figlio a un job con `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` e cade tutto
  l'albero insieme. Finche' non c'e', «fermare» e' vero per il caso comune e
  parziale per quello annidato, e va detto invece che lasciato scoprire.
- **L'annullamento copre poco.** Oggi vale per `fs.write` e per lo
  spostamento che sovrascrive. Restano fuori le operazioni delle capacita'
  Python (copia, rinomina in blocco) e tutto cio' che tocca l'interfaccia:
  `ui.set_text` cambia un campo e nessuno sa cosa c'era prima. Serve che
  ogni capacita' che modifica dichiari la propria inversa, altrimenti
  «annulla» diventa una promessa che vale solo a volte — e non si sa quando.
- **L'anteprima copre quattro capacita' su 46.** `fs.write`, `shell.exec`,
  `annulla.*` e `segreti.importa` sanno dire cosa succederebbe; le altre
  rifiutano onestamente. Va bene come punto di partenza — meglio un rifiuto
  chiaro di un'anteprima finta — ma le piu' utili mancano ancora: `ui.click`
  e `ui.set_text` dovrebbero dire *quale elemento* toccherebbero e in quale
  finestra, che e' proprio il controllo che si vorrebbe fare prima di lasciar
  premere un pulsante a qualcun altro.
- ~~La vista dipende dal cervello.~~ **Corretto.** Avevo scritto che con una
  chiave API NOVA sarebbe rimasta cieca: era sbagliato, e in un modo che vale
  la pena ricordare. I modelli **vedono** — Qwen, GPT e gli altri sono
  multimodali da tempo. Cio' che mancava non era la vista del cervello ma il
  **tubo**: nessuno costruiva un messaggio con l'immagine dentro. Attribuire ai
  modelli un limite che era nel nostro codice avrebbe portato a scrivere «NOVA
  e' cieca qui» invece di costruire dieci righe.

- **Niente riconoscimento ottico.** Un PDF scansionato viene rifiutato con un
  messaggio onesto, ma resta illeggibile. Vale anche per il testo dentro le
  immagini catturate dallo schermo.
- **L'osservatore guarda una cartella sola, non i sottolivelli**, e vive nel
  demone: si spegne quando si spegne lui. Per «tienilo d'occhio anche domani»
  servirebbe che le osservazioni si scrivano su disco come il giornale.
- ~~Il modello locale vede solo se il proiettore c'e'.~~ **Chiuso**:
  l'installer scarica `mmproj-F16.gguf` insieme al modello e lo conta nello
  spazio richiesto. Se il download fallisce lo dice, invece di lasciare la
  vista spenta in silenzio.
