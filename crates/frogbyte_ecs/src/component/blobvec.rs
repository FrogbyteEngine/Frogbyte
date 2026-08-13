use std::{
    alloc::{Layout, alloc, dealloc, handle_alloc_error, realloc},
    any::TypeId,
    ptr::NonNull,
};

use crate::component::Component;

pub struct BlobVec {
    ptr: NonNull<u8>,
    capacity: usize,
    len: usize,
    layout: Layout,
    type_id: TypeId,
    drop_fn: unsafe fn(*mut u8),
}

impl Drop for BlobVec {
    fn drop(&mut self) {
        for index in 0..self.len {
            let data = unsafe { self.ptr.add(index * self.layout.size()).as_ptr() };

            unsafe {
                (self.drop_fn)(data);
            }
        }

        if self.capacity != 0 {
            let (total_layout, _) = self.layout.repeat(self.capacity).unwrap();

            unsafe {
                dealloc(self.ptr.as_ptr(), total_layout);
            }
        }
    }
}

impl BlobVec {
    pub fn new<T: Component + 'static>() -> Self {
        Self {
            ptr: NonNull::dangling(),
            capacity: 0,
            len: 0,
            layout: Layout::new::<T>(),
            type_id: TypeId::of::<T>(),
            drop_fn: Self::drop_value::<T>,
        }
    }

    fn grow(&mut self) {
        let new_capacity = if self.capacity == 0 {
            1
        } else {
            self.capacity
                .checked_mul(2)
                .expect("Error: BlobVec max size capacity reached.")
        };

        let (new_layout, _) = Layout::repeat(&self.layout, new_capacity).unwrap();

        let new_ptr = if self.capacity == 0 {
            unsafe { alloc(new_layout) }
        } else {
            let (old_layout, _) = self.layout.repeat(self.capacity).unwrap();
            let old_ptr = self.ptr.as_ptr() as *mut u8;
            unsafe { realloc(old_ptr, old_layout, new_layout.size()) }
        };

        self.ptr = match NonNull::new(new_ptr as *mut u8) {
            Some(p) => p,
            None => handle_alloc_error(new_layout),
        };

        self.capacity = new_capacity;
    }

    pub fn push<T: Component + 'static>(&mut self, value: T) {
        assert_eq!(self.type_id, TypeId::of::<T>());

        if self.len >= self.capacity {
            self.grow();
        }

        let ptr = (unsafe { self.ptr.add(self.layout.size() * self.len).as_ptr() }) as *mut T;
        unsafe { ptr.write(value) };

        self.len += 1;
    }

    pub fn pop<T: Component + 'static>(&mut self) -> Option<T> {
        assert_eq!(self.type_id, TypeId::of::<T>());

        if self.len == 0 {
            return None;
        }

        self.len -= 1;

        let raw_value = unsafe { self.ptr.add(self.len * self.layout.size()).as_ptr() } as *mut T;
        Some(unsafe { raw_value.read() })
    }

    pub fn swap_remove<T: Component + 'static>(&mut self, index: usize) -> T {
        assert!(index < self.len);
        assert_eq!(self.type_id, TypeId::of::<T>());

        self.len -= 1;

        let ptr_to_remove = unsafe { self.ptr.add(index * self.layout.size()).as_ptr() as *mut T };
        let value_to_remove = unsafe { ptr_to_remove.read() };

        if self.len != index {
            let raw_data_to_swap =
                unsafe { self.ptr.add(self.len * self.layout.size()).as_ptr() as *mut T };
            let value_to_swap = unsafe { raw_data_to_swap.read() };
            unsafe { ptr_to_remove.write(value_to_swap) };
        }

        value_to_remove
    }

    pub fn get<T: Component + 'static>(&self, index: usize) -> Option<&T> {
        assert_eq!(self.type_id, TypeId::of::<T>());

        if index >= self.len {
            return None;
        }

        let raw_data = unsafe { self.ptr.add(index * self.layout.size()).as_ptr() as *const T };
        Some(unsafe { &*raw_data })
    }

    pub fn get_mut<T: Component + 'static>(&mut self, index: usize) -> Option<&mut T> {
        assert_eq!(self.type_id, TypeId::of::<T>());

        if index >= self.len {
            return None;
        }

        let raw_data = unsafe { self.ptr.add(index * self.layout.size()).as_ptr() } as *mut T;
        Some(unsafe { &mut *raw_data })
    }

    unsafe fn drop_value<T: Component + 'static>(ptr: *mut u8) {
        unsafe {
            ptr.cast::<T>().drop_in_place();
        }
    }
}
