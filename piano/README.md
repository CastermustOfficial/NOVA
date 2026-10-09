# piano/

Cosa c'è da fare, in ordine. Quando una cosa è fatta si spunta, con il commit che l'ha chiusa.

Il racconto lungo sta in [`docs/verso_la_beta.md`](../docs/verso_la_beta.md): le tre liste, i cantieri, il piano per il Rust e le cinque frasi del cancello della beta. Qui c'è solo l'elenco di quello che resta aperto, nell'ordine in cui lo si fa.

Aggiornato al 9 ottobre 2026.

## Da fare

Deciso con Gio il 30 settembre: non ci sono scadenze, conta solo il risultato, e il codice si tratta come sicurezza militare. L'ordine l'ho scelto io, con un criterio solo: **prima quello che protegge**, poi quello che apre porte, poi il resto.

### Il codice, in ordine

1. **`test_cerca.py` e' caduto una volta in CI.** L'8 ottobre, su `6ccc57c`, nel giro con Python 3.10: 20 controlli su 21, con una ricerca vera sul web. E' l'unico rosso nei 31 giri dal 6 ottobre. Una prova che fa una ricerca vera dipende dal motore e dalla rete: va capito quale controllo e' caduto (il registro del giro ce l'ha) prima di decidere se e' la prova o NOVA.
2. **I Dot** (D381, [`docs/dots.md`](../docs/dots.md)). Deciso con Gio il 7 e l'8 ottobre: in Rust dentro NOVA, il vault del Dot e' suo, il primo e' un ricercatore che consegna un rapporto con le fonti, pianifica col cervello piu' grande e assegna il cervello passo per passo con un revisore che fa salire, autonomia piena con le guardie che non sono permessi. In ordine: ~~il Dot su disco e il suo ciclo~~ (fatto, D382); ~~il ricercatore, col suo vault, il piano col cervello grande, i passi coi cervelli assegnati, il revisore e il rapporto con le fonti, e ogni scelta del cervello registrata~~ (fatto, D383); ~~il custode dei permessi, a cui un Dot chiede quando Nova chiederebbe all'utente, anche dal suo Claude Code~~ (fatto, D384); resta che quello che legge Claude Code NOVA non lo vede, e che chi ha in scala solo Claude Code o CLI non ha un giudice senza mani (`docs/dots.md`, «Cosa resta aperto, dopo il custode»); poi **l'azienda dei Dot** (D385, deciso con Gio l'8 ottobre, in `docs/dots.md`): l'utente chiede un progetto a Nova, Nova lo passa all'APM (*Artificial Project Manager*), l'APM chiede ad AR (*Artificial Resources*, le risorse umane: assume i Dot e sceglie i modelli) la squadra che gli serve, si forma la piramide (capi gruppo, ricercatori, programmatori, revisori), l'APM mostra piano, organigramma e tetto di spesa e aspetta il via, poi va da solo; i Dot restano e si riusano, ognuno col suo vault e col vault del progetto; l'utente scrive a chi vuole, Nova fa da tramite. In ordine: ~~Nova che li chiama~~ (fatto, D387); ~~i Dot che parlano fra loro~~ (fatto, D388); ~~l'harness (vista, organigramma, chat), come Teams~~ (fatto, D391, D392); poi, come un'azienda vera (D395, deciso con Gio il 9 ottobre): ~~la direzione (APM, AR, Architetto) e i reparti sempre presenti (legale, commerciale, ricerca, revisione, scrittura, dati e misure, qualita' e prove, amministrazione, sicurezza)~~ (fatto, D396); AR (~~riprendere, assumere, il cervello del compito~~, fatto, D397; resta licenziare); l'Architetto e il piano di sviluppo; l'APM e i progetti, coi controlli del legale; Nova tramite; CLM addestrato sulle scelte di AR, dell'APM e del custode; il progetto di prova di Gio (spazi compressi senza perdita per allargare la finestra di contesto).
3. **CANT-12, le decisioni che oggi sono euristiche.** `nova-decisioni` (quali decisioni, e cosa può uscire dal PC) e `nova-giudizio` (dai logit al giudizio) sono scritti. La metà che chiede a llama-server c'è dal 6 ottobre (D371, `nova_core::giudizio_casa`), dopo le due verifiche (4 ottobre, `misure/banco_giudizio_llama.py`: il ragionamento va chiuso prima della risposta, e la cache non ha cambiato decisioni; ripetute il 6 ottobre su Qwen3.8 27B, GLM-4.7-Flash e Qwen3-8B, con la lettera scelta: 10 giuste su 10 per tutti e quattro, e nessuna decisione cambiata dalla cache). La prima domanda vera, `QualeCervello`, è scritta, misurata (D372: le parole 20 su 34, le lettere da 30 a 34 su quattro modelli, CLM 12) e attaccata alla delega come ha deciso Gio (D373: il giudizio aggiunge salite, non le toglie; se non sa, valgono le parole). Mancano: per le altre decisioni l'euristica di oggi e il modello dietro lo stesso tratto; il banco che li confronta sui casi di NOVA. Un giudizio può solo stringere una guardia, mai allentarla (D313). La strada di CLM è provata (4 ottobre, `misure/banco_clm.py`): così com'è, sulle nostre domande, non è un giudice. Il 6 ottobre Gio ha deciso di addestrarlo, in tre passi, tutti fatti: NOVA tiene le sue decisioni (D374); il modello grande etichetta compiti sintetici con le lettere; le teste addestrate fanno da 27 a 29 su 34 a `QualeCervello` e da 27 a 32 su 40 a scegliere lo strumento (D375). Sulle stesse domande Claude Code (Opus 5) e Gemini 3.1 Pro fanno 34 su 34 e 40 su 40, e una cascata (CLM sopra 0,8, il grande sotto) fa 32 e 36 con un quarto delle chiamate (`misure/banco_cervelli_fuori.py`). Il 7 ottobre Gio ha deciso dove: CLM decide `QualeCervello` quando le lettere non ci sono, cioe' per chi usa come motore rapido un cervello di fuori (D378). Restano due cose. **CLM per gli strumenti**: deciso «anche per gli strumenti», ma un motore rapido che e' una CLI agentica gli strumenti di NOVA non li usa, e per il modello di casa va prima misurato quanto spesso lascia fuori quello giusto. **CLM accanto al modello di casa**: con `clm.attivo` acceso e le lettere che ci sono, il server dei vettori tiene la scheda occupata per servire solo quando le lettere cadono, e su 16 GB con Gemma non ci sta: insieme fanno 20,4 GB, e parte la tiene fuori dalla scheda il driver di Windows; va deciso se in quel caso il demone lo accende lo stesso.
4. **Una difesa contro le prompt injection.** Chiesta da Gio il 30 settembre, da valutare quasi sicuramente dentro CANT-12. Oggi ci sono pezzi sparsi: le descrizioni dei server MCP di altri arrivano citate e non obbedite (D264), quel che si legge dallo schermo non entra in memoria (D170), le azioni rischiose chiedono conferma (D333), e il recinto di Windows (D367) limita il danno. Manca un disegno unico per tutto il testo che arriva da fuori.
5. **`nova-mcp-cliente` attaccato a un binario.** Il cancello verso i server MCP di altri è scritto e provato (CANT-11), ma nessun binario lo usa. Va dopo il giudizio e la difesa dalle prompt injection, perché è una porta verso l'esterno.
6. **I vettori da un modello di embedding.** La versione Python sapeva chiederli a un secondo llama-server (`embedder: "llama"`). Il demone usa solo l'embedding di casa (D359).
7. **Le compatibilità che sono codice.** Le CLI Codex e Qwen, nel menu e mai provate; i dialetti del tool calling di OpenRouter, Groq e Together; l'installatore per un utente senza diritti di amministratore.
8. **Le ottimizzazioni aperte.** Gli schemi degli strumenti (circa 6.900 token, il blocco più grosso del prompt) e un prompt su misura per il modello di casa; lo speculative decoding; i round-trip del browser su un modulo intero; il modello di casa acceso all'avvio.
9. **CANT-9, Mac e Linux alla pari.** Mancano la tastiera, le finestre e l'albero di accessibilità (AT-SPI2, Accessibility API), l'archivio delle credenziali, il recinto su macOS e l'avvio automatico. Servono macchine vere, e prima la decisione di Gio qui sotto.

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

- 09/10/2026: AR sceglie chi lavora e con che cervello: Nova chiede un Dot ad AR, che riprende o assume, e il cervello vale per tutto il compito (D397).
- 09/10/2026: la direzione e i reparti dei Dot nascono coi Dot accesi; la direzione e il legale non prendono compiti a mano (D396, `e15c042`).
- 09/10/2026: nessuna prova accende piu' la finestra vera di NOVA: `test_harness_prova.py` la lasciava accesa con la sua casa finta, al posto di quella dell'utente; lo controlla `test_prove_in_casa_loro.py` (`665d1fd`).
- 09/10/2026: l'azienda dei Dot, il disegno come un'azienda vera: la direzione, i reparti sempre presenti, prima il piano e poi la squadra, il legale (D395, `6155c6e`).
- 09/10/2026: «il microfono non consegna niente» solo col microfono spento davvero, non con la stanza silenziosa (D394, `eb36b68`).
- 09/10/2026: `test_demone_ricerca` non cade piu' in CI: verde in tutti i 31 giri su master dal 6 ottobre, eseguita e non saltata; il giro a vuoto del browser, messo quel giorno, era la cura. In CI il tempo del giro a vuoto ora e' anche fra gli avvisi, per continuare a guardarlo (D393, `1a72c8b`).
- 09/10/2026: le prove stanno in casa loro: nessuna scrive piu' nei dati di chi la lancia (il registro delle azioni, i guasti, il fascicolo), e quelle che leggevano la sua configurazione usano quella di fabbrica; lo controlla `test_prove_in_casa_loro.py` (D393, `1a72c8b`).
- 09/10/2026: i Dot come Teams: le chat prima di tutto, i messaggi a piu' Dot, le chat fra di loro in cui scrivi anche tu, i gruppi interni, e i gruppi di un capo (D392, `8f94ae4`).
- 09/10/2026: i Dot nell'harness: l'organigramma (anche come schema), i gruppi, i file che toccano, e la scheda di ogni Dot con la sua chat (D391, `d0aff4b`).
- 09/10/2026: la scheda Cervello a voci: si spuntano i motori, si scelgono chi orchestra, il modello veloce e il modello, e si scrive solo con «Conferma» (D390, `643557c`).
- 09/10/2026: i Dot si accendono solo dove conviene: un interruttore nel pannello, di serie deciso da NOVA (D389, `eb0479a`).
- 09/10/2026: i Dot parlano fra loro: ogni Dot puo' avere un capo, il capo aspetta e riprende, la posta si legge al compito dopo, i gruppi li fa Nova (D388, `48e72ab`).
- 08/10/2026: Nova chiama i Dot: affida senza chiedere, fa nascere un Dot solo su richiesta, e la consegna arriva in chat e a voce (D387, `7e9b8f0`).
- 08/10/2026: Gemini Live: dopo «Nova» la conversazione dal vivo, con le funzioni di NOVA, la chiave e le 30 voci nel pannello, da ascoltare prima di scegliere; e il guscio che dimentica davvero la conversazione del demone (D386, `7bea712`).
- 08/10/2026: l'azienda dei Dot, il disegno: APM, AR che assume, la piramide, il via e il tetto per progetto, i Dot che restano (D385, `0efe9e8`).
- 08/10/2026: il custode dei permessi: un Dot chiede a lui quando Nova chiederebbe all'utente, anche dal suo Claude Code, col modello di casa e se no il cervello grande (D384, `b257144`).
- 08/10/2026: il ricercatore: il piano col cervello grande, i passi col cervello assegnato, il revisore, il rapporto con le fonti controllate, il vault suo, ogni scelta del cervello registrata (D383, `f96faf6`).
- 08/10/2026: il primo passo dei Dot: un Dot su disco, il suo ciclo, e niente permessi da chiedere (D382, `ed5a82c`).
- 08/10/2026: i Dot, il disegno (D381, `8abdda8`).
- 07/10/2026: l'orb, l'icona e il logo sono quelli disegnati da Gio (D380, `9d3bb4d`).
- 07/10/2026: NOVA consiglia la scala per quello che l'utente ha, e la applica solo col bottone del pannello (D379, `c32b74f`).
- 07/10/2026: CLM decide «quale cervello» quando le lettere non ci sono (D378, `06ade85`).
- 07/10/2026: i modelli dei cervelli di fuori sono i piu' recenti della famiglia, e Claude Code si aggiorna da sola (D377, `46ad5ab`).
- 07/10/2026: il PC delle misure e' il «PC di sviluppo», con cosa ha, anche in tutta la storia di git (D376, `f801660`).
- 06/10/2026: le teste di CLM addestrate sulle scelte del modello grande, misurate sui banchi (D375).
- 06/10/2026: NOVA tiene le sue decisioni, senza segreti, per insegnare a CLM (D374, `5084cf0`).
- 06/10/2026: `QualeCervello` decide con il giudizio del modello di casa accanto alle parole, nella delega (D373).
- 05/10/2026: le attivita' pianificate lanciano NOVA senza finestra, con `novaw`, e solo se il binario sa fare il comando; la prova che lasciava l'attivita' sul PC la toglie (D370, `bf79d05`).
- 04/10/2026: tolto il fermo dei lavori aperto il 3 ottobre per la falla del recinto di Windows con il demone da amministratore. La CI e' verde su `38b0cab`, e l'avviso e' pubblicato: GHSA-38cw-xfm5-xq9f (gravita' alta; versioni da `d7b5ae8` a `9c7a3f1`, corretta da `43b6263`).
- 03/10/2026: un comando nel recinto non riceve mai i poteri dell'amministratore, nemmeno se il demone li ha (D369, `43b6263`; la prova sul proprietario confrontata per SID in `e4f4e10`).
- 03/10/2026: il recinto per i comandi su Windows, un contenitore del sistema, e dove non basta lo dice (D367, `d7b5ae8`).
- 03/10/2026: le prove del demone su Windows tornano verdi, e quattro difetti veri del prodotto vengono fuori (D368, `d7b5ae8`; la riprova sulla pipe con una scadenza d'orologio in `ad054dd`).
- 30/09/2026: `kb.enabled` e `kb.inject_context` valgono anche nel demone (D366, `24e5113`).
- 30/09/2026: la prima mappatura del PC nel demone, e il vault che il demone crea da sé (D365, `86f4607`).
- 29/09/2026: `rete.cerca` cerca prima col browser senza finestra, come il Python (D363, `69592b6`).
- 29/09/2026: leggere la pagina dei risultati non solleva più, e `test_cerca.py` non accende un Edge vero (D364, `c886776`).
- 29/09/2026: i numeri del browser e del prompt nel README, misurati sul demone (D362, `f406da7`).
- 29/09/2026: al modello di casa 58 strumenti fissi, e la conversazione si taglia su quel che resta del contesto (D361, `f818a47`).
- 29/09/2026: le cinque cartelle di ogni progetto, in NOVA come indici (D360, `7f27a2f`).
- 28/09/2026: il README descrive il NOVA di oggi (D359, `dc41c4c`).
- 28/09/2026: il demone accende il modello di casa quando serve (D358, `79a1e43`).
- 28/09/2026: `novad --registro` (D357, `3d1fc1c`).
