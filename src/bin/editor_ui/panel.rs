use bevy::ecs::system::SystemState;
use bevy::prelude::*;
use bevy_egui::egui;
use hex_sandbox::{file_picker, map, tileset, ui, ui::widget::*};

use crate::{EditorState, EditorUiEvent};

#[derive(Default)]
pub struct EditorPanel;

impl BasicWidget for EditorPanel {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self::default()
    }

    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, id: egui::Id) {
        basic_widget::<TilesetPanel>(world, ui, id.with("tileset_panel"));
        basic_widget::<LayersPanel>(world, ui, id.with("layers_panel"));
        ui.allocate_space(ui.available_size());
    }
}

#[derive(Default, Clone)]
pub struct TilesetPanel;

impl BasicWidget for TilesetPanel {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self::default()
    }

    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, id: egui::Id) {
        fn_widget::<ui::widgets::PanelTitle>(world, ui, id.with("panel_tile"), "Tileset");
        basic_widget::<TilesetPanelHeader>(world, ui, id.with("tileset_panel_menu"));
        basic_widget::<TilesetViewer>(world, ui, id.with("tileset_viewer"));
    }
}

#[derive(Default, Clone)]
pub struct TilesetPanelHeader;

impl BasicWidget for TilesetPanelHeader {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self::default()
    }

    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, id: egui::Id) {
        ui.horizontal(|ui| {
            basic_widget::<TilesetDropdown>(world, ui, id.with("tileset_dropdown"));
            basic_widget::<TilesetMenu>(world, ui, id.with("tileset_menu"));
        });
    }
}

#[derive(Default, Clone)]
pub struct TilesetDropdown;

impl BasicWidget for TilesetDropdown {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self::default()
    }

    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, id: egui::Id) {
        let mut tilesets = world.query::<(Entity, &tileset::Tileset)>();
        let mut combo = egui::ComboBox::from_id_source(id.with("tileset_dropdown"));
        let state = world.resource::<EditorState>();
        if let Some(tileset_id) = state.active_tileset {
            if let Ok((_, tileset)) = tilesets.get(world, tileset_id) {
                combo = combo.selected_text(&tileset.name);
            }
        }
        let mut selection = Entity::PLACEHOLDER;

        combo.show_ui(ui, |ui| {
            for (entity, tileset) in tilesets.iter(world) {
                ui.selectable_value(&mut selection, entity, &tileset.name);
            }
        });

        if selection != Entity::PLACEHOLDER {
            let mut state = world.resource_mut::<EditorState>();
            state.active_tileset = Some(selection);
        }
    }
}

pub struct TilesetMenu;

// Inside the ListView widget:
impl BasicWidget for TilesetMenu {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self
    }

    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, _id: egui::Id) {
        egui::menu::menu_button(ui, "...", |ui| {
            if ui.button("Import Tileset").clicked() {
                world.spawn(
                    file_picker::Picker::for_many(crate::PickerEvent::TilesetImport(None))
                        .add_filter("Tileset Files", &["ron"])
                        .build(),
                );
                ui.close_menu();
            }

            let state = world.resource::<EditorState>();
            if let Some(tileset_id) = state.active_tileset {
                if ui.button("Export Tileset").clicked() {
                    world.spawn(
                        file_picker::Picker::save_dialog(crate::PickerEvent::TilesetExport(
                            tileset_id, None,
                        ))
                        .build(),
                    );
                    ui.close_menu();
                }
            }

            ui.separator();

            if ui.button("New Tileset").clicked() {
                let mut state = world.resource_mut::<EditorState>();
                state.new_tileset_window = true;
                ui.close_menu();
            }

            let state = world.resource::<EditorState>();

            if let Some(tileset_id) = state.active_tileset {
                if ui.button("Remove Tileset").clicked() {
                    world.spawn(ui::ConfirmationDialog {
                        title: "Delete Tileset",
                        message: "Are you sure you want to delete this tileset",
                        buttons: [
                            Some((
                                "Delete Tileset",
                                Some(EditorUiEvent::DeleteTileset(tileset_id)),
                            )),
                            Some(("Cancel", None)),
                            None,
                        ],
                    });
                }
            } else if ui
                .add_enabled(false, egui::Button::new("Remove Tileset"))
                .clicked()
            {
                unreachable!();
            }
        });
    }
}

#[derive(Default, Clone)]
pub struct RemoveTilesetButton;

impl BasicWidget for RemoveTilesetButton {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self::default()
    }

    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, _id: egui::Id) {
        let state = world.resource::<EditorState>();

        let Some(tileset_id) = state.active_tileset else {
            if ui.add_enabled(false, egui::Button::new("➖")).clicked() {
                unreachable!();
            }
            return;
        };

        if ui.button("➖").clicked() {
            // XXX expand dialog with details of the tileset

            world.spawn(ui::ConfirmationDialog {
                title: "Delete Tileset",
                message: "Are you sure you want to delete this tileset",
                buttons: [
                    Some((
                        "Delete Tileset",
                        Some(EditorUiEvent::DeleteTileset(tileset_id)),
                    )),
                    Some(("Cancel", None)),
                    None,
                ],
            });
        }
    }
}
#[derive(Default, Clone)]
pub struct TilesetViewer {
    height: f32,
}

impl BasicWidget for TilesetViewer {
    fn new(_world: &mut World, ui: &egui::Ui) -> Self {
        Self {
            height: ui.available_height() * 0.60,
        }
    }

    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, id: egui::Id) {
        egui::ScrollArea::vertical()
            .max_height(self.height)
            .auto_shrink([false, false])
            .id_source(id.with("vscroll"))
            .show(ui, |ui| {
                basic_widget::<TilePicker>(world, ui, id.with("tile_picker"));

                // fill in the scroll area so we don't shrink
                ui.allocate_space(ui.available_size());
            });
        ui.separator();
        basic_widget::<TilesetPanelFooter>(world, ui, id.with("tileset_footer"));
        self.height =
            fn_widget::<ui::widgets::VDragHandle>(world, ui, id.with("drag_handle"), self.height);
    }
}

#[derive(Default, Clone)]
pub struct TilesetPanelFooter;

impl BasicWidget for TilesetPanelFooter {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self::default()
    }

    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, id: egui::Id) {
        ui.horizontal(|ui| {
            basic_widget::<TilesetAddTiles>(world, ui, id.with("add_tiles"));
        });
    }
}

#[derive(Default, Clone)]
pub struct TilesetAddTiles;

impl BasicWidget for TilesetAddTiles {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self::default()
    }

    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, _id: egui::Id) {
        let state = world.resource::<EditorState>();

        let Some(tileset_id) = state.active_tileset else {
            if ui.add_enabled(false, egui::Button::new("➖")).clicked() {
                unreachable!();
            }
            return;
        };

        if ui.button("➕").clicked() {
            world.spawn(
                file_picker::Picker::for_many(crate::PickerEvent::AddTiles {
                    tileset_id,
                    files: None,
                })
                .add_filter("GLTF", &["glb"])
                .build(),
            );
        }
    }
}

pub struct TilePicker<'w: 'static, 's: 'static> {
    system_state: SystemState<(
        Res<'w, EditorState>,
        ResMut<'w, crate::TileSelection>,
        Query<'w, 's, &'static mut tileset::Tileset>,
    )>,
    tileset: Option<Entity>,
    start_range: Option<usize>,
    last_range: Option<Vec<tileset::TileRef>>,
    drag_start: Option<egui::Pos2>,
}

impl<'w, 's> BasicWidget for TilePicker<'w, 's> {
    fn new(world: &mut World, _ui: &egui::Ui) -> Self {
        Self {
            system_state: SystemState::new(world),
            tileset: None,
            start_range: None,
            last_range: None,
            drag_start: None,
        }
    }

    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, id: egui::Id) {
        use tileset::TileRef;

        let (state, mut selection, mut tilesets) = self.system_state.get_mut(world);

        if self.tileset != state.active_tileset {
            self.tileset = state.active_tileset;
            self.start_range = None;
            self.last_range = None;
            self.drag_start = None;
        }
        let Some(tileset_id) = state.active_tileset else {
            ui.label("no active tileset");
            return;
        };

        let modifiers = ui.input(|i| i.modifiers);
        let mut deselect_range = None;
        let mut select_range = None;
        let mut drop_index = None;
        let Ok(tileset) = tilesets.get(tileset_id) else {
            ui.label(format!("invalid tileset {:?}", tileset_id));
            return;
        };

        let tile_size = egui::Vec2::splat(48.0);
        let layout = egui::Layout::left_to_right(egui::Align::Min).with_main_wrap(true);
        let drag_layer = egui::LayerId::new(egui::Order::Tooltip, id.with("dragging"));
        ui.with_layout(layout, |ui| {
            let mut spacing = ui.spacing_mut();
            spacing.item_spacing = egui::vec2(0.0, 0.0);
            spacing.button_padding = egui::vec2(0.0, 0.0);
            let mut visuals = ui.visuals_mut();
            visuals.widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;

            for (index, tile_id) in tileset.tile_order.iter().enumerate() {
                let Some(tile) = tileset.tiles.get(tile_id) else {
                    warn!("unknown tile id in tileset order; \
                        tileset \"{}\" ({:?}), tile id {}",
                        tileset.name, tileset_id, tile_id);
                    continue;
                };
                let Some(texture) = tile.egui_texture_id else { continue };
                let tile_ref = TileRef {
                    tileset: tileset_id,
                    tile: *tile_id,
                };
                let selected = selection.tiles.contains(&tile_ref);

                let button = egui::ImageButton::new(texture, tile_size)
                    .selected(selected)
                    .sense(egui::Sense::click_and_drag());

                // if we're dragging, add any selected buttons to the drag layer
                if self.drag_start.is_some() {
                    let res = if selected {
                        // XXX move away from with_layer_id() because it causes
                        // resizing of the panel if you drag the left-most tile.
                        // We'll need to manually position things in a layer and
                        // translate it ourselves.
                        ui.with_layer_id(drag_layer, |ui| ui.add(button)).response
                    } else {
                        ui.add(button)
                    };

                    if res.hovered() && ui.input(|i| i.pointer.any_released()) {
                        drop_index = Some(index);
                        self.drag_start = None;
                    }
                    continue;
                }

                // not dragging, just draw the button
                let res = ui.add(button);
                if res.clicked() {
                    if modifiers.shift_only() {
                        deselect_range = self.last_range.take();
                        if let Some(start) = &self.start_range {
                            let range = if *start < index {
                                *start..=index
                            } else {
                                index..=*start
                            };
                            select_range = Some(range);
                        } else {
                            selection.tiles.insert(tile_ref);
                            self.start_range = Some(index);
                        }
                    } else if modifiers.command_only() {
                        if selected {
                            selection.tiles.remove(&tile_ref);
                            self.start_range = None;
                        } else {
                            selection.tiles.insert(tile_ref);
                            self.start_range = Some(index);
                        }
                        self.last_range = None;
                    } else {
                        selection.tiles.clear();
                        selection.tiles.insert(tile_ref);
                        self.start_range = Some(index);
                        self.last_range = None;
                    }
                } else if res.drag_delta().length() > 4.0 {
                    if !selected {
                        selection.tiles.clear();
                        selection.tiles.insert(tile_ref);
                        self.start_range = None;
                        self.last_range = None;
                    }
                    self.drag_start = Some(res.rect.center());
                }
            }
        });

        // XXX need hover target to drop at the bottom
        // XXX drag is sometimes resizing the panel; fix it

        // handle range-based changes to the selection; we handle deselect
        // before select because the deselect range will always overlap with
        // the select range if both are present.
        if let Some(range) = deselect_range {
            for tile_ref in range {
                selection.tiles.remove(&tile_ref);
            }
        }
        if let Some(range) = select_range {
            let mut added = Vec::new();
            for index in range {
                let tile_id = tileset.tile_order.get(index).unwrap();
                let tile_ref = TileRef {
                    tileset: tileset_id,
                    tile: *tile_id,
                };
                added.push(tile_ref.clone());
                selection.tiles.insert(tile_ref);
            }
            self.last_range = Some(added);
        }

        // if we're dragging, show the drag cursor, and translate the drag layer
        if let Some(drag_start) = self.drag_start {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            if let Some(pos) = ui.ctx().pointer_interact_pos() {
                let delta = pos - drag_start;
                ui.ctx().translate_layer(drag_layer, delta);
            }
        }

        // if there was a drop, shuffle the tile order to move all selected
        // tiles (in order) to the drop index.
        if let Some(mut insert_index) = drop_index {
            let mut tileset = tilesets.get_mut(tileset_id).unwrap();
            let mut moved = Vec::new();

            for (index, tile_id) in tileset.tile_order.iter().enumerate() {
                let tile_ref = TileRef {
                    tileset: tileset_id,
                    tile: *tile_id,
                };
                if selection.tiles.contains(&tile_ref) {
                    moved.push((*tile_id, index));
                    if index < insert_index {
                        insert_index -= 1;
                    }
                }
            }
            moved.reverse();

            for (_, index) in moved.iter() {
                tileset.tile_order.remove(*index);
            }
            for (tile_id, _) in moved.iter() {
                tileset.tile_order.insert(insert_index, *tile_id);
            }
        }
    }
}

#[derive(Default)]
pub struct LayersPanel;

impl BasicWidget for LayersPanel {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self::default()
    }

    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, id: egui::Id) {
        fn_widget::<ui::widgets::PanelTitle>(world, ui, id.with("title"), "Layers");
        egui::ScrollArea::vertical()
            .max_height(ui.available_height() - 25.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                basic_widget::<LayersList>(world, ui, id.with("layer_list"));
                ui.allocate_space(ui.available_size());
            });
        basic_widget::<LayersButtons>(world, ui, id.with("layers_buttons"));
    }
}

#[derive(Default)]
pub struct LayersList;

impl BasicWidget for LayersList {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self::default()
    }

    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, _id: egui::Id) {
        let state = world.resource::<EditorState>();
        let mut active_layer = state.active_layer.unwrap_or(Entity::PLACEHOLDER);
        let mut layers = world.query::<(Entity, &map::Layer)>();
        let mut changed = false;
        let layout = egui::Layout::top_down(egui::Align::LEFT).with_cross_justify(true);
        ui.with_layout(layout, |ui| {
            for (layer_id, layer) in layers.iter(world) {
                changed |= ui
                    .selectable_value(&mut active_layer, layer_id, &layer.name)
                    .changed();
            }
        });

        if changed {
            let mut state = world.resource_mut::<EditorState>();
            state.active_layer = Some(active_layer);
        }
    }
}

#[derive(Default)]
pub struct LayersButtons {
    show_popup: bool,
}

impl BasicWidget for LayersButtons {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self::default()
    }

    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, id: egui::Id) {
        let res = ui.button("➕");
        if res.clicked() {
            self.show_popup = true;
        }
        popup_widget::<CreateLayerPopup>(&mut self.show_popup, &res, world, ui, id.with("popup"));
    }
}

#[derive(Default, Clone)]
pub struct CreateLayerPopup {
    name: String,
}

impl PopupWidget for CreateLayerPopup {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self {
            name: "New Layer".to_string(),
        }
    }

    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, _id: egui::Id) -> bool {
        ui.horizontal(|ui| {
            ui.set_width(200.0);
            let res = ui.text_edit_singleline(&mut self.name);
            if ui.button("Create").clicked() {
                let mut query = world.query_filtered::<Entity, With<map::Map>>();
                let map = query.single(world);

                world
                    .spawn((
                        Name::new(format!("layer: {}", self.name)),
                        map::Layer::new(std::mem::take(&mut self.name)),
                        SpatialBundle::default(),
                    ))
                    .set_parent(map);
                return false;
            }
            res.request_focus();
            true
        })
        .inner
    }
}
