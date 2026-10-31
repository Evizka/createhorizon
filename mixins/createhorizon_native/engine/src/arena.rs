use alloc::vec::Vec;
use core::convert::TryFrom;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;

/// Stable typed handle to an arena slot. Removing a value invalidates every
/// handle to its previous generation before the slot can be reused.
pub struct Handle<T> {
    index: u32,
    generation: u32,
    marker: PhantomData<fn() -> T>,
}

impl<T> Handle<T> {
    const fn new(index: u32, generation: u32) -> Self {
        Self {
            index,
            generation,
            marker: PhantomData,
        }
    }

    #[must_use]
    pub const fn index(self) -> u32 {
        self.index
    }

    #[must_use]
    pub const fn generation(self) -> u32 {
        self.generation
    }
}

impl<T> Copy for Handle<T> {}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.generation == other.generation
    }
}

impl<T> Eq for Handle<T> {}

impl<T> PartialOrd for Handle<T> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Handle<T> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        (self.index, self.generation).cmp(&(other.index, other.generation))
    }
}

impl<T> Hash for Handle<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        self.generation.hash(state);
    }
}

impl<T> fmt::Debug for Handle<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Handle")
            .field("index", &self.index)
            .field("generation", &self.generation)
            .finish()
    }
}

struct Slot<T> {
    generation: u32,
    value: Option<T>,
}

/// Data-oriented storage with deterministic slot-order iteration and
/// generation-checked references instead of owning object pointers.
pub struct Arena<T> {
    slots: Vec<Slot<T>>,
    free: Vec<u32>,
    len: usize,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            len: 0,
        }
    }
}

impl<T> Arena<T> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            slots: Vec::with_capacity(capacity),
            free: Vec::new(),
            len: 0,
        }
    }

    /// Inserts a value, returning its handle. `Err(value)` means the arena
    /// exhausted the `u32` slot index space.
    pub fn insert(&mut self, value: T) -> Result<Handle<T>, T> {
        if let Some(index) = self.free.pop() {
            let slot_index = usize::try_from(index).expect("u32 slot index fits usize");
            let slot = &mut self.slots[slot_index];
            debug_assert!(slot.value.is_none());
            slot.value = Some(value);
            self.len += 1;
            return Ok(Handle::new(index, slot.generation));
        }

        let Ok(index) = u32::try_from(self.slots.len()) else {
            return Err(value);
        };
        self.slots.push(Slot {
            generation: 0,
            value: Some(value),
        });
        self.len += 1;
        Ok(Handle::new(index, 0))
    }

    #[must_use]
    pub fn get(&self, handle: Handle<T>) -> Option<&T> {
        let index = usize::try_from(handle.index).ok()?;
        let slot = self.slots.get(index)?;
        if slot.generation == handle.generation {
            slot.value.as_ref()
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, handle: Handle<T>) -> Option<&mut T> {
        let index = usize::try_from(handle.index).ok()?;
        let slot = self.slots.get_mut(index)?;
        if slot.generation != handle.generation {
            return None;
        }
        slot.value.as_mut()
    }

    /// Removes a live value and invalidates its handle. A slot at generation
    /// `u32::MAX` is retired instead of wrapping and reviving an old handle.
    pub fn remove(&mut self, handle: Handle<T>) -> Option<T> {
        let index = usize::try_from(handle.index).ok()?;
        let (value, reusable) = {
            let slot = self.slots.get_mut(index)?;
            if slot.generation != handle.generation {
                return None;
            }
            let value = slot.value.take()?;
            if let Some(next_generation) = slot.generation.checked_add(1) {
                slot.generation = next_generation;
                (value, true)
            } else {
                (value, false)
            }
        };
        self.len -= 1;
        if reusable {
            self.free.push(handle.index);
        }
        Some(value)
    }

    pub fn iter(&self) -> impl Iterator<Item = (Handle<T>, &T)> + '_ {
        self.slots.iter().enumerate().filter_map(|(index, slot)| {
            let value = slot.value.as_ref()?;
            let index = u32::try_from(index).ok()?;
            Some((Handle::new(index, slot.generation), value))
        })
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Handle<T>, &mut T)> + '_ {
        self.slots.iter_mut().enumerate().filter_map(|(index, slot)| {
            let value = slot.value.as_mut()?;
            let index = u32::try_from(index).ok()?;
            Some((Handle::new(index, slot.generation), value))
        })
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[must_use]
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::Arena;

    #[test]
    fn reused_slots_reject_stale_generations() {
        let mut arena = Arena::new();
        let old = arena.insert(10_u32).unwrap();
        assert_eq!(arena.remove(old), Some(10));
        assert_eq!(arena.get(old), None);

        let current = arena.insert(20).unwrap();
        assert_eq!(old.index(), current.index());
        assert_ne!(old.generation(), current.generation());
        assert_eq!(arena.get(old), None);
        assert_eq!(arena.get(current), Some(&20));
        assert_eq!(arena.len(), 1);
    }

    #[test]
    fn iteration_is_in_slot_order_and_mutation_is_checked() {
        let mut arena = Arena::new();
        let first = arena.insert(1_u8).unwrap();
        let second = arena.insert(2_u8).unwrap();
        for (_, value) in arena.iter_mut() {
            *value *= 2;
        }
        let entries: Vec<_> = arena.iter().map(|(handle, value)| (handle.index(), *value)).collect();
        assert_eq!(entries, [(first.index(), 2), (second.index(), 4)]);
    }
}
