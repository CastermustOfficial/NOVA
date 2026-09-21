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
   facili (nome utente senza spazi ne' accenti, niente OneDrive), che e'
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

   **Correzione, 3 settembre.** «Lo scarto sta dentro la riserva» era vero
   quel giorno, su quella macchina, a desktop fermo. Con un gioco aperto lo
   scarto e' diventato **+2.643** contro una riserva di 1.407, cioe' undici
   strati di troppo — proprio il rallentamento da dieci volte che questa
   voce doveva chiudere.

   Il difetto non era il numero: era la forma. Lo scarto non scala con la
   scheda, scala con **quanto stanno usando gli altri**, e una riserva fissa
   non puo' coprirlo per costruzione. Misurare una volta, a macchina scarica,
   e concluderne una costante e' lo stesso errore di prima in un vestito
   nuovo.

   Ora, dove c'e' `nvml.dll`, si prende il minore fra DXGI e il numero del
   driver. Non e' un ritorno a `nvidia-smi`: DXGI resta la risposta per
   tutti, NVML e' una libreria e non un processo, e puo' solo abbassare.
   Misurato subito dopo: 12.724 contro 12.725. Con due schede NVIDIA non si
   corregge niente, ed e' scritto nel codice il perche'. Vedi D92 e D93.

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

    **E la voce non era finita.** Due giorni dopo, di nuovo: «non e' spawnato
    nova all'avvio del pc». NOVA partiva — l'orb stava a x=-113, quasi tutto
    sul monitor che l'utente non vede. La via di ritorno l'avevo messa solo in
    `richiama()`, cioe' solo se qualcuno fa doppio clic una **seconda** volta;
    all'avvio l'orb tornava nel posto salvato dopo aver controllato che quel
    posto fosse su uno degli schermi elencati, e lo era.

    Ci avevo messo una regola — all'avvio l'orb torna sul principale se il
    posto salvato non e' li' — e **l'ho ritirata lo stesso giorno**. La
    premessa era che DISPLAY2 fosse uno schermo spento; e' invece uno schermo
    che l'utente vede e su cui tiene l'orb apposta, e la regola gliela
    spostava a ogni accensione. Curare il sintomo con l'ipotesi sbagliata
    costa la cosa che si voleva proteggere.

    **E il fatto era scritto.** `runtime/guscio.log` — che avevo cercato in
    `%APPDATA%` e non trovato, concludendone che non esistesse — tiene una
    riga per ogni avvio dal 30 agosto:

        2026-09-03T08:17:15.341Z  guscio in avvio
        2026-09-03T08:17:15.640Z  orb rimesso dov'era x=-104 y=1129
        2026-09-03T08:17:15.993Z  demone acceso all'avvio

    Sono le 10:17:15 locali; il PC si era acceso alle 10:16:10. **NOVA parte
    all'avvio, sessantacinque secondi dopo** — e non per lentezza sua, visto
    che fra la prima riga e l'ultima passano 0,65 secondi. E' Windows che
    ritarda apposta i programmi della chiave `Run`.

    Per chi guarda lo schermo, un minuto di niente non e' un ritardo: e' «non
    e' partita». Curato dove andava curato — **come** parte, non dove si
    mette: `install.ps1` registra un'attivita' pianificata «all'accesso», che
    quel ritardo non ce l'ha, e toglie la vecchia voce in `Run`.

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

### Windows si appoggia a NOVA, non il contrario

Deciso da Gio il 5 settembre, e riscrive cosa vuol dire «finire CANT-2».

Il conto di partenza — a memoria, e per questo sbagliato, vedi D136: «quattordici» punti in cui PowerShell non e' uno
strumento che NOVA usa, ma cio' che la regge. Gli appunti *sono*
`Get-Clipboard`. Il volume *e'* `SendKeys`. La cattura dello schermo *e'*
`Add-Type -AssemblyName System.Drawing`. L'elenco delle applicazioni *e'* una
query WMI incollata dentro una stringa.

Non e' una questione di eleganza. Vuol dire tre cose concrete:

- **tre dipendenze in mezzo** per ogni gesto: un processo da avviare, una
  shell che interpreta, una stringa da comporre — con il rischio delle
  virgolette ogni volta che ci finisce dentro un dato dell'utente;
- quelle capacita' **non esistono** dove PowerShell non c'e', o dove una
  policy aziendale lo blocca: e non degradano, spariscono;
- e la forma di NOVA cambia col sistema sotto, che e' il contrario di quello
  che deve succedere.

**La regola.** Il tratto lo dichiara il modulo che ne ha bisogno — `nova-strumenti`
dice «mi serve qualcuno che sappia copiare un testo negli appunti» — e la
piattaforma lo implementa. Mai il contrario: se fosse `nova-platform` a
dichiarare le sue capacita' e `nova-strumenti` ad adattarsi, sarebbero i verbi
di Windows a decidere la forma di NOVA.

I verbi sono di NOVA: «copia questo testo», non «chiama `SetClipboardData`».
Il backend Windows chiama l'API **diretta**, senza shell in mezzo. E ogni
capacita' ha una risposta anche dove non c'e' Windows, fosse solo «qui non si
puo', e te lo dico» — perche' una capacita' che sparisce in silenzio e' peggio
di una che manca (D130).

E' anche la ragione per cui i corpi dei file sono venuti bene: li' non c'era
niente da chiedere a Windows, solo `std::fs`. Il resto degli strumenti va
portato con la stessa disciplina, non traducendo le stringhe di PowerShell in
stringhe di PowerShell scritte in Rust.

**Il primo pezzo, e cosa ha fatto saltare fuori.** Gli appunti: `nova-platform`
che chiama Win32 diretto — `OpenClipboard`, `GetClipboardData`,
`SetClipboardData` — un binario `nova-appunti` che gli strumenti preferiscono,
e il ripiego PowerShell che resta ma **dichiarato**. Misurato: 179 ms contro
19, e di quei 19 quasi tutti sono l'avvio del processo.

Mettendo le due strade una accanto all'altra e chiedendo se dicessero la stessa
cosa, si e' scoperto che il ripiego **storpiava gli accenti**: PowerShell
scrive su stdout con la tabella codici della console e `_ps` leggeva UTF-8, per
cui «perche' citta' pero'» tornava con i punti interrogativi al posto delle
lettere. Nessun errore, uscita zero. Da `_ps` passano molte capacita', quindi il
guasto era di tutte — e cercando le altre chiamate sono saltate fuori dieci
chiamate a una shell in cinque moduli, con tre difetti diversi (D131, D135). Vale la pena dirlo perche' non
lo stavo cercando: l'ha trovato la disciplina di D130, non un sospetto.


**E poi ho contato.** «Quattordici» me l'ero ricordato, non misurato, e
l'avevo scritto in D130, nel diario, nei commenti del codice e in tre messaggi
di commit. Contate con un analizzatore di sintassi, le funzioni che passano da
una shell sono **24**, di cui **13 strumenti** esposti al modello. E fra i
tre esempi che avevo dato, uno era falso: la cattura dello schermo non passa
da PowerShell affatto — usa `mss` e `PIL`. Quel `System.Drawing` che
ricordavo e' delle notifiche, cioe' proprio il pezzo che avevo appena
riscritto.

Il numero sbagliato e' innocuo. L'esempio falso no: avrebbe mandato a
riscrivere una cosa che quel problema non ce l'ha (ne ha un altro — due
pacchetti Python — che e' un'altra decisione). Vedi D136.

**Il conto vero, oggi.** Tredici strumenti toccano ancora una shell. Quattro
hanno gia' la strada diretta e la tengono solo come ripiego dichiarato; nove
ci dipendono davvero.

| Strumento | Cosa usa oggi | Misurato |
|---|---|---|
| `read_clipboard` | Win32 diretto; PowerShell solo se manca il binario | 19 ms (era 179) |
| `write_clipboard` | idem | 19 ms |
| `set_volume` | Core Audio; `SendKeys` solo se manca tutto | letto davvero |
| `notify` | processo suo che aspetta al posto di NOVA | 5 ms (era 9.300) |
| `system_info` | API dirette; la query WMI solo se manca il binario | 41 ms (era 1.543) |
| `list_installed_apps` | registro diretto; PowerShell solo se manca il binario | 55 ms (era 594) |
| `list_windows` | `EnumWindows` diretto; PowerShell solo se manca il binario | 23 ms (era 275) |
| `focus_window` | `SetForegroundWindow` diretto, **con verifica** | e dice se Windows ha detto no (D142) |
| `close_application` | elenca, mostra, chiude **un pid** | via il modello di ricerca (D141) |
| `open_application` | `ShellExecuteExW` diretto | via il guaio delle virgolette |
| `type_text` | `SendInput` Unicode, **col fuoco verificato** | e la risposta nomina la finestra (D143) |
| `press_keys` | `SendInput`, e i simboli si rifiutano | dipendono dalla disposizione della tastiera |
| `list_processes` | `psutil`, e ripiega su `list_windows` se manca | 645 ms |
| `delete_path` | `IFileOperation`; le altre due strade solo se manca | si rompeva su un apostrofo (D147) |
| `move_path` | idem, quando lo spostamento passa dal Cestino | idem |

`create_reminder` non e' piu' in tabella: chiama `schtasks` con un elenco di
argomenti, come si chiama un programma. Non e' una shell — la differenza non
e' nominale, e' che non c'e' niente da comporre e quindi niente da rompere.

Questa tabella e' tenuta ferma da `test_conto_shell.py`, che la confronta con
cio' che il codice fa davvero **in tutti e due i versi**: nessuno che chiami
una shell puo' restare fuori, e nessuno puo' restarci dopo essere stato
portato. E' l'unico modo perche' un elenco scritto a mano non racconti
un'altra storia sei mesi dopo (D46, D136).

Ha gia' corretto la tabella due volte. La prima: ci avevo messo
`list_processes` fra quelli che dipendono da una shell, e non ci dipende
direttamente — usa `psutil`. La seconda e' piu' istruttiva. Quando
`close_application` ha spostato il suo ripiego dentro una funzione
d'appoggio, la prova ha smesso di vederlo: guardava solo il corpo dello
strumento, e **un conteggio che si azzera spostando tre righe in un'altra
funzione non e' un conteggio**. Ora segue un livello di chiamata — e appena
l'ha fatto ha trovato tre strumenti che la tabella non aveva mai nominato:
`list_processes` (per il ripiego su `list_windows`), `delete_path` e
`move_path` (per il Cestino, che senza `send2trash` passa da PowerShell).

L'ordine l'ha deciso la misura: `system_info` costava quasi un secondo e
mezzo, piu' di tutte le altre messe insieme, ed e' anche quella che il modello
chiede per prima quando vuole sapere dove si trova. E' stata la successiva, e
il tempo si e' rivelato la parte meno interessante: confrontando le due
risposte sono usciti due difetti che con la velocita' non c'entravano — una
descrizione che prometteva batteria e rete senza darle (D137) e i numeri
scritti nella lingua dell'utente, con due separatori decimali diversi nella
stessa risposta. Piu' un terzo, nella strada **nuova**: il registro dice
«Windows 10 Pro» su una macchina con Windows 11, e la query WMI che stavo
buttando via diceva giusto (D138).

Le prossime, per costo misurato: `list_installed_apps` (594 ms),
`list_windows` (275 ms). Le altre non sono state cronometrate perche'
cambiano lo stato del PC — si aprono finestre, si preme la tastiera — e una
misura non deve fare danni per sapere quanto costa.

### La lista del cantiere

Quindici pezzi fatti, e per la prima volta vale la pena scrivere quelli che
restano — non come promemoria, ma perche' l'ordine conta e finora l'ho scelto
un pezzo alla volta. Sceglierlo un pezzo alla volta e' anche il modo in cui ho
riscritto una cosa che c'era gia' (D99).

Il criterio dell'ordine e' uno solo: **quanto un pezzo avvicina il momento in
cui sul PC non serve piu' Python**. Non quanto e' bello portarlo.

Sigla `CANT-`. Le righe sono quelle del Python di oggi, e sono una misura di
mole, non di difficolta'.

| | Pezzo | Righe | Perche' li' nell'ordine |
|---|---|---|---|
| ~~CANT-1~~ | ~~**Il vault su disco**~~ — **fatto** | ~660 | Era il seguito diretto di `nova-nodi`, ed e' stato il primo pezzo scritto contro un tratto invece che sopra il filesystem nudo: e' quello che apre la strada a tutti gli altri. Ha ripagato prima di essere finito — la scrittura delle note dell'utente non era atomica (D102) — e ha portato dentro anche il guardiano dei segreti (D109, D110) |
| ~~CANT-2~~ | ~~**Gli strumenti**~~ — **fatto**, per la parte traducibile: dichiarazioni, guardie, formato, i corpi dei file, la shell, i tasti, le pagine, la **scelta** di cosa ricordare, e i quindici strumenti che chiedono davvero alla piattaforma. Quel che resta in `nova/tools/` appartiene ad altri cantieri, file per file (D150) | ~2.570 | Sono la meta' di NOVA che tocca il PC, ed e' esattamente quella che in Python costa di piu' in dipendenze. Tanti pezzi piccoli e indipendenti: si e' portato uno strumento per volta senza fermare niente |
| ~~CANT-3~~ | ~~**Il ciclo dell'agente e i cervelli**~~ — *tutto cio' che decide: fatto — il contesto, il prompt, i blocchi, le immagini, le procedure, i guasti, cosa si dice a un cervello che vive fuori, e i tre modi di non farcela (D213, D216, D217). Il **giro** e' un crate suo, `nova-ciclo`, provato con un mondo finto, e il braccio che lo attacca ai cervelli e agli strumenti veri e' `nova-core::mondo`. Il **taglio della conversazione** — il pezzo piu' delicato di tutto il progetto — era gia' in Rust da prima (`nova-contesto`, col suo banco) e adesso e' **attaccato**: `mondo` taglia prima di chiedere e si rimette da se' i campi che il taglio non guarda (D221, D222, D223). Chi tiene la conversazione fra un turno e l'altro e' `nova-core::sessione` (D226), e chi costruisce la scala dalla configurazione e' `mondo::scala_vera` (D227, D228). **Chiuso** per la parte che gli spettava: quel che resta del ciclo in Python e' impalcatura, cioe' CANT-7* | ~2.200 | E' il pezzo che davvero libera dal Python, ma va dopo gli strumenti: un ciclo che chiama strumenti Python non ha liberato niente. Dentro c'era la parte piu' delicata di tutto il progetto — il taglio del contesto a token, che se sbaglia perde pezzi di conversazione senza dirlo: **fatta**, ed e' stata la prima cosa a uscire da `agent.py` invece che l'ultima |
| ~~CANT-4~~ | ~~**Lanciare il modello locale**~~ — **fatto**: le decisioni (la riga di comando, la scala, i sei modi di dire «non ci sta»), il **giro** (cosa si fa quando non parte, D213; quanto si aspetta, D214) e chi lo esegue — `modello.accendi` nel demone, che avvia, aspetta la salute, legge il registro e scende di gradino da se' (D215). Quel che resta in Python e' il ripiego per chi non ha i binari, e muore con il resto | ~690 | Il calcolo degli strati era gia' in `nova-modelli`; restava il pezzo dove le decisioni si vedono poco e costano molto — la riga di comando, la scala dei layer, l'unico errore che vale la pena riprovare (D173) |
| ~~CANT-5~~ | ~~**Il server MCP**~~ — **fatto**: il protocollo, le trentatre' dichiarazioni, il rischio, la domanda in chiaro, gli allegati e la risposta al permesso. I corpi degli strumenti appartengono ai cantieri che chiamano | ~1.290 | Protocollo, quindi traducibile senza scelte — ma le **buste** hanno una regola che rompe i client quando si sbaglia (D175, D176) |
| ~~CANT-6~~ | ~~**Il browser e la ricerca**~~ — **fatto**, per la parte traducibile: i nove copioni che girano nella pagina, il confine fra argomento e codice, la scelta della scheda, cosa di una pagina e' testo, e i due raschiatori del motore. Quel che resta e' avviare Chrome e tenere la connessione: processi e rete, e appartiene a CANT-7 | ~760 | Nessuna scelta di interfaccia, ma il pezzo dove il confine fra argomento e codice conta piu' che altrove: quel testo lo esegue un interprete che non e' nostro (D178). Ed e' il cantiere in cui il banco ha trovato un difetto vero, non una differenza di porto (D181) |
| ~~CANT-7~~ | ~~**L'impalcatura**~~ — **fatto**: le guardie predefinite (D185); le **regole** di lettura della configurazione, provate da tutte e due le parti e con dentro quattro difetti in meno (D229, D248, D249, D250) — il crate e' `nova-configurazione`, col suo banco; la mappa dei dati e' `nova-dati` (D243, D244); il catalogo di cio' che si scarica e' `nova-componenti` (D245, D246, D247); la connessione a Chrome e' `nova-cdp`, ed era l'ultima scelta di libreria aperta di tutto il cantiere (D240, D241). `main.py` non si porta: i punti d'ingresso Rust ci sono gia' | ~1.800 | Non si porta: si **riscrive**, perche' meta' esiste solo per tenere insieme il Python. Andava per ultima fra quelle di sostanza, quando si sa cosa deve tenere insieme — e cosi' e' stato |
| CANT-8 | **L'harness dei documenti** — *la strada e' scelta: **prima la logica, poi la finestra**. **La logica e' fatta**: `nova-harness` (i blocchi e la ricerca: D269→D272), `nova-harness::modifica` (la proposta, e il rifiuto di scrivere sopra un file cambiato sotto: D273, D274), `nova-docx` (la chirurgia sul `.docx`, il premio di D237: D275, D276) e `nova-harness::prova` (il verificatore: «non e' verde, e' peggio di prima» — D277, D278). Resta **la finestra**, che e' la domanda che era stata rimandata apposta* | ~3.100, di cui **1.518 senza una riga di Qt** | Non e' «2.900 righe di finestra Qt»: solo `harness_finestra.py` tocca Qt, e di quelle 1.582 righe la parte Qt sono 164. Le 1.518 senza Qt sono **fatte**. La finestra e' un'altra domanda, e da quando il guscio e' Tauri con l'interfaccia in HTML, e' la domanda «una quarta finestra del guscio?» invece di «quali widget?» — e si fa guardando, non scrivendo |
| CANT-9 | **Mac e Linux, parita' piena** — *le prove Python girano in CI anche su Ubuntu e macOS, e sono **verdi** (D232). Il primo giro ha trovato cinque difetti che da Windows non si vedevano: una cartella che si spostava (D231), i percorsi protetti che fuori da Windows non proteggevano niente (D230), una prova che lasciava il mondo senza permessi (D233), una porta che chiedeva meta' di quel che serviva (D234) e una che misurava la velocita' della macchina (D235). Poi sei dei tredici pezzi di `nova-platform`: il **Cestino** secondo la specifica freedesktop (D259, D260), i **processi** — con un modo di spegnere tutto che Windows non ha (D258) — le **informazioni di sistema** (D261), e appunti, volume e notifiche (D262). Restano quelli che chiedono un ambiente grafico vero: **tastiera, finestre e l'albero di accessibilita'**, piu' l'avvio automatico, che qui vuol dire scrivere un installatore che non c'e'* | ~3.300 | Non e' in coda per caso: e' il primo cantiere che **non si puo' provare da qui**. Quel che resta e' esattamente la parte che non si puo' nemmeno scrivere onestamente da qui: un albero di accessibilita' scritto senza una macchina su cui guardarlo produrrebbe un elenco di controlli plausibile e falso, che e' peggio del rifiuto onesto che c'e' adesso |
| ~~CANT-10~~ | ~~**I fogli di calcolo**~~ — **fatto**: `nova-fogli` legge **e** scrive. I riferimenti (`A1`, `$B$7`, `C10:A1` che e' la stessa area di `A1:C10`), la regola che decide se un valore e' un numero (D254), come si legge una cella con dentro un conto mai calcolato (D253), e la scrittura che non spoglia il file — formule, formati, grassetti e secondo foglio restano, provato su un `.xlsx` vero (D239, D255). Di riflesso, le due letture Python che davano due testi diversi dello stesso file sono diventate una (D256) | ~150 oggi | Il pubblico lo chiede, e NOVA sapeva fare **meta' della meta'**: leggeva il testo delle celle, in sola lettura. Nasceva direttamente come crate, ed e' andata cosi': aritmetica e formati, cioe' esattamente il genere di cosa che si porta bene e si prova meglio |
| ~~CANT-11~~ | ~~**NOVA parla MCP da un lato solo**~~ — **fatto**: `nova-mcp-cliente`. Non e' un tubo, e' un **cancello**, e le regole sono cinque: un server si dichiara e non si scopre (D263), le parole di un estraneo si citano e cio' che comanda si dice invece di toglierlo di nascosto (D264), i nomi portano davanti quello del server (D265), uno strumento altrui non e' mai «sicuro» (D266), e quel che entra ha una misura (D267). Provato contro un server MCP vero, che ha trovato un difetto che nessuna prova scritta a mano aveva trovato (D268) | ~400 | `nova-mcp` diceva di se': «il protocollo con cui NOVA **si apre** a un altro programma». Era vero, ed era meta': NOVA sapeva farsi usare e non sapeva usare. Andava dopo i fogli perche' e' un cancello e non uno strumento — e infatti il codice del tubo e' un terzo, il resto e' chi puo' entrare |
| CANT-12 | **Le decisioni che oggi sono euristiche** | ~0 righe nuove, molte da togliere | NOVA decide un mucchio di cose con liste di parole, soglie e regex: quale cervello serve, se una frase e' un fatto da ricordare, se un risultato e' pertinente, se una chiamata e' rischiosa. Ognuna di quelle e' un giudizio travestito da conto. Restano cosi' non per scelta ma perche' l'alternativa costava un giro di LLM per ogni domanda, cioe' secondi e soldi. I **modelli System One** cambiano quel conto. Sta in fondo perche' e' un cantiere che si **prepara** adesso e si chiude quando ci sara' qualcosa da misurare: prima si danno un nome alle decisioni e un secondo braccio, poi si sceglie cosa spostare — e si sceglie con un banco, non con le cifre di chi vende |

Due cose che la tabella non dice.

**Il cancello.** Finche' resta un solo file Python, l'utente installa Python
lo stesso: il guadagno non e' proporzionale al lavoro fatto, arriva tutto
insieme quando esce l'ultimo. Percio' l'ordine non e' «prima i pezzi facili»
ma «prima quelli che sbloccano gli altri».

**Cosa non e' nella lista.** La voce e' gia' tutta in Rust; il demone, le
capacita', i segreti e la supervisione anche. Il Python di `nova/voice/` resta
solo come ripiego per chi non ha i binari, e muore da solo quando muore il
resto.

**Portare a pezzi non paga finche' resta un solo file Python.** Se meta' sta
in Rust e meta' no, l'utente installa comunque Python e ci sono due
implementazioni della stessa cosa da tenere allineate. Il guadagno arriva
tutto insieme, alla fine.

### CANT-9 — Mac e Linux: cosa manca davvero

La prima sorpresa e' buona, ed e' misurata: **il nucleo Rust si compila gia'
anche altrove**. Ogni modulo che tocca Windows ha gia' accanto il suo gemello
per gli altri sistemi, e quel gemello non e' vuoto: dice onestamente cosa non
sa fare. «le notifiche di sistema qui non ci sono». «premere i tasti qui si fa
in un altro modo». Sono tredici file in `nova-platform`, piu' qualche punto in
`nova-shell`, `nova-core`, `nova-proto` e `nova-cli`.

Vuol dire che la parita' non parte da zero: parte da **tredici caselle vuote
da riempire, due volte**. Le righe che oggi toccano Windows sono ~3.300 sulle
4.025 di `nova-platform`, e sono la misura di cosa va rifatto per macOS e per
Linux — non uguale, perche' meta' di quel codice e' logica che resta.

| Cosa | Su Windows | Altrove |
|---|---|---|
| appunti, tastiera, finestre | Win32 diretto | X11/Wayland e AppKit: due mondi diversi anche dentro Linux |
| notifiche | toast di sistema | `notify-send` e `NSUserNotification` |
| cestino | shell API | il cestino di freedesktop.org, che e' un file di metadati accanto |
| dischi, sistema | Win32 | `/proc`, `sysctl` |
| GPU | DXGI | Metal e `/sys/class/drm` — e senza DXGI non c'e' un modo solo di sapere quanta VRAM e' libera |
| nuvola | cartelle note nel profilo | iCloud Drive si chiama in un altro modo e sta in un altro posto |
| **avvio automatico** | una chiave di registro | **qui non si porta: si decide.** Un `.plist` di `launchd`, un `.desktop` in `autostart`. Non e' una traduzione, e' un'altra cosa che fa lo stesso mestiere |

Poi c'e' il contorno, che non e' Rust e pesa uguale: `install.ps1` e' 1.296
righe di PowerShell, e `build.ps1` altre 183. Mac e Linux vogliono il loro
installatore — e la trappola e' gia' scritta in D204: due installatori che
tengono la loro copia degli elenchi sono due elenchi destinati a divergere.
Quello che c'e' chiede a NOVA; anche gli altri due dovranno.

**Il primo passo pero' non e' codice: e' un lavoro della CI su `macos-latest`
e `ubuntu-latest`.** Questa macchina non puo' dire niente su quei due sistemi,
e la notte del 14 settembre ha mostrato cosa costa scrivere codice che nessuna
macchina diversa guarda mai: un filtro che non filtrava da sempre, e una
configurazione che su una macchina spoglia non nasceva. Prima si accende la
luce, poi si guarda.

**Accesa il 16 settembre, e cosa ha detto subito.** Prima di scrivere il
lavoro della CI ho preso una macchina Linux vera e ci ho costruito il
nucleo. Quattro risposte, tutte utili:

- **si compila tutto**, tranne `nova-shell` - che vuole gtk e webkit
  installati, ed e' l'involucro Tauri, cioe' proprio il pezzo che CANT-9
  dovra' portare davvero;
- serve **una sola cosa da fuori**: `libasound2-dev`, perche' cpal parla
  ad ALSA. Su macOS non serve, Core Audio c'e' gia';
- `nova-cartelle` era rossa **cinque prove su sette** - e non per colpa
  della funzione. Le prove erano scritte solo con i backslash, e su Linux
  un backslash non separa niente: tutto il percorso e' un componente solo
  e non ci si trova mai «onedrive». Le prove descrivevano un sistema solo;
- sistemate quelle: **505 prove verdi, zero rosse**.

E due cose che si vedono **solo** da li', e che nessuna prova su Windows
avrebbe mai potuto dire:

- su macOS OneDrive non sta nel profilo: sta in
  `~/Library/CloudStorage/OneDrive-Personal`. Trattino **senza** spazio -
  che e' esattamente la forma che la regola rifiuta apposta, per non
  scambiare `dropbox-export-2024` per una cartella sincronizzata. Non e'
  un difetto da correggere di corsa: e' una decisione da prendere, perche'
  allargare la regola rimette in gioco il falso allarme che era costato
  scriverla;
- nella tabella dei nomi c'e' `icloakdrive`, che e' un refuso per
  `iclouddrive`: una riga che non puo' corrispondere a niente. Sta in
  Python **e** in Rust, identica - portata fedelmente, refuso compreso. Il
  banco confronta i due elenchi e li trova d'accordo, perche' **una prova
  gemella dimostra che due cose sono uguali, non che hanno ragione**;
- `proiettore_accanto` in Python usa `Path.glob`, che su Windows **non guarda
  le maiuscole** e altrove si'. Un modello con accanto `MMPROJ-F16.GGUF`
  scritto in maiuscolo: su Windows NOVA vede il proiettore, su Linux e macOS
  no - e lo stesso modello e' multimodale o cieco a seconda del sistema. Il
  lato Rust confronta in minuscolo apposta, ed e' quello giusto: qui e' il
  Python da allineare. Trovato dal banco gemello girato su Linux, che e'
  l'unico posto da cui quella differenza si vede.

### CANT-10 — I fogli di calcolo: cosa c'e' gia', e cosa non c'e'

Oggi NOVA sa fare meta' della meta': legge il testo delle celle di un `.xlsx`,
in sola lettura, e non sa scrivere niente. Lo fa in **due posti** —
`nova/tools/documenti.py` per lo strumento e `nova/fascicolo.py` per il
fascicolo — che e' il solito odore delle due copie (D73).

E c'e' un difetto vero, misurato invece che sospettato. Tutti e due aprono il
file con `data_only=True`, cioe' chiedono il **risultato** invece della
formula. Sembra la scelta giusta, e su un file salvato da Excel lo e'. Su un
file scritto da un programma e mai aperto da Excel il risultato non e' salvato
da nessuna parte, e quella cella torna **vuota**:

    A1 = 3, A2 = 4, A3 = «=A1+A2»
    letto con data_only=True   ->  3, 4, None
    letto con data_only=False  ->  3, 4, "=A1+A2"

Cioe': a chi genera un foglio con uno script e poi chiede a NOVA di leggerlo,
NOVA risponde che i totali sono vuoti. Non sbaglia il numero: nega che ci sia.

**La decisione: i file, non Excel.** Leggere e scrivere `.xlsx` e `.csv` da
se', senza che Office sia installato. Non e' una rinuncia: e' la scelta che
funziona anche sul PC di chi Office non ce l'ha, e - non per caso - anche su
Mac e Linux, quindi CANT-10 aiuta CANT-9 invece di litigarci. Il ponte COM
verso l'Excel aperto sullo schermo resta una cosa possibile, dopo, e
dichiaratamente solo per Windows.

Cosa serve, in ordine di quanto e' chiaro cosa fare:

1. **leggere davvero**: valore *e* formula, i formati (una data non e' il
   numero 45.000), i fogli, le celle unite, e dire quale delle due si sta
   guardando invece di scegliere in silenzio;
2. **scrivere**: celle, formule, un foglio nuovo, senza spogliare il resto del
   file - la stessa regola che vale per i `.docx` (si modifica, non si
   riscrive);
3. **il `.csv` vero**, che non e' un formato ma una famiglia: separatore,
   codifica, virgolette, prima riga che a volte e' intestazione e a volte no;
**Due segnalazioni, e cosa dicono davvero.** Gio ha trovato un articolo e un
progetto. L'articolo (`excelwiz.net`) e' una panoramica di cosa Excel sa fare
da se' con l'IA — Ideas, Power Query, formule dinamiche, Python dentro Excel:
niente da adottare, ma dice cosa la gente **intende** per «automatizzare
Excel», ed e' quasi tutto analisi e cruscotti.

Il progetto e' un'altra cosa. `sbroenne/mcp-server-excel` e' l'altra meta' del
bivio, fatta per bene. Misurato clonandolo: circa **140.000 righe di C#** su
.NET 10, licenza MIT, **27 famiglie di comandi** — Power Query, DAX, VBA,
`=PY()`, PivotTable, grafici, slicer, XML Map. Pilota l'**applicazione Excel
vera** via COM, e lo dichiara in testa senza girarci intorno: richiede
Windows, Excel 2016 o piu' recente, **un desktop interattivo**, e l'accesso
esclusivo alla cartella di lavoro.

Non cambia D210: la conferma. Quella roba non si adotta come dipendenza senza
portarsi dentro .NET, Windows, Excel e uno schermo acceso — cioe' tutto cio'
da cui CANT-9 sta cercando di uscire.

Dice pero' due cose utili:

- le sue 27 famiglie sono un **catalogo misurato** di cosa si chiede a Excel,
  che vale piu' di quello che indovineremmo noi. Quando si decidera' cosa deve
  saper fare `nova-fogli`, e' da li' che conviene guardare;
- la meta' «Excel vero» si puo' avere **senza possederla**: e' un server MCP,
  con licenza MIT. Chi ha Windows ed Excel se lo installa e NOVA lo usa; chi
  non ce l'ha ha comunque i file. Solo che oggi NOVA non puo' — e questo e' il
  pezzo nuovo, qui sotto.

4. **cosa e' una tabella** in un foglio fatto da una persona - intestazioni
   che non stanno alla riga 1, righe vuote in mezzo, totali in fondo. Questa e'
   la parte difficile, ed e' la stessa difficolta' del taglio del contesto: un
   foglio grosso non entra in un prompt, e decidere cosa mostrare e' una
   decisione, non un troncamento.
### CANT-12 — Le decisioni che oggi sono euristiche

NOVA e' piena di punti in cui **decide**, e quasi tutti decidono con liste di
parole, soglie e espressioni regolari. Non per pigrizia: l'alternativa era
chiedere a un LLM, e un LLM per ogni domandina vuol dire secondi di attesa e
soldi, su una cosa che deve rispondere subito. Quindi si e' scritto un conto
al posto di un giudizio, e il conto sbaglia dove i conti sbagliano.

I **modelli System One** — un modello che non scrive testo ma risponde a
domande tipizzate, con una probabilita' e una confidenza — cambiano quel
conto. Non e' questo il posto per decidere se e quale usarne uno: e' il posto
per scrivere **quali decisioni sono in gioco**, perche' quella e' la parte che
vale a prescindere.

#### Il censimento

| Dove | Come decide oggi | Che domanda e' | Cosa dovrebbe uscire dal PC | Cosa costa sbagliare |
|---|---|---|---|---|
| `nova-scala::gradino_minimo` | liste di parole per categoria, piu' un numero minimo di allegati | **choice** fra i gradini | il testo del compito | salire quando non serve manda fuori casa roba che poteva restare; non salire lascia l'utente davanti a un muro |
| `nova-salita::serve_salire` | conta fallimenti e passi | resta un conto: **sta bene com'e'** | niente | — |
| `kb/memory.py::osserva` | `len(testo) >= 25 caratteri` | **noul**: «qui dentro c'e' un fatto durevole sull'utente?» | lo scambio, cioe' quanto di piu' personale ci sia | una soglia di lunghezza impara le frasi lunghe e inutili e butta «mi chiamo Gio» |
| `kb/retrieval.py`, `nova-memoria` | somiglianza a vettori + parole | **score** di pertinenza per candidato | il pezzo di vault e la domanda | un richiamo sbagliato non si vede: il modello risponde sicuro su un contesto che non c'entra |
| `ricette`, `procedure_da_secondi: 8` | ha impiegato piu' di otto secondi | **noul**: «valeva la fatica, questa?» | il racconto di cosa si e' fatto | l'archivio si riempie di procedure banali e propone quella sbagliata |
| `Risk` per strumento | dichiarato una volta, statico | **score** sulla **chiamata**, non sullo strumento | il comando cosi' com'e' | `run_command("ls")` e `run_command("rm -rf /")` hanno oggi lo stesso rischio dichiarato |
| `forme_riservate`, `nova-guasti::chiavi` | prefissi noti (`sk-`, `gsk_`, `AIza`) e regex | **noul**: «questo testo contiene una credenziale?» | **il segreto stesso: e qui la risposta e' no** | l'elenco sa solo quel che gli hanno detto (D185), ma chiederlo fuori vuol dire mandare fuori proprio la cosa da proteggere |
| `nova-guasti::spiega` | corrispondenze su stringhe d'errore | **choice** fra le spiegazioni | il messaggio d'errore, che puo' contenere percorsi e nomi | un errore spiegato male manda l'utente a cercare dalla parte sbagliata |

#### Il confine, che e' gia' deciso

**Tutto tranne i segreti** (D236). Qualunque decisione qui sopra puo' essere
presa fuori casa; quelle che riguardano credenziali e forme riservate no, e
non per adesso: per sempre. Chiedere a un servizio esterno «questa e' una
chiave?» vuol dire mandargli la chiave, e il costo e' esattamente il difetto
che si voleva evitare. Se quella domanda si fara', si fara' al cervello **in
casa**.

E sopra tutto resta `solo_locale`, che non e' una preferenza fra le altre: e'
la promessa che niente esce dal PC. Acceso, non esce niente — nemmeno le
materie che di solito possono.

Quel confine **non e' scritto qui**: e' scritto in `nova-decisioni`, in una
forma che non si puo' aggirare distrattamente. `Fuori::prepara` e' l'unico
modo di avere del materiale pronto a uscire, e da li' un segreto non si
costruisce. Non c'e' un controllo da ricordarsi di fare piu' avanti, perche'
piu' avanti non ci si arriva. Cinque mutazioni su cinque prese, compresa
quella che fa uscire il segreto e quella che ignora `solo_locale`.

#### Come si prepara il terreno

Non scrivendo l'integrazione. Un pezzo di codice scritto contro un servizio
che non si e' mai chiamato e' «da me funziona» in una forma nuova: non
funziona nemmeno da me.

Si prepara cosi', ed e' la stessa forma di tutto il resto che ha funzionato:

1. **Le decisioni prendono un nome e un tipo.** Ognuna delle righe qui sopra
   diventa una domanda dichiarata — scelta, punteggio, si'/no — con dentro
   cosa si guarda e cosa si risponde. Finche' sono sparse dentro le funzioni
   che le usano, non si possono ne' confrontare ne' sostituire.
2. **Due bracci dietro lo stesso tratto.** Il primo e' l'euristica di oggi,
   che resta e non si tocca. Il secondo e' vuoto, e aspetta. Il giorno in cui
   c'e' qualcosa da chiamare, si attacca li' senza riscrivere niente.
3. **Il banco prima della scelta.** Le due teste rispondono alle stesse
   domande sugli stessi casi, e si guarda dove divergono. E' esattamente cio'
   che si fa gia' fra Python e Rust, e per la stessa ragione: **due
   implementazioni che concordano non sono due implementazioni verificate**,
   ma due che divergono dicono dove guardare.
4. **La misura e' nostra.** Chi vende un modello pubblica i propri numeri.
   Questo progetto ha una regola sola su questo — misurato, non immaginato — e
   vale anche qui: il banco misura latenza e accordo sui casi di NOVA, non su
   quelli di chi vende.

Il guadagno che non dipende da nessun fornitore: **al punto 1 si e' gia'
vinto qualcosa**. Una decisione che oggi sta dentro un `if` diventa una cosa
con un nome, provabile da sola e mutabile in una prova. Anche se il secondo
braccio restasse vuoto per sempre, le euristiche sarebbero meglio provate di
adesso.

### I pezzi piccoli che restano

Non sono cantieri: sono cose che stanno in mezz'ora l'una, e che restano
aperte solo perche' nessuno le ha scritte in un posto dove si rivedono. Ognuna
e' verificata oggi, non ricordata.

| Cosa | Dove | Stato misurato |
|---|---|---|
| **Le impronte dei binari locali** | `bin/SHA256SUMS.txt` | Tre righe per **quindici** eseguibili, e le tre non corrispondono piu' ai file. Non e' tracciato, quindi non e' mai uscito di qui - il manifesto vero lo fa la CI al rilascio - ma finche' c'e', a chi lo apre dice una cosa falsa. O si rigenera sapendo cosa contiene, o si butta |
| **L'orb non cambia mai faccia** | `nova-shell/src/main.rs:165` | `stato_orb` esiste ed e' registrato fra i comandi (riga 322), e **nessuno lo chiama**: non compare in nessuna pagina dell'interfaccia. NOVA che pensa e NOVA che aspetta si vedono uguali |
| **Il modo di passare la domanda a una CLI** | `impostazioni.html:905` | `prompt: 'argomento'` e' scritto dentro un valore predefinito e non e' un campo: chi aggiunge una CLI che vuole la domanda sullo standard input non ha modo di dirlo dal pannello |
| **L'etichetta di Gemini gia' salvata** | la configurazione di chi ce l'ha | La voce nuova - «Gemini (licenza enterprise o chiave API)» - arriva solo a chi installa da adesso. Sovrascriverla vorrebbe dire cancellare una scelta che l'utente **puo'** aver fatto, visto che dal pannello l'etichetta si cambia. Per chi ce l'ha gia', la verita' la dice la prova |
| ~~Il client websocket per il browser~~ | `tungstenite`, in `nova-cdp` | **Scelta e misurata** contro un Chrome vero (D241): era l'ultima scelta di libreria rimasta aperta di tutto il cantiere. Bloccante e non async, perche' il CDP e' domanda-risposta e una sola connessione alla volta. E ha trovato una cosa che nessuna lettura della documentazione avrebbe dato: da Chrome 111 la connessione viene rifiutata con 403 se l'origine non e' fra quelle permesse (D240) |
### Da dove si riprende

Scritto qui e non in una chat, perche' una chat finisce e questo file no.

**Fatti, dentro CANT-2, sotto D130** — ognuno con la sua prova e la sua misura:

| | Era | E' | Cosa e' saltato fuori |
|---|---|---|---|
| appunti | `Get-Clipboard`, 179 ms | Win32 diretto, 19 ms | il ripiego storpiava gli accenti da sempre (D131) |
| volume | 50+N pressioni simulate, fino a 90 s | Core Audio, letto davvero | il muto *invertiva* invece di impostare (D132) |
| notifiche | 9.300 ms | 5 ms | il costo era `Start-Sleep 9`, non la shell (D134) |
| `system_info` | query WMI, 1.543 ms | API dirette, 19 ms | descrizione che prometteva rete e batteria senza darle; numeri nella lingua dell'utente; e il registro che dice «Windows 10» su Windows 11 (D137, D138) |

Piu' `nova/powershell.py`, il posto solo da cui si chiama una shell: le
chiamate erano dieci in cinque moduli, con tre difetti di codifica diversi
(D135).

**Il prossimo, per costo misurato:**

1. ~~`list_installed_apps`~~ — **fatto**: 594 ms → 55, 229 righe identiche in
   ordine identico. Si e' portato dietro un taglio silenzioso a 250 che adesso
   si dichiara (D129, D139).
2. ~~`list_windows`~~ — **fatto**: 275 ms → 23. La funzione c'era gia', dentro
   il backend UIA che non le serviva: spostata, non riscritta (D99). E la
   strada vecchia rispondeva a un'altra domanda — una finestra per programma
   invece di ogni finestra (D140).
3. ~~`focus_window`, `close_application`, `open_application`~~ — **fatti**, e
   qui non contava la velocita': `close_application` incollava il nome dentro
   un `-like` di PowerShell, e `*` selezionava 292 processi su 292 (D141).
   `focus_window` non guardava se `SetForegroundWindow` aveva detto di no
   (D142).
4. ~~`type_text` e `press_keys`~~ — **fatti**, e il difetto non era la shell:
   nessuno guardava dove andava a finire il testo. Adesso il fuoco si verifica
   prima, si ricontrolla al momento di premere, e la risposta nomina la
   finestra (D143). La consegna dei tasti e' verificata: il testo arriva
   identico, graffe ed emoji comprese. Per un po' non lo era, e il sintomo
   sembrava un difetto del codice: era un gioco a schermo intero che si
   riprendeva il primo piano. La prova si rifiutava di diventare verde per
   assenza, ed e' l'unica ragione per cui non sono andato a riparare codice
   sano.
5. **La strada separata, che e' quella giusta.** `ui.set_text` e `ui.click`
   parlano all'applicazione invece che a tastiera e mouse: non hanno bisogno
   del fuoco, e adesso non se lo prendono nemmeno — se una scrittura porta
   avanti una finestra, il primo piano torna dov'era (D145). E' la regola di
   NOVA: non si sovrappone a cio' che fa l'utente, lavora separatamente. Da
   qui in poi ogni cosa che tocca l'interfaccia va misurata anche su questo,
   non solo su «ha funzionato».
6. ~~`create_reminder`~~ — **fatto**, e non era «da migliorare»: non aveva
   **mai** funzionato. Tre livelli di virgolette annidate, e `schtasks`
   rifiutava tutti e otto i messaggi di prova compreso «chiamare il dentista».
   Adesso e' un XML, il testo dell'utente sta in un file, e il promemoria
   scatta davvero — provato aspettando che suonasse (D146).
7. ~~La **semina del vault**~~ — **fatta**: i comandi git non passano piu' da
   una shell, e CPU, RAM e applicazioni installate le chiede ai binari invece
   di rifare le stesse query (D144).

**Rimasto in sospeso:**

- `bin\SHA256SUMS.txt` descrive tre binari che nel frattempo sono stati
  sostituiti da build locali: le impronte non corrispondono piu'. Innocuo
  finche' non si installa da uno zip, ma e' il genere di file che un giorno
  produce la diagnosi sbagliata (D58). Non lo tocco: e' una decisione di chi
  pubblica le release, non di chi compila.

**CANT-1, prima meta': il vault su disco si legge in Rust.**
`nova-nodi::deposito` sa aprire un vault, accorgersi di cosa e' cambiato
fuori da NOVA, dimenticare chi e' sparito, e sciogliere la contesa quando due
file con lo stesso nome in cartelle diverse rivendicano lo stesso slug. Il
filesystem sta dietro un tratto: la macchina a stati si prova con un disco in
memoria, e il banco gira lo stesso scenario due volte — il Python su una
cartella vera, il Rust sulla finta — per verificare che il finto sia fedele
(D103). Undici scenari, e il dodicesimo ha trovato un difetto di settimane fa:
il titolo di ripiego non passava da `capitalize()` (D104, D105).

**CANT-2, la parte che non tocca il PC.** Uno strumento e' tre cose: la
dichiarazione, la guardia, e il fare. Le prime due sono portate, e non sono la
parte facile — sono quella dove sbagliare non si vede.

La **dichiarazione** e' 26.573 caratteri di schema JSON, circa 7.600 token che
il modello rilegge a ogni richiesta e su cui sceglie quale strumento usare:
una parola diversa e' un comportamento diverso, e nessun tipo se ne accorge
(D112). Le sessanta dichiarazioni sono state estratte dal registro Python, non
ricopiate, da uno script che si verifica da solo — e che ha scoperto di
prendere a meta' quattro f-string su piu' righe, e poi che la sua stessa
verifica aveva lo stesso buco che cercava (D114). Il banco confronta lo schema
carattere per carattere, e ha trovato subito che in Python `items` sta prima
di `description` (D113).

La **guardia** era sbagliata in tre modi insieme, e tutti e tre erano D56 mai
arrivato li': percorso risolto contro protetti non risolti; barra rovescia
scritta a mano; e nessun separatore, cosi' autorizzare `C:\dati` autorizzava
`C:\dati-altrui` — dimostrato con due cartelle. Chiuso quello ne restava uno
all'opposto: sui soli nomi una giunzione aggira la protezione, misurato con
`mklink /J`. Le due domande non sono la stessa, e in una guardia valgono tutte
e due (D118).

E il **formato** con cui un file si racconta al modello — l'ordine di un
elenco, la misura, il taglio di una lettura — sembra cosmesi e non lo e':
quel testo e' cio' su cui il modello decide il passo dopo. Settantotto misure
confrontate, comprese quelle in cui l'arrotondamento a meta' decide.

**E poi i corpi.** Quelli dei file — tredici strumenti — sono portati, e il
banco li prova nel modo piu' duro che ci sia: due cartelle identiche fino ai
byte, trentuno operazioni su ognuna, e alla fine il confronto **del disco**,
non solo di cio' che i due dicono (D119). Con loro il racconto di un comando
di shell, la traduzione delle combinazioni di tasti — dove sbagliare non da'
errore, preme altri tasti nella finestra dove l'utente sta lavorando (D124) —
la lettura di una pagina web, e come si racconta un ricordo al modello.

Resta cio' che **chiede davvero alla piattaforma**: gli appunti, le
notifiche, la cattura dello schermo, l'elenco dei processi, l'avvio di
un'applicazione, le chiamate HTTP. Sono una quindicina di strumenti, e per
ognuno la domanda non e' «come si traduce» ma «quanto deve crescere
`nova-platform`»: e' una decisione di quanta superficie di sistema operativo
tenere in casa, e va presa guardando l'insieme, non uno strumento per volta.

**CANT-1 e' chiuso.** Il vault, tutto: leggere, accorgersi di cosa e'
cambiato fuori da NOVA, scrivere, archiviare, riattivare, contare, generare
l'indice, tenere il registro con la sua rotazione. Il disco vero
(`disco_vero::Cartella`) sta dietro il tratto e mantiene il contratto della
scrittura atomica; rifiuta i percorsi con `..` e non segue i collegamenti
simbolici (D111). Il guardiano dei segreti e' attaccato per davvero, con una
prova che verifica proprio l'attacco (D110).

Il guardiano e' l'ultima cosa che ho portato, apposta: e' quella che meno di
tutte va fatta a meta'. Vive in `nova-guasti`, accanto a chi maschera i
messaggi d'errore, perche' **le forme sono le stesse** e due elenchi separati
sanno sempre cose diverse (D73). Ma le soglie no: si maschera con la mano
larga e si rifiuta con la mano ferma, perche' coprire di troppo costa una
parola illeggibile in un registro mentre rifiutare di troppo costa un ricordo
che NOVA non avra' mai (D109). Il banco chiede «e' rimasto fuori qualcosa che
doveva essere rifiutato?» **e** «e' stato rifiutato qualcosa che si doveva
poter ricordare?»: venti forme e dieci ricordi legittimi, da tutte e due le
parti.

Poi la seconda meta': **scrivere**. `Deposito::salva` e' l'unica porta da cui
si entra in memoria e fa nell'ordine le stesse cose del Python — chiede al
guardiano, cerca chi c'e' gia' per slug e poi per somiglianza, rilegge dal
disco perche' l'utente potrebbe aver appena corretto quel file in Obsidian,
controlla i tipi, fonde, e scrive dove il file gia' sta. Undici scenari
confrontati file per file, contenuto compreso: zero divergenze. Il guardiano
dei segreti sta dietro un tratto che chi scrive **deve** passare, cosi' la
porta resta una sola anche in Rust; il guardiano vero e' un pezzo suo e resta
da portare, perche' e' la cosa che meno di tutte va fatta a meta' (D106).

E prima di portarla, la scrittura e' stata
**aggiustata**: `Vault.upsert` scriveva le note dell'utente con `write_text`,
cioe' apri-tronca-scrivi, e ci passa `MemoryWriter` da un thread di sfondo
dopo quasi ogni scambio. Un'interruzione a meta' lasciava una nota vuota, che
alla ricerca dopo c'e' ancora e non dice piu' niente. Adesso c'e'
`nova/scrittura.py` — temporaneo, `fsync`, rinomina — e ci passano il vault,
gli strumenti file, l'harness, la configurazione e la pianificazione (D102).
Si porta una cosa giusta, non una da correggere dall'altra parte.

**Il pezzo che non andava scritto.** Volevo portare il retrieval — BM25,
fusione RRF, taglio del corpo — e ho scritto un `nova-ricerca` intero, con
banco e sedici confronti verdi al primo colpo, 120 dei quali sulle domande
vere del vault. Poi la suite ha segnalato `test_memoria_rust.py` rosso, e il
motivo era che **quel codice esisteva gia'**: `nova-memoria`, il secondo pezzo
del cantiere, e' proprio BM25 piu' la fusione.

Avevo cercato «cosa resta da portare» guardando il Python. Non il Rust. In un
progetto la cui unica ragione e' avere **una** risposta in **un** posto, avevo
appena creato la seconda.

`nova-ricerca` e' stato cancellato. Di suo e' rimasto quello che aggiungeva
davvero: le mappe ordinate al posto di quelle a dispersione, lo spareggio per
slug dentro `rrf`, il taglio testa-e-coda del corpo, e il banco piu' largo —
tutto dentro `nova-memoria`, che adesso passa 45 confronti.

E il pezzo si e' ripagato lo stesso, perche' ha trovato un difetto **prima di
esistere**. Riscrivere l'RRF impone una domanda che in Python nessuno era
costretto a farsi: a parita' di punteggio, chi viene prima? La' la risposta
era «chi e' arrivato prima nel dizionario», e l'ordine di arrivo risaliva a
`set(tokenizza(query))` e a `postings[t]` — due insiemi di stringhe, e Python
randomizza l'hash delle stringhe a ogni processo. Misurato sul vault vero: 444
domande, undici con un ordine diverso fra due processi, tre in cui cambiava
**quale nodo veniva ricordato**. Fra queste «progetto», la domanda piu'
naturale che si possa fare a questa memoria. Vedi D94 e D95.

Il Rust aveva lo stesso difetto, tradotto fedelmente: il commento sopra `rrf`
diceva «a parita' vince chi e' stato inserito prima», e riprodurre fedelmente
un difetto resta un difetto. Il banco non se ne accorgeva perche' riordinava
per slug **in uscita**: la lotteria restava dentro la libreria e il confronto
la nascondeva. Vedi D98.

Un difetto invece resta dentro apposta: l'insieme dei caratteri «di parola»
contiene otto lettere accentate e non le altre, cosi' `Nunez` con gli accenti
diventa `n` e `ez`. L'indice non e' una funzione pura, e' un contratto con i
nodi che stanno nel vault adesso: allargarlo cambierebbe la tokenizzazione di
ogni nodo gia' scritto e farebbe sparire dei ricordi mentre si crede di aver
migliorato qualcosa. Se un giorno si allarga, da tutte e due le parti insieme
e con un reindicizzamento. Vedi D96.

**Il quattordicesimo pezzo: dove vive un nodo.** `nova-nodi::posto` porta
l'altra meta' di `Vault.upsert` che non tocca il disco: in che cartella va un
nodo, e sotto che nome.

Le cartelle numerate — `01-profilo`, `02-persone`, `03-progetti` — non sono un
vezzo. Il vault si apre in Obsidian, e nell'albero a sinistra l'utente le
vede in quell'ordine: e' l'unica parte di NOVA la cui interfaccia e' un file
manager. Quella tabella e' un contratto con l'utente prima che col codice.

Lo slug e' la regola che conta. Il nome porta il tipo davanti —
`persona-anna`, non `anna` — perche' la **persona** Anna e il **progetto**
Anna sono due nodi diversi, e senza prefisso il secondo scriverebbe sopra il
primo. Ma se anche col prefisso il posto e' occupato, si numera **solo** se il
tipo e' incompatibile: un «fatto» e una «persona» convivono benissimo nello
stesso nodo, e anzi e' proprio quello che deve succedere quando NOVA impara
qualcosa di nuovo su Anna. Con un `!=` al posto di `tipi_compatibili`, ogni
annotazione automatica creerebbe `persona-anna-2`, `persona-anna-3`, e la
memoria si sbriciolerebbe in copie che non si parlano — senza nessun errore,
perche' il file verrebbe scritto lo stesso.

Percio' il banco non chiede solo «le due meta' sono d'accordo?» ma anche, in
chiaro, «un fatto su Anna finisce dentro Anna?» (D51). Il percorso torna a
pezzi e non come stringa: le barre le mette chi conosce il sistema operativo,
e sono l'unica cosa che cambia fra Windows e il resto.

Trentacinque confronti, zero divergenze.

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


**CANT-2 e' chiuso, e non perche' sia finito tutto.** La parte traducibile e'
finita: la dichiarazione, la guardia, il formato, i corpi dei file, la shell,
i tasti, le pagine, la scelta di cosa ricordare, e i quindici strumenti che
chiedono davvero alla piattaforma - appunti, volume, notifiche, informazioni
di sistema, applicazioni installate, finestre, processi, tastiera, Cestino,
promemoria. Quello che resta in `nova/tools/` non e' traduzione rimasta
indietro: e' lavoro che appartiene a un altro cantiere, e l'ho verificato
file per file invece di supporlo.

| File | Righe | Perche' non e' CANT-2 |
|---|---|---|
| `documenti.py` | 170 | Importa `pypdf`, `docx`, `openpyxl`. Non e' una traduzione, e' la scelta di tre librerie Rust per PDF, DOCX e XLSX: e' CANT-8, l'harness dei documenti |
| `schermo.py` | 89 | Importa `mss` e `PIL`, e per la regione di una finestra chiama gia' il core. Stessa domanda: quale libreria, e quanta ne tiene `nova-platform` |
| `deleghe.py` | 164 | E' il router visto da uno strumento. Il router e' `nova-scala`, e il collegamento e' CANT-3 |
| `kb.py` | 178 | E' il vault e il motore visti da uno strumento. Entrambi sono gia' in Rust: manca il filo, e il filo e' CANT-3 |
| `web.py` | 228 | Ricerca e lettura di pagine: CANT-6, insieme al browser via CDP |
| `riparazione.py` | 176 | Sei strumenti che pilotano il banco. Il banco e' CANT-8 |
| `automazioni.py` | 214 | Esegue corpi Python **per disegno**: e' il posto dove NOVA scrive strumenti nuovi mentre gira. Non e' codice da tradurre, e' una decisione da prendere - e va presa quando si sa cosa resta di Python |
| `procedure.py`, `tempo.py` | 255 | Gia' portati sotto: `nova-registro`, `nova-pianificazione`, `attivita.py` |

**Dove ho sbagliato, qui.** Chiudendo il Cestino ho scritto a Gio che «dentro
CANT-2 restano i corpi degli altri strumenti: `automazioni.py`,
`documenti.py`, `riparazione.py`, `web.py`, `deleghe.py`. Continuo di li'».
Non era vero, e non l'avevo verificato: avevo letto i nomi dei file rimasti e
avevo dedotto il lavoro dal nome. Cinque minuti di `grep` sugli import hanno
detto che nessuno dei cinque e' CANT-2. Se avessi «continuato di li'» avrei
scelto tre librerie Rust per i documenti dentro il cantiere sbagliato, senza
la domanda che quel cantiere si porta dietro. **Un elenco di file rimasti non
e' un elenco di lavoro rimasto** (D150).


**CANT-3, primo pezzo: il taglio del contesto.** Duecento righe di Python che
decidono cosa il modello legge e cosa no, e sono il posto piu' pericoloso di
tutto il progetto per la stessa asimmetria di D148: un ordinamento sbagliato
si vede, un messaggio buttato no. Adesso stanno in `nova-contesto`, e fuori
restano di proposito il tokenizzatore vero — sta nel modello, cambia col
modello, e chiederglielo costerebbe un giro di rete per messaggio a ogni turno
solo per decidere se tagliare — e la configurazione: chi chiama sa quanto vale
il contesto, e zero vuol dire «non lo so», cioe' non si tocca niente.

Il banco confronta ventisette scenari contro l'`Agent` vero, e non solo
l'elenco finale: anche **quanti** messaggi sono stati tolti e **per quale**
ragione (D151). Settantatre verifiche, verdi al primo colpo — e un verde al
primo colpo su un pezzo cosi' non vale finche' non si e' visto diventare
rosso. Tre mutazioni fatte apposta:

| Mutazione nel Rust | Verifiche accese |
|---|---|
| stima a byte invece che a caratteri | 7, **tutte e sole quelle con testo accentato** |
| obiettivo del taglio a 0,80 invece di 0,75 | 4 |
| scarto delle risposte di tool orfane tolto | 5 |

La prima e' la piu' istruttiva: con soli scenari in inglese il banco sarebbe
rimasto verde per sempre, e il difetto — tagliare **prima** del dovuto, in
silenzio, nelle conversazioni in italiano — sarebbe uscito sul PC di qualcuno
(D152).

E due cose scoperte scrivendo le prove, portate **uguali** e non aggiustate
(D153): il taglio a numero non ha la rete che ha il taglio a token, quindi una
coda tutta di risposte di tool orfane lascia il solo messaggio di sistema e la
conversazione sparisce senza dirlo; e la funzione che accorcia il messaggio
piu' grosso oggi ne riceve sempre **uno solo**, perche' dal giro normale non
ci si arriva mai con piu' di uno. Sono da discutere con Gio, non da correggere
di nascosto: prima le due parti devono essere uguali.


**CANT-3, secondo pezzo: i due punti in cui il testo tocca il mondo.** Non e'
ancora il ciclo. Sono le chiamate che certi modelli scrivono **dentro il
discorso** invece che nel canale apposito — il punto in cui della prosa
diventa un'azione — e il ritorno: un risultato troppo lungo che va su file,
lasciando testa, coda e il percorso per rileggerlo. Stanno in
`nova-strumenti`, dove c'era gia' tutto il resto di una chiamata: la
dichiarazione, la guardia, il racconto (D99).

Il lettore delle chiamate e' scritto a mano e non ricopiato dall'espressione
regolare, perche' scriverlo a mano costringe a dire la regola ad alta voce:
la graffa dev'essere la prima cosa non bianca dopo l'apertura, e
`<tool_call> ecco: {...}` non e' una chiamata (D154). E gli argomenti si
rendono **come li rende Python**, separatori compresi, perche' quella stringa
non e' una rappresentazione: e' cio' che lo strumento riceve (D155).


**CANT-3, terzo pezzo: il messaggio numero zero.** Il prompt di sistema —
ventimila caratteri fra il prompt predefinito e le regole operative — che il
modello rilegge a ogni richiesta e che da solo vale circa 5.200 token, un
terzo del contesto prima che l'utente abbia detto qualcosa. Sta in
`nova-contesto` perche' e' il primo messaggio della finestra, e i testi sono
**estratti** dal Python da uno script che si rilegge da solo (D158).

E qui il porting ha trovato un difetto vero, non una differenza di
traduzione. Il prompt si componeva con `str.format`, che non guarda i tre
segnaposto ma **tutte** le graffe: un `system_prompt` personalizzato con
dentro un esempio JSON, o una graffa vuota, faceva saltare
`Agent.__init__` — cioe' **NOVA non partiva**, e quello che si leggeva era
`KeyError: '"a"'`. Chi scrive un prompt di sistema ci mette esempi, e gli
esempi hanno le graffe. Corretto da tutte e due le parti insieme, non solo nel
Rust (D157). E' il punto 2 del cancello — nessun traceback raggiunge
l'utente — trovato dove nessuna delle tre liste lo cercava.


**CANT-3, quarto pezzo: i blocchi che si attaccano in coda.** La memoria e le
procedure aggiungono roba **alla domanda**, non al prompt di sistema, e la
ragione e' la stessa aritmetica del fondo nel taglio dei messaggi: non cambia
cosa il modello legge, cambia quanto spesso si butta via la cache del
prefisso. La regola di composizione sta in `nova-contesto`; il testo delle
procedure sta in `nova-ricette`, accanto al suo dato (D160).

Qui il banco ha trovato un errore mio al primo giro: arrotondavo il punteggio
di somiglianza dentro la funzione che lo scrive, mentre nel Python arrotonda
`proponi` — cioe' due volte. Un carattere di differenza su un caso a 0,125.
Separate le due cose, e l'arrotondamento fatto come lo fa Python invece che
con `(x * 100).round() / 100`: quella moltiplicazione diverge su 0,125, 0,615
e 2,675, misurato (D161).


**CANT-3, quinto pezzo: un codice HTTP detto in italiano.** Non e' dentro
`agent.py`: e' la faccia che i fornitori mostrano quando qualcosa non va, e
sta in `nova-guasti` perche' la stessa risposta la ricevono tutti i cervelli a
pagamento (D162). Dentro ci sono i due rami che non si indovinano leggendo la
specifica: llama.cpp usa **400** per il contesto sfondato invece di 413, e
risponde **500** quando gli arriva un'immagine e lui e' partito senza
proiettore.

E la parte che non e' cortesia: quando la chiave e' sbagliata il fornitore la
rimanda indietro **dentro il proprio messaggio d'errore**, e da li' finirebbe
in chat e nel registro. Ottantacinque spiegazioni e diciannove corpi
confrontati, uno con una chiave vera dentro — e una verifica che si arrabbia
se quel corpo non c'e', perche' senza, il controllo sarebbe verde per assenza
(D163). Questo e' il punto 7 della lista dell'attrito, chiuso in Rust.


**E una cosa che tocca il punto 1 del cancello, trovata per caso.** Il banco
aveva due colonne, verde e rossa, e guardava solo `returncode == 0`. Ma alcune
prove escono con codice **2** per dire «questa macchina non ha come provarmi»:
`test_tastiera.py` quando non riesce a prendere il fuoco — e non scrive alla
cieca — e `test_scala_rust.py` quando il banco Rust non e' costruito su questo
sistema. Finivano fra le rosse.

Qui non si era mai visto, perche' i banchi erano costruiti e il fuoco libero.
Si e' visto la sera in cui davanti c'era una partita a schermo intero. Ma il
punto 1 e' «qualcuno che non e' l'autore l'ha installato»: su quella macchina
meta' della suite avrebbe detto «rossa» parlando **di se' e non del codice**,
e il primo che la installa avrebbe cercato difetti che non esistono. Adesso le
colonne sono tre, e le non provabili si dicono sempre (D164).

Il confine e' la parte delicata: «non provabile» si dichiara solo su una
premessa che si e' **verificata** mancante. Se la scrittura vera fallisce, da
li' non si puo' sapere se sia NOVA o l'applicazione cavia, e chiamarla non
provabile sarebbe scegliere l'ipotesi comoda — che e' il modo esatto in cui
una prova diventa verde per assenza. Resta rossa, ma senza traceback (D165).


**CANT-3, sesto pezzo: le immagini — e una domanda di privacy dentro una
funzione di comodo.** La regola «se il risultato di uno strumento nomina
un'immagine che sta su disco, quella si guarda» era stata scritta per
`screenshot`, che di immagini ne produce una. Ma `search_files` restituisce
percorsi assoluti uno per riga: «trova le foto del matrimonio» faceva
convertire in base64 le **prime due** e allegarle alla conversazione — quindi,
con un cervello a pagamento, uscivano dal PC al giro dopo, sotto una riga che
diceva «questa e' la figura prodotta dallo strumento» e non era vero.
Misurato con tre file finti.

Il confine non e' la cartella — «guarda questa foto sul desktop» e' legittimo
— ma il **numero**: una sola si consegna, molte si dichiarano e non si
allegano (D166, D167). E' una scelta di prodotto, e resta da confermare: la
regola del numero manda comunque una foto se la ricerca ne trova esattamente
una.


**CANT-3, settimo pezzo: come NOVA impara una procedura.** Il testo che si
manda al modello, la decisione se valga la pena mandarlo, la lettura di cio'
che risponde. Il prompt — millesettecento caratteri che decidono cosa NOVA
impara — non e' stato ricopiato: catturato prima, rifattorizzato, e
verificato che producesse gli stessi caratteri di prima (D169).

E una trappola che vale la pena ricordare oltre questo pezzo:
`str.splitlines()` di Python taglia anche su `\r`, `\v`, `\f` e su U+2028;
`str::lines()` di Rust taglia solo su `\n`. Qui il testo lo scrive un
**modello**, e con `lines()` una risposta perfettamente buona separata da
U+2028 diventa «risposta troppo corta»: NOVA non impara, e nel registro c'e'
scritto che il modello ha risposto male (D168). Ovunque si legga testo altrui,
questa e' la domanda da farsi.


**CANT-3, ottavo e nono pezzo: la provenienza, e i cervelli.**

`GUARDANO_LO_SCHERMO` — l'elenco degli strumenti che mostrano *cosa c'e'
aperto adesso* — sta adesso accanto al guardiano dei segreti, perche' e' la
stessa domanda posta alla **provenienza** invece che alla forma: il guardiano
riconosce una chiave dentro un testo, ma il titolo di una finestra e' una
stringa qualunque e proprio per questo passerebbe. Il banco confronta
l'elenco **intero**, non un campione (D170).

E dai cervelli: «questo indirizzo e' in casa?», che e' la frase su cui NOVA
sta in piedi ridotta a una domanda sola. Si decide sull'host, estratto a mano
come lo estrae `urlparse` — ventisei indirizzi confrontati, e una mutazione
per sottostringa che chiama «casa» `localhost.evil.example.com` (D171). Con
il ragionamento separato dalla risposta anche quando il `<think>` non e'
chiuso (D172), e «Claude Code:» che non e' piu' seguito dal nulla.


**CANT-3, decimo pezzo: cosa si dice a un cervello che vive fuori.** Un
cervello esterno riceve tre cose, e in tutte e tre sbagliare **non da' un
errore**: una riga di comando, un prompt di sistema, un payload JSON.

La riga di comando di Claude Code e' il pezzo con la storia peggiore. Su
Windows `claude` e' un file batch, `cmd.exe` rianalizza la riga, e un
argomento con degli a capo la chiude li': NOVA perdeva le proprie
quarantanove capacita' **solo nelle sessioni nuove**, che e' l'unico caso in
cui il prompt di sistema viene passato (D184). E l'elenco dei trentatre'
strumenti permessi e' una stringa sola: quando era una lista, «Read», «Glob»
e «Grep» restavano appesi in fondo alla riga e non erano permessi — senza
`Read`, NOVA scattava screenshot che non poteva guardare (D183).

Dodici mutazioni, dodici rosse. Fra queste, quasi tutti i difetti veri
rimessi dentro apposta: l'elenco rispezzato, le opzioni MCP dopo il prompt,
il prompt passato anche alle sessioni riprese, un livello di autonomia
sconosciuto che da' le mani libere.

**Cosa resta di CANT-3.**

| Pezzo | Righe | Dove appartiene |
|---|---|---|
| `agent.send`, `agent._giro`, `agent._execute_call`, `agent._sali_di_gradino` | ~280 | Il ciclo vero e proprio. E' l'ultimo, e per una ragione: un ciclo che chiama strumenti Python non ha liberato niente |
| `claude_cli._esegui`, `_traccia_avvio`, le sessioni su file, `tipo_accesso` | ~150 | Avviare un processo, leggerne l'uscita, tenere il capo del filo su disco: impalcatura (CANT-7) |
| ~~`openai_compat._post`~~, `rileva_modello`, `disponibile` | ~20 | **Fatto** il giro dei tentativi, con la rete dietro un tratto e `ureq` — che era gia' in casa — dall'altra parte (D191). Restano due chiamate di servizio: chiedere l'elenco dei modelli e chiedere se il server e' su |
| `cli_generic._esegui`, `_trova` | ~35 | Un processo e una ricerca nel PATH: sistema |


**CANT-3: il conto di cosa resta, guardato invece che dedotto.** Chiudendo
CANT-2 avevo annunciato cinque file da portare scegliendoli dai **nomi** nella
cartella, e nessuno dei cinque era CANT-2 (D150). Quindi stavolta non si
guarda come si chiamano le funzioni: si guarda cosa **toccano**. Uno script
(`attrezzi/_conto_cant3.py`) legge l'albero sintattico di `agent.py` e dei quattro
cervelli e marca ogni funzione con cio' che il suo corpo nomina — la rete, il
disco, i processi, i fili, il registro degli strumenti, i cervelli,
l'orologio. Quello che non tocca niente si puo' portare adesso; il resto no.

| | Righe |
|---|---|
| funzioni che non toccano niente | 930 |
| di cui **gia' portate** | ~455 |
| di cui restano, e sono adattatori sottili sui cervelli | ~180 |
| il resto: costanti, `__init__`, metodi astratti | ~295 |
| funzioni che toccano il mondo | 1.039 |

Le quattro grosse che toccano tutto — `_giro` (110 righe), `_sali_di_gradino`
(68), `_execute_call` (61), `send` (39) — sono **il ciclo**, e nominano
insieme il cervello, gli strumenti e l'orologio. Non e' una traduzione
rimandata per pigrizia: un ciclo in Rust che chiama strumenti Python e
cervelli Python non ha liberato niente, ed e' scritto nel cantiere fin
dall'inizio. Va dopo, e «dopo» vuol dire dopo CANT-4 (il modello locale) e
CANT-7 (l'impalcatura).

Quindi CANT-3 resta aperto con dentro **il ciclo e il collegamento dei
cervelli**, e si passa a CANT-4.


**CANT-4, primo pezzo: le decisioni, non l'avvio.** Prima di scrivere una riga
si e' chiesto *cosa c'e' gia'* (D99), e la risposta e' che il grosso di
`runtime.py` stava in `nova-modelli` da settimane: i GGUF, i motori, i conti
sulla VRAM, gli strati. Restava la **riga di comando** di llama-server, la
**scala dei layer** e il riconoscimento dell'errore di memoria — il pezzo dove
un flag in meno e' meta' della memoria sprecata e un numero sbagliato e' un
modello che gira dieci volte piu' piano senza dire niente (D173).

E portandolo sono saltate fuori due forme di «memoria finita» che **oggi non
si riconoscono**: `VK_ERROR_OUT_OF_DEVICE_MEMORY` con i trattini bassi, e il
messaggio di ggml quando l'allocazione fallisce senza usare nessuna delle sei
parole. Sono i due casi in cui NOVA non riprova con meno layer e si arrende.
Portate uguali e **dichiarate** in una prova che si chiama
`e_due_forme_che_oggi_NON_si_riconoscono` (D174): allargare la rete e' una
decisione, e chi la prende deve sapere cosa sta decidendo.


**CANT-5 e' chiuso.** Il server MCP e' il posto da cui un altro programma —
Claude Code — entra in NOVA, e la parte che si porta e' il **protocollo**: la
busta JSON-RPC, il dispacciamento, le trentatre' dichiarazioni (estratte, non
ricopiate, con l'estrattore che si rilegge da solo), il giudizio di rischio,
la domanda che l'utente legge, gli allegati con il loro tetto, e la risposta
a chi chiede il permesso.

Il protocollo si porta «senza scelte» solo finche' non lo si guarda da vicino.
Due regole che a leggere la specifica in fretta si perdono, e sono quelle che
rompono i client:

- **una richiesta senza `id` e' una notifica, e a una notifica non si risponde
  mai** — nemmeno per dire che il metodo non esiste (D175). Il banco confronta
  anche i `None`, e si arrabbia se nessuno scenario e' senza risposta: un banco
  che prova solo le domande non prova il silenzio.
- **«non ha funzionato» e «non ci siamo capiti» sono due buste diverse**: uno
  strumento che non esiste e' `-32601`, uno che esplode e' un risultato
  riuscito con `isError` (D176). E il banco ha trovato al primo giro che una
  `tools/call` senza `params` scrive «strumento sconosciuto: **None**», non
  stringa vuota.

E una regola che non e' di protocollo ma di fiducia: **se non si e' potuto
chiedere il permesso, si nega** (D177). Se il demone non risponde nessuno puo'
autorizzare, e rispondere «consenti» vorrebbe dire che un guasto di NOVA si
trasforma in un permesso.

Quel che resta in `mcp_kb.py` non e' protocollo: sono i **corpi** dei
trentatre' strumenti, e ognuno e' una riga che chiama un pezzo di NOVA piu' il
modo in cui ne racconta la risposta. Quel racconto appartiene al cantiere del
pezzo che chiama, non a questo — file per file, come per CANT-2:

| Strumenti | Dove appartengono |
|---|---|
| `kb_search`, `kb_note` | il vault e il motore: CANT-1, gia' in Rust — manca il filo |
| `harness_*`, `fascicolo*` (11) | l'harness dei documenti: CANT-8 |
| `web_*` (12) | il browser e la ricerca: CANT-6 |
| `delega`, `modelli` | il router: `nova-scala`, collegamento in CANT-3 |
| `pianifica_*`, `avvisi_recenti`, `azione_registra`, `azioni_recenti`, `dati_dove` | `nova-pianificazione` e `nova-registro`, gia' in Rust |

Il trentaquattresimo, quello che chiede il permesso, non e' in tabella perche'
non e' rimasto: la sua decisione e' portata, e di la' resta solo la chiamata
al demone, che e' impalcatura. Restano fuori per la stessa ragione il ciclo su
stdio e `scrivi_config` (CANT-7).

Una nota sul perche' quella tabella e' scritta cosi': `test_conto_shell.py`
legge questo file e si arrabbia se una riga `| nome |` elenca come «da fare»
qualcosa che e' gia' fatto. E' un controllo che invecchia all'indietro, ed e'
proprio quello che ha trovato la riga di troppo appena l'avevo scritta.


**CANT-6, primo pezzo: quello che gira dentro la pagina.** Il browser di NOVA
si guida dal di dentro — `document.querySelector("#docs-file-menu")` invece di
venti passi nell'albero di accessibilita' — e questo vuol dire che NOVA fa
eseguire del **suo** JavaScript su una pagina dove l'utente e' gia'
autenticato. Ottomila caratteri di copioni, estratti e non ricopiati, e una
funzione sola da cui gli argomenti ci entrano: e' li' che passa il confine fra
un dato dell'utente e del codice (D178).

Si scrive come lo scrive `json.dumps` di Python — `\uXXXX` compresi, coppie
surrogate comprese — non perche' l'altra forma sia sbagliata, ma perche' un
banco che accetta due scritture diverse smette di accorgersi di tutto il
resto. Mutazione con `serde_json` al posto suo: quattro verifiche rosse, tutte
e sole quelle con accenti o emoji.

E la scelta della scheda, che e' il posto dove sbagliare non da' un errore ma
il **contenuto di un'altra pagina** (D179). Il banco ha apposta il caso che
distingue le due regole; mutazione con l'ordine invertito: chiedere la scheda
«esempio» ne restituisce un'altra.

**CANT-6, secondo pezzo: cosa, di una pagina, e' testo.** Quando Chrome non
c'e', NOVA cerca leggendo l'HTML che ha risposto il motore. E' la strada di
ripiego, e sotto ci sono due dichiarazioni che non si possono ricopiare a
mano: le **duemiladuecentotrentuno** entita' HTML che `html.unescape`
conosce, e le espressioni regolari che separano il testo dal codice della
pagina. Estratte tutte e due (D180) — dagli oggetti compilati quando sono
costanti, dall'albero sintattico quando vivono dentro una funzione.

E qui il banco ha trovato **un difetto vero**, non una differenza di porto: il
riassunto di un risultato veniva cercato con la stessa espressione del
titolo, in un gruppo facoltativo. Un risultato senza riassunto si prendeva
quello del risultato dopo, e siccome `finditer` riparte da dove ha finito, si
portava via anche quel risultato. Un elenco piu' corto e una descrizione
attaccata all'indirizzo sbagliato, senza nessun errore da nessuna parte.
Corretto in tutti e due i linguaggi (D181).

Nove mutazioni, otto rosse; la nona e' un mutante **equivalente** dichiarato
come tale. Due erano passate al primo giro: una prova aveva il carattere
giusto nel posto sbagliato, e l'altra era resa cieca proprio dal difetto che
serviva a trovare.

Ed e' nato `nova-pitone`, che tiene le abitudini di Python che il porto deve
rispettare — dove finisce una riga, cos'e' uno spazio. Due funzioni, seconda
occorrenza, quindi condivise (D182).

**Cosa resta di `browser.py`, `cerca.py` e `tools/web.py`, e dove va.**

| Pezzo | Righe | Dove appartiene |
|---|---|---|
| `browser.avvia`, `cerca.avvia` | ~52 | Avviare Chrome col profilo e la porta: processi. Stessa famiglia dell'avvio di llama-server (CANT-4) |
| `browser.chiama`, `browser._parla`, `_Sessione` | ~35 | La connessione WebSocket al DevTools Protocol: rete, e una libreria da scegliere |
| `browser.apri`, `browser.schede`, `browser._versione`, `cerca.prendi`, `cerca._chiudi` | ~68 | Le richieste HTTP al browser: rete |
| `browser.carica`, `browser._eseguibile`, `browser.profilo`, `cerca.profilo` | ~60 | Trovare Chrome ed Edge sul disco: e' `nova-platform`, un backend per sistema |
| `web.fetch_url`, `web._rete`, `web.open_in_browser` | ~45 | Scaricare una pagina e aprirne una: rete e sistema |
| `web.web_search` (l'orchestrazione), `cerca.cerca` | ~80 | Il **giro**: prima il browser, poi i raschiatori, e cosa dire se falliscono tutti e due. Si porta quando c'e' sotto qualcosa da orchestrare |

Nessuno di questi e' una decisione: sono processi, connessioni e percorsi di
sistema, cioe' impalcatura. Vanno con CANT-7, che e' il cantiere
dell'impalcatura, e non prima — portarli adesso vorrebbe dire scegliere una
libreria di rete per un ciclo che ancora non esiste.


**CANT-7, primo pezzo: la configurazione — e due elenchi di guardie che
sapevano cose diverse.** Aprendo il cantiere dell'impalcatura dalla parte che
si puo' aprire, la domanda «cosa c'e' gia'?» (D99) ha trovato un secondo
elenco di guardie scritto a mano nella configurazione del demone. Mancavano
di la' `cipher /w` — che cancella lo spazio libero, cioe' rende
irrecuperabile cio' che era **gia'** stato cancellato — e `wevtutil cl`, che
svuota i registri eventi; mancavano di qua le due forme Unix; e i due lati
confrontavano in due modi diversi, cosi' `vssadmin.exe delete shadows`
passava dal demone e veniva fermato da NOVA (D185).

Ora l'elenco e' uno, sta in Python perche' e' li' che l'utente lo puo'
cambiare, e il demone usa la stessa guardia. La prova che tiene ferma la
riparazione non e' quella sui pattern — quella dice solo che oggi coincidono
— ma quella che va a cercare **se ne esiste un secondo**, su tutti i file
Rust.


E subito dopo la domanda che quel difetto obbligava a fare: **quanti altri
elenchi sono in quello stato?** Ventuno in tutto; otto generati da un
estrattore, e degli altri tredici **nove non avevano nessuno che li
confrontasse col Python**. Coincidevano tutti e nove — per fortuna, non per
costruzione.

`test_elenchi_gemelli.py` adesso li conta tutti e pretende che ognuno sia
generato, gemellato o dichiarato senza gemello con scritto perche' (D186). Il
valore non e' il confronto di oggi: e' che un elenco nuovo non possa entrare
senza dire da che parte sta.



**CANT-7, quarto pezzo: la configurazione letta da tutte e due le parti — e
quattro modi di perderla.** Le regole di lettura erano gia' provate dalla
parte Python (D229). Scriverne il gemello in Rust e poi metterli uno di
fronte all'altro su cento file di configurazione plausibili e sgangherati ha
trovato quello che nessuna delle due meta' trovava da sola.

Il piu' grosso non e' una differenza fra i due: e' una cosa che la parte
Python faceva e nessuno aveva mai chiesto a nessuno di fare. `"safety":
"ciao"` in `config.json` — un carattere sbagliato, un incollato male — non
riportava NOVA ai predefiniti. Alzava `AttributeError` dentro `_merge`, che
sta **fuori** dal riparo di `load()`: NOVA non partiva affatto. Un file che
l'utente puo' aprire e correggere a mano, e che se sbaglia non degrada
niente, spegne tutto (D248).

Poi, uno dietro l'altro, altri tre. Un `config.json` da zero byte veniva
letto come «illeggibile» invece che come «vuoto» (D249). Una configurazione
che non si era saputa leggere veniva **riscritta** con i predefiniti al primo
avvio — cioe' cancellata, chiave API compresa, proprio mentre si diceva
all'utente che c'era un problema (D250). E `errore_caricamento` esisteva da
mesi con scritto accanto «l'interfaccia lo mostra»: non lo mostrava nessuno.
Adesso lo stampa l'avvio, insieme all'elenco delle sezioni saltate.

Il quarto l'ha trovato la disciplina delle mutazioni, non la prova. Su
diciotto difetti messi apposta nel codice, due sono passati verdi. Uno era la
prova che si accecava da sola: toglieva i campi di diagnostica da tutte e due
le parti prima di confrontarli, e fra quelli c'era proprio la regola «la
diagnostica non arriva da fuori» (D251). L'altro era una CLI svuotata che non
arriva sempre come `null` — puo' arrivare come falso, zero o stringa vuota, e
il Python lo sapeva mentre il gemello lo indovinava (D252). Le due mutazioni
sopravvissute valgono piu' delle sedici viste: sono le uniche che hanno detto
qualcosa che non si sapeva gia'.

Con questo pezzo CANT-7 e' chiuso.


**CANT-10: i fogli, e la cella che si leggeva vuota.** NOVA sapeva fare meta'
della meta': leggeva il testo delle celle di un `.xlsx`, in sola lettura, e
non sapeva scrivere niente. `nova-fogli` e' il resto — con la libreria gia'
scelta misurando invece che leggendo la documentazione (D239).

La cosa che non mi aspettavo e' che la meta' che gia' c'era fosse rotta. Un
`.xlsx` porta, per ogni cella con una formula, due cose: la formula, e il
risultato dell'ultima volta che qualcuno l'ha calcolata. Nessuna libreria
calcola niente — ne' `openpyxl` ne' `umya` — e quella cache e' **vuota in
ogni file generato da un programma** invece che da Excel. Cioe' NOVA, davanti
a un foglio scritto da un'altra macchina, leggeva celle vuote proprio dove
stanno i totali, e non aveva modo di saperlo: `data_only=True` restituisce
`None`, e `None` diventava la stringa vuota come una cella davvero vuota
(D253).

La decisione piu' delicata del cantiere invece e' la scrittura, ed e' una
sola: **quando un valore e' un numero**. Scrivere in una cella il testo che
un modello ha prodotto vuol dire deciderlo per ogni valore, e sbagliare e'
silenzioso in tutti e due i versi — `007` che diventa `7`, un IBAN di sedici
cifre che torna indietro arrotondato, contro un prezzo scritto come testo che
da' un totale che non somma. Tre danni invisibili contro uno visibile: si
sceglie quello visibile (D254).

Per contorno, portare le regole in Rust ha mostrato che in Python ce n'erano
due copie — lo strumento e il fascicolo, con due separatori, due limiti e due
idee di riga vuota (D256). E la CI ha trovato una prova di `nova-componenti`
verde qui e rossa su Windows: il codice andava su tutti e due i sistemi, la
prova era scritta per uno solo (D257).


**CANT-9, seconda parte: sei dei tredici pezzi, e quello che non si puo'
scrivere da qui.** La prima parte aveva portato le prove Python su Ubuntu e
macOS e trovato cinque difetti. Questa porta il codice: `nova-platform` aveva
tredici funzioni che fuori da Windows dicevano onestamente «qui si fa in un
altro modo», e sei di quelle ora fanno qualcosa.

Il **Cestino** e' il pezzo che conta, perche' e' la premessa N2 del progetto —
prima la reversibilita', poi il permesso — e fuori da Windows non c'era. Ora
c'e' la specifica freedesktop su Linux e `~/.Trash` su macOS, con due regole
che valgono piu' del codice: non si copia e poi si cancella (D259), e la
scheda si scrive prima del file e con `O_EXCL`, che e' il lucchetto con cui
due programmi non si sovrascrivono a vicenda (D260).

I **processi** hanno portato una cosa che Windows non ha. `kill(0, ...)` non
ferma un processo: lo ferma a tutto il gruppo di chi chiama. E' l'equivalente
Unix dell'asterisco che selezionava duecentonovantadue processi, con in piu'
che uno zero puo' arrivare da un campo vuoto. Si rifiuta per nome (D258), e
la mutazione che toglie quel controllo non e' semplicemente passata rossa:
**ha ucciso il processo che eseguiva le prove**.

Appunti, volume, notifiche e informazioni di sistema sono il resto, con la
regola che quando manca uno strumento esterno l'errore dice **quale** si
cercava (D262) — e `MemAvailable` invece di `MemFree`, che e' la differenza
fra «hai 200 MB liberi» e «ne hai otto giga» sulla stessa macchina (D261).

**Cosa resta, e perche' resta.** Tastiera, elenco delle finestre e albero di
accessibilita': tutte e tre chiedono un ambiente grafico vero, e sono le uniche
di questo cantiere che non si possono nemmeno **scrivere** onestamente da qui.
Un albero di accessibilita' buttato giu' senza una macchina su cui guardarlo
produrrebbe un elenco di controlli plausibile e falso, che e' peggio del
rifiuto onesto che c'e' adesso. Piu' l'avvio automatico, che fuori da Windows
vuol dire un installatore che non esiste: `install.ps1` e' PowerShell da cima
a fondo.


**CANT-11: NOVA che usa, e il cancello che decide chi entra.** `nova-mcp`
diceva di se': «il protocollo con cui NOVA si apre a un altro programma». Era
vero ed era meta'. Adesso c'e' l'altra: ogni volta che una cosa esiste gia'
come server MCP — pilotare Excel, un gestionale, un servizio interno — la
scelta era fra riscriverla e rinunciarci.

Il codice del tubo e' un terzo del crate. Il resto e' il cancello, e la
ragione sta in una frase: **un server MCP descrive i propri strumenti con
parole sue, e quelle parole finiscono nel prompt di NOVA**. Non «in teoria» —
la descrizione di uno strumento e' testo libero che arriva da un processo che
non e' nostro, e il modello la legge come legge tutto il resto.

Cinque regole, ognuna contro un modo preciso di entrare: si dichiara e non si
scopre (D263), si cita e non si obbedisce — e cio' che comanda si **dice**
invece di toglierlo di nascosto, perche' toglierlo vuol dire che l'attacco
riesce a meta' (D264), i nomi portano davanti quello del server cosi' che un
`Bash` altrui non copra il nostro (D265), niente di altrui e' mai «sicuro»
(D266), e tutto quel che entra ha una misura (D267).

E il banco contro un server MCP vero — non un finto — ha trovato la cosa che
nessuna prova scritta a mano aveva trovato: chiamare uno strumento che non
esiste tornava «riuscito». Nel protocollo l'errore dello strumento arriva
come una risposta riuscita con `isError` acceso, e guardare solo il livello
del protocollo vuol dire leggere «non conosco questo strumento» come un
risultato valido (D268).


**CANT-8, primo pezzo: il taglio e la ricerca.** L'harness e' il posto dove
NOVA dice «lo trovi a pagina 12, terzo blocco», ed e' una promessa forte: o
quel blocco contiene quella cosa o non la contiene, e chi legge puo' andare a
controllare. Tutto il resto del cantiere discende da li' — i blocchi esistono
per **indicare**, e un modo di dividere un documento che non produce punti
indicabili non serve a niente.

`nova-harness` ha quella meta': come si divide un documento (una riga per
blocco per il codice, un paragrafo per il testo, paragrafi e righe di tabella
per il `.docx`, pezzi col riquadro per il PDF), quali file di un progetto si
guardano, e quale blocco risponde a una domanda. Non apre nessun file, e la
separazione e' voluta: il taglio e' una **decisione**, aprire un file e'
un'operazione, e tenerle distinte permette di provare la decisione senza
avere il file e di cambiare libreria senza ridiscutere il taglio.

Tre regole che sembrano dettagli e non lo sono. Il nome di un blocco e' il
numero di riga **vero** e non slitta, se no l'errore di un compilatore e il
blocco dell'harness parlano di due righe diverse (D270). A pari punteggio
vince chi viene prima nel documento, perche' una risposta che cambia ordine
fra due domande uguali fa dubitare anche della parte giusta (D271). E un
blocco non si riporta mai a meta': troncato direbbe una cosa che il documento
non dice (D272).

Il banco ha trovato una cosa piccola e precisa: `.gitignore` stava dentro
l'elenco dei file apribili, ma `Path(".gitignore").suffix` in Python e' la
stringa vuota — quindi l'albero lo saltava sempre e aprirlo rispondeva «non
so aprire un file senza estensione». Un elenco che dice una cosa e un
dispacciamento che ne fa un'altra (D269).


**CANT-8, secondo pezzo: la proposta, e due modi di scrivere nel posto
sbagliato.** Toccare un documento e' l'azione che non si annulla da se',
quindi nell'harness non esiste una funzione che modifichi e basta: esiste una
proposta, che non tocca niente, e un'applicazione, che si chiede dopo aver
visto cosa cambia. L'anteprima e l'applicazione sono **lo stesso codice** —
un'anteprima calcolata a parte prima o poi mostra qualcosa di diverso da quel
che poi succede, ed e' il modo piu' sicuro di far perdere fiducia a chi deve
premere il bottone.

Portandolo in Rust sono venuti fuori due modi, tutti e due silenziosi, di
scrivere la modifica nel posto sbagliato.

Il primo: la proposta si controlla contro i blocchi letti quando il documento
e' stato **aperto**, e si applica anche mezz'ora dopo. Se in mezzo qualcuno
ha toccato il file, le righe a quel numero vogliono dire un'altra cosa — e
NOVA ci scriveva sopra senza dire niente, con la copia `.prima` che conteneva
gia' la versione sbagliata. Adesso ogni modifica si porta dietro cosa c'era,
e se non c'e' piu' non si scrive niente: applicare meta' di quel che si e'
mostrato e' peggio che non applicare (D273).

Il secondo: `str.splitlines()` in Python taglia anche sul salto pagina. Un
`.txt` con dentro un `\x0c` diventava quattro righe dove il file ne ha tre, e
da li' in giu' ogni numero di blocco era slittato di uno. `r12` non era piu'
la riga 12 per nessuno tranne che per l'harness — e proprio D270 dice che
dev'esserlo (D274).


**CANT-8, terzo pezzo: la chirurgia sul `.docx`.** D237 diceva che per
modificare un `.docx` non serve una libreria di `.docx`, e lo diceva avendolo
misurato una volta sola, su un banco fuori dal progetto. Adesso e' codice, ed
e' `nova-docx`: si apre lo zip, si tocca **solo** `word/document.xml`, e ogni
altra parte si ricopia byte per byte. Quel che non si guarda non si puo'
rovinare.

Dentro `document.xml` non c'e' un parser XML, e non ci deve essere: c'e' uno
scanner che sa trovare dove comincia e dove finisce un elemento, contando
aperture e chiusure. Basta, perche' tutto quel che si fa e' sostituire testo
dentro elementi che esistono gia'. Un parser vero servirebbe per fare di piu',
e fare di piu' e' esattamente cio' che ha spogliato il documento nel giro col
`docx-rs` (D275).

La prova non gira su XML scritto da me: costruisce un `.docx` vero con
`python-docx`, lo fa modificare al Rust, e poi va a contare. Diciassette parti
dello zip prima, diciassette dopo, le stesse. Lo stile del titolo c'e'. Il
grassetto in mezzo al secondo paragrafo c'e'. La tabella e' intatta. Il file
non e' raddoppiato. E i paragrafi che vede il Rust sono quelli che vede
`python-docx` — che e' cio' su cui contano i blocchi `p0`, `p1`, `t0r0`
dell'harness.


**CANT-8, quarto pezzo: il verificatore, e la logica e' finita.** E' il pezzo
che trasforma l'harness da un buon posto per leggere a un posto dove si puo'
programmare: senza, NOVA propone una modifica al codice e l'utente deve
fidarsi. Va bene per tre righe, non va bene per un file che non si conosce.

Due regole, e sono tutte e due contro la versione ingenua. **Non «e' verde»,
ma «e' peggio di prima»**: se la suite era gia' rossa, «verde dopo» e'
irraggiungibile e si starebbe rifiutando una modifica buona per un guasto che
c'era gia' (D277). E **un file di una lingua che il progetto non prova non
prende nessun verde**: far girare i test per un `.md` non e' una prova, e'
un verde che non parla di quel file (D278).

Con questo, le 1.518 righe di CANT-8 che non toccano Qt sono portate. Quel
che resta e' **la finestra**, cioe' la domanda che all'apertura del cantiere
era stata rimandata apposta — e da quando il guscio e' Tauri con
l'interfaccia in HTML, non e' piu' «quali widget» ma «una quarta finestra del
guscio?». Si risponde guardando, non scrivendo.


**Excel: la domanda che restava, e un banco per rispondere.** CANT-10 aveva
scelto la libreria (`umya`) e chiuso il cantiere, ma una cosa era rimasta
aperta e scritta solo in una nota: **nessuna libreria di `.xlsx` calcola le
formule**. Leggono la cache, e la cache e' vuota in ogni file scritto da un
programma invece che da Excel. NOVA scrive `=SUM(A1:A10)`, rilegge, e vede la
formula al posto del numero — lo stesso difetto che D253 ha chiuso in
lettura, visto dal lato di chi scrive.

Le strade erano due, e adesso c'e' un banco che le misura invece di
sceglierle: `banco_fogli/`. Quattro fogli fatti apposta per essere difficili
— formule di eta' diverse, una nota, un formato percentuale, un grafico, una
formattazione condizionale, quattromila formule, e funzioni che nessun
programma conosce.

I numeri sono identici dalle due parti. La differenza e' **cosa resta del
foglio di qualcuno**: LibreOffice in silenzio riscrive l'archivio intero —
due parti perse, quattro aggiunte, undici su undici cambiate — mentre un
motore in Rust tocca **una parte su tredici** e fa il giro su quattromila
formule in due terzi di secondo contro un secondo e otto (D281). Ed e' lo
stesso comportamento che D237 aveva gia' rifiutato per il `.docx`, trovato
una seconda volta su un formato diverso.

E i due modi di non farcela non sono lo stesso: uno scrive `#NAME?` dentro il
file consegnato, l'altro rifiuta il foglio e non scrive niente (D282). Per un
programma che tocca i documenti di qualcuno, il secondo e' il verso giusto.

Resta un buco che nessuno dei due copre — FILTER, UNIQUE e SORT — e non e'
un difetto delle librerie: quelle formule, scritte da fuori, non sono un
foglio valido finche' Excel non le apre.


**Il primo filo: il demone legge la sua configurazione con le regole di
NOVA.** Trentacinque crate, e il demone ne raggiungeva diciassette: gli altri
erano decisioni portate e provate che non eseguiva nessuno. Questo e' il
primo attaccato, ed e' stato scelto perche' e' piccolo — e perche' aprendolo
si vedeva subito che non era solo un collegamento da fare.

`nova-core::config` leggeva cosi': `serde_json::from_str(...).unwrap_or_default()`.
Tre cose, tutte e tre silenziose.

Un campo solo scritto male faceva perdere **tutto** il file. E tornare ai
predefiniti li' vuol dire `write_roots` vuoto, cioe' il confinamento delle
scritture che sparisce — per un campo che non c'entrava niente (D285).
`protected_paths` si lasciava sostituire da un file salvato, mentre
`forbidden_commands` si univa gia' ai predefiniti: due guardie nello stesso
file con due comportamenti diversi. E l'unico avviso che diceva «ho ignorato
la tua configurazione» era un `tracing::warn!` emesso **prima** che il logger
esistesse — obbligatoriamente, perche' il livello del log sta nella
configurazione. Non lo leggeva nessuno, mai (D286).

Adesso le regole sono quelle di `nova-configurazione`, le stesse che la prova
gemella tiene allineate col Python. Cambiano i nomi dei campi e una cosa sola
di sostanza: un valore di un tipo che la fabbrica non ha resta fuori, perche'
chi legge dentro una struttura tipata non puo' permetterselo (D284).

**Il secondo filo: chi parla MCP, e con che versione.** `nova-mcp` e' il
porto del server MCP di NOVA — quello che Claude Code apre per leggere il
vault — ed era scollegato come gli altri. Attaccarlo ha fatto vedere che
NOVA parla MCP da **tre** posti: `nova/mcp_kb.py`, il demone, e questo porto.

Alla domanda d'apertura — «parlo la versione tale» — due dei tre rispondevano
sempre la stessa, chiunque avesse chiesto. Il terzo, il demone, no: aveva
imparato, e nel codice c'era il commento di quando qualcuno, sentendosi
rispondere una versione che non aveva nominato, aveva chiuso il collegamento
senza dire niente. Il sintomo di quel guasto e' un modello che si ritrova
senza nessuno strumento e un registro che non spiega perche'.

Adesso la regola sta in un posto solo (D287): si echeggia la versione chiesta
se la conosciamo, altrimenti la piu' recente che sappiamo parlare, com'e'
scritto nella specifica. Il demone la prende da li' — e una delle tre copie
ha smesso di esistere.

Sotto sono venuti fuori altri due buchi, tutti e due nelle prove e non nel
codice, che e' il posto peggiore dove averli:

- l'elenco delle versioni, messo a mano dentro `dichiarazioni.rs`, era
  **invisibile** alla prova che conta tutti gli elenchi del progetto: quella
  prova salta i file generati, e lo capisce da una riga nell'intestazione
  (D288). Ora l'elenco lo genera l'estrattore, e il banco gemello confronta
  le due liste invece di fidarsi della parola «generato»;
- il banco del protocollo confrontava il Rust con un `gestisci` **riscritto a
  mano** dentro il banco stesso, perche' quello vero stava dentro una classe
  che per esistere costruisce il vault, il router e il browser. Cioe'
  confrontava il Rust con un'imitazione del Python che nessuno confrontava
  col Python (D289). Il protocollo e' uscito dalla classe: ora il banco
  esegue quello vero.

Crate raggiunti dai binari pubblicati: **ventuno su trentacinque**. Il conto
di prima — «diciassette, e diciotto morti» — seguiva le dipendenze da tre
binari soli, e NOVA ne pubblica quindici: `nova-cartelle` e `nova-catalogo`
hanno un binario loro e li chiama l'installatore. La correzione, e cosa ho
sbagliato a misurare, stanno in `dove_ho_sbagliato.md`.


**Il terzo filo: cosa ha fatto il demone mentre non guardavi.** `nova-registro`
e' il porto del registro delle azioni che non si annullano — candidature
inviate, moduli compilati, click su un pulsante. Cercando dove attaccarlo e'
venuta fuori una cosa piu' grossa del collegamento: **il demone non ci
scriveva niente**.

Il demone e' il pezzo di NOVA che gira quando non c'e' nessuno a guardare, ed
e' quello che esegue `shell.exec`, cioe' un comando qualunque nella shell del
sistema. Di quel comando restava un evento sul bus — che muore col processo —
e nel suo giornale, che pero' e' un'altra cosa: il giornale serve ad
**annullare**, e la domanda «come torno indietro» non e' la domanda «cosa e'
successo». Chi chiedeva a NOVA «cosa hai fatto?» riceveva meta' della storia
senza sapere che era meta' (D290).

Adesso ogni comando lascia una riga nello **stesso file** che scrive il
Python, e ogni vuol dire ogni: distinguere i comandi «pesanti» dagli altri e'
un giudizio che sbaglia — `Remove-Item` si riconosce, `python pulisci.py` no —
e un registro che tiene solo cio' che riconosce sembra completo e non lo e'.
Ci finisce anche una `fs.write` di cui non si e' potuta conservare la copia di
prima, che e' esattamente un file di qualcuno sovrascritto per sempre.

Tre cose sono venute dietro, e nessuna delle tre era prevista:

- **il mascheramento.** Una riga di comando porta volentieri un
  `Authorization: Bearer`, e il registro e' un file che resta. Il filtro
  esisteva gia' in Rust ed era gia' confrontato col Python; gli mancava il
  pezzo che riconosce l'**etichetta** quando il valore sta in un altro campo —
  «scritto in #password» in uno e «Tramonto2026!» nell'altro. Adesso c'e', e
  il banco lo confronta su trentanove nomi di campo in una direzione sola: il
  Rust non puo' riconoscerne **meno**;
- **la potatura.** Due megabyte e uno storico: la regola stava dentro il
  vault, e il demone avrebbe dovuto tirarsi dentro il vault o riscriversela.
  E' andata in un crate suo, e la prova che la sorveglia adesso **conta le
  copie** invece di guardare un file solo (D291);
- **il fuso orario.** Il Python scrive l'ora dell'orologio di casa. Un demone
  che scrivesse UTC sullo stesso file non darebbe nessun errore: darebbe righe
  sbagliate di un'ora, e chi rilegge la propria giornata non ha modo di
  accorgersene (D292).

E c'e' una prova nuova che non c'era mai stata: `test_demone_registro.py`
**accende il demone vero**, gli fa eseguire un comando con dentro una chiave
finta, e pretende che la riga sia nel file di NOVA, che il Python la rilegga e
la racconti con le stesse parole, e che la chiave non ci sia. Finora nessuna
prova aveva mai acceso `novad`.

Crate raggiunti dai binari: **ventitre' su trentasei** — trentasei perche' i
crate sono uno di piu', `nova-potatura`, nato da questo filo. I tredici che
restano li elenca `test_crate_attaccati.py`, ognuno con scritto cosa gli
manca: e' anche la lista di cosa resta da fare.


## Il piano per il Rust

«L'obiettivo e' che sia praticamente solo Rust», ha detto Gio. Non e' una
riscrittura da cominciare dal primo file: e' un ordine di mosse, e ognuna
deve lasciare NOVA funzionante la sera in cui si fa.

Il punto d'appoggio e' questo: **il turno**. Finche' un turno e' un processo
Python, ogni pezzo portato in Rust resta una libreria che aspetta; dal
momento in cui il turno gira nel demone, ogni pezzo portato ha un posto dove
attaccarsi il giorno stesso. Per questo e' la prima mossa e non l'ultima.

1. ~~**Il turno nel demone.**~~ Fatto, nudo: chiedi, esegui, rileggi, con gli
   strumenti veri e le guardie vere (D294). Manca la memoria, mancano le
   procedure, mancano le regole operative nel prompt.
2. ~~**Il guscio chiama il demone**~~ invece di lanciare `python -m nova
   --ask`. Fatto: niente interprete da accendere per messaggio, gli
   avanzamenti che scorrono sul bus invece che su stderr, e la conversazione
   che vive nel demone invece che in un file. Il ripiego su Python resta, e
   la scelta si fa **prima** di imboccare una strada (D305, D306, D307).
3. ~~**La memoria e le procedure dentro il turno.**~~ Fatto: tre crate
   attaccati in un colpo (`nova-nodi`, `nova-memoria`, `nova-ricette`). Il
   contesto che il demone compone e' **identico** a quello del Python — la
   prova lo confronta con `KBEngine.contesto_per` sulla stessa cartella, e
   l'embedding di casa e' stato portato fin dentro la scelta della casella
   (D299, D300). E adesso il demone **impara** anche: a turno finito
   ricostruisce la procedura e la archivia, con le stesse regole del Python
   e senza far aspettare nessuno, perche' lui resta acceso (D303). Resta
   fuori l'imparare **automatico** della memoria: i fatti durevoli li estrae
   ancora solo il Python. Ma NOVA in Rust adesso ci **scrive**, quando e'
   l'utente a dirglielo: sei capacita' `kb.*`, con il guardiano dei segreti
   dentro la porta (D319).
4. **Gli strumenti che al demone mancano.** Due famiglie su otto, e
   nessuna delle due riscrivendo niente: i **file** (D308) e la **memoria**
   (D319) — i corpi stavano gia' in `nova-strumenti` e in `nova-nodi`,
   confrontati col Python da un banco. Restano: `sistema` (ventidue
   strumenti, la piu' grossa e la piu' eterogenea), `app` e finestre, `web`,
   le deleghe a un altro cervello, lo schermo, e i due documenti —
   `nova-harness`, `nova-docx`, `nova-fogli`, `nova-browser` + `nova-cdp`.

   | famiglia | in Python | nel demone |
   | --- | ---: | ---: |
   | file | 14 | 12 |
   | memoria | 8 | 6 |
   | shell | 3 | 3 |
   | sistema | 22 | 0 |
   | app / finestre | 6 | 0 |
   | web | 3 | 0 |
   | deleghe | 3 | 0 |
   | schermo | 1 | 0 |
5. **Il prompt.** Le regole operative stanno in `nova/config.py` come testo, e
   il turno del demone oggi manda solo cio' che l'utente ha in `config.json`.
   E' l'ultima cosa da spostare, perche' finche' le due strade coesistono
   devono dire la stessa cosa.
6. **Quel che resta del Python** — l'installatore, il primo avvio, il
   pannello delle impostazioni — si sposta quando il resto e' fermo, non
   prima: e' la parte che si vede, e romperla si vede subito.

La regola che tiene insieme le sei mosse: **nessuna cancella niente**. Il
Python resta finche' il Rust non fa la stessa cosa, e la prova che lo dice e'
un banco gemello o una prova che accende tutti e due. Una migrazione che
spegne una strada prima che l'altra sia provata non e' una migrazione, e' una
scommessa.

**Il turno in casa, la prima mossa.** `agente/turno` prende una frase e
restituisce una risposta, dentro il demone. Il cervello e' quello della
configurazione di NOVA (D295), gli strumenti sono le capacita' del demone —
le stesse che vede Claude Code, con le stesse guardie, in ordine stabile
perche' i fornitori tengono la cache sulla prima regione della richiesta
(D296) — e la conversazione resta aperta fra un turno e l'altro invece di
nascere e morire con un processo.

`test_demone_turno.py` lo prova per intero senza scaricare un modello: accende
`novad` vero, gli mette davanti un cervello finto che risponde come llama.cpp
— prima con una chiamata a uno strumento, poi con una frase — e guarda che
cosa e' successo davvero. Che la domanda sia arrivata al cervello, che lo
strumento l'abbia eseguito il demone, che la sua risposta sia tornata in
conversazione, che il prompt di sistema venga dal `config.json` di NOVA con i
segnaposto sostituiti, e che il secondo turno veda il primo.

**E adesso ci parla il guscio.** Fino a ieri ogni messaggio scritto nella
chat accendeva un interprete Python, gli passava la domanda sulla riga di
comando e ne leggeva la risposta su stdout. Funzionava, ed era onesto — se
il cervello andava in crisi non si portava dietro la finestra — ma costava
un processo per frase, teneva la continuita' del discorso in un file, e
«ferma» voleva dire ammazzare un albero di processi con `taskkill /T /F`.

Adesso il guscio chiede al demone, e la strada si sceglie **prima** di
imboccarla: `agente/pronto` costa quanto un ping e dice se il turno in Rust
si puo' fare con la scala che c'e' oggi in `config.json`. Se non si puo' —
il primo gradino e' una CLI da lanciare, tipo `claude` — si passa dalla
meta' Python come sempre. Provare e ripiegare sarebbe stato piu' semplice da
scrivere e sbagliato da usare: un turno morto a meta' ha gia' eseguito degli
strumenti, e rifarlo dall'altra parte li eseguirebbe due volte (D305).

Quel che si vede: gli avanzamenti arrivano dal bus invece che da stderr, e
quindi valgono anche per un turno partito dalla voce o da `nova chiedi`
(D306); «ferma» ferma il turno dentro il demone, che e' lo stesso «ferma»
di tutto il resto; e chi parla al microfono riceve la stessa postilla di
prima, portata in Rust byte per byte (D307).

Con `NOVA_CERVELLO` si forza la strada: `python` non chiede niente a
nessuno, `demone` non ripiega mai. La seconda serve a noi, ed e' il motivo
per cui esiste — senza, la meta' Rust puo' restare indietro per mesi mentre
ogni singola domanda ripiega e risponde lo stesso.


## Le mani sul disco, e chi dice dove si puo' mettere

Il turno in Rust ha gli strumenti sui file: otto capacita' nuove, e nessuna
riga di logica riscritta — i corpi stanno in `nova-strumenti::file_disco` e
un banco gemello li confronta col Python operazione per operazione. Quel che
si e' aggiunto qui e' cio' che il demone ha e la cassetta non puo' avere: le
guardie della configurazione dell'utente, il giornale (come si torna
indietro) e il Cestino (D308).

Aprendo questa famiglia sono venuti a galla due difetti che erano li' da
mesi, e valgono piu' delle otto capacita'.

**Uno**: `sposta` con `overwrite` faceva `rename` sopra la destinazione, che
spariva per sempre; dall'altra parte quel file andava nel Cestino. Il banco
era verde perche' provava lo spostamento sopra un file **solo senza**
`overwrite`, cioe' il caso in cui le due meta' si rifiutano tutte e due.

**Due**: le guardie. `config.json` — quello del pannello — e `core.json` —
quello del demone — avevano ciascuno il proprio `write_roots`. Chi scriveva
«solo in Documenti» nel pannello non era protetto dal processo che esegue.
Adesso valgono insieme: i divieti si uniscono, le cartelle autorizzate si
incastrano, e senza incastro non si scrive da nessuna parte (D309). Dentro
al demone c'era anche un `check_write` scritto a mano che confrontava i
prefissi senza separatore: autorizzare `C:\dati` autorizzava
`C:\dati-altrui`. Adesso i percorsi passano per `Guardie`, come gia'
facevano i comandi.

`test_demone_file.py` accende il demone vero e, per ogni azione, chiede due
cose: cosa ha fatto, e cosa succede se l'utente cambia idea (D310).


## Decidere senza scrivere: il primitivo che manca

Un modello a cui si chiede di classificare qualcosa **scrive** una risposta:
token dopo token, in un formato che si spera sia JSON valido. Ma per una
decisione non serve testo — serve **quale opzione, e quanto sicura**. Quella
informazione e' gia' nel modello dopo un forward pass solo: e' la probabilita'
che assegna a ciascuna risposta possibile.

Il modo di leggerla e' una domanda a scelta multipla in cui ogni risposta e'
**una lettera maiuscola**. Si leggono i logit di quelle lettere e basta. Zero
token generati, niente ciclo di decodifica, niente JSON da riparare.

L'idea viene da **Jev** di TypeSafe; la dimostrazione che si fa in casa con
pesi aperti e' di [Rizzo Flow](https://github.com/Rizzo-AI-Academy/rizzo-flow),
a sua volta ispirato a [SemIf](https://github.com/TheoLeeCJ/SemIf). Qui non c'e'
codice loro: c'e' la stessa idea, con le scelte di NOVA.

**Perche' interessa a NOVA.** `nova_decisioni` dice, nella sua prima pagina, che
le euristiche di oggi — liste di parole, soglie, espressioni regolari — «restano
cosi' non per pigrizia ma perche' l'alternativa costava un giro di modello per
ogni domandina». Quel prezzo non c'e' piu'. E i posti dove una decisione
tipizzata sostituisce un contatore o una regex sono quelli che contano: il
cancello delle approvazioni, la salita di gradino, la pertinenza di un ricordo,
e CANT-12 — «quale materiale puo' uscire dal PC» e' letteralmente una decisione
tipizzata.

**Fatto: la meta' pura** (`nova-giudizio`). Dalla domanda ai candidati, dai
logit al giudizio. Non tocca nessun modello e si prova per intero senza
scaricare un peso: 1320 giudizi confrontati con una seconda scrittura in Python
della stessa matematica, su quattro forme di domanda, sei politiche, sei forme
di logit e tre temperature — piu' il testo della domanda carattere per
carattere, perche' quel testo finisce nel prompt. Otto mutazioni deliberate,
otto rossi.

Tre scelte che divergono dal riferimento, e sono le tre che contano:
un giudizio **non ha un valore nullo** (D311), «non basta» vuol dire «chiedo» e
non «no» (D312), e un giudizio puo' **solo stringere** una guardia
deterministica, mai allentarla (D313). Quest'ultima e' la stessa regola di D309
guardata da un'altra porta: sarebbe assurdo chiudere un buco nelle guardie la
mattina e riaprirlo la sera lasciando che un modello dica «tranquillo».

**Da fare: la meta' che parla.** NOVA gia' accende llama-server, quindi non
serve ne' MLX ne' un secondo modello: `cache_prompt` da' il prefisso condiviso,
`n_probs` la distribuzione dopo un forward pass, `/tokenize` il controllo che
una lettera sia un token solo. Due cose vanno verificate **prima** di scriverci
sopra, e con una richiesta sola:

1. `n_probs` torna i primi N del vocabolario, non le righe che chiedi tu. Se una
   lettera ammessa non entra nei primi N, la sua probabilita' non arriva. Da
   provare: N molto alto, oppure una grammatica che restringe i candidati alle
   sole lettere (il README dice «date le impostazioni di campionamento», il che
   lo suggerisce ma non lo garantisce).
2. `cache_prompt` e' dichiarato **non deterministico** dal README di llama.cpp.
   Serve un modo «diretto» di riferimento e il conto pubblicato di quante
   decisioni cambiano, come fa Rizzo Flow (loro: 2 su 777, max delta 0.144).

E una terza cosa che non e' tecnica: il riferimento gira su un modello scelto
perche' e' bravo a questo, e sul loro stesso banco il modello piccolo fa 0.45
contro 0.95. NOVA gira su quello che l'utente ha in `config.json`. Quindi il
primitivo deve **dichiarare quando non sa**, e l'astensione deve finire su
«chiedo all'utente», mai su un valore di ripiego.


## Il confine che tiene il kernel

Un revisore ha proposto una cosa grossa: che il modello generi solo un grafo
dichiarativo di transizioni, che il runtime ne verifichi «matematicamente» le
invarianti, e che l'esecuzione avvenga dentro una sandbox del kernel monouso
con permessi a scadenza.

Metà di quella proposta non regge com'è scritta, e vale la pena dire quale.
Un linguaggio di piano abbastanza ristretto da essere verificabile è un
linguaggio in cui NOVA smette di essere il boss finale del PC — che è la
premessa N1; uno abbastanza espressivo da fare quel che NOVA fa oggi non è
verificabile, e la «verifica matematica» torna a essere un elenco di
controlli come quelli che ci sono già. La cosa verificabile davvero non è il
piano: è **l'impronta** — quali percorsi, quali comandi — e quella si può sia
controllare prima sia **imporre** dopo.

L'altra metà invece aveva ragione piena, ed è stata fatta. Fino a ieri le
guardie di NOVA vivevano tutte dentro il processo che decide: confronti di
percorsi, espressioni regolari sui comandi. Servono a dire di no **prima**.
Dopo non servivano a niente — quel che passava il controllo girava con tutti
i privilegi dell'utente, e un comando che la regola non aveva riconosciuto
poteva scrivere ovunque.

Adesso `shell.exec` parte dentro un recinto che tiene il kernel (D301). Su
Linux è Landlock: il processo dichiara cosa gli serve, il kernel gli toglie
tutto il resto, e la restrizione non si allenta nemmeno da dentro. Il
confine si costruisce da `write_roots`, cioè dalla riga in cui l'utente ha
già detto dove NOVA può scrivere; dove quella riga non c'è, **non si stringe
niente** e la risposta lo dichiara (D302).

E la «sandbox monouso» del revisore c'è, nella forma che serve davvero: ogni
comando riceve una cartella temporanea sua, dentro il recinto, che nasce con
lui e muore con lui. Senza, un comando confinato che deve appoggiare un file
intermedio fallisce in modi che non somigliano a un problema di permessi.

La prova non dice «la funzione torna Ok»: accende il demone vero, gli fa
eseguire un comando che prova a scrivere in due posti — uno dichiarato, uno
no — e pretende che il secondo **non ci riesca**, con il rifiuto che arriva
dal sistema. Dove il recinto non esiste (Windows, per ora, o un kernel
vecchio) la prova si dichiara saltata invece di passare per finta.

Cosa resta: **Windows**. Lì il recinto si fa con un token ristretto e un job
object, e AppContainer è il gradino sopra; è la prossima mossa di questo
filone, e fino ad allora la risposta del demone dice, a chi la legge, che su
Windows il confine è ancora solo quello della policy.


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

> **Primo passo sulla prima frase, il 6 settembre.** Non si puo' installare
> NOVA su un'altra macchina da questa, ma si puo' leggere **cosa NOVA dice a
> chi non ha gia' tutto**. I percorsi personali erano gia' a posto, e i
> percorsi ostili hanno gia' il loro banco. Non era a posto la cosa che
> nessuno poteva vedere da qui: i tre messaggi che si leggono solo senza
> Chrome, senza llama-server e senza un GGUF erano gli unici tre del progetto
> a dire cosa manca senza dire cosa fare (D193). Adesso lo dicono, e una
> prova tiene l'elenco di cio' che NOVA chiede al mondo fuori — quindici
> voci, ognuna con scritto cosa succede senza.

> **Secondo passo, il 7 settembre.** Su questa macchina si puo' guardare una
> cosa che su una macchina nuova non si vede: cosa diventa NOVA dopo mesi di
> uso. `avvio.log` era a 2,8 MB e 13.186 righe — il file che si apre proprio
> il giorno che qualcosa non parte, e che a tredicimila righe non apre piu'
> nessuno. Adesso nessun diario cresce per sempre, e la regola sta in un
> posto solo invece che in quattro con quattro tetti diversi (D194). Vale per
> la beta piu' di quanto sembri: chi installa NOVA adesso questo difetto lo
> incontra fra sei mesi, e da solo.

> **Terzo passo, l'11 settembre.** Il cancello della beta e' «cosa vede chi
> non ha gia' tutto», e qui si vedeva male: NOVA diceva «pronto» a un cervello
> che al primo messaggio rifiuta. Ora glielo si **chiede** invece di dedurlo
> dai file, e al primo avvio la chat non offre piu' tre prove destinate a
> fallire a chi non ha ancora collegato niente — dice cosa non va e come si
> sistema (D201). Vale il caso piu' comune di tutti: chi installa NOVA adesso
> ha una CLI installata e non collegata, ed e' esattamente lo stato su cui
> tutti i controlli precedenti rispondevano «tutto a posto».




L'ordine di lavoro che ne segue: prima la lista 3 dal punto 7 in giu' (gli
errori), poi la lista 2 (le macchine altrui), poi la lista 1 (la velocita').
La velocita' e' l'ultima non perche' conti poco, ma perche' e' l'unica delle
tre che si puo' misurare da soli — e quindi l'unica che non ha bisogno che la
beta sia gia' cominciata.
