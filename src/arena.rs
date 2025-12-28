use std::alloc::{AllocError, Allocator, Layout};
use std::cell::UnsafeCell;
use std::ptr::NonNull;

const CHUNK_SIZE: usize = 64 * 1024; // 64KB chunks

struct Chunk {
    data: *mut u8,
    layout: Layout,
    next: Option<Box<Chunk>>,
}

impl Chunk {
    fn new(size: usize) -> Option<Box<Self>> {
        // Align chunk memory to 16 bytes.
        // This gives us a known baseline, though we still calculate offsets dynamically.
        let layout = Layout::from_size_align(size, 16).ok()?;
        let data = unsafe { std::alloc::alloc(layout) };
        if data.is_null() {
            return None;
        }
        Some(Box::new(Self {
            data,
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
            // Iteratively drop the rest of the chain to prevent stack overflow
            let mut next = chunk.next.take();
            while let Some(mut c) = next {
                next = c.next.take();
            }
            
            // Reset pointer to start of data
            let ptr = chunk.data as usize;
            *self.ptr.get_mut() = ptr;
            *self.end.get_mut() = ptr + chunk.layout.size();
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
            let end = ptr + new_chunk.layout.size();
            
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
            let ptr = *self.ptr.get();
            let end = *self.end.get();
            
            // Try to align within current chunk
            let align_offset = (ptr as *const u8).align_offset(layout.align());
            
            let fits = if align_offset != usize::MAX {
                if let Some(aligned_ptr) = ptr.checked_add(align_offset) {
                    if let Some(new_ptr) = aligned_ptr.checked_add(layout.size()) {
                        new_ptr <= end
                    } else { false }
                } else { false }
            } else { false };

            if fits {
                let aligned_ptr = ptr + align_offset;
                let new_ptr = aligned_ptr + layout.size();
                *self.ptr.get() = new_ptr;
                let ptr_non_null = NonNull::new_unchecked(aligned_ptr as *mut u8);
                return Ok(NonNull::slice_from_raw_parts(ptr_non_null, layout.size()));
            }

            // Need new chunk. 
            // We must request enough space to cover the size AND potential alignment adjustment.
            // Since we don't know the base address of the new chunk yet, we assume worst-case padding.
            // Worst case padding is `layout.align() - 1`.
            let required_size = layout.size().checked_add(layout.align()).ok_or(AllocError)?;
            self.alloc_chunk(required_size)?;
            
            // Retry allocation in new chunk
            let ptr = *self.ptr.get();
            // The new chunk is fresh, so this should succeed
            let align_offset = (ptr as *const u8).align_offset(layout.align());
            let aligned_ptr = ptr + align_offset;
            let new_ptr = aligned_ptr + layout.size();
            
            // Verify bounds (sanity check)
            let end = *self.end.get();
            if new_ptr > end {
                return Err(AllocError);
            }
            
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

impl Drop for Arena {
    fn drop(&mut self) {
        // Iteratively drop the chain to prevent stack overflow
        let mut current = self.current.get_mut().take();
        while let Some(mut chunk) = current {
            current = chunk.next.take();
        }
    }
}
