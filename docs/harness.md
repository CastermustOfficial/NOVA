# L'harness: cosa deve avere

Deciso con Gio il 26 settembre 2026, **prima** di pensare a come renderlo
bello. Questo file dice cosa c'è dentro; l'aspetto è quello della bozza
approvata, [`harness_bozza.html`](harness_bozza.html).

## A cosa serve

Due lavori, da fare anche insieme nella stessa finestra:

1. **Documenti**: studiarli, modificarli, chiederci sopra. Un documento di
   testo è un file come gli altri: si apre, si legge, si scrive.
2. **Programmare**, come in un IDE con un assistente: l'editor, i file del
   progetto, i test, il terminale — e NOVA che lavora dentro, quanto Claude
   Code.

La chat sta lì dentro, per comodità. È la stessa conversazione dell'orb, non
una seconda.

## La forma

| Zona | Cosa c'è |
| --- | --- |
| **Sinistra** | quattro viste: *Esplora* (l'albero della cartella), *Cerca* (in tutti i file), *Modifiche* (le proposte di NOVA in attesa), *Test* |
| **Centro** | l'editor a **schede**, che si divide in due: un PDF accanto al codice, un `.md` accanto alla sua anteprima |
| **Destra** | la chat di NOVA, che sa cosa è aperto e cosa è selezionato; `@file` per passarle altri file |
| **Sotto** | il **terminale**, e l'uscita dei test |

## I file

| Tipo | Come si vede | Chi lo modifica |
| --- | --- | --- |
| Codice | Monaco (l'editor di VS Code): colori, numeri di riga, cerca e sostituisci, il confronto fra due versioni | Gio e NOVA |
| Markdown, testo | un foglio scrivibile, con l'anteprima accanto o a scomparsa | Gio e NOVA |
| Word | il documento, paragrafo per paragrafo | Gio e NOVA (se glielo chiede) |
| PDF | le pagine vere (pdf.js), la parola evidenziata nel punto giusto, la selezione per chiedere | nessuno: si legge |
| HTML | disegnato, e il sorgente con un clic | sul sorgente |
| Immagini | si guardano | — |

Monaco, pdf.js e il terminale (xterm.js) girano nella finestra web del
guscio: niente da compilare, e niente `mupdf`.

**Word, detto prima di farlo.** Il file si salva con la chirurgia di
`nova-docx`: si tocca solo il paragrafo cambiato, e tutto il resto — stili,
temi, immagini — resta byte per byte. Ma dentro un paragrafo cambiato il
testo nuovo va nella prima porzione (D276): se in quel paragrafo c'era una
parola in grassetto, dopo la modifica non lo è più. I paragrafi non toccati
restano come erano. Si può fare meglio — seguire le porzioni mentre si
scrive — e sta nella lista del «dopo».

## NOVA dentro l'harness

- **Chiede e risponde** sulla selezione, sul file, sul progetto.
- **Le risposte puntano al posto**: «pagina 12», «riga 40» si cliccano, e
  il documento ci va.
- **Lavora quanto Claude Code**: apre file nell'harness, cerca, legge,
  propone modifiche su più file di fila, esegue i test e i comandi. Ogni
  comando e ogni modifica passano dal cancello dei permessi (D333) con il
  livello di autonomia del pannello.
- **Le modifiche si vedono prima**: come confronto dentro l'editor, da
  accettare o rifiutare pezzo per pezzo — o tutte insieme — e da ritoccare
  prima di accettarle. Non si applicano a un file cambiato sotto (D273).
- **Applica e prova**: la modifica resta solo se i test non peggiorano
  (D277, D278).
- **Il terminale è di Gio**: NOVA non ci scrive dentro. I comandi di NOVA
  passano da `shell.exec` col permesso, e la loro uscita si vede nel
  pannello, accanto al terminale, perché si sappia cosa ha fatto.

**Non c'è** il completamento mentre si scrive (il testo grigio da accettare
con Tab): scelto di no.

## Riaprire e ritrovare

Chiusa e riaperta, l'harness ritrova i file aperti, le schede, il punto in
cui si era, e le proposte in sospeso. Il registro della sessione è su disco,
in aggiunta, come quello del Python.

## In che ordine

1. La finestra: *Esplora*, l'editor a schede con codice, Markdown e testo,
   il salvataggio, la chat a destra che sa cosa è aperto e selezionato, e
   NOVA che apre file nell'harness.
2. Le proposte su più file, il confronto, *Modifiche*; i test e «applica e
   prova».
3. Il terminale.
4. PDF e Word.
5. *Cerca* nel progetto, e la sessione che si ritrova.

## Com'è andata

**Fase 1 — fatta** (D336, D337). La finestra `harness` del guscio
(`nova-shell/ui/harness.html`, il Rust in `nova-shell/src/harness.rs`):

- *Esplora* legge la cartella un livello per volta, e non mostra le cartelle
  che l'harness di NOVA non guarda (`target`, `node_modules`, `.git`…).
- L'editor è Monaco, scaricato alla compilazione e non tenuto in git (D336);
  senza rete resta un foglio semplice, e la finestra lo dice. Il Markdown ha
  l'anteprima accanto.
- Salvare non scrive sopra a un file cambiato sul disco dopo averlo aperto:
  chiede. Un file cambiato da fuori — da NOVA, per esempio — si ricarica da
  solo se non ci sono modifiche, altrimenti si chiede quale versione tenere.
  Il segno UTF-8 in testa e gli a capo di Windows restano com'erano.
- La chat è la stessa dell'orb: quel che si scrive in una finestra compare
  nell'altra. Con la domanda va cosa è aperto e cosa è selezionato, in coda,
  come la postilla della voce (`nova_harness::aperti`). I pezzetti sopra al
  campo dicono cosa parte, e si tolgono con un clic.
- `main.rs:40` e «riga 40» nelle risposte si cliccano.
- **NOVA apre file qui** con lo strumento che ha già, `harness_apri`: il
  guscio si dichiara finestra dell'harness e ne segue il puntatore (D337).
  PDF, Word e HTML aperti da NOVA vanno ancora alla finestra di prima (in
  Qt), che li mostra con le pagine vere: il guscio la accende da sé. Qui
  arrivano con la quarta fase.
- Chiusa, la finestra si nasconde e ritrova tutto; riaperto il guscio,
  ritrova la cartella, le schede e le cartelle espanse.

**Fase 2 — fatta** (D339). Le proposte di NOVA si guardano qui:

- Quando NOVA propone, la finestra si apre sul confronto: dentro l'editor,
  in linea o affiancato, con quel che se ne va in rosso e quel che arriva in
  verde. Nella chat compare la carta delle proposte, con i file e le righe
  che cambiano, e i bottoni per decidere senza lasciare la conversazione.
- *Modifiche* le elenca tutte, anche su più file; il piede dice quante ne
  aspettano.
- Si accettano intere o **a pezzi**: «Rifiuta questo pezzo» (o la freccia
  nel margine) rimette le righe di prima, il resto resta. Il lato destro si
  scrive: ritoccare prima di accettare è scrivere lì.
- **Applica e prova**: prova il progetto prima e dopo, e se cade un test che
  passava rimette tutto com'era e la proposta resta. L'uscita si legge nel
  pannello di sotto, un pezzo alla volta mentre gira.
- *Test* prova il progetto da solo, riconoscendo come: cargo, npm, go,
  pytest, o gli script di prova.
- Una proposta su un file cambiato dopo che l'hai guardata non si scrive:
  si ricarica il confronto. Accanto a ogni file scritto resta la copia
  `.prima`.
- Le proposte su PDF e Word si guardano ancora nella finestra di prima.

**Fase 3 — fatta** (D340). Il pannello di sotto ha tre linguette:

- **Terminale**: una console vera, nella cartella del progetto, con la
  shell di tutti i giorni. Se ne aprono quante se ne vuole (+), e Ctrl+ò lo
  apre e lo chiude. Dentro, i tasti sono della shell.
- **Test**: l'uscita delle prove, un pezzo alla volta.
- **Comandi di NOVA**: quel che NOVA ha eseguito, dove, con che esito e
  cosa ha scritto. Nel terminale scrive solo Gio.

**Fase 4, prima metà — fatta** (D341):

- **PDF**: le pagine vere, disegnate man mano che si scorre, col testo che
  si seleziona; pagina, zoom, «adatta alla larghezza». I punti che NOVA ha
  appena indicato si accendono sulla pagina, e la pagina ci scende sopra.
  «pagina 12» nelle risposte si clicca.
- **Word**: il documento paragrafo per paragrafo, da scrivere: Invio fa un
  paragrafo nuovo, Backspace su un paragrafo vuoto lo toglie, Ctrl+S salva.
  Si toccano solo i paragrafi cambiati (vedi sopra per il grassetto). Le
  tabelle per ora si leggono.
- **HTML**: disegnato accanto al sorgente; si ridisegna quando si salva.
- **Immagini**: si guardano.
- La selezione in un PDF o in un Word va con la domanda, con la pagina.

**Fase 4, seconda metà — fatta** (D342):

- Le proposte di NOVA su un **Word** o un **PDF** si guardano qui, **voce
  per voce**: cosa c'era e cosa diventa, il paragrafo aggiunto con quello
  accanto, il pezzo di pagina da evidenziare, la nota. Ogni voce ha la sua
  casella; «Vedi nel documento» apre il PDF alla pagina, col riquadro dove
  andrebbe il segno, o il Word al paragrafo.
- Le scrive il demone. Nel Word si toccano solo i paragrafi e le righe di
  tabella della proposta; nel PDF le evidenziazioni e le note vanno **in
  coda al file**, e quel che c'era resta byte per byte. Accanto resta la
  copia `.prima`.
- Una voce su un paragrafo cambiato dopo la proposta si spegne, e dice
  perché; le altre si possono scegliere lo stesso.
- Le note gialle di un PDF si vedono sulla pagina, col testo passandoci
  sopra.
- **La finestra Qt non c'è più.** Quando NOVA apre un documento e il
  guscio è spento, lo strumento accende il guscio con `--harness`, che
  mostra subito il documento. Con lei se ne vanno PyQt6, PyQt6-WebEngine e
  Pygments.

**Fase 5 — fatta** (D343):

- ***Cerca*** (Ctrl+Maiusc+F): una stringa in tutti i file della cartella,
  con maiuscole distinte, parola intera, espressione regolare — e anche
  dentro PDF e Word, dove il posto è la pagina o il paragrafo. Si guarda
  quel che guarda *Esplora*. Un clic sul risultato apre il file sul punto,
  con la parola selezionata. Quel che è selezionato nell'editor diventa la
  domanda.
- **Riaprire e ritrovare**: il ricordo sta sul disco, accanto alle sessioni
  di NOVA (`schede.json`): la cartella, le schede e il punto in ciascuna (la
  riga e lo scorrimento, la pagina e lo zoom del PDF), la vista a sinistra,
  la ricerca, il pannello di sotto. E le **bozze**: un file cambiato e non
  salvato torna cambiato anche dopo aver chiuso il guscio; se nel frattempo
  il file sul disco è cambiato, si chiede quale tenere.
- Se NOVA apre un file mentre la finestra riparte, si apre **sopra** a
  quel che c'era, invece di prenderne il posto.
- Il diario della finestra (`schede.jsonl`): cosa si è aperto, salvato,
  chiuso, applicato, scartato.
- Le proposte in sospeso c'erano già: stanno nei loro file.

Con questo le cinque fasi sono fatte.

**E gli strumenti di NOVA** (D344): `harness_apri`, `harness_cerca` e gli
altri sette sono nel demone, e li ha ogni cervello, non solo Claude Code. I
bottoni della finestra si chiamano `finestra.*`.

**I Dot** (D391, D392): una quinta vista a sinistra (Ctrl+Maiusc+D), fatta
come Teams: le chat (con ogni Dot, i gruppi, quelle fra di loro), poi
l'organigramma, che si vede anche come schema, e i file che i Dot stanno
toccando. Ogni conversazione si apre in una scheda al centro; quella con un
Dot ha anche i compiti e i loro passi. Il disegno e com'e' andata stanno in
[`dots.md`](dots.md).

## Dopo

- Le porzioni di Word seguite mentre si scrive, così il grassetto dentro un
  paragrafo cambiato resta.
- Evidenziare e annotare un PDF a mano, non solo accettando quel che propone
  NOVA.
- Git: cosa è cambiato dall'ultimo commit.
