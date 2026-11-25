# Hot-air balloons for FableFlight, built procedurally in Blender: an
# envelope in eight fictional colour schemes, the skirt, the flying wires,
# two rigs (basket, burner frame, burners, passengers) and the burner flame.
#
#   blender -b --factory-startup --python assets/blender/balloons.py -- <repo>/assets [--render <dir>]
#
# Saves assets/blender/balloons.blend and exports assets/balloons.bin.gz (the
# same file format as the aircraft and the birds; read by src/balloons.rs).
# The rebuild starts from scratch, so it throws away anything edited by hand
# in the .blend. To keep hand edits, export the .blend as it stands instead:
#
#   blender -b assets/blender/balloons.blend --python assets/blender/balloons.py -- <repo>/assets --export-only
#
# Two sizes share the envelope (the large one is the same mesh scaled by
# LARGE_SCALE in the game):
#
# - a sport balloon, after the Cameron O-105 (105,000 ft³, about 3,000 m³):
#   envelope 64 ft (19.5 m) high and 61 ft (18.6 m) across, from Cameron's
#   O-type table (https://cameronballoons.com/products/hot-air-balloons/envelopes/o-type);
#   a four-person basket.
# - a passenger balloon, after the Ultramagic N-425 (12,000 m³): 31.2 m high,
#   29.5 m across, 28 gores, 20 passengers (Ultramagic catalogue; the flight
#   manual gives 35.5 m overall). The envelope here is the sport one scaled by
#   1.6 (31.2 / 19.5), with 24 gores rather than 28: a difference nobody sees
#   from another aircraft.
#
# Estimates, from photographs of balloons at launch and in flight (no maker's
# drawings were used): 24 gores on the sport envelope (common on modern
# envelopes; Cameron's O-type table lists 12), the natural shape's profile
# (widest at about 62% of the height, a throat about 21% of the diameter),
# 14 rows of panels, the skirt (scoop) about 1.3 m deep, the basket sizes
# (sport 1.25 x 1.55 x 1.12 m; passenger 4.6 x 2.2 x 1.25 m, a pilot's
# compartment in the middle and four for passengers), the heights of the
# burner frames and of the mouth above the basket floor, and the flame.
#
# The colour schemes are fictional, in the spirit of balloons seen at
# festivals (rainbow gores, harlequin blocks, bands, chevrons, checks, a
# wave, stripes, blocks on yellow). No real operator's markings or logos.
#
# Pieces:
#   "env <scheme>"       the envelope, near (24 gores with load tapes)
#   "env <scheme> far"   the same, coarse, for far away
#   "skirt"              the scoop below the mouth (dark, flame-proof fabric)
#   "wires sport"        flying wires from the mouth to the sport burner frame
#   "wires large"        the same for the passenger balloon (its own lengths)
#   "rig sport"          basket, padding, poles, burner frame, double burner,
#                        cylinders, three people
#   "rig large"          passenger basket, quad burner, pilot and passengers
#   "rig far"            a plain basket for far away
#   "flame"              the burner flame, 1 m nominal, nozzle at the origin
# Origins: the envelope, the skirt and the far envelope at the middle of the
# mouth; the rigs and wires at the middle of the basket floor; the flame at
# the burner nozzle. Custom property "pivot" on each rig: its burner nozzle
# (where the flame goes), and "axis": the mouth's position (x, y, z) above
# the basket floor, for src/balloons.rs to check its constants against.
#
# Colours: a "Col" colour attribute on the face corners (linear albedo).
#
# Blender frame: +X right, +Y forward, +Z up, metres. The game's frame is +X
# right, +Y up, -Z forward, so a point (x, y, z) here is (x, z, -y) there.
import bpy, bmesh, math, struct, gzip, sys, os
from mathutils import Vector as V

argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
ASSETS = os.path.abspath(argv[0]) if argv else os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RENDER = argv[argv.index("--render") + 1] if "--render" in argv else None
EXPORT_ONLY = "--export-only" in argv

# ---------------------------------------------------------------- dimensions
H = 19.5            # envelope, mouth to crown (m)
R = 9.3             # envelope radius at its widest
GORES = 24
ROWS = 14           # rows of panels
LARGE_SCALE = 1.6

# The natural shape: (height share, radius share), mouth to crown.
PROFILE = [
    (0.00, 0.210), (0.06, 0.320), (0.14, 0.480), (0.24, 0.650), (0.34, 0.790),
    (0.44, 0.900), (0.53, 0.968), (0.62, 1.000), (0.70, 0.992), (0.77, 0.955),
    (0.84, 0.895), (0.90, 0.790), (0.945, 0.645), (0.975, 0.460), (0.992, 0.260),
    (1.000, 0.000),
]

# Sport rig (basket floor at z = 0).
S_BASKET = (1.25, 1.55, 1.12)       # width (x), length (y), height
S_FRAME_Z = 2.45                    # burner frame
S_NOZZLE_Z = 2.82                   # top of the burner coils
S_MOUTH_Z = 4.55                    # mouth (throat) above the basket floor
# Passenger rig.
L_BASKET = (2.2, 4.6, 1.25)
L_FRAME_Z = 2.95
L_NOZZLE_Z = 3.35
L_MOUTH_Z = 6.2

# --------------------------------------------------------------- colours (linear)
def c(r, g, b):
    return (r, g, b)
RED = c(0.55, 0.025, 0.02)
ORANGE = c(0.78, 0.20, 0.012)
YELLOW = c(0.82, 0.56, 0.02)
LIME = c(0.36, 0.62, 0.03)
GREEN = c(0.03, 0.30, 0.06)
SKY = c(0.04, 0.30, 0.66)
BLUE = c(0.02, 0.10, 0.50)
NAVY = c(0.010, 0.020, 0.10)
PURPLE = c(0.13, 0.025, 0.28)
PINK = c(0.75, 0.07, 0.24)
CREAM = c(0.78, 0.64, 0.36)
WHITE = c(0.76, 0.76, 0.73)
BLACK = c(0.018, 0.018, 0.022)
TEAL = c(0.02, 0.32, 0.32)
WICKER = c(0.42, 0.29, 0.14)
WICKER_DK = c(0.27, 0.17, 0.07)
LEATHER = c(0.10, 0.055, 0.03)
SUEDE = c(0.20, 0.10, 0.05)
STEEL = c(0.50, 0.50, 0.52)
DARK_STEEL = c(0.10, 0.10, 0.11)
NOMEX = c(0.045, 0.045, 0.05)
FLOOR = c(0.06, 0.05, 0.04)
SKIN = c(0.55, 0.35, 0.24)
JACKETS = [c(0.02, 0.08, 0.30), RED, c(0.05, 0.05, 0.06), c(0.30, 0.30, 0.28), GREEN, c(0.55, 0.30, 0.02), PURPLE, SKY]

RAINBOW = [RED, ORANGE, YELLOW, GREEN, BLUE, PURPLE]


def scheme_colour(scheme, g, k):
    """Panel colour of gore g (0..GORES-1), row k (0 at the mouth .. ROWS-1 at the crown)."""
    top = k >= ROWS - 1
    if scheme == "rainbow":
        return RAINBOW[g % 6]
    if scheme == "harlequin":
        # Diagonal steps of colour over a dark lower part (blocks offset gore by gore).
        if k < 4:
            return NAVY if (g % 2 == 0) else BLACK
        if k == 4:
            return BLUE if g % 2 == 0 else NAVY
        pal = [YELLOW, CREAM, ORANGE, PINK, PURPLE, BLUE]
        return pal[(k + g) % 6] if not top else YELLOW
    if scheme == "bands":
        band = [RED, RED, ORANGE, ORANGE, YELLOW, YELLOW, WHITE, YELLOW, YELLOW, ORANGE, ORANGE, RED, RED, RED]
        col = band[k]
        return col if g % 2 == 0 else tuple(x * 0.82 for x in col)
    if scheme == "chevron":
        if k < 3:
            return ORANGE
        if top or k >= ROWS - 3:
            return YELLOW if k < ROWS - 1 else RED
        zig = abs((g % 6) - 3)
        return LIME if ((k + zig) // 2) % 2 == 0 else BLACK
    if scheme == "blocks":
        # Yellow with black stripes every fourth gore and a band of colour.
        if k < 2:
            return ORANGE
        if g % 4 == 0:
            return BLACK
        if 6 <= k <= 8:
            return [SKY, PINK, LIME, PURPLE, RED, BLUE][(g + k) % 6]
        if k == 5 or k == 9:
            return BLACK
        return YELLOW
    if scheme == "wave":
        # Orange above, blue below, a white wave between.
        mid = 7 + (1 if (g // 2) % 2 else 0)
        if k == mid:
            return WHITE
        return ORANGE if k > mid else (BLUE if k > 2 else NAVY)
    if scheme == "checks":
        upper = k >= 7
        a, b = (BLUE, YELLOW) if upper else (GREEN, YELLOW)
        if top:
            return BLUE
        return a if (g + k) % 2 == 0 else b
    if scheme == "stripes":
        if k < 2:
            return RED
        return [NAVY, WHITE, RED][g % 3] if not top else NAVY
    raise ValueError(scheme)


SCHEMES = ["rainbow", "harlequin", "bands", "chevron", "blocks", "wave", "checks", "stripes"]

# ------------------------------------------------------------------- helpers
def pchip(xs, ys):
    """Monotone cubic interpolation (Fritsch-Carlson)."""
    n = len(xs)
    h = [xs[i + 1] - xs[i] for i in range(n - 1)]
    d = [(ys[i + 1] - ys[i]) / h[i] for i in range(n - 1)]
    m = [0.0] * n
    m[0], m[-1] = d[0], d[-1]
    for i in range(1, n - 1):
        if d[i - 1] * d[i] <= 0:
            m[i] = 0.0
        else:
            w1, w2 = 2 * h[i] + h[i - 1], h[i] + 2 * h[i - 1]
            m[i] = (w1 + w2) / (w1 / d[i - 1] + w2 / d[i])

    def f(x):
        x = min(max(x, xs[0]), xs[-1])
        i = max(0, min(n - 2, next((j for j in range(n - 1) if x <= xs[j + 1]), n - 2)))
        t = (x - xs[i]) / h[i]
        h00, h10, h01, h11 = 2 * t**3 - 3 * t**2 + 1, t**3 - 2 * t**2 + t, -2 * t**3 + 3 * t**2, t**3 - t**2
        return h00 * ys[i] + h10 * h[i] * m[i] + h01 * ys[i + 1] + h11 * h[i] * m[i + 1]
    return f


PR = pchip([p[0] for p in PROFILE], [p[1] for p in PROFILE])


def radius(t):
    return R * max(PR(t), 0.0)


def arc_stations(n_rows, sub):
    """Heights (share of H) splitting the meridian into rows of equal fabric
    length (panels are cut about the same height), each split into `sub`."""
    ts = [i / 2000 for i in range(2001)]
    s = [0.0]
    for a, b in zip(ts, ts[1:]):
        s.append(s[-1] + math.hypot((b - a) * H, radius(b) - radius(a)))
    total = s[-1]
    out = []
    for j in range(n_rows * sub + 1):
        want = total * j / (n_rows * sub)
        i = next((i for i, v in enumerate(s) if v >= want), len(s) - 1)
        out.append(ts[i])
    out[-1] = 1.0
    return out


def make_object(name, bm, coll, sharp_angle=None):
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    ob = bpy.data.objects.new(name, me)
    coll.objects.link(ob)
    return ob


def colour_layer(bm):
    return bm.loops.layers.color.get("Col") or bm.loops.layers.color.new("Col")


def paint(face, layer, col):
    for l in face.loops:
        l[layer] = (*col, 1.0)


def tube(bm, layer, a, b, r, col, n=6, caps=True):
    """A round bar from a to b."""
    a, b = V(a), V(b)
    ax = (b - a)
    if ax.length < 1e-6:
        return
    ax.normalize()
    u = ax.orthogonal().normalized()
    w = ax.cross(u)
    ra, rb = [], []
    for i in range(n):
        ang = 2 * math.pi * i / n
        d = u * math.cos(ang) + w * math.sin(ang)
        ra.append(bm.verts.new(a + d * r))
        rb.append(bm.verts.new(b + d * r))
    for i in range(n):
        j = (i + 1) % n
        f = bm.faces.new((ra[i], ra[j], rb[j], rb[i]))
        f.smooth = True
        paint(f, layer, col)
    if caps:
        for ring in (ra[::-1], rb):
            f = bm.faces.new(ring)
            paint(f, layer, col)


def box(bm, layer, lo, hi, col, smooth=False):
    (x0, y0, z0), (x1, y1, z1) = lo, hi
    vs = [bm.verts.new(p) for p in [(x0, y0, z0), (x1, y0, z0), (x1, y1, z0), (x0, y1, z0), (x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1)]]
    for q in [(0, 3, 2, 1), (4, 5, 6, 7), (0, 1, 5, 4), (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7)]:
        f = bm.faces.new([vs[i] for i in q])
        f.smooth = smooth
        paint(f, layer, col)


def lathe(bm, layer, prof, n, col_of, z0=0.0, centre=(0.0, 0.0), smooth=True):
    """A body of revolution: prof = [(r, z)], colour per ring band."""
    cx, cy = centre
    rings = []
    for (r, z) in prof:
        if r < 1e-6:
            rings.append([bm.verts.new((cx, cy, z0 + z))])
        else:
            rings.append([bm.verts.new((cx + r * math.cos(2 * math.pi * i / n), cy + r * math.sin(2 * math.pi * i / n), z0 + z)) for i in range(n)])
    for k in range(len(rings) - 1):
        a, b = rings[k], rings[k + 1]
        for i in range(n):
            j = (i + 1) % n
            if len(a) == 1:
                f = bm.faces.new((a[0], b[i], b[j]))
            elif len(b) == 1:
                f = bm.faces.new((a[i], a[j], b[0]))
            else:
                f = bm.faces.new((a[i], a[j], b[j], b[i]))
            f.smooth = smooth
            paint(f, layer, col_of(k, i))


def person(bm, layer, x, y, z_floor, jacket, h=1.72, facing=0.0, rng=0.0):
    """Someone standing in a basket: legs hidden, torso, shoulders, head, cap."""
    torso_r = 0.19
    # Torso (slightly flattened front to back), from the hips up to the shoulders.
    z_hip, z_sh = z_floor + 0.95 * h / 1.72, z_floor + 1.42 * h / 1.72
    prof = [(0.0, 0.0), (0.16, 0.0), (0.19, 0.12), (0.21, 0.32), (0.20, z_sh - z_hip - 0.04), (0.12, z_sh - z_hip + 0.02), (0.06, z_sh - z_hip + 0.06)]
    rings = []
    for (r, z) in prof:
        rings.append([V((x + r * math.cos(a) * 1.0, y + r * math.sin(a) * 0.62, z_hip + z)) for a in [2 * math.pi * i / 8 + facing for i in range(8)]])
    vs = [[bm.verts.new(p) for p in ring] for ring in rings]
    for k in range(len(vs) - 1):
        for i in range(8):
            j = (i + 1) % 8
            f = bm.faces.new((vs[k][i], vs[k][j], vs[k + 1][j], vs[k + 1][i]))
            f.smooth = True
            paint(f, layer, jacket)
    top = bm.faces.new(vs[-1])
    paint(top, layer, jacket)
    # Head and a cap or hair.
    hz = z_sh + 0.17 * h / 1.72
    hair = [c(0.03, 0.02, 0.015), c(0.12, 0.07, 0.03), c(0.30, 0.22, 0.10), c(0.08, 0.08, 0.09)][int(rng * 4) % 4]
    lathe(bm, layer, [(0.0, -0.12), (0.07, -0.11), (0.095, -0.05), (0.10, 0.02), (0.085, 0.08), (0.05, 0.115), (0.0, 0.125)], 8,
          lambda k, i: hair if k >= 4 else SKIN, z0=hz, centre=(x, y))


def wicker_wall(bm, layer, p0, p1, z0, z1, inside, cell=(0.06, 0.035)):
    """A flat wicker wall from p0 to p1 (x, y): rows of rattan woven in and
    out of upright stakes, as a grid of small faces in a few shades (each row
    shifted half a stake from the last, the strands catching the light a
    little differently)."""
    p0, p1 = V((*p0, 0)), V((*p1, 0))
    L = (p1 - p0).length
    nu = max(2, int(L / cell[0]))
    nv = max(2, int((z1 - z0) / cell[1]))
    grid = [[bm.verts.new(p0 + (p1 - p0) * (i / nu) + V((0, 0, z0 + (z1 - z0) * j / nv))) for j in range(nv + 1)] for i in range(nu + 1)]
    seed = int(abs(p0.x * 1000 + p0.y * 7919 + p1.x * 31 + z0 * 101)) + 1
    for i in range(nu):
        for j in range(nv):
            q = (grid[i][j], grid[i + 1][j], grid[i + 1][j + 1], grid[i][j + 1])
            f = bm.faces.new(q if not inside else q[::-1])
            seed = (seed * 1103515245 + 12345) % 2147483648
            jitter = 0.86 + 0.28 * (seed / 2147483648)
            under = (i + j % 2) % 2 == 1
            k = jitter * (0.78 if under else 1.0) * (0.92 if j % 6 == 5 else 1.0)
            col = tuple(x * k for x in WICKER)
            if inside:
                col = tuple(x * 0.72 for x in col)
            paint(f, layer, col)


# ---------------------------------------------------------------- the envelope
def gore_columns(near):
    # Shares across a gore: a load tape on each edge, the bulging panel between.
    return [0.0, 0.012, 0.25, 0.5, 0.75, 0.988, 1.0] if near else [0.0, 0.5, 1.0]


def envelope(name, scheme, near, coll):
    bm = bmesh.new()
    layer = colour_layer(bm)
    sub = 3 if near else 1
    ts = arc_stations(ROWS, sub)
    cols = gore_columns(near)
    rows = []
    for j, t in enumerate(ts):
        z = t * H
        r = radius(t)
        ring = []
        for g in range(GORES):
            for ci, u in enumerate(cols[:-1]):
                ang = 2 * math.pi * (g + u) / GORES
                # Between the load tapes the fabric bulges out (less at the
                # mouth and the crown, where the gores are narrow).
                bulge = 0.022 * math.sin(math.pi * u) * math.sin(math.pi * min(max(t, 0.0), 1.0)) ** 0.5
                rr = r * (1 + bulge)
                ring.append(bm.verts.new((rr * math.cos(ang), rr * math.sin(ang), z)))
        if r < 1e-6:
            for v in ring:
                bm.verts.remove(v)
            ring = [bm.verts.new((0.0, 0.0, z))]
        rows.append(ring)
    n_per = len(cols) - 1
    tape_cols = {0, n_per - 1} if near else set()
    for j in range(len(rows) - 1):
        a, b = rows[j], rows[j + 1]
        k = min(j // sub, ROWS - 1)
        for g in range(GORES):
            panel = scheme_colour(scheme, g, k)
            tape = tuple(0.55 * x + 0.02 for x in panel)
            for ci in range(n_per):
                i = g * n_per + ci
                i2 = (i + 1) % len(a) if len(a) > 1 else 0
                col = tape if ci in tape_cols else panel
                if len(b) == 1:
                    f = bm.faces.new((a[i], a[i2], b[0]))
                else:
                    f = bm.faces.new((a[i], a[i2], b[i2 if len(b) > 1 else 0], b[i]))
                f.smooth = True
                paint(f, layer, col)
    # The load tapes are creases: sharp edges along the gore seams.
    bm.edges.ensure_lookup_table()
    for e in bm.edges:
        a, b = e.verts
        if abs(a.co.z - b.co.z) > 1e-4 and math.hypot(a.co.x, a.co.y) > 1e-3:
            ang = math.atan2(a.co.y, a.co.x) % (2 * math.pi)
            frac = ang / (2 * math.pi) * GORES
            if abs(frac - round(frac)) < 1e-4:
                e.smooth = False
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    return make_object(name, bm, coll)


def skirt(coll):
    """The scoop: flame-proof fabric hanging below the mouth, a little wider."""
    bm = bmesh.new()
    layer = colour_layer(bm)
    r0 = radius(0.0)
    prof = [(r0 * 1.0, 0.0), (r0 * 0.99, -0.45), (r0 * 1.0, -0.9), (r0 * 1.02, -1.3)]
    n = GORES * 2
    rings = [[bm.verts.new((r * math.cos(2 * math.pi * i / n), r * math.sin(2 * math.pi * i / n), z)) for i in range(n)] for r, z in prof]
    for k in range(len(rings) - 1):
        for i in range(n):
            j = (i + 1) % n
            f = bm.faces.new((rings[k][i], rings[k][j], rings[k + 1][j], rings[k + 1][i]))
            f.smooth = True
            paint(f, layer, NOMEX if k < 2 else tuple(x * 1.4 for x in NOMEX))
    # (Two-sided: the inside, seen from the basket, is a copy facing in.)
    inner = bmesh.ops.duplicate(bm, geom=list(bm.faces))["geom"]
    bmesh.ops.reverse_faces(bm, faces=[f for f in inner if isinstance(f, bmesh.types.BMFace)])
    return make_object("skirt", bm, coll)


def wires(name, frame_half, frame_z, mouth_z, scale, coll):
    """Flying wires: from the bottom of every second load tape at the mouth
    down to the nearest corner of the burner frame."""
    bm = bmesh.new()
    layer = colour_layer(bm)
    r0 = radius(0.0) * scale
    corners = [(sx * frame_half, sy * frame_half) for sx, sy in [(1, 1), (-1, 1), (-1, -1), (1, -1)]]
    for g in range(0, GORES, 2):
        ang = 2 * math.pi * g / GORES
        top = V((r0 * math.cos(ang), r0 * math.sin(ang), mouth_z))
        cx, cy = min(corners, key=lambda cc: (cc[0] - top.x) ** 2 + (cc[1] - top.y) ** 2)
        tube(bm, layer, (cx, cy, frame_z + 0.05), top, 0.012 * scale ** 0.5, DARK_STEEL, n=4, caps=False)
    # The skirt's own lines down to the frame, and the crown line's end
    # (tied off at the basket in flight: left out).
    return make_object(name, bm, coll)


# -------------------------------------------------------------------- the rigs
def burner_unit(bm, layer, cx, cy, z, r=0.16, h=0.30):
    """One burner: a stainless coil (as a ribbed cylinder), its jets on top,
    the blast valve handle on the side."""
    prof = []
    for i in range(9):
        zz = h * i / 8
        prof.append((r * (1.0 if i % 2 == 0 else 0.93), zz))
    lathe(bm, layer, [(0.0, 0.0)] + prof + [(r * 0.55, h + 0.02), (0.0, h + 0.03)], 12,
          lambda k, i: STEEL if k % 2 == 0 else tuple(x * 0.7 for x in STEEL), z0=z, centre=(cx, cy))
    # Blast valve and handle.
    box(bm, layer, (cx + r, cy - 0.03, z + 0.05), (cx + r + 0.12, cy + 0.03, z + 0.12), DARK_STEEL)
    tube(bm, layer, (cx + r + 0.10, cy, z + 0.08), (cx + r + 0.10, cy, z - 0.18), 0.012, BLACK, n=5)


def rig_sport(coll):
    bm = bmesh.new()
    layer = colour_layer(bm)
    w, l, h = S_BASKET
    hx, hy = w / 2, l / 2
    # Floor (plywood on runners) and the hide-covered skids.
    box(bm, layer, (-hx, -hy, 0.0), (hx, hy, 0.05), FLOOR)
    for sx in (-1, 1):
        box(bm, layer, (sx * (hx - 0.12) - 0.04, -hy + 0.02, -0.06), (sx * (hx - 0.12) + 0.04, hy - 0.02, 0.0), LEATHER)
    # Wicker walls, outside and inside.
    pts = [(-hx, -hy), (hx, -hy), (hx, hy), (-hx, hy)]
    for i in range(4):
        a, b = pts[i], pts[(i + 1) % 4]
        wicker_wall(bm, layer, a, b, 0.02, h - 0.06, False)
        t = 0.06
        ia = (a[0] - math.copysign(t, a[0]), a[1] - math.copysign(t, a[1]))
        ib = (b[0] - math.copysign(t, b[0]), b[1] - math.copysign(t, b[1]))
        wicker_wall(bm, layer, ia, ib, 0.05, h - 0.06, True, (0.12, 0.08))
    # The padded leather rim, and rope handles.
    for i in range(4):
        a, b = pts[i], pts[(i + 1) % 4]
        tube(bm, layer, (a[0], a[1], h - 0.03), (b[0], b[1], h - 0.03), 0.065, LEATHER, n=8)
    for i in range(4):
        a, b = V((*pts[i], 0)), V((*pts[(i + 1) % 4], 0))
        for s in (0.3, 0.7):
            p = a.lerp(b, s)
            o = p.normalized() * 0.02
            tube(bm, layer, (p.x + o.x, p.y + o.y, h - 0.32), (p.x + o.x, p.y + o.y, h - 0.20), 0.018, CREAM, n=5)
    # Four propane cylinders in the corners (seen from above).
    for sx in (-1, 1):
        for sy in (-1, 1):
            cx, cy = sx * (hx - 0.22), sy * (hy - 0.22)
            lathe(bm, layer, [(0.0, 0.05), (0.15, 0.05), (0.155, 0.10), (0.155, 0.72), (0.12, 0.80), (0.05, 0.83), (0.05, 0.88), (0.0, 0.88)], 10,
                  lambda k, i: STEEL if k < 5 else DARK_STEEL, centre=(cx, cy))
    # The burner poles (nylon rods in padded sleeves) from the corners up to the frame.
    f = 0.36
    for sx in (-1, 1):
        for sy in (-1, 1):
            tube(bm, layer, (sx * (hx - 0.05), sy * (hy - 0.05), h), (sx * f, sy * f, S_FRAME_Z), 0.035, SUEDE, n=6)
    # The burner frame: a square of stainless tube.
    sq = [(f, f), (-f, f), (-f, -f), (f, -f)]
    for i in range(4):
        a, b = sq[i], sq[(i + 1) % 4]
        tube(bm, layer, (a[0], a[1], S_FRAME_Z), (b[0], b[1], S_FRAME_Z), 0.022, STEEL, n=6)
    tube(bm, layer, (-f, 0, S_FRAME_Z), (f, 0, S_FRAME_Z), 0.02, STEEL, n=6)
    # Double burner, nozzle top at S_NOZZLE_Z.
    for cx in (-0.17, 0.17):
        burner_unit(bm, layer, cx, 0.0, S_NOZZLE_Z - 0.33)
    # Fuel hoses from the burner down into the basket.
    for cx in (-0.17, 0.17):
        tube(bm, layer, (cx + 0.05, 0.12, S_NOZZLE_Z - 0.33), (cx * 2.4, hy - 0.28, 0.95), 0.016, BLACK, n=5)
    # The pilot and two passengers.
    person(bm, layer, 0.28, -0.35, 0.05, JACKETS[0], 1.80, 0.2, 0.1)
    person(bm, layer, -0.30, -0.30, 0.05, JACKETS[1], 1.66, -0.4, 0.6)
    person(bm, layer, 0.05, 0.42, 0.05, JACKETS[3], 1.74, 3.0, 0.3)
    ob = make_object("rig sport", bm, coll)
    ob["pivot"] = (0.0, 0.0, S_NOZZLE_Z)
    ob["axis"] = (0.0, 0.0, S_MOUTH_Z)
    return ob


def rig_large(coll):
    bm = bmesh.new()
    layer = colour_layer(bm)
    w, l, h = L_BASKET
    hx, hy = w / 2, l / 2
    box(bm, layer, (-hx, -hy, 0.0), (hx, hy, 0.06), FLOOR)
    for sy in (-1, 0, 1):
        box(bm, layer, (-hx + 0.05, sy * (hy - 0.4) - 0.05, -0.08), (hx - 0.05, sy * (hy - 0.4) + 0.05, 0.0), LEATHER)
    pts = [(-hx, -hy), (hx, -hy), (hx, hy), (-hx, hy)]
    for i in range(4):
        a, b = pts[i], pts[(i + 1) % 4]
        wicker_wall(bm, layer, a, b, 0.02, h - 0.07, False, (0.09, 0.05))
        t = 0.07
        ia = (a[0] - math.copysign(t, a[0]), a[1] - math.copysign(t, a[1]))
        ib = (b[0] - math.copysign(t, b[0]), b[1] - math.copysign(t, b[1]))
        wicker_wall(bm, layer, ia, ib, 0.06, h - 0.07, True, (0.15, 0.10))
        tube(bm, layer, (a[0], a[1], h - 0.035), (b[0], b[1], h - 0.035), 0.075, LEATHER, n=8)
    # Compartments: the pilot's across the middle (with the cylinders), two
    # passenger compartments each end, split along the length.
    pc = 0.55
    for sy in (-1, 1):
        for side in (False, True):
            wicker_wall(bm, layer, (-hx, sy * pc), (hx, sy * pc), 0.06, h - 0.07, side, (0.15, 0.10))
        tube(bm, layer, (-hx, sy * pc, h - 0.035), (hx, sy * pc, h - 0.035), 0.06, LEATHER, n=8)
        for side in (False, True):
            a, b = (0.0, sy * pc), (0.0, sy * hy)
            wicker_wall(bm, layer, a, b, 0.06, h - 0.07, side, (0.15, 0.10))
        tube(bm, layer, (0.0, sy * pc, h - 0.035), (0.0, sy * hy, h - 0.035), 0.06, LEATHER, n=8)
    for sx in (-1, 1):
        for cy in (-0.28, 0.28):
            lathe(bm, layer, [(0.0, 0.06), (0.16, 0.06), (0.165, 0.12), (0.165, 0.80), (0.12, 0.88), (0.05, 0.91), (0.0, 0.95)], 10,
                  lambda k, i: STEEL if k < 5 else DARK_STEEL, centre=(sx * (hx - 0.25), cy))
    # Poles: six, to a larger frame.
    f = 0.62
    for sx in (-1, 1):
        for sy in (-1, 1):
            tube(bm, layer, (sx * (hx - 0.06), sy * (pc + 0.05), h), (sx * f, sy * f, L_FRAME_Z), 0.045, SUEDE, n=6)
    sq = [(f, f), (-f, f), (-f, -f), (f, -f)]
    for i in range(4):
        a, b = sq[i], sq[(i + 1) % 4]
        tube(bm, layer, (a[0], a[1], L_FRAME_Z), (b[0], b[1], L_FRAME_Z), 0.028, STEEL, n=6)
    for sx in (-1, 1):
        tube(bm, layer, (sx * f, -f, L_FRAME_Z), (sx * f * 0.5, 0, L_FRAME_Z), 0.022, STEEL, n=6)
    # Quad burner.
    for cx in (-0.2, 0.2):
        for cy in (-0.2, 0.2):
            burner_unit(bm, layer, cx, cy, L_NOZZLE_Z - 0.36, r=0.18, h=0.33)
    # The pilot in the middle, passengers in the four compartments.
    person(bm, layer, 0.3, 0.0, 0.06, JACKETS[2], 1.82, 0.0, 0.2)
    seed = 0.13
    k = 0
    for sy in (-1, 1):
        for sx in (-1, 1):
            for (px, py) in [(0.33, 0.95), (0.70, 0.95), (0.33, 1.55), (0.72, 1.60), (0.50, 2.0)]:
                seed = (seed * 9301 + 49297) % 233280 / 233280
                person(bm, layer, sx * px * (hx - 0.1) / 0.9, sy * min(py, hy - 0.25), 0.06, JACKETS[k % len(JACKETS)], 1.6 + 0.25 * seed, seed * 6.28, seed)
                k += 1
    ob = make_object("rig large", bm, coll)
    ob["pivot"] = (0.0, 0.0, L_NOZZLE_Z)
    ob["axis"] = (0.0, 0.0, L_MOUTH_Z)
    return ob


def rig_far(coll):
    bm = bmesh.new()
    layer = colour_layer(bm)
    w, l, h = S_BASKET
    box(bm, layer, (-w / 2, -l / 2, 0.0), (w / 2, l / 2, h), WICKER)
    f = 0.36
    for sx in (-1, 1):
        for sy in (-1, 1):
            tube(bm, layer, (sx * w / 2, sy * l / 2, h), (sx * f, sy * f, S_FRAME_Z), 0.05, SUEDE, n=3, caps=False)
    box(bm, layer, (-0.35, -0.2, S_FRAME_Z), (0.35, 0.2, S_NOZZLE_Z), STEEL)
    return make_object("rig far", bm, coll)


def flame(coll):
    """The burner flame, 1 m nominal (the game scales and flickers it): a
    blue root, a white-yellow core, orange, and red-tinged tips."""
    bm = bmesh.new()
    layer = colour_layer(bm)
    prof = [(0.0, 0.0), (0.05, 0.0), (0.09, 0.08), (0.14, 0.22), (0.17, 0.40), (0.16, 0.58), (0.12, 0.74), (0.075, 0.87), (0.03, 0.96), (0.0, 1.0)]
    cols = [c(0.25, 0.45, 1.0), c(1.0, 0.95, 0.75), c(1.0, 0.85, 0.45), c(1.0, 0.62, 0.16), c(1.0, 0.45, 0.06), c(1.0, 0.33, 0.03), c(0.95, 0.22, 0.02), c(0.80, 0.12, 0.01), c(0.60, 0.06, 0.0)]
    lathe(bm, layer, [(r, z) for r, z in prof], 10, lambda k, i: cols[min(k, len(cols) - 1)])
    return make_object("flame", bm, coll)


# ---------------------------------------------------------------------- build
EXPECTED = [f"env {s}" for s in SCHEMES] + [f"env {s} far" for s in SCHEMES] + ["skirt", "wires sport", "wires large", "rig sport", "rig large", "rig far", "flame"]


def build():
    for ob in list(bpy.data.objects):
        bpy.data.objects.remove(ob, do_unlink=True)
    coll = bpy.context.scene.collection
    for i, s in enumerate(SCHEMES):
        ob = envelope(f"env {s}", s, True, coll)
        ob.location = (i % 4 * 24.0, i // 4 * 24.0, S_MOUTH_Z)
        far = envelope(f"env {s} far", s, False, coll)
        far.location = (i % 4 * 24.0, i // 4 * 24.0 - 70.0, S_MOUTH_Z)
    sk = skirt(coll)
    sk.location = (0.0, 0.0, S_MOUTH_Z)
    wires("wires sport", 0.36, S_FRAME_Z, S_MOUTH_Z, 1.0, coll)
    wl = wires("wires large", 0.62, L_FRAME_Z, L_MOUTH_Z, LARGE_SCALE, coll)
    wl.location = (0.0, -150.0, 0.0)
    rig_sport(coll)
    rl = rig_large(coll)
    rl.location = (0.0, -150.0, 0.0)
    rf = rig_far(coll)
    rf.location = (0.0, -70.0, 0.0)
    fl = flame(coll)
    fl.location = (0.0, 0.0, S_NOZZLE_Z)
    # The passenger balloon's envelope, for looking at (not exported: the
    # game scales "env <scheme>").
    big = bpy.data.objects["env harlequin"].copy()
    big.name = "preview large envelope"
    big.scale = (LARGE_SCALE,) * 3
    big.location = (0.0, -150.0, L_MOUTH_Z)
    coll.objects.link(big)
    big_skirt = sk.copy()
    big_skirt.name = "preview large skirt"
    big_skirt.scale = (LARGE_SCALE,) * 3
    big_skirt.location = (0.0, -150.0, L_MOUTH_Z)
    coll.objects.link(big_skirt)


def game(v):
    return (v[0], v[2], -v[1])


def export(path):
    out = bytearray(b"FFAC")
    obs = [o for o in bpy.data.objects if o.type == "MESH"]
    names = {o.name for o in obs}
    missing = [n for n in EXPECTED if n not in names]
    if missing:
        raise SystemExit(f"balloons export: the game needs these pieces: {', '.join(missing)}")
    obs = [bpy.data.objects[n] for n in EXPECTED]
    out += struct.pack("<II", 1, len(obs))
    total = 0
    for ob in obs:
        me = ob.data
        me.calc_loop_triangles()
        col = me.color_attributes.get("Col")
        # Exported about the piece's own origin (its placement in the .blend
        # is only for looking at them side by side).
        nm = V((1, 1, 1))
        verts, index, idx = [], [], {}
        for tri in me.loop_triangles:
            for k in range(3):
                vi, li = tri.vertices[k], tri.loops[k]
                n = V(tri.split_normals[k]).normalized()
                cc = tuple(round(x, 4) for x in col.data[li].color[:3]) if col else (0.5, 0.5, 0.5)
                key = (vi, round(n.x, 3), round(n.y, 3), round(n.z, 3), cc)
                if key not in idx:
                    idx[key] = len(verts)
                    verts.append((*game(me.vertices[vi].co), *game(n), *cc, 0.0, 0.0, 0.0, 0.0))
                index.append(idx[key])
        pivot = V(ob.get("pivot", (0.0, 0.0, 0.0)))
        axis = V(ob.get("axis", (0.0, 0.0, 1.0)))
        name = ob.name.encode()
        out += struct.pack("<I", len(name)) + name
        # (The rigs' "axis" is the mouth's position, not a direction: as it is.)
        out += struct.pack("<7f", *game(pivot), *game(axis), 0.0)
        out += struct.pack("<I", len(verts))
        for v in verts:
            out += struct.pack("<13f", *v)
        out += struct.pack("<I", len(index))
        out += struct.pack("<%dI" % len(index), *index)
        total += len(index) // 3
        print(f"  {ob.name}: {len(index) // 3} triangles, {len(verts)} vertices")
    with gzip.open(path, "wb", compresslevel=9) as f:
        f.write(bytes(out))
    print("wrote", path, len(out), "bytes raw,", total, "triangles")


# --------------------------------------------------------------------- render
def setup_look():
    """Materials for looking at the .blend (and --render): the colour
    attribute as albedo, fabric a little translucent, the flame glowing."""
    for ob in bpy.data.objects:
        if ob.type != "MESH":
            continue
        name = "flame" if ob.name.startswith("flame") else ("fabric" if ob.name.startswith(("env", "preview", "skirt")) else "solid")
        mat = bpy.data.materials.get(name)
        if mat is None:
            mat = bpy.data.materials.new(name)
            mat.use_nodes = True
            nt = mat.node_tree
            bsdf = next(n for n in nt.nodes if n.type == "BSDF_PRINCIPLED")
            attr = nt.nodes.new("ShaderNodeVertexColor")
            attr.layer_name = "Col"
            if name == "flame":
                out = next(n for n in nt.nodes if n.type == "OUTPUT_MATERIAL")
                em = nt.nodes.new("ShaderNodeEmission")
                em.inputs["Strength"].default_value = 25.0
                nt.links.new(attr.outputs["Color"], em.inputs["Color"])
                nt.links.new(em.outputs["Emission"], out.inputs["Surface"])
            else:
                nt.links.new(attr.outputs["Color"], bsdf.inputs["Base Color"])
                bsdf.inputs["Roughness"].default_value = 0.55 if name == "fabric" else 0.8
                if name == "fabric":
                    for key in ("Subsurface Weight", "Transmission Weight"):
                        if key in bsdf.inputs:
                            pass
                    if "Sheen Weight" in bsdf.inputs:
                        bsdf.inputs["Sheen Weight"].default_value = 0.2
            mat.use_backface_culling = False
        ob.data.materials.clear()
        ob.data.materials.append(mat)


def render(outdir):
    os.makedirs(outdir, exist_ok=True)
    sc = bpy.context.scene
    engines = [e.identifier for e in bpy.types.RenderSettings.bl_rna.properties["engine"].enum_items]
    sc.render.engine = "CYCLES" if "CYCLES" in engines else engines[0]
    if sc.render.engine == "CYCLES":
        sc.cycles.samples = 48
        sc.cycles.use_denoising = True
    sc.render.resolution_x, sc.render.resolution_y = 1000, 1250
    world = bpy.data.worlds.new("sky")
    world.use_nodes = True
    nt = world.node_tree
    bg = next(n for n in nt.nodes if n.type == "BACKGROUND")
    try:
        sky = nt.nodes.new("ShaderNodeTexSky")
        nt.links.new(sky.outputs["Color"], bg.inputs["Color"])
        bg.inputs["Strength"].default_value = 0.35
    except Exception:
        bg.inputs["Color"].default_value = (0.35, 0.55, 0.9, 1)
    sc.world = world
    sun = bpy.data.objects.new("sun", bpy.data.lights.new("sun", "SUN"))
    sun.data.energy = 4.0
    sun.rotation_euler = (math.radians(55), 0, math.radians(35))
    sc.collection.objects.link(sun)
    try:
        sc.view_settings.view_transform = "AgX"
    except Exception:
        pass
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
    sc.collection.objects.link(cam)
    sc.camera = cam

    def shot(fname, loc, target, lens=35, only=None):
        if only is not None:
            for ob in bpy.data.objects:
                if ob.type == "MESH":
                    ob.hide_render = ob.name not in only
        cam.location = V(loc)
        d = V(target) - V(loc)
        cam.rotation_euler = d.to_track_quat("-Z", "Y").to_euler()
        cam.data.lens = lens
        sc.render.filepath = os.path.join(outdir, fname)
        bpy.ops.render.render(write_still=True)

    def balloon(scheme, at, large=False, flame_len=0.0, yaw=0.0):
        """A whole balloon (linked copies) with its basket floor at `at`."""
        k = LARGE_SCALE if large else 1.0
        mouth = L_MOUTH_Z if large else S_MOUTH_Z
        nozzle = L_NOZZLE_Z if large else S_NOZZLE_Z
        parts = [(f"env {scheme}", mouth, k), ("skirt", mouth, k), ("wires large" if large else "wires sport", 0.0, 1.0), ("rig large" if large else "rig sport", 0.0, 1.0)]
        if flame_len > 0:
            parts.append(("flame", nozzle, flame_len))
        made = []
        for name, z, sc_ in parts:
            ob = bpy.data.objects[name].copy()
            ob.location = (at[0], at[1], at[2] + z)
            ob.scale = (sc_ * (0.5 if name == "flame" else 1.0), sc_ * (0.5 if name == "flame" else 1.0), sc_) if name == "flame" else (sc_,) * 3
            ob.rotation_euler = (0, 0, yaw)
            sc.collection.objects.link(ob)
            made.append(ob.name)
        return made

    for ob in bpy.data.objects:
        if ob.type == "MESH":
            ob.hide_render = True
    one = balloon("harlequin", (0, 0, 0), flame_len=3.2)
    shot("sport_below.png", (10.0, -26.0, -1.0), (0.0, 0.0, 11.0), 26, one)
    shot("basket.png", (3.4, -4.2, 1.9), (0.0, 0.0, 1.9), 32, one)
    shot("burner.png", (1.2, -1.6, 2.0), (0.0, 0.0, 3.2), 24, one)
    big = balloon("rainbow", (100, 0, 0), large=True, flame_len=4.0)
    shot("large.png", (118.0, -42.0, 2.0), (100.0, 0.0, 17.0), 26, big)
    shot("large_basket.png", (104.5, -6.5, 3.6), (100.0, 0.0, 1.2), 30, big)
    # A morning launch: many balloons at different heights and distances.
    fleet = []
    import random
    rnd = random.Random(7)
    for i in range(26):
        s_ = SCHEMES[i % len(SCHEMES)]
        d = rnd.uniform(60, 700)
        az = rnd.uniform(-0.55, 0.55)
        x, y = 300 + d * math.sin(az), d * math.cos(az)
        z = rnd.uniform(20, 380)
        fleet += balloon(s_, (x, y, z), large=rnd.random() < 0.3, flame_len=3.0 if rnd.random() < 0.3 else 0.0, yaw=rnd.uniform(0, 6.28))
    sc.render.resolution_x, sc.render.resolution_y = 1400, 1400
    shot("fleet.png", (300.0, -40.0, 40.0), (300.0, 400.0, 230.0), 30, fleet)
    # All the schemes side by side.
    line = []
    for i, s_ in enumerate(SCHEMES):
        line += balloon(s_, ((i - 3.5) * 21.0 + 1000, 0.0, 0.0))
    sc.render.resolution_x, sc.render.resolution_y = 1800, 700
    shot("schemes.png", (1000.0, -150.0, 16.0), (1000.0, 0.0, 14.0), 40, line)


def main():
    blend = os.path.join(ASSETS, "blender", "balloons.blend")
    if not EXPORT_ONLY:
        build()
        setup_look()
        bpy.ops.wm.save_as_mainfile(filepath=blend)
    export(os.path.join(ASSETS, "balloons.bin.gz"))
    if RENDER:
        render(RENDER)


main()
