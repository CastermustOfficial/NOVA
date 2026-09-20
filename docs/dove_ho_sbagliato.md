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

Rileggendole di fila, sono quasi tutte una di queste quattro:

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
