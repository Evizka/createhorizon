//! Host stand-ins for the Create Horizon native mod.
//!
//! The launcher compiles the bundle in [`mixins/createhorizon_native`] into
//! Horizon's own `game` module, where `Game`, `HudBatch`, `Corners`,
//! `NativeMod` and the world tables it reads already exist. This crate
//! rebuilds just enough of those, under the same names and with the same
//! signatures, so the bundle's sources compile and their tests run here with
//! nothing but `cargo test`.
//!
//! It is a checking aid, not a port of Horizon: the stand-ins model the
//! pinned `horizon-client` API (`compatibility.json`), and every value a
//! test feeds them is the mod's own fixture.
#![forbid(unsafe_code)]

pub mod game;
