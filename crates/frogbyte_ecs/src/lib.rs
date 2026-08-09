//! Entity Component System for Frogbyte.
//!
//! This crate is in an early foundation phase. It currently provides
//! generational entity identity and allocation via [`entity::Entity`] and
//! [`entity::EntityAllocator`]; components, archetypes, and queries are not
//! implemented yet.
pub mod entity;
