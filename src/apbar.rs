//! the autopilot's controls on screen: a strip under the heading tape with
//! the modes and four knobs, as on a mode control panel, and the same knobs
//! on the autopilot page. each knob is two rings: the inner one turns in
//! fine steps, the outer one in coarse steps; a press on the middle sets it
//! to what the aircraft is doing.

use crate::autopilot::{self, Lat, Vert};
use crate::flightui::{self, key};
use crate::menu;
use crate::{t, Game};
use egui::{pos2, vec2, Align2, Color32, Sense, Stroke};

/// one detent of a knob, radians.
const DETENT: f32 = 0.2618;

/// what a knob did this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Turn {
    pub fine: i32,
    pub coarse: i32,
    pub push: bool,
}

impl Turn {
    pub fn any(&self) -> bool {
        self.fine != 0 || self.coarse != 0 || self.push
    }
}

/// a knob's state between frames: ring angles, turn left over, the ring held.
#[derive(Clone, Copy, Default)]
struct KnobState {
    outer: f32,
    inner: f32,
    acc: f32,
    wheel: f32,
    ring: u8,
}

fn wrap(a: f32) -> f32 {
    (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

/// a two ring knob `d` across. clockwise, up and the wheel away all turn it up.
pub fn knob(ui: &mut egui::Ui, d: f32) -> Turn {
    let (rect, resp) = ui.allocate_exact_size(vec2(d, d), Sense::click_and_drag());
    let c = rect.center();
    let big = d * 0.5;
    let small = big * 0.6;
    let id = resp.id;
    let mut st: KnobState = ui.data(|m| m.get_temp(id)).unwrap_or_default();
    let mut out = Turn::default();
    let pointer = ui.input(|i| i.pointer.hover_pos());
    let near_inner = pointer.is_some_and(|p| (p - c).length() < small * 1.08);
    let shift = ui.input(|i| i.modifiers.shift);
    let mut steps = 0;
    if resp.drag_started() {
        st.ring = if near_inner { 1 } else { 2 };
        st.acc = 0.0;
    }
    if resp.dragged() {
        if let Some(p) = pointer {
            let prev = p - resp.drag_delta();
            let mut da = 0.0;
            // round the middle, or up and down when close to it.
            if (p - c).length() > big * 0.3 && (prev - c).length() > big * 0.3 {
                da += wrap((p - c).angle() - (prev - c).angle());
            }
            da += -resp.drag_delta().y * 0.022;
            st.acc += da;
            while st.acc >= DETENT {
                st.acc -= DETENT;
                steps += 1;
            }
            while st.acc <= -DETENT {
                st.acc += DETENT;
                steps -= 1;
            }
        }
    }
    let coarse_ring = |ring: u8| ring == 2 || shift;
    if steps != 0 {
        if coarse_ring(st.ring) { out.coarse += steps } else { out.fine += steps }
    }
    if resp.hovered() {
        // notches of the wheel this frame, from its own events.
        let wheel: f32 = ui.input(|i| {
            i.events.iter().filter_map(|e| match e {
                egui::Event::MouseWheel { unit, delta, .. } => Some(match unit {
                    egui::MouseWheelUnit::Line => delta.y + delta.x,
                    egui::MouseWheelUnit::Point => (delta.y + delta.x) / 40.0,
                    egui::MouseWheelUnit::Page => (delta.y + delta.x) * 3.0,
                }),
                _ => None,
            }).sum()
        });
        if wheel != 0.0 {
            st.wheel += wheel;
        }
        // the wheel turns this knob, not the page.
        ui.ctx().input_mut(|i| i.smooth_scroll_delta = egui::Vec2::ZERO);
        let n = st.wheel.trunc() as i32;
        if n != 0 {
            st.wheel -= n as f32;
            if !near_inner || shift { out.coarse += n } else { out.fine += n }
        }
    } else {
        st.wheel = 0.0;
    }
    if resp.clicked() && near_inner {
        out.push = true;
    }
    if !resp.dragged() {
        st.ring = 0;
    }
    st.outer += out.coarse as f32 * DETENT;
    st.inner += out.fine as f32 * DETENT;
    ui.data_mut(|m| m.insert_temp(id, st));

    // the knob: a shadow, a knurled outer ring, a lighter inner cap with its mark.
    let p = ui.painter();
    let lit = menu::ease(ui.ctx(), id, resp.hovered() || resp.dragged());
    p.circle_filled(c + vec2(0.0, 1.5), big, Color32::from_black_alpha(120));
    p.circle_filled(c, big, Color32::from_rgb(34, 38, 45));
    let held_outer = resp.dragged() && st.ring == 2 || (resp.hovered() && !near_inner);
    let held_inner = resp.dragged() && st.ring == 1 || (resp.hovered() && near_inner);
    for k in 0..28 {
        let a = st.outer + k as f32 * std::f32::consts::TAU / 28.0;
        let dir = egui::Vec2::angled(a);
        p.line_segment([c + dir * (big * 0.8), c + dir * (big * 0.97)], Stroke::new(1.4, Color32::from_gray(if held_outer { 120 } else { 78 })));
    }
    p.circle_stroke(c, big, Stroke::new(1.0, menu::mix(Color32::from_white_alpha(30), menu::ACCENT, if held_outer { lit } else { 0.0 })));
    p.circle_filled(c, small, Color32::from_rgb(62, 68, 78));
    for k in 0..18 {
        let a = st.inner + k as f32 * std::f32::consts::TAU / 18.0;
        let dir = egui::Vec2::angled(a);
        p.line_segment([c + dir * (small * 0.78), c + dir * small], Stroke::new(1.2, Color32::from_gray(if held_inner { 150 } else { 104 })));
    }
    p.circle_stroke(c, small, Stroke::new(1.0, menu::mix(Color32::from_white_alpha(40), menu::ACCENT, if held_inner { lit } else { 0.0 })));
    // the mark on the cap, pointing up at rest.
    let m = egui::Vec2::angled(st.inner - std::f32::consts::FRAC_PI_2);
    p.line_segment([c + m * (small * 0.25), c + m * (small * 0.7)], Stroke::new(2.0, Color32::WHITE));
    let resp = resp.on_hover_text(t!("ap.knob_hint"));
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }
    out
}

/// what one knob sets: heading, altitude, vertical speed, speed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dial {
    Hdg,
    Alt,
    Vs,
    Spd,
}

impl Game {
    /// a knob turned: fine and coarse steps, or a push to the aircraft's own value.
    pub(crate) fn ap_turn(&mut self, d: Dial, t: Turn) {
        let s = autopilot::State::of(&self.plane);
        let (f, c) = (t.fine as f64, t.coarse as f64);
        match d {
            Dial::Hdg => {
                self.ap.hdg = if t.push { s.hdg.round() } else { (self.ap.hdg.round() + f + 10.0 * c).rem_euclid(360.0) };
            }
            Dial::Alt => {
                let ft = if t.push { (s.alt * 3.28084 / 100.0).round() * 100.0 } else { ((self.ap.alt * 3.28084 / 100.0).round() * 100.0 + 100.0 * f + 1000.0 * c).clamp(0.0, 90_000.0) };
                self.ap.alt = ft / 3.28084;
            }
            Dial::Vs => {
                let fpm = if t.push { (s.vs * 196.85 / 100.0).round() * 100.0 } else { ((self.ap.vs * 196.85 / 100.0).round() * 100.0 + 100.0 * f + 500.0 * c).clamp(-6000.0, 6000.0) };
                self.ap.vs = fpm / 196.85;
            }
            Dial::Spd => {
                let kt = if t.push { (s.ias * 1.943_84).round() } else { ((self.ap.spd * 1.943_84).round() + f + 10.0 * c).max(40.0) };
                self.ap.spd = kt / 1.943_84;
            }
        }
    }

    /// the value a knob sets, as shown.
    pub(crate) fn dial_text(&self, d: Dial) -> (String, &'static str) {
        match d {
            Dial::Hdg => (format!("{:03.0}", self.ap.hdg.rem_euclid(360.0)), "°"),
            Dial::Alt => (format!("{:.0}", (self.ap.alt * 3.28084 / 100.0).round() * 100.0), "ft"),
            Dial::Vs => (format!("{:+.0}", (self.ap.vs * 196.85 / 100.0).round() * 100.0), "fpm"),
            Dial::Spd => (format!("{:.0}", self.ap.spd * 1.943_84), "kt"),
        }
    }

    /// the strip on screen under the heading tape.
    pub(crate) fn ap_bar(&mut self, ctx: &egui::Context) {
        if !self.ap_bar_on || self.crashed || self.spec.rocket().is_some() || self.info_open {
            return;
        }
        // nothing set yet: the knobs start from what the aircraft is doing.
        if !self.ap.on && self.ap.alt == 0.0 {
            let s = autopilot::State::of(&self.plane);
            self.ap.sync(&s);
            self.ap.spd = s.ias;
        }
        let (on, armed) = self.ap.annunciation();
        let mut lat = None;
        let mut vert = None;
        let mut toggle = false;
        let mut at = false;
        let mut turns: Vec<(Dial, Turn)> = Vec::new();
        egui::Area::new(egui::Id::unique("ap_bar")).anchor(Align2::CENTER_TOP, [0.0, 92.0]).show(ctx, |ui| {
            let frame = egui::Frame::new().fill(Color32::from_rgba_premultiplied(8, 10, 14, 214)).stroke(Stroke::new(1.0, Color32::from_white_alpha(28))).corner_radius(flightui::R_HUD).inner_margin(egui::Margin::symmetric(10, 8));
            frame.show(ui, |ui| {
                // the annunciator: what flies now in green, what waits in white.
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 14.0;
                    let ap_col = if self.ap.on { Color32::from_rgb(80, 230, 120) } else { Color32::from_gray(120) };
                    ui.label(egui::RichText::new(if self.ap.on { "AP" } else { "AP OFF" }).font(menu::bold(13.0)).color(ap_col));
                    if self.ap.on {
                        for m in &on {
                            ui.label(egui::RichText::new(m).font(menu::data()).color(Color32::from_rgb(80, 230, 120)));
                        }
                        for m in &armed {
                            ui.label(egui::RichText::new(m).font(menu::data()).color(Color32::WHITE));
                        }
                    } else if self.ap.at {
                        ui.label(egui::RichText::new(format!("SPD {:.0}", self.ap.spd * 1.943_84)).font(menu::data()).color(Color32::from_rgb(80, 230, 120)));
                    }
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    // engage and the lateral modes, two rows.
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 4.0;
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            toggle = key(ui, "AP", self.ap.on, 46.0, 24.0).on_hover_text("F5").clicked();
                            at = key(ui, "AT", self.ap.at, 46.0, 24.0).on_hover_text(t!("game.autothrottle")).clicked();
                        });
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            for (m, name) in [(Lat::Nav, "NAV"), (Lat::Gps, "GPS"), (Lat::Apr, "APR")] {
                                if key(ui, name, self.ap.on && self.ap.lat == m, 30.0, 22.0).clicked() {
                                    lat = Some(m);
                                }
                            }
                        });
                    });
                    ui.add_space(6.0);
                    for (d, name, mode) in [(Dial::Hdg, "HDG", 0), (Dial::Alt, "ALT", 1), (Dial::Vs, "VS", 2), (Dial::Spd, "SPD", 3)] {
                        let sep = ui.cursor().left() - 6.0;
                        ui.painter().vline(sep, ui.cursor().top()..=ui.cursor().top() + 52.0, Stroke::new(1.0, Color32::from_white_alpha(22)));
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            let lit = match mode {
                                0 => self.ap.on && self.ap.lat == Lat::Hdg,
                                1 => self.ap.on && self.ap.vert == Vert::Alt,
                                2 => self.ap.on && self.ap.vert == Vert::Vs,
                                _ => self.ap.at,
                            };
                            if key(ui, name, lit, 46.0, 20.0).clicked() {
                                match mode {
                                    0 => lat = Some(Lat::Hdg),
                                    1 => vert = Some(Vert::Alt),
                                    2 => vert = Some(Vert::Vs),
                                    _ => at = true,
                                }
                            }
                            let (v, unit) = self.dial_text(d);
                            let (r, _) = ui.allocate_exact_size(vec2(70.0, 28.0), Sense::hover());
                            let vr = ui.painter().text(pos2(r.left(), r.center().y), Align2::LEFT_CENTER, v, menu::bold(19.0), Color32::WHITE);
                            ui.painter().text(pos2(vr.right() + 3.0, vr.bottom() - 3.0), Align2::LEFT_BOTTOM, unit, menu::caption(), flightui::SOFT);
                        });
                        let t = knob(ui, 50.0);
                        if t.any() {
                            turns.push((d, t));
                        }
                        ui.add_space(4.0);
                    }
                });
            });
        });
        for (d, t) in turns {
            self.ap_turn(d, t);
        }
        if at {
            self.ap_at_toggle();
        }
        if toggle {
            self.ap_toggle();
        }
        if let Some(m) = lat {
            self.ap_lat(m);
        }
        if let Some(m) = vert {
            self.ap_vert(m);
        }
    }

    /// the four knobs on the autopilot page, each under its value.
    pub(crate) fn ap_knobs(&mut self, ui: &mut egui::Ui) {
        let mut turns: Vec<(Dial, Turn)> = Vec::new();
        let dials = [(Dial::Hdg, "HDG"), (Dial::Alt, "ALT"), (Dial::Vs, "VS"), (Dial::Spd, "SPD")];
        flightui::cells(ui, 4, 150.0, |ui, i| {
            let (d, name) = dials[i];
            flightui::tag(ui, name);
            let (v, unit) = self.dial_text(d);
            flightui::big(ui, &v, unit, Color32::WHITE);
            ui.add_space(6.0);
            let w = ui.available_width();
            ui.horizontal(|ui| {
                ui.add_space(((w - 64.0) * 0.5).max(0.0));
                let t = knob(ui, 64.0);
                if t.any() {
                    turns.push((d, t));
                }
            });
        });
        for (d, t) in turns {
            self.ap_turn(d, t);
        }

    }

    /// in ALT, a new altitude set on the knob starts a climb or descent to it
    /// once the knob has been still a moment; called each step with the autopilot on.
    pub(crate) fn ap_auto_level(&mut self, s: &autopilot::State) {
        if (self.ap.alt - self.ap_alt_seen.0).abs() > 0.5 {
            self.ap_alt_seen = (self.ap.alt, self.real);
        }
        if self.ap.vert != Vert::Alt || self.plane.on_ground {
            return;
        }
        let to_go = self.ap.alt - self.ap.holding();
        if to_go.abs() > 90.0 && self.real - self.ap_alt_seen.1 > 1.2 {
            let rate = autopilot::level_rate(self.spec);
            self.ap.level_change(s, rate);
            let ft = format!("{:.0}", (self.ap.alt * 3.28084 / 100.0).round() * 100.0);
            let fpm = format!("{:.0}", rate * 196.85);
            self.status = if to_go > 0.0 { t!("ap.climbing_to", ft = ft, fpm = fpm) } else { t!("ap.descending_to", ft = ft, fpm = fpm) };
            self.status_until = self.real + 4.0;
        }
    }
}
