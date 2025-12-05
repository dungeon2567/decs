use decs::ecs::Ecs;
use decs::frame::Frame;
use decs::system; // for `system!`
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

static INIT: Once = Once::new();
fn register_components_once() {
    INIT.call_once(|| {
        Ecs::register::<Position>();
        Ecs::register::<Velocity>();
    });
}

system!(MutatePosOnVel {
    query fn update(pos: &mut ViewMut<Position>, _vel: View<Velocity>) {
        pos.x += 1.0;
        pos.y += 2.0;
    }
});

system!(NoopSystem { query fn update(_pos: View<Position>) { let _ = _pos.x; } });

#[test]
fn viewmut_marks_changed_and_updates_values() {
    register_components_once();
    let mut world = World::new();
    {
        let f = Frame::new(world.current_tick());
        for i in 0..128u32 {
            let pos = world.get_storage_mut::<Position>();
            pos.set(&f, i, Position { x: 0.0, y: 0.0 });
            if i % 2 == 0 {
                let vel = world.get_storage_mut::<Velocity>();
                vel.set(&f, i, Velocity { x: 1.0, y: 1.0 });
            }
        }
    }
    let sys = MutatePosOnVel::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();
    world.run();
    assert!(world.verify_invariants());

    let pos_ptr = world.get_storage::<Position>();
    for i in 0..128u32 {
        let p = unsafe { (*pos_ptr).get(i).unwrap() };
        if i % 2 == 0 {
            assert_eq!((p.x, p.y), (1.0, 2.0));
        } else {
            assert_eq!((p.x, p.y), (0.0, 0.0));
        }
    }
}

#[test]
fn viewmut_first_change_stores_old_value_once() {
    register_components_once();
    let mut world = World::new();
    world.set_tick(decs::tick::Tick(70));
    {
        let f = Frame::new(world.current_tick());
        let pos = world.get_storage_mut::<Position>();
        pos.set(&f, 10, Position { x: 5.0, y: 6.0 });
        let vel = world.get_storage_mut::<Velocity>();
        vel.set(&f, 10, Velocity { x: 1.0, y: 1.0 });
    }

    let sys = MutatePosOnVel::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();
    world.run();

    // Mutate again in same tick via system re-run
    let sys2 = MutatePosOnVel::new(&mut world);
    world.scheduler_mut().add_system(sys2);
    world.scheduler_mut().build_wavefronts();
    world.run();

    world.rollback(decs::tick::Tick(70));
    let pos_ptr = world.get_storage::<Position>();
    let p = unsafe { (*pos_ptr).get(10).unwrap() };
    assert_eq!((p.x, p.y), (5.0, 6.0));

    let sys3 = NoopSystem::new(&mut world);
    world.scheduler_mut().add_system(sys3);
    world.scheduler_mut().build_wavefronts();
    world.run();
}

#[test]
fn viewmut_does_not_change_presence_or_fullness_masks() {
    register_components_once();
    let mut world = World::new();
    {
        let f = Frame::new(world.current_tick());
        for i in 0..64u32 {
            let pos = world.get_storage_mut::<Position>();
            pos.set(&f, i, Position { x: 1.0, y: 2.0 });
        }
        let vel = world.get_storage_mut::<Velocity>();
        vel.set(&f, 0, Velocity { x: 1.0, y: 1.0 });
    }
    let sys = MutatePosOnVel::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();
    world.run();

    assert!(world.verify_invariants());

    let sys2 = NoopSystem::new(&mut world);
    world.scheduler_mut().add_system(sys2);
    world.scheduler_mut().build_wavefronts();
    world.run();
}

#[test]
fn view_and_viewmut_combination_multiple_chunks() {
    register_components_once();
    let mut world = World::new();
    {
        let f = Frame::new(world.current_tick());
        for i in [0, 63, 64, 127, 128, 255].iter().copied() {
            let pos = world.get_storage_mut::<Position>();
            pos.set(
                &f,
                i,
                Position {
                    x: i as f32,
                    y: i as f32,
                },
            );
            let vel = world.get_storage_mut::<Velocity>();
            vel.set(&f, i, Velocity { x: 0.5, y: 1.5 });
        }
    }
    let sys = MutatePosOnVel::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();
    world.run();
    assert!(world.verify_invariants());

    let pos_ptr = world.get_storage::<Position>();
    for i in [0, 63, 64, 127, 128, 255].iter().copied() {
        let p = unsafe { (*pos_ptr).get(i).unwrap() };
        assert_eq!((p.x, p.y), (i as f32 + 1.0, i as f32 + 2.0));
    }

    let sys2 = NoopSystem::new(&mut world);
    world.scheduler_mut().add_system(sys2);
    world.scheduler_mut().build_wavefronts();
    world.run();
}
