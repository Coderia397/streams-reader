// Neutral studio for spot illustrations and turntables: soft key, fill and rim, no background (transparent renders).
import * as THREE from '../vendor/three/three.module.min.js';
import { deg } from './util.js';

export function buildStudio({ ground = true } = {}) {
  const root = new THREE.Group(); root.name = 'studio';
  const lights = {};
  lights.hemi = new THREE.HemisphereLight('#EAF0FF', '#6B5A4A', 1.2); root.add(lights.hemi);
  lights.key = new THREE.DirectionalLight('#FFF1DE', 2.6); lights.key.position.set(2.2, 4, 3.2);
  lights.key.castShadow = true; lights.key.shadow.mapSize.set(2048, 2048); lights.key.shadow.radius = 8; lights.key.shadow.bias = -0.0008;
  Object.assign(lights.key.shadow.camera, { left: -2, right: 2, top: 2.5, bottom: -1, near: 0.5, far: 12 });
  root.add(lights.key);
  lights.rim = new THREE.DirectionalLight('#8FB0FF', 1.8); lights.rim.position.set(-3, 2.5, -3); root.add(lights.rim);
  lights.fill = new THREE.DirectionalLight('#FFD2A8', 0.6); lights.fill.position.set(-3, 1, 3); root.add(lights.fill);
  if (ground) {
    // shadow catcher: only the contact shadow is visible
    const sc = new THREE.Mesh(new THREE.PlaneGeometry(6, 6), new THREE.ShadowMaterial({ opacity: 0.28 }));
    sc.rotation.x = -Math.PI / 2; sc.receiveShadow = true; root.add(sc);
  }
  return { name: 'studio', root, anchors: { center: new THREE.Vector3() }, lights, background: new THREE.Color('#0B0B0C'), fog: null, exposure: 1.0, envIntensity: 0.6,
    look: { dof: { on: false }, bloom: { strength: 0.2, threshold: 1.2 }, grade: { vignette: 0, grain: 0, aberration: 0 } }, update() {} };
}
