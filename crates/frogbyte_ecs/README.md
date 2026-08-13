# frogbyte_ecs

Entity Component System for Frogbyte.

This crate is in an early foundation phase. It currently provides generational
entity identity and allocation and per-type component storage; archetypes and
queries are not implemented yet.

## Entities

An [`Entity`](src/entity/mod.rs) is a generational handle made of an `index`
and a `generation`. The index identifies a slot in the allocator; the
generation distinguishes successive entities that have occupied the same
slot over time.

Two entities are equal only if both their index and generation match. This
means a handle captured before a slot was freed and reused is a different
value from the handle returned for the new occupant of that slot, even
though both share the same index.

## EntityAllocator

[`EntityAllocator`](src/entity/entity_allocator.rs) owns entity slots and is
responsible for creating and removing entities.

- `create()` returns a new, live entity. If a previously removed slot is
  available, it is reused with its generation incremented; otherwise a new
  slot is appended.
- `remove(entity)` releases an entity's slot back to the allocator so a
  future `create()` call may reuse it. It fails when the given handle does
  not currently refer to a live entity, which covers:
  - an index that was never allocated;
  - an index whose slot is currently dead (already removed);
  - a stale handle whose generation no longer matches the slot's current
    generation, even if that slot is alive again under a new occupant.

Because removal always increments the slot's generation before it is handed
out again, a stale handle can never be mistaken for the entity that replaced
it.

Freed slots are currently handed back out most-recently-freed first, so
callers should not depend on any particular reuse ordering beyond what is
covered by the crate's tests; it is an implementation detail of the
allocator, not a documented API guarantee.

## Components

[`Component`](src/component/mod.rs) is a marker trait for plain data types
that can be stored per-entity, such as
[`Position`](src/component/position_component.rs).

[`BlobVec`](src/component/blobvec.rs) is contiguous, type-erased storage for
the components of a single type. It is created for one concrete type, and
every later call is checked against that type, so a mismatched call panics
instead of reinterpreting the stored bytes.

`BlobVec` does not yet associate stored components with the entities that own
them; that link, together with archetypes and queries, is not implemented
yet.
