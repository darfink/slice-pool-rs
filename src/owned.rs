use std::{mem, fmt};
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};

use super::Chunk;

/// Interface for any object compatible with `SlicePool`.
pub trait Sliceable<T>: AsMut<[T]> + AsRef<[T]> { }

/// Implements the trait for vectors and similar types.
impl<T, V> Sliceable<T> for V where V: AsRef<[T]> + AsMut<[T]> { }

/// A thread-safe interface for allocating chunks in an owned slice.
pub struct SlicePool<T>(Arc<Mutex<ChunkableInner<T>>>);

impl<T> SlicePool<T> {
    /// Takes ownership of a slice with a chunkable interface
    pub fn new<Data: Sliceable<T> + 'static>(data: Data) -> Self {
        SlicePool(Arc::new(Mutex::new(ChunkableInner {
            values: vec![Chunk { size: data.as_ref().len(), offset: 0, free: true }],
            memory: Box::new(data),
        })))
    }

    /// Allocates a new chunk in the slice.
    pub fn allocate(&mut self, size: usize) -> Option<PoolVal<T>> {
        (*self.0).lock()
            .unwrap()
            .allocate(size)
            .map(|slice| PoolVal { inner: self.0.clone(), data: slice })
    }

    /// Returns the pointer to the underlying slice.
    pub fn as_ptr(&self) -> *const T {
        (*self.0).lock().unwrap().memory.deref().as_ref().as_ptr()
    }

    /// Returns the size of the underlying slice.
    pub fn len(&self) -> usize {
        (*self.0).lock().unwrap().memory.deref().as_ref().len()
    }
}

/// An allocated chunk, that acts as a slice.
pub struct PoolVal<T: 'static> {
    inner: Arc<Mutex<ChunkableInner<T>>>,
    data: &'static mut [T],
}

impl<T: fmt::Debug> fmt::Debug for PoolVal<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.deref())
    }
}

impl<T> Deref for PoolVal<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T> DerefMut for PoolVal<T> {
    fn deref_mut<'b>(&'b mut self) -> &'b mut [T] {
        self.data
    }
}

impl<T> Drop for PoolVal<T> {
    /// Returns the ownership of the slice.
    fn drop(&mut self) {
        unsafe { (*self.inner).lock().unwrap().release(self.data) };
    }
}

/// Shared reference to the slice data.
struct ChunkableInner<T> {
    values: Vec<Chunk>,
    memory: Box<Sliceable<T>>,
}

impl<T> ChunkableInner<T> {
    /// Tries to allocate a new chunk with `size` in the slice.
    fn allocate(&mut self, size: usize) -> Option<&'static mut [T]> {
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
                mem::transmute(&mut self.memory.deref_mut().as_mut()[chunk_range])
            })
        } else {
            None
        }
    }

    /// Releases an allocated chunk, idenfitifed by it's memory reference.
    unsafe fn release(&mut self, slice: &mut [T]) {
        let index = self.values.iter().position(|chunk| {
            // Identify the associated chunk by comparing slice offsets
            self.memory.deref().as_ref().as_ptr().offset(chunk.offset as isize) == slice.as_ptr()
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

/// Always accessed through a `Mutex`.
unsafe impl<T> Send for ChunkableInner<T> { }
