//! Le trentatre' dichiarazioni degli strumenti che NOVA apre a un altro
//! programma.
//!
//! **Generato da `_estrai_mcp.py`, poi mantenuto a mano.** Sono diciottomila
//! caratteri di schema che Claude Code rilegge a ogni sessione e su cui
//! sceglie quale strumento di NOVA usare: una parola diversa e' un
//! comportamento diverso che nessun tipo intercetta (D112). Ricopiarle
//! sarebbe stato trentatre' occasioni di sbagliarne una.
//!
//! Si tengono come **testo**, non come struttura, per la stessa ragione per
//! cui il banco degli strumenti interni confronta il testo e non l'albero:
//! e' il testo che finisce nel prompt di chi riceve, e l'ordine delle chiavi
//! ne fa parte.

/// La versione del protocollo che NOVA dichiara.
pub const PROTOCOLLO: &str = "2024-11-05";

/// Le dichiarazioni, cosi' come le scrive il Python.
pub const STRUMENTI_JSON: &str = r#"[
  {
    "name": "kb_search",
    "description": "Cerca nella memoria a lungo termine di NOVA (knowledge base a grafo): profilo dell'utente, preferenze, progetti, persone, fatti appresi. Usalo prima di chiedere qualcosa che potrebbe essere gia' noto.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "query": {
          "type": "string",
          "description": "Cosa cercare"
        },
        "top_k": {
          "type": "integer",
          "description": "Quanti nodi (default 5)"
        }
      },
      "required": [
        "query"
      ]
    }
  },
  {
    "name": "delega",
    "description": "Passa un compito a un modello piu' capace di te e ricevi la risposta. Usalo quando il compito lo merita: ragionamenti difficili, codice delicato, decisioni che pesano. Scrivi il compito per intero, perche' chi lo riceve non vede questa conversazione, e passa i percorsi dei file in «file» invece di ricopiarne il contenuto.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "a": {
          "type": "string",
          "description": "Gradino a cui delegare, es. 'difficile'"
        },
        "compito": {
          "type": "string",
          "description": "Il compito, autoconsistente"
        },
        "motivo": {
          "type": "string",
          "description": "Perche' non lo fai tu"
        },
        "contesto": {
          "type": "string",
          "description": "Vincoli e dati brevi"
        },
        "file": {
          "type": "array",
          "items": {
            "type": "string"
          },
          "description": "Percorsi da allegare"
        }
      },
      "required": [
        "a",
        "compito"
      ]
    }
  },
  {
    "name": "modelli",
    "description": "Elenca i gradini disponibili e il loro stato.",
    "inputSchema": {
      "type": "object",
      "properties": {},
      "required": []
    }
  },
  {
    "name": "kb_note",
    "description": "Salva o aggiorna un nodo nella memoria di NOVA. Usalo quando emerge un'informazione durevole sull'utente, sul suo lavoro o sulle sue preferenze.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "titolo": {
          "type": "string",
          "description": "Titolo breve (2-6 parole)"
        },
        "testo": {
          "type": "string",
          "description": "Contenuto autoconsistente"
        },
        "tipo": {
          "type": "string",
          "description": "profilo|preferenza|progetto|app|persona|abitudine|fatto"
        },
        "tags": {
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "relazioni": {
          "type": "array",
          "items": {
            "type": "string"
          },
          "description": "Slug di nodi a cui collegarlo"
        }
      },
      "required": [
        "titolo",
        "testo"
      ]
    }
  },
  {
    "name": "chiedi_permesso",
    "description": "Chiede all'utente il permesso di eseguire un'azione e ne aspetta la risposta. Lo chiama Claude Code da solo quando incontra un'azione che richiede conferma: non va invocato a mano.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "tool_name": {
          "type": "string"
        },
        "input": {
          "type": "object"
        },
        "tool_use_id": {
          "type": "string"
        }
      },
      "required": [
        "tool_name",
        "input"
      ]
    }
  },
  {
    "name": "web_apri",
    "description": "Apre un indirizzo nel browser di NOVA e restituisce l'identificativo della scheda, da passare agli altri strumenti web. E' un browser suo, con un profilo separato da quello dell'utente: la prima volta su un sito puo' servire un accesso.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "url": {
          "type": "string",
          "description": "L'indirizzo"
        }
      },
      "required": [
        "url"
      ]
    }
  },
  {
    "name": "web_trova",
    "description": "Cerca elementi in una pagina con un selettore CSS e restituisce tag, id, ruolo, aria-label e testo di ognuno. E' il modo giusto di guardare una pagina web: costa centesimi di secondo, mentre l'albero di accessibilita' va percorso un livello per volta. Gli id sono quelli che si vedrebbero con «Ispeziona» del browser.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "selettore": {
          "type": "string",
          "description": "Selettore CSS PURO, es. '#docs-file-menu'. Niente :has-text(), che e' di Playwright e in CSS non esiste"
        },
        "testo": {
          "type": "string",
          "description": "Cerca per quello che c'e' scritto sopra. Si puo' usare col selettore: il selettore restringe, il testo sceglie"
        },
        "scheda": {
          "type": "string",
          "description": "Identificativo dato da web_apri"
        },
        "quanti": {
          "type": "integer",
          "description": "Massimo risultati (default 20)"
        }
      }
    }
  },
  {
    "name": "web_leggi",
    "description": "Il testo visibile della pagina, con titolo e indirizzo.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "scheda": {
          "type": "string",
          "description": "Identificativo dato da web_apri"
        },
        "caratteri": {
          "type": "integer",
          "description": "Quanti caratteri (default 6000)"
        }
      }
    }
  },
  {
    "name": "web_click",
    "description": "Preme un elemento della pagina: con un selettore CSS, oppure con «testo», cioe' quello che c'e' scritto sopra - «ACCETTO», «Accedi». Per i banner dei cookie e i bottoni senza id «testo» e' la strada corta. Manda la sequenza intera di eventi del mouse, perche' i menu delle applicazioni web spesso ascoltano mousedown e non click.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "selettore": {
          "type": "string",
          "description": "Selettore CSS PURO. Niente :has-text(), che in CSS non esiste"
        },
        "testo": {
          "type": "string",
          "description": "Il testo scritto sull'elemento. Preme il piu' interno che lo contiene"
        },
        "scheda": {
          "type": "string",
          "description": "Identificativo dato da web_apri"
        }
      }
    }
  },
  {
    "name": "web_scrivi",
    "description": "Scrive in un campo della pagina. Per una PASSWORD non usare «testo»: usa «segreto» con il nome della credenziale nell'archivio, e il valore va dall'archivio al campo senza passare da te - non finisce nella conversazione e nessuno puo' estrarlo.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "selettore": {
          "type": "string",
          "description": "Selettore CSS del campo"
        },
        "testo": {
          "type": "string",
          "description": "Testo da scrivere (mai una password)"
        },
        "segreto": {
          "type": "string",
          "description": "Nome di una credenziale in archivio: si scrive il suo valore"
        },
        "scheda": {
          "type": "string",
          "description": "Identificativo dato da web_apri"
        }
      },
      "required": [
        "selettore"
      ]
    }
  },
  {
    "name": "harness_apri",
    "description": "Apre un documento nell'harness: il documento sta a sinistra, la conversazione resta qui. Apre .pdf .docx .txt .md. Da usare quando il lavoro ha un POSTO che dura piu' di un turno - studiare un documento, controllarlo, cercarci dentro. All'harness il materiale, alla chat il verdetto: qui dentro scrivi due righe, non il rapporto.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "percorso": {
          "type": "string",
          "description": "Percorso del documento"
        },
        "profilo": {
          "type": "string",
          "description": "Per ora solo «studio» (sola lettura)"
        }
      },
      "required": [
        "percorso"
      ]
    }
  },
  {
    "name": "harness_cerca",
    "description": "Dove sta, nel documento aperto, quello che si sta cercando. Torna una POSIZIONE - identificativo del blocco, pagina, testo - e la fa evidenziare a sinistra. Rispondi citando quella posizione: «lo trovi a pagina 12». Se non c'e', dillo: qui non si deduce, si indica.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "domanda": {
          "type": "string",
          "description": "Cosa cercare"
        },
        "quanti": {
          "type": "integer",
          "description": "Quanti punti (default 5)"
        }
      },
      "required": [
        "domanda"
      ]
    }
  },
  {
    "name": "harness_leggi",
    "description": "Il testo attorno a un punto del documento, per capire in che contesto quella cosa sta. Senza «intorno» da' l'inizio.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "intorno": {
          "type": "string",
          "description": "Identificativo di blocco dato da harness_cerca"
        },
        "blocchi": {
          "type": "integer",
          "description": "Quanti blocchi prima e dopo (default 3)"
        }
      }
    }
  },
  {
    "name": "harness_stato",
    "description": "Cosa c'e' aperto nell'harness adesso, e cosa e' evidenziato.",
    "inputSchema": {
      "type": "object",
      "properties": {}
    }
  },
  {
    "name": "harness_cerca_progetto",
    "description": "Cerca in TUTTI i file aperti come progetto, non solo in quello che si sta guardando. Serve quando la pila e' piu' alta di un documento: sei PDF di un esame, una documentazione, il codice di un progetto. Torna file + blocco + pagina, cioe' un posto che si puo' controllare. Poi con harness_apri vai sul file giusto e con harness_cerca ti fermi sul punto.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "domanda": {
          "type": "string",
          "description": "Cosa cerchi, a parole tue"
        },
        "quanti": {
          "type": "integer",
          "description": "Quanti risultati (default 8)"
        }
      },
      "required": [
        "domanda"
      ]
    }
  },
  {
    "name": "harness_proponi",
    "description": "Cambia il documento aperto - ma non subito: la modifica compare nella finestra con il prima e il dopo, e l'utente sceglie se applicarla. QUESTO E' IL MODO DI SCRIVERE in un documento suo. I blocchi si prendono da harness_cerca o harness_leggi. Su .md e .txt e su .docx: sostituisci, prima, dopo, elimina. Su .pdf il testo non si riscrive - le lettere stanno in un punto della pagina, non in paragrafi - ma si puo' evidenzia e nota. Dopo aver proposto DILLO e fermati: applicare non tocca a te.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "modifiche": {
          "type": "array",
          "description": "Una per ogni punto da cambiare",
          "items": {
            "type": "object",
            "properties": {
              "blocco": {
                "type": "string",
                "description": "Identificativo del blocco (es. r12, p3, p0b4)"
              },
              "azione": {
                "type": "string",
                "description": "sostituisci | prima | dopo | elimina | evidenzia | nota"
              },
              "testo": {
                "type": "string",
                "description": "Il testo nuovo (non serve per elimina/evidenzia)"
              }
            },
            "required": [
              "blocco"
            ]
          }
        },
        "motivo": {
          "type": "string",
          "description": "Una riga sul perche', che l'utente legge accanto ai bottoni"
        }
      },
      "required": [
        "modifiche"
      ]
    }
  },
  {
    "name": "harness_applica",
    "description": "Applica la proposta in attesa. Usalo SOLO se l'utente lo ha chiesto dopo averla vista: di norma il bottone lo preme lui. Su codice passa verifica=true: prova il progetto prima e dopo, e se cade qualcosa che prima passava rimette il file com'era.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "verifica": {
          "type": "boolean",
          "description": "Prova i test del progetto e applica solo se non peggiora niente. Su codice, si'."
        }
      }
    }
  },
  {
    "name": "harness_prova",
    "description": "Esegue i test del progetto aperto e dice cosa passa e cosa cade. Serve per sapere da che punto si parte prima di toccare il codice, e per raccontare all'utente come sta il progetto. Riconosce da solo come si prova: cargo, npm, go, pytest, oppure gli script test_*.py.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "file": {
          "type": "string",
          "description": "Il file su cui stai lavorando: serve a scegliere quale suite provare invece di provarle tutte."
        }
      }
    }
  },
  {
    "name": "harness_scarta",
    "description": "Butta via la proposta in attesa senza applicarla.",
    "inputSchema": {
      "type": "object",
      "properties": {}
    }
  },
  {
    "name": "fascicolo",
    "description": "Cosa c'e' nel fascicolo dell'utente: CV, esperienze, progetti, testi che ha scritto lui. GUARDA QUI PRIMA di scrivere qualcosa a nome suo - una candidatura, una lettera, una biografia. I fatti si prendono da qui; quello che qui non c'e' SI CHIEDE, non si deduce: un'esperienza inventata non e' un errore, e' una dichiarazione falsa con sopra la firma dell'utente.",
    "inputSchema": {
      "type": "object",
      "properties": {}
    }
  },
  {
    "name": "fascicolo_leggi",
    "description": "Legge un file del fascicolo come testo. Apre .txt .md .pdf .docx .xlsx .csv .json. Serve anche per il TONO: chi ha gia' scritto tre lettere ne ha gia' la voce, e ricopiarla e' meglio che immaginarla.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "nome": {
          "type": "string",
          "description": "Nome del file come lo da' «fascicolo»"
        },
        "caratteri": {
          "type": "integer",
          "description": "Quanti caratteri (default 8000)"
        }
      },
      "required": [
        "nome"
      ]
    }
  },
  {
    "name": "pianifica_crea",
    "description": "Mette in calendario un'automazione GIA' ESISTENTE, perche' parta da sola. «quando»: «ogni giorno 08:00», «ogni lunedi 09:00», «ogni 30 minuti», «ogni ora». Con sentinella=true non esegue e basta: guarda il risultato e lascia un avviso solo se e' CAMBIATO rispetto alla volta prima - e' il modo di accorgersi di una risposta arrivata, di un prezzo sceso, di un file diverso. La prima volta registra da se' l'attivita' di sistema che fa partire tutto.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "nome": {
          "type": "string",
          "description": "Come chiamarla"
        },
        "automazione": {
          "type": "string",
          "description": "Nome di un'automazione esistente"
        },
        "quando": {
          "type": "string",
          "description": "«ogni giorno 08:00», «ogni 30 minuti», ..."
        },
        "dati": {
          "type": "object",
          "description": "Parametri da passarle"
        },
        "sentinella": {
          "type": "boolean",
          "description": "Avvisa solo se il risultato cambia"
        },
        "guarda": {
          "type": "string",
          "description": "Quale campo del risultato guardare (vuoto = tutto)"
        }
      },
      "required": [
        "nome",
        "automazione",
        "quando"
      ]
    }
  },
  {
    "name": "pianifica_elenco",
    "description": "Cosa parte da solo, quando, e com'e' andata l'ultima volta.",
    "inputSchema": {
      "type": "object",
      "properties": {}
    }
  },
  {
    "name": "pianifica_elimina",
    "description": "Toglie una voce dal calendario (l'automazione resta).",
    "inputSchema": {
      "type": "object",
      "properties": {
        "nome": {
          "type": "string"
        }
      },
      "required": [
        "nome"
      ]
    }
  },
  {
    "name": "avvisi_recenti",
    "description": "Gli avvisi lasciati dalle sentinelle mentre nessuno guardava. Da leggere quando l'utente torna e chiede «novita'?».",
    "inputSchema": {
      "type": "object",
      "properties": {
        "quanti": {
          "type": "integer"
        }
      }
    }
  },
  {
    "name": "azione_registra",
    "description": "Annota un'azione CHE NON SI PUO' ANNULLARE, appena l'hai fatta: una mail inviata, una candidatura mandata, un modulo inoltrato, un acquisto, una cancellazione, una pubblicazione. Non chiede permesso e non ferma niente - serve perche' l'utente possa sapere cosa e' partito e a chi, anche se non stava guardando. Scrivilo con parole sue, non con nomi di selettori.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "azione": {
          "type": "string",
          "description": "Cosa hai fatto, in una riga: «inviata candidatura per X»"
        },
        "dove": {
          "type": "string",
          "description": "A chi o dove: destinatario, azienda, sito"
        },
        "dettagli": {
          "type": "string",
          "description": "Quel che serve a ricostruire: oggetto, importo, file allegato"
        }
      },
      "required": [
        "azione"
      ]
    }
  },
  {
    "name": "dati_dove",
    "description": "Dove NOVA tiene le cose dell'utente - credenziali, fascicolo, memoria, registro, configurazione - quanto pesano e cosa succede se le cancella. Rispondi con questo a «dove sono i miei dati?», «cosa sai di me?», «come faccio a cancellare tutto?»: sono domande di fiducia, e una risposta vaga vale come un no.",
    "inputSchema": {
      "type": "object",
      "properties": {}
    }
  },
  {
    "name": "azioni_recenti",
    "description": "Rilegge il registro delle azioni irreversibili. Serve a rispondere a «cosa hai fatto?» senza ricostruirlo a memoria - la memoria di una sessione chiusa non c'e' piu', il registro si'. Con «cerca» risponde anche a «cosa avevo mandato a quella societa'?» settimane dopo: le parole si trovano in qualunque campo, senza accenti e senza maiuscole.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "cerca": {
          "type": "string",
          "description": "Parole da cercare fra le azioni. Vuoto = le ultime."
        },
        "quante": {
          "type": "integer",
          "description": "Quante righe (default 30)"
        },
        "ore": {
          "type": "number",
          "description": "Solo le ultime N ore (0 = tutte)"
        },
        "tipo": {
          "type": "string",
          "description": "browser | documento | ... per restringere"
        }
      }
    }
  },
  {
    "name": "web_cerca",
    "description": "Cerca in rete e torna titolo, indirizzo e riassunto dei risultati. NON apre nessuna finestra: usa un browser senza schermo, su un profilo suo. E' il PREAMBOLO: prima si cerca dove andare, poi si apre. Andare su google con web_apri per cercare sono quattro chiamate al posto di una. Attenzione: la domanda esce dal computer, quindi non metterci dati dell'utente.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "domanda": {
          "type": "string",
          "description": "Cosa cercare"
        },
        "quanti": {
          "type": "integer",
          "description": "Quanti risultati (default 8)"
        }
      },
      "required": [
        "domanda"
      ]
    }
  },
  {
    "name": "web_prendi",
    "description": "Scarica una pagina e la restituisce come testo, senza browser: mezzo secondo invece di sei. Per tutto cio' che sta fermo - un articolo, una documentazione, un JSON, un elenco. Il browser serve per AGIRE (accedere, compilare, premere, incollare) e per le pagine che senza JavaScript non esistono; per leggere, questo.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "url": {
          "type": "string",
          "description": "Indirizzo http o https"
        },
        "caratteri": {
          "type": "integer",
          "description": "Quanti caratteri (default 6000)"
        }
      },
      "required": [
        "url"
      ]
    }
  },
  {
    "name": "web_tabella",
    "description": "Una tabella intera della pagina, gia' pronta come TSV - tabulazioni fra le colonne, a capo fra le righe. UNA chiamata al posto di dieci con web_trova: non tastare una tabella un selettore per volta. Senza selettore prende quella con piu' testo nella pagina. Legge anche le griglie fatte di div con i ruoli ARIA. Quello che torna e' gia' nella forma che web_incolla si aspetta.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "selettore": {
          "type": "string",
          "description": "Selettore CSS della tabella. Vuoto = la piu' grande della pagina"
        },
        "righe": {
          "type": "integer",
          "description": "Massimo righe (default 400)"
        },
        "scheda": {
          "type": "string",
          "description": "Identificativo dato da web_apri"
        }
      }
    }
  },
  {
    "name": "web_incolla",
    "description": "Mette un BLOCCO INTERO di testo nella pagina in una mossa sola. Per una tabella usa le tabulazioni fra le colonne e gli a capo fra le righe: i fogli di calcolo le spacchettano da soli in celle. E' la differenza fra una chiamata e quaranta: non scrivere mai una tabella cella per cella con web_scrivi.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "testo": {
          "type": "string",
          "description": "Il blocco. Tabulazioni fra le colonne, a capo fra le righe"
        },
        "selettore": {
          "type": "string",
          "description": "Dove incollare. Vuoto = dove sta il fuoco nella pagina"
        },
        "scheda": {
          "type": "string",
          "description": "Identificativo dato da web_apri"
        }
      },
      "required": [
        "testo"
      ]
    }
  },
  {
    "name": "web_carica",
    "description": "Consegna un file gia' pronto a un campo di caricamento della pagina (input di tipo file). Non apre nessuna finestra di dialogo e non tocca mouse ne' tastiera. E' la strada piu' corta per far entrare una tabella in un servizio web: scrivi un CSV su disco e caricalo, invece di riempire il modulo a mano.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "selettore": {
          "type": "string",
          "description": "Selettore CSS dell'input file"
        },
        "percorsi": {
          "type": "array",
          "items": {
            "type": "string"
          },
          "description": "Percorsi assoluti dei file da consegnare"
        },
        "scheda": {
          "type": "string",
          "description": "Identificativo dato da web_apri"
        }
      },
      "required": [
        "selettore",
        "percorsi"
      ]
    }
  }
]"#;
