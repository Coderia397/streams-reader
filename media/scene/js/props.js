// Props: the phone (with the live app as its screen), a paper book, a mug with steam, headphone cable.
import * as THREE from '../vendor/three/three.module.min.js';
import { RoundedBoxGeometry } from '../vendor/three/addons/geometries/RoundedBoxGeometry.js';
import { radialSprite, rng, pixelTexture, hexRgb, clamp } from './util.js';
import { RectAreaLightUniformsLib } from '../vendor/three/addons/lights/RectAreaLightUniformsLib.js';
RectAreaLightUniformsLib.init();

// Screen size follows the app viewport 390x844.
export const PHONE = { w: 0.092, h: 0.192, d: 0.0095, screenW: 0.084, screenH: 0.084 * 844 / 390 };

export class Phone {
  constructor({ body = '#20232C', frame = '#3A3F4D' } = {}) {
    this.group = new THREE.Group();
    this.group.name = 'phone';
    const g = new RoundedBoxGeometry(PHONE.w, PHONE.h, PHONE.d, 3, 0.011);
    const bodyM = new THREE.MeshStandardMaterial({ color: body, roughness: 0.32, metalness: 0.55 });
    const mesh = new THREE.Mesh(g, bodyM);
    mesh.castShadow = true; mesh.receiveShadow = true;
    this.group.add(mesh);
    const rim = new THREE.Mesh(new RoundedBoxGeometry(PHONE.w + 0.001, PHONE.h + 0.001, PHONE.d * 0.55, 3, 0.011),
      new THREE.MeshStandardMaterial({ color: frame, roughness: 0.25, metalness: 0.85 }));
    this.group.add(rim);
    // glass + screen: a shader mixing two video frames (for loop crossfades) with brightness control
    this.uniforms = {
      mapA: { value: null }, mapB: { value: null }, mixAB: { value: 0 }, bright: { value: 1.0 }, on: { value: 1 },
      offColor: { value: new THREE.Color('#05060A') },
    };
    const screenM = new THREE.ShaderMaterial({
      uniforms: this.uniforms, toneMapped: false,
      vertexShader: 'varying vec2 vUv; void main(){ vUv=uv; gl_Position=projectionMatrix*modelViewMatrix*vec4(position,1.0); }',
      fragmentShader: `uniform sampler2D mapA; uniform sampler2D mapB; uniform float mixAB; uniform float bright; uniform float on; uniform vec3 offColor; varying vec2 vUv;
        vec3 toLin(vec3 c){ return pow(c, vec3(2.2)); }
        void main(){
          vec3 a = toLin(texture2D(mapA, vUv).rgb); vec3 b = toLin(texture2D(mapB, vUv).rgb);
          vec3 c = mix(a, b, mixAB) * bright;
          // rounded screen corners
          vec2 p = abs(vUv - 0.5) * vec2(${PHONE.screenW.toFixed(4)}, ${PHONE.screenH.toFixed(4)});
          vec2 q = p - vec2(${(PHONE.screenW / 2 - 0.008).toFixed(4)}, ${(PHONE.screenH / 2 - 0.008).toFixed(4)});
          float d = length(max(q, 0.0)) - 0.008;
          if (d > 0.0) c = offColor;
          gl_FragColor = vec4(mix(offColor, c, on), 1.0);
          #include <colorspace_fragment>
        }`,
    });
    const screen = new THREE.Mesh(new THREE.PlaneGeometry(PHONE.screenW, PHONE.screenH), screenM);
    screen.position.z = PHONE.d / 2 + 0.0004;
    this.group.add(screen);
    this.screen = screen;
    // dynamic island
    const isl = new THREE.Mesh(new RoundedBoxGeometry(0.02, 0.0055, 0.001, 2, 0.0027), new THREE.MeshBasicMaterial({ color: '#000' }));
    isl.position.set(0, PHONE.screenH / 2 - 0.006, PHONE.d / 2 + 0.0008);
    this.group.add(isl);
    // glass reflection layer
    const glass = new THREE.Mesh(new THREE.PlaneGeometry(PHONE.screenW, PHONE.screenH),
      new THREE.MeshPhysicalMaterial({ color: '#000', roughness: 0.18, metalness: 0, transparent: true, opacity: 0.05, clearcoat: 0.5, envMapIntensity: 1.0 }));
    glass.position.z = PHONE.d / 2 + 0.0007;
    // (glass layer kept out: its lamp highlight sat on top of the text)
    // camera bump on the back
    const bump = new THREE.Mesh(new RoundedBoxGeometry(0.034, 0.034, 0.003, 2, 0.008), bodyM);
    bump.position.set(-0.02, 0.066, -PHONE.d / 2 - 0.0012);
    this.group.add(bump);
    // light the screen casts onto the face/hands
    // the screen lights the face and hands: an area light the size of the screen plus a soft point fill
    this.area = new THREE.RectAreaLight('#CFDCFF', 0, PHONE.screenW, PHONE.screenH);
    this.area.position.z = PHONE.d / 2 + 0.002;
    this.area.rotation.y = 0; // RectAreaLight emits along -z; turned to face +z below
    this.area.lookAt(0, 0, 1);
    this.group.add(this.area);
    this.light = new THREE.PointLight('#A9C0FF', 0.0, 0.9, 2);
    this.light.position.set(0, 0.02, 0.18);
    this.group.add(this.light);
    // cable plug point (bottom edge)
    this.plug = new THREE.Object3D(); this.plug.position.set(0, -PHONE.h / 2, 0); this.group.add(this.plug);
  }

  setTextures(a, b) { this.uniforms.mapA.value = a; this.uniforms.mapB.value = b || a; }
}

export function makeBook({ w = 0.15, h = 0.22, d = 0.035, cover = '#7A2E2E', seed = 3, open = 0 } = {}) {
  const g = new THREE.Group();
  const coverM = new THREE.MeshStandardMaterial({ map: pixelTexture(seed, 16, (x, y, r) => {
    const [R, G, B] = hexRgb(cover); const band = (y === 3 || y === 12) ? 30 : 0; const d2 = (r() - 0.5) * 10 + band;
    return [clamp(R + d2, 0, 255), clamp(G + d2 * 0.8, 0, 255), clamp(B + d2 * 0.5, 0, 255)];
  }), roughness: 0.8 });
  const pageM = new THREE.MeshStandardMaterial({ color: '#EFE6D2', roughness: 0.95 });
  const c = new THREE.Mesh(new RoundedBoxGeometry(w, h, d, 2, 0.004), coverM);
  const p = new THREE.Mesh(new THREE.BoxGeometry(w * 0.94, h * 0.95, d * 0.8), pageM);
  p.position.x = 0.004;
  [c, p].forEach((m) => { m.castShadow = m.receiveShadow = true; g.add(m); });
  return g;
}

// An open book lying in a lap or on a table: two page blocks with a spine crease.
export function makeOpenBook({ w = 0.14, h = 0.2, cover = '#7A2E2E', seed = 4 } = {}) {
  const g = new THREE.Group();
  const coverM = new THREE.MeshStandardMaterial({ color: cover, roughness: 0.8 });
  const pageTex = pixelTexture(seed, 32, (x, y, r) => {
    const line = (y % 3 === 1 && x > 3 && x < 29 && !(y > 26 && x > 20)) ? -38 + (r() * 10) : 0;
    const v = 236 + (r() - 0.5) * 6 + line;
    return [v, v - 6, v - 18];
  });
  const pageM = new THREE.MeshStandardMaterial({ map: pageTex, roughness: 0.95 });
  for (const sx of [1, -1]) {
    const half = new THREE.Group(); half.rotation.y = sx * 0.12; g.add(half);
    const cv = new THREE.Mesh(new THREE.BoxGeometry(w, h, 0.004), coverM);
    cv.position.set(sx * w / 2, 0, -0.012);
    const pg = new THREE.Mesh(new THREE.BoxGeometry(w * 0.96, h * 0.95, 0.018), pageM);
    pg.position.set(sx * w / 2, 0, 0);
    [cv, pg].forEach((m) => { m.castShadow = m.receiveShadow = true; half.add(m); });
  }
  return g;
}

export function makeMug({ color = '#2F6BFF', seed = 9 } = {}) {
  const g = new THREE.Group();
  const m = new THREE.MeshStandardMaterial({ color, roughness: 0.35 });
  const body = new THREE.Mesh(new RoundedBoxGeometry(0.08, 0.095, 0.08, 2, 0.012), m);
  body.position.y = 0.0475;
  const handle = new THREE.Mesh(new RoundedBoxGeometry(0.02, 0.055, 0.045, 2, 0.008), m);
  handle.position.set(0.05, 0.05, 0);
  const tea = new THREE.Mesh(new THREE.BoxGeometry(0.066, 0.004, 0.066), new THREE.MeshStandardMaterial({ color: '#6B3F1F', roughness: 0.15 }));
  tea.position.y = 0.088;
  [body, handle, tea].forEach((x) => { x.castShadow = x.receiveShadow = true; g.add(x); });
  // steam: a few soft sprites that rise and fade (periodic in t)
  const sprite = radialSprite(64, 'rgba(255,255,255,0.55)', 'rgba(255,255,255,0)');
  const r = rng(seed);
  g.userData.steam = [];
  for (let i = 0; i < 7; i++) {
    const s = new THREE.Sprite(new THREE.SpriteMaterial({ map: sprite, transparent: true, depthWrite: false, opacity: 0 }));
    s.userData = { ph: r(), dx: r.range(-0.01, 0.01), sp: r.range(0.8, 1.2) };
    g.add(s); g.userData.steam.push(s);
  }
  g.userData.update = (t, period = 0) => {
    for (const s of g.userData.steam) {
      const T = period > 0 ? period / Math.max(1, Math.round(period / 2.4)) : 2.4;
      const u = ((t / T) + s.userData.ph) % 1;
      s.position.set(s.userData.dx + 0.012 * Math.sin(u * 9 + s.userData.ph * 6), 0.1 + u * 0.16, 0);
      s.scale.setScalar(0.03 + u * 0.06);
      s.material.opacity = 0.22 * Math.sin(Math.PI * u);
    }
  };
  return g;
}

// A cable from a to b with sag; geometry rebuilt each frame (cheap: 40 segments).
export class Cable {
  constructor(color = '#15171F', radius = 0.0035) {
    this.mat = new THREE.MeshStandardMaterial({ color, roughness: 0.5 });
    this.mesh = new THREE.Mesh(new THREE.BufferGeometry(), this.mat);
    this.mesh.castShadow = true;
    this.radius = radius;
    this.mesh.frustumCulled = false;
  }
  update(a, b, sag = 0.12, sway = 0, via = null) {
    const mid = a.clone().lerp(b, 0.5); mid.y -= sag; mid.x += sway;
    const pts = via ? [a, ...via, b] : [a, a.clone().lerp(mid, 0.5).add(new THREE.Vector3(0, -sag * 0.3, 0)), mid, mid.clone().lerp(b, 0.5), b];
    const curve = new THREE.CatmullRomCurve3(pts, false, 'centripetal');
    this.mesh.geometry.dispose();
    this.mesh.geometry = new THREE.TubeGeometry(curve, 48, this.radius, 6, false);
    this.points = curve.getPoints(40);
  }
}
