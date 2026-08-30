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
