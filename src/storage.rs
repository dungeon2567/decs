use crate::component::Component;
use crate::rollback::{RollbackStorage, VecQueue};
use crate::tick::Tick;
use std::any::Any;
use std::mem::MaybeUninit;

/// Trait for storage-like structures that can verify their invariants.
pub trait StorageLike: Any {
    /// Verifies that all invariants hold for this storage and all its nested structures.
    /// Also checks that all changed_mask values are 0 at every level.
    /// Returns true if all invariants are satisfied, false otherwise.
    fn verify_invariants(&self) -> bool;

    fn changed_mask_zero(&self) -> bool;

    fn clear_changed_masks_all_levels(&mut self);

    /// Rolls back this storage to the specified tick.
    /// This is a type-erased method that internally calls Storage<T>::rollback.
    fn rollback(&mut self, target_tick: Tick);

    /// Returns a reference to the underlying Any trait object for downcasting.
    fn as_any(&self) -> &dyn Any;

    /// Returns a mutable reference to the underlying Any trait object for downcasting.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// A hierarchical storage structure for efficiently storing and querying data.
///
/// The storage is organized in three levels:
/// - Storage contains 64 Pages
/// - Page contains 64 Chunks
/// - Chunk contains 64 values of type T
///
/// Total capacity: 64 * 64 * 64 = 262,144 items
///
/// # Mask Semantics
///
/// ## presence_mask
/// - For non-leaf nodes (Storage, Page): Bit at index `i` is set (1) if any child has at least 1 element
/// - Used to accelerate AND/OR queries by quickly identifying which indices have data
/// - Example: `presence_mask & query_mask` quickly finds matching indices
///
/// ## fullness_mask
/// - For non-leaf nodes (Storage, Page): Bit at index `i` is set (1) if all children are FULL
/// - Used to track when all children have reached their capacity
#[repr(align(64))]
pub struct Storage<T: Component> {
    pub presence_mask: u64,
    pub fullness_mask: u64,
    pub changed_mask: u64,
    pub count: u32,
    pub rollback: Box<RollbackStorage<T>>,
    pub prev: VecQueue<Box<RollbackStorage<T>>>,
    pub rollback_pool: Vec<Box<RollbackStorage<T>>>,
    pub data: [*mut Page<T>; 64],
    pub generation: u64,
    pub default_chunk_ptr: *const Chunk<T>,
    pub default_page_ptr: *const Page<T>,
}

impl<T: Component> Storage<T> {
    /// Creates a new empty Storage instance.
    pub fn new() -> Self {
        // Allocate default chunk (will be leaked intentionally as static default)
        let default_chunk_ptr: *const Chunk<T> = Box::into_raw(Box::new(Chunk::<T>::new()));

        // Allocate default page (will be leaked intentionally as static default)
        let default_page_ptr: *const Page<T> = Box::into_raw(Box::new(Page::<T> {
            presence_mask: 0,
            fullness_mask: 0,
            changed_mask: 0,
            count: 0,
            data: [default_chunk_ptr as *mut Chunk<T>; 64],
        }));
        Self {
            presence_mask: 0,
            fullness_mask: 0,
            changed_mask: 0,
            count: 0,
            rollback: Box::new(RollbackStorage::new()),
            prev: VecQueue::new(),
            rollback_pool: Vec::new(),
            data: [default_page_ptr as *mut Page<T>; 64],
            generation: 0, // Start at 0, will increment to 1 on first spawn (only used for Entity)
            default_chunk_ptr,
            default_page_ptr,
        }
    }

    #[inline]
    pub fn ensure_rollback_tick(&mut self, ct: Tick) {
        if self.rollback.tick() != ct {
            let new_current: Box<RollbackStorage<T>> =
                if let Some(mut pooled) = self.rollback_pool.pop() {
                    pooled.reset_for_tick(ct);
                    pooled
                } else {
                    Box::new(RollbackStorage::with_tick(ct))
                };
            let old = std::mem::replace(&mut self.rollback, new_current);
            let mut merged = std::mem::take(&mut self.prev);
            merged.push_back(old);
            self.prev = merged;

            // Limit rollback history to 64 ticks
            // This prevents unbounded memory growth but limits maximum rollback distance
            while self.prev.len() > 64 {
                self.prev.pop_front();
            }
        }
    }

    /// Gets a reference to a value at the given global index.
    /// Returns None if the value doesn't exist.
    #[inline(always)]
    pub fn get(&self, index: u32) -> Option<&T> {
        // Validate index is within valid range: 0 to 262,143 (64*64*64 - 1)
        if index >= 64 * 64 * 64 {
            return None;
        }

        let chunk_idx = index & 63;
        let page_idx = (index >> 6) & 63;
        let storage_idx = index >> 12;

        unsafe {
            let page_ptr = self.data[storage_idx as usize];

            let chunk_ptr = (*page_ptr).data[page_idx as usize];

            let bit = 1u64 << chunk_idx;

            if ((*chunk_ptr).presence_mask & bit) == 0 {
                return None;
            }

            Some((*chunk_ptr).data[chunk_idx as usize].assume_init_ref())
        }
    }

    /// Sets a value at the given global index.
    pub fn set(&mut self, frame: &crate::frame::Frame, index: u32, value: T) {
        let chunk_idx = index % 64;
        let page_idx = (index / 64) % 64;
        let storage_idx = index / (64 * 64);

        assert!(storage_idx < 64, "Storage index out of range");

        // Create page if needed
        let page_was_new = (self.presence_mask >> storage_idx) & 1 == 0;

        if page_was_new {
            let new_page = Box::new(Page::new(self.default_chunk_ptr));
            self.data[storage_idx as usize] = Box::into_raw(new_page);
            self.presence_mask |= 1u64 << storage_idx;
            self.changed_mask |= 1u64 << storage_idx;

            debug_assert!(
                self.fullness_mask & !self.presence_mask == 0,
                "Storage fullness_mask invariant violated after creating page"
            );
        }

        let (chunk_was_new, chunk_is_full, was_present, old_value) = {
            let page = unsafe { &mut *self.data[storage_idx as usize] };

            // Create chunk if needed
            let chunk_was_new = (page.presence_mask >> page_idx) & 1 == 0;

            if chunk_was_new {
                let new_chunk = Box::new(Chunk::new());
                page.data[page_idx as usize] = Box::into_raw(new_chunk);
                page.presence_mask |= 1u64 << page_idx;
                page.changed_mask |= 1u64 << page_idx;
                debug_assert!(
                    page.fullness_mask & !page.presence_mask == 0,
                    "Page fullness_mask invariant violated after creating chunk"
                );
            }

            let chunk = unsafe { &mut *page.data[page_idx as usize] };
            let was_present = (chunk.presence_mask >> chunk_idx) & 1 != 0;

            let old_value = if was_present {
                Some(unsafe { chunk.data[chunk_idx as usize].assume_init_read() })
            } else {
                None
            };

            chunk.data[chunk_idx as usize].write(value);
            chunk.presence_mask |= 1u64 << chunk_idx;
            chunk.fullness_mask |= 1u64 << chunk_idx; // fullness_mask == presence_mask at Chunk level
            chunk.changed_mask |= 1u64 << chunk_idx;

            let presence_mask = chunk.presence_mask;
            let chunk_is_full = presence_mask == u64::MAX;

            (chunk_was_new, chunk_is_full, was_present, old_value)
        };

        // set always changes state; rotate rollback if tick mismatched
        self.ensure_rollback_tick(frame.current_tick);

        // Update rollback storage at all levels
        // Reuse single page reference to avoid redundant get_or_create_page calls
        let rollback_page = self.rollback.get_or_create_page(storage_idx);
        let rollback_chunk = rollback_page.get_or_create_chunk(page_idx);

        // Check if item was removed or created earlier in this tick (idempotent operations)
        let was_removed = (rollback_chunk.removed_mask >> chunk_idx) & 1 != 0;
        let was_created = (rollback_chunk.created_mask >> chunk_idx) & 1 != 0;

        // Update chunk level
        if was_created {
            // Item was created earlier in this tick and is being modified
            // Created + modified on same tick should remain as created only
            // Don't store old value (created items don't have old values)
            // Keep created_mask, clear removed_mask and changed_mask
            rollback_chunk.removed_mask &= !(1u64 << chunk_idx);
            rollback_chunk.changed_mask &= !(1u64 << chunk_idx);
            rollback_chunk.created_mask |= 1u64 << chunk_idx;
        } else if was_present {
            // Value existed before this tick - this is a change, store old value
            // Only store the old value on the FIRST change within the tick
            if ((rollback_chunk.changed_mask >> chunk_idx) & 1 == 0)
                && ((rollback_chunk.removed_mask >> chunk_idx) & 1 == 0)
                && let Some(old_val) = old_value
            {
                rollback_chunk.data[chunk_idx as usize].write(old_val);
            }
            // Clear created_mask and removed_mask, set changed_mask
            rollback_chunk.removed_mask &= !(1u64 << chunk_idx);
            rollback_chunk.created_mask &= !(1u64 << chunk_idx);
            rollback_chunk.changed_mask |= 1u64 << chunk_idx;
        } else if was_removed {
            // Value didn't exist in Storage, but was removed earlier in this tick
            // This means the item EXISTED BEFORE and was successfully removed
            // This is an idempotent operation: Remove->Add = Change
            // The old value is already stored in RollbackStorage from the remove operation
            // Clear removed_mask and created_mask, set changed_mask
            rollback_chunk.removed_mask &= !(1u64 << chunk_idx);
            rollback_chunk.created_mask &= !(1u64 << chunk_idx);
            rollback_chunk.changed_mask |= 1u64 << chunk_idx;
        } else {
            // Value didn't exist before - this is a creation
            // Clear removed_mask and changed_mask, set created_mask
            rollback_chunk.removed_mask &= !(1u64 << chunk_idx);
            rollback_chunk.changed_mask &= !(1u64 << chunk_idx);
            rollback_chunk.created_mask |= 1u64 << chunk_idx;
        }

        // Update page level (reusing the same page reference)
        // Any change (creation, modification, or removal) sets changed_mask
        rollback_page.changed_mask |= 1u64 << page_idx;

        // Update rollback storage at storage level
        // Any change (creation, modification, or removal) sets changed_mask
        self.rollback.changed_mask |= 1u64 << storage_idx;

        // Re-acquire page reference after potential rollback rotation
        {
            let page = unsafe { &mut *self.data[storage_idx as usize] };

            // Update counts: increment only if this is a new value (not overwriting)
            if !was_present {
                page.count = page.count.saturating_add(1);
                self.count = self.count.saturating_add(1);
            }

            // Update page masks (presence_mask already set if chunk_was_new)
            if !chunk_was_new {
                page.presence_mask |= 1u64 << page_idx;
            }

            // Use count to determine if page is full: all 64 chunks exist AND all are full
            if chunk_is_full {
                page.fullness_mask |= 1u64 << page_idx;
            } else {
                page.fullness_mask &= !(1u64 << page_idx);
            }
            page.fullness_mask &= page.presence_mask;
            page.changed_mask |= 1u64 << page_idx;

            debug_assert!(
                page.fullness_mask & !page.presence_mask == 0,
                "Page fullness_mask invariant violated after mask update"
            );

            // Use count to determine if page is full: page.count == 64*64 (all slots filled)
            let page_is_full = page.count == 64 * 64;

            // Update storage masks (presence_mask already set if page_was_new)
            if !page_was_new {
                self.presence_mask |= 1u64 << storage_idx;
            }

            if page_is_full {
                self.fullness_mask |= 1u64 << storage_idx;
            } else {
                self.fullness_mask &= !(1u64 << storage_idx);
            }

            self.fullness_mask &= self.presence_mask;
            self.changed_mask |= 1u64 << storage_idx;
            debug_assert!(
                self.fullness_mask & !self.presence_mask == 0,
                "Storage fullness_mask invariant violated after mask update"
            );
        }

        // Verify rollback invariants
        if was_created {
            // Created + modified on same tick should remain as created
            debug_assert!(
                self.rollback.verify_was_created(index),
                "RollbackStorage invariant violated: index {} should be marked as created after set() (created+modified in same tick)",
                index
            );
        } else if was_present {
            debug_assert!(
                self.rollback.verify_was_modified(index),
                "RollbackStorage invariant violated: index {} should be marked as modified after set()",
                index
            );
        } else if was_removed {
            // Idempotent operation: remove+add = change
            debug_assert!(
                self.rollback.verify_was_modified(index),
                "RollbackStorage invariant violated: index {} should be marked as modified after idempotent set() (remove+add)",
                index
            );
        } else {
            debug_assert!(
                self.rollback.verify_was_created(index),
                "RollbackStorage invariant violated: index {} should be marked as created after set()",
                index
            );
        }
    }

    /// Removes a value at the given global index.
    /// Returns true if the value was removed, false if it didn't exist.
    pub fn remove(&mut self, frame: &crate::frame::Frame, index: u32) -> bool {
        let chunk_idx = index % 64;
        let page_idx = (index / 64) % 64;
        let storage_idx = index / (64 * 64);

        if storage_idx >= 64 {
            return false;
        }

        if (self.presence_mask >> storage_idx) & 1 == 0 {
            return false;
        }

        if page_idx >= 64 {
            return false;
        }

        let (chunk_has_present, old_value) = {
            let page = unsafe { &mut *self.data[storage_idx as usize] };
            if (page.presence_mask >> page_idx) & 1 == 0 {
                return false;
            }

            let chunk = unsafe { &mut *page.data[page_idx as usize] };

            // Check if value exists using only presence_mask
            if (chunk.presence_mask >> chunk_idx) & 1 == 0 {
                return false;
            }

            // Read old value for rollback before dropping
            let old_value = unsafe { chunk.data[chunk_idx as usize].assume_init_read() };

            // Only set changed_mask if we're actually removing a value
            page.changed_mask |= 1u64 << page_idx;

            // Remove value: clear both presence_mask and fullness_mask
            chunk.presence_mask &= !(1u64 << chunk_idx);
            chunk.fullness_mask &= !(1u64 << chunk_idx); // fullness_mask == presence_mask at Chunk level
            chunk.changed_mask |= 1u64 << chunk_idx;

            // Cache presence_mask before dropping mutable reference
            let chunk_has_present = chunk.presence_mask != 0;

            (chunk_has_present, old_value)
        };
        let was_created_in_rollback = if self.rollback.tick() != { frame.current_tick } {
            false
        } else {
            let rollback_page = self.rollback.get_or_create_page(storage_idx);
            let rollback_chunk = rollback_page.get_or_create_chunk(page_idx);
            let has_created_mask = (rollback_chunk.created_mask >> chunk_idx) & 1 != 0;
            let has_stored_value = (rollback_chunk.changed_mask >> chunk_idx) & 1 != 0
                || (rollback_chunk.removed_mask >> chunk_idx) & 1 != 0;
            has_created_mask && !has_stored_value
        };
        if !was_created_in_rollback {
            self.ensure_rollback_tick(frame.current_tick);
        }

        // Update page masks and counts after potential rollback rotation
        {
            let page = unsafe { &mut *self.data[storage_idx as usize] };
            if chunk_has_present {
                page.presence_mask |= 1u64 << page_idx;
            } else {
                page.presence_mask &= !(1u64 << page_idx);
            }
            // After removing a value, the chunk cannot be full, so clear fullness_mask
            page.fullness_mask &= !(1u64 << page_idx);
            page.fullness_mask &= page.presence_mask;
            page.changed_mask |= 1u64 << page_idx;

            // Update counts: decrement when removing a value (use saturating_sub to prevent overflow)
            page.count = page.count.saturating_sub(1);
            self.count = self.count.saturating_sub(1);
        }

        if !chunk_has_present {
            unsafe {
                let page = &mut *self.data[storage_idx as usize];
                // Drop owned chunk and reset pointer to default
                let chunk_ptr = page.data[page_idx as usize];
                if !std::ptr::eq(chunk_ptr, self.default_chunk_ptr) {
                    drop(Box::from_raw(chunk_ptr));
                }
                page.data[page_idx as usize] = self.default_chunk_ptr as *mut Chunk<T>;
                debug_assert!(
                    page.fullness_mask & !page.presence_mask == 0,
                    "Page fullness_mask invariant violated after dropping chunk"
                );
            }
        }

        // Update storage masks based on page state (compute after potentially dropping chunk)
        // Use presence_mask for optimal check: if presence_mask != 0, page has chunks
        let page_has_present = unsafe { (&*self.data[storage_idx as usize]).presence_mask != 0 };

        // Use count to determine if page is full: page.count == 64*64 (all slots filled)
        let page_is_full = unsafe { (&*self.data[storage_idx as usize]).count == 64 * 64 };

        if page_has_present {
            self.presence_mask |= 1u64 << storage_idx;
        } else {
            self.presence_mask &= !(1u64 << storage_idx);
        }

        if page_is_full {
            self.fullness_mask |= 1u64 << storage_idx;
        } else {
            self.fullness_mask &= !(1u64 << storage_idx);
        }

        self.fullness_mask &= self.presence_mask;
        self.changed_mask |= 1u64 << storage_idx;

        // Check if item was created in this tick (idempotent: Add->Remove = no change)
        // An item is considered "created in this tick" if it has created_mask set
        // and doesn't have a stored value (created items don't store values).
        // Note: clear_changed_masks() doesn't create a new rollback state, so we can't
        // use it as a boundary. Instead, we check if created_mask is set and no value is stored.
        // was_created_in_rollback already computed above

        // Update rollback storage at chunk level
        {
            let rollback_page = self.rollback.get_or_create_page(storage_idx);
            let rollback_chunk = rollback_page.get_or_create_chunk(page_idx);

            if was_created_in_rollback {
                // Add → Remove within the same tick: No change
                // Do NOT store a value; clear all masks for this index to enforce exclusivity
                // created items have no stored values; nothing to drop
                rollback_chunk.created_mask &= !(1u64 << chunk_idx);
                rollback_chunk.changed_mask &= !(1u64 << chunk_idx);
                rollback_chunk.removed_mask &= !(1u64 << chunk_idx);
            } else {
                // Item existed before this tick - this is a removal
                // Store old value ONLY if this is the first change within the tick
                if ((rollback_chunk.changed_mask >> chunk_idx) & 1 == 0)
                    && ((rollback_chunk.removed_mask >> chunk_idx) & 1 == 0)
                {
                    rollback_chunk.data[chunk_idx as usize].write(old_value);
                }

                // Clear created_mask and changed_mask, set removed_mask
                rollback_chunk.created_mask &= !(1u64 << chunk_idx);
                rollback_chunk.changed_mask &= !(1u64 << chunk_idx);
                rollback_chunk.removed_mask |= 1u64 << chunk_idx;
            }
        }

        // Update rollback storage at page and storage levels
        if was_created_in_rollback {
            // Add→Remove = no change: clear hierarchical changed_mask bits if no other changes remain
            if (self.rollback.changed_mask >> storage_idx) & 1 != 0 {
                let rollback_page = self.rollback.get_or_create_page(storage_idx);
                let chunk_has_other_changes = {
                    if let Some(chunk) = rollback_page.get(page_idx) {
                        (chunk.created_mask | chunk.changed_mask | chunk.removed_mask) != 0
                    } else {
                        false
                    }
                };
                if !chunk_has_other_changes {
                    // Clear page bit for this index
                    rollback_page.changed_mask &= !(1u64 << page_idx);
                    // If page has no other changed chunks, possibly clear storage-level bit
                    let page_has_other_changes = rollback_page.changed_mask != 0;
                    if !page_has_other_changes {
                        self.rollback.changed_mask &= !(1u64 << storage_idx);
                    }
                }
            }
        } else {
            // Removal is a change, so set changed_mask
            {
                let rollback_page = self.rollback.get_or_create_page(storage_idx);
                rollback_page.changed_mask |= 1u64 << page_idx;
            }
            self.rollback.changed_mask |= 1u64 << storage_idx;
        }

        // Check if page is empty (no chunks)
        if !page_has_present {
            // Page is empty - drop it and reset pointer to default
            // Note: self.presence_mask and self.fullness_mask were already updated above
            let page_ptr = self.data[storage_idx as usize];
            if !std::ptr::eq(page_ptr, self.default_page_ptr) {
                unsafe {
                    drop(Box::from_raw(page_ptr));
                }
            }
            self.data[storage_idx as usize] = self.default_page_ptr as *mut Page<T>;
            debug_assert!(
                self.fullness_mask & !self.presence_mask == 0,
                "Storage fullness_mask invariant violated after dropping page"
            );
        }

        // Verify rollback invariants (only if not idempotent)
        if !was_created_in_rollback {
            debug_assert!(
                self.rollback.verify_was_removed(index),
                "RollbackStorage invariant violated: index {} should be marked as removed after remove()",
                index
            );
        }

        true
    }

    pub fn rollback(&mut self, target_tick: Tick)
    where
        T: Clone,
    {
        // Efficient rollback using bitmasks to track visited indices.
        // This ensures at most 1 clone and 1 drop per index without HashMap/Vec allocations.

        // Find the generation value at or before target_tick
        // Search from newest to oldest: self.rollback -> self.prev.iter().rev()
        let mut found_generation = None;
        let all_rollbacks_rev = std::iter::once(&self.rollback).chain(self.prev.iter().rev());

        // We can stop searching once we find a tick <= target_tick
        for rb in all_rollbacks_rev {
            if rb.tick() <= target_tick {
                found_generation = Some(rb.get_saved_generation());
                break;
            }
        }

        // Build unified storage-level changed_mask (OR of all rollback states > target_tick)
        let mut unified_storage_mask = 0u64;

        // Optimization: iterate newest to oldest and stop when tick <= target_tick
        let relevant_rollbacks_rev = std::iter::once(&self.rollback)
            .chain(self.prev.iter().rev())
            .take_while(|rb| rb.tick() > target_tick);

        for rb in relevant_rollbacks_rev {
            unified_storage_mask |= rb.changed_mask;
        }

        // Iterate through each storage index that has changes
        let mut storage_mask = unified_storage_mask;

        while storage_mask != 0 {
            let storage_idx = storage_mask.trailing_zeros();
            storage_mask &= !(1u64 << storage_idx);

            // Build unified page-level changed_mask for this storage index
            let mut unified_page_mask = 0u64;

            // Re-create the iterator for this scope (borrow checker)
            let relevant_rollbacks_rev = std::iter::once(&self.rollback)
                .chain(self.prev.iter().rev())
                .take_while(|rb| rb.tick() > target_tick);

            for rb in relevant_rollbacks_rev {
                if let Some(rb_page) = rb.get_page(storage_idx) {
                    unified_page_mask |= rb_page.changed_mask;
                }
            }

            // Iterate through each page index that has changes
            let mut page_mask = unified_page_mask;

            while page_mask != 0 {
                let page_idx = page_mask.trailing_zeros();
                page_mask &= !(1u64 << page_idx);

                // visited_mask tracks which chunk indices we've already processed
                let mut visited_mask = 0u64;

                // Process rollback states from oldest to newest (self.prev is ordered oldest to newest)
                // Optimization: skip ticks <= target_tick used skip_while
                // Note: We MUST iterate Oldest -> Newest for correct "first modification" restoration logic
                let relevant_rollbacks = self
                    .prev
                    .iter()
                    .chain(std::iter::once(&self.rollback))
                    .skip_while(|rb| rb.tick() <= target_tick);

                for rb in relevant_rollbacks {
                    if let Some(rb_page) = rb.get_page(storage_idx)
                        && let Some(rb_chunk) = rb_page.get(page_idx)
                    {
                        // Process all three change types for this chunk
                        let combined_mask =
                            rb_chunk.created_mask | rb_chunk.changed_mask | rb_chunk.removed_mask;
                        let unvisited = combined_mask & !visited_mask;

                        let mut m = unvisited;

                        while m != 0 {
                            let chunk_idx = m.trailing_zeros();
                            m &= !(1u64 << chunk_idx);

                            // Mark as visited
                            visited_mask |= 1u64 << chunk_idx;

                            // Determine action based on this rollback state
                            let is_created = (rb_chunk.created_mask >> chunk_idx) & 1 != 0;

                            let chunk_idx_usize = chunk_idx as usize;
                            let page_idx_usize = page_idx as usize;
                            let storage_idx_usize = storage_idx as usize;

                            if is_created {
                                // Remove created item (at most 1 drop)
                                if (self.presence_mask >> storage_idx) & 1 != 0 {
                                    let page = unsafe { &mut *self.data[storage_idx_usize] };
                                    if (page.presence_mask >> page_idx) & 1 != 0 {
                                        let chunk = unsafe { &mut *page.data[page_idx_usize] };
                                        if (chunk.presence_mask >> chunk_idx) & 1 != 0 {
                                            unsafe {
                                                chunk.data[chunk_idx_usize].assume_init_drop();
                                            }
                                            chunk.presence_mask &= !(1u64 << chunk_idx);
                                            chunk.fullness_mask &= !(1u64 << chunk_idx);
                                            page.count = page.count.saturating_sub(1);
                                            self.count = self.count.saturating_sub(1);
                                            // chunk cannot be full after removal
                                            page.fullness_mask &= !(1u64 << page_idx);
                                            // drop chunk if it became empty
                                            if chunk.presence_mask == 0 {
                                                let chunk_ptr = page.data[page_idx_usize];
                                                if !std::ptr::eq(chunk_ptr, self.default_chunk_ptr)
                                                {
                                                    unsafe {
                                                        drop(Box::from_raw(chunk_ptr));
                                                    }
                                                }
                                                page.data[page_idx_usize] =
                                                    self.default_chunk_ptr as *mut Chunk<T>;
                                                page.presence_mask &= !(1u64 << page_idx);
                                            }
                                        }
                                    }
                                }
                            } else {
                                // Restore changed/removed item (at most 1 clone + 1 drop)
                                // Clone old value from rollback storage
                                let old_value = unsafe {
                                    rb_chunk.data[chunk_idx_usize].assume_init_ref().clone()
                                };

                                // Ensure page/chunk exist
                                if (self.presence_mask >> storage_idx) & 1 == 0 {
                                    let new_page = Box::new(Page::new(self.default_chunk_ptr));
                                    self.data[storage_idx_usize] = Box::into_raw(new_page);
                                    self.presence_mask |= 1u64 << storage_idx;
                                }

                                let page = unsafe { &mut *self.data[storage_idx_usize] };
                                if (page.presence_mask >> page_idx) & 1 == 0 {
                                    let new_chunk = Box::new(Chunk::new());
                                    page.data[page_idx_usize] = Box::into_raw(new_chunk);
                                    page.presence_mask |= 1u64 << page_idx;
                                }

                                let chunk = unsafe { &mut *page.data[page_idx_usize] };
                                let was_present = (chunk.presence_mask >> chunk_idx) & 1 != 0;

                                if was_present {
                                    unsafe {
                                        chunk.data[chunk_idx_usize].assume_init_drop();
                                    }
                                } else {
                                    page.count = page.count.saturating_add(1);
                                    self.count = self.count.saturating_add(1);
                                }

                                chunk.data[chunk_idx_usize].write(old_value);
                                chunk.presence_mask |= 1u64 << chunk_idx;
                                chunk.fullness_mask |= 1u64 << chunk_idx;
                                // update page fullness bit for this chunk
                                if chunk.presence_mask == u64::MAX {
                                    page.fullness_mask |= 1u64 << page_idx;
                                } else {
                                    page.fullness_mask &= !(1u64 << page_idx);
                                }
                            }
                        }
                    }
                }

                // finalize page-level masks and possibly drop empty page
                {
                    let storage_idx_usize = storage_idx as usize;
                    let page_ptr = self.data[storage_idx_usize];
                    if !std::ptr::eq(page_ptr, self.default_page_ptr) {
                        let page = unsafe { &mut *page_ptr };
                        // keep invariant: page.fullness_mask subset of presence
                        page.fullness_mask &= page.presence_mask;
                        if page.presence_mask == 0 {
                            unsafe {
                                drop(Box::from_raw(page_ptr));
                            }
                            self.data[storage_idx_usize] = self.default_page_ptr as *mut Page<T>;
                            self.presence_mask &= !(1u64 << storage_idx);
                            self.fullness_mask &= !(1u64 << storage_idx);
                        } else {
                            self.presence_mask |= 1u64 << storage_idx;
                            if page.count == 64 * 64 {
                                self.fullness_mask |= 1u64 << storage_idx;
                            } else {
                                self.fullness_mask &= !(1u64 << storage_idx);
                            }
                            self.fullness_mask &= self.presence_mask;
                        }
                    }
                }
            }
        }

        // Restore generation if found in rollback history
        if let Some(generation_value) = found_generation {
            self.generation = generation_value;
        }
        self.clear_changed_masks();
    }

    /// Clears the changed_mask at all levels (Storage, Page, and Chunk).
    /// This recursively clears changed_mask for all pages and chunks that have changes.
    /// Uses changed_mask & presence_mask to efficiently iterate only over changed items.
    /// Note: Does NOT clear rollback masks - those must persist for rollback operations.
    pub fn clear_changed_masks(&mut self) {
        let mut storage_mask = self.changed_mask & self.presence_mask;
        while storage_mask != 0 {
            let start = storage_mask.trailing_zeros() as usize;
            let shifted = storage_mask >> start;
            let run_len = shifted.trailing_ones() as usize;
            for i in start..start + run_len {
                let page = unsafe { &mut *self.data[i] };
                let mut page_mask = page.changed_mask & page.presence_mask;
                while page_mask != 0 {
                    let p_start = page_mask.trailing_zeros() as usize;
                    let p_shifted = page_mask >> p_start;
                    let p_run_len = p_shifted.trailing_ones() as usize;
                    for j in p_start..p_start + p_run_len {
                        let chunk = unsafe { &mut *page.data[j] };
                        chunk.changed_mask = 0;
                    }
                    page_mask &= !((u64::MAX >> (64 - p_run_len)) << p_start);
                }
                page.changed_mask = 0;
            }
            storage_mask &= !((u64::MAX >> (64 - run_len)) << start);
        }
        self.changed_mask = 0;
    }

    /// Verifies that all invariants hold for this Storage and all its Pages.
    /// Returns true if all invariants are satisfied, false otherwise.
    pub fn verify_invariants(&self) -> bool {
        // Verify Storage-level invariants
        if self.fullness_mask & !self.presence_mask != 0 {
            return false;
        }

        if self.count as usize > 64 * 64 * 64 {
            return false;
        }

        let mut total_count = 0u32;
        let mut mask = self.presence_mask;
        while mask != 0 {
            let start = mask.trailing_zeros() as usize;
            let shifted = mask >> start;
            let run_len = shifted.trailing_ones() as usize;
            for i in start..start + run_len {
                unsafe {
                    let page = &*self.data[i];
                    if !page.verify_invariants() {
                        return false;
                    }
                    total_count = total_count.saturating_add(page.count);
                }
            }
            mask &= !((u64::MAX >> (64 - run_len)) << start);
        }

        if total_count != self.count {
            return false;
        }

        true
    }
}

impl<T: Component> StorageLike for Storage<T> {
    fn verify_invariants(&self) -> bool {
        Storage::verify_invariants(self)
    }

    fn rollback(&mut self, target_tick: Tick) {
        Storage::rollback(self, target_tick)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn changed_mask_zero(&self) -> bool {
        self.changed_mask == 0
    }

    fn clear_changed_masks_all_levels(&mut self) {
        self.clear_changed_masks();
    }
}

impl<T: Component> Default for Storage<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl Storage<crate::entity::Entity> {
    /// Saves the current generation value to rollback storage.
    /// Should be called at the start of each tick.
    pub fn save_generation_for_rollback(&mut self) {
        self.rollback.save_generation(self.generation);
    }

    /// Rolls back the generation to the value saved at the start of the tick.
    /// Should be called when rolling back changes.
    pub fn rollback_generation(&mut self) {
        self.generation = self.rollback.get_saved_generation();
    }

    /// Finds the first zero bit (first free index) in a mask.
    /// Returns the index of the first 0 bit, or None if all bits are set.
    /// Uses trailing_ones() to find how many consecutive 1s are at the start,
    /// which directly gives us the position of the first 0.
    fn first_0_index(mask: u64) -> Option<usize> {
        let ones = mask.trailing_ones() as usize;
        if ones == 64 { None } else { Some(ones) }
    }

    /// Spawns a new entity by finding the first free index.
    ///
    /// Uses global generation counter that wraps at 64 bits.
    /// Returns `Some(Entity)` if a free slot was found, `None` if storage is full.
    pub fn spawn(&mut self, frame: &crate::frame::Frame) -> Option<crate::entity::Entity> {
        // Increment generation for new entity
        self.generation = self.generation.wrapping_add(1);

        // Find first non-full storage slot using mask
        if let Some(storage_idx) = Self::first_0_index(self.fullness_mask) {
            let storage_bit = 1u64 << storage_idx;

            // Ensure page exists
            if (self.presence_mask & storage_bit) == 0 {
                let new_page = Box::new(Page::new(self.default_chunk_ptr));
                self.data[storage_idx] = Box::into_raw(new_page);
                self.presence_mask |= storage_bit;
            }

            let page = unsafe { &mut *self.data[storage_idx] };

            // Find first non-full chunk in page using mask
            if let Some(page_idx) = Self::first_0_index(page.fullness_mask) {
                let page_bit = 1u64 << page_idx;

                // Ensure chunk exists
                if (page.presence_mask & page_bit) == 0 {
                    let new_chunk = Box::new(crate::storage::Chunk::new());
                    page.data[page_idx] = Box::into_raw(new_chunk);
                    page.presence_mask |= page_bit;
                }

                let chunk = unsafe { &mut *page.data[page_idx] };

                // Find first free index in chunk using mask
                if let Some(chunk_idx) = Self::first_0_index(chunk.fullness_mask) {
                    let global_index = (storage_idx * 64 * 64 + page_idx * 64 + chunk_idx) as u32;
                    let entity = crate::entity::Entity::new(global_index, self.generation);

                    // Set the entity in storage
                    self.set(frame, global_index, entity);

                    return Some(entity);
                }
            }
        }

        None // Storage is full
    }
}

impl Storage<crate::hierarchy::ChildOf> {
    pub fn set_parent(
        &mut self,
        frame: &crate::frame::Frame,
        child_index: u32,
        parent: crate::entity::Entity,
    ) {
        self.set_pending_parent_fast(frame, child_index, parent);
    }

    pub fn apply_pending_parent_changes(&mut self, frame: &crate::frame::Frame) {
        let mut storage_mask = self.presence_mask;
        while storage_mask != 0 {
            let storage_start = storage_mask.trailing_zeros() as usize;
            let storage_shifted = storage_mask >> storage_start;
            let storage_run_len = storage_shifted.trailing_ones() as usize;
            for storage_idx in storage_start..storage_start + storage_run_len {
                let page = unsafe { &*self.data[storage_idx] };
                let mut page_mask = page.presence_mask;
                while page_mask != 0 {
                    let page_start = page_mask.trailing_zeros() as usize;
                    let page_shifted = page_mask >> page_start;
                    let page_run_len = page_shifted.trailing_ones() as usize;
                    for page_idx in page_start..page_start + page_run_len {
                        let chunk = unsafe { &*page.data[page_idx] };
                        let mut chunk_mask = chunk.presence_mask;
                        while chunk_mask != 0 {
                            let chunk_start = chunk_mask.trailing_zeros() as usize;
                            let chunk_shifted = chunk_mask >> chunk_start;
                            let chunk_run_len = chunk_shifted.trailing_ones() as usize;
                            for chunk_idx in chunk_start..chunk_start + chunk_run_len {
                                let global_index =
                                    (storage_idx * 64 * 64 + page_idx * 64 + chunk_idx) as u32;
                                let current = unsafe {
                                    (&*page.data[page_idx]).data[chunk_idx]
                                        .assume_init_ref()
                                        .clone()
                                };
                                if let Some(p) = current.pending_parent {
                                    let mut updated = current;
                                    updated.parent = Some(p);
                                    updated.pending_parent = None;
                                    self.set(frame, global_index, updated);
                                }
                            }
                            chunk_mask &= !(((1u64 << chunk_run_len) - 1) << chunk_start);
                        }
                    }
                    page_mask &= !(((1u64 << page_run_len) - 1) << page_start);
                }
            }
            storage_mask &= !(((1u64 << storage_run_len) - 1) << storage_start);
        }
    }

    pub fn set_pending_parent_fast(
        &mut self,
        frame: &crate::frame::Frame,
        index: u32,
        parent: crate::entity::Entity,
    ) {
        let chunk_idx = index & 63;
        let page_idx = (index >> 6) & 63;
        let storage_idx = index >> 12;

        assert!(storage_idx < 64, "Storage index out of range");

        let bit = 1u64 << chunk_idx;

        let page_was_new = (self.presence_mask >> storage_idx) & 1 == 0;
        if page_was_new {
            let new_page = Box::new(Page::new(self.default_chunk_ptr));
            self.data[storage_idx as usize] = Box::into_raw(new_page);
            self.presence_mask |= 1u64 << storage_idx;
            self.changed_mask |= 1u64 << storage_idx;
        }

        let page = unsafe { &mut *self.data[storage_idx as usize] };
        let chunk_was_new = (page.presence_mask >> page_idx) & 1 == 0;
        if chunk_was_new {
            let new_chunk = Box::new(Chunk::new());
            page.data[page_idx as usize] = Box::into_raw(new_chunk);
            page.presence_mask |= 1u64 << page_idx;
            page.changed_mask |= 1u64 << page_idx;
        }

        let chunk = unsafe { &mut *page.data[page_idx as usize] };
        let was_present = (chunk.presence_mask & bit) != 0;

        // ensure rollback tick before writing
        self.ensure_rollback_tick(frame.current_tick);
        let rb_page = self.rollback.get_or_create_page(storage_idx);
        let rb_chunk = rb_page.get_or_create_chunk(page_idx);

        if was_present {
            // store old value once
            if (rb_chunk.changed_mask & bit) == 0 && (rb_chunk.removed_mask & bit) == 0 {
                let old_val = unsafe { chunk.data[chunk_idx as usize].assume_init_ref().clone() };
                rb_chunk.data[chunk_idx as usize].write(old_val);
            }
            rb_chunk.created_mask &= !bit;
            rb_chunk.removed_mask &= !bit;
            rb_chunk.changed_mask |= bit;

            // mark hierarchy on first change in this chunk
            if chunk.changed_mask == 0 {
                rb_page.changed_mask |= 1u64 << page_idx;
                self.rollback.changed_mask |= 1u64 << storage_idx;
            }

            // mutate in place
            let v_mut = unsafe { chunk.data[chunk_idx as usize].assume_init_mut() };
            v_mut.pending_parent = Some(parent);

            chunk.changed_mask |= bit;
        } else {
            // create new value
            let v = crate::hierarchy::ChildOf {
                parent: None,
                next_sibling: None,
                prev_sibling: None,
                pending_parent: Some(parent),
            };
            chunk.data[chunk_idx as usize].write(v);
            chunk.presence_mask |= bit;
            chunk.fullness_mask |= bit;
            chunk.changed_mask |= bit;

            page.count = page.count.saturating_add(1);
            self.count = self.count.saturating_add(1);

            // rollback mark as created
            rb_chunk.removed_mask &= !bit;
            rb_chunk.changed_mask &= !bit;
            rb_chunk.created_mask |= bit;
            rb_page.changed_mask |= 1u64 << page_idx;
            self.rollback.changed_mask |= 1u64 << storage_idx;
        }

        // update page and storage masks
        if !chunk_was_new {
            page.presence_mask |= 1u64 << page_idx;
        }
        if chunk.presence_mask == u64::MAX {
            page.fullness_mask |= 1u64 << page_idx;
        } else {
            page.fullness_mask &= !(1u64 << page_idx);
        }
        page.fullness_mask &= page.presence_mask;
        page.changed_mask |= 1u64 << page_idx;

        if !page_was_new {
            self.presence_mask |= 1u64 << storage_idx;
        }
        if page.count == 64 * 64 {
            self.fullness_mask |= 1u64 << storage_idx;
        } else {
            self.fullness_mask &= !(1u64 << storage_idx);
        }
        self.fullness_mask &= self.presence_mask;
        self.changed_mask |= 1u64 << storage_idx;
    }
}

impl<T: Component> Drop for Storage<T> {
    fn drop(&mut self) {
        // Drop all pages
        let mut mask = self.presence_mask;
        while mask != 0 {
            let start = mask.trailing_zeros() as usize;
            let shifted = mask >> start;
            let run_len = shifted.trailing_ones() as usize;
            for i in start..start + run_len {
                let page_ptr = self.data[i];
                if !std::ptr::eq(page_ptr, self.default_page_ptr) {
                    unsafe {
                        drop(Box::from_raw(page_ptr));
                    }
                }
            }
            mask &= !((u64::MAX >> (64 - run_len)) << start);
        }

        // Drop default pointers (intentionally leaked during new())
        unsafe {
            drop(Box::from_raw(self.default_chunk_ptr as *mut Chunk<T>));
            drop(Box::from_raw(self.default_page_ptr as *mut Page<T>));
        }
    }
}

/// A Page within a Storage, containing 64 Chunks.
///
/// # Mask Semantics
///
/// See Storage documentation for details on presence_mask and fullness_mask.
#[repr(align(64))]
pub struct Page<T: Component> {
    pub presence_mask: u64,
    pub fullness_mask: u64,
    pub changed_mask: u64,
    pub count: u32,
    pub data: [*mut Chunk<T>; 64],
}

impl<T: Component> Page<T> {
    /// Creates a new Page.
    pub fn new(default_chunk_ptr: *const Chunk<T>) -> Self {
        Self {
            presence_mask: 0,
            fullness_mask: 0,
            changed_mask: 0,
            count: 0,
            data: [default_chunk_ptr as *mut Chunk<T>; 64],
        }
    }

    /// Verifies that all invariants hold for this Page.
    pub fn verify_invariants(&self) -> bool {
        if self.fullness_mask & !self.presence_mask != 0 {
            return false;
        }

        if self.count as usize > 64 * 64 {
            return false;
        }

        let mut total_count = 0u32;
        let mut mask = self.presence_mask;
        while mask != 0 {
            let start = mask.trailing_zeros() as usize;
            let shifted = mask >> start;
            let run_len = shifted.trailing_ones() as usize;
            for i in start..start + run_len {
                unsafe {
                    let chunk = &*self.data[i];
                    if !chunk.verify_invariants() {
                        return false;
                    }
                    total_count = total_count.saturating_add(chunk.presence_mask.count_ones());
                }
            }
            mask &= !((u64::MAX >> (64 - run_len)) << start);
        }

        if total_count != self.count {
            return false;
        }

        true
    }
}

impl<T: Component> Drop for Page<T> {
    fn drop(&mut self) {
        // Only drop chunks where presence_mask indicates they exist
        let mut mask = self.presence_mask;
        while mask != 0 {
            let start = mask.trailing_zeros() as usize;
            let shifted = mask >> start;
            let run_len = shifted.trailing_ones() as usize;
            for i in start..start + run_len {
                unsafe {
                    drop(Box::from_raw(self.data[i]));
                }
            }
            mask &= !((u64::MAX >> (64 - run_len)) << start);
        }
    }
}

/// A Chunk containing 64 values.
#[repr(align(64))]
pub struct Chunk<T: Component> {
    pub presence_mask: u64,
    pub fullness_mask: u64,
    pub changed_mask: u64,
    pub data: [MaybeUninit<T>; 64],
}

impl<T: Component> Default for Chunk<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Component> Chunk<T> {
    /// Creates a new empty Chunk.
    pub fn new() -> Self {
        Self {
            presence_mask: 0,
            fullness_mask: 0,
            changed_mask: 0,
            data: [const { MaybeUninit::uninit() }; 64],
        }
    }

    /// Verifies that all invariants hold for this Chunk.
    pub fn verify_invariants(&self) -> bool {
        // At chunk level, fullness_mask should equal presence_mask
        self.fullness_mask == self.presence_mask
    }
}

impl<T: Component> Drop for Chunk<T> {
    fn drop(&mut self) {
        // Only drop values where presence_mask indicates they exist
        let mut mask = self.presence_mask;
        while mask != 0 {
            let start = mask.trailing_zeros() as usize;
            let shifted = mask >> start;
            let run_len = shifted.trailing_ones() as usize;
            for i in start..start + run_len {
                unsafe {
                    self.data[i].assume_init_drop();
                }
            }
            mask &= !((u64::MAX >> (64 - run_len)) << start);
        }
    }
}
