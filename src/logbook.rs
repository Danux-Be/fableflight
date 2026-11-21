//! the logbook screen: hours, flights and landings, by aircraft, and every flight.

use crate::menu::{self, ACCENT, DIM, FAINT, TEXT};
use crate::profile::{self, Totals};
use crate::{t, Game};
use egui::{pos2, vec2, Align2, Color32, Rect, RichText, Sense, Stroke};

/// one figure: a big number over a small label.
fn figure(p: &egui::Painter, at: egui::Pos2, value: &str, unit: &str, label: &str, big: bool) {
    let size = if big { 54.0 } else { 30.0 };
    let g = p.layout_no_wrap(value.to_string(), menu::bold(size), Color32::WHITE);
    let w = g.size().x;
    let h = g.size().y;
    p.galley(at, g, Color32::WHITE);
    if !unit.is_empty() {
        p.text(pos2(at.x + w + 6.0, at.y + h - 6.0), Align2::LEFT_BOTTOM, unit, menu::medium(if big { 18.0 } else { 14.0 }), DIM);
    }
    p.text(pos2(at.x + 1.0, at.y + h + 4.0), Align2::LEFT_TOP, label.to_uppercase(), menu::bold(11.0), DIM);
}

impl Game {
    /// the logbook (see the module docs).
    pub(crate) fn logbook_screen(&mut self, ctx: &egui::Context) {
        let screen = ctx.content_rect();
        let tot: Totals = self.profile.totals();
        let mut back = false;
        egui::Area::new(egui::Id::unique("logbook_bg")).order(egui::Order::Background).fixed_pos(screen.min).show(ctx, |ui| {
            let p = ui.painter();
            p.rect_filled(screen, 0.0, Color32::from_rgba_premultiplied(4, 6, 10, 210));
        });
        let m = 28.0;
        let w = (screen.width() - 2.0 * m).min(1240.0);
        let x0 = screen.center().x - w * 0.5;
        egui::Area::new(egui::Id::unique("logbook")).fixed_pos(pos2(x0, m)).show(ctx, |ui| {
            ui.set_width(w);
            // header: back, title, the pilot's name.
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(RichText::new(format!("‹  {}", t!("game.menu"))).font(menu::label())).min_size(vec2(120.0, 34.0))).clicked() {
                    back = true;
                }
                ui.add_space(14.0);
                ui.label(RichText::new(t!("profile.logbook")).font(menu::bold(30.0)).color(Color32::WHITE));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let r = ui.add(egui::TextEdit::singleline(&mut self.profile.name).desired_width(220.0).hint_text(t!("profile.pilot_name")).margin(vec2(8.0, 6.0)));
                    if r.lost_focus() {
                        self.profile.save();
                    }
                    ui.label(RichText::new(t!("profile.pilot")).font(menu::caption()).color(DIM));
                });
            });
            ui.add_space(18.0);
            // the figures, the hours first and largest.
            let (r, _) = ui.allocate_exact_size(vec2(w, 112.0), Sense::hover());
            let p = ui.painter();
            p.rect_filled(r, menu::R_PANEL, menu::PANEL);
            p.rect_stroke(r, menu::R_PANEL, Stroke::new(1.0, menu::LINE_HI), egui::StrokeKind::Inside);
            p.rect_filled(Rect::from_min_size(r.min, vec2(3.0, r.height())), 0.0, ACCENT);
            let x = r.left() + 30.0;
            figure(p, pos2(x, r.top() + 14.0), &profile::hours(tot.air_s), t!("profile.h"), t!("profile.flight_time"), true);
            let cols = [
                (tot.flights.to_string(), "", t!("profile.flights")),
                (tot.landings.to_string(), "", t!("profile.landings")),
                (format!("{:.0}", tot.dist_nm), "nm", t!("profile.distance")),
                (profile::hhmm(tot.longest_s), "", t!("profile.longest")),
                (format!("{:.0}", tot.highest_ft), "ft", t!("profile.highest")),
            ];
            let step = (r.width() - 300.0) / cols.len() as f32;
            for (k, (v, u, l)) in cols.iter().enumerate() {
                figure(p, pos2(r.left() + 290.0 + step * k as f32, r.top() + 30.0), v, u, l, false);
            }
            ui.add_space(16.0);
            let rest = (screen.bottom() - ui.cursor().top() - m).max(200.0);
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                let left_w = w * 0.64;
                // every flight, the latest first.
                ui.allocate_ui_with_layout(vec2(left_w, rest), egui::Layout::top_down(egui::Align::Min), |ui| {
                    menu::panel().inner_margin(egui::Margin::symmetric(18, 14)).show(ui, |ui| {
                        ui.set_width(left_w - 36.0);
                        ui.set_min_height(rest - 28.0);
                        ui.label(RichText::new(t!("profile.flights_title")).font(menu::label()).color(Color32::WHITE));
                        ui.add_space(6.0);
                        let cols = [0.0, 128.0, 300.0, 430.0, 500.0, 566.0];
                        let heads = [t!("profile.col_date"), t!("profile.col_aircraft"), t!("profile.col_route"), t!("profile.col_time"), "NM", t!("profile.col_ldg")];
                        let (hr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 18.0), Sense::hover());
                        for (k, h) in heads.iter().enumerate() {
                            ui.painter().text(pos2(hr.left() + cols[k], hr.center().y), Align2::LEFT_CENTER, *h, menu::bold(10.5), DIM);
                        }
                        if self.profile.log.is_empty() {
                            ui.add_space(30.0);
                            ui.label(RichText::new(t!("profile.empty")).font(menu::body()).color(DIM));
                            return;
                        }
                        egui::ScrollArea::vertical().max_height(rest - 100.0).auto_shrink([false, false]).show(ui, |ui| {
                            for e in self.profile.log.iter().rev() {
                                let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::hover());
                                let p = ui.painter();
                                if resp.hovered() {
                                    p.rect_filled(r, 3.0, Color32::from_white_alpha(8));
                                }
                                p.hline(r.x_range(), r.bottom(), Stroke::new(1.0, menu::LINE));
                                let y = r.center().y;
                                p.text(pos2(r.left() + cols[0], y), Align2::LEFT_CENTER, profile::date(e.unix), menu::data(), TEXT);
                                let ac = p.layout(e.aircraft.clone(), menu::caption(), Color32::WHITE, cols[2] - cols[1] - 10.0);
                                p.galley(pos2(r.left() + cols[1], y - ac.size().y * 0.5), ac, Color32::WHITE);
                                let to = if e.to.is_empty() { "…".to_string() } else { e.to.clone() };
                                let route = format!("{}  →  {}", e.from, to);
                                p.text(pos2(r.left() + cols[2], y), Align2::LEFT_CENTER, route, menu::data(), if e.crashed { Color32::from_rgb(255, 130, 90) } else { Color32::WHITE });
                                p.text(pos2(r.left() + cols[3], y), Align2::LEFT_CENTER, profile::hhmm(e.air_s), menu::data(), ACCENT);
                                p.text(pos2(r.left() + cols[4], y), Align2::LEFT_CENTER, format!("{:.0}", e.dist_nm), menu::data(), TEXT);
                                p.text(pos2(r.left() + cols[5], y), Align2::LEFT_CENTER, e.landings.to_string(), menu::data(), TEXT);
                                let tip = t!("profile.row_tip", alt = format!("{:.0}", e.max_alt_ft));
                                resp.on_hover_text(tip);
                            }
                        });
                    });
                });
                // hours by aircraft, and the airports flown from and to.
                let right_w = w - left_w - 16.0;
                ui.allocate_ui_with_layout(vec2(right_w, rest), egui::Layout::top_down(egui::Align::Min), |ui| {
                    menu::panel().inner_margin(egui::Margin::symmetric(18, 14)).show(ui, |ui| {
                        ui.set_width(right_w - 36.0);
                        ui.set_min_height(rest - 28.0);
                        ui.label(RichText::new(t!("profile.by_aircraft")).font(menu::label()).color(Color32::WHITE));
                        ui.add_space(8.0);
                        let most = tot.by_type.first().map(|b| b.1).unwrap_or(1.0).max(1.0);
                        for (name, secs, n) in tot.by_type.iter().take(10) {
                            let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::hover());
                            let p = ui.painter();
                            p.text(pos2(r.left(), r.top() + 8.0), Align2::LEFT_CENTER, name, menu::caption(), Color32::WHITE);
                            p.text(pos2(r.right(), r.top() + 8.0), Align2::RIGHT_CENTER, format!("{} h  ·  {n}", profile::hours(*secs)), menu::data(), TEXT);
                            let bar = Rect::from_min_size(pos2(r.left(), r.top() + 20.0), vec2(r.width(), 5.0));
                            p.rect_filled(bar, 2.0, Color32::from_white_alpha(14));
                            p.rect_filled(Rect::from_min_size(bar.min, vec2(bar.width() * (*secs / most) as f32, bar.height())), 2.0, ACCENT);
                        }
                        if tot.by_type.is_empty() {
                            ui.label(RichText::new("—").font(menu::body()).color(FAINT));
                        }
                        ui.add_space(14.0);
                        ui.label(RichText::new(t!("profile.airports")).font(menu::label()).color(Color32::WHITE));
                        ui.add_space(6.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                            for (a, n) in tot.airports.iter().take(40) {
                                let g = ui.painter().layout_no_wrap(format!("{a}  {n}"), menu::data(), TEXT);
                                let (r, _) = ui.allocate_exact_size(g.size() + vec2(16.0, 8.0), Sense::hover());
                                ui.painter().rect_stroke(r, menu::R_WIDGET, Stroke::new(1.0, menu::LINE_HI), egui::StrokeKind::Inside);
                                ui.painter().galley(r.min + vec2(8.0, 4.0), g, TEXT);
                            }
                        });
                        ui.add_space(14.0);
                        let since = profile::date(self.profile.since);
                        ui.label(RichText::new(t!("profile.since", date = since.get(..10).unwrap_or(&since))).font(menu::caption()).color(FAINT));
                        ui.label(RichText::new(t!("profile.how")).font(menu::caption()).color(FAINT));
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
