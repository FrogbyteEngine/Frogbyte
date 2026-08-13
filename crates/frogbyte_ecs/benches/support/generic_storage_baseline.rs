//! Benchmark-only baseline for component storage comparisons.
//!
//! This is the previous generic, non-type-erased storage implementation.
//! It is intentionally retained to compare storage strategies over time.
//!
//! This implementation is not production ECS code and must not be used
//! outside benchmarks.

use std::mem::MaybeUninit;

use frogbyte_ecs::component::Component;
pub(crate) struct GenericStorageBaseline<T> {
    buffer: Box<[MaybeUninit<T>]>,
    len: usize,
}

impl<T> Drop for GenericStorageBaseline<T> {
    fn drop(&mut self) {
        for i in 0..self.len {
            // SAFETY: every slot below `self.len` was initialized by `push`,
            // and the removal paths move values out only after shrinking that
            // prefix, so each stored component is dropped exactly once here.
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
            // SAFETY: slot `i` is inside the initialized prefix, and
            // `new_size` is twice the current buffer length, so the value is
            // moved out of an initialized slot into an in-bounds slot that is
            // still uninitialized. The old buffer holds only `MaybeUninit`
            // values, so releasing it does not drop the moved-out components.
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

        // SAFETY: `self.len` was just decremented to the index of the last
        // initialized slot, so the value is initialized. It now sits outside
        // the initialized prefix, so it is moved out exactly once.
        unsafe { Some(self.buffer[self.len].assume_init_read()) }
    }

    pub(crate) fn swap_remove(&mut self, index: usize) -> T {
        assert!(index < self.len);

        // SAFETY: the assertion above places `index` inside the initialized
        // prefix, so the component is initialized and is moved out once.
        let data_value_to_remove = unsafe { self.buffer[index].assume_init_read() };

        self.len -= 1;

        if index != self.len {
            // SAFETY: `self.len` now indexes the last initialized slot and
            // differs from `index`, so the last component is moved exactly
            // once into the slot that was just vacated, and no slot inside the
            // shortened prefix is left uninitialized.
            unsafe {
                self.buffer[index].write(self.buffer[self.len].assume_init_read());
            }
        }

        data_value_to_remove
    }
}
