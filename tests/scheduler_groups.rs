use decs::ecs::Ecs;
use decs::frame::Frame;
use decs::system; // for `system!`
use decs::view::{View, ViewMut};
use decs::world::World;
use decs_macros::Component;
use std::sync::Once;

#[derive(Clone, Debug, PartialEq, Component)]
struct Position { x: f32, y: f32 }

#[derive(Clone, Debug, PartialEq, Component)]
struct Velocity { x: f32, y: f32 }

static INIT: Once = Once::new();
fn register_components_once() {
    INIT.call_once(|| {
        Ecs::register::<Position>();
        Ecs::register::<Velocity>();
    });
}

// No InitializationGroup in current world; use Simulation/Cleanup only
system!(SimSystem { query fn update(pos: &mut ViewMut<Position>, _vel: View<Velocity>) { pos.x += 1.0; } Parent=[decs::world::SimulationGroup] });
system!(CleanupSystemLocal { query fn update(_pos: View<Position>) { let _ = _pos.x; } Parent=[decs::world::CleanupGroup] });

#[test]
fn scheduler_respects_group_ordering() {
    register_components_once();
    let mut world = World::new();
    {
        let f = Frame::new(world.current_tick());
        let pos = world.get_storage_mut::<Position>();
        pos.set(&f, 0, Position { x: 0.0, y: 0.0 });
        let vel = world.get_storage_mut::<Velocity>();
        vel.set(&f, 0, Velocity { x: 1.0, y: 1.0 });
    }

    let s_sim = SimSystem::new(&mut world);
    let s_cleanup = CleanupSystemLocal::new(&mut world);
    world.scheduler_mut().add_system(s_sim);
    world.scheduler_mut().add_system(s_cleanup);
    world.scheduler_mut().build_wavefronts();
    world.run();
    assert!(world.verify_invariants());

    let pos_ptr = world.get_storage::<Position>();
    let p = unsafe { (*pos_ptr).get(0).unwrap() };
    assert_eq!(p.x, 1.0);
}

system!(NoopSystem { query fn update(_pos: View<Position>) { let _ = _pos.x; } });

#[test]
fn scheduler_can_build_multiple_wavefronts() {
    register_components_once();
    let mut world = World::new();
    {
        let f = Frame::new(world.current_tick());
        let pos = world.get_storage_mut::<Position>();
        pos.set(&f, 1, Position { x: 2.0, y: 3.0 });
    }
    let s1 = NoopSystem::new(&mut world);
    let s2 = NoopSystem::new(&mut world);
    world.scheduler_mut().add_system(s1);
    world.scheduler_mut().add_system(s2);
    world.scheduler_mut().build_wavefronts();
    world.run();
    assert!(world.verify_invariants());
}
