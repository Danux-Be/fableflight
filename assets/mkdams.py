#!/usr/bin/env python3
"""Packs how tall the world's dams are into assets/dams.bin.gz, which
src/dams.rs reads to raise the dams the map tiles outline to their height.

The elevation data the game stands its terrain on is older than many dams
and their lakes, and the map tiles give a dam's outline (landuse "dam") but
not its height. This table gives it, for every dam that has one:

- OpenStreetMap (© OpenStreetMap contributors, ODbL): every way, relation
  or node tagged waterway=dam or man_made=dam with a `height` (taken as the
  height above the ground at its foot) or `ele` (its crest's elevation),
  or a `wikidata` link;
- Wikidata (CC0): the height (P2048) of the item an OpenStreetMap dam
  links to, where OpenStreetMap has none. Wikidata gives the structural
  height, from the foundation, often a good deal more than what shows
  above the ground; the game takes 85% of it.

Fetch both from QLever (seconds each), as tab-separated values:

    # osm.sparql, https://qlever.dev/api/osm-planet
    PREFIX osmkey: <https://www.openstreetmap.org/wiki/Key:>
    PREFIX geo: <http://www.opengis.net/ont/geosparql#>
    SELECT ?o ?height ?ele ?wd ?material ?wkt WHERE {
      { ?o osmkey:waterway "dam" } UNION { ?o osmkey:man_made "dam" }
      OPTIONAL { ?o osmkey:height ?height } OPTIONAL { ?o osmkey:ele ?ele }
      OPTIONAL { ?o osmkey:wikidata ?wd } OPTIONAL { ?o osmkey:material ?material }
      FILTER(BOUND(?height) || BOUND(?ele) || BOUND(?wd))
      ?o geo:hasGeometry/geo:asWKT ?wkt .
    }

    # wd.sparql, https://qlever.dev/api/wikidata
    PREFIX wd: <http://www.wikidata.org/entity/>
    PREFIX wdt: <http://www.wikidata.org/prop/direct/>
    PREFIX p: <http://www.wikidata.org/prop/>
    PREFIX psv: <http://www.wikidata.org/prop/statement/value/>
    PREFIX wikibase: <http://wikiba.se/ontology#>
    SELECT ?item ?h ?u WHERE {
      ?item wdt:P31/wdt:P279* wd:Q12323 .
      ?item p:P2048/psv:P2048 ?v . ?v wikibase:quantityAmount ?h . ?v wikibase:quantityUnit ?u .
    }

    curl -sSL --post301 -H 'Accept: text/tab-separated-values' --data-urlencode query@osm.sparql \\
        https://qlever.dev/api/osm-planet -o osm_dams.tsv
    curl -sSL --post301 -H 'Accept: text/tab-separated-values' --data-urlencode query@wd.sparql \\
        https://qlever.dev/api/wikidata -o wd_heights.tsv
    python3 assets/mkdams.py osm_dams.tsv wd_heights.tsv [assets/dams.bin.gz]

Format, little-endian, gzip:
  "FFDM" u32 version (1), u32 n
  per dam (sorted by latitude), 16 bytes:
      i32 lat, i32 lon (1e-6 degrees: the middle of its outline),
      u16 height (decimetres above its foot, 0 unknown),
      i16 crest elevation (m, -32768 unknown),
      u16 length (m, the outline's longest side, to 65535), u8 material
      (0 unknown, 1 concrete, 2 earth or rock fill), u8 source (1 OSM
      height, 2 OSM ele, 4 Wikidata)
"""

import gzip
import math
import re
import struct
import sys

NUM = re.compile(r"[-+]?\d+(?:[.,]\d+)?")
FEET = "http://www.wikidata.org/entity/Q3710"
METRE = "http://www.wikidata.org/entity/Q11573"


def metres(s):
    s = (s or "").strip('"')
    m = NUM.search(s)
    if not m:
        return None
    v = float(m.group(0).replace(",", "."))
    if "ft" in s or "'" in s:
        v *= 0.3048
    return v


def points(wkt):
    return [(float(a), float(b)) for a, b in re.findall(r"(-?[\d.]+) (-?[\d.]+)", wkt)]


def main():
    osm, wdf = sys.argv[1], sys.argv[2]
    out = sys.argv[3] if len(sys.argv) > 3 else "assets/dams.bin.gz"
    wd = {}
    with open(wdf) as f:
        next(f)
        for line in f:
            c = line.rstrip("\n").split("\t")
            if len(c) < 3:
                continue
            q = c[0].rsplit("/", 1)[-1].rstrip(">")
            try:
                h = float(c[1].split("^^")[0].strip('"'))
            except ValueError:
                continue
            u = c[2].strip("<>")
            if u == FEET:
                h *= 0.3048
            elif u != METRE:
                continue
            if 2.0 <= h <= 350.0:
                wd[q] = max(wd.get(q, 0.0), h)
    recs = {}
    with open(osm, encoding="utf-8", errors="replace") as f:
        next(f)
        for line in f:
            c = line.rstrip("\n").split("\t")
            if len(c) < 6:
                continue
            _, height, ele, q, material, wkt = c[:6]
            pts = points(wkt)
            if not pts:
                continue
            lon = sum(p[0] for p in pts) / len(pts)
            lat = sum(p[1] for p in pts) / len(pts)
            # (The longest stretch across the outline: a dam's length.)
            coslat = math.cos(math.radians(lat))
            length = 0.0
            if len(pts) > 1:
                xs = [(p[0] - lon) * coslat * 111_320 for p in pts]
                ys = [(p[1] - lat) * 110_574 for p in pts]
                length = max(math.hypot(xs[i] - xs[j], ys[i] - ys[j]) for i in range(0, len(pts), max(1, len(pts) // 40)) for j in range(0, len(pts), max(1, len(pts) // 40)))
            h = metres(height)
            e = metres(ele)
            src = 0
            if h is not None and 2.0 <= h <= 350.0:
                src |= 1
            else:
                h = None
            if e is not None and -400 < e < 6000:
                src |= 2
            else:
                e = None
            q = q.strip('"')
            if h is None and q in wd:
                h = wd[q] * 0.85
                src |= 4
            if h is None and e is None:
                continue
            mat = (material or "").strip('"').lower()
            m = 1 if mat in ("concrete", "masonry", "stone", "steel") else 2 if mat in ("earth", "rock", "soil", "gravel", "clay") else 0
            key = (round(lat, 4), round(lon, 4))
            old = recs.get(key)
            # (The same dam as an outline and a line: keep the one with more.)
            if old is None or (old[2] == 0 and h) or length > old[4]:
                recs[key] = (lat, lon, h or 0.0, e, length, m, src)
    rows = sorted(recs.values())
    b = bytearray(b"FFDM" + struct.pack("<II", 1, len(rows)))
    for (lat, lon, h, e, length, m, src) in rows:
        b += struct.pack("<iiHhHBB", int(round(lat * 1e6)), int(round(lon * 1e6)), min(int(round(h * 10)), 65535),
                         int(round(e)) if e is not None else -32768, min(int(length), 65535), m, src)
    with gzip.open(out, "wb", compresslevel=9) as f:
        f.write(bytes(b))
    n_h = sum(1 for r in rows if r[2] > 0)
    print(f"{len(rows)} dams ({n_h} with a height, {sum(1 for r in rows if r[6] & 4)} of them from Wikidata): {out}")


if __name__ == "__main__":
    main()
