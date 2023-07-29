use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task},
};
use futures_lite::future;
use rfd::FileDialog;
use std::marker::PhantomData;

pub struct Plugin<E: PickerEvent> {
    phantom: PhantomData<E>,
}

impl<E: PickerEvent> Default for Plugin<E> {
    fn default() -> Self {
        Self {
            phantom: PhantomData,
        }
    }
}

impl<E: PickerEvent> bevy::app::Plugin for Plugin<E> {
    fn build(&self, app: &mut App) {
        app.add_system(update_pickers::<E>);
    }
}

/// Trait that must be implemented on Events used with file_picker
pub trait PickerEvent: Event {
    fn set_result(&mut self, result: Vec<std::path::PathBuf>);
}

#[derive(Debug)]
enum PickerMode {
    Save,
    OpenOne,
    OpenMany,
}

#[derive(Debug)]
pub struct Picker<E: PickerEvent> {
    mode: PickerMode,
    dialog: FileDialog,
    event: E,
}

impl<E: PickerEvent> Picker<E> {
    pub fn new(event: E) -> Self {
        Self {
            mode: PickerMode::OpenOne,
            dialog: FileDialog::new(),
            event,
        }
    }
    pub fn for_many(event: E) -> Self {
        Self {
            mode: PickerMode::OpenMany,
            dialog: FileDialog::new(),
            event,
        }
    }
    pub fn save_dialog(event: E) -> Self {
        Self {
            mode: PickerMode::Save,
            dialog: FileDialog::new(),
            event,
        }
    }

    pub fn add_filter(mut self, desc: &str, extensions: &[&str]) -> Self {
        self.dialog = self.dialog.add_filter(desc, extensions);
        self
    }

    pub fn build(self) -> PickerDialog<E> {
        let task_pool = IoTaskPool::get();
        let task = match self.mode {
            PickerMode::OpenOne => {
                task_pool.spawn(async move { self.dialog.pick_file().map(|p| vec![p]) })
            }
            PickerMode::Save => {
                task_pool.spawn(async move { self.dialog.save_file().map(|p| vec![p]) })
            }
            PickerMode::OpenMany => task_pool.spawn(async move { self.dialog.pick_files() }),
        };
        PickerDialog {
            task,
            event: Some(self.event),
        }
    }
}

#[derive(Component, Debug)]
pub struct PickerDialog<E: PickerEvent> {
    task: Task<Option<Vec<std::path::PathBuf>>>,
    event: Option<E>, // only an option here for take()
}

fn update_pickers<E: PickerEvent>(
    mut commands: Commands,
    mut pickers: Query<(Entity, &mut PickerDialog<E>)>,
    mut events: EventWriter<E>,
) {
    for (entity, mut task) in &mut pickers {
        if let Some(result) = future::block_on(future::poll_once(&mut task.task)) {
            let mut event = task.event.take().unwrap();
            if let Some(result) = result {
                event.set_result(result);
            }
            events.send(event);
            commands.entity(entity).despawn();
        }
    }
}
