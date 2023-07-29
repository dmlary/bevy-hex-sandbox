#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

pub mod constants;
pub mod file_picker;
pub mod map;
pub mod persistence;
pub mod thumbnail_render;
pub mod tileset;
pub mod ui;
pub mod util;

pub mod prelude {
    pub use super::map::{Map, WorldMapExt};
}
