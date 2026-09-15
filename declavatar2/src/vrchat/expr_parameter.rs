use serde::{Deserialize, Serialize};

use crate::unity::AnimatedValueType;

#[derive(Debug, Clone, PartialEq)]
pub enum Parameter {
    Expression(ExpressionParameter),
    Vrchat(VrchatProvidedParameter),
}

/// Represents a [Expression Parameter](https://creators.vrchat.com/avatars/expression-menu-and-controls).
#[derive(Debug, Clone, PartialEq)]
pub struct ExpressionParameter {
    pub name: String,
    pub type_default: ExpressionParameterTypeDefault,
    pub saved: bool,
    pub synced: bool,
}

/// Represents type and default value (if exist) of a [`ExpressionParameter`]s.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExpressionParameterTypeDefault {
    /// Boolean.
    Bool(Option<bool>),

    /// Integer.
    Int { width: ExpressionParameterWidth, default: Option<i32> },

    /// Float \[-1.0, 1.0\].
    Float { width: ExpressionParameterWidth, default: Option<f32> },
}

/// Represents [`ExpressionParameter`] bit width for Int/Float parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpressionParameterWidth {
    /// The width is not specified by the script.
    /// Actual width will be determined from its usage.
    Unspecified,

    /// The width is specified by the script.
    Specified(u8),
}

impl ExpressionParameterTypeDefault {
    /// Bit width of this type.
    pub fn data_width(&self) -> Option<u8> {
        match self {
            ExpressionParameterTypeDefault::Bool(_) => Some(1),
            ExpressionParameterTypeDefault::Int { width, .. } | ExpressionParameterTypeDefault::Float { width, .. } => match width {
                ExpressionParameterWidth::Unspecified => None,
                ExpressionParameterWidth::Specified(w) => Some(*w),
            },
        }
    }

    pub fn animated_value_type(&self) -> AnimatedValueType {
        match self {
            ExpressionParameterTypeDefault::Bool(_) => AnimatedValueType::Bool,
            ExpressionParameterTypeDefault::Int { .. } => AnimatedValueType::Int,
            ExpressionParameterTypeDefault::Float { .. } => AnimatedValueType::Float,
        }
    }
}

/// Provided parameters by VRChat.
/// See [VRChat's documentation](https://creators.vrchat.com/avatars/animator-parameters/#built-in-parameters) about details.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VrchatProvidedParameter {
    IsLocal,
    PreviewMode,
    Viseme,
    Voice,
    GestureLeft,
    GestureRight,
    GestureLeftWeight,
    GestureRightWeight,
    AngularY,
    VelocityX,
    VelocityY,
    VelocityZ,
    VelocityMagnitude,
    Upright,
    Grounded,
    Seated,
    #[serde(rename = "AFK")]
    Afk,
    TrackingType,
    #[serde(rename = "VRMode")]
    VrMode,
    MuteSelf,
    InStation,
    Earmuffs,
    IsOnFriendsList,
    AvatarVersion,
    IsAnimtorEnabled,
    ScaleModified,
    ScaleFactor,
    ScaleFactorInverse,
    EyeHeightAsMeters,
    EyeHeightAsPercent,
}

impl VrchatProvidedParameter {
    pub fn animated_value_type(&self) -> AnimatedValueType {
        match self {
            VrchatProvidedParameter::IsLocal => AnimatedValueType::Bool,
            VrchatProvidedParameter::PreviewMode => AnimatedValueType::Int,
            VrchatProvidedParameter::Viseme => AnimatedValueType::Int,
            VrchatProvidedParameter::Voice => AnimatedValueType::Float,
            VrchatProvidedParameter::GestureLeft => AnimatedValueType::Int,
            VrchatProvidedParameter::GestureRight => AnimatedValueType::Int,
            VrchatProvidedParameter::GestureLeftWeight => AnimatedValueType::Float,
            VrchatProvidedParameter::GestureRightWeight => AnimatedValueType::Float,
            VrchatProvidedParameter::AngularY => AnimatedValueType::Float,
            VrchatProvidedParameter::VelocityX => AnimatedValueType::Float,
            VrchatProvidedParameter::VelocityY => AnimatedValueType::Float,
            VrchatProvidedParameter::VelocityZ => AnimatedValueType::Float,
            VrchatProvidedParameter::VelocityMagnitude => AnimatedValueType::Float,
            VrchatProvidedParameter::Upright => AnimatedValueType::Float,
            VrchatProvidedParameter::Grounded => AnimatedValueType::Bool,
            VrchatProvidedParameter::Seated => AnimatedValueType::Bool,
            VrchatProvidedParameter::Afk => AnimatedValueType::Bool,
            VrchatProvidedParameter::TrackingType => AnimatedValueType::Int,
            VrchatProvidedParameter::VrMode => AnimatedValueType::Int,
            VrchatProvidedParameter::MuteSelf => AnimatedValueType::Bool,
            VrchatProvidedParameter::InStation => AnimatedValueType::Bool,
            VrchatProvidedParameter::Earmuffs => AnimatedValueType::Bool,
            VrchatProvidedParameter::IsOnFriendsList => AnimatedValueType::Bool,
            VrchatProvidedParameter::AvatarVersion => AnimatedValueType::Int,
            VrchatProvidedParameter::IsAnimtorEnabled => AnimatedValueType::Bool,
            VrchatProvidedParameter::ScaleModified => AnimatedValueType::Bool,
            VrchatProvidedParameter::ScaleFactor => AnimatedValueType::Float,
            VrchatProvidedParameter::ScaleFactorInverse => AnimatedValueType::Float,
            VrchatProvidedParameter::EyeHeightAsMeters => AnimatedValueType::Float,
            VrchatProvidedParameter::EyeHeightAsPercent => AnimatedValueType::Float,
        }
    }
}
