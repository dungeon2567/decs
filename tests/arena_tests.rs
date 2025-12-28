#![feature(allocator_api)]
use decs::arena::Arena;
use std::alloc::Allocator;
use std::alloc::Layout;

#[test]
fn test_large_alignment() {
    let arena = Arena::new();
    // Request alignment larger than chunk default (16)
    let layout = Layout::from_size_align(10, 4096).unwrap();
    
    // This should force a large padding in the new chunk or proper alignment in current
    let ptr1 = arena.allocate(layout).unwrap();
    
    // cast to *mut u8 to get address
    assert_eq!(ptr1.as_ptr() as *mut u8 as usize % 4096, 0);
    
    // Fill chunk with small allocations
    for _ in 0..1000 {
            let _ = arena.allocate(Layout::new::<u64>()).unwrap();
    }
    
    // Force another large alignment that might cross chunk boundary or require new chunk
    let ptr2 = arena.allocate(layout).unwrap();
    assert_eq!(ptr2.as_ptr() as *mut u8 as usize % 4096, 0);
}

#[test]
fn test_iterative_drop() {
    let arena = Arena::new();
    // Allocate many chunks to test stack overflow
    // CHUNK_SIZE is 64KB. Allocate 65KB items to force new chunks.
    let layout = Layout::from_size_align(65 * 1024, 16).unwrap();
    
    for _ in 0..10000 {
        let _ = arena.allocate(layout);
    }
    // Arena drops here. If recursive, it might overflow stack.
}
