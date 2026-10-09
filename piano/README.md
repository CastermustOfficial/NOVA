# piano/

Cosa c'è da fare, in ordine. Quando una cosa è fatta si spunta, con il commit che l'ha chiusa.

Il racconto lungo sta in [`docs/verso_la_beta.md`](../docs/verso_la_beta.md): le tre liste, i cantieri, il piano per il Rust e le cinque frasi del cancello della beta. Qui c'è solo l'elenco di quello che resta aperto, nell'ordine in cui lo si fa.

Aggiornato all'8 ottobre 2026.

## Da fare

Deciso con Gio il 30 settembre: non ci sono scadenze, conta solo il risultato, e il codice si tratta come sicurezza militare. L'ordine l'ho scelto io, con un criterio solo: **prima quello che protegge**, poi quello che apre porte, poi il resto.

### Il codice, in ordine

1. **`test_demone_ricerca` cade a tratti in CI.** Il 5 ottobre la prima ricerca è caduta quattro volte su sei, su commit che non toccavano la ricerca, mentre GitHub Actions era in avaria: il browser delle ricerche non apriva la porta entro i 25 secondi del demone. Nel contenitore di lavoro passa; su Windows la prova non gira. Adesso, quando cade, la prova scrive quanto ci ha messo la porta ad aprirsi e cosa ha detto il browser su stderr. Alla caduta successiva (`b889e7e`) la porta si è aperta fra 25 e 27,2 secondi dopo la partenza, e la prima riga del browser su stderr è arrivata dopo più di venti: il tempo se ne va prima che il browser parta davvero, e nel contenitore lo stesso Chromium apre la porta in 1,1 s la prima volta. Quindi la prova, prima del demone, fa fare al browser un giro a vuoto e scrive quanto ci mette. Si chiude quando la CI la passa di nuovo e quel numero dice se era il runner a caricare lentamente il browser; se no, l'attesa del demone (la stessa del Python, `ATTESA_AVVIO_S`) va ripensata anche per chi usa NOVA. Il primo giro dopo (`23de1e6`) è verde, con il giro a vuoto in 7,1 s contro 0,6 nel contenitore: il runner carica il browser più lentamente, ma quella volta non oltre i 25 secondi. Resta qui finché altri giri non lo confermano.
2. **Una prova scrive nel registro delle azioni vero.** Trovato il 7 ottobre: dopo la suite sul PC di sviluppo, `azioni.jsonl` dell'utente aveva tre righe di `prove/nova/test_harness_prova.py` («modifica rifiutata dai test», «modificato un documento», su file in `%TEMP%`). La prova usa `nova.registro` senza spostare `APPDATA`, quindi scrive dove scrive NOVA. Va spostata in una cartella sua, e va guardato se `test_strumenti_rust.py`, che prova la stessa modifica dal lato Rust, fa lo stesso. Prima delle altre perche' tocca i dati dell'utente. Della stessa famiglia, trovato lo stesso giorno: `test_routing.py` leggeva la configurazione dell'utente ed e' diventata rossa sul PC di sviluppo appena la scala e' cambiata; ora usa quella di fabbrica. Altre otto prove chiamano ancora `Config.load()` (`test_guardie`, `test_avvio_cervello`, `test_procedure`, `test_prefisso`, `test_harness`, `test_modello_flag`, `test_schede`, `test_strumenti_rust`): per ognuna va deciso se vuole davvero la configurazione di chi la lancia.
3. **I Dot** (D381, [`docs/dots.md`](../docs/dots.md)). Deciso con Gio il 7 e l'8 ottobre: in Rust dentro NOVA, il vault del Dot e' suo, il primo e' un ricercatore che consegna un rapporto con le fonti, pianifica col cervello piu' grande e assegna il cervello passo per passo con un revisore che fa salire, autonomia piena con le guardie che non sono permessi. In ordine: ~~il Dot su disco e il suo ciclo~~ (fatto, D382); ~~il ricercatore, col suo vault, il piano col cervello grande, i passi coi cervelli assegnati, il revisore e il rapporto con le fonti, e ogni scelta del cervello registrata~~ (fatto, D383); ~~il custode dei permessi, a cui un Dot chiede quando Nova chiederebbe all'utente, anche dal suo Claude Code~~ (fatto, D384); resta che quello che legge Claude Code NOVA non lo vede, e che chi ha in scala solo Claude Code o CLI non ha un giudice senza mani (`docs/dots.md`, «Cosa resta aperto, dopo il custode»); poi **l'azienda dei Dot** (D385, deciso con Gio l'8 ottobre, in `docs/dots.md`): l'utente chiede un progetto a Nova, Nova lo passa all'APM (*Artificial Project Manager*), l'APM chiede ad AR (*Artificial Resources*, le risorse umane: assume i Dot e sceglie i modelli) la squadra che gli serve, si forma la piramide (capi gruppo, ricercatori, programmatori, revisori), l'APM mostra piano, organigramma e tetto di spesa e aspetta il via, poi va da solo; i Dot restano e si riusano, ognuno col suo vault e col vault del progetto; l'utente scrive a chi vuole, Nova fa da tramite. In ordine: ~~Nova che li chiama~~ (fatto, D387); ~~i Dot che parlano fra loro~~ (fatto, D388); l'harness (vista, organigramma, chat); AR; l'APM e i progetti; Nova tramite; CLM addestrato sulle scelte di AR e del custode; il progetto di prova di Gio (spazi compressi senza perdita per allargare la finestra di contesto).
4. **CANT-12, le decisioni che oggi sono euristiche.** `nova-decisioni` (quali decisioni, e cosa può uscire dal PC) e `nova-giudizio` (dai logit al giudizio) sono scritti. La metà che chiede a llama-server c'è dal 6 ottobre (D371, `nova_core::giudizio_casa`), dopo le due verifiche (4 ottobre, `misure/banco_giudizio_llama.py`: il ragionamento va chiuso prima della risposta, e la cache non ha cambiato decisioni; ripetute il 6 ottobre su Qwen3.8 27B, GLM-4.7-Flash e Qwen3-8B, con la lettera scelta: 10 giuste su 10 per tutti e quattro, e nessuna decisione cambiata dalla cache). La prima domanda vera, `QualeCervello`, è scritta, misurata (D372: le parole 20 su 34, le lettere da 30 a 34 su quattro modelli, CLM 12) e attaccata alla delega come ha deciso Gio (D373: il giudizio aggiunge salite, non le toglie; se non sa, valgono le parole). Mancano: per le altre decisioni l'euristica di oggi e il modello dietro lo stesso tratto; il banco che li confronta sui casi di NOVA. Un giudizio può solo stringere una guardia, mai allentarla (D313). La strada di CLM è provata (4 ottobre, `misure/banco_clm.py`): così com'è, sulle nostre domande, non è un giudice. Il 6 ottobre Gio ha deciso di addestrarlo, in tre passi, tutti fatti: NOVA tiene le sue decisioni (D374); il modello grande etichetta compiti sintetici con le lettere; le teste addestrate fanno da 27 a 29 su 34 a `QualeCervello` e da 27 a 32 su 40 a scegliere lo strumento (D375). Sulle stesse domande Claude Code (Opus 5) e Gemini 3.1 Pro fanno 34 su 34 e 40 su 40, e una cascata (CLM sopra 0,8, il grande sotto) fa 32 e 36 con un quarto delle chiamate (`misure/banco_cervelli_fuori.py`). Il 7 ottobre Gio ha deciso dove: CLM decide `QualeCervello` quando le lettere non ci sono, cioe' per chi usa come motore rapido un cervello di fuori (D378). Restano due cose. **CLM per gli strumenti**: deciso «anche per gli strumenti», ma un motore rapido che e' una CLI agentica gli strumenti di NOVA non li usa, e per il modello di casa va prima misurato quanto spesso lascia fuori quello giusto. **CLM accanto al modello di casa**: con `clm.attivo` acceso e le lettere che ci sono, il server dei vettori tiene la scheda occupata per servire solo quando le lettere cadono, e su 16 GB con Gemma non ci sta: insieme fanno 20,4 GB, e parte la tiene fuori dalla scheda il driver di Windows; va deciso se in quel caso il demone lo accende lo stesso.
5. **Una difesa contro le prompt injection.** Chiesta da Gio il 30 settembre, da valutare quasi sicuramente dentro CANT-12. Oggi ci sono pezzi sparsi: le descrizioni dei server MCP di altri arrivano citate e non obbedite (D264), quel che si legge dallo schermo non entra in memoria (D170), le azioni rischiose chiedono conferma (D333), e il recinto di Windows (D367) limita il danno. Manca un disegno unico per tutto il testo che arriva da fuori.
6. **`nova-mcp-cliente` attaccato a un binario.** Il cancello verso i server MCP di altri è scritto e provato (CANT-11), ma nessun binario lo usa. Va dopo il giudizio e la difesa dalle prompt injection, perché è una porta verso l'esterno.
7. **I vettori da un modello di embedding.** La versione Python sapeva chiederli a un secondo llama-server (`embedder: "llama"`). Il demone usa solo l'embedding di casa (D359).
8. **Le compatibilità che sono codice.** Le CLI Codex e Qwen, nel menu e mai provate; i dialetti del tool calling di OpenRouter, Groq e Together; l'installatore per un utente senza diritti di amministratore.
9. **Le ottimizzazioni aperte.** Gli schemi degli strumenti (circa 6.900 token, il blocco più grosso del prompt) e un prompt su misura per il modello di casa; lo speculative decoding; i round-trip del browser su un modulo intero; il modello di casa acceso all'avvio.
10. **CANT-9, Mac e Linux alla pari.** Mancano la tastiera, le finestre e l'albero di accessibilità (AT-SPI2, Accessibility API), l'archivio delle credenziali, il recinto su macOS e l'avvio automatico. Servono macchine vere, e prima la decisione di Gio qui sotto.

### Fuori dal codice

- **Il cancello della beta, le frasi che si verificano da qui.** La quarta e la quinta sono avanzate con D359, D362, D363, D365 e D366. La prima, la seconda e la terza vogliono una macchina che non sia quella di Gio.
- **Le decisioni che spettano a Gio** (da `verso_la_beta.md`):
  - Mac e Linux: una promessa con una data, o NOVA è un programma Windows;
  - automazioni e riparazioni: oggi NOVA si scrive strumenti in Python e si ripara provando le modifiche sul proprio Python, e tutta in Rust vorrebbe dire scegliere in che lingua si scrive da sola;
  - il motore di ricalcolo dei fogli: `nova-fogli` legge i valori già calcolati e non ricalcola le formule (D281);
  - `build.ps1`: cosa diventa per Mac e Linux;
  - quando si chiude il cancello della beta;

## Fatto

Quando una voce si chiude la si sposta qui, con la data e il commit.

- 09/10/2026: i Dot parlano fra loro: ogni Dot puo' avere un capo, il capo aspetta e riprende, la posta si legge al compito dopo, i gruppi li fa Nova (D388).
- 08/10/2026: Nova chiama i Dot: affida senza chiedere, fa nascere un Dot solo su richiesta, e la consegna arriva in chat e a voce (D387, `9b9e784`).
- 08/10/2026: Gemini Live: dopo «Nova» la conversazione dal vivo, con le funzioni di NOVA, la chiave e le 30 voci nel pannello, da ascoltare prima di scegliere; e il guscio che dimentica davvero la conversazione del demone (D386, `6ac29d7`).
- 08/10/2026: l'azienda dei Dot, il disegno: APM, AR che assume, la piramide, il via e il tetto per progetto, i Dot che restano (D385, `6ccc57c`).
- 08/10/2026: il custode dei permessi: un Dot chiede a lui quando Nova chiederebbe all'utente, anche dal suo Claude Code, col modello di casa e se no il cervello grande (D384, `eb68820`).
- 08/10/2026: il ricercatore: il piano col cervello grande, i passi col cervello assegnato, il revisore, il rapporto con le fonti controllate, il vault suo, ogni scelta del cervello registrata (D383, `d742a89`).
- 08/10/2026: il primo passo dei Dot: un Dot su disco, il suo ciclo, e niente permessi da chiedere (D382, `ff0f95c`).
- 08/10/2026: i Dot, il disegno (D381, `0b991cf`).
- 07/10/2026: l'orb, l'icona e il logo sono quelli disegnati da Gio (D380, `a8c0d91`).
- 07/10/2026: NOVA consiglia la scala per quello che l'utente ha, e la applica solo col bottone del pannello (D379, `67a8b17`).
- 07/10/2026: CLM decide «quale cervello» quando le lettere non ci sono (D378, `21c1c4b`).
- 07/10/2026: i modelli dei cervelli di fuori sono i piu' recenti della famiglia, e Claude Code si aggiorna da sola (D377, `c4925fa`).
- 07/10/2026: il PC delle misure e' il «PC di sviluppo», con cosa ha, anche in tutta la storia di git (D376, `9064631`).
- 06/10/2026: le teste di CLM addestrate sulle scelte del modello grande, misurate sui banchi (D375).
- 06/10/2026: NOVA tiene le sue decisioni, senza segreti, per insegnare a CLM (D374, `0cdbb50`).
- 06/10/2026: `QualeCervello` decide con il giudizio del modello di casa accanto alle parole, nella delega (D373).
- 05/10/2026: le attivita' pianificate lanciano NOVA senza finestra, con `novaw`, e solo se il binario sa fare il comando; la prova che lasciava l'attivita' sul PC la toglie (D370, `4dfa357`).
- 04/10/2026: tolto il fermo dei lavori aperto il 3 ottobre per la falla del recinto di Windows con il demone da amministratore. La CI e' verde su `b6c17d0`, e l'avviso e' pubblicato: GHSA-38cw-xfm5-xq9f (gravita' alta; versioni da `e9d298b` a `ea52524`, corretta da `97f01d8`).
- 03/10/2026: un comando nel recinto non riceve mai i poteri dell'amministratore, nemmeno se il demone li ha (D369, `97f01d8`; la prova sul proprietario confrontata per SID in `96e54ca`).
- 03/10/2026: il recinto per i comandi su Windows, un contenitore del sistema, e dove non basta lo dice (D367, `e9d298b`).
- 03/10/2026: le prove del demone su Windows tornano verdi, e quattro difetti veri del prodotto vengono fuori (D368, `e9d298b`; la riprova sulla pipe con una scadenza d'orologio in `5ba06c8`).
- 30/09/2026: `kb.enabled` e `kb.inject_context` valgono anche nel demone (D366, `d9aad22`).
- 30/09/2026: la prima mappatura del PC nel demone, e il vault che il demone crea da sé (D365, `f2a3062`).
- 29/09/2026: `rete.cerca` cerca prima col browser senza finestra, come il Python (D363, `277c0ac`).
- 29/09/2026: leggere la pagina dei risultati non solleva più, e `test_cerca.py` non accende un Edge vero (D364, `8789476`).
- 29/09/2026: i numeri del browser e del prompt nel README, misurati sul demone (D362, `0fc434e`).
- 29/09/2026: al modello di casa 58 strumenti fissi, e la conversazione si taglia su quel che resta del contesto (D361, `49c5a73`).
- 29/09/2026: le cinque cartelle di ogni progetto, in NOVA come indici (D360, `4ab0c35`).
- 28/09/2026: il README descrive il NOVA di oggi (D359, `99a3449`).
- 28/09/2026: il demone accende il modello di casa quando serve (D358, `c265834`).
- 28/09/2026: `novad --registro` (D357, `312db3e`).
