use crate::{
    avatar::menu::{MenuAxis, MenuItem},
    decl::{
        behavior::Drive,
        menu::{self, Axis, AxisTarget},
    },
    transform::{
        behavior::drive_location,
        context::{Context, LayerInfo},
        error::{TransformError, TransformErrorKind},
    },
    unity::value::AnimatedValueType,
};

/// Compiles the menu block. An item that fails is dropped and its error recorded, so every item gets checked.
pub(crate) fn compile(context: &Context, items: &[menu::MenuItem], errors: &mut Vec<TransformError>) -> Vec<MenuItem> {
    self::items(context, "root", items, errors)
}

fn items(context: &Context, owner: &str, items: &[menu::MenuItem], errors: &mut Vec<TransformError>) -> Vec<MenuItem> {
    if items.len() > MenuItem::CAPACITY {
        errors.push(
            TransformErrorKind::MenuTooLarge {
                name: owner.to_owned(),
                count: items.len(),
                capacity: MenuItem::CAPACITY,
            }
            .into(),
        );
    }
    items
        .iter()
        .filter_map(|item| match self::item(context, item, errors) {
            Ok(compiled) => Some(compiled),
            Err(error) => {
                errors.push(error);
                None
            }
        })
        .collect()
}

fn item(context: &Context, item: &menu::MenuItem, errors: &mut Vec<TransformError>) -> Result<MenuItem, TransformError> {
    Ok(match item {
        menu::MenuItem::SubMenu { name, items: children } => MenuItem::SubMenu {
            name: name.clone(),
            items: items(context, name, children, errors),
        },
        menu::MenuItem::Toggle { name, drive } => {
            let (parameter, value) = context.drive(drive)?;
            MenuItem::Toggle {
                name: name.clone(),
                parameter,
                value,
            }
        }
        menu::MenuItem::Button { name, drive } => {
            let (parameter, value) = context.drive(drive)?;
            MenuItem::Button {
                name: name.clone(),
                parameter,
                value,
            }
        }
        menu::MenuItem::Radial { name, axis } => MenuItem::Radial {
            name: name.clone(),
            axis: self::axis(context, axis)?,
        },
        menu::MenuItem::TwoAxis { name, axes } => MenuItem::TwoAxis {
            name: name.clone(),
            horizontal: axis(context, &axes.horizontal)?,
            vertical: axis(context, &axes.vertical)?,
        },
        menu::MenuItem::FourAxis { name, axes } => MenuItem::FourAxis {
            name: name.clone(),
            up: axis(context, &axes.up)?,
            down: axis(context, &axes.down)?,
            left: axis(context, &axes.left)?,
            right: axis(context, &axes.right)?,
        },
    })
}

fn axis(context: &Context, axis: &Axis) -> Result<MenuAxis, TransformError> {
    let parameter = match &axis.target {
        AxisTarget::Parameter(parameter) => context.parameters.resolve_typed(parameter, AnimatedValueType::Float)?,
        AxisTarget::Drive(Drive::Puppet { layer, value: None }) => {
            let info = context.layer(layer)?;
            let LayerInfo::Puppet { parameter } = info else {
                return Err(TransformErrorKind::LayerKindMismatch {
                    name: layer.value.clone(),
                    expected: "puppet",
                    found: info.kind(),
                }
                .at(layer.at.clone()));
            };
            context.parameters.resolve_typed(parameter, AnimatedValueType::Float)?
        }
        AxisTarget::Drive(drive) => return Err(TransformErrorKind::InvalidAxis.at(drive_location(drive).cloned())),
    };
    Ok(MenuAxis {
        parameter,
        positive: axis.positive.clone(),
        negative: axis.negative.clone(),
    })
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::{
        avatar::controller::ParameterRef,
        core::resolution::{Resolved, Unresolved},
        decl::{
            Avatar,
            behavior::Content,
            controller::Controller,
            layer::{GroupLayer, GroupOption, Layer, PuppetLayer, SwitchContent, SwitchLayer},
            menu::{FourAxes, TwoAxes},
            parameter::{Parameter, PrimitiveParameter, PrimitiveParameterValue},
        },
        transform::layer::located_at,
        unity::value::AnimatedValue,
        vrchat::playable_layer::PlayableLayer,
    };

    fn parameter(name: &str, value: PrimitiveParameterValue) -> Parameter {
        Parameter::Primitive(PrimitiveParameter {
            name: name.into(),
            value,
            scope: None,
            save: None,
            at: None,
        })
    }

    fn context() -> Context {
        let declaration = Avatar {
            parameters: vec![
                parameter("Emote", PrimitiveParameterValue::Int { default: None, width: None }),
                parameter("Hat", PrimitiveParameterValue::Bool { default: None }),
                parameter("Wink", PrimitiveParameterValue::Float { default: None, width: None }),
                parameter("Move", PrimitiveParameterValue::Float { default: None, width: None }),
            ],
            controllers: vec![Controller::new(
                PlayableLayer::Fx,
                vec![
                    Layer::Group(GroupLayer {
                        name: "Expressions".into(),
                        driven_by: Some("Emote".to_owned().into()),
                        symmetric: None,
                        default: None,
                        options: vec![GroupOption {
                            name: "smile".into(),
                            content: Content::new(),
                            at: None,
                        }],
                        at: None,
                    }),
                    Layer::Switch(SwitchLayer {
                        name: "Hat".into(),
                        driven_by: None,
                        content: SwitchContent::Toggle(Content::new()),
                        at: None,
                    }),
                    Layer::Puppet(PuppetLayer {
                        name: "Wink".into(),
                        driven_by: None,
                        keyframes: vec![],
                        at: None,
                    }),
                ],
            )],
            ..Avatar::default()
        };
        Context::collect(&declaration).0
    }

    fn resolved(name: &str, value_type: AnimatedValueType) -> ParameterRef {
        Resolved::new(name.into(), value_type)
    }

    fn run(items: Vec<menu::MenuItem>) -> (Vec<MenuItem>, Vec<TransformError>) {
        let mut errors = Vec::new();
        let compiled = compile(&context(), &items, &mut errors);
        (compiled, errors)
    }

    #[rstest]
    fn controls_resolve_their_drives_and_axes() {
        let (compiled, errors) = run(vec![
            menu::MenuItem::Toggle {
                name: "Hat".into(),
                drive: Drive::Switch {
                    layer: "Hat".to_owned().into(),
                    value: None,
                },
            },
            menu::MenuItem::Button {
                name: "Smile".into(),
                drive: Drive::Group {
                    layer: "Expressions".to_owned().into(),
                    option: "smile".into(),
                },
            },
            menu::MenuItem::Radial {
                name: "Wink".into(),
                axis: Box::new(Axis::bare(AxisTarget::Drive(Drive::Puppet {
                    layer: "Wink".to_owned().into(),
                    value: None,
                }))),
            },
            menu::MenuItem::SubMenu {
                name: "More".into(),
                items: vec![menu::MenuItem::TwoAxis {
                    name: "Move".into(),
                    axes: Box::new(TwoAxes {
                        horizontal: Axis {
                            target: AxisTarget::Parameter("Move".to_owned().into()),
                            positive: Some("Right".into()),
                            negative: Some("Left".into()),
                        },
                        vertical: Axis::bare(AxisTarget::Parameter("Wink".to_owned().into())),
                    }),
                }],
            },
        ]);

        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(
            compiled,
            vec![
                MenuItem::Toggle {
                    name: "Hat".into(),
                    parameter: resolved("Hat", AnimatedValueType::Bool),
                    value: AnimatedValue::Bool(true),
                },
                MenuItem::Button {
                    name: "Smile".into(),
                    parameter: resolved("Emote", AnimatedValueType::Int),
                    value: AnimatedValue::Int(1),
                },
                MenuItem::Radial {
                    name: "Wink".into(),
                    axis: MenuAxis {
                        parameter: resolved("Wink", AnimatedValueType::Float),
                        positive: None,
                        negative: None,
                    },
                },
                MenuItem::SubMenu {
                    name: "More".into(),
                    items: vec![MenuItem::TwoAxis {
                        name: "Move".into(),
                        horizontal: MenuAxis {
                            parameter: resolved("Move", AnimatedValueType::Float),
                            positive: Some("Right".into()),
                            negative: Some("Left".into()),
                        },
                        vertical: MenuAxis {
                            parameter: resolved("Wink", AnimatedValueType::Float),
                            positive: None,
                            negative: None,
                        },
                    }],
                },
            ]
        );
    }

    #[rstest]
    fn a_four_axis_control_names_every_direction() {
        let axis = |name: &str| Axis {
            target: AxisTarget::Parameter("Move".to_owned().into()),
            positive: Some(name.into()),
            negative: None,
        };
        let (compiled, errors) = run(vec![menu::MenuItem::FourAxis {
            name: "Move".into(),
            axes: Box::new(FourAxes {
                up: axis("U"),
                down: axis("D"),
                left: axis("L"),
                right: axis("R"),
            }),
        }]);

        assert!(errors.is_empty(), "{errors:?}");
        let MenuItem::FourAxis { up, down, left, right, .. } = &compiled[0] else {
            panic!("expected a four-axis control");
        };
        assert_eq!(
            [up, down, left, right].map(|axis| axis.positive.clone().unwrap()),
            ["U", "D", "L", "R"].map(str::to_owned)
        );
    }

    #[rstest]
    #[case::int_axis(
        AxisTarget::Parameter(Unresolved::located("Emote".into(), located_at(3).unwrap())),
        TransformErrorKind::ParameterTypeMismatch { name: "Emote".into(), expected: AnimatedValueType::Float, found: AnimatedValueType::Int },
    )]
    #[case::valued_drive(
        AxisTarget::Drive(Drive::Puppet { layer: Unresolved::located("Wink".into(), located_at(3).unwrap()), value: Some(0.5) }),
        TransformErrorKind::InvalidAxis,
    )]
    #[case::switch_drive(
        AxisTarget::Drive(Drive::Switch { layer: Unresolved::located("Hat".into(), located_at(3).unwrap()), value: None }),
        TransformErrorKind::InvalidAxis,
    )]
    #[case::wrong_layer(
        AxisTarget::Drive(Drive::Puppet { layer: Unresolved::located("Hat".into(), located_at(3).unwrap()), value: None }),
        TransformErrorKind::LayerKindMismatch { name: "Hat".into(), expected: "puppet", found: "switch" },
    )]
    fn a_bad_axis_is_reported(#[case] target: AxisTarget, #[case] expected: TransformErrorKind) {
        let (compiled, errors) = run(vec![menu::MenuItem::Radial {
            name: "Bad".into(),
            axis: Box::new(Axis::bare(target)),
        }]);
        assert!(compiled.is_empty());
        assert_eq!(errors, vec![expected.at(located_at(3))]);
    }

    #[rstest]
    fn every_bad_item_is_reported_and_the_good_ones_are_kept() {
        let bad = |line: u32| menu::MenuItem::Toggle {
            name: "Bad".into(),
            drive: Drive::Switch {
                layer: Unresolved::located("Missing".into(), located_at(line).unwrap()),
                value: None,
            },
        };
        let good = menu::MenuItem::Toggle {
            name: "Hat".into(),
            drive: Drive::Switch {
                layer: "Hat".to_owned().into(),
                value: None,
            },
        };
        let (compiled, errors) = run(vec![
            bad(1),
            good.clone(),
            menu::MenuItem::SubMenu {
                name: "Sub".into(),
                items: vec![bad(2), good],
            },
        ]);

        assert_eq!(
            errors,
            vec![
                TransformErrorKind::UnknownLayer { name: "Missing".into() }.at(located_at(1)),
                TransformErrorKind::UnknownLayer { name: "Missing".into() }.at(located_at(2)),
            ]
        );
        assert_eq!(compiled.len(), 2);
        let MenuItem::SubMenu { items, .. } = &compiled[1] else {
            panic!("expected a submenu");
        };
        assert_eq!(items.len(), 1);
    }

    #[rstest]
    fn a_menu_holds_eight_controls_at_most() {
        let toggle = menu::MenuItem::Toggle {
            name: "Hat".into(),
            drive: Drive::Switch {
                layer: "Hat".to_owned().into(),
                value: None,
            },
        };
        let (compiled, errors) = run(vec![menu::MenuItem::SubMenu {
            name: "Sub".into(),
            items: vec![toggle; 9],
        }]);

        assert_eq!(
            errors,
            vec![
                TransformErrorKind::MenuTooLarge {
                    name: "Sub".into(),
                    count: 9,
                    capacity: 8,
                }
                .into()
            ]
        );
        assert_eq!(compiled.len(), 1);
    }
}
