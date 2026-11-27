//! Power lines: OpenStreetMap's transmission lines (power=line, on steel
//! towers) and distribution lines (power=minor_line, on poles), data ©
//! OpenStreetMap contributors, ODbL. Each vertex of such a way is a support.
//!
//! assets/mkpower.py cuts the planet's lines into 1 x 1 degree tiles, packed
//! in 10 x 10 degree bundles that a GitHub release serves (`RELEASE`); the
//! game downloads the bundle round the aircraft once, keeps it in its cache,
//! and reads the tiles near the camera from it on a thread of its own.
//! `--power-data DIR_OR_URL` takes the bundles from elsewhere.
//!
//! Supports come from assets/blender/pylons.py: lattice towers by voltage
//! (400, 220 and 63-150 kV types) and a concrete pole for the distribution
//! lines, each standing across the line, on the bisector where it turns.
//! The conductors hang between them as catenaries (wires.wgsl draws each
//! span, at least a pixel wide so they don't break up far off), one per
//! phase (three a circuit, as the way's `cables` tag says), and the earth
//! wire over the top. Flying into a tower or the wires is a crash.

use crate::geo;
use crate::graphics::Level;
use crate::render::{Inst, MeshVertex};
use glam::{DVec3, Mat4, Vec3};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::OnceLock;

/// Where the bundles are published (a GitHub release of the game).
pub const RELEASE: &str = "https://github.com/Rolandjg/fableflight/releases/download/power-data-1/";

// ------------------------------------------------------------------ models

/// The supports, in pylons.py's export order; then their far versions.
pub const PIECES: [&str; 8] = ["t400", "t220", "t110", "pole", "t400 far", "t220 far", "t110 far", "pole far"];
pub const BASE: u32 = crate::turbines::BASE + crate::turbines::PIECES.len() as u32;
const FAR: u32 = 4;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    T400 = 0,
    T220 = 1,
    T110 = 2,
    Pole = 3,
}

pub struct Models {
    pub meshes: Vec<(Vec<MeshVertex>, Vec<u32>)>,
    /// per kind: where the phases hang (game frame: x across the line, y up), the earth wires
    pub phases: [Vec<Vec3>; 4],
    pub earth: [Vec<Vec3>; 4],
    pub height: [f32; 4],
}

pub fn models() -> &'static Models {
    static M: OnceLock<Models> = OnceLock::new();
    M.get_or_init(|| {
        let objs = crate::aircraft::fa18::load(include_bytes!("../assets/pylons.bin.gz"), "pylons");
        let meshes = PIECES
            .iter()
            .map(|name| {
                let o = objs.get(*name).unwrap_or_else(|| panic!("pylons model: no piece \"{name}\""));
                let v = o.mesh.v.iter().map(|v| MeshVertex { pos: v.pos, nrm: v.nrm, col: [v.col[0], v.col[1], v.col[2], 1.0] }).collect();
                (v, o.mesh.i.clone())
            })
            .collect();
        let j: serde_json::Value = serde_json::from_str(include_str!("../assets/pylons.json")).expect("pylons.json");
        let pts = |k: &str, f: &str| -> Vec<Vec3> { j[k][f].as_array().map(|a| a.iter().map(|p| Vec3::new(p[0].as_f64().unwrap_or(0.0) as f32, p[1].as_f64().unwrap_or(0.0) as f32, p[2].as_f64().unwrap_or(0.0) as f32)).collect()).unwrap_or_default() };
        let names = ["t400", "t220", "t110", "pole"];
        Models {
            meshes,
            phases: names.map(|n| pts(n, "phases")),
            earth: names.map(|n| pts(n, "earth")),
            height: names.map(|n| j[n]["height"].as_f64().unwrap_or(10.0) as f32),
        }
    })
}

/// How far the transmission lines are drawn (m), by setting; the
/// distribution lines a fraction of it.
pub fn reach(level: Level) -> f64 {
    match level {
        Level::Off => 0.0,
        Level::Low => 4_000.0,
        Level::Medium => 8_000.0,
        Level::High | Level::Ultra => 12_000.0,
    }
}
fn reach_poles(level: Level) -> f64 {
    reach(level) * 0.25
}

// -------------------------------------------------------------------- data

/// One line as a tile has it.
#[derive(Clone, Debug)]
pub struct Way {
    pub id: u64,
    /// 0 line (towers), 1 minor line (poles)
    pub minor: bool,
    pub kv: u16,
    pub cables: u8,
    /// (lat, lon) degrees
    pub pts: Vec<(f64, f64)>,
}

impl Way {
    pub fn kind(&self) -> Kind {
        if self.minor || (self.kv > 0 && self.kv < 45) {
            Kind::Pole
        } else if self.kv >= 300 {
            Kind::T400
        } else if self.kv >= 180 {
            Kind::T220
        } else {
            Kind::T110
        }
    }
    /// Conductors hung: three per circuit, as the tag says, else as the
    /// supports carry (two circuits on the towers, one on a pole).
    pub fn phases(&self) -> usize {
        let max = if self.kind() == Kind::Pole { 3 } else { 6 };
        match self.cables {
            3 | 4 => 3,
            6..=8 => 6,
            0 => max,
            n => ((n as usize / 3) * 3).clamp(3, 6),
        }
        .min(max)
    }
}

fn varint(b: &[u8], at: &mut usize) -> i64 {
    let (mut v, mut s) = (0u64, 0);
    while *at < b.len() {
        let c = b[*at];
        *at += 1;
        v |= ((c & 0x7f) as u64) << s;
        if c & 0x80 == 0 || s >= 63 {
            break;
        }
        s += 7;
    }
    ((v >> 1) as i64) ^ -((v & 1) as i64)
}

/// A tile's ways (mkpower.py's format).
pub fn parse_tile(gz: &[u8]) -> Vec<Way> {
    let mut raw = Vec::new();
    if flate2::read::GzDecoder::new(gz).read_to_end(&mut raw).is_err() || raw.len() < 12 || &raw[0..4] != b"FFPW" {
        return Vec::new();
    }
    let u32_at = |p: usize| u32::from_le_bytes(raw[p..p + 4].try_into().unwrap());
    let n = u32_at(8) as usize;
    let mut at = 12;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        if at + 16 > raw.len() {
            break;
        }
        let id = u64::from_le_bytes(raw[at..at + 8].try_into().unwrap());
        let minor = raw[at + 8] == 1;
        let kv = u16::from_le_bytes([raw[at + 9], raw[at + 10]]);
        let cables = raw[at + 11];
        let np = u32_at(at + 12) as usize;
        at += 16;
        let (mut la, mut lo) = (0i64, 0i64);
        let mut pts = Vec::with_capacity(np);
        for _ in 0..np {
            la += varint(&raw, &mut at);
            lo += varint(&raw, &mut at);
            pts.push((la as f64 * 1e-6, lo as f64 * 1e-6));
        }
        out.push(Way { id, minor, kv, cables, pts });
    }
    out
}

/// A bundle's tiles: (lat, lon) -> the tile's gzip bytes.
type Bundle = HashMap<(i16, i16), Vec<u8>>;

/// The tiles of a bundle (mkpower.py).
pub fn parse_bundle(b: &[u8]) -> Bundle {
    let mut out = HashMap::new();
    if b.len() < 12 || &b[0..4] != b"FFPB" {
        return out;
    }
    let n = u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize;
    for k in 0..n {
        let p = 12 + k * 12;
        if p + 12 > b.len() {
            break;
        }
        let la = i16::from_le_bytes([b[p], b[p + 1]]);
        let lo = i16::from_le_bytes([b[p + 2], b[p + 3]]);
        let off = u32::from_le_bytes(b[p + 4..p + 8].try_into().unwrap()) as usize;
        let len = u32::from_le_bytes(b[p + 8..p + 12].try_into().unwrap()) as usize;
        if let Some(s) = b.get(off..off + len) {
            out.insert((la, lo), s.to_vec());
        }
    }
    out
}

fn bundle_of(t: (i16, i16)) -> (i16, i16) {
    (t.0.div_euclid(10) * 10, t.1.div_euclid(10) * 10)
}

/// The loader thread: tile wanted -> its ways (empty if there are none, or
/// the bundle could not be had).
struct Loader {
    ask: mpsc::Sender<(i16, i16)>,
    got: mpsc::Receiver<((i16, i16), Vec<Way>)>,
}

impl Loader {
    fn start(source: String, cache: PathBuf, offline: bool) -> Loader {
        let (ask, rx) = mpsc::channel::<(i16, i16)>();
        let (tx, got) = mpsc::channel();
        std::thread::Builder::new()
            .name("power lines".into())
            .spawn(move || {
                let mut bundles: HashMap<(i16, i16), Bundle> = HashMap::new();
                let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(600))).user_agent(crate::fetch::USER_AGENT).build().into();
                let mut failed: HashMap<(i16, i16), std::time::Instant> = HashMap::new();
                while let Ok(t) = rx.recv() {
                    let bk = bundle_of(t);
                    if !bundles.contains_key(&bk) && failed.get(&bk).is_none_or(|w| w.elapsed().as_secs() > 120) {
                        let name = format!("power_{}_{}.bin", bk.0, bk.1);
                        let bytes = if source.starts_with("http") {
                            let path = cache.join("power").join(&name);
                            std::fs::read(&path).ok().filter(|b| !b.is_empty()).or_else(|| {
                                if offline {
                                    return None;
                                }
                                let url = format!("{}{}", source, name);
                                match agent.get(&url).call() {
                                    Ok(mut res) => {
                                        let b = res.body_mut().with_config().limit(256 << 20).read_to_vec().ok()?;
                                        let _ = std::fs::create_dir_all(path.parent()?);
                                        let tmp = path.with_extension("tmp");
                                        if std::fs::write(&tmp, &b).is_ok() {
                                            let _ = std::fs::rename(&tmp, &path);
                                        }
                                        log::info!("power lines: {name} downloaded ({:.1} MB)", b.len() as f64 / 1e6);
                                        Some(b)
                                    }
                                    // (No bundle: no lines there.)
                                    Err(ureq::Error::StatusCode(404)) => Some(Vec::new()),
                                    Err(e) => {
                                        log::warn!("power lines: {url}: {e}");
                                        None
                                    }
                                }
                            })
                        } else {
                            Some(std::fs::read(PathBuf::from(&source).join(&name)).unwrap_or_default())
                        };
                        match bytes {
                            Some(b) => {
                                bundles.insert(bk, parse_bundle(&b));
                                // (Keep a few: the ones round the aircraft.)
                                if bundles.len() > 6 {
                                    let far = *bundles.keys().filter(|k| **k != bk).max_by_key(|k| (k.0 - bk.0).abs() + (k.1 - bk.1).abs()).unwrap();
                                    bundles.remove(&far);
                                }
                            }
                            None => {
                                failed.insert(bk, std::time::Instant::now());
                            }
                        }
                    }
                    let ways = bundles.get(&bk).and_then(|b| b.get(&t)).map(|gz| parse_tile(gz)).unwrap_or_default();
                    if tx.send((t, ways)).is_err() {
                        break;
                    }
                }
            })
            .expect("power lines thread");
        Loader { ask, got }
    }
}

// ------------------------------------------------------------ in the world

/// A support near the camera.
struct Support {
    lat: f64,
    lon: f64,
    kind: Kind,
    /// the way it is on, and where along it
    way: usize,
    idx: usize,
    /// its foot (planet frame) once the ground there is known
    foot: Option<DVec3>,
    /// across the line (unit, planet frame), at its foot
    across: DVec3,
}

/// A span of wire, as wires.wgsl draws it (48 bytes): ends relative to the
/// camera, how far it sags at mid-span, its radius, the local up and kind
/// (0 conductor, 1 earth wire).
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub struct Wire {
    pub a: [f32; 4],
    pub b: [f32; 4],
    pub up: [f32; 4],
}

#[derive(Default)]
pub struct Power {
    loader: Option<Loader>,
    /// where the bundles come from (URL or folder)
    pub source: Option<String>,
    offline: bool,
    tiles: HashMap<(i16, i16), Vec<Way>>,
    asked: HashSet<(i16, i16)>,
    ways: Vec<Way>,
    supports: Vec<Support>,
    /// (support a, support b) of each span
    spans: Vec<(usize, usize)>,
    at: Option<(DVec3, f64)>,
    since: f64,
    dirty: bool,
}

impl Power {
    pub fn new(source: Option<String>, offline: bool) -> Self {
        Power { source, offline, ..Default::default() }
    }

    /// Asks for the tiles round the camera, takes in those that came, and
    /// stands the supports near it on the ground (`ground(lat, lon)` in
    /// radians).
    pub fn step(&mut self, dt: f64, cam: DVec3, level: Level, ground: &dyn Fn(f64, f64) -> Option<f64>) {
        let reach = reach(level);
        if reach <= 0.0 {
            self.supports.clear();
            self.spans.clear();
            self.at = None;
            return;
        }
        let loader = self.loader.get_or_insert_with(|| {
            let cache = dirs::cache_dir().unwrap_or_else(|| PathBuf::from(".")).join("fablegame");
            Loader::start(self.source.clone().unwrap_or_else(|| RELEASE.to_string()), cache, self.offline)
        });
        let (lat, lon, _) = geo::pos_to_lla(cam);
        let (lat, lon) = (lat.to_degrees(), lon.to_degrees());
        let (dla, dlo) = (reach / 111_000.0, reach / (111_000.0 * lat.to_radians().cos().max(0.05)));
        for la in (lat - dla).floor() as i32..=(lat + dla).floor() as i32 {
            for lo in (lon - dlo).floor() as i32..=(lon + dlo).floor() as i32 {
                let t = (la as i16, ((lo + 180).rem_euclid(360) - 180) as i16);
                if self.asked.insert(t) {
                    let _ = loader.ask.send(t);
                }
            }
        }
        while let Ok((t, ways)) = loader.got.try_recv() {
            self.tiles.insert(t, ways);
            self.dirty = true;
        }
        let moved = self.at.is_none_or(|(p, r)| p.distance(cam) > 0.15 * reach || r != reach);
        if moved || self.dirty {
            self.gather(lat, lon, reach, reach_poles(level));
            self.at = Some((cam, reach));
            self.dirty = false;
            self.since = 1e9;
            // (Tiles far behind are let go, to be asked for again if wanted.)
            let keep = |t: &(i16, i16)| ((t.0 as f64 + 0.5 - lat).abs() < 2.0) && (((t.1 as f64 + 0.5 - lon + 540.0) % 360.0 - 180.0).abs() < 3.0);
            self.tiles.retain(|t, _| keep(t));
            self.asked.retain(keep);
        }
        self.since += dt;
        if self.since > 1.5 {
            self.since = 0.0;
            for s in &mut self.supports {
                if let Some(h) = ground(s.lat.to_radians(), s.lon.to_radians()) {
                    s.foot = Some(geo::lla_to_pos(s.lat.to_radians(), s.lon.to_radians(), h));
                }
            }
        }
    }

    /// The supports within reach of (lat, lon), and the spans between them.
    fn gather(&mut self, lat: f64, lon: f64, reach: f64, reach_poles: f64) {
        let old: HashMap<(u64, usize), DVec3> = self.supports.iter().filter_map(|s| Some(((self.ways[s.way].id, s.idx), s.foot?))).collect();
        let mut seen = HashSet::new();
        self.ways.clear();
        for ways in self.tiles.values() {
            for w in ways {
                if seen.insert(w.id) {
                    self.ways.push(w.clone());
                }
            }
        }
        self.supports.clear();
        self.spans.clear();
        let coslat = lat.to_radians().cos();
        let dist = |p: (f64, f64)| {
            let dy = (p.0 - lat) * 111_000.0;
            let mut dx = p.1 - lon;
            dx -= 360.0 * (dx / 360.0).round();
            (dx * 111_000.0 * coslat).hypot(dy)
        };
        for (wi, w) in self.ways.iter().enumerate() {
            let kind = w.kind();
            let r = if kind == Kind::Pole { reach_poles } else { reach };
            let mut prev: Option<usize> = None;
            for (i, &p) in w.pts.iter().enumerate() {
                if dist(p) > r {
                    prev = None;
                    continue;
                }
                // Across the line: square to the bisector of its two spans.
                let pos = |q: (f64, f64)| geo::lla_to_pos(q.0.to_radians(), q.1.to_radians(), 0.0);
                let here = pos(p);
                let up = here.normalize();
                let dir_of = |a: DVec3, b: DVec3| {
                    let d = b - a;
                    (d - up * d.dot(up)).normalize_or_zero()
                };
                let din = if i > 0 { dir_of(pos(w.pts[i - 1]), here) } else { DVec3::ZERO };
                let dout = if i + 1 < w.pts.len() { dir_of(here, pos(w.pts[i + 1])) } else { DVec3::ZERO };
                let along = (din + dout).normalize_or_zero();
                let along = if along == DVec3::ZERO { geo::enu(here).1 } else { along };
                let across = along.cross(up).normalize();
                let foot = old.get(&(w.id, i)).copied();
                let si = self.supports.len();
                self.supports.push(Support { lat: p.0, lon: p.1, kind, way: wi, idx: i, foot, across });
                if let Some(pi) = prev {
                    self.spans.push((pi, si));
                }
                prev = Some(si);
            }
        }
    }

    pub fn counts(&self) -> (usize, usize, usize) {
        (self.ways.len(), self.supports.len(), self.supports.iter().filter(|s| s.foot.is_some()).count())
    }

    fn frame(s: &Support) -> Option<(DVec3, DVec3, DVec3, DVec3)> {
        let foot = s.foot?;
        let up = foot.normalize();
        let across = (s.across - up * s.across.dot(up)).normalize();
        let along = up.cross(across);
        Some((foot, across, up, along))
    }

    /// Where a support's phase or earth wire hangs (planet frame).
    fn hang(s: &Support, local: Vec3) -> Option<DVec3> {
        let (foot, across, up, along) = Self::frame(s)?;
        Some(foot + across * local.x as f64 + up * local.y as f64 - along * local.z as f64)
    }

    /// The supports in view, as instanced solids, and the spans of wire.
    pub fn instances(&self, cam: DVec3, level: Level, visible: &dyn Fn(DVec3, f64) -> bool, out: &mut Vec<(u32, Inst)>, wires: &mut Vec<Wire>) {
        let reach = reach(level);
        if reach <= 0.0 {
            return;
        }
        let m = models();
        for s in &self.supports {
            let Some((foot, across, up, along)) = Self::frame(s) else { continue };
            let rel = foot - cam;
            let d = rel.length();
            let h = m.height[s.kind as usize] as f64;
            if !visible(rel + up * (h * 0.5), h * 0.6) {
                continue;
            }
            let far = d > if s.kind == Kind::Pole { 400.0 } else { 1_500.0 };
            let mat = Mat4::from_cols(across.as_vec3().extend(0.0), up.as_vec3().extend(0.0), (-along).as_vec3().extend(0.0), rel.as_vec3().extend(1.0));
            out.push((BASE + s.kind as u32 + if far { FAR } else { 0 }, inst(mat)));
        }
        for &(ia, ib) in &self.spans {
            let (a, b) = (&self.supports[ia], &self.supports[ib]);
            let (Some(fa), Some(_)) = (a.foot, b.foot) else { continue };
            let kind = a.kind;
            let w = &self.ways[a.way];
            let span = fa.distance(b.foot.unwrap());
            if span < 5.0 || span > 3_000.0 {
                continue;
            }
            let mid = (fa + b.foot.unwrap()) * 0.5;
            let up = mid.normalize();
            if !visible(mid - cam + up * 20.0, span * 0.5 + 30.0) {
                continue;
            }
            // A conductor's sag at mid-span: about 3% of the span on the
            // lines, 2% on the poles (more on long ones over valleys).
            let sag = (span * if kind == Kind::Pole { 0.02 } else { 0.03 }).clamp(0.2, 40.0);
            let radius = match kind {
                // (Bundles of two to four subconductors: thicker from afar.)
                Kind::T400 => 0.16,
                Kind::T220 => 0.08,
                Kind::T110 => 0.025,
                Kind::Pole => 0.008,
            };
            let mut push = |la: Vec3, lb: Vec3, r: f32, k: f32, sag: f64| {
                if let (Some(pa), Some(pb)) = (Self::hang(a, la), Self::hang(b, lb)) {
                    let (ra, rb) = ((pa - cam).as_vec3(), (pb - cam).as_vec3());
                    wires.push(Wire { a: [ra.x, ra.y, ra.z, sag as f32], b: [rb.x, rb.y, rb.z, r], up: [up.x as f32, up.y as f32, up.z as f32, k] });
                }
            };
            let n = w.phases();
            let (pa, pb) = (&m.phases[a.kind as usize], &m.phases[b.kind as usize]);
            for k in 0..n.min(pa.len()).min(pb.len()) {
                push(pa[k], pb[k], radius, 0.0, sag);
            }
            let (ea, eb) = (&m.earth[a.kind as usize], &m.earth[b.kind as usize]);
            for k in 0..ea.len().min(eb.len()) {
                push(ea[k], eb[k], 0.011, 1.0, sag * 0.85);
            }
        }
    }

    /// An aircraft from `a` to `b` (radius `r`) into a tower or the wires:
    /// where it hit.
    pub fn hit(&self, a: DVec3, b: DVec3, r: f64) -> Option<DVec3> {
        let m = models();
        for s in &self.supports {
            let Some((foot, ..)) = Self::frame(s) else { continue };
            let h = m.height[s.kind as usize] as f64;
            if foot.distance(b) > h + 60.0 {
                continue;
            }
            let (p, q) = closest(a, b, foot, foot + foot.normalize() * h);
            let rad = if s.kind == Kind::Pole { 0.3 } else { 3.0 };
            if p.distance(q) < r * 0.5 + rad {
                return Some(p);
            }
        }
        // The wires: each span as a few straight pieces along its sag.
        for &(ia, ib) in &self.spans {
            let (sa, sb) = (&self.supports[ia], &self.supports[ib]);
            let (Some(fa), Some(fb)) = (sa.foot, sb.foot) else { continue };
            let mid = (fa + fb) * 0.5;
            let span = fa.distance(fb);
            if (b - mid).length() > span * 0.5 + 80.0 {
                continue;
            }
            let up = mid.normalize();
            let sag = (span * if sa.kind == Kind::Pole { 0.02 } else { 0.03 }).clamp(0.2, 40.0);
            let lines: Vec<(Vec3, Vec3)> = m.phases[sa.kind as usize].iter().zip(m.phases[sb.kind as usize].iter()).map(|(x, y)| (*x, *y)).chain(m.earth[sa.kind as usize].iter().zip(m.earth[sb.kind as usize].iter()).map(|(x, y)| (*x, *y))).collect();
            for (la, lb) in lines {
                let (Some(pa), Some(pb)) = (Self::hang(sa, la), Self::hang(sb, lb)) else { continue };
                let at = |t: f64| pa + (pb - pa) * t - up * (sag * 4.0 * t * (1.0 - t));
                for k in 0..8 {
                    let (p0, p1) = (at(k as f64 / 8.0), at((k + 1) as f64 / 8.0));
                    let (p, q) = closest(a, b, p0, p1);
                    if p.distance(q) < r * 0.4 + 0.3 {
                        return Some(q);
                    }
                }
            }
        }
        None
    }
}

/// The closest points between segments a-b and c-d.
fn closest(a: DVec3, b: DVec3, c: DVec3, d: DVec3) -> (DVec3, DVec3) {
    let (u, v, w) = (b - a, d - c, a - c);
    let (aa, bb, cc, dd, ee) = (u.dot(u), u.dot(v), v.dot(v), u.dot(w), v.dot(w));
    let den = aa * cc - bb * bb;
    let mut s = if den > 1e-9 { ((bb * ee - cc * dd) / den).clamp(0.0, 1.0) } else { 0.0 };
    let t = if cc > 1e-9 { ((bb * s + ee) / cc).clamp(0.0, 1.0) } else { 0.0 };
    if aa > 1e-9 {
        s = ((bb * t - dd) / aa).clamp(0.0, 1.0);
    }
    (a + u * s, c + v * t)
}

fn inst(m: Mat4) -> Inst {
    let c = m.to_cols_array_2d();
    Inst { m: [[c[0][0], c[1][0], c[2][0], c[3][0]], [c[0][1], c[1][1], c[2][1], c[3][1]], [c[0][2], c[1][2], c[2][2], c[3][2]]], tint: [1.0; 4], ext: [0.0; 4] }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tile_reads_back_as_written() {
        // One way of three points, as mkpower.py writes it.
        let mut raw = b"FFPW".to_vec();
        raw.extend(1u32.to_le_bytes());
        raw.extend(1u32.to_le_bytes());
        raw.extend(4242u64.to_le_bytes());
        raw.push(0);
        raw.extend(400u16.to_le_bytes());
        raw.push(6);
        raw.extend(3u32.to_le_bytes());
        let zz = |v: i64| ((v << 1) ^ (v >> 63)) as u64;
        let mut put = |v: i64, raw: &mut Vec<u8>| {
            let mut u = zz(v);
            while u >= 0x80 {
                raw.push((u as u8 & 0x7f) | 0x80);
                u >>= 7;
            }
            raw.push(u as u8);
        };
        let pts = [(50_100_000i64, 4_200_000i64), (50_102_000, 4_203_000), (50_101_500, 4_207_000)];
        let (mut pl, mut po) = (0, 0);
        for (la, lo) in pts {
            put(la - pl, &mut raw);
            put(lo - po, &mut raw);
            (pl, po) = (la, lo);
        }
        let mut gz = Vec::new();
        {
            use std::io::Write;
            let mut e = flate2::write::GzEncoder::new(&mut gz, flate2::Compression::default());
            e.write_all(&raw).unwrap();
        }
        let ways = parse_tile(&gz);
        assert_eq!(ways.len(), 1);
        let w = &ways[0];
        assert_eq!((w.id, w.minor, w.kv, w.cables, w.pts.len()), (4242, false, 400, 6, 3));
        assert!((w.pts[2].0 - 50.1015).abs() < 1e-9 && (w.pts[2].1 - 4.207).abs() < 1e-9);
        assert_eq!((w.kind(), w.phases()), (Kind::T400, 6));
    }

    #[test]
    fn supports_by_voltage_and_their_wires() {
        let w = |minor, kv, cables| Way { id: 1, minor, kv, cables, pts: vec![] };
        assert_eq!(w(false, 400, 0).kind(), Kind::T400);
        assert_eq!(w(false, 225, 3).kind(), Kind::T220);
        assert_eq!(w(false, 63, 0).kind(), Kind::T110);
        assert_eq!(w(false, 20, 0).kind(), Kind::Pole);
        assert_eq!(w(true, 0, 0).kind(), Kind::Pole);
        assert_eq!(w(false, 400, 3).phases(), 3);
        assert_eq!(w(true, 20, 0).phases(), 3);
        let m = models();
        assert_eq!(m.meshes.len(), PIECES.len());
        for k in 0..4 {
            assert!(m.phases[k].len() >= 3 && m.phases[k].iter().all(|p| p.y > 5.0 && p.y <= m.height[k] + 0.01), "{k}");
        }
        assert_eq!(m.earth[Kind::T400 as usize].len(), 1);
    }

    #[test]
    fn an_aircraft_into_the_wires_hits_them() {
        let pos = |la: f64, lo: f64| geo::lla_to_pos(la.to_radians(), lo.to_radians(), 100.0);
        let (a, b) = (pos(50.0, 4.0), pos(50.0, 4.005));
        let up = a.normalize();
        let east = (b - a).normalize();
        let across = east.cross(up).normalize();
        let s = |lat: f64, lon: f64, idx: usize, foot: DVec3| Support { lat, lon, kind: Kind::T400, way: 0, idx, foot: Some(foot), across };
        let p = Power {
            ways: vec![Way { id: 1, minor: false, kv: 400, cables: 6, pts: vec![(50.0, 4.0), (50.0, 4.005)] }],
            supports: vec![s(50.0, 4.0, 0, a), s(50.0, 4.005, 1, b)],
            spans: vec![(0, 1)],
            ..Default::default()
        };
        let mid = (a + b) * 0.5;
        // Through the lower phases at mid-span (28 m up, less the sag), across the line.
        let h = 28.0 - 0.03 * a.distance(b);
        let x = models().phases[0][0].x as f64;
        let at = mid + up * h + across * x;
        assert!(p.hit(at - east.cross(up) * 30.0 - east * 0.1, at + east.cross(up) * 30.0, 4.0).is_some());
        // Well over the top: clear.
        assert!(p.hit(mid + up * 150.0 - across * 30.0, mid + up * 150.0 + across * 30.0, 4.0).is_none());
    }
}
