use super::Component;

/// A 3D position component, in world-space units.
pub struct Position {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Component for Position {}
