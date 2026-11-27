#!/usr/bin/env python3
"""Cuts OpenStreetMap's power lines into the tiles src/power.rs reads.

Data © OpenStreetMap contributors, under the Open Database License (ODbL):
every way tagged power=line (transmission, on towers) or power=minor_line
(distribution, on poles). Each vertex of such a way is a support: a tower
or a pole. Fetch them all from QLever's OpenStreetMap endpoint (about two
minutes, 1.4 GB of tab-separated values):

    PREFIX osmkey: <https://www.openstreetmap.org/wiki/Key:>
    PREFIX geo: <http://www.opengis.net/ont/geosparql#>
    PREFIX osm: <https://www.openstreetmap.org/>
    PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
    SELECT ?way ?kind ?voltage ?cables ?wires ?wkt WHERE {
      ?way osmkey:power ?kind . VALUES ?kind { "line" "minor_line" }
      ?way rdf:type osm:way .
      ?way geo:hasGeometry/geo:asWKT ?wkt .
      OPTIONAL { ?way osmkey:voltage ?voltage }
      OPTIONAL { ?way osmkey:cables ?cables }
      OPTIONAL { ?way osmkey:wires ?wires }
    }

    curl -sSL --post301 --compressed -H 'Accept: text/tab-separated-values' \\
        --data-urlencode query@lines.sparql https://qlever.dev/api/osm-planet -o lines.tsv
    python3 assets/mkpower.py lines.tsv OUT_DIR

Writes OUT_DIR/power_<lat>_<lon>.bin: a bundle for each 10 x 10 degree
square some line crosses (its south-west corner, degrees), each holding the
1 x 1 degree tiles inside it, to upload as the assets of a GitHub release
(a release takes at most 1000 files; there are a few hundred bundles). The
game downloads the bundle round the aircraft and keeps it in its cache. A
way goes in every tile one of its spans touches, whole; the game keeps each
way once by its id.

Bundle format, little-endian:
  "FFPB" u32 version (1), u32 n_tiles
  per tile: i16 lat, i16 lon (the tile's south-west corner), u32 offset, u32 length
  then the tiles, each gzip'd:

  "FFPW" u32 version (1), u32 n_ways
  per way: u64 OSM id, u8 kind (0 line, 1 minor line), u16 voltage (kV,
      the highest the tag lists; 0 unknown), u8 cables (0 unknown),
      u32 n_points, then the points as zig-zag varint deltas of latitude
      and longitude in 1e-6 degrees (the first from 0)
"""

import gzip
import math
import os
import re
import struct
import sys
from collections import defaultdict

PT = re.compile(r"(-?[\d.]+) (-?[\d.]+)")


def num(s, scale=1.0):
    """The largest number a tag value lists (`400000;225000`, `20 kV`)."""
    best = 0.0
    for m in re.finditer(r"\d+(?:[.,]\d+)?", s or ""):
        best = max(best, float(m.group(0).replace(",", ".")))
    return best * scale


def varint(v, out):
    v = (v << 1) ^ (v >> 63)  # zig-zag
    v &= (1 << 64) - 1
    while v >= 0x80:
        out.append((v & 0x7F) | 0x80)
        v >>= 7
    out.append(v)


def main():
    src, out_dir = sys.argv[1], sys.argv[2]
    tiles = defaultdict(bytearray)
    counts = defaultdict(int)
    n = 0
    with open(src, encoding="utf-8", errors="replace") as f:
        next(f)
        for line in f:
            cols = line.rstrip("\n").split("\t")
            if len(cols) < 6:
                continue
            way, kind, volt, cables, _wires, wkt = cols[:6]
            m = re.search(r"/way/(\d+)", way)
            if not m or not wkt.startswith('"LINESTRING'):
                continue
            wid = int(m.group(1))
            pts = [(float(b), float(a)) for (a, b) in PT.findall(wkt[wkt.index("(") + 1:wkt.index(")")])]
            if len(pts) < 2:
                continue
            kv = num(volt.strip('"'))
            # (Volts, as OSM has them; a few lines give kilovolts.)
            kv = kv / 1000 if kv >= 1000 else kv
            rec = bytearray()
            rec += struct.pack("<QBHBI", wid, 0 if kind.strip('"') == "line" else 1, min(int(kv), 65535), min(int(num(cables.strip('"'))), 255), len(pts))
            plat = plon = 0
            for (la, lo) in pts:
                ila, ilo = int(round(la * 1e6)), int(round(lo * 1e6))
                varint(ila - plat, rec)
                varint(ilo - plon, rec)
                plat, plon = ila, ilo
            # Every cell a span passes through (spans are short: their ends,
            # and the cells between when a long one crosses several).
            cells = set()
            for (a, b) in zip(pts, pts[1:]):
                steps = max(1, int(max(abs(a[0] - b[0]), abs(a[1] - b[1])) / 0.25) + 1)
                for k in range(steps + 1):
                    t = k / steps
                    cells.add((math.floor(a[0] + (b[0] - a[0]) * t), math.floor(a[1] + (b[1] - a[1]) * t)))
            for c in cells:
                tiles[c] += rec
                counts[c] += 1
            n += 1
            if n % 500000 == 0:
                print(f"  {n} ways", flush=True)
    os.makedirs(out_dir, exist_ok=True)
    bundles = defaultdict(list)
    for (la, lo) in sorted(tiles):
        data = gzip.compress(b"FFPW" + struct.pack("<II", 1, counts[(la, lo)]) + bytes(tiles[(la, lo)]), 9)
        bundles[(la // 10 * 10, lo // 10 * 10)].append((la, lo, data))
    total = 0
    for (bla, blo), ts in sorted(bundles.items()):
        head = bytearray(b"FFPB" + struct.pack("<II", 1, len(ts)))
        off = 12 + 12 * len(ts)
        body = bytearray()
        for (la, lo, data) in ts:
            head += struct.pack("<hhII", la, lo, off + len(body), len(data))
            body += data
        with open(os.path.join(out_dir, f"power_{bla}_{blo}.bin"), "wb") as f:
            f.write(head + body)
        total += len(head) + len(body)
    print(f"{len(bundles)} bundles")
    print(f"{n} ways in {len(tiles)} tiles, {total / 1e6:.1f} MB: {out_dir}")


if __name__ == "__main__":
    main()
