#![no_std]
#![forbid(unsafe_code)]

//! Public math facade over the single-source engine crate shared with the native mod.

pub use create_engine::{Angle, Fx, Rat, SIM_MATH_VERSION, Vec3};
