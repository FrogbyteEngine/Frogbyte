//! Component storage: the [`Component`] marker trait and the
//! [`blobvec::BlobVec`] storage that holds components of a single type
//! contiguously.

pub mod blobvec;
pub mod position_component;

/// Marker trait for types that can be stored in a [`blobvec::BlobVec`].
pub trait Component {}
