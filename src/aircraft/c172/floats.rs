//! the 172 on floats, a game variant: the 172rg's airframe and cockpit on a
//! pair of edo 89-2000 style pontoons in place of its wheels. (no 172rg flew
//! on floats: the real float 172s are the fixed gear models.) the floats are
//! about 6.1 m long and 0.75 m across, 2.3 m apart, keels some 0.6 m below
//! where the wheels touched; they add about 90 kg and some drag, and cost the
//! 172 about 15 kt of cruise. figures from edo's float data and pilots'
//! reports, rounded: estimates, not a certified configuration.

use crate::aircraft::{Leg, Spec};
use crate::airframe::{mt, mat, Mesh, Model, Role, Vis, G_GEAR, N_GROUPS};
use glam::{DVec3, Vec3};

const KT: f64 = 1.0 / 1.943_84;
/// the floats' centres either side, the keel's height, the bow and the stern.
const X: f32 = 1.15;
const KEEL: f32 = -2.12;
const DECK: f32 = -1.52;
const BOW: f32 = -3.45;
const STERN: f32 = 2.65;

fn float_leg(x: f64, z: f64, steer: f64, name: &'static str) -> Leg {
    Leg { at: DVec3::new(x, KEEL as f64, z), wheel_r: 0.3, travel: 0.6, k: 17_000.0, c: 4_500.0, nf_max: 45_000.0, steer, brakes: false, side_k: 3_000.0, side_mu: 0.8, roll_k: 3_000.0, roll_mu: 0.45, brake_k: 3_000.0, brake_mu: 0.45, fold_vn: 6.0, fold_kick: DVec3::new(0.3, 0.0, 0.0), name }
}

pub fn spec() -> Spec {
    let mut s = super::SPEC.clone();
    s.name = "Cessna 172 on floats";
    s.keys = &["c172f", "floatplane", "seaplane"];
    s.blurb = "A game variant: the 172RG on a pair of floats. Takes off from and lands on water; slides on land.";
    s.build = build;
    s.floats = true;
    s.gear_rate = 0.0;
    s.cd_gear = 0.014;
    s.mass += 90.0;
    s.cruise = 105.0 * KT;
    let legs: Vec<Leg> = vec![
        float_leg(-X as f64, -1.45, 0.0, "LEFT FLOAT"),
        float_leg(X as f64, -1.45, 0.0, "RIGHT FLOAT"),
        float_leg(-X as f64, 0.55, 0.35, "LEFT FLOAT STEP"),
        float_leg(X as f64, 0.55, 0.35, "RIGHT FLOAT STEP"),
    ];
    s.gear = Box::leak(legs.into_boxed_slice());
    s
}

/// the 172 with its wheels, legs and gear doors gone and the floats on.
fn build() -> Model {
    let mut model = (super::SPEC.build)();
    for p in &mut model.pieces {
        if p.group == G_GEAR && (matches!(p.role, Role::Gear(_)) || p.name.contains("door") || p.name.contains("wheel") || p.name.contains("leg")) {
            p.mesh = Mesh::default();
        }
    }
    let paint = mt(mat::PAINT, 0.82, 0.82, 0.80);
    let alu = mt(mat::ALU, 0.62, 0.63, 0.65);
    let mut m = Mesh::default();
    // each float: rounded at the bow, full through the middle, the step, tapering to the stern.
    let prof: [[f32; 2]; 9] = [[0.0, BOW], [0.16, BOW + 0.12], [0.30, BOW + 0.45], [0.37, BOW + 1.1], [0.38, 0.35], [0.33, 0.42], [0.28, 1.5], [0.14, STERN - 0.15], [0.0, STERN]];
    let mid = (KEEL + DECK) * 0.5;
    for sx in [-1.0f32, 1.0] {
        let start = m.v.len();
        m.lathe(&paint, Vec3::new(0.0, 0.0, 0.0), Vec3::Z, &prof, 12, true);
        // flattened to a float's section: wider than deep.
        let squash = glam::Mat4::from_translation(Vec3::new(sx * X, mid, 0.0)) * glam::Mat4::from_scale(Vec3::new(1.0, (DECK - KEEL) / 0.76, 1.0));
        m.transform(start, squash);
    }
    // struts up to the fuselage, and the spreader bars between the floats.
    for z in [-1.55f32, 0.5] {
        for sx in [-1.0f32, 1.0] {
            m.tube(&alu, &[Vec3::new(sx * X, DECK + 0.02, z), Vec3::new(sx * 0.42, -0.70, z * 0.9)], &|_| 0.035, 8, true);
        }
        m.tube(&alu, &[Vec3::new(-X, DECK + 0.05, z), Vec3::new(X, DECK + 0.05, z)], &|_| 0.025, 6, true);
    }
    // a diagonal brace each side, and the water rudders at the sterns.
    for sx in [-1.0f32, 1.0] {
        m.tube(&alu, &[Vec3::new(sx * X, DECK + 0.02, 0.5), Vec3::new(sx * 0.42, -0.70, -1.4)], &|_| 0.022, 6, true);
        m.tube(&alu, &[Vec3::new(sx * X, KEEL + 0.18, STERN - 0.05), Vec3::new(sx * X, KEEL - 0.12, STERN + 0.12)], &|_| 0.03, 6, true);
    }
    let mut b = crate::airframe::Builder { pieces: std::mem::take(&mut model.pieces) };
    let i = b.fixed("floats", G_GEAR, m);
    b.pieces[i].occluder = true;
    let pieces = b.pieces;
    let mut groups = vec![Mesh::default(); N_GROUPS];
    for p in &pieces {
        if !p.glass && p.vis != Vis::Chute {
            groups[p.group].append(&p.mesh);
        }
    }
    Model { pieces, groups, tex: None }
}
