use std::{mem, fmt};
use std::ops::{Deref, DerefMut};
use std::cell::RefCell;
use std::rc::Rc;

use super::Chunk;

/// An interface for allocating chunks in a referenced slice.
pub struct SlicePoolRef<'a, T: 'a>(Rc<RefCell<ChunkableInner<'a, T>>>);

impl<'a, T> SlicePoolRef<'a, T> {
    /// Wraps a slice with a chunkable interface.
    pub fn new(slice: &'a mut [T]) -> Self {
        SlicePoolRef(Rc::new(RefCell::new(ChunkableInner {
            values: vec![Chunk { size: slice.len(), offset: 0, free: true }],
            memory: slice,
        })))
    }

    /// Allocates a new chunk in the slice.
    pub fn allocate(&mut self, size: usize) -> Option<PoolRef<'a, T>> {
        (*self.0).borrow_mut()
            .allocate(size)
            .map(|slice| PoolRef { inner: self.0.clone(), data: slice })
    }

    /// Returns the pointer to the underlying slice.
    pub fn as_ptr(&self) -> *const T {
        (*self.0).borrow().memory.deref().as_ref().as_ptr()
    }

    /// Returns the size of the underlying slice.
    pub fn len(&self) -> usize {
        (*self.0).borrow().memory.len()
    }
}

/// A reference to an allocated chunk, that acts as a slice.
pub struct PoolRef<'a, T: 'a> {
    inner: Rc<RefCell<ChunkableInner<'a, T>>>,
    data: &'a mut [T],
}

impl<'a, T: fmt::Debug> fmt::Debug for PoolRef<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.deref())
    }
}

impl<'a, T> Deref for PoolRef<'a, T> {
    type Target = [T];

    fn deref<'b>(&'b self) -> &'b Self::Target {
        &self.data
    }
}

impl<'a, T> DerefMut for PoolRef<'a, T> {
    fn deref_mut<'b>(&'b mut self) -> &'b mut [T] {
        self.data
    }
}

impl<'a, T> Drop for PoolRef<'a, T> {
    /// Returns the ownership of the slice.
    fn drop(&mut self) {
        unsafe { (*self.inner).borrow_mut().release(self.data) };
    }
}

/// Shared reference to the slice data.
struct ChunkableInner<'a, T: 'a> {
    values: Vec<Chunk>,
    memory: &'a mut [T],
}

impl<'a, T> ChunkableInner<'a, T> {
    /// Tries to allocate a new chunk with `size` in the slice.
    fn allocate(&mut self, size: usize) -> Option<&'a mut [T]> {
        // Check if there is any free chunk index with the required amount of memory
        if let Some(index) = self.values.iter().position(|chunk| chunk.free && chunk.size >= size) {
            let mut chunk = self.values[index].clone();

            let chunk_range = chunk.offset..(chunk.offset + size);
            let delta_size = chunk.size - size;

            chunk.size -= delta_size;
            chunk.free = false;

            // Update the internal chunk
            self.values[index] = chunk;

            if delta_size > 0 {
                let adjacent_index = index + 1;

                // Insert any left-over memory as a new chunk
                self.values.insert(adjacent_index, Chunk {
                    offset: chunk_range.end,
                    size: delta_size,
                    free: true
                });

                self.defragment(adjacent_index);
            }

            Some(unsafe {
                // Create a reference to the slice associated with this chunk
                mem::transmute(&mut self.memory[chunk_range])
            })
        } else {
            None
        }
    }

    /// Releases an allocated chunk, idenfitifed by it's memory reference.
    unsafe fn release(&mut self, slice: &mut [T]) {
        let index = self.values.iter().position(|chunk| {
            // Identify the associated chunk by comparing slice offsets
            self.memory.as_ptr().offset(chunk.offset as isize) == slice.as_ptr()
        }).unwrap();

        self.values[index].free = true;
        self.defragment(index);
    }

    /// Merges up to three adjacent (free) chunks.
    fn defragment(&mut self, index: usize) {
        let adjacent_index = index + 1;

        // Determine if this chunk can be merged with the one after
        if self.values.get(adjacent_index).iter().any(|chunk| chunk.free) {
            self.values[index].size += self.values[adjacent_index].size;
            self.values.remove(adjacent_index);
        }

        // Determine if this chunk can be merged with the one before
        if index > 0 && self.values[index - 1].free {
            self.values[index - 1].size += self.values[index].size;
            self.values.remove(index);
        }
    }
}
