# piano/

Cosa c'è da fare, in ordine. Quando una cosa è fatta si spunta, con il commit che l'ha chiusa.

Il racconto lungo sta in [`docs/verso_la_beta.md`](../docs/verso_la_beta.md): le tre liste, i cantieri, il piano per il Rust e le cinque frasi del cancello della beta. Qui c'è solo l'elenco di quello che resta aperto, nell'ordine in cui lo si fa.

Aggiornato al 6 ottobre 2026.

## Da fare

Deciso con Gio il 30 settembre: non ci sono scadenze, conta solo il risultato, e il codice si tratta come sicurezza militare. L'ordine l'ho scelto io, con un criterio solo: **prima quello che protegge**, poi quello che apre porte, poi il resto.

### Il codice, in ordine

1. **`test_demone_ricerca` cade a tratti in CI.** Il 5 ottobre la prima ricerca è caduta quattro volte su sei, su commit che non toccavano la ricerca, mentre GitHub Actions era in avaria: il browser delle ricerche non apriva la porta entro i 25 secondi del demone. Nel contenitore di lavoro passa; su Windows la prova non gira. Adesso, quando cade, la prova scrive quanto ci ha messo la porta ad aprirsi e cosa ha detto il browser su stderr. Alla caduta successiva (`8aec032`) la porta si è aperta fra 25 e 27,2 secondi dopo la partenza, e la prima riga del browser su stderr è arrivata dopo più di venti: il tempo se ne va prima che il browser parta davvero, e nel contenitore lo stesso Chromium apre la porta in 1,1 s la prima volta. Quindi la prova, prima del demone, fa fare al browser un giro a vuoto e scrive quanto ci mette. Si chiude quando la CI la passa di nuovo e quel numero dice se era il runner a caricare lentamente il browser; se no, l'attesa del demone (la stessa del Python, `ATTESA_AVVIO_S`) va ripensata anche per chi usa NOVA. Il primo giro dopo (`28a5879`) è verde, con il giro a vuoto in 7,1 s contro 0,6 nel contenitore: il runner carica il browser più lentamente, ma quella volta non oltre i 25 secondi. Resta qui finché altri giri non lo confermano.
2. **CANT-12, le decisioni che oggi sono euristiche.** `nova-decisioni` (quali decisioni, e cosa può uscire dal PC) e `nova-giudizio` (dai logit al giudizio) sono scritti. La metà che chiede a llama-server c'è dal 6 ottobre (D371, `nova_core::giudizio_casa`), dopo le due verifiche (4 ottobre, `misure/banco_giudizio_llama.py`: il ragionamento va chiuso prima della risposta, e la cache non ha cambiato decisioni; ripetute il 6 ottobre su Qwen3.8 27B, GLM-4.7-Flash e Qwen3-8B, con la lettera scelta: 10 giuste su 10 per tutti e quattro, e nessuna decisione cambiata dalla cache), ma nessuna decisione la chiama ancora. La prima domanda vera, `QualeCervello`, è scritta, misurata (D372: le parole 20 su 34, le lettere da 30 a 34 su quattro modelli, CLM 12) e attaccata alla delega come ha deciso Gio (D373: il giudizio aggiunge salite, non le toglie; se non sa, valgono le parole). Mancano: per le altre decisioni l'euristica di oggi e il modello dietro lo stesso tratto; il banco che li confronta sui casi di NOVA. Un giudizio può solo stringere una guardia, mai allentarla (D313). La strada di CLM è provata (4 ottobre, `misure/banco_clm.py`): così com'è, sulle nostre domande, non è un giudice. Il 6 ottobre Gio ha deciso di addestrarlo, in tre passi: NOVA tiene le sue decisioni (fatto, D374); il modello grande etichetta compiti sintetici con le lettere; le teste si addestrano sulla GPU del PC di sviluppo (RTX 4060 Ti, 16 GB di VRAM) e si rimisurano sugli stessi banchi.
3. **Una difesa contro le prompt injection.** Chiesta da Gio il 30 settembre, da valutare quasi sicuramente dentro CANT-12. Oggi ci sono pezzi sparsi: le descrizioni dei server MCP di altri arrivano citate e non obbedite (D264), quel che si legge dallo schermo non entra in memoria (D170), le azioni rischiose chiedono conferma (D333), e il recinto di Windows (D367) limita il danno. Manca un disegno unico per tutto il testo che arriva da fuori.
4. **`nova-mcp-cliente` attaccato a un binario.** Il cancello verso i server MCP di altri è scritto e provato (CANT-11), ma nessun binario lo usa. Va dopo il giudizio e la difesa dalle prompt injection, perché è una porta verso l'esterno.
5. **I vettori da un modello di embedding.** La versione Python sapeva chiederli a un secondo llama-server (`embedder: "llama"`). Il demone usa solo l'embedding di casa (D359).
6. **Le compatibilità che sono codice.** Le CLI Codex e Qwen, nel menu e mai provate; i dialetti del tool calling di OpenRouter, Groq e Together; l'installatore per un utente senza diritti di amministratore.
7. **Le ottimizzazioni aperte.** Gli schemi degli strumenti (circa 6.900 token, il blocco più grosso del prompt) e un prompt su misura per il modello di casa; lo speculative decoding; i round-trip del browser su un modulo intero; il modello di casa acceso all'avvio.
8. **CANT-9, Mac e Linux alla pari.** Mancano la tastiera, le finestre e l'albero di accessibilità (AT-SPI2, Accessibility API), l'archivio delle credenziali, il recinto su macOS e l'avvio automatico. Servono macchine vere, e prima la decisione di Gio qui sotto.

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

- 06/10/2026: NOVA tiene le sue decisioni, senza segreti, per insegnare a CLM (D374).
- 06/10/2026: `QualeCervello` decide con il giudizio del modello di casa accanto alle parole, nella delega (D373).
- 05/10/2026: le attivita' pianificate lanciano NOVA senza finestra, con `novaw`, e solo se il binario sa fare il comando; la prova che lasciava l'attivita' sul PC la toglie (D370, `eb24c2b`).
- 04/10/2026: tolto il fermo dei lavori aperto il 3 ottobre per la falla del recinto di Windows con il demone da amministratore. La CI e' verde su `7ea9ec2`, e l'avviso e' pubblicato: GHSA-38cw-xfm5-xq9f (gravita' alta; versioni da `7edebc4` a `c441a61`, corretta da `40175f5`).
- 03/10/2026: un comando nel recinto non riceve mai i poteri dell'amministratore, nemmeno se il demone li ha (D369, `40175f5`; la prova sul proprietario confrontata per SID in `26586a5`).
- 03/10/2026: il recinto per i comandi su Windows, un contenitore del sistema, e dove non basta lo dice (D367, `7edebc4`).
- 03/10/2026: le prove del demone su Windows tornano verdi, e quattro difetti veri del prodotto vengono fuori (D368, `7edebc4`; la riprova sulla pipe con una scadenza d'orologio in `573ccf7`).
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
