use crate::{
    avatar::{self, controller::ParameterRef},
    core::resolution::{SourceLocation, Unresolved},
    decl::behavior::{Behavior, Drive},
    transform::{
        context::{Context, LayerInfo},
        error::{TransformError, TransformErrorKind},
    },
    unity::value::{AnimatedValue, AnimatedValueCast, AnimatedValueType},
    vrchat::state_behaviour::{ParameterDrive, ParameterDriveTarget},
};

impl Context {
    pub fn behaviors(&self, behaviors: &[Behavior]) -> Result<Vec<avatar::Behavior>, TransformError> {
        behaviors.iter().map(|behavior| self.behavior(behavior)).collect()
    }

    fn behavior(&self, behavior: &Behavior) -> Result<avatar::Behavior, TransformError> {
        Ok(match behavior {
            Behavior::Drive(drive) => {
                let (parameter, value) = self.drive(drive)?;
                avatar::Behavior::ParameterDrive(ParameterDrive {
                    target: ParameterDriveTarget::Set { parameter, value },
                })
            }
            Behavior::TrackingControl(tracking) => avatar::Behavior::TrackingControl(tracking.clone()),
            Behavior::Generic(generic) => avatar::Behavior::Generic(generic.clone()),
        })
    }

    /// The parameter and the value a drive sets, with a layer-targeting drive turned into the parameter of that layer.
    pub fn drive(&self, drive: &Drive) -> Result<(ParameterRef, AnimatedValue<()>), TransformError> {
        match drive {
            Drive::Group { layer, option } => {
                let info = self.layer(layer)?;
                let LayerInfo::Group { parameter, options } = info else {
                    return Err(kind_mismatch(layer, "group", info));
                };
                let index = options.get(option).ok_or_else(|| {
                    TransformErrorKind::UnknownOption {
                        layer: layer.value.clone(),
                        option: option.clone(),
                    }
                    .at(layer.at.clone())
                })?;
                let parameter = self.parameters.resolve_typed(parameter, AnimatedValueType::Int)?;
                Ok((parameter, AnimatedValue::Int(*index)))
            }
            Drive::Switch { layer, value } => {
                let info = self.layer(layer)?;
                let LayerInfo::Switch { source } = info else {
                    return Err(kind_mismatch(layer, "switch", info));
                };
                Ok((self.switch_parameter(source)?, AnimatedValue::Bool(value.unwrap_or(true))))
            }
            Drive::Puppet { layer, value } => {
                let info = self.layer(layer)?;
                let LayerInfo::Puppet { parameter } = info else {
                    return Err(kind_mismatch(layer, "puppet", info));
                };
                let parameter = self.parameters.resolve_typed(parameter, AnimatedValueType::Float)?;
                Ok((parameter, AnimatedValue::Float(value.unwrap_or(1.0))))
            }
            Drive::Parameter { parameter, value } => {
                let resolved = self.parameters.resolve(parameter)?;
                let value = cast(value, resolved.context).map_err(|error| error.or_at(parameter.at.as_ref()))?;
                Ok((resolved, value))
            }
        }
    }
}

/// Casts a written value to the type of the parameter it is for, accepting the lossless conversions `AnimatedValue::cast` knows.
pub(crate) fn cast(value: &AnimatedValue<()>, expected: AnimatedValueType) -> Result<AnimatedValue<()>, TransformError> {
    match value.cast(expected) {
        AnimatedValueCast::Same => Ok(value.clone()),
        AnimatedValueCast::Compatible(converted) => Ok(converted),
        AnimatedValueCast::Incompatible => Err(TransformErrorKind::ValueTypeMismatch {
            expected,
            found: value.value_type(),
        }
        .into()),
    }
}

/// Where a drive was written, for errors that have no better place to point at.
pub(crate) fn drive_location(drive: &Drive) -> Option<&SourceLocation> {
    match drive {
        Drive::Group { layer, .. } | Drive::Switch { layer, .. } | Drive::Puppet { layer, .. } => layer.at.as_ref(),
        Drive::Parameter { parameter, .. } => parameter.at.as_ref(),
    }
}

fn kind_mismatch(layer: &Unresolved<String>, expected: &'static str, found: &LayerInfo) -> TransformError {
    TransformErrorKind::LayerKindMismatch {
        name: layer.value.clone(),
        expected,
        found: found.kind(),
    }
    .at(layer.at.clone())
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::{
        core::resolution::Resolved,
        decl::{
            Avatar,
            avatar::Export,
            behavior::Content,
            controller::Controller,
            layer::{GroupLayer, GroupOption, Layer, PuppetLayer, SwitchContent, SwitchLayer, SwitchSource},
            parameter::{Parameter, PrimitiveParameter, PrimitiveParameterValue},
        },
        unity::state::GenericStateBehavior,
        vrchat::playable_layer::PlayableLayer,
        vrchat::state_behaviour::{TrackingControl, TrackingControlMode, TrackingControlTarget},
    };

    fn at(line: u32) -> SourceLocation {
        SourceLocation {
            chunk: "avatar.lua".into(),
            line,
        }
    }

    fn located(name: &str, line: u32) -> Unresolved<String> {
        Unresolved::located(name.into(), at(line))
    }

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
                parameter("Weight", PrimitiveParameterValue::Float { default: None, width: None }),
            ],
            controllers: vec![Controller::new(
                PlayableLayer::Fx,
                vec![
                    Layer::Group(GroupLayer {
                        name: "Expressions".into(),
                        driven_by: Some("Emote".to_owned().into()),
                        symmetric: None,
                        default: None,
                        options: ["smile", "angry"]
                            .map(|name| GroupOption {
                                name: name.into(),
                                content: Content::new(),
                                at: None,
                            })
                            .into(),
                        at: None,
                    }),
                    Layer::Switch(SwitchLayer {
                        name: "Hat".into(),
                        source: Some(SwitchSource::Parameter("Hat".to_owned().into())),
                        content: SwitchContent::Toggle(Content::new()),
                        at: None,
                    }),
                    Layer::Switch(SwitchLayer {
                        name: "Gated".into(),
                        source: Some(SwitchSource::Gate("HatShown".to_owned().into())),
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
            exports: vec![Export::Gate {
                name: "HatShown".into(),
                at: None,
            }],
            ..Avatar::default()
        };
        let (context, errors) = Context::collect(&declaration);
        assert!(errors.is_empty(), "{errors:?}");
        context
    }

    fn resolved(name: &str, value_type: AnimatedValueType) -> ParameterRef {
        Resolved::new(name.into(), value_type)
    }

    #[rstest]
    #[case::group(
        Drive::Group { layer: "Expressions".to_owned().into(), option: "angry".into() },
        (resolved("Emote", AnimatedValueType::Int), AnimatedValue::Int(2)),
    )]
    #[case::switch_on(Drive::Switch { layer: "Hat".to_owned().into(), value: None }, (resolved("Hat", AnimatedValueType::Bool), AnimatedValue::Bool(true)))]
    #[case::switch_off(Drive::Switch { layer: "Hat".to_owned().into(), value: Some(false) }, (resolved("Hat", AnimatedValueType::Bool), AnimatedValue::Bool(false)))]
    #[case::gated(Drive::Switch { layer: "Gated".to_owned().into(), value: None }, (resolved("HatShown", AnimatedValueType::Bool), AnimatedValue::Bool(true)))]
    #[case::puppet(Drive::Puppet { layer: "Wink".to_owned().into(), value: Some(0.25) }, (resolved("Wink", AnimatedValueType::Float), AnimatedValue::Float(0.25)))]
    #[case::puppet_full(Drive::Puppet { layer: "Wink".to_owned().into(), value: None }, (resolved("Wink", AnimatedValueType::Float), AnimatedValue::Float(1.0)))]
    #[case::parameter(
        Drive::Parameter { parameter: "Emote".to_owned().into(), value: AnimatedValue::Int(4) },
        (resolved("Emote", AnimatedValueType::Int), AnimatedValue::Int(4)),
    )]
    #[case::converted(
        Drive::Parameter { parameter: "Weight".to_owned().into(), value: AnimatedValue::Int(1) },
        (resolved("Weight", AnimatedValueType::Float), AnimatedValue::Float(1.0)),
    )]
    fn drives_resolve_to_a_parameter_and_a_value(#[case] drive: Drive, #[case] expected: (ParameterRef, AnimatedValue<()>)) {
        assert_eq!(context().drive(&drive).unwrap(), expected);
    }

    #[rstest]
    #[case::unknown_layer(
        Drive::Switch { layer: located("Missing", 3), value: None },
        TransformErrorKind::UnknownLayer { name: "Missing".into() },
    )]
    #[case::wrong_kind(
        Drive::Group { layer: located("Hat", 3), option: "on".into() },
        TransformErrorKind::LayerKindMismatch { name: "Hat".into(), expected: "group", found: "switch" },
    )]
    #[case::unknown_option(
        Drive::Group { layer: located("Expressions", 3), option: "sad".into() },
        TransformErrorKind::UnknownOption { layer: "Expressions".into(), option: "sad".into() },
    )]
    #[case::unknown_parameter(
        Drive::Parameter { parameter: located("Missing", 3), value: AnimatedValue::Bool(true) },
        TransformErrorKind::UnknownParameter { name: "Missing".into() },
    )]
    #[case::wrong_value(
        Drive::Parameter { parameter: located("Hat", 3), value: AnimatedValue::Vector2([0.0, 0.0].into()) },
        TransformErrorKind::ValueTypeMismatch { expected: AnimatedValueType::Bool, found: AnimatedValueType::Vector2 },
    )]
    fn a_bad_drive_is_reported_where_it_was_written(#[case] drive: Drive, #[case] expected: TransformErrorKind) {
        assert_eq!(context().drive(&drive).unwrap_err(), expected.at(Some(at(3))));
    }

    #[rstest]
    fn behaviors_keep_their_order_and_pass_the_typed_ones_through() {
        let tracking = TrackingControl {
            values: [(TrackingControlTarget::Mouth, TrackingControlMode::Animation)].into(),
        };
        let generic = GenericStateBehavior {
            type_name: "Custom".into(),
            fields: Default::default(),
        };
        let compiled = context()
            .behaviors(&[
                Behavior::TrackingControl(tracking.clone()),
                Behavior::Drive(Drive::Switch {
                    layer: "Hat".to_owned().into(),
                    value: None,
                }),
                Behavior::Generic(generic.clone()),
            ])
            .unwrap();

        assert_eq!(
            compiled,
            vec![
                avatar::Behavior::TrackingControl(tracking),
                avatar::Behavior::ParameterDrive(ParameterDrive {
                    target: ParameterDriveTarget::Set {
                        parameter: resolved("Hat", AnimatedValueType::Bool),
                        value: AnimatedValue::Bool(true),
                    },
                }),
                avatar::Behavior::Generic(generic),
            ]
        );
    }
}
