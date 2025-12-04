use decs::ecs::Ecs;
use decs::entity::Entity;
use decs::frame::Frame;
use decs::storage::Storage;
use decs::tick::Tick;
use decs::world::World;
use decs_macros::Component;
use std::sync::Once;
fn register_components_once() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        Ecs::register::<Entity>();
        Ecs::register::<TestComponent>();
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Component, Default)]
struct TestComponent {
    value: i32,
}

#[test]
fn test_spawn_entity_generation_and_index() {
    register_components_once();
    let mut world = World::new();

    let storage = world.get_storage::<Entity>();
    let storage = unsafe { &mut *storage };
    let frame = Frame::new(world.current_tick());

    // Spawn first entity - should have index 0 and generation 1 (global counter starts at 1)
    let entity1 = storage.spawn(&frame).expect("Should spawn entity");
    assert_eq!(
        entity1.index(),
        0,
        "First spawned entity should have index 0"
    );
    assert_eq!(
        entity1.generation(),
        1,
        "First spawned entity should have generation 1"
    );

    // Spawn second entity - should have index 1 and generation 2 (global counter increments)
    let entity2 = storage.spawn(&frame).expect("Should spawn entity");
    assert_eq!(
        entity2.index(),
        1,
        "Second spawned entity should have index 1"
    );
    assert_eq!(
        entity2.generation(),
        2,
        "Second spawned entity should have generation 2"
    );

    // Spawn third entity - should have index 2 and generation 3
    let entity3 = storage.spawn(&frame).expect("Should spawn entity");
    assert_eq!(
        entity3.index(),
        2,
        "Third spawned entity should have index 2"
    );
    assert_eq!(
        entity3.generation(),
        3,
        "Third spawned entity should have generation 3"
    );

    // Remove entity at index 1 (entity2)
    assert!(
        storage.remove(&frame, 1),
        "Should successfully remove entity at index 1"
    );

    // Spawn again - should reuse index 1 but with next global generation
    let entity4 = storage.spawn(&frame).expect("Should spawn entity");
    assert_eq!(
        entity4.index(),
        1,
        "Spawned entity after removal should reuse index 1"
    );
    assert_eq!(
        entity4.generation(),
        4,
        "Spawned entity after removal should have generation 4 (next global generation)"
    );

    // Remove entity at index 1 again
    assert!(
        storage.remove(&frame, 1),
        "Should successfully remove entity at index 1 again"
    );

    // Spawn again - should reuse index 1 with next global generation
    let entity5 = storage.spawn(&frame).expect("Should spawn entity");
    assert_eq!(
        entity5.index(),
        1,
        "Spawned entity after second removal should reuse index 1"
    );
    assert_eq!(
        entity5.generation(),
        5,
        "Spawned entity after second removal should have generation 5"
    );

    // Remove entity at index 0
    assert!(
        storage.remove(&frame, 0),
        "Should successfully remove entity at index 0"
    );

    // Spawn again - should reuse index 0 with next global generation
    let entity6 = storage.spawn(&frame).expect("Should spawn entity");
    assert_eq!(entity6.index(), 0, "Spawned entity should reuse index 0");
    assert_eq!(
        entity6.generation(),
        6,
        "Spawned entity should have generation 6 (next global generation)"
    );

    // Verify all entities are stored correctly
    assert_eq!(
        storage.get(0).copied(),
        Some(entity6),
        "Entity at index 0 should match spawned entity"
    );
    assert_eq!(
        storage.get(1).copied(),
        Some(entity5),
        "Entity at index 1 should match spawned entity"
    );
    assert_eq!(
        storage.get(2).copied(),
        Some(entity3),
        "Entity at index 2 should match spawned entity"
    );

    // Clear changed masks before verifying invariants
    storage.clear_changed_masks();

    // Verify invariants
    assert!(
        world.verify_invariants(),
        "World invariants should hold after spawn operations"
    );
}

#[test]
fn test_spawn_multiple_entities_sequential() {
    register_components_once();
    let mut world = World::new();
    let storage_ptr = world.get_storage::<Entity>();
    {
        let storage = unsafe { &mut *storage_ptr };
        // Spawn 10 entities and verify they have correct indices and sequential global generations
        let mut entities = Vec::new();
        let frame = Frame::new(world.current_tick());
        for i in 0..10 {
            let entity = storage.spawn(&frame).expect("Should spawn entity");
            assert_eq!(entity.index(), i, "Entity {} should have index {}", i, i);
            assert_eq!(
                entity.generation(),
                (i + 1) as u64,
                "Entity {} should have generation {}",
                i,
                i + 1
            );
            entities.push(entity);
        }

        // Verify all entities are stored
        for (i, &entity) in entities.iter().enumerate() {
            assert_eq!(
                storage.get(i as u32).copied(),
                Some(entity),
                "Entity at index {} should match",
                i
            );
        }

        // Remove entity at index 5 to create a tombstone
        assert!(
            storage.remove(&frame, 5),
            "Should successfully remove entity at index 5"
        );

        // Spawn new entity - should reuse index 5 with next global generation
        let entity5_new = storage.spawn(&frame).expect("Should spawn entity");
        assert_eq!(
            entity5_new.index(),
            5,
            "Should reuse index 5 (first tombstone)"
        );
        assert_eq!(
            entity5_new.generation(),
            11,
            "Should have generation 11 (next global generation)"
        );

        // Remove entity at index 5 again
        assert!(
            storage.remove(&frame, 5),
            "Should successfully remove entity at index 5 again"
        );

        // Spawn again - should reuse index 5 with next global generation
        let entity5_new2 = storage.spawn(&frame).expect("Should spawn entity");
        assert_eq!(entity5_new2.index(), 5, "Should reuse index 5 again");
        assert_eq!(
            entity5_new2.generation(),
            12,
            "Should have generation 12 (next global generation)"
        );

        // After all tombstones are used, next spawn should use first empty slot (index 10)
        let entity10 = storage.spawn(&frame).expect("Should spawn entity");
        assert_eq!(
            entity10.index(),
            10,
            "Should use first empty slot after tombstones"
        );
        assert_eq!(
            entity10.generation(),
            13,
            "Should have generation 13 (next global generation)"
        );

        // Clear changed masks before verifying invariants
        storage.clear_changed_masks();
    }

    // Verify invariants after mutable borrow ends
    assert!(world.verify_invariants(), "World invariants should hold");
}

#[test]
fn test_spawn_after_remove_rollback_correctness() {
    register_components_once();
    let mut storage = Storage::<Entity>::new();
    let mut frame = Frame::new(Tick(0));

    // 1. Spawn an entity (index 0, gen 1)
    let entity1 = storage.spawn(&frame).unwrap();
    assert_eq!(entity1.index(), 0);
    assert_eq!(entity1.generation(), 1);

    // 2. Clear rollback (simulate new tick)
    // We simulate a new tick by updating the storage's current tick
    frame.current_tick = Tick(1);

    // Tick 1 starts.
    // Remove Entity 1.
    storage.remove(&frame, entity1.index());

    // Verify removal is tracked
    assert!(storage.rollback.verify_was_removed(entity1.index()));

    // Spawn Entity 2. It should reuse index 0.
    let entity2 = storage.spawn(&frame).unwrap();
    assert_eq!(entity2.index(), 0);
    assert_eq!(entity2.generation(), 2);

    // Now check rollback state.
    // It should be "modified" (Change), NOT "created".
    // Because we removed an existing entity and put a new one in.
    // The net effect is a change from Entity1 to Entity2.

    assert!(
        storage.rollback.verify_was_modified(0),
        "Should be marked as modified (Change)"
    );
    assert!(
        !storage.rollback.verify_was_created(0),
        "Should NOT be marked as created"
    );
    assert!(
        !storage.rollback.verify_was_removed(0),
        "Should NOT be marked as removed"
    );

    // Also check that the old value (Entity 1) is stored in rollback
    let old_val = storage
        .rollback
        .get(0)
        .expect("Should have old value stored");
    assert_eq!(
        old_val.generation(),
        1,
        "Should have stored the old generation"
    );
}

#[test]
fn test_storage_invariants_inherent() {
    register_components_once();
    let mut storage = Storage::<TestComponent>::new();
    let frame = Frame::new(Tick(0));
    storage.set(&frame, 0, TestComponent { value: 1 });
    storage.set(&frame, 64, TestComponent { value: 2 });
    storage.set(&frame, 4096, TestComponent { value: 3 });
    storage.clear_changed_masks();
    assert!(storage.verify_invariants());
}

#[test]
fn test_storage_like_verify_invariants_changed_mask_required_clear() {
    register_components_once();
    let mut world = World::new();
    let storage_ptr = world.get_storage::<TestComponent>();
    {
        let storage = unsafe { &mut *storage_ptr };
        let frame = Frame::new(world.current_tick());
        storage.set(&frame, 5, TestComponent { value: 10 });
    }
    assert!(!world.verify_invariants());
    {
        let storage = unsafe { &mut *storage_ptr };
        storage.clear_changed_masks();
    }
    assert!(world.verify_invariants());
}
