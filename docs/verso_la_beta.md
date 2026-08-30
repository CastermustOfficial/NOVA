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
9. ~~**Verificare che il prefisso sia davvero stabile.**~~ Fatto:
   `test_prefisso.py` confronta due prompt a ora ferma e pretende che siano
   identici, e controlla che memoria e ricette restino in coda alla domanda.

### Il codice (dove sta poco, ma si puo' prendere)

10. ~~**`contesto_per` a 25 ms.**~~ Fatto, e non come previsto: i
    ventidue millisecondi non erano l'indice, erano `refresh_if_changed()`
    che faceva lo `stat` di tutti i file del vault a ogni messaggio. Una riga
    di attesa, e sono 3,2 ms. Resta l'indice persistente fra un avvio e
    l'altro, che adesso pero' vale 0,35 ms.
11. ~~**Avvio a freddo: 206 ms di import dei tool.**~~ Fatto, e non con
    l'import pigro per categoria: 115 dei 163 ms erano `requests`, tirato
    dentro da un solo modulo. Spostato dentro la funzione, sono 94 ms. Se
    servisse di piu', il passo successivo e' quello previsto.
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

14. **La macchina che sviluppa NOVA non fa partire quello che fa partire
    l'installer.** Su questo PC l'avvio automatico punta a
    `core\target\release\nova-shell.exe` - il prodotto della compilazione -
    mentre `install.ps1` lo punta a `bin\nova-shell.exe`. Sono due binari
    diversi che si possono disallineare in silenzio, e il secondo e' l'unico
    che un utente vedra' mai. E' la forma piu' pura di «da me funziona»:
    l'unica macchina su cui NOVA e' provata sta provando qualcos'altro.

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

13. **L'orb si apre due volte.** Non c'e' una guardia di istanza singola:
    due doppi clic sul collegamento danno due orb, che si contendono lo
    stesso demone e la stessa configurazione. Si vede subito e sembra un
    guasto. In Tauri si risolve con la guardia di istanza singola, che alla
    seconda apertura mostra la finestra che c'e' gia' invece di crearne
    un'altra.


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

---

## Da fare, ma dopo

Cose decise e messe da parte apposta, per non confonderle con le tre liste
qui sopra: quelle portano alla beta, queste vengono dopo.

### Il trascrittore

Registrare una chiamata — una call di lavoro, una riunione — e restituire non
la trascrizione ma **cio' che serve dopo la chiamata**:

- il trascritto con **chi ha detto cosa** (diarizzazione, non un muro di
  testo);
- il riassunto, e separati i **punti decisi** da quelli rimasti aperti;
- **le cose da fare**: chi, cosa, entro quando — pronte per finire nelle
  attivita' pianificate di NOVA;
- **le bozze di risposta** gia' scritte: la mail di riepilogo, la risposta a
  chi ha chiesto una cosa, il documento promesso.

I pezzi ci sono quasi tutti. `nova-voce` gia' trascrive (whisper.cpp e
ElevenLabs Scribe); l'harness gia' e' il posto dove un testo lungo si guarda
mentre se ne parla; le proposte gia' sanno nascere dentro un documento. Quello
che manca e' la cattura dell'audio di sistema (non solo del microfono: in una
call meta' delle voci arrivano dall'altoparlante), la diarizzazione, e il
formato del risultato.

Vale la pena notare perche' e' un caso buono per NOVA e non per una chat: la
registrazione **non deve uscire dal PC**. E' esattamente il tipo di materiale
per cui il modello locale non e' un ripiego.

### Il cantiere Rust, e perche' non e' un progetto di velocita'

Deciso: prima si finisce l'attrito cognitivo, poi si apre il cantiere.

E vale la pena scrivere l'errore di inquadratura, perche' e' facile
rifarlo. Il banco dice che il Python costa ventotto millisecondi per turno,
e da li' la conclusione «riscriverlo non serve» sembra ovvia. E' giusta
sulla velocita' e sbagliata sull'obiettivo: guardando la lista
compatibilita', meta' delle voci **spariscono** se sul PC dell'utente non
c'e' piu' Python.

- CMP-5 (3.10 / 3.11 / 3.12 / 3.13) non esiste piu'.
- CMP-2 (utente senza diritti) diventa banale: un `.exe` non installa niente.
- CMP-4 (SmartScreen) si firma una volta, non ventisette pacchetti pip.
- OTT-10 (206 ms di import) sparisce.
- ATT-1 (nessun traceback) non si puo' nemmeno produrre.

Il porting e' un progetto di **distribuzione e di robustezza**, non di
prestazioni. Detto cosi', vale i mesi che costa; detto come «per andare piu'
veloce», no.

Due conseguenze pratiche.

**Portare a pezzi non paga finche' resta un solo file Python.** Se meta' sta
in Rust e meta' no, l'utente installa comunque Python e ci sono due
implementazioni della stessa cosa da tenere allineate. Il guadagno arriva
tutto insieme, alla fine.

**Meno si dipende da Windows, meglio e'.** Questo cambia cosa si scrive nel
cantiere: non «Rust su Windows» ma il disegno che `nova-platform` ha gia' -
un trait, tre backend. Ogni pezzo che si porta va scritto contro il trait,
anche quando l'unico backend implementato e' quello Windows: e' la
differenza fra avere macOS e Linux a una implementazione di distanza e
doverli riscrivere da capo. Il punto 13 della lista compatibilita' smette di
essere una decisione da prendere e diventa un ordine di lavoro.


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
