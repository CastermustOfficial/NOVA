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
