use std::cell::Cell;
use std::rc::Rc;

use frogbyte_ecs::component::Component;
use frogbyte_ecs::component::blobvec::BlobVec;

#[derive(Debug, PartialEq, Clone, Copy)]
struct Item(i32);

impl Component for Item {}

struct Marker;

impl Component for Marker {}

struct DropCounter(Rc<Cell<u32>>);

impl Component for DropCounter {}

impl Drop for DropCounter {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

fn drop_counter(counter: &Rc<Cell<u32>>) -> DropCounter {
    DropCounter(Rc::clone(counter))
}

#[test]
fn push_and_get_returns_the_pushed_value() {
    let mut blob = BlobVec::new::<Item>();
    blob.push(Item(42));

    assert_eq!(blob.get::<Item>(0), Some(&Item(42)));
}

#[test]
fn push_multiple_values_are_retrievable_at_their_index() {
    let mut blob = BlobVec::new::<Item>();
    for i in 0..5 {
        blob.push(Item(i));
    }

    for i in 0..5 {
        assert_eq!(blob.get::<Item>(i as usize), Some(&Item(i)));
    }
}

#[test]
fn get_returns_none_for_out_of_bounds_index() {
    let mut blob = BlobVec::new::<Item>();
    blob.push(Item(1));

    assert_eq!(blob.get::<Item>(1), None);
}

#[test]
fn get_returns_none_on_a_freshly_created_blobvec() {
    let blob = BlobVec::new::<Item>();

    assert_eq!(blob.get::<Item>(0), None);
}

#[test]
fn get_mut_allows_in_place_mutation() {
    let mut blob = BlobVec::new::<Item>();
    blob.push(Item(1));

    if let Some(item) = blob.get_mut::<Item>(0) {
        item.0 = 99;
    }

    assert_eq!(blob.get::<Item>(0), Some(&Item(99)));
}

#[test]
fn pop_returns_values_in_lifo_order() {
    let mut blob = BlobVec::new::<Item>();
    blob.push(Item(1));
    blob.push(Item(2));
    blob.push(Item(3));

    assert_eq!(blob.pop::<Item>(), Some(Item(3)));
    assert_eq!(blob.pop::<Item>(), Some(Item(2)));
    assert_eq!(blob.pop::<Item>(), Some(Item(1)));
}

#[test]
fn pop_on_empty_blobvec_returns_none() {
    let mut blob = BlobVec::new::<Item>();

    assert_eq!(blob.pop::<Item>(), None);
}

#[test]
fn pop_shrinks_visible_length_so_the_slot_is_no_longer_reachable_via_get() {
    let mut blob = BlobVec::new::<Item>();
    blob.push(Item(1));
    blob.push(Item(2));

    blob.pop::<Item>();

    assert_eq!(blob.get::<Item>(1), None);
    assert_eq!(blob.get::<Item>(0), Some(&Item(1)));
}

#[test]
fn push_beyond_initial_capacity_preserves_all_values() {
    let mut blob = BlobVec::new::<Item>();
    let count = 100;

    for i in 0..count {
        blob.push(Item(i));
    }

    for i in 0..count {
        assert_eq!(blob.get::<Item>(i as usize), Some(&Item(i)));
    }
}

#[test]
fn swap_remove_moves_the_last_element_into_the_removed_slot() {
    let mut blob = BlobVec::new::<Item>();
    blob.push(Item(10));
    blob.push(Item(20));
    blob.push(Item(30));
    blob.push(Item(40));

    let removed = blob.swap_remove::<Item>(1);

    assert_eq!(removed, Item(20));
    assert_eq!(blob.get::<Item>(0), Some(&Item(10)));
    assert_eq!(blob.get::<Item>(1), Some(&Item(40)));
    assert_eq!(blob.get::<Item>(2), Some(&Item(30)));
    assert_eq!(blob.get::<Item>(3), None);
}

#[test]
fn swap_remove_of_the_last_index_does_not_corrupt_state() {
    let mut blob = BlobVec::new::<Item>();
    blob.push(Item(10));
    blob.push(Item(20));
    blob.push(Item(30));

    let removed = blob.swap_remove::<Item>(2);

    assert_eq!(removed, Item(30));
    assert_eq!(blob.get::<Item>(0), Some(&Item(10)));
    assert_eq!(blob.get::<Item>(1), Some(&Item(20)));
    assert_eq!(blob.get::<Item>(2), None);
}

#[test]
fn swap_remove_of_the_only_element_empties_the_blobvec() {
    let mut blob = BlobVec::new::<Item>();
    blob.push(Item(7));

    let removed = blob.swap_remove::<Item>(0);

    assert_eq!(removed, Item(7));
    assert_eq!(blob.get::<Item>(0), None);
}

#[test]
fn state_remains_consistent_after_interleaved_push_pop_and_swap_remove() {
    let mut blob = BlobVec::new::<Item>();

    blob.push(Item(1));
    blob.push(Item(2));
    blob.push(Item(3));
    blob.push(Item(4));

    // [1, 2, 3, 4] -> swap_remove(0) swaps the last element (4) into slot 0.
    assert_eq!(blob.swap_remove::<Item>(0), Item(1));
    // [4, 2, 3] -> pop removes the trailing element (3).
    assert_eq!(blob.pop::<Item>(), Some(Item(3)));
    // [4, 2] -> push appends a new trailing element.
    blob.push(Item(5));
    // [4, 2, 5]

    assert_eq!(blob.get::<Item>(0), Some(&Item(4)));
    assert_eq!(blob.get::<Item>(1), Some(&Item(2)));
    assert_eq!(blob.get::<Item>(2), Some(&Item(5)));
    assert_eq!(blob.get::<Item>(3), None);
}

#[test]
fn dropping_the_blobvec_drops_all_remaining_elements() {
    let counter = Rc::new(Cell::new(0));
    let mut blob = BlobVec::new::<DropCounter>();

    for _ in 0..4 {
        blob.push(drop_counter(&counter));
    }
    assert_eq!(counter.get(), 0);

    drop(blob);

    assert_eq!(counter.get(), 4);
}

#[test]
fn popped_values_are_dropped_exactly_once() {
    let counter = Rc::new(Cell::new(0));
    let mut blob = BlobVec::new::<DropCounter>();

    for _ in 0..3 {
        blob.push(drop_counter(&counter));
    }

    let popped = blob.pop::<DropCounter>().expect("value should be popped");
    assert_eq!(
        counter.get(),
        0,
        "a popped value must not be dropped until it goes out of scope itself"
    );

    drop(popped);
    assert_eq!(counter.get(), 1);

    drop(blob);
    assert_eq!(
        counter.get(),
        3,
        "the remaining elements must be dropped exactly once when the BlobVec is dropped"
    );
}

#[test]
fn swap_removed_values_are_dropped_exactly_once() {
    let counter = Rc::new(Cell::new(0));
    let mut blob = BlobVec::new::<DropCounter>();

    for _ in 0..3 {
        blob.push(drop_counter(&counter));
    }

    let removed = blob.swap_remove::<DropCounter>(0);
    assert_eq!(
        counter.get(),
        0,
        "a swap-removed value must not be dropped until it goes out of scope itself"
    );

    drop(removed);
    assert_eq!(counter.get(), 1);

    drop(blob);
    assert_eq!(
        counter.get(),
        3,
        "the remaining elements must be dropped exactly once when the BlobVec is dropped"
    );
}

#[test]
#[should_panic]
fn push_panics_on_component_type_mismatch() {
    let mut blob = BlobVec::new::<Item>();
    blob.push(Marker);
}

#[test]
#[should_panic]
fn get_panics_on_component_type_mismatch() {
    let mut blob = BlobVec::new::<Item>();
    blob.push(Item(1));

    let _ = blob.get::<Marker>(0);
}

#[test]
#[should_panic]
fn swap_remove_panics_on_out_of_bounds_index() {
    let mut blob = BlobVec::new::<Item>();
    blob.push(Item(1));

    blob.swap_remove::<Item>(1);
}
