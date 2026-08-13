//! Contiguous, type-erased storage for a single [`Component`] type.

use std::{
    alloc::{Layout, alloc, dealloc, handle_alloc_error, realloc},
    any::TypeId,
    ptr::NonNull,
};

use crate::component::Component;

/// Contiguous, type-erased storage for a single [`Component`] type.
///
/// A `BlobVec` is created for one concrete type `T` (see [`BlobVec::new`])
/// and rejects any other type at its call boundary: every method that reads
/// or writes an element takes its own `T` and asserts it matches the type
/// the vector was created with before touching the buffer.
///
/// # Invariants
///
/// - `ptr` is `NonNull::dangling()` while `capacity == 0`; once `capacity` is
///   non-zero, `ptr` points to a live allocation sized for exactly
///   `capacity` elements laid out according to `layout`.
/// - `len <= capacity`. Slots `0..len` hold live, initialized values of the
///   type identified by `type_id`; slots `len..capacity` are uninitialized.
/// - `layout`, `type_id`, and `drop_fn` are fixed for the lifetime of the
///   vector and always describe the same type `T` it was created with.
pub struct BlobVec {
    /// Start of the backing allocation, or dangling while `capacity == 0`.
    ptr: NonNull<u8>,
    /// Number of elements the current allocation can hold.
    capacity: usize,
    /// Number of initialized elements currently stored; always `<= capacity`.
    len: usize,
    /// Size and alignment of a single stored element.
    layout: Layout,
    /// The element type this vector was created for, checked against every
    /// accessor's `T` before the buffer is touched.
    type_id: TypeId,
    /// Drops the value at a given element pointer as the `T` this vector was
    /// created for.
    drop_fn: unsafe fn(*mut u8),
}

impl Drop for BlobVec {
    fn drop(&mut self) {
        for index in 0..self.len {
            // SAFETY[UNSAFE-001]: `index` is within `0..self.len`, and
            // `self.len <= self.capacity` (struct invariant), so the byte
            // offset stays inside the `capacity`-element allocation `self.ptr`
            // points to, giving a pointer to the value stored at `index`.
            let data = unsafe { self.ptr.add(index * self.layout.size()).as_ptr() };

            // SAFETY[UNSAFE-002]: `drop_fn` is `Self::drop_value::<T>` for the
            // same `T` this vector was created with, and `data` addresses a
            // slot in `0..self.len` that was initialized by `push` and has
            // not been read out since, so this drops a live value of the
            // right type exactly once.
            unsafe {
                (self.drop_fn)(data);
            }
        }

        if self.capacity != 0 {
            let (total_layout, _) = self.layout.repeat(self.capacity).unwrap();

            // SAFETY[UNSAFE-003]: `self.capacity != 0` means `self.ptr` was
            // last produced by the `alloc`/`realloc` call in `grow` that set
            // `self.capacity` to its current value, using a layout computed
            // the same way from the same `self.layout`, so `total_layout`
            // matches the layout that allocation is still live under.
            unsafe {
                dealloc(self.ptr.as_ptr(), total_layout);
            }
        }
    }
}

impl BlobVec {
    /// Creates an empty, zero-capacity storage for `T`.
    ///
    /// Every later call must use the same `T`; mismatched calls panic
    /// instead of reinterpreting the stored bytes.
    ///
    /// `T` must have a non-zero size: [`BlobVec::grow`] allocates with a
    /// byte size derived from `T`'s layout, and a zero-sized `T` would make
    /// that allocation invalid. `BlobVec` does not currently guard against
    /// zero-sized components.
    pub fn new<T: Component + 'static>() -> Self {
        Self {
            ptr: NonNull::dangling(),
            capacity: 0,
            len: 0,
            layout: Layout::new::<T>(),
            type_id: TypeId::of::<T>(),
            drop_fn: Self::drop_value::<T>,
        }
    }

    /// Grows the backing allocation to the next capacity, preserving every
    /// already-stored byte in place.
    ///
    /// Doubles `capacity`, or allocates room for one element if `capacity`
    /// is `0`. The allocation is grown or moved as a whole; stored elements
    /// keep their existing slot index, which is why `push` can write
    /// straight into slot `self.len` after calling this.
    ///
    /// # Panics
    ///
    /// Panics if doubling the capacity would overflow `usize`, or if the
    /// allocator reports allocation failure.
    fn grow(&mut self) {
        let new_capacity = if self.capacity == 0 {
            1
        } else {
            self.capacity
                .checked_mul(2)
                .expect("Error: BlobVec max size capacity reached.")
        };

        let (new_layout, _) = Layout::repeat(&self.layout, new_capacity).unwrap();

        let new_ptr = if self.capacity == 0 {
            // SAFETY[UNSAFE-004]: `alloc` requires a non-zero-size layout.
            // `new_capacity` is `1` here, so this holds as long as `T` is not
            // a zero-sized type (see [`BlobVec::new`]).
            unsafe { alloc(new_layout) }
        } else {
            let (old_layout, _) = self.layout.repeat(self.capacity).unwrap();
            let old_ptr = self.ptr.as_ptr() as *mut u8;
            // SAFETY[UNSAFE-005]: `self.capacity != 0` means `old_ptr` was
            // itself produced by a previous `alloc`/`realloc` call in this
            // function using a layout computed the same way from the same
            // `self.layout`, so `old_layout` matches the layout that
            // allocation is still live under, and `new_layout` has a larger
            // size computed from the same element layout and alignment.
            unsafe { realloc(old_ptr, old_layout, new_layout.size()) }
        };

        self.ptr = match NonNull::new(new_ptr as *mut u8) {
            Some(p) => p,
            None => handle_alloc_error(new_layout),
        };

        self.capacity = new_capacity;
    }

    /// Appends `value` as a new last element.
    ///
    /// # Panics
    ///
    /// Panics if `T` is not the type this vector was created for.
    pub fn push<T: Component + 'static>(&mut self, value: T) {
        assert_eq!(self.type_id, TypeId::of::<T>());

        if self.len >= self.capacity {
            self.grow();
        }

        // SAFETY[UNSAFE-006]: `self.len < self.capacity` after the growth
        // check above, so the byte offset stays inside the allocation and
        // addresses the first uninitialized slot; the assertion above proved
        // `T` matches the layout that offset was computed with, so writing a
        // `T` there is correctly aligned and does not overlap a live value.
        let ptr = (unsafe { self.ptr.add(self.layout.size() * self.len).as_ptr() }) as *mut T;
        unsafe { ptr.write(value) };

        self.len += 1;
    }

    /// Removes and returns the last element, or `None` if the vector is
    /// empty.
    ///
    /// # Panics
    ///
    /// Panics if `T` is not the type this vector was created for.
    pub fn pop<T: Component + 'static>(&mut self) -> Option<T> {
        assert_eq!(self.type_id, TypeId::of::<T>());

        if self.len == 0 {
            return None;
        }

        self.len -= 1;

        // SAFETY[UNSAFE-007]: `self.len` was just decremented to the index of
        // the last initialized slot, which stays inside the allocation, and
        // the assertion above proved `T` matches the layout that slot was
        // written with, so reading a `T` out of it is sound. The slot is now
        // outside `0..self.len`, so `Drop` will not read it again and this
        // value is moved out exactly once.
        let raw_value = unsafe { self.ptr.add(self.len * self.layout.size()).as_ptr() } as *mut T;
        Some(unsafe { raw_value.read() })
    }

    /// Removes the element at `index`, moving the current last element into
    /// its place, and returns the removed value.
    ///
    /// Does not preserve element order.
    ///
    /// # Panics
    ///
    /// Panics if `index >= len()`, or if `T` is not the type this vector was
    /// created for.
    pub fn swap_remove<T: Component + 'static>(&mut self, index: usize) -> T {
        assert!(index < self.len);
        assert_eq!(self.type_id, TypeId::of::<T>());

        self.len -= 1;

        // SAFETY[UNSAFE-008]: the assertion above proved `index < self.len`
        // (before the decrement), so the offset stays inside the allocation
        // and addresses an initialized slot, matching the layout that offset
        // was computed with, so reading a `T` out of it is sound.
        let ptr_to_remove = unsafe { self.ptr.add(index * self.layout.size()).as_ptr() as *mut T };
        let value_to_remove = unsafe { ptr_to_remove.read() };

        if self.len != index {
            // SAFETY[UNSAFE-009]: `self.len` (after the decrement above) is
            // the index of the current last initialized slot and differs
            // from `index`, so it stays inside the allocation and addresses a
            // live, initialized `T` distinct from the slot just vacated at
            // `index`; reading it here and writing it into `ptr_to_remove`
            // moves it exactly once, leaving no slot inside the shortened
            // `0..self.len` prefix uninitialized.
            let raw_data_to_swap =
                unsafe { self.ptr.add(self.len * self.layout.size()).as_ptr() as *mut T };
            let value_to_swap = unsafe { raw_data_to_swap.read() };
            unsafe { ptr_to_remove.write(value_to_swap) };
        }

        value_to_remove
    }

    /// Returns a shared reference to the element at `index`, or `None` if
    /// `index` is out of bounds.
    ///
    /// # Panics
    ///
    /// Panics if `T` is not the type this vector was created for.
    pub fn get<T: Component + 'static>(&self, index: usize) -> Option<&T> {
        assert_eq!(self.type_id, TypeId::of::<T>());

        if index >= self.len {
            return None;
        }

        // SAFETY[UNSAFE-010]: `index < self.len <= self.capacity`, so the
        // offset stays inside the allocation and addresses an initialized
        // slot whose type matches the layout it was computed with, so the
        // resulting reference is valid for reads and points to a live `T`
        // for as long as `&self` is borrowed.
        let raw_data = unsafe { self.ptr.add(index * self.layout.size()).as_ptr() as *const T };
        Some(unsafe { &*raw_data })
    }

    /// Returns an exclusive reference to the element at `index`, or `None`
    /// if `index` is out of bounds.
    ///
    /// # Panics
    ///
    /// Panics if `T` is not the type this vector was created for.
    pub fn get_mut<T: Component + 'static>(&mut self, index: usize) -> Option<&mut T> {
        assert_eq!(self.type_id, TypeId::of::<T>());

        if index >= self.len {
            return None;
        }

        // SAFETY[UNSAFE-011]: `index < self.len <= self.capacity`, so the
        // offset stays inside the allocation and addresses an initialized
        // slot whose type matches the layout it was computed with. The
        // exclusive borrow of `self` for the lifetime of the returned
        // reference prevents any other access to this slot while it is
        // outstanding, so the resulting `&mut T` is valid and unaliased.
        let raw_data = unsafe { self.ptr.add(index * self.layout.size()).as_ptr() } as *mut T;
        Some(unsafe { &mut *raw_data })
    }

    /// Drops the value at `ptr` as a `T`.
    ///
    /// Used as the vector's `drop_fn` so it can drop its remaining elements
    /// without knowing `T` at the call site.
    ///
    /// # Safety
    ///
    /// `ptr` must point to a live, initialized `T` that the caller will not
    /// read, write, or drop again afterwards.
    unsafe fn drop_value<T: Component + 'static>(ptr: *mut u8) {
        // SAFETY[UNSAFE-012]: guaranteed by this function's own safety
        // contract, documented above.
        unsafe {
            ptr.cast::<T>().drop_in_place();
        }
    }
}
