//! The four T56 nacelles, the 54H60 propellers and the external fuel tanks.
use super::*;
use glam::Mat4;
use std::f32::consts::TAU;

/// The nacelle's section along its length, relative to the propeller hub:
/// (distance aft of the propeller plane, half width, top, bottom,
/// squareness). Ahead of the wing it is deep and slightly flat-sided, the
/// chin swelling under the spinner round the engine's air intake and the
/// oil cooler's scoop (the T56's compressor sits below the offset
/// reduction gearbox that drives the propeller); from the leading edge its
/// top runs up into the wing, and under the wing it closes up into the
/// lower skin ahead of the flaps. [est, from photographs; the leading edge
/// is 2.45 m behind the propeller plane on both engines (dims)]
#[rustfmt::skip]
const NAC: [[f32; 5]; 10] = [
    [0.00, 0.42, 0.42, -0.42, 2.0],
    [0.15, 0.50, 0.50, -0.60, 2.1],
    [0.40, 0.58, 0.57, -0.80, 2.3],
    [0.80, 0.63, 0.62, -0.91, 2.5],
    [1.60, 0.65, 0.66, -0.95, 2.5],
    [2.40, 0.65, 0.70, -0.95, 2.5],
    [2.75, 0.65, 0.98, -0.94, 2.5],
    [3.60, 0.62, 1.00, -0.88, 2.5],
    [4.40, 0.55, 0.96, -0.72, 2.4],
    [5.10, 0.40, 0.92, -0.45, 2.3],
];
/// Where the nacelle's tail closes against the wing's lower skin: the
/// distance aft of the propeller plane (just ahead of the flaps' noses) and
/// the height there (relative to the hub).
fn nac_tail(e: usize) -> (f32, f32) {
    let x = ENG[e][0];
    let s_end = le_d(x) + (XF_FLAP - 0.03) * chord(x) - ENG[e][2];
    let lower = wing_foil().pt(x, XF_FLAP - 0.03, -1.0, -1.0).y - prop_hub(e).y;
    (s_end, lower)
}

/// Catmull-Rom through the table's column.
fn nac_at(col: usize, s: f32, tbl: &[[f32; 5]]) -> f32 {
    let n = tbl.len();
    if s <= tbl[0][0] {
        return tbl[0][col];
    }
    if s >= tbl[n - 1][0] {
        return tbl[n - 1][col];
    }
    let i = (0..n - 1).find(|&i| s < tbl[i + 1][0]).unwrap();
    let h = tbl[i + 1][0] - tbl[i][0];
    let t = (s - tbl[i][0]) / h;
    let slope = |k: usize| -> f32 {
        let (a, b) = (k.saturating_sub(1), (k + 1).min(n - 1));
        (tbl[b][col] - tbl[a][col]) / (tbl[b][0] - tbl[a][0]) * h
    };
    let (y0, y1, m0, m1) = (tbl[i][col], tbl[i + 1][col], slope(i), slope(i + 1));
    let (t2, t3) = (t * t, t * t * t);
    (2.0 * t3 - 3.0 * t2 + 1.0) * y0 + (t3 - 2.0 * t2 + t) * m0 + (-2.0 * t3 + 3.0 * t2) * y1 + (t3 - t2) * m1
}

/// The nacelle's table for engine e, its tail closing on the wing.
fn nac_table(e: usize) -> Vec<[f32; 5]> {
    let (s_end, lower) = nac_tail(e);
    let mut t: Vec<[f32; 5]> = NAC.iter().copied().filter(|r| r[0] < s_end - 1.6).collect();
    t.push([s_end - 1.0, 0.44, 0.94, lower - 0.62, 2.3]);
    t.push([s_end - 0.45, 0.26, 0.90, lower - 0.28, 2.2]);
    t.push([s_end, 0.03, 0.85, lower + 0.04, 2.0]);
    t
}

/// A point on engine e's nacelle (right side): s aft of the propeller
/// plane, a round from the top (0) through the outboard side.
fn nac_pt(e: usize, tbl: &[[f32; 5]], s: f32, a: f32) -> Vec3 {
    let (w, t, bt, n) = (nac_at(1, s, tbl), nac_at(2, s, tbl), nac_at(3, s, tbl), nac_at(4, s, tbl));
    let (sn, cs) = a.sin_cos();
    let ex = |v: f32| v.signum() * v.abs().powf(2.0 / n);
    let y = if cs > 0.0 { t * ex(cs) } else { -bt * ex(cs) };
    prop_hub(e) + Vec3::new(w * ex(sn), y, s)
}

/// One nacelle (right side, engine e): the cowling, its intakes, the jet
/// pipe's exit on the outboard side under the wing, latches.
fn nacelle(e: usize) -> Mesh {
    let hub = prop_hub(e);
    let tbl = nac_table(e);
    let s_end = tbl.last().unwrap()[0];
    let cowl = SKIN.pat(pat::NACELLE);
    let mut m = Mesh::default();
    let ss = stations(0.0, s_end, &|s| if s < 0.5 { 0.04 } else if s > s_end - 1.2 { 0.08 } else { 0.15 });
    const NA: usize = 48;
    let rows: Vec<Vec<SP>> = ss
        .iter()
        .map(|&s| {
            let mut arc = 0.0;
            let mut prev = nac_pt(e, &tbl, s, 0.0);
            (0..=NA)
                .map(|k| {
                    let a = k as f32 / NA as f32 * TAU;
                    let p = nac_pt(e, &tbl, s, a);
                    arc += (p - prev).length();
                    prev = p;
                    (p, [s, arc], 0.0)
                })
                .collect()
        })
        .collect();
    m.grid(&cowl, &rows, true, false);
    // The front: a ring round the spinner's back plate.
    m.lathe(&cowl, hub, Vec3::Z, &[[0.36, 0.0], [0.42, 0.0]], 32, false);
    // The engine's air intake under the spinner: a polished lip round a dark duct.
    let ic = hub + Vec3::new(0.0, -0.50, 0.12);
    let mut intake = Mesh::default();
    intake.lathe(&mt(mat::ALU, 0.70, 0.71, 0.73), Vec3::ZERO, Vec3::Z, &[[0.15, 0.20], [0.15, 0.04], [0.165, 0.0], [0.19, -0.012], [0.215, 0.0], [0.225, 0.05], [0.22, 0.14]], 24, true);
    intake.disc(&DARK, Vec3::Z * 0.18, Vec3::NEG_Z, Vec3::Y, 0.155, 20);
    for v in &mut intake.v {
        let q = Vec3::from(v.pos);
        v.pos = (ic + Vec3::new(q.x * 1.25, q.y, q.z)).into();
    }
    m.append(&intake);
    // The oil cooler's scoop under the chin, and its exit flap further aft.
    let oc = hub + Vec3::new(0.0, -0.86, 0.62);
    m.rbox(&cowl, oc + Vec3::new(0.0, -0.02, 0.18), Vec3::new(0.21, 0.09, 0.22), 0.04, Quat::IDENTITY);
    m.rbox(&DARK, oc + Vec3::new(0.0, -0.02, -0.035), Vec3::new(0.17, 0.06, 0.012), 0.01, Quat::IDENTITY);
    let fl = hub + Vec3::new(0.0, nac_at(3, 2.2, &tbl) - 0.01, 2.2);
    m.rbox(&mt(mat::SATIN, 0.2, 0.2, 0.2), fl, Vec3::new(0.18, 0.02, 0.16), 0.01, Quat::from_rotation_x(0.18));
    // The cooling-air scoop on top behind the spinner.
    let tc = hub + Vec3::new(-0.12, nac_at(2, 0.9, &tbl), 0.9);
    m.rbox(&cowl, tc + Vec3::new(0.0, 0.03, 0.12), Vec3::new(0.08, 0.04, 0.16), 0.025, Quat::IDENTITY);
    m.rbox(&DARK, tc + Vec3::new(0.0, 0.035, -0.035), Vec3::new(0.06, 0.025, 0.008), 0.006, Quat::IDENTITY);
    // Cowl latches along the lower cowling's seams, both sides.
    for sx in [-1.0f32, 1.0] {
        for k in 0..5 {
            let s = 0.75 + k as f32 * 0.35;
            let a = if sx > 0.0 { 2.15 } else { TAU - 2.15 };
            let p = nac_pt(e, &tbl, s, a);
            let n = (p - (hub + Vec3::new(0.0, -0.2, s))).normalize();
            m.rbox(&mt(mat::SATIN, 0.40, 0.41, 0.42), p + n * 0.006, Vec3::new(0.012, 0.012, 0.05), 0.004, Quat::from_rotation_arc(Vec3::X, n));
        }
    }
    // The jet pipe: out of the nacelle's outboard side just under the wing's
    // lower skin, angled a little outboard (the T56's tailpipe; the soot it
    // leaves runs back under the flap) [est: dims EXHAUST_*].
    let ex = exhaust(e);
    let dir = Vec3::new(0.30, 0.0, 1.0).normalize();
    let start = ex - dir * 0.75 - Vec3::X * 0.18;
    m.tube(&cowl, &[start, ex - dir * 0.10], &|t| EXHAUST_R + 0.03 * (1.0 - t), 20, false);
    m.tube(&EXHAUST_MAT, &[ex - dir * 0.10, ex], &|_| EXHAUST_R + 0.01, 20, false);
    m.lathe(&EXHAUST_MAT, ex, dir, &[[EXHAUST_R + 0.01, 0.0], [EXHAUST_R - 0.015, 0.0], [EXHAUST_R - 0.015, -0.12]], 20, false);
    m.disc(&DARK, ex - dir * 0.10, dir, Vec3::Y, EXHAUST_R - 0.014, 20);
    m
}

/// The propeller (spinner, four blades, blur disc) on hub `hub`, turning
/// clockwise seen from behind on every engine (as all four 54H60s do).
/// The blades: broad paddles of nearly constant chord, square-tipped, the
/// root cuffs and the tips flat black, the rest grey (`mat::PROP`), the
/// maker's decal on each face [est: planform and colours from photographs;
/// one fixed pitch, about 30° at the tip].
fn propeller(hub: Vec3) -> (Mesh, Mesh, Mesh) {
    let mut sp = Mesh::default();
    // The spinner: a long blunt cone, its after-body round to the cowling.
    let prof: Vec<[f32; 2]> = (0..=20)
        .map(|k| {
            let u = k as f32 / 20.0;
            let r = SPINNER_R * (1.0 - u.powf(1.15)).max(0.0).powf(0.72);
            [if k == 20 { 0.0 } else { r.max(0.012) }, -SPINNER_L * u]
        })
        .collect();
    sp.lathe(&FLAT_BLACK, hub, Vec3::Z, &prof, 36, true);
    sp.lathe(&FLAT_BLACK, hub, Vec3::Z, &[[0.36, 0.06], [SPINNER_R + 0.005, 0.04], [SPINNER_R + 0.005, 0.0], [SPINNER_R, -0.02]], 36, true);
    // Its fasteners round the joint with the after-body.
    for k in 0..16 {
        let a = k as f32 / 16.0 * TAU;
        let n = Vec3::new(a.cos(), a.sin(), 0.0);
        sp.screw(&SCREW_STEEL, hub + n * (SPINNER_R + 0.002) - Vec3::Z * 0.06, n, 0.006, k as f32);
    }
    let mut blades = Mesh::default();
    let r_tip = PROP_R;
    for k in 0..4 {
        let mut bl = Mesh::default();
        let blade = Foil {
            at: Box::new(move |r: f32| {
                let beta = (0.95 / r.max(0.3)).atan() + 0.08;
                // the round shank, flaring into the broad blade by 0.85 m
                let flare = smoothstep(0.42, 0.85, r);
                let mut c = 0.17 + (0.40 - 0.17) * flare + 0.02 * smoothstep(0.9, 1.5, r) * (1.0 - smoothstep(1.6, 2.0, r));
                let q = ((r - (r_tip - 0.16)) / 0.16).clamp(0.0, 1.0);
                c *= 0.35 + 0.65 * (1.0 - q.powi(4)).max(0.0).powf(0.25);
                let tf = 12.0 * (1.0 - flare) + (2.0 - 1.4 * ((r - 0.85) / 1.2).clamp(0.0, 1.0)) * flare;
                let cdir = -Vec3::X * beta.cos() + Vec3::Z * beta.sin();
                let up = Vec3::NEG_Z * beta.cos() - Vec3::X * beta.sin();
                Sect { le: hub + Vec3::Y * r - cdir * (0.38 * c), c, cdir, up, tf }
            }),
            t: 0.08,
            camber: 0.03,
        };
        let cuff = 0.82;
        let tip = r_tip - 0.09;
        blade.skin(&mut bl, &FLAT_BLACK, 0.30, cuff, 10, &|_| 0.0, &|_| 1.0, 12, false);
        let n0 = bl.v.len();
        blade.skin(&mut bl, &mt(mat::PROP, 0.5, 0.5, 0.5), cuff, tip, 20, &|_| 0.0, &|_| 1.0, 12, false);
        // The shader paints mat::PROP's tip band past uv.x 0.86: keep it off these big blades.
        for v in &mut bl.v[n0..] {
            v.uv[0] *= 0.4;
        }
        blade.skin(&mut bl, &FLAT_BLACK, tip, r_tip, 4, &|_| 0.0, &|_| 1.0, 12, false);
        blade.cap(&mut bl, &FLAT_BLACK, r_tip, 0.0, 1.0, false);
        // the maker's decal on each face
        for side in [1.0f32, -1.0] {
            let (p, n) = foil_frame(&blade, 1.45, 0.45, side);
            bl.disc(&mt(mat::PAINT, 0.80, 0.80, 0.78), p + n * 0.002, n, Vec3::Y, 0.035, 12);
            bl.disc(&mt(mat::PAINT, 0.55, 0.04, 0.03), p + n * 0.003, n, Vec3::Y, 0.024, 12);
        }
        bl.transform(0, Mat4::from_translation(hub) * Mat4::from_rotation_z(k as f32 * PI * 0.5 + PI * 0.25) * Mat4::from_translation(-hub));
        blades.append(&bl);
    }
    let mut disc = Mesh::default();
    disc.disc(&mt(mat::PROPDISC, 0.1, 0.1, 0.1), hub + Vec3::Z * 0.005, Vec3::NEG_Z, Vec3::Y, r_tip, 64);
    (sp, blades, disc)
}

/// The external tank's radius (fraction of `EXT_TANK_R`) at fraction u of
/// its length from the nose: a blunt ogive nose, a parallel middle, a long
/// tapered tail [est].
pub(super) fn tank_r(u: f32) -> f32 {
    if u < 0.24 {
        let v = 1.0 - u / 0.24;
        (1.0 - v * v).max(0.0).sqrt()
    } else if u < 0.52 {
        1.0
    } else {
        let v = (u - 0.52) / 0.48;
        (1.0 - v.powf(1.7)).max(0.0).powf(0.8)
    }
}

/// An external fuel tank (right side) on its pylon.
fn ext_tank() -> Mesh {
    let mut t = Mesh::default();
    let c = ext_tank_nose();
    let tank = SKIN.pat(pat::NACELLE);
    // A blunt ogive nose, a parallel middle, a long tapered tail [est].
    let prof: Vec<[f32; 2]> = (0..=40)
        .rev()
        .map(|k| {
            let u = k as f32 / 40.0;
            let r = tank_r(u);
            [EXT_TANK_R * r.max(if k == 0 || k == 40 { 0.0 } else { 0.01 }), u * EXT_TANK_L]
        })
        .collect();
    t.lathe(&tank, c, Vec3::Z, &prof, 32, true);
    // the nose cap and the tail's vent
    t.lathe(&mt(mat::PAINT, 0.62, 0.60, 0.55), c, Vec3::NEG_Z, &[[0.075, -0.06], [0.07, 0.0], [0.04, 0.035], [0.0, 0.045]], 14, true);
    t.tube(&ALU, &[c + Vec3::Z * (EXT_TANK_L - 0.05), c + Vec3::Z * (EXT_TANK_L + 0.12)], &|_| 0.012, 6, true);
    // seams where the sections join, and the filler cap on top
    for u in [0.24f32, 0.52] {
        t.lathe(&tank, c + Vec3::Z * (u * EXT_TANK_L), Vec3::Z, &[[EXT_TANK_R + 0.003, -0.01], [EXT_TANK_R + 0.003, 0.01]], 32, false);
    }
    t.lathe(&mt(mat::LIVERY, 0.2, 0.22, 0.22), c + Vec3::new(0.0, EXT_TANK_R - 0.004, 0.3 * EXT_TANK_L), Vec3::Y, &[[0.06, 0.0], [0.06, 0.01], [0.0, 0.012]], 14, false);
    // The pylon up into the wing.
    let x = EXT_TANK[0];
    let y0 = c.y + EXT_TANK_R * 0.85;
    let y1 = wing_foil().pt(x, 0.35, -1.0, -1.0).y + 0.12;
    let (f0, f1) = (c.z + 0.30 * EXT_TANK_L, bz(le_d(x)) + 0.12);
    let (b0, b1) = (c.z + 0.66 * EXT_TANK_L, bz(le_d(x)) + 0.62 * chord(x));
    let pylon = Foil {
        at: Box::new(move |s: f32| {
            let u = s.clamp(0.0, 1.0);
            let le = Vec3::new(x, y0 + (y1 - y0) * u, f0 + (f1 - f0) * u);
            Sect { le, c: (b0 + (b1 - b0) * u) - le.z, cdir: Vec3::Z, up: Vec3::X, tf: 1.0 }
        }),
        t: 0.09,
        camber: 0.0,
    };
    pylon.skin(&mut t, &SKIN.pat(pat::NACELLE), 0.0, 1.0, 6, &|_| 0.0, &|_| 1.0, 16, false);
    t
}

pub(super) fn engines(b: &mut Builder) {
    const NAC_N: [[&str; 2]; 2] = [["left inboard nacelle", "right inboard nacelle"], ["left outboard nacelle", "right outboard nacelle"]];
    const SPIN: [[&str; 2]; 2] = [["left inboard spinner", "right inboard spinner"], ["left outboard spinner", "right outboard spinner"]];
    const BLADES: [[&str; 2]; 2] = [["left inboard propeller", "right inboard propeller"], ["left outboard propeller", "right outboard propeller"]];
    const DISC: [[&str; 2]; 2] = [["left inboard prop disc", "right inboard prop disc"], ["left outboard prop disc", "right outboard prop disc"]];
    for e in 0..2 {
        let m = nacelle(e);
        b.fixed(NAC_N[e][0], G_LWING, mirrored(&m));
        b.fixed(NAC_N[e][1], G_RWING, m);
        // The propellers are not mirrored: all four turn the same way.
        for (side, group) in [(0usize, G_LWING), (1, G_RWING)] {
            let mut hub = prop_hub(e);
            if side == 0 {
                hub.x = -hub.x;
            }
            let (sp, bl, disc) = propeller(hub);
            let spin = Anim::hinge(Drive::new(Input::Prop, -1.0));
            let k = b.add(SPIN[e][side], group, sp, spin, hub, Vec3::Z, None);
            b.role(k, Role::Prop);
            let k = b.add(BLADES[e][side], group, bl, spin, hub, Vec3::Z, None);
            b.role(k, Role::Prop);
            b.pieces[k].vis = Vis::SlowProp;
            b.pieces[k].occluder = false;
            let k = b.add(DISC[e][side], group, disc, spin, hub, Vec3::Z, None);
            b.role(k, Role::Prop);
            b.pieces[k].vis = Vis::FastProp;
            b.pieces[k].glass = true;
            b.pieces[k].occluder = false;
        }
    }
    let t = ext_tank();
    b.fixed("left external tank", G_LWING, mirrored(&t));
    b.fixed("right external tank", G_RWING, t);
}
