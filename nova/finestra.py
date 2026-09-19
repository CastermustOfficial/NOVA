"""La finestra: quanto della conversazione ci sta nella memoria del modello.

Questo modulo non tocca niente. Guarda le righe di una conversazione e dice
**quali tenere** e **quali accorciare**; chi le tiene davvero - che ha per le
mani messaggi ben piu' ricchi di un ruolo e un testo, con dentro chiamate a
strumenti e identificativi - applica il piano e basta.

La ragione di questa divisione e' che il taglio e' un pezzo di ragionamento
delicato, pieno di casi che sono gia' andati storti una volta, e va potuto
provare senza un agente intorno. Il gemello in Rust e'
`core/crates/nova-finestra`: le due teste devono dire la stessa cosa, e c'e'
un banco che glielo chiede.
"""
from __future__ import annotations

#: Quanti caratteri vale un token, per stimare senza tokenizzatore.
#:
#: Misurato due volte sulla macchina di casa, con prompt veri: 3,88 su una
#: conversazione in italiano e 4,37 su del testo ripetitivo letto da un file.
#: Si tiene il numero **piu' basso** dei due, anzi un filo sotto: sbagliare
#: per eccesso di token vuol dire tagliare un po' presto, che si nota appena;
#: sbagliare per difetto vuol dire sfondare il contesto, e quello e' un
#: errore in faccia all'utente.
CARATTERI_PER_TOKEN = 3.5

#: Quanto lasciare libero per la risposta. Il contesto non serve solo a
#: leggere: il modello ci scrive dentro.
RISERVA_RISPOSTA_TOKEN = 1024

#: Sopra questo numero di righe si taglia.
TETTO_MESSAGGI = 60

#: E si scende fino a qui. La distanza fra i due e' il punto: senza, si
#: taglia a ogni turno.
FONDO_MESSAGGI = 40

#: Sotto questa lunghezza un messaggio non si accorcia piu': quel che resta e'
#: gia' solo l'inizio e la fine, e continuare vorrebbe dire toglierne il senso
#: invece che il peso.
MINIMO_ACCORCIABILE = 400


def stima_token(testo: str | None) -> int:
    """Quanti token vale un testo, senza tokenizzatore.

    Serve una stima e non una misura: il tokenizzatore vero sta nel modello,
    cambia con il modello, e chiederglielo costerebbe un giro di rete per ogni
    messaggio a ogni turno solo per decidere se tagliare.
    """
    return int(len(testo or "") / CARATTERI_PER_TOKEN) + 1


def spazio_per_la_conversazione(ctx: int, sistema: str, strumenti: str = "") -> int:
    """Quanto resta alla conversazione, tolto il prefisso fisso.

    Il conto non e' un dettaglio contabile, e' la scoperta che ha fatto
    nascere tutto questo. Sulla macchina di casa, con il contesto a 16.384::

        messaggio di sistema     ~5.200 token
        schemi dei sessanta tool ~6.900 token   (il 42% del contesto)
        riserva per la risposta   1.024 token
        ------------------------------------
        resta alla conversazione ~3.300 token   (il 20%)

    Il prefisso fisso si mangia i tre quarti del contesto, e quello che avanza
    e' molto meno di quanto sessanta messaggi possano pesare.

    `ctx` a zero vuol dire che il contesto non lo decide questa
    configurazione - un cervello dietro una API non lo dice - e allora si
    torna zero, cioe' «non lo so».
    """
    if ctx <= 0:
        return 0
    fissi = stima_token(sistema) + RISERVA_RISPOSTA_TOKEN
    if strumenti:
        fissi += stima_token(strumenti)
    return max(0, ctx - fissi)


def fondo_sicuro(tetto: int, fondo: int) -> int:
    """Il fondo davvero usabile, dato il tetto.

    Un fondo troppo vicino al tetto riporta al difetto di prima senza dirlo, e
    «un turno di distanza» non basta: con `tetto - 2` si taglierebbe a turni
    alterni invece che a ogni turno, che e' meta' del difetto e non la sua
    assenza. La distanza minima e' un quarto del tetto, cioe' una decina di
    turni di respiro.
    """
    return max(2, min(fondo, tetto * 3 // 4))


def _token_delle(righe: list[tuple[str, str]], indici: list[int]) -> int:
    return sum(stima_token(righe[i][1]) for i in indici)


def taglia(righe: list[tuple[str, str]], tetto: int = TETTO_MESSAGGI,
           fondo: int = FONDO_MESSAGGI,
           disponibili: int = 0) -> list[tuple[int, str | None]]:
    """Il piano di taglio: `(da_dove, testo_nuovo_o_None)` per ogni riga tenuta.

    Si butta cio' che sta **subito dopo la prima riga**, e quello e' il posto
    peggiore: la cache del prefisso di llama.cpp vale finche' i token in testa
    sono gli stessi, quindi spostare la seconda riga invalida tutto il resto e
    si rielabora l'intera conversazione. Per questo si taglia di rado e si
    scende fino a un fondo, invece di fermarsi sul filo del tetto.

    Misurato con `banco_taglio.py` su Gemma 4 26B-A4B, ottantuno messaggi,
    15.379 token di prefisso::

        a caldo, prefisso intatto           175 ms
        fermandosi sul filo del tetto     1.771 ms
        e il turno seguente               1.731 ms   <- non guarisce
        scendendo fino al fondo           1.217 ms
        e il turno seguente                 226 ms   <- guarito

    Scendere fino a un fondo non cambia **cosa** si butta: cambia quanto
    spesso. Si taglia una volta ogni dieci turni invece che a ogni turno, e
    nei nove in mezzo il prefisso resta valido. Il prezzo e' che quando si
    taglia si butta di piu' in un colpo solo, ed e' un prezzo che si paga
    volentieri: la memoria vera di NOVA non e' questa finestra, e' il vault.
    """
    if not righe:
        return []
    indici = list(range(len(righe)))

    # -- taglio a numero -------------------------------------------------
    if len(righe) > tetto:
        giu = fondo_sicuro(tetto, fondo)
        coda = indici[-(giu - 1):]
        # Una risposta di strumento senza la chiamata che l'ha prodotta non e'
        # leggibile da nessun modello: si scarta finche' la coda non comincia
        # da qualcosa di sensato.
        while coda and righe[coda[0]][0] == "tool":
            coda.pop(0)
        indici = [0] + coda
    # Sotto il tetto non si taglia per numero - ma il taglio a token va fatto
    # lo stesso, ed e' proprio questo il caso che conta: dodici scambi con
    # dentro il contenuto di un file sono venticinque messaggi, quindi passano
    # di qui, e sono centomila token. La prima versione metteva il taglio a
    # token **dopo** l'uscita anticipata, cioe' non lo eseguiva mai nel solo
    # caso per cui era stato scritto.

    # -- taglio a token --------------------------------------------------
    # `< 2` e non `<= 2`: con esattamente due righe - sistema piu' una
    # risposta enorme - non c'e' niente da **togliere**, ma c'e' ancora da
    # **accorciare**, ed e' il caso che ha fatto scrivere l'accorciamento.
    if disponibili <= 0 or len(indici) < 2:
        return [(i, None) for i in indici]
    testa, coda = indici[0], indici[1:]
    if _token_delle(righe, coda) <= disponibili:
        return [(i, None) for i in indici]
    # Stessa idea del fondo: si scende sotto la soglia, non ci si ferma sopra,
    # o si ritaglia a ogni turno e la cache non si riforma mai.
    obiettivo = disponibili * 3 // 4
    while coda and _token_delle(righe, coda) > obiettivo:
        coda.pop(0)
    while coda and righe[coda[0]][0] == "tool":
        coda.pop(0)
    # Non si resta mai senza l'ultimo scambio: una conversazione vuota non e'
    # una conversazione accorciata, e' una amnesia.
    if not coda:
        coda = [indici[-1]]

    fuori: list[tuple[int, str | None]] = [(i, None) for i in coda]
    if _token_delle(righe, coda) > disponibili:
        # E se cio' che resta non ci sta **comunque**, vuol dire che una sola
        # riga e' piu' grande di tutto lo spazio: il contenuto di un file
        # letto, una pagina web intera. Buttarla vorrebbe dire perdere proprio
        # la cosa di cui l'utente ha chiesto conto; tenerla intera vuol dire
        # sfondare il contesto. Si accorcia, e lo si dice nel testo - un
        # taglio dichiarato il modello lo capisce, uno silenzioso gli fa
        # credere che il file finisca li'.
        fuori = _accorcia_le_piu_grosse(righe, fuori, obiettivo)
    return [(testa, None)] + fuori


def _accorcia_le_piu_grosse(righe, fuori, obiettivo):
    """Accorcia le righe piu' grosse finche' la coda non ci sta.

    **La lunghezza da togliere si calcola, non si indovina.** La prima
    versione tagliava a una misura fissa - millecinquecento caratteri in testa
    e altrettanti in coda - e non terminava: la scritta che dichiara il taglio
    e' lunga quanto i caratteri che alla seconda passata restavano da
    togliere, quindi il testo si accorciava di ottanta caratteri e ricresceva
    di ottanta, per sempre. Un ciclo che «ovviamente» finisce e non finisce.
    Adesso a ogni passata si punta alla lunghezza che serve, e si esce se non
    si e' guadagnato niente: due condizioni invece di una, perche' una si e'
    gia' vista sbagliare.
    """
    fuori = list(fuori)

    def testo_di(f):
        return f[1] if f[1] is not None else righe[f[0]][1]

    for _ in range(len(fuori) + 8):            # tetto: mai un ciclo aperto
        ora = sum(stima_token(testo_di(f)) for f in fuori)
        if ora <= obiettivo:
            return fuori
        eccesso = ora - obiettivo
        k = max(range(len(fuori)), key=lambda j: len(testo_di(fuori[j])))
        testo = testo_di(fuori[k])
        if len(testo) <= MINIMO_ACCORCIABILE:
            return fuori                       # niente piu' di grosso
        # Quanto deve diventare lunga, piu' un margine per la scritta.
        da_togliere = int(eccesso * CARATTERI_PER_TOKEN) + 200
        voluta = max(MINIMO_ACCORCIABILE, len(testo) - da_togliere)
        nuovo = accorcia(testo, voluta)
        if nuovo is None:
            return fuori                       # non si guadagna niente
        fuori[k] = (fuori[k][0], nuovo)
    return fuori


def accorcia(testo: str, voluta: int) -> str | None:
    """Accorcia un testo alla lunghezza voluta, tenendo l'inizio e la fine.

    L'inizio dice cos'era, la fine spesso porta la conclusione, ed e' il mezzo
    che si puo' perdere. Torna `None` se non ci si guadagna niente.
    """
    meta = max(60, voluta // 2)
    if meta * 2 >= len(testo):
        return None
    tolti = len(testo) - meta * 2
    nuovo = (
        testo[:meta]
        + f"\n\n[...tagliati {tolti} caratteri perche' non ci stavano"
          " nella memoria del modello...]\n\n"
        + testo[-meta:]
    )
    return None if len(nuovo) >= len(testo) else nuovo
