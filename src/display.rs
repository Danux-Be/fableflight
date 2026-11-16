//! The window: in a window, full screen on one monitor, or across all of
//! them (one panoramic view over side-by-side monitors). Kept in
//! `display.json` beside the other settings; F11 goes between the window and
//! the last full-screen choice.
//!
//! Across all monitors: one borderless window over the bounding box of the
//! monitors, drawn as one wide view whose horizontal field of view is kept
//! within `wide_hfov` (a flat 32:9 frustum at the usual vertical angle would
//! be a fish-eye). A Wayland client cannot place its window, so on Wayland
//! the game runs through XWayland in that mode (decided at start: see
//! `wants_x11`).
use std::path::PathBuf;
use winit::window::{Fullscreen, Window};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Windowed,
    Fullscreen,
    Span,
}

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Mode::Windowed => "windowed",
            Mode::Fullscreen => "fullscreen",
            Mode::Span => "span",
        }
    }
    pub fn parse(s: &str) -> Option<Mode> {
        match s.to_ascii_lowercase().as_str() {
            "windowed" | "window" => Some(Mode::Windowed),
            "fullscreen" | "full" => Some(Mode::Fullscreen),
            "span" | "all" | "panoramic" => Some(Mode::Span),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Display {
    pub mode: Mode,
    /// the monitor for full screen (its name; None: the one the window is on)
    pub monitor: Option<String>,
    /// the full-screen mode F11 goes back to
    pub last_full: Mode,
    /// across the monitors: which (their names; empty: all of them). The
    /// view covers the box around them, laid out as the system has them.
    pub screens: Vec<String>,
    /// across the monitors: the one the menus and instruments are on (None: the system's main one)
    pub ui_screen: Option<String>,
    /// across wide screens, the widest horizontal field of view (degrees)
    pub wide_hfov: f32,
}

pub const WIDE_HFOV_DEFAULT: f32 = 110.0;

impl Default for Display {
    fn default() -> Self {
        Display { mode: Mode::Windowed, monitor: None, last_full: Mode::Fullscreen, screens: Vec::new(), ui_screen: None, wide_hfov: WIDE_HFOV_DEFAULT }
    }
}

fn path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("fablegame").join("display.json"))
}

impl Display {
    pub fn load() -> Self {
        let j: serde_json::Value = path().and_then(|p| std::fs::read(p).ok()).and_then(|d| serde_json::from_slice(&d).ok()).unwrap_or_default();
        Self::from_json(&j)
    }
    fn from_json(j: &serde_json::Value) -> Self {
        let d = Self::default();
        Display {
            mode: j["mode"].as_str().and_then(Mode::parse).unwrap_or(d.mode),
            monitor: j["monitor"].as_str().map(str::to_string).filter(|s| !s.is_empty()),
            last_full: j["last_full"].as_str().and_then(Mode::parse).filter(|m| *m != Mode::Windowed).unwrap_or(d.last_full),
            ui_screen: j["ui_screen"].as_str().map(str::to_string).filter(|s| !s.is_empty()),
            screens: j["screens"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).take(32).collect()).unwrap_or_default(),
            wide_hfov: j["wide_hfov"].as_f64().filter(|x| x.is_finite()).map_or(d.wide_hfov, |x| (x as f32).clamp(70.0, 150.0)),
        }
    }
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({ "mode": self.mode.name(), "monitor": self.monitor, "last_full": self.last_full.name(), "screens": self.screens, "ui_screen": self.ui_screen, "wide_hfov": self.wide_hfov })
    }
    pub fn save(&self) {
        let Some(p) = path() else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = p.with_extension("json.tmp");
        if std::fs::write(&tmp, serde_json::to_vec_pretty(&self.to_json()).unwrap_or_default()).is_ok() {
            let _ = std::fs::rename(&tmp, p);
        }
    }

    /// F11: full screen (as last chosen) from a window, a window from full screen.
    pub fn toggle(&mut self) {
        if self.mode == Mode::Windowed {
            self.mode = self.last_full;
        } else {
            self.last_full = self.mode;
            self.mode = Mode::Windowed;
        }
    }

    /// Puts the window as the mode says.
    pub fn apply(&self, w: &Window) {
        match self.mode {
            Mode::Windowed => {
                w.set_fullscreen(None);
                w.set_decorations(true);
            }
            Mode::Fullscreen => {
                let m = self.monitor.as_ref().and_then(|name| w.available_monitors().find(|m| m.name().as_deref() == Some(name.as_str())));
                w.set_fullscreen(Some(Fullscreen::Borderless(m.or_else(|| w.current_monitor()))));
            }
            Mode::Span => {
                w.set_fullscreen(None);
                w.set_decorations(false);
                if let Some((p, s)) = span_box(&self.chosen(&layout(w.available_monitors()))) {
                    w.set_outer_position(p);
                    let _ = w.request_inner_size(s);
                }
            }
        }
    }

    /// The monitors the view spans: those chosen that are there, else all.
    pub fn chosen(&self, all: &[Screen]) -> Vec<Screen> {
        let picked: Vec<Screen> = all.iter().filter(|m| self.screens.contains(&m.name)).cloned().collect();
        if picked.is_empty() { all.to_vec() } else { picked }
    }

    /// The monitors to choose from: name, and "name · width×height".
    pub fn monitors(w: &Window) -> Vec<(String, String)> {
        w.available_monitors().filter_map(|m| {
            let name = m.name()?;
            let s = m.size();
            Some((name.clone(), format!("{name} · {}×{}", s.width, s.height)))
        }).collect()
    }
}

/// A monitor as the system lays them out (pixels).
#[derive(Clone, Debug, PartialEq)]
pub struct Screen {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

/// The monitors, laid out.
pub fn layout(mons: impl Iterator<Item = winit::monitor::MonitorHandle>) -> Vec<Screen> {
    mons.enumerate()
        .map(|(i, m)| {
            let (p, s) = (m.position(), m.size());
            Screen { name: m.name().unwrap_or_else(|| format!("screen {}", i + 1)), x: p.x, y: p.y, w: s.width, h: s.height }
        })
        .collect()
}

/// The box around some monitors: where, and how big.
pub fn span_box(screens: &[Screen]) -> Option<(winit::dpi::PhysicalPosition<i32>, winit::dpi::PhysicalSize<u32>)> {
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for m in screens {
        x0 = x0.min(m.x);
        y0 = y0.min(m.y);
        x1 = x1.max(m.x + m.w as i32);
        y1 = y1.max(m.y + m.h as i32);
    }
    (x1 > x0 && y1 > y0).then(|| (winit::dpi::PhysicalPosition::new(x0, y0), winit::dpi::PhysicalSize::new((x1 - x0) as u32, (y1 - y0) as u32)))
}

/// Under X11: asks the window manager for full screen over all the monitors
/// (EWMH `_NET_WM_FULLSCREEN_MONITORS`), so that the window covers them
/// whole, panels included, as a full-screen game should.
#[cfg(target_os = "linux")]
pub fn x11_fullscreen_all(w: &Window, chosen: &[Screen]) -> Result<(), String> {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use x11rb::connection::Connection;
    use x11rb::protocol::xinerama::ConnectionExt as _;
    use x11rb::protocol::xproto::{AtomEnum, ClientMessageEvent, ConnectionExt as _, EventMask};
    let win = match w.window_handle().map_err(|e| e.to_string())?.as_raw() {
        RawWindowHandle::Xlib(h) => h.window as u32,
        RawWindowHandle::Xcb(h) => h.window.get(),
        _ => return Err("not an X11 window".into()),
    };
    let (conn, screen) = x11rb::connect(None).map_err(|e| e.to_string())?;
    let root = conn.setup().roots[screen].root;
    // Xinerama's screens, which the hint counts in: the topmost, bottommost, leftmost and rightmost.
    let all = conn.xinerama_query_screens().map_err(|e| e.to_string())?.reply().map_err(|e| e.to_string())?.screen_info;
    // (Only the chosen monitors, found by where they are.)
    let screens: Vec<(usize, &x11rb::protocol::xinerama::ScreenInfo)> = all.iter().enumerate().filter(|(_, s)| chosen.iter().any(|c| c.x == s.x_org as i32 && c.y == s.y_org as i32 && c.w == s.width as u32 && c.h == s.height as u32)).collect();
    if screens.len() < 2 {
        return Ok(());
    }
    let pick = |key: &dyn Fn(&x11rb::protocol::xinerama::ScreenInfo) -> i32, max: bool| -> u32 {
        let it = screens.iter();
        (if max { it.max_by_key(|(_, s)| key(s)) } else { it.min_by_key(|(_, s)| key(s)) }).map_or(0, |(i, _)| *i as u32)
    };
    let (top, bottom) = (pick(&|s| s.y_org as i32, false), pick(&|s| s.y_org as i32 + s.height as i32, true));
    let (left, right) = (pick(&|s| s.x_org as i32, false), pick(&|s| s.x_org as i32 + s.width as i32, true));
    let atom = |n: &str| -> Result<u32, String> { Ok(conn.intern_atom(false, n.as_bytes()).map_err(|e| e.to_string())?.reply().map_err(|e| e.to_string())?.atom) };
    let (monitors, state, full) = (atom("_NET_WM_FULLSCREEN_MONITORS")?, atom("_NET_WM_STATE")?, atom("_NET_WM_STATE_FULLSCREEN")?);
    let mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;
    let send = |t: u32, data: [u32; 5]| -> Result<(), String> {
        let ev = ClientMessageEvent::new(32, win, t, data);
        conn.send_event(false, root, mask, ev).map_err(|e| e.to_string())?;
        Ok(())
    };
    send(monitors, [top, bottom, left, right, 1])?;
    send(state, [1, full, 0, 1, 0])?;
    let _ = AtomEnum::NONE;
    conn.flush().map_err(|e| e.to_string())?;
    Ok(())
}

/// Whether to run through X11 (XWayland) rather than Wayland: when the
/// window is to span the monitors, which only X11 lets a program arrange,
/// and an X server is there.
pub fn wants_x11(d: &Display) -> bool {
    d.mode == Mode::Span && std::env::var_os("WAYLAND_DISPLAY").is_some() && std::env::var_os("DISPLAY").is_some()
}

/// The vertical field of view to draw with (radians): `vfov`, narrowed when
/// the window is so wide that the horizontal angle would pass `max_hfov`.
pub fn fit_fov(vfov: f32, aspect: f32, max_hfov_deg: f32) -> f32 {
    let h = 2.0 * ((vfov * 0.5).tan() * aspect).atan();
    let hmax = max_hfov_deg.to_radians();
    if h <= hmax {
        vfov
    } else {
        2.0 * ((hmax * 0.5).tan() / aspect).atan()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_toggle() {
        let d = Display { mode: Mode::Span, monitor: Some("DP-3".into()), last_full: Mode::Span, screens: vec!["DP-3".into(), "HDMI-A-1".into()], ui_screen: Some("HDMI-A-1".into()), wide_hfov: 120.0 };
        assert_eq!(Display::from_json(&d.to_json()), d);
        assert_eq!(Display::from_json(&serde_json::json!({ "wide_hfov": 999.0, "mode": "nope" })).wide_hfov, 150.0);
        let mut d = Display::default();
        d.toggle();
        assert_eq!(d.mode, Mode::Fullscreen);
        d.mode = Mode::Span;
        d.toggle();
        assert_eq!((d.mode, d.last_full), (Mode::Windowed, Mode::Span));
        d.toggle();
        assert_eq!(d.mode, Mode::Span);
    }

    #[test]
    fn the_view_covers_the_chosen_screens() {
        // Six screens, three across and two high; the top row is chosen.
        let all: Vec<Screen> = (0..6).map(|i| Screen { name: format!("S{i}"), x: (i % 3) * 1920, y: (i / 3) * 1080, w: 1920, h: 1080 }).collect();
        let mut d = Display { screens: vec!["S0".into(), "S1".into(), "S2".into()], ..Display::default() };
        let (p, s) = span_box(&d.chosen(&all)).unwrap();
        assert_eq!((p.x, p.y, s.width, s.height), (0, 0, 5760, 1080));
        // None chosen (or none of them there any more): all six, 5760x2160.
        d.screens = vec!["gone".into()];
        let (_, s) = span_box(&d.chosen(&all)).unwrap();
        assert_eq!((s.width, s.height), (5760, 2160));
    }

    #[test]
    fn wide_windows_keep_a_sane_angle() {
        let v = 60f32.to_radians();
        // 16:9 keeps its 60 degrees; 32:9 is narrowed to 110 degrees across.
        assert_eq!(fit_fov(v, 16.0 / 9.0, 110.0), v);
        let f = fit_fov(v, 32.0 / 9.0, 110.0);
        let h = 2.0 * ((f * 0.5).tan() * 32.0 / 9.0).atan();
        assert!((h.to_degrees() - 110.0).abs() < 0.01 && f < v, "{}", f.to_degrees());
    }
}
