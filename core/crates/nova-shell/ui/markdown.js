/**
 * Markdown, quel tanto che basta.
 *
 * Scritto in casa e non preso da una libreria per un motivo solo: NOVA deve
 * funzionare senza rete, e uno script preso da un CDN in una pagina offline
 * e' una pagina rotta. Copre cio' che un assistente produce davvero —
 * grassetto, codice, elenchi, titoli, citazioni — e ignora il resto.
 *
 * L'ordine conta: si sfugge PRIMA l'HTML, poi si trasforma. Al contrario, un
 * testo che contiene del markup diventerebbe markup.
 */
'use strict';

const sfuggi = s => String(s).replace(/[&<>"']/g, c =>
  ({ '&':'&amp;', '<':'&lt;', '>':'&gt;', '"':'&quot;', "'":'&#39;' }[c]));

const SEGNO = '';

/** Grassetto, corsivo, codice, barrato, link. Su testo gia' sfuggito. */
function inline(t){
  // Il codice per primo, e il suo contenuto va da parte: dentro il codice
  // gli asterischi sono asterischi, non grassetto.
  const riserva = [];
  t = t.replace(/`([^`]+)`/g, function(_, c){
    riserva.push('<code>' + c + '</code>');
    return SEGNO + (riserva.length - 1) + SEGNO;
  });
  t = t.replace(/\*\*([^*]+)\*\*/g, '<b>$1</b>');
  t = t.replace(/(^|[^*])\*([^*\n]+)\*(?!\*)/g, '$1<i>$2</i>');
  t = t.replace(/~~([^~]+)~~/g, '<s>$1</s>');
  t = t.replace(/\[([^\]]+)\]\(([^)\s]+)\)/g,
                '<a href="$2" target="_blank" rel="noreferrer">$1</a>');
  return t.replace(new RegExp(SEGNO + '(\\d+)' + SEGNO, 'g'),
                   function(_, i){ return riserva[+i]; });
}

/* Una riga di tabella: `| a | b |` o `a | b`. Le barre dentro il codice
   contano lo stesso, come in quasi tutti i lettori: e' raro, e il costo di
   distinguerle e' un secondo analizzatore. */
function celle(riga){
  let r = riga.trim();
  if(r.startsWith('|')) r = r.slice(1);
  if(r.endsWith('|')) r = r.slice(0, -1);
  return r.split('|').map(c => c.trim());
}
const SEPARATORE = /^\s*\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)*\|?\s*$/;

/**
 * Da markdown a HTML.
 *
 * `documento`: il testo e' un file da leggere, non una risposta in chat.
 * Cambia una cosa sola — le righe di fila fanno un paragrafo, come in ogni
 * lettore di markdown. In chat no: li' un modello va a capo perche' vuole
 * andare a capo, e unire le sue righe incollerebbe frasi che aveva separato.
 */
export function rendi(testo, { documento = false } = {}){
  const righe = sfuggi(testo || '').split('\n');
  const fuori = [];
  let elenco = null, inCodice = false, citazione = false, paragrafo = [];
  const chiudiPar = () => {
    if(paragrafo.length){ fuori.push('<p>' + paragrafo.map(inline).join(' ') + '</p>'); paragrafo = []; }
  };
  const chiudiElenco = () => { if(elenco){ fuori.push('</' + elenco + '>'); elenco = null; } };
  const chiudiCit = () => { if(citazione){ fuori.push('</blockquote>'); citazione = false; } };
  const chiudiTutto = () => { chiudiPar(); chiudiElenco(); chiudiCit(); };

  for(let i = 0; i < righe.length; i++){
    const riga = righe[i];
    if(/^\s*```/.test(riga)){
      if(inCodice){ fuori.push('</code></pre>'); inCodice = false; }
      else { chiudiTutto(); fuori.push('<pre><code>'); inCodice = true; }
      continue;
    }
    if(inCodice){ fuori.push(riga + '\n'); continue; }
    if(!riga.trim()){ chiudiTutto(); continue; }

    if(riga.includes('|') && i + 1 < righe.length && righe[i + 1].includes('|')
       && SEPARATORE.test(righe[i + 1]) && celle(righe[i + 1]).length === celle(riga).length){
      chiudiTutto();
      const testa = celle(riga);
      fuori.push('<table><thead><tr>' + testa.map(c => '<th>' + inline(c) + '</th>').join('')
                 + '</tr></thead><tbody>');
      i += 2;
      for(; i < righe.length && righe[i].includes('|') && righe[i].trim(); i++){
        fuori.push('<tr>' + celle(righe[i]).map(c => '<td>' + inline(c) + '</td>').join('') + '</tr>');
      }
      i--;
      fuori.push('</tbody></table>');
      continue;
    }

    const titolo = riga.match(/^(#{1,4})\s+(.*)$/);
    if(titolo){
      chiudiTutto();
      const n = documento ? titolo[1].length : Math.min(titolo[1].length + 2, 6);
      fuori.push('<h' + n + '>' + inline(titolo[2]) + '</h' + n + '>');
      continue;
    }
    if(/^(---+|\*\*\*+|___+)$/.test(riga.trim())){
      chiudiTutto(); fuori.push('<hr>'); continue;
    }
    const cit = riga.match(/^\s*&gt;\s?(.*)$/);
    if(cit){
      chiudiPar(); chiudiElenco();
      if(!citazione){ fuori.push('<blockquote>'); citazione = true; }
      fuori.push(inline(cit[1]) + '<br>');
      continue;
    }
    const punto = riga.match(/^\s*[-*+]\s+(.*)$/);
    const numero = riga.match(/^\s*\d+[.)]\s+(.*)$/);
    if(punto || numero){
      chiudiPar(); chiudiCit();
      const tipo = punto ? 'ul' : 'ol';
      if(elenco !== tipo){ chiudiElenco(); fuori.push('<' + tipo + '>'); elenco = tipo; }
      fuori.push('<li>' + inline((punto || numero)[1]) + '</li>');
      continue;
    }
    chiudiElenco(); chiudiCit();
    if(documento){ paragrafo.push(riga.trim()); continue; }
    fuori.push('<p>' + inline(riga) + '</p>');
  }
  if(inCodice) fuori.push('</code></pre>');
  chiudiTutto();
  return fuori.join('');
}

/**
 * Toglie il rumore che il processo di NOVA stampa insieme alla risposta.
 *
 * Ora i log vanno su stderr e non dovrebbero piu' arrivare qui: questa resta
 * come rete, perche' una riga di diagnostica dentro la bolla del messaggio e'
 * la cosa che fa sembrare rotto un sistema che funziona.
 */
export function pulisci(testo){
  return String(testo || '')
    .split('\n')
    .filter(r => !/^\s*(\[nova\]|\[kb\]|\[conferma richiesta\]|->|<-|~>)/.test(r.trim()))
    .join('\n')
    .replace(/^\s*NOVA:\s*/, '')
    .trim();
}
