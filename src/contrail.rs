//! contrails: ice trails behind engines high up in cold air.
//! formation follows a simple schmidt appleman threshold, about -40 c for a
//! modern turbofan, a little warmer in moist air. they persist and spread
//! in ice supersaturated air and fade in seconds in dry air.
//! the upper humidity is a game model, the weather gives none.
//! refs: schumann 1996, meteorol. z. 5, 4-23 (threshold temperature);
//! paoli and shariff 2016, annu. rev. fluid mech. 48 (vortex phase, sinking 100 to 300 m).

use glam::DVec3;
use std::collections::VecDeque;

/// segments sent to the gpu at most, render.rs buffer size.
pub const MAX_SEGS: usize = 24_576;

/// one segment for the gpu: a xyz width, b xyz width, alphas and along coords.
pub type Seg = [f32; 12];

/// what makes a trail: an aircraft and its engine exits in world space.
pub struct Source {
    /// stable id per aircraft.
    pub id: u64,
    pub vel: DVec3,
    /// unit right axis of the aircraft.
    pub right: DVec3,
    /// exits in world space, the lateral offset of each from the centreline (m).
    pub exits: Vec<(DVec3, f64)>,
    pub span: f64,
    /// 0..1, how much water the exhaust carries, idle less.
    pub power: f64,
    /// the air it flies in.
    pub air: Air,
}

#[derive(Clone, Copy)]
struct Node {
    /// where it was left, world.
    pos: DVec3,
    side: DVec3,
    /// lateral offset at the exit and the vortex it is drawn into (m).
    y0: f32,
    yv: f32,
    born: f64,
    life: f32,
    persist: bool,
    /// 0..1 strength.
    on: f32,
    /// metres flown since the trail began.
    along: f32,
}

struct Trail {
    id: u64,
    span: f32,
    nodes: VecDeque<Node>,
    along: f32,
    last: Option<DVec3>,
    seen: f64,
}

/// the air the trail forms in.
#[derive(Clone, Copy, Debug)]
pub struct Air {
    pub temp_c: f64,
    /// relative humidity over water, 0..1.
    pub rh: f64,
    /// relative humidity over ice, persistent when above 1.
    pub rhi: f64,
}

/// isa temperature at an altitude (m), with a surface offset (c).
pub fn isa_temp(alt: f64, offset: f64) -> f64 {
    15.0 - 6.5 * alt.min(11_000.0) / 1000.0 + offset
}

/// saturation vapour pressure over water and ice (hpa), magnus forms.
fn e_water(t: f64) -> f64 {
    6.112 * (17.62 * t / (243.12 + t)).exp()
}
fn e_ice(t: f64) -> f64 {
    6.112 * (22.46 * t / (272.62 + t)).exp()
}

/// rhi from rh at a temperature.
pub fn rhi_of(rh: f64, t: f64) -> f64 {
    rh * e_water(t) / e_ice(t)
}

/// threshold temperature (c) below which a trail forms, from schumann 1996
/// fitted for a propulsion efficiency near 0.3 at 250 hpa: about -40 c dry,
/// a few degrees warmer saturated.
pub fn threshold(rh: f64) -> f64 {
    -40.0 + 6.0 * rh.clamp(0.0, 1.0).powf(0.7)
}

/// 0..1, whether a trail forms in this air.
pub fn forms(a: &Air) -> f64 {
    let t = threshold(a.rh);
    ((t + 1.5 - a.temp_c) / 3.0).clamp(0.0, 1.0)
}

/// how long a trail lasts in this air (s), and whether it persists.
pub fn lifetime(a: &Air) -> (f32, bool) {
    if a.rhi >= 1.0 {
        // spreading cirrus, bounded so the sky does not fill up.
        (540.0 + 300.0 * ((a.rhi - 1.0) * 5.0).min(1.0) as f32, true)
    } else {
        // dry air: gone in a few seconds, a minute near saturation.
        (1.2 + 50.0 * (a.rhi.max(0.0) as f32).powi(6), false)
    }
}

/// width of a trail at an age (m): a few metres per engine at first, some
/// 100 m after a minute or two, hundreds more as persistent cirrus spreads.
fn width(age: f32, persist: bool, span: f32) -> f32 {
    let w0 = 3.0 + span * 0.04;
    let young = 1.1 * age.powf(0.8);
    if persist {
        w0 + young + 0.9 * (age - 60.0).max(0.0).powf(0.9)
    } else {
        w0 + young
    }
}

/// opacity of a node at an age.
fn opacity(n: &Node, age: f32) -> f32 {
    // the plume needs a wingspan or so to freeze.
    let onset = (age / 0.35).clamp(0.0, 1.0);
    let fade = if n.persist {
        let k = age / n.life;
        (1.0 - (k - 0.6).max(0.0) / 0.4) * (0.62 / (1.0 + age / 240.0)).max(0.16)
    } else {
        (-age / (n.life * 0.45)).exp() * 0.62
    };
    (onset * onset * fade * n.on).clamp(0.0, 1.0)
}

#[derive(Default)]
pub struct Contrails {
    trails: Vec<Trail>,
}

impl Contrails {
    pub fn clear(&mut self) {
        self.trails.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.trails.iter().all(|t| t.nodes.is_empty())
    }

    /// adds nodes behind each source and drops old ones.
    pub fn update(&mut self, now: f64, sources: &[Source]) {
        for s in sources {
            let f = forms(&s.air) * (0.45 + 0.55 * s.power.clamp(0.0, 1.0));
            let (life, persist) = lifetime(&s.air);
            let speed = s.vel.length();
            // closer together when slow, never more than 40 m apart.
            let gap = (speed * 0.15).clamp(12.0, 40.0);
            for (k, (exit, y0)) in s.exits.iter().enumerate() {
                let id = s.id.wrapping_mul(16) + k as u64;
                let ti = match self.trails.iter().position(|t| t.id == id) {
                    Some(i) => i,
                    None => {
                        self.trails.push(Trail { id, span: s.span as f32, nodes: VecDeque::new(), along: 0.0, last: None, seen: now });
                        self.trails.len() - 1
                    }
                };
                let t = &mut self.trails[ti];
                t.seen = now;
                let moved = t.last.map(|l| (*exit - l).length());
                // a jump (teleport, new flight) starts the trail over.
                if moved.is_some_and(|m| m > 2000.0) {
                    t.last = None;
                }
                let due = t.last.is_none_or(|l| (*exit - l).length() >= gap);
                if !due {
                    continue;
                }
                if f <= 0.01 {
                    // no trail here: a gap, a node at zero ends the ribbon softly.
                    if t.nodes.back().is_some_and(|n| n.on > 0.0) {
                        t.along += moved.unwrap_or(0.0) as f32;
                        t.nodes.push_back(Node { pos: *exit, side: s.right, y0: *y0 as f32, yv: vortex(*y0, s.span), born: now, life, persist, on: 0.0, along: t.along });
                    }
                    t.last = Some(*exit);
                    continue;
                }
                t.along += moved.unwrap_or(0.0) as f32;
                t.nodes.push_back(Node { pos: *exit, side: s.right, y0: *y0 as f32, yv: vortex(*y0, s.span), born: now, life, persist, on: f as f32, along: t.along });
                t.last = Some(*exit);
            }
        }
        for t in &mut self.trails {
            while t.nodes.front().is_some_and(|n| (now - n.born) as f32 > n.life) {
                t.nodes.pop_front();
            }
            thin(&mut t.nodes, now);
        }
        // trails of aircraft no longer around go once empty.
        self.trails.retain(|t| !t.nodes.is_empty() || now - t.seen < 5.0);
    }

    /// segments relative to the camera, nearest trails kept when there are too many.
    pub fn segments(&self, now: f64, cam: DVec3, wind: DVec3, out: &mut Vec<Seg>) {
        let up = cam.normalize();
        for t in &self.trails {
            let span = t.span;
            let mut prev: Option<(DVec3, f32, f32, f32)> = None;
            for n in &t.nodes {
                let age = (now - n.born).max(0.0) as f32;
                let p = place(n, age, wind, up);
                let rel = p - cam;
                let w = width(age, n.persist, span);
                let a = opacity(n, age);
                if let Some((q, wq, aq, lq)) = prev {
                    if (a > 0.003 || aq > 0.003) && out.len() < MAX_SEGS {
                        out.push([q.x as f32, q.y as f32, q.z as f32, wq, rel.x as f32, rel.y as f32, rel.z as f32, w, aq, a, lq, n.along]);
                    }
                }
                prev = Some((rel, w, a, n.along));
            }
        }
    }
}

/// lateral position of the wingtip vortex a plume rolls into, b0 = pi/4 b.
fn vortex(y0: f64, span: f64) -> f32 {
    let half = std::f64::consts::FRAC_PI_4 * span * 0.5;
    if y0.abs() < 0.5 { 0.0 } else { (y0.signum() * half) as f32 }
}

/// a node where it is now: rolled into its vortex, sunk and carried by the wind.
fn place(n: &Node, age: f32, wind: DVec3, up: DVec3) -> DVec3 {
    let roll = 1.0 - (-age / 6.0).exp();
    let y = n.y0 + (n.yv - n.y0) * roll;
    // the vortex pair sinks about 2 m/s for a minute or two, then stops.
    let sink = 2.0 * 90.0 * (1.0 - (-age / 90.0).exp());
    n.pos + n.side * (y - n.y0) as f64 - up * sink as f64 + wind * age as f64
}

/// fewer nodes the older they are, spacing grows with age.
fn thin(nodes: &mut VecDeque<Node>, now: f64) {
    if nodes.len() < 8 {
        return;
    }
    let mut i = 1;
    while i + 1 < nodes.len() {
        let age = (now - nodes[i].born) as f32;
        if age < 8.0 {
            break;
        }
        let want = 35.0 * (1.0 + age / 12.0) as f64;
        let d = (nodes[i + 1].pos - nodes[i - 1].pos).length();
        // keep ends of a gap.
        let keep = nodes[i].on == 0.0 || nodes[i - 1].on == 0.0 || nodes[i + 1].on == 0.0;
        if d < want && !keep {
            nodes.remove(i);
        } else {
            i += 1;
        }
    }
}

/// a game model of upper humidity over ice: moister with mid level cloud,
/// in patches some 150 km across and a few hundred metres deep, the
/// ice supersaturated regions persistent trails need.
pub fn upper_rhi(lat: f64, lon: f64, alt: f64, temp_c: f64, clouds: f64, alto: f64, day: f64) -> Air {
    let h = |x: f64| (x.sin() * 43758.5453).fract().abs();
    let n2 = |x: f64, y: f64| {
        let (ix, iy) = (x.floor(), y.floor());
        let (fx, fy) = (x - ix, y - iy);
        let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
        let c = |a: f64, b: f64| h(a * 127.1 + b * 311.7);
        let top = c(ix, iy) + (c(ix + 1.0, iy) - c(ix, iy)) * sx;
        let bot = c(ix, iy + 1.0) + (c(ix + 1.0, iy + 1.0) - c(ix, iy + 1.0)) * sx;
        top + (bot - top) * sy
    };
    let km_lat = lat * 111.0;
    let km_lon = lon * 111.0 * lat.to_radians().cos();
    let patch = n2(km_lon / 150.0 + day * 3.1, km_lat / 150.0 + day * 1.7);
    let layer = n2(alt / 700.0 + day * 5.3, km_lon / 400.0);
    let rh = (0.25 + 0.3 * alto + 0.15 * clouds + 0.45 * patch * layer.powf(0.5)).clamp(0.05, 1.0);
    Air { temp_c, rh, rhi: rhi_of(rh, temp_c) }
}

/// engine exits in the body frame (+x right, +y up, -z forward), metres from the cg.
/// estimates from published three views, the contrail only needs them roughly:
/// b747: inboard 11.8 m, outboard 21.2 m off the centreline (747-400 airport planning doc d6-58326-1);
/// concorde: pairs at about 3.3 and 4.6 m; f-22, f-4, f/a-18: twin nozzles about 0.6 to 1 m apart;
/// g650: rear fuselage nacelles about 2.5 m out; sr-71: nacelles at about 5 m.
pub fn exits(spec: &crate::aircraft::Spec) -> Vec<[f64; 3]> {
    use crate::aircraft::Engine;
    let key = spec.keys.first().copied().unwrap_or("");
    let span = spec.span as f64;
    let nozzle = match &spec.engine {
        Engine::Fan(f) if f.nozzle_z != 0.0 => f.nozzle_z,
        _ => span * 0.3,
    };
    let pairs = |xs: &[f64], y: f64, z: f64| xs.iter().flat_map(|x| [[-x, y, z], [*x, y, z]]).collect::<Vec<_>>();
    match key {
        "b747" => pairs(&[11.8, 21.2], -1.6, 3.0),
        "concorde" => pairs(&[3.3, 4.6], -1.0, nozzle),
        "f22" => pairs(&[0.95], 0.0, nozzle),
        "f4" => pairs(&[0.55], -0.3, nozzle),
        "fa18" => pairs(&[0.55], -0.1, nozzle),
        "g650" => pairs(&[2.5], 0.9, 9.0),
        "sr71" => pairs(&[5.0], 0.0, nozzle),
        _ => match &spec.engine {
            Engine::Fan(f) if f.n >= 4 => pairs(&[span * 0.19, span * 0.33], -1.2, span * 0.05),
            Engine::Fan(f) if f.n >= 2 => pairs(&[span * 0.16], -1.0, span * 0.05),
            Engine::Fan(_) => vec![[0.0, 0.0, nozzle]],
            // a piston's exhaust leaves under the nose.
            _ => vec![[0.0, -0.5, -span * 0.15]],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forms_at_cruise_not_low() {
        // fl350 on a standard day is about -54 c: trails form even in dry air.
        let t = isa_temp(10_670.0, 0.0);
        assert!((t + 54.3).abs() < 0.5, "{t}");
        assert!(forms(&Air { temp_c: t, rh: 0.2, rhi: rhi_of(0.2, t) }) > 0.99);
        // fl200 is about -25 c: none.
        let t = isa_temp(6100.0, 0.0);
        assert_eq!(forms(&Air { temp_c: t, rh: 0.9, rhi: rhi_of(0.9, t) }), 0.0);
        // ice saturation comes before water saturation in cold air.
        assert!(rhi_of(0.7, -50.0) > 1.0);
        assert!(rhi_of(0.5, -50.0) < 1.0);
    }

    #[test]
    fn persistent_in_moist_air_short_in_dry() {
        let (l, p) = lifetime(&Air { temp_c: -55.0, rh: 0.8, rhi: rhi_of(0.8, -55.0) });
        assert!(p && l > 500.0);
        let (l, p) = lifetime(&Air { temp_c: -55.0, rh: 0.2, rhi: rhi_of(0.2, -55.0) });
        assert!(!p && l < 3.0, "{l}");
    }

    #[test]
    fn trails_grow_drift_and_thin() {
        let mut c = Contrails::default();
        let r = 6_371_000.0 + 11_000.0;
        let wet = Air { temp_c: -56.0, rh: 0.85, rhi: rhi_of(0.85, -56.0) };
        let v = DVec3::new(0.0, 0.0, -240.0);
        let mut now = 0.0;
        for _ in 0..6000 {
            let pos = DVec3::new(0.0, r, 0.0) + v * now;
            let s = Source { id: 1, vel: v, right: DVec3::X, exits: vec![(pos - DVec3::X * 10.0, -10.0), (pos + DVec3::X * 10.0, 10.0)], span: 60.0, power: 0.8, air: wet };
            c.update(now, &[s]);
            now += 0.05;
        }
        // 300 s at 240 m/s is 72 km of trail on each side, thinned to a few hundred nodes.
        let n: usize = c.trails.iter().map(|t| t.nodes.len()).sum();
        assert!(n > 100 && n < 1600, "{n}");
        let mut segs = Vec::new();
        let wind = DVec3::X * 20.0;
        c.segments(now, DVec3::new(0.0, r, 0.0), wind, &mut segs);
        assert!(!segs.is_empty());
        // the oldest segment has drifted east with the wind and spread wide.
        let old = segs.iter().max_by(|a, b| a[3].total_cmp(&b[3])).unwrap();
        assert!(old[3] > 150.0, "{}", old[3]);
        assert!(old[0] > 3000.0, "{}", old[0]);
    }

    #[test]
    fn dry_trails_vanish() {
        let mut c = Contrails::default();
        let dry = Air { temp_c: -56.0, rh: 0.1, rhi: rhi_of(0.1, -56.0) };
        let v = DVec3::new(0.0, 0.0, -240.0);
        let mut now = 0.0;
        while now < 20.0 {
            let pos = DVec3::new(0.0, 6_382_000.0, 0.0) + v * now;
            c.update(now, &[Source { id: 2, vel: v, right: DVec3::X, exits: vec![(pos, 0.0)], span: 30.0, power: 1.0, air: dry }]);
            now += 0.05;
        }
        let n: usize = c.trails.iter().map(|t| t.nodes.len()).sum();
        // only the last couple of seconds remain.
        assert!(n < 40, "{n}");
    }
}
