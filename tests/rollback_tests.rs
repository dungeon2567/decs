use decs::frame::Frame;
use decs::storage::Storage;
use decs::tick::Tick;
use decs_macros::Component;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Component, Default)]
struct TestComponent {
    value: i32,
}

#[derive(Debug, Component)]
struct Probe {
    id: u32,
    value: i32,
    clones: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
}

#[derive(Debug, Component)]
struct CounterComponent {
    value: i32,
}

impl Clone for Probe {
    fn clone(&self) -> Self {
        self.clones
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Probe {
            id: self.id,
            value: self.value,
            clones: self.clones.clone(),
            drops: self.drops.clone(),
        }
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        self.drops.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

static CLONES: AtomicUsize = AtomicUsize::new(0);
static DROPS: AtomicUsize = AtomicUsize::new(0);

impl Clone for CounterComponent {
    fn clone(&self) -> Self {
        CLONES.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        CounterComponent { value: self.value }
    }
}

impl Drop for CounterComponent {
    fn drop(&mut self) {
        DROPS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

#[test]
fn test_rollback_idempotent_remove_add_successful() {
    let mut storage = Storage::<TestComponent>::new();

    // Step 1: Create an item (it didn't exist before, so this is an Add operation)
    let frame = Frame::new(Tick(0));
    storage.set(&frame, 0, TestComponent { value: 100 });

    // Verify: After Add, item should be marked as created
    assert!(
        storage.rollback.verify_was_created(0),
        "After Add operation, item should be marked as created in RollbackStorage"
    );

    // Step 2: Remove the item (successful - item existed in Storage)
    assert!(
        storage.remove(&frame, 0),
        "Remove operation should succeed when item exists in Storage"
    );

    // Verify: After Add->Remove (idempotent), no change should be tracked
    assert!(
        storage.rollback.verify_not_changed(0),
        "After Add->Remove idempotent operation, item should not be tracked (no change occurred)"
    );

    // Step 3: Add the item back (since previous Add->Remove cleared tracking, this is a new Add)
    storage.set(&frame, 0, TestComponent { value: 200 });

    // Verify: Item exists in Storage with the new value
    assert_eq!(
        storage.get(0).unwrap().value,
        200,
        "Item should exist in Storage with the new value after second Add"
    );

    // Verify: After Add (second time), item should be marked as created (not modified)
    // This is because Add->Remove cleared all tracking, so the second Add is treated as a new creation
    assert!(
        storage.rollback.verify_was_created(0),
        "After Add->Remove->Add sequence, item should be marked as created (second Add is a new creation, not Remove->Add)"
    );

    // Verify: Item should not be marked as "not changed" (it was created)
    assert!(
        !storage.rollback.verify_not_changed(0),
        "Item should be tracked as created, not as 'not changed'"
    );

    // Verify: Item should not be marked as removed
    assert!(
        !storage.rollback.verify_was_removed(0),
        "Item should not be marked as removed after Add->Remove->Add sequence"
    );

    // Verify: Created items do not store old values in rollback
    let old_value = storage.rollback.get(0);
    assert!(
        old_value.is_none(),
        "Created items have no stored old value in RollbackStorage"
    );
}

#[test]
fn test_rollback_full_tree_invariance() {
    let mut storage = Storage::<TestComponent>::new();

    // Create items across different storage, page, and chunk indices to test full tree
    // Storage 0, Page 0, Chunk 0
    let frame = Frame::new(Tick(0));
    storage.set(&frame, 0, TestComponent { value: 1 });
    // Storage 0, Page 0, Chunk 63
    storage.set(&frame, 63, TestComponent { value: 2 });
    // Storage 0, Page 1, Chunk 0
    storage.set(&frame, 64, TestComponent { value: 3 });
    // Storage 1, Page 0, Chunk 0
    storage.set(&frame, 4096, TestComponent { value: 4 });
    // Storage 1, Page 1, Chunk 0
    storage.set(&frame, 4160, TestComponent { value: 5 });

    // Verify all items are marked as created
    assert!(
        storage.rollback.verify_was_created(0),
        "Item at index 0 should be marked as created"
    );
    assert!(
        storage.rollback.verify_was_created(63),
        "Item at index 63 should be marked as created"
    );
    assert!(
        storage.rollback.verify_was_created(64),
        "Item at index 64 should be marked as created"
    );
    assert!(
        storage.rollback.verify_was_created(4096),
        "Item at index 4096 should be marked as created"
    );
    assert!(
        storage.rollback.verify_was_created(4160),
        "Item at index 4160 should be marked as created"
    );

    // Perform Add->Remove idempotent operation on item at index 0
    storage.remove(&frame, 0);
    assert!(
        storage.rollback.verify_not_changed(0),
        "After Add->Remove at index 0, item should not be tracked (idempotent: no change)"
    );

    // Add item back at index 0 (new Add, not Remove->Add)
    storage.set(&frame, 0, TestComponent { value: 10 });
    assert!(
        storage.rollback.verify_was_created(0),
        "After Add->Remove->Add at index 0, item should be marked as created (new Add, not Remove->Add)"
    );

    // Perform Add->Remove idempotent operation on item at index 16384 (different storage)
    storage.remove(&frame, 4096);
    assert!(
        storage.rollback.verify_not_changed(4096),
        "After Add->Remove at index 4096, item should not be tracked (idempotent: no change)"
    );

    // Add item back at index 16384 (new Add, not Remove->Add)
    storage.set(&frame, 4096, TestComponent { value: 40 });
    assert!(
        storage.rollback.verify_was_created(4096),
        "After Add->Remove->Add at index 4096, item should be marked as created (new Add, not Remove->Add)"
    );

    // Verify all invariants hold across the entire tree
    assert!(
        storage.rollback.verify_invariants(),
        "RollbackStorage invariants should hold across all storage, page, and chunk levels"
    );
    assert!(
        storage.verify_invariants(),
        "Storage invariants should hold across all storage, page, and chunk levels"
    );

    // Verify final states
    assert_eq!(
        storage.get(0).unwrap().value,
        10,
        "Item at index 0 should have final value 10"
    );
    assert_eq!(
        storage.get(63).unwrap().value,
        2,
        "Item at index 63 should have value 2"
    );
    assert_eq!(
        storage.get(64).unwrap().value,
        3,
        "Item at index 64 should have value 3"
    );
    assert_eq!(
        storage.get(4096).unwrap().value,
        40,
        "Item at index 4096 should have final value 40"
    );
    assert_eq!(
        storage.get(4160).unwrap().value,
        5,
        "Item at index 4160 should have value 5"
    );
}

#[test]
fn test_rollback_full_coverage_per_index_max_one_op() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Use top-level Probe type

    // top-level impls used

    let mut storage = Storage::<Probe>::new();

    let indices: [u32; 6] = [0, 63, 64, 4096, 4160, 50000];
    let mut clone_counts = std::collections::HashMap::new();
    let mut drop_counts = std::collections::HashMap::new();

    for &id in &indices {
        clone_counts.insert(id, Arc::new(AtomicUsize::new(0)));
        drop_counts.insert(id, Arc::new(AtomicUsize::new(0)));
    }

    let mut frame = Frame::new(Tick(1));
    for &id in &indices[..5] {
        let p = Probe {
            id,
            value: 32,
            clones: clone_counts[&id].clone(),
            drops: drop_counts[&id].clone(),
        };
        storage.set(&frame, id, p);
    }
    storage.clear_changed_masks();

    frame.current_tick = Tick(23);
    for &id in &[0u32, 63u32, 64u32] {
        let p = Probe {
            id,
            value: 100,
            clones: clone_counts[&id].clone(),
            drops: drop_counts[&id].clone(),
        };
        storage.set(&frame, id, p);
    }
    storage.clear_changed_masks();

    frame.current_tick = Tick(30);
    {
        let id = 50000u32;
        let p = Probe {
            id,
            value: 77,
            clones: clone_counts[&id].clone(),
            drops: drop_counts[&id].clone(),
        };
        storage.set(&frame, id, p);
    }
    storage.clear_changed_masks();

    storage.clear_changed_masks();
    storage.rollback(Tick(2));

    for &id in &indices[..5] {
        assert_eq!(
            storage.get(id).unwrap().value,
            32,
            "index {} value should equal target tick value 32",
            id
        );
    }
    assert!(
        storage.get(50000).is_none(),
        "index 50000 should not exist at target tick"
    );

    for &id in &indices {
        let c = clone_counts[&id].load(Ordering::SeqCst);
        let d = drop_counts[&id].load(Ordering::SeqCst);
        assert!(c <= 1, "index {} clone count {} exceeds 1", id, c);
        assert!(d <= 2, "index {} drop count {} exceeds 2", id, d);
    }
}

#[test]
fn test_rollback_pages_chunks_5k_min_ops() {
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Use top-level Probe type

    // top-level impls used

    let mut storage = Storage::<Probe>::new();

    let n = 5000u32;
    let mut clone_counts: HashMap<u32, Arc<AtomicUsize>> = HashMap::new();
    let mut drop_counts: HashMap<u32, Arc<AtomicUsize>> = HashMap::new();

    for id in 0..n {
        clone_counts.insert(id, Arc::new(AtomicUsize::new(0)));
        drop_counts.insert(id, Arc::new(AtomicUsize::new(0)));
    }
    for id in n..(n + 1000) {
        clone_counts.insert(id, Arc::new(AtomicUsize::new(0)));
        drop_counts.insert(id, Arc::new(AtomicUsize::new(0)));
    }

    // Tick 1: create baseline values across 0..n
    let mut frame = Frame::new(Tick(1));
    for id in 0..n {
        let p = Probe {
            id,
            value: 1,
            clones: clone_counts[&id].clone(),
            drops: drop_counts[&id].clone(),
        };
        storage.set(&frame, id, p);
    }
    storage.clear_changed_masks();

    // Tick 23: change a subset (every 3rd) to value 2
    frame.current_tick = Tick(23);
    for id in (0..n).step_by(3) {
        let p = Probe {
            id,
            value: 2,
            clones: clone_counts[&id].clone(),
            drops: drop_counts[&id].clone(),
        };
        storage.set(&frame, id, p);
    }
    storage.clear_changed_masks();

    // Tick 38: remove a different subset (every 5th)
    frame.current_tick = Tick(38);
    for id in (0..n).step_by(5) {
        let _ = storage.remove(&frame, id);
    }
    storage.clear_changed_masks();

    // Tick 30: create new entities after target across n..n+1000
    frame.current_tick = Tick(30);
    for id in n..(n + 1000) {
        let p = Probe {
            id,
            value: 99,
            clones: clone_counts[&id].clone(),
            drops: drop_counts[&id].clone(),
        };
        storage.set(&frame, id, p);
    }
    storage.clear_changed_masks();

    // Current tick 40: rollback to tick 2 and validate full coverage
    // frame.current_tick = Tick(40); // not required before rollback
    storage.rollback(Tick(2));

    for id in 0..n {
        assert_eq!(
            storage.get(id).unwrap().value,
            1,
            "index {} should equal target tick value 1",
            id
        );
    }
    for id in n..(n + 1000) {
        assert!(
            storage.get(id).is_none(),
            "index {} should not exist at target tick",
            id
        );
    }

    for id in 0..(n + 1000) {
        let c = clone_counts[&id].load(Ordering::SeqCst);
        let d = drop_counts[&id].load(Ordering::SeqCst);
        assert!(c <= 1, "index {} clone count {} exceeds 1", id, c);
        assert!(d <= 1, "index {} drop count {} exceeds 1", id, d);
    }

    assert!(storage.verify_invariants());
    assert!(storage.rollback.verify_invariants());
}
#[test]
fn test_rollback_minimal_ops_no_leak() {
    use std::sync::atomic::Ordering;
    // top-level statics and impls used

    // Reset counters
    CLONES.store(0, Ordering::SeqCst);
    DROPS.store(0, Ordering::SeqCst);

    let mut storage = Storage::<CounterComponent>::new();
    let idx = 10u32;

    // Tick 1: create with value 32
    let mut frame = Frame::new(Tick(1));
    storage.set(&frame, idx, CounterComponent { value: 32 });
    storage.clear_changed_masks();

    // Tick 23: change to 100 (stores old value 32 in rollback)
    frame.current_tick = Tick(23);
    storage.set(&frame, idx, CounterComponent { value: 100 });
    storage.clear_changed_masks();

    // Tick 38: remove (stores old value 100 in rollback)
    frame.current_tick = Tick(38);
    assert!(storage.remove(&frame, idx));
    storage.clear_changed_masks();

    // Current tick 40: rollback to tick 2
    // frame.current_tick = Tick(40); // not required before rollback
    storage.rollback(Tick(2));

    // After rollback, value should be as at tick 2: 32
    assert_eq!(storage.get(idx).unwrap().value, 32);

    // Strict minimal op: exactly one clone to reach target state
    assert_eq!(
        CLONES.load(Ordering::SeqCst),
        1,
        "Rollback should perform exactly one clone"
    );

    // Invariants hold (no structural inconsistencies)
    assert!(storage.verify_invariants());
    assert!(storage.rollback.verify_invariants());

    // Drop storage to release all values; ensure drops happened (no leaks observed)
    drop(storage);
    let drops = DROPS.load(Ordering::SeqCst);
    assert!(
        drops >= 3,
        "Expected at least three drops (stored old values + final value), got {}",
        drops
    );
}
#[test]
fn test_rollback_pages_chunks_20k_values_correct() {
    let mut storage = Storage::<TestComponent>::new();

    let n = 20000u32;

    let mut frame = Frame::new(Tick(1));
    for id in 0..n {
        storage.set(&frame, id, TestComponent { value: 1 });
    }
    storage.clear_changed_masks();

    frame.current_tick = Tick(23);
    for id in (0..n).step_by(7) {
        storage.set(&frame, id, TestComponent { value: 2 });
    }
    storage.clear_changed_masks();

    frame.current_tick = Tick(38);
    for id in (0..n).step_by(11) {
        let _ = storage.remove(&frame, id);
    }
    storage.clear_changed_masks();

    // current tick change not required before rollback
    storage.rollback(Tick(2));

    for id in 0..n {
        assert_eq!(storage.get(id).unwrap().value, 1);
    }
}
