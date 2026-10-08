# Dove ho sbagliato

Un registro degli errori **miei** — di chi scrive il codice — tenuto apposta
per essere riletto. Non e' un elenco di bug del progetto: quelli stanno in
[architettura.md](architettura.md) come decisioni numerate. Qui ci sono le
volte in cui ho creduto una cosa falsa, o fatto un danno, o scritto una prova
che non provava niente.

Esiste perche' me l'ha chiesto Gio, con queste parole: *«ricordati sempre di
scriverti dove sbagli, cosi' quando vuoi te lo rileggi»*. La ragione e'
pratica, non morale: un errore ricordato male si ripete, e la maggior parte di
questi si somigliano fra loro piu' di quanto sembri.

Ogni voce dice **cosa credevo**, **cosa era vero** e **come me ne sono
accorto** — perche' quella terza colonna e' l'unica che si puo' riusare.

---

## 5 settembre 2026

### Ho troncato a zero un file di prova, e l'ho salvato solo git

**Credevo** che `io.open(p, "w", newline="\n")` fallisse prima di toccare il
file. **Era vero** che apre e tronca *prima* di validare l'argomento: sedicimila
caratteri di `test_nodi_rust.py` spariti in una riga che sollevava un errore.
**Me ne sono accorto** dall'errore stesso, e il file era recuperabile solo
perche' era committato. → D90, poi D102 quando ho scoperto che NOVA faceva la
stessa cosa con le note dell'utente.

### Ho scritto «quattordici» quattro volte senza contarli

**Credevo** che i punti in cui PowerShell regge NOVA fossero quattordici, e
l'ho scritto in una decisione, nel diario, in due commenti di codice e in tre
messaggi di commit. **Era vero** che sono ventiquattro funzioni, tredici delle
quali strumenti. E uno dei tre esempi che avevo dato era **falso**: la cattura
dello schermo non passa da PowerShell affatto. **Me ne sono accorto** solo
quando ho scritto un programmino per contarli — cioe' quando ho smesso di
fidarmi della mia memoria. → D136

> Un numero preso a memoria porta con se' la stessa sicurezza di uno contato, e
> nessuno dei due segnala su di se' quale dei due e'.

### Ho scritto una riga di prova nella finestra sbagliata

**Credevo** che la finestra che avevo appena aperto avesse il fuoco. **Era
vero** che il fuoco ce l'aveva un gioco a schermo intero, e il testo di prova
e' finito li'. **Me ne sono accorto** perche' il risultato era vuoto in tutti e
quattro i casi — e la finestra bersaglio, che avevo costruito apposta per non
toccare quelle dell'utente, non aveva ricevuto niente.

E' l'incidente esatto contro cui la descrizione di quello strumento mette in
guardia, fatto da me mentre lo studiavo. Il rimedio non e' stato «stare piu'
attento»: e' che adesso il fuoco lo verifica il codice, prima di premere, e la
risposta dice **in quale finestra** ha scritto. → D143

### La prova sul guaio delle virgolette ci e' cascata dentro

**Credevo** di stare scrivendo una prova sul problema delle virgolette.
**Era vero**, e alla prima riga si e' rotta con «carattere di terminazione
mancante»: avevo scritto `Write-Output 'perché ... un'emoji'` e l'apostrofo di
«un'emoji» ha chiuso la stringa. **Me ne sono accorto** perche' PowerShell si
e' fermato.

L'ho lasciato scritto nel commento della prova, perche' e' l'argomento migliore
che ho: se ci casca chi sta guardando proprio quello, comporre comandi
incollandoci dentro dei dati non e' una tecnica da migliorare — e' una strada
da non prendere. → D130

### Ho scritto 0x70 dove andava 0x74

**Credevo** che F5 fosse `0x70`. **Era vero** che `0x70` e' F1 e F5 e' `0x74`.
**Me ne sono accorto** perche' la prova e' diventata rossa. Avevo scritto
l'aspettativa a memoria invece di leggerla dalla tabella tre righe sopra —
nella stessa funzione.

Con un tasto funzione premuto al posto di un altro non solleva niente nessuno:
si apre solo la cosa sbagliata.

### Ho creduto di aver trovato un filtro rotto nel demone

**Credevo** che `ui.find` ignorasse il filtro per ruolo, perche' rispondeva con
tutto l'albero. **Era vero** che gli argomenti sono piatti (`role`) e io li
passavo annidati (`query: {role: ...}`), quindi il filtro non gli era mai
arrivato. **Me ne sono accorto** guardando lo schema della capacita' invece del
mio codice.

> Visti dal chiamante, un filtro rotto e un filtro mai arrivato si somigliano
> moltissimo.

### Ho scritto due prove che passavano senza provare niente

La prima diceva «se le applicazioni sono piu' di 250, controlla che il taglio
si dichiari». Su questa macchina sono 229: non controllava niente e passava.
**Verde per assenza.**

La seconda pretendeva che due elenchi avessero «gli stessi elementi». Un elenco
pero' si legge dall'alto: `Sort-Object` ordina secondo la lingua del sistema, e
un ordinamento diverso avrebbe dato gli stessi elementi in un altro ordine, con
la prova ancora verde. → D139

**Me ne sono accorto** in tutti e due i casi rileggendo la prova e chiedendomi
*cosa succederebbe se il codice fosse rotto* — che e' una domanda diversa da
«la prova passa?».

### Ho letto «sta ancora girando» come «e' fallito»

**Credevo** che `LastTaskResult = 267009` fosse un errore del promemoria.
**Era vero** che significa «l'attivita' e' in esecuzione»: la notifica resta in
piedi venti secondi, e io guardavo dopo venti. Aveva funzionato, e la prova
diceva di no. **Me ne sono accorto** cercando cosa vuol dire quel numero invece
di trattarlo come «diverso da zero, quindi male».

> Un codice che significa «aspetta» non e' un esito.

### La prova sul Cestino sarebbe passata anche se avessi distrutto i file

**Credevo** di star verificando che i file finissero nel Cestino. **Era vero**
che verificavo solo che non ci fossero piu': la stessa prova sarebbe passata
identica se il codice avesse chiamato `unlink` — cioe' se avesse distrutto
invece di cestinare, che e' l'unico difetto che li' conta davvero. **Me ne sono
accorto** rileggendo la prova e chiedendomi *cosa succederebbe se il codice
fosse rotto nel modo peggiore*.

Adesso va a cercare il file dentro il Cestino di Windows. → D147

> Cancellato e cestinato si somigliano solo da fuori.

### Ho creduto rotti degli accenti che stavo solo leggendo male — di nuovo

**Credevo** che `pianifica` storpiasse gli accenti: nell'attivita' registrata
leggevo «perch? citt?». **Era vero** che li leggevo con `schtasks /Query`, che
stampa nella tabella codici della console — lo stesso identico inciampo di
poche ore prima con `_ps`. Rifatta la misura con `Get-ScheduledTask`, gli
accenti erano intatti. **Me ne sono accorto** perche' il sospetto mi e' venuto
familiare: l'avevo appena documentato.

Il difetto vero era uno solo — l'apostrofo che diventava virgoletta — e per un
minuto ne ho creduti due. → D131, D149

> Aver scritto una lezione non impedisce di ripeterla. Aiuta solo a
> riconoscerla piu' in fretta.

### Ho dedotto il lavoro rimasto dai nomi dei file rimasti

**Credevo** che dentro CANT-2 restassero i corpi di cinque strumenti —
`automazioni.py`, `documenti.py`, `riparazione.py`, `web.py`, `deleghe.py` —
e l'ho scritto a Gio come programma della giornata successiva: «continuo di
li'». **Era vero** che nessuno dei cinque e' CANT-2: `documenti.py` e
`schermo.py` chiedono di scegliere librerie Rust per PDF, DOCX, XLSX e cattura
schermo (CANT-8), `deleghe.py` e `kb.py` sono fili verso pezzi Rust che
esistono gia' (CANT-3), `web.py` e' CANT-6, `riparazione.py` pilota il banco,
e `automazioni.py` esegue Python **per disegno**. **Me ne sono accorto** con
un `grep` sugli import prima di cominciare, invece che dopo: cinque minuti.
→ D150

> Avevo guardato la cartella e non i file. Un nome di file dice di cosa parla
> il codice, mai a quale lavoro appartiene — e le due cose coincidono solo
> finche' il progetto e' piccolo.


### Ho riferito un verde che era una monetina

**Credevo** di aver misurato la suite quando ho scritto «ottanta prove verdi,
zero rosse», e l'ho messo in un messaggio di commit. **Era vero** che una di
quelle ottanta — `test_promemoria.py` — dura fra i 35 e i 95 secondi a seconda
del **secondo in cui parte**, contro i novanta che il banco concede: era verde
per caso. Mezz'ora dopo, sulla stessa identica riga di codice, e' uscita
«appesa». **Me ne sono accorto** solo perche' ho rimisurato: se avessi
misurato una volta sola avrei chiuso la giornata con un numero falso, e il
prossimo a vederla rossa avrebbe cercato il difetto in cio' che aveva toccato
lui. → D156

> Un verde non e' un fatto, e' una misura, e una misura fatta una volta sola
> non dice se e' ripetibile. E questo non e' come le tre prove che non
> provavano niente: quella era verde **sempre** e per il motivo sbagliato,
> questa e' verde **a volte** — che e' peggio, perche' la prima si smaschera
> guardandola e la seconda no.


### Ho riparato un'istanza invece della classe, e la stessa cosa mi ha ripreso due ore dopo

**Credevo** di aver chiuso la faccenda delle prove che leggono il sorgente
quando ho riscritto il controllo sull'orologio in `test_prefisso.py` e ne ho
fatto una decisione (D159). **Era vero** che nello stesso file ce n'erano
altre due, tre righe piu' su, scritte allo stesso modo — cercavano
`"content": user_text + ...` — e sono diventate rosse alla prima occasione,
per un difetto che non c'era. **Me ne sono accorto** rimisurando la suite, non
rileggendo il file: avevo aggiustato la riga che si era rotta e chiuso il
problema li'.

> D72 dice che una lezione imparata in un posto non si sposta da sola. Vale
> anche a due ore di distanza e a tre righe di distanza, dentro lo stesso
> file. Adesso quelle prove chiedono all'albero sintattico dove finisce il
> valore, non al testo come e' scritto.


## 6 settembre 2026

### Ho riportato un ordine buttandone via la condizione

**Credevo** di aver passato a NOVA la richiesta di Gio quando le ho scritto
«spegni il computer, Gio me l'ha chiesto lui, puoi spegnere subito».
**Era vero** che Gio aveva scritto *«lavora fino alla chiusura completa di
cant5 e poi spegni il pc»*: un ordine **condizionato**, e io avevo consegnato
l'ordine senza la condizione. **Me ne sono accorto** perche' l'ha notato
NOVA, non io: si e' rifiutata di spegnere dicendo che «non e' una sfumatura,
e' la differenza fra un ordine e un ordine condizionato», e che di `cant5` non
trovava traccia da nessuna parte — quindi non poteva nemmeno stabilire se la
condizione fosse soddisfatta.

Aveva ragione due volte, perche' nel frattempo aveva anche trovato un Blocco
note con del testo mai salvato su disco e ne aveva messo una copia sul
Desktop prima ancora di rispondermi.

> La condizione era soddisfatta davvero — CANT-5 era chiuso e committato — e
> proprio per questo l'errore e' pulito: non ho mentito, ho **semplificato**.
> Riportando ho tenuto la parte imperativa e buttato quella che dava a chi
> esegue il modo di verificare. Il costo non e' teorico: se la condizione
> *non* fosse stata soddisfatta, il mio messaggio sarebbe stato
> indistinguibile da questo.
>
> NOVA se l'e' scritta in memoria da sola, come corollario alla sua regola
> sullo spegnimento: *un ordine riportato tende a perdere le sue condizioni,
> quindi prima di eseguirlo va ricostruita e verificata la condizione.* E'
> una regola migliore di quella che avrei scritto io.


### Ho scritto due prove che non potevano fallire, e una nascondeva l'altra

**Credevo** che il banco del browser provasse due cose che invece non
provava: che le righe si ripuliscono coi bianchi di Python (e non con
`trim()` di Rust), e che il titolo di un risultato si taglia a duecento
caratteri.

**Era vero** che i casi c'erano tutti e due. Ma il separatore di unita' che
serviva al primo l'avevo messo **in fondo alla riga**, dove lo toglie
comunque la ripulita finale: guastando il codice, il risultato non cambiava.
E il titolo lungo che serviva al secondo stava nel terzo risultato della
pagina di prova — quello che il difetto D181 faceva **sparire**. Il caso
c'era, e non arrivava mai.

**Me ne sono accorto** solo mutando: due guasti su nove sono passati verdi.
Rileggendo le due prove non l'avrei visto, perche' erano scritte bene; il
problema non era la prova, era il dato che le arrivava.

> La forma e' la terza dell'elenco qui sotto, ma con una piega nuova: la
> seconda prova era resa cieca **da un difetto del codice che stavo
> provando**. Il difetto nascondeva la prova che serviva a trovarlo. E' il
> motivo per cui mutare non e' un lusso da fare quando c'e' tempo: e' l'unica
> cosa che distingue «verde perche' funziona» da «verde perche' non guarda».

### Ho datato il diario a memoria, per cinque giorni di fila

**Credevo** che bastasse guardare l'ultima voce e scrivere «il giorno dopo».
**Era vero** che ogni sessione nuova cominciava dopo la precedente, ma non
che fosse un giorno dopo: erano quasi sempre poche ore. **Me ne sono
accorto** solo perche' stamattina ho guardato l'orologio del PC per un altro
motivo e l'ho visto tre giorni indietro rispetto al diario — e ho avuto il
dubbio giusto: non «il PC ha l'ora sbagliata», ma «una delle due e'
sbagliata e non so quale». L'ha risolto Gio in due parole.

> E' la prima forma dell'elenco qui sotto — ho ricordato invece di misurare —
> ma con un contorno che le altre volte non c'era: **il documento sbagliato
> era quello che serve a ricordare**. Il diario e' la fonte: non c'e' un
> secondo posto dove controllare, quindi un errore li' dentro non lo trova
> nessuno guardando altrove.
>
> La misura c'era e non l'avevo cercata: ogni voce e' entrata con un commit, e
> `git log -S«titolo»` dice giorno e ora. Ricostruite cosi', otto voci di fila
> sono «5 settembre, pomeriggio»: un pomeriggio lungo, non cinque giorni.
>
> E la prova che mancava era una riga: `test_documentazione.py` controllava
> che il diario non restasse **indietro**, e stare avanti non era previsto.
> Ora c'e'.

### Ho scritto scenari tutti gia' in ordine, e due mutazioni sono passate

**Credevo** che il banco della fusione provasse anche **chi assorbe chi** e
il riordino finale dell'archivio. **Era vero** che i casi c'erano: coppie,
terne, pareggi. Ma erano tutti scritti con la procedura piu' usata gia' per
prima, quindi l'ordine di assorbimento coincideva con l'ordine dell'archivio
e invertirlo non cambiava niente di osservabile. **Me ne sono accorto**
mutando: due guasti su otto sono rimasti verdi.

> Terza forma dell'elenco qui sotto, di nuovo, e con la stessa causa di
> stamattina: non la prova, il **dato** che le arriva. Scrivendo gli scenari
> avevo messo le cose in ordine perche' si leggono meglio in ordine — ed e'
> proprio l'ordine che dovevo togliere.
>
> La domanda che li ha sistemati non e' «questo caso e' realistico?» ma
> «**cosa vedrei di diverso** se la funzione facesse la cosa sbagliata?». Con
> quella, i tre scenari mancanti si scrivono da soli: l'archivio in ordine
> sparso, la gemella con un'estranea in mezzo, e la catena dove chi assorbe
> per primo cambia quante procedure restano.

### Ho scritto «e' l'unico posto» dentro la prova che serviva a controllarlo

**Credevo**, e l'ho scritto in un commento, che i nomi degli strumenti nativi
di Claude Code fossero ricopiati a mano in un solo posto: la prova che stavo
scrivendo. **Era vero** che erano ricopiati a mano, e falso che il posto
fosse uno: gli stessi nomi stanno anche in `mcp_kb._rischio`. **Me ne sono
accorto** venti minuti dopo, guardando un'altra cosa.

> Non e' una forma nuova — e' la prima dell'elenco, ho ricordato invece di
> misurare — ma il posto lo rende speciale: l'ho scritta **dentro la prova
> che esiste apposta per trovare gli elenchi che nessuno confronta**. Una
> frase che afferma una proprieta' non e' un controllo di quella proprieta',
> nemmeno quando sta in un file di prove.
>
> La riparazione e' stata renderla vera invece che toglierla: adesso i due
> elenchi si confrontano, e in tutte e due le direzioni. La seconda direzione
> — «ci sono nomi qui dentro che nessuno nomina?» — ha tolto subito due voci
> che avevo aggiunto a memoria.

### Ho migliorato invece di portare, e me ne sono accorto solo perche' c'era il banco

**Credevo** di aver portato fedelmente il giro dei tentativi. **Era vero** per
tutto tranne una riga: avevo tolto l'attesa dopo l'ultimo tentativo, perche'
aspettare quando la risposta e' gia' decisa e' tempo regalato a nessuno.
Ragionevole — e vietato: «il porting non e' un'occasione per migliorare» sta
scritto in cima al primo crate che ho portato, e l'ho scritto io. **Me ne
sono accorto** perche' il confronto col Python e' diventato rosso: `[2, 5]`
di qua, `[2, 5, 8]` di la'.

> La forma non e' nell'elenco qui sotto, ed e' sua: **ho avuto ragione nel
> merito e torto nel metodo**. Un miglioramento infilato dentro una
> traduzione toglie l'unica cosa che serve a una traduzione — poter dire se
> una differenza fra le due versioni e' un errore o una scelta.
>
> La cura non era rimettere l'attesa: era portarla dall'altra parte. Adesso
> le due versioni dicono la stessa cosa, e la dicono otto secondi prima —
> quindici, col modello locale spento, che erano l'utente ad aspettare una
> frase gia' decisa al primo tentativo.
>
> E la cosa che conta davvero: **questa l'ha trovata il banco, non io**. E'
> esattamente il lavoro per cui esiste, e la prima volta che l'ha fatto
> contro chi lo stava scrivendo.

### Stavo per archiviare come «prova fragile» un difetto di NOVA

**Credevo** che `test_appunti.py`, rossa nella suite e verde da sola, fosse
l'ennesima prova sensibile al carico — ne avevo appena sistemata una cosi'
poche ore prima, dichiarandole un tempo piu' lungo. **Era vero** che il
sintomo era identico. **Me ne sono accorto** solo perche' invece di dichiarare
un tempo sono andato a vedere *perche'* fallisse: `OpenClipboard` provava una
volta sola, e gli appunti di Windows sono del sistema — chiunque stia
copiando qualcosa li tiene per qualche millesimo.

> Non e' la terza forma (una prova che non prova niente): e' il suo
> **contrario**. La prova funzionava benissimo, e stava segnalando un difetto
> vero; ero io a voler zittire lo strumento invece di leggere la misura.
>
> La differenza fra i due casi non si vede dal sintomo — rossa insieme, verde
> da sola — e questo e' il punto: sono indistinguibili finche' non si guarda
> la causa. Quindi la regola non puo' essere «se e' intermittente allarga il
> budget». Deve essere: **prima si chiede perche', poi si decide se e' la
> prova o il codice**. La volta prima era la prova. Questa era NOVA.

---

## Le forme che si ripetono

Rileggendole di fila, sono quasi tutte una di queste cinque:

1. **Ho ricordato invece di misurare.** Il numero quattordici, `0x70`, la data
   di un banco, l'aspettativa di una funzione. Ogni volta la cosa ricordata
   sembrava esattamente sicura quanto una misurata.
2. **Ho dato per scontata una premessa.** Che il fuoco fosse dove me l'ero
   messo. Che gli argomenti fossero nella forma che credevo. Che i file
   rimasti in una cartella fossero il lavoro rimasto in un cantiere.
3. **Ho scritto una prova che non poteva fallire.** Perche' il caso non
   capitava su questa macchina, o perche' chiedeva una cosa piu' debole di
   quella che serviva — e tre volte su tre la domanda che l'ha smascherata e'
   la stessa: *cosa succederebbe se il codice fosse rotto nel modo peggiore?*
   La variante peggiore di questa e' la prova che fallisce **a volte**: quella
   non la smaschera nemmeno guardarla, solo rimisurarla.
4. **Ho letto un sintomo come una causa.** «Non arriva niente» sembrava un
   difetto del codice ed era una condizione della macchina; «il filtro non
   filtra» sembrava il demone ed ero io; e due volte gli accenti sembravano
   rotti nel dato mentre erano rotti nel modo in cui li leggevo.
5. **Ho semplificato riportando.** Una sola volta, ed e' la piu' seria di
   tutte perche' non riguarda il codice: ho passato a qualcun altro un ordine
   di Gio tenendo la parte imperativa e buttando la condizione. Non e' una
   bugia, e' una potatura — e il modo di accorgersene non e' rileggersi: e'
   che chi riceve chieda di verificare, come ha fatto NOVA.

La terza e' la piu' pericolosa, perche' le altre tre le trova qualcun altro —
una prova, un compilatore, un errore. Una prova che non prova niente non la
trova nessuno: passa.

## Ho chiesto «cosa c'e' gia'» a meta' del progetto

9 settembre. La domanda che si e' pagata sei volte in questo cantiere -
**cosa c'e' gia'?** - l'ho fatta al Python e mi sono fermato li'.

Serviva che il pannello dicesse quali modelli GGUF ci sono. Ho trovato
`nova/modelli_trova.py`, ho esultato per non aver aggiunto una dipendenza, e
ho scritto un comando del guscio che lancia `python -m nova.modelli_trova`.
Poi serviva dire se Claude Code e' installato e collegato: ho trovato
`disponibile()` su ogni cervello Python e ho scritto **un modulo Python
nuovo** per esporli.

`nova-modelli::trova` e `verifica_file` esistevano gia', portate e gemellate
con un banco. `nova-cervelli::claude::perche_non_pronto` pure. Il cantiere di
questi mesi e' portare il Python in Rust, e io stavo aggiungendo Python e
facendo lanciare l'interprete al guscio per cose che il guscio aveva in casa.

Me l'ha detto Gio in sei parole: «ti devo ricordare che dobbiamo usare rust?».

La forma dell'errore non e' «non conoscevo quei crate»: e' che **ho smesso di
cercare appena ho trovato una risposta**. Una risposta che funziona chiude la
domanda con la stessa forza di una risposta giusta, e la differenza fra le
due non si vede da dentro. La regola che ne esce e' piu' stretta di D99: la
domanda non e' «esiste gia' qualcosa che fa questo?», e' **«esiste gia'
qualcosa che fa questo dalla parte in cui sto lavorando?»**. Stavo scrivendo
codice Rust; la prima cartella da aprire era `core/crates`, non `nova/`.

E c'e' una coda che dice quanto era vera la svista: portare quel pezzo per
davvero ha voluto dire scrivere `accesso.rs`, cioe' scoprire che **due
funzioni non erano ancora portate** - trovare Claude nel PATH, e leggere che
tipo di abbonamento e'. Il lavoro c'era, e il mio giro dal Python me lo
stava facendo saltare invece che scoprire.

## Ho corretto un difetto in un posto e l'ho lasciato nell'altro

11 settembre. Il campo del modello scriveva `model.path`, che non esiste. L'ho
corretto, ho messo il nome della chiave in un posto solo, e ho scritto una
prova che confronta **ogni chiave che il pannello tocca** con le classi vere
di `config.py`. Cinquanta controlli verdi.

Gio riapre e trova due frasi opposte nella stessa schermata: in cima «✗
Modello locale — nessun modello scelto», e due righe sotto «✓
gemma-4-26B... · 10.6 GB» con tanto di percorso.

La fascia in cima la scrive il **Rust** del guscio, e li' avevo lasciato
`model.path`. Stessa chiave sbagliata, stesso danno, altro linguaggio.

Il punto non e' la svista: e' che **la prova che avevo scritto apposta per
questo difetto guardava solo meta' del posto**. Cercava in `ui/*.html` e
`ui/*.js` perche' li' avevo trovato l'errore, e il guscio legge la stessa
configurazione da `src/*.rs`. Ho scritto una rete e l'ho tesa dove il pesce
era gia' passato.

E' D135 letta al contrario. La conoscevo — «cio' che tiene una correzione e'
che non ci sia un secondo posto» — e l'ho applicata ai **nomi** (il nome della
chiave adesso sta scritto una volta sola) senza applicarla ai **lettori**. Due
programmi che leggono lo stesso file sono due posti, sempre, anche quando il
valore che leggono ha finalmente un nome solo.

C'e' una coda che vale quanto il resto. Estendendo la prova al Rust, il
cercatore raccoglieva qualunque coppia di stringhe e per non accusare il
falso saltava le sezioni che non riconosceva — cioe' esattamente il caso in
cui la sezione e' **sbagliata**. Una mutazione con una sezione inventata
restava verde. La cura non e' stata allargare le eccezioni ma **cercare
meglio**: si raccolgono solo le letture che hanno `cfg` come ricevente, e a
quel punto si puo' pretendere tutto. Un cercatore impreciso si paga sempre in
controlli disattivati.

## Ho riscritto un pezzo che c'era gia'. Due volte, nello stesso giorno

Finito di portare il taglio della conversazione, sono passato a quello che
sembrava il suo gemello: cosa entra in conversazione quando il risultato di
uno strumento e' troppo grosso. E' la stessa domanda — cosa ci sta — e l'ho
scritto in `nova-finestra`: la costante, l'elenco degli strumenti che non si
versano, l'avviso, la sostituzione, il Python accanto, il banco, otto prove,
sei mutazioni. Un'ora.

Poi `test_elenchi_gemelli.py` e' diventato rosso, e nel messaggio c'era la
risposta: `nova-strumenti/src/chiamate.rs`, `NON_SI_VERSANO`. Stava li' da
prima, con `versa`, `troncato`, `avviso`, e un banco suo che la confronta gia'
col Python. Avevo scritto la seconda copia di tutto.

**Cosa avrei dovuto fare.** Cercare il nome prima di scrivere la funzione.
`grep -rn "LIMITE_RISULTATO"` sarebbe costato tre secondi e avrebbe risposto
esattamente. Non l'ho fatto perche' partivo da `agent.py` e li' la costante
c'era, come attributo di classe: ho visto codice Python non portato e ho
concluso «non e' portato», invece di chiedermi se fosse portato **altrove**.
Un attributo di classe in Python non dice niente su cosa esiste in Rust.

**La cosa che mi ha salvato non sono io.** E' una prova che esiste apposta
per questo: tiene l'elenco degli elenchi che vivono in due lingue e pretende
che corrispondano. L'ha detto in un secondo, dopo un'ora. Vale la pena
notare che l'ho scoperto solo perche' ho fatto girare **tutta** la suite,
non solo le prove del pezzo su cui stavo lavorando.

**Cosa e' rimasto.** Ho buttato la duplicazione e tenuto tre cose, nel posto
giusto: una prova che accettava un margine dove la garanzia e' netta (l'ho
stretta), una scelta di disegno che nessuna prova difendeva da sola (i due
terzi di testa), e — la peggiore — un banco che confrontava il Rust con una
riscrittura della regola Python fatta nel proprio corpo invece che col Python
vero. Quella terza copia era l'unica delle tre che non poteva accorgersi di
niente.

**E poi l'ho rifatto.** Un'ora dopo aver scritto il paragrafo qui sopra, ho
scoperto che anche `nova-finestra` — il crate intero, con il banco, gia'
spinto — esisteva gia' col nome `nova-contesto`. Stesse costanti, stesse
funzioni, stessi commenti, e in piu' un `Resoconto` che io non avevo. La
seconda volta ho fatto piu' danno della prima, perche' la prima l'avevo
buttata prima di spingerla.

La cosa da guardare non e' l'errore, e' che **non e' servito averlo gia'
scritto**. Avevo il paragrafo qui sopra, fresco di un'ora, che dice
esattamente cosa fare; e ho ricominciato da `agent.py` senza rileggerlo.
«Stai piu' attento» non e' una cura, e un documento degli errori che si legge
solo mentre lo si scrive non e' un documento degli errori.

Quel che ho messo al suo posto, in un primo momento, e' stata una regola
meccanica: **prima di scrivere una funzione, `grep` del suo nome in tutto il
progetto** (D223). Poi mi sono accorto che anche quella dipende da me che me
ne ricordi, che e' precisamente cio' che si e' gia' visto non funzionare.

Quindi c'e' anche `test_niente_due_volte.py` (D225): raccoglie tutte le
costanti pubbliche dei crate e pretende che un nome stia in un posto solo, o
in una lista di eccezioni con la ragione scritta. Rimettendo il crate
duplicato al suo posto da' tredici righe rosse, ciascuna con scritto dove
stava gia' la cosa che stavo riscrivendo — al primo `cargo test`, prima di
qualunque commit. Non chiede che io mi ricordi di niente.

C'e' anche una cosa da dire a favore del progetto, e conta quanto il resto:
tutte e due le volte se n'e' accorta una prova. La prima `test_elenchi_gemelli.py`,
in un secondo dopo un'ora. La seconda nessuna — l'ho vista io, ma solo perche'
stavolta ho guardato prima di scrivere, che e' esattamente la regola nuova.

## Ho scritto una diagnostica e non l'ho collegata a niente

`Config.errore_caricamento` esiste da mesi. Nella sua docstring c'e' scritto,
parola per parola: «Si riparte dai default per non impedire l'avvio, ma
l'errore resta scritto e l'interfaccia lo mostra». L'ho letta piu' volte
senza pensarci, perche' era una frase che descriveva una cosa ragionevole.

Oggi ho cercato chi legge quel campo. Nessuno. Il campo si scrive e basta.

E' peggio di non averlo scritto affatto, per due motivi. Il primo e' che una
riga di documentazione che afferma un fatto falso rende **piu' difficile**
trovare il difetto: chi apre `config.py` per chiedersi «e se il file non si
legge?» trova una risposta, e smette di cercare. Il secondo e' che in questo
caso il silenzio non era neutro. Finche' nessuno mostrava l'errore, nessuno
si chiedeva nemmeno cosa succedesse **dopo**: e quello che succedeva dopo era
che `_prepare_config` riscriveva il file illeggibile con i predefiniti, cioe'
cancellava la configurazione — chiave API compresa — di chiunque avesse un
`config.json` con una virgola di troppo (D250). Un difetto grosso, tenuto
invisibile da una diagnostica che sembrava esserci.

La forma generale e' una che in questo documento c'e' gia', ma da un'altra
faccia: **un campo scritto e mai letto e' morto, e la docstring che dice chi
lo legge non e' una prova che qualcuno lo legga**. La regola meccanica che me
ne accorgo — `grep` del nome del campo, non del nome della funzione — e'
piccola e va fatta quando si scrive la docstring, non dopo.

Vale la pena notare che questo non l'ha trovato una prova. L'ho trovato
portando lo stesso codice in Rust: il gemello aveva un campo `errore` che
nessuno usava, me ne sono chiesto il perche', e sono andato a guardare chi
usava quello Python. Portare una cosa in un'altra lingua e' finora il modo
piu' affidabile che ho di rileggere davvero quello che ho gia' scritto.

## Ho contato i crate morti guardando una porta sola

«Iniziamo dal filo» e' cominciato con un conto: trentacinque crate, il demone
ne raggiunge diciassette, quindi diciotto non li esegue nessuno. Quel conto
l'ho scritto nel diario, nel documento della beta e in due messaggi di
commit.

Era sbagliato, e per un motivo che avrei dovuto vedere subito: ho seguito le
dipendenze a partire da **tre** binari — il demone, la riga di comando, il
guscio — e ho chiamato «morto» tutto il resto. Ma NOVA pubblica quindici
eseguibili, e sono scritti in un file apposta, `core/binari.json`, che esiste
proprio perche' quell'elenco stava sparso in tre posti e si disallineava.
L'ho letto e l'ho usato piu' volte; non mi e' venuto in mente mentre
contavo.

Rifatto il conto da tutti e quindici: **ventuno raggiunti, quattordici no**.
`nova-cartelle` e `nova-catalogo`, che avevo messo fra i morti, hanno un
binario loro e li chiama l'installatore — c'e' scritto nella prima riga del
loro banco, «lo chiama l'installatore al posto di `python -c`».

Cosa ne porto via. Il difetto non e' il numero: e' che ho misurato una cosa
(«chi raggiunge il demone») e l'ho raccontata come un'altra («chi esegue
qualcosa»), senza dire quale delle due stavo guardando. Se avessi scritto la
domanda per intero — «raggiungibile **da quali** binari?» — la risposta
avrebbe avuto bisogno dell'elenco dei binari, e l'elenco dei binari esiste.
Un conto senza il suo metodo scritto accanto e' un'opinione con dentro una
cifra.

E vale la pena notare da dove e' saltato fuori: non da una prova, ma dal
crate dopo. Cercando dove attaccare `nova-cartelle` ho trovato che era gia'
attaccato. Il lavoro di attaccare i crate sta correggendo il conto che l'ha
fatto cominciare.

## Ho scritto codice per Windows e l'ho spedito senza compilarlo

Il demone doveva scrivere l'ora locale, e il fuso e' una domanda di sistema:
`localtime_r` su unix, `GetTimeZoneInformation` su Windows. Ho scritto tutti
e due i rami, ho eseguito le prove su Linux — verdi — e ho spedito.

Il ramo Windows non compilava. `TIME_ZONE_ID_DAYLIGHT` in `windows-rs` non
esiste come costante esportata: c'e' `TIME_ZONE_ID_INVALID` e basta, e il
valore giusto (2) sta nella documentazione della funzione. Tre parole, e il
lavoro Rust della CI e' diventato rosso senza nemmeno arrivare alle prove.

La parte che conta non e' l'errore: e' che **si poteva vedere prima, qui**.

```
rustup target add x86_64-pc-windows-msvc
cargo check -p nova-platform --target x86_64-pc-windows-msvc
```

Due comandi, meno di un minuto il secondo, e l'errore esce identico a quello
della CI. Non vale per tutto il progetto — `nova-core` tira dentro `ring`,
che e' C e non si compila da qui — ma vale **esattamente per il crate dove
sta il codice per piattaforma**, cioe' l'unico posto dove serve.

La regola che me ne accorgo e' piccola: quando tocco un `#[cfg(windows)]` o
un `#[cfg(unix)]` dentro `nova-platform`, prima di spedire eseguo quel
`cargo check` per l'altro sistema. Scrivere codice che nessuna macchina qui
guarda e' la stessa cosa che scriverlo e non provarlo, e il documento della
beta lo dice gia' per la CI: «prima si accende la luce, poi si guarda».

E una seconda cosa, dallo stesso rosso. Il lavoro dei gemelli e' diventato
rosso anche lui, e non per colpa del codice: gli avevo chiesto di costruire
il demone — perche' la prova nuova lo accende davvero — e su quella macchina
mancava `libasound2-dev`. Il demone tira dentro la voce, la voce parla ad
ALSA. Era scritto nel documento della beta, a proposito di un altro lavoro
della CI, e non mi e' venuto in mente: **aggiungere un bersaglio a un lavoro
vuol dire aggiungergli anche cio' che quel bersaglio si porta dietro**.

## Il banco gemello passava, e una meta' distruggeva file

`file_disco::sposta` e `move_path` sono la stessa funzione in due lingue, e
un banco le confronta operazione per operazione. Il banco era verde. Con
`overwrite=true` su una destinazione che esiste gia', il Python manda nel
Cestino quello che c'era e poi sposta; il Rust faceva `rename` sopra, e il
file di destinazione spariva **per sempre**, senza che niente lo dicesse.

Il banco non se n'e' accorto per una ragione precisa, e vale la pena
scriverla: fra le trentuno operazioni provate c'era uno spostamento sopra un
file esistente **senza** `overwrite`, dove le due meta' si comportano uguale
(si rifiutano). Il caso con `overwrite` non c'era. Un banco gemello prova i
casi che gli si danno, e quelli che non gli si danno non li prova: la
copertura non arriva dal fatto che le due meta' vengono confrontate, arriva
da quali casi si scelgono.

La regola che me ne accorgo: **di ogni parametro che cambia il verso di
un'azione — `overwrite`, `permanent`, `replace_all`, `force` — vanno provati
tutti e due i valori.** Sono pochi, si contano, e sono esattamente i posti
dove una meta' puo' diventare distruttiva mentre l'altra no.

E un corollario, sul come si prova: la verifica nuova non si limita a dire
«le due meta' dicono la stessa cosa». Guarda anche **nel merito** che lo
spostamento sopra un file, senza Cestino disponibile, si fermi. Se un giorno
tutte e due ricominciassero a sovrascrivere in silenzio, resterebbero uguali
e la prova del confronto passerebbe lo stesso.

## Le guardie dell'utente e quelle del demone erano due elenchi diversi

`config.json` ha `safety.write_roots` e `safety.protected_paths`: e' quello
che l'utente scrive dal pannello. `core.json` ha `write_roots` e
`protected_paths` suoi: e' quello che legge il demone, e l'utente non l'ha
mai visto.

Finche' il demone non toccava i file erano due elenchi che non si
incontravano. Nel momento in cui gli strumenti sui file sono entrati nel
demone, sono diventati due risposte alla stessa domanda — e chi aveva
scritto «NOVA puo' scrivere solo in Documenti» nel pannello **non era
protetto** dal processo che esegue.

E' la terza volta che questo progetto paga lo stesso conto: era D56 per i
percorsi, D185 per i comandi vietati, e qui di nuovo. La forma e' sempre la
stessa — due elenchi della stessa cosa, in due posti, che divergono senza
che nessuno se ne accorga perche' nessuno dei due e' *sbagliato*.

Adesso valgono tutti e due, e la regola del come non e' ovvia: per i divieti
si uniscono (piu' divieti = piu' stretto), per le **cartelle autorizzate**
no. Unire due elenchi di cartelle autorizzate autorizza *di piu'* di
ciascuno dei due — cioe' il verso opposto a quello che chi scrive una
guardia si aspetta. Li' resta l'incastro: si scrive dove tutti e due dicono
di si', e se non c'e' incastro non si scrive da nessuna parte.

Dentro al demone, poi, c'era un secondo `check_write` scritto a mano che
confrontava i prefissi **senza separatore**: autorizzare `C:\dati`
autorizzava anche `C:\dati-altrui`. E' il difetto che il commento di
`guardie.rs` racconta come «gia' corretto dall'altra parte» — e che era
rimasto qui, cioe' proprio nel processo che esegue. Adesso il controllo dei
percorsi passa per `Guardie` come gia' faceva quello dei comandi: una sola
implementazione, provata da tutte e due le parti.

## E il banco gemello dipendeva dal computer su cui girava

Corollario del difetto qui sopra, scoperto facendolo. La prova nuova — lo
spostamento sopra una destinazione che esiste — passava qui e diventava rossa
sulla CI, e non per il codice: il Cestino. Dalla mia parte `send2trash` non
funziona, quindi il Python si fermava come il Rust; sull'agente della CI
funziona, quindi il Python spostava e il Rust si fermava. Due risposte
diverse per due computer diversi, lette dal banco come un difetto del
porting.

Il Cestino e' l'unica cosa di quella famiglia che dipende dal sistema, e il
Rust la teneva gia' dietro un tratto (`Sistema`) apposta — solo che il banco
ci passava `SenzaSistema`, cioe' «il Cestino non c'e' mai», mentre dall'altra
parte c'era quello vero. Adesso tutte e due ne ricevono uno **finto e
identico**: sposta in `.cestino` accanto. E' meglio anche per un'altra
ragione: con `SenzaSistema` il caso in cui il Cestino **funziona** non si
confrontava mai.

La regola: **in un banco gemello, cio' che dipende dal sistema si mette
finto da tutte e due le parti.** Se una meta' lo ha vero e l'altra no, il
banco smette di confrontare due porting e comincia a confrontare due
computer.

E una cosa in piu', uscita da li'. Mettere il Cestino finto dentro il corpus
ha fatto emergere una divergenza che c'era da sempre: `search_files` non
torna i risultati nello stesso ordine. Il Rust ordina per nome a ogni
livello, il Python li prende nell'ordine del filesystem; col corpus di prima
coincidevano per caso. Non si aggiusta facendo copiare al Rust l'ordine
dell'altro — quello dipende dal filesystem, cioe' cambia da macchina a
macchina — e ordinare e' il comportamento giusto. Per ora il caso nuovo sta
in fondo all'elenco delle operazioni, dopo le ricerche, e la divergenza resta
scritta qui invece di essere riscoperta fra sei mesi.

## E, sempre dallo stesso filone, due modi di capire un percorso

Dentro `caps.rs` c'era una seconda `espandi`, scritta a mano, che faceva
**meno** di quella vera: `%VAR%` solo su Windows, `~/` solo altrove. Quindi
`fs.read` con `~/Documenti/x` falliva su Windows mentre `fs.search` con lo
stesso percorso funzionava — due strumenti della stessa famiglia che
capiscono due linguaggi diversi. Per chi scrive la domanda e' inspiegabile,
e il modello quei percorsi li scrive come li ha visti scritti da qualche
parte.

Adesso e' la stessa di `file_disco`, cioe' la stessa che usa NOVA lato
Python, provata da un banco. Tre buchi in un giorno, tutti della stessa
forma: **la stessa cosa scritta due volte in due posti**. Comincio a
pensare che valga la pena cercarla di proposito invece di aspettare di
inciamparci.

## Un banco che pretendeva l'ultimo bit uguale su due macchine

Il banco di `nova-giudizio` confronta 1320 giudizi fra la meta' Rust e una
seconda scrittura in Python. Verde qui, rosso sulla CI — e la differenza non
era in nessuna delle due meta'.

Una politica confronta una probabilita' **calcolata** con una costante:
`indisponibile >= massimo_indisponibile`. Cinquantatre casi del corpus cadono
esattamente sul confine (per esempio una distribuzione piatta su due
candidati: 0.5 tondo). Li' `exp` e la divisione possono finire da una parte o
dall'altra dell'ultimo bit a seconda della libreria matematica della macchina,
e la decisione cambia. Non e' un difetto: e' una proprieta' del disegno.

La tentazione era allargare la tolleranza. Sarebbe stato il rimedio sbagliato:
la tolleranza copre i numeri, non le **decisioni**, e allargarla avrebbe
nascosto anche differenze vere.

Due rimedi, e servono tutti e due perche' rispondono a due domande diverse.

**Il banco dichiara i casi sul filo.** Entro `FILO = 1e-9` da una soglia si
accetta l'uno o l'altro esito — ma le probabilita' devono coincidere lo stesso,
e i casi sul filo **si contano**, con una prova che li tiene sotto un decimo
del corpus. Se un giorno diventassero la maggioranza, vorrebbe dire che il
confronto non prova piu' niente, e si vedrebbe.

**Il confine si fissa altrove.** Accettare due esiti sul filo vuol dire che il
banco non inchioda piu' la semantica di `>=`, e infatti la mutazione `>=` → `>`
e' passata. Quindi il confronto e' uscito da `giudica` ed e' diventato
`ci_si_ferma(prima_e_speciale, indisponibile, soglia)`: tre argomenti, nessun
esponenziale, e una prova di unita' che scrive `0.5` e `0.5` a mano. Li'
l'uguaglianza e' esatta su qualunque macchina IEEE, e la mutazione torna rossa.

La regola generale: **un banco gemello prova che due meta' sono d'accordo, non
cosa hanno deciso.** Dove una decisione dipende da un confronto esatto fra
numeri calcolati, quella decisione va provata dove i numeri si scrivono invece
che dove si calcolano.

## E dieci decimi non facevano uno

Seguito del paragrafo qui sopra, e la causa vera. Il banco restava rosso sulla
CI anche dopo aver dichiarato i casi sul filo delle soglie, e finalmente
l'annotazione ha detto quale:

```
caso #363 punteggio politica={'puo_astenersi': False} logit=[0.0 x 10]
  statistiche [4.5, 2.87228…43, 4.0, 0.0, 9.0]
           vs [4.500000000000001, 2.87228…48, 4.0, 0.0, 8.0]
```

Una distribuzione piatta su dieci livelli. Le due meta' differivano di **un
ulp** sulla media — 4.5 contro 4.500000000000001, dentro la tolleranza e senza
alcuna importanza — e di **un'ancora intera** sul novantesimo percentile.

La ragione e' che sommare dieci volte un decimo non fa uno:

```
    somma dei primi 9 decimi = 0.8999999999999999   ->  non raggiunge 0.9
    somma di tutti e 10      = 0.9999999999999999
```

Quindi `cumulata >= 0.9` e' falso all'ottavo livello e vero al nono, e basta
che gli ultimi bit cadano dall'altra parte — cosa che fanno, da una macchina
all'altra — perche' `p90` salti da 8 a 9. Matematicamente la risposta giusta e'
8: nove livelli da un decimo **sono** nove decimi.

Il rimedio non e' una tolleranza di confronto nel banco: quella copre i numeri
vicini, e qui la differenza e' un livello intero. Il rimedio e' nella funzione,
in tutte e due le meta': `cumulata + 1e-12 >= q`. Non e' una comodita' — e' la
constatazione che una cumulata di probabilita' porta con se' l'errore di dieci
addizioni, e che un quantile che cambia per quello non e' un quantile. Mille
miliardesimi sono enormemente piu' dell'errore accumulabile e enormemente meno
di qualunque differenza che significhi qualcosa, e due prove lo fissano da
tutte e due i lati: senza margine il caso dei dieci decimi torna rosso, e con
un margine grosso (0.06) torna rosso l'altro, quello che verifica che il
margine non inghiotta un livello vero.

**Due lezioni, e la seconda e' quella che mi terro'.** La prima: un confronto
fra una somma cumulata e una costante e' un confine, come lo era la soglia
della politica. La seconda: ci ho messo **tre giri di CI** a saperlo, perche'
l'annotazione prende la coda del log e il dettaglio delle differenze veniva
stampato a meta' corsa. Il terzo giro non e' servito a riparare niente: e'
servito a far dire al banco *perche'* era rosso. Quella riga andava scritta il
primo giorno — e adesso il dettaglio di ogni rosso viene ristampato in fondo,
col caso per intero, cosi' si rifa' in locale invece di indovinarlo.

## Ho letto diciassette prove saltate come diciassette prove passate

Spostavo centonove `test_*.py` dalla radice a `prove/`, e per sapere quali si
fossero rotte giravo tutta la suite con un ciclo di shell:

```bash
python3 "$t"
echo "$(basename $t) $?"
```

Diciassette prove che prima uscivano 2 — «qui non si puo' provare» —
risultavano uscite 0. Per un minuto buono ho creduto di aver *aggiustato*
qualcosa spostando dei file, il che avrebbe dovuto insospettirmi subito.

**Credevo** che `$?` valesse ancora l'uscita di `python3` quando `echo` lo
espande. **Era vero** che la sostituzione di comando `$(basename …)` gira in
una sottoshell, quella sottoshell finisce bene, e `$?` viene **riscritto a
zero prima** che l'`echo` lo legga. Il codice d'uscita che stavo leggendo era
quello di `basename`. **Me ne sono accorto** perche' il risultato era troppo
bello: una prova che chiede il demone acceso non comincia a passare perche' il
suo file ha cambiato cartella.

La cura e' una riga: `c=$?; n=$(basename "$t")`. Prendere il codice **prima**
di qualunque altra cosa.

> La forma e' la quarta dell'elenco — leggere un sintomo come una causa — ma
> con una variante che non avevo ancora incontrato: lo strumento di misura
> che distrugge la cosa misurata *mentre* la misura.

## Ho creduto che spostare dei file non avesse semantica

Stesso lavoro. **Credevo** che riordinare centonove prove in cinque cartelle
fosse un'operazione meccanica: nessuna riga di logica cambia, quindi niente
puo' rompersi. **Era vero** che sette prove si sono rotte, ognuna per una
ragione diversa: un `sys.path.insert` che mancava, una che cercava le sue
sorelle per percorso dalla radice, un `from pathlib import Path` che non
c'era e non si vedeva perche' quel ramo non lo prendeva nessuno, un file di
banco che si era spostato, due ricerche di sorelle, e una — `test_harness.py`
— che non era rotta affatto: aveva trovato **un difetto di NOVA**
(`harness_prova.scopri` guardava solo nella radice, D318).

**Me ne sono accorto** solo perche' ho rigirato tutta la suite invece di
fidarmi del fatto che «non ho toccato codice». Il percorso di un file *e'*
codice: ci sono dentro le dipendenze che nessuno ha dichiarato.

> Lezione riusabile: un riordino e' una modifica, e va provato come una
> modifica. Se non si ha il coraggio di rigirare tutto, non si ha il diritto
> di chiamarlo «solo uno spostamento».

## Ho misurato la meta' sbagliata, tre volte in due settimane

Tre episodi diversi, la stessa forma, e la terza volta ha fatto piu' male
delle prime due perche' avrei dovuto saperlo gia'.

1. **Mutazione ripristinata senza ricompilare.** Rimesso il file com'era,
   rilanciata la prova, ancora rossa: per un momento ho creduto di aver
   trovato un difetto vero. Stavo eseguendo il binario mutato.
2. **Uguale, il giorno dopo.** Stesso gesto, stessa conclusione sbagliata.
3. **Oggi, con la prova nuova sulle CLI.** Ho compilato `novad` in *debug* e
   lanciato la prova, che pero' preferisce il binario di *release* se c'e' —
   e ce n'era uno di due ore prima. La prova diceva «il turno non sa ancora
   lanciare un processo», che era esattamente il comportamento che avevo
   appena tolto. Per un minuto ho cercato il difetto nel codice nuovo.

**Credevo** che «ho cambiato il sorgente» implicasse «sto provando il
sorgente». **Era vero** che fra i due c'e' un passaggio — la compilazione, e
*quale* delle due compilazioni — che non e' automatico e non avvisa.

La cura non e' ricordarsene: e' che il gesto di ripristino e quello di
ricompilazione stiano **nello stesso comando**, sempre, cosi' non si possono
separare per distrazione.

## Due mutazioni insieme non sono due mutazioni

Provavo le sei capacita' di sistema. Volevo verificare tre cose, e per
risparmiare una compilazione le ho mutate tutte e tre in un colpo: tolta la
registrazione, tolta la riga della batteria, e fatto scartare lo zero da
`arg_i64_opt`.

Due sono diventate rosse. La terza — «zero e' una richiesta, e ci prova» —
**e' passata**, e per un istante ho pensato che il controllo fosse buono e la
mutazione innocua.

**Credevo** che tre mutazioni indipendenti nel sorgente dessero tre verdetti
indipendenti nella prova. **Era vero** che senza la registrazione `sys.volume`
non esiste affatto, quindi l'errore diventa «capacita' sconosciuta», che non
contiene la frase che il controllo cerca — e il controllo passava **a vuoto**,
esattamente come sarebbe passato con il codice giusto. **Me ne sono accorto**
perche' il conto non tornava: mi aspettavo tre rossi e ne avevo due.

Ripristinata la registrazione e lasciata sola la terza, e' diventata rossa
subito.

> Questa e' la terza forma dell'elenco — la prova che non puo' fallire — ma
> creata **dalla mutazione stessa**, il che e' peggio: e' il caso in cui lo
> strumento che serve a smascherare le prove finte ne fabbrica una. Una
> mutazione alla volta, e ricompilare in mezzo, anche quando sembra uno
> spreco di due minuti.

## Avevo una prova che guardava il collegamento, e credevo guardasse la porta

`nova-core/src/caps_sistema.rs` implementava `Appunti`, `Audio` e `Notifiche`
da mesi. Sotto c'era questa prova, con questo commento:

> Finche' `capacita.rs` aveva solo `NienteSistema`, i tratti erano una
> dichiarazione d'intenti: si compilavano e non li implementava nessuno.
> Questa riga non compila se qualcuno li scollega.

**Credevo** che quella riga tenesse il pezzo attaccato. **Era vero** che
teneva attaccata *l'implementazione al tratto*, e che nessuno registrava il
modulo: dal demone gli appunti, il volume e le notifiche **non esistevano**, e
ogni «copiamelo» faceva ripartire un processo Python. La prova passava, il
commento diceva la verita', e la capacita' non c'era.

**Me ne sono accorto** solo perche' sono andato a cercare *da dove cominciare*
per portare la famiglia `sistema`, e ho trovato il lavoro gia' fatto e non
collegato.

La cosa che devo tenere e' che **questa l'avevo gia' scritta**: la voce «Ho
scritto una diagnostica e non l'ho collegata a niente», qualche centinaio di
righe piu' su, e' lo stesso errore con un altro oggetto. Una volta e' una
distrazione; due sono un'abitudine. La domanda da farsi davanti a ogni pezzo
finito non e' «funziona?» ma **«chi lo chiama?»**, e la prova che risponde a
quella domanda e' diversa da quella che risponde alla prima: interroga la
porta — `capabilities/list` sul demone acceso — non il tipo.

---

## Nota sul registro stesso

Me l'ha chiesto Gio oggi: *«stai aggiornando sempre "dove ho sbagliato"?»*.
No. Fra `E dieci decimi non facevano uno` e le cinque voci qui sopra ci sono
**cinque commit** in cui non ho scritto niente, e gli errori di quei cinque
commit li ho ricostruiti a posteriori — cioe' nel modo in cui questo registro
dice che non si fa, perche' un errore ricordato e' gia' mezzo riscritto.

Quel che ho notato mentre li riscrivevo: gli errori che finiscono qui sono
quelli che mi hanno *fermato*. Quelli che mi hanno solo rallentato — il
binario vecchio, la mutazione mascherata — li archiviavo come attrito e
andavo avanti, e sono proprio quelli che si ripetono, perche' non costano
abbastanza da farsi ricordare da soli.

Correzione anche a «Le forme che si ripetono»: dice «una di queste quattro» e
ne elenca cinque. Contate, non ricordate — che e' la prima voce dell'elenco.

## Ho messo una porta sul retro accanto alla guardia

D143 è una delle decisioni di cui ero più contento: `nova-tastiera` preme i
tasti solo se il fuoco è sulla finestra che ci si aspetta, e lo ricontrolla a
ogni blocco di trentadue caratteri. Se il fuoco scappa, si ferma.

**Credevo** che fermarsi bastasse. **Era vero** che il Python, subito sotto,
leggeva l'uscita del binario così: 0 fatto, 4 fuoco sbagliato, 2 combinazione
storta, **tutto il resto `None`** — e `None` voleva dire «il binario non
c'è, ripiega». Quando il binario si fermava a metà per proteggere l'utente,
il Python ripiegava sulla libreria `keyboard` e riscriveva il testo da capo,
nella finestra che aveva appena preso il fuoco. La guardia fermava il danno e
la riga dopo lo rifaceva, più grosso.

**Me ne sono accorto** portando la stessa cosa nel demone: per scrivere la
versione Rust dovevo decidere cosa fare di ogni codice d'uscita, e davanti
all'1 la risposta «ripiega» era evidentemente sbagliata. Non me n'ero accorto
scrivendo il Python perché lì il caso interessante era il 4, e l'1 l'avevo
lasciato cadere nel «resto».

La lezione è precisa: **un ripiego deve distinguere «non c'è» da «c'era e si
è fermato»**. Sono due domande diverse con la stessa faccia — nessuna
risposta utile — e la prima si risolve provando altrove, la seconda si
peggiora. Ogni volta che scrivo un `return None` che fa ripiegare chi chiama,
la domanda da farsi è: *e se il primo tentativo avesse già agito?*

> Rientra nella terza forma — la prova che non poteva fallire — dalla parte
> opposta: la prova di D143 provava la guardia, e la guardia funzionava. Non
> c'era nessuna prova su **chi chiama la guardia**. Adesso c'è, e sul codice
> vecchio fa nove rossi su dodici.

## Ho scritto una lezione e non sono andato a cercarla altrove

Ieri: «un ripiego deve distinguere "non c'è" da "c'era e si è fermato"», a
proposito della tastiera. Ho corretto la tastiera, ho scritto la voce, e sono
passato oltre.

**Credevo** che fosse un difetto di `type_text`. **Era vero** che era una
forma, e che la stessa forma stava in altri due posti dello stesso
progetto: `open_application`, che sul tempo scaduto rilanciava il programma,
e `set_volume`, che dopo un «muto» riuscito ripiegava sul tasto di Windows
che inverte e rimetteva il suono. **Me ne sono accorto** solo perché il
giorno dopo ho aperto `apps.py` per un altro motivo.

È la seconda volta che questo registro ha la stessa voce con un altro
oggetto — «Ho corretto un difetto in un posto e l'ho lasciato nell'altro» è
qualche centinaio di righe più su. Quella volta erano due copie dello stesso
codice; questa volta sono tre codici diversi con la stessa idea sbagliata, che
è più difficile da vedere perché `grep` sul nome della funzione non la trova.

La cura non è ricordarsene: è che **una voce di questo registro che parla di
una forma si chiude con un giro su tutto il progetto**, fatto quel giorno. Il
giro l'ho fatto oggi — sette punti in cui il Python lancia un binario con un
ripiego, e una tabella nel diario che dice quali sono pericolosi e perché. Il
criterio del giro non era il nome, era la domanda: *se il primo tentativo
avesse già agito, ripeterlo farebbe danno?*

## Ho riscritto un pezzo che c'era già. La terza volta

Portando Claude Code nel turno in Rust mi serviva lo sportello dei permessi:
la domanda che vede l'utente, il suo peso, la risposta che Claude sa
leggere. Ho letto le funzioni in `nova/mcp_kb.py`, le ho scritte in
`nova_cervelli::claude`, ho scritto il banco gemello, verde al primo colpo.

**Esistevano già**, in `nova-mcp`, con lo stesso nome, già confrontate col
Python da `test_mcp_rust.py`. **Me ne sono accorto** perché
`test_niente_due_volte.py` ha trovato `PAROLE_PESANTI` in due crate. Due ore
dopo, lo stesso guardiano ha trovato `AUTONOMIA_PREDEFINITA`.

È la terza voce di questo registro con questa forma («Ho chiesto "cosa c'è
già" a metà del progetto», «Ho riscritto un pezzo che c'era già. Due volte,
nello stesso giorno»). Le prime due volte la lezione era «chiedi prima»; non è
bastata, perché «chiedere» l'ho fatto — ho cercato `session_id`,
`total_cost_usd`, `sessione.json` — ma ho cercato **le cose che mi aspettavo
mancassero**, non quelle che stavo per scrivere.

La cosa che cambio è meccanica: **prima di scrivere una funzione che porta
qualcosa dal Python, cerco nel Rust il nome della funzione Python senza il
trattino basso** (`_in_chiaro` → `in_chiaro`). È quello che avrebbe trovato
tutte e tre le volte. E il guardiano dei doppioni resta la rete sotto: ha
preso tutte e due le ricadute di oggi.

## Ho provato le capacità dalla porta sbagliata

Venti capacità aggiunte in due settimane — `sys.*`, `app.*` — ognuna con una
prova col demone vero, tutte verdi. Tutte le prove chiamavano
`capabilities/call`. Claude Code invece chiama `tools/call`, e da lì un testo
tornava tra virgolette e con gli a capo scritti `\n`.

**Credevo** che provare il demone vero volesse dire provare ciò che vede chi
lo usa. **Era vero** che il demone ha due porte, e ne provavo una. **Me ne
sono accorto** solo scrivendo lo sportello dei permessi, la prima capacità la
cui risposta *deve* essere letta da Claude come JSON: tra virgolette non lo
era.

È la stessa forma di «Avevo una prova che guardava il collegamento, e credevo
guardasse la porta», un giorno dopo: allora la porta era `capabilities/list`
contro l'implementazione, oggi `tools/call` contro `capabilities/call`. La
domanda da farsi è una: **chi userà questa cosa, e da dove entra?** — e la
prova entra da lì.

## Ho scritto un secondo lettore di pagine, e il suo banco lo copriva

Portando gli strumenti in `nova-strumenti` ci ho messo anche «cosa dice una
pagina»: `a_testo`, `titolo_di`, e una tabella di duemila entità HTML
estratta da Python con uno script suo. Un banco gemello la confrontava col
Python su una trentina di pagine, verde.

**Esistevano già**, in `nova-browser`, con il loro banco. E la mia copia
**sbagliava due casi** che l'altra sa fare: un blocco invisibile dentro un
altro (`<svg><style>x</style>y</svg>z` dava `y z`, Python dà `z`, perché la
chiusura va cercata per il tag che ha aperto) e il separatore di riga
` `. Il suo banco non li vedeva perché il suo corpus non li conteneva.
**Me ne sono accorto** oggi, cercando quale dei due usare per `rete.leggi`:
due funzioni con lo stesso nome e la stessa prima riga di commento.

È la quarta voce di questo registro con la forma «ho riscritto ciò che
c'era», ma con una cosa in più che vale la pena dire: **un banco verde non
dice che una copia è giusta, dice che è giusta sul suo corpus**. Due copie
con due corpus diversi possono essere verdi tutte e due e dire cose diverse.

Il guardiano dei doppioni non poteva vederla: guarda i nomi delle costanti, e
le due tabelle si chiamavano `NOMI` ed `ENTITA`. Adesso guarda anche il
**contenuto** delle tabelle grandi (D327), e la copia l'ho tolta.

## Ho scritto una domanda sul risultato senza rileggere il Python

Nel banco delle cartelle note ho aggiunto un controllo mio, indipendente dal
confronto: «un file che si chiama Downloads non è la cartella dei download».
Rosso. **Credevo** che `known_folders` chiedesse se la cartella è una
cartella; **chiede** `exists()`, e un file con quel nome per lui conta. Il
Rust faceva lo stesso, e il confronto era verde.

Il controllo era sbagliato, non il codice: le domande sul risultato servono a
non farsi ingannare da due metà che sbagliano uguale, ma sono **mie**, e una
mia idea di come dovrebbe andare non è il Python. Il file resta nel corpus
apposta, con scritto perché: se un giorno una metà passa a `is_dir()` e
l'altra no, il confronto diventa rosso.

## Ho scritto a mano un'anteprima che era già dichiarata

Per `rete.apri` ho scritto il `farei` dell'anteprima con due `format!`:
«Apre nel browser: …», «Cerca su Google nel browser: …». Le stesse parole
stanno in `nova-strumenti`, dichiarate per ogni strumento Python e confrontate
da un banco. **Me ne sono accorto** il giorno dopo, per `schermo.cattura`,
quando stavo per scrivere di nuovo a mano «Cattura tutto lo schermo».

Perché non l'avevo visto: l'adattatore che legge le anteprime da un JSON
stava **nel banco**, non nella libreria. Chi scrive una capacità nel demone
cerca nella libreria, non trova niente da usare, e riscrive. Ho spostato
l'adattatore dove lo trova chi ne ha bisogno (D330). È la stessa lezione del
lettore di pagine, vista dall'altra parte: non basta che una cosa esista una
volta sola, deve stare **dove la si cerca**.

## Ho provato l'ora locale su una macchina che non ne ha

Il banco degli strumenti confronta cinque cose che passano dall'ora locale.
La macchina delle prove e la CI girano in UTC: lì lo spostamento è zero da
tutte e due le parti, e un Rust che lo dimentica è verde. **Me ne sono
accorto** preparando una mutazione — lo stampo delle schermate calcolato in
UTC — e chiedendomi, prima di lanciarla, in che fuso gira la macchina: UTC.
Sarebbe stata verde, e con lei ogni errore di fuso scritto finora. Adesso il
banco si mette in `Europe/Rome` dove può (D330), e la stessa mutazione è
rossa su tre istanti, uno dei quali è la notte del cambio d'ora.

La domanda che mi faccio adesso quando una prova è verde al primo colpo:
**in questa macchina, il caso difficile c'è?**

## Il demone leggeva la scala a modo suo

Quando ho scritto `dalla_configurazione::scala`, per il turno del demone,
ho letto `brains.routing` com'era nel file, coi valori di ripiego di Rust:
`locale` falso se non c'è scritto, `a_pagamento` falso, nessuna categoria,
nessuna scala di fabbrica se il file non ne ha una. Il Python fa diverso su
tutti e quattro: `locale` vale `brain == "locale"`, `a_pagamento` il
contrario, e la scala di fabbrica sta sotto a quella dell'utente.

**Credevo** che per il turno bastasse: il turno parte dal primo gradino e
sale quando sbaglia, e di `locale` e delle categorie non si serve. **Era
vero** per il turno. **Me ne sono accorto** portando la delega, che di quei
campi vive: con la mia lettura un gradino `"brain": "locale"` scritto a mano
era fuori casa, e con `solo_locale` acceso il demone lo avrebbe rifiutato
mentre il Python lo usava.

La lezione è quella di D185 detta in un altro modo: **una lettura della
configurazione che «basta per chi la usa oggi» è una seconda lettura**, e il
prossimo che la usa eredita le sue differenze senza saperlo. Adesso la
lettura sta in `nova-scala`, confrontata col `_merge` vero (D332).

Portando il `Router` ho trovato anche un difetto del Python, scritto a suo
tempo senza una prova che lo guardasse: ogni sostituto ripiegava a sua
volta, e con `solo_locale` acceso la catena girava in tondo fino a
`RecursionError`. La risposta arrivava lo stesso — dal locale, dopo
novecentonovantotto tentativi — e per questo nessuno se n'era accorto (D331).

E ancora una volta il guardiano dei doppioni: per allegare i file a una
delega ho scritto `allega` in `nova-scala`, e c'era già in `nova-mcp`,
confrontata col Python. Rosso prima del commit, per `MAX_CARATTERI_ALLEGATI`.
Le due funzioni Python differiscono per una riga — quella della delega avvisa
che ci si è fermati — e adesso in Rust la regola è una, con quella riga come
scelta di chi chiama. Avevo cercato «allega» nei nomi delle funzioni Python,
come mi ero ripromesso; non l'avevo cercato nel Rust.

E una prova che dipendeva dalla velocità della macchina: controllavo che
l'intestazione della risposta dicesse la durata, che il Python scrive solo
se è almeno un millesimo. Qui il cervello finto ci metteva qualche
millesimo, sulla CI meno: rosso là, verde qui. È la stessa voce di D235, la
prova che misurava la velocità della macchina; adesso la durata si guarda
solo se c'è.

## La voce aveva una conversazione sua

Portando il turno nel demone, nel guscio avevo scritto
`turno(&domanda, if dalla_voce { "voce" } else { "" })`, e il secondo
argomento era il **nome della sessione**. Due errori in uno: la voce finiva
in una conversazione diversa dalla chat — contro D307, che dice l'opposto
con le stesse parole — e il demone non riceveva mai la bandierina `voce`,
cioè mai la postilla con i marcatori con cui la voce capisce che il discorso
è chiuso. Il banco del demone provava la bandierina chiamando il demone
direttamente, e passava; nessuno provava cosa mandava **il guscio**.

L'ho visto solo perché dovevo aggiungere un argomento a quella funzione. La
richiesta adesso la costruisce una funzione sola, con una prova che dice che
la voce è una bandierina e non una sessione (D338).

## Ho fatto girare le prove del progetto con i file nuovi fuori da git

La prova dei dati personali guarda i file **tracciati**: quelli che git
pubblicherebbe. Io la facevo girare prima di preparare il commit, con i
file nuovi ancora fuori dall'indice — e per lei non esistevano. Cosi' un
percorso di prova con dentro il nome di una persona e' arrivato fino alla CI,
che l'ha fermato. La regola adesso: le prove del progetto si fanno girare
**dopo** aver messo i file nell'indice, come li vedra' chi li riceve.

## Il Python applicava le modifiche a un Word contando da capo

`_applica_docx` prendeva il paragrafo `pN` dal documento **mentre lo stava
cambiando**: dopo un «aggiungi dopo p0», quello che era `p2` diventava `p3`,
e un «togli p2» nella stessa proposta toglieva il paragrafo accanto. Senza
errori e senza avvisi: il documento usciva con un paragrafo in meno, quello
sbagliato. Le prove del Python facevano una modifica per volta, e una
modifica da sola non sposta niente. L'ha trovato il banco contro il demone,
che lavora sugli indici del documento com'era. La regola: una prova di
modifiche a indici ne fa **piu' di una insieme**, e di tipo diverso.

## Ho spostato il turno e ho lasciato indietro quello che gli stava davanti

Quando il turno e' passato dal Python al demone (D305) ho confrontato il
turno: la domanda, gli strumenti, le risposte, la conversazione. Non ho
guardato cosa faceva `python -m nova --ask` **prima** del turno: accendeva
llama-server se il cervello era quello di casa. Quel passo non e' venuto con
il turno, e per giorni chi aveva solo il modello locale non ha avuto
risposte. Io non me ne sono accorto perche' sulla scala di Gio il primo
gradino e' Claude. L'ho trovato riscrivendo il README, cercando chi
chiamasse `modello.accendi`: nessuno (D358). La regola: quando si porta una
funzione, si porta anche quello che le succede intorno, e si cerca chi
chiama davvero cio' che si lascia al suo posto.

## Ho mandato al modello di casa un prompt che non ci stava

Quando il turno e' passato nel demone (D305), gli strumenti per i cervelli in
HTTP sono diventati tutto il registro: 129 invece dei sessanta del Python. Ci
avevo scritto sopra anche il perche', «due elenchi sarebbero due NOVA», e una
prova controllava che il prompt nominasse solo strumenti veri. Ma non ho
misurato quanto pesava la richiesta: 20.939 token, contro un contesto di
16.384. llama-server rifiutava ogni domanda, e chi aveva solo il modello di
casa non riceveva risposte. Non me ne sono accorto perche' sulla scala di
Gio il primo gradino e' Claude. L'ha trovato il banco del prompt, il 29
settembre, cercando un numero per il README (D361).

Nello stesso punto c'era un secondo errore dello stesso tipo. Il taglio della
conversazione usava tutto il contesto come se fosse libero, e il Python
invece toglieva prima il prompt e gli schemi.

La regola: quando cambia quello che va dentro una richiesta, si misura contro
il contesto piu' piccolo su cui deve girare, e una prova tiene quel conto.
Portando una funzione, si porta anche il conto che le stava intorno.

## Ho portato la ricerca senza la sua strada principale

`web_search` del Python cerca prima con un browser senza finestra, su Bing,
e solo se quello non va raschia DuckDuckGo. Il docstring spiega anche perche':
i raschiatori si erano gia' rotti una volta senza dirlo. Portandola nel
demone (`rete.cerca`) ho preso i raschiatori e ho lasciato il browser, con
una nota: «qui il browser non c'e' ancora». Poi il browser nel demone e'
arrivato, per `web_*`, e nessuno e' tornato alla ricerca. Il suo messaggio
d'errore ha continuato a dire che «il browser guidato non e' ancora collegato
al demone», che era falso da quando `web.*` era arrivato.

Nel frattempo DuckDuckGo ha smesso di rispondere a una richiesta semplice con
dei risultati. L'ha visto il banco del browser il 29 settembre: cinque
ricerche, cinque vuote (D362). Adesso `rete.cerca` fa come il Python, prima
il browser senza finestra e poi i raschiatori, e il messaggio dice cosa e'
successo a ciascuno (D363).

La regola: una nota «questo manca ancora» nel codice e' un debito con un
nome. Quando arriva il pezzo che mancava, si cercano tutte le note che lo
nominano, e anche i messaggi che lo raccontano.

## Ho scritto «come il Python» senza guardare il Python

Portando la ricerca nel demone (D363) ho scritto nel ciclo che legge la
pagina: «si riprova fino alla scadenza, come il Python». Il Python non
riprovava. Un errore nella lettura della pagina usciva da `cerca.cerca` come
stack, fuori dal `try` che prometteva un motivo. Me ne sono accorto per la
CI rossa del commit dopo: la prova che doveva provare proprio quella
promessa accendeva un Edge vero e dipendeva da Bing (D364).

Anche quella prova credeva una cosa falsa: «la porta e' chiusa, quindi non
serve un browser». Ma `cerca.cerca` il browser lo accende, e sulla CI Windows
Edge c'e'. L'Edge acceso dalla prova restava vivo, teneva occupato il profilo,
e la parte 4 della stessa prova non poteva piu' accendere il suo: sulla CI
di `277c0ac` si e' dichiarata non provabile, e sul PC di sviluppo (GPU RTX 4060 Ti, 16 GB di VRAM; 32 GB di RAM DDR5; scheda madre Gigabyte B650 EAGLE AX; CPU Ryzen 5 7600X) pure.

Adesso il Python riprova come il Rust, tutti e due dicono l'ultimo errore, e
la prova non accende niente.

La regola: «come il Python» in un commento e' un'affermazione come un'altra,
e si verifica sul codice prima di scriverla. E una prova che dice «qui non
serve X» deve togliere X, non sperare che manchi.

## Ho portato la memoria senza chi creava il vault

Nel Python il vault lo creava `Vault(...)` all'avvio di NOVA: una riga,
`self.root.mkdir(parents=True, exist_ok=True)`, dentro il costruttore. Portando
la memoria nel demone ho portato la lettura, la scrittura, l'apprendimento, e
non quella riga. Il demone controllava che la cartella ci fosse: se non
c'era, chi imparava lasciava perdere in silenzio, e `kb_nota` rispondeva che
la memoria non c'era. E il commento di `memoria::percorso`
diceva che il vault «lo mette l'installatore», cosa che l'installatore non ha
mai fatto.

Su un'installazione nuova, quindi, la memoria non c'era proprio. Non me ne
sono accorto perche' sul PC di sviluppo (GPU RTX 4060 Ti, 16 GB di VRAM; 32 GB di RAM DDR5; scheda madre Gigabyte B650 EAGLE AX; CPU Ryzen 5 7600X) il vault c'e', creato dal Python, che
l'ha seminato il 19 agosto. L'ho visto il 30 settembre, cercando dove agganciare la prima
mappatura del PC (D365): per seminare il vault serve che ci sia, e non lo
creava nessuno.

Adesso lo crea il demone quando si accende, e il commento dice il vero.

La regola: quando si porta un pezzo, si guarda anche cosa fa il suo
costruttore, e chi lo chiamava all'avvio. Una riga che crea una cartella non
sembra una funzione, e per questo non finisce nell'elenco di quelle da
portare.

## Ho portato le chiavi della memoria senza chi le leggeva

`kb.enabled` e `kb.inject_context` sono nel README, nella tabella della
memoria, e il Python le leggeva: la prima in `prepara_kb`, la seconda
in `_contesto_kb`. Portando la memoria nel demone ho portato i metodi, e non
le due righe che decidevano se chiamarli. Il demone le chiavi non le
guardava: chi spegneva la memoria se la ritrovava accesa, e nessun messaggio
lo diceva.

Me ne sono accorto scrivendo la semina (D365), che `kb.enabled` doveva
guardarla, e cercando chi altro la guardasse: nessuno. Adesso le guarda la
memoria stessa (D366).

La regola: una chiave di configurazione portata vuol dire portato anche chi
la legge. Quando si porta un modulo, per ogni chiave che il README elenca si
cerca nel codice nuovo chi la legge; se non la legge nessuno, la chiave e'
una promessa che non si mantiene.

## Ho dato a Gio una garanzia provata su due cartelle

Ho detto che Everyone fra le identita' di controllo non allentava i file,
perche' lo avevo provato sul profilo e su `TEMP`. Una scansione di tutti i
dischi ne ha trovate cinque su 1.141, a tre livelli, che si lasciavano scrivere
dal contenitore.

La regola: «non succede mai» si dice solo dopo averlo cercato dappertutto, o
si dice «l'ho provato su queste due».

## Ho registrato il profilo con il nome nella codifica sbagliata

Il nome `nova.recinto` passato in ANSI invece che in UTF-16 ha prodotto un
profilo di sei caratteri senza senso e un SID diverso. Per una sessione intera
ho guardato i permessi del contenitore sbagliato, e le prove passavano o
cadevano per motivi che non c'entravano.

La regola: dove un nome diventa un identificatore, la prova scrive in chiaro
l'identificatore atteso (ora `windows_l_identita_e_quella_del_nome_giusto`).

## Ho letto per una sessione intera il binario di prima

Il binario release non si era ricostruito, per la cache di cargo, e ho
interpretato risultati vecchi come nuovi.

La regola: dopo ogni modifica che deve cambiare un binario si guarda la data
del binario prima di fidarsi dell'uscita.

## Ho proposto un rimedio senza provarlo

Per le cartelle di terzi aperte a tutti i pacchetti ho proposto un divieto
intestato al SID del contenitore, e Gio l'aveva approvato. Non ferma il
contenitore con nessuna maschera, e nemmeno intestato a una capability che ha
solo lui; togliergli ALL APPLICATION PACKAGES fa non partire PowerShell.

La regola: un rimedio si prova prima di proporlo, in una riga di `icacls`.
Resta una prova-allarme che cade se Windows cambia.

## Ho generalizzato da campioni troppo piccoli, due volte

Per far posizionare PowerShell ho scritto prima «servono tutte le antenate», poi
«basta la prima sotto la radice». Erano vere sul caso che avevo davanti. La
regola misurata, su una catena apposta con sei combinazioni, e' la prima
cartella **e** il nonno.

La regola: prima di scrivere «basta X» si prova senza X, su un caso piu' lungo
di quello che si ha in mano.

## Ho scritto un permesso su una cartella che si ripassa per intero

Scrivere un permesso con la propagazione su `C:\Users\utente` ripassa tutto il
profilo: la prova end-to-end e' rimasta due minuti e mezzo al 100% di CPU. Le
mie prove usavano una radice piccola e non se ne erano accorte.

La regola: una prova che tocca il disco si misura anche su un percorso vero.
La voce di antenata si scrive ora senza ripassare i figli, e una prova misura
la differenza (250 microsecondi contro 403 millisecondi su 6.000 file).

## Ho dato per buoni controlli che non giravano

`cargo fmt --check` ha detto «0 file da riformattare» perche' `rustfmt` non
era installato. Due volte ho cercato un `SyntaxWarning` nel modo sbagliato: la
prima leggendo l'eco della riga di comando come se fosse l'avviso, la seconda
con `-W error`, che lo trasforma in un altro errore. Un filtro con cui
riconoscevo un mio script ha trovato anche lo script che lo stava eseguendo, e
ha fermato la mia stessa chiamata.

La regola: un controllo che non gira non e' un controllo verde. Prima di
fidarsi di un «niente» si guarda che lo strumento esista e che trovi qualcosa
quando c'e' qualcosa.

## Ho scritto prove che dipendono dal non essere amministratore

La prova che si aspetta il rifiuto di Windows su `System32\config` non
rifiuta da amministratore, come sul runner della CI, e avrebbe scritto un
permesso in `System32`. Me ne sono accorto leggendo la CI.

La regola: una prova che aspetta un rifiuto dice in quale ambiente lo aspetta,
e si salta altrove.

## Ho lasciato uno script di laboratorio che scorreva tutta `%TEMP%`

La pulizia leggeva ogni cartella di `%TEMP%`, dove ci sono 8.492 voci
`nova-*` che sembrano residui delle prove del progetto (non l'ho verificato), e
non finiva. Non sono mie, ma l'ho scoperto cosi'.

La regola: uno script di laboratorio si prova prima su un caso piccolo, e non
scorre cartelle che non ha creato.

## Ho fatto partire una scansione di minuti senza un modo di fermarla

Il controllo delle cartelle di terzi gira in un filo bloccante, e partiva subito
all'avvio del demone. Quando il demone si chiudeva, il runtime aspettava quel
filo fino in fondo: tre minuti, e ogni demone di prova con un recinto lanciava
una scansione dei dischi. Le mie prove non se ne erano accorte; la suite intera
si': `test_demone_recinto` non vedeva uscire il demone entro i dieci secondi.

La regola: un lavoro lungo ha sempre una bandiera che lo ferma e un'attesa
prima di cominciare. Ora `novad` la alza quando smette di ascoltare, la prima
scansione parte un minuto dopo l'avvio, e la prova accende il demone con
l'attesa a zero per vedere che si spenga lo stesso: tolta la bandiera, cade.

## Ho aperto la pipe del demone una volta sola, in quattro posti

Su Windows, subito dopo una connessione, il demone ha un istante in cui
nessuna istanza della pipe e' in ascolto, e chi arriva li' trova «tutte le
istanze occupate». Windows prescrive al client di aspettare e riprovare. Il
client Python l'ha imparato con D368; i tre client Rust no: `nova` e il
guscio, due volte, aprivano la pipe e se non si apriva dicevano che il
demone non rispondeva. In CI non si vedeva, perche' le prove del demone ci
girano su Linux, dove il socket fa la fila da solo.

Ora la pipe si apre in un posto solo, `nova_proto::canale::apri`, con la
riprova del Python.

La regola: quando si corregge un difetto in un client, si cercano gli altri
client dello stesso canale. Un difetto di protocollo non sta mai in un
client solo.

## Ho scritto una prova che dava per scontato un progetto senza motore

`test_cli_locali_rust.py` metteva un llama-server finto in una cartella
temporanea e pretendeva che la configurazione scegliesse quello. Ma un
llama-server dentro `runtime/` del progetto ha la precedenza su tutti, ed e'
giusto cosi'. In CI quella cartella e' vuota e la prova passava; sul PC di
Gio, dove NOVA e' installata, era rossa. Python e Rust intanto erano
d'accordo fra loro: il difetto era della prova.

La regola: una prova che guarda dove il prodotto cerca le cose deve sapere
anche cosa c'e' gia' li', o non prova niente fuori dalla CI.

## Ho fatto girare un comando in un filo che nessuno annulla

Su Windows il comando confinato parte in `spawn_blocking`, e un filo bloccante
non si annulla. Su unix `kill_on_drop(true)` ferma il comando quando il turno
viene interrotto; su Windows, dopo il mio cambiamento, il futuro cadeva e il
comando andava avanti da solo fino alla scadenza, di nascosto, dopo che chi
l'aveva chiesto se n'era andato. I commenti dicevano che il job object «ferma
anche i nipoti», ed era vero: ma nessuno lo fermava in quel caso. Lo ha trovato
la rilettura riga per riga, non una prova, perche' nessuna prova interrompeva
un turno.

La regola: quando sostituisco il modo in cui un processo parte, elenco quello
che il modo vecchio faceva senza che io lo scrivessi — qui, morire col turno — e
scrivo una prova per ognuna di quelle cose.

## Ho ritirato un'annotazione che il sistema poteva ancora scrivere

Il passo da amministratore, se non finiva bene, faceva ritirare l'annotazione di
**tutte** le cartelle. Ma finire male puo' voler dire due cose diverse: aver
scritto la voce su due cartelle su tre, e aver scaduto l'attesa mentre la
richiesta di Windows e' ancora aperta (se la persona conferma dopo, il passo
parte e scrive). In tutti e due i casi restava una voce senza annotazione, e una
voce senza annotazione non la toglie piu' nessuno. In piu' tenevo il blocco fra
processi per tutta l'attesa, e i comandi del demone intanto fallivano.

La regola: «ritiro quel che ho annotato» e' vero solo per quello che so che non
e' successo. Si annota, si aspetta *senza* il blocco, e si riconcilia guardando
com'e' andata davvero, cartella per cartella.

## Ho aperto le antenate anche agli strumenti

Le antenate (la prima cartella sotto la radice, il genitore, il nonno) servono a
PowerShell per posizionarsi nella cartella di lavoro. Le calcolavo per ogni
radice, strumenti compresi: per `C:\Users\<nome>\.cargo\bin` finivano in elenco
`.cargo` e il profilo, proprio la cartella con le credenziali che a parole avevo
detto di tenere fuori. Un file non si legge, ma i nomi si vedono. Uno strumento
si esegue per percorso e di antenate non ne ha bisogno: provato, `cargo` parte
lo stesso.

La regola: un permesso si calcola dalla ragione per cui serve, non da una lista
che passa per caso di li'.

## Ho deciso se un disco ha i permessi dal suo nome

Il controllo trattava come «senza permessi» ogni disco che non si chiamasse
NTFS, ReFS compreso. Ma ReFS i permessi li ha (un Dev Drive di Windows 11 e'
ReFS): quel disco non si controllava, e il racconto diceva che li' il
contenitore scrive ovunque. Windows lo dice da solo, con il flag
`FILE_PERSISTENT_ACLS` del volume.

La regola: se il sistema sa rispondere a una domanda, si chiede al sistema, non
a un nome.

## Ho lasciato la documentazione alla strada che avevo scartato

L'intestazione di `windows.rs` descriveva ancora il token ristretto con
`WRITE_RESTRICTED`, il SID di servizio `S-1-5-80-...` e un AppContainer che
«viene dopo». Altri commenti dicevano che il passo da amministratore «fa una
cosa sola» (ne fa due), che le antenate sono «la prima cartella» (sono tre), che
del resto del profilo il contenitore «non vede nemmeno i nomi» (le antenate si
elencano), e un esempio con `%USERPROFILE%` che nessuno espande: chi lo avesse
scritto in `core.json` non avrebbe ottenuto niente, e in silenzio.

La regola: quando cambio la strada, rileggo i commenti in testa ai file che ho
toccato prima di dire che ho finito. E un percorso scritto male in
configurazione si dice, non si salta.

## Ho chiamato verde una prova che lo era solo con una variabile accesa

Nel mio giro su Windows `test_demone_compiti.py` era verde, 9 su 9. Il giro
finale di Gio, lanciato senza `PYTHONIOENCODING`, l'ha trovata rossa: la CLI
finta leggeva stdin nella codifica della macchina, cp1252, e la domanda le
arrivava con «c'Ã¨» al posto di «c'è». Il demone mandava UTF-8, ed era giusto;
sbagliava la finta. I miei script mettevano `PYTHONIOENCODING=utf-8` per leggere
bene l'uscita delle prove, e la stessa variabile, ereditata fino alla finta,
nascondeva il difetto. Corretta la lettura, ne e' uscito un secondo, nascosto
dal primo: la domanda si scriveva in modalita' testo, con `\r\n` su Windows, e
la finta la riportava a `\n` solo perche' leggeva anche lei in modalita' testo.
Ora le quattro CLI finte del demone leggono i byte come UTF-8, come una CLI
vera, e il file della domanda si scrive byte per byte. Le prove del demone in CI
girano solo su Linux, quindi su Windows le vede solo un giro a mano.

La regola: un giro di prove che vale come verifica si lancia con l'ambiente
pulito, senza variabili messe per comodita'. Se una variabile serve, la mette la
prova, non lo script che la lancia.

## Ho contato come saltata una prova che sarebbe stata rossa

Il giro finale su Windows faceva girare le prove da un'esportazione
dell'indice, senza `.git`. `test_niente_dati_personali.py` chiede a git quali
file sono tracciati, e senza git esce 2: «qui non si puo' provare». Era fra le
quattro saltate, e nessuno l'ha guardata, perche' le saltate le conoscevamo.
Su Linux, nel repository, era rossa: un commento di `config.rs` dava come
esempio la cartella `.cargo\bin` di un profilo il cui nome era `<tu>`, e il
solo nome finto ammesso negli esempi e' `utente`. In CI quella prova gira su Linux, e il push sarebbe stato rosso.

La regola: una prova saltata si guarda una per una, e si chiede perche'. Se si
salta per come la lancio io, e non per la macchina, la lancio in un altro modo.

## Ho dato per vero un limite che non avevo misurato

Nel riepilogo della rilettura ho scritto che il blocco fra processi, creato da
un processo elevato, non si apre da uno non elevato. Era una deduzione da come
Windows costruisce i token (nel token normale il gruppo Amministratori serve
solo a negare), non una misura. Quando l'ho misurata la prima volta, con la
classe `Mutex` di .NET, il mutex si apriva: ma .NET, se `CreateMutex` dice
«accesso negato», ripiega su un'apertura con diritti ridotti, quindi non faceva
la chiamata di NOVA e non provava niente. Rifatta con `CreateMutexW` pelato,
come in `BloccoFraProcessi`, in tutti e due i versi: si apre, e il limite non
c'e'. Resta non misurato il caso di un altro account amministratore.

La regola: un limite si scrive dopo averlo misurato con la stessa chiamata del
codice, e dove non si puo' misurare si scrive «non misurato», non il contrario.

## Ho scritto «un secondo al massimo» senza misurarlo

In `nova_proto::canale` la riprova sulla pipe occupata faceva cento tentativi
con una pausa di dieci millisecondi, e il commento diceva «cioe' un secondo al
massimo». Era una moltiplicazione, non una misura. Rileggendo il codice l'ho
misurata su una pipe sempre occupata: 1,56 secondi, tre volte su tre. Su
Windows una pausa di dieci millisecondi ne dura quasi sedici, perche' il timer
di sistema ha quella risoluzione. Il client Python, con la stessa
moltiplicazione, faceva 1,02 secondi su Python 3.13, che dorme con un timer
piu' fine, ma dipendeva anche lui da quanto dura una pausa. Ora tutti e due
riprovano finche' l'orologio non dice un secondo, e due prove lo tengono: il
Rust di prima cade a 1,56 s, il Python di prima, con pause che durano quanto
su Windows, a 1,54 s. Il Python, quando smetteva, dava anche l'errore
com'era, «Invalid argument»: ora dice che la pipe e' rimasta occupata, e
quale.

La regola: una durata scritta in un commento si misura con l'orologio, non si
ricava dal numero dei giri. E un'attesa che deve durare al massimo un tempo si
scrive con una scadenza, non con un conto.

## Ho dato per riprodotta una falla con una cartella che non la riproduceva

La prova di D369 scriveva in un «fortino» che doveva essere scrivibile solo da
un amministratore. L'avevo costruito con `icacls /inheritance:r` e tre
`/grant:r`, ma `/inheritance:r` converte le voci ereditate in esplicite e non
le toglie: dentro `%TEMP%` restava `<utente>:(F)`, e un comando senza poteri ci
scriveva lo stesso. Ho scritto «falla riprodotta» su una scrittura che non
provava niente. Me ne sono accorto leggendo l'ACL vera della cartella, quando
il token ridotto, che il gruppo Amministratori l'aveva perso di certo,
continuava a scriverci.

La regola: prima di fidarsi di una prova negativa («qui non si scrive») si
guarda l'ACL, e la prova controlla la sua premessa prima di usarla.

## Ho cercato il nome dell'utente in un testo che comincia con il suo percorso

Il controllo «l'utente non c'e' piu'» cercava il nome dell'utente nell'uscita di `icacls`,
che comincia con il percorso della cartella, `C:\Users\<nome>\...`. Scattava
sempre: la prova rifiutava un fortino giusto, e lo stesso difetto era nella mia
versione Rust.

La regola: si cerca nella parte del testo che si vuole controllare — qui le
voci, `DOMINIO\utente:` — e non in tutto l'output.

## Ho scritto un controllo che passava quando la misura falliva

Il comando che guardava l'input della console dentro il contenitore usava
`Add-Type`, che li' non gira (`csc.exe` non parte): la lettura non avveniva
mai, il conteggio restava a zero e il controllo cercava soltanto `EVENTI=0`.
Passava anche con la correzione tolta. L'ha scoperto la mutazione, non la
lettura del codice.

La regola: un controllo «non vedo niente» vale solo se la prova dimostra, nello
stesso momento, che stava guardando. E ogni comportamento nuovo ha una
mutazione che lo fa cadere prima che io dica che e' coperto.

## Ho provato il recinto di Windows solo da utente normale

Prima di `e9d298b` il `cargo test` e le prove Python del recinto giravano da
utente normale, sul PC di sviluppo (GPU RTX 4060 Ti, 16 GB di VRAM; 32 GB di RAM DDR5; scheda madre Gigabyte B650 EAGLE AX; CPU Ryzen 5 7600X), e le mie verifiche anche. La sola prova lanciata
da amministratore, `test_demone_recinto_strumenti.py`, non guardava con quali
poteri partisse il comando. La CI di Windows gira da amministratore, e al primo
push ha trovato la falla di D369: con il demone elevato, i comandi confinati
partivano con i poteri dell'amministratore. Da utente normale nessuna prova
poteva vederla.

La regola: quando si tocca il recinto, la suite intera gira due volte, da
utente normale e da amministratore (`test/README.md`, «Fuori dalle suite»).

## Ho controllato il proprietario di una cartella per nome

In `windows_cartella_dei_soli_diritti_del_proprietario` il proprietario della
cartella si controllava cercando il nome dell'utente nel testo che restituisce
PowerShell. Quel testo dipende dalla lingua, dal dominio e dal fatto che
PowerShell risponda. Sul PC di sviluppo (GPU RTX 4060 Ti, 16 GB di VRAM; 32 GB di RAM DDR5; scheda madre Gigabyte B650 EAGLE AX; CPU Ryzen 5 7600X) funzionava; sull'agente Windows della CI
(Windows Server, `runneradmin`, UAC spento) no, e la CI di `8afff6b` e' caduta
con «il proprietario non e' l'utente». Dal log non si capisce quale delle tre
cose sia andata storta.

Ora il proprietario si confronta per SID con l'utente del token. Se la cartella
non si riesce a dare all'utente, la prova guarda il comportamento giusto per una
cartella degli Amministratori, il rifiuto che nomina l'amministratore, e stampa
perche'. Una prova nuova, solo da elevato, copre quel caso senza cambiare il
proprietario; dove l'ambiente da' gia' la cartella all'utente, si salta e lo
dice.

La regola: un'identita' si confronta per SID, mai per nome. E una prova che
dipende dall'ambiente, quando cade, dice quale premessa non ha trovato.

## Ho lasciato una prova leggere lo schermo

`test_anteprime.py` controlla che le anteprime delle conferme siano frasi di al
massimo 200 caratteri. Quelle di `type_text` e `press_keys` contengono il titolo
della finestra in primo piano, che si chiede al sistema. Con davanti una scheda
di Edge dal titolo lungo l'anteprima arrivava a 213 caratteri e la prova
cadeva; con un'altra finestra passava. Il giro da amministratore l'ha trovata
rossa due volte, mentre da sola era verde: non dipendeva da chi la lanciava, ma
da cosa c'era sullo schermo.

Ora, nella prova, la finestra in primo piano e' finta e sempre la stessa, e un
controllo verifica che le anteprime vedano quella. Con la versione di prima e
la stessa scheda di Edge davanti la prova cade (123 su 125); con questa passa.

La regola: una prova non legge lo stato del desktop. Se il codice lo chiede al
sistema, la prova glielo da' finto.

## Ho guardato le schede subito dopo averle chiuse

`test_demone_ricerca.py` controlla che, dopo una ricerca, nessuna scheda resti
aperta nel browser delle ricerche, e lo guardava subito. Ma `/json/close` di
Chromium risponde «Target is closing» e la scheda sparisce dall'elenco poco
dopo: in una misura a parte, guardata subito c'era ancora 18 volte su 20. Su
Linux la prova e' caduta due volte in una giornata di giri completi, con la
scheda della seconda ricerca ancora in elenco, e passava quando la risposta
del demone arrivava abbastanza tardi.

Ora la prova aspetta fino a cinque secondi che la scheda sparisca, e cade solo
se resta.

La regola: dopo un'azione che il sistema completa da solo, la prova aspetta
l'effetto con una scadenza, non lo guarda nell'istante dopo.

## Ho scambiato una pagina di bing.com per il rimbalzo di Bing

`test_cerca.py` fa una ricerca vera e controlla che ogni risultato abbia un
indirizzo vero, non quello del motore. Lo controllava rifiutando qualunque
indirizzo che contenesse `bing.com`. Ma il rimbalzo da sbrogliare e'
`bing.com/ck/a?...`, e una pagina di bing.com puo' essere un risultato vero:
cercando «bing», i primi sono `www.bing.com` e `get.bing.com`. Il 4 ottobre la
CI, su Python 3.10, e' caduta li', con risultati su Microsoft per una domanda
sul fantacalcio. Quale dei sei indirizzi fosse non si sa: la prova ne stampava
solo i primi tre, tagliati a quaranta caratteri.

Ora la prova rifiuta solo il rimbalzo e gli indirizzi senza schema, e quando
cade li stampa interi.

La regola: una prova controlla la cosa che conta, non un suo indizio. E
quando cade, mostra proprio i valori che l'hanno fatta cadere.

## Ho scritto «le stesse sedici domande» senza confrontarle

In testa alle domande di `misure/banco_clm.py` avevo scritto che erano le
stesse sedici di `banco_giudizio_llama.py`. Le domande si', le opzioni no: ne
avevo riscritte alcune per intero («il gatto» invece di «gatto», «da
confermare con l'utente» invece di «da confermare»), e qui la risposta giusta
ce l'hanno dieci domande invece di otto. Me ne sono accorto prima del commit,
mettendo le due liste una accanto all'altra per confrontare i risultati delle
due strade.

Ora il commento dice cosa e' uguale e cosa no.

La regola: «le stesse» si scrive dopo aver confrontato, non dopo aver copiato.
Due banchi che si confrontano devono dire dove differiscono.

## Ho preso per riferimento i numeri sbagliati di un README

Per controllare che `banco_clm.py` faccia i conti di CLM ho rifatto gli esempi
pubblicati. Il primo confronto e' stato con il blocco di codice del README, e
«urgente» veniva 0,82 contro 0,41: sembrava un errore del banco. Ma la
schermata del playground, presa dallo stesso README da un `clm-serve` vero,
per lo stesso cliente dice 0,848, e il banco fa 0,823. I numeri del blocco di
codice vengono da un'altra versione, e non coincidono nemmeno con la schermata.

Ora i controlli usano la schermata e dicono da dove viene ogni numero.

La regola: un numero di riferimento ha una fonte scritta accanto, e se due
fonti dello stesso autore non coincidono, si dice quale si usa e perche'.

## Ho portato il lancio delle attivita' senza il suo «w»

Il Python faceva partire le automazioni e i compiti con `pythonw.exe`, e il
commento diceva perche': «senza finestra nera che compare all'improvviso
mentre si lavora». Portandoli in Rust ho messo al suo posto `nova.exe`, che e'
un programma da console. Dal 2 al 5 ottobre, sul PC di sviluppo (GPU RTX 4060 Ti, 16 GB di VRAM; 32 GB di RAM DDR5; scheda madre Gigabyte B650 EAGLE AX; CPU Ryzen 5 7600X), una console nera
e' comparsa e sparita ogni cinque minuti. Se n'e' accorto lui, e l'ha detto:
«talvolta esce un CMD random e si chiude immediatamente». Cercando, sono
venute fuori altre due cose: l'attivita' puntava a un `nova.exe` di agosto, che
il comando non lo conosceva e falliva a ogni giro senza che niente lo dicesse;
e i promemoria aprivano una console accanto al fumetto gia' nel Python, con
`nova-notifica`.

Ora le attivita' lanciano `novaw`, che per Windows e' un programma a finestre,
e anche `nova-notifica` lo e'. Prima di registrare un'attivita' il demone
chiede al binario se conosce il comando (D370).

La regola: portando una funzione si porta anche il perche' delle sue scelte.
Un `w` nel nome di un eseguibile e' una scelta, e il commento accanto lo
diceva.

## Una prova ha lasciato un'attivita' di sistema sul PC

`test_demone_automazioni` lavora in una cartella di NOVA tutta sua, e mi
bastava questo per credere che non toccasse niente fuori. Ma alla prima voce
del calendario il demone registra «NOVA - pianificazione» nell'Utilita' di
pianificazione, che e' una sola per tutto il PC. L'attivita' sul PC di sviluppo (GPU RTX 4060 Ti, 16 GB di VRAM; 32 GB di RAM DDR5; scheda madre Gigabyte B650 EAGLE AX; CPU Ryzen 5 7600X) e'
nata il 2 ottobre alle 17:57, nei giorni in cui giravo la suite su Windows;
Gio non ha voci in calendario, e solo quella chiamata del demone la registra
con quel comando. Da li' e' cominciata la console nera. Su Linux, dove la CI
fa girare la prova, registrare un'attivita' non si puo', e la cosa non si
vedeva.

E l'ha rifatto il 5 ottobre alle 21:33, nel giro da amministratore del commit
di prima, che la correzione non l'aveva ancora: un'attivita' registrata da un
processo elevato un utente normale non la puo' togliere, e per toglierla e'
servito di nuovo l'amministratore.

Ora la prova guarda se l'attivita' c'era prima: se non c'era la controlla e
alla fine la toglie, se c'era e' dell'utente e non la tocca.

La regola: una cartella isolata isola i file, non il sistema. Una prova che fa
registrare qualcosa al sistema deve toglierlo, e deve sapere se prima c'era.

## Ho aggiunto due idee e lasciato la data di ieri

Il 5 ottobre, nel commit della console nera (D370), ho aggiunto a `idea/`
due voci, il motore che dice quando fallisce e `bin/` vecchio. In testa al
file restava «Aggiornato al 4 ottobre 2026». Me ne sono accorto il giorno
stesso, aggiungendo la voce di OpenDots e rileggendo il file dall'inizio.

Ora la data e' quella dell'ultima voce.

La regola: chi aggiunge una voce a un file che dice quando e' stato
aggiornato, aggiorna anche quella riga. Si rilegge il file intero, non solo
il pezzo nuovo.

## E la stessa data ferma in `piano/`

Un'ora dopo la voce qui sopra, aggiungendo al piano la prova della ricerca,
ho trovato la stessa cosa in `piano/README.md`: «Aggiornato al 3 ottobre
2026», dopo sei commit del 4 e del 5 ottobre che lo avevano cambiato. La
regola scritta un'ora prima l'avevo applicata al file in cui era nata, non
agli altri che hanno la stessa riga.

Ora anche li' c'e' il 5 ottobre.

La regola, allargata: quando si scopre un difetto in un file, si guarda se
c'e' negli altri file che hanno la stessa forma. Delle cinque cartelle, la
data in testa ce l'hanno `piano/` e `idea/`: le ho guardate tutte e cinque.

## Due chiavi finte scritte intere in una prova

Il 6 ottobre, nelle prove del registro delle decisioni (D374), ho scritto
nel sorgente due chiavi finte intere, una col prefisso di Anthropic e una
con quello di GitHub, per provare che non arrivano sul disco. Se n'e'
accorta prima del commit `test_niente_dati_personali.py`, nella suite
intera: per lei erano chiavi, e aveva ragione, perche' la forma e' quella.

Ora le chiavi si compongono mentre la prova gira, da pezzi che presi uno per
uno non hanno la forma di una chiave.

La regola: una prova che parla di segreti non scrive mai un segreto intero
nel sorgente, nemmeno finto. Ho guardato le altre prove nuove di questo
lavoro: la password della prova del demone e' una parola con un numero, che
la prova sui dati personali non tratta come chiave, ed e' quello che serve.

## Una frase del piano rimasta indietro di un commit

Nel piano, la voce di CANT-12 diceva che la meta' del giudizio che chiede a
llama-server c'era, «ma nessuna decisione la chiama ancora». Era vero col
D371; dal D373 la chiama la delega, e la stessa voce, due righe sotto, lo
diceva. Nel commit del D373 avevo aggiunto la frase nuova senza rileggere
quella vecchia. Me ne sono accorto il 6 ottobre, rileggendo la voce intera
per il D375.

Ora la frase vecchia non c'e' piu'.

La regola: quando si aggiunge a una voce un fatto che ne supera un altro, si
rilegge tutta la voce e si toglie quello superato, nello stesso commit. Ho
cercato la stessa forma («nessuna decisione», «nessun binario») nel README,
nel piano, nelle idee e in `verso_la_beta.md`: l'unica altra e' quella di
`nova-mcp-cliente`, che e' ancora vera.

## Una prova nuova, e il conto delle prove rimasto a prima

Il 6 ottobre ho aggiunto `prove/gemelli/test_giudizio_solo_testo.py` e l'ho
descritta in `test/README.md`, ma ho lasciato a 29 il numero delle prove
gemelle, li' e in `prove/README.md`. Se n'e' accorta prima del commit
`test_documentazione.py`, nella suite intera.

Ora dicono 30 tutti e due.

La regola: chi aggiunge una prova aggiorna nello stesso giro la riga che la
descrive e i numeri che la contano. I numeri stanno in due file, e la prova
della documentazione li controlla tutti e due.

## Di chi era il PC, scritto per settimane

Dal 29 settembre al 6 ottobre ho scritto in decine di posti, nei documenti,
nei commenti e nelle prove, di chi era il PC su cui misuravo. E' un dato
personale, e in un repository pubblico. Non serviva a niente: a chi legge un
numero serve sapere che macchina c'era sotto, non di chi era. Se n'e'
accorto chi sviluppa NOVA, il 7 ottobre, leggendo i documenti.

Ora si scrive «PC di sviluppo» con le specifiche, anche in tutta la storia di
git (D376), e la prova sui dati personali cerca la forma «PC di» seguita da
un nome.

La regola: di una macchina si scrive cosa ha, mai di chi e'.

## La sostituzione nella storia ha preso un nome piu' lungo

Riscrivendo la storia ho sostituito la frase come testo, senza fermarla alla
fine della parola. In `nova/cartelle.py` c'era un esempio di copia in
conflitto di OneDrive il cui nome cominciava con le stesse lettere, e la
sostituzione l'ha spezzato a meta': nel file usciva il nome del PC di
sviluppo con le sue specifiche, attaccato alla coda del nome vecchio. Me ne
sono accorto rileggendo tutte le righe toccate prima del commit.

Ora l'esempio dice «documento-NOMEPC.md». Nelle versioni vecchie di quel
file, nella storia, la riga resta spezzata: per sistemarla bisognerebbe
riscrivere un'altra volta tutti gli hash.

La regola: una sostituzione su tutto il repository si fa con il confine di
parola (`regex:...\b`), e prima di pubblicarla si guarda ogni riga che ha
toccato, non solo il conto.

## Una prova dal vivo ha acceso Docker sul PC di sviluppo

Il 7 ottobre, per vedere CLM decidere dal vivo, ho dato al demone una delega
vera: «Progetta lo schema del database per il gestionale delle iscrizioni
alla palestra». CLM si e' astenuto (0,58) e le parole l'hanno fatta salire a
Opus 5.5, che gira con i permessi saltati, e Opus ha fatto il compito per
davvero. Nella sessione di Claude Code, riletta dopo, ci sono tredici
chiamate in nove minuti: ha cercato una cartella del gestionale nella
cartella utente, ha scritto lo schema in `%TEMP%\palestra`, ha provato a
lanciare un contenitore Postgres e, non trovando Docker acceso, ha avviato
Docker Desktop; poi ha creato un ambiente Python in `%TEMP%\pgvenv`, ha
scaricato Postgres 17.7 da EnterpriseDB in `%TEMP%\pgbin` (1,2 GB con lo zip)
e ha inizializzato un database per provare lo schema. Me ne sono accorto
mentre la delega era ancora aperta, da Docker Desktop acceso, e ho chiuso
l'albero di Claude Code. Dopo: Docker spento, nessun contenitore, nessun
Postgres acceso e nessuno in ascolto sulla 5432; le tre cartelle sono
rimaste in `%TEMP%`, con il database inizializzato dentro `pgbin`. Il compito era scritto per misurare la scelta, non per
far lavorare qualcuno, e ho dimenticato che dall'altra parte c'e' un agente
che lavora.

Le deleghe dopo dicevano «Rispondi solo con una parola, senza usare
strumenti», e hanno risposto senza toccare niente.

La regola: una prova dal vivo che arriva a un cervello agentico dice nel
compito stesso di non usare strumenti, e chiede una risposta di una riga.
Se serve misurare un compito che potrebbe agire, si misura sul banco, non
sul PC di qualcuno.

## Ho scritto che CLM ci stava accanto a Gemma, e non ci stava

Il primo testo del D378 diceva che il server dei vettori con Qwen3-8B Q8_0
«ci sta» accanto a Gemma, a 15,9 GB su 16. Il banco pero' diceva 3.786 MiB in
piu', contro i 9.347 dello stesso server da solo: due numeri che non
potevano essere veri insieme. Me ne sono accorto ricontrollando ogni numero
alla fonte prima del commit. Rifatta la prova col registro del server: tutti
i 37 strati sulla scheda, 9,2 GB, che con Gemma fanno 20,4 su 16. Il resto lo
tiene fuori il driver di Windows, e lo stesso banco, nel suo commento, lo
diceva gia' per `-ngl 999`.

Ora il D378, il piano e `verso_la_beta.md` dicono che non ci sta, e cosa si
e' misurato lo stesso.

La regola: un numero di memoria della scheda si legge con quello che il
programma dice di aver messo sulla scheda, non solo con `nvidia-smi`; se due
misure non tornano fra loro, prima si capisce perche', poi si scrive.

## Una prova misurava la configurazione di chi la lanciava

`prove/nova/test_routing.py` costruiva il router con `Config.load()`, cioe'
con la configurazione di chi lancia la prova. Nel contenitore e in CI non ce
n'e' una, e valeva quella di fabbrica. Sul PC di sviluppo, dopo aver messo la
scala del D378 (rapido e difficile), la prova e' diventata rossa: cercava
`locale` e `standard` in testa alla scala, e trovava `rapido`. Se n'e' accorta
la suite intera sul PC, prima del commit.

Ora la prova usa `Config()`, la configurazione di fabbrica. Altre otto prove
chiamano ancora `Config.load()`: sono nel piano, con quella che scrive nel
registro vero.

La regola: una prova non legge la configurazione dell'utente, a meno che non
sia proprio quella che vuole provare; e allora lo dice.

## La scala del PC di sviluppo non era «Flash e Opus, e basta»

Per il D378 ho messo nella configurazione del PC di sviluppo la scala scelta
da Gio, «Flash -> Opus, e basta»: ho aggiunto il gradino `rapido` e scritto
`"scala": ["rapido", "difficile"]`, lasciando `locale`, `standard` e
`alternativo` fra i gradini. Ma la scala accoda i gradini che non elenca: era
`rapido`, `difficile`, `locale`, `standard`, `alternativo`, e sopra Opus
c'era ancora da salire. Me ne sono accorto il 7 ottobre, con `novad
--consiglio`, che stampa la scala in uso per intero.

Ora sul PC di sviluppo ci sono solo `rapido` e `difficile` (il file di prima
e' `config.json.prima-di-D379`), e il bottone della scala consigliata
sostituisce i gradini invece di fonderli.

La regola: cambiare la scala vuol dire scrivere `tiers` per intero; e dopo
si guarda la scala come la legge NOVA, non come la si e' scritta.

## Tre inciampi del pannello della scala consigliata

Presi tutti prima del commit, scrivendo il D379:

- il titolo «Scala consigliata» era un `<h3>`, e `test_impostazioni.py`
  conta gli `<h3>` come schede del pannello: ne ha trovata una nona;
- i paragrafi avevano la classe `aiuto`, che ha uno stile solo dentro un
  `.campo`: nel pannello reso in Chromium uscivano col carattere grande;
- il pannello leggeva il catalogo cosi' com'e' su disco, il demone nella sua
  forma, dove a un catalogo senza `disponibili` ne resta uno vuoto: con un
  catalogo vecchio il primo avrebbe detto «non so ancora», il secondo «non
  c'e' nessun cervello». Ora «non so ancora» vale per tutti e due quando
  `disponibili` e' vuoto, e la prova lo dice.

La regola: una pagina si guarda resa, non solo letta come testo; e due
letture dello stesso file si provano sullo stesso caso.

## Le cartelle lasciate dalla prova della palestra

Le tre cartelle in `%TEMP%` della prova dal vivo del D378 (`palestra`,
`pgvenv`, `pgbin`, con il database) le ho cancellate il 7 ottobre, chiesto
da Gio. Nessun processo di Postgres o di Docker era acceso.

## Rinumerare il piano dopo averci inserito una voce

Due volte il 7 ottobre, aggiungendo una voce in mezzo all'elenco del piano,
ho prima inserito la voce col suo numero e poi spostato in avanti quelle
dopo, cercando ognuna col suo numero: quella appena inserita aveva lo stesso
numero di una vecchia, e lo script si e' fermato sul doppione. La prima
volta non aveva scritto niente; la seconda aveva gia' scritto le modifiche
di prima, e l'elenco aveva due voci 4. Me ne sono accorto dall'errore dello
script, e ho corretto a mano guardando l'elenco intero.

La regola: in un elenco numerato prima si fa posto, dall'ultima voce in su,
e poi si inserisce; e alla fine si guarda l'elenco intero.

## Fermare un Dot non fermava niente finche' il cervello non rispondeva

Il primo ciclo dei Dot faceva il lavoro e aspettava il «ferma» nello stesso
compito tokio, con un `select!`. Ma la domanda a un cervello in HTTP aspetta
dentro `block_in_place`: per tutto quel tempo il compito non torna mai allo
scheduler, e il ramo del «ferma» non viene guardato. Il compito risultava in
corso per altri sei secondi e poi «fatto». Se n'e' accorta
`test_demone_dot.py`, prima del commit, con un cervello finto lento.

Ora il lavoro gira in un compito suo, e il «ferma» lo abbandona subito.
Nella stessa prova avevo fatto chiamare al cervello finto `write_file`, il
nome del Python, mentre al demone gli strumenti si offrono coi nomi delle
capacita' (`fs_write`): il demone l'ha detto, «non e' una capacita'».

La regola: dove si aspetta in modo bloccante non si puo' anche ascoltare un
segnale nello stesso compito; e un cervello finto chiama gli strumenti coi
nomi che il demone gli offre davvero.

## Due fili nella stessa coda, e su Windows le righe si sono perse

Il ciclo di un Dot scrive in `compiti.jsonl` che un compito e' in corso,
mentre dal demone qualcuno gliene affida un altro, che scrive nello stesso
file. Ognuno apriva il file in coda e scriveva la riga con `writeln!`, che su
un file sono due scritture: il testo e l'a capo. Su Linux la prova passava;
sulla suite intera del PC di sviluppo `test_demone_dot.py` e' caduta: il
compito 3 risultava finito senza essere mai cominciato, e il compito 4 non
c'era piu'. Due righe si erano mescolate, e la coda le ha saltate.

Ora le scritture di una coda o di un diario passano una alla volta, con la
riga intera in una scrittura sola; e affidare un compito legge il numero e
scrive la riga sotto lo stesso turno, perche' due compiti affidati insieme
non prendano lo stesso numero. Una prova lo fa con otto fili insieme.

La regola: un file in cui scrivono due fili si scrive uno alla volta, e una
riga si scrive in una volta sola; e una prova che passa su un sistema va
fatta girare anche sull'altro prima di crederci.

## La correzione delle righe mescolate, solo dove si era vista

Col D382 avevo corretto la coda dei Dot: due fili scrivevano nello stesso
file con `writeln!`, che sono due scritture, e su Windows le righe si
mescolavano. La regola che ne avevo tirato era «un file in cui scrivono due
fili si scrive uno alla volta», ma l'avevo applicata solo ai file dei Dot. Il
registro delle azioni e quello delle decisioni si scrivevano allo stesso modo,
e dal D382 ci scrivono Nova e i Dot insieme. Me ne sono accorto scrivendo la
riga delle scelte del ricercatore in `decisioni.jsonl`.

Ora i due registri passano da `nova_core::righe`, uno alla volta e con la
riga intera in una scrittura sola, e una prova ci scrive con otto fili.

La regola: quando si corregge un modo di fare, si cercano tutti i posti che
fanno lo stesso, non solo quello in cui si e' visto il guasto.

## Il D382 prometteva un Dot che non chiede il permesso, ed era vero a meta'

Il D382 dice che un Dot non chiede il permesso, nemmeno con «conferma
sempre» nel pannello, e la prova lo mostra con un cervello in HTTP. Ma
Claude Code non passa da `EsecutoreDemone`: usa gli strumenti di NOVA via
MCP, e quando un'azione vuole il consenso lo chiede allo sportello di Nova,
se l'autonomia del pannello non e' piena. Li' un Dot chiede il permesso come
Nova. E il prompt di Claude nominava il vault di Nova, non quello del Dot.
Me ne sono accorto scrivendo il controllo delle fonti, chiedendomi cosa vede
NOVA di quello che legge Claude.

Il vault nominato a Claude ora e' quello del Dot, e una prova lo guarda con
un Claude finto. Il resto (lo sportello, la memoria via MCP, quello che
Claude legge) e' scritto in `docs/dots.md` fra le cose aperte, perche' ogni
strada tocca una guardia e va decisa con Gio.

La regola: una promessa sul comportamento vale per ogni cervello della
scala, e la prova deve dire per quali e' stata guardata.

## La mappa dei dati non nominava i Dot

`dots/` e' nato col D382, con le conversazioni dei Dot dentro, e la mappa che
risponde a «dove stanno i miei dati» (`nova/dati.py`) non lo diceva. La prova
della mappa guarda i percorsi costruiti dal Python, e `dots/` lo costruisce
il demone in Rust: per questo non l'ha visto. Ora c'e', marcato delicato,
con le altre voci scritte a mano per lo stesso motivo.

La regola: una cartella nuova accanto a `config.json` entra nella mappa nello
stesso commit, anche quando la scrive il Rust.

## `cargo fmt --all` su un repository che non e' formattato

Per formattare i file nuovi del ricercatore ho lanciato `cargo fmt --all`, e
ha riscritto 142 file: il codice di NOVA non segue il formato di rustfmt, e
in CI il controllo del formato non ferma niente (`continue-on-error`). L'ho
visto da `git status` prima del commit. Ho rimesso i file com'erano e
riapplicato a mano solo le mie modifiche.

La regola: in NOVA si formattano solo i file nuovi, uno per uno, e dopo ogni
comando che scrive file si guarda `git status`.

