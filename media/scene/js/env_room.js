// Environment: a cosy reading room at night. Armchair, floor lamp, bookshelf, plants, rug, side table with a mug and
// books, fairy lights, a rainy window onto a voxel city. Anchors: seat (where the character sits), window, lamp.
import * as THREE from '../vendor/three/three.module.min.js';
import { RoundedBoxGeometry } from '../vendor/three/addons/geometries/RoundedBoxGeometry.js';
import { rng, pixelTexture, hexRgb, clamp, radialSprite, deg } from './util.js';
import { makeMug, makeBook } from './props.js';

const G = new Map();
function rb(w, h, d, r = 0.02) {
  const k = `${w},${h},${d},${r}`;
  if (!G.has(k)) G.set(k, new RoundedBoxGeometry(w, h, d, 2, Math.min(r, w / 2.1, h / 2.1, d / 2.1)));
  return G.get(k);
}
function add(parent, w, h, d, m, pos, r = 0.02, shadow = true) {
  const mesh = new THREE.Mesh(rb(w, h, d, r), m);
  mesh.position.set(...pos);
  mesh.castShadow = shadow; mesh.receiveShadow = true;
  parent.add(mesh);
  return mesh;
}
const std = (o) => new THREE.MeshStandardMaterial({ roughness: 0.85, ...o });

// A rough ellipsoid of cubes (voxel foliage), one InstancedMesh with per-cube colour jitter.
export function voxelBlob(r, c, rad, cube, cols, fill = 0.6) {
  const pts = [];
  const nx = Math.ceil(rad[0] / cube), ny = Math.ceil(rad[1] / cube), nz = Math.ceil(rad[2] / cube);
  for (let i = -nx; i <= nx; i++) for (let j = -ny; j <= ny; j++) for (let k = -nz; k <= nz; k++) {
    const d = (i * cube / rad[0]) ** 2 + (j * cube / rad[1]) ** 2 + (k * cube / rad[2]) ** 2;
    if (d > 1) continue;
    const shell = d > 0.45;
    if (!shell) continue;
    if (r() > fill + (1 - d) * 0.5) continue;
    pts.push([i * cube, j * cube, k * cube]);
  }
  const m = new THREE.InstancedMesh(new RoundedBoxGeometry(cube, cube, cube, 1, cube * 0.12), new THREE.MeshStandardMaterial({ roughness: 0.8 }), pts.length);
  const m4 = new THREE.Matrix4(), col = new THREE.Color();
  pts.forEach((p, i) => {
    m4.makeTranslation(c[0] + p[0] + r.range(-0.01, 0.01), c[1] + p[1], c[2] + p[2] + r.range(-0.01, 0.01));
    m.setMatrixAt(i, m4);
    m.setColorAt(i, col.set(r.pick(cols)).multiplyScalar(r.range(0.85, 1.1) * (0.8 + 0.25 * (p[1] / rad[1] + 1) / 2)));
  });
  m.castShadow = true; m.receiveShadow = true;
  return m;
}

function planks(seed) {
  return pixelTexture(seed, 64, (x, y, r) => {
    const row = Math.floor(y / 8);
    const off = (row * 23) % 64;
    const seam = (y % 8 === 0) || ((x + off) % 32 === 0);
    const tone = [0, -10, 6, -4, 10, -7, 3, -12][row % 8];
    const grain = Math.sin((x + row * 7) * 0.7) * 3 + (r() - 0.5) * 8;
    const v = seam ? -34 : tone + grain;
    return [clamp(122 + v, 0, 255), clamp(82 + v * 0.8, 0, 255), clamp(56 + v * 0.6, 0, 255)];
  }, { repeat: [3, 3] });
}
function plaster(hex, seed, size = 32) {
  const [R, G2, B] = hexRgb(hex);
  return pixelTexture(seed, size, (x, y, r) => { const d = (r() - 0.5) * 7; return [R + d, G2 + d, B + d]; }, { repeat: [4, 3] });
}
function fabric(hex, seed) {
  const [R, G2, B] = hexRgb(hex);
  return pixelTexture(seed, 16, (x, y, r) => {
    const d = (r() - 0.5) * 10 + (((x + y) % 4 === 0) ? -6 : 0);
    return [clamp(R + d, 0, 255), clamp(G2 + d, 0, 255), clamp(B + d, 0, 255)];
  });
}
function rugTex(seed) {
  const cols = ['#C9B79A', '#7D88A0', '#DCCFB6', '#A8614A', '#3A4258'].map(hexRgb);
  return pixelTexture(seed, 32, (x, y, r) => {
    const bx = Math.min(x, 31 - x), by = Math.min(y, 31 - y), b = Math.min(bx, by);
    let c = cols[0];
    if (b === 1) c = cols[4]; else if (b === 3) c = cols[3]; else if (b > 5 && ((x + y) % 12 === 0 || (x - y + 72) % 12 === 0)) c = cols[2];
    if (b > 7 && b < 9 && (x + y) % 4 === 0) c = cols[1];
    const d = (r() - 0.5) * 12;
    return [clamp(c[0] + d, 0, 255), clamp(c[1] + d, 0, 255), clamp(c[2] + d, 0, 255)];
  });
}

export function buildRoom({ seed = 11, rain = true, wall = '#3C5363', chair = '#A4553E' } = {}) {
  const r = rng(seed);
  const root = new THREE.Group(); root.name = 'room';
  const anchors = {};
  const updaters = [];

  // floor, walls, skirting, wainscot
  const floor = new THREE.Mesh(new THREE.PlaneGeometry(8, 8), std({ map: planks(seed + 1), roughness: 0.55 }));
  floor.rotation.x = -Math.PI / 2; floor.receiveShadow = true; root.add(floor);
  const wallM = std({ map: plaster(wall, seed + 2), roughness: 0.95 });
  const back = new THREE.Mesh(new THREE.PlaneGeometry(8, 3.2), wallM); back.position.set(0, 1.6, -2.2); back.receiveShadow = true; root.add(back);
  const left = new THREE.Mesh(new THREE.PlaneGeometry(6, 3.2), wallM); left.rotation.y = Math.PI / 2; left.position.set(-2.8, 1.6, 0.8); left.receiveShadow = true; root.add(left);
  const wainM = std({ color: new THREE.Color(wall).multiplyScalar(0.72), roughness: 0.8 });
  add(root, 8, 0.9, 0.04, wainM, [0, 0.45, -2.18], 0.01, false);
  add(root, 0.04, 0.9, 6, wainM, [-2.78, 0.45, 0.8], 0.01, false);
  add(root, 8, 0.05, 0.06, std({ color: '#E8E0D2' }), [0, 0.92, -2.16], 0.01, false); // chair rail
  add(root, 8, 0.1, 0.05, std({ color: '#E8E0D2' }), [0, 0.05, -2.17], 0.01, false);

  // window with frame, sill, outside city + sky
  const win = new THREE.Group(); win.position.set(1.05, 1.55, -2.2); root.add(win);
  anchors.window = new THREE.Vector3(1.05, 1.55, -2.2);
  const frameM = std({ color: '#EDE6DA', roughness: 0.6 });
  const W = 1.35, H = 1.35;
  add(win, W + 0.14, 0.09, 0.14, frameM, [0, H / 2 + 0.03, 0.02], 0.02);
  add(win, W + 0.14, 0.09, 0.14, frameM, [0, -H / 2 - 0.03, 0.02], 0.02);
  add(win, 0.09, H + 0.1, 0.14, frameM, [W / 2 + 0.03, 0, 0.02], 0.02);
  add(win, 0.09, H + 0.1, 0.14, frameM, [-W / 2 - 0.03, 0, 0.02], 0.02);
  add(win, 0.05, H, 0.08, frameM, [0, 0, 0.02], 0.01);
  add(win, W, 0.05, 0.08, frameM, [0, 0.1, 0.02], 0.01);
  add(win, W + 0.34, 0.06, 0.3, frameM, [0, -H / 2 - 0.1, 0.12], 0.02); // sill
  // cut the wall: a dark hole is faked by drawing the outside slightly in front of the wall plane
  const outside = new THREE.Group(); outside.position.set(1.05, 1.55, -2.3); root.add(outside);
  const skyM = new THREE.ShaderMaterial({
    depthWrite: false, toneMapped: true,
    uniforms: {},
    vertexShader: 'varying vec2 vUv; void main(){ vUv=uv; gl_Position=projectionMatrix*modelViewMatrix*vec4(position,1.0); }',
    fragmentShader: `varying vec2 vUv; void main(){ vec3 top=vec3(0.015,0.025,0.07); vec3 bot=vec3(0.10,0.09,0.20);
      vec3 c=mix(bot,top,smoothstep(0.0,1.0,vUv.y)); gl_FragColor=vec4(c,1.0);
      #include <colorspace_fragment>
    }`,
  });
  const sky = new THREE.Mesh(new THREE.PlaneGeometry(14, 8), skyM); sky.position.set(0, 0.5, -6); outside.add(sky);
  // moon
  const moon = new THREE.Mesh(new THREE.CircleGeometry(0.18, 24), new THREE.MeshBasicMaterial({ color: new THREE.Color('#FFF4DA').multiplyScalar(2.2), toneMapped: false }));
  moon.position.set(-0.9, 1.9, -5.9); outside.add(moon);
  // city: blocky towers with lit windows (emissive pixel texture)
  const winTex = (s) => pixelTexture(s, 16, (x, y, rr) => {
    const lit = (x % 4 !== 0 && y % 4 !== 0 && x % 4 !== 3) && rr() > 0.8;
    const warm = rr() > 0.3;
    return lit ? (warm ? [255, 196, 120] : [150, 185, 255]) : [10, 12, 22];
  });
  for (let i = 0; i < 16; i++) {
    const bw = r.range(0.35, 0.8), bh = r.range(1.2, 4.2), bd = 0.5;
    const tex = winTex(seed + 40 + i); tex.repeat.set(Math.max(1, Math.round(bw * 2)), Math.max(1, Math.round(bh * 2)));
    const m = new THREE.MeshStandardMaterial({ color: '#0D1020', emissive: '#FFFFFF', emissiveMap: tex, emissiveIntensity: 0.6, roughness: 1 });
    const b = new THREE.Mesh(new THREE.BoxGeometry(bw, bh, bd), m);
    const z = -2.2 - r.range(0, 2.5);
    b.position.set(-3.2 + i * 0.42 + r.range(-0.1, 0.1), -1.6 + bh / 2 - (z + 2.2) * 0.2, z);
    outside.add(b);
    if (r() > 0.6) { // rooftop red light
      const l = new THREE.Mesh(new THREE.BoxGeometry(0.04, 0.04, 0.04), new THREE.MeshBasicMaterial({ color: new THREE.Color('#FF4A3A').multiplyScalar(3), toneMapped: false }));
      l.position.set(b.position.x, b.position.y + bh / 2 + 0.03, z); outside.add(l);
    }
  }
  // wall around the window opening: four wall patches in front of the outside (hide city outside the frame)
  const patchM = wallM;
  const wallPatch = (w, h, x, y) => { const m = new THREE.Mesh(new THREE.PlaneGeometry(w, h), patchM); m.position.set(x, y, -2.199); m.receiveShadow = true; root.add(m); };
  // back wall is drawn in front of the outside except through the window: use a stencil-free trick, the back plane
  // gets a hole by replacing it with 4 patches.
  root.remove(back);
  const bx0 = -4, bx1 = 4, by0 = 0, by1 = 3.2, wx0 = 1.05 - W / 2, wx1 = 1.05 + W / 2, wy0 = 1.55 - H / 2, wy1 = 1.55 + H / 2;
  wallPatch(wx0 - bx0, by1 - by0, (bx0 + wx0) / 2, 1.6);
  wallPatch(bx1 - wx1, by1 - by0, (wx1 + bx1) / 2, 1.6);
  wallPatch(W, wy0 - by0, 1.05, wy0 / 2);
  wallPatch(W, by1 - wy1, 1.05, (wy1 + by1) / 2);
  // window reveal (depth of the wall)
  const revM = std({ color: '#D9D0C2' });
  add(root, W, 0.02, 0.2, revM, [1.05, wy1, -2.3], 0.005, false);
  add(root, W, 0.02, 0.2, revM, [1.05, wy0, -2.3], 0.005, false);
  add(root, 0.02, H, 0.2, revM, [wx0, 1.55, -2.3], 0.005, false);
  add(root, 0.02, H, 0.2, revM, [wx1, 1.55, -2.3], 0.005, false);
  // rain streaks on the glass
  if (rain) {
    const rainM = new THREE.MeshBasicMaterial({ color: '#AFC4FF', transparent: true, opacity: 0.16, depthWrite: false });
    const drops = [];
    for (let i = 0; i < 22; i++) {
      const d = new THREE.Mesh(new THREE.PlaneGeometry(0.008, r.range(0.1, 0.24)), rainM);
      d.userData = { x: r.range(-W / 2, W / 2), ph: r(), sp: r.range(0.7, 1.4) };
      d.position.set(d.userData.x, 0, -2.235); win.add(d); drops.push(d);
      d.position.z = -0.04;
    }
    updaters.push((t, period) => {
      for (const d of drops) {
        const T = period > 0 ? period / Math.max(1, Math.round(period / (1.1 / d.userData.sp))) : 1.1 / d.userData.sp;
        const u = ((t / T) + d.userData.ph) % 1;
        d.position.y = H / 2 - u * H;
      }
    });
  }
  // moonlight shaft through the window (additive gradient volume)
  const shaftM = new THREE.ShaderMaterial({
    transparent: true, depthWrite: false, blending: THREE.AdditiveBlending, side: THREE.DoubleSide,
    uniforms: { color: { value: new THREE.Color('#7FA0FF') }, strength: { value: 0.07 } },
    vertexShader: 'varying vec2 vUv; void main(){ vUv=uv; gl_Position=projectionMatrix*modelViewMatrix*vec4(position,1.0); }',
    fragmentShader: `uniform vec3 color; uniform float strength; varying vec2 vUv;
      void main(){ float a = smoothstep(0.0,0.25,vUv.x)*smoothstep(1.0,0.75,vUv.x)*pow(vUv.y,1.4); gl_FragColor=vec4(color*a*strength,1.0); }`,
  });
  for (let i = 0; i < 3; i++) {
    const s = new THREE.Mesh(new THREE.PlaneGeometry(0.55 + i * 0.25, 2.6), shaftM);
    s.position.set(1.05 - 0.25 + i * 0.12, 1.0, -1.35);
    s.rotation.set(deg(-38), deg(-12 + i * 9), 0);
    root.add(s);
  }

  // rug
  const rug = new THREE.Mesh(new RoundedBoxGeometry(2.4, 0.02, 1.7, 2, 0.008), std({ map: rugTex(seed + 3), roughness: 1 }));
  rug.position.set(-0.15, 0.01, -0.85); rug.receiveShadow = true; root.add(rug);

  // armchair (character sits at anchors.seat)
  const chairG = new THREE.Group(); chairG.position.set(-0.2, 0, -1.35); root.add(chairG);
  const fab = std({ map: fabric(chair, seed + 4), roughness: 0.98 });
  const fabD = std({ map: fabric(new THREE.Color(chair).multiplyScalar(0.8).getHexString().replace(/^/, '#'), seed + 5), roughness: 0.98 });
  const wood = std({ color: '#5A3B26', roughness: 0.6 });
  add(chairG, 1.05, 0.26, 0.86, fabD, [0, 0.25, 0], 0.06);
  add(chairG, 0.78, 0.13, 0.72, fab, [0, 0.43, 0.05], 0.06);
  add(chairG, 1.05, 0.82, 0.22, fabD, [0, 0.72, -0.34], 0.08);
  add(chairG, 0.74, 0.5, 0.1, fab, [0, 0.72, -0.2], 0.05).rotation.x = deg(-8); // back cushion
  add(chairG, 0.16, 0.42, 0.86, fabD, [0.46, 0.55, 0], 0.07);
  add(chairG, 0.16, 0.42, 0.86, fabD, [-0.46, 0.55, 0], 0.07);
  for (const [x, z] of [[-0.42, 0.34], [0.42, 0.34], [-0.42, -0.34], [0.42, -0.34]]) add(chairG, 0.07, 0.12, 0.07, wood, [x, 0.06, z], 0.015);
  // a knitted throw over the arm
  add(chairG, 0.2, 0.3, 0.5, std({ map: fabric('#E5D7B8', seed + 6) }), [0.5, 0.62, 0.05], 0.06).rotation.z = deg(-8);
  anchors.seat = new THREE.Vector3(-0.2, 0.0, -1.27);
  anchors.seatHipsY = 0.5 - 0.62 + 0.0; // hips drop when sitting (seat top 0.5, hips rest 0.62)

  // floor lamp (left of chair)
  const lamp = new THREE.Group(); lamp.position.set(-1.05, 0, -1.55); root.add(lamp);
  const brass = std({ color: '#B8894A', metalness: 0.7, roughness: 0.35 });
  add(lamp, 0.34, 0.04, 0.34, brass, [0, 0.02, 0], 0.015);
  add(lamp, 0.035, 1.5, 0.035, brass, [0, 0.77, 0], 0.01);
  add(lamp, 0.035, 0.035, 0.32, brass, [0, 1.52, 0.14], 0.01);
  const shadeM = new THREE.MeshStandardMaterial({ color: '#F4E3C2', emissive: '#FFC27A', emissiveIntensity: 0.22, roughness: 0.9, side: THREE.DoubleSide });
  const shade = new THREE.Mesh(new THREE.CylinderGeometry(0.16, 0.25, 0.26, 8, 1, true), shadeM);
  shade.position.set(0, 1.42, 0.3); lamp.add(shade);
  const bulb = new THREE.Mesh(new THREE.BoxGeometry(0.07, 0.07, 0.07), new THREE.MeshBasicMaterial({ color: new THREE.Color('#FFD9A0').multiplyScalar(1.6), toneMapped: false }));
  bulb.position.set(0, 1.36, 0.3); lamp.add(bulb);
  anchors.lamp = new THREE.Vector3(-1.05, 1.36, -1.25);

  // side table (right of chair) with mug, book stack, small plant
  const table = new THREE.Group(); table.position.set(0.62, 0, -1.2); root.add(table);
  add(table, 0.46, 0.04, 0.46, wood, [0, 0.55, 0], 0.015);
  add(table, 0.05, 0.53, 0.05, wood, [0, 0.27, 0], 0.01);
  add(table, 0.3, 0.03, 0.3, wood, [0, 0.015, 0], 0.01);
  const mug = makeMug({ color: '#2F6BFF', seed: seed + 7 }); mug.position.set(0.1, 0.57, 0.1); table.add(mug);
  updaters.push((t, period) => mug.userData.update(t, period));
  const bookCols = ['#7A2E2E', '#2C3550', '#E1B862', '#3E6B55'];
  for (let i = 0; i < 4; i++) {
    const b = makeBook({ w: 0.2 - i * 0.015, h: 0.27 - i * 0.02, d: 0.04, cover: bookCols[i], seed: seed + 20 + i });
    b.rotation.set(-Math.PI / 2, 0, r.range(-0.3, 0.3));
    b.position.set(-0.08, 0.59 + i * 0.041, -0.05); table.add(b);
  }
  // little succulent
  const potM = std({ color: '#C8714E' }), leafM = std({ color: '#5E8E5A', roughness: 0.7 });
  add(table, 0.09, 0.08, 0.09, potM, [0.12, 0.61, -0.13], 0.015);
  for (let i = 0; i < 5; i++) add(table, 0.04, 0.07, 0.03, leafM, [0.12 + Math.cos(i * 1.3) * 0.025, 0.68, -0.13 + Math.sin(i * 1.3) * 0.025], 0.012).rotation.set(r.range(-0.4, 0.4), i, r.range(-0.4, 0.4));

  // bookshelf on the back-left
  const shelf = new THREE.Group(); shelf.position.set(-2.05, 0, -1.95); root.add(shelf);
  const shelfM = std({ color: '#6B4A33', roughness: 0.7 });
  add(shelf, 1.2, 2.1, 0.06, shelfM, [0, 1.05, -0.15], 0.01);
  add(shelf, 0.06, 2.1, 0.36, shelfM, [-0.6, 1.05, 0], 0.01);
  add(shelf, 0.06, 2.1, 0.36, shelfM, [0.6, 1.05, 0], 0.01);
  const shelfYs = [0.05, 0.5, 0.95, 1.4, 1.85, 2.1];
  shelfYs.forEach((y) => add(shelf, 1.2, 0.05, 0.36, shelfM, [0, y, 0], 0.01));
  const spineCols = ['#7A2E2E', '#2C3550', '#E1B862', '#3E6B55', '#B5543C', '#E9DFC9', '#2F6BFF', '#5B4A7A', '#C9A27A', '#1F3A2E'];
  const bookGeo = new THREE.BoxGeometry(1, 1, 1);
  const bookMat = new THREE.MeshStandardMaterial({ roughness: 0.75 });
  const inst = new THREE.InstancedMesh(bookGeo, bookMat, 140);
  inst.castShadow = true; inst.receiveShadow = true;
  let n = 0;
  const m4 = new THREE.Matrix4(), q = new THREE.Quaternion(), sc = new THREE.Vector3(), ps = new THREE.Vector3();
  for (let s = 0; s < 4; s++) {
    let x = -0.55;
    while (x < 0.52 && n < 140) {
      const w = r.range(0.03, 0.06), h = r.range(0.26, 0.38), d = r.range(0.22, 0.28);
      if (r() < 0.07) { x += r.range(0.06, 0.14); continue; }
      const lean = (r() < 0.06) ? deg(r.range(8, 16)) : 0;
      ps.set(x + w / 2, shelfYs[s] + 0.025 + h / 2, 0.02); sc.set(w, h, d); q.setFromEuler(new THREE.Euler(0, 0, -lean));
      m4.compose(ps, q, sc); inst.setMatrixAt(n, m4);
      inst.setColorAt(n, new THREE.Color(r.pick(spineCols)).multiplyScalar(r.range(0.75, 1.05)));
      x += w + 0.004; n++;
    }
  }
  inst.count = n; shelf.add(inst);
  // top shelf: a small plant and a framed photo
  add(shelf, 0.12, 0.11, 0.12, potM, [0.3, 2.18, 0], 0.02);
  for (let i = 0; i < 7; i++) add(shelf, 0.05, 0.16, 0.03, leafM, [0.3 + Math.cos(i) * 0.05, 2.3 + r.range(-0.02, 0.06), Math.sin(i) * 0.05], 0.015).rotation.set(r.range(-0.6, 0.6), i, r.range(-0.6, 0.6));

  // big potted plant by the window
  const plant = new THREE.Group(); plant.position.set(2.05, 0, -1.7); root.add(plant);
  add(plant, 0.42, 0.42, 0.42, std({ color: '#D8CDBA', roughness: 0.6 }), [0, 0.21, 0], 0.04);
  add(plant, 0.06, 0.6, 0.06, std({ color: '#5A4632' }), [0, 0.7, 0], 0.01);
  const leafG = std({ map: pixelTexture(seed + 8, 8, (x, y, rr) => { const d = (rr() - 0.5) * 20 + (x === 4 ? 25 : 0); return [60 + d, 118 + d, 72 + d]; }), roughness: 0.6 });
  plant.add(voxelBlob(r, [0, 1.15, 0], [0.42, 0.55, 0.42], 0.11, ['#3F7A4A', '#4E8F55', '#5FA35F', '#356842'], 0.62));
  plant.add(voxelBlob(r, [0.18, 0.9, 0.12], [0.22, 0.22, 0.22], 0.1, ['#4E8F55', '#5FA35F'], 0.7));

  // wall art: a framed print with a blue/ember composition
  const art = new THREE.Group(); art.position.set(-0.85, 2.05, -2.17); root.add(art);
  add(art, 0.62, 0.8, 0.04, std({ color: '#2A2420', roughness: 0.5 }), [0, 0, 0], 0.01);
  const artTex = pixelTexture(seed + 9, 16, (x, y) => {
    const c = (y > 10) ? [47, 107, 255] : (Math.hypot(x - 10, y - 6) < 3.2 ? [255, 180, 92] : [233, 223, 201]);
    return y > 10 && (x + y) % 5 === 0 ? [29, 75, 189] : c;
  });
  const print = new THREE.Mesh(new THREE.PlaneGeometry(0.5, 0.68), std({ map: artTex, roughness: 0.9 }));
  print.position.z = 0.022; art.add(print);

  // fairy lights along the top of the back wall: warm emissive beads on a sagging line
  const beadM = new THREE.MeshBasicMaterial({ color: new THREE.Color('#FFC67A').multiplyScalar(5), toneMapped: false });
  const beadG = new THREE.BoxGeometry(0.03, 0.03, 0.03);
  const wireM = std({ color: '#222' });
  const pts = [];
  for (let i = 0; i <= 60; i++) {
    const u = i / 60, x = -2.6 + u * 5.0;
    const sagU = (u * 4) % 1;
    const y = 2.6 - 0.16 * Math.sin(Math.PI * sagU);
    pts.push(new THREE.Vector3(x, y, -2.14));
    if (i % 2 === 0) { const b = new THREE.Mesh(beadG, beadM); b.position.set(x, y - 0.025, -2.13); root.add(b); }
  }
  root.add(new THREE.Mesh(new THREE.TubeGeometry(new THREE.CatmullRomCurve3(pts), 120, 0.004, 4), wireM));

  // dust motes floating in the lamp light (periodic drift)
  const mote = radialSprite(32, 'rgba(255,225,180,1)', 'rgba(255,225,180,0)');
  const motes = [];
  for (let i = 0; i < 70; i++) {
    const s = new THREE.Sprite(new THREE.SpriteMaterial({ map: mote, transparent: true, depthWrite: false, blending: THREE.AdditiveBlending, opacity: r.range(0.25, 0.7) }));
    s.userData = { p: new THREE.Vector3(r.range(-1.5, 0.7), r.range(0.4, 2.2), r.range(-1.9, -0.2)), a: r.range(0.03, 0.09), ph: r() * 6.28, f: r.range(0.04, 0.12) };
    s.scale.setScalar(r.range(0.008, 0.016));
    root.add(s); motes.push(s);
  }
  updaters.push((t, period) => {
    for (const s of motes) {
      let f = s.userData.f; if (period > 0) f = Math.max(1, Math.round(f * period)) / period;
      const ph = s.userData.ph, a = s.userData.a;
      s.position.copy(s.userData.p).add(new THREE.Vector3(Math.sin(6.283 * f * t + ph) * a, Math.sin(6.283 * f * t * 2 + ph * 1.7) * a * 0.6, Math.cos(6.283 * f * t + ph * 0.6) * a));
    }
  });

  // lights (presets set intensities)
  const lights = {};
  lights.hemi = new THREE.HemisphereLight('#3B4D7A', '#1A1410', 0.28);
  root.add(lights.hemi);
  lights.lamp = new THREE.PointLight('#FFB566', 3.4, 7, 1.6);
  lights.lamp.position.copy(anchors.lamp);
  lights.lamp.castShadow = true; lights.lamp.shadow.mapSize.set(1024, 1024); lights.lamp.shadow.radius = 6; lights.lamp.shadow.bias = -0.002;
  lights.lamp.shadow.camera.near = 0.05;
  root.add(lights.lamp);
  lights.moon = new THREE.DirectionalLight('#7C9BFF', 0.9);
  lights.moon.position.set(2.4, 3.6, -5.5); lights.moon.target.position.set(0.3, 0, -0.6);
  lights.moon.castShadow = true; lights.moon.shadow.mapSize.set(1024, 1024); lights.moon.shadow.radius = 4; lights.moon.shadow.bias = -0.0015;
  Object.assign(lights.moon.shadow.camera, { left: -3, right: 3, top: 3, bottom: -3, near: 0.5, far: 14 });
  root.add(lights.moon); root.add(lights.moon.target);
  lights.fill = new THREE.PointLight('#FF9A55', 1.2, 6, 2); lights.fill.position.set(0, 2.5, -2.0); root.add(lights.fill);
  lights.rim = new THREE.SpotLight('#6F92FF', 6, 6, deg(35), 0.6, 1.5); lights.rim.position.set(1.3, 2.2, -2.0); lights.rim.target.position.set(-0.2, 0.9, -1.3);
  root.add(lights.rim); root.add(lights.rim.target);

  return {
    name: 'room', root, anchors, lights,
    background: new THREE.Color('#07080D'),
    fog: new THREE.Fog('#0A0B12', 6, 14),
    exposure: 1.0,
    look: { dof: { aperture: 0.006, maxblur: 0.012 }, bloom: { strength: 0.55, radius: 0.65, threshold: 0.85 },
      grade: { gain: [1.03, 0.99, 0.95], lift: [0.01, 0.01, 0.025], saturation: 1.05, contrast: 1.05, vignette: 0.42, grain: 0.03 } },
    update(t, period) { updaters.forEach((u) => u(t, period)); },
  };
}
