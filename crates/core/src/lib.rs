//! gw_core: every rule of Graftwood.
//!
//! Builder requirement: this crate performs no input or output. It takes
//! values and returns values, so `cargo test` reaches every rule without a
//! browser. `tests/boundary.rs` enforces that, along with the dependency
//! allowlist and the reference ban on floats, hash maps and clocks.
#![allow(clippy::needless_range_loop, clippy::too_many_arguments)]

pub mod combat;
pub mod content;
pub mod creature;
pub mod fx;
pub mod game;
pub mod geom;
pub mod map;
pub mod mesh;
pub mod render;
pub mod rng;
pub mod run;
pub mod scene;
