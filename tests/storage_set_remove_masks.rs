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
struct TestC { v: i32 }

static INIT: Once = Once::new();
fn register_components_once() {
    INIT.call_once(|| {
        Ecs::register::<Position>();
        Ecs::register::<TestC>();
    });
}

system!(NoopSystem { query fn update(_pos: View<Position>) { let _ = _pos.x; } });

#[test]
fn set_creates_page_chunk_and_marks_masks() {
    register_components_once();
    let mut world = World::new();
    let frame = Frame::new(world.current_tick());
    let s = world.get_storage_mut::<TestC>();
    s.set(&frame, 64, TestC { v: 1 });
    assert!(world.verify_invariants());

    let sys = NoopSystem::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();
    world.run();
    assert!(world.verify_invariants());
}

#[test]
fn remove_nonexistent_returns_false() {
    register_components_once();
    let mut world = World::new();
    let frame = Frame::new(world.current_tick());
    let s = world.get_storage_mut::<TestC>();
    assert!(!s.remove(&frame, 12345));

    let sys = NoopSystem::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();
    world.run();
    assert!(world.verify_invariants());
}

#[test]
fn remove_out_of_range_returns_false() {
    register_components_once();
    let mut world = World::new();
    let frame = Frame::new(world.current_tick());
    let s = world.get_storage_mut::<TestC>();
    assert!(!s.remove(&frame, 64 * 64 * 64));

    let sys = NoopSystem::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();
    world.run();
}

#[test]
fn remove_drops_chunk_when_empty() {
    register_components_once();
    let mut world = World::new();
    let frame = Frame::new(world.current_tick());
    let s = world.get_storage_mut::<TestC>();
    s.set(&frame, 65, TestC { v: 7 });
    assert!(s.remove(&frame, 65));
    assert!(world.verify_invariants());

    let sys = NoopSystem::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();
    world.run();
}

#[test]
fn remove_drops_page_when_empty() {
    register_components_once();
    let mut world = World::new();
    let frame = Frame::new(world.current_tick());
    let s = world.get_storage_mut::<TestC>();
    s.set(&frame, 64, TestC { v: 3 });
    assert!(s.remove(&frame, 64));
    assert!(world.verify_invariants());

    let sys = NoopSystem::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();
    world.run();
}

#[test]
fn set_then_remove_same_tick_idempotent() {
    register_components_once();
    let mut world = World::new();
    world.set_tick(decs::tick::Tick(10));
    let frame = Frame::new(world.current_tick());
    let s = world.get_storage_mut::<TestC>();
    s.set(&frame, 4097, TestC { v: 5 });
    assert!(s.remove(&frame, 4097));
    assert!(world.verify_invariants());

    world.rollback(decs::tick::Tick(9));
    assert!(world.verify_invariants());

    let sys = NoopSystem::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();
    world.run();
}

#[test]
fn set_modify_same_tick_keeps_created_mask_effect() {
    register_components_once();
    let mut world = World::new();
    world.set_tick(decs::tick::Tick(20));
    {
        let f = Frame::new(world.current_tick());
        let s = world.get_storage_mut::<TestC>();
        s.set(&f, 512, TestC { v: 1 });
        s.set(&f, 512, TestC { v: 2 });
    }
    assert!(world.verify_invariants());

    world.rollback(decs::tick::Tick(19));
    let sp = world.get_storage::<TestC>();
    assert!(unsafe { (*sp).get(512).is_none() });

    let sys = NoopSystem::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();
    world.run();
}

#[test]
fn set_after_remove_same_tick_marks_changed_effect() {
    register_components_once();
    let mut world = World::new();
    world.set_tick(decs::tick::Tick(30));
    {
        let f = Frame::new(world.current_tick());
        let s = world.get_storage_mut::<TestC>();
        s.set(&f, 777, TestC { v: 9 });
    }
    world.set_tick(decs::tick::Tick(31));
    {
        let f = Frame::new(world.current_tick());
        let s = world.get_storage_mut::<TestC>();
        assert!(s.remove(&f, 777));
        s.set(&f, 777, TestC { v: 11 });
    }
    world.rollback(decs::tick::Tick(30));
    let sp = world.get_storage::<TestC>();
    let v = unsafe { (*sp).get(777).unwrap().v };
    assert_eq!(v, 9);

    let sys = NoopSystem::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();
    world.run();
}

#[test]
fn get_out_of_bounds_none() {
    register_components_once();
    let mut world = World::new();
    let sp = world.get_storage::<TestC>();
    assert!(unsafe { (*sp).get(64 * 64 * 64).is_none() });

    let f = Frame::new(world.current_tick());
    let pos = world.get_storage_mut::<Position>();
    pos.set(&f, 0, Position { x: 1.0, y: 1.0 });
    let sys = NoopSystem::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();
    world.run();
}

#[test]
fn presence_fullness_invariants_after_many_sets_and_removes() {
    register_components_once();
    let mut world = World::new();
    {
        let f = Frame::new(world.current_tick());
        let s = world.get_storage_mut::<TestC>();
        for i in [0, 63, 64, 127, 4096, 4160, 8191].iter().copied() {
            s.set(&f, i, TestC { v: i as i32 });
        }
        assert!(s.remove(&f, 63));
        assert!(s.remove(&f, 4160));
    }
    assert!(world.verify_invariants());

    let sys = NoopSystem::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();
    world.run();
}

