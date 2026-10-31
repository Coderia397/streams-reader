"""Streams Reader film v2: build one segment of the film in Blender from a shot JSON, then render it.

Pure bpy, no add-ons. Run headless:

    blender -b --factory-startup -P build_scene.py -- --shot shots/film.json --segment room_book \
        --out OUT [--frames 1,60,120 | --anim [--range 1:120]] [--engine cycles|eevee] [--samples 128] \
        [--percent 100] [--screen DIR_OF_JPGS] [--save scene.blend] [--metrics]

What it builds (all from code, deterministic by seed):
- the voxel mascot as two skinned meshes on a real armature: IK legs (feet planted on targets, knee poles),
  IK arms (wrists on grip targets that ride on the phone, book or headphone cups, elbow poles), hinge locks on
  knees and elbows, look-at (Damped Track) on neck and head, face bones (eyes blink/gaze, brows, mouth);
- bevelled voxel geometry (Bevel modifier driven by per-edge weights, hardened normals) with a procedural
  pixel texture and per-box colour attributes;
- the reading room at night and the park at golden hour, physically based lights (lamp, fairy lights, moon,
  screen glow as an area light on the face, low sun with volumetric haze), Nishita sky;
- per-frame baked procedural animation (walk with planted feet, breathing, head drift, blinks, holds,
  headphone put-on, cable sway) on controllers; Blender solves IK, skinning, DOF and motion blur.
Coordinates: Blender Z up, metres. In the room the character faces -Y; in the park it walks along +X.
"""
import bpy
import json
import math
import os
import random
import sys
import time
from mathutils import Matrix, Quaternion, Vector

T0 = time.time()
FPS = 30


def log(*a):
    print(f"[film {time.time() - T0:7.1f}s]", *a, flush=True)


# ---------------------------------------------------------------------------------------------------- small maths
def clamp(x, a=0.0, b=1.0):
    return a if x < a else b if x > b else x


def lerp(a, b, u):
    return a + (b - a) * u


def smooth(u):
    u = clamp(u)
    return u * u * (3 - 2 * u)


def smoother(u):
    u = clamp(u)
    return u * u * u * (u * (u * 6 - 15) + 10)


EASE = {"linear": lambda u: clamp(u), "inout": smooth, "smooth": smoother, "in": lambda u: clamp(u) ** 2,
        "out": lambda u: 1 - (1 - clamp(u)) ** 2, "step": lambda u: 1.0 if u >= 1 else 0.0}


def mix(a, b, u):
    """Linear mix of floats, sequences or dicts (missing dict keys are taken as 0 / the other side)."""
    if isinstance(a, dict) or isinstance(b, dict):
        a = a or {}
        b = b or {}
        out = {}
        for k in set(a) | set(b):
            out[k] = mix(a.get(k, zero_like(b.get(k))), b.get(k, zero_like(a.get(k))), u)
        return out
    if isinstance(a, (list, tuple, Vector)):
        return [lerp(x, y, u) for x, y in zip(a, b)]
    return lerp(a, b, u)


def zero_like(v):
    if isinstance(v, dict):
        return {k: zero_like(x) for k, x in v.items()}
    if isinstance(v, (list, tuple, Vector)):
        return [0.0] * len(v)
    return 0.0


def track(keys, t, default=None, field="v"):
    """Value of a key track at time t. Keys: {"t", field, "blend" (s, default 0 = jump), "ease"}.
    A key eases from the value held at its time to its own value over `blend` seconds."""
    if not keys:
        return default
    keys = sorted(keys, key=lambda k: k["t"])
    val = keys[0][field]
    for i, k in enumerate(keys):
        if t < k["t"]:
            break
        held = val
        b = k.get("blend", 0.0)
        if b > 0 and t < k["t"] + b:
            val = mix(held, k[field], EASE[k.get("ease", "smooth")]((t - k["t"]) / b))
        else:
            val = k[field]
    return val


class Noise1:
    """Smooth 1D value noise in [-1, 1], deterministic by seed."""

    def __init__(self, seed):
        r = random.Random(seed)
        self.v = [r.uniform(-1, 1) for _ in range(1024)]

    def at(self, t, f=1.0):
        x = t * f
        i = math.floor(x)
        u = smoother(x - i)
        a, b = self.v[i % 1024], self.v[(i + 1) % 1024]
        return a + (b - a) * u

    def fbm(self, t, f=1.0):
        return 0.7 * self.at(t, f) + 0.3 * self.at(t + 37.1, f * 2.3)


def srgb_to_lin(c):
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def col(h, mul=1.0):
    if isinstance(h, (list, tuple)):
        return (h[0] * mul, h[1] * mul, h[2] * mul, 1.0)
    h = h.lstrip("#")
    r, g, b = (int(h[i:i + 2], 16) / 255 for i in (0, 2, 4))
    return (srgb_to_lin(r) * mul, srgb_to_lin(g) * mul, srgb_to_lin(b) * mul, 1.0)


def jitter(c, r, amt=0.08):
    k = 1 + r.uniform(-amt, amt)
    return (c[0] * k, c[1] * k, c[2] * k, 1.0)


def rad(d):
    return math.radians(d)


def eul(x=0, y=0, z=0):
    from mathutils import Euler
    return Euler((rad(x), rad(y), rad(z)), "XYZ").to_matrix().to_4x4()


def frame_axes(y_axis, z_axis):
    """Rotation matrix (3x3) whose Y and Z columns are the given directions (Z exact, Y orthogonalised)."""
    z = Vector(z_axis).normalized()
    y = Vector(y_axis)
    y = (y - z * y.dot(z)).normalized()
    x = y.cross(z)
    return Matrix((x, y, z)).transposed()


def compose(loc, rot3, scale=1.0):
    m = rot3.to_4x4() if rot3 is not None else Matrix.Identity(4)
    if scale != 1.0:
        m = m @ Matrix.Diagonal((scale, scale, scale, 1.0))
    m.translation = Vector(loc)
    return m


def blend_matrix(a, b, u):
    la, ra, sa = a.decompose()
    lb, rb, sb = b.decompose()
    return Matrix.LocRotScale(la.lerp(lb, u), ra.slerp(rb, u), sa.lerp(sb, u))


# ---------------------------------------------------------------------------------------------------- materials
MATS = {}


def _principled(nt):
    for n in nt.nodes:
        if n.type == "BSDF_PRINCIPLED":
            return n
    return None


def set_in(node, name, value):
    if name in node.inputs:
        node.inputs[name].default_value = value


def voxel_material(name, kind="matte", rough=0.85, pixel=0.03, amp=0.07, sheen=0.0, coat=0.0, metal=0.0,
                   sss=0.0, emit=0.0, bump=0.0, translucent=0.0, spec=0.5):
    """Principled material that reads the per-box colour attribute `col` and adds a pixel-art value variation
    sampled on the snapped rest position (`rest`), so textures stay glued to the mesh while it deforms."""
    if name in MATS:
        return MATS[name]
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    N, L = nt.nodes, nt.links
    bsdf = _principled(nt)
    out = [n for n in N if n.type == "OUTPUT_MATERIAL"][0]
    a = N.new("ShaderNodeAttribute")
    a.attribute_type = "GEOMETRY"
    a.attribute_name = "col"
    rest = N.new("ShaderNodeAttribute")
    rest.attribute_type = "GEOMETRY"
    rest.attribute_name = "rest"
    snap = N.new("ShaderNodeVectorMath")
    snap.operation = "SNAP"
    snap.inputs[1].default_value = (pixel, pixel, pixel)
    L.new(rest.outputs["Vector"], snap.inputs[0])
    wn = N.new("ShaderNodeTexWhiteNoise")
    wn.noise_dimensions = "3D"
    L.new(snap.outputs["Vector"], wn.inputs["Vector"])
    mr = N.new("ShaderNodeMapRange")
    mr.inputs["To Min"].default_value = 1 - amp
    mr.inputs["To Max"].default_value = 1 + amp
    L.new(wn.outputs["Value"], mr.inputs["Value"])
    # broad, soft variation (dirt/fading) on top of the pixel noise
    nz = N.new("ShaderNodeTexNoise")
    nz.inputs["Scale"].default_value = 3.0
    L.new(rest.outputs["Vector"], nz.inputs["Vector"])
    mr2 = N.new("ShaderNodeMapRange")
    mr2.inputs["To Min"].default_value = 0.94
    mr2.inputs["To Max"].default_value = 1.06
    L.new(nz.outputs["Fac"], mr2.inputs["Value"])
    k = N.new("ShaderNodeMath")
    k.operation = "MULTIPLY"
    L.new(mr.outputs["Result"], k.inputs[0])
    L.new(mr2.outputs["Result"], k.inputs[1])
    sc = N.new("ShaderNodeVectorMath")
    sc.operation = "SCALE"
    L.new(a.outputs["Color"], sc.inputs[0])
    L.new(k.outputs["Value"], sc.inputs["Scale"])
    L.new(sc.outputs["Vector"], bsdf.inputs["Base Color"])
    set_in(bsdf, "Roughness", rough)
    set_in(bsdf, "Metallic", metal)
    set_in(bsdf, "Specular IOR Level", spec)
    if sheen:
        set_in(bsdf, "Sheen Weight", sheen)
        set_in(bsdf, "Sheen Roughness", 0.6)
    if coat:
        set_in(bsdf, "Coat Weight", coat)
        set_in(bsdf, "Coat Roughness", 0.15)
    if sss:
        set_in(bsdf, "Subsurface Weight", sss)
        set_in(bsdf, "Subsurface Scale", 0.02)
        set_in(bsdf, "Subsurface Radius", (1.0, 0.45, 0.25))
    if emit:
        L.new(sc.outputs["Vector"], bsdf.inputs["Emission Color"])
        set_in(bsdf, "Emission Strength", emit)
    if bump:
        b = N.new("ShaderNodeBump")
        b.inputs["Strength"].default_value = bump
        b.inputs["Distance"].default_value = 0.002
        L.new(wn.outputs["Value"], b.inputs["Height"])
        L.new(b.outputs["Normal"], bsdf.inputs["Normal"])
    if translucent:
        tr = N.new("ShaderNodeBsdfTranslucent")
        L.new(sc.outputs["Vector"], tr.inputs["Color"])
        mx = N.new("ShaderNodeMixShader")
        mx.inputs[0].default_value = translucent
        L.new(bsdf.outputs[0], mx.inputs[1])
        L.new(tr.outputs[0], mx.inputs[2])
        L.new(mx.outputs[0], out.inputs["Surface"])
    MATS[name] = m
    return m


def std_materials():
    return {
        "matte": voxel_material("matte", rough=0.85),
        "fabric": voxel_material("fabric", rough=0.95, pixel=0.025, amp=0.09, sheen=0.5, bump=0.25),
        "knit": voxel_material("knit", rough=0.95, pixel=0.022, amp=0.1, sheen=0.6, bump=0.35),
        "denim": voxel_material("denim", rough=0.9, pixel=0.02, amp=0.1, sheen=0.2, bump=0.3),
        "skin": voxel_material("skin", rough=0.55, pixel=0.04, amp=0.025, sss=0.12, spec=0.4),
        "hair": voxel_material("hair", rough=0.5, pixel=0.025, amp=0.14, coat=0.15, bump=0.2),
        "plastic": voxel_material("plastic", rough=0.32, pixel=0.05, amp=0.02, coat=0.3),
        "rubber": voxel_material("rubber", rough=0.7, pixel=0.05, amp=0.02),
        "eye": voxel_material("eye", rough=0.12, pixel=0.05, amp=0.0, coat=1.0),
        "glow": voxel_material("glow", rough=0.3, pixel=0.05, amp=0.0, emit=1.5),
        "glow_soft": voxel_material("glow_soft", rough=0.3, pixel=0.05, amp=0.0, emit=0.35),
        "wood": voxel_material("wood", rough=0.5, pixel=0.03, amp=0.08, bump=0.15),
        "metal": voxel_material("metal", rough=0.32, pixel=0.05, amp=0.03, metal=1.0),
        "paper": voxel_material("paper", rough=0.9, pixel=0.01, amp=0.03, sss=0.05),
        "leaf": voxel_material("leaf", rough=0.7, pixel=0.08, amp=0.06, translucent=0.3, spec=0.3),
        "plaster": voxel_material("plaster", rough=0.95, pixel=0.06, amp=0.03),
        "stone": voxel_material("stone", rough=0.9, pixel=0.06, amp=0.08, bump=0.2),
        "ceramic": voxel_material("ceramic", rough=0.2, pixel=0.05, amp=0.02, coat=0.5),
        "bead": voxel_material("bead", rough=0.3, pixel=0.05, amp=0.0, emit=40.0),
        "window_lit": voxel_material("window_lit", rough=0.3, pixel=0.05, amp=0.0, emit=3.0),
    }


# ---------------------------------------------------------------------------------------------------- mesh builder
BEVEL_MAX = 0.1
CUBE_FACES = [(0, 1, 3, 2), (4, 6, 7, 5), (0, 4, 5, 1), (2, 3, 7, 6), (0, 2, 6, 4), (1, 5, 7, 3)]
CUBE_EDGES = [(i, i | b) for i in range(8) for b in (1, 2, 4) if not i & b]


class MB:
    """Accumulates bevelled boxes (one mesh, per-box colour, material, bone group and bevel radius)."""

    def __init__(self):
        self.v, self.f, self.fm, self.fc, self.vg = [], [], [], [], []
        self.ew = {}
        self.mats = []
        self.stack = [Matrix.Identity(4)]

    def push(self, m):
        self.stack.append(self.stack[-1] @ m)

    def pop(self):
        self.stack.pop()

    def box(self, size, pos, r=0.02, c=(0.8, 0.8, 0.8, 1), mat="matte", group=None, rot=None):
        sx, sy, sz = size
        r = max(0.0, min(r, sx / 2.2, sy / 2.2, sz / 2.2))
        m = self.stack[-1] @ Matrix.Translation(Vector(pos))
        if rot is not None:
            m = m @ (eul(*rot) if isinstance(rot, (list, tuple)) else rot)
        base = len(self.v)
        for ix in (-0.5, 0.5):
            for iy in (-0.5, 0.5):
                for iz in (-0.5, 0.5):
                    self.v.append(m @ Vector((ix * sx, iy * sy, iz * sz)))
                    self.vg.append(group)
        if mat not in self.mats:
            self.mats.append(mat)
        mi = self.mats.index(mat)
        for f in CUBE_FACES:
            self.f.append(tuple(base + i for i in f))
            self.fm.append(mi)
            self.fc.append(c)
        w = r / BEVEL_MAX
        if w > 0:
            for a, b in CUBE_EDGES:
                self.ew[(base + a, base + b)] = w
        return base

    def build(self, name, mats, bevel=True, segments=3, coll=None):
        me = bpy.data.meshes.new(name)
        me.from_pydata([tuple(v) for v in self.v], [], self.f)
        me.update()
        for mname in self.mats:
            me.materials.append(mats[mname])
        me.polygons.foreach_set("material_index", self.fm)
        ca = me.color_attributes.new("col", "FLOAT_COLOR", "CORNER")
        flat = []
        for p in me.polygons:
            flat.extend(list(self.fc[p.index]) * p.loop_total)
        ca.data.foreach_set("color", flat)
        ra = me.attributes.new("rest", "FLOAT_VECTOR", "POINT")
        ra.data.foreach_set("vector", [x for v in self.v for x in v])
        if bevel:
            bw = me.attributes.new("bevel_weight_edge", "FLOAT", "EDGE")
            ws = []
            for e in me.edges:
                a, b = sorted(e.vertices)
                ws.append(self.ew.get((a, b), 0.0))
            bw.data.foreach_set("value", ws)
        me.shade_smooth()
        ob = bpy.data.objects.new(name, me)
        (coll or bpy.context.scene.collection).objects.link(ob)
        if bevel:
            md = ob.modifiers.new("Bevel", "BEVEL")
            md.width = BEVEL_MAX
            md.segments = segments
            md.limit_method = "WEIGHT"
            md.profile = 0.5
            md.use_clamp_overlap = True
            md.harden_normals = True
        groups = sorted({g for g in self.vg if g})
        for gname in groups:
            vg = ob.vertex_groups.new(name=gname)
            vg.add([i for i, g in enumerate(self.vg) if g == gname], 1.0, "REPLACE")
        return ob


def new_obj(name, data=None, coll=None):
    ob = bpy.data.objects.new(name, data)
    (coll or bpy.context.scene.collection).objects.link(ob)
    return ob


def empty(name, size=0.05):
    ob = new_obj(name)
    ob.empty_display_size = size
    ob.rotation_mode = "QUATERNION"
    return ob


# ---------------------------------------------------------------------------------------------------- keying
_QPREV = {}


def key_matrix(ob, M, f):
    loc, q, s = M.decompose()
    p = _QPREV.get(ob.name)
    if p is not None and p.dot(q) < 0:
        q.negate()
    _QPREV[ob.name] = q.copy()
    ob.rotation_mode = "QUATERNION"
    ob.location = loc
    ob.rotation_quaternion = q
    ob.scale = s
    ob.keyframe_insert("location", frame=f)
    ob.keyframe_insert("rotation_quaternion", frame=f)
    ob.keyframe_insert("scale", frame=f)


def key_value(owner, path, value, f):
    setattr(owner, path, value)
    owner.keyframe_insert(path, frame=f)


# ---------------------------------------------------------------------------------------------------- character
PAL = {
    "skin": "#F0C29C", "hair": "#3B2A24", "hairHi": "#5A4034",
    "hoodie": "#E6DCC8", "hoodieDark": "#CDBFA6", "trim": "#B9A88C",
    "pants": "#3A4868", "pantsDark": "#2C3752", "shoe": "#F4F1EA", "sole": "#FFB45C",
    "eye": "#2B1B17", "brow": "#3B2A24", "mouth": "#5B2730", "blush": "#F2A08C",
    "blue": "#2F6BFF", "blueDeep": "#1D4BBD", "ember": "#FFB45C", "band": "#1B1E2A",
}
OFF = 0.047          # lifts the three.js character so the soles touch z = 0 in the rest pose
HEAD_S = 0.86        # head scale (as in v1)


def P(x, y, z):
    """three.js character space (Y up, facing +Z) -> Blender rest space (Z up, facing -Y)."""
    return Vector((x, -z, y + OFF))


def S(w, h, d):
    return (w, d, h)


# joint positions (Blender rest space)
J = {
    "hips": P(0, 0.62, 0), "spine": P(0, 0.68, 0), "neck": P(0, 1.12, 0), "head": P(0, 1.18, 0),
}
for _s, _x in (("L", 1), ("R", -1)):
    J["hip." + _s] = P(0.1 * _x, 0.58, 0)
    J["knee." + _s] = P(0.1 * _x, 0.28, 0)
    J["ankle." + _s] = P(0.1 * _x, 0.03, 0)
    J["shoulder." + _s] = P(0.3 * _x, 1.05, 0)
    J["elbow." + _s] = P(0.3 * _x, 0.815, 0)
    J["wrist." + _s] = P(0.3 * _x, 0.6, 0)


def head_pt(x, y, z):
    """Head-local three.js point (scaled 0.86) -> Blender rest space."""
    return J["head"] + Vector((x * HEAD_S, -z * HEAD_S, y * HEAD_S))


FACE = (0.0, 0.25, 0.262)


def face_pt(x, y, z=0.0):
    return head_pt(FACE[0] + x, FACE[1] + y, FACE[2] + z)


class Character:
    def __init__(self, mats, seed=7, coll=None):
        self.mats = mats
        self.seed = seed
        self.coll = coll
        self.build_armature()
        self.build_meshes()
        self.build_headphones()
        self.build_controls()

    # -------------------------------------------------------------------------------------------- armature
    def build_armature(self):
        ad = bpy.data.armatures.new("rig")
        arm = new_obj("rig", ad, self.coll)
        self.arm = arm
        bpy.context.view_layer.objects.active = arm
        arm.select_set(True)
        bpy.ops.object.mode_set(mode="EDIT")
        eb = ad.edit_bones

        def bone(n, h, t, parent=None, connect=False, deform=True):
            b = eb.new(n)
            b.head = h
            b.tail = t
            b.roll = 0.0
            b.use_deform = deform
            if parent:
                b.parent = eb[parent]
                b.use_connect = connect
            return b

        up = Vector((0, 0, 1))
        bone("hips", J["hips"], J["spine"])
        bone("spine", J["spine"], J["neck"], "hips", True)
        bone("neck", J["neck"], J["head"], "spine", True)
        bone("head", J["head"], J["head"] + up * 0.45, "neck", True)
        for s in ("L", "R"):
            knee = J["knee." + s] + Vector((0, -0.015, 0))      # small forward pre-bend: knees flex forward
            elbow = J["elbow." + s] + Vector((0, 0.015, 0))     # elbows flex backward
            bone("thigh." + s, J["hip." + s], knee, "hips")
            bone("shin." + s, knee, J["ankle." + s], "thigh." + s, True)
            bone("foot." + s, J["ankle." + s], J["ankle." + s] + Vector((0, -0.16, 0)), "shin." + s, True)
            bone("upper_arm." + s, J["shoulder." + s], elbow, "spine")
            bone("forearm." + s, elbow, J["wrist." + s], "upper_arm." + s, True)
            bone("hand." + s, J["wrist." + s], J["wrist." + s] + Vector((0, 0, -0.1)), "forearm." + s, True)
            sx = 1 if s == "L" else -1
            e = face_pt(0.115 * sx, -0.01)
            bone("eye." + s, e, e + up * 0.05, "head")
            b = face_pt(0.115 * sx, 0.108, 0.004)
            bone("brow." + s, b, b + up * 0.04, "head")
            mc = face_pt(0.04 * sx, -0.105 + 0.01, 0.004)
            bone("mouth_corner." + s, mc, mc + up * 0.03, "head")
        m = face_pt(0, -0.105, 0.004)
        bone("mouth", m, m + up * 0.03, "head")
        bpy.ops.object.mode_set(mode="OBJECT")
        for pb in arm.pose.bones:
            pb.rotation_mode = "XYZ"
        ad.display_type = "STICK"

    # -------------------------------------------------------------------------------------------- meshes
    def build_meshes(self):
        r = random.Random(self.seed)
        c = {k: col(v) for k, v in PAL.items()}
        body = MB()
        bx = body.box
        # hips + legs
        bx(S(0.40, 0.14, 0.25), P(0, 0.62, 0), 0.03, c["pants"], "denim", "hips")
        for s, sx in (("L", 1), ("R", -1)):
            bx(S(0.185, 0.16, 0.2), P(0.1 * sx, 0.57, 0), 0.07, c["pants"], "denim", "hips")
            bx(S(0.18, 0.32, 0.2), P(0.1 * sx, 0.43, 0), 0.03, c["pants"], "denim", "thigh." + s)
            bx(S(0.172, 0.17, 0.19), P(0.1 * sx, 0.28, 0), 0.075, c["pants"], "denim", "thigh." + s)
            bx(S(0.17, 0.25, 0.185), P(0.1 * sx, 0.17, 0), 0.03, c["pants"], "denim", "shin." + s)
            bx(S(0.18, 0.035, 0.195), P(0.1 * sx, 0.065, 0), 0.012, c["pantsDark"], "denim", "shin." + s)
            bx(S(0.19, 0.075, 0.28), P(0.1 * sx, 0.005, 0.04), 0.03, c["shoe"], "rubber", "foot." + s)
            bx(S(0.196, 0.034, 0.286), P(0.1 * sx, -0.03, 0.04), 0.012, c["sole"], "rubber", "foot." + s)
            bx(S(0.06, 0.012, 0.05), P(0.1 * sx, 0.045, 0.16), 0.004, c["blue"], "plastic", "foot." + s)  # lace tab
        # torso
        bx(S(0.46, 0.44, 0.28), P(0, 0.90, 0), 0.05, c["hoodie"], "knit", "spine")
        bx(S(0.47, 0.07, 0.29), P(0, 0.705, 0), 0.025, c["trim"], "knit", "spine")
        bx(S(0.26, 0.11, 0.03), P(0, 0.80, 0.14), 0.012, c["hoodieDark"], "knit", "spine")
        bx(S(0.024, 0.11, 0.02), P(0.05, 1.04, 0.148), 0.008, c["trim"], "fabric", "spine")
        bx(S(0.024, 0.09, 0.02), P(-0.05, 1.05, 0.148), 0.008, c["trim"], "fabric", "spine")
        bx(S(0.40, 0.13, 0.12), P(0, 1.11, -0.12), 0.05, c["hoodieDark"], "knit", "spine")
        bx(S(0.15, 0.08, 0.14), P(0, 1.15, 0), 0.02, c["skin"], "skin", "neck")
        # arms
        for s, sx in (("L", 1), ("R", -1)):
            bx(S(0.16, 0.16, 0.17), P(0.3 * sx, 1.04, 0), 0.07, c["hoodie"], "knit", "upper_arm." + s)
            bx(S(0.15, 0.27, 0.16), P(0.3 * sx, 0.95, 0), 0.04, c["hoodie"], "knit", "upper_arm." + s)
            bx(S(0.145, 0.15, 0.155), P(0.3 * sx, 0.815, 0), 0.065, c["hoodie"], "knit", "upper_arm." + s)
            bx(S(0.14, 0.2, 0.15), P(0.3 * sx, 0.735, 0), 0.035, c["hoodie"], "knit", "forearm." + s)
            bx(S(0.15, 0.045, 0.16), P(0.3 * sx, 0.63, 0), 0.015, c["trim"], "knit", "forearm." + s)
            bx(S(0.12, 0.12, 0.1), P(0.3 * sx, 0.55, 0), 0.03, c["skin"], "skin", "hand." + s)
            bx(S(0.04, 0.07, 0.045), P(0.3 * sx - 0.06 * sx, 0.565, 0.035), 0.015, c["skin"], "skin", "hand." + s)
        self.body = body.build("body", self.mats, coll=self.coll)

        head = MB()
        hb = head.box

        def H(size, p, rr, cc, mat, grp="head"):
            hb(tuple(v * HEAD_S for v in S(*size)), head_pt(*p), rr * HEAD_S, cc, mat, grp)

        H((0.58, 0.52, 0.52), (0, 0.27, 0), 0.09, c["skin"], "skin")
        for sx in (1, -1):
            H((0.06, 0.1, 0.08), (0.3 * sx, 0.24, -0.01), 0.02, c["skin"], "skin")
        hair = lambda: jitter(c["hair"], r, 0.06)
        H((0.61, 0.12, 0.55), (0, 0.505, -0.01), 0.05, hair(), "hair")
        H((0.62, 0.36, 0.14), (0, 0.35, -0.22), 0.05, hair(), "hair")
        for sx in (1, -1):
            H((0.06, 0.14, 0.4), (0.29 * sx, 0.42, -0.05), 0.025, hair(), "hair")
        H((0.3, 0.1, 0.05), (0.15, 0.44, 0.255), 0.02, hair(), "hair")
        H((0.14, 0.06, 0.05), (0.21, 0.375, 0.258), 0.02, hair(), "hair")
        H((0.2, 0.08, 0.05), (-0.06, 0.455, 0.256), 0.02, hair(), "hair")
        H((0.16, 0.13, 0.05), (-0.215, 0.42, 0.255), 0.02, hair(), "hair")
        H((0.46, 0.06, 0.4), (0.03, 0.55, 0.02), 0.03, col(PAL["hairHi"]), "hair")
        H((0.24, 0.05, 0.22), (0.1, 0.585, 0.08), 0.02, hair(), "hair")
        for s, sx in (("L", 1), ("R", -1)):
            ex, ey, ez = FACE[0] + 0.115 * sx, FACE[1] - 0.01, FACE[2]
            H((0.088, 0.115, 0.02), (ex, ey, ez), 0.014, c["eye"], "eye", "eye." + s)
            H((0.026, 0.026, 0.012), (ex + 0.018 * sx, ey + 0.028, ez + 0.012), 0.006, col("#FFFFFF"), "glow_soft", "eye." + s)
            H((0.012, 0.012, 0.012), (ex - 0.02 * sx, ey - 0.03, ez + 0.012), 0.003, col("#FFFFFF"), "glow_soft", "eye." + s)
            H((0.1, 0.026, 0.02), (ex, FACE[1] + 0.108, ez + 0.004), 0.01, c["brow"], "hair", "brow." + s)
            H((0.055, 0.022, 0.008), (0.19 * sx, FACE[1] - 0.08, ez + 0.002), 0.006, c["blush"], "skin")
            H((0.026, 0.02, 0.02), (0.04 * sx, FACE[1] - 0.095, ez + 0.004), 0.008, c["mouth"], "matte", "mouth_corner." + s)
        H((0.05, 0.018, 0.02), (0, FACE[1] - 0.105, FACE[2] + 0.004), 0.008, c["mouth"], "matte", "mouth")
        self.head = head.build("head", self.mats, coll=self.coll)
        for ob in (self.body, self.head):
            md = ob.modifiers.new("Armature", "ARMATURE")
            md.object = self.arm
            ob.parent = self.arm

    # -------------------------------------------------------------------------------------------- headphones
    def build_headphones(self):
        """Rigid over-ear headphones in their own frame: X across, Z up (band), cups at x = +-0.335 (scaled)."""
        c = {k: col(v) for k, v in PAL.items()}
        hp = MB()
        s = HEAD_S
        for sx in (1, -1):
            hp.box((0.09 * s, 0.17 * s, 0.2 * s), (0.335 * sx * s, 0, 0), 0.04 * s, c["blue"], "plastic")
            hp.box((0.03 * s, 0.12 * s, 0.15 * s), ((0.335 + 0.05) * sx * s, 0, 0), 0.012 * s, c["ember"], "glow")
            hp.box((0.04 * s, 0.15 * s, 0.17 * s), ((0.335 - 0.05) * sx * s, 0, 0), 0.03 * s, c["blueDeep"], "fabric")
            hp.box((0.035 * s, 0.03 * s, 0.06 * s), (0.335 * sx * s, 0, 0.12 * s), 0.01 * s, c["band"], "plastic")
        R = 0.36
        for i in range(9):
            a = math.pi * i / 8
            cc = c["band"] if i in (0, 8) else (c["blue"] if i % 2 else c["blueDeep"])
            hp.box((0.085 * s, 0.075 * s, 0.055 * s), (math.cos(a) * R * s, 0, (math.sin(a) * R * 1.02 + 0.06) * s),
                   0.02 * s, cc, "plastic", rot=(0, -(math.degrees(a) - 90), 0))
        self.hp = hp.build("headphones", self.mats, coll=self.coll)
        self.hp.rotation_mode = "QUATERNION"
        # jack point (bottom of the left cup) in headphone space
        self.hp_jack = Vector((0.335 * s, -0.02, -0.1 * s))

    # -------------------------------------------------------------------------------------------- controls
    def build_controls(self):
        arm = self.arm
        self.tg = {}
        for s in ("L", "R"):
            for n in ("ik_foot", "pole_knee", "ik_hand", "pole_elbow"):
                self.tg[f"{n}.{s}"] = empty(f"{n}.{s}")
        self.look = empty("look_target")
        pb = arm.pose.bones
        self.con = {}
        for s in ("L", "R"):
            for chain, tip, tgt, pole in (("shin", "foot", "ik_foot", "pole_knee"), ("forearm", "hand", "ik_hand", "pole_elbow")):
                ik = pb[f"{chain}.{s}"].constraints.new("IK")
                ik.target = self.tg[f"{tgt}.{s}"]
                ik.pole_target = self.tg[f"{pole}.{s}"]
                ik.chain_count = 2
                ik.use_tail = True
                ik.use_stretch = False
                pb[f"{chain}.{s}"].lock_ik_y = True
                pb[f"{chain}.{s}"].lock_ik_z = True
                cr = pb[f"{tip}.{s}"].constraints.new("COPY_ROTATION")
                cr.target = self.tg[f"{tgt}.{s}"]
                cr.target_space = "WORLD"
                cr.owner_space = "WORLD"
                cr.mix_mode = "REPLACE"
                self.con[f"{chain}.{s}"] = ik
                self.con[f"{tip}.{s}"] = cr
        for b, w in (("neck", 0.35), ("head", 0.65)):
            dt = pb[b].constraints.new("DAMPED_TRACK")
            dt.target = self.look
            dt.track_axis = "TRACK_Z"
            dt.influence = 0.0
            self.con["look." + b] = dt
        self.calibrate_poles()

    def world_bone(self, name, dg=None):
        dg = dg or bpy.context.evaluated_depsgraph_get()
        a = self.arm.evaluated_get(dg)
        return a.matrix_world @ a.pose.bones[name].matrix

    def calibrate_poles(self):
        """Pick each IK pole angle so the knee/elbow bends towards its pole target (tested in a bent pose)."""
        tests = {
            "shin": (lambda sx: Vector((0.1 * sx, -0.25, 0.3)), lambda sx: Vector((0.1 * sx, -1.0, 0.35)), "thigh"),
            "forearm": (lambda sx: Vector((0.28 * sx, -0.22, 0.85)), lambda sx: Vector((0.7 * sx, 0.6, 0.7)), "upper_arm"),
        }
        self.arm.location = (0, 0, 0)
        self.pole_angle = {}
        for chain, (tgt, pole, upper) in tests.items():
            for s, sx in (("L", 1), ("R", -1)):
                key = "ik_foot" if chain == "shin" else "ik_hand"
                pkey = "pole_knee" if chain == "shin" else "pole_elbow"
                self.tg[f"{key}.{s}"].location = tgt(sx)
                self.tg[f"{pkey}.{s}"].location = pole(sx)
                best = None
                for ang in (-90, 0, 90, 180):
                    self.con[f"{chain}.{s}"].pole_angle = rad(ang)
                    bpy.context.view_layer.update()
                    root = self.world_bone(f"{upper}.{s}").translation
                    mid = self.world_bone(f"{chain}.{s}").translation
                    tip = (self.world_bone(f"{chain}.{s}") @ Vector((0, self.arm.data.bones[f"{chain}.{s}"].length, 0)))
                    axis = (tip - root).normalized()
                    proj = root + axis * (mid - root).dot(axis)
                    bend = (mid - proj)
                    pv = pole(sx) - proj
                    pv = pv - axis * pv.dot(axis)
                    score = bend.normalized().dot(pv.normalized()) - (tip - tgt(sx)).length * 5
                    if best is None or score > best[0]:
                        best = (score, ang)
                self.con[f"{chain}.{s}"].pole_angle = rad(best[1])
                self.pole_angle[f"{chain}.{s}"] = best
        log("pole angles", self.pole_angle)


# ---------------------------------------------------------------------------------------------------- props
def build_phone(mats, screen_dir, offset, coll=None):
    """Phone 0.138 x 0.288 m (1.5x a real phone so the app reads on screen). Frame: X right, Y up, Z screen normal."""
    s = 1.5
    w, h, d = 0.092 * s, 0.192 * s, 0.0095 * s
    mb = MB()
    mb.box((w, h, d), (0, 0, 0), 0.011 * s, col("#20232C"), "metal")
    mb.box((w + 0.002, h + 0.002, d * 0.55), (0, 0, 0), 0.011 * s, col("#3A3F4D"), "metal")
    mb.box((0.034 * s, 0.034 * s, 0.003 * s), (-0.02 * s, 0.066 * s, -d / 2 - 0.0012), 0.008 * s, col("#20232C"), "metal")
    ob = mb.build("phone", mats, coll=coll)
    ob.rotation_mode = "QUATERNION"
    # screen: a plane with the app capture as an image sequence (emission), glass on top
    sw, sh = 0.084 * s, 0.084 * s * 844 / 390
    me = bpy.data.meshes.new("screen")
    z = d / 2 + 0.0006
    me.from_pydata([(-sw / 2, -sh / 2, z), (sw / 2, -sh / 2, z), (sw / 2, sh / 2, z), (-sw / 2, sh / 2, z)], [], [(0, 1, 2, 3)])
    uv = me.uv_layers.new(name="UVMap")
    for i, (u, v) in enumerate(((0, 0), (1, 0), (1, 1), (0, 1))):
        uv.data[i].uv = (u, v)
    scr = new_obj("screen", me, coll)
    scr.parent = ob
    m = bpy.data.materials.new("screen")
    m.use_nodes = True
    nt = m.node_tree
    bsdf = _principled(nt)
    set_in(bsdf, "Base Color", (0.0, 0.0, 0.0, 1))
    set_in(bsdf, "Roughness", 0.08)
    set_in(bsdf, "Specular IOR Level", 0.25)
    tex = nt.nodes.new("ShaderNodeTexImage")
    tex.extension = "EXTEND"
    frames = sorted(f for f in os.listdir(screen_dir) if f.endswith(".jpg")) if screen_dir and os.path.isdir(screen_dir) else []
    if frames:
        img = bpy.data.images.load(os.path.join(screen_dir, frames[0]))
        img.source = "SEQUENCE"
        tex.image = img
        iu = tex.image_user
        iu.frame_duration = len(frames)
        iu.frame_start = 1
        iu.frame_offset = int(offset)
        iu.use_auto_refresh = True
        iu.use_cyclic = False
    else:
        log("WARNING: no screen frames in", screen_dir)
    nt.links.new(tex.outputs["Color"], bsdf.inputs["Emission Color"])
    set_in(bsdf, "Emission Strength", 1.0)
    me.materials.append(m)
    phone = {"obj": ob, "screen": scr, "emission": bsdf.inputs["Emission Strength"], "w": w, "h": h, "d": d,
             "jack": Vector((0, -h / 2, 0)), "center": Vector((0, 0, z))}
    # the screen's own light on the face and hands: a rectangle light the size of the screen
    ld = bpy.data.lights.new("screen_glow", "AREA")
    ld.shape = "RECTANGLE"
    ld.size = sw
    ld.size_y = sh
    ld.color = (0.78, 0.86, 1.0)
    ld.energy = 2.0
    lo = new_obj("screen_glow", ld, coll)
    lo.parent = ob
    lo.location = (0, 0, z + 0.003)
    lo.rotation_euler = (math.pi, 0, 0)       # area lights emit along local -Z; the screen faces +Z
    try:
        lo.visible_camera = False
    except AttributeError:
        pass
    phone["light"] = ld
    return phone


def build_book(mats, coll=None):
    """Open paperback, 0.30 x 0.21 m spread. Frame: X across the spread, Y up the page, Z out of the pages."""
    mb = MB()
    cover = col("#7A2E2E")
    page = col("#F1E8D6")
    for sx in (1, -1):
        mb.push(Matrix.Translation((0.0, 0, 0)) @ eul(0, -6 * sx, 0))
        mb.box((0.15, 0.215, 0.005), (0.075 * sx, 0, -0.012), 0.002, cover, "fabric")
        mb.box((0.142, 0.205, 0.014), (0.073 * sx, 0, -0.002), 0.003, page, "paper")
        # printed lines (dark strips)
        for i in range(13):
            ln = 0.105 if i not in (0, 12) else 0.06
            mb.box((ln, 0.005, 0.001), (0.075 * sx - (0.105 - ln) / 2 * sx, 0.085 - i * 0.0135, 0.0055), 0.0,
                   col("#5A5048"), "matte")
        mb.pop()
    mb.box((0.012, 0.215, 0.02), (0, 0, -0.012), 0.004, cover, "fabric")
    ob = mb.build("book", mats, coll=coll)
    ob.rotation_mode = "QUATERNION"
    return {"obj": ob, "center": Vector((0, 0, 0.005)), "w": 0.3, "h": 0.215}


# grips: hand centre in the item frame, hand axes as columns (bone X, Y = fingers, Z) in the item frame
GRIP_ROT = Matrix(((-1, 0, 0), (0, 1, 0), (0, 0, -1)))   # palm faces the edge, fingers up, thumb on the screen side


def grip_matrix(item_world, item, side):
    sx = 1 if side == "R" else -1
    if item.get("kind") == "book":
        p = Vector((sx * (item["w"] / 2 + 0.035), -0.03, -0.012))
    else:
        p = Vector((sx * (item["w"] / 2 + 0.03), -0.045, -0.012))
    return item_world @ compose(p, GRIP_ROT)


def hand_target_from_center(M):
    """IK target sits on the wrist pivot; the hand centre is 0.05 along the bone Y axis."""
    out = M.copy()
    out.translation = M.translation - M.to_3x3() @ Vector((0, 0.05, 0))
    return out


# ---------------------------------------------------------------------------------------------------- sets
def pixel_image(name, size, fn):
    img = bpy.data.images.new(name, size, size, alpha=False)
    px = []
    for y in range(size):
        for x in range(size):
            r, g, b = fn(x, size - 1 - y)
            px.extend((r / 255, g / 255, b / 255, 1.0))      # byte image in sRGB: store display values
    img.pixels = px
    img.pack()
    return img


def image_material(name, img, rough=0.9, emit=0.0):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    bsdf = _principled(nt)
    tc = nt.nodes.new("ShaderNodeTexCoord")
    tex = nt.nodes.new("ShaderNodeTexImage")
    tex.image = img
    tex.interpolation = "Closest"
    nt.links.new(tc.outputs["UV"], tex.inputs["Vector"])
    nt.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
    set_in(bsdf, "Roughness", rough)
    if emit:
        nt.links.new(tex.outputs["Color"], bsdf.inputs["Emission Color"])
        set_in(bsdf, "Emission Strength", emit)
    return m


def quad(name, w, h, mat, M, coll=None):
    me = bpy.data.meshes.new(name)
    me.from_pydata([(-w / 2, -h / 2, 0), (w / 2, -h / 2, 0), (w / 2, h / 2, 0), (-w / 2, h / 2, 0)], [], [(0, 1, 2, 3)])
    uv = me.uv_layers.new(name="UVMap")
    for i, (u, v) in enumerate(((0, 0), (1, 0), (1, 1), (0, 1))):
        uv.data[i].uv = (u, v)
    me.materials.append(mat)
    ob = new_obj(name, me, coll)
    ob.matrix_world = M
    return ob


def node_material(name):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    return m, m.node_tree, _principled(m.node_tree)


def planks_material():
    m, nt, bsdf = node_material("planks")
    N, L = nt.nodes, nt.links
    tc = N.new("ShaderNodeTexCoord")
    br = N.new("ShaderNodeTexBrick")
    br.offset = 0.37
    br.inputs["Color1"].default_value = col("#7A5238")
    br.inputs["Color2"].default_value = col("#8C6142")
    br.inputs["Mortar"].default_value = col("#3A271B")
    br.inputs["Scale"].default_value = 1.0
    br.inputs["Mortar Size"].default_value = 0.004
    br.inputs["Brick Width"].default_value = 1.2
    br.inputs["Row Height"].default_value = 0.16
    mp = N.new("ShaderNodeMapping")
    L.new(tc.outputs["Object"], mp.inputs["Vector"])
    mp.inputs["Rotation"].default_value = (0, 0, math.pi / 2)
    L.new(mp.outputs["Vector"], br.inputs["Vector"])
    nz = N.new("ShaderNodeTexNoise")
    nz.inputs["Scale"].default_value = 18
    nz.inputs["Detail"].default_value = 4
    L.new(mp.outputs["Vector"], nz.inputs["Vector"])
    mr = N.new("ShaderNodeMapRange")
    mr.inputs["To Min"].default_value = 0.72
    mr.inputs["To Max"].default_value = 1.12
    L.new(nz.outputs["Fac"], mr.inputs["Value"])
    mv = N.new("ShaderNodeVectorMath")
    mv.operation = "SCALE"
    L.new(br.outputs["Color"], mv.inputs[0])
    L.new(mr.outputs["Result"], mv.inputs["Scale"])
    L.new(mv.outputs["Vector"], bsdf.inputs["Base Color"])
    set_in(bsdf, "Roughness", 0.45)
    set_in(bsdf, "Coat Weight", 0.25)
    bump = N.new("ShaderNodeBump")
    bump.inputs["Strength"].default_value = 0.3
    L.new(br.outputs["Fac"], bump.inputs["Height"])
    L.new(bump.outputs["Normal"], bsdf.inputs["Normal"])
    return m


def build_room(mats, rng, coll=None):
    """Reading room at night (three.js v1 layout, converted; armchair lowered so the feet reach the floor)."""
    mb = MB()
    B = mb.box
    wall = col("#3C5363")
    wain = col("#2C3D49")
    trim = col("#E8E0D2")
    # floor (separate object for the plank material)
    floor = MB()
    floor.box((8, 7, 0.04), (0, -0.3, -0.02), 0.0, col("#FFFFFF"), "matte")
    fo = floor.build("floor", {"matte": planks_material()}, bevel=False, coll=coll)
    # walls: back wall with a window hole (x 0.375..1.725, z 0.875..2.225), left wall, ceiling
    WX0, WX1, WZ0, WZ1 = 0.375, 1.725, 0.875, 2.225
    B((WX0 + 4, 0.06, 3.0), ((-4 + WX0) / 2, 2.23, 1.5), 0.0, wall, "plaster")
    B((4 - WX1, 0.06, 3.0), ((WX1 + 4) / 2, 2.23, 1.5), 0.0, wall, "plaster")
    B((WX1 - WX0, 0.06, WZ0), ((WX0 + WX1) / 2, 2.23, WZ0 / 2), 0.0, wall, "plaster")
    B((WX1 - WX0, 0.06, 3.0 - WZ1), ((WX0 + WX1) / 2, 2.23, (WZ1 + 3.0) / 2), 0.0, wall, "plaster")
    B((0.06, 6.0, 3.0), (-2.83, -0.8, 1.5), 0.0, wall, "plaster")
    B((8, 7, 0.06), (0, -0.3, 3.03), 0.0, col("#2A3440"), "plaster")
    B((8, 0.04, 0.9), (0, 2.18, 0.45), 0.005, wain, "plaster")
    B((0.04, 6.0, 0.9), (-2.78, -0.8, 0.45), 0.005, wain, "plaster")
    B((8, 0.06, 0.05), (0, 2.16, 0.92), 0.01, trim, "matte")
    B((8, 0.05, 0.1), (0, 2.17, 0.05), 0.01, trim, "matte")
    B((0.06, 6.0, 0.05), (-2.76, -0.8, 0.92), 0.01, trim, "matte")
    # window frame, mullions, sill, reveal
    frame = col("#EDE6DA")
    cx, cz, W, H = 1.05, 1.55, 1.35, 1.35
    B((W + 0.14, 0.14, 0.09), (cx, 2.18, cz + H / 2 + 0.03), 0.02, frame, "matte")
    B((W + 0.14, 0.14, 0.09), (cx, 2.18, cz - H / 2 - 0.03), 0.02, frame, "matte")
    B((0.09, 0.14, H + 0.1), (cx + W / 2 + 0.03, 2.18, cz), 0.02, frame, "matte")
    B((0.09, 0.14, H + 0.1), (cx - W / 2 - 0.03, 2.18, cz), 0.02, frame, "matte")
    B((0.05, 0.08, H), (cx, 2.18, cz), 0.01, frame, "matte")
    B((W, 0.08, 0.05), (cx, 2.18, cz + 0.1), 0.01, frame, "matte")
    B((W + 0.34, 0.3, 0.06), (cx, 2.08, cz - H / 2 - 0.1), 0.02, frame, "matte")
    for dx, dz, sw, sh in ((0, H / 2, W, 0.02), (0, -H / 2, W, 0.02), (-W / 2, 0, 0.02, H), (W / 2, 0, 0.02, H)):
        B((sw, 0.2, sh), (cx + dx, 2.3, cz + dz), 0.005, col("#D9D0C2"), "matte")
    # rug
    rug_img = pixel_image("rug", 32, rug_pixels(rng))
    q = quad("rug", 2.4, 1.7, image_material("rug", rug_img, 1.0), Matrix.Translation((-0.15, 0.85, 0.012)), coll)
    # armchair: centre (-0.2, 1.35), seat top 0.44, armrest top 0.63
    fab = col("#A4553E")
    fabd = col("#A4553E", 0.72)
    ch = Matrix.Translation((-0.2, 1.35, 0))
    mb.push(ch)
    B((1.05, 0.86, 0.24), (0, 0, 0.2), 0.06, fabd, "fabric")
    B((0.78, 0.74, 0.12), (0, -0.04, 0.38), 0.06, fab, "fabric")
    B((1.05, 0.22, 0.9), (0, 0.34, 0.6), 0.08, fabd, "fabric")
    B((0.74, 0.1, 0.5), (0, 0.2, 0.68), 0.05, fab, "fabric", rot=(-8, 0, 0))
    for sx in (1, -1):
        B((0.16, 0.86, 0.36), (0.46 * sx, 0, 0.45), 0.07, fabd, "fabric")
    for x, y in ((-0.42, -0.34), (0.42, -0.34), (-0.42, 0.34), (0.42, 0.34)):
        B((0.07, 0.07, 0.12), (x, y, 0.06), 0.015, col("#5A3B26"), "wood")
    B((0.2, 0.5, 0.3), (-0.5, -0.05, 0.6), 0.06, col("#E5D7B8"), "knit", rot=(0, 8, 0))
    mb.pop()
    # ottoman in front of the chair: the short voxel legs rest on it instead of dangling
    B((0.64, 0.46, 0.22), (-0.2, 0.68, 0.17), 0.06, fab, "fabric")
    B((0.6, 0.42, 0.06), (-0.2, 0.68, 0.06), 0.02, fabd, "fabric")
    for x, y in ((-0.45, 0.5), (0.05, 0.5), (-0.45, 0.86), (0.05, 0.86)):
        B((0.05, 0.05, 0.06), (x, y, 0.03), 0.01, col("#5A3B26"), "wood")
    # floor lamp
    brass = col("#B8894A")
    mb.push(Matrix.Translation((-1.05, 1.6, 0)))
    B((0.34, 0.34, 0.04), (0, 0, 0.02), 0.015, brass, "metal")
    B((0.035, 0.035, 1.64), (0, 0, 0.84), 0.01, brass, "metal")
    B((0.035, 0.32, 0.035), (0, -0.14, 1.66), 0.01, brass, "metal")
    mb.pop()
    # side table with mug, books and a succulent
    wood = col("#5A3B26")
    mb.push(Matrix.Translation((0.62, 1.2, 0)))
    B((0.46, 0.46, 0.04), (0, 0, 0.55), 0.015, wood, "wood")
    B((0.05, 0.05, 0.53), (0, 0, 0.27), 0.01, wood, "wood")
    B((0.3, 0.3, 0.03), (0, 0, 0.015), 0.01, wood, "wood")
    B((0.09, 0.09, 0.1), (0.1, -0.1, 0.62), 0.02, col("#2F6BFF"), "ceramic")
    B((0.02, 0.025, 0.05), (0.16, -0.1, 0.625), 0.008, col("#2F6BFF"), "ceramic")
    B((0.075, 0.075, 0.005), (0.1, -0.1, 0.668), 0.002, col("#3B2416"), "ceramic")
    for i, cc in enumerate(("#7A2E2E", "#2C3550", "#E1B862", "#3E6B55")):
        B((0.2 - i * 0.015, 0.27 - i * 0.02, 0.04), (-0.08, 0.05, 0.59 + i * 0.041), 0.006, col(cc), "fabric",
          rot=(0, 0, rng.uniform(-17, 17)))
    B((0.09, 0.09, 0.08), (0.12, 0.13, 0.61), 0.015, col("#C8714E"), "ceramic")
    for i in range(5):
        B((0.04, 0.03, 0.07), (0.12 + math.cos(i * 1.3) * 0.025, 0.13 - math.sin(i * 1.3) * 0.025, 0.68), 0.012,
          col("#5E8E5A"), "leaf", rot=(rng.uniform(-23, 23), rng.uniform(-23, 23), i * 57))
    mb.pop()
    # bookshelf
    shelf = col("#6B4A33")
    mb.push(Matrix.Translation((-2.05, 1.95, 0)))
    B((1.2, 0.06, 2.1), (0, 0.15, 1.05), 0.01, shelf, "wood")
    for sx in (1, -1):
        B((0.06, 0.36, 2.1), (0.6 * sx, 0, 1.05), 0.01, shelf, "wood")
    ys = [0.05, 0.5, 0.95, 1.4, 1.85, 2.1]
    for z in ys:
        B((1.2, 0.36, 0.05), (0, 0, z), 0.01, shelf, "wood")
    spines = ["#7A2E2E", "#2C3550", "#E1B862", "#3E6B55", "#B5543C", "#E9DFC9", "#2F6BFF", "#5B4A7A", "#C9A27A", "#1F3A2E"]
    for si in range(4):
        x = -0.55
        while x < 0.52:
            w, h, d = rng.uniform(0.03, 0.06), rng.uniform(0.26, 0.38), rng.uniform(0.22, 0.28)
            if rng.random() < 0.07:
                x += rng.uniform(0.06, 0.14)
                continue
            lean = rng.uniform(8, 16) if rng.random() < 0.06 else 0
            B((w, d, h), (x + w / 2, -0.02, ys[si] + 0.025 + h / 2), 0.004, jitter(col(rng.choice(spines)), rng, 0.15),
              "fabric", rot=(0, lean, 0))
            x += w + 0.004
    B((0.12, 0.12, 0.11), (0.3, 0, 2.18), 0.02, col("#C8714E"), "ceramic")
    for i in range(7):
        B((0.05, 0.03, 0.16), (0.3 + math.cos(i) * 0.05, -math.sin(i) * 0.05, 2.3 + rng.uniform(-0.02, 0.06)), 0.015,
          col("#5E8E5A"), "leaf", rot=(rng.uniform(-34, 34), rng.uniform(-34, 34), i * 57))
    mb.pop()
    # big plant by the window
    mb.push(Matrix.Translation((2.05, 1.7, 0)))
    B((0.42, 0.42, 0.42), (0, 0, 0.21), 0.04, col("#D8CDBA"), "ceramic")
    B((0.06, 0.06, 0.6), (0, 0, 0.7), 0.01, col("#5A4632"), "wood")
    voxel_blob(mb, rng, (0, 0, 1.15), (0.42, 0.42, 0.55), 0.11, ["#3F7A4A", "#4E8F55", "#5FA35F", "#356842"], 0.62)
    voxel_blob(mb, rng, (0.18, -0.12, 0.9), (0.22, 0.22, 0.22), 0.1, ["#4E8F55", "#5FA35F"], 0.7)
    mb.pop()
    # wall art frame (print added as an image quad)
    B((0.62, 0.04, 0.8), (-0.85, 2.17, 2.05), 0.01, col("#2A2420"), "wood")
    art = pixel_image("art", 16, art_pixels)
    quad("art", 0.5, 0.68, image_material("art", art, 0.9), Matrix.Translation((-0.85, 2.147, 2.05)) @ eul(90, 0, 0), coll)
    # fairy lights: beads on a sagging string along the back wall
    pts = []
    for i in range(61):
        u = i / 60
        x = -2.6 + u * 5.0
        z = 2.62 - 0.16 * math.sin(math.pi * ((u * 4) % 1))
        pts.append((x, 2.12, z))
        if i % 2 == 0:
            B((0.03, 0.03, 0.03), (x, 2.11, z - 0.025), 0.008, col("#FFC67A"), "bead")
    room = mb.build("room", mats, coll=coll)
    wm = bpy.data.materials.new("wire")
    wm.use_nodes = True
    set_in(_principled(wm.node_tree), "Base Color", col("#222222"))
    curve_from_points("fairy_wire", pts, 0.004, wm, coll)
    # outside: night sky, moon, city towers with lit windows, rain on the glass
    sky_m, nt, bsdf = node_material("night_sky")
    set_in(bsdf, "Base Color", (0, 0, 0, 1))
    tc = nt.nodes.new("ShaderNodeTexCoord")
    sep = nt.nodes.new("ShaderNodeSeparateXYZ")
    nt.links.new(tc.outputs["UV"], sep.inputs[0])
    ramp = nt.nodes.new("ShaderNodeValToRGB")
    ramp.color_ramp.elements[0].color = col("#1A1733")
    ramp.color_ramp.elements[1].color = col("#04060F")
    nt.links.new(sep.outputs["Y"], ramp.inputs[0])
    nt.links.new(ramp.outputs["Color"], bsdf.inputs["Emission Color"])
    set_in(bsdf, "Emission Strength", 1.0)
    quad("night_sky", 16, 9, sky_m, Matrix.Translation((1.05, 9.0, 2.2)) @ eul(90, 0, 0), coll)
    moon_m, nt, bsdf = node_material("moon")
    set_in(bsdf, "Emission Color", col("#FFF4DA"))
    set_in(bsdf, "Emission Strength", 6.0)
    mm = MB()
    mm.box((0.42, 0.05, 0.42), (0.2, 8.8, 3.6), 0.2, col("#FFF4DA"), "m")
    mm.build("moon", {"m": moon_m}, coll=coll)
    city = MB()
    for i in range(18):
        bw, bh = rng.uniform(0.5, 1.1), rng.uniform(1.6, 5.5)
        y = 4.4 + rng.uniform(0, 3.0)
        x = 1.05 - 4.2 + i * 0.5 + rng.uniform(-0.12, 0.12)
        zb = -1.2 - (y - 4.4) * 0.25
        city.box((bw, 0.6, bh), (x, y, zb + bh / 2), 0.0, col("#0D1020"), "tower")
        if rng.random() > 0.6:
            city.box((0.05, 0.05, 0.05), (x, y, zb + bh + 0.03), 0.0, col("#FF4A3A"), "beacon")
    city.build("city", {"tower": city_material(), "beacon": voxel_material("beacon", emit=12.0)}, bevel=False, coll=coll)
    rain_quad(coll)
    return {"room": room, "floor": fo, "window": Vector((1.05, 2.25, 1.6)), "seat": Vector((-0.2, 1.3, 0.0))}


def rug_pixels(rng):
    cols = [(201, 183, 154), (125, 136, 160), (220, 207, 182), (168, 97, 74), (58, 66, 88)]
    noise = [[rng.uniform(-6, 6) for _ in range(32)] for _ in range(32)]

    def fn(x, y):
        b = min(x, 31 - x, y, 31 - y)
        c = cols[0]
        if b == 1:
            c = cols[4]
        elif b == 3:
            c = cols[3]
        elif b > 5 and ((x + y) % 12 == 0 or (x - y + 72) % 12 == 0):
            c = cols[2]
        if 7 < b < 9 and (x + y) % 4 == 0:
            c = cols[1]
        d = noise[x][y]
        return tuple(clamp(v + d, 0, 255) for v in c)
    return fn


def art_pixels(x, y):
    y = 15 - y
    if y > 10:
        return (29, 75, 189) if (x + y) % 5 == 0 else (47, 107, 255)
    if math.hypot(x - 10, y - 6) < 3.2:
        return (255, 180, 92)
    return (233, 223, 201)


def city_material():
    """Dark towers with a grid of windows, a random share of them lit warm or cool (object-space pattern)."""
    m, nt, bsdf = node_material("tower")
    N, L = nt.nodes, nt.links
    set_in(bsdf, "Base Color", col("#0D1020"))
    set_in(bsdf, "Roughness", 0.8)
    rest = N.new("ShaderNodeAttribute")
    rest.attribute_name = "rest"
    sc = N.new("ShaderNodeVectorMath")
    sc.operation = "SCALE"
    sc.inputs["Scale"].default_value = 7.0
    L.new(rest.outputs["Vector"], sc.inputs[0])
    fl = N.new("ShaderNodeVectorMath")
    fl.operation = "FLOOR"
    L.new(sc.outputs["Vector"], fl.inputs[0])
    fr = N.new("ShaderNodeVectorMath")
    fr.operation = "FRACTION"
    L.new(sc.outputs["Vector"], fr.inputs[0])
    sep = N.new("ShaderNodeSeparateXYZ")
    L.new(fr.outputs["Vector"], sep.inputs[0])
    wn = N.new("ShaderNodeTexWhiteNoise")
    wn.noise_dimensions = "3D"
    L.new(fl.outputs["Vector"], wn.inputs["Vector"])
    lit = N.new("ShaderNodeMath")
    lit.operation = "GREATER_THAN"
    lit.inputs[1].default_value = 0.72
    L.new(wn.outputs["Value"], lit.inputs[0])
    # window opening: 0.2..0.8 in x and z of each cell
    masks = []
    for comp in ("X", "Z"):
        a = N.new("ShaderNodeMath")
        a.operation = "GREATER_THAN"
        a.inputs[1].default_value = 0.22
        L.new(sep.outputs[comp], a.inputs[0])
        b = N.new("ShaderNodeMath")
        b.operation = "LESS_THAN"
        b.inputs[1].default_value = 0.78
        L.new(sep.outputs[comp], b.inputs[0])
        mm = N.new("ShaderNodeMath")
        mm.operation = "MULTIPLY"
        L.new(a.outputs[0], mm.inputs[0])
        L.new(b.outputs[0], mm.inputs[1])
        masks.append(mm)
    m1 = N.new("ShaderNodeMath")
    m1.operation = "MULTIPLY"
    L.new(masks[0].outputs[0], m1.inputs[0])
    L.new(masks[1].outputs[0], m1.inputs[1])
    m2 = N.new("ShaderNodeMath")
    m2.operation = "MULTIPLY"
    L.new(m1.outputs[0], m2.inputs[0])
    L.new(lit.outputs[0], m2.inputs[1])
    ramp = N.new("ShaderNodeValToRGB")
    ramp.color_ramp.interpolation = "CONSTANT"
    ramp.color_ramp.elements[0].color = col("#96B9FF")
    ramp.color_ramp.elements[1].position = 0.3
    ramp.color_ramp.elements[1].color = col("#FFC478")
    L.new(wn.outputs["Color"], ramp.inputs[0]) if "Color" in wn.outputs else None
    st = N.new("ShaderNodeMath")
    st.operation = "MULTIPLY"
    st.inputs[1].default_value = 2.5
    L.new(m2.outputs[0], st.inputs[0])
    L.new(ramp.outputs["Color"], bsdf.inputs["Emission Color"])
    L.new(st.outputs[0], bsdf.inputs["Emission Strength"])
    return m


def rain_quad(coll):
    """Rain streaks running down the glass: elongated Voronoi cells scrolled by a keyed offset."""
    m = bpy.data.materials.new("rain")
    m.use_nodes = True
    nt = m.node_tree
    N, L = nt.nodes, nt.links
    for n in list(N):
        if n.type == "BSDF_PRINCIPLED":
            N.remove(n)
    out = [n for n in N if n.type == "OUTPUT_MATERIAL"][0]
    tc = N.new("ShaderNodeTexCoord")
    mp = N.new("ShaderNodeMapping")
    mp.inputs["Scale"].default_value = (40, 3, 1)
    L.new(tc.outputs["UV"], mp.inputs["Vector"])
    vo = N.new("ShaderNodeTexVoronoi")
    vo.feature = "DISTANCE_TO_EDGE" if False else "F1"
    L.new(mp.outputs["Vector"], vo.inputs["Vector"])
    cr = N.new("ShaderNodeMath")
    cr.operation = "LESS_THAN"
    cr.inputs[1].default_value = 0.06
    L.new(vo.outputs["Distance"], cr.inputs[0])
    em = N.new("ShaderNodeEmission")
    em.inputs["Color"].default_value = col("#AFC4FF")
    em.inputs["Strength"].default_value = 0.35
    tr = N.new("ShaderNodeBsdfTransparent")
    mx = N.new("ShaderNodeMixShader")
    L.new(cr.outputs[0], mx.inputs[0])
    L.new(tr.outputs[0], mx.inputs[1])
    L.new(em.outputs[0], mx.inputs[2])
    L.new(mx.outputs[0], out.inputs["Surface"])
    ob = quad("rain", 1.33, 1.33, m, Matrix.Translation((1.05, 2.24, 1.55)) @ eul(90, 0, 0), coll)
    try:
        ob.visible_shadow = False
    except AttributeError:
        pass
    return mp


def curve_from_points(name, pts, radius, mat, coll=None):
    cu = bpy.data.curves.new(name, "CURVE")
    cu.dimensions = "3D"
    cu.bevel_depth = radius
    cu.bevel_resolution = 2
    sp = cu.splines.new("POLY")
    sp.points.add(len(pts) - 1)
    for p, q in zip(sp.points, pts):
        p.co = (q[0], q[1], q[2], 1.0)
    cu.materials.append(mat)
    ob = new_obj(name, cu, coll)
    return ob


def voxel_blob(mb, rng, c, rad3, cube, cols, fill=0.6, mat="leaf"):
    nx, ny, nz = (math.ceil(r / cube) for r in rad3)
    for i in range(-nx, nx + 1):
        for j in range(-ny, ny + 1):
            for k in range(-nz, nz + 1):
                d = (i * cube / rad3[0]) ** 2 + (j * cube / rad3[1]) ** 2 + (k * cube / rad3[2]) ** 2
                if d > 1 or d <= 0.45:
                    continue
                if rng.random() > fill + (1 - d) * 0.5:
                    continue
                shade = rng.uniform(0.85, 1.1) * (0.8 + 0.25 * (k / max(1, nz) + 1) / 2)
                mb.box((cube, cube, cube), (c[0] + i * cube + rng.uniform(-0.01, 0.01), c[1] + j * cube + rng.uniform(-0.01, 0.01),
                                            c[2] + k * cube), cube * 0.12, col(rng.choice(cols), shade), mat)


def build_park(mats, rng, coll=None):
    """Park at golden hour: grass, stone path along X, voxel trees, shrubs, flowers, benches, lamp posts."""
    gm, nt, bsdf = node_material("grass")
    N, L = nt.nodes, nt.links
    tc = N.new("ShaderNodeTexCoord")
    sc = N.new("ShaderNodeVectorMath")
    sc.operation = "SNAP"
    sc.inputs[1].default_value = (0.06, 0.06, 0.06)
    L.new(tc.outputs["Object"], sc.inputs[0])
    wn = N.new("ShaderNodeTexWhiteNoise")
    wn.noise_dimensions = "3D"
    L.new(sc.outputs["Vector"], wn.inputs["Vector"])
    nz = N.new("ShaderNodeTexNoise")
    nz.inputs["Scale"].default_value = 0.35
    L.new(tc.outputs["Object"], nz.inputs["Vector"])
    ramp = N.new("ShaderNodeValToRGB")
    ramp.color_ramp.elements[0].color = col("#4F7A30")
    ramp.color_ramp.elements[1].color = col("#7FA046")
    L.new(nz.outputs["Fac"], ramp.inputs[0])
    mr = N.new("ShaderNodeMapRange")
    mr.inputs["To Min"].default_value = 0.85
    mr.inputs["To Max"].default_value = 1.12
    L.new(wn.outputs["Value"], mr.inputs["Value"])
    vs = N.new("ShaderNodeVectorMath")
    vs.operation = "SCALE"
    L.new(ramp.outputs["Color"], vs.inputs[0])
    L.new(mr.outputs["Result"], vs.inputs["Scale"])
    L.new(vs.outputs["Vector"], bsdf.inputs["Base Color"])
    set_in(bsdf, "Roughness", 0.95)
    set_in(bsdf, "Sheen Weight", 0.4)
    g = MB()
    g.box((200, 200, 0.1), (0, 0, -0.05), 0.0, col("#FFFFFF"), "g")
    g.build("ground", {"g": gm}, bevel=False, coll=coll)
    # path (stone slabs)
    pm, nt, bsdf = node_material("path")
    N, L = nt.nodes, nt.links
    tc = N.new("ShaderNodeTexCoord")
    br = N.new("ShaderNodeTexBrick")
    br.inputs["Color1"].default_value = col("#C4AC86")
    br.inputs["Color2"].default_value = col("#B39C78")
    br.inputs["Mortar"].default_value = col("#7E6E56")
    br.inputs["Scale"].default_value = 1.0
    br.inputs["Mortar Size"].default_value = 0.012
    br.inputs["Brick Width"].default_value = 0.6
    br.inputs["Row Height"].default_value = 0.45
    br.offset = 0.5
    L.new(tc.outputs["Object"], br.inputs["Vector"])
    L.new(br.outputs["Color"], bsdf.inputs["Base Color"])
    set_in(bsdf, "Roughness", 0.85)
    bump = N.new("ShaderNodeBump")
    bump.inputs["Strength"].default_value = 0.4
    L.new(br.outputs["Fac"], bump.inputs["Height"])
    L.new(bump.outputs["Normal"], bsdf.inputs["Normal"])
    p = MB()
    p.box((200, 1.8, 0.04), (0, 0, 0.02), 0.0, col("#FFFFFF"), "p")
    p.build("path", {"p": pm}, bevel=False, coll=coll)
    mb = MB()
    B = mb.box
    for y in (-0.95, 0.95):
        B((200, 0.12, 0.08), (0, y, 0.04), 0.02, col("#9C8F78"), "stone")
    trunk = col("#68492F")
    greens = [["#4E8F55", "#5FA35F", "#3F7A4A", "#6DB06A"], ["#7FA34A", "#93B552", "#6A8F3E"],
              ["#E0A23A", "#D98B2B", "#EDB64F"], ["#C9692E", "#B4572A", "#DA7B3A"]]

    def tree(x, y, s=1.0):
        h = rng.uniform(1.6, 2.4) * s
        B((0.26 * s, 0.26 * s, h), (x, y, h / 2), 0.03, trunk, "wood")
        B((0.5 * s, 0.08 * s, 0.08 * s), (x + 0.2 * s, y, h * 0.7), 0.02, trunk, "wood", rot=(0, -35, 0))
        pal = greens[rng.randint(0, 1)] if rng.random() < 0.62 else greens[rng.randint(2, 3)]
        voxel_blob(mb, rng, (x, y, h + 0.7 * s), (1.1 * s, 1.1 * s, 1.0 * s), 0.24 * s, pal, 0.55)
        if rng.random() < 0.6:
            voxel_blob(mb, rng, (x + rng.uniform(-0.5, 0.5) * s, y + rng.uniform(-0.4, 0.4) * s, h + 1.4 * s),
                       (0.7 * s, 0.7 * s, 0.6 * s), 0.22 * s, pal, 0.6)

    x = -22.0
    while x < 34:
        tree(x + rng.uniform(-0.6, 0.6), rng.uniform(3.2, 5.5), rng.uniform(0.9, 1.25))
        x += rng.uniform(2.6, 4.2)
    x = -22.0
    while x < 34:
        tree(x, rng.uniform(9, 13), rng.uniform(1.2, 1.6))
        x += rng.uniform(5, 8)
    x = -22.0
    while x < 34:
        tree(x, -rng.uniform(5.5, 6.5), rng.uniform(0.9, 1.1))
        x += rng.uniform(7, 11)
    shr = ["#4E8F55", "#5FA35F", "#3F7A4A"]
    x = -22.0
    while x < 34:
        voxel_blob(mb, rng, (x, -rng.uniform(1.6, 2.6), 0.3), (0.5, 0.4, 0.35), 0.17, shr, 0.7)
        x += rng.uniform(1.8, 3.2)
    x = -22.0
    while x < 34:
        voxel_blob(mb, rng, (x, rng.uniform(1.7, 2.3), 0.25), (0.45, 0.35, 0.3), 0.15, shr, 0.7)
        x += rng.uniform(2, 3.5)
    flowers = ["#F4E3C2", "#FFB45C", "#E97F7F", "#F2D16B", "#B9A6E8"]
    for i in range(500):
        y = rng.uniform(1.1, 3.0) * (1 if rng.random() < 0.5 else -1)
        B((0.05, 0.05, 0.05), (rng.uniform(-22, 34), y, 0.04), 0.012, col(rng.choice(flowers)), "matte")
    # grass tufts near the path: small blades of voxel grass break up the flat ground
    for i in range(2500):
        y = rng.uniform(1.0, 4.0) * (1 if rng.random() < 0.5 else -1)
        h = rng.uniform(0.04, 0.1)
        B((0.025, 0.025, h), (rng.uniform(-14, 22), y, h / 2), 0.0, col(rng.choice(["#4F7A30", "#5F8B3A", "#6E9A42"]), rng.uniform(0.85, 1.1)), "leaf")
    bench = col("#8A5A36")
    iron = col("#262A33")
    for x in range(-24, 36, 12):
        bx = x + 6
        B((1.6, 0.42, 0.06), (bx, 1.3, 0.45), 0.015, bench, "wood")
        B((1.6, 0.06, 0.3), (bx, 1.5, 0.72), 0.015, bench, "wood")
        for sx in (-0.7, 0.7):
            B((0.06, 0.4, 0.45), (bx + sx, 1.3, 0.22), 0.01, iron, "metal")
            B((0.06, 0.06, 0.4), (bx + sx, 1.5, 0.62), 0.01, iron, "metal")
    for x in range(-24, 36, 12):
        B((0.12, 0.12, 3.0), (x, 1.25, 1.5), 0.02, iron, "metal")
        B((0.3, 0.3, 0.06), (x, 1.25, 3.05), 0.01, iron, "metal")
        B((0.24, 0.24, 0.32), (x, 1.25, 2.86), 0.03, col("#FFE2B0"), "glow_soft")
    # far city in haze
    for i in range(30):
        bw, bh = rng.uniform(2, 5), rng.uniform(4, 14)
        B((bw, 3, bh), (-40 + i * 4.5 + rng.uniform(-1, 1), 40 + rng.uniform(0, 8), bh / 2), 0.05, col("#8E9AB8"), "matte")
    park = mb.build("park", mats, coll=coll)
    return {"park": park}


def falling_leaves(mats, rng, frames, region, n=60, coll=None):
    """Leaves drifting down through the light (two linear keys each; no sim needed)."""
    cols = ["#E0A23A", "#C9692E", "#EDB64F", "#9DB552"]
    for i in range(n):
        x0 = rng.uniform(region[0], region[1])
        y0 = rng.uniform(region[2], region[3])
        z0 = rng.uniform(2.5, 5.0)
        lm = MB()
        lm.box((0.07, 0.05, 0.005), (0, 0, 0), 0.0, col(rng.choice(cols)), "leaf")
        ob = lm.build(f"leaf{i}", mats, bevel=False, coll=coll)
        t1 = frames
        fall = rng.uniform(0.35, 0.7) * frames / FPS
        ob.location = (x0, y0, z0)
        ob.rotation_euler = (rng.uniform(0, 6), rng.uniform(0, 6), rng.uniform(0, 6))
        ob.keyframe_insert("location", frame=1)
        ob.keyframe_insert("rotation_euler", frame=1)
        ob.location = (x0 + fall * 0.4, y0 + rng.uniform(-0.5, 0.5), max(-0.1, z0 - fall))
        ob.rotation_euler = (ob.rotation_euler[0] + rng.uniform(4, 9), ob.rotation_euler[1] + rng.uniform(3, 7), ob.rotation_euler[2] + 2)
        ob.keyframe_insert("location", frame=t1)
        ob.keyframe_insert("rotation_euler", frame=t1)


# ---------------------------------------------------------------------------------------------------- lights/world
def light(name, kind, energy, color, loc, size=0.05, coll=None, direction=None, spot=None):
    ld = bpy.data.lights.new(name, kind)
    ld.energy = energy
    ld.color = color[:3]
    if kind in ("POINT", "SPOT"):
        ld.shadow_soft_size = size
    elif kind == "AREA":
        ld.size = size
    elif kind == "SUN":
        ld.angle = rad(size)
    if spot:
        ld.spot_size = rad(spot)
        ld.spot_blend = 0.6
    ob = new_obj(name, ld, coll)
    ob.location = loc
    if direction is not None:
        ob.rotation_euler = Vector(direction).to_track_quat("-Z", "Y").to_euler()
    return ob


def kelvin(k):
    """Approximate blackbody colour (linear RGB, normalised)."""
    t = k / 100.0
    if t <= 66:
        r = 255
        g = 99.47 * math.log(t) - 161.12
        b = 0 if t <= 19 else 138.52 * math.log(t - 10) - 305.04
    else:
        r = 329.7 * (t - 60) ** -0.1332
        g = 288.1 * (t - 60) ** -0.0755
        b = 255
    c = [srgb_to_lin(clamp(v, 0, 255) / 255) for v in (r, g, b)]
    mx = max(c)
    return (c[0] / mx, c[1] / mx, c[2] / mx, 1.0)


def world_night(strength=1.0):
    w = bpy.data.worlds.new("night")
    w.use_nodes = True
    bg = w.node_tree.nodes["Background"]
    bg.inputs["Color"].default_value = col("#0A0E1C")
    bg.inputs["Strength"].default_value = 0.08 * strength
    bpy.context.scene.world = w


def world_golden(sun_elev, sun_azim, strength=0.35):
    w = bpy.data.worlds.new("golden")
    w.use_nodes = True
    nt = w.node_tree
    bg = nt.nodes["Background"]
    try:
        sky = nt.nodes.new("ShaderNodeTexSky")
        sky.sky_type = "NISHITA"
        sky.sun_disc = False
        sky.sun_elevation = rad(sun_elev)
        sky.sun_rotation = rad(sun_azim)
        sky.altitude = 0
        sky.air_density = 1.2
        sky.dust_density = 2.5
        nt.links.new(sky.outputs["Color"], bg.inputs["Color"])
    except Exception as e:  # noqa: BLE001
        log("sky texture unavailable:", e)
        bg.inputs["Color"].default_value = col("#F6D3A0")
    bg.inputs["Strength"].default_value = strength
    bpy.context.scene.world = w


def haze_volume(center, size, density, color=(1, 0.93, 0.82, 1), aniso=0.55, coll=None):
    m = bpy.data.materials.new("haze")
    m.use_nodes = True
    nt = m.node_tree
    for n in list(nt.nodes):
        if n.type == "BSDF_PRINCIPLED":
            nt.nodes.remove(n)
    out = [n for n in nt.nodes if n.type == "OUTPUT_MATERIAL"][0]
    v = nt.nodes.new("ShaderNodeVolumePrincipled")
    v.inputs["Density"].default_value = density
    v.inputs["Color"].default_value = color
    v.inputs["Anisotropy"].default_value = aniso
    nt.links.new(v.outputs[0], out.inputs["Volume"])
    mb = MB()
    mb.box(size, center, 0.0, col("#FFFFFF"), "h")
    ob = mb.build("haze", {"h": m}, bevel=False, coll=coll)
    return ob


# ---------------------------------------------------------------------------------------------------- animation
POSES = {
    # hips: [x, up, front] metres (bone-local), rotations in degrees (bone-local XYZ: +X leans/nods forward,
    # +Y turns to the character's left, +Z tilts towards the right shoulder)
    "stand": {"hips_loc": [0, 0, 0], "hips": [0, 0, 0], "spine": [0, 0, 0], "neck": [0, 0, 0], "head": [0, 0, 0]},
    "sit": {"hips_loc": [0, -0.09, 0], "hips": [-6, 0, 0], "spine": [-7, 0, 0], "neck": [8, 0, 0], "head": [14, -6, -3]},
    "sit_nod": {"hips_loc": [0, -0.09, 0], "hips": [-6, 0, 0], "spine": [-5, 0, 0], "neck": [14, 0, 0], "head": [24, -4, 1]},
    "sit_up": {"hips_loc": [0, -0.09, 0], "hips": [-6, 0, 0], "spine": [-4, 0, 0], "neck": [-4, 0, 0], "head": [-8, 4, 0]},
    "walk": {"hips_loc": [0, -0.052, 0], "hips": [0, 0, 0], "spine": [4, 0, 0], "neck": [2, 0, 0], "head": [4, 0, 0]},
}
EXPR = {
    "neutral": {"open": 0.82, "brow": 0, "tilt": 0, "smile": 0.08, "mouth": 0, "gaze": 0},
    "calm": {"open": 0.74, "brow": -0.05, "tilt": 0, "smile": 0.12, "mouth": 0, "gaze": -0.3},
    "absorbed": {"open": 0.64, "brow": 0.05, "tilt": 0.1, "smile": 0.2, "mouth": 0, "gaze": -1},
    "look_up_calm": {"open": 0.86, "brow": 0.15, "tilt": 0, "smile": 0.25, "mouth": 0, "gaze": 0.5},
    "soft_smile": {"open": 0.72, "brow": 0.1, "tilt": 0, "smile": 0.55, "mouth": 0, "gaze": 0},
    "content": {"open": 0.7, "brow": 0.05, "tilt": 0, "smile": 0.85, "mouth": 0.04, "gaze": 0},
    "sleepy": {"open": 0.36, "brow": -0.35, "tilt": -0.4, "smile": 0.04, "mouth": 0, "gaze": -0.4},
    "tired": {"open": 0.55, "brow": -0.2, "tilt": -0.5, "smile": 0.0, "mouth": 0, "gaze": -0.6},
}


def pose_track(keys):
    out = []
    for k in keys:
        p = dict(POSES[k["pose"]])
        for b, v in k.get("add", {}).items():
            p[b] = [a + d for a, d in zip(p.get(b, [0, 0, 0]), v)]
        out.append({"t": k["t"], "v": p, "blend": k.get("blend", 0.0), "ease": k.get("ease", "smooth")})
    return out


def expr_track(keys):
    return [{"t": k["t"], "v": EXPR[k["expr"]], "blend": k.get("blend", 0.0)} for k in keys]


def blink_times(seed, dur):
    r = random.Random(seed * 31 + 5)
    t = r.uniform(0.6, 2.0)
    out = []
    while t < dur:
        out.append(t)
        t += r.uniform(2.6, 5.2)
    return out


class Walk:
    """Walk with planted feet: world-space foot plants, constant body speed, stance 60 %, swing arcs."""

    def __init__(self, start, heading_deg, speed, cadence, phase=0.0):
        self.start = Vector(start)
        h = rad(heading_deg)
        self.fwd = Vector((math.cos(h), math.sin(h), 0))
        self.left = Vector((-math.sin(h), math.cos(h), 0))
        self.v = speed
        self.f = cadence
        self.phase = phase
        self.step = speed / (2 * cadence)
        self.stance = 0.6

    def body(self, t):
        return self.start + self.fwd * (self.v * t)

    TOE, HEEL, SOLE = 0.18, 0.10, 0.077    # toe / heel distance from the ankle along the foot, ankle height

    def pivot_toe(self, ankle, pitch):
        """Ankle position when the foot (planted flat with its ankle at `ankle`) rolls onto its toe by pitch deg."""
        v = self.fwd * self.TOE + Vector((0, 0, -self.SOLE))
        rot = Matrix.Rotation(rad(pitch), 3, self.fwd.cross(Vector((0, 0, 1))))
        return ankle + v - rot @ v

    def min_corner(self, pitch):
        a = rad(pitch)
        toe = -self.SOLE * math.cos(a) + self.TOE * math.sin(a)
        heel = -self.SOLE * math.cos(a) - self.HEEL * math.sin(a)
        return min(toe, heel)

    def foot(self, side, t):
        """World ankle position, pitch (deg, + = toes up) and swing progress for a foot at time t.
        Cycle: flat stance 0-0.42, heel-off rolling over the toe 0.42-0.6, swing 0.6-1 (lift, toes up, land flat)."""
        off = 0.0 if side == "L" else 0.5
        u = self.f * t + self.phase + off
        n = math.floor(u)
        frac = u - n
        lat = self.left * (0.1 if side == "L" else -0.1)
        ahead = self.step * self.stance          # the foot lands this far ahead of the body (at frac 0)

        def plant(k):
            tl = (k - self.phase - off) / self.f
            return self.body(tl) + self.fwd * ahead + lat + Vector((0, 0, self.SOLE))

        if frac < 0.42:
            return plant(n), 0.0, 0.0
        if frac < self.stance:
            pitch = -20 * smooth((frac - 0.42) / (self.stance - 0.42))
            return self.pivot_toe(plant(n), pitch), pitch, 0.0
        w = (frac - self.stance) / (1 - self.stance)
        a = self.pivot_toe(plant(n), -20)
        b = plant(n + 1)
        p = a.lerp(b, smooth(w)) + Vector((0, 0, 0.07 * math.sin(math.pi * w)))
        pitch = lerp(-20, 9, smooth(w / 0.8)) if w < 0.8 else lerp(9, 0, smooth((w - 0.8) / 0.2))
        p.z = max(p.z, -self.min_corner(pitch))
        return p, pitch, w

    def hips_bob(self, t):
        u = self.f * t + self.phase
        return 0.011 * math.cos(4 * math.pi * (u - self.stance / 2))

    def swing_phase(self, t):
        return 2 * math.pi * (self.f * t + self.phase)


def ankle_rot(fwd, pitch_deg=0.0, yaw_deg=0.0):
    f = Vector(fwd)
    f.rotate(Matrix.Rotation(rad(yaw_deg), 3, "Z"))
    up = Vector((0, 0, 1))
    side = f.cross(up)          # points to the foot's right; rotating about it by +pitch lifts the toes
    rot = Matrix.Rotation(rad(pitch_deg), 3, side)
    y = rot @ f
    z = rot @ up
    return frame_axes(y, z)


# ---------------------------------------------------------------------------------------------------- the build
class Film:
    def __init__(self, shot, seg, args):
        self.shot = shot
        self.seg = seg
        self.args = args
        self.dur = seg["end"] - seg["start"] + seg.get("tail", 0.0)
        self.n = int(round(self.dur * FPS))
        self.rng = random.Random(shot.get("seed", 11))
        self.noise = {k: Noise1(i * 101 + shot.get("seed", 11)) for i, k in enumerate(
            ("head", "sway", "cam_r", "cam_p", "cam_y", "cam_x", "cam_z", "cable", "hand"))}

    def run(self):
        sc = bpy.context.scene
        for ob in list(bpy.data.objects):
            bpy.data.objects.remove(ob)
        sc.render.fps = FPS
        sc.frame_start = 1
        sc.frame_end = self.n
        bpy.context.preferences.edit.keyframe_new_interpolation_type = "LINEAR"
        self.mats = std_materials()
        seg = self.seg
        self.set = seg["set"]
        if self.set == "room":
            self.env = build_room(self.mats, random.Random(11))
            world_night()
            self.room_lights()
        else:
            self.env = build_park(self.mats, random.Random(21))
            self.park_lights()
        log("set built")
        self.ch = Character(self.mats, seed=7)
        ch = seg["character"]
        self.item = None
        if ch.get("item") == "book":
            self.item = build_book(self.mats)
            self.item["kind"] = "book"
        elif ch.get("item") == "phone":
            scr = seg.get("screen", {})
            self.item = build_phone(self.mats, self.args.screen, round(scr.get("in", 0.0) * FPS))
            self.item["kind"] = "phone"
        if ch.get("pov"):
            for ob in (self.ch.head, self.ch.hp):
                ob.visible_camera = False
        self.cable = None
        if ch.get("cable"):
            cm = bpy.data.materials.new("cable_dark")     # curves carry no colour attribute: flat material
            cm.use_nodes = True
            b = _principled(cm.node_tree)
            set_in(b, "Base Color", col("#1B1E2A"))
            set_in(b, "Roughness", 0.4)
            self.cable = curve_from_points("cable", [(0, 0, i * 0.01) for i in range(24)], 0.0032, cm, None)
            self.cable.data.bevel_resolution = 3
        self.camera_setup()
        log("character and props built")
        self.bake_body()
        log("pass A (body) keyed")
        self.bake_items()
        log("pass B1 (held items) keyed")
        self.bake_hands_camera()
        log("pass B2 (hands, headphones, cable, camera) keyed")
        if seg.get("leaves"):
            falling_leaves(self.mats, self.rng, self.n, seg["leaves"])
        self.render_settings()
        return self

    # -------------------------------------------------------------------------------------------- lights
    def room_lights(self):
        L = self.seg.get("lights", {})
        warm = kelvin(2600)
        self.lamp = light("lamp", "POINT", L.get("lamp", 45), warm, (-1.05, 1.46, 1.52), size=0.05)
        # lamp shade: an open cone of translucent fabric around the bulb
        m = bpy.data.materials.new("shade")
        m.use_nodes = True
        nt = m.node_tree
        b = _principled(nt)
        set_in(b, "Base Color", col("#F4E3C2"))
        set_in(b, "Roughness", 0.9)
        set_in(b, "Transmission Weight", 0.0)
        tr = nt.nodes.new("ShaderNodeBsdfTranslucent")
        tr.inputs["Color"].default_value = col("#FFD9A0")
        mx = nt.nodes.new("ShaderNodeMixShader")
        mx.inputs[0].default_value = 0.55
        out = [n for n in nt.nodes if n.type == "OUTPUT_MATERIAL"][0]
        nt.links.new(b.outputs[0], mx.inputs[1])
        nt.links.new(tr.outputs[0], mx.inputs[2])
        nt.links.new(mx.outputs[0], out.inputs["Surface"])
        bpy.ops.mesh.primitive_cone_add(vertices=8, radius1=0.25, radius2=0.16, depth=0.26, end_fill_type="NOTHING",
                                        location=(-1.05, 1.46, 1.55))
        shade = bpy.context.active_object
        shade.name = "lamp_shade"
        shade.data.materials.append(m)
        bulb = MB()
        bulb.box((0.07, 0.07, 0.09), (-1.05, 1.46, 1.5), 0.03, col("#FFD9A0"), "b")
        bm = voxel_material("bulb", emit=25.0)
        bulb.build("bulb", {"b": bm})
        for i, x in enumerate((-2.1, -0.9, 0.4, 1.7)):
            light(f"fairy{i}", "POINT", L.get("fairy", 3.0), kelvin(2400), (x, 2.0, 2.52), size=0.25)
        moon_dir = Vector((-2.1, -4.9, -3.6)).normalized()
        light("moon", "SUN", L.get("moon", 0.35), (0.55, 0.66, 1.0, 1), (1, 3, 4), size=1.0, direction=moon_dir)
        light("fill", "AREA", L.get("fill", 18), kelvin(3000), (0.3, -1.5, 2.6), size=2.0,
              direction=Vector((-0.3, 1.0, -0.6)))
        light("rim", "SPOT", L.get("rim", 25), (0.45, 0.58, 1.0, 1), (1.3, 2.0, 2.2), size=0.2,
              direction=Vector((-1.5, -0.7, -1.3)), spot=40)
        if L.get("haze", 0) > 0:
            haze_volume((0, 0.5, 1.5), (6, 6, 3), L["haze"], color=(0.9, 0.92, 1.0, 1), aniso=0.3)

    def park_lights(self):
        L = self.seg.get("lights", {})
        elev, azim = L.get("sun_elev", 11), L.get("sun_azim", 150)
        d = Vector((math.cos(rad(elev)) * math.cos(rad(azim)), math.cos(rad(elev)) * math.sin(rad(azim)), math.sin(rad(elev))))
        self.sun_dir = d
        light("sun", "SUN", L.get("sun", 4.5), kelvin(L.get("sun_k", 3300)), (0, 0, 10), size=1.2, direction=-d)
        world_golden(elev, azim - 90, L.get("sky", 0.3))
        if L.get("haze", 0) > 0:
            haze_volume((6, 4, 7), (60, 44, 14), L["haze"], color=(1.0, 0.92, 0.8, 1), aniso=0.6)

    # -------------------------------------------------------------------------------------------- timeline helpers
    def T(self, f):
        return (f - 1) / FPS

    def root_matrix(self, t):
        ch = self.seg["character"]
        if ch.get("walk"):
            w = self.walk
            p = w.body(t)
            return Matrix.Translation(p) @ Matrix.Rotation(rad(ch["walk"]["heading"] + 90), 4, "Z")
        base = Vector(ch.get("root", [-0.2, 1.3, 0.0]))
        return Matrix.Translation(base) @ Matrix.Rotation(rad(ch.get("yaw", 0)), 4, "Z")

    # -------------------------------------------------------------------------------------------- pass A
    def bake_body(self):
        ch = self.seg["character"]
        C = self.ch
        arm = C.arm
        pb = arm.pose.bones
        if ch.get("walk"):
            wk = ch["walk"]
            start = Vector(wk["start"])
            self.walk = Walk(start, wk["heading"], wk.get("speed", 0.62), wk.get("cadence", 0.9), wk.get("phase", 0.0))
        poses = pose_track(ch["poses"])
        exprs = expr_track(ch["expressions"])
        blinks = blink_times(ch.get("blink_seed", 7) + int(self.seg["start"] * 10), self.dur + 1)
        look_keys = ch.get("look_w", [{"t": 0, "v": 0.0}])
        for f in range(1, self.n + 1):
            t = self.T(f)
            R = self.root_matrix(t)
            key_matrix(arm, R, f)
            p = track(poses, t)
            hl = list(p["hips_loc"])
            hips = list(p["hips"])
            spine = list(p["spine"])
            neck = list(p["neck"])
            head = list(p["head"])
            life = ch.get("life", 1.0)
            br = math.sin(2 * math.pi * 0.22 * t)
            spine[0] += 0.9 * br
            head[0] += 1.6 * life * self.noise["head"].fbm(t + 17.3, 0.6) - 0.5 * br
            head[1] += 3.0 * life * self.noise["head"].fbm(t, 0.5)
            head[2] += 1.4 * life * self.noise["sway"].fbm(t + 3.1, 0.4)
            spine[2] += 0.8 * life * self.noise["sway"].fbm(t, 0.3)
            armL = [0.0, 0.0, 0.0]
            foreL = [0.0, 0.0, 0.0]
            if ch.get("walk"):
                w = self.walk
                ph = w.swing_phase(t)
                hl[1] += w.hips_bob(t)
                hips[1] += -6 * math.cos(ph)
                hips[2] += 2.0 * math.sin(ph)
                spine[1] += 7 * math.cos(ph)
                spine[2] += -1.5 * math.sin(ph)
                head[1] += -3 * math.cos(ph)
                armL = [22 * math.cos(ph), 0, -7]
                foreL = [-18 - 10 * max(0.0, -math.cos(ph)), 0, 0]
            for bn, v in (("hips", hips), ("spine", spine), ("neck", neck), ("head", head)):
                pb[bn].rotation_euler = [rad(a) for a in v]
                pb[bn].keyframe_insert("rotation_euler", frame=f)
            pb["hips"].location = hl
            pb["hips"].keyframe_insert("location", frame=f)
            pb["upper_arm.L"].rotation_euler = [rad(a) for a in armL]
            pb["forearm.L"].rotation_euler = [rad(a) for a in foreL]
            pb["upper_arm.L"].keyframe_insert("rotation_euler", frame=f)
            pb["forearm.L"].keyframe_insert("rotation_euler", frame=f)
            # face
            e = track(exprs, t)
            blink = 0.0
            for bt in blinks:
                d = abs(t - bt)
                if d < 0.1:
                    blink = max(blink, 1 - d / 0.1)
            open_ = max(0.07, e["open"] * (1 - blink))
            for s, sx in (("L", 1), ("R", -1)):
                eb = pb["eye." + s]
                eb.scale = (1.0, open_, 1.0)
                eb.location = (0, 0.016 * HEAD_S * e["gaze"], 0)
                eb.keyframe_insert("scale", frame=f)
                eb.keyframe_insert("location", frame=f)
                bb = pb["brow." + s]
                bb.location = (0, 0.03 * HEAD_S * e["brow"] - (0.018 if open_ < 0.5 else 0.0), 0)
                bb.rotation_euler = (0, 0, rad(14 * e["tilt"] * (1.0 if s == "L" else -1.0)))
                bb.keyframe_insert("location", frame=f)
                bb.keyframe_insert("rotation_euler", frame=f)
                mc = pb["mouth_corner." + s]
                k = smooth((e["smile"] - 0.1) / 0.25)
                mc.scale = (k + 1e-3, k + 1e-3, k + 1e-3)
                mc.location = (0.012 * e["smile"] * sx, 0.012 * e["smile"], 0)
                mc.keyframe_insert("scale", frame=f)
                mc.keyframe_insert("location", frame=f)
            mo = pb["mouth"]
            mo.scale = (1 + 0.4 * e["smile"], 1 + 2.4 * e["mouth"], 1)
            mo.location = (0, -0.006 * e["smile"] - 0.012 * e["mouth"], 0)
            mo.keyframe_insert("scale", frame=f)
            mo.keyframe_insert("location", frame=f)
            # look-at weights and IK switches
            lw = track(look_keys, t, 0.0)
            for b, w in (("neck", 0.35), ("head", 0.7)):
                key_value(C.con["look." + b], "influence", clamp(lw * w), f)
            armL_ik = track(ch.get("ik_left", [{"t": 0, "v": 1.0}]), t, 1.0)
            key_value(C.con["forearm.L"], "influence", armL_ik, f)
            key_value(C.con["hand.L"], "influence", armL_ik, f)
            # feet and knee poles
            if ch.get("walk"):
                w = self.walk
                for s in ("L", "R"):
                    p3, pitch, sw = w.foot(s, t)
                    key_matrix(C.tg["ik_foot." + s], compose(p3, ankle_rot(w.fwd, pitch)), f)
                    key_matrix(C.tg["pole_knee." + s], Matrix.Translation(w.body(t) + w.fwd * 1.2 + w.left * (0.12 if s == "L" else -0.12) + Vector((0, 0, 0.4))), f)
            else:
                for s, sx in (("L", 1), ("R", -1)):
                    fp = ch.get("feet", {}).get(s, [0.14 * sx, -0.42, 0.077])
                    yaw = ch.get("feet_yaw", {}).get(s, -8 * sx)
                    M = R @ compose(Vector(fp), ankle_rot((0, -1, 0), 0, yaw))
                    key_matrix(C.tg["ik_foot." + s], M, f)
                    key_matrix(C.tg["pole_knee." + s], R @ Matrix.Translation((0.2 * sx, -1.2, 0.55)), f)
        # screen brightness keys
        if self.item and self.item.get("kind") == "phone":
            on = self.seg.get("screen", {}).get("on", [{"t": 0, "v": 1.0}])
            base = self.seg.get("screen", {}).get("bright", 1.6)
            glow = self.seg.get("lights", {}).get("screen_glow", 2.0)
            for f in range(1, self.n + 1):
                v = track(on, self.T(f), 1.0)
                key_value(self.item["emission"], "default_value", base * v, f)
                key_value(self.item["light"], "energy", glow * v, f)

    # -------------------------------------------------------------------------------------------- pass B1
    def hold_matrix(self, t, spineM, R):
        ch = self.seg["character"]
        hk = ch.get("hold", [])
        h = track([{"t": k["t"], "v": {"pos": k["pos"], "aim": k.get("aim", [0, 0.62, 0.2]), "roll": [k.get("roll", 0.0)],
                                       "frame": [1.0 if k.get("frame", "spine") == "spine" else 0.0]},
                    "blend": k.get("blend", 0.0)} for k in hk], t)
        if h is None:
            return None
        bob = Vector((self.noise["hand"].fbm(t, 0.35) * 0.006, self.noise["hand"].fbm(t + 9, 0.3) * 0.006, 0))
        if ch.get("walk"):
            ph = self.walk.swing_phase(t)
            bob += Vector((0, 0.008 * math.cos(2 * ph), 0))

        def place(frameM):
            pos = Vector(h["pos"]) + bob
            aim = Vector(h["aim"])
            n = (aim - pos).normalized()
            rot = frame_axes(Vector((0, 0.35, 1.0)), n)            # top edge leans away from the body
            rot = rot @ Matrix.Rotation(rad(h["roll"][0]), 3, "Z")
            return frameM @ compose(pos, rot)

        # spine frame: spine bone matrix (X left, Y up, Z front); root frame: hold given in root space with
        # the same axis convention (X left, Y up, Z front)
        rootF = R @ Matrix(((1, 0, 0, 0), (0, 0, -1, 0), (0, 1, 0, 0), (0, 0, 0, 1)))
        a = place(spineM)
        if h["frame"][0] >= 0.999:
            return a
        b = place(rootF)
        return blend_matrix(b, a, h["frame"][0])

    def bake_items(self):
        sc = bpy.context.scene
        C = self.ch
        for f in range(1, self.n + 1):
            sc.frame_set(f)
            t = self.T(f)
            dg = bpy.context.evaluated_depsgraph_get()
            spineM = C.world_bone("spine", dg)
            R = self.root_matrix(t)
            if self.item:
                M = self.hold_matrix(t, spineM, R)
                key_matrix(self.item["obj"], M, f)
                look = M @ self.item["center"]
            else:
                look = spineM @ Vector((0, 0.5, 1.5))
            key_matrix(C.look, Matrix.Translation(look), f)

    # -------------------------------------------------------------------------------------------- pass B2
    def hp_matrix(self, t, dg):
        """Headphones: blend between resting on the neck and on the ears (0 = neck, 1 = ears)."""
        C = self.ch
        ch = self.seg["character"]
        w = track(ch.get("headphones", [{"t": 0, "v": 0.0}]), t, 0.0)
        head = C.world_bone("head", dg)
        neck = C.world_bone("neck", dg)
        hpc = ch.get("hp_neck", {"pos": [0, 0.07, -0.03], "rot": -165})
        ears = head @ compose((0, 0.25 * HEAD_S, -0.01 * HEAD_S), Matrix.Rotation(rad(-90), 3, "X"))
        rest = neck @ compose(Vector(hpc["pos"]), Matrix.Rotation(rad(hpc["rot"]), 3, "X"))
        if w <= 0:
            return rest, w
        if w >= 1:
            return ears, w
        e = smooth(w)
        M = blend_matrix(rest, ears, e)
        # lift outward on the way so the cups clear the jaw
        M = Matrix.Translation(head.to_3x3() @ Vector((0, 0.05 * math.sin(math.pi * e), -0.03 * math.sin(math.pi * e)))) @ M
        return M, w

    def bake_hands_camera(self):
        sc = bpy.context.scene
        C = self.ch
        ch = self.seg["character"]
        self.metrics = []
        hand_keys = ch.get("hands", {})
        cam_rows = []
        for f in range(1, self.n + 1):
            sc.frame_set(f)
            t = self.T(f)
            dg = bpy.context.evaluated_depsgraph_get()
            R = self.root_matrix(t)
            spineM = C.world_bone("spine", dg)
            hpM, hpw = self.hp_matrix(t, dg)
            key_matrix(C.hp, hpM, f)
            itemM = self.item["obj"].evaluated_get(dg).matrix_world.copy() if self.item else None
            # hand targets: weighted blend of named grips
            for s, sx in (("L", 1), ("R", -1)):
                keys = hand_keys.get(s, [{"t": 0, "v": "rest"}])
                cand = {}
                if itemM is not None:
                    cand["item"] = grip_matrix(itemM, self.item, "R" if s == "R" else "L")
                cupM = hpM @ Matrix.Translation(((0.335 + 0.14) * sx * HEAD_S, 0.0, -0.01))
                # hand on the cup: palm on the outer face, fingers up along the cup, thumb forward
                cup_rot = hpM.to_3x3().normalized() @ Matrix(((1, 0, 0), (0, 0, 1), (0, -1, 0))).transposed()
                cand["cup"] = compose(cupM.translation, cup_rot)
                rest = ch.get("rest", {}).get(s)
                if rest:
                    cand["rest"] = R @ compose(Vector(rest["pos"]), frame_axes(Vector(rest["y"]), Vector(rest["z"])))
                else:
                    cand["rest"] = R @ compose(Vector((0.4 * sx, -0.08, 0.7)), frame_axes(Vector((0, -1, 0)), Vector((sx, 0, 0))))
                seq = sorted(keys, key=lambda k: k["t"])
                cur = seq[0]["v"]
                M = cand.get(cur, cand["rest"])
                for k in seq:
                    if t < k["t"]:
                        break
                    prev = cur
                    cur = k["v"]
                    b = k.get("blend", 0.4)
                    A = cand.get(prev, cand["rest"])
                    Bm = cand.get(cur, cand["rest"])
                    u = smooth((t - k["t"]) / b) if b > 0 else 1.0
                    M = blend_matrix(A, Bm, u)
                    if 0 < u < 1:
                        lift = math.sin(math.pi * u) * 0.04
                        M = Matrix.Translation(Vector((0, 0, lift))) @ M
                tgt = hand_target_from_center(M)
                # keep the target within reach of the shoulder (no stretched or popping arms)
                sh = C.world_bone("upper_arm." + s, dg).translation
                d = tgt.translation - sh
                reach = 0.235 + 0.215 - 0.01
                if d.length > reach:
                    tgt.translation = sh + d.normalized() * reach
                key_matrix(C.tg["ik_hand." + s], tgt, f)
                pole = spineM @ Vector((0.75 * sx, -0.55, -0.55))
                key_matrix(C.tg["pole_elbow." + s], Matrix.Translation(pole), f)
            if self.cable is not None:
                self.key_cable(f, t, hpM, spineM, itemM)
            cam_rows.append(self.camera_frame(f, t, dg, spineM, itemM))
        self.cam_rows = cam_rows

    def key_cable(self, f, t, hpM, spineM, itemM):
        jack = hpM @ self.ch.hp_jack
        down = hpM.to_3x3() @ Vector((0, 0, -1))
        pts = [jack, jack + down * 0.06 + spineM.to_3x3() @ Vector((0, 0, 0.03))]
        for p in ((0.09, 0.47, 0.17), (0.06, 0.32, 0.185), (0.02, 0.17, 0.2)):
            pts.append(spineM @ Vector(p))
        if itemM is not None:
            pj = itemM @ self.item["jack"]
            pdir = itemM.to_3x3() @ Vector((0, -1, 0))
            pts.append(pj + pdir * 0.07 + Vector((0, 0, -0.03)))
            pts.append(pj)
        # Catmull-Rom through the guide points, resampled to the curve's point count, with a gentle sway
        n = len(self.cable.data.splines[0].points)
        samples = catmull(pts, n)
        nz = self.noise["cable"]
        sp = self.cable.data.splines[0]
        for i, p in enumerate(samples):
            env = math.sin(math.pi * i / (n - 1))
            sway = Vector((nz.fbm(t * 1.3 + i * 0.21, 0.8), nz.fbm(t * 1.1 + i * 0.17 + 40, 0.8), 0)) * 0.012 * env
            q = p + sway
            sp.points[i].co = (q.x, q.y, q.z, 1.0)
            sp.points[i].keyframe_insert("co", frame=f)

    # -------------------------------------------------------------------------------------------- camera
    def camera_setup(self):
        cd = bpy.data.cameras.new("cam")
        cam = new_obj("cam", cd)
        cam.rotation_mode = "QUATERNION"
        cd.sensor_fit = "VERTICAL"
        cd.sensor_height = 24.0
        cd.sensor_width = 24.0 * 16 / 9
        cd.dof.use_dof = True
        cd.dof.aperture_fstop = self.seg["camera"].get("fstop", 2.8)
        cd.dof.aperture_blades = 7
        cd.dof.aperture_rotation = rad(10)
        cd.clip_start = 0.02
        cd.clip_end = 300
        bpy.context.scene.camera = cam
        self.cam = cam

    def named_point(self, name, dg, spineM, itemM, t):
        C = self.ch
        if isinstance(name, (list, tuple)):
            return Vector(name)
        if isinstance(name, (int, float)):
            return None
        head = C.world_bone("head", dg)
        if name == "face":
            return head @ Vector((0, 0.215, 0.18))
        if name == "eyes":
            return head @ Vector((0, 0.205, 0.235))
        if name == "pov":
            return head @ Vector((0, 0.2, 0.27))
        if name == "chest":
            return spineM @ Vector((0, 0.3, 0.15))
        if name in ("phone", "book", "item") and itemM is not None:
            return itemM @ self.item["center"]
        if name == "window":
            return self.env.get("window", Vector((1.05, 2.25, 1.6)))
        if name == "root":
            return self.root_matrix(t).translation.copy()
        raise ValueError(f"unknown point {name}")

    def camera_frame(self, f, t, dg, spineM, itemM):
        cam = self.seg["camera"]
        keys = sorted(cam["keys"], key=lambda k: k["t"])

        def resolve(k):
            look = self.named_point(k.get("look", "face"), dg, spineM, itemM, t)
            look = look + Vector(k.get("aim", (0, 0, 0)))
            if "orbit" in k:
                yaw, pitch, dist = k["orbit"]
                around = self.named_point(k.get("around", k.get("look", "face")), dg, spineM, itemM, t)
                R = self.root_matrix(t).to_3x3()
                fwd = R @ Vector((0, -1, 0))
                left = R @ Vector((1, 0, 0))
                fwd.z = 0
                left.z = 0
                fwd.normalize()
                left.normalize()
                d = (fwd * math.cos(rad(yaw)) + left * math.sin(rad(yaw))) * math.cos(rad(pitch)) + Vector((0, 0, math.sin(rad(pitch))))
                pos = around + d * dist
            else:
                pos = Vector(k["pos"])
                if k.get("rel") == "root":
                    pos = self.root_matrix(t).translation + pos
                elif k.get("rel") == "pov":
                    pos = self.named_point("pov", dg, spineM, itemM, t) + pos
            if k.get("rel_look") == "root":
                look = self.root_matrix(t).translation + Vector(k["look"])
            return pos, look, k.get("lens", 40.0)

        pos, look, lens = resolve(keys[0])
        for k in keys[1:]:
            if t < k["t"]:
                pk = keys[keys.index(k) - 1]
                u = (t - pk["t"]) / max(1e-6, k["t"] - pk["t"])
                e = EASE[k.get("ease", "inout")](u)
                p2, l2, n2 = resolve(k)
                pos, look, lens = pos.lerp(p2, e), look.lerp(l2, e), lerp(lens, n2, e)
                break
            pos, look, lens = resolve(k)
        # handheld: slow rotational drift plus a little positional float
        hh = cam.get("handheld", 0.3)
        nz = self.noise
        rot = frame_axes(Vector((0, 0, 1)), -(look - pos).normalized())
        dr = Matrix.Rotation(rad(hh * 0.9 * nz["cam_r"].fbm(t, 0.35)), 3, "X") @ Matrix.Rotation(rad(hh * 1.2 * nz["cam_y"].fbm(t, 0.3)), 3, "Y") @ Matrix.Rotation(rad(hh * 0.5 * nz["cam_p"].fbm(t, 0.25)), 3, "Z")
        # camera looks down its local -Z; frame_axes gives Z = -view direction, Y = up
        rot = rot @ dr
        pos = pos + Vector((nz["cam_x"].fbm(t, 0.2), 0, nz["cam_z"].fbm(t, 0.22))) * 0.004 * hh
        M = compose(pos, rot)
        key_matrix(self.cam, M, f)
        self.cam.data.lens = lens
        self.cam.data.keyframe_insert("lens", frame=f)
        fk = cam.get("focus", [{"t": 0, "on": "face"}])
        fd = None
        vals = []
        for k in sorted(fk, key=lambda k: k["t"]):
            on = k["on"]
            if isinstance(on, (int, float)):
                d = float(on)
            else:
                p = self.named_point(on, dg, spineM, itemM, t)
                d = (p - pos).dot(-(rot @ Vector((0, 0, 1))))
            vals.append({"t": k["t"], "v": max(0.05, d), "blend": k.get("blend", 0.0)})
        fd = track(vals, t, 2.0)
        self.cam.data.dof.focus_distance = fd
        self.cam.data.dof.keyframe_insert("focus_distance", frame=f)
        fs = cam.get("fstop_keys")
        if fs:
            self.cam.data.dof.aperture_fstop = track(fs, t, cam.get("fstop", 2.8))
            self.cam.data.dof.keyframe_insert("aperture_fstop", frame=f)
        return {"f": f, "focus": round(fd, 3), "lens": round(lens, 1)}

    # -------------------------------------------------------------------------------------------- render
    def render_settings(self):
        a = self.args
        sc = bpy.context.scene
        r = sc.render
        r.resolution_x, r.resolution_y = self.shot.get("resolution", [1920, 1080])
        r.resolution_percentage = a.percent
        r.use_motion_blur = True
        r.motion_blur_shutter = self.seg.get("shutter", 0.5)
        r.image_settings.file_format = "PNG"
        r.image_settings.color_mode = "RGB"
        r.image_settings.color_depth = "8"
        r.film_transparent = False
        try:
            sc.view_settings.view_transform = "AgX"
            sc.view_settings.look = self.seg.get("look", "AgX - Medium High Contrast")
        except TypeError as e:
            log("colour management:", e)
        sc.view_settings.exposure = self.seg.get("exposure", 0.0)
        if a.engine == "cycles":
            r.engine = "CYCLES"
            cy = sc.cycles
            cy.device = "GPU"
            cy.samples = a.samples or self.seg.get("samples", 128)
            cy.use_adaptive_sampling = True
            cy.adaptive_threshold = 0.025
            cy.use_denoising = True
            try:
                cy.denoiser = "OPTIX" if a.device == "OPTIX" else "OPENIMAGEDENOISE"
            except TypeError:
                cy.denoiser = "OPENIMAGEDENOISE"
            cy.denoising_input_passes = "RGB_ALBEDO_NORMAL"
            cy.max_bounces = 8
            cy.diffuse_bounces = 3
            cy.glossy_bounces = 3
            cy.transmission_bounces = 6
            cy.volume_bounces = 1
            cy.transparent_max_bounces = 8
            cy.caustics_reflective = False
            cy.caustics_refractive = False
            cy.sample_clamp_indirect = 6.0
            cy.volume_step_rate = 2.0
            cy.volume_max_steps = 256
            r.use_persistent_data = True
            setup_gpu(a.device)
        else:
            for name in ("BLENDER_EEVEE_NEXT", "BLENDER_EEVEE"):
                try:
                    r.engine = name
                    break
                except TypeError:
                    continue
            ee = sc.eevee
            ee.taa_render_samples = a.samples or 64
            for k, v in (("use_raytracing", True), ("use_shadows", True), ("use_volumetric_shadows", True),
                         ("volumetric_tile_size", "4"), ("use_gtao", True)):
                try:
                    setattr(ee, k, v)
                except (AttributeError, TypeError):
                    pass
        self.compositor()

    def compositor(self):
        sc = bpy.context.scene
        try:
            sc.use_nodes = True
            nt = sc.node_tree
            for n in list(nt.nodes):
                nt.nodes.remove(n)
            rl = nt.nodes.new("CompositorNodeRLayers")
            comp = nt.nodes.new("CompositorNodeComposite")
            gl = nt.nodes.new("CompositorNodeGlare")
            gl.glare_type = "FOG_GLOW"
            for k, v in (("quality", "HIGH"), ("mix", -0.82), ("threshold", 1.2), ("size", 8)):
                try:
                    setattr(gl, k, v)
                except (AttributeError, TypeError):
                    pass
            for k, v in (("Threshold", 1.2), ("Highlights Threshold", 1.2), ("Strength", 0.18), ("Size", 0.6)):
                if k in gl.inputs:
                    try:
                        gl.inputs[k].default_value = v
                    except (TypeError, AttributeError):
                        pass
            nt.links.new(rl.outputs["Image"], gl.inputs["Image"])
            nt.links.new(gl.outputs["Image"], comp.inputs["Image"])
        except Exception as e:  # noqa: BLE001
            log("compositor disabled:", e)
            sc.use_nodes = False

    # -------------------------------------------------------------------------------------------- QA metrics
    def qa(self, frames):
        sc = bpy.context.scene
        C = self.ch
        out = []
        for f in frames:
            sc.frame_set(f)
            dg = bpy.context.evaluated_depsgraph_get()
            row = {"f": f}
            for s in ("L", "R"):
                wr = C.world_bone("hand." + s, dg).translation
                tg = C.tg["ik_hand." + s].evaluated_get(dg).matrix_world.translation
                row["hand_err_" + s] = round((wr - tg).length, 4)
                an = C.world_bone("foot." + s, dg)
                tf = C.tg["ik_foot." + s].evaluated_get(dg).matrix_world.translation
                row["foot_err_" + s] = round((an.translation - tf).length, 4)
                lo = min((an @ Vector((x, y, -0.077))).z for x in (-0.095, 0.095) for y in (-0.10, 0.18))
                row["sole_" + s] = round(lo, 4)
                for b in ("shin", "forearm"):
                    m1 = C.world_bone(("thigh." if b == "shin" else "upper_arm.") + s, dg).to_3x3()
                    m2 = C.world_bone(b + "." + s, dg).to_3x3()
                    ang = math.degrees(m1.col[1].angle(m2.col[1]))
                    row[f"{b}_{s}_deg"] = round(ang, 1)
            out.append(row)
        return out


def lens_to_fov(lens):
    return math.degrees(2 * math.atan(12.0 / lens))


def catmull(pts, n):
    pts = [Vector(p) for p in pts]
    seg_len = [(pts[i + 1] - pts[i]).length for i in range(len(pts) - 1)]
    total = sum(seg_len)
    out = []
    for j in range(n):
        s = total * j / (n - 1)
        i = 0
        while i < len(seg_len) - 1 and s > seg_len[i]:
            s -= seg_len[i]
            i += 1
        u = s / max(1e-6, seg_len[i])
        p0 = pts[max(0, i - 1)]
        p1, p2 = pts[i], pts[i + 1]
        p3 = pts[min(len(pts) - 1, i + 2)]
        u2, u3 = u * u, u * u * u
        out.append(0.5 * ((2 * p1) + (-p0 + p2) * u + (2 * p0 - 5 * p1 + 4 * p2 - p3) * u2 + (-p0 + 3 * p1 - 3 * p2 + p3) * u3))
    return out


def setup_gpu(device):
    prefs = bpy.context.preferences.addons["cycles"].preferences
    for kind in ([device] if device else []) + ["OPTIX", "CUDA"]:
        try:
            prefs.compute_device_type = kind
        except TypeError:
            continue
        try:
            prefs.refresh_devices()
        except AttributeError:
            prefs.get_devices()
        devs = [d for d in prefs.devices if d.type == kind]
        if devs:
            for d in prefs.devices:
                d.use = d.type == kind
            log("GPU:", kind, [d.name for d in devs])
            return kind
    log("WARNING: no GPU device, rendering on CPU")
    bpy.context.scene.cycles.device = "CPU"
    return "CPU"


# ---------------------------------------------------------------------------------------------------- main
def parse_args():
    import argparse
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    ap = argparse.ArgumentParser()
    ap.add_argument("--shot", required=True)
    ap.add_argument("--segment", required=True)
    ap.add_argument("--out", default="out")
    ap.add_argument("--frames", default="")
    ap.add_argument("--anim", action="store_true")
    ap.add_argument("--range", default="")
    ap.add_argument("--engine", default="cycles")
    ap.add_argument("--device", default="OPTIX")
    ap.add_argument("--samples", type=int, default=0)
    ap.add_argument("--percent", type=int, default=100)
    ap.add_argument("--screen", default="")
    ap.add_argument("--save", default="")
    ap.add_argument("--metrics", action="store_true")
    ap.add_argument("--prefix", default="")
    return ap.parse_args(argv)


def main():
    a = parse_args()
    shot = json.load(open(a.shot))
    seg = next(s for s in shot["segments"] if s["id"] == a.segment)
    os.makedirs(a.out, exist_ok=True)
    film = Film(shot, seg, a).run()
    log("build done; frames", film.n)
    if a.save:
        bpy.ops.wm.save_as_mainfile(filepath=a.save)
    sc = bpy.context.scene
    timings = []
    if a.metrics:
        frames = list(range(1, film.n + 1, 3))
        json.dump({"qa": film.qa(frames), "camera": film.cam_rows[::3], "poles": film.ch.pole_angle},
                  open(os.path.join(a.out, f"{a.prefix}{seg['id']}.metrics.json"), "w"), indent=0)
        log("metrics written")
    if a.frames:
        for f in [int(x) for x in a.frames.split(",") if x]:
            f = max(1, min(film.n, f))
            sc.frame_set(f)
            sc.render.filepath = os.path.join(a.out, f"{a.prefix}{seg['id']}_{f:04d}.png")
            t = time.time()
            bpy.ops.render.render(write_still=True)
            timings.append({"frame": f, "seconds": round(time.time() - t, 2)})
            log("still", f, timings[-1]["seconds"], "s")
    if a.anim:
        f0, f1 = 1, film.n
        if a.range:
            f0, f1 = (int(x) for x in a.range.split(":"))
        sc.frame_start, sc.frame_end = f0, min(f1, film.n)
        sc.render.filepath = os.path.join(a.out, seg["id"] + "_")
        stamps = []
        bpy.app.handlers.render_write.append(lambda s, *_: stamps.append(time.time()))
        t = time.time()
        bpy.ops.render.render(animation=True)
        timings.append({"anim": [sc.frame_start, sc.frame_end], "seconds": round(time.time() - t, 1),
                        "per_frame": [round(b - a2, 2) for a2, b in zip([t] + stamps[:-1], stamps)]})
        log("animation", timings[-1]["seconds"], "s")
    json.dump({"segment": seg["id"], "engine": a.engine, "frames": film.n, "timings": timings,
               "build_seconds": round(time.time() - T0, 1)},
              open(os.path.join(a.out, f"{a.prefix}{seg['id']}.timing.json"), "w"), indent=1)


if __name__ == "__main__":
    try:
        main()
    except Exception:
        import traceback
        traceback.print_exc()
        sys.stdout.flush()
        sys.exit(1)
