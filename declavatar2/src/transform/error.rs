use std::{
    error::Error,
    fmt::{self, Display},
};

use thiserror::Error;

use crate::{core::resolution::SourceLocation, unity::value::AnimatedValueType};

/// One thing the transform could not accept, and where in the script it was written.
#[derive(Debug, Clone, PartialEq)]
pub struct TransformError {
    pub at: Option<SourceLocation>,
    pub kind: TransformErrorKind,
}

impl TransformError {
    pub fn new(kind: TransformErrorKind) -> Self {
        Self { at: None, kind }
    }

    pub fn at(kind: TransformErrorKind, at: Option<SourceLocation>) -> Self {
        Self { at, kind }
    }

    /// Fills the location in when the error has none, so an error inside a layer points at the layer at least.
    pub fn or_at(mut self, at: Option<&SourceLocation>) -> Self {
        if self.at.is_none() {
            self.at = at.cloned();
        }
        self
    }
}

impl Display for TransformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.at {
            Some(at) => write!(f, "{}:{}: {}", at.chunk, at.line, self.kind),
            None => write!(f, "{}", self.kind),
        }
    }
}

impl Error for TransformError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.kind)
    }
}

/// Every error the transform found. The transform keeps going after an error so that one run reports them all.
#[derive(Debug, Clone, PartialEq)]
pub struct TransformErrors(pub Vec<TransformError>);

impl Display for TransformErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (offset, error) in self.0.iter().enumerate() {
            if offset > 0 {
                writeln!(f)?;
            }
            write!(f, "{error}")?;
        }
        Ok(())
    }
}

impl Error for TransformErrors {}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum TransformErrorKind {
    #[error("parameter `{name}` is declared more than once")]
    DuplicateParameter { name: String },

    #[error("parameter `{name}` collides with a provided parameter of the same name")]
    ParameterCollidesWithProvided { name: String },

    #[error("provided parameters `{group}` are declared more than once")]
    DuplicateProvidedGroup { group: String },

    #[error("parameter `{name}` is not declared")]
    UnknownParameter { name: String },

    #[error("parameter `{name}` is {found:?}, but {expected:?} is needed here")]
    ParameterTypeMismatch {
        name: String,
        expected: AnimatedValueType,
        found: AnimatedValueType,
    },

    #[error("the transform needs parameter `{name}` for itself, but the script declares it")]
    GeneratedParameterCollision { name: String },

    #[error("layer `{name}` is declared more than once")]
    DuplicateLayer { name: String },

    #[error("layer `{name}` is not declared")]
    UnknownLayer { name: String },

    #[error("layer `{name}` is a {found} layer, but a {expected} layer is needed here")]
    LayerKindMismatch {
        name: String,
        expected: &'static str,
        found: &'static str,
    },

    #[error("option `{option}` of `{layer}` is declared more than once")]
    DuplicateOption { layer: String, option: String },

    #[error("layer `{layer}` has no option `{option}`")]
    UnknownOption { layer: String, option: String },

    #[error("the options of `{layer}` animate {targets}, which cannot be zeroed and must be written in the default")]
    OptionsNeedDefault { layer: String, targets: String },

    #[error("the toggle list of `{layer}` animates {target}, which cannot be zeroed for the disabled side")]
    ToggleNeedsBothSides { layer: String, target: String },

    #[error("puppet layer `{layer}` has no keyframes")]
    NoKeyframes { layer: String },

    #[error("puppet layer `{layer}` places two keyframes at {time}")]
    DuplicateKeyframe { layer: String, time: f64 },

    #[error("the keyframes of `{layer}` write {target} as both {first:?} and {second:?}")]
    KeyframeTypeMismatch {
        layer: String,
        target: String,
        first: AnimatedValueType,
        second: AnimatedValueType,
    },

    #[error("children of blend layer `{layer}` both animate {target}, and children sum instead of overriding")]
    OverlappingBlendTargets { layer: String, target: String },

    #[error("state `{state}` of `{layer}` is declared more than once")]
    DuplicateState { layer: String, state: String },

    #[error("layer `{layer}` has no state `{state}`")]
    UnknownState { layer: String, state: String },

    #[error("`{condition}` cannot be applied to parameter `{parameter}` of type {value_type:?}")]
    UnsupportedCondition {
        condition: &'static str,
        parameter: String,
        value_type: AnimatedValueType,
    },

    #[error("a {expected:?} value is needed here, but a {found:?} value was written")]
    ValueTypeMismatch { expected: AnimatedValueType, found: AnimatedValueType },

    #[error("`{option}` can only be set on the motion of a state, not on a field of a blend tree")]
    NestedPlayback { option: &'static str },

    #[error("an axis takes a parameter name or `da.drive_puppet(layer)` without a value")]
    InvalidAxis,

    #[error("menu `{name}` holds {count} controls, but a menu can hold {capacity} at most")]
    MenuTooLarge { name: String, count: usize, capacity: usize },
}

impl TransformErrorKind {
    pub fn at(self, at: Option<SourceLocation>) -> TransformError {
        TransformError::at(self, at)
    }
}

impl From<TransformErrorKind> for TransformError {
    fn from(kind: TransformErrorKind) -> Self {
        TransformError::new(kind)
    }
}

#[cfg(test)]
mod tests {
    use rstest::*;

    use super::*;

    fn at(line: u32) -> Option<SourceLocation> {
        Some(SourceLocation {
            chunk: "avatar.lua".into(),
            line,
        })
    }

    #[rstest]
    fn a_located_error_names_its_line() {
        let error = TransformErrorKind::UnknownParameter { name: "Emote".into() }.at(at(9));
        assert_eq!(error.to_string(), "avatar.lua:9: parameter `Emote` is not declared");
    }

    #[rstest]
    fn an_unlocated_error_is_the_message_alone() {
        let error = TransformError::new(TransformErrorKind::UnknownParameter { name: "Emote".into() });
        assert_eq!(error.to_string(), "parameter `Emote` is not declared");
    }

    #[rstest]
    fn or_at_fills_a_missing_location_only() {
        let kept = TransformErrorKind::UnknownParameter { name: "A".into() }.at(at(3)).or_at(at(7).as_ref());
        assert_eq!(kept.at, at(3));

        let filled = TransformError::new(TransformErrorKind::UnknownParameter { name: "A".into() }).or_at(at(7).as_ref());
        assert_eq!(filled.at, at(7));
    }

    #[rstest]
    fn errors_are_listed_one_per_line() {
        let errors = TransformErrors(vec![
            TransformErrorKind::UnknownParameter { name: "A".into() }.at(at(1)),
            TransformErrorKind::UnknownLayer { name: "B".into() }.at(at(2)),
        ]);
        assert_eq!(
            errors.to_string(),
            "avatar.lua:1: parameter `A` is not declared\navatar.lua:2: layer `B` is not declared"
        );
    }
}
