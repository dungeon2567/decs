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

#[derive(Clone, Debug, PartialEq, Component)]
struct TestC {
    v: i32,
}

static INIT: Once = Once::new();
fn register_components_once() {
    INIT.call_once(|| {
        Ecs::register::<Position>();
        Ecs::register::<TestC>();
    });
}

system!(NoopSystem { query fn update(_pos: View<Position>) { let _ = _pos.x; } });

#[test]
fn rollback_restore_modified_value() {
    register_components_once();
    let mut world = World::new();

    world.set_tick(decs::tick::Tick(1));
    {
        let f = Frame::new(world.current_tick());
        let s = world.get_storage_mut::<TestC>();
        s.set(&f, 100, TestC { v: 1 });
    }
    world.set_tick(decs::tick::Tick(2));
    {
        let f = Frame::new(world.current_tick());
        let s = world.get_storage_mut::<TestC>();
        s.set(&f, 100, TestC { v: 2 });
    }
    world.rollback(decs::tick::Tick(1));
    let sp = world.get_storage::<TestC>();
    let v = unsafe { (*sp).get(100).unwrap().v };
    assert_eq!(v, 1);

    let sys = NoopSystem::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();
    world.run();
}

#[test]
fn rollback_remove_created_item() {
    register_components_once();
    let mut world = World::new();
    world.set_tick(decs::tick::Tick(5));
    {
        let f = Frame::new(world.current_tick());
        let s = world.get_storage_mut::<TestC>();
        s.set(&f, 5000, TestC { v: 7 });
    }
    world.rollback(decs::tick::Tick(4));
    let sp = world.get_storage::<TestC>();
    assert!(unsafe { (*sp).get(5000).is_none() });

    let sys = NoopSystem::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();
    world.run();
}

#[test]
fn rollback_restore_removed_value() {
    register_components_once();
    let mut world = World::new();
    world.set_tick(decs::tick::Tick(10));
    {
        let f = Frame::new(world.current_tick());
        let s = world.get_storage_mut::<TestC>();
        s.set(&f, 777, TestC { v: 3 });
    }
    world.set_tick(decs::tick::Tick(11));
    {
        let f = Frame::new(world.current_tick());
        let s = world.get_storage_mut::<TestC>();
        assert!(s.remove(&f, 777));
    }
    world.rollback(decs::tick::Tick(10));
    let sp = world.get_storage::<TestC>();
    let v = unsafe { (*sp).get(777).unwrap().v };
    assert_eq!(v, 3);

    let sys = NoopSystem::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();
    world.run();
}

#[test]
fn rollback_multiple_indices_mixed_changes() {
    register_components_once();
    let mut world = World::new();

    world.set_tick(decs::tick::Tick(21));
    {
        let f = Frame::new(world.current_tick());
        let s = world.get_storage_mut::<TestC>();
        for &i in &[0u32, 63, 64, 4096, 4160] {
            s.set(&f, i, TestC { v: i as i32 });
        }
    }
    world.set_tick(decs::tick::Tick(22));
    {
        let f = Frame::new(world.current_tick());
        let s = world.get_storage_mut::<TestC>();
        s.set(&f, 64, TestC { v: -9 });
        assert!(s.remove(&f, 4096));
    }
    world.rollback(decs::tick::Tick(21));
    let sp = world.get_storage::<TestC>();
    assert_eq!(unsafe { (*sp).get(64).unwrap().v }, 64);
    assert!(unsafe { (*sp).get(4096).is_some() });

    let sys = NoopSystem::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();
    world.run();
}

#[test]
fn rollback_with_no_changes_keeps_state() {
    register_components_once();
    let mut world = World::new();
    world.set_tick(decs::tick::Tick(1));
    let f = Frame::new(world.current_tick());
    let s = world.get_storage_mut::<TestC>();
    s.set(&f, 123, TestC { v: 4 });
    world.rollback(decs::tick::Tick(0));
    let sp = world.get_storage::<TestC>();
    assert!(unsafe { (*sp).get(123).is_none() });

    let sys = NoopSystem::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();
    world.run();
}

#[test]
fn rollback_across_generation_restores_entity_generation() {
    register_components_once();
    let mut world = World::new();
    // touch entity storage and spawn a few
    let ent = world.get_entity_storage();
    {
        let f = Frame::new(world.current_tick());
        unsafe { &mut *ent }.save_generation_for_rollback();
        let _ = unsafe { &mut *ent }.spawn(&f);
        let _ = unsafe { &mut *ent }.spawn(&f);
    }
    world.rollback(decs::tick::Tick(0));
    assert!(world.verify_invariants());

    let f = Frame::new(world.current_tick());
    let pos = world.get_storage_mut::<Position>();
    pos.set(&f, 0, Position { x: 0.0, y: 0.0 });
    let sys = NoopSystem::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();
    world.run();
}

#[test]
fn rollback_boundary_indices_created_changed_removed() {
    register_components_once();
    let mut world = World::new();
    world.set_tick(decs::tick::Tick(50));
    {
        let f = Frame::new(world.current_tick());
        let s = world.get_storage_mut::<TestC>();
        for &i in &[63u32, 64u32, 4095u32, 4096u32, 4097u32] {
            s.set(&f, i, TestC { v: 1 });
        }
    }
    world.set_tick(decs::tick::Tick(51));
    {
        let f = Frame::new(world.current_tick());
        let s = world.get_storage_mut::<TestC>();
        s.set(&f, 63, TestC { v: 2 });
        assert!(s.remove(&f, 4096));
    }
    world.rollback(decs::tick::Tick(50));
    let sp = world.get_storage::<TestC>();
    assert_eq!(unsafe { (*sp).get(63).unwrap().v }, 1);
    assert!(unsafe { (*sp).get(4096).is_some() });

    let sys = NoopSystem::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();
    world.run();
}
