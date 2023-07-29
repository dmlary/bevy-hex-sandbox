#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

use anyhow::{bail, Context, Result};
use bevy::ecs::event::ManualEventReader;
use bevy::{core_pipeline::tonemapping::Tonemapping, prelude::*};
use bevy_dolly::prelude::*;
use bevy_egui::{egui, EguiContexts};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_mod_picking::prelude::*;
use bevy_mod_sysfail::macros::*;
use leafwing_input_manager::prelude::*;

use hex_sandbox::{file_picker, map, persistence, prelude::*, tileset};

mod editor_ui;
use editor_ui as ui;

fn main() -> Result<()> {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "hex sandbox".to_string(),
            ..default()
        }),
        ..default()
    }))
    .add_plugin(InputManagerPlugin::<InputActions>::default())
    .add_plugin(WorldInspectorPlugin::default().run_if(inspector_enabled))
    .add_plugins(
        DefaultPickingPlugins
            .build()
            .disable::<DebugPickingPlugin>(),
    )
    .add_plugin(file_picker::Plugin::<PickerEvent>::default())
    .add_plugin(hex_sandbox::thumbnail_render::Plugin)
    .add_plugin(tileset::Plugin)
    .add_plugin(map::Plugin)
    .add_plugin(persistence::Plugin)
    .insert_resource(EditorState::default())
    .insert_resource(TileSelection::default())
    .add_event::<PickerEvent>()
    .add_event::<EditorUiEvent>()
    .add_event::<MapCursorMoveEvent>()
    .register_type::<MapCursor>()
    .add_startup_system(setup)
    .add_system(Dolly::<MainCamera>::update_active)
    .add_systems((
        draw_ui,
        // must handle input after drawing ui to work around egui issue:
        // https://github.com/emilk/egui/issues/2690#issuecomment-1593439516
        //
        // The egui context must be updated with the panel locations
        handle_input.after(draw_ui),
        handle_ui_events,
        handle_picker_events,
        handle_map_cursor_events,
        hex_sandbox::ui::draw_confirmation_dialog::<EditorUiEvent>,
        // update_cursor,
        update_cursor_model,
        map_loaded,
    ));

    // XXX to help debug leafwing/egui ordering issue
    // dump_main_schedule(&mut app)?;

    app.run();
    Ok(())
}

#[allow(dead_code)]
fn dump_main_schedule(app: &mut App) -> Result<()> {
    use chrono::*;
    use std::fs::File;
    use std::io::prelude::*;

    let dot = bevy_mod_debugdump::schedule_graph_dot(
        app,
        CoreSchedule::Main,
        &bevy_mod_debugdump::schedule_graph::Settings {
            prettify_system_names: false,
            ..default()
        }
        .filter_name(|name| {
            name.contains("egui") || name.contains("leafwing") || name.contains("editor")
        }),
    );

    let now: DateTime<Local> = Local::now();
    let mut f = File::create(format!(
        "schedule-order_{}.dot",
        now.to_rfc3339_opts(SecondsFormat::Secs, true),
    ))?;
    f.write_all(&dot.into_bytes())?;
    Ok(())
}

fn setup(
    mut commands: Commands,
    mut contexts: EguiContexts,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    use egui::epaint::{Color32, Shadow};

    let ctx = contexts.ctx_mut();
    catppuccin_egui::set_theme(ctx, catppuccin_egui::LATTE);

    let mut vis = ctx.style().visuals.clone();
    vis.window_shadow = Shadow {
        extrusion: 5.0,
        color: Color32::from_black_alpha(96),
    };
    ctx.set_visuals(vis);

    commands.insert_resource(EditorState::default());
    commands.insert_resource(EditorUiEventReader::default());

    // input handler
    commands.spawn((InputManagerBundle::<InputActions> {
        action_state: ActionState::default(),
        input_map: input_map(),
    },));

    // Add the world camera
    commands
        .spawn((
            MainCamera,
            bevy::render::view::RenderLayers::layer(0),
            Camera3dBundle {
                tonemapping: Tonemapping::None,
                projection: OrthographicProjection {
                    near: 0.0,
                    far: 10000.0,
                    scaling_mode: bevy::render::camera::ScalingMode::WindowSize(48.0),
                    ..default()
                }
                .into(),
                ..default()
            },
            Rig::builder()
                .with(Position::new(Vec3::new(0.0, 0.0, 0.0)))
                .with(YawPitch::new().pitch_degrees(-30.0).yaw_degrees(45.0))
                .with(Smooth::new_position(0.3))
                .with(Smooth::new_rotation(1.0))
                .with(Arm::new(Vec3::Z * 200.0))
                .build(),
            RaycastPickCamera::default(),
            Visibility::Visible,
            ComputedVisibility::default(),
        ))
        .with_children(|commands| {
            // Add a directional light (the sun)
            commands.spawn((DirectionalLightBundle {
                directional_light: DirectionalLight {
                    illuminance: 18000.0,
                    ..default()
                },
                // -0.7, -0.5
                transform: Transform::from_rotation(Quat::from_euler(
                    EulerRot::XYZ,
                    -0.6,
                    -0.5,
                    0.0,
                )),
                ..default()
            },));
        });

    // fun colors
    commands.insert_resource(ClearColor(Color::rgb(1.0, 210.0 / 255.0, 202.0 / 255.0)));

    // Also ambient light
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 0.15,
    });

    // let's create a plane for picking grid positions
    commands.spawn((
        Name::new("grid_selection_plane"),
        GridSelectionPlane,
        PbrBundle {
            mesh: meshes.add(Mesh::from(shape::Plane::from_size(10000.0))),
            material: materials.add(Color::NONE.into()),
            ..default()
        },
        PickableBundle::default(),
        RaycastPickTarget::default(),
        OnPointer::<Move>::send_event::<MapCursorMoveEvent>(),
    ));

    // and a cursor
    commands.spawn((
        Name::new("map_cursor"),
        MapCursor::default(),
        tileset::TileTransform::default(),
        SpatialBundle::default(),
    ));
}

fn inspector_enabled(state: Res<EditorState>) -> bool {
    state.inspector
}

#[derive(Resource, Debug)]
struct EditorState {
    // UI elements
    inspector: bool,           // display World Inspector
    right_panel: bool,         // display left panel
    egui_visuals_window: bool, // display egui visuals window
    properties_window: bool,   // show the properties window
    egui_debug: bool,          // show the egui debugging window
    new_tileset_window: bool,  // show create tileset window

    //editor state
    map_path: Option<std::path::PathBuf>, // current loaded map path
    unsaved_changes: bool,                // tracks if there are unsaved changes
    active_layer: Option<Entity>,         // selected layer in the ui
    active_tileset: Option<Entity>,       // active tileset
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            inspector: false,
            right_panel: true,
            egui_visuals_window: false,
            properties_window: true,
            egui_debug: false,
            new_tileset_window: false,
            map_path: None,
            active_tileset: None,
            active_layer: None,
            unsaved_changes: false,
        }
    }
}

#[derive(Default, Debug, Reflect, Clone)]
enum EditorSelection {
    #[default]
    None,
    TilesetTile(tileset::TileRef),
    TilesetTiles {
        tileset: Entity,
        tiles: Vec<tileset::TileId>,
    },
    Layer(Entity),
    Tileset(Entity),
}

#[derive(Resource, Default, Debug)]
struct TileSelection {
    tiles: std::collections::HashSet<tileset::TileRef>,
}

impl TileSelection {
    pub fn active_tile(&self) -> Option<&tileset::TileRef> {
        self.tiles.iter().next()
    }
}

#[derive(Debug, Clone)]
enum EditorUiEvent {
    MapNew,
    MapClose,
    MapSave(std::path::PathBuf),
    MapLoad(std::path::PathBuf),
    MapSaveAs,
    DeleteTileset(Entity),
    // UpdateSelection(EditorSelection),
    RedrawMapTiles,
}

/// Events sent for mouse events on the map plane
#[derive(Debug, Clone, Copy)]
struct MapCursorMoveEvent(Vec3);

impl From<ListenedEvent<Move>> for MapCursorMoveEvent {
    fn from(value: ListenedEvent<Move>) -> Self {
        match value.pointer_event.hit.position {
            None => unreachable!(),
            Some(p) => Self(p),
        }
    }
}

#[derive(Resource, Default)]
struct EditorUiEventReader(ManualEventReader<EditorUiEvent>);

// event type for file pickers
#[derive(Debug)]
enum PickerEvent {
    AddTiles {
        tileset_id: Entity,
        files: Option<Vec<std::path::PathBuf>>,
    },
    MapSave(Option<std::path::PathBuf>),
    MapLoad(Option<std::path::PathBuf>),
    TilesetImport(Option<Vec<std::path::PathBuf>>),
    TilesetExport(Entity, Option<std::path::PathBuf>),
}

impl file_picker::PickerEvent for PickerEvent {
    fn set_result(&mut self, result: Vec<std::path::PathBuf>) {
        use PickerEvent::*;

        *self = match *self {
            AddTiles { tileset_id, .. } => AddTiles {
                tileset_id,
                files: Some(result),
            },
            MapSave(_) => MapSave(Some(result[0].clone())),
            MapLoad(_) => MapLoad(Some(result[0].clone())),
            TilesetImport(_) => TilesetImport(Some(result)),
            TilesetExport(t, _) => TilesetExport(t, Some(result[0].clone())),
        };
    }
}

#[derive(Component)]
struct MainCamera;

#[derive(Component)]
struct GridSelectionPlane;

#[derive(Component, Default, Debug, Reflect)]
struct MapCursor {
    position: Vec3,
    grid_location: map::Location,
    tile_transform: tileset::TileTransform,
}

#[derive(Actionlike, PartialEq, Eq, Clone, Copy, Hash, Debug)]
pub enum InputActions {
    MouseMove,
    MouseScrollY,
    LeftClick,
    CameraPan,
    CameraScale,
    CameraRotateCW,
    CameraRotateCCW,
    ResetCamera,
    ZeroCamera,
    CameraControl,
    TileRotateCW,
    TileRotateCCW,
}

#[rustfmt::skip]
fn input_map() -> InputMap<InputActions> {
    InputMap::default()
        .insert(MouseButton::Left, InputActions::LeftClick)
        .insert(DualAxis::mouse_motion(), InputActions::MouseMove)
        .insert(SingleAxis::mouse_wheel_y(), InputActions::CameraScale)
        .insert(KeyCode::RBracket, InputActions::CameraRotateCW)
        .insert(KeyCode::LBracket, InputActions::CameraRotateCCW)
        .insert(KeyCode::Z, InputActions::ResetCamera)
        .insert(KeyCode::Key0, InputActions::ZeroCamera)
        .insert(KeyCode::Space, InputActions::CameraPan)
        .insert(KeyCode::Q, InputActions::TileRotateCW)
        .insert(KeyCode::E, InputActions::TileRotateCCW)
        .build()
}

fn handle_input(
    action_state: Query<&ActionState<InputActions>>,
    mut cursor: Query<&mut tileset::TileTransform, With<MapCursor>>,
    mut camera: Query<(&mut Rig, &mut Projection, &Transform), With<MainCamera>>,
    mut egui_contexts: EguiContexts,
) {
    let actions = action_state.single();
    let (mut rig, mut projection, transform) = camera.single_mut();
    let Projection::Orthographic(projection) = projection.as_mut() else { panic!("wrong scaling mode") };

    // workaround for https://github.com/emilk/egui/issues/2690
    //
    // check if the pointer is an egui region so we can skip any mouse actions
    // that shouldn't be handled by us.
    let mouse_input = !egui_contexts.ctx_mut().is_pointer_over_area();

    if mouse_input && actions.pressed(InputActions::CameraPan) {
        let vector =
            actions.axis_pair(InputActions::MouseMove).unwrap().xy() * -0.02 * projection.scale;

        let (mut euler, axis_angle) = transform.rotation.to_axis_angle();
        euler.x = 0.;
        euler.z = 0.;
        let rotation = Quat::from_axis_angle(euler, axis_angle);

        if let Some(pos) = rig.try_driver_mut::<Position>() {
            pos.translate(rotation * Vec3::new(vector.x, 0.0, vector.y));
        }
    }

    // This is strange, but if we don't pull this every frame, we get jumpy
    // animation when rotating the camera.
    let camera_yp = rig.driver_mut::<YawPitch>();
    if actions.just_pressed(InputActions::CameraRotateCW) {
        let yaw = camera_yp.yaw_degrees + 60.0;
        camera_yp.yaw_degrees = yaw.rem_euclid(360.0);
    } else if actions.just_pressed(InputActions::CameraRotateCCW) {
        let yaw = camera_yp.yaw_degrees - 60.0;
        camera_yp.yaw_degrees = yaw.rem_euclid(360.0);
    }

    if actions.just_pressed(InputActions::ResetCamera) {
        camera_yp.yaw_degrees = 45.0;
        camera_yp.pitch_degrees = -30.0;
        projection.scale = 1.0;
        // XXX on second hit of reset, move to 0,0
    }

    if actions.just_pressed(InputActions::ZeroCamera) {
        camera_yp.yaw_degrees = 0.0;
        camera_yp.pitch_degrees = -90.0;
        projection.scale = 1.0;
    }

    let scale = actions.value(InputActions::CameraScale);
    if mouse_input && scale != 0.0 {
        projection.scale = (projection.scale * (1.0 - scale * 0.005)).clamp(0.001, 15.0);
    }

    let mut tile_transform = cursor.single_mut();
    if actions.just_pressed(InputActions::TileRotateCW) {
        tile_transform.rotation = tile_transform.rotation.clockwise();
    }

    if actions.just_pressed(InputActions::TileRotateCCW) {
        tile_transform.rotation = tile_transform.rotation.counter_clockwise();
    }
}

trait ResultLogger {
    fn log_err(&self);
}

impl<T> ResultLogger for Result<T> {
    fn log_err(&self) {
        if let Err(e) = self {
            error!("{:?}", e);
        }
    }
}

fn handle_ui_events(world: &mut World) {
    use hex_sandbox::util::run_system;
    use EditorUiEvent::*;

    let mut events = world.remove_resource::<Events<EditorUiEvent>>().unwrap();

    for event in events.drain() {
        // trace!("EditorUiEvent::{:?}", event);
        match event {
            // UpdateSelection(s) => run_system(world, s.clone(), set_selection),
            MapNew => {
                run_system(world, (), close_map);
                run_system(world, (), create_map);
            }
            MapClose => run_system(world, (), close_map),
            // need this until ConfirmationDialog supports Fn for button presses
            MapSaveAs => {
                world.spawn(file_picker::Picker::save_dialog(PickerEvent::MapSave(None)).build());
            }
            MapSave(path) => run_system(world, path.clone(), save_map),
            MapLoad(path) => run_system(world, path.clone(), load_map),
            RedrawMapTiles => run_system(world, (), redraw_map_tiles),
            DeleteTileset(entity) => run_system(world, entity, remove_tileset),
        }
    }

    world.insert_resource(events);
}

fn save_map(
    In(path): In<std::path::PathBuf>,
    mut commands: Commands,
    mut state: ResMut<EditorState>,
    map: Query<Entity, With<map::Map>>,
) {
    let Ok(entity) = map.get_single() else {
        warn!("no map loaded");
        return;
    };
    info!("save map to {}", path.to_string_lossy());
    commands.add(persistence::SaveMapCommand::new(path, entity));
    // XXX bug here; should only be updated when finished writing to disk
    state.unsaved_changes = false;
}

fn load_map(In(path): In<std::path::PathBuf>, mut commands: Commands) {
    info!("load map {}", path.to_string_lossy());
    commands.spawn(persistence::MapImporter::new(path));
}

fn close_map(
    mut commands: Commands,
    mut state: ResMut<EditorState>,
    mut tile_selection: ResMut<TileSelection>,
    map: Query<Entity, With<map::Map>>,
    cursor: Query<Entity, With<MapCursor>>,
) {
    if state.unsaved_changes {
        info!("closing map {:?}; discarding changes", state.map_path);
    } else {
        info!("closing map {:?}", state.map_path);
    }

    let cursor = cursor.single();
    commands
        .entity(cursor)
        .remove::<(tileset::TileRef, Handle<Scene>)>()
        .despawn_descendants();
    tile_selection.tiles.clear();

    if let Ok(entity) = map.get_single() {
        commands.entity(entity).despawn_recursive();
    }
    state.map_path = None;
    state.unsaved_changes = false;
    state.active_tileset = None;
    state.active_layer = None;
}

fn create_map(mut commands: Commands, mut state: ResMut<EditorState>) {
    info!("create new map");
    commands
        .spawn((
            Name::new("map"),
            map::Map::default(),
            SpatialBundle::default(),
        ))
        .with_children(|map| {
            let layer = map
                .spawn((
                    Name::new("layer"),
                    map::Layer::new("Background".into()),
                    SpatialBundle::default(),
                ))
                .id();
            state.active_layer = Some(layer);
            let tileset = map
                .spawn((
                    Name::new("tileset"),
                    tileset::Tileset::new("Default Tileset"),
                ))
                .id();
            state.active_tileset = Some(tileset);
        });
    state.map_path = None;
    state.unsaved_changes = false;
}

fn map_loaded(
    mut state: ResMut<EditorState>,
    map: Query<&Children, Added<map::Map>>,
    tilesets: Query<&mut tileset::Tileset>,
    layers: Query<&mut map::Layer>,
) {
    let Ok(map_children) = map.get_single() else { return; };
    for child in map_children {
        if state.active_tileset.is_none() && tilesets.get(*child).is_ok() {
            state.active_tileset = Some(*child);
        }

        if state.active_layer.is_none() && layers.get(*child).is_ok() {
            state.active_layer = Some(*child);
        }
    }
}

fn remove_tileset(
    In(tileset_id): In<Entity>,
    mut state: ResMut<EditorState>,
    mut commands: Commands,
    tilesets: Query<Entity, With<tileset::Tileset>>,
) {
    commands.entity(tileset_id).despawn_recursive();
    state.active_tileset = tilesets.iter().find(|entity| *entity != tileset_id);
}

fn redraw_map_tiles(
    mut commands: Commands,
    tile_selection: Res<TileSelection>,
    tiles: Query<(
        Entity,
        &tileset::TileRef,
        &tileset::TileTransform,
        &map::Location,
    )>,
    tilesets: Query<&tileset::Tileset>,
    map: Query<&map::Map>,
) {
    let Ok(map) = map.get_single() else { return; };
    for (entity, tile_ref, tile_transform, location) in &tiles {
        if !tile_selection.tiles.contains(tile_ref) {
            continue;
        }
        let Ok(tileset) = tilesets.get(tile_ref.tileset) else {
            warn!("unknown tileset {:?} in entity {:?}",
                tile_ref.tileset, entity);
            continue;
        };

        let bundle = tileset::TileBundle::new(
            map,
            *location,
            tile_transform.clone(),
            tileset,
            tile_ref.tileset,
            tile_ref.tile,
        );
        commands.entity(entity).insert(bundle);
    }
}

fn handle_picker_events(
    mut commands: Commands,
    mut picker_events: EventReader<PickerEvent>,
    mut state: ResMut<EditorState>,
    mut tilesets: Query<&mut tileset::Tileset>,
    mut editor_events: EventWriter<EditorUiEvent>,
    map: Query<Entity, With<map::Map>>,
) {
    for event in picker_events.iter() {
        match event {
            PickerEvent::AddTiles { tileset_id, files } => {
                let Ok(mut tileset) = tilesets.get_mut(*tileset_id) else { continue };
                let Some(paths) = files else { continue };
                for path in paths {
                    tileset.add_tile(path.clone());
                }
                state.unsaved_changes = true;
            }
            PickerEvent::MapSave(path) => {
                let Some(path) = path else { continue };
                if state.map_path.is_none() {
                    state.map_path = Some(path.clone());
                }

                editor_events.send(EditorUiEvent::MapSave(path.clone()));
            }
            PickerEvent::MapLoad(path) => {
                let Some(path) = path else { continue };
                if state.map_path.is_none() {
                    state.map_path = Some(path.clone());
                }

                editor_events.send(EditorUiEvent::MapLoad(path.clone()));
            }
            PickerEvent::TilesetImport(paths) => {
                let Some(paths) = paths else { continue };
                let Ok(map) = map.get_single() else {
                    error!("no map found; not loading tileset");
                    continue;
                };
                commands.entity(map).with_children(|map| {
                    for path in paths {
                        let id = map.spawn(tileset::TilesetImporter::new(path.clone())).id();
                        state.active_tileset = Some(id);
                    }
                });
            }
            PickerEvent::TilesetExport(tileset_id, path) => {
                let Some(path) = path else { continue };
                let Ok(tileset) = tilesets.get(*tileset_id) else {
                    warn!("tileset not found: {:?}", event);
                    continue;
                };
                commands.spawn(tileset::TilesetExporter::new(path.clone(), tileset.clone()));
            }
        }
    }
    picker_events.clear();
}

#[sysfail(log)]
fn handle_map_cursor_events(
    mut commands: Commands,
    mut events: EventReader<MapCursorMoveEvent>,
    state: Res<EditorState>,
    map: Query<&map::Map>,
    buttons: Res<Input<MouseButton>>,
    cursor: Query<(Entity, &tileset::TileRef, &tileset::TileTransform), With<MapCursor>>,
    tiles: Query<
        (
            Entity,
            &map::Location,
            &tileset::TileRef,
            &tileset::TileTransform,
            &Parent,
        ),
        Without<MapCursor>,
    >,
) -> Result<()> {
    let Some(event) = events.iter().last() else { return Ok(()) };
    let Ok(map) = map.get_single() else { return Ok(()) };
    let (_, location) = map.snap_to_grid(event.0);

    // update the cursor location
    let Ok((cursor, tile_ref, tile_transform)) = cursor.get_single() else { return Ok(()); };
    commands.entity(cursor).insert(location);
    trace!("move cursor: {:?}, {:?}", event, location);

    // nothing more to be done if no mouse buttons have been pressed
    if buttons.get_pressed().len() == 0 {
        return Ok(());
    }

    let layer = state.active_layer.context("no active layer")?;
    // let start = std::time::Instant::now();

    for (tile_entity, tile_location, tile_tile_ref, tile_tile_transform, tile_parent) in &tiles {
        if tile_parent.get() != layer {
            continue;
        }
        if *tile_location != location {
            continue;
        }

        // if the tile matches, and they're adding a tile do nothing
        if tile_tile_ref == tile_ref
            && tile_tile_transform == tile_transform
            && buttons.pressed(MouseButton::Left)
        {
            return Ok(());
        }

        // we're either removing the tile, or replacing it; so despawn the tile
        commands.entity(tile_entity).despawn_recursive();
    }
    // debug!("tiles {}, duration {:?}", tiles.iter().count(), start.elapsed());

    if buttons.pressed(MouseButton::Left) {
        commands
            .spawn((
                location,
                *tile_ref,
                tile_transform.clone(),
                SpatialBundle::default(),
            ))
            .set_parent(layer);

        debug!("insert tile: {:?} @ {:?}", tile_ref, location);
    }
    Ok(())
}

/// update the cursor model when the TileSelection is changed
#[sysfail(log)]
fn update_cursor_model(
    mut commands: Commands,
    tile_selection: Res<TileSelection>,
    cursor: Query<Entity, With<MapCursor>>,
) -> Result<()> {
    if !tile_selection.is_changed() {
        return Ok(());
    }
    let Some(tile_ref) = tile_selection.active_tile() else { return Ok(()); };

    let cursor = cursor.get_single().context("failed to get cursor entity")?;
    commands.entity(cursor).insert(*tile_ref);
    Ok(())
}

#[sysfail(log)]
fn update_cursor(
    mut commands: Commands,
    map: Query<&map::Map>,
    cursor: Query<(Entity, &MapCursor)>,
    tile_selection: Res<TileSelection>,
    tilesets: Query<&tileset::Tileset>,
) -> Result<()> {
    if !tile_selection.is_changed() {
        return Ok(());
    }
    let map = map.get_single().context("get Map resource")?;
    let tileset::TileRef {
        tileset: tileset_id,
        tile: tile_id,
    } = *tile_selection.active_tile().context("get active tile")?;

    let (entity, cursor) = cursor.single();
    let tileset = tilesets.get(tileset_id)?;
    let tile = tileset.tiles.get(&tile_id).context(format!(
        "tile not found: Tileset {} ({:?}), TileId {}",
        tileset.name, tileset_id, tile_id
    ))?;
    let Some(scene) = &tile.scene else {
        bail!(
            "no scene for tile: Tileset {} ({:?}), TileId {}",
            tileset.name, tileset_id, tile_id
        );
    };

    let transform = map.tile_transform(tile, cursor.grid_location, &cursor.tile_transform);

    commands
        .entity(entity)
        .insert(scene.clone())
        .insert(transform);

    Ok(())
}

pub fn draw_ui(world: &mut World) {
    use hex_sandbox::ui::widget::*;

    hex_sandbox::ui::with_world_and_egui_context(world, |mut world, ctx| {
        // menu bar
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            basic_widget::<ui::EditorMenuBar>(world, ui, ui.id().with("menubar"));
        });

        // right panel
        if world.get_map().is_ok() {
            let state = world.resource_mut::<EditorState>();
            egui::SidePanel::right("right_panel")
                .resizable(true)
                .default_width(200.0)
                .show_animated(ctx, state.right_panel, |ui| {
                    basic_widget::<ui::EditorPanel>(world, ui, ui.id().with("panel"));
                });
        }

        let state = world.resource::<EditorState>();
        let mut property_window = state.properties_window;
        let mut egui_visuals_window = state.egui_visuals_window;
        let mut egui_debug = state.egui_debug;
        let new_tileset_window = state.new_tileset_window;

        // properties window
        egui::Window::new("Properties")
            .open(&mut property_window)
            .constrain(true)
            .hscroll(true)
            .pivot(egui::Align2::RIGHT_TOP)
            .default_width(200.0)
            .default_height(300.0)
            .default_pos([1280.0, 0.0])
            .show(ctx, |ui| {
                basic_widget::<ui::TileProperties>(world, ui, ui.id().with("tile_properties"));
                ui.allocate_space(ui.available_size());
            });

        // egui settings window
        egui::Window::new("🔧 Settings")
            .open(&mut egui_visuals_window)
            .vscroll(true)
            .show(ctx, |ui| ctx.settings_ui(ui));

        if new_tileset_window {
            egui::Window::new("Create New Tileset")
                .anchor(egui::Align2::CENTER_TOP, egui::Vec2::new(0.0, 200.0))
                .show(ctx, |ui| {
                    basic_widget::<ui::CreateTileset>(world, ui, ui.id().with("create_tileset"));
                });
        }

        egui::Window::new("egui pointer debug")
            .open(&mut egui_debug)
            .default_width(200.0)
            .show(ctx, |ui| {
                basic_widget::<ui::EguiDebug>(world, ui, ui.id().with("egui_debug"))
            });

        let mut state = world.resource_mut::<EditorState>();
        state.properties_window = property_window;
        state.egui_visuals_window = egui_visuals_window;
        state.egui_debug = egui_debug;
    });
}
