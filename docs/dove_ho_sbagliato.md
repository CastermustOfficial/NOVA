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

---

## Le forme che si ripetono

Rileggendole di fila, sono quasi tutte una di queste quattro:

1. **Ho ricordato invece di misurare.** Il numero quattordici, `0x70`, la data
   di un banco, l'aspettativa di una funzione. Ogni volta la cosa ricordata
   sembrava esattamente sicura quanto una misurata.
2. **Ho dato per scontata una premessa.** Che il fuoco fosse dove me l'ero
   messo. Che gli argomenti fossero nella forma che credevo.
3. **Ho scritto una prova che non poteva fallire.** Perche' il caso non
   capitava su questa macchina, o perche' chiedeva una cosa piu' debole di
   quella che serviva — e tre volte su tre la domanda che l'ha smascherata e'
   la stessa: *cosa succederebbe se il codice fosse rotto nel modo peggiore?*
4. **Ho letto un sintomo come una causa.** «Non arriva niente» sembrava un
   difetto del codice ed era una condizione della macchina; «il filtro non
   filtra» sembrava il demone ed ero io; e due volte gli accenti sembravano
   rotti nel dato mentre erano rotti nel modo in cui li leggevo.

La terza e' la piu' pericolosa, perche' le altre tre le trova qualcun altro —
una prova, un compilatore, un errore. Una prova che non prova niente non la
trova nessuno: passa.
