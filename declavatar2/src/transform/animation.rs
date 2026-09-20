use crate::{
    core::{
        phase::{Compiled, Declared},
        resolution::Unresolved,
        value_set::ValueSet,
    },
    decl::behavior::Animation,
    transform::{context::Context, error::TransformError},
    unity::{
        animation::FixedAnimationEntry,
        animator::{
            AnimatedAnimatorProperty, AnimatedAnimatorTarget, AnimatedComponentProperty, AnimatedComponentTarget, AnimatedGameObjectProperty,
            AnimatedGameObjectTarget, AnimatedRendererProperty, AnimatedRendererTarget, AnimatedTarget,
        },
        external::AssetLocator,
        value::{AnimatedValue, AnimatedValueType},
    },
};

pub(crate) type CompiledAnimation = ValueSet<FixedAnimationEntry<Compiled>>;
pub(crate) type CompiledValue = AnimatedValue<<Compiled as crate::core::phase::Phase>::ObjectRef>;

impl Context {
    /// Interns the Unity-side references of a target and resolves the animator parameter it may name.
    pub fn target(&mut self, target: &AnimatedTarget<Declared>) -> Result<AnimatedTarget<Compiled>, TransformError> {
        Ok(match target {
            AnimatedTarget::AnimatorSelf(animator) => AnimatedTarget::AnimatorSelf(AnimatedAnimatorTarget {
                property: match &animator.property {
                    AnimatedAnimatorProperty::ParameterFloatValue { name } => AnimatedAnimatorProperty::ParameterFloatValue {
                        name: self.parameters.resolve_typed(name, AnimatedValueType::Float)?,
                    },
                },
            }),
            AnimatedTarget::GameObject(object) => AnimatedTarget::GameObject(AnimatedGameObjectTarget {
                path: self.externals.object_paths.intern(object.path.clone()),
                property: object.property,
            }),
            AnimatedTarget::Renderer(renderer) => AnimatedTarget::Renderer(AnimatedRendererTarget {
                path: self.externals.object_paths.intern(renderer.path.clone()),
                renderer_type: renderer.renderer_type.clone(),
                property: renderer.property.clone(),
            }),
            AnimatedTarget::Component(component) => AnimatedTarget::Component(AnimatedComponentTarget {
                path: self.externals.object_paths.intern(component.path.clone()),
                component_type: self.externals.component_types.intern(component.component_type.clone()),
                property: component.property.clone(),
                value_type: component.value_type,
            }),
        })
    }

    pub fn value(&mut self, value: &AnimatedValue<Unresolved<AssetLocator>>) -> CompiledValue {
        value.clone().map_reference(|asset| self.externals.assets.intern(asset))
    }

    pub fn animation(&mut self, animation: &Animation) -> Result<CompiledAnimation, TransformError> {
        let mut compiled = ValueSet::new();
        for (_, entry) in animation.entries() {
            compiled.insert(FixedAnimationEntry {
                key: self.target(&entry.key)?,
                value: self.value(&entry.value),
            });
        }
        Ok(compiled)
    }
}

/// Names a target the way a script writer would recognize it, for error messages.
pub(crate) fn describe(target: &AnimatedTarget<Declared>) -> String {
    match target {
        AnimatedTarget::AnimatorSelf(animator) => match &animator.property {
            AnimatedAnimatorProperty::ParameterFloatValue { name } => format!("animator parameter `{}`", name.value),
        },
        AnimatedTarget::GameObject(object) => {
            let property = match object.property {
                AnimatedGameObjectProperty::Active => "active",
                AnimatedGameObjectProperty::TransformPosition => "position",
                AnimatedGameObjectProperty::TransformRotationQuaternion | AnimatedGameObjectProperty::TransformRotationEuler => "rotation",
                AnimatedGameObjectProperty::TransformScale => "scale",
            };
            format!("`{}` {property}", object.path.value)
        }
        AnimatedTarget::Renderer(renderer) => {
            let property = match &renderer.property {
                AnimatedRendererProperty::Enabled => "enabled".to_owned(),
                AnimatedRendererProperty::BlendShape { name } => format!("shape `{name}`"),
                AnimatedRendererProperty::Material { slot } => format!("material {slot}"),
                AnimatedRendererProperty::MaterialProperty { name } => format!("material property `{name}`"),
                AnimatedRendererProperty::Serialized { name } => format!("field `{name}`"),
            };
            format!("`{}` {property}", renderer.path.value)
        }
        AnimatedTarget::Component(component) => {
            let property = match &component.property {
                AnimatedComponentProperty::Enabled => "enabled".to_owned(),
                AnimatedComponentProperty::Serialized { name } => format!("field `{name}`"),
            };
            format!("`{}` {} {property}", component.path.value, component.component_type.value)
        }
    }
}

pub(crate) fn describe_all<'a>(targets: impl IntoIterator<Item = &'a AnimatedTarget<Declared>>) -> String {
    targets.into_iter().map(describe).collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::{
        core::resolution::{Resolved, SourceLocation},
        decl::Avatar,
        decl::parameter::{Parameter, PrimitiveParameter, PrimitiveParameterValue},
        transform::error::TransformErrorKind,
    };

    fn at(line: u32) -> SourceLocation {
        SourceLocation {
            chunk: "avatar.lua".into(),
            line,
        }
    }

    fn context() -> Context {
        let declaration = Avatar {
            parameters: vec![Parameter::Primitive(PrimitiveParameter {
                name: "Blend".into(),
                value: PrimitiveParameterValue::Float { default: None, width: None },
                scope: None,
                save: None,
                at: None,
            })],
            ..Avatar::default()
        };
        Context::collect(&declaration).0
    }

    fn shape(path: &str, name: &str, line: u32) -> AnimatedTarget<Declared> {
        AnimatedTarget::Renderer(AnimatedRendererTarget {
            path: Unresolved::located(path.into(), at(line)),
            renderer_type: "UnityEngine.SkinnedMeshRenderer".into(),
            property: AnimatedRendererProperty::BlendShape { name: name.into() },
        })
    }

    #[rstest]
    fn paths_are_interned_once_and_remember_every_line() {
        let mut context = context();
        let first = context.target(&shape("Face", "smile", 3)).unwrap();
        let second = context.target(&shape("Face", "angry", 5)).unwrap();
        let other = context
            .target(&AnimatedTarget::GameObject(AnimatedGameObjectTarget {
                path: Unresolved::located("Hat".into(), at(7)),
                property: AnimatedGameObjectProperty::Active,
            }))
            .unwrap();

        let (AnimatedTarget::Renderer(first), AnimatedTarget::Renderer(second), AnimatedTarget::GameObject(other)) = (first, second, other) else {
            panic!("target kinds should be kept");
        };
        assert_eq!(first.path, second.path);
        assert_ne!(first.path, other.path);
        assert_eq!(context.externals.object_paths.len(), 2);
        assert_eq!(context.externals.object_paths.get(first.path).referenced_at, vec![at(3), at(5)]);
        assert_eq!(context.externals.object_paths.get(other.path).value, "Hat");
    }

    #[rstest]
    fn components_intern_their_type_as_well() {
        let mut context = context();
        let target = context
            .target(&AnimatedTarget::Component(AnimatedComponentTarget {
                path: Unresolved::new("Root".into()),
                component_type: Unresolved::located("UnityEngine.Light".into(), at(2)),
                property: AnimatedComponentProperty::Enabled,
                value_type: AnimatedValueType::Bool,
            }))
            .unwrap();

        let AnimatedTarget::Component(component) = target else {
            panic!("a component should stay a component");
        };
        assert_eq!(context.externals.component_types.get(component.component_type).value, "UnityEngine.Light");
        assert_eq!(context.externals.component_types.get(component.component_type).referenced_at, vec![at(2)]);
    }

    #[rstest]
    fn an_animator_parameter_target_resolves_a_float_parameter() {
        let mut context = context();
        let target = context
            .target(&AnimatedTarget::AnimatorSelf(AnimatedAnimatorTarget {
                property: AnimatedAnimatorProperty::ParameterFloatValue {
                    name: Unresolved::new("Blend".into()),
                },
            }))
            .unwrap();

        assert_eq!(
            target,
            AnimatedTarget::AnimatorSelf(AnimatedAnimatorTarget {
                property: AnimatedAnimatorProperty::ParameterFloatValue {
                    name: Resolved::new("Blend".into(), AnimatedValueType::Float),
                },
            })
        );

        let error = context
            .target(&AnimatedTarget::AnimatorSelf(AnimatedAnimatorTarget {
                property: AnimatedAnimatorProperty::ParameterFloatValue {
                    name: Unresolved::located("Missing".into(), at(9)),
                },
            }))
            .unwrap_err();
        assert_eq!(error, TransformErrorKind::UnknownParameter { name: "Missing".into() }.at(Some(at(9))));
    }

    #[rstest]
    fn object_references_are_interned_as_assets() {
        let mut context = context();
        let locator = AssetLocator::Named {
            asset_type: "UnityEngine.Material".into(),
            name: "Skin".into(),
        };
        let value = context.value(&AnimatedValue::ObjectReference(Unresolved::located(locator.clone(), at(4))));
        let again = context.value(&AnimatedValue::ObjectReference(Unresolved::new(locator.clone())));

        assert_eq!(value, again);
        let AnimatedValue::ObjectReference(asset) = value else {
            panic!("a reference should stay a reference");
        };
        assert_eq!(context.externals.assets.get(asset).value, locator);
        assert_eq!(context.externals.assets.get(asset).referenced_at, vec![at(4)]);
        assert_eq!(context.value(&AnimatedValue::Float(0.5)), AnimatedValue::Float(0.5));
    }

    #[rstest]
    #[case::shape(shape("Face", "smile", 1), "`Face` shape `smile`")]
    #[case::active(
        AnimatedTarget::GameObject(AnimatedGameObjectTarget {
            path: Unresolved::new("Hat".into()),
            property: AnimatedGameObjectProperty::Active,
        }),
        "`Hat` active",
    )]
    #[case::material(
        AnimatedTarget::Renderer(AnimatedRendererTarget {
            path: Unresolved::new("Body".into()),
            renderer_type: "UnityEngine.SkinnedMeshRenderer".into(),
            property: AnimatedRendererProperty::Material { slot: 2 },
        }),
        "`Body` material 2",
    )]
    #[case::component(
        AnimatedTarget::Component(AnimatedComponentTarget {
            path: Unresolved::new("Root".into()),
            component_type: Unresolved::new("UnityEngine.Light".into()),
            property: AnimatedComponentProperty::Serialized { name: "m_Intensity".into() },
            value_type: AnimatedValueType::Float,
        }),
        "`Root` UnityEngine.Light field `m_Intensity`",
    )]
    #[case::animator(
        AnimatedTarget::AnimatorSelf(AnimatedAnimatorTarget {
            property: AnimatedAnimatorProperty::ParameterFloatValue { name: Unresolved::new("Blend".into()) },
        }),
        "animator parameter `Blend`",
    )]
    fn targets_are_described_for_messages(#[case] target: AnimatedTarget<Declared>, #[case] expected: &str) {
        assert_eq!(describe(&target), expected);
    }
}
