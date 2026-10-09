# idea/

Ogni possibile miglioria, anche solo teorica o non ancora decisa. Per ognuna: cosa, perché, cosa costerebbe. Un'idea non è un impegno: quando si decide di farla passa in [`piano/`](../piano/README.md), quando la si scarta resta qui con il perché.

Aggiornato al 9 ottobre 2026. Le idee che c'erano già erano sparse nei documenti: qui sono raccolte, con il rimando a dove sono nate.

## CANT-12: decisioni tipizzate al posto delle euristiche

**Cosa.** NOVA decide molte cose con liste di parole, soglie ed espressioni regolari: quale cervello serve, se una frase è un fatto da ricordare, se un ricordo è pertinente, se una chiamata è rischiosa. Un modello «System One» risponde a domande tipizzate (scelta, punteggio, sì/no) con una probabilità, senza generare testo. Il censimento delle decisioni e il confine su cosa può uscire dal PC sono già scritti in `nova-decisioni`. La metà pura, dai logit al giudizio, è già in `nova-giudizio`. Manca la metà che parla a un modello.

**Le strade raccolte** (in [`docs/verso_la_beta.md`](../docs/verso_la_beta.md), sezione CANT-12):

- **I logit delle lettere**, come Rizzo Flow: una domanda a scelta multipla e le probabilità delle lettere dopo un solo passaggio del modello, con il llama-server che NOVA già accende. Le probabilità si leggono con `n_probs`, e il prefisso comune si riusa con `cache_prompt`: tutti e due vanno verificati prima.
- **CLM-v0.1-8B** (aggiunto da Gio il 28 settembre): un Qwen3-8B congelato più due teste da circa 20M parametri, Apache 2.0. Sul suo banco rende quanto Jev o meglio. Misurato il 4 ottobre con `misure/banco_clm.py`: il vettore lo dà llama-server (`--embeddings --pooling last`), anche lo stesso che fa la chat, e Q8_0 sposta poco i giudizi rispetto al bf16 (4 su 52), Q4_K_M troppo (11 su 52). Ma sulle domande del banco delle lettere, senza addestrarlo, ne indovina 2 su 10 in italiano e 4 in inglese con le opzioni corte, 5 e 6 con le risposte intere. Così com'è non è un giudice per NOVA.
- **jevlike**: un classificatore addestrato su un codificatore piccolo. Vuole dati per addestrarlo.

**Perché.** Un conto sbaglia dove i conti sbagliano: una soglia di lunghezza impara le frasi lunghe e inutili e butta «mi chiamo Gio».

**Cosa costerebbe.** Per CLM le tre misure sono fatte (sopra, e i numeri in `verso_la_beta.md`). Resta una strada sola: **addestrare le teste sulle decisioni di NOVA**. Il repository di CLM lo prevede (`train/finetune.py --task choice`) e le teste sono piccole, ma servono esempi etichettati delle decisioni del censimento, in italiano, e un banco tenuto da parte su cui misurarle; e il vettore resta legato al Qwen3-8B, quindi vale solo per chi usa quel modello o accetta un secondo modello acceso (6.149 MiB di VRAM a Q4_K_M, 9.362 a Q8_0). Poi, come per le lettere, un banco che confronti i giudizi con le euristiche di oggi. Quel banco c'è per `QualeCervello` (D372): CLM senza addestramento fa 12 su 34, le parole 20, le lettere da 30 a 34. Dove CLM potrebbe avere un vantaggio pratico, ancora da misurare: **molti candidati** (i 58 strumenti del modello di casa, i pezzi di memoria per `QuantoCentra`), dove le lettere si fermano a ventisei e CLM tiene i vettori dei candidati già calcolati; **molte domande sullo stesso stato**, dove un vettore solo serve a tutte; e **imparare dalle lettere**: il modello grande etichetta, con le lettere, migliaia di compiti, e le teste di CLM si addestrano su quelle etichette, su Colab come proponeva Gio. Il primo dei tre l'ho misurato (6 ottobre, `misure/banco_strumento_clm.py`): fra i 58 strumenti CLM mette primo quello giusto 8 volte su 40 nel formato migliore, BM25 19. Resta l'addestramento, e per gli strumenti i dati ci sarebbero gia': ogni volta che il modello grande sceglie uno strumento per una richiesta, quella coppia e' un esempio etichettato. Dal 6 ottobre NOVA conserva la richiesta accanto agli strumenti usati (D374, `decisioni.jsonl`). **Addestrato lo stesso giorno** (D375, `misure/clm_addestra.py`) su dati sintetici etichettati dal modello grande: `QualeCervello` da 27 a 29 su 34, lo strumento giusto primo fra 58 da 27 a 32 volte su 40. **Entrato il 7 ottobre** (D378) dove le lettere non ci sono: chi usa come motore rapido un cervello di fuori. Prima delle lettere avrebbe peggiorato (sul PC di sviluppo le lettere di Gemma fanno 33 su 34). La soglia c'e': 0,8, scelta su 482 compiti nuovi, 95,5% d'accordo col maestro su quelli che decide. Restano aperte: CLM per scegliere gli strumenti da offrire al modello di casa (senza buttare la cache, e misurando quanto spesso lascia fuori quello giusto); un Qwen3-8B Q4_K_M al posto del Q8_0, che occupa 6.149 MiB invece di 9.347 e fa un vettore in 48 ms invece di 63, ma con teste addestrate sui vettori del Q8_0 va riaddestrato o almeno rimisurato; il giudice per chi ha gia' Qwen3-8B come cervello di casa, dove il server dei vettori e' lo stesso.

## La scala consigliata per altri fornitori

Nata col D379 (7 ottobre). **Cosa.** Oggi il consiglio conosce tre cose: il
modello sul PC, Claude Code e Antigravity. Restano fuori Codex (un
abbonamento a OpenAI), le API con una chiave (OpenRouter, Groq, Together) e
Gemini CLI con una licenza enterprise. **Perche'.** «Si usa quel che si ha»
vale anche per chi ha solo quelli. **Cosa costerebbe.** Per Codex sapere se
ha fatto l'accesso e con quali modelli, e una ricetta decisa con Gio; per le
API, una chiave c'e' gia' in configurazione, ma quale modello sia «rapido» e
quale «difficile» dipende dal fornitore. E prima una verifica che manca
anche per Antigravity: cosa stampa `agy models` su un PC dove l'accesso non
e' stato fatto. Oggi un elenco non vuoto vale come accesso fatto, ed e'
verificato solo sul PC di sviluppo, dove l'accesso c'e'.

## I motori spuntati anche fuori dalla scala

Nata col D390 (9 ottobre). **Cosa.** Un motore spuntato e non usato da
nessuna voce oggi resta solo spuntato. Potrebbe essere l'elenco da cui
`cervelli.delega` e `cervelli.secondo_parere` prendono a chi chiedere, e da
cui un Dot sceglie il suo cervello. **Perche'.** La spunta direbbe «questo lo
posso usare», e la scala «questo lo uso da solo»: due cose diverse, oggi
scritte una sola volta. **Cosa costerebbe.** Leggere `brains.routing.motori`
in quei tre posti, decidere cosa fare quando la lista manca (le scale scritte
prima del D390 non ce l'hanno), e una prova per ognuno.

## I file di un comando, e la scheda di un Dot con la domanda

Nate col D391 (9 ottobre). **Cosa.** Due cose che la vista dei Dot
nell'harness non fa. La prima: i file toccati da un comando di shell; oggi
si vedono solo quelli degli strumenti di NOVA. La seconda: guardando un Dot,
chiedere a Nova «cosa sta facendo?» senza nominarlo, come oggi la domanda
porta con se' il file aperto. **Perche'.** Un Dot che programma lavora molto
coi comandi; e la scheda aperta e' il contesto piu' ovvio. **Cosa
costerebbe.** Per la prima, guardare la cartella prima e dopo il comando
(nel recinto si sa dove puo' scrivere), senza indovinare dal testo del
comando; per la seconda, una postilla «sta guardando il Dot X» accanto a
quella dei file (`nova_harness::aperti`), e una prova che la porti.

## Una voce che parla davvero: un modello speech-to-speech davanti a NOVA

Aggiunta da Gio il 7 ottobre: i tempi morti, il microfono e le interruzioni vengono dalla catena voce -> testo -> cervello -> voce, che occupa memoria e somma ritardi. Serve un modello che ascolti e parli da solo, e che passi il lavoro a NOVA. Per dopo, ed e' un di piu': chi ha una chiave la usa, guidato da NOVA, gli altri restano con la voce di oggi.

**Cosa.** Un modello vocale davanti, NOVA dietro: lui tiene la conversazione, e quando serve agire chiama una funzione di NOVA (`delega`, `quale cervello`); decide CLM o, per chi non lo vuole, le lettere del modello o le parole; le cose complesse salgono al gradino successivo, e lui intanto continua a parlare e poi racconta il risultato. E' lo strato «per parlare» dell'idea qui sotto.

**Provato il 7 ottobre**, con una chiave della Gemini API senza credito, dal contenitore di lavoro (negli Stati Uniti, non dall'Italia), con script fuori dal repository e una domanda a voce sintetica (non un microfono):

- `gemini-3.8-live` risponde in italiano, breve e naturale; la domanda a voce l'ha trascritta giusta;
- primo pezzo di voce: da 0,63 a 1,08 s dopo una domanda scritta; da 1,45 a 1,93 s dalla fine di una domanda a voce, compreso il tempo per capire che si e' finito di parlare; una volta su quattro 25,36 s, senza errore, e non so perche';
- la funzione finta `delega`, non bloccante: chiamata da sola dopo 2,69 s riscrivendo il compito per intero, «Ci sto lavorando proprio ora» detto mentre aspettava, il risultato (mandato dopo 4 s) riassunto a voce senza chiederlo;
- token: 282 di voce in uscita per 11 s di audio, circa 26 al secondo;
- sul piano gratuito, dalla console di Gio: richieste al minuto e al giorno illimitate, 65.000 token al minuto (il picco della prova: 575); il tetto vero e' il contesto della sessione, 131.072 token in entrata, che in una conversazione lunga si riempie perche' ogni turno ricarica la conversazione. Il TTS separato (`gemini-3.8-flash-tts`) ha 10 richieste al giorno gratis: con Live non serve.

**Le altre strade guardate** (7 ottobre). In locale: PersonaPlex di NVIDIA (Moshi rifinito, full-duplex, voce e persona dai prompt, licenza commerciale), Moshi di Kyutai e Mini-Omni2 parlano solo inglese e non usano strumenti; un adattatore italiano di Moshi esiste, ma il suo autore lo dice inutilizzabile; Qwen3-Omni parla italiano e chiama funzioni, ma e' un 30B da circa 79 GB in bf16. Nel cloud: GPT-Live-1 di OpenAI (full-duplex, delega a un modello dietro, 5 cent al minuto piu' il modello), la Realtime API (gpt-realtime-mini, da 6 a 15 cent al minuto misurati da altri), ElevenLabs Agents (8 cent al minuto piu' l'LLM; non e' chiaro se sia speech-to-speech).

**Cosa costerebbe.** Un cervello nuovo che parla in WebSocket e in audio, col microfono e l'altoparlante del guscio, e le funzioni di NOVA dichiarate al modello vocale. La voce esce dal PC: e' un cervello di fuori, scelto apposta, mai di serie. Sul piano gratuito, nello Spazio economico europeo i dati non addestrano i modelli di Google (termini della Gemini API, in vigore dal 23 marzo 2026), ma per offrire la Gemini API a utenti europei di un'app i termini chiedono i servizi a pagamento: se NOVA la proponesse a chi la installa, va letto bene. Chi ha un abbonamento Google AI Pro o Ultra ha anche 10 o 100 $ al mese di crediti per la Gemini API (dal 27 gennaio 2026); che valgano per la Live API non e' scritto.

**Fatta l'8 ottobre** (D386, nel piano), con Gemini Live: dopo «Nova», scelta nel pannello. Restano aperte: riprendere dopo una pausa la stessa conversazione di Live con la maniglia, invece di aprirne una nuova (NOVA ricorda gia', il modello vocale no; va verificato quanto vale una maniglia, e se e' scaduta si apre come oggi); il «ferma» riconosciuto sul PC anche durante Live, che oggi passa dal modello; parlarle sopra anche con le casse, togliendo l'eco della voce dal microfono; una seconda voce dal vivo (la Realtime API, GPT-Live-1) dietro le stesse funzioni.

## OpenDots: colleghi sempre accesi, fra testo, chiamate e Slack

Aggiunta da Gio il 5 ottobre, come spinta per NOVA insieme all'harness, e da decidere insieme alle strade di CANT-12 in un'integrazione sola.

**Deciso il 7 ottobre** (nel piano): i Dot si fanno dentro NOVA, in Rust, e da OpenDots si prende solo il disegno. Niente Node ne' Docker; vivono nell'harness, Nova li puo' chiamare, e il primo e' un Dot solo, sempre acceso. Quello che segue resta come lettura di OpenDots.

**Cosa.** [OpenDots](https://github.com/CopilotKit/OpenDots) di CopilotKit, licenza MIT, in alpha: «Always-on AI coworkers that move between text, calls, and Slack». È un modello da cui partire, non un servizio: lo si installa e lo si configura. Ogni agente, un «Dot», ha la sua chat, può parlare in chiamata e rispondere nei thread di Slack, e può avere un computer suo (profilo del browser, file, shell) dentro un contenitore di OpenBot. Le chiamate affiancano alla voce in tempo reale un secondo agente che lavora, così la conversazione va avanti mentre gira il lavoro lungo. Prima di uno strumento una scheda chiede «Approve & save» o «Decline». È in TypeScript, Node.js 24, con React e AG-UI per portare messaggi, chiamate agli strumenti e stato fra il server e l'interfaccia. Il modello è un endpoint compatibile OpenAI (`OPENAI_BASE_URL`), la voce usa la Realtime API di OpenAI, le conversazioni stanno in CopilotKit Intelligence (con `INTELLIGENCE_API_KEY`; la documentazione nomina anche un Intelligence ospitato in proprio), le pagine in un SQLite locale. Di Windows la documentazione non parla. Letto dal README e da `docs/SETUP.md` il 5 ottobre, non provato.

**Perché.** È quello che a NOVA manca fuori dal PC: esserci in Slack e in una chiamata, e lavorare mentre si parla. Le parti si accostano a cose che NOVA ha già. La scheda di approvazione è il ponte delle approvazioni (`caps_approvazione`). Il computer di un Dot è un posto separato dove l'agente lavora, come il recinto. E il demone sa già fare un turno in una conversazione sua, fuori dalla chat (i compiti, D345). L'endpoint compatibile OpenAI potrebbe essere il llama-server di casa, e gli strumenti di NOVA arriverebbero ai Dot dal ponte MCP che c'è già (`nova mcp`), harness compreso: i documenti e i progetti di un Dot diventerebbero quelli dell'harness.

**Cosa costerebbe.** Prima le domande, poi il codice:

- **Cosa esce dal PC.** Con la configurazione del README escono le conversazioni (Intelligence) e la voce (OpenAI), e in `compose.yml` la telemetria di CopilotKit è accesa se non la si spegne (`COPILOTKIT_TELEMETRY_DISABLED`, predefinito `false`). Con `solo_locale` acceso non si può; e anche spento, in una conversazione intera possono esserci proprio le credenziali che il D236 tiene in casa per sempre. A meno di un Intelligence ospitato in proprio e di una voce di casa (`nova-voce`: Kokoro e whisper.cpp). Da verificare se OpenDots accetta un adattatore vocale diverso.
- **Le dipendenze.** Node.js 24, e Docker per i contenitori (`compose.yml`, `deployment/computers`). NOVA ha appena smesso di volere Python per installarsi (D350); un runtime in più va pesato.
- **Due confini.** Il computer di un Dot è un contenitore di OpenBot, il recinto di NOVA su Windows è un AppContainer (D367). Quale vale, quando un Dot usa gli strumenti di NOVA?
- **Su Windows** va provato da zero.

La strada più corta per misurarlo: OpenDots acceso così com'è, con il modello di casa al posto di OpenAI e `nova mcp` come fornitore di strumenti, su un compito vero dell'harness.

## NOVA a strati: uno per parlare, uno per lavorare, e i Dot organizzati come un'azienda

Aggiunta da Gio il 5 ottobre: «si sta sviluppando un ecosistema notevole». Tiene insieme le due voci qui sopra, CANT-12 e OpenDots.

**Cosa.** Tre strati. (1) Un modello piccolo per la conversazione, che risponde subito e chiama quello grande solo quando serve. (2) Un giudice veloce, CLM o le lettere, per le decisioni facili e prima di tutto per quella che le regola tutte: questa richiesta la gestisco io o la passo al cervello grande? (3) Il cervello grande lavora per conto dei Dot, che si organizzano in una struttura «industriale», con un capo, chi guida un gruppo, chi tiene il progetto, chi esegue. Anche le prove e il collaudo (UAT) passano dal giudice veloce. Sotto, quello che NOVA ha già: la memoria (`nova-memoria` e il vault), i permessi e il recinto, l'harness per documenti e progetti.

**Perché.** La forma c'è già in piccolo. `nova-scala` dice che «il modello locale orchestra» e passa la palla a un gradino più alto quando il compito lo supera. La prima decisione del censimento di CANT-12 è proprio `QualeCervello`, che oggi si prende con liste di parole. Un giudice veloce al posto di quelle liste è il primo passo, e serve a tutto il resto. Per le prove c'è un appiglio misurato da altri: dopo un addestramento leggero, CLM è il verificatore migliore pubblicato su due banchi di agenti che scrivono codice, cioè sceglie la soluzione buona fra più candidate (Terminal-Bench 2.1 87,6%, DeepSWE 81,6%, dal README di CLM). Scegliere fra più tentativi è quello che fa un collaudo.

**Cosa costerebbe.** Le misure di oggi frenano due promesse:

- **I millisecondi.** Con CLM sono millisecondi solo le teste, e i candidati già calcolati. Ogni testo nuovo, cioè ogni frase dell'utente, costa un passaggio di Qwen3-8B: 59–92 ms sul PC di sviluppo (GPU RTX 4060 Ti, 16 GB di VRAM; 32 GB di RAM DDR5; scheda madre Gigabyte B650 EAGLE AX; CPU Ryzen 5 7600X), e da 6.149 a 9.362 MiB di VRAM (sopra, in CANT-12). Le lettere costano un passaggio del cervello che è già acceso.
- **Il giudice va addestrato.** Senza addestrarlo, sulle nostre domande CLM sceglie quasi a caso (2 su 10 in italiano). Per instradare o per collaudare servono esempi etichettati. Dal 6 ottobre NOVA registra le sue decisioni (D374), e con dati sintetici etichettati dal modello grande CLM fa da 27 a 29 su 34 a `QualeCervello` (D375).
- **La cascata, misurata** (6 ottobre, `misure/banco_cervelli_fuori.py`). CLM decide quando la sua prima scelta arriva a 0,8, sotto chiama il cervello grande: `QualeCervello` 32 su 34 con 8 chiamate, lo strumento 36 su 40 con 8, dove il grande da solo fa 34 e 40 con tutte le chiamate. La soglia e' stata guardata sugli stessi casi: va scelta su casi nuovi.
- **Due modelli accesi.** Il piccolo per parlare e il grande per lavorare devono stare insieme nella memoria del PC, o il grande va acceso a richiesta, come oggi accende il modello di casa quando serve (D358), pagando l'attesa.
- **Un'azienda di agenti.** Ogni livello in più moltiplica le chiamate al cervello grande, e con loro il costo e i posti dove un errore si propaga. Ogni Dot che agisce passa dai permessi e dal recinto come NOVA, non da regole sue; e un giudizio può solo stringere una guardia, mai allentarla (D313). **Decisa l'8 ottobre** (D385, nel piano): l'APM in cima, AR che assume, un tetto di spesa per progetto che l'utente conferma col via.

L'ordine che ne viene: registrare le decisioni e le correzioni; un giudice per `QualeCervello`, misurato contro le liste di oggi; poi il modello piccolo per parlare; per ultimi i Dot organizzati, quando i primi tre reggono.

## Osservare tutto il sistema, e le istantanee del disco

**Cosa.** Sentire cosa succede sul PC mentre succede (ETW su Windows, eBPF su Linux, EndpointSecurity su macOS). E fotografare il disco prima di un'azione per poter tornare indietro del tutto (VSS, APFS, overlayfs).

**Perché.** Oggi il ritorno indietro è il giornale più il cestino: copre i file che NOVA tocca con i suoi strumenti, non quello che fa un comando qualunque. Un'istantanea coprirebbe tutto.

**Cosa costerebbe.** Da stimare: di codice non ce n'è. Il vecchio README le dava come fatte, e D359 le ha tolte per questo.

## Un prompt su misura per il modello di casa

**Cosa.** Le regole del prompt nominano 26 strumenti che al modello di casa non si offrono: `web_*`, `ui_*` tranne `ui_windows`, `harness_*`, `fascicolo_*`, `avvisi_recenti`, `azione_registra`, `registro_racconta`. Il modello li può chiamare lo stesso per nome, ma senza schema indovina gli argomenti. Si potrebbe comporre il prompt dal pezzo offerto: per il modello di casa solo le regole degli strumenti che ha.

**Perché.** Il Python aveva la stessa incoerenza. Un'istruzione che nomina uno strumento senza schema porta a chiamate con argomenti sbagliati, e ogni tentativo costa un giro.

**Cosa costerebbe.** Aggiungere quegli strumenti al pezzo fisso non si può: prompt e schemi passerebbero da circa 44.800 a circa 61.800 caratteri, cioè circa 15.500 token al rapporto misurato di 0,25 token a carattere. Su 16.384 non resterebbe quasi niente per la conversazione (D361). Servirebbe dividere le regole per strumento, in `nova-contesto`, e una prova che il prompt di ogni cervello nomini solo strumenti che quel cervello riceve.

## Il modello di casa acceso all'avvio

**Cosa.** Accendere llama-server quando parte il demone, invece che alla prima domanda.

**Perché.** La prima risposta non aspetterebbe il caricamento.

**Scartata per ora** (D358): costa gigabyte di memoria video anche a chi quel giorno usa Claude, e la versione Python non l'ha mai fatto. Si può riaprire come impostazione, spenta di serie.

## Riseminare i progetti nuovi

**Cosa.** La prima mappatura del PC si fa una volta sola (D365). Un progetto nato dopo entra in memoria solo se NOVA lo impara parlando, o se si rilancia `novad --semina`. Si potrebbe riguardare le cartelle dei progetti ogni tanto, per esempio una volta alla settimana, e aggiungere solo quelli nuovi.

**Perché.** La mappa invecchia: dopo qualche mese i progetti su cui si lavora non sono più quelli del primo giorno.

**Cosa costerebbe.** Poco codice: `trova_progetti` c'è già. Da decidere cosa fare di un progetto sparito dal disco (archiviarlo o lasciarlo) e come non riscrivere un nodo che l'utente ha corretto a mano: `salva` fonde, ma la fusione di un corpo corretto con quello scansionato va provata.

## Le persone nella prima mappatura — scartata

**Cosa.** Il Python deduceva i collaboratori dai commit dei repository e ne scriveva un nodo per ciascuno, con l'email.

**Perché è scartata.** Scelto con Gio (D365): nel vault finivano nomi ed email di persone che a NOVA non hanno mai detto niente, e ci restavano per sempre. Una persona entra in memoria quando è l'utente a nominarla.

**Cosa costerebbe riprenderla.** Un consenso esplicito, e una prova che nessuna email entri nel vault senza.

## Il recinto nel kernel, a ring 0

**Cosa.** Detto da Gio il 30 settembre, per dopo: arrivera' il momento di lavorare a ring 0, con l'assembly dove serve, e senza compromessi nemmeno con Linux. Il recinto di oggi usa quello che il sistema offre a un programma normale: Landlock su Linux (D301), e su Windows il token ristretto e il job object che sono il primo punto del piano. A ring 0 il confine lo terrebbe codice di NOVA dentro il kernel: un minifilter del file system su Windows; su Linux un programma eBPF agganciato ai controlli di sicurezza del kernel (BPF LSM), o un kernel costruito apposta.

**Perche'.** Da un programma normale certe cose non si chiudono: un comando confinato puo' chiedere a un processo fuori dal recinto di agire al posto suo (il servizio delle attivita' pianificate, WMI, un server COM, D-Bus su Linux). Dal kernel si vede ogni apertura di file e ogni processo che nasce, anche quelli fatti nascere da altri su richiesta del comando: e' il punto da cui quella strada si puo' chiudere, anche se capire chi ha chiesto cosa non e' gratis.

**Cosa costerebbe.** Molto, e va detto intero prima di cominciare. Su Windows un driver del kernel si carica solo firmato: serve un certificato EV e la firma di Microsoft, e un errore non chiude un programma, ferma il PC. Su Linux un modulo esterno non puo' registrarsi fra i controlli di sicurezza del kernel: resta BPF LSM, che va acceso all'avvio del kernel (`lsm=...,bpf`) e che molte distribuzioni tengono spento, oppure un kernel proprio. Le prove girano in macchine virtuali, non sul PC di chi sviluppa. Va dopo il recinto da programma normale, che resta comunque: e' quello che serve a chi non installa un driver.

## Chiudere le cartelle di terzi aperte a tutti i pacchetti

**Cosa.** Le cartelle di terzi aperte a ALL APPLICATION PACKAGES (sul PC di sviluppo, con GPU RTX 4060 Ti, 16 GB di VRAM, 32 GB di RAM DDR5, scheda madre Gigabyte B650 EAGLE AX e CPU Ryzen 5 7600X: le tre di Segnalazione errori di Windows e `NVIDIA Corporation\Drs`) il contenitore le può scrivere, e oggi NOVA le rileva e le dichiara (D367). Un divieto intestato al contenitore non le chiude (provato, con nessuna maschera), e senza ALL APPLICATION PACKAGES PowerShell non parte.

**Perché.** Un confine che dichiara i suoi buchi è onesto, ma resta bucato. Una strada da provare: un token del contenitore con identità di controllo (restricting SID) solo sue, così che la scrittura richieda anche una voce intestata a un'identità che quelle cartelle non hanno.

**Cosa costerebbe.** Una ricerca vera: il token ristretto in scrittura della strada A ha già dato problemi con .NET, e metterlo dentro un contenitore è un territorio non provato. Va misurato prima su PowerShell, su `git` e su `python`.

## L'eseguibile del passo da amministratore in una cartella protetta

**Cosa.** Windows, nella richiesta di conferma, mostra il nome dell'eseguibile e non gli argomenti. `novad.exe` oggi sta in una cartella che l'utente può scrivere, e un altro programma potrebbe sostituirlo mentre si aspetta la conferma.

**Perché.** È il limite di ogni elevazione, e cade se l'installatore mette `novad.exe` in una cartella protetta (Program Files) o se il passo da amministratore è un eseguibile a parte, firmato.

**Cosa costerebbe.** Cambia l'installatore e l'aggiornamento, e per la firma serve un certificato.

## La rete spenta anche su Linux

**Cosa.** `shell_senza_rete` spegne la rete su Windows. Su Linux il racconto dice che la richiesta è ignorata: Landlock non tocca la rete.

**Perché.** Le versioni recenti di Landlock dovrebbero saper limitare le connessioni TCP: da verificare sulla documentazione del kernel prima di promettere qualcosa.

**Cosa costerebbe.** Poco in codice, ma la regola è per porte e non per «tutta la rete»: va deciso cosa lasciare (DNS, il demone stesso).

## Una temporanea per comando anche su Windows

**Cosa.** La cartella temporanea del contenitore la impone Windows, è una sola ed è condivisa fra i comandi in corso. Si svuota quando ne finisce l'ultimo. Su Linux ogni comando ha la sua.

**Perché.** Due comandi insieme possono vedersi i file temporanei.

**Cosa costerebbe.** Da capire se un contenitore per comando (un profilo per comando) è praticabile: ogni profilo ha una cartella e una voce nel registro.

## I residui delle prove nel `%TEMP%`

**Cosa.** Migliaia di voci `nova-*` in `%TEMP%` (8.492 il 2 ottobre sul PC di sviluppo, con GPU RTX 4060 Ti, 16 GB di VRAM, 32 GB di RAM DDR5, scheda madre Gigabyte B650 EAGLE AX e CPU Ryzen 5 7600X: `nova-r2`, `nova-kb`, `nova-q`, `nova-scelta`, `nova-priv`, `nova-segreti`...) sembrano residui delle prove del progetto. Non l'ho verificato.

**Perché.** Ingombrano, e rallentano chi scorre quella cartella. Non sono segreti, ma sono residui.

**Cosa costerebbe.** Trovare quali prove non ripuliscono e farle ripulire; una prova che fallisce se `%TEMP%` cresce dopo una suite.

## Il `SyntaxWarning` di `nova/runtime.py`

**Cosa.** Una prova che analizza i sorgenti Python (`prove/progetto/test_dove_stanno_i_dati.py`) emette `SyntaxWarning: invalid escape sequence` alla riga 191 di `nova/runtime.py`, un docstring che contiene `C:\v1.2.3\`. C'è anche sul commit di partenza.

**Perché.** Con Python più nuovi diventerà un errore.

**Cosa costerebbe.** Una riga: raddoppiare la barra o rendere il docstring grezzo.

## I comandi non confinati di un demone elevato

**Cosa.** Senza `write_roots` non c'e' recinto, e un comando di un demone elevato gira con i poteri del demone (D369). Si potrebbe togliere i poteri anche a quelli.

**Perché.** La prova `test_demone_elevato.py` e D369 promettono che un comando **confinato** non riceve mai i poteri dell'amministratore: i comandi non confinati restano fuori da quella promessa. E per chi vuole amministrare il PC con NOVA elevato, togliere i poteri a tutti toglie anche quella possibilità: è una scelta, non una svista.

**Cosa costerebbe.** Applicare il token ridotto e la console ereditata anche quando `permessi` è vuoto (poche righe, la parte difficile è già fatta), e decidere con Gio se NOVA elevato debba poter fare cose da amministratore.

## Una pseudoconsole al posto della console tutta sua del demone elevato

**Cosa.** Dare a ogni comando confinato di un demone elevato una pseudoconsole (`CreatePseudoConsole`) invece di far ereditare a tutti la console privata del demone.

**Perché.** Oggi il demone elevato lascia la console da cui è partito (i log su console e il Ctrl+C non gli arrivano più) e i comandi in corso insieme condividono la stessa console privata, che può quindi essere letta dall'uno o dall'altro.

**Cosa costerebbe.** Non è misurato se la pseudoconsole soffre dello stesso `0xC0000142` della console nuova con un token ridotto: va misurato prima. Poi è un cambio grande nel punto in cui si crea il processo.

## Il demone elevato senza nessuna console

**Cosa.** Provare in isolamento il caso di un demone elevato che non ha alcuna console (un servizio, un avvio staccato): oggi `console_privata` ne crea una, ma l'unico caso misurato è quello di una console condivisa.

**Perché.** La CI di Windows gira da amministratore e potrebbe non avere una console: nel dubbio la correzione la crea, ma non l'ho visto succedere.

**Cosa costerebbe.** Una prova in un processo avviato staccato (`DETACHED_PROCESS`) da elevato, che lancia un comando confinato e controlla che parta.

## Il titolo della finestra, tagliato, nelle anteprime di `type_text` e `press_keys`

**Cosa.** Nell'anteprima che chiede la conferma, tagliare il titolo della finestra in primo piano oltre una certa lunghezza, in Python (`nova/tools/system.py`) e in Rust (`nova-strumenti`, `capacita.rs`) allo stesso modo.

**Perché.** Il titolo lo decide un altro programma: il 4 ottobre una scheda di Edge ha portato l'anteprima di `press_keys` a 213 caratteri. Chi approva deve leggere la riga intera, e la parte che conta, cosa si sta per fare, finisce in mezzo a un titolo che non ha scelto nessuno.

**Cosa costerebbe.** Poche righe per lato e un confronto gemello sulla stessa regola di taglio. Da decidere con Gio: un titolo tagliato dice meno su **quale** finestra sia, che e' proprio l'informazione per cui c'e' (D143).

## Il motore delle automazioni dice quando fallisce

**Cosa.** `pianificazione.racconta` scrive «motore attivo» se l'attività «NOVA - pianificazione» esiste. Dovrebbe guardare anche com'è finita l'ultima volta (il `LastTaskResult` dell'Utilità di pianificazione), e dire «il motore non parte» quando esce con un errore.

**Perché.** Dal 2 al 5 ottobre l'attività è uscita con 2 a ogni giro, perché lanciava un `nova` di agosto, e chi chiedeva il calendario avrebbe letto «motore attivo». Il controllo prima di registrarla (D370) copre quel caso, non un binario che invecchia dopo.

**Cosa costerebbe.** Leggere l'esito senza dipendere dalla lingua: `schtasks /query /v` stampa intestazioni tradotte, quindi l'API dell'Utilità di pianificazione, o PowerShell. Più lo stesso in `nova/pianificazione.py`, che è il gemello.

## `bin/` vecchio, preferito a `core/target/` più nuovo

**Cosa.** `binario()` (in Rust) e `binari.trova` (in Python) prendono prima l'eseguibile in `bin/`, poi quello in `core/target/release/`. Sul PC di chi sviluppa `bin/` invecchia finché non si rilancia `build.ps1`: il 5 ottobre sul PC di sviluppo (GPU RTX 4060 Ti, 16 GB di VRAM; 32 GB di RAM DDR5; scheda madre Gigabyte B650 EAGLE AX; CPU Ryzen 5 7600X) c'erano un `nova.exe` del 30 agosto e un `novad.exe` del 9 settembre. Si potrebbe avvisare quando il file in `bin/` è più vecchio di quello in `core/target/release/`.

**Perché.** Un eseguibile vecchio non dà errore finché non gli si chiede una cosa nuova, e a quel punto l'errore arriva lontano da dove si capisce.

**Cosa costerebbe.** Un confronto di date in due funzioni gemelle, e decidere con Gio cosa fare: avvisare soltanto, o preferire il più nuovo. Per chi installa `bin/` è l'unico, e non cambia niente.

## Vedere cosa leggono Claude Code e le CLI

**Cosa.** Lanciare Claude Code con `--output-format stream-json` invece di `json`, e raccogliere dalle sue chiamate agli strumenti (`WebFetch`, `WebSearch`, `Read`) gli indirizzi che ha letto, come fa `EsecutoreDemone::viste` con gli strumenti di NOVA. Per le CLI che lo permettono, lo stesso.

**Perché.** Il ricercatore controlla le fonti del rapporto contro quello che il Dot ha letto davvero (D383). Quando un passo lo fa Claude Code, NOVA non vede cosa ha letto, e le sue fonti restano «da verificare»: con Claude in cima alla scala possono esserlo quasi tutte, perche' fa il piano e i passi che il piano gli assegna.

**Cosa costerebbe.** Leggere un flusso di righe JSON invece di un oggetto solo in `lancia_claude`, e misurare che la risposta finale resti la stessa. Per Gemini CLI va guardato se esiste un'uscita equivalente. Va deciso insieme alla domanda aperta su un Dot che lavora su Claude Code (`docs/dots.md`).

## Il custode che chiede a Claude Code senza mani

**Cosa.** Quando nella scala non c'e' nessun cervello che risponde a un indirizzo oltre al modello di casa — solo Claude Code o CLI — il custode dei Dot (D384) non ha a chi chiedere e nega. Potrebbe chiedere a Claude Code lanciato senza strumenti: nessun collegamento MCP, nessuno strumento permesso, una domanda sola.

**Perché.** Chi usa NOVA solo con Claude, e un'autonomia che chiede, oggi si vede negare dal custode tutto quello che Nova gli chiederebbe. Il custode non chiede a Claude Code perche' un giudice che legge testo scritto da altri non deve avere mani.

**Cosa costerebbe.** Va verificato quali opzioni di Claude Code tolgono davvero tutti gli strumenti in modalita' `-p` (una lista vuota di strumenti permessi, `--disallowedTools` per tutti), e provato con un Claude finto che i flag arrivino. Poi va deciso con Gio se basta.

## La consegna di un Dot dentro Gemini Live, e nella conversazione di Nova

Nata col D387, l'8 ottobre.

**Cosa.** Quando un Dot finisce un compito di Nova, oggi l'utente lo legge in chat e lo sente dalla voce di casa. Due cose restano fuori. Durante una conversazione con Gemini Live la consegna si scrive e non si dice: si potrebbe passarla a Live come testo, e Live la racconterebbe con la sua voce nel punto giusto della conversazione. E la consegna non entra nella conversazione di Nova: se l'utente poi chiede «com'e' andata?», Nova la rilegge con `dot.stato`.

**Perché.** Con Live acceso l'utente sta parlando proprio con NOVA, e scoprire solo dopo, in chat, che il lavoro era finito e' il contrario di un compagno che ti avvisa. E una Nova che sa gia' cosa le e' stato consegnato risponde senza un giro di strumenti.

**Cosa costerebbe.** Per Live: un canale dal demone alla sessione aperta (oggi `conversa` ascolta solo il microfono e il server), e un messaggio `clientContent` che non interrompa l'utente mentre parla; va provato come lo prende il modello. Per la conversazione di Nova: un messaggio senza una domanda davanti, o una domanda finta; va visto come lo prendono i cervelli della scala, e la cache del prefisso non cambia perche' si aggiunge in coda.

## I Dot dopo il D388: il capo che cambia, i gruppi col modello di casa, il capo fermato

Nate col D388, il 9 ottobre.

**Cosa.** Tre cose che i Dot fra loro non fanno ancora. Cambiare il capo di un Dot dopo la nascita, o spostarlo in un'altra parte della piramide. Fare i gruppi col modello di casa, che `dot.gruppo` non lo riceve. Fermare un capo e, con lui, i pezzi che ha affidato e che aspetta.

**Perché.** Senza la prima la piramide si sbaglia una volta e resta sbagliata; la faranno AR e l'APM, e ne avranno bisogno. Senza la seconda chi usa NOVA solo col modello del PC non ha gruppi, se non chiedendoli a Claude o alla porta del demone. Senza la terza un capo fermato lascia i sottoposti a lavorare per un compito che nessuno aspetta piu'.

**Cosa costerebbe.** La prima: un campo che cambia in `dot.json`, scritto tutto insieme, e una regola contro i giri (A capo di B capo di A). La seconda: circa cento token di schema in piu' per il modello di casa, che oggi ha 2.100 token per la conversazione contro una soglia di 2.000; si potrebbe accorciare un altro schema, o fare di `dot.gruppo` un modo di `dot.crea`. La terza: `dot.ferma` che chiude anche un compito in attesa e manda il «ferma» ai pezzi aperti, uno per uno.

