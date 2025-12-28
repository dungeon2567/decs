use crate::tick::Tick;
use crate::arena::Arena;
use std::collections::VecDeque;
use std::mem::MaybeUninit;
// Debug-only allocation counters removed: single-threaded environment doesn't need atomics

// Using &'static Arena for allocator_api in Box::new_in. We create a stable
// arena reference by converting Box<Arena> into a raw pointer and then a
// 'static reference that remains valid until we reconstruct and drop the Box in Drop.

pub type VecQueue<T> = VecDeque<T>;

/// A hierarchical rollback storage structure for efficiently tracking changes from Storage<T>.
///
/// RollbackStorage is created from changes to Storage<T> by comparing the current state
/// (at current tick) to the previous state (at tick - 1). It stores the OLD values needed
/// for rollback operations.
///
/// The storage is organized in three levels:
/// - RollbackStorage contains 64 Pages
/// - Page contains 64 Chunks
/// - Chunk contains 64 values of type T
///
/// Total capacity: 64 * 64 * 64 = 262,144 items
///
/// # Mask Semantics
///
/// These masks track changes between tick-1 and the current tick, based on the original Storage<T>:
///
/// ## created_mask
/// - Bit at index `i` is set (1) if the item had NO VALUE at tick-1 and now has a value
/// - Used to track which items are new and need to be rolled back (removed)
/// - **Note**: Created items don't have values stored in RollbackStorage (there was no old value)
///
/// ## changed_mask
/// - Bit at index `i` is set (1) if the item HAD A VALUE at tick-1 and now has a different value
/// - Used to track which items were changed and need to be rolled back
/// - **Note**: Changed items store the OLD value (from tick-1) in RollbackStorage (to restore on rollback)
///
/// ## removed_mask
/// - Bit at index `i` is set (1) if the item EXISTED at tick-1 and now it was destroyed/removed
/// - Used to track which items were removed and need to be restored on rollback
/// - **Note**: Removed items store the OLD value (from tick-1) in RollbackStorage (to restore on rollback)
///
/// # Important
/// The invariants and mask semantics are based on the original Storage<T> at the current tick,
/// not on RollbackStorage itself. RollbackStorage is a diff/snapshot structure.
#[repr(align(64))]
pub struct RollbackStorage<T: Clone> {
    pub changed_mask: u64, // Set if any child has any change (creation, modification, or removal)
    pub tick: Tick,
    pub data: [MaybeUninit<Box<RollbackPage<T>, &'static Arena>>; 64],
    pub generation_at_tick_start: u64,
    pub arena_box: Box<Arena>,
}

impl<T: Clone> RollbackStorage<T> {
    #[inline]
    fn arena(&self) -> &'static Arena {
        unsafe { &*(self.arena_box.as_ref() as *const Arena) }
    }
    /// Creates a new empty RollbackStorage instance.
    pub fn new() -> Self {
        let arena_box = Box::new(Arena::new());
        Self {
            changed_mask: 0,
            tick: Tick(0),
            data: unsafe { MaybeUninit::uninit().assume_init() },
            generation_at_tick_start: 0,
            arena_box,
        }
    }

    pub fn reset_for_tick(&mut self, tick: Tick) {
        let mut mask = self.changed_mask;
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
        self.changed_mask = 0;
        self.arena_box.reset();
        self.generation_at_tick_start = 0;
        self.tick = tick;
    }

    /// Creates a new RollbackStorage instance with the given tick.
    pub fn with_tick(tick: Tick) -> Self {
        let arena_box = Box::new(Arena::new());
        Self {
            changed_mask: 0,
            tick,
            data: unsafe { MaybeUninit::uninit().assume_init() },
            generation_at_tick_start: 0,
            arena_box,
        }
    }
    /// Saves the current generation value for rollback (used for all storages).
    pub fn save_generation(&mut self, generation: u64) {
        self.generation_at_tick_start = generation;
    }

    /// Gets the saved generation value for rollback.
    pub fn get_saved_generation(&self) -> u64 {
        self.generation_at_tick_start
    }

    /// Clears the saved generation value (resets to 0).
    pub fn clear_saved_generation(&mut self) {
        self.generation_at_tick_start = 0;
    }

    /// Returns the current tick.
    pub fn tick(&self) -> Tick {
        self.tick
    }

    /// Sets the tick.
    pub fn set_tick(&mut self, tick: Tick) {
        self.tick = tick;
    }

    /// Verifies that the given index is marked as created.
    /// Returns true if the index is marked as created, false otherwise.
    /// Only checks at the chunk level, which is the source of truth for a specific item.
    pub fn verify_was_created(&self, index: u32) -> bool {
        let chunk_idx = index % 64;
        let page_idx = (index / 64) % 64;
        let storage_idx = index / (64 * 64);

        if storage_idx >= 64 {
            return false;
        }

        // Navigate to chunk and check chunk-level mask (source of truth for specific item)
        if let Some(page) = self.get_page(storage_idx)
            && let Some(chunk) = page.get(page_idx)
        {
            return (chunk.created_mask >> chunk_idx) & 1 != 0;
        }

        false
    }

    /// Verifies that the given index is marked as changed/modified.
    /// Returns true if the index is marked as changed, false otherwise.
    /// Only checks at the chunk level, which is the source of truth for a specific item.
    pub fn verify_was_modified(&self, index: u32) -> bool {
        let chunk_idx = index % 64;
        let page_idx = (index / 64) % 64;
        let storage_idx = index / (64 * 64);

        if storage_idx >= 64 {
            return false;
        }

        // Navigate to chunk and check chunk-level mask (source of truth for specific item)
        if let Some(page) = self.get_page(storage_idx)
            && let Some(chunk) = page.get(page_idx)
        {
            return (chunk.changed_mask >> chunk_idx) & 1 != 0;
        }

        false
    }

    /// Verifies that the given index is marked as removed.
    /// Returns true if the index is marked as removed, false otherwise.
    /// Only checks at the chunk level, which is the source of truth for a specific item.
    pub fn verify_was_removed(&self, index: u32) -> bool {
        let chunk_idx = index % 64;
        let page_idx = (index / 64) % 64;
        let storage_idx = index / (64 * 64);

        if storage_idx >= 64 {
            return false;
        }

        // Navigate to chunk and check chunk-level mask (source of truth for specific item)
        if let Some(page) = self.get_page(storage_idx)
            && let Some(chunk) = page.get(page_idx)
        {
            return (chunk.removed_mask >> chunk_idx) & 1 != 0;
        }

        false
    }

    /// Verifies that the given index is NOT tracked in rollback (no change occurred).
    /// Returns true if the index is not marked as created, modified, or removed, false otherwise.
    /// This is used to verify idempotent operations (e.g., Add->Remove = no change).
    /// Only checks at the chunk level, which is the source of truth for a specific item.
    ///
    /// Note: If both created_mask and removed_mask are set, this indicates Add->Remove idempotent
    /// operation, which should be treated as "no change" (returns true).
    pub fn verify_not_changed(&self, index: u32) -> bool {
        let chunk_idx = index % 64;
        let page_idx = (index / 64) % 64;
        let storage_idx = index / (64 * 64);

        if storage_idx >= 64 {
            return true; // Index out of range, so not tracked
        }

        // Navigate to chunk and check that none of the masks are set
        if let Some(page) = self.get_page(storage_idx)
            && let Some(chunk) = page.get(page_idx)
        {
            let has_created = (chunk.created_mask >> chunk_idx) & 1 != 0;
            let has_changed = (chunk.changed_mask >> chunk_idx) & 1 != 0;
            let has_removed = (chunk.removed_mask >> chunk_idx) & 1 != 0;

            if has_created && has_removed && !has_changed {
                return true;
            }

            return !has_created && !has_changed && !has_removed;
        }

        // If chunk doesn't exist, it's not tracked
        true
    }

    pub fn get_page(&self, index: u32) -> Option<&RollbackPage<T>> {
        if index >= 64 {
            return None;
        }
        if (self.changed_mask >> index) & 1 == 0 {
            return None;
        }
        unsafe { Some(self.data[index as usize].assume_init_ref()) }
    }

    pub fn get_page_mut(&mut self, index: u32) -> Option<&mut RollbackPage<T>> {
        if index >= 64 {
            return None;
        }
        if (self.changed_mask >> index) & 1 == 0 {
            return None;
        }
        // Don't set changed_mask here - it should only be set when values actually change
        unsafe { Some(self.data[index as usize].assume_init_mut()) }
    }

    pub fn get_or_create_page(&mut self, index: u32) -> &mut RollbackPage<T> {
        assert!(index < 64, "Page index must be in range 0-63");

        if (self.changed_mask >> index) & 1 == 0 {
            let alloc = self.arena();
            let page = Box::new_in(RollbackPage::new_with_alloc(alloc), alloc);
            self.data[index as usize].write(page);
            self.changed_mask |= 1u64 << index;
        }

        unsafe { self.data[index as usize].assume_init_mut() }
    }

    /// Gets a reference to a value at the given global index.
    /// Returns None if the value doesn't exist.
    pub fn get(&self, index: u32) -> Option<&T> {
        let chunk_idx = index % 64;
        let page_idx = (index / 64) % 64;
        let storage_idx = index / (64 * 64);

        if storage_idx >= 64 {
            return None;
        }

        let page = self.get_page(storage_idx)?;
        let chunk = page.get(page_idx)?;

        if chunk_idx >= 64 {
            return None;
        }
        let has_changed = (chunk.changed_mask >> chunk_idx) & 1 != 0;
        let has_removed = (chunk.removed_mask >> chunk_idx) & 1 != 0;
        if !has_changed && !has_removed {
            return None;
        }
        if has_removed {
            return None;
        }
        unsafe { Some(chunk.data[chunk_idx as usize].assume_init_ref()) }
    }

    /// Gets a mutable reference to a value at the given global index.
    /// Returns None if the value doesn't exist.
    pub fn get_mut(&mut self, index: u32) -> Option<&mut T> {
        let chunk_idx = index % 64;
        let page_idx = (index / 64) % 64;
        let storage_idx = index / (64 * 64);

        if storage_idx >= 64 {
            return None;
        }

        let page = self.get_page_mut(storage_idx)?;
        let chunk = page.get_mut(page_idx)?;

        if chunk_idx >= 64 {
            return None;
        }
        let has_changed = (chunk.changed_mask >> chunk_idx) & 1 != 0;
        let has_removed = (chunk.removed_mask >> chunk_idx) & 1 != 0;
        if !has_changed {
            return None;
        }
        if has_removed {
            return None;
        }
        unsafe { Some(chunk.data[chunk_idx as usize].assume_init_mut()) }
    }

    /// Sets a value at the given global index.
    pub fn set(&mut self, index: u32, value: T) {
        let chunk_idx = index % 64;
        let page_idx = (index / 64) % 64;
        let storage_idx = index / (64 * 64);

        assert!(storage_idx < 64, "Storage index out of range");

        let was_removed;
        let was_created;
        {
            let page = self.get_or_create_page(storage_idx);
            let chunk = page.get_or_create_chunk(page_idx);

            was_removed = (chunk.removed_mask >> chunk_idx) & 1 != 0;
            was_created = (chunk.created_mask >> chunk_idx) & 1 != 0;
            let was_present =
                was_created || (chunk.changed_mask >> chunk_idx) & 1 != 0 || was_removed;

            unsafe {
                if was_present && !was_removed {
                    chunk.data[chunk_idx as usize].assume_init_drop();
                }
                chunk.data[chunk_idx as usize].write(value);
            }

            if was_removed {
                chunk.removed_mask &= !(1u64 << chunk_idx);
                chunk.created_mask &= !(1u64 << chunk_idx);
                chunk.changed_mask |= 1u64 << chunk_idx;
            } else if was_created {
                chunk.changed_mask &= !(1u64 << chunk_idx);
            } else {
                chunk.created_mask |= 1u64 << chunk_idx;
                chunk.removed_mask &= !(1u64 << chunk_idx);
                chunk.changed_mask &= !(1u64 << chunk_idx);
            }
        }

        // Update page and storage level masks after releasing chunk borrow
        // Any change (creation, modification, or removal) sets changed_mask
        {
            let page = self.get_page_mut(storage_idx).unwrap();
            page.changed_mask |= 1u64 << page_idx;
        }

        // Update storage level masks - any change sets changed_mask
        self.changed_mask |= 1u64 << storage_idx;
    }

    /// Clears the changed_mask at all levels (RollbackStorage, Page, and Chunk).
    /// This recursively clears changed_mask for all pages and chunks that have changes.
    /// Note: changed_mask is kept set if pages still have chunks with created/removed masks
    /// (needed to track structure existence).
    pub fn clear_changed_masks(&mut self) {
        // Use changed_mask to visit all pages
        let mut storage_mask = self.changed_mask;
        let mut new_changed_mask = 0u64;

        while storage_mask != 0 {
            let storage_start = storage_mask.trailing_zeros() as usize;
            let shifted = storage_mask >> storage_start;
            let storage_run_len = shifted.trailing_ones() as usize;

            for storage_idx in storage_start..storage_start + storage_run_len {
                unsafe {
                    let page = self.data[storage_idx].assume_init_mut();
                    page.clear_changed_masks();
                    // If page still has changed_mask set (chunks with created/removed masks), keep it
                    if page.changed_mask != 0 {
                        new_changed_mask |= 1u64 << storage_idx;
                    }
                }
            }

            storage_mask &= !((1u64 << storage_run_len) - 1) << storage_start;
        }

        // Update storage level changed_mask - keep it set if pages have chunks with created/removed masks
        self.changed_mask = new_changed_mask;
    }

    /// Verifies that all invariants hold for this RollbackStorage and all its Pages.
    /// Returns true if all invariants are satisfied, false otherwise.
    pub fn verify_invariants(&self) -> bool {
        let mut mask = self.changed_mask;
        while mask != 0 {
            let start = mask.trailing_zeros() as usize;
            let shifted = mask >> start;
            let run_len = shifted.trailing_ones() as usize;
            for i in start..start + run_len {
                unsafe {
                    let page = &**self.data[i].assume_init_ref();
                    if !page.verify_invariants() {
                        return false;
                    }
                }
            }
            mask &= !((u64::MAX >> (64 - run_len)) << start);
        }
        true
    }
}

impl<T: Clone> Default for RollbackStorage<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone> Drop for RollbackStorage<T> {
    fn drop(&mut self) {
        // Drop all pages that exist (changed_mask set means the page exists)
        // At this level, we're dropping Box<RollbackPage<T>> structures, not values
        let mut mask = self.changed_mask;
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
        // arena_box drops automatically here
    }
}

/// A Page within a RollbackStorage, containing 64 Chunks.
///
/// # Mask Semantics
///
/// changed_mask: Set if any child chunk has any change (creation, modification, or removal)
#[repr(align(64))]
pub struct RollbackPage<T> {
    pub changed_mask: u64, // Set if any child has any change (creation, modification, or removal)
    pub data: [MaybeUninit<Box<RollbackChunk<T>, &'static Arena>>; 64],
    alloc: &'static Arena,
}

impl<T> RollbackPage<T> {
    pub fn new_with_alloc(alloc: &'static Arena) -> Self {
        Self {
            changed_mask: 0,
            data: unsafe { MaybeUninit::uninit().assume_init() },
            alloc,
        }
    }

    /// Gets a reference to the Chunk at the given index (0-127).
    /// Returns None if the chunk doesn't exist.
    pub fn get(&self, index: u32) -> Option<&RollbackChunk<T>> {
        if index >= 64 {
            return None;
        }
        if (self.changed_mask >> index) & 1 == 0 {
            return None;
        }
        unsafe { Some(&**self.data[index as usize].assume_init_ref()) }
    }

    /// Gets a mutable reference to the Chunk at the given index (0-127).
    /// Returns None if the chunk doesn't exist.
    pub fn get_mut(&mut self, index: u32) -> Option<&mut RollbackChunk<T>> {
        if index >= 64 {
            return None;
        }
        if (self.changed_mask >> index) & 1 == 0 {
            return None;
        }
        // Don't set changed_mask here - it should only be set when values actually change
        unsafe { Some(self.data[index as usize].assume_init_mut()) }
    }

    /// Gets or creates a Chunk at the given index (0-127).
    pub fn get_or_create_chunk(&mut self, index: u32) -> &mut RollbackChunk<T> {
        assert!(index < 64, "Chunk index must be in range 0-63");

        if (self.changed_mask >> index) & 1 == 0 {
            let chunk = Box::new_in(RollbackChunk::new(), self.alloc);
            self.data[index as usize].write(chunk);
            self.changed_mask |= 1u64 << index;
        }

        unsafe { self.data[index as usize].assume_init_mut() }
    }

    /// Verifies that all invariants hold for this RollbackPage and all its Chunks.
    /// Returns true if all invariants are satisfied, false otherwise.
    pub fn verify_invariants(&self) -> bool {
        let mut mask = self.changed_mask;
        while mask != 0 {
            let start = mask.trailing_zeros() as usize;
            let shifted = mask >> start;
            let run_len = shifted.trailing_ones() as usize;
            for i in start..start + run_len {
                unsafe {
                    let chunk = &**self.data[i].assume_init_ref();
                    if !chunk.verify_invariants() {
                        return false;
                    }
                }
            }
            mask &= !((u64::MAX >> (64 - run_len)) << start);
        }
        true
    }

    /// Clears the changed_mask at Page and Chunk levels.
    /// This recursively clears changed_mask for all chunks that have changes.
    /// Note: changed_mask is kept set if chunks still have created/removed masks
    /// (needed to track structure existence).
    pub fn clear_changed_masks(&mut self) {
        // Use changed_mask to visit all chunks
        let mut page_mask = self.changed_mask;
        let mut new_changed_mask = 0u64;

        while page_mask != 0 {
            let page_start = page_mask.trailing_zeros() as usize;
            let shifted = page_mask >> page_start;
            let page_run_len = shifted.trailing_ones() as usize;

            for page_idx in page_start..page_start + page_run_len {
                unsafe {
                    let chunk = self.data[page_idx].assume_init_mut();
                    chunk.clear_changed_masks();
                    // If chunk still has created/removed masks, keep changed_mask set
                    if (chunk.created_mask | chunk.removed_mask) != 0 {
                        new_changed_mask |= 1u64 << page_idx;
                    }
                }
            }

            page_mask &= !((u64::MAX >> (64 - page_run_len)) << page_start);
        }

        // Update page level changed_mask - keep it set if chunks have created/removed masks
        self.changed_mask = new_changed_mask;
    }
}

impl<T> Drop for RollbackPage<T> {
    fn drop(&mut self) {
        // Drop all chunks that exist (changed_mask set means the chunk exists)
        // At this level, we're dropping Box<RollbackChunk<T>> structures, not values
        let mut mask = self.changed_mask;
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

/// A Chunk within a RollbackPage, containing 64 values of type T.
///
/// # Mask Semantics
///
/// See RollbackStorage documentation for details on created_mask, changed_mask, and removed_mask.
#[repr(align(64))]
pub struct RollbackChunk<T> {
    pub created_mask: u64,
    pub changed_mask: u64,
    pub removed_mask: u64,
    pub data: [MaybeUninit<T>; 64],
}

impl<T> RollbackChunk<T> {
    /// Creates a new RollbackChunk
    pub fn new() -> Self {
        Self {
            created_mask: 0,
            changed_mask: 0,
            removed_mask: 0,
            data: unsafe { MaybeUninit::uninit().assume_init() },
        }
    }

    pub fn verify_invariants(&self) -> bool {
        // No specific invariants to check for rollback chunks
        true
    }

    /// Clears the changed_mask at Chunk level.
    pub fn clear_changed_masks(&mut self) {
        self.changed_mask = 0;
    }
}

impl<T> Default for RollbackChunk<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Drop for RollbackChunk<T> {
    fn drop(&mut self) {
        // Only drop items that have stored values (changed or removed), not created items
        // Created items don't have values in rollback storage, so nothing to drop
        let mut mask = self.changed_mask | self.removed_mask;

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

// debug_alloc_counts removed
