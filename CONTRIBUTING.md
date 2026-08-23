# Contribuire a NOVA

Grazie: un progetto che tocca il sistema operativo di chi lo installa vive di
occhi in piu'.

## La cosa piu' utile che puoi fare

**Aprire una issue quando qualcosa si rompe.** NOVA e' in alpha e gira su
hardware molto diverso dal mio. Un rapporto che dice *quale scheda madre,
quale versione di Windows, quale cervello stavi usando e cosa e' successo*
vale piu' di dieci righe di codice.

## Prima di aprire una pull request

Leggi il [documento di architettura](docs/architettura.md). Non e' burocrazia:
contiene le decisioni gia' prese **con il loro perche'**, comprese quelle
scartate. Se una tua proposta va contro una di quelle, va benissimo — ma
parliamone nella issue prima che tu scriva il codice, cosi' non butti tempo.

## Regole che non si negoziano

Sono poche e vengono tutte dallo stesso principio: NOVA gira sul computer di
qualcun altro, con i suoi dati dentro.

1. **Niente dati personali nel repository.** Ne' nei file, ne' nei test, ne'
   nei messaggi di commit. C'e' un controllo automatico in CI, ma non fidarti
   solo di quello.
2. **L'installer non chiede esclusioni antivirus.** Mai. E' il gesto che
   distingue un programma onesto da un malware, e all'installazione la fiducia
   dell'utente e' zero.
3. **Il predefinito e' cauto.** Ogni funzione nuova che tocca il sistema parte
   sotto conferma. Il confine e' una manopola dell'utente, non una decisione
   nostra.
4. **Le richieste di permesso dicono cosa succede.** «Sto per eliminare 4 file
   in Download: [elenco]» e non «consentire operazione su file?». Una domanda
   generica fa cliccare si' a occhi chiusi, e a quel punto il presidio e'
   sparito pur sembrando in piedi.
5. **Una funzione che dipende da un permesso deve poter mancare.** Si rileva
   la capacita', si offre solo cio' che esiste, e senza il permesso il resto
   continua a funzionare.

## Stile

Il codice e i commenti sono **in italiano**. I commenti spiegano *perche'*,
non *cosa*: il cosa si legge dal codice.

```powershell
.\build.ps1 -Test      # test Rust
python -m pytest -q    # test Python
```

Fai passare i test prima di aprire la PR. Se ne rompi uno di proposito perche'
il comportamento vecchio era sbagliato, dillo nella descrizione.

## Sicurezza

Se trovi una falla che mette a rischio i dati di chi usa NOVA — l'archivio
credenziali, la memoria, l'esecuzione di comandi — **non aprire una issue
pubblica**. Scrivi in privato al proprietario del repository.