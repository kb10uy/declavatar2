use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::PathBuf,
};

use declavatar2::{
    avatar::{
        AnimatorCondition, AnimatorController, AnimatorLayer, AnimatorState, AnimatorTransition, Avatar, Behavior, BlendTree, Clip, DirectBlendTree,
        DirectField, MenuAxis, MenuItem, Motion, ParameterRef, ParametricBlendTree, ParametricField, PlayableController, Playback, TransitionSource,
        TransitionTarget,
    },
    core::{Compiled, Extern, Resolved, SourceLocation, Unresolved, value_set::ValueSet},
    interop::{Diagnostic, DiagnosticStage, Diagnostics, decode_avatar, decode_diagnostics, encode_avatar, encode_diagnostics},
    unity::{
        AnimatedAnimatorProperty, AnimatedAnimatorTarget, AnimatedComponentProperty, AnimatedComponentTarget, AnimatedGameObjectProperty,
        AnimatedGameObjectTarget, AnimatedRendererProperty, AnimatedRendererTarget, AnimatedTarget, AnimatedValue, AnimatedValueType, AnimatorParameter, Asset,
        AssetLocator, BlendTreeType, ClipAttributes, ComponentType, Curve, Externals, FixedAnimationEntry, GenericStateBehavior, GenericValue, InlineAnimation,
        Interpolation, KeyedAnimation, KeyedAnimationEntry, Keyframe, MergeMode, ObjectPath, PathMode,
    },
    vrchat::{
        ParameterDrive, ParameterDriveTarget, PlayableLayer, TrackingControl, TrackingControlMode, TrackingControlTarget,
        expr_parameter::{ExpressionParameter, ExpressionParameterTypeDefault, ExpressionParameterWidth},
    },
};
use nalgebra::{UnitQuaternion, Vector2, Vector3, Vector4};
use rstest::rstest;

const UPDATE_VARIABLE: &str = "DECLAVATAR2_UPDATE_GOLDEN";

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../assets/interop").join(name)
}

fn check_golden(name: &str, encoded: &[u8]) -> Vec<u8> {
    let path = golden_path(name);
    if std::env::var_os(UPDATE_VARIABLE).is_some() {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, encoded).unwrap();
    }
    let golden = fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}; run with {UPDATE_VARIABLE}=1 to create it", path.display()));
    if golden != encoded {
        let offset = golden.iter().zip(encoded).position(|(a, b)| a != b).unwrap_or(golden.len().min(encoded.len()));
        panic!(
            "encoding differs from {} at offset {offset} (golden {} bytes, encoded {} bytes); if the layout changed on purpose, bump the version in assets/interop-format.md and regenerate with {UPDATE_VARIABLE}=1",
            path.display(),
            golden.len(),
            encoded.len(),
        );
    }
    golden
}

fn at(chunk: &str, line: u32) -> SourceLocation {
    SourceLocation { chunk: chunk.into(), line }
}

fn param(name: &str, value_type: AnimatedValueType) -> ParameterRef {
    Resolved::new(name.into(), value_type)
}

fn int(name: &str) -> ParameterRef {
    param(name, AnimatedValueType::Int)
}

fn float(name: &str) -> ParameterRef {
    param(name, AnimatedValueType::Float)
}

fn boolean(name: &str) -> ParameterRef {
    param(name, AnimatedValueType::Bool)
}

struct Refs {
    body: Extern<ObjectPath>,
    hat: Extern<ObjectPath>,
    light: Extern<ComponentType>,
    material: Extern<Asset>,
    clip: Extern<Asset>,
    mask: Extern<Asset>,
}

fn externals() -> (Externals, Refs) {
    let mut externals = Externals::default();
    let body = externals.object_paths.intern(Unresolved::located("Body".into(), at("avatar.lua", 3)));
    externals.object_paths.intern(Unresolved::located("Body".into(), at("avatar.lua", 3)));
    externals.object_paths.intern(Unresolved::located("Body".into(), at("lib/face.lua", 41)));
    let hat = externals.object_paths.intern(Unresolved::new("Accessories/Hat".into()));
    let light = externals
        .component_types
        .intern(Unresolved::located("UnityEngine.Light".into(), at("avatar.lua", 12)));
    let material = externals.assets.intern(Unresolved::located(
        AssetLocator::Named {
            asset_type: "UnityEngine.Material".into(),
            name: "Skin".into(),
        },
        at("avatar.lua", 8),
    ));
    let clip = externals
        .assets
        .intern(Unresolved::new(AssetLocator::Guid("0123456789abcdef0123456789abcdef".into())));
    let mask = externals.assets.intern(Unresolved::new(AssetLocator::Path("Assets/Masks/Hands.mask".into())));
    externals.needs_relative_root = true;
    (
        externals,
        Refs {
            body,
            hat,
            light,
            material,
            clip,
            mask,
        },
    )
}

fn renderer(path: Extern<ObjectPath>, property: AnimatedRendererProperty) -> AnimatedTarget<Compiled> {
    AnimatedTarget::Renderer(AnimatedRendererTarget {
        path,
        renderer_type: "UnityEngine.SkinnedMeshRenderer".into(),
        property,
    })
}

fn game_object(path: Extern<ObjectPath>, property: AnimatedGameObjectProperty) -> AnimatedTarget<Compiled> {
    AnimatedTarget::GameObject(AnimatedGameObjectTarget { path, property })
}

fn component(
    path: Extern<ObjectPath>,
    component_type: Extern<ComponentType>,
    property: AnimatedComponentProperty,
    value_type: AnimatedValueType,
) -> AnimatedTarget<Compiled> {
    AnimatedTarget::Component(AnimatedComponentTarget {
        path,
        component_type,
        property,
        value_type,
    })
}

fn fixed(entries: impl IntoIterator<Item = (AnimatedTarget<Compiled>, AnimatedValue<Extern<Asset>>)>) -> Motion {
    Motion::Clip(Clip::Inline(InlineAnimation::Fixed(ValueSet::from(
        entries.into_iter().map(|(key, value)| FixedAnimationEntry { key, value }),
    ))))
}

fn every_value_kind(refs: &Refs) -> Motion {
    fixed([
        (
            renderer(refs.body, AnimatedRendererProperty::BlendShape { name: "smile".into() }),
            AnimatedValue::Float(1.0),
        ),
        (
            renderer(refs.body, AnimatedRendererProperty::BlendShape { name: "眉".into() }),
            AnimatedValue::Float(f64::MAX),
        ),
        (renderer(refs.body, AnimatedRendererProperty::Enabled), AnimatedValue::Bool(true)),
        (
            renderer(refs.body, AnimatedRendererProperty::Material { slot: 2 }),
            AnimatedValue::ObjectReference(refs.material),
        ),
        (
            renderer(refs.body, AnimatedRendererProperty::MaterialProperty { name: "_Color".into() }),
            AnimatedValue::Color(Vector4::new(0.25, 0.5, 0.75, 1.0)),
        ),
        (
            renderer(refs.body, AnimatedRendererProperty::Serialized { name: "m_Layer".into() }),
            AnimatedValue::Int(i64::MIN),
        ),
        (game_object(refs.hat, AnimatedGameObjectProperty::Active), AnimatedValue::Bool(false)),
        (
            game_object(refs.hat, AnimatedGameObjectProperty::TransformPosition),
            AnimatedValue::Vector3(Vector3::new(0.0, 1.5, -0.25)),
        ),
        (
            game_object(refs.hat, AnimatedGameObjectProperty::TransformRotationQuaternion),
            AnimatedValue::Quaternion(UnitQuaternion::from_axis_angle(&Vector3::y_axis(), std::f64::consts::FRAC_PI_2)),
        ),
        (
            game_object(refs.hat, AnimatedGameObjectProperty::TransformRotationEuler),
            AnimatedValue::Vector3(Vector3::new(90.0, 0.0, -45.0)),
        ),
        (
            game_object(refs.hat, AnimatedGameObjectProperty::TransformScale),
            AnimatedValue::Vector3(Vector3::new(1.0, 1.0, 1.0)),
        ),
        (
            component(refs.hat, refs.light, AnimatedComponentProperty::Enabled, AnimatedValueType::Bool),
            AnimatedValue::Bool(true),
        ),
        (
            component(
                refs.hat,
                refs.light,
                AnimatedComponentProperty::Serialized { name: "m_Range".into() },
                AnimatedValueType::Float,
            ),
            AnimatedValue::Float(f64::NEG_INFINITY),
        ),
        (
            component(
                refs.hat,
                refs.light,
                AnimatedComponentProperty::Serialized { name: "m_Offset".into() },
                AnimatedValueType::Vector2,
            ),
            AnimatedValue::Vector2(Vector2::new(-1.0, 1.0)),
        ),
        (
            component(
                refs.hat,
                refs.light,
                AnimatedComponentProperty::Serialized { name: "m_Rect".into() },
                AnimatedValueType::Vector4,
            ),
            AnimatedValue::Vector4(Vector4::new(0.0, 0.0, 1.0, 1.0)),
        ),
        (
            component(
                refs.hat,
                refs.light,
                AnimatedComponentProperty::Serialized { name: "m_Count".into() },
                AnimatedValueType::Int,
            ),
            AnimatedValue::Int(i64::MAX),
        ),
        (
            AnimatedTarget::AnimatorSelf(AnimatedAnimatorTarget {
                property: AnimatedAnimatorProperty::ParameterFloatValue { name: float("Weight") },
            }),
            AnimatedValue::Float(0.5),
        ),
    ])
}

fn keyed(refs: &Refs) -> Motion {
    Motion::Clip(Clip::Inline(InlineAnimation::Keyed(KeyedAnimation {
        attributes: ClipAttributes {
            length: 2.5,
            loop_time: true,
            loop_blend: false,
            cycle_offset: 0.1,
        },
        curves: ValueSet::from([
            KeyedAnimationEntry {
                key: renderer(refs.body, AnimatedRendererProperty::BlendShape { name: "smile".into() }),
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
                                y1: 0.1,
                                x2: 0.75,
                                y2: 0.9,
                            },
                            Keyframe {
                                time: 1.0,
                                value: AnimatedValue::Float(0.5),
                            },
                        ),
                    ],
                },
            },
            KeyedAnimationEntry {
                key: game_object(refs.hat, AnimatedGameObjectProperty::Active),
                curve: Curve {
                    first: Keyframe {
                        time: 0.0,
                        value: AnimatedValue::Bool(false),
                    },
                    rest: vec![(
                        Interpolation::Constant,
                        Keyframe {
                            time: 1.0,
                            value: AnimatedValue::Bool(true),
                        },
                    )],
                },
            },
            KeyedAnimationEntry {
                key: renderer(refs.body, AnimatedRendererProperty::Material { slot: 0 }),
                curve: Curve::single(Keyframe {
                    time: 0.0,
                    value: AnimatedValue::ObjectReference(refs.material),
                }),
            },
        ]),
    })))
}

fn trees(refs: &Refs) -> Motion {
    Motion::BlendTree(BlendTree::Parametric(ParametricBlendTree {
        tree_type: BlendTreeType::Linear,
        x: float("Blend"),
        y: None,
        fields: vec![
            ParametricField {
                position: [-1.0, 0.0],
                speed: 1.0,
                motion: fixed([(game_object(refs.hat, AnimatedGameObjectProperty::Active), AnimatedValue::Bool(true))]),
            },
            ParametricField {
                position: [1.0, 0.0],
                speed: 0.5,
                motion: Motion::BlendTree(BlendTree::Parametric(ParametricBlendTree {
                    tree_type: BlendTreeType::Freeform2d,
                    x: float("Blend"),
                    y: Some(float("Weight")),
                    fields: vec![
                        ParametricField {
                            position: [0.0, 1.0],
                            speed: 1.0,
                            motion: Motion::BlendTree(BlendTree::Direct(DirectBlendTree {
                                fields: vec![DirectField {
                                    weight_by: float("Weight"),
                                    speed: 2.0,
                                    motion: Motion::Clip(Clip::External(refs.clip)),
                                }],
                            })),
                        },
                        ParametricField {
                            position: [1.0, -1.0],
                            speed: 1.0,
                            motion: Motion::BlendTree(BlendTree::Parametric(ParametricBlendTree {
                                tree_type: BlendTreeType::Simple2d,
                                x: float("Blend"),
                                y: Some(float("Weight")),
                                fields: vec![],
                            })),
                        },
                        ParametricField {
                            position: [-1.0, -1.0],
                            speed: 1.0,
                            motion: Motion::BlendTree(BlendTree::Parametric(ParametricBlendTree {
                                tree_type: BlendTreeType::Cartesian2d,
                                x: float("Blend"),
                                y: Some(float("Weight")),
                                fields: vec![],
                            })),
                        },
                    ],
                })),
            },
        ],
    }))
}

fn behaviors() -> Vec<Behavior> {
    let drive = |target| Behavior::ParameterDrive(ParameterDrive { target });
    vec![
        drive(ParameterDriveTarget::Set {
            parameter: int("Emote"),
            value: AnimatedValue::Int(1),
        }),
        drive(ParameterDriveTarget::Add {
            parameter: float("Blend"),
            value: AnimatedValue::Float(-0.25),
        }),
        drive(ParameterDriveTarget::RandomInt {
            parameter: int("Emote"),
            range: [0, 7],
        }),
        drive(ParameterDriveTarget::RandomBool {
            parameter: boolean("Hat"),
            chance: 0.5,
        }),
        drive(ParameterDriveTarget::RandomFloat {
            parameter: float("Blend"),
            range: [-1.0, 1.0],
        }),
        drive(ParameterDriveTarget::Copy {
            from: float("Weight"),
            to: float("Blend"),
        }),
        drive(ParameterDriveTarget::RangedCopy {
            from: float("Blend"),
            from_range: [-1.0, 1.0],
            to: int("Emote"),
            to_range: [0.0, 255.0],
        }),
        Behavior::TrackingControl(TrackingControl {
            values: HashMap::from([
                (TrackingControlTarget::Head, TrackingControlMode::Animation),
                (TrackingControlTarget::Eyes, TrackingControlMode::Tracking),
                (TrackingControlTarget::Mouth, TrackingControlMode::Animation),
            ]),
        }),
        Behavior::Generic(GenericStateBehavior {
            type_name: "VRC.SDK3.Avatars.Components.VRCAnimatorLayerControl".into(),
            fields: BTreeMap::from([
                ("blendDuration".to_string(), GenericValue::Float(0.5)),
                ("debugString".to_string(), GenericValue::String(String::new())),
                ("flags".to_string(), GenericValue::List(vec![GenericValue::Bool(true), GenericValue::Int(-1)])),
                ("layer".to_string(), GenericValue::Int(3)),
                (
                    "nested".to_string(),
                    GenericValue::Map(BTreeMap::from([("x".to_string(), GenericValue::List(vec![]))])),
                ),
            ]),
        }),
    ]
}

fn parameters() -> Vec<AnimatorParameter> {
    vec![
        AnimatorParameter::create_int("Emote", Some(42)),
        AnimatorParameter::create_bool("Hat", Some(true)),
        AnimatorParameter::create_float("Blend", None),
        AnimatorParameter::create_bool("顔", None),
        AnimatorParameter::create_int("GestureLeft", None),
        AnimatorParameter::create_float("Weight", Some(1.0)),
        AnimatorParameter::create_int("Big", Some(i32::MAX)),
        AnimatorParameter::create_int("Small", Some(i32::MIN)),
        AnimatorParameter::create_float("Tiny", Some(f32::MIN_POSITIVE)),
    ]
}

fn state(name: &str, motion: Option<Motion>, playback: Playback, write_defaults: bool, behaviors: Vec<Behavior>) -> AnimatorState {
    AnimatorState {
        name: name.into(),
        motion,
        playback,
        write_defaults,
        behaviors,
    }
}

fn fx_layers(refs: &Refs) -> Vec<AnimatorLayer> {
    vec![
        AnimatorLayer {
            name: "Expressions".into(),
            default_state: Some(0),
            states: vec![
                state(
                    "Default",
                    Some(fixed([(
                        renderer(refs.body, AnimatedRendererProperty::BlendShape { name: "eyelid_L".into() }),
                        AnimatedValue::Float(0.3),
                    )])),
                    Playback::default(),
                    false,
                    vec![],
                ),
                state("smile", Some(every_value_kind(refs)), Playback::default(), false, behaviors()),
            ],
            transitions: vec![
                AnimatorTransition {
                    from: TransitionSource::Entry,
                    to: TransitionTarget::State(1),
                    duration: 0.0,
                    conditions: vec![AnimatorCondition::Equals(int("Emote"), 1)],
                },
                AnimatorTransition {
                    from: TransitionSource::State(1),
                    to: TransitionTarget::Exit,
                    duration: 0.25,
                    conditions: vec![AnimatorCondition::NotEqual(int("Emote"), 1), AnimatorCondition::Greater(float("Blend"), 0.5)],
                },
                AnimatorTransition {
                    from: TransitionSource::State(0),
                    to: TransitionTarget::State(1),
                    duration: 0.0,
                    conditions: vec![AnimatorCondition::If(boolean("Hat")), AnimatorCondition::Less(float("Blend"), -0.5)],
                },
                AnimatorTransition {
                    from: TransitionSource::State(1),
                    to: TransitionTarget::State(0),
                    duration: 1.0,
                    conditions: vec![AnimatorCondition::IfNot(boolean("顔"))],
                },
                AnimatorTransition {
                    from: TransitionSource::Entry,
                    to: TransitionTarget::Exit,
                    duration: 0.0,
                    conditions: vec![],
                },
            ],
        },
        AnimatorLayer {
            name: "Puppet".into(),
            default_state: Some(0),
            states: vec![state(
                "Keyed",
                Some(keyed(refs)),
                Playback {
                    speed: 2.0,
                    speed_by: Some(float("Weight")),
                    time_by: Some(float("Blend")),
                },
                true,
                vec![],
            )],
            transitions: vec![],
        },
        AnimatorLayer {
            name: "External".into(),
            default_state: None,
            states: vec![
                state("Clip", Some(Motion::Clip(Clip::External(refs.clip))), Playback::default(), false, vec![]),
                state("Empty", None, Playback::default(), true, vec![]),
            ],
            transitions: vec![],
        },
        AnimatorLayer {
            name: "Trees".into(),
            default_state: Some(1),
            states: vec![
                state("Parametric", Some(trees(refs)), Playback::default(), true, vec![]),
                state(
                    "Direct",
                    Some(Motion::BlendTree(BlendTree::Direct(DirectBlendTree {
                        fields: vec![
                            DirectField {
                                weight_by: float("Weight"),
                                speed: 1.0,
                                motion: fixed([]),
                            },
                            DirectField {
                                weight_by: float("Blend"),
                                speed: 0.0,
                                motion: keyed(refs),
                            },
                        ],
                    }))),
                    Playback::default(),
                    true,
                    vec![],
                ),
            ],
            transitions: vec![],
        },
        AnimatorLayer {
            name: String::new(),
            default_state: None,
            states: vec![],
            transitions: vec![],
        },
    ]
}

fn gesture_layers(refs: &Refs) -> Vec<AnimatorLayer> {
    vec![AnimatorLayer {
        name: "Hat".into(),
        default_state: Some(0),
        states: vec![
            state(
                "Disabled",
                Some(fixed([(game_object(refs.hat, AnimatedGameObjectProperty::Active), AnimatedValue::Bool(false))])),
                Playback::default(),
                false,
                vec![],
            ),
            state(
                "Enabled",
                Some(fixed([(game_object(refs.hat, AnimatedGameObjectProperty::Active), AnimatedValue::Bool(true))])),
                Playback::default(),
                false,
                vec![],
            ),
        ],
        transitions: vec![
            AnimatorTransition {
                from: TransitionSource::State(0),
                to: TransitionTarget::State(1),
                duration: 0.0,
                conditions: vec![AnimatorCondition::If(boolean("Hat"))],
            },
            AnimatorTransition {
                from: TransitionSource::State(1),
                to: TransitionTarget::State(0),
                duration: 0.0,
                conditions: vec![AnimatorCondition::IfNot(boolean("Hat"))],
            },
        ],
    }]
}

fn menu() -> Vec<MenuItem> {
    let axis = |name: &str, positive: Option<&str>, negative: Option<&str>| MenuAxis {
        parameter: float(name),
        positive: positive.map(Into::into),
        negative: negative.map(Into::into),
    };
    vec![
        MenuItem::SubMenu {
            name: "Emotes".into(),
            items: vec![
                MenuItem::Toggle {
                    name: "Smile".into(),
                    parameter: int("Emote"),
                    value: AnimatedValue::Int(1),
                },
                MenuItem::Button {
                    name: "Wave".into(),
                    parameter: int("Emote"),
                    value: AnimatedValue::Int(2),
                },
                MenuItem::Radial {
                    name: "Blend".into(),
                    axis: axis("Blend", None, None),
                },
                MenuItem::TwoAxis {
                    name: "Look".into(),
                    horizontal: axis("Blend", Some("Right"), Some("Left")),
                    vertical: axis("Weight", Some("Up"), None),
                },
                MenuItem::FourAxis {
                    name: "Move".into(),
                    up: axis("Blend", Some("Forward"), None),
                    down: axis("Weight", None, Some("Back")),
                    left: axis("Blend", None, None),
                    right: axis("Weight", Some("R"), Some("L")),
                },
                MenuItem::SubMenu {
                    name: "Empty".into(),
                    items: vec![],
                },
            ],
        },
        MenuItem::Toggle {
            name: "Hat".into(),
            parameter: boolean("Hat"),
            value: AnimatedValue::Bool(true),
        },
        MenuItem::Toggle {
            name: "顔".into(),
            parameter: boolean("顔"),
            value: AnimatedValue::Bool(false),
        },
        MenuItem::Button {
            name: "Half".into(),
            parameter: float("Blend"),
            value: AnimatedValue::Float(0.5),
        },
    ]
}

fn fixture_avatar() -> Avatar {
    let (externals, refs) = externals();
    Avatar {
        expression_parameters: vec![
            ExpressionParameter {
                name: "Emote".into(),
                type_default: ExpressionParameterTypeDefault::Int {
                    width: ExpressionParameterWidth::Specified(8),
                    default: Some(42),
                },
                saved: false,
                synced: true,
            },
            ExpressionParameter {
                name: "Hat".into(),
                type_default: ExpressionParameterTypeDefault::Bool(Some(true)),
                saved: true,
                synced: false,
            },
            ExpressionParameter {
                name: "Blend".into(),
                type_default: ExpressionParameterTypeDefault::Float {
                    width: ExpressionParameterWidth::Unspecified,
                    default: None,
                },
                saved: false,
                synced: true,
            },
            ExpressionParameter {
                name: "顔".into(),
                type_default: ExpressionParameterTypeDefault::Bool(None),
                saved: true,
                synced: true,
            },
            ExpressionParameter {
                name: "Tiny".into(),
                type_default: ExpressionParameterTypeDefault::Float {
                    width: ExpressionParameterWidth::Specified(1),
                    default: Some(f32::MIN_POSITIVE),
                },
                saved: false,
                synced: false,
            },
        ],
        controllers: vec![
            PlayableController {
                playable: PlayableLayer::Fx,
                mode: MergeMode::Append,
                priority: 0,
                path_mode: PathMode::Absolute,
                mask: None,
                controller: AnimatorController {
                    parameters: parameters(),
                    layers: fx_layers(&refs),
                },
            },
            PlayableController {
                playable: PlayableLayer::Gesture,
                mode: MergeMode::Replace,
                priority: -5,
                path_mode: PathMode::Relative,
                mask: Some(refs.mask),
                controller: AnimatorController {
                    parameters: parameters(),
                    layers: gesture_layers(&refs),
                },
            },
        ],
        menu: menu(),
        externals,
    }
}

fn transform_diagnostics() -> Diagnostics {
    Diagnostics {
        stage: DiagnosticStage::Transform,
        items: vec![
            Diagnostic {
                at: Some(at("avatar.lua", 9)),
                message: "parameter `Emote` is not declared".into(),
            },
            Diagnostic {
                at: None,
                message: "menu `Emotes` holds 9 controls, but a menu can hold 8 at most".into(),
            },
            Diagnostic {
                at: Some(at("lib/顔.lua", u32::MAX)),
                message: String::new(),
            },
        ],
    }
}

fn script_diagnostics() -> Diagnostics {
    Diagnostics {
        stage: DiagnosticStage::Script,
        items: vec![Diagnostic {
            at: None,
            message: "avatar.lua:3: attempt to call a nil value (field 'toggle')\nstack traceback:\n\t[C]: in ?".into(),
        }],
    }
}

#[rstest]
fn the_avatar_fixture_matches_its_golden_blob() {
    let avatar = fixture_avatar();
    let golden = check_golden("avatar.bin", &encode_avatar(&avatar).unwrap());
    assert_eq!(decode_avatar(&golden).unwrap(), avatar);
}

#[rstest]
#[case::transform("diagnostics-transform.bin", transform_diagnostics())]
#[case::script("diagnostics-script.bin", script_diagnostics())]
fn the_diagnostics_fixtures_match_their_golden_blobs(#[case] name: &str, #[case] diagnostics: Diagnostics) {
    let golden = check_golden(name, &encode_diagnostics(&diagnostics).unwrap());
    assert_eq!(decode_diagnostics(&golden).unwrap(), diagnostics);
}
