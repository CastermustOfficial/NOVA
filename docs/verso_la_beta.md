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

1. ~~**Misurare il modello acceso.**~~ Fatto: `banco_modello.py`. 25,8 s a freddo, 1,5 s a caldo, 6,0 t/s. Prompt eval e generazione com'e' oggi, con
   il prompt vero da 11.900 token, non con una frase di prova. Senza questa
   riga tutte le altre sono opinioni.
2. ~~**`-fa` (flash attention).**~~ Non serve: in questa build il valore di
   fabbrica e' `auto`, che vuol dire on. Misurato, nessuna differenza.
3. ~~**KV cache a `q8_0`.**~~ Fatto, ed era il punto giusto: +50% di
   generazione grazie ai layer che la memoria liberata permette. Vecchio testo: (`--cache-type-k`, `--cache-type-v`). Dimezza la
   cache: su una 16 GB con dodici layer sulla CPU, quei layer si ricomprano.
   E' il singolo intervento con il rapporto guadagno/rischio migliore.
4. ~~**`--cache-reuse`.**~~ Misurato, e **non serve**: 1.745 ms contro 1.771,
   dentro il rumore. La ragione e' istruttiva — quel flag riusa i pezzi di
   cache **prima** del punto in cui il prefisso diverge, e qui la divergenza
   e' subito dopo il messaggio di sistema. Non c'e' niente prima da riusare.
   La cura non era il flag, era il taglio: vedi il punto 5.
5. ~~**Ripensare `trim_history`.**~~ Fatto, e il difetto non era **dove** si
   tagliava ma **quanto spesso**. Si tagliava fino a `tetto - 1`, cioe' si
   tornava esattamente sul filo; il turno dopo aggiungeva due messaggi, si
   superava di nuovo, si tagliava di nuovo. **Dal trentesimo turno in poi si
   tagliava a ogni turno** — trentuno tagli su sessanta turni — quindi la
   cache del prefisso non si riformava mai piu' e ogni risposta pagava il
   prompt da capo, per il resto della conversazione. Niente si rompeva e
   nessuno lo diceva.

   Misurato con `banco_taglio.py` (Gemma 4 26B-A4B, 81 messaggi, 15.379
   token di prefisso):

   | | token rielaborati | prompt |
   |---|---|---|
   | a caldo, prefisso intatto | 10 | 130 ms |
   | dopo il taglio di prima | 2.854 | 1.786 ms |
   | e il turno seguente | 2.738 | **1.748 ms** — non guariva |
   | col fondo, dopo il taglio | 1.898 | 1.240 ms |
   | e il turno seguente | 38 | **231 ms** — guarito |

   La cura e' un fondo: superato il tetto si scende a quaranta invece di
   fermarsi a cinquantanove. Non cambia **cosa** si butta, cambia quanto
   spesso: tre tagli su sessanta turni invece di trentuno, e nei turni in
   mezzo il prefisso resta valido. Il prezzo e' che quando si taglia si butta
   di piu' in un colpo, e si paga volentieri — la memoria vera di NOVA non e'
   questa finestra, e' il vault.
6. **Speculative decoding** con un draft piccolo (`--model-draft`). Sul codice
   e sull'output strutturato — che e' quasi tutto quello che NOVA genera — vale
   spesso 1,5-2x.
7. ~~**Provare un MoE davvero**, non solo consigliarlo nel README.~~ Fatto,
   e il README aveva ragione: Gemma 4 26B-A4B Q3_K_XL fa **42,4 tok/s** e
   145 ms di prompt a caldo, contro 6,0 tok/s e 1.363 ms di Qwen3.8 27B
   Q4_K_M — stessa macchina, stessa configurazione, stessa sessione. Sette
   volte. La ragione non e' il MoE in se': e' **30 strati su 30** contro 53
   su 65. Uno ci sta e l'altro no. I numeri sono nel README, con le due
   avvertenze che meritano (quantizzazioni non pari — di proposito, perche' la
   regola e' «la piu' grande che entra» — e si misura la velocita', non la
   qualita' delle risposte).

   **Ne resta una decisione aperta, e non e' tecnica:** `models.json` dice
   `consigliata: true` su Qwen3.8 27B. Su una scheda da 16 GB quel consiglio
   ora ha contro una tabella.

### Il prompt (dove stanno i token)

8. **Gli schemi dei tool sono ~6.900 token, il blocco piu' grosso di tutti** —
   e la voce era scritta guardando la moneta sbagliata. Diceva «stanno nella
   cache del prefisso, quindi il guadagno e' basso»: vero per la **velocita'**,
   falso per la **capienza**. Misurato:

   | | token | quota del contesto |
   |---|---|---|
   | messaggio di sistema | ~5.200 | 32% |
   | schemi dei sessanta tool | ~6.900 | **42%** |
   | riserva per la risposta | 1.024 | 6% |
   | resta alla conversazione | ~3.300 | **20%** |

   Il prefisso fisso si mangia i tre quarti del contesto. Accorciare le
   descrizioni non fa guadagnare millisecondi — quelli sono gia' in cache —
   fa guadagnare **conversazione**, che e' un'altra valuta e quella che
   finisce prima.

   E la seconda meta' della voce adesso si puo' chiudere con una prova invece
   che con un timore: mandare solo i tool pertinenti cambierebbe il prefisso a
   ogni turno, ed e' esattamente la malattia curata al punto 5 — 1.748 ms a
   turno contro 231. **Non si fa.**
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
12. ~~**Primo porting in Rust: ricette + BM25.**~~ Fatto, e andato ben oltre:
    sei pezzi portati (ricette, memoria, registro, modelli, motore, scala).
    Vecchio testo:** Sono algoritmi puri, senza GUI e
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
3. ~~**Percorsi ostili**~~ — e questa voce non ha mai avuto bisogno di una
   seconda macchina: i percorsi ostili si costruiscono qui. Questa ha quelli
   facili (`C:\Users\giova`, niente OneDrive, niente accenti), che e'
   esattamente il motivo per cui nessuno di questi casi era mai stato provato.

   `test_percorsi_ostili.py` costruisce sei cartelle che rompono cose diverse
   — spazi, accenti, apostrofo, parentesi e `&`, trattini, punto iniziale — ci
   mette dentro dei GGUF e ci passa sopra tutta la catena, da tutte e due le
   parti. **Tutto verde al primo colpo**, ed e' un risultato: vuol dire che
   passare le radici come dati invece che come stringhe di comando, scelta
   fatta per poter provare, ha pagato anche qui.

   **Ma il guaio di OneDrive era un altro**, e non e' quello che questa voce
   immaginava. Non e' che il percorso si rompe: e' che **NOVA ci si installa
   dentro**. L'installatore mette i modelli sotto la propria cartella, e la
   propria cartella e' dove qualcuno ha scompattato il file. Se e' Documenti,
   e Documenti e' sincronizzato: dodici gigabyte partono verso il cloud (su un
   piano gratuito da cinque non ci stanno), il vault viene sincronizzato
   mentre NOVA ci scrive e nascono le copie in conflitto, e — la peggiore — i
   file vengono «liberati» per far spazio e restano in elenco come segnaposti
   vuoti. Quest'ultima capita **mesi dopo**, a NOVA che funzionava.

   `nova/cartelle.py` lo riconosce e l'installatore lo dice prima di creare la
   cartella. Non e' un divieto: la cartella e' dell'utente e la scelta e' sua.
   Ma la scelta si fa sapendo, e queste tre conseguenze non le indovina
   nessuno.

   *Una cosa resta scoperta, e va detta.* I percorsi oltre i 260 caratteri qui
   funzionano perche' su questa macchina `LongPathsEnabled` e' **acceso**, e
   il valore di fabbrica e' spento. La prova lo legge dal registro e lo
   scrive, invece di passare in silenzio: passare senza dirlo sarebbe falsa
   sicurezza, cioe' «da me funziona» con un bollino verde sopra.
4. ~~**SmartScreen e antivirus.**~~ Deciso: **si spiega**. Una firma costa
   qualche centinaio di euro l'anno e, per un editore nuovo, non toglie
   comunque l'avviso finche' SmartScreen non gli ha costruito una
   reputazione: si pagherebbe per non risolvere il problema. Il giorno che
   NOVA avra' abbastanza installazioni la firma avra' senso; oggi no.

   «Spiegare» pero' vale solo se la spiegazione arriva **dove capita il
   fatto**, e i posti sono tre.

   - **Nei due README, prima di installare.** Cosa dira' Windows, perche',
     e cosa fa invece l'installer: confronta le impronte SHA256 pubblicate
     con la release. E' un controllo che l'avviso di Windows non fa — quello
     dice «non conosco l'editore», non «questo file e' diverso da quello
     pubblicato».
   - **Nell'installer, nel momento in cui succede.** Prima diceva
     «l'archivio non conteneva tutti i binari attesi» anche quando li
     conteneva tutti e l'antivirus se n'era portato via uno dopo. Sono due
     guasti che si somigliano e si curano in modo opposto: uno e' colpa
     della release e sul PC non c'e' niente da fare, l'altro e' il
     contrario. Adesso l'installer legge l'elenco dei file **dentro
     l'archivio** prima di scompattarlo, e se un binario c'era e poi non
     c'e' piu' lo dice, con il percorso da consentire in Sicurezza di
     Windows.
   - **Sul contrassegno «scaricato da Internet»**, che senza toglierlo fa
     comparire la schermata blu a **ogni** avvio, non una volta. Si toglie,
     ma solo ai file appena verificati, e dicendolo. L'ordine e' la cosa che
     conta e c'e' una prova che lo blocca: prima le impronte, poi
     `Unblock-File`. Al contrario si zittirebbe l'avviso di Windows su un
     file di cui non si sa ancora niente.
5. ~~**Python 3.10, 3.11, 3.12, 3.13.**~~ Chiusa. L'installer dichiarava
   3.10+ e la CI ne provava **una**: non e' una copertura parziale, e' una
   frase che nessuno aveva verificato, e le tre non provate erano
   esattamente quelle su cui l'utente resta da solo.

   Ora la CI le prova tutte e quattro, con `fail-fast: false` — «si rompe» e
   «si rompe solo su 3.10» sono due notizie diverse, e fermarsi alla prima
   nasconde la seconda.

   Ma il rosso su un agente arriva tardi e costa un giro. `test_python_minimo.py`
   controlla tre cose sul portatile: che il numero dichiarato sia lo stesso
   nell'installer e nei due README (una promessa scritta in tre posti si
   sdoppia al primo cambio); che tutti i file si leggano con la **grammatica**
   del minimo dichiarato — `ast.parse(..., feature_version=)` sa fingere di
   essere piu' vecchio di quanto e', quindi quattro grammatiche si provano con
   un interprete solo; e che non si usi niente della libreria standard
   arrivato dopo, perche' `import tomllib` si compila benissimo su 3.10 e poi
   non parte.

   Misurato mentre la scrivevo: 128 file, tutti leggibili con la grammatica
   3.10, e nessun uso di roba piu' nuova. La promessa era vera — ma non lo
   sapeva nessuno, ed e' una differenza che conta.

   E la prova sa dire di no: prima di fidarsi le si fa bocciare `type X = int`
   (che e' 3.12) e riconoscere un `import tomllib` piantato apposta. Una prova
   che passa va guardata come una che fallisce.

### La scheda video

6. ~~**AMD e Intel.**~~ Fatto, ed e' l'unica voce di questa lista che non
   aveva bisogno di un secondo PC. La stima ora chiede a **DXGI**, che
   risponde per qualunque scheda sappia disegnare su Windows; `nvidia-smi`
   resta come ripiego. Costa microsecondi invece di avviare un processo con
   quindici secondi di tetto, e uno zero adesso arriva sempre col motivo.

   Due cose sono uscite scrivendola, e nessuna era prevista.

   **La trappola dell'integrata.** Su questa macchina ci sono due schede: la
   GeForce e una Radeon integrata. La Radeon ha 485 MiB suoi e dichiarava
   **15.643 MiB disponibili** — memoria di sistema che puo' farsi prestare.
   Sono numeri veri, e sono RAM. Scegliere la scheda per memoria *libera*
   avrebbe fatto vincere sempre l'integrata, con la RAM di qualcun altro: il
   rallentamento da dieci volte con l'aria del successo. Ora il libero e'
   tagliato al dedicato, e si sceglie per memoria propria.

   **DXGI e' piu' ottimista di `nvidia-smi`**, e quella e' la direzione
   pericolosa. Misurato: 15.341 contro 14.793 MiB, scarto +548. Non misurano
   la stessa cosa — `memory.free` e' quanto e' libero adesso, il budget di
   DXGI e' quanto il sistema e' disposto a darci sfrattando chi non usa la
   sua — e lo scarto sta dentro la riserva (900 MiB piu' il 4%), che quindi
   smette di essere una cifra scritta a caso. In pratica la stima passa da 53
   a 56 layer, e il ginocchio misurato su questa scheda e' a 60.

   **E il tiro al buio non c'e' piu'.** Quando la VRAM non si leggeva,
   `_gpu_layer_ladder` partiva da `-ngl 64`, e non si poteva correggere: la
   scala di ripiego scende di sei layer a ogni errore di memoria, ma la
   memoria condivisa **non da' errori** — accetta tutto e va dieci volte piu'
   piano. Era un meccanismo di sicurezza che aspettava un'eccezione da
   qualcosa che non ne solleva, cioe' nessun meccanismo di sicurezza.

   Deciso: NOVA deve stare su tutti i PC, quindi **il calcolo e' dovuto
   sempre**. Senza un numero si va sul processore — lento di sicuro invece
   che finto veloce — e lo si dice, con il motivo e con come rimediare a
   mano. Perche' «sul processore» resti un caso raro e non la normale,
   servono piu' fonti, ed e' quello che si e' fatto:

   - il budget di DXGI dove c'e' (**misurata**);
   - la sola memoria dedicata dove il budget non risponde, meno quello che il
     desktop tiene occupato di solito (**dedotta**, con margine doppio);
   - `/sys/class/drm/card*/device/mem_info_vram_*` su Linux, che e' amdgpu;
   - `nvidia-smi` come ultimo ripiego, dove esiste.

   La differenza fra **misurata** e **dedotta** viaggia insieme al numero e
   cambia il margine: su una deduzione se ne tiene novecento MiB in piu',
   perche' non sappiamo cosa la scheda stia gia' usando e l'errore in eccesso
   e' quello che non si vede. Su questa macchina, a parita' di dodici
   gigabyte dichiarati: 42 strati se misurata, 39 se dedotta.

   Il pezzo Linux e' provato con un albero di cartelle finto, non con una
   Radeon: non serve avere la scheda per provare la lettura, serve avere i
   file. Resta scoperto macOS, che vorra' Metal.
7. ~~**Nessuna GPU.**~~ Misurata, ed era il seguito dovuto del punto 6: da
   quando il calcolo degli strati e' dovuto sempre, questa e' la strada su
   cui finisce chiunque abbia una scheda che NOVA non sa leggere. Il README
   prometteva «funziona, piu' lento» senza che nessuno l'avesse
   cronometrato.

   | in CPU pura | prompt a caldo | generazione |
   |---|---|---|
   | Gemma 4 26B-A4B (MoE, 3,8B attivi) | 1,3 s | **7,6 tok/s** |
   | Qwen3.8 27B (denso) | 5,7 s | **1,8 tok/s** |

   La promessa e' vera per meta'. Sul processore si paga per i parametri che
   si **accendono**, non per quelli che ci sono: 7,6 token al secondo sono
   piu' veloci di quanto legga una persona e NOVA si usa; a 1,8 una risposta
   di ottanta token arriva in quarantacinque secondi e non si usa. E il MoE
   in CPU batte il denso sulla GPU di questa macchina (6,0).

   Quindi cio' che si promette a chi non ha una scheda video non e' «un
   modello qualsiasi, piu' lento»: e' **un MoE, e funziona**. Riscritto nel
   README con i numeri.

   **Fatto: senza GPU non si scarica** (`nova/catalogo.py`). Il conto sta in
   un posto solo e l'installatore lo interroga come gia' fa per la ricerca
   dei GGUF — due copie della stessa regola sono due regole destinate a
   divergere. Il campo nuovo in `models.json` e' `frazione_letta`, e la
   soglia (`soglia_gb_per_token: 5.0`) sta anch'essa nel catalogo, perche'
   quando arriveranno misure da altre macchine si cambi il file e non il
   codice.

   Scrivendolo sono usciti **due** vincoli invece di uno, e il secondo l'ha
   trovato una prova che sbagliava per il motivo giusto: oltre alla velocita'
   c'e' lo **spazio**. Sulla GPU il file sta in VRAM; sul processore sta in
   RAM, tutto, e accanto ci devono stare il sistema, il browser e NOVA. Un
   modello abbastanza veloce ma troppo grande entra sulla carta e in pratica
   manda la macchina a paginare su disco — un altro modo di essere
   lentissimi, stavolta con la ventola accesa. Ora i rifiuti sono due frasi
   diverse, perche' chi ha poca RAM puo' comprarne e chi ha un modello troppo
   denso no.

   Il testo qui sotto resta come traccia di com'era la decisione quando l'ho
   ricevuta.

   *Com'era: senza GPU non si scarica.*
   L'installatore oggi propone comunque il modello consigliato e si limita a
   scrivere «servono N GB di VRAM: andra' piano». Con i numeri in mano quella
   riga e' troppo gentile: per un denso da 27B non e' «piu' piano», e' un
   programma che si installa dopo tredici gigabyte di scaricamento e non si
   apre piu'. Il modo di dirlo non e' un avvertimento piu' grosso — e' **non
   offrire la scelta**: senza scheda video leggibile si salta il passo, e al
   suo posto c'e' un suggerimento del tipo «puoi provare piu' avanti, dalla
   configurazione, con un modello estremamente leggero».

   Due cose da definire prima di scriverlo, ed e' la ragione per cui non e'
   gia' fatto.

   *La soglia va detta in numeri, non in aggettivi* — e il numero giusto non
   e' quello che avevo scritto qui la prima volta. «Parametri attivi sotto i
   quattro miliardi» sembra la regola, e non lo e': e' il caso particolare di
   una regola piu' semplice.

   Generare un token, a una richiesta per volta, non e' un lavoro di calcolo:
   e' **leggere i pesi dalla memoria**. La velocita' e' quindi
   `banda / byte letti per token`, e i byte letti sono i parametri che si
   accendono **moltiplicati per quanti bit ciascuno occupa**. La
   quantizzazione entra nel conto quanto l'architettura.

   Le due misure di stasera lo confermano, e danno anche la banda di questa
   macchina:

   | | byte letti per token | tok/s in CPU | banda implicita |
   |---|---|---|---|
   | Qwen3.8 27B Q4_K_M (denso) | 15,7 GB (tutti) | 1,8 | ~28 GB/s |
   | Gemma 4 26B-A4B Q3_K_XL (MoE) | ~3,7 GB | 7,6 | ~28 GB/s |

   La stessa banda spiega tutte e due le righe, il che vuol dire che il
   modello di costo e' quello giusto. E il MoE non legge il 15% del file, che
   sarebbe la proporzione dei parametri attivi: ne legge circa un terzo,
   perche' attenzione e strati condivisi si rileggono a ogni token comunque.

   Quindi il campo che manca in `models.json` non e' «parametri attivi»: e'
   **byte letti per token**, che si ricava da parametri attivi, parametri
   totali e dimensione del file. La soglia diventa una divisione: sotto i
   quattro gigabyte per token si sta sopra i sette token al secondo, che e'
   piu' veloce di quanto legga una persona.

   *L'esempio e' stato verificato, e la risposta e' «non ancora».* Bonsai 27B
   e' interessante e non e' quello che sembrava. Non e' un MoE: e' un **denso
   da 27B derivato da Qwen3.6-27B con i pesi portati a un bit** (Q1_0_g128,
   1,125 bit per peso), 3,8 GB di file, e dichiara il 89,5% del punteggio
   della versione a 16 bit su quindici prove. Se quei numeri reggono e' la
   dimostrazione migliore del paragrafo qui sopra: un denso torna leggero
   comprimendo i pesi invece che spegnendoli.

   Applicandogli la formula: 3,8 GB letti per token, quindi **circa 7,4 tok/s
   su questa macchina in CPU** — praticamente identico al Gemma MoE. Il suo
   vantaggio non sarebbe la velocita', sarebbe la qualita' a parita' di byte.
   E' una previsione, ed e' li' apposta per essere smentita.

   **Avevo scritto che c'era un blocco. Non c'e'.** La scheda di `prism-ml`
   dice di clonare la loro fork di llama.cpp per i kernel `Q1_0_g128`, e da
   quella riga avevo concluso che a monte il formato non esistesse. Bastava
   un comando per verificarlo, sul binario che sta gia' in `runtime/`:

       llama-quantize --help
         40  or  Q1_0    :  1.125 bpw quantization

   `Q1_0` a 1,125 bit per peso — esattamente la larghezza che la scheda
   dichiara — c'e' **gia'** nella build che NOVA scarica (10502). La fork
   serve ai loro kernel ottimizzati, non a far girare il modello: e'
   un'accelerazione, non un requisito. E `lmstudio-community` pubblica una
   riquantizzazione fatta con gli attrezzi standard
   (`llama-quantize --pure ... Q1_0`, 3,80 GB) dichiarata «validated with
   llama-server».

   Quindi Bonsai 27B **e' una candidatura vera**, e per giunta con tre
   proprieta' che la rendono interessante oltre al peso: e' **multimodale**
   (c'e' l'`mmproj`, quindi le schermate funzionano anche col cervello di
   casa), sta **tutta in VRAM con dodici gigabyte di margine** su una scheda
   da 16, e la formula le prevede circa 7,4 token al secondo in CPU pura.

   Resta da misurarla — e' un download da 3,8 GB e il banco e' pronto — ma
   non c'e' piu' niente che lo impedisca.

   *Due note sulle fonti, perche' e' la seconda volta in una sera.* Il
   collegamento arrivato per primo era la variante **MLX**, che e' formato
   Apple Silicon e non gira su Windows. E la scheda di `prism-ml` dice denso
   mentre un articolo che gira lo definisce MoE con 3B attivi: si e' tenuta
   la scheda, che e' la fonte primaria.

   *E una nota di metodo, che vale piu' delle altre due.* La conclusione
   sbagliata veniva dal README di chi ha interesse a mandarti sulla propria
   fork, ed e' stata scritta senza interrogare lo strumento che stava sul
   disco a due metri. Un comando. E' la stessa lezione di tutta la giornata —
   guardare la cosa vera invece di quello che se ne dice — applicata a una
   fonte scritta invece che a un pezzo di codice.

### Il cervello

8. ~~**Modelli che non sono Qwen.**~~ Provato, ed e' la quarta voce che non
   aveva bisogno di una seconda macchina: i modelli sono qui. Finora erano
   stati misurati in **velocita'**, che e' un'altra cosa - un modello puo'
   fare quaranta token al secondo e non saper chiamare un tool, e allora quei
   token non servono a niente.

   `banco_cervello.py` non chiede «quanto e' veloce» ma «sceglie il tool
   giusto fra sessanta»: dieci domande con un tool atteso, tre a cui si
   risponde parlando (che e' il caso che i modelli piccoli sbagliano di piu',
   chiamando qualcosa per compiacenza).

   | | tool giusti | inventati | mancati | chiamate di troppo |
   |---|---|---|---|---|
   | Gemma 4 26B-A4B Q3_K_XL | 7 su 8 | 0 | 0 | 0 |
   | Qwen3.8 27B Q4_K_M | 7 su 8 | 0 | 0 | 0 |

   **Gemma regge il confronto con Qwen**, e la riga del README che lo
   consiglia adesso ha una prova sotto invece di una speranza. Nessuno dei due
   inventa strumenti, nessuno dei due ne chiama uno quando basta parlare.

   **Ma sbagliano la stessa domanda**, ed e' il motivo per cui questa voce ha
   fruttato piu' di quanto chiedesse. Vedi il punto 15.

15. ~~**«Ricordati che...» non finisce in memoria.**~~ Chiusa, e la chiusura
    e' che la voce misurava la strada sbagliata.

    Prima cosa, la diagnosi vecchia era imprecisa. Rimisurato pulito su Gemma,
    a temperatura 0: «Ricordati che il mio gatto si chiama Ugo» non chiama
    `kb_search`. Non chiama **niente**: risponde a parole, «certo, me lo
    ricordero'». Le altre due forme («Il mio gatto si chiama Ugo», «Salva in
    memoria: ...») chiamano `kb_note` giuste, e «Che cosa sai di me» chiama
    `kb_search` giusto. Tre su quattro. A vacillare e' solo l'imperativo, e
    vacilla fra parlare e agire, non fra i due strumenti.

    Seconda cosa, e piu' importante: **ho provato a forzarlo dal prompt e l'ho
    peggiorato**. Ho rafforzato la descrizione di `kb_note` e il prompt di
    sistema — «rispondere lo ricordero' senza scrivere e' mentire». Risultato
    misurato: da 3/4 a **1/6**. Detto piu' forte, il modello obbedisce di
    meno: la lingua insistente lo spinge a rassicurare a parole invece di
    agire. Ripristinato tutto. Vale la pena tenerlo scritto: sul tool calling
    di un modello locale, alzare la voce nel prompt e' spesso
    controproducente.

    Terza cosa, quella che chiude la voce. NOVA ha **due strade** verso la
    memoria, non una. La prima e' `kb_note`, che il modello chiama o no. La
    seconda e' l'apprendimento automatico: un estrattore in sottofondo che a
    ogni turno rilegge lo scambio e scrive da se' i fatti durevoli — e legge
    il messaggio dell'**utente**, non solo la risposta. Il banco misurava solo
    la prima strada. Ma la domanda dell'utente non e' «hai chiamato lo
    strumento giusto?», e' «te lo sei ricordato?».

    Misurato end-to-end, nel caso peggiore (il modello non chiama niente,
    risponde solo «certo, me lo ricordero'»):

        utente: Ricordati che il mio gatto si chiama Ugo.
        NOVA (solo parole): Certo, me lo ricordero'!
        -> vault: [il-gatto-di-gio] «Il gatto di Gio si chiama Ugo.»

        utente: Ricorda che lavoro meglio la mattina presto.
        NOVA (solo parole): Perfetto, ne terro' conto.
        -> vault: [preferenza-orario-di-lavoro] «Gio lavora meglio
                   durante le prime ore del mattino.»

    Il fatto arriva in memoria comunque. La promessa del README —
    «una memoria che sopravvive alle sessioni» — e' mantenuta dalla seconda
    strada, che il banco non aveva mai guardato. E' D51 di nuovo: due
    implementazioni che concordano non sono verificate, e un banco che misura
    lo strumento scelto non misura il risultato. La prova giusta chiede
    «e' finito nel vault?», e la risposta e' si'.

    Resta un margine di lucidatura per dopo, non un blocco: l'imperativo
    potrebbe far scattare `kb_note` piu' spesso, cosi' il fatto compare
    **subito** in conversazione invece che al giro dell'estrattore. Ma non e'
    la differenza fra ricordare e dimenticare — e' la differenza fra ora e
    fra due secondi.
9. **Le CLI dichiarate ma non provate**: Gemini, Codex, Qwen. Sono nel menu.
   Ognuna ha permessi e formato di output suoi.
10. **Gli endpoint API.** OpenRouter, Groq, Together parlano lo stesso dialetto
    «quasi»: il tool calling e' il punto dove smettono di somigliarsi.
11. ~~**Il modello locale senza `mmproj`.**~~ Chiusa, e la misura ha
    cambiato la forma della cura. Avevo scritto «`schermo` non deve
    rompersi»: `schermo` non si rompeva affatto, scattava benissimo. A
    rompersi era il **turno dopo**.

    Misurato: `llama-server` avviato senza proiettore, una chiamata con
    dentro un `image_url`, e risponde **HTTP 500** con
    `image input is not supported - hint: [...] provide the mmproj`. Da li'
    tre difetti che non si vedevano:

    - `spiega_http` mandava ogni 5xx a «di solito passa da solo». Questo non
      passa da solo: e' un file che non e' stato scaricato. Mandare qualcuno
      ad aspettare una cosa che non succedera' mai e' peggio che dirgli
      «non lo so»;
    - il messaggio con la figura **restava in conversazione**, quindi il
      turno dopo la rimandava e falliva uguale. Non un turno perso: una
      conversazione murata finche' non la si butta;
    - e nessuno chiedeva **prima**. `runtime` sapeva gia' se il proiettore
      c'era — lo cercava per decidere se passare `--mmproj` — ma chi
      allegava le figure non glielo domandava mai.

    Ora la domanda si fa prima (`vede_il_modello_locale`), la figura non
    parte e al modello arriva una riga che gli dice di non fingere di aver
    guardato e di usare `ui.tree`; se l'errore arriva lo stesso — server
    adottato, endpoint di qualcun altro — si sfila l'immagine e si riprova
    una volta, e la spiegazione dice la verita'.

    Da tenere a mente per la prossima volta: il difetto non stava dove
    diceva la voce. Una schermata consegnata a un modello cieco non e' uno
    strumento che fallisce, e' uno strumento che **riesce** e avvelena il
    resto della conversazione — e sono i secondi quelli che non si trovano
    guardando l'elenco degli strumenti.

### Il contorno

12. ~~**Solo Chrome ed Edge**, via CDP.~~ Scritto. Non c'era niente da
    provare: c'era da **dirlo**. Il README diceva «NOVA pilota Chrome», che e'
    sbagliato in tutte e due le direzioni — cerca prima Edge, e Firefox non lo
    pilota affatto. Ora dice quali due, in che ordine, e cosa succede se non
    c'e' nessuno dei due: il resto di NOVA funziona e i comandi del browser
    dicono che non trovano un browser da pilotare.

    Una voce chiusa scrivendo una frase invece che del codice conta quanto le
    altre. La differenza fra alpha e beta non e' quante cose fa un programma:
    e' quante di quelle che dice di fare sono vere.
13. **macOS e Linux.** Oggi `non_implementato.rs` compila e non fa niente. La
    decisione da prendere non e' tecnica: o e' una promessa con una data, o si
    dice che NOVA e' un programma Windows.

14. ~~**La macchina che sviluppa NOVA non fa partire quello che fa partire
    l'installer.**~~ Fatto, e il difetto era piu' grande di come l'avevo
    scritto. Non erano due copie: erano **quattro elenchi** di cosa e' fatto
    NOVA, scritti a mano in posti diversi — la CI che raccoglie i binari,
    l'installatore che controlla di averli, i processi da fermare nella
    disinstallazione, e `build.ps1`, che non ne copiava nessuno. Si erano gia'
    disallineati: aggiungendo `nova-schede` avevo aggiornato la CI e non
    l'installatore, senza nessun errore, perche' su questa macchina il binario
    c'era comunque.

    Ora l'elenco sta in `core/binari.json` — non e' codice, e' un dato, come
    `models.json` — e lo leggono tutti. `build.ps1` pubblica in `bin\` dopo
    ogni compilazione di release, quindi chi sviluppa fa girare esattamente
    quello che gira all'utente.

    La parte che vale piu' del resto e' `test_binari.py`: pretende che
    l'elenco corrisponda ai bersagli veri del workspace **in tutte e due le
    direzioni** — chi aggiunge un eseguibile e non lo mette nell'elenco trova
    la suite rossa, e cosi' chi scrive un nome che non esiste. Verificata
    togliendo e aggiungendo una voce per vedere che diventasse davvero rossa.
    I banchi di confronto restano fuori di proposito: vivono dietro la feature
    `banco` e non si consegnano a nessuno.

    E provandolo per davvero e' uscita una cosa in piu'. Con NOVA aperta la
    compilazione falliva dopo un minuto e mezzo con «failed to remove file ...
    Accesso negato. (os error 5)» piu' un traceback di PowerShell: il nome di
    un errore, non un messaggio — D28 applicata al build. Adesso `build.ps1`
    guarda prima se NOVA sta girando e lo dice in italiano in un secondo.
    `-Controlla` resta permesso, perche' `cargo check` non scrive binari ed e'
    proprio cio' che serve a chi vuole sapere se il codice sta in piedi senza
    chiudere l'assistente che sta usando.

---

## Lista 3 — Attrito cognitivo

Non «cosa non funziona» ma «cosa fa sentire stupido chi lo usa». E' la lista
che di solito non si scrive, ed e' quella che decide se qualcuno lo tiene
installato dopo il primo giorno.

### I primi cinque minuti

1. ~~**Cosa vede uno appena finita l'installazione?**~~ Fatto: l'orb accoglie
   con tre prove da fare subito (`PROVE` in `index.html`), e ne cambia il testo
   se il cervello non c'e' ancora. `test_primi_minuti.py` pretende che ci
   siano e che siano cose che funzionano di sicuro.
2. ~~**Il README e' lungo.**~~ Fatto: «I primi cinque minuti» e' la prima
   sezione, prima di qualunque spiegazione.
3. ~~**L'orb dice se e' acceso?**~~ Fatto, ed era rotto in silenzio: il
   battito di stato esisteva in Python e non arrivava mai all'interfaccia.
   Ora passa da `nova://passo`.

### Quando lavora

4. ~~**Trenta secondi di attesa senza niente sembrano rotti.**~~ Fatto:
   `nova/attesa.py`. Dice a che passo e', e da quanto sta andando — ma non si
   inventa quanto manca (D32).
5. ~~**La conferma deve dire *cosa* fa.**~~ Fatto: `test_anteprime.py`, 125
   controlli su tutti i tool, non sui primi dieci.
6. ~~**Il registro azioni si legge?**~~ Fatto: `registro.cerca` con filtri per
   testo, tipo, esito e giorni, piu' il racconto in italiano. Poi portato in
   Rust (`nova-registro`).

### Quando non ce la fa

7. ~~**Nessun traceback deve arrivare all'utente.**~~ Fatto in casa:
   `nova/guasti.py` traduce, il traceback va nel file (D28), e
   `test_guasti.py` fa la guardia perche' non rientri — ha gia' bocciato me
   due volte. Resta da provare **sulle strade di qualcun altro**, ed e' per
   questo che il cancello della beta ce l'ha ancora aperto.
8. ~~**«Non ci riesco» deve dire perche' e cosa fare.**~~ Fatto: sono quattro
   messaggi diversi, e la quota e' `LimiteUso` invece di un errore generico
   (D30) — prima il ripiego non partiva mai.
9. ~~**Il verificatore dell'harness.**~~ Fatto: `nova/harness_prova.py`
   trova ed esegue i test del progetto, e `applica(verifica=True)` scrive solo
   se il verdetto non peggiora — confronto con **prima**, non col verde
   assoluto (D31).

14. ~~**L'orb si apre due volte.**~~ Chiuso e **provato dal vivo**: due
    avvii di `nova-shell.exe` di fila, e il secondo si chiude da solo mentre
    il primo resta — stesso PID prima e dopo. La guardia e'
    `tauri-plugin-single-instance`, registrata come **primo** plugin (deve
    vedere l'avvio prima di chiunque altro), e alla seconda apertura richiama
    l'orb che c'e' gia' invece di crearne un altro: `finestre::richiama`, che
    lo rimette nell'angolo in basso a destra dello schermo principale.

    Nota che la voce e' costata piu' del suo codice non per il codice ma per
    la **verifica**: Windows non lascia riscrivere un `.exe` mentre gira, e
    l'orb tiene aperto `nova-shell.exe`, quindi il binario nuovo non si
    compilava finche' NOVA era accesa. `build.ps1` adesso se ne accorge e lo
    dice invece di lasciar fallire `cargo` dopo un minuto — ma la prova vera
    resta chiudere, compilare, riaprire, e guardare due avvii diventare uno.


### Fiducia

10. ~~**«Dove sono i miei dati?»**~~ Fatto: `nova/dati.py` e `--dati`.
    Risponde cosa c'e', dove sta, quanto pesa, e cosa succede se lo cancelli.
11. ~~**«Cosa esce dal mio PC?»**~~ Fatto: la fascia della riservatezza nel
    pannello lo dice mentre si sceglie il cervello, distinguendo cosa resta in
    casa da cosa va a un fornitore e a quale.
12. ~~**Disinstallare deve togliere tutto**, dire cosa ha tolto e cosa ha
    lasciato apposta.~~ Chiuso, e la meta' che mancava era la seconda.

    Il «cosa ho tolto» c'era gia': riga per riga, con «rimosso» o «non
    c'era», comprese le attivita' pianificate — che sono l'unica cosa che
    *continua a girare* dopo la disinstallazione. Il «cosa ho lasciato»
    invece era una frase: «i tuoi dati restano dove sono». Vera e inutile,
    perche' non diceva **dove**, e i posti sono tre, non uno:

        Le credenziali          %APPDATA%\NOVA        via con -ConIDati
        Il fascicolo            Documenti\NOVA        resta sempre
        La memoria a grafo      dove l'hai messa tu    restava, in silenzio

    `-ConIDati` cancellava `%APPDATA%\NOVA` e basta. Il vault, se
    configurato altrove — ed e' il caso normale, chi lo apre in Obsidian lo
    tiene con le sue note — sopravviveva senza che nessuno lo dicesse. E il
    modello, che sono sedici gigabyte, non era nominato da nessuna parte.

    Ora l'elenco lo fa `nova.dati.rendiconto()`, cioe' **la stessa lista che
    risponde a «dove sono i miei dati»**, e l'installer la legge in JSON
    invece di riscriverla: una seconda copia di cio' di cui una cosa e'
    fatta si disallinea sempre, e questa e' la terza volta che il progetto
    lo impara. Alla fine il disinstallatore stampa cosa resta, con nome,
    peso e percorso, e con quale comando toglierlo.

    Una scelta che resta: **fuori da `%APPDATA%\NOVA` non si cancella
    niente**, nemmeno con `-ConIDati`. Il fascicolo sono file scritti
    dall'utente; il vault puo' essere una cartella di Obsidian che l'utente
    usa anche per i fatti suoi. Un disinstallatore che cancella qualcosa che
    non ha creato lui e' un disinstallatore di cui non ci si fida mai piu'.
    Si dice dove sta e si lascia decidere a chi possiede il file.
13. ~~**Il menu delle impostazioni e' disordinato.**~~ Fatto: tre fasce —
    chi ragiona, come ti parla, com'e' messa.

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

**Il tredicesimo pezzo: la fusione.** `nova-nodi::fusione` porta la parte di
`Vault.upsert` che non tocca il disco — cosa succede quando NOVA impara
qualcosa su un fatto che **sa gia'**. E' il posto dove la memoria si corrompe
in silenzio: un nodo peggiorato ha lo stesso aspetto di un nodo giusto, e
nessuno se ne accorge finche' non serve.

Le regole sono tutte difetti gia' successi, e i commenti del Python li
chiamano per nome: un «fatto» generico che declassava una persona (e il file
restava in `02-persone` mentre l'indice diceva `06-fatti`); la confidenza che
saliva a ogni **riformulazione**, cioe' premiava proprio il caso in cui NOVA
non aveva imparato niente; un primo paragrafo piu' lungo del tetto che
congelava il nodo per sempre, facendo sparire in silenzio ogni fatto nuovo.

Trentuno confronti, zero divergenze al primo colpo — la prima volta nel
cantiere. Le regole erano gia' scritte bene: qui il porting non ha trovato
difetti, ha trovato conferme.

**Il dodicesimo pezzo, e il primo del vault.** `nova-nodi`: cos'e' un nodo
della memoria e come si scrive su disco. Arriva subito dopo il guardiano —
cosa **puo'** entrare in memoria — che era stato scritto per una porta che
ancora non c'era.

Il vault e' una cartella di `.md` che si apre in Obsidian, e questo vuol dire
che il formato **e' un contratto**: se le due meta' scrivono il frontmatter in
due modi, la prima che rilegge il file dell'altra perde dei campi in silenzio.
Percio' il banco non confronta una funzione, confronta il **giro completo**, e
anche incrociato: il Python deve rileggere il file del Rust come il proprio, e
viceversa.

Due divergenze, e la prima e' quella da ricordare. «Œuvre» dava `oeuvre` in
Rust e `uvre` in Python, perche' `Œ` in Unicode non ha nessuna decomposizione
e NFKD la butta. Il mio era piu' bello ed era **sbagliato**: il nome del file
e' un contratto gia' firmato con i file che stanno nel vault adesso, e
cambiarlo li rinomina tutti — due nomi per lo stesso nodo sono due nodi. La
seconda: la confidenza `1.0`, che Rust stampa `1` e Python `1.0`. Un carattere,
e il file diverge.

**L'undicesimo pezzo, e la domanda che non faceva nessuno.**
`nova-cartelle` riconosce se una cartella e' sincronizzata col cloud prima che
ci finiscano dentro dodici gigabyte. Chiude, con `nova-catalogo`, il gruppo
delle cose che l'installatore chiede **prima** che Python esista: adesso
entrambe le domande le fa un binario.

Ma la parte che conta e' un'altra. `solo_segnaposto` — il rilevatore del guaio
**peggiore** dei tre, il modello «liberato» dal cloud che resta in elenco con
zero byte dentro — esisteva in Python da settimane, con la sua prova, e in
tutto il programma **non la chiamava nessuno**. NOVA descriveva con precisione
il guasto che capita mesi dopo, a NOVA che funzionava, e non lo guardava mai.
Adesso sta in `nova-platform` (e' una domanda di filesystem, e quelle stanno
li') e l'installatore la fa davvero, prima di accettare un modello che
l'utente indica.

Una prova apposta pretende che `install.ps1` la chiami: una funzione che c'e'
e che nessuno invoca e' come non averla, e la differenza non si vede finche'
non serve.

E un dettaglio di lingua, che qui e' sostanza: lo stesso servizio usciva
scritto in due modi nella stessa installazione — «OneDrive» se riconosciuto
dalla variabile d'ambiente, «Onedrive» se riconosciuto dal nome della
cartella, perche' il secondo passava da un `.title()`. Ora c'e' una tabella di
come si scrivono. E' il genere di sciatteria che fa sembrare un messaggio
generato invece che scritto, proprio nel punto in cui deve essere creduto.

**Il decimo pezzo, e il primo che toglie Python da una strada vera.**
`nova-catalogo` decide se un modello ha senso su questa macchina, quale
variante, e cosa dire mentre lo si fa. E' il pezzo con il criterio d'ordine
piu' limpido di tutto il cantiere: lo chiama **l'installatore**, che gira
prima che le dipendenze del progetto esistano. In Python era di sola libreria
standard proprio per questo; in Rust il vincolo sparisce, perche' e' un
binario e non ha niente da installare.

`install.ps1` adesso chiede al binario e ripiega su Python solo per chi
compila da sorgente e non ha ancora i binari. La regola resta scritta in un
posto solo — l'installatore non ne ha una copia sua — ma non serve piu' un
interprete per applicarla.

Due difetti trovati **dall'integrazione, non dalle prove di unita'**, ed e' la
parte da ricordare. Il primo: PowerShell scrive un BOM davanti al JSON, e
serde si fermava con «expected value at line 1 column 1» — vero e inutile. Non
e' sciatteria di chi chiama: e' l'ambiente in cui quel binario deve
funzionare, ed e' compito suo incontrarlo li'. Il secondo, peggiore: il banco
faceva `unwrap_or_default()` su una domanda illeggibile, e la famiglia vuota
che ne usciva produceva un verdetto **perfettamente formato** — «legge almeno
0.0 GB per token, non te lo faccio scaricare». Sembrava una risposta. Sarebbe
stato un installatore che rifiuta ogni modello dando all'utente una ragione
inventata.

**Il nono pezzo.** `nova-salita`: quando si sale di gradino, e quando si
sta solo girando a vuoto. Chiude il gruppo delle decisioni insieme a
`nova-scala` e `nova-pianificazione`, ed e' il pezzo che decide **quando un
compito esce dal PC** — salire quando non serve manda fuori roba che poteva
restare in casa, non salire quando serve lascia l'utente davanti a un muro.

Due sintomi diversi con due cure opposte, e vale la pena tenerli distinti:
sbattere contro un muro (N fallimenti di fila) si cura **salendo**; girare a
vuoto (la stessa chiamata con gli stessi argomenti, magari riuscendo ogni
volta) non si cura salendo, perche' non c'e' niente da far salire — si cura
**facendolo notare**. La ripetizione produce un promemoria, mai un divieto:
la decisione resta al modello, e una ripetizione legittima non viene bloccata
da niente. C'e' una prova apposta su questo, perche' se un domani diventasse
un blocco il confronto fra le due implementazioni resterebbe verde — sarebbero
d'accordo nel fare la cosa sbagliata.

**L'ottavo pezzo.** Portati: ricette, memoria, registro, modelli, motore,
scala, guasti, **calendario e pianificazione**. L'ottavo sta nel gruppo delle
decisioni insieme a `nova-scala`: `prossimo()` traduce «ogni lunedi' alle 9»
nell'istante in cui tocca, e sbagliarlo di un giorno vuol dire un'attivita'
che non parte o che parte a ripetizione.

E' arrivato con un pezzo che non era in programma. `giorni_del_mese` esisteva
gia', privata, dentro `nova-registro`, e serviva di nuovo al calendario della
pianificazione. Scriverne una seconda copia sarebbe stata la quarta volta —
dopo l'elenco dei binari, le cartelle sincronizzate e i posti dei dati — che
questo progetto si accorge di aver duplicato cio' di cui una cosa e' fatta.
Alla seconda occorrenza si mette in comune, non alla quarta: e' nato
`nova-calendario`, e `nova-registro` adesso lo usa al posto della sua copia.

`nova-calendario` non ha un orologio dentro, di proposito: l'ora si passa
sempre da fuori, come `oggi` in `nova-registro::giorno`. Cosi' si prova a
qualunque ora, anche il 29 febbraio, anche a Capodanno. E non ha fusi: un
fuso e' una domanda di piattaforma e sta in `nova-platform`; qui resta la
parte che non cambia mai.

**Il settimo pezzo, e il conto di quel che resta.** Portati: ricette,
memoria, registro, modelli, motore, scala, guasti. I primi tre erano logica pura; gli ultimi due sono
il gruppo che risponde alla domanda «cosa c'e' su questo PC» — dischi, GGUF,
schede video, llama-server — cioe' tutto quello che va saputo **prima** del
primo avvio. Quel gruppo adesso e' finito, e sta insieme: le radici da
percorrere si passano sempre da fuori, e chi parla al sistema operativo sta
sempre in `nova-platform`.

Il sesto apre un gruppo nuovo, quello delle **decisioni**: `nova-scala` e'
la parte di `routing.py` che sceglie chi risponde a cosa. E' il pezzo con le
conseguenze piu' pesanti del cantiere, perche' e' quello che tiene o rompe la
frase su cui NOVA sta in piedi - niente esce dal PC finche' qualcuno non
delega davvero. Nello stesso gruppo restano da portare l'auto-valutazione e
`pianificazione.py`.

Il settimo, `nova-guasti`, non appartiene a nessuno dei due gruppi: e' quello
che serve a tutti. Quando il Python andra' via, ogni pezzo in Rust che debba
dire «non ci sono riuscito» dovra' dirlo in italiano e senza far uscire una
chiave, e quel codice deve esistere prima.

**C'e' un ordine, e non e' quello delle liste.** Portando i primi quattro
pezzi e' venuto fuori un criterio che nessuna delle tre liste conteneva:
**quando** una cosa deve funzionare. `modelli_trova.py` porta scritto in testa
che e' di sola libreria standard «perche' viene eseguito dall'installatore
prima che le dipendenze del progetto siano garantite» — cioe' il primo passo
dell'installazione dipende da qualcosa che l'installazione non ha ancora
fatto. Quel modulo va portato per primo non perche' sia lento o incompatibile,
ma perche' e' il solo che deve girare su una macchina dove NOVA non c'e'
ancora. Chi viene dopo, con lo stesso criterio: il rilevamento della GPU, il
recupero del runtime, la scelta della quantizzazione. Tutto cio' che parla
all'utente **prima** del primo avvio.

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
3. ~~**La disinstallazione e' pulita** e lo dice.~~ Fatto: dice cosa ha tolto riga per riga, e cosa ha lasciato apposta con nome, peso e
   percorso. Resta da provarlo su una macchina che non e' questa.
4. **Le compatibilita' dichiarate sono provate**, oppure sono state tolte dal
   README. Nessuna promessa in sospeso.
5. **I numeri del README sono misurati**, anche quelli dei modelli consigliati.

L'ordine di lavoro che ne segue: prima la lista 3 dal punto 7 in giu' (gli
errori), poi la lista 2 (le macchine altrui), poi la lista 1 (la velocita').
La velocita' e' l'ultima non perche' conti poco, ma perche' e' l'unica delle
tre che si puo' misurare da soli — e quindi l'unica che non ha bisogno che la
beta sia gia' cominciata.
