/* Lo sportello dei permessi, dentro una conversazione.
 *
 * Quando NOVA sta per fare qualcosa che il livello di autonomia dice di
 * chiedere, il demone mette la domanda allo sportello e aspetta. Fino a
 * qui non c'era un bottone per rispondere: l'orb diventava «aspetto un tuo
 * ok» e dopo dieci minuti la domanda scadeva, cioe' diventava un no che
 * nessuno aveva detto (D333). La carta dice cosa succede se si consente —
 * la frase, non il nome dello strumento — e i due bottoni chiamano la
 * stessa capacita' che chiamerebbe la voce o la riga di comando: se si
 * risponde altrove, la carta lo scopre dall'evento e si chiude da sola.
 *
 * Sta in un file suo perche' le conversazioni sono due finestre — la
 * nuvoletta e l'harness — e lo sportello e' uno: una carta scritta due
 * volte e' una carta che un giorno dice due cose diverse.
 */

/**
 * `discorso`: dove si appendono le carte. `prima()`: cosa fare prima di
 * appenderne una (togliere il benvenuto). `nota(testo)` e `fondo()`: come
 * quella finestra scrive una riga di servizio e scorre in fondo.
 */
export function sportello({ discorso, invoke, T, nota, fondo, prima = () => {} }) {
  const carte = new Map();

  function carta(r) {
    if (!r?.id || carte.has(r.id) || (r.origine && r.origine !== 'utente')) return;
    prima();
    const c = document.createElement('div');
    c.className = 'permesso';
    const t = document.createElement('b');
    t.textContent = r.rischio === 'dangerous'
      ? T('Posso farlo? È un’azione rischiosa.')
      : T('Posso farlo?');
    c.appendChild(t);
    const cosa = document.createElement('div');
    cosa.className = 'cosa';
    cosa.textContent = String(r.dettaglio || r.strumento || '');
    c.appendChild(cosa);
    const quale = document.createElement('div');
    quale.className = 'quale';
    quale.textContent = String(r.strumento || '');
    c.appendChild(quale);
    const bottoni = document.createElement('div');
    bottoni.className = 'bottoni';
    const si = document.createElement('button');
    si.className = 'si';
    si.textContent = T('Sì, fallo');
    const no = document.createElement('button');
    no.textContent = T('No');
    bottoni.append(si, no);
    c.appendChild(bottoni);
    const rispondi = async consenti => {
      si.disabled = no.disabled = true;
      try {
        const esito = await invoke('demone_chiama', {
          capacita: 'approvazione.rispondi', args: { id: r.id, consenti } });
        /* «ok: false» vuol dire che era gia' decisa o scaduta: lo dice
           l'evento, e la carta si chiude da li'. */
        if (esito?.ok === false) chiudi(r.id, esito.motivo || T('non più in attesa'));
      } catch (e) {
        si.disabled = no.disabled = false;
        nota(T('non sono riuscita a mandare la risposta: ') + String(e).slice(0, 80));
      }
    };
    si.onclick = () => rispondi(true);
    no.onclick = () => rispondi(false);
    carte.set(r.id, c);
    discorso.appendChild(c);
    fondo();
    /* Il fuoco resta dov'era, apposta: chi sta scrivendo e preme Invio non
       deve consentire un'azione che non ha ancora letto. */
  }

  function chiudi(id, testo) {
    const c = carte.get(id);
    if (!c) return;
    carte.delete(id);
    c.classList.add('deciso');
    c.querySelector('.bottoni')?.remove();
    const e = document.createElement('div');
    e.className = 'esito';
    e.textContent = testo;
    c.appendChild(e);
  }

  /** Una domanda fatta mentre la finestra era chiusa aspetta ancora. */
  async function inAttesa() {
    if (!invoke) return;
    try {
      const a = await invoke('demone_chiama', { capacita: 'approvazione.attese', args: {} });
      for (const r of a?.richieste || []) carta(r);
    } catch (e) { console.warn('permessi:', e); }
  }

  /** Gli eventi del demone che riguardano lo sportello. */
  function evento(e) {
    if (e.topic === 'approvazione.richiesta') carta(e.dati);
    if (e.topic === 'approvazione.decisa') {
      chiudi(e.dati?.id, e.dati?.consentito ? T('consentito') : T('negato'));
    }
    if (e.topic === 'approvazione.scaduta') {
      chiudi(e.dati?.id, T('nessuna risposta: l’ho considerato un no'));
    }
  }

  return { carta, chiudi, inAttesa, evento };
}
