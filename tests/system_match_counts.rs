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

#[derive(Clone, Debug, PartialEq, Component)]
struct Frozen;

static INIT: Once = Once::new();
fn register_components_once() {
    INIT.call_once(|| {
        Ecs::register::<Position>();
        Ecs::register::<Velocity>();
        Ecs::register::<Frozen>();
    });
}

fn count_none_frozen_with_vel(world: &mut World) -> u32 {
    unsafe {
        let pos = &*world.get_storage::<Position>();
        let vel = &*world.get_storage::<Velocity>();
        let frz = &*world.get_storage::<Frozen>();
        let mut total: u32 = 0;
        let mut storage_mask = pos.presence_mask & vel.presence_mask;
        while storage_mask != 0 {
            let s_start = storage_mask.trailing_zeros() as usize;
            let s_shift = storage_mask >> s_start;
            let s_len = s_shift.trailing_ones() as usize;
            for storage_idx in s_start..s_start + s_len {
                let pos_page = &*pos.data[storage_idx];
                let vel_page = &*vel.data[storage_idx];
                let frz_page = &*frz.data[storage_idx];
                let mut page_mask = pos_page.presence_mask & vel_page.presence_mask;
                while page_mask != 0 {
                    let p_start = page_mask.trailing_zeros() as usize;
                    let p_shift = page_mask >> p_start;
                    let p_len = p_shift.trailing_ones() as usize;
                    for page_idx in p_start..p_start + p_len {
                        let pos_chunk = &*pos_page.data[page_idx];
                        let vel_chunk = &*vel_page.data[page_idx];
                        let frz_chunk = &*frz_page.data[page_idx];
                        let item_mask = (pos_chunk.presence_mask & vel_chunk.presence_mask)
                            & !frz_chunk.presence_mask;
                        total = total.saturating_add(item_mask.count_ones());
                    }
                    page_mask &= !((u64::MAX >> (64 - p_len)) << p_start);
                }
            }
            storage_mask &= !((u64::MAX >> (64 - s_len)) << s_start);
        }
        total
    }
}

fn count_changed_position(world: &mut World) -> u32 {
    unsafe {
        let pos = &*world.get_storage::<Position>();
        let rb = &*pos.rollback;
        let mut total: u32 = 0;
        let mut storage_mask = rb.changed_mask;
        while storage_mask != 0 {
            let s_start = storage_mask.trailing_zeros() as usize;
            let s_shift = storage_mask >> s_start;
            let s_len = s_shift.trailing_ones() as usize;
            for storage_idx in s_start..s_start + s_len {
                if let Some(page) = rb.get_page(storage_idx as u32) {
                    let mut page_mask = page.changed_mask;
                    while page_mask != 0 {
                        let p_start = page_mask.trailing_zeros() as usize;
                        let p_shift = page_mask >> p_start;
                        let p_len = p_shift.trailing_ones() as usize;
                        for page_idx in p_start..p_start + p_len {
                            if let Some(chunk) = page.get(page_idx as u32) {
                                total = total.saturating_add(chunk.changed_mask.count_ones());
                            }
                        }
                        page_mask &= !((u64::MAX >> (64 - p_len)) << p_start);
                    }
                }
            }
            storage_mask &= !((u64::MAX >> (64 - s_len)) << s_start);
        }
        total
    }
}

fn count_storage_changed_position(world: &mut World) -> u32 {
    unsafe {
        let pos = &*world.get_storage::<Position>();
        let mut total: u32 = 0;
        let mut storage_mask = pos.changed_mask & pos.presence_mask;
        while storage_mask != 0 {
            let s_start = storage_mask.trailing_zeros() as usize;
            let s_shift = storage_mask >> s_start;
            let s_len = s_shift.trailing_ones() as usize;
            for storage_idx in s_start..s_start + s_len {
                let page = &*pos.data[storage_idx];
                let mut page_mask = page.changed_mask & page.presence_mask;
                while page_mask != 0 {
                    let p_start = page_mask.trailing_zeros() as usize;
                    let p_shift = page_mask >> p_start;
                    let p_len = p_shift.trailing_ones() as usize;
                    for page_idx in p_start..p_start + p_len {
                        let chunk = &*page.data[page_idx];
                        total = total.saturating_add(chunk.changed_mask.count_ones());
                    }
                    page_mask &= !((u64::MAX >> (64 - p_len)) << p_start);
                }
            }
            storage_mask &= !((u64::MAX >> (64 - s_len)) << s_start);
        }
        total
    }
}

// Mutates positions only where Velocity exists to mark changes in current tick
system!(ViewMutChangeSystem {
    query fn update(pos: &mut ViewMut<Position>, _vel: View<Velocity>) {
        pos.x += 1.0;
    }
    Parent=[decs::world::SimulationGroup]
});

system!(MutateAllPos {
    query fn update(pos: &mut ViewMut<Position>) {
        pos.x += 1.0;
    }
    Parent=[decs::world::SimulationGroup]
});

fn count_all_frozen_with_pos(world: &mut World) -> u32 {
    unsafe {
        let pos = &*world.get_storage::<Position>();
        let frz = &*world.get_storage::<Frozen>();
        let mut total: u32 = 0;
        let mut storage_mask = pos.presence_mask & frz.presence_mask;
        while storage_mask != 0 {
            let s_start = storage_mask.trailing_zeros() as usize;
            let s_shift = storage_mask >> s_start;
            let s_len = s_shift.trailing_ones() as usize;
            for storage_idx in s_start..s_start + s_len {
                let pos_page = &*pos.data[storage_idx];
                let frz_page = &*frz.data[storage_idx];
                let mut page_mask = pos_page.presence_mask & frz_page.presence_mask;
                while page_mask != 0 {
                    let p_start = page_mask.trailing_zeros() as usize;
                    let p_shift = page_mask >> p_start;
                    let p_len = p_shift.trailing_ones() as usize;
                    for page_idx in p_start..p_start + p_len {
                        let pos_chunk = &*pos_page.data[page_idx];
                        let frz_chunk = &*frz_page.data[page_idx];
                        let item_mask = pos_chunk.presence_mask & frz_chunk.presence_mask;
                        total = total.saturating_add(item_mask.count_ones());
                    }
                    page_mask &= !((u64::MAX >> (64 - p_len)) << p_start);
                }
            }
            storage_mask &= !((u64::MAX >> (64 - s_len)) << s_start);
        }
        total
    }
}

#[test]
fn none_query_counts_matches() {
    register_components_once();
    let mut world = World::new();

    {
        let frame = Frame::new(world.current_tick());
        for i in 0..10_000u32 {
            {
                let pos = world.get_storage_mut::<Position>();
                pos.set(&frame, i, Position { x: 0.0, y: 0.0 });
            }
            {
                let vel = world.get_storage_mut::<Velocity>();
                vel.set(&frame, i, Velocity { x: 1.0, y: 2.0 });
            }
            if i >= 500 {
                let frz = world.get_storage_mut::<Frozen>();
                frz.set(&frame, i, Frozen);
            }
        }
    }

    let count = count_none_frozen_with_vel(&mut world);
    assert_eq!(count, 500);
    assert!(world.verify_invariants());
}

#[test]
fn changed_query_counts_modified_positions() {
    register_components_once();
    let mut world = World::new();

    // Tick 1: seed 1000 positions and tag every 20th with Velocity
    world.set_tick(decs::tick::Tick(1));
    {
        let frame = Frame::new(world.current_tick());
        for i in 0..1000u32 {
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
            if i % 20 == 0 {
                // 50 items will be eligible for mutation
                let vel = world.get_storage_mut::<Velocity>();
                vel.set(&frame, i, Velocity { x: 1.0, y: 0.0 });
            }
        }
    }

    // Systems: mutate eligible items via ViewMut, then count Changed=[Position]
    let change_sys = ViewMutChangeSystem::new(&mut world);
    world.scheduler_mut().add_system(change_sys);
    world.scheduler_mut().build_wavefronts();
    world.run();
    let count = count_changed_position(&mut world);
    assert_eq!(count, 50);
    assert!(world.verify_invariants());
}

#[test]
fn all_query_counts_items_with_frozen_present() {
    register_components_once();
    let mut world = World::new();

    {
        let frame = Frame::new(world.current_tick());
        for i in 0..2000u32 {
            {
                let pos = world.get_storage_mut::<Position>();
                pos.set(&frame, i, Position { x: 0.0, y: 0.0 });
            }
            if i % 4 == 0 {
                // 500 items
                let frz = world.get_storage_mut::<Frozen>();
                frz.set(&frame, i, Frozen);
            }
        }
    }

    let count = count_all_frozen_with_pos(&mut world);
    assert_eq!(count, 500);
    assert!(world.verify_invariants());
}

#[test]
fn viewmut_counts_items_and_mutates_values() {
    register_components_once();

    let mut world = World::new();

    {
        let frame = Frame::new(world.current_tick());

        for i in 0..300u32 {
            let pos = world.get_storage_mut::<Position>();
            pos.set(&frame, i, Position { x: 1.0, y: 0.0 });
        }
    }

    let sys = MutateAllPos::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();
    world.run();

    let count = count_storage_changed_position(&mut world);
    assert_eq!(count, 0);

    assert!(world.verify_invariants());

    let pos = unsafe { &*world.get_storage::<Position>() };

    for i in 0..300u32 {
        let p = pos.get(i).unwrap();
        assert_eq!(p.x, 2.0);
    }
}
