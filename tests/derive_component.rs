use decs::component::Component;
use decs_macros::Component;
use decs::ecs::Ecs;

#[allow(dead_code)]
#[derive(Component)]
struct Foo(u32);

#[test]
fn derive_component_assigns_id() {
    Ecs::register::<Foo>();

    let id1 = Foo::id();
    let id2 = Foo::id();
    assert_eq!(id1, id2);
}
