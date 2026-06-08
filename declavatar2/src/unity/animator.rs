use crate::unity::value::AnimatedValueType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnimatedTargetSelector {
    Path(String),
    Object(String),
    Mesh(String),
    AnimatorSelf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnimatedProperty {
    Active,
    Enabled,
    TransformPosition,
    TransformRotationQuaternion,
    TransformRotationEuler,
    TransformScale,
    BlendShape { name: String },
    MaterialProperty { name: String },
    AnimatorParameterFloatValue { name: String },
    Serialized { name: String },
}

impl AnimatedProperty {
    pub fn required_value_type(&self) -> Option<AnimatedValueType> {
        match self {
            Self::Active | Self::Enabled => Some(AnimatedValueType::Bool),
            Self::TransformPosition => Some(AnimatedValueType::Vector3),
            Self::TransformRotationQuaternion => Some(AnimatedValueType::Quaternion),
            Self::TransformRotationEuler => Some(AnimatedValueType::Vector3),
            Self::TransformScale => Some(AnimatedValueType::Vector3),
            Self::BlendShape { .. } => Some(AnimatedValueType::Float),
            Self::AnimatorParameterFloatValue { .. } => Some(AnimatedValueType::Float),
            Self::MaterialProperty { .. } | Self::Serialized { .. } => None,
        }
    }

    pub fn accepts_value_type(&self, value_type: AnimatedValueType) -> bool {
        self.required_value_type().is_none_or(|required| required == value_type)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimatedTarget {
    pub selector: AnimatedTargetSelector,
    pub component_type: String,
    pub property: AnimatedProperty,
    pub value_type: AnimatedValueType,
}

impl AnimatedTarget {
    pub fn new(
        selector: AnimatedTargetSelector,
        component_type: impl Into<String>,
        property: AnimatedProperty,
        value_type: AnimatedValueType,
    ) -> Self {
        Self {
            selector,
            component_type: component_type.into(),
            property,
            value_type,
        }
    }

    pub fn accepts_value_type(&self, value_type: AnimatedValueType) -> bool {
        self.value_type == value_type && self.property.accepts_value_type(value_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_property_types_are_checked() {
        let target = AnimatedTarget::new(
            AnimatedTargetSelector::Path("Armature/Hips".into()),
            "Transform",
            AnimatedProperty::TransformRotationQuaternion,
            AnimatedValueType::Quaternion,
        );

        assert!(target.accepts_value_type(AnimatedValueType::Quaternion));
        assert!(!target.accepts_value_type(AnimatedValueType::Vector4));
    }

    #[test]
    fn serialized_targets_can_use_explicit_value_types() {
        let target = AnimatedTarget::new(
            AnimatedTargetSelector::Path("AvatarRoot/Light".into()),
            "UnityEngine.Light",
            AnimatedProperty::Serialized {
                name: "m_Intensity".into(),
            },
            AnimatedValueType::Float,
        );

        assert!(target.accepts_value_type(AnimatedValueType::Float));
        assert!(!target.accepts_value_type(AnimatedValueType::Bool));
    }
}
