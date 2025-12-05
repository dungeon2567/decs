use decs::ecs::Ecs;
use decs::frame::Frame;
use decs::system; // for `system!`
use decs::system::System;
use decs::system::SystemGroup;
use decs::view::{View, ViewMut};
use decs::world::World;
use decs_macros::Component;
use std::any::TypeId;
use std::sync::Once;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq, Component)]
struct Position {
    x: f32,
    y: f32,
}

#[derive(Clone, Debug, PartialEq, Component)]
struct Velocity {
    x: f32,
    y: f32,
}

static INIT: Once = Once::new();
fn register_components_once() {
    INIT.call_once(|| {
        Ecs::register::<Position>();
        Ecs::register::<Velocity>();
    });
}

struct WritePos {
    order: Arc<Mutex<[u32; 32]>>,
    step: Arc<AtomicU32>,
}
impl System for WritePos {
    fn run(&self, _: &Frame) {
        let s = self.step.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        self.order.lock().unwrap()[s as usize] = 100;
    }
    fn writes(&self) -> &'static [TypeId] {
        static W: &[TypeId] = &[TypeId::of::<Position>()];
        W
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct ReadPosA {
    order: Arc<Mutex<[u32; 32]>>,
    step: Arc<AtomicU32>,
}
impl System for ReadPosA {
    fn run(&self, _: &Frame) {
        let s = self.step.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        self.order.lock().unwrap()[s as usize] = 200;
    }
    fn reads(&self) -> &'static [TypeId] {
        static R: &[TypeId] = &[TypeId::of::<Position>()];
        R
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct ReadPosB {
    order: Arc<Mutex<[u32; 32]>>,
    step: Arc<AtomicU32>,
}
impl System for ReadPosB {
    fn run(&self, _: &Frame) {
        let s = self.step.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        self.order.lock().unwrap()[s as usize] = 300;
    }
    fn reads(&self) -> &'static [TypeId] {
        static R: &[TypeId] = &[TypeId::of::<Position>()];
        R
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct WriteVel {
    order: Arc<Mutex<[u32; 32]>>,
    step: Arc<AtomicU32>,
}
impl System for WriteVel {
    fn run(&self, _: &Frame) {
        let s = self.step.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        self.order.lock().unwrap()[s as usize] = 400;
    }
    fn writes(&self) -> &'static [TypeId] {
        static W: &[TypeId] = &[TypeId::of::<Velocity>()];
        W
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[test]
fn writer_reader_order_enforced() {
    register_components_once();
    let mut world = World::new();
    let f = Frame::new(world.current_tick());
    let pos = world.get_storage_mut::<Position>();
    pos.set(&f, 0, Position { x: 0.0, y: 0.0 });
    let order = Arc::new(Mutex::new([0; 32]));
    let step = Arc::new(AtomicU32::new(0));
    let w = WritePos {
        order: order.clone(),
        step: step.clone(),
    };
    let r = ReadPosA {
        order: order.clone(),
        step: step.clone(),
    };
    world.scheduler_mut().add_system(w);
    world.scheduler_mut().add_system(r);
    world.scheduler_mut().build_wavefronts();
    assert!(world.scheduler().wavefronts().len() >= 2);
    world.run();
    let o = order.lock().unwrap();
    assert!(o[1] == 100 && o[2] == 200);
}

#[test]
fn writer_writer_chain_serialized() {
    register_components_once();
    let mut world = World::new();
    let f = Frame::new(world.current_tick());
    let pos = world.get_storage_mut::<Position>();
    pos.set(&f, 0, Position { x: 0.0, y: 0.0 });
    let order = Arc::new(Mutex::new([0; 32]));
    let step = Arc::new(AtomicU32::new(0));
    let w1 = WritePos {
        order: order.clone(),
        step: step.clone(),
    };
    let w2 = WritePos {
        order: order.clone(),
        step: step.clone(),
    };
    world.scheduler_mut().add_system(w1);
    world.scheduler_mut().add_system(w2);
    world.scheduler_mut().build_wavefronts();
    world.run();
    let o = order.lock().unwrap();
    assert!(o[1] == 100 && o[2] == 100);
}

#[test]
fn independent_readers_same_wavefront() {
    let mut world = World::new();
    struct Reader1 {
        order: Arc<Mutex<[u32; 32]>>,
        step: Arc<AtomicU32>,
    }
    impl System for Reader1 {
        fn run(&self, _: &Frame) {
            let s = self.step.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
            self.order.lock().unwrap()[s as usize] = 210;
        }
        fn reads(&self) -> &'static [TypeId] {
            static R: &[TypeId] = &[TypeId::of::<Position>()];
            R
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }
    struct Reader2 {
        order: Arc<Mutex<[u32; 32]>>,
        step: Arc<AtomicU32>,
    }
    impl System for Reader2 {
        fn run(&self, _: &Frame) {
            let s = self.step.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
            self.order.lock().unwrap()[s as usize] = 220;
        }
        fn reads(&self) -> &'static [TypeId] {
            static R: &[TypeId] = &[TypeId::of::<Position>()];
            R
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }
    let order = Arc::new(Mutex::new([0; 32]));
    let step = Arc::new(AtomicU32::new(0));
    world.scheduler_mut().add_system(Reader1 {
        order: order.clone(),
        step: step.clone(),
    });
    world.scheduler_mut().add_system(Reader2 {
        order: order.clone(),
        step: step.clone(),
    });
    world.scheduler_mut().build_wavefronts();
    world.run();
    let o = order.lock().unwrap();
    assert!((o[1] == 210 && o[2] == 220) || (o[1] == 220 && o[2] == 210));
}

struct SimSys2 {
    order: Arc<Mutex<[u32; 32]>>,
    step: Arc<AtomicU32>,
}
struct CleanupSys2 {
    order: Arc<Mutex<[u32; 32]>>,
    step: Arc<AtomicU32>,
}
struct DestroySys2 {
    order: Arc<Mutex<[u32; 32]>>,
    step: Arc<AtomicU32>,
}
impl System for SimSys2 {
    fn run(&self, _frame: &Frame) {
        let s = self.step.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        self.order.lock().unwrap()[s as usize] = 500;
    }
    fn name(&self) -> &'static str {
        "SimSys2"
    }
    fn parent(&self) -> Option<&dyn decs::system::SystemGroup> {
        Some(decs::world::SimulationGroup::instance())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl System for CleanupSys2 {
    fn run(&self, _frame: &Frame) {
        let s = self.step.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        self.order.lock().unwrap()[s as usize] = 600;
    }
    fn name(&self) -> &'static str {
        "CleanupSys2"
    }
    fn parent(&self) -> Option<&dyn decs::system::SystemGroup> {
        Some(decs::world::CleanupGroup::instance())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl System for DestroySys2 {
    fn run(&self, _frame: &Frame) {
        let s = self.step.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        self.order.lock().unwrap()[s as usize] = 700;
    }
    fn name(&self) -> &'static str {
        "DestroySys2"
    }
    fn parent(&self) -> Option<&dyn decs::system::SystemGroup> {
        Some(decs::world::DestroyGroup::instance())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[test]
fn group_order_simulation_before_cleanup_and_cleanup_before_destroy() {
    register_components_once();
    let mut world = World::new();
    let f = Frame::new(world.current_tick());
    let pos = world.get_storage_mut::<Position>();
    pos.set(&f, 0, Position { x: 0.0, y: 0.0 });
    let vel = world.get_storage_mut::<Velocity>();
    vel.set(&f, 0, Velocity { x: 1.0, y: 1.0 });
    let order = Arc::new(Mutex::new([0; 32]));
    let step = Arc::new(AtomicU32::new(0));
    world.scheduler_mut().add_system(SimSys2 {
        order: order.clone(),
        step: step.clone(),
    });
    world.scheduler_mut().add_system(CleanupSys2 {
        order: order.clone(),
        step: step.clone(),
    });
    world.scheduler_mut().add_system(DestroySys2 {
        order: order.clone(),
        step: step.clone(),
    });
    world.scheduler_mut().build_wavefronts();
    world.run();
    let o = order.lock().unwrap();
    assert!(o[1] == 500 && o[2] == 600 && o[3] == 700);
}

// Hand-written systems to validate System::before/after edges
struct SysA {
    order: Arc<Mutex<[u32; 32]>>,
    step: Arc<AtomicU32>,
}
struct SysB {
    order: Arc<Mutex<[u32; 32]>>,
    step: Arc<AtomicU32>,
}
impl System for SysA {
    fn run(&self, _frame: &Frame) {
        let s = self.step.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        self.order.lock().unwrap()[s as usize] = 800;
    }
    fn name(&self) -> &'static str {
        "SysA"
    }
    fn before(&self) -> &[TypeId] {
        static B: &[TypeId] = &[TypeId::of::<SysB>()];
        B
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl System for SysB {
    fn run(&self, _frame: &Frame) {
        let s = self.step.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        self.order.lock().unwrap()[s as usize] = 900;
    }
    fn name(&self) -> &'static str {
        "SysB"
    }
    fn after(&self) -> &[TypeId] {
        static A: &[TypeId] = &[TypeId::of::<SysA>()];
        A
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[test]
fn system_before_after_edges_respected() {
    let mut world = World::new();
    let order = Arc::new(Mutex::new([0; 32]));
    let step = Arc::new(AtomicU32::new(0));
    world.scheduler_mut().add_system(SysA {
        order: order.clone(),
        step: step.clone(),
    });
    world.scheduler_mut().add_system(SysB {
        order: order.clone(),
        step: step.clone(),
    });
    world.scheduler_mut().build_wavefronts();
    world.run();
    let o = order.lock().unwrap();
    assert!(o[1] == 800 && o[2] == 900);
}

#[test]
fn conflicting_writes_split_wavefronts() {
    let mut world = World::new();
    let order = Arc::new(Mutex::new([0; 32]));
    let step = Arc::new(AtomicU32::new(0));
    struct WriterPos1 {
        order: Arc<Mutex<[u32; 32]>>,
        step: Arc<AtomicU32>,
    }
    impl System for WriterPos1 {
        fn run(&self, _: &Frame) {
            let s = self.step.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
            self.order.lock().unwrap()[s as usize] = 310;
        }
        fn writes(&self) -> &'static [TypeId] {
            static W: &[TypeId] = &[TypeId::of::<Position>()];
            W
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }
    struct WriterPos2 {
        order: Arc<Mutex<[u32; 32]>>,
        step: Arc<AtomicU32>,
    }
    impl System for WriterPos2 {
        fn run(&self, _: &Frame) {
            let s = self.step.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
            self.order.lock().unwrap()[s as usize] = 320;
        }
        fn writes(&self) -> &'static [TypeId] {
            static W: &[TypeId] = &[TypeId::of::<Position>()];
            W
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }
    struct ReaderPos {
        order: Arc<Mutex<[u32; 32]>>,
        step: Arc<AtomicU32>,
    }
    impl System for ReaderPos {
        fn run(&self, _: &Frame) {
            let s = self.step.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
            self.order.lock().unwrap()[s as usize] = 330;
        }
        fn reads(&self) -> &'static [TypeId] {
            static R: &[TypeId] = &[TypeId::of::<Position>()];
            R
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }
    world.scheduler_mut().add_system(WriterPos1 {
        order: order.clone(),
        step: step.clone(),
    });
    world.scheduler_mut().add_system(WriterPos2 {
        order: order.clone(),
        step: step.clone(),
    });
    world.scheduler_mut().add_system(ReaderPos {
        order: order.clone(),
        step: step.clone(),
    });
    world.scheduler_mut().build_wavefronts();
    assert!(world.scheduler().wavefronts().len() >= 2);
    world.run();
}

#[test]
fn independent_components_merge_wavefronts() {
    let mut world = World::new();
    let order = Arc::new(Mutex::new([0; 32]));
    let step = Arc::new(AtomicU32::new(0));
    struct WriterVel {
        order: Arc<Mutex<[u32; 32]>>,
        step: Arc<AtomicU32>,
    }
    impl System for WriterVel {
        fn run(&self, _: &Frame) {
            let s = self.step.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
            self.order.lock().unwrap()[s as usize] = 410;
        }
        fn writes(&self) -> &'static [TypeId] {
            static W: &[TypeId] = &[TypeId::of::<Velocity>()];
            W
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }
    struct ReaderPosOnly {
        order: Arc<Mutex<[u32; 32]>>,
        step: Arc<AtomicU32>,
    }
    impl System for ReaderPosOnly {
        fn run(&self, _: &Frame) {
            let s = self.step.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
            self.order.lock().unwrap()[s as usize] = 420;
        }
        fn reads(&self) -> &'static [TypeId] {
            static R: &[TypeId] = &[TypeId::of::<Position>()];
            R
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }
    world.scheduler_mut().add_system(WriterVel {
        order: order.clone(),
        step: step.clone(),
    });
    world.scheduler_mut().add_system(ReaderPosOnly {
        order: order.clone(),
        step: step.clone(),
    });
    world.scheduler_mut().build_wavefronts();
    world.run();
}
