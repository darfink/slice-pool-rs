//! This crate provides functionality for using a sliceable type as the
//! underlying memory for a pool.
//!
//! The allocated memory can be a mutable slice of any type.
//!
//! ```
//! use slice_pool::SlicePool;
//!
//! let values = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
//! let mut memory = SlicePool::new(values);
//! assert_eq!(memory.len(), 10);
//!
//! // Not enough memory available (only 10 elements)
//! assert!(memory.allocate(11).is_none());
//!
//! let mut first = memory.allocate(2).unwrap();
//! assert_eq!(*first, [10, 20]);
//! first[1] = 15;
//! assert_eq!(*first, [10, 15]);
//!
//! let mem2 = memory.allocate(5).unwrap();
//! assert_eq!(*mem2, [30, 40, 50, 60, 70]);
//! ```

pub use owned::{SlicePool, PoolVal, Sliceable};
pub use refd::{SlicePoolRef, PoolRef};

mod owned;
mod refd;

/// A chunk of memory inside a slice.
#[derive(Debug, Clone)]
struct Chunk {
    offset: usize,
    size: usize,
    free: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_ref() {
        let mut values = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
        let mut memory = SlicePoolRef::new(&mut values);

        let mem = {
            let mem = memory.allocate(2).unwrap();
            assert_eq!(*mem, [10, 20]);
            {
                let mem = memory.allocate(5).unwrap();
                assert_eq!(*mem, [30, 40, 50, 60, 70]);
            }

            let mem = memory.allocate(1).unwrap();
            assert_eq!(*mem, [30]);
            mem
        };
        assert_eq!(*mem, [30]);
    }

    #[test]
    fn pool_owned() {
        let mem = {
            let values = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
            let mut memory = SlicePool::new(values);

            let mem = {
                let mem = memory.allocate(2).unwrap();
                assert_eq!(*mem, [10, 20]);
                {
                    let mem = memory.allocate(5).unwrap();
                    assert_eq!(*mem, [30, 40, 50, 60, 70]);
                }

                let mem = memory.allocate(1).unwrap();
                assert_eq!(*mem, [30]);
                mem
            };
            assert_eq!(*mem, [30]);
            mem
        };
        assert_eq!(*mem, [30]);
    }
}
