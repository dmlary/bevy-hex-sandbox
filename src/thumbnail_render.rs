use bevy::{
    core_pipeline::tonemapping::Tonemapping, prelude::*, render::view::RenderLayers,
    scene::SceneInstance,
};
use std::collections::VecDeque;

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system(setup).add_system(render_thumbnails);
    }
}

fn setup(mut commands: Commands) {
    commands.insert_resource(RenderQueue::default());

    // add a thumbnail rendering camera
    commands.spawn((
        Name::new("thumbnail_render::camera"),
        ThumbnailCamera,
        bevy::render::view::RenderLayers::layer(crate::constants::THUMBNAIL_RENDER_LAYER),
        Camera3dBundle {
            camera_3d: Camera3d {
                clear_color: bevy::core_pipeline::clear_color::ClearColorConfig::Custom(
                    Color::NONE,
                ),
                ..default()
            },
            camera: Camera {
                // render before the "main pass" camera
                order: -1,
                is_active: false,
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(3.0, 2.5, 3.0))
                .looking_at(Vec3::new(0.0, 0.25, 0.0), Vec3::Y),
            tonemapping: Tonemapping::None,
            projection: OrthographicProjection {
                near: -100.0,
                far: 100.0,
                scaling_mode: bevy::render::camera::ScalingMode::Fixed {
                    width: 1.3,
                    height: 1.3,
                },
                scale: 1.0,
                ..default()
            }
            .into(),
            ..default()
        },
    ));
}

#[derive(Resource, Default, Debug)]
pub struct RenderQueue {
    queue: VecDeque<(Handle<Image>, Handle<Scene>)>,
    scene: Option<Entity>,
}

impl RenderQueue {
    pub fn push(&mut self, image: Handle<Image>, scene: Handle<Scene>) {
        self.queue.push_back((image, scene));
    }
}

#[derive(Component)]
struct ThumbnailCamera;

#[derive(Component)]
struct ThumbnailScene;

fn render_thumbnails(
    mut commands: Commands,
    mut render_queue: ResMut<RenderQueue>,
    mut camera: Query<(&mut Camera, &RenderLayers), With<ThumbnailCamera>>,
    scene_instances: Query<&SceneInstance, With<ThumbnailScene>>,
    scene_manager: Res<SceneSpawner>,
) {
    use bevy::render::camera::RenderTarget;

    let (mut camera, render_layers) = camera
        .get_single_mut()
        .expect("a single ThumbnailCamera to exist");

    // if we're working on an existing scene, see if it's loaded
    if let Some(scene) = render_queue.scene {
        if let Ok(instance) = scene_instances.get(scene) {
            // check if the scene has been loaded
            if !scene_manager.instance_is_ready(**instance) {
                debug!("scene not loaded {:?}", scene);
                return;
            }

            // scene is loaded, update all the child entities to be in the
            // proper render layer
            for entity in scene_manager.iter_instance_entities(**instance) {
                commands.entity(entity).insert(*render_layers);
            }

            // enable the camera, and clear the tag; we'll render the scene to
            // the image, then despawn the scene entity on the next call of
            // this system.
            debug!("render thumbnail {:?}", scene);
            camera.is_active = true;
            commands
                .entity(scene)
                .remove::<ThumbnailScene>()
                .insert(Visibility::Visible);
            return;
        } else {
            debug!("despawn thumbnail {:?}", scene);
            camera.is_active = false;
            commands.entity(scene).despawn_recursive();
            render_queue.scene = None;
        }
    }

    // scene has been loaded, so let's pop the request off the queue
    let Some((image, scene)) = render_queue.queue.pop_front() else { return };

    // update camera to write to the new image
    camera.target = RenderTarget::Image(image);

    // spawn the new model
    let entity = commands
        .spawn((
            ThumbnailScene,
            SceneBundle {
                scene,
                visibility: Visibility::Hidden,
                ..default()
            },
            *render_layers,
        ))
        .id();
    render_queue.scene = Some(entity);
    debug!("spawn thumbnail {:?}", entity);
}
