use anyhow::{Context, Result};
use bevy::math::Vec3Swizzles;
use bevy::prelude::*;
use hexx::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::tileset;

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.register_type::<HashMap<usize, tileset::Tile>>()
            .register_type::<HashMap<Location, (Entity, tileset::TileRef)>>()
            .register_type::<(Entity, tileset::TileRef)>()
            .register_type::<Location>()
            .register_type::<Layer>()
            .add_systems((update_location,).in_base_set(CoreSet::First));
    }
}

#[derive(
    Component,
    Default,
    Debug,
    PartialEq,
    Reflect,
    Eq,
    Hash,
    Copy,
    Clone,
    FromReflect,
    Serialize,
    Deserialize,
)]
#[reflect_value(Component, Hash, Serialize, Deserialize)]
pub struct Location {
    pub x: i32,
    pub y: i32,
}

impl Location {
    pub fn hex(&self) -> Hex {
        Hex::from(*self)
    }
}

impl From<Hex> for Location {
    fn from(h: Hex) -> Location {
        Location { x: h.x, y: h.y }
    }
}

impl From<(i32, i32)> for Location {
    fn from(p: (i32, i32)) -> Location {
        Location { x: p.0, y: p.1 }
    }
}

impl From<Location> for Hex {
    fn from(l: Location) -> Hex {
        Hex { x: l.x, y: l.y }
    }
}

#[derive(Component, Default)]
pub struct Map {
    pub layout: HexLayout,
}

pub trait WorldMapExt: Sized {
    fn get_map(&mut self) -> Result<&Map>;
}

impl WorldMapExt for &mut World {
    fn get_map(&mut self) -> Result<&Map> {
        let mut query = self.query::<&Map>();
        query
            .get_single(self)
            .context("failed to get single Map entity")
    }
}

#[derive(Component, Default, Reflect, Debug)]
#[reflect(Component)]
pub struct Layer {
    pub name: String,
    pub tiles: HashMap<Location, (Entity, tileset::TileRef)>,
}

impl Layer {
    pub fn new(name: String) -> Self {
        Self {
            name,
            tiles: HashMap::new(),
        }
    }
}

/// Add this component to anything with a Location that should be updated based
/// on its GlobalTransform
#[derive(Component)]
pub struct UpdateLocation;

impl Map {
    pub fn new() -> Self {
        Map::default()
    }

    pub fn snap_to_grid(&self, pos: Vec3) -> (Vec3, Location) {
        let hex = self.layout.world_pos_to_hex(pos.xz());
        let snapped = self.layout.hex_to_world_pos(hex);
        (Vec3::new(snapped.x, pos.y, snapped.y), hex.into())
    }

    pub fn translation(&self, location: Location) -> Vec3 {
        let pos = self.layout.hex_to_world_pos(location.into());
        Vec3::new(pos.x, 0.0, pos.y)
    }

    pub fn tile_translation(&self, tile: &tileset::Tile, location: Location) -> Vec3 {
        let pos = self.layout.hex_to_world_pos(location.into());
        Vec3::new(pos.x, tile.transform.translation.y, pos.y)
    }

    pub fn tile_transform(
        &self,
        tile: &tileset::Tile,
        location: Location,
        tile_transform: &tileset::TileTransform,
    ) -> Transform {
        let pos = self.layout.hex_to_world_pos(location.into());
        Transform {
            translation: Vec3::new(pos.x, tile.transform.translation.y, pos.y),
            rotation: tile.transform.rotation
                * Quat::from_euler(EulerRot::XYZ, 0.0, tile_transform.rotation.into(), 0.0),
            scale: tile.transform.scale,
        }
    }

    pub fn hex_to_world_pos(&self, hex: Hex, y: f32) -> Vec3 {
        let hex = self.layout.hex_to_world_pos(hex);
        Vec3::new(hex.x, y, hex.y)
    }
}

fn update_location(
    mut query: Query<
        (&mut Location, &GlobalTransform),
        (With<UpdateLocation>, Changed<GlobalTransform>),
    >,
    map: Query<&Map>,
) {
    let Ok(map) = map.get_single() else { return; };
    for (mut loc, transform) in &mut query {
        let hex = map.layout.world_pos_to_hex(transform.translation().xz());
        loc.set_if_neq(hex.into());
    }
}
