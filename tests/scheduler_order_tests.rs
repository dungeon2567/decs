use decs::scheduler::Scheduler;
use decs::system::{System, SystemGroup};
use decs_macros::Component;
use std::any::TypeId;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Component)]
struct C1;

#[derive(Clone, Copy, Component)]
struct C2;

#[derive(Clone)]
struct Recorder(Arc<Mutex<Vec<&'static str>>>);

struct S1;
struct S2;

impl System for S1 {
    fn run(&self, _frame: &decs::frame::Frame) {}
    fn before(&self) -> &[TypeId] { static B: &[TypeId] = &[TypeId::of::<S2>()]; B }
    fn as_any(&self) -> &dyn std::any::Any { self }
}

impl System for S2 {
    fn run(&self, _frame: &decs::frame::Frame) {}
    fn as_any(&self) -> &dyn std::any::Any { self }
}

struct Writer;
struct Reader;

impl System for Writer {
    fn run(&self, _frame: &decs::frame::Frame) {}
    fn writes(&self) -> &[TypeId] { static W: &[TypeId] = &[TypeId::of::<C1>()]; W }
    fn as_any(&self) -> &dyn std::any::Any { self }
}

impl System for Reader {
    fn run(&self, _frame: &decs::frame::Frame) {}
    fn reads(&self) -> &[TypeId] { static R: &[TypeId] = &[TypeId::of::<C1>()]; R }
    fn as_any(&self) -> &dyn std::any::Any { self }
}

struct SP;
struct SR;
struct SA;
struct SB;

decs_macros::system_group!(G { Before=[SR], After=[SP] });

impl System for SP {
    fn run(&self, _frame: &decs::frame::Frame) {}
    fn as_any(&self) -> &dyn std::any::Any { self }
}

impl System for SR {
    fn run(&self, _frame: &decs::frame::Frame) {}
    fn as_any(&self) -> &dyn std::any::Any { self }
}

impl System for SA {
    fn run(&self, _frame: &decs::frame::Frame) {}
    fn parent(&self) -> Option<&dyn SystemGroup> { Some(G::instance()) }
    fn as_any(&self) -> &dyn std::any::Any { self }
}

impl System for SB {
    fn run(&self, _frame: &decs::frame::Frame) {}
    fn parent(&self) -> Option<&dyn SystemGroup> { Some(G::instance()) }
    fn as_any(&self) -> &dyn std::any::Any { self }
}

#[test]
fn scheduler_respects_before_after() {
    let mut s = Scheduler::new();
    s.add_system(S2);
    s.add_system(S1);
    s.build_wavefronts();
    let w = s.wavefronts();
    let pos = |id: usize| w.iter().position(|lvl| lvl.contains(&id)).unwrap();
    assert!(pos(1) < pos(0));
}

#[test]
fn scheduler_respects_write_read() {
    let mut s = Scheduler::new();
    s.add_system(Reader);
    s.add_system(Writer);
    s.build_wavefronts();
    let w = s.wavefronts();
    let pos = |id: usize| w.iter().position(|lvl| lvl.contains(&id)).unwrap();
    assert!(pos(1) < pos(0));
    assert_ne!(pos(1), pos(0));
}

#[test]
fn scheduler_respects_group_inheritance() {
    let mut s = Scheduler::new();
    s.add_system(SR);
    s.add_system(SA);
    s.add_system(SB);
    s.add_system(SP);
    s.build_wavefronts();
    let w = s.wavefronts();
    let pos = |id: usize| w.iter().position(|lvl| lvl.contains(&id)).unwrap();
    assert!(pos(3) < pos(1));
    assert!(pos(3) < pos(2));
    assert!(pos(1) < pos(0));
    assert!(pos(2) < pos(0));
}

struct R1;
struct R2;
impl System for R1 { fn run(&self, _frame: &decs::frame::Frame) {} fn reads(&self) -> &[TypeId] { static R: &[TypeId] = &[TypeId::of::<C1>()]; R } fn as_any(&self) -> &dyn std::any::Any { self } }
impl System for R2 { fn run(&self, _frame: &decs::frame::Frame) {} fn reads(&self) -> &[TypeId] { static R: &[TypeId] = &[TypeId::of::<C1>()]; R } fn as_any(&self) -> &dyn std::any::Any { self } }

struct W1;
struct W2;
impl System for W1 { fn run(&self, _frame: &decs::frame::Frame) {} fn writes(&self) -> &[TypeId] { static W: &[TypeId] = &[TypeId::of::<C1>()]; W } fn as_any(&self) -> &dyn std::any::Any { self } }
impl System for W2 { fn run(&self, _frame: &decs::frame::Frame) {} fn writes(&self) -> &[TypeId] { static W: &[TypeId] = &[TypeId::of::<C1>()]; W } fn as_any(&self) -> &dyn std::any::Any { self } }

#[test]
fn readers_share_wavefront() {
    let mut s = Scheduler::new();
    s.add_system(R1);
    s.add_system(R2);
    s.build_wavefronts();
    let w = s.wavefronts();
    let pos = |id: usize| w.iter().position(|lvl| lvl.contains(&id)).unwrap();
    assert_eq!(pos(0), pos(1));
}

#[test]
fn writers_separated_wavefronts() {
    let mut s = Scheduler::new();
    s.add_system(W1);
    s.add_system(W2);
    s.build_wavefronts();
    let w = s.wavefronts();
    let pos = |id: usize| w.iter().position(|lvl| lvl.contains(&id)).unwrap();
    assert!(pos(0) < pos(1));
    assert_ne!(pos(0), pos(1));
}

#[test]
fn after_chain_levels() {
    struct A; struct B; struct C;
    impl System for A { fn run(&self, _frame: &decs::frame::Frame) {} fn after(&self) -> &[TypeId] { static AFT: &[TypeId] = &[TypeId::of::<B>()]; AFT } fn as_any(&self) -> &dyn std::any::Any { self } }
    impl System for B { fn run(&self, _frame: &decs::frame::Frame) {} fn after(&self) -> &[TypeId] { static AFT: &[TypeId] = &[TypeId::of::<C>()]; AFT } fn as_any(&self) -> &dyn std::any::Any { self } }
    impl System for C { fn run(&self, _frame: &decs::frame::Frame) {} fn as_any(&self) -> &dyn std::any::Any { self } }
    let mut s = Scheduler::new();
    s.add_system(A);
    s.add_system(B);
    s.add_system(C);
    s.build_wavefronts();
    let w = s.wavefronts();
    let pos = |id: usize| w.iter().position(|lvl| lvl.contains(&id)).unwrap();
    assert!(pos(2) < pos(1));
    assert!(pos(1) < pos(0));
}

#[test]
fn before_chain_levels() {
    struct A; struct B; struct C;
    impl System for A { fn run(&self, _frame: &decs::frame::Frame) {} fn before(&self) -> &[TypeId] { static BF: &[TypeId] = &[TypeId::of::<B>()]; BF } fn as_any(&self) -> &dyn std::any::Any { self } }
    impl System for B { fn run(&self, _frame: &decs::frame::Frame) {} fn before(&self) -> &[TypeId] { static BF: &[TypeId] = &[TypeId::of::<C>()]; BF } fn as_any(&self) -> &dyn std::any::Any { self } }
    impl System for C { fn run(&self, _frame: &decs::frame::Frame) {} fn as_any(&self) -> &dyn std::any::Any { self } }
    let mut s = Scheduler::new();
    s.add_system(A);
    s.add_system(B);
    s.add_system(C);
    s.build_wavefronts();
    let w = s.wavefronts();
    let pos = |id: usize| w.iter().position(|lvl| lvl.contains(&id)).unwrap();
    assert!(pos(0) < pos(1));
    assert!(pos(1) < pos(2));
}

#[test]
fn nested_group_constraints() {
    decs_macros::system_group!(PG { After=[SP2] });
    decs_macros::system_group!(CG { Before=[SR2], Parent=PG });
    struct SA2; struct SB2; struct SP2; struct SR2;
    impl System for SA2 { fn run(&self, _frame: &decs::frame::Frame) {} fn parent(&self) -> Option<&dyn SystemGroup> { Some(CG::instance()) } fn as_any(&self) -> &dyn std::any::Any { self } }
    impl System for SB2 { fn run(&self, _frame: &decs::frame::Frame) {} fn parent(&self) -> Option<&dyn SystemGroup> { Some(CG::instance()) } fn as_any(&self) -> &dyn std::any::Any { self } }
    impl System for SP2 { fn run(&self, _frame: &decs::frame::Frame) {} fn as_any(&self) -> &dyn std::any::Any { self } }
    impl System for SR2 { fn run(&self, _frame: &decs::frame::Frame) {} fn as_any(&self) -> &dyn std::any::Any { self } }
    let mut s = Scheduler::new();
    s.add_system(SR2);
    s.add_system(SA2);
    s.add_system(SB2);
    s.add_system(SP2);
    s.build_wavefronts();
    let w = s.wavefronts();
    let pos = |id: usize| w.iter().position(|lvl| lvl.contains(&id)).unwrap();
    assert!(pos(3) < pos(1));
    assert!(pos(3) < pos(2));
    assert!(pos(1) < pos(0));
    assert!(pos(2) < pos(0));
}
