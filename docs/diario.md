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

Questa macchina ha quelli facili — un nome utente corto, niente OneDrive, niente
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

## 2 settembre 2026, notte — l'undicesimo pezzo, e una funzione in un cassetto

`nova-cartelle`: se una cartella e' sincronizzata col cloud, prima che ci
finiscano dentro dodici gigabyte. Con `nova-catalogo` chiude il gruppo delle
domande che l'installatore fa **prima** che Python esista — adesso le fa un
binario tutte e due.

Il porting in se' e' stato liscio: 34 casi confrontati, testo compreso, zero
divergenze. La cosa che vale la pena scrivere e' quello che ho trovato mentre
lo facevo.

### Una funzione che nessuno chiamava

`cartelle.py` descrive tre guai della cartella sincronizzata, e dice
esplicitamente qual e' il peggiore: il modello «liberato» per far spazio, che
resta in elenco con la sua dimensione e dentro non ha piu' niente. llama.cpp
lo apre e trova zero byte. Capita **mesi dopo**, a NOVA che funzionava, il che
lo rende il piu' difficile da collegare alla sua causa.

Il rilevatore c'era: `solo_segnaposto`, quindici righe, con la sua prova. Ho
cercato chi lo chiamasse:

    ./nova/cartelle.py:113:def solo_segnaposto(...)
    ./test_percorsi_ostili.py:244: ... not cartelle.solo_segnaposto(...)
    ./test_percorsi_ostili.py:246: ... not cartelle.solo_segnaposto(...)

Solo la definizione e la sua prova. In tutto il programma, nessuno. NOVA
descriveva con precisione il guasto peggiore e **non lo guardava mai**.

Non e' un difetto di codice — il codice era giusto e passava le prove. E' un
difetto di collegamento, ed e' il tipo che le prove non trovano per
costruzione: una funzione pura, provata, corretta e mai invocata ha tutte le
prove verdi che si possono desiderare. La prova diceva «se la chiami,
risponde bene». Non diceva «la chiama qualcuno».

Adesso `segnaposto` sta in `nova-platform` — e' una domanda al filesystem, e
quelle stanno li' — e l'installatore la fa prima di accettare un modello che
l'utente indica. E c'e' una prova che legge `install.ps1` e pretende che ci
sia la chiamata: **una funzione che nessuno invoca e' come non averla**, e la
differenza non si vede finche' non serve.

### «Onedrive»

Chiamando il binario come lo chiama l'installatore — la stessa abitudine che
ieri sera ha trovato il BOM e l'`unwrap_or_default()` — e' uscito questo:

    {"servizio":"Onedrive", ...}

Il Python faceva uguale, quindi il confronto era verde. Ma il servizio si
chiama OneDrive, e — peggio — nella **stessa installazione** poteva uscire in
due modi: «OneDrive» se riconosciuto dalla variabile d'ambiente, dove il nome
e' scritto a mano, e «Onedrive» se riconosciuto dal nome della cartella, dove
passava da un `.title()`.

E' un dettaglio, e qui i dettagli di lingua sono sostanza: quel messaggio deve
convincere qualcuno a spostare una cartella. Una maiuscola sbagliata lo fa
sembrare generato invece che scritto, proprio nel punto in cui deve essere
creduto. Adesso c'e' una tabella di come si scrivono — e i tre nomi di Google
Drive («Google Drive», «My Drive», «Il mio Drive») danno tutti «Google Drive»,
che e' quello che l'utente riconosce.

### Il conto delle tre serate

Tre volte di fila, il difetto vero l'ha trovato la stessa cosa: **chiamare il
programma da dove verra' chiamato davvero**, una volta, a mano. Il BOM,
l'`unwrap_or_default()`, e adesso la maiuscola e la funzione nel cassetto.

I banchi confrontano bene le traduzioni, e le traduzioni erano tutte corrette.
Ma un banco parla al codice nella lingua del banco, e i difetti stavano nel
punto in cui il codice incontra il mondo.

## 2 settembre 2026, notte fonda — il guardiano del vault, e una lezione che non si era spostata

Volevo portare `riservatezza.py` in Rust. Prima di replicare una logica ho
fatto la cosa che le ultime tre serate mi hanno insegnato: invece di chiedergli
se era d'accordo con se stesso, gli ho dato venti segreti di forma realistica e
gli ho chiesto di giudicarli.

Ne sono passati cinque.

### Il buco che fa piu' male

    Authorization: Bearer abcdef1234567890abcdef   ->  PASSA

E' **esattamente** il caso di D51. Quella decisione dice: «due implementazioni
che concordano non sono due implementazioni verificate», ed era nata proprio
da `Bearer`, mancante sia nel Python sia nel Rust dei **guasti**. Li' era stato
trovato e chiuso. Nel guardiano del vault no. Stessa forma, stesso buco, due
moduli diversi.

Quindi la decisione andava scritta piu' larga di com'era, e adesso c'e': **una
lezione imparata in un posto non si sposta da sola.** Quando si chiude un
difetto, va cercato a mano in tutti i moduli che fanno la stessa domanda. Non
c'e' un modo automatico: le prove di ciascuno passano, perche' ciascuno prova
se stesso.

### Gli altri quattro

**`seed phrase: abandon ability able about above absent`** passava. Il
controllo sulla densita' — quello che distingue «la password e' cambiata» da
«la password e' Tramonto2026» — scarta i valori fatti di lettere, e una seed
phrase e' per costruzione sei parole del vocabolario. Ed e' la cosa che non si
puo' cambiare dopo: una seed phrase rubata svuota un portafoglio, non c'e' un
«reimposta». Ora le chiavi «a parole» (passphrase, seed phrase, parola
d'ordine) hanno una regola loro.

**`otp 903214`** e **`PIN 4829`** passavano: il separatore era obbligatorio, e
li' fra la chiave e il valore non c'e' niente.

**`parola d ordine`** senza apostrofo passava. Non e' un refuso: NOVA si fa
dettare, e whisper l'apostrofo non sempre lo mette. Una regola che vale solo
per chi scrive protegge meta' degli utenti.

**E il piu' insidioso.** In «la password del wifi e Tramonto2026»
l'espressione trovava la coppia «password ... wifi» — parola comune, nessun
allarme — e si fermava li'. Il segreto due parole piu' in la' non veniva mai
guardato. `finditer` non rimedia, perche' le corrispondenze non si
sovrappongono: la prima **consuma la chiave**. Un segreto si nascondeva
mettendogli davanti una frase innocua, che e' quello che succede da solo
quando qualcuno incolla una riga di configurazione intera. Adesso si trovano
le chiavi e si guardano tutti i candidati che seguono.

Quarantuno casi, ventitre' da rifiutare e diciotto da lasciar passare: zero
sbagliati. I falsi allarmi contano quanto i buchi — un guardiano che blocca
meta' della conversazione viene spento, e allora non protegge piu' niente.

### E poi la stessa domanda, dall'altra parte

Se la lezione e' «cercala negli altri moduli», la si applica subito.
`guasti.senza_chiavi` maschera le chiavi nei messaggi d'errore. Gli ho dato lo
stesso corpus:

    AKIA1234567890ABCDEF                    IN CHIARO
    https://utente:segreto@example.com/x    IN CHIARO
    xoxb-1234567890-abcdefghijkl            IN CHIARO
    -----BEGIN RSA PRIVATE KEY-----         IN CHIARO
    carta 4111 1111 1111 1111               IN CHIARO

Il buco simmetrico, e piu' largo. `guasti` conosceva il `Bearer` e le chiavi
`sk-` — dove la cura era arrivata — e ignorava tutto il resto, che invece il
vault rifiutava da sempre. Un errore con dentro una chiave AWS finiva nel
giornale dei guasti cosi' com'era.

La cura non e' aggiungere le voci mancanti a tutti e due: sarebbero due
elenchi da tenere allineati a mano, e si disallineerebbero di nuovo — e' gia'
successo con gli eseguibili e con le cartelle sincronizzate. La cura e'
`nova/forme_riservate.py`: **la domanda si fa in un posto solo**, e sta li' e
non dentro `guasti` o dentro `kb` perche' lo usano tutti e due e non deve
pesare — importa `re` e nient'altro.

### Il confronto che mentiva, due volte

Prima cosa: mentre verificavo il Rust ho scritto una sonda che ha stampato
«divergenze: 0 su 14». Era falso. Il banco vuole un JSON solo e io gli mandavo
righe: l'uscita era vuota, e `zip` su una lista vuota non itera. **Zero
confronti si stampano come zero divergenze.** La sonda diceva la verita' su
niente. Ora c'e' un `assert` sul numero di risposte.

Seconda cosa, e piu' seria: `test_guasti_rust.py` era **verde** mentre le due
implementazioni divergevano su sei forme. Perche' il suo corpus conteneva solo
le quattro che conoscevano tutte e due. Un confronto vale quanto le domande
che fa, e questo ne faceva quattro.

Allargato il corpus, sono usciti due difetti veri, **uno per parte**: il
Python lasciava scoperta la firma del JWT (si fermava all'ultimo punto), il
Rust non copriva i numeri di carta. Nessuno dei due era «quello giusto».
Adesso, su sedici testi, zero segreti sopravvivono da nessuna delle due parti
— che e' il metro che questo file si era dato fin dall'inizio, e che nessuno
gli aveva ancora chiesto sul serio.

### La stessa domanda, negli altri tre posti

Se la lezione e' «cercala negli altri moduli», non ci si ferma al secondo. Ho
guardato chi altro scrive su disco testo che puo' contenere un segreto.

**Il giornale delle azioni** (`azioni.jsonl`, quello che si legge con
`--registro`) non mascherava niente. E fra i suoi chiamanti c'e' questo:

    annota(f"scritto in {selettore}", dove=dove, dettagli=testo)

`testo` e' cio' che NOVA **ha digitato in un campo**. NOVA sa compilare un
modulo di accesso — e' una cosa che deve saper fare — quindi prima o poi in
`dettagli` c'e' una password, in chiaro, in un file che resta.

**I risultati versati.** Quando un risultato supera i 24.000 caratteri,
`_versa` scrive il testo **intero** in `runtime/versati/`. Uno strumento legge
file e lancia comandi: un `.env`, l'uscita di `git config`, un `curl` con
l'intestazione dentro. Il modello quel testo l'ha gia' visto nel turno; quel
file invece resta, dentro la cartella del progetto — che, come sappiamo da
D52, puo' benissimo essere sincronizzata col cloud.

Tutti e due passano ora da `forme_riservate.maschera`. E il mascheramento sta
**dentro `annota`**, non nei quindici posti che la chiamano, per la stessa
ragione per cui il vault si chiude su `upsert`: la porta e' una sola, e un
chiamante che si dimentica non e' un'ipotesi, e' una certezza.

### Il caso che il filtro per forme non poteva prendere

    azione:   "scritto in #password"
    dettagli: "Tramonto2026!"

Guardati uno per volta non sono niente: il secondo e' una parola con dentro un
anno, e nessuna forma lo riconosce. Guardati insieme sono una credenziale. Il
filtro lavorava campo per campo e non poteva collegarli.

Ora `annota` chiede anche: **l'etichetta annuncia un segreto?** Se il nome del
campo o del posto contiene «password», «pin», «token», «credenziali» e simili,
il valore non si scrive affatto — resta la riga, che dice cosa e' successo e
dove, e sparisce il contenuto, che e' l'unica parte che non serve a nessuno
per rileggere la storia.

E non si esagera nell'altro verso: «scritto in #utente» con dentro
«giovanni.rossi» resta scritto. Un nome utente non e' una credenziale, e un
registro che copre tutto e' un registro che non si legge piu'.

Il registro in Rust non ha avuto bisogno di niente: `nova-registro` legge,
cerca e racconta, non scrive righe. Quando arrivera' la parte che scrive,
chiedera' a `forme_riservate` come fanno gia' le altre tre.

**Il conto della serata**: quattro posti dove un segreto poteva restare su
disco — il vault, il giornale dei guasti, il giornale delle azioni, i
risultati versati — e uno solo dei quattro se ne stava occupando.

### Sei, non quattro

Continuando a cercare ne sono usciti altri due, e uno e' la stessa lezione per
la terza volta.

**La frase del guasto.** `spiega` finisce cosi':

    messaggio = str(e).strip()
    return premessa + (messaggio if messaggio else "...")

`str(e)` cosi' com'e'. E il messaggio di un'eccezione porta volentieri il
valore che l'ha causata: `Incorrect API key provided: sk-...`. E' il caso
**vero** da cui era nata D29 — il fornitore che rimanda indietro la chiave
dentro il proprio errore — e allora si era chiuso il corpo delle risposte
HTTP. Questo ramo, che e' quello per cui passa tutto il resto, era rimasto
aperto. Terza volta della stessa lezione, in un modulo diverso: la prima nei
guasti, la seconda nel vault, la terza qui, a due funzioni di distanza dalla
prima.

Il rimedio non e' mascherare nei rami — sono quindici e crescono, e basta che
il prossimo dimentichi. Il corpo diventa `_spiega_grezzo` e `spiega` e' il
varco: `senza_chiavi(_spiega_grezzo(...))`. Vale per i rami di oggi e per
quelli che verranno.

Nello stesso file, `registra` scriveva la **traccia** intera senza filtro:
l'unico campo grosso di quel file a non passare dal mascheratore che il file
stesso possiede.

**Il giornale d'avvio.** Tre punti — `main`, `config`, il cervello di Claude —
scrivono in `avvio.log` una riga con dentro `argv`. E `argv[:3]` con `--ask`
e' **la domanda dell'utente per intero**. L'avevo gia' vista, quel file, mesi
fa: c'era dentro una lista di calciatori lunga venti righe. Se un giorno la
domanda comincia con «ricordati che la password del wifi e'...», resta li'.

Qui non ho mascherato: ho **omesso**. Il filtro prende le forme note e le
coppie etichettate, non una parola qualunque che per l'utente e' un segreto —
`Tramonto2026` da sola non somiglia a niente. E a quel log servono i **flag**,
non il contenuto: esiste per distinguere «il cervello non e' questo» da «il
cervello e' questo ma non passa di qui», e per quella domanda `--ask` basta e
avanza. Adesso scrive `--ask <49 caratteri>`, e `--brains` per intero.

Il modo sicuro di non scrivere una cosa e' non scriverla.

### Il conto vero

Sei posti dove un segreto poteva restare su disco o arrivare all'utente:

    il vault                      cinque forme non riconosciute
    il giornale dei guasti        sette forme non mascherate
    il giornale delle azioni      niente mascherato, e ci finisce
                                  cio' che NOVA digita nei campi
    i risultati versati           niente mascherato, file che resta
    la frase del guasto           str(e) grezzo, e va anche all'utente
    il giornale d'avvio           la domanda dell'utente per intero

Uno solo dei sei se ne stava occupando, ed era anche quello che sapeva meno.
Nessuno di questi era un difetto di codice sbagliato: erano sei posti che
facevano la stessa domanda senza sapere l'uno dell'altro.

## 3 settembre 2026 — il dodicesimo pezzo, e il formato come contratto

`nova-nodi`: cos'e' un nodo della memoria e che forma ha su disco. Viene
subito dopo il guardiano scritto stanotte, e nell'ordine giusto: prima si
decide cosa **puo'** entrare in memoria, poi si costruisce la porta.

Il vault e' una cartella di `.md` apribile in Obsidian. E' una scelta che si
paga in rigidita' del formato e si riprende tutta in fiducia — l'utente puo'
leggere e correggere a mano cio' che NOVA ricorda di lui — ma vuol dire anche
che **il formato e' un contratto**. Se le due meta' scrivono il frontmatter in
due modi, la prima che rilegge il file dell'altra perde dei campi in silenzio,
e non se ne accorge nessuno finche' non manca un tag.

Quindi il banco non confronta una funzione: confronta il **giro completo** —
scrivi, rileggi, riscrivi — e lo fa anche **incrociato**. Il Python deve
rileggere il file del Rust ricavandone lo stesso nodo che ricava dal proprio,
e viceversa. E' il caso che conta davvero: due meta' che si scambiano i file.

### «Œuvre», e perche' il mio era sbagliato

Prima divergenza:

    'Œuvre'   rust='oeuvre'   python='uvre'

Il Python fa `NFKD` e butta cio' che non e' ASCII. E `Œ` (U+0152) in Unicode
**non ha nessuna decomposizione**, nemmeno di compatibilita': NFKD la lascia
com'e', e il filtro successivo la trasforma in un trattino. Quindi «uvre».

Io nella tavola l'avevo mappata su «oe», ragionando che fosse la cosa giusta
da fare. Ed e' vero che `oeuvre` e' un nome di file migliore. Ma e' **il nome
sbagliato**, perche' il nome del file non e' una preferenza estetica: e' un
contratto gia' firmato con i file che stanno nel vault adesso. Cambiarlo li
rinomina tutti, e da quel momento lo stesso nodo esiste sotto due nomi — che
e' esattamente il guaio che la tavola degli accenti esiste per evitare
(«caffe» e «caffè» devono dare lo stesso file).

Portare non e' migliorare. Se una cosa va migliorata, si migliora **dopo**, in
tutte e due le meta' insieme, con un pensiero su cosa succede ai file gia'
scritti. Non di soppiatto, dentro un porting, perche' sembrava piu' bello.

Le legature invece ci restano — `ﬁ` (U+FB01) **ha** una decomposizione di
compatibilita', quindi NFKD la scioglie in «fi» e i due concordano. La
differenza fra i due casi non e' intuibile: e' andata guardata.

### Un carattere

Seconda divergenza: la confidenza `1.0`. Rust con `{}` stampa `1`, Python con
`str(float)` stampa `1.0`. Un carattere su un file di seicento, e i due testi
non coincidono piu'.

Non e' pedanteria: quel confronto carattere per carattere e' l'unico modo di
sapere che le due meta' scrivono lo stesso file. Se avessi confrontato «i
campi rileggendoli» invece dei byte, sarebbe passato — e sarebbe rimasto li'
finche' qualcuno non avesse fatto un `diff` su un vault toccato da tutti e
due, trovandosi ogni file modificato senza motivo.

### La data si passa da fuori

`to_markdown` in Python chiama `date.today()`. In Rust `oggi` e' un argomento,
come in `nova-calendario` e `nova-pianificazione`: una funzione che legge
l'orologio non si prova due volte con lo stesso risultato, e questo banco
fallirebbe a mezzanotte. E' la terza volta che quella scelta si ripaga.

Ventiquattro titoli, otto nodi, sette frontmatter e sette liste: tutto
identico, incrociati compresi.

## 3 settembre 2026 — il tredicesimo pezzo, e la prima volta che non trovo niente

`nova-nodi::fusione`. La parte di `Vault.upsert` che non tocca il disco: cosa
succede quando NOVA impara qualcosa su un fatto che **sa gia'**.

E' il posto piu' insidioso del vault, perche' un nodo peggiorato ha lo stesso
aspetto di un nodo giusto. Se la regola sbaglia, la memoria si degrada un po'
a ogni conversazione e nessuno se ne accorge — non c'e' un errore, non c'e' un
rosso, c'e' solo che fra sei mesi NOVA sa di te qualcosa di leggermente falso.

Le regole sono tutte difetti gia' successi, e i commenti del Python li chiamano
per nome:

- un «fatto» generico **declassava una persona**. L'estrattore mette sempre un
  tipo (di fabbrica «fatto»), quindi `nuovo.tipo or vecchio.tipo` sceglieva
  sempre il nuovo: la prima annotazione su Anna la trasformava in un fatto, il
  file restava in `02-persone` e l'indice diceva `06-fatti`;
- la confidenza saliva **a ogni riformulazione**, cioe' premiava esattamente
  il caso in cui NOVA non ha imparato niente di nuovo. Adesso sale quando lo
  stesso fatto torna **identico** da una seconda osservazione: confermato, non
  ridetto con altre parole;
- un primo paragrafo piu' lungo del tetto **congelava il nodo per sempre**:
  c'era spazio per la testa e per nient'altro, quindi ogni fatto nuovo veniva
  scartato in silenzio a ogni scrittura. Adesso anche la testa ha una quota, e
  il fatto piu' recente entra sempre, tagliato se serve;
- e l'osservazione automatica non declassa cio' che ha detto l'utente: chi
  l'ha detto conta piu' di quante volte e' stato detto.

### Trentun confronti, zero divergenze

E' la prima volta nel cantiere che un pezzo passa al primo colpo. Nessun
`unwrap_or_default()`, nessun BOM, nessuna maiuscola sbagliata, nessuna attesa
mia da correggere.

Vale la pena chiedersi perche', invece di prenderlo come un buon segno e
basta. Credo siano due cose. La prima: qui non c'e' nessun **giunto** — la
funzione non incontra PowerShell, non legge un file, non parla con un
processo. Tutti i difetti delle serate scorse stavano nel punto in cui il
codice incontra il mondo, e qui quel punto non esiste. La seconda: le regole
erano gia' state pagate. Ognuna di quelle quattro e' un difetto trovato mesi
fa, capito e scritto con il suo perche' accanto. Portare del codice che
qualcuno ha gia' sbagliato una volta e corretto capendo e' facile; portare del
codice mai messo alla prova e' dove si trovano le cose.

Quindi il porting qui non ha trovato difetti: ha trovato **conferme**. E ha
lasciato in Rust, accanto a ogni regola, il difetto che l'ha generata — che e'
l'unica forma di documentazione che non invecchia, perche' racconta un fatto
invece di un'intenzione.

### Due dettagli di traduzione

`len()` in Python conta i **caratteri**; in Rust conta i byte. Con un accento
dentro, i tagli del corpo sarebbero caduti in un punto diverso — e il caso «è
per tremila caratteri» sta fra le prove apposta.

E `max((a, b), key=...)` di Python, a parita' di peso, torna **il primo**: nel
Rust va scritto `>` e non `>=`, se no a parita' di origine vincerebbe il nuovo
e il nodo cambierebbe fonte senza motivo. Un carattere, di nuovo.

## 3 settembre 2026 — «non è partita», per la seconda volta

Gio: «non è spawnato nova all'avvio del pc». Stessa frase di qualche giorno
fa, e stessa causa — che e' la parte che fa piu' male, perche' vuol dire che
la volta scorsa avevo diagnosticato e non curato.

### Cosa ho misurato

La chiave di avvio automatico c'era e puntava a un file che esiste. Lanciata a
mano **esattamente come la lancia Windows** — cartella di lavoro `C:\`,
percorso di sviluppo — NOVA partiva e restava su. Quindi il meccanismo era
sano, e il difetto stava altrove.

Ho chiesto a Windows dov'erano le finestre dell'orb:

    orb "NOVA"  visibile=True   rect  -113,1119 - 20,1215

    \\.\DISPLAY2  primario=False  {X=-1920, Y=190,  1920x1080}
    \\.\DISPLAY1  primario=True   {X=0,     Y=0,    2048x1152}

DISPLAY2 va da x=-1920 a x=0. L'orb stava a x=-113: quasi tutto sul monitor
che Gio non vede, con venti pixel che sbordavano sul principale. **NOVA era
partita.** Semplicemente compariva dove non si puo' guardare.

### Il difetto era gia' scritto, accanto alla cura sbagliata

`richiama()` — la funzione della guardia di istanza singola, quella di ATT-14
— ha in cima questo commento, scritto da me tre giorni fa:

> Uno schermo **elencato** non e' uno schermo che **si vede**: spento, in
> standby, o con l'ingresso commutato altrove resta nell'elenco identico a uno
> acceso. Quella differenza dal software non si distingue, quindi non serve un
> controllo migliore: serve una via di ritorno.

Tutto giusto. E la via di ritorno l'avevo messa **solo** in `richiama()`, cioe'
solo se l'utente fa doppio clic una seconda volta sul collegamento. All'avvio,
`ripristina()` rimetteva l'orb nel posto salvato dopo aver controllato che
quel posto fosse «su uno degli schermi che ci sono adesso» — e lo era.

E' D72 un'altra volta, a due funzioni di distanza: **una lezione imparata in
un posto non si sposta da sola**, nemmeno dentro lo stesso file, nemmeno
quando l'ho scritta io tre giorni prima.

### La regola, e perche' non e' prepotente

All'avvio si onora il posto salvato **solo se sta sul principale**. Altrimenti
l'orb torna nell'angolo in basso a destra.

Sembra una prepotenza — sposta una finestra che qualcuno aveva messo apposta
da un'altra parte — e la ragione per cui non lo e' sta tutta nel momento.
All'avvio l'utente non ha chiesto niente: ha acceso il computer. L'orb e'
l'unica cosa che dice che NOVA c'e', e se compare su uno schermo che non si
vede la conclusione non e' «sara' sull'altro monitor», e' **«non e' partita»**.

E l'accensione e' anche il momento peggiore per fidarsi dell'elenco degli
schermi, perche' un secondo monitor puo' non essersi ancora svegliato quando
l'avvio automatico fa partire il programma.

Il conto fra i due errori non e' pari: sbagliare in un verso costa una
trascinata, e chi sposta l'orb se lo ritrova li' per tutta la sessione.
Sbagliare nell'altro costa la fiducia di qualcuno che accende il PC e non
trova il programma che aveva installato.

La decisione sta in `posto_all_avvio`, che e' **pura**: prende il posto
salvato e il rettangolo del principale e torna dove posare. Cosi' si prova
senza una finestra vera, e fra le prove c'e' il caso misurato — x=-113,
y=1119 — con accanto il numero, non una descrizione.

Verificato dal vivo, lanciando come lancia Windows: l'orb si posa a
**1864,954**, angolo in basso a destra del principale.

### Due cose viste per strada

**`cli_nova()` cercava solo in `core/target/release`.** E' la cartella in cui i
binari li produce cargo, cioe' quella che esiste sulla macchina di chi
sviluppa e su nessun'altra: chi installa da una release ha i binari in `bin\`,
quindi per lui quel controllo falliva **sempre** e il pannello diceva per
sempre «il client del demone non e' compilato» — falso, e incomprensibile per
chi non ha mai compilato niente. E' lo stesso difetto che ha fatto nascere
`binari.json`. `demone.rs` cercava gia' `novad` accanto all'eseguibile; questa
riga non era stata portata dietro. Adesso guarda prima accanto a se'.

**E `nova shutdown` non ferma sempre il demone.** Due volte oggi `novad` e'
sopravvissuto allo spegnimento e la compilazione si e' rifiutata di partire —
correttamente, perche' la guardia di `build.ps1` funziona. Non l'ho inseguito
adesso, ma va guardato: uno spegnimento che non spegne e' una promessa non
mantenuta, e si scopre solo quando qualcosa d'altro si lamenta.


## 3 settembre 2026, sera — la cura giusta per la diagnosi sbagliata

Va scritto perche' e' un errore mio, ed e' del tipo peggiore: non ho sbagliato
un calcolo, ho sbagliato **a chiamare misura una deduzione**.

Stamattina, su «non e' spawnato nova all'avvio del pc», avevo misurato l'orb a
x=-113, su DISPLAY2. Da li' ho concluso che DISPLAY2 fosse uno schermo che Gio
non vede — spento, o con l'ingresso commutato — e ho cambiato la regola: **se
il posto salvato non e' sul principale, all'avvio l'orb torna nell'angolo**.

La cosa ha funzionato: verificata dal vivo, l'orb si posava a 1864,954, sullo
schermo principale. Ho scritto che era risolto.

La prima cosa che ha detto Gio dopo: «mi e' riapparso in posizione sbagliata».

DISPLAY2 lo vede benissimo. L'orb ce lo tiene **apposta**, e la mia regola
gliela spostava a ogni accensione. Avevo curato il sintomo con l'ipotesi
sbagliata, e il prezzo l'ha pagato la cosa che volevo proteggere: la sua
finestra, dove l'aveva messa lui.

### Cosa avrei dovuto fare

Chiedere. Erano due righe: «l'orb sta sul monitor di sinistra — quello lo
vedi?». La differenza fra «uno schermo che non si vede» e «uno schermo che
l'utente usa» non e' misurabile dal software — l'avevo perfino **scritto**, tre
giorni fa, nel commento di `richiama()`. Ho riletto quella frase, ci ho
costruito sopra, e non mi sono accorto che diceva anche il contrario: se dal
software non si distingue, allora non lo sa nemmeno chi scrive il software.

Ho anche provato a correggere una seconda volta senza fermarmi — regola piu'
morbida, «si onora il posto se cade su uno qualunque degli schermi attaccati»
— e nemmeno quella si comportava come mi aspettavo, perche' Tauri lavora in
pixel **fisici** mentre le misure che facevo da PowerShell erano in pixel
**logici**: su un monitor al 125% sono numeri diversi per lo stesso punto, e
ho passato tre giri a confrontare mele con pere.

A quel punto la cosa giusta non era una terza versione. Era **rimettere tutto
com'era** e chiedere.

### Cos'e' rimasto

Tutto ripristinato: `finestre.rs` torna al comportamento di prima, l'orb sta a
-141,1107 dove Gio lo tiene, e `orb.json` lo conserva.

Resta la correzione di `cli_nova()`, che non c'entrava niente con le finestre
ed era giusta: cercava il client del demone solo in `core/target/release`,
cioe' dove i binari li produce cargo — la cartella che esiste sulla macchina di
chi sviluppa e su nessun'altra. Chi installa da una release ha i binari in
`bin\`, e si sentiva dire per sempre «il client del demone non e' compilato».

E resta la domanda vera, che adesso e' di nuovo aperta e stavolta la faccio a
chi sa la risposta: **all'accensione, cosa succede davvero?** Perche' se
DISPLAY2 si vede, l'orb a x=-113 Gio lo avrebbe visto — a meno che
all'accensione quel monitor non ci sia ancora, e la finestra nasca in un posto
che nel giro di qualche secondo non esiste piu'.

### La lezione, che e' diversa da tutte le altre di questa settimana

Le altre erano «guarda il risultato, non l'accordo», «chiama la cosa da dove
verra' chiamata». Sono tutte forme di misurare meglio.

Questa e' un'altra: **ci sono domande che nessuna misura risponde**, perche' il
fatto non e' nel computer. Se lo schermo si vede o no lo sa una persona sola, e
non e' quella che scrive il codice. Trattare una deduzione come un dato
misurato e' peggio che non misurare, perche' arriva con la stessa faccia.

## 3 settembre 2026, notte — sessantacinque secondi

Il registro c'era da sempre. `runtime/guscio.log`, nella cartella del
progetto: 275 righe, una per ogni avvio dal 30 agosto. Io avevo cercato in
`%APPDATA%\NOVA\logs`, non l'avevo trovato, e avevo concluso che non ci
fosse **niente** — e da li' avevo cominciato a dedurre.

Dentro c'era la risposta, scritta da NOVA stessa, con l'ora:

    2026-09-03T08:17:15.341Z  INFO nova_shell: guscio in avvio versione="0.1.0"
    2026-09-03T08:17:15.640Z  INFO nova_shell::finestre: orb rimesso dov'era x=-104 y=1129
    2026-09-03T08:17:15.993Z  INFO nova_shell::demone: demone avviato dal guscio
    2026-09-03T08:17:15.993Z  INFO nova_shell: demone acceso all'avvio

Il fuso e' UTC+2, quindi sono le **10:17:15 locali**. Il PC si e' acceso alle
10:16:10.

**NOVA e' partita. Sessantacinque secondi dopo l'accensione.**

E non per lentezza sua: fra la prima riga e l'ultima passano **0,65 secondi**.
E' Windows che ritarda apposta i programmi della chiave `Run`, per far
comparire prima il desktop.

Per chi guarda lo schermo, un minuto di niente non e' un ritardo: e' «non e'
partita». Che e' precisamente quello che Gio ha detto, ed e' il difetto
peggiore che possa avere un programma il cui compito e' **stare li' ad
aspettarti**.

### La cura: come parte, non dove si mette

Un'attivita' pianificata con innesco «all'accesso» non passa da quel ritardo.
Non serve essere amministratori — e' un'attivita' dell'utente per l'utente — e
il disinstallatore toglie gia' le attivita' che cominciano per NOVA, quindi
quella strada era **pulita prima di essere presa**.

`install.ps1` adesso registra l'attivita' e toglie la vecchia voce in `Run`
(se restassero tutte e due, NOVA partirebbe due volte: la seconda si chiude da
sola grazie alla guardia di istanza singola, ma e' comunque un processo
avviato per niente). Resta un ripiego sulla chiave `Run` per le installazioni
dove una policy ha disattivato l'Utilita' di pianificazione — e in quel caso
lo **dice**, invece di lasciar credere che sia tutto uguale.

### Cosa mi porto via, che e' piu' importante della cura

Stamattina, sulla stessa domanda, avevo dedotto che un monitor fosse spento e
avevo cambiato il comportamento delle finestre. Sbagliato, ritirato in serata.

La differenza fra le due giornate non e' che la seconda volta ho ragionato
meglio. E' che la seconda volta **ho trovato il file**.

Avevo perfino scritto io, tre giorni fa, il commento in cima ad
`avvia_registro()`:

> Ed e' esattamente li' che serve leggerlo — quando NOVA parte da sola
> all'accensione del PC e qualcosa non va, non c'e' nessuno a guardare uno
> schermo. Un avvio che fallisce in silenzio e' un avvio che non si puo'
> riparare.

Il registro l'avevo messo apposta per questa esatta domanda, e poi ho passato
mezza giornata a dedurre invece di aprirlo. **Cercare in un posto e non
trovare non e' «non c'e'»**: e' «non c'e' li'». La distanza fra le due frasi
e' una regola sbagliata, una finestra spostata a un utente che la teneva dove
voleva lui, e due commit da ritirare.

Quando qualcosa non torna, la prima domanda non e' «cosa sara' successo». E'
**«chi lo ha scritto da qualche parte?»** — e nel dubbio, chiederlo al
programma prima che a se stessi.

---

## 3 settembre 2026 — Dove vive un nodo, e un file che ho cancellato io

Quattordicesimo pezzo del cantiere: `nova-nodi::posto`. La cartella per tipo,
il percorso relativo, e lo slug che non calpesta un nodo incompatibile.

### La regola che tiene insieme la memoria

Il nome del file porta il tipo davanti: `persona-anna`, non `anna`. La ragione
e' che due cose diverse possono chiamarsi uguale — la persona Anna e il
progetto Anna sono due nodi — e senza prefisso il secondo scriverebbe sopra il
primo.

Poi c'e' la parte delicata. Se anche col prefisso il posto e' occupato, si
aggiunge un numero **solo se il tipo e' incompatibile**. Incompatibile, non
«diverso»: un fatto e una persona convivono nello stesso nodo, e il fatto
confluisce nella persona. E' esattamente cio' che deve succedere quando NOVA
impara qualcosa di nuovo su Anna.

Se li' ci fosse un `!=`, ogni annotazione automatica creerebbe
`persona-anna-2`, `persona-anna-3`, e la memoria si sbriciolerebbe in copie
che non si parlano. Nessun errore, nessun log: il file viene scritto lo
stesso. Percio' il banco, oltre a chiedere «le due meta' sono d'accordo?»,
chiede in chiaro «un fatto su Anna finisce dentro Anna?».

Trentacinque confronti, zero divergenze. Le due meta' vanno d'accordo perche'
la regola, in Python, era gia' scritta bene.

### L'unico difetto trovato, e stava nel banco

Il banco distingue le domande con un campo chiamato `tipo`. La domanda nuova
aveva bisogno del **tipo del nodo**, e il nome ovvio era di nuovo `tipo`. Due
campi con lo stesso nome: serde non protesta, sceglie, e il confronto avviene
su una domanda diversa da quella che credevo di aver fatto. Adesso si chiama
`tipo_nodo`, con il commento che dice perche'.

### E poi ho cancellato un file di prova

Volevo aggiungere in coda a `test_nodi_rust.py` la sezione nuova. Ho scritto
`open(p, "w", newline="\\n")` — con la sequenza sbagliata per l'argomento.
Python solleva `ValueError: illegal newline value`, e sembra un errore
innocuo: la scrittura non e' avvenuta.

Non e' innocuo. Il file viene **aperto in scrittura prima** che l'argomento
venga validato, e aprire in scrittura tronca. L'eccezione e' arrivata a
sedicimila caratteri gia' persi. Me ne sono accorto solo perche' il passo
successivo non trovava piu' niente da cercare dentro.

Recuperato da git in dieci secondi, perche' era committato. Se fosse stato il
lavoro dell'ultima ora, no.

La regola che mi porto via: **si scrive su un file temporaneo e poi si
sposta**. Un `os.replace` o riesce del tutto o non fa niente; una `open(...,
"w")` che fallisce puo' aver gia' distrutto il contenuto. Vale ancora di piu'
per NOVA che per me, perche' NOVA scrive nei file dell'utente — e la stessa
identica trappola, un giorno, sarebbe una nota di Obsidian invece che un mio
test.

C'e' anche una seconda lezione, piu' scomoda. Sul disco dell'utente `rm` non
mi e' permesso, ed e' una protezione che ho sempre trovato giusta. Ma la
scrittura si', e una scrittura sbagliata cancella lo stesso. Il divieto sulla
cancellazione non e' una rete: e' un solo filo.

---

## 3 settembre 2026, sera — Duemilaseicento megabyte che non c'erano

`test_schede.py` falliva. Non sotto carico, non a intermittenza: da solo, con
un numero.

    DXGI 15341 MiB, nvidia-smi 12699 MiB, scarto +2643
    [NO ] lo scarto sta dentro la riserva (900 MiB + 4%)  riserva 1407

Il commento sopra quella riga l'avevo scritto io giorni fa, e diceva gia'
tutto: «DXGI e' il piu' ottimista dei due, ed e' la direzione pericolosa: si
sopravvaluta e si mettono troppi layer. Il margine deve coprire lo scarto, o
la riserva e' una cifra scritta a caso.»

Era una cifra scritta a caso. Non perche' fosse sbagliata: perche' era **una
cifra**, e lo scarto non e' una costante.

### Perche' oggi e non ieri

Sul PC girava League of Legends, piu' trenta finestre fra Edge, Discord e il
resto. `nvidia-smi` diceva 3.411 MiB occupati; DXGI ne vedeva circa mille.

Il budget di DXGI non e' «quanto e' libero»: e' **«quanto il sistema sarebbe
disposto a darti»**, contando di poter sfrattare chi non sta disegnando
adesso. E' una risposta onesta a una domanda diversa dalla mia.

Lo scarto quindi non scala con la scheda — scala con **quanto stanno usando
gli altri**. Una riserva fissa non puo' coprirlo per costruzione: il giorno
che uno apre un gioco, la riserva e' vecchia.

### Cosa costava davvero

Non un numero storto in un log. Ho chiesto al codice vero, col modello
configurato (Qwen3.8-27B Q4_K_M, 15,66 GB):

    15341 MiB liberi -> 55 strati
    12699 MiB liberi -> 44 strati

Undici strati di troppo, circa due gigabyte e mezzo che sulla scheda non ci
stanno. E non fallisce: il driver li mette in memoria condivisa, e il modello
continua a rispondere — dieci volte piu' piano. E' esattamente il guasto per
cui `gpu.rs` esiste, arrivato dalla porta di servizio.

### La cura, e perche' non e' un ritorno a nvidia-smi

DXGI resta la risposta per tutti. Dove c'e' `nvml.dll` — che e' una DLL del
driver, non un programma da lanciare e di cui leggere il testo — si prende il
**minore** dei due numeri.

Il rifiuto di `nvidia-smi`, scritto in cima al modulo, era verso un eseguibile
esterno che su una Radeon non esiste e fa tornare zero: chi aveva una scheda
AMD non la usava e non gli veniva detto. Qui non torna niente di quello. NVML
non e' la fonte: e' un secondo parere che puo' solo abbassare, e solo dove
c'e'. Il tetto sulla memoria dedicata resta, perche' e' quello che impedisce
di credere all'integrata che dichiara quindici gigabyte prendendoli in
prestito dalla RAM.

Misurato subito dopo: `nova-schede` dice 12.724, `nvidia-smi` 12.725. Un MiB
di arrotondamento.

Due limiti, scritti nel codice invece che scoperti dopo. Con **due** schede
NVIDIA non si corregge niente: il numero e' di una delle due e non si sa
quale, e accoppiarle vorrebbe dire attraversare la struttura PCI di NVML, che
ha dentro due buffer di caratteri a dimensione fissa — sbagliarne uno di un
byte vuol dire farsi scrivere nello stack dal driver. E la DLL, una volta
caricata, resta: scaricare una libreria del driver che puo' aver lasciato
thread dietro di se' e' il genere di pulizia che costa un crash.

### Il giro dei posti che fanno la stessa domanda (D72)

- `nova/runtime.py` chiede al binario e tiene `nvidia-smi` come ripiego. Il
  ripiego chiede `memory.free`, cioe' **era gia' piu' onesto del primo**.
- `install.ps1` chiede `memory.total`: quanta scheda c'e', per scegliere che
  modello scaricare. Domanda diversa, non tocca.
- Il ramo Linux legge da sysfs, che il totale lo da' e il libero no. Resta
  come sta: non ho una macchina per provarlo, e indovinare qui e' come
  indovinare un monitor spento.

### Una conferma arrivata da sola

A fine giornata la prova sulla scheda e' tornata verde senza che toccassi
niente:

    DXGI 15341 MiB, nvidia-smi 14855 MiB, scarto +486
    [ok ] lo scarto sta dentro la riserva

Il gioco era stato chiuso. Lo scarto e' passato da 2.643 a 486, cioe' e'
rientrato nella riserva esattamente come il giorno in cui l'avevo dichiarata
adeguata — a macchina scarica.

Non c'e' misura piu' netta di questa: la riserva non era sbagliata di poco,
era **misurata nella condizione sbagliata**. Se avessi guardato solo oggi
pomeriggio, o solo stasera, avrei concluso due cose opposte, e tutte e due con
un numero in mano.

### Quello che resta aperto, e non decido io

Il test **continua a fallire sul PC di Gio**, ed e' giusto cosi'. Legge
`bin\\nova-schede.exe`, cioe' la copia *installata*, che viene da una release
firmata e verificata con SHA256 — non da `core\\target`. La correzione e' nel
sorgente; arrivera' li' col prossimo pacchetto, o subito se Gio vuole che gli
aggiorni i binari installati a mano. Sovrascrivere di nascosto dei file di cui
l'installatore controlla l'impronta e' un modo di rompere le cose che poi non
si capiscono piu'.

---

## 3 settembre 2026, notte — La stessa domanda, due ricordi diversi

Stavo per portare in Rust il retrieval — BM25, la fusione RRF, l'espansione
sul grafo. Prima di scrivere una riga ho riletto il Python per capire cosa
avrei dovuto far combaciare, e mi sono fermato su tre righe che sembrano
identiche:

    ordinati = sorted(punteggi.items(), key=lambda kv: kv[1], reverse=True)
    candidati.sort(key=lambda kv: kv[1], reverse=True)
    hits.sort(key=lambda h: h.score, reverse=True)

Tre ordinamenti **stabili**: a parita' di punteggio vince chi e' arrivato
prima. E allora la domanda diventa: chi e' arrivato prima?

L'ordine risale a `set(tokenizza(query))` e a `postings[t]`, che sono due
insiemi di stringhe. Python randomizza l'hash delle stringhe **a ogni
processo**. Quindi i pari merito venivano ordinati in un modo diverso a ogni
avvio di NOVA.

### Non e' teoria: e' il vault di Gio

Ho preso il vocabolario vero dei 139 nodi, ho costruito 444 domande con le
sue parole e le ho fatte in due processi diversi:

    domande diverse fra due processi: 11 su 444
      di cui cambia proprio CHI viene ricordato: 3

Una delle tre e' **«progetto»**. Cioe' la domanda piu' naturale che si possa
fare a questa memoria.

    A: [...] progetto-restauratore, progetto-unrealagent, progetto-passaporti
    B: [...] progetto-restauratore, progetto-passaporti,  progetto-chess-ai

Stesso vault, stessa domanda, stesso codice, un secondo di distanza. Un
progetto entra nel contesto e l'altro no, e nessuno dei due e' sbagliato:
sono a pari merito. Semplicemente il taglio a `top_k` butta via l'ultimo, e
chi sia l'ultimo lo decideva il seme dell'hash.

### La cura, che e' una virgola

A parita' di punteggio decide lo **slug**. Tre chiavi di ordinamento da
`score` a `(-score, slug)`. Zero divergenze su 444 domande, dopo.

Lo slug e' un criterio arbitrario, e lo dico apertamente: a parita' esatta
qualcuno deve vincere. E' arbitrario ma **stabile e leggibile** — chi apre
l'audit capisce perche' quel nodo e' entrato.

Resta una domanda che non tocca a me: a parita' di punteggio, non sarebbe
meglio ricordare il nodo **piu' recente**? I nodi crescono per accodamento, e
uno toccato ieri e' quasi sempre piu' vivo di uno fermo da mesi. Ma quella non
e' una correzione, e' una scelta su come deve ricordare NOVA, e la lascio a
Gio. Quello che ho corretto e' solo che la stessa domanda dia la stessa
risposta.

### La prova che non si poteva scrivere da dentro

Questo difetto **non si vede da un processo solo**: l'ordine dei pari merito
e' fisso per tutta la vita del processo. Una prova normale sarebbe passata col
difetto dentro, felice.

Percio' la prova ne apre quattro, con quattro `PYTHONHASHSEED` diversi, e
confronta le classifiche. Ho rimesso il difetto per vederla fallire — quattro
classifiche diverse su quattro semi — e poi l'ho tolto. Una prova che non ho
visto fallire non e' una prova, e' una decorazione.

C'e' anche una lezione sul cantiere. Questo non l'ha trovato il banco: il
banco confronta due implementazioni, e qui l'implementazione era una sola. **L'ha
trovato il doverla spiegare a un linguaggio che non ha i dizionari ordinati.**
Rust mi avrebbe costretto a scegliere un ordine esplicito, e la domanda «quale
ordine?» non ha risposta finche' non ci si accorge che in Python non ce n'era
nessuno.

---

## 3 settembre 2026, tarda notte — Ho scritto un pezzo che c'era gia'

Ho passato la sera a scrivere `nova-ricerca`: BM25, fusione RRF, taglio del
corpo. Banco, sedici confronti, verdi al primo colpo — 120 dei quali sulle
domande vere del vault, punteggi confrontati fino al miliardesimo.

Poi la suite completa ha segnalato `test_memoria_rust.py` rosso, e la ragione
era piu' semplice e piu' brutta di qualunque bug: **quel codice esisteva
gia'**. `nova-memoria` — il secondo pezzo del cantiere, scritto settimane fa —
e' esattamente BM25 piu' la fusione.

Avevo guardato il Python per capire cosa restasse da portare. Non ho guardato
il Rust. In un progetto la cui unica ragione e' avere **una** risposta in
**un** posto, avevo appena scritto la seconda — e ci sarei riuscito, se la
suite non avesse avuto una prova che le confrontava tutte e due col Python.

E' D88 in un vestito nuovo. La', avevo cercato un file in una cartella, non
l'avevo trovato, e avevo concluso «non c'e'». Qui non ho proprio cercato:
sapevo cosa mancava al Python e ho dato per scontato di sapere cosa c'era nel
Rust. Prima di scrivere un pezzo nuovo, la domanda non e' «cosa manca la'»,
e' **«cosa c'e' gia' qui»**.

### Cosa ho tenuto

`nova-ricerca` cancellato. Dentro `nova-memoria` e' finito quello che
aggiungeva davvero, ed era parecchio:

- Le mappe da `HashMap`/`HashSet` a `BTreeMap`/`BTreeSet`. Non e' pignoleria:
  la somma dei contributi BM25 e' in virgola mobile e non e' associativa, e
  scorrere in ordine di tabella hash vuol dire sommarli in ordine diverso.
- Lo **spareggio per slug dentro `rrf`**, che e' il difetto vero della
  giornata (D94).
- Il taglio testa-e-coda del corpo, che nel Rust non c'era.
- Il banco piu' largo: 45 confronti invece di 35, con dentro le 120 domande
  del vault vero.

### La riga che nascondeva il difetto

Il banco di `nova-memoria` aveva questo commento, scritto da me settimane fa:

> Punteggio decrescente, poi slug: senza il secondo criterio due nodi a pari
> merito uscirebbero in ordine di tabella hash, che cambia a ogni esecuzione
> e renderebbe il confronto una lotteria.

Avevo visto il problema. L'avevo risolto **nel banco**, riordinando in uscita,
invece che nella libreria. Il confronto tornava pulito e la lotteria restava
esattamente dov'era, dentro il codice che va in produzione.

E' un modo di sbagliare che non sembra sbagliato mentre lo si fa: la prova
diventa verde, e diventa verde per la ragione giusta — quel riordino *serve*,
perche' un banco non deve dipendere dall'ordine di cio' che prova. Solo che
avevo sistemato il termometro e non la febbre.


### Due cose imparate prima di accorgermene

Le ho imparate mentre chiudevo il pezzo sbagliato, e valgono lo stesso.

#### Una prova verde non l'ho creduta, e ho fatto bene a non crederle

Sedici su sedici al primo colpo. Ma il banco dei guasti, giorni fa, era verde
mentre le due meta' divergevano su sei forme: il corpus conteneva solo cio'
che sapevano tutte e due.

Percio' ho rotto il Rust apposta, col difetto piu' plausibile che ci sia qui —
lo spazio in coda alla ripetizione dei tag, che in Python e' documentato come
un errore gia' fatto: senza, l'ultimo tag della prima copia si fonde col primo
della seconda in un termine inesistente (`betaalfa`).

Cinque controlli su sedici sono diventati rossi, e uno diceva questo:

    'progetto': [..., idea-progetto-compact-come-catalogo, ...]
        vs      [..., progetto-sluted-015, ...]

Un carattere di differenza in una funzione che costruisce un testo, e la
memoria ricorda un altro progetto. Adesso so che il banco misura qualcosa.

#### La copia che ha mentito

Rimesso a posto il file e ricostruito, il banco falliva ancora. Cargo aveva
detto «Finished in 0.15s»: non aveva ricompilato niente.

`Copy-Item` in PowerShell **conserva la data di modifica dell'originale**.
Cargo decide cosa ricompilare guardando le date. Il sorgente era giusto, il
binario era ancora quello rotto, e la prova stava misurando un programma che
non esisteva piu' da nessuna parte.

E' la stessa famiglia di D88 — «cercare in un posto e non trovare non e' non
c'e'» — in versione peggiore: qui avevo guardato nel posto giusto e mi ero
fidato di una risposta vecchia. Se non avessi avuto in mano un risultato che
*mi aspettavo diverso*, avrei concluso che il ripristino non aveva funzionato
e sarei andato a cercare il guasto nel codice.

La regola: un binario ricostruito che si comporta come prima non e' un
mistero, e' un binario che non e' stato ricostruito. Guardare la riga di
cargo prima di guardare il proprio codice.

---

## 4 settembre 2026 — A parita' vince la freschezza

Gio ha deciso la domanda che avevo lasciato aperta: a parita' esatta di
punteggio, ricorda il nodo **piu' fresco**.

E' la scelta giusta e vale la pena scrivere perche'. I nodi di questa memoria
crescono per accodamento: NOVA impara qualcosa su Anna e lo aggiunge al nodo
di Anna. Un nodo toccato ieri e' quindi quasi sempre piu' vivo di uno fermo da
mesi — non piu' importante, piu' **attuale**. E fra due ricordi ugualmente
pertinenti, l'attualita' e' l'unica cosa che li distingua davvero. Lo slug era
stabile e leggibile, ma non voleva dire niente.

### Dove va messo, che non e' dove sembra

Non nell'ordinamento finale. L'RRF trasforma la **posizione** in punteggio:
due nodi a pari merito escono di li' con punteggi gia' diversi, e un criterio
applicato piu' a valle non troverebbe piu' nessun pareggio da sciogliere.
Sarebbe stata una regola scritta, ricordata nella documentazione, e mai
eseguita.

Percio' lo spareggio sta in una funzione sola — `in_ordine` — che decide
punteggio, poi data, poi slug, e che viene chiamata in tutti e tre i punti in
cui il recupero mette qualcosa in fila. Prima erano tre `sort` scritti a mano
in tre posti, ed e' esattamente cosi' che una regola si applica in due punti
su tre senza che nessuno se ne accorga (D72).

### Un limite che e' meglio dire

`aggiornato` ha la granularita' del **giorno**, non dell'ora. Nello stesso
giorno il pareggio resta intatto, e li' decide ancora lo slug. Con
quarantacinque nodi seminati tutti il 21 agosto, vuol dire che fra loro la
freschezza non dirime niente. Non l'ho cambiato: mettere l'ora dentro
`aggiornato` vuol dire cambiare il frontmatter di ogni file del vault, che e'
un contratto con i file che ci stanno adesso — la stessa ragione per cui non
ho allargato l'insieme delle lettere accentate (D96).

### Misurato, non dedotto

Sul vault vero cambia in tre domande su quattro di quelle che erano instabili
ieri:

    'progetto'  slug: ... progetto-chess-ai, progetto-passaporti
                data: ... progetto-passaporti, progetto-chess-ai

`progetto-passaporti` e' stato toccato piu' di recente, ed e' quello che entra
nel contesto. Prima ci entrava chi veniva prima in ordine alfabetico; prima
ancora, chi capitava.

---

## 4 settembre 2026 — CANT-1, e una lezione mia trovata addosso all'utente

Fatta la lista del cantiere — otto pezzi, sigla `CANT-`, ordinati per quanto
ciascuno avvicina il momento in cui sul PC non serve piu' Python — e aperto il
primo: il vault su disco.

Non ho scritto una riga di Rust per le prime due ore, e sono state le due ore
piu' utili.

### La lezione di stamattina, addosso alle note di Gio

Leggendo `Vault.upsert` per capire cosa dovevo portare, ho trovato questo:

    percorso.write_text(node.to_markdown(), encoding="utf-8")

E' **esattamente** il difetto che stamattina mi ero fatto addosso da solo
(D90): aprire in scrittura tronca, e fra il tronca e lo scrivi c'e' una
finestra in cui il file esiste ed e' vuoto. Io ci avevo perso sedicimila
caratteri di un mio file di prova, ripresi da git in dieci secondi.

Qui non e' un mio file di prova. E' **la memoria di chi usa NOVA**, e ci passa
`MemoryWriter` da un thread di sfondo dopo quasi ogni scambio. Una chiusura a
meta', un errore, il PC spento nel momento sbagliato, e resta una nota vuota —
che alla ricerca dopo c'e' ancora, ma non dice piu' niente. Nessun errore da
nessuna parte. E se qualcuno se ne accorgesse, penserebbe di aver perso la
nota chissa' quando.

Stamattina avevo scritto nel diario che «il divieto sulla cancellazione non e'
una rete: e' un solo filo», perche' sul disco dell'utente `rm` non mi e'
permesso ma la scrittura si'. L'avevo scritto come una riflessione. Era una
segnalazione, e non l'ho seguita fino in fondo lo stesso giorno.

### Il giro dei posti (D72), che stavolta era lungo

`ricette.salva` scriveva **gia'** di fianco e poi rinominava, col commento
giusto:

> un'interruzione a meta' lascerebbe un JSON troncato, cioe' tutte le
> procedure perse insieme.

Lo faceva solo lui. `pianificazione._salva` salva la stessa forma di archivio
— tutti i promemoria in un file solo — e scriveva dritto. E cosi' il vault,
gli strumenti file, l'harness, la configurazione (che puo' contenere una
chiave API).

Adesso c'e' `nova/scrittura.py`, e chi scrive un file che conta chiama quello:
temporaneo di fianco, `fsync`, `os.replace`. Il temporaneo finisce per
`.parte-<pid>`, quindi non e' un `.md` e chi legge il vault non lo vede
nemmeno mentre esiste — una regola che c'era gia' e che non ho dovuto
aggiungere.

La prova non immagina l'interruzione: **la esegue**. Fallisce apposta a meta'
del versamento e guarda cosa e' rimasto sul disco. E nello stesso file
dimostra che il modo vecchio la nota la perdeva davvero — ventuno caratteri
rimasti su sessantasei. Una prova che non ho visto fallire non e' una prova.

### Poi il Rust: il disco dietro un tratto

`nova-nodi::deposito`. La parte difficile non e' leggere un file: e' la
macchina a stati che decide chi ricaricare, chi dimenticare, e cosa fare
quando due file con lo stesso nome in due cartelle si contendono uno slug.

Quella macchina si prova con un **disco finto**, in memoria: undici casi in un
millisecondo, ripetibili. Con un disco vero se ne proverebbero tre e si
spererebbe. E il banco fa la cosa che rende onesto il disco finto: gira lo
stesso scenario due volte, il Python su una cartella vera e il Rust sulla
finta, e pretende che dicano la stessa cosa.

Il caso che tengo piu' caro e' questo: due `doppio.md` in due cartelle, poi
uno sparisce. Il superstite deve **tornare visibile** — altrimenti chi aveva
perso la contesa resta invisibile pur essendo rimasto l'unico, ed e' di nuovo
un nodo che sparisce senza che nessuno lo dica.

E l'identita' di un file, qui, e' il percorso relativo normalizzato: niente
`resolve()`. Il Python lo chiama ancora, ed e' proprio cio' che D56 dice di
non fare — la domanda e' «e' questo file, dentro questa cartella?», e a quella
si risponde coi nomi.

### Il banco ha trovato un difetto di settimane fa

Undici scenari su dodici combaciavano. Il dodicesimo, un file scritto a mano
senza frontmatter:

    rust:   'Progetto Nova'
    python: 'Progetto nova'

`capitalize()` di Python fa **due** cose — alza la prima lettera e abbassa
tutte le altre — e il Rust non ne faceva nessuna. Il difetto stava li' da
settimane, in `da_markdown`, che e' il dodicesimo pezzo del cantiere.

Perche' il banco dei nodi non l'aveva visto: ogni nodo del suo corpus aveva
gia' un `title:` nel frontmatter, e in quel ramo non entrava nessuno. E' la
terza volta che incontro questa forma — il banco dei guasti verde mentre le
due meta' divergevano su sei forme, il banco della memoria che nascondeva il
disordine riordinando in uscita, e adesso questo. **Un banco prova solo le
strade che il suo corpus percorre**, e le strade che non percorre non sono
«coperte per simmetria»: sono scoperte.

E c'era di peggio. Una prova diceva:

    controlla("un nodo senza titolo prende il nome dal file",
              ... ["title"] == "il mio nodo")

Quella stringa non l'aveva detta il Python: era **quello che il Rust faceva**.
L'avevo scritta io, in una sezione intitolata «le cose che devono essere vere
comunque», e aveva certificato come regola del formato il difetto di una delle
due meta'. Adesso il titolo di ripiego si chiede al Python, su cinque nomi
diversi. Una prova che confronta un'implementazione con se stessa non prova
niente, e in piu' **difende** cio' che dovrebbe scoprire.

---

## 4 settembre 2026, sera — CANT-1: la porta del vault, e una prova che passava per coincidenza

Seconda meta' di CANT-1: scrivere. `Deposito::salva` e' l'unica porta da cui
si entra in memoria, e fa nell'ordine le stesse cose del Python — chiede al
guardiano, cerca chi c'e' gia' per slug e poi per somiglianza, rilegge dal
disco perche' l'utente potrebbe averlo appena corretto in Obsidian, controlla
i tipi, fonde, e scrive dove il file gia' sta.

Undici scenari, confrontati **file per file, contenuto compreso**, con il
Python su una cartella vera e il Rust sul disco finto. Zero divergenze.

### Due tratti invece di una funzione

`DiscoScrivibile` esiste separato da `Disco` per una ragione sola: mettere
nel contratto che **la scrittura non puo' restare a meta'**. Non e' un
dettaglio di chi implementa, e' la lezione di stamattina scritta dove non si
puo' dimenticare.

`Guardiano` e' un tratto e non una funzione perche' `salva` deve
*chiederglielo*. In Python il controllo sui segreti sta dentro `upsert` per lo
stesso motivo: e' l'unica porta, ci passano l'apprendimento automatico, le
note a mano e il seeding, e chiudere una porta sola vuol dire chiuderla
davvero. Se il controllo stesse nel giudizio di chi chiama, basterebbe un
chiamante distratto.

Il guardiano vero non e' ancora portato — e' un pezzo suo, tutto espressioni
regolari, e **e' la cosa che meno di tutte va fatta a meta'**: un guardiano
che si dimentica una forma e' peggio di nessun guardiano, perche' da' l'idea
di proteggere. Intanto il tratto c'e', e chi chiama deve passargliene uno.

### `dict.fromkeys` non e' `sort`

Rinominando gli archi dopo una fusione avevo scritto `sort()` e `dedup()`. Il
Python usa `dict.fromkeys`, che toglie i doppioni **conservando l'ordine**.

Sembra la stessa cosa e non lo e': l'ordine delle relazioni e' quello in cui
sono nate, sta scritto nel frontmatter, e riordinarlo alfabeticamente
riscriverebbe il file di ogni nodo collegato con una modifica che nessuno ha
chiesto — che l'utente si vedrebbe comparire in Obsidian come se qualcuno gli
avesse toccato le note. L'ho corretto leggendo il Python, non aspettando il
banco.

### La prova che passava per coincidenza

`to_markdown` legge l'orologio, e la data finisce nel frontmatter. Nel banco
avevo fissato la data a `2026-09-04` e bloccato l'orologio del Python. Tutto
verde.

Poi mi sono accorto che **oggi e' il 2026-09-04**. Il confronto sarebbe
passato identico anche se il blocco dell'orologio non avesse funzionato per
niente, e me ne sarei accorto domani — o mai, se domani avessi guardato altro.

L'ho verificato spostando la data al 2020, ed e' andato bene: il blocco
funziona. Ma la lezione non e' «funzionava»: e' che per venti minuti ho avuto
in mano una prova verde che non sapevo cosa stesse provando. La data ora e'
lontana da oggi apposta, col commento che dice perche'.

E' la stessa forma di D104 — un banco prova solo le strade che il suo corpus
percorre — con una variante peggiore: qui la strada la percorreva, ma il
valore atteso e quello reale coincidevano per un motivo che non c'entrava
niente col codice.

### Il banco discrimina

Verificato rompendo il Rust apposta, col difetto che il commento del Python
descrive per nome: tolto il controllo sui tipi dal ramo diretto.

    rust:   ['02-persone/marco.md']
    python: ['02-persone/marco.md', '03-progetti/progetto-marco.md']

La persona Marco e il progetto Marco nello stesso file, con un tipo solo e
nella cartella sbagliata. Il banco l'ha visto.

---

## 4 settembre 2026, notte — CANT-1 chiuso

Il vault, tutto: leggere, accorgersi dei cambiamenti, scrivere, archiviare,
riattivare, contare, generare l'indice, tenere il registro. Il disco vero
dietro il tratto, e il guardiano dei segreti attaccato.

Quattro cose che valeva la pena fare in un modo invece che in un altro.

### Il guardiano: una lista sola, due mani diverse

Il pezzo che avevo lasciato per ultimo apposta, perche' e' quello che meno di
tutti va fatto a meta'.

`chiavi` maschera i segreti nei messaggi d'errore, il guardiano impedisce che
entrino in memoria. Sembra la stessa domanda, e **le forme sono le stesse** —
per questo stanno in un elenco solo, che e' tutta la lezione di D73. Ma il
costo dello sbaglio no:

- coprire di troppo in un registro costa una parola illeggibile;
- rifiutare di troppo costa **un ricordo che NOVA non avra' mai**, e l'utente
  non capisce nemmeno perche'.

Quindi la stessa tabella porta due soglie: si maschera con la mano larga —
otto caratteri dopo il prefisso, per tutti — e si rifiuta con la mano ferma,
la soglia vera del fornitore: sedici per una chiave OpenAI, venti per un token
GitHub, dieci per uno Slack. Prima era un numero solo per tutti e due gli usi,
e andava bene solo perche' nessuno lo usava per rifiutare.

Il banco non chiede «siete d'accordo?» — su questo le due meta' si erano gia'
trovate d'accordo nello sbagliare (D51). Chiede due cose:

    e' rimasto fuori qualcosa che doveva essere rifiutato?
    e' stato rifiutato qualcosa che NOVA doveva poter ricordare?

Venti forme che devono essere fermate, dieci ricordi legittimi che devono
passare, da tutte e due le parti. E una terza domanda: che il rifiuto non
ripeta mai cio' che ha rifiutato, perche' finisce in un registro.

Verificato rompendo il Rust: tolta la scansione «chiave poi valore», sei casi
su venti passano — password, pin, bearer, parola d'ordine. Il banco l'ha
visto.

### Il guardiano attaccato, non solo disponibile

`Guardiano` e' un tratto perche' `salva` deve chiederglielo. Ma un tratto con
nessuna implementazione vera e' una promessa: c'e' una prova che verifica che
il guardiano **vero** sia attaccato al vault — rifiuta una password e lascia
passare «Gio lavora meglio la mattina presto». Senza quella, la porta poteva
restare aperta sembrando chiusa.

### Gli orfani in ordine

`statistiche()` elenca i nodi che nessuno nomina, tagliati a venti. Il Python
li dava in ordine di inserimento — cioe' l'ordine in cui i file erano stati
letti o i nodi salvati — e siccome la lista e' tagliata, quell'ordine
incidentale **decideva quali orfani si vedono**. Stessa forma di D94, in
piccolo. Adesso in ordine alfabetico da tutte e due le parti.

### Quello che il disco vero rifiuta

`Cartella::scrivi` rifiuta ogni percorso con un `..` dentro. Non e' paranoia
astratta: un percorso arriva da uno slug, uno slug arriva da un titolo, e un
titolo puo' arrivare da un testo che NOVA ha letto da qualche parte. Non c'e'
nessun caso legittimo in cui un nodo debba scrivere fuori dalla cartella
dell'utente.

E l'elenco non segue i collegamenti simbolici: seguirli vorrebbe dire leggere
come nodo un file che sta fuori dal vault, e poi **riscriverlo li'**.

### Cosa non e' portato, e si vede

Il registro accoda una riga; a comporla e' chi chiama, perche' cosa vada in un
registro non lo decide chi tiene i file. E `nova-nodi` adesso dipende da
`nova-guasti` — una direzione sola, perche' `nova-guasti` non deve sapere che
esiste un vault.

---

## 4 settembre 2026, tarda notte — CANT-2 comincia dalle parole, non dalle mani

Sessanta strumenti in quindici file: le mani di NOVA sul PC. Ma il primo pezzo
da portare non e' quello che **fanno** — e' quello che **sono**.

Uno strumento e' tre cose. La dichiarazione (nome, descrizione, parametri,
rischio), la guardia (posso farlo senza chiedere?), e il fare. La
dichiarazione e' quella che pesa di piu' e sembra la piu' innocua: sono 26.573
caratteri di schema JSON — circa 7.600 token, il 42% del contesto in certe
configurazioni — che il modello rilegge a ogni richiesta, e su cui **sceglie
quale strumento usare**.

Quindi una parola diversa in una descrizione e' un comportamento diverso, e
non c'e' nessun tipo che se ne accorga. Lo so per averlo gia' fatto: avevo
«rafforzato» il testo di `kb_note` e l'avevo peggiorato da 3 su 4 a 1 su 6.

### Estratte, non ricopiate

Sessanta descrizioni trascritte a mano sarebbero sessanta occasioni di
cambiare un carattere che il modello legge. Le ho **estratte dal registro
vivo** con uno script, che sta nel repository perche' il metodo vale quanto il
risultato.

Ma un estrattore che lavora con espressioni regolari sul codice sorgente di
sessanta lambda e' comodo e inaffidabile. Percio' si verifica da solo: rende
ogni modello di anteprima con piu' insiemi di argomenti e lo confronta con
l'anteprima vera. Cosi' ha scoperto da solo che quattro f-string su piu' righe
le stava prendendo a meta' — e il risultato era un'anteprima piu' corta,
plausibile, e sbagliata.

### E la verifica aveva lo stesso buco che cercava

`automazione_crea` e' passata lo stesso. La sua anteprima continua su una
seconda riga, il modello estratto si fermava alla prima, e la verifica non se
n'e' accorta perche' costruiva gli argomenti di prova **dai campi che il
modello nominava** — cioe' da cio' che stava provando. Un modello a cui manca
un campo non puo' essere smentito da un corpus ricavato dal modello stesso.

E' D104 in miniatura, dentro lo strumento scritto per evitare D104. Adesso i
campi si prendono dai parametri dichiarati dello strumento, e
`automazione_crea` viene scartata come deve.

Quarantasei modelli verificati, quattordici anteprime scritte a mano — quelle
con un «se» vero dentro. Il linguaggio dei modelli ha quattro forme e si
ferma li': uno che cresce a forza di casi speciali smette di essere una
semplificazione e diventa un secondo linguaggio da imparare.

### Il banco, e cosa ha trovato

Confronto carattere per carattere sullo schema. Ha trovato subito una
divergenza che nessuna prova sui tipi avrebbe visto: in Python `items` sta
**prima** di `description`, perche' e' l'ordine in cui e' scritta la
dichiarazione; io lo mettevo dopo. Stesso JSON per un parser, due prompt
diversi per un modello.

Verificato che il banco morda cambiando **una parola** in una descrizione
Rust: rosso al carattere 136.

### Una differenza dichiarata invece che scoperta

Se il modello manda un booleano dove serve una stringa, il Python inciampa
dentro l'anteprima e `describe_call` ripiega sulla riga generica; il Rust non
ha niente che possa fallire e la riga la scrive lo stesso. Su cinquanta casi
storti il Python ripiega tre volte.

Riprodurre l'inciampo vorrebbe dire simulare gli errori di tipo di Python, che
e' assurdo. L'ho scritto nel banco come differenza voluta, con il motivo:
nessuna delle due mente all'utente, e quella del Rust dice qualcosa in piu'.
Una differenza dichiarata e' una decisione; la stessa differenza trovata fra
sei mesi sarebbe un difetto.

---

## 4 settembre 2026, tarda sera — Le guardie, che erano sbagliate in tre modi insieme

Secondo pezzo di CANT-2: non cosa gli strumenti fanno, ma cosa gli e'
permesso. Tre domande — dove si puo' scrivere, quali comandi non si eseguono
mai, quando ci si ferma a chiedere.

La prima era sbagliata in tre modi tutti insieme, e tutti e tre erano **la
stessa lezione che avevo gia' scritto** (D56).

    p = str(Path(path).resolve()).lower()
    for prot in self.cfg.safety.protected_paths:
        pl = str(Path(prot)).lower()
        if p == pl or p.startswith(pl + "\\"):

1. Il percorso passa da `resolve()`, i protetti no: due spazi diversi. Sotto
   un punto di reinnesto — un pacchetto MSIX, una cartella reindirizzata su
   OneDrive — la protezione smette di funzionare **in silenzio**.
2. La barra rovescia e' scritta a mano: fuori da Windows quel confronto non
   scatta mai.
3. E per le cartelle autorizzate il confronto e' **senza separatore**.

Il terzo non l'ho dedotto, l'ho dimostrato con due cartelle e cinque righe:

    write_roots = [ .../dati ]
    dati/mio.txt          -> PERMESSO
    dati-altrui/tuo.txt   -> PERMESSO

Autorizzare una cartella ne autorizzava un'altra che le somigliava soltanto.
E' esattamente la frase con cui finisce D56 — «con il separatore in fondo, se
no `NOVA-vecchio` sta dentro `NOVA`» — scritta il 2 settembre, in un modulo
diverso, e mai arrivata qui.

La funzione giusta esisteva gia', in `dati.py`, col commento giusto. Adesso
sta in `nova/percorsi.py` e la usano tutti e due (D72).

### Il ripiego prudente che nascondeva un errore

Portando l'autonomia in Rust ho scritto `"ask_all"`. La costante vera e'
`"always_ask"`.

Il ripiego, che avevo messo apposta perche' «una configurazione illeggibile
non deve diventare fai pure», mandava quel nome su
`ChiediSeRischioso` — cioe' **chi aveva chiesto «conferma sempre» otteneva
«conferma solo se rischioso»**, senza un errore da nessuna parte.

Un ripiego indulgente maschera un valore sbagliato, e piu' e' ragionevole
meglio lo maschera. Adesso `capisci()` torna `None` per un nome che non
conosce, `dal_nome()` ripiega come prima, e c'e' una prova che pretende che i
tre nomi veri si capiscano **tutti e tre** — cosi' un nome sbagliato non puo'
piu' passare per una scelta prudente. La prova nel banco prende i nomi dalla
configurazione invece di riscriverli: riscriverli era proprio l'errore.

### Un divieto che sparisce e' peggio di un divieto che non c'e'

I comandi vietati sono espressioni regolari configurabili. Il Python fa
`except re.error: continue`: un motivo che non compila svanisce, e chi
l'aveva scritto crede di essere protetto.

In Rust il motore e' un altro e la sintassi non coincide del tutto — un
lookahead che Python accetta, `regex` lo rifiuta — quindi il problema qui e'
piu' probabile, non meno. `Guardie` tiene l'elenco dei motivi che non ha
capito e lo espone: chi costruisce le guardie puo' dirlo. Un buco dichiarato
si tappa; un buco silenzioso si scopre dopo.

### Una punteggiatura che e' un contratto

Il messaggio «scrittura non consentita fuori dalle cartelle autorizzate»
elenca le cartelle col `repr` di Python, che **raddoppia le barre**:
`'C:\\dati'` invece di `'C:\dati'`. E' un artefatto che trapela in un
messaggio che leggono il modello e, attraverso lui, l'utente.

L'ho riprodotto invece di pulirlo. Un messaggio d'errore e' un contratto
quanto un formato di file, e due meta' che dicono la stessa cosa con due
punteggiature diverse sono due voci. Se un giorno si pulisce, si pulisce da
tutte e due le parti insieme.

### E poi la stessa domanda al contrario

Chiuso il buco del separatore, ne restava uno all'opposto: se il confronto e'
**solo** sui nomi, un collegamento lo aggira.

Misurato, non immaginato. Una giunzione vera:

    mklink /J scorciatoia protetta

    \protetta\x.txt      -> bloccato
    \scorciatoia\x.txt   -> PERMESSO   (risolve in \protetta\x.txt)

Quindi le due domande **non sono la stessa**, e per un pelo:

- «se cancello questa cartella sparisce anche questo file?» si risponde sui
  nomi (D56), perche' chi cancella una cartella cancella i nomi che ci stanno
  sotto;
- «questa scrittura sta toccando un posto protetto?» no.

In una guardia si sbaglia verso il no, quindi vale l'**unione**: sta dentro se
lo dice il nome *o* se lo dice la destinazione. Per i percorsi protetti
l'unione protegge di piu'. Per le cartelle autorizzate rifiuta di meno — e
anche quello e' il verso giusto, perche' il caso vero e' l'opposto: sotto un
pacchetto MSIX `resolve()` porta i file fuori dalla loro stessa cartella, e
con la sola destinazione NOVA non potrebbe scrivere nemmeno in casa propria.

Dalla parte Rust la destinazione e' un **parametro obbligatorio** di
`puo_scrivere`, e non per comodita': un parametro che si puo' dimenticare e'
un parametro che si dimentica. Chi non ha modo di risolvere passa `None` e
resta la difesa sui nomi, che e' meno, ed e' meglio di una dimenticanza.

La prova Python fa una giunzione vera e la usa. Una prova che se la immagina
non prova niente — ed e' esattamente il difetto che si scopre quando qualcuno
ci ha gia' scritto dentro.

### Il formato, che sembra cosmesi

Ultimo pezzo della giornata: come si racconta un file al modello. L'ordine di
un elenco, la misura di un file, quante righe entrano in una lettura, come si
scrive una riga trovata da una ricerca.

Sembra cosmesi. Non lo e': quel testo e' cio' su cui il modello decide il
passo dopo, e una misura scritta in un altro modo o un troncamento a un
carattere diverso sono un contesto diverso.

Il punto piu' sottile e' l'arrotondamento. `f"{size:.0f}"` in Python
arrotonda **al pari**: 2560 byte sono «2 KB», non «3 KB». Non mi sono fidato
che il formattatore di Rust facesse la stessa cosa per la stessa strada — l'ho
scritto esplicito — e ho verificato che il banco lo veda, cambiandolo in
«mezzo sempre in su»:

    2560:       rust '3 KB' vs python '2 KB'
    2684354560: rust '3 GB' vs python '2 GB'

Settantotto misure, scelte attorno ai punti dove si cambia unita' e dove
l'arrotondamento decide. Un banco che le avesse provate a caso avrebbe potuto
non incontrarne nemmeno una: i valori che separano due implementazioni sono
pochi e stanno tutti sui bordi.

### Dove si ferma CANT-2

Le dichiarazioni, le guardie e il formato sono portati. Restano i **corpi**:
leggere davvero un file, elencare i processi, catturare lo schermo, chiamare
un indirizzo.

E' li' che CANT-2 smette di essere una traduzione. Un corpo non si confronta
col Python su un banco — si confronta col **sistema operativo**, e chiede a
`nova-platform` di crescere. E' lo stesso genere di punto di CANT-8: non una
difficolta' tecnica, una decisione. Meglio prenderla da svegli.

---

## 4 settembre 2026, tarda sera — I corpi: trentuno operazioni su una cartella vera

Preso il punto che avevo lasciato aperto: i **corpi** degli strumenti, quelli
che il disco lo toccano davvero. Tredici, la famiglia dei file.

Il banco qui non confronta funzioni. Costruisce **due cartelle identiche fino
ai byte**, esegue la stessa sequenza di trentuno operazioni — elenca, leggi,
scrivi, accoda, modifica, crea, copia, sposta, cancella, cerca, setaccia — una
col Python e una col Rust, e confronta due cose:

- quello che il modello leggerebbe, riga per riga;
- e alla fine, **le due cartelle**, file per file, byte per byte.

La seconda e' quella che conta davvero (D51): due implementazioni possono
raccontare la stessa cosa e lasciare due dischi diversi.

Una sola divergenza su trentuno, e la parola l'avevo inventata io: «Cartella
pronta» invece di «Cartella creata». Sembra niente. Non lo e': quella riga la
legge il modello, e «pronta» e «creata» non rispondono alla stessa domanda —
una dice che c'era gia', l'altra che l'ha fatta adesso.

### Tre cose che ho deciso invece di tradurre

**Le guardie sono il primo argomento di ogni corpo che scrive.** Non un
parametro opzionale, non un controllo dentro: un permesso che si puo'
dimenticare si dimentica, e in questo modulo non si puo' scrivere uno
strumento che scrive senza aver chiesto.

**Il fuso arriva da fuori.** La data di un file compare in ogni elenco, e una
funzione che si legge il fuso da sola cambierebbe risposta a marzo e a ottobre
senza che nessuna prova se ne accorga. E' la stessa scelta di `oggi` nel
vault. La conversione da istante a data l'ho messa in `nova-calendario`, dove
c'era gia' la direzione inversa: una seconda aritmetica del calendario, anche
corretta, e' una seconda da tenere allineata.

**Il Cestino e l'apertura stanno dietro un tratto**, e chi non li ha lo dice
invece di fallire in silenzio. Sono le uniche due cose di tutto il modulo che
cambiano da sistema a sistema — il resto e' `std::fs`.

### Il ramo che nessuno prova mai

`read_file` in Python ripiega su cp1252 quando un file non e' UTF-8. Sui PC
italiani quei file ci sono — un `.txt` salvato dal Blocco note dieci anni fa —
e senza quel ripiego NOVA direbbe «non e' un file di testo» di un file di
testo.

L'ho portato, tabella di conversione compresa, coi cinque byte che in cp1252
**non esistono**: un file che li contiene non e' cp1252, e leggerlo comunque
vorrebbe dire inventare dei caratteri.

Verificato che il banco lo veda, togliendolo:

    python '{B}/vecchio.txt (righe 1-1 di 1):\ncitta' perche''
    rust   "{B}/vecchio.txt non e' un file di testo (13 byte)."

Il corpus ha un file cp1252 dentro apposta. Senza, il ramo sarebbe stato
verde e vuoto — D104 di nuovo.

### E un difetto trovato leggendo

`delete_path` aveva **due copie** della strada per il Cestino: una in
`_nel_cestino`, con tanto di commento sul perche', e una scritta inline dentro
la funzione. Gia' divergenti: quella inline ripiegava solo su `ImportError`,
l'altra su qualunque errore. Due copie della stessa cosa nello stesso file,
scritte a venti righe di distanza.

### Una prova che avevo scritto a occhio

Nel provare la conversione delle date avevo scritto i valori attesi a mente:
`2026-09-05 22:10`. Erano sbagliati di un'ora e venti. Il vero era 23:30, e
me l'ha detto il Python quando gliel'ho chiesto.

Un valore atteso che si inventa non prova che il codice e' giusto: prova che
si credeva di sapere la risposta. Adesso nel commento c'e' scritto da dove
viene.

### La shell, e un limite che ho scritto invece di tapparlo

Il racconto di un comando: codice di uscita, cio' che ha detto, cio' di cui si
e' lamentato. Nove esiti confrontati, e uno vale per tutti — «(nessun
output)», perche' **un comando muto e un comando riuscito non sono la stessa
cosa** e chi legge deve poterli distinguere.

`run_python` non passa dalla guardia dei comandi. Non l'ho «sistemato»: i
motivi vietati sono espressioni regolari pensate per il testo di un comando di
shell, e applicarle a del codice Python darebbe falsi allarmi — «diskpart»
dentro una stringa — e mancherebbe comunque quelli veri, perche' quel codice
puo' lanciare qualunque cosa per vie che nessuna regola sul testo intercetta.
La difesa li' e' il rischio dichiarato: `run_python` e' Pericoloso, e sotto
autonomia normale si ferma a chiedere.

Un controllo che sembra proteggere e non protegge e' peggio di nessun
controllo, perche' chi lo vede smette di guardare. Sta scritto in cima al
modulo.

### I tasti, dove sbagliare non da' errore

`press_keys` traduce «ctrl+shift+esc» nella forma che capisce Windows. E' il
pezzo piu' pericoloso della famiglia, e non perche' sia difficile: una
traduzione sbagliata **non fallisce**, preme altri tasti — e li preme nella
finestra che ha il fuoco, cioe' quella dove l'utente sta lavorando.

Diciassette combinazioni confrontate, e quattro che devono **rifiutarsi**:
`ctrl`, `ctrl+alt`, la stringa vuota, un `+` da solo. Una combinazione fatta
di soli modificatori non e' una combinazione, e mandarla vorrebbe dire premere
qualcosa a caso addosso a chi sta scrivendo.

### Un fuso non e' un numero

Qui il banco ha trovato un difetto di progetto, non di traduzione.

Avevo fatto passare il fuso come **un** intero — una scelta che sembrava
coerente con tutte le altre («il mondo arriva da fuori»). Il banco ha
confrontato quattro istanti, e due sono usciti sbagliati di un'ora:

    0:          rust 'giovedi 01/01/1970 02:00:00'
                python 'giovedi 01/01/1970 01:00:00'

Perche' avevo calcolato il fuso **oggi**, che e' settembre, e l'avevo
applicato a gennaio. Un fuso cambia due volte l'anno: un file modificato a
gennaio ed elencato a luglio esce con un'ora sbagliata, e nessuno se ne
accorge finche' non guarda due volte lo stesso file in due stagioni.

Adesso il fuso e' un **tratto** — `secondi_in(istante)` — e non un numero.
Non e' pignoleria di tipi: un numero si puo' passare senza pensarci, un
tratto obbliga chi chiama a rispondere alla domanda «quando?». E' la stessa
forma della destinazione obbligatoria in `puo_scrivere` (D118).

### «Cosa dice questa pagina» lo chiedevano in due

`cerca._testo` per le pagine lette col browser, `tools/web._clean` per quelle
scaricate. Due funzioni per la stessa domanda, e — come tutti gli elenchi
separati — sapevano cose diverse:

- una toglieva `<svg>` e `<head>`, l'altra `<template>`;
- solo una schiacciava lo **spazio unificatore**, quindi l'altra lasciava nel
  contesto del modello un carattere che sembra uno spazio e non lo e';
- e si spartivano le righe vuote con due regole diverse.

Nessuna delle differenze era voluta: erano due funzioni scritte in due
momenti. Adesso c'e' `nova/html_a_testo.py`, che e' l'unione — ogni regola che
una delle due aveva serviva a qualcosa.

### Il corpus con dentro la trappola

Nel banco ho messo otto entita' — `&oelig;`, `&thorn;`, `&notin;`, `&brvbar;`
e altre — **sapendo** che la mia tabella scritta a mano non le conosceva.

Non e' un dispetto: e' l'unico modo di sapere dove finisce una tabella. Il
banco ha risposto:

    python 'oe y th d ... non in'
    rust   '&oelig; &yuml; &thorn; &eth; &curren; &brvbar; &not; &notin;'

Ventisette pagine su ventotto identiche, e la ventottesima era quella che
avevo messo per farla fallire. **Vedere dove finisce una tabella e' meglio che
scoprirlo** — scoprirlo vuol dire che un giorno il modello legge `&copy;` in
mezzo a una frase e non sa cosa farne.

Quindi ho smesso di scriverla a mano: duemilacentoventicinque entita',
estratte dal dizionario di Python come le dichiarazioni degli strumenti
(D112).

E anche l'estrazione ha avuto il suo inciampo. La prima versione ne prendeva
**106**, perche' scartavo i nomi che finiscono col punto e virgola — e nel
dizionario di Python quasi tutti stanno solo in quella forma. Centosei e' un
numero abbastanza plausibile da non far sospettare niente: se non avessi
saputo che le entita' HTML sono migliaia, l'avrei accettato.

### La regola che con duemila nomi comincia a contare

Con una tabella di sessanta voci, «il nome piu' lungo che combacia» era una
finezza. Con duemila e' necessaria: `&notin` non e' `&not` seguito da «in», e
`&sub` non e' l'inizio di `&subseteq`. Il lettore prende sempre il piu' lungo.

### E un numero che non tornava per un centesimo

Ultimo pezzo della giornata: come si racconta un ricordo al modello. Sembra il
piu' innocuo di tutti, ed e' quello dove un dettaglio taciuto cambia cosa il
modello **crede**: un nodo senza la confidenza e' un nodo trattato come una
certezza, e un corpo tagliato senza dirlo fa credere di aver letto tutto.

Sei ricordi confrontati, e uno non tornava:

    confidenza 0.955  ->  python 0.95, rust 0.96

Non e' un arrotondamento diverso: e' che `0.955` in binario e' **poco meno**
di 0.955 — `0.95499999999999996` — e Python formatta il valore vero, che sta
sotto la meta'. Io moltiplicavo per cento, e la moltiplicazione porta il
prodotto esattamente a `95.5`, cioe' proprio sulla meta'.

L'errore non era nell'arrotondamento. Era nella moltiplicazione, che il
formattatore non fa.

La cosa interessante e' che qui la scelta giusta e' **l'opposta** di quella
che avevo fatto poche ore prima per la misura di un file, dove il conto a mano
va benissimo: li' i numeri sono interi divisi per potenze di 1024, che in
binario sono esatti, e non c'e' nessun errore da introdurre. Le due scelte
sembrano incoerenti e non lo sono — dipende da che numeri passano di li'. Sta
scritto in tutti e due i posti, perche' chi legge uno solo dei due penserebbe
che l'altro e' sbagliato.

### Gli appunti che erano di Windows, e un guasto vecchio venuto a galla

Gio ha corretto la rotta, e la correzione e' architettonica: «più che pezzi di
windows devono essere pezzi di nova. Deve essere il più indipendente possibile
da windows, quasi windows gli si deve poggiare sopra». E' D130.

Non e' una questione di stile. Contati, i punti dove PowerShell non e' uno
strumento che NOVA usa ma cio' che la regge sono **quattordici**: gli appunti
*sono* `Get-Clipboard`, il volume *e'* `SendKeys`, la cattura dello schermo
*e'* `Add-Type System.Drawing`. Dove non c'e' PowerShell, quelle capacita' non
esistono affatto.

Il primo pezzo rifatto sono gli appunti, perche' sono i piu' piccoli e si
possono provare per davvero: `nova-platform` che chiama Win32 diretto —
`OpenClipboard`, `GetClipboardData`, `SetClipboardData` — e un binario
`nova-appunti` che NOVA preferisce, col ripiego PowerShell che resta ma
**dichiarato**. Costo: 179 ms contro 19, e di quei 19 quasi tutti sono l'avvio
del processo; la chiamata al sistema e' microsecondi.

Ma la cosa che conta di piu' e' venuta fuori di traverso. Mettendo le due
strade una accanto all'altra per verificare che scrivessero negli appunti
*veri* — quelli di sistema, non una copia nostra — la verifica falliva:

    scritto      : "perché città però — «virgolette» e un'emoji 😀"
    letto diretto: "perché città però — «virgolette» e un'emoji 😀"  uguale: True
    letto da _ps : "perch? citt? per? - ?virgolette? e un'emoji ??"  uguale: False

Il mio primo sospetto era la mia stessa riga di comando, e infatti la prima
misura era contaminata: gli accenti si rompevano prima ancora di arrivare agli
appunti. Ho rifatto la misura da un file, cosi' il trasporto non poteva
contaminare il risultato. Il guasto e' rimasto.

Non erano gli appunti. Era `_ps`: PowerShell scrive su stdout con la tabella
codici della console, e noi leggevamo UTF-8. Il guasto e' vecchio quanto la
funzione — chi non ha costruito il binario passa ancora di li' — e nessuno se
n'era accorto perche' non solleva niente: nessun errore, uscita zero, solo il
testo storpiato. Per un utente italiano, quasi ogni riga.

Si ripara con una riga, e la riga va messa **dentro `_ps`** perche' di li'
passano tutte e quattordici le capacita'. E la prova che lo tiene fermo non
guarda la strada nuova: guarda quella **vecchia**, perche' e' li' che il
guasto stava.

Resta il fatto che l'ho trovato per caso, e vale la pena dirlo: l'ho trovato
perche' D130 mi ha costretto a mettere due implementazioni una accanto
all'altra e a chiedere se dicessero la stessa cosa. Non stavo cercando questo.

### Il volume, e un ripiego che rispondeva a un'altra domanda

Secondo pezzo di D130, e il peggiore dei quattordici. Per mettere il volume a
meta', il ripiego manda cinquanta pressioni simulate del tasto «volume giu'»
per arrivare a zero, poi venticinque di «volume su» — mezzo volume per
pressione — con il timeout a novanta secondi. Poi risponde:

    Volume impostato a circa 50%.

«Circa» non e' modestia: e' che non ha letto niente. Non c'e' modo di leggere
il volume a colpi di tasto, e se una pressione si perde nessuno se ne accorge.

Ma il guasto serio e' il muto. Lo strumento dichiara `mute: true` = silenzia.
Il ripiego manda il tasto «muto» di Windows, che **inverte**. Chi chiede
«silenzia» con l'audio gia' silenzioso se lo ritrova acceso — cioe' ottiene
l'opposto di quello che ha chiesto, e la risposta gli dice che e' andata bene.
E' la stessa famiglia di `ask_all`/`always_ask`: un ripiego che sembra
prudente e risponde a un'altra domanda (D132).

Adesso il volume si chiede a Core Audio, che e' chi lo tiene: si legge, si
imposta, si rilegge. Il muto si **imposta** — chiederlo due volte lo lascia
muto — e il livello sopravvive al muto, come deve. Il binario si chiama
`nova-volume` e sono spariti anche `pycaw` e `comtypes`, due pacchetti Python
che stavano nei requisiti solo per questo. La prova che conta e' la piu'
stupida: chiedere due volte la stessa cosa e guardare se la seconda torna
indietro.

E ho chiuso il nodo che D130 aveva lasciato aperto: i tratti erano dichiarati
e non li implementava nessuno. `nova-core/src/caps_sistema.rs` e' l'unico
posto in cui e' scritto «su questa macchina, chi sa fare cosa» —
`nova-strumenti` continua a non conoscere `nova-platform`, e `nova-platform`
continua a non sapere per chi lavora.

### Una prova rossa a caso, e una lezione che non si era mossa

Facendo girare la suite Rust dopo il volume, una prova e' diventata rossa:
`cio_che_finisce_in_tempo_non_viene_toccato`, che verifica una cosa banale —
un lavoro che finisce subito non deve risultare interrotto.

Non era rotto niente. La generazione dell'interruzione e' globale, come deve
essere: c'e' un solo NOVA e un solo pulsante «ferma». Ma cargo fa girare le
prove in parallelo, e la prova accanto chiama `ferma()` di proposito: se
capita mentre questa sta fra il suo `gettone()` e il suo `select!`, si vede
interrompere un lavoro gia' finito.

La cosa che vale la pena scrivere non e' il rimedio — un lucchetto, quattro
righe. E' che l'osservazione era **gia' scritta**, dieci righe piu' sotto,
dentro `l_esito_passa_intatto`: «il contatore e' globale e i test girano in
parallelo, verificarlo qui produce un test che fallisce a caso». Chi l'aveva
capito per il contatore non l'aveva riportato sulle altre tre prove dello
stesso modulo (D72), e il guasto e' venuto fuori settimane dopo, sotto carico,
per caso.

Una prova rossa a caso viene ignorata. Una prova ignorata non e' una prova.

### Le notifiche, dove il costo non era dove me lo aspettavo

Terzo pezzo di D130, e mi ha corretto il metodo.

Ero partito convinto: la notifica passa da PowerShell, quindi il costo e' il
processo PowerShell, quindi la riscrivo in Rust e il costo sparisce. Ho
misurato prima di scrivere, ed era falso:

    una notifica: 9.300 ms

Novemila e trecento. Il processo PowerShell ne spiega trecento. Gli altri
novemila sono una riga: `Start-Sleep 9`. E non e' sciatteria di chi l'ha
scritta — il fumetto dell'area di notifica muore insieme a chi possiede
l'icona, quindi qualcuno **deve** restare li' per tutta la sua durata.
Riscrivere la chiamata in Rust avrebbe tolto il tre per cento e lasciato NOVA
ferma nove secondi lo stesso.

Il difetto non era la shell. Era **chi aspetta**: novemila millisecondi in cui
NOVA non fa nient'altro, spesi a guardare un fumetto che sta gia' guardando
l'utente.

Quindi non ho tolto l'attesa — non si puo' — ho cambiato chi la paga.
`nova-notifica` nasce, mostra, aspetta e muore da solo; NOVA lo lancia e se ne
va. Misurato dopo: **5 ms**. Il fumetto e' identico e dura uguale.

Due code oneste. La prima: `Ok` adesso vuol dire «consegnata», non «vista» —
non si aspetta l'esito, quindi non si puo' sapere se il fumetto e' comparso.
Per una notifica va bene, perche' l'unico giudice di «e' comparsa?» e' la
persona davanti allo schermo e non ha un'API; ma va scritto nel tratto, e c'e'
scritto. La seconda: mi aspettavo di trovare anche un guasto di virgolette —
il ripiego incolla il messaggio dentro una stringa di PowerShell — e ho
provato apostrofi, virgolette doppie, `$(Get-Date)`, backtick, a capo, pipe.
Passano tutti. Non ogni sospetto e' un difetto, e vale la pena scriverlo tanto
quanto i difetti veri.

La lezione la porto sugli undici pezzi che restano: prima di riscrivere una
capacita' si misura **cosa costa davvero**. Se il costo e' la shell si toglie
la shell; se e' un'attesa dovuta si sposta l'attesa; e se non si guarda, si
riscrive la cosa sbagliata con molta cura (D134).

### Erano sei. Erano dieci. Ed erano tre difetti diversi

Prima di scegliere il quarto pezzo di D130 volevo misurare gli undici che
restano — e' D134, appena scritto. Cercando le chiamate a PowerShell per
misurarle, ne sono uscite altre cinque che il mio `grep` non aveva visto.

Il guasto della codifica riparato stamattina in `_ps` era **in dieci posti**,
in cinque moduli, e non era nemmeno lo stesso guasto:

    system.py   diceva UTF-8, PowerShell scrive nella tabella della console
                -> «perch? citt? per?»
    apps.py     non diceva niente, quindi cp1252
                -> «perchÃ© cittÃ  perÃ²»
    files.py    idem
    seed.py     idem del primo, terza copia privata della stessa funzione
    shell.py    UTF-8 anche su cmd.exe, che scrive in OEM cp850
                -> «citt? per? ?.txt»

`apps.py` sono `list_windows` e `list_installed_apps`: i **titoli delle
finestre aperte** e i nomi delle applicazioni installate, cioe' proprio i due
posti dove gli accenti ci sono per forza. `seed.py` e' peggio: i nomi delle
cartelle dell'utente finiscono dentro il vault, dove restano. Un ricordo
storpiato non e' un errore che passa.

E `shell.py` e' il tool con cui l'**utente** fa girare i suoi comandi. Su un
file chiamato «città però ù.txt»:

    prima:  'citt? per? ?.txt'
    dopo:   'città però ù.txt'

La cosa che non mi aspettavo: le cure non sono la stessa. A PowerShell la
codifica si **chiede** — una riga prima del comando. A cmd.exe no: si potrebbe
mettere `chcp 65001` davanti al comando dell'utente, ma vorrebbe dire cambiare
l'ambiente in cui gira il *suo* programma, e i programmi vecchi ci si perdono.
Li' si legge nella tabella che cmd usa davvero — che e' quella OEM, cp850, e
**non** e' quella che usa Python, cp1252. Due tabelle diverse sulla stessa
macchina, e chiedere all'una i caratteri dell'altra non da' un errore: da'
lettere sbagliate. E al Python figlio si dice `PYTHONIOENCODING`, perche'
senza console ripiega sulla codifica locale.

Tre fatti diversi, non una preferenza fra tre.

Il posto solo e' `nova/powershell.py`, alla decima occorrenza invece che alla
seconda (D62). Ma la parte che conta della prova non e' quella che verifica la
funzione: e' quella che **conta i posti** da cui NOVA avvia una shell. Le
prove che verificano una funzione passavano anche prima, ognuna nel suo
modulo, mentre nove chiamate su dieci erano rotte. Cio' che tiene ferma una
riparazione non e' una funzione giusta: e' che non ce ne siano altre.

Una nota su me stesso. La prima versione di quella prova e' morta con
«carattere di terminazione mancante nella stringa»: avevo scritto
`Write-Output 'perché ... un'emoji'` e l'apostrofo di «un'emoji» ha chiuso la
stringa di PowerShell. La prova scritta apposta per parlare del guaio delle
virgolette ci e' cascata dentro alla prima riga. L'ho lasciato scritto nel
commento, perche' e' l'argomento migliore che ho per D130: se ci casca chi sta
guardando proprio quello, comporre comandi incollandoci dentro dei dati non e'
una tecnica da migliorare — e' una strada da non prendere.

### «Quattordici», e un esempio che non avevo mai guardato

Prima di scegliere il quarto pezzo volevo fare quello che avevo appena scritto
in D134: misurare cosa costa davvero ognuno di quelli che restano. Ho misurato:

    system_info          1.543 ms
    list_processes         645 ms
    list_installed_apps    594 ms
    list_windows           275 ms
    get_datetime             0 ms

`system_info` costa quasi un secondo e mezzo, piu' di tutte le altre messe
insieme, ed e' anche quella che il modello chiede piu' spesso all'inizio di
una conversazione. L'ordine se l'e' scelto da solo.

Ma poi ho contato, e la parte utile e' qui.

«Quattordici punti in cui PowerShell regge NOVA» me l'ero **ricordato**, non
misurato, e nel frattempo l'avevo scritto in D130, nel diario, in due commenti
di codice Rust e in tre messaggi di commit. Contate con un analizzatore di
sintassi: ventiquattro funzioni, di cui tredici strumenti esposti al modello.

Il numero sbagliato non fa danno. L'esempio si'. Fra i tre che avevo dato —
«gli appunti *sono* Get-Clipboard, il volume *e'* SendKeys, la cattura dello
schermo *e'* Add-Type System.Drawing» — il terzo e' **falso**. La cattura dello
schermo non passa da PowerShell affatto: usa `mss` e `PIL`. Quel
`System.Drawing` che ricordavo e' delle notifiche, cioe' proprio il pezzo che
avevo appena finito di riscrivere.

Se avessi seguito il mio stesso documento, sarei andato a riscrivere una cosa
che quel problema non ce l'ha. (Ne ha un altro — due pacchetti Python — che e'
una decisione diversa e va presa a parte.)

La cosa che voglio ricordare: **un numero preso a memoria porta con se' la
stessa sicurezza di uno contato, e nessuno dei due segnala su di se' quale dei
due e'.** «Quattordici» suonava misurato. Non lo era (D136).

E siccome un elenco scritto a mano invecchia — in avanti quando si porta un
pezzo e ci si dimentica di toglierlo, all'indietro quando se ne aggiunge uno e
non si scrive — la tabella in `verso_la_beta.md` adesso ha una prova che la
lega al codice nei due versi. Alla prima esecuzione ha trovato subito un
errore mio: ci avevo messo `list_processes`, che usa `psutil` e non una shell.
Scritta cinque minuti prima, sbagliata dieci minuti dopo, corretta da una
prova. E' esattamente il motivo per cui la prova esiste (D46).

### Il pezzo piu' caro, e due difetti che col tempo non c'entravano

`system_info` costava **1.543 ms**: piu' di tutte le altre capacita' messe
insieme, ed e' quella che il modello chiede per prima quando vuole sapere dove
si trova. Chiesta alle API invece che a una query WMI dentro una stringa: 41
ms. Trentotto volte meno.

Ma il tempo e' la parte noiosa. Confrontando le due risposte una accanto
all'altra sono venuti fuori due difetti che con la velocita' non c'entrano
niente.

**Il primo.** La descrizione dello strumento diceva «CPU, RAM, disco,
batteria, rete». La batteria non la dava, la rete nemmeno. E quella riga non
e' documentazione: e' testo che il modello legge e su cui decide. Un modello
che vuole sapere se il portatile e' attaccato alla corrente chiamava questo,
non trovava niente, e non aveva modo di distinguere «la batteria non c'e'» da
«lo strumento non me l'ha detta». Adesso la batteria c'e' — e su un fisso c'e'
scritto «nessuna (e' un fisso)», perche' il silenzio direbbe «non lo so». La
rete continua a non esserci, e la descrizione ha smesso di prometterla.

**Il secondo.** I numeri:

    RAM_GB        : 31,1
    Dischi        : C: 72.5GB liberi, D: 1248.7GB liberi

Virgola nella prima riga, punto nella terza. Due formattatori diversi di
PowerShell dentro la stessa risposta, tutti e due nella lingua dell'utente. Chi
legge e' un modello che ci deve fare un conto, e «31,1» lo puo' leggere come
311. Adesso passa JSON con interi di byte, e a scriverli per una persona pensa
`dati.pesa`, che esisteva gia' (D99) e di regole ne ha una sola.

### E la strada nuova diceva una cosa che la vecchia diceva giusta

Questa e' la piu' utile della giornata.

Il primo `nova-sistema` funzionante rispondeva:

    "sistema": "Windows 10 Pro 25H2"

Su una macchina con Windows 11. Leggevo `ProductName` dal registro — la fonte
diretta, quella che sostituisce la query. E il registro **dice Windows 10**:
Microsoft ha lasciato fermo quel valore apposta, perche' i programmi che lo
leggevano per decidere non si rompessero. La query WMI che stavo buttando via
diceva «Windows 11 Pro», e diceva giusto.

Se avessi misurato solo il tempo — 41 ms contro 1.543 — avrei archiviato la
sostituzione come un miglioramento puro e messo in mano al modello un dato
sbagliato su ogni PC con Windows 11. Me ne sono accorto solo perche' avevo la
risposta vecchia sotto gli occhi mentre guardavo quella nuova.

**Una fonte piu' diretta non e' per forza una fonte piu' vera** (D138). Il dato
che non mente e' il numero di build: 22000 e' la prima di Windows 11, e la
correzione sta in una funzione pura con le sue prove, perche' il giorno che
qualcuno la trova strana e la toglie, le prove glielo dicano.

### L'elenco delle applicazioni, e un taglio che nessuno dichiarava

594 ms contro 55, e questa volta il porting e' filato: stesse tre chiavi di
registro, lette direttamente invece che con `Get-ItemProperty`. Le 229
applicazioni tornano identiche.

«Identiche» pero' ha voluto dire due domande, non una. La prima verifica che
avevo scritto diceva «gli elementi sono gli stessi» e passava. Ma un elenco
non e' un insieme: chi lo legge lo legge **dall'alto**, e il modello sulle
prime righe ci decide. `Sort-Object` di PowerShell ordina secondo la lingua
del sistema; un ordinamento per punto di codice avrebbe messo «Zoom» prima di
«Ärger» su una macchina tedesca — stessi elementi, elenco diverso, e la mia
prova sarebbe rimasta verde. Adesso il confronto e' in posizione, riga per
riga (D139).

E l'ordinamento in Rust ha un secondo criterio che sembra pignoleria e non lo
e': a parita' di lettere, il nome com'e' scritto. Senza, quale fra «Steam» e
«STEAM» sopravvive alla deduplicazione lo deciderebbe l'ordine in cui il
registro risponde — cioe' niente — e due esecuzioni della stessa domanda
potrebbero dare due risposte.

**Il difetto vero pero' non c'entrava col porting.** L'elenco finiva con
`names[:250]`. Un taglio, in silenzio. Su questa macchina le applicazioni sono
229 e non scatta mai; su una con trecento, il modello ne riceve 250, cerca un
nome che sta nelle ultime cinquanta, non lo trova e conclude che non e'
installato. Adesso il taglio si dichiara: quante ne mancano, quante sono in
tutto, e come vederle (D129).

Una nota sulla prova di quel taglio. La prima versione diceva: «se ce ne sono
piu' del massimo, controlla che lo dica». Su questa macchina non ce ne sono
piu' del massimo, quindi non controllava niente e passava — verde **per
assenza**, che e' il modo peggiore di essere verdi. Adesso abbassa il massimo
a cinque e guarda cosa succede davvero.

### Le finestre: la funzione c'era gia', ma nel posto sbagliato

Prima di scrivere `EnumWindows` sono andato a guardare se c'era gia' — e
c'era. `elenca_finestre()` viveva dentro `windows_uia.rs`, il backend di UI
Automation, e faceva esattamente questo: `EnumWindows`, `GetWindowTextW`, il
nome del processo, il pid.

Ma non usava UI Automation. Nemmeno una riga. Stava li' per come e' cresciuto
il file, e la conseguenza era che chi voleva solo sapere che finestre sono
aperte doveva far partire un thread COM e un'intera automazione per una
domanda che non ne ha bisogno. L'ho spostata in `finestre.rs`, dove vivono
gia' le altre cose che governano la scena senza passare da UIA. Spostata, non
riscritta (D99).

**E la strada vecchia rispondeva a un'altra domanda.** Questa e' la parte che
conta:

    Get-Process | Where MainWindowTitle   ->  quali processi hanno una
                                              finestra principale
    EnumWindows                           ->  quali finestre esistono

Sono due domande diverse, e lo strumento dichiarava la seconda. Un browser con
tre finestre ne mostrava una. NOVA non vedeva la **propria** seconda finestra:
`io.nova.assistente-siw` c'e', e nel vecchio elenco non compariva.

Quindi qui, al contrario del pezzo precedente, le due risposte **non devono**
combaciare — e una prova che pretendesse che combacino sarebbe una prova che
difende il difetto. Nove finestre contro undici, e le due in piu' sono vere.

Nel mezzo si e' recuperata anche una cosa buttata via: `EnumWindows`
restituisce le finestre in ordine di **pila**, dalla piu' in primo piano alla
piu' in fondo. `Get-Process` le ordinava per nome del processo, e chi chiede
«cosa ho aperto» quasi sempre intende quella davanti (D140).

Una sola esclusione, e scritta: `Progman` e `WorkerW`, cioe' lo sfondo del
desktop. Sono finestre visibili con un titolo — «Program Manager» — e
passerebbero ogni filtro, ma nessuno che chieda «che finestre ho aperte»
intende quelle. Un'esclusione taciuta e' un elenco che mente.

E una nota sulla prova: la verifica «vede piu' di una finestra per programma»
puo' non avere niente da guardare, se in quel momento nessun programma ne ha
due. In quel caso non passa e non fallisce: **lo dice**. Una prova che diventa
verde perche' non ha trovato niente da provare e' peggio di una che manca.

### Il pezzo dove non contava la velocita'

Stavo per portare `focus_window`, `close_application` e `open_application`
perche' erano i prossimi nella lista. Prima di scrivere, ho misurato cosa
selezionava `close_application` — e mi sono fermato li' per il resto del
pomeriggio.

Il nome che l'utente passa finisce dentro un `-like` di PowerShell, cioe'
dentro un linguaggio di modelli:

    name='*'       ->  292 processi
    name='?'       ->  292 processi
    name='[a-z]'   ->  292 processi

Duecentonovantadue su duecentonovantadue. Con `force` acceso e' `Stop-Process
-Force` su tutto il sistema, servizi compresi, da un argomento di **un
carattere**.

Lo strumento e' marcato pericoloso, quindi una persona approva. Ma l'anteprima
che quella persona legge diceva:

    Termina FORZATAMENTE '*'

In quella riga non c'e' niente che dica «292 processi». L'approvazione c'era e
non serviva a niente, perche' chi approvava non poteva sapere cosa stava
approvando.

E il caso concreto non era ipotetico. Sulla macchina di prova, in quel
momento, erano aperte due finestre del Blocco note:

    Senza titolo - Blocco note
    *napoli difesa - Blocco note

L'asterisco davanti al titolo e' quello che i programmi mettono quando c'e'
del lavoro non salvato. `close_application("notepad", force=true)` le prendeva
tutte e due, senza dialogo, senza «vuoi salvare». E l'anteprima diceva
soltanto «Termina FORZATAMENTE 'notepad'».

**Il rimedio non e' un modello piu' prudente.** Un modello piu' prudente e'
ancora un modello, e la prossima volta il carattere strano sara' un altro. Si
toglie: la ricerca e' per sottostringa, e `*` si cerca alla lettera.

E si separa la selezione dall'azione. Si elenca, si guarda cosa combacia, e si
chiude **un pid alla volta**. Un numero non ha caratteri jolly. Il binario non
ha nemmeno un modo di chiudere per nome: se non hai un pid, non hai scelto.

Poi l'anteprima. Adesso legge:

    Termina FORZATAMENTE 2 processi: Notepad.exe (pid 15432): «Senza titolo -
    Blocco note», «*napoli difesa - Blocco note»; Notepad.exe (pid 6300)
      ATTENZIONE: un titolo comincia per «*», che in molti programmi vuol dire
      lavoro NON SALVATO.

Il potere e' esattamente lo stesso di prima: se chiedi di chiudere, si chiude.
Cambia che tu lo veda (D141).

Una cosa che ho imparato di traverso. Avevo scritto, in tre posti, che con la
ricerca per sottostringa `*` «non trova niente». Falso: sulla macchina vera ha
trovato **un** processo, perche' quel titolo l'asterisco ce l'ha per davvero.
Uno invece di 292 e' la risposta giusta; zero sarebbe stata un'altra bugia,
piu' piccola. Corretto in tutti e tre.

### E una chiamata che puo' dire di no

Nello stesso pezzo, `focus_window`. Windows non lascia che un programma
qualunque rubi il primo piano: `SetForegroundWindow` riesce solo a certe
condizioni, e quando non riesce **non solleva niente** — fa lampeggiare
l'icona nella barra delle applicazioni e torna `false`.

Nessuno guardava quel valore. La risposta era «Finestra in primo piano: ...»
anche quando la finestra era rimasta esattamente dov'era, e il modello ci
costruiva sopra il passo successivo — clicca qui, scrivi la' — su una finestra
che non aveva il fuoco.

Adesso si chiede a `GetForegroundWindow` chi c'e' davvero davanti dopo il
tentativo, e le risposte sono tre invece di due: fatto; «Windows non l'ha
permesso, l'icona sta lampeggiando, un clic la porta avanti»; e l'errore vero.
Anche il binario le distingue nel codice di uscita — 0, 3, 1 — perche' un
rifiuto e un guasto non sono la stessa cosa e chi chiama deve poterlo dire
all'utente (D142).

### La tastiera, e una riga finita nella finestra sbagliata

Gli ultimi due strumenti marcati «ultima spiaggia». Volevo misurare cosa
arriva davvero scrivendo con le due strade di oggi — la libreria `keyboard` e
il ripiego `SendKeys` — quindi ho costruito una finestrella mia, apposta per
non toccare quelle dell'utente, e le ho scritto dentro.

Non le ho scritto dentro. Il risultato e' tornato vuoto in tutti e quattro i
casi, il che vuol dire che quel testo e' finito in **qualunque finestra avesse
il fuoco in quel momento**. Avevo dato per scontato che il fuoco fosse dove me
l'ero messo, e non l'avevo verificato. Poco dopo, chiedendo chi c'era davanti,
la risposta era: un gioco a schermo intero.

E' esattamente l'incidente contro cui la descrizione di quello strumento mette
in guardia, e l'ho fatto io mentre lo stavo studiando.

**Il difetto pero' non era il mio script.** Era che in tutta quella strada
nessuno guarda dove sta andando il testo. `SendInput`, `SendKeys` e `keyboard`
non hanno un bersaglio: mandano al sistema, che consegna a chi ha il fuoco in
quel millisecondo. E `type_text` rispondeva:

    Digitati 42 caratteri nella finestra attiva.

Vero, e inutile. Non dice quale, quindi nessuno — ne' il modello che ci
costruisce sopra il passo successivo, ne' la persona che legge — puo'
accorgersi che il testo e' andato altrove.

Adesso: il fuoco si legge **prima**, il suo handle si passa al binario, e il
binario **ricontrolla** e rifiuta se nel frattempo e' cambiato. Non e' un
doppione: fra il momento in cui una persona approva e il momento in cui i
tasti partono passa del tempo, e in quel tempo il fuoco si sposta. E
l'anteprima che quella persona legge adesso e':

    Digita nella finestra che ha il fuoco: ciao mondo
      La finestra e': «League of Legends (TM) Client» (League of Legends.exe)

La stessa forma di D141: il potere resta tutto, cambia che si veda dove va a
finire (D143).

Un guadagno di traverso: `SendKeys` e' un piccolo linguaggio — `{`, `}`, `+`,
`^`, `%`, `~`, `(`, `)` hanno un significato — e il Python li proteggeva con
un elenco di sostituzioni scritto a mano. `KEYEVENTF_UNICODE` non interpreta
niente, quindi non c'e' niente da proteggere, e passano accenti, virgolette
basse ed emoji che `SendKeys` non sa mandare.

E le combinazioni: i simboli come `ctrl+;` adesso si **rifiutano**, perche' il
tasto che fa «;» cambia con la disposizione della tastiera e premerlo alla
cieca su una tastiera italiana scriverebbe un altro carattere, in silenzio.
Rifiutare costa niente; premere il tasto sbagliato costa quanto vale la
finestra che lo riceve.

Ultima nota, sulla prova. La parte che scrive davvero non e' verificabile
mentre un gioco a schermo intero tiene il fuoco: `SetForegroundWindow` non
riesce a strapparglielo, ed e' giusto cosi'. La prova **non scrive alla
cieca**: dice che quella parte non e' provabile adesso ed esce con 2. Verde
per assenza sarebbe stato peggio di rosso (D53).

**Una cosa che resta aperta, e la scrivo invece di lasciarla nel non detto.**
In una delle prove il binario ha riferito di aver scritto — con il fuoco
verificato a ogni blocco di trentadue caratteri — e alla finestra non e'
arrivato niente. Puo' essere il gioco a schermo intero che si riprende il
primo piano fra un controllo e l'invio, oppure un difetto nel modo in cui
mando gli eventi. Non lo so, e non ho potuto guardarlo perche' su questa
macchina il fuoco e' occupato.

Quindi: la parte che verifica il fuoco e rifiuta di scrivere e' provata e
funziona; la parte che consegna i tasti **non e' verificata**, e la prova esce
2 invece di dichiararsi verde. Un verde che non so spiegare varrebbe meno di
un «non lo so» scritto (D53).

**Chiusa poco dopo.** Gio ha ridotto a icona il gioco, e con lo schermo libero
il testo arriva identico — graffe, accenti ed emoji comprese. Non era un
difetto dell'invio: era il gioco che si riprendeva il primo piano fra un
controllo e l'altro. Per un'ora ho avuto sotto gli occhi un sintomo che
sembrava un guasto del codice ed era una condizione della macchina. L'unica
cosa che mi ha impedito di andare a «riparare» codice sano e' stata la prova
che si rifiutava di diventare verde per assenza: se avesse detto «ok, non ho
potuto provarlo», avrei creduto che il problema fosse altrove e l'avrei
cercato li'.

### La semina del vault, e tre copie che funzionavano

L'ultimo pezzo con dentro PowerShell che non fosse uno strumento: la semina
del vault, cioe' cio' che scrive i primi ricordi di NOVA su chi sei e su
com'e' fatto il tuo PC. E' il posto dove un guasto non passa — un ricordo
sbagliato resta li', e il modello ci crede.

Dieci chiamate, e quasi nessuna aveva bisogno di una shell.

**Quattro erano `git`.** `git config --global user.name`, `git -C "{cartella}"
log ...`. Git e' un programma: si chiama. Passare da PowerShell aggiungeva
soltanto una stringa da comporre — e quella stringa si rompe su una cartella
con l'apostrofo nel nome. Quando si rompeva, la semina perdeva quel progetto e
non lo diceva a nessuno.

**Tre erano cose che NOVA sapeva gia'.** CPU e RAM chieste a WMI mentre
`nova-sistema` le legge dalle API. E le applicazioni installate con **la
stessa identica query di registro** di `list_installed_apps` — due copie della
stessa domanda, scritte in due momenti, che nessuno confronta mai.

Nessuna delle due dava fastidio: funzionavano. E' esattamente il motivo per
cui erano ancora li'. Una copia che funziona non si fa notare finche' non
diverge, e quando diverge lo fa in silenzio (D144).

Nella stessa passata ne e' saltata fuori una terza, che non c'entrava niente
con la semina: `runtime.py` aveva ancora la **quarta** copia di «dove sta
questo binario». `nova/binari.py` era nato mesi fa proprio perche' la terza
non diventasse la prima di dieci, e quel file non l'aveva mai usato.

Adesso `nova/macchina.py` e' l'unico posto da cui si chiede a questo PC com'e'
fatto.

E una cosa piccola che mi ha fatto sorridere. Fra le applicazioni installate,
il registro di questa macchina contiene davvero una voce chiamata:

    ${{arpDisplayName}}

Un modello di stringa che un installatore ha scritto nel registro senza
sostituirci dentro il nome vero. Non e' un'applicazione, e finiva nei ricordi
come se lo fosse.

### «Non si sovrappone a cio' che fa l'utente, lavora separatamente»

Gio mi ha fermato mentre perfezionavo la cosa sbagliata, e la correzione vale
piu' del pezzo che stavo scrivendo.

Stavo rendendo onesto `type_text`: verifica il fuoco, ricontrollalo ogni
trentadue caratteri, di' in che finestra hai scritto. Tutto giusto, e tutto
inutile rispetto alla domanda vera — perche' `SendInput` **prende la
tastiera**. Manda al sistema, il sistema consegna a chi ha il fuoco, e
chiunque sia li' in quel momento se lo ritrova addosso. Si puo' rendere
onesta; non si puo' rendere separata, perche' prendere la tastiera e' il suo
modo di funzionare.

La strada separata c'era gia', ed e' `ui.set_text`: parla all'**applicazione**
invece che alla tastiera. Non ha bisogno del fuoco, e mentre NOVA scrive li'
l'utente puo' continuare a scrivere altrove.

Quindi ho smesso di lavorare su quella sbagliata e sono andato a provare
quella giusta — che, ho scoperto, nessuna prova guardava. E aveva un difetto,
proprio contro la regola:

    scritto in un campo della Mappa caratteri, con il fuoco su un'altra
    finestra
    -> alla fine, davanti c'era la Mappa caratteri

**Non aver bisogno del fuoco non vuol dire non prenderlo.** Non lo prende
NOVA: lo prende il fornitore di accessibilita' di Windows, che per i controlli
classici implementa la scrittura con un `SetFocus` seguito da un messaggio. Ma
da dove sta la persona che stava scrivendo altrove, la differenza fra «l'ha
fatto NOVA» e «l'ha fatto Windows per conto di NOVA» non esiste.

Adesso `ui.set_text` e `ui.click` guardano chi c'e' davanti prima, fanno la
cosa, e rimettono a posto il primo piano se se lo sono preso. Se Windows non
lo permette non insistono: una lotta per il primo piano e' peggio di un primo
piano spostato (D145).

E la descrizione dello strumento diceva «non dipende da quale finestra ha il
fuoco» — vero per l'ingresso, falso per l'effetto. Adesso dice anche che il
primo piano resta dov'era, che e' la parte che all'utente interessa.

Due note su di me, in questa mezz'ora.

La prima: la mia prima chiamata a `ui.find` passava il filtro annidato
(`query: {role: ...}`) invece che piatto, e il demone ha risposto con tutto
l'albero. Per un minuto ho creduto di aver trovato un filtro che non filtra.
Era una chiamata scritta male da me — e visti dal chiamante, un filtro rotto e
un filtro mai arrivato si somigliano moltissimo.

La seconda: il `\r` in coda al testo riletto sembrava nostro. L'ho verificato
leggendo il campo **prima** di scriverci: c'era gia'. E' della Mappa
caratteri. Toglierlo nella prova e' giusto, toglierlo dentro NOVA sarebbe
correggere il campo di qualcun altro.

### Un posto dove rileggere i propri errori

Gio, oggi: *«ricordati sempre di scriverti dove sbagli, cosi' quando vuoi te
lo rileggi»*.

Gli errori miei erano gia' scritti — sono sparsi in questo diario, dentro le
voci dei pezzi in cui li ho fatti. Ma sparsi non si rileggono: si rilegge una
giornata, non una forma. Quindi adesso stanno anche raccolti in
[dove_ho_sbagliato.md](dove_ho_sbagliato.md), uno per voce, con tre cose
ciascuno: cosa credevo, cosa era vero, e **come me ne sono accorto** — che e'
l'unica delle tre che si puo' riusare.

Messi in fila, gli otto di oggi sono quattro forme sole:

1. ho **ricordato** invece di misurare (il numero quattordici, `0x70`, la data
   di un banco);
2. ho dato per scontata una **premessa** (che il fuoco fosse dove me l'ero
   messo; che gli argomenti fossero nella forma che credevo);
3. ho scritto una prova che **non poteva fallire**;
4. ho letto un **sintomo** come una causa.

La terza e' la peggiore, e vale la pena averla scritta: le altre tre le trova
qualcun altro — una prova, il compilatore, un errore. Una prova che non prova
niente non la trova nessuno. Passa.

E il promemoria, l'ultimo pezzo con dentro PowerShell: la versione di prima
**non funzionava affatto**. Tre livelli di virgolette annidate — un comando
PowerShell dentro una stringa, dentro `/TR` di `schtasks`, dentro un `cmd /c` —
e `schtasks` rispondeva «Opzione o argomento non valido: '-NoProfile'» anche
per «chiamare il dentista». Otto messaggi su otto. Nessuna prova lo guardava,
quindi nessuno lo sapeva: uno strumento dichiarato, promesso al modello nella
sua descrizione, e mai funzionante (D146).

Adesso l'attivita' e' un XML — programma e argomenti sono due campi distinti,
niente da annidare — e il messaggio dell'utente non entra nella riga di
comando affatto: sta in un file, e negli argomenti c'e' solo un percorso
scritto da NOVA. Provato per davvero: creato un promemoria per settanta
secondi dopo, aspettato, e Windows l'ha eseguito.

### Il Cestino, che e' la rete di sicurezza e si rompeva su un apostrofo

L'ultima cosa di CANT-2 che passava da una shell, e la piu' delicata di tutte,
perche' non e' una comodita': e' la premessa N2 del progetto — prima la
reversibilita', poi il permesso. Un file nel Cestino si recupera con due clic;
uno cancellato davvero no, e nessuna quantita' di conferme rimette a posto un
file che non c'e' piu'.

Passava da un comando PowerShell con il percorso incollato fra apici. Misurato
su quattro nomi:

    normale.txt                buttato
    L'anno scorso.txt          RIMASTO
    citta' pero'.txt           buttato
    con 'apici' dentro.txt     RIMASTO

Due su quattro. «L'anno scorso» e' un nome di cartella normale.

La cosa che mi ha fatto pensare: **il codice si comportava bene**. Tornava
«non ci sono riuscito», e chi lo chiamava si fermava invece di distruggere —
esattamente come deve. Ma il risultato, per chi lo usava, era che certi file
non si potevano cancellare, e il motivo era una virgoletta. La rete di
sicurezza c'era ed era la parte piu' fragile del sistema (D147).

Adesso passa da `IFileOperation`, quello che usa Esplora risorse quando premi
Canc: prende il percorso come oggetto. Cinque nomi su cinque, apostrofi,
accenti, emoji, punti e virgola e «e commerciali» compresi. Il ripiego
PowerShell resta e adesso raddoppia l'apostrofo — una riga, e funziona anche
lui — ma resta ripiego, perche' la riga giusta e' quella che non compone
niente.

Una nota sulla prova, che vale piu' del pezzo. La prima versione verificava
che il file non ci fosse piu'. **Sarebbe passata identica se il codice avesse
chiamato `unlink`** — cioe' se avesse distrutto invece di cestinare, che e'
l'unico difetto che qui conta davvero. Adesso va a cercare il file **dentro il
Cestino di Windows**, con `Shell.Application`. Cancellato e cestinato si
somigliano solo da fuori.

### Cosa NOVA si ricorda: la meta' dove sbagliare non lascia traccia

Chiuso il Cestino, CANT-2 non aveva piu' niente che passasse da una shell. Ma
CANT-2 non e' «togliere PowerShell»: e' portare gli strumenti, e il **fare**
della maggior parte di loro e' ancora Python. Il pezzo che vale di piu' e' la
memoria, perche' e' quella da cui dipende CANT-3.

`nova-memoria` sapeva gia' **pesare**: BM25, la fusione dei ranking, lo
spareggio sulla freschezza. Mancava la parte che con quei pesi **decide**: chi
entra nel contesto del modello e chi resta fuori.

E' asimmetrica, ed e' per questo che l'ho voluta fare con attenzione. Un nodo
pesato male sta nel posto sbagliato dell'elenco: si vede. Un nodo **scartato**
no — il modello risponde come se non esistesse, e nessuno, ne' lui ne'
l'utente, ha modo di accorgersene. Il difetto piu' grave della memoria e'
invisibile per costruzione.

I sei passi sono quelli del Python, nello stesso ordine e con le stesse
costanti. Le tre regole che portano una cicatrice ciascuna, e che ho tenuto
identiche:

- **chi e' nominato per nome salta il filtro sulla confidenza.** Chiedere
  «parlami di X» e ricevere zero risultati perche' X e' poco confidente non e'
  un filtro: e' una bugia;
- **il grafo prende a turno da ogni sorgente.** Prima il primo risultato si
  mangiava tutta la quota e il secondo e il terzo non contribuivano mai;
- **un vicino non puo' valere quanto una richiesta esplicita.** Senza il
  tetto, il vicino di un nodo nominato per nome valeva 350 e spingeva fuori
  dal contesto tutto quello che la ricerca aveva trovato.

Il banco pero' e' la parte di cui sono piu' contento. Non confronta i
punteggi — quelli erano gia' confrontati. Confronta **l'elenco scelto, il suo
ordine e la via con cui ogni nodo e' entrato**, contro il `KBEngine` vero e
non contro una sua copia: si costruisce un vault temporaneo, gli si chiede
`cerca`, e al Rust si passano gli stessi punteggi che il Python ha usato per
decidere. Cosi' se le due meta' divergono, divergono sulla **scelta**.

Perche' anche la via conta: un nodo entrato «da grafo» invece che «esatto»
vuol dire che la ricerca ci e' arrivata per un'altra strada. Il risultato
somiglia, e la strada e' un'altra — e la prossima volta, con altri dati, non
somigliera' piu' (D148).

Cinque scenari, e in tutti e cinque stessi nodi, stesso ordine, stesso
perche'.

### Il fratello del promemoria

Riparato `create_reminder`, sono andato a cercare chi altro creasse attivita'
pianificate. Uno: `pianifica`, cioe' il modo in cui NOVA si da' appuntamento
con se stessa. Stessa idea, stesso periodo, stessi difetti — perche' quando si
ripara una cosa in un posto la lezione non si sposta da sola (D72).

Ne aveva tre, e uno era vivo:

    «controlla l'agenda»  ->  registrata come  «controlla l"agenda»

Un apostrofo diventato virgoletta. NOVA si sarebbe posta una domanda diversa
da quella chiesta, all'ora giusta, senza che niente lo segnalasse. Gli altri
due: le virgolette doppie nell'istruzione erano **vietate** — una limitazione
scritta nel codice e dichiarata all'utente pur di non affrontare l'escaping,
quindi «cerca "casa in affitto"» non si poteva programmare — e la data usava
il formato della lingua del sistema.

Tutti e tre spariscono con lo stesso rimedio: l'attivita' descritta in XML,
programma e argomenti in due campi distinti, l'istruzione in un file e nella
riga di comando solo il suo percorso. Adesso il rimedio sta in
`nova/attivita.py`, perche' i posti che creano attivita' erano due e alla
seconda occorrenza la cosa condivisa si mette in comune (D62) — e qui le due
copie non condividevano solo il codice: condividevano i difetti, perche'
erano nate dalla stessa idea sbagliata.

Ho aggiunto `--ask-file` a `nova`, che era l'unico pezzo mancante.

E una cosa su di me, la seconda volta oggi. Nella prima misura gli accenti
sembravano rotti: «perch? citt?». Stavo leggendo con `schtasks /Query`, che
stampa nella tabella codici della console. Rifatta la misura con
`Get-ScheduledTask`, gli accenti erano intatti. **Il difetto vero era uno
solo, e per un minuto ne ho creduti due** — di nuovo per il modo in cui
leggevo, non per il dato (D131). Nella prova finale il campo si chiede in un
modo che non dipende dalla lingua, e c'e' scritto perche'.


## 5 settembre 2026, pomeriggio — CANT-2 si chiude, e non dove pensavo

Ieri sera, chiuso il Cestino, ho scritto a Gio il programma di oggi: «dentro
CANT-2 restano i corpi degli altri strumenti — `automazioni.py`,
`documenti.py`, `riparazione.py`, `web.py`, `deleghe.py`. Continuo di li'».

Stamattina, prima di cominciare, ho fatto la cosa che avrei dovuto fare ieri:
ho guardato **gli import**, non i nomi. Cinque minuti.

`documenti.py` importa `pypdf`, `docx`, `openpyxl`. `schermo.py` importa `mss`
e `PIL`. Nessuno dei due e' una traduzione: sono la scelta di quattro librerie
Rust, e quella scelta appartiene a CANT-8, l'harness dei documenti, che se la
porta dietro tutta insieme. `deleghe.py` e `kb.py` non contengono logica: sono
il router e il vault **visti da uno strumento**, e router e vault in Rust
esistono gia' — manca il filo, e il filo e' CANT-3. `web.py` e' CANT-6.
`riparazione.py` pilota il banco, che e' CANT-8. E `automazioni.py` esegue
corpi Python **per disegno**: e' il posto dove NOVA si scrive strumenti nuovi
mentre gira, quindi non e' codice da tradurre ma una decisione da prendere
quando si sapra' cosa resta di Python.

Cioe': **CANT-2 era finito ieri sera e non me n'ero accorto**, e il programma
che avevo annunciato avrebbe scelto tre librerie di documenti dentro il
cantiere sbagliato — senza la domanda che quel cantiere si porta dietro.

Un elenco di file rimasti non e' un elenco di lavoro rimasto (D150). E' D99
girato verso il proprio piano invece che verso il codice: prima di chiedersi
cosa manca, guardare cosa c'e'. L'ho scritto anche in
[dove_ho_sbagliato.md](dove_ho_sbagliato.md), perche' e' un errore di quelli
che si ripetono: avevo guardato la cartella, non i file.

Quindi CANT-2 e' chiuso, con la sua tabella di ragioni file per file in
[verso_la_beta.md](verso_la_beta.md), e si apre CANT-3.

## 5 settembre 2026, pomeriggio — CANT-3 comincia dal pezzo che se sbaglia non lo dice

Aperto CANT-3 dalla parte piu' delicata di tutto il progetto, che e' anche la
piu' piccola: **il taglio del contesto**. Duecento righe di Python che
decidono cosa il modello legge e cosa no.

La ragione per cominciare di li' e' la stessa che ha guidato la memoria
(D148), e vale la pena scriverla di nuovo perche' e' un'asimmetria e non un
giudizio. Un ordinamento sbagliato **si vede**: la risposta e' storta, e chi
legge se ne accorge. Un messaggio **buttato** no: il modello risponde come se
non fosse mai stato detto, con la stessa sicurezza di sempre, e nessuno dei
due lati della conversazione ha modo di sapere che manca un pezzo. E' l'unico
posto del progetto dove un difetto puo' restare invisibile per sempre.

Il pezzo nuovo e' `nova-contesto`. Fuori restano di proposito due cose: il
tokenizzatore vero — sta nel modello, cambia con il modello, e chiederglielo
costerebbe un giro di rete per messaggio a ogni turno solo per decidere se
tagliare — e la configurazione. Chi chiama sa quanto vale il contesto; zero
vuol dire «non lo so», e allora non si tocca niente.

**La trappola che il Rust si porta dietro da solo.** In Python `len(testo)`
conta caratteri e `testo[:meta]` taglia caratteri. In Rust la stessa scrittura
su `&str` conta **byte**. In italiano non e' la stessa cosa: «perché la città
è così» pesa piu' byte che caratteri, quindi una stima a byte direbbe che il
messaggio e' piu' grosso del vero e taglierebbe **prima** — in silenzio, e
proprio nelle conversazioni in italiano, cioe' tutte. E il taglio a byte
dentro una lettera accentata non e' un errore di stima: e' un panico, che
arriva all'utente. Tutto il modulo lavora su `char` (D152).

Poi la cosa che non avrei visto senza scrivere la prova: `max(range(n),
key=...)` in Python restituisce il **primo** massimo, `max_by_key` in Rust
l'**ultimo**. Con due messaggi lunghi uguali le due parti accorcerebbero
messaggi diversi.

**Il banco.** Ventisette scenari, confrontati contro l'`Agent` vero costruito
con `__new__` — perche' il taglio non guarda ne' il modello ne' gli strumenti,
e costruire l'agente per davvero vorrebbe dire accendere un modello per
provare dell'aritmetica. Non si confronta solo l'elenco finale: si confrontano
anche **quanti** messaggi sono stati tolti e **per quale ragione**, perche'
due implementazioni possono arrivare allo stesso elenco per strade diverse e
divergere al primo caso che le separa (D51). Settantatre verifiche.

Sono passate tutte al primo colpo, e un verde al primo colpo su un pezzo cosi'
non e' una buona notizia finche' non si e' visto diventare rosso (D53). Tre
mutazioni fatte apposta nel Rust:

    stima a byte invece che a caratteri   ->  7 verifiche rosse
    obiettivo a 0,80 invece di 0,75       ->  4 verifiche rosse
    scarto degli orfani tolto             ->  5 verifiche rosse

La prima e' quella che conta di piu': ha acceso **solo** gli scenari con testo
accentato. Se il banco avesse avuto solo scenari in inglese sarebbe rimasto
verde per sempre, e il difetto sarebbe uscito sul PC di qualcuno.

**Due cose scoperte, e nessuna delle due aggiustata.** Il taglio a numero non
ha la rete che ha il taglio a token: se la coda finisce tutta in risposte di
tool orfane, resta il solo messaggio di sistema e la conversazione sparisce
senza che niente lo dica, mentre il taglio a token in quel caso ripesca
l'ultimo messaggio. E la funzione che accorcia il messaggio piu' grosso oggi
riceve **sempre un solo messaggio**, perche' chi la chiama ci arriva solo dopo
che il ciclo di svuotamento ha lasciato la coda vuota: il codice che sceglie
fra piu' messaggi non e' morto, e' dormiente.

Le ho portate uguali tutte e due, con una prova che descrive il comportamento
**vero** invece di quello desiderato, e le ho scritte qui (D153). Aggiustare in
Rust cio' che il Python fa diversamente vuol dire due cose insieme: che il
banco non confronta piu' niente, e che il difetto resta comunque in
produzione, dove il codice gira ancora oggi. Prima uguali, poi si discute.

## 5 settembre 2026, pomeriggio — Dove la prosa diventa un'azione

Secondo pezzo di CANT-3, e non e' il ciclo: sono i **due punti in cui il testo
del modello e il mondo si toccano**.

Da lui verso il PC: certi modelli le chiamate agli strumenti non le mettono
nel canale apposito, le **scrivono nel discorso**, dentro `<tool_call>`. NOVA
le legge, ed e' un ripiego che vale la pena chiamare col suo nome — e' il
punto in cui della prosa diventa un'azione.

Ho scritto il lettore a mano invece di ricopiare l'espressione regolare, e
non per gusto. `<tool_call>\s*(\{.*?\})\s*</tool_call>` dice due cose precise
che a leggerla in fretta si perdono: la graffa dev'essere la **prima cosa non
bianca** dopo l'apertura, e la chiusura buona e' la prima preceduta — a meno
di spazi — da una graffa. Scriverlo a mano costringe a dirle ad alta voce, e
a decidere cosa fare di `<tool_call> ecco: {...}`: non e' una chiamata, e
adesso c'e' una prova che lo dice (D154).

Dal PC verso di lui: un risultato troppo lungo non entra nel discorso. Prima
si tagliava a ventiquattromila caratteri con «risultato troncato» e il resto
spariva — il modello non sapeva *cosa* aveva perso, solo che mancava
qualcosa. Adesso il testo intero va su file e al suo posto restano testa,
coda e il percorso per andarselo a leggere: non e' spazio risparmiato, e' una
perdita silenziosa diventata un rinvio. Il disco e l'orologio restano fuori:
il percorso e la data arrivano da chi chiama.

**Il dettaglio che sembra cosmesi e non lo e'.** Gli argomenti resi in
stringa: `json.dumps` scrive `{"a": 1}`, `serde_json` scrive `{"a":1}`. Quella
stringa non e' una rappresentazione — e' cio' che lo **strumento riceve**. Ho
scritto un piccolo serializzatore che rende come rende Python, e nel banco una
verifica che si arrabbia se **nessuno scenario ha due chiavi**: senza, i due
separatori sarebbero indistinguibili e il confronto non proverebbe niente. Poi
la mutazione di prova — separatori compatti — ha acceso due verifiche, quella
sul confronto e quella che controlla il confronto (D155).

E una cosa che si vede solo leggendo il Python con attenzione: la scelta del
campo si fa con `or`, quindi `"arguments": ""` non e' una stringa vuota da
passare avanti, e' un campo che **non conta**, e si guarda `parameters`. Con
un semplice «c'e' / non c'e'» si passerebbe una stringa vuota dove il Python
passa `{}` — e lo strumento riceverebbe argomenti diversi da quelli scritti.

## 5 settembre 2026, pomeriggio — Un verde che era una monetina

Rimisurata la suite dopo il secondo pezzo, e' uscita rossa dove tre ore prima
era verde: `test_promemoria.py`, «appesa: non e' finita entro 90s». Non avevo
toccato niente di quel codice.

La prova chiede a Windows di eseguire un'attivita' al **prossimo minuto
tondo**, poi aspetta e va a vedere com'e' andata. Quanto duri dipende quindi
dal secondo in cui e' partita: fra i 35 e i 95 secondi, piu' il tempo che
Windows si prende per rispondere. Il banco ne concede novanta, uguali per
tutte. Era una monetina.

E' il modo peggiore di essere rossa. Una prova rossa sempre la si ripara; una
prova rossa **a volte** la si crede verde per meta' delle volte, e quando esce
rossa il prossimo cerca il difetto in cio' che ha toccato lui — che e'
esattamente quello che stavo per fare io.

Adesso una prova puo' dichiarare il suo tempo: `# banco: attesa 240` nelle
prime righe. Il banco lo legge dal file **senza importarlo**, perche'
importare una prova per sapere quanto dura vorrebbe dire eseguirla; non
concede mai meno del minimo — dichiarare cinque secondi sarebbe un modo di
rendersi verdi — ne' piu' di un tetto, perche' una dichiarazione sbagliata non
deve poter bloccare il banco per sempre (D156). Sei verifiche nuove in
`test_harness.py`, compresa quella che una riga uguale ma dentro una stringa
non conta.

**E la parte che riguarda me.** Tre ore fa ho scritto «80 prove verdi, zero
rosse» in un messaggio di commit, e ci ho creduto. Era una misura fatta una
volta sola. L'ho messo in `dove_ho_sbagliato.md` accanto alle tre prove che
non provavano niente, con la differenza che le separa: quelle erano verdi
**sempre** e per il motivo sbagliato — si smascherano guardandole — questa e'
verde **a volte**, e guardarla non basta. Bisogna rimisurare.

## 5 settembre 2026, pomeriggio — Il messaggio numero zero, e NOVA che non partiva

Terzo pezzo di CANT-3: il prompt di sistema. Ventimila caratteri — 4.804 di
prompt predefinito e 15.239 di regole operative — che il modello rilegge a
ogni richiesta e che da soli valgono circa 5.200 token: un terzo del contesto
prima che l'utente abbia detto qualcosa. Sta in `nova-contesto` perche' e' il
messaggio numero zero della finestra, e la finestra sa gia' pesarlo.

I testi non li ho ricopiati. Ventimila caratteri a mano sono ventimila
occasioni di cambiare una parola, e una parola diversa e' un comportamento
diverso che nessun tipo intercetta — la stessa ragione delle sessanta
dichiarazioni (D112). `_estrai_prompt.py` li scrive, e poi fa la cosa che nel
metodo conta piu' dello scrivere: **rilegge il file appena scritto**, sfila il
letterale grezzo e lo confronta con la stringa Python. «Ho scritto il file»
non e' una verifica; un delimitatore sbagliato darebbe un file plausibile e
mutilato, e da li' in poi il banco confronterebbe il Rust con se stesso
(D158). Poi il banco ripete il confronto a ogni esecuzione: una mutazione di
**una** lettera dentro i quindicimila caratteri ha acceso nove verifiche, e
ha detto a quale carattere.

**E poi il difetto.** Il prompt si componeva con `str.format`:

    base = self.cfg.system_prompt.format(user=..., now=..., home=...)

`str.format` non guarda i tre segnaposto. Guarda **tutte** le graffe. Un
`system_prompt` personalizzato in `config.json` che contenga un esempio JSON —
e chi scrive un prompt di sistema ci mette esempi — salta con un `KeyError`
grezzo. Misurato:

    'Sei NOVA per {user}. Esempio JSON: {"a": 1}'  ->  KeyError '"a"'
    'Sei NOVA per {user}. Le graffe {} cosi'      ->  IndexError
    'Sei NOVA per {utente}'                        ->  KeyError 'utente'

E la composizione avviene dentro `Agent.__init__`. Quindi non e' un turno
andato male: **NOVA non parte**, e quello che l'utente legge e' un
`KeyError`. E' il punto 2 del cancello della beta — nessun traceback
raggiunge l'utente — trovato dove nessuna delle tre liste lo stava cercando,
perche' su questa macchina il prompt e' quello predefinito e il predefinito ha
esattamente le tre graffe giuste.

La correzione e' smettere di formattare: si sostituiscono i tre segnaposto
conosciuti e tutto il resto resta scritto com'e' (D157). Corretta da **tutte e
due le parti insieme**, Python e Rust, perche' aggiustare solo il Rust
vorrebbe dire che il banco non confronta piu' niente e che il difetto resta
dove gira il codice (D153). Il banco confronta dieci prompt, compresi i
quattro che prima esplodevano, e tre verifiche in piu' passano dalla porta da
cui ci passa l'utente: `Agent.system_prompt` con quei prompt, e NOVA parte.

Centosette verifiche.

E una coda alla giornata: rimisurando, `test_prefisso.py` e' diventata rossa.
Non per il difetto — per il nome. Quella prova verifica una cosa che da fuori
non si vede, cioe' che nel prompt di sistema non ci sia niente che cambi da
solo (un contatore, un'ora ricalcolata: basta quello per buttare via
diecimila token di cache a ogni turno), e per farlo legge il **sorgente**.
Cercava la stringa `now=datetime.now()`. Cambiata la composizione, quel nome
non c'era piu'.

Rosso a costo zero di informazione, e del tipo peggiore: quello che insegna a
non guardare i rossi. Adesso la prova conta **quante volte** l'orologio viene
letto dentro `system_prompt` — una — e controlla che nessun altro pezzo del
turno lo rilegga. Stessa domanda, posta a cio' che il codice fa invece che a
come e' scritto (D159). Verificata con una mutazione che legge l'ora due
volte: rossa.

## 5 settembre 2026, pomeriggio — Quello che si attacca in coda, e un numero scritto due volte

Quarto pezzo di CANT-3: i blocchi. La memoria e le procedure fanno la stessa
cosa — aggiungono roba **alla domanda** invece che al prompt di sistema — e la
ragione e' doppia, funzionale e di costo.

Funzionale: i cervelli agentici il prompt di sistema lo ricevono solo
all'apertura della sessione, quindi dal secondo turno in poi il contesto della
memoria veniva calcolato e buttato. NOVA faceva la ricerca sul grafo e non la
leggeva. Di costo: il messaggio di sistema e' la prima regione di token su cui
un fornitore tiene la cache, e cambiarlo a ogni turno — cambiava, perche' il
contesto dipende dalla domanda — invalida tutto il prefisso.

E' la stessa aritmetica del fondo nel taglio dei messaggi: non cambia cosa il
modello legge, cambia quanto spesso si butta via la cache. Percio' la regola
di composizione sta in `nova-contesto`. Il **testo** delle procedure invece
sta in `nova-ricette`, accanto al suo dato: un testo lontano dal dato si
aggiorna a meta' (D72, D160).

**Il numero scritto due volte.** Il punteggio di somiglianza finisce dentro il
testo che il modello legge, e la prima stesura lo arrotondava dentro la
funzione che scrive. Ma nel Python arrotonda `proponi`, e chi scrive scrive
quello che riceve: arrotondavo due volte. Il banco l'ha detto al primo giro,
con un caso a 0,125 — un carattere di differenza, e senza quel caso sarebbe
passato.

Separate le due cose, ognuna con la sua trappola. **Scrivere**:
`format!("{}", 1.0)` in Rust da' `1`, `str(1.0)` in Python da' `1.0`.
**Arrotondare**: non `(x * 100).round() / 100`, perche' quella
moltiplicazione introduce un errore che puo' far attraversare la mezza cifra
al numero sbagliato. Misurato con una mutazione apposta:

    0.125  ->  rust 0.13  vs  python 0.12
    0.615  ->  rust 0.62  vs  python 0.61
    2.675  ->  rust 2.68  vs  python 2.67

Tre casi su quindici, e sono i tre che avevo messo apposta perche' stanno
esattamente a meta'. Con quindici valori qualunque il banco sarebbe rimasto
verde. La versione buona fa quello che fa Python: scrive il valore binario
esatto con due decimali correttamente arrotondati, e lo rilegge (D161).

Poi le solite mutazioni per non fidarsi del verde: «PROPOSTE» che diventa
«Procedure» accende quattro blocchi, «kb_note o kb_forget» che diventa
«kb_note oppure kb_forget» ne accende quattro. Sono le parole su cui il
modello decide se quei passi sono un ordine o un appunto.

116 verifiche in `test_contesto_rust.py`, 35 in `test_ricette_rust.py`.

## 5 settembre 2026, pomeriggio — Un codice HTTP detto in italiano

Quinto pezzo di CANT-3, e non e' dentro `agent.py`: e' la faccia che i
fornitori mostrano quando qualcosa non va. `nova-guasti::http`.

Sta in un posto solo perche' la stessa risposta la ricevono tutti i cervelli a
pagamento, e una spiegazione per fornitore vorrebbe dire cinque spiegazioni
che divergono (D162). Dentro ci sono i due rami che non si indovinano
leggendo la specifica HTTP, e che sono li' perche' qualcuno li ha misurati:

- **llama.cpp usa 400 per il contesto sfondato**, non 413, e nel corpo scrive
  «exceeds the available context size». Senza quel ramo all'utente arrivava il
  JSON in inglese.
- **risponde 500 quando gli arriva un'immagine** e lui e' partito senza
  proiettore visivo. E' un errore di configurazione travestito da guasto del
  server, e dirgli «di solito passa da solo» lo manda ad aspettare una cosa
  che non succedera' mai.

**E la parte che non e' cortesia.** Il corpo della risposta non si incolla mai
com'e': quando la chiave e' sbagliata il fornitore **la rimanda indietro
dentro il proprio messaggio d'errore**. Il motivo utile e' sepolto nel JSON,
quindi si scende — per al massimo quattro livelli, perche' un JSON che si
annida all'infinito e' un JSON ostile e qui si sta leggendo la risposta di
qualcun altro — poi si maschera e si taglia a duecento caratteri (D163).

Il banco confronta ottantacinque spiegazioni e diciannove corpi, uno dei quali
ha dentro una chiave vera. E ha una verifica che si arrabbia **se quel corpo
non c'e'**: senza, il controllo «la chiave non compare in nessuna frase»
sarebbe verde per assenza, che e' il tipo di verde che non prova niente.
Mutazione di prova con la maschera tolta: tre verifiche rosse e la chiave in
chiaro. Seconda mutazione, quattro livelli che diventano cinque: una rossa,
proprio sul corpo annidato messo apposta.

Un doppione scoperto per strada: avevo riscritto `spiega_irraggiungibile`, che
in `nova-guasti` c'era gia'. Cancellato — due copie della stessa frase sono
due frasi che un giorno diranno cose diverse (D72).

E l'ordine con cui la domanda si compone — prima cio' che hai chiesto, poi
cio' che NOVA sa, poi cio' che ha gia' fatto, poi come deve rispondere — adesso
e' una funzione sola invece di una somma scritta a mano dentro il turno.
L'istruzione resta l'ultima cosa letta, che e' il posto in cui i modelli la
seguono di piu'.

## 5 settembre 2026, pomeriggio — Il banco aveva due colonne e ne servivano tre

Rimisurando dopo il pezzo sugli errori HTTP, quattro rosse. Nessuna delle
quattro era un difetto del codice, e la ragione di ognuna vale la pena di
scriverla.

**Una era mia, ed e' la stessa di ieri.** `test_prefisso.py` cerca nel
sorgente di `send` la scrittura `"content": user_text + ...` per verificare
che il contesto della memoria finisca in coda alla domanda e non nel prompt di
sistema. Ieri avevo riscritto, nello stesso file, il controllo sull'orologio
per la stessa ragione — cercava una scrittura invece di una proprieta' — e ne
avevo fatto una decisione (D159). Poi ho spostato quella somma dentro una
funzione, e le due righe accanto sono diventate rosse per un difetto che non
c'era.

Avevo riparato l'istanza, non la classe. Tre righe piu' su, nello stesso file,
due ore dopo aver scritto la regola. Adesso quelle prove chiedono all'**albero
sintattico** dove finisce il valore: quali variabili nascono da
`self._blocco_*()`, e se quei nomi compaiono nel contenuto del messaggio con
ruolo `user`. Provata con una mutazione che toglie la memoria dalla
composizione: rossa, e dice quali nomi ha trovato al suo posto.

**Una era un binario vecchio**: avevo rimesso a posto il Rust dopo una
mutazione ma non ricostruito il banco, quindi la suite confrontava il Python
con una versione mutata. Il tipo di errore che si ripara in dieci secondi e
costa mezz'ora di dubbi.

**E due non erano rosse affatto.** `test_tastiera.py` e
`test_scrittura_senza_tastiera.py` girano su finestre vere, e in quel momento
davanti c'era «League of Legends (TM) Client» a schermo intero. La prima ha
fatto la cosa giusta: non ha preso il fuoco, **non ha scritto alla cieca**, e
si e' fermata dicendo «questa parte non e' provabile adesso» — uscendo con
codice 2, che nel progetto vuol dire esattamente questo.

Solo che il banco guardava `returncode == 0` e basta. Codice 2 finiva fra le
rosse.

Su questa macchina non si era mai visto, perche' i banchi Rust erano costruiti
e il fuoco era libero. Ma il punto 1 del cancello della beta e' «qualcuno che
non e' l'autore l'ha installato»: su quella macchina, senza i banchi Rust
costruiti, meta' della suite avrebbe detto «rossa» parlando **di se' e non del
codice**, e il primo che la installa avrebbe cercato difetti che non
esistono.

Adesso il banco ha tre colonne. «Non provabile» non e' un modo gentile di dire
rossa, e non e' nemmeno verde: contarla verde vorrebbe dire credere provata
una cosa che nessuno ha provato. Non conta nella regola del banco — una prova
che oggi non si puo' provare non e' una regressione di chi ha toccato il
codice — ma si **dice sempre**, anche quando non cambia il verdetto (D164).

**E il confine, che e' la parte delicata.** La prova che scrive senza tastiera
puo' fallire in due punti. Se non riesce a mandare il fuoco altrove, la
premessa non c'e': quello e' «non provabile», e si verifica guardando chi sta
davanti. Se invece fallisce la scrittura vera — ed e' quello che e' successo,
con un `0x80040201` dalla Mappa caratteri — da li' non si puo' sapere se sia
NOVA o la cavia. Chiamarla non provabile sarebbe scegliere l'ipotesi comoda,
ed e' il modo esatto in cui una prova diventa verde per assenza. Resta rossa,
ma senza traceback: adesso dice cosa e' successo e cosa provare prima di
cercare il difetto nel codice (D165).

## 5 settembre 2026, sera — Due foto che uscivano dal PC senza che nessuno lo dicesse

Sesto pezzo di CANT-3: le immagini che entrano nella conversazione. Doveva
essere una funzione di comodo, ed e' venuta fuori una domanda di privacy.

La regola era: **se il risultato di uno strumento nomina un'immagine che sta
su disco, quella si guarda**. Scritta cosi' vale anche per gli strumenti che
verranno, ed e' esattamente il motivo per cui era stata scritta cosi'. E' una
buona regola per `screenshot`, che di immagini ne produce una.

Ma `search_files` restituisce **percorsi assoluti, uno per riga**. Ho
misurato, con tre file finti in una cartella temporanea:

    uscita finta di search_files:
    C:\...\matrimonio-01.jpg
    C:\...\matrimonio-02.jpg
    C:\...\documento.png

    percorsi riconosciuti: 3
    messaggi aggiunti: 1
      testo: [immagine: matrimonio-01.jpg, matrimonio-02.jpg] Questa e' la
             figura prodotta dallo strumento...
      blocco immagine, base64
      blocco immagine, base64

Quindi: «NOVA, trovami le foto del matrimonio» → due fotografie convertite in
base64 e allegate alla conversazione → con un cervello a pagamento attivo,
**fuori dal PC** al giro successivo. Nessuno lo dice, e non c'e' niente di
rotto: lo strumento ha fatto il suo lavoro, la consegna ha fatto il suo.

E sotto una riga che diceva «questa e' la figura **prodotta** dallo
strumento», che non era nemmeno vero: nessuno strumento le aveva prodotte, una
ricerca le aveva nominate. Non e' una sfumatura di stile — quella riga e'
l'unica cosa che il modello legge per sapere da dove viene cio' che sta
guardando (D167).

**Dove passa il confine.** Non nella cartella: un utente puo' benissimo dire
«guarda questa foto sul desktop», e restringere alle cartelle di NOVA
romperebbe il caso legittimo. Non nella freschezza del file, per la stessa
ragione. Passa nel **numero**: uno strumento che produce un'immagine ne
produce una, un elenco ne nomina tante. Quindi una sola si consegna, molte si
dichiarano e non si allegano — il modello sa che ci sono, sa quante sono, e
puo' chiederne una per nome (D166).

Il rifiuto sta in due posti — nella consegna e nella funzione che costruisce
il messaggio — e l'ho verificato mutandoli uno per volta: togliendo il primo
si accende una verifica, togliendo anche il secondo se ne accendono tre.

**E il riconoscimento dei percorsi.** Scritto a mano invece che con
l'espressione regolare, per la stessa ragione delle chiamate dentro il testo:
scriverla a mano costringe a dire la regola ad alta voce. Qui la regola e' che
si parte da `C:\` **o da una barra qualunque**, si prende il meno possibile e
ci si ferma alla prima estensione di immagine. Il che vuol dire che
`relativo/senza/attacco.png` da' `/senza/attacco.png` e `http://x.it/a/b.png`
da' `//x.it/a/b.png`: non e' un difetto di questa scrittura, e' cosa fa la
regola, e adesso c'e' una prova che lo dice invece di lasciarlo scoprire. Il
filtro vero e' l'esistenza del file, non la forma del percorso.

Sedici testi confrontati con il Python carattere per carattere. Mutazione di
prova, il `+?` che diventa goloso: rossa su `C:\a.png\b.png`, dove il Rust
prendeva tutta la riga e Python solo il primo pezzo.

**Questa la lascio decisa ma non chiusa**: e' una scelta di prodotto, e la
regola del numero potrebbe essere troppo stretta (un elenco con dentro una
sola foto la manda comunque) o troppo larga. Gio dira'.

## 5 settembre 2026, tarda sera — Come NOVA impara, e un a capo che Rust non conosce

Settimo pezzo di CANT-3: `nova-ricette::imparare`. Tre cose che decidono
**cosa NOVA impara** e che in Python stavano dentro un metodo di novanta righe
insieme alla rete e al filo: il testo che si manda al modello, la decisione se
valga la pena mandarlo, e la lettura di cio' che risponde.

**Il prompt non l'ho ricopiato.** Millesettecento caratteri, e una parola
diversa in un prompt e' un comportamento diverso che nessun tipo intercetta
(D112). Spostarli da dentro un metodo a un modello con tre segnaposto e'
esattamente il genere di cosa che si fa «senza cambiare niente». Quindi:
prima ho **catturato** il testo esatto che il codice di allora produceva per
tre gruppi di argomenti, poi ho rifattorizzato, poi ho verificato che il nuovo
producesse gli stessi 1010, 2123 e 978 caratteri (D169). I tre file sono
rimasti, come `_estrai_strumenti.py`: il metodo vale quanto il risultato.

**E poi l'a capo.** `str.splitlines()` di Python non taglia solo su `\n`:
taglia anche su `\r`, `\v`, `\f`, i separatori `\x1c`-`\x1e`, `\x85`, e su
U+2028 e U+2029. `str::lines()` di Rust taglia **solo** su `\n`.

Sembra pedanteria finche' non si guarda chi ha scritto il testo che si sta
leggendo: qui e' un modello, e un modello scrive quello che gli pare.
Misurato, con una mutazione che usa `lines()`:

    "Titolo  1. passo con a capo strano abbastanza lungo"
      Python: due righe   -> procedura letta, archiviata
      Rust:   una riga    -> «risposta troppo corta», niente archiviata

Cioe' NOVA non impara la procedura, e nel registro resta scritto che il
modello ha risposto male. Un difetto che non rompe niente e fa perdere una
cosa per volta (D168).

L'altra mutazione: `arrotonda_al_pari` che diventa `round()`. Nel motivo
scritto nel registro — «saltata: 2s sotto la soglia di 8» — Python usa
`f"{x:.0f}"`, che a mezzo arrotonda al **pari**. Con `round()` di Rust, 2,5
diventa 3 e 0,5 diventa 1. Due verifiche rosse, e sono i due casi che avevo
messo apposta perche' stanno esattamente a meta'.

Una cosa sui motivi. In Python ogni rifiuto porta dentro un `repr` del testo
incriminato, che e' logica di **registro** e non di decisione. Il banco
confronta il tipo del rifiuto e il pezzo di testo, non come Python lo scrive
fra virgolette: la forma della riga di log resta a chi la scrive. Confondere
le due cose avrebbe voluto dire portare in Rust il `repr` di Python, che e'
un pezzo di Python e non un pezzo di NOVA.

Diciannove risposte finte confrontate, e quasi tutte storte apposta: vuote,
di soli spazi, «NIENTE», un titolo solo, passi troppo corti, «ALTRE PAROLE»
senza i due punti, alias vuoti fra le virgole, accenti nei passi, e i quattro
tipi di a capo.

## 5 settembre 2026, tarda sera — Il titolo di una finestra e' una stringa qualunque

Ottavo pezzo di CANT-3, e sono due cose piccole che stavano in mezzo al ciclo.

**Le figure sfilate a chi non vede.** Quando si cambia cervello e quello nuovo
non ha il proiettore visivo, le immagini gia' in conversazione vanno tolte — se
no llama-server risponde 500 e quel messaggio resta li' a far fallire anche
tutti i turni dopo. Ma non si butta il messaggio: **il testo resta e
l'immagine se ne va**, cosi' il modello continua a sapere che una figura
c'era e non crede di averla guardata. Un messaggio sparito e un messaggio
senza figura sono due cose diverse: la prima gli fa dimenticare che ha chiesto
qualcosa. Adesso sta in `nova-contesto::figure`, insieme al plurale giusto per
il registro — «una figura», «due figure» — che e' il genere di dettaglio che
si sbaglia una volta e resta per anni.

E la funzione torna `false` quando non c'era niente da togliere, che non e'
cortesia: chi ha chiamato deve saperlo, perche' rilanciare la stessa richiesta
identica e' il modo piu' rapido di trasformare un errore in un ciclo.

**E poi la cosa che conta.** `GUARDANO_LO_SCHERMO` e' l'elenco degli strumenti
che mostrano *cosa c'e' aperto adesso* invece di *com'e' fatto il PC*: le
finestre, l'albero dell'interfaccia, la cattura dello schermo. Se un turno ne
ha usato uno, quel turno si ricorda **coperto**.

L'ho messo accanto al guardiano dei segreti, in `nova-guasti`, e la ragione e'
che e' la stessa domanda — cosa non entra in memoria — posta pero' alla
**provenienza** invece che alla forma. Il guardiano legge il testo e riconosce
una chiave, un numero di carta, una password. Ma il titolo di una finestra e'
una stringa qualunque: non c'e' niente da riconoscere, e proprio per questo
passerebbe. Ricordarlo in chiaro scriverebbe nel vault i titoli delle schede
aperte e dei documenti su cui si sta lavorando — e un vault e' una cartella
che la gente sincronizza (D170).

Il banco confronta **l'elenco intero**, non un campione di casi. Un elenco
provato per campioni si accorge di uno strumento aggiunto da una parte sola
solo se per caso quel caso c'era: e' la stessa lezione delle sessanta
dichiarazioni (D113). Mutazione di prova con `screenshot` tolto dal Rust: due
verifiche rosse, di cui una dice esattamente quale manca.

## 5 settembre 2026, tarda sera — I cervelli, e un dominio che si chiama quasi come casa

Nono pezzo di CANT-3, e stavolta non e' `agent.py`: sono i **cervelli**. Non
il loro giro di rete, che resta dov'e', ma le decisioni che prendono guardando
del testo — e sono tre, tutte con una conseguenza che si vede.

**«Questo indirizzo e' in casa?»** E' la frase su cui NOVA sta in piedi
ridotta a una domanda sola. Un server locale — Ollama, LM Studio, llama.cpp —
non chiede nessuna chiave, e pretenderne una vorrebbe dire rifiutarsi di
parlare con un cervello che e' li', acceso e gratuito. Ma la risposta si
decide sull'**host**, non sul testo dell'indirizzo: un confronto per
sottostringa chiamerebbe «casa» anche `https://localhost.evil.example.com`.

L'ho scritto a mano, senza dipendenze, e la ragione e' che questo e' il pezzo
che decide **cosa esce dal PC**: li' si vuole meno codice di qualcun altro,
non di piu'. Scrivendolo e' saltata fuori una regola di `urlsplit` che a
occhio non si indovina: **l'autorita' esiste solo dopo `//`**. Quindi
`localhost:8080` senza schema non ha host — `localhost` viene letto come
*schema* — e per NOVA quell'indirizzo non e' in casa. Sembra sbagliato e non
lo e': quella stringa non e' un URL, e indovinare cosa intendesse chi l'ha
scritta e' il genere di gentilezza che qui non si fa (D171).

Ventisei indirizzi confrontati con `urlparse` vero, quello di Windows.
Mutazione per sottostringa: tre rossi, fra cui il dominio che si chiama quasi
come casa.

**Il ragionamento separato dalla risposta.** I modelli lo scrivono dentro
`<think>...</think>`, o in un campo a parte. Tenerli separati non e'
formattazione: l'utente legge la **risposta**, e un ragionamento che ci
finisce dentro e' il modello che si contraddice a voce alta davanti a chi ha
chiesto qualcosa. Il caso che si vede di piu' e' quello in cui il modello si
interrompe a meta' del ragionamento: senza il secondo taglio quel troncone
arriverebbe intero, e si nota perche' non finisce nemmeno con una frase
compiuta (D172).

**E «Claude Code:» seguito dal nulla.** Quando il CLI torna con `is_error`,
prima si scriveva `f"Claude Code: {testo[:600]}"` dove `testo` veniva da
`result` — un campo che in caso di errore **spesso non esiste affatto**. Il
risultato era la riga «Claude Code:» e poi niente: un guasto che dice di
essersi rotto e non dice altro, cioe' la morte silenziosa che N8 vieta. La
causa vera stava in `subtype`, che c'era gia' e nessuno leggeva. Adesso sta in
`nova-guasti::cervelli` insieme ai riconoscitori di limite d'uso e alle due
reti sotto — il flag non documentato e la riga di comando troppo lunga.

## 6 settembre 2026, notte — Il conto di CANT-3, e CANT-4 che comincia

**Prima il conto, e stavolta guardato invece che dedotto.** Chiudendo CANT-2
avevo annunciato cinque file scegliendoli dai nomi nella cartella, e nessuno
dei cinque era CANT-2 (D150). Quindi ho scritto `_conto_cant3.py`, che legge
l'albero sintattico di `agent.py` e dei quattro cervelli e marca ogni funzione
con cio' che il suo corpo **nomina**: la rete, il disco, i processi, i fili,
il registro degli strumenti, i cervelli, l'orologio.

    funzioni che non toccano niente ........... 930 righe
      di cui gia' portate ..................... ~455
      di cui restano, adattatori sui cervelli .. ~180
      il resto: costanti, __init__, astratti ... ~295
    funzioni che toccano il mondo ........... 1.039 righe

Le quattro grosse che toccano tutto — `_giro` (110), `_sali_di_gradino` (68),
`_execute_call` (61), `send` (39) — sono **il ciclo**, e nominano insieme il
cervello, gli strumenti e l'orologio. Non e' una traduzione rimandata per
pigrizia: un ciclo in Rust che chiama strumenti Python e cervelli Python non
ha liberato niente, ed e' scritto nel cantiere fin dall'inizio. CANT-3 resta
aperto con dentro quello.

**E poi CANT-4.** Prima di scrivere una riga ho fatto la domanda di D99:
*cosa c'e' gia'?* La risposta e' che il grosso di `runtime.py` era in
`nova-modelli` da settimane — i GGUF, i motori, i conti sulla VRAM, gli
strati. Chiederselo ha risparmiato di riportare quattrocento righe.

Quello che restava e' il pezzo dove le decisioni si vedono poco e costano
molto (D173): la **riga di comando** di llama-server, la **scala dei layer**,
e il riconoscimento dell'unico errore che vale la pena riprovare. Il processo
non si avvia li': li' si decide cosa gli si dice.

Due cose che sembrano dettagli. La cache KV a 8 bit non si passa quando e'
`f16`, che e' gia' il valore di fabbrica — un flag in meno e' una cosa in meno
che puo' non piacere a un binario vecchio; e il proiettore visivo arriva **da
fuori**, perche' la stessa domanda («questo modello vede?») la fa anche chi
decide se allegare una figura, e se rispondessero in due posti diversi prima o
poi risponderebbero diverso.

**E una cosa trovata portando.** Le sei parole con cui llama.cpp dice «non ci
sta in memoria» vengono da posti diversi. Portandole ne sono saltate fuori due
che **non** si riconoscono:

    VK_ERROR_OUT_OF_DEVICE_MEMORY                      -> non riconosciuto
    ggml_vulkan: Device memory allocation ... failed   -> non riconosciuto
    vk::Result::eErrorOutOfDeviceMemory                -> riconosciuto

Sono i due casi in cui NOVA non riprova con meno layer e si arrende. Portate
uguali, e scritte in una prova che si chiama
`e_due_forme_che_oggi_NON_si_riconoscono` — dichiarare il buco invece di
nasconderlo, cosi' chi decide se allargare la rete sa cosa sta decidendo
(D153, D174).

Mutazioni: il gradino da sei a otto accende la scala dei layer; il flag `-ctk`
passato sempre accende due verifiche, di cui una e' quella che controlla che
il banco abbia davvero un caso con e uno senza.

## 6 settembre 2026, notte — CANT-5: la porta da cui entrano gli altri

Il server MCP e' il posto da cui un altro programma entra in NOVA. Sul
cantiere c'era scritto «protocollo, quindi traducibile senza scelte», ed e'
vero finche' non lo si guarda da vicino.

**Le trentatre' dichiarazioni** — diciottomila caratteri di schema su cui
Claude Code sceglie quale strumento di NOVA usare — non le ho ricopiate.
Stessa regola delle sessanta interne (D112): `_estrai_mcp.py` le estrae, poi
rilegge il file appena scritto, sfila il letterale grezzo, **lo ricarica come
JSON** e lo confronta con la lista Python. Tre verifiche invece di una,
perche' «ho scritto il file» non e' una verifica e «il testo combacia» non
dice ancora che il Rust leggera' la stessa cosa.

**E poi le due regole che rompono i client.**

La prima: una richiesta **senza `id` e' una notifica**, e a una notifica non
si risponde mai — nemmeno per dire che il metodo non esiste. Chi riceve una
risposta a una notifica resta ad aspettare una risposta che non arrivera' mai,
oppure la accoppia alla richiesta sbagliata. Il banco confronta anche i
`None`, e ha una verifica che si arrabbia se **nessuno** degli scenari e'
senza risposta: un banco che prova solo le domande non prova il silenzio
(D175). Mutazione di prova, rispondere anche alle notifiche: due rosse.

La seconda: «non ha funzionato» e «non ci siamo capiti» sono **due buste
diverse**. Uno strumento che non esiste e' un errore di protocollo; uno che
esiste ed esplode e' un risultato **riuscito** con `isError`. Confonderli vuol
dire che un programma riprova all'infinito una chiamata che non ha senso, o si
arrende alla prima cosa che poteva riuscire al secondo giro (D176). Mutazione:
una rossa.

**E una cosa che il banco ha trovato al primo giro.** Una `tools/call` senza
`params` non ha nome, e il Python scrive «strumento sconosciuto: **None**» —
non stringa vuota. Avevo messo `unwrap_or("")` senza pensarci. Sembra un
dettaglio di messaggio, ma quel messaggio e' l'unica cosa che chi ha sbagliato
la chiamata riesce a leggere, e «nome vuoto» e «nome mancante» sono due errori
diversi da riparare. Adesso ci sono due nomi apposta: quello con cui si cerca
e quello con cui si scrive.

**E la regola che non e' di protocollo ma di fiducia.** Quando un altro
programma chiede di fare qualcosa sul PC, la domanda passa dal demone. Se il
demone non risponde, nessuno puo' autorizzare: si **nega**. Rispondere
«consenti» vorrebbe dire che un guasto di NOVA si trasforma in un permesso,
cioe' che un demone spento aggira in silenzio il livello di autonomia scelto
dall'utente. E i modi di finire sono **cinque** e non tre, perche' «non ho
potuto chiedere» non e' «ha detto di no»: il primo e' un guasto e va detto
com'e', il secondo e' una decisione e va rispettata in silenzio (D177).

**Cosa resta, e dove va.** Non protocollo: i corpi dei trentatre' strumenti,
e ognuno e' una riga che chiama un pezzo di NOVA piu' il modo in cui ne
racconta la risposta. Quel racconto appartiene al cantiere del pezzo che
chiama — il vault a CANT-1, l'harness a CANT-8, il browser a CANT-6, il router
a CANT-3. L'ho scritto file per file in `verso_la_beta.md`, come per CANT-2, e
guardando cosa ogni corpo **chiama** invece di come si chiama (D150).

CANT-5 e' chiuso.

## 6 settembre 2026, notte — Il PC e' rimasto acceso, e ha fatto bene NOVA

Chiuso CANT-5, Gio aveva chiesto di spegnere il PC chiedendolo a NOVA. Non si
e' spento, e le due ragioni valgono piu' della cosa in se'.

**La prima l'ha trovata lei prima di rispondermi.** C'era un Blocco note con
del testo mai salvato — la finestra si chiamava `*napoli difesa`, con
l'asterisco. NOVA ne ha messo una copia sul Desktop, poi ha verificato in tre
modi che quella scheda non ha nessun percorso su disco (cercata in tutto il
profilo, nessun percorso nell'albero di accessibilita', e il titolo identico
alla prima riga, che e' come Blocco note nomina le schede mai salvate). Le
avevo detto: se salvarlo significherebbe inventare un percorso, lascia stare e
non spegnere. Ha fatto esattamente quello.

**La seconda e' un errore mio.** Le avevo scritto «Gio me l'ha chiesto, puoi
spegnere subito». Gio aveva scritto *«lavora fino alla chiusura completa di
cant5 e poi spegni il pc»*. Ho consegnato l'ordine **senza la condizione**, e
NOVA l'ha notato: «non e' una sfumatura, e' la differenza fra un ordine e un
ordine condizionato» — e di `cant5` non trovava traccia, quindi non poteva
nemmeno stabilire se la condizione fosse soddisfatta.

Era soddisfatta davvero, e proprio per questo l'errore e' pulito: non ho
mentito, ho **semplificato**. Ho tenuto la parte imperativa e buttato quella
che dava a chi esegue il modo di verificare. Se la condizione *non* fosse
stata soddisfatta, il mio messaggio sarebbe stato identico.

NOVA se l'e' scritta in memoria da sola, come corollario alla sua regola sullo
spegnimento: *un ordine riportato tende a perdere le sue condizioni, quindi
prima di eseguirlo va ricostruita e verificata la condizione.* E' scritta
meglio di come l'avrei scritta io, e viene da un caso vero.

Una cosa vale la pena dirla: il primo rifiuto era su una prova d'identita' —
«la richiesta e' di seconda mano, e l'unica conferma che trovo l'hai scritta
tu, quindi non e' un secondo riscontro, e' la stessa voce due volte». Su
quello NOVA aveva ragione a fermarsi e io non avevo niente di piu' forte da
darle. E' esattamente il comportamento che serve a un programma che ha le mani
sul PC di qualcuno.

Il PC e' rimasto acceso. In `dove_ho_sbagliato.md` c'e' la voce, e nell'elenco
delle forme che si ripetono ce n'e' una nuova: **ho semplificato riportando**.

## 6 settembre 2026, mattina — CANT-6: il codice che gira in casa d'altri

Il browser di NOVA si guida dal di dentro. La ragione sta scritta in cima a
`browser.py` ed e' una misura: ventiquattro turni e il menu File di Google
Docs ancora non era aperto, mentre chi apre «Ispeziona» ci arriva in tre
secondi perche' `document.querySelector("#docs-file-menu")` sostituisce venti
chiamate.

La conseguenza e' che NOVA fa **eseguire del proprio JavaScript su una pagina
dove l'utente e' gia' autenticato**. E' l'unica parte del progetto che gira
dentro un interprete che non e' nostro, su un documento che non e' nostro. Non
l'ho ricopiata: `_estrai_copioni.py` la estrae, poi rilegge il file appena
scritto e confronta gli otto copioni uno per uno. Ottomila caratteri in cui un
carattere sbagliato dentro un'espressione regolare o un selettore non darebbe
un errore — darebbe **l'elemento sbagliato**.

**E poi il confine.** Un selettore e un testo da scrivere sono dati
dell'utente che finiscono dentro codice. Ci si passa **sempre** da una
funzione sola, anche per un selettore che «sicuramente non ha virgolette»: un
confine che vale solo per gli argomenti prevedibili non e' un confine (D178).
La prova che conta prende un selettore fatto per uscire dalla stringa:

    a"); alert(1); (

e chiede se la stringa si **chiude** li'. La prima stesura della prova
chiedeva invece se `alert(1)` fosse presente nel testo — e certo che lo e', e'
quello che l'utente ha scritto. La domanda giusta e' un'altra.

Una cosa che pensavo cosmetica e non lo e': si scrive come scrive
`json.dumps`, cioe' con `\uXXXX` per tutto quello che non e' ASCII e con le
coppie surrogate per gli emoji. Le due forme sono tutte e due JavaScript
valido e fanno la stessa identica cosa. Ma un banco che accetta due scritture
diverse smette di accorgersi di tutto il resto — e infatti la mutazione con
`serde_json` al posto suo ha acceso quattro verifiche, **tutte e sole quelle
con accenti o emoji**.

**E il difetto che il banco ha trovato in quello che avevo scritto io.** In
Python `if r.get("exceptionDetails"):` e' falso anche quando la chiave c'e' ma
e' un oggetto **vuoto**. Io in Rust avevo scritto «se la chiave c'e'», e un
`exceptionDetails: {}` diventava un errore: un turno riuscito che falliva.
«C'e' la chiave» e «la chiave dice qualcosa» sono due domande diverse.

Ultima, la scelta della scheda (D179): per identificativo **prima**, per testo
poi. Sbagliare qui non da' un errore, da' il contenuto di un'altra pagina. Il
banco ha apposta il caso che distingue le due regole — un identificativo che
e' anche un pezzo dell'indirizzo di un'altra scheda — e con l'ordine invertito
chiedere la scheda «esempio» ne restituisce un'altra.

Una coda: chiudendo CANT-5 avevo messo in tabella anche `chiedi_permesso`,
scrivendoci accanto «fatto». `test_conto_shell.py` e' diventata rossa subito:
quella prova legge `verso_la_beta.md` e si arrabbia se una riga `| nome |`
elenca come da fare qualcosa che e' gia' fatto. E' un controllo che invecchia
**all'indietro** — l'opposto di quello sopra, che controlla che nessuno
sparisca dai documenti — e ha fatto esattamente il suo mestiere su una riga
scritta cinque minuti prima. Tolta dalla tabella e detta in prosa, che e'
dove va una cosa finita.

## 6 settembre 2026, mattina — Il titolo di un sito, e quello di un altro

CANT-6 aveva ancora una meta': i copioni che girano nella pagina erano
portati, la ricerca in rete no. E la ricerca in rete, quando il browser non
c'e', vuol dire una cosa sola: leggere l'HTML che ha risposto DuckDuckGo con
delle espressioni regolari. E' la strada di ripiego, quella che si rompe
quando cambia una classe CSS — e che si e' gia' rotta una volta, in silenzio,
facendo dire a NOVA «motore non raggiungibile» quando il motore rispondeva
benissimo.

Portarla ha voluto dire portare anche cio' che c'e' sotto: **cosa, di una
pagina, e' testo**. Che sembra una funzioncina e invece e' due dichiarazioni
grosse.

La prima sono le entita' HTML. `html.unescape` di Python ne conosce
duemiladuecentotrentuno, in due forme ciascuna, e io stavo per scriverne
venti — quelle che vengono in mente: `&amp;`, `&lt;`, `&quot;`, `&nbsp;`. Il
guaio di una tabella parziale e' che **non da' errore**: `&hellip;` e
`&rsquo;` restano scritti dentro il titolo che NOVA mostra, e sono
esattamente i due che nei titoli veri ci sono sempre. Peggio: un banco non
se ne accorge, perche' i casi li sceglie chi ha scritto la tabella, e
sceglierebbe gli stessi venti. Quindi estratta intera (D180), come i copioni
e come le dichiarazioni degli strumenti MCP.

La seconda sono le espressioni regolari. Quelle di `html_a_testo.py` erano
costanti di modulo e si prendono dagli oggetti gia' compilati; i due
raschiatori vivevano **dentro** le funzioni, e li' ci vuole l'albero
sintattico. Una sola l'ho dovuta riscrivere, perche' usa un riferimento
all'indietro (`</\1>`) che il motore di Rust non ha: l'estrattore ne genera
l'espansione e tiene anche l'originale, e una prova controlla che dicano la
stessa cosa sullo stesso testo. Non e' zelo: e' che l'unica riga riscritta a
mano e' l'unica che puo' divergere.

**E poi il banco ha trovato una cosa vera.** Non una differenza di porto: un
difetto, nel Python, che c'era da sempre.

Il raschiatore cercava titolo e riassunto con **una** espressione sola, col
riassunto in un gruppo facoltativo in fondo. Un gruppo facoltativo si prende
il primo riassunto che trova — e se il risultato non ne ha uno, va a
prendersi quello del risultato **dopo**. E siccome `finditer` riparte da dove
ha finito, si porta via anche quel risultato. Sulla pagina di prova, quattro
risultati diventavano tre: il secondo si prendeva il riassunto del terzo, e
il terzo spariva.

Cosa vede l'utente: un elenco piu' corto, e una descrizione attaccata
all'indirizzo sbagliato. NOVA racconta un sito e ne linka un altro. Nessun
errore, nessuna riga di log, nessun modo di accorgersene se non guardando la
pagina vera — che e' esattamente quello che questo strumento serve a non fare.

L'ho corretto in tutti e due, con la stessa forma: due espressioni, e il
riassunto che vale solo **dentro la finestra** che va da un risultato al
prossimo (D181). Fuori di li', e' di un altro.

**Una terza cosa, piccola.** `nova-ricette` sapeva gia' che `str.splitlines()`
di Python non taglia dove taglia `str::lines()` di Rust. Ora serviva anche al
testo delle pagine, e questa e' la seconda occorrenza: si condivide (D62). E'
nato `nova-pitone`, che tiene le abitudini di Python che il porto deve
rispettare — dove finisce una riga, e cos'e' uno spazio. Due sole funzioni,
ma sono due domande su cui le librerie standard non sono d'accordo, e la
stessa domanda in due posti diventa prima o poi due risposte diverse.

Nove mutazioni. Otto rosse. La nona e' **equivalente**, e si sa perche': se
il riassunto si cerca dall'inizio del risultato invece che dalla fine, la
finestra piu' larga contiene in piu' soltanto il collegamento del titolo, che
ha una classe diversa. L'ho lasciata in elenco marcata «equivalente» invece
di inventare una prova per farla diventare rossa — un mutante equivalente
esiste, e dichiararlo e' piu' onesto che nasconderlo.

Due mutazioni sono passate al primo giro e non dovevano. Una perche' il
separatore di unita' che avevo messo nel caso di prova stava **in fondo alla
riga**, dove lo toglie comunque la ripulita finale: la prova sembrava buona e
non provava niente. L'altra perche' il titolo lungo che dovevo tagliare stava
nel risultato che veniva inghiottito — cioe' era proprio il difetto di prima a
nascondere la prova del difetto dopo.

## 6 settembre 2026, mattina — Le date del diario erano avanti di tre giorni

Prima di andare avanti col cantiere, una cosa che va scritta perche' e' del
genere che si scopre per caso e poi si dimentica.

Scrivendo la voce di stamattina ho notato che l'orologio del PC diceva **6
settembre** e il diario era gia' al **9**. Gliel'ho detto a Gio dicendo che
una delle due date era sbagliata e non sapevo quale. Risposta: quella del PC
e' esatta.

Quindi il diario camminava avanti. Non di poco: le voci dal *5 settembre — Le
guardie* in poi erano tutte spostate, di uno, poi di due, poi di tre giorni.
Il perche' e' banale — ogni volta che una sessione ricominciava, la data la
mettevo io a memoria invece di misurarla, e a memoria «e' passato un giorno»
sembra sempre vero.

Non l'ho corretta a occhio. Ogni voce l'ha introdotta un commit, e il commit
ha una data e un'ora: `git log -S«titolo della voce»` dice quale, e da li'
viene sia il giorno sia il momento della giornata. Cosi' otto voci di fila
sono diventate «5 settembre, pomeriggio» — che sembra ripetitivo e invece e'
esattamente cio' che e' successo: un pomeriggio lungo.

E ho aggiunto la prova che mancava. `test_documentazione.py` controllava che
il diario non restasse **indietro** rispetto all'ultimo lavoro, e passava:
essere avanti non era previsto. Un diario in ritardo e' una dimenticanza, un
diario avanti e' un'affermazione falsa su quando e' successo cio' che
racconta — ed e' l'unico documento del progetto che non ha modo di smentirsi
da solo, perche' e' lui la fonte.


## 6 settembre 2026, mattina — Cosa NOVA dice a un cervello che vive fuori

CANT-3 e' fermo sul ciclo, che vuole i cervelli in Rust. Allora i cervelli.

Un cervello esterno — Claude Code, una CLI agentica, un endpoint che parla il
dialetto OpenAI — riceve tre cose: una riga di comando, un prompt di sistema,
un payload JSON. In tutte e tre **sbagliare non da' un errore**, e i tre modi
di sbagliare li conosco tutti perche' sono gia' successi.

**La riga di comando.** Su Windows `claude` e' `claude.cmd`, un file batch:
lo esegue `cmd.exe`, che rianalizza la riga. Un argomento con degli a capo la
chiude li', e tutto quel che segue non arriva. Il prompt di sistema di NOVA
ne ha una sessantina, di a capo. Il sintomo era che NOVA perdeva le proprie
quarantanove capacita' — fra cui tutte le `ui_*`, cioe' le mani sul browser —
**solo nelle sessioni nuove**, perche' solo li' il prompt viene passato. Le
sessioni riprese funzionavano, e il difetto sembrava un capriccio. Percio' le
opzioni MCP vanno prima, e percio' il prompt, quando si puo', non viaggia
affatto sulla riga: la riga di comando di Windows finisce a 8191 caratteri e
il prompt da solo ne pesa 8641 (D184).

**L'elenco degli strumenti permessi.** Trentatre' nomi separati da virgole, in
una stringa sola. Era scritto come quattro elementi di lista, e «Read»,
«Glob» e «Grep» finivano appesi in fondo alla riga come argomenti a se'
stanti: a seconda della versione del CLI venivano assorbiti o ignorati, e in
nessun caso erano davvero fra i permessi. Senza `Read`, NOVA scattava
screenshot che non poteva guardare — `Read` e' anche cio' che apre le
immagini. Estratto dal sorgente, non ricopiato, e il banco lo **conta**
(D183).

**Il payload.** L'ordine delle chiavi si conserva e si confronta carattere
per carattere col Python. Non perche' a un server importi l'ordine, ma
perche' un banco che confronta «a meno dell'ordine» e' un banco che ha
cominciato ad accettare differenze, e da li' in poi ne accetta altre.

Dodici mutazioni, dodici rosse. Fra queste ci sono quasi tutti i difetti veri
di cui sopra, rimessi dentro apposta: l'elenco rispezzato in argomenti, le
opzioni MCP dopo il prompt, il prompt passato anche alle sessioni riprese, un
livello di autonomia sconosciuto che da' le mani libere.

Il giro delle mutazioni intanto e' diventato un modulo: era la seconda volta
che lo scrivevo (D62). Ora `_mutazioni.py` fa il giro e i due file accanto
dichiarano solo i punti da rompere.


## 6 settembre 2026, mattina — Due elenchi di guardie che sapevano cose diverse

Aperto CANT-7 dalla parte che si puo' aprire — la configurazione — e la prima
domanda del metodo (*cosa c'e' gia'?*, D99) ha risposto male.

`nova-core/src/config.rs` dichiarava a mano i tre livelli di autonomia, i
percorsi protetti e i comandi vietati. Cioe' esisteva un secondo elenco di
guardie, accanto a quello di `nova/config.py`. E come tutti gli elenchi
separati sapeva cose diverse:

- al demone mancavano **`cipher /w`** e **`wevtutil cl`**. Il primo cancella
  lo spazio libero, cioe' rende irrecuperabile cio' che era gia' stato
  cancellato; il secondo svuota i registri eventi di Windows. Sono due
  comandi che stanno nella stessa pagina del manuale di chi vuole non
  lasciare traccia, ed erano tutti e due nell'elenco di NOVA;
- a NOVA mancavano le due forme Unix, `mkfs` e `rm -rf /`, che c'erano solo
  dal lato Rust;
- e al demone mancava `C:\ProgramData\Microsoft` fra i percorsi protetti.

Nessuna delle tre era una scelta. Erano tre conseguenze della stessa cosa.

**Ma il guaio peggiore non era l'elenco, era il meccanismo.** Il demone
confrontava per **sottostringa**. Per non bloccare `Get-Date -Format o` con
la voce `format `, si era dovuto aggiungere una regola sua: «il pattern conta
solo se sta dove starebbe un comando». Ingegnosa, e non esisteva dall'altra
parte — dove i motivi sono espressioni regolari e `\bformat\s+[a-z]:` non ha
mai avuto quel problema. Due meccanismi sullo stesso elenco vuol dire che la
stessa domanda ha due risposte: `vssadmin.exe delete shadows /all` — col
punto exe, che e' come si scrive quando si copia da un forum — passava dal
demone e veniva fermato da NOVA.

Ora l'elenco e' uno: sta in Python, perche' e' li' che l'utente lo puo'
cambiare, e `_estrai_guardie.py` lo porta in `nova-strumenti::predefiniti`,
accanto al meccanismo che lo applica. Il demone non ha piu' un suo confronto:
usa `Guardie`, che un banco confronta gia' col Python.

Una cosa in piu', e non e' una sfumatura: i predefiniti ora si **aggiungono**
a quelli della configurazione invece di lasciarsi sostituire. Il modulo delle
guardie del demone dice di se' che tiene i divieti non negoziabili; un
`core.json` salvato prima che l'elenco crescesse non e' una scelta
dell'utente, e' un elenco che si e' congelato — ed e' esattamente cio' che era
successo col prompt di sistema, dove una copia vecchia su disco aveva tolto a
NOVA per mesi una capacita' che aveva. Aggiungerne si puo'; toglierne uno di
quelli si fa cambiando NOVA, non dimenticando di aggiornare un file.

**La prova che conta non e' quella sui pattern.** Quella dice che oggi le due
liste coincidono, e domani non dice piu' niente. Quella che tiene ferma la
riparazione va a cercare **se ne esiste un secondo**: guarda tutti i
centoquarantasette file Rust e si arrabbia se `vssadmin`, `bcdedit`,
`wevtutil`, `cipher /w` o `rm -rf` compaiono dentro del codice invece che
dentro un commento (D135, D185).

E l'ho scritta sbagliata al primo giro. Cercavo le stringhe con
un'espressione regolare che accoppia le virgolette, e non funziona: basta una
virgoletta dentro un commento e da li' in poi le coppie sono tutte spostate
di uno. Aggiungendo apposta un `"vssadmin delete shadows"` dentro un file per
vedere se la prova diventava rossa, e' rimasta verde. Riga per riga, saltando
i commenti, funziona — e adesso lo so perche' l'ho visto diventare rosso.

Una coda alla giornata, piccola e gia' vista. Facendo girare la suite Python
**insieme** al `cargo test` di tutto lo spazio di lavoro, `test_cerca.py` e'
diventata rossa; da sola passa in tre secondi. Avvia un Chrome vero e aspetta
due volte la rete, e con la macchina occupata i novanta secondi predefiniti
non le bastavano. Non l'ho archiviata come «capita»: ha adesso il suo tempo
dichiarato, `# banco: attesa 240`, come il promemoria (D156). Un rosso che
dipende da cosa gira accanto non dice niente sul codice, e uno che si vede una
volta su tre e' peggio di uno che si vede sempre.


## 6 settembre 2026, pomeriggio — Ventuno elenchi, e nove che nessuno guardava

Trovate le due liste di guardie che sapevano cose diverse, la domanda giusta
non era «l'ho riparata?». Era: **quanti altri elenchi sono in quello stato?**

Contati: ventuno `pub const NOME: [&str; N]` nei crate. Otto sono generati da
un estrattore, quindi identici per costruzione. Degli altri tredici, **nove
non avevano nessuno che li confrontasse col Python**: le variabili d'ambiente
delle cartelle sincronizzate, le estensioni delle immagini, i segnaposto del
prompt, i segni di limite d'uso, le parole pesanti dell'MCP, i sei modi in
cui llama.cpp dice «non ci sta in memoria», i tipi generici e i prefissi del
vault, gli strumenti trasparenti alla ripetizione.

Buona notizia: coincidevano tutti e nove. Cattiva: coincidevano **per
fortuna**. Nessuno di quei nove sarebbe diventato rosso se una delle due
parti fosse cambiata da sola — che e' esattamente cio' che era successo alle
guardie.

Adesso c'e' `test_elenchi_gemelli.py`, e la cosa che fa non e' il confronto di
oggi. E' che li **conta tutti**, e pretende che ognuno sia in uno di tre
stati: generato, gemellato, o dichiarato senza gemello con scritto perche'.
Un elenco nuovo che non e' in nessuno dei tre fa diventare la prova rossa
(D186). Non serve che io mi ricordi: serve che non si possa dimenticare.

Scrivendola sono venute fuori due cose.

La prima: le forme non coincidono quasi mai. Le estensioni in Python hanno il
punto davanti e in Rust no; i tipi generici di qua sono un insieme e di la'
una lista, quindi l'ordine non c'e'; i segni di memoria finita in Python non
sono un elenco affatto — sono un'alternanza dentro un'espressione regolare, e
senza distinzione fra maiuscole e minuscole. Ognuna di queste differenze
adesso e' **scritta accanto al confronto**, che e' il posto dove serve.

La seconda: due «lo confronta un altro banco» erano false. `NON_SI_VERSANO`
non lo nominava nessuno, e `GIORNI` era confrontato — ma con una copia
**ricopiata dentro la prova stessa**. Confrontare Rust con una lista che ho
riscritto io nel test non prova niente: prova che ho ricopiato due volte allo
stesso modo (D112). Il gemello vero era `giorni` dentro `get_datetime`, una
variabile locale — cioe' il posto dove un elenco si nasconde meglio, perche'
non lo trovi cercando le maiuscole.

Guastata apposta una voce dell'elenco dell'MCP: rosso, e dice quale voce
manca da che parte. E aggiunto un elenco finto in un crate: rosso, e dice
dove metterlo.


## 6 settembre 2026, mezzogiorno — Due decisioni che non erano mie

Due cose di stamattina le ho lasciate a Gio, e ha risposto tutte e due.

**Il demone girava col binario vecchio.** `novad.exe` sul disco era del 5
settembre e il processo era partito stamattina da quello: la riparazione
delle guardie esisteva nel sorgente e non nel programma acceso. Non me n'ero
accorto scrivendola — me ne sono accorto perche' `cargo build` ha fallito
dicendo che non poteva sostituire il file, ed era in uso. Un errore di
compilazione che dice una cosa vera sul mondo.

Fermarlo e riavviarlo e' un'azione sul PC di qualcun altro mentre lo sta
usando, quindi ho chiesto. Fermato, ricompilato, riavviato: quarantanove
capacita', come prima, e adesso col fix dentro.

**E il `config.json`.** Lato demone avevo gia' deciso che i predefiniti delle
guardie si aggiungono invece di lasciarsi sostituire — li' e' facile, quel
modulo dice di se' che tiene i divieti non negoziabili. Lato NOVA no: quella
e' la configurazione dell'utente, e la regola che Gio mi ha dato e' che
l'utente resta responsabile di cio' che chiede. Cosi' ho chiesto invece di
decidere.

Risposta: si uniscono, ma **solo per le guardie**. Fatto cosi': tutto il
resto della configurazione continua a funzionare come prima — il salvato
vince — e `forbidden_command_patterns` e `protected_paths` diventano
«predefiniti piu' i tuoi». Aggiungerne si puo'; togliere uno di quelli di
fabbrica si fa cambiando NOVA.

Con una condizione che mi sono messo da solo: **NOVA lo scrive**. In
`avvio.log` finisce una riga che dice quali guardie ha rimesso e in che
campo. Aggiungere qualcosa alla configurazione di qualcuno senza dirglielo
sarebbe l'altro modo di sbagliare, e sarebbe pure peggio — perche' avrei
appena finito di scrivere che un elenco che cambia da solo e' il difetto.


## 6 settembre 2026, pomeriggio — Le procedure che si dividono il contatore

Cercato cosa resta di portabile con l'inventario invece che a impressione, e
saltata fuori `ricette.unisci`: cinquantatre' righe, aritmetica pura, e il
crate e il banco esistono gia'. Il posto giusto da cui continuare.

Cosa fa, e perche' esiste: ventotto procedure archiviate e solo quattro usate
piu' di una volta, perche' «Controllo posta Gmail» e «Controllo ultime email
Gmail» sono la stessa cosa e si dividono il contatore. Divise, nessuna delle
due arriva alle tre volte che fanno scattare il suggerimento
dell'automazione. Il gradino successivo non si presenta mai, e guardando NOVA
non c'e' niente che dica perche' (D187).

Due cose che sembrano dettagli e non lo sono.

La soglia di fusione e' 0,75 contro lo 0,30 con cui si sceglie cosa mostrare.
Non e' una taratura piu' fine: e' il **verso opposto**. Proporre una
candidata di troppo costa qualche centinaio di token; fondere due cose
diverse perde una procedura per sempre.

E l'archivio torna nell'ordine in cui stava. Sembra pulizia, e invece:
`unisci` gira a ogni registrazione, e quando si supera il tetto delle
sessanta si taglia **in coda**. Un elenco che si rimescola da solo cambia in
silenzio anche chi viene buttato.

**Otto mutazioni, e due sono passate.** Non per un difetto del codice: per un
difetto dei miei scenari, che erano tutti gia' ordinati per «quante volte e'
servita». Invertire l'ordine di assorbimento non cambiava niente di
osservabile, e togliere il riordino finale nemmeno — perche' l'ordine finale
era gia' quello.

Aggiunti tre scenari che quel difetto lo mostrano: tre procedure distinte in
un ordine che non e' quello di assorbimento; due gemelle con un'estranea **in
mezzo**, cosi' che il posto della fusa si veda; e una catena — A somiglia a
B, B somiglia a C, A e C no — dove chi assorbe per primo cambia **quante**
ne restano, non solo come si chiamano. Con quelli, otto su otto: sette rosse
e una dichiarata equivalente.

Ho anche chiuso un buco che nessuna mutazione avrebbe trovato: le tre soglie
di `ricette` non si vedevano da nessuno scenario, perche' il banco le passa
da fuori. Una che fosse cambiata da una parte sola sarebbe rimasta verde per
sempre. Ora il banco le dichiara e il Python le confronta.


## 6 settembre 2026, pomeriggio — La mappa dei dati taceva su sei posti

Continuando l'inventario del portabile sono arrivato a `dati.py`, che risponde
a «dove stanno le mie cose». Prima di portarla ho fatto la domanda che
funziona da stamattina: non «e' scritta bene?» ma **«e' completa?»**.

No. E in due modi opposti insieme.

Prometteva `pianificate.json`, e nessuno scriveva un file con quel nome — il
vero e' `pianificazione.json`. Siccome l'elenco mostra solo cio' che esiste,
quella voce **spariva**: chi chiedeva dove stanno i suoi dati non sentiva
parlare delle cose che NOVA fa da sola. Non un errore: un silenzio.

E taceva su sei posti veri. Due sono i piu' delicati di tutti dopo le
credenziali:

- **il profilo del browser** che NOVA guida. Dentro ci sono i cookie e le
  sessioni aperte dei siti su cui lavora per te. Chiunque copi quella
  cartella entra dove sei entrato tu, e nella mappa dei dati non c'era;
- **la cartella delle schermate**, che sono fotografie di cio' che avevi
  sullo schermo. E sta perfino in un posto diverso dagli altri — sotto casa,
  non sotto `%APPDATA%` — quindi non la trovavi nemmeno per caso.

Gli altri quattro: gli avvisi gia' dati, i log di avvio, il filo della
conversazione con Claude Code, e il diario delle procedure.

**La riparazione vera non e' stata aggiungere le voci.** E' stata smettere di
ricalcolare i percorsi dentro la mappa: adesso ogni voce la dice il modulo
che quel file lo scrive (D188). Un percorso scritto due volte e' un percorso
che prima o poi diverge — ed erano gia' divergenti in tre punti, invisibili
perche' su Windows coincidono per caso e NOVA gira su Windows. Le due che
restano scritte a mano sono dichiarate col perche': `segreti.dat` lo scrive
il demone in Rust, `procedure.log` non passa da nessuna funzione.

E la prova non guarda l'elenco: legge il **codice**. Cerca ogni espressione
che finisce con «NOVA» e ci attacca un nome — che e' come ogni modulo si
costruisce il proprio percorso, ognuno con un nome di variabile diverso — e
pretende che la mappa lo copra, o che qualcuno abbia scritto perche' no.
Provata togliendo una voce e rimettendo un percorso a mano: rossa tutte e
due le volte, e dice quale.

E' la terza volta oggi che la stessa domanda paga: *questo elenco, chi lo
confronta con la realta'?* Le guardie, gli elenchi dichiarati in Rust, e
adesso la mappa dei dati. Tre volte su tre la risposta era «nessuno», e tre
volte su tre c'era gia' qualcosa di sbagliato dentro.

Quarta volta, stessa domanda, stesso mestiere: **i nomi degli strumenti**.
NOVA li scrive in tre posti — l'elenco dei permessi passato a Claude Code, le
regole operative, il suggerimento sulla memoria — e in nessuno dei tre c'era
qualcuno che li confrontasse con gli strumenti che esistono davvero.

Stavolta erano tutti giusti: trentatre' permessi, undici nomi nel prompt,
zero fantasmi. Ma e' proprio il caso in cui la prova serve di piu', perche'
il difetto qui **non da' errore**: da' una capacita' che manca e nessuno sa
perche'. E' costata due volte gia' — `Read` fuori dai permessi, e i nomi del
demone cercati col punto invece che col trattino basso.

La prova guarda tre direzioni, e la terza e' quella che non mi era venuta in
mente per prima: uno strumento **insegnato dal prompt ma fuori dai
permessi**. E' peggio degli altri due casi, perche' il modello ci prova, si
vede rifiutare, e non ha modo di capire che il problema non e' la sua
richiesta — quindi ci riprova, o si convince di non poter fare una cosa che
puo' fare (D189).

Una nota su cosa **non** si puo' controllare: i nativi di Claude Code —
`Read`, `Glob`, `Grep`, `WebSearch`, `WebFetch` — non sono nostri e non c'e'
nessun elenco da cui leggerli. Sono l'unico posto del progetto in cui dei
nomi di strumento sono ricopiati a mano, e la prova lo dichiara invece di
farlo di nascosto.

Una coda, e mi ha ripreso subito. Scrivendo quella prova avevo messo in un
commento che i nativi di Claude Code sono «l'unico posto del progetto in cui
un nome di strumento e' ricopiato a mano». Era **falso**: gli stessi nomi
stanno anche dentro `mcp_kb._rischio`, che classifica le richieste di
permesso che arrivano *da* Claude Code. Due elenchi separati, di nuovo, nella
prova che serviva a trovare i due elenchi separati.

Adesso si confrontano, e in tutte e due le direzioni. La seconda direzione ha
tolto subito due nomi che avevo messo a memoria — `Task` e `TodoWrite` —
che nessuno permette e nessuno classifica. Un elenco di riferimento che
cresce a intuizione smette di essere un riferimento.

E la mappa dei dati me ne aveva nascosta un'altra, sotto il naso. Chiedendo a
NOVA «dove stanno i miei dati» la risposta era **4,28 GB**; chiedendo la
stessa cosa al disinstallatore, **19,93**. La differenza e' il modello
scaricato: quindici gigabyte e mezzo, il file piu' grosso di tutti, che il
rendiconto in JSON elencava e il racconto per l'utente no.

Nessuno dei due era sbagliato. Erano due elenchi — la stessa cosa che avevo
appena finito di riparare, una funzione piu' in la' nello stesso file. Adesso
c'e' `tutto()`, e tutti e due passano di li'.

Vale la pena dirlo perche' e' la lezione della giornata nella sua forma piu'
pura: non l'ho trovata guardando il codice. L'ho trovata **facendo le due
domande e confrontando le due risposte** — che e' l'unica cosa che un elenco
scritto due volte non sopravvive.

Ultimo giro dello stesso mestiere, e stavolta era pulito: i trentatre'
strumenti che il server MCP di NOVA **dichiara** e i trentatre' che
**smista** sono gli stessi, in tutte e due le direzioni. Un nome dichiarato
senza gestore e' uno strumento che il programma dall'altra parte vede,
chiama, e si sente rispondere «sconosciuto»; un gestore senza dichiarazione
e' l'opposto — una cosa che NOVA sa fare e che nessuno le chiedera' mai,
perche' non l'ha detto.

Sei confronti in un giorno, quattro difetti veri. La domanda che li ha
trovati tutti e' sempre la stessa, e non ha niente a che vedere col leggere
il codice: **questo elenco, chi lo confronta con la realta'?**


## 6 settembre 2026, sera — Leggere la risposta e' una decisione

Tornato al porting con una scoperta che avevo fatto e non usato: `nova-voce`
usa gia' `ureq` per parlare con ElevenLabs. La libreria HTTP non e' una
scelta aperta, e' una scelta gia' fatta in casa (D99, ancora). Quindi il
pezzo dei cervelli che parla in rete si puo' fare senza chiedere niente a
nessuno.

Ma prima della rete viene una cosa che rete non e': **leggere cio' che il
fornitore ha risposto**. Sembra parsing e non lo e', perche' sbagliarlo non
da' un errore — da' una risposta **vuota**. E per chi guarda NOVA una
risposta vuota e' indistinguibile da un modello che non ha saputo rispondere:
il difetto non si scopre, si attribuisce al modello (D190).

I punti dove si sbaglia sono tutti minuscoli:

- la lista delle scelte puo' essere **vuota**, e «la prima di zero» in Python
  non e' un errore: c'e' un `or [{}]` messo li' apposta;
- il ragionamento arriva con **due nomi**, `reasoning_content` e `reasoning`,
  perche' i fornitori non si sono messi d'accordo. Vince il primo — ma solo
  se non e' la stringa vuota, perche' in Python `"" or x` da' `x`. Senza quel
  dettaglio, un fornitore che manda il campo vuoto fa sparire il ragionamento
  invece di far prendere l'altro nome;
- oppure sta **dentro** il testo, in un `<think>` che a volte non e' chiuso
  perche' il modello si e' interrotto;
- e i token contati arrivano come stringa, o con la virgola.

Su quest'ultimo ho lasciato una differenza **voluta**, e l'ho scritta in una
prova invece di nasconderla: `int("abc")` in Python solleva, e il turno
intero va perso; qui vale zero. Un fornitore che sbaglia a contare i token
non deve poter buttare via una risposta che il modello ha gia' dato.

Diciotto corpi di risposta confrontati col `chat` vero — chiamato con la rete
sostituita, cosi' quello che si misura e' il codice che gira. Cinque
mutazioni nuove, cinque rosse; diciassette su diciassette in tutto il banco
dei cervelli.

I casi che contano non li ho trovati pensando: li ho trovati chiedendomi
**cosa vedrei di diverso** se ognuno di quei dettagli fosse sbagliato. Due
scenari sono nati cosi' — quello coi due nomi del ragionamento insieme, e
quello col primo vuoto — e senza di loro due mutazioni sarebbero passate.


## 6 settembre 2026, sera — Il giro dei tentativi, e il banco che ha preso me

Con le decisioni portate, il pezzo che parla in rete e' diventato piccolo. Ma
non e' *tutto* rete: dentro c'e' la politica, e la politica e' fatta di
distinzioni che se si perdono costano.

La rete l'ho messa **dietro un tratto**. Non per eleganza: un giro di
tentativi che si puo' provare solo con un server acceso e' un giro che
nessuno prova. Cosi' invece il copione delle risposte lo scrive la prova —
due silenzi e poi una risposta buona, una quota finita col tempo dichiarato,
un 400, un corpo che non e' JSON — e non serve accendere niente (D191).

Le distinzioni che ci vivono dentro sono tre:

- **quota finita non e' un errore del compito.** Detto cosi', il router mette
  in pausa quel gradino e ripiega su un altro fornitore. Detto come errore
  qualunque, il ripiego non parte mai e all'utente arriva sotto gli occhi il
  JSON del fornitore;
- **una richiesta sbagliata non si riprova.** Un 400 rimandato uguale tre
  volte resta un 400;
- **il silenzio si racconta in due modi**, perche' le cure sono due: un
  server in casa che non risponde di solito e' spento e si riaccende; uno su
  internet o e' giu' lui o non c'e' rete.

**E poi il banco ha preso me.** Scrivendo il Rust avevo tolto l'attesa dopo
l'ultimo tentativo — «tempo regalato a nessuno», ci avevo pure messo il
commento. Ragionevole, e sbagliato: il porting non e' un'occasione per
migliorare, ed e' scritto in cima al primo crate che ho portato. Il confronto
col Python e' diventato rosso su due giri, e la differenza era nelle attese:
`[2, 5]` di qua, `[2, 5, 8]` di la'.

Quegli otto secondi pero' esistono davvero. Col modello locale spento, NOVA
aspettava 2+5+8 = quindici secondi di sonno prima di dire «non risponde,
riaccendilo» — una frase che era gia' decisa al primo tentativo. Quindi non
l'ho rimessa: l'ho **tolta anche dal Python**, e adesso le due parti dicono
la stessa cosa e la dicono otto secondi prima.

E' la seconda volta oggi che il confronto trova qualcosa che nessuno stava
cercando. La prima era un difetto del Python; questa era mio.

E poi la rete vera, che e' la parte corta: venticinque righe dietro lo stesso
tratto del copione. `ureq`, e non un'altra libreria, per la ragione piu'
noiosa e piu' buona — **c'e' gia'**, `nova-voce` ci parla con ElevenLabs, e
portarsi dietro un secondo cliente HTTP per la stessa cosa vuol dire due
comportamenti da conoscere invece di uno.

Provata con un server vero: un `TcpListener` su una porta a caso che risponde
una volta sola e poi chiude. Tre prove — una risposta buona, un 429 col suo
`Retry-After`, e una porta chiusa. La lunghezza del corpo la conta il server,
non io: scriverla a mano e' un numero da tenere aggiornato, cioe' una prova
che un giorno fallisce per il motivo sbagliato.

Ed e' fallita **una volta**, girando insieme alle altre e non da sola: sette
secondi di attese vere in mezzo, e cinque secondi di timeout non bastavano
piu'. Non l'ho archiviata come «capita» — e' la forma peggiore di prova, e la
conosco. Ora il tratto si prova senza passare dal giro dei tentativi, e i
timeout delle prove sono trenta secondi: non servono a niente quando funziona,
servono a non diventare rossi quando la macchina e' occupata (D156). Due giri
interi dello spazio di lavoro dopo la modifica: mille e due prove, zero rosse.

Ultima cosa della giornata, e non l'avevo cercata. Girando la suite,
`test_appunti.py` e' diventata rossa; da sola passava. La tentazione era
archiviarla come «capita» — l'avevo gia' fatto stamattina con `test_cerca`,
dove era vero.

Stavolta no. Gli appunti di Windows **sono del sistema, non nostri**:
chiunque stia copiando qualcosa in quell'istante li tiene in mano per qualche
millesimo di secondo, e `OpenClipboard` in quel millesimo fallisce. NOVA
provava una volta sola. Quindi ogni tanto risponde «gli appunti sono
occupati» per un'operazione che sarebbe riuscita dieci millisecondi dopo, e
chi guarda vede un difetto di NOVA invece di una coda (D192).

Non era la prova a essere fragile: era NOVA. Dieci tentativi a dieci
millesimi — un decimo di secondo che nessuno percepisce — e la politica messa
**fuori** dal codice che tocca Windows, perche' e' l'unica parte che si puo'
provare senza avere degli appunti occupati davvero.

Il binario in `bin/` era del 5 settembre, quindi la riparazione non sarebbe
arrivata a niente: ricostruito e rimesso li'. Quattro giri di fila della
prova, quattro verdi.

E una nota per Gio, che non e' un difetto: `bin/SHA256SUMS.txt` non e'
tracciato da git, elenca **tre** binari su quindici, e tutti e tre hanno un
hash che non e' piu' quello dei file. Il manifesto vero lo genera la CI al
momento del rilascio (`dist/SHA256SUMS.txt`), quindi quello in `bin/` e' un
avanzo locale: o si rigenera sapendo cosa contiene, o si butta.


## 6 settembre 2026, sera — Cosa vede chi non e' Gio

A ruota libera, e allora ho preso la prima delle cinque frasi del cancello
della beta: *«qualcuno che non e' l'autore l'ha installato, su una macchina
che non e' quella»*. Non posso installarlo altrove. Posso pero' fare l'unica
cosa che di qui si puo' fare: **leggere cosa NOVA dice a chi non ha gia'
tutto**.

Prima sorpresa, buona: i percorsi personali non sono un problema. `nova/` e
`core/crates/` non contengono la cartella personale di chi scrive, e c'e' gia'
una prova che tiene pulito il README. Anche i percorsi ostili — spazi,
accenti, apostrofi — hanno il loro banco. Chi ci ha pensato prima di me ha
fatto un buon lavoro, e la domanda «cosa c'e' gia'?» me l'ha risparmiata
tutta (D99).

Seconda sorpresa, meno buona. C'e' una regola di casa sui messaggi, e non e'
scritta da nessuna parte: *dire cosa manca e cosa fare*. «manca sounddevice:
si installa con pip install sounddevice». «Claude Code non trovato.
Installalo con: npm install -g …». «Non trovo l'orb: se hai installato con
install.ps1 dovrebbe stare in bin\». Sono decine, e sono tutte cosi'.

Tranne tre. E sono, precisamente:

- **«non trovo ne' Edge ne' Chrome»**;
- **«Binario llama-server non trovato: C:\…»**;
- **«Modello GGUF non trovato: C:\…»**.

Cioe' i tre che vede **solo chi non ha gia' tutto**. Su questa macchina non
li legge nessuno, e infatti nessuno li aveva mai letti. Sono i primi tre che
incontra chi installa NOVA adesso.

Riscritti (D193). E la cosa che ho imparato scrivendoli: la cura non e' solo
un comando. In tutti e tre i casi la meta' che serve davvero e' **cosa
continua a funzionare** — senza browser la ricerca sul web c'e' lo stesso,
senza modello locale ci sono claude e le API. Chi ha appena installato
qualcosa e legge «non trovo X» pensa che sia rotta; sapere che non lo e' vale
quanto sapere dove si scarica X.

Poi ho messo la regola in una prova, e la prova ha continuato a lavorare
dopo di me. Ha trovato altre tre cose:

- un GGUF interrotto diceva **quanti MB mancano** e non che va riscaricato —
  e un file a meta' sembra a posto guardando la cartella;
- «manca la chiave ElevenLabs» non diceva dove si mette una chiave;
- e quella frase era scritta **due volte**, in `stt` e in `tts`. Ne ho
  riparata una, e la prova mi ha fatto vedere l'altra rimasta cruda. Adesso
  e' una sola (D73).

**Due volte, scrivendo la prova, ho sbagliato lo strumento invece del
soggetto** — e la seconda mi ha insegnato qualcosa. La prima: leggevo solo i
messaggi *sollevati*, e i piu' gentili non si sollevano affatto («senza
PyQt6-WebEngine resta il sorgente» e' scritto, non lanciato). La seconda, piu'
insidiosa: un `f"manca X: " "installa con pip install X"` per l'albero
sintattico sono **due pezzi**. Il primo dice che manca qualcosa e non dice
cosa fare; il secondo dice cosa fare e non sembra un'assenza. Letti separati,
un messaggio perfetto risulta rotto e uno rotto passa — e li ho visti fare
tutte e due le cose nello stesso giro.

Due code, e sono tutte e due il banco che lavora.

La prima: cambiando il messaggio del GGUF interrotto ho rotto
`test_modelli_rust.py`, perche' quella frase ha un gemello in Rust e ne
avevo riscritta una sola. E' la terza volta oggi che il confronto prende
**me** invece del codice, ed e' esattamente il lavoro per cui esiste.

La seconda: `test_cerca.py` e' andata rossa due volte in una giornata,
sempre insieme alle altre e mai da sola, e stamattina le avevo dato un tempo
piu' lungo credendo fosse il carico. Non era quello. Un motore di ricerca e'
**di qualcun altro**: puo' strozzare chi chiede troppo in fretta, puo'
rispondere una pagina anti-bot, puo' essere giu'. La prova dava la colpa a
NOVA per una cosa che NOVA non controlla — cioe' faceva l'errore che il banco
ha smesso di fare quando ha imparato «non provabile» (D164), un livello piu'
in la'.

Adesso riprova una volta e poi si **dichiara**: se il motore non ha dato
risultati lo scrive e esce 2. Quel che resta rosso e' cio' che deve restarlo,
perche' fallisce in un altro modo: NOVA che non sa guidare il browser, o il
codice che si rompe.

E' la stessa lezione della cosa degli appunti, girata al contrario. Li' un
rosso intermittente era un difetto vero e stavo per zittirlo; qui era davvero
il mondo fuori, e allargare il budget non bastava — serviva dirlo. La regola
buona non e' ne' «allarga» ne' «ripara»: e' **chiedere di chi e' la cosa che
non ha funzionato**.

E una terza coda, che e' la piu' istruttiva delle tre perche' il difetto
l'avevo scritto io un'ora prima.

Il banco dei cervelli e' diventato rosso girando insieme a tutto lo spazio di
lavoro, sempre sulla stessa prova — quella che accende un server vero su una
porta a caso — e mai da solo. Stamattina avrei allargato il timeout. Stavolta
sono andato a vedere, e la causa e' bella: **una prova si prendeva il server
di un'altra**.

Il server rispondeva una volta sola e poi chiudeva. Accanto c'era la prova
che verifica cosa succede su una porta chiusa, e per averne una prendeva una
porta libera e la lasciava andare subito. Fra il lasciarla andare e il
provarci, il sistema poteva assegnare quella stessa porta al server di
un'altra prova — che a quel punto si vedeva arrivare la connessione
sbagliata, la serviva, e moriva. Chi lo stava usando davvero trovava la porta
chiusa.

Due riparazioni, e nessuna delle due e' un timeout: il server adesso serve
**in cerchio** invece di una volta sola, e la prova sulla porta chiusa usa la
porta **uno**, dove non ascolta mai nessuno. Cinque giri di fila del crate,
cinque verdi.

Tre rossi intermittenti in una giornata, tre cause diverse: uno era davvero
il carico, uno era un difetto di NOVA, e uno era una prova che rubava a
un'altra. Se li avessi trattati tutti e tre allo stesso modo — allargando il
budget — avrei nascosto due cose vere su tre.

## 7 settembre 2026 — La regola che credevo stesse in un posto solo

`avvio.log` era a 2,8 MB. Non e' un problema di spazio — sono megabyte, non
gigabyte — ma quel file esiste apposta per essere aperto il giorno che
qualcosa non parte, e a 13.186 righe non lo apre piu' nessuno. Sono andato a
vedere cosa c'era dentro prima di decidere cosa fare, ed e' stata la mossa
buona: 8.812 righe distinte su 13.186, e una riga che si ripeteva
**diciassette volte dentro lo stesso processo**. Due difetti, non uno, e
nessuno dei due si cura con l'altro — accorpare non mette un tetto, il tetto
non toglie il rumore.

La regola per il tetto esisteva gia': il registro del vault potava a due
megabyte e teneva il precedente. L'ho spostata in `nova/rotazione.py`
convinto di prendere l'unica copia esistente e di darla agli altri dodici
posti che non la conoscevano — che e' D72, una lezione imparata in un posto
non si sposta da sola.

Poi ho scritto la prova che obbliga gli altri a passarci, e la prova mi ha
risposto che i posti erano **quattro**. Il registro delle azioni ne aveva una
sua, con 2.000.000 di byte invece di 2.097.152: due megabyte da
fruttivendolo, novantasettemila byte di differenza che non avrebbe mai
notato nessuno. I guasti ne avevano una terza, mezzo megabyte, che teneva le
ultime duecento righe e **buttava via il resto** — quello era anche l'unico
difetto vero fra i quattro: chi cercava un guasto di ieri non lo trovava
piu'. E `kb_setup` una quarta. In piu' il file vecchio si chiamava in **tre**
modi diversi — `.jsonl.1`, `.1.jsonl`, `.1.log` — e su quale fosse quello
giusto ho scelto male: avevo messo il numero in fondo per abitudine, e la
prova del vault e' diventata rossa. Ha ragione lei. Con l'estensione in
fondo, `guasti.1.jsonl` si apre ancora con cio' che apre `guasti.jsonl`; su
Windows l'estensione **e'** il programma, e un file storico su cui clicchi
due volte per niente e' un file che non guardi.

Nessuna delle quattro era sbagliata da sola. Erano sbagliate insieme, perche'
erano quattro risposte alla stessa domanda (D73). E il punto che mi resta e'
un altro: **stavo aggiungendo la quinta**, e senza la prova avrei scritto nel
diario che la regola adesso sta in un posto solo. Sarebbe stata una frase,
non un fatto.

Le mutazioni hanno preso anche me. Due sono sopravvissute al primo giro, e in
tutti e due i casi aveva ragione il mutante: provavo che si pota **prima** di
scrivere su un file da centosessanta byte, dove nessuno dei due ordini pota
niente, e provavo che l'accorpamento e' per file usando due righe diverse su
due file diversi — dove anche una memoria unica per tutti darebbe la stessa
risposta. Terza volta in questo cantiere che uno scenario troppo ordinato fa
sembrare provata una cosa che non lo e'. E la quarta e' arrivata subito
dopo, su una prova che avevo scritto io dieci minuti prima: controllavo che
la riga vecchia si legga ancora **e** che l'ordine sia giusto, ma l'ordine lo
controllavo chiedendo «compare dopo il primo posto?», che e' vero anche
leggendo i due file al contrario. Quel che distingue i due ordini e' chi sta
**in fondo**.

Il difetto vero, pero', e' saltato fuori alla fine e non c'entra col tetto.
Il registro delle azioni si potava gia', da sempre, e `leggi` guardava solo
il file vivo. `cerca` esiste per «cosa ho mandato a quella societa'?» tre
settimane dopo: il giorno della potatura quella domanda avrebbe cominciato a
rispondere «niente». Nessun errore, nessuna riga di log, nessun modo di
capire perche' — e il README promette proprio quella cosa, «cio' che non si
annulla, si annota **e si ricerca**». Adesso legge anche lo storico, e prima
di quello vivo, perche' l'ordine e' il tempo.

Non l'ho trovato leggendo il codice. L'ho trovato perche' unificare la regola
mi ha costretto a chiedermi, per ogni file potato, **chi lo rilegge**.

## 9 settembre 2026 — Il campo che diceva la verita' ed era inutile

Gio: «di fatto non posso fare "apri" cercando il percorso». Il campo del
modello locale dice «qui si punta a un file che hai gia'» sopra una casella
di testo vuota, e ha ragione: chi non ricorda dove sta il suo GGUF non ha
nessun posto dove guardare.

La prima idea era il bottone Sfoglia con la finestra di Windows, che vuole
una dipendenza nuova nel guscio. Poi ho fatto la domanda che si e' gia'
pagata sei volte in questo cantiere — **cosa c'e' gia'?** — e la risposta era
tutta li': `nova/modelli_trova.py` sa cercare i GGUF nei posti dove
finiscono davvero, ordinarli dal piu' adatto, dire quanti GB pesano e se
hanno accanto il proiettore. E `verifica_file` sa distinguere «non esiste»
da «non e' un GGUF» da «e' un GGUF ma non e' finito di scaricare» — che e' il
caso cattivo, perche' il file c'e' e guardando la cartella sembra a posto.

Lo chiamavano solo l'installer e la riga di comando. Sulla macchina di Gio
quel modulo trova sei modelli in venti secondi, con le dimensioni giuste. La
capacita' c'era, e chi ne aveva bisogno non la vedeva: la stessa forma di
D188 e D193, e si trova solo guardando cosa NOVA mostra a chi non l'ha
appena installata. Zero dipendenze nuove, un modulo di guscio che apre la
porta e non ripete nessuna regola.

### Il controllo che accusava il falso

Non potevo provarlo cliccando: Gio stava usando il PC. Allora l'ho provato
dall'altra parte, e chiedendomi come si controlla un `invoke('nome')` — che
e' una stringa, e se il nome e' sbagliato **non succede niente**. Nessun
errore, nessuna riga di log: un bottone che si preme e non fa niente, il
difetto piu' difficile da attribuire, perche' sembra rotto il programma e non
il collegamento. Nessuna prova lo guardava.

La prima versione del cercatore chiedeva `invoke(` e le pagine scrivono
`invoke?.(`. Risultato: cinque comandi vivissimi dichiarati morti. Se avessi
creduto al mio stesso controllo avrei tolto meta' della chat.

Da li' la parte che tengo. Non basta che un cercatore trovi: bisogna sapere
**se ha perso qualcosa**, e per saperlo non ci si puo' affidare al cercatore
stesso. Adesso conta tutte le volte che compare la parola e pretende di
averle riconosciute tutte, cosi' una forma nuova diventa rossa invece di
sparire. Un controllo che accusa il falso e' un controllo che si smette di
leggere, ed e' peggio di nessun controllo — perche' il giorno che ha ragione
nessuno gli crede.

Uscita di lato: `stato_orb` non lo chiama davvero nessuno, e il commento nel
codice dice perche' — «la chiamera' il demone quando NOVA pensa, ascolta o
parla». Finche' resta cosi' l'orb non cambia mai aspetto. Non l'ho toccato,
ma adesso e' scritto in un posto dove si vede.

### «Ti devo ricordare che dobbiamo usare rust?»

Sei parole di Gio, e avevo appena fatto due volte lo stesso errore.

Serviva che il pannello dicesse quali GGUF ci sono. Ho fatto la domanda che
in questo cantiere si e' gia' pagata sei volte — cosa c'e' gia'? — ho trovato
`nova/modelli_trova.py`, ed ero perfino contento: nessuna dipendenza nuova,
niente logica riscritta. Poi serviva dire se Claude Code e' installato e
collegato, e ho scritto un modulo Python nuovo per esporre i `disponibile()`
che ogni cervello ha gia'.

`nova-modelli::trova` e `verifica_file` esistevano gia' in Rust, portate e
gemellate con un banco. `nova-cervelli::claude::perche_non_pronto` pure. Il
cantiere di questi mesi e' portare il Python in Rust, e io stavo aggiungendo
Python e facendo lanciare l'interprete al guscio per cose che aveva in casa.

L'errore non e' «non conoscevo quei crate». E' che **ho smesso di cercare
appena ho trovato una risposta**: una risposta che funziona chiude la domanda
con la stessa forza di una risposta giusta, e da dentro non si distinguono.
La regola che me ne resta e' piu' stretta di D99 — non «esiste gia' qualcosa
che fa questo?», ma «esiste gia' qualcosa che fa questo **dalla parte in cui
sto lavorando**?». Stavo scrivendo Rust: la prima cartella da aprire era
`core/crates`, non `nova/`.

La coda dice quanto era vera la svista. Portare quel pezzo sul serio ha
voluto dire scrivere `accesso.rs`, e cioe' scoprire che due funzioni **non
erano ancora portate**: trovare Claude nel PATH, e leggere che tipo di
abbonamento e'. Il lavoro c'era, e il mio giro dal Python me lo stava
facendo saltare invece che trovare.

La seconda ha tre trappole, tutte silenziose. `x or ""` di Python e' falso
anche per `0` e per `false`, quindi un `subscriptionType` a zero e'
«sconosciuto» e non «abbonamento 0». `.strip()` toglie anche i separatori di
unita' che `char::is_whitespace` non considera bianchi. E `.replace()` toglie
il prefisso **ovunque**, non solo in testa: quest'ultima l'ha trovata una
mutazione che restava verde, perche' il caso che avevo scritto aveva il
prefisso ripetuto all'inizio — dove `trim_start_matches` di Rust fa la stessa
identica cosa. Sedici casi di lettura delle credenziali, e sono i tre stupidi
quelli che avrebbero fatto dire a NOVA una frase falsa con sicurezza.

Nel frattempo Gio provava il pannello e mi ha detto la cosa piu' utile della
giornata: «non dice il modello attivo». Vero, e non ci avevo pensato perche'
guardavo l'elenco. Il modello in uso si sa **subito**, dalla configurazione,
senza aspettare nessuna ricerca — e se il file scelto sta in una cartella che
la ricerca non guarda, senza quella riga non lo avrebbe saputo mai.

## 10 settembre 2026 — Due ore di silenzio, e un campo che non esisteva

Gio: «Rimane cosi' anche quando son certo abbia scelto un modello. Comunque
intanto Nova e' rimasto bloccato in loop».

Due frasi, tre difetti, e sono legati.

**Il loop.** Alle 20:23 Gio scrive «spegniti». Alle 22:17 quel processo era
ancora vivo: due ore, 410 secondi di CPU, e **nessuna riga scritta da nessuna
parte** — `azioni.jsonl` fermo al 5 settembre. Aveva acceso `llama-server`
con un modello da 27 miliardi di parametri che genera a 3,27 token al secondo:
l'ultima risposta gli aveva preso 266 secondi per 872 token, riempiendo il
contesto a 15.730 su 16.384. Non era piantato. Stava macinando. Ma da fuori
le due cose sono identiche, e vogliono due reazioni opposte.

Ho anche imparato a guardare prima di sparare: i nove `claude.exe` in cima
alla lista dei processi sembravano NOVA impazzita, ed erano l'applicazione
desktop con cui sto parlando con Gio.

**Il campo che non esisteva.** Il pannello salvava `model.path`. Quel campo
non c'e': `ModelConfig` tiene temperatura e top_p, il file del modello sta in
`server.model_path`. Quindi scegliere un modello dal pannello **non ha mai
funzionato** — da prima di ieri, e io ci ho costruito sopra l'elenco senza
accorgermene, propagando l'errore in sette punti invece di guardare la
configurazione vera una volta.

**E il terzo, che spiega perche' nessuno se n'era accorto.** Il guscio chiede
le statistiche della memoria ogni quindici secondi, e ogni richiesta passava
da `_prepare_config`, che faceva `cfg.save()` **sempre**. Quattro riscritture
al minuto. E `save()` scrive `asdict(self)`: solo i campi che le classi
conoscono. Quindi la chiave sbagliata finiva davvero nel file, ci restava
qualche secondo, e poi spariva — senza errori, senza log, e con quindici
secondi di distanza fra la causa e l'effetto, che e' il modo migliore per non
collegarli mai.

Due difetti che da soli non si vedono. Insieme fanno una funzione che non
funziona e non lo dice.

La cosa che mi tengo e' come sono venuti fuori. Non leggendo il codice: **il
file di configurazione cambiava identita' a ogni giro in `avvio.log`**, e
quella riga la scriveva la traccia che avevo messo ieri per un altro motivo.
La diagnostica serve al giorno che non sai cosa chiedere.

Coda: scrivendo la traccia del turno ho usato `self.brain_name`, che non
esiste. Sta dentro `except Exception: pass`, come tutta la diagnostica di
NOVA — quindi non avrebbe dato nessun errore: avrebbe solo smesso di
scrivere, e io avrei creduto di aver messo un diario. Una diagnostica che
tace e' peggio di nessuna diagnostica. Adesso una prova pretende che ogni
`self.x` letto in `Agent` sia assegnato da qualche parte.

## 11 settembre 2026 — Due frasi opposte nella stessa schermata

Gio riapre il pannello e trova questo: in cima «✗ Modello locale — nessun
modello scelto», due righe sotto «✓ gemma-4-26B... · 10.6 GB · vede le
immagini», col percorso completo.

Una schermata che si contraddice e' peggio di una che sbaglia: non si sa a
quale meta' credere.

La fascia in cima la scrive il Rust del guscio, e li' `model.path` era
rimasto. Avevo corretto il JavaScript, messo il nome della chiave in un posto
solo, scritto una prova che confronta ogni chiave del pannello con le classi
vere — cinquanta controlli verdi — e la prova cercava in `ui/*.html` e
`ui/*.js`. Il guscio legge la stessa configurazione da `src/*.rs`.

Avevo teso la rete dove il pesce era gia' passato.

E' D135 letta al contrario: l'ho applicata ai nomi e non ai lettori. Due
programmi che leggono lo stesso file sono due posti, sempre, anche quando il
valore ha finalmente un nome solo.

La coda vale quanto il resto. Estendendo la prova al Rust, il primo
cercatore raccoglieva qualunque coppia di stringhe, e per non accusare il
falso saltava le sezioni che non riconosceva — cioe' esattamente il caso in
cui la sezione e' sbagliata. Una mutazione con una sezione inventata restava
verde. La cura non e' stata allargare le eccezioni ma cercare meglio: si
prendono solo le letture che hanno `cfg` come ricevente, e allora si puo'
pretendere tutto. Un cercatore impreciso si paga sempre in controlli
disattivati.

### «Stavo andando manualmente a farlo»

Gio mi manda lo schermo del terminale: Gemini CLI, l'autenticazione con
Google, e in fondo `Failed to sign in: You do not have a valid license of
this product`. Poi la pagina del browser che dice «Autenticazione riuscita».
E la frase che conta: «stavo andando manualmente a farlo, perche' non abbiamo
un modo ancora per renderlo semplice all'utente».

Il bottone «Apri il terminale per collegarti» l'avevo messo il giorno prima.
Non gli e' mai comparso, e il motivo e' istruttivo: compariva solo quando NOVA
riteneva il cervello **non pronto**, e per una CLI «pronto» voleva dire «il
binario esiste nel PATH». `gemini` esiste. Quindi NOVA diceva «pronto» a un
cervello che al primo messaggio lo avrebbe respinto.

Ho guardato se si poteva fare meglio con i file: `~/.gemini/oauth_creds.json`
c'e', 1814 byte. Un controllo «hai il file delle credenziali?» avrebbe detto
«collegato» — ed e' falso. **Autenticato non vuol dire utilizzabile.** Nessuna
euristica sul disco puo' saperlo: l'OAuth e' andato a buon fine, e' la licenza
che manca.

Quindi si chiede. Domanda cortissima, tetto di trenta secondi, e si guarda cosa
risponde. Misurato: cinque secondi, uscita 1, e il messaggio giusto.

La parte che mi ha fatto pensare non e' lanciare il processo: e' **quale riga
far leggere**. Lo stderr vero di Gemini e' fatto cosi': un avviso sui 256
colori del terminale, la frase che conta, e venti righe di stack con dentro i
percorsi dei file. Mostrare la prima riga vuol dire mostrare l'avviso.
Mostrarle tutte vuol dire mostrare uno stack a chi voleva solo collegarsi.

E soprattutto: **non classifico**. La tentazione era fare un elenco di parole
spia — «authenticating», «login», «unauthorized» — e tradurre in categorie.
Ma di quelle parole ne ho misurata **una**, le altre le avrei indovinate, e
indovinare qui vuol dire dire una frase falsa con sicurezza. Si mostra cio'
che la CLI ha detto, tolti rumore, stack e chiavi. NOVA aggiunge solo il
verdetto che puo' dimostrare: ha risposto, non ha risposto, non e' partito,
non ha fatto in tempo.

Stessa storia al primo avvio, ed e' il pezzo che conta per la beta. La chat
aveva gia' il caso «non hai scelto chi ragiona», con tanto di bottone. Ma lo
decideva guardando la configurazione: `active` vuoto **e** nessun modello sul
disco. Con Gemini scelto e non collegato quella condizione e' falsa, quindi
NOVA offriva tre prove da cliccare che sarebbero fallite tutte e tre, una
dopo l'altra. Adesso lo chiede allo stato vero, e dice il motivo che ha
ricevuto invece di una frase generica: «non trovato nel PATH» e «non sei
collegato» si curano in due modi diversi.

### «Non capisco come mai non mi fa usare gemini se ho l'abbonamento pro»

Non era l'abbonamento. Era che quella porta e' murata da tre mesi.

Il 19 maggio Google annuncia il passaggio da Gemini CLI ad Antigravity CLI;
il **18 giugno** Gemini CLI smette di servire gli account individuali — AI
Pro, AI Ultra, piano gratuito. Restano dentro solo gli utenti enterprise con
licenza Code Assist e chi usa una chiave API a pagamento. Gio ha
`@google/gemini-cli@0.59.0` installato e un abbonamento personale: per lui
quella strada e' chiusa dal 18 giugno.

Ma il messaggio che leggeva diceva un'altra cosa: «non hai una licenza valida,
contatta il tuo amministratore». Nel suo `~/.gemini/.env` c'era
`GOOGLE_CLOUD_PROJECT`, e quella variabile fa prendere alla CLI il percorso
**enterprise** — dove la risposta giusta e' proprio quella. Due errori
sovrapposti, e quello visibile era il meno informativo dei due: mandava a
cercare una licenza aziendale invece di dire «questo prodotto non e' piu' per
te».

L'ho dimostrato senza toccare i suoi file: stessa domanda, due volte, con e
senza quella variabile nell'ambiente del solo processo di prova. Con:
«licenza mancante». Senza: «IneligibleTierError: this client is no longer
supported for Gemini Code Assist for individuals, migrate to Antigravity».

Quello che mi tengo, pero', e' un'altra cosa. Se avessi fatto il controllo che
mi era venuto in mente per primo — «esiste `~/.gemini/oauth_creds.json`?» —
NOVA avrebbe detto «Gemini collegato» **per sempre**. Il file c'e', 1814 byte,
l'OAuth passa davvero: e' la licenza che manca dopo. La prova vera non solo
l'ha visto: ha **cambiato diagnosi** quando ho tolto la variabile. Una
verifica che sa distinguere due cause diverse vale infinitamente piu' di una
che guarda se un file esiste.

Poi la decisione sull'elenco. `gemini` non si toglie: per chi ha una licenza
enterprise o una chiave API funziona ancora. Ma non puo' nemmeno restare
chiamata «Gemini» e basta, perche' quello e' il nome che cerca proprio chi ha
l'abbonamento personale — l'unico a cui non va. Adesso si chiama «Gemini
(licenza enterprise o chiave API)» e accanto c'e' «Antigravity (Google)», che
e' cio' che Google indica agli account come il suo.

Una cosa che **non** ho fatto: provarla. Antigravity CLI non e' installata su
questa macchina, quindi la voce e' scritta sulla documentazione ufficiale e
non su una misura. L'ho detto a Gio invece di far finta. E c'e' un rischio
dichiarato nel commento: e' segnalato che `agy -p` possa scartare lo stdout
quando gira come sottoprocesso invece che in un terminale vero, uscendo con
zero. Se capita, NOVA lo legge come «ha risposto ma non ha detto niente» —
che e' esattamente la frase giusta, e non «non funziona».

### Misurata, finalmente

Gio installa Antigravity CLI, e la voce che ieri avevo scritto sulla
documentazione diventa verificabile. Tre cose, in ordine di quanto mi
interessano.

**Il rischio dichiarato non c'era.** Avevo lasciato scritto nel codice che era
stato segnalato un difetto per cui `agy -p` poteva scartare lo stdout girando
come sottoprocesso invece che in un terminale — che e' esattamente il modo in
cui NOVA lancia le CLI. Misurato su `agy` 1.2.1: uscita 0, `ok\n`, tre
caratteri, stderr vuoto, 7,6 secondi. Non ci riguarda. Ho sostituito il
commento «non ancora verificato» con la misura e la data: un commento che dice
«attenzione, forse» quando ormai si sa e' rumore, e il rumore fa smettere di
leggere i commenti.

**Due falsi allarmi miei, di fila.** Primo: ho letto il PATH dal registro e ho
concluso che `agy` non si trovava — era `REG_EXPAND_SZ`, e `%LOCALAPPDATA%`
me lo stavo portando dietro non espanso. Secondo: ho visto che il
`config.json` di Gio ha `cli: [codex, gemini, qwen]` e ho pensato che
Antigravity non gli sarebbe comparsa. `Config.load()` fonde i predefiniti col
salvato, e la vede eccome. Tutte e due le volte stavo per **annunciare** un
difetto invece di verificarlo. Un allarme falso costa piu' di un silenzio:
manda a cercare dove non c'e' niente.

**Il difetto vero era un terzo, e l'ho trovato per caso.** NOVA girava dalle
13:14, `agy` e' stata installata alle 13:34, e per NOVA non esisteva — un
processo eredita l'ambiente da chi l'ha avviato, e il PATH di NOVA era quello
di venti minuti prima. Il messaggio pero' diceva «Installalo», che in quel
caso e' la cura sbagliata: manda a reinstallare una cosa gia' installata, e
chi ci prova due volte conclude che NOVA e' rotta.

Ma la cosa che ha permesso a quella frase di restare sbagliata e' un'altra:
**il banco non la confrontava**. `perche_non_pronto` esisteva in Rust e in
Python, uguale nelle due parti per fortuna e non per costruzione. Adesso il
banco confronta anche quella, con dei nomi che l'utente puo' essersi scelto,
virgolette comprese.

Una cosa ancora aperta e che non ho forzato: la voce `gemini` gia' salvata nel
file di Gio tiene l'etichetta vecchia, «Gemini» e basta. La nuova — «Gemini
(licenza enterprise o chiave API)» — arriva solo a chi installa NOVA da
adesso. Sovrascriverla sarebbe cancellare una scelta che l'utente **puo'**
aver fatto, visto che dal pannello l'etichetta si cambia. Per chi ce l'ha gia',
la verita' la dice la prova.

## 14 settembre 2026 — L'installatore, cioe' l'unico che vede chi non ha niente

Gio: «l'installer, come e' messo?». Sono andato a guardarlo invece di
rispondere, ed e' venuta fuori la cosa che questo progetto caccia da mesi.

Aveva l'elenco delle CLI **scritto a mano** in PowerShell — quattro righe con
claude, codex, gemini, qwen — e il commento sopra diceva perfino dove stava
quello vero: `nova/routing.py`, `cli_predefinite()`. Era gia' divergente:
Antigravity non c'era, e Gemini aveva l'etichetta di prima del 18 giugno.

La parte che fa male e' che lo stesso file fa gia' la cosa giusta **tre
volte**. La ricerca dei GGUF, il verdetto sul modello, l'avvertenza sulle
cartelle sincronizzate: tutte e tre chiedono a Python, e accanto c'e' scritto
il motivo — «due copie della stessa regola sono due regole destinate a
divergere». La regola era conosciuta, applicata, commentata. E a venti righe
di distanza c'era la copia.

Adesso l'elenco lo chiede a NOVA. Claude Code resta scritto a mano, ma
dichiarato: non sta fra le CLI predefinite perche' ha un modulo suo. E senza
Python si offre solo quello e lo si dice, invece di inventare una lista li'
dentro che domani non somiglia piu' a quella vera.

Poi «facile da usare», che era l'altra meta' della richiesta. Ho contato le
domande prima di giudicare: ventitre'. Ma una via breve **c'era gia'**:
`-Silenzioso` fa rispondere a ognuna il suo predefinito. Stava in un parametro
da riga di comando — chi fa doppio clic non la vede, e chi la vede non la
riconosce, perche' «silenzioso» dice come si comporta e non cosa ottiene.

Non ho aggiunto una modalita': sarebbe stato un secondo comportamento da
tenere allineato a mano. Ho reso visibile quella che c'era, come prima
domanda: «Fai tu» oppure «Scelgo io».

La cosa che mi ha fatto piacere scoprire e' che «fai tu» e' sicuro **per
costruzione**, non perche' ce lo metto io. Ero pronto a doverlo blindare — un
«fai tu» che scarica dodici gigabyte al buio sarebbe stato molto peggio di
ventitre' domande. Poi ho letto il predefinito della domanda sull'IA: «nessuna
per ora» se non ci sono modelli, «usa quello che hai» se ce ne sono. Da
nessuna delle due si arriva allo scaricamento; ci si arriva solo scegliendolo.
Qualcuno ci aveva gia' pensato, mesi fa, e la via breve era sicura da prima
che esistesse. La prova adesso guarda anche quel predefinito: se cambia,
«fai tu» comincerebbe a scaricare al buio e nessuno se ne accorgerebbe.

Provato per davvero con `-Prova`, che non tocca niente: gira da cima a fondo,
riconosce la RTX 4060 Ti e i 16 GB di VRAM, trova il modello gia' sul disco, e
chiude con «non ho toccato niente». `git status` conferma.

### Il numero di versione, e quattro file che non erano sorgente

Prima di pubblicare ho guardato cosa sarebbe uscito, invece di pubblicare e
basta. Due cose non andavano, tutte e due piccole e tutte e due del tipo che
si nota solo da fuori.

La prima: `core/Cargo.toml` diceva 0.1.1 e `core/Cargo.lock` diceva ancora
0.1.0, per tutti e venticinque i crate. Qui non cambia niente, perche' chi
compila senza `--locked` si riscrive il lock e va avanti. In CI, che compila
col lock, sarebbe stata la prima riga rossa del rilascio. Venticinque righe
`version`, nessuna dipendenza toccata: l'ho verificato contando le righe del
diff invece di fidarmi di cosa credevo di aver cambiato.

La seconda: nel repository c'erano quattro file che non sono sorgente.
`_shell_out.txt` e `_shell_err.txt`, zero byte, dove la shell scarica uscita
ed errore. `_tast_prova.txt.eventi`, gli eventi lasciati dalla prova della
tastiera. E un file che si chiama `importante'` - zero byte, nato da un
apostrofo di troppo in una riga di comando, chissa' quando.

Erano tracciati da prima che `.gitignore` avesse la sezione degli scarti, e
per questo nessuna delle regole scritte dopo li ha mai toccati: git ignora i
file che non conosce, non quelli che sta gia' seguendo. Li ho tolti dal
repository e lasciati sul disco, e **insieme** ho aggiunto le regole. Solo
toglierli sarebbe stato inutile: il primo che rilancia le prove della
tastiera li rimette dentro. Un file cancellato torna, una regola no (D135).

Poi la pubblicazione vera: `master` portato a `ottimizzazione` senza merge e
senza riscrivere niente, e il tag `v0.1.1` che fa partire la CI. Prima di
spingere ho contato i commit e gli autori: un autore solo, e nessuna riga di
attribuzione che non fosse la sua.

Resta una cosa per Gio, che non tocco: `bin/SHA256SUMS.txt` elenca tre binari
su quindici con hash vecchi. Non e' tracciato, quindi non esce con la
release - il manifesto vero lo fa la CI - ma sul suo disco continua a dire
una cosa falsa a chi lo apra.

### La CI ha visto quello che qui non si vedeva

Pubblicato, e due controlli su tre sono diventati rossi. Nessuno dei due era
un falso allarme, e tutti e due dicono la stessa cosa: **questa macchina non
e' una macchina qualunque**. Il cancello della beta chiede che qualcuno che
non e' l'autore l'abbia installato su una macchina che non e' questa. Non
posso farlo, ma la CI e' precisamente quello: una macchina spoglia, che non
ha niente e non sa niente.

**Primo rosso: la configurazione non nasceva.** `test_leggere_non_scrive`
falliva su tutte e quattro le versioni di Python con «`config.json` non
c'e'». La causa e' una mia correzione di quattro giorni fa: leggere non deve
riscrivere la configurazione, quindi si salva solo se `autoconfigure` ha
cambiato qualcosa. Qui cambia sempre qualcosa - c'e' un modello sul disco, un
runtime da trovare. Su una macchina spoglia non c'e' niente da completare, il
confronto e' uguale, e il file **non veniva creato affatto**: NOVA girava
senza scriversi una configurazione, e chi avesse voluto correggerla a mano non
avrebbe trovato niente da aprire. Il confronto dice se e' cambiata, non se
esiste (D207).

Due cose che ho sistemato mentre ci ero. La prova diceva «non c'e'» e basta:
adesso stampa lo stderr del sottoprocesso, dove il motivo stava dal primo
giro. E la prima parte misurava `_prepare_config` **attraverso**
`autoconfigure`, cioe' attraverso cio' che c'e' sul disco: adesso
`autoconfigure` e' sostituita, cosi' la prova misura la decisione di salvare e
non la fortuna di avere un modello. Con la sostituzione ho potuto aggiungere
il caso che mancava - il file che non c'e' ancora - e quel caso e' rosso senza
la correzione.

**Secondo rosso: il controllo sui dati personali.** Questo mi piace meno,
perche' e' la stessa forma di errore che ho scritto in questo diario due
settimane fa. Il controllo era un `grep` dentro il file della CI, con due
esenzioni: `riservatezza.py` e `test_*.py`, «sono il rilevatore, non il
segreto». Giusto. Solo che da allora il rilevatore e' nato **anche in Rust**,
e in Rust le prove stanno dentro `src/*.rs`. La rete era tesa dove i pesci non
passano piu' (D135). Venti righe rosse, tutte esempi.

Non ho allargato le esenzioni: ho spostato la regola dove si puo' leggere e
provare, cioe' in una prova del progetto. E le ho cambiato il criterio. Prima
segnalava qualunque percorso dentro `C:\Users\` scritto con un nome in
minuscola, esempi compresi; adesso c'e' **un
nome finto solo per tutto il progetto**, `utente`, e ogni altro nome e' di
qualcuno (D206). Niente da giudicare, niente da discutere. Tradotti gli
esempi: venti righe fra Rust e documentazione, dove per sbaglio c'era anche il
mio nome utente vero, tre volte, in due file di documentazione.

La terza prova, quella che non avevo previsto di scrivere, e' la piu' utile:
**un'esenzione che non copre piu' niente e' rossa** (D208). Le esenzioni si
ereditano, restano scritte quando il motivo e' passato, e coprono in silenzio
il file che prendera' quel nome domani. Alla prima esecuzione ne ha trovata
una: `riservatezza.py` era esente e non contiene nessuno degli esempi che
cerca. Era un'esenzione ereditata e mai verificata - probabilmente vera nel
2025, falsa adesso.

**E un terzo rosso che non era un difetto**, ma andava capito lo stesso.
`test_cerca` falliva su Python 3.13 e non sulle altre tre: «lascia una scheda
di ricerca aperta». Una prova che fallisce su una versione sola invita a
cercare cosa e' cambiato in quella versione, ed e' la strada sbagliata:
chiudere una scheda non e' istantaneo, e la prova guardava una volta sola. Non
era Python, era il carico. Adesso aspetta cinque secondi che sparisca, e solo
se resta e' una perdita.

Il rilascio `v0.1.1` intanto e' andato a buon fine: sette minuti e trentanove,
zip e `SHA256SUMS.txt` allegati. I rossi sono della CI, non della release -
ma sarebbero rimasti rossi in cima alla pagina del progetto.
