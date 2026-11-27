//! Dams: the outlines OpenStreetMap gives them (the map tiles' landuse
//! "dam") raised into concrete or earth walls to their height.
//!
//! The elevation data the terrain stands on is older than many dams (the
//! Three Gorges Dam is not in it at all), and the tiles give no height, so
//! assets/dams.bin.gz (assets/mkdams.py: OpenStreetMap's `height` and `ele`
//! tags, Wikidata's heights; © OpenStreetMap contributors, ODbL; Wikidata
//! CC0) says how tall each one is. Where it has nothing, the dam stands as
//! high as the ground under its outline (the dams the elevation data does
//! hold), and is left to the terrain if that is no wall.
//!
//! The side that meets the water is the upstream face, plumb; the others
//! slope out to the valley floor as a gravity dam's downstream face does
//! (0.75 across for each metre down), or, for an earth or rock fill dam, as
//! an embankment (2 across). The crest is a road between parapets. The
//! concrete (buildings.wgsl, plain surface 9) is poured in lifts between
//! contraction joints and streaked where water has run down it.

use crate::buildings::{in_rings, point_in_ring, Ctx, Footprint, Mesh, K_PLAIN};
use crate::geo::TileId;
use crate::sources::{Sources, VecTile, DATA_ZOOM};
use std::collections::HashMap;
use glam::{Vec2, Vec3};
use std::io::Read;
use std::sync::OnceLock;

/// One dam from the table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dam {
    pub lat: f64,
    pub lon: f64,
    /// above its foot (m), 0 unknown
    pub height: f32,
    /// its crest's elevation (m), if known
    pub crest: Option<f32>,
    /// the outline's longest stretch (m)
    pub length: f32,
    /// 0 unknown, 1 concrete, 2 earth or rock fill
    pub material: u8,
}

/// The table, sorted by latitude.
pub fn table() -> &'static [Dam] {
    static T: OnceLock<Vec<Dam>> = OnceLock::new();
    T.get_or_init(|| parse(include_bytes!("../assets/dams.bin.gz")))
}

pub fn parse(gz: &[u8]) -> Vec<Dam> {
    let mut raw = Vec::new();
    if flate2::read::GzDecoder::new(gz).read_to_end(&mut raw).is_err() || raw.len() < 12 || &raw[0..4] != b"FFDM" {
        return Vec::new();
    }
    let n = u32::from_le_bytes(raw[8..12].try_into().unwrap()) as usize;
    raw[12..]
        .chunks_exact(16)
        .take(n)
        .map(|r| {
            let i32_at = |k: usize| i32::from_le_bytes(r[k..k + 4].try_into().unwrap());
            let crest = i16::from_le_bytes([r[10], r[11]]);
            Dam {
                lat: i32_at(0) as f64 * 1e-6,
                lon: i32_at(4) as f64 * 1e-6,
                height: u16::from_le_bytes([r[8], r[9]]) as f32 * 0.1,
                crest: (crest != i16::MIN).then_some(crest as f32),
                length: u16::from_le_bytes([r[12], r[13]]) as f32,
                material: r[14],
            }
        })
        .collect()
}

/// The table's dam nearest (lat, lon) (degrees), within `reach` metres.
pub fn nearest(lat: f64, lon: f64, reach: f64) -> Option<&'static Dam> {
    let t = table();
    // (Long dams reach further than `reach` from their middle: look as far
    // as the longest might, then by each one's own length.)
    let dlat = reach.max(4_000.0) / 111_000.0;
    let lo = t.partition_point(|d| d.lat < lat - dlat);
    let coslat = lat.to_radians().cos().max(0.05);
    t[lo..]
        .iter()
        .take_while(|d| d.lat <= lat + dlat)
        .map(|d| (d, ((d.lat - lat) * 111_000.0).hypot((d.lon - lon) * 111_000.0 * coslat)))
        .filter(|(d, m)| *m < reach.max(d.length as f64 * 0.6))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(d, _)| d)
}

/// The concrete's colour, and the surface kind: buildings.wgsl's plain surface, dam (9).
const CONCRETE: [u8; 4] = [176, 172, 162, K_PLAIN];
const FILL: [u8; 4] = [128, 124, 104, K_PLAIN];
const ROAD: [u8; 4] = [96, 95, 92, K_PLAIN];
const SUB_DAM: u8 = 9;

/// A face seen from both sides (the building pipeline culls back faces,
/// and a dam's faces turn every way).
fn two(mesh: &mut Mesh, pts: &[(Vec3, [f32; 2])], col: [u8; 4], sty: [u8; 4], up: Option<Vec3>) {
    mesh.poly(false, pts, col, sty, up);
    let rev: Vec<(Vec3, [f32; 2])> = pts.iter().rev().copied().collect();
    mesh.poly(false, &rev, col, sty, up);
}

/// A dam's walls, crest and parapets in the tile's mesh, and its footprint
/// (so aircraft hit it and nothing is built or grown on it). Returns the
/// outlines it built, in tile units.
pub fn build(cx: &Ctx, vt: &VecTile, src: &Sources, mesh: &mut Mesh, footprints: &mut Vec<Footprint>) -> Vec<Vec<[f32; 2]>> {
    let mut built = Vec::new();
    // The elevation anywhere (a dam may run over several tiles).
    let mut elev: HashMap<TileId, Option<std::sync::Arc<crate::sources::ElevTile>>> = HashMap::new();
    let mut dem = |lat: f64, lon: f64| -> Option<f32> {
        let t = TileId::at(lat.to_radians(), lon.to_radians(), DATA_ZOOM);
        let e = elev.entry(t).or_insert_with(|| src.elev(t).ok()).clone()?;
        let (u, v) = t.frac(lat.to_radians(), lon.to_radians());
        Some(e.sample(u, v))
    };
    let ext = cx.ext;
    let um = cx.unit_m;
    let waters: Vec<&Vec<Vec<[f32; 2]>>> = vt.water.iter().filter(|w| w.0 >= 2).map(|w| &w.1).collect();
    let in_water = |p: [f32; 2]| waters.iter().any(|rings| in_rings(p, rings));
    // The lakes, at their surfaces (Sources::lake_level).
    let lakes = src.lakes_of(cx.id, vt);
    let lake_at = |p: [f32; 2]| lakes.iter().filter(|(_, r)| in_rings(p, r)).map(|(l, _)| *l).reduce(f32::max);
    for (_, rings) in vt.landuse.iter().filter(|l| l.0 == crate::sources::LU_DAM) {
        let Some(outer) = rings.first() else { continue };
        let mut ring: Vec<[f32; 2]> = outer.clone();
        if ring.len() > 1 && ring[0] == ring[ring.len() - 1] {
            ring.pop();
        }
        if ring.len() < 3 {
            continue;
        }
        let n = ring.len();
        let c = ring.iter().fold([0.0f32; 2], |a, p| [a[0] + p[0] / n as f32, a[1] + p[1] / n as f32]);
        let (lat, lon) = cx.id.lat_lon((c[0] / ext) as f64, (c[1] / ext) as f64);
        let rec = nearest(lat.to_degrees(), lon.to_degrees(), 400.0);
        // Outward, by the outline's winding.
        let area2: f32 = (0..n).map(|i| {
            let (a, b) = (ring[i], ring[(i + 1) % n]);
            a[0] * b[1] - b[0] * a[1]
        }).sum();
        let ccw = area2 > 0.0;
        let out_n = |i: usize| {
            let (a, b) = (Vec2::from(ring[i]), Vec2::from(ring[(i + 1) % n]));
            let d = (b - a).normalize_or_zero();
            if ccw { Vec2::new(d.y, -d.x) } else { Vec2::new(-d.y, d.x) }
        };
        let at = |p: Vec2, dir: Vec2, m: f32| -> [f32; 2] { (p + dir * (m / um)).into() };
        // The ground: highest under the outline (the dam itself, where the
        // elevation data holds it), lowest round it (the valley floor).
        let mut top_g = cx.ground(c);
        let mut foot = f32::MAX;
        for (i, &p) in ring.iter().enumerate() {
            let a = Vec2::from(p);
            top_g = top_g.max(cx.ground(p));
            for m in [40.0, 100.0, 200.0] {
                foot = foot.min(cx.ground(at(a, out_n(i), m)));
            }
        }
        // A dam the table knows stands as high as it says over the valley
        // floor round the middle it gives, the same from every tile it
        // crosses; one it does not know, as high as the ground under it.
        let rec_foot = rec.map(|d| {
            let r = (d.length as f64 * 0.6).max(150.0);
            let coslat = d.lat.to_radians().cos().max(0.05);
            let mut f = f32::MAX;
            for k in 0..24 {
                let a = std::f64::consts::TAU * k as f64 / 24.0;
                for s in [0.35, 0.7, 1.0] {
                    let (dy, dx) = (a.sin() * r * s / 111_000.0, a.cos() * r * s / (111_000.0 * coslat));
                    if let Some(h) = dem(d.lat + dy, d.lon + dx) {
                        f = f.min(h);
                    }
                }
            }
            f
        });
        if let Some(f) = rec_foot.filter(|f| *f < f32::MAX) {
            foot = f;
        }
        let mut crest = match rec {
            Some(d) if d.height > 0.0 => foot + d.height,
            Some(Dam { crest: Some(e), .. }) if *e > foot + 3.0 && *e < foot + 400.0 => *e,
            _ => top_g,
        };
        if crest - foot < 4.0 {
            continue;
        }
        let fill = rec.is_some_and(|d| d.material == 2);
        let col = if fill { FILL } else { CONCRETE };
        let sty = [((c[0] * 13.0 + c[1] * 7.0) as u32 & 255) as u8, SUB_DAM, 128, (crest - foot).min(255.0) as u8];
        let upv = (cx.pos(c, crest) + cx.center.as_vec3()).normalize();
        let pos = |p: [f32; 2], z: f32| cx.pos(p, z);
        // (Edges on the tile's border are where a neighbour's piece of
        // the same dam carries on: no wall there.)
        let border = |a: [f32; 2], b: [f32; 2]| {
            let e = 0.002 * ext;
            (a[0] <= e && b[0] <= e) || (a[0] >= ext - e && b[0] >= ext - e) || (a[1] <= e && b[1] <= e) || (a[1] >= ext - e && b[1] >= ext - e)
        };
        // Which way is upstream: across the dam's length, the side with
        // more water (the lake; the river below is narrower).
        let (mut ia, mut ib, mut best) = (0, 0, 0.0f32);
        for i in 0..n {
            for j in i + 1..n {
                let d = Vec2::from(ring[i]).distance_squared(Vec2::from(ring[j]));
                if d > best {
                    (ia, ib, best) = (i, j, d);
                }
            }
        }
        let axis = (Vec2::from(ring[ib]) - Vec2::from(ring[ia])).normalize_or_zero();
        let across = Vec2::new(-axis.y, axis.x);
        // How much water each side, and the highest lake there.
        let wet = |sgn: f32| {
            let (mut k, mut lv) = (0, None::<f32>);
            for t in [0.3f32, 0.5, 0.7] {
                let p = Vec2::from(ring[ia]).lerp(Vec2::from(ring[ib]), t);
                for m in (2..=12).map(|j| j as f32 * 50.0) {
                    let q = at(p, across * sgn, m);
                    k += in_water(q) as i32;
                    if let Some(l) = lake_at(q) {
                        lv = Some(lv.map_or(l, |v: f32| v.max(l)));
                    }
                }
            }
            (k, lv)
        };
        let ((wp, lp), (wm, lm)) = (wet(1.0), wet(-1.0));
        // Edge by edge, the lake just before it (if any).
        let edge_lv: Vec<Option<f32>> = (0..n)
            .map(|i| {
                let mid = Vec2::from(ring[i]).lerp(Vec2::from(ring[(i + 1) % n]), 0.5);
                [20.0f32, 45.0].iter().filter_map(|m| lake_at(at(mid, out_n(i), *m))).reduce(f32::max)
            })
            .collect();
        let top_lake = edge_lv.iter().flatten().copied().reduce(f32::max);
        let (up_edge, up_level): (Vec<bool>, Option<f32>) = if let Some(top) = top_lake {
            // The upstream faces are the edges the highest lake it touches
            // comes up against, whatever the outline's shape. Round a lake
            // on every side, it is a mole or a breakwater: left alone.
            let up: Vec<bool> = (0..n).map(|i| edge_lv[i].is_some_and(|l| l >= top - 1.0) && !border(ring[i], ring[(i + 1) % n])).collect();
            let below = (0..n).any(|i| !border(ring[i], ring[(i + 1) % n]) && edge_lv[i].is_none_or(|l| l < top - 3.0));
            if below { (up, Some(top)) } else { (vec![false; n], None) }
        } else {
            // No lake: the side with clearly more water (a river) is upstream,
            // and the edges facing it with nothing of the outline before them.
            let up_dir = if wp as f32 > wm as f32 * 1.5 + 2.0 { across } else if wm as f32 > wp as f32 * 1.5 + 2.0 { -across } else { Vec2::ZERO };
            if up_dir == Vec2::ZERO {
                (vec![false; n], None)
            } else {
                let front = |i: usize| {
                    let mid = Vec2::from(ring[i]).lerp(Vec2::from(ring[(i + 1) % n]), 0.5);
                    !(1..=30).any(|k| point_in_ring(at(mid, up_dir, k as f32 * 15.0), &ring))
                };
                ((0..n).map(|i| out_n(i).dot(up_dir) > 0.3 && !border(ring[i], ring[(i + 1) % n]) && front(i)).collect(), None)
            }
        };
        let _ = (lp, lm);
        let sectioned = up_edge.iter().any(|b| *b);
        // One the table knows that none of this can place is left on the
        // ground rather than raised wrong.
        if !sectioned && rec.is_some() {
            continue;
        }
        // The crest stands clear of the lake it holds back. (One the table
        // does not know: as high as the lake and its freeboard, not as the
        // hillsides its outline runs onto.)
        if let Some(l) = up_level {
            let free = (0.06 * (crest - foot)).clamp(3.0, 12.0);
            crest = if rec.is_some() { crest.max(l + free) } else { l + free };
        }
        if sectioned {
            // A section along the upstream faces: what else the outline
            // holds downstream (training walls, powerhouses, aprons) stays
            // on the ground.
                // Runs of consecutive upstream edges, as their vertices.
            let start = (0..n).find(|&i| !up_edge[i]).unwrap_or(0);
            let mut runs: Vec<Vec<usize>> = Vec::new();
            let mut cur: Vec<usize> = Vec::new();
            for k in 0..=n {
                let i = (start + k) % n;
                if k < n && up_edge[i] {
                    if cur.is_empty() {
                        cur.push(i);
                    }
                    cur.push((i + 1) % n);
                } else if cur.len() >= 2 {
                    runs.push(std::mem::take(&mut cur));
                } else {
                    cur.clear();
                }
            }
            let height = crest - foot;
            let cw = if fill { 10.0 } else { (0.12 * height).clamp(6.0, 30.0) };
            let (up_s, down_s) = if fill { (2.5, 2.0) } else { (0.0, 0.75) };
            for run in runs {
                let m = run.len();
                // Downstream at each vertex: the mitred mean of its edges' inward normals.
                let dn: Vec<Vec2> = (0..m)
                    .map(|k| {
                        let e = |j: usize| -out_n(run[j]);
                        let (e0, e1) = (if k > 0 { e(k - 1) } else { e(0) }, if k + 1 < m { e(k) } else { e(m - 2) });
                        let v = (e0 + e1).normalize_or_zero();
                        let v = if v == Vec2::ZERO { e0 } else { v };
                        v / v.dot(e0).max(0.5)
                    })
                    .collect();
                // The sloping faces run out to where they meet the ground.
                let reach = |p: Vec2, d: Vec2, s: f32| -> ([f32; 2], f32) {
                    if s <= 0.0 {
                        return (p.into(), cx.ground(p.into()) - 2.0);
                    }
                    let g0 = cx.ground(p.into());
                    let q = at(p, d, s * (crest - g0).max(0.0));
                    let g = cx.ground(q);
                    (at(p, d, s * (crest - g).max(0.0)), g - 2.0)
                };
                // Every 15 m or so along the run, so the faces' feet follow the
                // ground (a valley between two corners on the hillsides).
                let mut dense: Vec<(Vec2, Vec2, f32)> = Vec::new();
                for k in 0..m - 1 {
                    let (p0, p1) = (Vec2::from(ring[run[k]]), Vec2::from(ring[run[k + 1]]));
                    let len = p0.distance(p1) * um;
                    let steps = (len / 15.0).ceil().max(1.0) as usize;
                    let d0 = dense.last().map_or(0.0, |l| l.2);
                    for j in 0..steps {
                        let t = j as f32 / steps as f32;
                        dense.push((p0.lerp(p1, t), if j == 0 { dn[k] } else { -out_n(run[k]) }, d0 + len * t));
                    }
                    if k + 2 == m {
                        dense.push((p1, dn[m - 1], d0 + len));
                    }
                }
                let m = dense.len();
                let mut sec = Vec::with_capacity(m);
                let mut dist = vec![0.0f32; m];
                for (k, &(p, d, l)) in dense.iter().enumerate() {
                    dist[k] = l;
                    let ce: [f32; 2] = at(p, d, cw);
                    let (ub, uz) = reach(p, -d, up_s);
                    let (db, dz) = reach(Vec2::from(ce), d, down_s);
                    sec.push((<[f32; 2]>::from(p), ce, ub, uz, db, dz));
                }
                for k in 0..m - 1 {
                    let (s0, s1) = (sec[k], sec[k + 1]);
                    let (l0, l1) = (dist[k], dist[k + 1]);
                    // Upstream face, crest, downstream face.
                    two(mesh, &[(pos(s0.0, crest), [l0, 0.0]), (pos(s1.0, crest), [l1, 0.0]), (pos(s1.2, s1.3), [l1, crest - s1.3]), (pos(s0.2, s0.3), [l0, crest - s0.3])], col, sty, None);
                    let (m0, m1) = (cx.metres(s0.0), cx.metres(s1.1));
                    two(mesh, &[(pos(s0.0, crest), [m0.x, m0.y]), (pos(s1.0, crest), [cx.metres(s1.0).x, cx.metres(s1.0).y]), (pos(s1.1, crest), [m1.x, m1.y]), (pos(s0.1, crest), [cx.metres(s0.1).x, cx.metres(s0.1).y])], ROAD, [sty[0], SUB_DAM, 0, 0], Some(upv));
                    two(mesh, &[(pos(s0.1, crest), [l0, 0.0]), (pos(s1.1, crest), [l1, 0.0]), (pos(s1.4, s1.5), [l1, crest - s1.5]), (pos(s0.4, s0.5), [l0, crest - s0.5])], col, sty, None);
                    // Parapets on both edges of the crest.
                    for (a, b) in [(s0.0, s1.0), (s0.1, s1.1)] {
                        two(mesh, &[(pos(a, crest), [l0, 0.0]), (pos(b, crest), [l1, 0.0]), (pos(b, crest + 1.1), [l1, 1.1]), (pos(a, crest + 1.1), [l0, 1.1])], CONCRETE, sty, None);
                    }
                }
                // The ends: the dam's section, where it meets the valley side.
                for k in [0, m - 1] {
                    let s0 = sec[k];
                    let section = [(pos(s0.2, s0.3), [0.0, crest - s0.3]), (pos(s0.0, crest), [0.0, 0.0]), (pos(s0.1, crest), [cw, 0.0]), (pos(s0.4, s0.5), [cw, crest - s0.5])];
                    two(mesh, &section, col, sty, None);
                }
                // Its footprint: from the upstream toe round to the downstream one.
                let fp: Vec<[f32; 2]> = sec.iter().map(|s| s.2).chain(sec.iter().rev().map(|s| s.4)).collect();
                let norm: Vec<[f32; 2]> = fp.iter().map(|p| [p[0] / ext, p[1] / ext]).collect();
                let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
                for p in &norm {
                    lo = [lo[0].min(p[0]), lo[1].min(p[1])];
                    hi = [hi[0].max(p[0]), hi[1].max(p[1])];
                }
                footprints.push(Footprint { min: lo, max: hi, ring: norm, base: foot - 2.0, top: crest + 1.1, kind: 1, col: [176, 172, 162], main: (0, 0), det: (0, 0), fine: (0, 0), far: (0, 0), alive: true, center: cx.pos(c, crest), area: 0.0, sty: [0; 4], gref: 0.0, roof_col: [0; 3], tiers: Vec::new() });
                built.push(fp);
            }
            continue;
        }
        // Nothing to tell upstream by: the outline raised as it is.
        let mut along = 0.0f32;
        for i in 0..n {
            let (a, b) = (ring[i], ring[(i + 1) % n]);
            let len = Vec2::from(a).distance(Vec2::from(b)) * um;
            if len < 0.05 || border(a, b) {
                along += len;
                continue;
            }
            let nrm = out_n(i);
            // (Which way is upstream is not known here: an embankment's
            // faces slope, a concrete dam's stand plumb.)
            let slope = if fill { 2.0 } else { 0.0 };
            // Each end down to the ground where the face meets it (one
            // step of refinement: the ground out there, then that far).
            let base = |p: [f32; 2]| {
                let g0 = cx.ground(p);
                let run = slope * (crest - g0).max(0.0);
                let q = at(Vec2::from(p), nrm, run);
                let g = cx.ground(q);
                (at(Vec2::from(p), nrm, slope * (crest - g).max(0.0)), g - 2.0)
            };
            let ((qa, za), (qb, zb)) = (base(a), base(b));
            two(mesh, &[(pos(a, crest), [along, 0.0]), (pos(b, crest), [along + len, 0.0]), (pos(qb, zb), [along + len, crest - zb]), (pos(qa, za), [along, crest - za])], col, sty, None);
            // A gravity dam's sloping face leaves a wedge at each end, down
            // to where the face meets the hillside: close it.
            if slope > 0.0 {
                for (p, q, z) in [(a, qa, za), (b, qb, zb)] {
                    two(mesh, &[(pos(p, crest), [along, 0.0]), (pos(q, z), [along, crest - z]), (pos(p, z), [along, crest - z])], col, sty, None);
                }
            }
            // Parapets along the crest: a low wall on its edge.
            let inward = -nrm * (0.4 / um);
            let (ai, bi) = ((Vec2::from(a) + inward).into(), (Vec2::from(b) + inward).into());
            for (p, q) in [(a, b), (ai, bi)] {
                two(mesh, &[(pos(p, crest), [along, 0.0]), (pos(q, crest), [along + len, 0.0]), (pos(q, crest + 1.1), [along + len, 1.1]), (pos(p, crest + 1.1), [along, 1.1])], CONCRETE, sty, None);
            }
            two(mesh, &[(pos(a, crest + 1.1), [along, 0.0]), (pos(b, crest + 1.1), [along + len, 0.0]), (pos(bi, crest + 1.1), [along + len, 0.4]), (pos(ai, crest + 1.1), [along, 0.4])], CONCRETE, sty, Some(upv));
            along += len;
        }
        // The crest: the outline, flat, as a roadway.
        let flat: Vec<f64> = ring.iter().flat_map(|p| [p[0] as f64, p[1] as f64]).collect();
        if let Ok(ix) = earcutr::earcut(&flat, &[], 2) {
            for t in ix.chunks_exact(3) {
                let pts: Vec<(Vec3, [f32; 2])> = t.iter().map(|&k| {
                    let m = cx.metres(ring[k]);
                    (pos(ring[k], crest + 0.05), [m.x, m.y])
                }).collect();
                two(mesh, &pts, ROAD, [sty[0], SUB_DAM, 0, 0], Some(upv));
            }
        }
        let norm: Vec<[f32; 2]> = ring.iter().map(|p| [p[0] / ext, p[1] / ext]).collect();
        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
        for p in &norm {
            lo = [lo[0].min(p[0]), lo[1].min(p[1])];
            hi = [hi[0].max(p[0]), hi[1].max(p[1])];
        }
        footprints.push(Footprint { min: lo, max: hi, ring: norm, base: foot - 2.0, top: crest + 1.1, kind: 1, col: [176, 172, 162], main: (0, 0), det: (0, 0), fine: (0, 0), far: (0, 0), alive: true, center: cx.pos(c, crest), area: 0.0, sty: [0; 4], gref: 0.0, roof_col: [0; 3], tiers: Vec::new() });
        built.push(ring);
    }
    built
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_knows_the_three_gorges_dam() {
        assert!(table().len() > 10_000, "{} dams", table().len());
        // OpenStreetMap: height 115 m, 2.3 km long, concrete.
        let d = nearest(30.8225, 111.004, 400.0).expect("Three Gorges Dam");
        assert!((100.0..130.0).contains(&d.height), "{d:?}");
        assert!(d.length > 2000.0 && d.material == 1, "{d:?}");
        assert!(nearest(0.0, -30.0, 400.0).is_none(), "in the middle of the Atlantic");
    }

    #[test]
    fn a_feature_with_two_polygons_and_a_hole_is_read_even_odd() {
        // Two squares, the first with an island: as a map tile's feature has them.
        let sq = |x0: f32, y0: f32, s: f32| vec![[x0, y0], [x0 + s, y0], [x0 + s, y0 + s], [x0, y0 + s], [x0, y0]];
        let rings = vec![sq(0.0, 0.0, 10.0), sq(4.0, 4.0, 2.0), sq(20.0, 0.0, 5.0)];
        assert!(in_rings([1.0, 1.0], &rings));
        assert!(!in_rings([5.0, 5.0], &rings), "the island");
        assert!(in_rings([22.0, 2.0], &rings), "the second polygon");
        assert!(!in_rings([15.0, 2.0], &rings));
    }
}

