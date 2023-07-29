use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    path::PathBuf,
};

use anyhow::{bail, Context, Result};
use bevy::{
    ecs::system::{Command, EntityCommands},
    prelude::*,
    tasks::{IoTaskPool, Task},
};
use futures_lite::future;
use hexx::HexLayout;
use ron::ser::{to_writer_pretty, PrettyConfig};
use serde::{de::Visitor, Deserialize, Serialize};

use crate::{map, tileset};

pub struct Plugin;
impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.register_type::<SaveId>()
            .add_system(map_writers)
            .add_system(map_importer);
    }
}

/// Entity-like ID used in save files
///
/// It is necessary to support references within the save file.  For example
/// each tile on the map references the Tileset containing the model for that
/// tile.  Within `World`, `Entity` values are used for these references, but
/// as `Entity` values can vary during runtime.  This is a problem because
/// small changes to a Map would result in many `Entity` values changing in the
/// save file.
///
/// This is important for this case as human-readability & diffing of Map saves
/// is a necessary feature.
///
/// Instead of saving Entity values, we map them to stable `SaveId`s that when
/// assigned are stored with the Entity in the World.  If a saved Entity needs
/// a SaveId, it will be assigned in the World and reused from that point
/// forward.
///
/// NOTE: Right now there's no support for resolving SaveId collisions
#[derive(Clone, Component, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Reflect)]
pub struct SaveId(usize);

/// serialize SaveId as a bare u64
impl Serialize for SaveId {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u64(self.0 as u64)
    }
}

struct SaveIdVisitor;

impl<'de> Visitor<'de> for SaveIdVisitor {
    type Value = SaveId;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("expecting unsigned integer")
    }

    fn visit_u64<E>(self, v: u64) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(SaveId(v as usize))
    }
}

/// deserialize SaveId from a bare u64
impl<'de> Deserialize<'de> for SaveId {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_u64(SaveIdVisitor)
    }
}

impl std::ops::Add<usize> for SaveId {
    type Output = Self;
    fn add(self, rhs: usize) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl std::ops::AddAssign<usize> for SaveId {
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs;
    }
}

/// trait to add SaveId helper methods to World
pub trait WorldSaveIdExt {
    /// get the next unused SaveId in the World
    fn save_id_next(&mut self) -> SaveId;

    /// assign SaveIds to the provided Entities, and return a map
    fn assign_save_ids(
        &mut self,
        entities: impl Iterator<Item = Entity>,
    ) -> Result<HashMap<Entity, SaveId>>;
}

impl WorldSaveIdExt for &mut World {
    fn save_id_next(&mut self) -> SaveId {
        let mut query = self.query::<&SaveId>();
        query
            .iter(self)
            .max()
            .map(|id| *id + 1)
            .unwrap_or(SaveId(0))
    }

    fn assign_save_ids(
        &mut self,
        entities: impl Iterator<Item = Entity>,
    ) -> Result<HashMap<Entity, SaveId>> {
        let mut next_id = self.save_id_next();
        let mut entity_map = HashMap::new();

        for entity in entities {
            let mut entity_ref = self
                .get_entity_mut(entity)
                .context(format!("unknown entity: {:?}", entity))?;
            let id = match entity_ref.get::<SaveId>() {
                Some(id) => *id,
                None => {
                    let id = next_id;
                    entity_ref.insert(id);
                    next_id += 1;
                    id
                }
            };

            // add the entity/id pair to our output map
            entity_map.insert(entity, id);
        }

        Ok(entity_map)
    }
}

/// save file representation of a map tile
#[derive(Default, Debug, Serialize, Deserialize)]
struct Tile {
    location: map::Location,
    tileset: SaveId,
    tile_id: tileset::TileId,
    rotation: tileset::TileRotation,
}

/// save file representation of a tilemap layer
#[derive(Default, Debug, Serialize, Deserialize)]
struct Layer {
    name: String,
    tiles: Vec<Tile>,
}

impl From<&map::Layer> for Layer {
    fn from(value: &map::Layer) -> Self {
        Self {
            name: value.name.clone(),
            tiles: Vec::new(),
        }
    }
}

impl From<&Layer> for map::Layer {
    fn from(value: &Layer) -> Self {
        Self {
            name: value.name.clone(),
            tiles: HashMap::new(),
        }
    }
}

const MAP_FORMAT_VERSION: usize = 1;
#[derive(Default, Debug, Serialize, Deserialize)]
struct MapFormat {
    version: usize,
    layout: HexLayout,
    tilesets: BTreeMap<SaveId, tileset::Tileset>, // btree map for enforced order
    layers: Vec<Layer>,

    // just used during construction, do not save
    #[serde(skip)]
    entity_map: HashMap<Entity, SaveId>,
}

impl MapFormat {
    /// build a MapFormat struct from the World and the root Map entity
    fn try_new(world: &mut World, root: Entity) -> Result<Self> {
        let mut map = Self {
            version: MAP_FORMAT_VERSION,
            ..default()
        };
        let root_entity = world.entity(root);
        map.layout = root_entity
            .get::<map::Map>()
            .context(format!(
                "failed to get Map component for map root {:?}",
                root
            ))?
            .layout
            .clone();

        map.add_tilesets(world, root)?.add_layers(world, root)?;
        Ok(map)
    }

    fn add_tilesets(&mut self, mut world: &mut World, root: Entity) -> Result<&mut Self> {
        let mut query = world.query_filtered::<(Entity, &Parent), With<tileset::Tileset>>();
        let tilesets: Vec<Entity> = query
            .iter(world)
            .filter_map(|(entity, parent)| {
                if parent.get() == root {
                    Some(entity)
                } else {
                    None
                }
            })
            .collect();
        self.entity_map = world.assign_save_ids(tilesets.iter().cloned())?;

        let mut query = world.query::<&tileset::Tileset>();
        for entity in tilesets {
            let id = self
                .entity_map
                .get(&entity)
                .context(format!("failed to get SaveId for Tileset {:?}", entity))?;
            let tileset = query.get(world, entity)?;
            self.tilesets.insert(*id, tileset.clone());
        }

        Ok(self)
    }

    fn add_layers(&mut self, world: &mut World, root: Entity) -> Result<&mut Self> {
        let mut query = world.query::<(&map::Layer, &Parent, &Children)>();
        let mut tiles =
            world.query::<(&map::Location, &tileset::TileRef, &tileset::TileTransform)>();
        for (layer, parent, children) in query.iter(world) {
            if parent.get() != root {
                continue;
            }
            let mut layer: Layer = layer.into();

            for child in children {
                let Ok((location, tile_ref, tile_transform)) = tiles.get(world, *child) else { continue; };
                let tileset = self
                    .entity_map
                    .get(&tile_ref.tileset)
                    .context(format!("tileset SaveId not found: {:?}", tile_ref))?;

                // construct our tile structure and add it to the layer
                let tile = Tile {
                    location: *location,
                    tileset: *tileset,
                    tile_id: tile_ref.tile,
                    rotation: tile_transform.rotation,
                };
                layer.tiles.push(tile);
            }
            self.layers.push(layer);
        }
        Ok(self)
    }

    pub fn try_spawn(&self, root: &mut EntityCommands) -> Result<()> {
        if self.version != MAP_FORMAT_VERSION {
            bail!(
                "unsupported map version: {} != {}",
                self.version,
                MAP_FORMAT_VERSION
            );
        }
        debug!("loading map into {:?}", root.id());

        let map = map::Map {
            layout: self.layout.clone(),
        };

        // restore tilesets & create a SaveId -> Entity map for the tilesets
        let mut entity_map = HashMap::new();
        for (id, tileset) in &self.tilesets {
            let entity = root
                .commands()
                .spawn((Name::new("tileset"), tileset.clone()))
                .id();
            root.add_child(entity);
            entity_map.insert(id, entity);
        }

        // restore layers
        for layer in &self.layers {
            let layer_component: map::Layer = layer.into();
            let layer_entity = root
                .commands()
                .spawn((
                    Name::new("layer"),
                    layer_component,
                    SpatialBundle::default(),
                ))
                .id();
            root.add_child(layer_entity);

            let mut tiles = Vec::new();

            for tile in &layer.tiles {
                let tile_ref = tileset::TileRef {
                    tileset: *entity_map.get(&tile.tileset).unwrap(),
                    tile: tile.tile_id,
                };
                let tile_entity = root
                    .commands()
                    .spawn((
                        tile.location,
                        tile_ref,
                        tileset::TileTransform {
                            rotation: tile.rotation,
                        },
                        SpatialBundle::default(),
                    ))
                    .id();
                tiles.push(tile_entity);
            }
            root.commands().entity(layer_entity).push_children(&tiles);
        }

        root.insert((SpatialBundle::default(), map));
        Ok(())
    }
}

/// Command used to save a `map::Map` to a given path
///
/// This needs to be a Command because we need `&mut World` to create queries
/// for accessing all the sub-entities that make up the map.
pub struct SaveMapCommand {
    /// destination path to write map to
    path: std::path::PathBuf,
    /// root entity of map; has `map::Map` component
    map: Entity,
}

impl SaveMapCommand {
    pub fn new(path: std::path::PathBuf, map: Entity) -> Self {
        Self { path, map }
    }
}

impl Command for SaveMapCommand {
    fn write(self, world: &mut World) {
        let map = match MapFormat::try_new(world, self.map) {
            Ok(map) => map,
            Err(err) => {
                warn!("failed to save map: {:#?}", err);
                return;
            }
        };

        let task_pool = IoTaskPool::get();
        let task = task_pool.spawn(async move {
            let f = File::create(self.path.clone()).context(format!("open map {:?}", self.path))?;
            to_writer_pretty(f, &map, PrettyConfig::default())
                .context(format!("writing map to {:?}", self.path))?;
            Ok::<(), anyhow::Error>(())
        });
        world.spawn(MapWriterTask(task));
    }
}

/// This component is used to track the IoTask that is writing the map to the
/// disk.
#[derive(Component)]
struct MapWriterTask(Task<Result<()>>);

fn map_writers(mut commands: Commands, mut map_writers: Query<(Entity, &mut MapWriterTask)>) {
    for (entity, mut writer) in &mut map_writers {
        let Some(result) = future::block_on(future::poll_once(&mut writer.0)) else { continue };
        if let Err(e) = result {
            warn!("{:#?}", e);
        }
        commands.entity(entity).despawn();
    }
}

#[derive(Component)]
pub struct MapImporter {
    path: PathBuf,
    task: Task<Result<MapFormat>>,
}

impl MapImporter {
    pub fn new(path: PathBuf) -> Self {
        let path_copy = path.clone();
        let task_pool = IoTaskPool::get();
        let task = task_pool.spawn(async move {
            let buf = std::fs::read_to_string(path).context("failed to read file")?;
            let map = ron::from_str(&buf).context("failed to parse map")?;
            Ok(map)
        });

        Self {
            path: path_copy,
            task,
        }
    }
}

fn map_importer(mut commands: Commands, mut map_importers: Query<(Entity, &mut MapImporter)>) {
    for (entity, mut importer) in &mut map_importers {
        let Some(result) = future::block_on(future::poll_once(&mut importer.task)) else { continue };
        match result {
            Err(e) => {
                warn!(
                    "failed to load map {}: {:?}",
                    importer.path.to_string_lossy(),
                    e
                );
                commands.entity(entity).despawn();
            }
            Ok(map) => {
                let name = importer.path.file_stem().unwrap().to_string_lossy();
                let mut entity_ref = commands.entity(entity);

                // try to spawn the map using the existing entity as the root node
                if let Err(e) = map.try_spawn(&mut entity_ref) {
                    // failed to spawn the map; log it and despawn the entity
                    error!(
                        "failed to spawn map {}: {:?}",
                        importer.path.to_string_lossy(),
                        e
                    );
                    entity_ref.despawn_recursive();
                    continue;
                }

                // map loaded successfully, remove the importer and insert a name
                entity_ref
                    .remove::<MapImporter>()
                    .insert(Name::new(format!("map: {}", name)));
            }
        };
    }
}

#[cfg(test)]
mod tests {

    use map::*;
    use test_log::test;

    fn spawn_map(world: &mut World) -> Entity {
        world
            .spawn((Name::new("map root"), crate::map::Map::new()))
            .with_children(|map| {
                // add tilesets
                let tileset_a = map.spawn(tileset::Tileset::new("tileset a")).id();
                let tileset_b = map.spawn(tileset::Tileset::new("tileset b")).id();
                // add some layers
                map.spawn(crate::map::Layer::new("layer 0".into()))
                    .with_children(|layer| {
                        spawn_tile(layer, tileset_a, 0, 0, 0, tileset::TileRotation::None);
                        spawn_tile(layer, tileset_b, 1, 2, 3, tileset::TileRotation::None);
                    });
            })
            .id()
    }

    fn spawn_tile(
        layer: &mut WorldChildBuilder,
        tileset: Entity,
        tile: usize,
        x: i32,
        y: i32,
        rotation: tileset::TileRotation,
    ) {
        layer.spawn((
            tileset::TileRef { tileset, tile },
            crate::map::Location { x, y },
            tileset::TileTransform { rotation },
        ));
    }

    #[test]
    fn world_assign_save_ids() {
        let mut world = World::new();
        let entity = world.spawn(Name::new("entity")).id();
        let mut w = &mut world;
        let entity_map = w
            .assign_save_ids(vec![entity].iter().cloned())
            .expect("assign_save_ids() to return Ok");
        assert_eq!(entity_map.get(&entity), Some(&SaveId(0)));
        assert_eq!(world.entity(entity).get::<SaveId>(), Some(&SaveId(0)));
    }

    #[test]
    fn world_assign_save_ids_already_assigned() {
        let mut world = World::new();
        let entity = world.spawn((Name::new("entity"), SaveId(13))).id();
        let mut w = &mut world;
        let entity_map = w
            .assign_save_ids(vec![entity].iter().cloned())
            .expect("assign_save_ids() to return Ok");
        assert_eq!(entity_map.get(&entity), Some(&SaveId(13)));
        assert_eq!(world.entity(entity).get::<SaveId>(), Some(&SaveId(13)));
    }

    #[test]
    fn map_format_try_new() {
        let mut world = World::new();
        let root = spawn_map(&mut world);
        let map_format =
            MapFormat::try_new(&mut world, root).expect("try_new() to create a MapFormat");
        debug!("{:#?}", map_format);
        assert_eq!(map_format.tilesets.len(), 2);
        assert_eq!(map_format.layers.len(), 1);
    }

    #[test]
    fn map_format_try_new_empty_map() {
        let mut world = World::new();
        let root = world.spawn(crate::map::Map::default()).id();
        let map_format =
            MapFormat::try_new(&mut world, root).expect("try_new() to create a MapFormat");
        debug!("{:#?}", map_format);
        assert_eq!(map_format.tilesets.len(), 0);
        assert_eq!(map_format.layers.len(), 0);
    }

    #[test]
    fn save_id_serde() {
        let id = SaveId(231);
        let str = ron::to_string(&id).expect("serialize successfully");
        assert_eq!(str, "231");
        let value = ron::from_str::<SaveId>(&str).expect("deserialize successfully");
        assert_eq!(value, id);
    }
}
