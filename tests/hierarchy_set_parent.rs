use decs::ecs::Ecs;
use decs::frame::Frame;
use decs::world::World;

#[test]
fn childof_set_parent_updates_pending_parent() {
    Ecs::register::<decs::hierarchy::ChildOf>();
    let mut world = World::new();
    let f = Frame::new(world.current_tick());
    let storage = world.get_storage_mut::<decs::hierarchy::ChildOf>();

    let parent = decs::entity::Entity::new(1, 1);
    storage.set_parent(&f, 0, parent);

    let v = unsafe {
        (*world.get_storage::<decs::hierarchy::ChildOf>())
            .get(0)
            .unwrap()
    };
    
    assert_eq!(v.pending_parent, Some(parent));
}
