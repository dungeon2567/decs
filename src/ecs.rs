use crate::component::Component;

pub struct Ecs;

static mut NEXT_ID: u32 = 0;

impl Ecs {
    pub fn register<T: Component>() {
        unsafe {
            let mut id = NEXT_ID;
            NEXT_ID = NEXT_ID.wrapping_add(1);
            if id == u32::MAX {
                id = NEXT_ID;
                NEXT_ID = NEXT_ID.wrapping_add(1);
            }
            T::initialize(id);
        }
    }
}

