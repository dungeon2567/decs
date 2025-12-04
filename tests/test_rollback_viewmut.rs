#[cfg(test)]
mod tests {
    use decs::ecs::Ecs;
    use decs::view::ViewMut;
    use decs::world::World;
    use decs_macros::Component;
    use std::sync::Once;

    use decs::frame::Frame;
    use decs::system;
    use decs::tick::Tick;

    #[derive(Clone, Debug, PartialEq, Component)]
    struct Comp(i32);

    system!(TestSystem {
        query fn update(comp: &mut ViewMut<Comp>) {
            **comp = Comp(20);
        }
    });

    #[test]
    fn test_viewmut_rollback() {
        register_components_once();
        let mut world = World::new();
        let _ = world.get_storage::<Comp>();

        let mut scheduler = decs::scheduler::Scheduler::new();
        scheduler.add_system(TestSystem::new(&mut world));
        scheduler.build_wavefronts();

        // Tick 1: Spawn entity with Comp(10)
        world.set_tick(Tick(1));
        let entity = {
            let frame = Frame::new(world.current_tick());
            let entity_storage = unsafe { &mut *world.get_entity_storage() };
            entity_storage
                .spawn(&frame)
                .expect("Failed to spawn entity")
        };
        {
            let comp_storage = unsafe { &mut *world.get_storage::<Comp>() };
            let frame = Frame::new(world.current_tick());
            comp_storage.set(&frame, entity.index(), Comp(10));
        }

        // Verify initial state
        {
            let comp_storage = unsafe { &*world.get_storage::<Comp>() };
            assert_eq!(comp_storage.get(entity.index()), Some(&Comp(10)));
        }

        // Tick 2: Run system to change value to 20
        world.set_tick(Tick(2));
        {
            let frame = Frame::new(world.current_tick());
            scheduler.run(&frame);
        }

        // Verify change
        {
            let comp_storage = unsafe { &*world.get_storage::<Comp>() };
            assert_eq!(comp_storage.get(entity.index()), Some(&Comp(20)));
        }

        // Verify RollbackStorage has the old value
        // We can't easily access RollbackStorage directly from here without exposing internals,
        // but we can test the rollback functionality itself.

        // Rollback to Tick 1
        {
            let comp_storage = unsafe { &mut *world.get_storage::<Comp>() };
            comp_storage.rollback(Tick(1));
        }

        // Verify value is restored to 10
        {
            let comp_storage = unsafe { &*world.get_storage::<Comp>() };
            assert_eq!(
                comp_storage.get(entity.index()),
                Some(&Comp(10)),
                "Value should be restored to 10 after rollback"
            );
        }
    }

    #[test]
    fn test_viewmut_created_and_modified_same_tick_idempotent() {
        register_components_once();
        let mut world = World::new();
        let _ = world.get_storage::<Comp>();

        let mut scheduler = decs::scheduler::Scheduler::new();
        scheduler.add_system(TestSystem::new(&mut world));
        scheduler.build_wavefronts();

        // Tick N-1: nothing present
        world.set_tick(Tick(10));

        // Tick N: create and then modify via ViewMut in same tick
        world.set_tick(Tick(11));
        let entity = {
            let frame = Frame::new(world.current_tick());
            let entity_storage = unsafe { &mut *world.get_entity_storage() };
            entity_storage
                .spawn(&frame)
                .expect("Failed to spawn entity")
        };
        {
            let comp_storage = unsafe { &mut *world.get_storage::<Comp>() };
            let frame = Frame::new(world.current_tick());
            comp_storage.set(&frame, entity.index(), Comp(1));
        }
        // System modifies Comp(1) -> Comp(20) using ViewMut in same tick
        {
            let frame = Frame::new(world.current_tick());
            scheduler.run(&frame);
        }

        // Roll back to previous tick (10): Comp should be removed (created+modified => created only)
        {
            let comp_storage = unsafe { &mut *world.get_storage::<Comp>() };
            comp_storage.rollback(Tick(10));
        }
        {
            let comp_storage = unsafe { &*world.get_storage::<Comp>() };
            assert_eq!(
                comp_storage.get(entity.index()),
                None,
                "Comp should not exist after rollback to before creation"
            );
        }
    }
    fn register_components_once() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            Ecs::register::<Comp>();
        });
    }

    #[test]
    fn test_viewmut_modify_idx0_rollback() {
        register_components_once();
        let mut world = World::new();
        let _ = world.get_storage::<Comp>();

        let mut scheduler = decs::scheduler::Scheduler::new();
        scheduler.add_system(TestSystem::new(&mut world));
        scheduler.build_wavefronts();

        world.set_tick(Tick(1));
        {
            let s = unsafe { &mut *world.get_storage::<Comp>() };
            let frame = Frame::new(world.current_tick());
            s.set(&frame, 0, Comp(5));
        }

        world.set_tick(Tick(2));
        {
            let frame = Frame::new(world.current_tick());
            scheduler.run(&frame);
        }

        {
            let s = unsafe { &mut *world.get_storage::<Comp>() };
            s.rollback(Tick(1));
        }
        {
            let s = unsafe { &*world.get_storage::<Comp>() };
            assert_eq!(s.get(0), Some(&Comp(5)));
        }
    }

    #[test]
    fn test_viewmut_modify_idx63_rollback() {
        register_components_once();
        let mut world = World::new();
        let _ = world.get_storage::<Comp>();

        let mut scheduler = decs::scheduler::Scheduler::new();
        scheduler.add_system(TestSystem::new(&mut world));
        scheduler.build_wavefronts();

        world.set_tick(Tick(1));
        {
            let s = unsafe { &mut *world.get_storage::<Comp>() };
            let frame = Frame::new(world.current_tick());
            s.set(&frame, 63, Comp(7));
        }
        world.set_tick(Tick(2));
        {
            let frame = Frame::new(world.current_tick());
            scheduler.run(&frame);
        }
        {
            let s = unsafe { &mut *world.get_storage::<Comp>() };
            s.rollback(Tick(1));
        }
        {
            let s = unsafe { &*world.get_storage::<Comp>() };
            assert_eq!(s.get(63), Some(&Comp(7)));
        }
    }

    #[test]
    fn test_viewmut_modify_idx64_rollback() {
        register_components_once();
        let mut world = World::new();
        let _ = world.get_storage::<Comp>();

        let mut scheduler = decs::scheduler::Scheduler::new();
        scheduler.add_system(TestSystem::new(&mut world));
        scheduler.build_wavefronts();

        world.set_tick(Tick(5));
        {
            let s = unsafe { &mut *world.get_storage::<Comp>() };
            let frame = Frame::new(world.current_tick());
            s.set(&frame, 64, Comp(9));
        }
        world.set_tick(Tick(6));
        {
            let frame = Frame::new(world.current_tick());
            scheduler.run(&frame);
        }
        {
            let s = unsafe { &mut *world.get_storage::<Comp>() };
            s.rollback(Tick(5));
        }
        {
            let s = unsafe { &*world.get_storage::<Comp>() };
            assert_eq!(s.get(64), Some(&Comp(9)));
        }
    }

    #[test]
    fn test_viewmut_created_modified_same_tick_idx64() {
        register_components_once();
        let mut world = World::new();
        let _ = world.get_storage::<Comp>();

        let mut scheduler = decs::scheduler::Scheduler::new();
        scheduler.add_system(TestSystem::new(&mut world));
        scheduler.build_wavefronts();

        world.set_tick(Tick(10));
        world.set_tick(Tick(11));
        {
            let s = unsafe { &mut *world.get_storage::<Comp>() };
            let frame = Frame::new(world.current_tick());
            s.set(&frame, 64, Comp(1));
        }
        {
            let frame = Frame::new(world.current_tick());
            scheduler.run(&frame);
        }
        {
            let s = unsafe { &mut *world.get_storage::<Comp>() };
            s.rollback(Tick(10));
        }
        {
            let s = unsafe { &*world.get_storage::<Comp>() };
            assert_eq!(s.get(64), None);
        }
    }

    #[test]
    fn test_viewmut_multi_mod_same_tick_rollback() {
        register_components_once();
        let mut world = World::new();
        let _ = world.get_storage::<Comp>();

        let mut scheduler = decs::scheduler::Scheduler::new();
        scheduler.add_system(TestSystem::new(&mut world));
        scheduler.build_wavefronts();

        world.set_tick(Tick(19));
        {
            let s = unsafe { &mut *world.get_storage::<Comp>() };
            let frame = Frame::new(world.current_tick());
            s.set(&frame, 1, Comp(33));
        }
        // Two modifications in the next tick via system
        world.set_tick(Tick(20));
        {
            let frame = Frame::new(world.current_tick());
            scheduler.run(&frame);
            scheduler.run(&frame);
        }
        {
            let s = unsafe { &mut *world.get_storage::<Comp>() };
            s.rollback(Tick(19));
        }
        {
            let s = unsafe { &*world.get_storage::<Comp>() };
            assert_eq!(s.get(1), Some(&Comp(33)));
        }
    }

    #[test]
    fn test_viewmut_two_entities_same_chunk_rollback() {
        register_components_once();
        let mut world = World::new();
        let _ = world.get_storage::<Comp>();

        let mut scheduler = decs::scheduler::Scheduler::new();
        scheduler.add_system(TestSystem::new(&mut world));
        scheduler.build_wavefronts();

        world.set_tick(Tick(30));
        {
            let s = unsafe { &mut *world.get_storage::<Comp>() };
            let frame = Frame::new(world.current_tick());
            s.set(&frame, 2, Comp(100));
            s.set(&frame, 3, Comp(200));
        }
        world.set_tick(Tick(31));
        {
            let frame = Frame::new(world.current_tick());
            scheduler.run(&frame);
        }
        {
            let s = unsafe { &mut *world.get_storage::<Comp>() };
            s.rollback(Tick(30));
        }
        {
            let s = unsafe { &*world.get_storage::<Comp>() };
            assert_eq!(s.get(2), Some(&Comp(100)));
            assert_eq!(s.get(3), Some(&Comp(200)));
        }
    }

    #[test]
    fn test_viewmut_two_entities_across_page_rollback() {
        register_components_once();
        let mut world = World::new();
        let _ = world.get_storage::<Comp>();

        let mut scheduler = decs::scheduler::Scheduler::new();
        scheduler.add_system(TestSystem::new(&mut world));
        scheduler.build_wavefronts();

        world.set_tick(Tick(40));
        {
            let s = unsafe { &mut *world.get_storage::<Comp>() };
            let frame = Frame::new(world.current_tick());
            s.set(&frame, 63, Comp(8));
            s.set(&frame, 64, Comp(9));
        }
        world.set_tick(Tick(41));
        {
            let frame = Frame::new(world.current_tick());
            scheduler.run(&frame);
        }
        {
            let s = unsafe { &mut *world.get_storage::<Comp>() };
            s.rollback(Tick(40));
        }
        {
            let s = unsafe { &*world.get_storage::<Comp>() };
            assert_eq!(s.get(63), Some(&Comp(8)));
            assert_eq!(s.get(64), Some(&Comp(9)));
        }
    }

    #[test]
    fn test_viewmut_two_entities_across_storage_boundary_rollback() {
        register_components_once();
        let mut world = World::new();
        let _ = world.get_storage::<Comp>();

        let mut scheduler = decs::scheduler::Scheduler::new();
        scheduler.add_system(TestSystem::new(&mut world));
        scheduler.build_wavefronts();

        world.set_tick(Tick(50));
        {
            let s = unsafe { &mut *world.get_storage::<Comp>() };
            let frame = Frame::new(world.current_tick());
            s.set(&frame, 4095, Comp(77));
            s.set(&frame, 4096, Comp(88));
        }
        world.set_tick(Tick(51));
        {
            let frame = Frame::new(world.current_tick());
            scheduler.run(&frame);
        }
        {
            let s = unsafe { &mut *world.get_storage::<Comp>() };
            s.rollback(Tick(50));
        }
        {
            let s = unsafe { &*world.get_storage::<Comp>() };
            assert_eq!(s.get(4095), Some(&Comp(77)));
            assert_eq!(s.get(4096), Some(&Comp(88)));
        }
    }
}
