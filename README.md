# NOVA

***Italiano** · [English](README.en.md)*

**Un esperto seduto accanto a te, dentro il tuo PC.**

NOVA non e' una chat che da' consigli: apre programmi, compila moduli, scrive
file, esegue comandi. E lo fa **senza rubarti il posto** — lavora in una
finestra sua, agendo sull'albero di accessibilita' invece che su mouse e
tastiera, cosi' puoi continuare a lavorare mentre lei fa il suo pezzo.

[![ci](https://github.com/CastermustOfficial/NOVA/actions/workflows/ci.yml/badge.svg)](https://github.com/CastermustOfficial/NOVA/actions/workflows/ci.yml)
[![licenza: MIT](https://img.shields.io/badge/licenza-MIT-blue.svg)](LICENSE)

> **Stato: alpha.** Funziona sulla macchina di chi la sviluppa. Se la provi,
> aspettati spigoli — e aprine una issue, che e' il modo piu' utile di aiutare.

## I primi cinque minuti

Questo documento e' lungo. Se sei arrivato qui per capire se NOVA ti serve,
questa e' la parte corta.

**Installi** (Windows, tre minuti; Python 3.10+ facoltativo):

```powershell
git clone https://github.com/CastermustOfficial/NOVA.git
cd NOVA
.\install.ps1
```

> **Windows dira' che non conosce l'editore.** I binari di NOVA non sono
> firmati con un certificato: una firma costa qualche centinaio di euro
> l'anno e, per un editore nuovo, **non toglie comunque** l'avviso finche'
> non ha una reputazione. Quindi la scelta e' dichiarata invece che
> nascosta. Cosa aspettarsi: SmartScreen puo' dire «Windows ha protetto il
> PC» al primo avvio — «Ulteriori informazioni» > «Esegui comunque»; e
> l'antivirus puo' mettere in quarantena un eseguibile appena scaricato,
> nel qual caso l'installer te lo dice e ti dice dove ripristinarlo.
> L'installer confronta le impronte SHA256 pubblicate con la release prima
> di installare qualunque cosa: e' un controllo che l'avviso di Windows non
> fa, ed e' quello che dice se il file e' davvero il nostro.

**Compare un orb** in un angolo dello schermo. Cliccalo, oppure chiamala per
nome. La prima volta ti propone tre cose da provare — sono queste. Nessuna
tocca un tuo file: le prime due leggono e basta, la terza si scrive un
appunto in memoria, che e' il punto, e si cancella dicendole di dimenticarlo.

| Chiedile | E vedi che |
|---|---|
| «Perche' il PC va piano?» | legge memoria, processi e dischi davvero, invece di indovinare |
| «Cosa c'e' nella cartella Download?» | guarda una cartella tua e te la racconta |
| «Ricordati che lavoro meglio la mattina presto» | se lo scrive in memoria, e alla prossima accensione lo sa ancora |

La terza e' quella che vale la pena aspettare: chiudi tutto, riapri domani, e
chiedile quando lavori meglio.

**Due comandi che rispondono senza far partire niente**, utili prima di
fidarsi e utili se un giorno NOVA non parte:

```powershell
.\bin\novad --dati          # dove tiene le tue cose, quanto pesano, cosa succede se le cancelli
.\bin\novad --registro      # cosa ha fatto e non si puo' annullare
```

Se qualcosa non funziona, il resto del documento spiega perche'.


## Cosa sa fare

Un elenco di aggettivi non dice niente. Questi sono i numeri, contati dal
codice: **129 strumenti** per un cervello agentico come Claude Code, e **58**
per il modello che gira sul tuo PC e per le API, sempre gli stessi, perche'
tutti insieme non starebbero nel suo contesto; **38 formati** di file che sa
aprire e mostrare.

### Agisce sul sistema, e non ti ruba il posto

File, applicazioni, finestre, PowerShell, appunti, volume, notifiche. La
differenza che conta non e' cosa tocca ma **come**: NOVA agisce sull'albero di
accessibilita', non su mouse e tastiera. Puo' compilare un modulo in una
finestra in secondo piano mentre tu scrivi in un'altra, e nessuna finestra
salta in primo piano a rubarti il fuoco.

E' una regola scritta nel suo prompt, non un effetto collaterale: *lavora
dietro, non davanti*.

### Usa il browser come lo useresti tu, ma a blocchi

NOVA pilota **Edge o Chrome** parlando con loro in CDP — cerca prima Edge,
che su Windows c'e' sempre, poi Chrome. **Firefox e Safari no**, e non e' una
dimenticanza: parlano un altro protocollo, e far finta che vada ovunque
sarebbe una promessa che si rompe sul PC di qualcun altro. Se non hai ne' Edge
ne' Chrome, tutto il resto di NOVA funziona e i comandi del browser dicono che
non trovano un browser da pilotare.

Non simula le battute sui tasti: incolla. Riempire cinque campi di un foglio di calcolo online costa **una**
chiamata invece di cinque, e leggere una tabella intera ne costa una sola.

| Operazione | Misurato |
|---|---|
| `web_incolla` — cinque righe in tre colonne | 2,5 ms |
| `web_tabella` — una tabella 5x4 letta tutta | 1,4 ms |
| `rete_cerca` — cercare senza aprire il browser | 0,46 s |

Misurati sul demone il 29 settembre con `misure/banco_web_demone.py`, andata
e ritorno sul canale locale: la mediana di venti chiamate per le prime due, e
di cinque ricerche vere per l'ultima. La piu' lenta delle cinque ha preso
3 s: la prima deve anche accendere il browser delle ricerche. Nella versione
Python erano 35 ms, 33 ms e circa 0,9 s.

L'ultima riga e' quella che cambia il carattere dell'assistente: **prima di
aprire una pagina, NOVA cerca**. Un browser che si apre e' una finestra che
compare sul tuo schermo; una ricerca che passa da un browser senza volto non
lo e'. Il demone cerca come la versione Python: con un browser senza
finestra, su una porta e un profilo suoi, che cerca su Bing. Se non ci sono
ne' Edge ne' Chrome prova DuckDuckGo con una richiesta semplice, che pero'
oggi risponde con pagine senza risultati. Fino al 29 settembre il demone
aveva solo questa seconda strada, e su cinque ricerche ne tornavano vuote
cinque (D362, D363).

### Ricorda, e quello che impara resta tuo

Una memoria a grafo su file `.md` — apribile in Obsidian, versionabile in git,
leggibile senza NOVA. Impara i fatti durevoli dopo ogni scambio, e **le
procedure**: come ha risolto una richiesta, per non doverla ricercare la volta
dopo. Le procedure si ritrovano anche quando la richiesta e' scritta in modo
diverso o con un refuso, perche' il confronto passa da tri-grammi di caratteri
e non da un'uguaglianza di stringhe.

Sul disco di chi scrive queste righe, il 28 settembre: 143 note e 31 procedure
imparate.

### Custodisce le credenziali senza farle vedere al modello

Archivio cifrato con DPAPI. NOVA puo' compilare un accesso senza che la
password passi mai dal modello: nel prompt entra un riferimento, nel campo
entra il valore. E' l'unico modo per cui «l'assistente conosce le mie
password» possa essere una frase accettabile.

### Fa da sola quello che deve ripetere

Automazioni scritte da lei, procedure imparate, attivita' pianificate
(«ogni giorno alle 8»), sentinelle che avvisano solo quando un valore cambia.
E un **registro delle azioni irreversibili**: cio' che non si annulla, si
annota. Il registro non scrive mai il valore di una credenziale.

### Vede

Legge lo schermo quando serve — ma prima prova a leggere il sistema. Uno
screenshot e' un accessorio, non il modo normale di sapere cosa c'e' su una
finestra: l'albero di accessibilita' e' piu' preciso, piu' veloce e non
dipende da cosa e' visibile.

---

## Cosa sa fare NOVA? Alcuni casi d'uso

Ogni voce ha un marcatore, perche' «sa fare» e' una parola che si allunga
troppo facilmente:

- **c'e'** — funziona con gli strumenti che ci sono adesso;
- **si scrive** — NOVA se lo costruisce al momento, con uno script o
  un'automazione che poi resta;
- **manca** — non c'e', e qui sotto c'e' scritto cosa manca. Un elenco che
  nomina solo cio' che funziona e' un elenco di cui non ci si fida la seconda
  volta.

### Burocrazia e scadenze

- **c'e'** — Compilare un modulo online lungo prendendo i dati dal fascicolo:
  rimborsi, iscrizioni, moduli della scuola, garanzie, disdette.
- **c'e'** — Tenere d'occhio una scadenza e avvisare *prima*: bollo,
  assicurazione, revisione, passaporto, rinnovo di un dominio.
- **si scrive** — Raccogliere i documenti sparsi per una pratica in una
  cartella sola, rinominati in modo coerente.
- **manca** — Tutto cio' che passa da SPID o CIE. Non e' un limite tecnico da
  aggirare: l'autenticazione forte la deve fare la persona, ed e' giusto cosi'.

### Soldi di casa

- **si scrive** — Estratti conto in PDF che diventano un foglio di calcolo:
  «dove sono andati i soldi questo mese».
- **c'e'** — Sorvegliare un prezzo e avvisare **solo quando scende**.
- **si scrive** — Fatture e scontrini: raccolti, rinominati per data e
  fornitore, sommati.
- **c'e'** — Confrontare due offerte - luce, gas, telefono - leggendo le
  pagine e mettendole in tabella.

### Documenti e lettere

- **c'e'** — Scrivere una lettera formale con i dati veri: disdetta, reclamo,
  richiesta di rimborso, ricorso a una multa.
- **c'e'** — Rileggere e correggere un documento con le proposte dentro il
  testo. Su un `.docx` senza perdere l'impaginazione.
- **si scrive** — Unire piu' PDF, estrarne pagine, convertirli.
- **manca** — La firma digitale dentro l'harness.
- **manca** — Le presentazioni: nessuno strumento produce `.pptx`.

### Fogli e dati

- **si scrive** — Ripulire un foglio disordinato: doppioni, colonne fuori
  posto, date scritte in tre modi diversi.
- **si scrive** — Da PDF a tabella, per listini ed estratti.
- **manca** — Da PDF **scansionato** a tabella: senza riconoscimento ottico
  quel PDF resta un'immagine, e NOVA lo dice invece di inventarsi i numeri.
- **c'e'** — Portare una tabella da un gestionale a un altro che non ha API.

### Il PC

- **c'e'** — «Perche' e' lento?», guardando lo stato vero.
- **si scrive** — Fare spazio: i file enormi, e i duplicati veri - stesso
  contenuto, non stesso nome.
- **c'e'** — Backup di una cartella su un disco esterno, ripetuto ogni
  settimana.
- **si scrive** — Mettere in ordine foto e scaricati: per data, per tipo, per
  evento.
- **manca** — «Credo di avere un virus». NOVA puo' guardare processi, avvii
  automatici e connessioni, e dire cosa vede; **non e' un antivirus** e non
  deve comportarsi come se lo fosse.

### Posta e persone

- **c'e'** — Triage della posta: cosa chiede una risposta, cosa puo' aspettare.
- **c'e'** — Preparare la risposta e mandarla **solo dopo conferma**.
- **si scrive** — Il richiamo: «se fra cinque giorni non rispondono,
  ricordamelo».

### Studio

- **c'e'** — Studiare su una pila di PDF con citazioni che si possono
  controllare: file e pagina.
- **c'e'** — Riassumere un documento lungo mostrando da dove viene ogni pezzo.
- **si scrive** — Preparare domande di ripasso dal materiale.

### Chi il PC fa fatica a usarlo

Questo non fa risparmiare mezz'ora: cambia chi puo' usare un computer.

- **c'e'** — Usarlo **a voce**, chiamandola per nome. «Nova, scrivi a mio
  figlio.» «Nova, cerca la ricetta del pane.»
- **c'e'** — Aiutare un genitore a distanza. La differenza con un programma di
  controllo remoto e' che NOVA **non prende il mouse**: agisce sull'albero di
  accessibilita', quindi chi sta davanti a quel computer continua a usarlo
  mentre lei fa la sua parte.

### Vendere e comprare

- **c'e'** — Scrivere l'annuncio e caricare le foto.
- **c'e'** — Cercare un usato su piu' portali e mettere i risultati in una
  tabella.

---

## Gli stessi casi, visti da dentro

Un elenco di strumenti non dice cosa succede quando si mettono in fila. Questo
si': ogni caso qui sotto e' una richiesta sola che diventa una catena, e sotto
ognuno c'e' scritta la catena vera, con i nomi degli strumenti che la fanno.

Gli esempi non sono immaginati: le famiglie vengono dall'archivio delle
procedure di una macchina in uso. Ventotto voci, e la meta' e' una sola cosa
fatta dall'inizio alla fine.

### Cercare lavoro, e candidarsi

E' il caso che ha spinto piu' funzioni di ogni altro, perche' e' lungo e noioso
esattamente dove un assistente serve:

> «Cerca offerte per AI engineer, guarda quali hanno senso per me, e candidati.»

NOVA cerca sui portali, apre gli annunci, legge il tuo **fascicolo** — CV,
esperienze, testi che hai scritto tu — e da li' prende i fatti. Compila il
modulo, comprese le tendine e i campi React che non si lasciano riempire da
soli, manda, e poi controlla nella posta che la conferma sia arrivata.

Due cose vanno dette, e sono nel prompt di NOVA non nella buona volonta':
**quello che non c'e' nel fascicolo si chiede, non si deduce** — un'esperienza
inventata non e' un errore, e' una dichiarazione falsa con sopra la tua firma
— e ogni invio e' un'azione che non si annulla, quindi finisce nel registro.

### Riempire fogli e moduli con molti dati

> «Preparami un foglio Google con questi quarantatre giocatori, divisi per
> ruolo.»

Fatto per davvero. La differenza fra NOVA e una macro e' che non batte i tasti:
apre il foglio, cerca i ruoli dove stanno scritti, e **incolla a blocchi** —
cinque valori in tre campi in 35 millisecondi. Molti dati non si mettono uno
per volta.

### Studiare una pila di documenti

> «In quale di questi sei PDF si parla di entropia, e a che pagina?»

L'harness apre la cartella, cerca in tutti i file insieme e risponde con file
e pagina, poi ci scende sopra e la evidenzia. Serve una citazione che si possa
controllare, non un riassunto di cui fidarsi.

### Scrivere e correggere un documento

> «Rileggi questa relazione e proponi le correzioni.»

Le proposte compaiono **dentro il testo**, colorate. Le correggi dove le leggi
e le applichi quando vuoi tu. Su un `.docx` cambia il paragrafo e lascia
intatta l'impaginazione.

### La posta, e le cose di tutti i giorni

Controllare la posta, salvare un contatto, preparare una bozza e mandarla dopo
conferma, aprire un documento condiviso, verificare che un sito sia online.
Sono le richieste che si ripetono, ed e' li' che l'archivio delle procedure
paga: la seconda volta non si ricomincia da capo.

### Cose che si ripetono da sole

- **Attivita' pianificate**: «ogni giorno alle 8, guarda se ci sono offerte
  nuove».
- **Sentinelle**: avvisano solo quando un valore **cambia**, non a ogni giro.
  Un promemoria che parla tutti i giorni si spegne dopo una settimana.
- **Automazioni scritte da lei**: quando una procedura si ripete abbastanza,
  NOVA la trasforma in uno strumento e smette di rifarla a mano.

### Spostare dati fra due sistemi che non si parlano

> «Prendi la tabella da questo gestionale e mettila nel foglio dell'altro.»

E' il lavoro che esiste perche' *non c'e' un'API*, e che di solito si fa a
mano per un'ora. `web_tabella` legge una tabella intera in una chiamata sola,
gia' come TSV; `web_incolla` la rimette dall'altra parte a blocchi;
`web_carica` consegna un file a un campo di caricamento senza aprire nessuna
finestra di dialogo. Nessun tasto premuto, nessuna finestra che salta davanti.

**La catena:** `web_apri` -> `web_tabella` -> `web_incolla` / `web_carica`

### Una ricerca con fonti che si possono controllare

> «Fammi il punto sullo stato dell'arte dei modelli aperti, con le fonti.»

`rete_cerca` trova senza aprire il browser, `rete_leggi` scarica una pagina
come testo in mezzo secondo invece di sei, e i documenti che hai gia' sul
disco entrano nell'harness. La differenza rispetto a farsi riassumere le cose
da una chat e' che la risposta dice **dove**: file e pagina, non «mi risulta
che».

**La catena:** `rete_cerca` -> `rete_leggi` -> `harness_apri` ->
`harness_cerca_progetto` -> `harness_proponi` (il testo nasce nel documento)

### Sorvegliare qualcosa e parlare solo se cambia

> «Guarda ogni mattina se escono offerte nuove e dimmelo solo se ce ne sono.»

Una sentinella non e' un promemoria: confronta il valore di oggi con quello di
ieri e tace se e' uguale. Un avviso che arriva tutti i giorni si spegne dopo
una settimana; uno che arriva quando qualcosa e' cambiato si legge.

**La catena:** `pianifica_crea` (sentinella) -> ... -> `avvisi_recenti`
quando torni

### Accedere a un servizio senza che la password passi dal modello

> «Entra nel portale e scarica le fatture del mese.»

Le credenziali stanno in un archivio cifrato con DPAPI. Nel prompt entra un
riferimento, nel campo entra il valore: il modello non vede mai la password,
e nemmeno il registro delle azioni la scrive. E' l'unico modo per cui
«l'assistente conosce le mie password» possa essere una frase accettabile.

**La catena:** archivio credenziali -> `web_scrivi` -> `azione_registra`

### Chiedere un secondo parere a un modello piu' capace

> «Questa cosa e' delicata: falla guardare a qualcuno piu' bravo.»

NOVA non e' un modello solo. Quello di casa orchestra - e' veloce e non costa
niente - e quando il compito lo merita **delega**: un ragionamento difficile,
del codice delicato, una decisione che pesa. Chi riceve il compito non vede la
conversazione, quindi NOVA glielo riscrive per intero.

**La catena:** `cervelli_stato` (chi c'e') -> `cervelli_delega` -> la risposta torna dentro
la stessa conversazione

### Capire perche' il PC va piano

> «Perche' e' lento?»

Legge lo stato vero invece di indovinare: memoria, processi, dischi, quanti
layer del modello stanno davvero in VRAM. Su Windows, quando la VRAM finisce,
il driver ripiega in silenzio sulla RAM condivisa e il modello va dieci volte
piu' piano senza dire niente - NOVA lo vede e lo dice.

**La catena:** `sys_info` -> `app_processi` -> `shell_exec`

### Smettere di rifare a mano una cosa gia' fatta tre volte

> «Questa e' la terza volta: fattela da sola.»

Quando una procedura si ripete abbastanza, NOVA la trasforma in uno strumento
suo e da quel momento non la ricostruisce piu' un passo per volta. Il
guadagno non e' teorico: una richiesta risolta da un'automazione costa due
giri di modello invece di dieci.

**La catena:** ricette (la strada imparata) -> `automazione_crea` ->
`automazioni_elenco`

### E anche il suo stesso codice

Nell'archivio c'e' «git tag e push». NOVA lavora sul progetto che la contiene:
apre i propri sorgenti nell'harness, li legge con i colori, propone modifiche
e le applica quando glielo dici. Il **banco** (`ripara_apri`, D349) le
permette di provare una riparazione su una copia, con `cargo test`, prima di
toccare l'originale: si applica solo se nessuna prova verde diventa rossa.

---

### Quello che tutti questi casi hanno in comune

Tre cose, e sono le stesse tre ovunque:

**Se una strada non cede, ne prova un'altra.** E se la strada giusta non
esiste, se la costruisce - un'automazione, uno script, un giro diverso. E'
scritto nel prompt come principio, non come suggerimento.

**Lavora dietro, non davanti.** Nessuna finestra che salta in primo piano,
nessun tasto premuto al posto tuo, nessuna console nera che compare. Puoi
continuare a lavorare mentre lo fa.

**Cio' che non si annulla, si annota.** Una candidatura mandata, una mail
partita, un file cancellato: NOVA non chiede il permesso ogni volta - lo
chiede secondo il livello di autonomia che hai scelto - ma quello che ha fatto
e non si puo' disfare resta scritto, e lo puoi rileggere.

---

## Le ricette: come fa a non rifare due volte la stessa fatica

Quando NOVA risolve qualcosa di non banale, non tiene solo il risultato: tiene
**la strada**. Titolo, passi, e le parole con cui gliel'avevi chiesta. La
volta dopo, prima di ricominciare, guarda se una di quelle strade somiglia
alla richiesta nuova.

Il problema vero e' «somiglia». Un confronto fra stringhe non serve a niente:
nessuno chiede due volte la stessa cosa con le stesse parole, e chi scrive di
fretta scrive *inobx*. La soluzione presa in prestito dai lavori sugli
**engram** — la memoria a n-grammi di DeepSeek e di Qwen — e' che il recupero
deve essere **economico**, e la scelta finale la fa il modello:

- **Le parole rare pesano di piu'.** Una parola che sta in ogni procedura non
  distingue niente; il peso e' `1 + N/(1+n)`, cioe' una rarita' senza
  logaritmo. «Posta» vale poco se hai dieci procedure sulla posta; «fantacalcio»
  vale molto.
- **Si misura quanto della domanda e' coperto**, non quanto le due frasi si
  somigliano. Una procedura ricca di dettagli non deve perdere contro una
  povera solo perche' ha piu' parole: e' un **contenimento asimmetrico**, non
  un coseno.
- **Le parole si confrontano a tri-grammi.** «inobx» e «inbox» condividono
  quasi tutti i pezzi da tre lettere, quindi valgono l'una per l'altra. Con
  due guardie, imparate sbagliando: stessa lettera iniziale, e lunghezze che
  non differiscono di piu' di uno — senza, «ricetta» somigliava a «letta».
- **Si pesca largo.** La soglia e' 0,30 e non 0,42, perche' una candidata di
  troppo costa qualche centinaio di token, una mancata costa i dieci turni che
  ci vogliono a rifare la strada da capo. Il blocco delle ricette entra nel
  prompt come **appunto, non come ordine**: il modello e' autorizzato a
  scartarlo.
- **Ci sono anche gli alias**: gli altri modi di chiedere la stessa cosa, che
  il modello elenca quando la procedura nasce. Contano quasi quanto le parole
  vere — quasi, perche' sono l'ipotesi di qualcun altro su come parlerai.

Non e' memoria neurale e non pretende di esserlo: e' uno strato di recupero
lessicale che costa microsecondi. L'idea presa dagli engram non e'
l'architettura, e' la divisione dei compiti — **cercare deve costare poco,
decidere tocca a chi ha il contesto.**

L'archivio si tiene pulito da solo: massimo sessanta voci, i doppioni si
fondono, le meno usate cadono. Un archivio che cresce all'infinito diventa
rumore, e il rumore fa proporre la strada sbagliata.

---

## L'harness: dove si studia e dove si scrive

E' la parte piu' recente e la meno ovvia. Un documento o un progetto non sono
un messaggio in chat: durano piu' di un turno, e vanno guardati mentre se ne
parla. L'harness e' una finestra con il documento a sinistra, l'albero dei
file quando c'e' un progetto, e la conversazione a destra — **la stessa
conversazione** del resto di NOVA, non una seconda.

### Documenti

| Formato | Come si apre |
|---|---|
| `.pdf` | le **pagine vere**, disegnate, col testo selezionabile e le note gialle |
| `.docx` | paragrafo per paragrafo, e si scrive: il resto del file resta com'era |
| `.md` `.txt` | nell'editor, con l'anteprima accanto |
| `.html` | **reso**: e' un artifact, si guarda per quello che fa |

Chiedere «dove si parla di entropia» non torna una frase: torna una
**posizione** — file e pagina — e il documento ci scende sopra e la evidenzia.
Con una cartella aperta come progetto la ricerca vale su tutta la pila, che e'
la domanda vera quando i documenti sono sei PDF di un esame: non «dove sta in
questo file» ma «in quale file sta».

### Codice

Trentadue estensioni, dal Python al Rust al Vue. Il codice si apre su fondo
scuro, nello stesso editor di Visual Studio Code, con i numeri di riga,
perche' un errore si dice cosi': file e riga. Sotto c'e' un terminale vero.
Un `.html` mostra il risultato, e il sorgente e' a un click: si cambia, si
salva, e la pagina si ridisegna.

### E NOVA scrive dentro, ma non di nascosto

Questa e' la parte che vale la pena spiegare bene, perche' e' una scelta e non
una limitazione.

**Non esiste una funzione che modifichi un documento.** Esiste una proposta.
Compare **dentro il testo**, al posto suo, con addosso il colore: quello che
arriva su fondo brace, quello che se ne va in grigio sbarrato. La si puo'
correggere dove la si legge — e quello che si applica e' quello che si e'
visto, anche se nel frattempo lo si e' cambiato. Il bottone lo premi tu, e
prima di sovrascrivere resta una copia intatta accanto.

Piu' il modello e' debole, piu' questo ciclo vale: un modello forte che scrive
diretto e' accettabile, un modello debole che scrive diretto e' ingestibile,
un modello debole che **propone** e' utilizzabile.

Sui formati non si promette quello che non si sa mantenere:

- **`.md`, `.txt`, codice**: si riscrivono per intero, nessuna conversione in
  mezzo. Gli asterischi di un commento Python non diventano corsivo.
- **`.docx`**: si modifica **un paragrafo alla volta**, e grassetti, corpo,
  stile e impaginazione restano quelli di chi lo ha scritto. Rifare il file
  dal testo estratto sarebbe stato molto piu' facile, e avrebbe buttato via il
  lavoro dell'utente.
- **`.pdf`**: il testo **non si riscrive**, e NOVA lo dice. Un PDF non
  contiene paragrafi ma lettere messe in un punto della pagina. Si evidenzia e
  si annota per davvero — annotazioni che restano nel file e si aprono in
  qualunque lettore.

### E sul codice, prova prima di applicare

Il verificatore e' la parte che trasforma l'harness da un buon posto per
leggere a un posto dove si programma. `harness_prova` riconosce da solo come
si prova un progetto — `cargo`, `npm`, `go`, pytest, oppure gli script
`test_*.py` — e sceglie la suite giusta per il file che si sta toccando:
provare tutto il Rust perche' e' cambiata una riga di Python e' tempo
buttato.

Con «Applica e prova» la modifica si scrive solo se i test non peggiorano. E
il confronto e' con **prima**, non con il verde assoluto: su un progetto vero
qualche prova rossa c'e' quasi sempre, e un verificatore che pretende il verde
non si accende mai. Quello che conta e' se cade qualcosa che prima passava —
in quel caso il file torna com'era, la proposta **resta** (un test rosso e'
una cosa da correggere, non un motivo per ricominciare da capo) e l'uscita dei
test torna a NOVA, che sa cosa aggiustare.

Un'uscita `2` vuol dire «qui non si puo' provare» — serve il demone, serve un
browser — e non conta come fallimento: contarla bloccherebbe ogni modifica.

## Installazione

### Requisiti

| | |
|---|---|
| Sistema | **Windows 10/11 a 64 bit** |
| Python | facoltativo, 3.10 o superiore: serve alle automazioni che NOVA si scrive |
| Disco | 3 GB per il minimo; 15-30 GB se scegli un modello locale |
| GPU | facoltativa: serve solo per il modello locale |

NOVA e' legata a Windows in profondita': l'automazione usa UI Automation e
l'archivio credenziali usa DPAPI. Su macOS e Linux il demone gira e fa quello
che non ha bisogno delle finestre, ma l'installatore c'e' solo per Windows:
la tabella in «Perche' Rust, e perche' un demone» dice cosa c'e' e cosa manca.
**Non serve ne' Rust ne' Visual Studio**: il core arriva gia' compilato.

### Passi

```powershell
git clone https://github.com/CastermustOfficial/NOVA.git
cd NOVA
.\install.ps1
```

| Opzione | Cosa fa |
|---|---|
| `.\install.ps1` | installa tutto e configura l'avvio automatico |
| `.\install.ps1 -ConCuda` | scarica anche llama.cpp CUDA, per il modello locale |
| `.\install.ps1 -DaSorgente` | compila il core invece di scaricarlo (serve Rust + MSVC) |
| `.\install.ps1 -SenzaAvvioAuto` | non parte all'accensione |
| `.\install.ps1 -Disinstalla` | toglie processi, attivita' pianificate, avvio automatico e collegamento, e dice cosa resta |
| `.\install.ps1 -Disinstalla -ConIDati` | e anche memoria, credenziali e configurazione. Il fascicolo e i file fuori dalla cartella di NOVA non si toccano mai: te li elenca |

Poi avvia NOVA dal collegamento sul Desktop: comparira' un orb in un angolo
dello schermo. Cliccalo per scrivere, oppure chiamala per nome.

## Il cervello: chi ragiona

NOVA non e' legata a un modello, e non pretende che tu scarichi il suo. Chi ne
ha gia' uno non ricomincia da capo: l'installer guarda prima cosa c'e' sulla
macchina, e solo dopo propone di scaricare.

| Strada | Per chi | Nota |
|---|---|---|
| **Chiave API** | qualita' massima, si paga a consumo | OpenAI, OpenRouter, Groq, qualunque endpoint compatibile |
| **Un abbonamento che hai gia'** | chi paga Claude, ChatGPT, Gemini o Qwen | l'installer cerca `claude`, `codex`, `gemini`, `qwen` nel PATH; vedi l'avvertenza sotto |
| **Un modello che hai gia'** | chiunque abbia un `.gguf` da qualche parte | l'installer lo cerca in LM Studio, Jan, GPT4All, koboldcpp, nella cache di HuggingFace, in Download e sul Desktop; oppure indichi il percorso |
| **Un server gia' acceso** | chi ha Ollama o LM Studio in funzione | rilevato sulle porte 11434, 1234, 8080, 5001; nessuna chiave richiesta |
| **Scarico io un modello** | chi parte da zero | Qwen3.8 27B, con la quantizzazione che sta nella tua VRAM - ma puoi sceglierne un'altra, e decidere su quale disco finisce |

Nessuna di queste e' obbligatoria all'installazione: si puo' rispondere
«decido dopo» e cambiare idea dal menu **Cervello**, o da `brains.active` in
`config.json`. Le CLI riconosciute le elenca `.\bin\nova cli-predefinite`:
aggiungerne una non richiede codice, solo una voce sotto `brains.cli`, o il
pannello.

Il modello indicato a mano viene controllato davvero: i primi quattro byte di
un GGUF sono `GGUF`, e uno scaricamento interrotto non li ha. Se accanto al
file c'e' un proiettore `mmproj`, NOVA lo usa e il modello ci vede; se non
c'e', l'installer te lo dice invece di lasciartelo scoprire fra un mese.

Cambiare strada dopo non richiede di reinstallare niente: e' il menu
**Cervello** nell'interfaccia, oppure `brains.active` in `config.json`.

> **Avvertenza sugli abbonamenti.** Usare la CLI di un abbonamento consumer
> come motore di un'applicazione terza e' fuori dai termini di servizio della
> maggior parte dei fornitori, e il rischio ricade sul tuo account. NOVA
> supporta questa strada perche' e' comoda, ma non e' quella predefinita e non
> te la consiglia.

Il catalogo dei modelli locali sta in [`models.json`](models.json): e' un
dato, non codice, cosi' aggiornare la classifica non richiede una release.

## Permessi

NOVA parte con **«conferma sempre»**: chiede il permesso prima di ogni azione
che tocca il sistema, e la richiesta dice *cosa* sta per fare, non un generico
«consentire operazione?». Puoi allentare il vincolo quando ti fidi — e'
una manopola tua, non una decisione sua.

Quello che resta sul tuo disco e non esce mai: memoria, credenziali,
configurazione. Vivono in `%APPDATA%\NOVA` — ma «vivono in una cartella» non
e' una risposta, quindi c'e' un comando che la da' per intero:

```powershell
.\bin\novad --dati                    # cosa c'e', dove, quanto pesa, e cosa succede se lo cancelli
.\bin\novad --registro                # cosa NOVA ha fatto e non si puo' annullare
.\bin\novad --registro fattura        # le azioni che nominano una fattura
.\bin\novad --registro --giorni 7     # quelle dell'ultima settimana
```

Tutti e due leggono il disco e basta: niente configurazione, niente cervello
acceso. Il momento in cui serve saperlo e' spesso quello **prima** di
fidarsi abbastanza da far partire il resto — o quello in cui NOVA non parte
piu'. E il valore di una credenziale non compare in nessuno dei due: ne
compare il nome, e il fatto che l'archivio esiste.

## Documentazione

- [Documento di architettura](docs/architettura.md) — le decisioni prese e il
  perche', comprese quelle scartate.
- [Diario di lavoro](docs/diario.md) — cosa si e' scoperto **mentre** si
  faceva. Il registro delle modifiche e' `git log`; questo tiene le cose che
  un messaggio di commit non dice, e che dopo sei mesi non si ricostruiscono.
- [Verso la beta](docs/verso_la_beta.md) — cosa manca, in tre liste, e le
  cinque frasi che devono essere vere per togliere la parola «alpha».
- [Dove ho sbagliato](docs/dove_ho_sbagliato.md) — gli errori di chi scrive il
  codice, tenuti apposta per essere riletti. Non i bug del progetto: le volte
  in cui ho creduto una cosa falsa, e come me ne sono accorto.
- [Come contribuire](CONTRIBUTING.md)

Nella radice ci sono cinque cartelle che fanno da indice, le stesse in ogni
progetto: [`errori/`](errori/README.md) (gli sbagli, e la regola che ne
viene), [`piano/`](piano/README.md) (cosa resta da fare, in ordine),
[`idea/`](idea/README.md) (le migliorie possibili, anche solo teoriche),
[`test/`](test/README.md) (ogni prova, una per una) e
[`analisi/`](analisi/README.md) (le decisioni confermate).

## Per chi sviluppa

```powershell
.\build.ps1              # compila il core Rust (release) e lo pubblica in bin\
.\build.ps1 -Test        # esegue i test Rust
foreach ($f in Get-ChildItem -Path prove -Recurse -Filter "test_*.py") { python $f.FullName }
```

Le prove Python sono **programmi**, non casi di `pytest`: ognuna si lancia da
sola, stampa cosa controlla e finisce con un codice di uscita — 0 passata,
1 rossa, **2 «qui non si puo' fare»**. Stanno in `prove/`, divise per cosa
serve a farle girare: [`prove/README.md`](prove/README.md) lo spiega.

Il resto di questo documento e' la documentazione tecnica di dettaglio.

---

## Perche' Rust, e perche' un demone

La domanda giusta non e' «perche' Rust» ma **perche' un processo che vive nel
sistema invece di un'applicazione che apri**. NOVA deve poter parlare mentre
non e' aperta, sorvegliare llama-server, tenere il registro delle capacita' e
sopravvivere alla chiusura di qualunque finestra. Le interfacce — l'orb,
l'harness, la CLI, la voce, un cervello agentico — sono client sottili:
possono morire e ripartire senza fermare NOVA.

Rust viene dopo, ed e' scelto per tre cose concrete:

**Perche' il demone non puo' cadere.** E' l'unico processo che deve stare in
piedi sempre. Un errore di memoria in un servizio che possiede i processi
lunghi non e' un messaggio d'errore, e' un assistente che si spegne mentre
lavori.

**Perche' le capacita' che servono sono la stessa cosa con tre nomi.** Il
demone e' costruito come *un trait, tre backend*: la stessa domanda — «che
finestre ci sono?», «metti questo nel cestino» — con una risposta per ogni
sistema. Oggi il backend completo e' uno solo, quello di Windows, e la tabella
dice cosa c'e' davvero:

| Serve per | Windows | Linux | macOS |
|---|---|---|---|
| Controllare qualsiasi app | UI Automation | **manca** (AT-SPI2) | **manca** (Accessibility API) |
| Chiudere un comando in un recinto | contenitore (AppContainer) | Landlock | **manca** |
| Annullare cio' che si e' fatto | giornale + Cestino | giornale + cestino freedesktop | giornale + `~/.Trash` |
| Custodire le credenziali | DPAPI | **manca** | **manca** |
| Canale locale | named pipe | socket unix | socket unix |

Su Linux e macOS il demone compila, gira, e fa quello che non ha bisogno
delle finestre: file, memoria, cervelli, web. Le prove del core girano anche
li', a ogni push, in CI. Osservare tutto il sistema (ETW, eBPF) e le istantanee del
disco (VSS, APFS) erano scritti qui come piano: codice non ce n'e', e finche'
non c'e' non stanno in tabella.

**Perche' un binario e' un binario.** Il demone si scarica compilato: chi
installa NOVA non ha bisogno ne' di Rust ne' di Visual Studio.

**E Python?** NOVA e' nata in Python, e per mesi le due meta' hanno convissuto:
il demone in Rust per i processi lunghi, il ciclo dell'agente, gli strumenti e
la memoria in Python. Da settembre 2026 fa tutto il Rust — le domande, gli
strumenti, la memoria, i cervelli, l'installatore — e NOVA funziona senza
Python. Ogni pezzo e' stato portato con un banco gemello che confronta le
due versioni sugli stessi casi, e `nova/` resta per questo: e' il termine di
paragone delle prove, e chi vuole puo' ancora usarla dal terminale con
`python -m nova`. Python sul PC serve a una cosa sola: e' la lingua in cui
NOVA scrive le sue automazioni. Il percorso e' raccontato in
[`docs/verso_la_beta.md`](docs/verso_la_beta.md), e la mappa della versione
Python sta in [`nova/README.md`](nova/README.md).

## Architettura

```
bin/                  i binari: l'orb, il demone, la riga di comando, gli aiutanti
install.ps1           installazione, runtime CUDA, avvio automatico, collegamento
build.ps1             compila il core e lo pubblica in bin/
core/crates/
  novad/              il demone: bus, capacita', processi lunghi, RPC locale
  nova-shell/         l'orb e le finestre (Tauri): chat, impostazioni, harness
  nova-cli/           `nova`: parlare al demone, e configurare senza di lui; `novaw`, lo stesso senza finestra
  nova-core/          il motore: registro delle capacita', turno, permessi, giornale
  nova-proto/         JSON-RPC su named pipe o socket unix, della forma di MCP
  nova-mcp/           NOVA come server MCP per un cervello agentico
  nova-mcp-cliente/   NOVA come cliente dei server MCP di altri
  nova-platform/      il sistema: UI Automation, finestre, audio, cestino, GPU
  nova-ciclo/         il turno: chiedi, esegui, rileggi
  nova-contesto/      quanto della conversazione ci sta, e cosa si butta quando no
  nova-scala/         chi risponde a cosa: gradini, deleghe, ripieghi sulla quota
  nova-salita/        quando si sale di gradino, e quando si gira a vuoto
  nova-cervelli/      cosa NOVA dice a un cervello che vive fuori: CLI, Claude Code, API
  nova-strumenti/     gli strumenti e le guardie dell'autonomia
  nova-nodi/          la memoria: un nodo per file .md, e le regole per impararne
  nova-memoria/       BM25, embedding di casa, fusione, scelta
  nova-ricette/       le procedure imparate, ritrovate anche con un refuso
  nova-registro/      cercare e raccontare il registro delle azioni
  nova-potatura/      nessun diario cresce per sempre: due MB, uno storico
  nova-modelli/       trovare i GGUF e llama-server, e i conti sulla VRAM
  nova-catalogo/      quale modello ha senso su questa macchina
  nova-componenti/    cosa serve a ogni funzione, e come procurarlo
  nova-cartelle/      se una cartella la sincronizza qualcun altro col cloud
  nova-voce/          voce e ascolto in casa: Kokoro, whisper.cpp; ElevenLabs a scelta
  nova-browser/       il codice che gira dentro la pagina
  nova-cdp/           il protocollo per parlare a Edge e Chrome
  nova-harness/       documenti e progetti: cercare una posizione, proporre, provare
  nova-docx/          modificare un .docx senza rifarlo da capo
  nova-fogli/         i fogli di calcolo: celle, riferimenti, scrittura senza perdite
  nova-documenti/     leggere dentro pdf, docx, fogli
  nova-configurazione/ la configurazione: il file dell'utente sopra quella di fabbrica
  nova-dati/          dove NOVA tiene le tue cose, e cosa succede se le cancelli
  nova-guasti/        un guasto detto in italiano, e le chiavi che non escono
  nova-calendario/    i conti sulle date, con l'ora passata da fuori
  nova-pianificazione/ «ogni giorno alle 8», e l'attivita' di Windows che lo fa
  nova-pitone/        le abitudini di Python che il porto doveva rispettare
  nova-decisioni/     quali decisioni possono uscire dal PC (CANT-12)
  nova-giudizio/      decisioni tipizzate lette dai logit (CANT-12)
  nova-clm/           le due teste di CLM, sui vettori di Qwen3-8B (CANT-12)
nova/                 la prima versione, in Python: termine di paragone dei banchi
prove/                le prove, divise per cosa serve a farle girare
```

Trentanove crate. Gli ultimi tre sono il terreno di CANT-12.
`nova_core::giudizio_casa` (D371) chiede i logit al modello di casa e li passa
a `nova-giudizio`, e la prima decisione che lo usa e' la delega: quale
cervello serve (D373). `nova-decisioni` non lo usa nessun binario, e lo
stesso vale per `nova-mcp-cliente`.
Per addestrare le teste di CLM il demone tiene anche le sue decisioni in
`decisioni.jsonl` (D374): la richiesta, gli strumenti usati, il gradino, la
scelta del giudice. Una richiesta con dentro una credenziale non si scrive, e
`kb.decisioni` spegne tutto.
Le teste, addestrate su compiti sintetici etichettati dal modello grande
(`misure/clm_addestra.py`), fanno da 27 a 29 su 34 a dire quale cervello
serve e scelgono lo strumento giusto fra 58 da 27 a 32 volte su 40 (D375).
Il demone le usa (`nova-clm`, D378) per chi non ha le lettere, cioe' non
tiene un modello sul PC: si accendono con `clm.attivo`, e decidono quale
cervello serve sopra una soglia scelta su casi nuovi. Sulle stesse domande Claude Code con Opus 5 e Gemini 3.1
Pro da Antigravity fanno 34 su 34 e 40 su 40, in secondi invece che in
millisecondi (`misure/banco_cervelli_fuori.py`).

## Livelli di autonomia

Impostabili al volo dal menu in alto a destra (o in `config.json`):

| Livello | Comportamento |
|---|---|
| `always_ask` | conferma per **ogni** azione, anche le sole letture |
| `ask_risky` | conferma solo per le azioni `DANGEROUS` (shell, delete, chiusura app, tasti) |
| `autonomous` | nessuna conferma, tutto tracciato nel registro azioni |

Ogni capacita' e' classificata `safe` / `moderate` / `dangerous`. Oltre
all'autonomia valgono sempre tre guardie non aggirabili dal modello:

- `safety.protected_paths` - percorsi mai scrivibili (Windows, Program Files, ...)
- `safety.forbidden_command_patterns` - regex di comandi bloccati (format, diskpart, ...)
- `safety.write_roots` - se valorizzato, le scritture sono confinate a quelle cartelle

Il confine vale anche dopo che il comando e' partito. Su Linux lo tiene
Landlock; su Windows un contenitore del sistema (AppContainer) con un job
object, che vede le cartelle di `write_roots` in scrittura, gli strumenti
elencati in sola lettura, e del resto del profilo non legge niente (D367).
Se il demone gira da amministratore, i comandi nel recinto partono lo stesso
senza i suoi poteri (D369). Su Windows si configura anche in `core.json`,
accanto a `config.json`:

- `tool_roots` - le cartelle di strumenti (python, node, cargo) che i comandi
  possono leggere ed eseguire, mai scrivere. Si concede la cartella piu'
  stretta: `.cargo\bin` e non `.cargo`, dove stanno le credenziali
- `shell_senza_rete` - spegne la rete ai comandi; di default e' accesa, come
  su Linux

Quattro comandi si lanciano a mano: `novad --recinto --proponi` dice quali
cartelle del `PATH` il contenitore non legge; `--prepara` apre, con la
conferma di amministratore che chiede Windows, quelle che l'utente non puo'
aprire da solo (`C:\Users` per un progetto nel profilo, `C:\Python313`);
`--controlla` dice quali cartelle di terzi il contenitore puo' scrivere perche'
aperte a tutti i pacchetti di Windows, e non si possono chiudere solo per lui,
quindi il racconto di ogni comando le elenca con la data del controllo; e
`--togli` toglie ogni permesso scritto sulle cartelle e il profilo del
contenitore, anche alla disinstallazione.

## Runtime del modello

Il modello di casa lo accende il demone, quando serve: se il primo gradino
della scala e' il modello locale, prima di rispondere guarda se all'indirizzo
di `server.host`/`server.port` risponde qualcuno. Se si', usa quello — anche
LM Studio o Ollama. Se no, lo accende e aspetta che sia pronto (D358).

`llama-server` si cerca in quest'ordine, se `server.binary` non lo indica:

1. `NOVA\runtime\` (build CUDA scaricata da `get_cuda_runtime.ps1`)
2. i backend gia' presenti in `%USERPROFILE%\.lmstudio\extensions\backends`
3. `LLAMA_CPP_HOME` o `LLAMACPP_HOME`

Prima di accenderlo stima quanti layer entrano nella memoria video, e se il
modello non ci sta lo riaccende con sei layer in meno, finche' non parte. Il
processo e' del demone: chiudere la finestra non lo scarica.

## Aggiungere una capacita'

Una capacita' e' una cosa che il demone sa fare, con un nome, una descrizione
per il modello, un rischio e uno schema degli argomenti. La stessa capacita'
la vedono il modello di casa, un cervello agentico via MCP, la chat e la riga
di comando.

```rust
struct OraCap;

#[async_trait]
impl Capability for OraCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "sys.ora".into(),
            description: "Data e ora correnti del PC.".into(),
            risk: Risk::Safe,
            category: "sys".into(),
            schema: schema(&[]),
        }
    }

    async fn call(&self, _args: Value, _ctx: &Ctx) -> Result<Value> {
        Ok(json!("lunedi 28/09/2026 21:00"))
    }
}
```

Si registra con `reg.add(Arc::new(OraCap))` nella funzione `register` di un
file `caps_*.rs` di `nova-core`, e il modello la vede come `sys_ora`. Chi
cambia qualcosa implementa anche `anteprima`: e' quello che si mostra nella
richiesta di conferma. Senza, a chi chiede di provarla «per finta» il demone
risponde di no, invece di eseguirla davvero.

## Voce

Tutto in casa, senza Python: la voce e' Kokoro (ONNX, con espeak-ng per la
pronuncia), l'ascolto e' whisper.cpp. ElevenLabs e' una scelta, per chi la
vuole, e se la quota finisce si torna alla voce di casa. I pezzi si scaricano
quando servono:

```powershell
.\bin\nova componenti elenco               # cosa c'e' e cosa manca
.\bin\nova componenti scarica voce_locale  # Kokoro, e allo stesso modo onnx, espeak, ascolto_locale
```

Poi si accende dal pannello, alla voce **Voce**. NOVA risponde al proprio
nome: non c'e' un modello apposta per la parola di richiamo, e' whisper che
trascrive e il nome che apre la frase.

## Prestazioni e tuning

NOVA calcola da sola quanti layer entrano in VRAM (`nova-modelli` legge i
metadati del GGUF e li confronta con la VRAM libera).
Serve perche' su Windows, quando la VRAM finisce, il driver NVIDIA ripiega in
silenzio sulla memoria condivisa: il modello parte lo stesso ma va ~10x piu'
lento.

Misure su RTX 4060 Ti 16 GB con Qwen3.8-27B Q4_K_M (15,7 GB), con il prompt
vero della versione Python: 12.492 token fra regole e schemi dei sessanta
strumenti. Quelle sui flag e sui layer dipendono da llama-server, non da chi
lo chiama. Il banco e' `misure/banco_modello.py`, e le misura da solo.

Il prompt del demone l'ha misurato `misure/banco_prompt_demone.py` il 29
settembre, sulla stessa scheda ma con Gemma 4 26B-A4B IQ3_XXS e 30 layer in
GPU (il modello e' spiegato piu' sotto). Il demone manda 58 schemi e la
versione Python sessanta, e per il modello costano quasi uguale: 11.486 token contro
11.685, 5,9 s a freddo tutti e due, 146 ms a caldo contro 123.

**Il primo numero da guardare non e' la velocita', e' il divario fra freddo e
caldo:**

| | prompt a freddo | prompt a caldo |
|---|---|---|
| primo messaggio di una conversazione | **25,8 s** | — |
| tutti quelli dopo | — | **1,5 s** |

Diciassette volte meno, ed e' la cache del prefisso che lavora: regole e
schemi non cambiano fra un turno e l'altro, quindi si rielaborano una volta
sola. E' il motivo per cui il contesto della memoria e le ricette stanno **in
coda alla domanda** e non nel messaggio di sistema; spostarli «al posto
giusto» costerebbe venticinque secondi a messaggio, in silenzio.

**Poi i flag.** Misurati, non dedotti:

| Configurazione | Layer su GPU | Prompt a caldo | Generazione |
|---|---|---|---|
| come prima | 53 | 1504 ms | 6,0 t/s |
| `-fa on` | 53 | 1541 ms | 6,1 t/s |
| KV a 8 bit | 53 | 1281 ms | 6,5 t/s |
| KV a 8 bit, 60 layer | 60 | **691 ms** | **9,0 t/s** |
| KV a 8 bit, 62 layer | 62 | 600 ms | 7,7 t/s |
| KV a 8 bit, 64 layer | 64 | — | la VRAM satura, crolla |

Due cose che non si potevano sapere leggendo. **Flash attention era gia'
acceso**: in questa build il valore di fabbrica e' `auto`, e auto vuol dire
on — metterlo a mano non cambia niente. E **la KV cache a 8 bit non serve a
calcolare piu' in fretta**: serve a occupare meta' memoria, e su una scheda
dove il modello non ci sta tutto quella meta' diventa layer che tornano sulla
GPU. Il guadagno vero e' li', non nel calcolo.

NOVA adesso usa la KV a 8 bit di suo (`server.kv_cache_type`), e la stima dei
layer sa che la cache e' piu' piccola. La stima resta pero' prudente — 55
layer invece dei 60 che si sono misurati — perche' sbagliare per eccesso non
da' un errore: da' un modello che parte e va dieci volte piu' piano senza
dirlo. Chi vuole i 60 li mette a mano:

```jsonc
// config.json
"server": { "n_gpu_layers": 60, "kv_cache_type": "q8_0" }
```

E si rimisura, perche' la VRAM libera dipende da cos'altro c'e' acceso:

```powershell
python misure/banco_modello.py            # tutte le configurazioni
python misure/banco_modello.py kv8-60     # una sola
python misure/banco_prompt_demone.py --demone core\target\release\novad.exe
                                          # il prompt del demone contro quello del Python
python misure/banco_web_demone.py         # web_incolla, web_tabella e rete_cerca, chiesti al demone acceso
python misure/banco_taglio.py             # quanto costa accorciare la conversazione
python misure/banco_cervello.py           # sa scegliere lo strumento giusto? (quelli della versione Python)
python misure/banco_giudizio_llama.py     # le lettere del giudizio: n_probs, cache_prompt, /tokenize, quante giuste (CANT-12; --modello per un altro GGUF)
python misure/banco_clm.py confronta      # CLM da un GGUF contro il bf16 (CANT-12; i passi prima sono in testa al file)
python misure/banco_quale_cervello.py     # quale cervello: le parole di oggi contro le lettere, o CLM con --clm (CANT-12)
python misure/banco_giudizio_slot.py      # un giudizio sullo stesso llama-server costa la cache della conversazione?
python misure/banco_strumento_clm.py      # CLM sceglie lo strumento fra i 58 del modello di casa? Contro BM25
python misure/clm_addestra.py compiti     # le teste di CLM addestrate sulle scelte del modello grande (CANT-12; i passi in testa al file)
python misure/banco_cervelli_fuori.py chiedi --braccio claude   # le stesse domande a Claude Code e ad Antigravity, e la cascata con CLM
python misure/banco_clm_accanto.py        # CLM accanto al modello di casa: memoria video e millisecondi
```

`banco_cervello.py` misura una cosa diversa dalle altre: non quanto e' veloce
un modello, ma se **sa usare gli strumenti**. Un modello puo' fare quaranta
token al secondo e non saper chiamare un tool, e allora quei token non servono
a niente. Gemma 4 26B-A4B e Qwen3.8 27B fanno tutti e due 7 su 8, senza
inventare strumenti e senza chiamarne quando basta rispondere a parole.

`banco_taglio.py` misura una cosa che non si vede: quando la conversazione si
allunga NOVA la accorcia, e accorciarla butta via la cache del prompt. Se lo
si fa a ogni turno — ed e' quello che succedeva — da un certo punto in poi
ogni risposta rielabora tutto da capo e non si torna piu' indietro. Ora si
taglia di rado: tre volte in sessanta turni invece di trentuno, e il turno
dopo un taglio torna a costare 231 ms invece di 1.748.

Un 27B a Q4 su 16 GB non ci sta interamente, e cinque layer sulla CPU restano
il collo di bottiglia. Per andare molto piu' veloci ci sono due strade,
entrambe a un cambio di riga in `config.json`:

- un quant piu' piccolo dello stesso modello (Q3_K_M ~12,5 GB entra tutto in
  VRAM: 3-4x piu' veloce, qualita' leggermente inferiore);
- un modello piu' piccolo (8-14B) come "cervello rapido" per i comandi
  quotidiani, tenendo il 27B per i compiti complessi.

Per forzare un valore a mano: `server.n_gpu_layers` in `config.json`
(qualsiasi valore < 99 disattiva la stima automatica).

### Quale modello mettere: la scelta che conta piu' di tutte

Il catalogo (`models.json`) propone **Qwen3.8 27B**, ed e' una scelta
prudente: denso, forte, e su una 16 GB **non ci sta**. I numeri qui sopra
sono quelli di un modello con dodici layer sulla CPU.

C'e' una strada che quei numeri li ribalta, e vale la pena spiegarla perche'
non e' ovvia: i modelli **MoE**. In un modello denso da 27B ogni token fa
lavorare tutti i 27 miliardi di parametri. In un mixture-of-experts, per ogni
token se ne accende una frazione — il resto sta in memoria e tace.

| Modello | Totale | Attivi per token | Contesto | Note |
|---|---|---|---|---|
| Qwen3.8 27B (nel catalogo) | 27B | 27B — denso | 256K | il piu' forte, il piu' lento |
| [Gemma 4 26B-A4B](https://huggingface.co/google/gemma-4-26B-A4B) | 25,2B | **3,8B** | 256K | Apache 2.0, **multimodale**, chiamata di funzione nativa |
| [Nemotron 3 Nano 30B-A3B](https://unsloth.ai/docs/models/nemotron-3) | ~30B | **3B** | 1M | ibrido MoE, pensato per lavori agentici |

**Il compromesso, detto senza girarci intorno:** su un ragionamento difficile
un denso da 27B resta avanti. Ma un MoE con quattro miliardi di parametri
attivi *entra tutto in VRAM* su una scheda da 16 GB, e li' non si guadagna una
frazione — si cambia categoria. Un assistente che risponde in due secondi e
sbaglia una volta su venti e' piu' utile di uno che risponde in trenta e
sbaglia una volta su venticinque, perche' il secondo non lo apri.

**E adesso non e' piu' una previsione: e' una misura.** Stessa macchina
(RTX 4060 Ti, 16 GB), stessa configurazione, stesso prompt vero da dodicimila
token, i due modelli uno dopo l'altro nella stessa sessione:

| | strati in GPU | prompt a freddo | prompt a caldo | generazione |
|---|---|---|---|---|
| Qwen3.8 27B Q4_K_M (15,7 GB) | 53 su 65 | 26,5 s | 1.363 ms | **6,0 tok/s** |
| Gemma 4 26B-A4B Q3_K_XL (12,0 GB) | **30 su 30** | 6,1 s | **145 ms** | **42,4 tok/s** |

**Sette volte** in generazione, nove sul prompt a caldo. La colonna che
spiega tutte le altre e' la prima: 30 su 30 contro 53 su 65. Non e' il MoE a
fare la magia — e' che uno dei due ci sta e l'altro no, e i dodici strati che
Qwen lascia in RAM costano piu' di tutto il resto messo insieme. Il MoE serve
a rendere possibile quel «ci sta»: 3,8 miliardi di parametri attivi invece di
27 sono cio' che permette a un modello da 26B di stare in dodici gigabyte
senza diventare inservibile.

Due avvertenze oneste su questa tabella. Le quantizzazioni non sono pari
(Q3_K_XL contro Q4_K_M): e' voluto, perche' la regola e' scegliere **la piu'
grande che entra**, ed e' proprio quella la scelta che si sta misurando. E si
sta misurando la **velocita'**, non la qualita' delle risposte: quella non si
misura con un cronometro, e su un ragionamento difficile il denso resta
avanti.

Per NOVA in particolare, due dettagli di Gemma 4 pesano piu' dei benchmark:
e' **multimodale** — quindi le schermate funzionano anche con il cervello di
casa, non solo con quello in rete — e ha la **chiamata di funzione nativa**,
che e' esattamente il modo in cui NOVA parla ai suoi strumenti.

**Come sceglierne uno diverso.** `models.json` non e' codice, e' un dato: il
migliore cambia ogni mese, e se stesse nel codice ogni modello nuovo sarebbe
una release. Si aggiunge una famiglia al file, oppure si punta direttamente a
un `.gguf` che hai gia':

```jsonc
// config.json
"server": { "model_path": "D:/modelli/gemma-4-26B-A4B-Q4_K_M.gguf" }
```

E la regola per la quantizzazione e' una sola, la stessa che usa LM Studio:
**si sceglie la piu' grande che ENTRA, non la piu' grande che si riesce a
caricare.** Se non ci sta tutta, llama.cpp mette una parte dei layer in RAM e
funziona lo stesso — dieci volte piu' piano, senza dire niente.

### Se non hai una scheda video

Anche questo era scritto e non misurato, e misurandolo e' venuto fuori che la
risposta dipende quasi tutta da **quale** modello, non da quanto e' potente il
processore. Stessa macchina, stesso prompt, zero layer sulla GPU:

| in CPU pura | prompt a caldo | generazione |
|---|---|---|
| Gemma 4 26B-A4B (MoE, 3,8B attivi) | 1,3 s | **7,6 tok/s** |
| Qwen3.8 27B (denso) | 5,7 s | **1,8 tok/s** |

**Quattro volte e mezzo di differenza fra due modelli della stessa taglia**,
perche' sul processore si paga per i parametri che si accendono, non per
quelli che ci sono. E il numero che sorprende di piu' e' un altro: il MoE **in
CPU** (7,6 tok/s) va piu' veloce del denso **sulla GPU** (6,0 tok/s) di questa
macchina.

Quindi, detto onestamente: 7,6 token al secondo sono piu' veloci di quanto
legga una persona, e NOVA senza scheda video **si usa**. A 1,8 una risposta di
ottanta token arriva in quarantacinque secondi, e non si usa. Se non hai una
GPU la scelta che conta non e' fra locale e non locale: e' **prendere un
MoE**.

Restano due strade, e nessuna delle due e' un ripiego: **un abbonamento che
hai gia'** (Claude Code, Codex, Gemini, Qwen — NOVA li pilota come cervelli)
oppure una **chiave API**. Il modello locale e' una scelta di riservatezza e
di costo, non l'unica via.

---

## Memoria: knowledge base a grafo

NOVA ha una memoria a lungo termine che sopravvive alle sessioni: un **vault
markdown in `NOVA\vault`, apribile in Obsidian cosi' com'e'** (frontmatter +
`[[wikilink]]`, quindi la vista a grafo di Obsidian funziona senza plugin).

La pipeline di retrieval viene da `knowledge-lab/backend/src/retrival`, ed e'
in `nova-memoria`:

```
query
  1. bypass codice esatto      slug o tag identico -> boost
  2a. sparse  (BM25)           titolo x2.5, tag x2.0
  2b. dense   (embedding)      similarita' coseno
  3. RRF fusion (k=60)         un solo ordinamento
  4. filtro                    PRIMA del taglio a top-K, mai a valle
  5. espansione grafo 1-hop    i vicini dei migliori, ri-filtrati
  6. taglio a top-K
  7. audit                     vault\.nova\audit.jsonl
```

### Struttura

```
core/crates/
  nova-nodi/      nodo + frontmatter, il vault su disco, il guardiano dei segreti
  nova-memoria/   BM25 + embedding di casa + RRF + scelta
  nova-ricette/   le procedure imparate
  nova-core/      memoria.rs (la ricerca nel turno), imparare.rs (i fatti
                  durevoli), caps_memoria.rs (gli strumenti kb_*)
```

### Il vault

```
vault/
  _INDICE.md          hub di navigazione, rigenerato a ogni scrittura
  01-profilo/         profilo utente, preferenze
  02-persone/         collaboratori (dedotti dai co-autori git)
  03-progetti/        un nodo per repo o cartella di lavoro
  04-ambiente/        hardware, app installate, modelli, runtime
  05-abitudini/
  06-fatti/           tutto il resto
  .nova/audit.jsonl   ogni ricerca e ogni scrittura, con timestamp
```

Ogni nodo porta `origine` (`scansione` | `auto` | `utente`) e `confidenza`:
si distingue sempre cio' che NOVA ha dedotto da cio' che le hai detto tu.
Un fatto confermato una seconda volta alza la propria confidenza; `utente`
vince sempre su `auto`.

### Come impara

- **Seed**: la prima volta che si accende, il demone crea il vault e ci
  scrive quel che trova sul PC: il profilo (col nome git, senza email), le
  preferenze nella lingua scelta, l'ambiente, le applicazioni e i progetti
  nelle cartelle dell'utente. Le persone no. `novad --semina` la rifa' a
  mano.
- **Automatico**: dopo ogni scambio una fila in background estrae i fatti
  *durevoli* (preferenze, progetti, persone, decisioni) e li scrive. Non
  memorizza richieste una tantum, output di comandi o orari.
  Non impara da un turno che ha guardato lo schermo.
- **Esplicito**: gli strumenti `kb_nota`, `kb_collega`, `kb_dimentica` quando
  dici "ricordati che...". Un testo che somiglia a una credenziale non entra.
- **Iniezione**: prima di ogni turno i nodi rilevanti finiscono **in coda alla
  domanda**, non nel prompt di sistema: cosi' la cache del prompt resta buona e
  NOVA non ti richiede cose che gia' sa.

### Strumenti esposti al modello

| Strumento | Cosa fa |
|---|---|
| `kb_cerca` | cerca nella memoria (pipeline ibrida completa) |
| `kb_nota` | crea o aggiorna un nodo |
| `kb_collega` | collega due nodi (grafo non orientato) |
| `kb_vicini` | esplora i collegamenti di un nodo |
| `kb_dimentica` | archivia un nodo superato (il file resta sul disco) |
| `kb_stato` | nodi, tipi, collegamenti, nodi isolati |
| `kb_procedure` | le procedure imparate |
| `kb_procedura_dimentica` | toglie una procedura sbagliata |

### Da riga di comando

```powershell
.\bin\nova call kb.cerca query="orario di lavoro"    # interroga la memoria
.\bin\nova call kb.stato                             # stato della memoria
```

### Configurazione (`kb` in config.json)

| Chiave | Default | Cosa fa |
|---|---|---|
| `enabled` | `true` | attiva la memoria: spenta, non si crea il vault, non si cerca, non si scrive e non si impara |
| `vault_path` | `NOVA\vault` | dove vivono i nodi |
| `auto_seed` | `true` | mappatura iniziale del PC, alla prima accensione del demone |
| `auto_learn` | `true` | scrittura automatica dopo ogni scambio |
| `inject_context` | `true` | iniezione del contesto prima del turno |
| `top_k` | `5` | quanti nodi entrano nel prompt |
| `min_confidence` | `0.25` | sotto questa soglia un nodo non viene usato |
| `embedder` | `hash` | `hash`, l'embedding di casa: offline, niente modelli |

L'embedding di casa non capisce i sinonimi: e' un hashing di parole, e il
lavoro vero lo fa BM25. La versione Python sapeva anche chiedere i vettori a
un modello di embedding su un'altra porta (`embedder: "llama"`); nel demone
quella strada **manca**, e la configurazione che la chiede usa quello di casa.

## Nota sul ragionamento

Qwen3.8 e' un modello *thinking*: lasciato libero produce 1000+ token di
ragionamento per turno, che a 7 t/s significa due minuti di attesa. Per questo
il server parte con `--reasoning-budget 512`. Alzalo in
`server.extra_args` se preferisci risposte piu' ragionate e piu' lente,
mettilo a `0` per disattivare del tutto il ragionamento.

---

## Tre cervelli intercambiabili

Quello che *pensa* sta dietro `nova-cervelli`. Si cambia a caldo dal menu
**Cervello**, senza perdere la conversazione ne' la memoria.

| Cervello | Cos'e' | Agentico |
|---|---|---|
| `locale` | il GGUF servito da llama-server sul tuo PC | no |
| `claude` | Claude Code CLI in headless | si' |
| una CLI | Codex, Gemini, Qwen, o una dichiarata in `brains.cli` | si' |
| `api` | qualunque endpoint OpenAI-compatibile | no |

**Agentico** e' la differenza che conta. `locale` e `api` *propongono* tool
call e NOVA li esegue applicando guardie e livelli di autonomia. `claude` ha
mani proprie: NOVA gli fa da tramite, gli passa il contesto e la memoria, e
riporta cosa ha fatto, in quanti turni e quanto e' costato.

```powershell
.\bin\nova call cervelli.stato    # chi c'e' e chi e' pronto
.\bin\nova chiedi "..."           # una richiesta sola, dal demone
```

### Claude Code come cervello

Serve `npm install -g @anthropic-ai/claude-code` e un `claude` gia'
autenticato. NOVA:

- lo lancia in headless (`-p --output-format json`): la domanda passa dallo
  standard input e il prompt di sistema da un file, perche' su Windows una
  riga di comando si ferma a 8191 caratteri
- mantiene la sessione fra un turno e l'altro con `--resume <session_id>`
- traduce i **tuoi** livelli di autonomia nei suoi permessi:

  | Autonomia NOVA | `--permission-mode` |
  |---|---|
  | Conferma sempre | `default` (chiede per ogni azione, con lo sportello di NOVA) |
  | Conferma azioni rischiose | `acceptEdits` |
  | Autonomo | `bypassPermissions` |

- gli collega il demone come **server MCP** (`nova mcp`, un ponte fra lo
  standard input e il canale del demone): Claude vede tutti i 129
  strumenti, da `mcp__nova-core__kb_cerca` in giu', con le stesse guardie del
  modello di casa, che ne riceve 58 perche' nel suo contesto tutti non ci
  stanno. Le conferme passano dallo sportello di NOVA
  (`--permission-prompt-tool`), cioe' dal bottone nella chat.
- riporta costo e token di ogni turno nel registro azioni.

Il modello si sceglie da solo (D377): `ultimo:sonnet`, `ultimo:opus`,
`ultimo:fable` vogliono dire «il piu' recente di quella famiglia che parte».
Il demone lo trova provando Claude Code (un nome che non esiste costa zero e
risponde in un paio di secondi), tiene il risultato in `modelli.json` e lo
rifa' ogni giorno; `novad --modelli` lo rifa' subito. Gli alias da soli non
bastano: su Claude Code 2.1.292 `sonnet` porta ancora a `claude-sonnet-5`,
mentre `claude-sonnet-5-5` c'e'. Se il modello piu' recente vuole una Claude
Code piu' nuova, NOVA lancia `claude update` e lo scrive nel registro azioni
(`brains.aggiorna_claude: false` lo spegne). Un nome scritto per intero resta
quello; i vecchi predefiniti `claude-sonnet-5` e `claude-opus-5` valgono come
`ultimo:sonnet` e `ultimo:opus`.

### API esterna

`brains.api_base_url` + `brains.api_model` + una chiave (in `brains.api_key`
oppure nella variabile d'ambiente indicata da `brains.api_key_env`). Va con
OpenAI, OpenRouter, Groq, Together e chiunque parli lo stesso dialetto. Usa il
ciclo di tool di NOVA, quindi guardie e autonomia restano identiche.

### Cosa esce dal PC

Con `locale` niente, mai. Con `claude` e `api` escono la richiesta, il
contesto della conversazione e i nodi di memoria pertinenti al messaggio: e'
la scelta che rende quei cervelli utili, ma va fatta sapendo cosa comporta.
Il selettore e' li' apposta: per il lavoro sensibile torna su `locale`.

---

## Il modello appartiene al demone

llama-server non e' un processo della finestra: e' del demone, che lo accende
quando serve e lo supervisiona.

```
llama-server pid 2760 -> padre: novad
```

Conseguenze pratiche:

| | prima | adesso |
|---|---|---|
| Chiudi la finestra | il modello si scarica | resta caricato |
| Riapri NOVA | ~2 minuti di caricamento | **2 secondi** |
| Il server cade | resta giu' | si riaccende alla domanda dopo |
| Log del modello | file che nessuno legge | eventi `proc.output` sul bus, piu' buffer circolare |

Prima di un turno che comincia dal modello di casa il demone prova nella
sequenza: *qualcuno risponde gia' sulla porta?* → lo usa, che sia il suo o un
altro; *e' il suo e si sta caricando?* → aspetta; altrimenti lo accende
(D358).

Chiavi in `server` di `config.json`:

| Chiave | Default | Cosa fa |
|---|---|---|
| `autostart_model` | `true` | accende il modello quando un turno ne ha bisogno |
| `binary` | vuoto | quale llama-server; vuoto = il migliore fra quelli trovati |
| `n_gpu_layers` | `999` | sotto 99 e' una scelta tua, da 99 in su lo stima NOVA |
| `startup_timeout` | `600` | quanto aspettare il caricamento, in secondi |

La finestra si sottoscrive a `proc.*` e ha i log del modello dal bus.

---

## Chi risponde a cosa: il router

Il modello locale **orchestra**. È gratis, è privato, sta già in VRAM, e per
capire cosa vuoi e chiamare i tool giusti basta e avanza. Quando il compito lo
supera non ci prova lo stesso: passa la palla e riprende in mano il risultato.

I gradini sono in `brains.routing.tiers` di `config.json`, in ordine di
potenza. Quelli predefiniti:

| Gradino | Cervello | Modello | Quando |
|---|---|---|---|
| `locale` | GGUF sul PC | Qwen3.8-27B | orchestrazione e compiti semplici |
| `standard` | Claude Code | `ultimo:sonnet` | il cavallo da lavoro |
| `difficile` | Claude Code | `ultimo:opus` | quando il compito lo merita |
| `alternativo` | Antigravity | `ultimo:gemini-*-pro-high` | seconda opinione |

```powershell
.\bin\nova call cervelli.stato     # gradini, stato, speso / tetto
```

### Come passa la palla

Tre strade, in ordine di intelligenza:

1. **`cervelli_delega`** — il modello sceglie. Scrive il compito per intero (chi lo
   riceve non vede la conversazione) e passa i **percorsi** dei file in `file`:
   li allega NOVA, gratis. Poi riprende lui con la risposta.
2. **Escalation automatica** — se NOVA sbaglia due volte di fila, o fa quattro
   chiamate senza arrivare a una risposta, sale di gradino da sola e infila il
   risultato nella conversazione. Sono due modi diversi di non farcela:
   sbattere contro un muro, e girare a vuoto.
3. **`cervelli_secondo_parere`** — la stessa domanda a due gradini, per confrontare.

### Guardie

| Chiave (`brains.routing`) | Default | Cosa fa |
|---|---|---|
| `orchestratore` | `locale` | chi guida la conversazione |
| `escalation_automatica` | `true` | sale da sola quando serve |
| `fallimenti_prima_di_salire` | `2` | tentativi andati male |
| `passi_prima_di_salire` | `4` | chiamate senza risposta |
| `salite_massime` | `2` | quante volte per turno |
| `tetto_usd_sessione` | `5.0` | oltre, le deleghe a pagamento si fermano |
| `solo_locale` | `false` | `true` = niente esce dal PC, punto |

### Aggiungere un modello senza scrivere codice

Le CLI agentiche esterne si dichiarano in `brains.cli`; poi si citano in un
gradino. `{model}` viene sostituito.

```json
"cli": {
  "deepseek": {
    "etichetta": "DeepSeek",
    "binary": "deepseek",
    "args": ["--model", "{model}"],
    "model": "deepseek-reasoner",
    "prompt": "stdin"
  }
}
```

### Numeri misurati

| | tempo | costo |
|---|---|---|
| `standard` (Sonnet), domanda secca | 7,1 s | 0,016 $ |
| `alternativo` (Gemini), domanda secca | 21,7 s | 0 $ |
| `difficile` (Opus), review di un file da 300 righe | 112,7 s | **0,89 $** |

Opus costa: con il tetto a 5 $ ci stanno cinque review come quella. È il motivo
per cui l'orchestratore è il locale e non lui.

### Cosa ha insegnato la prova

Alla prima versione il modello locale **non delegava**: davanti a «critica
architetturale severa di questo file» ha fatto dieci chiamate di tool per
raccogliere contesto senza mai passare la palla. Due correzioni:

- il prompt ora elenca i casi concreti in cui delegare *subito* (giudicare
  codice, progettare, ragionamenti lunghi, molti file insieme) invece di dire
  genericamente «se ti supera»;
- l'escalation automatica guarda anche il numero di passi, non solo i
  fallimenti — perché girare a vuoto è l'altro modo di non farcela.

Dopo le correzioni, con la stessa richiesta: legge il file, annuncia
«ora delego la critica a un modello più capace», sceglie **`difficile`** da
solo e motiva — *«richiede ragionamento fine su race condition tokio e
correctness concorrente; supera le mie possibilità di analisi affidabile»* —
allega i file e riprende il controllo con la risposta.

## Lo screenshot è un accessorio

C'è uno strumento, `schermo_cattura`, e serve per le domande sull'aspetto delle cose
(«che ne pensi di questa interfaccia?»). **Non è una fondamenta**: per *agire*
su un'applicazione NOVA usa l'albero di accessibilità, che è preciso,
istantaneo e non costa niente. Dare la vista a un modello per fargli premere
un pulsante è lento e caro; averla per esprimere un giudizio è un di più.

### Abbonamento, non spesa

NOVA legge `~/.claude/.credentials.json` e riconosce il tipo di accesso. Su
questo PC:

```
accesso: ('abbonamento', 'max_5x')
```

Con un abbonamento il `total_cost_usd` che Claude Code riporta è un
**equivalente API**: dice quanto pesa una richiesta, non quanto hai speso. Il
tetto in dollari quindi **non si applica** ai gradini coperti da abbonamento —
si applica solo a chi paga a token (`brain: "api"`, oppure una CLI dichiarata
con `"a_consumo": true`).

```
orchestratore: locale   nessun gradino a consumo:
0.0 $ è l'equivalente API, non una spesa

* locale       locale       predefinito                locale       pronto
  standard     claude       ultimo:sonnet              abbonamento  pronto
  difficile    claude       ultimo:opus                abbonamento  pronto
  alternativo  antigravity  ultimo:gemini-*-pro-high   incluso      pronto
```

### Quando finisce la quota

Con l'abbonamento il vincolo vero non sono i soldi, sono i **limiti d'uso**. È
una cosa diversa da un errore: non vuol dire «non ci riesco», vuol dire
«riprova più tardi». NOVA la tratta come tale:

1. riconosce il messaggio di quota esaurita (`usage limit`, `rate limit`, 429, …)
   come «non adesso», non come un errore generico;
2. mette **quel gradino in pausa** per il tempo indicato;
3. **ripiega su un altro fornitore** — non su un altro modello dello stesso,
   perché il limite è sul conto, non sul modello — e in ultima istanza torna
   sul locale.

```
«difficile» in pausa per 30 minuti: quota esaurita
«difficile» è a quota: ripiego su «alternativo»
esito finale: da «alternativo»
motivo: prova (ripiego: «difficile» a quota)
```

Si disattiva con `ripiego_su_limite: false`, se preferisci che si fermi e te lo
dica invece di cambiare modello da solo.
