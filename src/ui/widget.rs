use bevy::prelude::*;
use bevy_egui::egui;
use std::collections::HashMap;

/// storing widget states
#[derive(Resource)]
struct WidgetState<W: 'static + Sync + Send>(HashMap<egui::Id, W>);

pub trait BasicWidget: Send + Sync {
    fn new(world: &mut World, ui: &egui::Ui) -> Self;
    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, id: egui::Id);
}

pub fn basic_widget<W: BasicWidget + 'static>(world: &mut World, ui: &mut egui::Ui, id: egui::Id) {
    if !world.contains_resource::<WidgetState<W>>() {
        world.insert_resource(WidgetState::<W>(HashMap::new()));
    }
    world.resource_scope(|world, mut states: Mut<WidgetState<W>>| {
        let state = states.0.entry(id).or_insert(W::new(world, ui));
        state.draw(world, ui, id);
    });
}

/// egui widget that takes an argument and returns a value
pub trait FnWidget<Arg = (), Output = ()>: Send + Sync {
    type Arg;
    type Output;

    fn new(world: &mut World, ui: &egui::Ui) -> Self;
    fn draw(
        &mut self,
        world: &mut World,
        ui: &mut egui::Ui,
        id: egui::Id,
        arg: Self::Arg,
    ) -> Self::Output;
}

pub fn fn_widget<W: FnWidget + 'static>(
    world: &mut World,
    ui: &mut egui::Ui,
    id: egui::Id,
    arg: <W as FnWidget>::Arg,
) -> <W as FnWidget>::Output {
    if !world.contains_resource::<WidgetState<W>>() {
        world.insert_resource(WidgetState::<W>(HashMap::new()));
    }
    world.resource_scope(|world, mut states: Mut<WidgetState<W>>| {
        let state = states.0.entry(id).or_insert(W::new(world, ui));
        state.draw(world, ui, id, arg)
    })
}

pub trait PopupWidget: Send + Sync {
    fn new(world: &mut World, ui: &egui::Ui) -> Self;
    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, id: egui::Id) -> bool;
}

/// display a popup widget
pub fn popup_widget<Inner: PopupWidget + 'static>(
    show: &mut bool,
    widget_response: &egui::Response,
    world: &mut World,
    ui: &mut egui::Ui,
    id: egui::Id,
) {
    if !*show {
        return;
    }

    if !world.contains_resource::<WidgetState<Inner>>() {
        world.insert_resource(WidgetState::<Inner>(HashMap::new()));
    }

    let res = egui::Area::new(id)
        .order(egui::Order::Foreground)
        .constrain(true)
        .fixed_pos(widget_response.rect.left_bottom())
        .pivot(egui::Align2::RIGHT_TOP)
        .show(ui.ctx(), |ui| {
            ui.set_width(200.0);
            let frame = egui::Frame::popup(ui.style());
            let frame_margin = frame.total_margin();
            frame
                .show(ui, |ui| {
                    ui.set_width(widget_response.rect.width() - frame_margin.sum().x);

                    world.resource_scope(|world, mut states: Mut<WidgetState<Inner>>| {
                        let state = states.0.entry(id).or_insert(Inner::new(world, ui));
                        state.draw(world, ui, id.with("inner"))
                    })
                })
                .inner
        });

    // the inner can return false to close the popup, so apply any changes now
    *show = res.inner;

    let click_pos = ui.ctx().input(|i| {
        if i.pointer.any_click() {
            i.pointer.interact_pos()
        } else {
            None
        }
    });

    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        *show = false;
    }

    // If the popup is outside of the clip_rect for the UI, the
    // res.response.rect isn't updated for being translated into the
    // clip_rect.
    //
    // We adjust for that here.
    let mut popup_rect = res.response.rect;
    let clip_rect = ui.clip_rect();
    if !clip_rect.contains_rect(popup_rect) {
        let clip_max = clip_rect.max;
        let popup_max = popup_rect.max;
        let delta = egui::Vec2::new(
            (clip_max.x - popup_max.x).clamp(f32::NEG_INFINITY, 0.0),
            (clip_max.y - popup_max.y).clamp(f32::NEG_INFINITY, 0.0),
        );
        popup_rect = popup_rect.translate(delta);
    }

    // egui's popup doesn't properly check to see if the click happens
    // inside the popup
    if let Some(pos) = click_pos {
        if !popup_rect.contains(pos) && widget_response.clicked_elsewhere() {
            *show = false;
        }
    }

    if !*show {
        world.resource_mut::<WidgetState<Inner>>().0.remove(&id);
    }
}
