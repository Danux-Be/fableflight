# The moment-based flight model (6 DOF)

FableFlight has two ways of turning the air into rotation:

- **Handling** (`aircraft::Handling`, the original model): angular
  accelerations written directly from dynamic pressure, with the stick
  commanding an angle of attack around an auto-trimmed value. Every type
  without an `aero` block still flies this way.
- **Moments** (`aircraft::Aero`, `src/plane/moments.rs`): aerodynamic
  moments built up from stability and control derivatives and integrated
  through Euler's rigid-body equations. A type opts in by setting
  `aero: Some(...)` in its `Spec`. The Cessna 172RG is the first.

The translational side (lift, drag, thrust, gravity, ground contact) is
shared: `plane::lift_drag` gives the polar, and the moment model adds its own
increments to it.

## Axes and signs

The game's body frame is +X right, +Y up, −Z forward. Derivatives use the
usual aerodynamic body axes (x forward, y right, z down), so
`moments::to_aero` / `from_aero` map between them:

| aero axes | game frame |
|---|---|
| p (roll, right wing down +) | −ω.z |
| q (pitch, nose up +) | ω.x |
| r (yaw, nose right +) | −ω.y |
| L, M, N | torque.z = −L, torque.x = M, torque.y = −N |

Control deflections: elevator δe positive trailing edge down (Cmδe < 0);
aileron δa positive with the right aileron up (rolls right, Clδa > 0); rudder
δr positive trailing edge left (yaws left, Cnδr < 0). Rates are
non-dimensional: p̂ = p·b/2V, q̂ = q·c/2V, r̂ = r·b/2V, α̇̂ = α̇·c/2V.

## What is modelled

**Pitch**

    Cm = Cm0 + Cmα·sin α + Cm_flap·δf + Cmq·q̂ + Cmα̇·α̇̂ + Cmδe·δe·η + CL·(h − h_ref) + Cm_stall

- `h − h_ref`: the CG (from load and fuel) against the derivatives'
  reference CG. An aft CG loses static margin through the lift's own arm,
  so there are no hand-written CG multipliers. The neutral point is
  `h_ref − Cmα/CLα`.
- `δe` is the stick's deflection plus the trim tab's. The tab is set from
  `Plane::trim`, which is still read as an angle of attack (the one the tab
  holds clean at the reference CG). The trim wheel, copilot and HUD are
  therefore unchanged, while flaps, CG and power move the trimmed state as
  they do on a real aircraft.
- `η = 1 + k·(T/A)/q̄`: the propeller slipstream over the tail
  (`prop_wash`).
- If the tailplane is lost, its share of Cmα (`pitch_a_tail`) and most of
  Cmq and Cmα̇ go with it. What remains is unstable.

**Roll**

    Cl = Cl0 + Clβ·sin β + Clp·p̂·(1 − 1.5·stall) + (Clr + Clr_stall)·r̂ + Clδa·δa + Clδr·δr − Cl_drop·stall

Roll damping fades and then reverses through the stall (autorotation), and
one wing drops first.

**Yaw**

    Cn = Cn0 + Cnβ·sin β + Cnp·p̂ + Cnr·r̂ + Cnδa·δa + Cnδr·δr·η

If the fin is lost, its share of Cnβ (`yaw_b_fin`) goes with it.

**Lift and side-force increments**

    ΔCL = CLq·q̂ + CLα̇·α̇̂ + CLδe·δe
    CY  = CYβ·sin β + CYp·p̂ + CYr·r̂ + CYδr·δr

**Propeller** (`moments::propeller`):

- the reaction to the torque the propeller absorbs (a left roll for a
  clockwise propeller);
- slipstream swirl and P-factor yaw (`prop_yaw_arm`, `prop_p_factor`);
- the thrust line's offset from the CG (`thrust_y`);
- the gyroscopic moment of the spinning propeller, −ω × h.

`Cl0` and `Cn0` are the rigging that cancels the propeller's torque and swirl
at cruise power.

**Rigid body** (`moments::euler`): I·ω̇ = M − ω × (I·ω), with the Ixz product
of inertia. The moments from ground contact go through the same equations.
The rate cap is 15 rad/s and only guards the integrator in a tumbling crash.

## Pilot aids

- **Auto-trim** works as before. While the stick is held, it winds the tab
  toward the 1 g angle of attack.
- **Auto-rudder** (`Plane::auto_rudder`, on by default, airborne only, only
  while the pedals are untouched) works the rudder against the sideslip and
  the yaw rate. It centres the ball and damps the Dutch roll, standing in for
  a pilot's feet when the player has no pedals. Turn it off to fly the bare
  airframe. There is no settings toggle for it yet.

## Validation (`src/plane/modes.rs`)

These tests fly the full nonlinear simulation hands off, disturb it, and fit
the free response with Prony's method. The results are compared with the
classical approximations (Nelson, *Flight Stability and Automatic Control*)
built from the same derivatives, at the source data's cruise (5000 ft,
219 ft/s):

| mode | simulation | approximation |
|---|---|---|
| short period | ωn 6.25 rad/s, ζ 0.76 | 6.24 rad/s, 0.70 |
| phugoid | 37 s, ζ 0.085 | 30 s (Lanchester), 0.096 |
| Dutch roll | ωn 3.37 rad/s (1.9 s), ζ 0.22 | 3.21 rad/s, 0.22 |
| roll subsidence | τ 0.076 s | 0.079 s |

Lanchester's phugoid period is known to be short, because it ignores
propeller and compressibility effects. Other tests check the following:

- the trimmed angle moves with the CG;
- the neutral point lies aft of the aft limit;
- the auto-rudder damps the Dutch roll;
- the aircraft holds a hands-off cruise;
- it flares and lands on its mains.

The existing performance checks still hold: top speed, cruise at 8000 ft,
climb, glide ratio, stall, the approach, and the autopilot tests.

These checks show that the simulation reproduces the dynamics its derivatives
imply. They are not a substitute for flight-test data for the type.

## Adding a type

1. Find the derivatives for the exact variant: Roskam (*Airplane Flight
   Dynamics*, appendix B), Nelson appendix B, NASA reports, or a published
   JSBSim/LaRCsim set. Record the sources and the flight condition in the
   aircraft's comments, as `aircraft/c172/mod.rs` does. Mark estimates as
   estimates.
2. Make sure `inertia` (pitch, yaw, roll) and `mac`, `wing_area` and `b`
   match the source's reference geometry. Set `cg_ref` to the CG of the
   derivatives.
3. Convert sign conventions to the ones above. Aileron signs in particular
   differ between sources.
4. Add the type to a copy of the mode tests, and check them against its
   own approximations and any flight-test figures.
5. Tune `Cl0`/`Cn0` (and, for a jet, `thrust_y`) so it flies straight hands
   off at cruise power.

The autopilot scales its pitch servo by `Spec::alpha_per_stick`, so it adapts
to a type's elevator power. Imported FlightGear types keep the handling
model (`hangar.rs` clears `aero`) until they get their own derivatives.
