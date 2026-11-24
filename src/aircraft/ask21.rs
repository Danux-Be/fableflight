//! the schleicher ask 21: a two seat glass fibre glider, the club trainer.
//! figures from schleicher's data sheet and flight manual: span 17.0 m, wing
//! area 17.95 m² (aspect ratio 16.1), length 8.35 m, empty mass 360 kg, most
//! 600 kg, best glide 34 at about 50 kt, least sink 0.65 m/s at about 40 kt,
//! never exceed 151 kt, schempp-hirth airbrakes, a fixed main and nose wheel,
//! a t tail. its skins are plain gloss white (glass fibre: no rivets or laps). built on the robin's spec (handling, sounds) with all of that
//! set; the model is built here, kept simple: the pod, the wings, the tail,
//! the canopy, and the front seat with its instruments. no engine: it starts
//! in the air (or on a tow line it does not have) and soars in the thermals.

use crate::aircraft::{self, Engine, Hotspot, Leg, Part, Spec};
use crate::airframe::{mat, mt, pat, Anim, Builder, Drive, Foil, Input, Mat, Mesh, Model, Role, Sect, BLACK, G_ENGINE, G_FUSE, G_GEAR, G_LWING, G_RWING, G_TAIL, RUBBER};
use crate::aircraft::c172::cabin::gauge;
use glam::{DVec3, Quat, Vec3};

const KT: f64 = 1.0 / 1.943_84;
/// half span, the root and tip chords, the wing's leading edge at the root (y, z).
const SPAN: f32 = 8.5;
const ROOT: f32 = 1.10;
const TIP: f32 = 0.42;
const WING_Y: f32 = 0.18;
const WING_Z: f32 = -0.42;
/// ailerons and airbrakes, span stations.
const AILERON: [f32; 2] = [5.0, 8.2];
const BRAKES: [f32; 2] = [1.6, 3.1];
const NOSE_Z: f32 = -3.95;
const TAIL_Z: f32 = 4.40;
const EYE: Vec3 = Vec3::new(0.0, 0.38, -2.05);
const PANEL_Z: f32 = -2.72;

const WHITE: Mat = mt(mat::PAINT, 0.86, 0.86, 0.84);
const GREY: Mat = mt(mat::PAINT, 0.42, 0.43, 0.45);
const GLASS: Mat = mt(mat::GLASS, 0.6, 0.7, 0.75);
const FACE: Mat = mt(mat::GAUGE, 0.02, 0.02, 0.02);
const BLUE: Mat = mt(mat::PAINT, 0.08, 0.20, 0.62);
const GREEN: Mat = mt(mat::PAINT, 0.10, 0.45, 0.16);
const YELLOW: Mat = mt(mat::PAINT, 0.85, 0.65, 0.05);

/// the right wing's section at a span station: tapered, 4 deg of dihedral, a little forward sweep.
fn wing_sect(s: f32) -> Sect {
    let t = (s / SPAN).clamp(0.0, 1.0);
    let c = ROOT + (TIP - ROOT) * t.powf(0.9);
    let up = Quat::from_rotation_z(4f32.to_radians()) * Vec3::Y;
    let le = Vec3::new(s, WING_Y + s * 4f32.to_radians().tan(), WING_Z - 0.025 * s + (ROOT - c) * 0.25);
    let tip = (1.0 - ((t - 0.97) / 0.03).clamp(0.0, 1.0)).sqrt().max(0.25);
    Sect { le, c, cdir: Vec3::Z, up, tf: tip }
}

fn foil() -> Foil {
    Foil { at: Box::new(wing_sect), t: 0.16, camber: 0.04 }
}

/// the pod and boom: (station z, half width, half height, height of the middle).
fn fuse_at(z: f32) -> (f32, f32, f32) {
    let k = (z - NOSE_Z) / (TAIL_Z - NOSE_Z);
    // the cockpit pod, then the slim boom to the tail.
    let pod = (1.0 - ((k - 0.24) / 0.24).powi(2)).max(0.0).sqrt();
    let w = 0.06 + 0.30 * pod * (1.0 - smooth(0.42, 0.62, k)) + 0.07 * (1.0 - smooth(0.4, 1.0, k));
    let h = 0.08 + 0.38 * pod * (1.0 - smooth(0.42, 0.62, k)) + 0.09 * (1.0 - smooth(0.4, 1.0, k));
    let y = 0.02 - 0.10 * pod + 0.18 * smooth(0.5, 1.0, k);
    (w.max(0.03), h.max(0.03), y)
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn fuselage(b: &mut Builder) {
    let mut m = Mesh::default();
    let n = 64;
    let segs = 20;
    let base = m.v.len() as u32;
    for i in 0..=n {
        let z = NOSE_Z + (TAIL_Z - NOSE_Z) * (i as f32 / n as f32);
        let (w, h, y) = fuse_at(z);
        // (a point at the very nose)
        let (w, h) = if i == 0 { (0.001, 0.001) } else { (w, h) };
        for k in 0..=segs {
            let a = k as f32 / segs as f32 * std::f32::consts::TAU;
            let p = Vec3::new(w * a.cos(), y + h * a.sin(), z);
            let nrm = Vec3::new(a.cos() / w.max(0.01), a.sin() / h.max(0.01), 0.0).normalize_or_zero();
            m.push(p, nrm, [a * 0.3, z], &WHITE, 0.0);
        }
    }
    let nc = segs as u32 + 1;
    for i in 0..n as u32 {
        for k in 0..segs as u32 {
            let a = base + i * nc + k;
            m.i.extend([a, a + 1, a + nc, a + 1, a + nc + 1, a + nc]);
        }
    }
    b.fixed("fuselage", G_FUSE, m);
    // the canopy: one long bubble over both seats.
    let mut g = Mesh::default();
    let (cz0, cz1) = (-3.0f32, -0.95f32);
    let start = g.v.len();
    g.lathe(&GLASS, Vec3::ZERO, Vec3::Z, &[[0.0, 0.0], [0.16, 0.12], [0.28, 0.45], [0.31, 1.0], [0.30, 1.55], [0.22, 1.9], [0.0, cz1 - cz0]], 24, true);
    g.transform(start, glam::Mat4::from_translation(Vec3::new(0.0, 0.20, cz0)) * glam::Mat4::from_scale(Vec3::new(1.0, 0.95, 1.0)));
    let i = b.fixed("canopy", G_FUSE, g);
    b.pieces[i].glass = true;
    b.pieces[i].occluder = false;
}

fn wings(b: &mut Builder) {
    let f = foil();
    let skin = WHITE;
    let one = |_: f32| 1.0;
    let zero = |_: f32| 0.0;
    let ah = |_: f32| 0.76;
    let mut w = Mesh::default();
    f.skin(&mut w, &skin, 0.0, AILERON[0], 30, &zero, &one, 26, false);
    f.skin(&mut w, &skin, AILERON[0], AILERON[1], 18, &zero, &ah, 26, false);
    f.skin(&mut w, &skin, AILERON[1], SPAN, 10, &zero, &one, 26, false);
    f.cap(&mut w, &GREY, AILERON[0], 0.76, 1.0, false);
    f.cap(&mut w, &GREY, AILERON[1], 0.76, 1.0, false);
    b.pair(["left wing", "right wing"], [G_LWING, G_RWING], w, |_| Anim::Fixed, Vec3::ZERO, Vec3::X, [None, None]);
    // the ailerons behind their hinge line.
    let mut a = Mesh::default();
    f.skin(&mut a, &WHITE, AILERON[0], AILERON[1], 18, &ah, &one, 10, true);
    let hinge0 = f.pt(AILERON[0], 0.76, 0.0, -1.0);
    let hinge1 = f.pt(AILERON[1], 0.76, 0.0, -1.0);
    let (l, r) = b.pair(["left aileron", "right aileron"], [G_LWING, G_RWING], a, |sd| Anim::hinge(Drive::new(Input::Aileron(sd), -0.35)), hinge0, hinge1 - hinge0, [None, None]);
    b.role(l, Role::Aileron(0)).role(r, Role::Aileron(1));
    // the airbrakes: paddles that rise out of the top of the wing.
    let mut p = Mesh::default();
    let (s0, s1) = (BRAKES[0], BRAKES[1]);
    let y0 = f.pt(s0, 0.5, 1.0, -1.0);
    let y1 = f.pt(s1, 0.5, 1.0, -1.0);
    p.rbox(&GREY, (y0 + y1) * 0.5 - Vec3::Y * 0.06, Vec3::new((s1 - s0) * 0.5, 0.06, 0.012), 0.004, Quat::IDENTITY);
    b.pair(["left airbrake", "right airbrake"], [G_LWING, G_RWING], p, |sd| Anim::slide(Drive::new(Input::Spoiler(sd), 0.17), Vec3::Y), (y0 + y1) * 0.5, Vec3::X, [None, None]);
}

fn tail(b: &mut Builder) {
    // the fin and rudder, then the tailplane on top of it with its elevator.
    let fin = Foil { at: Box::new(|s: f32| Sect { le: Vec3::new(0.0, 0.30 + s, 3.45 + s * 0.35), c: 0.95 - s * 0.38, cdir: Vec3::Z, up: Vec3::X, tf: 1.0 }), t: 0.11, camber: 0.0 };
    let mut m = Mesh::default();
    let one = |_: f32| 1.0;
    let zero = |_: f32| 0.0;
    let rh = |_: f32| 0.66;
    fin.skin(&mut m, &WHITE, 0.0, 1.15, 10, &zero, &rh, 18, false);
    fin.cap(&mut m, &GREY, 1.15, 0.0, 1.0, false);
    b.fixed("fin", G_TAIL, m);
    let mut r = Mesh::default();
    fin.skin(&mut r, &WHITE, 0.0, 1.15, 10, &rh, &one, 8, true);
    let h0 = fin.pt(0.0, 0.66, 0.0, -1.0);
    let h1 = fin.pt(1.15, 0.66, 0.0, -1.0);
    let rud = b.add("rudder", G_TAIL, r, Anim::hinge(Drive::new(Input::Rudder, 0.5)), h0, h1 - h0, None);
    b.role(rud, Role::Rudder);
    let stab = Foil { at: Box::new(|s: f32| Sect { le: Vec3::new(s, 1.46, 3.95 + s * 0.05), c: 0.62 - s * 0.16, cdir: Vec3::Z, up: Vec3::Y, tf: 1.0 }), t: 0.10, camber: 0.0 };
    let eh = |_: f32| 0.62;
    let mut st = Mesh::default();
    stab.skin(&mut st, &WHITE, 0.0, 1.6, 10, &zero, &eh, 16, false);
    stab.cap(&mut st, &GREY, 1.6, 0.0, 1.0, false);
    b.pair(["left stabiliser", "right stabiliser"], [G_TAIL, G_TAIL], st, |_| Anim::Fixed, Vec3::ZERO, Vec3::X, [None, None]);
    let mut e = Mesh::default();
    stab.skin(&mut e, &WHITE, 0.0, 1.6, 10, &eh, &one, 8, true);
    let e0 = stab.pt(0.0, 0.62, 0.0, -1.0);
    let (l, rr) = b.pair(["left elevator", "right elevator"], [G_TAIL, G_TAIL], e, |_| Anim::hinge(Drive::new(Input::Elevator, 0.4)), e0, Vec3::X, [None, None]);
    b.role(l, Role::Elevator).role(rr, Role::Elevator);
}

fn gear(b: &mut Builder) {
    let mut m = Mesh::default();
    // the main wheel half sunk in the belly, the small nose wheel, the tail skid.
    for (z, r, w) in [(0.15f32, 0.19f32, 0.12f32), (-2.95, 0.10, 0.07)] {
        let y = fuse_at(z).2 - fuse_at(z).1 - r * 0.45;
        m.lathe(&RUBBER, Vec3::new(-w * 0.5, y, z), Vec3::X, &[[0.0, 0.0], [r * 0.85, 0.0], [r, w * 0.2], [r, w * 0.8], [r * 0.85, w], [0.0, w]], 20, true);
    }
    m.rbox(&BLACK, Vec3::new(0.0, -0.30, 4.30), Vec3::new(0.02, 0.05, 0.10), 0.01, Quat::IDENTITY);
    b.fixed("wheels", G_GEAR, m);
}

/// the front seat: the panel with its three instruments and the compass, the
/// stick, the blue airbrake lever, the green trim lever and the yellow release.
fn cockpit(b: &mut Builder) {
    let mut m = Mesh::default();
    let panel = Vec3::new(0.0, 0.24, PANEL_Z);
    m.rbox(&GREY, panel, Vec3::new(0.24, 0.09, 0.012), 0.02, Quat::from_rotation_x(-0.35));
    let n = Quat::from_rotation_x(-0.35) * Vec3::Z;
    let up = Quat::from_rotation_x(-0.35) * Vec3::Y;
    for (id, x) in [(gauge::ASI, -0.14f32), (gauge::ALT, 0.0), (gauge::VSI, 0.14)] {
        let c = panel + Vec3::X * x + n * 0.013;
        m.disc(&FACE.x(id), c, n, up, 0.04, 36);
        m.lathe(&BLACK, c - n * 0.002, n, &[[0.04, 0.0], [0.046, 0.004], [0.046, 0.006], [0.04, 0.006]], 36, false);
    }
    // the compass on top of the panel.
    m.rbox(&BLACK, panel + up * 0.11 + n * 0.02, Vec3::new(0.035, 0.025, 0.03), 0.01, Quat::IDENTITY);
    // the seat pan and the floor.
    m.rbox(&mt(mat::PAINT, 0.12, 0.12, 0.13).pat(pat::SEAT), Vec3::new(0.0, -0.18, -1.75), Vec3::new(0.24, 0.04, 0.30), 0.03, Quat::from_rotation_x(0.15));
    m.rbox(&BLACK, Vec3::new(0.0, -0.32, -2.4), Vec3::new(0.22, 0.01, 0.6), 0.01, Quat::IDENTITY);
    b.fixed("cockpit", G_FUSE, m);
    // the stick, in the middle.
    let mut s = Mesh::default();
    s.tube(&BLACK, &[Vec3::new(0.0, -0.30, -2.25), Vec3::new(0.0, 0.02, -2.32)], &|_| 0.012, 10, true);
    s.lathe(&RUBBER, Vec3::new(0.0, 0.02, -2.32), Vec3::Y, &[[0.0, 0.0], [0.018, 0.0], [0.02, 0.08], [0.0, 0.1]], 12, true);
    b.add("stick", G_FUSE, s, Anim::hinge2(Drive::new(Input::Pitch, -0.25), Drive::NONE), Vec3::new(0.0, -0.30, -2.25), Vec3::X, None);
    // the airbrake lever on the left wall: back to open.
    let mut ab = Mesh::default();
    let ab_piv = Vec3::new(-0.27, -0.10, -1.85);
    ab.tube(&GREY, &[ab_piv, ab_piv + Vec3::new(0.0, 0.16, -0.06)], &|_| 0.008, 8, true);
    ab.lathe(&BLUE, ab_piv + Vec3::new(0.0, 0.16, -0.06), Vec3::Y, &[[0.0, 0.0], [0.016, 0.0], [0.018, 0.05], [0.0, 0.06]], 12, true);
    let k = b.add("airbrake lever", G_FUSE, ab, Anim::hinge(Drive::new(Input::Speedbrake, 0.9)), ab_piv, Vec3::X, None);
    b.pieces[k].hotspot = Some(Hotspot::click(Vec3::new(0.0, 0.19, -0.06), 0.03, "speedbrake"));
    // the trim lever on the left, by the seat.
    let mut tr = Mesh::default();
    let tr_piv = Vec3::new(-0.24, -0.20, -1.55);
    tr.tube(&GREY, &[tr_piv, tr_piv + Vec3::new(0.0, 0.10, -0.03)], &|_| 0.006, 8, true);
    tr.rbox(&GREEN, tr_piv + Vec3::new(0.0, 0.11, -0.03), Vec3::new(0.012, 0.012, 0.02), 0.005, Quat::IDENTITY);
    let k = b.add("trim lever", G_FUSE, tr, Anim::hinge(Drive::new(Input::Trim, 3.0)), tr_piv, Vec3::X, None);
    b.pieces[k].hotspot = Some(Hotspot::wheel(Vec3::new(0.0, 0.11, -0.03), 0.025, "trim_down", "trim_up"));
    // the yellow release knob under the panel (no tow line here: for looks).
    let mut rel = Mesh::default();
    rel.lathe(&YELLOW, Vec3::new(-0.16, 0.10, PANEL_Z + 0.06), Vec3::Z, &[[0.0, 0.0], [0.018, 0.0], [0.02, 0.03], [0.0, 0.035]], 12, true);
    b.fixed("release knob", G_ENGINE, rel);
}

fn build() -> Model {
    let mut b = Builder::default();
    fuselage(&mut b);
    wings(&mut b);
    tail(&mut b);
    gear(&mut b);
    cockpit(&mut b);
    Model::finish(b.pieces)
}

/// its picture in the aircraft list, drawn here: a glider from above against the evening sky.
fn still() -> &'static [u8] {
    let (w, h) = (680u32, 392u32);
    let mut img = image::RgbaImage::new(w, h);
    let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.52);
    for (x, y, px) in img.enumerate_pixels_mut() {
        let t = y as f32 / h as f32;
        let sky = [40.0 + 120.0 * t, 70.0 + 90.0 * t, 120.0 + 60.0 * t];
        let (u, v) = ((x as f32 - cx) / 30.0, (y as f32 - cy) / 30.0);
        // the wing: long, tapered; the fuselage; the tailplane.
        let span = (u.abs() / 10.0).min(1.0);
        let chord = 0.55 - 0.32 * span;
        let wing = u.abs() < 10.0 && v > -0.2 - chord * 0.3 && v < chord * 0.7;
        let fuse = u.abs() < 0.32 * (1.0 - ((v + 1.5) / 6.0).clamp(0.0, 1.0)) + 0.08 && v > -3.6 && v < 4.4;
        let tail = u.abs() < 1.6 && v > 3.9 && v < 4.4;
        let c = if wing || fuse || tail { [235.0, 236.0, 232.0] } else { sky };
        *px = image::Rgba([c[0] as u8, c[1] as u8, c[2] as u8, 255]);
    }
    let mut out = std::io::Cursor::new(Vec::new());
    let _ = img.write_to(&mut out, image::ImageFormat::Png);
    Box::leak(out.into_inner().into_boxed_slice())
}

const fn leg(at: DVec3, r: f64, k: f64, steer: f64, brakes: bool, name: &'static str) -> Leg {
    Leg { at, wheel_r: r, travel: 0.05, k, c: k * 0.12, nf_max: 30_000.0, steer, brakes, side_k: 2500.0, side_mu: 0.8, roll_k: 60.0, roll_mu: 0.03, brake_k: 1500.0, brake_mu: 0.5, fold_vn: 4.0, fold_kick: DVec3::new(0.3, 0.0, 0.0), name }
}

/// the ask 21's type, on the robin's.
pub fn spec() -> Spec {
    let mut s = aircraft::dr400::SPEC.clone();
    s.id = 10;
    s.name = "Schleicher ASK 21";
    s.keys = &["ask21", "glider", "sailplane"];
    s.blurb = "Two-seat glass fibre glider, the club trainer. 17 m of wing, a glide of 1 in 34: find the thermals and stay up.";
    s.still = still();
    s.paint = None;
    s.build = build;
    s.span = SPAN;
    s.wing_sect = wing_sect;
    s.fuse = |z| {
        let (_, h, y) = fuse_at(z);
        (y + h, y, y - h)
    };
    s.aileron = AILERON;
    s.tank = [0.0, 0.0];
    s.eye = EYE;
    s.eye2 = None;
    s.sees_inside = |_, d| d.y < -0.45;
    s.hotspots = &[];
    s.contacts = &[
        ([0.0, -0.20, -3.85], Part::Nose),
        ([0.0, -0.36, -2.6], Part::Nose),
        ([0.0, -0.42, -1.4], Part::Belly),
        ([0.0, -0.36, 0.4], Part::Belly),
        ([-0.32, 0.0, -2.0], Part::Side),
        ([0.32, 0.0, -2.0], Part::Side),
        ([0.0, 0.52, -1.9], Part::Roof),
    ];
    s.tail_pts = &[[0.0, -0.18, 2.5], [0.0, -0.12, 3.8], [0.0, 0.1, 4.4]];
    s.wing_st = [0.6 / SPAN, 2.2 / SPAN, 4.0 / SPAN, 6.0 / SPAN, 8.2 / SPAN];
    s.hard = [DVec3::new(0.0, -0.02, -3.95), DVec3::new(-8.5, 0.77, -0.6), DVec3::new(8.5, 0.77, -0.6), DVec3::new(0.0, 0.1, 4.4), DVec3::new(0.0, 1.5, 4.2), DVec3::new(0.0, -0.42, -1.0), DVec3::new(0.0, -0.42, -1.0)];
    s.gear = Box::leak(Box::new([
        leg(DVec3::new(0.0, -0.70, -2.95), 0.10, 18_000.0, 0.0, false, "NOSE WHEEL"),
        leg(DVec3::new(0.0, -0.69, 0.15), 0.19, 40_000.0, 0.0, true, "MAIN WHEEL"),
        // the wing tip skids it leans on, standing.
        leg(DVec3::new(-8.2, 0.05, -0.3), 0.05, 6_000.0, 0.0, false, "LEFT TIP SKID"),
        leg(DVec3::new(8.2, 0.05, -0.3), 0.05, 6_000.0, 0.0, false, "RIGHT TIP SKID"),
        leg(DVec3::new(0.0, -0.35, 4.3), 0.05, 8_000.0, 0.0, false, "TAIL SKID"),
    ]));
    s.groups = [("fuselage", 120.0), ("tail", 25.0), ("left wing", 95.0), ("right wing", 95.0), ("release", 1.0), ("propeller", 0.0), ("gear", 10.0)];
    s.hole = aircraft::Hole { fus: [0.36, 0.46, -0.08, 0.3], wing: [0.18, 0.07, 0.09, 0.04], stab: [1.46, 0.04, 1.6], fin: [0.3, 1.15, 0.05, 0.03], pods: [[0.0; 3]; 2] };
    // two aboard at about 80 kg each on 360 kg empty.
    s.mass = 520.0;
    s.fuel_kg = 0.0;
    s.fuel_flow = [0.0; 3];
    s.payload = [160.0, 240.0, -0.3];
    s.mac = 1.0;
    s.cg = [0.30, 0.22, 0.42];
    s.fuel_arm = 0.0;
    // a long span: much slower to roll and yaw than a light aircraft.
    s.inertia = DVec3::new(950.0, 3400.0, 2900.0);
    s.wing_area = 17.95;
    s.b = 17.0;
    // aspect ratio 16.1 and a laminar section: best glide 34.
    s.cl0 = 0.40;
    s.cl_a = 5.6;
    s.cd0 = 0.0098;
    s.k_ind = 0.022;
    s.cd_gear = 0.0;
    s.cd_flap = [0.0, 0.0];
    s.flap_cl = 0.0;
    s.stall_alpha = 0.18;
    s.stall_flap = 0.0;
    s.stall_slat = 0.0;
    s.alpha_trim = 0.17;
    s.mcrit = 0.0;
    s.cd_wave = 0.0;
    // the airbrakes on the speedbrake key: drag and some lift dumped.
    s.spoiler_cd = 0.045;
    s.spoiler_cl = 0.22;
    s.chute = None;
    s.flaps = &[0.0];
    s.vfe = &[1e9];
    s.lever = None;
    s.slats_at = 0;
    s.gear_rate = 0.0;
    s.vne = 151.0 * KT;
    s.mmo = 0.0;
    s.flutter_v = 175.0 * KT;
    s.g_limit = 5.3;
    s.g_break = 8.0;
    s.steer_max = 0.0;
    // no engine: a piston of no power and no fuel never runs.
    if let Engine::Piston(p) = &mut s.engine {
        p.power = 1.0;
        p.prop_d = 0.05;
        p.static_thrust = 0.0;
    }
    s.handling.roll_ctl *= 0.7;
    s.handling.roll_d *= 1.3;
    s.handling.yaw_ctl *= 0.8;
    s.handling.adverse = 0.55;
    s.cruise = 52.0 * KT;
    s.approach = (55.0 * KT, 0);
    s.cam_dist = 16.0;
    s.shadow_r = 9.0;
    s.floats = false;
    s
}
