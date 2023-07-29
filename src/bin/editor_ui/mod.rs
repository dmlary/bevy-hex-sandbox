use bevy::ecs::system::SystemState;
use bevy::prelude::*;
use hex_sandbox::{tileset, ui::widget::*};

mod menu;
mod panel;
mod tile_properties;

pub use menu::EditorMenuBar;
pub use panel::EditorPanel;
pub use tile_properties::TileProperties;

pub struct CreateTileset {
    just_opened: bool,
    name: String,
}

impl BasicWidget for CreateTileset {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self {
            name: "New Tileset".to_string(),
            just_opened: true,
        }
    }

    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, _id: egui::Id) {
        use crate::EditorState;

        ui.set_width(200.0);
        let text_box = ui.text_edit_singleline(&mut self.name);

        let (create, cancel) = ui
            .with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                (ui.button("Create"), ui.button("Cancel"))
            })
            .inner;

        if self.just_opened {
            self.just_opened = false;
            return;
        }

        if create.clicked()
            || text_box.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
        {
            let id = world.spawn(tileset::Tileset::new(&self.name)).id();
            let mut state = world.resource_mut::<EditorState>();
            state.active_tileset = Some(id);
            state.new_tileset_window = false;
            *self = Self::new(world, ui);
            return;
        }
        if cancel.clicked()
            || ui.input(|i| i.key_pressed(egui::Key::Escape))
            || ui.input(|i| i.pointer.any_click() && !ui.ui_contains_pointer())
        {
            let mut state = world.resource_mut::<EditorState>();
            state.new_tileset_window = false;
            *self = Self::new(world, ui);
            return;
        }

        text_box.request_focus();
    }
}

pub struct EguiDebug<'w: 'static, 's: 'static> {
    system_state: SystemState<(
        Query<'w, 's, &'static leafwing_input_manager::prelude::ActionState<crate::InputActions>>,
    )>,
}

impl<'w, 's> BasicWidget for EguiDebug<'w, 's> {
    fn new(world: &mut World, _ui: &egui::Ui) -> Self {
        Self {
            system_state: SystemState::new(world),
        }
    }

    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, id: egui::Id) {
        let action_state = self.system_state.get_mut(world).0;
        let actions = action_state.single();
        egui::Grid::new(id.with("basic"))
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("ctx.wants_pointer_input");
                ui.label(format!("{:?}", ui.ctx().wants_pointer_input()));
                ui.end_row();
                ui.label("ctx.is_pointer_over_area");
                ui.label(format!("{:?}", ui.ctx().is_pointer_over_area()));
                ui.end_row();
                ui.label("ctx.is_using_pointer");
                ui.label(format!("{:?}", ui.ctx().is_using_pointer()));
                ui.end_row();
                ui.label("ActionState Y scroll");
                ui.label(format!(
                    "{:#?}",
                    actions.value(crate::InputActions::CameraScale)
                ));
                ui.end_row();

                ui.label("ctx.layer_id_at");
                if let Some(pointer_pos) = ui.ctx().input(|i| i.pointer.interact_pos()) {
                    ui.label(format!("{:?}", ui.ctx().layer_id_at(pointer_pos)));
                } else {
                    ui.label("no pointer pos");
                }
                ui.end_row();
            });
    }
}
