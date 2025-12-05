use decs::ecs::Ecs;
use decs::frame::Frame;
use decs::system; // import macro namespace for `system!`
use decs::view::{View, ViewMut};
use decs::world::World;
use decs_macros::Component;
use std::sync::Once;

#[derive(Clone, Debug, PartialEq, Component)]
struct Position {
    x: f32,
    y: f32,
}

#[derive(Clone, Debug, PartialEq, Component)]
struct Velocity {
    x: f32,
    y: f32,
}

#[derive(Clone, Debug, PartialEq, Component)]
struct Frozen;

fn register_components_once() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        Ecs::register::<Position>();
        Ecs::register::<Velocity>();
        Ecs::register::<Frozen>();
    });
}

// Simple movement system using View/ViewMut
system!(MoveSystem {
    query fn update(pos: &mut ViewMut<Position>, vel: View<Velocity>) {
        pos.x += vel.x;
        pos.y += vel.y;
    }
});

// Movement system that respects None filter (skips Frozen)
system!(FilteredMoveSystem {
    query fn update(pos: &mut ViewMut<Position>, vel: View<Velocity>) {
        pos.x += vel.x;
        pos.y += vel.y;
    }
    None=[Frozen]
});

#[test]
fn world_invariants_with_sets_and_removes() {
    register_components_once();
    let mut world = World::new();

    {
        let frame = Frame::new(world.current_tick());
        for &i in &[0u32, 63, 64, 4096] {
            {
                let pos = world.get_storage_mut::<Position>();
                pos.set(
                    &frame,
                    i,
                    Position {
                        x: i as f32,
                        y: i as f32,
                    },
                );
            }
            {
                let vel = world.get_storage_mut::<Velocity>();
                vel.set(&frame, i, Velocity { x: 1.0, y: 2.0 });
            }
        }
    }

    assert!(world.verify_invariants());

    {
        let frame = Frame::new(world.current_tick());
        let pos = world.get_storage_mut::<Position>();
        assert!(pos.remove(&frame, 63));
        assert!(pos.remove(&frame, 4096));
    }
    assert!(world.verify_invariants());
}

#[test]
fn world_system_view_viewmut_integration_and_invariants() {
    register_components_once();
    let mut world = World::new();

    {
        let frame = Frame::new(world.current_tick());
        {
            let pos = world.get_storage_mut::<Position>();
            pos.set(&frame, 0, Position { x: 0.0, y: 0.0 });
        }
        {
            let vel = world.get_storage_mut::<Velocity>();
            vel.set(&frame, 0, Velocity { x: 1.0, y: 2.0 });
        }
        {
            let pos = world.get_storage_mut::<Position>();
            pos.set(&frame, 1, Position { x: 10.0, y: 20.0 });
        }
        {
            let vel = world.get_storage_mut::<Velocity>();
            vel.set(&frame, 1, Velocity { x: 5.0, y: 10.0 });
        }
    }

    let sys = MoveSystem::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();

    world.run();
    assert!(world.verify_invariants());

    let pos_ptr = world.get_storage::<Position>();
    let p0 = unsafe { (*pos_ptr).get(0).unwrap() };
    let p1 = unsafe { (*pos_ptr).get(1).unwrap() };
    assert_eq!((p0.x, p0.y), (1.0, 2.0));
    assert_eq!((p1.x, p1.y), (15.0, 30.0));
}

#[test]
fn world_system_with_none_filter_and_invariants() {
    register_components_once();
    let mut world = World::new();

    {
        let frame = Frame::new(world.current_tick());
        {
            let pos = world.get_storage_mut::<Position>();
            pos.set(&frame, 0, Position { x: 0.0, y: 0.0 });
        }
        {
            let vel = world.get_storage_mut::<Velocity>();
            vel.set(&frame, 0, Velocity { x: 1.0, y: 2.0 });
        }
        {
            let pos = world.get_storage_mut::<Position>();
            pos.set(&frame, 1, Position { x: 10.0, y: 20.0 });
        }
        {
            let vel = world.get_storage_mut::<Velocity>();
            vel.set(&frame, 1, Velocity { x: 5.0, y: 10.0 });
        }
        {
            let frz = world.get_storage_mut::<Frozen>();
            frz.set(&frame, 1, Frozen);
        }
    }

    let sys = FilteredMoveSystem::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();

    world.run();
    assert!(world.verify_invariants());

    let pos_ptr = world.get_storage::<Position>();
    let p0 = unsafe { (*pos_ptr).get(0).unwrap() };
    let p1 = unsafe { (*pos_ptr).get(1).unwrap() };
    assert_eq!((p0.x, p0.y), (1.0, 2.0));
    assert_eq!((p1.x, p1.y), (10.0, 20.0));
}

#[test]
fn world_rollback_over_many_ticks_manual() {
    register_components_once();
    let mut world = World::new();

    world.set_tick(decs::tick::Tick(1));
    {
        let f1 = Frame::new(world.current_tick());
        let pos = world.get_storage_mut::<Position>();
        pos.set(&f1, 5, Position { x: 1.0, y: 2.0 });
    }

    world.set_tick(decs::tick::Tick(2));
    {
        let f2 = Frame::new(world.current_tick());
        let pos = world.get_storage_mut::<Position>();
        pos.set(&f2, 5, Position { x: 3.0, y: 4.0 });
    }

    world.set_tick(decs::tick::Tick(3));
    {
        let f3 = Frame::new(world.current_tick());
        let pos = world.get_storage_mut::<Position>();
        assert!(pos.remove(&f3, 5));
    }
    assert!(world.verify_invariants());

    world.rollback(decs::tick::Tick(2));
    assert!(world.verify_invariants());
    let pos_ptr = world.get_storage::<Position>();
    let p = unsafe { (*pos_ptr).get(5).unwrap() };
    assert_eq!((p.x, p.y), (3.0, 4.0));

    world.rollback(decs::tick::Tick(1));
    assert!(world.verify_invariants());
    let p = unsafe { (*pos_ptr).get(5).unwrap() };
    assert_eq!((p.x, p.y), (1.0, 2.0));
}

#[test]
fn world_system_run_then_rollback() {
    register_components_once();
    let mut world = World::new();

    {
        let frame = Frame::new(world.current_tick());
        {
            let pos = world.get_storage_mut::<Position>();
            pos.set(&frame, 0, Position { x: 0.0, y: 0.0 });
        }
        {
            let vel = world.get_storage_mut::<Velocity>();
            vel.set(&frame, 0, Velocity { x: 1.0, y: 2.0 });
        }
    }

    let sys = MoveSystem::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();

    world.run();
    assert!(world.verify_invariants());

    world.run();
    assert!(world.verify_invariants());

    let pos_ptr = world.get_storage::<Position>();
    let p = unsafe { (*pos_ptr).get(0).unwrap() };
    assert_eq!((p.x, p.y), (2.0, 4.0));

    let target = decs::tick::Tick(world.current_tick().value() - 1);
    world.rollback(target);
    assert!(world.verify_invariants());

    let p = unsafe { (*pos_ptr).get(0).unwrap() };
    assert_eq!((p.x, p.y), (1.0, 2.0));
}
