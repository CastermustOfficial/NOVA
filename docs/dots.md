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
| **Autonomia** | Piena. Un Dot non chiede l'ok all'utente prima di agire: è la cosa più autonoma della piattaforma, fatta per finire il compito. L'utente supervisiona, scrive, ferma. **Quando Nova chiederebbe all'utente, un Dot chiede al custode** (Gio, 8 ottobre, D384): un Dot che decide i permessi degli altri, sempre e solo quello. I Dot sono un piccolo ecosistema che fa le cose da solo, potenzialmente per tutto. |

### Cosa resta anche con l'autonomia piena

L'autonomia toglie le domande, non le guardie. Un Dot:

- lavora con gli **strumenti di NOVA**, e solo con quelli;
- esegue i comandi **nel recinto** (D367), con le cartelle che il recinto
  concede;
- non passa i **comandi vietati** (`safety.forbidden_command_patterns`) né
  le altre guardie che non sono permessi;
- non vede mai le **credenziali** (D236);
- chiede il permesso al **custode** quando Nova lo chiederebbe all'utente,
  con l'autonomia del pannello (D384);
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

L'ordine dal terzo passo in poi l'ha cambiato l'azienda dei Dot (D385, qui
sotto): vale «In che ordine, con l'azienda».

## L'azienda dei Dot

Decisa con Gio l'8 ottobre, prima del codice (D385). I Dot non sono solo
colleghi uno per uno: sono un'azienda che si organizza da sola attorno a un
progetto, e che resta.

### Come va

1. **L'utente chiede a Nova un progetto**, anche grande. L'esempio di Gio,
   che sarà il progetto di prova: cercare nuovi spazi matematici e tecniche
   di addestramento nativo in spazi compressi senza perdita, per allargare la
   finestra di contesto — le dimensioni delle onde audio, la trasformata di
   Fourier, gli spettrogrammi, Huffman, Brotli, o qualunque altro metodo
   esistente o da inventare — per un nuovo «formato» di pensiero, di
   ragionamento e di uscita, da tradurre con un decoder.
2. **Nova lo passa all'APM** (*Artificial Project Manager*), il Dot in cima
   alla gerarchia: il CEO. L'utente sta sopra di lui.
3. **L'APM dice ad AR cosa gli serve.** AR (*Artificial Resources*) è le
   risorse umane dell'azienda: **assume i Dot** — ricercatori, programmatori,
   revisori, capi gruppo — con il mestiere, il ruolo e il cervello adatti, e
   per ogni compito **sceglie il modello** (la nota di Gio del D382).
4. **Si forma la piramide**: chi guida un gruppo, chi esegue, chi rivede. I
   compiti scendono, i rapporti salgono.
5. **L'APM mostra il progetto all'utente e aspetta il via**: il piano,
   l'organigramma, il tetto di spesa. Poi va da solo fino alla fine. L'utente
   lo può fermare o cambiare in corsa.
6. **La squadra lavora**, e consegna; Nova legge il resoconto.

### Le scelte di Gio

| | Deciso |
|---|---|
| **Il via** | L'APM mostra piano, organigramma e costo, e aspetta un sì. Poi è autonomo fino alla fine. |
| **Il tetto** | Uno per progetto: lo propone l'APM nel piano, lo conferma l'utente. Quando sta per finirlo, l'APM chiede se allargarlo. I cervelli gratis (il modello di casa, gli abbonamenti) non contano. |
| **La memoria** | Un vault del progetto, condiviso dalla squadra, e il vault personale di ogni Dot, che se lo porta al progetto dopo: come una persona che cambia lavoro e si ricorda il mestiere. |
| **Dopo il progetto** | I Dot restano, liberi, con la loro esperienza. AR li riprende per il progetto dopo se fanno al caso, prima di assumerne di nuovi. L'utente può licenziarli. |
| **Le chat** | L'utente scrive a chi vuole: la chat di gruppo completa, i gruppi interni che si formano, i rappresentanti, un Dot solo. |
| **Nova** | È il tramite e il vero compagno dell'utente: legge il resoconto finale o un aggiornamento preciso, e se l'utente lo chiede cambia le cose. |
| **I permessi** | Li decide il custode (D384), per tutti i Dot dell'azienda. Le guardie che non sono permessi restano. |

### Cosa non è ancora deciso

- **L'APM è uno solo**, come il custode e AR, e guida tutti i progetti; o
  ogni progetto ha il suo, assunto da AR? La mia proposta: uno solo, che per
  i progetti grandi fa assumere un capo progetto per ciascuno.
- **Quanti Dot lavorano insieme.** Col modello di casa c'è un posto solo
  (D381): i Dot che lo usano vanno in fila. Coi cervelli di fuori possono
  lavorare insieme entro i limiti del fornitore, e il tetto del progetto.
- **Cosa vede un Dot degli altri**: il vault del progetto, i messaggi del suo
  gruppo, i rapporti di chi gli sta sotto. Quanto del resto, da decidere
  quando si fanno i messaggi fra Dot.

### In che ordine, con l'azienda

1. ~~Il Dot su disco e il suo ciclo~~ (D382), ~~il ricercatore~~ (D383),
   ~~il custode~~ (D384).
2. ~~**Nova li chiama**: affidare, chiedere lo stato, leggere l'esito~~ (D387).
3. **I Dot parlano fra loro**: i messaggi, la chat di gruppo e i gruppi,
   affidare un compito a chi sta sotto e consegnarlo a chi sta sopra.
4. **L'harness**: la vista dei Dot e dei progetti, l'organigramma, le chat,
   i rapporti nell'editor. Da qui l'utente vede e scrive.
5. **AR**: assume i Dot (mestiere, ruolo, cervello), li riprende, li
   licenzia, sceglie il modello per ogni compito. Ogni scelta si registra.
6. **L'APM e i progetti**: il piano, l'organigramma, il via, il tetto, il
   vault del progetto, i resoconti.
7. **Nova tramite**: i resoconti e gli aggiornamenti, e le modifiche chieste
   dall'utente.
8. **CLM addestrato** sulle scelte di AR e del custode.
9. Il progetto di prova di Gio, dall'inizio alla fine.

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
  *Deciso lo stesso giorno*: le prime due le chiude il custode (D384, qui
  sotto); la terza resta.
- Un compito ripreso dopo un riavvio rifà il piano da capo.

**Il custode — fatto** (D384). Deciso con Gio l'8 ottobre: i Dot hanno
l'autonomia piena, e il permesso lo chiedono a un Dot che si occupa di
questo, sempre. Le tre scelte: si chiede **quando Nova chiederebbe
all'utente**, con l'autonomia del pannello; decide **il modello di casa, se
no il cervello grande**; il custode decide **solo per i Dot**, e Nova
continua a chiedere all'utente.

- **Chi è.** Un Dot col mestiere `custode`, che NOVA fa nascere all'avvio se
  non c'è (`dots/custode/`). È uno solo: nessun altro Dot può chiamarsi così
  o avere quel mestiere, e non prende compiti. Nel suo diario c'è ogni
  permesso che decide.
- **Quando.** Una chiamata di un Dot va al custode quando
  `permessi::si_chiede` direbbe a Nova di chiedere all'utente: con «chiedi
  sempre» ogni azione, con «chiedi se rischioso» quelle pericolose, con
  l'autonomia piena nessuna. Lo sportello e il freno non chiedono mai; gli
  strumenti solo per la persona un Dot non li chiama comunque.
- **Cosa sa.** Il Dot, il suo ruolo, il compito che sta facendo, lo strumento
  col suo rischio, e cosa succederebbe (l'anteprima della capacità).
- **Chi decide.** Prima il modello di casa, con una domanda sì/no letta
  dalle lettere (`giudizio_casa`). Se non c'è, non si accende, o non sa
  decidere, il cervello più grande che risponde a un indirizzo, con una
  domanda sola e senza strumenti: CONSENTI, o NEGA col perché. Claude Code
  e le CLI no: un giudice che legge testo scritto da altri non deve avere
  mani. Se la domanda dovrebbe uscire dal PC si chiede a `nova_decisioni`
  (la decisione `PermessoDiUnDot`): con «solo sul PC» non esce, e una
  credenziale dentro non esce mai.
- **Nel dubbio, no.** Nessun cervello a cui chiedere, una risposta che non
  si legge, una domanda che non può uscire: il Dot legge «AZIONE NEGATA dal
  custode», col perché, e va avanti per un'altra strada.
- **Si registra.** Ogni permesso è una riga `permesso_dot` in
  `decisioni.jsonl` (cosa sapeva il custode, chi ha deciso, cosa, con che
  probabilità), una nel diario del custode, e l'evento `dot.permesso`. Sono
  esempi per CLM, come le scelte del cervello.
- **Il Claude Code di un Dot.** Il collegamento MCP che riceve è legato al
  Dot: il ponte `nova mcp --per-dot <gettone>` si presenta al demone col
  gettone del Dot (`mcp/per_conto_di`), e da lì ogni richiesta gira per conto
  del Dot. Gli strumenti di NOVA che chiama passano dal custode e la memoria
  è quella del Dot; quando vuole usare uno strumento suo e chiede allo
  sportello (`approvazione.claude`), risponde il custode. Il collegamento e il
  prompt di sistema stanno nella cartella del Dot, non nei file di Nova. Un
  gettone che non è di nessun Dot non viene servito. Il gettone è nuovo a
  ogni accensione del demone, e non apre niente che il canale del demone non
  apra già: serve a legare un collegamento al Dot giusto.
- Prove: quattro nuove in `nova_dot::custode`, una in `permessi` (quando un
  Dot chiede al custode, al posto di quella del D382), una in
  `nova_core::custode` (cosa può uscire), una in `decisioni`, una in
  `nova_cervelli::claude` (il gettone nel collegamento); il custode entra
  nella prova del mestiere e nel conto delle decisioni di `nova_decisioni`; e
  `test_demone_custode.py` (30 controlli): il modello di casa che dice sì,
  no e «non so», il cervello di fuori che dice CONSENTI, NEGA e una frase
  che non si legge, l'autonomia piena che non chiede, Nova che chiede ancora
  all'utente, e il Claude di un Dot attraverso il ponte vero.

### Cosa resta aperto, dopo il custode

- **Quello che legge Claude Code NOVA non lo vede**: le sue fonti restano da
  verificare (`idea/`, «Vedere cosa leggono Claude Code e le CLI»).
- **Chi ha in scala solo Claude Code o CLI** e un'autonomia che chiede: il
  custode non ha un cervello senza mani a cui chiedere, e nega. Si potrebbe
  chiedere a Claude Code senza strumenti (`idea/`).
- **Le CLI agentiche** si lanciano senza permessi (Antigravity con
  `--dangerously-skip-permissions`, Gemini CLI con `--approval-mode yolo`,
  in `nova_cervelli::cli`): con le loro mani non passano né dall'utente né
  dal custode, per un Dot come per Nova.

**Nova li chiama — fatto** (D387). Le scelte di Gio, l'8 ottobre: prima che
ci sia AR, Nova fa nascere un Dot **solo se l'utente lo chiede**, con la
conferma del pannello come ogni azione che modifica; affida a un Dot **senza
chiedere il permesso**; e quando un Dot finisce un compito di Nova,
l'utente lo sa **in chat e a voce**.

- **Quattro strumenti per Nova** (`nova_core::caps_dot`): `dot.crea`
  (modifica, con l'anteprima di chi nasce), `dot.affida` (torna subito col
  numero del compito, e scrive `da: "nova"` nella coda), `dot.stato` (senza
  nome tutti, col mestiere e chi prende compiti; col nome i suoi compiti;
  col numero l'esito intero e il rapporto, fino a 20.000 caratteri, col
  taglio detto) e `dot.ferma`. `dot.affida` e `dot.ferma` non chiedono mai,
  nemmeno con «conferma sempre» (`permessi::NON_CHIEDONO_MAI`, accanto al
  freno di Nova).
- **Anche il modello di casa li vede**: `dot.affida`, `dot.crea` e
  `dot.stato` entrano negli strumenti offerti a un cervello in HTTP, che
  passano da 58 a 61. Alla conversazione restano 2.209 token su 16.384
  (erano 2.549; la soglia della prova e' 2.000, e il Python ne lasciava
  1.832). `dot.ferma` resta fuori: il «fermati» di Nova ferma gia' tutti.
- **La consegna**: quando un Dot chiude un compito con `da: "nova"`, il
  demone manda `dot.consegna` con la riga per la chat (com'e' andata, il
  compito in breve e l'esito fino a 1.200 caratteri) e la frase da dire,
  corta e senza percorsi (`nova_dot::consegna`). Il guscio scrive la riga
  nella chat; la frase la dice il demone se la voce e' accesa, ha un motore e
  non c'e' una conversazione con Gemini Live in corso, dove due voci insieme
  non si capirebbero e si scrive soltanto. Un compito dato dall'utente non si
  consegna a Nova.
- **Un Dot non affida ad altri Dot**: lo strumento gli risponde di no, col
  perche'. I Dot che si passano il lavoro sono il passo dopo, coi messaggi e
  i gruppi.
- Prove: quattro in `nova_dot::consegna`, due in `caps_dot`, una in `dot`,
  una in `permessi`, una nel guscio, quelle degli strumenti offerti, e
  `test_demone_nova_e_i_dot.py` (23 controlli, col cervello finto: Nova fa
  nascere un Dot solo col si', affida con «conferma sempre» senza chiedere,
  la consegna arriva con le parole giuste e va a voce solo con la voce
  accesa, un compito dell'utente non si consegna, `dot.stato` coi suoi tre
  modi, un Dot che prova ad affidare si sente dire di no).

### Cosa resta aperto, dopo che Nova li chiama

- **La consegna non entra nella conversazione di Nova**: la vede l'utente,
  in chat e a voce, ma se poi chiede a Nova «com'e' andata?», Nova lo
  rilegge con `dot.stato`. Metterla nella conversazione vorrebbe dire un
  messaggio di Nova senza una domanda davanti, e va visto prima come lo
  prendono i cervelli della scala.
- **Durante Gemini Live** la consegna si scrive e non si dice. Si potrebbe
  passarla a Live, che la racconterebbe con la sua voce (`idea/`).

