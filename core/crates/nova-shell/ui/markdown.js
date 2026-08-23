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

export function rendi(testo){
  const righe = sfuggi(testo || '').split('\n');
  const fuori = [];
  let elenco = null, inCodice = false, citazione = false;
  const chiudiElenco = () => { if(elenco){ fuori.push('</' + elenco + '>'); elenco = null; } };
  const chiudiCit = () => { if(citazione){ fuori.push('</blockquote>'); citazione = false; } };

  for(const riga of righe){
    if(/^\s*```/.test(riga)){
      if(inCodice){ fuori.push('</code></pre>'); inCodice = false; }
      else { chiudiElenco(); chiudiCit(); fuori.push('<pre><code>'); inCodice = true; }
      continue;
    }
    if(inCodice){ fuori.push(riga + '\n'); continue; }
    if(!riga.trim()){ chiudiElenco(); chiudiCit(); continue; }

    const titolo = riga.match(/^(#{1,4})\s+(.*)$/);
    if(titolo){
      chiudiElenco(); chiudiCit();
      const n = Math.min(titolo[1].length + 2, 6);
      fuori.push('<h' + n + '>' + inline(titolo[2]) + '</h' + n + '>');
      continue;
    }
    if(/^(---+|\*\*\*+|___+)$/.test(riga.trim())){
      chiudiElenco(); chiudiCit(); fuori.push('<hr>'); continue;
    }
    const cit = riga.match(/^\s*&gt;\s?(.*)$/);
    if(cit){
      chiudiElenco();
      if(!citazione){ fuori.push('<blockquote>'); citazione = true; }
      fuori.push(inline(cit[1]) + '<br>');
      continue;
    }
    const punto = riga.match(/^\s*[-*+]\s+(.*)$/);
    const numero = riga.match(/^\s*\d+[.)]\s+(.*)$/);
    if(punto || numero){
      chiudiCit();
      const tipo = punto ? 'ul' : 'ol';
      if(elenco !== tipo){ chiudiElenco(); fuori.push('<' + tipo + '>'); elenco = tipo; }
      fuori.push('<li>' + inline((punto || numero)[1]) + '</li>');
      continue;
    }
    chiudiElenco(); chiudiCit();
    fuori.push('<p>' + inline(riga) + '</p>');
  }
  if(inCodice) fuori.push('</code></pre>');
  chiudiElenco(); chiudiCit();
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
