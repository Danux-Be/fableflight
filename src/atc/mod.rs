//! Air traffic control: controllers who talk the player through a flight.
//!
//! - `controller`: who says what, and when (a state machine over the flight's phases);
//! - `phrase`: the messages, shown and said in ICAO style;
//! - `tts`, `lexicon`, `install`: the voices (Kokoro-82M, downloaded on request);
//! - `audio::radio`: what a voice sounds like on the radio.
//!
//! `Atc` runs it in the game: it queues the controllers' calls one after
//! another with a pilot's pause between, asks for each voice as soon as the
//! call is queued (so it is usually ready when its turn comes), hands the
//! speech to the mixer, and keeps the words for the subtitles and the radio
//! log. Without the voice pack (or with voices off) the calls still come, as
//! text.

pub mod controller;
pub mod install;
pub mod lexicon;
pub mod phrase;
pub mod tts;

use controller::{Call, Controller, Facility, Situation};
use crossbeam_channel::Receiver;
use std::collections::VecDeque;

/// The pilot's readback and a breath, between one call and the next, s.
const GAP: f64 = 1.2;
/// How long to wait for a voice that isn't ready before showing the words alone, s.
const VOICE_WAIT: f64 = 12.0;
/// Calls kept in the radio log.
const LOG: usize = 8;

struct Queued {
    call: Call,
    /// when it may start (game-running seconds)
    due: f64,
    voice: Option<Receiver<Result<tts::Speech, String>>>,
}

/// A call on the air or in the log.
#[derive(Clone, Debug)]
pub struct Heard {
    pub station: String,
    pub freq_khz: u32,
    pub text: String,
    /// when it started, and how long it lasts (s)
    pub at: f64,
    pub len: f64,
}

pub struct Atc {
    pub controller: Controller,
    queue: VecDeque<Queued>,
    /// running time (s), and when the frequency is free again
    t: f64,
    busy_until: f64,
    pub log: VecDeque<Heard>,
    /// wait for every voice however long it takes (a simulated flight runs
    /// far faster than the voices can be made)
    #[cfg(test)]
    wait: bool,
}

impl Atc {
    pub fn new(callsign: &str) -> Self {
        Atc {
            controller: Controller::new(callsign),
            queue: VecDeque::new(),
            t: 0.0,
            busy_until: 0.0,
            log: VecDeque::new(),
            #[cfg(test)]
            wait: false,
        }
    }

    pub fn set_callsign(&mut self, c: &str) {
        let c = c.trim();
        self.controller.callsign = if c.is_empty() { "Fable 123".into() } else { c.to_owned() };
    }

    /// A new flight: nothing queued, the controllers start over.
    pub fn reset(&mut self) {
        self.controller.reset();
        self.queue.clear();
    }

    /// One frame of `dt` seconds while flying. `voices`: speak (the pack is
    /// installed and voices are on). Calls that start now are handed to
    /// `play` with their speech (None: words only).
    pub fn update(&mut self, s: &Situation, dt: f64, voices: bool, mut play: impl FnMut(&Call, Option<&tts::Speech>)) {
        self.t += dt;
        for call in self.controller.step(s, dt) {
            let due = self.queue.back().map_or(self.t, |q| q.due).max(self.t) + call.after;
            let voice = voices.then(|| tts::Tts::get().say(&call.phrase.said, voice_for(&call.station), 1.12));
            self.queue.push_back(Queued { call, due, voice });
        }
        // the next call, once the frequency is free and its time has come
        let Some(q) = self.queue.front_mut() else { return };
        if self.t < self.busy_until || self.t < q.due {
            return;
        }
        #[cfg(test)]
        if self.wait {
            if let Some(rx) = q.voice.take() {
                let r = rx.recv().unwrap_or_else(|_| Err("gone".into()));
                let (tx, again) = crossbeam_channel::bounded(1);
                let _ = tx.send(r);
                q.voice = Some(again);
            }
        }
        let speech = match &q.voice {
            Some(rx) => match rx.try_recv() {
                Ok(Ok(s)) => Some(s),
                Ok(Err(_)) | Err(crossbeam_channel::TryRecvError::Disconnected) => None,
                Err(crossbeam_channel::TryRecvError::Empty) if self.t - q.due < VOICE_WAIT => return,
                Err(_) => None,
            },
            None => None,
        };
        let q = self.queue.pop_front().unwrap();
        let len = match &speech {
            Some(s) => s.samples.len() as f64 / s.rate as f64 + 0.3,
            // reading time: about 2.6 words a second, as controllers speak
            None => 1.5 + q.call.phrase.said.split_whitespace().count() as f64 / 2.6,
        };
        play(&q.call, speech.as_ref());
        self.busy_until = self.t + len + GAP;
        self.log.push_back(Heard { station: q.call.station.clone(), freq_khz: q.call.freq_khz, text: q.call.phrase.shown.clone(), at: self.t, len });
        while self.log.len() > LOG {
            self.log.pop_front();
        }
    }

    /// The call on the air now, for the subtitle (with how far through it is, 0..1).
    pub fn on_air(&self) -> Option<(&Heard, f32)> {
        let h = self.log.back()?;
        let k = (self.t - h.at) / h.len.max(0.1);
        // the words stay up a moment after the voice ends
        (self.t < h.at + h.len + 2.5).then_some((h, k.min(1.0) as f32))
    }
}

/// Each station keeps the same voice.
pub fn voice_for(station: &str) -> &'static str {
    tts::VOICES[(controller::hash(station) % tts::VOICES.len() as u64) as usize]
}

/// What a facility's radio sounds like.
pub fn radio_station(f: Facility) -> crate::audio::radio::Station {
    use crate::audio::radio::Station;
    match f {
        Facility::Ground => Station::GROUND,
        Facility::Tower => Station::TOWER,
        Facility::Departure | Facility::Approach | Facility::Center => Station::RADAR,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calls_wait_their_turn() {
        let apt = crate::airports::Db::get().find("KSFO").unwrap();
        let a = &crate::airports::Db::get().apts[apt as usize];
        let mut atc = Atc::new("Fable 123");
        // airborne near SFO going there: approach calls, then (close in) approach and tower
        let s = Situation { lat: a.lat, lon: a.lon + 0.1, alt_ft: 2_000.0, agl_ft: 2_000.0, gs_kt: 100.0, vs_fpm: -500.0, dest: Some(apt), qnh: 1013.0, wind_from: 280.0, wind_kt: 8.0, ..Default::default() };
        let mut heard = Vec::new();
        for _ in 0..600 {
            atc.update(&s, 0.1, false, |c, sp| {
                assert!(sp.is_none());
                heard.push(c.phrase.shown.clone());
            });
        }
        assert_eq!(heard.len(), 1, "{heard:?}");
        assert!(heard[0].contains("cleared to land"));
        assert!(atc.on_air().is_none(), "the subtitle goes once it has been read");
        assert_eq!(atc.log.len(), 1);
    }

    /// A whole flight, San Francisco to Sacramento in a light single, through the
    /// controllers, the voices (installed pack) and the radio: what the pilot
    /// hears, the quiet stretches cut short, written to `ATC_OUT` (a WAV), and
    /// the words printed. `cargo test --release --bin fablegame atc_flight -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn atc_flight() {
        use crate::audio::radio::{wav_bytes, Transmission, OUT_RATE};
        use crate::flightplan;
        let db = crate::airports::Db::get();
        let (sfo, smf) = (db.find("KSFO").unwrap(), db.find("KSMF").unwrap());
        let (a, b) = (&db.apts[sfo as usize], &db.apts[smf as usize]);
        let (from, to) = ((a.lat, a.lon), (b.lat, b.lon));
        let total = flightplan::dist(from, to);
        let mut atc = Atc::new("N172SP");
        atc.wait = true;
        let voices = tts::Tts::available();
        let mut s = Situation { lat: a.lat, lon: a.lon, alt_ft: 13.0, on_ground: true, wind_from: 290.0, wind_kt: 9.0, qnh: 1016.0, dest: Some(smf), cruise_ft: 5_500.0, ..Default::default() };
        let field = |idx: u32, rwy: &str| controller::Field { idx, runway: Some(rwy.into()), ..Default::default() };
        let mut out: Vec<f32> = Vec::new();
        let mut words = Vec::new();
        // (where in the recording each call starts, s; who; what)
        let mut marks: Vec<String> = Vec::new();
        let mut t = 0.0;
        let mut step = |atc: &mut Atc, s: &Situation, secs: f64, out: &mut Vec<f32>, t: &mut f64| {
            let n = (secs / 0.5).ceil() as usize;
            for _ in 0..n {
                *t += 0.5;
                atc.update(s, 0.5, voices, |call, sp| {
                    words.push(format!("{:>6.0}s  [{} {}]  {}", *t, call.station, crate::atc_freq(call.freq_khz), call.phrase.shown));
                    if let Some(sp) = sp {
                        let x = Transmission::new(sp.samples.to_vec(), sp.rate, radio_station(call.facility), call.signal, *t as u32).render();
                        out.extend(std::iter::repeat_n(0.0, (OUT_RATE as f32 * 0.9) as usize));
                        marks.push(format!("{:.3}\t{:.3}\t{} {}\t{}", out.len() as f32 / OUT_RATE as f32, x.len() as f32 / OUT_RATE as f32, call.station, crate::atc_freq(call.freq_khz), call.phrase.shown));
                        out.extend(x);
                    }
                });
            }
        };
        // at the stand, the engine started
        s.field = Some(field(sfo, "28R"));
        s.engine = true;
        step(&mut atc, &s, 40.0, &mut out, &mut t);
        // taxied to the hold
        s.field.as_mut().unwrap().at_hold = true;
        step(&mut atc, &s, 30.0, &mut out, &mut t);
        // the climb out, then the cruise, along the way
        s.field = None;
        s.on_ground = false;
        s.gs_kt = 105.0;
        let mut d = 0.0;
        while d < total {
            d += 105.0 * 0.514 * 10.0;
            let (lat, lon) = flightplan::between(from, to, (d / total).min(1.0));
            let left = total - d;
            // climb at 700 ft/min to 5,500 ft, descend from 30 km out
            let alt: f64 = (13.0 + d / (105.0 * 0.514) / 60.0 * 700.0).min(5_500.0).min(100.0 + left / 30_000.0 * 5_400.0).max(50.0);
            s.vs_fpm = (alt - s.alt_ft) / 10.0 * 60.0;
            (s.lat, s.lon, s.alt_ft, s.agl_ft) = (lat, lon, alt, alt - 27.0);
            if left < 9_000.0 {
                s.field = Some(field(smf, "35L"));
            }
            step(&mut atc, &s, 10.0, &mut out, &mut t);
        }
        // down, slowing on the runway, then at the stand with the engine off
        (s.on_ground, s.agl_ft, s.alt_ft, s.gs_kt, s.vs_fpm) = (true, 0.0, 27.0, 35.0, 0.0);
        step(&mut atc, &s, 40.0, &mut out, &mut t);
        println!("{}", words.join("\n"));
        assert!(words.len() >= 9, "{} calls", words.len());
        assert!(words.iter().any(|w| w.contains("Sacramento Approach")) && words.iter().any(|w| w.contains("cleared to land")));
        // each hand-off comes from the controller the pilot is talking to
        for pair in words.windows(2) {
            if let Some((_, to)) = pair[0].split_once("contact ") {
                let next = pair[1].split_once('[').unwrap().1;
                assert!(to.starts_with(next.split(" 1").next().unwrap()) || to.starts_with("ground"), "{} → {}", pair[0], pair[1]);
            }
        }
        if let Ok(p) = std::env::var("ATC_OUT") {
            std::fs::write(&p, wav_bytes(&out, OUT_RATE)).unwrap();
            std::fs::write(format!("{p}.tsv"), marks.join("\n")).unwrap();
            println!("{:.0}s of radio written to {p}", out.len() as f32 / OUT_RATE as f32);
        }
    }

    #[test]
    fn a_station_keeps_its_voice() {
        assert_eq!(voice_for("San Francisco Tower"), voice_for("San Francisco Tower"));
        let voices: std::collections::HashSet<_> = ["San Francisco Tower", "San Francisco Ground", "Oakland Center", "London Control", "Paris Approach", "Denver Center"].iter().map(|s| voice_for(s)).collect();
        assert!(voices.len() >= 3, "too few voices: {voices:?}");
    }
}
