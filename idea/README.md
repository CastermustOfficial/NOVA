# idea/

Ogni possibile miglioria, anche solo teorica o non ancora decisa. Per ognuna: cosa, perché, cosa costerebbe. Un'idea non è un impegno: quando si decide di farla passa in [`piano/`](../piano/README.md), quando la si scarta resta qui con il perché.

Aggiornato al 29 settembre 2026. Le idee che c'erano già erano sparse nei documenti: qui sono raccolte, con il rimando a dove sono nate.

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

## Il modello di casa acceso all'avvio

**Cosa.** Accendere llama-server quando parte il demone, invece che alla prima domanda.

**Perché.** La prima risposta non aspetterebbe il caricamento.

**Scartata per ora** (D358): costa gigabyte di memoria video anche a chi quel giorno usa Claude, e la versione Python non l'ha mai fatto. Si può riaprire come impostazione, spenta di serie.
