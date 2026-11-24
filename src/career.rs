//! the career: a pilot who starts as a student with a little money at a home
//! airport and works up. licences come from the tutorials, hours and
//! reputation (private, instrument, commercial, airline transport) and type
//! ratings are bought as training; each opens more aircraft and more jobs.
//! the job board at the airport the pilot is at offers real airports in
//! range of the aircraft: passengers, cargo, ferry flights, aerial photo
//! flights and air displays, with a deadline. a job pays by how it went (on
//! time, the landing, the controllers' word on it, no damage) less the fuel,
//! the landing fee and the rental, and moves the pilot to where it ended.
//! saved as career.json beside the settings. the economy is a game's.

use crate::airports::{Apt, Db};
use crate::missions::Rating;
use crate::{aircraft, flightui, geo, menu, profile, tr_en, Game};
use egui::{pos2, vec2, Color32, Rect, RichText, Sense, Stroke};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// a licence or a type rating.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Hash)]
pub enum Lic {
    Ppl,
    Ir,
    Cpl,
    Atpl,
    BizJet,
    Airliner,
    Sst,
    Display,
    Recon,
}

pub const LICS: [Lic; 9] = [Lic::Ppl, Lic::Ir, Lic::Cpl, Lic::Atpl, Lic::BizJet, Lic::Airliner, Lic::Sst, Lic::Display, Lic::Recon];

impl Lic {
    pub fn name(self) -> &'static str {
        match self {
            Lic::Ppl => "Private pilot",
            Lic::Ir => "Instrument rating",
            Lic::Cpl => "Commercial pilot",
            Lic::Atpl => "Airline transport pilot",
            Lic::BizJet => "Business jet type rating",
            Lic::Airliner => "Boeing 747 type rating",
            Lic::Sst => "Concorde type rating",
            Lic::Display => "Air display team",
            Lic::Recon => "Strategic reconnaissance",
        }
    }

    /// what it takes, in words.
    pub fn needs(self) -> &'static str {
        match self {
            Lic::Ppl => "Tutorials 1, 2 and 3 (first flight, landing, engine start).",
            Lic::Ir => "Private pilot, tutorials 4 and 6 (radio navigation, autopilot and ILS), 10 hours.",
            Lic::Cpl => "Private pilot, 25 hours and 10 jobs done.",
            Lic::Atpl => "Commercial pilot, instrument rating, 100 hours and 1,500 reputation.",
            Lic::BizJet => "Commercial pilot and instrument rating; training costs $150,000.",
            Lic::Airliner => "Airline transport pilot; training costs $500,000.",
            Lic::Sst => "Boeing 747 type rating and 3,000 reputation; training costs $1,000,000.",
            Lic::Display => "Commercial pilot and 800 reputation; training costs $250,000.",
            Lic::Recon => "Air display team and 4,000 reputation.",
        }
    }

    /// training fee ($), none for the ones earned by flying.
    pub fn fee(self) -> f64 {
        match self {
            Lic::BizJet => 150_000.0,
            Lic::Airliner => 500_000.0,
            Lic::Sst => 1_000_000.0,
            Lic::Display => 250_000.0,
            _ => 0.0,
        }
    }
}

/// what kind of aircraft a type is, as the career goes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Class {
    Light,
    BizJet,
    Airliner,
    Sst,
    Fighter,
    Recon,
    None,
}

pub fn class_of(spec: &aircraft::Spec) -> Class {
    let body = spec.origin.map(|o| o.body).unwrap_or(spec.keys.first().copied().unwrap_or(""));
    match spec.id {
        _ if spec.rocket().is_some() => Class::None,
        0 | 8 => Class::Light,
        9 => Class::BizJet,
        1 => Class::Airliner,
        6 => Class::Sst,
        2 | 3 | 4 => Class::Fighter,
        7 => Class::Recon,
        _ if body.contains("747") => Class::Airliner,
        _ if spec.is_jet() && spec.mass > 100_000.0 => Class::Airliner,
        _ if spec.is_jet() => Class::BizJet,
        _ => Class::Light,
    }
}

impl Class {
    pub fn licence(self) -> Option<Lic> {
        match self {
            Class::Light => Some(Lic::Ppl),
            Class::BizJet => Some(Lic::BizJet),
            Class::Airliner => Some(Lic::Airliner),
            Class::Sst => Some(Lic::Sst),
            Class::Fighter => Some(Lic::Display),
            Class::Recon => Some(Lic::Recon),
            Class::None => None,
        }
    }

    /// rental by the hour ($), a buying price (none: not for sale), and the
    /// fuel price ($ a kg).
    pub fn rent(self) -> f64 {
        match self {
            Class::Light => 160.0,
            Class::BizJet => 3_200.0,
            Class::Airliner => 16_000.0,
            Class::Sst => 28_000.0,
            Class::Fighter => 9_000.0,
            Class::Recon => 22_000.0,
            Class::None => 0.0,
        }
    }

    pub fn price(self) -> Option<f64> {
        match self {
            Class::Light => Some(70_000.0),
            Class::BizJet => Some(9_000_000.0),
            Class::Airliner => Some(30_000_000.0),
            Class::Sst => Some(45_000_000.0),
            _ => None,
        }
    }

    pub fn fuel_price(self) -> f64 {
        if self == Class::Light { 2.1 } else { 0.95 }
    }

    /// how far its jobs go (nm), the shortest runway it takes (ft) and whether it must be paved.
    fn reach(self) -> (f64, f64, f64, bool) {
        match self {
            Class::Light => (25.0, 220.0, 1_800.0, false),
            Class::BizJet => (250.0, 2_600.0, 5_000.0, true),
            Class::Airliner => (500.0, 4_500.0, 8_500.0, true),
            Class::Sst => (1_500.0, 3_400.0, 10_500.0, true),
            Class::Fighter => (60.0, 400.0, 7_000.0, true),
            Class::Recon => (400.0, 1_800.0, 9_000.0, true),
            Class::None => (0.0, 0.0, 0.0, true),
        }
    }

    /// the pay a nautical mile, and the start of each job.
    fn rates(self) -> (f64, f64) {
        match self {
            Class::Light => (6.0, 180.0),
            Class::BizJet => (24.0, 4_000.0),
            Class::Airliner => (95.0, 25_000.0),
            Class::Sst => (210.0, 60_000.0),
            Class::Fighter => (60.0, 12_000.0),
            Class::Recon => (120.0, 40_000.0),
            Class::None => (0.0, 0.0),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Kind {
    Passengers,
    Cargo,
    Ferry,
    Photo,
    Medical,
    Display,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Passengers => "Passengers",
            Kind::Cargo => "Cargo",
            Kind::Ferry => "Ferry flight",
            Kind::Photo => "Aerial photography",
            Kind::Medical => "Medical transfer",
            Kind::Display => "Air display",
        }
    }
}

/// a job on the board.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Job {
    pub id: u64,
    pub kind: Kind,
    pub from: String,
    pub to: String,
    /// a point to pass over on the way (photo flights, displays): lat, lon.
    pub via: Option<(f64, f64)>,
    pub dist_nm: f64,
    pub payload_kg: f64,
    pub pax: u32,
    pub pay: f64,
    /// game seconds it should take at most.
    pub limit_s: f64,
    /// the aircraft class it is for.
    pub class: String,
}

/// a sale or a cost, for the statement.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub unix: u64,
    pub what: String,
    pub amount: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Career {
    pub started: bool,
    pub name: String,
    pub base: String,
    pub money: f64,
    pub rep: f64,
    pub hours: f64,
    pub jobs_done: u32,
    pub lics: Vec<Lic>,
    /// aircraft owned, by their key.
    pub owned: Vec<String>,
    pub board: Vec<Job>,
    /// what the board was made for: the airport and how many times it was refreshed.
    pub board_for: String,
    pub refresh: u32,
    pub active: Option<Job>,
    pub ledger: Vec<Entry>,
    pub next_id: u64,
}

impl Default for Career {
    fn default() -> Self {
        Self { started: false, name: String::new(), base: "KHAF".into(), money: 15_000.0, rep: 0.0, hours: 0.0, jobs_done: 0, lics: Vec::new(), owned: Vec::new(), board: Vec::new(), board_for: String::new(), refresh: 0, active: None, ledger: Vec::new(), next_id: 1 }
    }
}

fn path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("fablegame").join("career.json"))
}

/// $12,345 (a minus in front when it is owed).
pub fn money(v: f64) -> String {
    let n = v.abs().round() as u64;
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    format!("{}${out}", if v < 0.0 { "-" } else { "" })
}

struct Rng(u64);
impl Rng {
    fn f(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn longest_ft(a: &Apt) -> (f64, bool) {
    a.rwys.iter().map(|r| (r.len as f64, r.paved)).fold((0.0, false), |m, r| if r.0 > m.0 { r } else { m })
}

impl Career {
    pub fn load() -> Career {
        path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(p) = path() else { return };
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let tmp = p.with_extension("json.tmp");
            if std::fs::write(&tmp, text).is_ok() {
                let _ = std::fs::rename(&tmp, &p);
            }
        }
    }

    pub fn has(&self, l: Lic) -> bool {
        self.lics.contains(&l)
    }

    fn pay(&mut self, what: impl Into<String>, amount: f64) {
        self.money += amount;
        self.ledger.push(Entry { unix: profile::now_unix(), what: what.into(), amount });
        if self.ledger.len() > 200 {
            self.ledger.remove(0);
        }
    }

    /// whether a licence can be had now (the training done, the hours, the reputation).
    pub fn can_get(&self, l: Lic, best: &std::collections::HashMap<String, u32>) -> bool {
        let done = |id: &str| best.contains_key(id);
        !self.has(l)
            && match l {
                Lic::Ppl => done("t1") && done("t2") && done("t3"),
                Lic::Ir => self.has(Lic::Ppl) && done("t4") && done("t6") && self.hours >= 10.0,
                Lic::Cpl => self.has(Lic::Ppl) && self.hours >= 25.0 && self.jobs_done >= 10,
                Lic::Atpl => self.has(Lic::Cpl) && self.has(Lic::Ir) && self.hours >= 100.0 && self.rep >= 1500.0,
                Lic::BizJet => self.has(Lic::Cpl) && self.has(Lic::Ir),
                Lic::Airliner => self.has(Lic::Atpl),
                Lic::Sst => self.has(Lic::Airliner) && self.rep >= 3000.0,
                Lic::Display => self.has(Lic::Cpl) && self.rep >= 800.0,
                Lic::Recon => self.has(Lic::Display) && self.rep >= 4000.0,
            }
            && self.money >= l.fee()
    }

    pub fn get(&mut self, l: Lic) {
        if l.fee() > 0.0 {
            self.pay(format!("{} training", l.name()), -l.fee());
        }
        self.lics.push(l);
    }

    /// may fly this class.
    pub fn rated(&self, c: Class) -> bool {
        c.licence().is_some_and(|l| self.has(l))
    }

    /// a fresh board of jobs out of the base for the classes the pilot is rated on.
    pub fn make_board(&mut self) {
        let db = Db::get();
        let Some(bi) = db.find(&self.base) else { return };
        let base = &db.apts[bi as usize];
        let mut rng = Rng((bi as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ ((self.refresh as u64 + 1) * 0x2545_F491_4F6C_DD1D) ^ self.jobs_done as u64);
        let mut classes: Vec<Class> = [Class::Light, Class::BizJet, Class::Airliner, Class::Sst, Class::Fighter, Class::Recon].into_iter().filter(|c| self.rated(*c)).collect();
        // a student sees what the private licence opens up.
        if classes.is_empty() {
            classes.push(Class::Light);
        }
        let mut board = Vec::new();
        for c in &classes {
            let (lo, hi, rwy_ft, paved) = c.reach();
            let mut cands: Vec<(u32, f64)> = db.apts.iter().enumerate().filter(|(i, a)| {
                if *i as u32 == bi || a.rwys.is_empty() {
                    return false;
                }
                let (len, pv) = longest_ft(a);
                len >= rwy_ft && (pv || !paved) && (*c == Class::Light || a.kind >= 1)
            }).map(|(i, a)| (i as u32, crate::flightplan::dist((base.lat, base.lon), (a.lat, a.lon)) / 1852.0)).filter(|(_, d)| *d >= lo && *d <= hi).collect();
            if cands.is_empty() {
                continue;
            }
            cands.sort_by(|a, b| a.1.total_cmp(&b.1));
            let n = if classes.len() > 2 { 3 } else { 5 };
            let kinds: &[Kind] = match c {
                Class::Light => &[Kind::Passengers, Kind::Cargo, Kind::Ferry, Kind::Photo, Kind::Medical],
                Class::BizJet => &[Kind::Passengers, Kind::Medical, Kind::Cargo, Kind::Ferry],
                Class::Airliner | Class::Sst => &[Kind::Passengers, Kind::Cargo],
                Class::Fighter => &[Kind::Display],
                Class::Recon => &[Kind::Photo],
                Class::None => &[],
            };
            let spec = aircraft::BUILTIN.iter().copied().find(|s| class_of(s) == *c);
            let max_payload = spec.map_or(500.0, |s| s.payload[1]);
            for _ in 0..n {
                let (ai, dist) = cands[(rng.f() * cands.len() as f64) as usize % cands.len()];
                let kind = kinds[(rng.f() * kinds.len() as f64) as usize % kinds.len()];
                let dest = &db.apts[ai as usize];
                let (rate, start) = c.rates();
                let (payload, pax) = match kind {
                    Kind::Passengers => {
                        let pax = ((max_payload * (0.3 + 0.6 * rng.f())) / 90.0).max(1.0).floor();
                        (pax * 90.0, pax as u32)
                    }
                    Kind::Cargo => ((max_payload * (0.35 + 0.6 * rng.f())).round(), 0),
                    Kind::Medical => (180.0_f64.min(max_payload), 1),
                    _ => (0.0, 0),
                };
                // a photo flight or a display goes over a point and comes back to land.
                let (to, via, d) = match kind {
                    Kind::Photo | Kind::Display => {
                        let b = rng.f() * std::f64::consts::TAU;
                        let r = (lo * 0.5 + rng.f() * (hi - lo) * 0.3) * 1852.0 / geo::R;
                        let lat = base.lat + r.to_degrees() * b.cos();
                        let lon = base.lon + r.to_degrees() * b.sin() / base.lat.to_radians().cos().max(0.2);
                        let d = 2.0 * crate::flightplan::dist((base.lat, base.lon), (lat, lon)) / 1852.0;
                        let via = if kind == Kind::Display { (dest.lat, dest.lon) } else { (lat, lon) };
                        let d = if kind == Kind::Display { 2.0 * dist } else { d };
                        (base.ident.clone(), Some(via), d)
                    }
                    _ => (dest.ident.clone(), None, dist),
                };
                let urgency = if kind == Kind::Medical { 1.6 } else { 1.0 };
                let pay = ((start + rate * d * (0.85 + 0.3 * rng.f())) * urgency / 10.0).round() * 10.0;
                let cruise_kt = match c { Class::Light => 110.0, Class::Sst => 1150.0, Class::Recon => 1600.0, Class::Fighter => 420.0, _ => 460.0 };
                let limit_s = (d / cruise_kt * 3600.0 * if kind == Kind::Medical { 1.3 } else { 1.7 }) + 1800.0;
                board.push(Job { id: self.next_id, kind, from: base.ident.clone(), to, via, dist_nm: d, payload_kg: payload, pax, pay, limit_s, class: format!("{c:?}") });
                self.next_id += 1;
            }
        }
        board.sort_by(|a, b| a.pay.total_cmp(&b.pay));
        self.board = board;
        self.board_for = self.base.clone();
    }
}

/// the job being flown.
pub struct Flight {
    pub job: Job,
    pub spec_key: String,
    pub rented: bool,
    pub t0: f64,
    pub fuel0: f64,
    pub violations0: u32,
    /// it went over the point on the way.
    pub via_done: bool,
    pub airborne: bool,
    up_for: f64,
    vs_air: f64,
    pub rating: Option<Rating>,
    pub max_g: f64,
    still: f64,
    pub result: Option<Result>,
}

/// how a job came out.
#[derive(Clone, Debug)]
pub struct Result {
    pub ok: bool,
    pub lines: Vec<(String, f64)>,
    pub rep: f64,
    pub note: String,
}

fn class_parse(s: &str) -> Class {
    match s {
        "BizJet" => Class::BizJet,
        "Airliner" => Class::Airliner,
        "Sst" => Class::Sst,
        "Fighter" => Class::Fighter,
        "Recon" => Class::Recon,
        _ => Class::Light,
    }
}

impl Game {
    /// the aircraft the career flies a job's class in: one owned, else the built-in one.
    fn career_spec(&self, c: Class) -> Option<&'static aircraft::Spec> {
        let owned = self.career.owned.iter().filter_map(|k| aircraft::by_key(k)).find(|s| class_of(s) == c);
        owned.or_else(|| aircraft::BUILTIN.iter().copied().find(|s| class_of(s) == c))
    }

    /// takes a job: off to the base with the aircraft loaded and the route set.
    pub(crate) fn fly_job(&mut self, job: Job) {
        let c = class_parse(&job.class);
        let Some(spec) = self.career_spec(c) else { return };
        let db = Db::get();
        let Some(bi) = db.find(&job.from) else { return };
        self.set_aircraft(spec);
        self.menu.aircraft = aircraft::index_of(spec);
        // the load, and fuel for the trip with an hour (a light aircraft 45 min) to spare.
        let most = spec.payload[1].max(1.0);
        self.menu.payload = Some((job.payload_kg + 85.0).min(most) / most);
        let kt = if c == Class::Light { 110.0 } else { 450.0 };
        let need = spec.fuel_flow[1] * 0.7 * (job.dist_nm / kt + if c == Class::Light { 0.75 } else { 1.0 }) * 3600.0;
        self.menu.fuel = (need / spec.fuel_kg.max(1.0)).clamp(0.25, 1.0);
        self.menu.cold = false;
        self.stand_start = true;
        self.end_mission();
        self.atc.reset();
        self.start_airport(bi, None, false, true);
        self.plan.clear();
        let a = &db.apts[bi as usize];
        let route = match job.via {
            Some((lat, lon)) => format!("{lat:.5},{lon:.5} {}", job.to),
            None => job.to.clone(),
        };
        let _ = self.plan.add_route(&route, Some((a.lat, a.lon)));
        self.plan.cruise_ft = match c { Class::Light => 4_500.0, Class::Sst => 55_000.0, Class::Recon => 75_000.0, Class::Fighter => 15_000.0, _ => 35_000.0_f64 }.min(5_000.0 + job.dist_nm * 150.0);
        self.plan.begin((a.lat, a.lon));
        let owned = self.career.owned.iter().any(|k| spec.keys.contains(&k.as_str()));
        self.career_flight = Some(Flight { job: job.clone(), spec_key: spec.keys.first().copied().unwrap_or("").into(), rented: !owned, t0: self.time, fuel0: spec.fuel_kg * self.menu.fuel, violations0: 0, via_done: job.via.is_none(), airborne: false, up_for: 0.0, vs_air: 0.0, rating: None, max_g: 1.0, still: 0.0, result: None });
        self.career.active = Some(job);
        self.career.save();
    }

    /// once a step of a job: over the point, the landing, the stop at the other end.
    pub(crate) fn career_step(&mut self, dt: f64) {
        let Some(mut f) = self.career_flight.take() else { return };
        if f.result.is_some() {
            self.career_flight = Some(f);
            return;
        }
        let p = &self.plane;
        let (la, lo, _) = geo::pos_to_lla(p.pos);
        let (lat, lon) = (la.to_degrees(), lo.to_degrees());
        if !p.on_ground {
            f.up_for += dt;
            f.vs_air = p.vel.dot(p.pos.normalize());
            if f.up_for > 5.0 {
                f.airborne = true;
            }
            f.max_g = f.max_g.max(p.g_load.abs());
        }
        if let Some((vl, vo)) = f.job.via {
            if crate::flightplan::dist((lat, lon), (vl, vo)) / 1852.0 < 1.5 {
                f.via_done = true;
            }
        }
        let db = Db::get();
        let at_dest = db.find(&f.job.to).map(|i| &db.apts[i as usize]).is_some_and(|a| crate::flightplan::dist((lat, lon), (a.lat, a.lon)) < 5_000.0);
        // the touchdown at the other end, rated as the challenges are.
        if p.on_ground && f.airborne && f.rating.is_none() && at_dest && f.via_done {
            let fpm = f.vs_air * 196.85;
            let (_, _, u) = geo::enu(p.pos);
            let bank = (p.rot * glam::DVec3::X).dot(u).asin().to_degrees();
            let on_rwy = self.world.airfield_near(p.pos, 6_000.0).is_some_and(|fl| fl.geo.on_runway(fl.geo.local(p.pos), 3.0).is_some());
            let score = Rating::score(fpm, 300.0, 0.0, bank, on_rwy, self.spec.is_jet());
            f.rating = Some(Rating { fpm, along: 0.0, off: 0.0, bank, on_runway: on_rwy, score });
        }
        if p.on_ground {
            f.up_for = 0.0;
        }
        f.still = if p.on_ground && p.vel.length() < 0.5 { f.still + dt } else { 0.0 };
        let crashed = self.crashed;
        let done = f.rating.is_some() && f.still > 8.0;
        if crashed || done {
            let r = self.settle_job(&f, crashed);
            f.result = Some(r);
        }
        self.career_flight = Some(f);
    }

    /// pays out a job, or charges for it gone wrong.
    fn settle_job(&mut self, f: &Flight, crashed: bool) -> Result {
        let c = class_parse(&f.job.class);
        let mut lines: Vec<(String, f64)> = Vec::new();
        let hours = ((self.time - f.t0) / 3600.0).max(0.0);
        let fuel_used = (f.fuel0 - self.plane.fuel).max(0.0);
        let fuel_cost = fuel_used * c.fuel_price();
        let rent = if f.rented { c.rent() * hours.max(0.25) } else { 0.0 };
        let violations = self.atc.violations.saturating_sub(f.violations0);
        let mut rep;
        let note;
        if crashed {
            let repair = c.price().unwrap_or(c.rent() * 200.0) * 0.15;
            lines.push((tr_en("Damage to the aircraft").into(), -repair));
            lines.push((tr_en("Fuel").into(), -fuel_cost));
            rep = -120.0;
            note = tr_en("The aircraft was lost. The job is failed.").into();
        } else {
            let late = hours * 3600.0 > f.job.limit_s;
            let landing = f.rating.map_or(50, |r| r.score) as f64;
            let comfort = if f.max_g > 2.2 && f.job.kind == Kind::Passengers { 0.85 } else { 1.0 };
            let quality = ((0.75 + landing / 400.0) * comfort * if late { 0.5 } else { 1.0 } * if violations > 0 { 0.85 } else { 1.0 }).clamp(0.3, 1.25);
            lines.push((format!("{}: {}", tr_en(f.job.kind.name()), f.job.to), f.job.pay));
            if (quality - 1.0).abs() > 0.005 {
                lines.push((tr_en("Quality (landing, timing, comfort, ATC)").into(), f.job.pay * (quality - 1.0)));
            }
            lines.push((tr_en("Fuel").into(), -fuel_cost));
            if rent > 0.0 {
                lines.push((format!("{} {:.1} h", tr_en("Rental"), hours), -rent));
            }
            let fee = match c { Class::Light => 15.0, Class::BizJet => 350.0, Class::Fighter | Class::Recon => 0.0, _ => 2_500.0 };
            if fee > 0.0 {
                lines.push((tr_en("Landing fee").into(), -fee));
            }
            rep = 20.0 + landing * 0.5 + f.job.dist_nm * 0.05 - violations as f64 * 15.0 - if late { 25.0 } else { 0.0 };
            note = if late { tr_en("Late: the client paid half.").into() } else if violations > 0 { tr_en("ATC reported a pilot deviation.").into() } else { tr_en("Delivered.").into() };
            self.career.jobs_done += 1;
            self.career.base = f.job.to.clone();
        }
        rep = rep.round();
        let total: f64 = lines.iter().map(|l| l.1).sum();
        self.career.pay(format!("{} {} to {} ({})", tr_en(f.job.kind.name()), f.job.from, f.job.to, f.spec_key.to_uppercase()), total);
        self.career.rep = (self.career.rep + rep).max(0.0);
        self.career.hours += hours;
        self.career.active = None;
        self.career.board.clear();
        self.career.save();
        Result { ok: !crashed, lines, rep, note }
    }

    /// the job card in flight: where to, how far, the time left; the pay at the end.
    pub(crate) fn job_card(&mut self, ctx: &egui::Context) {
        let Some(f) = self.career_flight.as_ref() else { return };
        let mut close = false;
        let mut abandon = false;
        let db = Db::get();
        let (la, lo, _) = geo::pos_to_lla(self.plane.pos);
        let dest = db.find(&f.job.to).map(|i| &db.apts[i as usize]);
        let (target, label) = match (f.job.via, f.via_done) {
            (Some(v), false) => (Some(v), tr_en("the point to pass over")),
            _ => (dest.map(|a| (a.lat, a.lon)), dest.map_or("", |a| a.name.as_str())),
        };
        let left_nm = target.map(|t| crate::flightplan::dist((la.to_degrees(), lo.to_degrees()), t) / 1852.0);
        let left_s = f.job.limit_s - (self.time - f.t0);
        egui::Area::new(egui::Id::unique("job_card")).anchor(egui::Align2::RIGHT_TOP, [-14.0, 170.0]).show(ctx, |ui| {
            flightui::panel(ui, flightui::glass().inner_margin(14.0), |ui| {
                ui.set_width(300.0);
                flightui::tag(ui, &format!("{}  ·  {}", tr_en("CAREER"), tr_en(f.job.kind.name()).to_uppercase()));
                ui.label(RichText::new(format!("{} → {}", f.job.from, f.job.to)).font(menu::semibold(16.0)).color(Color32::WHITE));
                match &f.result {
                    None => {
                        if let Some(d) = left_nm {
                            flightui::note(ui, &format!("{d:.0} nm {} {label}", tr_en("to")));
                        }
                        let m = (left_s / 60.0).floor();
                        let col = if left_s < 0.0 { menu::BAD } else { flightui::SOFT };
                        ui.label(RichText::new(if left_s >= 0.0 { format!("{} {}", profile::hhmm(left_s), tr_en("left")) } else { format!("{} {:.0} min", tr_en("late by"), -m) }).font(menu::caption()).color(col));
                        if f.job.payload_kg > 0.0 {
                            flightui::note(ui, &format!("{:.0} kg{}", f.job.payload_kg, if f.job.pax > 0 { format!("  ·  {} {}", f.job.pax, tr_en("aboard")) } else { String::new() }));
                        }
                        flightui::note(ui, &format!("{} {}", tr_en("pays"), money(f.job.pay)));
                        if flightui::key(ui, tr_en("Abandon"), false, 90.0, 24.0).clicked() {
                            abandon = true;
                        }
                    }
                    Some(r) => {
                        ui.label(RichText::new(&r.note).font(menu::medium(14.0)).color(if r.ok { flightui::LIT } else { menu::BAD }));
                        for (what, amt) in &r.lines {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(what).font(menu::caption()).color(flightui::SOFT));
                                menu::at_right(ui, ("job_line", what.as_str()), |ui| {
                                    ui.label(RichText::new(money(*amt)).font(menu::caption()).color(if *amt < 0.0 { menu::BAD } else { Color32::WHITE }));
                                });
                            });
                        }
                        let total: f64 = r.lines.iter().map(|l| l.1).sum();
                        ui.label(RichText::new(format!("{} {}  ·  {} {:+.0}", tr_en("Net"), money(total), tr_en("reputation"), r.rep)).font(menu::semibold(14.0)).color(Color32::WHITE));
                        if let Some(rt) = f.rating {
                            flightui::note(ui, &format!("{} {}  ·  {:.0} ft/min", tr_en("Landing"), rt.grade(), rt.fpm.abs()));
                        }
                        if flightui::key(ui, tr_en("Close"), false, 90.0, 26.0).clicked() {
                            close = true;
                        }
                    }
                }
            });
        });
        if abandon {
            self.career.rep = (self.career.rep - 20.0).max(0.0);
            self.career.active = None;
            self.career.save();
            self.career_flight = None;
        } else if close {
            self.career_flight = None;
        }
    }

    /// the career screen: the pilot, the board, the licences, the hangar, the statement.
    pub(crate) fn career_screen(&mut self, ctx: &egui::Context) {
        let screen = ctx.content_rect();
        let mut back = false;
        let mut go: Option<Job> = None;
        egui::Area::new(egui::Id::unique("career_bg")).order(egui::Order::Background).fixed_pos(screen.min).show(ctx, |ui| {
            ui.painter().rect_filled(screen, 0.0, Color32::from_rgba_premultiplied(4, 6, 10, 215));
        });
        if !self.career.started {
            self.career_start(ctx);
            return;
        }
        if self.career.board.is_empty() || self.career.board_for != self.career.base {
            self.career.make_board();
            self.career.save();
        }
        let m = 32.0;
        let w = (screen.width() - 2.0 * m).min(1280.0);
        let x0 = screen.center().x - w * 0.5;
        let best = self.profile.best.clone();
        egui::Area::new(egui::Id::unique("career")).fixed_pos(pos2(x0, m)).show(ctx, |ui| {
            ui.set_width(w);
            ui.horizontal(|ui| {
                ui.set_min_height(48.0);
                if menu::choice(ui, &format!("‹   {}", tr_en("Menu")), false, 120.0).clicked() {
                    back = true;
                }
                ui.add_space(18.0);
                menu::screen_heading(ui, tr_en("Career"));
                menu::at_right(ui, "career_who", |ui| {
                    let base = Db::get().find(&self.career.base).map(|i| Db::get().apts[i as usize].name.clone()).unwrap_or_default();
                    ui.label(RichText::new(format!("{}  ·  {} {}", self.career.name, self.career.base, base)).font(menu::label()).color(menu::DIM));
                });
            });
            ui.add_space(14.0);
            // the figures.
            let (r, _) = ui.allocate_exact_size(vec2(w, 92.0), Sense::hover());
            let p = ui.painter();
            p.rect_filled(r, menu::R_PANEL, menu::PANEL);
            p.rect_stroke(r, menu::R_PANEL, Stroke::new(1.0, menu::LINE_HI), egui::StrokeKind::Inside);
            p.rect_filled(Rect::from_min_size(r.min, vec2(3.0, r.height())), 0.0, menu::ACCENT);
            let lic = LICS.iter().rev().find(|l| self.career.has(**l) && matches!(l, Lic::Ppl | Lic::Cpl | Lic::Atpl)).map_or(tr_en("Student pilot"), |l| tr_en(l.name()));
            let figs = [(money(self.career.money), tr_en("money")), (format!("{:.0}", self.career.rep), tr_en("reputation")), (format!("{:.1}", self.career.hours), tr_en("hours")), (self.career.jobs_done.to_string(), tr_en("jobs")), (lic.to_string(), tr_en("licence"))];
            let step = (r.width() - 40.0) / figs.len() as f32;
            for (k, (v, l)) in figs.iter().enumerate() {
                let left = r.left() + 28.0 + step * k as f32;
                let g = p.layout_no_wrap(v.clone(), menu::bold(if k == 4 { 20.0 } else { 30.0 }), Color32::WHITE);
                p.galley(pos2(left, r.center().y - g.size().y * 0.5 - 8.0), g.clone(), Color32::WHITE);
                p.text(pos2(left, r.center().y + g.size().y * 0.5 - 4.0), egui::Align2::LEFT_TOP, *l, menu::caption(), menu::DIM);
            }
            ui.add_space(14.0);
            let rest = (screen.bottom() - ui.cursor().top() - m).max(240.0);
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                let left_w = (w * 0.6).floor();
                ui.allocate_ui_with_layout(vec2(left_w, rest), egui::Layout::top_down(egui::Align::Min), |ui| {
                    menu::panel().inner_margin(egui::Margin::symmetric(18, 14)).show(ui, |ui| {
                        ui.set_width(left_w - 36.0);
                        ui.set_min_height(rest - 28.0);
                        egui::ScrollArea::vertical().id_salt("career_board").max_height(rest - 40.0).show(ui, |ui| {
                            if let Some(j) = self.career.active.clone() {
                                ui.label(menu::section_text(tr_en("YOUR JOB")));
                                job_row(ui, &j, true);
                                ui.horizontal(|ui| {
                                    if menu::choice(ui, tr_en("Fly it"), true, 140.0).clicked() {
                                        go = Some(j.clone());
                                    }
                                    if menu::choice(ui, tr_en("Give it up"), false, 140.0).clicked() {
                                        self.career.active = None;
                                        self.career.rep = (self.career.rep - 20.0).max(0.0);
                                        self.career.save();
                                    }
                                });
                                ui.add_space(12.0);
                            }
                            ui.horizontal(|ui| {
                                ui.label(menu::section_text(tr_en("JOB BOARD")));
                                menu::at_right(ui, "career_refresh", |ui| {
                                    if menu::choice(ui, tr_en("New jobs"), false, 120.0).clicked() {
                                        self.career.refresh += 1;
                                        self.career.make_board();
                                        self.career.save();
                                    }
                                });
                            });
                            if !self.career.has(Lic::Ppl) {
                                ui.label(RichText::new(tr_en("A student flies the tutorials first: the private licence (right) opens these jobs.")).font(menu::caption()).color(menu::DIM));
                            }
                            let board = self.career.board.clone();
                            for j in &board {
                                let c = class_parse(&j.class);
                                let ok = self.career.rated(c) && self.career.active.is_none();
                                ui.horizontal(|ui| {
                                    job_row(ui, j, false);
                                    if menu::choice(ui, tr_en("Take"), false, 80.0).clicked() && ok {
                                        self.career.active = Some(j.clone());
                                        self.career.board.retain(|b| b.id != j.id);
                                        self.career.save();
                                    }
                                });
                            }
                        });
                    });
                });
                let right_w = w - left_w - 16.0;
                ui.allocate_ui_with_layout(vec2(right_w, rest), egui::Layout::top_down(egui::Align::Min), |ui| {
                    menu::panel().inner_margin(egui::Margin::symmetric(18, 14)).show(ui, |ui| {
                        ui.set_width(right_w - 36.0);
                        ui.set_min_height(rest - 28.0);
                        egui::ScrollArea::vertical().id_salt("career_side").max_height(rest - 40.0).show(ui, |ui| {
                            ui.label(menu::section_text(tr_en("LICENCES AND RATINGS")));
                            let mut get: Option<Lic> = None;
                            let mut train: Option<&'static str> = None;
                            for l in LICS {
                                let has = self.career.has(l);
                                let can = self.career.can_get(l, &best);
                                ui.horizontal(|ui| {
                                    ui.vertical(|ui| {
                                        ui.set_width(right_w - 170.0);
                                        ui.label(RichText::new(format!("{} {}", if has { "✓" } else { "○" }, tr_en(l.name()))).font(menu::label()).color(if has { menu::ACCENT } else { Color32::WHITE }));
                                        if !has {
                                            ui.label(RichText::new(tr_en(l.needs())).font(menu::caption()).color(menu::DIM));
                                        }
                                    });
                                    if !has && can && menu::choice(ui, if l.fee() > 0.0 { tr_en("Train") } else { tr_en("Claim") }, true, 90.0).clicked() {
                                        get = Some(l);
                                    }
                                    if !has && !can && matches!(l, Lic::Ppl | Lic::Ir) {
                                        let next = ["t1", "t2", "t3", "t4", "t6"].into_iter().find(|t| !best.contains_key(*t) && (l == Lic::Ppl) == ["t1", "t2", "t3"].contains(t));
                                        if let Some(t) = next {
                                            if menu::choice(ui, tr_en("Lesson"), false, 90.0).clicked() {
                                                train = Some(t);
                                            }
                                        }
                                    }
                                });
                                ui.add_space(4.0);
                            }
                            if let Some(l) = get {
                                self.career.get(l);
                                self.career.board.clear();
                                self.career.save();
                            }
                            if let Some(t) = train.and_then(crate::missions::by_id) {
                                self.start_mission(t);
                            }
                            ui.add_space(10.0);
                            ui.label(menu::section_text(tr_en("HANGAR")));
                            let mut buy: Option<(&'static aircraft::Spec, f64)> = None;
                            for s in aircraft::BUILTIN {
                                let c = class_of(s);
                                let Some(price) = c.price() else { continue };
                                let own = self.career.owned.iter().any(|k| s.keys.contains(&k.as_str()));
                                ui.horizontal(|ui| {
                                    ui.vertical(|ui| {
                                        ui.set_width(right_w - 170.0);
                                        ui.label(RichText::new(s.name).font(menu::label()).color(Color32::WHITE));
                                        let line = if own { tr_en("Owned: no rental to pay").to_string() } else { format!("{} {}  ·  {} {}/h", tr_en("buy"), money(price), tr_en("rent"), money(c.rent())) };
                                        ui.label(RichText::new(line).font(menu::caption()).color(menu::DIM));
                                    });
                                    if !own && self.career.rated(c) && self.career.money >= price && menu::choice(ui, tr_en("Buy"), false, 90.0).clicked() {
                                        buy = Some((s, price));
                                    }
                                });
                            }
                            if let Some((s, price)) = buy {
                                self.career.pay(format!("{} {}", tr_en("Bought"), s.name), -price);
                                self.career.owned.push(s.keys.first().copied().unwrap_or("").into());
                                self.career.save();
                            }
                            ui.add_space(10.0);
                            ui.label(menu::section_text(tr_en("STATEMENT")));
                            for e in self.career.ledger.iter().rev().take(12) {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(&e.what).font(menu::caption()).color(menu::DIM));
                                    menu::at_right(ui, ("ledger", e.unix, e.what.as_str()), |ui| {
                                        ui.label(RichText::new(money(e.amount)).font(menu::caption()).color(if e.amount < 0.0 { menu::BAD } else { Color32::WHITE }));
                                    });
                                });
                            }
                            ui.add_space(10.0);
                            if menu::choice(ui, tr_en("Start a new career"), false, 220.0).clicked() {
                                self.career = Career::default();
                                self.career.save();
                            }
                        });
                    });
                });
            });
        });
        if let Some(j) = go {
            self.fly_job(j);
        }
        if back {
            self.menu.screen = crate::menu::Screen::Main;
            self.pad_home = true;
        }
    }

    /// a new career: the pilot's name and a home airport.
    fn career_start(&mut self, ctx: &egui::Context) {
        let mut start = false;
        let mut back = false;
        egui::Area::new(egui::Id::unique("career_new")).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            menu::panel().inner_margin(28.0).show(ui, |ui| {
                ui.set_width(560.0);
                menu::screen_heading(ui, tr_en("A new career"));
                ui.add_space(8.0);
                ui.add(egui::Label::new(RichText::new(tr_en("Start as a student pilot with $15,000. Fly the tutorials for your private licence, then take jobs from the board at whatever airport you are at: each pays by how it goes, less the fuel and the rental. Hours and reputation bring the commercial and airline licences; type ratings open the jets.")).font(menu::body()).color(menu::TEXT)).wrap());
                ui.add_space(12.0);
                menu::form_row(ui, tr_en("Pilot"), "", |ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.career.name).desired_width(260.0).hint_text(tr_en("your name")).font(menu::label()));
                });
                menu::form_row(ui, tr_en("Home airport"), "", |ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.career.base).desired_width(120.0).hint_text("KHAF").font(menu::label()));
                    let db = Db::get();
                    let found = db.find(&self.career.base).map(|i| db.apts[i as usize].name.clone());
                    ui.label(RichText::new(found.unwrap_or_else(|| tr_en("unknown code").into())).font(menu::caption()).color(menu::DIM));
                });
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if menu::choice(ui, tr_en("Begin"), true, 160.0).clicked() {
                        start = true;
                    }
                    if menu::choice(ui, tr_en("Back"), false, 120.0).clicked() {
                        back = true;
                    }
                });
            });
        });
        if start {
            let db = Db::get();
            if let Some(i) = db.find(&self.career.base) {
                self.career.base = db.apts[i as usize].ident.clone();
                if self.career.name.trim().is_empty() {
                    self.career.name = if self.profile.name.is_empty() { "Pilot".into() } else { self.profile.name.clone() };
                }
                self.career.started = true;
                let base = self.career.base.clone();
                self.career.pay(format!("{} {base}", tr_en("Started at")), 0.0);
                self.career.make_board();
                self.career.save();
            }
        }
        if back {
            self.menu.screen = crate::menu::Screen::Main;
        }
    }
}

/// one job on the board.
fn job_row(ui: &mut egui::Ui, j: &Job, active: bool) {
    ui.vertical(|ui| {
        ui.set_width((ui.available_width() - 100.0).max(200.0));
        let db = Db::get();
        let dest = db.find(&j.to).map(|i| db.apts[i as usize].name.clone()).unwrap_or_default();
        let title = match j.kind {
            Kind::Photo | Kind::Display => format!("{}  ·  {} {}", tr_en(j.kind.name()), tr_en("from and back to"), j.from),
            _ => format!("{}  ·  {} → {} {dest}", tr_en(j.kind.name()), j.from, j.to),
        };
        ui.label(RichText::new(title).font(menu::label()).color(if active { menu::ACCENT } else { Color32::WHITE }));
        let load = if j.pax > 0 { format!("{} {}", j.pax, tr_en("passengers")) } else if j.payload_kg > 0.0 { format!("{:.0} kg", j.payload_kg) } else { tr_en("empty").into() };
        let class = match j.class.as_str() { "BizJet" => "Gulfstream G650", "Airliner" => "Boeing 747", "Sst" => "Concorde", "Fighter" => "Fighter", "Recon" => "SR-71", _ => "Light aircraft" };
        ui.label(RichText::new(format!("{:.0} nm  ·  {load}  ·  {}  ·  {}  ·  {} {}", j.dist_nm, tr_en(class), money(j.pay), tr_en("within"), profile::hhmm(j.limit_s))).font(menu::caption()).color(menu::DIM));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_board_has_jobs_in_reach_and_money_reads_well() {
        let mut c = Career { started: true, base: "KHAF".into(), lics: vec![Lic::Ppl], ..Default::default() };
        c.make_board();
        assert!(!c.board.is_empty());
        for j in &c.board {
            assert!(j.pay > 0.0 && j.dist_nm > 0.0, "{j:?}");
        }
        assert_eq!(money(1_234_567.4), "$1,234,567");
        assert_eq!(money(-50.0), "-$50");
        assert!(!c.can_get(Lic::Ir, &Default::default()));
    }
}
