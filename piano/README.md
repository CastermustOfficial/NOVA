# piano/

Cosa c'è da fare, in ordine. Quando una cosa è fatta si spunta, con il commit che l'ha chiusa.

Il racconto lungo sta in [`docs/verso_la_beta.md`](../docs/verso_la_beta.md): le tre liste, i cantieri, il piano per il Rust e le cinque frasi del cancello della beta. Qui c'è solo l'elenco di quello che resta aperto, nell'ordine in cui si propone di farlo. L'ordine è una proposta da confermare con Gio, non una decisione presa.

Aggiornato al 30 settembre 2026.

## Da fare

1. **Il cancello della beta, le frasi che si verificano da qui.** La quarta e la quinta sono avanzate con D359, D362, D363 e D365: i numeri del README adesso sono presi dal demone, compreso quello della ricerca, che dal D363 funziona, e la prima mappatura del PC non è più un «manca». La prima, la seconda e la terza vogliono una macchina che non sia quella di Gio.
2. **Le decisioni che spettano a Gio** (da `verso_la_beta.md`):
   - il motore di ricalcolo dei fogli: `nova-fogli` legge i valori già calcolati e non ricalcola le formule (D281);
   - `build.ps1`: cosa diventa per Mac e Linux;
   - quando si chiude il cancello della beta.
3. **Il recinto per i comandi su Windows.** Su Linux c'è Landlock (D301). Su Windows il token ristretto con il job object non c'è ancora (`recinto.rs`).
4. **CANT-9, Mac e Linux alla pari.** Mancano la tastiera, le finestre e l'albero di accessibilità (AT-SPI2, Accessibility API), più l'avvio automatico. Servono macchine vere per scriverli onestamente.
5. **`nova-mcp-cliente` attaccato a un binario.** Il cancello verso i server MCP di altri è scritto e provato (CANT-11), ma nessun binario lo usa ancora.
6. **I vettori da un modello di embedding.** La versione Python sapeva chiederli a un secondo llama-server (`embedder: "llama"`). Il demone usa solo l'embedding di casa (D359).
7. **`kb.enabled` e `kb.inject_context` nel demone.** Il demone non le guarda: la memoria si legge e si scrive anche con `enabled` spento, e il contesto si inietta anche con `inject_context` spento. `enabled` conta solo per creare il vault e seminarlo (D365). Il README adesso lo dice.

## Fatto

Quando una voce si chiude la si sposta qui, con la data e il commit.

- 30/09/2026: la prima mappatura del PC nel demone, e il vault che il demone crea da sé (D365).
- 29/09/2026: `rete.cerca` cerca prima col browser senza finestra, come il Python (D363, `c45dc0a`).
- 29/09/2026: leggere la pagina dei risultati non solleva più, e `test_cerca.py` non accende un Edge vero (D364, `861d0bd`).
- 29/09/2026: i numeri del browser e del prompt nel README, misurati sul demone (D362, `c617080`).
- 29/09/2026: al modello di casa 58 strumenti fissi, e la conversazione si taglia su quel che resta del contesto (D361, `cdcff8a`).
- 29/09/2026: le cinque cartelle di ogni progetto, in NOVA come indici (D360, `47401b4`).
- 28/09/2026: il README descrive il NOVA di oggi (D359, `df563d6`).
- 28/09/2026: il demone accende il modello di casa quando serve (D358, `790c791`).
- 28/09/2026: `novad --registro` (D357, `074273d`).
