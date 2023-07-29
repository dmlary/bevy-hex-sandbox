use bevy::ecs::system::BoxedSystem;
use bevy::prelude::*;
use std::marker::PhantomData;

#[derive(Resource)]
struct InitializedSystem<I, O, S>
where
    I: Send + 'static,
    O: Send + 'static,
    S: Send + 'static + Sync,
{
    system: BoxedSystem<I, O>,
    _phantom: PhantomData<S>,
}

pub fn run_system<I, O, S, Marker>(world: &mut World, input: I, system: S) -> O
where
    I: Send + 'static,
    O: Send + 'static,
    S: IntoSystem<I, O, Marker> + Send + 'static + Sync,
{
    // get the initialized system
    let mut system = match world.remove_resource::<InitializedSystem<I, O, S>>() {
        Some(system) => system,
        None => {
            let mut sys = IntoSystem::into_system(system);
            sys.initialize(world);
            InitializedSystem::<I, O, S> {
                system: Box::new(sys),
                _phantom: PhantomData::<S> {},
            }
        }
    };

    // run the system
    let result = system.system.run(input, world);

    // apply any changes
    // XXX probably not the best place to do this; maybe need to note which
    // systems need to be flushed and do it all as a single system?
    system.system.apply_buffers(world);

    // put the system back
    world.insert_resource(system);

    result
}
