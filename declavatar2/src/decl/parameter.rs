use crate::{
    core::resolution::SourceLocation,
    unity::AnimatedValueType,
    vrchat::expr_parameter::{ExpressionParameterSignedness, ExpressionParameterWidth, ValueRange},
};

pub use crate::vrchat::expr_parameter::ProvidedParameterGroup;

/// Entry of the `parameters` block.
#[derive(Debug, Clone, PartialEq)]
pub enum Parameter {
    /// Parameter declared by the script itself.
    Primitive(PrimitiveParameter),

    /// Bulk declaration of parameters provided by the platform.
    Provided(ProvidedParameterGroup),
}

/// Parameter declared by `da.bool` / `da.int` / `da.float`.
/// Unspecified options stay `None` here; the transform decides their actual values.
#[derive(Debug, Clone, PartialEq)]
pub struct PrimitiveParameter {
    pub name: String,
    pub value: PrimitiveParameterValue,
    pub scope: Option<ParameterScope>,
    pub save: Option<bool>,
    pub at: Option<SourceLocation>,
}

impl PrimitiveParameter {
    /// Values the parameter can hold as an expression parameter, or `None` when it is Bool or internal.
    pub fn value_range(&self) -> Option<ValueRange> {
        if self.scope == Some(ParameterScope::Internal) {
            return None;
        }
        let (value_type, repr) = match self.value {
            PrimitiveParameterValue::Bool { .. } => return None,
            PrimitiveParameterValue::Int { repr, .. } => (AnimatedValueType::Int, repr),
            PrimitiveParameterValue::Float { repr, .. } => (AnimatedValueType::Float, repr),
        };
        ValueRange::of(value_type, ParameterRepr::width_of(repr), ParameterRepr::signedness_of(repr))
    }

    /// The written default and the range it falls outside of, when it does.
    pub fn default_out_of_range(&self) -> Option<(String, ValueRange)> {
        let range = self.value_range()?;
        let (inside, written) = match self.value {
            PrimitiveParameterValue::Int { default: Some(value), .. } => (range.contains_int(value), value.to_string()),
            PrimitiveParameterValue::Float { default: Some(value), .. } => (range.contains_float(value), format!("{value:?}")),
            _ => return None,
        };
        (!inside).then_some((written, range))
    }
}

/// Type of a declared parameter and its default value, if written.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PrimitiveParameterValue {
    Bool { default: Option<bool> },
    Int { default: Option<i64>, repr: Option<ParameterRepr> },
    Float { default: Option<f64>, repr: Option<ParameterRepr> },
}

/// Signedness and bit width written as `repr`, such as `"u4"` or `"s"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParameterRepr {
    pub signedness: Signedness,
    pub width: Option<u8>,
}

impl ParameterRepr {
    pub fn width_of(repr: Option<ParameterRepr>) -> ExpressionParameterWidth {
        repr.and_then(|repr| repr.width)
            .map_or(ExpressionParameterWidth::Unspecified, ExpressionParameterWidth::Specified)
    }

    pub fn signedness_of(repr: Option<ParameterRepr>) -> ExpressionParameterSignedness {
        match repr.map(|repr| repr.signedness) {
            None => ExpressionParameterSignedness::Unspecified,
            Some(Signedness::Unsigned) => ExpressionParameterSignedness::Unsigned,
            Some(Signedness::Signed) => ExpressionParameterSignedness::Signed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signedness {
    Unsigned,
    Signed,
}

/// How far a parameter is visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ParameterScope {
    /// Synced to remote players.
    Synced,

    /// Expression parameter that is not synced.
    Local,

    /// Animator-only parameter, not exported as an expression parameter.
    Internal,
}
