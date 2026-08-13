//! Benchmark-only baseline for component storage comparisons.
//!
//! This is the previous generic, non-type-erased storage implementation.
//! It is retained as a reference for comparing storage strategies and is not
//! production ECS code.

use std::mem::MaybeUninit;

use frogbyte_ecs::component::Component;

/// Generic component storage retained as a benchmark baseline.
///
/// # Invariants
///
/// - `len <= buffer.len()`.
/// - Slots in `0..len` are initialized.
/// - Slots in `len..buffer.len()` are uninitialized.
pub(crate) struct GenericStorageBaseline<T> {
    buffer: Box<[MaybeUninit<T>]>,
    len: usize,
}

impl<T> Drop for GenericStorageBaseline<T> {
    fn drop(&mut self) {
        for i in 0..self.len {
            // SAFETY: [UNSAFE-001] Slots below `len` are initialized and have
            // not been moved out, so each remaining value is dropped once.
            unsafe {
                self.buffer[i].assume_init_drop();
            }
        }
    }
}

impl<T: Component> GenericStorageBaseline<T> {
    pub(crate) fn new() -> Self {
        Self {
            buffer: Box::new([MaybeUninit::uninit()]),
            len: 0,
        }
    }

    fn grow(&mut self) {
        let new_size = self.buffer.len() * 2;
        let mut new_capacity = Box::<[T]>::new_uninit_slice(new_size);

        for i in 0..self.len {
            // SAFETY: [UNSAFE-002] `i` is inside the initialized prefix and
            // the destination slot exists in the larger uninitialized buffer.
            // Reading moves the value, while the old `MaybeUninit` slot will
            // not drop it again.
            unsafe {
                let data_value = self.buffer[i].assume_init_read();
                new_capacity[i].write(data_value);
            }
        }

        self.buffer = new_capacity;
    }

    pub(crate) fn push(&mut self, value: T) {
        if self.len >= self.buffer.len() {
            self.grow();
        }

        self.buffer[self.len].write(value);
        self.len += 1;
    }

    pub(crate) fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }

        self.len -= 1;

        // SAFETY: [UNSAFE-003] The decremented length identifies the previous
        // last initialized slot. It is now outside the live prefix, so moving
        // the value out cannot cause it to be dropped twice.
        unsafe { Some(self.buffer[self.len].assume_init_read()) }
    }

    pub(crate) fn swap_remove(&mut self, index: usize) -> T {
        assert!(index < self.len);

        // SAFETY: [UNSAFE-004] The bounds check guarantees that `index`
        // contains an initialized value, which is moved out here.
        let data_value_to_remove = unsafe { self.buffer[index].assume_init_read() };

        self.len -= 1;

        if index != self.len {
            // SAFETY: [UNSAFE-005] `self.len` now identifies the previous last
            // initialized slot. Its value is moved into the vacant `index`
            // slot, restoring a fully initialized `0..len` prefix.
            unsafe {
                self.buffer[index].write(self.buffer[self.len].assume_init_read());
            }
        }

        data_value_to_remove
    }
}
