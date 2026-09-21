# Le prove

Centodieci file, in cinque cartelle. La cartella dice **cosa serve per
farle girare**, che e' l'unica domanda che ci si pone davanti a una prova
che non si conosce.

| cartella | quante | cosa serve | se non c'e' |
| --- | ---: | --- | --- |
| `gemelli/` | 23 | un banco Rust costruito con `cargo` | esce 2 e stampa la riga per costruirlo |
| `demone/` | 5 | il binario `novad` costruito | esce 2 |
| `macchina/` | 18 | una macchina vera: Windows, uno schermo, l'audio, Chrome | esce 2 |
| `progetto/` | 26 | niente: guarda il repository stesso | — |
| `nova/` | 38 | niente: sono prove di unita' in puro Python | — |

**Uscita 0 passata, 1 rossa, 2 «qui non si puo' fare».** Il 2 non e' un
fallimento ed e' importante che resti distinto: una prova che non si puo'
eseguire su questa macchina non ha trovato niente di rotto, e chiamarla rossa
insegnerebbe a ignorare i rossi.

Ogni prova e' **un programma**, non un caso di `pytest`: si lancia da sola,
stampa cosa controlla, e finisce con un codice di uscita.

```bash
python prove/nova/test_taglio.py          # una sola
for f in prove/*/test_*.py; do python "$f"; done   # tutte
```

Si lanciano **dalla radice del repository**. Ognuna si calcola la radice da
se' (`Path(__file__).resolve().parents[2]`) e se la mette nel percorso di
ricerca, quindi funzionano anche da altrove — ma i banchi e i binari si
cercano rispetto alla radice, e quella deve essere giusta.

## I gruppi, in dettaglio

- **`gemelli/`** — mettono la meta' Python e la meta' Rust una di fronte
  all'altra sullo stesso ingresso e pretendono la stessa uscita. Sono il modo
  in cui questo progetto porta il codice da un linguaggio all'altro senza
  fidarsi. Il banco si costruisce con la riga che la prova stessa stampa
  quando non lo trova.
- **`demone/`** — accendono `novad` vero e gli parlano via RPC. Provano il
  giro intero, non le funzioni.
- **`macchina/`** — chiedono qualcosa che qui non c'e': il registro di
  Windows, una finestra, il mixer audio, l'utilita' di pianificazione, una
  scheda video. Nella CI escono 2 quasi sempre; su una macchina vera no.
- **`progetto/`** — non provano NOVA, provano il repository: che i documenti
  dicano quello che il codice fa, che un elenco non esista in due copie, che
  nessun dato personale sia finito in un commit, che ogni crate sia raggiunto
  da qualcosa o dichiari perche' no.
- **`nova/`** — tutto il resto. Puro Python, nessun ambiente speciale.

`test_prove_ordinate.py`, in `progetto/`, tiene questa regola: una prova
rimessa in radice o in una cartella non dichiarata fa rosso li' invece di
sparire da tutti i giri della CI in silenzio.

## Dove sta il resto

- `attrezzi/` — gli script che si lanciano **a mano**: quelli che hanno
  estratto verso Rust i testi del prompt, le guardie, le entita' HTML, e i
  giri di mutazione con cui si verifica che un banco gemello guardi davvero.
- `misure/` — i banchi di **prestazione**: quanto costa un turno, quanto
  costa tagliare la conversazione, se un modello piu' piccolo sa ancora
  scegliere lo strumento giusto fra sessanta.
