// Environment: a voxel park at golden hour (also 'day' and 'dusk'), or a city street ('setting': 'street').
// A straight path runs along +x; the character walks it while the camera tracks alongside through foreground foliage.
import * as THREE from '../vendor/three/three.module.min.js';
import { RoundedBoxGeometry } from '../vendor/three/addons/geometries/RoundedBoxGeometry.js';
import { rng, pixelTexture, hexRgb, clamp, radialSprite, deg } from './util.js';
import { voxelBlob } from './env_room.js';
import { Character } from './character.js';

const std = (o) => new THREE.MeshStandardMaterial({ roughness: 0.85, ...o });
const G = new Map();
function rb(w, h, d, r = 0.02) {
  const k = `${w},${h},${d},${r}`;
  if (!G.has(k)) G.set(k, new RoundedBoxGeometry(w, h, d, 2, Math.min(r, w / 2.1, h / 2.1, d / 2.1)));
  return G.get(k);
}
function add(parent, w, h, d, m, pos, r = 0.02) {
  const mesh = new THREE.Mesh(rb(w, h, d, r), m);
  mesh.position.set(...pos); mesh.castShadow = true; mesh.receiveShadow = true; parent.add(mesh); return mesh;
}

const TIMES = {
  golden: { sky: ['#F7B26A', '#F6D3A0', '#7FA6D8'], sun: '#FFC27A', sunI: 4.4, sunDir: [-0.75, 0.3, -0.45], hemi: ['#B9CCEB', '#6A5232', 0.55], fog: '#F2C694', fogNear: 18, fogFar: 70, lamps: 0.15, exposure: 1.0, sunDisc: 1,
    grade: { gain: [1.08, 1.0, 0.88], lift: [0.03, 0.015, 0.0], gamma: 0.97, saturation: 1.1, contrast: 1.08, vignette: 0.36 } },
  day: { sky: ['#CFE4F7', '#A9CBEF', '#5D8FD6'], sun: '#FFF4E2', sunI: 3.0, sunDir: [-0.4, 0.75, -0.35], hemi: ['#CFE2FF', '#5E6B3A', 1.1], fog: '#C9DDF0', fogNear: 25, fogFar: 90, lamps: 0, exposure: 1.0, sunDisc: 0,
    grade: { gain: [0.98, 1.0, 1.03], lift: [0, 0.005, 0.012], saturation: 1.05, contrast: 1.02, vignette: 0.25 } },
  dusk: { sky: ['#E8879A', '#B98AC2', '#2E3A78'], sun: '#FF9D7A', sunI: 1.3, sunDir: [-0.85, 0.08, -0.4], hemi: ['#8E8FD0', '#3A2A3A', 0.7], fog: '#B387A8', fogNear: 14, fogFar: 60, lamps: 1.6, exposure: 1.05, sunDisc: 0.7,
    grade: { gain: [1.02, 0.97, 1.04], lift: [0.02, 0.0, 0.03], saturation: 1.05, contrast: 1.05, vignette: 0.38 } },
};

export function buildPark({ seed = 21, setting = 'park', time = 'golden' } = {}) {
  const r = rng(seed);
  const root = new THREE.Group(); root.name = 'park';
  const updaters = [];
  const anchors = { path_start: new THREE.Vector3(-4, 0, 0), bench: new THREE.Vector3(6, 0, -1.3) };

  // ground: grass with voxel texture
  const grass = pixelTexture(seed + 1, 32, (x, y, rr) => {
    const v = rr();
    const base = v > 0.92 ? [118, 160, 70] : v > 0.6 ? [96, 140, 60] : [86, 128, 56];
    const d = (rr() - 0.5) * 10;
    return [base[0] + d, base[1] + d, base[2] + d * 0.6];
  }, { repeat: [40, 40] });
  const ground = new THREE.Mesh(new THREE.PlaneGeometry(160, 160), std({ map: grass, roughness: 0.95 }));
  ground.rotation.x = -Math.PI / 2; ground.receiveShadow = true; root.add(ground);
  // path along x
  const pathTex = pixelTexture(seed + 2, 32, (x, y, rr) => {
    const stone = ((Math.floor(x / 8) + Math.floor(y / 8)) % 2) ? 6 : -6;
    const seam = (x % 8 === 0 || y % 8 === 0) ? -26 : 0;
    const d = (rr() - 0.5) * 12 + stone + seam;
    return setting === 'street' ? [150 + d, 150 + d, 155 + d] : [196 + d, 172 + d, 134 + d];
  }, { repeat: [80, 1] });
  const path = new THREE.Mesh(new THREE.BoxGeometry(160, 0.04, 1.8), std({ map: pathTex, roughness: 0.9 }));
  path.position.set(0, 0.02, 0); path.receiveShadow = true; root.add(path);
  const edgeM = std({ color: setting === 'street' ? '#8A8D96' : '#9C8F78' });
  for (const z of [-0.95, 0.95]) { const e = new THREE.Mesh(new THREE.BoxGeometry(160, 0.08, 0.12), edgeM); e.position.set(0, 0.04, z); e.receiveShadow = true; root.add(e); }

  // sky dome with sun disc
  const skyU = { top: { value: new THREE.Color() }, mid: { value: new THREE.Color() }, hor: { value: new THREE.Color() }, sunDir: { value: new THREE.Vector3() }, sunCol: { value: new THREE.Color() }, disc: { value: 1 } };
  const sky = new THREE.Mesh(new THREE.SphereGeometry(120, 32, 16), new THREE.ShaderMaterial({
    side: THREE.BackSide, depthWrite: false, fog: false, uniforms: skyU,
    vertexShader: 'varying vec3 vD; void main(){ vD = normalize(position); gl_Position = projectionMatrix*modelViewMatrix*vec4(position,1.0); }',
    fragmentShader: `uniform vec3 top; uniform vec3 mid; uniform vec3 hor; uniform vec3 sunDir; uniform vec3 sunCol; uniform float disc; varying vec3 vD;
      void main(){ float h = clamp(vD.y, 0.0, 1.0);
        vec3 c = mix(hor, mid, smoothstep(0.0, 0.18, h)); c = mix(c, top, smoothstep(0.15, 0.7, h));
        float s = max(dot(normalize(vD), normalize(sunDir)), 0.0);
        c += sunCol * (pow(s, 900.0) * 6.0 * disc + pow(s, 24.0) * 0.35 * disc + pow(s, 4.0) * 0.12);
        gl_FragColor = vec4(c, 1.0);
        #include <colorspace_fragment>
      }`,
  }));
  root.add(sky);

  // trees: trunk + voxel canopy, rows behind the path, a few in front for parallax
  const trunkM = std({ map: pixelTexture(seed + 3, 8, (x, y, rr) => { const d = (rr() - 0.5) * 20 + (x % 3 === 0 ? -12 : 0); return [104 + d, 74 + d, 50 + d]; }) });
  const greens = [['#4E8F55', '#5FA35F', '#3F7A4A', '#6DB06A'], ['#7FA34A', '#93B552', '#6A8F3E'], ['#E0A23A', '#D98B2B', '#EDB64F'], ['#C9692E', '#B4572A', '#DA7B3A']];
  const tree = (x, z, s = 1) => {
    const g = new THREE.Group(); g.position.set(x, 0, z); root.add(g);
    const h = r.range(1.6, 2.4) * s;
    add(g, 0.26 * s, h, 0.26 * s, trunkM, [0, h / 2, 0], 0.03);
    const pal = r() < 0.65 ? greens[r.int(0, 1)] : greens[r.int(2, 3)];
    g.add(voxelBlob(r, [0, h + 0.7 * s, 0], [1.1 * s, 1.0 * s, 1.1 * s], 0.24 * s, pal, 0.55));
    if (r() < 0.6) g.add(voxelBlob(r, [r.range(-0.5, 0.5) * s, h + 1.4 * s, r.range(-0.4, 0.4) * s], [0.7 * s, 0.6 * s, 0.7 * s], 0.22 * s, pal, 0.6));
    return g;
  };
  const shrubM = ['#4E8F55', '#5FA35F', '#3F7A4A'];
  if (setting === 'street') {
    // facades along the back with shop windows; a few street trees in planters
    const wallCols = ['#C9A27A', '#B9805E', '#D8C3A0', '#8E9AAE', '#A7746A'];
    for (let x = -40; x < 40; x += r.range(3.5, 5.5)) {
      const w = r.range(3.2, 5), h = r.range(5, 11);
      const m = std({ color: r.pick(wallCols), roughness: 0.9 });
      add(root, w, h, 3, m, [x + w / 2, h / 2, -5.5], 0.05);
      const winM = std({ color: '#2A3346', emissive: '#FFD9A0', emissiveIntensity: 0.0, roughness: 0.2, metalness: 0.3 });
      for (let fy = 1.6; fy < h - 1; fy += 2.2) for (let fx = 0.7; fx < w - 0.6; fx += 1.4) add(root, 0.8, 1.2, 0.08, winM, [x + fx + 0.2, fy, -3.98], 0.02);
    }
    for (let x = -30; x < 30; x += 7) { add(root, 0.9, 0.5, 0.9, std({ color: '#7C7F88' }), [x, 0.25, -1.9], 0.04); tree(x, -1.9, 0.7); }
  } else {
    for (let x = -36; x < 36; x += r.range(2.6, 4.2)) tree(x + r.range(-0.6, 0.6), r.range(-3.2, -5.5), r.range(0.9, 1.25));
    for (let x = -36; x < 36; x += r.range(5, 8)) tree(x, r.range(-9, -13), r.range(1.2, 1.6));
    for (let x = -36; x < 36; x += r.range(7, 11)) tree(x, r.range(5.0, 6.2), r.range(0.9, 1.1)); // foreground row, clear of tracking cameras
    for (let x = -36; x < 36; x += r.range(1.8, 3.2)) root.add(voxelBlob(r, [x, 0.3, r.range(1.6, 2.6)], [0.5, 0.35, 0.4], 0.17, shrubM, 0.7));
    for (let x = -36; x < 36; x += r.range(2, 3.5)) root.add(voxelBlob(r, [x, 0.25, r.range(-1.7, -2.3)], [0.45, 0.3, 0.35], 0.15, shrubM, 0.7));
    // flowers: tiny coloured cubes in the grass
    const fl = new THREE.InstancedMesh(new THREE.BoxGeometry(0.05, 0.05, 0.05), std({ roughness: 0.6 }), 400);
    const m4 = new THREE.Matrix4(), col = new THREE.Color();
    for (let i = 0; i < 400; i++) {
      const z = r() < 0.5 ? r.range(1.1, 3) : r.range(-1.1, -3);
      m4.makeTranslation(r.range(-36, 36), 0.04, z); fl.setMatrixAt(i, m4);
      fl.setColorAt(i, col.set(r.pick(['#F4E3C2', '#FFB45C', '#E97F7F', '#F2D16B', '#B9A6E8'])));
    }
    root.add(fl);
    // distant skyline in the haze
    for (let i = 0; i < 26; i++) {
      const b = new THREE.Mesh(new THREE.BoxGeometry(r.range(2, 5), r.range(4, 14), 3), std({ color: '#7E8AA8', roughness: 1 }));
      b.position.set(-60 + i * 5 + r.range(-1, 1), b.geometry.parameters.height / 2, -40 - r.range(0, 8)); root.add(b);
    }
  }
  // benches and lamp posts along the path
  const benchWood = std({ color: '#8A5A36', roughness: 0.7 }), iron = std({ color: '#262A33', metalness: 0.5, roughness: 0.5 });
  const lampHeads = [];
  for (let x = -30; x <= 30; x += 12) {
    const b = new THREE.Group(); b.position.set(x + 6, 0, -1.3); root.add(b);
    add(b, 1.6, 0.06, 0.42, benchWood, [0, 0.45, 0], 0.015);
    add(b, 1.6, 0.3, 0.06, benchWood, [0, 0.72, -0.2], 0.015);
    for (const sx of [-0.7, 0.7]) { add(b, 0.06, 0.45, 0.4, iron, [sx, 0.22, 0], 0.01); add(b, 0.06, 0.4, 0.06, iron, [sx, 0.62, -0.2], 0.01); }
    const l = new THREE.Group(); l.position.set(x, 0, -1.25); root.add(l);
    add(l, 0.12, 3.0, 0.12, iron, [0, 1.5, 0], 0.02);
    add(l, 0.3, 0.06, 0.3, iron, [0, 3.05, 0], 0.01);
    const headM = new THREE.MeshStandardMaterial({ color: '#FFE2B0', emissive: '#FFC27A', emissiveIntensity: 0.2, roughness: 0.4 });
    add(l, 0.24, 0.32, 0.24, headM, [0, 2.86, 0], 0.03);
    lampHeads.push(headM);
  }
  // god rays: soft additive slabs aligned with the sun
  const rayM = new THREE.ShaderMaterial({
    transparent: true, depthWrite: false, blending: THREE.AdditiveBlending, side: THREE.DoubleSide, fog: false,
    uniforms: { color: { value: new THREE.Color('#FFD7A0') }, strength: { value: 0.05 } },
    vertexShader: 'varying vec2 vUv; void main(){ vUv=uv; gl_Position=projectionMatrix*modelViewMatrix*vec4(position,1.0); }',
    fragmentShader: `uniform vec3 color; uniform float strength; varying vec2 vUv;
      void main(){ float a = smoothstep(0.0,0.35,vUv.x)*smoothstep(1.0,0.65,vUv.x)*smoothstep(0.0,0.5,vUv.y)*smoothstep(1.0,0.7,vUv.y); gl_FragColor=vec4(color*a*strength,1.0); }`,
  });
  const rays = new THREE.Group(); root.add(rays);
  for (let i = 0; i < 14; i++) {
    const p = new THREE.Mesh(new THREE.PlaneGeometry(r.range(0.6, 1.8), 14), rayM);
    p.position.set(-30 + i * 4.6 + r.range(-1, 1), 3, r.range(-3.5, -1.0));
    p.rotation.set(0, deg(55), deg(62));
    rays.add(p);
  }
  // falling leaves (periodic)
  const leafG = new THREE.PlaneGeometry(0.07, 0.05);
  const leafMs = ['#E0A23A', '#C9692E', '#EDB64F', '#9DB552'].map((c) => new THREE.MeshStandardMaterial({ color: c, side: THREE.DoubleSide, roughness: 0.7 }));
  const leaves = [];
  for (let i = 0; i < 90; i++) {
    const l = new THREE.Mesh(leafG, r.pick(leafMs)); l.castShadow = false;
    l.userData = { x: r.range(-14, 22), z: r.range(-3.8, 1.0), h: r.range(3, 5), ph: r(), T: r.range(5, 9), sw: r.range(0.2, 0.6), sp: r.range(1, 3) };
    root.add(l); leaves.push(l);
  }
  updaters.push((t, period) => {
    for (const l of leaves) {
      const u = l.userData;
      const T = period > 0 ? period / Math.max(1, Math.round(period / u.T)) : u.T;
      const k = ((t / T) + u.ph) % 1;
      l.position.set(u.x + Math.sin(k * 6.283 * 2 + u.ph * 9) * u.sw + k * 1.5, u.h * (1 - k), u.z + Math.cos(k * 6.283 + u.ph) * u.sw * 0.5);
      l.rotation.set(k * 6.283 * u.sp, k * 6.283 * 0.7, Math.sin(k * 12) * 0.8);
      l.visible = l.position.y > 0.02;
    }
  });
  // birds: small blocky Vs crossing the sky, flapping
  const birdM = std({ color: '#3A3530' });
  const birds = [];
  for (let i = 0; i < 6; i++) {
    const b = new THREE.Group();
    const wl = add(b, 0.22, 0.02, 0.07, birdM, [0.1, 0, 0], 0.005), wr = add(b, 0.22, 0.02, 0.07, birdM, [-0.1, 0, 0], 0.005);
    add(b, 0.06, 0.05, 0.14, birdM, [0, 0, 0], 0.01);
    b.userData = { wl, wr, y: r.range(6, 10), z: r.range(-14, -8), ph: r(), T: r.range(14, 20) };
    root.add(b); birds.push(b);
  }
  updaters.push((t, period) => {
    for (const b of birds) {
      const u = b.userData;
      const T = period > 0 ? period / Math.max(1, Math.round(period / u.T)) : u.T;
      const k = ((t / T) + u.ph) % 1;
      b.position.set(-25 + k * 50, u.y + Math.sin(k * 20) * 0.2, u.z);
      b.rotation.y = deg(90);
      const f = Math.sin(t * 6.283 * 2.5 + u.ph * 10) * 0.6;
      u.wl.rotation.z = f; u.wr.rotation.z = -f;
      u.wl.position.y = u.wr.position.y = 0;
    }
  });

  // depth: a second path behind the first row of trees with a few people strolling (same rig, other palettes)
  if (setting !== 'street') {
    const back = new THREE.Mesh(new THREE.BoxGeometry(160, 0.03, 1.4), std({ map: pathTex, roughness: 0.9 }));
    back.position.set(0, 0.015, -7.6); back.receiveShadow = true; root.add(back);
  }
  const walkers = [];
  const looks = [
    { hoodie: '#7D8FB3', hoodieDark: '#687A9E', hoodieTrim: '#5E6F90', pants: '#3B3B44', skin: '#C98E62', hair: '#1E1A18' },
    { hoodie: '#B5543C', hoodieDark: '#9C4632', hoodieTrim: '#8A3E2C', pants: '#2C3550', skin: '#F2CDA8', hair: '#8A5A2E' },
    { hoodie: '#5E8E5A', hoodieDark: '#4F7A4B', hoodieTrim: '#456B42', pants: '#4A4036', skin: '#8D5A3B', hair: '#151313' },
    { hoodie: '#E1B862', hoodieDark: '#C9A04E', hoodieTrim: '#B08A40', pants: '#2F3A4A', skin: '#E8B48C', hair: '#3B2A24' },
  ];
  [[-6, -7.4, -1], [3, -7.8, 1], [11, -7.5, -1], [19, -7.7, 1]].forEach(([x, z, dir], i) => {
    if (setting === 'street') z = -3.2;
    const c = new Character({ seed: 40 + i, palette: looks[i], headphones: i === 1 ? 'ears' : 'none' });
    c.group.scale.setScalar(0.96 + 0.06 * (i % 2));
    root.add(c.group);
    walkers.push({ c, x, z, dir, cad: 0.8 + 0.06 * i });
  });
  updaters.push((t) => {
    for (const w of walkers) {
      const v = w.c.strideSpeed({ amount: 0.9, cadence: w.cad });
      w.c.update({ root: { pos: [w.x + w.dir * v * t, 0, w.z], rotY: w.dir * 90 }, joints: {}, ground: 0,
        walk: { amount: 0.9, cadence: w.cad, arms: 1, phase: w.x }, expr: undefined, life: 0.6 }, t + w.x);
    }
  });

  const lights = {};
  lights.hemi = new THREE.HemisphereLight('#BFD3F2', '#6A5232', 0.9); root.add(lights.hemi);
  lights.sun = new THREE.DirectionalLight('#FFC27A', 3.2);
  lights.sun.castShadow = true; lights.sun.shadow.mapSize.set(2048, 2048); lights.sun.shadow.radius = 5; lights.sun.shadow.bias = -0.0008; lights.sun.shadow.normalBias = 0.02;
  Object.assign(lights.sun.shadow.camera, { left: -9, right: 9, top: 9, bottom: -9, near: 1, far: 60 });
  root.add(lights.sun); root.add(lights.sun.target);
  lights.bounce = new THREE.DirectionalLight('#FFE3C0', 0.5); root.add(lights.bounce);

  const env = {
    name: 'park', root, anchors, lights,
    background: new THREE.Color('#F6D3A0'),
    fog: new THREE.Fog('#F2C694', 18, 70),
    exposure: 1.0, envIntensity: 0.5,
    look: { dof: { aperture: 0.004, maxblur: 0.01 }, bloom: { strength: 0.45, radius: 0.7, threshold: 0.92 }, grade: TIMES[time].grade },
    time,
    setTime(name) {
      const T = TIMES[name] || TIMES.golden;
      if (env._time === name) return; env._time = name;
      skyU.hor.value.set(T.sky[0]); skyU.mid.value.set(T.sky[1]); skyU.top.value.set(T.sky[2]);
      skyU.sunCol.value.set(T.sun); skyU.disc.value = T.sunDisc;
      const d = new THREE.Vector3(...T.sunDir).normalize(); skyU.sunDir.value.copy(d);
      env.sunDir = d;
      lights.sun.color.set(T.sun); lights.sun.intensity = T.sunI;
      lights.sun.userData.base = { i: T.sunI, c: new THREE.Color(T.sun) };
      lights.hemi.color.set(T.hemi[0]); lights.hemi.groundColor.set(T.hemi[1]); lights.hemi.intensity = T.hemi[2];
      lights.hemi.userData.base = { i: T.hemi[2], c: new THREE.Color(T.hemi[0]) };
      env.fog.color.set(T.fog); env.fog.near = T.fogNear; env.fog.far = T.fogFar;
      env.background = new THREE.Color(T.sky[1]);
      env.exposure = T.exposure;
      env.look.grade = T.grade;
      lampHeads.forEach((m) => { m.emissiveIntensity = 0.2 + T.lamps * 2; });
      rays.visible = name !== 'day';
      rayM.uniforms.color.value.set(T.sun);
    },
    // keep the sun's shadow frustum around the subject
    focus(p) {
      const d = env.sunDir || new THREE.Vector3(-0.75, 0.22, -0.5).normalize();
      lights.sun.target.position.copy(p);
      lights.sun.position.copy(p).addScaledVector(d, 30);
      lights.bounce.position.copy(p).add(new THREE.Vector3(2, 1, 4));
      lights.bounce.target = lights.sun.target;
    },
    update(t, period) { updaters.forEach((u) => u(t, period)); },
  };
  env.setTime(time);
  env.focus(new THREE.Vector3());
  return env;
}
