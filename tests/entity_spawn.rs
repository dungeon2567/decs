use decs::frame::Frame;
use decs::system; // for `system!`
use decs::view::View;
use decs::world::World;

system!(NoopSystem { query fn update(_e: View<decs::entity::Entity>) { let _ = _e.index(); } });

#[test]
fn spawn_creates_entities_and_sets_in_storage() {
    let mut world = World::new();
    let ent_storage = world.get_entity_storage();
    let f = Frame::new(world.current_tick());
    unsafe { &mut *ent_storage }.save_generation_for_rollback();
    let e1 = unsafe { &mut *ent_storage }.spawn(&f).unwrap();
    let e2 = unsafe { &mut *ent_storage }.spawn(&f).unwrap();

    let sp = world.get_storage::<decs::entity::Entity>();
    assert!(unsafe { (*sp).get(e1.index()).is_some() });
    assert!(unsafe { (*sp).get(e2.index()).is_some() });

    let sys = NoopSystem::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();
    world.run();
}

#[test]
fn spawn_until_chunk_boundary_then_remove_and_spawn_again() {
    let mut world = World::new();
    let ent_storage = world.get_entity_storage();
    {
        let f = Frame::new(world.current_tick());
        unsafe { &mut *ent_storage }.save_generation_for_rollback();
        for _ in 0..70 {
            let _ = unsafe { &mut *ent_storage }.spawn(&f);
        }
    }
    let sp = world.get_storage::<decs::entity::Entity>();
    assert!(unsafe { (*sp).get(0).is_some() });

    let f2 = Frame::new(world.current_tick());
    assert!(unsafe { &mut *sp }.remove(&f2, 0));

    let f3 = Frame::new(world.current_tick());
    let e_new = unsafe { &mut *ent_storage }.spawn(&f3).unwrap();
    assert!(unsafe { (*sp).get(e_new.index()).is_some() });

    let sys = NoopSystem::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();
    world.run();
}

#[test]
fn spawn_respects_fullness_mask_across_page() {
    let mut world = World::new();
    let ent_storage = world.get_entity_storage();
    {
        let f = Frame::new(world.current_tick());
        unsafe { &mut *ent_storage }.save_generation_for_rollback();
        // spawn enough to cross into next chunk
        for _ in 0..130 {
            let _ = unsafe { &mut *ent_storage }.spawn(&f);
        }
    }
    assert!(world.verify_invariants());

    let sys = NoopSystem::new(&mut world);
    world.scheduler_mut().add_system(sys);
    world.scheduler_mut().build_wavefronts();
    world.run();
}
