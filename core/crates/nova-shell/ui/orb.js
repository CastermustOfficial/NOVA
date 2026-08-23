/**
 * L'ORB di NOVA: la faccia della voce.
 *
 * Portato da knowledge-lab (OrbCompanion.tsx) senza React. Lo shader e' quello
 * originale, riga per riga: e' il pezzo che vale, e riscriverlo sarebbe solo
 * un modo di rompere qualcosa che gia' funziona. Cambia il guscio — da
 * componente React a poche righe che si attaccano a un canvas.
 *
 * L'orb respira con la conversazione: quieto quando riposa, si gonfia quando
 * ASCOLTA, vortica lento quando PENSA. E' l'unica cosa di NOVA sempre in
 * scena, quindi e' anche l'unico posto dove leggere a colpo d'occhio cosa
 * sta succedendo.
 */
'use strict';

/**
 * energia = ampiezza del respiro; velocita = quanto corre il tempo;
 * tinta = filtro sul campo iridescente — e' LEI che rende gli stati leggibili
 * senza scritte; grigio = desaturazione: spento l'orb resta li', in bianco e
 * nero. La voce dorme, il compagno no.
 */
export const STATI = {
  quiete:  { energia: 0.25, velocita: 0.55, tinta: [1.05, 0.85, 0.80], grigio: 0,   occhio: 0 },
  ascolto: { energia: 1.00, velocita: 1.10, tinta: [0.45, 1.15, 0.75], grigio: 0,   occhio: 0 },
  penso:   { energia: 0.35, velocita: 0.30, tinta: [0.60, 0.75, 1.35], grigio: 0,   occhio: 0 },
  parlo:   { energia: 0.95, velocita: 1.25, tinta: [1.30, 0.60, 0.90], grigio: 0,   occhio: 0 },
  agisco:  { energia: 0.70, velocita: 1.45, tinta: [1.35, 0.95, 0.45], grigio: 0,   occhio: 0 },
  chiedo:  { energia: 0.55, velocita: 0.85, tinta: [1.40, 0.80, 0.35], grigio: 0,   occhio: 0 },
  allarme: { energia: 0.90, velocita: 1.90, tinta: [1.60, 0.20, 0.20], grigio: 0,   occhio: 0 },
  spento:  { energia: 0.15, velocita: 0.30, tinta: [0.78, 0.78, 0.82], grigio: 0.9, occhio: 0 },
  bifrost: { energia: 1.20, velocita: 1.70, tinta: [1.05, 1.05, 1.05], grigio: 0,   occhio: 0 },
  occhio:  { energia: 0.80, velocita: 1.10, tinta: [1.40, 0.55, 0.18], grigio: 0,   occhio: 1 },
};

const VERT = `
attribute vec2 aP;
varying vec2 vP;
void main() { vP = aP; gl_Position = vec4(aP, 0.0, 1.0); }
`;

const FRAG = `
precision mediump float;
varying vec2 vP;
uniform float uTime;
uniform float uEnergia;
uniform vec3 uTinta;
uniform float uGrigio;
uniform float uOcchio;
uniform vec2 uMouse;
uniform float uAspetto;

float hash21(vec2 p) {
  p = fract(p * vec2(123.34, 345.45));
  p += dot(p, p + 34.345);
  return fract(p.x * p.y);
}
float vnoise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  f = f * f * (3.0 - 2.0 * f);
  return mix(
    mix(hash21(i), hash21(i + vec2(1.0, 0.0)), f.x),
    mix(hash21(i + vec2(0.0, 1.0)), hash21(i + vec2(1.0, 1.0)), f.x),
    f.y
  );
}
float fbm(vec2 p) {
  float v = 0.0;
  float a = 0.5;
  for (int i = 0; i < 4; i++) {
    v += a * vnoise(p);
    p *= 2.03;
    a *= 0.5;
  }
  return v;
}

void main() {
  vec2 p = vec2(vP.x * uAspetto, vP.y);
  float r = length(p);
  float th = atan(p.y, p.x);
  float t = uTime;

  float wob = sin(th * 3.0 - t * 1.5) * 0.5
            + sin(th * 5.0 + t * 2.1) * 0.3
            + sin(th * 7.0 - t * 3.3) * 0.2;
  float R = 0.74 + sin(t * 0.9) * 0.015 * (1.0 + uEnergia) + wob * 0.05 * (0.35 + uEnergia);
  float sPalla = mix(1.0, 0.52, uOcchio);
  R *= sPalla;
  if (uOcchio < 0.01 && r > R + 0.06) discard;

  vec2 uv = p * (1.25 - 0.2 * uEnergia);
  float d = -t * 0.5;
  float a = 0.0;
  for (float i = 0.0; i < 8.0; ++i) {
    a += cos(i - d - a * uv.x);
    d += sin(uv.y * i + a);
  }
  d += t * 0.5;
  vec3 col = vec3(cos(uv * vec2(d, a)) * 0.6 + 0.4, cos(a + d) * 0.5 + 0.5);
  col = cos(col * cos(vec3(d, a, 2.5)) * 0.5 + 0.5);
  col *= uTinta;
  col = mix(col, vec3(dot(col, vec3(0.299, 0.587, 0.114))), uGrigio);

  float bordo = smoothstep(R - 0.09, R, r);
  col += vec3(0.22) * bordo;

  float alpha = (1.0 - smoothstep(R - 0.02, R + 0.05, r)) * 0.96;

  float slit = clamp(1.0 - length((p - uMouse * 0.12 * sPalla) * vec2(9.0, 2.3) / sPalla), 0.0, 1.0);
  col = mix(col, col * 0.05, slit * uOcchio);

  float fuocoOn = smoothstep(0.55, 0.97, uOcchio);
  vec2 ve = vec2(p.x, p.y * 1.9) / mix(0.55, 1.0, fuocoOn);
  float de = length(ve);
  float ft = t * 0.7;
  vec2 pu = vec2(de * 2.0, (2.0 * atan(ve.x, ve.y)) / 6.28 * 0.3);
  float nB = fbm(pu * vec2(0.3, 4.0) * 6.0 + vec2(-ft * 1.4, 0.0));
  float nC = fbm(pu * vec2(0.1, 5.0) * 6.0 + vec2(-ft * 0.7, 0.0));
  float dm = 1.0 - de;
  float fiamma = clamp((dm + 0.25) * (nB * 1.9 + 0.15), 0.0, 2.0);
  float bag = clamp(1.0 - length(ve * vec2(0.55, 1.0)) + 0.5, 0.0, 1.0) + nC - 0.5;
  bag = clamp(bag * bag + dm * 0.5, 0.0, 1.5);
  float corpo = clamp(max(fiamma, bag), 0.0, 3.0)
    * smoothstep(-0.35, 0.1, dm)
    * smoothstep(R - 0.06, R + 0.12, r)
    * fuocoOn;
  vec3 colFiamme = mix(vec3(1.0, 0.75, 0.2), vec3(1.0, 0.25, 0.04), clamp(-dm * 1.6 + 0.55, 0.0, 1.0)) * corpo * 1.4;
  float alphaFiamme = clamp(corpo * 1.6, 0.0, 1.0);

  vec3 outPre = col * alpha + colFiamme * alphaFiamme * (1.0 - alpha);
  float outA = alpha + alphaFiamme * (1.0 - alpha);
  gl_FragColor = vec4(outPre, outA);
}
`;

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

/** Senza WebGL: un gradiente fermo, non un buco. */
function ripiegoStatico(canvas) {
  const ctx = canvas.getContext('2d');
  if (!ctx) return;
  const c = canvas.width / 2;
  const g = ctx.createRadialGradient(c, c, c * 0.15, c, c, c * 0.8);
  g.addColorStop(0, '#3a0d0d');
  g.addColorStop(0.75, '#dc2626');
  g.addColorStop(1, 'rgba(252,165,165,0)');
  ctx.fillStyle = g;
  ctx.fillRect(0, 0, canvas.width, canvas.height);
}

/**
 * Attacca l'orb a un canvas. Ritorna un oggetto con `stato(nome)` e `ferma()`.
 */
export function creaOrb(canvas) {
  let corrente = 'spento';
  const gl = canvas.getContext('webgl', { alpha: true, premultipliedAlpha: true });
  if (!gl) {
    ripiegoStatico(canvas);
    return { stato() {}, ferma() {}, disponibile: false };
  }
  const vs = compila(gl, gl.VERTEX_SHADER, VERT);
  const fs = compila(gl, gl.FRAGMENT_SHADER, FRAG);
  const prog = gl.createProgram();
  if (!vs || !fs || !prog) {
    ripiegoStatico(canvas);
    return { stato() {}, ferma() {}, disponibile: false };
  }
  gl.attachShader(prog, vs);
  gl.attachShader(prog, fs);
  gl.linkProgram(prog);
  gl.useProgram(prog);

  const quad = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, quad);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
  const aP = gl.getAttribLocation(prog, 'aP');
  gl.enableVertexAttribArray(aP);
  gl.vertexAttribPointer(aP, 2, gl.FLOAT, false, 0, 0);

  const u = n => gl.getUniformLocation(prog, n);
  const uTime = u('uTime'), uEnergia = u('uEnergia'), uTinta = u('uTinta');
  const uGrigio = u('uGrigio'), uOcchio = u('uOcchio'), uMouse = u('uMouse'), uAspetto = u('uAspetto');

  const ridimensiona = () => {
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const larg = Math.max(1, Math.round((canvas.clientWidth || 48) * dpr));
    const alt = Math.max(1, Math.round((canvas.clientHeight || canvas.clientWidth || 48) * dpr));
    if (canvas.width !== larg || canvas.height !== alt) {
      canvas.width = larg;
      canvas.height = alt;
    }
    gl.useProgram(prog);
    gl.viewport(0, 0, larg, alt);
    gl.uniform1f(uAspetto, larg / alt);
  };
  ridimensiona();
  const osservatore = new ResizeObserver(ridimensiona);
  osservatore.observe(canvas);

  gl.enable(gl.BLEND);
  gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);

  const calmo = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false;

  let vivo = true, raf = 0, tempoOrb = 0, ultimo = performance.now();
  let energia = STATI[corrente].energia;
  let velocita = STATI[corrente].velocita;
  const tinta = [...STATI[corrente].tinta];
  let grigio = STATI[corrente].grigio;
  let occhio = STATI[corrente].occhio;

  const mira = { x: 0, y: 0 }, mouse = { x: 0, y: 0 };
  const suMouse = e => {
    const box = canvas.getBoundingClientRect();
    const cx = box.left + box.width / 2, cy = box.top + box.height / 2;
    const norma = Math.max(box.width, 60);
    mira.x = Math.max(-1.5, Math.min(1.5, (e.clientX - cx) / norma));
    mira.y = Math.max(-1.5, Math.min(1.5, -(e.clientY - cy) / norma));
  };
  window.addEventListener('mousemove', suMouse);

  const disegna = ora => {
    if (!vivo) return;
    const dt = Math.min(0.1, (ora - ultimo) / 1000);
    ultimo = ora;
    const meta = STATI[corrente] || STATI.quiete;
    energia += (meta.energia - energia) * 0.08;
    velocita += ((calmo ? meta.velocita * 0.15 : meta.velocita) - velocita) * 0.08;
    for (let i = 0; i < 3; i++) tinta[i] += (meta.tinta[i] - tinta[i]) * 0.06;
    grigio += (meta.grigio - grigio) * 0.06;
    occhio += (meta.occhio - occhio) * 0.05;
    tempoOrb += dt * velocita;
    const ritmo = corrente === 'parlo'
      ? 0.45 + 0.55 * Math.abs(Math.sin(tempoOrb * 6.3) * Math.sin(tempoOrb * 2.9 + 1.7))
      : 1;
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.uniform1f(uTime, tempoOrb);
    gl.uniform1f(uEnergia, energia * ritmo);
    gl.uniform3f(uTinta, tinta[0], tinta[1], tinta[2]);
    gl.uniform1f(uGrigio, grigio);
    gl.uniform1f(uOcchio, occhio);
    mouse.x += (mira.x - mouse.x) * 0.07;
    mouse.y += (mira.y - mouse.y) * 0.07;
    gl.uniform2f(uMouse, mouse.x, mouse.y);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
    raf = requestAnimationFrame(disegna);
  };
  raf = requestAnimationFrame(disegna);

  return {
    disponibile: true,
    stato(nome) { if (STATI[nome]) corrente = nome; },
    qualeStato() { return corrente; },
    ferma() {
      vivo = false;
      cancelAnimationFrame(raf);
      window.removeEventListener('mousemove', suMouse);
      osservatore.disconnect();
    },
  };
}
