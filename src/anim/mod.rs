//! Animation toolkit: the shared vocabulary behind the animator-grade scenes.
//!
//! - [`draw`]: alpha-blended, anti-aliased primitives (strokes, capsules,
//!   polygons, ellipses, béziers, gradients, lights, shadows) that draw onto
//!   the live [`crate::canvas::Canvas`] or onto an offscreen [`draw::Plate`]
//!   so static layers are painted once and composited every frame.
//! - [`light`]: palettes, aerial perspective, rim light, tone bands and point
//!   lights, so one light source can touch every layer of a scene.
//! - [`rig`]: forward-kinematic bone chains, two-bone IK, eased keyframe
//!   tracks, spring "follow" for overlapping action, squash and stretch.
//! - [`stage`]: orientation-aware composition (pixel aspect, portrait
//!   detection, hero sizing), a drifting camera and parallax layers.
//! - [`beat`]: a deterministic looping timeline with named beats.
//! - [`scenery`]: reusable scenery — star fields, moons, ridge profiles,
//!   pines — extracted from the catalog so every scene draws them the same way.
//!
//! Everything is deterministic: no wall-clock, no global RNG. Scenes feed
//! their own seeded `StdRng` and elapsed time in.
pub mod beat;
pub mod draw;
pub mod light;
pub mod rig;
pub mod scenery;
pub mod stage;

/// 8-bit RGB triple, the color type used across the crate.
pub type Rgb = (u8, u8, u8);

pub use beat::Timeline;
pub use draw::{Plate, Surface};
pub use light::{Palette, PointLight};
pub use rig::{Ease, Follow, Skeleton, Track};
pub use stage::{Camera, ParallaxLayer, Stage};
