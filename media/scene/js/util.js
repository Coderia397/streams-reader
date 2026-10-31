// Shared helpers: seeded randomness, easing, keyframe interpolation, periodic noise, pixel textures.
import * as THREE from '../vendor/three/three.module.min.js';

// ---------------------------------------------------------------- seeded randomness (mulberry32)
export function rng(seed) {
  let a = (seed >>> 0) || 1;
  const f = () => {
    a |= 0; a = (a + 0x6D2B79F5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  f.range = (lo, hi) => lo + (hi - lo) * f();
  f.int = (lo, hi) => Math.floor(lo + (hi - lo + 1) * f());
  f.pick = (arr) => arr[Math.floor(f() * arr.length)];
  return f;
}

export function hashStr(s) {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) { h ^= s.charCodeAt(i); h = Math.imul(h, 16777619); }
  return h >>> 0;
}

// ---------------------------------------------------------------- easing
export const clamp = (x, a = 0, b = 1) => Math.min(b, Math.max(a, x));
export const lerp = (a, b, t) => a + (b - a) * t;
export const smooth = (t) => { t = clamp(t); return t * t * (3 - 2 * t); };
export const smoother = (t) => { t = clamp(t); return t * t * t * (t * (t * 6 - 15) + 10); };
export const easeOut = (t) => 1 - Math.pow(1 - clamp(t), 3);
export const easeIn = (t) => Math.pow(clamp(t), 3);
export const easeInOut = (t) => { t = clamp(t); return t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2; };
export const EASE = { step: (t) => (t >= 1 ? 1 : 0), linear: (t) => clamp(t), smooth, smoother, in: easeIn, out: easeOut, inout: easeInOut };

// Interpolate a list of keys [{t, ...values}] at time t. `get` maps a key to a number array.
export function sampleKeys(keys, t, get, easeDefault = 'inout') {
  if (!keys || !keys.length) return null;
  if (t <= keys[0].t) return get(keys[0]);
  const last = keys[keys.length - 1];
  if (t >= last.t) return get(last);
  let i = 0;
  while (i + 1 < keys.length && keys[i + 1].t <= t) i++;
  const a = keys[i], b = keys[i + 1];
  const u = (EASE[b.ease || easeDefault] || EASE.inout)((t - a.t) / Math.max(1e-6, b.t - a.t));
  const va = get(a), vb = get(b);
  return va.map((v, k) => lerp(v, vb[k], u));
}

// ---------------------------------------------------------------- periodic noise
// Smooth pseudo-noise built from sinusoids. With a loop period every frequency snaps to k/period, so motion loops.
export class Noise {
  constructor(seed, period = 0) {
    const r = rng(seed);
    this.period = period;
    this.terms = [];
    for (let i = 0; i < 4; i++) this.terms.push({ f: 0.25 * Math.pow(1.9, i) * r.range(0.8, 1.25), p: r() * 6.283, a: Math.pow(0.55, i) });
  }
  at(t, speed = 1) {
    let s = 0, n = 0;
    for (const k of this.terms) {
      let f = k.f * speed;
      if (this.period > 0) f = Math.max(1, Math.round(f * this.period)) / this.period;
      s += k.a * Math.sin(6.2831853 * f * t + k.p); n += k.a;
    }
    return s / n;
  }
}

// ---------------------------------------------------------------- pixel textures (voxel look)
// A tiny canvas painted per pixel with seeded jitter, sampled with NearestFilter.
export function pixelTexture(seed, size, paint, { nearest = true, srgb = true, repeat = [1, 1] } = {}) {
  const c = document.createElement('canvas');
  c.width = c.height = size;
  const g = c.getContext('2d');
  const img = g.createImageData(size, size);
  const r = rng(seed);
  for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) {
    const [R, G, B, A = 255] = paint(x, y, r);
    const o = (y * size + x) * 4;
    img.data[o] = R; img.data[o + 1] = G; img.data[o + 2] = B; img.data[o + 3] = A;
  }
  g.putImageData(img, 0, 0);
  const t = new THREE.CanvasTexture(c);
  if (nearest) { t.magFilter = THREE.NearestFilter; t.minFilter = THREE.NearestMipmapLinearFilter; }
  if (srgb) t.colorSpace = THREE.SRGBColorSpace;
  t.wrapS = t.wrapT = THREE.RepeatWrapping;
  t.repeat.set(repeat[0], repeat[1]);
  t.anisotropy = 4;
  return t;
}

export function hexRgb(hex) {
  const c = new THREE.Color(hex);
  return [Math.round(c.r * 255), Math.round(c.g * 255), Math.round(c.b * 255)];
}

// Jittered flat colour: subtle per-pixel variation around a base colour.
export function jitter(hex, amount = 10, seed = 1, size = 16) {
  const [R, G, B] = hexRgb(hex);
  return pixelTexture(seed, size, (x, y, r) => {
    const d = (r() - 0.5) * 2 * amount;
    return [clamp(R + d, 0, 255), clamp(G + d, 0, 255), clamp(B + d, 0, 255)];
  });
}

export function radialSprite(size = 64, inner = 'rgba(255,255,255,1)', outer = 'rgba(255,255,255,0)') {
  const c = document.createElement('canvas');
  c.width = c.height = size;
  const g = c.getContext('2d');
  const grd = g.createRadialGradient(size / 2, size / 2, 0, size / 2, size / 2, size / 2);
  grd.addColorStop(0, inner); grd.addColorStop(1, outer);
  g.fillStyle = grd; g.fillRect(0, 0, size, size);
  const t = new THREE.CanvasTexture(c);
  t.colorSpace = THREE.SRGBColorSpace;
  return t;
}

export const deg = (d) => d * Math.PI / 180;
