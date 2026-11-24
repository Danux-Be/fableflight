//! Flights kept and played back: the recorder that follows the aircraft, the
//! replay that puts it back where it was, and flights saved to disk.
//!
//! The recorder keeps the last hour of the flight, twenty frames a second:
//! where the aircraft was, how it sat, and what moved on it (controls, flaps,
//! gear, propeller, oleos). A replay draws the aircraft from these frames
//! instead of the physics; taking over from a frame starts the flight again
//! from there.
//!
//! A saved flight is the aircraft's state (enough to fly on from it) and the
//! recording so far, gzipped, in `<config>/fablegame/flights/`.

use crate::aircraft::{Spec, MAX_GEAR};
use crate::nav;
use crate::plane::Plane;
use bytemuck::{Pod, Zeroable};
use glam::{DQuat, DVec3, Quat, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::path::PathBuf;

/// frames a second
pub const HZ: f64 = 20.0;
/// seconds kept
pub const KEEP: f64 = 3600.0;
/// replay speeds offered
pub const SPEEDS: [f64; 6] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0];

const ON_GROUND: u32 = 1 << 8;
const GEAR_DOWN: u32 = 1 << 9;
const LIGHTS: u32 = 1 << 10;
const RUNNING: u32 = 1 << 11;

/// The aircraft at one instant, as drawn.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug, PartialEq)]
pub struct Frame {
    /// seconds since the recording began
    pub t: f64,
    pub pos: [f64; 3],
    pub rot: [f32; 4],
    pub vel: [f32; 3],
    pub stick: [f32; 3],
    pub throttle: f32,
    pub power: f32,
    pub prop_rate: f32,
    pub n1: f32,
    pub burner: f32,
    pub flaps: f32,
    pub slats: f32,
    pub spoilers: f32,
    pub speedbrake: f32,
    pub reverser: f32,
    pub gear: f32,
    pub canopy: f32,
    pub trim: f32,
    pub compress: [f32; MAX_GEAR],
    pub wheel_spin: [f32; MAX_GEAR],
    /// the flap notch (low byte) and the flags above
    pub bits: u32,
}

const _: () = assert!(std::mem::size_of::<Frame>() % 8 == 0);

impl Frame {
    pub fn of(t: f64, p: &Plane) -> Self {
        let f = |x: f64| x as f32;
        let mut bits = p.flap_set as u32;
        for (on, b) in [(p.on_ground, ON_GROUND), (p.gear_down, GEAR_DOWN), (p.landing_lights, LIGHTS), (p.running, RUNNING)] {
            if on {
                bits |= b;
            }
        }
        Self {
            t,
            pos: p.pos.to_array(),
            rot: p.rot.as_quat().to_array(),
            vel: p.vel.as_vec3().to_array(),
            stick: p.stick.as_vec3().to_array(),
            throttle: f(p.throttle),
            power: f(p.power),
            prop_rate: f(p.prop_rate),
            n1: f(p.n1),
            burner: f(p.burner),
            flaps: f(p.flaps),
            slats: f(p.slats),
            spoilers: f(p.spoilers),
            speedbrake: f(p.speedbrake),
            reverser: f(p.reverser),
            gear: f(p.gear),
            canopy: f(p.canopy),
            trim: f(p.trim),
            compress: p.compress.map(f),
            wheel_spin: p.wheel_spin.map(f),
            bits,
        }
    }

    pub fn pos(&self) -> DVec3 {
        DVec3::from_array(self.pos)
    }
    pub fn rot(&self) -> DQuat {
        Quat::from_array(self.rot).as_dquat().normalize()
    }
    pub fn vel(&self) -> DVec3 {
        Vec3::from_array(self.vel).as_dvec3()
    }
    pub fn on_ground(&self) -> bool {
        self.bits & ON_GROUND != 0
    }
    pub fn running(&self) -> bool {
        self.bits & RUNNING != 0
    }

    /// Between `a` and `b`, `k` of the way (0..1). The switches are `a`'s.
    pub fn lerp(a: &Frame, b: &Frame, k: f64) -> Frame {
        let kf = k as f32;
        let l = |x: f32, y: f32| x + (y - x) * kf;
        let la = |x: [f32; MAX_GEAR], y: [f32; MAX_GEAR]| std::array::from_fn(|i| l(x[i], y[i]));
        let l3 = |x: [f32; 3], y: [f32; 3]| std::array::from_fn(|i| l(x[i], y[i]));
        Frame {
            t: a.t + (b.t - a.t) * k,
            pos: a.pos().lerp(b.pos(), k).to_array(),
            rot: a.rot().slerp(b.rot(), k).as_quat().to_array(),
            vel: l3(a.vel, b.vel),
            stick: l3(a.stick, b.stick),
            throttle: l(a.throttle, b.throttle),
            power: l(a.power, b.power),
            prop_rate: l(a.prop_rate, b.prop_rate),
            n1: l(a.n1, b.n1),
            burner: l(a.burner, b.burner),
            flaps: l(a.flaps, b.flaps),
            slats: l(a.slats, b.slats),
            spoilers: l(a.spoilers, b.spoilers),
            speedbrake: l(a.speedbrake, b.speedbrake),
            reverser: l(a.reverser, b.reverser),
            gear: l(a.gear, b.gear),
            canopy: l(a.canopy, b.canopy),
            trim: l(a.trim, b.trim),
            compress: la(a.compress, b.compress),
            wheel_spin: la(a.wheel_spin, b.wheel_spin),
            bits: a.bits,
        }
    }

    /// Puts the aircraft where this frame has it, to be drawn (and heard).
    /// `dt`: how far the replay moved on, for the propeller and wheels.
    pub fn show(&self, p: &mut Plane, dt: f64) {
        let d = |x: f32| x as f64;
        p.pos = self.pos();
        p.rot = self.rot();
        p.vel = self.vel();
        p.omega = DVec3::ZERO;
        p.stick = Vec3::from_array(self.stick).as_dvec3();
        p.throttle = d(self.throttle);
        p.power = d(self.power);
        p.prop_rate = d(self.prop_rate);
        p.prop = (p.prop + p.prop_rate * dt).rem_euclid(std::f64::consts::TAU * 64.0);
        p.n1 = d(self.n1);
        p.burner = d(self.burner);
        p.flaps = d(self.flaps);
        p.flap_set = (self.bits & 0xff) as u8;
        p.slats = d(self.slats);
        p.spoilers = d(self.spoilers);
        p.speedbrake = d(self.speedbrake);
        p.reverser = d(self.reverser);
        p.gear = d(self.gear);
        p.gear_down = self.bits & GEAR_DOWN != 0;
        p.canopy = d(self.canopy);
        p.canopy_open = self.canopy > 0.5;
        p.trim = d(self.trim);
        p.compress = self.compress.map(d);
        p.wheel_spin = self.wheel_spin.map(d);
        for k in 0..MAX_GEAR {
            p.wheel_rot[k] = (p.wheel_rot[k] + p.wheel_spin[k] * dt).rem_euclid(std::f64::consts::TAU);
        }
        p.on_ground = self.on_ground();
        p.landing_lights = self.bits & LIGHTS != 0;
        p.running = self.running();
    }

    /// Flies on from this frame: the state a replay is taken over at.
    pub fn take_over(&self, p: &mut Plane) {
        self.show(p, 0.0);
        p.omega = DVec3::ZERO;
        if !self.running() {
            p.cold_and_dark();
            p.sys.elec.master = true;
        }
        p.settle_after_load();
    }
}

/// The frames of the flight so far, the last `KEEP` seconds of them.
#[derive(Default)]
pub struct Recorder {
    pub frames: VecDeque<Frame>,
    t: f64,
    next: f64,
}

impl Recorder {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// One physics step of `h` seconds has been flown.
    pub fn step(&mut self, h: f64, p: &Plane) {
        if self.t + 1e-9 >= self.next {
            self.frames.push_back(Frame::of(self.t, p));
            self.next = (self.next + 1.0 / HZ).max(self.t);
            while self.frames.front().is_some_and(|f| self.t - f.t > KEEP) {
                self.frames.pop_front();
            }
        }
        self.t += h;
    }

    pub fn start(&self) -> f64 {
        self.frames.front().map_or(0.0, |f| f.t)
    }
    pub fn end(&self) -> f64 {
        self.frames.back().map_or(0.0, |f| f.t)
    }
    /// Seconds there is to play back.
    pub fn len_s(&self) -> f64 {
        self.end() - self.start()
    }

    /// The aircraft at time `t`, between the frames around it.
    pub fn at(&self, t: f64) -> Option<Frame> {
        let f = &self.frames;
        let i = f.partition_point(|x| x.t <= t);
        match (i.checked_sub(1).and_then(|j| f.get(j)), f.get(i)) {
            (Some(a), Some(b)) => Some(Frame::lerp(a, b, ((t - a.t) / (b.t - a.t).max(1e-9)).clamp(0.0, 1.0))),
            (Some(a), None) => Some(*a),
            (None, Some(b)) => Some(*b),
            (None, None) => None,
        }
    }

    /// Forgets everything after `t`, to record on from there.
    pub fn cut(&mut self, t: f64) {
        while self.frames.back().is_some_and(|f| f.t > t) {
            self.frames.pop_back();
        }
        self.t = t;
        self.next = t + 1.0 / HZ;
    }

    /// Carries on from frames recorded before (a saved flight).
    pub fn resume(&mut self, frames: Vec<Frame>) {
        let t = frames.last().map_or(0.0, |f| f.t);
        self.frames = frames.into();
        self.t = t;
        self.next = t + 1.0 / HZ;
    }
}

/// A time in the recording: "1:05", or "1:02:05" past the hour.
pub fn clock(secs: f64) -> String {
    let s = secs.max(0.0) as i64;
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{}:{:02}", s / 60, s % 60) }
}

/// A replay under way: where in the recording, how fast, playing or held.
pub struct Player {
    pub t: f64,
    pub speed: f64,
    pub playing: bool,
}

impl Player {
    /// Starts a minute before the end (the landing or crash just flown), or
    /// at the beginning of a shorter recording.
    pub fn new(rec: &Recorder) -> Self {
        Self { t: (rec.end() - 60.0).max(rec.start()), speed: 1.0, playing: true }
    }

    /// Moves on `dt` seconds of real time; it holds at the end.
    pub fn advance(&mut self, dt: f64, rec: &Recorder) -> f64 {
        let before = self.t;
        if self.playing {
            self.t += dt * self.speed;
        }
        if self.t >= rec.end() {
            self.t = rec.end();
            self.playing = false;
        }
        self.t = self.t.max(rec.start());
        self.t - before
    }

    pub fn faster(&mut self, up: bool) {
        let i = SPEEDS.iter().position(|s| *s >= self.speed).unwrap_or(2);
        self.speed = SPEEDS[if up { (i + 1).min(SPEEDS.len() - 1) } else { i.saturating_sub(1) }];
    }
}

/// A radio station tuned, as kept in a saved flight (found again by name).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Tuned {
    pub kind: u8,
    pub ident: String,
    pub name: String,
    pub lat: f64,
    pub lon: f64,
}

impl Tuned {
    fn of(s: &nav::Station) -> Self {
        let kind = match s.kind {
            nav::Kind::Vor => 0,
            nav::Kind::Ndb => 1,
            nav::Kind::Ils => 2,
        };
        Self { kind, ident: s.ident.clone(), name: s.name.clone(), lat: s.lat, lon: s.lon }
    }

    pub fn find(&self) -> Option<nav::Station> {
        let kind = [nav::Kind::Vor, nav::Kind::Ndb, nav::Kind::Ils][self.kind.min(2) as usize];
        nav::near(self.lat, self.lon, 5_000.0, kind).into_iter().map(|(s, _)| s).find(|s| s.ident == self.ident && s.name == self.name)
    }
}

/// What a saved flight flies on from.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct State {
    /// the aircraft (its first key, as `--aircraft` takes)
    pub aircraft: String,
    pub pos: [f64; 3],
    pub vel: [f64; 3],
    pub rot: [f64; 4],
    pub omega: [f64; 3],
    pub throttle: f64,
    pub mixture: f64,
    pub propeller: f64,
    pub flap_set: u8,
    pub gear_down: bool,
    pub speedbrake: f64,
    pub trim: f64,
    pub fuel: f64,
    pub payload: f64,
    pub running: bool,
    pub ignition: bool,
    pub master: bool,
    pub parking_brake: bool,
    pub landing_lights: bool,
    pub on_ground: bool,
    /// the time of day (UTC hours) and day of the year
    pub utc: f64,
    pub doy: f64,
    pub nav: Option<Tuned>,
    pub obs: f64,
    pub adf: Option<Tuned>,
}

impl State {
    pub fn of(p: &Plane, utc: f64, doy: f64, radios: &nav::Radios) -> Self {
        Self {
            aircraft: p.spec.keys.first().copied().unwrap_or_default().to_string(),
            pos: p.pos.to_array(),
            vel: p.vel.to_array(),
            rot: p.rot.to_array(),
            omega: p.omega.to_array(),
            throttle: p.throttle,
            mixture: p.mixture,
            propeller: p.propeller,
            flap_set: p.flap_set,
            gear_down: p.gear_down,
            speedbrake: p.speedbrake,
            trim: p.trim,
            fuel: p.fuel,
            payload: p.payload,
            running: p.running,
            ignition: p.ignition,
            master: p.sys.elec.master,
            parking_brake: p.parking_brake,
            landing_lights: p.landing_lights,
            on_ground: p.on_ground,
            utc,
            doy,
            nav: radios.nav.as_ref().map(Tuned::of),
            obs: radios.obs,
            adf: radios.adf.as_ref().map(Tuned::of),
        }
    }

    pub fn pos(&self) -> DVec3 {
        DVec3::from_array(self.pos)
    }

    /// Sets a freshly spawned aircraft of the same type to this state.
    pub fn apply(&self, p: &mut Plane) {
        p.pos = self.pos();
        p.vel = DVec3::from_array(self.vel);
        p.rot = DQuat::from_array(self.rot).normalize();
        p.omega = DVec3::from_array(self.omega);
        p.throttle = self.throttle;
        p.mixture = self.mixture;
        p.propeller = self.propeller;
        p.flap_set = self.flap_set.min(p.spec.max_notch());
        p.flaps = p.spec.flap_frac(p.flap_set);
        p.slats = if p.spec.slats_at > 0 && p.flap_set >= p.spec.slats_at { 1.0 } else { 0.0 };
        p.gear_down = self.gear_down || p.spec.fixed_gear();
        p.gear = if p.gear_down { 1.0 } else { 0.0 };
        p.speedbrake = self.speedbrake;
        p.spoilers = self.speedbrake;
        p.trim = self.trim;
        p.fuel = self.fuel;
        p.payload = self.payload;
        if !self.running {
            p.cold_and_dark();
        }
        p.running = self.running;
        p.ignition = self.ignition;
        p.sys.elec.master = self.master;
        p.parking_brake = self.parking_brake;
        p.landing_lights = self.landing_lights;
        p.on_ground = self.on_ground;
        p.settle_after_load();
    }

    pub fn radios(&self) -> nav::Radios {
        let mut r = nav::Radios { nav: self.nav.as_ref().and_then(Tuned::find), obs: self.obs, adf: self.adf.as_ref().and_then(Tuned::find), baro: None };
        if r.nav.is_none() {
            r.obs = 0.0;
        }
        r
    }
}

/// What the list of saved flights shows of one, read without its frames.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Header {
    pub version: u32,
    pub name: String,
    /// when it was saved (Unix seconds)
    pub saved: i64,
    /// the aircraft's display name
    pub aircraft_name: String,
    /// the airport it is at or near, else empty
    pub place: String,
    /// seconds of recording kept with it
    pub recorded: f64,
    pub state: State,
}

pub struct Saved {
    pub header: Header,
    pub frames: Vec<Frame>,
}

const MAGIC: &[u8; 4] = b"FFLT";
const VERSION: u32 = 1;

fn dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("fablegame").join("flights"))
}

impl Saved {
    pub fn new(name: String, spec: &Spec, place: String, state: State, rec: &Recorder) -> Self {
        let frames: Vec<Frame> = rec.frames.iter().copied().collect();
        let recorded = rec.len_s();
        Saved { header: Header { version: VERSION, name, saved: crate::flightlog::now_unix(), aircraft_name: spec.name.to_string(), place, recorded, state }, frames }
    }

    pub fn encode(&self) -> std::io::Result<Vec<u8>> {
        let json = serde_json::to_vec(&self.header)?;
        let mut z = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        z.write_all(MAGIC)?;
        z.write_all(&VERSION.to_le_bytes())?;
        z.write_all(&(json.len() as u32).to_le_bytes())?;
        z.write_all(&json)?;
        z.write_all(&(self.frames.len() as u32).to_le_bytes())?;
        z.write_all(bytemuck::cast_slice(&self.frames))?;
        z.finish()
    }

    /// Reads the header and, with `frames`, the recording.
    pub fn decode(data: &[u8], frames: bool) -> Option<Saved> {
        let mut z = flate2::read::GzDecoder::new(data);
        let mut head = [0u8; 12];
        z.read_exact(&mut head).ok()?;
        let word = |i: usize| u32::from_le_bytes(head[i..i + 4].try_into().unwrap());
        if &head[..4] != MAGIC || word(4) != VERSION {
            return None;
        }
        let mut json = vec![0u8; word(8) as usize];
        z.read_exact(&mut json).ok()?;
        let header: Header = serde_json::from_slice(&json).ok()?;
        let mut out = Vec::new();
        if frames {
            let mut n = [0u8; 4];
            z.read_exact(&mut n).ok()?;
            let n = u32::from_le_bytes(n) as usize;
            // (a header can claim anything: no more than an hour and a bit)
            if n > (KEEP * HZ) as usize * 2 {
                return None;
            }
            out = vec![Frame::zeroed(); n];
            z.read_exact(bytemuck::cast_slice_mut(&mut out)).ok()?;
        }
        Some(Saved { header, frames: out })
    }

    /// Writes it to the flights folder; the file's name is returned.
    pub fn save(&self) -> Option<String> {
        let d = dir()?;
        std::fs::create_dir_all(&d).ok()?;
        let mut file = format!("{}.ffl", self.header.saved);
        let mut k = 1;
        while d.join(&file).exists() {
            file = format!("{}-{k}.ffl", self.header.saved);
            k += 1;
        }
        let tmp = d.join(format!("{file}.tmp"));
        std::fs::write(&tmp, self.encode().ok()?).ok()?;
        std::fs::rename(&tmp, d.join(&file)).ok()?;
        Some(file)
    }

    pub fn load(file: &str) -> Option<Saved> {
        Saved::decode(&std::fs::read(dir()?.join(file)).ok()?, true)
    }
}

/// The saved flights, newest first: file name and header.
pub fn list() -> Vec<(String, Header)> {
    let Some(rd) = dir().and_then(|d| std::fs::read_dir(d).ok()) else { return Vec::new() };
    let mut out: Vec<(String, Header)> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if !name.ends_with(".ffl") {
                return None;
            }
            // (only the start of the file is read for its header)
            let mut data = Vec::new();
            std::fs::File::open(e.path()).ok()?.take(256 * 1024).read_to_end(&mut data).ok()?;
            Some((name, Saved::decode(&data, false)?.header))
        })
        .collect();
    out.sort_by(|a, b| b.1.saved.cmp(&a.1.saved).then_with(|| b.0.cmp(&a.0)));
    out
}

pub fn delete(file: &str) {
    if file.ends_with(".ffl") && !file.contains(['/', '\\']) {
        if let Some(d) = dir() {
            let _ = std::fs::remove_file(d.join(file));
        }
    }
}

/// A saved flight named by the user, by file name or by name (`--load`).
pub fn find(what: &str) -> Option<String> {
    let all = list();
    all.iter().find(|(f, _)| f == what || f.trim_end_matches(".ffl") == what).or_else(|| all.iter().find(|(_, h)| h.name.eq_ignore_ascii_case(what))).map(|(f, _)| f.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plane() -> Plane {
        Plane::spawn(crate::aircraft::by_key("c172").unwrap(), 51.19f64.to_radians(), 4.46f64.to_radians(), 600.0, 1.0, 50.0)
    }

    #[test]
    fn the_recorder_keeps_twenty_frames_a_second_and_an_hour() {
        let p = plane();
        let mut r = Recorder::default();
        for _ in 0..180 * 10 {
            r.step(1.0 / 180.0, &p);
        }
        assert!((r.frames.len() as i64 - 200).abs() <= 1, "{}", r.frames.len());
        assert!((r.len_s() - 9.95).abs() < 0.06, "{}", r.len_s());
        let mut r = Recorder::default();
        for _ in 0..(KEEP as usize + 120) {
            r.step(1.0, &p);
        }
        assert!(r.len_s() <= KEEP);
        assert!(r.len_s() > KEEP - 2.0);
    }

    #[test]
    fn frames_in_between_are_blended() {
        let mut p = plane();
        let mut r = Recorder::default();
        let start = p.pos;
        r.step(0.05, &p);
        p.pos += p.forward() * 10.0;
        p.flaps = 1.0;
        r.step(0.05, &p);
        r.step(0.05, &p);
        let f = r.at(0.025).unwrap();
        assert!((f.pos().distance(start) - 5.0).abs() < 1e-6);
        assert!((f.flaps - 0.5).abs() < 1e-6);
        // past the ends: the end frames
        assert_eq!(r.at(-1.0).unwrap().pos(), start);
        r.cut(0.0);
        assert_eq!(r.frames.len(), 1);
        let mut q = plane();
        f.show(&mut q, 0.1);
        assert!((q.pos - f.pos()).length() < 1e-9);
        assert!((q.flaps - 0.5).abs() < 1e-6);
    }

    #[test]
    fn clocks_read_right() {
        assert_eq!(clock(65.4), "1:05");
        assert_eq!(clock(3725.0), "1:02:05");
        assert_eq!(clock(-3.0), "0:00");
    }

    #[test]
    fn the_player_holds_at_the_end_and_changes_speed() {
        let p = plane();
        let mut r = Recorder::default();
        for _ in 0..(20 * 90) {
            r.step(0.05, &p);
        }
        let mut pl = Player::new(&r);
        assert!((r.end() - pl.t - 60.0).abs() < 1e-6);
        pl.faster(true);
        assert_eq!(pl.speed, 2.0);
        pl.faster(false);
        pl.faster(false);
        assert_eq!(pl.speed, 0.5);
        pl.advance(1000.0, &r);
        assert_eq!(pl.t, r.end());
        assert!(!pl.playing);
    }

    #[test]
    fn a_saved_flight_round_trips() {
        let mut p = plane();
        p.flap_set = 1;
        p.fuel = 77.0;
        let mut r = Recorder::default();
        for _ in 0..40 {
            r.step(0.05, &p);
        }
        let state = State::of(&p, 13.5, 100.0, &nav::Radios::default());
        let s = Saved::new("EBAW circuit".into(), p.spec, "EBAW".into(), state.clone(), &r);
        let data = s.encode().unwrap();
        let back = Saved::decode(&data, true).unwrap();
        // (JSON keeps the floats to within a rounding)
        assert_eq!((&back.header.name, &back.header.place, back.header.state.fuel), (&s.header.name, &s.header.place, 77.0));
        assert!(back.header.state.pos().distance(p.pos) < 1e-6);
        assert_eq!(back.frames, s.frames);
        assert_eq!(Saved::decode(&data, false).unwrap().frames.len(), 0);
        assert!(Saved::decode(&data[..data.len() / 2], true).is_none());
        let mut q = plane();
        back.header.state.apply(&mut q);
        assert!(q.pos.distance(p.pos) < 1e-6);
        assert_eq!(q.fuel, 77.0);
        assert_eq!(q.flap_set, 1);
        assert_eq!(State::of(&q, 13.5, 100.0, &nav::Radios::default()).aircraft, "c172");
    }
}
