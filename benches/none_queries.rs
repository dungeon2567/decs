use criterion::{Criterion, criterion_group, criterion_main};
// removed unused import
use decs::ecs::Ecs;
use decs::frame::Frame;
use decs::system;
use decs::system::System;
use decs::view::View;
use decs::world::World;
use decs_macros::Component;

#[allow(dead_code)]
#[derive(Clone, Copy, Component)]
struct Position {
    x: f32,
    y: f32,
}

static mut COUNT_MATCHED: usize = 0;

#[allow(dead_code)]
#[derive(Clone, Copy, Component)]
struct Velocity {
    x: f32,
    y: f32,
}

#[derive(Clone, Copy, Component)]
struct Frozen;

system!(FilteredMovement {
    query fn update(_vel: View<Velocity>) {
        unsafe { COUNT_MATCHED += 1; }
    }
    None=[Frozen]
});

system!(FilteredPosition {
    query fn update(_el: View<Velocity>) {
        unsafe { COUNT_MATCHED += 1; }
    }
    None=[Position]
});

fn bench_none_query(c: &mut Criterion) {
    Ecs::register::<Position>();
    Ecs::register::<Velocity>();
    Ecs::register::<Frozen>();
    let mut world = World::new();
    {
        let frame = Frame::new(world.current_tick());
        let pos_ptr = world.get_storage::<Position>();
        let vel_ptr = world.get_storage::<Velocity>();
        let frozen_ptr = world.get_storage::<Frozen>();
        let pos_storage = unsafe { &mut *pos_ptr };
        let vel_storage = unsafe { &mut *vel_ptr };
        let frozen_storage = unsafe { &mut *frozen_ptr };

        for i in 0..10_000u32 {
            if i >= 150 {
                pos_storage.set(&frame, i, Position { x: 0.0, y: 0.0 });
            }

            vel_storage.set(&frame, i, Velocity { x: 1.0, y: 2.0 });

            if i >= 500 {
                frozen_storage.set(&frame, i, Frozen);
            }
        }
    }
    let system = FilteredMovement::new(&mut world);

    c.bench_function("none_query_10k_skip_9500", |b| {
        b.iter(|| {
            unsafe {
                COUNT_MATCHED = 0;
            }

            let frame = Frame::new(world.current_tick());
            system.run(&frame);
        });
    });

    println!("DEBUG! none_query_10k_skip_9500 matched {:}", unsafe {
        COUNT_MATCHED
    });

    let system = FilteredPosition::new(&mut world);

    c.bench_function("none_query_10k_skip_9850", |b| {
        b.iter(|| {
            unsafe {
                COUNT_MATCHED = 0;
            }

            let frame = Frame::new(world.current_tick());
            system.run(&frame);
        });
    });

    println!("DEBUG! none_query_10k_skip_9850 matched {:}", unsafe {
        COUNT_MATCHED
    });
}

criterion_group!(benches, bench_none_query);
criterion_main!(benches);
