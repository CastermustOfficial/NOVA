/* ============================================================
   I Dot nell'harness (D391).

   Le scelte sulla bozza, il 9 ottobre: una vista a sinistra, «I Dot»,
   con l'organigramma (Tu e Nova in cima, ogni Dot sotto il suo capo), il
   custode a parte, i gruppi, e i file che i Dot stanno toccando come in
   Esplora; l'organigramma si vede anche come schema, in una scheda.
   Un clic su un Dot apre la sua scheda al centro, accanto ai file: chi e',
   cosa sta facendo, e la sua chat coi compiti, i passi e i messaggi. I
   rapporti si aprono nell'editor.

   Sotto si scrive: un **messaggio**, che il Dot legge al prossimo compito
   (D388), o un **compito**, che va in coda e fa da solo. Chi scrive da qui
   firma come Nova: per i Dot l'utente e Nova sono la stessa cosa.

   Quello che serve lo dice il demone, con `dot.vista` (solo della
   persona); quello che si fa passa da `dot.scrivi`, `dot.affida`,
   `dot.ferma`, `dot.crea` e `dot.gruppo`, gli stessi strumenti che usa
   Nova. La pagina si ridisegna sugli eventi `dot.*` del demone.
   ============================================================ */
import { T } from './lingue.js';

const PREFISSO = 'nova-dot:';

const esc = s => String(s ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
const dataOra = q => (String(q || '').match(/(\d{4}-\d{2}-\d{2})[T ](\d{2}:\d{2})/) || []).slice(1);
function quando(q) {
  const [giorno, ore] = dataOra(q);
  if (!giorno) return '';
  const oggi = new Date();
  const qui = `${oggi.getFullYear()}-${String(oggi.getMonth() + 1).padStart(2, '0')}-${String(oggi.getDate()).padStart(2, '0')}`;
  return giorno === qui ? ore : `${giorno.slice(8, 10)}/${giorno.slice(5, 7)} ${ore}`;
}
const nomeDi = p => String(p).split(/[\\/]/).filter(Boolean).pop() || String(p);

/* Come sta un Dot, in parole e in colore. */
const STA = {
  lavora: ['lavora', 'lavora'],
  aspetta: ['aspetta i suoi', 'aspetta'],
  in_coda: ['ha compiti in coda', 'coda'],
  libero: ['libero', 'libero'],
  custode: ['decide i permessi', 'custode'],
};
const NOME_STATO = {
  affidato: 'in coda', in_corso: 'in corso', in_attesa: 'in attesa', fatto: 'fatto',
  fallito: 'fallito', fermato: 'fermato', interrotto: 'interrotto',
};
const CHIUSI = ['fatto', 'fallito', 'fermato', 'interrotto'];

const STILE = `
.dot-vista .albero{flex:none;overflow:visible;padding-bottom:6px}
.dot-nota{margin:2px 12px 10px;padding:8px 10px;border:1px solid rgba(232,176,74,.35);border-radius:8px;font-size:11.5px;color:var(--mezzo);line-height:1.45}
.dot-nota b{color:var(--attenzione);font-weight:600}
.dot-vuoto{padding:4px 14px 10px;font-size:11.5px;color:var(--fioco);line-height:1.45}
.dot-nodo .st,.dot-casella .st{width:8px;height:8px;border-radius:50%;flex:none;background:var(--fioco)}
.st.lavora{background:var(--pensiero);box-shadow:0 0 0 3px rgba(122,162,247,.18)}
.st.aspetta{background:var(--attenzione)}
.st.coda{background:var(--brace)}
.st.libero{background:var(--ascolto)}
.st.custode{background:var(--parola)}
.st.tu{background:var(--inchiostro)}
.st.nova{background:var(--brace)}
.dot-nodo.fisso{cursor:default;color:var(--fioco)}
.dot-nodo.fisso:hover{background:none}
.dot-nodo small{color:var(--fioco);margin-left:5px;font-size:11px}
.dot-nodo .seg{color:var(--fioco);font:10px var(--mono)}
.dot-file .ic.scritto{color:var(--brace)}
.dot-file .ic.tolto{color:var(--pericolo)}
.dot-file .chi{margin-left:auto;font-size:10px;color:var(--fioco);overflow:hidden;text-overflow:ellipsis;max-width:45%}
.dot-modulo{margin:0 12px 10px;padding:10px;border:1px solid var(--linea-2);border-radius:9px;display:flex;flex-direction:column;gap:7px;font-size:12px}
.dot-modulo input,.dot-modulo select{height:27px;border-radius:7px;border:1px solid var(--linea-2);background:var(--fondo);color:var(--inchiostro);padding:0 8px;font:12px var(--testo);outline:none}
.dot-modulo label{display:flex;align-items:center;gap:6px;color:var(--mezzo)}
.dot-modulo .fila{display:flex;gap:6px;justify-content:flex-end}
.dot-modulo button{font-size:11.5px;padding:4px 10px;border-radius:7px;border:1px solid var(--linea-2);color:var(--mezzo)}
.dot-modulo button.primo{background:var(--brace);color:#160a06;border-color:transparent;font-weight:600}

.dot-pagina{flex:1;min-width:0;min-height:0;display:flex;flex-direction:column;background:var(--fondo)}
.dot-scorre{flex:1;overflow:auto;padding:18px 22px 24px;display:flex;flex-direction:column;gap:14px;min-height:0}
.dot-chi{display:flex;gap:14px;align-items:flex-start}
.dot-faccia{width:40px;height:40px;border-radius:12px;background:var(--brace-16);display:grid;place-items:center;font-weight:700;color:var(--brace);flex:none;text-transform:uppercase}
.dot-chi h2{font-size:16px;font-weight:600}
.dot-chi p{color:var(--mezzo);font-size:12.5px;max-width:620px;margin-top:2px}
.dot-et{display:flex;gap:6px;flex-wrap:wrap;margin-top:7px}
.dot-et span{font:9.5px var(--mono);letter-spacing:.08em;padding:2px 7px;border-radius:6px;border:1px solid var(--linea);color:var(--fioco);text-transform:uppercase;cursor:default}
.dot-et span.vai{cursor:pointer}
.dot-et span.vai:hover{color:var(--inchiostro);border-color:var(--linea-2)}
.dot-et span.blu{color:var(--pensiero);border-color:rgba(122,162,247,.35)}
.dot-chi .bottoni{margin-left:auto;display:flex;gap:6px;flex:none}
.dot-btn{height:28px;padding:0 11px;border-radius:8px;border:1px solid var(--linea-2);font-size:12px;color:var(--mezzo);white-space:nowrap}
.dot-btn:hover{color:var(--inchiostro);background:var(--vetro-2)}
.dot-btn.primo{background:var(--brace);border-color:var(--brace);color:#160a06;font-weight:600}
.dot-btn.rosso{color:var(--pericolo);border-color:rgba(232,96,74,.4)}
.dot-btn:disabled{opacity:.45;cursor:default}
.dot-sotto{display:flex;gap:4px;border-bottom:1px solid var(--linea)}
.dot-sotto button{padding:6px 10px;font-size:12px;color:var(--fioco);border-bottom:2px solid transparent}
.dot-sotto button.si{color:var(--inchiostro);border-bottom-color:var(--brace)}
.dot-filo{display:flex;flex-direction:column;gap:10px}
.dot-msg{max-width:78%;padding:9px 12px;border-radius:12px;border:1px solid var(--linea);background:var(--vetro);white-space:pre-wrap;word-break:break-word;font-size:12.5px}
.dot-msg .da{font:10.5px var(--mono);color:var(--fioco);margin-bottom:3px;display:flex;gap:8px;flex-wrap:wrap;white-space:normal}
.dot-msg.nostro{align-self:flex-end;background:var(--brace-08);border-color:rgba(232,115,74,.25)}
.dot-msg.suo{align-self:flex-start}
.dot-compito{border:1px solid var(--linea-2);border-radius:12px;padding:10px 12px;background:#141210;display:flex;flex-direction:column;gap:6px}
.dot-compito .r1{display:flex;gap:8px;align-items:center;font-size:12px;color:var(--mezzo);flex-wrap:wrap}
.dot-compito .r1 b{color:var(--inchiostro);font-weight:600}
.dot-compito .r1 .tag{margin-left:auto;font:9.5px var(--mono);letter-spacing:.08em;padding:2px 7px;border-radius:6px;border:1px solid var(--linea);color:var(--fioco);text-transform:uppercase}
.dot-compito .r1 .tag.in_corso,.dot-compito .r1 .tag.in_attesa{color:var(--pensiero);border-color:rgba(122,162,247,.35)}
.dot-compito .r1 .tag.fallito,.dot-compito .r1 .tag.interrotto{color:var(--pericolo);border-color:rgba(232,96,74,.35)}
.dot-compito .testo{font-size:12.5px;white-space:pre-wrap;word-break:break-word}
.dot-compito .esito{font-size:12.5px;color:var(--mezzo);white-space:pre-wrap;word-break:break-word;border-top:1px solid var(--linea);padding-top:6px}
.dot-compito .vai{font:11.5px var(--mono);color:var(--brace);cursor:pointer;align-self:flex-start}
.dot-compito .vai:hover{text-decoration:underline}
.dot-passi{font:11px var(--mono);color:var(--fioco);border-left:2px solid var(--linea-2);padding-left:9px;display:flex;flex-direction:column;gap:2px}
.dot-passi summary{cursor:pointer;color:var(--mezzo)}
.dot-passi span{display:block}
.dot-righe{display:flex;flex-direction:column;gap:2px}
.dot-riga{display:flex;gap:10px;align-items:center;padding:6px 8px;border-radius:7px;font-size:12.5px;cursor:pointer}
.dot-riga:hover{background:var(--vetro-2)}
.dot-riga .n{font:11px var(--mono);color:var(--fioco);width:38px;flex:none}
.dot-riga .t{flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.dot-scrivi{border-top:1px solid var(--linea);padding:12px 22px;display:flex;flex-direction:column;gap:8px;background:var(--fondo-2)}
.dot-scrivi textarea{width:100%;min-height:54px;max-height:200px;resize:vertical;background:var(--fondo);border:1px solid var(--linea-2);border-radius:10px;padding:9px 11px;color:var(--inchiostro);font:13px var(--testo);outline:none}
.dot-scrivi textarea:focus{border-color:rgba(232,115,74,.55)}
.dot-scrivi .fila{display:flex;gap:8px;align-items:center;flex-wrap:wrap}
.dot-scrivi .fila span{color:var(--fioco);font-size:11.5px;flex:1 1 auto;min-width:0}

.dot-schema{flex:1;overflow:auto;padding:22px 24px 40px;min-height:0}
.dot-schema h3{font:10.5px var(--mono);letter-spacing:.14em;text-transform:uppercase;color:var(--fioco);margin:26px 0 10px}
.dot-albero{display:flex;justify-content:center;min-width:max-content}
.dot-albero ul{display:flex;justify-content:center;padding-top:20px;position:relative;margin:0}
.dot-albero>ul{padding-top:0}
.dot-albero li{list-style:none;position:relative;padding:20px 6px 0;display:flex;flex-direction:column;align-items:center}
.dot-albero li::before,.dot-albero li::after{content:'';position:absolute;top:0;right:50%;width:50%;height:20px;border-top:1px solid var(--linea-2)}
.dot-albero li::after{right:auto;left:50%;border-left:1px solid var(--linea-2)}
.dot-albero li:only-child::before,.dot-albero li:only-child::after{display:none}
.dot-albero li:only-child{padding-top:0}
.dot-albero li:first-child::before,.dot-albero li:last-child::after{border:0 none}
.dot-albero li:last-child::before{border-right:1px solid var(--linea-2);border-radius:0 6px 0 0}
.dot-albero li:first-child::after{border-radius:6px 0 0 0}
.dot-albero ul ul::before{content:'';position:absolute;top:0;left:50%;height:20px;border-left:1px solid var(--linea-2)}
.dot-albero>ul>li{padding-top:0}
.dot-albero>ul>li::before,.dot-albero>ul>li::after{display:none}
.dot-casella{min-width:150px;max-width:220px;padding:9px 11px;border-radius:11px;border:1px solid var(--linea-2);background:#141210;text-align:left;display:flex;flex-direction:column;gap:4px;cursor:pointer}
.dot-casella:hover{border-color:rgba(232,115,74,.5)}
.dot-casella.fissa{cursor:default}
.dot-casella.fissa:hover{border-color:var(--linea-2)}
.dot-casella .chi{display:flex;align-items:center;gap:7px;font-weight:600;font-size:12.5px}
.dot-casella .cosa{font-size:11px;color:var(--fioco)}
.dot-casella .ora{font-size:11px;color:var(--mezzo);display:-webkit-box;-webkit-line-clamp:3;-webkit-box-orient:vertical;overflow:hidden}
.dot-fuori{display:flex;gap:10px;flex-wrap:wrap}
.dot-gruppi{display:flex;gap:8px;flex-wrap:wrap}
.dot-gruppi button{padding:5px 10px;border-radius:8px;border:1px solid var(--linea-2);font-size:12px;color:var(--mezzo)}
.dot-gruppi button:hover{color:var(--inchiostro);background:var(--vetro-2)}
.dot-legenda{display:flex;gap:14px;flex-wrap:wrap;font-size:11px;color:var(--fioco);margin-top:22px}
.dot-legenda span{display:flex;align-items:center;gap:5px}
`;

export function avviaDot(h) {
  const { $, chiama, apriFile, messaggio } = h;
  const stile = document.createElement('style');
  stile.textContent = STILE;
  document.head.appendChild(stile);

  let vista = null;           // l'ultima risposta di dot.vista, senza nome
  let modulo = '';            // '', 'dot' o 'gruppo': il modulo aperto a sinistra
  const fileChiusi = new Set();
  let nuove = 0;              // compiti chiusi da quando non si guarda

  const dotDi = nome => (vista?.dots || []).find(d => d.nome === nome) || null;
  const lavoratori = () => (vista?.dots || []).filter(d => d.mestiere !== 'custode');

  /* ---------------------------------------------------- la vista a sinistra */
  async function carica() {
    try { vista = await chiama('dot.vista', { nome: '' }); }
    catch (e) { vista = { errore: String(e), dots: [], gruppi: [], file: [] }; }
    disegnaVista();
    return vista;
  }

  function disegnaVista() {
    const el = $('elencoDot');
    if (!el) return;
    if (!vista) { el.innerHTML = `<div class="dot-vuoto">${esc(T('Leggo i Dot…'))}</div>`; return; }
    let html = '';
    if (vista.errore) html += `<div class="dot-nota">${esc(T('Non riesco a leggere i Dot:'))} ${esc(vista.errore)}</div>`;
    else if (vista.accesi && !vista.accesi.accesi) {
      html += `<div class="dot-nota"><b>${esc(T('I Dot sono spenti'))}</b> (${esc(vista.accesi.perche)}). ${esc(T('Si accendono nelle impostazioni, alla voce «I Dot».'))}</div>`;
    }
    html += `<div class="sez"><span>${esc(T('Organigramma'))}</span>
      <button data-dot-azione="schema" title="${esc(T('Vedi come schema'))}"><svg viewBox="0 0 24 24"><rect x="9" y="3" width="6" height="5" rx="1"/><rect x="3" y="16" width="6" height="5" rx="1"/><rect x="15" y="16" width="6" height="5" rx="1"/><path d="M12 8v4M6 16v-4h12v4"/></svg></button>
      <button data-dot-azione="nuovo" title="${esc(T('Fai nascere un Dot'))}"><svg viewBox="0 0 24 24"><path d="M12 5v14M5 12h14"/></svg></button></div>`;
    if (modulo === 'dot') html += moduloDot();
    html += `<div class="albero">${alberoLista()}</div>`;
    const custodi = (vista.dots || []).filter(d => d.mestiere === 'custode');
    if (custodi.length) {
      html += `<div class="sez"><span>${esc(T('Fuori dalla piramide'))}</span></div><div class="albero">`
        + custodi.map(d => rigaDot(d, 0)).join('') + `</div>`;
    }
    html += `<div class="sez"><span>${esc(T('Gruppi'))}</span>
      <button data-dot-azione="gruppo" title="${esc(T('Fai un gruppo'))}"><svg viewBox="0 0 24 24"><path d="M12 5v14M5 12h14"/></svg></button></div>`;
    if (modulo === 'gruppo') html += moduloGruppo();
    html += `<div class="albero">${(vista.gruppi || []).length
      ? vista.gruppi.map(g => `<div class="nodo dot-nodo" data-dot-gruppo="${esc(g.nome)}" title="${esc(g.membri.join(', '))}">
          <span class="ic">#</span><span class="n">${esc(g.nome)}<small>${g.membri.length} ${esc(T('membri'))}</small></span>
          <span class="seg">${g.messaggi || ''}</span></div>`).join('')
      : `<div class="dot-vuoto">${esc(T('Nessun gruppo.'))}</div>`}</div>`;
    html += `<div class="sez"><span>${esc(T('File toccati'))}</span></div><div class="albero">${alberoFile()}</div>`;
    el.innerHTML = html;
    lega(el);
  }

  /* Tu e Nova in cima, poi i Dot senza capo, e sotto ognuno i suoi. Un capo
     che non c'e' piu' non nasconde i suoi: salgono sotto Nova. */
  function radici() {
    const ds = lavoratori();
    const nomi = new Set(ds.map(d => d.nome));
    return ds.filter(d => !d.capo || !nomi.has(d.capo));
  }
  const figli = nome => lavoratori().filter(d => d.capo === nome);

  function rigaDot(d, livello) {
    const [parola, classe] = STA[d.sta] || STA.libero;
    const seg = d.compito ? `n.${d.compito.id}` : d.in_coda ? `${d.in_coda} ${T('in coda')}` : '';
    const aperta = h.attiva()?.percorso === PREFISSO + d.nome;
    return `<div class="nodo dot-nodo${aperta ? ' aperto' : ''}" data-dot="${esc(d.nome)}" style="padding-left:${8 + livello * 14}px"
        title="${esc(T(parola))}${d.compito ? ' — ' + esc(d.compito.testo) : ''}">
      <span class="st ${classe}"></span><span class="n">${esc(d.nome)}<small>${esc(T(d.mestiere))}</small></span>
      <span class="seg">${esc(seg)}</span></div>`;
  }

  function alberoLista() {
    const giu = (d, livello, visti) => {
      if (visti.has(d.nome)) return '';
      visti.add(d.nome);
      return rigaDot(d, livello) + figli(d.nome).map(f => giu(f, livello + 1, visti)).join('');
    };
    const visti = new Set();
    let html = `<div class="nodo dot-nodo fisso" style="padding-left:8px"><span class="st tu"></span><span class="n">${esc(T('Tu'))}</span></div>
      <div class="nodo dot-nodo fisso" style="padding-left:22px"><span class="st nova"></span><span class="n">Nova</span></div>`;
    const r = radici();
    if (!r.length) return html + `<div class="dot-vuoto">${esc(T('Ancora nessun Dot. Si fanno nascere col +, o chiedendolo a Nova.'))}</div>`;
    return html + r.map(d => giu(d, 2, visti)).join('');
  }

  /* I file toccati come in Esplora: una cartella per livello, e le catene di
     cartelle con un figlio solo in una riga sola. */
  function alberoFile() {
    const file = vista.file || [];
    if (!file.length) return `<div class="dot-vuoto">${esc(T('Nessun file toccato.'))}</div>`;
    const radice = { figli: new Map(), file: [] };
    for (const f of file) {
      const pezzi = String(f.percorso).split(/[\\/]/).filter(Boolean);
      let n = radice;
      for (const p of pezzi.slice(0, -1)) {
        if (!n.figli.has(p)) n.figli.set(p, { figli: new Map(), file: [] });
        n = n.figli.get(p);
      }
      n.file.push(f);
    }
    const righe = [];
    const giu = (nodo, nome, chiave, livello) => {
      // Una catena di cartelle con un figlio solo si scrive in una riga.
      while (nodo.file.length === 0 && nodo.figli.size === 1) {
        const [n2, f2] = [...nodo.figli.entries()][0];
        nome = nome ? `${nome}/${n2}` : n2;
        chiave = `${chiave}/${n2}`;
        nodo = f2;
      }
      if (nome) {
        const chiusa = fileChiusi.has(chiave);
        righe.push(`<div class="nodo dot-nodo" data-dot-cartella="${esc(chiave)}" style="padding-left:${8 + livello * 12}px" title="${esc(nome)}">
          <span class="ic">${chiusa ? '▸' : '▾'}</span><span class="n">${esc(nome)}</span></div>`);
        if (chiusa) return;
        livello += 1;
      }
      for (const [n2, f2] of [...nodo.figli.entries()].sort((a, b) => a[0].localeCompare(b[0]))) giu(f2, n2, `${chiave}/${n2}`, livello);
      for (const f of [...nodo.file].sort((a, b) => nomeDi(a.percorso).localeCompare(nomeDi(b.percorso)))) {
        const come = f.tolto ? 'tolto' : f.scritto ? 'scritto' : 'letto';
        const segno = { tolto: '✕', scritto: '✎', letto: '○' }[come];
        const parole = [f.letto && T('letto'), f.scritto && T('scritto'), f.tolto && T('tolto')].filter(Boolean).join(', ');
        righe.push(`<div class="nodo dot-nodo dot-file" data-dot-file="${esc(f.percorso)}" data-tolto="${f.tolto && !f.scritto ? 1 : 0}"
            style="padding-left:${8 + livello * 12}px" title="${esc(f.percorso)}\n${esc(parole)} — ${esc(f.dots.join(', '))} · ${esc(quando(f.ultimo))}">
          <span class="ic ${come}">${segno}</span><span class="n">${esc(nomeDi(f.percorso))}</span>
          <span class="chi">${esc(f.dots.join(', '))}</span></div>`);
      }
    };
    giu(radice, '', '', 0);
    return righe.join('');
  }

  function moduloDot() {
    const capi = lavoratori().map(d => `<option value="${esc(d.nome)}">${esc(d.nome)}</option>`).join('');
    return `<form class="dot-modulo" data-dot-modulo="dot">
      <input name="nome" placeholder="${esc(T('nome, per esempio lettore-3'))}" required>
      <input name="ruolo" placeholder="${esc(T('chi è, cosa fa'))}" required>
      <select name="mestiere"><option value="">${esc(T('generico'))}</option><option value="ricercatore">${esc(T('ricercatore'))}</option></select>
      <select name="capo"><option value="">${esc(T('nessun capo: sta sotto Nova'))}</option>${capi}</select>
      <div class="fila"><button type="button" data-dot-azione="annulla">${esc(T('Annulla'))}</button><button class="primo">${esc(T('Fai nascere'))}</button></div>
    </form>`;
  }

  function moduloGruppo() {
    const ds = lavoratori();
    return `<form class="dot-modulo" data-dot-modulo="gruppo">
      <input name="nome" placeholder="${esc(T('nome del gruppo'))}" required>
      ${ds.length ? ds.map(d => `<label><input type="checkbox" name="membri" value="${esc(d.nome)}"> ${esc(d.nome)}</label>`).join('')
        : `<div class="dot-vuoto">${esc(T('Ancora nessun Dot da mettere in un gruppo.'))}</div>`}
      <div class="fila"><button type="button" data-dot-azione="annulla">${esc(T('Annulla'))}</button><button class="primo">${esc(T('Fai il gruppo'))}</button></div>
    </form>`;
  }

  function lega(el) {
    el.querySelectorAll('[data-dot]').forEach(n => n.onclick = () => apri('dot', n.dataset.dot));
    el.querySelectorAll('[data-dot-gruppo]').forEach(n => n.onclick = () => apri('gruppo', n.dataset.dotGruppo));
    el.querySelectorAll('[data-dot-cartella]').forEach(n => n.onclick = () => {
      const k = n.dataset.dotCartella;
      if (fileChiusi.has(k)) fileChiusi.delete(k); else fileChiusi.add(k);
      disegnaVista();
    });
    el.querySelectorAll('[data-dot-file]').forEach(n => n.onclick = () => {
      if (n.dataset.tolto === '1') { messaggio(T('Il file non c’è più: un Dot l’ha tolto di lì.'), 5000); return; }
      apriFile(n.dataset.dotFile);
    });
    el.querySelectorAll('[data-dot-azione]').forEach(b => b.onclick = () => {
      const a = b.dataset.dotAzione;
      if (a === 'schema') apri('schema', 'organigramma');
      else if (a === 'nuovo') { modulo = modulo === 'dot' ? '' : 'dot'; disegnaVista(); el.querySelector('[data-dot-modulo] input')?.focus(); }
      else if (a === 'gruppo') { modulo = modulo === 'gruppo' ? '' : 'gruppo'; disegnaVista(); el.querySelector('[data-dot-modulo] input')?.focus(); }
      else if (a === 'annulla') { modulo = ''; disegnaVista(); }
    });
    const f = el.querySelector('[data-dot-modulo]');
    if (f) f.onsubmit = async ev => {
      ev.preventDefault();
      const dati = new FormData(f);
      try {
        if (f.dataset.dotModulo === 'dot') {
          const nome = String(dati.get('nome') || '').trim();
          await chiama('dot.crea', { nome, ruolo: String(dati.get('ruolo') || '').trim(),
            mestiere: String(dati.get('mestiere') || ''), capo: String(dati.get('capo') || '') });
          modulo = '';
          await carica();
          apri('dot', nome);
        } else {
          const nome = String(dati.get('nome') || '').trim();
          await chiama('dot.gruppo', { nome, membri: dati.getAll('membri').map(String) });
          modulo = '';
          await carica();
          apri('gruppo', nome);
        }
      } catch (e) { messaggio(String(e), 9000); }
    };
  }

  /* ------------------------------------------------------- le schede ---- */
  function apri(cosa, chi) {
    const percorso = PREFISSO + (cosa === 'dot' ? chi : `${cosa}:${chi}`);
    let s = h.schede().find(x => x.percorso === percorso);
    if (!s) {
      s = { percorso, tipo: 'dot', cosa, chi, sotto: 'chat', bozza: '',
            nome: cosa === 'schema' ? T('Organigramma') : cosa === 'gruppo' ? `# ${chi}` : chi };
      h.aggiungiScheda(s);
    }
    h.mostraScheda(s);
  }

  async function mostra(s) {
    const el = $('dotPagina');
    disegnaVista();
    if (s.cosa === 'schema') {
      if (!vista) await carica();
      disegnaSchema(s);
      return;
    }
    if (!s.dati) el.innerHTML = `<div class="dot-scorre"><div class="dot-vuoto">${esc(T('Leggo…'))}</div></div>`;
    await ricaricaScheda(s);
  }

  async function ricaricaScheda(s) {
    try {
      s.dati = await chiama('dot.vista', { nome: s.cosa === 'gruppo' ? `gruppo:${s.chi}` : s.chi });
      s.errore = '';
    } catch (e) { s.errore = String(e); }
    if (h.attiva() === s) disegnaScheda(s);
  }

  function disegnaScheda(s) {
    const el = $('dotPagina');
    const scorre = el.querySelector('.dot-scorre');
    const alto = scorre && el.dataset.scheda === s.percorso ? scorre.scrollTop : null;
    const inFondo = scorre && el.dataset.scheda === s.percorso ? scorre.scrollHeight - scorre.scrollTop - scorre.clientHeight < 40 : true;
    const area = el.querySelector('textarea');
    if (area && el.dataset.scheda === s.percorso) s.bozza = area.value;
    el.dataset.scheda = s.percorso;
    if (s.errore) {
      el.innerHTML = `<div class="dot-scorre"><div class="dot-nota">${esc(s.errore)}</div></div>`;
      return;
    }
    el.innerHTML = (s.cosa === 'gruppo' ? paginaGruppo(s) : paginaDot(s));
    const nuovo = el.querySelector('.dot-scorre');
    if (nuovo) nuovo.scrollTop = inFondo || alto == null ? (s.sotto === 'chat' ? nuovo.scrollHeight : 0) : alto;
    legaScheda(el, s);
  }

  function paginaDot(s) {
    const x = s.dati;
    const d = x.dot;
    const [parola] = STA[x.sta] || STA.libero;
    const custode = d.mestiere === 'custode';
    const compiti = x.compiti || [];
    const ora = compiti.find(c => c.id === x.in_corso) || compiti.find(c => c.stato === 'in_attesa');
    const et = [
      `<span>${esc(T(d.mestiere))}</span>`,
      d.capo ? `<span class="vai" data-dot="${esc(d.capo)}">${esc(T('capo'))}: ${esc(d.capo)}</span>` : '',
      x.sottoposti.length ? `<span>${esc(T('capo di'))} ${x.sottoposti.length}</span>` : '',
      ...x.gruppi.map(g => `<span class="vai" data-dot-gruppo="${esc(g)}"># ${esc(g)}</span>`),
      `<span class="blu">${esc(T(parola))}${ora ? ' · ' + esc(T('compito')) + ' n.' + ora.id : ''}</span>`,
    ].join('');
    let corpo = '';
    if (s.sotto === 'compiti') corpo = righeCompiti(compiti);
    else if (s.sotto === 'file') corpo = righeFile(x.file || []);
    else corpo = filo(s);
    const sotto = [['chat', T('Chat')], ['compiti', `${T('Compiti')} · ${compiti.length}`], ['file', `${T('File')} · ${(x.file || []).length}`]]
      .map(([k, t]) => `<button data-sotto="${k}" class="${s.sotto === k ? 'si' : ''}">${esc(t)}</button>`).join('');
    return `<div class="dot-scorre">
      <div class="dot-chi">
        <div class="dot-faccia">${esc(d.nome.slice(0, 1))}</div>
        <div><h2>${esc(d.nome)}</h2><p>${esc(d.ruolo)}</p><div class="dot-et">${et}</div></div>
        <div class="bottoni">${x.in_corso ? `<button class="dot-btn rosso" data-ferma="1">${esc(T('Ferma il compito'))}</button>` : ''}</div>
      </div>
      <div class="dot-sotto">${sotto}</div>
      ${corpo}
    </div>${custode
      ? `<div class="dot-scrivi"><div class="fila"><span>${esc(T('Il custode non prende compiti e non legge la posta: decide i permessi degli altri Dot.'))}</span></div></div>`
      : `<div class="dot-scrivi">
        <textarea placeholder="${esc(T('Scrivi a') + ' ' + d.nome + '…')}">${esc(s.bozza || '')}</textarea>
        <div class="fila"><span>${esc(T('Un messaggio lo legge al prossimo compito. Un compito va in coda e lo fa da solo.'))}</span>
          <button class="dot-btn" data-manda="messaggio">${esc(T('Manda come messaggio'))}</button>
          <button class="dot-btn primo" data-manda="compito">${esc(T('Affida come compito'))}</button></div>
      </div>`}`;
  }

  /* La chat di un Dot: i compiti che riceve, coi loro passi, e i messaggi,
     ricevuti e mandati, in ordine di tempo. Quello che scrivono Nova e
     l'utente sta dalla stessa parte: per i Dot sono la stessa cosa. */
  function filo(s) {
    const x = s.dati;
    const voci = [];
    // I passi si vedono aperti per il compito che lavora o aspetta, e per
    // quelli che si sono aperti a mano: ridisegnare non li richiude.
    if (!s.aperti) s.aperti = new Set((x.compiti || []).filter(c => c.stato === 'in_corso' || c.stato === 'in_attesa').map(c => c.id));
    for (const c of x.compiti || []) voci.push({ q: c.affidato, html: cartaCompito(x, c, s.aperti) });
    for (const m of x.posta || []) {
      const nostro = m.da === 'nova';
      const dove = String(m.a).startsWith('gruppo:') ? ` · ${T('nel gruppo')} ${m.a.slice(7)}` : '';
      voci.push({ q: m.quando, html: `<div class="dot-msg ${nostro ? 'nostro' : 'suo'}"><div class="da">${esc(nostro ? T('Nova e tu') : m.da)}${esc(dove)} · ${esc(quando(m.quando))}
        <span>${esc(m.letto ? T('letto') : T('da leggere al prossimo compito'))}</span></div>${esc(m.testo)}</div>` });
    }
    for (const m of x.inviati || []) {
      const a = m.a === 'nova' ? T('Nova e te') : String(m.a).startsWith('gruppo:') ? `# ${m.a.slice(7)}` : m.a;
      voci.push({ q: m.quando, html: `<div class="dot-msg suo"><div class="da">${esc(x.dot.nome)} → ${esc(a)} · ${esc(quando(m.quando))}</div>${esc(m.testo)}</div>` });
    }
    voci.sort((a, b) => String(a.q).localeCompare(String(b.q)));
    if (!voci.length) return `<div class="dot-vuoto">${esc(T('Ancora niente: affidagli un compito, o scrivigli.'))}</div>`;
    return `<div class="dot-filo">${voci.map(v => v.html).join('')}</div>`;
  }

  function passoInParole(p) {
    const t = p.tipo;
    if (t === 'comincia') return T('comincia');
    if (t === 'turno') {
      const chi = p.cervello || p.gradino || p.cervello_chiesto || '';
      const per = p.per ? `${p.per} · ` : '';
      const strumenti = (p.strumenti || []).length ? p.strumenti.join(', ') : T('nessuno strumento');
      return `${per}${T('turno')}${p.giro ? ' ' + p.giro : ''} · ${chi} · ${strumenti}${p.secondi != null ? ` · ${p.secondi}s` : ''}`;
    }
    if (t === 'aspetta') return `${T('aspetta')} ${(p.chi || []).join(', ')}`;
    if (t === 'finisce') return `${T('finisce')}: ${T(NOME_STATO[p.stato] || p.stato || '')}`;
    if (t === 'piano') return `${T('piano')}: ${(p.passi || []).length} ${T('passi')}`;
    if (t === 'salita') return `${T('sale')}: ${p.da} → ${p.a}${p.motivo ? ' · ' + p.motivo : ''}`;
    if (t === 'rapporto') return `${T('rapporto')}: ${nomeDi(p.file || '')}`;
    if (t === 'vault') return p.errore ? `${T('vault')}: ${p.errore}` : `${T('vault')}: ${p.slug || ''}`;
    return t || '';
  }

  function cartaCompito(x, c, aperti) {
    const passi = (x.passi || []).filter(p => p.compito === c.id);
    const squadra = (c.attende || []).map(a =>
      `<span>→ ${esc(a.dot)} n.${a.id}: ${esc(T(NOME_STATO[a.stato] || a.stato))}</span>`).join('');
    const da = c.da === 'nova' ? T('da Nova e te') : c.da ? `${T('affidato da')} ${c.da}` : '';
    const chiuso = CHIUSI.includes(c.stato);
    const esito = chiuso && c.esito ? (c.esito.length > 600 ? c.esito.slice(0, 600) + '…' : c.esito) : '';
    return `<div class="dot-compito">
      <div class="r1"><b>${esc(T('Compito'))} n.${c.id}</b> · ${esc(da)} · ${esc(quando(c.affidato))}<span class="tag ${esc(c.stato)}">${esc(T(NOME_STATO[c.stato] || c.stato))}</span></div>
      <div class="testo">${esc(c.testo)}</div>
      ${passi.length || squadra ? `<details class="dot-passi" data-passi="${c.id}"${aperti.has(c.id) ? ' open' : ''}><summary>${passi.length} ${esc(T('passi'))}</summary>
        ${passi.map(p => `<span>${esc(quando(p.quando))} ${esc(passoInParole(p))}</span>`).join('')}${squadra}</details>` : ''}
      ${esito ? `<div class="esito">${esc(esito)}</div>` : ''}
      ${c.rapporto ? `<span class="vai" data-apri="${esc(c.rapporto)}">→ ${esc(T('il rapporto, nell’editor'))}</span>` : ''}
    </div>`;
  }

  function righeCompiti(compiti) {
    if (!compiti.length) return `<div class="dot-vuoto">${esc(T('Nessun compito.'))}</div>`;
    return `<div class="dot-righe">${[...compiti].reverse().map(c => `<div class="dot-riga" data-compito="${c.id}">
      <span class="n">n.${c.id}</span><span class="t">${esc(c.testo)}</span>
      <span class="n" style="width:auto">${esc(T(NOME_STATO[c.stato] || c.stato))}</span></div>`).join('')}</div>`;
  }

  function righeFile(file) {
    if (!file.length) return `<div class="dot-vuoto">${esc(T('Nessun file toccato.'))}</div>`;
    return `<div class="dot-righe">${file.map(f => {
      const parole = [f.letto && T('letto'), f.scritto && T('scritto'), f.tolto && T('tolto')].filter(Boolean).join(', ');
      return `<div class="dot-riga" data-apri="${esc(f.percorso)}" data-tolto="${f.tolto && !f.scritto ? 1 : 0}" title="${esc(f.percorso)}">
        <span class="t">${esc(f.percorso)}</span><span class="n" style="width:auto">${esc(parole)} · ${esc(quando(f.ultimo))}</span></div>`;
    }).join('')}</div>`;
  }

  function paginaGruppo(s) {
    const x = s.dati;
    const membri = x.gruppo.membri.map(m => `<span class="vai" data-dot="${esc(m)}">${esc(m)}</span>`).join('');
    const chat = (x.chat || []).map(m => {
      const nostro = m.da === 'nova';
      return `<div class="dot-msg ${nostro ? 'nostro' : 'suo'}"><div class="da">${esc(nostro ? T('Nova e tu') : m.da)} · ${esc(quando(m.quando))}</div>${esc(m.testo)}</div>`;
    }).join('');
    const tagliata = x.messaggi > (x.chat || []).length
      ? `<div class="dot-vuoto">${esc(T('Messaggi qui:'))} ${(x.chat || []).length} / ${x.messaggi}, ${esc(T('i più recenti.'))}</div>` : '';
    return `<div class="dot-scorre">
      <div class="dot-chi"><div class="dot-faccia">#</div>
        <div><h2>${esc(x.gruppo.nome)}</h2><p>${esc(T('Un messaggio al gruppo va nella posta di ogni membro, che lo legge al prossimo compito.'))}</p>
        <div class="dot-et">${membri}</div></div></div>
      ${tagliata}
      ${chat ? `<div class="dot-filo">${chat}</div>` : `<div class="dot-vuoto">${esc(T('Ancora nessun messaggio.'))}</div>`}
    </div>
    <div class="dot-scrivi">
      <textarea placeholder="${esc(T('Scrivi al gruppo') + ' ' + x.gruppo.nome + '…')}">${esc(s.bozza || '')}</textarea>
      <div class="fila"><span>${esc(T('Lo leggono tutti i membri, al prossimo compito.'))}</span>
        <button class="dot-btn primo" data-manda="gruppo">${esc(T('Manda al gruppo'))}</button></div>
    </div>`;
  }

  function legaScheda(el, s) {
    el.querySelectorAll('[data-dot]').forEach(n => n.onclick = () => apri('dot', n.dataset.dot));
    el.querySelectorAll('[data-dot-gruppo]').forEach(n => n.onclick = () => apri('gruppo', n.dataset.dotGruppo));
    el.querySelectorAll('[data-sotto]').forEach(b => b.onclick = () => { s.sotto = b.dataset.sotto; disegnaScheda(s); });
    el.querySelectorAll('[data-compito]').forEach(r => r.onclick = () => { s.sotto = 'chat'; disegnaScheda(s); });
    el.querySelectorAll('[data-apri]').forEach(a => a.onclick = () => {
      if (a.dataset.tolto === '1') { messaggio(T('Il file non c’è più: un Dot l’ha tolto di lì.'), 5000); return; }
      apriFile(a.dataset.apri);
    });
    el.querySelectorAll('[data-passi]').forEach(d => d.ontoggle = () => {
      const id = Number(d.dataset.passi);
      if (d.open) s.aperti?.add(id); else s.aperti?.delete(id);
    });
    const ferma = el.querySelector('[data-ferma]');
    if (ferma) ferma.onclick = async () => {
      ferma.disabled = true;
      try { await chiama('dot.ferma', { nome: s.chi }); } catch (e) { messaggio(String(e), 9000); }
      ricaricaScheda(s);
    };
    const area = el.querySelector('textarea');
    if (area) {
      area.oninput = () => { s.bozza = area.value; };
      area.onkeydown = ev => {
        if (ev.key === 'Enter' && (ev.ctrlKey || ev.metaKey)) { ev.preventDefault(); el.querySelector('[data-manda].primo')?.click(); }
      };
    }
    el.querySelectorAll('[data-manda]').forEach(b => b.onclick = async () => {
      const testo = (area?.value || '').trim();
      if (!testo) { area?.focus(); return; }
      el.querySelectorAll('[data-manda]').forEach(x => { x.disabled = true; });
      try {
        if (b.dataset.manda === 'compito') {
          const r = await chiama('dot.affida', { nome: s.chi, compito: testo });
          messaggio(`${T('Affidato a')} ${s.chi}: ${T('compito')} n.${r.compito}`, 5000);
        } else {
          await chiama('dot.scrivi', { a: s.cosa === 'gruppo' ? `gruppo:${s.chi}` : s.chi, testo });
        }
        s.bozza = '';
        if (area) area.value = '';
        await ricaricaScheda(s);
        carica();
      } catch (e) {
        messaggio(String(e), 9000);
        el.querySelectorAll('[data-manda]').forEach(x => { x.disabled = false; });
      }
    });
  }

  /* L'organigramma come schema: le stesse caselle della lista, con le linee
     da ogni capo ai suoi. */
  function disegnaSchema(s) {
    const el = $('dotPagina');
    el.dataset.scheda = s.percorso;
    const casella = d => {
      const [parola, classe] = STA[d.sta] || STA.libero;
      const ora = d.compito ? `n.${d.compito.id} · ${d.compito.testo}` : d.in_coda ? `${d.in_coda} ${T('in coda')}` : T(parola);
      return `<div class="dot-casella" data-dot="${esc(d.nome)}" title="${esc(d.ruolo)}">
        <div class="chi"><span class="st ${classe}"></span>${esc(d.nome)}</div>
        <div class="cosa">${esc(T(d.mestiere))}${d.posta_da_leggere ? ` · ${d.posta_da_leggere} ${esc(T('messaggi da leggere'))}` : ''}</div>
        <div class="ora">${esc(ora)}</div></div>`;
    };
    const ramo = (d, visti) => {
      if (visti.has(d.nome)) return '';
      visti.add(d.nome);
      const sotto = figli(d.nome).map(f => ramo(f, visti)).filter(Boolean);
      return `<li>${casella(d)}${sotto.length ? `<ul>${sotto.join('')}</ul>` : ''}</li>`;
    };
    const visti = new Set();
    const rami = radici().map(d => ramo(d, visti)).filter(Boolean);
    const fissa = (classe, nome, cosa) => `<div class="dot-casella fissa"><div class="chi"><span class="st ${classe}"></span>${esc(nome)}</div><div class="cosa">${esc(cosa)}</div></div>`;
    const custodi = (vista?.dots || []).filter(d => d.mestiere === 'custode');
    const gruppi = vista?.gruppi || [];
    el.innerHTML = `<div class="dot-schema">
      <div class="dot-albero"><ul><li>${fissa('tu', T('Tu'), T('sopra a tutti'))}<ul><li>${fissa('nova', 'Nova', T('il tramite'))}${rami.length ? `<ul>${rami.join('')}</ul>` : ''}</li></ul></li></ul></div>
      ${rami.length ? '' : `<div class="dot-vuoto" style="text-align:center;margin-top:14px">${esc(T('Ancora nessun Dot. Si fanno nascere col +, o chiedendolo a Nova.'))}</div>`}
      ${custodi.length ? `<h3>${esc(T('Fuori dalla piramide'))}</h3><div class="dot-fuori">${custodi.map(casella).join('')}</div>` : ''}
      ${gruppi.length ? `<h3>${esc(T('Gruppi'))}</h3><div class="dot-gruppi">${gruppi.map(g => `<button data-dot-gruppo="${esc(g.nome)}" title="${esc(g.membri.join(', '))}"># ${esc(g.nome)} · ${g.membri.length}</button>`).join('')}</div>` : ''}
      <div class="dot-legenda">${Object.entries(STA).map(([, [parola, classe]]) => `<span><i class="st ${classe}" style="display:inline-block;width:8px;height:8px;border-radius:50%"></i>${esc(T(parola))}</span>`).join('')}</div>
    </div>`;
    el.querySelectorAll('[data-dot]').forEach(n => n.onclick = () => apri('dot', n.dataset.dot));
    el.querySelectorAll('[data-dot-gruppo]').forEach(n => n.onclick = () => apri('gruppo', n.dataset.dotGruppo));
  }

  /* ---------------------------------------------- gli eventi del demone - */
  let _presto = 0;
  function aggiornaPresto() {
    clearTimeout(_presto);
    _presto = setTimeout(async () => {
      const guardo = h.vistaAttiva() === 'dot';
      const s = h.attiva();
      const aperta = s?.tipo === 'dot';
      if (!guardo && !aperta) return;
      await carica();
      if (aperta && s.cosa === 'schema') disegnaSchema(s);
      else if (aperta) ricaricaScheda(s);
    }, 400);
  }

  function evento(topic, dati) {
    if (!String(topic).startsWith('dot.')) return;
    if (topic === 'dot.compito' && CHIUSI.includes(String(dati?.stato)) && h.vistaAttiva() !== 'dot') {
      nuove += 1;
      h.pallino(nuove);
    }
    aggiornaPresto();
  }

  function mostrata() {
    nuove = 0;
    h.pallino(0);
    carica();
  }

  return { mostra, evento, mostrata, apri, PREFISSO };
}
