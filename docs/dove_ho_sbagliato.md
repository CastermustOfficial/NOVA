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
