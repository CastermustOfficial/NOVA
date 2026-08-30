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

## Cosa sa fare

Un elenco di aggettivi non dice niente. Questi sono i numeri, contati dal
codice: **60 strumenti** per il modello che gira sul tuo PC, **31** per un
cervello agentico che lavora per conto suo, **38 formati** di file che sa
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

NOVA pilota Chrome parlando con lui in CDP. Non simula le battute sui tasti:
incolla. Riempire cinque campi di un foglio di calcolo online costa **una**
chiamata invece di cinque, e leggere una tabella intera ne costa una sola.

| Operazione | Misurato |
|---|---|
| `web_incolla` — cinque valori in tre campi | 35 ms |
| `web_tabella` — una tabella 5x4 letta tutta | 33 ms |
| `web_cerca` — cercare senza aprire il browser | ~0,9 s |

L'ultima riga e' quella che cambia il carattere dell'assistente: **prima di
aprire una pagina, NOVA cerca**. Un browser che si apre e' una finestra che
compare sul tuo schermo; una ricerca che passa da un browser senza volto non
lo e'.

### Ricorda, e quello che impara resta tuo

Una memoria a grafo su file `.md` — apribile in Obsidian, versionabile in git,
leggibile senza NOVA. Impara i fatti durevoli dopo ogni scambio, e **le
procedure**: come ha risolto una richiesta, per non doverla ricercare la volta
dopo. Le procedure si ritrovano anche quando la richiesta e' scritta in modo
diverso o con un refuso, perche' il confronto passa da tri-grammi di caratteri
e non da un'uguaglianza di stringhe.

Sul disco di chi scrive queste righe, adesso: 138 note e 28 procedure imparate.

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

`web_cerca` trova senza aprire il browser, `web_prendi` scarica una pagina
come testo in mezzo secondo invece di sei, e i documenti che hai gia' sul
disco entrano nell'harness. La differenza rispetto a farsi riassumere le cose
da una chat e' che la risposta dice **dove**: file e pagina, non «mi risulta
che».

**La catena:** `web_cerca` -> `web_prendi` -> `harness_apri` ->
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

**La catena:** `modelli` (chi c'e') -> `delega` -> la risposta torna dentro
la stessa conversazione

### Capire perche' il PC va piano

> «Perche' e' lento?»

Legge lo stato vero invece di indovinare: memoria, processi, dischi, quanti
layer del modello stanno davvero in VRAM. Su Windows, quando la VRAM finisce,
il driver ripiega in silenzio sulla RAM condivisa e il modello va dieci volte
piu' piano senza dire niente - NOVA lo vede e lo dice.

**La catena:** `system_info` -> `list_processes` -> `run_powershell`

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
e le applica quando glielo dici. Il **banco** (`nova/banco.py`) le permette di
provare una riparazione su una copia prima di toccare l'originale.

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
| `.pdf` | le **pagine vere**, disegnate come immagini, non il testo estratto |
| `.docx` | in lettura, con la struttura |
| `.md` `.txt` | su un foglio bianco che si scrive, con i ferri del mestiere |
| `.html` | **reso**, con Chromium: e' un artifact, si guarda per quello che fa |

Chiedere «dove si parla di entropia» non torna una frase: torna una
**posizione** — file e pagina — e il documento ci scende sopra e la evidenzia.
Con una cartella aperta come progetto la ricerca vale su tutta la pila, che e'
la domanda vera quando i documenti sono sei PDF di un esame: non «dove sta in
questo file» ma «in quale file sta».

### Codice

Trentadue estensioni, dal Python al Rust al Vue. Il codice si apre su fondo
scuro, con i colori di Pygments — cinquecento linguaggi, non i quattro che
avremmo scritto a mano — e i numeri di riga, perche' un errore si dice cosi':
file e riga. Un `.html` mostra il risultato, e il sorgente e' a un click:
si cambia, si salva, e la pagina si ridisegna.

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

Cosa **non** fa ancora: non esegue i test del progetto e non applica una
modifica «solo se passano». Il verificatore e' il pezzo che manca, ed e'
quello che trasformerebbe l'harness da un buon posto per leggere a un buon
posto per programmare.

## Installazione

### Requisiti

| | |
|---|---|
| Sistema | **Windows 10/11 a 64 bit** |
| Python | 3.10 o superiore |
| Disco | 3 GB per il minimo; 15-30 GB se scegli un modello locale |
| GPU | facoltativa: serve solo per il modello locale |

NOVA e' legata a Windows in profondita': l'automazione usa UI Automation e
l'archivio credenziali usa DPAPI. Su macOS e Linux il codice compila ma non
fa niente. **Non serve ne' Rust ne' Visual Studio**: il core arriva gia'
compilato.

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
| `.\install.ps1 -Disinstalla` | toglie avvio automatico e collegamento |

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
`config.json`. Le CLI riconosciute sono descritte in `nova/routing.py`
(`cli_predefinite`): aggiungerne una non richiede codice, solo una voce sotto
`brains.cli`.

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
configurazione. Vivono in `%APPDATA%\NOVA`.

## Documentazione

- [Documento di architettura](docs/architettura.md) — le decisioni prese e il
  perche', comprese quelle scartate.
- [Come contribuire](CONTRIBUTING.md)

## Per chi sviluppa

```powershell
.\build.ps1              # compila il core Rust (release)
.\build.ps1 -Test        # esegue i test Rust
python -m pytest -q       # esegue i test Python
```

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
demone e' costruito come *un trait, tre backend*:

| Serve per | Windows | macOS | Linux |
|---|---|---|---|
| Controllare qualsiasi app | UI Automation | Accessibility API | AT-SPI2 |
| Osservare tutto il sistema | ETW | EndpointSecurity | eBPF |
| Annullare cio' che si e' fatto | VSS | snapshot APFS | overlayfs / btrfs |
| Canale locale | named pipe | socket unix | socket unix |

Il vincolo vero e' la portabilita', non il ring 0: queste capacita' esistono
gia' in userspace su ogni sistema. Non serve un sistema operativo, serve un
processo scritto attorno a quella forma.

**Perche' un binario e' un binario.** Il demone si scarica compilato: chi
installa NOVA non ha bisogno ne' di Rust ne' di Visual Studio.

Quello che resta in Python e' il ciclo dell'agente, gli strumenti e la
memoria — dove le idee cambiano ogni settimana e la velocita' di modifica vale
piu' della velocita' di esecuzione. Il confine fra i due e' voluto e sta
scritto in [`core/README.md`](core/README.md).

## Architettura

```
bin/nova-shell.exe    l'orb e le finestre (avviato all'accensione)
install.ps1           installazione, runtime CUDA, avvio automatico, collegamento
nova/
  main.py             entrypoint, GUI o CLI
  config.py           configurazione persistente (%APPDATA%\NOVA\config.json)
  setup_wizard.py     rilevamento automatico di modello GGUF e runtime
  runtime.py          avvia/sorveglia/spegne llama-server.exe (+ auto-tuning GPU)
  agent.py            ciclo agente: modello <-> tool, sicurezza, approvazioni
  processi.py         nessun processo di NOVA apre una finestra nera
  browser.py          pilota Chrome in CDP: incolla, tabelle, caricamenti
  cerca.py            ricerca web senza aprire un browser sullo schermo
  ricette.py          le procedure imparate, ritrovate anche con un refuso
  registro.py         cio' che non si annulla, si annota
  pianificazione.py   attivita' ricorrenti e sentinelle
  fascicolo.py        i fatti veri sull'utente: CV, esperienze, testi suoi
  harness.py          documenti e progetti: aprire, cercare, indicare
  harness_modifica.py proporre modifiche, e applicarle solo su richiesta
  harness_finestra.py la finestra: documento, albero, chat
  evidenzia.py        i colori del codice (Pygments) e i numeri di riga
  markdown_qt.py      Markdown fedele in andata e ritorno
  mcp_kb.py           i 31 strumenti esposti a un cervello agentico
  tools/
    base.py           registry, schemi OpenAI, livelli di rischio
    files.py          leggere, scrivere, cercare, spostare, aprire
    apps.py           avviare app, elencare/focalizzare/chiudere finestre
    shell.py          PowerShell, CMD, Python
    web.py            ricerca web, lettura pagine, apertura nel browser
    system.py         appunti, tasti, volume, notifiche, promemoria, info PC
    schermo.py        schermate, quando leggere il sistema non basta
    automazioni.py    strumenti che NOVA scrive da se'
    procedure.py      come ha risolto una richiesta, per rifarla
    riparazione.py    il banco: si ripara da sola senza rompersi
  ui/main_window.py   finestra chat + registro azioni + tray + hotkey
  voice/              ascolto e voce: Kokoro, whisper.cpp, ElevenLabs, SAPI
core/crates/
  nova-core/          il demone: bus, capacita', processi lunghi, RPC
  nova-voce/          audio, Kokoro, whisper, Scribe: niente Python
  nova-shell/         l'orb e le finestre (Tauri)
```

## Livelli di autonomia

Impostabili al volo dal menu in alto a destra (o in `config.json`):

| Livello | Comportamento |
|---|---|
| `always_ask` | conferma per **ogni** azione, anche le sole letture |
| `ask_risky` | conferma solo per le azioni `DANGEROUS` (shell, delete, chiusura app, tasti) |
| `autonomous` | nessuna conferma, tutto tracciato nel registro azioni |

Ogni tool e' classificato `SAFE` / `MODERATE` / `DANGEROUS`. Oltre
all'autonomia valgono sempre due guardie non aggirabili dal modello:

- `safety.protected_paths` - percorsi mai scrivibili (Windows, Program Files, ...)
- `safety.forbidden_command_patterns` - regex di comandi bloccati (format, diskpart, ...)
- `safety.write_roots` - se valorizzato, le scritture sono confinate a quelle cartelle

## Runtime del modello

All'avvio NOVA cerca un `llama-server.exe`, in quest'ordine:

1. `NOVA\runtime\` (build CUDA scaricata da `get_cuda_runtime.ps1`)
2. i backend gia' presenti in `%USERPROFILE%\.lmstudio\extensions\backends`
3. `LLAMA_CPP_HOME`

Poi lancia il server come processo figlio e lo spegne alla chiusura. Se il
modello non entra in VRAM, `auto_tune_gpu_layers` riprova da solo scalando i
layer offloadati finche' non parte.

## Aggiungere un tool

```python
from nova.tools.base import Risk, tool

@tool(
    "invia_email",
    "Invia una email tramite Outlook.",
    {"to": {"type": "string", "description": "Destinatario"},
     "subject": {"type": "string", "description": "Oggetto"},
     "body": {"type": "string", "description": "Testo"}},
    Risk.DANGEROUS, category="mail",
    preview=lambda a: f"Invia email a {a['to']}: {a['subject']}",
)
def invia_email(to: str, subject: str, body: str) -> str:
    ...
    return "Email inviata."
```

Importa il modulo in `nova/tools/__init__.py` e il modello lo vede subito.

## Voce

`nova/voice/` e' gia' predisposto: `stt.py` (faster-whisper, push-to-talk o
wake word) e `tts.py` (SAPI di Windows, zero dipendenze). Per attivarli:

```powershell
pip install faster-whisper sounddevice
```

poi `voice.enabled = true` in `config.json`.

## Prestazioni e tuning

NOVA calcola da sola quanti layer entrano in VRAM (`nova/gguf.py` legge i
metadati del modello, `estimate_gpu_layers` li confronta con la VRAM libera).
Serve perche' su Windows, quando la VRAM finisce, il driver NVIDIA ripiega in
silenzio sulla memoria condivisa: il modello parte lo stesso ma va ~10x piu'
lento.

Misure su RTX 4060 Ti 16 GB con Qwen3.8-27B Q4_K_M (15,7 GB):

| Configurazione | Layer su GPU | Generazione |
|---|---|---|
| CUDA, `-ngl 99` (VRAM saturata) | 65 | ~2 t/s, prompt 40 t/s |
| Vulkan, auto | 56 | ~8 t/s |
| CUDA, auto (stima VRAM) | 53 | ~7-9 t/s |

Un 27B a Q4 su 16 GB non ci sta interamente: circa 12 layer restano sulla CPU
ed e' quello il collo di bottiglia. Per andare molto piu' veloci ci sono due
strade, entrambe a un cambio di riga in `config.json`:

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

Per NOVA in particolare, due dettagli di Gemma 4 pesano piu' dei benchmark:
e' **multimodale** — quindi le schermate funzionano anche con il cervello di
casa, non solo con quello in rete — e ha la **chiamata di funzione nativa**,
che e' esattamente il modo in cui NOVA parla ai suoi sessanta strumenti.

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

NOVA gira lo stesso, e va piano: si passa da decine di token al secondo a
pochi. Va detto prima, non scoperto dopo. In quel caso le strade sensate sono
due, e nessuna delle due e' un ripiego: **un abbonamento che hai gia'**
(Claude Code, Codex, Gemini, Qwen — NOVA li pilota come cervelli) oppure una
**chiave API**. Il modello locale e' una scelta di riservatezza e di costo,
non l'unica via.

---

## Memoria: knowledge base a grafo

NOVA ha una memoria a lungo termine che sopravvive alle sessioni: un **vault
markdown in `NOVA\vault`, apribile in Obsidian cosi' com'e'** (frontmatter +
`[[wikilink]]`, quindi la vista a grafo di Obsidian funziona senza plugin).

La pipeline di retrieval e' il porting Python di
`knowledge-lab/backend/src/retrival`:

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
nova/kb/
  schema.py     nodo + frontmatter (parser proprio, come nodeLoader.ts)
  store.py      vault su disco, indice, relazioni bidirezionali, dedup, audit
  retrieval.py  BM25 + embedder + RRF + espansione grafo + KBEngine
  memory.py     apprendimento automatico dalle conversazioni
  seed.py       mappatura iniziale del PC
nova/kb_setup.py  regia: vault + motore + memoria
nova/tools/kb.py  i 6 tool con cui il modello usa la memoria
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

- **Seed**: alla prima esecuzione mappa profilo, progetti, ambiente e persone.
- **Automatico**: dopo ogni scambio un thread in background estrae i fatti
  *durevoli* (preferenze, progetti, persone, decisioni) e li scrive. Non
  memorizza richieste una tantum, output di comandi o orari.
- **Esplicito**: i tool `kb_note`, `kb_link`, `kb_forget` quando dici
  "ricordati che...".
- **Iniezione**: prima di ogni turno i nodi rilevanti finiscono nel prompt di
  sistema, cosi' NOVA non ti richiede cose che gia' sa.

### Tool esposti al modello

| Tool | Cosa fa |
|---|---|
| `kb_search` | cerca nella memoria (pipeline ibrida completa) |
| `kb_note` | crea o aggiorna un nodo |
| `kb_link` | collega due nodi (grafo non orientato) |
| `kb_neighbors` | esplora i collegamenti di un nodo |
| `kb_forget` | archivia un nodo superato (il file resta sul disco) |
| `kb_stats` | nodi, tipi, collegamenti, nodi isolati |

### Da riga di comando

```powershell
python -m nova --kb "orario di lavoro"    # interroga la memoria
python -m nova --kb-stats                 # stato della KB
python -m nova --seed-kb                  # ri-mappa il PC (idempotente)
```

### Configurazione (`kb` in config.json)

| Chiave | Default | Cosa fa |
|---|---|---|
| `enabled` | `true` | attiva la memoria |
| `vault_path` | `NOVA\vault` | dove vivono i nodi |
| `auto_seed` | `true` | mappatura iniziale del PC |
| `auto_learn` | `true` | scrittura automatica dopo ogni scambio |
| `inject_context` | `true` | iniezione del contesto prima del turno |
| `top_k` | `5` | quanti nodi entrano nel prompt |
| `min_confidence` | `0.25` | sotto questa soglia un nodo non viene usato |
| `embedder` | `hash` | `hash` (offline) oppure `llama` |
| `embedder_url` | `:8421` | secondo llama-server con un modello di embedding |

Con `embedder: "llama"` NOVA usa un vero modello di embedding servito su
un'altra porta (es. `nomic-embed-text`), guadagnando sulle riformulazioni.
Se non risponde, ricade da sola sull'embedder locale: la KB non si rompe mai
per colpa di un server spento.

## Nota sul ragionamento

Qwen3.8 e' un modello *thinking*: lasciato libero produce 1000+ token di
ragionamento per turno, che a 7 t/s significa due minuti di attesa. Per questo
il server parte con `--reasoning-budget 512`. Alzalo in
`server.extra_args` se preferisci risposte piu' ragionate e piu' lente,
mettilo a `0` per disattivare del tutto il ragionamento.

---

## Tre cervelli intercambiabili

Quello che *pensa* sta dietro l'astrazione `nova/brains`. Si cambia a caldo
dal menu **Cervello** in alto, senza perdere la conversazione ne' la memoria.

| Cervello | Cos'e' | Agentico |
|---|---|---|
| `locale` | il GGUF servito da llama-server sul tuo PC | no |
| `claude` | Claude Code CLI in headless | si' |
| `api` | qualunque endpoint OpenAI-compatibile | no |

**Agentico** e' la differenza che conta. `locale` e `api` *propongono* tool
call e NOVA li esegue applicando guardie e livelli di autonomia. `claude` ha
mani proprie: NOVA gli fa da tramite, gli passa il contesto e la memoria, e
riporta cosa ha fatto, in quanti turni e quanto e' costato.

```powershell
python -m nova --brains                    # chi c'e' e chi e' pronto
python -m nova --brain claude              # cambia e avvia
python -m nova --brain claude --ask "..."  # una richiesta sola
```

### Claude Code come cervello

Serve `npm install -g @anthropic-ai/claude-code` e un `claude` gia'
autenticato. NOVA:

- lo lancia in headless (`-p --output-format json`), prompt via stdin
- mantiene la sessione fra un turno e l'altro con `--resume <session_id>`
- traduce i **tuoi** livelli di autonomia nei suoi permessi:

  | Autonomia NOVA | `--permission-mode` |
  |---|---|
  | Conferma sempre | `plan` (analizza e propone, non tocca nulla) |
  | Conferma azioni rischiose | `acceptEdits` |
  | Autonomo | `bypassPermissions` |

- gli espone la memoria a grafo come **server MCP** (`nova/mcp_kb.py`), quindi
  Claude usa `mcp__nova__kb_search` e `mcp__nova__kb_note`: stessa pipeline di
  retrieval del modello locale, stesso formato dei nodi. Se l'MCP non parte,
  ricade sulla lettura diretta dei file .md del vault.
- riporta costo e token di ogni turno nel registro azioni.

Attenzione a `brains.claude_model`: l'alias `opus` su CLI datate punta a
`claude-opus-4-1`, che e' stato ritirato e risponde 404. Il default e'
`sonnet`, che funziona.

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

Da quando c'e' `core/` (vedi `core/README.md`), NOVA non genera piu' llama-server
come processo figlio: lo affida a **nova-core**, che lo supervisiona.

```
llama-server pid 2760 -> padre: novad
```

Conseguenze pratiche:

| | prima | adesso |
|---|---|---|
| Chiudi la finestra | il modello si scarica | resta caricato |
| Riapri NOVA | ~2 minuti di caricamento | **2 secondi** |
| Il server cade | resta giu' | il demone lo rialza |
| Log del modello | file che nessuno legge | eventi `proc.output` sul bus, piu' buffer circolare |

All'avvio NOVA prova nella sequenza: *il demone possiede gia' il modello?* →
lo **adotta** (recupera anche con quanti `-ngl` era partito, chiedendolo al
demone); *c'e' qualcosa sulla porta?* → lo riusa; altrimenti chiede a nova-core
di avviarlo, e solo se il demone manca ricade sul vecchio processo figlio. Se
`core/` non e' compilato NOVA funziona esattamente come prima.

Chiavi in `server` di `config.json`:

| Chiave | Default | Cosa fa |
|---|---|---|
| `use_daemon` | `true` | affida il modello a nova-core |
| `daemon_autostart` | `true` | accende nova-core se non gira |
| `stop_model_on_exit` | `false` | chiudere NOVA **non** scarica il modello |

La finestra si sottoscrive a `proc.*` e mostra nel registro azioni i log del
modello presi dal bus, non piu' da un processo che possiede lei.

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
| `standard` | Claude Code | `sonnet` | il cavallo da lavoro |
| `difficile` | Claude Code | `claude-opus-4-5-20251101` | quando il compito lo merita |
| `alternativo` | Gemini CLI | `gemini-2.5-pro` | seconda opinione |

```powershell
python -m nova --modelli     # gradini, stato, speso / tetto
```

### Come passa la palla

Tre strade, in ordine di intelligenza:

1. **`delega`** — il modello sceglie. Scrive il compito per intero (chi lo
   riceve non vede la conversazione) e passa i **percorsi** dei file in `file`:
   li allega NOVA, gratis. Poi riprende lui con la risposta.
2. **Escalation automatica** — se NOVA sbaglia due volte di fila, o fa sei
   chiamate senza arrivare a una risposta, sale di gradino da sola e infila il
   risultato nella conversazione. Sono due modi diversi di non farcela:
   sbattere contro un muro, e girare a vuoto.
3. **`secondo_parere`** — la stessa domanda a due gradini, per confrontare.

### Guardie

| Chiave (`brains.routing`) | Default | Cosa fa |
|---|---|---|
| `orchestratore` | `locale` | chi guida la conversazione |
| `escalation_automatica` | `true` | sale da sola quando serve |
| `fallimenti_prima_di_salire` | `2` | tentativi andati male |
| `passi_prima_di_salire` | `6` | chiamate senza risposta |
| `salite_massime` | `1` | quante volte per turno |
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

C'è un tool `screenshot`, e serve per le domande sull'aspetto delle cose
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

* locale       locale   predefinito                locale       pronto
  standard     claude   sonnet                     abbonamento  pronto
  difficile    claude   claude-opus-4-5-...        abbonamento  pronto
  alternativo  gemini   gemini-2.5-pro             incluso      pronto
```

### Quando finisce la quota

Con l'abbonamento il vincolo vero non sono i soldi, sono i **limiti d'uso**. È
una cosa diversa da un errore: non vuol dire «non ci riesco», vuol dire
«riprova più tardi». NOVA la tratta come tale:

1. riconosce il messaggio di quota esaurita (`usage limit`, `rate limit`, 429, …)
   e solleva `LimiteUso`, non un errore generico;
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
