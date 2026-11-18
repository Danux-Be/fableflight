//! The moment-based flight model (six degrees of freedom) for a type with an
//! `aircraft::Aero`: the aerodynamic forces and moments built up from its
//! stability and control derivatives, the propeller's moments, and Euler's
//! rigid-body equations that turn the moments into angular accelerations.
//!
//! The game's body frame is +X right, +Y up, −Z forward. The derivatives
//! live in the aerodynamic body axes (x forward, y right, z down), so rates
//! and moments cross over here:
//!   p = −ω.z, q = ω.x, r = −ω.y;  torque.x = M, torque.y = −N, torque.z = −L.
use crate::aircraft::{Aero, Spec};
use glam::DVec3;

/// Game body-frame angular vector → aerodynamic axes (p, q, r) / (L, M, N).
pub fn to_aero(w: DVec3) -> DVec3 {
    DVec3::new(-w.z, w.x, -w.y)
}

/// Aerodynamic axes (p, q, r) / (L, M, N) → game body frame.
pub fn from_aero(a: DVec3) -> DVec3 {
    DVec3::new(a.y, -a.z, -a.x)
}

/// What the air is doing to the airframe this instant, and what is left of
/// the airframe to do it with.
pub struct State {
    /// angle of attack, sideslip (rad), its filtered rate of change of α (rad/s)
    pub alpha: f64,
    pub beta: f64,
    pub alpha_dot: f64,
    /// true airspeed (m/s) and dynamic pressure (Pa)
    pub v: f64,
    pub qbar: f64,
    /// body rates relative to the air (game frame, rad/s)
    pub omega: DVec3,
    /// smoothed stick (pitch, yaw, roll; −1..1) and the trimmed angle of attack
    pub stick: DVec3,
    pub trim: f64,
    /// flap extension (0..1) and the share of the flaps still there
    pub flaps: f64,
    pub flap_eff: f64,
    /// how far into the stall (0..1); the lift coefficient as it is now; the CG (fraction of MAC)
    pub stall: f64,
    pub cl: f64,
    pub cg: f64,
    /// what is left: elevator (with its tailplane), tailplane, fin, rudder, aileron authority
    pub elevator: f64,
    pub stab: f64,
    pub fin: f64,
    pub rudder: f64,
    pub ailerons: f64,
    /// a stalled wing's tendency to drop (−1 left .. 1 right, it wanders)
    pub drop: f64,
    /// the propeller slipstream's extra dynamic pressure, T/A (Pa)
    pub wash_q: f64,
}

/// Aerodynamic increments over `lift_drag`, and the moments.
pub struct Out {
    /// lift coefficient from pitch rate, α̇ and the elevator, and side force coefficient
    pub d_cl: f64,
    pub cy: f64,
    /// L, M, N (N m, aerodynamic axes, about the CG)
    pub moment: DVec3,
}

/// The elevator deflection (rad, trailing edge down positive) that holds the
/// angle of attack `alpha` with the flaps up, the CG at its reference and no
/// rotation: what a trim tab set for that angle is worth, as an elevator
/// angle. The trim wheel therefore still reads as a trimmed angle of attack
/// (`Plane::trim`), but load, flaps and power move the aircraft off it as
/// they would a real one.
pub fn trim_elevator(spec: &Spec, ae: &Aero, alpha: f64) -> f64 {
    let cl = spec.cl0 + spec.cl_a * alpha;
    -(ae.pitch_0 + ae.pitch_a * alpha.sin() + cl * (spec.cg[0] - ae.cg_ref)) / ae.pitch_de
}

/// The trim setting (`Plane::trim`, read as the angle of attack the tab
/// holds clean at the reference CG) that holds `alpha` in steady flight
/// with the flaps at `flaps` (0..1) and the CG at `cg`: what a pilot winds
/// in after setting up a configuration.
pub fn trim_for(spec: &Spec, ae: &Aero, alpha: f64, flaps: f64, cg: f64) -> f64 {
    let cl = spec.cl0 + spec.flap_cl * flaps + spec.cl_a * alpha;
    let de = -(ae.pitch_0 + ae.pitch_a * alpha.sin() + ae.pitch_flap * flaps + cl * (cg - ae.cg_ref)) / ae.pitch_de;
    // trim_elevator is smooth and monotonic in the setting: a few Newton steps.
    let mut t = alpha;
    for _ in 0..8 {
        let f = trim_elevator(spec, ae, t) - de;
        let d = (trim_elevator(spec, ae, t + 1e-4) - trim_elevator(spec, ae, t - 1e-4)) / 2e-4;
        if d.abs() < 1e-9 {
            break;
        }
        t -= f / d;
    }
    t
}

fn smooth(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Builds the coefficients up from the derivatives.
pub fn aero(spec: &Spec, ae: &Aero, s: &State) -> Out {
    let v = s.v.max(1.0);
    let (b, c) = (spec.b, spec.mac);
    let w = to_aero(s.omega);
    let (ph, qh, rh) = (w.x * b / (2.0 * v), w.y * c / (2.0 * v), w.z * b / (2.0 * v));
    let adh = s.alpha_dot * c / (2.0 * v);

    // The surfaces. The stick sets the elevator (more travel up than down),
    // the trim tab adds its own; the pedals the rudder, the wheel the ailerons.
    let de_stick = if s.stick.x > 0.0 { -s.stick.x * ae.de_max[0] } else { -s.stick.x * ae.de_max[1] };
    let de = de_stick + trim_elevator(spec, ae, s.trim);
    let da = s.stick.z * ae.da_max;
    let dr = -s.stick.y * ae.dr_max;
    // The tail sits in the propeller's slipstream: low and slow at full
    // power the elevator and rudder bite as they would much faster.
    let wash = 1.0 + (ae.prop_wash * s.wash_q / s.qbar.max(1.0)).min(6.0);

    // The flow is the right way round (not parked tail into the wind).
    let ahead = smooth(0.0, 0.3, s.alpha.cos());
    let stall = s.stall * ahead;

    // Lift and side force increments.
    let d_cl = ae.lift_q * qh + ae.lift_adot * adh + ae.lift_de * de * s.elevator;
    let cy = ae.side_b * s.beta.sin() + ae.side_p * ph + ae.side_r * rh * (0.3 + 0.7 * s.fin) + ae.side_dr * dr * s.rudder * wash;

    // Pitch. The tailplane gives most of the stiffness and nearly all the
    // damping; without it the wing and fuselage alone are unstable. Off the
    // reference CG the lift's own arm moves the balance: aft, less stable.
    let tail_k = 0.15 + 0.85 * s.stab;
    let pitch_a = (ae.pitch_a - ae.pitch_a_tail) + ae.pitch_a_tail * s.stab;
    let cm = ae.pitch_0
        + pitch_a * s.alpha.sin()
        + ae.pitch_flap * s.flaps * s.flap_eff
        + ae.pitch_q * qh * tail_k
        + ae.pitch_adot * adh * tail_k
        + ae.pitch_de * de * s.elevator * wash
        + s.cl * (s.cg - ae.cg_ref)
        + ae.pitch_stall * stall * s.alpha.signum();

    // Roll. Its damping fades through the stall and past it turns round (the
    // wing going down meets the air at a still higher angle and loses more
    // lift: autorotation); one wing goes first, and a yaw rate drops the
    // retreating one (a spin).
    let roll_damp = (1.0 - 1.5 * stall).max(-0.35);
    let cl_roll = ae.roll_0
        + ae.roll_b * s.beta.sin()
        + ae.roll_p * roll_damp * ph
        + (ae.roll_r + ae.roll_stall_r * stall) * rh
        + ae.roll_da * da * s.ailerons
        + ae.roll_dr * dr * s.rudder * wash
        - ae.roll_stall_drop * stall * s.drop;

    // Yaw. The fin gives the weathercock stiffness (the fuselage alone is
    // unstable) and most of the damping.
    let yaw_b = (ae.yaw_b - ae.yaw_b_fin) + ae.yaw_b_fin * s.fin;
    let cn = ae.yaw_0 + yaw_b * s.beta.sin() + ae.yaw_p * ph + ae.yaw_r * rh * (0.3 + 0.7 * s.fin) + ae.yaw_da * da * s.ailerons + ae.yaw_dr * dr * s.rudder * wash;

    let qs = s.qbar * spec.wing_area;
    Out { d_cl, cy, moment: DVec3::new(cl_roll * qs * b, cm * qs * c, cn * qs * b) }
}

/// The propeller's moments (aerodynamic axes): the reaction to the torque it
/// absorbs (it turns clockwise seen from the cockpit, so the airframe rolls
/// left), its spiralling slipstream on the fin and its P-factor (both yaw
/// left), the thrust line off the CG, and the gyroscopic moment of the
/// spinning propeller and crankshaft (pitching up yaws right, as on every
/// clockwise single).
pub fn propeller(ae: &Aero, thrust: f64, torque: f64, alpha: f64, spin_h: f64, omega: DVec3) -> DVec3 {
    let w = to_aero(omega);
    let yaw = -thrust.max(0.0) * ae.prop_yaw_arm * (1.0 + ae.prop_p_factor * alpha.clamp(0.0, 0.35));
    // −ω × h, with h = (spin_h, 0, 0)
    let gyro = DVec3::new(0.0, -w.z * spin_h, w.y * spin_h);
    DVec3::new(-torque, -thrust * ae.thrust_y, yaw) + gyro
}

/// Euler's equations for a body with an Ixz product of inertia:
///   I·ω̇ = M − ω × (I·ω)
/// `inertia` is `Spec::inertia` (pitch, yaw, roll); `omega` and the result
/// are in the game's body frame, `moment` in aerodynamic axes.
pub fn euler(inertia: DVec3, ixz: f64, omega: DVec3, moment: DVec3) -> DVec3 {
    let (iyy, izz, ixx) = (inertia.x, inertia.y, inertia.z);
    let w = to_aero(omega);
    let (p, q, r) = (w.x, w.y, w.z);
    let l = moment.x + (iyy - izz) * q * r + ixz * p * q;
    let m = moment.y + (izz - ixx) * p * r + ixz * (r * r - p * p);
    let n = moment.z + (ixx - iyy) * p * q - ixz * q * r;
    let gamma = ixx * izz - ixz * ixz;
    let pd = (izz * l + ixz * n) / gamma;
    let rd = (ixz * l + ixx * n) / gamma;
    from_aero(DVec3::new(pd, m / iyy, rd))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axes_round_trip_and_match_the_game_frame() {
        let w = DVec3::new(0.1, -0.2, 0.3);
        assert!((from_aero(to_aero(w)) - w).length() < 1e-15);
        // Nose up about +X is a positive q; the right wing going down (ω.z < 0) a positive p.
        assert_eq!(to_aero(DVec3::X).y, 1.0);
        assert_eq!(to_aero(DVec3::NEG_Z).x, 1.0);
        // A nose-right yaw (ω.y < 0) is a positive r.
        assert_eq!(to_aero(DVec3::NEG_Y).z, 1.0);
    }

    #[test]
    fn euler_conserves_angular_momentum_and_energy() {
        // A free tumbling body: no moment, so I·ω keeps its length and the
        // kinetic energy stays put (an asymmetric top, with Ixz).
        let inertia = DVec3::new(1825.0, 2667.0, 1285.0);
        let ixz = 120.0;
        let mut w = DVec3::new(0.4, 0.3, -0.8);
        let tensor = |w: DVec3| {
            let a = to_aero(w);
            DVec3::new(inertia.z * a.x - ixz * a.z, inertia.x * a.y, inertia.y * a.z - ixz * a.x)
        };
        let energy = |w: DVec3| 0.5 * to_aero(w).dot(tensor(w));
        let (h0, e0) = (tensor(w).length(), energy(w));
        let dt = 1e-4;
        for _ in 0..20_000 {
            // midpoint (second order) so the check is on the equations, not the integrator
            let k1 = euler(inertia, ixz, w, DVec3::ZERO);
            let k2 = euler(inertia, ixz, w + k1 * dt * 0.5, DVec3::ZERO);
            w += k2 * dt;
        }
        // (|I·ω| is conserved in the inertial frame; in body axes its length is.)
        assert!((tensor(w).length() / h0 - 1.0).abs() < 1e-4, "{} {}", tensor(w).length(), h0);
        assert!((energy(w) / e0 - 1.0).abs() < 1e-4);
    }
}
