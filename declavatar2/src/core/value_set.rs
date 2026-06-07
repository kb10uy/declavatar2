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
    /// If any non-zeroable entry is found only in `others`, they are returned in Err value.
    pub fn union_fill_as_zero<'a>(
        &'a mut self,
        others: impl IntoIterator<Item = &'a ValueSet<E>>,
    ) -> Result<(), Vec<E::Key>> {
        let mut non_zeroables = Vec::new();
        let mut zeroed_entries = Vec::new();
        for (other_key, other_entry) in others.into_iter().flat_map(|vs| vs.entries()) {
            match other_entry.zeroed() {
                Some(zeroed) => zeroed_entries.push(zeroed),
                None if self.entries.contains_key(other_key) => (),
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

    /// Unions the entries from `defaults` into this set, using original entries.
    /// Existing entries are won't be replaced.
    /// If "orphan" entries exist in this set, they are returned in Err value.
    pub fn union_from_defaults(&mut self, defaults: &ValueSet<E>) -> Result<(), Vec<E::Key>> {
        let non_zeroable_orphans: Vec<_> = self
            .entries
            .iter()
            .filter(|(key, _)| !defaults.entries.contains_key(key))
            .map(|(key, _)| key.clone())
            .collect();
        if !non_zeroable_orphans.is_empty() {
            return Err(non_zeroable_orphans);
        }

        for (key, entry) in defaults.entries() {
            if self.entries.contains_key(key) {
                continue;
            }
            self.entries.insert(key.clone(), entry.clone());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::slice::from_ref;

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

    #[rstest]
    fn non_zeroable_in_default_set_can_be_merged() {
        let mut default_set = ValueSet::from([
            TestEntry::non_zeroable("key1"),
            TestEntry::non_zeroable("key2"),
        ]);
        let other1 = ValueSet::from([TestEntry::zeroable("key3", 10)]);
        let other2 = ValueSet::from([TestEntry::zeroable("key4", 100)]);

        let merge_result = default_set.union_fill_as_zero(&[other1, other2]);
        assert!(merge_result.is_ok());
        assert_eq!(
            default_set,
            ValueSet::from([
                TestEntry::non_zeroable("key1"),
                TestEntry::non_zeroable("key2"),
                TestEntry::zeroable("key3", 0),
                TestEntry::zeroable("key4", 0),
            ]),
        );
    }

    #[rstest]
    fn non_zeroable_both_in_default_set_and_others_can_be_merged() {
        let mut default_set = ValueSet::from([
            TestEntry::non_zeroable("key1"),
            TestEntry::non_zeroable("key2"),
            TestEntry::non_zeroable("key3"),
            TestEntry::non_zeroable("key4"),
        ]);
        let other1 = ValueSet::from([
            TestEntry::non_zeroable("key3"),
            TestEntry::zeroable("key5", 100),
        ]);
        let other2 = ValueSet::from([
            TestEntry::non_zeroable("key4"),
            TestEntry::zeroable("key6", 100),
        ]);

        let merge_result = default_set.union_fill_as_zero(&[other1, other2]);
        assert!(merge_result.is_ok());
        assert_eq!(
            default_set,
            ValueSet::from([
                TestEntry::non_zeroable("key1"),
                TestEntry::non_zeroable("key2"),
                TestEntry::non_zeroable("key3"),
                TestEntry::non_zeroable("key4"),
                TestEntry::zeroable("key5", 0),
                TestEntry::zeroable("key6", 0),
            ]),
        );
    }

    #[rstest]
    fn defaults_can_be_merged() {
        let mut defaults = ValueSet::from([
            TestEntry::zeroable("key1", 10),
            TestEntry::zeroable("key3", 100),
        ]);
        let mut set = ValueSet::from([
            TestEntry::zeroable("key1", 20),
            TestEntry::zeroable("key2", 50),
        ]);

        let default_merge_result = defaults.union_fill_as_zero(from_ref(&set));
        assert!(default_merge_result.is_ok());

        let merge_result = set.union_from_defaults(&defaults);
        assert!(merge_result.is_ok());
        assert_eq!(
            set,
            ValueSet::from([
                TestEntry::zeroable("key1", 20),
                TestEntry::zeroable("key2", 50),
                TestEntry::zeroable("key3", 100),
            ]),
        );
    }

    #[rstest]
    fn orphan_defaults_will_fail() {
        let defaults = ValueSet::from([
            TestEntry::zeroable("key1", 10),
            TestEntry::zeroable("key3", 100),
        ]);
        let mut set = ValueSet::from([
            TestEntry::zeroable("key1", 20),
            TestEntry::zeroable("key2", 50),
        ]);

        let merge_result = set.union_from_defaults(&defaults);
        assert_eq!(merge_result, Err(vec!["key2"]));
    }
}
