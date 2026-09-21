use serde::Serialize;

use crate::{core::phase::Phase, unity::value::AnimatedValueType};

pub trait Target {
    /// Returns the type hint for this target, if known.
    fn type_hint(&self) -> Option<AnimatedValueType>;
}

/// Target specifier for an animated property.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(bound(serialize = "Ph::ParameterRef: Serialize, Ph::ObjectPath: Serialize, Ph::ComponentType: Serialize"))]
pub enum AnimatedTarget<Ph: Phase> {
    /// Animator itself. Mainly used for AAPs.
    AnimatorSelf(AnimatedAnimatorTarget<Ph>),

    /// GameObject.
    GameObject(AnimatedGameObjectTarget<Ph>),

    /// Renderer, especially MeshRenderer and SkinnedMeshRenderer.
    Renderer(AnimatedRendererTarget<Ph>),

    /// Other component.
    Component(AnimatedComponentTarget<Ph>),
}

impl<Ph: Phase> Target for AnimatedTarget<Ph> {
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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(bound(serialize = "Ph::ParameterRef: Serialize"))]
pub struct AnimatedAnimatorTarget<Ph: Phase> {
    pub property: AnimatedAnimatorProperty<Ph>,
}

/// Represents animated property on an Animator component.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(bound(serialize = "Ph::ParameterRef: Serialize"))]
pub enum AnimatedAnimatorProperty<Ph: Phase> {
    /// Float parameter value.
    ParameterFloatValue { name: Ph::ParameterRef },
}

impl<Ph: Phase> Target for AnimatedAnimatorTarget<Ph> {
    fn type_hint(&self) -> Option<AnimatedValueType> {
        match self.property {
            AnimatedAnimatorProperty::ParameterFloatValue { .. } => Some(AnimatedValueType::Float),
        }
    }
}

/// Target specifier object for GameObject properties.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(bound(serialize = "Ph::ObjectPath: Serialize"))]
pub struct AnimatedGameObjectTarget<Ph: Phase> {
    pub path: Ph::ObjectPath,
    pub property: AnimatedGameObjectProperty,
}

/// Represents animated property of a GameObject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
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

impl<Ph: Phase> Target for AnimatedGameObjectTarget<Ph> {
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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(bound(serialize = "Ph::ObjectPath: Serialize"))]
pub struct AnimatedRendererTarget<Ph: Phase> {
    pub path: Ph::ObjectPath,
    pub renderer_type: String,
    pub property: AnimatedRendererProperty,
}

/// Represents animated property of a Renderer.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum AnimatedRendererProperty {
    /// Enabled/disabled state.
    Enabled,

    /// Blend shape value.
    BlendShape { name: String },

    /// Material of the given slot.
    Material { slot: u32 },

    /// Material property value.
    MaterialProperty { name: String },

    /// Arbitrary serialized property value.
    Serialized { name: String },
}

impl<Ph: Phase> Target for AnimatedRendererTarget<Ph> {
    fn type_hint(&self) -> Option<AnimatedValueType> {
        match self.property {
            AnimatedRendererProperty::Enabled => Some(AnimatedValueType::Bool),
            AnimatedRendererProperty::BlendShape { .. } => Some(AnimatedValueType::Float),
            AnimatedRendererProperty::Material { .. } => Some(AnimatedValueType::ObjectReference),
            AnimatedRendererProperty::MaterialProperty { .. } | AnimatedRendererProperty::Serialized { .. } => None,
        }
    }
}

/// Target specifier object for arbitrary Unity components.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(bound(serialize = "Ph::ObjectPath: Serialize, Ph::ComponentType: Serialize"))]
pub struct AnimatedComponentTarget<Ph: Phase> {
    pub path: Ph::ObjectPath,
    pub component_type: Ph::ComponentType,
    pub property: AnimatedComponentProperty,
    pub value_type: AnimatedValueType,
}

/// Represents animated property of a Unity component.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum AnimatedComponentProperty {
    /// Enabled/disabled state.
    Enabled,

    /// Arbitrary serialized property value.
    Serialized { name: String },
}

impl<Ph: Phase> Target for AnimatedComponentTarget<Ph> {
    fn type_hint(&self) -> Option<AnimatedValueType> {
        match self.property {
            AnimatedComponentProperty::Enabled => Some(AnimatedValueType::Bool),
            AnimatedComponentProperty::Serialized { .. } => None,
        }
    }
}

/// How a generated controller is applied to the playable layer it is bound for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum MergeMode {
    /// The layers are appended to what the playable layer already has.
    #[default]
    Append,

    /// The controller replaces the playable layer as a whole.
    Replace,
}

/// What the object paths written in a controller are relative to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum PathMode {
    /// Paths start at the avatar root.
    #[default]
    Absolute,

    /// Paths start at a root the client supplies, so the controller can be applied anywhere in the hierarchy.
    Relative,
}

/// Represents a parameter of AnimatorControllers.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimatorParameter {
    pub name: String,
    pub value_type: AnimatorParameterType,
    pub default_value: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum AnimatorParameterType {
    Bool,
    Int,
    Float,
}

impl AnimatorParameterType {
    /// The animator parameter type that holds values of the given type, if any.
    pub fn of(value_type: AnimatedValueType) -> Option<Self> {
        match value_type {
            AnimatedValueType::Bool => Some(Self::Bool),
            AnimatedValueType::Int => Some(Self::Int),
            AnimatedValueType::Float => Some(Self::Float),
            _ => None,
        }
    }

    pub fn animated_value_type(&self) -> AnimatedValueType {
        match self {
            Self::Bool => AnimatedValueType::Bool,
            Self::Int => AnimatedValueType::Int,
            Self::Float => AnimatedValueType::Float,
        }
    }
}

/// How a blend tree places its fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum BlendTreeType {
    Linear,
    Simple2d,
    Freeform2d,
    Cartesian2d,
}

impl BlendTreeType {
    /// Whether this type blends by two parameters.
    pub fn is_two_dimensional(&self) -> bool {
        !matches!(self, BlendTreeType::Linear)
    }
}

impl AnimatorParameter {
    pub fn create_bool(name: impl Into<String>, default_value: Option<bool>) -> Self {
        Self {
            name: name.into(),
            value_type: AnimatorParameterType::Bool,
            default_value: default_value.map(|v| if v { 1.0 } else { 0.0 }),
        }
    }

    pub fn create_int(name: impl Into<String>, default_value: Option<i32>) -> Self {
        Self {
            name: name.into(),
            value_type: AnimatorParameterType::Int,
            default_value: default_value.map(|v| v as f32),
        }
    }

    pub fn create_float(name: impl Into<String>, default_value: Option<f32>) -> Self {
        Self {
            name: name.into(),
            value_type: AnimatorParameterType::Float,
            default_value,
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::{
        core::{
            external::ExternTable,
            phase::{Compiled, Declared},
            resolution::{Resolved, Unresolved},
        },
        unity::external::ObjectPath,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    enum TestPhase {}

    impl Phase for TestPhase {
        type ParameterRef = &'static str;
        type ObjectPath = &'static str;
        type ComponentType = &'static str;
        type ObjectRef = ();
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    enum IndexPhase {}

    impl Phase for IndexPhase {
        type ParameterRef = &'static str;
        type ObjectPath = u32;
        type ComponentType = u32;
        type ObjectRef = u32;
    }

    #[rstest]
    #[case(
        AnimatedTarget::<TestPhase>::AnimatorSelf(AnimatedAnimatorTarget {
            property: AnimatedAnimatorProperty::ParameterFloatValue {
                name: "GestureLeft",
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
            property: AnimatedRendererProperty::Material { slot: 0 },
        }),
        Some(AnimatedValueType::ObjectReference),
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
    fn animated_target_type_hints_match_expected(#[case] target: AnimatedTarget<TestPhase>, #[case] expected: Option<AnimatedValueType>) {
        assert_eq!(target.type_hint(), expected);
    }

    #[rstest]
    #[case(
        AnimatedAnimatorTarget::<TestPhase> {
            property: AnimatedAnimatorProperty::ParameterFloatValue {
                name: "GestureRight",
            },
        },
        Some(AnimatedValueType::Float),
    )]
    fn animator_target_reports_expected_type_hint(#[case] target: AnimatedAnimatorTarget<TestPhase>, #[case] expected: Option<AnimatedValueType>) {
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
    fn game_object_target_reports_expected_type_hint(#[case] target: AnimatedGameObjectTarget<TestPhase>, #[case] expected: Option<AnimatedValueType>) {
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
            property: AnimatedRendererProperty::Material { slot: 2 },
        },
        Some(AnimatedValueType::ObjectReference),
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
    fn renderer_target_reports_expected_type_hint(#[case] target: AnimatedRendererTarget<TestPhase>, #[case] expected: Option<AnimatedValueType>) {
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
    fn component_target_reports_expected_type_hint(#[case] target: AnimatedComponentTarget<TestPhase>, #[case] expected: Option<AnimatedValueType>) {
        assert_eq!(target.type_hint(), expected);
    }

    #[rstest]
    fn declared_targets_with_same_name_are_equal() {
        let first = AnimatedTarget::<Declared>::GameObject(AnimatedGameObjectTarget {
            path: Unresolved::new("Armature/Hips".into()),
            property: AnimatedGameObjectProperty::Active,
        });
        let second = AnimatedTarget::<Declared>::GameObject(AnimatedGameObjectTarget {
            path: Unresolved::new("Armature/Hips".into()),
            property: AnimatedGameObjectProperty::Active,
        });
        assert_eq!(first, second);
    }

    #[rstest]
    fn compiled_targets_serialize_references_as_index_and_name() {
        let mut paths = ExternTable::<ObjectPath>::new();
        paths.intern(Unresolved::new("Body".into()));
        let hips = paths.intern(Unresolved::new("Armature/Hips".into()));

        let game_object = AnimatedTarget::<Compiled>::GameObject(AnimatedGameObjectTarget {
            path: hips,
            property: AnimatedGameObjectProperty::Active,
        });
        let expected = AnimatedTarget::<IndexPhase>::GameObject(AnimatedGameObjectTarget {
            path: 1,
            property: AnimatedGameObjectProperty::Active,
        });
        assert_eq!(rmp_serde::to_vec_named(&game_object).unwrap(), rmp_serde::to_vec_named(&expected).unwrap());

        let animator_self = AnimatedTarget::<Compiled>::AnimatorSelf(AnimatedAnimatorTarget {
            property: AnimatedAnimatorProperty::ParameterFloatValue {
                name: Resolved::new("GestureLeft".into(), AnimatedValueType::Int),
            },
        });
        let expected = AnimatedTarget::<TestPhase>::AnimatorSelf(AnimatedAnimatorTarget {
            property: AnimatedAnimatorProperty::ParameterFloatValue { name: "GestureLeft" },
        });
        assert_eq!(rmp_serde::to_vec_named(&animator_self).unwrap(), rmp_serde::to_vec_named(&expected).unwrap());
    }
}
