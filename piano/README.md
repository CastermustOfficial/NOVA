# piano/

Cosa c'è da fare, in ordine. Quando una cosa è fatta si spunta, con il commit che l'ha chiusa.

Il racconto lungo sta in [`docs/verso_la_beta.md`](../docs/verso_la_beta.md): le tre liste, i cantieri, il piano per il Rust e le cinque frasi del cancello della beta. Qui c'è solo l'elenco di quello che resta aperto, nell'ordine in cui si propone di farlo. L'ordine è una proposta da confermare con Gio, non una decisione presa.

Aggiornato al 29 settembre 2026.

## Da fare

1. **Rimisurare sul demone quattro numeri del README.** Sono i tempi di `web_incolla`, di `web_tabella` e della ricerca senza browser, e il prompt a freddo, che col demone porta 129 schemi invece di sessanta. Oggi il README li dichiara presi dalla versione Python (D359). Si misurano sul PC di Gio.
2. **La prima mappatura del PC (seed) nel demone.** La versione Python riempiva la memoria al primo avvio con profilo, progetti, ambiente e persone. Nel demone manca, e un'installazione nuova parte con la memoria vuota (D359).
3. **Il cancello della beta, le frasi che si verificano da qui.** La quarta e la quinta sono avanzate con D359: restano i numeri del punto 1. La prima, la seconda e la terza vogliono una macchina che non sia quella di Gio.
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

- 28/09/2026: il README descrive il NOVA di oggi (D359, `df563d6`).
- 28/09/2026: il demone accende il modello di casa quando serve (D358, `790c791`).
- 28/09/2026: `novad --registro` (D357, `074273d`).
