use decs::ecs::Ecs;
use decs::frame::Frame;
use decs::system; // for `system!`
use decs::view::View;
use decs::world::World;
use decs_macros::Component;
use std::sync::Once;

#[derive(Clone, Debug, PartialEq, Component)]
struct Position {
    x: f32,
    y: f32,
}

static INIT: Once = Once::new();
fn register_components_once() {
    INIT.call_once(|| {
        Ecs::register::<Position>();
    });
}

system!(NoopSystem { query fn update(_pos: View<Position>) { let _ = _pos.x; } });

#[test]
fn chunk_fullness_equals_presence_mask() {
    register_components_once();
    let mut world = World::new();
    {
        let f = Frame::new(world.current_tick());
        for i in 0..64u32 {
            let pos = world.get_storage_mut::<Position>();
            pos.set(&f, i, Position { x: 0.0, y: 0.0 });
        }
    }
    assert!(world.verify_invariants());
    let sys = NoopSystem::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();
    world.run();
}

#[test]
fn page_count_matches_sum_of_chunk_bits() {
    register_components_once();
    let mut world = World::new();
    {
        let f = Frame::new(world.current_tick());
        for i in [0, 1, 2, 3, 64, 65, 66, 67].iter().copied() {
            let pos = world.get_storage_mut::<Position>();
            pos.set(&f, i, Position { x: 1.0, y: 2.0 });
        }
    }
    assert!(world.verify_invariants());
    let sys = NoopSystem::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();
    world.run();
}

#[test]
fn clear_changed_masks_resets_masks() {
    register_components_once();
    let mut world = World::new();
    let f = Frame::new(world.current_tick());
    let s = world.get_storage_mut::<Position>();
    s.set(&f, 10, Position { x: 0.0, y: 0.0 });
    s.clear_changed_masks();
    assert!(world.verify_invariants());
    let sys = NoopSystem::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();
    world.run();
}

#[test]
fn removing_all_values_drops_page_and_chunk() {
    register_components_once();
    let mut world = World::new();
    let f = Frame::new(world.current_tick());
    let s = world.get_storage_mut::<Position>();
    s.set(&f, 64, Position { x: 0.0, y: 0.0 });
    assert!(s.remove(&f, 64));
    assert!(world.verify_invariants());
    let sys = NoopSystem::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();
    world.run();
}
