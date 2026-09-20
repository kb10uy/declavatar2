use std::{
    cmp::Ordering,
    collections::BTreeMap,
    fmt::{self, Debug},
    hash::{Hash, Hasher},
    marker::PhantomData,
};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::core::resolution::{SourceLocation, Unresolved};

/// Marker type for a kind of reference whose existence can only be checked by the client.
pub trait ExternKind: 'static {
    type Value: Debug + Clone + Ord;
}

/// A reference to an entry of `ExternTable`.
/// It can only be obtained by `ExternTable::intern`, so every `Extern` is backed by a table entry.
/// The client verifies the referenced entry and uses this index afterwards.
pub struct Extern<K: ExternKind> {
    index: u32,
    _kind: PhantomData<fn() -> K>,
}

impl<K: ExternKind> Extern<K> {
    pub fn index(&self) -> u32 {
        self.index
    }
}

impl<K: ExternKind> Debug for Extern<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Extern").field(&self.index).finish()
    }
}

impl<K: ExternKind> Clone for Extern<K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K: ExternKind> Copy for Extern<K> {}

impl<K: ExternKind> PartialEq for Extern<K> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl<K: ExternKind> Eq for Extern<K> {}

impl<K: ExternKind> PartialOrd for Extern<K> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<K: ExternKind> Ord for Extern<K> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.index.cmp(&other.index)
    }
}

impl<K: ExternKind> Hash for Extern<K> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index.hash(state);
    }
}

impl<K: ExternKind> Serialize for Extern<K> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u32(self.index)
    }
}

impl<'de, K: ExternKind> Deserialize<'de> for Extern<K> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        u32::deserialize(deserializer).map(|index| Self { index, _kind: PhantomData })
    }
}

/// An entry of `ExternTable`: the value the client should look up, and every place in the script that referenced it.
#[derive(Debug, Clone, Serialize)]
#[serde(bound(serialize = "K::Value: Serialize"))]
pub struct ExternEntry<K: ExternKind> {
    pub value: K::Value,
    pub referenced_at: Vec<SourceLocation>,
}

impl<K: ExternKind> PartialEq for ExternEntry<K> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value && self.referenced_at == other.referenced_at
    }
}

impl<K: ExternKind> Eq for ExternEntry<K> {}

/// Deduplicated table of external references of one kind.
/// Serialized as the plain list of entries; `Extern` values index into it.
#[derive(Debug, Clone, Serialize)]
#[serde(transparent, bound(serialize = "K::Value: Serialize"))]
pub struct ExternTable<K: ExternKind> {
    entries: Vec<ExternEntry<K>>,
    #[serde(skip)]
    lookup: BTreeMap<K::Value, u32>,
}

impl<K: ExternKind> Default for ExternTable<K> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            lookup: BTreeMap::new(),
        }
    }
}

impl<K: ExternKind> PartialEq for ExternTable<K> {
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}

impl<K: ExternKind> Eq for ExternTable<K> {}

impl<K: ExternKind> ExternTable<K> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn intern(&mut self, unresolved: Unresolved<K::Value>) -> Extern<K> {
        let Unresolved { value, at } = unresolved;
        let index = *self.lookup.entry(value.clone()).or_insert_with(|| {
            self.entries.push(ExternEntry {
                value,
                referenced_at: Vec::new(),
            });
            (self.entries.len() - 1) as u32
        });
        if let Some(at) = at {
            let referenced_at = &mut self.entries[index as usize].referenced_at;
            if !referenced_at.contains(&at) {
                referenced_at.push(at);
            }
        }
        Extern { index, _kind: PhantomData }
    }

    pub fn get(&self, reference: Extern<K>) -> &ExternEntry<K> {
        &self.entries[reference.index as usize]
    }

    pub fn entries(&self) -> &[ExternEntry<K>] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use rstest::*;

    use super::*;

    enum TestKind {}

    impl ExternKind for TestKind {
        type Value = String;
    }

    fn at(line: u32) -> SourceLocation {
        SourceLocation {
            chunk: "avatar.lua".into(),
            line,
        }
    }

    #[rstest]
    fn intern_deduplicates_and_records_locations() {
        let mut table = ExternTable::<TestKind>::new();
        let hips1 = table.intern(Unresolved::located("Armature/Hips".into(), at(3)));
        let body = table.intern(Unresolved::located("Body".into(), at(5)));
        let hips2 = table.intern(Unresolved::located("Armature/Hips".into(), at(8)));
        let hips3 = table.intern(Unresolved::new("Armature/Hips".into()));

        assert_eq!(hips1, hips2);
        assert_eq!(hips1, hips3);
        assert_ne!(hips1, body);
        assert_eq!(table.len(), 2);
        assert_eq!(hips1.index(), 0);
        assert_eq!(body.index(), 1);
        assert_eq!(table.get(hips1).value, "Armature/Hips");
        assert_eq!(table.get(hips1).referenced_at, vec![at(3), at(8)]);
        assert_eq!(table.get(body).referenced_at, vec![at(5)]);
    }

    #[rstest]
    fn a_location_is_recorded_once_however_often_it_is_interned() {
        let mut table = ExternTable::<TestKind>::new();
        table.intern(Unresolved::located("Body".into(), at(3)));
        table.intern(Unresolved::located("Body".into(), at(3)));
        let body = table.intern(Unresolved::located("Body".into(), at(5)));

        assert_eq!(table.get(body).referenced_at, vec![at(3), at(5)]);
    }

    #[rstest]
    fn extern_serializes_as_index() {
        let mut table = ExternTable::<TestKind>::new();
        table.intern(Unresolved::new("Body".into()));
        let hips = table.intern(Unresolved::new("Armature/Hips".into()));

        let bytes = rmp_serde::to_vec(&hips).unwrap();
        assert_eq!(bytes, rmp_serde::to_vec(&1u32).unwrap());
        let restored: Extern<TestKind> = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(restored, hips);
    }

    #[rstest]
    fn table_serializes_as_entry_list() {
        let mut table = ExternTable::<TestKind>::new();
        table.intern(Unresolved::located("Body".into(), at(2)));
        table.intern(Unresolved::new("Armature/Hips".into()));

        let expected = vec![
            ExternEntry::<TestKind> {
                value: "Body".into(),
                referenced_at: vec![at(2)],
            },
            ExternEntry::<TestKind> {
                value: "Armature/Hips".into(),
                referenced_at: vec![],
            },
        ];
        assert_eq!(rmp_serde::to_vec(&table).unwrap(), rmp_serde::to_vec(&expected).unwrap());
    }
}
