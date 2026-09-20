# Il banco dei documenti

La domanda di CANT-8 e CANT-10 e' una sola: **quali librerie Rust reggono i
documenti veri**. Non si risponde leggendo la documentazione — si costruiscono
tre documenti fatti apposta per essere difficili e si guarda cosa resta dopo.

Non sta nel workspace di proposito: `mupdf-sys` compila MuPDF **dai sorgenti
C**, e sono minuti. Farlo pagare a ogni `cargo test` per una decisione gia'
presa sarebbe il genere di costo che poi si toglie di mezzo spegnendo qualcosa.

## Come si rifa'

```bash
python3 banco_documenti/prepara.py     # costruisce i tre documenti
cd banco_documenti && cargo run --bin fogli
cd banco_documenti && cargo run --bin chirurgia && python3 verifica_docx.py
```

## Cosa e' venuto fuori

| Domanda | Risposta | Come si e' vista |
|---|---|---|
| Leggere un `.xlsx` con formule e formati | `umya-spreadsheet` | formula `C2-B2`, formato `0.0%`, grassetto e secondo foglio **sopravvivono** al giro leggi-tocca-riscrivi |
| Modificare un `.docx` senza spogliarlo | **niente libreria**: zip + una sostituzione | `docx-rs` perde `theme1.xml`, `customXml/`, `webSettings.xml` e lo stile `Normal`, e il file passa da 37 a 105 kB. Aprendo lo zip e toccando **solo** `word/document.xml`: zero parti perse, zero aggiunte, una cambiata |
| Sapere **dove** sta il testo in un PDF | `mupdf` | `pdf-extract` da' le pagine ma **mescola le due colonne nella stessa riga**; `mupdf` da' blocchi con le coordinate, e le colonne restano a x=72 e x=320 |
