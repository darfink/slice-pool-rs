//! This crate provides functionality for wrapping a slice and exposing it as a
//! chunkable interface (i.e acts as a memory pool).
//!
//! The underlying memory can be a mutable slice of any type.
//!
//! ```
//! use slice_pool::SlicePool;
//!
//! let mut values = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
//! let mut memory = SlicePool::new(&mut values);
//!
//! // Not enough memory available (only 10 elements)
//! assert!(memory.allocate(11).is_none());
//!
//! let mut first = memory.allocate(2).unwrap();
//! assert_eq!(*first, [10, 20]);
//! first[1] = 15;
//! assert_eq!(*first, [10, 15]);
//!
//! // Amount of chunks (i.e the fragmentation)
//! assert_eq!(memory.len(), 2);
//!
//! let mem2 = memory.allocate(5).unwrap();
//! assert_eq!(*mem2, [30, 40, 50, 60, 70]);
//! assert_eq!(memory.len(), 3);
//! ```

use std::mem;
use std::ops::{Deref, DerefMut};
use std::cell::RefCell;
use std::rc::Rc;
use std::fmt;

/// An interface for allocating chunks in a slice.
pub struct SlicePool<'a, T: 'a>(Rc<RefCell<ChunkableInner<'a, T>>>);

impl<'a, T> SlicePool<'a, T> {
    /// Wraps a slice with a chunkable interface.
    pub fn new(slice: &'a mut [T]) -> Self {
        SlicePool(Rc::new(RefCell::new(ChunkableInner {
            values: vec![Chunk { size: slice.len(), offset: 0, free: true }],
            memory: slice,
        })))
    }

    /// Allocates a new chunk in the slice.
    pub fn allocate(&mut self, size: usize) -> Option<ChunkRef<'a, T>> {
        (*self.0).borrow_mut()
            .allocate(size)
            .map(|slice| ChunkRef { inner: self.0.clone(), data: slice })
    }

    /// Returns the number of chunks in the slice.
    pub fn len(&self) -> usize {
        (*self.0).borrow().values.len()
    }
}

/// A reference to an allocated chunk.
pub struct ChunkRef<'a, T: 'a> {
    inner: Rc<RefCell<ChunkableInner<'a, T>>>,
    data: &'a mut [T],
}

impl<'a, T> fmt::Debug for ChunkRef<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", *self)
    }
}

impl<'a, T> Deref for ChunkRef<'a, T> {
    type Target = [T];

    fn deref<'b>(&'b self) -> &'b Self::Target {
        &self.data
    }
}

impl<'a, T> DerefMut for ChunkRef<'a, T> {
    fn deref_mut<'b>(&'b mut self) -> &'b mut [T] {
        self.data
    }
}

impl<'a, T> Drop for ChunkRef<'a, T> {
    /// Returns the ownership of the slice.
    fn drop(&mut self) {
        unsafe { (*self.inner).borrow_mut().release(self.data) };
    }
}

/// A chunk of memory inside a slice.
#[derive(Debug, Clone)]
struct Chunk {
    offset: usize,
    size: usize,
    free: bool,
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
    unsafe fn release<'s>(&mut self, slice: &'s mut [T]) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let mut values = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
        let mut memory = SlicePool::new(&mut values);

        let mem = {
            let mem = memory.allocate(2).unwrap();
            assert_eq!(memory.len(), 2);
            assert_eq!(*mem, [10, 20]);
            {
                let mem = memory.allocate(5).unwrap();
                assert_eq!(memory.len(), 3);
                assert_eq!(*mem, [30, 40, 50, 60, 70]);
            }

            let mem = memory.allocate(1).unwrap();
            assert_eq!(memory.len(), 3);
            assert_eq!(*mem, [30]);
            mem
        };
        assert_eq!(memory.len(), 3);
        assert_eq!(*mem, [30]);
    }
}
