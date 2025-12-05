use decs::ecs::Ecs;
use decs::frame::Frame;
use decs::system; // for `system!`
use decs::view::View;
use decs::world::World;
use decs_macros::Component;
use std::sync::Once;

#[derive(Clone, Debug, PartialEq, Component)]
struct Position { x: f32, y: f32 }

#[derive(Clone, Debug, PartialEq, Component)]
struct Health { hp: i32 }

#[derive(Clone, Debug, PartialEq, Component)]
struct DestroyedTag;

static INIT: Once = Once::new();
fn register_components_once() {
    INIT.call_once(|| {
        Ecs::register::<Position>();
        Ecs::register::<Health>();
        // Built-in Destroyed component is already registered in World::new
    });
}

system!(NoopSystem { query fn update(_pos: View<Position>) { let _ = _pos.x; } });

#[test]
fn component_cleanup_removes_t_when_destroyed_present() {
    register_components_once();
    let mut world = World::new();
    {
        let f = Frame::new(world.current_tick());
        for i in [0, 1, 64, 65].iter().copied() {
            let pos = world.get_storage_mut::<Position>();
            pos.set(&f, i, Position { x: 0.0, y: 0.0 });
            let hp = world.get_storage_mut::<Health>();
            hp.set(&f, i, Health { hp: 100 });
            let d = world.get_storage_mut::<decs::component::Destroyed>();
            d.set(&f, i, decs::component::Destroyed {});
        }
    }
    world.scheduler_mut().build_wavefronts();
    world.run();
    assert!(world.verify_invariants());

    let hp_ptr = world.get_storage::<Health>();
    for i in [0, 1, 64, 65].iter().copied() {
        assert!(unsafe { (*hp_ptr).get(i).is_none() });
    }

    let sys = NoopSystem::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();
    world.run();
}

#[test]
fn temporary_cleanup_clears_destroyed_storage() {
    register_components_once();
    let mut world = World::new();
    {
        let f = Frame::new(world.current_tick());
        for i in [2, 3, 66, 67].iter().copied() {
            let d = world.get_storage_mut::<decs::component::Destroyed>();
            d.set(&f, i, decs::component::Destroyed {});
        }
    }
    world.scheduler_mut().build_wavefronts();
    world.run();
    assert!(world.verify_invariants());

    let d_ptr = world.get_storage::<decs::component::Destroyed>();
    for i in [2, 3, 66, 67].iter().copied() {
        assert!(unsafe { (*d_ptr).get(i).is_none() });
    }

    let sys = NoopSystem::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();
    world.run();
}

#[test]
fn cleanup_preserves_invariants_across_pages() {
    register_components_once();
    let mut world = World::new();
    {
        let f = Frame::new(world.current_tick());
        for i in [0, 63, 64, 4096, 4160].iter().copied() {
            let pos = world.get_storage_mut::<Position>();
            pos.set(&f, i, Position { x: i as f32, y: i as f32 });
            let hp = world.get_storage_mut::<Health>();
            hp.set(&f, i, Health { hp: 10 });
            let d = world.get_storage_mut::<decs::component::Destroyed>();
            d.set(&f, i, decs::component::Destroyed {});
        }
    }
    world.scheduler_mut().build_wavefronts();
    world.run();
    assert!(world.verify_invariants());

    let hp_ptr = world.get_storage::<Health>();
    for i in [0, 63, 64, 4096, 4160].iter().copied() {
        assert!(unsafe { (*hp_ptr).get(i).is_none() });
    }

    let sys = NoopSystem::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();
    world.run();
}

