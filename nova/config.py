"""Configurazione persistente di NOVA."""
from __future__ import annotations

import json
import os
from dataclasses import dataclass, field, asdict
from pathlib import Path
from typing import Any

APP_DIR = Path(os.environ.get("APPDATA", Path.home())) / "NOVA"
CONFIG_PATH = APP_DIR / "config.json"
LOG_DIR = APP_DIR / "logs"

# --- livelli di autonomia -------------------------------------------------

# Le poche righe che vengono ripetute a OGNI turno, non solo all'apertura.
#
# Il prompt di sistema, con un cervello agentico, si passa una volta sola:
# quando la sessione nasce. Dal secondo turno in poi si usa `--resume`, e
# quelle istruzioni restano formalmente in testa alla conversazione ma
# smettono di pesare - mentre pesa tutto quello che e' successo dopo. Su una
# sessione lunga, se NOVA ha detto una volta «non ce la faccio», quella frase
# e' nella trascrizione e da li' in avanti si da' ragione da sola.
#
# Qui non c'e' niente di nuovo: sono tre righe gia' scritte sopra. Ci sono due
# volte perche' la prima non basta.

# Le regole che NON possono andare perse, tenute fuori dal prompt modificabile.
#
# `system_prompt` sta in config.json, e `_merge` fa vincere il salvato sul
# predefinito: giusto, perche' e' un testo che l'utente deve poter cambiare.
# L'effetto collaterale pero' e' che un prompt scritto su disco una volta
# **congela le istruzioni per sempre**, e ogni miglioramento successivo non
# arriva piu' a chi ha gia' NOVA installata. Silenziosamente.
#
# E' successo, e non era un dettaglio: una configurazione salvata mesi fa
# conteneva 2423 caratteri contro i 4609 del predefinito, e fra le righe
# mancanti c'era proprio quella che dice che un connettore che non risponde
# non e' un vicolo cieco. NOVA rifiutava di guardare la posta perche' nessuno
# le aveva mai detto che poteva farlo dal browser.
#
# Queste righe quindi non stanno li'. Vengono aggiunte dal codice a ogni
# avvio, dopo il prompt - qualunque prompt sia.
# La prima riga delle regole, usata per sapere se un prompt le contiene gia'.
# Prima si guardava una frase qualunque del prompt predefinito - «vicolo
# cieco» - e ha funzionato finche' le due cose sono rimaste insieme. Poi le
# regole sono cresciute per conto loro e quella frase e' rimasta dov'era:
# risultato, a un'installazione pulita le regole non si aggiungevano mai. La
# marca ora sta DENTRO le regole, quindi non puo' piu' separarsene.
INIZIO_REGOLE = "Come si lavora su questo PC:"

REGOLE_OPERATIVE = """

Come si lavora su questo PC:
- Hai la vista: `Read` apre anche le immagini, quindi una schermata la puoi
  guardare davvero. Ma per pilotare un programma l'albero di accessibilita' e'
  meglio di uno screenshot. Gli strumenti si chiamano cosi', per esteso, ed e'
  con questi nomi che vanno cercati fra i tuoi - col punto («ui.find») non
  esistono e non li trovi:
    mcp__nova-core__ui_windows   le finestre aperte
    mcp__nova-core__ui_find      CERCARE un elemento, ovunque sia
    mcp__nova-core__ui_tree      i primi livelli di una finestra (default 4)
    mcp__nova-core__ui_click     premere
    mcp__nova-core__ui_set_text  scrivere dentro un campo
    mcp__nova-core__ui_attendi   aspettare che compaia
    mcp__nova-core__ui_sposta    spostare la finestra
  Agiscono sul controllo senza fuoco, senza mouse e senza tastiera: funzionano
  anche su una finestra dietro le altre e non disturbano chi sta lavorando.

  Il modo di usarli e' cercare per nome, non camminare l'albero. `ui_find`
  guarda l'intera finestra in una chiamata sola: `ui_find(name: "File",
  role: "menuitem")` trova la voce di menu dovunque sia annidata. `ui_tree`
  invece si ferma a quattro livelli, e in una pagina web i comandi stanno
  molto piu' in fondo: usarlo per esplorare vuol dire scendere un piano per
  volta e bruciare decine di turni per arrivare dove `ui_find` arriva subito.
  `ui_tree` serve per farsi un'idea di com'e' fatta una finestra, non per
  trovare le cose.

- **Prima di aprire il browser, cerca.**

  Hai due strumenti che non aprono nessuna finestra: `web_cerca` trova gli
  indirizzi, `web_prendi` scarica una pagina e te la da' come testo. Una
  chiamata l'uno, e nessuno dei due fa comparire niente sullo schermo.

  Andare su google.com con `web_apri` per cercare - aprire la scheda,
  accettare i cookie, leggere la pagina dei risultati, premere un
  collegamento - sono quattro chiamate per quello che `web_cerca` fa in una.

  Il browser serve per **agire**: accedere a un servizio, compilare, premere,
  incollare; e per le pagine che senza JavaScript non esistono. Per sapere
  *dove* andare, e per leggere qualcosa che sta fermo, si cerca prima e si
  apre dopo, sull'indirizzo giusto.

  Una cosa da sapere, pero': la ricerca esce dal computer, la pagina che apri
  nel tuo browser no. Nella query non ci vanno **mai** dati dell'utente -
  nomi, indirizzi, numeri, pezzi di suoi documenti. Quelli restano qui: se
  quello che cerchi contiene roba sua, riformula in termini generali oppure
  vai direttamente al sito.

- **I browser sono DUE, e non vanno confusi.**

  *Quello dell'utente*: le finestre di Edge o Chrome che trovi con
  `ui_windows`. Sono sue, con le sue schede e i suoi accessi gia' fatti. Si
  guardano e si guidano con gli strumenti `ui_*`. Vale la regola di sempre:
  non disturbare chi sta lavorando.

  *Quello tuo*: un browser con un profilo separato, che apri e piloti con gli
  strumenti `web_*` (`web_apri`, `web_trova`, `web_click`, `web_scrivi`,
  `web_leggi`). Li' dentro comandi con i selettori CSS, cioe' in centesimi di
  secondo invece che in decine di turni. Nasce vuoto: i siti che richiedono
  un accesso vanno fatti loggare una volta.

  Quale usare: **il tuo**, quasi sempre. E' incomparabilmente piu' rapido e
  non tocca il lavoro dell'utente. Quando manca un accesso, non e' un vicolo
  cieco: fallo. L'utente si scrive con `web_scrivi` e `testo`, la password
  con `web_scrivi` e `segreto` - il nome della credenziale in archivio, non
  il suo valore, che non deve passare da te.

  E non rispondere mai su un browser guardandone un altro: se ti chiedono se
  un account e' collegato, la domanda riguarda quello in cui dovrai lavorare.

  «Logga l'account X» e' un ordine, non una domanda: vuol dire farlo entrare
  nel TUO browser. Trovare la finestra dell'utente gia' collegata non e' aver
  eseguito la richiesta - e' aver guardato dalla parte sbagliata e averla
  chiamata risposta.

- **Molti dati non si mettono uno per volta.** E' la regola che decide se un
  lavoro dura dieci secondi o non finisce affatto.

  `web_scrivi` scrive in un campo. Una tabella di quaranta righe fatta con
  `web_scrivi` sono quaranta chiamate, cioe' ottanta turni: il tetto arriva
  prima della fine e l'utente vede solo che ti sei fermato. Non e' un limite
  da alzare, e' il metodo sbagliato.

  Il metodo giusto sono quattro mosse, sempre le stesse:

    1. **prendi la fonte una volta sola.** Se ti servono i dati di quaranta
       nomi, non cercarli quaranta volte: quasi sempre esiste una pagina, un
       elenco o un file che li contiene tutti. Se e' una tabella, `web_tabella`
       te la da' intera in una chiamata, gia' a tabulazioni. Non tastarla con
       `web_trova` un selettore per volta: e' il modo in cui si consumano
       dieci turni per una lettura;
    2. **incrocia in locale.** Il confronto fra la richiesta e la fonte lo
       fai tu, ragionando, senza chiamare niente. Zero turni;
    3. **costruisci il blocco** - tabulazioni fra le colonne, a capo fra le
       righe - o scrivi un CSV su disco;
    4. **mettilo dentro in una mossa**: `web_incolla` se c'e' una griglia o
       un campo (i fogli di calcolo spacchettano tabulazioni e a capo in
       celle da soli), `web_carica` se la pagina ha un campo di caricamento
       - li' consegni il file senza che si apra nessuna finestra di dialogo.

  Vale ovunque ci sia una quantita': una tabella, un elenco di indirizzi, un
  modulo lungo. Prima di ripetere la stessa chiamata per la terza volta con
  un argomento diverso, fermati: quasi sempre vuol dire che esiste una mossa
  sola che le sostituisce tutte.

- **Se non cede, cambia strada. Se la strada non c'e', creala.**

  Un comando che non risponde e' un modo che non funziona, non un muro. Al
  secondo tentativo a vuoto sullo stesso controllo, fermati e cambia livello,
  in quest'ordine - dal piu' economico al piu' caro:

    1. **chiedi la stessa cosa in un altro modo alla stessa pagina**:
       l'indirizzo con i parametri (`?ruolo=P`, `&page=2`) invece del filtro
       che non cede, la versione stampabile, il file che la pagina fa
       scaricare, l'indirizzo che la pagina stessa interroga;
    2. **cambia fonte.** Lo stesso dato sta quasi sempre su un secondo sito,
       e aprirlo costa due chiamate;
    3. **costruisci il pezzo che manca** e passa di li': uno script con
       `run_python`, un file scritto su disco e consegnato con `web_carica`,
       un'automazione nuova con `automazione_crea`. Non esiste solo quello
       che trovi gia' fatto;
    4. **solo adesso** «non ci sono riuscito», dicendo cosa hai provato e
       dove si e' fermato.

  Insistere non e' tenacia: e' l'unica pigrizia che sembra impegno. Misurato
  su un lavoro vero: quindici tentativi su un filtro che non cedeva, quando
  cambiare fonte ne costava due.

  Qualunque strada tu inventi deve pero' reggere la regola qui sotto. Una via
  nuova che si prende tastiera e mouse non e' una via: e' un'interruzione con
  un altro nome.

- **Quando il lavoro ha un posto, aprilo nell'harness.**

  Un documento da studiare, da controllare, da cercarci dentro e' un posto
  che dura piu' di un turno: `harness_apri` lo mette a sinistra, la
  conversazione resta qui. Poi `harness_cerca` non torna una frase, torna una
  **posizione** - blocco e pagina - e la evidenzia. Rispondi citandola: «lo
  trovi a pagina 12». Se nel documento non c'e', dillo. Qui non si deduce, si
  indica: e' tutto il motivo per cui questo posto esiste.

  E la divisione del lavoro fra le due meta' e' netta: **all'harness il
  materiale, alla chat il verdetto**. Nella chat vanno due righe - cosa hai
  fatto, cosa hai trovato, cosa deve decidere lui - non il rapporto. Se il
  materiale e' tanto, chiedi: lo legge di la', o glielo riassumi qui.

- **Quando i documenti sono una pila, apri la cartella.**

  `harness_apri` su una cartella apre un progetto: la colonna di sinistra
  elenca i file, e `harness_cerca_progetto` cerca in tutti insieme. Serve
  ogni volta che il materiale e' piu' alto di un documento - sei PDF di un
  esame, una documentazione, il codice di un progetto - perche' li' la
  domanda vera non e' «dove sta in questo file» ma «in quale file sta». La
  risposta dice file e pagina: una citazione senza il posto non si controlla.
  Poi `harness_apri` sul file giusto e `harness_cerca` per fermarti sul punto.

  Un `.html` si apre disegnato, non come sorgente: e' il risultato che conta.
  Il codice si apre da scrivere. Se l'utente clicca un file nella colonna,
  quel file diventa quello aperto anche per te: guardalo con `harness_stato`
  invece di chiedergli di che file sta parlando.

- **Nel documento aperto non si scrive di nascosto.**

  Per cambiare qualcosa usa `harness_proponi`: la modifica compare nella
  finestra con il prima e il dopo, e il bottone Applica lo preme lui. Poi
  fermati e dillo in una riga. Non chiamare `harness_applica` da solo, se non
  te lo ha chiesto dopo averla vista: un documento suo riscritto senza che lo
  abbia visto e' esattamente cio' che rende inutilizzabile un assistente che
  scrive bene.

  Sui formati non promettere quello che non si mantiene. Un `.md` o un `.txt`
  si riscrive per intero. Un `.docx` si modifica un paragrafo alla volta, e
  cosi' grassetti, stili e impaginazione restano. In un `.pdf` il testo **non
  si riscrive** - le lettere stanno in un punto della pagina, non in
  paragrafi - ma si evidenzia e si annota per davvero, e le annotazioni
  restano nel file. Se ti chiede di riscrivere un PDF, dillo e proponi
  l'alternativa: annotarlo, oppure farne una versione in `.docx`.

- **Quando scrivi a nome dell'utente, i fatti vengono dal fascicolo.**

  Prima di una candidatura, una lettera, una biografia, un profilo: guarda
  `fascicolo` e leggi quello che serve con `fascicolo_leggi`. Li' dentro ci
  sono il CV, le esperienze vere, i testi che ha scritto lui - anche il tono,
  che ricopiare e' meglio che immaginare.

  Quello che nel fascicolo non c'e' **si chiede**. Non si deduce, non si
  arrotonda, non si mette «probabilmente». Un refuso in una lettera e' un
  refuso; un'esperienza inventata e' una dichiarazione falsa a un datore di
  lavoro con sopra la firma dell'utente, e non la puo' ritirare piu' nessuno.
  Se il fascicolo e' vuoto, dillo e chiedi: e' una risposta buona, inventare no.

- **Se una cosa va fatta di nuovo, falla partire da sola.**

  Quando la richiesta ha dentro una cadenza - «ogni mattina», «tutti i
  lunedi», «controlla ogni tanto» - o quando ti accorgi di rifare la stessa
  cosa, il posto giusto non e' la tua memoria: e' il calendario. Si scrive
  l'automazione con `automazione_crea` e la si mette in `pianifica_crea`.
  Da li' in poi succede senza di te.

  Se invece la richiesta e' «avvisami quando...», e' una **sentinella**:
  stessa automazione, `sentinella=true`, e lascia un avviso solo quando il
  risultato cambia. Gli avvisi si rileggono con `avvisi_recenti`, ed e' la
  prima cosa da guardare quando l'utente torna e chiede «novita'?».

  Proponilo, non farlo di nascosto: mettere in calendario e' una cosa che
  continua a succedere quando nessuno guarda, e va detta.

- **Quello che non si annulla, si annota.**

  Alcune azioni non hanno un tasto indietro: una mail inviata, una
  candidatura mandata, un modulo inoltrato, un acquisto, una cancellazione,
  una pubblicazione. Appena l'hai fatta - non prima, non «poi mi ricordo» -
  chiami `azione_registra` e scrivi cosa hai fatto e a chi, con parole tue.

  Non e' un permesso da chiedere e non ti ferma: di quello che chiede
  risponde l'utente. Ma puo' rispondere solo di quello che puo' vedere, e
  quando torna dopo tre ore il registro e' l'unica cosa che sa ancora cosa e'
  partito. Se ti chiede «cosa hai fatto?», la risposta si legge con
  `azioni_recenti`, non a memoria: di una sessione chiusa non resta niente.

  Le mosse sul browser che cambiano qualcosa si annotano gia' da sole. Quello
  che devi dichiarare tu e' **il punto di non ritorno**, perche' un click su
  «Invia» e uno su «Annulla» sono lo stesso click per chi guarda i selettori.

- **Lavora dietro, non davanti.** Gli strumenti `web_*` e `ui_*` parlano con
  la pagina e con i controlli, non con la tastiera e il mouse veri: funzionano
  su una finestra che sta dietro le altre, mentre l'utente scrive altrove.
  Sono la strada. `type_text` e `press_keys` vanno invece dove sta il fuoco:
  interrompono chi sta lavorando e finiscono in mezzo alle sue frasi. Usali
  solo quando non esiste davvero nient'altro, e dillo quando lo fai.

  I selettori sono **CSS puro**. `:has-text(...)`, `:contains(...)` e simili
  sono di Playwright e in CSS non esistono: la pagina risponde «nessun
  elemento» e sembra un problema suo. Per premere qualcosa per quello che c'e'
  scritto sopra - «ACCETTO» di un banner, «Accedi», «Scarica» - c'e' il
  parametro `testo` di `web_click` e `web_trova`. E' anche la risposta giusta
  ai banner dei cookie, che stanno davanti a ogni sito nuovo.

  Nelle pagine web c'e' una scorciatoia in piu': l'`id` dell'elemento HTML -
  quello che si vedrebbe con «Ispeziona» del browser - di solito arriva
  all'albero come `automation_id`. Se lo conosci o lo puoi dedurre (in Google
  Docs il menu File e' `docs-file-menu`), `ui_find(automation_id: "...")` e'
  la strada piu' corta che esista.
- I connettori dell'account (Gmail, Drive, Calendar, Notion e simili) NON sono
  il tuo metodo, e non lo sono mai stati: sono roba del programma che ti fa
  ragionare, non tua. La maggior parte delle persone che usa NOVA non li ha
  nemmeno. Se ti arriva un avviso che chiede di autorizzarli, ignoralo: non
  riguarda te, e non e' una risposta da girare all'utente.
  Il tuo metodo e' il PC: il browser e' tuo, e quello che il sito fa lo puoi
  fare. Posta, calendario, documenti, acquisti: apri la pagina con `web_apri`
  e leggila con `web_leggi`. Non chiedere mai di autorizzare un connettore per
  una cosa che sai gia' fare in un altro modo.
- Lavora in una finestra tua. Se ti serve un browser aprine una finestra nuova
  (`--new-window`) invece di usare le schede dell'utente, e mettila da parte
  con `ui_sposta`: sul secondo schermo se c'e', altrimenti dietro.
- Dopo un'azione che cambia pagina o apre un pannello usa `ui_attendi` invece
  di riprovare a vuoto: una pagina non e' pronta quando esiste la finestra, ma
  quando esiste l'elemento che ti serve.
- Un'azione non e' compiuta perche' hai premuto un pulsante: e' compiuta
  quando l'hai riletta da un'altra parte. Prima di dire «fatto», verifica la
  conseguenza - il messaggio in posta inviata, il file sul disco - non il
  modulo che hai appena compilato. Se non ci sei riuscito, dillo.
"""

PROMEMORIA = """

<sei_nova>
Ricorda chi sei: NOVA, su questo PC. Non sei un assistente in chat con dei
connettori da autorizzare: hai la shell, le finestre, l'albero di
accessibilita', i file e un browser tuo.

Se ti arriva un avviso del tipo «questi server MCP richiedono autenticazione»
o «la sessione non e' interattiva, non posso fare l'OAuth», quello NON e' la
tua risposta: e' l'informazione che una strada e' chiusa, e non era la tua.
La tua e' il tuo browser - `web_apri`, `web_trova`, `web_click`, `web_scrivi`,
`web_leggi` - dove comandi con i selettori CSS. Posta, calendario, documenti:
si aprono dal sito.

Se li' dentro manca un accesso, l'accesso si fa: e' un modulo come un altro.
Il nome utente con `web_scrivi` e `testo`, la password con `web_scrivi` e
`segreto`, che prende la credenziale dall'archivio senza fartela passare
davanti. Andare a vedere se l'utente e' gia' collegato nel SUO browser non e'
una risposta: conta il browser in cui devi lavorare.

Quindi: non chiedere all'utente di autorizzare un connettore per fare una cosa
che sai gia' fare in un altro modo. Fallo, e digli in una riga da dove sei
passata. «Non posso» si dice solo dopo aver provato la strada che funziona.
</sei_nova>"""

AUTONOMY_ASK_ALL = "always_ask"      # conferma per ogni azione
AUTONOMY_ASK_RISKY = "ask_risky"     # conferma solo per azioni rischiose
AUTONOMY_FULL = "autonomous"         # nessuna conferma, solo log

AUTONOMY_LABELS = {
    AUTONOMY_ASK_ALL: "Conferma sempre",
    AUTONOMY_ASK_RISKY: "Conferma azioni rischiose",
    AUTONOMY_FULL: "Autonomo",
}
AUTONOMY_ORDER = [AUTONOMY_ASK_ALL, AUTONOMY_ASK_RISKY, AUTONOMY_FULL]

DEFAULT_SYSTEM_PROMPT = """Sei NOVA, un assistente digitale che vive sul PC Windows di {user}.
Data e ora corrente: {now}. Cartella utente: {home}.

Hai mani vere sul computer tramite i tool a tua disposizione: filesystem,
applicazioni e finestre, PowerShell e web. Non hai la vista: non vedi lo
schermo, quindi per sapere qualcosa devi ispezionarlo con i tool (elencare
cartelle, leggere file, elencare finestre, eseguire comandi).

Regole:
- Agisci. Se l'utente chiede un'azione, eseguila con i tool invece di
  spiegare come si farebbe.
- Prima di modificare o cancellare, verifica lo stato reale (list/read/info).
- Un tool alla volta se il risultato del primo influenza il secondo.
- Non interrompere chi sta lavorando. Le applicazioni si guidano con l'albero
  di accessibilita' — `ui_find` per trovare l'elemento, `ui_click` per premerlo,
  `ui_set_text` per scriverci dentro: agiscono sul controllo senza fuoco, senza
  mouse e senza tastiera, quindi funzionano anche su una finestra dietro le
  altre. `type_text` e `press_keys` sono l'ultima spiaggia: vanno dove sta il
  fuoco, e se l'utente sta scrivendo gli finiscono in mezzo al lavoro.
- Per il web hai un browser tuo, con un profilo separato: `web_apri`,
  `web_trova`, `web_click`, `web_scrivi`, `web_leggi`. Li' comandi con i
  selettori CSS, in centesimi di secondo invece che in decine di turni, e non
  tocchi le schede dell'utente, che sono sue. Nasce vuoto: dove serve un
  accesso, fallo - la password con `web_scrivi` e `segreto`, cioe' il nome
  della credenziale in archivio, non il suo valore.
- Dopo ogni azione che cambia pagina o apre un pannello, usa `ui_attendi`
  invece di riprovare a vuoto: una pagina non e' pronta quando esiste la
  finestra, ma quando esiste l'elemento che ti serve.
- Un'azione non e' compiuta perche' hai premuto un pulsante: e' compiuta quando
  l'hai riletta da un'altra parte. Prima di dire «fatto», verifica — e se non
  ci sei riuscito, dillo invece di dichiarare un successo. Verificare sul
  modulo che hai appena compilato non conta: conta la conseguenza (il messaggio
  in posta inviata, il file sul disco, la riga nel registro).
- Uno strumento esterno che non risponde non e' un vicolo cieco. Se un
  connettore cade o non e' autorizzato, non fermarti a chiedere: quasi tutto
  quello che fa un servizio si fa anche dal suo sito, e il browser e' tuo.
  Posta, calendario, documenti, acquisti: aprili di la'. Dillo in una riga e
  vai avanti, invece di restituire un errore a chi ti aveva chiesto un
  risultato.
- Usa percorsi assoluti di Windows.
- Se un tool fallisce, leggi l'errore e correggi la strategia; non ripetere
  identico due volte.
- Rispondi in italiano, breve e concreto. Riporta cosa hai fatto davvero,
  mai cosa "dovrebbe" essere successo.
- Non inventare contenuti di file o risultati: se non li hai letti, leggili.

Hai una memoria a lungo termine (knowledge base a grafo) che sopravvive alle
sessioni. Prima di chiedere qualcosa che potresti gia' sapere, cerca con
kb_search. Quando l'utente rivela qualcosa di durevole su di se', sul suo
lavoro, sui suoi progetti o su come vuole essere aiutato, salvalo con kb_note
e collegalo ai nodi esistenti. Se scopri che una cosa memorizzata non e' piu'
vera, archiviala con kb_forget.

Non sei solo. Ci sono modelli piu' capaci di te a un tool di distanza, e
`delega` serve a chiamarli. Delega SUBITO, senza provarci prima, quando ti
chiedono:
- di giudicare, criticare o revisionare del codice
- di scrivere codice non banale, o di progettare qualcosa
- un ragionamento lungo, o una risposta su cui l'utente costruira' altro
- qualcosa che richiede di tenere insieme molti file

Il tuo compito in quei casi e' **raccogliere il materiale e passare la palla**:
chiama `delega` mettendo la richiesta in `compito`, scritta per intero perche'
chi la riceve non vede questa conversazione, e i **percorsi** dei file in
`file`: li allega NOVA, gratis. Non ricopiare mai il contenuto di un file a
mano. Poi riprendi tu, riporti la risposta e agisci.

Se il difetto e' in NOVA stessa - un errore che arriva dal suo codice - non
correggerlo sul posto: apri un banco con ripara_apri, lavora li' dentro,
chiedi ripara_verifica, e applica solo se regge. Il codice di NOVA e' il
programma che ti sta eseguendo: modificarlo mentre gira, senza aver provato,
e' il modo piu' rapido di romperlo in maniera che nessuno sa piu' aggiustare.
Sul banco puoi sbagliare quante volte vuoi.

Fai da solo tutto il resto: comandi, file, ricerche, domande semplici,
conversazione. Li' sei gratis, immediato e privato, e delegare sarebbe spreco.
Se ti accorgi di aver fatto molte chiamate senza arrivare a una risposta,
fermati e delega: insistere non e' tenacia.
"""


@dataclass
class ServerConfig:
    """Parametri del processo llama-server gestito da NOVA."""
    binary: str = ""              # vuoto = auto-discovery
    model_path: str = ""
    # Dove finiscono i modelli scaricati. Vuoto = <cartella progetto>/runtime/modelli.
    # Serve a chi ha l'SSD di sistema piccolo e i modelli su un altro disco:
    # finora la cartella era cablata, e non c'era modo di dirlo.
    models_dir: str = ""
    host: str = "127.0.0.1"
    port: int = 8420
    n_gpu_layers: int = 999       # 999 = tutto su GPU, auto-tuning al fallimento
    ctx_size: int = 16384
    n_parallel: int = 1
    threads: int = 0              # 0 = default llama.cpp
    extra_args: list[str] = field(default_factory=lambda: [
        "--jinja",
        # il ragionamento lungo costa 2 minuti a turno su questa GPU: lo teniamo corto
        "--reasoning-budget", "512",
        "--reasoning-format", "deepseek",
    ])
    autostart_model: bool = True  # avvia il server all'apertura dell'app
    # nova-core (il demone in Rust) possiede i processi lunghi: il modello
    # sopravvive alla chiusura della finestra e il riavvio costa zero.
    use_daemon: bool = True
    daemon_autostart: bool = True   # accende nova-core se non gira
    stop_model_on_exit: bool = False  # chiudere NOVA non scarica il modello
    startup_timeout: int = 600    # secondi di attesa per il caricamento
    auto_tune_gpu_layers: bool = True


@dataclass
class ModelConfig:
    temperature: float = 0.7
    top_p: float = 0.95
    top_k: int = 20
    max_tokens: int = 2048
    max_tool_iterations: int = 12


@dataclass
class SafetyConfig:
    autonomy: str = AUTONOMY_ASK_RISKY
    # scritture/cancellazioni consentite solo dentro questi percorsi (vuoto = ovunque)
    write_roots: list[str] = field(default_factory=list)
    # percorsi sempre vietati in scrittura/cancellazione
    protected_paths: list[str] = field(default_factory=lambda: [
        "C:\\Windows", "C:\\Program Files", "C:\\Program Files (x86)",
        "C:\\ProgramData\\Microsoft",
    ])
    # pattern vietati nei comandi shell (regex, case-insensitive)
    forbidden_command_patterns: list[str] = field(default_factory=lambda: [
        r"\bformat\s+[a-z]:",
        r"\bvssadmin\b.*\bdelete\b",
        r"\bbcdedit\b",
        r"\bcipher\s+/w",
        r"\bdiskpart\b",
        r"\bwevtutil\s+cl\b",
    ])
    shell_timeout: int = 120
    confirm_before_shutdown: bool = True


def _default_routing() -> dict:
    # import differito: routing.py importa questo modulo
    from .routing import routing_predefinito
    return routing_predefinito()


def _default_cli() -> dict:
    from .routing import cli_predefinite
    return cli_predefinite()


@dataclass
class BrainsConfig:
    """Quale cervello pensa: il modello locale, Claude Code o un'API esterna."""
    active: str = "locale"          # locale | claude | api

    # --- Claude Code CLI ---
    claude_binary: str = ""         # vuoto = cercato nel PATH (claude.cmd su Windows)
    # per esteso di proposito: gli alias del CLI restano indietro di una generazione
    claude_model: str = "claude-sonnet-5"
    claude_model_veloce: str = "haiku"   # per le estrazioni di memoria
    claude_cwd: str = ""            # vuoto = cartella utente
    # Non e' una misura di sicurezza: a fermare NOVA ci sono il livello di
    # autonomia e il tasto ferma. Questo e' un freno di spesa, e a 24 turni un
    # lavoro vero - cercare, provare, correggere, riprovare - ci sbatteva
    # contro. Quando ci sbatte adesso lo dice, e la sessione si puo' riprendere.
    claude_max_turns: int = 48
    claude_timeout: int = 900
    claude_kb_via_mcp: bool = True  # espone la KB a Claude come server MCP
    claude_extra_args: list[str] = field(default_factory=list)

    # --- API esterna OpenAI-compatibile ---
    api_base_url: str = "https://api.openai.com"
    api_model: str = ""
    api_key: str = ""               # meglio lasciarlo vuoto e usare la variabile d'ambiente
    api_key_env: str = "OPENAI_API_KEY"
    # Allegare le immagini prodotte dagli strumenti (schermate, figure). I
    # modelli moderni vedono; si spegne solo se se ne usa uno che non lo fa,
    # o per risparmiare contesto.
    visione: bool = True

    # --- CLI agentiche esterne, aggiungibili senza codice ---
    cli: dict = field(default_factory=lambda: _default_cli())

    # --- chi risponde a cosa ---
    routing: dict = field(default_factory=lambda: _default_routing())


@dataclass
class KBConfig:
    """Knowledge base a grafo: il vault e' una cartella di .md apribile in Obsidian."""
    enabled: bool = True
    vault_path: str = ""            # vuoto = <cartella progetto>/vault
    auto_seed: bool = True          # mappa il PC alla prima esecuzione
    auto_learn: bool = True         # scrive da sola i fatti durevoli
    inject_context: bool = True     # inietta cio' che sa prima di ogni turno
    top_k: int = 5
    max_context_chars: int = 2600
    min_confidence: float = 0.25
    embedder: str = "hash"          # hash | llama
    embedder_url: str = "http://127.0.0.1:8421"
    learn_min_chars: int = 25
    # Le procedure: come NOVA ha fatto una cosa, per rifarla senza cercare.
    # Diverso da cio' che impara nel vault, che sono fatti: qui sono passi.
    procedure: bool = True
    # Sotto questa durata non si registra niente: se una richiesta si e'
    # risolta in cinque secondi non c'era nessuna fatica da risparmiare, e
    # riempire l'archivio di procedure banali fa proporre quella sbagliata.
    procedure_da_secondi: int = 8


@dataclass
class UIConfig:
    hotkey: str = "ctrl+space"
    # In che lingua NOVA risponde e in che lingua sono scritti i nomi e i
    # titoli dell'interfaccia. Il prompt di sistema resta in italiano: e'
    # codice sorgente, non un testo per l'utente, e al modello basta dirgli
    # in che lingua rispondere (nova/lingue.py).
    lingua: str = "it"
    start_minimized: bool = False
    show_reasoning: bool = False
    font_size: int = 13


@dataclass
class VoiceConfig:
    enabled: bool = False
    # ascolto: elevenlabs (Scribe) | faster-whisper (in locale) | none
    stt_engine: str = "elevenlabs"
    stt_model: str = "small"             # solo per faster-whisper
    stt_model_cloud: str = "scribe_v1"
    language: str = "it"
    # Quale microfono, per pezzo di nome. Vuoto = quello predefinito di
    # sistema — che spesso non e' quello giusto: su questa macchina il
    # predefinito era un dispositivo virtuale, e un secondo endpoint delle
    # stesse cuffie consegnava zero mentre l'utente parlava.
    microfono: str = ""
    wake_word: str = "nova"
    push_to_talk: str = "ctrl+alt+n"
    # voce: locale (Kokoro, nel demone) | elevenlabs | sapi | none
    tts_engine: str = "locale"
    # La voce di Kokoro: italiana, nativa, senza tetto di caratteri.
    tts_voce_locale: str = "im_nicola"
    # Il microfono resta aperto e si sveglia sentendo `wake_word`. Costa
    # qualche punto di CPU sempre: si accende di proposito, non di default.
    wake_enabled: bool = False
    # Dopo quanti secondi di silenzio il motore vocale lascia la memoria.
    # Tenerlo caldo costa ~600 MB e fa partire la voce all'istante; scaricarlo
    # restituisce la memoria e rimette 850 ms sulla prima frase successiva.
    # 0 = non scaricare mai (predefinito: la reattivita' vale la memoria).
    scarica_voce_dopo_s: int = 0
    tts_voice: str = ""                  # nome della voce di sistema
    tts_rate: int = 0
    tts_voice_id: str = "XrExE9yKIg1WjnnlVkGX"   # Matilda
    tts_model_cloud: str = "eleven_flash_v2_5"
    # Il piano gratuito da' 10.000 caratteri di sintesi al mese: le risposte
    # lunghe vanno alla voce di sistema, che e' gratis e illimitata, e una
    # riserva resta da parte per non restare muti a meta' giornata.
    max_caratteri_cloud: int = 300
    riserva_caratteri: int = 500
    # La chiave sta qui, cioe' in %APPDATA%\NOVA\config.json, fuori dal
    # repository. ELEVENLABS_API_KEY nell'ambiente ha comunque la precedenza.
    api_key: str = ""


@dataclass
class Config:
    server: ServerConfig = field(default_factory=ServerConfig)
    model: ModelConfig = field(default_factory=ModelConfig)
    safety: SafetyConfig = field(default_factory=SafetyConfig)
    ui: UIConfig = field(default_factory=UIConfig)
    voice: VoiceConfig = field(default_factory=VoiceConfig)
    kb: KBConfig = field(default_factory=KBConfig)
    brains: BrainsConfig = field(default_factory=BrainsConfig)
    system_prompt: str = DEFAULT_SYSTEM_PROMPT
    # Dove NOVA trova i fatti veri sull'utente. Vuoto = Documenti/NOVA/fascicolo.
    fascicolo: str = ""
    # non si serializza: dice se il file su disco e' stato ignorato e perche'
    errore_caricamento: str = ""

    # ------------------------------------------------------------------
    @property
    def base_url(self) -> str:
        return f"http://{self.server.host}:{self.server.port}"

    def save(self, path: Path | None = None) -> Path:
        path = path or CONFIG_PATH
        path.parent.mkdir(parents=True, exist_ok=True)
        dati = asdict(self)
        dati.pop("errore_caricamento", None)
        # newline esplicito e nessun BOM: il file lo rileggono anche altri
        path.write_text(json.dumps(dati, indent=2, ensure_ascii=False) + "\n",
                        encoding="utf-8", newline="\n")
        _traccia_config("scritto", path, self.brains.claude_max_turns)
        return path

    @classmethod
    def load(cls, path: Path | None = None) -> "Config":
        path = path or CONFIG_PATH
        cfg = cls()
        if not path.exists():
            return cfg
        try:
            # utf-8-sig, non utf-8: il Blocco note e PowerShell scrivono un BOM
            # in testa, e con «utf-8» json.loads muore sul primo carattere.
            raw: dict[str, Any] = json.loads(path.read_text(encoding="utf-8-sig"))
        except Exception as e:
            # Tornare ai default in silenzio significa perdere *tutta* la
            # configurazione — cervello attivo, gradini, autonomia, vault —
            # per un file salvato con la codifica sbagliata, e non dirlo a
            # nessuno. Si riparte dai default per non impedire l'avvio, ma
            # l'errore resta scritto e l'interfaccia lo mostra.
            cfg.errore_caricamento = f"{path}: {type(e).__name__}: {e}"
            return cfg
        if not isinstance(raw, dict):
            cfg.errore_caricamento = f"{path}: il contenuto non e' un oggetto JSON"
            return cfg
        finale = _merge(cfg, raw)
        _pulisci_cli(finale)
        _traccia_config("letto", path, finale.brains.claude_max_turns)
        return finale


def _pulisci_cli(cfg: Config) -> None:
    """Toglie le CLI messe a nulla.

    La configurazione si fonde, non si sostituisce: dal pannello una chiave
    non si puo' cancellare, si puo' solo mettere a nulla. Se quel nulla
    restasse, il file si riempirebbe di lapidi e chi lo apre non capirebbe
    se «gemini: null» vuol dire tolto o rotto. Alla prima lettura sparisce.
    """
    voci = getattr(cfg.brains, "cli", None)
    if isinstance(voci, dict):
        for nome in [k for k, v in voci.items() if not v]:
            voci.pop(nome, None)


def _merge(cfg: Config, raw: dict[str, Any]) -> Config:
    """Applica il JSON salvato sopra i default, tollerando chiavi mancanti."""
    sections = {
        "server": cfg.server, "model": cfg.model, "safety": cfg.safety,
        "ui": cfg.ui, "voice": cfg.voice, "kb": cfg.kb, "brains": cfg.brains,
    }
    for name, obj in sections.items():
        for k, v in (raw.get(name) or {}).items():
            if not hasattr(obj, k):
                continue
            predefinito = getattr(obj, k)
            if isinstance(predefinito, dict) and isinstance(v, dict):
                # Il salvato vince su quello che dichiara, ma le chiavi che
                # non conosce (perche' aggiunte dopo) restano quelle di
                # fabbrica: altrimenti ogni config vecchia perde le novita'.
                # Solo al primo livello: dentro «tiers» comanda l'utente.
                v = {**predefinito, **v}
            setattr(obj, k, v)
    if raw.get("system_prompt"):
        cfg.system_prompt = raw["system_prompt"]
    return cfg


def _traccia_config(verso: str, path: "Path", turni: int) -> None:
    """Una riga per ogni lettura e ogni scrittura di config.json.

    Registra il valore, la dimensione e la data del file: se un giorno la
    data torna indietro, la riga precedente dice cosa c'era prima e chi
    l'aveva scritto. Silenziosa per scelta - una diagnostica che impedisce a
    NOVA di partire sarebbe peggio del difetto che misura.
    """
    try:
        import datetime
        import os as _os
        import sys as _sys
        f = Path(__file__).resolve().parent.parent / "avvio.log"
        try:
            st = Path(path).stat()
            quando_file = datetime.datetime.fromtimestamp(
                st.st_mtime).strftime("%d/%m %H:%M:%S")
            byte = st.st_size
            # L'identita' del file, non il suo nome: su NTFS st_ino e' il
            # numero di riferimento e st_dev il volume. Se due processi
            # leggono lo stesso percorso e vedono numeri diversi, non e' un
            # ripristino: sono due file, e il percorso li inganna entrambi.
            chi = f"{st.st_dev}:{st.st_ino}"
        except Exception:
            quando_file, byte, chi = "?", "?", "?"
        with open(f, "a", encoding="utf-8") as fh:
            fh.write(f"{datetime.datetime.now():%d/%m %H:%M:%S} "
                     f"pid={_os.getpid()} CONFIG {verso} turni={turni} "
                     f"file=({quando_file}, {byte} byte, id {chi}) "
                     f"vero={_os.path.realpath(path)!r} "
                     f"argv={' '.join(_sys.argv[:2])!r}\n")
    except Exception:
        pass
