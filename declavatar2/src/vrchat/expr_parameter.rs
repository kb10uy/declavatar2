use crate::unity::AnimatedValueType;

#[derive(Debug, Clone, PartialEq)]
pub enum Parameter {
    Expression(ExpressionParameter),
}

/// Represents a [Expression Parameter](https://creators.vrchat.com/avatars/expression-menu-and-controls).
#[derive(Debug, Clone, PartialEq)]
pub struct ExpressionParameter {
    pub name: String,
    pub type_default: ExpressionParameterTypeDefault,
    pub saved: bool,
    pub synced: bool,
}

/// Represents type and default value (if exist) of a `ExpressionParameter`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExpressionParameterTypeDefault {
    /// Boolean.
    Bool(Option<bool>),

    /// Integer.
    Int(Option<u8>),

    /// Float \[-1.0, 1.0\].
    Float(Option<f32>),
}

impl ExpressionParameterTypeDefault {
    /// Bit width of this type.
    pub fn data_width(&self) -> usize {
        match self {
            ExpressionParameterTypeDefault::Bool(_) => 1,
            ExpressionParameterTypeDefault::Int(_) => 8,
            ExpressionParameterTypeDefault::Float(_) => 8,
        }
    }

    pub fn animated_value_type(&self) -> AnimatedValueType {
        match self {
            ExpressionParameterTypeDefault::Bool(_) => AnimatedValueType::Bool,
            ExpressionParameterTypeDefault::Int(_) => AnimatedValueType::Int,
            ExpressionParameterTypeDefault::Float(_) => AnimatedValueType::Float,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VrchatProvidedParameter {}

impl VrchatProvidedParameter {
    pub fn animated_value_type(&self) -> AnimatedValueType {
        match self {}
    }
}
