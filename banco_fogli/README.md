# Il banco dei fogli

La domanda di CANT-10, quella rimasta aperta dopo aver scelto
`umya-spreadsheet`: **chi sa calcolare le formule, e a che prezzo?**

Nessuna libreria di `.xlsx` calcola niente — non `umya`, non `openpyxl`, non
`calamine`. Leggono il risultato che l'ultimo programma ha lasciato in cache,
e quella cache e' **vuota** in ogni file scritto da un programma invece che
da Excel. Cioe': NOVA scrive `=SUM(A1:A10)`, rilegge, e vede la formula al
posto del numero. Chi legge quel file con un altro programma vede il vuoto.
E' lo stesso difetto che D253 ha chiuso in lettura, visto dal lato di chi
scrive.

Non sta nel workspace di proposito, come `banco_documenti`: `formualizer` si
porta dietro Arrow e mezzo mondo, e sono minuti di compilazione. Un banco che
si usa quando si decide non deve pesare su ogni `cargo test` di chi non sta
decidendo niente.

## Come si rifa'

```bash
python3 banco_fogli/prepara.py          # costruisce i quattro fogli
cd banco_fogli && cargo run --bin ricalcolo
python3 banco_fogli/verifica.py         # fa girare anche LibreOffice e conta
```

`verifica.py` usa `recalc.py` della skill `xlsx` di Anthropic per la strada
LibreOffice. Se non c'e', quella colonna resta vuota e il resto funziona.

## Cosa e' venuto fuori

Misurato il 20 settembre. I valori calcolati sono **identici** dalle due
parti dove tutte e due ce la fanno: la differenza non e' nei numeri.

| Foglio | formualizer (Rust) | LibreOffice in silenzio |
|---|---|---|
| `conti` — 6 formule, grassetto, formato %, nota, 2° foglio | tutto giusto, **1 parte cambiata su 13** | tutto giusto, **2 parti perse, 4 aggiunte, 11 su 11 cambiate** |
| `moderne` — XLOOKUP, IFS, TEXTJOIN, MAXIFS, e una inventata | XLOOKUP **calcolato**, la inventata dichiarata come errore | XLOOKUP diventa `#NAME?` **dentro il file**, e anche la inventata |
| `spandono` — FILTER, UNIQUE, SORT | **rifiuta il foglio** e non scrive niente | tutte e tre diventano `#NAME?` dentro il file |
| `grande` — 4.002 formule, grafico, formattazione condizionale | 0,66 s (build di debug), 1 parte su 13 | 1,8 s, 2 aggiunte, 13 su 13 cambiate |

Il grafico e la formattazione condizionale sopravvivono dalle due parti.

## Cosa vuol dire

**La differenza non e' il calcolo, e' cosa resta del foglio di qualcuno.**
LibreOffice riscrive l'archivio intero: rinomina le parti dei commenti,
aggiunge un `theme1.xml`, tocca tutto. E' esattamente il comportamento che
D237 ha rifiutato per il `.docx` — ricostruire invece di toccare — trovato
una seconda volta su un formato diverso. Su un foglio fatto da NOVA non si
nota. Su un modello finanziario di qualcuno, con dentro cose che nessuno dei
due programmi conosce, e' il modo di perderle.

**I due modi di non farcela non sono lo stesso.** Su una funzione che non sa
calcolare, LibreOffice scrive `#NAME?` **nel file consegnato**: un errore
che sembra un dato, e che chi apre il foglio deve accorgersi da se' che non
c'era prima. `formualizer` invece dichiara l'errore nel suo rendiconto, e
davanti a una cosa che non sa fare affatto — le formule che spandono su piu'
celle — **rifiuta l'intero foglio senza scriverlo**. Per NOVA e' il verso
giusto: e' la stessa frase di D259 sul Cestino («qui l'unica alternativa e'
distruggere, e va detta, non fatta») e di D273 sul file cambiato sotto.

**Il prezzo di LibreOffice non e' solo quello.** Sono qualche centinaio di
megabyte installati sul PC di qualcuno per calcolare una somma, un processo
esterno da avviare, e una macro StarBasic da infilare nel profilo utente —
e' cosi' che lo fa la skill `xlsx`, e non c'e' un modo piu' pulito.

**E resta un buco che nessuno dei due copre**: FILTER, UNIQUE e SORT. Sono
le funzioni che spandono, e un `.xlsx` scritto da fuori non ha i metadati
dello spandimento. Non e' un difetto di queste librerie: e' che quelle
formule, scritte da un programma, non sono un foglio valido finche' Excel
non le apre.

## Dove si rischia di sbagliare

Tre cose che non si vedono in questa tabella e che valgono quanto lei:

- **Un ricalcolo verde non vuol dire che le formule sono giuste.** Un
  intervallo spostato di una riga da' un file pulito con dentro numeri
  sbagliati. Il verde dice «si calcola», non «e' corretto».
- **Un foglio che punta a un altro file perde i collegamenti** se lo si
  riscrive e poi lo si ricalcola: la formula dice `='[1]Riepilogo'!$B$2`, il
  file `[1]` non c'e', e il valore in cache era l'unica cosa che teneva su
  quel dato. Lo racconta la skill `xlsx` di Anthropic, che si rifiuta di
  ricalcolare in quello stato.
- **`formualizer` e' alla versione 0.9**, di un autore solo. Sta andando
  forte e le misure qui sopra sono sue, ma legarsi a un motore giovane per
  una cosa che tocca i file di qualcuno vuole una via di fuga: il motore va
  dietro un'interfaccia, con LibreOffice come seconda strada — che e'
  esattamente quel che fa `spreadsheet-mcp`, l'unico altro progetto che ha
  affrontato lo stesso problema per un agente.
