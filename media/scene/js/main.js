// Streams Reader film studio: deterministic Three.js scene driven by a shot timeline (media/shots/*.json).
// The page exposes window.studio = { load(shot), frame(i), still(preset) }. Time only advances through frame(i).
import * as THREE from '../vendor/three/three.module.min.js';
import { RoomEnvironment } from '../vendor/three/addons/environments/RoomEnvironment.js';
import { Character, POSES, EXPRESSIONS, blendExpr } from './character.js';
import { Phone, Cable, makeOpenBook, makeBook } from './props.js';
import { buildRoom } from './env_room.js';
import { buildPark } from './env_park.js';
import { buildStudio } from './env_studio.js';
import { Post } from './post.js';
import { Overlay } from './overlay.js';
import { buildProp } from './props_spot.js';
import { sampleKeys, Noise, deg, clamp, lerp, smooth, hashStr } from './util.js';

const ENVS = { room: buildRoom, park: buildPark, studio: buildStudio };

async function loadTokens() {
  try {
    const t = await (await fetch('../../brand/dist/tokens/tokens.json')).json();
    return { blue: t.palette['blue-500'], blueDeep: t.palette['blue-700'], ember: t.palette['ember-400'], void: t.palette.void };
  } catch { return { blue: '#2F6BFF', blueDeep: '#1D4BBD', ember: '#FFB45C', void: '#0B0B0C' }; }
}

// Hold presets for items, in the character's spine space. `face: true` aims the screen at the face.
const HOLDS = {
  read: { pos: [0.0, 0.34, 0.36], face: true, hands: 'both', tilt: -8 },
  read_one: { pos: [-0.06, 0.3, 0.32], face: true, hands: 'right', tilt: -6 },
  read_low: { pos: [0.02, 0.14, 0.36], face: true, hands: 'both' },
  read_high: { pos: [0.0, 0.42, 0.34], face: true, hands: 'both' },
  show: { pos: [-0.04, 0.34, 0.4], rot: [-8, 0, 0], hands: 'right' }, // screen towards the camera (+z)
  walk: { pos: [-0.14, 0.18, 0.3], face: true, hands: 'right' },
  side: { pos: [-0.3, -0.08, 0.06], rot: [80, 0, 90], hands: 'right' },
  lap: { pos: [0.0, -0.02, 0.32], rot: [-90, 0, 0], hands: 'none' },
  pocket: { pos: [-0.16, -0.12, 0.13], rot: [0, 0, 0], hands: 'none', hidden: true },
  book_read: { pos: [0.0, 0.2, 0.36], face: true, hands: 'both', tilt: 18 },
  book_lap: { pos: [0.0, 0.02, 0.36], face: true, hands: 'both', tilt: 50 },
};

class Studio {
  constructor() {
    this.canvas = document.getElementById('stage');
    this.overlay = new Overlay(document.getElementById('overlay'));
  }

  async load(shot) {
    this.shot = shot;
    const [W, H] = shot.resolution;
    this.W = W; this.H = H; this.fps = shot.fps || 30;
    this.period = shot.loop ? shot.duration : 0;
    this.alpha = !!shot.transparent;
    document.documentElement.style.setProperty('--w', W + 'px');
    document.documentElement.style.setProperty('--h', H + 'px');
    this.canvas.width = W; this.canvas.height = H;
    const r = this.renderer = new THREE.WebGLRenderer({ canvas: this.canvas, antialias: true, alpha: this.alpha, preserveDrawingBuffer: true, powerPreference: 'high-performance' });
    r.setPixelRatio(1); r.setSize(W, H, false);
    r.shadowMap.enabled = true; r.shadowMap.type = THREE.PCFSoftShadowMap;
    r.toneMapping = THREE.ACESFilmicToneMapping; r.toneMappingExposure = 1.0;
    r.outputColorSpace = THREE.SRGBColorSpace;
    this.tokens = await loadTokens();
    const pm = new THREE.PMREMGenerator(r);
    this.envMap = pm.fromScene(new RoomEnvironment(), 0.04).texture;

    this.scene = new THREE.Scene();
    this.scene.environment = this.envMap;
    this.scene.environmentIntensity = 0.25;
    this.camera = new THREE.PerspectiveCamera(35, W / H, 0.03, 120);

    // environments used by the shot (each at the origin; only the active one is visible)
    this.envs = {};
    const envNames = new Set((shot.segments || []).map((s) => s.environment?.preset || 'room'));
    for (const n of envNames) {
      const seg = shot.segments.find((s) => (s.environment?.preset || 'room') === n);
      const env = ENVS[n]({ seed: shot.seed ?? 11, ...(seg.environment?.options || {}), tokens: this.tokens });
      env.root.visible = false;
      this.scene.add(env.root);
      this.envs[n] = env;
    }
    // character, phone, book, cable
    this.char = new Character({ seed: shot.seed ?? 7, period: this.period, palette: { blue: this.tokens.blue, blueDeep: this.tokens.blueDeep, ember: this.tokens.ember, sole: this.tokens.ember, ...(shot.character_palette || {}) } });
    this.scene.add(this.char.group);
    this.phone = new Phone();
    this.scene.add(this.phone.group);
    this.book = makeOpenBook({ cover: '#7A2E2E' });
    this.scene.add(this.book);
    this.cable = new Cable('#14161E', 0.0032);
    this.scene.add(this.cable.mesh);
    this.extras = new THREE.Group(); this.scene.add(this.extras);

    this.post = new Post(r, W, H, { alpha: this.alpha });
    this.handheld = { p: new Noise(91, this.period), r: new Noise(92, this.period), q: new Noise(93, this.period) };
    await this.loadScreens(shot);
    this.overlay.load(shot.overlays || [], W, H);
    await document.fonts.ready;
    return { frames: Math.round(shot.duration * this.fps), fps: this.fps, w: W, h: H, gl: this.glInfo() };
  }

  glInfo() {
    const gl = this.renderer.getContext();
    const e = gl.getExtension('WEBGL_debug_renderer_info');
    return e ? gl.getParameter(e.UNMASKED_RENDERER_WEBGL) : 'unknown';
  }

  async loadScreens(shot) {
    this.videos = {};
    const clips = new Set((shot.segments || []).map((s) => s.screen?.clip).filter(Boolean));
    for (const c of clips) {
      const blob = await (await fetch(c)).blob();
      const url = URL.createObjectURL(blob);
      const mk = async () => {
        const v = document.createElement('video');
        v.muted = true; v.playsInline = true; v.preload = 'auto'; v.src = url;
        await new Promise((res, rej) => { v.onloadeddata = res; v.onerror = () => rej(new Error('video ' + c + ' ' + (v.error && v.error.code) + ' ' + (v.error && v.error.message) + ' ' + blob.type + ' ' + blob.size)); });
        // frames are copied through a 2D canvas: direct video uploads come out black in headless GPU Chrome
        const cv = document.createElement('canvas'); cv.width = v.videoWidth; cv.height = v.videoHeight;
        const ctx = cv.getContext('2d');
        const tex = new THREE.CanvasTexture(cv);
        tex.colorSpace = THREE.NoColorSpace; tex.minFilter = THREE.LinearMipmapLinearFilter; tex.generateMipmaps = true;
        tex.anisotropy = 8;
        return { v, tex, ctx, t: -1 };
      };
      this.videos[c] = [await mk(), await mk()];
    }
    if (!clips.size) {
      const t = new THREE.DataTexture(new Uint8Array([10, 12, 20, 255]), 1, 1); t.needsUpdate = true;
      this.phone.setTextures(t, t);
    }
  }

  async seek(slot, time) {
    const dur = slot.v.duration || 0;
    const target = clamp(time, 0, Math.max(0, dur - 0.04));
    if (Math.abs(slot.t - target) < 1e-4) return;
    await new Promise((res) => { slot.v.onseeked = res; slot.v.currentTime = target; });
    if (slot.v.requestVideoFrameCallback) await new Promise((res) => { slot.v.requestVideoFrameCallback(() => res()); setTimeout(res, 120); });
    slot.ctx.drawImage(slot.v, 0, 0);
    slot.tex.needsUpdate = true; slot.t = target;
  }

  // ------------------------------------------------------------------------------------------------ timeline
  segmentsAt(t) {
    const segs = this.shot.segments;
    const out = [];
    for (let i = 0; i < segs.length; i++) {
      const s = segs[i];
      const tr = segs[i + 1]?.transition_in;
      const trDur = tr && tr.type !== 'cut' ? (tr.duration ?? 0.6) : 0;
      if (t >= s.start && t < s.end + trDur / 2 + 1e-9 && (i === 0 || t >= s.start - ((s.transition_in && s.transition_in.type !== 'cut') ? (s.transition_in.duration ?? 0.6) / 2 : 0))) out.push(s);
    }
    if (!out.length) out.push(t < segs[0].start ? segs[0] : segs[segs.length - 1]);
    return out;
  }

  applySegment(seg, tg) {
    const t = tg - seg.start; // segment-local time
    const envName = seg.environment?.preset || 'room';
    for (const [n, e] of Object.entries(this.envs)) e.root.visible = n === envName;
    const env = this.envs[envName];
    env.update(tg, this.period);
    this.scene.background = this.alpha ? null : env.background;
    this.scene.fog = this.alpha ? null : env.fog;
    this.scene.environmentIntensity = seg.lighting?.env_intensity ?? env.envIntensity ?? 0.25;
    this.renderer.toneMappingExposure = seg.lighting?.exposure ?? env.exposure ?? 1;
    if (env.setTime) env.setTime(seg.environment?.options?.time || 'golden');
    // lighting overrides: {lights: {name: {intensity, color}}}
    for (const [k, L] of Object.entries(env.lights)) {
      L.userData.base ??= { i: L.intensity, c: L.color.clone() };
      const o = seg.lighting?.lights?.[k];
      L.intensity = o?.intensity ?? L.userData.base.i;
      L.color.copy(o?.color ? new THREE.Color(o.color) : L.userData.base.c);
    }

    // ---- character
    const C = seg.character || {};
    const anchor = C.anchor && env.anchors[C.anchor] ? env.anchors[C.anchor] : new THREE.Vector3();
    const off = sampleKeys(C.keys || [{ t: 0, pos: [0, 0, 0], rotY: 0 }], t, (k) => [...(k.pos || [0, 0, 0]), k.rotY || 0]);
    const walk = C.walk ? { amount: C.walk.amount ?? 1, cadence: C.walk.cadence ?? 0.9, arms: C.walk.arms ?? 1, phase: 0 } : null;
    let rootPos = [anchor.x + off[0], anchor.y + off[1], anchor.z + off[2]];
    if (C.walk && C.walk.speed) {
      const dir = deg(C.walk.heading ?? 90);
      const v = C.walk.speed === 'auto' ? this.char.strideSpeed(walk) : C.walk.speed;
      rootPos[0] += Math.sin(dir) * v * t; rootPos[2] += Math.cos(dir) * v * t;
    }
    if (this.char.headphonesMode !== (C.headphones || 'neck')) this.char.setHeadphones(C.headphones || 'neck');
    const joints = this.poseAt(C.poses || [{ t: 0, pose: 'stand' }], t);
    const sitting = (C.poses || []).some((p) => String(p.pose).includes('sit'));
    const exprKeys = C.expressions || [{ t: 0, expr: 'neutral' }];
    const expr = this.exprAt(exprKeys, t);
    const lookAt = sampleKeys(C.look || [{ t: 0, v: [0, 0] }], t, (k) => k.v);
    const st = {
      root: { pos: rootPos, rotY: off[3] }, joints, expr, look: lookAt, walk,
      hipsY: C.hipsY ?? (sitting ? (env.anchors.seatHipsY ?? -0.12) : 0), life: C.life ?? 1,
      ground: (sitting || C.hipsY) ? null : (C.ground ?? 0),
    };
    // held item (phone/book) placed in spine space, hands solved with IK
    const holdKeys = C.hold || [{ t: 0, item: 'phone', hold: 'read' }];
    let hk = holdKeys[0];
    for (const k of holdKeys) if (k.t <= t) hk = k;
    this.char.update({ ...st, ik: {} }, tg); // FK pass to know the spine transform
    const H = HOLDS[hk.hold] || HOLDS.read;
    const item = hk.item === 'book' ? this.book : this.phone.group;
    this.phone.group.visible = hk.item === 'phone' && !H.hidden;
    if (hk.item === 'none') this.phone.group.visible = false;
    this.book.visible = hk.item === 'book';
    if (C.phone_visible === false) this.phone.group.visible = false;
    const spine = this.char.joints.spine, head = this.char.joints.head;
    spine.updateMatrixWorld(true);
    const p = new THREE.Vector3(...H.pos);
    const hp = sampleKeys(C.hold_offset || [{ t: 0, v: [0, 0, 0] }], t, (k) => k.v);
    p.add(new THREE.Vector3(...hp));
    spine.localToWorld(p);
    item.position.copy(p);
    if (hk.item === 'phone') this.phone.group.scale.setScalar(C.phone_scale ?? 1.6);
    if (H.face) {
      const eye = head.localToWorld(new THREE.Vector3(0, 0.24, 0.05));
      // the phone's top points forward-and-up from the chest (a held phone never has its top towards the face)
      const up = new THREE.Vector3(0, 0.6, 1).normalize().applyQuaternion(spine.getWorldQuaternion(new THREE.Quaternion()));
      const m = new THREE.Matrix4().lookAt(eye, p, up); // +z of item towards the eye
      item.quaternion.setFromRotationMatrix(m);
      if (H.tilt) item.rotateX(deg(H.tilt));
      if (C.hold_yaw) item.rotateY(deg(C.hold_yaw));
      if (C.hold_pitch) item.rotateX(deg(C.hold_pitch));
    } else {
      const q = spine.getWorldQuaternion(new THREE.Quaternion());
      item.quaternion.copy(q).multiply(new THREE.Quaternion().setFromEuler(new THREE.Euler(...(H.rot || [0, 0, 0]).map(deg))));
    }
    item.updateMatrixWorld(true);
    const ik = {};
    const hw = sampleKeys(C.hands_w || [{ t: 0, v: [1] }], t, (k) => k.v)[0];
    // free IK targets in the character's root space (spot poses): {R: [x,y,z], L: [x,y,z]}
    if (C.ik) for (const [side, v] of Object.entries(C.ik)) ik[side] = { w: 1, pos: this.char.group.localToWorld(new THREE.Vector3(...v)) };
    if (!H.hidden && H.hands !== 'none' && hk.item !== 'none') {
      const isBook = hk.item === 'book';
      // phone: palms behind the lower corners, so the thumbs sit at the bottom edge and the screen stays clear
      const R = item.localToWorld(new THREE.Vector3(isBook ? 0.14 : 0.03, isBook ? -0.02 : -0.07, isBook ? -0.03 : -0.038));
      const L = item.localToWorld(new THREE.Vector3(isBook ? -0.14 : -0.03, isBook ? -0.02 : -0.075, isBook ? -0.03 : -0.038));
      const grip = isBook ? null : item.getWorldQuaternion(new THREE.Quaternion()).multiply(new THREE.Quaternion().setFromAxisAngle(new THREE.Vector3(0, 0, 1), Math.PI));
      if (H.hands === 'both' || H.hands === 'right') ik.R = { w: hw, pos: R, quat: grip };
      if (H.hands === 'both') ik.L = { w: hw, pos: L, quat: grip };
    }
    this._ikT = ik;
    this.char.update({ ...st, ik }, tg);
    this._ground = st.ground;
    this.char.joints.head.visible = !C.pov;
    this.placeProps(seg);
    env.focus?.(this.char.group.position);
    // cable from the left cup to the phone (wired headphones)
    const wired = C.cable && ['ears', 'earbuds'].includes(this.char.headphonesMode);
    this.cable.mesh.visible = !!wired;
    if (wired) {
      this.char.group.updateMatrixWorld(true);
      const a = this.char.cupWorld(1);
      const b = this.phone.group.visible ? this.phone.plug.getWorldPosition(new THREE.Vector3()) : this.char.joints.hipR.localToWorld(new THREE.Vector3(-0.02, -0.1, 0.1));
      const chest = spine.localToWorld(new THREE.Vector3(-0.12, 0.3, 0.17));
      const sway = 0.02 * Math.sin(6.283 * this.char.snapF(1.8) * tg) * (walk ? 1 : 0.2);
      this.cable.update(a, b, 0.05, sway, [chest.clone().add(new THREE.Vector3(sway, 0, 0)), chest.clone().lerp(b, 0.5).add(new THREE.Vector3(sway * 1.5, -0.06, 0.02))]);
    }
    // phone screen: clip time and brightness
    const S = seg.screen;
    this.phone.uniforms.on.value = S ? sampleKeys(S.on || [{ t: 0, v: [1] }], t, (k) => k.v)[0] : 0;
    this.phone.uniforms.bright.value = S?.brightness ?? 1.3;
    const glowOn = this.phone.uniforms.on.value * (this.phone.group.visible ? 1 : 0);
    this.phone.light.intensity = (seg.lighting?.screen_glow ?? 0.12) * glowOn;
    this.phone.area.intensity = (seg.lighting?.screen_area ?? 5) * glowOn;
    this.phone.light.color.set(seg.lighting?.screen_color || '#9DB8FF');

    // ---- camera
    const keys = seg.camera?.keys || [{ t: 0, pos: [0, 1.2, 3], target: [0, 1, 0], fov: 35 }];
    const resolve = (v) => {
      if (Array.isArray(v)) return v;
      if (v === 'phone') return this.phone.group.getWorldPosition(new THREE.Vector3()).toArray();
      if (v === 'face') return head.localToWorld(new THREE.Vector3(0, 0.25, 0.2)).toArray();
      if (v === 'eye') return head.localToWorld(new THREE.Vector3(0, 0.25, 0.12)).toArray();
      if (v === 'chest') return spine.localToWorld(new THREE.Vector3(0, 0.25, 0.1)).toArray();
      if (v === 'char') return this.char.group.position.toArray();
      if (env.anchors[v]) return env.anchors[v].toArray();
      return [0, 1, 0];
    };
    // keys may be relative to the character root (follow: true) for tracking shots
    const follow = seg.camera?.follow ? this.char.group.position.clone() : new THREE.Vector3();
    const facing = deg(off[3]);
    const cam = sampleKeys(keys, t, (k) => {
      const tg2 = resolve(k.target).map((v, i) => v + (k.aim ? k.aim[i] : 0));
      let pos = typeof k.pos === 'string' ? resolve(k.pos) : (k.pos || [0, 1, 2]);
      if (k.offset) pos = pos.map((v, i) => v + k.offset[i]);
      if (k.orbit) { // [yaw, pitch, dist] around the target; yaw 0 = in front of the character, 90 = its left
        const [yw, pt, ds] = k.orbit, a = facing + deg(yw), e = deg(pt);
        pos = [tg2[0] + ds * Math.cos(e) * Math.sin(a), tg2[1] + ds * Math.sin(e), tg2[2] + ds * Math.cos(e) * Math.cos(a)];
        if (k.shift) pos = pos.map((v, i) => v + k.shift[i]);
        const f2 = seg.camera?.follow ? follow : new THREE.Vector3();
        pos = [pos[0] - f2.x, pos[1] - f2.y, pos[2] - f2.z];
      }
      const f = seg.camera?.follow ? follow : new THREE.Vector3();
      return [pos[0] + f.x, pos[1] + f.y, pos[2] + f.z, ...(Array.isArray(k.target) ? [tg2[0] + f.x, tg2[1] + f.y, tg2[2] + f.z] : tg2), k.fov ?? 35, k.roll ?? 0];
    });
    const hh = seg.camera?.handheld ?? 0.3;
    const T = tg * 0.9;
    this.camera.position.set(cam[0] + hh * 0.012 * this.handheld.p.at(T), cam[1] + hh * 0.009 * this.handheld.p.at(T + 40), cam[2] + hh * 0.006 * this.handheld.p.at(T + 80));
    this.camera.fov = cam[6];
    this.camera.aspect = this.W / this.H;
    this.camera.updateProjectionMatrix();
    this.camera.up.set(0, 1, 0);
    this.camera.lookAt(cam[3], cam[4], cam[5]);
    this.camera.rotateZ(deg(cam[7] + hh * 0.35 * this.handheld.r.at(T)));
    this.camera.rotateX(deg(hh * 0.25 * this.handheld.q.at(T + 9)));
    this.camera.updateMatrixWorld(true);
    // focus: number, or a subject name, keyed for rack focus
    const fkeys = seg.camera?.focus || [{ t: 0, on: 'face' }];
    const fdist = sampleKeys(fkeys, t, (k) => [typeof k.on === 'number' ? k.on : this.camera.position.distanceTo(new THREE.Vector3(...resolve(k.on)))]);
    const look = { ...(env.look || {}), ...(seg.look || {}) };
    look.dof = { ...(env.look?.dof || {}), ...(seg.look?.dof || {}), focus: fdist[0] };
    if (seg.camera?.aperture !== undefined) look.dof.aperture = seg.camera.aperture;
    look.grade = { ...(env.look?.grade || {}), ...(seg.look?.grade || {}) };
    look.bloom = { ...(env.look?.bloom || {}), ...(seg.look?.bloom || {}) };
    if (this.qa) { // QA close-up on a joint: orbit [yaw, pitch, dist] relative to the character's facing
      const j = this.char.joints[this.qa.joint];
      const p = j.getWorldPosition(new THREE.Vector3());
      const [yw, pt, ds] = this.qa.orbit, a = deg(off[3]) + deg(yw), e = deg(pt);
      this.camera.position.set(p.x + ds * Math.cos(e) * Math.sin(a), p.y + ds * Math.sin(e), p.z + ds * Math.cos(e) * Math.cos(a));
      this.camera.up.set(0, 1, 0); this.camera.fov = 38; this.camera.updateProjectionMatrix(); this.camera.lookAt(p);
      this.camera.updateMatrixWorld(true);
      this.char.joints.head.visible = true;
      look.dof.focus = ds;
    }
    // fade keys (to/from black)
    if (seg.fade) look.grade.fade = sampleKeys(seg.fade, t, (k) => [k.v])[0];
    return look;
  }

  // props: [{type, attach: 'handR'|'handL'|'root'|'world', pos, rot (deg), scale}] built once per segment
  placeProps(seg) {
    for (const p of this._props || []) p.removeFromParent();
    this._props = [];
    const list = seg.props || [];
    seg._built ??= list.map((p) => buildProp(p.type, this.tokens));
    list.forEach((p, i) => {
      const g = seg._built[i];
      const parent = p.attach === 'handR' ? this.char.joints.wristR : p.attach === 'handL' ? this.char.joints.wristL
        : p.attach === 'root' ? this.char.group : p.attach === 'head' ? this.char.joints.head : this.scene;
      parent.add(g);
      const base = p.attach && p.attach.startsWith('hand') ? [0, -0.08, 0.02] : [0, 0, 0];
      g.position.set(...(p.pos || [0, 0, 0]).map((v, k) => v + base[k]));
      g.rotation.set(...(p.rot || [0, 0, 0]).map(deg));
      g.scale.setScalar(p.scale ?? 1);
      this._props.push(g);
    });
  }

  poseAt(keys, t) {
    const get = (k) => {
      const names = String(k.pose).split('+');
      const o = {};
      for (const n of names) for (const [j, v] of Object.entries(POSES[n] || {})) { o[j] ??= [0, 0, 0]; o[j] = o[j].map((x, i) => x + v[i]); }
      for (const [j, v] of Object.entries(k.joints || {})) { o[j] ??= [0, 0, 0]; o[j] = o[j].map((x, i) => x + v[i]); }
      return o;
    };
    let a = keys[0], b = keys[0];
    for (const k of keys) { if (k.t <= t) a = k; }
    const idx = keys.indexOf(a);
    b = keys[idx + 1] || a;
    const dur = b.t - a.t;
    const blendIn = b.blend ?? Math.min(0.6, dur);
    const u = b === a ? 0 : smooth((t - (b.t - blendIn)) / Math.max(1e-6, blendIn));
    const A = get(a), B = get(b);
    const out = {};
    for (const j of new Set([...Object.keys(A), ...Object.keys(B)])) {
      const va = A[j] || [0, 0, 0], vb = B[j] || [0, 0, 0];
      out[j] = va.map((x, i) => lerp(x, vb[i], u));
    }
    return out;
  }

  exprAt(keys, t) {
    let a = keys[0];
    for (const k of keys) if (k.t <= t) a = k;
    const b = keys[keys.indexOf(a) + 1];
    if (!b) return blendExpr(a.expr, a.expr, 0);
    const bl = b.blend ?? 0.35;
    return blendExpr(a.expr, b.expr, (t - (b.t - bl)) / bl);
  }

  qaMetrics() {
    const m = this.char.qa(), issues = [];
    if (this._ground !== null && this._ground !== undefined) {
      const lo = Math.min(m.soleL, m.soleR);
      if (Math.abs(lo - this._ground) > 0.01) issues.push(`feet ${lo > this._ground ? 'float' : 'sink'} ${lo.toFixed(3)}`);
    }
    for (const s of ['L', 'R']) if (m.ikErr[s] > 0.02 && this._ikT?.[s]) issues.push(`hand ${s} misses target by ${m.ikErr[s]}`);
    if (m.elbowL > 2 || m.elbowR > 2) issues.push(`elbow bends backwards ${m.elbowL}/${m.elbowR}`);
    if (m.kneeL < -2 || m.kneeR < -2) issues.push(`knee bends backwards ${m.kneeL}/${m.kneeR}`);
    this._cablePts = this.cable.mesh.visible ? this.cable.points : null;
    if (this.cable.mesh.visible && this._cablePts) {
      const sp = this.char.joints.spine, inv = new THREE.Matrix4().copy(sp.matrixWorld).invert();
      const inside = this._cablePts.filter((p) => { const q = p.clone().applyMatrix4(inv); return Math.abs(q.x) < 0.22 && q.y > 0.02 && q.y < 0.43 && Math.abs(q.z) < 0.13; }).length;
      if (inside) issues.push(`cable inside torso at ${inside} points`);
    }
    return { ...m, issues };
  }

  debug() {
    const w = (o, v = [0, 0, 0]) => o.localToWorld(new THREE.Vector3(...v)).toArray().map((x) => +x.toFixed(3));
    const J = this.char.joints;
    return { phone: w(this.phone.group), phoneVisible: this.phone.group.visible, phoneScale: this.phone.group.scale.x,
      handR: w(J.wristR, [0, -0.05, 0]), handL: w(J.wristL, [0, -0.05, 0]), shR: w(J.shoulderR), shL: w(J.shoulderL),
      head: w(J.head, [0, 0.25, 0]), spine: w(J.spine), hips: w(J.hips), cam: this.camera.position.toArray(), ikErr: this.char.ikErr, tR: this._ikT?.R?.pos?.toArray(), tL: this._ikT?.L?.pos?.toArray() };
  }

  // ------------------------------------------------------------------------------------------------ frames
  async frame(i) {
    const fps = this.fps, tg = i / fps;
    const segs = this.segmentsAt(tg);
    if (this.alpha) { // transparent stills: straight render with tone mapping, no post (keeps the alpha channel)
      this.applySegment(segs[0], tg);
      const r = this.renderer;
      r.setRenderTarget(null); r.setClearColor(0x000000, 0); r.clear();
      r.render(this.scene, this.camera);
      return true;
    }
    const mb = this.shot.motion_blur || 1, shutter = this.shot.shutter ?? 0.5;
    const draws = [];
    for (const seg of segs) {
      const S = seg.screen;
      if (S?.clip) {
        const slots = this.videos[S.clip];
        const local = tg - seg.start;
        let ct = (S.in ?? 0) + local * (S.rate ?? 1);
        let mixAB = 0;
        if (this.period > 0 && S.loop_xfade) { // crossfade the screen back to its in-point near the loop end
          const xf = S.loop_xfade, rem = this.shot.duration - tg;
          if (rem < xf) { mixAB = smooth(1 - rem / xf); await this.seek(slots[1], (S.in ?? 0) + (local - this.shot.duration)); }
        }
        if (S.hold_at !== undefined && local >= S.hold_at) ct = (S.in ?? 0) + S.hold_at;
        await this.seek(slots[0], Math.max(0, ct));
        this.phone.setTextures(slots[0].tex, mixAB > 0 ? slots[1].tex : slots[0].tex);
        this.phone.uniforms.mixAB.value = mixAB;
      }
      draws.push(seg);
    }
    const render = (seg, target, tsub, idx) => {
      const look = this.applySegment(seg, tsub);
      this.post.configure(this.scene, this.camera, look, i * 7 + idx);
      this.post.renderTo(target);
    };
    const drawAt = (seg, target) => this.post.frame((k, n, tgt) => {
      const ts = tg + (n > 1 ? ((k + 0.5) / n - 0.5) * shutter / fps : 0);
      render(seg, tgt, ts, k);
    }, mb);
    let final;
    if (draws.length === 1) {
      final = drawAt(draws[0]);
    } else {
      const [A, B] = draws;
      const tr = B.transition_in || { type: 'dissolve', duration: 0.6 };
      const d = tr.duration ?? 0.6;
      const u = clamp((tg - (B.start - d / 2)) / d);
      const ta = drawAt(A);
      if (!this._hold) this._hold = new THREE.WebGLRenderTarget(this.W, this.H, { type: THREE.HalfFloatType });
      this.post.copy(ta.texture, this._hold);
      const tb = drawAt(B);
      if (!this._mix) this._mix = new THREE.WebGLRenderTarget(this.W, this.H, { type: THREE.HalfFloatType });
      const mode = tr.type === 'whip' || tr.type === 'slide' ? 1 : 0;
      const eased = tr.type === 'whip' ? (u < 0.5 ? 4 * u * u * u : 1 - Math.pow(-2 * u + 2, 3) / 2) : smooth(u);
      this.post.blend(this._hold.texture, tb.texture, eased, mode, tr.direction || [1, 0], this._mix);
      final = this._mix;
    }
    this.post.present(final.texture);
    this.overlay.update(tg);
    return true;
  }
}

window.studio = new Studio();
window.studioReady = true;
document.documentElement.dataset.ready = '1';
