#![cfg_attr(not(test), no_std)]

extern crate alloc;

use core::{
    mem::{forget, replace},
    ops::IndexMut,
};

use alloc::{collections::VecDeque, vec::Vec};

/// A view into a single entry which may either be vacant or occupied.
///
/// This `enum` is constructed from the [`entry`] method on [`EntryExt`].
///
pub enum Entry<'a, C: ?Sized> {
    /// An occupied entry.
    Occupied(OccupiedEntry<'a, C>),

    /// A vacant entry.
    Vacant(VacantEntry<'a, C>),
}

pub struct OccupiedEntry<'a, C: ?Sized> {
    index: usize,
    container: &'a mut C,
}

pub struct VacantEntry<'a, C: ?Sized> {
    index: usize,
    container: &'a mut C,
}

impl<'a, C: 'a + Entriable> OccupiedEntry<'a, C> {
    /// Gets a reference to the index of the entry.
    pub fn key(&self) -> &usize {
        &self.index
    }

    /// Take the ownership of the index and value from the container.
    pub fn remove_entry(self) -> (usize, C::T) {
        debug_assert!(self.index < self.container.len());
        (self.index, self.remove())
    }

    /// Gets a reference to the value in the entry.
    pub fn get(&self) -> &C::T {
        debug_assert!(self.index < self.container.len());
        &self.container[self.index]
    }

    /// Gets a mutable reference to the value in the entry.
    ///
    /// If you need a reference to the `OccupiedEntry` which may outlive the
    /// destruction of the `Entry` value, see [`into_mut`].
    ///
    /// [`into_mut`]: #method.into_mut
    pub fn get_mut(&mut self) -> &mut C::T {
        debug_assert!(self.index < self.container.len());
        &mut self.container[self.index]
    }

    /// Converts the OccupiedEntry into a mutable reference to the value in the entry
    /// with a lifetime bound to the map itself.
    ///
    /// If you need multiple references to the `OccupiedEntry`, see [`get_mut`].
    ///
    /// [`get_mut`]: #method.get_mut
    pub fn into_mut(self) -> &'a mut C::T {
        debug_assert!(self.index < self.container.len());
        &mut self.container[self.index]
    }

    /// Sets the value of the entry, and returns the entry’s old value.
    pub fn insert(&mut self, value: C::T) -> C::T {
        replace(self.get_mut(), value)
    }

    /// Takes the value out of the entry, and returns it. Keeps the allocated memory for reuse.
    pub fn remove(self) -> C::T {
        debug_assert!(self.index < self.container.len());
        self.container.remove(self.index)
    }

    /// Replaces the entry, returning the index and value.
    pub fn replace_entry(mut self, value: C::T) -> (usize, C::T) {
        (self.index, self.insert(value))
    }

    /// Provides shared access to the key and owned access to the value of the
    /// entry and allows to replace or remove it based on the value of the
    /// returned option.
    pub fn replace_entry_with<F, R>(mut self, f: F) -> (Entry<'a, C>, R)
    where
        F: FnOnce(&usize, C::T) -> (Option<C::T>, R),
    {
        struct RemoveDropHandler<'b, 'a, C: 'a + Entriable>(&'b mut OccupiedEntry<'a, C>);
        impl<'b, 'a, C: 'a + Entriable> Drop for RemoveDropHandler<'b, 'a, C> {
            fn drop(&mut self) {
                forget(self.0.container.remove(self.0.index));
            }
        }

        // get the pointer before we construct the handler to minimize the chance
        // of panic-drop-leaking
        let index = self.index;
        let ptr: *mut _ = self.get_mut();
        let handler = RemoveDropHandler(&mut self);
        // This is safe because we either write over it or forget it
        let v = unsafe { core::ptr::read(ptr) };

        match f(&index, v) {
            (None, r) => {
                // Go ahead and forgetfully remove the entry
                drop(handler);
                let entry = if self.container.len() <= self.index {
                    Entry::Vacant(VacantEntry {
                        index: self.index,
                        container: self.container,
                    })
                } else {
                    Entry::Occupied(self)
                };
                (entry, r)
            }
            (Some(v), val) => {
                // Don't remove the entry, just overwrite it forgetfully
                forget(handler);
                unsafe { core::ptr::write(ptr, v) };

                (Entry::Occupied(self), val)
            }
        }
    }
}

impl<'a, C: 'a + Entriable> VacantEntry<'a, C> {
    /// Gets a reference to the index of the entry.
    pub fn key(&self) -> &usize {
        &self.index
    }

    /// Take ownership of the key.
    pub fn into_key(self) -> usize {
        self.index
    }

    /// Set the value of the entry, and return a mutable reference to it.
    ///
    /// # Panics
    ///
    /// Panics if the entry's index is greater than container's length
    pub fn insert(self, v: C::T) -> &'a mut C::T {
        self.container.insert(self.index, v);
        &mut self.container[self.index]
    }
}

pub trait EntryExt {
    /// Get a key's entry for in-place manipulation.
    fn entry<'a>(&'a mut self, index: usize) -> Entry<'a, Self>;
}

pub trait Entriable: IndexMut<usize, Output = Self::T> {
    type T;

    /// Returns the number of elements in the container.
    fn len(&self) -> usize;

    /// Inserts an element at `index` within the container, shifting all elements
    /// with indices greater than or equal to `index` towards the back.
    ///
    /// Element at index 0 is the front.
    ///
    /// # Panics
    ///
    /// Panics if `index` is greater than container's length
    fn insert(&mut self, index: usize, value: Self::T);

    /// Removes and returns the element at `index` from the container, shifting
    /// all elements with indices greater than or equal to `index` towards the
    /// front.
    ///
    /// Element at index 0 is the front of the container.
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds.
    fn remove(&mut self, index: usize) -> Self::T;
}

impl<C: Entriable> EntryExt for C {
    fn entry<'a>(&'a mut self, index: usize) -> Entry<'a, Self> {
        if index >= self.len() {
            Entry::Vacant(VacantEntry {
                index,
                container: self,
            })
        } else {
            Entry::Occupied(OccupiedEntry {
                index,
                container: self,
            })
        }
    }
}

impl<T> Entriable for Vec<T> {
    type T = T;

    fn len(&self) -> usize {
        Vec::len(self)
    }
    fn insert(&mut self, index: usize, value: T) {
        Vec::insert(self, index, value)
    }
    fn remove(&mut self, index: usize) -> T {
        Vec::remove(self, index)
    }
}

impl<T> Entriable for VecDeque<T> {
    type T = T;

    fn len(&self) -> usize {
        VecDeque::len(self)
    }
    fn insert(&mut self, index: usize, value: T) {
        VecDeque::insert(self, index, value)
    }
    fn remove(&mut self, index: usize) -> T {
        VecDeque::remove(self, index).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use core::{cell::Cell, sync::atomic::AtomicUsize};
    use std::panic;

    use alloc::{sync::Arc, vec};

    use super::*;

    #[test]
    fn it_works() {
        let mut v = vec![69u64, 420, 0xDEADBEEF];

        match v.entry(1) {
            Entry::Vacant(_) => unreachable!(),
            Entry::Occupied(mut o) => {
                assert_eq!(*o.get(), 420);
                assert_eq!(*o.get_mut(), 420);

                o.replace_entry_with(|k, v| {
                    assert_eq!(*k, 1);
                    assert_eq!(v, 420);
                    (None, ())
                });
            }
        }

        assert_eq!(v, [69, 0xDEADBEEF])
    }

    #[test]
    fn drop_count() {
        struct DropCounter<'a>(&'a Cell<usize>);
        impl<'a> Drop for DropCounter<'a> {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1)
            }
        }

        let c = Cell::new(0);
        let mut v = vec![
            DropCounter(&c),
            DropCounter(&c),
            DropCounter(&c),
            DropCounter(&c),
        ];

        let entry = match v.entry(1) {
            Entry::Vacant(_) => unreachable!(),
            Entry::Occupied(o) => {
                assert_eq!(c.get(), 0);
                let (entry, ()) = o.replace_entry_with(|k, v| {
                    assert_eq!(*k, 1);
                    assert_eq!(c.get(), 0);
                    drop(v);
                    assert_eq!(c.get(), 1);
                    (None, ())
                });
                assert_eq!(c.get(), 1);
                entry
            }
        };

        match entry {
            Entry::Vacant(_) => unreachable!(),
            Entry::Occupied(o) => {
                assert_eq!(c.get(), 1);
                let (entry, ()) = o.replace_entry_with(|k, v| {
                    assert_eq!(*k, 1);
                    assert_eq!(c.get(), 1);
                    drop(v);
                    assert_eq!(c.get(), 2);
                    (Some(DropCounter(&c)), ())
                });
                assert_eq!(c.get(), 2);
                entry
            }
        };
        drop(v);
        assert_eq!(c.get(), 5);
    }
    #[test]
    fn panic_drop_count() {
        use core::sync::atomic::Ordering::Relaxed;
        struct DropCounter(Arc<AtomicUsize>);
        impl Drop for DropCounter {
            fn drop(&mut self) {
                self.0.fetch_add(1, Relaxed);
            }
        }
        let c = Arc::new(AtomicUsize::new(0));
        let mut v = vec![
            DropCounter(Arc::clone(&c)),
            DropCounter(Arc::clone(&c)),
            DropCounter(Arc::clone(&c)),
            DropCounter(Arc::clone(&c)),
        ];

        let c2 = Arc::clone(&c);
        let prev_hook = panic::take_hook();
        panic::set_hook(Box::new(|_| {}));
        panic::catch_unwind(move || match v.entry(1) {
            Entry::Vacant(_) => unreachable!(),
            Entry::Occupied(o) => {
                assert_eq!(c2.load(Relaxed), 0);
                o.replace_entry_with(|_k, _v| {
                    assert_eq!(c2.load(Relaxed), 0);
                    #[allow(unreachable_code)]
                    (None, panic!("oh no!"))
                });
            }
        })
        .unwrap_err();
        panic::set_hook(prev_hook);
        assert_eq!(c.load(Relaxed), 4);
    }
}
