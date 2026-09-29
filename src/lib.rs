//! Library target so examples/benches can link the internals.
//! The binary (main.rs) uses these same modules via `use termpaper::…`.
pub mod anim;
pub mod brand;
pub mod calibrate;
pub mod canvas;
pub mod check;
pub mod color_grade;
pub mod config;
pub mod desk;
pub mod engine;
pub mod filter;
pub mod governor;
pub mod hypr;
pub mod launch;
pub mod link;
pub mod look;
pub mod menu;
pub mod overlay;
pub mod pace;
pub mod physics;
pub mod platform;
pub mod prefs;
pub mod render;
pub mod scene;
pub mod studio;
pub mod sync;
pub mod term_caps;
pub mod theme;
pub mod theme_cli;
pub mod transition;
pub mod wallplan;
pub mod wall;

#[cfg(feature = "gpu")]
pub mod gpu;
