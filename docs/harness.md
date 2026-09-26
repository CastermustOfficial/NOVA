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
  Qt), che li mostra con le pagine vere: il guscio la accende da sé. La
  accende anche quando NOVA propone una modifica, perché il confronto da
  accettare sta ancora lì. Qui arrivano con la seconda e la quarta fase.
- Chiusa, la finestra si nasconde e ritrova tutto; riaperto il guscio,
  ritrova la cartella, le schede e le cartelle espanse.

## Dopo

- Le porzioni di Word seguite mentre si scrive, così il grassetto dentro un
  paragrafo cambiato resta.
- Le note e le evidenziazioni che restano su un PDF.
- Git: cosa è cambiato dall'ultimo commit.
