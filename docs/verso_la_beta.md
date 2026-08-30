# Verso la beta

Alpha vuol dire una cosa sola, ed e' onesto scriverla: **NOVA ha girato su una
macchina.** Tutto il resto — i test verdi, la CI, i sessanta strumenti — e'
vero su quella. Beta vuol dire che ha girato su macchine di cui non sappiamo
niente, e non si e' rotta in modo che l'utente non capisce.

Questo documento e' la strada per arrivarci. Tre liste, in ordine di quanto
pesano: cosa rendere veloce, cosa far funzionare altrove, cosa smettere di far
sembrare difficile.

---

## Prima: cosa dicono le misure

Il banco (`banco_prestazioni.py`, `python banco_prestazioni.py --json`) misura
i pezzi che girano a ogni turno **senza il modello**. Su questa macchina, con
136 nodi in memoria e 28 procedure in archivio:

| pezzo | mediana |
|---|---|
| memoria a grafo (`contesto_per`) | 25,3 ms |
| ricette (`blocco`) | 2,8 ms |
| schemi dei 60 tool | 0,14 ms |
| **totale Python, per turno** | **~28 ms** |

Avvio a freddo: 53 ms `import nova`, 206 ms con tutti i tool, 81 ms PyQt6.

Quello che parte a ogni chiamata al modello: **47.449 caratteri, ~11.900
token**. Regole operative ~3.681, schemi dei tool ~6.635, contesto KB ~590,
ricette ~955.

**La conclusione scomoda.** Riscrivere il Python in Rust fa risparmiare
ventotto millisecondi su un turno che ne dura migliaia. Il tempo non e' nel
codice di NOVA, e' nel modello — e nel numero di volte che lo si chiama. Il
basso livello che paga davvero sta un piano sotto: llama.cpp, la VRAM, la KV
cache.

Il porting in Rust resta la direzione giusta, ma per altri motivi — un binario
solo, niente Python da installare, un processo che non cade — non per la
velocita'. Vale la pena dirselo prima di spendere mesi.

**Quello che gia' e' fatto bene, e non va toccato:** contesto KB e ricette
stanno **in coda alla domanda**, non nel prompt di sistema. I ~10.300 token di
prefisso stabile llama.cpp li riusa, e per turno ne rielabora ~1.500. Quella
scelta li' vale piu' di qualunque riscrittura, e va protetta da chi verra'
dopo: chiunque sposti quei blocchi nel messaggio di sistema fa dieci volte il
danno che qualunque ottimizzazione ripara.

---

## Lista 1 — Ottimizzazione

In ordine di guadagno atteso, non di difficolta'.

### Il modello (dove sta il tempo)

1. **Misurare il modello acceso.** Prompt eval e generazione com'e' oggi, con
   il prompt vero da 11.900 token, non con una frase di prova. Senza questa
   riga tutte le altre sono opinioni.
2. **`-fa` (flash attention).** Oggi llama-server parte senza. Piu' veloce e
   occupa meno KV.
3. **KV cache a `q8_0`** (`--cache-type-k`, `--cache-type-v`). Dimezza la
   cache: su una 16 GB con dodici layer sulla CPU, quei layer si ricomprano.
   E' il singolo intervento con il rapporto guadagno/rischio migliore.
4. **`--cache-reuse`.** Serve perche' `trim_history` taglia **in mezzo** alla
   conversazione: il prefisso non combacia piu' e si rielabora tutto. Misurare
   quanto costa un taglio, poi decidere.
5. **Ripensare `trim_history`.** Tagliare la coda non rompe niente, tagliare il
   centro si'. O si riassume il centro in un messaggio solo, o si taglia solo
   dal fondo.
6. **Speculative decoding** con un draft piccolo (`--model-draft`). Sul codice
   e sull'output strutturato — che e' quasi tutto quello che NOVA genera — vale
   spesso 1,5-2x.
7. **Provare un MoE davvero**, non solo consigliarlo nel README: Gemma 4
   26B-A4B o Nemotron 3 Nano 30B-A3B, e mettere i numeri misurati accanto a
   quelli del denso.

### Il prompt (dove stanno i token)

8. **Gli schemi dei tool sono 6.635 token, il blocco piu' grosso di tutti.**
   Accorciare le descrizioni si puo' fare subito. Mandare solo i tool
   pertinenti invece no, non a cuor leggero: cambia il prefisso e butta la
   cache. Da misurare prima di decidere.
9. **Verificare che il prefisso sia davvero stabile** fra un turno e l'altro:
   una data, un'ora, un contatore dentro il prompt di sistema costano l'intera
   rielaborazione e non si vedono.

### Il codice (dove sta poco, ma si puo' prendere)

10. **`contesto_per` a 25 ms** e' il pezzo Python piu' caro. L'indice si
    ricostruisce a ogni avvio: renderlo persistente sul disco.
11. **Avvio a freddo: 206 ms di import dei tool.** Import pigro per categoria —
    i sessanta strumenti non servono tutti al primo messaggio.
12. **Primo porting in Rust: ricette + BM25.** Sono algoritmi puri, senza GUI e
    senza Windows: si portano in `nova-core` con un test che confronta i
    risultati delle due versioni riga per riga. Non e' il guadagno, e' il
    banco di prova per capire quanto costa davvero portare il resto.
13. **Misurare i round-trip CDP** in una sessione vera, non su una pagina di
    prova. 35 ms per un incolla e' il numero buono; quello che conta e' quanti
    ne servono per un modulo intero.

---

## Lista 2 — Compatibilita'

Ogni riga qui e' un modo in cui NOVA funziona qui e non altrove. Sono le
uniche che separano l'alpha dalla beta.

### La macchina

1. **Windows 10.** Tutto e' provato su 11. UI Automation e DPAPI ci sono anche
   su 10, ma «ci sono» non e' «provato».
2. **Utente senza diritti di amministratore.** L'installer scrive
   nell'avvio automatico e crea un collegamento: cosa succede se non puo'.
3. **Percorsi ostili**: spazi, accenti, e soprattutto **Documenti
   ridiretto su OneDrive**, che e' il caso normale e non quello raro.
4. **SmartScreen e antivirus.** Un binario non firmato scaricato da GitHub
   viene messo in quarantena, e l'utente pensa a un virus. Decidere se si
   firma o se si spiega.
5. **Python 3.10, 3.11, 3.12, 3.13.** L'installer dichiara 3.10+; la CI ne
   prova una.

### La scheda video

6. **AMD e Intel.** La stima della VRAM chiama `nvidia-smi`. Vulkan gia'
   funziona come backend, ma senza stima si finisce nella memoria condivisa —
   il fallimento silenzioso che tutto il resto del codice cerca di evitare.
7. **Nessuna GPU.** La strada c'e' nel README; provarla davvero e misurarla,
   cosi' si sa cosa promettere.

### Il cervello

8. **Modelli che non sono Qwen.** Template di chat diverso, function calling
   diverso, ragionamento diverso. Gemma 4 e Nemotron sono nel README: vanno
   provati o tolti.
9. **Le CLI dichiarate ma non provate**: Gemini, Codex, Qwen. Sono nel menu.
   Ognuna ha permessi e formato di output suoi.
10. **Gli endpoint API.** OpenRouter, Groq, Together parlano lo stesso dialetto
    «quasi»: il tool calling e' il punto dove smettono di somigliarsi.
11. **Il modello locale senza `mmproj`.** L'installer lo dice, ma il resto del
    sistema deve degradare bene: `schermo` non deve rompersi, deve spiegarsi.

### Il contorno

12. **Solo Chrome ed Edge**, via CDP. Firefox no, e va scritto invece che
    scoperto.
13. **macOS e Linux.** Oggi `non_implementato.rs` compila e non fa niente. La
    decisione da prendere non e' tecnica: o e' una promessa con una data, o si
    dice che NOVA e' un programma Windows.

---

## Lista 3 — Attrito cognitivo

Non «cosa non funziona» ma «cosa fa sentire stupido chi lo usa». E' la lista
che di solito non si scrive, ed e' quella che decide se qualcuno lo tiene
installato dopo il primo giorno.

### I primi cinque minuti

1. **Cosa vede uno appena finita l'installazione?** Oggi: un orb. Serve una
   cosa da provare subito, che funzioni di sicuro e che faccia capire cosa e'.
2. **Il README e' lungo.** Serve un percorso «primi cinque minuti» in testa,
   per chi non lo leggera' mai tutto.
3. **L'orb dice se e' acceso?** Se sta ascoltando? Se sta pensando? Uno stato
   che non si vede e' uno stato che non c'e'.

### Quando lavora

4. **Trenta secondi di attesa senza niente sembrano rotti.** Lo stato deve
   dire a che passo e' e cosa sta facendo, non «Sto pensando...».
5. **La conferma deve dire *cosa* fa.** C'e' il campo `preview` sui tool: va
   verificato che sia scritto e sensato per tutti e sessanta, non per i primi
   dieci.
6. **Il registro azioni si legge?** Si cerca dentro? Se e' un file che nessuno
   apre, la promessa «cio' che non si annulla si annota» e' mezza mantenuta.

### Quando non ce la fa

7. **Nessun traceback deve arrivare all'utente.** E' il confine piu' netto fra
   alpha e beta: un errore Python sullo schermo dice «questo programma non e'
   finito».
8. **«Non ci riesco» deve dire perche' e cosa fare.** Modello spento, quota
   finita, permesso negato, file bloccato: sono quattro messaggi diversi.
9. **Il verificatore dell'harness.** Oggi NOVA propone una modifica al codice e
   l'utente deve fidarsi. Eseguire i test del progetto e applicare solo se
   passano e' il pezzo che manca — ed e' quello che rende l'harness un posto
   dove si programma, non solo dove si legge.

### Fiducia

10. **«Dove sono i miei dati?»** Un comando solo che risponde: memoria,
    credenziali, registro, configurazione, e quanto pesano.
11. **«Cosa esce dal mio PC?»** Il README lo dice; deve dirlo anche
    l'interfaccia, nel momento in cui si cambia cervello.
12. **Disinstallare deve togliere tutto**, dire cosa ha tolto e cosa ha
    lasciato apposta. Un disinstallatore che lascia in giro roba e' l'ultima
    cosa che un utente ricorda.
13. **Il menu delle impostazioni e' disordinato.** Segnalato, ancora vero.

---

## Il cancello della beta

Non e' una data, sono cinque frasi che devono essere vere insieme:

1. **Qualcuno che non e' l'autore l'ha installato**, su una macchina che non e'
   questa, e gli ha fatto fare qualcosa di utile.
2. **Nessun traceback raggiunge l'utente**, in nessuna delle strade provate.
3. **La disinstallazione e' pulita** e lo dice.
4. **Le compatibilita' dichiarate sono provate**, oppure sono state tolte dal
   README. Nessuna promessa in sospeso.
5. **I numeri del README sono misurati**, anche quelli dei modelli consigliati.

L'ordine di lavoro che ne segue: prima la lista 3 dal punto 7 in giu' (gli
errori), poi la lista 2 (le macchine altrui), poi la lista 1 (la velocita').
La velocita' e' l'ultima non perche' conti poco, ma perche' e' l'unica delle
tre che si puo' misurare da soli — e quindi l'unica che non ha bisogno che la
beta sia gia' cominciata.
