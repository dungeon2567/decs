use decs::ecs::Ecs;
use std::sync::Once;
fn register_components_once() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        Ecs::register::<DebugComp>();
    });
}
use decs_macros::Component;
use decs::storage::Storage;
use decs::tick::Tick;
use decs::frame::Frame;


#[allow(dead_code)]
#[derive(Clone, Copy, Component)]
struct DebugComp {
    v: u32,
}

 

#[test]
fn debug_allocation_logs() {
    register_components_once();
    let mut storage = Storage::<DebugComp>::new();
    let frame = Frame::new(Tick(1));

    for storage_idx in 0..3u32 {
        for page_idx in 0..5u32 {
            let base = storage_idx * (64 * 64) + page_idx * 64;
            for c in [0u32, 17, 31, 63] {
                storage.set(&frame, base + c, DebugComp { v: c });
            }
        }
    }
}
