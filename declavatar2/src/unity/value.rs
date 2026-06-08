use nalgebra::{UnitQuaternion, Vector2, Vector3, Vector4};

/// Represents types of values that can be animated within Unity Animator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
            (Self::Float(x), AnimatedValueType::Int) => {
                AnimatedValueCast::Compatible(Self::Int(*x as i64))
            }
            (Self::Float(x), AnimatedValueType::Bool) => {
                AnimatedValueCast::Compatible(Self::Bool(*x >= 0.5))
            }
            (Self::Int(x), AnimatedValueType::Float) => {
                AnimatedValueCast::Compatible(Self::Float(*x as f64))
            }
            (Self::Int(x), AnimatedValueType::Bool) => {
                AnimatedValueCast::Compatible(Self::Bool(*x != 0))
            }
            (Self::Bool(x), AnimatedValueType::Float) => {
                AnimatedValueCast::Compatible(Self::Float(if *x { 1.0 } else { 0.0 }))
            }
            (Self::Bool(x), AnimatedValueType::Int) => {
                AnimatedValueCast::Compatible(Self::Int(if *x { 1 } else { 0 }))
            }
            _ => AnimatedValueCast::Incompatible,
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
    fn animated_value_reports_its_type(
        #[case] value: AnimatedValue<()>,
        #[case] expected: AnimatedValueType,
    ) {
        assert_eq!(value.value_type(), expected);
        assert_eq!(value.cast(expected), AnimatedValueCast::Same);
    }

    #[rstest]
    #[case(AnimatedValue::Float(std::f64::consts::PI), AnimatedValueType::Int, AnimatedValue::Int(3))]
    #[case(AnimatedValue::Float(std::f64::consts::PI), AnimatedValueType::Bool, AnimatedValue::Bool(true))]
    #[case(AnimatedValue::Int(42), AnimatedValueType::Float, AnimatedValue::Float(42.0))]
    #[case(AnimatedValue::Int(42), AnimatedValueType::Bool, AnimatedValue::Bool(true))]
    #[case(AnimatedValue::Bool(true), AnimatedValueType::Float, AnimatedValue::Float(1.0))]
    #[case(AnimatedValue::Bool(true), AnimatedValueType::Int, AnimatedValue::Int(1))]
    fn value_cast_works(
        #[case] value: AnimatedValue<()>,
        #[case] target: AnimatedValueType,
        #[case] expected: AnimatedValue<()>,
    ) {
        assert_eq!(value.cast(target), AnimatedValueCast::Compatible(expected));
    }
}
