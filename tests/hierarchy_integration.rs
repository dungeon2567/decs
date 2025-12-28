use decs::ecs::Ecs;
use decs::entity::Entity;
use decs::frame::Frame;
use decs::hierarchy::{ChildOf, Parent, UpdateHierarchySystem};
use decs::system::System;
use decs::world::World;

#[test]
fn test_hierarchy_updates() {
    // Register components
    Ecs::register::<ChildOf>();
    Ecs::register::<Parent>();
    Ecs::register::<Entity>();

    let mut world = World::new();
    let frame = Frame::new(world.current_tick());

    // Create entities manually in storage since we don't have a high-level spawn helper in this test scope
    // We need to set Entity component for them to be "real"
    let parent_ent = Entity::new(1, 1);
    let child1_ent = Entity::new(2, 1);
    let child2_ent = Entity::new(3, 1);

    // Setup Entity storage
    {
        let entity_storage = world.get_entity_storage();
        unsafe {
            (*entity_storage).set(&frame, parent_ent.index(), parent_ent);
            (*entity_storage).set(&frame, child1_ent.index(), child1_ent);
            (*entity_storage).set(&frame, child2_ent.index(), child2_ent);
        }
    }

    // Setup ChildOf storage
    {
        let child_storage = world.get_storage_mut::<ChildOf>();
        
        // Child 1 requests Parent
        child_storage.set(&frame, child1_ent.index(), ChildOf {
            parent: None,
            next_sibling: None,
            prev_sibling: None,
            pending_parent: Some(parent_ent),
        });
    }

    // Run system
    let system = UpdateHierarchySystem::new(&mut world);
    system.run(&frame);

    // Verify Child 1 attached
    {
        let parent_storage = world.get_storage_mut::<Parent>();
        let p_comp = parent_storage.get(parent_ent.index()).expect("Parent component should exist");
        assert_eq!(p_comp.first_child, child1_ent);
        assert_eq!(p_comp.last_child, child1_ent);
    }

    {
        let child_storage = world.get_storage_mut::<ChildOf>();
        let c1_comp = child_storage.get(child1_ent.index()).expect("Child1 component should exist");
        assert_eq!(c1_comp.parent, Some(parent_ent));
        assert_eq!(c1_comp.pending_parent, None);

        // Child 2 requests same Parent (should append)
        child_storage.set(&frame, child2_ent.index(), ChildOf {
            parent: None,
            next_sibling: None,
            prev_sibling: None,
            pending_parent: Some(parent_ent),
        });
    }

    system.run(&frame);

    // Verify Child 2 appended
    {
        let parent_storage = world.get_storage_mut::<Parent>();
        let p_comp = parent_storage.get(parent_ent.index()).unwrap();
        assert_eq!(p_comp.first_child, child1_ent);
        assert_eq!(p_comp.last_child, child2_ent);
    }

    {
        let child_storage = world.get_storage_mut::<ChildOf>();
        let c1_comp = child_storage.get(child1_ent.index()).unwrap();
        assert_eq!(c1_comp.next_sibling, Some(child2_ent));

        let c2_comp = child_storage.get(child2_ent.index()).unwrap();
        assert_eq!(c2_comp.prev_sibling, Some(child1_ent));
        assert_eq!(c2_comp.parent, Some(parent_ent));
    }
}
