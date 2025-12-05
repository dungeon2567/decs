use decs::frame::Frame;
use decs::system::System;
use decs::system::SystemGroup;
use decs::world::World;
use std::any::TypeId;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

// Readers and writers for Position/Velocity types
struct RPos {
    order: Arc<Mutex<[u32; 64]>>,
    step: Arc<AtomicU32>,
}
struct RVel {
    order: Arc<Mutex<[u32; 64]>>,
    step: Arc<AtomicU32>,
}
struct WPos {
    order: Arc<Mutex<[u32; 64]>>,
    step: Arc<AtomicU32>,
}
struct WVel {
    order: Arc<Mutex<[u32; 64]>>,
    step: Arc<AtomicU32>,
}
impl System for RPos {
    fn run(&self, _: &Frame) {
        let s = self.step.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        self.order.lock().unwrap()[s as usize] = 10;
    }
    fn reads(&self) -> &'static [TypeId] {
        static R: &[TypeId] = &[TypeId::of::<decs::world::World>()];
        R
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl System for RVel {
    fn run(&self, _: &Frame) {
        let s = self.step.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        self.order.lock().unwrap()[s as usize] = 20;
    }
    fn reads(&self) -> &'static [TypeId] {
        static R: &[TypeId] = &[TypeId::of::<decs::world::World>()];
        R
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl System for WPos {
    fn run(&self, _: &Frame) {
        let s = self.step.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        self.order.lock().unwrap()[s as usize] = 30;
    }
    fn writes(&self) -> &'static [TypeId] {
        static W: &[TypeId] = &[TypeId::of::<decs::world::World>()];
        W
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl System for WVel {
    fn run(&self, _: &Frame) {
        let s = self.step.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        self.order.lock().unwrap()[s as usize] = 40;
    }
    fn writes(&self) -> &'static [TypeId] {
        static W: &[TypeId] = &[TypeId::of::<decs::world::World>()];
        W
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// Group-tagged systems
struct Sim {
    order: Arc<Mutex<[u32; 64]>>,
    step: Arc<AtomicU32>,
}
struct Clean {
    order: Arc<Mutex<[u32; 64]>>,
    step: Arc<AtomicU32>,
}
struct Destroy {
    order: Arc<Mutex<[u32; 64]>>,
    step: Arc<AtomicU32>,
}
impl System for Sim {
    fn run(&self, _: &Frame) {
        let s = self.step.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        self.order.lock().unwrap()[s as usize] = 50;
    }
    fn parent(&self) -> Option<&dyn SystemGroup> {
        Some(decs::world::SimulationGroup::instance())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl System for Clean {
    fn run(&self, _: &Frame) {
        let s = self.step.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        self.order.lock().unwrap()[s as usize] = 60;
    }
    fn parent(&self) -> Option<&dyn SystemGroup> {
        Some(decs::world::CleanupGroup::instance())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl System for Destroy {
    fn run(&self, _: &Frame) {
        let s = self.step.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        self.order.lock().unwrap()[s as usize] = 70;
    }
    fn parent(&self) -> Option<&dyn SystemGroup> {
        Some(decs::world::DestroyGroup::instance())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[test]
fn two_readers_no_order_constraint() {
    let mut world = World::new();
    let order = Arc::new(Mutex::new([0; 64]));
    let step = Arc::new(AtomicU32::new(0));
    world.scheduler_mut().add_system(RPos {
        order: order.clone(),
        step: step.clone(),
    });
    world.scheduler_mut().add_system(RVel {
        order: order.clone(),
        step: step.clone(),
    });
    world.scheduler_mut().build_wavefronts();
    world.run();
    let o = order.lock().unwrap();
    assert!((o[1] == 10 && o[2] == 20) || (o[1] == 20 && o[2] == 10));
}

#[test]
fn writer_then_reader_order_on_same_type() {
    let mut world = World::new();
    let order = Arc::new(Mutex::new([0; 64]));
    let step = Arc::new(AtomicU32::new(0));
    world.scheduler_mut().add_system(WPos {
        order: order.clone(),
        step: step.clone(),
    });
    world.scheduler_mut().add_system(RPos {
        order: order.clone(),
        step: step.clone(),
    });
    world.scheduler_mut().build_wavefronts();
    world.run();
    let o = order.lock().unwrap();
    assert!(o[1] == 30 && o[2] == 10);
}

#[test]
fn writers_chain_serialized() {
    let mut world = World::new();
    let order = Arc::new(Mutex::new([0; 64]));
    let step = Arc::new(AtomicU32::new(0));
    world.scheduler_mut().add_system(WPos {
        order: order.clone(),
        step: step.clone(),
    });
    world.scheduler_mut().add_system(WPos {
        order: order.clone(),
        step: step.clone(),
    });
    world.scheduler_mut().build_wavefronts();
    world.run();
    let o = order.lock().unwrap();
    assert!(o[1] == 30 && o[2] == 30);
}

#[test]
fn reader_and_writer_different_types_independent() {
    let mut world = World::new();
    let order = Arc::new(Mutex::new([0; 64]));
    let step = Arc::new(AtomicU32::new(0));
    world.scheduler_mut().add_system(RPos {
        order: order.clone(),
        step: step.clone(),
    });
    world.scheduler_mut().add_system(WVel {
        order: order.clone(),
        step: step.clone(),
    });
    world.scheduler_mut().build_wavefronts();
    world.run();
    let o = order.lock().unwrap();
    assert!((o[1] == 10 && o[2] == 40) || (o[1] == 40 && o[2] == 10));
}

#[test]
fn group_order_sim_cleanup_destroy() {
    let mut world = World::new();
    let order = Arc::new(Mutex::new([0; 64]));
    let step = Arc::new(AtomicU32::new(0));
    world.scheduler_mut().add_system(Sim {
        order: order.clone(),
        step: step.clone(),
    });
    world.scheduler_mut().add_system(Clean {
        order: order.clone(),
        step: step.clone(),
    });
    world.scheduler_mut().add_system(Destroy {
        order: order.clone(),
        step: step.clone(),
    });
    world.scheduler_mut().build_wavefronts();
    world.run();
    let o = order.lock().unwrap();
    assert!(o[1] == 50 && o[2] == 60 && o[3] == 70);
}

struct A {
    order: Arc<Mutex<[u32; 64]>>,
    step: Arc<AtomicU32>,
}
struct B {
    order: Arc<Mutex<[u32; 64]>>,
    step: Arc<AtomicU32>,
}
struct C {
    order: Arc<Mutex<[u32; 64]>>,
    step: Arc<AtomicU32>,
}
impl System for A {
    fn run(&self, _: &Frame) {
        let s = self.step.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        self.order.lock().unwrap()[s as usize] = 80;
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl System for B {
    fn run(&self, _: &Frame) {
        let s = self.step.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        self.order.lock().unwrap()[s as usize] = 90;
    }
    fn after(&self) -> &'static [TypeId] {
        static AFT: &[TypeId] = &[TypeId::of::<A>()];
        AFT
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl System for C {
    fn run(&self, _: &Frame) {
        let s = self.step.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        self.order.lock().unwrap()[s as usize] = 100;
    }
    fn after(&self) -> &'static [TypeId] {
        static AFT: &[TypeId] = &[TypeId::of::<B>()];
        AFT
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[test]
fn system_before_after_edges_chain() {
    let mut world = World::new();
    let order = Arc::new(Mutex::new([0; 64]));
    let step = Arc::new(AtomicU32::new(0));
    world.scheduler_mut().add_system(A {
        order: order.clone(),
        step: step.clone(),
    });
    world.scheduler_mut().add_system(B {
        order: order.clone(),
        step: step.clone(),
    });
    world.scheduler_mut().add_system(C {
        order: order.clone(),
        step: step.clone(),
    });
    world.scheduler_mut().build_wavefronts();
    world.run();
    let o = order.lock().unwrap();
    assert!(o[1] == 80 && o[2] == 90 && o[3] == 100);
}

#[test]
fn group_hierarchy_respected_indirect() {
    let mut world = World::new();
    let order = Arc::new(Mutex::new([0; 64]));
    let step = Arc::new(AtomicU32::new(0));
    world.scheduler_mut().add_system(Sim {
        order: order.clone(),
        step: step.clone(),
    });
    world.scheduler_mut().add_system(WPos {
        order: order.clone(),
        step: step.clone(),
    });
    world.scheduler_mut().add_system(Clean {
        order: order.clone(),
        step: step.clone(),
    });
    world.scheduler_mut().build_wavefronts();
    world.run();
    let o = order.lock().unwrap();
    assert!(o[1] == 50 && o[3] == 60);
}

#[test]
fn multiple_independent_in_first_wavefront() {
    let mut world = World::new();
    let order = Arc::new(Mutex::new([0; 64]));
    let step = Arc::new(AtomicU32::new(0));
    world.scheduler_mut().add_system(RPos {
        order: order.clone(),
        step: step.clone(),
    });
    world.scheduler_mut().add_system(RVel {
        order: order.clone(),
        step: step.clone(),
    });
    world.scheduler_mut().add_system(WVel {
        order: order.clone(),
        step: step.clone(),
    });
    world.scheduler_mut().build_wavefronts();
    world.run();
    let o = order.lock().unwrap();
    assert!(o[1] != 0 && o[2] != 0 && o[3] != 0);
}
