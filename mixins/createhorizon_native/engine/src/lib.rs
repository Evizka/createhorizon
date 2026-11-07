#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;
#[cfg(test)]
#[macro_use]
extern crate std;

mod angle;
mod arena;
mod fx;
mod gear;
mod geometry;
mod kinetics;
mod rat;
mod vec3;

pub use angle::Angle;
pub use arena::{Arena, Handle};
pub use fx::Fx;
pub use gear::{GearCoupling, GearSize, Gearwheel, additional_gear_offsets, coupling_between};
pub use geometry::{Axis, BlockPos};
pub use kinetics::{
    AddPartError, KineticLink, KineticPart, KineticSource, KineticState, Kinetics, NetworkSummary,
    Rotational, StressCapacity, StressImpact, Terminal, Terminals, WATER_WHEEL_FULL_FLOW,
    WaterSide, water_wheel_capacity, water_wheel_flow, water_wheel_speed,
};
pub use rat::Rat;
pub use vec3::Vec3;

/// Version of fixed-point rounding, angle representation, and CORDIC behavior.
/// Increment when simulation math or serialized raw conventions change.
pub const SIM_MATH_VERSION: u32 = 1;

/// Version of the recovered gear-contact and kinetic propagation rules.
/// Increment when coupling or network-solving behavior changes.
///
/// 2: contacts are resolved per terminal (a gearbox presents two, on
/// different axes, the output one inverted), and a water wheel, a gearbox, a
/// speedometer and a clutch join the shaft, cogwheel, hand crank and load.
pub const KINETICS_RULES_VERSION: u32 = 2;
