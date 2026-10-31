// Voxel props for the spot illustrations (and any shot): each builder returns a THREE.Group in metres, origin at
// the grip point. Colours come from the brand palette passed in (blue, ember) plus neutral paper/ink tones.
import * as THREE from '../vendor/three/three.module.min.js';
import { RoundedBoxGeometry } from '../vendor/three/addons/geometries/RoundedBoxGeometry.js';
import { deg } from './util.js';
import { makeBook, makeOpenBook, makeMug } from './props.js';

const geo = new Map();
function rb(w, h, d, r) {
  const k = `${w},${h},${d},${r}`;
  if (!geo.has(k)) geo.set(k, new RoundedBoxGeometry(w, h, d, 2, Math.min(r ?? 0.01, w / 2.1, h / 2.1, d / 2.1)));
  return geo.get(k);
}
const M = new Map();
function mat(color, o = {}) {
  const k = color + JSON.stringify(o);
  if (!M.has(k)) M.set(k, new THREE.MeshStandardMaterial({ color, roughness: 0.6, ...o }));
  return M.get(k);
}
function box(g, w, h, d, color, pos = [0, 0, 0], r = 0.01, o) {
  const m = new THREE.Mesh(rb(w, h, d, r), mat(color, o));
  m.position.set(...pos); m.castShadow = true; m.receiveShadow = true; g.add(m); return m;
}

export function buildProp(type, T) {
  const B = T.blue, E = T.ember, ink = '#1B1E2A', paper = '#F3F1EC', grey = '#9AA3B5';
  const g = new THREE.Group();
  switch (type) {
    case 'book': { const b = makeBook({ cover: B, w: 0.2, h: 0.28, d: 0.05 }); b.rotation.y = deg(90); g.add(b); break; }
    case 'book_red': { const b = makeBook({ cover: '#B5543C', w: 0.2, h: 0.28, d: 0.05 }); g.add(b); break; }
    case 'open_book': { const b = makeOpenBook({ cover: B }); g.add(b); break; }
    case 'book_stack_tipped': {
      ['#B5543C', B, '#E1B862', '#3E6B55'].forEach((c, i) => { const b = makeBook({ cover: c, w: 0.22, h: 0.3, d: 0.05 }); b.rotation.set(-Math.PI / 2, 0, i * 0.3); b.position.set(i * 0.05, 0.03 + i * 0.05, 0); if (i === 3) { b.rotation.set(-1.2, 0.4, 0.6); b.position.set(0.25, 0.12, 0.05); } g.add(b); });
      break;
    }
    case 'mug': g.add(makeMug({ color: B })); break;
    case 'paper': {
      box(g, 0.22, 0.3, 0.006, paper, [0, 0, 0], 0.003);
      for (let i = 0; i < 6; i++) box(g, i === 5 ? 0.09 : 0.16, 0.012, 0.004, i === 2 ? E : grey, [i === 5 ? -0.035 : 0, 0.1 - i * 0.04, 0.004], 0.002);
      break;
    }
    case 'link': {
      for (const [x, rot, c] of [[-0.06, 0, B], [0.06, 90, E]]) {
        const l = new THREE.Group(); l.position.x = x; l.rotation.x = deg(rot); g.add(l);
        box(l, 0.16, 0.035, 0.035, c, [0, 0.045, 0], 0.012); box(l, 0.16, 0.035, 0.035, c, [0, -0.045, 0], 0.012);
        box(l, 0.035, 0.125, 0.035, c, [0.0625, 0, 0], 0.012); box(l, 0.035, 0.125, 0.035, c, [-0.0625, 0, 0], 0.012);
      }
      break;
    }
    case 'file_box': {
      box(g, 0.4, 0.26, 0.3, '#C9A27A', [0, 0, 0], 0.02);
      box(g, 0.42, 0.05, 0.32, '#B38A5E', [0, 0.14, 0], 0.015);
      box(g, 0.12, 0.04, 0.01, ink, [0, 0.03, 0.152], 0.004);
      for (let i = 0; i < 4; i++) box(g, 0.3, 0.16, 0.012, i % 2 ? paper : '#DCE6FF', [0, 0.17 + (i % 2) * 0.02, -0.08 + i * 0.05], 0.003);
      break;
    }
    case 'mic': {
      box(g, 0.05, 0.22, 0.05, ink, [0, -0.04, 0], 0.015);
      box(g, 0.1, 0.12, 0.1, grey, [0, 0.12, 0], 0.04, { metalness: 0.6, roughness: 0.35 });
      box(g, 0.11, 0.025, 0.11, B, [0, 0.06, 0], 0.01);
      break;
    }
    case 'waves': {
      for (let i = 0; i < 3; i++) for (const s of [-1, 1]) {
        const r = 0.14 + i * 0.07;
        for (let k = -2; k <= 2; k++) { const a = k * 0.32; box(g, 0.03, 0.03, 0.03, i === 1 ? E : B, [s * r * Math.cos(a), r * Math.sin(a), 0], 0.008); }
      }
      break;
    }
    case 'sliders': {
      box(g, 0.42, 0.26, 0.05, ink, [0, 0, 0], 0.02);
      [0.6, 0.3, 0.75, 0.45].forEach((v, i) => {
        const x = -0.15 + i * 0.1;
        box(g, 0.012, 0.18, 0.01, '#3A3F4D', [x, 0, 0.03], 0.004);
        box(g, 0.05, 0.03, 0.03, i === 2 ? E : B, [x, -0.09 + v * 0.18, 0.04], 0.008);
      });
      break;
    }
    case 'cloud': {
      const c = '#E6EBF5';
      [[0, 0, 0, 0.5, 0.22], [-0.2, -0.02, 0.02, 0.3, 0.18], [0.2, -0.03, 0.02, 0.32, 0.16], [0.05, 0.12, 0, 0.3, 0.2], [-0.1, 0.08, -0.02, 0.24, 0.18]]
        .forEach(([x, y, z, w, h]) => box(g, w, h, 0.22, c, [x, y, z], 0.05));
      for (let i = 0; i < 6; i++) box(g, 0.02, 0.07, 0.02, '#8AAEFF', [-0.2 + i * 0.08, -0.25 - (i % 2) * 0.08, 0.02], 0.006);
      break;
    }
    case 'magnifier': {
      for (let k = 0; k < 12; k++) { const a = k / 12 * Math.PI * 2; box(g, 0.045, 0.045, 0.035, ink, [Math.cos(a) * 0.1, 0.18 + Math.sin(a) * 0.1, 0], 0.01); }
      const lens = new THREE.Mesh(new THREE.CircleGeometry(0.09, 20), new THREE.MeshPhysicalMaterial({ color: '#BFD3FF', transparent: true, opacity: 0.35, roughness: 0.05 }));
      lens.position.set(0, 0.18, 0); g.add(lens);
      box(g, 0.045, 0.16, 0.045, B, [0, 0.02, 0], 0.012);
      break;
    }
    case 'thumbs': break;
    case 'phone': {
      box(g, 0.13, 0.27, 0.014, '#20232C', [0, 0, 0], 0.02, { metalness: 0.5, roughness: 0.3 });
      box(g, 0.118, 0.255, 0.004, '#0F1424', [0, 0, 0.008], 0.012);
      for (let i = 0; i < 5; i++) box(g, i === 2 ? 0.05 : 0.085, 0.01, 0.003, i === 2 ? E : '#8AAEFF', [i === 2 ? -0.015 : 0, 0.07 - i * 0.03, 0.011], 0.002);
      break;
    }
    case 'laptop': {
      box(g, 0.46, 0.025, 0.32, '#C7CCD6', [0, 0, 0], 0.012, { metalness: 0.4, roughness: 0.35 });
      const lid = new THREE.Group(); lid.position.set(0, 0.012, -0.16); lid.rotation.x = deg(-15); g.add(lid);
      box(lid, 0.46, 0.3, 0.02, '#C7CCD6', [0, 0.15, 0], 0.012, { metalness: 0.4, roughness: 0.35 });
      box(lid, 0.42, 0.26, 0.004, '#0F1424', [0, 0.15, 0.011], 0.006);
      box(lid, 0.2, 0.012, 0.003, B, [-0.06, 0.22, 0.014], 0.002); box(lid, 0.3, 0.012, 0.003, '#8AAEFF', [0, 0.18, 0.014], 0.002); box(lid, 0.12, 0.012, 0.003, E, [-0.09, 0.14, 0.014], 0.002);
      break;
    }
    case 'desktop': {
      box(g, 0.7, 0.44, 0.04, ink, [0, 0.42, 0], 0.02);
      box(g, 0.64, 0.38, 0.004, '#0F1424', [0, 0.42, 0.022], 0.006);
      box(g, 0.3, 0.014, 0.003, B, [-0.12, 0.54, 0.025], 0.002); box(g, 0.48, 0.014, 0.003, '#8AAEFF', [0, 0.48, 0.025], 0.002); box(g, 0.2, 0.014, 0.003, E, [-0.17, 0.42, 0.025], 0.002);
      box(g, 0.06, 0.2, 0.06, '#3A3F4D', [0, 0.12, -0.02], 0.01);
      box(g, 0.28, 0.02, 0.18, '#3A3F4D', [0, 0.01, -0.02], 0.008);
      break;
    }
    case 'envelope': {
      box(g, 0.3, 0.2, 0.02, paper, [0, 0, 0], 0.008);
      const f1 = box(g, 0.2, 0.012, 0.006, '#C9C2B4', [-0.07, 0.03, 0.012], 0.002); f1.rotation.z = deg(-33);
      const f2 = box(g, 0.2, 0.012, 0.006, '#C9C2B4', [0.07, 0.03, 0.012], 0.002); f2.rotation.z = deg(33);
      box(g, 0.05, 0.05, 0.012, E, [0, -0.02, 0.014], 0.01);
      break;
    }
    case 'padlock': {
      box(g, 0.26, 0.22, 0.12, B, [0, 0, 0], 0.03);
      box(g, 0.04, 0.12, 0.04, '#B8C2D6', [-0.08, 0.16, 0], 0.012, { metalness: 0.7, roughness: 0.3 });
      box(g, 0.04, 0.12, 0.04, '#B8C2D6', [0.08, 0.16, 0], 0.012, { metalness: 0.7, roughness: 0.3 });
      box(g, 0.2, 0.04, 0.04, '#B8C2D6', [0, 0.22, 0], 0.012, { metalness: 0.7, roughness: 0.3 });
      box(g, 0.04, 0.07, 0.01, E, [0, -0.01, 0.062], 0.006);
      break;
    }
    case 'wrench': {
      box(g, 0.05, 0.3, 0.03, '#B8C2D6', [0, 0, 0], 0.012, { metalness: 0.7, roughness: 0.3 });
      box(g, 0.12, 0.06, 0.03, '#B8C2D6', [0, 0.17, 0], 0.012, { metalness: 0.7, roughness: 0.3 });
      box(g, 0.04, 0.06, 0.032, '#ffffff', [0, 0.19, 0], 0.005, { transparent: true, opacity: 0 });
      box(g, 0.06, 0.1, 0.035, E, [0, -0.1, 0], 0.012);
      break;
    }
    case 'gear': {
      box(g, 0.2, 0.2, 0.06, B, [0, 0, 0], 0.04);
      for (let k = 0; k < 8; k++) { const a = k / 8 * Math.PI * 2; const t = box(g, 0.06, 0.06, 0.06, B, [Math.cos(a) * 0.12, Math.sin(a) * 0.12, 0], 0.01); t.rotation.z = a; }
      box(g, 0.07, 0.07, 0.07, '#0F1424', [0, 0, 0.01], 0.02);
      break;
    }
    case 'shelf_empty': {
      const w = '#8A5A36';
      box(g, 1.2, 0.05, 0.3, w, [0, 0.0, 0], 0.01); box(g, 1.2, 0.05, 0.3, w, [0, 0.45, 0], 0.01); box(g, 1.2, 0.05, 0.3, w, [0, 0.9, 0], 0.01);
      box(g, 0.05, 0.95, 0.3, w, [-0.6, 0.45, 0], 0.01); box(g, 0.05, 0.95, 0.3, w, [0.6, 0.45, 0], 0.01);
      const b = makeBook({ cover: E, w: 0.2, h: 0.28, d: 0.04 }); b.position.set(0.45, 0.2, 0); b.rotation.set(0, deg(90), deg(-18)); g.add(b);
      break;
    }
    case 'bubbles': {
      const cols = [B, E, '#3DD6A6', '#E6EBF5'];
      const glyphs = [[[0, 0], [1, 0], [0, 1], [1, 1], [2, 1]], [[0, 0], [0, 1], [1, 1], [2, 1], [2, 0]], [[1, 0], [0, 1], [1, 1], [2, 1], [1, 2]], [[0, 0], [1, 0], [2, 0], [1, 1], [1, 2]]];
      [[-0.32, 0.1], [0.3, 0.2], [-0.1, 0.42], [0.22, -0.12]].forEach(([x, y], i) => {
        const b = new THREE.Group(); b.position.set(x, y, 0); g.add(b);
        box(b, 0.3, 0.2, 0.06, cols[i], [0, 0, 0], 0.04);
        box(b, 0.06, 0.06, 0.06, cols[i], [-0.08, -0.12, 0], 0.015);
        for (const [gx, gy] of glyphs[i]) box(b, 0.035, 0.035, 0.02, i === 3 ? '#1B1E2A' : '#FFFFFF', [-0.05 + gx * 0.045, -0.04 + gy * 0.045, 0.035], 0.006);
      });
      break;
    }
    case 'sparkles': {
      [[0, 0, 0.06], [0.2, 0.15, 0.04], [-0.18, 0.2, 0.05]].forEach(([x, y, s]) => { box(g, s, s * 3, s, E, [x, y, 0], 0.01); box(g, s * 3, s, s, E, [x, y, 0], 0.01); });
      break;
    }
    default: break;
  }
  return g;
}
