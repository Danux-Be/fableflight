//! the normal checklists, one set per family of aircraft, after the order of
//! the makers' handbooks (cessna 172rg poh section 4, the robin dr400 flight
//! manual, boeing's 747-400 normal checklist, the f/a-18 natops pocket
//! checklist) and cut down to what the sim models. items the sim can see tick
//! themselves; the rest are ticked by hand. for the game, not for real flying.

use crate::aircraft::Spec;
use crate::plane::{FuelSelector, Plane};
use crate::{flightui, menu, tr_en, Game};
use egui::{pos2, vec2, Color32, Rect, Sense, Stroke};

/// what the sim can tell of an item.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Is {
    /// ticked by hand.
    Hand,
    ParkingBrake(bool),
    Master(bool),
    Ignition(bool),
    Running(bool),
    MixtureRich,
    MixtureCutoff,
    FuelOn,
    ThrottleIdle,
    ThrottleFull,
    FlapsUp,
    /// a takeoff setting: past up, short of full.
    FlapsTakeoff,
    FlapsFull,
    GearDown,
    GearUp,
    Lights(bool),
    CanopyOpen(bool),
    SpeedbrakeIn,
    Autopilot(bool),
    PropHigh,
    CowlOpen,
    Airborne,
    Stopped,
    ReversersStowed,
    CarbHeat(bool),
    PitotHeat(bool),
}

/// what an item is checked against.
pub struct Ctx<'a> {
    pub plane: &'a Plane,
    pub ap: bool,
}

impl Is {
    /// whether the sim sees the item done; none for one ticked by hand.
    pub fn met(self, c: &Ctx) -> Option<bool> {
        let p = c.plane;
        let max = p.spec.max_notch();
        Some(match self {
            Is::Hand => return None,
            Is::ParkingBrake(on) => p.parking_brake == on,
            Is::Master(on) => p.sys.elec.master == on,
            Is::Ignition(on) => p.ignition == on,
            Is::Running(on) => p.running == on,
            Is::MixtureRich => p.mixture > 0.9,
            Is::MixtureCutoff => p.mixture < 0.05,
            Is::FuelOn => p.fuel_selector != FuelSelector::Off,
            Is::ThrottleIdle => p.throttle < 0.08,
            Is::ThrottleFull => p.throttle > 0.95,
            Is::FlapsUp => p.flap_set == 0,
            Is::FlapsTakeoff => p.flap_set > 0 && (p.flap_set < max || max == 1),
            Is::FlapsFull => p.flap_set == max,
            Is::GearDown => p.gear_down || p.spec.fixed_gear(),
            Is::GearUp => !p.gear_down || p.spec.fixed_gear(),
            Is::Lights(on) => p.landing_lights == on,
            Is::CanopyOpen(open) => p.canopy_open == open,
            Is::SpeedbrakeIn => p.speedbrake < 0.05,
            Is::Autopilot(on) => c.ap == on,
            Is::PropHigh => p.propeller > 0.95,
            Is::CowlOpen => p.cowl_flaps > 0.9,
            Is::Airborne => !p.on_ground,
            Is::Stopped => p.on_ground && p.vel.length() < 0.5,
            Is::ReversersStowed => p.reverser < 0.05,
            Is::CarbHeat(on) => p.carb_heat == on,
            Is::PitotHeat(on) => p.pitot_heat == on,
        })
    }
}

/// one line: the item, what it should be, and how the sim tells.
pub struct Item {
    pub what: &'static str,
    pub state: &'static str,
    pub is: Is,
}

pub struct List {
    pub name: &'static str,
    pub items: &'static [Item],
}

const fn i(what: &'static str, state: &'static str, is: Is) -> Item {
    Item { what, state, is }
}

/// the 172rg cutlass (poh section 4).
static PISTON_RG: &[List] = &[
    List { name: "Before starting engine", items: &[
        i("Preflight inspection", "COMPLETE", Is::Hand),
        i("Seats and belts", "ADJUST, LOCK", Is::Hand),
        i("Brakes", "TEST, SET", Is::ParkingBrake(true)),
        i("Landing gear lever", "DOWN", Is::GearDown),
        i("Fuel selector", "BOTH", Is::FuelOn),
        i("Avionics", "OFF", Is::Hand),
    ] },
    List { name: "Starting engine", items: &[
        i("Mixture", "RICH", Is::MixtureRich),
        i("Propeller", "HIGH RPM", Is::PropHigh),
        i("Throttle", "OPEN 1/4 INCH", Is::Hand),
        i("Master switch", "ON", Is::Master(true)),
        i("Propeller area", "CLEAR", Is::Hand),
        i("Ignition", "START, then BOTH", Is::Running(true)),
        i("Oil pressure", "CHECK", Is::Hand),
    ] },
    List { name: "Before takeoff", items: &[
        i("Parking brake", "SET", Is::ParkingBrake(true)),
        i("Flight controls", "FREE AND CORRECT", Is::Hand),
        i("Flight instruments", "SET", Is::Hand),
        i("Fuel selector", "BOTH", Is::FuelOn),
        i("Mixture", "RICH", Is::MixtureRich),
        i("Elevator trim", "TAKEOFF", Is::Hand),
        i("Run up, magnetos", "CHECK at 1700 RPM", Is::Hand),
        i("Propeller", "HIGH RPM", Is::PropHigh),
        i("Cowl flaps", "OPEN", Is::CowlOpen),
        i("Wing flaps", "UP to 10°", Is::Hand),
        i("Pitot heat", "ON in visible moisture", Is::Hand),
        i("Landing light", "ON", Is::Lights(true)),
    ] },
    List { name: "Takeoff", items: &[
        i("Parking brake", "RELEASE", Is::ParkingBrake(false)),
        i("Power", "FULL THROTTLE, 2700 RPM", Is::ThrottleFull),
        i("Rotate", "55 KIAS", Is::Airborne),
        i("Landing gear", "RETRACT in climb", Is::GearUp),
    ] },
    List { name: "Climb", items: &[
        i("Airspeed", "75 to 85 KIAS", Is::Hand),
        i("Wing flaps", "UP", Is::FlapsUp),
        i("Power", "25 IN, 2500 RPM", Is::Hand),
        i("Cowl flaps", "OPEN as required", Is::Hand),
    ] },
    List { name: "Cruise", items: &[
        i("Power", "15 to 25 IN, 2100 to 2600 RPM", Is::Hand),
        i("Cowl flaps", "CLOSED", Is::Hand),
        i("Mixture", "LEAN", Is::Hand),
    ] },
    List { name: "Descent", items: &[
        i("Altimeter", "SET", Is::Hand),
        i("Mixture", "ENRICHEN as needed", Is::Hand),
        i("Cowl flaps", "CLOSED", Is::Hand),
    ] },
    List { name: "Before landing", items: &[
        i("Seats and belts", "SECURE", Is::Hand),
        i("Fuel selector", "BOTH", Is::FuelOn),
        i("Landing gear", "DOWN, green light", Is::GearDown),
        i("Mixture", "RICH", Is::MixtureRich),
        i("Propeller", "HIGH RPM", Is::PropHigh),
        i("Autopilot", "OFF", Is::Autopilot(false)),
        i("Landing light", "ON", Is::Lights(true)),
        i("Wing flaps", "AS DESIRED", Is::Hand),
    ] },
    List { name: "After landing", items: &[
        i("Wing flaps", "UP", Is::FlapsUp),
        i("Cowl flaps", "OPEN", Is::CowlOpen),
    ] },
    List { name: "Securing airplane", items: &[
        i("Parking brake", "SET", Is::ParkingBrake(true)),
        i("Throttle", "IDLE", Is::ThrottleIdle),
        i("Mixture", "IDLE CUT OFF", Is::MixtureCutoff),
        i("Ignition", "OFF", Is::Ignition(false)),
        i("Master switch", "OFF", Is::Master(false)),
    ] },
];

/// the robin dr400 (flight manual, normal procedures).
static PISTON_FIXED: &[List] = &[
    List { name: "Before starting engine", items: &[
        i("Preflight inspection", "COMPLETE", Is::Hand),
        i("Harnesses", "FASTENED", Is::Hand),
        i("Parking brake", "ON", Is::ParkingBrake(true)),
        i("Fuel selector", "MAIN TANK", Is::FuelOn),
        i("Mixture", "RICH", Is::MixtureRich),
    ] },
    List { name: "Starting engine", items: &[
        i("Throttle", "OPEN 1 CM", Is::Hand),
        i("Master switch", "ON", Is::Master(true)),
        i("Propeller area", "CLEAR", Is::Hand),
        i("Starter", "ENGAGE", Is::Running(true)),
        i("Oil pressure", "RISING", Is::Hand),
    ] },
    List { name: "Before takeoff", items: &[
        i("Run up, magnetos", "CHECK at 2000 RPM", Is::Hand),
        i("Carburettor heat", "CHECK, then COLD", Is::CarbHeat(false)),
        i("Flaps", "TAKEOFF", Is::FlapsTakeoff),
        i("Trim", "TAKEOFF", Is::Hand),
        i("Fuel pump", "ON", Is::Hand),
        i("Canopy", "CLOSED AND LOCKED", Is::CanopyOpen(false)),
        i("Landing light", "ON", Is::Lights(true)),
    ] },
    List { name: "Takeoff", items: &[
        i("Parking brake", "OFF", Is::ParkingBrake(false)),
        i("Throttle", "FULL", Is::ThrottleFull),
        i("Rotate", "55 KT", Is::Airborne),
        i("Flaps", "UP above 300 FT", Is::FlapsUp),
    ] },
    List { name: "Before landing", items: &[
        i("Fuel selector", "FULLEST TANK", Is::FuelOn),
        i("Fuel pump", "ON", Is::Hand),
        i("Mixture", "RICH", Is::MixtureRich),
        i("Carburettor heat", "HOT", Is::CarbHeat(true)),
        i("Flaps", "LANDING", Is::FlapsFull),
    ] },
    List { name: "After landing", items: &[
        i("Flaps", "UP", Is::FlapsUp),
        i("Carburettor heat", "COLD", Is::CarbHeat(false)),
        i("Fuel pump", "OFF", Is::Hand),
    ] },
    List { name: "Engine shutdown", items: &[
        i("Parking brake", "ON", Is::ParkingBrake(true)),
        i("Throttle", "IDLE", Is::ThrottleIdle),
        i("Mixture", "IDLE CUT OFF", Is::MixtureCutoff),
        i("Magnetos", "OFF", Is::Ignition(false)),
        i("Master switch", "OFF", Is::Master(false)),
    ] },
];

/// airliners and business jets (boeing's normal checklist order).
static JET: &[List] = &[
    List { name: "Before start", items: &[
        i("Flight deck preparation", "COMPLETE", Is::Hand),
        i("Battery", "ON", Is::Master(true)),
        i("Parking brake", "SET", Is::ParkingBrake(true)),
        i("Gear lever", "DOWN", Is::GearDown),
        i("Fuel control switches", "CUTOFF", Is::Running(false)),
        i("Takeoff speeds", "SET", Is::Hand),
    ] },
    List { name: "Engine start", items: &[
        i("Engine area", "CLEAR", Is::Hand),
        i("Starters", "START", Is::Running(true)),
        i("Engine indications", "STABLE AT IDLE", Is::Hand),
    ] },
    List { name: "Before taxi", items: &[
        i("Flaps", "TAKEOFF", Is::FlapsTakeoff),
        i("Flight controls", "CHECKED", Is::Hand),
        i("Autobrakes", "RTO", Is::Hand),
        i("Transponder", "TA/RA", Is::Hand),
    ] },
    List { name: "Before takeoff", items: &[
        i("Probe heat", "ON", Is::PitotHeat(true)),
        i("Anti ice", "AS REQUIRED", Is::Hand),
        i("Speedbrake", "DOWN", Is::SpeedbrakeIn),
        i("Landing lights", "ON", Is::Lights(true)),
        i("Parking brake", "RELEASED", Is::ParkingBrake(false)),
        i("Takeoff clearance", "RECEIVED", Is::Hand),
    ] },
    List { name: "After takeoff", items: &[
        i("Landing gear", "UP", Is::GearUp),
        i("Flaps", "UP", Is::FlapsUp),
        i("Autopilot", "ENGAGE", Is::Autopilot(true)),
    ] },
    List { name: "Descent", items: &[
        i("Arrival and approach", "BRIEFED", Is::Hand),
        i("Landing speeds", "SET", Is::Hand),
        i("Altimeters", "SET", Is::Hand),
    ] },
    List { name: "Landing", items: &[
        i("Landing gear", "DOWN", Is::GearDown),
        i("Flaps", "LANDING", Is::FlapsFull),
        i("Speedbrake", "ARMED", Is::Hand),
        i("Autobrakes", "SET", Is::Hand),
    ] },
    List { name: "After landing", items: &[
        i("Speedbrake", "DOWN", Is::SpeedbrakeIn),
        i("Reversers", "STOWED", Is::ReversersStowed),
        i("Flaps", "UP", Is::FlapsUp),
        i("Landing lights", "OFF", Is::Lights(false)),
    ] },
    List { name: "Shutdown", items: &[
        i("Aircraft", "STOPPED at the stand", Is::Stopped),
        i("Parking brake", "SET", Is::ParkingBrake(true)),
        i("Fuel control switches", "CUTOFF", Is::Running(false)),
        i("Battery", "OFF", Is::Master(false)),
    ] },
];

/// concorde: the flap lever is its nose and visor (up, 5°, 12.5°).
static SST: &[List] = &[
    List { name: "Before start", items: &[
        i("Flight deck preparation", "COMPLETE", Is::Hand),
        i("Battery", "ON", Is::Master(true)),
        i("Parking brake", "SET", Is::ParkingBrake(true)),
        i("Fuel control switches", "CUTOFF", Is::Running(false)),
    ] },
    List { name: "Engine start", items: &[
        i("Engines 3, 2, 4, 1", "START", Is::Running(true)),
        i("Reheat", "SELECTED for takeoff", Is::Hand),
    ] },
    List { name: "Before takeoff", items: &[
        i("Visor and nose", "DOWN, 5°", Is::FlapsTakeoff),
        i("Landing lights", "ON", Is::Lights(true)),
        i("Parking brake", "RELEASED", Is::ParkingBrake(false)),
    ] },
    List { name: "Takeoff", items: &[
        i("Throttles", "FULL, reheat lit", Is::ThrottleFull),
        i("Rotate", "195 KT", Is::Airborne),
        i("Landing gear", "UP", Is::GearUp),
    ] },
    List { name: "Climb", items: &[
        i("Visor and nose", "UP", Is::FlapsUp),
        i("Reheat", "OFF until transonic", Is::Hand),
        i("Autopilot", "ENGAGE", Is::Autopilot(true)),
    ] },
    List { name: "Landing", items: &[
        i("Visor and nose", "DOWN, 12.5°", Is::FlapsFull),
        i("Landing gear", "DOWN", Is::GearDown),
        i("Autopilot", "OFF by 500 FT", Is::Hand),
    ] },
    List { name: "After landing", items: &[
        i("Reversers", "STOWED", Is::ReversersStowed),
        i("Visor and nose", "5°", Is::FlapsTakeoff),
        i("Landing lights", "OFF", Is::Lights(false)),
    ] },
    List { name: "Shutdown", items: &[
        i("Parking brake", "SET", Is::ParkingBrake(true)),
        i("Fuel control switches", "CUTOFF", Is::Running(false)),
        i("Battery", "OFF", Is::Master(false)),
    ] },
];

/// fighters (the hornet's pocket checklist, kept to what they share).
static FIGHTER: &[List] = &[
    List { name: "Before start", items: &[
        i("Ejection seat", "ARMED", Is::Hand),
        i("Parking brake", "SET", Is::ParkingBrake(true)),
        i("Battery", "ON", Is::Master(true)),
        i("Gear handle", "DOWN", Is::GearDown),
    ] },
    List { name: "Engine start", items: &[
        i("Intakes", "CLEAR", Is::Hand),
        i("Engines", "CRANK, then IDLE", Is::Running(true)),
        i("Warnings", "CHECKED", Is::Hand),
    ] },
    List { name: "Before taxi", items: &[
        i("Flaps", "HALF", Is::FlapsTakeoff),
        i("Trim", "TAKEOFF", Is::Hand),
        i("Canopy", "CLOSED", Is::CanopyOpen(false)),
        i("Speedbrake", "IN", Is::SpeedbrakeIn),
    ] },
    List { name: "Takeoff", items: &[
        i("Parking brake", "OFF", Is::ParkingBrake(false)),
        i("Throttles", "MIL or MAX", Is::ThrottleFull),
        i("Rotate", "ON SPEED", Is::Airborne),
        i("Landing gear", "UP", Is::GearUp),
        i("Flaps", "AUTO", Is::FlapsUp),
    ] },
    List { name: "Landing", items: &[
        i("Landing gear", "DOWN", Is::GearDown),
        i("Flaps", "FULL", Is::FlapsFull),
        i("Speedbrake", "IN", Is::SpeedbrakeIn),
        i("Approach speed", "ON SPEED AOA", Is::Hand),
    ] },
    List { name: "After landing", items: &[
        i("Flaps", "AUTO", Is::FlapsUp),
        i("Canopy", "OPEN", Is::CanopyOpen(true)),
    ] },
    List { name: "Shutdown", items: &[
        i("Parking brake", "SET", Is::ParkingBrake(true)),
        i("Throttles", "OFF", Is::Running(false)),
        i("Battery", "OFF", Is::Master(false)),
    ] },
];

/// a glider (the ask 21's manual and the cb sift cbe drill before a launch).
static GLIDER: &[List] = &[
    List { name: "Before launch", items: &[
        i("Controls", "FREE AND CORRECT", Is::Hand),
        i("Ballast", "CHECKED", Is::Hand),
        i("Straps", "TIGHT", Is::Hand),
        i("Instruments", "SET", Is::Hand),
        i("Trim", "SET for the launch", Is::Hand),
        i("Canopy", "CLOSED AND LOCKED", Is::CanopyOpen(false)),
        i("Airbrakes", "CLOSED AND LOCKED", Is::SpeedbrakeIn),
        i("Eventualities", "BRIEFED: a cable break", Is::Hand),
    ] },
    List { name: "Launch", items: &[
        i("Wheel brake", "OFF", Is::ParkingBrake(false)),
        i("Airborne", "HOLD the attitude", Is::Airborne),
        i("Release", "AT THE TOP", Is::Hand),
    ] },
    List { name: "Before landing", items: &[
        i("Approach speed", "55 KT plus half the wind", Is::Hand),
        i("Airbrakes", "UNLOCKED, as needed", Is::Hand),
        i("Field", "CHOSEN, circuit planned", Is::Hand),
    ] },
    List { name: "After landing", items: &[
        i("Stopped", "WINGS LEVEL as it slows", Is::Stopped),
        i("Canopy", "OPEN", Is::CanopyOpen(true)),
    ] },
];

/// a piston helicopter (the r44's pilot's operating handbook, section 4, cut down).
static HELI: &[List] = &[
    List { name: "Before starting engine", items: &[
        i("Seat belts", "FASTENED", Is::Hand),
        i("Collective", "FULL DOWN, friction on", Is::ThrottleIdle),
        i("Cyclic and pedals", "NEUTRAL", Is::Hand),
        i("Battery", "ON", Is::Master(true)),
    ] },
    List { name: "Starting engine", items: &[
        i("Rotor area", "CLEAR", Is::Hand),
        i("Key", "START, release when it fires", Is::Running(true)),
        i("Clutch", "ENGAGE, rotor turning", Is::Hand),
        i("Governor", "ON, rotor 101 to 102 %", Is::Hand),
    ] },
    List { name: "Before takeoff", items: &[
        i("Doors", "LATCHED", Is::Hand),
        i("Warning lights", "OUT", Is::Hand),
        i("Rotor speed", "102 %", Is::Hand),
    ] },
    List { name: "Takeoff", items: &[
        i("Collective", "RAISE SLOWLY to a hover", Is::Airborne),
        i("Hover check", "WEIGHT, BALANCE, POWER", Is::Hand),
        i("Transition", "NOSE DOWN, accelerate through 15 KT", Is::Hand),
    ] },
    List { name: "Shutdown", items: &[
        i("Collective", "FULL DOWN", Is::ThrottleIdle),
        i("Key", "OFF", Is::Ignition(false)),
        i("Battery", "OFF", Is::Master(false)),
    ] },
];

/// the checklists for a type: by its own kind, or the body it borrows.
pub fn lists(spec: &Spec) -> &'static [List] {
    let body = spec.origin.map(|o| o.body).unwrap_or(spec.keys.first().copied().unwrap_or(""));
    match spec.id {
        _ if spec.rocket().is_some() => &[],
        _ if spec.rotor.is_some() => HELI,
        _ if spec.fuel_kg <= 0.0 => GLIDER,
        0 => PISTON_RG,
        8 => PISTON_FIXED,
        1 | 9 => JET,
        6 => SST,
        2 | 3 | 4 | 7 => FIGHTER,
        _ if body.contains("747") || body.contains("650") => JET,
        _ if spec.is_jet() => FIGHTER,
        _ if spec.fixed_gear() => PISTON_FIXED,
        _ => PISTON_RG,
    }
}

/// the ticks made by hand, and the list showing.
#[derive(Default)]
pub struct Checklists {
    pub list: usize,
    ticks: Vec<Vec<bool>>,
    /// the lists the ticks belong to (their address).
    of: usize,
}

impl Checklists {
    /// starts over for another type, or a new flight.
    pub fn reset(&mut self, lists: &'static [List]) {
        self.list = 0;
        self.ticks = lists.iter().map(|l| vec![false; l.items.len()]).collect();
        self.of = lists.as_ptr() as usize;
    }

    fn sync(&mut self, lists: &'static [List]) {
        if self.of != lists.as_ptr() as usize || self.ticks.len() != lists.len() {
            self.reset(lists);
        }
        self.list = self.list.min(lists.len().saturating_sub(1));
    }

    fn ticked(&self, l: usize, k: usize) -> bool {
        self.ticks.get(l).and_then(|t| t.get(k)).copied().unwrap_or(false)
    }

    fn toggle(&mut self, l: usize, k: usize) {
        if let Some(t) = self.ticks.get_mut(l).and_then(|t| t.get_mut(k)) {
            *t = !*t;
        }
    }
}

impl Game {
    fn check_ctx(&self) -> Ctx<'_> {
        Ctx { plane: &self.plane, ap: self.ap.on }
    }

    /// the checklist page of the flight panel.
    pub(crate) fn checklist_page(&mut self, ui: &mut egui::Ui) {
        use flightui::{key, note, tag, LIT, SOFT};
        let lists = lists(self.spec);
        self.checks.sync(lists);
        if lists.is_empty() {
            note(ui, tr_en("This aircraft flies without a checklist here."));
            return;
        }
        let l = self.checks.list;
        let list = &lists[l];
        let ctx = self.check_ctx();
        let done: Vec<(bool, Option<bool>)> = list.items.iter().enumerate().map(|(k, it)| {
            let met = it.is.met(&ctx);
            (met == Some(true) || self.checks.ticked(l, k), met)
        }).collect();
        let n_done = done.iter().filter(|d| d.0).count();
        let mut go: Option<usize> = None;
        let mut flip: Option<usize> = None;
        let mut reset = false;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            if key(ui, "‹", false, 30.0, 26.0).clicked() && l > 0 {
                go = Some(l - 1);
            }
            ui.vertical(|ui| {
                tag(ui, &format!("{} {} / {}", tr_en("CHECKLIST"), l + 1, lists.len()));
                ui.label(egui::RichText::new(tr_en(list.name)).font(menu::semibold(17.0)).color(Color32::WHITE));
            });
            menu::at_right(ui, "check_nav", |ui| {
                if key(ui, "›", false, 30.0, 26.0).clicked() && l + 1 < lists.len() {
                    go = Some(l + 1);
                }
                if key(ui, tr_en("Reset"), false, 64.0, 26.0).clicked() {
                    reset = true;
                }
                let col = if n_done == list.items.len() { LIT } else { SOFT };
                ui.label(egui::RichText::new(format!("{n_done}/{}", list.items.len())).font(menu::semibold(14.0)).color(col));
            });
        });
        ui.add_space(6.0);
        for (k, it) in list.items.iter().enumerate() {
            if item_row(ui, tr_en(it.what), tr_en(it.state), done[k].0, done[k].1.is_some()).clicked() {
                flip = Some(k);
            }
        }
        ui.add_space(6.0);
        if n_done == list.items.len() && l + 1 < lists.len() {
            let next = format!("{}  ›  {}", tr_en("Complete"), tr_en(lists[l + 1].name));
            if key(ui, &next, true, ui.available_width(), 30.0).clicked() {
                go = Some(l + 1);
            }
        } else {
            note(ui, tr_en("Items with a dot tick themselves from the cockpit; click any item to tick it by hand."));
        }
        // every list at a glance, to jump to one.
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            for (j, li) in lists.iter().enumerate() {
                if flightui::chip(ui, tr_en(li.name), false, j == l).clicked() {
                    go = Some(j);
                }
            }
        });
        if let Some(k) = flip {
            self.checks.toggle(l, k);
        }
        if reset {
            self.checks.reset(lists);
        }
        if let Some(j) = go {
            self.checks.list = j;
        }
    }
}

/// one item: a tick box, the item, what it should be; a dot where the sim checks it.
fn item_row(ui: &mut egui::Ui, what: &str, state: &str, done: bool, sim: bool) -> egui::Response {
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, done, what));
    let lit = menu::ease(ui.ctx(), resp.id, resp.hovered() || resp.has_focus());
    let p = ui.painter();
    if lit > 0.0 {
        p.rect_filled(r, 4.0, Color32::from_white_alpha((14.0 * lit) as u8));
    }
    let good = Color32::from_rgb(120, 220, 140);
    let bx = Rect::from_center_size(pos2(r.left() + 16.0, r.center().y), vec2(16.0, 16.0));
    if done {
        p.rect_filled(bx, 4.0, good.gamma_multiply(0.25));
        p.rect_stroke(bx, 4.0, Stroke::new(1.4, good), egui::StrokeKind::Inside);
        p.line(vec![bx.left_center() + vec2(3.5, 0.0), bx.center_bottom() + vec2(-1.0, -4.0), bx.right_top() + vec2(-3.5, 4.0)], Stroke::new(1.8, good));
    } else {
        p.rect_stroke(bx, 4.0, Stroke::new(1.2, Color32::from_white_alpha(120)), egui::StrokeKind::Inside);
    }
    let col = if done { flightui::SOFT } else { Color32::WHITE };
    let right = p.layout_no_wrap(state.to_string(), menu::semibold(13.0), if done { good } else { flightui::LIT });
    let room = (r.width() - 40.0 - right.size().x - 30.0).max(40.0);
    let left = menu::one_line(p, what, menu::caption(), col, room);
    p.galley(pos2(r.left() + 32.0, r.center().y - left.size().y * 0.5), left, col);
    // dotted leader between the item and its state.
    let x0 = r.left() + 32.0 + room.min(p.layout_no_wrap(what.to_string(), menu::caption(), col).size().x) + 8.0;
    let x1 = r.right() - 14.0 - right.size().x - 8.0;
    let mut x = x0;
    while x < x1 {
        p.circle_filled(pos2(x, r.center().y + 4.0), 0.8, Color32::from_white_alpha(60));
        x += 5.0;
    }
    let rw = right.size();
    p.galley(pos2(r.right() - 10.0 - rw.x, r.center().y - rw.y * 0.5), right, col);
    if sim {
        p.circle_filled(pos2(r.right() - 4.0, r.center().y), 2.2, Color32::from_rgb(120, 180, 255));
    }
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_type_has_lists_but_the_rocket() {
        for s in crate::aircraft::BUILTIN {
            assert_eq!(lists(s).is_empty(), s.rocket().is_some(), "{}", s.name);
            for l in lists(s) {
                assert!(!l.items.is_empty(), "{}: {}", s.name, l.name);
            }
        }
    }
}
