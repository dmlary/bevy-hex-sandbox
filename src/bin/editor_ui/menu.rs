use bevy::prelude::*;
use bevy_egui::egui;
use hex_sandbox::{file_picker, prelude::*, ui, ui::widget::*};

use crate::EditorUiEvent;

#[derive(Default, Clone)]
pub struct EditorMenuBar;

// Inside the ListView widget:
impl BasicWidget for EditorMenuBar {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self::default()
    }
    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, _id: egui::Id) {
        egui::menu::bar(ui, |ui| {
            egui::menu::menu_button(ui, "File", |ui| {
                let id = ui.id().with("file");
                basic_widget::<MapNew>(world, ui, id.with("map_new"));
                basic_widget::<MapOpen>(world, ui, id.with("map_open"));
                ui.separator();
                basic_widget::<MapSave>(world, ui, id.with("map_save"));
                basic_widget::<MapSaveAs>(world, ui, id.with("map_save_as"));
                ui.separator();
                basic_widget::<MapClose>(world, ui, id.with("map_close"));
                basic_widget::<Quit>(world, ui, id.with("quit"));
            });
            egui::menu::menu_button(ui, "Edit", |ui| {
                let id = ui.id().with("edit");
                basic_widget::<Undo>(world, ui, id.with("undo"));
                basic_widget::<Redo>(world, ui, id.with("redo"));
                ui.separator();
                basic_widget::<Cut>(world, ui, id.with("cut"));
                basic_widget::<MenuCopy>(world, ui, id.with("copy"));
                basic_widget::<Paste>(world, ui, id.with("paste"));
            });
            egui::menu::menu_button(ui, "View", |ui| {
                // don't need widgets here as all of these are simple checkboxes
                let mut state = world.resource_mut::<crate::EditorState>();
                if ui.checkbox(&mut state.right_panel, "Right Panel").clicked() {
                    ui.close_menu();
                }
                if ui
                    .checkbox(&mut state.properties_window, "Properties")
                    .clicked()
                {
                    ui.close_menu();
                }
                ui.separator();
                if ui
                    .checkbox(&mut state.inspector, "World Inspector")
                    .clicked()
                {
                    ui.close_menu();
                }
                if ui
                    .checkbox(&mut state.egui_visuals_window, "egui Settings")
                    .clicked()
                {
                    ui.close_menu();
                }
                if ui.checkbox(&mut state.egui_debug, "egui Debug").clicked() {
                    ui.close_menu();
                }
            });
        });
    }
}

#[derive(Default, Clone)]
pub struct MapNew;

impl BasicWidget for MapNew {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self::default()
    }
    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, _id: egui::Id) {
        if !ui.button("New Map").clicked() {
            return;
        }

        let state = world.resource::<crate::EditorState>();
        if state.unsaved_changes {
            let (save_label, save_event) = match &state.map_path {
                Some(path) => ("Save", EditorUiEvent::MapSave(path.clone())),
                None => ("Save As...", EditorUiEvent::MapSaveAs),
            };
            let dialog = ui::ConfirmationDialog::new(
                "Warning: Unsaved Changes",
                "There are unsaved changes to this map.  Would you like to save them?",
            )
            .button("Cancel", None)
            .button("Discard Changes", Some(EditorUiEvent::MapNew))
            .button(save_label, Some(save_event));

            world.spawn(dialog);
        }

        let mut events = world.resource_mut::<Events<crate::EditorUiEvent>>();
        events.send(EditorUiEvent::MapNew);

        ui.close_menu();
    }
}

#[derive(Default, Clone)]
pub struct MapOpen;

impl BasicWidget for MapOpen {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self::default()
    }
    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, _id: egui::Id) {
        if ui.button("Open Map...").clicked() {
            world.spawn(file_picker::Picker::new(crate::PickerEvent::MapLoad(None)).build());
            ui.close_menu();
        }
    }
}

#[derive(Default, Clone)]
pub struct MapSave;

impl BasicWidget for MapSave {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self::default()
    }
    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, _id: egui::Id) {
        let state = world.resource::<crate::EditorState>();

        let Some(path) = &state.map_path else {
            if ui.add_enabled(false, egui::Button::new("Save Map")).clicked() {
                unreachable!();
            }
            return;
        };

        if ui.button("Save Map").clicked() {
            let event = EditorUiEvent::MapSave(path.clone());
            let mut events = world.resource_mut::<Events<crate::EditorUiEvent>>();
            events.send(event);
            ui.close_menu();
        }
    }
}

#[derive(Default, Clone)]
pub struct MapSaveAs;

impl BasicWidget for MapSaveAs {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self::default()
    }
    fn draw(&mut self, mut world: &mut World, ui: &mut egui::Ui, _id: egui::Id) {
        if world.get_map().is_err() {
            if ui
                .add_enabled(false, egui::Button::new("Save As..."))
                .clicked()
            {
                unreachable!();
            }
            return;
        };

        if ui.button("Save As...").clicked() {
            let mut events = world.resource_mut::<Events<crate::EditorUiEvent>>();
            events.send(EditorUiEvent::MapSaveAs);
            ui.close_menu();
        }
    }
}

#[derive(Default, Clone)]
pub struct MapClose;

impl BasicWidget for MapClose {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self::default()
    }

    fn draw(&mut self, mut world: &mut World, ui: &mut egui::Ui, _id: egui::Id) {
        if world.get_map().is_err() {
            if ui
                .add_enabled(false, egui::Button::new("Close Map"))
                .clicked()
            {
                unreachable!();
            }
            return;
        };

        if !ui.button("Close Map").clicked() {
            return;
        }

        let state = world.resource::<crate::EditorState>();
        if state.unsaved_changes {
            let (save_label, save_event) = match &state.map_path {
                Some(path) => ("Save", EditorUiEvent::MapSave(path.clone())),
                None => ("Save As...", EditorUiEvent::MapSaveAs),
            };
            let dialog = ui::ConfirmationDialog::new(
                "Warning: Unsaved Changes",
                "There are unsaved changes to this map.  Would you like to save them?",
            )
            .button("Cancel", None)
            .button("Discard Changes", Some(EditorUiEvent::MapClose))
            .button(save_label, Some(save_event));

            world.spawn(dialog);
        }

        let mut events = world.resource_mut::<Events<crate::EditorUiEvent>>();
        events.send(EditorUiEvent::MapClose);

        ui.close_menu();
    }
}

#[derive(Default, Clone)]
pub struct Quit;

impl BasicWidget for Quit {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self::default()
    }

    fn draw(&mut self, _world: &mut World, ui: &mut egui::Ui, _id: egui::Id) {
        if ui.button("Quit").clicked() {
            debug!("quit");
            ui.close_menu();
            std::process::exit(0);
        }
    }
}

#[derive(Default, Clone)]
pub struct Undo;

impl BasicWidget for Undo {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self::default()
    }

    fn draw(&mut self, _world: &mut World, ui: &mut egui::Ui, _id: egui::Id) {
        if ui.button("Undo").clicked() {
            debug!("undo");
            ui.close_menu();
        }
    }
}

#[derive(Default, Clone)]
pub struct Redo;

impl BasicWidget for Redo {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self::default()
    }

    fn draw(&mut self, _world: &mut World, ui: &mut egui::Ui, _id: egui::Id) {
        if ui.button("Redo").clicked() {
            debug!("redo");
            ui.close_menu();
        }
    }
}

#[derive(Default, Clone)]
pub struct Cut;

impl BasicWidget for Cut {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self::default()
    }

    fn draw(&mut self, _world: &mut World, ui: &mut egui::Ui, _id: egui::Id) {
        if ui.button("Cut").clicked() {
            debug!("cut");
            ui.close_menu();
        }
    }
}

#[derive(Default, Clone)]
pub struct MenuCopy;

impl BasicWidget for MenuCopy {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self::default()
    }

    fn draw(&mut self, _world: &mut World, ui: &mut egui::Ui, _id: egui::Id) {
        if ui.button("Copy").clicked() {
            debug!("copy");
            ui.close_menu();
        }
    }
}

#[derive(Default, Clone)]
pub struct Paste;

impl BasicWidget for Paste {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self::default()
    }

    fn draw(&mut self, _world: &mut World, ui: &mut egui::Ui, _id: egui::Id) {
        if ui.button("Paste").clicked() {
            debug!("paste");
            ui.close_menu();
        }
    }
}
