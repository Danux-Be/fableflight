//! the logbook screen: hours, flights and landings, by aircraft, and every flight.

use crate::menu::{self, ACCENT, BAD, DIM, FAINT, TEXT};
use crate::profile::{self, Entry, Totals};
use crate::{t, Game};
use egui::{pos2, vec2, Align2, Color32, Rect, RichText, Sense, Stroke};

/// one figure in the strip: a big number, its unit, a small label under it.
fn figure(p: &egui::Painter, r: Rect, value: &str, unit: &str, label: &str, big: bool) {
    let g = p.layout_no_wrap(value.to_string(), menu::bold(if big { 46.0 } else { 34.0 }), Color32::WHITE);
    let (w, h) = (g.size().x, g.size().y);
    let at = pos2(r.left(), r.center().y - h * 0.5 - 8.0);
    p.galley(at, g, Color32::WHITE);
    if !unit.is_empty() {
        p.text(pos2(at.x + w + 5.0, at.y + h - 7.0), Align2::LEFT_BOTTOM, unit, menu::medium(if big { 18.0 } else { 15.0 }), DIM);
    }
    p.text(pos2(at.x + 1.0, at.y + h + 2.0), Align2::LEFT_TOP, label, menu::caption(), DIM);
}

/// a place as the logbook keeps it: an airport code as it is, a "lat,lon" pair as degrees.
fn place(s: &str) -> String {
    let Some((a, b)) = s.split_once(',') else { return s.to_string() };
    match (a.trim().parse::<f64>(), b.trim().parse::<f64>()) {
        (Ok(lat), Ok(lon)) => format!("{:.2}°{} {:.2}°{}", lat.abs(), if lat < 0.0 { 'S' } else { 'N' }, lon.abs(), if lon < 0.0 { 'W' } else { 'E' }),
        _ => s.to_string(),
    }
}

/// a red or yellow tag in a table cell.
fn tag(p: &egui::Painter, left: f32, y: f32, text: &str, col: Color32) {
    let g = p.layout_no_wrap(text.to_string(), menu::semibold(12.5), col);
    let r = Rect::from_min_size(pos2(left, y - 10.0), vec2(g.size().x + 14.0, 20.0));
    p.rect_filled(r, 10.0, col.gamma_multiply(0.14));
    p.rect_stroke(r, 10.0, Stroke::new(1.0, col.gamma_multiply(0.5)), egui::StrokeKind::Inside);
    p.galley(r.center() - g.size() * 0.5, g, col);
}

/// The table's columns: where each starts as a share of the width, and
/// whether it is a number (set against its right edge); the last marks the end.
const COLS: [(f32, bool); 8] = [(0.0, false), (0.16, false), (0.35, false), (0.54, false), (0.71, true), (0.82, true), (0.92, true), (1.0, false)];

/// one flight's row.
fn flight_row(ui: &mut egui::Ui, e: &Entry, odd: bool) {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::hover());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(r, 3.0, Color32::from_white_alpha(10));
    } else if odd {
        p.rect_filled(r, 3.0, Color32::from_white_alpha(4));
    }
    let y = r.center().y;
    let x = |k: usize| r.left() + 12.0 + (r.width() - 24.0) * COLS[k].0;
    let room = |k: usize| (r.width() - 24.0) * (COLS[k + 1].0 - COLS[k].0) - 14.0;
    let cell = |k: usize, text: &str, col: Color32| {
        let g = menu::one_line(p, text, menu::label(), col, room(k));
        let at = if COLS[k].1 { pos2(x(k + 1) - 14.0 - g.size().x, y - g.size().y * 0.5) } else { pos2(x(k), y - g.size().y * 0.5) };
        p.galley(at, g, col);
    };
    cell(0, &profile::date(e.unix), DIM);
    cell(1, &e.aircraft, Color32::WHITE);
    cell(2, &place(&e.from), TEXT);
    if e.crashed {
        tag(p, x(3), y, t!("profile.crashed"), BAD);
    } else if e.to.is_empty() {
        // ended without a landing being seen.
        cell(3, "—", FAINT);
    } else {
        cell(3, &place(&e.to), TEXT);
    }
    cell(4, &profile::hhmm(e.air_s), Color32::WHITE);
    cell(5, &format!("{:.0} nm", e.dist_nm), TEXT);
    cell(6, &e.landings.to_string(), TEXT);
    resp.on_hover_text(t!("profile.row_tip", alt = format!("{:.0}", e.max_alt_ft)));
}

impl Game {
    /// the logbook (see the module docs).
    pub(crate) fn logbook_screen(&mut self, ctx: &egui::Context) {
        let screen = ctx.content_rect();
        let tot: Totals = self.profile.totals();
        let mut back = false;
        egui::Area::new(egui::Id::unique("logbook_bg")).order(egui::Order::Background).fixed_pos(screen.min).show(ctx, |ui| {
            ui.painter().rect_filled(screen, 0.0, Color32::from_rgba_premultiplied(4, 6, 10, 215));
        });
        let m = 32.0;
        let w = (screen.width() - 2.0 * m).min(1280.0);
        let x0 = screen.center().x - w * 0.5;
        egui::Area::new(egui::Id::unique("logbook")).fixed_pos(pos2(x0, m)).show(ctx, |ui| {
            ui.set_width(w);
            // the back button, the title, the pilot's name.
            ui.horizontal(|ui| {
                ui.set_min_height(48.0);
                if menu::choice(ui, &format!("‹   {}", t!("game.menu")), false, 120.0).clicked() {
                    back = true;
                }
                ui.add_space(18.0);
                menu::screen_heading(ui, t!("profile.logbook"));
                menu::at_right(ui, "pilot", |ui| {
                    ui.label(RichText::new(t!("profile.pilot")).font(menu::label()).color(DIM));
                    ui.add_space(4.0);
                    let r = ui.add(egui::TextEdit::singleline(&mut self.profile.name).desired_width(240.0).hint_text(t!("profile.pilot_name")).font(menu::label()).margin(vec2(10.0, 7.0)));
                    if r.lost_focus() {
                        self.profile.save();
                    }
                });
            });
            ui.add_space(18.0);
            // the figures, the hours first and largest.
            let (r, _) = ui.allocate_exact_size(vec2(w, 108.0), Sense::hover());
            let p = ui.painter();
            p.rect_filled(r, menu::R_PANEL, menu::PANEL);
            p.rect_stroke(r, menu::R_PANEL, Stroke::new(1.0, menu::LINE_HI), egui::StrokeKind::Inside);
            p.rect_filled(Rect::from_min_size(r.min, vec2(3.0, r.height())), 0.0, ACCENT);
            let cols = [
                (profile::hours(tot.air_s), t!("profile.h"), t!("profile.flight_time")),
                (tot.flights.to_string(), "", t!("profile.flights")),
                (tot.landings.to_string(), "", t!("profile.landings")),
                (format!("{:.0}", tot.dist_nm), "nm", t!("profile.distance")),
                (profile::hhmm(tot.longest_s), "", t!("profile.longest")),
                (format!("{:.0}", tot.highest_ft), "ft", t!("profile.highest")),
            ];
            // the hours take a wider cell, the rest share what is left evenly.
            let first = 260.0;
            let step = (r.width() - 32.0 - first) / (cols.len() - 1) as f32;
            for (k, (v, u, l)) in cols.iter().enumerate() {
                let left = r.left() + 32.0 + if k == 0 { 0.0 } else { first + step * (k - 1) as f32 };
                if k > 0 {
                    p.vline(left - 22.0, (r.top() + 24.0)..=(r.bottom() - 24.0), Stroke::new(1.0, Color32::from_white_alpha(12)));
                }
                figure(p, Rect::from_min_size(pos2(left, r.top()), vec2(step, r.height())), v, u, l, k == 0);
            }
            ui.add_space(16.0);
            let rest = (screen.bottom() - ui.cursor().top() - m).max(220.0);
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                let left_w = (w * 0.68).floor();
                // every flight, the latest first.
                ui.allocate_ui_with_layout(vec2(left_w, rest), egui::Layout::top_down(egui::Align::Min), |ui| {
                    menu::panel().inner_margin(egui::Margin::symmetric(20, 16)).show(ui, |ui| {
                        ui.set_width(left_w - 40.0);
                        ui.set_min_height(rest - 32.0);
                        ui.label(menu::section_text(t!("profile.flights_title")));
                        ui.add_space(10.0);
                        let heads = [t!("profile.col_date"), t!("profile.col_aircraft"), t!("profile.col_from"), t!("profile.col_to"), t!("profile.col_time"), t!("profile.col_dist"), t!("profile.col_ldg")];
                        let (hr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::hover());
                        let p = ui.painter();
                        let x = |k: usize| hr.left() + 12.0 + (hr.width() - 24.0) * COLS[k].0;
                        for (k, h) in heads.iter().enumerate() {
                            if COLS[k].1 {
                                p.text(pos2(x(k + 1) - 14.0, hr.center().y), Align2::RIGHT_CENTER, *h, menu::caption(), FAINT);
                            } else {
                                p.text(pos2(x(k), hr.center().y), Align2::LEFT_CENTER, *h, menu::caption(), FAINT);
                            }
                        }
                        p.hline(hr.x_range(), hr.bottom() + 2.0, Stroke::new(1.0, Color32::from_white_alpha(16)));
                        ui.add_space(6.0);
                        if self.profile.log.is_empty() {
                            ui.add_space(30.0);
                            menu::hint(ui, t!("profile.empty"));
                            return;
                        }
                        egui::ScrollArea::vertical().max_height(rest - 112.0).auto_shrink([false, false]).show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            for (k, e) in self.profile.log.iter().rev().enumerate() {
                                flight_row(ui, e, k % 2 == 1);
                            }
                        });
                    });
                });
                // hours by aircraft, and the airports flown from and to.
                let right_w = w - left_w - 16.0;
                ui.allocate_ui_with_layout(vec2(right_w, rest), egui::Layout::top_down(egui::Align::Min), |ui| {
                    menu::panel().inner_margin(egui::Margin::symmetric(20, 16)).show(ui, |ui| {
                        ui.set_width(right_w - 40.0);
                        ui.set_min_height(rest - 32.0);
                        ui.label(menu::section_text(t!("profile.by_aircraft")));
                        ui.add_space(10.0);
                        let most = tot.by_type.first().map(|b| b.1).unwrap_or(1.0).max(1.0);
                        for (name, secs, n) in tot.by_type.iter().take(10) {
                            let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 42.0), Sense::hover());
                            let p = ui.painter();
                            let figures = t!("profile.short", h = profile::hours(*secs), n = n);
                            let fg = p.layout_no_wrap(figures, menu::caption(), DIM);
                            let ng = menu::one_line(p, name, menu::label(), Color32::WHITE, r.width() - fg.size().x - 12.0);
                            p.galley(pos2(r.left(), r.top() + 2.0), ng, Color32::WHITE);
                            p.galley(pos2(r.right() - fg.size().x, r.top() + 4.0), fg, DIM);
                            let bar = Rect::from_min_size(pos2(r.left(), r.top() + 26.0), vec2(r.width(), 4.0));
                            p.rect_filled(bar, 2.0, Color32::from_white_alpha(14));
                            p.rect_filled(Rect::from_min_size(bar.min, vec2(bar.width() * (*secs / most) as f32, bar.height())), 2.0, ACCENT);
                        }
                        if tot.by_type.is_empty() {
                            ui.label(RichText::new("—").font(menu::body()).color(FAINT));
                        }
                        ui.add_space(18.0);
                        ui.label(menu::section_text(t!("profile.airports")));
                        ui.add_space(10.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                            for (a, n) in tot.airports.iter().take(40) {
                                let g = ui.painter().layout_no_wrap(a.to_string(), menu::label(), Color32::WHITE);
                                let ng = ui.painter().layout_no_wrap(n.to_string(), menu::caption(), DIM);
                                let (r, _) = ui.allocate_exact_size(vec2(g.size().x + ng.size().x + 30.0, 30.0), Sense::hover());
                                let p = ui.painter();
                                p.rect_filled(r, menu::R_WIDGET, menu::RAISED);
                                p.rect_stroke(r, menu::R_WIDGET, Stroke::new(1.0, menu::LINE_HI), egui::StrokeKind::Inside);
                                p.galley(pos2(r.left() + 10.0, r.center().y - g.size().y * 0.5), g.clone(), Color32::WHITE);
                                p.galley(pos2(r.right() - 10.0 - ng.size().x, r.center().y - ng.size().y * 0.5), ng, DIM);
                            }
                        });
                        if tot.airports.is_empty() {
                            ui.label(RichText::new("—").font(menu::body()).color(FAINT));
                        }
                        ui.add_space(18.0);
                        let since = profile::date(self.profile.since);
                        menu::hint(ui, &t!("profile.since", date = since.get(..10).unwrap_or(&since)));
                        menu::hint(ui, t!("profile.how"));
                    });
                });
            });
        });
        if back {
            self.menu.screen = menu::Screen::Main;
            self.pad_home = true;
        }
    }

    /// the logbook bar's words: hours and flights.
    pub(crate) fn logbook_short(&self) -> String {
        let t = self.profile.totals();
        t!("profile.short", h = profile::hours(t.air_s), n = t.flights)
    }
}
