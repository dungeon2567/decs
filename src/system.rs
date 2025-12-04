use crate::component::{Component, Destroyed};
use crate::storage::Storage;
use crate::world::World;
use decs::world::CleanupGroup;
use std::any::{Any, TypeId};
use std::marker::PhantomData;

pub trait SystemGroup: Any + Send + Sync + 'static {
    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    fn instance() -> &'static dyn SystemGroup
    where
        Self: Sized;

    fn before(&self) -> &'static [TypeId] {
        &[]
    }
    fn after(&self) -> &'static [TypeId] {
        &[]
    }
    fn reads(&self) -> &'static [TypeId] {
        &[]
    }
    fn writes(&self) -> &'static [TypeId] {
        &[]
    }

    fn parent(&self) -> Option<&dyn SystemGroup> {
        None
    }

    /// Returns a reference to the underlying Any trait object for downcasting.
    fn as_any(&self) -> &dyn Any;
}

pub trait System: Any + Send + Sync + 'static {
    fn run(&self, frame: &crate::frame::Frame);
    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
    fn before(&self) -> &[TypeId] {
        &[]
    }
    fn after(&self) -> &[TypeId] {
        &[]
    }
    fn reads(&self) -> &[TypeId] {
        &[]
    }
    fn writes(&self) -> &[TypeId] {
        &[]
    }

    fn parent(&self) -> Option<&dyn SystemGroup> {
        None
    }

    /// Returns a reference to the underlying Any trait object for downcasting.
    fn as_any(&self) -> &dyn Any;

    fn debug_counts(&self) -> (usize, usize) {
        (0, 0)
    }
}

pub struct ComponentCleanupSystem<T: Component> {
    pub writes: [TypeId; 1],
    pub t_storage: *mut Storage<T>,
    pub destroyed_storage: *const Storage<Destroyed>,
}

// Safety: DestroySystem is only used within the same thread context where the storages are valid.
// The raw pointers are only dereferenced in methods that ensure proper synchronization.
unsafe impl<T: Component> Send for ComponentCleanupSystem<T> {}
unsafe impl<T: Component> Sync for ComponentCleanupSystem<T> {}

impl<T: Component> ComponentCleanupSystem<T> {
    /// Creates a new ComponentCleanupSystem by getting storages from the world.
    pub fn new(world: &World) -> Self {
        // Get both storages as raw pointers (safe to obtain without unsafe)
        let t_ptr = world.get_storage_ptr::<T>();
        let d_ptr = world.get_storage_ptr::<Destroyed>() as *const Storage<Destroyed>;
        Self {
            writes: [TypeId::of::<T>()],
            t_storage: t_ptr,
            destroyed_storage: d_ptr,
        }
    }

    /// Creates a cleanup system from explicit storages, avoiding world aliasing.
    pub fn from_storages(
        t_storage: &mut Storage<T>,
        destroyed_storage: &Storage<Destroyed>,
    ) -> Self {
        Self {
            writes: [TypeId::of::<T>()],
            t_storage: t_storage as *mut Storage<T>,
            destroyed_storage: destroyed_storage as *const Storage<Destroyed>,
        }
    }

    /// Removes all T components from entities that have the Destroyed component.
    /// Iterates through the intersection of presence masks at each level (both T and Destroyed)
    /// and removes T components directly. Only entities that exist in both storages will be cleaned up.
    fn cleanup_destroyed_components(&self, frame: &crate::frame::Frame) {
        unsafe {
            let t_storage = &mut *self.t_storage;
            let destroyed_storage = &*self.destroyed_storage;

            let mut storage_mask = t_storage.presence_mask & destroyed_storage.presence_mask;

            while storage_mask != 0 {
                let storage_start = storage_mask.trailing_zeros() as usize;
                let shifted = storage_mask >> storage_start;
                let storage_run_len = shifted.trailing_ones() as usize;

                for storage_idx in (storage_start..storage_start + storage_run_len).rev() {
                    let t_page_mask = (&*t_storage.data[storage_idx]).presence_mask;
                    let destroyed_page_mask = (&*destroyed_storage.data[storage_idx]).presence_mask;
                    let mut page_mask_iter = t_page_mask & destroyed_page_mask;

                    while page_mask_iter != 0 {
                        let page_start = page_mask_iter.trailing_zeros() as usize;
                        let page_shifted = page_mask_iter >> page_start;
                        let page_run_len = page_shifted.trailing_ones() as usize;

                        for page_idx in (page_start..page_start + page_run_len).rev() {
                            let t_chunk_mask = {
                                let t_page = &*t_storage.data[storage_idx];
                                let t_chunk = &*t_page.data[page_idx];
                                t_chunk.presence_mask
                            };
                            let destroyed_chunk_mask = {
                                let d_page = &*destroyed_storage.data[storage_idx];
                                let d_chunk = &*d_page.data[page_idx];
                                d_chunk.presence_mask
                            };
                            let mut remove_mask = t_chunk_mask & destroyed_chunk_mask;
                            let page_mut = &mut *t_storage.data[storage_idx];

                            while remove_mask != 0 {
                                let chunk_start = remove_mask.trailing_zeros() as usize;
                                let chunk_shifted = remove_mask >> chunk_start;
                                let chunk_run_len = chunk_shifted.trailing_ones() as usize;

                                for chunk_idx in (chunk_start..chunk_start + chunk_run_len).rev() {
                                    let chunk_mut = &mut *page_mut.data[page_idx];
                                    let old_value = chunk_mut.data[chunk_idx].assume_init_read();

                                    let was_created_in_rollback =
                                        if t_storage.rollback.tick() != frame.current_tick {
                                            false
                                        } else if let Some(rb_page) =
                                            t_storage.rollback.get_page(storage_idx as u32)
                                        {
                                            if let Some(rb_chunk) = rb_page.get(page_idx as u32) {
                                                let has_created =
                                                    (rb_chunk.created_mask >> chunk_idx) & 1 != 0;
                                                let has_stored =
                                                    (rb_chunk.changed_mask >> chunk_idx) & 1 != 0
                                                        || (rb_chunk.removed_mask >> chunk_idx) & 1
                                                            != 0;
                                                has_created && !has_stored
                                            } else {
                                                false
                                            }
                                        } else {
                                            false
                                        };

                                    if !was_created_in_rollback {
                                        let rb_page = t_storage
                                            .rollback
                                            .get_or_create_page(storage_idx as u32);
                                        let rb_chunk = rb_page.get_or_create_chunk(page_idx as u32);
                                        if ((rb_chunk.changed_mask >> chunk_idx) & 1 == 0)
                                            && ((rb_chunk.removed_mask >> chunk_idx) & 1 == 0)
                                        {
                                            rb_chunk.data[chunk_idx].write(old_value);
                                        }
                                        rb_chunk.created_mask &= !(1u64 << chunk_idx);
                                        rb_chunk.changed_mask &= !(1u64 << chunk_idx);
                                        rb_chunk.removed_mask |= 1u64 << chunk_idx;
                                    } else if let Some(rb_page_mut) =
                                        t_storage.rollback.get_page_mut(storage_idx as u32)
                                        && let Some(rb_chunk_mut) =
                                            rb_page_mut.get_mut(page_idx as u32)
                                    {
                                        rb_chunk_mut.created_mask &= !(1u64 << chunk_idx);
                                        rb_chunk_mut.removed_mask &= !(1u64 << chunk_idx);
                                        rb_chunk_mut.changed_mask &= !(1u64 << chunk_idx);
                                    }

                                    chunk_mut.presence_mask &= !(1u64 << chunk_idx);
                                    chunk_mut.fullness_mask &= !(1u64 << chunk_idx);
                                    chunk_mut.changed_mask |= 1u64 << chunk_idx;
                                    page_mut.changed_mask |= 1u64 << page_idx;
                                    t_storage.changed_mask |= 1u64 << storage_idx;
                                    page_mut.count = page_mut.count.saturating_sub(1);
                                    t_storage.count = t_storage.count.saturating_sub(1);

                                    if chunk_mut.presence_mask == 0 {
                                        let _ = chunk_mut;
                                        debug_assert!(
                                            (page_mut.presence_mask >> page_idx) & 1 != 0
                                        );
                                        let t_storage = &mut *t_storage;
                                        if let Some((new_ptr, moved_idx)) =
                                            t_storage.chunk_pool.free_chunk(page_mut.data[page_idx])
                                        {
                                            page_mut.data[moved_idx as usize] = new_ptr;
                                        }
                                        let dc =
                                            <T as crate::component::Component>::default_chunk();
                                        page_mut.data[page_idx] = dc as *const _ as *mut _;
                                        page_mut.presence_mask &= !(1u64 << page_idx);
                                        page_mut.fullness_mask &= !(1u64 << page_idx);
                                    }
                                }

                                remove_mask &= !(((1u64 << chunk_run_len) - 1) << chunk_start);
                            }

                            let page_is_full = page_mut.count == 64 * 64;
                            if page_is_full {
                                t_storage.fullness_mask |= 1u64 << storage_idx;
                            } else {
                                t_storage.fullness_mask &= !(1u64 << storage_idx);
                            }
                            t_storage.fullness_mask &= t_storage.presence_mask;
                        }

                        page_mask_iter &= !(((1u64 << page_run_len) - 1) << page_start);
                    }
                }

                storage_mask &= !(((1u64 << storage_run_len) - 1) << storage_start);
            }

            t_storage.clear_changed_masks();
            debug_assert!(
                t_storage.verify_invariants(),
                "T storage invariants violated after cleanup"
            );
        }
    }
}

impl<T: Component> System for ComponentCleanupSystem<T> {
    fn run(&self, frame: &crate::frame::Frame) {
        self.cleanup_destroyed_components(frame);
    }

    fn reads(&self) -> &'static [TypeId] {
        static READS: &[TypeId] = &[std::any::TypeId::of::<Destroyed>()];
        READS
    }

    fn writes(&self) -> &[TypeId] {
        &self.writes
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn parent(&self) -> Option<&dyn SystemGroup> {
        Some(CleanupGroup::instance())
    }
}

pub struct TemporaryComponentCleanupSystem<T: Component, Group: SystemGroup> {
    pub writes: [TypeId; 1],
    pub t_storage: *mut Storage<T>,
    pub _group: PhantomData<Group>,
}

// Safety: TemporaryComponentCleanupSystem is only used within the same thread context where the storage is valid.
// The raw pointer is only dereferenced in methods that ensure proper synchronization.
unsafe impl<T: Component, Group: SystemGroup> Send for TemporaryComponentCleanupSystem<T, Group> {}
unsafe impl<T: Component, Group: SystemGroup> Sync for TemporaryComponentCleanupSystem<T, Group> {}

impl<T: Component, Group: SystemGroup> TemporaryComponentCleanupSystem<T, Group> {
    /// Creates a new TemporaryComponentCleanupSystem by getting storage from the world.
    pub fn new(world: &World) -> Self {
        let t_ptr = world.get_storage_ptr::<T>();

        Self {
            writes: [TypeId::of::<T>()],
            t_storage: t_ptr,
            _group: PhantomData,
        }
    }

    /// Clears the entire storage by dropping all components, chunks, and pages.
    /// This will drop everything in the storage unconditionally.
    fn cleanup_storage(&self) {
        unsafe {
            let t_storage = &mut *self.t_storage;

            // Iterate through all pages in reverse order to safely modify during iteration
            let mut storage_mask = t_storage.presence_mask;

            while storage_mask != 0 {
                let storage_start = storage_mask.trailing_zeros() as usize;
                let shifted = storage_mask >> storage_start;
                let storage_run_len = shifted.trailing_ones() as usize;

                // Process storage indices in reverse order
                for storage_idx in (storage_start..storage_start + storage_run_len).rev() {
                    // Read page mask first (immutable access)
                    // Note: page must exist since we're iterating based on storage_mask
                    let initial_page_mask = {
                        let t_page_ref = &*t_storage.data[storage_idx];
                        t_page_ref.presence_mask
                    };

                    // Iterate through all chunks in reverse order
                    let mut page_mask_iter = initial_page_mask;

                    while page_mask_iter != 0 {
                        let page_start = page_mask_iter.trailing_zeros() as usize;
                        let page_shifted = page_mask_iter >> page_start;
                        let page_run_len = page_shifted.trailing_ones() as usize;

                        // Process chunk indices in reverse order
                        for page_idx in (page_start..page_start + page_run_len).rev() {
                            // Get mutable reference to the chunk
                            // Note: chunk must exist since we're iterating based on page_mask_iter
                            let t_page = &mut *t_storage.data[storage_idx];
                            let t_chunk = &mut *t_page.data[page_idx];

                            // Drop all component data in the chunk
                            // Iterate through all present components and drop them
                            let mut chunk_mask = t_chunk.presence_mask;

                            while chunk_mask != 0 {
                                let chunk_start = chunk_mask.trailing_zeros() as usize;
                                let chunk_shifted = chunk_mask >> chunk_start;
                                let chunk_run_len = chunk_shifted.trailing_ones() as usize;

                                // Drop all components in this run and update masks
                                for chunk_component_idx in chunk_start..chunk_start + chunk_run_len
                                {
                                    // Drop the component data
                                    t_chunk.data[chunk_component_idx].assume_init_drop();

                                    // Update masks - remove value
                                    // Clear both presence_mask and fullness_mask
                                    t_chunk.presence_mask &= !(1u64 << chunk_component_idx);
                                    t_chunk.fullness_mask &= !(1u64 << chunk_component_idx);
                                    t_chunk.changed_mask |= 1u64 << chunk_component_idx;
                                }

                                chunk_mask &= !((1u64 << chunk_run_len) - 1) << chunk_start;
                            }

                            // Verify chunk invariants after updating all masks
                            debug_assert!(
                                t_chunk.verify_invariants(),
                                "Chunk invariants violated after dropping components"
                            );

                            // Ensure fullness_mask matches presence_mask (invariant)
                            t_chunk.fullness_mask = t_chunk.presence_mask;

                            // At this point, all slots should be empty (presence_mask=0, fullness_mask=0)
                            // Chunk should be empty: no values exist (presence_mask & !fullness_mask == 0)
                            debug_assert!(
                                (t_chunk.presence_mask & !t_chunk.fullness_mask) == 0,
                                "Chunk should be empty after cleanup"
                            );

                            // Drop the chunk itself
                            let _ = t_chunk;
                            debug_assert!((t_page.presence_mask >> page_idx) & 1 != 0);
                            if let Some((new_ptr, moved_idx)) =
                                t_storage.chunk_pool.free_chunk(t_page.data[page_idx])
                            {
                                t_page.data[moved_idx as usize] = new_ptr;
                            }
                            let dc = <T as crate::component::Component>::default_chunk();
                            t_page.data[page_idx] = dc as *const _ as *mut _;
                            // Clear chunk from page mask so Page::drop() won't try to drop it again
                            t_page.presence_mask &= !(1u64 << page_idx);
                            t_page.fullness_mask &= !(1u64 << page_idx);
                            // Note: count was already decremented when we removed values from the chunk
                        }

                        page_mask_iter &= !((1u64 << page_run_len) - 1) << page_start;
                    }

                    // Clear all page masks to 0 before dropping to prevent Page::drop() from trying to drop already-dropped chunks
                    {
                        let t_page_ref = &mut *t_storage.data[storage_idx];
                        t_page_ref.presence_mask = 0;
                        t_page_ref.fullness_mask = 0;
                        t_page_ref.changed_mask = 0;
                        t_page_ref.count = 0;
                    }

                    // Drop the page itself and reset pointer to default
                    debug_assert!((t_storage.presence_mask >> storage_idx) & 1 != 0);
                    if let Some((new_ptr, moved_idx)) =
                        t_storage.page_pool.free_page(t_storage.data[storage_idx])
                    {
                        t_storage.data[moved_idx as usize] = new_ptr;
                    }
                    let dp = <T as crate::component::Component>::default_page();
                    t_storage.data[storage_idx] = dp as *const _ as *mut _;

                    // Update storage masks to reflect that page is dropped
                    t_storage.presence_mask &= !(1u64 << storage_idx);
                    t_storage.fullness_mask &= !(1u64 << storage_idx);
                    t_storage.changed_mask &= !(1u64 << storage_idx);
                    // Note: count was already decremented when we removed values from chunks
                }

                storage_mask &= !((1u64 << storage_run_len) - 1) << storage_start;
            }

            // All pages have been dropped, so all masks should be 0
            // Just clear changed_mask to ensure it's 0 (presence_mask and fullness_mask should already be 0)
            t_storage.changed_mask = 0;
            t_storage.count = 0;

            // Verify invariants after cleanup
            debug_assert!(
                t_storage.verify_invariants(),
                "T storage invariants violated after cleanup"
            );
        }
    }
}

impl<T: Component, Group: SystemGroup> System for TemporaryComponentCleanupSystem<T, Group> {
    fn run(&self, _frame: &crate::frame::Frame) {
        self.cleanup_storage();
    }

    fn reads(&self) -> &[TypeId] {
        &[]
    }

    fn writes(&self) -> &[TypeId] {
        &self.writes
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn parent(&self) -> Option<&dyn SystemGroup> {
        Some(Group::instance())
    }
}
