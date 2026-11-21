//! the pilot profile: a logbook of flights and the hours in it.
//! time counts from lift off to the last touchdown, in sim time, so a pause
//! stops the clock. saved next to the settings as profile.json, every couple
//! of minutes during a flight so a crash of the game loses little.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// one flight in the logbook.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    /// seconds since 1970, utc, when it started.
    pub unix: u64,
    pub aircraft: String,
    /// icao code, or a place name.
    pub from: String,
    pub to: String,
    /// seconds in the air.
    pub air_s: f64,
    /// distance flown, nm.
    pub dist_nm: f64,
    pub landings: u32,
    pub max_alt_ft: f64,
    #[serde(default)]
    pub crashed: bool,
    /// still flying when last saved.
    #[serde(default)]
    pub open: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    /// unix seconds the profile began.
    pub since: u64,
    pub log: Vec<Entry>,
}

/// sums over the logbook.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Totals {
    pub air_s: f64,
    pub flights: usize,
    pub landings: u32,
    pub dist_nm: f64,
    pub longest_s: f64,
    pub highest_ft: f64,
    /// aircraft by hours, most first: name, seconds, flights.
    pub by_type: Vec<(String, f64, usize)>,
    /// airports by visits, most first.
    pub airports: Vec<(String, usize)>,
}

pub fn now_unix() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// yyyy-mm-dd hh:mm utc from unix seconds.
pub fn date(unix: u64) -> String {
    let days = (unix / 86_400) as i64;
    let secs = unix % 86_400;
    // civil from days, howard hinnant's algorithm.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", secs / 3600, (secs % 3600) / 60)
}

/// hours and minutes, like 12:05.
pub fn hhmm(secs: f64) -> String {
    let m = (secs.max(0.0) / 60.0).round() as u64;
    format!("{}:{:02}", m / 60, m % 60)
}

/// decimal hours, like 12.1.
pub fn hours(secs: f64) -> String {
    format!("{:.1}", secs.max(0.0) / 3600.0)
}

fn path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("fablegame").join("profile.json"))
}

impl Profile {
    pub fn load() -> Profile {
        let mut p: Profile = path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        if p.since == 0 {
            p.since = now_unix();
        }
        // a flight left open by a game that did not close cleanly ends where it was.
        for e in &mut p.log {
            e.open = false;
        }
        p
    }

    pub fn save(&self) {
        let Some(path) = path() else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let Ok(text) = serde_json::to_string_pretty(self) else { return };
        // written whole then moved, so a crash mid write keeps the old one.
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }

    /// puts a flight in the log, replacing its open copy.
    pub fn record(&mut self, e: Entry) {
        if let Some(last) = self.log.last_mut() {
            if last.open && last.unix == e.unix {
                *last = e;
                return;
            }
        }
        self.log.push(e);
    }

    pub fn totals(&self) -> Totals {
        let mut t = Totals::default();
        let mut types: Vec<(String, f64, usize)> = Vec::new();
        let mut apts: Vec<(String, usize)> = Vec::new();
        for e in &self.log {
            t.air_s += e.air_s;
            t.flights += 1;
            t.landings += e.landings;
            t.dist_nm += e.dist_nm;
            t.longest_s = t.longest_s.max(e.air_s);
            t.highest_ft = t.highest_ft.max(e.max_alt_ft);
            match types.iter_mut().find(|x| x.0 == e.aircraft) {
                Some(x) => {
                    x.1 += e.air_s;
                    x.2 += 1;
                }
                None => types.push((e.aircraft.clone(), e.air_s, 1)),
            }
            for a in [&e.from, &e.to] {
                // icao codes only, not place names.
                if a.len() != 4 || !a.chars().all(|c| c.is_ascii_alphanumeric()) {
                    continue;
                }
                match apts.iter_mut().find(|x| &x.0 == a) {
                    Some(x) => x.1 += 1,
                    None => apts.push((a.clone(), 1)),
                }
            }
        }
        types.sort_by(|a, b| b.1.total_cmp(&a.1));
        apts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        t.by_type = types;
        t.airports = apts;
        t
    }
}

/// follows the flight being flown.
#[derive(Clone, Debug, Default)]
pub struct Tracker {
    pub entry: Entry,
    airborne: bool,
    /// seconds since lift off, to ignore a bounce.
    up_for: f64,
    last: Option<(f64, f64)>,
    since_save: f64,
}

/// what a step saw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    None,
    TookOff,
    Landed,
}

impl Tracker {
    pub fn start(aircraft: &str, from: &str) -> Tracker {
        Tracker { entry: Entry { unix: now_unix(), aircraft: aircraft.into(), from: from.into(), open: true, ..Default::default() }, ..Default::default() }
    }

    /// one step of sim time. at in degrees, alt in feet.
    pub fn step(&mut self, dt: f64, on_ground: bool, at: (f64, f64), alt_ft: f64) -> Event {
        let mut ev = Event::None;
        if dt <= 0.0 {
            return ev;
        }
        if let Some(l) = self.last {
            let d = crate::flightplan::dist(l, at) / 1852.0;
            // a jump is a teleport, not flying.
            if d < 5.0 && !on_ground {
                self.entry.dist_nm += d;
            }
        }
        self.last = Some(at);
        self.since_save += dt;
        if !on_ground {
            if !self.airborne {
                self.airborne = true;
                self.up_for = 0.0;
                ev = Event::TookOff;
            }
            self.up_for += dt;
            self.entry.air_s += dt;
            self.entry.max_alt_ft = self.entry.max_alt_ft.max(alt_ft);
        } else if self.airborne {
            self.airborne = false;
            // a hop of a few seconds is a bounce, not a landing.
            if self.up_for > 8.0 {
                self.entry.landings += 1;
                ev = Event::Landed;
            }
        }
        ev
    }

    /// due for a save of the open flight.
    pub fn save_due(&mut self) -> bool {
        if self.since_save > 120.0 {
            self.since_save = 0.0;
            return true;
        }
        false
    }

    /// worth a line in the log: some time in the air.
    pub fn worth_logging(&self) -> bool {
        self.entry.air_s >= 20.0 || self.entry.landings > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(date(0), "1970-01-01 00:00");
        assert_eq!(date(1_795_000_000), "2026-11-18 11:06");
        assert_eq!(hhmm(3600.0 * 2.0 + 5.0 * 60.0), "2:05");
        assert_eq!(hours(5400.0), "1.5");
    }

    #[test]
    fn a_flight_with_a_bounce_and_a_landing() {
        let mut t = Tracker::start("Cessna 172", "KSFO");
        let mut at = (37.6, -122.4);
        assert_eq!(t.step(1.0, true, at, 10.0), Event::None);
        assert_eq!(t.step(1.0, false, at, 50.0), Event::TookOff);
        // a 3 s bounce does not count.
        t.step(2.0, false, at, 60.0);
        assert_eq!(t.step(1.0, true, at, 10.0), Event::None);
        t.step(1.0, false, at, 50.0);
        for _ in 0..600 {
            at.1 += 0.0005;
            t.step(1.0, false, at, 3500.0);
        }
        assert_eq!(t.step(1.0, true, at, 10.0), Event::Landed);
        assert_eq!(t.entry.landings, 1);
        assert!((t.entry.air_s - 604.0).abs() < 1.0);
        assert!(t.entry.dist_nm > 12.0 && t.entry.dist_nm < 15.0, "{}", t.entry.dist_nm);
        assert_eq!(t.entry.max_alt_ft, 3500.0);
        let mut p = Profile::default();
        p.record(t.entry.clone());
        let mut e = t.entry.clone();
        e.open = false;
        e.to = "KOAK".into();
        p.record(e);
        assert_eq!(p.log.len(), 1);
        let tot = p.totals();
        assert_eq!((tot.flights, tot.landings), (1, 1));
        assert_eq!(tot.airports.len(), 2);
        assert_eq!(tot.by_type[0].0, "Cessna 172");
    }
}
