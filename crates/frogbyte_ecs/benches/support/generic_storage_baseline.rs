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

        unsafe {
            return Some(self.buffer[self.len].assume_init_read());
        }
    }

    pub(crate) fn swap_remove(&mut self, index: usize) -> T {
        assert!(index < self.len);

        let data_value_to_remove = unsafe { self.buffer[index].assume_init_read() };

        self.len -= 1;

        if index != self.len {
            unsafe {
                self.buffer[index].write(self.buffer[self.len].assume_init_read());
            }
        }

        data_value_to_remove
    }
}
