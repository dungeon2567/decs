use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use decs::component::Component;
use decs_macros::Component;
use decs::storage::Storage;
use decs::tick::Tick;
use decs::entity::Entity;
use decs::world::World;
use decs::ecs::Ecs;
use decs::frame::Frame;

#[derive(Clone, Component)]
struct BenchComp {
    v: i32,
}

fn bench_full_pipeline_20k(c: &mut Criterion) {
    c.bench_function("full_pipeline_20k", |b| {
        b.iter(|| {
            static INIT: std::sync::Once = std::sync::Once::new();
            INIT.call_once(|| {
                Ecs::register::<BenchComp>();
                Ecs::register::<Entity>();
            });
            let mut storage = Storage::<BenchComp>::new();
            let mut frame = Frame::new(Tick(0));
            let n = 20_000u32;

            // Tick 1: baseline
            frame.current_tick = Tick(1);
            for id in 0..n {
                storage.set(&frame, id, BenchComp { v: 1 });
            }
            storage.clear_changed_masks();

            // Tick 23: change every 3rd
            frame.current_tick = Tick(23);
            let mut i = 0u32;
            while i < n {
                storage.set(&frame, i, BenchComp { v: 2 });
                i += 3;
            }
            storage.clear_changed_masks();

            // Tick 38: remove every 5th
            frame.current_tick = Tick(38);
            let mut j = 0u32;
            while j < n {
                let _ = storage.remove(&frame, j);
                j += 5;
            }
            storage.clear_changed_masks();

            // Tick 30: create after-target new items
            frame.current_tick = Tick(30);
            for id in n..(n + 1000) {
                storage.set(&frame, id, BenchComp { v: 99 });
            }
            storage.clear_changed_masks();

            // Tick 40: rollback to Tick(2)
            frame.current_tick = Tick(40);
            storage.rollback(Tick(2));

            // Spawn entities (20k)
            let mut entity_storage = Storage::<Entity>::new();
            let frame = Frame::new(Tick(0));
            for _ in 0..20_000u32 {
                let _ = entity_storage.spawn(&frame);
            }
        });
    });
}

#[derive(Clone, Component)]
struct L0 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L1 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L2 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L3 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L4 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L5 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L6 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L7 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L8 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L9 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L10 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L11 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L12 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L13 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L14 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L15 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L16 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L17 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L18 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L19 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L20 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L21 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L22 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L23 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L24 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L25 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L26 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L27 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L28 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L29 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L30 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L31 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L32 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L33 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L34 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L35 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L36 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L37 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L38 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L39 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L40 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L41 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L42 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L43 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L44 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L45 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L46 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L47 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L48 { d: [u8; 64] }
#[derive(Clone, Component)]
struct L49 { d: [u8; 64] }

#[derive(Clone, Component)]
struct S0 { v: u32 }
#[derive(Clone, Component)]
struct S1 { v: u32 }
#[derive(Clone, Component)]
struct S2 { v: u32 }
#[derive(Clone, Component)]
struct S3 { v: u32 }
#[derive(Clone, Component)]
struct S4 { v: u32 }
#[derive(Clone, Component)]
struct S5 { v: u32 }
#[derive(Clone, Component)]
struct S6 { v: u32 }
#[derive(Clone, Component)]
struct S7 { v: u32 }
#[derive(Clone, Component)]
struct S8 { v: u32 }
#[derive(Clone, Component)]
struct S9 { v: u32 }
#[derive(Clone, Component)]
struct S10 { v: u32 }
#[derive(Clone, Component)]
struct S11 { v: u32 }
#[derive(Clone, Component)]
struct S12 { v: u32 }
#[derive(Clone, Component)]
struct S13 { v: u32 }
#[derive(Clone, Component)]
struct S14 { v: u32 }
#[derive(Clone, Component)]
struct S15 { v: u32 }
#[derive(Clone, Component)]
struct S16 { v: u32 }
#[derive(Clone, Component)]
struct S17 { v: u32 }
#[derive(Clone, Component)]
struct S18 { v: u32 }
#[derive(Clone, Component)]
struct S19 { v: u32 }
#[derive(Clone, Component)]
struct S20 { v: u32 }
#[derive(Clone, Component)]
struct S21 { v: u32 }
#[derive(Clone, Component)]
struct S22 { v: u32 }
#[derive(Clone, Component)]
struct S23 { v: u32 }
#[derive(Clone, Component)]
struct S24 { v: u32 }
#[derive(Clone, Component)]
struct S25 { v: u32 }
#[derive(Clone, Component)]
struct S26 { v: u32 }
#[derive(Clone, Component)]
struct S27 { v: u32 }
#[derive(Clone, Component)]
struct S28 { v: u32 }
#[derive(Clone, Component)]
struct S29 { v: u32 }
#[derive(Clone, Component)]
struct S30 { v: u32 }
#[derive(Clone, Component)]
struct S31 { v: u32 }
#[derive(Clone, Component)]
struct S32 { v: u32 }
#[derive(Clone, Component)]
struct S33 { v: u32 }
#[derive(Clone, Component)]
struct S34 { v: u32 }
#[derive(Clone, Component)]
struct S35 { v: u32 }
#[derive(Clone, Component)]
struct S36 { v: u32 }
#[derive(Clone, Component)]
struct S37 { v: u32 }
#[derive(Clone, Component)]
struct S38 { v: u32 }
#[derive(Clone, Component)]
struct S39 { v: u32 }
#[derive(Clone, Component)]
struct S40 { v: u32 }
#[derive(Clone, Component)]
struct S41 { v: u32 }
#[derive(Clone, Component)]
struct S42 { v: u32 }
#[derive(Clone, Component)]
struct S43 { v: u32 }
#[derive(Clone, Component)]
struct S44 { v: u32 }
#[derive(Clone, Component)]
struct S45 { v: u32 }
#[derive(Clone, Component)]
struct S46 { v: u32 }
#[derive(Clone, Component)]
struct S47 { v: u32 }
#[derive(Clone, Component)]
struct S48 { v: u32 }
#[derive(Clone, Component)]
struct S49 { v: u32 }


#[derive(Clone, Component)] struct Tag0; #[derive(Clone, Component)] struct Tag1; #[derive(Clone, Component)] struct Tag2; #[derive(Clone, Component)] struct Tag3; #[derive(Clone, Component)] struct Tag4;
#[derive(Clone, Component)] struct Tag5; #[derive(Clone, Component)] struct Tag6; #[derive(Clone, Component)] struct Tag7; #[derive(Clone, Component)] struct Tag8; #[derive(Clone, Component)] struct Tag9;
#[derive(Clone, Component)] struct Tag10; #[derive(Clone, Component)] struct Tag11; #[derive(Clone, Component)] struct Tag12; #[derive(Clone, Component)] struct Tag13; #[derive(Clone, Component)] struct Tag14;
#[derive(Clone, Component)] struct Tag15; #[derive(Clone, Component)] struct Tag16; #[derive(Clone, Component)] struct Tag17; #[derive(Clone, Component)] struct Tag18; #[derive(Clone, Component)] struct Tag19;
#[derive(Clone, Component)] struct Tag20; #[derive(Clone, Component)] struct Tag21; #[derive(Clone, Component)] struct Tag22; #[derive(Clone, Component)] struct Tag23; #[derive(Clone, Component)] struct Tag24;
#[derive(Clone, Component)] struct Tag25; #[derive(Clone, Component)] struct Tag26; #[derive(Clone, Component)] struct Tag27; #[derive(Clone, Component)] struct Tag28; #[derive(Clone, Component)] struct Tag29;
#[derive(Clone, Component)] struct Tag30; #[derive(Clone, Component)] struct Tag31; #[derive(Clone, Component)] struct Tag32; #[derive(Clone, Component)] struct Tag33; #[derive(Clone, Component)] struct Tag34;
#[derive(Clone, Component)] struct Tag35; #[derive(Clone, Component)] struct Tag36; #[derive(Clone, Component)] struct Tag37; #[derive(Clone, Component)] struct Tag38; #[derive(Clone, Component)] struct Tag39;
#[derive(Clone, Component)] struct Tag40; #[derive(Clone, Component)] struct Tag41; #[derive(Clone, Component)] struct Tag42; #[derive(Clone, Component)] struct Tag43; #[derive(Clone, Component)] struct Tag44;
#[derive(Clone, Component)] struct Tag45; #[derive(Clone, Component)] struct Tag46; #[derive(Clone, Component)] struct Tag47; #[derive(Clone, Component)] struct Tag48; #[derive(Clone, Component)] struct Tag49;

 

fn bench_perf_reference_like(c: &mut Criterion) {
    c.bench_function("perf_reference_like", |b| {
        b.iter(|| {
            // Register component IDs once
            static INIT: std::sync::Once = std::sync::Once::new();
            INIT.call_once(|| {
                Ecs::register::<L0>(); Ecs::register::<L1>(); Ecs::register::<L2>(); Ecs::register::<L3>(); Ecs::register::<L4>();
                Ecs::register::<L5>(); Ecs::register::<L6>(); Ecs::register::<L7>(); Ecs::register::<L8>(); Ecs::register::<L9>();
                Ecs::register::<L10>(); Ecs::register::<L11>(); Ecs::register::<L12>(); Ecs::register::<L13>(); Ecs::register::<L14>();
                Ecs::register::<L15>(); Ecs::register::<L16>(); Ecs::register::<L17>(); Ecs::register::<L18>(); Ecs::register::<L19>();
                Ecs::register::<L20>(); Ecs::register::<L21>(); Ecs::register::<L22>(); Ecs::register::<L23>(); Ecs::register::<L24>();
                Ecs::register::<L25>(); Ecs::register::<L26>(); Ecs::register::<L27>(); Ecs::register::<L28>(); Ecs::register::<L29>();
                Ecs::register::<L30>(); Ecs::register::<L31>(); Ecs::register::<L32>(); Ecs::register::<L33>(); Ecs::register::<L34>();
                Ecs::register::<L35>(); Ecs::register::<L36>(); Ecs::register::<L37>(); Ecs::register::<L38>(); Ecs::register::<L39>();
                Ecs::register::<L40>(); Ecs::register::<L41>(); Ecs::register::<L42>(); Ecs::register::<L43>(); Ecs::register::<L44>();
                Ecs::register::<L45>(); Ecs::register::<L46>(); Ecs::register::<L47>(); Ecs::register::<L48>(); Ecs::register::<L49>();
                Ecs::register::<S0>(); Ecs::register::<S1>(); Ecs::register::<S2>(); Ecs::register::<S3>(); Ecs::register::<S4>();
                Ecs::register::<S5>(); Ecs::register::<S6>(); Ecs::register::<S7>(); Ecs::register::<S8>(); Ecs::register::<S9>();
                Ecs::register::<S10>(); Ecs::register::<S11>(); Ecs::register::<S12>(); Ecs::register::<S13>(); Ecs::register::<S14>();
                Ecs::register::<S15>(); Ecs::register::<S16>(); Ecs::register::<S17>(); Ecs::register::<S18>(); Ecs::register::<S19>();
                Ecs::register::<S20>(); Ecs::register::<S21>(); Ecs::register::<S22>(); Ecs::register::<S23>(); Ecs::register::<S24>();
                Ecs::register::<S25>(); Ecs::register::<S26>(); Ecs::register::<S27>(); Ecs::register::<S28>(); Ecs::register::<S29>();
                Ecs::register::<S30>(); Ecs::register::<S31>(); Ecs::register::<S32>(); Ecs::register::<S33>(); Ecs::register::<S34>();
                Ecs::register::<S35>(); Ecs::register::<S36>(); Ecs::register::<S37>(); Ecs::register::<S38>(); Ecs::register::<S39>();
                Ecs::register::<S40>(); Ecs::register::<S41>(); Ecs::register::<S42>(); Ecs::register::<S43>(); Ecs::register::<S44>();
                Ecs::register::<S45>(); Ecs::register::<S46>(); Ecs::register::<S47>(); Ecs::register::<S48>(); Ecs::register::<S49>();
            });
            let mut world = World::new();
            let ids: [u32; 100] = {
                [
                    L0::id(),L1::id(),L2::id(),L3::id(),L4::id(),L5::id(),L6::id(),L7::id(),L8::id(),L9::id(),
                    L10::id(),L11::id(),L12::id(),L13::id(),L14::id(),L15::id(),L16::id(),L17::id(),L18::id(),L19::id(),
                    L20::id(),L21::id(),L22::id(),L23::id(),L24::id(),L25::id(),L26::id(),L27::id(),L28::id(),L29::id(),
                    L30::id(),L31::id(),L32::id(),L33::id(),L34::id(),L35::id(),L36::id(),L37::id(),L38::id(),L39::id(),
                    L40::id(),L41::id(),L42::id(),L43::id(),L44::id(),L45::id(),L46::id(),L47::id(),L48::id(),L49::id(),
                    S0::id(),S1::id(),S2::id(),S3::id(),S4::id(),S5::id(),S6::id(),S7::id(),S8::id(),S9::id(),
                    S10::id(),S11::id(),S12::id(),S13::id(),S14::id(),S15::id(),S16::id(),S17::id(),S18::id(),S19::id(),
                    S20::id(),S21::id(),S22::id(),S23::id(),S24::id(),S25::id(),S26::id(),S27::id(),S28::id(),S29::id(),
                    S30::id(),S31::id(),S32::id(),S33::id(),S34::id(),S35::id(),S36::id(),S37::id(),S38::id(),S39::id(),
                    S40::id(),S41::id(),S42::id(),S43::id(),S44::id(),S45::id(),S46::id(),S47::id(),S48::id(),S49::id(),
                ]
            };

            // Pre-create storages
            {
                let _ = world.get_storage::<L0>(); let _ = world.get_storage::<L1>(); let _ = world.get_storage::<L2>(); let _ = world.get_storage::<L3>();
                let _ = world.get_storage::<L4>(); let _ = world.get_storage::<L5>(); let _ = world.get_storage::<L6>(); let _ = world.get_storage::<L7>();
                let _ = world.get_storage::<L8>(); let _ = world.get_storage::<L9>(); let _ = world.get_storage::<L10>(); let _ = world.get_storage::<L11>();
                let _ = world.get_storage::<L12>(); let _ = world.get_storage::<L13>(); let _ = world.get_storage::<L14>(); let _ = world.get_storage::<L15>();
                let _ = world.get_storage::<L16>(); let _ = world.get_storage::<L17>(); let _ = world.get_storage::<L18>(); let _ = world.get_storage::<L19>();
                let _ = world.get_storage::<L20>(); let _ = world.get_storage::<L21>(); let _ = world.get_storage::<L22>(); let _ = world.get_storage::<L23>();
                let _ = world.get_storage::<L24>(); let _ = world.get_storage::<L25>(); let _ = world.get_storage::<L26>(); let _ = world.get_storage::<L27>();
                let _ = world.get_storage::<L28>(); let _ = world.get_storage::<L29>(); let _ = world.get_storage::<L30>(); let _ = world.get_storage::<L31>();
                let _ = world.get_storage::<L32>(); let _ = world.get_storage::<L33>(); let _ = world.get_storage::<L34>(); let _ = world.get_storage::<L35>();
                let _ = world.get_storage::<L36>(); let _ = world.get_storage::<L37>(); let _ = world.get_storage::<L38>(); let _ = world.get_storage::<L39>();
                let _ = world.get_storage::<L40>(); let _ = world.get_storage::<L41>(); let _ = world.get_storage::<L42>(); let _ = world.get_storage::<L43>();
                let _ = world.get_storage::<L44>(); let _ = world.get_storage::<L45>(); let _ = world.get_storage::<L46>(); let _ = world.get_storage::<L47>();
                let _ = world.get_storage::<L48>(); let _ = world.get_storage::<L49>();
                let _ = world.get_storage::<S0>(); let _ = world.get_storage::<S1>(); let _ = world.get_storage::<S2>(); let _ = world.get_storage::<S3>();
                let _ = world.get_storage::<S4>(); let _ = world.get_storage::<S5>(); let _ = world.get_storage::<S6>(); let _ = world.get_storage::<S7>();
                let _ = world.get_storage::<S8>(); let _ = world.get_storage::<S9>(); let _ = world.get_storage::<S10>(); let _ = world.get_storage::<S11>();
                let _ = world.get_storage::<S12>(); let _ = world.get_storage::<S13>(); let _ = world.get_storage::<S14>(); let _ = world.get_storage::<S15>();
                let _ = world.get_storage::<S16>(); let _ = world.get_storage::<S17>(); let _ = world.get_storage::<S18>(); let _ = world.get_storage::<S19>();
                let _ = world.get_storage::<S20>(); let _ = world.get_storage::<S21>(); let _ = world.get_storage::<S22>(); let _ = world.get_storage::<S23>();
                let _ = world.get_storage::<S24>(); let _ = world.get_storage::<S25>(); let _ = world.get_storage::<S26>(); let _ = world.get_storage::<S27>();
                let _ = world.get_storage::<S28>(); let _ = world.get_storage::<S29>(); let _ = world.get_storage::<S30>(); let _ = world.get_storage::<S31>();
                let _ = world.get_storage::<S32>(); let _ = world.get_storage::<S33>(); let _ = world.get_storage::<S34>(); let _ = world.get_storage::<S35>();
                let _ = world.get_storage::<S36>(); let _ = world.get_storage::<S37>(); let _ = world.get_storage::<S38>(); let _ = world.get_storage::<S39>();
                let _ = world.get_storage::<S40>(); let _ = world.get_storage::<S41>(); let _ = world.get_storage::<S42>(); let _ = world.get_storage::<S43>();
                let _ = world.get_storage::<S44>(); let _ = world.get_storage::<S45>(); let _ = world.get_storage::<S46>(); let _ = world.get_storage::<S47>();
                let _ = world.get_storage::<S48>(); let _ = world.get_storage::<S49>();
            }

            let n_entities = 1000u32;
            for pass in 0..24u32 {
                // advance tick per pass
                world.set_tick(Tick(pass));
                // write components for all entities
                let frame = Frame::new(world.current_tick());
                for id in 0..n_entities {
                    macro_rules! set_all {
                        ($ty:ty, $val:expr) => {{
                            let s = unsafe { &mut *world.get_storage::<$ty>() };
                            s.set(&frame, id, $val);
                        }};
                    }
                    set_all!(L0, L0{ d:[pass as u8;64] }); set_all!(L1, L1{ d:[pass as u8;64] });
                    set_all!(L2, L2{ d:[pass as u8;64] }); set_all!(L3, L3{ d:[pass as u8;64] });
                    set_all!(L4, L4{ d:[pass as u8;64] }); set_all!(L5, L5{ d:[pass as u8;64] });
                    set_all!(L6, L6{ d:[pass as u8;64] }); set_all!(L7, L7{ d:[pass as u8;64] });
                    set_all!(L8, L8{ d:[pass as u8;64] }); set_all!(L9, L9{ d:[pass as u8;64] });
                    set_all!(L10, L10{ d:[pass as u8;64] }); set_all!(L11, L11{ d:[pass as u8;64] });
                    set_all!(L12, L12{ d:[pass as u8;64] }); set_all!(L13, L13{ d:[pass as u8;64] });
                    set_all!(L14, L14{ d:[pass as u8;64] }); set_all!(L15, L15{ d:[pass as u8;64] });
                    set_all!(L16, L16{ d:[pass as u8;64] }); set_all!(L17, L17{ d:[pass as u8;64] });
                    set_all!(L18, L18{ d:[pass as u8;64] }); set_all!(L19, L19{ d:[pass as u8;64] });
                    set_all!(L20, L20{ d:[pass as u8;64] }); set_all!(L21, L21{ d:[pass as u8;64] });
                    set_all!(L22, L22{ d:[pass as u8;64] }); set_all!(L23, L23{ d:[pass as u8;64] });
                    set_all!(L24, L24{ d:[pass as u8;64] }); set_all!(L25, L25{ d:[pass as u8;64] });
                    set_all!(L26, L26{ d:[pass as u8;64] }); set_all!(L27, L27{ d:[pass as u8;64] });
                    set_all!(L28, L28{ d:[pass as u8;64] }); set_all!(L29, L29{ d:[pass as u8;64] });
                    set_all!(L30, L30{ d:[pass as u8;64] }); set_all!(L31, L31{ d:[pass as u8;64] });
                    set_all!(L32, L32{ d:[pass as u8;64] }); set_all!(L33, L33{ d:[pass as u8;64] });
                    set_all!(L34, L34{ d:[pass as u8;64] }); set_all!(L35, L35{ d:[pass as u8;64] });
                    set_all!(L36, L36{ d:[pass as u8;64] }); set_all!(L37, L37{ d:[pass as u8;64] });
                    set_all!(L38, L38{ d:[pass as u8;64] }); set_all!(L39, L39{ d:[pass as u8;64] });
                    set_all!(L40, L40{ d:[pass as u8;64] }); set_all!(L41, L41{ d:[pass as u8;64] });
                    set_all!(L42, L42{ d:[pass as u8;64] }); set_all!(L43, L43{ d:[pass as u8;64] });
                    set_all!(L44, L44{ d:[pass as u8;64] }); set_all!(L45, L45{ d:[pass as u8;64] });
                    set_all!(L46, L46{ d:[pass as u8;64] }); set_all!(L47, L47{ d:[pass as u8;64] });
                    set_all!(L48, L48{ d:[pass as u8;64] }); set_all!(L49, L49{ d:[pass as u8;64] });

                    macro_rules! set_small {
                        ($ty:ty, $val:expr) => {{ let s = unsafe { &mut *world.get_storage::<$ty>() }; s.set(&frame, id, $val); }};
                    }
                    set_small!(S0, S0{ v: pass }); set_small!(S1, S1{ v: pass }); set_small!(S2, S2{ v: pass }); set_small!(S3, S3{ v: pass });
                    set_small!(S4, S4{ v: pass }); set_small!(S5, S5{ v: pass }); set_small!(S6, S6{ v: pass }); set_small!(S7, S7{ v: pass });
                    set_small!(S8, S8{ v: pass }); set_small!(S9, S9{ v: pass }); set_small!(S10, S10{ v: pass }); set_small!(S11, S11{ v: pass });
                    set_small!(S12, S12{ v: pass }); set_small!(S13, S13{ v: pass }); set_small!(S14, S14{ v: pass }); set_small!(S15, S15{ v: pass });
                    set_small!(S16, S16{ v: pass }); set_small!(S17, S17{ v: pass }); set_small!(S18, S18{ v: pass }); set_small!(S19, S19{ v: pass });
                    set_small!(S20, S20{ v: pass }); set_small!(S21, S21{ v: pass }); set_small!(S22, S22{ v: pass }); set_small!(S23, S23{ v: pass });
                    set_small!(S24, S24{ v: pass }); set_small!(S25, S25{ v: pass }); set_small!(S26, S26{ v: pass }); set_small!(S27, S27{ v: pass });
                    set_small!(S28, S28{ v: pass }); set_small!(S29, S29{ v: pass }); set_small!(S30, S30{ v: pass }); set_small!(S31, S31{ v: pass });
                    set_small!(S32, S32{ v: pass }); set_small!(S33, S33{ v: pass }); set_small!(S34, S34{ v: pass }); set_small!(S35, S35{ v: pass });
                    set_small!(S36, S36{ v: pass }); set_small!(S37, S37{ v: pass }); set_small!(S38, S38{ v: pass }); set_small!(S39, S39{ v: pass });
                    set_small!(S40, S40{ v: pass }); set_small!(S41, S41{ v: pass }); set_small!(S42, S42{ v: pass }); set_small!(S43, S43{ v: pass });
                    set_small!(S44, S44{ v: pass }); set_small!(S45, S45{ v: pass }); set_small!(S46, S46{ v: pass }); set_small!(S47, S47{ v: pass });
                    set_small!(S48, S48{ v: pass }); set_small!(S49, S49{ v: pass });
                }
                // clear rollback masks to simulate end of save pass (subset)
                {
                    let s = unsafe { &mut *world.get_storage::<L0>() }; s.rollback.clear_changed_masks();
                    let s = unsafe { &mut *world.get_storage::<L1>() }; s.rollback.clear_changed_masks();
                    let s = unsafe { &mut *world.get_storage::<S0>() }; s.rollback.clear_changed_masks();
                    let s = unsafe { &mut *world.get_storage::<S1>() }; s.rollback.clear_changed_masks();
                }
            }
        });
    });
}

fn bench_rollback_only(c: &mut Criterion) {
    use criterion::BatchSize;
    c.bench_function("rollback_only_10000x150x24", |b| {
        b.iter_batched(
            || {
                let mut world = World::new();
                static INIT: std::sync::Once = std::sync::Once::new();
                INIT.call_once(|| {
                    Ecs::register::<L0>(); Ecs::register::<L1>(); Ecs::register::<L2>(); Ecs::register::<L3>(); Ecs::register::<L4>();
                    Ecs::register::<L5>(); Ecs::register::<L6>(); Ecs::register::<L7>(); Ecs::register::<L8>(); Ecs::register::<L9>();
                    Ecs::register::<L10>(); Ecs::register::<L11>(); Ecs::register::<L12>(); Ecs::register::<L13>(); Ecs::register::<L14>();
                    Ecs::register::<L15>(); Ecs::register::<L16>(); Ecs::register::<L17>(); Ecs::register::<L18>(); Ecs::register::<L19>();
                    Ecs::register::<L20>(); Ecs::register::<L21>(); Ecs::register::<L22>(); Ecs::register::<L23>(); Ecs::register::<L24>();
                    Ecs::register::<L25>(); Ecs::register::<L26>(); Ecs::register::<L27>(); Ecs::register::<L28>(); Ecs::register::<L29>();
                    Ecs::register::<L30>(); Ecs::register::<L31>(); Ecs::register::<L32>(); Ecs::register::<L33>(); Ecs::register::<L34>();
                    Ecs::register::<L35>(); Ecs::register::<L36>(); Ecs::register::<L37>(); Ecs::register::<L38>(); Ecs::register::<L39>();
                    Ecs::register::<L40>(); Ecs::register::<L41>(); Ecs::register::<L42>(); Ecs::register::<L43>(); Ecs::register::<L44>();
                    Ecs::register::<L45>(); Ecs::register::<L46>(); Ecs::register::<L47>(); Ecs::register::<L48>(); Ecs::register::<L49>();
                    Ecs::register::<S0>(); Ecs::register::<S1>(); Ecs::register::<S2>(); Ecs::register::<S3>(); Ecs::register::<S4>();
                    Ecs::register::<S5>(); Ecs::register::<S6>(); Ecs::register::<S7>(); Ecs::register::<S8>(); Ecs::register::<S9>();
                    Ecs::register::<S10>(); Ecs::register::<S11>(); Ecs::register::<S12>(); Ecs::register::<S13>(); Ecs::register::<S14>();
                    Ecs::register::<S15>(); Ecs::register::<S16>(); Ecs::register::<S17>(); Ecs::register::<S18>(); Ecs::register::<S19>();
                    Ecs::register::<S20>(); Ecs::register::<S21>(); Ecs::register::<S22>(); Ecs::register::<S23>(); Ecs::register::<S24>();
                    Ecs::register::<S25>(); Ecs::register::<S26>(); Ecs::register::<S27>(); Ecs::register::<S28>(); Ecs::register::<S29>();
                    Ecs::register::<S30>(); Ecs::register::<S31>(); Ecs::register::<S32>(); Ecs::register::<S33>(); Ecs::register::<S34>();
                    Ecs::register::<S35>(); Ecs::register::<S36>(); Ecs::register::<S37>(); Ecs::register::<S38>(); Ecs::register::<S39>();
                    Ecs::register::<S40>(); Ecs::register::<S41>(); Ecs::register::<S42>(); Ecs::register::<S43>(); Ecs::register::<S44>();
                    Ecs::register::<S45>(); Ecs::register::<S46>(); Ecs::register::<S47>(); Ecs::register::<S48>(); Ecs::register::<S49>();
                });
                {
                    let _ = world.get_storage::<L0>(); let _ = world.get_storage::<L1>(); let _ = world.get_storage::<L2>(); let _ = world.get_storage::<L3>();
                    let _ = world.get_storage::<L4>(); let _ = world.get_storage::<L5>(); let _ = world.get_storage::<L6>(); let _ = world.get_storage::<L7>();
                    let _ = world.get_storage::<L8>(); let _ = world.get_storage::<L9>(); let _ = world.get_storage::<L10>(); let _ = world.get_storage::<L11>();
                    let _ = world.get_storage::<L12>(); let _ = world.get_storage::<L13>(); let _ = world.get_storage::<L14>(); let _ = world.get_storage::<L15>();
                    let _ = world.get_storage::<L16>(); let _ = world.get_storage::<L17>(); let _ = world.get_storage::<L18>(); let _ = world.get_storage::<L19>();
                    let _ = world.get_storage::<L20>(); let _ = world.get_storage::<L21>(); let _ = world.get_storage::<L22>(); let _ = world.get_storage::<L23>();
                    let _ = world.get_storage::<L24>(); let _ = world.get_storage::<L25>(); let _ = world.get_storage::<L26>(); let _ = world.get_storage::<L27>();
                    let _ = world.get_storage::<L28>(); let _ = world.get_storage::<L29>(); let _ = world.get_storage::<L30>(); let _ = world.get_storage::<L31>();
                    let _ = world.get_storage::<L32>(); let _ = world.get_storage::<L33>(); let _ = world.get_storage::<L34>(); let _ = world.get_storage::<L35>();
                    let _ = world.get_storage::<L36>(); let _ = world.get_storage::<L37>(); let _ = world.get_storage::<L38>(); let _ = world.get_storage::<L39>();
                    let _ = world.get_storage::<L40>(); let _ = world.get_storage::<L41>(); let _ = world.get_storage::<L42>(); let _ = world.get_storage::<L43>();
                    let _ = world.get_storage::<L44>(); let _ = world.get_storage::<L45>(); let _ = world.get_storage::<L46>(); let _ = world.get_storage::<L47>();
                    let _ = world.get_storage::<L48>(); let _ = world.get_storage::<L49>();
                    let _ = world.get_storage::<S0>(); let _ = world.get_storage::<S1>(); let _ = world.get_storage::<S2>(); let _ = world.get_storage::<S3>();
                    let _ = world.get_storage::<S4>(); let _ = world.get_storage::<S5>(); let _ = world.get_storage::<S6>(); let _ = world.get_storage::<S7>();
                    let _ = world.get_storage::<S8>(); let _ = world.get_storage::<S9>(); let _ = world.get_storage::<S10>(); let _ = world.get_storage::<S11>();
                    let _ = world.get_storage::<S12>(); let _ = world.get_storage::<S13>(); let _ = world.get_storage::<S14>(); let _ = world.get_storage::<S15>();
                    let _ = world.get_storage::<S16>(); let _ = world.get_storage::<S17>(); let _ = world.get_storage::<S18>(); let _ = world.get_storage::<S19>();
                    let _ = world.get_storage::<S20>(); let _ = world.get_storage::<S21>(); let _ = world.get_storage::<S22>(); let _ = world.get_storage::<S23>();
                    let _ = world.get_storage::<S24>(); let _ = world.get_storage::<S25>(); let _ = world.get_storage::<S26>(); let _ = world.get_storage::<S27>();
                    let _ = world.get_storage::<S28>(); let _ = world.get_storage::<S29>(); let _ = world.get_storage::<S30>(); let _ = world.get_storage::<S31>();
                    let _ = world.get_storage::<S32>(); let _ = world.get_storage::<S33>(); let _ = world.get_storage::<S34>(); let _ = world.get_storage::<S35>();
                    let _ = world.get_storage::<S36>(); let _ = world.get_storage::<S37>(); let _ = world.get_storage::<S38>(); let _ = world.get_storage::<S39>();
                    let _ = world.get_storage::<S40>(); let _ = world.get_storage::<S41>(); let _ = world.get_storage::<S42>(); let _ = world.get_storage::<S43>();
                    let _ = world.get_storage::<S44>(); let _ = world.get_storage::<S45>(); let _ = world.get_storage::<S46>(); let _ = world.get_storage::<S47>();
                    let _ = world.get_storage::<S48>(); let _ = world.get_storage::<S49>();
                }
                let n_entities = 10000u32;
                for pass in 0..24u32 {
                    world.set_tick(Tick(pass));
                    let frame = Frame::new(world.current_tick());
                    for id in 0..n_entities {
                        macro_rules! set_large { ($ty:ty, $val:expr) => {{ let s = unsafe { &mut *world.get_storage::<$ty>() }; s.set(&frame, id, $val); }}; }
                        set_large!(L0, L0{ d:[pass as u8;64] }); set_large!(L1, L1{ d:[pass as u8;64] }); set_large!(L2, L2{ d:[pass as u8;64] }); set_large!(L3, L3{ d:[pass as u8;64] });
                        set_large!(L4, L4{ d:[pass as u8;64] }); set_large!(L5, L5{ d:[pass as u8;64] }); set_large!(L6, L6{ d:[pass as u8;64] }); set_large!(L7, L7{ d:[pass as u8;64] });
                        set_large!(L8, L8{ d:[pass as u8;64] }); set_large!(L9, L9{ d:[pass as u8;64] }); set_large!(L10, L10{ d:[pass as u8;64] }); set_large!(L11, L11{ d:[pass as u8;64] });
                        set_large!(L12, L12{ d:[pass as u8;64] }); set_large!(L13, L13{ d:[pass as u8;64] }); set_large!(L14, L14{ d:[pass as u8;64] }); set_large!(L15, L15{ d:[pass as u8;64] });
                        set_large!(L16, L16{ d:[pass as u8;64] }); set_large!(L17, L17{ d:[pass as u8;64] }); set_large!(L18, L18{ d:[pass as u8;64] }); set_large!(L19, L19{ d:[pass as u8;64] });
                        set_large!(L20, L20{ d:[pass as u8;64] }); set_large!(L21, L21{ d:[pass as u8;64] }); set_large!(L22, L22{ d:[pass as u8;64] }); set_large!(L23, L23{ d:[pass as u8;64] });
                        set_large!(L24, L24{ d:[pass as u8;64] }); set_large!(L25, L25{ d:[pass as u8;64] }); set_large!(L26, L26{ d:[pass as u8;64] }); set_large!(L27, L27{ d:[pass as u8;64] });
                        set_large!(L28, L28{ d:[pass as u8;64] }); set_large!(L29, L29{ d:[pass as u8;64] }); set_large!(L30, L30{ d:[pass as u8;64] }); set_large!(L31, L31{ d:[pass as u8;64] });
                        set_large!(L32, L32{ d:[pass as u8;64] }); set_large!(L33, L33{ d:[pass as u8;64] }); set_large!(L34, L34{ d:[pass as u8;64] }); set_large!(L35, L35{ d:[pass as u8;64] });
                        set_large!(L36, L36{ d:[pass as u8;64] }); set_large!(L37, L37{ d:[pass as u8;64] }); set_large!(L38, L38{ d:[pass as u8;64] }); set_large!(L39, L39{ d:[pass as u8;64] });
                        set_large!(L40, L40{ d:[pass as u8;64] }); set_large!(L41, L41{ d:[pass as u8;64] }); set_large!(L42, L42{ d:[pass as u8;64] }); set_large!(L43, L43{ d:[pass as u8;64] });
                        set_large!(L44, L44{ d:[pass as u8;64] }); set_large!(L45, L45{ d:[pass as u8;64] }); set_large!(L46, L46{ d:[pass as u8;64] }); set_large!(L47, L47{ d:[pass as u8;64] });
                        set_large!(L48, L48{ d:[pass as u8;64] }); set_large!(L49, L49{ d:[pass as u8;64] });
                        macro_rules! set_small { ($ty:ty, $val:expr) => {{ let s = unsafe { &mut *world.get_storage::<$ty>() }; s.set(&frame, id, $val); }}; }
                        set_small!(S0, S0{ v: pass }); set_small!(S1, S1{ v: pass }); set_small!(S2, S2{ v: pass }); set_small!(S3, S3{ v: pass });
                        set_small!(S4, S4{ v: pass }); set_small!(S5, S5{ v: pass }); set_small!(S6, S6{ v: pass }); set_small!(S7, S7{ v: pass });
                        set_small!(S8, S8{ v: pass }); set_small!(S9, S9{ v: pass }); set_small!(S10, S10{ v: pass }); set_small!(S11, S11{ v: pass });
                        set_small!(S12, S12{ v: pass }); set_small!(S13, S13{ v: pass }); set_small!(S14, S14{ v: pass }); set_small!(S15, S15{ v: pass });
                        set_small!(S16, S16{ v: pass }); set_small!(S17, S17{ v: pass }); set_small!(S18, S18{ v: pass }); set_small!(S19, S19{ v: pass });
                        set_small!(S20, S20{ v: pass }); set_small!(S21, S21{ v: pass }); set_small!(S22, S22{ v: pass }); set_small!(S23, S23{ v: pass });
                        set_small!(S24, S24{ v: pass }); set_small!(S25, S25{ v: pass }); set_small!(S26, S26{ v: pass }); set_small!(S27, S27{ v: pass });
                        set_small!(S28, S28{ v: pass }); set_small!(S29, S29{ v: pass }); set_small!(S30, S30{ v: pass }); set_small!(S31, S31{ v: pass });
                        set_small!(S32, S32{ v: pass }); set_small!(S33, S33{ v: pass }); set_small!(S34, S34{ v: pass }); set_small!(S35, S35{ v: pass });
                        set_small!(S36, S36{ v: pass }); set_small!(S37, S37{ v: pass }); set_small!(S38, S38{ v: pass }); set_small!(S39, S39{ v: pass });
                        set_small!(S40, S40{ v: pass }); set_small!(S41, S41{ v: pass }); set_small!(S42, S42{ v: pass }); set_small!(S43, S43{ v: pass });
                        set_small!(S44, S44{ v: pass }); set_small!(S45, S45{ v: pass }); set_small!(S46, S46{ v: pass }); set_small!(S47, S47{ v: pass });
                        set_small!(S48, S48{ v: pass }); set_small!(S49, S49{ v: pass });
                    }
                }
                world
            },
            |mut world| {
                let t = Tick(0);
                { let s = unsafe { &mut *world.get_storage::<L0>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L1>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L2>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L3>() }; s.rollback(t); }
                { let s = unsafe { &mut *world.get_storage::<L4>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L5>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L6>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L7>() }; s.rollback(t); }
                { let s = unsafe { &mut *world.get_storage::<L8>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L9>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L10>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L11>() }; s.rollback(t); }
                { let s = unsafe { &mut *world.get_storage::<L12>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L13>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L14>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L15>() }; s.rollback(t); }
                { let s = unsafe { &mut *world.get_storage::<L16>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L17>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L18>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L19>() }; s.rollback(t); }
                { let s = unsafe { &mut *world.get_storage::<L20>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L21>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L22>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L23>() }; s.rollback(t); }
                { let s = unsafe { &mut *world.get_storage::<L24>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L25>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L26>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L27>() }; s.rollback(t); }
                { let s = unsafe { &mut *world.get_storage::<L28>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L29>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L30>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L31>() }; s.rollback(t); }
                { let s = unsafe { &mut *world.get_storage::<L32>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L33>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L34>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L35>() }; s.rollback(t); }
                { let s = unsafe { &mut *world.get_storage::<L36>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L37>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L38>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L39>() }; s.rollback(t); }
                { let s = unsafe { &mut *world.get_storage::<L40>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L41>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L42>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L43>() }; s.rollback(t); }
                { let s = unsafe { &mut *world.get_storage::<L44>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L45>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L46>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L47>() }; s.rollback(t); }
                { let s = unsafe { &mut *world.get_storage::<L48>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<L49>() }; s.rollback(t); }
                { let s = unsafe { &mut *world.get_storage::<S0>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S1>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S2>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S3>() }; s.rollback(t); }
                { let s = unsafe { &mut *world.get_storage::<S4>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S5>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S6>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S7>() }; s.rollback(t); }
                { let s = unsafe { &mut *world.get_storage::<S8>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S9>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S10>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S11>() }; s.rollback(t); }
                { let s = unsafe { &mut *world.get_storage::<S12>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S13>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S14>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S15>() }; s.rollback(t); }
                { let s = unsafe { &mut *world.get_storage::<S16>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S17>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S18>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S19>() }; s.rollback(t); }
                { let s = unsafe { &mut *world.get_storage::<S20>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S21>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S22>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S23>() }; s.rollback(t); }
                { let s = unsafe { &mut *world.get_storage::<S24>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S25>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S26>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S27>() }; s.rollback(t); }
                { let s = unsafe { &mut *world.get_storage::<S28>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S29>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S30>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S31>() }; s.rollback(t); }
                { let s = unsafe { &mut *world.get_storage::<S32>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S33>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S34>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S35>() }; s.rollback(t); }
                { let s = unsafe { &mut *world.get_storage::<S36>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S37>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S38>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S39>() }; s.rollback(t); }
                { let s = unsafe { &mut *world.get_storage::<S40>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S41>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S42>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S43>() }; s.rollback(t); }
                { let s = unsafe { &mut *world.get_storage::<S44>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S45>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S46>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S47>() }; s.rollback(t); }
                { let s = unsafe { &mut *world.get_storage::<S48>() }; s.rollback(t); let s = unsafe { &mut *world.get_storage::<S49>() }; s.rollback(t); }
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_apples_to_apples_1000_150_24(c: &mut Criterion) {
    c.bench_function("apples_1000x150x24", |b| {
        b.iter(|| {
            let mut world = World::new();
            static INIT: std::sync::Once = std::sync::Once::new();
            INIT.call_once(|| {
                Ecs::register::<L0>(); Ecs::register::<L1>(); Ecs::register::<L2>(); Ecs::register::<L3>(); Ecs::register::<L4>();
                Ecs::register::<L5>(); Ecs::register::<L6>(); Ecs::register::<L7>(); Ecs::register::<L8>(); Ecs::register::<L9>();
                Ecs::register::<L10>(); Ecs::register::<L11>(); Ecs::register::<L12>(); Ecs::register::<L13>(); Ecs::register::<L14>();
                Ecs::register::<L15>(); Ecs::register::<L16>(); Ecs::register::<L17>(); Ecs::register::<L18>(); Ecs::register::<L19>();
                Ecs::register::<L20>(); Ecs::register::<L21>(); Ecs::register::<L22>(); Ecs::register::<L23>(); Ecs::register::<L24>();
                Ecs::register::<L25>(); Ecs::register::<L26>(); Ecs::register::<L27>(); Ecs::register::<L28>(); Ecs::register::<L29>();
                Ecs::register::<L30>(); Ecs::register::<L31>(); Ecs::register::<L32>(); Ecs::register::<L33>(); Ecs::register::<L34>();
                Ecs::register::<L35>(); Ecs::register::<L36>(); Ecs::register::<L37>(); Ecs::register::<L38>(); Ecs::register::<L39>();
                Ecs::register::<L40>(); Ecs::register::<L41>(); Ecs::register::<L42>(); Ecs::register::<L43>(); Ecs::register::<L44>();
                Ecs::register::<L45>(); Ecs::register::<L46>(); Ecs::register::<L47>(); Ecs::register::<L48>(); Ecs::register::<L49>();
                Ecs::register::<S0>(); Ecs::register::<S1>(); Ecs::register::<S2>(); Ecs::register::<S3>(); Ecs::register::<S4>();
                Ecs::register::<S5>(); Ecs::register::<S6>(); Ecs::register::<S7>(); Ecs::register::<S8>(); Ecs::register::<S9>();
                Ecs::register::<S10>(); Ecs::register::<S11>(); Ecs::register::<S12>(); Ecs::register::<S13>(); Ecs::register::<S14>();
                Ecs::register::<S15>(); Ecs::register::<S16>(); Ecs::register::<S17>(); Ecs::register::<S18>(); Ecs::register::<S19>();
                Ecs::register::<S20>(); Ecs::register::<S21>(); Ecs::register::<S22>(); Ecs::register::<S23>(); Ecs::register::<S24>();
                Ecs::register::<S25>(); Ecs::register::<S26>(); Ecs::register::<S27>(); Ecs::register::<S28>(); Ecs::register::<S29>();
                Ecs::register::<S30>(); Ecs::register::<S31>(); Ecs::register::<S32>(); Ecs::register::<S33>(); Ecs::register::<S34>();
                Ecs::register::<S35>(); Ecs::register::<S36>(); Ecs::register::<S37>(); Ecs::register::<S38>(); Ecs::register::<S39>();
                Ecs::register::<S40>(); Ecs::register::<S41>(); Ecs::register::<S42>(); Ecs::register::<S43>(); Ecs::register::<S44>();
                Ecs::register::<S45>(); Ecs::register::<S46>(); Ecs::register::<S47>(); Ecs::register::<S48>(); Ecs::register::<S49>();
                Ecs::register::<Tag0>(); Ecs::register::<Tag1>(); Ecs::register::<Tag2>(); Ecs::register::<Tag3>(); Ecs::register::<Tag4>();
                Ecs::register::<Tag5>(); Ecs::register::<Tag6>(); Ecs::register::<Tag7>(); Ecs::register::<Tag8>(); Ecs::register::<Tag9>();
                Ecs::register::<Tag10>(); Ecs::register::<Tag11>(); Ecs::register::<Tag12>(); Ecs::register::<Tag13>(); Ecs::register::<Tag14>();
                Ecs::register::<Tag15>(); Ecs::register::<Tag16>(); Ecs::register::<Tag17>(); Ecs::register::<Tag18>(); Ecs::register::<Tag19>();
                Ecs::register::<Tag20>(); Ecs::register::<Tag21>(); Ecs::register::<Tag22>(); Ecs::register::<Tag23>(); Ecs::register::<Tag24>();
                Ecs::register::<Tag25>();
            });
            {
                let _ = world.get_storage::<L0>(); let _ = world.get_storage::<L1>(); let _ = world.get_storage::<L2>(); let _ = world.get_storage::<L3>();
                let _ = world.get_storage::<L4>(); let _ = world.get_storage::<L5>(); let _ = world.get_storage::<L6>(); let _ = world.get_storage::<L7>();
                let _ = world.get_storage::<L8>(); let _ = world.get_storage::<L9>(); let _ = world.get_storage::<L10>(); let _ = world.get_storage::<L11>();
                let _ = world.get_storage::<L12>(); let _ = world.get_storage::<L13>(); let _ = world.get_storage::<L14>(); let _ = world.get_storage::<L15>();
                let _ = world.get_storage::<L16>(); let _ = world.get_storage::<L17>(); let _ = world.get_storage::<L18>(); let _ = world.get_storage::<L19>();
                let _ = world.get_storage::<L20>(); let _ = world.get_storage::<L21>(); let _ = world.get_storage::<L22>(); let _ = world.get_storage::<L23>();
                let _ = world.get_storage::<L24>(); let _ = world.get_storage::<L25>(); let _ = world.get_storage::<L26>(); let _ = world.get_storage::<L27>();
                let _ = world.get_storage::<L28>(); let _ = world.get_storage::<L29>(); let _ = world.get_storage::<L30>(); let _ = world.get_storage::<L31>();
                let _ = world.get_storage::<L32>(); let _ = world.get_storage::<L33>(); let _ = world.get_storage::<L34>(); let _ = world.get_storage::<L35>();
                let _ = world.get_storage::<L36>(); let _ = world.get_storage::<L37>(); let _ = world.get_storage::<L38>(); let _ = world.get_storage::<L39>();
                let _ = world.get_storage::<L40>(); let _ = world.get_storage::<L41>(); let _ = world.get_storage::<L42>(); let _ = world.get_storage::<L43>();
                let _ = world.get_storage::<L44>(); let _ = world.get_storage::<L45>(); let _ = world.get_storage::<L46>(); let _ = world.get_storage::<L47>();
                let _ = world.get_storage::<L48>(); let _ = world.get_storage::<L49>();
                let _ = world.get_storage::<S0>(); let _ = world.get_storage::<S1>(); let _ = world.get_storage::<S2>(); let _ = world.get_storage::<S3>();
                let _ = world.get_storage::<S4>(); let _ = world.get_storage::<S5>(); let _ = world.get_storage::<S6>(); let _ = world.get_storage::<S7>();
                let _ = world.get_storage::<S8>(); let _ = world.get_storage::<S9>(); let _ = world.get_storage::<S10>(); let _ = world.get_storage::<S11>();
                let _ = world.get_storage::<S12>(); let _ = world.get_storage::<S13>(); let _ = world.get_storage::<S14>(); let _ = world.get_storage::<S15>();
                let _ = world.get_storage::<S16>(); let _ = world.get_storage::<S17>(); let _ = world.get_storage::<S18>(); let _ = world.get_storage::<S19>();
                let _ = world.get_storage::<S20>(); let _ = world.get_storage::<S21>(); let _ = world.get_storage::<S22>(); let _ = world.get_storage::<S23>();
                let _ = world.get_storage::<S24>(); let _ = world.get_storage::<S25>(); let _ = world.get_storage::<S26>(); let _ = world.get_storage::<S27>();
                let _ = world.get_storage::<S28>(); let _ = world.get_storage::<S29>(); let _ = world.get_storage::<S30>(); let _ = world.get_storage::<S31>();
                let _ = world.get_storage::<S32>(); let _ = world.get_storage::<S33>(); let _ = world.get_storage::<S34>(); let _ = world.get_storage::<S35>();
                let _ = world.get_storage::<S36>(); let _ = world.get_storage::<S37>(); let _ = world.get_storage::<S38>(); let _ = world.get_storage::<S39>();
                let _ = world.get_storage::<S40>(); let _ = world.get_storage::<S41>(); let _ = world.get_storage::<S42>(); let _ = world.get_storage::<S43>();
                let _ = world.get_storage::<S44>(); let _ = world.get_storage::<S45>(); let _ = world.get_storage::<S46>(); let _ = world.get_storage::<S47>();
                let _ = world.get_storage::<S48>(); let _ = world.get_storage::<S49>();
                let _ = world.get_storage::<Tag0>(); let _ = world.get_storage::<Tag1>(); let _ = world.get_storage::<Tag2>(); let _ = world.get_storage::<Tag3>();
                let _ = world.get_storage::<Tag4>(); let _ = world.get_storage::<Tag5>(); let _ = world.get_storage::<Tag6>(); let _ = world.get_storage::<Tag7>();
                let _ = world.get_storage::<Tag8>(); let _ = world.get_storage::<Tag9>(); let _ = world.get_storage::<Tag10>(); let _ = world.get_storage::<Tag11>();
                let _ = world.get_storage::<Tag12>(); let _ = world.get_storage::<Tag13>(); let _ = world.get_storage::<Tag14>(); let _ = world.get_storage::<Tag15>();
                let _ = world.get_storage::<Tag16>(); let _ = world.get_storage::<Tag17>(); let _ = world.get_storage::<Tag18>(); let _ = world.get_storage::<Tag19>();
                let _ = world.get_storage::<Tag20>(); let _ = world.get_storage::<Tag21>(); let _ = world.get_storage::<Tag22>(); let _ = world.get_storage::<Tag23>();
                let _ = world.get_storage::<Tag24>(); let _ = world.get_storage::<Tag25>();
            }
            let n_entities = 1000u32;
            for pass in 0..24u32 {
                world.set_tick(Tick(pass));
                let frame = Frame::new(world.current_tick());
                for id in 0..n_entities {
                    macro_rules! set_large { ($ty:ty, $val:expr) => {{ let s = unsafe { &mut *world.get_storage::<$ty>() }; s.set(&frame, id, $val); }}; }
                    set_large!(L0, L0{ d:[pass as u8;64] }); set_large!(L1, L1{ d:[pass as u8;64] }); set_large!(L2, L2{ d:[pass as u8;64] }); set_large!(L3, L3{ d:[pass as u8;64] });
                    set_large!(L4, L4{ d:[pass as u8;64] }); set_large!(L5, L5{ d:[pass as u8;64] }); set_large!(L6, L6{ d:[pass as u8;64] }); set_large!(L7, L7{ d:[pass as u8;64] });
                    set_large!(L8, L8{ d:[pass as u8;64] }); set_large!(L9, L9{ d:[pass as u8;64] }); set_large!(L10, L10{ d:[pass as u8;64] }); set_large!(L11, L11{ d:[pass as u8;64] });
                    set_large!(L12, L12{ d:[pass as u8;64] }); set_large!(L13, L13{ d:[pass as u8;64] }); set_large!(L14, L14{ d:[pass as u8;64] }); set_large!(L15, L15{ d:[pass as u8;64] });
                    set_large!(L16, L16{ d:[pass as u8;64] }); set_large!(L17, L17{ d:[pass as u8;64] }); set_large!(L18, L18{ d:[pass as u8;64] }); set_large!(L19, L19{ d:[pass as u8;64] });
                    set_large!(L20, L20{ d:[pass as u8;64] }); set_large!(L21, L21{ d:[pass as u8;64] }); set_large!(L22, L22{ d:[pass as u8;64] }); set_large!(L23, L23{ d:[pass as u8;64] });
                    set_large!(L24, L24{ d:[pass as u8;64] }); set_large!(L25, L25{ d:[pass as u8;64] }); set_large!(L26, L26{ d:[pass as u8;64] }); set_large!(L27, L27{ d:[pass as u8;64] });
                    set_large!(L28, L28{ d:[pass as u8;64] }); set_large!(L29, L29{ d:[pass as u8;64] }); set_large!(L30, L30{ d:[pass as u8;64] }); set_large!(L31, L31{ d:[pass as u8;64] });
                    set_large!(L32, L32{ d:[pass as u8;64] }); set_large!(L33, L33{ d:[pass as u8;64] }); set_large!(L34, L34{ d:[pass as u8;64] }); set_large!(L35, L35{ d:[pass as u8;64] });
                    set_large!(L36, L36{ d:[pass as u8;64] }); set_large!(L37, L37{ d:[pass as u8;64] }); set_large!(L38, L38{ d:[pass as u8;64] }); set_large!(L39, L39{ d:[pass as u8;64] });
                    set_large!(L40, L40{ d:[pass as u8;64] }); set_large!(L41, L41{ d:[pass as u8;64] }); set_large!(L42, L42{ d:[pass as u8;64] }); set_large!(L43, L43{ d:[pass as u8;64] });
                    set_large!(L44, L44{ d:[pass as u8;64] }); set_large!(L45, L45{ d:[pass as u8;64] }); set_large!(L46, L46{ d:[pass as u8;64] }); set_large!(L47, L47{ d:[pass as u8;64] });
                    set_large!(L48, L48{ d:[pass as u8;64] }); set_large!(L49, L49{ d:[pass as u8;64] });
                    macro_rules! set_small { ($ty:ty, $val:expr) => {{ let s = unsafe { &mut *world.get_storage::<$ty>() }; s.set(&frame, id, $val); }}; }
                    set_small!(S0, S0{ v: pass }); set_small!(S1, S1{ v: pass }); set_small!(S2, S2{ v: pass }); set_small!(S3, S3{ v: pass });
                    set_small!(S4, S4{ v: pass }); set_small!(S5, S5{ v: pass }); set_small!(S6, S6{ v: pass }); set_small!(S7, S7{ v: pass });
                    set_small!(S8, S8{ v: pass }); set_small!(S9, S9{ v: pass }); set_small!(S10, S10{ v: pass }); set_small!(S11, S11{ v: pass });
                    set_small!(S12, S12{ v: pass }); set_small!(S13, S13{ v: pass }); set_small!(S14, S14{ v: pass }); set_small!(S15, S15{ v: pass });
                    set_small!(S16, S16{ v: pass }); set_small!(S17, S17{ v: pass }); set_small!(S18, S18{ v: pass }); set_small!(S19, S19{ v: pass });
                    set_small!(S20, S20{ v: pass }); set_small!(S21, S21{ v: pass }); set_small!(S22, S22{ v: pass }); set_small!(S23, S23{ v: pass });
                    set_small!(S24, S24{ v: pass }); set_small!(S25, S25{ v: pass }); set_small!(S26, S26{ v: pass }); set_small!(S27, S27{ v: pass });
                    set_small!(S28, S28{ v: pass }); set_small!(S29, S29{ v: pass }); set_small!(S30, S30{ v: pass }); set_small!(S31, S31{ v: pass });
                    set_small!(S32, S32{ v: pass }); set_small!(S33, S33{ v: pass }); set_small!(S34, S34{ v: pass }); set_small!(S35, S35{ v: pass });
                    set_small!(S36, S36{ v: pass }); set_small!(S37, S37{ v: pass }); set_small!(S38, S38{ v: pass }); set_small!(S39, S39{ v: pass });
                    set_small!(S40, S40{ v: pass }); set_small!(S41, S41{ v: pass }); set_small!(S42, S42{ v: pass }); set_small!(S43, S43{ v: pass });
                    set_small!(S44, S44{ v: pass }); set_small!(S45, S45{ v: pass }); set_small!(S46, S46{ v: pass }); set_small!(S47, S47{ v: pass });
                    set_small!(S48, S48{ v: pass }); set_small!(S49, S49{ v: pass });
                    macro_rules! set_tag { ($ty:ident) => {{ let s = unsafe { &mut *world.get_storage::<$ty>() }; s.set(id, $ty); }}; }
                    set_tag!(Tag0); set_tag!(Tag1); set_tag!(Tag2); set_tag!(Tag3); set_tag!(Tag4);
                    set_tag!(Tag5); set_tag!(Tag6); set_tag!(Tag7); set_tag!(Tag8); set_tag!(Tag9);
                    set_tag!(Tag10); set_tag!(Tag11); set_tag!(Tag12); set_tag!(Tag13); set_tag!(Tag14);
                    set_tag!(Tag15); set_tag!(Tag16); set_tag!(Tag17); set_tag!(Tag18); set_tag!(Tag19);
                    set_tag!(Tag20); set_tag!(Tag21); set_tag!(Tag22); set_tag!(Tag23); set_tag!(Tag24);
                    set_tag!(Tag25);
                }
            }
        });
    });
}

fn bench_apples_to_apples_10k_15_12(c: &mut Criterion) {
    c.bench_function("apples_10kx15x12", |b| {
        b.iter(|| {
            let mut world = World::new();
            static INIT: std::sync::Once = std::sync::Once::new();
            INIT.call_once(|| {
                Ecs::register::<L0>(); Ecs::register::<L1>(); Ecs::register::<L2>(); Ecs::register::<L3>(); Ecs::register::<L4>();
                Ecs::register::<S0>(); Ecs::register::<S1>(); Ecs::register::<S2>(); Ecs::register::<S3>(); Ecs::register::<S4>();
                Ecs::register::<Tag0>(); Ecs::register::<Tag1>(); Ecs::register::<Tag2>(); Ecs::register::<Tag3>(); Ecs::register::<Tag4>();
            });
            let _ = world.get_storage::<L0>(); let _ = world.get_storage::<S0>(); let _ = world.get_storage::<Tag0>();
            let _ = world.get_storage::<L1>(); let _ = world.get_storage::<S1>(); let _ = world.get_storage::<Tag1>();
            let _ = world.get_storage::<L2>(); let _ = world.get_storage::<S2>(); let _ = world.get_storage::<Tag2>();
            let _ = world.get_storage::<L3>(); let _ = world.get_storage::<S3>(); let _ = world.get_storage::<Tag3>();
            let _ = world.get_storage::<L4>(); let _ = world.get_storage::<S4>(); let _ = world.get_storage::<Tag4>();
            let n_entities = 10000u32;
            for pass in 0..12u32 {
                world.set_tick(Tick(pass));
                let frame = Frame::new(world.current_tick());
                for id in 0..n_entities {
                    let sL0 = unsafe { &mut *world.get_storage::<L0>() }; sL0.set(&frame, id, L0{ d:[pass as u8;64] });
                    let sL1 = unsafe { &mut *world.get_storage::<L1>() }; sL1.set(&frame, id, L1{ d:[pass as u8;64] });
                    let sL2 = unsafe { &mut *world.get_storage::<L2>() }; sL2.set(&frame, id, L2{ d:[pass as u8;64] });
                    let sL3 = unsafe { &mut *world.get_storage::<L3>() }; sL3.set(&frame, id, L3{ d:[pass as u8;64] });
                    let sL4 = unsafe { &mut *world.get_storage::<L4>() }; sL4.set(&frame, id, L4{ d:[pass as u8;64] });
                    let sS0 = unsafe { &mut *world.get_storage::<S0>() }; sS0.set(&frame, id, S0{ v: pass });
                    let sS1 = unsafe { &mut *world.get_storage::<S1>() }; sS1.set(&frame, id, S1{ v: pass });
                    let sS2 = unsafe { &mut *world.get_storage::<S2>() }; sS2.set(&frame, id, S2{ v: pass });
                    let sS3 = unsafe { &mut *world.get_storage::<S3>() }; sS3.set(&frame, id, S3{ v: pass });
                    let sS4 = unsafe { &mut *world.get_storage::<S4>() }; sS4.set(&frame, id, S4{ v: pass });
                    let sT0 = unsafe { &mut *world.get_storage::<Tag0>() }; sT0.set(&frame, id, Tag0);
                    let sT1 = unsafe { &mut *world.get_storage::<Tag1>() }; sT1.set(&frame, id, Tag1);
                    let sT2 = unsafe { &mut *world.get_storage::<Tag2>() }; sT2.set(&frame, id, Tag2);
                    let sT3 = unsafe { &mut *world.get_storage::<Tag3>() }; sT3.set(&frame, id, Tag3);
                    let sT4 = unsafe { &mut *world.get_storage::<Tag4>() }; sT4.set(&frame, id, Tag4);
                }
            }
        });
    });
}

criterion_group!(benches, bench_full_pipeline_20k, bench_perf_reference_like, bench_rollback_only, bench_apples_to_apples_1000_150_24, bench_apples_to_apples_10k_15_12);
criterion_main!(benches);
