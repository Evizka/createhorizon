# Strict simulation math and native boundary

## Simulation policy

- The canonical simulation core is `mixins/createhorizon_native/engine`; it is
  a `no_std` Rust crate and forbids `unsafe`. `create-math` and
  `create-kinetics` are compatibility facades that re-export this same source.
- Simulation values use signed Q32.32 (`Fx`, backed by `i64`); no `f32` or
  `f64` enters the core. Arithmetic saturates. Multiply, divide, and rational
  application round to nearest, with halfway cases rounded away from zero.
- Angles are unsigned Q0.32 turns (`Angle`): a full turn wraps to zero.
  `sin_cos` and `atan2` use 32-step integer CORDIC routines and hard-coded
  Q0.64 turn constants. There are no platform transcendental functions.
- Rational transmission ratios remain reduced integer fractions until applied
  to a fixed-point speed. An unrepresentable composed fraction is an explicit
  error (`None`), never a platform-dependent overflow.
- Ordered maps, sets, sorted component positions and stable tie-breaks define
  all iteration that can affect a simulation result. The core does not use
  `HashMap`, floating-point simulation, `mul_add`, relaxed SIMD or fast-math.
  Future catch-up must advance by repeated ordinary ticks, not one large `dt`.
- `SIM_MATH_VERSION = 1` versions fixed-point/angle conventions and rounding;
  `KINETICS_RULES_VERSION = 1` versions gear contacts and network solving.
  Update golden tests and increment the corresponding version whenever its
  behavior changes.

## Native Horizon adapter

Horizon's pinned source has no runtime for gameplay WASM mods. The user-facing
mod therefore uses the native `.hnmod` source-mixin path plus a separate
content-only `.hmod`; it is not a WASM mod. The adapter targets Horizon
`ba6344a59cb9e755b730dcf02285fc0d33b97d3c`, horizon-mixins
`d9c83d7096b7ea5b83678da2efdfd6161f74997e`, game line `26.3`, and native-hook
API 3. The full pins and source SHA-256 checks are in `compatibility.json`.

The native hooks add the mod to Options > Mods, register two configurable keys,
draw a compact HUD, list two buttons on the mod's settings page, and intercept
`/createhorizon scan`. The crank key (C by default) toggles the nearest hand
crank in the scan radius; it supplies 16 RPM and capacity 16 while enabled, and
inactive hand cranks are passive shafts. The clutch key (V by default) engages
and releases every clutch, and the settings page's two buttons move the same two
toggles. The HUD's four-frame glyph is a 2D status indicator, not an in-world
block animation. The runtime still only reads public block states and does not
mutate blocks, items, or world state. The pinned hook API has no block-render
callback, so animated shaft/cogwheel models need a future renderer integration.

The water wheel is the one part that reads more than a block's name: the four
blocks beside its axle are read for `StateFacts::fluid`, `fluid_level` (1..=8,
8 being a source or a falling column) and `fluid_falling`. Everything the
runtime reads stays inside the pinned API.

The content pack defines eleven simple cube blocks in the `createhorizon:`
namespace. A content block copies a vanilla template and cannot declare
block-state properties, so each block's axle is fixed by its id: the shaft,
cogwheels, hand crank, loads and water wheel turn about Z, the vertical shaft
about Y, the gearbox takes rotation in along Z and sends it out along Y, and
the speedometer and the clutch are fittings that present a face on every axis.
The millstone's sample stress is 12 and the crushing wheel's is 24. These are
test parameters, not a claim of full Create parity.

## Recovered behavior and provenance

`create-rs` from the handoff was not found in the checkout, uploads or public
repository search. Behavior was reconstructed against public Create
`mc1.21.1/dev`, commit `4148fd8d5b05c85d88947ed00fb495a5074f57`; this is a
reference, not the missing Rust source.

Gear contact rules currently modeled:

- small/small: Manhattan distance one, not along the axle; parallel axes,
  1:1 speed ratio and inversion;
- large/small: same axis and one-block offsets along both perpendicular
  coordinates; 2:1 from large to small, 1:2 in reverse, with inversion;
- large/large: distinct axes, unit offsets along both axle coordinates and
  zero along the remaining coordinate; 1:1, with direction set by the signs;
- diagonal candidates have fixed iteration order: four for small and twelve
  for large cogwheels.

Contacts are resolved per terminal, not per axle. A terminal is an axis a
neighbour may touch a component on, with the direction reversal at that face:

- a shaft, a cogwheel, a hand crank, a load and a water wheel present one
  terminal, on their own axis;
- a gearbox presents two, its output terminal inverted, so rotation going in
  along Z comes out along Y reversed and a component on the input axis is
  driven straight through;
- a speedometer and a clutch present one on every axis, being fittings;
- two components that are not cogwheels mesh when they touch along one axis
  that both of them present, at 1:1, with the reversal of their two faces
  exclusive-or'd.

Components beyond the four this started with:

- a water wheel is a source whose speed is `flow / 4` RPM and whose capacity is
  `flow` stress units, where flow is the four sides' water levels (plus four
  for a falling column) clamped to 16; at flow 0 it is a passive shaft;
- a speedometer adds neither stress nor capacity and reports the speed of the
  network it stands in;
- a clutch, while released, breaks every contact it has on both sides, so the
  networks around it run apart.

`KINETICS_RULES_VERSION` is 2 for the rules above (terminals, the four new
components and their contacts); `SIM_MATH_VERSION` stays 1, the fixed-point,
angle and rounding conventions being unchanged.

The kinetic prototype builds a typed generational arena, detects deterministic
contacts, propagates exact rational speed ratios, chooses the strongest source
with a positional tie-break, and stops an overloaded or inconsistent network
when stress is strictly greater than capacity. A stable FNV-1a hash is exposed
for regression checks. This is an initial, testable subset—not the complete
Create rotation propagator or kinetic gameplay.

Any simulation-math or gear-rule change must update golden tests, its version,
and this document. A future host ABI or simulation contract change must have
its own version and documentation update; native hook versions remain those
provided by the pinned Horizon catalog.

## Checking the bundle without Horizon

The bundle is compiled by the launcher inside `horizon-client`, so a mistake in
it would only surface there. `harness/` rebuilds the host types the bundle
names — `Game` and its native hooks, `HudBatch`, `Corners`, `NativeMod`, the
world and its state facts, and the `horizon_client::blocks::Fluid` the water
wheel reads — under the same names and signatures, and `include!`s the
bundle's sources, so they compile and their tests run with `cargo test`. The
stand-ins model the pinned API; they are not a port of Horizon, and every value
a test feeds them is the mod's own fixture. `tools/bootstrap-rust.py` fetches
an offline Rust toolchain for environments that have none.
