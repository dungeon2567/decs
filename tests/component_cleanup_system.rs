use decs::component::Destroyed;
use decs::ecs::Ecs;
use decs::frame::Frame;
use decs::system::ComponentCleanupSystem;
use decs::system::System;
use decs::tick::Tick;
use decs::world::World;
use decs_macros::Component;
use std::sync::Once;
fn register_components_once() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        Ecs::register::<CompT>();
        Ecs::register::<Destroyed>();
    });
}

#[allow(dead_code)]
#[derive(Clone, Component)]
struct CompT {
    v: u32,
}

#[test]
fn component_cleanup_removes_t_where_destroyed_and_clears_masks() {
    register_components_once();
    let mut world = World::new();
    let _ = world.get_storage::<CompT>();
    let _ = world.get_storage::<Destroyed>();

    {
        let frame = Frame::new(world.current_tick());
        let t_storage = unsafe { &mut *world.get_storage::<CompT>() };
        let d_storage = unsafe { &mut *world.get_storage::<Destroyed>() };
        t_storage.set(&frame, 0, CompT { v: 10 });
        t_storage.set(&frame, 1, CompT { v: 20 });
        d_storage.set(&frame, 0, Destroyed());
    }

    {
        let frame = Frame::new(Tick(1));
        let sys = ComponentCleanupSystem::<CompT>::from_storages(
            unsafe { &mut *world.get_storage::<CompT>() },
            unsafe { &*world.get_storage::<Destroyed>() },
        );
        sys.run(&frame);
    }

    {
        let t_storage = unsafe { &*world.get_storage::<CompT>() };
        assert!(t_storage.get(0).is_none());
        assert!(t_storage.get(1).is_some());
        assert!(t_storage.rollback.verify_was_removed(0));
        assert_eq!(t_storage.changed_mask, 0);
    }

    {
        let d_storage = unsafe { &mut *world.get_storage::<Destroyed>() };
        d_storage.clear_changed_masks();
    }

    assert!(world.verify_invariants());
}
