// Post-processing: depth of field, bloom, tone mapping, colour grade with vignette and grain, plus shot transitions
// and optional motion blur by sub-frame accumulation.
import * as THREE from '../vendor/three/three.module.min.js';
import { EffectComposer } from '../vendor/three/addons/postprocessing/EffectComposer.js';
import { RenderPass } from '../vendor/three/addons/postprocessing/RenderPass.js';
import { BokehPass } from '../vendor/three/addons/postprocessing/BokehPass.js';
import { UnrealBloomPass } from '../vendor/three/addons/postprocessing/UnrealBloomPass.js';
import { OutputPass } from '../vendor/three/addons/postprocessing/OutputPass.js';
import { ShaderPass } from '../vendor/three/addons/postprocessing/ShaderPass.js';

const GradeShader = {
  uniforms: {
    tDiffuse: { value: null }, gain: { value: new THREE.Vector3(1, 1, 1) }, lift: { value: new THREE.Vector3(0, 0, 0) },
    gamma: { value: 1.0 }, saturation: { value: 1.0 }, contrast: { value: 1.0 }, vignette: { value: 0.35 },
    grain: { value: 0.03 }, seed: { value: 0 }, res: { value: new THREE.Vector2(1, 1) }, aberration: { value: 0.0015 },
    letterbox: { value: 0.0 }, fade: { value: 0.0 },
  },
  vertexShader: 'varying vec2 vUv; void main(){ vUv=uv; gl_Position=projectionMatrix*modelViewMatrix*vec4(position,1.0); }',
  fragmentShader: `uniform sampler2D tDiffuse; uniform vec3 gain; uniform vec3 lift; uniform float gamma; uniform float saturation;
    uniform float contrast; uniform float vignette; uniform float grain; uniform float seed; uniform vec2 res; uniform float aberration;
    uniform float letterbox; uniform float fade; varying vec2 vUv;
    float hash(vec2 p){ p=fract(p*vec2(443.897,441.423)+seed); p+=dot(p,p.yx+19.19); return fract((p.x+p.y)*p.x); }
    void main(){
      vec2 d = vUv-0.5;
      vec3 c;
      c.r = texture2D(tDiffuse, vUv - d*aberration).r;
      c.g = texture2D(tDiffuse, vUv).g;
      c.b = texture2D(tDiffuse, vUv + d*aberration).b;
      c = c*gain + lift*(1.0-c);
      c = pow(max(c,0.0), vec3(1.0/gamma));
      float l = dot(c, vec3(0.2126,0.7152,0.0722));
      c = mix(vec3(l), c, saturation);
      c = (c-0.5)*contrast+0.5;
      float v = smoothstep(0.85, 0.2, length(d*vec2(res.x/res.y,1.0))*1.05);
      c *= mix(1.0, v, vignette);
      c += (hash(vUv*res)-0.5)*grain;
      if (letterbox > 0.0 && (vUv.y < letterbox || vUv.y > 1.0-letterbox)) c = vec3(0.0);
      c = mix(c, vec3(0.0), fade);
      gl_FragColor = vec4(clamp(c,0.0,1.0), 1.0);
    }`,
};

const BlendShader = {
  uniforms: { tA: { value: null }, tB: { value: null }, mixv: { value: 0 }, mode: { value: 0 }, dir: { value: new THREE.Vector2(1, 0) } },
  vertexShader: 'varying vec2 vUv; void main(){ vUv=uv; gl_Position=projectionMatrix*modelViewMatrix*vec4(position,1.0); }',
  fragmentShader: `uniform sampler2D tA; uniform sampler2D tB; uniform float mixv; uniform int mode; uniform vec2 dir; varying vec2 vUv;
    vec4 blurAt(sampler2D t, vec2 uv, float amt){ vec4 s=vec4(0.0); for(int i=0;i<32;i++){ float o=(float(i)/31.0-0.5)*amt; s+=texture2D(t, uv+dir*o);} return s/32.0; }
    void main(){
      if (mode==0) { gl_FragColor = mix(texture2D(tA,vUv), texture2D(tB,vUv), mixv); return; }
      // whip / slide: A slides out, B slides in, with directional blur peaking mid-way
      float m = mixv; float blur = sin(3.14159*m)*0.18;
      vec2 off = dir * m;
      vec2 uvA = vUv + off; vec2 uvB = vUv + off - dir;
      vec4 a = blurAt(tA, uvA, blur); vec4 b = blurAt(tB, uvB, blur);
      float inA = step(0.0, uvA.x)*step(uvA.x,1.0)*step(0.0,uvA.y)*step(uvA.y,1.0);
      gl_FragColor = mix(b, a, inA);
    }`,
};

const AccumShader = {
  uniforms: { tDiffuse: { value: null }, weight: { value: 1 } },
  vertexShader: 'varying vec2 vUv; void main(){ vUv=uv; gl_Position=projectionMatrix*modelViewMatrix*vec4(position,1.0); }',
  fragmentShader: 'uniform sampler2D tDiffuse; uniform float weight; varying vec2 vUv; void main(){ gl_FragColor = texture2D(tDiffuse, vUv)*weight; }',
};

export class Post {
  constructor(renderer, w, h, { alpha = false } = {}) {
    this.r = renderer; this.w = w; this.h = h; this.alpha = alpha;
    const rt = new THREE.WebGLRenderTarget(w, h, { type: THREE.HalfFloatType, samples: 4 });
    this.composer = new EffectComposer(renderer, rt);
    this.composer.renderToScreen = false;
    this.renderPass = new RenderPass(null, null);
    this.bokeh = new BokehPass(new THREE.Scene(), new THREE.PerspectiveCamera(), { focus: 2, aperture: 0.002, maxblur: 0.008 });
    this.bloom = new UnrealBloomPass(new THREE.Vector2(w, h), 0.6, 0.6, 0.9);
    this.output = new OutputPass();
    this.grade = new ShaderPass(GradeShader);
    this.grade.uniforms.res.value.set(w, h);
    this.composer.addPass(this.renderPass);
    this.composer.addPass(this.bokeh);
    this.composer.addPass(this.bloom);
    this.composer.addPass(this.output);
    this.composer.addPass(this.grade);
    const mk = () => new THREE.WebGLRenderTarget(w, h, { type: THREE.HalfFloatType });
    this.stashA = mk(); this.stashB = mk(); this.accum = mk();
    this.quad = new FullScreen();
    this.copyMat = new THREE.ShaderMaterial({ ...AccumShader, uniforms: THREE.UniformsUtils.clone(AccumShader.uniforms), depthTest: false, depthWrite: false });
    this.addMat = new THREE.ShaderMaterial({ ...AccumShader, uniforms: THREE.UniformsUtils.clone(AccumShader.uniforms), blending: THREE.AdditiveBlending, transparent: true, depthTest: false, depthWrite: false });
    this.blendMat = new THREE.ShaderMaterial({ ...BlendShader, uniforms: THREE.UniformsUtils.clone(BlendShader.uniforms), depthTest: false, depthWrite: false });
    this.screenMat = new THREE.ShaderMaterial({ ...AccumShader, uniforms: THREE.UniformsUtils.clone(AccumShader.uniforms), depthTest: false, depthWrite: false });
  }

  // look: {dof:{on, focus, aperture, maxblur}, bloom:{strength, radius, threshold}, grade:{...}}
  configure(scene, camera, look, frameSeed) {
    this.renderPass.scene = scene; this.renderPass.camera = camera;
    const dof = look.dof || {};
    this.bokeh.enabled = dof.on !== false && !this.alpha;
    this.bokeh.scene = scene; this.bokeh.camera = camera;
    const bu = this.bokeh.uniforms;
    bu.focus.value = dof.focus ?? 2; bu.aperture.value = dof.aperture ?? 0.006; bu.maxblur.value = dof.maxblur ?? 0.012;
    bu.nearClip.value = camera.near; bu.farClip.value = camera.far; bu.aspect.value = camera.aspect;
    const b = look.bloom || {};
    this.bloom.enabled = b.on !== false;
    this.bloom.strength = b.strength ?? 0.6; this.bloom.radius = b.radius ?? 0.6; this.bloom.threshold = b.threshold ?? 0.9;
    const g = look.grade || {}, U = this.grade.uniforms;
    U.gain.value.set(...(g.gain || [1, 1, 1])); U.lift.value.set(...(g.lift || [0, 0, 0]));
    U.gamma.value = g.gamma ?? 1; U.saturation.value = g.saturation ?? 1; U.contrast.value = g.contrast ?? 1;
    U.vignette.value = g.vignette ?? 0.35; U.grain.value = this.alpha ? 0 : (g.grain ?? 0.025); U.seed.value = (frameSeed % 1000) / 1000;
    U.aberration.value = g.aberration ?? 0.0012; U.letterbox.value = g.letterbox ?? 0; U.fade.value = g.fade ?? 0;
    this.grade.enabled = !this.alpha || g.forceAlpha;
  }

  renderTo(target) {
    this.composer.render();
    this.copy(this.composer.readBuffer.texture, target);
  }

  copy(tex, target, weight = 1, additive = false) {
    const m = additive ? this.addMat : this.copyMat;
    m.uniforms.tDiffuse.value = tex; m.uniforms.weight.value = weight;
    this.quad.render(this.r, m, target, !additive);
  }

  // drawSub(i, n, target) renders sub-frame i of n into target (already graded)
  frame(drawSub, n = 1) {
    if (n <= 1) { drawSub(0, 1, this.stashA); return this.stashA; }
    this.r.setRenderTarget(this.accum); this.r.setClearColor(0x000000, 0); this.r.clear();
    for (let i = 0; i < n; i++) { drawSub(i, n, this.stashB); this.copy(this.stashB.texture, this.accum, 1 / n, true); }
    return this.accum;
  }

  blend(texA, texB, mixv, mode, dir, target) {
    const u = this.blendMat.uniforms;
    u.tA.value = texA; u.tB.value = texB; u.mixv.value = mixv; u.mode.value = mode; u.dir.value.set(...dir);
    this.quad.render(this.r, this.blendMat, target, true);
  }

  present(tex) {
    this.screenMat.uniforms.tDiffuse.value = tex; this.screenMat.uniforms.weight.value = 1;
    this.quad.render(this.r, this.screenMat, null, true);
  }
}

class FullScreen {
  constructor() {
    this.cam = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);
    this.mesh = new THREE.Mesh(new THREE.PlaneGeometry(2, 2));
    this.scene = new THREE.Scene(); this.scene.add(this.mesh);
  }
  render(r, mat, target, clear) {
    this.mesh.material = mat;
    r.setRenderTarget(target);
    if (clear) { r.setClearColor(0x000000, 0); r.clear(); }
    const ac = r.autoClear; r.autoClear = false;
    r.render(this.scene, this.cam);
    r.autoClear = ac;
    r.setRenderTarget(null);
  }
}
