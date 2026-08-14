//! Contiguous, type-erased storage for components of a single type.

use std::{
    alloc::{Layout, alloc, dealloc, handle_alloc_error, realloc},
    any::TypeId,
    ptr::NonNull,
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

        // Advance first so this element is never dropped twice if its
        // destructor panics.
        self.next += 1;

        // SAFETY: `index` belonged to the initialized prefix before `next`
        // was advanced, and offsets follow the stored component layout.
        let data = unsafe { self.ptr.add(index * self.layout.size()).as_ptr() };

        // SAFETY: `drop_fn` matches the stored component type and `data`
        // identifies the initialized value being removed from the prefix.
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

            // SAFETY: Non-ZST storage was allocated with this repeated layout
            // and all initialized elements have now been dropped.
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

        // SAFETY: [UNSAFE-006] After growth, `len < capacity`, and the type
        // check guarantees this slot has the layout of `T`.
        let ptr = (unsafe { self.ptr.add(self.layout.size() * self.len).as_ptr() }) as *mut T;
        // SAFETY: [UNSAFE-007] `ptr` addresses the next uninitialized slot.
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

        // SAFETY: [UNSAFE-008] The decremented length identifies the previous
        // last initialized slot, whose type was checked above.
        let raw_value = unsafe { self.ptr.add(self.len * self.layout.size()).as_ptr() } as *mut T;
        // SAFETY: [UNSAFE-009] The value is moved out after its slot has been
        // removed from the initialized prefix.
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

        // SAFETY: [UNSAFE-010] `index` was checked against the previous length
        // and therefore identifies an initialized `T`.
        let ptr_to_remove = unsafe { self.ptr.add(index * self.layout.size()).as_ptr() as *mut T };
        // SAFETY: [UNSAFE-011] The value is moved out of the initialized slot.
        let value_to_remove = unsafe { ptr_to_remove.read() };

        if self.len != index {
            // SAFETY: [UNSAFE-012] The new length identifies the previous last
            // initialized slot, distinct from the removed slot.
            let raw_data_to_swap =
                unsafe { self.ptr.add(self.len * self.layout.size()).as_ptr() as *mut T };
            // SAFETY: [UNSAFE-013] The previous last value is moved out after
            // its slot leaves the initialized prefix.
            let value_to_swap = unsafe { raw_data_to_swap.read() };
            // SAFETY: [UNSAFE-014] `ptr_to_remove` is now a vacant in-bounds
            // slot and receives the moved last value.
            unsafe { ptr_to_remove.write(value_to_swap) };
        }

        value_to_remove
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

        // SAFETY: [UNSAFE-015] `index < len`, and the type check guarantees
        // that the initialized slot contains a `T`.
        let raw_data = unsafe { self.ptr.add(index * self.layout.size()).as_ptr() as *const T };
        // SAFETY: [UNSAFE-016] `raw_data` points to a live initialized `T`
        // covered by the shared borrow of `self`.
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

        // SAFETY: [UNSAFE-017] `index < len`, and the type check guarantees
        // that the initialized slot contains a `T`.
        let raw_data = unsafe { self.ptr.add(index * self.layout.size()).as_ptr() } as *mut T;
        // SAFETY: [UNSAFE-018] The mutable borrow of `self` provides exclusive
        // access to the referenced component.
        Some(unsafe { &mut *raw_data })
    }

    /// Drops the value at `ptr` as a `T`.
    ///
    /// # Safety
    ///
    /// `ptr` must point to a live, properly aligned and initialized `T`.
    unsafe fn drop_value<T: Component + 'static>(ptr: *mut u8) {
        // SAFETY: [UNSAFE-019] Guaranteed by the caller of this type-erased
        // destructor.
        unsafe {
            ptr.cast::<T>().drop_in_place();
        }
    }
}
