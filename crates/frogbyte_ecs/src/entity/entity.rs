#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Entity {
    index: u32,
    generation: u32,
}

impl Entity {
    pub fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    pub fn index(&self) -> u32 {
        self.index
    }

    pub fn generation(&self) -> u32 {
        self.generation
    }
}