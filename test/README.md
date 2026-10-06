# test/

Per ogni suite di test di NOVA: cosa controlla, come si lancia e cosa serve per farla girare.

Le prove stanno in [`prove/`](../prove/README.md), divise per cosa serve a farle girare, e le prove del core Rust stanno dentro i crate. Questo è l'indice di tutte. Lo tiene allineato `prove/progetto/test_documentazione.py`: una prova nuova che non compare qui fa rosso.

## Le regole comuni

- **Uscita 0** vuol dire passata, **1** rossa, **2** «qui non si può fare», per esempio perché manca un binario o una macchina vera. Il 2 non è un fallimento.
- Ogni prova Python è un programma, non un caso di `pytest`: si lancia da sola e stampa cosa controlla.
- Si fanno girare **dopo** `git add`, così vedono i file come li vedrà la CI.

```bash
python prove/nova/test_taglio.py                    # una sola
for f in prove/*/test_*.py; do python "$f"; done    # tutte
cd core && cargo test --workspace                   # le prove del core Rust
```

## Le prove del core Rust

I crate in `core/crates/` hanno le loro prove di unità, dentro i file sorgente (`#[cfg(test)]`) o in `tests/`: tutti tranne `novad`, che è solo il punto d'ingresso del demone e si prova acceso, da `prove/demone/`. Girano con `cargo test`, senza macchine speciali. I crate con un **banco** (la feature `banco`, binari `banco-*`) lo espongono alle prove gemelle Python, che lo confrontano con la versione Python sugli stessi casi.

Le prove del recinto di Windows (`nova-platform` e `nova-core`, solo `cfg(windows)`) toccano il PC davvero: registrano il profilo `nova.recinto`, creano cartelle nella radice del disco e le tolgono, e non chiedono amministratore. Quelle che aspettano un rifiuto di Windows si saltano da amministratore, come sul runner della CI. Una cosa non si automatizza: il passo da amministratore, perche' la conferma di Windows non la clicca nessun programma (vedi sotto, «Fuori dalle suite»).

## Dove girano in CI

`.github/workflows/ci.yml`, a ogni push:

| job | dove | cosa fa |
|---|---|---|
| `rust` | Windows | `cargo fmt --check`, `cargo clippy`, `cargo test` |
| `python` | Windows, Python 3.10–3.13 | tutte le `prove/*/test_*.py` |
| `gemelli` | Ubuntu | costruisce i banchi e `novad`, poi le prove gemelle e quelle del demone |
| `altrove` | Ubuntu e macOS | `cargo test --workspace` tranne il guscio, e le prove Python |
| `niente-dati-personali` | Ubuntu | nessun dato personale nei file tracciati |

`.github/workflows/release.yml` compila i binari per Windows quando si pubblica un tag, o a mano.

## Fuori dalle suite

- **Il passo da amministratore** (D367), a mano, con una configurazione di prova: `novad --recinto --prepara` (chiede la conferma di Windows e apre `C:\Users` e le cartelle di strumenti degli amministratori), poi `python prove/demone/test_demone_recinto_strumenti.py` e `python prove/demone/test_demone_recinto.py`, poi `novad --recinto --togli` (altra conferma). Alla fine `C:\Users`, `Temp`, `Python313` e `nodejs` non devono avere piu' voci del contenitore.
- **La suite intera da amministratore** (D369), ogni volta che si tocca il recinto di Windows: `cargo test` e tutte le `prove/*/test_*.py` da un terminale elevato, oltre al giro da utente normale. Solo da amministratore girano `test_demone_elevato.py` e `test_demone_recinto_strumenti.py`; da utente normale escono 2.
- **Il giudizio con un modello vero** (D371): con un llama-server acceso, `NOVA_GIUDIZIO_URL=http://127.0.0.1:8499 cargo test -p nova-core --lib giudizio_casa -- --ignored --nocapture`. Stampa i giudizi di tre domande e pretende che la capitale d'Italia sia Roma; le altre prove di `giudizio_casa` usano un llama-server finto e girano sempre.
- `misure/`: i banchi di prestazione, lanciati a mano sul PC con la scheda video.
- `attrezzi/`: gli script lanciati a mano, compresi i giri di mutazione con cui si verifica che un banco gemello guardi davvero.

## `prove/progetto/`: 27 prove

Cosa serve: niente: guardano il repository stesso.

| prova | cosa controlla |
|---|---|
| `test_attesa.py` | Trenta secondi fermi sembrano un programma rotto. |
| `test_binari.py` | Di cosa e' fatto NOVA sta scritto in un posto solo, e tutti leggono quello. |
| `test_conto_shell.py` | Quanti strumenti di NOVA poggiano ancora su una shell: contati, non detti. |
| `test_cosa_serve_da_fuori.py` | Quello che NOVA chiede al mondo fuori, e cosa dice quando non c'e'. |
| `test_crate_attaccati.py` | Ogni crate o lo esegue qualcuno, o dichiara perche' no. |
| `test_dipendenze.py` | Ogni pacchetto che il codice importa deve stare in requirements.txt. |
| `test_documentazione.py` | La documentazione si scolla un pezzo per volta, non tutta insieme. |
| `test_dove_stanno_i_dati.py` | La mappa dei dati non deve tacere su niente, ne' inventare niente. |
| `test_elenchi_gemelli.py` | Ogni elenco dichiarato in Rust ha un gemello in Python, e si confronta. |
| `test_guardie_predefinite.py` | Le guardie sono un elenco solo, e non ne esiste un secondo. |
| `test_impostazioni.py` | Il pannello deve saper aggiungere, non solo scegliere. |
| `test_installer.py` | L'installer chiede quattro cose, e scrive solo valori che qualcuno legge. |
| `test_niente_cresce_per_sempre.py` | Nessun diario di NOVA puo' crescere per sempre. |
| `test_niente_dati_personali.py` | Niente dati personali nel repository. |
| `test_niente_due_volte.py` | Una costante pubblica vive in un crate solo, o e' dichiarato perche' no. |
| `test_niente_finestre.py` | NOVA lavora dietro, non davanti: nessun processo apre una console. |
| `test_nomi_degli_strumenti.py` | Ogni strumento nominato deve esistere, e col nome giusto. |
| `test_nomi_strumenti.py` | I nomi degli strumenti nel prompt devono esistere davvero. |
| `test_pannello_e_configurazione.py` | Il pannello scrive dove Python legge davvero. |
| `test_ponte_col_guscio.py` | Il ponte fra la pagina e il guscio regge da tutte e due le parti. |
| `test_primi_minuti.py` | «Cosa le chiedo?» e' la prima domanda, e non e' «come funziona». |
| `test_prove_ordinate.py` | Le prove stanno in una cartella, e ogni cartella dice cosa serve per girarci. |
| `test_python_minimo.py` | CMP-5. «Python 3.10 o superiore» e' una promessa: che sia vera. |
| `test_readme.py` | Il README dice numeri: che siano quelli veri. |
| `test_stringhe_senza_buchi.py` | Nessuna stringa con dentro l'indentazione della riga dopo. |
| `test_una_porta.py` | Di interfacce ce n'e' una, e si sa qual e'. |
| `test_versione.py` | Un numero di versione solo, in tre file. |

## `prove/nova/`: 41 prove

Cosa serve: niente: puro Python, la prima versione di NOVA.

| prova | cosa controlla |
|---|---|
| `test_anteprime.py` | La conferma deve dire *cosa* sta per fare, in italiano. |
| `test_automazioni.py` | Le automazioni: nascono solo se girano, e diventano strumenti veri. |
| `test_avvio_cervello.py` | Le due condizioni perche' una conversazione nuova possa esistere. |
| `test_avvio_veloce.py` | Accendere NOVA non deve costare piu' del necessario. |
| `test_catalogo.py` | Senza scheda video non si scarica un modello che non si potra' usare. |
| `test_categorie.py` | Verifica la regola strutturale: certe categorie salgono da sole. |
| `test_cerca.py` | Cercare e leggere il web senza aprire una finestra. |
| `test_configurazione_non_perde_niente.py` | Tutto cio' che si salva si rilegge. |
| `test_core_client_riprova.py` | Il client del demone aspetta e riprova se la pipe e' occupata, per un secondo d'orologio anche quando le pause durano piu' del chiesto, e poi lo dice; non riprova se la pipe non c'e' (D368). |
| `test_dati.py` | «Dove sono i miei dati?» e' una domanda di fiducia. |
| `test_diario_del_turno.py` | Un turno che comincia si vede, e un attributo inventato non passa. |
| `test_figure.py` | Quali immagini entrano nella conversazione, e quante. |
| `test_guardie.py` | Le guardie di sicurezza, provate su cartelle vere. |
| `test_guasti.py` | Un guasto detto in italiano, e nessuno che sparisce in silenzio. |
| `test_harness.py` | Le due idee prese in prestito da deepseek-harness, provate sul serio. |
| `test_harness_modifica.py` | Scrivere dentro un documento e' l'azione che non si annulla da se'. |
| `test_harness_prova.py` | Il verificatore: si applica solo se i test non peggiorano. |
| `test_harness_studio.py` | Nova Harness, profilo «studio»: aprire, trovare, leggere intorno. |
| `test_kb_qualita.py` | I difetti che non perdevano dati ma degradavano la memoria col tempo. |
| `test_kb_review2.py` | I nove rilievi della seconda review incrociata. |
| `test_kb_store.py` | I quattro difetti che facevano perdere dati veri, gia' scritti su disco. |
| `test_leggere_non_scrive.py` | Leggere non scrive. |
| `test_mcp_strumenti.py` | Il server MCP di NOVA: dichiara quello che sa fare, e sa fare quello che dichiara. |
| `test_memoria_seconda_strada.py` | COM-15. La domanda non e' «hai chiamato kb_note?», e' «te lo sei ricordato?». |
| `test_percorsi_ostili.py` | Spazi, accenti, apostrofi, parentesi, e percorsi lunghissimi. |
| `test_pianificazione.py` | Automazioni che partono da sole, e il fascicolo da cui si pescano i fatti. |
| `test_prefisso.py` | Il prefisso del prompt non deve cambiare fra un turno e l'altro. |
| `test_procedure.py` | Le procedure imparate: riconoscimento, registrazione, aggancio all'agente. |
| `test_registro.py` | Il registro delle azioni che non si annullano. |
| `test_registro_ricerca.py` | Un registro che non si cerca e' un registro che si legge il primo giorno. |
| `test_ricette_ngram.py` | I trigrammi, la pesca larga, e la fusione dei doppioni. |
| `test_ripiego_app.py` | Aprire un'applicazione una volta sola, anche quando l'avvio tarda. |
| `test_ripiego_sistema.py` | Un guasto a meta' resta un guasto: non si rifa' per altra strada (D322). |
| `test_riservatezza.py` | Cosa NON deve finire nel vault. |
| `test_riservatezza_buchi.py` | I segreti che il guardiano del vault lasciava passare. |
| `test_routing.py` | Verifica i cinque difetti corretti, senza toccare modelli veri. |
| `test_scrittura.py` | Una scrittura interrotta non deve lasciare un file a meta'. |
| `test_segreti.py` | La porta del vault non lascia entrare le credenziali. |
| `test_taglio.py` | Tagliare la conversazione di rado, non a ogni turno. |
| `test_visione.py` | COM-11. Il modello locale che non vede, e cosa succede se lo si ignora. |
| `test_voce.py` | Voce: le decisioni che si prendono senza rete. |

## `prove/gemelli/`: 30 prove

Cosa serve: un banco Rust costruito con `cargo`: la prova stampa la riga per costruirlo.

| prova | cosa controlla |
|---|---|
| `test_browser_rust.py` | Il JavaScript che parte verso la pagina dev'essere lo stesso. |
| `test_cartelle_rust.py` | Riconoscere una cartella sincronizzata deve dare lo stesso esito in Rust. |
| `test_catalogo_rust.py` | Il verdetto sul modello deve essere identico in Rust e in Python. |
| `test_cervelli_rust.py` | Cosa NOVA dice a un cervello che vive fuori, e che dev'essere lo stesso. |
| `test_cli_locali_rust.py` | `nova config`, `nova configura`, `nova modelli`, `nova cli-predefinite`: le stesse risposte del Python che l'installatore chiamava (D350). |
| `test_componenti_cli_rust.py` | `nova componenti`: lo stesso elenco del Python, e gli scaricamenti veri da uno specchio in casa (D351). |
| `test_componenti_rust.py` | Il catalogo dei componenti deve dire la stessa cosa in Rust. |
| `test_configurazione_rust.py` | Leggere la configurazione deve dare lo stesso risultato in Rust. |
| `test_contesto_rust.py` | La finestra di conversazione in Rust deve tagliare esattamente come in Python. |
| `test_dati_rust.py` | «Dove sono i miei dati?» deve rispondere identico in Rust. |
| `test_documenti_rust.py` | `read_document` in Rust contro `read_document` in Python. |
| `test_docx_rust.py` | Per modificare un .docx non serve una libreria di .docx — e va dimostrato. |
| `test_fogli_rust.py` | I fogli di calcolo devono ragionare uguale in Rust. |
| `test_giudizio_rust.py` | Le due meta' leggono gli stessi logit e danno lo stesso giudizio. |
| `test_giudizio_solo_testo.py` | La domanda del giudice ai cervelli di fuori e' quella del modello di casa, carattere per carattere; le risposte si leggono. |
| `test_guasti_rust.py` | I guasti in Rust devono dire le stesse parole, e coprire le stesse chiavi. |
| `test_harness_rust.py` | L'harness deve tagliare e cercare uguale in Rust. |
| `test_imparare_rust.py` | Da uno scambio, il demone deve imparare le stesse cose di `memory.py`. |
| `test_mcp_rust.py` | Il protocollo MCP in Rust deve rispondere la stessa busta del Python. |
| `test_memoria_rust.py` | BM25 e fusione: le due versioni devono dire la stessa cosa. |
| `test_modelli_rust.py` | La ricerca dei modelli in Rust deve trovare quello che trova Python. |
| `test_nodi_rust.py` | Il nodo della memoria deve avere la stessa forma su disco in Rust. |
| `test_pianificazione_rust.py` | «Ogni lunedi' alle 9» deve dare lo stesso istante in Rust e in Python. |
| `test_registro_novad.py` | `novad --registro`: lo stesso racconto di `python -m nova --registro` (D357). |
| `test_registro_rust.py` | Il registro in Rust deve dire esattamente quello che dice in Python. |
| `test_ricette_rust.py` | Le due versioni delle ricette devono dire la stessa cosa. |
| `test_salita_rust.py` | Salire di gradino e girare a vuoto devono decidersi identici in Rust. |
| `test_scala_rust.py` | La scala in Rust deve decidere esattamente come decide in Python. |
| `test_semina_rust.py` | La prima mappatura del PC scrive in Rust gli stessi nodi del Python. |
| `test_strumenti_rust.py` | Gli strumenti dichiarati allo stesso modo, in Rust. |

## `prove/demone/`: 28 prove

Cosa serve: il binario `novad` costruito.

| prova | cosa controlla |
|---|---|
| `test_approvazione.py` | Il ponte delle approvazioni: chi chiede aspetta, chi risponde sblocca. |
| `test_demone_app.py` | Applicazioni, finestre e processi, dal demone. |
| `test_demone_automazioni.py` | Le automazioni del demone contro quelle del Python (D346). |
| `test_demone_cervelli.py` | Passare la palla a un cervello piu' capace, dal demone; il giudice di casa e la sua riga nel registro delle decisioni. |
| `test_demone_claude.py` | Cio' che Claude Code vede del demone: gli strumenti e lo sportello. |
| `test_demone_cli.py` | Il demone fa un turno con un cervello che e' un **programma**, non un URL. |
| `test_demone_compiti.py` | Un compito pianificato fa il suo turno senza Python (D345). |
| `test_demone_documenti.py` | Leggere un documento, dal demone. |
| `test_demone_elevato.py` | Un comando confinato non riceve mai i poteri dell'amministratore, nemmeno da un demone elevato: o parte senza (gruppo non attivo, cartella degli Amministratori non scrivibile, `write_roots` si') o il demone rifiuta e lo dice (D369). Va lanciata da amministratore; da utente normale, e in CI, esce 2: la guardia in CI e' la prova Rust `windows_un_comando_non_riceve_i_poteri_dell_amministratore`. |
| `test_demone_fascicolo.py` | Il fascicolo, il registro dichiarato e «dove sono i miei dati», nel demone (D352). |
| `test_demone_file.py` | Gli strumenti sui file, dentro il demone, con il modo di tornare indietro. |
| `test_demone_harness.py` | Le proposte di NOVA nell'harness, dal demone (D339). |
| `test_demone_harness_strumenti.py` | Gli strumenti `harness_*` del demone contro quelli del Python (D344). |
| `test_demone_impara.py` | A turno finito il demone impara i fatti durevoli, come `memory.py`. |
| `test_demone_memoria.py` | Il demone scrive nella memoria, e quello che scrive lo rilegge NOVA. |
| `test_demone_modello_locale.py` | Il demone accende il modello di casa quando un turno ne ha bisogno (D358). |
| `test_demone_permessi.py` | Prima di agire si chiede: il turno del demone e la porta MCP. |
| `test_demone_recinto.py` | Il confine vale anche **dopo** che il comando e' partito. Su Windows anche l'elenco delle voci, la cartella di lavoro e `--togli` (D367). |
| `test_demone_recinto_strumenti.py` | Gli strumenti installati nel profilo partono nel recinto solo con `tool_roots`. Dove la cartella e' degli amministratori la prova lo dice e esce 2 (D367). |
| `test_demone_registro.py` | Quello che fa il demone finisce nello stesso registro di quello che fa NOVA. |
| `test_demone_rete.py` | Il web senza browser, le cartelle note e le procedure, dal demone. |
| `test_demone_ricerca.py` | La ricerca del demone, col browser senza finestra, con un browser vero. |
| `test_demone_riparazione.py` | NOVA si ripara in Rust: banco, prove, binari nuovi, e ritorno (D349). |
| `test_demone_schermo.py` | Lo schermo in un'immagine, dal demone. |
| `test_demone_semina.py` | La prima mappatura del PC, fatta dal demone quando si accende (D365). |
| `test_demone_sistema.py` | Gli appunti, il volume, le notifiche, l'ora e com'e' fatto il PC, dal demone. |
| `test_demone_turno.py` | Il demone fa un turno intero da solo: chiede, esegue, risponde; e lascia la decisione, senza segreti. |
| `test_demone_web.py` | Il browser di NOVA, guidato dal demone, con un browser vero. |

## `prove/macchina/`: 16 prove

Cosa serve: una macchina vera: Windows, uno schermo, l'audio, Chrome.

| prova | cosa controlla |
|---|---|
| `test_app_finestre.py` | Avviare, portare davanti e chiudere: senza modelli di ricerca. |
| `test_app_installate.py` | L'elenco delle applicazioni, chiesto al registro invece che a PowerShell. |
| `test_appunti.py` | Gli appunti chiamati direttamente, senza shell in mezzo. |
| `test_browser_blocco.py` | Le due mani nuove sul browser: incollare un blocco e consegnare un file. |
| `test_cestino.py` | Cancellare in un modo che si puo' disfare, anche se il nome ha un apostrofo. |
| `test_finestre.py` | Le finestre aperte, chieste a Windows invece che a PowerShell. |
| `test_modello_flag.py` | Come si accende il modello: i flag che cambiano il doppio dei numeri. |
| `test_notifiche.py` | Una notifica non deve costare nove secondi di NOVA. |
| `test_pianifica.py` | NOVA si da' appuntamento con se stessa, e l'istruzione arriva intera. |
| `test_powershell.py` | Un posto solo da cui si chiama PowerShell, e una codifica sola. |
| `test_promemoria.py` | Un promemoria che parte davvero, e senza righe di comando annidate. |
| `test_schede.py` | La memoria video si legge anche se la scheda non e' NVIDIA. |
| `test_scrittura_senza_tastiera.py` | Scrivere dentro un campo **senza toccare la tastiera e senza il fuoco**. |
| `test_sistema.py` | Com'e' fatto il PC, chiesto al sistema invece che a una query. |
| `test_tastiera.py` | Premere i tasti sapendo dove finiscono. |
| `test_volume.py` | Il volume chiesto a chi lo tiene, invece che premuto a colpi di tasto. |
