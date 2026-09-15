//! Library target so examples/benches can link the internals.
//! The binary (main.rs) uses these same modules via `use termpaper::…`.
pub mod brand;
pub mod canvas;
pub mod color_grade;
pub mod color_wheel;
pub mod config;
pub mod filter;
pub mod link;
pub mod menu;
pub mod physics;
pub mod render;
pub mod scene;
pub mod transition;
pub mod wall;

#[cfg(feature = "gpu")]
pub mod gpu;
