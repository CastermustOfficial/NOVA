/* ============================================================
   L'albero dal vivo (D418).

   Deciso con Gio il 13 ottobre, sulla bozza: nell'harness si vede la
   cartella del progetto mentre i Dot ci lavorano. Esplora si rilegge da sola
   ogni 2,5 secondi (lo stesso ritmo delle schede aperte); quello che e' nato
   o cambiato negli ultimi dieci minuti ha un segno, verde se e' nuovo e
   arancio se e' cambiato; una cartella chiusa con dentro un segno lo porta
   anche lei; accanto alla cartella di un compito in corso c'e' il Dot che ci
   lavora. «Segui» apre da solo l'ultimo file scritto, ed e' spento di serie.

   Qui solo i conti, senza la pagina: cosi' si provano da soli
   (`prove/progetto/test_harness_vivo.py`). Le date sono in secondi dal 1970,
   come le scrive il guscio nell'elenco di una cartella.
   ============================================================ */

/* Per quanto un file resta segnato. */
export const FINESTRA_S = 10 * 60;

/* Ogni quanto l'albero si rilegge. */
export const OGNI_MS = 2500;

const chiave = p => String(p || '').replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase();

/* La memoria dell'albero: da quando c'e' ogni percorso visto nascere, e
   quello che c'era in ogni cartella l'ultima volta che la si e' letta. */
export function nuovaMemoria() {
  return { nati: new Map(), dentro: new Map() };
}

/* Una cartella appena riletta: quello che non c'era la volta prima e' nato
   adesso. La prima lettura di una cartella non fa nascere niente: quello che
   c'era gia' non e' nuovo. Torna se qualcosa e' cambiato. */
export function rileggi(memoria, cartella, voci, adesso) {
  const k = chiave(cartella);
  const prima = memoria.dentro.get(k);
  const ora = new Map(voci.map(v => [chiave(v.percorso), v.modificato ?? null]));
  memoria.dentro.set(k, ora);
  if (!prima) return true;
  let cambiato = prima.size !== ora.size;
  for (const [p, q] of ora) {
    if (!prima.has(p)) { memoria.nati.set(p, adesso); cambiato = true; }
    else if (prima.get(p) !== q) cambiato = true;
  }
  return cambiato;
}

/* Il segno di una voce: «nuovo», «cambiato», «dentro» o niente. Una
   cartella non e' mai «cambiata»: la sua data cambia quando ci nasce o ci
   sparisce qualcosa, e allora e' qualcosa dentro che e' cambiato. Lo stesso
   se dentro, in quello che si e' letto, c'e' una voce segnata
   (`conDentro`, da `cartelleConDentro`). */
export function segno(memoria, voce, adesso, conDentro = null) {
  const nato = memoria.nati.get(chiave(voce.percorso));
  if (nato != null && adesso - nato < FINESTRA_S) return 'nuovo';
  const fresco = voce.modificato != null && adesso - voce.modificato < FINESTRA_S;
  if (!voce.cartella) return fresco ? 'cambiato' : '';
  return fresco || conDentro?.has(chiave(voce.percorso)) ? 'dentro' : '';
}

/* Le cartelle che hanno dentro, a qualunque profondita' letta, una voce
   segnata: portano il segno anche da chiuse. */
export function cartelleConDentro(memoria, adesso) {
  const fuori = new Set();
  for (const [cartella, voci] of memoria.dentro) {
    for (const [p, q] of voci) {
      const nato = memoria.nati.get(p);
      const fresco = (nato != null && adesso - nato < FINESTRA_S) || (q != null && adesso - q < FINESTRA_S);
      if (!fresco) continue;
      // Ogni antenato letto, fino alla radice.
      let c = cartella;
      while (c) {
        fuori.add(c);
        const i = c.lastIndexOf('/');
        if (i <= 0) break;
        c = c.slice(0, i);
      }
    }
  }
  return fuori;
}

/* Chi lavora in quale cartella: dai Dot della vista, quelli con un compito
   in corso che ha una cartella. */
export function chiLavora(dots) {
  const fuori = new Map();
  for (const d of dots || []) {
    const c = d?.compito?.cartella;
    if (!c || d.sta !== 'lavora') continue;
    const k = chiave(c);
    if (!fuori.has(k)) fuori.set(k, []);
    fuori.get(k).push(d.nome);
  }
  return fuori;
}

/* Per «segui»: il file cambiato piu' di recente fra quelli letti, se e' piu'
   recente di `dopo`. */
export function ultimoScritto(letture, dopo) {
  let meglio = null;
  for (const voci of letture) {
    for (const v of voci) {
      if (v.cartella || v.modificato == null || v.modificato <= dopo) continue;
      if (!meglio || v.modificato > meglio.modificato) meglio = v;
    }
  }
  return meglio;
}
