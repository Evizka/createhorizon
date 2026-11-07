//! The one `horizon_client` item the native bundle imports.
//!
//! `mixins/createhorizon_native/runtime.rs` names
//! `horizon_client::blocks::Fluid` to read the water beside a water wheel.
//! In a Horizon build that is the game's own enum; here it is this stand-in,
//! which the harness fills with the same values.

#![forbid(unsafe_code)]

pub mod blocks {
    /// `horizon_items::blocks::Fluid`, re-exported by `horizon-client`.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Fluid {
        None,
        Water,
        Lava,
    }
}
