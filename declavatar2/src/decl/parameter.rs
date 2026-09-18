use crate::core::resolution::SourceLocation;

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

/// Type of a declared parameter and its default value, if written.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PrimitiveParameterValue {
    Bool { default: Option<bool> },
    Int { default: Option<i64>, width: Option<u8> },
    Float { default: Option<f64>, width: Option<u8> },
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

/// Set of parameters that the platform defines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProvidedParameterGroup {
    Vrchat,
}
