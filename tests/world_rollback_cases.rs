use decs::ecs::Ecs;
use decs::frame::Frame;
use decs::world::World;
use decs_macros::Component;
use std::sync::Once;

#[derive(Clone, Debug, PartialEq, Component)]
struct TestC {
    v: i32,
}

fn register_components_once() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        Ecs::register::<TestC>();
    });
}

#[test]
fn rollback_add_modify_remove_across_boundaries() {
    register_components_once();
    let mut world = World::new();

    // t=1 add values across chunk/page/storage boundaries
    world.set_tick(decs::tick::Tick(1));
    {
        let frame = Frame::new(world.current_tick());
        for &i in &[0u32, 63, 64, 4096, 4160, 50000] {
            let s = world.get_storage_mut::<TestC>();
            s.set(&frame, i, TestC { v: i as i32 });
        }
    }
    assert!(world.verify_invariants());

    // t=2 modify some
    world.set_tick(decs::tick::Tick(2));
    {
        let frame = Frame::new(world.current_tick());
        for &i in &[0u32, 64u32, 50000u32] {
            let s = world.get_storage_mut::<TestC>();
            s.set(&frame, i, TestC { v: -1 });
        }
    }
    assert!(world.verify_invariants());

    // t=3 remove some
    world.set_tick(decs::tick::Tick(3));
    {
        let frame = Frame::new(world.current_tick());
        for &i in &[63u32, 4160u32] {
            let s = world.get_storage_mut::<TestC>();
            assert!(s.remove(&frame, i));
        }
    }
    assert!(world.verify_invariants());

    // rollback to t=2
    world.rollback(decs::tick::Tick(2));
    assert!(world.verify_invariants());
    {
        let s_ptr = world.get_storage::<TestC>();
        assert_eq!(unsafe { (*s_ptr).get(0).unwrap().v }, -1);
        assert_eq!(unsafe { (*s_ptr).get(64).unwrap().v }, -1);
        assert_eq!(unsafe { (*s_ptr).get(50000).unwrap().v }, -1);
        assert_eq!(unsafe { (*s_ptr).get(63).unwrap().v }, 63);
        assert_eq!(unsafe { (*s_ptr).get(4160).unwrap().v }, 4160);
    }

    // rollback to t=1
    world.rollback(decs::tick::Tick(1));
    assert!(world.verify_invariants());
    {
        let s_ptr = world.get_storage::<TestC>();
        assert_eq!(unsafe { (*s_ptr).get(0).unwrap().v }, 0);
        assert_eq!(unsafe { (*s_ptr).get(64).unwrap().v }, 64);
        assert_eq!(unsafe { (*s_ptr).get(50000).unwrap().v }, 50000);
        assert_eq!(unsafe { (*s_ptr).get(63).unwrap().v }, 63);
        assert_eq!(unsafe { (*s_ptr).get(4160).unwrap().v }, 4160);
    }
}

#[test]
fn rollback_idempotent_add_remove_same_tick_no_effect() {
    register_components_once();
    let mut world = World::new();

    // t=5 add then remove in same tick on multiple indices
    world.set_tick(decs::tick::Tick(5));
    {
        let frame = Frame::new(world.current_tick());
        for &i in &[1u32, 65u32, 4097u32] {
            let s = world.get_storage_mut::<TestC>();
            s.set(&frame, i, TestC { v: 7 });
            assert!(s.remove(&frame, i));
        }
    }
    assert!(world.verify_invariants());

    // Rollback to t=4 (no changes should be applied)
    world.rollback(decs::tick::Tick(4));
    assert!(world.verify_invariants());
    {
        let s_ptr = world.get_storage::<TestC>();
        assert!(unsafe { (*s_ptr).get(1).is_none() });
        assert!(unsafe { (*s_ptr).get(65).is_none() });
        assert!(unsafe { (*s_ptr).get(4097).is_none() });
    }
}
