use bevy::ecs::system::SystemState;
use bevy::prelude::*;
use bevy_egui::egui;
use hex_sandbox::{tileset, ui::widget::*};

use crate::{EditorUiEvent, TileSelection};

#[derive(Reflect, Clone, Copy, Debug)]
enum TileTransform {
    Basic {
        y_translation: f32, // translate along Y
        y_rotation: f32,    // euclid rotation around y-axis
        scale: f32,         // fixed ratio x, y, z
    },
    Full(Transform),
}

impl TileTransform {
    fn into_full(self) -> TileTransform {
        match self {
            TileTransform::Full(_) => self,
            TileTransform::Basic { .. } => TileTransform::Full(self.into()),
        }
    }

    fn into_basic(self) -> TileTransform {
        match self {
            TileTransform::Basic { .. } => self,
            TileTransform::Full(t) => TileTransform::Basic {
                y_translation: t.translation.y,
                y_rotation: t.rotation.to_euler(EulerRot::XYZ).1,
                scale: t.scale.x,
            },
        }
    }
}

impl Default for TileTransform {
    fn default() -> Self {
        Self::Basic {
            y_translation: 0.0,
            y_rotation: 0.0,
            scale: 1.0,
        }
    }
}

impl From<Transform> for TileTransform {
    fn from(transform: Transform) -> TileTransform {
        let translation = transform.translation;
        let rotation = transform.rotation;
        let scale = transform.scale;
        if translation.x == 0.0
            && translation.z == 0.0
            && rotation.x == 0.0
            && rotation.z == 0.0
            && scale.x == scale.y
            && scale.x == scale.z
        {
            TileTransform::Full(transform).into_basic()
        } else {
            TileTransform::Full(transform)
        }
    }
}
impl From<TileTransform> for Transform {
    fn from(tt: TileTransform) -> Transform {
        match tt {
            TileTransform::Basic {
                y_translation,
                y_rotation,
                scale,
            } => Transform {
                translation: Vec3::new(0.0, y_translation, 0.0),
                rotation: Quat::from_euler(EulerRot::XYZ, 0.0, y_rotation, 0.0),
                scale: Vec3::splat(scale),
            },
            TileTransform::Full(t) => t,
        }
    }
}

pub struct TileProperties<'w: 'static, 's: 'static> {
    system_state: SystemState<(
        Res<'w, TileSelection>,
        Res<'w, AppTypeRegistry>,
        Query<'w, 's, &'static mut tileset::Tileset>,
        EventWriter<'w, EditorUiEvent>,
    )>,
    transform: TileTransform,
}

impl<'w, 's> BasicWidget for TileProperties<'w, 's> {
    fn new(world: &mut World, _ui: &egui::Ui) -> Self {
        Self {
            system_state: SystemState::new(world),
            transform: TileTransform::default(),
        }
    }

    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, id: egui::Id) {
        let (selection, type_registry, mut tilesets, mut events) = self.system_state.get_mut(world);
        let Some(tile_ref) = selection.active_tile() else {
            ui.label("No tiles selected");
            return;
        };

        let Ok(tileset) = tilesets.get(tile_ref.tileset) else {
            ui.label(format!("Error: unknown tileset {:?}", tile_ref.tileset));
            return;
        };

        if selection.is_changed() {
            let Some(tile) = tileset.tiles.get(&tile_ref.tile) else {
                ui.label(format!("Error: unknown tile {} in tileset {} ({:?})",
                tile_ref.tile, tileset.name, tile_ref.tileset));
                return;
            };
            self.transform = tile.transform.into();
        }

        let mut full = false;
        let changed = match &mut self.transform {
            TileTransform::Full(t) => {
                full = true;
                bevy_inspector_egui::reflect_inspector::ui_for_value(t, ui, &type_registry.read())
            }
            TileTransform::Basic {
                y_translation,
                y_rotation,
                scale,
            } => {
                egui::Grid::new(id.with("basic"))
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label("y-translation");
                        let mut changed = ui
                            .add(egui::DragValue::new(y_translation).speed(0.1))
                            .changed();
                        ui.end_row();

                        ui.label("y-rotation");
                        changed |= ui.drag_angle_tau(y_rotation).changed();
                        ui.end_row();

                        ui.label("scale");
                        changed |= ui
                            .add(egui::DragValue::new(scale).speed(0.05).fixed_decimals(2))
                            .changed();
                        ui.end_row();

                        changed
                    })
                    .inner
            }
        };

        if ui.checkbox(&mut full, "advanced").changed() {
            if full {
                self.transform = self.transform.into_full();
            } else {
                self.transform = self.transform.into_basic();
            }
        }

        if !changed {
            return;
        }

        for tile_ref in &selection.tiles {
            let Ok(mut tileset) = tilesets.get_mut(tile_ref.tileset) else {
                warn!("Error: unknown tileset {:?}", tile_ref.tileset);
                continue;
            };

            let Some(mut tile) = tileset.tiles.get_mut(&tile_ref.tile) else {
                warn!("Error: unknown tile {} in tileset {} ({:?})",
                    tile_ref.tile, tileset.name, tile_ref.tileset);
                continue;
            };

            tile.transform = self.transform.into();
        }
        events.send(EditorUiEvent::RedrawMapTiles);
        self.system_state.apply(world);
    }
}
