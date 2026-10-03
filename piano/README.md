# piano/

Cosa c'è da fare, in ordine. Quando una cosa è fatta si spunta, con il commit che l'ha chiusa.

Il racconto lungo sta in [`docs/verso_la_beta.md`](../docs/verso_la_beta.md): le tre liste, i cantieri, il piano per il Rust e le cinque frasi del cancello della beta. Qui c'è solo l'elenco di quello che resta aperto, nell'ordine in cui lo si fa.

Aggiornato al 2 ottobre 2026.

## Da fare

Deciso con Gio il 30 settembre: non ci sono scadenze, conta solo il risultato, e il codice si tratta come sicurezza militare. L'ordine l'ho scelto io, con un criterio solo: **prima quello che protegge**, poi quello che apre porte, poi il resto.

### Il codice, in ordine

1. **CANT-12, le decisioni che oggi sono euristiche.** `nova-decisioni` (quali decisioni, e cosa può uscire dal PC) e `nova-giudizio` (dai logit al giudizio) sono scritti e non li usa nessun binario. Mancano: la metà che chiede a llama-server (`n_probs`, `cache_prompt`, `/tokenize`), dopo le due verifiche scritte in `verso_la_beta.md`; per ogni decisione l'euristica di oggi e il modello dietro lo stesso tratto; il banco che li confronta sui casi di NOVA. Un giudizio può solo stringere una guardia, mai allentarla (D313).
2. **Una difesa contro le prompt injection.** Chiesta da Gio il 30 settembre, da valutare quasi sicuramente dentro CANT-12. Oggi ci sono pezzi sparsi: le descrizioni dei server MCP di altri arrivano citate e non obbedite (D264), quel che si legge dallo schermo non entra in memoria (D170), le azioni rischiose chiedono conferma (D333), e il recinto di Windows (D367) limita il danno. Manca un disegno unico per tutto il testo che arriva da fuori.
3. **`nova-mcp-cliente` attaccato a un binario.** Il cancello verso i server MCP di altri è scritto e provato (CANT-11), ma nessun binario lo usa. Va dopo il giudizio e la difesa dalle prompt injection, perché è una porta verso l'esterno.
4. **I vettori da un modello di embedding.** La versione Python sapeva chiederli a un secondo llama-server (`embedder: "llama"`). Il demone usa solo l'embedding di casa (D359).
5. **Le compatibilità che sono codice.** Le CLI Codex e Qwen, nel menu e mai provate; i dialetti del tool calling di OpenRouter, Groq e Together; l'installatore per un utente senza diritti di amministratore.
6. **Le ottimizzazioni aperte.** Gli schemi degli strumenti (circa 6.900 token, il blocco più grosso del prompt) e un prompt su misura per il modello di casa; lo speculative decoding; i round-trip del browser su un modulo intero; il modello di casa acceso all'avvio.
7. **CANT-9, Mac e Linux alla pari.** Mancano la tastiera, le finestre e l'albero di accessibilità (AT-SPI2, Accessibility API), l'archivio delle credenziali, il recinto su macOS e l'avvio automatico. Servono macchine vere, e prima la decisione di Gio qui sotto.

### Fuori dal codice

- **Il cancello della beta, le frasi che si verificano da qui.** La quarta e la quinta sono avanzate con D359, D362, D363, D365 e D366. La prima, la seconda e la terza vogliono una macchina che non sia quella di Gio.
- **Le decisioni che spettano a Gio** (da `verso_la_beta.md`):
  - Mac e Linux: una promessa con una data, o NOVA è un programma Windows;
  - automazioni e riparazioni: oggi NOVA si scrive strumenti in Python e si ripara provando le modifiche sul proprio Python, e tutta in Rust vorrebbe dire scegliere in che lingua si scrive da sola;
  - il motore di ricalcolo dei fogli: `nova-fogli` legge i valori già calcolati e non ricalcola le formule (D281);
  - `build.ps1`: cosa diventa per Mac e Linux;
  - quando si chiude il cancello della beta.

## Fatto

Quando una voce si chiude la si sposta qui, con la data e il commit.

- 02/10/2026: il recinto per i comandi su Windows, un contenitore del sistema, e dove non basta lo dice (D367).
- 02/10/2026: le prove del demone su Windows tornano verdi, e quattro difetti veri del prodotto vengono fuori (D368).
- 30/09/2026: `kb.enabled` e `kb.inject_context` valgono anche nel demone (D366, `6b0b752`).
- 30/09/2026: la prima mappatura del PC nel demone, e il vault che il demone crea da sé (D365, `46dd2fc`).
- 29/09/2026: `rete.cerca` cerca prima col browser senza finestra, come il Python (D363, `c45dc0a`).
- 29/09/2026: leggere la pagina dei risultati non solleva più, e `test_cerca.py` non accende un Edge vero (D364, `861d0bd`).
- 29/09/2026: i numeri del browser e del prompt nel README, misurati sul demone (D362, `c617080`).
- 29/09/2026: al modello di casa 58 strumenti fissi, e la conversazione si taglia su quel che resta del contesto (D361, `cdcff8a`).
- 29/09/2026: le cinque cartelle di ogni progetto, in NOVA come indici (D360, `47401b4`).
- 28/09/2026: il README descrive il NOVA di oggi (D359, `df563d6`).
- 28/09/2026: il demone accende il modello di casa quando serve (D358, `790c791`).
- 28/09/2026: `novad --registro` (D357, `074273d`).
