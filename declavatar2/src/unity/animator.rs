use crate::unity::value::AnimatedValueType;

pub trait Target {
    /// Returns the type hint for this target, if known.
    fn type_hint(&self) -> Option<AnimatedValueType>;
}

/// Target specifier for an animated property.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnimatedTarget<P, T> {
    /// Animator itself. Mainly used for AAPs.
    AnimatorSelf(AnimatedAnimatorTarget),

    /// GameObject.
    GameObject(AnimatedGameObjectTarget<P>),

    /// Renderer, especially MeshRenderer and SkinnedMeshRenderer.
    Renderer(AnimatedRendererTarget<P>),

    /// Other component.
    Component(AnimatedComponentTarget<P, T>),
}

impl<P, T> Target for AnimatedTarget<P, T> {
    fn type_hint(&self) -> Option<AnimatedValueType> {
        match self {
            Self::AnimatorSelf(animator_self) => animator_self.type_hint(),
            Self::GameObject(game_object) => game_object.type_hint(),
            Self::Renderer(renderer) => renderer.type_hint(),
            Self::Component(component) => component.type_hint(),
        }
    }
}

/// Target specifier object for Animator properties.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimatedAnimatorTarget {
    pub property: AnimatedAnimatorProperty,
}

/// Represents animated property on an Animator component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnimatedAnimatorProperty {
    /// Float parameter value.
    ParameterFloatValue { name: String },
}

impl Target for AnimatedAnimatorTarget {
    fn type_hint(&self) -> Option<AnimatedValueType> {
        match self.property {
            AnimatedAnimatorProperty::ParameterFloatValue { .. } => Some(AnimatedValueType::Float),
        }
    }
}

/// Target specifier object for GameObject properties.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimatedGameObjectTarget<P> {
    pub path: P,
    pub property: AnimatedGameObjectProperty,
}

/// Represents animated property of a GameObject.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimatedGameObjectProperty {
    /// Active/Inactive state.
    Active,

    /// Transform local position.
    TransformPosition,

    /// Transform local rotation (quaternion expression).
    TransformRotationQuaternion,

    /// Transform local rotation (euler angles expression).
    TransformRotationEuler,

    /// Transform local scale.
    TransformScale,
}

impl<P> Target for AnimatedGameObjectTarget<P> {
    fn type_hint(&self) -> Option<AnimatedValueType> {
        match self.property {
            AnimatedGameObjectProperty::Active => Some(AnimatedValueType::Bool),
            AnimatedGameObjectProperty::TransformPosition => Some(AnimatedValueType::Vector3),
            AnimatedGameObjectProperty::TransformRotationQuaternion => Some(AnimatedValueType::Quaternion),
            AnimatedGameObjectProperty::TransformRotationEuler => Some(AnimatedValueType::Vector3),
            AnimatedGameObjectProperty::TransformScale => Some(AnimatedValueType::Vector3),
        }
    }
}

/// Target specifier object for Renderers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimatedRendererTarget<P> {
    pub path: P,
    pub renderer_type: String,
    pub property: AnimatedRendererProperty,
}

/// Represents animated property of a Renderer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnimatedRendererProperty {
    /// Enabled/disabled state.
    Enabled,

    /// Blend shape value.
    BlendShape { name: String },

    /// Material property value.
    MaterialProperty { name: String },

    /// Arbitrary serialized property value.
    Serialized { name: String },
}

impl<P> Target for AnimatedRendererTarget<P> {
    fn type_hint(&self) -> Option<AnimatedValueType> {
        match self.property {
            AnimatedRendererProperty::Enabled => Some(AnimatedValueType::Bool),
            AnimatedRendererProperty::BlendShape { .. } => Some(AnimatedValueType::Float),
            AnimatedRendererProperty::MaterialProperty { .. } | AnimatedRendererProperty::Serialized { .. } => None,
        }
    }
}

/// Target specifier object for arbitrary Unity components.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimatedComponentTarget<P, T> {
    pub path: P,
    pub component_type: T,
    pub property: AnimatedComponentProperty,
    pub value_type: AnimatedValueType,
}

/// Represents animated property of a Unity component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnimatedComponentProperty {
    /// Enabled/disabled state.
    Enabled,

    /// Arbitrary serialized property value.
    Serialized { name: String },
}

impl<P, T> Target for AnimatedComponentTarget<P, T> {
    fn type_hint(&self) -> Option<AnimatedValueType> {
        match self.property {
            AnimatedComponentProperty::Enabled => Some(AnimatedValueType::Bool),
            AnimatedComponentProperty::Serialized { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(
        AnimatedTarget::<&str, &str>::AnimatorSelf(AnimatedAnimatorTarget {
            property: AnimatedAnimatorProperty::ParameterFloatValue {
                name: "GestureLeft".into(),
            },
        }),
        Some(AnimatedValueType::Float),
    )]
    #[case(
        AnimatedTarget::GameObject(AnimatedGameObjectTarget {
            path: "Armature/Hips",
            property: AnimatedGameObjectProperty::Active,
        }),
        Some(AnimatedValueType::Bool),
    )]
    #[case(
        AnimatedTarget::GameObject(AnimatedGameObjectTarget {
            path: "Armature/Hips",
            property: AnimatedGameObjectProperty::TransformPosition,
        }),
        Some(AnimatedValueType::Vector3),
    )]
    #[case(
        AnimatedTarget::GameObject(AnimatedGameObjectTarget {
            path: "Armature/Hips",
            property: AnimatedGameObjectProperty::TransformRotationQuaternion,
        }),
        Some(AnimatedValueType::Quaternion),
    )]
    #[case(
        AnimatedTarget::GameObject(AnimatedGameObjectTarget {
            path: "Armature/Hips",
            property: AnimatedGameObjectProperty::TransformRotationEuler,
        }),
        Some(AnimatedValueType::Vector3),
    )]
    #[case(
        AnimatedTarget::GameObject(AnimatedGameObjectTarget {
            path: "Armature/Hips",
            property: AnimatedGameObjectProperty::TransformScale,
        }),
        Some(AnimatedValueType::Vector3),
    )]
    #[case(
        AnimatedTarget::Renderer(AnimatedRendererTarget {
            path: "Body",
            renderer_type: "UnityEngine.SkinnedMeshRenderer".into(),
            property: AnimatedRendererProperty::Enabled,
        }),
        Some(AnimatedValueType::Bool),
    )]
    #[case(
        AnimatedTarget::Renderer(AnimatedRendererTarget {
            path: "Body",
            renderer_type: "UnityEngine.SkinnedMeshRenderer".into(),
            property: AnimatedRendererProperty::BlendShape {
                name: "Smile".into(),
            },
        }),
        Some(AnimatedValueType::Float),
    )]
    #[case(
        AnimatedTarget::Renderer(AnimatedRendererTarget {
            path: "Body",
            renderer_type: "UnityEngine.Renderer".into(),
            property: AnimatedRendererProperty::MaterialProperty {
                name: "_Color".into(),
            },
        }),
        None,
    )]
    #[case(
        AnimatedTarget::Renderer(AnimatedRendererTarget {
            path: "Body",
            renderer_type: "UnityEngine.Renderer".into(),
            property: AnimatedRendererProperty::Serialized {
                name: "m_ProbeAnchor".into(),
            },
        }),
        None,
    )]
    #[case(
        AnimatedTarget::Component(AnimatedComponentTarget {
            path: "AvatarRoot/Light",
            component_type: "UnityEngine.Light",
            property: AnimatedComponentProperty::Enabled,
            value_type: AnimatedValueType::Bool,
        }),
        Some(AnimatedValueType::Bool),
    )]
    #[case(
        AnimatedTarget::Component(AnimatedComponentTarget {
            path: "AvatarRoot/PhysBone",
            component_type: "VRC.SDK3.Dynamics.PhysBone.Components.VRCPhysBone",
            property: AnimatedComponentProperty::Serialized {
                name: "pull".into(),
            },
            value_type: AnimatedValueType::Float,
        }),
        None,
    )]
    fn animated_target_type_hints_match_expected(#[case] target: AnimatedTarget<&str, &str>, #[case] expected: Option<AnimatedValueType>) {
        assert_eq!(target.type_hint(), expected);
    }

    #[rstest]
    #[case(
        AnimatedAnimatorTarget {
            property: AnimatedAnimatorProperty::ParameterFloatValue {
                name: "GestureRight".into(),
            },
        },
        Some(AnimatedValueType::Float),
    )]
    fn animator_target_reports_expected_type_hint(#[case] target: AnimatedAnimatorTarget, #[case] expected: Option<AnimatedValueType>) {
        assert_eq!(target.type_hint(), expected);
    }

    #[rstest]
    #[case(
        AnimatedGameObjectTarget {
            path: "Armature/Head",
            property: AnimatedGameObjectProperty::Active,
        },
        Some(AnimatedValueType::Bool),
    )]
    #[case(
        AnimatedGameObjectTarget {
            path: "Armature/Head",
            property: AnimatedGameObjectProperty::TransformRotationEuler,
        },
        Some(AnimatedValueType::Vector3),
    )]
    fn game_object_target_reports_expected_type_hint(#[case] target: AnimatedGameObjectTarget<&str>, #[case] expected: Option<AnimatedValueType>) {
        assert_eq!(target.type_hint(), expected);
    }

    #[rstest]
    #[case(
        AnimatedRendererTarget {
            path: "Body",
            renderer_type: "UnityEngine.Renderer".into(),
            property: AnimatedRendererProperty::Enabled,
        },
        Some(AnimatedValueType::Bool),
    )]
    #[case(
        AnimatedRendererTarget {
            path: "Body",
            renderer_type: "UnityEngine.SkinnedMeshRenderer".into(),
            property: AnimatedRendererProperty::Serialized {
                name: "m_Quality".into(),
            },
        },
        None,
    )]
    fn renderer_target_reports_expected_type_hint(#[case] target: AnimatedRendererTarget<&str>, #[case] expected: Option<AnimatedValueType>) {
        assert_eq!(target.type_hint(), expected);
    }

    #[rstest]
    #[case(
        AnimatedComponentTarget {
            path: "AvatarRoot/Light",
            component_type: "UnityEngine.Light",
            property: AnimatedComponentProperty::Enabled,
            value_type: AnimatedValueType::Bool,
        },
        Some(AnimatedValueType::Bool),
    )]
    #[case(
        AnimatedComponentTarget {
            path: "AvatarRoot/PhysBone",
            component_type: "VRC.SDK3.Dynamics.PhysBone.Components.VRCPhysBone",
            property: AnimatedComponentProperty::Serialized {
                name: "immobile".into(),
            },
            value_type: AnimatedValueType::Float,
        },
        None,
    )]
    fn component_target_reports_expected_type_hint(#[case] target: AnimatedComponentTarget<&str, &str>, #[case] expected: Option<AnimatedValueType>) {
        assert_eq!(target.type_hint(), expected);
    }
}
