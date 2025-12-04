use decs::storage::Storage;
use decs_macros::Component;
use decs::tick::Tick;
use decs::frame::Frame;

#[derive(Clone, Copy, Component)]
struct TestComponent { value: i32 }

#[test]
fn test_set_created_then_modified_same_tick() {
    let mut storage = Storage::<TestComponent>::new();
    let frame = Frame::new(Tick(0));
    storage.set(&frame, 1, TestComponent { value: 10 });
    storage.set(&frame, 1, TestComponent { value: 20 });
    assert!(storage.rollback.verify_was_created(1));
    assert!(!storage.rollback.verify_was_modified(1));
}

#[test]
fn test_set_existing_modified_after_tick() {
    let mut storage = Storage::<TestComponent>::new();
    let mut frame = Frame::new(Tick(0));
    storage.set(&frame, 2, TestComponent { value: 10 });
    frame.current_tick = Tick(1);
    storage.clear_changed_masks();

    storage.set(&frame, 2, TestComponent { value: 30 });
    assert!(storage.rollback.verify_was_modified(2));
    assert_eq!(storage.rollback.get(2).unwrap().value, 10);
}

#[test]
fn test_remove_created_idempotent() {
    let mut storage = Storage::<TestComponent>::new();
    let frame = Frame::new(Tick(0));
    storage.set(&frame, 3, TestComponent { value: 5 });
    storage.clear_changed_masks();

    assert!(storage.remove(&frame, 3));
    assert!(storage.rollback.verify_not_changed(3));
}

#[test]
fn test_remove_existing_tracks_removed() {
    let mut storage = Storage::<TestComponent>::new();
    let mut frame = Frame::new(Tick(0));
    storage.set(&frame, 4, TestComponent { value: 7 });
    storage.clear_changed_masks();
    frame.current_tick = Tick(1);
    assert!(storage.remove(&frame, 4));
    assert!(storage.rollback.verify_was_removed(4));
}

#[test]
fn test_remove_add_same_tick_tracks_modified() {
    let mut storage = Storage::<TestComponent>::new();
    let mut frame = Frame::new(Tick(0));
    storage.set(&frame, 5, TestComponent { value: 11 });
    storage.clear_changed_masks();
    frame.current_tick = Tick(1);
    assert!(storage.remove(&frame, 5));
    storage.set(&frame, 5, TestComponent { value: 13 });
    assert!(storage.rollback.verify_was_modified(5));
}

#[test]
fn test_add_change_remove_same_tick_no_change() {
    let mut storage = Storage::<TestComponent>::new();
    let frame = Frame::new(Tick(0));
    storage.set(&frame, 6, TestComponent { value: 100 });
    storage.set(&frame, 6, TestComponent { value: 200 });
    assert!(storage.rollback.verify_was_created(6));
    assert!(!storage.rollback.verify_was_modified(6));
    assert!(storage.remove(&frame, 6));
    assert!(storage.rollback.verify_not_changed(6));
}

#[test]
fn test_remove_add_change_same_tick_preserve_old_value() {
    let mut storage = Storage::<TestComponent>::new();
    let mut frame = Frame::new(Tick(0));
    storage.set(&frame, 7, TestComponent { value: 7 });
    storage.clear_changed_masks();
    frame.current_tick = Tick(1);
    assert!(storage.remove(&frame, 7));
    storage.set(&frame, 7, TestComponent { value: 8 });
    storage.set(&frame, 7, TestComponent { value: 9 });
    assert!(storage.rollback.verify_was_modified(7));
    assert_eq!(storage.rollback.get(7).unwrap().value, 7);
}

#[test]
fn test_remove_nonexistent_then_add_creation() {
    let mut storage = Storage::<TestComponent>::new();
    let frame = Frame::new(Tick(0));
    assert!(!storage.remove(&frame, 8));
    storage.set(&frame, 8, TestComponent { value: 42 });
    assert!(storage.rollback.verify_was_created(8));
    assert!(!storage.rollback.verify_was_modified(8));
    assert!(!storage.rollback.verify_was_removed(8));
    assert!(storage.rollback.get(8).is_none());
}

#[test]
fn test_multiple_changes_on_existing_same_tick_preserve_old_value() {
    let mut storage = Storage::<TestComponent>::new();
    let mut frame = Frame::new(Tick(0));
    storage.set(&frame, 9, TestComponent { value: 1 });
    storage.clear_changed_masks();
    frame.current_tick = Tick(1);
    storage.set(&frame, 9, TestComponent { value: 2 });
    storage.set(&frame, 9, TestComponent { value: 3 });
    assert!(storage.rollback.verify_was_modified(9));
    assert_eq!(storage.rollback.get(9).unwrap().value, 1);
}
