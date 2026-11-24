//! pilot progression: experience for what is flown, levels and ranks, and
//! achievements. everything counts, free flight as much as the career: a
//! take off, each landing by its grade, time and distance in the air, a new
//! airport or type, the tutorials, exams, challenges, missions and jobs.
//! what was earned shows for a few seconds at the top of the screen.

use crate::flightlog::{Flight, Grade, Landing, Outcome};
use crate::{aircraft, geo, menu, tr_en, Game};
use egui::{pos2, vec2, Align2, Color32, Rect, Stroke};

/// the highest level.
pub const MAX_LEVEL: u32 = 99;

/// experience from one level to the next.
pub fn step_xp(level: u32) -> u64 {
    let n = level.saturating_sub(1) as u64;
    300 + 120 * n + 4 * n * n
}

/// a level, how far into it and how much it takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Level {
    pub n: u32,
    pub into: u64,
    pub span: u64,
}

impl Level {
    pub fn frac(&self) -> f32 {
        if self.span == 0 { 1.0 } else { (self.into as f64 / self.span as f64) as f32 }
    }
}

pub fn level(xp: u64) -> Level {
    let mut n = 1;
    let mut left = xp;
    while n < MAX_LEVEL && left >= step_xp(n) {
        left -= step_xp(n);
        n += 1;
    }
    Level { n, into: left, span: if n >= MAX_LEVEL { 0 } else { step_xp(n) } }
}

/// the rank a level carries.
pub fn rank(n: u32) -> &'static str {
    match n {
        0..=2 => "Cadet",
        3..=5 => "Aviator",
        6..=9 => "Flight officer",
        10..=14 => "First officer",
        15..=19 => "Senior first officer",
        20..=29 => "Captain",
        30..=39 => "Senior captain",
        40..=54 => "Commander",
        55..=74 => "Ace",
        _ => "Legend",
    }
}

/// experience for a landing of a grade.
pub fn landing_xp(g: Grade) -> u64 {
    match g {
        Grade::Butter => 150,
        Grade::Smooth => 100,
        Grade::Firm => 60,
        Grade::Hard => 25,
        Grade::VeryHard => 0,
    }
}

/// experience for a flight's time and distance: 2 a minute, 1 every 2 nm, up to 1,500.
pub fn flight_xp(f: &Flight) -> u64 {
    if f.outcome == Outcome::Crashed {
        return 0;
    }
    ((f.air_time / 30.0) + f.distance / 1852.0 / 2.0).min(1500.0) as u64
}

/// an achievement.
pub struct Badge {
    pub id: &'static str,
    pub name: &'static str,
    pub about: &'static str,
    pub xp: u64,
}

const fn b(id: &'static str, name: &'static str, about: &'static str, xp: u64) -> Badge {
    Badge { id, name, about, xp }
}

pub static BADGES: &[Badge] = &[
    b("first_flight", "First flight", "Take off for the first time.", 100),
    b("first_landing", "Down safely", "Land for the first time.", 100),
    b("butter", "Butter", "A touchdown under 60 feet a minute.", 150),
    b("greaser", "Greaser", "Ten butter landings.", 500),
    b("landings_25", "Circuit regular", "Twenty-five landings.", 250),
    b("landings_100", "Hundred landings", "One hundred landings.", 800),
    b("hours_10", "Ten hours", "Ten hours in the air.", 300),
    b("hours_100", "Hundred hours", "One hundred hours in the air.", 1500),
    b("airports_10", "Explorer", "Land at ten different airports.", 400),
    b("airports_50", "Globetrotter", "Land at fifty different airports.", 1500),
    b("long_haul", "Long haul", "One flight of 1,000 nautical miles.", 500),
    b("night", "Night flyer", "Land between dusk and dawn.", 250),
    b("sound_barrier", "Sound barrier", "Fly faster than Mach 1.", 300),
    b("mach3", "Blackbird", "Fly at Mach 3.", 600),
    b("edge", "Edge of space", "Climb above 70,000 feet.", 500),
    b("rotor", "Rotorhead", "Fly the helicopter.", 150),
    b("soaring", "Silent wings", "Land the glider after fifteen minutes aloft.", 250),
    b("all_types", "Type collector", "Fly every built-in aircraft.", 1000),
    b("graduate", "Graduate", "Finish every tutorial.", 500),
    b("precision", "Precision", "Score 90 or more in a landing challenge.", 400),
    b("challenger", "Challenger", "Pass every landing challenge.", 1000),
    b("ppl", "Private pilot", "Earn the private pilot licence.", 300),
    b("cpl", "Commercial pilot", "Earn the commercial pilot licence.", 600),
    b("atpl", "Airline captain", "Earn the airline transport pilot licence.", 1200),
    b("first_job", "Open for business", "Finish a first career job.", 150),
    b("flawless", "Flawless", "A five star career job.", 400),
    b("owner", "Aircraft owner", "Buy an aircraft.", 400),
    b("chief_pilot", "Chief pilot", "Hire a pilot for your company.", 400),
    b("millionaire", "Millionaire", "Have $1,000,000 in the bank.", 1500),
];

pub fn badge(id: &str) -> Option<&'static Badge> {
    BADGES.iter().find(|b| b.id == id)
}

/// what shows at the top of the screen for a while.
#[derive(Clone, Debug)]
pub enum Toast {
    Xp { what: String, xp: u64 },
    Level { n: u32 },
    Badge { id: &'static str },
}

/// the built-in types a collector has to fly (not the rocket).
fn collectable() -> Vec<&'static str> {
    aircraft::BUILTIN.iter().filter(|s| s.rocket().is_none()).map(|s| s.name).collect()
}

impl Game {
    /// experience for something done, shown and kept.
    pub(crate) fn award(&mut self, what: &str, xp: u64) {
        if xp == 0 {
            return;
        }
        let before = level(self.profile.xp).n;
        self.profile.add_xp(what, xp);
        self.toast(Toast::Xp { what: what.to_string(), xp });
        let after = level(self.profile.xp).n;
        if after > before {
            self.toast(Toast::Level { n: after });
        }
        self.keep_profile();
    }

    /// an achievement, once.
    pub(crate) fn unlock(&mut self, id: &'static str) {
        let Some(b) = badge(id) else { return };
        if self.profile.has_badge(id) {
            return;
        }
        self.profile.badges.insert(id.to_string(), crate::profile::now_unix());
        self.toast(Toast::Badge { id });
        self.award(&format!("{} {}", tr_en("Achievement:"), tr_en(b.name)), b.xp);
    }

    fn toast(&mut self, t: Toast) {
        self.toasts.push((t, self.real));
        if self.toasts.len() > 6 {
            self.toasts.remove(0);
        }
    }

    pub(crate) fn keep_profile(&self) {
        if self.logbook_saves {
            self.profile.save();
        }
    }

    /// achievements that come from the totals: hours, landings, airports, types, money.
    pub(crate) fn sweep_badges(&mut self) {
        let t = crate::profile::totals(&self.logbook.flights);
        let p = &self.profile;
        let mut got: Vec<&'static str> = Vec::new();
        if t.landings >= 25 { got.push("landings_25"); }
        if t.landings >= 100 { got.push("landings_100"); }
        if t.air_s >= 10.0 * 3600.0 { got.push("hours_10"); }
        if t.air_s >= 100.0 * 3600.0 { got.push("hours_100"); }
        if p.airports.len() >= 10 { got.push("airports_10"); }
        if p.airports.len() >= 50 { got.push("airports_50"); }
        if p.butters >= 10 { got.push("greaser"); }
        if collectable().iter().all(|n| p.types.contains(*n)) { got.push("all_types"); }
        if crate::missions::ALL.iter().filter(|m| m.cat == crate::missions::Cat::Tutorial).all(|m| p.best.contains_key(m.id)) { got.push("graduate"); }
        if crate::challenge::ALL.iter().all(|c| self.challenge_records.top(c.id).is_some()) { got.push("challenger"); }
        if crate::challenge::ALL.iter().any(|c| self.challenge_records.top(c.id).is_some_and(|s| s >= 90)) { got.push("precision"); }
        if self.career.money >= 1_000_000.0 { got.push("millionaire"); }
        for id in got {
            self.unlock(id);
        }
    }

    /// a take off: experience, the type flown, the first flight.
    pub(crate) fn pilot_takeoff(&mut self) {
        let name = self.spec.name;
        if self.profile.types.insert(name.to_string()) {
            self.award(&format!("{} {}", tr_en("First flight in the"), name), 100);
        }
        if self.spec.rotor.is_some() {
            self.unlock("rotor");
        }
        self.unlock("first_flight");
        self.award(tr_en("Take-off"), 25);
    }

    /// a graded landing: experience by the grade, a new airport, night, butter.
    pub(crate) fn pilot_landed(&mut self, l: &Landing) {
        let xp = landing_xp(l.grade);
        self.award(&format!("{} {}", l.grade.text(), tr_en("landing")), xp);
        self.unlock("first_landing");
        if l.grade == Grade::Butter {
            self.profile.butters += 1;
            self.unlock("butter");
        }
        if let Some(a) = l.airport.clone().filter(|_| l.on_runway.is_some()) {
            if self.profile.airports.insert(a.clone()) {
                self.award(&format!("{} {a}", tr_en("New airport:")), 150);
            }
        }
        let lon = geo::pos_to_lla(self.plane.pos).1.to_degrees();
        let local = (self.utc + lon / 15.0).rem_euclid(24.0);
        if !(6.5..=19.0).contains(&local) {
            self.profile.night_landings += 1;
            self.unlock("night");
        }
        self.sweep_badges();
    }

    /// a flight logged: experience for its time and distance.
    pub(crate) fn pilot_flight_done(&mut self, f: &Flight) {
        let xp = flight_xp(f);
        if xp > 0 {
            self.award(&format!("{} {}", tr_en("Flight time"), crate::profile::hhmm(f.air_time)), xp);
        }
        if f.distance / 1852.0 >= 1000.0 && f.outcome != Outcome::Crashed {
            self.unlock("long_haul");
        }
        if f.outcome == Outcome::Landed && f.air_time >= 900.0 && self.spec.keys.contains(&"ask21") {
            self.unlock("soaring");
        }
        self.sweep_badges();
    }

    /// the fastest and highest, looked at now and then in the air.
    pub(crate) fn pilot_tick(&mut self) {
        if self.plane.on_ground || self.crashed {
            return;
        }
        let m = self.plane.mach();
        let ft = geo::pos_to_lla(self.plane.pos).2 * 3.28084;
        self.profile.max_mach = self.profile.max_mach.max(m);
        self.profile.max_ft = self.profile.max_ft.max(ft);
        if m > 1.0 {
            self.unlock("sound_barrier");
        }
        if m > 2.95 {
            self.unlock("mach3");
        }
        if ft > 70_000.0 {
            self.unlock("edge");
        }
    }

    /// the awards of the last few seconds, at the top of the screen.
    pub(crate) fn toasts_ui(&mut self, ctx: &egui::Context) {
        const LIFE: f64 = 4.5;
        let now = self.real;
        self.toasts.retain(|(_, t0)| now - *t0 < LIFE);
        if self.toasts.is_empty() {
            return;
        }
        let screen = ctx.content_rect();
        // under the notices at the top centre, if any.
        let top = if self.mode == crate::Mode::Fly && self.hud_on() {
            crate::flightui::shown(ctx, "notices", egui::Order::Middle).map_or(crate::flightui::HEADING_TOP + crate::flightui::HEADING_H + 10.0, |r| r.bottom() + 8.0)
        } else {
            24.0
        };
        let toasts = self.toasts.clone();
        egui::Area::new(egui::Id::unique("xp_toasts")).order(egui::Order::Tooltip).fixed_pos(screen.min).interactable(false).show(ctx, |ui| {
            let p = ui.painter();
            let mut y = screen.top() + top;
            for (t, t0) in toasts.iter().rev().take(4) {
                let age = now - t0;
                // in, held, out.
                let a = ((age / 0.25).min(1.0) * ((LIFE - age) / 0.6).clamp(0.0, 1.0)) as f32;
                let slide = (1.0 - (age / 0.25).min(1.0) as f32) * -10.0;
                let (h, w) = match t {
                    Toast::Level { .. } => (64.0, 380.0),
                    Toast::Badge { .. } => (56.0, 380.0),
                    Toast::Xp { .. } => (34.0, 300.0),
                };
                let r = Rect::from_center_size(pos2(screen.center().x, y + h * 0.5 + slide), vec2(w, h));
                let fill = Color32::from_rgba_premultiplied(9, 12, 17, (225.0 * a) as u8);
                p.rect_filled(r, menu::R_PANEL, fill);
                p.rect_stroke(r, menu::R_PANEL, Stroke::new(1.0, Color32::from_white_alpha((30.0 * a) as u8)), egui::StrokeKind::Inside);
                let acc = menu::ACCENT.gamma_multiply(a);
                let white = Color32::WHITE.gamma_multiply(a);
                let dim = menu::DIM.gamma_multiply(a);
                match t {
                    Toast::Xp { what, xp } => {
                        p.text(pos2(r.left() + 14.0, r.center().y), Align2::LEFT_CENTER, format!("+{xp} XP"), menu::bold(17.0), acc);
                        let g = menu::one_line(p, what, menu::label(), white, r.width() - 110.0);
                        p.galley(pos2(r.left() + 92.0, r.center().y - g.size().y * 0.5), g, white);
                    }
                    Toast::Level { n } => {
                        p.rect_filled(Rect::from_min_size(r.min, vec2(4.0, r.height())), 0.0, acc);
                        level_ring(p, pos2(r.left() + 40.0, r.center().y), 22.0, 1.0, *n, a);
                        p.text(pos2(r.left() + 76.0, r.center().y - 10.0), Align2::LEFT_CENTER, tr_en("LEVEL UP"), menu::bold(20.0), acc);
                        p.text(pos2(r.left() + 76.0, r.center().y + 13.0), Align2::LEFT_CENTER, format!("{} {n}  ·  {}", tr_en("Level"), tr_en(rank(*n))), menu::label(), white);
                    }
                    Toast::Badge { id } => {
                        let bd = badge(id);
                        medal(p, pos2(r.left() + 32.0, r.center().y), 17.0, true, a);
                        p.text(pos2(r.left() + 62.0, r.center().y - 10.0), Align2::LEFT_CENTER, tr_en("ACHIEVEMENT"), menu::semibold(12.0), dim);
                        p.text(pos2(r.left() + 62.0, r.center().y + 9.0), Align2::LEFT_CENTER, bd.map_or("", |b| tr_en(b.name)), menu::semibold(17.0), white);
                    }
                }
                y += h + 8.0;
            }
        });
    }
}

/// a level in a ring that fills with the way to the next one.
pub fn level_ring(p: &egui::Painter, c: egui::Pos2, r: f32, frac: f32, n: u32, alpha: f32) {
    let track = Color32::from_white_alpha((26.0 * alpha) as u8);
    p.circle_stroke(c, r, Stroke::new(3.0, track));
    let frac = frac.clamp(0.0, 1.0);
    if frac > 0.0 {
        let a0 = -std::f32::consts::FRAC_PI_2;
        let steps = (48.0 * frac).ceil().max(2.0) as usize;
        let pts: Vec<egui::Pos2> = (0..=steps).map(|i| {
            let a = a0 + std::f32::consts::TAU * frac * i as f32 / steps as f32;
            c + vec2(a.cos(), a.sin()) * r
        }).collect();
        p.add(egui::Shape::line(pts, Stroke::new(3.0, menu::ACCENT.gamma_multiply(alpha))));
    }
    p.text(c, Align2::CENTER_CENTER, n.to_string(), menu::bold(r * 0.95), Color32::WHITE.gamma_multiply(alpha));
}

/// an achievement's medal: lit when earned.
pub fn medal(p: &egui::Painter, c: egui::Pos2, r: f32, lit: bool, alpha: f32) {
    let col = if lit { menu::ACCENT } else { Color32::from_white_alpha(60) }.gamma_multiply(alpha);
    // the ribbon, then the disc and a star in it.
    let rib = if lit { Color32::from_rgb(90, 120, 200) } else { Color32::from_white_alpha(30) }.gamma_multiply(alpha);
    p.add(egui::Shape::convex_polygon(vec![c + vec2(-r * 0.55, -r * 1.1), c + vec2(-r * 0.15, -r * 1.1), c + vec2(r * 0.1, -r * 0.4), c + vec2(-r * 0.3, -r * 0.4)], rib, Stroke::NONE));
    p.add(egui::Shape::convex_polygon(vec![c + vec2(r * 0.15, -r * 1.1), c + vec2(r * 0.55, -r * 1.1), c + vec2(r * 0.3, -r * 0.4), c + vec2(-r * 0.1, -r * 0.4)], rib.gamma_multiply(0.8), Stroke::NONE));
    p.circle_filled(c + vec2(0.0, r * 0.15), r * 0.72, if lit { Color32::from_rgb(60, 48, 8).gamma_multiply(alpha) } else { Color32::from_white_alpha(12) });
    p.circle_stroke(c + vec2(0.0, r * 0.15), r * 0.72, Stroke::new(2.0, col));
    star(p, c + vec2(0.0, r * 0.15), r * 0.38, col);
}

/// a five pointed star, filled.
pub fn star(p: &egui::Painter, c: egui::Pos2, r: f32, col: Color32) {
    let pts: Vec<egui::Pos2> = (0..10).map(|i| {
        let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / 5.0;
        let rr = if i % 2 == 0 { r } else { r * 0.45 };
        c + vec2(a.cos(), a.sin()) * rr
    }).collect();
    // a star is not convex: a fan of triangles from the middle.
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(c, col);
    for q in &pts {
        mesh.colored_vertex(*q, col);
    }
    for i in 0..10u32 {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % 10);
    }
    p.add(mesh);
}

/// stars out of five, `n` of them lit.
pub fn stars(p: &egui::Painter, left_centre: egui::Pos2, r: f32, n: u8) {
    for i in 0..5u8 {
        let c = left_centre + vec2(i as f32 * r * 2.4, 0.0);
        star(p, c, r, if i < n { menu::ACCENT } else { Color32::from_white_alpha(36) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_climb_and_stop_at_the_top() {
        assert_eq!(level(0), Level { n: 1, into: 0, span: 300 });
        assert_eq!(level(299).n, 1);
        assert_eq!(level(300), Level { n: 2, into: 0, span: 424 });
        assert_eq!(level(u64::MAX / 2).n, MAX_LEVEL);
        assert_eq!(level(u64::MAX / 2).span, 0);
        // a few weeks of flying, not a lifetime, reach the middle ranks.
        let mut xp = 0;
        for n in 1..20 {
            xp += step_xp(n);
        }
        assert!(xp < 80_000, "{xp}");
        assert_eq!(rank(1), "Cadet");
        assert_eq!(rank(20), "Captain");
        assert_eq!(rank(99), "Legend");
    }

    #[test]
    fn every_badge_is_unique_and_named() {
        for (i, a) in BADGES.iter().enumerate() {
            assert!(!a.name.is_empty() && !a.about.is_empty() && a.xp > 0);
            assert!(BADGES.iter().skip(i + 1).all(|b| b.id != a.id), "{}", a.id);
        }
    }

    #[test]
    fn landings_and_flights_give_experience() {
        assert!(landing_xp(Grade::Butter) > landing_xp(Grade::Firm));
        assert_eq!(landing_xp(Grade::VeryHard), 0);
        let f = Flight { date: 0, aircraft: "x".into(), from: None, to: None, air_time: 3600.0, distance: 200.0 * 1852.0, max_alt: 0.0, landings: Vec::new(), outcome: Outcome::Landed };
        assert_eq!(flight_xp(&f), 220);
        assert_eq!(flight_xp(&Flight { outcome: Outcome::Crashed, ..f }), 0);
    }
}
