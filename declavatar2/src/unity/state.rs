use std::{collections::BTreeMap, fmt::Debug};

/// Traits for AnimatorController StateBehaviour definitions.
///
/// Note: the spelling of `StateBehavior` is intentionally different from `StateBehaviour`.
pub trait StateBehavior: Debug {
    /// Returns the identification name of the state behavior.
    /// This takes self receiver to keep itself dyn-compatible.
    fn name(&self) -> &str;

    /// Clones this definition.
    /// This is apart of the `Clone` trait due to its `Sized` requirement, which leads to dyn-incompatible.
    fn clone(&self) -> Box<dyn StateBehavior>;
}

impl Clone for Box<dyn StateBehavior> {
    fn clone(&self) -> Self {
        self.as_ref().clone()
    }
}

/// State behavior whose type declavatar2 does not know about.
/// Its fields are carried through the transform untouched, and the client applies them to the actual component.
#[derive(Debug, Clone, PartialEq)]
pub struct GenericStateBehavior {
    pub type_name: String,
    pub fields: BTreeMap<String, GenericValue>,
}

/// Value held by a `GenericStateBehavior`.
/// This is plain data; parameters, object paths and assets are never resolved or interned inside it.
#[derive(Debug, Clone, PartialEq)]
pub enum GenericValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    List(Vec<GenericValue>),
    Map(BTreeMap<String, GenericValue>),
}

impl StateBehavior for GenericStateBehavior {
    fn name(&self) -> &str {
        &self.type_name
    }

    fn clone(&self) -> Box<dyn StateBehavior> {
        Box::new(Clone::clone(self))
    }
}

#[cfg(test)]
mod tests {
    use rstest::*;

    use super::*;

    fn physbone() -> GenericStateBehavior {
        GenericStateBehavior {
            type_name: "VRC.SDK3.Avatars.Components.VRCAnimatorLayerControl".into(),
            fields: BTreeMap::from([
                ("layer".into(), GenericValue::Int(3)),
                ("goalWeight".into(), GenericValue::Float(1.0)),
                ("debugString".into(), GenericValue::String("hello".into())),
                (
                    "blendableLayers".into(),
                    GenericValue::List(vec![GenericValue::Bool(true), GenericValue::Bool(false)]),
                ),
                ("nested".into(), GenericValue::Map(BTreeMap::from([("x".into(), GenericValue::Float(0.5))]))),
            ]),
        }
    }

    #[rstest]
    fn generic_behavior_reports_its_own_type_name() {
        let behavior = physbone();
        assert_eq!(behavior.name(), "VRC.SDK3.Avatars.Components.VRCAnimatorLayerControl");
    }

    #[rstest]
    fn boxed_generic_behavior_clones_into_an_equal_value() {
        let behavior = physbone();
        let boxed: Box<dyn StateBehavior> = Box::new(Clone::clone(&behavior));
        let cloned = Clone::clone(&boxed);

        assert_eq!(cloned.name(), behavior.name());
        assert_eq!(format!("{cloned:?}"), format!("{behavior:?}"));
    }
}
