#[cfg(test)]
mod tests {
    use decs::ecs::Ecs;
    use decs::frame::Frame;
    use decs::system;
    use decs::system::System;
    use decs::view::{View, ViewMut};
    use decs::world::World;
    use decs_macros::Component;
    use std::sync::Once;
    fn register_components_once() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            Ecs::register::<Position>();
            Ecs::register::<Velocity>();
            Ecs::register::<Frozen>();
        });
    }

    static mut COUNT_MATCHED: u32 = 0;

    #[derive(Debug, Clone, Copy, PartialEq, Component)]
    struct Position {
        x: f32,
        y: f32,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Component)]
    struct Velocity {
        x: f32,
        y: f32,
    }

    system!(MovementSystem {
        query fn update(pos: &mut ViewMut<Position>, vel: View<Velocity>) {
            pos.x += vel.x;
            pos.y += vel.y;
        }
    });

    #[derive(Debug, Clone, Copy, PartialEq, Component)]
    struct Frozen;

    system!(FilteredMovement {
        query fn update(pos: &mut ViewMut<Position>, vel: View<Velocity>) {
            pos.x += vel.x;
            pos.y += vel.y;
        }
        None=[Frozen]
    });

    system!(FilteredMovementCount {
        query fn update(_pos: &mut ViewMut<Position>, _vel: View<Velocity>) {
            unsafe { COUNT_MATCHED += 1; }
        }
        None=[Frozen]
    });

    #[test]
    fn test_system_macro_basic() {
        register_components_once();
        let mut world = World::new();

        // Create entities with Position and Velocity
        {
            let frame = Frame::new(world.current_tick());
            let pos_ptr = world.get_storage::<Position>();
            let vel_ptr = world.get_storage::<Velocity>();
            let pos_storage = unsafe { &mut *pos_ptr };
            let vel_storage = unsafe { &mut *vel_ptr };

            pos_storage.set(&frame, 0, Position { x: 0.0, y: 0.0 });
            vel_storage.set(&frame, 0, Velocity { x: 1.0, y: 2.0 });

            pos_storage.set(&frame, 1, Position { x: 10.0, y: 20.0 });
            vel_storage.set(&frame, 1, Velocity { x: 5.0, y: 10.0 });
        }

        // Create and run system
        let system = MovementSystem::new(&mut world);

        let frame = Frame::new(world.current_tick());
        system.run(&frame);

        // Verify results
        {
            let pos_storage = unsafe { &*world.get_storage::<Position>() };

            let pos0 = pos_storage.get(0).unwrap();

            assert_eq!(pos0.x, 1.0);
            assert_eq!(pos0.y, 2.0);

            let pos1 = pos_storage.get(1).unwrap();
            assert_eq!(pos1.x, 15.0);
            assert_eq!(pos1.y, 30.0);
        }
    }

    #[test]
    fn test_system_macro_none_filter() {
        register_components_once();
        let mut world = World::new();

        {
            let frame = Frame::new(world.current_tick());
            let pos_storage = world.get_storage::<Position>();
            let vel_storage = world.get_storage::<Velocity>();
            let pos_storage = unsafe { &mut *pos_storage };
            let vel_storage = unsafe { &mut *vel_storage };
            let frozen_storage = world.get_storage::<Frozen>();
            let frozen_storage = unsafe { &mut *frozen_storage };

            pos_storage.set(&frame, 0, Position { x: 0.0, y: 0.0 });
            vel_storage.set(&frame, 0, Velocity { x: 1.0, y: 2.0 });

            pos_storage.set(&frame, 1, Position { x: 10.0, y: 20.0 });
            vel_storage.set(&frame, 1, Velocity { x: 5.0, y: 10.0 });

            frozen_storage.set(&frame, 1, Frozen);
        }

        let system = FilteredMovement::new(&mut world);
        let frame = Frame::new(world.current_tick());
        system.run(&frame);

        {
            let pos_storage = unsafe { &*world.get_storage::<Position>() };
            let pos0 = pos_storage.get(0).unwrap();
            assert_eq!(pos0.x, 1.0);
            assert_eq!(pos0.y, 2.0);

            let pos1 = pos_storage.get(1).unwrap();
            assert_eq!(pos1.x, 10.0);
            assert_eq!(pos1.y, 20.0);
        }
    }

    #[test]
    fn none_test_10k_matches_500() {
        register_components_once();

        unsafe {
            COUNT_MATCHED = 0;
        }

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
                pos_storage.set(&frame, i, Position { x: 0.0, y: 0.0 });
                vel_storage.set(&frame, i, Velocity { x: 1.0, y: 2.0 });
                if i >= 500 {
                    frozen_storage.set(&frame, i, Frozen);
                }
            }
        }

        let system = FilteredMovementCount::new(&mut world);
        let frame = Frame::new(world.current_tick());

        system.run(&frame);

        let matched = unsafe { COUNT_MATCHED };
        assert_eq!(matched, 500);
    }

    #[test]
    fn viewmut_mask_propagation_basic() {
        register_components_once();
        let mut world = World::new();
        {
            let frame = Frame::new(world.current_tick());
            let pos_ptr = world.get_storage::<Position>();
            let vel_ptr = world.get_storage::<Velocity>();
            let pos_storage = unsafe { &mut *pos_ptr };
            let vel_storage = unsafe { &mut *vel_ptr };
            pos_storage.set(&frame, 0, Position { x: 0.0, y: 0.0 });
            vel_storage.set(&frame, 0, Velocity { x: 1.0, y: 2.0 });
            pos_storage.set(&frame, 1, Position { x: 10.0, y: 20.0 });
            vel_storage.set(&frame, 1, Velocity { x: 5.0, y: 10.0 });
            pos_storage.clear_changed_masks();
            vel_storage.clear_changed_masks();
        }
        let system = MovementSystem::new(&mut world);
        let frame = Frame::new(world.current_tick());
        system.run(&frame);
        {
            let pos_storage = unsafe { &*world.get_storage::<Position>() };
            let vel_storage = unsafe { &*world.get_storage::<Velocity>() };
            assert_eq!(pos_storage.changed_mask & 1, 1);
            unsafe {
                let page = &*pos_storage.data[0];
                assert_eq!(page.changed_mask & 1, 1);
                let chunk = &*page.data[0];
                assert_eq!(chunk.changed_mask & 1, 1);
                assert_eq!((chunk.changed_mask >> 1) & 1, 1);
            }
            assert_eq!(vel_storage.changed_mask, 0);
        }
    }

    #[test]
    fn viewmut_mask_propagation_with_none_filter() {
        register_components_once();
        let mut world = World::new();
        {
            let frame = Frame::new(world.current_tick());
            let pos_storage = world.get_storage::<Position>();
            let vel_storage = world.get_storage::<Velocity>();
            let frozen_storage = world.get_storage::<Frozen>();
            let pos_storage = unsafe { &mut *pos_storage };
            let vel_storage = unsafe { &mut *vel_storage };
            let frozen_storage = unsafe { &mut *frozen_storage };
            pos_storage.set(&frame, 0, Position { x: 0.0, y: 0.0 });
            vel_storage.set(&frame, 0, Velocity { x: 1.0, y: 2.0 });
            pos_storage.set(&frame, 1, Position { x: 10.0, y: 20.0 });
            vel_storage.set(&frame, 1, Velocity { x: 5.0, y: 10.0 });
            frozen_storage.set(&frame, 1, Frozen);
            pos_storage.clear_changed_masks();
            vel_storage.clear_changed_masks();
            frozen_storage.clear_changed_masks();
        }
        let system = FilteredMovement::new(&mut world);
        let frame = Frame::new(world.current_tick());
        system.run(&frame);
        {
            let pos_storage = unsafe { &*world.get_storage::<Position>() };
            let vel_storage = unsafe { &*world.get_storage::<Velocity>() };
            assert_eq!(pos_storage.changed_mask & 1, 1);
            unsafe {
                let page = &*pos_storage.data[0];
                assert_eq!(page.changed_mask & 1, 1);
                let chunk = &*page.data[0];
                assert_eq!(chunk.changed_mask & 1, 1);
                assert_eq!((chunk.changed_mask >> 1) & 1, 0);
            }
            assert_eq!(vel_storage.changed_mask, 0);
        }
    }
}
