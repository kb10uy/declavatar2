use nalgebra::{UnitQuaternion, Vector2, Vector3, Vector4};
use serde::{Deserialize, Serialize};

/// Represents types of values that can be animated within Unity Animator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AnimatedValueType {
    Float,
    Int,
    Bool,
    Vector2,
    Vector3,
    Vector4,
    Quaternion,
    Color,
    ObjectReference,
}

impl AnimatedValueType {
    pub fn is_interpolable(&self) -> bool {
        !matches!(self, Self::Int | Self::Bool | Self::ObjectReference)
    }
}

/// Represents a value that can be expressed and animated within Unity Animator,
/// with a generic type `R` for object references.
#[derive(Debug, Clone, PartialEq)]
pub enum AnimatedValue<R> {
    Float(f64),
    Int(i64),
    Bool(bool),
    Vector2(Vector2<f64>),
    Vector3(Vector3<f64>),
    Vector4(Vector4<f64>),
    Quaternion(UnitQuaternion<f64>),
    Color(Vector4<f64>),
    ObjectReference(R),
}

/// Result of attempting to cast an `AnimatedValue` to a specific `AnimatedValueType`.
#[derive(Debug, Clone, PartialEq)]
pub enum AnimatedValueCast<R> {
    Same,
    Compatible(AnimatedValue<R>),
    Incompatible,
}

impl<R> AnimatedValue<R> {
    /// Rewrites the object reference, if this value holds one, and keeps every other value as it is.
    pub fn map_reference<R2>(self, f: impl FnOnce(R) -> R2) -> AnimatedValue<R2> {
        match self {
            AnimatedValue::Float(value) => AnimatedValue::Float(value),
            AnimatedValue::Int(value) => AnimatedValue::Int(value),
            AnimatedValue::Bool(value) => AnimatedValue::Bool(value),
            AnimatedValue::Vector2(value) => AnimatedValue::Vector2(value),
            AnimatedValue::Vector3(value) => AnimatedValue::Vector3(value),
            AnimatedValue::Vector4(value) => AnimatedValue::Vector4(value),
            AnimatedValue::Quaternion(value) => AnimatedValue::Quaternion(value),
            AnimatedValue::Color(value) => AnimatedValue::Color(value),
            AnimatedValue::ObjectReference(reference) => AnimatedValue::ObjectReference(f(reference)),
        }
    }

    /// Linear interpolation between two values of the same interpolable type.
    /// `None` when either value is not interpolable or the types differ.
    pub fn lerp(&self, other: &Self, t: f64) -> Option<Self>
    where
        R: Clone,
    {
        Some(match (self, other) {
            (AnimatedValue::Float(a), AnimatedValue::Float(b)) => AnimatedValue::Float(a + (b - a) * t),
            (AnimatedValue::Vector2(a), AnimatedValue::Vector2(b)) => AnimatedValue::Vector2(a.lerp(b, t)),
            (AnimatedValue::Vector3(a), AnimatedValue::Vector3(b)) => AnimatedValue::Vector3(a.lerp(b, t)),
            (AnimatedValue::Vector4(a), AnimatedValue::Vector4(b)) => AnimatedValue::Vector4(a.lerp(b, t)),
            (AnimatedValue::Quaternion(a), AnimatedValue::Quaternion(b)) => AnimatedValue::Quaternion(a.try_slerp(b, t, 1e-9).unwrap_or_else(|| a.nlerp(b, t))),
            (AnimatedValue::Color(a), AnimatedValue::Color(b)) => AnimatedValue::Color(a.lerp(b, t)),
            _ => return None,
        })
    }
}

impl<R: Clone> AnimatedValue<R> {
    pub fn value_type(&self) -> AnimatedValueType {
        match self {
            Self::Float(_) => AnimatedValueType::Float,
            Self::Int(_) => AnimatedValueType::Int,
            Self::Bool(_) => AnimatedValueType::Bool,
            Self::Vector2(_) => AnimatedValueType::Vector2,
            Self::Vector3(_) => AnimatedValueType::Vector3,
            Self::Vector4(_) => AnimatedValueType::Vector4,
            Self::Quaternion(_) => AnimatedValueType::Quaternion,
            Self::Color(_) => AnimatedValueType::Color,
            Self::ObjectReference(_) => AnimatedValueType::ObjectReference,
        }
    }

    pub fn cast(&self, target_type: AnimatedValueType) -> AnimatedValueCast<R> {
        match (self, target_type) {
            (s, t) if s.value_type() == t => AnimatedValueCast::Same,
            (Self::Float(x), AnimatedValueType::Int) => AnimatedValueCast::Compatible(Self::Int(*x as i64)),
            (Self::Float(x), AnimatedValueType::Bool) => AnimatedValueCast::Compatible(Self::Bool(*x >= 0.5)),
            (Self::Int(x), AnimatedValueType::Float) => AnimatedValueCast::Compatible(Self::Float(*x as f64)),
            (Self::Int(x), AnimatedValueType::Bool) => AnimatedValueCast::Compatible(Self::Bool(*x != 0)),
            (Self::Bool(x), AnimatedValueType::Float) => AnimatedValueCast::Compatible(Self::Float(if *x { 1.0 } else { 0.0 })),
            (Self::Bool(x), AnimatedValueType::Int) => AnimatedValueCast::Compatible(Self::Int(if *x { 1 } else { 0 })),
            _ => AnimatedValueCast::Incompatible,
        }
    }

    pub fn zeroed(&self) -> Option<AnimatedValue<R>> {
        match self {
            AnimatedValue::Float(_) => Some(AnimatedValue::Float(0.0)),
            AnimatedValue::Int(_) => Some(AnimatedValue::Int(0)),
            AnimatedValue::Bool(_) => Some(AnimatedValue::Bool(false)),
            AnimatedValue::Vector2(_) => Some(AnimatedValue::Vector2(Vector2::zeros())),
            AnimatedValue::Vector3(_) => Some(AnimatedValue::Vector3(Vector3::zeros())),
            AnimatedValue::Vector4(_) => Some(AnimatedValue::Vector4(Vector4::zeros())),
            AnimatedValue::Quaternion(_) => Some(AnimatedValue::Quaternion(UnitQuaternion::identity())),
            AnimatedValue::Color(_) => Some(AnimatedValue::Color(Vector4::zeros())),
            AnimatedValue::ObjectReference(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::*;

    use super::*;

    #[rstest]
    #[case(AnimatedValue::Float(std::f64::consts::PI), AnimatedValueType::Float)]
    #[case(AnimatedValue::Int(42), AnimatedValueType::Int)]
    #[case(AnimatedValue::Bool(true), AnimatedValueType::Bool)]
    #[case(AnimatedValue::Vector2([0.0, 0.0].into()), AnimatedValueType::Vector2)]
    #[case(AnimatedValue::Vector3([0.0, 0.0, 0.0].into()), AnimatedValueType::Vector3)]
    #[case(AnimatedValue::Vector4([0.0, 0.0, 0.0, 0.0].into()), AnimatedValueType::Vector4)]
    #[case(AnimatedValue::Quaternion(UnitQuaternion::identity()), AnimatedValueType::Quaternion)]
    #[case(AnimatedValue::Color([0.0, 0.0, 0.0, 0.0].into()), AnimatedValueType::Color)]
    #[case(AnimatedValue::ObjectReference(()), AnimatedValueType::ObjectReference)]
    fn animated_value_reports_its_type(#[case] value: AnimatedValue<()>, #[case] expected: AnimatedValueType) {
        assert_eq!(value.value_type(), expected);
        assert_eq!(value.cast(expected), AnimatedValueCast::Same);
    }

    #[rstest]
    #[case::float(AnimatedValue::Float(0.0), AnimatedValue::Float(10.0), Some(AnimatedValue::Float(2.5)))]
    #[case::vector(AnimatedValue::Vector3([0.0, 0.0, 0.0].into()), AnimatedValue::Vector3([4.0, 8.0, -4.0].into()), Some(AnimatedValue::Vector3([1.0, 2.0, -1.0].into())))]
    #[case::color(AnimatedValue::Color([0.0, 0.0, 0.0, 1.0].into()), AnimatedValue::Color([1.0, 1.0, 1.0, 1.0].into()), Some(AnimatedValue::Color([0.25, 0.25, 0.25, 1.0].into())))]
    #[case::int(AnimatedValue::Int(0), AnimatedValue::Int(4), None)]
    #[case::bool(AnimatedValue::Bool(false), AnimatedValue::Bool(true), None)]
    #[case::reference(AnimatedValue::ObjectReference(()), AnimatedValue::ObjectReference(()), None)]
    #[case::mixed(AnimatedValue::Float(0.0), AnimatedValue::Int(4), None)]
    fn lerp_applies_to_interpolable_values_only(#[case] from: AnimatedValue<()>, #[case] to: AnimatedValue<()>, #[case] expected: Option<AnimatedValue<()>>) {
        assert_eq!(from.lerp(&to, 0.25), expected);
    }

    #[rstest]
    fn lerp_turns_a_quaternion_along_the_shortest_arc() {
        let from = UnitQuaternion::identity();
        let to = UnitQuaternion::from_axis_angle(&Vector3::y_axis(), std::f64::consts::FRAC_PI_2);
        let AnimatedValue::Quaternion(half) = AnimatedValue::<()>::Quaternion(from).lerp(&AnimatedValue::Quaternion(to), 0.5).unwrap() else {
            panic!("a quaternion should stay a quaternion");
        };
        assert!((half.angle() - std::f64::consts::FRAC_PI_4).abs() < 1e-9);
    }

    #[rstest]
    fn map_reference_rewrites_the_reference_only() {
        assert_eq!(AnimatedValue::ObjectReference("a").map_reference(str::len), AnimatedValue::ObjectReference(1));
        assert_eq!(AnimatedValue::<&str>::Int(3).map_reference(str::len), AnimatedValue::Int(3));
    }

    #[rstest]
    #[case(AnimatedValue::Float(std::f64::consts::PI), AnimatedValueType::Int, AnimatedValue::Int(3))]
    #[case(AnimatedValue::Float(std::f64::consts::PI), AnimatedValueType::Bool, AnimatedValue::Bool(true))]
    #[case(AnimatedValue::Int(42), AnimatedValueType::Float, AnimatedValue::Float(42.0))]
    #[case(AnimatedValue::Int(42), AnimatedValueType::Bool, AnimatedValue::Bool(true))]
    #[case(AnimatedValue::Bool(true), AnimatedValueType::Float, AnimatedValue::Float(1.0))]
    #[case(AnimatedValue::Bool(true), AnimatedValueType::Int, AnimatedValue::Int(1))]
    fn value_cast_works(#[case] value: AnimatedValue<()>, #[case] target: AnimatedValueType, #[case] expected: AnimatedValue<()>) {
        assert_eq!(value.cast(target), AnimatedValueCast::Compatible(expected));
    }
}
