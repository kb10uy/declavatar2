use std::{
    fmt::{self, Display},
    ops::RangeInclusive,
};

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
    Int {
        width: ExpressionParameterWidth,
        signedness: ExpressionParameterSignedness,
        default: Option<i32>,
    },

    /// Float, \[-1.0, 1.0\] when signed and \[0.0, 1.0\] when unsigned.
    Float {
        width: ExpressionParameterWidth,
        signedness: ExpressionParameterSignedness,
        default: Option<f32>,
    },
}

/// Represents [`ExpressionParameter`] bit width for Int/Float parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpressionParameterWidth {
    /// The width is not specified by the script.
    /// Actual width will be determined from its usage.
    Unspecified,

    /// The width is specified by the script, in `MIN_BIT_WIDTH..=MAX_BIT_WIDTH`.
    Specified(u8),
}

impl ExpressionParameterWidth {
    pub const MIN_BIT_WIDTH: u8 = 2;
    pub const MAX_BIT_WIDTH: u8 = 8;

    /// The bit count values are checked against. An unspecified width never exceeds the widest one.
    pub fn bits(self) -> u8 {
        match self {
            ExpressionParameterWidth::Unspecified => Self::MAX_BIT_WIDTH,
            ExpressionParameterWidth::Specified(width) => width,
        }
    }
}

/// Represents [`ExpressionParameter`] signedness for Int/Float parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpressionParameterSignedness {
    /// The signedness is not specified by the script.
    /// VRChat takes unsigned for Int and signed for Float.
    Unspecified,

    Unsigned,
    Signed,
}

impl ExpressionParameterSignedness {
    /// Whether values of the given type are signed, with VRChat's default applied when unspecified.
    pub fn is_signed(self, value_type: AnimatedValueType) -> bool {
        match self {
            ExpressionParameterSignedness::Unspecified => value_type == AnimatedValueType::Float,
            ExpressionParameterSignedness::Unsigned => false,
            ExpressionParameterSignedness::Signed => true,
        }
    }
}

/// Values a numeric expression parameter can hold.
#[derive(Debug, Clone, PartialEq)]
pub enum ValueRange {
    Int(RangeInclusive<i64>),
    Float(RangeInclusive<f64>),
}

impl ValueRange {
    pub fn of(value_type: AnimatedValueType, width: ExpressionParameterWidth, signedness: ExpressionParameterSignedness) -> Option<Self> {
        let signed = signedness.is_signed(value_type);
        match value_type {
            AnimatedValueType::Int => {
                let bits = u32::from(width.bits());
                Some(ValueRange::Int(if signed {
                    -(1 << (bits - 1))..=(1 << (bits - 1)) - 1
                } else {
                    0..=(1 << bits) - 1
                }))
            }
            AnimatedValueType::Float => Some(ValueRange::Float(if signed { -1.0..=1.0 } else { 0.0..=1.0 })),
            _ => None,
        }
    }

    pub fn contains_int(&self, value: i64) -> bool {
        match self {
            ValueRange::Int(range) => range.contains(&value),
            ValueRange::Float(range) => range.contains(&(value as f64)),
        }
    }

    pub fn contains_float(&self, value: f64) -> bool {
        match self {
            ValueRange::Int(range) => value.fract() == 0.0 && range.contains(&(value as i64)),
            ValueRange::Float(range) => range.contains(&value),
        }
    }
}

impl Display for ValueRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueRange::Int(range) => write!(f, "{}..={}", range.start(), range.end()),
            ValueRange::Float(range) => write!(f, "{:?}..={:?}", range.start(), range.end()),
        }
    }
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

    /// Values this parameter can hold, or `None` for Bool.
    pub fn value_range(&self) -> Option<ValueRange> {
        match *self {
            ExpressionParameterTypeDefault::Bool(_) => None,
            ExpressionParameterTypeDefault::Int { width, signedness, .. } => ValueRange::of(AnimatedValueType::Int, width, signedness),
            ExpressionParameterTypeDefault::Float { width, signedness, .. } => ValueRange::of(AnimatedValueType::Float, width, signedness),
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

/// Set of parameters that the platform defines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProvidedParameterGroup {
    Vrchat,
}

/// Provided parameters by VRChat.
/// See [VRChat's documentation](https://creators.vrchat.com/avatars/animator-parameters/#built-in-parameters) about details.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    Afk,
    TrackingType,
    VrMode,
    MuteSelf,
    InStation,
    Earmuffs,
    IsOnFriendsList,
    AvatarVersion,
    IsAnimatorEnabled,
    ScaleModified,
    ScaleFactor,
    ScaleFactorInverse,
    EyeHeightAsMeters,
    EyeHeightAsPercent,
}

impl VrchatProvidedParameter {
    /// Every provided parameter, in the order VRChat documents them.
    pub const ALL: &[Self] = &[
        Self::IsLocal,
        Self::PreviewMode,
        Self::Viseme,
        Self::Voice,
        Self::GestureLeft,
        Self::GestureRight,
        Self::GestureLeftWeight,
        Self::GestureRightWeight,
        Self::AngularY,
        Self::VelocityX,
        Self::VelocityY,
        Self::VelocityZ,
        Self::VelocityMagnitude,
        Self::Upright,
        Self::Grounded,
        Self::Seated,
        Self::Afk,
        Self::TrackingType,
        Self::VrMode,
        Self::MuteSelf,
        Self::InStation,
        Self::Earmuffs,
        Self::IsOnFriendsList,
        Self::AvatarVersion,
        Self::IsAnimatorEnabled,
        Self::ScaleModified,
        Self::ScaleFactor,
        Self::ScaleFactorInverse,
        Self::EyeHeightAsMeters,
        Self::EyeHeightAsPercent,
    ];

    /// The name of the animator parameter VRChat writes this value into.
    pub fn name(&self) -> &'static str {
        match self {
            VrchatProvidedParameter::IsLocal => "IsLocal",
            VrchatProvidedParameter::PreviewMode => "PreviewMode",
            VrchatProvidedParameter::Viseme => "Viseme",
            VrchatProvidedParameter::Voice => "Voice",
            VrchatProvidedParameter::GestureLeft => "GestureLeft",
            VrchatProvidedParameter::GestureRight => "GestureRight",
            VrchatProvidedParameter::GestureLeftWeight => "GestureLeftWeight",
            VrchatProvidedParameter::GestureRightWeight => "GestureRightWeight",
            VrchatProvidedParameter::AngularY => "AngularY",
            VrchatProvidedParameter::VelocityX => "VelocityX",
            VrchatProvidedParameter::VelocityY => "VelocityY",
            VrchatProvidedParameter::VelocityZ => "VelocityZ",
            VrchatProvidedParameter::VelocityMagnitude => "VelocityMagnitude",
            VrchatProvidedParameter::Upright => "Upright",
            VrchatProvidedParameter::Grounded => "Grounded",
            VrchatProvidedParameter::Seated => "Seated",
            VrchatProvidedParameter::Afk => "AFK",
            VrchatProvidedParameter::TrackingType => "TrackingType",
            VrchatProvidedParameter::VrMode => "VRMode",
            VrchatProvidedParameter::MuteSelf => "MuteSelf",
            VrchatProvidedParameter::InStation => "InStation",
            VrchatProvidedParameter::Earmuffs => "Earmuffs",
            VrchatProvidedParameter::IsOnFriendsList => "IsOnFriendsList",
            VrchatProvidedParameter::AvatarVersion => "AvatarVersion",
            VrchatProvidedParameter::IsAnimatorEnabled => "IsAnimatorEnabled",
            VrchatProvidedParameter::ScaleModified => "ScaleModified",
            VrchatProvidedParameter::ScaleFactor => "ScaleFactor",
            VrchatProvidedParameter::ScaleFactorInverse => "ScaleFactorInverse",
            VrchatProvidedParameter::EyeHeightAsMeters => "EyeHeightAsMeters",
            VrchatProvidedParameter::EyeHeightAsPercent => "EyeHeightAsPercent",
        }
    }

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
            VrchatProvidedParameter::IsAnimatorEnabled => AnimatedValueType::Bool,
            VrchatProvidedParameter::ScaleModified => AnimatedValueType::Bool,
            VrchatProvidedParameter::ScaleFactor => AnimatedValueType::Float,
            VrchatProvidedParameter::ScaleFactorInverse => AnimatedValueType::Float,
            VrchatProvidedParameter::EyeHeightAsMeters => AnimatedValueType::Float,
            VrchatProvidedParameter::EyeHeightAsPercent => AnimatedValueType::Float,
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::*;

    use super::*;

    #[rstest]
    fn every_provided_parameter_is_listed_once() {
        let mut names: Vec<_> = VrchatProvidedParameter::ALL.iter().map(VrchatProvidedParameter::name).collect();
        let count = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), count);
    }
}
