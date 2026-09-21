use std::{
    cmp::Ordering,
    hash::{Hash, Hasher},
};

/// Position in the script where a reference was written.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceLocation {
    pub chunk: String,
    pub line: u32,
}

/// A reference that is only known by its name; whether it exists is not yet checked.
/// Equality and ordering consider only the value, so two references to the same name are the same reference.
/// This type intentionally has no interop encoding so that unresolved references can never leak into output.
#[derive(Debug, Clone)]
pub struct Unresolved<T> {
    pub value: T,
    pub at: Option<SourceLocation>,
}

impl<T> Unresolved<T> {
    pub fn new(value: T) -> Self {
        Self { value, at: None }
    }

    pub fn located(value: T, at: SourceLocation) -> Self {
        Self { value, at: Some(at) }
    }
}

impl<T> From<T> for Unresolved<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T: PartialEq> PartialEq for Unresolved<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<T: Eq> Eq for Unresolved<T> {}

impl<T: PartialOrd> PartialOrd for Unresolved<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.value.partial_cmp(&other.value)
    }
}

impl<T: Ord> Ord for Unresolved<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.cmp(&other.value)
    }
}

impl<T: Hash> Hash for Unresolved<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

/// A reference whose existence has been confirmed by the transformer.
/// `context` carries information gathered during resolution and is never encoded; only `value` is.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Resolved<T, C> {
    pub value: T,
    pub context: C,
}

impl<T, C> Resolved<T, C> {
    pub fn new(value: T, context: C) -> Self {
        Self { value, context }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use rstest::*;

    use super::*;

    fn at(line: u32) -> SourceLocation {
        SourceLocation {
            chunk: "avatar.lua".into(),
            line,
        }
    }

    #[rstest]
    fn unresolved_identity_ignores_location() {
        let first = Unresolved::located("Emote", at(1));
        let second = Unresolved::located("Emote", at(20));
        let other = Unresolved::new("Hat");

        assert_eq!(first, second);
        assert_ne!(first, other);
        assert_eq!(BTreeSet::from([first, second, other]).len(), 2);
    }
}
