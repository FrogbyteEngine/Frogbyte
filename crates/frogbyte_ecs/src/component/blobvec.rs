//! Contiguous, type-erased storage for components of a single type.

use std::{
    alloc::{Layout, alloc, dealloc, handle_alloc_error, realloc},
    any::TypeId,
    ptr::{self, NonNull},
};

use crate::component::Component;

/// Stores components of one concrete type in a contiguous allocation.
///
/// # Invariants
///
/// - `len <= capacity`.
/// - Elements in `0..len` are initialized.
/// - `layout`, `type_id`, and `drop_fn` describe the type used at creation.
/// - For non-ZST components, `capacity > 0` means `ptr` references the
///   corresponding allocation.
/// - For ZST components, `capacity == usize::MAX` and `ptr` is a
///   correctly aligned dangling pointer; no allocation is performed.
pub struct BlobVec {
    ptr: NonNull<u8>,
    capacity: usize,
    len: usize,
    layout: Layout,
    type_id: TypeId,
    drop_fn: unsafe fn(*mut u8),
}

struct DropGuard {
    ptr: NonNull<u8>,
    next: usize,
    len: usize,
    capacity: usize,
    layout: Layout,
    drop_fn: unsafe fn(*mut u8),
}

impl DropGuard {
    fn drop_next(&mut self) {
        let index = self.next;

        // Advance before dropping so unwinding resumes with the next element
        // instead of attempting to drop this one twice.
        self.next += 1;

        // SAFETY: [UNSAFE-001] `index` was inside the initialized range before
        // `next` was advanced, and `ptr` is correctly aligned for the stored
        // component type. For ZSTs the offset is zero and the aligned dangling
        // pointer remains valid for the type-erased destructor.
        let data = unsafe { self.ptr.add(index * self.layout.size()).as_ptr() };

        // SAFETY: [UNSAFE-002] `drop_fn` was created for the component type
        // described by `layout`, and `data` identifies one initialized value
        // that has not previously been dropped or moved out.
        unsafe {
            (self.drop_fn)(data);
        }
    }
}

impl Drop for DropGuard {
    fn drop(&mut self) {
        while self.next < self.len {
            self.drop_next();
        }

        if self.capacity != 0 && self.layout.size() != 0 {
            let (total_layout, _) = self
                .layout
                .repeat(self.capacity)
                .expect("BlobVec allocation layout must remain valid");

            // SAFETY: [UNSAFE-003] Non-ZST storage was allocated using this
            // repeated layout, every initialized element has now been dropped,
            // and `ptr` has not been deallocated yet.
            unsafe {
                dealloc(self.ptr.as_ptr(), total_layout);
            }
        }
    }
}

impl Drop for BlobVec {
    fn drop(&mut self) {
        let mut guard = DropGuard {
            ptr: self.ptr,
            next: 0,
            len: self.len,
            capacity: self.capacity,
            layout: self.layout,
            drop_fn: self.drop_fn,
        };

        while guard.next < guard.len {
            guard.drop_next();
        }
    }
}

impl BlobVec {
    /// Creates empty storage for components of type `T`.
    pub fn new<T: Component + 'static>() -> Self {
        Self {
            ptr: NonNull::<T>::dangling().cast::<u8>(),
            capacity: if size_of::<T>() == 0 { usize::MAX } else { 0 },
            len: 0,
            layout: Layout::new::<T>(),
            type_id: TypeId::of::<T>(),
            drop_fn: Self::drop_value::<T>,
        }
    }

    /// Doubles the backing capacity, starting at one element.
    fn grow(&mut self) {
        assert_ne!(self.layout.size(), 0, "BlobVec ZST capacity overflow");

        let new_capacity = if self.capacity == 0 {
            1
        } else {
            self.capacity
                .checked_mul(2)
                .expect("Error: BlobVec max size capacity reached.")
        };

        let (new_layout, _) = Layout::repeat(&self.layout, new_capacity)
            .expect("Error: BlobVec layout must remain valid");

        let new_ptr = if self.capacity == 0 {
            // SAFETY: [UNSAFE-004] `grow` is only used for non-ZST storage, so
            // `new_layout` has non-zero size and is valid for allocation.
            unsafe { alloc(new_layout) }
        } else {
            let (old_layout, _) = self
                .layout
                .repeat(self.capacity)
                .expect("Error: BlobVec layout must remain valid");
            let old_ptr = self.ptr.as_ptr();
            // SAFETY: [UNSAFE-005] `old_ptr` was allocated with `old_layout`,
            // and `new_layout` preserves the element alignment while growing.
            unsafe { realloc(old_ptr, old_layout, new_layout.size()) }
        };

        self.ptr = match NonNull::new(new_ptr) {
            Some(p) => p,
            None => handle_alloc_error(new_layout),
        };

        self.capacity = new_capacity;
    }

    /// Appends a component to the storage.
    ///
    /// # Panics
    ///
    /// Panics if `T` differs from the type used to create this storage.
    pub fn push<T: Component + 'static>(&mut self, value: T) {
        assert_eq!(self.type_id, TypeId::of::<T>());

        if self.len >= self.capacity {
            self.grow();
        }

        // SAFETY: [UNSAFE-006] For non-ZSTs, `len < capacity` identifies an
        // uninitialized slot inside the allocation. For ZSTs, `ptr` is a correctly
        // aligned dangling pointer and the offset is zero. The type check guarantees
        // the resulting pointer has the layout of `T`.
        let ptr = (unsafe { self.ptr.add(self.layout.size() * self.len).as_ptr() }) as *mut T;
        // SAFETY: [UNSAFE-007] `ptr` is correctly aligned for `T` and represents the
        // next logical storage slot. Writing transfers ownership of `value` into the
        // initialized prefix.
        unsafe { ptr.write(value) };

        self.len += 1;
    }

    /// Removes and returns the last component, or `None` when empty.
    ///
    /// # Panics
    ///
    /// Panics if `T` differs from the stored component type.
    pub fn pop<T: Component + 'static>(&mut self) -> Option<T> {
        assert_eq!(self.type_id, TypeId::of::<T>());

        if self.len == 0 {
            return None;
        }

        self.len -= 1;

        // SAFETY: [UNSAFE-008] The decremented length identifies the previous last
        // initialized value, and the type check guarantees the pointer has type `T`
        let raw_value = unsafe { self.ptr.add(self.len * self.layout.size()).as_ptr() } as *mut T;
        // SAFETY: [UNSAFE-009] The slot has already been removed from the initialized
        // prefix, so moving its value out cannot cause BlobVec to drop it again.
        Some(unsafe { raw_value.read() })
    }

    /// Removes the component at `index`, replacing it with the last component.
    ///
    /// Element order is not preserved.
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds or `T` differs from the stored type.
    pub fn swap_remove<T: Component + 'static>(&mut self, index: usize) -> T {
        assert!(index < self.len);
        assert_eq!(self.type_id, TypeId::of::<T>());

        self.len -= 1;

        // SAFETY: [UNSAFE-010] `index` was checked against the previous length, so it
        // identifies an initialized value of the type verified above.
        let ptr_to_remove = unsafe { self.ptr.add(index * self.layout.size()).as_ptr() as *mut T };
        // SAFETY: [UNSAFE-011] The value is moved out of an initialized slot that will
        // either remain vacant or receive the previous last value below.
        let value_to_remove = unsafe { ptr_to_remove.read() };

        if self.len != index {
            // SAFETY: [UNSAFE-012] The new length identifies the previous last initialized
            // value. When this branch runs, that slot is distinct from `index`.
            let raw_data_to_swap =
                unsafe { self.ptr.add(self.len * self.layout.size()).as_ptr() as *mut T };
            // SAFETY: [UNSAFE-013] The previous last slot has left the initialized prefix,
            // so moving its value out cannot cause it to be dropped twice.
            let value_to_swap = unsafe { raw_data_to_swap.read() };
            // SAFETY: [UNSAFE-014] `ptr_to_remove` is the vacant in-bounds slot created by
            // moving out the removed value, so writing the previous last value restores
            // the initialized `0..len` prefix.
            unsafe { ptr_to_remove.write(value_to_swap) };
        }

        value_to_remove
    }

    /// Structurally removes the value at `index` without destroying it.
    ///
    /// The final initialized value is swapped into `index` when necessary and
    /// the logical length is reduced. The returned pointer identifies the
    /// removed initialized value, now outside `0..len`.
    ///
    /// The caller is responsible for eventually destroying that value exactly
    /// once with the destructor associated with this BlobVec before its backing
    /// allocation is invalidated.
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds.
    pub(crate) fn raw_swap_remove(&mut self, index: usize) -> *mut u8 {
        assert!(index < self.len);

        // SAFETY: [UNSAFE-020] `index < len`, so for non-ZST storage this
        // offset identifies an initialized slot inside the allocation. For
        // ZSTs the offset is zero and the aligned dangling pointer remains
        // valid for zero-sized pointer operations.
        let ptr_to_remove = unsafe { self.ptr.add(index * self.layout.size()).as_ptr() };

        // SAFETY: [UNSAFE-021] `len > 0` follows from the bounds check, so
        // `len - 1` identifies the final initialized component slot. For ZSTs
        // the zero-sized offset preserves the correctly aligned dangling
        // pointer.
        let raw_data_to_swap =
            unsafe { self.ptr.add((self.len - 1) * self.layout.size()).as_ptr() };

        if self.len - 1 != index {
            // SAFETY: [UNSAFE-022] For non-ZST storage, `index` and `len - 1`
            // identify distinct initialized regions of `layout.size()` bytes
            // within the same allocation, so they do not overlap. For ZSTs the
            // byte count is zero and no memory is accessed.
            unsafe {
                ptr::swap_nonoverlapping(raw_data_to_swap, ptr_to_remove, self.layout.size())
            };
        }

        self.len -= 1;

        raw_data_to_swap
    }

    /// Returns the component at `index`, or `None` when out of bounds.
    ///
    /// # Panics
    ///
    /// Panics if `T` differs from the stored component type.
    pub fn get<T: Component + 'static>(&self, index: usize) -> Option<&T> {
        assert_eq!(self.type_id, TypeId::of::<T>());

        if index >= self.len {
            return None;
        }

        // SAFETY: [UNSAFE-015] `index < len`, so the slot is initialized, and the
        // type check guarantees that the stored value is a `T`.
        let raw_data = unsafe { self.ptr.add(index * self.layout.size()).as_ptr() as *const T };
        // SAFETY: [UNSAFE-016] `raw_data` is non-null, correctly aligned and points to
        // a live initialized `T`. The shared borrow of `self` prevents mutable access
        // through this BlobVec for the lifetime of the returned reference.
        Some(unsafe { &*raw_data })
    }

    /// Returns mutable access to the component at `index`.
    ///
    /// Returns `None` when `index` is out of bounds.
    ///
    /// # Panics
    ///
    /// Panics if `T` differs from the stored component type.
    pub fn get_mut<T: Component + 'static>(&mut self, index: usize) -> Option<&mut T> {
        assert_eq!(self.type_id, TypeId::of::<T>());

        if index >= self.len {
            return None;
        }

        // SAFETY: [UNSAFE-017] `index < len`, so the slot is initialized, and the
        // type check guarantees that the stored value is a `T`.
        let raw_data = unsafe { self.ptr.add(index * self.layout.size()).as_ptr() } as *mut T;
        // SAFETY: [UNSAFE-018] `raw_data` is non-null, correctly aligned and points to
        // a live initialized `T`. Borrowing `self` mutably provides exclusive access
        // to the returned component for the reference lifetime.
        Some(unsafe { &mut *raw_data })
    }

    /// Returns the concrete component type stored by this BlobVec.
    pub fn type_id(&self) -> TypeId {
        self.type_id
    }

    /// Returns the type-erased destructor for values stored by this BlobVec.
    ///
    /// Calling the returned function is unsafe: its argument must identify one
    /// live, properly aligned and initialized value of this BlobVec's component
    /// type that must be destroyed exactly once.
    pub(crate) fn drop_fn(&self) -> unsafe fn(*mut u8) {
        self.drop_fn
    }

    /// Drops the value at `ptr` as a `T`.
    ///
    /// # Safety
    ///
    /// `ptr` must point to a live, properly aligned and initialized `T`.
    unsafe fn drop_value<T: Component + 'static>(ptr: *mut u8) {
        // SAFETY: [UNSAFE-019] The caller guarantees that `ptr` identifies a live,
        // correctly aligned and initialized value of `T` that must be dropped exactly
        // once.
        unsafe {
            ptr.cast::<T>().drop_in_place();
        }
    }
}
