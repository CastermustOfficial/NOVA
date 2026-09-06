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


## 9 settembre 2026

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
