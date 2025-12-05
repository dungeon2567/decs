use crate::component::Component;
use crate::storage::{Chunk, Storage};
use crate::tick::Tick;
use std::ops::{Deref, DerefMut};

/// Immutable view wrapper for component data.
///
/// # Usage Restrictions
///
/// **IMPORTANT**: `View` should ONLY be used within Systems. Do not use this for direct storage manipulation.
///
/// Systems are responsible for maintaining all storage invariants when using View/ViewMut.
pub struct View<'a, T: Component> {
    pub data: &'a T,
}

/// Mutable view wrapper for component data.
///
/// # Usage Restrictions
///
/// **IMPORTANT**: `ViewMut` should ONLY be used within Systems. Do not use this for direct storage manipulation.
///
/// # System Responsibilities
///
/// `ViewMut` operates at the **Chunk level only** for performance. When `DerefMut` is called, it sets
/// the `changed_mask` bit **only at the Chunk level**. It does NOT propagate changes to Page or Storage levels.
///
/// **The System (or system framework) is responsible for:**
/// 1. Propagating `changed_mask` from Chunk → Page → Storage after processing
/// 2. Maintaining `fullness_mask` and `presence_mask` consistency if needed
/// 3. Ensuring all storage invariants remain valid
///
/// Failure to propagate masks will cause issues with:
/// - `Storage::clear_changed_masks()` won't find changed chunks
/// - Rollback operations may miss changes
/// - Change tracking queries won't work correctly
///
/// # Example System Pattern
///
/// ```ignore
/// // After processing chunks with ViewMut:
/// if chunk.changed_mask != 0 {
///     page.changed_mask |= 1u128 << page_idx;
///     storage.changed_mask |= 1u128 << storage_idx;
/// }
/// ```
pub struct ViewMut<'a, T: Component + Clone> {
    pub chunk: &'a mut Chunk<T>,
    pub index: u32,
    pub storage: *mut Storage<T>,
    pub storage_idx: u32,
    pub page_idx: u32,
    pub current_tick: Tick,
}

impl<'a, T: Component> View<'a, T> {
    pub fn new(data: &'a T) -> Self {
        Self { data }
    }
}

impl<'a, T: Component> std::ops::Deref for View<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<'a, T: Component + Clone> ViewMut<'a, T> {
    pub fn new(
        chunk: &'a mut Chunk<T>,
        index: u32,
        storage: *mut Storage<T>,
        storage_idx: u32,
        page_idx: u32,
        current_tick: Tick,
    ) -> Self {
        Self {
            chunk,
            index,
            storage,
            storage_idx,
            page_idx,
            current_tick,
        }
    }
}

impl<'a, T: Component + Clone> Deref for ViewMut<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.chunk.data[self.index as usize].assume_init_ref() }
    }
}

impl<'a, T: Component + Clone> DerefMut for ViewMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            let bit = 1u64 << self.index;
            // Check if this is the first change in this chunk for the current processing
            let chunk_not_changed_before = self.chunk.changed_mask == 0;
            // Mark item-level change in chunk
            self.chunk.changed_mask |= bit;

            // Ensure rollback tick matches current tick
            let storage_mut = &mut *self.storage;
            storage_mut.ensure_rollback_tick(self.current_tick);

            // Access rollback page/chunk
            let rb_page = storage_mut.rollback.get_or_create_page(self.storage_idx);
            let rb_chunk = rb_page.get_or_create_chunk(self.page_idx);

            let was_created = (rb_chunk.created_mask & bit) != 0;
            let was_changed = (rb_chunk.changed_mask & bit) != 0;
            let was_removed = (rb_chunk.removed_mask & bit) != 0;

            if was_created {
                // Created + modified in same tick remains created only; no old value stored
                rb_chunk.removed_mask &= !bit;
                rb_chunk.changed_mask &= !bit;
                rb_chunk.created_mask |= bit;
            } else {
                // Store old value only once per tick if not already tracked
                if !was_changed && !was_removed {
                    let old_val = self.chunk.data[self.index as usize]
                        .assume_init_ref()
                        .clone();
                    rb_chunk.data[self.index as usize].write(old_val);
                }
                rb_chunk.removed_mask &= !bit;
                rb_chunk.created_mask &= !bit;
                rb_chunk.changed_mask |= bit;
            }

            // Set hierarchical changed masks in rollback only on first change in this chunk
            if chunk_not_changed_before {
                rb_page.changed_mask |= 1u64 << self.page_idx;
                storage_mut.rollback.changed_mask |= 1u64 << self.storage_idx;
            }

            // Return mutable reference to the value
            self.chunk.data[self.index as usize].assume_init_mut()
        }
    }
}
