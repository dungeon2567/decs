use decs::entity::Entity;
use decs_macros::Component;
use std::any::TypeId;

use crate::storage::Storage;
use crate::system::{System, SystemGroup};
use crate::world::{HierarchyGroup, World};

#[derive(Debug, Component, Clone, PartialEq, Eq, Hash)]
pub struct Parent {
    pub first_child: Entity,
    pub last_child: Entity,
}

#[derive(Debug, Component, Clone, PartialEq, Eq, Hash)]
pub struct ChildOf {
    pub parent: Option<Entity>,
    pub next_sibling: Option<Entity>,
    pub prev_sibling: Option<Entity>,
    pub pending_parent: Option<Entity>,
}

pub struct UpdateHierarchySystem {
    child_storage: *mut Storage<ChildOf>,
    parent_storage: *mut Storage<Parent>,
    entity_storage: *mut Storage<Entity>,
}

impl UpdateHierarchySystem {
    pub fn new(world: &mut World) -> Self {
        let child_ptr = world.get_storage::<ChildOf>();
        let parent_ptr = world.get_storage::<Parent>();
        let entity_ptr = world.get_entity_storage();
        Self {
            child_storage: child_ptr,
            parent_storage: parent_ptr,
            entity_storage: entity_ptr,
        }
    }
}

impl System for UpdateHierarchySystem {
    fn run(&self, frame: &crate::frame::Frame) {
        let storage = unsafe { &mut *self.child_storage };
        let parents = unsafe { &mut *self.parent_storage };
        let entities = unsafe { &mut *self.entity_storage };
        // Pass 1: Collect all pending changes
        struct PendingChange {
            child: Entity,
            old_parent: Option<Entity>,
            new_parent: Entity,
        }
        let mut changes = Vec::new();

        let mut storage_mask = storage.changed_mask & storage.presence_mask;
        while storage_mask != 0 {
            let storage_start = storage_mask.trailing_zeros() as usize;
            let storage_shifted = storage_mask >> storage_start;
            let storage_run_len = storage_shifted.trailing_ones() as usize;
            for storage_idx in storage_start..storage_start + storage_run_len {
                let page = unsafe { &mut *storage.data[storage_idx] };
                let mut page_mask = page.changed_mask & page.presence_mask;
                while page_mask != 0 {
                    let page_start = page_mask.trailing_zeros() as usize;
                    let page_shifted = page_mask >> page_start;
                    let page_run_len = page_shifted.trailing_ones() as usize;
                    for page_idx in page_start..page_start + page_run_len {
                        let chunk = unsafe { &mut *page.data[page_idx] };
                        let mut chunk_mask = chunk.changed_mask & chunk.presence_mask;
                        while chunk_mask != 0 {
                            let chunk_start = chunk_mask.trailing_zeros() as usize;
                            let chunk_shifted = chunk_mask >> chunk_start;
                            let chunk_run_len = chunk_shifted.trailing_ones() as usize;
                            for idx in chunk_start..chunk_start + chunk_run_len {
                                let v = unsafe { chunk.data[idx].assume_init_mut() };
                                if let Some(new_parent) = v.pending_parent.take() {
                                    let global_index =
                                        (storage_idx * 64 * 64 + page_idx * 64 + idx) as u32;

                                    let me = entities
                                        .get(global_index)
                                        .copied()
                                        .unwrap_or_else(|| Entity::new(global_index, 1)); // Default gen 1 to avoid is_none() issues

                                    changes.push(PendingChange {
                                        child: me,
                                        old_parent: v.parent,
                                        new_parent,
                                    });
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

        // Pass 2: Apply changes
        for change in changes {
            let child_idx = change.child.index();

            // 1. Detach from old parent (if any)
            if let Some(old_parent) = change.old_parent {
                let (prev_sibling, next_sibling) = {
                    if let Some(child_comp) = storage.get(child_idx) {
                        (child_comp.prev_sibling, child_comp.next_sibling)
                    } else {
                        (None, None)
                    }
                };

                // Update siblings
                if let Some(prev) = prev_sibling
                    && let Some(pv) = storage.get_mut(frame, prev.index())
                {
                    pv.next_sibling = next_sibling;
                }
                if let Some(next) = next_sibling
                    && let Some(nv) = storage.get_mut(frame, next.index())
                {
                    nv.prev_sibling = prev_sibling;
                }

                // Update old parent
                let update_head = prev_sibling.is_none();
                let update_tail = next_sibling.is_none();

                if update_head || update_tail {
                    // Get parent mut - ensure we create it if missing (though it should exist if it was a parent)
                    // Use get_mut directly since we handle creation logic below if needed, but for old_parent it must exist
                    if let Some(old_p) = parents.get_mut(frame, old_parent.index()) {
                        if update_head {
                            old_p.first_child = next_sibling.unwrap_or(Entity::none());
                        }
                        if update_tail {
                            old_p.last_child = prev_sibling.unwrap_or(Entity::none());
                        }
                        if old_p.first_child.is_none() {
                            old_p.last_child = Entity::none();
                        }
                    }
                }
            }

            // 2. Attach to new parent
            // Create parent if missing
            if parents.get(change.new_parent.index()).is_none() {
                parents.set(
                    frame,
                    change.new_parent.index(),
                    Parent {
                        first_child: Entity::none(),
                        last_child: Entity::none(),
                    },
                );
            }

            if let Some(pv) = parents.get_mut(frame, change.new_parent.index()) {
                let tail = pv.last_child;
                
                // Update child component
                if let Some(child_comp) = storage.get_mut(frame, child_idx) {
                    child_comp.parent = Some(change.new_parent);
                    child_comp.next_sibling = None;
                    child_comp.prev_sibling = if tail.is_none() { None } else { Some(tail) };
                }

                // Update new parent and previous tail
                if tail.is_none() {
                    // First child
                    pv.first_child = change.child;
                    pv.last_child = change.child;
                } else {
                    // Append to tail
                    if let Some(tv) = storage.get_mut(frame, tail.index()) {
                        tv.next_sibling = Some(change.child);
                    }
                    pv.last_child = change.child;
                }
            }
        }
    }

    fn writes(&self) -> &'static [TypeId] {
        static W: &[TypeId] = &[TypeId::of::<ChildOf>(), TypeId::of::<Parent>()];
        W
    }

    fn parent(&self) -> Option<&dyn crate::system::SystemGroup> {
        Some(HierarchyGroup::instance())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// Safety: UpdateHierarchySystem is used within the scheduler on a single thread context.
// Raw storage pointer is only dereferenced during run with valid lifetime ensured by World.
unsafe impl Send for UpdateHierarchySystem {}
unsafe impl Sync for UpdateHierarchySystem {}
