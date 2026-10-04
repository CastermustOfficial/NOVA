# idea/

Ogni possibile miglioria, anche solo teorica o non ancora decisa. Per ognuna: cosa, perché, cosa costerebbe. Un'idea non è un impegno: quando si decide di farla passa in [`piano/`](../piano/README.md), quando la si scarta resta qui con il perché.

Aggiornato al 30 settembre 2026. Le idee che c'erano già erano sparse nei documenti: qui sono raccolte, con il rimando a dove sono nate.

## CANT-12: decisioni tipizzate al posto delle euristiche

**Cosa.** NOVA decide molte cose con liste di parole, soglie ed espressioni regolari: quale cervello serve, se una frase è un fatto da ricordare, se un ricordo è pertinente, se una chiamata è rischiosa. Un modello «System One» risponde a domande tipizzate (scelta, punteggio, sì/no) con una probabilità, senza generare testo. Il censimento delle decisioni e il confine su cosa può uscire dal PC sono già scritti in `nova-decisioni`. La metà pura, dai logit al giudizio, è già in `nova-giudizio`. Manca la metà che parla a un modello.

**Le strade raccolte** (in [`docs/verso_la_beta.md`](../docs/verso_la_beta.md), sezione CANT-12):

- **I logit delle lettere**, come Rizzo Flow: una domanda a scelta multipla e le probabilità delle lettere dopo un solo passaggio del modello, con il llama-server che NOVA già accende. Le probabilità si leggono con `n_probs`, e il prefisso comune si riusa con `cache_prompt`: tutti e due vanno verificati prima.
- **CLM-v0.1-8B** (aggiunto da Gio il 28 settembre): un Qwen3-8B congelato più due teste da circa 20M parametri, Apache 2.0. Sul suo banco rende quanto Jev o meglio. Il vettore potrebbe darlo llama-server (`--embeddings --pooling last`), e le teste si scriverebbero in Rust.
- **jevlike**: un classificatore addestrato su un codificatore piccolo. Vuole dati per addestrarlo.

**Perché.** Un conto sbaglia dove i conti sbagliano: una soglia di lunghezza impara le frasi lunghe e inutili e butta «mi chiamo Gio».

**Cosa costerebbe.** Per CLM, prima tre misure: quanto il GGUF quantizzato sposta i vettori rispetto al riferimento, quanto rendono le teste sull'italiano, e se il vettore può venire dallo stesso llama-server o serve un secondo modello acceso. Poi un banco che confronti i giudizi con le euristiche di oggi.

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

**Cosa.** Le cartelle di terzi aperte a ALL APPLICATION PACKAGES (sul PC di sviluppo (GPU RTX 4060 Ti, 16 GB di VRAM; 32 GB di RAM DDR5; scheda madre Gigabyte B650 EAGLE AX; CPU Ryzen 5 7600X), le tre di Segnalazione errori di Windows e `NVIDIA Corporation\Drs`) il contenitore le può scrivere, e oggi NOVA le rileva e le dichiara (D367). Un divieto intestato al contenitore non le chiude (provato, con nessuna maschera), e senza ALL APPLICATION PACKAGES PowerShell non parte.

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

**Cosa.** Migliaia di voci `nova-*` in `%TEMP%` (8.492 il 2 ottobre sul PC di sviluppo (GPU RTX 4060 Ti, 16 GB di VRAM; 32 GB di RAM DDR5; scheda madre Gigabyte B650 EAGLE AX; CPU Ryzen 5 7600X): `nova-r2`, `nova-kb`, `nova-q`, `nova-scelta`, `nova-priv`, `nova-segreti`...) sembrano residui delle prove del progetto. Non l'ho verificato.

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
