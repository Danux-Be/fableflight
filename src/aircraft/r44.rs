//! the robinson r44 raven ii, a four seat piston helicopter. from robinson's
//! specifications and pilot's operating handbook: main rotor 10.06 m across
//! (two blades, 400 rpm), tail rotor 1.47 m on the left of the boom, 11.66 m
//! long overall with the rotors turning, 3.28 m tall, skids 2.18 m apart,
//! lycoming io-540 of 245 hp derated to 205 hp for takeoff, 1134 kg most,
//! 46.7 us gal usable, never exceed 130 kias, cruise about 110 ktas. the
//! pilot flies from the right seat. built here, kept simple: the cabin and
//! its bubble, the boom, both rotors, the skids, and the instrument console.

use crate::aircraft::c172::cabin::gauge;
use crate::aircraft::{self, Hotspot, Leg, Part, Rotor, Spec};
use crate::airframe::{mat, mt, Anim, Builder, Drive, Input, Mat, Mesh, Model, Role, BLACK, G_ENGINE, G_FUSE, G_GEAR, G_PROP, G_TAIL, RUBBER};
use glam::{DVec3, Quat, Vec3};

const KT: f64 = 1.0 / 1.943_84;
const NOSE_Z: f32 = -2.30;
const REAR_Z: f32 = 0.95;
const HUB: Vec3 = Vec3::new(0.0, 2.0, 0.3);
const TAIL_HUB: Vec3 = Vec3::new(-0.26, 0.88, 6.2);
const SKID_Y: f32 = -1.10;
const EYE: Vec3 = Vec3::new(0.33, 0.42, -0.95);

const PAINT: Mat = mt(mat::PAINT, 0.62, 0.05, 0.05);
const WHITE: Mat = mt(mat::PAINT, 0.88, 0.88, 0.86);
const GLASS: Mat = mt(mat::GLASS, 0.6, 0.7, 0.75);
const ALU: Mat = mt(mat::ALU, 0.62, 0.63, 0.65);
const FACE: Mat = mt(mat::GAUGE, 0.02, 0.02, 0.02);

/// the cabin's half width, half height and middle at station z.
fn cabin_at(z: f32) -> (f32, f32, f32) {
    let k = ((z - NOSE_Z) / (REAR_Z - NOSE_Z)).clamp(0.0, 1.0);
    let round = (1.0 - (1.0 - k).powi(2)).max(0.0).sqrt();
    (0.64 * round.max(0.02), 0.78 * round.max(0.02) * (1.0 - 0.15 * k), 0.30 - 0.05 * k)
}

/// the cabin as rings of quads: the front and upper part glass, the rest painted.
fn cabin(b: &mut Builder) {
    let (mut paint, mut glass) = (Mesh::default(), Mesh::default());
    let (n, segs) = (28, 28);
    let ring = |i: usize, k: usize| -> (Vec3, Vec3) {
        let z = NOSE_Z + (REAR_Z - NOSE_Z) * i as f32 / n as f32;
        let (w, h, y) = cabin_at(z);
        let a = k as f32 / segs as f32 * std::f32::consts::TAU;
        (Vec3::new(w * a.cos(), y + h * a.sin(), z), Vec3::new(a.cos() / w.max(0.02), a.sin() / h.max(0.02), -0.2).normalize_or_zero())
    };
    for i in 0..n {
        for k in 0..segs {
            let q = [ring(i, k), ring(i, k + 1), ring(i + 1, k + 1), ring(i + 1, k)];
            let mid = (q[0].0 + q[2].0) * 0.5;
            // the bubble: everything ahead of the doors above the floor, and the roof over the seats.
            let is_glass = (mid.z < -0.55 && mid.y > -0.25) || (mid.y > 0.75 && mid.z < 0.2);
            let m = if is_glass { &mut glass } else { &mut paint };
            let mat = if is_glass { &GLASS } else if mid.y > -0.1 { &PAINT } else { &WHITE };
            let base = m.v.len() as u32;
            for (p, nn) in q {
                m.push(p, nn, [p.x, p.z], mat, 0.0);
            }
            m.i.extend([base, base + 2, base + 1, base, base + 3, base + 2]);
        }
    }
    b.fixed("cabin", G_FUSE, paint);
    let g = b.fixed("bubble", G_FUSE, glass);
    b.pieces[g].glass = true;
    b.pieces[g].occluder = false;
    // the engine cowling behind the cabin, under the mast.
    let mut c = Mesh::default();
    c.rbox(&PAINT, Vec3::new(0.0, 0.55, 1.45), Vec3::new(0.42, 0.42, 0.65), 0.12, Quat::IDENTITY);
    c.tube(&ALU, &[Vec3::new(0.0, 1.0, HUB.z), Vec3::new(0.0, HUB.y, HUB.z)], &|_| 0.06, 12, true);
    b.fixed("cowling and mast", G_ENGINE, c);
}

fn tail(b: &mut Builder) {
    let mut m = Mesh::default();
    m.tube(&PAINT, &[Vec3::new(0.0, 0.55, 1.9), Vec3::new(0.0, 0.88, TAIL_HUB.z)], &|t| 0.17 - 0.1 * t, 14, true);
    // the fins above and below the end of the boom, the small tailplane ahead of them.
    m.rbox(&PAINT, Vec3::new(0.0, 1.25, TAIL_HUB.z + 0.05), Vec3::new(0.02, 0.38, 0.22), 0.01, Quat::from_rotation_x(-0.35));
    m.rbox(&PAINT, Vec3::new(0.0, 0.50, TAIL_HUB.z + 0.05), Vec3::new(0.02, 0.32, 0.18), 0.01, Quat::from_rotation_x(0.35));
    m.rbox(&WHITE, Vec3::new(0.0, 0.85, TAIL_HUB.z - 0.6), Vec3::new(0.55, 0.02, 0.14), 0.01, Quat::IDENTITY);
    b.fixed("tail boom", G_TAIL, m);
}

/// the rotors: two blades each, turning with the engine (the tail rotor about six times as fast).
fn rotors(b: &mut Builder) {
    let mut m = Mesh::default();
    for s in [-1.0f32, 1.0] {
        m.rbox(&BLACK, HUB + Vec3::new(s * 2.6, 0.05, 0.0), Vec3::new(2.43, 0.02, 0.127), 0.01, Quat::IDENTITY);
    }
    m.rbox(&ALU, HUB + Vec3::Y * 0.05, Vec3::new(0.2, 0.06, 0.08), 0.02, Quat::IDENTITY);
    let r = b.add("main rotor", G_PROP, m, Anim::hinge(Drive::new(Input::Prop, -1.0)), HUB, Vec3::Y, None);
    b.role(r, Role::Prop);
    let mut t = Mesh::default();
    for s in [-1.0f32, 1.0] {
        t.rbox(&WHITE, TAIL_HUB + Vec3::new(0.0, s * 0.39, 0.0), Vec3::new(0.012, 0.35, 0.05), 0.005, Quat::IDENTITY);
    }
    let r = b.add("tail rotor", G_PROP, t, Anim::hinge(Drive::new(Input::Prop, 6.07)), TAIL_HUB, Vec3::X, None);
    b.role(r, Role::Prop);
}

fn skids(b: &mut Builder) {
    let mut m = Mesh::default();
    for s in [-1.0f32, 1.0] {
        let x = s * 1.09;
        m.tube(&ALU, &[Vec3::new(x, SKID_Y + 0.25, -1.75), Vec3::new(x, SKID_Y + 0.05, -1.5), Vec3::new(x, SKID_Y, -1.2), Vec3::new(x, SKID_Y, 1.25)], &|_| 0.035, 10, true);
        for z in [-0.75f32, 0.7] {
            m.tube(&ALU, &[Vec3::new(x, SKID_Y, z), Vec3::new(x * 0.75, SKID_Y + 0.45, z), Vec3::new(x * 0.35, -0.42, z)], &|_| 0.038, 10, true);
        }
    }
    b.fixed("skids", G_GEAR, m);
}

/// the seats, the console between them with the instruments, the cyclic, the collective, the key.
fn cockpit(b: &mut Builder) {
    let mut m = Mesh::default();
    for x in [-0.33f32, 0.33] {
        m.rbox(&mt(mat::PAINT, 0.10, 0.10, 0.11), Vec3::new(x, -0.25, -0.55), Vec3::new(0.24, 0.05, 0.25), 0.04, Quat::IDENTITY);
        m.rbox(&mt(mat::PAINT, 0.10, 0.10, 0.11), Vec3::new(x, 0.15, -0.30), Vec3::new(0.24, 0.40, 0.05), 0.04, Quat::from_rotation_x(-0.15));
    }
    m.rbox(&BLACK, Vec3::new(0.0, -0.45, -1.2), Vec3::new(0.6, 0.02, 0.6), 0.02, Quat::IDENTITY);
    // the console: a pedestal rising to the instrument panel.
    let panel = Vec3::new(0.0, 0.08, -1.55);
    let tilt = Quat::from_rotation_x(-0.5);
    m.rbox(&mt(mat::PAINT, 0.08, 0.08, 0.09), panel, Vec3::new(0.36, 0.17, 0.03), 0.02, tilt);
    m.rbox(&mt(mat::PAINT, 0.08, 0.08, 0.09), Vec3::new(0.0, -0.25, -1.45), Vec3::new(0.12, 0.2, 0.1), 0.02, Quat::IDENTITY);
    let n = tilt * Vec3::Z;
    let up = tilt * Vec3::Y;
    for (id, x, y) in [(gauge::ASI, -0.22f32, 0.07f32), (gauge::ALT, -0.07, 0.07), (gauge::VSI, 0.08, 0.07), (gauge::TACH, 0.23, 0.07), (gauge::MP, 0.23, -0.07), (gauge::AI, 0.08, -0.07)] {
        let c = panel + Vec3::X * x + up * y + n * 0.032;
        m.disc(&FACE.x(id), c, n, up, 0.055, 36);
        m.lathe(&BLACK, c - n * 0.002, n, &[[0.055, 0.0], [0.061, 0.004], [0.061, 0.006], [0.055, 0.006]], 36, false);
    }
    b.fixed("console", G_FUSE, m);
    // the cyclic: a centre post with the t bar across in front of both seats.
    let mut cy = Mesh::default();
    let base = Vec3::new(0.0, -0.40, -0.95);
    cy.tube(&BLACK, &[base, base + Vec3::new(0.0, 0.42, -0.05)], &|_| 0.015, 10, true);
    cy.tube(&BLACK, &[base + Vec3::new(-0.30, 0.47, -0.08), base + Vec3::new(0.30, 0.47, -0.08)], &|_| 0.014, 10, true);
    for s in [-1.0f32, 1.0] {
        cy.lathe(&RUBBER, base + Vec3::new(s * 0.30, 0.47, -0.08), Vec3::Y, &[[0.0, -0.06], [0.02, -0.06], [0.022, 0.05], [0.0, 0.06]], 10, true);
    }
    b.add("cyclic", G_FUSE, cy, Anim::hinge2(Drive::new(Input::Pitch, -0.18), Drive::NONE), base, Vec3::X, None);
    // the collective on the pilot's left, its twist grip at the end: up for more.
    let mut co = Mesh::default();
    let piv = Vec3::new(0.08, -0.38, -0.25);
    co.tube(&BLACK, &[piv, piv + Vec3::new(0.0, 0.08, -0.55)], &|_| 0.016, 10, true);
    co.lathe(&RUBBER, piv + Vec3::new(0.0, 0.08, -0.55), Vec3::Z, &[[0.0, 0.0], [0.022, 0.0], [0.024, -0.12], [0.0, -0.13]], 10, true);
    let k = b.add("collective", G_FUSE, co, Anim::hinge(Drive::new(Input::Throttle, 0.35).offset(-0.1)), piv, Vec3::X, None);
    b.pieces[k].hotspot = Some(Hotspot::wheel(Vec3::new(0.0, 0.08, -0.6), 0.04, "throttle_up", "throttle_down"));
    // the key (on, then turned to start) and the battery switch on the console.
    let mut key = Mesh::default();
    let kp = Vec3::new(-0.08, -0.12, -1.36);
    key.rbox(&ALU, kp, Vec3::new(0.012, 0.02, 0.004), 0.003, Quat::IDENTITY);
    let k = b.add("ignition key", G_FUSE, key, Anim::hinge(Drive::new(Input::Ignition, 0.6)), kp, Vec3::Z, None);
    b.pieces[k].hotspot = Some(Hotspot { at: [0.0, 0.0, -0.01], r: 0.025, click: Some("ignition"), hold: Some("starter"), wheel: None });
    let mut sw = Mesh::default();
    let sp = Vec3::new(0.08, -0.12, -1.36);
    sw.rbox(&WHITE, sp, Vec3::new(0.006, 0.016, 0.006), 0.002, Quat::IDENTITY);
    let k = b.add("master switch", G_FUSE, sw, Anim::hinge(Drive::new(Input::Master, 0.5).offset(-0.25)), sp, Vec3::X, None);
    b.pieces[k].hotspot = Some(Hotspot::click(Vec3::ZERO, 0.02, "master"));
}

fn build() -> Model {
    let mut b = Builder::default();
    cabin(&mut b);
    tail(&mut b);
    rotors(&mut b);
    skids(&mut b);
    cockpit(&mut b);
    Model::finish(b.pieces)
}

fn skid(x: f64, z: f64, name: &'static str) -> Leg {
    Leg { at: DVec3::new(x, SKID_Y as f64 - 0.035, z), wheel_r: 0.04, travel: 0.06, k: 60_000.0, c: 6_000.0, nf_max: 60_000.0, steer: 0.0, brakes: false, side_k: 4000.0, side_mu: 0.5, roll_k: 4000.0, roll_mu: 0.5, brake_k: 4000.0, brake_mu: 0.6, fold_vn: 3.8, fold_kick: DVec3::new(0.5, 0.0, 0.0), name }
}

/// a picture for the aircraft list: the helicopter from the side, drawn here.
fn still() -> &'static [u8] {
    let (w, h) = (680u32, 392u32);
    let mut img = image::RgbaImage::new(w, h);
    let (cx, cy) = (w as f32 * 0.42, h as f32 * 0.55);
    for (x, y, px) in img.enumerate_pixels_mut() {
        let t = y as f32 / h as f32;
        let sky = [90.0 + 90.0 * t, 130.0 + 70.0 * t, 190.0 + 40.0 * t];
        let (u, v) = ((x as f32 - cx) / 26.0, (cy - y as f32) / 26.0);
        let cabin = (u / 1.7).powi(2) + ((v - 0.2) / 1.0).powi(2) < 1.0;
        let boom = u > 0.8 && u < 6.6 && (v - 0.55 - (u - 0.8) * 0.05).abs() < 0.16 - 0.015 * u;
        let fin = u > 6.2 && u < 6.6 && v > 0.4 && v < 1.6;
        let rotor = (v - 2.1).abs() < 0.06 && u.abs() < 5.0;
        let skidl = (v + 1.1).abs() < 0.07 && u > -1.6 && u < 1.4;
        let c = if cabin || boom || fin { [190.0, 25.0, 25.0] } else if rotor || skidl { [40.0, 40.0, 44.0] } else { sky };
        *px = image::Rgba([c[0] as u8, c[1] as u8, c[2] as u8, 255]);
    }
    let mut out = std::io::Cursor::new(Vec::new());
    let _ = img.write_to(&mut out, image::ImageFormat::Png);
    Box::leak(out.into_inner().into_boxed_slice())
}

/// the r44's type, on the 172's (the same engine family, the same instruments).
pub fn spec() -> Spec {
    let mut s = aircraft::c172::SPEC.clone();
    s.id = 11;
    s.name = "Robinson R44 Raven II";
    s.keys = &["r44", "helicopter", "heli"];
    s.blurb = "Four-seat piston helicopter. The throttle keys are the collective, the stick the cyclic, the rudder the pedals.";
    s.still = still();
    s.paint = None;
    s.build = build;
    s.span = 0.7;
    s.eye = EYE;
    s.eye2 = None;
    s.sees_inside = |_, d| d.y < -0.6;
    s.hotspots = &[];
    s.contacts = &[([0.0, -0.45, -1.2], Part::Belly), ([0.0, -0.4, 0.4], Part::Belly), ([0.0, 0.9, -1.0], Part::Roof)];
    s.tail_pts = &[[0.0, 0.6, 3.0], [0.0, 0.85, 6.2]];
    s.hard = [DVec3::new(0.0, 0.2, -2.3), DVec3::new(-0.64, 0.2, 0.0), DVec3::new(0.64, 0.2, 0.0), DVec3::new(0.0, 0.88, 6.4), DVec3::new(0.0, 1.6, 6.3), DVec3::new(0.0, -0.45, 0.0), DVec3::new(0.0, -1.13, 0.0)];
    s.gear = Box::leak(Box::new([skid(-1.09, -1.0, "LEFT SKID"), skid(1.09, -1.0, "RIGHT SKID"), skid(-1.09, 1.0, "LEFT SKID AFT"), skid(1.09, 1.0, "RIGHT SKID AFT")]));
    s.groups = [("fuselage", 420.0), ("tail", 40.0), ("left wing", 0.0), ("right wing", 0.0), ("engine", 180.0), ("rotors", 60.0), ("skids", 25.0)];
    // two aboard, half the fuel: 680 kg empty.
    s.mass = 1000.0;
    s.fuel_kg = 127.0;
    s.fuel_flow = [11.0 / 3600.0, 48.0 / 3600.0, 48.0 / 3600.0];
    s.payload = [160.0, 380.0, 0.0];
    s.inertia = DVec3::new(1350.0, 1400.0, 520.0);
    s.gear_rate = 0.0;
    s.vne = 130.0 * KT;
    s.cruise = 90.0 * KT;
    s.approach = (50.0 * KT, 0);
    s.flaps = &[0.0];
    s.vfe = &[1e9];
    s.cam_dist = 16.0;
    s.shadow_r = 7.0;
    s.rotor = Some(Rotor { radius: 5.03, rpm: 400.0, thrust: 1.45 * 1134.0 * 9.81, hub: HUB.as_dvec3(), tail: TAIL_HUB.as_dvec3(), drag_area: 1.6, cyclic: 3.0, pedal: 2600.0, vne: 130.0 * KT });
    s
}
