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
