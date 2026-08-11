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
    capacity: Box<[MaybeUninit<T>]>,
    len: usize,
}

impl<T: Comp> MyStorage<T> {
    pub fn new() -> Self {
        Self {
            capacity: Box::new([MaybeUninit::uninit()]),
            len: 0,
        }
    }

    pub fn grow(&mut self) {
        let new_size = self.capacity.len() * 2;
        let mut new_capacity = Box::<[T]>::new_uninit_slice(new_size);

        for i in 0..self.len {
            unsafe {
                let data_value = self.capacity[i].assume_init_read();
                new_capacity[i].write(data_value);
            }
        }

        self.capacity = new_capacity;
    }

    pub fn push(&mut self, value: T) {
        if self.len >= self.capacity.len() {
            self.grow();
        }

        self.capacity[self.len].write(value);
        self.len += 1;
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
