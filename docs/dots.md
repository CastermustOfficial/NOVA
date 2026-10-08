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
| **Cervello** | Il Dot pianifica col cervello **più grande** che ha la scala, e a ogni passo assegna il cervello adatto al tipo di passo. Un **revisore** giudica i risultati dei passi fatti coi cervelli piccoli: se sono scarsi, il passo si rifà un gradino più su, anche a lavoro in corso. Nel primo traguardo il revisore è un ruolo dentro il ricercatore (un passo che chiama il cervello grande); con la squadra diventa un Dot suo. |
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
  conversazione.jsonl i messaggi, per riprendere dopo un riavvio
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
3. **Nova lo chiama.** Affidare, chiedere lo stato, leggere l'esito, e
   l'evento a compito finito.
4. **L'harness.** La vista dei Dot, la chat con un Dot, i rapporti aperti
   nell'editor.
5. **Dopo.** Il revisore come Dot suo, gli orari, la squadra (un capo,
   chi guida un gruppo, chi esegue).
