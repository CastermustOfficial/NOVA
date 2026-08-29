/* I nomi e i titoli dell'interfaccia, nelle lingue in cui NOVA li sa dire.
 *
 * La chiave e' il testo italiano, non un identificatore. E' una scelta, e la
 * ragione e' semplice: con le chiavi astratte (`btn.ferma.title`) una voce
 * dimenticata lascia sullo schermo la chiave, o il vuoto. Con l'italiano come
 * chiave, una voce dimenticata lascia l'italiano - che sara' pure la lingua
 * sbagliata, ma e' una frase di senso compiuto, e chi traduce vede subito cosa
 * manca senza dover aprire due file in parallelo.
 *
 * Il prompt del modello NON passa di qui: al cervello si dice in che lingua
 * rispondere (nova/lingue.py) e fa lui. Qui c'e' solo cio' che nessun modello
 * vede: bottoni, etichette, note.
 *
 * Aggiungere una lingua = aggiungere un oggetto qui sotto, e la voce
 * corrispondente in nova/lingue.py.
 */

export const LINGUE = {
  en: {
    /* --- chat --- */
    'a riposo': 'idle',
    'ti ascolto': 'listening',
    'ci penso': 'thinking',
    'sto facendo': 'working',
    'sto parlando': 'speaking',
    'aspetto un tuo ok': 'waiting for your OK',
    'demone spento': 'daemon stopped',
    'Dimmi pure.': 'Go ahead.',
    'Scrivi qui, oppure chiamami dicendo «Nova».': 'Type here, or call me by saying "Nova".',
    'Ferma quello che sta facendo (Esc)': 'Stop what it is doing (Esc)',
    'Ricomincia da capo': 'Start over',
    'Impostazioni': 'Settings',
    'Chiudi (Esc)': 'Close (Esc)',
    'Invia (Invio)': 'Send (Enter)',
    'Scrivi a NOVA…': 'Write to NOVA…',
    'nuova conversazione — il filo di prima è chiuso': 'new conversation — the previous thread is closed',
    'ti ascolto — il nome non serve più': 'listening — you do not need my name any more',
    'in pausa — di’ «Nova» per riprendere': 'paused — say "Nova" to resume',
    'conversazione chiusa': 'conversation closed',
    'il microfono non consegna niente': 'the microphone is delivering nothing',
    'microfono di nuovo attivo': 'microphone active again',
    'voce locale: ': 'local voice: ',

    /* --- impostazioni: intestazione --- */
    'NOVA — Impostazioni': 'NOVA — Settings',
    'IMPOSTAZIONI': 'SETTINGS',
    'SALVATO': 'SAVED',
    'JS NON PARTITO': 'JS DID NOT START',

    /* --- cervello --- */
    'Cervello': 'Brain',
    'Modello locale': 'Local model',
    'Il GGUF sul PC. Gratis, privato, orchestra.': 'The GGUF on this PC. Free, private, orchestrates.',
    'Claude Code': 'Claude Code',
    'Agentico: agisce con i propri strumenti.': 'Agentic: acts with its own tools.',
    'API esterna': 'External API',
    'Endpoint compatibile OpenAI.': 'OpenAI-compatible endpoint.',
    'A CONSUMO': 'PAY PER USE',
    'Lingua': 'Language',
    'In che lingua NOVA risponde e scrive i suoi menu': 'The language NOVA answers in, and writes its menus in',

    /* --- autonomia --- */
    'Autonomia': 'Autonomy',
    'Conferma sempre': 'Always confirm',
    'Solo rischiose': 'Risky only',
    'Autonomo': 'Autonomous',
    'Nessuna conferma. Veloce, e ogni errore è già successo quando lo leggi.':
      'No confirmation. Fast, and every mistake has already happened by the time you read it.',
    "Con il cervello Claude la conferma arriva da una finestra a parte: NOVA chiede il permesso passando dal demone, e finche' non rispondi Claude aspetta.":
      'With the Claude brain, confirmation comes from a separate window: NOVA asks permission through the daemon, and Claude waits until you answer.',

    /* --- memoria --- */
    'Memoria': 'Memory',
    'Impara da sola': 'Learns on its own',
    'Dopo ogni scambio NOVA estrae i fatti durevoli': 'After each exchange NOVA extracts the lasting facts',
    'Contesto nel prompt': 'Context in the prompt',
    'Cerca nella memoria prima di rispondere': 'Searches memory before answering',
    "Quello che NOVA vede sullo schermo non entra mai in memoria: legge i titoli delle finestre per poter agire sui programmi, ma nel vault finisce l'applicazione, non cosa ci avevi aperto dentro.":
      'What NOVA sees on screen never enters memory: it reads window titles so it can act on programs, but what lands in the vault is the application, not what you had open inside it.',

    'Impara le procedure': 'Learns procedures',
    'Come ha risolto una richiesta, per rifarla senza ricercare':
      'How it solved a request, so it can repeat it without searching again',

    /* --- voce --- */
    'Voce': 'Voice',
    'Attiva': 'On',
    'Ascolto e sintesi': 'Listening and speech',
    'Motore': 'Engine',
    'Locale — Kokoro, nel demone': 'Local — Kokoro, in the daemon',
    'ElevenLabs — cloud, 10.000 caratteri al mese': 'ElevenLabs — cloud, 10,000 characters a month',
    'Voce di sistema — gratis, illimitata, meccanica': 'System voice — free, unlimited, robotic',
    'Nessuna': 'None',
    "Gira nel demone: nessun tetto, e l'audio non esce dal PC.":
      'Runs in the daemon: no cap, and the audio never leaves the PC.',
    'Voce migliore, ma il piano gratuito dà 10.000 caratteri al mese.':
      'Better voice, but the free plan gives 10,000 characters a month.',
    'La voce di Windows: sempre disponibile, timbro meccanico.':
      'The Windows voice: always available, robotic timbre.',
    'NOVA non parla.': 'NOVA does not speak.',
    'Microfono': 'Microphone',
    'Il predefinito di sistema spesso non è quello giusto: provalo.':
      'The system default is often not the right one: test it.',
    'Predefinito di sistema': 'System default',
    "Prova · di' qualcosa": 'Test · say something',
    'Non ancora provato.': 'Not tested yet.',
    'Cambiato: provalo.': 'Changed: test it.',
    'In ascolto…': 'Listening…',
    'Italiana e nativa, senza tetto di caratteri': 'Native voice, no character cap',
    'im_nicola — maschile': 'im_nicola — male',
    'if_sara — femminile': 'if_sara — female',
    'Memoria del motore': 'Engine memory',
    'Tenerlo caldo costa ~600 MB e fa partire la voce all’istante':
      'Keeping it warm costs ~600 MB and makes the voice start instantly',
    'Resta caricato (predefinito)': 'Stay loaded (default)',
    'Scarica dopo 5 minuti di silenzio': 'Unload after 5 minutes of silence',
    'Scarica dopo 15 minuti': 'Unload after 15 minutes',
    "Scarica dopo un'ora": 'Unload after an hour',
    'Parola di risveglio': 'Wake word',
    'Il microfono resta aperto e si sveglia su «': 'The microphone stays open and wakes on "',

    /* --- componenti --- */
    'Componenti': 'Components',
    'Cosa serve a ogni funzione, e cosa manca': 'What each feature needs, and what is missing',
    'Scarica': 'Download',
    'Presente': 'Installed',
    'Manca': 'Missing',
    'Scarico…': 'Downloading…',
    'Ferma': 'Stop',
    'Fatto.': 'Done.',
    'Scaricamento interrotto.': 'Download stopped.',

    /* --- legenda dell'orb e code delle sezioni --- */
    'ASCOLTO': 'LISTENING',
    'PENSO': 'THINKING',
    'PARLO': 'SPEAKING',
    'AGISCO': 'ACTING',
    'CHIEDO': 'ASKING',
    'SPENTA': 'OFF',
    'ATTIVA': 'ON',
    'IMPARA': 'LEARNING',
    'FERMA': 'PAUSED',

    /* --- autonomia, per esteso --- */
    'Ogni azione passa dal tuo benestare, anche quelle innocue. Lento, ma non succede niente che tu non abbia visto.':
      'Every action goes through your approval, even the harmless ones. Slow, but nothing happens that you have not seen.',
    'Legge e cerca da sola; chiede solo prima di scrivere, cancellare o eseguire. È il compromesso ragionevole.':
      'It reads and searches on its own; it asks only before writing, deleting or running. The reasonable compromise.',

    /* --- tendina della lingua --- */
    'menu in italiano': 'menus in Italian',
    'NOVA risponde in questa lingua e i menu sono tradotti.':
      'NOVA answers in this language and the menus are translated.',
    'NOVA risponde in questa lingua, ma i nomi e i titoli restano in italiano: manca il dizionario.':
      'NOVA answers in this language, but names and titles stay in Italian: the dictionary is missing.',
    'Il prompt di sistema resta in italiano: e\' il sorgente di NOVA, non un testo da leggere, e al modello basta dirgli in che lingua rispondere. I nomi e i titoli invece si traducono davvero, perche\' li\' non c\'e\' nessun modello in mezzo.':
      'The system prompt stays in Italian: it is NOVA\'s source code, not something to read, and the model only needs to be told which language to answer in. Names and titles are genuinely translated, because there no model sits in between.',
    'Il demone non risponde: ': 'The daemon is not answering: ',

    'DA SCARICARE': 'TO DOWNLOAD',
    'AL COMPLETO': 'COMPLETE',
    'Senza:': 'Without it:',
    'comincio\u2026': 'starting\u2026',
    'estraggo': 'extracting',
    'apro il pacchetto': 'opening the package',
    'Non ce l\'ho fatta:': 'It did not work:',
    'Non riesco a leggere i componenti: ': 'I cannot read the components: ',
    'Il ponte col guscio non c\'e\': non posso guardare i componenti.':
      'The bridge to the shell is missing: I cannot look at the components.',
    'Quello che manca si scarica da qui. Prima si poteva solo cambiare il menu: chi sceglieva la voce locale senza averne i file restava muto, e l\u2019unico programma capace di procurarli era l\'installer.':
      'Whatever is missing gets downloaded here. Before, all you could change was the menu: choosing the local voice without its files left you mute, and the only program able to fetch them was the installer.',

    /* nomi dei componenti: arrivano da nova/componenti.py, in italiano */
    'Voce locale (Kokoro)': 'Local voice (Kokoro)',
    'ONNX Runtime': 'ONNX Runtime',
    'espeak-ng (fonemi)': 'espeak-ng (phonemes)',
    'Ascolto locale (whisper.cpp)': 'Local listening (whisper.cpp)',
    'Far parlare NOVA senza che l\'audio esca dal PC':
      'Let NOVA speak without any audio leaving the PC',
    'NOVA scrive ma non parla, a meno di usare ElevenLabs o la voce di Windows':
      'NOVA writes but does not speak, unless you use ElevenLabs or the Windows voice',
    'Eseguire Kokoro: il crate lo carica a runtime, non e\' collegato dentro':
      'Runs Kokoro: the crate loads it at runtime, it is not linked in',
    'La voce locale non parte nemmeno con i suoi modelli al posto giusto':
      'The local voice will not start even with its models in place',
    'Trasformare il testo in fonemi: senza, Kokoro non sa cosa pronunciare':
      'Turns text into phonemes: without it, Kokoro does not know what to pronounce',
    'NOVA capisce quello che dici ma non risponde a voce':
      'NOVA understands what you say but does not answer out loud',
    'GPLv3 - non ridistribuito, si scarica dalla fonte ufficiale':
      'GPLv3 - not redistributed, fetched from the official source',
    'Trascrivere quello che dici senza mandare l\'audio a nessuno':
      'Transcribes what you say without sending the audio to anyone',
    'L\'ascolto passa da ElevenLabs, quindi la tua voce esce dal PC':
      'Listening goes through ElevenLabs, so your voice leaves the PC',

    /* --- stato --- */
    'Stato': 'Status',
    'CONTROLLO…': 'CHECKING…',
    'TUTTO A POSTO': 'ALL GOOD',
    'DA GUARDARE': 'NEEDS A LOOK',
    'NON LEGGIBILE': 'UNREADABLE',
    '(nessuno)': '(none)',
    'A RIPOSO': 'IDLE',
    'Risvegliata.': 'Woken.',
    "Questi non sono i valori scritti nel file: sono i fatti, chiesti adesso. Un pannello serve soprattutto quando qualcosa e' rotto.":
      'These are not the values written in the file: they are the facts, asked for right now. A panel earns its keep when something is broken.',

    /* --- orb --- */
    "L'orb": 'The orb',
    'SEMPRE IN SCENA': 'ALWAYS ON STAGE',
    "L'orb e' l'unica cosa di NOVA sempre visibile, e parla per colore. Passaci sopra per vedere ogni stato.":
      'The orb is the only part of NOVA always visible, and it speaks in colour. Hover it to see every state.',
    'Per questo il resto dell’interfaccia non ha colori forti: se tutto fosse verde, il verde smetterebbe di voler dire «ti sto ascoltando».':
      'That is why the rest of the interface has no strong colours: if everything were green, green would stop meaning "I am listening".',
  },
};

/* Le lingue in cui NOVA sa rispondere. `interfaccia` dice se esiste anche il
 * dizionario qui sopra: dove manca, NOVA parla la tua lingua ma i menu
 * restano in italiano - ed e' meglio dirlo nella tendina che lasciarlo
 * scoprire dopo aver cambiato. L'elenco rispecchia nova/lingue.py.
 */
export const OFFERTE = [
  { codice: 'it', nome: 'Italiano' },
  { codice: 'en', nome: 'English' },
  { codice: 'es', nome: 'Espanol' },
  { codice: 'fr', nome: 'Francais' },
  { codice: 'de', nome: 'Deutsch' },
  { codice: 'pt', nome: 'Portugues' },
  { codice: 'nl', nome: 'Nederlands' },
  { codice: 'pl', nome: 'Polski' },
  { codice: 'ru', nome: 'Russkij' },
  { codice: 'zh', nome: 'Zhongwen' },
  { codice: 'ja', nome: 'Nihongo' },
].map(l => ({ ...l, interfaccia: l.codice === 'it' || LINGUE[l.codice] !== undefined }));

let corrente = 'it';

/** Il testo nella lingua scelta. Se manca, resta l'italiano. */
export function T(testo) {
  if (corrente === 'it') return testo;
  const d = LINGUE[corrente];
  if (!d) return testo;
  const chiave = String(testo);
  if (d[chiave] !== undefined) return d[chiave];
  // Le stringhe scritte nell'HTML arrivano con gli a capo e i rientri del
  // sorgente: si confronta anche la versione a spazi normalizzati, altrimenti
  // ogni frase su piu' righe risulterebbe non tradotta.
  const piatto = chiave.replace(/\s+/g, ' ').trim();
  return d[piatto] !== undefined ? d[piatto] : testo;
}

export function linguaCorrente() { return corrente; }

/** Chiede al guscio quale lingua e' configurata. Non si aspetta all'infinito. */
export async function avviaLingua(invoke) {
  if (!invoke) return corrente;
  try {
    const cfg = await Promise.race([
      invoke('config_leggi'),
      new Promise(r => setTimeout(() => r(null), 400)),
    ]);
    const l = cfg?.ui?.lingua;
    if (l && (l === 'it' || LINGUE[l])) corrente = l;
  } catch (_) { /* il guscio non c'e': si resta in italiano */ }
  return corrente;
}

export function impostaLingua(codice) {
  corrente = (codice === 'it' || LINGUE[codice]) ? codice : 'it';
  return corrente;
}

/* Attributi che l'utente legge. `value` no: e' un dato, non un'etichetta. */
const ATTRIBUTI = ['title', 'placeholder', 'aria-label'];

/**
 * Traduce il documento gia' scritto nell'HTML.
 *
 * Solo quello: il testo che arriva dopo - le battute della conversazione, le
 * risposte di NOVA, quello che scrive l'utente - non passa mai di qui. Un
 * traduttore che gira su tutto il DOM prima o poi riscrive una frase
 * dell'utente, e sarebbe un danno peggiore di un'etichetta in italiano.
 */
export function traduciDocumento(radice = document) {
  if (corrente === 'it') return;
  const camminatore = document.createTreeWalker(radice, NodeFilter.SHOW_TEXT, {
    acceptNode(n) {
      const p = n.parentElement;
      if (!p) return NodeFilter.FILTER_REJECT;
      const tag = p.tagName;
      if (tag === 'SCRIPT' || tag === 'STYLE') return NodeFilter.FILTER_REJECT;
      return n.nodeValue.trim() ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT;
    },
  });
  const nodi = [];
  while (camminatore.nextNode()) nodi.push(camminatore.currentNode);
  for (const n of nodi) {
    const grezzo = n.nodeValue;
    const tradotto = T(grezzo.trim());
    if (tradotto !== grezzo.trim()) {
      // si conservano gli spazi attorno: tolti, le frasi si incollano
      const pre = grezzo.match(/^\s*/)[0];
      const post = grezzo.match(/\s*$/)[0];
      n.nodeValue = pre + tradotto + post;
    }
  }
  for (const el of radice.querySelectorAll('[title],[placeholder],[aria-label]')) {
    for (const a of ATTRIBUTI) {
      const v = el.getAttribute(a);
      if (v) {
        const t = T(v);
        if (t !== v) el.setAttribute(a, t);
      }
    }
  }
  const titolo = document.querySelector('title');
  if (titolo) document.title = T(titolo.textContent);
}
