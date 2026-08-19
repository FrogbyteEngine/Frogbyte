# frogbyte_ecs

Entity Component System for Frogbyte.

This crate is currently in an early foundation phase. It provides generational
entity identity and allocation, type-erased component storage, and basic
archetype storage.

Queries, archetype transitions, and world-level orchestration are not
implemented yet.

## Entities

An [`Entity`](src/entity/mod.rs) is a generational handle made of an `index`
and a `generation`.

The index identifies a slot in the allocator, while the generation
distinguishes successive entities that occupied the same slot.

Two entities are equal only when both their index and generation match. This
prevents stale entity handles from being mistaken for newer entities that
reuse the same allocator slot.

## EntityAllocator

[`EntityAllocator`](src/entity/entity_allocator.rs) owns entity slots and is
responsible for creating and removing entities.

- `create()` returns a new live entity. A previously removed slot may be reused
  with an updated generation.
- `remove(entity)` releases a live entity slot and rejects invalid, dead, or
  stale handles.

Slot reuse ordering is an implementation detail and should not be relied upon.

## Component storage

Components implement the [`Component`](src/component/mod.rs) marker trait.

[`BlobVec`](src/component/blobvec.rs) provides contiguous, type-erased storage
for values of one concrete component type.

It stores the component layout, `TypeId`, and destructor required to manage
values without knowing their concrete type at runtime.

`ComponentSet` represents heterogeneous component tuples used to create and
populate archetype columns.

The trait is sealed: external crates can use the supported tuple
implementations but cannot provide arbitrary `ComponentSet` implementations.

## Archetypes

[`Archetype`](src/archetype/mod.rs) stores entities that share one exact set of
component types.

Each component type is stored in its own contiguous `BlobVec` column. Entity
rows and component rows remain aligned:

```text
Entity       Position       Velocity
------       --------       --------
E0           P0             V0
E1           P1             V1
E2           P2             V2