use decs::ecs::Ecs;
use decs::frame::Frame;
use decs::system;
use decs::system::System;
use decs::view::View;
use decs::world::World;
use decs_macros::Component;
use std::sync::Once;
fn register_components_once() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        Ecs::register::<Position>();
        Ecs::register::<Velocity>();
    });
}

#[allow(dead_code)]
#[derive(Clone, Copy, Component)]
struct Position {
    x: f32,
    y: f32,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Component)]
struct Velocity {
    x: f32,
    y: f32,
}

static mut COUNT_MATCHED: usize = 0;

system!(FilteredPosition {
    query fn update(_el: View<Velocity>) {
        unsafe { COUNT_MATCHED += 1; }
    }
    None=[Position]
});

#[test]
fn none_query_counts_velocity_without_position() {
    register_components_once();
    let mut world = World::new();
    {
        let frame = Frame::new(world.current_tick());
        let pos_ptr = world.get_storage::<Position>();
        let vel_ptr = world.get_storage::<Velocity>();
        let pos = unsafe { &mut *pos_ptr };
        let vel = unsafe { &mut *vel_ptr };

        for i in 0..1000u32 {
            vel.set(
                &frame,
                i,
                Velocity {
                    x: i as f32,
                    y: (i * 2) as f32,
                },
            );
        }
        for i in (0..1000u32).step_by(10) {
            pos.set(
                &frame,
                i,
                Position {
                    x: i as f32,
                    y: (i * 3) as f32,
                },
            );
        }
    }

    unsafe {
        COUNT_MATCHED = 0;
    }
    let system = FilteredPosition::new(&mut world);
    let frame = Frame::new(world.current_tick());
    system.run(&frame);

    let matched = unsafe { COUNT_MATCHED };
    assert_eq!(matched, 900);
}
