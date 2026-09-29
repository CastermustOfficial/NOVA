# piano/

Cosa c'è da fare, in ordine. Quando una cosa è fatta si spunta, con il commit che l'ha chiusa.

Il racconto lungo sta in [`docs/verso_la_beta.md`](../docs/verso_la_beta.md): le tre liste, i cantieri, il piano per il Rust e le cinque frasi del cancello della beta. Qui c'è solo l'elenco di quello che resta aperto, nell'ordine in cui si propone di farlo. L'ordine è una proposta da confermare con Gio, non una decisione presa.

Aggiornato al 29 settembre 2026.

## Da fare

1. **La ricerca del demone rimessa in piedi.** Il 29 settembre `rete_cerca` ha fallito cinque ricerche su cinque (D362): DuckDuckGo, chiesto con una richiesta semplice, risponde con pagine senza risultati. La versione Python in quel caso cercava su Bing con un browser senza finestra, su una porta e un profilo suoi (`nova/cerca.py`, porta 9223); nel demone quella strada manca. Anche il messaggio d'errore è sbagliato: dice che «il browser guidato non è ancora collegato al demone», e invece `web_*` nel demone c'è (`nova-browser/src/scaricata.rs`, `nessun_risultato`). Proposto per primo perché senza ricerca il modello di casa, per sapere qualcosa del mondo fuori, deve già conoscere l'indirizzo della pagina; l'ordine va confermato con Gio.
2. **La prima mappatura del PC (seed) nel demone.** La versione Python riempiva la memoria al primo avvio con profilo, progetti, ambiente e persone. Nel demone manca, e un'installazione nuova parte con la memoria vuota (D359). Scelto con Gio: profilo, ambiente, applicazioni e progetti; niente persone e niente email di altri; il profilo tiene il nome git ma non l'email; la lingua viene dalla configurazione.
3. **Il cancello della beta, le frasi che si verificano da qui.** La quarta e la quinta sono avanzate con D359 e D362: i numeri del README adesso sono presi dal demone, e quello della ricerca dice che oggi non funziona. La prima, la seconda e la terza vogliono una macchina che non sia quella di Gio.
4. **Le decisioni che spettano a Gio** (da `verso_la_beta.md`):
   - il motore di ricalcolo dei fogli: `nova-fogli` legge i valori già calcolati e non ricalcola le formule (D281);
   - `build.ps1`: cosa diventa per Mac e Linux;
   - quando si chiude il cancello della beta.
5. **Il recinto per i comandi su Windows.** Su Linux c'è Landlock (D301). Su Windows il token ristretto con il job object non c'è ancora (`recinto.rs`).
6. **CANT-9, Mac e Linux alla pari.** Mancano la tastiera, le finestre e l'albero di accessibilità (AT-SPI2, Accessibility API), più l'avvio automatico. Servono macchine vere per scriverli onestamente.
7. **`nova-mcp-cliente` attaccato a un binario.** Il cancello verso i server MCP di altri è scritto e provato (CANT-11), ma nessun binario lo usa ancora.
8. **I vettori da un modello di embedding.** La versione Python sapeva chiederli a un secondo llama-server (`embedder: "llama"`). Il demone usa solo l'embedding di casa (D359).

## Fatto

Quando una voce si chiude la si sposta qui, con la data e il commit.

- 29/09/2026: i numeri del browser e del prompt nel README, misurati sul demone (D362).
- 29/09/2026: al modello di casa 58 strumenti fissi, e la conversazione si taglia su quel che resta del contesto (D361, `cdcff8a`).
- 29/09/2026: le cinque cartelle di ogni progetto, in NOVA come indici (D360, `47401b4`).
- 28/09/2026: il README descrive il NOVA di oggi (D359, `df563d6`).
- 28/09/2026: il demone accende il modello di casa quando serve (D358, `790c791`).
- 28/09/2026: `novad --registro` (D357, `074273d`).
