# analisi/

Ogni passaggio confermato, spiegato nel dettaglio: cosa si è deciso, su quali prove e con quali numeri.

In NOVA l'analisi sta in tre documenti, più vecchi di questa cartella. Non si spostano: spostarli romperebbe i rimandi dal README, dal diario e dalle prove.

- [`docs/architettura.md`](../docs/architettura.md) raccoglie le decisioni, numerate da D1 in su. Ogni riga dice cosa si è deciso e perché; le più recenti dicono anche su quali prove e cosa si è scartato. È il posto dove si guarda per primo.
- [`docs/diario.md`](../docs/diario.md) racconta cosa si è scoperto **mentre** si faceva, giorno per giorno: le cose che un messaggio di commit non dice.
- [`docs/harness.md`](../docs/harness.md) descrive nel dettaglio l'harness, deciso con Gio il 26 settembre: a cosa serve, com'è fatta la finestra, i file, NOVA dentro l'harness, e com'è andata.

Un passaggio si considera confermato quando ha la sua riga in `architettura.md` e la prova che lo tiene è verde in CI. Le voci nuove si scrivono nello stesso commit del lavoro.
