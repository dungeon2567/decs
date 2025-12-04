use decs::component::Component;
use decs::ecs::Ecs;
use decs_macros::Component;
use std::sync::Once;
fn register_components_once() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        Ecs::register::<Position>();
        Ecs::register::<Velocity>();
        Ecs::register::<Health>();
    });
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Component)]
struct Position {
    x: f32,
    y: f32,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Component)]
struct Velocity {
    x: f32,
    y: f32,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Component)]
struct Health {
    value: i32,
}

#[test]
fn test_component_id() {
    register_components_once();
    let pos_id = Position::id();
    let vel_id = Velocity::id();

    // Different component types should have different IDs
    assert_ne!(pos_id, vel_id);

    // IDs should not be default (u32::MAX)
    assert_ne!(pos_id, u32::MAX);
    assert_ne!(vel_id, u32::MAX);
}

#[test]
fn test_component_id_consistency() {
    register_components_once();
    // Component IDs should be consistent across calls
    let id1 = Position::id();
    let id2 = Position::id();
    assert_eq!(id1, id2);
}

#[test]
fn test_register_sets_id() {
    register_components_once();
    let before = Health::id();
    Ecs::register::<Health>();
    let after = Health::id();
    assert_eq!(before, after);
    assert_ne!(after, u32::MAX);
}

#[test]
fn test_multiple_component_types() {
    register_components_once();
    let pos_id = Position::id();
    let vel_id = Velocity::id();
    let health_id = Health::id();

    // All should be different
    assert_ne!(pos_id, vel_id);
    assert_ne!(pos_id, health_id);
    assert_ne!(vel_id, health_id);
}
