//! Arena allocation for knowledge graphs and ASTs using
//! `generational_arena` for generation-checked indices that prevent
//! use-after-free style stale handles.

use generational_arena::{Arena, Index};

/// Graph/AST arena wrapper.
#[derive(Debug, Clone)]
pub struct GraphArena<T> {
    inner: Arena<T>,
}

impl<T> Default for GraphArena<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> GraphArena<T> {
    pub fn new() -> Self {
        Self {
            inner: Arena::new(),
        }
    }

    pub fn insert(&mut self, value: T) -> Index {
        self.inner.insert(value)
    }

    pub fn get(&self, index: Index) -> Option<&T> {
        self.inner.get(index)
    }

    pub fn get_mut(&mut self, index: Index) -> Option<&mut T> {
        self.inner.get_mut(index)
    }

    pub fn remove(&mut self, index: Index) -> Option<T> {
        self.inner.remove(index)
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_get_remove() {
        let mut a = GraphArena::new();
        let i = a.insert(42);
        assert_eq!(a.get(i), Some(&42));
        assert_eq!(a.remove(i), Some(42));
        assert!(a.get(i).is_none());
    }
}
