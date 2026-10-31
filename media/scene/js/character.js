// The Streams Reader mascot: a blocky, bevelled voxel character built from boxes, with an FK rig, two-bone arm IK,
// facial expressions, secondary motion and a signature pair of Streams-blue headphones with ember rings.
import * as THREE from '../vendor/three/three.module.min.js';
import { RoundedBoxGeometry } from '../vendor/three/addons/geometries/RoundedBoxGeometry.js';
import { pixelTexture, hexRgb, clamp, lerp, deg, rng, Noise, smooth } from './util.js';

const _geoCache = new Map();
function rbox(w, h, d, r = 0.02) {
  const k = [w, h, d, r].map((v) => v.toFixed(4)).join(',');
  if (!_geoCache.has(k)) _geoCache.set(k, new RoundedBoxGeometry(w, h, d, 2, Math.min(r, w / 2.2, h / 2.2, d / 2.2)));
  return _geoCache.get(k);
}

export const PALETTE = {
  skin: '#F0C29C', skinShade: '#DDA27C', blush: '#F28C7C',
  hair: '#3B2A24', hairHi: '#5A4034',
  hoodie: '#E6DCC8', hoodieDark: '#CDBFA6', hoodieTrim: '#B9A88C',
  pants: '#3A4868', pantsDark: '#2C3752',
  shoe: '#F4F1EA', sole: '#FFB45C',
  eye: '#2B1B17', brow: '#3B2A24', mouth: '#5B2730',
  blue: '#2F6BFF', blueDeep: '#1D4BBD', ember: '#FFB45C', band: '#1B1E2A',
};

function knit(hex, seed) {
  const [R, G, B] = hexRgb(hex);
  return pixelTexture(seed, 16, (x, y, r) => {
    const rib = (x % 4 === 0) ? -9 : 0;
    const d = (r() - 0.5) * 10 + rib + ((y % 2) ? 2 : -2);
    return [clamp(R + d, 0, 255), clamp(G + d, 0, 255), clamp(B + d, 0, 255)];
  });
}
function denim(hex, seed) {
  const [R, G, B] = hexRgb(hex);
  return pixelTexture(seed, 16, (x, y, r) => {
    const tw = ((x + y) % 3 === 0) ? 7 : -2;
    const d = (r() - 0.5) * 8 + tw;
    return [clamp(R + d, 0, 255), clamp(G + d, 0, 255), clamp(B + d * 1.3, 0, 255)];
  });
}
function flat(hex, seed, amt = 6) {
  const [R, G, B] = hexRgb(hex);
  return pixelTexture(seed, 8, (x, y, r) => {
    const d = (r() - 0.5) * amt;
    return [clamp(R + d, 0, 255), clamp(G + d, 0, 255), clamp(B + d, 0, 255)];
  });
}

function mat(opts) { return new THREE.MeshStandardMaterial({ roughness: 0.82, metalness: 0, ...opts }); }

// Two-bone IK in the shoulder parent's space. Returns {q (shoulder quaternion), elbow (radians)}.
// Candidate shoulder rotations for a two-bone arm whose elbow is a hinge about the upper arm's local x axis and only
// flexes forward (negative x). Returns up to four (bend-plane side x upper-arm side) candidates; the caller keeps the
// one that reaches the target with the elbow closest to the pole.
function solveArmCandidates(p, a, b, pole) {
  const dist = clamp(p.length(), 0.02, a + b - 1e-4);
  const dir = p.clone().normalize();
  const interior = Math.acos(clamp((a * a + b * b - dist * dist) / (2 * a * b), -1, 1));
  const alpha = Math.acos(clamp((a * a + dist * dist - b * b) / (2 * a * dist), -1, 1));
  const n0 = new THREE.Vector3().crossVectors(dir, pole);
  if (n0.lengthSq() < 1e-8) n0.set(1, 0, 0);
  n0.normalize();
  const out = [];
  for (const ns of [1, -1]) for (const us of [1, -1]) {
    const n = n0.clone().multiplyScalar(ns);
    const u = dir.clone().applyAxisAngle(n, us * alpha);
    const x = n, y = u.clone().negate(), z = new THREE.Vector3().crossVectors(x, y);
    const q = new THREE.Quaternion().setFromRotationMatrix(new THREE.Matrix4().makeBasis(x, y, z));
    out.push({ q, elbow: -(Math.PI - interior), elbowPos: u.clone().multiplyScalar(a) });
  }
  return out;
}

export const POSES = {
  stand: {},
  sit: { hipL: [-86, -6, 0], hipR: [-86, 6, 0], kneeL: [84, 0, 0], kneeR: [84, 0, 0], spine: [4, 0, 0], ankleL: [2, 0, 0], ankleR: [2, 0, 0] },
  sit_slouch: { hipL: [-80, -10, 0], hipR: [-80, 10, 0], kneeL: [70, 0, 0], kneeR: [76, 0, 0], spine: [-6, 0, 0], neck: [10, 0, 0], head: [16, 0, 6], ankleL: [10, 0, 0], ankleR: [10, 0, 0] },
  sit_relaxed: { spine: [-9, 0, 0], neck: [8, 0, 0], head: [17, -8, -3], hipL: [-84, -8, 0], kneeL: [86, 0, 0], ankleL: [4, 0, 0],
    hipR: [-112, -28, 0], kneeR: [96, 0, 0], ankleR: [-10, 0, 0], shoulderL: [-8, 0, 26], elbowL: [-48, 0, 0] },
  sit_cross: { hipL: [-86, -22, 8], hipR: [-80, 14, 0], kneeL: [96, 0, 0], kneeR: [74, 0, 0], spine: [2, 0, 0] },
  look_up: { neck: [-8, 0, 0], head: [-18, 0, 0] },
  look_down: { neck: [10, 0, 0], head: [16, 0, 0] },
  read: { neck: [4, 0, 0], head: [9, 0, -3] },
  wave: { shoulderR: [0, 0, -150], elbowR: [-25, 0, 0], head: [0, -6, 6] },
  arms_rest: { shoulderL: [6, 0, 6], shoulderR: [6, 0, -6], elbowL: [-12, 0, 0], elbowR: [-12, 0, 0] },
  arms_lap: { shoulderL: [-14, 0, 8], shoulderR: [-14, 0, -8], elbowL: [-62, 0, 0], elbowR: [-62, 0, 0] },
  thumbs_up: { shoulderR: [-40, 0, -10], elbowR: [-100, 0, 0], wristR: [0, 0, 90] },
  jump: { shoulderL: [0, 0, 150], shoulderR: [0, 0, -150], elbowL: [-10, 0, 0], elbowR: [-10, 0, 0], hipL: [-20, 0, 0], kneeL: [40, 0, 0], hipR: [10, 0, 0], kneeR: [20, 0, 0] },
  sit_floor: { hipL: [-88, -10, 0], hipR: [-88, 10, 0], kneeL: [6, 0, 0], kneeR: [10, 0, 0], ankleL: [-10, 0, 0], ankleR: [-10, 0, 0], spine: [-4, 0, 0] },
  arms_up: { shoulderL: [-160, 0, 18], shoulderR: [-160, 0, -18], elbowL: [-20, 0, 0], elbowR: [-20, 0, 0] },
  scratch_head: { shoulderR: [-150, 0, -30], elbowR: [-110, 0, 0], head: [6, 0, 10] },
};

export const EXPRESSIONS = {
  neutral: { open: 0.82, happy: 0, brow: 0, browTilt: 0, smile: 0.08, mouthOpen: 0, blush: 0.15 },
  calm: { open: 0.74, happy: 0, brow: -0.05, browTilt: 0, smile: 0.12, mouthOpen: 0, blush: 0.15, gaze: -0.3 },
  absorbed: { open: 0.62, happy: 0, brow: 0.05, browTilt: 0.1, smile: 0.18, mouthOpen: 0, blush: 0.2, gaze: -1 },
  look_up_calm: { open: 0.85, happy: 0, brow: 0.15, browTilt: 0, smile: 0.2, mouthOpen: 0, blush: 0.15, gaze: 0.6 },
  speak: { open: 0.85, happy: 0, brow: 0.2, browTilt: 0, smile: 0.3, mouthOpen: 0.6, blush: 0.2 },
  soft_smile: { open: 0.7, happy: 0, brow: 0.1, browTilt: 0, smile: 0.55, mouthOpen: 0, blush: 0.3 },
  smile: { open: 1, happy: 0, brow: 0.15, browTilt: 0, smile: 1, mouthOpen: 0.15, blush: 0.6 },
  happy: { open: 1, happy: 1, brow: 0.3, browTilt: 0, smile: 1, mouthOpen: 0.5, blush: 0.8 },
  sleepy: { open: 0.38, happy: 0, brow: -0.35, browTilt: -0.4, smile: 0.05, mouthOpen: 0, blush: 0.3 },
  tired: { open: 0.55, happy: 0, brow: -0.2, browTilt: -0.5, smile: 0, mouthOpen: 0, blush: 0.25 },
  surprised: { open: 1.2, happy: 0, brow: 0.6, browTilt: 0, smile: 0.1, mouthOpen: 0.9, blush: 0.4 },
  curious: { open: 1.05, happy: 0, brow: 0.35, browTilt: 0.6, smile: 0.4, mouthOpen: 0.1, blush: 0.4 },
  focused: { open: 0.85, happy: 0, brow: -0.1, browTilt: 0.15, smile: 0.25, mouthOpen: 0, blush: 0.35 },
  content: { open: 0.7, happy: 0.35, brow: 0.05, browTilt: 0, smile: 0.9, mouthOpen: 0.05, blush: 0.6 },
};
for (const e of Object.values(EXPRESSIONS)) e.gaze ??= 0;
const EXPR_KEYS = Object.keys(EXPRESSIONS.neutral);

export class Character {
  constructor({ seed = 7, palette = {}, headphones = 'neck', period = 0 } = {}) {
    this.P = { ...PALETTE, ...palette };
    this.seed = seed;
    this.period = period;
    this.headphonesMode = headphones;
    this.group = new THREE.Group();
    this.group.name = 'character';
    this.joints = {};
    this.meshes = [];
    this.noise = { head: new Noise(seed + 1, period), sway: new Noise(seed + 2, period), breath: new Noise(seed + 3, period) };
    this.build();
  }

  m(name, opts) { return (this._mats ??= {})[name] ??= mat(opts); }

  box(parent, w, h, d, mtl, pos = [0, 0, 0], r = 0.022, name) {
    const mesh = new THREE.Mesh(rbox(w, h, d, r), mtl);
    mesh.position.set(...pos);
    mesh.castShadow = true; mesh.receiveShadow = true;
    if (name) mesh.name = name;
    parent.add(mesh);
    this.meshes.push(mesh);
    return mesh;
  }

  joint(name, parent, pos) {
    const g = new THREE.Group();
    g.name = name;
    g.position.set(...pos);
    g.rotation.order = 'YXZ';
    parent.add(g);
    this.joints[name] = g;
    g.userData.rest = g.position.clone();
    return g;
  }

  build() {
    const P = this.P, s = this.seed;
    const skin = this.m('skin', { map: flat(P.skin, s + 10, 5), roughness: 0.7 });
    const hood = this.m('hoodie', { map: knit(P.hoodie, s + 11), roughness: 0.95 });
    const hoodD = this.m('hoodieD', { map: knit(P.hoodieDark, s + 12), roughness: 0.95 });
    const trim = this.m('trim', { map: flat(P.hoodieTrim, s + 13, 8), roughness: 0.95 });
    const pants = this.m('pants', { map: denim(P.pants, s + 14), roughness: 0.9 });
    const shoe = this.m('shoe', { map: flat(P.shoe, s + 15, 5), roughness: 0.6 });
    const sole = this.m('sole', { color: P.sole, roughness: 0.6 });
    const hair = this.m('hair', { map: pixelTexture(s + 16, 16, (x, y, r) => {
      const [R, G, B] = hexRgb((y % 4 === 0 && r() > 0.5) ? P.hairHi : P.hair); const d = (r() - 0.5) * 8;
      return [clamp(R + d, 0, 255), clamp(G + d, 0, 255), clamp(B + d, 0, 255)];
    }), roughness: 0.75 });
    const eye = this.m('eye', { color: P.eye, roughness: 0.25 });
    const white = this.m('white', { color: '#FFFFFF', emissive: '#FFFFFF', emissiveIntensity: 0.15, roughness: 0.2 });
    const brow = this.m('brow', { color: P.brow, roughness: 0.8 });
    const mouth = this.m('mouth', { color: P.mouth, roughness: 0.6 });
    const blush = this.m('blush', { color: P.blush, transparent: true, opacity: 0.45, roughness: 0.9, depthWrite: false });

    // root at floor level between the feet; hips joint carries everything
    const hips = this.joint('hips', this.group, [0, 0.62, 0]);
    this.box(hips, 0.40, 0.14, 0.25, pants, [0, 0.0, 0], 0.03);
    // legs
    for (const [side, sx] of [['L', 1], ['R', -1]]) {
      const hip = this.joint('hip' + side, hips, [0.1 * sx, -0.04, 0]);
      this.box(hip, 0.18, 0.32, 0.2, pants, [0, -0.15, 0], 0.03);
      this.box(hips, 0.185, 0.16, 0.2, pants, [0.1 * sx, -0.05, 0], 0.07); // hip cap
      const knee = this.joint('knee' + side, hip, [0, -0.3, 0]);
      this.box(hip, 0.172, 0.17, 0.19, pants, [0, -0.3, 0], 0.075); // knee cap (round, so it reads the same at any bend)
      this.box(knee, 0.17, 0.25, 0.185, pants, [0, -0.11, 0], 0.03);
      this.box(knee, 0.18, 0.035, 0.195, this.m('pantsD', { color: P.pantsDark, roughness: 0.9 }), [0, -0.215, 0], 0.012);
      const ankle = this.joint('ankle' + side, knee, [0, -0.25, 0]);
      this.box(ankle, 0.19, 0.075, 0.28, shoe, [0, -0.025, 0.04], 0.03);
      this.box(ankle, 0.196, 0.034, 0.286, sole, [0, -0.06, 0.04], 0.012);
    }
    // torso
    const spine = this.joint('spine', hips, [0, 0.06, 0]);
    this.box(spine, 0.46, 0.44, 0.28, hood, [0, 0.22, 0], 0.05);
    this.box(spine, 0.47, 0.07, 0.29, trim, [0, 0.025, 0], 0.025); // waistband
    this.box(spine, 0.26, 0.11, 0.03, hoodD, [0, 0.12, 0.14], 0.012); // kangaroo pocket
    this.box(spine, 0.024, 0.11, 0.02, trim, [0.05, 0.36, 0.148], 0.008); // drawstrings
    this.box(spine, 0.024, 0.09, 0.02, trim, [-0.05, 0.37, 0.148], 0.008);
    this.box(spine, 0.40, 0.13, 0.12, hoodD, [0, 0.43, -0.12], 0.05); // hood bunched behind the neck
    const neck = this.joint('neck', spine, [0, 0.44, 0]);
    this.box(neck, 0.15, 0.08, 0.14, skin, [0, 0.03, 0], 0.02);
    const head = this.joint('head', neck, [0, 0.06, 0]);
    head.scale.setScalar(0.86); // a little less toy-like than a 1:1 chibi head
    this.headMesh = this.box(head, 0.58, 0.52, 0.52, skin, [0, 0.27, 0], 0.09);
    this.box(head, 0.06, 0.1, 0.08, skin, [0.3, 0.24, -0.01], 0.02); // ears
    this.box(head, 0.06, 0.1, 0.08, skin, [-0.3, 0.24, -0.01], 0.02);
        // hair: cap, back, side burns, asymmetric fringe and a signature two-block cowlick
    this.box(head, 0.61, 0.12, 0.55, hair, [0, 0.505, -0.01], 0.05);
    this.box(head, 0.62, 0.36, 0.14, hair, [0, 0.35, -0.22], 0.05);
    this.box(head, 0.06, 0.14, 0.4, hair, [0.29, 0.42, -0.05], 0.025);
    this.box(head, 0.06, 0.14, 0.4, hair, [-0.29, 0.42, -0.05], 0.025);
    this.box(head, 0.3, 0.1, 0.05, hair, [0.15, 0.44, 0.255], 0.02);
    this.box(head, 0.14, 0.06, 0.05, hair, [0.21, 0.375, 0.258], 0.02);
    this.box(head, 0.2, 0.08, 0.05, hair, [-0.06, 0.455, 0.256], 0.02);
    this.box(head, 0.16, 0.13, 0.05, hair, [-0.215, 0.42, 0.255], 0.02);
    // textured crop: a little volume at the front, swept to one side (no topknot)
    const cow = new THREE.Group(); cow.position.set(0, 0.55, 0.0); head.add(cow);
    this.box(cow, 0.46, 0.06, 0.4, hair, [0.03, 0.0, 0.02], 0.03);
    this.box(cow, 0.24, 0.05, 0.22, hair, [0.1, 0.035, 0.08], 0.02).rotation.z = deg(-5);
    this.cowlick = cow;
    // face
    const face = new THREE.Group(); face.position.set(0, 0.25, 0.262); head.add(face); this.face = face;
    this.eyes = [];
    for (const sx of [1, -1]) {
      const g = new THREE.Group(); g.position.set(0.115 * sx, -0.01, 0); face.add(g);
      const e = this.box(g, 0.088, 0.115, 0.02, eye, [0, 0, 0], 0.014);
      const c1 = this.box(g, 0.026, 0.026, 0.012, white, [0.018 * sx, 0.028, 0.012], 0.006);
      const c2 = this.box(g, 0.012, 0.012, 0.012, white, [-0.02 * sx, -0.03, 0.012], 0.003);
      // happy "^" eyes
      const hg = new THREE.Group(); g.add(hg);
      const h1 = this.box(hg, 0.06, 0.022, 0.02, eye, [-0.022, 0, 0], 0.008); h1.rotation.z = deg(35);
      const h2 = this.box(hg, 0.06, 0.022, 0.02, eye, [0.022, 0, 0], 0.008); h2.rotation.z = deg(-35);
      const b = this.box(face, 0.1, 0.026, 0.02, brow, [0.115 * sx, 0.108, 0.004], 0.01);
      const bl = this.box(face, 0.055, 0.022, 0.008, blush, [0.19 * sx, -0.08, 0.004], 0.01);
      bl.castShadow = false;
      this.eyes.push({ g, e, c1, c2, hg, b, bl, sx });
    }
    const mg = new THREE.Group(); mg.position.set(0, -0.105, 0.004); face.add(mg);
    this.mouth = {
      g: mg,
      c: this.box(mg, 0.05, 0.018, 0.02, mouth, [0, 0, 0], 0.008),
      l: this.box(mg, 0.026, 0.02, 0.02, mouth, [0.04, 0.01, 0], 0.008),
      r: this.box(mg, 0.026, 0.02, 0.02, mouth, [-0.04, 0.01, 0], 0.008),
    };
    // arms
    for (const [side, sx] of [['L', 1], ['R', -1]]) {
      const sh = this.joint('shoulder' + side, spine, [0.3 * sx, 0.37, 0]);
      this.box(sh, 0.15, 0.27, 0.16, hood, [0, -0.1, 0], 0.04);
      const el = this.joint('elbow' + side, sh, [0, -0.235, 0]);
      this.box(sh, 0.145, 0.15, 0.155, hood, [0, -0.235, 0], 0.065); // elbow cap
      this.box(el, 0.14, 0.2, 0.15, hood, [0, -0.08, 0], 0.035);
      this.box(el, 0.15, 0.045, 0.16, trim, [0, -0.185, 0], 0.015);
      const wr = this.joint('wrist' + side, el, [0, -0.215, 0]);
      this.box(wr, 0.12, 0.12, 0.1, skin, [0, -0.05, 0], 0.03);
      this.box(wr, 0.04, 0.07, 0.045, skin, [-0.06 * sx, -0.035, 0.035], 0.015); // thumb
    }
    this.armLen = { a: 0.235, b: 0.215 + 0.05 };
    this.buildHeadphones();
    this.restPose = {};
    for (const [k, j] of Object.entries(this.joints)) this.restPose[k] = j.position.clone();
  }

  buildHeadphones() {
    const P = this.P;
    const blue = this.m('hpBlue', { color: P.blue, roughness: 0.38, metalness: 0.05 });
    const deep = this.m('hpDeep', { color: P.blueDeep, roughness: 0.5 });
    const ember = this.m('hpEmber', { color: P.ember, emissive: P.ember, emissiveIntensity: 0.35, roughness: 0.4 });
    const band = this.m('hpBand', { color: P.band, roughness: 0.6 });
    const hp = new THREE.Group(); hp.name = 'headphones';
    this.hp = hp;
    // cups
    this.cups = [];
    for (const sx of [1, -1]) {
      const cup = new THREE.Group();
      cup.position.set(0.335 * sx, 0, 0);
      this.box(cup, 0.09, 0.2, 0.17, blue, [0.0, 0, 0], 0.04);
      this.box(cup, 0.03, 0.15, 0.12, ember, [0.05 * sx, 0, 0], 0.012);
      this.box(cup, 0.04, 0.17, 0.15, deep, [-0.05 * sx, 0, 0], 0.03);
      this.box(cup, 0.035, 0.06, 0.03, band, [0, 0.12, 0], 0.01);
      hp.add(cup);
      this.cups.push(cup);
    }
    // band: an arc of blocks over the head (radius ~0.37)
    const N = 9;
    this.bandParts = [];
    for (let i = 0; i < N; i++) {
      const a = Math.PI * (i / (N - 1));
      const seg = this.box(hp, 0.085, 0.055, 0.075, (i === 0 || i === N - 1) ? band : (i % 2 ? blue : deep), [0, 0, 0], 0.02);
      seg.userData.a = a;
      this.bandParts.push(seg);
    }
    this.setHeadphones(this.headphonesMode);
  }

  // headphones: 'ears' | 'neck' | 'none' | 'earbuds'
  setHeadphones(mode) {
    this.headphonesMode = mode;
    const hp = this.hp;
    hp.removeFromParent();
    if (this.buds) this.buds.forEach((b) => b.removeFromParent());
    if (mode === 'none') return;
    if (mode === 'earbuds') {
      const white = this.m('bud', { color: '#F4F4F6', roughness: 0.3 });
      const blue = this.m('hpBlue');
      this.buds = [1, -1].map((sx) => {
        const g = new THREE.Group(); g.position.set(0.33 * sx, 0.22, 0.0);
        this.box(g, 0.05, 0.06, 0.06, white, [0, 0, 0], 0.02);
        this.box(g, 0.02, 0.03, 0.03, blue, [0.025 * sx, 0, 0], 0.008);
        this.joints.head.add(g); return g;
      });
      return;
    }
    const R = 0.36;
    if (mode === 'ears') {
      this.joints.head.add(hp);
      hp.position.set(0, 0.25, -0.01);
      hp.rotation.set(0, 0, 0);
      this.cups.forEach((c, i) => { c.position.set((i ? -1 : 1) * 0.335, 0, 0); c.rotation.set(0, 0, 0); });
      for (const seg of this.bandParts) {
        const a = seg.userData.a;
        seg.position.set(Math.cos(a) * R, Math.sin(a) * R * 1.02 + 0.06, 0);
        seg.rotation.set(0, 0, a - Math.PI / 2);
      }
    } else { // neck: resting around the neck, cups facing up/forward, band behind
      // resting around the neck: cups on the collarbones facing up, band hugging the back of the neck
      this.joints.neck.add(hp);
      hp.position.set(0, 0.0, 0.02);
      hp.rotation.set(0, 0, 0);
      this.cups.forEach((c, i) => { const sx = i ? -1 : 1; c.position.set(0.17 * sx, -0.02, 0.04); c.rotation.set(deg(-10), 0, deg(72 * sx)); });
      for (const seg of this.bandParts) {
        const a = seg.userData.a;
        seg.position.set(Math.cos(a) * 0.17, 0.02, -Math.sin(a) * 0.13 - 0.01);
        seg.rotation.set(Math.PI / 2, 0, 0);
        seg.rotation.z = 0; seg.rotation.y = a - Math.PI / 2;
      }
    }
  }

  // Cup anchor (world) for cables
  cupWorld(i = 1, out = new THREE.Vector3()) {
    if (this.headphonesMode === 'earbuds' && this.buds) return this.buds[i].getWorldPosition(out);
    return this.cups[i].localToWorld(out.set(0, -0.1, 0));
  }

  // ---------------------------------------------------------------------------------------------- per-frame update
  // st: { joints: {name:[x,y,z]}, root:{pos:[x,y,z], rotY}, hipsY, expr:{...}, walk:{speed, phase, amount},
  //       ik: {L:{w, pos:Vector3(world), quat}, R:{...}}, look:[yaw,pitch] }
  update(st, t) {
    const J = this.joints;
    this.group.position.set(...(st.root?.pos || [0, 0, 0]));
    this.group.rotation.set(0, deg(st.root?.rotY || 0), 0);
    for (const [k, j] of Object.entries(J)) {
      const r = st.joints?.[k] || [0, 0, 0];
      j.rotation.set(deg(r[0]), deg(r[1]), deg(r[2]));
      j.position.copy(this.restPose[k]);
    }
    J.hips.position.y += st.hipsY || 0;
    if (st.lookAt && (st.lookW ?? 1) > 0) this.aimHead(st.lookAt, st.lookW ?? 1);
    // secondary motion: breathing, idle sway, head drift
    const br = Math.sin(6.2831853 * this.snapF(0.22) * t);
    J.spine.scale.set(1 + 0.008 * br, 1 + 0.012 * br, 1 + 0.01 * br);
    J.spine.rotation.x += deg(0.8 * br);
    const life = st.life ?? 1;
    J.head.rotation.y += deg(3.0 * life * this.noise.head.at(t, 0.5));
    J.head.rotation.x += deg(1.6 * life * this.noise.head.at(t + 17.3, 0.6));
    J.head.rotation.z += deg(1.4 * life * this.noise.sway.at(t + 3.1, 0.4));
    J.spine.rotation.z += deg(0.8 * life * this.noise.sway.at(t, 0.3));
    if (st.look) { J.head.rotation.y += deg(st.look[0]); J.head.rotation.x += deg(st.look[1]); }
    // walking cycle
    if (st.walk && st.walk.amount > 0) this.applyWalk(st.walk, t);
    this.cowlick.rotation.z = deg(1.5 * Math.sin(6.2831853 * this.snapF(1.7) * t) * (st.walk?.amount || 0));
    // feet on the ground: lift or drop the whole body so the lowest sole touches y = ground (standing and walking)
    this.group.updateMatrixWorld(true);
    if (st.ground !== undefined && st.ground !== null) {
      const lo = Math.min(this.soleMin('L'), this.soleMin('R'));
      this.group.position.y += st.ground - lo;
      this.group.updateMatrixWorld(true);
    }
    // arm IK (world targets)
    for (const side of ['L', 'R']) {
      const ik = st.ik?.[side];
      if (ik && ik.w > 0) this.applyIK(side, ik);
    }
    this.applyExpression(st.expr || EXPRESSIONS.neutral, t, st.blinkSeed ?? this.seed);
  }

  // look-at constraint: neck and head share the yaw/pitch towards a world point (35 % / 65 %), clamped, blended
  // with the pose by w. Applied before the secondary noise so breathing and drift stay on top.
  aimHead(target, w) {
    const J = this.joints;
    this.group.updateMatrixWorld(true);
    const eye = J.neck.localToWorld(new THREE.Vector3(0, 0.3, 0.1));
    const q = J.spine.getWorldQuaternion(new THREE.Quaternion()).invert();
    const d = target.clone().sub(eye).applyQuaternion(q);
    const yaw = clamp(Math.atan2(d.x, d.z), -0.8, 0.8);
    const pitch = clamp(Math.atan2(-d.y, Math.hypot(d.x, d.z)), -0.5, 0.75);
    J.neck.rotation.x = lerp(J.neck.rotation.x, 0.35 * pitch, w); J.neck.rotation.y = lerp(J.neck.rotation.y, 0.35 * yaw, w);
    J.head.rotation.x = lerp(J.head.rotation.x, 0.65 * pitch, w); J.head.rotation.y = lerp(J.head.rotation.y, 0.65 * yaw, w);
  }

  soleMin(side) {
    const a = this.joints['ankle' + side];
    let m = Infinity;
    for (const x of [-0.095, 0.095]) for (const z of [-0.1, 0.18]) m = Math.min(m, a.localToWorld(new THREE.Vector3(x, -0.08, z)).y);
    return m;
  }

  // numbers for QA: hinge angles (deg; elbows flex negative, knees positive) and sole heights
  qa() {
    const J = this.joints, d = (r) => +(r * 180 / Math.PI).toFixed(1);
    return { elbowL: d(J.elbowL.rotation.x), elbowR: d(J.elbowR.rotation.x), kneeL: d(J.kneeL.rotation.x), kneeR: d(J.kneeR.rotation.x),
      soleL: +this.soleMin('L').toFixed(3), soleR: +this.soleMin('R').toFixed(3), ikErr: this.ikErr || {} };
  }

  snapF(f) { return this.period > 0 ? Math.max(1, Math.round(f * this.period)) / this.period : f; }

  // ground speed that matches the stride (no foot sliding): two steps per cycle, each 2 * L * sin(swing)
  strideSpeed(w) {
    const L = 0.32 + 0.25 + 0.075, swing = deg(28 * (w.amount ?? 1));
    return 4 * L * Math.sin(swing) * this.snapF(w.cadence || 0.9) * 0.92;
  }

  applyWalk(w, t) {
    const J = this.joints, a = w.amount;
    const f = this.snapF(w.cadence || 0.9); // strides per second (one full L+R cycle)
    const ph = 6.2831853 * f * t + (w.phase || 0);
    const s = Math.sin(ph), c = Math.cos(ph);
    J.hipL.rotation.x += deg(28 * s * a); J.hipR.rotation.x += deg(-28 * s * a);
    J.kneeL.rotation.x += deg(Math.max(0, -c) * 42 * a + 6 * a);
    J.kneeR.rotation.x += deg(Math.max(0, c) * 42 * a + 6 * a);
    J.ankleL.rotation.x += deg(-10 * s * a); J.ankleR.rotation.x += deg(10 * s * a);
    J.hips.position.y += a * (0.025 * Math.abs(Math.cos(ph)) - 0.012);
    J.hips.rotation.y += deg(6 * s * a);
    J.spine.rotation.y += deg(-9 * s * a);
    J.spine.rotation.x += deg(4 * a);
    J.spine.rotation.z += deg(1.5 * Math.cos(ph) * a);
    J.head.rotation.y += deg(4 * s * a);
    J.head.rotation.x += deg(1.5 * Math.cos(2 * ph) * a);
    const armA = w.arms ?? 1;
    J.shoulderL.rotation.x += deg(-24 * s * a * armA); J.shoulderR.rotation.x += deg(24 * s * a * armA);
    J.elbowL.rotation.x += deg((-14 - 10 * Math.max(0, -s)) * a * armA);
    J.elbowR.rotation.x += deg((-14 - 10 * Math.max(0, s)) * a * armA);
    J.shoulderL.rotation.z += deg(5 * a); J.shoulderR.rotation.z += deg(-5 * a);
  }

  applyIK(side, ik) {
    const J = this.joints;
    const sh = J['shoulder' + side], el = J['elbow' + side], wr = J['wrist' + side];
    const parent = sh.parent;
    parent.updateMatrixWorld(true);
    const inv = new THREE.Matrix4().copy(parent.matrixWorld).invert();
    // with a grip orientation, aim the wrist pivot so the hand centre (0,-0.05,0 in wrist space) lands on the target
    const handOff = new THREE.Vector3(0, -0.05, 0);
    const pivotW = ik.quat ? ik.pos.clone().sub(handOff.clone().applyQuaternion(ik.quat)) : ik.pos;
    const target = pivotW.clone().applyMatrix4(inv).sub(sh.position);
    const sx = side === 'L' ? 1 : -1;
    // elbows point down, out and back; the hinge only flexes forward
    const pole = new THREE.Vector3(0.7 * sx, -1, -0.6).normalize();
    const fkQ = sh.quaternion.clone(), fkE = el.rotation.x;
    let best = null;
    const b = ik.quat ? 0.215 : this.armLen.b;
    for (const c of solveArmCandidates(target, this.armLen.a, b, pole)) {
      sh.quaternion.copy(c.q); el.rotation.set(c.elbow, 0, 0);
      sh.updateMatrixWorld(true);
      const err = ik.quat ? wr.getWorldPosition(new THREE.Vector3()).distanceTo(pivotW)
        : wr.localToWorld(new THREE.Vector3(0, -0.05, 0)).distanceTo(ik.pos);
      const score = err * 10 - c.elbowPos.clone().normalize().dot(pole);
      if (!best || score < best.score) best = { ...c, err, score };
    }
    (this.ikErr ??= {})[side] = +best.err.toFixed(3);
    sh.quaternion.copy(best.q); el.rotation.set(best.elbow, 0, 0);
    if (ik.w < 1) { sh.quaternion.slerpQuaternions(fkQ, best.q, ik.w); el.rotation.x = lerp(fkE, best.elbow, ik.w); }
    sh.updateMatrixWorld(true);
    if (ik.quat) {
      const pw = new THREE.Quaternion(); el.getWorldQuaternion(pw);
      const local = pw.invert().multiply(ik.quat);
      wr.quaternion.slerp(local, ik.w * (ik.quatW ?? 1));
    }
  }


  applyExpression(e, t, seed) {
    // blinks: deterministic schedule (every 2.5-5 s), snapped to the loop period when set
    let blink = 0;
    const r = rng(seed * 31 + 5);
    const span = this.period > 0 ? this.period : 600;
    const tt = this.period > 0 ? ((t % span) + span) % span : t;
    for (let bt = r.range(0.6, 2.0); bt < span; bt += r.range(2.6, 5.2)) {
      if (this.period > 0 && bt > span - 0.4) break;
      const d = Math.abs(tt - bt);
      if (d < 0.09) blink = Math.max(blink, 1 - d / 0.09);
    }
    const open = Math.max(0.06, e.open * (1 - blink));
    for (const E of this.eyes) {
      const hp = clamp(e.happy);
      E.g.position.y = -0.01 + 0.016 * (e.gaze || 0);
      E.e.scale.set(1, open * (1 - hp) + 0.0001, 1);
      E.c1.visible = E.c2.visible = open * (1 - hp) > 0.45;
      E.c1.position.y = 0.028 * Math.min(1, open); E.c2.position.y = -0.03 * Math.min(1, open);
      E.e.visible = hp < 0.98;
      E.hg.visible = hp > 0.02;
      E.hg.scale.setScalar(Math.max(0.001, hp));
      E.hg.position.y = 0.01;
      E.b.position.y = 0.108 + 0.03 * e.brow - (open < 0.5 ? 0.02 : 0);
      E.b.rotation.z = deg(14 * e.browTilt * E.sx * (E.sx > 0 ? 1 : -1)) * (E.sx > 0 ? 1 : 0.4);
      E.bl.material.opacity = 0.3 * e.blush;
    }
    const M = this.mouth, sm = e.smile, op = e.mouthOpen;
    M.c.scale.set(1 + 0.4 * sm, 1 + 2.4 * op, 1);
    M.c.position.y = -0.006 * sm - 0.012 * op;
    M.l.position.set(0.04 + 0.012 * sm, 0.006 + 0.014 * sm, 0);
    M.r.position.set(-0.04 - 0.012 * sm, 0.006 + 0.014 * sm, 0);
    M.l.visible = M.r.visible = sm > 0.15;
  }
}

export function blendPose(names, weights) {
  const out = {};
  names.forEach((n, i) => {
    const p = POSES[n] || {};
    for (const [k, v] of Object.entries(p)) {
      out[k] ??= [0, 0, 0];
      out[k] = out[k].map((x, a) => x + v[a] * weights[i]);
    }
  });
  return out;
}

export function blendExpr(a, b, u) {
  const A = EXPRESSIONS[a] || EXPRESSIONS.neutral, B = EXPRESSIONS[b] || EXPRESSIONS.neutral;
  const o = {};
  for (const k of EXPR_KEYS) o[k] = lerp(A[k], B[k], smooth(u));
  return o;
}
