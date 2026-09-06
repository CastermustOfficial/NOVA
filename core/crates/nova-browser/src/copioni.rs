//! Il JavaScript che NOVA fa girare **dentro la pagina dell'utente**.
//!
//! **Generato da `_estrai_copioni.py`, poi mantenuto a mano.** E' l'unica
//! parte di NOVA eseguita da un interprete che non e' nostro, su un documento
//! che non e' nostro: ricopiarla a mano sarebbe stato ottomila caratteri di
//! occasioni di cambiare un carattere in un'espressione regolare o in un
//! selettore, e quell'errore non darebbe un errore — darebbe l'elemento
//! sbagliato (D112).
//!
//! I `{}` sono i posti dove entrano gli argomenti. **Non si sostituiscono a
//! mano**: si passa da [`crate::dentro`], che li scrive come li scrive
//! `json.dumps` — ed e' li' che passa il confine fra un argomento e del
//! codice.


/// Gli elementi che corrispondono a un selettore CSS.
pub const TROVA: &str = r#"
(() => {
  const q = %s;
  const nodi = Array.from(document.querySelectorAll(q)).slice(0, %d);
  return nodi.map(n => ({
    tag: n.tagName.toLowerCase(),
    id: n.id || null,
    ruolo: n.getAttribute('role') || null,
    etichetta: n.getAttribute('aria-label') || null,
    testo: (n.innerText || n.value || '').trim().slice(0, 120) || null,
    visibile: !!(n.offsetWidth || n.offsetHeight || n.getClientRects().length),
  }));
})()
"#;

/// Preme su un elemento. La sequenza intera di eventi e' il punto: i menu
/// delle applicazioni web spesso ascoltano `mousedown`, non `click`.
pub const CLICCA: &str = r#"
(() => {
  const n = document.querySelector(%s);
  if (!n) return {ok: false, motivo: 'nessun elemento per quel selettore'};
  n.scrollIntoView({block: 'center'});
  // I menu delle applicazioni web spesso ascoltano mousedown, non click:
  // si manda la sequenza intera, come farebbe una mano.
  for (const tipo of ['pointerdown','mousedown','pointerup','mouseup','click']) {
    n.dispatchEvent(new MouseEvent(tipo, {bubbles: true, cancelable: true, view: window}));
  }
  return {ok: true, su: (n.innerText || n.getAttribute('aria-label') || n.id || '').trim().slice(0,80)};
})()
"#;

/// Scrive in un campo. Il setter nativo invece di `n.value = ...`: React e
/// compagnia intercettano la proprieta' e senza quello non si accorgono di
/// niente. E niente setter rubato su un `<select>`, dove e' un controllo
/// nativo di provenienza e lancia «Illegal invocation».
pub const SCRIVI: &str = r#"
(() => {
  const n = document.querySelector(%s);
  if (!n) return {ok: false, motivo: 'nessun elemento per quel selettore'};
  n.focus();
  const testo = %s;
  if (n.tagName === 'SELECT') {
    // Niente setter rubato a HTMLInputElement: e' un branding check nativo
    // e su un <select> lancia "Illegal invocation". Il valore lo si sceglie
    // per corrispondenza — sul value dell'opzione o sul suo testo visibile,
    // perche' di solito e' quello che arriva qui, non il value interno.
    const opt = Array.from(n.options).find(o => o.value === testo || o.text.trim() === testo);
    n.value = opt ? opt.value : testo;
  } else if ('value' in n) {
    // Il setter nativo, non `n.value = ...`: React e compagnia intercettano
    // la proprieta' e senza questo non si accorgono di niente.
    const proto = n instanceof HTMLTextAreaElement ? HTMLTextAreaElement : HTMLInputElement;
    const setter = Object.getOwnPropertyDescriptor(proto.prototype, 'value').set;
    setter.call(n, testo);
  } else {
    n.textContent = testo;
  }
  n.dispatchEvent(new Event('input', {bubbles: true}));
  n.dispatchEvent(new Event('change', {bubbles: true}));
  return {ok: true};
})()
"#;

/// Incolla un blocco intero. Le griglie — Fogli Google, Excel sul web,
/// Airtable — non hanno un campo di testo: hanno un ascoltatore di `paste`
/// che spacchetta da solo tabulazioni e a capo in celle. E non serve la
/// clipboard vera del sistema, che e' dell'utente e non nostra.
pub const INCOLLA: &str = r#"
(() => {
  const testo = %s, sel = %s;
  const n = sel ? document.querySelector(sel) : document.activeElement;
  if (!n) return {ok: false, motivo: 'nessun elemento su cui incollare'};
  if (n.focus) n.focus();
  const dt = new DataTransfer();
  dt.setData('text/plain', testo);
  const ev = new ClipboardEvent('paste', {bubbles: true, cancelable: true, clipboardData: dt});
  // dispatchEvent torna false quando qualcuno ha chiamato preventDefault:
  // qui vuol dire che la pagina l'incolla se l'e' preso in carico lei.
  const preso = !n.dispatchEvent(ev);
  return {ok: true, preso_dalla_pagina: preso,
          su: (n.tagName || '?').toLowerCase() + (n.id ? '#' + n.id : ''),
          scrivibile: ('value' in n) || !!n.isContentEditable};
})()
"#;

/// Una tabella intera come TSV, in una chiamata. Senza selettore prende
/// quella con piu' testo: nelle pagine vere e' quasi sempre quella che
/// interessa, e chiederlo costa zero turni.
pub const TABELLA: &str = r#"
(() => {
  const q = %s, massimo = %d, maxcar = %d;
  const pulisci = x => (x.innerText || x.textContent || '').replace(/\s+/g, ' ').trim().slice(0, maxcar);
  const celle = riga => {
    let c = Array.from(riga.querySelectorAll(':scope > th, :scope > td'));
    if (!c.length) c = Array.from(riga.querySelectorAll('[role="cell"], [role="gridcell"], [role="columnheader"]'));
    if (!c.length) c = Array.from(riga.children);
    return c.map(pulisci);
  };
  let radice = null;
  if (q) {
    radice = document.querySelector(q);
    if (!radice) return {ok: false, motivo: 'nessun elemento per quel selettore'};
  } else {
    // Senza selettore: la tabella con piu' testo dentro. Nelle pagine vere e'
    // quasi sempre quella che interessa, e chiederlo costa zero turni.
    const cand = Array.from(document.querySelectorAll('table, [role="table"], [role="grid"]'));
    radice = cand.sort((a, b) => (b.innerText || '').length - (a.innerText || '').length)[0] || null;
    if (!radice) return {ok: false, motivo: 'nessuna tabella in questa pagina'};
  }
  let righe = Array.from(radice.querySelectorAll('tr'));
  if (!righe.length) righe = Array.from(radice.querySelectorAll('[role="row"]'));
  if (!righe.length) righe = Array.from(radice.children);
  const dati = righe.map(celle).filter(r => r.some(c => c));
  const usate = dati.slice(0, massimo);
  return {ok: true, righe: dati.length, tagliato: dati.length > massimo,
          colonne: usate.reduce((m, r) => Math.max(m, r.length), 0),
          tsv: usate.map(r => r.join('\t')).join('\n'),
          quale: radice.tagName.toLowerCase() + (radice.id ? '#' + radice.id : '')};
})()
"#;

/// Cercare per quello che c'e' scritto. Il filtro sui piu' interni e' il
/// punto: senza, «ACCETTO» risponde anche `html` e `body`, che lo
/// contengono.
pub const PER_TESTO: &str = r#"
(() => {
  const cercato = %s.replace(/\s+/g, ' ').trim().toLowerCase();
  const dove = %s || '*', quanti = %d, esatto = %s;
  const visto = n => (n.innerText || n.value || n.getAttribute('aria-label') || '')
                       .replace(/\s+/g, ' ').trim();
  const buoni = Array.from(document.querySelectorAll(dove)).filter(n => {
    const s = visto(n).toLowerCase();
    return s && (esatto ? s === cercato : s.includes(cercato));
  });
  const foglie = buoni.filter(n => !buoni.some(m => m !== n && n.contains(m)));
  return {trovati: foglie.length, nodi: foglie.slice(0, quanti).map(n => ({
    tag: n.tagName.toLowerCase(),
    id: n.id || null,
    ruolo: n.getAttribute('role') || null,
    etichetta: n.getAttribute('aria-label') || null,
    testo: visto(n).slice(0, 120) || null,
    visibile: !!(n.offsetWidth || n.offsetHeight || n.getClientRects().length),
  }))};
})()
"#;

/// Premere per quello che c'e' scritto sopra. Esiste perche' meta' dei
/// bottoni del web non hanno un id, e la sintassi che tutti conoscono —
/// `button:has-text("...")` — e' di Playwright e in CSS non esiste.
pub const CLICCA_TESTO: &str = r#"
(() => {
  const cercato = %s.replace(/\s+/g, ' ').trim().toLowerCase();
  const dove = %s || 'button, a, [role="button"], input[type="submit"], input[type="button"], label, *';
  const visto = n => (n.innerText || n.value || n.getAttribute('aria-label') || '')
                       .replace(/\s+/g, ' ').trim();
  const vale = n => {
    const s = visto(n).toLowerCase();
    return s && s.includes(cercato)
           && !!(n.offsetWidth || n.offsetHeight || n.getClientRects().length);
  };
  const buoni = Array.from(document.querySelectorAll(dove)).filter(vale);
  const foglie = buoni.filter(n => !buoni.some(m => m !== n && n.contains(m)));
  if (!foglie.length) return {ok: false, motivo: 'nessun elemento visibile con quel testo'};
  const n = foglie[0];
  n.scrollIntoView({block: 'center'});
  for (const tipo of ['pointerdown','mousedown','pointerup','mouseup','click']) {
    n.dispatchEvent(new MouseEvent(tipo, {bubbles: true, cancelable: true, view: window}));
  }
  return {ok: true, su: visto(n).slice(0, 80), altri: foglie.length - 1};
})()
"#;

/// Il testo della pagina, con il titolo e l'indirizzo.
pub const LEGGI: &str = r#"
(() => {
  const t = (document.body && document.body.innerText || '').trim();
  return {titolo: document.title, url: location.href,
          testo: t.slice(0, %d), tagliato: t.length > %d};
})()
"#;

/// I risultati di una ricerca, letti dalla pagina del motore. Il pezzo
/// che sbroglia l'indirizzo vero da quello di rimbalzo e' li' perche'
/// altrimenti NOVA riporterebbe l'indirizzo del motore invece che
/// quello del sito, e chi legge non saprebbe dove sta andando.
pub const ESTRAI: &str = r#"
(() => {
  const vero = href => {
    try {
      const u = new URL(href);
      const p = u.searchParams.get('u');
      if (p && p.startsWith('a1')) {
        let b = p.slice(2).replace(/-/g, '+').replace(/_/g, '/');
        while (b.length %% 4) b += '=';
        return decodeURIComponent(escape(atob(b)));
      }
    } catch (e) {}
    return href;
  };
  const testo = n => n ? (n.textContent || '').replace(/\s+/g, ' ').trim() : '';
  const out = [];
  for (const n of document.querySelectorAll('li.b_algo')) {
    const a = n.querySelector('h2 a[href]');
    if (!a) continue;
    out.push({
      titolo: testo(n.querySelector('h2')).slice(0, 120),
      url: vero(a.href).slice(0, 300),
      testo: testo(n.querySelector('.b_caption p, .b_lineclamp2, p')).slice(0, %d),
    });
  }
  return {quanti: out.length, risultati: out.slice(0, %d)};
})()
"#;
