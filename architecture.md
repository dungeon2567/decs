# Architecture Documentation: Storage and Rollback System

## Overview

This document describes the architecture, invariants, and semantics of the hierarchical storage and rollback system. The system consists of two main components:

1. **Storage<T>**: A hierarchical data structure for storing component values
2. **RollbackStorage<T>**: A hierarchical structure for tracking changes to enable rollback operations

Both structures use a three-level hierarchy:
- **Storage/RollbackStorage** (64 entries)
  - **Page/RollbackPage** (64 entries)
    - **Chunk/RollbackChunk** (64 values of type T)

**Total capacity**: 64 × 64 × 64 = 262,144 items

---

## Storage<T> Architecture

### Structure

```
Storage<T>
├── presence_mask: u64    // Tracks which pages exist
├── fullness_mask: u64    // Tracks which pages are full
├── changed_mask: u64     // Tracks which pages have changes
├── count: u32            // Total number of values stored
├── rollback: Box<RollbackStorage<T>>
├── prev: VecQueue<Box<RollbackStorage<T>>>  // Rollback history (max 64 ticks)
├── rollback_pool: Vec<Box<RollbackStorage<T>>>  // Pool of recycled rollback instances
├── generation: u64       // Global generation counter (Entity only)
├── default_chunk_ptr: *const Chunk<T>  // Shared default chunk pointer
├── default_page_ptr: *const Page<T>    // Shared default page pointer
└── data: [*mut Page<T>; 64]
    └── Page<T>
        ├── presence_mask: u64
        ├── fullness_mask: u64
        ├── changed_mask: u64
        ├── count: u32
        └── data: [*mut Chunk<T>; 64]
            └── Chunk<T>
                ├── presence_mask: u64
                ├── fullness_mask: u64
                ├── changed_mask: u64
                └── data: [MaybeUninit<T>; 64]
```

### Mask Semantics

#### presence_mask

**For non-leaf nodes (Storage, Page):**
- Bit at index `i` is set (1) if any child has at least 1 element
- Used to accelerate queries by quickly identifying which indices have data
- Example: `presence_mask & query_mask` quickly finds matching indices

**For leaf nodes (Chunk):**
- Bit at index `i` is set (1) if the slot **currently** has a value
- When set, the slot contains an initialized value of type T
- **Only `presence_mask` is used to determine if a chunk currently has values**

#### fullness_mask

**For non-leaf nodes (Storage, Page):**
- Bit at index `i` is set (1) if all children are FULL
- Used to track when all children have reached their capacity
- **Invariant**: `fullness_mask & !presence_mask == 0` (fullness can only be set where presence is set)

**For leaf nodes (Chunk):**
- Bit at index `i` is set (1) if the slot has a value (same as presence_mask)
- **At Chunk level, fullness_mask == presence_mask** (if a component is set, the slot is full)
  - `presence_mask=1, fullness_mask=1` → slot with a value
  - `presence_mask=0, fullness_mask=0` → uninitialized/unused slot
- **Note**: A chunk is considered "full" when all 64 slots have `presence_mask=1` (and thus `fullness_mask=1`)

#### changed_mask

- Bit at index `i` is set (1) if the item at that index has been modified
- Used to track which items need processing or synchronization
- Should be cleared after processing changes

### Storage Invariants

1. **Fullness Mask Invariant** (non-leaf nodes):
   ```
   fullness_mask & !presence_mask == 0
   ```
   Fullness can only be set where presence is also set.

2. **Count Invariant**:
   ```
   Storage.count == sum of all Page.count values
   Page.count == sum of all Chunk values (presence_mask only)
   ```
   The count at each level must match the sum of child counts.
   Note: `presence_mask` alone is sufficient for counting.

3. **Fullness Count Invariant**:
   ```
If Storage.count == 64*64*64, then fullness_mask == presence_mask
If Page.count == 64*64, then Page.fullness_mask bit is set
   ```
   When a level is completely full, all presence bits should also be in fullness_mask.

4. **Presence Mask Consistency**:
   ```
   If presence_mask[i] == 1, then data[i] is initialized
   If presence_mask[i] == 0, then data[i] may be uninitialized
   ```

5. **Fullness Invariant** (Chunk level only):
   ```
   fullness_mask == presence_mask
   ```
   At Chunk level, if a component is present, the slot is full. The masks are always equal.

---

## RollbackStorage<T> Architecture

### Structure

#[repr(align(64))]
pub struct RollbackStorage<T: Clone> {
    pub changed_mask: u64, // Set if any child has any change (creation, modification, or removal)
    pub tick: Tick,
    pub data: [MaybeUninit<Box<RollbackPage<T>, &'static Bump>>; 64],
    pub generation_at_tick_start: u64, // Saved generation for rollback
    pub arena_box: Box<Bump>,          // Bump allocator for pages/chunks
}

### Mask Semantics

RollbackStorage tracks changes between **tick-1** and the **current tick**, based on the original Storage<T>.

#### changed_mask (Storage/Page levels)

**For non-leaf nodes (RollbackStorage, RollbackPage):**
- Bit at index `i` is set (1) if **ANY change occurred** in any child (creation, modification, or removal)
- Used to efficiently track which pages/chunks have changes without needing to distinguish the type of change
- **Simplification**: At storage and page levels, we only track that *something* changed, not *what* changed
- The specific change type (created, changed, removed) is tracked at the chunk level only

#### created_mask (Chunk level only)

- Bit at index `i` is set (1) if the item had **NO VALUE at tick-1** and now has a value
- Used to track which items are new and need to be rolled back (removed)
- **Critical**: Created items **don't have values stored** in RollbackStorage (there was no old value)
- On rollback: Remove the item (it didn't exist at tick-1)
- **Only exists at RollbackChunk level**

#### changed_mask (Chunk level)

- Bit at index `i` is set (1) if the item **HAD A VALUE at tick-1** and now has a different value
- Used to track which items were changed and need to be rolled back
- **Critical**: Changed items **store the OLD value** (from tick-1) in RollbackStorage
- On rollback: Restore the old value
- **Exists at all levels, but semantics differ**: At chunk level, it means "modified"; at page/storage levels, it means "any change"

#### removed_mask (Chunk level only)

- Bit at index `i` is set (1) if the item **EXISTED at tick-1** and now it was destroyed/removed
- Used to track which items were removed and need to be restored on rollback
- **Critical**: Removed items **store the OLD value** (from tick-1) in RollbackStorage
- On rollback: Restore the old value
- **Only exists at RollbackChunk level**

### Rollback Invariants

1. **Value Storage Invariant**:
   ```
   If created_mask[i] == 1, then data[i] is NOT initialized (no value to store)
   If changed_mask[i] == 1, then data[i] IS initialized (stores old value)
   If removed_mask[i] == 1, then data[i] IS initialized (stores old value)
   ```

2. **Mask Mutually Exclusive Invariant**:
   ```
   For any index i, at most one of {created_mask, changed_mask, removed_mask} is set
   ```
   An item cannot be simultaneously created, changed, and removed in the same tick.

3. **Hierarchical Consistency**:
   ```
   If RollbackStorage.changed_mask[i] == 1, then RollbackPage.changed_mask[j] == 1 for some j
   If RollbackPage.changed_mask[j] == 1, then RollbackChunk has at least one mask set for some k
   ```
   At storage/page levels, `changed_mask` indicates any change in any child. The specific change type (created/changed/removed) is only tracked at the chunk level.

4. **Drop Invariant** (Storage/Page levels):
  ```
  Drop all pages/chunks where changed_mask is set
  ```
  At Storage/Page levels, we drop Box structures, so `changed_mask` set means the structure exists (some change occurred in a child).

6. **Drop Invariant** (Chunk level):
   ```
   Drop only values where changed_mask OR removed_mask is set
   Do NOT drop values where only created_mask is set (no value stored)
   ```
   At Chunk level, we drop actual values, so only drop if a value was stored.

6. **Idempotence Invariants** (operations within the same tick):
   
   These invariants ensure that operations that cancel each other out within the same tick are treated correctly:
   
   a. **Add → Remove = No Change**:
   ```
   If an item is created and then removed in the same tick:
   - All masks are cleared (created_mask, changed_mask, removed_mask)
   - No value is stored in RollbackStorage
   - changed_mask at page/storage levels is cleared if no other changes exist
   - Result: As if the operations never happened
   ```
   
   b. **Add → Change = Add** (with final value):
   ```
   If an item is created and then modified in the same tick:
   - created_mask remains set, changed_mask and removed_mask are cleared
   - No value is stored in RollbackStorage (created items have no old value)
   - The final value in Storage is the modified value
   - Result: Treated as a creation with the final value
   ```
   
   c. **Remove → Add = Change** (only if remove was successful):
   ```
   If an item that EXISTED BEFORE is removed and then added back in the same tick:
   - changed_mask is set, created_mask and removed_mask are cleared
   - The OLD value (from before removal) is stored in RollbackStorage
   - Result: Treated as a modification, not a removal+creation
   
   If an item that DID NOT EXIST is "removed" (remove fails) and then added:
   - This is just a creation (Add operation)
   - created_mask is set, changed_mask and removed_mask are cleared
   - No value is stored in RollbackStorage (created items have no old value)
   - Result: Treated as a creation, not a modification
   ```
   
   d. **Remove without existence = No Change**:
   ```
   If remove() is called on an item that doesn't exist:
   - remove() returns false
   - No rollback tracking occurs (no masks set, no values stored)
   - Result: As if the operation never happened
   ```
   
   These invariants ensure that the rollback system correctly tracks the net effect of operations within a single tick, maintaining consistency and avoiding unnecessary rollback data.

---

## Storage ↔ RollbackStorage Interaction

### When Setting a Value in Storage

1. **If value existed before (was_present = true)**:
   - Read old value from Storage
   - Store old value in RollbackStorage (drop any existing value first)
   - At chunk level: Set `changed_mask`, clear `created_mask` and `removed_mask`
   - At page/storage levels: Set `changed_mask` (indicates any change occurred)

2. **If value didn't exist before (was_present = false)**:
   - No old value to read
   - At chunk level: Set `created_mask`, clear `changed_mask` and `removed_mask`
   - At page/storage levels: Set `changed_mask` (indicates any change occurred)
   - **Do NOT store a value in RollbackStorage** (created items have no old value)

### When Removing a Value from Storage

1. Read old value from Storage before dropping
2. Store old value in RollbackStorage (drop any existing value first)
3. At chunk level: Set `removed_mask`, clear `created_mask` and `changed_mask`
4. At page/storage levels: Set `changed_mask` (indicates any change occurred)

### Critical Rules

1. **Never drop `created_mask` items**: Created items don't have stored values, so checking `created_mask` before dropping is a bug
2. **Always drop before storing**: If `changed_mask` or `removed_mask` is set, drop the existing value before storing a new one
3. **Mask propagation**: Changes at chunk level must propagate `changed_mask` to page and storage levels (any change sets `changed_mask` at parent levels)
4. **Idempotent operations**: 
   - Remove → Add = Change (ONLY if remove was successful - item existed before)
   - Remove → Add = Add (if remove failed - item didn't exist, treat as creation)
   - Add → Remove = No Change (clear all masks, no rollback data)
   - Add → Change = Add (remains as creation with final value)
   - Remove without existence = No Change (no rollback tracking)
5. **Simplified hierarchy**: At storage/page levels, only `changed_mask` is used - it indicates "something changed" without specifying the type
6. **Idempotence handling**: When operations cancel out (Add→Remove), the rollback tree is cleaned up - changed_mask is cleared at all levels if no other changes exist

---

## Hierarchy System

The hierarchy system provides parent-child relationship management through `Parent` and `ChildOf` components, maintained by `UpdateHierarchySystem`.

### Components

1.  **Parent**:
    - `first_child: Entity` - Head of the child list
    - `last_child: Entity` - Tail of the child list

2.  **ChildOf**:
    - `parent: Option<Entity>` - Current parent
    - `next_sibling: Option<Entity>` - Next sibling in list
    - `prev_sibling: Option<Entity>` - Previous sibling in list
    - `pending_parent: Option<Entity>` - Used to request reparenting

### UpdateHierarchySystem

- **Responsibility**: Processes `ChildOf` components with a set `pending_parent`.
- **Logic**:
    1.  Detaches child from old parent (updating old parent's `first_child`/`last_child` and siblings' `next`/`prev` links).
    2.  Attaches child to new parent (appending to tail, updating new parent's `last_child`).
    3.  Updates `parent` field and clears `pending_parent`.
- **Ordering**: Sibling order is maintained as a doubly-linked list. New children are appended to the end.

---

## System Integration & View Semantics

### View and ViewMut Usage

1. **Restricted Usage**: `View` and `ViewMut` wrappers are designed to be used **ONLY** within Systems. They should not be used directly for manual storage manipulation.
2. **Scope**: These wrappers operate at the **Chunk level** (leaf nodes) for performance reasons.
   - `ViewMut` updates `changed_mask` at the Chunk level.
   - `ViewMut` handles **RollbackStorage** updates: it saves old values and updates the RollbackStorage hierarchy masks.
   - `ViewMut` does **NOT** update the main Storage/Page level `changed_mask`. This is the responsibility of the System (or system execution macro).
   - `ViewMut` does not modify `presence_mask` or `fullness_mask`.

### System Responsibilities

Because `ViewMut` does not propagate `changed_mask` up the main Storage hierarchy, the **System** (or the code driving the system iteration) bears the responsibility for maintaining invariants:

1. **Mask Propagation**: After a System finishes processing a chunk (or during processing), it must ensure that `changed_mask` is correctly propagated to Page and Storage levels of the main Storage if any changes occurred. The `system!` macro handles this automatically at the chunk level.
2. **Invariant Maintenance**: The System must ensure that `fullness_mask` and `presence_mask` remain consistent if it performs operations that could affect them (though `ViewMut` typically only modifies data, not presence).
3. **Batch Updates**: Systems should ideally process updates in batches and propagate masks once per Page or after the entire run, rather than per component, to minimize overhead.

### Preconditions for ViewMut

1. `ViewMut` is called only for entities that already exist in `Storage`.
   - The target slot’s Chunk `presence_mask` bit is set (1).
   - The item has not been removed in the current tick.
2. Therefore, `ViewMut` does not modify `presence_mask` or `fullness_mask`.
   - It marks changes via `changed_mask` and captures old values for rollback when needed.
   - Creation or removal (which change `presence_mask`) are handled by other APIs like `set()`, `remove()`, or `spawn()`.

---

### Cleanup Systems

- `ComponentCleanupSystem` runs after all systems.
- For component type `T` (non-temporary):
  - Removes all `T` instances that also have a `Destroyed` component.
  - Clears `changed_mask` for `T` at Chunk/Page/Storage after processing.
  - Does not need to maintain `changed_mask` invariant during its run; it does not set `changed_mask` and instead cleans it at the end via `clear_changed_masks()`.
  - Maintains all invariants required for `T` in `RollbackStorage` (mask propagation and idempotence semantics).
- It does not run for temporary components (e.g., `Destroyed`).
- For `Destroyed`, `TemporaryComponentCleanupSystem` runs and fully cleans its storage (drops components, chunks, and pages), leaving masks and counts reset.

### World Run Postconditions

- After `world.run()`, cleanup systems execute: `ComponentCleanupSystem<T>` for non-temporary components and `TemporaryComponentCleanupSystem` for temporary ones.
- All `changed_mask` values are cleared at Chunk, Page, and Storage levels for processed components.
- Temporary components (such as `Destroyed`) are fully removed; their storages are cleaned, and masks and counts are reset.
- Storage invariants are maintained; `verify_invariants()` should pass following cleanup.

---

## Memory Safety

### MaybeUninit Usage

The system uses `MaybeUninit` to avoid unnecessary initialization:

- **Storage/Page**: Use raw pointers (`*mut Page<T>`, `*mut Chunk<T>`) which are nullable/default-initialized.
- **Chunk**: Uses `[MaybeUninit<T>; 64]` for component storage.
- **RollbackStorage/RollbackPage**: Use `[MaybeUninit<Box<...>>; 64]` to avoid allocating empty branches.

This requires careful handling:

1. **Before accessing `data[i]`**:
   - Check that the corresponding mask bit is set.
   - For `Storage`: Ensure pointer is not the default shared pointer.
   - For `RollbackStorage`: Use `assume_init_ref()` or `assume_init_mut()` only after verification.

2. **Before writing to `data[i]`**:
   - Use `write()` for uninitialized slots (Rollback/Chunk).
   - Use `assume_init_drop()` then `write()` for initialized slots (Rollback/Chunk).

3. **Before dropping**:
   - Only drop slots where masks indicate initialized data.
   - For RollbackStorage: Only drop `changed_mask | removed_mask`, never `created_mask`.

### Drop Order

1. **Storage/Page levels**: Drop all children where `presence_mask` is set
2. **RollbackStorage/RollbackPage levels**: Drop all children where `changed_mask` is set
3. **RollbackChunk level**: Drop only values where `changed_mask | removed_mask` is set

---

## Index Calculation

Global index `i` is decomposed into three levels:

```rust
chunk_idx = i % 64
page_idx = (i / 64) % 64
storage_idx = i / (64 * 64)
```

**Range checks**:
- `storage_idx` must be < 64
- `page_idx` must be < 64
- `chunk_idx` must be < 64

---

## Verification Functions

### Storage::verify_invariants()

Checks:
1. `fullness_mask & !presence_mask == 0` (fullness invariant)
2. `count == sum of all child counts` (count invariant)
3. If full, `fullness_mask == presence_mask`
4. Recursively verifies all pages and chunks

### RollbackStorage::verify_invariants()

Checks:
1. Recursively verifies all pages and chunks

### RollbackStorage::verify_was_created/modified/removed()

Verifies that an index is marked at the **chunk level** (source of truth for specific items):
- Returns `true` only if the corresponding mask is set at the chunk level
- Navigates through storage and page levels using `changed_mask` to find the chunk
- Used for debug assertions to ensure rollback state is consistent
- **Note**: Only chunk level tracks the specific change type (created/changed/removed); storage/page levels only track that *something* changed

---

## Performance Characteristics

### Space Complexity

- **Storage**: O(n) where n is the number of stored items (sparse structure)
- **RollbackStorage**: O(m) where m is the number of changed/removed items (only stores diffs)

### Time Complexity

- **get()**: O(1) - direct index calculation and mask check
- **set()**: O(1) - direct index calculation, mask updates, and rollback updates
- **remove()**: O(1) - direct index calculation, mask updates, and rollback updates
- **clear_changed_masks()**: O(k) where k is the number of changed items (uses bit iteration)

### Bit Mask Operations

The system uses efficient bit operations:
- `mask.trailing_zeros()` - find first set bit
- `mask.trailing_ones()` - find consecutive set bits
- `mask & !((1u128 << len) - 1) << start` - clear a range of bits

This enables efficient iteration over sparse data structures.

---

## Common Pitfalls

1. **Dropping created_mask items**: Always check `changed_mask | removed_mask` before dropping, never include `created_mask`
2. **Inconsistent mask propagation**: Ensure `changed_mask` is updated at all three levels when any change occurs (chunk, page, storage)
3. **Forgetting to clear masks**: After processing, call `clear_changed_masks()` to reset state
4. **Fullness mask**: In Storage Chunks, `fullness_mask` equals `presence_mask` (if present, then full)
5. **Parent pointer invalidation**: Be careful with parent pointers when moving or copying structures
6. **Mask level semantics**: Remember that `changed_mask` at storage/page levels means "any change", while at chunk level it means "modified"

---

## Testing Recommendations

1. **Invariant verification**: Call `verify_invariants()` after every operation
2. **Rollback verification**: After set/remove, verify rollback masks are correct
3. **Edge cases**: Test with empty storage, full storage, single item, all items
4. **Mask consistency**: Verify masks are consistent across all three levels
5. **Memory safety**: Use tools like Miri to detect undefined behavior with MaybeUninit

---

## Summary

The storage system provides:
- **Efficient sparse storage** with O(1) operations
- **Hierarchical structure** for cache-friendly access patterns
- **Rollback support** with minimal overhead (only stores diffs)
- **Type safety** through careful MaybeUninit usage
- **Invariant checking** for debugging and validation

The rollback system provides:
- **Change tracking** between ticks
- **Minimal storage** (only stores old values for changed/removed items)
- **Simplified hierarchy** (only `changed_mask` at storage/page levels, full mask set at chunk level)
- **Hierarchical consistency** across all levels
- **Safe rollback** operations with proper value restoration

---

## Scheduler & Concurrency

### Execution Model

- The scheduler organizes systems into wavefronts (levels) computed from dependency constraints and executes wavefronts sequentially.
- Each wavefront contains systems that have no ordering edges between them; they can be executed concurrently without violating dependency rules.
- The execution order is derived from a dependency graph built from:
  - `before()/after()` edges for explicit orchestration.
  - `reads()/writes()` edges to prevent write→read and write→write hazards for the same component.
  - Group hierarchy (`parent()`) inheritance, which introduces additional `before/after` constraints from parent groups.

### Constraints Construction

- For every system `S`:
  - For each type in `S.before()`, add edges `S → T` to all systems of type `T`.
  - For each type in `S.after()`, add edges `T → S` to all systems of type `T`.
- For component access:
  - For every component `C`, add edges from any writer of `C` to any reader of `C` in the same tick.
  - If multiple writers for `C` exist, add a chain of edges among writers to serialize writes.
- For group inheritance:
  - Walk `parent()` chain; for each ancestor group, apply its `before()/after()` constraints to the member system.

### Wavefront Construction

- Perform a topological levelization over the dependency graph:
  - Collect all nodes with in-degree 0 into the first level; remove them and decrement successors’ in-degrees.
  - Repeat to produce subsequent levels until all nodes are covered.
  - Remaining nodes (if any cycles) are emitted together in a final level to surface misconfigurations.

### Parallel Execution Guidance

- Dispatch each wavefront to a thread pool; systems inside a wavefront can run in parallel.
- Keep group hierarchies shallow to minimize inherited constraints that reduce parallelism.
- Prefer explicit `before()/after()` only where necessary; overuse adds edges and lowers concurrency.
- Use `reads()/writes()` precisely; avoid shared writes across many systems, as they serialize execution.
- Ensure each system maintains storage invariants when using `ViewMut` so parallel waves remain safe.

### Practical Notes

- The current implementation computes wavefronts and runs systems per wavefront; replacing per-wave sequential execution with thread-pool dispatch enables parallelism without changing dependency semantics.
- Parallel scheduling must respect the per-wave independence guaranteed by the dependency graph; cross-wave dependencies remain strictly ordered.


## System Scheduling

### Goals

- Generate parallelizable "wavefronts" of systems per tick.
- Respect explicit ordering constraints (`before()`, `after()`).
- Enforce data access safety based on `reads()` and `writes()`.
- Respect optional `SystemGroup` constraints (`before()`, `after()`), including nested groups.

### Inputs

- `System` provides: `before()`, `after()`, `reads()`, `writes()` and optional `parent()` (a `SystemGroup`).
- `SystemGroup` provides: `before()`, `after()`, optionally `reads()`, `writes()`, and optional `parent()` for nesting.

### Dependency Graph Construction

- Nodes: one per `System` instance in the scheduler.
- Edges are added for:
  - Direct ordering:
    - For each `S`: add `S -> T` for all `T` whose concrete type matches any type in `S.before()`.
    - For each `S`: add `T -> S` for all `T` whose concrete type matches any type in `S.after()`.
  - Read/Write conflicts between systems `i` and `j`:
    - If `i.writes ∩ j.reads ≠ ∅`, add `i -> j`.
    - If `j.writes ∩ i.reads ≠ ∅`, add `j -> i`.
    - If `i.writes ∩ j.writes ≠ ∅` and neither of the above applies, use insertion order: add `i -> j`.
  - Group ordering inheritance for each system `S` with `parent()` = `G`:
    - Treat `G.before()` as additional `S.before()`; add `S -> K` for matching `K` types.
    - Treat `G.after()` as additional `S.after()`; add `K -> S` for matching `K` types.
    - Repeat inheritance up the `parent()` chain for nested groups.


### Wavefront Generation (Parallel Sets)

- Use a levelized variant of Kahn’s algorithm:
  - Initialize `in_degree[v]` for all nodes `v`.
  - While there are unprocessed nodes:
    - Collect current wavefront `W = { v | in_degree[v] == 0 and v not processed }`.
    - `W` is safe to run in parallel (no unresolved ordering edges between members).
    - For each `v ∈ W`: mark processed; decrement `in_degree` of its outgoing neighbors.
    - Emit `W` and continue until all nodes are processed.
- If a cycle is detected (no zero in-degree nodes remain, but unprocessed nodes exist), fall back to insertion order for the remaining nodes or report an error in debug builds.

### Concurrency Semantics

- Within a wavefront:
  - Systems in `W` can execute concurrently because all required predecessors have completed.
  - Data safety is guaranteed by prior edge construction using `reads()`/`writes()`.
- Across `SystemGroup` boundaries:
  - A group’s `before()`/`after()` constraints are inherited by its descendants, shaping edges so that descendants honor external ordering.
  - Execution within a concrete group type may still be sequential if the group’s own `run()` chooses to run children in sequence.

### Current Implementation Notes

- The scheduler computes a linear execution order via topological sort and runs systems sequentially.
- To enable parallel execution, replace linear emission with wavefront emission as described above; each wavefront can be dispatched to a thread pool.

### Practical Guidance

- Prefer explicit `before()/after()` for coarse-grained orchestration; rely on `reads()/writes()` for fine-grained safety.
- Keep group hierarchies shallow where possible; deep nesting increases inherited constraints.
- Avoid unnecessary shared writes across many systems; it reduces parallelization potential by adding edges.

