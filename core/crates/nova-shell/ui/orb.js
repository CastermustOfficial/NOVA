/**
 * L'ORB di NOVA: la faccia della voce.
 *
 * Dal 7 ottobre 2026 e' quello disegnato da Gio (`nova-orb.html`, D380): una
 * nova vista da vicino, un nucleo bianco-oro che sfuma in arancio, magenta,
 * viola e blu, con le punte di diffrazione e un'onda d'urto che si allarga.
 * Lo shader e' il suo, riga per riga, in WebGL2: e' il pezzo che vale. Qui
 * cambia il guscio, cioe' le poche righe che lo attaccano a un canvas, e si
 * aggiungono tre cose che a lui non servivano e all'orb di NOVA si':
 *
 * - **gli stati che il suo non ha**. I suoi quattro (`idle`, `listening`,
 *   `thinking`, `speaking`) sono `quiete`, `ascolto`, `penso`, `parlo`.
 *   `agisco`, `chiedo`, `allarme`, `spento` e `occhio` hanno valori scelti
 *   qui, con le stesse manopole; `allarme`, `spento` e `occhio` vogliono
 *   anche tre uniformi in piu' (il rosso, il grigio, la pupilla), perche'
 *   con il solo caldo/freddo non si dicono;
 * - **la scala**. L'orb vive in canvas da 26 a 96 pixel: nei piu' piccoli le
 *   punte non si vedrebbero e la sfera sarebbe un puntino, quindi si
 *   avvicina (`zoom`);
 * - **chi non vuole movimento** (`prefers-reduced-motion`) lo vede lento.
 *
 * L'orb respira con la conversazione: e' l'unica cosa di NOVA sempre in
 * scena, quindi e' anche l'unico posto dove leggere a colpo d'occhio cosa
 * sta succedendo.
 */
'use strict';

/**
 * Le manopole di Gio: energy = nucleo e bagliore; waveAmt = onda d'urto;
 * spike = punte; scale = dimensione; hue = -1 freddo .. +1 caldo; reach =
 * quanto si allarga il bagliore; speed = vortice; waveRate = onde al
 * secondo; spin = rotazione dell'aura (rad/s).
 * Quelle aggiunte: rosso, grigio e occhio da 0 a 1. `colore` e' quello della
 * legenda nel pannello: una tinta che si riconosce, non un valore dello
 * shader.
 */
export const STATI = {
  // I quattro di Gio, com'erano.
  quiete:  { energy: 0.85, waveAmt: 1.0, spike: 1.0, scale: 1.00, hue:  0.0, reach: 1.00, speed: 1.0,  waveRate: 0.20, spin: 0.12, rosso: 0, grigio: 0,   occhio: 0, colore: '#c58cff' },
  ascolto: { energy: 0.70, waveAmt: 0.0, spike: 0.4, scale: 1.08, hue: -1.0, reach: 1.00, speed: 0.55, waveRate: 0.00, spin: 0.06, rosso: 0, grigio: 0,   occhio: 0, colore: '#7fe0ff' },
  penso:   { energy: 1.00, waveAmt: 0.0, spike: 0.9, scale: 0.92, hue: -0.3, reach: 0.32, speed: 2.4,  waveRate: 0.00, spin: 2.40, rosso: 0, grigio: 0,   occhio: 0, colore: '#6f8cff' },
  parlo:   { energy: 1.25, waveAmt: 1.4, spike: 1.6, scale: 1.00, hue:  0.8, reach: 1.10, speed: 1.3,  waveRate: 0.30, spin: 0.35, rosso: 0, grigio: 0,   occhio: 0, colore: '#ffb86b' },
  // Lavora con le mani: fra il pensare e il parlare, piu' svelta, onde
  // corte e frequenti.
  agisco:  { energy: 1.10, waveAmt: 0.6, spike: 1.2, scale: 0.96, hue:  0.3, reach: 0.60, speed: 1.9,  waveRate: 0.45, spin: 1.20, rosso: 0, grigio: 0,   occhio: 0, colore: '#ff8ad8' },
  // Aspetta un ok: calda e ferma, un'onda lenta e regolare, come chi
  // aspetta senza fretta.
  chiedo:  { energy: 0.90, waveAmt: 0.8, spike: 0.7, scale: 1.00, hue:  1.0, reach: 0.80, speed: 0.6,  waveRate: 0.50, spin: 0.05, rosso: 0, grigio: 0,   occhio: 0, colore: '#ffd36b' },
  // Qualcosa non va: rossa, svelta, con le onde fitte.
  allarme: { energy: 1.20, waveAmt: 1.2, spike: 1.3, scale: 1.00, hue:  0.4, reach: 1.00, speed: 2.6,  waveRate: 0.90, spin: 0.50, rosso: 1, grigio: 0,   occhio: 0, colore: '#ff4d4d' },
  // Il demone dorme: l'orb resta li', quasi in bianco e nero, e quasi
  // ferma. La voce dorme, il compagno no.
  spento:  { energy: 0.35, waveAmt: 0.0, spike: 0.2, scale: 0.95, hue:  0.0, reach: 0.50, speed: 0.3,  waveRate: 0.00, spin: 0.02, rosso: 0, grigio: 0.9, occhio: 0, colore: '#9a9aa6' },
  // Guarda: la nova si stringe e una pupilla scura segue il mouse.
  occhio:  { energy: 0.90, waveAmt: 0.0, spike: 0.5, scale: 0.62, hue:  0.8, reach: 0.60, speed: 1.0,  waveRate: 0.00, spin: 0.10, rosso: 0, grigio: 0,   occhio: 1, colore: '#ff9a3c' },
};

/** Le manopole che si inseguono piano da uno stato all'altro. */
const MANOPOLE = ['energy', 'waveAmt', 'spike', 'scale', 'hue', 'reach', 'speed', 'waveRate', 'spin', 'rosso', 'grigio', 'occhio'];

const VERT = `#version 300 es
precision highp float;
layout(location=0) in vec2 a_pos;
void main(){ gl_Position = vec4(a_pos, 0.0, 1.0); }`;

const FRAG = `#version 300 es
precision highp float;

out vec4 fragColor;

uniform vec3  iResolution;   // (width, height, dpr)
uniform float iTime;         // seconds
uniform int   iFrame;        // frame counter
uniform vec4  iMouse;        // (x, y, L, R)
uniform float uAnim;         // accumulated animation time (speed-aware)
uniform float uWavePhase;    // accumulated shockwave phase
uniform vec4  uA;            // (energy, waveAmt, spikeAmt, scale)
uniform vec4  uB;            // (hue -1 cold..+1 warm, level 0..1, flare reach, spin angle)
uniform vec4  uC;            // NOVA: (rosso, grigio, occhio, zoom)

// ---------- hash / noise ----------
float hash31(vec3 p){
  p = fract(p * 0.3183099 + 0.1);
  p *= 17.0;
  return fract(p.x * p.y * p.z * (p.x + p.y + p.z));
}
float noise3(vec3 p){
  vec3 i = floor(p);
  vec3 f = fract(p);
  f = f * f * (3.0 - 2.0 * f);
  float n000 = hash31(i), n100 = hash31(i + vec3(1,0,0));
  float n010 = hash31(i + vec3(0,1,0)), n110 = hash31(i + vec3(1,1,0));
  float n001 = hash31(i + vec3(0,0,1)), n101 = hash31(i + vec3(1,0,1));
  float n011 = hash31(i + vec3(0,1,1)), n111 = hash31(i + vec3(1,1,1));
  return mix(mix(mix(n000, n100, f.x), mix(n010, n110, f.x), f.y),
             mix(mix(n001, n101, f.x), mix(n011, n111, f.x), f.y), f.z);
}
float fbm(vec3 p){
  float v = 0.0, a = 0.5;
  for (int i = 0; i < 4; i++){
    v += a * noise3(p);
    p = p * 2.07 + vec3(3.1, 7.3, 1.9);
    a *= 0.5;
  }
  return v;
}
mat2 rot(float a){ float c = cos(a), s = sin(a); return mat2(c, -s, s, c); }

// swirl the volume: rotation around Y that depends on radius and height
vec3 swirl(vec3 p, float t){
  float r = length(p);
  float a = t * 0.35 + r * 2.2 + p.y * 1.5;
  p.xz = rot(a) * p.xz;
  p.xy = rot(0.4 * sin(t * 0.3)) * p.xy;
  return p;
}

// ray / sphere intersection, returns (tNear, tFar) or (-1,-1)
vec2 sphere(vec3 ro, vec3 rd, float R){
  float b = dot(ro, rd);
  float c = dot(ro, ro) - R * R;
  float h = b * b - c;
  if (h < 0.0) return vec2(-1.0);
  h = sqrt(h);
  return vec2(-b - h, -b + h);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord){
  vec2 res = iResolution.xy;
  // NOVA: nei canvas piccoli la nova si avvicina (uC.w), se no e' un puntino.
  vec2 uv  = (fragCoord * 2.0 - res) / res.y / uC.w;

  vec2 m = vec2(0.0);
  if (iMouse.x > 0.0 || iMouse.y > 0.0) m = (iMouse.xy * 2.0 - res) / res.y;

  float t = uAnim;
  float energy = uA.x, waveAmt = uA.y, spikeAmt = uA.z, scale = uA.w;
  float hue = uB.x, level = uB.y, reach = max(uB.z, 0.05), spin = uB.w;

  // camera
  vec3 ro = vec3(0.0, 0.0, -3.2);
  vec3 rd = normalize(vec3(uv, 1.6));
  // tilt with mouse
  ro.yz = rot(-m.y * 0.25) * ro.yz;  rd.yz = rot(-m.y * 0.25) * rd.yz;
  ro.xz = rot( m.x * 0.35) * ro.xz;  rd.xz = rot( m.x * 0.35) * rd.xz;

  const float R = 1.0;
  vec2 hit = sphere(ro, rd, R * scale);

  vec3  acc   = vec3(0.0);   // premultiplied colour
  float alpha = 0.0;

  // palette (nova): white-gold core -> orange -> magenta -> violet -> blue
  vec3 cWhite = vec3(1.00, 0.98, 0.92);
  vec3 cGold  = vec3(1.00, 0.80, 0.40);
  vec3 cOrng  = vec3(1.00, 0.45, 0.20);
  vec3 cMag   = vec3(0.95, 0.25, 0.70);
  vec3 cViol  = vec3(0.55, 0.30, 1.00);
  vec3 cBlue  = vec3(0.20, 0.45, 1.00);
  vec3 cCyan  = vec3(0.55, 0.90, 1.00);

  // expanding shockwave: period ~5 s
  float cyc  = uWavePhase;
  float ph   = fract(cyc);
  float waveR = 0.15 + ph * 0.95;
  float waveA = (1.0 - ph) * (1.0 - ph) * waveAmt;

  if (hit.y > 0.0){
    float t0 = max(hit.x, 0.0), t1 = hit.y;
    const int   STEPS = 56;
    float dt = (t1 - t0) / float(STEPS) / scale;
    // per-pixel jitter hides banding
    float jit = hash31(vec3(fragCoord, float(iFrame % 64)));
    float tt  = t0 + dt * jit;

    for (int i = 0; i < STEPS; i++){
      if (alpha > 0.985) break;
      vec3 p = (ro + rd * tt) / scale;
      float r = length(p);

      vec3 q = swirl(p, t);
      vec3 dir = p / max(r, 1e-4);

      // radial streaks: noise over direction only, stretched along the ray from the core
      vec3 sdir = dir; sdir.xy = rot(spin) * sdir.xy;
      float st = fbm(sdir * 4.5 + vec3(t * 0.08, -t * 0.05, 0.0));
      float streak = pow(smoothstep(0.45, 0.80, st), 1.6) * smoothstep(1.0, 0.25, r) * smoothstep(0.05, 0.35, r);

      // soft wisps
      float wisp = fbm(q * 2.2 + vec3(0.0, -t * 0.15, t * 0.1));
      wisp = smoothstep(0.45, 0.80, wisp) * smoothstep(1.0, 0.75, r);

      // sharp grains ("dust")
      float g1 = noise3(q * 16.0 + vec3(t * 0.6, 0.0, -t * 0.4));
      float g2 = noise3(q * 29.0 - vec3(0.0, t * 0.9, t * 0.3));
      float grain = (smoothstep(0.78, 0.98, g1) * 1.4 + smoothstep(0.84, 1.0, g2) * 2.2) * smoothstep(1.0, 0.85, r);

      // shockwave shell, modulated by noise so it tears apart while expanding
      float shellN = fbm(dir * 3.0 + vec3(0.0, 0.0, cyc));
      float wave = exp(-pow((r - waveR) / 0.045, 2.0)) * waveA * (0.4 + 0.9 * shellN) * 3.0;

      // hot core volume
      float core = exp(-r * r * 18.0) * 6.0 * energy;

      float dens = streak * (1.6 + 1.5 * level) + wisp * 0.5 + grain * (1.0 + level) + wave + core;

      // colour by radius: temperature falls with distance from the core
      float tmp = smoothstep(1.0, 0.0, r);
      vec3 col = mix(cBlue, cViol, smoothstep(0.0, 0.45, tmp));
      col = mix(col, cMag,  smoothstep(0.35, 0.65, tmp));
      col = mix(col, cOrng, smoothstep(0.60, 0.82, tmp));
      col = mix(col, cGold, smoothstep(0.80, 0.93, tmp));
      col = mix(col, cWhite, smoothstep(0.92, 1.0, tmp));
      col = mix(col, cCyan,  clamp(grain * 0.35, 0.0, 1.0) * (1.0 - tmp));
      col = mix(col, cWhite, smoothstep(0.84, 1.0, g2) * 0.7);
      col = mix(col, mix(cGold, cWhite, 0.5), clamp(wave * 0.3, 0.0, 1.0));
      // state hue: cold (listening) / warm (speaking)
      col = mix(col, cCyan, max(-hue, 0.0) * 0.55);
      col = mix(col, cGold, max( hue, 0.0) * 0.45);

      float a = clamp(dens * dt * 2.4, 0.0, 1.0);
      acc   += (1.0 - alpha) * a * col * 1.5;
      alpha += (1.0 - alpha) * a;
      tt += dt;
    }
  }

  // screen-space flare: bright pulsing core + halo + diffraction spikes
  float dcen  = length(uv);
  float ang   = atan(uv.y, uv.x);
  float pulse = (0.85 + 0.15 * sin(t * 2.3) + 0.25 * waveA) * energy * (1.0 + 0.8 * level);
  float flare = 0.012 / (dcen * dcen / reach + 0.003) * pulse * mix(0.75, 1.0, reach);
  float halo  = exp(-max(dcen - 0.55 * reach, 0.0) * 5.0 / reach) * 0.16 * mix(0.6, 1.0, reach) * (0.8 + 0.45 * sin(3.0 * ang - spin * 1.5) * sin(2.0 * ang + spin));
  vec2  suv   = rot(spin + 0.15 * sin(t * 0.2)) * uv;
  float spikes = spikeAmt * (exp(-abs(suv.y) * 90.0) + exp(-abs(suv.x) * 90.0)) * exp(-dcen * 2.2 / reach) * 0.9 * pulse;
  spikes += spikeAmt * (exp(-abs(suv.y - suv.x) * 120.0) + exp(-abs(suv.y + suv.x) * 120.0)) * exp(-dcen * 3.5 / reach) * 0.35 * pulse;

  vec3 flareCol = mix(mix(cGold, cCyan, max(-hue, 0.0)), cWhite, clamp(flare, 0.0, 1.0)) * flare
                + mix(cOrng, cWhite, 0.6) * spikes
                + mix(cViol, cBlue, 0.5 + 0.5 * sin(t * 0.5)) * halo;
  // everything screen-space must die before the canvas edge, or the rectangle shows
  float edgeMask = smoothstep(0.98, 0.55, dcen * uC.w);
  flare  *= edgeMask;
  spikes *= edgeMask;
  halo   *= edgeMask;
  flareCol *= edgeMask;
  float flareA = clamp(flare + spikes + halo * 0.6, 0.0, 1.0);

  acc   += (1.0 - alpha) * flareCol;
  alpha += (1.0 - alpha) * flareA;

  acc = tanh(acc * 1.15);

  // NOVA: i tre stati che il caldo e il freddo non dicono.
  // L'allarme vira tutto al rosso, tenendo la luce che c'era.
  float luce = dot(acc, vec3(0.299, 0.587, 0.114));
  acc = mix(acc, vec3(1.0, 0.16, 0.12) * luce * 1.6, uC.x * 0.8);
  // Spento: quasi in bianco e nero.
  luce = dot(acc, vec3(0.299, 0.587, 0.114));
  acc = mix(acc, vec3(luce), uC.y);
  // L'occhio: una pupilla verticale scura che guarda verso il mouse.
  vec2 pup = (uv - m * 0.10) * vec2(7.5, 1.9) / max(scale, 0.2);
  float slit = clamp(1.0 - length(pup), 0.0, 1.0);
  acc *= 1.0 - smoothstep(0.0, 0.35, slit) * uC.z * 0.92;

  // kill sub-visible residue so the background stays truly transparent
  alpha = clamp((alpha - 0.004) / 0.996, 0.0, 1.0);
  acc *= step(0.0001, alpha);
  fragColor = vec4(acc, alpha);   // premultiplied
}

void main(){
  mainImage(fragColor, gl_FragCoord.xy);
}`;

function compila(gl, tipo, sorgente) {
  const s = gl.createShader(tipo);
  gl.shaderSource(s, sorgente);
  gl.compileShader(s);
  if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) {
    console.warn('[orb] shader:', gl.getShaderInfoLog(s));
    return null;
  }
  return s;
}

/** Senza WebGL2: una nova ferma, coi colori di quella vera, non un buco. */
function ripiegoStatico(canvas) {
  const ctx = canvas.getContext('2d');
  if (!ctx) return;
  const lato = Math.max(1, Math.round(canvas.clientWidth || 48));
  canvas.width = lato;
  canvas.height = lato;
  const c = lato / 2;
  const g = ctx.createRadialGradient(c, c, 0, c, c, c * 0.8);
  g.addColorStop(0, '#fff6dc');
  g.addColorStop(0.2, '#ffcc66');
  g.addColorStop(0.45, '#f040b3');
  g.addColorStop(0.7, '#3373ff');
  g.addColorStop(1, 'rgba(51,115,255,0)');
  ctx.fillStyle = g;
  ctx.fillRect(0, 0, lato, lato);
}

/** Quanto si avvicina la nova: nei canvas piccoli le punte non si vedono. */
export function zoomPer(lato) {
  return lato < 48 ? 1.9 : 1.25;
}

/**
 * Attacca l'orb a un canvas. Ritorna un oggetto con `stato(nome)`,
 * `qualeStato()`, `livello(0..1)` (l'ampiezza della voce, quando c'e') e
 * `ferma()`.
 */
export function creaOrb(canvas, opzioni = {}) {
  let corrente = 'spento';
  const nulla = { stato() {}, qualeStato() { return corrente; }, livello() {}, ferma() {}, disponibile: false };
  const gl = canvas.getContext('webgl2', { alpha: true, premultipliedAlpha: true, antialias: false });
  if (!gl) {
    ripiegoStatico(canvas);
    return nulla;
  }
  const vs = compila(gl, gl.VERTEX_SHADER, VERT);
  const fs = compila(gl, gl.FRAGMENT_SHADER, FRAG);
  const prog = gl.createProgram();
  if (!vs || !fs || !prog) {
    ripiegoStatico(canvas);
    return nulla;
  }
  gl.attachShader(prog, vs);
  gl.attachShader(prog, fs);
  gl.linkProgram(prog);
  if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) {
    console.warn('[orb] programma:', gl.getProgramInfoLog(prog));
    ripiegoStatico(canvas);
    return nulla;
  }

  const vao = gl.createVertexArray();
  gl.bindVertexArray(vao);
  const vbo = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, vbo);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
  gl.enableVertexAttribArray(0);
  gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);

  const u = n => gl.getUniformLocation(prog, n);
  const U = { res: u('iResolution'), time: u('iTime'), frame: u('iFrame'), mouse: u('iMouse'),
              anim: u('uAnim'), wave: u('uWavePhase'), a: u('uA'), b: u('uB'), c: u('uC') };

  let zoom = opzioni.zoom ?? zoomPer(canvas.clientWidth || 48);
  const ridimensiona = () => {
    const dpr = Math.max(1, Math.min(2, window.devicePixelRatio || 1));
    const larg = Math.max(1, Math.round((canvas.clientWidth || 48) * dpr));
    const alt = Math.max(1, Math.round((canvas.clientHeight || canvas.clientWidth || 48) * dpr));
    if (canvas.width !== larg || canvas.height !== alt) {
      canvas.width = larg;
      canvas.height = alt;
    }
    if (opzioni.zoom == null) zoom = zoomPer(canvas.clientWidth || 48);
    gl.viewport(0, 0, larg, alt);
  };
  ridimensiona();
  const osservatore = new ResizeObserver(ridimensiona);
  osservatore.observe(canvas);
  gl.clearColor(0, 0, 0, 0);

  const calmo = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false;

  // Il mouse inclina la nova e guida la pupilla. Sulla finestra intera, non
  // solo sul canvas: l'orb e' piccolo, e uno lo guarda da vicino.
  const mouse = { x: 0, y: 0 };
  const suMouse = e => {
    const r = canvas.getBoundingClientRect();
    const d = Math.max(1, Math.min(2, window.devicePixelRatio || 1));
    mouse.x = Math.max(0, Math.min(e.clientX - r.left, r.width)) * d;
    mouse.y = Math.max(0, Math.min(r.height - (e.clientY - r.top), r.height)) * d;
  };
  window.addEventListener('mousemove', suMouse);

  const cur = {};
  for (const k of MANOPOLE) cur[k] = STATI[corrente][k];
  let livello = 0, livelloLiscio = 0, livelloQuando = -1e9;
  let anim = 0, fase = 0, giro = 0, quadro = 0;
  let vivo = true, raf = 0;
  const inizio = performance.now();
  let ultimo = inizio;

  const disegna = ora => {
    if (!vivo) return;
    const dt = Math.min(0.1, (ora - ultimo) / 1000);
    ultimo = ora;
    quadro++;
    const meta = STATI[corrente] || STATI.quiete;
    const k = 1 - Math.exp(-dt * 4.0);
    for (const n of MANOPOLE) cur[n] += (meta[n] - cur[n]) * k;
    // Senza una voce vera che dia il livello, chi parla o ascolta ne ha uno
    // finto, come nella prova di Gio: se no in quei due stati l'orb e' muto.
    const finto = (corrente === 'parlo' || corrente === 'ascolto') && ora - livelloQuando > 500
      ? 0.35 + 0.35 * Math.sin(ora * 0.006) + 0.3 * Math.abs(Math.sin(ora * 0.021))
      : null;
    const bersaglio = finto ?? (ora - livelloQuando > 500 ? 0 : livello);
    livelloLiscio += (bersaglio - livelloLiscio) * (1 - Math.exp(-dt * 12.0));
    const lento = calmo ? 0.15 : 1;
    anim += dt * cur.speed * (1 + 0.6 * livelloLiscio) * lento;
    fase += dt * (cur.waveRate + (corrente === 'parlo' ? livelloLiscio * 1.2 : 0)) * lento;
    giro += dt * cur.spin * lento;

    const d = Math.max(1, Math.min(2, window.devicePixelRatio || 1));
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.useProgram(prog);
    gl.uniform3f(U.res, canvas.width, canvas.height, d);
    gl.uniform1f(U.time, (ora - inizio) / 1000);
    gl.uniform1i(U.frame, quadro);
    gl.uniform4f(U.mouse, mouse.x, mouse.y, 0, 0);
    gl.uniform1f(U.anim, anim);
    gl.uniform1f(U.wave, fase);
    gl.uniform4f(U.a, cur.energy, cur.waveAmt, cur.spike, cur.scale);
    gl.uniform4f(U.b, cur.hue, livelloLiscio, cur.reach, giro);
    gl.uniform4f(U.c, cur.rosso, cur.grigio, cur.occhio, zoom);
    gl.bindVertexArray(vao);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    raf = requestAnimationFrame(disegna);
  };
  raf = requestAnimationFrame(disegna);

  return {
    disponibile: true,
    stato(nome) { if (STATI[nome]) corrente = nome; },
    qualeStato() { return corrente; },
    /** L'ampiezza della voce, da 0 a 1; mezzo secondo senza vale zero. */
    livello(v) { livello = Math.max(0, Math.min(1, +v || 0)); livelloQuando = performance.now(); },
    ferma() {
      vivo = false;
      cancelAnimationFrame(raf);
      window.removeEventListener('mousemove', suMouse);
      osservatore.disconnect();
      gl.deleteBuffer(vbo);
      gl.deleteVertexArray(vao);
      gl.deleteProgram(prog);
    },
  };
}
