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

fn count_none_frozen(world: &mut World) -> u32 {
    unsafe {
        let pos = &*world.get_storage::<Position>();
        let frz = &*world.get_storage::<Frozen>();
        let mut total: u32 = 0;
        let mut storage_mask = pos.presence_mask;
        while storage_mask != 0 {
            let s_start = storage_mask.trailing_zeros() as usize;
            let s_shift = storage_mask >> s_start;
            let s_len = s_shift.trailing_ones() as usize;
            for storage_idx in s_start..s_start + s_len {
                let pos_page = &*pos.data[storage_idx];
                let frz_page = &*frz.data[storage_idx];
                let mut page_mask = pos_page.presence_mask;
                while page_mask != 0 {
                    let p_start = page_mask.trailing_zeros() as usize;
                    let p_shift = page_mask >> p_start;
                    let p_len = p_shift.trailing_ones() as usize;
                    for page_idx in p_start..p_start + p_len {
                        let pos_chunk = &*pos_page.data[page_idx];
                        let frz_chunk = &*frz_page.data[page_idx];
                        let item_mask = pos_chunk.presence_mask & !frz_chunk.presence_mask;
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

fn count_all_velocity(world: &mut World) -> u32 {
    unsafe {
        let pos = &*world.get_storage::<Position>();
        let vel = &*world.get_storage::<Velocity>();
        let mut total: u32 = 0;
        let mut storage_mask = pos.presence_mask & vel.presence_mask;
        while storage_mask != 0 {
            let s_start = storage_mask.trailing_zeros() as usize;
            let s_shift = storage_mask >> s_start;
            let s_len = s_shift.trailing_ones() as usize;
            for storage_idx in s_start..s_start + s_len {
                let pos_page = &*pos.data[storage_idx];
                let vel_page = &*vel.data[storage_idx];
                let mut page_mask = pos_page.presence_mask & vel_page.presence_mask;
                while page_mask != 0 {
                    let p_start = page_mask.trailing_zeros() as usize;
                    let p_shift = page_mask >> p_start;
                    let p_len = p_shift.trailing_ones() as usize;
                    for page_idx in p_start..p_start + p_len {
                        let pos_chunk = &*pos_page.data[page_idx];
                        let vel_chunk = &*vel_page.data[page_idx];
                        let item_mask = pos_chunk.presence_mask & vel_chunk.presence_mask;
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

system!(MutPosOnVel { query fn update(pos: &mut ViewMut<Position>, _vel: View<Velocity>) { pos.x += 1.0; } });

#[test]
fn none_filter_counts_without_frozen() {
    register_components_once();
    let mut world = World::new();
    {
        let f = Frame::new(world.current_tick());
        for i in 0..1000u32 {
            let pos = world.get_storage_mut::<Position>();
            pos.set(&f, i, Position { x: 0.0, y: 0.0 });
            if i % 5 == 0 { let frz = world.get_storage_mut::<Frozen>(); frz.set(&f, i, Frozen); }
        }
    }
    let c = count_none_frozen(&mut world);
    assert_eq!(c, 800);
}

#[test]
fn all_filter_counts_with_velocity_present() {
    register_components_once();
    let mut world = World::new();
    {
        let f = Frame::new(world.current_tick());
        for i in 0..1200u32 {
            let pos = world.get_storage_mut::<Position>();
            pos.set(&f, i, Position { x: 0.0, y: 0.0 });
            if i % 3 == 0 { let vel = world.get_storage_mut::<Velocity>(); vel.set(&f, i, Velocity { x: 1.0, y: 2.0 }); }
        }
    }
    let c = count_all_velocity(&mut world);
    assert_eq!(c, 400);
}

#[test]
fn changed_filter_counts_only_mutated_positions() {
    register_components_once();
    let mut world = World::new();
    world.set_tick(decs::tick::Tick(1));
    {
        let f = Frame::new(world.current_tick());
        for i in 0..900u32 {
            let pos = world.get_storage_mut::<Position>();
            pos.set(&f, i, Position { x: i as f32, y: 0.0 });
            if i % 4 == 0 { let vel = world.get_storage_mut::<Velocity>(); vel.set(&f, i, Velocity { x: 1.0, y: 1.0 }); }
        }
    }
    let m = MutPosOnVel::new(&mut world);
    world.scheduler_mut().add_system(m);
    world.scheduler_mut().build_wavefronts();
    world.run();
    let c = count_changed_position(&mut world);
    assert_eq!(c, 225);
}

#[test]
fn none_all_combination() {
    register_components_once();
    let mut world = World::new();
    {
        let f = Frame::new(world.current_tick());
        for i in 0..256u32 {
            let pos = world.get_storage_mut::<Position>();
            pos.set(&f, i, Position { x: 0.0, y: 0.0 });
            if i % 2 == 0 { let vel = world.get_storage_mut::<Velocity>(); vel.set(&f, i, Velocity { x: 1.0, y: 2.0 }); }
            if i % 3 == 0 { let frz = world.get_storage_mut::<Frozen>(); frz.set(&f, i, Frozen); }
        }
    }
    let c_none = count_none_frozen(&mut world);
    let c_all = count_all_velocity(&mut world);
    assert_eq!(c_none, 170);
    assert_eq!(c_all, 128);
}

#[test]
fn changed_after_multiple_viewmut_runs_counts_once_per_entity() {
    register_components_once();
    let mut world = World::new();
    world.set_tick(decs::tick::Tick(2));
    {
        let f = Frame::new(world.current_tick());
        for i in 0..300u32 {
            let pos = world.get_storage_mut::<Position>();
            pos.set(&f, i, Position { x: 0.0, y: 0.0 });
            let vel = world.get_storage_mut::<Velocity>();
            vel.set(&f, i, Velocity { x: 1.0, y: 1.0 });
        }
    }
    let m1 = MutPosOnVel::new(&mut world);
    let m2 = MutPosOnVel::new(&mut world);
    world.scheduler_mut().add_system(m1);
    world.scheduler_mut().add_system(m2);
    world.scheduler_mut().build_wavefronts();
    world.run();
    let c = count_changed_position(&mut world);
    assert_eq!(c, 300);
}

#[test]
fn none_filter_with_no_data_counts_zero() {
    register_components_once();
    let mut world = World::new();
    let c = count_none_frozen(&mut world);
    assert_eq!(c, 0);
}

#[test]
fn all_filter_with_no_velocity_counts_zero() {
    register_components_once();
    let mut world = World::new();
    {
        let f = Frame::new(world.current_tick());
        for i in 0..100u32 {
            let pos = world.get_storage_mut::<Position>();
            pos.set(&f, i, Position { x: 0.0, y: 0.0 });
        }
    }
    let c = count_all_velocity(&mut world);
    assert_eq!(c, 0);
}

#[test]
fn changed_filter_without_mutation_counts_zero() {
    register_components_once();
    let mut world = World::new();
    {
        let f = Frame::new(world.current_tick());
        for i in 0..50u32 {
            let pos = world.get_storage_mut::<Position>();
            pos.set(&f, i, Position { x: 1.0, y: 1.0 });
        }
    }
    let c = count_changed_position(&mut world);
    assert_eq!(c, 0);
}

#[test]
fn changed_filter_after_remove_and_add_counts_change() {
    register_components_once();
    let mut world = World::new();
    world.set_tick(decs::tick::Tick(9));
    {
        let f = Frame::new(world.current_tick());
        let pos = world.get_storage_mut::<Position>();
        pos.set(&f, 5, Position { x: 1.0, y: 2.0 });
    }
    world.set_tick(decs::tick::Tick(10));
    {
        let f = Frame::new(world.current_tick());
        let pos = world.get_storage_mut::<Position>();
        assert!(pos.remove(&f, 5));
        pos.set(&f, 5, Position { x: 3.0, y: 4.0 });
    }
    world.scheduler_mut().build_wavefronts();
    world.run();
    let c = count_storage_changed_position(&mut world);
    assert_eq!(c, 0);
}
