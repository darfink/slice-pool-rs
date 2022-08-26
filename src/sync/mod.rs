//! Synchronized memory pools.

pub use self::owned::{SliceBox, SlicePool, Sliceable};
use std::sync::Mutex;
use Chunk;

mod owned;

enum Order {
  Preceding,
  Following,
}

/// A thread-safe chunk chain.
struct ChunkChain(Mutex<Vec<Chunk>>);

impl ChunkChain {
  pub fn new(size: usize) -> Self {
    ChunkChain(Mutex::new(vec![Chunk::new(size)]))
  }

  pub fn allocate(&self, size: usize) -> Option<Chunk> {
    let mut chunks = self.0.lock().expect("poisoned chain");

    // Find a chunk with the least amount of memory required
    let (index, _) = chunks
      .iter()
      .enumerate()
      .filter(|(_, chunk)| chunk.free && chunk.size >= size)
      .min_by_key(|(_, chunk)| chunk.size)?;

    // Determine whether there is any memory surplus
    let delta = chunks[index].size - size;

    if delta > 0 {
      // Deduct the left over memory from the allocation
      chunks[index].size -= delta;

      // Insert a new chunk representing the surplus memory
      let offset = chunks[index].offset + size;
      chunks.insert(index + 1, Chunk::with_offset(delta, offset));
    }
    chunks[index].free = false;

    Some(chunks[index])
  }

  pub fn release(&self, offset: usize) {
    let mut chunks = self.0.lock().expect("poisoned chain");

    let index = chunks
      .binary_search_by_key(&offset, |chunk| chunk.offset)
      .expect("releasing chunk");

    let preceding_free = Self::has_free_adjacent(&chunks, index, Order::Preceding);
    let following_free = Self::has_free_adjacent(&chunks, index, Order::Following);

    if !preceding_free && !following_free {
      // No free adjacent chunks, simply mark this one as free
      chunks[index].free = true;
    } else {
      // Find range of free chunks.
      let start = if preceding_free { index - 1 } else { index };
      let end = if following_free { index + 1 } else { index };

      // Merge the free chunks
      chunks[start].free = true;
      chunks[start].size += chunks.drain(start+1..=end).map(|c| c.size).sum::<usize>();
    }
  }

  fn has_free_adjacent(chunks: &[Chunk], index: usize, order: Order) -> bool {
    match order {
      Order::Preceding => index > 0 && chunks[index - 1].free,
      Order::Following => index + 1 < chunks.len() && chunks[index + 1].free,
    }
  }
}
