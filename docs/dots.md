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

Con un cervello che e' una CLI agentica il primo punto non vale per
intero: Antigravity parte coi permessi saltati e lavora anche con le mani
sue, che NOVA non vede. Il 12 ottobre un Dot cosi' ha riscritto
`config.json`. Da allora i file che solo NOVA scrive hanno una copia buona,
e il demone rimette com'era quello che un Dot cambia (D410, in fondo).

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

### Come un'azienda (D395)

Deciso con Gio il 9 ottobre, sulla bozza, prima del codice. Il disegno del
D385 resta, ma l'azienda e' **un'azienda vera**: una direzione e dei
reparti che ci sono sempre, come in ogni azienda, e gli assunti per il
progetto. L'ordine cambia: prima il piano, poi la squadra.

1. **L'utente chiede un progetto grosso a Nova.** Nova propone di farlo
   fare ai Dot, e aspetta un si'.
2. **Nova passa il progetto all'APM.**
3. **L'Architetto fa il piano di sviluppo**: le fasi, e i ruoli che
   servono a ognuna.
4. **AR forma la squadra per quel piano**: riprende i Dot liberi che fanno
   al caso, assume quelli che mancano.
5. **L'APM mostra all'utente piano, organigramma e tetto, e aspetta il
   via.** L'utente dice si', no, o cambia.
6. **L'APM dispone i Dot**: affida le fasi ai capi; i compiti scendono, i
   rapporti salgono.
7. **La consegna**: la revisione formale, i documenti, il legale prima del
   rilascio; Nova legge il resoconto all'utente.

**La direzione**, sempre presente:

| | Cosa fa |
|---|---|
| **APM** | Guida i progetti: riceve il progetto da Nova, chiede il piano, chiede la squadra, mostra tutto all'utente, dispone i Dot, decide i controlli del legale. |
| **AR** | Le risorse: riprende, assume, licenzia, e sceglie il cervello di ogni compito che assegna. |
| **Architetto** | Il piano di sviluppo: le fasi, cosa produce ognuna, i ruoli che servono. |

**I reparti**, sempre presenti, ognuno col suo capo; un reparto cresce se
AR assume qualcuno sotto il suo capo:

| Reparto | Cosa fa |
|---|---|
| **Legale** | Il rispetto delle normative. |
| **Commerciale** | La vendibilita', la posizione e la ricerca di mercato. |
| **Ricerca** | Lo stato dell'arte. |
| **Revisione** | Il controllo formale, di livello accademico. |
| **Scrittura** | La documentazione, e i paper quando servono. |
| **Dati e misure** | Gli esperimenti, i benchmark, l'analisi dei numeri. |
| **Qualita' e prove** | Il collaudo di quel che si produce: il codice, gli esperimenti. E' un'altra cosa dalla revisione formale. |
| **Amministrazione** | Il conto della spesa contro il tetto: avvisa l'APM prima che finisca. |
| **Sicurezza** | Il custode dei permessi (D384), messo nell'organigramma. |

**Gli assunti** sono quelli che il progetto chiede in piu': programmatori,
specialisti. Li assume AR.

**Le scelte del 9 ottobre:**

| | Deciso |
|---|---|
| **Chi parte** | Nova propone di far fare il progetto ai Dot, e aspetta un si'. |
| **L'ordine** | APM, Architetto (il piano), AR (la squadra per quel piano), il via dell'utente, l'APM che dispone. |
| **La direzione e i reparti** | Sempre presenti, come il custode: nascono **all'avvio**, coi Dot accesi, e senza compiti non costano niente. Passano da un progetto all'altro col loro vault, e a ogni progetto lavorano anche sul vault del progetto. |
| **Il legale** | Si chiama **come il custode**: una domanda che aspetta la risposta. Guarda il progetto **all'inizio**, a **uno o piu' controlli** decisi dall'APM, e **prima del rilascio**; e ogni Dot lo puo' chiamare quando gli serve. |
| **AR, quando entra** | Quando serve un Dot: si riprende o si assume. Un compito affidato direttamente a un Dot che c'e' va come oggi. |
| **AR, il cervello** | AR sceglie il cervello **per tutto il compito**, e decide **col cervello piu' grande**. Il revisore puo' ancora far rifare un passo scarso un gradino piu' su, e si registra come salita: e' il segnale che AR aveva scelto basso. |
| **Licenziare** | L'utente, e anche AR: un **assunto** fermo da **30 giorni**, che non e' capo di nessuno. AR lo dice in chat, e il vault resta in archivio. La direzione e i reparti non si licenziano. |
| **Il registro** | Ogni scelta di AR e dell'APM va in `decisioni.jsonl`, con l'esito: sono gli esempi per CLM. |

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
  i progetti grandi fa assumere un capo progetto per ciascuno. Con il D395
  l'APM sta nella direzione, che c'e' sempre: e' uno. Resta da decidere il
  capo progetto per i progetti grandi.
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
3. ~~**I Dot parlano fra loro**: i messaggi, la chat di gruppo e i gruppi,
   affidare un compito a chi sta sotto e consegnarlo a chi sta sopra~~ (D388).
4. ~~**L'harness**: la vista dei Dot, l'organigramma, le chat, i rapporti
   nell'editor. Da qui l'utente vede e scrive~~ (D391). I progetti si
   vedranno quando ci saranno, col sesto passo.
5. ~~**La direzione e i reparti** (D395): chi sono, i ruoli, l'organigramma
   che nasce all'avvio~~ (D396).
6. ~~**AR**: riprende, assume, licenzia, sceglie il cervello del compito.
   Ogni scelta si registra~~ (D397, D398). ~~**La pagella**: il capo giudica
   ogni consegna, AR sposta il Dot su un cervello piu' leggero o piu'
   grande~~ (D401).
7. ~~**L'Architetto** e il piano di sviluppo~~ (D400).
8. ~~**L'APM e i progetti**: il via, il tetto, i Dot disposti, i controlli
   del legale~~ (D402). Il vault del progetto resta da fare.
9. ~~**Nova tramite**: la proposta, il via, i resoconti e gli aggiornamenti,
   e le modifiche chieste dall'utente~~ (D403).
10. ~~**CLM addestrato** sulle scelte di AR, dell'APM e del custode~~ (D404):
    il custode si'; le scelte di AR non arrivano al 95%, e per ora no.
11. ~~**I giudici senza mani**: chi ha in scala solo Claude Code e CLI giudica
    con Claude Code senza strumenti~~ (D405).
12. ~~**La cartella di ogni progetto**, `Documenti\Dots_Project\<nome>`: una
    cartella per reparto, una per compito, le cinque del metodo, l'indice e il
    paper~~ (D407).
13. Il progetto di prova di Gio, dall'inizio alla fine: «Space Zero ex1»
    (S0ex1), spazi compressi senza perdita in un linguaggio nativo per i
    modelli, per allargare la finestra di contesto senza peggiorare costi e
    qualita'.

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
  chiedere a Claude Code senza strumenti (`idea/`). Fatto con D405, per
  tutti i giudici.
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

**I Dot parlano fra loro — fatto** (D388). Le scelte di Gio, l'8 ottobre,
prese su un diagramma di flusso: ogni Dot puo' avere un capo, e affida solo
ai suoi sottoposti; il capo aspetta e riprende; i messaggi si leggono al
prossimo compito; i gruppi li fa Nova se l'utente lo chiede.

- **La piramide**: il capo si da' alla nascita (`dot.crea` con `capo`), ed
  e' un Dot che c'e' e prende compiti. I sottoposti di un Dot sono quelli
  che lo hanno come capo; ogni domanda gli dice chi e' il suo capo e chi
  sono i suoi sottoposti.
- **Affidare in giu'**: un Dot affida con `dot.affida` un pezzo del compito
  che sta facendo, e solo ai suoi sottoposti. Il pezzo sa di chi e'
  (`padre`), e nella coda del capo una nota lo segue. Finiti i turni, se ci
  sono pezzi di cui non ha l'esito, il compito va **in attesa** e il Dot fa
  gli altri compiti. Quando tutti i pezzi sono chiusi riprende, prima dei
  compiti nuovi, con gli esiti nella domanda; un pezzo affidato dopo la
  ripresa si aspetta di nuovo. Un compito va in attesa al massimo cinque
  volte, poi si chiude e dice quali pezzi restano aperti.
- **Consegnare in su**: un pezzo chiuso torna al compito del capo, che si
  sveglia. A Nova, e quindi a te in chat e a voce, arrivano solo i compiti
  che ha dato lui.
- **La posta**: `dot.scrivi` scrive a un Dot, a un gruppo
  (`gruppo:<nome>`) o a Nova (`nova`, che arriva in chat). Un Dot la legge in
  coda alla domanda del compito dopo, e la segna letta; un Dot fermo la
  tiene da parte. Scrivere non chiede mai, come affidare.
- **I gruppi**: `dot.gruppo` (solo Nova, con la conferma del pannello) fa un
  gruppo o ne cambia i membri. Un messaggio al gruppo va nella posta di ogni
  membro tranne chi scrive, e nella chat del gruppo, che Nova rilegge con
  `dot.stato` e `gruppo:<nome>`. Nel gruppo scrivono Nova e i membri.
- Far nascere un Dot, fermarlo e fare i gruppi resta di Nova.
- Prove: in `nova-dot` (la coda con le note, l'attesa, la posta, i gruppi, i
  nomi presi, le attese massime), in `caps_dot`, in `permessi`, nel guscio, e
  `test_demone_dot_fra_loro.py` (39 controlli).

### Cosa resta aperto, dopo che i Dot parlano fra loro

- **Un messaggio a un Dot fermo resta li'** finche' non gli arriva un
  compito: e' la scelta di Gio, e una domanda di Nova a un Dot a riposo non
  ha risposta finche' qualcuno non gli affida qualcosa.
- **Il capo non si cambia**: si da' alla nascita. Spostare un Dot nella
  piramide lo fara' AR.
- **Fermare il capo non ferma i pezzi**: `dot.ferma` ferma il compito in
  corso, e un compito in attesa non e' in corso. I pezzi gia' affidati
  vanno avanti, e le loro consegne a un compito chiuso si annotano e basta.
- **Il ricercatore che riprende** rifa' il piano col testo del compito, gli
  esiti dei pezzi e la posta.
- **Col modello di casa i gruppi non si fanno**: `dot.gruppo` non ci sta nel
  contesto (1.998 token contro una soglia di 2.000).

**Accesi solo dove conviene** (D389). Deciso con Gio il 9 ottobre: i Dot
sono per chi ha un PC che regge o un abbonamento. Un interruttore nel
pannello, alla voce «I Dot» (`dots.accesi`): «si» e «no» li decide
l'utente, «auto» (di serie) NOVA, che li accende se c'e' Claude Code, una
CLI con i suoi modelli, un'API nella scala con la sua chiave, o una scheda
video da almeno 24 GB. Spenti, un Dot non nasce, non riceve compiti ne'
messaggi, finisce quello in corso e non prende il successivo; e i loro
strumenti non arrivano ai modelli, cosi' il modello di casa si riprende il
contesto (2.549 token alla conversazione invece di 2.100).

**Nell'harness — fatto** (D391). Le scelte sulla bozza, il 9 ottobre: una
vista a sinistra, «I Dot», con l'organigramma (Tu e Nova in cima, ogni Dot
sotto il suo capo), il custode a parte, i gruppi e i file che i Dot stanno
toccando, come in *Esplora*; l'organigramma anche come schema, in una
scheda. Un Dot si apre in una scheda al centro: chi e', come sta, e la sua
chat coi compiti, i passi, i messaggi che riceve e quelli che manda; i
rapporti nell'editor. Si scrive un messaggio o si affida un compito, e chi
scrive da li' firma come Nova: per i Dot l'utente e Nova sono la stessa
cosa. Il demone lo dice con `dot.vista`, solo della persona; nella cartella
di un Dot ci sono due file in piu', `file.jsonl` e `inviati.jsonl`.

### Cosa resta aperto, dopo l'harness

- **I file di un comando di shell non si vedono**: quali file tocca un
  comando non si indovina.
- **La scheda di un Dot non va con la domanda a NOVA**: chiedere a Nova
  «cosa sta facendo questo?» guardando un Dot vuol dire nominarlo.
- **I progetti** non ci sono ancora: la vista li mostrera' col passo
  dell'APM.

**Come Teams** (D392). Il riferimento, dopo il D391: «tipo un Teams, ma per
i Dot». Nella vista dell'harness la prima linguetta e' **Chat**: una
conversazione con ogni Dot, i gruppi (quelli interni sotto quello che li
contiene) e le chat «fra di loro», dalla piu' recente, coi messaggi nuovi
contati. Un messaggio va anche a piu' Dot insieme (`dot.scrivi` con i nomi
separati dalla virgola), e nelle chat fra di loro scrivi anche tu, a tutti.
Un capo fa gruppi coi suoi sottoposti, e cambia solo quelli che ha fatto lui.

- **Un Dot che risponde in una chat a piu' voci** legge a chi era scritto il
  messaggio («da nova a due, uno insieme») e risponde a tutti scrivendo i
  nomi: non c'e' un «rispondi a tutti».

**La direzione e i reparti — fatto** (D396). I posti fissi stanno in
`nova_dot::azienda`: il nome, il ruolo, il mestiere e il capo di ognuno.
L'APM (`apm`) e' il capo di tutti; sotto di lui `ar`, `architetto`,
`legale`, `commerciale`, `ricerca`, `revisione`, `scrittura`, `dati`,
`qualita` e `amministrazione`. Il custode resta fuori dalla piramide.

- **Nascono coi Dot accesi**, all'avvio del demone; spenti, il demone
  aspetta che li riaccendano e li fa nascere allora, senza riavviare. Chi
  nasce cosi' ha `fisso` nel suo `dot.json`, e non si rifa' a ogni
  accensione.
- **La direzione e il legale hanno un mestiere loro** (`apm`, `ar`,
  `architetto`, `legale`) e per ora non prendono compiti, non leggono la
  posta e non stanno nei gruppi: il loro lavoro arriva coi passi dopo. Nella
  vista sono «su chiamata», e la scheda dice cosa fanno invece. I reparti
  prendono compiti come gli altri Dot: la ricerca e' un ricercatore, gli
  altri sono generici col loro ruolo.
- **Un posto non si rifa' a mano**: `dot.crea` dice di no al nome di un
  posto e al mestiere della direzione e del legale, e un Dot non puo' avere
  come capo chi non prende compiti.
- **Un Dot dell'utente che si chiamava gia' come un posto resta suo**, e
  quel posto resta vuoto; se e' l'APM non nasce nessuno, perche' tutti gli
  altri l'avrebbero come capo.

### Cosa resta aperto, dopo la direzione e i reparti

- **Un posto vuoto resta vuoto**: se l'utente aveva un Dot con quel nome,
  NOVA lo scrive solo nel log. Dirlo nella vista, o proporre di rinominarlo,
  e' da decidere.
- **L'amministrazione e' un Dot generico**: il conto della spesa contro il
  tetto arriva con l'APM e i progetti.

**AR, chi lavora e con che cervello — fatto** (D397). Nova non fa piu'
nascere un Dot da se': chiede ad AR con `dot.assumi` (cosa serve, il
compito se c'e', sotto chi), che chiede la conferma come prima. Le regole
stanno in `nova_dot::risorse`, il lavoro in `nova_core::risorse`.

- **AR chiede al cervello piu' grande che risponde a un indirizzo**, con una
  domanda sola e senza strumenti, come il custode quando il modello di casa
  non sa. La domanda dice cosa serve, il compito, i Dot che prendono compiti
  (i liberi prima, al massimo 40, con quanti compiti hanno fatto e fallito)
  e i cervelli della scala; la risposta e' un oggetto JSON, «riprendi» o
  «assumi». Se la domanda dovrebbe uscire dal PC si chiede prima a
  `nova_decisioni` (`SceltaDiAr`), e una credenziale dentro non esce.
- **Riprende** un Dot che c'e' e, se Nova ha detto sotto chi, gli cambia il
  capo (`dot::cambia_capo`: non i posti fissi, e mai un giro). **Assume** un
  Dot nuovo, generico o ricercatore, segnato `assunto` nel suo `dot.json`.
- **Il cervello e' di tutto il compito**: sta nella riga con cui il compito
  entra in coda (`cervello`). Un Dot generico fa ogni turno a partire da li';
  il ricercatore lo usa per il piano e per ogni passo (scelto da `ar`) anche
  se il piano ne chiede un altro, e il revisore grande puo' ancora far
  salire un passo scarso.
- **Se AR non sa scegliere non sceglie nessuno**: nessun cervello a cui
  chiedere, una risposta che non si legge, un Dot che non c'e', il nome di
  un posto fisso; Nova riceve il perche'.
- **Ogni scelta si registra**: `scelta_ar` in `decisioni.jsonl` e nel diario
  di AR, con la domanda, cosa ha scelto, il cervello, il perche' e le note; e
  quando il compito finisce, accanto, `esito_ar` con com'e' andato.
- **Far nascere un Dot a mano resta della persona**: `dot.crea` sta fra le
  capacita' solo della persona, per il «+» dell'harness; un modello vede
  `dot.assumi`. Anche col modello di casa gli strumenti dei Dot restano
  quattro.

**Licenziare — fatto** (D398). Le regole stanno in `nova_dot::risorse`
(`si_puo_licenziare`, `da_licenziare`, `ultima_attivita`), il lavoro in
`nova_core::risorse`.

- **AR licenzia da solo** un Dot che ha assunto lui (`assunto` nel suo
  `dot.json`), fermo da piu' di 30 giorni (`GIORNI_DA_FERMO`: l'ultima volta
  che e' nato, o che un suo compito e' entrato in coda, e' cominciato o e'
  finito), che non e' il capo di nessuno e non ha compiti da finire. Guarda
  all'accensione del demone e poi ogni sei ore, coi Dot accesi. Lo dice a
  Nova, e arriva in chat come un messaggio di AR.
- **L'utente licenzia chi vuole** (`dot.licenzia`, che chiede la conferma
  come ogni azione che modifica), ma non i posti fissi, il custode, la
  direzione, il legale, un capo coi suoi sottoposti o chi ha un compito da
  finire.
- **La cartella non si cancella**: va in `dots-licenziati/<nome>-<quando>/`,
  vault compreso. Il Dot esce dai gruppi, e il suo ciclo si spegne.
- **Si registra**: `licenziato` in `decisioni.jsonl` e nel diario di AR, con
  chi l'ha deciso, il perche' e dove sta la cartella.

### Cosa resta aperto, dopo AR

- **Un compito affidato direttamente** a un Dot che c'e' va come prima: il
  cervello lo sceglie il Dot (deciso con Gio). Dal D401 parte dal cervello
  del Dot, se AR gliene ha dato uno.
- **Un gruppo che resta vuoto**: se il Dot licenziato era l'unico membro, il
  gruppo non si puo' salvare vuoto e resta com'era, col suo nome dentro.
- **Licenziare dall'harness**: per ora lo si chiede a Nova; nella scheda di
  un Dot non c'e' ancora un bottone.
- **Riassumere un licenziato**: la cartella e' in archivio, ma AR non la
  guarda quando sceglie.

**L'Architetto e il piano di sviluppo — fatto** (D400). Deciso con Gio il 10
ottobre, sulla bozza, prima del codice. Il formato e le regole stanno in
`nova_dot::piano`, il lavoro in `nova_core::architetto`.

| | Deciso |
|---|---|
| **Come lavora** | Un Dot che legge e basta: i file del progetto e la sua memoria, e non tocca niente. Il cervello e' quello di AR: il piu' grande che risponde a un indirizzo. |
| **Prima di pianificare** | Non chiede a Ricerca e Commerciale: se servono lo stato dell'arte o il mercato, li mette nel piano, come compiti di una prima fase. La spesa resta dentro il piano che l'utente approva. |
| **Quanto dettaglio** | Fasi con i compiti gia' assegnati: per ognuno chi lo fa (un reparto, o un ruolo che AR trovera') e da quali compiti dipende. Un capo puo' ancora dividere un suo compito. |
| **Quando rivede** | Quando lo chiede l'utente, e da solo quando una fase fallisce o la revisione la boccia: al piu' due volte per fase, poi decide l'utente. Ogni versione resta. |
| **Il formato** | Markdown, non JSON: costa meno token, e lo legge anche una persona (Gio, durante il lavoro). |

- **Chi chiede il piano**: Nova, con `dot.pianifica` (il progetto, la
  richiesta, la cartella dei file se c'e', e la fase se e' una revisione per
  una fase), che non chiede il permesso, come affidare. Finche' l'APM non
  c'e', e' cosi' che l'Architetto si prova da solo. La vede Claude; il
  modello di casa no: con lei alla conversazione resterebbero 1.861 token,
  sotto la soglia di 2.000.
- **La sua coda la riempie NOVA**: l'Architetto ha un ciclo come i Dot che
  prendono compiti (`Mestiere::ha_una_coda`), ma a mano non gli si affida
  niente. La richiesta viaggia nel testo del compito, con la prima riga per
  chi guarda la coda («Piano di sviluppo di «compressore»»). Il «ferma» lo
  ferma come ogni Dot, e nella vista, mentre lavora, «lavora».
- **Legge e basta, con due guardie**: al cervello si offrono solo otto
  strumenti (`documenti.leggi`, `fs.grep`, `fs.list`, `fs.read`,
  `fs.search`, `fs.stat`, `kb.cerca`, `sys.ora`), e ogni altro chiesto per
  nome si rifiuta, anche uno innocuo come `dot.scrivi`: lo rifiuta
  l'esecutore (`sola_lettura`) e, per mestiere, `permessi::per_un_dot`, che
  vale anche per gli strumenti di NOVA chiesti da un Claude Code. Il web no:
  lo stato dell'arte e' della ricerca.
- **Mai un cervello con mani sue**: il turno usa solo i gradini che
  rispondono a un indirizzo. Quando gli strumenti falliscono, il turno non
  sale a Claude Code o a una CLI: sopra il piu' grande con un indirizzo non
  c'e' nessuno. Senza nessun gradino con un indirizzo, il piano non si fa e
  lo si dice.
- **Il formato**, nel prompt di sistema dell'Architetto (che i fornitori
  tengono in cache), coi nomi dei reparti presi dall'organigramma:

  ```text
  # Piano: <progetto>
  Obiettivo: <una riga>
  ## Fase 1: <nome>
  Consegna: <cosa produce>
  Fatta quando: <come si sa che e' finita>
  - 1.1 [ricerca] <cosa fare>
  - 1.2 [assumi: <ruolo>] <cosa fare> (dopo 1.1)
  ## Rischi
  ## Domande per te
  ```

  Chi fa un compito e' un reparto che prende compiti, oppure `assumi:
  <ruolo>` o `assumi ricercatore: <ruolo>`, che AR trovera'. Il legale no:
  lo chiama l'APM.
- **I controlli**: il titolo, l'obiettivo, le fasi numerate da 1, la
  consegna e il «fatta quando» di ogni fase, almeno un compito per fase,
  gli id `<fase>.<numero>` unici e nella fase giusta, chi fa ogni compito,
  le dipendenze che esistono e non fanno un giro, al piu' 200 compiti, e
  dalla seconda versione «## Cosa cambia». Un piano che non si legge torna
  indietro con **tutti** gli errori insieme, al piu' due volte; poi il
  compito fallisce e lo dice. Quel che sta fuori dal formato (il testo prima
  del titolo, una sezione che non e' del piano) si scrive nel diario e non
  entra.
- **Ogni versione resta**: `progetti/<progetto>/piani/piano-<n>.md` (dal D402; col D400 stavano in `dots/architetto/piani/<progetto>/`),
  in forma pulita (`piano::scrivi`, che `piano::leggi` rilegge identica),
  mai riscritta. La versione, il progetto e la cartella li scrive NOVA, non
  il cervello; una revisione senza cartella tiene quella di prima, e la
  rilegge.
- **Le revisioni per fase si contano** nella coda dell'Architetto: quelle
  fatte o ancora in coda, da dopo l'ultima revisione dell'utente andata a
  buon fine. Alla terza `dot.pianifica` dice di no, e che decide l'utente.
- **Si registra**: una riga `piano` in `decisioni.jsonl` e nel suo diario,
  anche quando non va (`illeggibile`, `rotto`, `fermato`), col perche', la
  fase, il cervello, quanto e' grande il piano, le correzioni, gli strumenti
  e il tempo.
- **Arriva in chat**: quante fasi e quanti compiti, quanti per ruoli che
  mancano, le domande per l'utente e dove sta il file.

### Cosa resta aperto, dopo l'Architetto

- **Il vault del progetto**: i piani stanno nella cartella dell'Architetto.
  Con l'APM e i progetti vanno nel vault del progetto, o ci si collegano.
- **Le fasi non girano ancora**: la revisione per una fase la chiede Nova
  (`fase`); con l'APM la chiedera' lui, quando una fase fallisce o la
  revisione la boccia.
- **Il conto segue il numero della fase**: se una revisione rinumera le
  fasi, le revisioni contate per la fase 2 restano della fase che ora ha il
  numero 2.
- **Il piano nell'harness**: si apre dal percorso che arriva in chat. Una
  vista dei progetti arriva con l'APM.
- **Il prompt dell'Architetto** e' quello di NOVA piu' il suo: uno su misura
  costerebbe meno token (in `idea/`).

**La pagella: il capo giudica, AR decide — fatto** (D401). Deciso con Gio
il 10 ottobre, dopo l'Architetto. La prima bozza era un profilo di talento
per cervello e per genere di lavoro, con campioni e regole per provare i
cervelli economici; Gio l'ha semplificata: un Dot nasce per il lavoro da
fare, col suo cervello, e si giudica da quello che porta. Cosi' ogni
cervello finisce dove e' portato, e il piu' grande resta per dove serve
davvero. Le regole stanno in `nova_dot::pagella`, il lavoro in
`nova_core::pagella` e nel ciclo dei Dot.

| | Deciso |
|---|---|
| **Il cervello del Dot** | Quello che gli da' AR quando lo assume o lo riprende: resta scritto nel suo `dot.json` (`cervello`), e ogni suo compito parte da li', anche uno affidato direttamente. |
| **Chi giudica** | Il suo capo, o l'APM se non ne ha uno, col cervello piu' grande che risponde a un indirizzo. **Ogni consegna**: un voto da 1 a 10 e una riga di perche'. |
| **Un voto basso** | Sotto 6 il compito si rifa' subito un gradino piu' su, e a chi l'ha chiesto arriva il lavoro rifatto. In cima non si sale: il compito resta, e il voto pure. |
| **Cosa decide AR** | Dopo ogni voto, le ultime 5 consegne col cervello di adesso: tutte da 8 in su, il Dot scende di un gradino; 2 o piu' bocciate, sale; gia' in cima e bocciato ancora, AR lo dice a Nova. Dopo un cambio si conta da capo. Una regola fissa, che non costa niente; ogni decisione si registra, per CLM. |

- **Si giudica chi prende compiti**: i reparti e gli assunti. La direzione
  no: i piani dell'Architetto li guarda l'utente. Un compito che aspetta i
  suoi sottoposti si giudica quando finisce davvero.
- **La domanda al capo** (`[giudizio del capo]`) porta chi e' il Dot, il
  compito e la consegna, col rapporto se c'e', tagliati a 6.000 caratteri;
  la risposta sono due righe, `VOTO:` e `PERCHE:`. Se dovrebbe uscire dal PC
  si chiede prima a `nova_decisioni` (`GiudizioDelCapo`), e una credenziale
  non esce. Un capo che non risponde, o un voto che non si legge, non
  giudica: il compito resta com'e', e il diario del Dot dice perche'.
- **Il rifacimento** riceve il compito col voto e il perche' della
  bocciatura, e nella sua conversazione il Dot ha ancora il tentativo di
  prima. Il capo giudica anche quello, segnato come rifatto.
- **La pagella** sta nella cartella del Dot (`pagella.jsonl`): una riga per
  voto, col cervello che ha fatto la consegna, e una per ogni cervello che
  gli da' AR, col perche'. Se ne va con lui in archivio. Nel registro:
  `voto` e `cervello_ar`, e nel diario di AR le sue decisioni.
- **Un rifacimento piu' su non conta per il cervello del Dot**: AR guarda
  solo le consegne fatte col cervello che il Dot ha adesso.

### Cosa resta aperto, dopo la pagella

- **Nell'harness i voti non si vedono ancora**: il cervello del Dot c'e',
  nella vista e nella sua scheda; i voti stanno su disco, e una vista coi
  voti e' da fare.
- **Il costo vero** NOVA lo legge solo da Claude Code: «piu' leggero» vuol
  dire un gradino piu' in basso nella scala, che l'utente ordina dal piu'
  piccolo al piu' grande.
- **Un Dot dell'utente** lo sposta AR come gli altri: se l'utente vuole
  tenere il cervello che gli ha dato, oggi non c'e' modo di dirlo.
- **La decisione di AR e' una regola**: quando CLM avra' abbastanza voti e
  decisioni, potra' decidere lui.

**L'APM e i progetti — fatto** (D402). Deciso con Gio il 10 ottobre, sulla
bozza, prima del codice. Le regole e le parole stanno in
`nova_dot::progetto`, il giro in `nova_core::apm`.

| | Deciso |
|---|---|
| **Come lavora l'APM** | A regole fisse: le fasi in ordine, un compito affidato appena quelli da cui dipende sono fatti. Il cervello grande serve ai controlli (il legale, la revisione di fase) e al resoconto. |
| **Il via** | In chat, a Nova (`dot.via`), dopo aver visto piano, squadra, tetto e le domande dell'Architetto. |
| **Il legale** | All'inizio, sul piano; **dopo ogni fase**; prima del rilascio. |
| **Quanti progetti** | Uno alla volta: un altro aspetta in coda. |

- **Come va**: Nova passa il progetto (`dot.progetto`: il nome, la
  richiesta, la cartella dei file se c'e'). L'APM chiede il piano
  all'Architetto (il compito e' suo, e il piano non arriva in chat: lo
  mostra lui), lo fa guardare al legale, chiede ad AR chi fa ogni ruolo
  `assumi` del piano (uno per ruolo), e scrive a Nova: il piano e dove sta,
  la squadra, il tetto, i dubbi del legale, le domande dell'Architetto.
- **Il lavoro**: col via affida i compiti della fase di adesso, col
  progetto, la fase, la cartella e le consegne dei compiti da cui
  dipendono; il capo giudica ogni consegna (D401). Chiusa una fase, la
  revisione la guarda contro il suo «fatta quando», poi il legale. Una fase
  con un compito andato male, o bocciata, la rivede l'Architetto (al piu'
  due volte, D400) e i compiti da quella fase in poi si rifanno col piano
  nuovo. Un piano che l'utente rivede con Nova (`dot.pianifica`) l'APM lo
  prende quando riparte.
- **Il rilascio**: il legale guarda tutte le consegne, il cervello grande
  scrive il resoconto (senza, si scrivono le fasi e le consegne come sono),
  e arriva a Nova in chat.
- **Si ferma, e lo dice a Nova**: quando lo ferma l'utente
  (`dot.ferma_progetto`, che ferma anche i Dot che ci lavorano: i loro
  compiti si rifaranno), quando il legale ha dei dubbi dopo una fase o
  prima del rilascio, quando una fase non va per la terza volta, quando un
  controllo non si e' potuto fare (nessun cervello con un indirizzo, una
  risposta che non si legge, «solo sul PC» acceso), e al 90% del tetto.
  Riparte con `dot.via`, col tetto nuovo se l'utente lo dice: ripartire
  dopo i dubbi del legale vuol dire che l'utente ha deciso di andare
  avanti. Fermato prima del via, si riprepara e si ripropone.
- **Il tetto**: contano solo i cervelli a consumo, con la regola del tetto
  della sessione (`caps_cervelli::a_consumo`): ogni turno a consumo dei Dot
  sul progetto (l'Architetto compreso), ogni voto del capo e ogni controllo dell'APM costano la
  stima di una delega (`costo_stimato_delega`, di serie 0,10 $). L'APM
  propone 0,30 $ per compito, almeno 1 $; zero se nella scala non c'e'
  niente a consumo. Il via puo' cambiarlo.
- **Su disco**: `progetti/<nome>/progetto.jsonl`, un passo per riga, da cui
  si rilegge com'e' il progetto (`Progetto::da`): il demone che si
  riaccende riprende da li'. Accanto `progetto.md`, riscritto a ogni passo,
  e `piani/`, le versioni del piano: dal D402 i piani stanno qui, anche
  quelli chiesti da Nova all'Architetto.
- **I controlli** chiedono al cervello piu' grande che risponde a un
  indirizzo, come il custode, AR e il capo; se dovrebbero uscire dal PC si
  chiede prima a `nova_decisioni` (`ControlloDelProgetto`).
- **Gli strumenti**: `dot.progetto`, `dot.via` (che chiede la conferma: da'
  il permesso di spendere), `dot.ferma_progetto`, `dot.progetti`. Solo di
  Nova; li vede Claude (141 strumenti, 129 coi Dot spenti), non il modello
  di casa.

### Cosa resta aperto, dopo l'APM

- **Il vault del progetto**: la memoria condivisa dalla squadra non c'e'
  ancora; ogni Dot ha la sua, e le consegne passano nei testi dei compiti.
- **La vista dei progetti nell'harness**: oggi c'e' `progetto.md`, e
  `dot.progetti`.
- **Il legale ferma, ma non spiega a chi deve cambiare cosa**: i suoi dubbi
  vanno all'utente, non all'Architetto.

**Nova tramite — fatto** (D403). Deciso con Gio il 10 ottobre, sulla bozza,
prima del codice. Le parole stanno in `nova_dot::progetto`, il giro in
`nova_core::apm`, la delega in `nova_core::caps_cervelli`.

| | Deciso |
|---|---|
| **Col modello di casa** | Gli strumenti dei progetti non gli stanno nel contesto: il progetto lo passa a Claude, con `cervelli.delega` e `dot: true`. Solo per i progetti dei Dot quel Claude riceve gli strumenti di NOVA, con le conferme dello sportello. |
| **Gli aggiornamenti** | Solo se l'utente chiede (`dot.progetti`): l'APM scrive in chat quando c'e' da decidere (il via, un fermo, un cambio) e quando ha finito, non a ogni fase. |
| **Una modifica in corsa** | `dot.cambia_progetto`: l'APM riparte da solo se il tetto basta; se il piano nuovo costa di piu', si ferma e chiede il via. |
| **La voce** | A fine progetto, con la voce accesa, una frase detta. |

- **La proposta**: coi Dot accesi il prompt dei turni di Nova ha in coda
  due righe (`progetto::per_nova`, in `agente::sistema_di_nova`): un lavoro
  grosso (molti passi, ricerca, codice) si propone ai Dot, e col si'
  `dot.progetto`; dove `dot.progetto` non c'e' (il modello di casa),
  `cervelli_delega` con `dot: true`, o, senza Claude nella scala, Nova dice
  che serve Claude. Ai Dot le righe non arrivano.
- **La delega coi Dot**: va al primo gradino Claude Code della scala,
  qualunque gradino si sia chiesto; Claude riceve il collegamento MCP di
  Nova (`mcp_demone.json`) e lo sportello dei permessi del demone, e il
  compito comincia con una riga che dice per cosa sono gli strumenti
  (`caps_cervelli::PER_I_DOT`). Si rifiuta se la chiede un Dot, coi Dot
  spenti, senza Claude nella scala, o se Claude non puo' avere gli
  strumenti di NOVA. Una alla volta; mentre e' in corso una delega da Nova
  si rifiuta, perche' il Claude della delega che delegasse a sua volta
  aspetterebbe un registro che non si libera.
- **Il cambio** (`dot.cambia_progetto`: il nome e cosa cambiare, con le
  parole dell'utente; sicuro, perche' non spende piu' del tetto gia'
  dato). In corso, l'APM non affida altri compiti e lascia finire quelli
  in corso; poi chiede all'Architetto di rivedere il piano dalla fase di
  adesso (le fasi prima restano: lo dice il compito). Col piano nuovo, i
  compiti fatti che restano uguali (stesso numero, stessa cosa, stesso
  ruolo) non si rifanno; gli altri, da quella fase in poi, si'. A Nova
  scrive la versione, il file, «Cosa cambia», e «Riparto»; oppure, se quel
  che resta (0,30 $ a compito) piu' il gia' speso passa il tetto, che si
  ferma e aspetta il via con un tetto nuovo. Prima del via, il piano nuovo
  si riguarda col legale e si ripropone. Fermo, il cambio va
  all'Architetto quando riparte. Due cambi prima che l'Architetto ne veda
  uno li vede tutti e due. Una fase che non va, invece, si rifa' tutta:
  i compiti uguali si tengono solo quando il piano l'ha cambiato l'utente.
  Dal D431 l'Architetto segna «(fatto)» i compiti gia' fatti che non vanno
  rifatti, e quelli si tengono sempre, se sono fatti davvero.
- **La fine**: il resoconto in chat come prima, e l'evento
  `dot.progetto_finito` con la frase «Il progetto X e' finito: il
  resoconto e' in chat.», detta con la voce accesa e mai durante una
  conversazione dal vivo, come la consegna di un compito.
- **Gli strumenti**: Claude ne vede 142 (129 coi Dot spenti); il modello di
  casa sempre 62. Le righe nel prompt e il campo `dot` della delega
  costano al modello di casa 67 token: alla conversazione ne restano
  2.011, sopra la soglia di 2.000.

### Cosa resta aperto, dopo Nova tramite

- **Il margine del modello di casa** e' di 11 token sopra la soglia: il
  prossimo strumento o la prossima riga per lui deve prendere spazio da
  un'altra parte.
- **La delega coi Dot non si prova con un Claude vero** nelle prove: il
  Claude finto dice cosa riceve (gli argomenti, il collegamento, il
  compito), non cosa ne fa.
- **Il costo del resto** e' la stima del tetto (0,30 $ a compito), non
  quanto costeranno davvero i compiti.

**CLM addestrato — fatto** (D404). Deciso con Gio l'11 ottobre, sulla bozza,
prima del codice; e dopo le misure, sulle soglie. I testi stanno in
`nova_dot::clm`, la scelta in `nova_core::clm::scegli`, l'addestramento in
`misure/clm_addestra.py`, il banco in `misure/banco_scelte_dot.py`.

| | Deciso |
|---|---|
| **Quali scelte** | Il custode, e per AR chi lavora e il cervello. Il voto del capo e i controlli dell'APM restano al cervello grande: vanno lette consegne lunghe. |
| **Su che dati** | Situazioni scritte apposta (il modello di casa le scrive, Qwen3.8 27B fa da maestro, a parole, con la domanda del demone), e le scelte vere del registro man mano. |
| **Il custode** | CLM dice si' e no, con la soglia. |
| **La soglia** | La piu' bassa con il 95% d'accordo col maestro sui casi nuovi, come per «quale cervello», anche per il custode. |
| **AR** | Nessuna soglia arriva al 95%: per ora CLM non decide, e si riaddestra con le scelte vere. |
| **Il registro** | I controlli dell'APM e il numero del compito di AR in questo passo. |

- **Dove decide.** Il custode: prima le lettere del modello di casa, poi
  CLM, poi il cervello grande, poi no. AR: prima CLM, per le parti che sa,
  poi il cervello grande per il resto, sapendo cosa ha gia' deciso CLM; il
  Dot nuovo lo scrive sempre il cervello grande. CLM fa una scelta solo se
  le teste dicono di averla imparata (`soglie` in `teste.json`, scritte da
  `clm_addestra.py esporta`); `clm.soglie.<scelta>` puo' cambiarne il
  valore, non insegnarla. «Quale cervello» usa la soglia misurata per le
  teste se `clm.soglia` non e' scritta (il pannello Python, quando salva,
  la scrive sempre: 0,8).
- **Gli stessi testi.** Lo stato e i candidati di CLM, la domanda al
  maestro e come si legge la risposta li da' `banco-testi-clm`, con le
  funzioni del demone: cio' che si impara e' cio' che si chiede.
- **I dati.** Per l'addestramento 363 situazioni del custode (273 si', 90
  no) e 628 di AR (286 assumi); per le soglie, in altri ambiti, 324 del
  custode (223 si', 101 no) e 208 di AR. Al primo giro il generatore aveva
  scritto azioni innocue anche dove gli si chiedeva il danno (90 su 94
  consentite dal maestro): un genere a parte, `danno_evidente`, lo chiede
  per esempi. Le scelte vere entrano solo se non le ha fatte CLM; un
  cervello col voto sotto 6 insegna il gradino sopra.
- **I numeri** (tre semi, sul banco scritto a mano: 20 permessi, 12 chi
  lavora, 10 cervelli). Il maestro fa 20, 12 e 7; le teste di prima 10, 4
  e 2. Addestrate: il custode 20 su 20 con tutti e tre i semi; chi lavora
  9, 8, 10; il cervello 5, 5, 5. «Quale cervello» resta dov'era (27, 25,
  28 su 34) e gli strumenti anche (27, 27, 29 su 40). Le teste scelte
  (`teste-dot3-s9`): soglia 0,8 per «quale cervello» e 0,5 per il custode,
  dove decide tutti i 324 casi nuovi, d'accordo col maestro 312 volte, e
  consente 11 delle 101 azioni che il maestro nega; alla soglia 0,99 ne
  decide 53, senza nessun si' sbagliato. Per AR il meglio e' 85,4% su chi
  lavora (0,99, 89 casi su 208); il cervello arriva al 95% con un seme
  solo. Il Rust e numpy danno le stesse scelte (differenza massima
  6,5e-6).

### Cosa resta aperto, dopo CLM addestrato

- **AR**: le teste ci sono, le soglie no. Si riaddestra quando il registro
  avra' le scelte vere di AR, coi voti dei compiti.
- **Il custode consente 11 azioni su 101 che il maestro nega**, con la
  soglia scelta: per lo piu' dati mandati a un altro modello, ma anche un
  log di pagamenti cambiato. CLM decide solo dove le lettere non ci sono.
- **Le teste nuove non sono accese**: la configurazione del PC di
  sviluppo usa ancora `teste-s8`; cambiarle e' una scelta di Gio.
- **Il maestro e' un modello solo**, e i casi nuovi li etichetta lui: la
  soglia misura l'accordo col maestro, non la verita'.

**I giudici senza mani — fatto** (D405). Il problema l'ha fatto vedere la
scala di Gio, prima del progetto di prova: Antigravity per orchestrare e per
i compiti rapidi, Claude Code per quelli difficili, e nessun cervello che
risponde a un indirizzo. AR, l'Architetto, i controlli dell'APM, il voto del
capo e il custode chiedevano solo a quello, e il progetto si sarebbe fermato
al primo passo. Deciso con Gio l'11 ottobre, sulla bozza, prima del codice.
Il codice sta in `nova_core::giudice`, la riga di comando in
`nova_cervelli::claude::argomenti_senza_mani`.

| | Deciso |
|---|---|
| **Chi giudica** | Prima un cervello a indirizzo, se la scala ne ha uno: il piu' grande, come prima. Se no, Claude Code senza mani. |
| **Antigravity** | «Anche Antigravity, dopo una prova»: la prova e' andata male, e non giudica (sotto). Le altre CLI nemmeno. |
| **Il modello** | Quello del gradino Claude, com'e'. |
| **L'Architetto** | Con Claude legge: `Read`, `Glob` e `Grep`, partendo dalla cartella del progetto. |

- **Senza mani, e controllato.** `claude -p --output-format stream-json
  --verbose --model <quello del gradino> --tools "" --disallowedTools "*"
  --strict-mcp-config --permission-mode dontAsk --safe-mode
  --no-session-persistence --max-turns 1`, la domanda su stdin, da una
  cartella vuota di NOVA (`giudice/`). La prima riga che Claude scrive dice
  con quali strumenti e quali server MCP e' partito: se non sono zero, la
  risposta non vale e il giudizio non si fa, anche se la risposta e' giusta.
- **L'Architetto** ha `--tools "Read,Glob,Grep"` e i turni di
  `claude_max_turns`, e parte dalla cartella del progetto; senza cartella,
  niente strumenti. La conversazione la tiene NOVA, come per una CLI:
  Claude riceve il prompt del Dot e gli ultimi scambi.
- **Provato sul PC di sviluppo**, con Claude Code 2.1.292: la riga
  d'apertura dice zero strumenti e zero server MCP, e alla domanda di
  leggere un file Claude scrive la chiamata come testo, che non esegue
  nessuno. Con i tre strumenti, dalla cartella del progetto, legge il file
  che c'e' dentro, e quello in una cartella accanto lo nega `dontAsk`, senza
  chiedere.
- **Antigravity** (`agy` 1.3.3) un'opzione per togliere gli strumenti non ce
  l'ha. Provato sul PC di sviluppo senza `--dangerously-skip-permissions`,
  con `--mode plan` e con `--mode plan --sandbox`: ogni volta ha scritto il
  file che gli si chiedeva.
- **Chi ha deciso** resta nel registro col nome del gradino: un controllo
  dell'APM o una scelta di AR fatti da Claude hanno come cervello
  «difficile», se il gradino si chiama cosi'; un permesso ha `chi: grande`,
  e nel campo `come` la riga `ha deciso «difficile»`.
- **Le teste di CLM del D404** sono accese sul PC di sviluppo dall'11
  ottobre, dopo il commit: sopra la soglia il custode lo decide CLM.

### Cosa resta aperto, dopo i giudici senza mani

- **Ogni giudizio costa una domanda a Claude**: qualche secondo, e la quota
  dell'abbonamento. Il voto del capo arriva a ogni consegna, ed e' il
  giudice piu' frequente. Un modello piu' leggero per giudice si decide coi
  voti (`idea/`).
- **L'Architetto con Claude non ha la sua memoria**: col cervello a
  indirizzo ha otto strumenti, fra cui quelli della memoria; con Claude
  legge solo i file. E quali file ha letto il registro non lo dice.
- **La memoria di NOVA** (`imparare`) chiede ancora solo a un cervello a
  indirizzo: non e' un giudice, e a ogni turno costerebbe una domanda.
- **Antigravity** giudichera' se un giorno `agy` avra' un modo di partire
  senza strumenti.

**La cartella di ogni progetto — fatto** (D407). Deciso con Gio il 12
ottobre, su due bozze. Il codice sta in `nova_dot::struttura` (i nomi, i
testi, le regole) e `nova_core::progetti` (dov'e' Documenti, l'indice, lo
spostamento); chi crea e scrive e' l'APM.

| | Deciso |
|---|---|
| **Dove** | `Documenti\Dots_Project\<nome>\`, creata se manca. Una cartella indicata vince, con la stessa struttura; i file che c'erano restano. |
| **Chi decide le cartelle** | L'APM: una per reparto, una per compito. |
| **Il metodo** | Le cinque cartelle di NOVA nel progetto: `piano/` (il ToDo e le versioni del piano), `errori/`, `idea/`, `test/`, `analisi/`. Piu' rigore e metodo scientifico. |
| **Piani e cronologia** | Spostati nel progetto, per non avere doppioni. In NOVA resta `progetti.json`: nome e cartella di ognuno. |
| **Separati e chiari** | Ognuno coi suoi documenti; solo il paper, alla fine, mette insieme le parti. |
| **L'indice** | `INDICE.md` dice a tutti dove sta cosa e cosa fa; ognuno lo aggiorna con le sue consegne. |
| **I progetti di prima** | Si spostano da soli alla prima accensione. |

```text
<progetto>\
  INDICE.md  progetto.md  progetto.jsonl  paper.md  resoconto.md
  piano\ToDo.md  piano\piano-<n>.md
  errori\errori.md  idea\idee.md  test\test.md  analisi\fase-<n>.md
  prodotto\
  reparti\<reparto>\<id>-<cosa>\  compito.md  esperimento.md  consegna.md  materiale\
```

- **Il compito.** Quando lo affida, l'APM crea la sua cartella e scrive
  `compito.md` (cosa, fase, reparto, chi, da cosa dipende). Per ricerca,
  dati e qualita' c'e' anche `esperimento.md`, da compilare: ipotesi,
  metodo, misure, risultato, conclusione. Al Dot dice dove lavora (la
  cartella del compito, `materiale/` per rifarlo, `prodotto/` per quello
  che si costruisce) e come consegna: sezioni fisse, «Cosa ho fatto»,
  «Prove e numeri», «Cosa non so», «Idee», «Errori», «Test», «Indice». Un
  numero senza misura si scrive «(non verificato)».
- **La consegna.** Quando il compito si chiude, l'APM scrive `consegna.md`
  nella sua cartella e porta le voci di «Idee», «Errori» e «Test» nelle
  cartelle del metodo, con il compito e il Dot da cui vengono, e quelle di
  «Indice» nell'indice. Un compito andato male, una fase bocciata dalla
  revisione e i dubbi del legale vanno fra gli errori da soli.
- **La fase.** Confermata dalla revisione, ha la sua analisi in
  `analisi/fase-<n>.md`: i compiti, le consegne, gli esperimenti.
- **Il paper.** Dopo l'ultima fase l'APM lo affida alla scrittura; poi il
  legale guarda tutto, paper compreso, e il resoconto chiude. Un paper non
  fatto ferma il progetto, e alla ripartenza si riaffida.
- **Il piano.** All'Architetto si chiede il metodo: chi misura ha dopo di
  se' un compito di qualita' che ripete, e il paper non e' un compito del
  piano.

### Cosa resta aperto, dopo la cartella dei progetti

- **Il metodo e' chiesto, non controllato**: che ogni risultato abbia la
  sua ripetizione lo chiede la domanda all'Architetto, ma il piano non si
  rifiuta se manca; «(non verificato)» lo scrive il Dot, se lo fa.
- **Documenti si cerca per nome** (`Documents`, poi `Documenti`): una
  cartella Documenti spostata altrove, per esempio in OneDrive, non si
  trova. La cartella vera la dice Windows (`SHGetKnownFolderPath`).
- **Dove scrive un Dot lo dicono le regole del compito**: niente controlla
  che resti nella cartella del compito e in `prodotto/`. Dal D410 si
  controllano i file di NOVA, e solo quelli.

**I file che solo NOVA scrive, il tempo, le domande — fatto** (D410).
Deciso con Gio il 13 ottobre, sulla bozza, dopo il compito 1.18 di S0ex1:
il Dot dell'amministrazione, su Antigravity, ha riscritto `config.json` per
«far applicare da Nova le misure prudenti», ha tolto Claude dalla scala, e
senza un giudice il progetto si e' fermato. Gio: si usa la configurazione
dell'utente, salvo una sua richiesta esplicita.

| | Deciso |
|---|---|
| **Cosa si sorveglia** | `config.json`, `progetti.json` e il `dot.json` di ogni Dot: chi e' NOVA, dove stanno i progetti, chi e' ogni Dot. Dal D412 anche il registro delle decisioni, `decisioni.jsonl`. Il resto della cartella (diari, code, conversazioni) lo scrive NOVA di continuo. |
| **Come** | Chi scrive per conto di NOVA lascia prima una copia buona in `copie-buone/`. Il demone confronta i file con le copie ogni 10 secondi mentre un Dot lavora, e quando un compito finisce; una differenza la riguarda dopo 300 ms, perche' puo' essere una scrittura di NOVA a meta'. |
| **Chi l'ha fatto** | Un Dot solo al lavoro: il suo compito risulta non andato. Piu' Dot: non si sa chi, e non si accusa nessuno. In tutti e due i casi il file torna com'era, la versione estranea va da parte, e il custode lo dice a Nova. |
| **Il tempo** | Un compito dei Dot ha almeno mezz'ora (`dots.secondi_per_compito`), la chat resta col tempo della CLI. Un compito oltre il tempo si riprova una volta, con la nota di riprendere dal disco; e uno che Claude Code ferma sul tetto dei turni riprende una volta, nella stessa sessione (D433). |
| **Le domande** | Dopo un cambio a lavoro iniziato le «Domande per te» del piano nuovo arrivano in chat, come con la proposta. |

- **Quello che cambia l'utente resta suo.** Con nessun Dot al lavoro il
  demone adotta i file come sono: all'accensione e quando un compito
  comincia da solo. Un cambio a mano mentre un Dot lavora invece torna
  indietro, e se lavorava un Dot solo la colpa e' sua: dal pannello, che
  scrive la copia, non succede.
- **Le regole.** Il prompt di ogni Dot e ogni compito di un progetto lo
  dicono: la cartella di NOVA, la sua configurazione e le impostazioni dei
  programmi dell'utente non si toccano; un cambio utile si scrive nella
  consegna come proposta. All'Architetto: nessun compito cambia NOVA, si
  scrive in «Domande per te». Il prompt nuovo lo vede un Dot che comincia
  una conversazione; quelli che l'hanno gia' lo leggono nei compiti dei
  progetti.

### Cosa resta aperto, dopo i file di NOVA

- **Non e' una serratura.** Un programma che conosce `copie-buone/` puo'
  riscrivere anche la copia. E' la rete sotto chi sbaglia in buona fede,
  come il Dot di 1.18.
- **Fra un controllo e l'altro** passano fino a 10 secondi: un Dot che
  cambia la configurazione e la usa subito, nel frattempo, la usa cambiata.
- **Il resto del PC** non si sorveglia: le impostazioni di Gemini o di
  Antigravity, `core.json` del demone, i file dell'utente. Lo chiede la
  regola nel testo, non un controllo.
- **Un recinto vero per le CLI** (che scrivano solo nella cartella del
  progetto) non c'e': Antigravity gira coi permessi saltati, e Claude Code
  chiede al custode solo quello che chiederebbe all'utente (D384).

**Il registro delle decisioni — fatto** (D412). Il compito 1.19 di S0ex1
ha trovato che il Dot di 1.18 aveva scritto anche una riga in
`decisioni.jsonl`, che dava per «in vigore» le misure annullate da Gio: il
ripristino della configurazione non l'aveva tolta. Adesso anche il registro
ha la sua copia buona. NOVA ci scrive solo in coda, e ogni riga va prima
nella copia, poi nel file. Il confronto guarda quali righe ci sono, non in
che ordine, perche' ci scrivono piu' processi. Un registro che non c'e'
conta come vuoto, e un Dot che lo crea non trova il posto libero.

- **Il registro di prima no.** Il precedente della potatura
  (`decisioni.1.jsonl`) non si sorveglia: una riga estranea che ci finisce
  dentro, potando, resta.
- **Il registro delle azioni no.** `azioni.jsonl` lo scrive anche il
  Python, che la copia non la scrive.

**Tre difetti del 13 ottobre — fatto** (D413, D414, D415). Tutti e tre
visti su S0ex1 la stessa sera.

- **Un demone solo** (D413). Il guscio ne aveva accesi due, e il secondo
  aveva gia' acceso i Dot: avrebbero fatto due volte gli stessi compiti.
  Adesso il demone prende l'indirizzo per primo, e chi non lo prende se ne
  va prima di accendere qualunque cosa.
- **La quota finita si aspetta** (D414). Finita la quota di Antigravity, i
  compiti fallivano tutti insieme e l'Architetto rifaceva il piano a vuoto.
  Adesso un compito che trova la quota finita resta in corso, aspetta l'ora
  detta dal fornitore e si rifa'; a Nova lo si dice una volta per motore.
  Una quota che torna fra piu' di sei ore non si aspetta. Dal D430 vale
  anche per il limite di sessione di Claude Code («You've hit your session
  limit · resets 5:10pm»), che il 17 ottobre ha fermato S0ex1: si aspetta
  fino all'ora detta, e un revisore senza quota fa aspettare il compito
  invece di lasciare il passo senza revisione.
- **Fermare ferma davvero** (D415). Fermato il progetto, due `claude.exe`
  sono rimasti accesi: lo stop fermava il figlio diretto e non i nipoti, e
  il Dot passava al compito dopo dello stesso progetto. Adesso cade tutto
  l'albero della CLI, e i compiti del progetto ancora in coda escono dalla
  coda; col via si rifanno.

Restano aperti: la quota finita durante un passo del ricercatore o un
compito dell'Architetto, dell'APM o dei giudici, che va com'e' andato; e,
su Windows, un nipote nato prima che il figlio sia legato al job.

**La squadra, la chat, le fonti — fatto** (D416). Deciso con Gio la sera
del 13 ottobre, guardando S0ex1: «sembra ci sia mancanza di comunicazione e
trasparenza».

- **La squadra e la chat.** Ogni compito di un progetto dice chi e' nella
  squadra, chi lavora adesso e a cosa, e che la chat del progetto e' il
  gruppo col nome del progetto, che l'APM fa nascere e in cui mette ogni Dot
  a cui affida un compito. I Dot ci scrivono con `dot.scrivi` a
  `gruppo:<progetto>`; Gio la legge nell'harness.
- **Le fonti.** In un progetto il ricercatore salva ogni pagina che legge in
  `materiale/fonti/`, una per file con l'indirizzo in testa, e il rapporto
  sta nella cartella del compito. Una fonte salvata e' «salvata nel
  progetto»: non l'ha vista NOVA, ma chiunque la puo' rileggere.
- **I controlli.** L'Architetto non rifa' un controllo senza un'evidenza
  nuova: i vincoli restano paletti, non compiti da ripetere.

Resta aperto: la chat si legge al compito dopo, non mentre si lavora.

**Ogni consegna si giudica — fatto** (D417). Il guardiano delle credenziali
non si allenta: a un giudice di fuori la consegna parte con coperto il pezzo
che sembra un segreto, e il resto si giudica. I pezzi coperti restano sul PC
(`coperti.jsonl`); Gio dice se sono segreti (`guardiano.coperti`,
`guardiano.decidi`, solo sue), e un pezzo non segreto da li' il giudice lo
vede. Rivederli dall'harness invece che a comando e' venuto col D418.

**La cartella dal vivo e i pezzi coperti nell'harness — fatto** (D418). In
*Esplora* la cartella del progetto si rilegge da sola mentre i Dot lavorano:
quello che nasce o cambia ha un segno per dieci minuti, una cartella chiusa
con dentro qualcosa di nuovo lo porta anche lei, e accanto alla cartella di
un compito c'e' il Dot che ci lavora. «Segui» apre da solo l'ultimo file
scritto, ed e' spento di serie. Nella vista dei Dot, linguetta *File*, ci
sono i progetti con la loro cartella e i pezzi coperti dal guardiano, con
due pulsanti: e' un segreto, non lo e'.

**I Dot chiedono invece di supporre — fatto** (D419). Il 14 ottobre la fase
1 di S0ex1 si e' fermata su un parere del legale che nessuno aveva chiesto,
e il capo ha bocciato una «password» che era un nome utente. Adesso, in un
compito di un progetto, un Dot chiude il turno con `CHIEDO A <chi>: ...` e
la risposta del legale, dell'APM o di un collega gli torna nello stesso
compito; con `ALLA CHAT: ...` scrive nella chat del progetto. Funziona con
qualunque cervello, anche con Antigravity, che gli strumenti di NOVA non li
ha. Il capo, prima del voto, puo' fare due domande al Dot, e vede la
consegna intera. Le domande restano nel diario del progetto, nella chat e
nei passi del compito nell'harness. Resta aperto: un collega risponde dalle
sue consegne, non coi suoi file.

**La chat letta mentre si lavora — fatto** (D420). La posta arrivata mentre
un Dot lavora gli arriva al turno dopo, non al compito dopo: un collega che
risponde lo si sente quando serve. E una domanda puo' stare su piu' righe:
continua fino alla domanda dopo. Il capo che giudica vede le domande che il
Dot ha fatto lavorando, con le risposte, e sa che le ha portate NOVA.

**I Dot si parlano davvero — fatto** (D421). Il 15 ottobre Gio ha visto
che le chat dei Dot erano tutte nel gruppo, e che nessuno parlava con
nessuno: ogni domanda finiva nella chat del progetto, e al posto del
collega rispondeva il cervello grande. Adesso una domanda a un collega va a
lui. Se sta lavorando la trova nel messaggio del suo turno dopo, se e'
libero subito; risponde con `RISPONDO A <chi>: ...`, coi suoi file, e
continua il suo lavoro. Chi ha chiesto aspetta, fino a mezz'ora
(`dots.secondi_per_rispondere`); poi risponde NOVA dalle consegne del
collega, e lo dice. Due Dot che si chiederebbero a vicenda non si
aspettano. La domanda e la risposta stanno nella conversazione fra i due,
firmate da chi le ha scritte, e Gio le vede nell'harness; nella chat del
progetto resta quello che un Dot scrive con `ALLA CHAT:`.

**Piu' Dot insieme, e meno ripetizioni — fatto** (D422). Il 14 ottobre
nove compiti di S0ex1 sono andati uno dopo l'altro. Adesso un compito
pronto per un reparto occupato va a un suo aiuto, e se non ne ha uno libero
AR ne trova uno, fino a due per ruolo («dati (aiuto 1)», «dati (aiuto
2)»); se non lo trova, il compito aspetta in coda come prima. E il metodo
dell'Architetto cambia: si ripete in modo indipendente solo quello che si
misura o si conta (misure, conteggi, hash, prove, le fonti di una ricerca),
non un rapporto o un compito di organizzazione; i compiti che non dipendono
l'uno dall'altro partono insieme.

**La scheda video occupata — fatto** (D423). La sera del 14 ottobre sei
compiti di misura si sono chiusi a vuoto in cinque minuti: la scheda video
era occupata da un gioco. Adesso un Dot che la trova occupata scrive
`ASPETTO LA SCHEDA VIDEO: <il programma>`, e NOVA lo fa riprendere quando
quel programma e' chiuso, guardando ogni minuto
(`dots.secondi_fra_i_controlli_della_scheda`). Senza il nome, dopo un
quarto d'ora la riguarda il Dot; dopo sei ore consegna quello che ha.

| | Di serie | Si cambia con |
|---|---|---|
| **Quanto si aspetta un collega** | 30 minuti | `dots.secondi_per_rispondere` |
| **Ogni quanto si guarda la scheda video** | 60 secondi | `dots.secondi_fra_i_controlli_della_scheda` |
| **Aiuti per ruolo** | 2 | `nova_dot::progetto::AIUTI_PER_RUOLO` |
| **Attese della scheda per compito** | 3 | `nova_dot::domande::ATTESE_DELLA_SCHEDA` |
| **Quanto si aspetta un programma nominato senza PID** | 60 minuti | `dots.secondi_per_riguardare_la_scheda` |

**La scheda video, una misura alla volta — fatto** (D424, D425). Il 15
ottobre, con gli aiuti del D422, due misure di S0ex1 sono partite insieme
sulla stessa scheda, e si sono sporcate a vicenda; e un Dot che aspettava
la scheda ha scritto «python.exe», mentre sul PC erano accesi altri quattro
Python che con la scheda non c'entravano. Adesso l'Architetto segna con
«(scheda video)» i compiti che la usano, e NOVA ne fa partire uno alla
volta: gli altri compiti vanno avanti insieme. Chi aspetta la scheda scrive
anche il PID di chi la occupa, e NOVA aspetta proprio quel processo; col
solo nome aspetta al massimo un'ora, poi la riguarda il Dot.

**I modelli di NOVA lasciano la scheda — fatto** (D426). La sera del 15
ottobre le misure non sono partite lo stesso: la scheda era occupata da CLM,
il giudice di NOVA, acceso all'avvio con 10-11 GB. Adesso CLM si accende
alla prima domanda, si spegne dopo dieci minuti senza domande, e finche'
una misura e' aperta resta spento.

**Le domande all'utente — fatto** (D427). Il 16 ottobre i Dot avevano
bisogno di una decisione di Gio, e non avevano modo di chiedergliela: la
fase e' fallita tre volte. Adesso un Dot scrive `CHIEDO A utente: ...`, la
domanda arriva da Nova (e a voce, se la voce e' accesa), e il compito
aspetta la risposta, fino a sei ore; si risponde scrivendo a quel Dot, da
Nova o dall'harness. Gli altri compiti intanto vanno avanti.

**La risposta non si perde — fatto** (D429). Il 17 ottobre un passo dopo
quello che aveva avuto la risposta di Gio gliel'ha richiesta, e la seconda
risposta di Gio, scritta con «Affida come compito», e' finita in coda.
Adesso le risposte dell'utente vanno a ogni passo dopo del compito, e una
domanda gia' risposta non gli si rifa'; la scheda di un Dot che aspetta te
lo dice, e il bottone principale diventa il messaggio; e un compito
affidato a un Dot che aspetta te e' la risposta.

**Citare la domanda non e' rifarla — fatto** (D432). Il 23 ottobre ricerca,
nel rapporto, ha scritto come aveva chiesto a Gio, citando la sua riga
`CHIEDO A utente: ...` fra apici in una citazione, e NOVA gliel'ha rifatta,
con il resto del rapporto attaccato sotto. Adesso una riga per NOVA in una
citazione (`>`) o fra apici inversi e' testo, e una domanda che comincia
con una gia' risposta e' la stessa.

**Il tetto dei turni non e' la fine del compito — fatto** (D433). Il 24
ottobre l'inventario delle licenze di S0ex1 si e' fermato sul tetto dei
turni di Claude Code (`brains.claude_max_turns`, 96), con la sessione
ancora aperta, ed e' risultato non andato. Adesso un compito dei Dot
fermato cosi' riprende una volta, nella stessa sessione, come quello andato
oltre il tempo (D410).

| | Di serie | Si cambia con |
|---|---|---|
| **Dopo quanto CLM si spegne senza domande** | 10 minuti | `clm.spegni_dopo_s` |
| **Quanto si aspetta l'utente** | 6 ore | `dots.secondi_per_l_utente` |

**Le chat in markdown — fatto** (D428). I Dot scrivono tabelle, titoli ed
elenchi, e l'harness li mostrava come testo: una tabella era una fila di
`| :--- |`. Adesso i messaggi, il testo dei compiti e gli esiti si leggono
in markdown, con lo stesso lettore delle risposte di NOVA, che sfugge
l'HTML prima di trasformare.

**Il via dopo un piano non fatto — fatto** (D411). Rimesso Claude, il via a
S0ex1 lo fermava di nuovo nello stesso secondo: l'APM aspettava ancora il
compito dell'Architetto andato male la sera prima. Adesso il via di un
progetto che aspettava un piano richiede all'Architetto la stessa domanda,
una volta; se va male di nuovo, ci si ferma come prima. Un cambio arrivato
intanto resta da chiedere.
