use crate::component::Component;
use crate::storage::{Chunk, Page};
use std::mem::MaybeUninit;
use std::ptr::NonNull;

#[repr(align(64))]
pub struct PoolPage<U> {
    items: [MaybeUninit<NonNull<U>>; 32],
    len: usize,
}

impl<U> PoolPage<U> {
    fn new() -> Self {
        Self {
            items: unsafe { MaybeUninit::uninit().assume_init() },
            len: 0,
        }
    }
}

pub struct Pool<U> {
    pages: [MaybeUninit<NonNull<PoolPage<U>>>; 128],
    len: usize,
}

impl<U> Pool<U> {
    pub fn new() -> Self {
        Self {
            pages: unsafe { MaybeUninit::uninit().assume_init() },
            len: 0,
        }
    }

    fn last_page_mut(&mut self) -> Option<&mut PoolPage<U>> {
        if self.len == 0 {
            None
        } else {
            let nn = unsafe { self.pages[self.len - 1].assume_init() };
            Some(unsafe { &mut *nn.as_ptr() })
        }
    }

    fn push_new_page(&mut self) -> &mut PoolPage<U> {
        assert!(self.len < 128, "Pool pages capacity exceeded");
        let boxed = Box::new(PoolPage::new());
        let ptr = unsafe { NonNull::new_unchecked(Box::into_raw(boxed)) };
        self.pages[self.len].write(ptr);
        self.len += 1;
        unsafe { &mut *ptr.as_ptr() }
    }

    pub fn free(&mut self, ptr_u: *mut U) {
        for i in 0..self.len {
            let page: &mut PoolPage<U> = unsafe { &mut *self.pages[i].assume_init().as_ptr() };
            let len = page.len;
            for idx in 0..len {
                let inner_ptr = unsafe { page.items[idx].assume_init().as_ptr() };
                if inner_ptr == ptr_u {
                    let last = len - 1;
                    if idx != last {
                        unsafe { drop(Box::from_raw(page.items[idx].assume_init().as_ptr())) };
                        page.items[idx] = page.items[last];
                    } else {
                        unsafe { drop(Box::from_raw(page.items[idx].assume_init().as_ptr())) };
                    }
                    page.len -= 1;
                    return;
                }
            }
        }
    }
}

impl<U> Default for Pool<U> {
    fn default() -> Self {
        Self::new()
    }
}

impl<U> Drop for Pool<U> {
    fn drop(&mut self) {
        for i in 0..self.len {
            let ptr = unsafe { self.pages[i].assume_init().as_ptr() };
            unsafe { drop(Box::from_raw(ptr)) };
        }
        self.len = 0;
    }
}

impl<T: Component> Pool<Chunk<T>> {
    pub fn alloc_chunk_to_slot(
        &mut self,
        owner_index: u8,
        out_slot: &mut *mut Chunk<T>,
    ) -> *mut Chunk<T> {
        let page_ref: &mut PoolPage<Chunk<T>> = match self.last_page_mut() {
            Some(p) if p.len < 32 => unsafe { &mut *(p as *mut _) },
            _ => unsafe { &mut *(self.push_new_page() as *mut _) },
        };
        let idx = page_ref.len;
        let boxed = Box::new(Chunk::new());
        let inner_ptr = Box::into_raw(boxed);
        page_ref.items[idx].write(unsafe { NonNull::new_unchecked(inner_ptr) });
        page_ref.len += 1;
        let page = page_ref as *mut PoolPage<Chunk<T>>;
        unsafe {
            let chunk = &mut *inner_ptr;
            chunk.pool_page = page;
            chunk.pool_slot = idx as u8;
            chunk.owner_index = owner_index;
        }
        *out_slot = inner_ptr;
        debug_assert!(page_ref.len <= 32);
        inner_ptr
    }

    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn free_chunk(&mut self, ptr_chunk: *mut Chunk<T>) -> Option<(*mut Chunk<T>, u8)> {
        for i in 0..self.len {
            let page_ref: &mut PoolPage<Chunk<T>> =
                unsafe { &mut *self.pages[i].assume_init().as_ptr() };
            let len = page_ref.len;
            for idx in 0..len {
                if unsafe { page_ref.items[idx].assume_init().as_ptr() } == ptr_chunk {
                    let last = len - 1;
                    if idx != last {
                        unsafe { drop(Box::from_raw(page_ref.items[idx].assume_init().as_ptr())) };
                        page_ref.items[idx] = page_ref.items[last];
                        let moved_ptr = unsafe { page_ref.items[idx].assume_init().as_ptr() };
                        let moved_chunk = unsafe { &mut *moved_ptr };
                        moved_chunk.pool_slot = idx as u8;
                        moved_chunk.pool_page = page_ref as *mut _;
                        let owner_idx = moved_chunk.owner_index;
                        page_ref.len -= 1;
                        debug_assert!(page_ref.len <= 32);
                        return Some((moved_ptr, owner_idx));
                    } else {
                        unsafe { drop(Box::from_raw(page_ref.items[idx].assume_init().as_ptr())) };
                        page_ref.len -= 1;
                        debug_assert!(page_ref.len <= 32);
                        return None;
                    }
                }
            }
        }
        None
    }
}

impl<T: Component> Pool<Page<T>> {
    pub fn alloc_page_to_slot(
        &mut self,
        owner_index: u8,
        out_slot: &mut *mut Page<T>,
        chunk_pool: *mut Pool<Chunk<T>>,
        default_chunk_ptr: *const Chunk<T>,
    ) -> *mut Page<T> {
        let page_ref: &mut PoolPage<Page<T>> = match self.last_page_mut() {
            Some(p) if p.len < 32 => unsafe { &mut *(p as *mut _) },
            _ => unsafe { &mut *(self.push_new_page() as *mut _) },
        };
        let idx = page_ref.len;
        let boxed = Box::new(Page::new_with_pool(chunk_pool, default_chunk_ptr as *mut Chunk<T>));
        let inner_ptr = Box::into_raw(boxed);
        page_ref.items[idx].write(unsafe { NonNull::new_unchecked(inner_ptr) });
        page_ref.len += 1;
        let page = page_ref as *mut PoolPage<Page<T>>;
        unsafe {
            let pg = &mut *inner_ptr;
            pg.pool_page = page;
            pg.pool_slot = idx as u8;
            pg.owner_index = owner_index;
        }
        *out_slot = inner_ptr;
        debug_assert!(page_ref.len <= 32);
        inner_ptr
    }

    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn free_page(&mut self, ptr_page: *mut Page<T>) -> Option<(*mut Page<T>, u8)> {
        for i in 0..self.len {
            let page_ref: &mut PoolPage<Page<T>> =
                unsafe { &mut *self.pages[i].assume_init().as_ptr() };
            let len = page_ref.len;
            for idx in 0..len {
                if unsafe { page_ref.items[idx].assume_init().as_ptr() } == ptr_page {
                    let last = len - 1;
                    if idx != last {
                        unsafe { drop(Box::from_raw(page_ref.items[idx].assume_init().as_ptr())) };
                        page_ref.items[idx] = page_ref.items[last];
                        let moved_ptr = unsafe { page_ref.items[idx].assume_init().as_ptr() };
                        let moved_pg = unsafe { &mut *moved_ptr };
                        moved_pg.pool_slot = idx as u8;
                        moved_pg.pool_page = page_ref as *mut _;
                        let owner_idx = moved_pg.owner_index;
                        page_ref.len -= 1;
                        debug_assert!(page_ref.len <= 32);
                        return Some((moved_ptr, owner_idx));
                    } else {
                        unsafe { drop(Box::from_raw(page_ref.items[idx].assume_init().as_ptr())) };
                        page_ref.len -= 1;
                        debug_assert!(page_ref.len <= 32);
                        return None;
                    }
                }
            }
        }
        None
    }
}
