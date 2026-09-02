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
| D28 | Un guasto si dice in italiano; il traceback va nel file, non sullo schermo | «PermissionError: [Errno 13]» non e' un messaggio, e' il nome di una classe: chi lo legge capisce solo che il programma e' rotto. Il traceback non sparisce, cambia posto: va dove serve a chi ripara |
| D29 | Il valore di una credenziale non entra in nessun messaggio, nemmeno in quello d'errore che arriva da fuori | il fornitore rimanda indietro la chiave dentro il proprio errore («Incorrect API key provided: sk-...»), e da li' finiva in chat e nel registro. Si copre prima di guardare cosa c'e' scritto |
| D30 | Quota finita e' `LimiteUso`, non un errore del compito | sono due notizie diverse: «non ci riesco» e «riprova piu' tardi». Detta cosi', il router mette in pausa il gradino e ripiega; detta come errore generico il ripiego non parte mai, ed e' quello che succedeva per i gradini a consumo |
| D31 | Il verificatore dell'harness confronta con prima, non con il verde assoluto | e' D18 applicata al codice dell'utente invece che al proprio: con una prova gia' rossa la regola severa rifiuterebbe ogni modifica, e il verificatore non si accenderebbe mai |
| D32 | Lo stato mentre lavora si dice a parole ed e' vivo, ma non finge di sapere quanto manca | un testo fermo e un programma fermo si somigliano troppo; una barra di avanzamento che si inventa una percentuale mente. Da quanto sta andando lo sappiamo, e quello si dice |
| D33 | Di interfacce ce n'e' una sola: l'orb | due interfacce non sono una scelta in piu' per l'utente, sono due posti dove le cose si scollano - e alla domanda «cosa vede uno appena installato» danno due risposte, cioe' nessuna |
| D34 | Il porting in Rust e' un progetto di distribuzione e robustezza, non di prestazioni; e si scrive contro il trait di `nova-platform` anche dove l'unico backend e' Windows | il Python costa ventotto millisecondi per turno: come progetto di velocita' non sta in piedi. Meta' della lista compatibilita' pero' sparisce se sul PC dell'utente non c'e' piu' Python. E scrivere contro il trait e' la differenza fra avere macOS e Linux a una implementazione di distanza e doverli rifare da capo |
| D35 | Il disinstallatore riconosce le proprie cose dall'inizio del nome, e non tocca il fascicolo | «contiene NOVA» cancellerebbe l'attivita' pianificata di qualcun altro chiamata «Innovation backup»; e cancellare il CV di qualcuno perche' ha disinstallato un programma sarebbe imperdonabile |
| D36 | Chi cerca sul disco non sa cosa sia un disco: le radici da percorrere si passano da fuori | e' la riga che rende il codice provabile su qualsiasi sistema con una cartella finta invece che solo sulla macchina di chi l'ha scritto. La domanda «quali dischi sono fissi» e' di piattaforma e sta in `nova-platform`, accanto a «quali schermi ci sono» |
| D37 | Un modello e' utilizzabile se arriva fin dove la sua tabella dei tensori dice, non se comincia con GGUF | l'intestazione sta all'inizio del file: uno scaricamento fermo al sessanta per cento ce l'ha tutta. Il controllo dei quattro byte prometteva di riconoscerlo e non poteva. Si guarda dove comincia l'ultimo tensore e non quanto e' lungo: la tabella dei tipi di ggml cambia fra le versioni, e un falso allarme su un modello sano e' peggio di un file a meta' non riconosciuto |
| D38 | Quanta memoria video c'e' lo dice DXGI, non `nvidia-smi`; e il libero non supera mai il dedicato | `nvidia-smi` e' il programma di NVIDIA: su una Radeon o su una Arc non esiste, il comando fallisce e la stima torna zero - cioe' «tutto in CPU» - senza dirlo. E il budget di DXGI comprende la RAM che la scheda puo' farsi prestare: su una integrata sono quindici gigabyte su quattrocento megabyte suoi, e crederci vorrebbe dire caricare il modello in RAM chiamandola GPU |
| D39 | Il calcolo degli strati e' dovuto sempre: senza memoria video leggibile si va sul processore, non a `-ngl` tirato a caso. E il numero viaggia con quanto crederci | NOVA deve stare su tutti i PC, e un numero mancante non e' un'informazione neutra: tre funzioni piu' in la' diventa un parametro deciso dal caso, proprio sulla macchina che non conosciamo. Lento di sicuro si dice e si rimedia; finto veloce no. La distinzione fra misurata e dedotta serve a non pagare quella prudenza due volte: su una deduzione si tiene margine doppio, ma la si usa |
| D40 | Cio' che si promette a chi non ha una scheda video e' un MoE, non «un modello qualsiasi, piu' lento» | misurato: sul processore si paga per i parametri che si accendono, e fra due modelli della stessa taglia ci sono 7,6 contro 1,8 token al secondo. Sopra i sette si legge mentre arriva e NOVA si usa; sotto i due una risposta arriva in tre quarti di minuto. E' la differenza fra un programma utilizzabile e uno che si installa e non si apre piu' |
| D41 | Senza scheda video leggibile l'installatore non offre il modello locale: salta il passo e suggerisce di riprovare piu' avanti con un modello leggero da leggere | «servono N GB di VRAM: andra' piano» e' troppo gentile per un denso da 27B: non e' piu' piano, e' un programma che si installa dopo tredici gigabyte e non si apre piu'. Un avvertimento piu' grosso non basta - la scelta va tolta |
| D42 | Il costo di un modello si misura in **byte letti per token**, non in gigabyte di file ne' in parametri attivi | generare un token a una richiesta per volta non e' calcolo, e' leggere i pesi dalla memoria: la velocita' e' banda diviso byte letti, e i byte letti sono i parametri che si accendono per i bit che ciascuno occupa. Misurato: 15,7 GB/token danno 1,8 tok/s e ~3,7 GB/token ne danno 7,6, con la stessa banda implicita. E' la formula che spiega perche' un MoE quantizzato e un denso a un bit finiscono nella stessa categoria |
| D43 | Che un formato sia supportato lo dice il binario che si ha, non il README di chi lo pubblica | la scheda di un modello mandava a clonare la fork del suo autore «perche' a monte i kernel non ci sono»; `llama-quantize --help` sulla build gia' installata elencava il formato alla riga 40. Una documentazione non e' una misura, meno che mai quando ha una preferenza su dove mandarti |
| D44 | Un percorso e' una struttura, non una stringa: «dentro» si chiede ai componenti, e un nome si riconosce a parole intere | tre difetti nello stesso file, tutti da questo: `runtime-vecchio` passava per `runtime` e si prendeva la precedenza assoluta; `cuda` cercato dentro tutto il percorso classificava come NVIDIA il motore di chi si chiama Cudale; la versione presa da tutto il percorso leggeva quella di una cartella qualunque piu' in alto. E' lo stesso errore del `bin/` nel `.gitignore` |
| D45 | Sul processore il modello sta in RAM, tutto: la velocita' non basta, ci vuole anche lo spazio | i byte letti per token dicono se e' abbastanza veloce, non se ci sta. Sulla GPU il file vive in VRAM; sul processore vive in RAM insieme al sistema e al browser, e un modello che entra sulla carta manda la macchina a paginare su disco - lentissimo con la ventola accesa. Sono due rifiuti diversi e meritano due frasi diverse: chi ha poca RAM puo' comprarne, chi ha un modello troppo denso no |
| D46 | Di cosa e' fatto NOVA sta in un dato, non in quattro elenchi scritti a mano, e una prova lega quel dato ai bersagli veri del workspace | gli elenchi erano quattro - CI, installatore, disinstallazione, compilazione - e si erano gia' disallineati: `nova-schede` era stato aggiunto a uno solo, senza che nulla lo segnalasse, perche' su chi sviluppa il binario c'e' comunque. Correggerli e' facile; impedire che diventino cinque no, e quella e' la prova |
| D47 | Una configurazione scritta male non deve poter mandare fuori casa ogni compito | il routing scarta gia' la categoria senza parole e senza soglia, che scatterebbe sempre. Ne mancava una scritta in un modo che sembra innocuo: `parole: ["*"]` diventava il pattern «\b» e basta, che trova un confine in qualunque testo. Il difetto non e' che l'utente sbaglia: e' che sbagliando manda i suoi dati fuori dal PC senza che nulla lo dica |
| D48 | Quando si accorcia la conversazione si scende fino a un fondo, non ci si ferma sul filo | fermandosi sul filo il turno dopo supera di nuovo e si taglia di nuovo: dal trentesimo turno in poi si tagliava a OGNI turno, la cache del prefisso non si riformava mai e ogni risposta pagava il prompt da capo per il resto della sessione. Misurato: 1.748 ms a turno contro 231. Non e' un flag a curarlo - `--cache-reuse` non cambia niente perche' la divergenza e' subito dopo il messaggio di sistema, e prima non c'e' niente da riusare |
| D49 | La conversazione si accorcia contando i **token**, non i messaggi | il limite del modello e' in token e la finestra si contava in messaggi: due unita' che non si parlavano. Dodici scambi con dentro il contenuto di un file fanno 102.953 token contro i 16.384 del contesto, e il taglio a messaggi non scattava nemmeno - erano venticinque messaggi. All'utente arrivava un JSON in inglese |
| D50 | Un messaggio piu' grande di tutto lo spazio si accorcia dichiarandolo, non si butta e non si tiene intero | buttarlo perde proprio la cosa di cui l'utente ha chiesto conto; tenerlo intero sfonda il contesto. Si tiene l'inizio, che dice cos'era, e la fine, che spesso porta la conclusione, e in mezzo si scrive quanto si e' tolto: un taglio dichiarato il modello lo capisce, uno silenzioso gli fa credere che il file finisca li' |
| D51 | Due implementazioni che concordano non sono due implementazioni verificate: la prova chiede «e' rimasto qualcosa di segreto?», non «dicono la stessa cosa?» | `Authorization: Bearer <token>` e' il modo piu' comune in cui una chiave finisce in un messaggio d'errore, e non era coperto ne' dal Python ne' dal Rust. Erano d'accordo, quindi qualunque confronto fra i due sarebbe passato. L'ha trovato una prova che guarda il risultato invece dell'accordo |
| D52 | I dodici gigabyte dei modelli non vanno in una cartella che sincronizza qualcun altro, e se ci vanno lo si dice prima | non e' il percorso a rompersi: e' che NOVA si installa dove l'utente ha scompattato il file, e se e' Documenti sincronizzato partono verso il cloud, il vault fa copie in conflitto, e i file vengono «liberati» restando in elenco come segnaposti vuoti - l'ultima mesi dopo, a NOVA che funzionava |
| D53 | Una prova che passa va guardata come una che fallisce: si chiede perche' passa | i percorsi oltre i 260 caratteri funzionano qui perche' `LongPathsEnabled` e' acceso su questa macchina e spento di fabbrica. Un verde che non si sa spiegare e' «da me funziona» con un bollino sopra, cioe' la versione che si difende da sola |
| D54 | Due modelli diversi che sbagliano identico non sono due modelli sbagliati: e' il prompt | a «ricordati che il mio gatto si chiama Ugo» sia Gemma sia Qwen chiamano `kb_search` invece di `kb_note`. Un banco che confrontasse i due l'avrebbe scritto come «sono d'accordo, tutto bene»: e' la prova che chiede se la scelta e' **giusta**, non se e' concorde, a vederlo |
| D55 | Un disinstallatore non cancella niente che non abbia creato lui: fuori dalla propria cartella elenca invece di rimuovere | «i tuoi dati» erano tre posti, non uno: `%APPDATA%\NOVA`, il fascicolo in Documenti e il vault dove l'ha messo l'utente. `-ConIDati` cancellava il primo e taceva sugli altri due, quindi chi disinstallava per ricominciare pulito si ritrovava la memoria intatta. Ma il vault puo' essere una cartella di Obsidian con dentro anche le note di chi la usa, e il modello sono sedici gigabyte che servono anche a LM Studio: si dice dove sono, con quanto pesano, e decide chi possiede il file |
| D56 | «Sta dentro questa cartella?» si chiede ai **nomi**, non a dove portano | `resolve()` segue i punti di reinnesto: misurato per sbaglio, un PowerShell dentro un pacchetto MSIX vede meta' di `%APPDATA%\NOVA` reindirizzata sotto `Packages\<pacchetto>\LocalCache`, e quattro file della stessa cartella risultavano **fuori** da essa. La domanda vera e' «se cancello questa cartella sparisce anche questo file?», e chi cancella una cartella cancella i nomi che ci stanno sotto. Con il separatore in fondo, se no `NOVA-vecchio` sta dentro `NOVA` |
| D57 | Una versione minima dichiarata si prova, non si spera: la grammatica con `feature_version`, la libreria standard cercando per nome | «Python 3.10 o superiore» era scritto in tre posti e provato in nessuno: la CI girava su una sola versione, e le tre non provate erano quelle su cui l'utente e' da solo. `ast.parse(..., feature_version=(3,10))` fa rifiutare la sintassi arrivata dopo, quindi quattro grammatiche si provano con un interprete solo; per la libreria non basta, perche' `import tomllib` si compila su 3.10 e poi non parte, e allora si cercano per nome le poche cose che uno usa senza accorgersi perche' sul suo PC ci sono gia' |
| D58 | Non si firma: si spiega, e la spiegazione va dove capita il fatto | una firma per un editore nuovo non toglie l'avviso di SmartScreen finche' non ha reputazione, quindi si pagherebbe per non risolvere il problema. La differenza fra tacere e spiegare non e' un paragrafo nel README: e' che «l'archivio non conteneva tutti i binari» era la diagnosi sbagliata nel caso piu' probabile — l'archivio li conteneva, li ha tolti l'antivirus dopo — e le due cure sono opposte |
| D59 | Il contrassegno «scaricato da Internet» si toglie **dopo** aver confrontato le impronte, mai prima | senza toglierlo SmartScreen chiede conferma a ogni avvio, non una volta. Toglierlo prima del controllo sarebbe zittire l'avviso di Windows su un file di cui non si sa ancora niente, che e' il comportamento di un installer malevolo: la stessa riga letta nei due ordini dice due cose opposte, e a separarle c'e' solo l'ordine. Una prova legge il sorgente e lo pretende |
| D60 | La memoria ha due strade, e quella che conta legge la domanda dell'utente non solo la risposta | «Ricordati che...» a volte non fa chiamare kb_note: il modello risponde a parole. Ma l'estrattore in sottofondo rilegge lo scambio e scrive il fatto da se', quindi il fatto arriva in memoria comunque. Il banco misurava solo lo strumento scelto in un turno; la domanda vera e' «e' finito nel vault?», e la risposta misurata end-to-end e' si' |
| D61 | Sul tool calling di un modello locale piccolo, alzare la voce nel prompt e' controproducente | rafforzare «usa kb_note, non limitarti a promettere» ha peggiorato la scelta da 3/4 a 1/6: il testo insistente sposta il modello verso il registro delle parole invece delle chiamate. Non ha un capo a cui obbedire, ha una distribuzione da seguire |
| D62 | Alla **seconda** occorrenza una cosa condivisa si mette in comune, non alla quarta | `giorni_del_mese` era privata dentro `nova-registro` e serviva di nuovo alla pianificazione. Le tre duplicazioni precedenti — l'elenco dei binari, le cartelle sincronizzate, i posti dei dati — si sono scoperte tutte **dopo** che si erano disallineate. Vederla prima e' l'unica differenza che conta: e' nato `nova-calendario` |
| D63 | Quando una prova fallisce, la prima cosa da verificare e' che abbia ragione lei | l'andata e ritorno di `piu_giorni` falliva su «366 giorni indietro dal primo gennaio 2027». Il codice diceva 31 dicembre 2025 ed era giusto: il 2026 non e' bisestile, ha 365 giorni. Avevo scritto 366 per analogia con «un anno», che vale solo negli anni bisestili. Cercare il difetto nel riporto sui mesi avrebbe rotto un calendario che funzionava |
| D64 | Sbattere contro un muro e girare a vuoto sono due guasti diversi: il primo si cura salendo di gradino, il secondo facendolo notare | il gradino sopra rifarebbe lo stesso giro, quindi far salire chi ripete la stessa chiamata riuscendo ogni volta costa e non risolve. E la ripetizione produce un promemoria, **mai** un divieto: la decisione resta al modello, e una ripetizione legittima non viene bloccata da niente |
| D65 | Una manopola a zero **spegne** la sua soglia, non la accende | `passi_prima_di_salire` a zero con un `>=` scritto senza pensarci diventa «passi >= 0», sempre vero: la manopola che serve a disattivare la funzione la farebbe scattare a ogni primo giro. C'e' una prova apposta |
| D66 | Nel porting si confrontano le decisioni, non le serializzazioni | far produrre al Rust la stessa stringa a byte di `json.dumps(sort_keys=True)` — spaziature, `ensure_ascii`, `default=str` — sarebbe lavoro vero per far combaciare un testo che non esce mai dal processo e serve solo a dire «uguale alla precedente». La serializzazione resta fuori, come i fusi in `nova-calendario` e il disco in `nova-registro` |
| D67 | Su un ingresso che viene da fuori, `unwrap_or_default()` non e' prudenza: e' una risposta inventata | una domanda illeggibile diventava una famiglia vuota, e la famiglia vuota produceva un verdetto perfettamente formato — «legge almeno 0.0 GB per token, non te lo faccio scaricare». L'installatore avrebbe rifiutato ogni modello dando all'utente una ragione inventata, e nessuno se ne sarebbe accorto perche' rifiutare e' la risposta giusta in molti casi veri |
| D68 | Un binario chiamato da PowerShell tollera il BOM: incontrare chi chiama e' compito suo | `Set-Content -Encoding UTF8` su Windows PowerShell 5.1 mette tre byte davanti al primo `{`, e serde si ferma con «expected value at line 1 column 1» — vero e inutile. Non e' sciatteria del chiamante: quel binario esiste **per** essere chiamato da li' |
| D69 | Il banco misura la traduzione, non il giunto: la cosa va chiamata almeno una volta da dove verra' chiamata davvero | 216 verdetti confrontati col Python erano verdi **con dentro** due difetti, perche' il confronto manda JSON di `json.dumps`: mai un BOM, sempre valido. Le prove parlavano al binario in una lingua che l'installatore non usa. Un minuto a mano ha trovato piu' di 216 casi automatici |
| D70 | Una funzione provata e mai invocata e' come non averla, e le prove non lo dicono | `solo_segnaposto` rilevava il **peggiore** dei guai della cartella sincronizzata — il modello «liberato» che resta in elenco con zero byte — ed era chiamata solo dalla sua prova. Una funzione pura, corretta e scollegata ha tutte le prove verdi che si possono desiderare: dicono «se la chiami risponde bene», non «la chiama qualcuno». Adesso una prova legge `install.ps1` e pretende la chiamata |
| D71 | Lo stesso servizio si scrive sempre allo stesso modo, e il nome si tiene in tabella invece di calcolarlo | «OneDrive» dalla variabile d'ambiente e «Onedrive» dal nome della cartella, per via di un `.title()`: due grafie nella stessa installazione. Quel messaggio deve convincere qualcuno a spostare una cartella, e una maiuscola sbagliata lo fa sembrare generato invece che scritto, proprio dove deve essere creduto |
| D72 | Una lezione imparata in un posto non si sposta da sola: quando si chiude un difetto, si cerca a mano in tutti i moduli che fanno la stessa domanda | `Authorization: Bearer <token>` era stato trovato e chiuso nei messaggi d'errore (D51) e passava indisturbato nel guardiano del vault, che e' il posto peggiore: cio' che entra li' finisce nel prompt a ogni turno, per sempre. Non c'e' un modo automatico di accorgersene — le prove di ciascun modulo passano, perche' ciascuno prova se stesso |
| D73 | Se due moduli chiedono «e' un segreto?», la domanda si fa in un posto solo | `guasti` conosceva il Bearer e le chiavi `sk-`, il vault conosceva AWS, Slack, le credenziali negli indirizzi e i numeri di carta: un errore con dentro `AKIA...` finiva in chiaro nel giornale. Aggiungere le voci mancanti a tutti e due avrebbe fatto due elenchi da allineare a mano, come gia' successo con gli eseguibili e le cartelle sincronizzate. `nova/forme_riservate.py` importa `re` e nient'altro, perche' lo carica anche l'installatore |
| D74 | Un confronto vale quanto le domande che fa, e zero confronti si stampano come zero divergenze | `test_guasti_rust.py` era verde mentre le due implementazioni divergevano su sei forme: il corpus conteneva solo le quattro che conoscevano entrambe. E una sonda scritta di fretta ha detto «0 divergenze su 14» con l'uscita vuota, perche' `zip` su una lista vuota non itera. Si contano le risposte prima di crederci |
| D75 | Il valore fatto di parole comuni e' comunque un segreto, se la chiave dice che lo e' | il controllo sulla densita' distingue «la password e' cambiata» da «la password e' Tramonto2026», e cosi' lasciava passare `seed phrase: abandon ability able...`, che e' sei parole di vocabolario. Ed e' l'unica cosa dell'elenco che non si puo' cambiare dopo: non esiste un «reimposta» per una seed phrase |
| D76 | Il mascheramento sta sulla porta, non nei chiamanti | il giornale delle azioni ha quindici punti che ci scrivono, e uno di loro registra il testo che NOVA **digita nei campi**: NOVA sa compilare un modulo di accesso, quindi prima o poi li' c'e' una password. Si maschera dentro `annota`, come il vault si chiude su `upsert`: un chiamante che si dimentica non e' un'ipotesi, e' una certezza |
| D77 | Se l'etichetta annuncia un segreto, il valore non si scrive — anche quando il valore da solo non sembra niente | «scritto in #password» in un campo e «Tramonto2026!» in un altro: separati non sono nulla, insieme sono una credenziale, e un filtro che guarda un campo per volta non puo' collegarli. Resta la riga, che dice cosa e' successo e dove, e sparisce il contenuto. Ma «scritto in #utente» con «giovanni.rossi» resta: un registro che copre tutto e' un registro che non si legge piu' |

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

  E chi porta un modello suo, senza passare dall'installer? Quel caso ora
  degrada da solo. `runtime.proiettore_accanto()` risponde a una domanda
  sola — «c'e' il proiettore accanto a questo GGUF?» — e la stessa risposta
  serve a due decisioni: se passare `--mmproj` alla riga di comando, e se
  allegare una figura alla conversazione. Tenerle separate voleva dire
  lasciarle divergere.

  Quando la risposta e' no, la schermata si scatta lo stesso (il file su
  disco ha valore comunque) ma al modello arriva una riga di testo al posto
  dell'immagine, che gli dice di non fingere di averla guardata e di usare
  `ui.tree`. Se l'immagine parte comunque — un server acceso da altri, un
  endpoint esterno — `llama-server` risponde **HTTP 500** con
  `image input is not supported [...] provide the mmproj`, e a quel punto
  NOVA sfila le figure dalla conversazione e riprova una volta: senza
  quello il messaggio con l'immagine resterebbe in coda e farebbe fallire
  **tutti** i turni successivi, non solo il suo.
