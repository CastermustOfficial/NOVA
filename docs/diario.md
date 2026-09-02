# Diario di lavoro

Il registro delle modifiche c'e' gia' ed e' `git log`. Questo documento serve
a un'altra cosa: **quello che si e' scoperto mentre si faceva**.

Un messaggio di commit dice cosa e' cambiato. Non dice che quella modifica e'
nata perche' provando un comando su una macchina vera si e' visto un
carattere sbagliato, ne' che il difetto peggiore di quel giro non era quello
che si stava cercando. Sono le cose che dopo sei mesi non si ricostruiscono
piu' - e sono quelle che insegnano dove guardare la volta dopo.

La regola: **ogni lavoro finisce qui**, con tre righe se e' andato liscio e
con di piu' se ha tirato fuori qualcosa. Anche gli errori, soprattutto quelli
fatti da chi scrive.

Le decisioni che ne escono vanno in [`architettura.md`](architettura.md),
sezione 7. La strada davanti sta in [`verso_la_beta.md`](verso_la_beta.md).

---

## 30 agosto 2026 — dall'alpha verso la beta

Ramo `ottimizzazione`, non pubblicato. Diciassette lavori, dal README in
inglese alla disinstallazione.

### Il punto di partenza: misurare prima di riscrivere

La richiesta era «piu' si scende a basso livello meglio e', idealmente tutto
in Rust». Prima di spendere mesi si e' costruito un banco
(`banco_prestazioni.py`) che misura i pezzi che girano a ogni turno **senza
il modello**.

| pezzo | mediana |
|---|---|
| memoria a grafo (136 nodi) | 25,3 ms |
| ricette (28 procedure) | 2,8 ms |
| schemi dei 60 tool | 0,14 ms |
| **totale Python, per turno** | **~28 ms** |

Avvio a freddo: 53 ms `import nova`, 206 ms con tutti i tool. A ogni chiamata
partono ~11.900 token: regole 3.681, schemi dei tool 6.635, contesto KB ~590,
ricette ~955.

**Cosa si e' imparato.** Riscrivere il Python in Rust risparmia ventotto
millisecondi su turni che ne durano migliaia: come progetto di *velocita'*
non sta in piedi. Guardando pero' la lista compatibilita', meta' delle voci
**spariscono** se sul PC dell'utente non c'e' piu' Python — le quattro
versioni da provare, i diritti di amministratore, SmartScreen su ventisette
pacchetti pip, i 206 ms di import, i traceback che un `Result` non produce.
Il porting e' un progetto di **distribuzione e robustezza**. Detto cosi' vale
i mesi che costa; detto come «per andare piu' veloce», no.

**Cosa si e' scoperto e non si cercava.** Il prefisso del prompt era gia'
cache-friendly: contesto KB e ricette stanno in coda alla domanda, non nel
messaggio di sistema. Sono ~10.300 token stabili che llama.cpp riusa, contro
~1.500 rielaborati per turno. Quella scelta vale piu' di qualunque
riscrittura, e va protetta: chiunque sposti quei blocchi nel prompt di
sistema fa dieci volte il danno che qualunque ottimizzazione ripara.

### Nessuno sparisce in silenzio (`nova/guasti.py`)

NOVA gira sotto `pythonw`: non ha una console. Un errore non gestito non
finiva **da nessuna parte** — il programma si chiudeva e basta, e all'utente
non restava niente da raccontare a nessuno.

Due cose in un modulo. Tradurre: «PermissionError: [Errno 13]» non e' un
messaggio, e' il nome di una classe. E tenere la rete: `sys.excepthook` piu'
`threading.excepthook`, il traceback in `%APPDATA%\NOVA\guasti.jsonl`, e una
finestra che lo dice quando c'e'. Il traceback non sparisce, cambia posto: va
dove serve a chi ripara.

**Non si vedeva nei test perche' i test una console ce l'hanno.** E' una
categoria di guasto da tenere a mente: quello che si manifesta solo nella
configurazione in cui il programma vive davvero.

### Quale dei quattro motivi (`spiega_http`, `LimiteUso`)

Modello spento, chiave rifiutata, credito finito, quota esaurita: quattro
notizie con quattro cure diverse, che arrivavano all'utente tutte uguali —
il JSON del fornitore dentro la bolla della chat.

**Due cose trovate, nessuna delle due cercata.**

La **chiave API finiva sullo schermo**. Il fornitore la rimanda indietro
dentro il proprio messaggio d'errore (`Incorrect API key provided: sk-...`),
e quel messaggio andava dritto in chat e nel registro. Ora `senza_chiavi()`
la copre prima che qualcuno la legga.

Il **ripiego sulla quota non e' mai partito** per le API. Il README lo
prometteva e il codice c'era, ma il riconoscimento del «riprova piu' tardi»
stava solo dentro `claude_cli`: un gradino a consumo che finiva la quota
sollevava un `RuntimeError` qualunque, che il router non riconosceva. Niente
pausa, niente ripiego, mai. Una funzione documentata che non era mai stata
esercitata.

### Il verificatore (`nova/harness_prova.py`)

Si applica una modifica al codice solo se i test non peggiorano. Il confronto
e' con **prima**, non con il verde assoluto: su un progetto vero qualche
prova rossa c'e' quasi sempre, e un verificatore che pretende il verde non si
accende mai (e' D18, gia' scritta per il banco, e vale identica qui).

**Il buco vero era piu' vecchio.** `applica()` conosceva quattro estensioni —
`.md .txt .html .htm`. L'harness apriva, colorava e commentava trentadue
estensioni di codice, sapeva proporre una modifica a un `.py`, e **non sapeva
applicarla**. Lo diceva solo al momento del bottone. Il verificatore sarebbe
stato inutile senza accorgersene: si scrive il controllore e si scopre che
non c'era niente da controllare.

### L'attesa che si vede passare (`nova/attesa.py`)

«Sto pensando...» fermo per trenta secondi non e' informazione: e' un
programma che sembra rotto, e la reazione non e' aspettare, e' chiudere la
finestra — cioe' buttare via il lavoro mentre stava per finire.

Il battito ripete lo stato con i secondi che passano. Non e' una barra di
avanzamento e non finge di esserlo: NOVA non sa quanto manca, e una barra che
si inventa una percentuale mente. Da quanto sta andando lo sa, e quello e'
vero.

**Il seguito, il giorno stesso:** tutto quel lavoro non arrivava a nessuno.
In `--ask` — che e' esattamente come il guscio interroga NOVA — `on_status`
era `lambda s: None`. Lo stato veniva calcolato a ogni passo e buttato. Si e'
aperto un canale vero: Python lo scrive marcato su stderr, `cervello.rs` lo
legge **riga per riga mentre scorre** (prima leggeva tutto alla fine con
`wait_with_output`: uno stato che arriva alla fine non e' uno stato, e' un
ricordo), e stdout se lo legge un filo suo — leggere un tubo per volta
significa riempire l'altro e restare li'.

### La conferma dice cosa fa (`preview`)

Cinquantacinque tool su sessanta la frase ce l'avevano. I cinque mancanti
sono elenchi, quindi non chiedono conferma e il buco non si era mai visto —
ma da quando la stessa frase e' anche lo stato, `automazioni_elenco({})` sullo
schermo dice a chi guarda solo che sta guardando dentro un programma.

Sistemato anche il ripiego, che e' quello che vedra' il tool scritto fra sei
mesi da chi si dimentica la `preview`.

### Il registro si cerca (`registro.cerca`, `--registro`)

La promessa e' «cio' che non si annulla, si annota». La meta' scritta c'era;
quella letta no. La domanda vera non arriva il primo giorno: arriva tre
settimane dopo, ed e' sempre della stessa forma — una parola che ci si
ricorda, e un periodo vago. Senza risposta, la responsabilita' resta teorica.

**Trovato provandolo sulla macchina vera:** dove c'era `— oggi —` si leggeva
`? oggi ?`. La console di Windows e' in tabella codici 850 e il trattino
lungo non ce l'ha. Non e' un guasto, ma da fuori sembra esattamente un
guasto, ed e' la prima riga che uno legge. Provare i comandi su un terminale
vero e' un controllo che nessun test sostituisce.

### «Dove sono i miei dati?» (`nova/dati.py`, `--dati`)

E' la domanda che decide se qualcuno lascia installato un programma che gli
legge la posta e gli tiene le password. Stava sparsa in dodici moduli e nel
README a parole: «vivono in `%APPDATA%\NOVA`», che e' vero e non e' una
risposta.

Quattro colonne, e la quarta e' quella che nessuno scrive mai: **cosa succede
se lo cancelli**. E' la sola che permetta di fare pulizia senza paura.

### Cosa esce dal PC, nel pannello

Il momento in cui uno decide se un cervello va bene e' quello in cui lo
clicca, non quello in cui legge il README.

«In casa» si decide dall'**indirizzo**, non dal nome del fornitore: Ollama e
LM Studio parlano lo stesso dialetto delle API remote ma girano qui. Darli
per remoti sarebbe un allarme falso, e gli allarmi falsi insegnano a non
leggere gli allarmi.

**Provato con Chromium su sette configurazioni**, e da li' e' uscita la frase
«A Altro endpoint compatibile OpenAI arrivano...», che non si puo' leggere.

### Di interfacce ce n'era una di troppo

Si e' vista perche' il collegamento sul Desktop di questa macchina puntava
ancora a `pythonw run_nova.pyw`, che apriva la vecchia finestra PyQt. Il
collegamento era vecchio, **ma la porta era aperta davvero**: ci passava
chiunque facesse doppio clic o lanciasse `python -m nova`.

Due interfacce non sono una scelta in piu' per l'utente, sono due posti dove
le cose si scollano: i menu del cervello stavano solo in una, la fascia sulla
riservatezza pure, e alla domanda «cosa vede uno appena installato» le
risposte erano due — il che vuol dire che a quella domanda non si rispondeva.
Tolte 746 righe.

### I primi cinque minuti

Finita l'installazione uno vede un orb, e sa che NOVA «puo' fare cose sul
PC», che e' un modo di non dire niente. La prima domanda non e' «come
funziona» ma **«cosa le chiedo»**.

Tre prove che si mandano con un clic, scelte per essere vere: nessuna tocca
un file dell'utente, tutte finiscono in fretta, ognuna mostra qualcosa che
una chat non sa fare. La terza si verifica il giorno dopo, ed e' l'unica
prova che una memoria esiste davvero.

Scrivendo il test si e' corretta una frase gia' scritta: «nessuna delle tre
cambia niente» era falso, la terza un nodo in memoria lo scrive. Adesso dice
quello che e'.

**Trovato per caso, e vale piu' della sezione:** la prova sul guscio
guardava **una pagina sola**. Controllava che `impostazioni.html` non fosse
piu' recente del binario, mentre si era appena cambiata `index.html` — che
sarebbe rimasta fuori dal guscio senza che nessuno lo dicesse. Ora guarda
tutta la cartella `ui/`.

### Disinstallare

Un disinstallatore e' l'ultima cosa che un utente ricorda di un programma, e
quello che si ricorda e' se ha lasciato in giro roba.

La cosa che pesava di piu' non era nell'elenco: le **attivita' pianificate**,
l'unica che *continua a girare* dopo la disinstallazione. Ogni cinque minuti
Windows prova ad avviare un programma che non c'e' piu'. Un file dimenticato
e' disordine; un'attivita' dimenticata e' un guasto che si presenta da solo
settimane dopo, a chi non ha piu' modo di capire da dove arriva.

Il filtro guarda l'inizio del nome e non «contiene NOVA»: con «contiene» si
cancellerebbe l'attivita' di qualcun altro chiamata «Innovation backup», e il
danno non si scopre finche' non serviva. Provato con quella come esca.

**Errore da registrare.** Per provare il giro vero e' stato copiato
`install.ps1` in una cartella temporanea e lanciato da li' con
`-Disinstalla`. Lo script agisce sul **sistema**, non sulla cartella da cui
parte: ha fermato tre processi, tolto l'avvio automatico e il collegamento
sul Desktop di una macchina in uso. Rimesso tutto entro un minuto. La
lezione, che vale oltre l'episodio: **il test si scrive prima**. Fosse
esistito, il caso «provalo senza farlo davvero» ci sarebbe gia' stato, e non
sarebbe servito provarlo a mano.

### Il pannello in tre fasce

Otto schede tutte uguali in una griglia sola: per trovarne una bisognava
leggere tutti e otto i titoli. E con `auto-fit` l'ordine cambiava con la
larghezza della finestra — quindi non si imparava mai dove sta una cosa, e
sapere a memoria dove stanno i comandi e' meta' del motivo per cui un
pannello sembra ordinato.

Tre fasce con un titolo e una riga di spiegazione: **chi ragiona** (cambia
cosa NOVA sa fare), **come ti parla** (come si presenta), **com'e' messa**
(non si tocca quasi mai, si guarda quando qualcosa non va). Tre domande
invece di otto voci.

L'altra meta' del disordine erano le altezze. Quattro schede sono sezioni e
non caselle — il cervello porta dentro un modulo intero, la voce ha sei campi
e due prove, la memoria tre interruttori, l'autonomia un selettore a tre —
e in colonna accanto a una scheda da tre righe lasciavano un buco alto mezzo
schermo. Vanno a riga intera, dopo le corte.

**Guardarlo e' servito piu' che pensarlo.** Ogni passaggio e' stato reso in
Chromium e guardato: il primo assetto lasciava Autonomia sola in una colonna
con un buco accanto, il secondo metteva Voce fra due schede corte con un
vuoto di ottocento pixel, e un terzo tentativo ha **rotto l'HTML** — la
scheda Memoria e' finita disegnata dentro quella Stato, perche' tagliare un
file per indice di stringa non rispetta i confini dei tag. Rifatto
ricostruendo il corpo dai pezzi interi. Le tre volte il difetto era evidente
in un'occhiata e invisibile a qualunque prova testuale.


---

## 30 agosto 2026, sera — il cantiere Rust, primo colpo

`core/crates/nova-ricette`: il riconoscimento delle procedure, portato dal
Python al Rust. Scelto per quello che **non** ha — nessuna finestra, nessuna
chiamata a Windows, nessuna rete, nessuna dipendenza — e perche' una prova
c'era gia'. Serve a rispondere con un numero invece che con una stima alla
domanda «quanto costa portare il resto».

### Il porting non e' un'occasione per migliorare

Soglie, pesi e guardie sono quelli del Python fino all'ultima cifra, comprese
le stranezze che sembrano sbagliate e non lo sono: la rarita' senza
logaritmo, il contenimento asimmetrico, i bordi `«»` sui trigrammi, la
guardia sulla lettera iniziale. Cambiare qualcosa mentre si traduce vorrebbe
dire non sapere piu' se una differenza fra le due versioni e' un errore di
traduzione o un miglioramento voluto — ed e' il modo in cui un porting
diventa una riscrittura che nessuno sa piu' confrontare.

### Il banco di confronto

Un porting che «sembra giusto» non e' un porting. Il Python scrive archivio e
domande su stdin, il binario Rust risponde con i propri punteggi su stdout, e
si confronta cifra per cifra. Sedici domande scelte per toccare i casi che
hanno insegnato le guardie: refusi (`inobx`), sinonimi (`email`/`mail`),
parole che non c'entrano, domanda vuota, sole parole vuote, accenti e
punteggiatura, e le due trappole storiche — `ricetta`/`letta` e
`stazione`/`situazione`.

**Sedici su sedici d'accordo, al primo colpo utile.** L'unico scostamento
trovato e' stato nel banco stesso, non nelle due implementazioni.

### I numeri, che erano il punto

| | |
|---|---|
| Python: nucleo del riconoscimento | 143 righe |
| Rust: la stessa cosa, piu' i commenti | 285 righe |
| Banco di confronto | 92 righe |
| Prove Rust proprie | 4 |
| Velocita' su 28 procedure vere | 1,92 ms → sotto 0,5 ms a domanda |

**Circa due righe di Rust per riga di Python**, commenti compresi — e i
commenti qui sono meta' del file, perche' spiegano perche' una soglia e'
quella. Il risparmio di tempo, un millisezzo e mezzo per turno, conferma
quello che il banco diceva gia': **non e' un progetto di velocita'.**

Una cosa che si e' vista solo scrivendo: `unicodedata.normalize("NFKD", ...)`
in Python e' una riga, in Rust vorrebbe dire portarsi dietro le tabelle
Unicode. Qui non serve — subito dopo si tiene solo `[a-z0-9]` e il resto cade
comunque — quindi bastano le lettere accentate che compaiono davvero. Ma e'
il tipo di riga che in Python non si nota e in Rust va decisa, ed e' li' che
il conto «due righe per una» viene fuori.


### Secondo colpo: il BM25, e la scoperta che era il pezzo sbagliato

`core/crates/nova-memoria`: BM25, tokenizzazione, fusione RRF e coseno. Stesso
metodo, stesso banco — trentacinque confronti, tutti d'accordo al primo colpo,
compresi i casi che mordono: accenti, domanda vuota, sole parole ferme, un
documento lunghissimo (dove conta la normalizzazione sulla lunghezza) e due
ranking a pari merito.

Manca l'embedder locale, e non per dimenticanza: assegna i secchielli con
MD5, e riprodurre gli **stessi** secchielli vorrebbe dire portarsi dietro
un'implementazione di MD5. La meta' sparsa e' quella che pesa di piu' —
l'embedder predefinito e' a hash e non sa che «guarda se ho posta» e
«controlla le mail» sono la stessa cosa.

**Poi si e' misurato, ed e' venuto fuori l'errore.**

| pezzo | costo |
|---|---|
| `bm25.cerca` | **0,004 ms** |
| `embedder.embed` (la domanda) | 0,024 ms |
| `reindicizza` | 0,35 ms |
| `cerca` per intero | **26,5 ms** |

Il BM25 costa quattro **microsecondi**. Ne avevo portato in Rust un pezzo che
non era mai stato lento, sulla base della stessa parola — «memoria a grafo,
25 ms» — che avevo misurato al mattino senza andare piu' a fondo.

I ventidue millisecondi mancanti stanno in `Vault.refresh_if_changed()`, che
`cerca` chiama per prima cosa: fa lo `stat` di tutti e centotrentasei i file
del vault, **a ogni messaggio**. Non e' calcolo, e' disco — e in Rust sarebbe
stato piu' veloce, ma sarebbe rimasto sbagliato.

La cura e' una riga di attesa: al massimo una rilettura ogni due secondi, con
un `forza` per chi ha appena scritto e vuole rileggere. **Da 25,7 ms a 3,2
ms**, e una nota corretta in Obsidian si vede lo stesso — «subito» ed «entro
due secondi» sono la stessa cosa per una persona, mentre rileggere a ogni
frase e rileggere ogni due secondi non lo sono affatto.

Il costo Python per turno misurato stamattina era ventotto millisecondi. Ne
restano **sei**, e non per il Rust: per una riga di Python.

### Una cosa che il porting ha fatto vedere

Nell'elenco delle parole ferme della memoria c'e' `e'` con l'apostrofo, e non
c'e' `è`. Sono due stringhe diverse, quindi chi scrive con l'accento si porta
in memoria un termine che non distingue niente. Non e' un errore di
traduzione — il Python fa lo stesso, e il banco infatti concorda — ma e' il
genere di cosa che si vede solo riscrivendo una riga che si e' sempre letta.


### Il prefisso, l'avvio, e una ricerca web che mentiva

**Il prefisso.** La cosa che vale di piu' in tutto il costo di un turno e non
si vede: diecimilatrecento token di regole e schemi che llama.cpp non
rielabora, perche' non cambiano. E' gia' scritto giusto — contesto e ricette
stanno in coda alla domanda — ma e' esattamente il genere di cosa che
qualcuno rimette «nel posto giusto» fra sei mesi. Non si romperebbe niente:
si diventerebbe dieci volte piu' lenti in silenzio. Adesso c'e' una prova,
che confronta due prompt costruiti a ora ferma e pretende che siano identici
carattere per carattere.

Scrivendola e' venuta fuori una prova instabile scritta da me: il primo
tentativo confrontava due prompt a un secondo di distanza, e l'ora dentro il
prompt e' al minuto — sarebbe passata cinquantaquattro volte su
cinquantacinque. Una prova che fallisce ogni tanto non e' una prova, e' una
che si impara a ignorare.

**L'avvio.** `import nova.tools` costava 163 ms, e 115 erano `requests`:
due terzi del tempo di accensione per una libreria che serve solo a chi cerca
sul web. Importata dentro la funzione invece che in cima al modulo, sono 94
ms. Non e' una cifra enorme, ma le importazioni pesanti si accumulano una
alla volta senza che nessuno decida mai di rallentare l'avvio, e ora c'e' una
prova che le tiene fuori.

**E poi la cosa vera.** Provando che la rete funzionasse ancora dopo il
cambio: `web_search` tornava «nessun risultato o motore di ricerca non
raggiungibile». Non l'avevo rotto io — i due raschiatori leggono l'HTML di
DuckDuckGo con delle espressioni regolari, e quell'HTML e' cambiato.

Il difetto pero' non e' che siano scaduti: e' che **non sollevavano niente**.
Trovavano zero risultati, e da zero risultati NOVA concludeva «non
raggiungibile» — che era falso. Il motore rispondeva benissimo, era il
lettore a non capirlo piu'. Un tool che mente sul motivo del proprio
fallimento manda chi lo usa a cercare il guasto dalla parte sbagliata: si
controlla la rete, il proxy, il firewall, e il guasto e' in una riga di
espressione regolare.

E c'erano **due ricerche web**, come c'erano due chat: `nova/cerca.py`, che
guida un Chrome vero in una porta e un profilo suoi e funziona benissimo, e
questa, che era morta. Ora `web_search` prova prima con il browser — che non
si rompe quando cambia una classe CSS — e tiene i raschiatori come ripiego.
Quando non trova niente dice cosa ha provato e come e' andata, invece di
inventarsi una diagnosi.


### La suite provata dove non c'e' niente

Otto file di prova nuovi in una giornata, tutti scritti e verificati sulla
macchina di sviluppo. Provati **in un ambiente nudo** — Linux, senza Chrome,
senza le librerie dei documenti, senza i binari Rust — sei fallivano.

Quattro erano miei, e tutti dello stesso tipo: **fallire invece di
dichiararsi non provabili**. La convenzione di NOVA e' che l'uscita 2
significhi «qui non si puo' provare», e la CI la tratta come «saltata»
invece che come rossa. Le due prove dei banchi Rust cercavano il binario
guardando se il file c'e': su un Linux che monta la cartella di Windows il
`.exe` si vede benissimo e non si esegue, quindi morivano con «Exec format
error». Le altre due morivano su un `import` mancante.

E' la stessa forma della lista compatibilita', vista da dentro: **una prova
che gira su una macchina sola non e' una prova, e' un promemoria.** Adesso
in un ambiente nudo la suite fa 34 verdi e 6 saltate, e le due che restano
rosse lo sono perche' quella VM non puo' cancellare un file — cosa che sulla
macchina vera e sulla CI non succede.

Vale la pena notare quando e' successo: dopo aver chiuso l'intera lista
sull'attrito cognitivo e portato due pezzi in Rust, cioe' quando sembrava
tutto a posto. Bastava eseguire la stessa suite altrove.


### Il modello acceso, e i primi numeri veri della giornata

Fin qui tutte le misure erano sui pezzi che girano *senza* il modello, e
dicevano millisecondi. Acceso llama-server e mandato il prompt che NOVA manda
davvero — 12.492 token — sono venuti fuori i numeri che contano.

**Il primo non e' una velocita', e' un divario.**

| | prompt |
|---|---|
| primo messaggio di una conversazione | **25,8 s** |
| tutti quelli dopo | **1,5 s** |

Diciassette volte. E' la cache del prefisso che lavora, e adesso c'e' un
numero sotto la frase «quella scelta vale piu' di qualunque riscrittura»
scritta stamattina. Chi spostasse memoria e ricette nel messaggio di sistema
pagherebbe venticinque secondi a messaggio, in silenzio.

Spiega anche una cosa che avevo attribuito altrove: i «trenta secondi che
sembrano rotti» di ATT-4 sono **questo**. Non e' il modello lento, e' il primo
prompt di una conversazione. Il battito che ho costruito stamattina serve
esattamente li'.

**I flag, misurati uno alla volta.**

| configurazione | layer | prompt a caldo | generazione |
|---|---|---|---|
| come prima | 53 | 1504 ms | 6,0 t/s |
| `-fa on` | 53 | 1541 ms | 6,1 t/s |
| KV a 8 bit | 53 | 1281 ms | 6,5 t/s |
| KV a 8 bit, 60 layer | 60 | **691 ms** | **9,0 t/s** |
| KV a 8 bit, 62 layer | 62 | 600 ms | 7,7 t/s |
| KV a 8 bit, 64 layer | 64 | — | satura, crolla |

**Flash attention era gia' acceso.** Il valore di fabbrica in questa build e'
`auto`, e auto vuol dire on: metterlo a mano sarebbe stata una riga di
changelog per un guadagno che non esiste. Era il punto 2 della lista, e la
lista lo dava per acquisito.

**E la KV a 8 bit non serve a calcolare piu' in fretta.** A parita' di layer
vale un otto per cento. Serve a occupare meta' memoria, e quella meta'
diventa layer che tornano sulla GPU: da 53 a 60 sono **piu' cinquanta per
cento di generazione e meno cinquantaquattro di prompt**. Il guadagno non e'
nel flag, e' in cosa il flag permette.

A 62 layer la generazione **peggiora** pur avendo il prompt piu' veloce, e a
64 la VRAM satura e crolla. E' la curva che il README descriveva a parole
(«il driver ripiega in silenzio sulla memoria condivisa») vista per la prima
volta con dei numeri sopra.

**Cosa si e' cambiato, e cosa no.** NOVA usa la KV a 8 bit di suo, e la stima
dei layer sa che la cache e' piu' piccola: 53 diventano 55. Non 60. Sbagliare
per eccesso non da' un errore — da' un modello che parte e va dieci volte piu'
piano senza dirlo — e una stima che indovina sulla macchina di chi la scrive
e sbaglia altrove e' esattamente il difetto che questa lista vuole togliere. I
60 stanno nel README, misurati, per chi li vuole a mano.

**Un errore per strada, che vale la pena scrivere.** Il primo banco lanciava
llama-server con `-ngl 999`, cioe' «tutto sulla GPU». E' il caso patologico
che NOVA evita apposta: la VRAM satura, il driver ripiega sulla RAM condivisa
e tutto rallenta di dieci volte. Ci sono voluti cinque minuti a 99% di GPU per
capire che stavo misurando un regime in cui **nessun confronto fra flag dice
niente**, perche' il collo di bottiglia e' un altro e resta lo stesso qualunque
cosa si cambi.


### Terzo colpo: il registro

`core/crates/nova-registro`: cercare, riassumere e raccontare le azioni che
non si annullano. I primi due pezzi erano aritmetica; questo e' il primo che
tocca la promessa su cui NOVA sta in piedi. Se le ricette divergono si sbaglia
un ordinamento; se diverge questo, una candidatura non si ritrova piu' — e una
responsabilita' che non si puo' esercitare non e' una responsabilita'.

Ventisette confronti: parole senza accenti e senza maiuscole, piu' parole in
qualunque ordine, filtri per tipo, per esito e per data, il taglio a N, il
racconto raggruppato per giorno, il riassunto.

Due cose che il porting ha costretto a decidere, e che in Python erano
implicite:

**«Ieri» e' aritmetica civile, non una libreria.** In Python `timedelta` lo
regala; in Rust senza dipendenze bisogna scriverlo, e scrivendolo si e'
dovuto decidere cosa succede il primo del mese, il primo dell'anno e il 29
febbraio. Le prove ci sono, compreso il 1900 — che non era bisestile, perche'
la regola gregoriana non e' «divisibile per quattro».

**A pari merito serve un secondo criterio.** Il riassunto ordina i tipi dal
piu' frequente, e con due tipi a pari merito una tabella hash li tira fuori
in ordine diverso a ogni esecuzione. In Python non si era mai visto perche'
i dizionari mantengono l'ordine di inserimento; in Rust si vede subito. Ora
a parita' si ordina per nome, in tutte e due le versioni.

**E il banco ha trovato un errore mio, non del codice.** Il confronto sul
filtro per data falliva: il Python «trovava» righe di tre settimane prima. La
`leggi` finta che gli avevo messo davanti ignorava la finestra temporale e
faceva passare tutto — quindi la prova non provava quel filtro, e la
differenza sembrava un difetto del Rust. E' il rischio di ogni banco di
confronto: se il finto non si comporta come il vero, si confrontano due cose
che non sono quelle.


### Quarto colpo: i modelli, e la circolarita' ammessa in un commento

Il quarto pezzo portato e' `nova-modelli`: trovare i GGUF sul disco, leggerne
la forma, calcolare quanti strati stanno in VRAM. E' quello con la ragione
piu' forte per stare in Rust, e la ragione non e' la velocita' — sta scritta
in testa al modulo Python, da mesi, senza che nessuno l'avesse letta come un
problema:

> Modulo di sola libreria standard, di proposito: viene eseguito
> dall'installatore prima che le dipendenze del progetto siano garantite.

E' una circolarita' ammessa in una riga di commento. Per decidere quale
modello serve a questa macchina bisogna gia' avere Python installato — cioe'
il primo passo dell'installazione dipende da qualcosa che l'installazione non
ha ancora fatto. Il commento la rende accettabile chiamandola disciplina
(«solo libreria standard»); e' una toppa ben messa su un buco che resta. Un
binario che cerca, legge e calcola su una macchina appena accesa la scioglie e
basta.

Vale la pena notare che nessuna delle tre liste conteneva questa voce. Non e'
ottimizzazione, non e' compatibilita' in senso stretto, non e' attrito
cognitivo: e' un vincolo di ordine che si vede solo guardando **quando** una
cosa deve funzionare, non cosa fa.

**Le radici si passano da fuori.** La versione Python sa cos'e' un disco
fisso: chiama `GetDriveTypeW` dentro il modulo della ricerca. Quella Rust no —
riceve un elenco di cartelle e percorre quelle. La riga che parla a Windows e'
finita in `nova-platform`, accanto a «quali schermi ci sono», che e' una
domanda della stessa famiglia. Non e' pignoleria di architettura: e' cio' che
ha permesso di provare la ricerca su una cartella finta, costruita apposta con
i casi che contano — il file rinominato, lo scaricamento a meta', il
proiettore accanto al modello, la stessa copia in due posti — invece di
provarla sui sei modelli veri che ci sono su questo PC, che dimostrerebbero
solo che i due codici sono d'accordo *qui*.

**Il vocabolario si conta e non si tiene.** In un GGUF moderno
`tokenizer.ggml.tokens` e' un vettore da centocinquantamila stringhe, e serve
a nessuno dei due lettori: si legge la lunghezza e si salta. Il lettore Python
lo materializzava e poi lo sostituiva con la scritta `<array len=N>` — allocava
decine di megabyte per buttarli. In Rust si scavalca con una `seek`. E' l'unico
punto del porting dove la velocita' e' un argomento vero, ed e' un dettaglio.

**E non si crede all'intestazione sui numeri.** Un file troncato a meta'
scaricamento dichiara volentieri quattro miliardi di chiavi. Se ci si crede,
si prova a fare spazio per quattro miliardi di voci e il processo muore per
esaurimento di memoria invece di dire «questo file e' incompleto». Ci sono tre
tetti — chiavi, stringhe, vettori — e sono li' per quello. La cartella degli
scaricamenti di chi sta installando NOVA e' esattamente il posto dove i file a
meta' si trovano.

### Le tre prove che sono fallite, e perche' era colpa della prova

Il banco e' partito con tre righe rosse su trentadue, e nessuna delle tre era
un difetto del codice.

La prima: `trova(extra=[...])` in Python **aggiunge** le cartelle indicate a
quelle note, e non esiste un modo di dire «guarda solo qui». Il lato Python
percorreva anche i sei modelli veri di LM Studio mentre il lato Rust vedeva
solo la cartella finta — due elenchi che non potevano coincidere. In Rust le
radici sono un dato che si passa; in Python sono cablate dentro la funzione.
La prova le zittisce, ma l'asimmetria resta da sanare, ed e' la stessa
riflessione di D36 vista da dietro.

Le altre due erano aritmetica mia: la soglia era sotto il file che doveva
scartare, e il conteggio atteso era quello di prima di averla alzata. La
lezione e' la stessa dell'errore col filtro delle date del terzo colpo, e a
questo punto si e' ripetuta abbastanza da essere una regola: **quando il banco
dice che il Rust sbaglia, il primo sospettato e' il banco.** Su quattro pezzi
portati, tutte le divergenze trovate finora sono state mie, nessuna del
codice — il che dice qualcosa di buono sul metodo (i due lati vengono davvero
confrontati cifra per cifra) e qualcosa di scomodo su chi scrive le prove.

### E poi il difetto vero, trovato guardando due file che scaricavano

Il pezzo era finito, provato e depositato. Poi, per curiosita', si e' chiesto
al binario appena costruito cosa pensasse dei due Gemma in corso di
scaricamento sul disco. Ha risposto **«va bene»** su entrambi.

Non e' un difetto del porting: e' un difetto di NOVA, vecchio quanto il
modulo, copiato fedelmente in Rust perche' il metodo dice di copiare fedelmente.
In tre punti - il commento di `_e_gguf`, il messaggio di `verifica_file` e la
documentazione - c'era scritto che i primi quattro byte riconoscono «uno
scaricamento interrotto». E' falso, e per capirlo bastava guardare dove sta
l'intestazione: **all'inizio del file**. Un modello fermo al sessanta per
cento ce l'ha tutta, ed e' indistinguibile da uno sano fino al momento in cui
llama.cpp prova a caricarlo e muore su qualcosa che non si legge. E' di nuovo
la forma di D28 vista da lontano: il guasto non e' che manca il controllo, e'
che il controllo c'era, aveva un nome che prometteva piu' di quello che
faceva, e nessuno l'aveva messo alla prova su un file a meta'.

La cartella degli scaricamenti di chi installa NOVA e' esattamente il posto
dove i file a meta' si trovano.

**Il controllo vero.** La tabella dei tensori dice dove comincia l'ultimo, e
il file deve arrivarci. Non si calcola quanto pesa ogni tensore: vorrebbe dire
tenere aggiornata la tabella dei tipi di ggml, che cambia fra una versione e
l'altra di llama.cpp, e sbagliarla vorrebbe dire dichiarare rotto un modello
sano. Meglio un controllo che non prende il file tagliato dentro l'ultimo
tensore che uno che ogni tanto accusa un modello innocente: qui i falsi
allarmi sono il danno peggiore.

Sui quattro file veri di questa macchina:

| file | ha | gli servono | verdetto |
|---|---|---|---|
| `gemma-4-26B-A4B-it-UD-Q3_K_XL` | 12,02 GB | 12,02 GB | intero |
| `Qwen3.8-27B-Q4_K_M` | 15,66 GB | 15,66 GB | intero |
| `gemma-4-26B-A4B-it-UD-IQ3_XXS` (in corso) | 7,17 GB | 10,63 GB | mancano 3.545 MB |
| `gemma-4-26B-A4B-it-UD-IQ4_NL` (in corso) | 9,57 GB | 12,68 GB | mancano 3.189 MB |

Nessun falso allarme sui due sani, e sui due a meta' un messaggio che dice
**quanto** manca invece di limitarsi a dire di no. La correzione e' andata in
tutti e due i lati - Rust e Python - perche' il Python e' quello che gira
oggi, e un difetto conosciuto lasciato in piedi «tanto poi lo togliamo» e' un
difetto in produzione.

E c'e' un piccolo regalo in coda: per contare i tensori bisogna attraversare i
metadati, e il lettore Python li attraversava **materializzando** il
vocabolario - centocinquantamila stringhe allocate e subito buttate, per ogni
file candidato. Ora salta, come fa il Rust. Non era il motivo per cui si
guardava li'.

### Il metodo, alla quarta ripetizione

Vale la pena fermarsi su una cosa. Il difetto non e' uscito da una lista, da
una prova, o dal porting in se'. E' uscito perche' avendo in mano uno
strumento nuovo lo si e' puntato su dei dati veri che si avevano sottomano —
due file che stavano scaricando. Tre righe di comando, nessuna aspettativa
particolare.

E' la stessa forma delle altre volte: la chiave nel messaggio d'errore,
l'orb che non riceveva lo stato, il ripiego mai partito. **I difetti peggiori
si trovano guardando qualcosa di vero mentre si stava facendo altro.** La
differenza, stavolta, e' che lo strumento con cui guardare l'avevamo appena
finito di costruire.

### I due modelli, uno dopo l'altro, e una previsione che si e' avverata

Con la GPU libera e il banco in mano, la misura che mancava: gli stessi
dodicimila token di prompt, la stessa configurazione, i due modelli uno dopo
l'altro nella stessa sessione — non due giornate diverse, che sarebbero due
misure diverse.

| | strati in GPU | freddo | caldo | generazione |
|---|---|---|---|---|
| Qwen3.8 27B Q4_K_M (15,7 GB) | 53 su 65 | 26,5 s | 1.363 ms | 6,0 tok/s |
| Gemma 4 26B-A4B Q3_K_XL (12,0 GB) | 30 su 30 | 6,1 s | 145 ms | 42,4 tok/s |

Sette volte in generazione, nove sul prompt a caldo. Il README lo aveva
previsto in prosa mesi fa — «non si guadagna una frazione, si cambia
categoria» — e adesso al posto della frase c'e' una tabella.

**Ma la lettura giusta non e' «i MoE sono veloci».** La colonna che spiega
tutte le altre e' la prima: 30 su 30 contro 53 su 65. E' la stessa curva che
il banco aveva gia' trovato girando intorno al limite della scheda — da 53 a
60 strati la generazione cresce del cinquanta per cento, a 64 crolla. Qui
non si sta girando intorno al ginocchio: uno dei due modelli sta tutto dentro
e l'altro no, e i dodici strati che Qwen lascia in RAM costano piu' di tutto
il resto messo insieme. Il MoE non fa la magia: rende possibile il «ci sta».
Tremilaottocento milioni di parametri attivi invece di ventisette miliardi
sono cio' che permette a un modello da 26B di stare in dodici gigabyte senza
diventare inservibile.

E' anche la difesa di `estimate_gpu_layers`, l'aritmetica portata in Rust
poche ore prima. Il primo banco della giornata era partito con `-ngl 999`,
cioe' «mettine quanti ne entrano, che ci pensa il driver»: e' il regime in cui
nessun confronto significa niente, perche' il driver ripiega in silenzio sulla
memoria condivisa e ogni configurazione misura la stessa lentezza. Qui i 53 e
i 30 non sono scelte del driver: sono conti fatti prima, ed e' per questo che
la tabella si puo' leggere.

**Due cose che questa misura non dice.** Le quantizzazioni non sono pari —
Q3_K_XL contro Q4_K_M — e non e' una svista: la regola del catalogo e'
scegliere la piu' grande che *entra*, quindi la disparita' e' esattamente la
scelta che si stava misurando. E si e' misurata la velocita', non la qualita'
delle risposte, che con un cronometro non si misura. Il compromesso che il
README dichiara resta in piedi: su un ragionamento difficile il denso e'
ancora avanti.

Resta una domanda che non e' tecnica e non decido io: `models.json` dice
`consigliata: true` su Qwen3.8 27B, e su una scheda da 16 GB quel consiglio
ora ha contro una tabella.

### La memoria video, e la scheda che prometteva quello che non aveva

CMP-6 era l'unica voce della lista compatibilita' che non aspettava un secondo
PC, ed era anche la piu' antipatica: la stima della VRAM chiamava
`nvidia-smi`. E' il programma di NVIDIA. Su una Radeon o su una Arc non
esiste, il comando fallisce, la stima torna zero, e zero vuol dire «tutto in
CPU». Chi aveva una scheda AMD non la usava e non gli veniva detto — il
fallimento silenzioso piu' vecchio rimasto in casa, per giunta dentro il
modulo che tutto il resto del codice serve a evitare.

DXGI risponde alla stessa domanda per qualunque scheda sappia disegnare su
Windows, senza avviare un processo: microsecondi invece dei quindici secondi
di tetto che `nvidia-smi` si portava dietro. Dietro `nova-platform`, come i
dischi, con la stessa forma: un `mod imp` per Windows e uno che dice
onestamente che macOS e Linux non ci sono ancora.

**Poi la prova ha trovato una cosa che non cercavo.** L'assertiva era la piu'
banale che si possa scrivere — «il libero non puo' superare il totale» — ed e'
diventata rossa:

    AMD Radeon(TM) Graphics: liberi 15643 su 485 totali

Su questa macchina ci sono **due** schede: la GeForce e la Radeon integrata
del processore. L'integrata ha 485 MiB suoi e dichiara quindici gigabyte
disponibili, perche' il «budget» di DXGI comprende la memoria di sistema che
puo' farsi prestare. Il numero e' vero. E' RAM.

Il pericolo non e' teorico: se `scheda_principale` avesse scelto per memoria
*libera* invece che per memoria *dedicata*, l'integrata avrebbe vinto sempre,
e NOVA avrebbe caricato dodici gigabyte di modello «sulla GPU» ritrovandoseli
in RAM. Sarebbe stato il rallentamento da dieci volte con l'aria del successo
— la stessa cosa che stavamo togliendo, rimessa dentro dalla porta di
servizio, in nome della compatibilita'. Ora il libero e' tagliato al dedicato,
e la scelta si fa sulla memoria propria.

**E DXGI e' piu' ottimista di `nvidia-smi`**, che e' la direzione sbagliata in
cui sbagliare. Misurato qui: 15.341 contro 14.793 MiB, scarto +548. Non
misurano la stessa cosa — `memory.free` e' quanto e' libero adesso in
assoluto, il budget e' quanto il sistema e' disposto a darci contando che puo'
sfrattare chi non sta usando la sua — quindi non devono coincidere, ma lo
scarto deve stare dentro il margine. Ci sta: 548 contro 900 MiB di riserva
piu' il 4%. E' la prima volta che quella riserva ha un numero che la
giustifica invece di essere una cifra prudente scritta a occhio. In pratica la
stima passa da 53 a 56 layer, e la curva misurata stamattina dice che il
massimo e' a 60 e il crollo a 64: ci si avvicina all'ottimo restando dalla
parte giusta.

**E il tiro al buio, che era rimasto aperto e adesso non c'e' piu'.** Quando
la VRAM non si leggeva, `_gpu_layer_ladder` partiva da `-ngl 64`. C'era una
scala di ripiego che scende di sei layer a ogni errore di memoria — ma la
memoria condivisa **non da' errori**: accetta tutto e va dieci volte piu'
piano, quindi la scala non scattava mai. Lo stesso difetto del GGUF a meta'
con un altro vestito: un meccanismo di sicurezza che aspetta un'eccezione da
qualcosa che non ne solleva, cioe' nessun meccanismo di sicurezza.

L'avevo lasciato aperto perche' mi sembrava una decisione di prodotto. La
risposta e' stata una riga: *«e' chiaro che debba stare su tutti i PC, quindi
il calcolo e' doveroso»* — ed e' piu' netta di come l'avevo posta io. Non e'
«zero o sessantaquattro»: e' che **un numero mancante non e' un'informazione
neutra**. Tre funzioni piu' in la' diventa un `-ngl` tirato a caso, e un
programma che deve girare su qualunque macchina non puo' permettersi un
parametro deciso dal caso proprio sulla macchina che non conosce.

Quindi il calcolo si fa sempre, e senza memoria video la risposta e' zero
strati — sul processore, lento di sicuro invece che finto veloce, e detto, con
il motivo e con come rimediare a mano.

**Ma «si va in CPU» non deve diventare la normale**, o si e' scambiato un
difetto silenzioso con un difetto rumoroso e basta. Perche' resti un caso
raro servono piu' fonti, e adesso ce ne sono quattro in scala:

1. il budget di DXGI, dove risponde;
2. la sola memoria dedicata quando il budget non risponde, meno quello che il
   desktop tiene occupato di solito;
3. `/sys/class/drm/card*/device/mem_info_vram_*` su Linux, che e' amdgpu;
4. `nvidia-smi` come ultimo ripiego, dove esiste.

E qui e' entrata una distinzione che prima non c'era: **misurata** contro
**dedotta**. Viaggia insieme al numero e cambia il margine — su una deduzione
se ne tengono novecento MiB in piu', perche' non sappiamo cosa la scheda stia
gia' usando e l'errore in eccesso e' quello che non si vede. Su questa
macchina, a parita' di dodici gigabyte dichiarati: 42 strati se misurata, 39
se dedotta. Non e' prudenza generica: e' che una stima dichiarata per quello
che e' vale piu' di una misura mancante, purche' si sappia che e' una stima.

Il pezzo Linux e' provato con un albero di cartelle finto — `card0`,
`vendor`, `mem_info_vram_total` — e la prova che ci tengo di piu' e' quella
che scarta `card0-DP-1`: in `/sys/class/drm` ci sono anche i connettori, e
contarli vorrebbe dire elencare tre volte la stessa scheda e poi sceglierne
una a caso. Non serve avere una Radeon per provare a leggerla: servono i
file. E' la stessa idea per cui la ricerca dei modelli non sa cosa sia un
disco, applicata alla scheda video.

**E due lezioni di consegna, non una.** Il binario nuovo funzionava qui e non
sarebbe mai arrivato a nessuno: la CI raccoglie tre eseguibili per nome, e il
quarto non era nell'elenco. Sarebbe stata CMP-6 risolta sulla sola macchina
dove il problema non c'era — cioe' CMP-14 in miniatura, «da me funziona»
applicato a una correzione di compatibilita'.

La seconda e' peggiore, e l'ha trovata `git status` non dicendo niente. Nel
`.gitignore` c'era `bin/` — pensato per la cartella dei binari compilati alla
radice. Senza la barra davanti, quella riga vale per **qualsiasi** cartella
che si chiami `bin` a qualunque profondita', e in un progetto Rust `src/bin/`
e' dove stanno i sorgenti degli eseguibili. Il sorgente di `nova-schede`
sarebbe rimasto sul mio disco: compilava qui, spariva dal repository, e il
prossimo che avesse clonato avrebbe trovato un `Cargo.toml` che dichiara un
binario di cui non c'e' il codice.

Tre volte in una giornata, la stessa forma: un controllo che promette piu' di
quello che fa (i quattro byte del GGUF), una scala di sicurezza che aspetta
un'eccezione da chi non ne solleva (la memoria condivisa), una regola che
copre piu' di quanto intendesse (`bin/`). Nessuna delle tre si annuncia. Sono
tutte silenzi.

### Senza scheda video, e la promessa che era vera per meta'

Togliere il tiro al buio ha avuto una conseguenza che andava guardata in
faccia: da adesso, chiunque abbia una scheda che NOVA non sa leggere finisce
sul processore. Il README prometteva che li' «funziona, piu' lento», e quella
frase non l'aveva mai cronometrata nessuno. Una funzione documentata non e'
una funzione provata — sta scritto qualche paragrafo piu' su, a proposito del
ripiego sulla quota che non era mai partito — e una **promessa** documentata
lo e' ancora meno.

Stessa macchina, stesso prompt, zero layer sulla GPU:

| in CPU pura | prompt a caldo | generazione |
|---|---|---|
| Gemma 4 26B-A4B (MoE, 3,8B attivi) | 1,3 s | 7,6 tok/s |
| Qwen3.8 27B (denso) | 5,7 s | 1,8 tok/s |

La promessa e' vera per meta', ed e' la meta' che nel README non era scritta.
Sul processore si paga per i parametri che si **accendono**, non per quelli
che esistono: fra due modelli della stessa taglia ci sono quattro volte e
mezzo. Settevirgolasei token al secondo sono piu' veloci di quanto legga una
persona, e NOVA senza scheda video si usa davvero; a uno virgola otto una
risposta di ottanta token arriva in quarantacinque secondi, e non si usa.

Il numero che mi ha sorpreso di piu' e' un altro: **il MoE in CPU (7,6) va
piu' veloce del denso sulla GPU (6,0)** di questa macchina. Un processore
senza scheda video batte una GeForce da 16 GB, se il modello e' quello
giusto. Detto cosi' sembra un paradosso, e non lo e': la GeForce stava
girando con dodici strati in RAM, quindi non era una gara fra GPU e CPU — era
una gara fra un modello che ci sta e uno che non ci sta, di nuovo.

Cambia cosa si promette. Non «un modello qualsiasi, piu' lento», ma **un MoE,
e funziona**. E cambia anche il senso della domanda aperta di stamattina su
`models.json`: il consiglio predefinito non riguarda solo chi ha una 4060 Ti,
riguarda soprattutto chi non ha niente.

### L'ultima decisione della giornata

A fine giornata la domanda su `models.json` ha avuto una risposta piu' larga
di quella che avevo posto: non «quale modello consigliare», ma **chi puo'
scaricarlo**. Senza scheda video l'installatore non deve nemmeno offrire il
passo; al suo posto un suggerimento di riprovare piu' avanti, dalla
configurazione, con un modello a pochi parametri attivi.

Vale la pena scrivere perche' e' piu' forte di un avvertimento. Oggi
l'installatore la scelta la offre lo stesso e scrive accanto «servono N GB di
VRAM: andra' piano». Con i numeri di stasera quella riga e' una bugia
gentile: per un denso da 27B non e' «piu' piano», sono tredici gigabyte
scaricati per ottenere un programma che non si apre piu'. Un avvertimento piu'
grosso non ripara niente — chi installa clicca avanti, e ha ragione a farlo,
perche' gli abbiamo appena detto che la cosa e' possibile. **La forma giusta
di dire «non farlo» e' non offrirlo.**

Ed e' anche l'unico punto della giornata in cui la responsabilita' dell'utente
non c'entra. NOVA e' fatta perche' chi la usa possa chiederle qualsiasi cosa e
risponderne: il principio e' che piu' lo strumento e' potente, piu' chi lo
impugna e' responsabile. Ma questo non e' un utente che sceglie un rischio —
e' un utente che non ha modo di sapere che sta scegliendo. Non gli si sta
togliendo una liberta': gli si sta togliendo una trappola, e la strada resta
aperta dalla configurazione per chi sa cosa sta facendo.

Restano due cose da definire, e sono la ragione per cui non l'ho scritto
stasera. La soglia va detta in **parametri attivi**, non in gigabyte — il MoE
da 26B usabile in CPU ne pesa dodici, e un denso da 7B che ne pesa quattro
andrebbe piu' piano di lui — e quel campo in `models.json` oggi non c'e'. E
l'esempio proposto va misurato prima di finire in un messaggio: su questo
disco di Bonsai 27B c'e' solo il proiettore visivo, non il modello. Oggi si e'
visto due volte cosa succede alle promesse non cronometrate; sarebbe buffo
chiuderla scrivendone una nuova.

### Bonsai, e il criterio che avevo scritto male

L'esempio proposto per il suggerimento — Bonsai 27B — andava verificato prima
di finire in un messaggio, e verificandolo ha corretto una cosa che avevo
scritto io un'ora prima.

Non e' un MoE. E' un **denso da 27B derivato da Qwen3.6-27B con i pesi portati
a un bit** (Q1_0_g128, 1,125 bit per peso): 3,8 GB di file, contro i 53,8
della versione a sedici bit, e dichiara l'89,5% del punteggio dell'originale
su quindici prove.

E qui casca il criterio. Avevo scritto che la soglia si esprime in «parametri
attivi sotto i quattro miliardi». E' sbagliato — o meglio, e' il caso
particolare di una regola piu' semplice che non avevo visto perche' avevo
guardato solo modelli quantizzati allo stesso modo.

Generare un token, a una richiesta per volta, non e' un lavoro di calcolo: e'
**leggere i pesi dalla memoria**. La velocita' e' `banda / byte letti per
token`, e i byte letti sono i parametri che si accendono **moltiplicati per
quanti bit ciascuno occupa**. La quantizzazione entra nel conto quanto
l'architettura, e un denso a un bit puo' finire nella stessa categoria di un
MoE a tre bit.

Le due misure di stasera si spiegano con questa formula e con una sola banda:

| | byte letti per token | tok/s in CPU | banda implicita |
|---|---|---|---|
| Qwen3.8 27B Q4_K_M (denso) | 15,7 GB | 1,8 | ~28 GB/s |
| Gemma 4 26B-A4B Q3_K_XL (MoE) | ~3,7 GB | 7,6 | ~28 GB/s |

Che le due righe diano la stessa banda e' cio' che rende il modello di costo
credibile: non e' una spiegazione costruita a posteriori su un numero solo.
Si nota anche che il MoE non legge il quindici per cento del file, che sarebbe
la proporzione dei parametri attivi — ne legge circa un terzo, perche'
attenzione e strati condivisi si rileggono comunque a ogni token.

Applicata a Bonsai la formula prevede **circa 7,4 token al secondo in CPU su
questa macchina**: praticamente identico al Gemma. Il suo vantaggio non
sarebbe la velocita', sarebbe la qualita' a parita' di byte letti. E' una
previsione scritta apposta per essere smentita, il giorno in cui si potra'
misurarla.

**E poi ho sbagliato la conclusione, e la correzione e' arrivata in tre
minuti.** Avevo scritto che il modello era inutilizzabile: la scheda di
`prism-ml` dice di clonare la loro fork di llama.cpp per i kernel
`Q1_0_g128`, e da quella riga avevo dedotto che a monte il formato non
esistesse. Poi e' arrivato il secondo collegamento —
`lmstudio-community/Bonsai-27B-GGUF`, che dichiara una riquantizzazione fatta
con gli attrezzi standard e «validated with llama-server» — e la verifica e'
costata un comando, sul binario che sta gia' in `runtime/`:

    llama-quantize --help
      40  or  Q1_0    :  1.125 bpw quantization

C'e' gia'. Nella build che NOVA scarica, con esattamente la larghezza che la
scheda dichiara. La fork serve ai loro kernel ottimizzati — e'
un'accelerazione, non un requisito.

Quindi Bonsai e' una candidatura vera, e con tre proprieta' che la rendono
piu' interessante del solo peso: e' **multimodale** (l'`mmproj` c'e', quindi
le schermate funzionano anche col cervello di casa, che oggi e' un vantaggio
del solo Qwen), starebbe **tutta in VRAM con dodici gigabyte di margine** su
una scheda da sedici, e la formula le prevede circa 7,4 token al secondo in
CPU. Resta da misurarla, ed e' un download da 3,8 GB.

**La lezione di metodo vale piu' del modello.** La conclusione sbagliata
veniva dal README di chi ha interesse a mandarti sulla propria fork, e l'ho
scritta senza interrogare lo strumento che stava sul disco a due metri. Un
comando. E' la stessa cosa che ho ripetuto tutto il giorno agli altri — i
quattro byte del GGUF che promettevano piu' di quello che facevano, la
promessa in CPU mai cronometrata, il ripiego sulla quota mai partito — e
l'ho rifatta io su una fonte scritta invece che su un pezzo di codice. **Una
documentazione non e' una misura**, nemmeno quando e' la documentazione
ufficiale di chi il software l'ha scritto; anzi, meno che mai quando quella
documentazione ha una preferenza su dove mandarti.

Due note sulle fonti, perche' e' la seconda volta in una sera. Il primo
collegamento era la variante **MLX**, che e' formato Apple Silicon e non gira
su Windows: il fratello utile e' il GGUF. E la scheda di `prism-ml` dice
denso mentre un articolo che gira lo definisce MoE con 3B attivi: si e'
tenuta la scheda, che e' la fonte primaria, e si e' scritto che c'e' un
disaccordo invece di sceglierne una in silenzio.

### Quello che questa giornata ha insegnato

Tre cose si ripetono abbastanza da meritare di essere scritte.

**I difetti peggiori non erano quelli che si cercavano.** La chiave nel
messaggio d'errore, il ripiego mai partito, le trentadue estensioni non
scrivibili, l'orb che non riceveva mai lo stato: tutti trovati mentre si
faceva altro. Nessuno di questi sarebbe emerso da una lista di cose da fare —
sono emersi facendo.

**Provare sulla macchina vera trova cose che i test non trovano.** La
tabella codici 850, il collegamento che puntava alla finestra sbagliata, il
guscio piu' vecchio della pagina. Tutti visibili in tre secondi guardando, e
invisibili a una suite verde.

**Una funzione documentata non e' una funzione provata.** Il ripiego sulla
quota stava nel README con tanto di esempio di output. Non era mai partito.

**Un guasto che non solleva e' peggio di uno che solleva.** Il ripiego sulla
quota non partiva, i raschiatori tornavano vuoti, l'orb non riceveva mai lo
stato: tre cose diverse con la stessa forma — niente si rompe, quindi niente
lo dice, e la funzione risulta presente per anni senza esserci.

**E misurare una volta non basta: bisogna misurare il pezzo giusto.** «La
memoria costa 25 ms» era vero e inutile. Sotto c'era un BM25 da quattro
microsecondi e una scansione di cartella da ventidue millisecondi, e la
differenza fra le due decide se la cura e' un porting o una riga.

**E per il disegno, guardare non si sostituisce.** Il pannello e' stato reso
in Chromium a ogni passaggio: buchi nella griglia, schede sbilanciate e una
volta l'HTML rotto, tutti evidenti in un'occhiata e invisibili a una prova
che legge il testo del file.


## 31 agosto 2026 — quinto colpo: il motore

Il criterio di ieri sera diceva quale pezzo viene dopo: **cosa deve
funzionare prima del primo avvio**. Fatti i dischi, i modelli e le schede
video, mancava l'altra meta' — un modello non basta, ci vuole il motore che
lo fa girare, e fra un llama-server compilato con CUDA e uno per la CPU ci
sono i dieci volte di ieri.

`nova-modelli::motore` fa tre cose: trova gli eseguibili, capisce con che
cosa sono stati costruiti, e li mette in ordine. Duecento righe. E come le
altre volte, la parte interessante non e' il codice: e' quello che il codice
vecchio nascondeva.

### Tre difetti, un errore solo

Il modulo Python ne aveva tre, e a guardarli in fila sono lo stesso:
**una stringa usata al posto di una struttura**.

**«Dentro» chiesto a un prefisso.** I binari dentro `runtime/` del progetto
hanno la precedenza assoluta — sono gli unici di cui si conosce la
provenienza — e la prova era `str(exe).startswith(str(radice / "runtime"))`.
Un prefisso non e' un percorso: `runtime-vecchio`, `runtime_backup` e
`runtime2` passavano tutti. Chi tiene una copia del motore che funzionava
prima di aggiornarlo — e il `.gitignore` di questo progetto dice esplicitamente
che qualcuno lo fa — si sarebbe ritrovato la copia vecchia promossa sopra
tutto il resto, in silenzio. E' il `bin/` del `.gitignore` di ieri, in un
altro file, ventiquattro ore dopo.

**Un nome cercato come sottostringa.** L'acceleratore si indovinava con
`"cuda" in str(percorso).lower()`. Chi si chiama Cudale e tiene i motori in
casa sua si vedeva classificare come CUDA anche quello per la CPU — e la
classificazione decide l'ordine, quindi NOVA avrebbe scelto per prima la cosa
piu' lenta credendola la piu' veloce. Ora si guardano i componenti del
percorso, spezzati anche sui trattini.

**Una versione cercata dappertutto.** `re.findall` sul percorso intero
raccoglieva anche i numeri delle cartelle piu' in alto: chi tiene i motori
sotto `C:\v1.2.3\` li avrebbe ordinati per il nome del nonno. La versione sta
dove i nomi la mettono davvero, in coda alla cartella del binario.

E una quarta, che non e' un difetto ma un confine: si cercava
`llama-server.exe`, con l'estensione scritta a mano. Su Linux e su macOS quel
file non esiste, quindi non si trovava niente e NOVA concludeva che non ci
fosse un motore. Insieme all'estensione delle librerie (`.dll`, `.so`,
`.dylib`) sono due righe che tolgono un pezzo di Windows.

Tutto corretto **da tutte e due le parti**, come per il GGUF a meta': il
Python e' quello che gira oggi, e un difetto conosciuto lasciato in piedi
«tanto poi lo togliamo» e' un difetto in produzione.

### E una che ho scritto io mentre le correggevo

Scrivendo la regola nuova per i nomi ho messo `w.starts_with(radice)`, e la
mia stessa prova l'ha bocciata in trenta secondi: `cudale` comincia per
`cuda`. Avevo riscritto il difetto che ero venuto a togliere, nella funzione
che lo toglieva.

La regola giusta ha una piega che non avevo visto: **non basta nemmeno il
confronto esatto**, perche' `cuda12` e' CUDA — le versioni si attaccano al
nome. Quindi «la parola intera, oppure la parola seguita da sole cifre». E'
il tipo di dettaglio che non si trova pensando: si trova scrivendo prima la
prova con i due casi che devono dare risposte opposte.

### I nomi veri, che erano gia' li'

Le prove non usano nomi inventati: usano quelli che stanno su questa
macchina, letti prima di scrivere il codice.
`llama.cpp-win-x86_64-vulkan-avx2-2.31.2`, `-2.28.2`, `-2.8.0`. Quel `2.8.0`
e' il caso che serviva: per una stringa viene **dopo** `2.31.2`, per dei
numeri prima. Se avessi inventato tre versioni da zero, con ogni probabilita'
avrei scritto 1.0, 2.0, 3.0 e non avrei provato niente.

Cinquantaquattro controlli nel banco, tutti verdi, e le quarantasei prove
della suite pure.

### Il catalogo, e il vincolo che avevo dimenticato

Con il gruppo «cosa c'e' su questo PC» finito, restava da scrivere la
decisione di ieri sera: senza scheda video l'installatore non offre il
modello locale. Il conto sta in `nova/catalogo.py`, e l'installatore lo
interroga come gia' fa per la ricerca dei GGUF — due copie della stessa
regola sono due regole destinate a divergere, e qui divergere vorrebbe dire
far scaricare a qualcuno tredici gigabyte che non gli servono.

La soglia e il campo nuovo (`frazione_letta`) stanno in `models.json` e non
nel codice, per la stessa ragione per cui ci stanno i modelli: quando
arriveranno misure da altre macchine si cambia il file, non si fa una
release.

**Ma i vincoli erano due, e ne avevo visto uno.** Se n'e' accorta una prova
che sbagliava per il motivo giusto. Avevo scritto che con un MoE finto a due
varianti — 15 GB e 12 GB — si sarebbe dovuta offrire quella da 12; il codice
offriva quella da 15, ed **era il codice ad avere ragione sulla velocita'**:
15 × 0,31 fa 4,65 GB per token, sotto la soglia. La mia aspettativa era
sbagliata.

Inseguendo il perche' l'avessi scritta cosi', pero', e' venuto fuori che
avevo in testa una cosa vera che non avevo messo nel codice: **sulla GPU il
file sta in VRAM, sul processore sta in RAM**. Tutto. E accanto ci devono
stare Windows, il browser e NOVA stessa. Quindici gigabyte di modello su una
macchina da sedici entrano sulla carta e in pratica la mandano a paginare su
disco: un altro modo di essere lentissimi, stavolta con la ventola accesa.

Adesso i vincoli sono due — la velocita' e lo spazio — e i rifiuti sono due
frasi diverse, perche' chi ha poca RAM puo' comprarne e chi ha un modello
troppo denso no.

E' la seconda volta in due giorni che una prova rossa non indica un difetto
del codice ma un buco nella mia idea di cosa stessi provando. La prima volta
(il filtro delle date del registro) la prova era sbagliata e basta; questa
era sbagliata **e** aveva ragione. Vale la pena distinguerle: quando
un'aspettativa e' sbagliata conviene chiedersi da dove veniva, prima di
correggerla e passare oltre.

**Una nota su come l'ho provato.** `install.ps1` non si esegue mai — quello
script agisce sul sistema, e una volta l'ho imparato nel modo peggiore
disinstallando NOVA da questa macchina. La prova esercita la funzione che
decide e poi **legge** l'installatore per controllare che la usi davvero, con
un controllo che fallirebbe se qualcuno rimettesse la vecchia riga «scarico
la variante piu' leggera». La sintassi di PowerShell si verifica con il
parser, che legge e non esegue.

### Di cosa e' fatto NOVA, e i quattro elenchi che non erano d'accordo

CMP-14 diceva una cosa sola: su questa macchina l'avvio automatico punta al
prodotto della compilazione e l'installatore punta a `bin\`, due file che si
possono disallineare. Andandoci dentro erano **quattro** elenchi di cosa e'
fatto NOVA, scritti a mano in posti diversi: la CI che raccoglie i binari,
l'installatore che controlla di averli, i processi da fermare nella
disinstallazione, e `build.ps1` — che non ne copiava nessuno.

E si erano gia' disallineati, ieri, per mano mia. Aggiungendo `nova-schede`
ho aggiornato la CI e non l'installatore. Nessun errore, nessun avviso:
`Core-Presente` continuava a dire di si', perche' controllava tre nomi su
quattro. Su questa macchina il binario c'era comunque, che e' esattamente il
motivo per cui non me ne sono accorto.

La cura e' quella che il progetto usa gia' per i modelli: **non e' codice, e'
un dato**. `core/binari.json`, e lo leggono tutti. `build.ps1` pubblica in
`bin\` dopo ogni compilazione, cosi' chi sviluppa fa girare quello che gira
all'utente — che era il punto di CMP-14 prima ancora che scoprissi il resto.

**La parte che conta e' la prova, non la correzione.** Correggere quattro
elenchi e' facile; impedire che diventino cinque no. `test_binari.py`
confronta l'elenco con i bersagli veri del workspace nelle due direzioni: chi
aggiunge un eseguibile e non lo mette nell'elenco trova la suite rossa, e cosi'
chi scrive un nome che non esiste. L'ho verificata togliendo `nova-schede` e
poi aggiungendo un `nova-inventato`, per vedere che diventasse rossa davvero e
non solo che fosse verde quando tutto e' a posto — una rete che non si e' mai
vista scattare non e' una rete, e' una decorazione.

C'e' un caso che ho lasciato in piedi di proposito: l'installatore tiene un
elenco di ripiego per quando il file non c'e', perche' puo' essere scaricato
da solo e uno che si ferma per un dato mancante e' peggio. Ma il ripiego e'
proprio la cosa che invecchia di nascosto — quindi non l'ho vietato, l'ho
**verificato**: la prova pretende che dica le stesse cose del file. La prima
versione della prova lo proibiva e basta, ed era la risposta pigra.

### E provandolo per davvero, un'altra frase in inglese

Ho lanciato `build.ps1` con NOVA aperta. Dopo un minuto e mezzo:

    error: failed to remove file `...\nova-shell.exe`
    Caused by: Accesso negato. (os error 5)

piu' un traceback di PowerShell. `cargo` non puo' sovrascrivere un programma
in esecuzione e Windows glielo nega: e' giusto che fallisca. Non e' giusto
**come** lo dice — il nome di un errore invece di un messaggio, dopo novanta
secondi di attesa. E' D28, che finora avevo applicato solo al codice che parla
all'utente, mentre `build.ps1` parla a chi sviluppa, che e' comunque una
persona.

Adesso guarda prima se NOVA sta girando — usando l'elenco nuovo, che e' il
suo terzo mestiere — e lo dice in un secondo, in italiano, con cosa fare.

Una piega che stavo per sbagliare: la prima versione bloccava anche
`-Controlla`. Ma `cargo check` fa tutto il lavoro del compilatore **tranne**
scrivere i binari, quindi con NOVA aperta funziona benissimo — ed e' proprio
cio' che serve a chi vuole sapere se il codice sta in piedi senza chiudere
l'assistente che sta usando. Fermarlo avrebbe tolto l'unica cosa che si
poteva ancora fare. Provata con NOVA aperta: `-Controlla` passa in diciotto
secondi, il build si ferma subito.

### Sesto colpo: la scala, cioe' cosa esce dal PC

`nova-scala` porta in Rust la parte di `routing.py` che **decide**: l'ordine
dei gradini, le salite obbligate per categoria, i ripieghi a quota esaurita,
il tetto di spesa, i confini di parola.

Vale la pena dire perche' questo pezzo e' diverso dagli altri cinque. Le
ricette, se sbagliano, sbagliano un suggerimento; il BM25 sbaglia un
ordinamento; il registro sbaglia una candidatura che non si ritrova — grave, e
lo scrissi. Questo sbaglia **la porta di casa**. La frase su cui NOVA sta in
piedi e' «niente esce dal PC finche' qualcuno non delega davvero», e qui c'e'
il codice che la mantiene o la rompe.

Fuori sono rimaste due cose di proposito, perche' non sono decisioni: chiedere
a un cervello vero se e' a consumo (si costruisce, non si decide) e la delega
in se'. Dentro c'e' solo la matematica.

### La stella che apriva la porta a tutti

Il banco e' partito con due righe rosse. Una era mia, e la dico subito perche'
e' il solito: avevo scritto un'assertiva che pretendeva troppo, e il modo
giusto di provare «la categoria scritta male non scatta» era un compito che
non incontra nessuna parola, non una condizione con tre or.

L'altra era un difetto vero, e nel Python.

`_parola_presente` accetta una stella in coda per dire «e i suoi derivati»:
`cancell*` prende «cancellerebbe». L'implementazione toglie la stella e cerca
`\b` + il gambo. Con **`*` da solo** il gambo e' vuoto, quindi il pattern
diventa `\b` e basta — e `\b` trova un confine in qualunque testo non vuoto.
Una categoria configurata con `parole: ["*"]` scattava percio' su **ogni**
compito, mandandolo sul gradino che quella categoria impone. Fuori dal PC.

La cosa che la rende brutta e' che il codice **gia' si difende** da questa
trappola, due funzioni piu' sotto: una categoria senza parole e senza soglia
viene scartata proprio perche' scatterebbe sempre, e c'e' il commento che lo
spiega. Solo che chi scrive `parole: ["*"]` pensando «tutte le parole» supera
quel controllo — la categoria le parole ce l'ha — e finisce nello stesso posto
per un'altra strada. La difesa c'era e guardava dalla parte sbagliata.

E' anche una lezione su chi paga l'errore. Non e' l'utente che ha scritto male
la configurazione a subirne le conseguenze in modo visibile: la sua NOVA
funziona, anzi risponde meglio, perche' usa sempre il modello piu' forte. Il
prezzo e' che i suoi dati escono di casa e la bolletta cresce, e nessuna delle
due cose si annuncia. **Un difetto che migliora l'apparenza e' il piu' difficile
da trovare guardando.**

Corretto da tutte e due le parti, con la prova che passa la configurazione
trappola a tutti e dodici i compiti di prova e pretende che nessuno salga.

### Un dettaglio che non e' un dettaglio: i confini di parola in italiano

Portare `\b` senza portarsi dietro una libreria di espressioni regolari vuol
dire decidere a mano cosa e' una lettera. In Python `\b` su una stringa usa i
caratteri di parola **Unicode**, quindi `à`, `è`, `é` sono lettere. Se in Rust
avessi usato `is_ascii_alphanumeric` — che e' la scelta che viene per prima —
«perché» si sarebbe spezzato in «perch» + «é», e i confini sarebbero caduti in
mezzo alle parole. Su un testo italiano, cioe' la lingua in cui la gente
scrive i compiti a NOVA, avrebbe sbagliato di continuo.

C'e' anche un piccolo pozzo piu' in basso: cercando la prossima occorrenza
bisogna avanzare di **un carattere**, non di un byte. Con `da + 1` un taglio
puo' cadere in mezzo a una lettera accentata e far esplodere la ricerca. Ci
sono due prove apposta, e una passa una stringa di sole `à`.

Trenta controlli nel banco, quarantanove file di prova, tutti verdi.

### Il taglio che si rifaceva a ogni turno

OTT-4 e OTT-5 erano scritte come due voci: «`--cache-reuse` serve perche'
`trim_history` taglia in mezzo» e «ripensare `trim_history`». Sono la stessa
cosa vista da due lati, e misurandole e' venuto fuori che la prima era la cura
sbagliata per un difetto piu' grosso di quello che credevo.

`trim_history` tiene il messaggio di sistema e gli ultimi cinquantanove, e
butta cio' che sta in mezzo. Che sia il posto peggiore lo sapevamo: la cache
del prefisso vale finche' i token in testa sono gli stessi, e spostare la
seconda riga invalida tutto il resto. Quello che non avevo visto e' **la
frequenza**. Si tagliava fino a `tetto - 1`, cioe' si tornava esattamente sul
filo; il turno dopo aggiunge due messaggi, si supera di nuovo, si taglia di
nuovo. Contati: dal trentesimo turno in poi si taglia **a ogni turno** —
trentuno tagli su sessanta.

Il che vuol dire che oltre la mezz'ora di conversazione NOVA perdeva la cache
e non la riprendeva **piu'**, pagando il prompt da capo a ogni risposta per
il resto della sessione. Non si rompeva niente. Non lo diceva nessuno.
Diventava lenta e restava lenta, e chi la usava avrebbe pensato «si e'
appesantita», che e' il modo in cui si accetta un difetto invece di
segnalarlo.

`banco_taglio.py` lo misura invece di dedurlo — Gemma 4 26B-A4B, ottantuno
messaggi, 15.379 token di prefisso:

| | token rielaborati | prompt |
|---|---|---|
| a caldo, prefisso intatto | 10 | 130 ms |
| dopo il taglio di prima | 2.854 | 1.786 ms |
| e il turno seguente | 2.738 | **1.748 ms** |
| col fondo, dopo il taglio | 1.898 | 1.240 ms |
| e il turno seguente | 38 | **231 ms** |

Le righe che contano sono la terza e la sesta. Col taglio di prima il turno
dopo costa quanto quello del taglio: non guarisce. Col fondo, il turno dopo
torna a duecento millisecondi: guarito. Sette volte e mezzo, su un modello
veloce — su Qwen, che elabora il prompt quattro volte piu' piano, sarebbero
sette secondi a turno.

**E `--cache-reuse` non serve**: 1.745 contro 1.771 millisecondi, dentro il
rumore. La ragione e' istruttiva e vale piu' del numero. Quel flag riusa i
pezzi di cache che stanno **prima** del punto in cui il prefisso diverge; qui
la divergenza e' subito dopo il messaggio di sistema, quindi prima non c'e'
niente da riusare. La voce OTT-4 era scritta come «serve perche' tagliamo in
mezzo» — ed era vero il perche' e sbagliata la conclusione. Non si compensa
con un flag un taglio fatto nel posto sbagliato: si sposta il taglio.

La cura e' un fondo. Superato il tetto si scende a quaranta invece di fermarsi
a cinquantanove: tre tagli su sessanta turni invece di trentuno, e nei turni
in mezzo il prefisso resta valido. Non cambia cosa si butta, cambia quanto
spesso — e il prezzo, buttare di piu' in un colpo solo, si paga volentieri,
perche' la memoria vera di NOVA non e' quella finestra, e' il vault.

### E la prova che misurava un'imitazione

Scrivendo `test_taglio.py` ho voluto confrontare «prima» e «ora», e per avere
il «prima» ho passato alla funzione nuova un fondo pari al tetto, pensando che
si comportasse come la vecchia. Non si comporta: la funzione nuova riporta il
fondo dentro i limiti apposta, quindi misuravo qualcosa di gia' mezzo
corretto. Il confronto diceva sedici tagli invece di trentuno — meta' del
difetto, che sembra un numero plausibile e per questo non salta all'occhio.

Ho copiato il codice vecchio dentro la prova, testualmente. **Il paragone col
passato si fa col passato, non con una sua imitazione.**

La stessa cosa valeva per il banco: calcolava il taglio nuovo con una copia
della logica invece di chiamare `Agent.trim_history`. Una prova che misura una
parafrasi misura la parafrasi — e se domani la funzione vera cambia, il banco
continuerebbe a dire che va tutto bene.

E c'e' una terza correzione, piu' piccola, arrivata dallo stesso giro. La
prima difesa che avevo messo contro un fondo scritto male era `tetto - 2`:
sembra prudente e non lo e'. Con quella distanza si taglia a turni alterni
invece che a ogni turno, cioe' **meta' del difetto invece della sua assenza**.
Adesso il fondo non puo' superare i tre quarti del tetto, che sono una decina
di turni di respiro. Una difesa che lascia passare la meta' del problema e'
una difesa che si e' scritta per sentirsi a posto.

### La finestra si contava in messaggi e il limite era in token

Il taglio a messaggi era corretto e non bastava, e me ne sono accorto per
caso: la conversazione finta del banco pesava 15.379 token e il contesto ne
tiene 16.384. Il 94%. Con quaranta scambi di due righe. E allora la domanda
ovvia: che succede a sfondarlo?

Nessuno l'aveva mai guardato. L'ho messo nel banco:

    request (102953 tokens) exceeds the available context size (16384 tokens)

**Dodici scambi**, non sessanta, ognuno con dentro il contenuto di un file
letto — cioe' il caso normale, non quello patologico. Centomila token contro
sedicimila. E `trim_history` non scattava nemmeno, perche' erano venticinque
messaggi su un tetto di sessanta.

La finestra si contava in **messaggi** e il limite del modello e' in **token**:
due unita' diverse che non si parlavano. Sessanta messaggi possono essere
trecento token o centomila. All'utente arrivava un JSON in inglese.

Adesso ci sono tre cose che prima non c'erano: una stima dei token (calibrata
su due misure vere, 3,88 e 4,37 caratteri per token — si tiene la piu' bassa,
perche' sbagliare per eccesso taglia un po' presto e sbagliare per difetto
sfonda), un taglio che rispetta lo spazio davvero disponibile, e un messaggio
in italiano per quando succede lo stesso.

**E il conto dello spazio disponibile e' la scoperta dentro la scoperta.**

| | token | quota del contesto |
|---|---|---|
| messaggio di sistema | ~5.200 | 32% |
| schemi dei sessanta tool | ~6.900 | 42% |
| riserva per la risposta | 1.024 | 6% |
| resta alla conversazione | ~3.300 | **20%** |

Il prefisso fisso si mangia i tre quarti del contesto. Questo ribalta OTT-8,
che diceva «gli schemi dei tool stanno nella cache, quindi accorciarli vale
poco». E' vero per la **velocita'** e falso per la **capienza**: quei token
sono gia' pagati in tempo e non lo sono in spazio. Sono due valute diverse, e
avevo guardato solo quella che si misura col cronometro.

### Quattro errori miei in un'ora, e sono tutti lo stesso

Questa correzione mi ha fatto sbagliare quattro volte, e le scrivo perche' a
guardarle in fila hanno una faccia sola: **avevo verificato la forma e non il
percorso**.

**Il taglio a token stava dopo un `return`.** L'avevo messo in fondo a
`trim_history`, dopo il controllo `if len(messaggi) <= tetto: return`. Cioe'
non veniva mai eseguito **nel solo caso per cui l'avevo scritto**: dodici
scambi sono venticinque messaggi, passano di li', e uscivano dalla porta prima
di arrivare al pezzo nuovo. Il codice era giusto, il posto no.

**Il guardiano `<= 2` saltava il caso limite.** «Con due messaggi non c'e'
niente da fare» — falso: non c'e' niente da *togliere*, ma c'e' ancora da
*accorciare*, ed e' proprio il caso del file enorme.

**Un ciclo che ovviamente finisce e non finiva.** Accorciando tenevo
millecinquecento caratteri in testa e in coda e ci scrivevo in mezzo quanti ne
avevo tolti. Alla seconda passata restavano da togliere ottanta caratteri e la
scritta ne aggiungeva ottanta: il testo si accorciava e ricresceva, per
sempre. La prova si e' appesa, ed e' cosi' che l'ho scoperto — non
leggendolo. Adesso la lunghezza si **calcola** invece di essere fissa, e c'e'
una seconda condizione d'uscita («se non ho guadagnato niente, smetto»), che
e' quello che si mette quando una condizione sola si e' gia' vista sbagliare.

**E una prova che diceva il falso.** Confrontavo i primi tre messaggi prima e
dopo, ma dopo il taglio ne restavano due: confrontavo una lista di due con una
di tre e leggevo «la testa si e' mossa» mentre non si era mossa affatto.

C'e' anche una quinta cosa, che non e' un errore ma un modo di sbagliare che
si e' ripetuto tre volte in mezz'ora. Le classi finte delle prove prendevano i
metodi di `Agent` uno per uno (`trim_history = Agent.trim_history`), e ogni
metodo nuovo le rompeva con un `AttributeError` che non c'entrava niente con
cio' che stavano provando. Adesso **ereditano**. E' la stessa lezione
dell'elenco dei binari di stamattina: una copia scritta a mano di cio' di cui
una cosa e' fatta si disallinea sempre, e la si scopre dal lato sbagliato.

Alla fine, sul modello vero: dove prima c'era `HTTPError 400`, adesso ci sono
due messaggi, 1.580 token e una risposta in un secondo.

### Settimo colpo: i guasti, e un buco che avevano tutti e due

`nova-guasti` e' il pezzo che serve a tutti gli altri: quando il Python sara'
andato via, qualunque parte di NOVA che debba dire «non ci sono riuscito»
dovra' poterlo dire come lo dice NOVA, non come lo dice il sistema operativo.
Le frasi, la tabella degli errori di Windows, i nomi dei pacchetti da
installare — e `senza_chiavi`, che non e' cortesia ma sicurezza.

Il confronto e' andato liscio sulle frasi: dodici guasti per due, con e senza
premessa, gli errori di Windows numero per numero, i nomi dei pacchetti.
Tutto identico al primo colpo.

Poi c'e' stata la parte che conta.

### La prova che non chiede l'accordo ma il risultato

Sul mascheramento delle chiavi non ho scritto la prova solita — «le due parti
dicono la stessa cosa» — e per una ragione che si e' rivelata giusta per il
motivo sbagliato. L'avevo scritta cosi' perche' **coprire di piu' e' un
fastidio e coprire di meno e' una chiave che esce**: sono due errori
asimmetrici, quindi pretendere l'uguaglianza sarebbe stato pretendere la cosa
sbagliata. La prova chiede due cose diverse: che il Rust non copra **meno** del
Python, e che di ogni segreto messo dentro **non resti niente da nessuna delle
due parti**.

E' la seconda a essere diventata rossa, su tutte e due.

    Authorization: Bearer abcdefghijklmnop1234567890

Non lo copriva nessuno. «Authorization» non finisce per key, token o secret;
«Bearer» nemmeno; e il valore non ha un prefisso noto. Ed e' il modo **piu'
comune** in cui una chiave finisce dentro un messaggio d'errore o una
richiesta registrata: e' l'intestazione HTTP che la trasporta.

La cosa da tenere non e' il buco, e' come si e' visto. Le due implementazioni
erano **d'accordo**, quindi qualunque confronto fra loro sarebbe passato: sei
pezzi di cantiere fatti bene, un metodo che ha trovato una decina di difetti,
e su questo sarebbe stato cieco per costruzione. Un confronto trova le
**differenze**; per trovare gli errori condivisi bisogna chiedere qualcosa al
risultato, non all'accordo.

E' anche il motivo per cui `test_binari.py` di stamattina confronta l'elenco
col workspace e non con una seconda copia dell'elenco, e per cui la prova del
GGUF a meta' guarda se il file arriva dove dice invece di guardare se i due
lettori concordano. Le avevo scritte cosi' d'istinto; adesso so perche'.

Corretto da tutte e due le parti: `bearer`, `authorization`, `password`,
`passwd` fra le parole spia, e il confronto ora insensibile alle maiuscole,
perche' quell'intestazione si scrive in tre modi diversi a seconda di chi la
manda.

### Una divergenza tenuta apposta

Le due parti non mascherano allo stesso modo, e va detto. Il Python sostituisce
tutta la corrispondenza, quindi `api_key: sk-...` diventa `[chiave]`; il Rust
tiene la parola e il separatore e copre solo il valore, quindi diventa
`api_key: [chiave]`. La seconda si legge e la prima no — chi guarda un
registro vuole sapere **quale** segreto e' stato coperto, non solo che ce n'era
uno. Non l'ho uniformata perche' e' un miglioramento, e la prova non la
segnala perche' non pretende l'uguaglianza: pretende che il Rust non copra
meno, e coprire meglio lo stesso valore non e' coprire meno.

## 1 settembre 2026 — le voci che non aspettavano nessuno

Detto che la seconda macchina per ora non c'e', la cosa utile non era
scegliere un'altra voce: era rileggere quelle che avevo messo in attesa.
Dieci voci di compatibilita' su dieci portavano l'etichetta «serve un altro
PC», e almeno tre non l'hanno mai meritata. **I percorsi ostili si
costruiscono qui.**

Questa macchina ha quelli facili — `C:\Users\giova`, niente OneDrive, niente
accenti — ed e' precisamente per questo che nessuno di quei casi era mai stato
esercitato. L'etichetta non descriveva la voce: descriveva la mia distrazione.

`test_percorsi_ostili.py` costruisce sei cartelle che rompono cose diverse, e
ognuna per un motivo suo: gli spazi rompono chi concatena una riga di comando
invece di passare una lista; gli accenti rompono chi apre un file con la
codifica di sistema; l'apostrofo rompe le virgolette di PowerShell ed e'
comunissimo in italiano; le parentesi e la `&` sono metacaratteri di shell.

**Tutto verde al primo colpo**, ed e' un risultato piu' che una delusione:
vuol dire che passare le radici come **dati** invece che come stringhe di
comando — scelta fatta giorni fa per poter provare con una cartella finta — ha
pagato anche qui, su un problema che non stavo cercando di risolvere.

### Ma il guaio di OneDrive era un altro

La voce diceva «Documenti ridiretto su OneDrive». Cercandolo si scopre che il
meccanismo non e' quello: **non e' il percorso che si rompe, e' che NOVA ci si
installa dentro**. L'installatore mette i modelli sotto la propria cartella, e
la propria cartella e' dove qualcuno ha scompattato il file. Se e' Documenti,
e Documenti e' sincronizzato:

- dodici gigabyte di modello partono verso il cloud, e su un piano gratuito da
  cinque non ci stanno: il messaggio che ne esce parla di quota e non di NOVA;
- il vault viene sincronizzato **mentre** NOVA ci scrive, e nascono le copie
  in conflitto accanto agli originali;
- e con i file su richiesta il modello viene «liberato» per far spazio: resta
  in elenco, diventa un segnaposto vuoto, e llama.cpp trova zero byte.

L'ultima e' la peggiore perche' capita **mesi dopo**, a NOVA che funzionava, e
chi la subisce non ha nessun motivo di collegarla all'installazione.

`nova/cartelle.py` lo riconosce, e l'installatore lo dice **prima** di creare
la cartella — la prova controlla anche l'ordine, perche' un avviso dopo il
fatto e' un rimprovero. Non e' un divieto: la cartella e' dell'utente. Ma la
scelta si fa sapendo.

### Un avviso sbagliato e' peggio di nessun avviso

La prima versione del riconoscimento accettava il nome del servizio seguito da
un trattino secco, per prendere «OneDrive - Acme». Cosi' segnalava anche
`dropbox-export-2024`, che e' una cartella di roba **tirata fuori** da
Dropbox: il contrario di una cartella sincronizzata. La prova l'ha bocciata, e
ha ragione — la seconda volta che un avviso e' sbagliato non lo legge piu'
nessuno, e il terzo che era giusto passa inosservato.

Rileggendo l'elenco dei nomi ci ho anche trovato tre varianti di «iCloud» che
non esistono, e un «sync» talmente generico da segnalare mezzo disco. Scritti
di getto e mai riletti.

### Il verde che vale meno di quello che sembra

I percorsi oltre i 260 caratteri qui funzionano. Ho controllato **perche'**:
`LongPathsEnabled` vale 1 su questa macchina, e il valore di fabbrica di
Windows e' 0. Quindi quella riga verde vuol dire «funziona dove non serviva
che funzionasse», e sulla macchina di chiunque altro fallirebbe.

Adesso la prova legge il registro e lo scrive a schermo. Non l'ho fatta
fallire — non c'e' niente di rotto qui — ma passare in silenzio sarebbe stata
falsa sicurezza: «da me funziona» con un bollino verde sopra, che e' la
versione peggiore perche' si difende da sola.

### E una prova a orologeria

Nel giro completo e' saltata fuori `test_prefisso.py`, che ha annunciato
sedicimila caratteri di differenza nel prompt di sistema — cioe' il prefisso
instabile, cioe' il disastro che ho passato ieri a curare.

Non c'era niente di rotto. Quella prova confrontava il prompt a **ora ferma**
(congelata al 30 agosto) con uno a **ora vera**. Ha funzionato per un giorno
esatto: il giorno in cui le due coincidevano. Oggi la data vera ha un nome di
giorno di lunghezza diversa, lo `zip` si e' disallineato, e tutto quello che
veniva dopo e' risultato diverso.

La cosa che fa piu' impressione e' che venti righe sopra, in quello stesso
file, c'e' scritto: «una prova che fallisce una volta ogni tanto non e' una
prova, e' una che si impara a ignorare». L'avevo scritto io, e due righe dopo
avevo messo una bomba a orologeria. Adesso si confrontano due ore **entrambe
ferme**, scelte perche' il testo abbia la stessa lunghezza, e si guarda **dove
cadono** le differenze invece di contarle.

### CMP-8: Gemma regge, e tutti e due sbagliano la stessa cosa

Quarta voce mal classificata. «Modelli che non sono Qwen: vanno provati o
tolti» stava fra quelle in attesa di un secondo PC, e i modelli sono qui — li
sto misurando da due giorni. Solo che li avevo misurati in **velocita'**, e un
modello puo' fare quaranta token al secondo senza saper chiamare un tool: quei
token non servono a niente.

`banco_cervello.py` chiede un'altra cosa: fra sessanta strumenti, sceglie
quello giusto? E sa anche **non** sceglierne nessuno, quando la domanda si
risponde parlando — che e' il caso che i modelli piccoli sbagliano di piu',
perche' chiamano qualcosa per compiacenza.

| | tool giusti | inventati | mancati | di troppo |
|---|---|---|---|---|
| Gemma 4 26B-A4B | 7 su 8 | 0 | 0 | 0 |
| Qwen3.8 27B | 7 su 8 | 0 | 0 | 0 |

Gemma regge il confronto. La riga del README che lo consiglia adesso ha una
prova sotto invece di una speranza, e la domanda su `models.json` — quale
famiglia sia `consigliata` — perde l'ultimo argomento contrario.

### Ma sbagliano **la stessa** domanda

A «Ricordati che il mio gatto si chiama Ugo» tutti e due chiamano `kb_search`
invece di `kb_note`. Cercano una cosa che l'utente sta dicendo in quel
momento.

Due modelli diversi, addestrati da aziende diverse, che sbagliano identico non
sono due modelli sbagliati: **e' NOVA che glielo sta dicendo male**. E il
banco non poteva dirmelo confrontandoli fra loro — l'avrebbe scritto come «i
due sono d'accordo, tutto bene». E' la stessa lezione delle chiavi di ieri, e
stavolta l'ho vista perche' la prova non chiedeva l'accordo: chiedeva se il
tool scelto fosse quello giusto.

Il sospetto naturale era la lingua. «Ricordati» in italiano e' un imperativo
che vuol dire «memorizza», ma un modello addestrato per lo piu' in inglese
puo' leggerlo come «recall», cioe' cerca. L'ho provato con «Il mio gatto si
chiama Ugo», che e' un'affermazione secca senza nessun verbo ambiguo. Stesso
esito. **Non e' la parola.**

Guardando il prompt di sistema il colpevole sembrava ovvio:

> Prima di chiedere qualcosa che potresti gia' sapere, cerca con kb_search.
> Quando l'utente rivela qualcosa di durevole [...], salvalo con kb_note

`kb_search` arriva per primo e in forma generale; `kb_note` dopo, con
«rivela», che e' passivo. L'ho riscritto mettendo prima il caso di chi
racconta, e ho corretto le descrizioni dei due strumenti.

**Non e' bastato**, e lo scrivo perche' e' misurato e non supposto. La
riscrittura e' piu' chiara e resta, ma il comportamento non e' cambiato. Le
ipotesi che restano, in ordine di costo: l'**ordine** degli strumenti
(`kb_search` compare prima di `kb_note` fra i sessanta, e la posizione pesa);
la **coda del turno**, perche' il banco manda sistema piu' domanda mentre NOVA
aggiunge memoria, procedure e postilla, e il difetto potrebbe stare li' senza
che il banco lo veda; oppure **un solo strumento di memoria** che decida da
se' se scrivere o cercare, invece di due che si somigliano.

E' un difetto che pesa piu' della sua dimensione: «ricordati che...» e' fra le
prime cose che chiunque prova, e il README promette una memoria che sopravvive
alle sessioni. Prometterla e non scriverla e' peggio che non prometterla.

### Due note su come e' andata la misura

**NOVA stava girando.** A meta' dei giri il banco ha cominciato a morire con
un `ConnectionResetError`: il modello di NOVA occupava 8,7 GB di VRAM e il
secondo non ci stava piu' accanto. Non ho fermato niente di suo — il banco ha
imparato `--strati 0`, che lo fa girare sul processore. Piu' lento, ma **quale
tool sceglie un modello non dipende da dove gira**: la risposta e' la stessa,
e la domanda a cui rispondevo non era sulla velocita'.

**E il banco confronta due modelli su una prova sola.** Otto domande non sono
una valutazione: sono un controllo di funzionamento. Non dicono che Gemma e'
bravo quanto Qwen, dicono che entrambi sanno usare gli strumenti di NOVA e che
nessuno dei due ne inventa. Per la voce CMP-8, che chiedeva «provati o tolti»,
e' esattamente quello che serviva.

## 2 settembre 2026 — lo strumento che riesce

COM-11 diceva: «il modello locale senza `mmproj` — `schermo` non deve
rompersi, deve spiegarsi». L'ho letta come una voce sulla robustezza di uno
strumento. Era una voce su una **bugia**.

`schermo` non si rompeva. Scattava la schermata, la salvava, tornava il
percorso: tutto giusto. Poi `_consegna_immagini` vedeva un file immagine
nominato nel risultato e lo allegava alla conversazione, come deve. E li'
finiva bene la parte che si vedeva.

### Cosa dice davvero llama-server

Non l'ho dedotto, l'ho chiesto. Server acceso a mano sulla 8499 con Gemma 4
26B-A4B, `-ngl 0` perche' NOVA aveva la GPU, `--no-warmup` perche' non serviva
generare niente, **senza** `--mmproj`. Una chiamata OpenAI con dentro un JPEG
di un pixel:

    HTTP 500
    {"error":{"code":500,
              "message":"image input is not supported - hint: if this is
                         unexpected, you may need to provide the mmproj",
              "type":"server_error"}}

Buona notizia: il server **non** ignora l'immagine in silenzio. Il modello
cieco non risponde inventandosi cosa c'era sullo schermo — quella era la
paura, e non era fondata. Cattiva notizia: da qui in poi tre cose sbagliate.

**Uno.** `spiega_http` mandava ogni 5xx a «il problema e' dall'altra parte,
non tua. Di solito passa da solo». Questo non passa da solo. E' un file che
non e' stato scaricato, e restera' non scaricato per sempre. Una diagnosi
sbagliata non e' neutra: manda qualcuno ad aspettare.

**Due, ed e' il vero difetto.** Il messaggio con la figura resta in
conversazione. Il turno dopo la rimanda. Fallisce uguale. E quello dopo
ancora. Non e' un turno perso: e' una conversazione che **non riparte piu'**
finche' non la si butta via. Un errore che si ripete a comando e' peggio di
uno che esplode una volta, perche' il secondo lo capisci.

**Tre.** Nessuno chiedeva prima. `runtime._build_args` cercava gia' il
proiettore accanto al modello per decidere se passare `--mmproj`. La risposta
c'era. Chi allegava le figure non la leggeva.

### La cura, nell'ordine in cui conta

Prima **non mandare**: `proiettore_accanto` esce da `_build_args` e diventa
una funzione sua, `vede_il_modello_locale` la usa, e `_consegna_immagini`
la chiede prima di allegare. La stessa condizione con cui si costruisce la
riga di comando decide se la figura parte — se rispondessero in due posti
diversi, prima o poi risponderebbero diverso.

Poi **dirlo al modello**. Non allegare e basta non bastava: il risultato dello
strumento nomina lo stesso un file, e un modello a cui arriva «salvata in
C:\...» e nient'altro racconta volentieri cosa c'era dentro. Al suo posto
arriva una riga che dice tre cose: la figura c'e', non te la posso far vedere,
non dire di averla guardata — e usa `ui.tree`, che legge l'interfaccia come
testo ed e' quello che NOVA preferisce comunque.

Poi **la verita' nell'errore**, per i casi che restano: un server adottato dal
demone, un endpoint di qualcun altro. `senza_vista()` guarda due indizi invece
di uno (la parola `mmproj` e la frase inglese), perche' il codice e' 500,
cioe' la casella dove finisce tutto quello che non ha una casella.

E infine **smurare**: se l'errore arriva lo stesso, si sfilano le immagini
dalla conversazione tenendo il testo, e si riprova una volta. Il turno
finisce invece di lasciare tutto bloccato.

### L'errore che ho fatto scrivendolo

Il ramo nuovo l'ho messo cosi':

    except RuntimeError as e:
        ...
    except LimiteUso as e:
        ...

`LimiteUso` **eredita da** `RuntimeError`. Scritto in quell'ordine, il ramo
nuovo si mangiava la quota finita: il gradino non sarebbe piu' andato in
pausa, il ripiego su un altro fornitore non sarebbe piu' partito, e l'utente
avrebbe ribattuto contro un muro esattamente come nel difetto che stavo
curando. Me ne sono accorto andando a controllare la classe base — non per
prudenza generica, ma perche' aggiungere un `except` sopra un `except` che
c'era gia' e' un posto dove si sbaglia.

In `test_visione.py` c'e' una prova che legge il sorgente di `_giro` e
controlla che `except LimiteUso` compaia **prima** di `except RuntimeError`.
E' una prova brutta e la tengo: la prossima persona che aggiunge un ramo li'
in mezzo non ha modo di sapere questa storia, e la prova gliela racconta.

Una seconda inciampata, piu' piccola e piu' istruttiva. La prova diceva
«non promettere che passa da solo» cercando la sottostringa `passa da solo`.
La risposta giusta e' «**Non** passa da solo». La prova bocciava la cura. Un
test che boccia la risposta corretta e' peggio di nessun test, perche' il modo
piu' rapido di farlo passare e' peggiorare il codice.

### Una nota di attrezzi

`git` dentro la VM Linux, su questa cartella montata, vede LF dove `git` di
Windows vede CRLF: dice 55 file modificati e 9.553 righe cambiate che non
esistono, e lascia un `index.lock` che da li' non si puo' cancellare. Da
Windows la stessa cartella e' pulita. Per lo stato e per i commit si usa
PowerShell.

32 prove nuove, tutte verdi.

## 2 settembre 2026, pomeriggio — quello che resta

Seconda voce chiusa oggi, e anche questa non stava dove diceva la sua riga.

«Disinstallare deve togliere tutto, dire cosa ha tolto e cosa ha lasciato
apposta.» Il primo pezzo c'era da un mese: l'uscita e' una tabella, «avvio
automatico: rimosso», «attivita' pianificate: rimosse (2)», e le attivita'
pianificate sono la cosa che pesa davvero, perche' sono l'unica che
*continua a girare* dopo che il programma non c'e' piu'.

Il secondo pezzo era una frase: «i tuoi dati restano dove sono». Vera. E che
non dice niente, perche' il punto e' **dove**.

### I posti sono tre

Gliel'ho chiesto invece di leggerlo:

    Le credenziali            %APPDATA%\NOVA            dentro
    Il fascicolo              Documenti\NOVA\fascicolo  fuori
    La memoria a grafo        C:\...\NOVA\vault         fuori
    Il registro, i guasti,
    le procedure, l'harness   %APPDATA%\NOVA            dentro
    Il modello                dove l'hai scaricato      fuori, 15,66 GB

`-ConIDati` cancellava `%APPDATA%\NOVA`. Cioe' due dei cinque. Il vault
sopravviveva **in silenzio**, e il vault e' la memoria: chi disinstalla per
ricominciare pulito si ritrovava NOVA che si ricorda tutto. E il modello,
sedici gigabyte, non era nominato da nessuna parte — un disinstallatore che
tace su sedici gigabyte non e' discreto, e' reticente.

### L'elenco non si riscrive

Dentro `install.ps1` la tentazione era mettere una lista di percorsi. Sarebbe
stata la terza volta: l'elenco dei binari, l'elenco delle cartelle
sincronizzate, e adesso questa. Una copia scritta a mano di cio' di cui una
cosa e' fatta si disallinea sempre, e si scopre il giorno in cui qualcuno si
fida.

`nova/dati.py` sapeva gia' tutto: e' il modulo che risponde a «dove sono i
miei dati». Gli ho aggiunto `rendiconto()`, che e' lo **stesso** elenco in
JSON, piu' il modello, piu' per ogni voce la sola cosa che l'installer deve
sapere: *sparisce cancellando `%APPDATA%\NOVA`, si' o no*. L'installer lo
legge — prima di cancellare, che se no i pesi sono tutti zero — e alla fine
stampa cosa resta, con nome, peso, percorso e il comando per toglierlo.

Una scelta che tengo: **fuori da `%APPDATA%\NOVA` non si cancella niente**,
nemmeno con `-ConIDati`. Il fascicolo sono file scritti dall'utente. Il vault
puo' essere una cartella di Obsidian dove ci sono anche le sue note. Un
disinstallatore che cancella una cosa che non ha creato lui e' un
disinstallatore di cui non ci si fida mai piu'. Si dice dov'e' e decide chi
possiede il file.

### Lo strumento che falsava la misura

La prima versione di `_dentro` faceva `figlio.resolve().relative_to(...)`, ed
e' uscita una tabella incoerente: `segreti.dat` e `ricette.json` dentro,
`config.json` e `azioni.jsonl` **fuori** — tutti e quattro nella stessa
cartella. Impossibile.

Non era NOVA. Era il PowerShell da cui stavo misurando: gira dentro un
pacchetto MSIX, e per quei processi Windows reindirizza pezzi di `%APPDATA%`
in `...\Packages\<pacchetto>\LocalCache\Roaming\`. `resolve()` segue il
reinnesto e porta il file **fuori dalla sua stessa cartella**.

Lo strumento con cui misuravo cambiava la misura. Poteva finire in due modi
peggiori: crederci e scrivere codice per un problema che non esiste, oppure
non guardare la tabella e non accorgersi che il confronto era fragile.

La cura e' comunque quella giusta a prescindere dal caso MSIX: il confronto e'
**lessicale**, sui nomi. La domanda vera e' «se cancello questa cartella, se
ne va anche questo file?», e chi cancella una cartella cancella i nomi che ci
stanno sotto, non le destinazioni dei collegamenti. C'e' una prova apposta sul
caso che rovina uno `startswith` scritto senza separatore: `NOVA-vecchio` non
sta dentro `NOVA`.

Con questa il cancello della beta ha una sbarra in meno. Resta da provarlo su
una macchina che non e' questa — ma non e' piu' una voce da scrivere: e' una
voce da guardare mentre gira.

## 2 settembre 2026, sera — la promessa che nessuno aveva verificato

Terza voce. «Python 3.10, 3.11, 3.12, 3.13: l'installer dichiara 3.10+, la CI
ne prova una.»

Una versione provata su quattro dichiarate non e' una copertura parziale. Le
tre non provate sono precisamente quelle dove l'utente e' da solo: chi ha
3.12 come me non scopre mai niente, e chi ha 3.10 scopre tutto insieme, il
primo giorno, e se ne va.

La CI adesso le prova tutte e quattro, con `fail-fast: false`. Non e' un
dettaglio: fermarsi alla prima che si rompe fa arrivare la notizia «si
rompe», mentre quella utile e' «si rompe **solo** su 3.10».

### Provare quattro grammatiche con un interprete solo

Il rosso su un agente costa un giro di push e qualche minuto. Meta' di quel
lavoro si puo' fare qui, e la parte piu' insidiosa e' proprio quella che si
puo' fare qui.

`ast.parse(sorgente, feature_version=(3, 10))` fa rifiutare a Python la
sintassi arrivata dopo la 3.10. Non serve avere i quattro interpreti: la
grammatica del minimo dichiarato si prova con quello che c'e'. 128 file, tutti
leggibili come 3.10.

La libreria standard no, e li' sta l'inciampo vero: `import tomllib` **si
compila benissimo** su 3.10 e poi non parte. Sono le cose che uno usa senza
pensarci perche' sul suo PC ci sono gia': `datetime.UTC`, `StrEnum`,
`itertools.batched`, `except*`, `Path.walk`. Sedici voci cercate per nome. Non
e' un elenco completo della libreria standard e non pretende di esserlo: e'
una rete per gli inciampi comuni. Nessuna presente.

Quindi la promessa era **vera**. Ed e' il risultato meno soddisfacente e piu'
istruttivo di oggi: era vera per caso, e nessuno lo sapeva. La differenza fra
alpha e beta non e' quante cose fa un programma — e' quante di quelle che dice
di fare sono verificate.

### La prova sa dire di no

Ce n'e' due che passerebbero anche se non guardassero niente: una
`feature_version` ignorata, una regex che non compila, e restano verdi per
sempre. Allora prima di fidarsene si chiede loro di bocciare qualcosa: `type
X = int`, che e' 3.12, deve fallire; un `import tomllib` piantato apposta deve
essere trovato. E' la lezione dei percorsi ostili — una prova che passa va
guardata come una che fallisce, chiedendosi *perche'* passa — applicata alla
prova stessa mentre la si scrive, invece che tre giorni dopo.

### E una nota su dove va a finire

Nella sezione «da fare, ma dopo» c'e' scritto che con il porting in Rust
questa voce **sparisce**: senza Python sul PC dell'utente non ci sono quattro
versioni da provare. E' vero e non cambia niente oggi: la beta si spedisce con
Python, e finche' e' cosi' la riga «3.10 o superiore» dev'essere vera per
misura e non per fortuna.

## 2 settembre 2026, sera — firmare o spiegare

Quarta voce, e la prima che si chiude con una **decisione** invece che con
del codice: «SmartScreen e antivirus. Decidere se si firma o se si spiega.»

Si spiega. Non per risparmiare: una firma per un editore nuovo **non toglie
comunque** l'avviso finche' SmartScreen non gli ha costruito una
reputazione, quindi si pagherebbe qualche centinaio di euro l'anno per non
risolvere il problema. Il giorno che NOVA avra' abbastanza installazioni la
firma avra' senso, perche' allora la reputazione ci sara'. Oggi no.

Ma «spiegare» e' facile da dire e quasi sempre finisce in una riga di README
che nessuno legge nel momento sbagliato. La spiegazione vale se arriva dove
capita il fatto, e i posti erano tre.

### Uno: prima, nei README

Cosa dira' Windows e perche'. E soprattutto la cosa che l'avviso di Windows
**non** dice: l'installer confronta le impronte SHA256 pubblicate con la
release. «Non conosco l'editore» e «questo file e' diverso da quello
pubblicato» sono due affermazioni diverse, e solo la seconda e' quella che
un utente vuole davvero sapere. Windows fa la prima, l'installer fa la
seconda.

### Due: nel momento in cui succede

Questo era un difetto vero, non una mancanza di documentazione.

    if (-not (Core-Presente)) { throw "l'archivio non conteneva tutti i binari attesi" }

Frase sbagliata nel caso piu' probabile. L'archivio li conteneva tutti: e'
Windows Defender che si porta via un eseguibile nuovo e non firmato appena
compare su disco. Sono due guasti che si somigliano e si curano in modo
**opposto**: se la release e' incompleta, sul PC dell'utente non c'e' niente
da fare e va segnalato a noi; se e' l'antivirus, la release e' a posto e la
cura sta tutta sul suo PC. Dirgli quella sbagliata gli fa perdere il
pomeriggio dalla parte sbagliata.

Adesso l'installer legge l'elenco dei file **dentro** l'archivio prima di
scompattarlo. Se un binario c'era e dopo non c'e' piu', la diagnosi e'
certa, e la frase dice dove ripristinarlo e quale cartella consentire.

### Tre: il contrassegno «scaricato da Internet»

Windows lo attacca a ogni file che arriva dalla rete, e senza toglierlo
SmartScreen chiede conferma a **ogni** avvio — non la prima volta: sempre.
La schermata blu «Windows ha protetto il PC», ogni giorno, su un programma
che l'utente ha installato di sua volonta'.

Si toglie con `Unblock-File`, che e' esattamente la spunta «Annulla blocco»
nelle proprieta' del file. La cosa che rende la scelta accettabile e'
l'**ordine**: si toglie solo dopo aver confrontato le impronte. Al
contrario, si zittirebbe l'avviso di Windows su un file di cui non si sa
ancora niente — che e' precisamente il comportamento di un installer
malevolo. C'e' una prova che legge il sorgente e pretende che
«Impronte verificate» venga prima di `Unblock-File`, perche' quell'ordine e'
l'unica cosa che separa le due letture della stessa riga.

E si dice, invece di farlo di nascosto. Piu' potente e' il mezzo, piu' alti
sono i rischi; piu' alti sono i rischi, piu' si e' responsabili — e la
responsabilita' comincia dal dire cosa si e' fatto al PC di qualcun altro.

## 2 settembre 2026, notte — la memoria, e il banco che guardava la strada sbagliata

COM-15 era la voce che «pesava piu' della sua dimensione»: «Ricordati che il
mio gatto si chiama Ugo» non finiva in memoria, e il README promette una
memoria che sopravvive alle sessioni. L'avevo lasciata aperta con tre
ipotesi. Oggi l'ho chiusa, e nessuna delle tre era la risposta.

### La diagnosi vecchia era imprecisa

Il diario diceva: «tutti e due i modelli chiamano kb_search invece di
kb_note». Rimisurato pulito, Gemma a temperatura 0:

    [NO] Ricordati che il mio gatto si chiama Ugo   -> (parla)
    [ok] Il mio gatto si chiama Ugo.                -> kb_note
    [ok] Salva in memoria: il mio gatto si chiama   -> kb_note
    [ok] Che cosa sai di me?                        -> kb_search

Non chiama `kb_search`. Non chiama **niente**: risponde a parole. E sbaglia
solo l'imperativo — le altre due forme scrivono giuste. Tre su quattro. La
diagnosi vecchia aveva unito due misure diverse (Qwen e Gemma, codice di
ieri) e ne era uscita una frase piu' netta del vero.

### Ho provato a forzarlo, e l'ho peggiorato

Ipotesi economica: dirlo piu' forte. Ho rafforzato la descrizione di
`kb_note` e il prompt di sistema — «rispondere lo ricordero' senza scrivere
e' il modo piu' facile di mentire». Misurato: da 3/4 a **1/6**. Ogni caso di
memoria e' passato a «(parla)». La lingua insistente spinge il modello a
rassicurare a parole invece di agire: piu' lo imploravo di usare lo strumento,
meno lo usava. Ripristinato tutto.

Lo scrivo perche' e' contro-intuitivo e vale per la prossima volta: sul tool
calling di un modello locale piccolo, alzare la voce nel prompt e' spesso
controproducente. Il modello non ha un capo a cui obbedire; ha una
distribuzione da seguire, e il testo drammatico la sposta verso il registro
drammatico, che e' fatto di parole, non di chiamate.

### La strada che il banco non guardava

Poi mi sono ricordato che NOVA ha **due** strade verso la memoria. `kb_note`
e' la prima. La seconda e' `MemoryWriter.osserva`: un estrattore in
sottofondo che a ogni turno rilegge lo scambio e scrive da se' i fatti
durevoli. E legge il messaggio dell'**utente**, non solo la risposta.

Il banco misurava solo la prima strada — quale tool sceglie il modello in un
turno. Ma la domanda dell'utente non e' «hai chiamato lo strumento giusto?».
E' «te lo sei ricordato?». Misurato end-to-end, nel caso peggiore in cui il
modello non chiama niente:

    utente: Ricordati che il mio gatto si chiama Ugo.
    NOVA (solo parole): Certo, me lo ricordero'!
    -> vault: [il-gatto-di-gio] «Il gatto di Gio si chiama Ugo.»

    utente: Ricorda che lavoro meglio la mattina presto.
    NOVA (solo parole): Perfetto, ne terro' conto.
    -> vault: [preferenza-orario-di-lavoro] «Gio lavora meglio
               durante le prime ore del mattino.»

Il fatto arriva in memoria comunque. La promessa e' mantenuta dalla seconda
strada. E' D51 un'altra volta: un banco che confronta lo strumento scelto non
misura il risultato, e la prova giusta chiede «e' finito nel vault?».

`test_memoria_seconda_strada.py` mette al posto del modello un finto LLM e
controlla la tubatura senza accenderne uno: che `osserva` passi il messaggio
dell'utente all'estrattore, che scriva il nodo anche quando la risposta e'
solo una promessa, e che un turno che ha guardato lo schermo non finisca in
memoria. Undici prove, tutte verdi.

Resta un margine di lucidatura per dopo — far scattare `kb_note` sull'imperativo
cosi' il fatto compare *subito* invece che al giro dell'estrattore — ma non e'
la differenza fra ricordare e dimenticare. E' la differenza fra ora e fra due
secondi, e non tiene aperta la voce.

## 2 settembre 2026, notte — il cantiere riaperto: calendario e pianificazione

Chiusa la todo di quel che si poteva chiudere da soli, ho riaperto il
cantiere. Ottavo pezzo: `prossimo()` di `pianificazione.py`, cioe' la
funzione che traduce «ogni lunedi' alle 9» nell'istante in cui tocca.

Sta nel gruppo delle **decisioni**, con `nova-scala`. Non calcola qualcosa da
mostrare: decide quando NOVA fa una cosa da sola. Sbagliarlo di un giorno vuol
dire un'attivita' che non parte, o che parte in continuazione.

### Il pezzo che non era in programma

Aprendo il lavoro serviva `giorni_del_mese`. Esisteva gia' — privata, dentro
`nova-registro`, dodici righe con la regola gregoriana giusta (il 1900 non
bisestile, il 2000 si').

La strada comoda era ricopiarla. Sarebbe stata la **quarta** volta che questo
progetto si accorge di aver duplicato cio' di cui una cosa e' fatta: l'elenco
dei binari, le cartelle sincronizzate, i posti dei dati, e adesso il
calendario. Le prime tre le ho scoperte dopo che si erano gia' disallineate.
Questa l'ho vista prima, il che e' l'unica differenza che conta.

Quindi e' nato `nova-calendario`, e `nova-registro` adesso lo usa al posto
della sua copia. Alla seconda occorrenza si mette in comune, non alla quarta.

Due scelte dentro, tutte e due ereditate da com'era gia' fatto il registro:
**nessun orologio** — l'ora si passa da fuori, come `oggi` in `giorno()`,
cosi' si prova a qualunque ora — e **nessun fuso**, perche' un fuso e' una
domanda di piattaforma e sta in `nova-platform`. Qui resta la parte che non
cambia mai: quanti giorni ha febbraio, che giorno della settimana e' il tre
marzo, cosa viene dopo il 31 dicembre.

### La prova che sbagliava era la prova

`piu_giorni` ha una proprieta' che volevo verificare: andata e ritorno devono
tornare al punto di partenza. L'ho scritta, e ha fallito:

    left:  "2025-12-31T10:00:00"
    right: "2026-01-01T10:00:00"

Il primo istinto e' cercare il difetto nel riporto sui mesi. Non c'era: il
2026 **non e' bisestile**, ha 365 giorni, quindi 366 giorni indietro dal primo
gennaio 2027 sono il 31 dicembre 2025. Avevo scritto 366 per analogia con «un
anno», che e' vero solo negli anni bisestili. **A sbagliare era l'attesa, non
il codice.**

E' il caso opposto a quello solito e vale la pena tenerlo scritto: quando una
prova fallisce, la prima cosa da verificare e' che abbia ragione lei. Adesso
la prova controlla 365 e 366 separati, e in piu' lo stesso conto a partire dal
2025 — che segue un anno bisestile e quindi da' un risultato diverso di un
giorno. Se un domani qualcuno «aggiusta» il riporto, quelle tre righe insieme
gli dicono che il calendario non e' simmetrico.

### Il confronto, e cosa il confronto non vede

`test_pianificazione_rust.py` manda 7 momenti per 28 frasi — 196 casi — a
tutte e due le implementazioni e le confronta cifra per cifra. Tutte uguali.

Ma il confronto da solo non basta, ed e' D51: due implementazioni che
concordano non sono due implementazioni verificate. Se avessi copiato un
errore dal Python al Rust senza accorgermene, 196 casi su 196 sarebbero
d'accordo e nessuno direbbe niente. Quindi la prova ha anche undici
**risultati attesi calcolati sul calendario**, scritti a mano senza chiedere
niente a nessuna delle due: che il 2 settembre 2026 e' un mercoledi', che dal
28 febbraio 2024 si passa per il 29 e dal 28 febbraio 2026 no, che «ogni 30
minuti» alle 23:30 del 31 dicembre da' Capodanno. Il Rust li da' giusti, e il
Python pure.

E una proprieta' che nessuno dei due deve rompere: **il prossimo e' sempre
avanti**. Un istante nel passato farebbe partire l'attivita' subito, e poi di
nuovo, e poi di nuovo — il difetto che non si vede finche' non e' notte.

126 prove Rust verdi in tutto il workspace, 196 casi di confronto, 0
divergenze.

## 2 settembre 2026, notte — il nono pezzo: quando si molla

`nova-salita`. Chiude il gruppo delle decisioni, e come `nova-scala` decide
una cosa che si vede in bolletta e in riservatezza: **quando un compito esce
dal PC**. Salire di gradino vuol dire passarlo a un modello piu' capace, che
quasi sempre e' un modello di qualcun altro.

### Due modi di non farcela, e non si curano allo stesso modo

Il Python aveva gia' la distinzione giusta, e portandola si vede meglio.

**Sbattere contro un muro**: N chiamate di fila che falliscono. Qui si sale.

**Girare a vuoto**: la stessa `list_directory` sulla stessa cartella, otto
volte, e ogni volta con esito OK. Qui non c'e' niente da far salire — il
gradino sopra rifarebbe lo stesso giro. C'e' da far **notare**.

La seconda e' quella che fa davvero il modello locale, ed e' anche quella in
cui e' piu' facile scrivere la cosa sbagliata: bloccare la chiamata ripetuta.
Non si fa. Esce un promemoria e la decisione resta al modello — riprovare
diversamente, cercare altrove, o concludere con quello che ha. Una
ripetizione legittima non viene impedita da niente.

Tre soglie — la terza, la quinta, l'ottava — e poi silenzio. Un promemoria a
ogni giro diventa rumore, e il rumore si impara a saltare.

E un dettaglio che sembra piccolo e non lo e': gli strumenti di servizio
(`get_datetime`, `kb_stats`, `modelli`) non spezzano la catena. Se contassero,
basterebbe un `get_datetime` in mezzo per ripulire un ciclo e renderlo
invisibile — cioe' il modo piu' facile di girare a vuoto senza che nessuno lo
dica.

### Cosa ho deciso di NON portare

Gli argomenti di una chiamata arrivano al Rust **gia' resi in testo**.

La tentazione era far produrre al Rust la stessa identica stringa di
`json.dumps(args, sort_keys=True)`, per confrontarla a byte col Python.
Sarebbe stato lavoro vero — le spaziature di `json.dumps` non sono quelle di
`serde_json`, e poi c'e' `ensure_ascii`, e poi `default=str` — per far
combaciare una stringa **che non esce mai dal processo**. Serve solo a dire
«questa chiamata e' uguale alla precedente».

Quindi la serializzazione resta fuori e si confrontano le **decisioni**. E'
la stessa linea di `nova-calendario` senza fusi e di `nova-registro` senza
disco: dentro la parte che decide, fuori la parte che parla con qualcun
altro.

### La prova che il confronto non puo' fare

512 combinazioni di `serve_salire` — quattro configurazioni per tutti i
valori di fallimenti, salite e passi — tutte identiche. La catena delle
ripetizioni confrontata passo per passo su una sequenza che contiene i due
casi scomodi: gli argomenti che cambiano, e il tool trasparente in mezzo.
Zero divergenze.

Ma c'e' una prova che il confronto non potrebbe mai fare, ed e' quella sul
promemoria che resta un promemoria. Se un domani qualcuno lo trasformasse in
un veto, e lo facesse in tutte e due le implementazioni, il confronto
resterebbe verde: sarebbero d'accordo nel fare la cosa sbagliata. Quindi c'e'
un controllo che guarda il **testo che esce** e pretende che dica al modello
che la scelta e' sua, e che non contenga «non puoi», «vietato», «bloccato».
E' D51 applicata a una proprieta' di prodotto invece che a un segreto.

Un caso che ho messo apposta fra le prove Rust: `passi_prima_di_salire` a
zero deve **spegnere** la soglia, non accenderla. Con un `>=` scritto senza
pensarci, «passi >= 0» e' sempre vero e si salirebbe al primo giro — cioe' la
manopola che serve a disattivare la funzione la farebbe scattare sempre.

## 2 settembre 2026, notte — il decimo pezzo, e Python che esce da una strada

`nova-catalogo`: se un modello ha senso su questa macchina, quale variante, e
cosa dire mentre lo si fa.

E' il pezzo con il criterio d'ordine piu' limpido del cantiere, quello scritto
mesi fa e mai applicato fino in fondo: **quando** una cosa deve funzionare.
`catalogo.py` porta in testa che e' di sola libreria standard «perche' viene
eseguito dall'installatore prima che le dipendenze del progetto siano
garantite». Cioe' il primo passo dell'installazione dipende da qualcosa che
l'installazione non ha ancora fatto. In Rust quel vincolo non esiste: e' un
binario, e i binari il core li ha gia' scaricati due sezioni prima.

`install.ps1` ora chiede al binario e ripiega su Python solo per chi compila
da sorgente. La regola resta in un posto solo — l'installatore non ne ha una
copia sua, che era il punto di `_principale()` fin dall'inizio — ma non serve
piu' un interprete per applicarla.

### Le prove passavano. Il programma no.

216 verdetti confrontati col Python, testo compreso, zero divergenze. Poi ho
fatto la cosa che le prove non facevano: ho chiamato il binario **come lo
chiama l'installatore**, da PowerShell, col catalogo vero.

    {"si_scarica":false,"motivo":"questo modello legge almeno 0.0 GB
     per ogni token che scrive, e senza scheda video ..."}

«questo modello». «0.0 GB». Nessun nome, nessun numero. La famiglia non era
arrivata affatto.

Due difetti, e il secondo e' quello che conta.

**Il BOM.** `Set-Content -Encoding UTF8` su Windows PowerShell 5.1 mette tre
byte davanti al primo `{`, e serde si ferma con «expected value at line 1
column 1» — che e' vero e non aiuta nessuno. Si potrebbe dire che sbaglia chi
chiama. Ma chi chiama e' PowerShell, e questo binario esiste **per** essere
chiamato da PowerShell: incontrarlo li' e' compito suo. Lo stesso inciampo era
gia' costato tempo dalla parte Python, dove i sorgenti si leggono con
`utf-8-sig`.

**E `unwrap_or_default()`.** Questo e' il difetto vero, e l'ho scritto io
un'ora prima. Una domanda illeggibile diventava una `Famiglia` vuota; la
famiglia vuota attraversava tutta la logica senza inciampare e produceva un
verdetto **perfettamente formato**: si_scarica false, un motivo in italiano,
un suggerimento. Sembrava una risposta. Sarebbe stato un installatore che
rifiuta ogni modello, a chiunque, dando una ragione inventata — e nessuno se
ne sarebbe accorto, perche' rifiutare e' anche la risposta giusta in molti
casi veri.

E' la stessa frase che sta scritta in cima a `immagini.py` da settimane: *uno
strumento che riesce senza consegnare niente e' peggio di uno che manca,
perche' produce fiducia mal riposta.* L'avevo scritta io, e un'ora dopo ho
messo un `unwrap_or_default()` su un ingresso esterno. Saperlo non basta:
serve guardare, e guardare vuol dire eseguire la cosa nel posto dove vivra'.

Adesso una domanda illeggibile dice «non ho capito la domanda sul modello» e
la prova pretende che il motivo **non** contenga «0.0 GB» — cioe' che non
finga di aver misurato qualcosa.

### Cosa ho imparato sull'ordine delle prove

Il confronto Rust/Python era verde con il difetto dentro, perche' il confronto
manda JSON scritto da `json.dumps`: niente BOM, e sempre valido. Le prove
parlavano al binario in una lingua che l'installatore non usa.

Non e' un difetto del metodo del banco — quello trova le divergenze, e le
trova bene. E' che il banco misura la **traduzione**, e questi due difetti
stavano nel **giunto**. Per quelli serve chiamare la cosa da dove verra'
chiamata davvero, una volta, a mano. Un minuto di lavoro che ha trovato piu' di
216 casi automatici.

12 prove nuove, 216 verdetti identici, e due che l'automatismo non poteva
vedere.
