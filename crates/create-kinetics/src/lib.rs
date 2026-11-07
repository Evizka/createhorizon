#![no_std]
#![forbid(unsafe_code)]

//! Public kinetics facade over the core shared with the native Horizon mod.

pub use create_engine::{
    AddPartError, Axis, BlockPos, GearCoupling, GearSize, Gearwheel, KINETICS_RULES_VERSION,
    KineticLink, KineticPart, KineticSource, KineticState, Kinetics, NetworkSummary, Rotational,
    StressCapacity, StressImpact, additional_gear_offsets, coupling_between,
};
