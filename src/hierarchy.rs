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

                                    // Detach from old parent and neighbors
                                    if let Some(old_parent) = v.parent.take() {
                                        // We'll fetch old_p only if we need to modify it (head/tail changes)
                                        // But we need to update neighbors first to avoid borrow conflicts?
                                        // storage and parents are disjoint, so no conflict.

                                        if let Some(prev) = v.prev_sibling {
                                            if let Some(pv) = storage.get_mut(frame, prev.index()) {
                                                pv.next_sibling = v.next_sibling;
                                            }
                                        }

                                        if let Some(next) = v.next_sibling {
                                            if let Some(nv) = storage.get_mut(frame, next.index()) {
                                                nv.prev_sibling = v.prev_sibling;
                                            }
                                        }

                                        // Update old parent
                                        // Logic:
                                        // if prev_sibling is None, then I was first_child. Update first_child to next_sibling.
                                        // if next_sibling is None, then I was last_child. Update last_child to prev_sibling.

                                        let update_head = v.prev_sibling.is_none();
                                        let update_tail = v.next_sibling.is_none();

                                        if update_head || update_tail {
                                            if let Some(old_p) =
                                                parents.get_mut(frame, old_parent.index())
                                            {
                                                if update_head {
                                                    old_p.first_child =
                                                        v.next_sibling.unwrap_or(Entity::none());
                                                }
                                                if update_tail {
                                                    old_p.last_child =
                                                        v.prev_sibling.unwrap_or(Entity::none());
                                                }
                                                if old_p.first_child.is_none() {
                                                    old_p.last_child = Entity::none();
                                                }
                                            } else {
                                                // If it didn't exist, create default and update
                                                let mut old_p = Parent {
                                                    first_child: Entity::none(),
                                                    last_child: Entity::none(),
                                                };
                                                if update_head {
                                                    old_p.first_child =
                                                        v.next_sibling.unwrap_or(Entity::none());
                                                }
                                                if update_tail {
                                                    old_p.last_child =
                                                        v.prev_sibling.unwrap_or(Entity::none());
                                                }
                                                parents.set(frame, old_parent.index(), old_p);
                                            }
                                        }
                                    }

                                    v.parent = Some(new_parent);

                                    // Compute this child's Entity
                                    let me = entities
                                        .get(global_index)
                                        .copied()
                                        .unwrap_or_else(|| Entity::new(global_index, 0));

                                    // DEBUG
                                    println!(
                                        "Processing child: {:?}, new_parent: {:?}",
                                        me, new_parent
                                    );

                                    // Create parent if missing
                                    if parents.get(new_parent.index()).is_none() {
                                        parents.set(
                                            frame,
                                            new_parent.index(),
                                            Parent {
                                                first_child: Entity::none(),
                                                last_child: Entity::none(),
                                            },
                                        );
                                    }

                                    // Attach to tail (O(1))
                                    if let Some(pv) = parents.get_mut(frame, new_parent.index()) {
                                        let head = pv.first_child;
                                        let tail = pv.last_child;

                                        if head.is_none() {
                                            v.prev_sibling = None;
                                            v.next_sibling = None;
                                            pv.first_child = me;
                                            pv.last_child = me;
                                        } else {
                                            v.prev_sibling = Some(tail);
                                            v.next_sibling = None;
                                            if let Some(tv) = storage.get_mut(frame, tail.index()) {
                                                tv.next_sibling = Some(me);
                                            }
                                            pv.last_child = me;
                                        }
                                    }
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
