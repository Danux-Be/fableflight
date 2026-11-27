# Power line supports for FableFlight, built procedurally in Blender: steel
# lattice towers for the transmission lines and a concrete pole for the
# distribution lines (src/power.rs picks one by the line's voltage).
#
#   blender -b --factory-startup --python assets/blender/pylons.py -- <repo>/assets [--render <dir>]
#
# Saves assets/blender/pylons.blend, exports assets/pylons.bin.gz (the format
# of the aircraft, birds and balloons) and assets/pylons.json: for each
# support, where its conductors and earth wires hang (the foot of each
# insulator string), in the game's frame.
#
# The towers are generic types, after common European designs (dimensions
# typical of the type, not one line's drawings):
#   t400  "Donau" double circuit for 380-400 kV: about 52 m, the lower arm
#         some 28 m across with two phases a side, the upper arm one a side,
#         one earth wire on the peak; insulator strings of about 5 m
#   t220  the same type for 220-275 kV, smaller (about 40 m, 3.5 m strings)
#   t110  a three-level double circuit tower ("tonneau") for 63-150 kV:
#         about 34 m, arms 9-11 m across, 1.8 m strings, one earth wire
#   pole  a 20 kV distribution pole: 11 m of concrete, a steel crossarm with
#         two pin insulators and a third on the top
# Each tower stands with its arms across the line (Blender X), the line
# along Y, its foot at the origin. "<name> far": the same outline in a few
# heavy bars, for far off.
import bpy, bmesh, math, struct, gzip, sys, os, json
from mathutils import Vector as V

ARGS = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
ASSETS = os.path.abspath(ARGS[0]) if ARGS and not ARGS[0].startswith("--") else os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
RENDER = ARGS[ARGS.index("--render") + 1] if "--render" in ARGS else None

STEEL = (0.40, 0.42, 0.43)
STEEL_DK = (0.30, 0.31, 0.32)
GLASS = (0.16, 0.26, 0.22)
CONCRETE = (0.52, 0.51, 0.47)
CERAMIC = (0.55, 0.50, 0.42)
PAINT_RED = (0.55, 0.05, 0.03)
PAINT_WHITE = (0.75, 0.75, 0.72)


def colour_layer(bm):
    return bm.loops.layers.color.get("Col") or bm.loops.layers.color.new("Col")


def face(bm, layer, vs, col):
    f = bm.faces.new(vs)
    for l in f.loops:
        l[layer] = (*col, 1.0)
    return f


def bar(bm, layer, a, b, w, col=STEEL):
    """A square bar (an angle section, near enough) w wide from a to b."""
    a, b = V(a), V(b)
    d = (b - a)
    if d.length < 1e-6:
        return
    d.normalize()
    up = V((0, 0, 1)) if abs(d.z) < 0.9 else V((1, 0, 0))
    u = d.cross(up).normalized() * (w / 2)
    v = d.cross(u).normalized() * (w / 2)
    ring = lambda p: [bm.verts.new(p + u + v), bm.verts.new(p - u + v), bm.verts.new(p - u - v), bm.verts.new(p + u - v)]
    ra, rb = ring(a), ring(b)
    for i in range(4):
        j = (i + 1) % 4
        face(bm, layer, (ra[i], ra[j], rb[j], rb[i]), col)
    face(bm, layer, ra[::-1], col)
    face(bm, layer, rb, col)


def lathe(bm, layer, prof, n, col, at=(0, 0, 0), smooth=True):
    """About z, at `at`: prof = [(r, z)]."""
    ax, ay, az = at
    rings = []
    for (r, z) in prof:
        if r < 1e-6:
            rings.append([bm.verts.new((ax, ay, az + z))])
        else:
            rings.append([bm.verts.new((ax + r * math.cos(2 * math.pi * i / n), ay + r * math.sin(2 * math.pi * i / n), az + z)) for i in range(n)])
    for k in range(len(rings) - 1):
        a, b = rings[k], rings[k + 1]
        for i in range(n):
            j = (i + 1) % n
            if len(a) == 1:
                f = face(bm, layer, (a[0], b[i], b[j]), col)
            elif len(b) == 1:
                f = face(bm, layer, (a[i], a[j], b[0]), col)
            else:
                f = face(bm, layer, (a[i], a[j], b[j], b[i]), col)
            f.smooth = smooth


def insulator(bm, layer, top, length, glass=True, far=False):
    """A suspension string hanging from `top`: a rod of discs, a clamp at
    its foot. Returns the foot (where the conductor hangs)."""
    x, y, z = top
    if far:
        bar(bm, layer, (x, y, z), (x, y, z - length), 0.25, GLASS)
        return (x, y, z - length)
    lathe(bm, layer, [(0.0, 0.0), (0.04, 0.0), (0.04, length), (0.0, length)], 6, STEEL_DK, at=(x, y, z - length))
    n = max(3, int(length / 0.16))
    for k in range(n):
        zz = z - 0.25 - (length - 0.45) * k / (n - 1)
        lathe(bm, layer, [(0.0, -0.05), (0.13, -0.03), (0.14, 0.0), (0.05, 0.05), (0.0, 0.05)], 10, GLASS if glass else CERAMIC, at=(x, y, zz))
    # The clamp: a short yoke.
    bar(bm, layer, (x - 0.18, y, z - length), (x + 0.18, y, z - length), 0.08, STEEL_DK)
    return (x, y, z - length)


def body(bm, layer, levels, w, far):
    """A tapered four-legged lattice body: levels = [(z, half width)]. Legs,
    girts at each level, X-bracing on the four faces between them."""
    legs = [(1, 1), (-1, 1), (-1, -1), (1, -1)]
    lw = w * (3.0 if far else 1.0)
    for (sx, sy) in legs:
        for (z0, h0), (z1, h1) in zip(levels, levels[1:]):
            bar(bm, layer, (sx * h0, sy * h0, z0), (sx * h1, sy * h1, z1), lw * 1.4)
    if far:
        return
    for (z, h) in levels[1:]:
        for i in range(4):
            (ax, ay), (bx, by) = legs[i], legs[(i + 1) % 4]
            bar(bm, layer, (ax * h, ay * h, z), (bx * h, by * h, z), w)
    for (z0, h0), (z1, h1) in zip(levels, levels[1:]):
        # (Tall panels get two crosses.)
        k = 2 if (z1 - z0) > 2.5 * (h0 + h1) else 1
        for j in range(k):
            za, zb = z0 + (z1 - z0) * j / k, z0 + (z1 - z0) * (j + 1) / k
            ha, hb = h0 + (h1 - h0) * j / k, h0 + (h1 - h0) * (j + 1) / k
            for i in range(4):
                (ax, ay), (bx, by) = legs[i], legs[(i + 1) % 4]
                bar(bm, layer, (ax * ha, ay * ha, za), (bx * hb, by * hb, zb), w * 0.8)
                bar(bm, layer, (bx * ha, by * ha, za), (ax * hb, ay * hb, zb), w * 0.8)


def arm(bm, layer, z, h, reach, depth, w, far):
    """A pair of triangular arms out of the body at height z (body half
    width h), reaching `reach` each side: a top chord, two bottom chords
    and the lacing between."""
    for s in (-1, 1):
        tip_b = (s * reach, 0.0, z)
        tip_t = (s * reach, 0.0, z + 0.4)
        if far:
            bar(bm, layer, (s * h, 0.0, z + depth), tip_t, w * 3)
            bar(bm, layer, (s * h, 0.0, z), tip_b, w * 3)
            continue
        for sy in (-1, 1):
            bar(bm, layer, (s * h, sy * h, z), tip_b, w)
        bar(bm, layer, (s * h, 0.0, z + depth), tip_t, w)
        bar(bm, layer, (s * h, -h, z + depth), (s * h, h, z + depth), w * 0.8)
        n = max(2, int((reach - h) / 1.6))
        for k in range(n):
            t0, t1 = k / n, (k + 1) / n
            xb = lambda t: s * (h + (reach - h) * t)
            zt = lambda t: z + depth + (0.4 - depth) * t
            hy = lambda t: h * (1 - t)
            for sy in (-1, 1):
                bar(bm, layer, (xb(t0), sy * hy(t0), z), (xb(t1), 0.0, zt(t1)), w * 0.7)
            bar(bm, layer, (xb(t1), -hy(t1), z), (xb(t1), hy(t1), z), w * 0.7)


def peak(bm, layer, z0, h, top, w, far):
    """The earth wire's peak: a small pyramid over the body."""
    for (sx, sy) in [(1, 1), (-1, 1), (-1, -1), (1, -1)]:
        bar(bm, layer, (sx * h, sy * h, z0), (0.0, 0.0, top), w * (3 if far else 1.2))
    if not far:
        bar(bm, layer, (-0.3, 0, top), (0.3, 0, top), w)


def tower(name, coll, spec, far):
    bm = bmesh.new()
    layer = colour_layer(bm)
    w = spec["bar"]
    body(bm, layer, spec["levels"], w, far)
    attach = []
    for (z, reach, depth, phases) in spec["arms"]:
        h = next(hh for (zz, hh) in reversed(spec["levels"]) if zz <= z + 0.01)
        arm(bm, layer, z, h, reach, depth, w, far)
        for x in phases:
            attach.append(insulator(bm, layer, (x, 0.0, z), spec["string"], far=far))
    zt, ht = spec["levels"][-1]
    peak(bm, layer, zt, ht, spec["top"], w, far)
    earth = [(0.0, 0.0, spec["top"])]
    # Concrete feet.
    if not far:
        for (sx, sy) in [(1, 1), (-1, 1), (-1, -1), (1, -1)]:
            h0 = spec["levels"][0][1]
            lathe(bm, layer, [(0.0, -0.2), (0.55, -0.2), (0.55, 0.35), (0.0, 0.35)], 8, CONCRETE, at=(sx * h0, sy * h0, 0.0), smooth=False)
    ob = make(name, bm, coll)
    return ob, attach, earth


def pole(name, coll, far):
    bm = bmesh.new()
    layer = colour_layer(bm)
    # A tapered concrete pole (H section near enough round from afar).
    lathe(bm, layer, [(0.0, 0.0), (0.17, 0.0), (0.09, 11.0), (0.0, 11.05)], 4 if far else 10, CONCRETE, smooth=not far)
    bar(bm, layer, (-1.05, 0.0, 10.2), (1.05, 0.0, 10.2), 0.25 if far else 0.09, STEEL)
    attach = []
    for x in (-0.9, 0.9):
        if not far:
            lathe(bm, layer, [(0.0, 0.0), (0.025, 0.0), (0.025, 0.12), (0.0, 0.12)], 6, STEEL_DK, at=(x, 0.0, 10.25))
            lathe(bm, layer, [(0.0, 0.0), (0.07, 0.0), (0.09, 0.05), (0.07, 0.1), (0.09, 0.15), (0.06, 0.24), (0.0, 0.26)], 10, CERAMIC, at=(x, 0.0, 10.35))
        attach.append((x, 0.0, 10.6))
    if not far:
        lathe(bm, layer, [(0.0, 0.0), (0.07, 0.0), (0.09, 0.05), (0.07, 0.1), (0.09, 0.15), (0.06, 0.24), (0.0, 0.26)], 10, CERAMIC, at=(0.0, 0.0, 11.05))
        # Diagonal braces under the crossarm.
        for s in (-1, 1):
            bar(bm, layer, (0.0, 0.0, 9.4), (s * 0.6, 0.0, 10.15), 0.05, STEEL)
    attach.insert(1, (0.0, 0.0, 11.3))
    return make(name, bm, coll), attach, []


def make(name, bm, coll):
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    ob = bpy.data.objects.new(name, me)
    coll.objects.link(ob)
    return ob


# Towers: body levels (z, half width), arms (z, reach, depth, phases' x),
# string length, top of the peak, bar width.
SPECS = {
    "t400": dict(levels=[(0.0, 5.2), (10.0, 3.9), (20.0, 2.7), (29.0, 1.75), (33.0, 1.55), (43.0, 1.3), (47.0, 1.2)],
                 arms=[(33.0, 14.5, 3.0, [-13.8, -7.6, 7.6, 13.8]), (43.0, 9.6, 2.6, [-9.0, 9.0])],
                 string=5.0, top=52.5, bar=0.16),
    "t220": dict(levels=[(0.0, 4.2), (8.0, 3.1), (16.0, 2.2), (23.0, 1.45), (26.0, 1.3), (34.0, 1.1), (37.0, 1.0)],
                 arms=[(26.0, 11.3, 2.4, [-10.8, -6.0, 6.0, 10.8]), (34.0, 7.6, 2.0, [-7.1, 7.1])],
                 string=3.6, top=41.0, bar=0.13),
    "t110": dict(levels=[(0.0, 3.0), (7.0, 2.2), (14.0, 1.5), (20.0, 1.1), (24.5, 1.0), (29.0, 0.9), (31.0, 0.85)],
                 arms=[(20.0, 4.8, 1.5, [-4.5, 4.5]), (24.5, 5.8, 1.5, [-5.5, 5.5]), (29.0, 4.8, 1.5, [-4.5, 4.5])],
                 string=1.9, top=34.5, bar=0.11),
}
# The order of the phases: a circuit's three on one side, then the other's
# (with a single circuit, the game uses the first three).
ORDER = {"t400": [0, 1, 4, 3, 2, 5], "t220": [0, 1, 4, 3, 2, 5], "t110": [0, 2, 4, 1, 3, 5]}
EXPECTED = ["t400", "t220", "t110", "pole", "t400 far", "t220 far", "t110 far", "pole far"]


def build():
    for o in list(bpy.data.objects):
        bpy.data.objects.remove(o)
    coll = bpy.context.scene.collection
    info = {}
    x = 0.0
    for name, spec in SPECS.items():
        ob, attach, earth = tower(name, coll, spec, False)
        ob.location = (x, 0, 0)
        tower(name + " far", coll, spec, True)[0].location = (x, 60, 0)
        attach = [attach[i] for i in ORDER[name]]
        info[name] = {"height": spec["top"], "phases": [game(a) for a in attach], "earth": [game(e) for e in earth]}
        x += 40.0
    ob, attach, earth = pole("pole", coll, False)
    ob.location = (x, 0, 0)
    pole("pole far", coll, True)[0].location = (x, 60, 0)
    info["pole"] = {"height": 11.3, "phases": [game(a) for a in attach], "earth": []}
    return info


def game(v):
    return [round(v[0], 3), round(v[2], 3), round(-v[1], 3)]


def export(path, info):
    out = bytearray(b"FFAC")
    obs = [bpy.data.objects[n] for n in EXPECTED]
    out += struct.pack("<II", 1, len(obs))
    total = 0
    for ob in obs:
        me = ob.data
        me.calc_loop_triangles()
        col = me.color_attributes.get("Col")
        verts, index, idx = [], [], {}
        for tri in me.loop_triangles:
            for k in range(3):
                vi, li = tri.vertices[k], tri.loops[k]
                n = V(tri.split_normals[k]).normalized()
                cc = tuple(round(c, 4) for c in col.data[li].color[:3])
                key = (vi, round(n.x, 3), round(n.y, 3), round(n.z, 3), cc)
                if key not in idx:
                    idx[key] = len(verts)
                    verts.append((*game(me.vertices[vi].co), *game(n), *cc, 1.0, 0.0, 0.0, 0.0))
                index.append(idx[key])
        name = ob.name.encode()
        out += struct.pack("<I", len(name)) + name
        out += struct.pack("<7f", 0, 0, 0, 0, 0, 1, 0)
        out += struct.pack("<I", len(verts))
        for v in verts:
            out += struct.pack("<13f", *v)
        out += struct.pack("<I", len(index))
        out += struct.pack("<%dI" % len(index), *index)
        total += len(index) // 3
        print(f"  {ob.name}: {len(index) // 3} triangles, {len(verts)} vertices")
    with gzip.open(path, "wb", compresslevel=9) as f:
        f.write(bytes(out))
    with open(os.path.join(os.path.dirname(path), "pylons.json"), "w") as f:
        json.dump(info, f, indent=1)
        f.write("\n")
    print("wrote", path, len(out), "bytes raw,", total, "triangles")


def render(outdir):
    sc = bpy.context.scene
    for ob in bpy.data.objects:
        if ob.type != "MESH":
            continue
        mat = bpy.data.materials.new(ob.name)
        mat.use_nodes = True
        nt = mat.node_tree
        bsdf = next(n for n in nt.nodes if n.type == "BSDF_PRINCIPLED")
        attr = nt.nodes.new("ShaderNodeVertexColor")
        attr.layer_name = "Col"
        nt.links.new(attr.outputs[0], bsdf.inputs["Base Color"])
        ob.data.materials.append(mat)
    sc.world = bpy.data.worlds.new("w")
    sc.world.color = (0.45, 0.55, 0.7)
    sun = bpy.data.objects.new("sun", bpy.data.lights.new("sun", "SUN"))
    sc.collection.objects.link(sun)
    sun.rotation_euler = (0.9, 0.2, 2.4)
    sun.data.energy = 4
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
    sc.collection.objects.link(cam)
    sc.camera = cam
    sc.render.engine = "CYCLES"
    sc.cycles.samples = 24
    sc.render.resolution_x, sc.render.resolution_y = 1400, 700
    for nm, pos, tgt, lens in [("all", (60, -150, 40), (60, 30, 22), 35), ("t400top", (12, -22, 44), (0, 0, 38), 30)]:
        cam.location = pos
        cam.data.lens = lens
        cam.rotation_euler = (V(tgt) - V(pos)).to_track_quat("-Z", "Y").to_euler()
        sc.render.filepath = os.path.join(outdir, f"pylons_{nm}.png")
        bpy.ops.render.render(write_still=True)


def main():
    info = build()
    export(os.path.join(ASSETS, "pylons.bin.gz"), info)
    bpy.ops.wm.save_as_mainfile(filepath=os.path.join(ASSETS, "blender", "pylons.blend"))
    if RENDER:
        render(RENDER)


main()
