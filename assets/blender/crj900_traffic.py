# Turns the CRJ-900 model from Sketchfab into the airport and live traffic's
# "regional jet" (airmodels.rs): assets/crj900.bin.gz.
#
# Source: "Airplane CRJ-900 Cityjet" by artoud (CityJet Training),
# https://sketchfab.com/3d-models/airplane-crj-900-cityjet-02c4fa44604243c2bb48db64506a39af
# licensed CC BY 4.0 (https://creativecommons.org/licenses/by/4.0/). Changed:
# the texture is baked into flat vertex colours, the CityJet livery (stripes,
# titles, logo) dropped so the traffic's own made-up liveries paint it, the
# cabin windows rebuilt as glass quads, and the model split into body, fin
# (with the winglets) and gear.
#
# In Blender: import the model (36.2 m long, nose towards -Y, wheels on z = 0,
# its mesh object named "Object_2"), set OUT, and run this script.
#
# The output, little-endian: for each of body, livery, gear: u32 vertex count,
# u32 index count, then the vertices (position, normal, colour: 10 f32; the
# colour's alpha is the material, 0 paint, 0.5 glass, 1 as coloured), then the
# indices (u32). In the game's frame: +X right, +Y up, +Z aft, the origin on
# the ground between the main wheels.
import bmesh
import bpy
import gzip
import numpy as np
import struct

OUT = globals().get("OUT", "crj900.bin.gz")

o = bpy.data.objects["Object_2"]
me = o.data
M = o.matrix_world
N3 = M.to_3x3().inverted().transposed()
img = next(n.image for n in me.materials[0].node_tree.nodes if n.type == "TEX_IMAGE" and n.image)
W, H = img.size
buf = np.empty(W * H * 4, np.float32)
img.pixels.foreach_get(buf)
px = buf.reshape(H, W, 4)[:, :, :3]
px = px.reshape(H // 2, 2, W // 2, 2, 3).mean((1, 3))
H2, W2 = H // 2, W // 2


def sample(u, v):
    x = np.clip((u % 1.0) * W2, 0, W2 - 1).astype(int)
    y = np.clip((v % 1.0) * H2, 0, H2 - 1).astype(int)
    return px[y, x]


def classify(c):
    """0 paint, 1 glass (the blue windows), 2 grey metal, 3 dark (tyres, intakes)."""
    r, g, b = c[..., 0], c[..., 1], c[..., 2]
    lum = 0.299 * r + 0.587 * g + 0.114 * b
    sat = c.max(-1) - c.min(-1)
    cls = np.zeros(r.shape, int)
    cls[(sat < 0.08) & (lum < 0.72)] = 2
    cls[lum < 0.2] = 3
    cls[(b > r + 0.1) & (b > 0.3)] = 1
    return cls


# Loose parts: the wheels and legs under the nose and the wing are the gear.
bm = bmesh.new()
bm.from_mesh(me)
bm.verts.ensure_lookup_table()
part = np.full(len(me.vertices), -1)
k = 0
for v in bm.verts:
    if part[v.index] >= 0:
        continue
    st = [v]
    part[v.index] = k
    while st:
        x = st.pop()
        for e in x.link_edges:
            y = e.other_vert(x)
            if part[y.index] < 0:
                part[y.index] = k
                st.append(y)
    k += 1
co = np.array([(M @ v.co)[:] for v in me.vertices])
gear_part = set()
for p in range(k):
    c = co[part == p]
    yc = c[:, 1].mean()
    if c[:, 2].max() < 1.5 and (1.5 < yc < 3.6 or 18.5 < yc < 21.0):
        gear_part.add(p)

# The game's frame: right +X (Blender -X), up +Y (Blender Z), aft +Z (Blender Y),
# the origin on the ground between the main wheels.
ORG = np.array([0.0, 19.785, co[:, 2].min()])


def to_game(p):
    q = p - ORG
    return (-q[0], q[2], q[1])


def nrm_game(n):
    return (-n[0], n[2], n[1])


me.calc_loop_triangles()
uvl = me.uv_layers.active.data
cn = me.corner_normals
out = {"body": [], "livery": [], "gear": []}
bary = np.array([[1/3, 1/3, 1/3], [.6, .2, .2], [.2, .6, .2], [.2, .2, .6], [.45, .45, .1], [.45, .1, .45], [.1, .45, .45], [.8, .1, .1], [.1, .8, .1], [.1, .1, .8], [.33, .6, .07], [.07, .33, .6]])
PAINT = (0.82, 0.82, 0.82, 0.0)
GLASS = (0.03, 0.03, 0.035, 0.5)
for t in me.loop_triangles:
    vi, li = list(t.vertices), list(t.loops)
    uv = np.array([uvl[l].uv[:] for l in li])
    s = bary @ uv
    cols = sample(s[:, 0], s[:, 1])
    cls = classify(cols)
    c = int(np.bincount(cls, minlength=4).argmax())
    if c == 0:
        col = PAINT  # (the livery's stripes and titles go with the white)
    elif c == 1:
        col = GLASS
    else:
        m = cols[cls == c].mean(0)
        col = (float(m[0]), float(m[1]), float(m[2]), 1.0)
    P = co[vi]
    cen = P.mean(0)
    if part[vi[0]] in gear_part:
        dest = "gear"
    elif (abs(cen[0]) < 0.35 and cen[2] > 4.45 and cen[1] > 27.0) or (abs(cen[0]) > 10.9 and cen[2] > 3.45):
        dest = "livery"  # the fin and the winglets, in the airline's colour
        col = PAINT if c != 1 else col
    else:
        dest = "body"
    for j in range(3):
        n = (N3 @ cn[li[j]].vector).normalized()
        out[dest].append((*to_game(P[j]), *nrm_game(n[:]), *col))

# The cabin windows, painted as small blue blobs: each put back as a glass quad
# where it is painted (the triangles are far bigger than the windows).
blue = (px[..., 2] > px[..., 0] + 0.1) & (px[..., 2] > 0.3)
lab = np.zeros(blue.shape, int)
blobs = []
nb = 0
for y0, x0 in zip(*np.nonzero(blue)):
    if lab[y0, x0]:
        continue
    nb += 1
    st = [(y0, x0)]
    lab[y0, x0] = nb
    pts = []
    while st:
        y, x = st.pop()
        pts.append((y, x))
        for dy, dx in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            yy, xx = y + dy, x + dx
            if 0 <= yy < H2 and 0 <= xx < W2 and blue[yy, xx] and not lab[yy, xx]:
                lab[yy, xx] = nb
                st.append((yy, xx))
    if 30 <= len(pts) <= 3000:
        a = np.array(pts)
        blobs.append(((a[:, 1].mean() + 0.5) / W2, (a[:, 0].mean() + 0.5) / H2, (np.ptp(a[:, 1]) + 1) / W2 / 2, (np.ptp(a[:, 0]) + 1) / H2 / 2))
tris = [(list(t.vertices), np.array([uvl[l].uv[:] for l in t.loops])) for t in me.loop_triangles]
for (u, v, du, dv) in blobs:
    for vi, uv in tris:
        if part[vi[0]] in gear_part:
            continue
        a, b, c = uv
        d = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1])
        if abs(d) < 1e-12:
            continue
        l1 = ((b[1] - c[1]) * (u - c[0]) + (c[0] - b[0]) * (v - c[1])) / d
        l2 = ((c[1] - a[1]) * (u - c[0]) + (a[0] - c[0]) * (v - c[1])) / d
        l3 = 1 - l1 - l2
        if min(l1, l2, l3) < -1e-6:
            continue
        P = co[vi]
        p = l1 * P[0] + l2 * P[1] + l3 * P[2]
        J = np.array([P[1] - P[0], P[2] - P[0]]).T @ np.linalg.inv(np.array([uv[1] - uv[0], uv[2] - uv[0]]).T)
        nrm = np.cross(P[1] - P[0], P[2] - P[0])
        nrm /= np.linalg.norm(nrm)
        if np.dot(nrm, p - np.array([0.0, p[1], 2.77])) < 0:
            nrm = -nrm
        q = p + nrm * 0.015
        cs = [q - J[:, 0] * du - J[:, 1] * dv, q + J[:, 0] * du - J[:, 1] * dv, q + J[:, 0] * du + J[:, 1] * dv, q - J[:, 0] * du + J[:, 1] * dv]
        if np.dot(np.cross(cs[1] - cs[0], cs[2] - cs[0]), nrm) < 0:
            cs = cs[::-1]
        for tri in ((0, 1, 2), (0, 2, 3)):
            for j in tri:
                out["body"].append((*to_game(cs[j]), *nrm_game(nrm), *GLASS))

# Shared vertices, rounded to a tenth of a millimetre, and the file.
data = bytearray()
for name in ("body", "livery", "gear"):
    verts, index, idx = [], {}, []
    for v in out[name]:
        key = tuple(round(float(x), 4) for x in v)
        if key not in index:
            index[key] = len(verts)
            verts.append(key)
        idx.append(index[key])
    data += struct.pack("<II", len(verts), len(idx))
    for v in verts:
        data += struct.pack("<10f", *v)
    data += struct.pack(f"<{len(idx)}I", *idx)
    print(name, len(verts), "vertices", len(idx) // 3, "triangles")
open(OUT, "wb").write(gzip.compress(bytes(data), 9, mtime=0))
print("windows", len(blobs), "written", OUT, len(data), "bytes raw")
