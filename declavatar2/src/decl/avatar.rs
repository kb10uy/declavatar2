use crate::decl::{controller::Controller, layer::Layer, menu::MenuItem, parameter::Parameter};

/// Root of a declaration, what one script returns.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Avatar {
    pub parameters: Vec<Parameter>,
    pub controllers: Vec<Controller>,
    pub menu: Vec<MenuItem>,
}

impl Avatar {
    /// Every layer of every controller, in the order they are written.
    pub fn layers(&self) -> impl Iterator<Item = &Layer> {
        self.controllers.iter().flat_map(|controller| controller.layers.iter())
    }
}

#[cfg(test)]
mod tests {
    use rstest::*;

    use super::*;
    use crate::{
        core::{phase::Declared, resolution::Unresolved},
        decl::{
            behavior::{Animation, Content, Drive},
            layer::{GroupLayer, GroupOption, SwitchContent, SwitchLayer},
            parameter::{Parameter, PrimitiveParameter, PrimitiveParameterValue, ProvidedParameterGroup},
        },
        unity::{
            animation::FixedAnimationEntry,
            animator::{AnimatedGameObjectProperty, AnimatedGameObjectTarget, AnimatedRendererProperty, AnimatedRendererTarget, AnimatedTarget},
            value::AnimatedValue,
        },
        vrchat::playable_layer::PlayableLayer,
    };

    fn shape(path: &str, name: &str, value: f64) -> FixedAnimationEntry<Declared> {
        FixedAnimationEntry {
            key: AnimatedTarget::Renderer(AnimatedRendererTarget {
                path: Unresolved::new(path.into()),
                renderer_type: "UnityEngine.SkinnedMeshRenderer".into(),
                property: AnimatedRendererProperty::BlendShape { name: name.into() },
            }),
            value: AnimatedValue::Float(value),
        }
    }

    fn active(path: &str, value: bool) -> FixedAnimationEntry<Declared> {
        FixedAnimationEntry {
            key: AnimatedTarget::GameObject(AnimatedGameObjectTarget {
                path: Unresolved::new(path.into()),
                property: AnimatedGameObjectProperty::Active,
            }),
            value: AnimatedValue::Bool(value),
        }
    }

    fn example_avatar() -> Avatar {
        Avatar {
            parameters: vec![
                Parameter::Provided(ProvidedParameterGroup::Vrchat),
                Parameter::Primitive(PrimitiveParameter {
                    name: "Emote".into(),
                    value: PrimitiveParameterValue::Int {
                        default: Some(42),
                        width: None,
                    },
                    scope: None,
                    save: None,
                    at: None,
                }),
                Parameter::Primitive(PrimitiveParameter {
                    name: "Hat".into(),
                    value: PrimitiveParameterValue::Bool { default: None },
                    scope: Some(crate::decl::parameter::ParameterScope::Local),
                    save: None,
                    at: None,
                }),
            ],
            controllers: vec![Controller::new(
                PlayableLayer::Fx,
                vec![
                    Layer::Group(GroupLayer {
                        name: "Expressions".into(),
                        driven_by: Some(Unresolved::new("Emote".into())),
                        symmetric: None,
                        default: Some(Content {
                            animation: Animation::from([shape("Face", "eyelid_L", 0.3)]),
                            behaviors: vec![],
                        }),
                        options: vec![GroupOption {
                            name: "smile".into(),
                            content: Content {
                                animation: Animation::from([shape("Face", "smile", 1.0), shape("Face", "eye_joy", 0.5)]),
                                behaviors: vec![],
                            },
                            at: None,
                        }],
                        at: None,
                    }),
                    Layer::Switch(SwitchLayer {
                        name: "Hat".into(),
                        driven_by: Some(Unresolved::new("Hat".into())),
                        content: SwitchContent::Toggle(Content {
                            animation: Animation::from([active("Hat", true)]),
                            behaviors: vec![],
                        }),
                        at: None,
                    }),
                ],
            )],
            menu: vec![MenuItem::Toggle {
                name: "Hat".into(),
                drive: Drive::Switch {
                    layer: Unresolved::new("Hat".into()),
                    value: None,
                },
            }],
        }
    }

    #[rstest]
    fn example_declaration_is_expressible() {
        let avatar = example_avatar();

        assert_eq!(avatar.parameters.len(), 3);
        assert_eq!(avatar.controllers.len(), 1);
        assert_eq!(avatar.layers().map(Layer::name).collect::<Vec<_>>(), ["Expressions", "Hat"]);
        assert_eq!(avatar.menu.iter().map(MenuItem::name).collect::<Vec<_>>(), ["Hat"]);

        let Layer::Group(group) = &avatar.controllers[0].layers[0] else {
            panic!("first layer should be a group layer");
        };
        assert_eq!(group.options[0].content.animation.entries().count(), 2);
    }

    #[rstest]
    fn animation_keeps_one_entry_per_target() {
        let animation = Animation::from([shape("Face", "smile", 0.3), shape("Face", "smile", 0.7)]);

        let values: Vec<_> = animation.entries().map(|(_, entry)| entry.value.clone()).collect();
        assert_eq!(values, [AnimatedValue::Float(0.7)]);
    }
}
