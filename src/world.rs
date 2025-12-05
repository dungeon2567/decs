#![allow(non_upper_case_globals)]
use crate::component::Component;
use crate::entity::Entity;
use crate::frame::Frame;
use crate::scheduler::Scheduler;
use crate::storage::{Storage, StorageLike};
use crate::tick::Tick;

decs_macros::system_group!(InitializationGroup {});
decs_macros::system_group!(SimulationGroup { After=[InitializationGroup] });
decs_macros::system_group!(CleanupGroup { After=[SimulationGroup] });
decs_macros::system_group!(DestroyGroup { After=[CleanupGroup] });

pub struct World {
    storage_mask: [u64; 4],
    storage_ptrs: [Option<Box<dyn StorageLike>>; 256],
    storage_raw_ptrs: [*mut (); 256],
    current_tick: Tick,
    scheduler: Scheduler,
}

impl World {
    /// Creates a new empty World.
    pub fn new() -> Self {
        let mut world = Self {
            storage_mask: [0; 4],
            storage_ptrs: [const { None }; 256],
            storage_raw_ptrs: [std::ptr::null_mut(); 256],
            current_tick: Tick(0),
            scheduler: Scheduler::new(),
        };

        let _ = world.get_storage::<Entity>();
        let _ = world.get_storage::<crate::component::Destroyed>();

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

        if !present {
            let storage_box: Box<Storage<T>> = Box::default();
            let raw = Box::into_raw(storage_box);
            self.storage_mask[seg] |= 1u64 << bit;
            let trait_box: Box<dyn StorageLike> = unsafe { Box::from_raw(raw) };
            self.storage_ptrs[index] = Some(trait_box);
            self.storage_raw_ptrs[index] = raw as *mut ();

            T::schedule_cleanup_system(self);
        }

        self.storage_raw_ptrs[index] as *mut Storage<T>
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
                    if let Some(ref boxed) = self.storage_ptrs[idx] {
                        if !boxed.verify_invariants() {
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

    /// Rolls back all component storages to the specified tick.
    /// This iterates through all active storages and calls their rollback method.
    /// After rolling back all storages, sets the world tick to target_tick.
    ///
    /// # Note
    /// Only works for components that implement Clone (required by Storage::rollback).
    /// The world tick is updated to target_tick after all rollbacks complete.
    pub fn rollback(&mut self, target_tick: Tick) {
        // Iterate through all storage segments
        for seg in 0..4 {
            let base = seg * 64;
            let mut remaining_mask = self.storage_mask[seg];
            
            while remaining_mask != 0 {
                let start = remaining_mask.trailing_zeros() as usize;
                let shifted = remaining_mask >> start;
                let run_len = shifted.trailing_ones() as usize;
                
                for i in start..start + run_len {
                    let idx = base + i;
                    
                    // Call rollback through StorageLike trait
                    if let Some(ref mut storage) = self.storage_ptrs[idx] {
                        storage.rollback(target_tick);
                    }
                }
                
                remaining_mask &= !((1u64 << run_len) - 1) << start;
            }
        }
        
        // Update world tick to target_tick
        self.set_tick(target_tick);
    }
}

impl Drop for World {
    fn drop(&mut self) {
        // Drop systems first to ensure they release any references to storages
        std::mem::drop(std::mem::take(&mut self.scheduler));
        // Then drop storages
        for seg in 0..4 {
            let base = seg * 64;
            let mut remaining_mask = self.storage_mask[seg];

            while remaining_mask != 0 {
                let start = remaining_mask.trailing_zeros() as usize;
                let shifted = remaining_mask >> start;
                let run_len = shifted.trailing_ones() as usize;

                for i in start..start + run_len {
                    let idx = base + i;
                    if let Some(boxed) = self.storage_ptrs[idx].take() {
                        drop(boxed);
                    }
                }

                remaining_mask &= !((1u64 << run_len) - 1) << start;
            }
        }
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

// (existing Drop above handles dropping scheduler first, then storages)
