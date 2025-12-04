#![allow(non_upper_case_globals)]
use crate::component::Component;
use crate::ecs::Ecs;
use crate::entity::Entity;
use crate::frame::Frame;
use crate::scheduler::Scheduler;
use crate::storage::{Storage, StorageLike};
use crate::tick::Tick;
use std::ptr::NonNull;

decs_macros::system_group!(InitializationGroup {});
decs_macros::system_group!(SimulationGroup { After=[InitializationGroup] });
decs_macros::system_group!(CleanupGroup { After=[SimulationGroup] });
decs_macros::system_group!(DestroyGroup { After=[CleanupGroup] });

pub struct World {
    storage_mask: [u64; 4],
    storage_ptrs: [Option<NonNull<dyn StorageLike>>; 256],
    typed_ptrs: [*mut (); 256],
    current_tick: Tick,
    scheduler: Scheduler,
}

impl World {
    /// Creates a new empty World.
    pub fn new() -> Self {
        let mut world = Self {
            storage_mask: [0; 4],
            storage_ptrs: [None; 256],
            typed_ptrs: [std::ptr::null_mut(); 256],
            current_tick: Tick(0),
            scheduler: Scheduler::new(),
        };

        // Register Entity component and create its storage immediately
        Ecs::register::<Entity>();
        let _ = world.get_storage::<Entity>();

        world
    }

    /// Returns an immutable reference to the scheduler.
    pub fn scheduler(&self) -> &Scheduler {
        &self.scheduler
    }

    /// Returns a mutable reference to the scheduler.
    pub fn scheduler_mut(&mut self) -> &mut Scheduler {
        &mut self.scheduler
    }

    /// Returns the current tick.
    pub fn current_tick(&self) -> Tick {
        self.current_tick
    }

    pub fn set_tick(&mut self, tick: Tick) {
        self.current_tick = tick;
        crate::tick::CURRENT_TICK.with(|c| c.set(tick));
    }

    pub fn run(&mut self) {
        self.current_tick = Tick(self.current_tick.0.wrapping_add(1));
        let frame = Frame::new(self.current_tick());
        self.scheduler.run(&frame);
    }

    /// Gets the Entity storage pointer.
    pub fn get_entity_storage(&mut self) -> *mut Storage<Entity> {
        self.get_storage::<Entity>()
    }

    /// Gets a raw pointer to the storage for component type T.
    /// Creates the storage if it doesn't exist.
    pub fn get_storage<T: Component>(&mut self) -> *mut Storage<T> {
        let id = T::id();
        assert!(id < 256, "Component ID must be less than 256");
        let index = id as usize;

        let seg = (id / 64) as usize;
        let bit = id % 64;
        let present = (self.storage_mask[seg] >> bit) & 1 != 0;

        if !present || self.storage_ptrs[index].is_none() {
            let storage_box: Box<Storage<T>> = Box::default();
            self.storage_mask[seg] |= 1u64 << bit;
            let raw_typed: *mut Storage<T> = Box::into_raw(storage_box);
            let raw_trait: *mut dyn StorageLike = raw_typed;
            let nn = NonNull::new(raw_trait).expect("Box::into_raw should not yield null");
            self.storage_ptrs[index] = Some(nn);
            self.typed_ptrs[index] = raw_typed as *mut ();
        }
        self.typed_ptrs[index] as *mut Storage<T>
    }

    /// Returns a mutable reference to the storage for component type T.
    /// Creates the storage if it doesn't exist.
    pub fn get_storage_mut<T: Component>(&mut self) -> &mut Storage<T> {
        let ptr = self.get_storage::<T>();
        unsafe { &mut *ptr }
    }

    /// Verifies that all invariants hold for this World and all its storages.
    /// Also checks that all changed_mask values are 0 at every level (Storage, Page, Chunk).
    ///
    /// Returns true if all invariants are satisfied, false otherwise.
    pub fn verify_invariants(&self) -> bool {
        for seg in 0..4 {
            let base = seg * 64;
            let mut remaining_mask = self.storage_mask[seg];
            while remaining_mask != 0 {
                let start = remaining_mask.trailing_zeros() as usize;
                let shifted = remaining_mask >> start;
                let run_len = shifted.trailing_ones() as usize;
                for i in start..start + run_len {
                    let idx = base + i;
                    if let Some(ptr) = self.storage_ptrs[idx] {
                        if !unsafe { ptr.as_ref().verify_invariants() } {
                            return false;
                        }
                    } else {
                        continue;
                    }
                }
                remaining_mask &= !((1u64 << run_len) - 1) << start;
            }
        }

        true
    }

    /// Gets a raw pointer to the storage for component type T without requiring a mutable borrow.
    /// The storage must already exist.
    pub fn get_storage_ptr<T: Component>(&self) -> *mut Storage<T> {
        let id = T::id();
        assert!(id < 256, "Component ID must be less than 256");
        let seg = (id / 64) as usize;
        let bit = id % 64;
        let present = (self.storage_mask[seg] >> bit) & 1 != 0;
        assert!(
            present,
            "Storage for component must exist before get_storage_ptr"
        );
        self.typed_ptrs[id as usize] as *mut Storage<T>
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for World {
    fn drop(&mut self) {
        unsafe {
            for seg in 0..4 {
                let base = seg * 64;
                let mut remaining_mask = self.storage_mask[seg];
                while remaining_mask != 0 {
                    let start = remaining_mask.trailing_zeros() as usize;
                    let shifted = remaining_mask >> start;
                    let run_len = shifted.trailing_ones() as usize;
                    for i in start..start + run_len {
                        let idx = base + i;
                        if let Some(nn) = self.storage_ptrs[idx] {
                            let raw = nn.as_ptr();
                            let boxed: Box<dyn StorageLike> = Box::from_raw(raw);
                            drop(boxed);
                            self.storage_ptrs[idx] = None;
                        }
                    }
                    remaining_mask &= !((1u64 << run_len) - 1) << start;
                }
            }
        }
    }
}
