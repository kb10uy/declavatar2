use std::collections::BTreeMap;

/// Represents an entry of `ValueSet`.
pub trait MaybeZeroableEntry: Sized + Clone {
    /// Key type in `ValueSet`.
    type Key: Clone + Ord;

    /// Returns the key of this entry.
    fn key(&self) -> Self::Key;

    /// Returns the zeroed version of this entry, if it is zeroable.
    fn zeroed(&self) -> Option<Self>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueSet<E: MaybeZeroableEntry> {
    entries: BTreeMap<E::Key, E>,
}

impl<E: MaybeZeroableEntry> Default for ValueSet<E> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }
}

impl<E: MaybeZeroableEntry, I: IntoIterator<Item = E>> From<I> for ValueSet<E> {
    fn from(iter: I) -> Self {
        let entries = iter.into_iter().map(|e| (e.key(), e)).collect();
        Self { entries }
    }
}

impl<E: MaybeZeroableEntry> ValueSet<E> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: E) -> Option<E> {
        self.entries.insert(entry.key(), entry)
    }

    pub fn entries(&self) -> impl Iterator<Item = (&E::Key, &E)> {
        self.entries.iter()
    }

    /// Unions the entries from `others` into this set, using zeroed values for zeroable entries.
    /// Existing entries won't be replaced with zeroed values.
    /// If non-zeroable entries are found in `others`, they are returned in Err value.
    pub fn union_fill_as_zero<'a>(
        &'a mut self,
        others: impl IntoIterator<Item = &'a ValueSet<E>>,
    ) -> Result<(), Vec<E::Key>> {
        let mut non_zeroables = Vec::new();
        let mut zeroed_entries = Vec::new();
        for (other_key, other_entry) in others.into_iter().flat_map(|vs| vs.entries()) {
            match other_entry.zeroed() {
                Some(zeroed) => zeroed_entries.push(zeroed),
                None => non_zeroables.push(other_key.clone()),
            }
        }

        if !non_zeroables.is_empty() {
            return Err(non_zeroables);
        }

        for zeroed_entry in zeroed_entries {
            self.entries
                .entry(zeroed_entry.key())
                .or_insert(zeroed_entry);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use either::Either;
    use rstest::*;

    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct TestEntry(&'static str, Either<usize, ()>);

    impl TestEntry {
        pub fn zeroable(key: &'static str, value: usize) -> TestEntry {
            TestEntry(key, Either::Left(value))
        }

        pub fn non_zeroable(key: &'static str) -> TestEntry {
            TestEntry(key, Either::Right(()))
        }
    }

    impl MaybeZeroableEntry for TestEntry {
        type Key = &'static str;

        fn key(&self) -> Self::Key {
            self.0
        }

        fn zeroed(&self) -> Option<Self> {
            self.1.left().map(|_| TestEntry(self.0, Either::Left(0)))
        }
    }

    #[rstest]
    fn zeroed_sets_can_be_merged() {
        let mut default_set = ValueSet::from([
            TestEntry::zeroable("key1", 0),
            TestEntry::zeroable("key2", 10),
        ]);
        let other1 = ValueSet::from([
            TestEntry::zeroable("key1", 10),
            TestEntry::zeroable("key3", 10),
        ]);
        let other2 = ValueSet::from([
            TestEntry::zeroable("key2", 30),
            TestEntry::zeroable("key4", 100),
        ]);

        let merge_result = default_set.union_fill_as_zero(&[other1, other2]);
        assert!(merge_result.is_ok());
        assert_eq!(
            default_set,
            ValueSet::from([
                TestEntry::zeroable("key1", 0),
                TestEntry::zeroable("key2", 10),
                TestEntry::zeroable("key3", 0),
                TestEntry::zeroable("key4", 0),
            ]),
        );
    }

    #[rstest]
    fn non_zeroable_entries_will_fail() {
        let mut default_set = ValueSet::from([
            TestEntry::zeroable("key1", 0),
            TestEntry::zeroable("key2", 10),
        ]);
        let other1 = ValueSet::from([
            TestEntry::zeroable("key1", 10),
            TestEntry::non_zeroable("key3"),
        ]);
        let other2 = ValueSet::from([
            TestEntry::zeroable("key2", 30),
            TestEntry::non_zeroable("key4"),
        ]);

        let merge_result = default_set.union_fill_as_zero(&[other1, other2]);
        assert_eq!(merge_result, Err(vec!["key3", "key4"]));
        assert_eq!(
            default_set,
            ValueSet::from([
                TestEntry::zeroable("key1", 0),
                TestEntry::zeroable("key2", 10),
            ])
        );
    }
}
