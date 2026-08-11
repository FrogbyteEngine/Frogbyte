use std::{
    mem::{MaybeUninit, align_of, size_of},
    ptr::{self, NonNull},
};

pub trait Comp {}

#[derive(Debug, Clone, Copy)]
pub struct Position {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub struct Test {
    pub a: u8,
    pub b: u64,
}

pub struct StringTest {
    pub str: String,
}

impl Comp for Position {}

impl Comp for Test {}

impl Comp for StringTest {}

pub struct MyStorage<T> {
    buffer: Box<[MaybeUninit<T>]>,
    len: usize,
}

impl<T> Drop for MyStorage<T> {
    fn drop(&mut self) {
        for i in 0..self.len {
            unsafe {
                self.buffer[i].assume_init_drop();
            }
        }
    }
}

impl<T: Comp> MyStorage<T> {
    pub fn new() -> Self {
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

    pub fn push(&mut self, value: T) {
        if self.len >= self.buffer.len() {
            self.grow();
        }

        self.buffer[self.len].write(value);
        self.len += 1;
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }

        self.len -= 1;

        unsafe {
            return Some(self.buffer[self.len - 1].assume_init_read());
        }

    }
}

fn main() {
    let mut custom_vec = MyStorage::<Position>::new();

    let pos = Position {
        x: 77.0,
        y: 102.0,
        z: 0.11,
    };

    custom_vec.push(pos);
}
