use std::{cell::RefCell, rc::Rc};

use frogbyte_ecs::component::{Component, blobvec::BlobVec, position_component::Position};

#[derive(Debug, Clone, Copy, PartialEq)]
struct Counter(i32);

impl Component for Counter {}

/// Records the id of every value dropped, so tests can assert exactly which
/// values were dropped and how many times, catching double-drop and leak
/// bugs that a plain counter would miss.
struct DropRecorder {
    id: u32,
    drops: Rc<RefCell<Vec<u32>>>,
}

impl Component for DropRecorder {}

impl Drop for DropRecorder {
    fn drop(&mut self) {
        self.drops.borrow_mut().push(self.id);
    }
}

#[test]
fn new_blobvec_has_no_elements() {
    let mut blob = BlobVec::new::<Counter>();

    assert_eq!(blob.get::<Counter>(0), None);
    assert_eq!(blob.pop::<Counter>(), None);
}

#[test]
fn push_then_get_returns_the_stored_value() {
    let mut blob = BlobVec::new::<Counter>();

    blob.push(Counter(42));

    assert_eq!(blob.get::<Counter>(0), Some(&Counter(42)));
}

#[test]
fn get_returns_none_for_an_out_of_bounds_index() {
    let mut blob = BlobVec::new::<Counter>();
    blob.push(Counter(1));

    assert_eq!(blob.get::<Counter>(1), None);
}

#[test]
fn get_mut_allows_mutating_the_stored_value() {
    let mut blob = BlobVec::new::<Counter>();
    blob.push(Counter(1));

    if let Some(value) = blob.get_mut::<Counter>(0) {
        value.0 = 42;
    }

    assert_eq!(blob.get::<Counter>(0), Some(&Counter(42)));
}

#[test]
fn pop_returns_values_in_reverse_insertion_order() {
    let mut blob = BlobVec::new::<Counter>();
    blob.push(Counter(1));
    blob.push(Counter(2));
    blob.push(Counter(3));

    assert_eq!(blob.pop::<Counter>(), Some(Counter(3)));
    assert_eq!(blob.pop::<Counter>(), Some(Counter(2)));
    assert_eq!(blob.pop::<Counter>(), Some(Counter(1)));
    assert_eq!(blob.pop::<Counter>(), None);
}

#[test]
fn push_growing_past_initial_capacity_preserves_existing_values() {
    let mut blob = BlobVec::new::<Counter>();

    // 20 elements forces several doublings (1, 2, 4, 8, 16, 32), each backed
    // by a realloc: a faulty grow could corrupt or lose already-stored data.
    for i in 0..20 {
        blob.push(Counter(i));
    }

    for i in 0..20 {
        assert_eq!(blob.get::<Counter>(i as usize), Some(&Counter(i)));
    }
}

#[test]
fn swap_remove_moves_the_last_element_into_the_removed_slot() {
    let mut blob = BlobVec::new::<Counter>();
    blob.push(Counter(10));
    blob.push(Counter(20));
    blob.push(Counter(30));
    blob.push(Counter(40));

    let removed = blob.swap_remove::<Counter>(1);

    assert_eq!(removed, Counter(20));
    assert_eq!(blob.get::<Counter>(0), Some(&Counter(10)));
    assert_eq!(blob.get::<Counter>(1), Some(&Counter(40)));
    assert_eq!(blob.get::<Counter>(2), Some(&Counter(30)));
    assert_eq!(blob.get::<Counter>(3), None);
}

#[test]
fn swap_remove_of_the_last_element_shrinks_without_duplicating_it() {
    let mut blob = BlobVec::new::<Counter>();
    blob.push(Counter(1));
    blob.push(Counter(2));
    blob.push(Counter(3));

    let removed = blob.swap_remove::<Counter>(2);

    assert_eq!(removed, Counter(3));
    assert_eq!(blob.get::<Counter>(0), Some(&Counter(1)));
    assert_eq!(blob.get::<Counter>(1), Some(&Counter(2)));
    assert_eq!(blob.get::<Counter>(2), None);
}

#[test]
#[should_panic]
fn swap_remove_panics_for_an_out_of_bounds_index() {
    let mut blob = BlobVec::new::<Counter>();
    blob.push(Counter(1));

    blob.swap_remove::<Counter>(1);
}

#[test]
#[should_panic]
fn push_panics_when_value_type_does_not_match_the_blobvec_element_type() {
    let mut blob = BlobVec::new::<Counter>();

    blob.push(Position {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    });
}

#[test]
#[should_panic]
fn get_panics_when_queried_type_does_not_match_the_blobvec_element_type() {
    let mut blob = BlobVec::new::<Counter>();
    blob.push(Counter(1));

    let _ = blob.get::<Position>(0);
}

#[test]
fn dropping_the_blobvec_drops_every_remaining_value() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let mut blob = BlobVec::new::<DropRecorder>();

    for id in 0..5 {
        blob.push(DropRecorder {
            id,
            drops: Rc::clone(&drops),
        });
    }

    drop(blob);

    let mut recorded = drops.borrow().clone();
    recorded.sort_unstable();
    assert_eq!(recorded, vec![0, 1, 2, 3, 4]);
}

#[test]
fn pop_removes_the_value_without_dropping_it_twice() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let mut blob = BlobVec::new::<DropRecorder>();
    blob.push(DropRecorder {
        id: 7,
        drops: Rc::clone(&drops),
    });

    let popped = blob.pop::<DropRecorder>();
    assert!(
        drops.borrow().is_empty(),
        "pop must hand back ownership without dropping the value"
    );

    drop(popped);
    assert_eq!(*drops.borrow(), vec![7]);

    drop(blob);
    assert_eq!(
        *drops.borrow(),
        vec![7],
        "the now-empty BlobVec must not drop the value pop already returned"
    );
}

#[test]
fn swap_remove_removes_the_value_without_dropping_the_removed_or_remaining_values_twice() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let mut blob = BlobVec::new::<DropRecorder>();
    blob.push(DropRecorder {
        id: 1,
        drops: Rc::clone(&drops),
    });
    blob.push(DropRecorder {
        id: 2,
        drops: Rc::clone(&drops),
    });
    blob.push(DropRecorder {
        id: 3,
        drops: Rc::clone(&drops),
    });

    let removed = blob.swap_remove::<DropRecorder>(0);
    assert!(
        drops.borrow().is_empty(),
        "swap_remove must hand back ownership without dropping the value"
    );

    drop(removed);
    assert_eq!(*drops.borrow(), vec![1]);

    drop(blob);

    let mut recorded = drops.borrow().clone();
    recorded.sort_unstable();
    assert_eq!(
        recorded,
        vec![1, 2, 3],
        "the value moved out of the last slot must still be dropped exactly once"
    );
}
