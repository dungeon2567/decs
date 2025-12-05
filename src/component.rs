use crate::world::World;
use decs::system::TemporaryComponentCleanupSystem;
use std::alloc::Allocator;

pub trait Component
where
    Self: Sized + Clone + 'static,
{
    fn id() -> u32;

    fn initialize(_id: u32) {}

    fn schedule_cleanup_system(world: &mut World);

    fn clone_in(&self, _allocator: &dyn Allocator) -> Self {
        self.clone()
    }
}

#[derive(Clone)]
pub struct Destroyed();

#[allow(non_upper_case_globals)]
static mut __DECS_COMPONENT_ID_Destroyed: u32 = 1;

impl Destroyed {}

impl Component for Destroyed {
    fn id() -> u32 {
        unsafe { __DECS_COMPONENT_ID_Destroyed }
    }
    fn initialize(id: u32) {
        unsafe {
            if __DECS_COMPONENT_ID_Destroyed == u32::MAX {
                __DECS_COMPONENT_ID_Destroyed = id;
            }
        }
    }

    fn schedule_cleanup_system(world: &mut World) {
        let sys =
            TemporaryComponentCleanupSystem::<Destroyed, crate::world::DestroyGroup>::new(world);
        world.scheduler_mut().add_system(sys);
    }
}
