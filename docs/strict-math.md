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
API 11. The full pins and source SHA-256 checks are in `compatibility.json`.

The native hooks add the mod to Options > Mods, draw a small HUD, and intercept
`/createhorizon scan`. The runtime reads the public world block states in a
small radius, converts the host-provided player position to integer block
coordinates at the boundary, and simulates the discovered sample components
with `create-engine`. GUI layout uses Horizon's `f32` HUD helpers, but those
values never enter simulation. The sample intentionally does not mutate blocks,
items, or world state.

The content pack defines six simple cube blocks in the `createhorizon:`
namespace. Their kinetic axes are fixed to Z for this prototype. A hand crank
is a constant 16-RPM source with capacity 16; the millstone's sample stress is
12 and the crushing wheel's is 24. These are test parameters, not a claim of
full Create parity. There is no moving-block animation or complete Create
content/recipe port yet.

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
