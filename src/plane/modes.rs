//! The moment-based model's dynamic modes, measured from the full nonlinear
//! simulation and checked against the classical approximations built from
//! the same derivatives (Etkin & Reid; Nelson, *Flight Stability and
//! Automatic Control*, ch. 4 and 5) and against the 172's known character.
//!
//! Each test trims the aircraft hands off, disturbs it, records one state and
//! fits the free response with Prony's method (a linear-prediction model of
//! the samples whose characteristic roots are the modes' discrete poles).
use super::*;
use crate::aircraft::c172;
use std::sync::Arc;

const C172: &Spec = &c172::SPEC;
const G: f64 = 9.81;

fn empty_world() -> World {
    let dir = std::env::temp_dir().join("fablegame-test-cache");
    let src = Arc::new(crate::sources::Sources::new(crate::fetch::Fetcher::new(dir, true), String::new()));
    World::new(crate::sources::Loader::new(src, 0), 14)
}

#[derive(Clone, Copy, Debug)]
struct C(f64, f64);
impl C {
    fn sub(self, o: C) -> C {
        C(self.0 - o.0, self.1 - o.1)
    }
    fn mul(self, o: C) -> C {
        C(self.0 * o.0 - self.1 * o.1, self.0 * o.1 + self.1 * o.0)
    }
    fn div(self, o: C) -> C {
        let d = o.0 * o.0 + o.1 * o.1;
        C((self.0 * o.0 + self.1 * o.1) / d, (self.1 * o.0 - self.0 * o.1) / d)
    }
}

/// A mode: ωn (rad/s) and ζ, from a continuous pole σ ± iω.
#[derive(Debug, Clone, Copy)]
struct Mode {
    sigma: f64,
    omega_d: f64,
}
impl Mode {
    fn wn(&self) -> f64 {
        self.sigma.hypot(self.omega_d)
    }
    fn zeta(&self) -> f64 {
        -self.sigma / self.wn()
    }
    fn period(&self) -> f64 {
        std::f64::consts::TAU / self.omega_d
    }
}

/// Prony's method: the continuous poles of `order` modes fitted to samples `x` taken every `dt`.
fn prony(x: &[f64], dt: f64, order: usize) -> Vec<Mode> {
    let n = order;
    // Least squares on x[k+n] = Σ a_i x[k+n-i], by the normal equations.
    let mut ata = vec![vec![0.0; n]; n];
    let mut atb = vec![0.0; n];
    for k in 0..x.len() - n {
        let row: Vec<f64> = (1..=n).map(|i| x[k + n - i]).collect();
        for i in 0..n {
            atb[i] += row[i] * x[k + n];
            for j in 0..n {
                ata[i][j] += row[i] * row[j];
            }
        }
    }
    // Gaussian elimination with partial pivoting.
    for c in 0..n {
        let piv = (c..n).max_by(|&a, &b| ata[a][c].abs().total_cmp(&ata[b][c].abs())).unwrap();
        ata.swap(c, piv);
        atb.swap(c, piv);
        for r in c + 1..n {
            let f = ata[r][c] / ata[c][c];
            let pivot = ata[c].clone();
            for (x, p) in ata[r].iter_mut().zip(&pivot).skip(c) {
                *x -= f * p;
            }
            atb[r] -= f * atb[c];
        }
    }
    let mut a = vec![0.0; n];
    for r in (0..n).rev() {
        let s: f64 = (r + 1..n).map(|k| ata[r][k] * a[k]).sum();
        a[r] = (atb[r] - s) / ata[r][r];
    }
    // Roots of z^n − a1 z^(n−1) − … − an (Durand–Kerner).
    let poly = |z: C| {
        let mut v = C(1.0, 0.0);
        for ai in &a {
            v = v.mul(z).sub(C(*ai, 0.0));
        }
        v
    };
    let mut roots = vec![C(0.0, 0.0); n];
    let mut seed = C(0.4, 0.9);
    for r in roots.iter_mut() {
        *r = seed;
        seed = seed.mul(C(0.4, 0.9));
    }
    for _ in 0..500 {
        for i in 0..n {
            let mut den = C(1.0, 0.0);
            for j in 0..n {
                if i != j {
                    den = den.mul(roots[i].sub(roots[j]));
                }
            }
            roots[i] = roots[i].sub(poly(roots[i]).div(den));
        }
    }
    roots
        .iter()
        .map(|z| {
            let m = z.0.hypot(z.1);
            Mode { sigma: m.ln() / dt, omega_d: z.1.atan2(z.0).abs() / dt }
        })
        .collect()
}

/// Trimmed hands off at `alt` (m) and `v` (m/s) true, then `kick` applied
/// to it; `rec` recorded every `every` steps for `secs`.
fn record(alt: f64, v: f64, secs: f64, every: usize, kick: impl Fn(&mut Plane), rec: impl Fn(&Plane) -> f64) -> (Vec<f64>, f64) {
    record_with(alt, v, secs, every, kick, rec, |_| Controls::default())
}

/// The wings held level with the ailerons and the ball centred with the
/// pedals, the elevator left alone: the longitudinal modes on their own,
/// without the slow spiral (whose divergence, hands off, is the 172's own).
fn wings_level(p: &Plane) -> Controls {
    let bank = p.right().dot(p.pos.normalize()).asin();
    let vb = p.rot.inverse() * p.airspeed();
    Controls { roll: (bank * 2.0 + p.omega.z * 0.3).clamp(-1.0, 1.0), yaw: (vb.x / vb.length() * 6.0).clamp(-1.0, 1.0), ..Default::default() }
}

fn record_with(alt: f64, v: f64, secs: f64, every: usize, kick: impl Fn(&mut Plane), rec: impl Fn(&Plane) -> f64, pilot: impl Fn(&Plane) -> Controls) -> (Vec<f64>, f64) {
    let w = empty_world();
    let dt = 1.0 / 180.0;
    let mut p = Plane::spawn(C172, 0.6, -2.1, alt, 0.0, v);
    // The bare airframe: no auto-rudder.
    p.auto_rudder = false;
    // Let the engine and the filters settle into the trim first.
    for _ in 0..(2.0 / dt) as usize {
        p.step(dt, &Controls::default(), &w);
    }
    kick(&mut p);
    let mut xs = Vec::new();
    for k in 0..(secs / dt) as usize {
        assert!(p.step(dt, &pilot(&p), &w).is_none());
        if k % every == 0 {
            xs.push(rec(&p));
        }
    }
    (xs, dt * every as f64)
}

/// The cruise of the source data: 5000 ft, 219 ft/s true.
const ALT: f64 = 1524.0;
const V: f64 = 66.75;

struct Dims {
    qs: f64,
    m: f64,
}
fn dims() -> Dims {
    let rho = density(ALT);
    Dims { qs: 0.5 * rho * V * V * C172.wing_area, m: C172.mass }
}

#[test]
fn short_period_matches_its_derivatives() {
    let ae = C172.aero.as_ref().unwrap();
    let d = dims();
    let (c, iyy) = (C172.mac, C172.inertia.x);
    // Nelson's short-period approximation: Mα, Mq, Mα̇ and Zα/u0.
    let k = d.qs * c / iyy;
    let m_a = ae.pitch_a * k;
    let m_q = ae.pitch_q * c / (2.0 * V) * k;
    let m_ad = ae.pitch_adot * c / (2.0 * V) * k;
    let z_a = -(C172.cl_a + C172.cd0) * d.qs / (d.m * V);
    let wn = (z_a * m_q - m_a).sqrt();
    let zeta = -(m_q + m_ad + z_a) / (2.0 * wn);

    let (q, dt) = record(ALT, V, 3.0, 1, |p| p.omega.x += 0.15, |p| p.omega.x);
    let modes = prony(&q[2..], dt, 4);
    let sp = modes.iter().filter(|m| m.omega_d > 1.0 && m.omega_d < 20.0).max_by(|a, b| a.omega_d.total_cmp(&b.omega_d)).copied().unwrap_or_else(|| panic!("no short period in {modes:?}"));
    println!("short period: ωn {:.2} rad/s ζ {:.2} (approximation {wn:.2}, {zeta:.2}); all {modes:?}", sp.wn(), sp.zeta());
    assert!((sp.wn() / wn - 1.0).abs() < 0.25, "{} vs {wn}", sp.wn());
    assert!((sp.zeta() - zeta).abs() < 0.15, "{} vs {zeta}", sp.zeta());
}

#[test]
fn phugoid_is_slow_and_lightly_damped() {
    // Lanchester: ωp ≈ √2·g/u0; ζp ≈ CD/(√2·CL).
    let wn = 2f64.sqrt() * G / V;
    let d = dims();
    let cl = C172.mass * G / d.qs;
    let cd = C172.cd0 + C172.k_ind * cl * cl;
    let zeta = cd / (2f64.sqrt() * cl);

    let (u, dt) = record_with(ALT, V, 150.0, 45, |p| p.vel += p.forward() * 4.0, |p| p.airspeed().length() - V, wings_level);
    let modes = prony(&u, dt, 3);
    let ph = modes.iter().find(|m| m.omega_d > 0.05 && m.omega_d < 1.0).copied().unwrap_or_else(|| panic!("no phugoid in {modes:?}"));
    println!("phugoid: period {:.1} s ζ {:.3} (Lanchester {:.1} s, {zeta:.3}); all {modes:?}", ph.period(), ph.zeta(), std::f64::consts::TAU / wn);
    assert!((ph.wn() / wn - 1.0).abs() < 0.25, "{} vs {wn}", ph.wn());
    // Lightly damped and stable (or very nearly: the propeller's thrust
    // falling with speed damps it, which Lanchester leaves out).
    assert!(ph.zeta() > -0.02 && ph.zeta() < 0.25, "{}", ph.zeta());
}

#[test]
fn dutch_roll_and_roll_subsidence_match_their_derivatives() {
    let ae = C172.aero.as_ref().unwrap();
    let d = dims();
    let b = C172.b;
    let (ixx, izz) = (C172.inertia.z, C172.inertia.y);
    // Nelson's Dutch-roll approximation: ωn² = (Yβ·Nr − Nβ·Yr + u0·Nβ)/u0,
    // 2ζωn = −(Yβ + u0·Nr)/u0; roll subsidence τ = −1/Lp.
    let kn = d.qs * b / izz;
    let n_b = ae.yaw_b * kn;
    let n_r = ae.yaw_r * b / (2.0 * V) * kn;
    let y_b = ae.side_b * d.qs / d.m;
    let y_r = ae.side_r * b / (2.0 * V) * d.qs / d.m;
    let wn = ((y_b * n_r - n_b * y_r + V * n_b) / V).sqrt();
    let zeta = -(y_b + V * n_r) / (V * 2.0 * wn);
    let l_p = ae.roll_p * b / (2.0 * V) * d.qs * b / ixx;
    let tau = -1.0 / l_p;

    let (r, dt) = record(ALT, V, 8.0, 2, |p| p.omega.y += 0.12, |p| -p.omega.y);
    let modes = prony(&r, dt, 4);
    let dr = modes.iter().find(|m| m.omega_d > 1.0 && m.omega_d < 10.0).copied().unwrap_or_else(|| panic!("no Dutch roll in {modes:?}"));
    println!("Dutch roll: ωn {:.2} rad/s (period {:.1} s) ζ {:.2} (approximation {wn:.2}, {zeta:.2}); all {modes:?}", dr.wn(), dr.period(), dr.zeta());
    assert!((dr.wn() / wn - 1.0).abs() < 0.25, "{} vs {wn}", dr.wn());
    assert!((dr.zeta() - zeta).abs() < 0.12, "{} vs {zeta}", dr.zeta());

    let (p, dt) = record(ALT, V, 1.0, 1, |p| p.omega.z -= 0.3, |p| -p.omega.z);
    let modes = prony(&p[1..], dt, 3);
    let roll = modes.iter().filter(|m| m.omega_d < 0.5).min_by(|a, b| a.sigma.total_cmp(&b.sigma)).copied().unwrap_or_else(|| panic!("no roll mode in {modes:?}"));
    println!("roll subsidence: τ {:.3} s (approximation {tau:.3}); all {modes:?}", -1.0 / roll.sigma);
    assert!((-1.0 / roll.sigma / tau - 1.0).abs() < 0.3, "{} vs {tau}", -1.0 / roll.sigma);
}

#[test]
fn hands_off_cruise_stays_put() {
    // Trimmed and left alone for a minute: no divergence, the wings stay
    // nearly level (the spiral mode is slow), the height within a phugoid's swing.
    let w = empty_world();
    let dt = 1.0 / 180.0;
    let mut p = Plane::spawn(C172, 0.6, -2.1, ALT, 0.0, 55.0);
    p.auto_rudder = false;
    for _ in 0..(60.0 / dt) as usize {
        assert!(p.step(dt, &Controls::default(), &w).is_none());
    }
    let (_, _, h) = geo::pos_to_lla(p.pos);
    let bank = p.right().dot(p.pos.normalize()).asin().to_degrees();
    println!("after a minute hands off: {:.0} m, bank {bank:.1}°, {:.1} m/s", h - ALT, p.airspeed().length());
    assert!((h - ALT).abs() < 60.0 && bank.abs() < 20.0 && (p.airspeed().length() - 55.0).abs() < 5.0);
}

#[test]
fn the_auto_rudder_damps_the_dutch_roll_and_centres_the_ball() {
    let fit = |aid: bool| {
        let (r, dt) = record(ALT, V, 8.0, 2, |p| { p.auto_rudder = aid; p.omega.y += 0.12 }, |p| -p.omega.y);
        prony(&r, dt, 4).into_iter().find(|m| m.omega_d > 1.0 && m.omega_d < 10.0)
    };
    let bare = fit(false).unwrap();
    // With the aid the oscillation may be damped past being one at all.
    let aided = fit(true).map_or(1.0, |m| m.zeta());
    println!("Dutch roll ζ: bare {:.2}, with the auto-rudder {aided:.2}", bare.zeta());
    assert!(aided > bare.zeta() + 0.1);
}

#[test]
fn trimmed_angle_moves_with_the_cg_and_the_flaps() {
    // The same tab setting, CG forward then aft: aft the lift's arm is
    // shorter, so it settles at a higher angle of attack (slower).
    let ae = C172.aero.as_ref().unwrap();
    let t = moments::trim_for(C172, ae, 0.05, 0.0, C172.cg[0]);
    let alpha_at = |cg: f64, flaps: f64| {
        // Cm(α) = 0 with that tab setting, by bisection.
        let de = moments::trim_elevator(C172, ae, t);
        let cm = |a: f64| ae.pitch_0 + ae.pitch_a * f64::sin(a) + ae.pitch_flap * flaps + (C172.cl0 + C172.flap_cl * flaps + C172.cl_a * a) * (cg - ae.cg_ref) + ae.pitch_de * de;
        let (mut lo, mut hi) = (-0.2, 0.4);
        for _ in 0..60 {
            let mid = 0.5 * (lo + hi);
            if cm(mid) > 0.0 { lo = mid } else { hi = mid }
        }
        lo
    };
    let (fwd, aft) = (alpha_at(C172.cg[1], 0.0), alpha_at(C172.cg[2], 0.0));
    println!("trimmed α: forward CG {:.1}°, aft {:.1}°", fwd.to_degrees(), aft.to_degrees());
    assert!(aft > fwd + 0.005);
    // The neutral point (where Cmα vanishes) lies aft of the aft limit: stable throughout.
    let np = ae.cg_ref - ae.pitch_a / C172.cl_a;
    println!("neutral point {:.0}% MAC, aft limit {:.0}%", np * 100.0, C172.cg[2] * 100.0);
    assert!(np > C172.cg[2] + 0.03);
}

#[test]
fn lands_on_its_mains_with_a_flare() {
    // Full flaps at 65 kt down a 3° glide, a flare from 8 m to hold the sink
    // off, power to idle: it should touch on the mains, gently, then lower
    // the nose wheel, with nothing broken.
    let mut w = empty_world();
    w.flat = Some(0.0);
    let dt = 1.0 / 180.0;
    let v = 65.0 / 1.943_84;
    let mut p = Plane::spawn(C172, 0.6, -2.1, 120.0, 0.0, v);
    p.configure(3, true, v * 3f64.to_radians().sin());
    let thr = p.throttle;
    let (mut mains_at, mut nose_at, mut vs_td) = (None, None, 0.0);
    for k in 0..(120.0 / dt) as usize {
        let h = geo::pos_to_lla(p.pos).2 - p.wheel_height();
        let vs = p.vel.dot(p.pos.normalize());
        let flare = h < 8.0;
        let vs_want = if flare { (-0.25 * h).max(-v * 3f64.to_radians().sin()).min(-0.3) } else { -v * 3f64.to_radians().sin() };
        let roll = (p.right().dot(p.pos.normalize()) * 3.0 + p.omega.z * 0.5).clamp(-1.0, 1.0);
        let c = Controls { pitch: ((vs_want - vs) * 0.12 - p.omega.x * 0.4 + if flare { 0.05 } else { 0.0 }).clamp(-1.0, 1.0), roll, throttle_set: Some(if flare { 0.0 } else { thr }), ..Default::default() };
        assert!(p.step(dt, &c, &w).is_none(), "crashed");
        let t = k as f64 * dt;
        if mains_at.is_none() && p.compress[1] > 0.0 && p.compress[2] > 0.0 {
            mains_at = Some(t);
            vs_td = vs;
        }
        if nose_at.is_none() && p.compress[0] > 0.0 {
            nose_at = Some(t);
        }
        if mains_at.is_some_and(|m| t > m + 10.0) {
            break;
        }
    }
    let (m, n) = (mains_at.expect("never touched down"), nose_at.expect("the nose wheel never came down"));
    println!("touchdown: mains at {m:.1} s, {:.0} fpm; nose wheel {:.1} s later; {:.0} kt; notes {:?}", vs_td * 196.85, n - m, p.vel.length() * 1.943_84, p.notes);
    assert!(n > m, "nose wheel first");
    assert!(vs_td > -2.0, "{vs_td}");
    assert!(p.notes.is_empty() && p.dmg.gear.iter().all(|&g| g), "{:?}", p.notes);
}
