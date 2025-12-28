use std::alloc::{AllocError, Allocator, Layout};
use std::cell::UnsafeCell;
use std::ptr::NonNull;

const CHUNK_SIZE: usize = 64 * 1024; // 64KB chunks

struct Chunk {
    data: *mut u8,
    capacity: usize,
    layout: Layout,
    next: Option<Box<Chunk>>,
}

impl Chunk {
    fn new(size: usize) -> Option<Box<Self>> {
        let layout = Layout::from_size_align(size, 16).ok()?;
        let data = unsafe { std::alloc::alloc(layout) };
        if data.is_null() {
            return None;
        }
        Some(Box::new(Self {
            data,
            capacity: size,
            layout,
            next: None,
        }))
    }
}

impl Drop for Chunk {
    fn drop(&mut self) {
        unsafe {
            std::alloc::dealloc(self.data, self.layout);
        }
    }
}

pub struct Arena {
    current: UnsafeCell<Option<Box<Chunk>>>,
    ptr: UnsafeCell<usize>,
    end: UnsafeCell<usize>,
}

impl Arena {
    pub fn new() -> Self {
        Self {
            current: UnsafeCell::new(None),
            ptr: UnsafeCell::new(0),
            end: UnsafeCell::new(0),
        }
    }

    pub fn reset(&mut self) {
        // Exclusive access due to &mut self
        let current = self.current.get_mut();
        if let Some(chunk) = current {
            // Drop all older chunks to reclaim memory, keep the current (latest/largest) one
            chunk.next = None;
            
            // Reset pointer to start of data
            *self.ptr.get_mut() = chunk.data as usize;
            // End remains the same (capacity of the chunk)
        }
    }

    fn alloc_chunk(&self, size: usize) -> Result<(), AllocError> {
        let size = size.max(CHUNK_SIZE);
        let mut new_chunk = Chunk::new(size).ok_or(AllocError)?;
        
        unsafe {
            let current_chunk = &mut *self.current.get();
            // Move current chunk to be the next of the new chunk
            new_chunk.next = current_chunk.take();
            
            let ptr = new_chunk.data as usize;
            let end = ptr + new_chunk.capacity;
            
            *current_chunk = Some(new_chunk);
            *self.ptr.get() = ptr;
            *self.end.get() = end;
        }
        
        Ok(())
    }
}

unsafe impl Allocator for Arena {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        unsafe {
            let mut ptr = *self.ptr.get();
            let end = *self.end.get();
            
            // Align pointer
            let align_offset = (ptr as *const u8).align_offset(layout.align());
            if align_offset != usize::MAX {
                if let Some(aligned_ptr) = ptr.checked_add(align_offset) {
                    if let Some(new_ptr) = aligned_ptr.checked_add(layout.size()) {
                        if new_ptr <= end {
                            *self.ptr.get() = new_ptr;
                            let ptr_non_null = NonNull::new_unchecked(aligned_ptr as *mut u8);
                            return Ok(NonNull::slice_from_raw_parts(ptr_non_null, layout.size()));
                        }
                    }
                }
            }

            // Need new chunk
            self.alloc_chunk(layout.size().max(layout.align()))?;
            
            // Retry allocation in new chunk
            let ptr = *self.ptr.get();
            // We know the new chunk is fresh, so just align
            let align_offset = (ptr as *const u8).align_offset(layout.align());
             // Should always succeed in a fresh chunk large enough
            let aligned_ptr = ptr + align_offset;
            let new_ptr = aligned_ptr + layout.size();
            
            *self.ptr.get() = new_ptr;
            let ptr_non_null = NonNull::new_unchecked(aligned_ptr as *mut u8);
            Ok(NonNull::slice_from_raw_parts(ptr_non_null, layout.size()))
        }
    }

    unsafe fn deallocate(&self, _ptr: NonNull<u8>, _layout: Layout) {
        // Arena allocator does not support individual deallocation
    }
}

impl Default for Arena {
    fn default() -> Self {
        Self::new()
    }
}
