use crate::component::Component;
use crate::rollback::{RollbackStorage, VecQueue};
use crate::tick::Tick;
use std::any::Any;
use std::mem::MaybeUninit;

use crate::pool::{Pool, PoolPage};

/// Trait for storage-like structures that can verify their invariants.
pub trait StorageLike: Any {
    /// Verifies that all invariants hold for this storage and all its nested structures.
    /// Also checks that all changed_mask values are 0 at every level.
    /// Returns true if all invariants are satisfied, false otherwise.
    fn verify_invariants(&self) -> bool;

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
pub struct Storage<T: Component> {
    pub presence_mask: u64,
    pub fullness_mask: u64,
    pub changed_mask: u64,
    pub count: u32,
    pub rollback: Box<RollbackStorage<T>>,
    pub prev: VecQueue<Box<RollbackStorage<T>>>,
    pub rollback_pool: Vec<Box<RollbackStorage<T>>>,
    pub page_pool: Pool<Page<T>>,
    pub chunk_pool: Pool<Chunk<T>>,
    pub data: [*mut Page<T>; 64],
    pub generation: u64,
    pub default_chunk_box: Box<Chunk<T>>,
    pub default_page_box: Box<Page<T>>,
    pub default_chunk_ptr: *const Chunk<T>,
    pub default_page_ptr: *const Page<T>,
}

impl<T: Component> Storage<T> {
    /// Creates a new empty Storage instance.
    pub fn new() -> Self {
        let default_chunk_box_tmp = Box::new(Chunk::<T>::new());
        let default_chunk_ptr: *const Chunk<T> = Box::into_raw(default_chunk_box_tmp) as *const Chunk<T>;
        let default_chunk_box = unsafe { Box::from_raw(default_chunk_ptr as *mut Chunk<T>) };
        let default_page_box_tmp = Box::new(Page::<T> {
            presence_mask: 0,
            fullness_mask: 0,
            changed_mask: 0,
            count: 0,
            data: [default_chunk_ptr as *mut Chunk<T>; 64],
            chunk_pool: std::ptr::null_mut(),
            pool_slot: 0,
            pool_page: std::ptr::null_mut(),
            owner_index: 0,
        });
        let default_page_ptr: *const Page<T> = Box::into_raw(default_page_box_tmp) as *const Page<T>;
        let default_page_box = unsafe { Box::from_raw(default_page_ptr as *mut Page<T>) };
        Self {
            presence_mask: 0,
            fullness_mask: 0,
            changed_mask: 0,
            count: 0,
            rollback: Box::new(RollbackStorage::new()),
            prev: VecQueue::new(),
            rollback_pool: Vec::new(),
            page_pool: Pool::new(),
            chunk_pool: Pool::new(),
            data: [default_page_ptr as *mut Page<T>; 64],
            generation: 0, // Start at 0, will increment to 1 on first spawn (only used for Entity)
            default_chunk_box,
            default_page_box,
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
            while self.prev.len() > 64 {
                self.prev.pop_front();
            }
        }
    }

    /// Gets a reference to a value at the given global index.
    /// Returns None if the value doesn't exist.
    #[inline(always)]
    pub fn get(&self, index: u32) -> Option<&T> {
        let chunk_idx = (index & 63) as u32;
        let page_idx = ((index >> 6) & 63) as u32;
        let storage_idx = index >> 12;

        if storage_idx >= 64 {
            return None;
        }

        unsafe {
            let page_ptr = self.data[storage_idx as usize];
            let chunk_ptr = (*page_ptr).data[page_idx as usize];
            if chunk_ptr == self.default_chunk_ptr as *mut Chunk<T> {
                return None;
            }
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
            let chunk_pool_ptr = &mut self.chunk_pool as *mut _;
            let _ = self.page_pool.alloc_page_to_slot(
                storage_idx as u8,
                &mut self.data[storage_idx as usize],
                chunk_pool_ptr,
                self.default_chunk_ptr,
            );
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
                let _ = self
                    .chunk_pool
                    .alloc_chunk_to_slot(page_idx as u8, &mut page.data[page_idx as usize]);
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

        // Update rollback storage at all levels (reuse page reference to avoid calling get_or_create_page twice)
        // Check if item was removed or created earlier in this tick (idempotent operations) before updating masks
        let (was_removed_in_rollback, was_created_in_rollback) = {
            let rollback_page = self.rollback.get_or_create_page(storage_idx);
            let rollback_chunk = rollback_page.get_or_create_chunk(page_idx);
            (
                (rollback_chunk.removed_mask >> chunk_idx) & 1 != 0,
                (rollback_chunk.created_mask >> chunk_idx) & 1 != 0,
            )
        };

        {
            let rollback_page = self.rollback.get_or_create_page(storage_idx);
            let rollback_chunk = rollback_page.get_or_create_chunk(page_idx);

            // Check if item was removed or created earlier in this tick (idempotent operations)
            let was_removed = was_removed_in_rollback;
            let was_created = was_created_in_rollback;

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
        }

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
        if was_created_in_rollback {
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
        } else if was_removed_in_rollback {
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
                self.chunk_pool.free(page.data[page_idx as usize]);
                let dc = self.default_chunk_ptr as *mut Chunk<T>;
                page.data[page_idx as usize] = dc;
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
            self.page_pool.free(self.data[storage_idx as usize]);
            let dp = self.default_page_box.as_mut() as *mut Page<T>;
            self.data[storage_idx as usize] = dp;
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
        // Collect rollback snapshots in chronological order from queue (oldest to newest)
        let mut chain: [MaybeUninit<&RollbackStorage<T>>; 32] =
            unsafe { MaybeUninit::uninit().assume_init() };
        let mut chain_len = 0usize;
        for prev in self.prev.iter() {
            if prev.tick() > target_tick {
                if chain_len < 32 {
                    chain[chain_len].write(prev);
                    chain_len += 1;
                } else {
                    break;
                }
            }
        }
        let current = &self.rollback;
        if current.tick() > target_tick && chain_len < 32 {
            chain[chain_len].write(current);
            chain_len += 1;
        }

        // Track which bits have been processed per (storage page, page chunk)
        let mut union_storage_mask = 0u64;
        for item in chain.iter().take(chain_len) {
            union_storage_mask |= unsafe { item.assume_init() }.changed_mask;
        }
        while union_storage_mask != 0 {
            let storage_start = union_storage_mask.trailing_zeros() as usize;
            let shifted = union_storage_mask >> storage_start;
            let storage_run_len = shifted.trailing_ones() as usize;

            for s in storage_start..storage_start + storage_run_len {
                if (self.presence_mask >> s) & 1 == 0 {
                    let chunk_pool_ptr = &mut self.chunk_pool as *mut _;
                    let _ = self.page_pool.alloc_page_to_slot(
                        s as u8,
                        &mut self.data[s],
                        chunk_pool_ptr,
                        self.default_chunk_ptr,
                    );
                    self.presence_mask |= 1u64 << s;
                }
                let page = unsafe { &mut *self.data[s] };

                let mut union_page_mask = 0u64;
                for item in chain.iter().take(chain_len) {
                    if let Some(rb_page) = unsafe { item.assume_init() }.get_page(s as u32) {
                        union_page_mask |= rb_page.changed_mask;
                    }
                }

                while union_page_mask != 0 {
                    let page_start = union_page_mask.trailing_zeros() as usize;
                    let shifted_p = union_page_mask >> page_start;
                    let page_run_len = shifted_p.trailing_ones() as usize;

                    for p in page_start..page_start + page_run_len {
                        if (page.presence_mask >> p) & 1 == 0 {
                            let _ = self
                                .chunk_pool
                                .alloc_chunk_to_slot(p as u8, &mut page.data[p]);
                            page.presence_mask |= 1u64 << p;
                        }
                        let is_zst = std::mem::size_of::<T>() == 0;
                        if is_zst {
                            let old_presence = {
                                let chunk = unsafe { &mut *page.data[p] };
                                chunk.presence_mask
                            };
                            let mut new_presence = old_presence;
                            for item in chain.iter().take(chain_len) {
                                if let Some(rb_page) =
                                    unsafe { item.assume_init() }.get_page(s as u32)
                                    && let Some(rb_chunk) = rb_page.get(p as u32)
                                {
                                    new_presence &= !rb_chunk.created_mask;
                                    new_presence |= rb_chunk.removed_mask | rb_chunk.changed_mask;
                                }
                            }
                            if new_presence != old_presence {
                                let old_pop = old_presence.count_ones();
                                let new_pop = new_presence.count_ones();
                            if new_presence == 0 {
                                unsafe {
                                    if let Some((new_ptr, moved_idx)) =
                                        self.chunk_pool.free_chunk(page.data[p])
                                    {
                                        page.data[moved_idx as usize] = new_ptr;
                                    }
                                    let dc = self.default_chunk_ptr as *mut Chunk<T>;
                                    page.data[p] = dc;
                                }
                                page.presence_mask &= !(1u64 << p);
                                page.fullness_mask &= !(1u64 << p);
                            } else {
                                    let chunk = unsafe { &mut *page.data[p] };
                                    chunk.presence_mask = new_presence;
                                    chunk.fullness_mask = new_presence;
                                }
                                let delta = new_pop as i32 - old_pop as i32;
                                if delta > 0 {
                                    let d = delta as u32;
                                    page.count = page.count.saturating_add(d);
                                    self.count = self.count.saturating_add(d);
                                } else if delta < 0 {
                                    let d = (-delta) as u32;
                                    page.count = page.count.saturating_sub(d);
                                    self.count = self.count.saturating_sub(d);
                                }
                            }
                        } else {
                            let chunk = unsafe { &mut *page.data[p] };
                            let mut processed_chunk = 0u64;
                            let mut pending_remove = 0u64;
                            for item in chain.iter().take(chain_len) {
                                if let Some(rb_page) =
                                    unsafe { item.assume_init() }.get_page(s as u32)
                                    && let Some(rb_chunk) = rb_page.get(p as u32)
                                {
                                    let to_restore = (rb_chunk.changed_mask
                                        | rb_chunk.removed_mask)
                                        & !processed_chunk;
                                    let mut mask = to_restore;
                                    while mask != 0 {
                                        let start = mask.trailing_zeros() as usize;
                                        let shifted = mask >> start;
                                        let run_len = shifted.trailing_ones() as usize;
                                        for i in start..start + run_len {
                                            let bit = 1u64 << i;
                                            if (chunk.presence_mask >> i) & 1 != 0 {
                                                unsafe { chunk.data[i].assume_init_drop() };
                                            } else {
                                                page.count = page.count.saturating_add(1);
                                                self.count = self.count.saturating_add(1);
                                            }
                                            let old_ref =
                                                unsafe { rb_chunk.data[i].assume_init_ref() };
                                            chunk.data[i].write(old_ref.clone());
                                            chunk.presence_mask |= bit;
                                            chunk.fullness_mask |= bit;
                                            processed_chunk |= bit;
                                        }
                                        mask &= !((u64::MAX >> (64 - run_len)) << start);
                                    }
                                    pending_remove |= rb_chunk.created_mask & !processed_chunk;
                                }
                            }
                            // Apply removals in one pass
                            let mut rem = pending_remove;
                            while rem != 0 {
                                let start = rem.trailing_zeros() as usize;
                                let shifted = rem >> start;
                                let run_len = shifted.trailing_ones() as usize;
                                for i in start..start + run_len {
                                    let bit = 1u64 << i;
                                    if (chunk.presence_mask >> i) & 1 != 0 {
                                        unsafe { chunk.data[i].assume_init_drop() };
                                        page.count = page.count.saturating_sub(1);
                                        self.count = self.count.saturating_sub(1);
                                    }
                                    chunk.presence_mask &= !bit;
                                    chunk.fullness_mask &= !bit;
                                }
                                rem &= !((u64::MAX >> (64 - run_len)) << start);
                            }
                        }

                        if is_zst {
                            // handled in branch
                            if (page.presence_mask >> p) & 1 != 0 {
                                let chunk = unsafe { &mut *page.data[p] };
                                if chunk.presence_mask == u64::MAX {
                                    page.fullness_mask |= 1u64 << p;
                                } else {
                                    page.fullness_mask &= !(1u64 << p);
                                }
                            }
                        } else if (page.presence_mask >> p) & 1 != 0 {
                            let chunk = unsafe { &mut *page.data[p] };
                            if chunk.presence_mask == 0 {
                                unsafe {
                                    if let Some((new_ptr, moved_idx)) =
                                        self.chunk_pool.free_chunk(page.data[p])
                                    {
                                        page.data[moved_idx as usize] = new_ptr;
                                    }
                                    let dc = self.default_chunk_box.as_mut() as *mut Chunk<T>;
                                    page.data[p] = dc;
                                }
                                page.presence_mask &= !(1u64 << p);
                                page.fullness_mask &= !(1u64 << p);
                            } else if chunk.presence_mask == u64::MAX {
                                page.fullness_mask |= 1u64 << p;
                            } else {
                                page.fullness_mask &= !(1u64 << p);
                            }
                        }
                    }

                    union_page_mask &= !((u64::MAX >> (64 - page_run_len)) << page_start);
                }

                if page.presence_mask == 0 {
                    unsafe {
                        if let Some((new_ptr, moved_idx)) = self.page_pool.free_page(self.data[s]) {
                            self.data[moved_idx as usize] = new_ptr;
                        }
                        let dp = self.default_page_ptr as *mut Page<T>;
                        self.data[s] = dp;
                    }
                    self.presence_mask &= !(1u64 << s);
                    self.fullness_mask &= !(1u64 << s);
                } else if page.count == 64 * 64 {
                    self.fullness_mask |= 1u64 << s;
                } else {
                    self.fullness_mask &= !(1u64 << s);
                }
            }

            union_storage_mask &= !((u64::MAX >> (64 - storage_run_len)) << storage_start);
        }

        self.generation = self.rollback.get_saved_generation();
        self.clear_changed_masks();
        self.fullness_mask &= self.presence_mask;
        self.rollback.set_tick(target_tick);

        {
            let len = self.prev.len();
            let mut kept = crate::rollback::VecQueue::with_capacity(len);
            for _ in 0..len {
                if let Some(rb) = self.prev.pop_front() {
                    if rb.tick() > target_tick {
                        self.rollback_pool.push(rb);
                    } else {
                        kept.push_back(rb);
                    }
                }
            }
            self.prev = kept;
        }

        debug_assert!(
            self.verify_invariants(),
            "Storage invariants violated after rollback()"
        )
    }

    /// Clears the changed_mask at all levels (Storage, Page, and Chunk).
    /// This recursively clears changed_mask for all pages and chunks that have changes.
    /// Uses changed_mask & presence_mask to efficiently iterate only over changed items.
    /// Note: Does NOT clear rollback masks - those must persist for rollback operations.
    pub fn clear_changed_masks(&mut self) {
        // Use intersection of changed_mask and presence_mask to only visit changed pages
        let mut storage_mask = self.changed_mask & self.presence_mask;

        while storage_mask != 0 {
            let storage_start = storage_mask.trailing_zeros() as usize;
            let shifted = storage_mask >> storage_start;
            let storage_run_len = shifted.trailing_ones() as usize;

            for storage_idx in storage_start..storage_start + storage_run_len {
                unsafe {
                    let page = &mut *self.data[storage_idx];
                    page.clear_changed_masks();
                }
            }

            storage_mask &= !((u64::MAX >> (64 - storage_run_len)) << storage_start);
        }

        // Clear storage level changed_mask after clearing all nested masks
        self.changed_mask = 0;
    }

    /// Verifies that all invariants hold for this Storage and all its Pages.
    /// Returns true if all invariants are satisfied, false otherwise.
    pub fn verify_invariants(&self) -> bool {
        // For non-leaf nodes, fullness_mask can only be set where presence_mask is also set
        if self.fullness_mask & !self.presence_mask != 0 {
            return false;
        }

        // Verify count matches total number of values (sum of all page counts)
        let mut total_count = 0u32;
        for i in 0..64 {
            if (self.presence_mask >> i) & 1 != 0 {
                unsafe {
                    let page_ptr = self.data[i];
                    let count = std::ptr::addr_of!((*page_ptr).count).read();
                    total_count = total_count.saturating_add(count);
                }
            }
        }
        if self.count != total_count {
            return false;
        }

        // If storage is full (count == 64*64*64), verify fullness_mask is correct
        if self.count == 64 * 64 * 64 {
            // Storage is full if all pages are full: fullness_mask == presence_mask
            if self.fullness_mask != self.presence_mask {
                return false;
            }
        }

        for i in 0..64 {
            if (self.presence_mask >> i) & 1 != 0 {
                unsafe {
                    let page_ptr = self.data[i];
                    if !Page::<T>::verify_invariants_ptr(page_ptr) {
                        return false;
                    }
                    let count = std::ptr::addr_of!((*page_ptr).count).read();
                    if (self.fullness_mask >> i) & 1 != 0 && count != 64 * 64 {
                        return false;
                    }
                }
            }
        }

        true
    }
}

impl<T: Component> StorageLike for Storage<T> {
    fn verify_invariants(&self) -> bool {
        // Check that changed_mask is 0
        if self.changed_mask != 0 {
            return false;
        }

        // Check basic mask invariants
        // For non-leaf nodes, fullness_mask can only be set where presence_mask is also set
        if self.fullness_mask & !self.presence_mask != 0 {
            return false;
        }

        // Verify count matches total number of values (sum of all page counts)
        let mut total_count = 0u32;
        for i in 0..64 {
            if (self.presence_mask >> i) & 1 != 0 {
                unsafe {
                    let page_ptr = self.data[i];
                    let count = std::ptr::addr_of!((*page_ptr).count).read();
                    total_count = total_count.saturating_add(count);
                }
            }
        }
        if self.count != total_count {
            return false;
        }

        // If storage is full (count == 64*64*64), verify fullness_mask is correct
        if self.count == 64 * 64 * 64 {
            // Storage is full if all pages are full: fullness_mask == presence_mask
            if self.fullness_mask != self.presence_mask {
                return false;
            }
        }

        // Recursively verify all pages and chunks
        for i in 0..64 {
            if (self.presence_mask >> i) & 1 != 0 {
                unsafe {
                    let page_ptr = self.data[i];
                    let changed = std::ptr::addr_of!((*page_ptr).changed_mask).read();
                    if changed != 0 {
                        return false;
                    }
                    if !Page::<T>::verify_invariants_ptr(page_ptr) {
                        return false;
                    }
                    let count = std::ptr::addr_of!((*page_ptr).count).read();
                    if (self.fullness_mask >> i) & 1 != 0 && count != 64 * 64 {
                        return false;
                    }
                }
            }
        }

        true
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
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
    #[inline(always)]
    fn first_0_index(mask: u64) -> Option<usize> {
        if mask == u64::MAX {
            None
        } else {
            Some(mask.trailing_ones() as usize)
        }
    }

    /// Spawns a new entity by finding the first free index.
    ///
    /// Uses global generation counter that wraps at 64 bits.
    /// Returns `Some(Entity)` if a free slot was found, `None` if storage is full.
    pub fn spawn(&mut self, frame: &crate::frame::Frame) -> Option<crate::entity::Entity> {
        use crate::entity::Entity;

        // Increment global generation (wraps at 64 bits, skip 0)
        self.generation = self.generation.wrapping_add(1);

        if self.generation == 0 {
            self.generation = 1; // Skip 0 (reserved for none)
        }

        let generation = self.generation;

        // Find first non-full page using first_0_index on fullness_mask (O(1))
        let storage_idx = Self::first_0_index(self.fullness_mask)?;

        // Prepare indices and update page/chunk state within a limited borrow scope
        let (page_idx, chunk_idx, chunk_is_full) = {
            let page = if (self.presence_mask >> storage_idx) & 1 != 0 {
                unsafe { &mut *self.data[storage_idx] }
            } else {
                let chunk_pool_ptr = &mut self.chunk_pool as *mut _;
                let _ = self.page_pool.alloc_page_to_slot(
                    storage_idx as u8,
                    &mut self.data[storage_idx],
                    chunk_pool_ptr,
                    self.default_chunk_ptr,
                );
                self.presence_mask |= 1u64 << storage_idx;
                self.changed_mask |= 1u64 << storage_idx;
                debug_assert!(
                    self.fullness_mask & !self.presence_mask == 0,
                    "Storage fullness_mask invariant violated after creating page"
                );
                unsafe { &mut *self.data[storage_idx] }
            };

            // Find first non-full chunk using first_0_index on page.fullness_mask (O(1))
            let page_idx = Self::first_0_index(page.fullness_mask)?;

            // Create chunk if it doesn't exist
            let chunk = if (page.presence_mask >> page_idx) & 1 != 0 {
                unsafe { &mut *page.data[page_idx] }
            } else {
                let _ = self
                    .chunk_pool
                    .alloc_chunk_to_slot(page_idx as u8, &mut page.data[page_idx]);
                page.presence_mask |= 1u64 << page_idx;
                page.changed_mask |= 1u64 << page_idx;
                debug_assert!(
                    page.fullness_mask & !page.presence_mask == 0,
                    "Page fullness_mask invariant violated after creating chunk"
                );
                unsafe { &mut *page.data[page_idx] }
            };

            // Find first free slot using first_0_index on chunk.presence_mask (O(1))
            let chunk_idx = Self::first_0_index(chunk.presence_mask)? as u32;

            // Write entity directly to chunk (spawn always creates new entity, was_present = false)
            // Note: we don't store old value as this is a new creation
            // The borrow of `page` and `chunk` ends after this block
            // Update chunk masks
            chunk.presence_mask |= 1u64 << chunk_idx;
            chunk.fullness_mask |= 1u64 << chunk_idx; // fullness_mask == presence_mask at Chunk level
            chunk.changed_mask |= 1u64 << chunk_idx;

            let chunk_is_full = chunk.presence_mask == u64::MAX;
            (page_idx, chunk_idx, chunk_is_full)
        };

        let global_index = (storage_idx as u32) * (64 * 64) + (page_idx as u32) * 64 + chunk_idx;

        // Create entity with global generation
        let entity = Entity::new(global_index, generation);

        // Write entity value now that we have the index
        {
            let page = unsafe { &mut *self.data[storage_idx] };
            let chunk = unsafe { &mut *page.data[page_idx] };
            chunk.data[chunk_idx as usize].write(entity);
        }

        // spawn always changes state; rotate rollback if tick mismatched
        self.ensure_rollback_tick(frame.current_tick);

        // Update rollback storage at chunk level
        {
            let rollback_page = self.rollback.get_or_create_page(storage_idx as u32);
            let rollback_chunk = rollback_page.get_or_create_chunk(page_idx as u32);

            // Check if item was removed earlier in this tick (idempotent operations)
            let was_removed = (rollback_chunk.removed_mask >> chunk_idx) & 1 != 0;

            if was_removed {
                // Item was removed earlier in this tick and is being spawned (re-added)
                // This is an idempotent operation: Remove->Add = Change
                // The old value is already stored in RollbackStorage from the remove operation
                // Clear removed_mask and created_mask, set changed_mask
                rollback_chunk.removed_mask &= !(1u64 << chunk_idx);
                rollback_chunk.created_mask &= !(1u64 << chunk_idx);
                rollback_chunk.changed_mask |= 1u64 << chunk_idx;
            } else {
                // Normal spawn - new entity
                // Clear removed_mask and changed_mask, set created_mask
                rollback_chunk.removed_mask &= !(1u64 << chunk_idx);
                rollback_chunk.changed_mask &= !(1u64 << chunk_idx);
                rollback_chunk.created_mask |= 1u64 << chunk_idx;
            }

            // Update page level - creation/change is a change
            rollback_page.changed_mask |= 1u64 << (page_idx as u32);
        }

        // Update rollback storage at storage level - creation is a change
        self.rollback.changed_mask |= 1u64 << storage_idx;

        // Update counts and masks after potential rollback rotation
        {
            let page = unsafe { &mut *self.data[storage_idx] };
            page.count = page.count.saturating_add(1);
            self.count = self.count.saturating_add(1);

            // Update page masks (presence_mask already set if chunk was new)
            let chunk_was_new = (page.presence_mask >> page_idx) & 1 == 0;
            if !chunk_was_new {
                page.presence_mask |= 1u64 << page_idx;
            }

            // Update page fullness_mask based on chunk state
            if chunk_is_full {
                page.fullness_mask |= 1u64 << page_idx;
            } else {
                page.fullness_mask &= !(1u64 << page_idx);
            }
            page.fullness_mask &= page.presence_mask;
            page.changed_mask |= 1u64 << page_idx;

            debug_assert!(
                page.fullness_mask & !page.presence_mask == 0,
                "Page fullness_mask invariant violated after spawn()"
            );

            // Update storage masks (presence_mask already set if page was new)
            let page_was_new = (self.presence_mask >> storage_idx) & 1 == 0;
            if !page_was_new {
                self.presence_mask |= 1u64 << storage_idx;
            }

            // Use count to determine if page is full: page.count == 64*64 (all slots filled)
            let page_is_full = page.count == 64 * 64;

            if page_is_full {
                self.fullness_mask |= 1u64 << storage_idx;
            } else {
                self.fullness_mask &= !(1u64 << storage_idx);
            }
            self.fullness_mask &= self.presence_mask;
            self.changed_mask |= 1u64 << storage_idx;

            debug_assert!(
                self.fullness_mask & !self.presence_mask == 0,
                "Storage fullness_mask invariant violated after spawn()"
            );
        }

        // Verify all invariants
        debug_assert!(
            self.verify_invariants(),
            "Storage invariants violated after spawn()"
        );

        // Verify rollback invariants
        debug_assert!(
            self.rollback.verify_was_created(global_index)
                || self.rollback.verify_was_modified(global_index),
            "RollbackStorage invariant violated: index {} should be marked as created OR modified after spawn()",
            global_index
        );

        Some(entity)
    }
}

impl<T: Component> Drop for Storage<T> {
    fn drop(&mut self) {
        let mut storage_mask = self.presence_mask;
        while storage_mask != 0 {
            let s_start = storage_mask.trailing_zeros() as usize;
            let s_shifted = storage_mask >> s_start;
            let s_run = s_shifted.trailing_ones() as usize;
            for s in s_start..s_start + s_run {
                unsafe {
                    let page = &mut *self.data[s];
                    let mut chunk_mask = page.presence_mask;
                    while chunk_mask != 0 {
                        let c_start = chunk_mask.trailing_zeros() as usize;
                        let c_shifted = chunk_mask >> c_start;
                        let c_run = c_shifted.trailing_ones() as usize;
                        for p in c_start..c_start + c_run {
                            if let Some((new_ptr, moved_idx)) =
                                self.chunk_pool.free_chunk(page.data[p])
                            {
                                page.data[moved_idx as usize] = new_ptr;
                            }
                        }
                        chunk_mask &= !((u64::MAX >> (64 - c_run)) << c_start);
                    }
                }
                unsafe {
                    if let Some((new_ptr, moved_idx)) = self.page_pool.free_page(self.data[s]) {
                        self.data[moved_idx as usize] = new_ptr;
                    }
                }
            }
            storage_mask &= !((u64::MAX >> (64 - s_run)) << s_start);
        }
    }
}

/// A Page within a Storage, containing 64 Chunks.
///
/// # Mask Semantics
///
/// See Storage documentation for details on presence_mask and fullness_mask.
pub struct Page<T: Component> {
    pub presence_mask: u64,
    pub fullness_mask: u64,
    pub changed_mask: u64,
    pub count: u32,
    pub data: [*mut Chunk<T>; 64],
    pub chunk_pool: *mut Pool<Chunk<T>>,
    pub pool_slot: u8,
    pub pool_page: *mut PoolPage<Page<T>>,
    pub owner_index: u8,
}

impl<T: Component> Page<T> {
    /// Creates a new Page.
    pub fn new_with_pool(chunk_pool: *mut Pool<Chunk<T>>, default_chunk_ptr: *mut Chunk<T>) -> Self {
        Self {
            presence_mask: 0,
            fullness_mask: 0,
            changed_mask: 0,
            count: 0,
            data: [default_chunk_ptr; 64],
            chunk_pool,
            pool_slot: 0,
            pool_page: std::ptr::null_mut(),
            owner_index: 0,
        }
    }

    /// Verifies that all invariants hold for this Page and all its Chunks.
    /// Returns true if all invariants are satisfied, false otherwise.
    pub fn verify_invariants(&self) -> bool {
        if self.fullness_mask & !self.presence_mask != 0 {
            return false;
        }
        true
    }

    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn verify_invariants_ptr(page_ptr: *const Page<T>) -> bool {
        let presence_mask = unsafe { (*page_ptr).presence_mask };
        let fullness_mask = unsafe { (*page_ptr).fullness_mask };
        if fullness_mask & !presence_mask != 0 {
            return false;
        }
        true
    }

    /// Clears the changed_mask at Page and Chunk levels.
    /// This recursively clears changed_mask for all chunks that have changes.
    /// Uses changed_mask & presence_mask to efficiently iterate only over changed chunks.
    pub fn clear_changed_masks(&mut self) {
        // Use intersection of changed_mask and presence_mask to only visit changed chunks
        let mut page_mask = self.changed_mask & self.presence_mask;

        while page_mask != 0 {
            let page_start = page_mask.trailing_zeros() as usize;
            let shifted = page_mask >> page_start;
            let page_run_len = shifted.trailing_ones() as usize;

            for page_idx in page_start..page_start + page_run_len {
                unsafe {
                    let chunk = &mut *self.data[page_idx];
                    chunk.clear_changed_masks();
                }
            }

            page_mask &= !((u64::MAX >> (64 - page_run_len)) << page_start);
        }

        // Clear page level changed_mask after clearing all nested masks
        self.changed_mask = 0;
    }
}

// Default for Page<T> intentionally omitted; pages are constructed with a default chunk pointer

impl<T: Component> Drop for Page<T> {
    fn drop(&mut self) {
        // Page memory is freed via the page pool. Chunks are freed by Storage before page free.
    }
}

/// A Chunk within a Page, containing 64 values of type T.
///
/// # Mask Semantics (Leaf Node)
///
/// ## presence_mask
/// - Bit at index `i` is set (1) if the slot **currently** has a value
/// - When set, the slot contains an initialized value of type T
/// - **Only `presence_mask` is used to determine if a chunk currently has values**
///   - To check if a chunk has any present items: `presence_mask != 0`
///   - To check if a specific slot has a value: `(presence_mask >> i) & 1 != 0`
///
/// ## fullness_mask
/// - Bit at index `i` is set (1) if the slot has a value (same as presence_mask)
/// - **At Chunk level, fullness_mask == presence_mask** (if a component is set, the slot is full)
///   - `presence_mask=1, fullness_mask=1` indicates a slot with a value
///   - `presence_mask=0, fullness_mask=0` indicates an uninitialized/unused slot
/// - **Note**: A chunk is considered "full" when all 128 slots have `presence_mask=1` (and thus `fullness_mask=1`)
pub struct Chunk<T: Component> {
    pub presence_mask: u64,
    pub fullness_mask: u64,
    pub changed_mask: u64,
    pub data: [MaybeUninit<T>; 64],
    pub pool_slot: u8,
    pub pool_page: *mut PoolPage<Chunk<T>>,
    pub owner_index: u8,
}

impl<T: Component> Chunk<T> {
    /// Creates a new Chunk.
    pub fn new() -> Self {
        Self {
            presence_mask: 0,
            fullness_mask: 0,
            changed_mask: 0,
            data: unsafe { MaybeUninit::uninit().assume_init() },
            pool_slot: 0,
            pool_page: std::ptr::null_mut(),
            owner_index: 0,
        }
    }

    pub fn verify_invariants(&self) -> bool {
        // For Chunk (leaf): fullness_mask must equal presence_mask
        // If a component is present, the slot is full
        if self.fullness_mask != self.presence_mask {
            return false;
        }
        true
    }

    /// Clears the changed_mask at Chunk level.
    pub fn clear_changed_masks(&mut self) {
        self.changed_mask = 0;
    }
}

impl<T: Component> Default for Chunk<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Component> Drop for Chunk<T> {
    fn drop(&mut self) {
        // Drop all values that exist (presence_mask=1)
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
