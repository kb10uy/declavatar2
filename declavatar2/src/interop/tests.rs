use std::{
    collections::{BTreeMap, HashMap},
    fmt::Debug,
};

use nalgebra::{UnitQuaternion, Vector2, Vector3, Vector4};
use rstest::*;

use super::{
    avatar::TRACKING_TARGETS,
    header::{read_blob, write_blob},
    wire::Context,
    *,
};
use crate::{
    EvaluateOptions,
    avatar::{
        AnimatorCondition, AnimatorController, AnimatorLayer, AnimatorState, AnimatorTransition, Behavior, BlendTree, Clip, DirectBlendTree, DirectField,
        LayerRef, LayerSettings, MenuAxis, MenuDirection, MenuItem, Motion, ParameterRef, ParametricBlendTree, ParametricField, PlayableController, Playback,
        StateMachine, TransitionSource, TransitionTarget,
    },
    compile,
    core::{
        external::{Extern, ExternTable},
        phase::Compiled,
        resolution::{Resolved, SourceLocation, Unresolved},
        value_set::ValueSet,
    },
    test_support::enum_cases,
    unity::{
        animation::{ClipAttributes, Curve, FixedAnimationEntry, InlineAnimation, Interpolation, KeyedAnimation, KeyedAnimationEntry, Keyframe},
        animator::{
            AnimatedAnimatorProperty, AnimatedAnimatorTarget, AnimatedComponentProperty, AnimatedComponentTarget, AnimatedGameObjectProperty,
            AnimatedGameObjectTarget, AnimatedRendererProperty, AnimatedRendererTarget, AnimatedTarget, AnimatorParameter, AnimatorParameterOrigin,
            AnimatorParameterTypeDefault, BlendTreeType, LayerBlending, MergeMode, PathMode,
        },
        external::{Asset, AssetLocator, ComponentType, Externals, ObjectPath},
        state::{GenericStateBehavior, GenericValue},
        value::{AnimatedValue, AnimatedValueType},
    },
    vrchat::{
        expr_parameter::{ExpressionParameter, ExpressionParameterTypeDefault, ExpressionParameterWidth, ProvidedParameterGroup},
        playable_layer::PlayableLayer,
        state_behaviour::{
            ApplySettings, AudioSetting, BlendablePlayable, LayerControl, LocomotionControl, ParameterDrive, ParameterDriveTarget, PlayAudio,
            PlayableLayerControl, PlaybackOrder, TemporaryPoseSpace, TrackingControl, TrackingControlMode, TrackingControlTarget,
        },
    },
};

fn context() -> Context {
    Context {
        extern_lens: [4, 4, 4],
        parameters: BTreeMap::from([
            ("Bool".to_string(), AnimatedValueType::Bool),
            ("Int".to_string(), AnimatedValueType::Int),
            ("Float".to_string(), AnimatedValueType::Float),
        ]),
        states: Some(4),
        machines: Some(2),
    }
}

fn encode<T: Encode + ?Sized>(value: &T) -> Vec<u8> {
    let mut writer = Writer::new();
    value.encode(&mut writer).expect("the fixture should be encodable");
    writer.into_bytes()
}

fn decode<T: Decode>(bytes: &[u8]) -> Result<T, DecodeError> {
    let mut reader = Reader::new(bytes);
    *reader.context_mut() = context();
    let value = T::decode(&mut reader)?;
    reader.finish()?;
    Ok(value)
}

fn round_trip<T: Encode + Decode + PartialEq + Debug>(value: T) {
    let bytes = encode(&value);
    assert_eq!(decode::<T>(&bytes).unwrap(), value);
}

fn param(name: &str) -> ParameterRef {
    let value_type = context().parameters[name];
    Resolved::new(name.into(), value_type)
}

fn asset(index: u32) -> Extern<Asset> {
    Extern::from_index(index)
}

fn object(index: u32) -> Extern<ObjectPath> {
    Extern::from_index(index)
}

fn component_ref(index: u32) -> Extern<ComponentType> {
    Extern::from_index(index)
}

fn at(line: u32) -> SourceLocation {
    SourceLocation {
        chunk: "avatar.lua".into(),
        line,
    }
}

fn blend_shape(name: &str) -> AnimatedTarget<Compiled> {
    AnimatedTarget::Renderer(AnimatedRendererTarget {
        path: object(0),
        renderer_type: "UnityEngine.SkinnedMeshRenderer".into(),
        property: AnimatedRendererProperty::BlendShape { name: name.into() },
    })
}

fn active(index: u32) -> AnimatedTarget<Compiled> {
    AnimatedTarget::GameObject(AnimatedGameObjectTarget {
        path: object(index),
        property: AnimatedGameObjectProperty::Active,
    })
}

fn fixed_clip() -> InlineAnimation<Compiled> {
    InlineAnimation::Fixed(ValueSet::from([
        FixedAnimationEntry {
            key: blend_shape("smile"),
            value: AnimatedValue::Float(1.0),
        },
        FixedAnimationEntry {
            key: active(1),
            value: AnimatedValue::Bool(true),
        },
    ]))
}

fn keyed_clip() -> InlineAnimation<Compiled> {
    InlineAnimation::Keyed(KeyedAnimation {
        attributes: ClipAttributes {
            length: 2.0,
            loop_time: true,
            loop_blend: false,
            cycle_offset: 0.25,
        },
        curves: ValueSet::from([KeyedAnimationEntry {
            key: blend_shape("smile"),
            curve: Curve {
                first: Keyframe {
                    time: 0.0,
                    value: AnimatedValue::Float(0.0),
                },
                rest: vec![
                    (
                        Interpolation::Linear,
                        Keyframe {
                            time: 0.5,
                            value: AnimatedValue::Float(1.0),
                        },
                    ),
                    (
                        Interpolation::Bezier {
                            x1: 0.25,
                            y1: 0.0,
                            x2: 0.75,
                            y2: 1.0,
                        },
                        Keyframe {
                            time: 1.0,
                            value: AnimatedValue::Float(0.5),
                        },
                    ),
                ],
            },
        }]),
    })
}

fn parametric_tree() -> ParametricBlendTree {
    ParametricBlendTree {
        tree_type: BlendTreeType::Linear,
        x: param("Float"),
        y: None,
        fields: vec![
            ParametricField {
                position: [-1.0, 0.0],
                speed: 1.0,
                motion: Motion::Clip(Clip::Inline(fixed_clip())),
            },
            ParametricField {
                position: [1.0, 0.0],
                speed: 0.5,
                motion: Motion::Clip(Clip::External(asset(2))),
            },
        ],
    }
}

fn direct_tree() -> DirectBlendTree {
    DirectBlendTree {
        fields: vec![DirectField {
            weight_by: param("Float"),
            speed: 1.0,
            motion: Motion::BlendTree(BlendTree::Parametric(parametric_tree())),
        }],
    }
}

fn axis(positive: Option<&str>, negative: Option<&str>) -> MenuAxis {
    MenuAxis {
        parameter: param("Float"),
        positive: positive.map(Into::into),
        negative: negative.map(Into::into),
    }
}

fn direction(label: Option<&str>) -> MenuDirection {
    MenuDirection {
        parameter: param("Float"),
        label: label.map(Into::into),
    }
}

fn state(name: &str, motion: Option<Motion>) -> AnimatorState {
    AnimatorState {
        name: name.into(),
        machine: None,
        motion,
        playback: Playback::default(),
        write_defaults: false,
        behaviors: Vec::new(),
    }
}

enum_cases! {
    fn asset_locators(value: AssetLocator) {
        round_trip(value);
    }
    cases {
        guid: AssetLocator::Guid(_) => AssetLocator::Guid("0123456789abcdef0123456789abcdef".into()),
        path: AssetLocator::Path(_) => AssetLocator::Path("Assets/Materials/Skin.mat".into()),
        named: AssetLocator::Named { .. } => AssetLocator::Named { asset_type: "UnityEngine.Material".into(), name: "Skin".into() },
    }
}

enum_cases! {
    fn expression_parameter_types(value: ExpressionParameterTypeDefault) {
        round_trip(value);
    }
    cases {
        bool: ExpressionParameterTypeDefault::Bool(_) => ExpressionParameterTypeDefault::Bool(Some(true)),
        int: ExpressionParameterTypeDefault::Int { .. } => ExpressionParameterTypeDefault::Int { width: ExpressionParameterWidth::Specified(8), default: Some(i32::MIN) },
        float: ExpressionParameterTypeDefault::Float { .. } => ExpressionParameterTypeDefault::Float { width: ExpressionParameterWidth::Unspecified, default: None },
    }
}

enum_cases! {
    fn expression_parameter_widths(value: ExpressionParameterWidth) {
        round_trip(value);
    }
    cases {
        unspecified: ExpressionParameterWidth::Unspecified => ExpressionParameterWidth::Unspecified,
        specified: ExpressionParameterWidth::Specified(_) => ExpressionParameterWidth::Specified(255),
    }
}

enum_cases! {
    fn playable_layers(value: PlayableLayer) {
        round_trip(value);
    }
    cases {
        base: PlayableLayer::Base => PlayableLayer::Base,
        additive: PlayableLayer::Additive => PlayableLayer::Additive,
        gesture: PlayableLayer::Gesture => PlayableLayer::Gesture,
        action: PlayableLayer::Action => PlayableLayer::Action,
        fx: PlayableLayer::Fx => PlayableLayer::Fx,
        sitting: PlayableLayer::Sitting => PlayableLayer::Sitting,
        tpose: PlayableLayer::TPose => PlayableLayer::TPose,
        ikpose: PlayableLayer::IkPose => PlayableLayer::IkPose,
    }
}

enum_cases! {
    fn layer_blendings(value: LayerBlending) {
        round_trip(value);
    }
    cases {
        override_: LayerBlending::Override => LayerBlending::Override,
        additive: LayerBlending::Additive => LayerBlending::Additive,
    }
}

#[rstest]
fn layer_settings_round_trip() {
    round_trip(LayerSettings {
        weight: 0.25,
        blending: LayerBlending::Additive,
        mask: Some(asset(3)),
    });
}

enum_cases! {
    fn merge_modes(value: MergeMode) {
        round_trip(value);
    }
    cases {
        append: MergeMode::Append => MergeMode::Append,
        replace: MergeMode::Replace => MergeMode::Replace,
    }
}

enum_cases! {
    fn path_modes(value: PathMode) {
        round_trip(value);
    }
    cases {
        absolute: PathMode::Absolute => PathMode::Absolute,
        relative: PathMode::Relative => PathMode::Relative,
    }
}

enum_cases! {
    fn animator_parameter_types(value: AnimatorParameterTypeDefault) {
        round_trip(value);
    }
    cases {
        bool: AnimatorParameterTypeDefault::Bool(_) => AnimatorParameterTypeDefault::Bool(None),
        int: AnimatorParameterTypeDefault::Int(_) => AnimatorParameterTypeDefault::Int(Some(i32::MAX)),
        float: AnimatorParameterTypeDefault::Float(_) => AnimatorParameterTypeDefault::Float(Some(f32::MIN)),
    }
}

enum_cases! {
    fn animator_parameter_origins(value: AnimatorParameterOrigin) {
        round_trip(AnimatorParameter::create_bool("Origin", None).with_origin(value));
        round_trip(value);
    }
    cases {
        declared: AnimatorParameterOrigin::Declared => AnimatorParameterOrigin::Declared,
        generated: AnimatorParameterOrigin::Generated => AnimatorParameterOrigin::Generated,
        provided: AnimatorParameterOrigin::Provided(_) => AnimatorParameterOrigin::Provided(ProvidedParameterGroup::Vrchat),
    }
}

enum_cases! {
    fn provided_parameter_groups(value: ProvidedParameterGroup) {
        round_trip(value);
    }
    cases {
        vrchat: ProvidedParameterGroup::Vrchat => ProvidedParameterGroup::Vrchat,
    }
}

enum_cases! {
    fn transition_sources(value: TransitionSource) {
        round_trip(value);
    }
    cases {
        entry: TransitionSource::Entry(_) => TransitionSource::Entry(Some(1)),
        state: TransitionSource::State(_) => TransitionSource::State(3),
        machine_exit: TransitionSource::MachineExit(_) => TransitionSource::MachineExit(0),
    }
}

enum_cases! {
    fn transition_targets(value: TransitionTarget) {
        round_trip(value);
    }
    cases {
        state: TransitionTarget::State(_) => TransitionTarget::State(0),
        exit: TransitionTarget::Exit => TransitionTarget::Exit,
        machine: TransitionTarget::Machine(_) => TransitionTarget::Machine(1),
    }
}

#[rstest]
fn the_root_entry_round_trips() {
    round_trip(TransitionSource::Entry(None));
}

#[rstest]
fn a_layer_with_nested_machines_round_trips() {
    let nested = |name: &str, machine: Option<usize>| AnimatorState { machine, ..state(name, None) };
    round_trip(AnimatorLayer {
        name: "Emote".into(),
        settings: LayerSettings::default(),
        default_state: Some(0),
        machines: vec![
            StateMachine {
                name: "Dance".into(),
                parent: None,
                default_state: Some(1),
            },
            StateMachine {
                name: "Finale".into(),
                parent: Some(0),
                default_state: Some(2),
            },
        ],
        states: vec![nested("Idle", None), nested("Step", Some(0)), nested("Bow", Some(1))],
        transitions: vec![
            AnimatorTransition {
                from: TransitionSource::State(0),
                to: TransitionTarget::Machine(0),
                duration: 0.0,
                conditions: vec![],
            },
            AnimatorTransition {
                from: TransitionSource::Entry(Some(0)),
                to: TransitionTarget::Machine(1),
                duration: 0.0,
                conditions: vec![],
            },
            AnimatorTransition {
                from: TransitionSource::State(2),
                to: TransitionTarget::Exit,
                duration: 0.0,
                conditions: vec![],
            },
            AnimatorTransition {
                from: TransitionSource::MachineExit(0),
                to: TransitionTarget::State(0),
                duration: 0.0,
                conditions: vec![],
            },
        ],
    });
}

enum_cases! {
    fn conditions(value: AnimatorCondition) {
        round_trip(value);
    }
    cases {
        if_true: AnimatorCondition::If(_) => AnimatorCondition::If(param("Bool")),
        if_not: AnimatorCondition::IfNot(_) => AnimatorCondition::IfNot(param("Bool")),
        equals: AnimatorCondition::Equals(..) => AnimatorCondition::Equals(param("Int"), i64::MIN),
        not_equal: AnimatorCondition::NotEqual(..) => AnimatorCondition::NotEqual(param("Int"), i64::MAX),
        greater: AnimatorCondition::Greater(..) => AnimatorCondition::Greater(param("Float"), f64::INFINITY),
        less: AnimatorCondition::Less(..) => AnimatorCondition::Less(param("Float"), f64::NEG_INFINITY),
    }
}

enum_cases! {
    fn motions(value: Motion) {
        round_trip(value);
    }
    cases {
        clip: Motion::Clip(_) => Motion::Clip(Clip::Inline(fixed_clip())),
        blend_tree: Motion::BlendTree(_) => Motion::BlendTree(BlendTree::Direct(direct_tree())),
    }
}

enum_cases! {
    fn clips(value: Clip) {
        let bytes = encode(&value);
        assert_eq!(bytes, encode(&Motion::Clip(value.clone())));
        round_trip(value);
    }
    cases {
        inline: Clip::Inline(_) => Clip::Inline(keyed_clip()),
        external: Clip::External(_) => Clip::External(asset(3)),
    }
}

enum_cases! {
    fn blend_trees(value: BlendTree) {
        let bytes = encode(&value);
        assert_eq!(bytes, encode(&Motion::BlendTree(value.clone())));
        round_trip(value);
    }
    cases {
        parametric: BlendTree::Parametric(_) => BlendTree::Parametric(ParametricBlendTree { y: Some(param("Float")), tree_type: BlendTreeType::Freeform2d, ..parametric_tree() }),
        direct: BlendTree::Direct(_) => BlendTree::Direct(direct_tree()),
    }
}

enum_cases! {
    fn blend_tree_types(value: BlendTreeType) {
        round_trip(value);
    }
    cases {
        linear: BlendTreeType::Linear => BlendTreeType::Linear,
        simple_2d: BlendTreeType::Simple2d => BlendTreeType::Simple2d,
        freeform_2d: BlendTreeType::Freeform2d => BlendTreeType::Freeform2d,
        cartesian_2d: BlendTreeType::Cartesian2d => BlendTreeType::Cartesian2d,
    }
}

enum_cases! {
    fn inline_animations(value: InlineAnimation<Compiled>) {
        round_trip(value);
    }
    cases {
        fixed: InlineAnimation::Fixed(_) => fixed_clip(),
        keyed: InlineAnimation::Keyed(_) => keyed_clip(),
    }
}

enum_cases! {
    fn interpolations(value: Interpolation) {
        round_trip(value);
    }
    cases {
        constant: Interpolation::Constant => Interpolation::Constant,
        linear: Interpolation::Linear => Interpolation::Linear,
        bezier: Interpolation::Bezier { .. } => Interpolation::Bezier { x1: 0.1, y1: -2.0, x2: 0.9, y2: 3.0 },
    }
}

enum_cases! {
    fn animated_targets(value: AnimatedTarget<Compiled>) {
        round_trip(value);
    }
    cases {
        animator_self: AnimatedTarget::AnimatorSelf(_) => AnimatedTarget::AnimatorSelf(AnimatedAnimatorTarget { property: AnimatedAnimatorProperty::ParameterFloatValue { name: param("Float") } }),
        game_object: AnimatedTarget::GameObject(_) => active(3),
        renderer: AnimatedTarget::Renderer(_) => blend_shape("smile"),
        component: AnimatedTarget::Component(_) => AnimatedTarget::Component(AnimatedComponentTarget { path: object(2), component_type: component_ref(3), property: AnimatedComponentProperty::Enabled, value_type: AnimatedValueType::Bool }),
    }
}

enum_cases! {
    fn animator_properties(value: AnimatedAnimatorProperty<Compiled>) {
        round_trip(value);
    }
    cases {
        parameter_float_value: AnimatedAnimatorProperty::ParameterFloatValue { .. } => AnimatedAnimatorProperty::ParameterFloatValue { name: param("Float") },
    }
}

enum_cases! {
    fn game_object_properties(value: AnimatedGameObjectProperty) {
        round_trip(value);
    }
    cases {
        active: AnimatedGameObjectProperty::Active => AnimatedGameObjectProperty::Active,
        position: AnimatedGameObjectProperty::TransformPosition => AnimatedGameObjectProperty::TransformPosition,
        rotation_quaternion: AnimatedGameObjectProperty::TransformRotationQuaternion => AnimatedGameObjectProperty::TransformRotationQuaternion,
        rotation_euler: AnimatedGameObjectProperty::TransformRotationEuler => AnimatedGameObjectProperty::TransformRotationEuler,
        scale: AnimatedGameObjectProperty::TransformScale => AnimatedGameObjectProperty::TransformScale,
    }
}

enum_cases! {
    fn renderer_properties(value: AnimatedRendererProperty) {
        round_trip(value);
    }
    cases {
        enabled: AnimatedRendererProperty::Enabled => AnimatedRendererProperty::Enabled,
        blend_shape: AnimatedRendererProperty::BlendShape { .. } => AnimatedRendererProperty::BlendShape { name: "smile".into() },
        material: AnimatedRendererProperty::Material { .. } => AnimatedRendererProperty::Material { slot: u32::MAX },
        material_property: AnimatedRendererProperty::MaterialProperty { .. } => AnimatedRendererProperty::MaterialProperty { name: "_Color".into() },
        serialized: AnimatedRendererProperty::Serialized { .. } => AnimatedRendererProperty::Serialized { name: "m_Mesh".into() },
    }
}

enum_cases! {
    fn component_properties(value: AnimatedComponentProperty) {
        round_trip(value);
    }
    cases {
        enabled: AnimatedComponentProperty::Enabled => AnimatedComponentProperty::Enabled,
        serialized: AnimatedComponentProperty::Serialized { .. } => AnimatedComponentProperty::Serialized { name: "m_Range".into() },
    }
}

enum_cases! {
    fn animated_value_types(value: AnimatedValueType) {
        let bytes = encode(&value);
        round_trip(value);
        let sample: AnimatedValue<Extern<Asset>> = match value {
            AnimatedValueType::Float => AnimatedValue::Float(0.0),
            AnimatedValueType::Int => AnimatedValue::Int(0),
            AnimatedValueType::Bool => AnimatedValue::Bool(false),
            AnimatedValueType::Vector2 => AnimatedValue::Vector2(Vector2::zeros()),
            AnimatedValueType::Vector3 => AnimatedValue::Vector3(Vector3::zeros()),
            AnimatedValueType::Vector4 => AnimatedValue::Vector4(Vector4::zeros()),
            AnimatedValueType::Quaternion => AnimatedValue::Quaternion(UnitQuaternion::identity()),
            AnimatedValueType::Color => AnimatedValue::Color(Vector4::zeros()),
            AnimatedValueType::ObjectReference => AnimatedValue::ObjectReference(asset(0)),
        };
        assert_eq!(bytes[0], encode(&sample)[0], "the type and the value must share a discriminator");
    }
    cases {
        float: AnimatedValueType::Float => AnimatedValueType::Float,
        int: AnimatedValueType::Int => AnimatedValueType::Int,
        bool: AnimatedValueType::Bool => AnimatedValueType::Bool,
        vector2: AnimatedValueType::Vector2 => AnimatedValueType::Vector2,
        vector3: AnimatedValueType::Vector3 => AnimatedValueType::Vector3,
        vector4: AnimatedValueType::Vector4 => AnimatedValueType::Vector4,
        quaternion: AnimatedValueType::Quaternion => AnimatedValueType::Quaternion,
        color: AnimatedValueType::Color => AnimatedValueType::Color,
        object_reference: AnimatedValueType::ObjectReference => AnimatedValueType::ObjectReference,
    }
}

enum_cases! {
    fn animated_values(value: AnimatedValue<Extern<Asset>>) {
        round_trip(value);
    }
    cases {
        float: AnimatedValue::Float(_) => AnimatedValue::Float(f64::MAX),
        int: AnimatedValue::Int(_) => AnimatedValue::Int(i64::MIN),
        bool: AnimatedValue::Bool(_) => AnimatedValue::Bool(true),
        vector2: AnimatedValue::Vector2(_) => AnimatedValue::Vector2(Vector2::new(1.0, -2.0)),
        vector3: AnimatedValue::Vector3(_) => AnimatedValue::Vector3(Vector3::new(1.0, -2.0, 3.0)),
        vector4: AnimatedValue::Vector4(_) => AnimatedValue::Vector4(Vector4::new(1.0, -2.0, 3.0, -4.0)),
        quaternion: AnimatedValue::Quaternion(_) => AnimatedValue::Quaternion(UnitQuaternion::from_axis_angle(&Vector3::y_axis(), 1.0)),
        color: AnimatedValue::Color(_) => AnimatedValue::Color(Vector4::new(0.25, 0.5, 0.75, 1.0)),
        object_reference: AnimatedValue::ObjectReference(_) => AnimatedValue::ObjectReference(asset(3)),
    }
}

enum_cases! {
    fn reference_free_animated_values(value: AnimatedValue<()>) {
        match value {
            AnimatedValue::ObjectReference(()) => {
                let mut writer = Writer::new();
                assert_eq!(
                    value.encode(&mut writer).unwrap_err(),
                    EncodeError::Unrepresentable("an object reference without an asset")
                );
            }
            other => round_trip(other),
        }
    }
    cases {
        float: AnimatedValue::Float(_) => AnimatedValue::Float(-0.5),
        int: AnimatedValue::Int(_) => AnimatedValue::Int(7),
        bool: AnimatedValue::Bool(_) => AnimatedValue::Bool(false),
        vector2: AnimatedValue::Vector2(_) => AnimatedValue::Vector2(Vector2::zeros()),
        vector3: AnimatedValue::Vector3(_) => AnimatedValue::Vector3(Vector3::zeros()),
        vector4: AnimatedValue::Vector4(_) => AnimatedValue::Vector4(Vector4::zeros()),
        quaternion: AnimatedValue::Quaternion(_) => AnimatedValue::Quaternion(UnitQuaternion::identity()),
        color: AnimatedValue::Color(_) => AnimatedValue::Color(Vector4::zeros()),
        object_reference: AnimatedValue::ObjectReference(_) => AnimatedValue::ObjectReference(()),
    }
}

enum_cases! {
    fn behaviors(value: Behavior) {
        round_trip(value);
    }
    cases {
        parameter_drive: Behavior::ParameterDrive(_) => Behavior::ParameterDrive(ParameterDrive { target: ParameterDriveTarget::Set { parameter: param("Int"), value: AnimatedValue::Int(1) } }),
        tracking_control: Behavior::TrackingControl(_) => Behavior::TrackingControl(TrackingControl { values: HashMap::from([(TrackingControlTarget::Head, TrackingControlMode::Animation), (TrackingControlTarget::Mouth, TrackingControlMode::Tracking)]) }),
        generic: Behavior::Generic(_) => Behavior::Generic(GenericStateBehavior { type_name: component_ref(1), fields: BTreeMap::from([("goalWeight".to_string(), GenericValue::Float(1.0))]) }),
        layer_control: Behavior::LayerControl(_) => Behavior::LayerControl(LayerControl { layer: LayerRef { controller: 1, layer: 2 }, goal_weight: 0.5, blend_duration: 0.25 }),
        locomotion_control: Behavior::LocomotionControl(_) => Behavior::LocomotionControl(LocomotionControl { disable_locomotion: true }),
        temporary_pose_space: Behavior::TemporaryPoseSpace(_) => Behavior::TemporaryPoseSpace(TemporaryPoseSpace { enter: true, fixed_delay: false, delay: 0.5 }),
        playable_layer_control: Behavior::PlayableLayerControl(_) => Behavior::PlayableLayerControl(PlayableLayerControl { playable: BlendablePlayable::Action, goal_weight: 1.0, blend_duration: 0.5 }),
        play_audio: Behavior::PlayAudio(_) => Behavior::PlayAudio(audio_fixture(PlaybackOrder::Parameter(param("Int")))),
    }
}

fn audio_fixture(order: PlaybackOrder<Compiled>) -> PlayAudio<Compiled> {
    PlayAudio {
        source: Some(object(2)),
        order,
        clips: AudioSetting {
            value: vec![asset(0), asset(3)],
            apply: ApplySettings::Always,
        },
        volume: AudioSetting {
            value: [0.25, 0.75],
            apply: ApplySettings::IfStopped,
        },
        pitch: AudioSetting {
            value: [-1.5, 2.0],
            apply: ApplySettings::Never,
        },
        looping: AudioSetting {
            value: true,
            apply: ApplySettings::Always,
        },
        delay: 1.5,
        play_on_enter: true,
        stop_on_enter: false,
        play_on_exit: true,
        stop_on_exit: false,
    }
}

enum_cases! {
    fn blendable_playables(value: BlendablePlayable) {
        round_trip(value);
    }
    cases {
        action: BlendablePlayable::Action => BlendablePlayable::Action,
        fx: BlendablePlayable::Fx => BlendablePlayable::Fx,
        gesture: BlendablePlayable::Gesture => BlendablePlayable::Gesture,
        additive: BlendablePlayable::Additive => BlendablePlayable::Additive,
    }
}

enum_cases! {
    fn playback_orders(value: PlaybackOrder<Compiled>) {
        round_trip(value);
    }
    cases {
        random: PlaybackOrder::Random => PlaybackOrder::Random,
        unique_random: PlaybackOrder::UniqueRandom => PlaybackOrder::UniqueRandom,
        roundabout: PlaybackOrder::Roundabout => PlaybackOrder::Roundabout,
        parameter: PlaybackOrder::Parameter(_) => PlaybackOrder::Parameter(param("Int")),
    }
}

enum_cases! {
    fn apply_settings(value: ApplySettings) {
        round_trip(value);
    }
    cases {
        always: ApplySettings::Always => ApplySettings::Always,
        if_stopped: ApplySettings::IfStopped => ApplySettings::IfStopped,
        never: ApplySettings::Never => ApplySettings::Never,
    }
}

#[rstest]
fn a_play_audio_without_a_source_plays_on_the_root() {
    round_trip(PlayAudio {
        source: None,
        clips: AudioSetting {
            value: vec![],
            apply: ApplySettings::IfStopped,
        },
        ..audio_fixture(PlaybackOrder::Random)
    });
}

enum_cases! {
    fn parameter_drive_targets(value: ParameterDriveTarget<Compiled>) {
        round_trip(value);
    }
    cases {
        set: ParameterDriveTarget::Set { .. } => ParameterDriveTarget::Set { parameter: param("Bool"), value: AnimatedValue::Bool(true) },
        add: ParameterDriveTarget::Add { .. } => ParameterDriveTarget::Add { parameter: param("Float"), value: AnimatedValue::Float(0.5) },
        random_int: ParameterDriveTarget::RandomInt { .. } => ParameterDriveTarget::RandomInt { parameter: param("Int"), range: [i64::MIN, i64::MAX] },
        random_bool: ParameterDriveTarget::RandomBool { .. } => ParameterDriveTarget::RandomBool { parameter: param("Bool"), chance: 0.25 },
        random_float: ParameterDriveTarget::RandomFloat { .. } => ParameterDriveTarget::RandomFloat { parameter: param("Float"), range: [-1.0, 1.0] },
        copy: ParameterDriveTarget::Copy { .. } => ParameterDriveTarget::Copy { from: param("Int"), to: param("Float") },
        ranged_copy: ParameterDriveTarget::RangedCopy { .. } => ParameterDriveTarget::RangedCopy { from: param("Float"), from_range: [0.0, 1.0], to: param("Int"), to_range: [0.0, 255.0] },
    }
}

enum_cases! {
    fn tracking_control_targets(value: TrackingControlTarget) {
        assert!(TRACKING_TARGETS.contains(&value));
        let control = TrackingControl {
            values: HashMap::from([(value, TrackingControlMode::Tracking)]),
        };
        let bytes = encode(&control);
        let slot = TRACKING_TARGETS.iter().position(|t| *t == value).unwrap();
        assert_eq!(bytes.len(), 10);
        assert!(bytes.iter().enumerate().all(|(i, b)| *b == u8::from(i == slot)));
        round_trip(control);
    }
    cases {
        head: TrackingControlTarget::Head => TrackingControlTarget::Head,
        left_hand: TrackingControlTarget::LeftHand => TrackingControlTarget::LeftHand,
        right_hand: TrackingControlTarget::RightHand => TrackingControlTarget::RightHand,
        hip: TrackingControlTarget::Hip => TrackingControlTarget::Hip,
        left_foot: TrackingControlTarget::LeftFoot => TrackingControlTarget::LeftFoot,
        right_foot: TrackingControlTarget::RightFoot => TrackingControlTarget::RightFoot,
        left_fingers: TrackingControlTarget::LeftFingers => TrackingControlTarget::LeftFingers,
        right_fingers: TrackingControlTarget::RightFingers => TrackingControlTarget::RightFingers,
        eyes: TrackingControlTarget::Eyes => TrackingControlTarget::Eyes,
        mouth: TrackingControlTarget::Mouth => TrackingControlTarget::Mouth,
    }
}

enum_cases! {
    fn tracking_control_modes(value: TrackingControlMode) {
        round_trip(value);
    }
    cases {
        no_change: TrackingControlMode::NoChange => TrackingControlMode::NoChange,
        tracking: TrackingControlMode::Tracking => TrackingControlMode::Tracking,
        animation: TrackingControlMode::Animation => TrackingControlMode::Animation,
    }
}

enum_cases! {
    fn generic_values(value: GenericValue) {
        round_trip(value);
    }
    cases {
        bool: GenericValue::Bool(_) => GenericValue::Bool(true),
        int: GenericValue::Int(_) => GenericValue::Int(-1),
        float: GenericValue::Float(_) => GenericValue::Float(2.5),
        string: GenericValue::String(_) => GenericValue::String("text".into()),
        list: GenericValue::List(_) => GenericValue::List(vec![GenericValue::Int(1), GenericValue::List(vec![])]),
        map: GenericValue::Map(_) => GenericValue::Map(BTreeMap::from([("nested".to_string(), GenericValue::Map(BTreeMap::new()))])),
    }
}

enum_cases! {
    fn menu_items(value: MenuItem) {
        round_trip(value);
    }
    cases {
        sub_menu: MenuItem::SubMenu { .. } => MenuItem::SubMenu { name: "Emotes".into(), items: vec![MenuItem::Radial { name: "Blend".into(), parameter: param("Float") }] },
        toggle: MenuItem::Toggle { .. } => MenuItem::Toggle { name: "Hat".into(), parameter: param("Bool"), value: AnimatedValue::Bool(true) },
        button: MenuItem::Button { .. } => MenuItem::Button { name: "Wave".into(), parameter: param("Int"), value: AnimatedValue::Int(3) },
        radial: MenuItem::Radial { .. } => MenuItem::Radial { name: "Blend".into(), parameter: param("Float") },
        two_axis: MenuItem::TwoAxis { .. } => MenuItem::TwoAxis { name: "Look".into(), horizontal: axis(Some("Right"), Some("Left")), vertical: axis(None, Some("Down")) },
        four_axis: MenuItem::FourAxis { .. } => MenuItem::FourAxis { name: "Move".into(), up: direction(None), down: direction(Some("D")), left: direction(None), right: direction(Some("R")) },
    }
}

enum_cases! {
    fn diagnostic_stages(value: DiagnosticStage) {
        round_trip(value);
    }
    cases {
        script: DiagnosticStage::Script => DiagnosticStage::Script,
        transform: DiagnosticStage::Transform => DiagnosticStage::Transform,
    }
}

enum_cases! {
    fn blob_kinds(value: BlobKind) {
        let blob = write_blob(value, &[1, 2, 3]).unwrap();
        assert_eq!(&blob[..4], value.magic());
        assert_eq!(BlobKind::of_magic(value.magic()), Some(value));
        assert_eq!(read_blob(value, &blob).unwrap(), [1, 2, 3]);
    }
    cases {
        avatar: BlobKind::Avatar => BlobKind::Avatar,
        diagnostics: BlobKind::Diagnostics => BlobKind::Diagnostics,
    }
}

#[rstest]
fn zero_bit_widths_have_no_representation() {
    let mut writer = Writer::new();
    assert_eq!(
        ExpressionParameterWidth::Specified(0).encode(&mut writer).unwrap_err(),
        EncodeError::Unrepresentable("a specified width of zero bits")
    );
}

#[cfg(target_pointer_width = "64")]
#[rstest]
fn state_indices_beyond_u32_are_encode_errors() {
    let mut writer = Writer::new();
    assert_eq!(
        TransitionSource::State(u32::MAX as usize + 1).encode(&mut writer).unwrap_err(),
        EncodeError::Overflow {
            what: "state index",
            value: u32::MAX as usize + 1
        }
    );
}

fn value_set_with_duplicate_targets() -> Vec<u8> {
    let entry = FixedAnimationEntry::<Compiled> {
        key: active(0),
        value: AnimatedValue::Bool(true),
    };
    let mut writer = Writer::new();
    writer.u8(0);
    writer.u32(2);
    entry.encode(&mut writer).unwrap();
    entry.encode(&mut writer).unwrap();
    writer.into_bytes()
}

fn extern_table_with_duplicate_values() -> Vec<u8> {
    let mut writer = Writer::new();
    writer.u32(2);
    for _ in 0..2 {
        writer.string("Body").unwrap();
        writer.u32(0);
    }
    writer.into_bytes()
}

fn layer_with_dangling_default_state() -> Vec<u8> {
    let mut writer = Writer::new();
    writer.string("Layer").unwrap();
    LayerSettings::default().encode(&mut writer).unwrap();
    writer.u8(1);
    writer.u32(0);
    writer.u32(0);
    writer.u32(0);
    writer.u32(0);
    writer.into_bytes()
}

fn layer_with_machine_before_its_parent() -> Vec<u8> {
    let mut writer = Writer::new();
    writer.string("Layer").unwrap();
    LayerSettings::default().encode(&mut writer).unwrap();
    writer.u8(0);
    writer.u32(1);
    writer.string("Child").unwrap();
    writer.u8(1);
    writer.u32(0);
    writer.u8(0);
    writer.into_bytes()
}

fn layer_with_dangling_machine_default() -> Vec<u8> {
    let mut writer = Writer::new();
    writer.string("Layer").unwrap();
    LayerSettings::default().encode(&mut writer).unwrap();
    writer.u8(0);
    writer.u32(1);
    writer.string("Machine").unwrap();
    writer.u8(0);
    writer.u8(1);
    writer.u32(0);
    writer.u32(0);
    writer.u32(0);
    writer.into_bytes()
}

fn duplicate_controller_parameters() -> Vec<u8> {
    let controller = PlayableController {
        playable: PlayableLayer::Fx,
        mode: MergeMode::Append,
        priority: 0,
        path_mode: PathMode::Absolute,
        mask: None,
        controller: AnimatorController {
            parameters: vec![AnimatorParameter::create_int("Twice", None), AnimatorParameter::create_int("Twice", None)],
            layers: Vec::new(),
        },
    };
    encode(&controller)
}

#[rstest]
#[case::extern_out_of_range(encode(&asset(4)), decode::<Extern<Asset>>, DecodeError::ExternOutOfRange { kind: "asset", index: 4, len: 4 })]
#[case::object_path_out_of_range(encode(&object(u32::MAX)), decode::<Extern<ObjectPath>>, DecodeError::ExternOutOfRange { kind: "object path", index: u32::MAX, len: 4 })]
#[case::state_out_of_range(encode(&TransitionSource::State(4)), decode::<TransitionSource>, DecodeError::StateOutOfRange { index: 4, len: 4 })]
#[case::target_out_of_range(encode(&TransitionTarget::State(5)), decode::<TransitionTarget>, DecodeError::StateOutOfRange { index: 5, len: 4 })]
#[case::dangling_default_state(layer_with_dangling_default_state(), decode::<AnimatorLayer>, DecodeError::StateOutOfRange { index: 0, len: 0 })]
#[case::machine_out_of_range(encode(&TransitionSource::MachineExit(2)), decode::<TransitionSource>, DecodeError::MachineOutOfRange { index: 2, len: 2 })]
#[case::machine_target_out_of_range(encode(&TransitionTarget::Machine(3)), decode::<TransitionTarget>, DecodeError::MachineOutOfRange { index: 3, len: 2 })]
#[case::machine_before_its_parent(layer_with_machine_before_its_parent(), decode::<AnimatorLayer>, DecodeError::MachineOutOfRange { index: 0, len: 0 })]
#[case::dangling_machine_default(layer_with_dangling_machine_default(), decode::<AnimatorLayer>, DecodeError::StateOutOfRange { index: 0, len: 0 })]
#[case::unknown_parameter(encode(&Resolved::new("Missing".to_string(), AnimatedValueType::Int)), decode::<ParameterRef>, DecodeError::UnknownParameter("Missing".into()))]
#[case::reference_without_asset(vec![8, 0, 0, 0, 0], decode::<AnimatedValue<()>>, DecodeError::InvalidDiscriminator { type_name: "AnimatedValue<()>", value: 8 })]
#[case::unknown_discriminator(vec![9], decode::<AnimatedValueType>, DecodeError::InvalidDiscriminator { type_name: "AnimatedValueType", value: 9 })]
#[case::unknown_motion(vec![4], decode::<Motion>, DecodeError::InvalidDiscriminator { type_name: "Motion", value: 4 })]
#[case::tree_tag_on_clip(vec![2], decode::<Clip>, DecodeError::InvalidDiscriminator { type_name: "Clip", value: 2 })]
#[case::clip_tag_on_tree(vec![0], decode::<BlendTree>, DecodeError::InvalidDiscriminator { type_name: "BlendTree", value: 0 })]
#[case::duplicate_targets(value_set_with_duplicate_targets(), decode::<InlineAnimation<Compiled>>, DecodeError::Duplicate { what: "animation target", key: format!("{:?}", active(0)) })]
#[case::duplicate_extern_values(extern_table_with_duplicate_values(), decode::<ExternTable<ObjectPath>>, DecodeError::Duplicate { what: "object path", key: "\"Body\"".into() })]
#[case::duplicate_parameters(duplicate_controller_parameters(), decode::<PlayableController>, DecodeError::Duplicate { what: "animator parameter", key: "`Twice`".into() })]
#[case::invalid_tracking_mode(vec![0, 0, 3, 0, 0, 0, 0, 0, 0, 0], decode::<TrackingControl>, DecodeError::InvalidDiscriminator { type_name: "TrackingControlMode", value: 3 })]
#[case::truncated_tracking(vec![0; 9], decode::<TrackingControl>, DecodeError::UnexpectedEnd { offset: 9, needed: 1, remaining: 0 })]
#[case::unknown_blendable_playable(vec![4], decode::<BlendablePlayable>, DecodeError::InvalidDiscriminator { type_name: "BlendablePlayable", value: 4 })]
#[case::unknown_apply_settings(vec![3], decode::<ApplySettings>, DecodeError::InvalidDiscriminator { type_name: "ApplySettings", value: 3 })]
#[case::unknown_playback_order(vec![4], decode::<PlaybackOrder<Compiled>>, DecodeError::InvalidDiscriminator { type_name: "PlaybackOrder", value: 4 })]
#[case::audio_source_out_of_range(encode(&PlayAudio { source: Some(object(4)), ..audio_fixture(PlaybackOrder::Random) }), decode::<PlayAudio<Compiled>>, DecodeError::ExternOutOfRange { kind: "object path", index: 4, len: 4 })]
#[case::audio_clip_out_of_range(encode(&PlayAudio { clips: AudioSetting { value: vec![asset(4)], apply: ApplySettings::Always }, ..audio_fixture(PlaybackOrder::Random) }), decode::<PlayAudio<Compiled>>, DecodeError::ExternOutOfRange { kind: "asset", index: 4, len: 4 })]
fn malformed_values_are_rejected<T: Decode + Debug>(
    #[case] bytes: Vec<u8>,
    #[case] decode: fn(&[u8]) -> Result<T, DecodeError>,
    #[case] expected: DecodeError,
) {
    assert_eq!(decode(&bytes).unwrap_err(), expected);
}

fn controller(playable: PlayableLayer, parameters: Vec<AnimatorParameter>, layers: Vec<AnimatorLayer>) -> PlayableController {
    PlayableController {
        playable,
        mode: MergeMode::Append,
        priority: 0,
        path_mode: PathMode::Absolute,
        mask: None,
        controller: AnimatorController { parameters, layers },
    }
}

fn layer_driven_by(name: &str, value_type: AnimatedValueType) -> AnimatorLayer {
    AnimatorLayer {
        name: "Layer".into(),
        settings: LayerSettings::default(),
        default_state: Some(0),
        machines: vec![],
        states: vec![state("A", None), state("B", None)],
        transitions: vec![AnimatorTransition {
            from: TransitionSource::State(0),
            to: TransitionTarget::State(1),
            duration: 0.0,
            conditions: vec![AnimatorCondition::If(Resolved::new(name.into(), value_type))],
        }],
    }
}

#[rstest]
fn layers_resolve_parameters_against_their_own_controller() {
    let avatar = Avatar {
        controllers: vec![
            controller(
                PlayableLayer::Fx,
                vec![AnimatorParameter::create_bool("Own", None)],
                vec![layer_driven_by("Own", AnimatedValueType::Bool)],
            ),
            controller(PlayableLayer::Gesture, vec![AnimatorParameter::create_bool("Other", None)], vec![]),
        ],
        ..Avatar::default()
    };
    assert_eq!(decode_avatar(&encode_avatar(&avatar).unwrap()).unwrap(), avatar);

    let crossed = Avatar {
        controllers: vec![
            controller(
                PlayableLayer::Fx,
                vec![AnimatorParameter::create_bool("Own", None)],
                vec![layer_driven_by("Other", AnimatedValueType::Bool)],
            ),
            controller(PlayableLayer::Gesture, vec![AnimatorParameter::create_bool("Other", None)], vec![]),
        ],
        ..Avatar::default()
    };
    assert_eq!(
        decode_avatar(&encode_avatar(&crossed).unwrap()).unwrap_err(),
        DecodeError::UnknownParameter("Other".into())
    );
}

#[rstest]
fn the_menu_resolves_parameters_against_every_list() {
    let menu = vec![
        MenuItem::Toggle {
            name: "Own".into(),
            parameter: Resolved::new("Own".into(), AnimatedValueType::Bool),
            value: AnimatedValue::Bool(true),
        },
        MenuItem::Button {
            name: "Expr".into(),
            parameter: Resolved::new("Expr".into(), AnimatedValueType::Int),
            value: AnimatedValue::Int(1),
        },
    ];
    let avatar = Avatar {
        expression_parameters: vec![ExpressionParameter {
            name: "Expr".into(),
            type_default: ExpressionParameterTypeDefault::Int {
                width: ExpressionParameterWidth::Unspecified,
                default: None,
            },
            saved: false,
            synced: true,
        }],
        controllers: vec![controller(PlayableLayer::Fx, vec![AnimatorParameter::create_bool("Own", None)], vec![])],
        menu,
        ..Avatar::default()
    };
    assert_eq!(decode_avatar(&encode_avatar(&avatar).unwrap()).unwrap(), avatar);
}

#[rstest]
fn conflicting_parameter_types_are_rejected() {
    let avatar = Avatar {
        expression_parameters: vec![ExpressionParameter {
            name: "Same".into(),
            type_default: ExpressionParameterTypeDefault::Bool(None),
            saved: false,
            synced: true,
        }],
        controllers: vec![controller(PlayableLayer::Fx, vec![AnimatorParameter::create_float("Same", None)], vec![])],
        ..Avatar::default()
    };
    assert_eq!(
        decode_avatar(&encode_avatar(&avatar).unwrap()).unwrap_err(),
        DecodeError::ConflictingParameter {
            name: "Same".into(),
            first: AnimatedValueType::Bool,
            second: AnimatedValueType::Float,
        }
    );
}

#[rstest]
fn externals_lead_the_avatar_payload_and_size_the_tables() {
    let mut externals = Externals::default();
    let body = externals.object_paths.intern(Unresolved::located("Body".into(), at(3)));
    externals.object_paths.intern(Unresolved::located("Body".into(), at(5)));
    let skin = externals.assets.intern(Unresolved::new(AssetLocator::Guid("abc".into())));
    let avatar = Avatar {
        externals,
        controllers: vec![controller(
            PlayableLayer::Fx,
            vec![],
            vec![AnimatorLayer {
                name: "Skin".into(),
                settings: LayerSettings::default(),
                default_state: None,
                machines: vec![],
                states: vec![state(
                    "Only",
                    Some(Motion::Clip(Clip::Inline(InlineAnimation::Fixed(ValueSet::from([FixedAnimationEntry {
                        key: AnimatedTarget::Renderer(AnimatedRendererTarget {
                            path: body,
                            renderer_type: "UnityEngine.SkinnedMeshRenderer".into(),
                            property: AnimatedRendererProperty::Material { slot: 0 },
                        }),
                        value: AnimatedValue::ObjectReference(skin),
                    }]))))),
                )],
                transitions: vec![],
            }],
        )],
        ..Avatar::default()
    };

    let blob = encode_avatar(&avatar).unwrap();
    assert_eq!(&blob[HEADER_LEN..HEADER_LEN + 4], [1, 0, 0, 0]);
    assert_eq!(&blob[HEADER_LEN + 4..HEADER_LEN + 12], [4, 0, 0, 0, b'B', b'o', b'd', b'y']);
    assert_eq!(decode_avatar(&blob).unwrap(), avatar);

    let mut without_assets = avatar.clone();
    without_assets.externals.assets = Default::default();
    assert_eq!(
        decode_avatar(&encode_avatar(&without_assets).unwrap()).unwrap_err(),
        DecodeError::ExternOutOfRange {
            kind: "asset",
            index: 0,
            len: 0
        }
    );
}

#[rstest]
fn an_empty_avatar_is_a_header_and_four_empty_tables_and_lists() {
    let blob = encode_avatar(&Avatar::default()).unwrap();
    assert_eq!(blob.len(), HEADER_LEN + 3 * 4 + 1 + 3 * 4);
    assert_eq!(&blob[..4], AVATAR_MAGIC);
    assert_eq!(decode_avatar(&blob).unwrap(), Avatar::default());
    assert_eq!(
        decode_diagnostics(&blob).unwrap_err(),
        DecodeError::UnexpectedKind {
            expected: BlobKind::Diagnostics,
            found: BlobKind::Avatar
        }
    );
}

fn controlling(playable: PlayableLayer, layer: LayerRef) -> PlayableController {
    let mut control = state("Control", None);
    control.behaviors.push(Behavior::LayerControl(LayerControl {
        layer,
        goal_weight: 1.0,
        blend_duration: 0.0,
    }));
    let layer = AnimatorLayer {
        name: "Control".into(),
        settings: LayerSettings::default(),
        default_state: Some(0),
        machines: vec![],
        states: vec![control],
        transitions: vec![],
    };
    controller(playable, vec![], vec![layer])
}

fn layer_named(name: &str) -> AnimatorLayer {
    AnimatorLayer {
        name: name.into(),
        settings: LayerSettings::default(),
        default_state: None,
        machines: vec![],
        states: vec![],
        transitions: vec![],
    }
}

#[rstest]
#[case::first_controller(vec![controlling(PlayableLayer::Fx, LayerRef { controller: 0, layer: 0 })])]
#[case::later_controller(vec![
    controller(PlayableLayer::Fx, vec![], vec![layer_named("A")]),
    controlling(PlayableLayer::Gesture, LayerRef { controller: 1, layer: 0 }),
])]
fn a_layer_control_reaches_a_layer_of_its_own_controller(#[case] controllers: Vec<PlayableController>) {
    let avatar = Avatar {
        controllers,
        ..Avatar::default()
    };
    assert_eq!(decode_avatar(&encode_avatar(&avatar).unwrap()).unwrap(), avatar);
}

#[rstest]
#[case::controller_out_of_range(
    vec![controlling(PlayableLayer::Fx, LayerRef { controller: 1, layer: 0 })],
    DecodeError::LayerInAnotherController { holder: 0, controller: 1 },
)]
#[case::later_controller_of_the_same_playable(
    vec![
        controlling(PlayableLayer::Gesture, LayerRef { controller: 1, layer: 1 }),
        controller(PlayableLayer::Gesture, vec![], vec![layer_named("A"), layer_named("B")]),
    ],
    DecodeError::LayerInAnotherController { holder: 0, controller: 1 },
)]
#[case::earlier_controller_of_the_same_playable(
    vec![controller(PlayableLayer::Fx, vec![], vec![layer_named("A")]), controlling(PlayableLayer::Fx, LayerRef { controller: 0, layer: 0 })],
    DecodeError::LayerInAnotherController { holder: 1, controller: 0 },
)]
#[case::another_playable(
    vec![controlling(PlayableLayer::Fx, LayerRef { controller: 1, layer: 0 }), controller(PlayableLayer::Action, vec![], vec![layer_named("A")])],
    DecodeError::LayerInAnotherController { holder: 0, controller: 1 },
)]
#[case::layer_out_of_range(
    vec![controlling(PlayableLayer::Fx, LayerRef { controller: 0, layer: 1 })],
    DecodeError::LayerOutOfRange { controller: 0, index: 1, len: 1 },
)]
#[case::uncontrollable(
    vec![controlling(PlayableLayer::Base, LayerRef { controller: 0, layer: 0 })],
    DecodeError::UncontrollablePlayable(PlayableLayer::Base),
)]
fn a_layer_control_outside_its_own_controller_is_rejected(#[case] controllers: Vec<PlayableController>, #[case] expected: DecodeError) {
    let avatar = Avatar {
        controllers,
        ..Avatar::default()
    };
    assert_eq!(decode_avatar(&encode_avatar(&avatar).unwrap()).unwrap_err(), expected);
}

#[rstest]
fn layer_indices_beyond_u32_are_encode_errors() {
    let mut writer = Writer::new();
    let error = LayerRef {
        controller: 0,
        layer: u32::MAX as usize + 1,
    }
    .encode(&mut writer)
    .unwrap_err();
    assert_eq!(
        error,
        EncodeError::Overflow {
            what: "layer index",
            value: u32::MAX as usize + 1,
        }
    );
}

#[rstest]
fn trailing_payload_bytes_are_rejected() {
    let mut blob = encode_avatar(&Avatar::default()).unwrap();
    blob.push(0);
    let len = u32::try_from(blob.len() - HEADER_LEN).unwrap();
    blob[12..16].copy_from_slice(&len.to_le_bytes());
    assert_eq!(decode_avatar(&blob).unwrap_err(), DecodeError::TrailingBytes(1));
}

#[rstest]
fn diagnostics_round_trip_with_and_without_locations() {
    let diagnostics = Diagnostics {
        stage: DiagnosticStage::Transform,
        items: vec![
            Diagnostic {
                at: Some(at(9)),
                message: "parameter `Emote` is not declared".into(),
            },
            Diagnostic { at: None, message: "".into() },
        ],
    };
    let blob = encode_diagnostics(&diagnostics).unwrap();
    assert_eq!(&blob[..4], DIAGNOSTICS_MAGIC);
    assert_eq!(decode_diagnostics(&blob).unwrap(), diagnostics);
}

#[rstest]
fn a_transform_failure_becomes_located_transform_diagnostics() {
    let error = compile(
        "local da = require \"declavatar\"\nreturn da.avatar({ menu = { da.toggle(\"Hat\", da.drive_bool(\"Hat\", true)) } })\n",
        "avatar.lua",
        &EvaluateOptions::new(),
    )
    .unwrap_err();
    let diagnostics = Diagnostics::from(&error);
    assert_eq!(diagnostics.stage, DiagnosticStage::Transform);
    assert_eq!(diagnostics.items.len(), 1);
    assert_eq!(diagnostics.items[0].at.as_ref().map(|at| at.chunk.as_str()), Some("avatar.lua"));
    assert_eq!(diagnostics.items[0].message, "parameter `Hat` is not declared");
    assert_eq!(decode_diagnostics(&encode_diagnostics(&diagnostics).unwrap()).unwrap(), diagnostics);
}

#[rstest]
fn a_script_failure_becomes_one_unlocated_script_diagnostic() {
    let error = compile("error(\"boom\")", "avatar.lua", &EvaluateOptions::new()).unwrap_err();
    let diagnostics = Diagnostics::from(&error);
    assert_eq!(diagnostics.stage, DiagnosticStage::Script);
    assert_eq!(diagnostics.items.len(), 1);
    assert_eq!(diagnostics.items[0].at, None);
    assert!(diagnostics.items[0].message.contains("boom"), "{}", diagnostics.items[0].message);
}
