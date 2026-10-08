# I Dot: cosa devono avere

Deciso con Gio il 7 e l'8 ottobre 2026, **prima** di scrivere codice (D381).
Questo file dice com'è fatto un Dot e in che ordine lo si costruisce; com'è
andata si aggiunge in fondo, fase per fase, come in [`harness.md`](harness.md).

## Cos'è

Un Dot è un collega con un nome che **porta a termine un compito da solo**.
Ha un ruolo scritto, una conversazione sua che non si perde, una coda di
compiti e un vault tutto suo. Lavora con gli strumenti di NOVA mentre
l'utente fa altro; l'utente lo guarda lavorare, gli scrive come a un collega
o come un direttore, e se serve lo ferma.

Si fa dentro NOVA, in Rust. Da [OpenDots](https://github.com/CopilotKit/OpenDots)
si prende solo il disegno (un collega sempre acceso che lavora mentre si fa
altro): niente Node, niente Docker. Vive nell'harness, e Nova lo può chiamare.

Il primo è **un ricercatore**, la base della piramide: riceve una domanda,
cerca, legge le fonti e consegna un **rapporto in Markdown con le fonti**.

## Le cinque scelte

| | Deciso |
|---|---|
| **Memoria** | Il vault è **del Dot**: una cartella sua, nello stesso formato di quello di NOVA. NOVA lo può leggere e anche toccare, ma è suo. |
| **Primo Dot** | Il ricercatore. |
| **Cervello** | Il Dot pianifica col cervello **più grande** che ha la scala, e a ogni passo assegna il cervello adatto al tipo di passo. Un **revisore** giudica i risultati dei passi fatti coi cervelli piccoli: se sono scarsi, il passo si rifà un gradino più su, anche a lavoro in corso. Nel primo traguardo il revisore è un ruolo dentro il ricercatore (un passo che chiama il cervello grande); con la squadra diventa un Dot suo. **Per ora decide il cervello grande** (Gio, 8 ottobre). Poi la scelta passa a un Dot specializzato, **AR** (*Artificial Resources*), che per ogni compito, secondo la complessità e i risultati ottenuti, delega a un modello invece che a un altro. **Ogni scelta si registra**, e su quelle scelte si addestra CLM a fare lo stesso: scegliere. |
| **Modello sul PC** | Va secondo le risorse: con un posto solo (`n_parallel`, di serie 1) i Dot che lo usano vanno uno dopo l'altro, e dopo la conversazione con Nova. Coi cervelli di fuori possono lavorare insieme, entro i limiti del fornitore. |
| **Autonomia** | Piena. Un Dot non chiede l'ok prima di agire: è la cosa più autonoma della piattaforma, fatta per finire il compito. L'utente supervisiona, scrive, ferma. |

### Cosa resta anche con l'autonomia piena

L'autonomia toglie le domande, non le guardie. Un Dot:

- lavora con gli **strumenti di NOVA**, e solo con quelli;
- esegue i comandi **nel recinto** (D367), con le cartelle che il recinto
  concede;
- non passa i **comandi vietati** (`safety.forbidden_command_patterns`) né
  le altre guardie che non sono permessi;
- non vede mai le **credenziali** (D236);
- non può chiamare gli strumenti che sono **solo per la persona**
  (`SOLO_PER_LA_PERSONA`: rispondere a un'approvazione, i bottoni delle
  finestre);
- non crea altri Dot e non affida compiti ad altri Dot, fino alla squadra;
- scrive le azioni che non si annullano nel **registro delle azioni**, come
  NOVA.

Un giudizio può solo stringere una guardia, mai allentarla (D313).

## Cosa c'è già, e cosa manca

| Serve | Oggi | Manca |
|---|---|---|
| Una conversazione sua | Il demone tiene fino a 16 conversazioni con un nome (`agente.rs`, `SESSIONI_MASSIME`) | Sono solo in memoria: un riavvio le perde |
| Lavorare con gli strumenti | Il turno usa le capacità del demone (`EsecutoreDemone`) | Niente: è lo stesso turno, con l'autonomia del Dot |
| Lavorare mentre si fa altro | Conversazioni diverse non si aspettano fra loro | Un ciclo nel demone che prende i compiti, e riparte dopo un riavvio |
| Il vault suo | Un vault solo per il PC (`memoria.rs`) | Un vault per Dot, e gli strumenti di memoria che sanno di quale si parla |
| Scegliere il cervello per passo | La scala e la delega (`nova-scala`, D331, D373) | Il piano a passi, il cervello per passo, il revisore e la salita |
| Essere chiamato da Nova | `cervelli.delega` passa un compito e aspetta | Affidare un compito senza aspettare, e sapere com'è andata |
| Farsi vedere | L'harness | Una vista dei Dot, e la chat con un Dot |
| Fermarlo | `azione.ferma` ferma tutto (`interruzione.rs`) | Fermare un Dot solo |

## Dove sta

```
<cartella di NOVA>/dots/<nome>/
  dot.json            chi e': nome, ruolo, quando e' nato
  compiti.jsonl       la coda: affidato, in corso, fatto, fallito, fermato
  conversazione.json  i messaggi, per riprendere dopo un riavvio
  diario.jsonl        cosa ha fatto, passo per passo: cervello, strumenti, esito, salite
  vault/              la sua memoria
  rapporti/           quello che consegna (il ricercatore: un .md per compito)
```

## Come lavora il ricercatore

1. **Riceve una domanda**, da Nova o dall'utente. Il compito va in coda e chi
   l'ha affidato riceve subito un numero, senza aspettare.
2. **Pianifica** col cervello più grande della scala: i passi (cercare,
   leggere una fonte, confrontare, scrivere), e per ognuno il cervello.
3. **Fa i passi.** Ogni passo è un turno nella sua conversazione, col
   cervello assegnato e gli strumenti di NOVA (le ricerche in rete, il
   browser, i file, il suo vault).
4. **Rivede.** Il cervello grande giudica i risultati dei passi fatti coi
   cervelli piccoli; un risultato scarso si rifà un gradino più su, e la
   salita si scrive nel diario.
5. **Consegna** il rapporto in `rapporti/`, con le fonti, e lo dice: un
   evento all'orb e all'harness, e l'esito per Nova.

## In che ordine

1. **Il Dot su disco e il suo ciclo.** Identità, coda, conversazione che
   sopravvive al riavvio, un compito alla volta, fermarlo da solo. Senza
   interfaccia, provato dalle prove del demone con cervelli finti.
2. **Il ricercatore.** Il piano col cervello grande, i passi col cervello
   assegnato, il revisore e la salita, il rapporto con le fonti, il vault suo.
   Ogni scelta del cervello si registra, con com'è andata, per addestrare
   CLM.
3. **Nova lo chiama.** Affidare, chiedere lo stato, leggere l'esito, e
   l'evento a compito finito.
4. **L'harness.** La vista dei Dot, la chat con un Dot, i rapporti aperti
   nell'editor.
5. **Dopo.** AR, il Dot che sceglie i modelli, e CLM addestrato sulle sue
   scelte; il revisore come Dot suo, gli orari, la squadra (un capo,
   chi guida un gruppo, chi esegue).

## Com'e' andata

**Fase 1 — fatta** (D382). Il Dot su disco e il suo ciclo:

- `nova-dot` (crate nuovo, senza turni) sa com'e' fatta la cartella e come
  si legge la coda: `compiti.jsonl` e' un diario di passaggi di stato, e lo
  stato di adesso si ottiene rileggendolo; una riga scritta a meta' si salta,
  un compito chiuso resta chiuso. Si scrive uno alla volta, la riga intera
  in una scrittura sola, e il numero di un compito nuovo si prende insieme
  alla riga che lo mette in coda. La conversazione sta in
  `conversazione.json`, scritta tutta insieme dopo ogni turno e tagliata a
  400 messaggi davanti a una domanda; il diario in `diario.jsonl`.
- `nova_core::dot` accende un ciclo per Dot all'avvio del demone: prende il
  compito piu' vecchio in coda e fa i turni nella conversazione del Dot, fino
  a sei se il modello finisce i passi, con il turno di Nova
  (`agente::turno_in`, separato apposta da `fai_un_turno`).
- Un Dot **non chiede il permesso** (`permessi::per_un_dot`): neanche con
  «conferma sempre» nel pannello. Gli strumenti solo per la persona restano
  suoi. La memoria di NOVA non entra nella sua domanda.
- I suoi eventi sono `dot.stato`, `dot.strumento` e `dot.compito`, col suo
  nome: l'orb di Nova non li segue.
- **Fermarlo** abbandona subito il compito in corso, che si chiude come
  fermato, e il ciclo passa al successivo. Il lavoro gira in un compito
  tokio suo perche' la domanda a un cervello in HTTP aspetta dentro
  `block_in_place`: nello stesso compito il «ferma» sarebbe stato guardato
  solo a risposta arrivata (la prova l'ha visto).
- **Dopo un riavvio** i compiti rimasti in corso tornano in coda, al massimo
  due volte, poi si chiudono come interrotti; la domanda dice al modello che
  il compito e' ripreso.
- Si usa dal demone: `dot/crea`, `dot/affida`, `dot/stato`, `dot/elenco`,
  `dot/ferma`. Non sono capacita': Nova li chiamera' con strumenti suoi
  (fase 3).
- Prove: otto in `nova-dot` (una con otto fili che affidano insieme), una in `nova_core::sessione` (la conversazione
  ripresa), una in `permessi`, e `test_demone_dot.py` con un cervello finto:
  nascere, affidare senza aspettare, scrivere un file con «conferma sempre»
  senza nessuna richiesta, fermare, riprendere dopo un riavvio.

**Fase 2 — fatta** (D383). Il ricercatore:

- Un Dot ha un **mestiere**, scritto in `dot.json`: `generico`, come i Dot
  del D382 (un `dot.json` senza mestiere si legge così), o `ricercatore`. Si
  sceglie alla nascita, con `dot/crea` e `mestiere: "ricercatore"`; una
  parola che non è un mestiere si rifiuta.
- **Il piano** lo fa il cervello più grande della scala: i passi (cerca,
  leggi, confronta, scrivi), al massimo otto, e per ognuno il cervello, coi
  nomi veri dei gradini. Un cervello che la scala non ha fa ripiegare quel
  passo sul cervello grande; un piano che non finisce con «scrivi» ne riceve
  uno; un piano che non si legge diventa quello di ripiego (cerca, leggi,
  scrivi), tutto col cervello grande. Il perché resta nel diario.
- **Ogni passo** è un turno nella conversazione del Dot, che parte dal
  gradino assegnato (`agente::turno_in`, `parti_da`), e continua fino a tre
  turni se il modello finisce i giri di strumenti. La domanda di un passo
  porta i risultati dei passi già fatti, 1.500 caratteri ciascuno: Claude
  Code non vede la conversazione, solo l'ultima domanda.
- **Il revisore** è il cervello grande. Giudica i passi dei cervelli più
  piccoli, col risultato dentro la domanda, e risponde BUONO o SCARSO col
  perché. Un passo scarso si rifà un gradino sopra quello che l'ha fatto, e
  chi lo rifà sa chi l'aveva fatto e cosa ha detto il revisore; i passi del
  cervello grande non si rivedono, perché sopra non c'è nessuno. Un giudizio
  che non comincia né con BUONO né con SCARSO tiene il passo, e il diario lo
  dice. Un cervello piccolo che non risponde fa salire allo stesso modo; il
  cervello grande che non risponde fa fallire il compito.
- **Il rapporto** è la risposta del passo «scrivi», in
  `rapporti/<compito>.md`, con in coda le fonti controllate da NOVA. Un
  indirizzo è *visto* se è passato da uno strumento di NOVA in questo
  compito, negli argomenti o nel risultato di una chiamata andata a buon
  fine; *non visto* se no; *da verificare* se una parte del lavoro l'ha fatta
  Claude Code o una CLI, che leggono con strumenti loro. L'esito del compito
  dice dove sta il rapporto e come stanno le fonti, e l'evento
  `dot.rapporto` lo annuncia.
- **Il vault è suo**: `dots/<nome>/vault/`. Gli strumenti `kb.*` chiamati da
  un Dot lavorano lì: chi chiama lo dice `agente::per_conto_di`, che
  `EsecutoreDemone` mette attorno a ogni capacità. Quello che c'è entra
  nelle domande del Dot come la memoria di Nova nelle sue. Le procedure sono
  di Nova: un Dot non le legge e non le cancella. A Claude Code si nomina il
  vault del Dot. Ogni rapporto lascia una nota nel vault, con dove sta. La
  memoria spenta (`kb.enabled`) è spenta anche per i Dot.
- **Ogni scelta del cervello si registra**, una riga per ogni volta che un
  passo si fa: `cervello_per_passo`, nel diario del Dot e in
  `decisioni.jsonl` (che si spegne con `kb.decisioni`, e dove una credenziale
  non arriva). La riga dice il passo, la scala, il cervello scelto e chi l'ha
  scelto (il piano, un ripiego, la salita), il cervello che ha risposto
  davvero, com'è finito il turno e il giudizio. Sono gli esempi per AR e per
  CLM.
- Nova e i Dot scrivono negli stessi registri, `azioni.jsonl` e
  `decisioni.jsonl`: ora uno alla volta, una riga per scrittura
  (`nova_core::righe`), come la coda dei Dot.
- Prove: tredici in `nova_dot::ricerca` (il piano, il giudizio, gli
  indirizzi, le fonti), una in più in `nova-dot` (il mestiere), una in
  `nova_core::decisioni` (la riga e le credenziali), una in
  `nova_core::righe` (otto fili, ottocento righe, nessuna mescolata), e
  `test_demone_ricercatore.py` (41 controlli, con due cervelli finti e un
  Claude Code finto).

### Cosa resta aperto

- **Un Dot su Claude Code o su una CLI.** Quei cervelli lavorano con
  strumenti loro, e tre cose non tornano. Claude Code, se l'autonomia del
  pannello non è piena, chiede il consenso allo sportello di Nova
  (`approvazione.claude`): lì un Dot **chiede il permesso** come Nova, e
  quello che dice D382 vale solo per i cervelli in HTTP. Gli strumenti di
  memoria che Claude chiama via MCP lavorano sul vault di Nova. E quello che
  leggono NOVA non lo vede: le loro fonti restano da verificare. Come dargli
  l'autonomia di un Dot senza allentare una guardia è da decidere con Gio.
- Un compito ripreso dopo un riavvio rifà il piano da capo.
