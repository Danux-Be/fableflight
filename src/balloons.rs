//! Hot-air balloons: the morning (and, here and there, evening) flights of
//! the places known for them, drifting with the day's wind.
//!
//! The models come from Blender (assets/blender/balloons.py,
//! assets/balloons.bin.gz): an envelope in eight fictional colour schemes,
//! near and far, its skirt and flying wires, two rigs (a sport balloon after
//! the Cameron O-105, a passenger balloon after the Ultramagic N-425, whose
//! envelope is the same one scaled by `LARGE_SCALE`), and the burner flame.
//! They are drawn as instanced solids (inst.wgsl): the fabric lets the sun
//! through a little, the flame glows.
//!
//! Each site has a schedule for the day, made from the site and the date (so
//! it is the same every time the day is flown): how many balloons, when each
//! takes off (from about a quarter of an hour before sunrise to three
//! quarters of an hour after, and for some sites in the two hours before
//! sunset), from which launch field, how high it cruises and for how long.
//! A balloon only takes off in calm air (under about 5 m/s at the ground)
//! without rain or storms; once up, it goes where the wind at its height
//! takes it, climbs and descends with the burner, and lands about an hour
//! later. Arriving in the middle of the flights, the balloons already up are
//! flown forward to where they would be now.
//!
//! Sites and their figures (balloons on a good morning in season; the
//! festivals' dates and numbers are typical ones, approximate):
//! - Göreme, Cappadocia: the authorities allow up to about 150 balloons a
//!   morning, nearly all large passenger balloons, all year;
//! - Albuquerque: the International Balloon Fiesta, nine days from the first
//!   Saturday of October, some 500 balloons (more than the game draws); a few
//!   commercial flights the rest of the year;
//! - Château-d'Œx: the Festival International de Ballons, late January;
//! - Chambley-Bussières: the Lorraine Mondial Air Ballons, late July in odd
//!   years, the largest gathering in Europe; flights through the summer;
//! - Bristol: the International Balloon Fiesta, August;
//! - Canberra: the Balloon Spectacular, March;
//! - Luxor (West Bank), Bagan (October to March), Teotihuacan, the Masai
//!   Mara and the Napa Valley: daily passenger flights;
//! - the Arve valley under the Mont Blanc and Flanders (evening flights,
//!   May to September): a handful of balloons.
//! Counts and seasons are estimates from operators' and festivals' published
//! figures and press reports, not data.

use crate::geo;
use crate::graphics::Level;
use crate::render::{Inst, MeshVertex};
use glam::{DVec3, Mat4};
use std::sync::OnceLock;

// ------------------------------------------------------------------- models

/// The colour schemes, as named in balloons.py.
pub const SCHEMES: [&str; 8] = ["rainbow", "harlequin", "bands", "chevron", "blocks", "wave", "checks", "stripes"];
/// The pieces after the envelopes (near, then far), in mesh-id order.
const PARTS: [&str; 7] = ["skirt", "wires sport", "wires large", "rig sport", "rig large", "rig far", "flame"];
/// First mesh id of the balloons in the instanced-solid library (main.rs `solids`).
pub const BASE: u32 = crate::birds::BASE + (crate::birds::Kind::ALL.len() * crate::birds::PIECES.len()) as u32;
const ENV: u32 = BASE;
const ENV_FAR: u32 = BASE + SCHEMES.len() as u32;
const SKIRT: u32 = BASE + 2 * SCHEMES.len() as u32;
const WIRES_SPORT: u32 = SKIRT + 1;
const WIRES_LARGE: u32 = SKIRT + 2;
const RIG_SPORT: u32 = SKIRT + 3;
const RIG_LARGE: u32 = SKIRT + 4;
const RIG_FAR: u32 = SKIRT + 5;
const FLAME: u32 = SKIRT + 6;

/// The passenger balloon's envelope is the sport one this much bigger.
pub const LARGE_SCALE: f64 = 1.6;
/// The envelope, mouth to crown, and its radius at its widest (balloons.py: H, R).
pub const ENV_H: f64 = 19.5;
pub const ENV_R: f64 = 9.3;
/// Above the basket floor: the mouth and the burner nozzle (balloons.py).
const MOUTH: [f64; 2] = [4.55, 6.2];
const NOZZLE: [f64; 2] = [2.82, 3.35];

pub struct Models {
    pub meshes: Vec<(Vec<MeshVertex>, Vec<u32>)>,
}

pub fn models() -> &'static Models {
    static M: OnceLock<Models> = OnceLock::new();
    M.get_or_init(|| {
        let objs = crate::aircraft::fa18::load(include_bytes!("../assets/balloons.bin.gz"), "balloons");
        let names: Vec<String> = SCHEMES.iter().map(|s| format!("env {s}")).chain(SCHEMES.iter().map(|s| format!("env {s} far"))).chain(PARTS.iter().map(|s| s.to_string())).collect();
        let meshes = names
            .iter()
            .map(|name| {
                let o = objs.get(name).unwrap_or_else(|| panic!("balloons model: no piece \"{name}\""));
                // (Colour alpha, inst.wgsl's material: 1.5 fabric that lets
                // the sun through, 2 the flame, 1 plain diffuse.)
                let m = if name.starts_with("env") || name == "skirt" {
                    1.5
                } else if name == "flame" {
                    2.0
                } else {
                    1.0
                };
                let v = o.mesh.v.iter().map(|v| MeshVertex { pos: v.pos, nrm: v.nrm, col: [v.col[0], v.col[1], v.col[2], m] }).collect();
                (v, o.mesh.i.clone())
            })
            .collect();
        Models { meshes }
    })
}

/// How many balloons at most, by setting.
pub fn cap(level: Level) -> usize {
    match level {
        Level::Off => 0,
        Level::Low => 25,
        Level::Medium => 80,
        Level::High | Level::Ultra => 220,
    }
}

// -------------------------------------------------------------------- sites

pub struct Site {
    pub name: &'static str,
    pub lat: f64,
    pub lon: f64,
    /// ground height (m), for when the terrain is not loaded
    pub elev: f64,
    /// launch fields lie within this (km)
    pub spread_km: f64,
    /// balloons on a good morning in season
    pub daily: (u32, u32),
    /// share of large passenger balloons
    pub large: f64,
    /// months flown (bit 0 January)
    pub months: u16,
    /// also flights in the evening (two hours before sunset)
    pub evening: bool,
    /// only in the evening
    pub evening_only: bool,
    /// festivals: first and last day of the year, balloons a morning, odd years only
    pub festivals: &'static [(u32, u32, u32, bool)],
}

const ALL_YEAR: u16 = 0xFFF;
const fn months(a: u32, b: u32) -> u16 {
    // a..=b, 1-based, wrapping over the new year.
    let mut m = 0u16;
    let mut i = a;
    loop {
        m |= 1 << (i - 1);
        if i == b {
            break;
        }
        i = if i == 12 { 1 } else { i + 1 };
    }
    m
}

pub static SITES: &[Site] = &[
    Site { name: "Cappadocia", lat: 38.6431, lon: 34.8289, elev: 1100.0, spread_km: 6.0, daily: (70, 150), large: 0.95, months: ALL_YEAR, evening: false, evening_only: false, festivals: &[] },
    Site { name: "Albuquerque", lat: 35.1966, lon: -106.5975, elev: 1550.0, spread_km: 3.0, daily: (2, 7), large: 0.25, months: ALL_YEAR, evening: false, evening_only: false, festivals: &[(277, 285, 500, false)] },
    Site { name: "Château-d'Œx", lat: 46.4747, lon: 7.1358, elev: 960.0, spread_km: 2.0, daily: (1, 3), large: 0.05, months: months(12, 3), evening: false, evening_only: false, festivals: &[(24, 32, 70, false)] },
    Site { name: "Chambley", lat: 49.0306, lon: 5.8786, elev: 260.0, spread_km: 3.0, daily: (2, 6), large: 0.1, months: months(4, 10), evening: true, evening_only: false, festivals: &[(206, 215, 350, true)] },
    Site { name: "Bristol", lat: 51.4436, lon: -2.6435, elev: 70.0, spread_km: 2.0, daily: (1, 3), large: 0.15, months: months(4, 9), evening: true, evening_only: false, festivals: &[(220, 223, 100, false)] },
    Site { name: "Canberra", lat: -35.2984, lon: 149.1311, elev: 580.0, spread_km: 3.0, daily: (1, 4), large: 0.3, months: ALL_YEAR, evening: false, evening_only: false, festivals: &[(68, 76, 40, false)] },
    Site { name: "Luxor", lat: 25.7200, lon: 32.6020, elev: 80.0, spread_km: 3.0, daily: (15, 40), large: 0.9, months: ALL_YEAR, evening: false, evening_only: false, festivals: &[] },
    Site { name: "Bagan", lat: 21.1717, lon: 94.8585, elev: 60.0, spread_km: 4.0, daily: (10, 25), large: 0.9, months: months(10, 3), evening: false, evening_only: false, festivals: &[] },
    Site { name: "Teotihuacan", lat: 19.6900, lon: -98.8600, elev: 2300.0, spread_km: 4.0, daily: (15, 35), large: 0.7, months: ALL_YEAR, evening: false, evening_only: false, festivals: &[] },
    Site { name: "Masai Mara", lat: -1.4900, lon: 35.1400, elev: 1550.0, spread_km: 8.0, daily: (6, 14), large: 0.8, months: ALL_YEAR, evening: false, evening_only: false, festivals: &[] },
    Site { name: "Napa Valley", lat: 38.4000, lon: -122.3600, elev: 30.0, spread_km: 6.0, daily: (3, 9), large: 0.3, months: ALL_YEAR, evening: false, evening_only: false, festivals: &[] },
    Site { name: "Arve valley", lat: 46.0700, lon: 6.4100, elev: 450.0, spread_km: 5.0, daily: (2, 5), large: 0.1, months: months(4, 10), evening: true, evening_only: false, festivals: &[] },
    Site { name: "Flanders", lat: 51.0540, lon: 3.7250, elev: 10.0, spread_km: 12.0, daily: (2, 7), large: 0.05, months: months(5, 9), evening: true, evening_only: true, festivals: &[] },
];

/// The sun's hour angle at rising (h, local solar time from noon), with the
/// game's sun (main.rs `sun_dir`) and the usual 0.83° for refraction and the
/// sun's size; None in polar day or night.
pub fn half_day(lat_deg: f64, doy: f64) -> Option<f64> {
    let decl = (-23.44f64).to_radians() * (std::f64::consts::TAU * (doy + 10.0) / 365.25).cos();
    let lat = lat_deg.to_radians();
    let c = ((-0.83f64).to_radians().sin() - lat.sin() * decl.sin()) / (lat.cos() * decl.cos());
    (c.abs() <= 1.0).then(|| c.acos().to_degrees() / 15.0)
}

// ------------------------------------------------------------------ flights

#[derive(Clone, Copy, Debug)]
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.next()
    }
}

/// One balloon's day, as planned.
#[derive(Clone, Copy, Debug)]
pub struct Plan {
    /// take-off, local solar hour
    pub launch: f64,
    /// launch point
    pub lat: f64,
    pub lon: f64,
    pub large: bool,
    pub scheme: u8,
    /// cruising heights above the ground (m): the first and the second half
    pub cruise: (f64, f64),
    /// flight time (s)
    pub minutes: f64,
    pub seed: u64,
}

/// The day's flights at a site (deterministic: the same date, the same flights).
pub fn schedule(site: &Site, si: usize, doy: u32, year_odd: bool) -> Vec<Plan> {
    let month = month_of(doy);
    let fest = site.festivals.iter().find(|f| doy >= f.0 && doy <= f.1 && (!f.3 || year_odd)).map(|f| f.2);
    if fest.is_none() && site.months & (1 << (month - 1)) == 0 {
        return Vec::new();
    }
    let mut rng = Rng(0x2545_F491_4F6C_DD1D ^ (si as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (doy as u64 + 1).wrapping_mul(0xBF58_476D_1CE4_E5B9));
    for _ in 0..4 {
        rng.next();
    }
    let Some(half) = half_day(site.lat, doy as f64) else { return Vec::new() };
    let (rise, set) = (12.0 - half, 12.0 + half);
    let n = match fest {
        Some(n) => n,
        None => rng.range(site.daily.0 as f64, site.daily.1 as f64 + 0.999) as u32,
    };
    // Launch fields: a few, the balloons gathered on them.
    let fields: Vec<(f64, f64)> = (0..(1 + n / 12).min(8))
        .map(|_| {
            let (a, r) = (rng.range(0.0, std::f64::consts::TAU), site.spread_km * 1000.0 * rng.next().sqrt());
            let dlat = r * a.cos() / geo::R;
            let dlon = r * a.sin() / (geo::R * site.lat.to_radians().cos().max(0.1));
            (site.lat + dlat.to_degrees(), site.lon + dlon.to_degrees())
        })
        .collect();
    let mut out = Vec::with_capacity(n as usize);
    for i in 0..n {
        let evening = site.evening_only || (site.evening && fest.is_none() && rng.next() < 0.35);
        let launch = if evening { set - rng.range(1.6, 2.4) } else { rise + rng.range(-0.25, 0.75) };
        let f = fields[(i as usize) % fields.len()];
        // (Some tens of metres apart on the field.)
        let (a, r) = (rng.range(0.0, std::f64::consts::TAU), rng.range(0.0, 180.0));
        let lat = f.0 + (r * a.cos() / geo::R).to_degrees();
        let lon = f.1 + (r * a.sin() / (geo::R * f.0.to_radians().cos().max(0.1))).to_degrees();
        let large = rng.next() < site.large;
        let cruise = (rng.range(120.0, 650.0), rng.range(80.0, 900.0));
        out.push(Plan { launch, lat, lon, large, scheme: (rng.next() * SCHEMES.len() as f64) as u8 % SCHEMES.len() as u8, cruise, minutes: rng.range(55.0, 80.0), seed: rng.0 | 1 });
    }
    out
}

fn month_of(doy: u32) -> u32 {
    const ENDS: [u32; 12] = [31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334, 365];
    ENDS.iter().position(|&e| doy < e).map_or(12, |i| i as u32 + 1)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    /// standing on its field, inflated, before take-off
    Standing,
    Flying,
    /// down, burner off, for a while before it is packed away
    Landed,
    /// hit by an aircraft: the envelope torn, falling
    Torn,
}

#[derive(Clone, Debug)]
pub struct Balloon {
    pub plan: Plan,
    pub site: usize,
    /// basket floor, planet frame
    pub p: DVec3,
    /// horizontal drift and climb rate
    pub v: DVec3,
    pub vz: f64,
    pub phase: Phase,
    /// time in the air (s), since landing (s)
    pub flown: f64,
    pub down: f64,
    /// heading of the basket (rad) and how fast it turns (rad/s)
    pub yaw: f64,
    pub yaw_rate: f64,
    /// burner: on, time left in this burn or this pause
    pub burner: bool,
    pub burn_t: f64,
    /// how far the envelope has collapsed (Torn: 0..1)
    pub collapse: f64,
    rng: Rng,
}

/// What the balloons need from the world.
pub struct Env<'a> {
    /// the ground height at a point, if known
    pub ground: &'a dyn Fn(DVec3) -> Option<f64>,
    /// the steady wind at a point, `agl` metres up
    pub wind: &'a dyn Fn(DVec3, f64) -> DVec3,
    /// launches go ahead: calm and dry where the site is
    pub calm: &'a dyn Fn(DVec3) -> bool,
}

impl Balloon {
    fn new(plan: Plan, site: usize, elev: f64) -> Self {
        let p = geo::lla_to_pos(plan.lat.to_radians(), plan.lon.to_radians(), elev);
        let mut rng = Rng(plan.seed);
        let yaw = rng.range(0.0, std::f64::consts::TAU);
        Balloon { plan, site, p, v: DVec3::ZERO, vz: 0.0, phase: Phase::Standing, flown: 0.0, down: 0.0, yaw, yaw_rate: rng.range(-0.02, 0.02), burner: false, burn_t: rng.range(2.0, 20.0), collapse: 0.0, rng }
    }

    fn mouth(&self) -> f64 {
        MOUTH[self.plan.large as usize]
    }

    /// Where the middle of the envelope is (for collisions).
    pub fn envelope(&self) -> (DVec3, f64) {
        let k = if self.plan.large { LARGE_SCALE } else { 1.0 };
        let up = self.p.normalize();
        (self.p + up * (self.mouth() + 0.6 * ENV_H * k), ENV_R * k * 0.95)
    }

    /// One step of `dt` seconds.
    fn step(&mut self, dt: f64, ground_h: f64, env: &Env) {
        let up = self.p.normalize();
        let agl = self.p.length() - geo::R - ground_h;
        // Turning slowly about the vertical, as balloons do.
        if self.rng.next() < dt / 30.0 {
            self.yaw_rate = self.rng.range(-0.025, 0.025);
        }
        self.yaw += self.yaw_rate * dt;
        let (want_vz, can_burn) = match self.phase {
            Phase::Standing => (0.0, true),
            Phase::Landed => {
                self.down += dt;
                (0.0, false)
            }
            Phase::Torn => {
                self.collapse = (self.collapse + dt / 6.0).min(1.0);
                (-12.0, false)
            }
            Phase::Flying => {
                self.flown += dt;
                let t = self.flown / (self.plan.minutes * 60.0);
                let target = if t < 0.5 { self.plan.cruise.0 } else if t < 0.9 { self.plan.cruise.1 } else { -5.0 };
                let max = if self.plan.large { 2.0 } else { 2.8 };
                (((target - agl) / 40.0).clamp(-max, max), true)
            }
        };
        // The envelope answers the burner slowly (about a quarter of a minute).
        let tau = if self.phase == Phase::Torn { 2.0 } else { 14.0 };
        self.vz += (want_vz - self.vz) * (dt / tau).min(1.0);
        // Burner: longer burns while climbing, short ones to hold the height.
        self.burn_t -= dt;
        if self.burn_t <= 0.0 {
            let need = can_burn && (want_vz - self.vz > -0.4 || self.phase == Phase::Standing);
            self.burner = need && !self.burner;
            self.burn_t = if self.burner {
                if want_vz > 0.8 { self.rng.range(4.0, 9.0) } else { self.rng.range(1.5, 4.0) }
            } else if want_vz > 0.8 {
                self.rng.range(3.0, 8.0)
            } else {
                self.rng.range(12.0, 35.0)
            };
        }
        if !can_burn {
            self.burner = false;
        }
        // Drift: the balloon goes with the air at its height (a little behind it).
        if matches!(self.phase, Phase::Flying | Phase::Torn) {
            let w = (env.wind)(self.p, agl.max(2.0));
            let w = w - up * w.dot(up);
            let drag = if agl < 3.0 { 0.0 } else { 1.0 };
            self.v += (w * drag - self.v) * (dt / 20.0).min(1.0);
        } else {
            self.v = DVec3::ZERO;
        }
        self.p += self.v * dt + up * self.vz * dt;
        // The ground.
        let agl = self.p.length() - geo::R - ground_h;
        if agl < 0.0 {
            self.p += up * -agl;
            if self.vz < 0.0 {
                self.vz = 0.0;
            }
            if self.phase == Phase::Flying && self.flown > 60.0 {
                self.phase = Phase::Landed;
                self.v = DVec3::ZERO;
            }
        }
    }
}

pub struct Balloons {
    pub list: Vec<Balloon>,
    /// the day and the local hour at the sites when they were last planned
    day: Option<(u32, f64)>,
    /// sites near enough to fly (index), as last checked
    active: Vec<usize>,
    /// plans already started today, by site and index
    started: Vec<(usize, usize)>,
    pending: Vec<(usize, usize, Plan)>,
    check: f64,
    /// asked for on the command line: balloons up around (km, bearing) now
    forced: Option<(f64, f64, u32)>,
}

impl Default for Balloons {
    fn default() -> Self {
        Balloons { list: Vec::new(), day: None, active: Vec::new(), started: Vec::new(), pending: Vec::new(), check: 0.0, forced: None }
    }
}

/// Sites within this of the camera fly (m).
const NEAR_SITE: f64 = 120_000.0;
/// Balloons are drawn within this (m).
const DRAW: f64 = 45_000.0;
/// The near models within this (m).
const NEAR_MODEL: f64 = 2_500.0;

impl Balloons {
    /// With a group of `n` balloons asked for, flying now `km` away on `bearing`.
    pub fn with_forced(forced: Option<(f64, f64, u32)>) -> Self {
        Balloons { forced, ..Default::default() }
    }

    pub fn count(&self) -> usize {
        self.list.len()
    }

    pub fn nearest(&self, p: DVec3) -> f64 {
        self.list.iter().map(|b| b.p.distance(p)).fold(f64::INFINITY, f64::min)
    }

    /// Advances the flights. `local` gives the local solar hour at a
    /// longitude (degrees), `doy` the day of the year, `year_odd` for the
    /// festivals of odd years.
    #[allow(clippy::too_many_arguments)]
    pub fn step(&mut self, dt: f64, cam: DVec3, level: Level, utc: f64, doy: f64, year_odd: bool, env: &Env) {
        let max = cap(level);
        if max == 0 {
            self.list.clear();
            self.day = None;
            return;
        }
        let day = doy.floor() as u32;
        // A jump in time (the clock set by hand, a new day): plan again.
        let jumped = self.day.is_some_and(|(d, u)| d != day || (utc - u).abs() > 0.2 && (utc - u).abs() < 23.8);
        if self.day.is_none() || jumped {
            self.list.clear();
            self.started.clear();
            self.pending.clear();
            self.active.clear();
            self.check = 0.0;
        }
        self.day = Some((day, utc));
        if let Some((km, brg, n)) = self.forced.take() {
            self.spawn_here(cam, km, brg, n, env);
        }
        // Which sites are near, now and then.
        self.check -= dt;
        if self.check <= 0.0 {
            self.check = 5.0;
            let near: Vec<usize> = SITES.iter().enumerate().filter(|(_, s)| geo::lla_to_pos(s.lat.to_radians(), s.lon.to_radians(), 0.0).distance(cam.normalize() * geo::R) < NEAR_SITE).map(|(i, _)| i).collect();
            for &si in &near {
                if !self.active.contains(&si) {
                    self.activate(si, utc, day, year_odd, env, max);
                }
            }
            // Sites left behind: their balloons go.
            self.active.retain(|si| near.contains(si));
            let active = self.active.clone();
            self.list.retain(|b| b.site == usize::MAX || active.contains(&b.site));
            self.pending.retain(|(si, _, _)| active.contains(si));
        }
        // Take-offs due (and the balloons standing a few minutes before).
        let mut due = Vec::new();
        self.pending.retain(|&(si, i, plan)| {
            let local = local_hour(utc, SITES[si].lon);
            if hours_between(plan.launch - 0.15, local) >= 0.0 {
                due.push((si, i, plan));
                false
            } else {
                true
            }
        });
        for (si, i, plan) in due {
            if self.list.len() >= max {
                break;
            }
            let site = &SITES[si];
            let p0 = geo::lla_to_pos(plan.lat.to_radians(), plan.lon.to_radians(), site.elev);
            if !(env.calm)(p0) {
                continue;
            }
            let g = (env.ground)(p0).unwrap_or(site.elev);
            self.list.push(Balloon::new(plan, si, g));
            self.started.push((si, i));
        }
        // Fly.
        for b in &mut self.list {
            let local = if b.site == usize::MAX { b.plan.launch + 1.0 } else { local_hour(utc, SITES[b.site].lon) };
            if b.phase == Phase::Standing && hours_between(b.plan.launch, local) >= 0.0 {
                b.phase = Phase::Flying;
            }
            let g = (env.ground)(b.p).unwrap_or_else(|| if b.site == usize::MAX { 0.0 } else { SITES[b.site].elev });
            b.step(dt, g, env);
        }
        // Packed away a while after landing.
        self.list.retain(|b| !(b.phase == Phase::Landed && b.down > 600.0) && !(b.phase == Phase::Torn && b.down > 120.0));
        for b in &mut self.list {
            if b.phase == Phase::Torn && b.p.length() - geo::R - (env.ground)(b.p).unwrap_or(0.0) < 1.0 {
                b.down += dt;
            }
        }
    }

    /// A site comes near: its day's flights, those already up flown forward to now.
    fn activate(&mut self, si: usize, utc: f64, day: u32, year_odd: bool, env: &Env, max: usize) {
        self.active.push(si);
        let site = &SITES[si];
        let local = local_hour(utc, site.lon);
        log::info!("balloons: {} is near, {:.2} h local solar time", site.name, local);
        for (i, plan) in schedule(site, si, day, year_odd).into_iter().enumerate() {
            if self.started.contains(&(si, i)) {
                continue;
            }
            let since = hours_between(plan.launch, local) * 3600.0;
            if since < -0.15 * 3600.0 {
                self.pending.push((si, i, plan));
                continue;
            }
            // Over and packed away already?
            if since > plan.minutes * 60.0 + 900.0 || self.list.len() >= max {
                continue;
            }
            let p0 = geo::lla_to_pos(plan.lat.to_radians(), plan.lon.to_radians(), site.elev);
            if !(env.calm)(p0) {
                continue;
            }
            let g0 = (env.ground)(p0).unwrap_or(site.elev);
            let mut b = Balloon::new(plan, si, g0);
            self.started.push((si, i));
            if since > 0.0 {
                b.phase = Phase::Flying;
                let mut t = 0.0;
                while t < since {
                    let h = (since - t).min(2.0);
                    let g = (env.ground)(b.p).unwrap_or(site.elev);
                    b.step(h, g, env);
                    t += h;
                }
            }
            self.list.push(b);
        }
    }

    /// `n` balloons in the air around a point `km` away on `bearing` (the
    /// command line's --balloons-at): for trying them out anywhere.
    fn spawn_here(&mut self, cam: DVec3, km: f64, brg: f64, n: u32, env: &Env) {
        let (e, nn, up) = geo::enu(cam);
        let a = brg.to_radians();
        let c = cam + (e * a.sin() + nn * a.cos()) * km * 1000.0;
        let mut rng = Rng(0x1234_5678_9ABC_DEF1 ^ n as u64);
        for i in 0..n {
            // (A small group close together, a big one spread over a few km.)
            let spread = (150.0 + 120.0 * n as f64).min(2500.0);
            let off = (e * rng.range(-1.0, 1.0) + nn * rng.range(-1.0, 1.0)) * spread;
            let p = c + off;
            let (la, lo, _) = geo::pos_to_lla(p);
            let g = (env.ground)(p).unwrap_or(0.0);
            let plan = Plan { launch: 0.0, lat: la.to_degrees(), lon: lo.to_degrees(), large: rng.next() < 0.4, scheme: (i % SCHEMES.len() as u32) as u8, cruise: (rng.range(150.0, 600.0), rng.range(100.0, 700.0)), minutes: rng.range(55.0, 80.0), seed: rng.0 | 1 };
            let mut b = Balloon::new(plan, usize::MAX, g);
            b.phase = Phase::Flying;
            b.flown = 300.0;
            b.p += up * rng.range(80.0, 500.0);
            b.p = b.p.normalize() * (geo::R + g + (b.p.length() - geo::R - g));
            self.list.push(b);
        }
    }

    /// An aircraft at `a` (moving to `b`, radius `r`) through an envelope: that
    /// balloon is torn and falls. Returns where it was hit.
    pub fn hit(&mut self, a: DVec3, b: DVec3, r: f64) -> Option<DVec3> {
        for bl in &mut self.list {
            if bl.phase == Phase::Torn {
                continue;
            }
            let (c, rad) = bl.envelope();
            let ab = b - a;
            let t = if ab.length_squared() > 1e-9 { ((c - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
            let q = a + ab * t;
            if q.distance(c) < rad + r {
                bl.phase = Phase::Torn;
                bl.burner = false;
                bl.down = 0.0;
                return Some(q);
            }
        }
        None
    }

    /// The instanced solids for the balloons in view of the camera at `cam`.
    pub fn instances(&self, cam: DVec3, time: f64, visible: &dyn Fn(DVec3, f64) -> bool, out: &mut Vec<(u32, Inst)>) {
        for (k, b) in self.list.iter().enumerate() {
            let rel = b.p - cam;
            let d = rel.length();
            let big = b.plan.large as usize;
            let ks = if b.plan.large { LARGE_SCALE } else { 1.0 };
            if d > DRAW || !visible(rel + b.p.normalize() * (MOUTH[big] + 0.5 * ENV_H * ks), ENV_H * ks) {
                continue;
            }
            let up = b.p.normalize();
            let (e, n, _) = geo::enu(b.p);
            let fwd = e * b.yaw.sin() + n * b.yaw.cos();
            let right = fwd.cross(up).normalize();
            let base = |scale: DVec3, lift: f64| -> Mat4 {
                Mat4::from_cols(
                    (right * scale.x).as_vec3().extend(0.0),
                    (up * scale.y).as_vec3().extend(0.0),
                    (-fwd * scale.z).as_vec3().extend(0.0),
                    (rel + up * lift).as_vec3().extend(1.0),
                )
            };
            // A torn envelope sags and folds in on itself as it falls.
            let squash = 1.0 - 0.7 * b.collapse;
            let env_s = DVec3::new(ks * (1.0 - 0.45 * b.collapse), ks * squash, ks * (1.0 - 0.45 * b.collapse));
            let scheme = b.plan.scheme as u32 % SCHEMES.len() as u32;
            if d > NEAR_MODEL {
                out.push((ENV_FAR + scheme, inst(base(env_s, MOUTH[big]))));
                if d < 9_000.0 {
                    out.push((RIG_FAR, inst(base(DVec3::splat(if b.plan.large { 1.6 } else { 1.0 }), 0.0))));
                }
            } else {
                out.push((ENV + scheme, inst(base(env_s, MOUTH[big]))));
                out.push((SKIRT, inst(base(DVec3::splat(ks), MOUTH[big]))));
                if b.collapse < 0.3 {
                    out.push((if b.plan.large { WIRES_LARGE } else { WIRES_SPORT }, inst(base(DVec3::ONE, 0.0))));
                }
                out.push((if b.plan.large { RIG_LARGE } else { RIG_SPORT }, inst(base(DVec3::ONE, 0.0))));
            }
            if b.burner && d < 15_000.0 {
                // A roaring jet about three metres long, flickering.
                let f = (time * 23.0 + k as f64 * 1.7).sin() * 0.5 + (time * 37.0 + k as f64).sin() * 0.3;
                let len = (if b.plan.large { 3.6 } else { 2.9 }) * (1.0 + 0.08 * f);
                let w = 1.0 + 0.1 * f;
                out.push((FLAME, inst(base(DVec3::new(w, len, w), NOZZLE[big]))));
            }
        }
    }
}

/// Local solar hour at a longitude (degrees), from UTC hours.
fn local_hour(utc: f64, lon: f64) -> f64 {
    (utc + lon / 15.0).rem_euclid(24.0)
}

/// Hours from `a` to `b` on the clock, between -12 and 12.
fn hours_between(a: f64, b: f64) -> f64 {
    (b - a + 12.0).rem_euclid(24.0) - 12.0
}

fn inst(m: Mat4) -> Inst {
    let c = m.to_cols_array_2d();
    Inst { m: [[c[0][0], c[1][0], c[2][0], c[3][0]], [c[0][1], c[1][1], c[2][1], c[3][1]], [c[0][2], c[1][2], c[2][2], c[3][2]]], tint: [1.0, 1.0, 1.0, 0.0], ext: [0.0; 4] }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_models_load_and_match_the_constants() {
        let m = models();
        assert_eq!(m.meshes.len(), 2 * SCHEMES.len() + PARTS.len());
        let objs = crate::aircraft::fa18::load(include_bytes!("../assets/balloons.bin.gz"), "balloons");
        for (rig, big) in [("rig sport", 0), ("rig large", 1)] {
            let o = &objs[rig];
            // (pivot: the nozzle; axis: the mouth, both above the basket floor, game frame y up.)
            assert!((o.pivot.y as f64 - NOZZLE[big]).abs() < 1e-3, "{rig} nozzle {}", o.pivot.y);
            assert!((o.axis.y as f64 - MOUTH[big]).abs() < 1e-3, "{rig} mouth {}", o.axis.y);
        }
        // The envelope: mouth at its origin, crown ENV_H above, ENV_R across.
        let env = &objs["env rainbow"];
        let top = env.mesh.v.iter().map(|v| v.pos[1]).fold(f32::MIN, f32::max) as f64;
        let wide = env.mesh.v.iter().map(|v| (v.pos[0] * v.pos[0] + v.pos[2] * v.pos[2]).sqrt()).fold(0.0f32, f32::max) as f64;
        assert!((top - ENV_H).abs() < 0.05, "{top}");
        assert!((wide - ENV_R).abs() / ENV_R < 0.04, "{wide}");
    }

    #[test]
    fn sunrise_is_where_the_almanac_puts_it() {
        // Göreme, 21 June: sunrise about 4:55 local solar time (civil 5:32,
        // UTC+3, 35° E is 20 min ahead of the zone's own meridian... the
        // solar figure is what the game's sun uses).
        let h = half_day(38.64, 172.0).unwrap();
        assert!((12.0 - h - 4.75).abs() < 0.25, "{}", 12.0 - h);
        // Equinox: about six.
        let h = half_day(0.0, 80.0).unwrap();
        assert!((12.0 - h - 6.0).abs() < 0.1);
    }

    #[test]
    fn the_schedule_is_the_same_each_time_and_follows_the_seasons() {
        let a = schedule(&SITES[0], 0, 150, false);
        let b = schedule(&SITES[0], 0, 150, false);
        assert!(a.len() >= 70 && a.len() <= 150, "{}", a.len());
        assert_eq!(a.iter().map(|p| p.launch).collect::<Vec<_>>(), b.iter().map(|p| p.launch).collect::<Vec<_>>());
        // Mornings around sunrise; mostly large balloons in Cappadocia.
        let rise = 12.0 - half_day(SITES[0].lat, 150.0).unwrap();
        assert!(a.iter().all(|p| p.launch > rise - 0.3 && p.launch < rise + 0.8));
        assert!(a.iter().filter(|p| p.large).count() as f64 / a.len() as f64 > 0.8);
        // Bagan flies October to March only; Albuquerque's Fiesta fills the sky.
        let bagan = SITES.iter().position(|s| s.name == "Bagan").unwrap();
        assert!(schedule(&SITES[bagan], bagan, 180, false).is_empty());
        assert!(!schedule(&SITES[bagan], bagan, 20, false).is_empty());
        let abq = SITES.iter().position(|s| s.name == "Albuquerque").unwrap();
        assert_eq!(schedule(&SITES[abq], abq, 280, false).len(), 500);
        // Chambley's Mondial only in odd years.
        let ch = SITES.iter().position(|s| s.name == "Chambley").unwrap();
        assert!(schedule(&SITES[ch], ch, 210, true).len() >= 300);
        assert!(schedule(&SITES[ch], ch, 210, false).len() < 10);
    }

    #[test]
    fn a_balloon_takes_off_drifts_and_lands() {
        let site = &SITES[0];
        let plan = Plan { launch: 6.0, lat: site.lat, lon: site.lon, large: true, scheme: 0, cruise: (400.0, 600.0), minutes: 60.0, seed: 99 };
        let mut b = Balloon::new(plan, 0, site.elev);
        b.phase = Phase::Flying;
        let start = b.p;
        let (e, _, _) = geo::enu(start);
        let ground = |_: DVec3| Some(site.elev);
        let wind = |_: DVec3, _: f64| e * 4.0;
        let calm = |_: DVec3| true;
        let env = Env { ground: &ground, wind: &wind, calm: &calm };
        let mut top: f64 = 0.0;
        let mut burned = 0.0;
        for _ in 0..(75 * 60) {
            b.step(1.0, site.elev, &env);
            top = top.max(b.p.length() - geo::R - site.elev);
            burned += b.burner as u32 as f64;
        }
        assert!(top > 350.0 && top < 700.0, "highest {top}");
        assert_eq!(b.phase, Phase::Landed, "{:?} at {:.0} m", b.phase, b.p.length() - geo::R - site.elev);
        // About 4 m/s for an hour: some 14 km east (it lags the wind a little).
        let went = (b.p - start).dot(e);
        assert!(went > 10_000.0 && went < 16_000.0, "{went}");
        // The burner burns now and then, not all the time.
        assert!(burned > 60.0 && burned < 0.5 * 3600.0, "{burned}");
    }

    #[test]
    fn an_aircraft_through_the_envelope_tears_it() {
        let site = &SITES[0];
        let plan = Plan { launch: 6.0, lat: site.lat, lon: site.lon, large: false, scheme: 0, cruise: (400.0, 600.0), minutes: 60.0, seed: 3 };
        let b = Balloon::new(plan, 0, site.elev);
        let mut bs = Balloons::default();
        bs.list.push(b);
        let (c, _) = bs.list[0].envelope();
        let (e, _, _) = geo::enu(c);
        assert!(bs.hit(c - e * 100.0 + c.normalize() * 30.0, c + e * 100.0 + c.normalize() * 30.0, 5.0).is_none(), "passing above");
        assert!(bs.hit(c - e * 100.0, c + e * 100.0, 5.0).is_some());
        assert_eq!(bs.list[0].phase, Phase::Torn);
    }

    #[test]
    fn near_cappadocia_after_sunrise_the_sky_fills() {
        let site = &SITES[0];
        let cam = geo::lla_to_pos(site.lat.to_radians(), (site.lon - 0.05).to_radians(), 1700.0);
        let ground = |_: DVec3| Some(site.elev);
        let wind = |p: DVec3, _: f64| geo::enu(p).0 * 3.0;
        let calm = |_: DVec3| true;
        let env = Env { ground: &ground, wind: &wind, calm: &calm };
        let mut bs = Balloons::default();
        // Day 150, 5.4 h local solar time: half an hour after sunrise.
        let utc = 5.4 - site.lon / 15.0;
        for _ in 0..20 {
            bs.step(0.5, cam, Level::High, utc, 150.0, false, &env);
        }
        let up = bs.list.iter().filter(|b| b.phase == Phase::Flying).count();
        assert!(bs.list.len() > 30, "{} balloons", bs.list.len());
        assert!(up > 10, "{up} flying");
    }
}

