use crate::component::Component;

pub struct Ecs;

static mut NEXT_ID: u32 = 2;

impl Ecs {
    pub fn register<T: Component>() {
        unsafe {
            let id = NEXT_ID;

            NEXT_ID = NEXT_ID.wrapping_add(1);

            T::initialize(id);
        }
    }
}
