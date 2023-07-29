use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

pub mod widget;
pub mod widgets;

#[derive(Component)]
pub struct ConfirmationDialog<E: Event> {
    pub title: &'static str,
    pub message: &'static str,
    pub buttons: [Option<(&'static str, Option<E>)>; 3],
}

impl<E: Event> ConfirmationDialog<E> {
    pub fn new(title: &'static str, message: &'static str) -> Self {
        Self {
            title,
            message,
            buttons: [None, None, None],
        }
    }
    pub fn simple(title: &'static str, message: &'static str, event: E) -> Self {
        Self {
            title,
            message,
            buttons: [
                Some(("Continue", Some(event))),
                Some(("Cancel", None)),
                None,
            ],
        }
    }

    pub fn button(mut self, message: &'static str, event: Option<E>) -> Self {
        for i in 0..self.buttons.len() {
            if self.buttons[i].is_none() {
                self.buttons[i] = Some((message, event));
                return self;
            }
        }
        panic!("ConfirmationDialog is limited to three buttons");
    }
}

pub fn draw_confirmation_dialog<E: Event>(
    mut commands: Commands,
    mut contexts: EguiContexts,
    mut events: EventWriter<E>,
    mut dialogs: Query<(Entity, &mut ConfirmationDialog<E>)>,
) {
    let ctx = contexts.ctx_mut();

    for (entity, mut dialog) in &mut dialogs {
        egui::Window::new(dialog.title)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label(dialog.message);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    for (label, event) in dialog.buttons.iter_mut().flatten().rev() {
                        if ui.button(*label).clicked() {
                            if let Some(event) = event.take() {
                                events.send(event);
                            }
                            commands.entity(entity).despawn();
                        }
                    }
                });
            });
    }
}

/// get access to both the world and the egui context
pub fn with_world_and_egui_context<T>(
    world: &mut World,
    f: impl FnOnce(&mut World, &mut egui::Context) -> T,
) -> T {
    use bevy::window::PrimaryWindow;
    use bevy_egui::EguiContext;

    let mut state = world.query_filtered::<Entity, (With<EguiContext>, With<PrimaryWindow>)>();
    let entity = state.single(world);
    let mut egui_context = world.entity_mut(entity).take::<EguiContext>().unwrap();

    let ctx = egui_context.get_mut();
    let res = f(world, ctx);
    world.entity_mut(entity).insert(egui_context);

    res
}
