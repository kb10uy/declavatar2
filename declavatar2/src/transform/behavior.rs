use crate::{
    avatar::{
        self,
        controller::{LayerRef, ParameterRef},
    },
    core::{
        phase::{Compiled, Declared},
        resolution::{SourceLocation, Unresolved},
    },
    decl::behavior::{Behavior, Drive},
    transform::{
        context::{Context, LayerInfo},
        error::{TransformError, TransformErrorKind},
    },
    unity::{
        state::GenericStateBehavior,
        value::{AnimatedValue, AnimatedValueCast, AnimatedValueType},
    },
    vrchat::state_behaviour::{AudioSetting, BlendablePlayable, LayerControl, ParameterDrive, ParameterDriveTarget, PlayAudio, PlaybackOrder},
};

/// Type names that `da.behavior` must not carry, because the layer index they take is only known to the client.
const LAYER_CONTROL_TYPES: &[&str] = &[
    "VRC.SDK3.Avatars.Components.VRCAnimatorLayerControl",
    "VRCAnimatorLayerControl",
    "VRC.SDKBase.VRC_AnimatorLayerControl",
];

impl Context {
    pub fn behaviors(&mut self, behaviors: &[Behavior]) -> Result<Vec<avatar::Behavior>, TransformError> {
        behaviors.iter().map(|behavior| self.behavior(behavior)).collect()
    }

    fn behavior(&mut self, behavior: &Behavior) -> Result<avatar::Behavior, TransformError> {
        Ok(match behavior {
            Behavior::Drive(drive) => {
                let (parameter, value) = self.drive(drive)?;
                avatar::Behavior::ParameterDrive(ParameterDrive {
                    target: ParameterDriveTarget::Set { parameter, value },
                })
            }
            Behavior::ParameterDrive(target) => avatar::Behavior::ParameterDrive(ParameterDrive {
                target: self.drive_target(target)?,
            }),
            Behavior::TrackingControl(tracking) => avatar::Behavior::TrackingControl(tracking.clone()),
            Behavior::Generic(generic) => {
                if LAYER_CONTROL_TYPES.contains(&generic.type_name.value.as_str()) {
                    return Err(TransformErrorKind::LayerControlByIndex {
                        type_name: generic.type_name.value.clone(),
                    }
                    .at(generic.type_name.at.clone()));
                }
                avatar::Behavior::Generic(GenericStateBehavior {
                    type_name: self.externals.component_types.intern(generic.type_name.clone()),
                    fields: generic.fields.clone(),
                })
            }
            Behavior::LayerControl(control) => avatar::Behavior::LayerControl(LayerControl {
                layer: self.controlled_layer(&control.layer)?,
                goal_weight: control.goal_weight,
                blend_duration: control.blend_duration,
            }),
            Behavior::LocomotionControl(control) => avatar::Behavior::LocomotionControl(*control),
            Behavior::TemporaryPoseSpace(control) => avatar::Behavior::TemporaryPoseSpace(*control),
            Behavior::PlayableLayerControl(control) => avatar::Behavior::PlayableLayerControl(control.clone()),
            Behavior::PlayAudio(audio) => avatar::Behavior::PlayAudio(self.play_audio(audio)?),
        })
    }

    /// The position of a layer that a layer control in the current controller may reach: a layer of that controller only.
    fn controlled_layer(&self, reference: &Unresolved<String>) -> Result<LayerRef, TransformError> {
        let current = self.current_controller.expect("state behaviors are compiled inside a controller");
        let playable = self.controllers[current].playable;
        if BlendablePlayable::of(playable).is_none() {
            return Err(TransformErrorKind::UncontrollablePlayable { playable }.at(reference.at.clone()));
        }

        self.layer(reference)?;
        let position = &self.layer_positions[&reference.value];
        if let Some(blend) = &position.merged_into {
            return Err(TransformErrorKind::LayerMergedIntoBlend {
                name: reference.value.clone(),
                blend: blend.clone(),
            }
            .at(reference.at.clone()));
        }
        let found = self.controllers[position.controller];
        if found.playable != playable {
            return Err(TransformErrorKind::LayerInAnotherPlayable {
                name: reference.value.clone(),
                expected: playable,
                found: found.playable,
            }
            .at(reference.at.clone()));
        }
        if position.controller != current {
            return Err(TransformErrorKind::LayerInAnotherController {
                name: reference.value.clone(),
                playable: found.playable,
                priority: found.priority,
            }
            .at(reference.at.clone()));
        }

        Ok(LayerRef {
            controller: position.controller,
            layer: position.layer,
        })
    }

    fn play_audio(&mut self, audio: &PlayAudio<Declared>) -> Result<PlayAudio<Compiled>, TransformError> {
        let order = match &audio.order {
            PlaybackOrder::Random => PlaybackOrder::Random,
            PlaybackOrder::UniqueRandom => PlaybackOrder::UniqueRandom,
            PlaybackOrder::Roundabout => PlaybackOrder::Roundabout,
            PlaybackOrder::Parameter(parameter) => PlaybackOrder::Parameter(self.parameters.resolve_typed(parameter, AnimatedValueType::Int)?),
        };
        let externals = &mut self.externals;
        Ok(PlayAudio {
            source: audio.source.clone().map(|path| externals.object_paths.intern(path)),
            order,
            clips: AudioSetting {
                value: audio.clips.value.iter().map(|clip| externals.assets.intern(clip.clone())).collect(),
                apply: audio.clips.apply,
            },
            volume: audio.volume.clone(),
            pitch: audio.pitch.clone(),
            looping: audio.looping.clone(),
            delay: audio.delay,
            play_on_enter: audio.play_on_enter,
            stop_on_enter: audio.stop_on_enter,
            play_on_exit: audio.play_on_exit,
            stop_on_exit: audio.stop_on_exit,
        })
    }

    fn drive_target(&self, target: &ParameterDriveTarget<Declared>) -> Result<ParameterDriveTarget<Compiled>, TransformError> {
        let parameters = &self.parameters;
        Ok(match target {
            ParameterDriveTarget::Set { parameter, value } => {
                let resolved = parameters.resolve(parameter)?;
                let value = cast(value, resolved.context).map_err(|error| error.or_at(parameter.at.as_ref()))?;
                ParameterDriveTarget::Set { parameter: resolved, value }
            }
            ParameterDriveTarget::Add { parameter, value } => {
                let resolved = parameters.resolve(parameter)?;
                if !matches!(resolved.context, AnimatedValueType::Int | AnimatedValueType::Float) {
                    return Err(TransformErrorKind::UnsupportedDrive {
                        drive: "add",
                        parameter: resolved.value,
                        value_type: resolved.context,
                    }
                    .at(parameter.at.clone()));
                }
                let value = cast(value, resolved.context).map_err(|error| error.or_at(parameter.at.as_ref()))?;
                ParameterDriveTarget::Add { parameter: resolved, value }
            }
            ParameterDriveTarget::RandomInt { parameter, range } => ParameterDriveTarget::RandomInt {
                parameter: parameters.resolve_typed(parameter, AnimatedValueType::Int)?,
                range: *range,
            },
            ParameterDriveTarget::RandomBool { parameter, chance } => ParameterDriveTarget::RandomBool {
                parameter: parameters.resolve_typed(parameter, AnimatedValueType::Bool)?,
                chance: *chance,
            },
            ParameterDriveTarget::RandomFloat { parameter, range } => ParameterDriveTarget::RandomFloat {
                parameter: parameters.resolve_typed(parameter, AnimatedValueType::Float)?,
                range: *range,
            },
            ParameterDriveTarget::Copy { from, to } => ParameterDriveTarget::Copy {
                from: parameters.resolve(from)?,
                to: parameters.resolve(to)?,
            },
            ParameterDriveTarget::RangedCopy {
                from,
                from_range,
                to,
                to_range,
            } => ParameterDriveTarget::RangedCopy {
                from: parameters.resolve(from)?,
                from_range: *from_range,
                to: parameters.resolve(to)?,
                to_range: *to_range,
            },
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
                let LayerInfo::Switch { parameter } = info else {
                    return Err(kind_mismatch(layer, "switch", info));
                };
                let parameter = self.parameters.resolve_typed(parameter, AnimatedValueType::Bool)?;
                Ok((parameter, AnimatedValue::Bool(value.unwrap_or(true))))
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
    use std::collections::BTreeMap;

    use rstest::rstest;

    use super::*;
    use crate::{
        core::external::Extern,
        core::resolution::Resolved,
        decl::{
            Avatar,
            behavior::Content,
            controller::Controller,
            layer::{GroupLayer, GroupOption, Layer, PuppetLayer, SwitchContent, SwitchLayer},
            parameter::{Parameter, PrimitiveParameter, PrimitiveParameterValue},
        },
        unity::state::GenericValue,
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
                        settings: Default::default(),
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
                        settings: Default::default(),
                        driven_by: Some("Hat".to_owned().into()),
                        content: SwitchContent::Toggle(Content::new()),
                        at: None,
                    }),
                    Layer::Puppet(PuppetLayer {
                        name: "Wink".into(),
                        settings: Default::default(),
                        driven_by: None,
                        keyframes: vec![],
                        at: None,
                    }),
                ],
            )],
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
        let fields = BTreeMap::from([("goalWeight".to_owned(), GenericValue::Float(1.0))]);
        let mut context = context();
        let compiled = context
            .behaviors(&[
                Behavior::TrackingControl(tracking.clone()),
                Behavior::Drive(Drive::Switch {
                    layer: "Hat".to_owned().into(),
                    value: None,
                }),
                Behavior::Generic(GenericStateBehavior {
                    type_name: located("Custom", 7),
                    fields: fields.clone(),
                }),
            ])
            .unwrap();

        let type_name = Extern::from_index(0);
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
                avatar::Behavior::Generic(GenericStateBehavior { type_name, fields }),
            ]
        );
        let entry = context.externals.component_types.get(type_name);
        assert_eq!(entry.value, "Custom");
        assert_eq!(entry.referenced_at, vec![at(7)]);
    }

    #[rstest]
    #[case::set(
        ParameterDriveTarget::Set { parameter: located("Weight", 3), value: AnimatedValue::Int(1) },
        ParameterDriveTarget::Set { parameter: resolved("Weight", AnimatedValueType::Float), value: AnimatedValue::Float(1.0) },
    )]
    #[case::add_int(
        ParameterDriveTarget::Add { parameter: located("Emote", 3), value: AnimatedValue::Int(2) },
        ParameterDriveTarget::Add { parameter: resolved("Emote", AnimatedValueType::Int), value: AnimatedValue::Int(2) },
    )]
    #[case::add_float(
        ParameterDriveTarget::Add { parameter: located("Weight", 3), value: AnimatedValue::Int(-1) },
        ParameterDriveTarget::Add { parameter: resolved("Weight", AnimatedValueType::Float), value: AnimatedValue::Float(-1.0) },
    )]
    #[case::random_int(
        ParameterDriveTarget::RandomInt { parameter: located("Emote", 3), range: [0, 7] },
        ParameterDriveTarget::RandomInt { parameter: resolved("Emote", AnimatedValueType::Int), range: [0, 7] },
    )]
    #[case::random_bool(
        ParameterDriveTarget::RandomBool { parameter: located("Hat", 3), chance: 0.25 },
        ParameterDriveTarget::RandomBool { parameter: resolved("Hat", AnimatedValueType::Bool), chance: 0.25 },
    )]
    #[case::random_float(
        ParameterDriveTarget::RandomFloat { parameter: located("Wink", 3), range: [-1.0, 1.0] },
        ParameterDriveTarget::RandomFloat { parameter: resolved("Wink", AnimatedValueType::Float), range: [-1.0, 1.0] },
    )]
    #[case::copy(
        ParameterDriveTarget::Copy { from: located("Emote", 3), to: located("Weight", 3) },
        ParameterDriveTarget::Copy { from: resolved("Emote", AnimatedValueType::Int), to: resolved("Weight", AnimatedValueType::Float) },
    )]
    #[case::ranged_copy(
        ParameterDriveTarget::RangedCopy { from: located("Wink", 3), from_range: [-1.0, 1.0], to: located("Emote", 3), to_range: [0.0, 255.0] },
        ParameterDriveTarget::RangedCopy {
            from: resolved("Wink", AnimatedValueType::Float),
            from_range: [-1.0, 1.0],
            to: resolved("Emote", AnimatedValueType::Int),
            to_range: [0.0, 255.0],
        },
    )]
    fn state_drives_resolve_their_parameters(#[case] written: ParameterDriveTarget<Declared>, #[case] expected: ParameterDriveTarget<Compiled>) {
        let compiled = context().behaviors(&[Behavior::ParameterDrive(written)]).unwrap();
        assert_eq!(compiled, vec![avatar::Behavior::ParameterDrive(ParameterDrive { target: expected })]);
    }

    #[rstest]
    #[case::add_bool(
        ParameterDriveTarget::Add { parameter: located("Hat", 3), value: AnimatedValue::Bool(true) },
        TransformErrorKind::UnsupportedDrive { drive: "add", parameter: "Hat".into(), value_type: AnimatedValueType::Bool },
    )]
    #[case::random_int_on_float(
        ParameterDriveTarget::RandomInt { parameter: located("Wink", 3), range: [0, 1] },
        TransformErrorKind::ParameterTypeMismatch { name: "Wink".into(), expected: AnimatedValueType::Int, found: AnimatedValueType::Float },
    )]
    #[case::random_bool_on_int(
        ParameterDriveTarget::RandomBool { parameter: located("Emote", 3), chance: 0.5 },
        TransformErrorKind::ParameterTypeMismatch { name: "Emote".into(), expected: AnimatedValueType::Bool, found: AnimatedValueType::Int },
    )]
    #[case::random_float_on_bool(
        ParameterDriveTarget::RandomFloat { parameter: located("Hat", 3), range: [0.0, 1.0] },
        TransformErrorKind::ParameterTypeMismatch { name: "Hat".into(), expected: AnimatedValueType::Float, found: AnimatedValueType::Bool },
    )]
    #[case::copy_from_nowhere(
        ParameterDriveTarget::Copy { from: located("Missing", 3), to: located("Emote", 4) },
        TransformErrorKind::UnknownParameter { name: "Missing".into() },
    )]
    fn a_bad_state_drive_is_reported_where_it_was_written(#[case] written: ParameterDriveTarget<Declared>, #[case] expected: TransformErrorKind) {
        let error = context().behaviors(&[Behavior::ParameterDrive(written)]).unwrap_err();
        assert_eq!(error, expected.at(Some(at(3))));
    }
}
