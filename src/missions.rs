//! guided flying: tutorials that walk through a flight a step at a time,
//! checkrides for the career's licences, scored on the landing, and short
//! missions. each starts the right aircraft at the right place and watches
//! what the pilot does; the best scores go in the profile. the landing
//! challenges are in challenge.rs.

use crate::airports::{self, Db};
use crate::{aircraft, atc, flightui, geo, menu, nav, tr_en, Game};
use egui::{Color32, RichText};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cat {
    Tutorial,
    /// a checkride for a licence (career.rs): passed at `pass_mark`.
    Exam,
    Mission,
}

/// where it starts.
#[derive(Clone, Copy, Debug)]
pub enum Start {
    Runway,
    Final,
    /// in the air, `nm` out from the airport on a bearing (deg), `ft` above it, at `kt`, heading for it.
    Air { nm: f64, bearing: f64, ft: f64, kt: f64 },
}

/// what has to happen for a step to be done.
#[derive(Clone, Copy, Debug)]
pub enum Goal {
    /// read it: done after this many seconds.
    Read(f64),
    ParkingBrake(bool),
    Master(bool),
    Ignition(bool),
    Running,
    MixtureRich,
    ThrottleAbove(f64),
    ThrottleBelow(f64),
    Airborne,
    GearUp,
    GearDown,
    FlapsAtLeast(u8),
    /// above / below the ground (ft).
    AglAbove(f64),
    AglBelow(f64),
    /// within this many feet a minute of level for this many seconds.
    Level(f64, f64),
    /// a heading (deg) within a tolerance.
    Heading(f64, f64),
    MachAbove(f64),
    AltAbove(f64),
    /// on the ground after flying, rated.
    Landed,
    Stopped,
    /// near a point (lat, lon, nm).
    Reach(f64, f64, f64),
    /// landed at an airport (ident).
    LandAt(&'static str),
    NavTuned,
    CdiCentred,
    NavFrom,
    Autopilot(bool),
    ApproachArmed,
    GlideslopeCaptured,
    ComTuned(atc::Kind),
    AtcPhase(atc::Phase),
}

pub struct Step {
    pub text: &'static str,
    pub goal: Goal,
}

pub struct Mission {
    pub id: &'static str,
    pub cat: Cat,
    pub title: &'static str,
    pub brief: &'static str,
    pub aircraft: &'static str,
    pub airport: &'static str,
    pub runway: Option<&'static str>,
    pub start: Start,
    /// local hour, and a weather preset (weather.rs `preset`).
    pub hour: f64,
    pub weather: Option<&'static str>,
    pub cold: bool,
    pub steps: &'static [Step],
}

const fn st(text: &'static str, goal: Goal) -> Step {
    Step { text, goal }
}

pub static ALL: &[Mission] = &[
    Mission { id: "t1", cat: Cat::Tutorial, title: "1. First flight", brief: "Take off from Half Moon Bay on the Californian coast, climb out and level off.", aircraft: "c172", airport: "KHAF", runway: Some("30"), start: Start::Runway, hour: 10.0, weather: Some("fair"), cold: false, steps: &[
        st("Welcome aboard. Release the parking brake with the full stop key (.).", Goal::ParkingBrake(false)),
        st("Push the throttle all the way in: hold Left Shift until it reads 100 %.", Goal::ThrottleAbove(0.95)),
        st("Keep straight with the rudder (Q and E). At 55 knots ease the nose up with S.", Goal::Airborne),
        st("Positive climb: raise the landing gear with L.", Goal::GearUp),
        st("Climb at about 80 knots to 1,500 feet above the ground.", Goal::AglAbove(1500.0)),
        st("Level off: lower the nose to the horizon and hold it there.", Goal::Level(200.0, 6.0)),
        st("Set cruise power: bring the throttle back to about 75 % with Left Ctrl.", Goal::ThrottleBelow(0.8)),
        st("Turn left to a heading of 180 with a gentle bank (A), then roll level.", Goal::Heading(180.0, 10.0)),
        st("Well flown. That is the end of the lesson.", Goal::Read(5.0)),
    ] },
    Mission { id: "t2", cat: Cat::Tutorial, title: "2. Landing", brief: "From a long final into Half Moon Bay: hold the glide path and the speed, flare and stop.", aircraft: "c172", airport: "KHAF", runway: Some("30"), start: Start::Final, hour: 16.0, weather: Some("fair"), cold: false, steps: &[
        st("You are on a 3 degree final, gear down and flaps set. Keep the aiming point steady in the windshield.", Goal::Read(8.0)),
        st("Hold about 70 knots: the throttle sets the glide path, the nose the speed. Down to 200 feet.", Goal::AglBelow(200.0)),
        st("Over the threshold close the throttle (hold Left Ctrl).", Goal::ThrottleBelow(0.1)),
        st("Flare: raise the nose gently and let it settle onto the main wheels.", Goal::Landed),
        st("Brake to a stop with B.", Goal::Stopped),
    ] },
    Mission { id: "t3", cat: Cat::Tutorial, title: "3. Engine start", brief: "Start the Cessna from cold and dark, as the checklist has it.", aircraft: "c172", airport: "KHAF", runway: Some("30"), start: Start::Runway, hour: 9.0, weather: Some("clear"), cold: true, steps: &[
        st("The aircraft is cold and dark. Set the parking brake with the full stop key (.).", Goal::ParkingBrake(true)),
        st("Battery master on: Insert.", Goal::Master(true)),
        st("Mixture full rich: Page Up.", Goal::MixtureRich),
        st("Crack the throttle open: tap Left Shift to about 10 %.", Goal::ThrottleAbove(0.05)),
        st("Magnetos on: I.", Goal::Ignition(true)),
        st("Hold the starter (O) until the engine catches.", Goal::Running),
        st("Running. The checklist (Tab, the clipboard) has the rest before taxi.", Goal::Read(6.0)),
    ] },
    Mission { id: "t4", cat: Cat::Tutorial, title: "4. Radio navigation", brief: "Track to a VOR with NAV 1 and the course needle.", aircraft: "c172", airport: "KSFO", runway: None, start: Start::Air { nm: 14.0, bearing: 200.0, ft: 3500.0, kt: 100.0 }, hour: 11.0, weather: Some("fair"), cold: false, steps: &[
        st("Open the radio page (Tab, then the mast) and tune NAV 1 to the nearest VOR.", Goal::NavTuned),
        st("Turn the OBS ([ and ]) or press Direct until the needle centres with a TO flag.", Goal::CdiCentred),
        st("Fly that course, keeping the needle centred, until the flag turns to FROM over the station.", Goal::NavFrom),
        st("Station passed. You can navigate by radio.", Goal::Read(5.0)),
    ] },
    Mission { id: "t5", cat: Cat::Tutorial, title: "5. Talking to ATC", brief: "From a towered airport: taxi, takeoff and handoff, all on the radio.", aircraft: "c172", airport: "KSQL", runway: None, start: Start::Runway, hour: 12.0, weather: Some("fair"), cold: false, steps: &[
        st("Open the ATC page (Tab, the headset) and tune the ground frequency from the list.", Goal::ComTuned(atc::Kind::Gnd)),
        st("Call ground: Request taxi.", Goal::AtcPhase(atc::Phase::Taxi)),
        st("Tune the tower and call: Ready for departure.", Goal::AtcPhase(atc::Phase::Takeoff)),
        st("Cleared for takeoff. Take off.", Goal::Airborne),
        st("Climb out. The tower hands you on to departure.", Goal::AtcPhase(atc::Phase::Enroute)),
        st("That is how a departure works on the radio.", Goal::Read(5.0)),
    ] },
    Mission { id: "t6", cat: Cat::Tutorial, title: "6. Autopilot and ILS", brief: "Let the Gulfstream's autopilot fly the ILS into San Francisco.", aircraft: "g650", airport: "KSFO", runway: Some("28R"), start: Start::Air { nm: 16.0, bearing: 110.0, ft: 4000.0, kt: 180.0 }, hour: 18.0, weather: Some("fair"), cold: false, steps: &[
        st("Engage the autopilot: F5.", Goal::Autopilot(true)),
        st("NAV 1 is on the ILS. Arm the approach mode with 9 (APR).", Goal::ApproachArmed),
        st("Slow down with the autothrottle or the throttle, and lower the gear (L).", Goal::GearDown),
        st("Flaps down two notches (F) as it slows.", Goal::FlapsAtLeast(2)),
        st("Wait for the glide slope: the autopilot captures it and starts down.", Goal::GlideslopeCaptured),
        st("At 200 feet disconnect the autopilot (F5) and land by hand.", Goal::Autopilot(false)),
        st("Touch down.", Goal::Landed),
        st("Stop on the runway.", Goal::Stopped),
    ] },
    Mission { id: "e_ppl", cat: Cat::Exam, title: "Private pilot checkride", brief: "The examiner is aboard: a circuit at Half Moon Bay, flown by hand, and a landing in the touchdown zone.", aircraft: "c172", airport: "KHAF", runway: Some("30"), start: Start::Runway, hour: 14.0, weather: Some("fair"), cold: false, steps: &[
        st("The examiner is watching. Release the brake and take off from runway 30.", Goal::Airborne),
        st("Climb straight ahead to 1,000 feet above the field.", Goal::AglAbove(1000.0)),
        st("Level off and hold your height for a few seconds.", Goal::Level(200.0, 5.0)),
        st("Fly the circuit and land back on runway 30: aim for the touchdown zone, on the centreline.", Goal::LandAt("KHAF")),
        st("Brake to a stop. The examiner marks the landing.", Goal::Stopped),
    ] },
    Mission { id: "e_ir", cat: Cat::Exam, title: "Instrument rating checkride", brief: "The ILS to San Francisco 28R by hand, in cloud, down to the minimums.", aircraft: "c172", airport: "KSFO", runway: Some("28R"), start: Start::Air { nm: 10.0, bearing: 110.0, ft: 3000.0, kt: 95.0 }, hour: 15.0, weather: Some("overcast"), cold: false, steps: &[
        st("NAV 1 is on the ILS. Keep the autopilot off: follow the localizer and the glide slope by hand.", Goal::Read(8.0)),
        st("Fly the needles down to 1,000 feet above the ground.", Goal::AglBelow(1000.0)),
        st("Stay on the glide slope to the minimums, 200 feet.", Goal::AglBelow(200.0)),
        st("Runway in sight: land.", Goal::Landed),
        st("Stop on the runway.", Goal::Stopped),
    ] },
    Mission { id: "e_cpl", cat: Cat::Exam, title: "Commercial pilot checkride", brief: "A cross-country from Half Moon Bay to San Carlos, and a landing to commercial standards.", aircraft: "c172", airport: "KHAF", runway: Some("30"), start: Start::Runway, hour: 11.0, weather: Some("fair"), cold: false, steps: &[
        st("Take off and turn east over the hills toward San Carlos (KSQL).", Goal::Airborne),
        st("Cruise at 2,500 feet or above.", Goal::AltAbove(2500.0)),
        st("Land at San Carlos: a stable approach and a touchdown on the centreline.", Goal::LandAt("KSQL")),
        st("Stop on the runway.", Goal::Stopped),
    ] },
    Mission { id: "m_golden_gate", cat: Cat::Mission, title: "Golden Gate sightseeing", brief: "Take a passenger up the coast to the Golden Gate Bridge and back to Half Moon Bay.", aircraft: "c172", airport: "KHAF", runway: Some("30"), start: Start::Runway, hour: 17.5, weather: Some("fair"), cold: false, steps: &[
        st("Take off and head north up the coast.", Goal::Airborne),
        st("Fly over the Golden Gate Bridge, below 2,000 feet.", Goal::Reach(37.8199, -122.4783, 0.8)),
        st("Head back south and land at Half Moon Bay.", Goal::LandAt("KHAF")),
        st("Stop and let your passenger out.", Goal::Stopped),
    ] },
    Mission { id: "m_mach2", cat: Cat::Mission, title: "Mach 2 over the Atlantic", brief: "Out of London in Concorde: climb, light the reheat and go supersonic over the sea.", aircraft: "concorde", airport: "EGLL", runway: Some("27R"), start: Start::Runway, hour: 11.0, weather: Some("fair"), cold: false, steps: &[
        st("Take off with the reheat lit (throttle past full, or Space).", Goal::Airborne),
        st("Gear up, nose and visor up (V), climb to 28,000 feet.", Goal::AltAbove(28_000.0)),
        st("Over the sea, accelerate through Mach 1 and climb on.", Goal::MachAbove(1.0)),
        st("Cruise climb to 50,000 feet at Mach 2.", Goal::MachAbove(1.95)),
        st("Mach 2: twice the speed of sound, on the edge of space.", Goal::Read(6.0)),
    ] },
    Mission { id: "m_blackbird", cat: Cat::Mission, title: "Blackbird at Mach 3", brief: "Take the SR-71 to 80,000 feet and Mach 3.", aircraft: "sr71", airport: "KSFO", runway: None, start: Start::Air { nm: 30.0, bearing: 60.0, ft: 25_000.0, kt: 420.0 }, hour: 13.0, weather: Some("clear"), cold: false, steps: &[
        st("Afterburners on (throttle past full or Space), climb through 40,000 feet.", Goal::AltAbove(40_000.0)),
        st("Go supersonic.", Goal::MachAbove(1.1)),
        st("Climb on and accelerate past Mach 3.", Goal::MachAbove(3.0)),
        st("80,000 feet.", Goal::AltAbove(80_000.0)),
        st("The fastest air-breathing manned aircraft ever flown.", Goal::Read(6.0)),
    ] },
];

pub fn by_id(id: &str) -> Option<&'static Mission> {
    ALL.iter().find(|m| m.id == id)
}

/// the score a checkride has to reach.
pub fn pass_mark(m: &Mission) -> u32 {
    match m.id {
        "e_heli" | "e_glider" => 60,
        "e_cpl" => 75,
        _ => 70,
    }
}

/// how a landing went.
#[derive(Clone, Copy, Debug, Default)]
pub struct Rating {
    pub fpm: f64,
    /// from the threshold (m), off the centreline (m), bank (deg).
    pub along: f64,
    pub off: f64,
    pub bank: f64,
    pub on_runway: bool,
    pub score: u32,
}

impl Rating {
    pub fn grade(&self) -> &'static str {
        match self.score {
            90.. => "A",
            75..=89 => "B",
            60..=74 => "C",
            40..=59 => "D",
            _ => "F",
        }
    }

    /// points: sink rate 40, touchdown zone 30, centreline 20, wings level 10.
    pub fn score(fpm: f64, along: f64, off: f64, bank: f64, on_runway: bool, big: bool) -> u32 {
        if !on_runway {
            return 0;
        }
        let sink = match fpm.abs() {
            v if v <= 100.0 => 40.0,
            v if v <= 200.0 => 35.0,
            v if v <= 300.0 => 28.0,
            v if v <= 450.0 => 18.0,
            v if v <= 600.0 => 8.0,
            _ => 0.0,
        };
        let (z0, z1) = if big { (150.0, 600.0) } else { (30.0, 300.0) };
        let zone = if along < 0.0 { 0.0 } else if along < z0 { 30.0 * along / z0 } else if along <= z1 { 30.0 } else { (30.0 - (along - z1) / 20.0).max(0.0) };
        let line = match off.abs() {
            v if v < 1.5 => 20.0,
            v if v < 4.0 => 14.0,
            v if v < 8.0 => 7.0,
            _ => 0.0,
        };
        let level = match bank.abs() {
            b if b < 2.0 => 10.0,
            b if b < 5.0 => 6.0,
            b if b < 8.0 => 2.0,
            _ => 0.0,
        };
        (sink + zone + line + level) as u32
    }
}

/// a mission being flown.
pub struct Run {
    pub m: &'static Mission,
    pub step: usize,
    /// when the step began (game s), how long its goal has held.
    pub since: f64,
    held: f64,
    pub rating: Option<Rating>,
    pub done: bool,
    pub failed: Option<String>,
    up_for: f64,
    ground: bool,
    /// the sink rate while still in the air (m/s), for the touchdown.
    vs_air: f64,
    /// the steps' first second: starting on the ground after a teleport is not a landing.
    settle: f64,
    /// the pilot's live weather and real time switches before the mission set its own weather.
    wx_was: Option<(bool, bool)>,
}

impl Run {
    pub fn new(m: &'static Mission, now: f64) -> Self {
        Self { m, step: 0, since: now, held: 0.0, rating: None, done: false, failed: None, up_for: 0.0, ground: true, vs_air: 0.0, settle: 3.0, wx_was: None }
    }

}

/// a point `nm` out from (lat, lon) on a bearing (deg).
fn offset(lat: f64, lon: f64, nm: f64, bearing: f64) -> (f64, f64) {
    let d = nm * 1852.0 / geo::R;
    let (p1, l1, b) = (lat.to_radians(), lon.to_radians(), bearing.to_radians());
    let p2 = (p1.sin() * d.cos() + p1.cos() * d.sin() * b.cos()).asin();
    let l2 = l1 + (b.sin() * d.sin() * p1.cos()).atan2(d.cos() - p1.sin() * p2.sin());
    (p2.to_degrees(), l2.to_degrees())
}

impl Game {
    /// takes the aircraft there and starts watching.
    pub(crate) fn start_mission(&mut self, m: &'static Mission) {
        let db = Db::get();
        let Some(idx) = db.find(m.airport) else { return };
        if let Some(spec) = aircraft::by_key(m.aircraft) {
            self.set_aircraft(spec);
            self.menu.aircraft = aircraft::index_of(spec);
        }
        self.menu.cold = m.cold;
        self.menu.hour = m.hour;
        // (an earlier mission's weather is not the pilot's own: keep the first one's record)
        let was = self.mission.as_ref().and_then(|r| r.wx_was).unwrap_or((self.wx.live, self.wx.real_time));
        if let Some(w) = m.weather {
            self.wx.live = false;
            self.wx.real_time = false;
            self.wx.preset(w);
        }
        self.set_pause_menu(false);
        self.info_open = false;
        self.atc.reset();
        self.plan.clear();
        match m.start {
            Start::Runway | Start::Final => self.start_airport(idx, m.runway.map(String::from), matches!(m.start, Start::Final), false),
            Start::Air { nm, bearing, ft, kt } => {
                let a = &db.apts[idx as usize];
                let (lat, lon) = offset(a.lat, a.lon, nm, bearing);
                let heading = (bearing + 180.0).rem_euclid(360.0).to_radians();
                self.utc = (m.hour - a.lon / 15.0).rem_euclid(24.0);
                if self.mode == crate::Mode::Menu {
                    self.mode = crate::Mode::Fly;
                }
                self.respawn_at(lat.to_radians(), lon.to_radians(), a.elev as f64 * 0.3048 + ft * 0.3048, heading, kt / 1.943_84);
                // the ils for the runway, as the crew would have it.
                if let Some(rw) = m.runway {
                    if let Some((s, _)) = nav::near(a.lat, a.lon, 30_000.0, nav::Kind::Ils).into_iter().find(|(s, _)| s.name == a.ident && airports::norm_ident(&s.ident) == airports::norm_ident(rw)) {
                        self.radios.tune_nav(s, self.plane.pos);
                    }
                }
            }
        }
        let mut run = Run::new(m, self.time);
        run.wx_was = m.weather.map(|_| was);
        self.mission = Some(run);
    }

    /// stops the mission, the pilot's weather switches as they were.
    pub(crate) fn end_mission(&mut self) {
        if let Some((live, real)) = self.mission.take().and_then(|r| r.wx_was) {
            (self.wx.live, self.wx.real_time) = (live, real);
        }
    }

    fn goal_met(&self, g: Goal, held: f64, run: &Run) -> bool {
        let p = &self.plane;
        let (la, lo, h) = geo::pos_to_lla(p.pos);
        let agl_ft = self.world.ground(la, lo).map_or(h, |gr| h - gr.h) * 3.28084;
        let (e, n, u) = geo::enu(p.pos);
        let f = p.forward();
        let hdg = f.dot(e).atan2(f.dot(n)).to_degrees().rem_euclid(360.0);
        let reading = self.radios.nav.as_ref().map(|st| nav::nav_reading(st, self.radios.obs, p.pos));
        match g {
            Goal::Read(s) => self.time - run.since > s,
            Goal::ParkingBrake(on) => p.parking_brake == on,
            Goal::Master(on) => p.sys.elec.master == on,
            Goal::Ignition(on) => p.ignition == on,
            Goal::Running => p.running,
            Goal::MixtureRich => p.mixture > 0.9,
            Goal::ThrottleAbove(t) => p.throttle >= t,
            Goal::ThrottleBelow(t) => p.throttle <= t,
            Goal::Airborne => !p.on_ground && agl_ft > 30.0,
            Goal::GearUp => !p.gear_down || p.spec.fixed_gear(),
            Goal::GearDown => p.gear_down,
            Goal::FlapsAtLeast(k) => p.flap_set >= k,
            Goal::AglAbove(ft) => agl_ft >= ft,
            Goal::AglBelow(ft) => agl_ft <= ft && !p.on_ground,
            Goal::Level(fpm, secs) => (p.vel.dot(u) * 196.85).abs() < fpm && held >= secs,
            Goal::Heading(want, tol) => ((hdg - want + 540.0).rem_euclid(360.0) - 180.0).abs() < tol,
            Goal::MachAbove(m) => p.mach() >= m,
            Goal::AltAbove(ft) => h * 3.28084 >= ft,
            Goal::Landed => run.rating.is_some(),
            Goal::Stopped => p.on_ground && p.vel.length() < 1.0,
            Goal::Reach(lat, lon, nm) => crate::flightplan::dist((la.to_degrees(), lo.to_degrees()), (lat, lon)) / 1852.0 < nm,
            Goal::LandAt(id) => p.on_ground && run.up_for > 20.0 && Db::get().near(la.to_degrees(), lo.to_degrees(), 4_000.0).iter().any(|(i, _)| Db::get().apts[*i as usize].ident == id),
            Goal::NavTuned => self.radios.nav.is_some(),
            Goal::CdiCentred => reading.is_some_and(|r| r.live && r.to == Some(true) && r.cdi.abs() < 0.25),
            Goal::NavFrom => reading.is_some_and(|r| r.live && r.to == Some(false)),
            Goal::Autopilot(on) => self.ap.on == on,
            Goal::ApproachArmed => self.ap.on && self.ap.lat == crate::autopilot::Lat::Apr,
            Goal::GlideslopeCaptured => self.ap.gs_cap,
            Goal::ComTuned(k) => self.atc.tuned(&self.atc_ctx()).is_some_and(|f| f.kind == k),
            Goal::AtcPhase(ph) => self.atc.phase == ph,
        }
    }

    /// once a step: the touchdown rated, the step's goal checked, on to the next.
    pub(crate) fn mission_step(&mut self, dt: f64) {
        let Some(mut run) = self.mission.take() else { return };
        if run.done || run.failed.is_some() {
            self.mission = Some(run);
            return;
        }
        run.settle -= dt;
        let p = &self.plane;
        if !p.on_ground {
            run.up_for += dt;
            run.vs_air = p.vel.dot(p.pos.normalize());
        }
        // the touchdown: sink rate, where on the runway, how far off the centreline.
        if p.on_ground && !run.ground && run.up_for > 6.0 && run.rating.is_none() && run.settle < 0.0 {
            let fpm = run.vs_air * 196.85;
            let (e, n, u) = geo::enu(p.pos);
            let right = p.rot * glam::DVec3::X;
            let bank = right.dot(u).asin().to_degrees();
            let f = p.forward();
            let mut along = -1.0;
            let mut off = 99.0;
            let mut on_runway = false;
            if let Some(field) = self.world.airfield_near(p.pos, 6_000.0) {
                let g = &field.geo;
                let lp = g.local(p.pos);
                if let Some(ri) = g.on_runway(lp, 2.0) {
                    let r = &g.runways[ri];
                    let (s, x) = r.local(lp);
                    let fwd = glam::Vec2::new(f.dot(e) as f32, f.dot(n) as f32);
                    // landing toward b, or toward a.
                    if fwd.dot(r.dir) >= 0.0 {
                        along = (s - r.thr[0]) as f64;
                        off = x as f64;
                    } else {
                        along = (r.len - s - r.thr[1]) as f64;
                        off = -x as f64;
                    }
                    on_runway = true;
                }
            }
            let big = self.spec.is_jet();
            let score = Rating::score(fpm, along, off, bank, on_runway, big);
            run.rating = Some(Rating { fpm, along, off, bank, on_runway, score });
        }
        run.ground = p.on_ground;
        if self.crashed {
            run.failed = Some(tr_en("The aircraft crashed.").into());
            self.mission = Some(run);
            return;
        }
        if let Some(step) = run.m.steps.get(run.step) {
            let ok_now = self.goal_met(step.goal, run.held, &run);
            // a level-off holds a while, the rest just have to happen.
            run.held = match step.goal {
                Goal::Level(fpm, _) => {
                    let (_, _, u) = geo::enu(self.plane.pos);
                    if (self.plane.vel.dot(u) * 196.85).abs() < fpm { run.held + dt } else { 0.0 }
                }
                _ => 0.0,
            };
            if ok_now && run.settle < 0.0 {
                run.step += 1;
                run.since = self.time;
                run.held = 0.0;
                if run.step >= run.m.steps.len() {
                    run.done = true;
                    let m = run.m;
                    let first = !self.profile.best.contains_key(m.id);
                    if m.cat == Cat::Exam {
                        // a checkride is the landing's score, and has to make the mark.
                        let s = run.rating.map_or(0, |r| r.score);
                        if s < pass_mark(m) {
                            run.done = false;
                            run.failed = Some(format!("{} {s} / 100, {} {}.", tr_en("The landing scored"), tr_en("the pass mark is"), pass_mark(m)));
                        } else {
                            let best = self.profile.best.entry(m.id.to_string()).or_insert(0);
                            *best = (*best).max(s);
                            self.mission = Some(run);
                            self.keep_profile();
                            self.award(&format!("{}: {}", tr_en("Passed"), tr_en(m.title)), if first { 500 } else { 50 });
                            return;
                        }
                    } else {
                        let s = run.rating.map_or(100, |r| r.score.max(60));
                        let best = self.profile.best.entry(m.id.to_string()).or_insert(0);
                        *best = (*best).max(s);
                        self.mission = Some(run);
                        self.keep_profile();
                        let xp = match (m.cat, first) {
                            (Cat::Tutorial, true) => 300,
                            (Cat::Mission, true) => 400,
                            _ => 50,
                        };
                        self.award(&format!("{}: {}", tr_en("Complete"), tr_en(m.title)), xp);
                        self.sweep_badges();
                        return;
                    }
                }
            }
        }
        self.mission = Some(run);
    }

    /// the mission card: what it is, the steps done and the one to do, and the result at the end.
    pub(crate) fn mission_card(&mut self, ctx: &egui::Context) {
        let Some(run) = self.mission.as_ref() else { return };
        let mut close = false;
        let mut again = false;
        let m = run.m;
        let (step, n, done, failed, rating) = (run.step, m.steps.len(), run.done, run.failed.clone(), run.rating);
        let top = if self.hud_on() { flightui::right_top(ctx) } else { 24.0 };
        egui::Area::new(egui::Id::unique("mission_card")).anchor(egui::Align2::RIGHT_TOP, [-14.0, top]).show(ctx, |ui| {
            flightui::panel(ui, flightui::glass().inner_margin(egui::Margin::symmetric(14, 12)), |ui| {
                ui.set_width(300.0);
                ui.spacing_mut().item_spacing.y = 4.0;
                // the title, and the step it is on beside it.
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tr_en(m.title)).font(menu::semibold(16.0)).color(Color32::WHITE));
                    if !done && failed.is_none() {
                        menu::at_right(ui, "mission_step", |ui| {
                            ui.label(RichText::new(format!("{} / {n}", step + 1)).font(menu::label()).color(flightui::SOFT));
                        });
                    }
                });
                ui.add_space(4.0);
                if let Some(why) = &failed {
                    result_line(ui, tr_en("Not passed"), menu::BAD);
                    ui.add(egui::Label::new(RichText::new(why).font(menu::body()).color(menu::TEXT)).wrap());
                } else if done {
                    result_line(ui, if m.cat == Cat::Exam { tr_en("Passed") } else { tr_en("Complete") }, menu::OK);
                } else if let Some(s) = m.steps.get(step) {
                    // only the step to do now.
                    ui.add(egui::Label::new(RichText::new(tr_en(s.text)).font(menu::medium(15.0)).color(Color32::WHITE)).wrap());
                    ui.add_space(6.0);
                    flightui::meter(ui, step as f32 / n.max(1) as f32, flightui::LIT);
                }
                if let Some(r) = rating {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{}", r.score)).font(menu::bold(30.0)).color(if r.score >= 75 { menu::ACCENT } else { Color32::WHITE }));
                        ui.vertical(|ui| {
                            ui.label(RichText::new(format!("{} {}", tr_en("Landing"), r.grade())).font(menu::semibold(14.0)).color(Color32::WHITE));
                            let along = if r.on_runway { format!("{:.0} m {}", r.along, tr_en("past the threshold")) } else { tr_en("off the runway").into() };
                            flightui::note(ui, &format!("{:.0} ft/min  ·  {along}", r.fpm.abs()));
                            flightui::note(ui, &format!("{:.1} m {}  ·  {:.1}° {}", r.off.abs(), tr_en("off centre"), r.bank.abs(), tr_en("bank")));
                        });
                    });
                    if m.cat == Cat::Exam {
                        flightui::note(ui, &format!("{} {}", tr_en("Pass mark"), pass_mark(m)));
                    }
                }
                ui.add_space(6.0);
                if done || failed.is_some() {
                    ui.horizontal(|ui| {
                        if flightui::key(ui, tr_en("Try again"), false, 100.0, 28.0).clicked() {
                            again = true;
                        }
                        if flightui::key(ui, tr_en("Close"), false, 80.0, 28.0).clicked() {
                            close = true;
                        }
                    });
                } else if flightui::text_link(ui, tr_en("Abandon")).clicked() {
                    close = true;
                }
            });
        });
        if again {
            self.start_mission(m);
        } else if close {
            self.end_mission();
        }
    }
}

/// a big word for how it ended.
fn result_line(ui: &mut egui::Ui, text: &str, col: Color32) {
    ui.label(RichText::new(text.to_uppercase()).font(menu::bold(24.0)).color(col));
}

/// a tick mark.
pub fn tick(p: &egui::Painter, c: egui::Pos2, s: f32, col: Color32) {
    p.add(egui::Shape::line(vec![c + egui::vec2(-s, 0.0), c + egui::vec2(-s * 0.3, s * 0.7), c + egui::vec2(s, -s * 0.6)], egui::Stroke::new(1.8, col)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mission_starts_somewhere_real_in_a_real_aircraft() {
        let db = Db::get();
        for m in ALL {
            assert!(db.find(m.airport).is_some(), "{}: {}", m.id, m.airport);
            assert!(aircraft::by_key(m.aircraft).is_some(), "{}: {}", m.id, m.aircraft);
            if let Some(rw) = m.runway {
                let a = &db.apts[db.find(m.airport).unwrap() as usize];
                assert!(a.rwys.iter().any(|r| r.ident.iter().any(|i| airports::norm_ident(i) == airports::norm_ident(rw))), "{}: runway {rw}", m.id);
            }
            assert!(!m.steps.is_empty());
        }
    }

    #[test]
    fn a_butter_landing_scores_full_marks() {
        assert_eq!(Rating::score(-80.0, 300.0, 0.5, 1.0, true, true), 100);
        assert_eq!(Rating::score(-900.0, 300.0, 0.5, 1.0, true, true), 60);
        assert_eq!(Rating::score(-80.0, 300.0, 0.5, 1.0, false, true), 0);
    }
}
