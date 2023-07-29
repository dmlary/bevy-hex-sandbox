use bevy::prelude::*;
use bevy_egui::egui;
use std::marker::PhantomData;

use crate::ui::widget::*;

#[derive(Clone)]
pub struct VResizePanel<Inner: BasicWidget + 'static> {
    height: f32,
    _phantom: PhantomData<Inner>,
}

impl<Inner: BasicWidget + 'static> BasicWidget for VResizePanel<Inner> {
    fn new(_world: &mut World, ui: &egui::Ui) -> Self {
        Self {
            height: ui.available_height() / 2.0,
            _phantom: PhantomData,
        }
    }

    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, id: egui::Id) {
        egui::ScrollArea::vertical()
            .max_height(self.height)
            .id_source(id.with("vscroll"))
            .show(ui, |ui| {
                basic_widget::<Inner>(world, ui, ui.id().with("inner"));

                // fill in the scroll area so we don't shrink
                ui.allocate_space(ui.available_size());
            });
        self.height = fn_widget::<VDragHandle>(world, ui, id.with("drag_handle"), self.height);
    }
}

#[derive(Clone)]
pub struct VDragHandle;

impl FnWidget for VDragHandle {
    type Arg = f32;
    type Output = f32;

    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self
    }

    fn draw(
        &mut self,
        _world: &mut World,
        ui: &mut egui::Ui,
        _id: egui::Id,
        mut height: f32,
    ) -> f32 {
        let max = height + ui.available_height() - 15.0;
        let size = egui::Vec2::new(ui.available_width(), 8.0);
        let (rect, res) = ui.allocate_at_least(size, egui::Sense::drag());

        let mut stroke = egui::Stroke {
            width: 2.0,
            ..default()
        };

        if res.dragged() {
            stroke.color = ui.visuals().widgets.active.bg_stroke.color;
            height += res.drag_delta().y;
            height = height.clamp(15.0, max);
        } else if res.hovered() {
            stroke.color = ui.visuals().widgets.hovered.bg_stroke.color;
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        } else {
            stroke.color = ui.visuals().widgets.noninteractive.bg_stroke.color;
        }

        if ui.is_rect_visible(res.rect) {
            let painter = ui.painter();
            painter.hline(
                (rect.left() - 8.0)..=(rect.right() + 8.0),
                painter.round_to_pixel(rect.center().y),
                stroke,
            );
        }

        height
    }
}

#[derive(Default, Clone)]
pub struct PanelTitle;

impl FnWidget for PanelTitle {
    type Arg = &'static str;
    type Output = ();

    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self::default()
    }

    fn draw(&mut self, _world: &mut World, ui: &mut egui::Ui, _id: egui::Id, title: &str) {
        ui.label(egui::RichText::new(title).heading());
    }
}
