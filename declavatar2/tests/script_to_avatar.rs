use std::collections::BTreeMap;

use declavatar2::{
    CompileError, EvaluateOptions,
    avatar::{
        Avatar, Behavior, LayerRef,
        controller::{AnimatorCondition, Clip, Motion, TransitionSource, TransitionTarget},
        menu::{MenuDirection, MenuItem},
    },
    compile,
    core::resolution::Resolved,
    unity::{
        animation::{ClipAttributes, InlineAnimation, Interpolation},
        animator::{AnimatedRendererProperty, AnimatedRendererTarget, AnimatedTarget, AnimatorParameter, AnimatorParameterOrigin, MergeMode, PathMode},
        external::AssetLocator,
        state::GenericValue,
        value::{AnimatedValue, AnimatedValueType},
    },
    vrchat::{
        ApplySettings, AudioSetting, BlendablePlayable, LayerControl, LocomotionControl, ParameterDrive, ParameterDriveTarget, PlayAudio, PlayableLayerControl,
        PlaybackOrder, TemporaryPoseSpace,
        expr_parameter::{ExpressionParameter, ExpressionParameterTypeDefault, ExpressionParameterWidth, ProvidedParameterGroup},
        playable_layer::PlayableLayer,
    },
};
use rstest::*;

const VRCHAT: AnimatorParameterOrigin = AnimatorParameterOrigin::Provided(ProvidedParameterGroup::Vrchat);

/// The script from the Lua API design section of AGENTS.md, compiled all the way to an avatar.
const SCRIPT: &str = r#"local da = require "declavatar"

local Face = da.renderer("Face")
local hat = da.symbol("ENABLE_HAT")

return da.avatar({
    parameters = {
        da.provided("VRChat"),
        da.int("Emote", { default = 42 }),
        hat and da.bool("Hat", { scope = "local" }),
    },
    controllers = {
        da.controller("fx", {
            da.group_layer("Expressions", { driven_by = "Emote" }, {
                da.default { Face:shape("eyelid_L", 0.3) },
                da.option("smile", { Face:shape("smile"), Face:shape("eye_joy", 0.5) }),
            }),
            hat and da.switch_layer("Hat", { driven_by = "Hat" }, { da.object("Hat"):active() }),
        }),
    },
    menu = {
        hat and da.toggle("Hat", da.drive_switch("Hat")),
    },
})
"#;

fn run(symbols: &[&str]) -> Avatar {
    let options = EvaluateOptions::new().symbols(symbols.iter().copied());
    compile(SCRIPT, "avatar.lua", &options).expect("the documented example should compile")
}

fn errors_of(source: &str) -> Vec<String> {
    match compile(source, "avatar.lua", &EvaluateOptions::new()) {
        Err(CompileError::Transform(errors)) => errors.0.iter().map(ToString::to_string).collect(),
        Err(other) => panic!("expected transform errors, got {other}"),
        Ok(_) => panic!("the script should be rejected"),
    }
}

#[rstest]
fn the_documented_example_compiles_to_the_avatar_it_describes() {
    let avatar = run(&["ENABLE_HAT"]);

    assert_eq!(
        avatar.expression_parameters,
        vec![
            ExpressionParameter {
                name: "Emote".into(),
                type_default: ExpressionParameterTypeDefault::Int {
                    width: ExpressionParameterWidth::Unspecified,
                    default: Some(42),
                },
                saved: false,
                synced: true,
            },
            ExpressionParameter {
                name: "Hat".into(),
                type_default: ExpressionParameterTypeDefault::Bool(None),
                saved: false,
                synced: false,
            },
        ]
    );

    assert_eq!(avatar.controllers.len(), 1);
    let fx = &avatar.controllers[0];
    assert_eq!(fx.playable, PlayableLayer::Fx);
    assert_eq!(fx.mode, MergeMode::Append);
    assert_eq!(fx.priority, 0);
    assert_eq!(fx.path_mode, PathMode::Absolute);
    assert_eq!(fx.mask, None);
    assert!(!avatar.externals.needs_relative_root);

    let parameters = &fx.controller.parameters;
    assert!(parameters.contains(&AnimatorParameter::create_int("GestureLeft", None).with_origin(VRCHAT)));
    assert!(parameters.contains(&AnimatorParameter::create_int("Emote", Some(42))));
    assert!(parameters.contains(&AnimatorParameter::create_bool("Hat", None)));

    let layers = &fx.controller.layers;
    assert_eq!(layers.iter().map(|layer| layer.name.clone()).collect::<Vec<_>>(), ["Expressions", "Hat"]);

    let expressions = &layers[0];
    assert_eq!(
        expressions.states.iter().map(|state| state.name.clone()).collect::<Vec<_>>(),
        ["Default", "smile"]
    );
    let Some(Motion::Clip(Clip::Inline(InlineAnimation::Fixed(smile)))) = &expressions.states[1].motion else {
        panic!("an option plays a fixed clip");
    };
    let face = avatar
        .externals
        .object_paths
        .entries()
        .iter()
        .position(|entry| entry.value == "Face")
        .expect("the face renderer is an external reference");
    let shapes: Vec<_> = smile
        .entries()
        .map(|(key, entry)| match key {
            AnimatedTarget::Renderer(AnimatedRendererTarget {
                path,
                property: AnimatedRendererProperty::BlendShape { name },
                ..
            }) => {
                assert_eq!(path.index() as usize, face);
                (name.clone(), entry.value.clone())
            }
            other => panic!("unexpected target {other:?}"),
        })
        .collect();
    assert_eq!(
        shapes,
        vec![
            ("eye_joy".into(), AnimatedValue::Float(0.5)),
            ("eyelid_L".into(), AnimatedValue::Float(0.3)),
            ("smile".into(), AnimatedValue::Float(1.0)),
        ]
    );
    let emote = Resolved::new("Emote".to_owned(), AnimatedValueType::Int);
    assert_eq!(expressions.transitions[0].from, TransitionSource::Entry(None));
    assert_eq!(expressions.transitions[0].to, TransitionTarget::State(1));
    assert_eq!(expressions.transitions[0].conditions, vec![AnimatorCondition::Equals(emote, 1)]);

    let hat = &layers[1];
    assert_eq!(hat.states.iter().map(|state| state.name.clone()).collect::<Vec<_>>(), ["Disabled", "Enabled"]);
    assert_eq!(
        hat.transitions[0].conditions,
        vec![AnimatorCondition::If(Resolved::new("Hat".to_owned(), AnimatedValueType::Bool))]
    );

    assert_eq!(
        avatar.menu,
        vec![MenuItem::Toggle {
            name: "Hat".into(),
            parameter: Resolved::new("Hat".to_owned(), AnimatedValueType::Bool),
            value: AnimatedValue::Bool(true),
        }]
    );

    let paths: Vec<_> = avatar.externals.object_paths.entries().iter().map(|entry| entry.value.clone()).collect();
    assert_eq!(paths, ["Face", "Hat"]);
    let mut face_lines: Vec<_> = avatar.externals.object_paths.entries()[0].referenced_at.iter().map(|at| at.line).collect();
    face_lines.sort_unstable();
    assert_eq!(face_lines, [15, 16]);
    assert!(avatar.externals.component_types.is_empty());
    assert!(avatar.externals.assets.is_empty());
}

#[rstest]
fn a_symbol_the_host_withholds_drops_everything_guarded_by_it() {
    let avatar = run(&[]);

    assert_eq!(avatar.expression_parameters.len(), 1);
    assert_eq!(avatar.controllers[0].controller.layers.len(), 1);
    assert!(avatar.menu.is_empty());
}

#[rstest]
fn compiling_the_same_script_twice_gives_the_same_avatar() {
    assert_eq!(run(&["ENABLE_HAT"]), run(&["ENABLE_HAT"]));
}

#[rstest]
fn every_mistake_is_reported_with_the_line_that_made_it() {
    let errors = errors_of(
        r#"local da = require "declavatar"
return da.avatar({
    parameters = {
        da.int("Emote"),
        da.int("Emote"),
    },
    controllers = {
        da.controller("fx", {
            da.group_layer("Expressions", { driven_by = "Missing" }, {}),
            da.switch_layer("Hat", { driven_by = "Emote" }, {}),
        }),
    },
    menu = {
        da.toggle("Hat", da.drive_switch("Nope")),
    },
})
"#,
    );

    assert_eq!(
        errors,
        vec![
            "avatar.lua:5: parameter `Emote` is declared more than once",
            "avatar.lua:9: parameter `Missing` is not declared",
            "avatar.lua:10: parameter `Emote` is Int, but Bool is needed here",
            "avatar.lua:14: layer `Nope` is not declared",
        ]
    );
}

#[rstest]
fn controllers_are_bound_per_playable_layer_with_how_they_are_applied() {
    let avatar = compile(
        r#"local da = require "declavatar"
return da.avatar({
    parameters = {
        da.provided("VRChat"),
        da.bool("Hat"),
    },
    controllers = {
        da.controller("gesture", { mode = "replace", priority = -10, mask = da.asset.path("Assets/Hands.mask") }, {
            da.switch_layer("Fist", { driven_by = "Hat" }, { da.object("Hat"):active() }),
        }),
        da.controller("fx", { path_mode = "relative" }, {
            da.switch_layer("Hat", { driven_by = "Hat" }, { da.object("Hat"):active() }),
        }),
    },
})
"#,
        "avatar.lua",
        &EvaluateOptions::new(),
    )
    .expect("the script should compile");

    let playables: Vec<_> = avatar.controllers.iter().map(|controller| controller.playable).collect();
    assert_eq!(playables, [PlayableLayer::Gesture, PlayableLayer::Fx]);

    let gesture = &avatar.controllers[0];
    assert_eq!(gesture.mode, MergeMode::Replace);
    assert_eq!(gesture.priority, -10);
    assert_eq!(gesture.path_mode, PathMode::Absolute);
    let mask = gesture.mask.expect("mask should be interned");
    assert_eq!(avatar.externals.assets.get(mask).value, AssetLocator::Path("Assets/Hands.mask".into()));
    assert_eq!(
        avatar.externals.assets.get(mask).referenced_at.iter().map(|at| at.line).collect::<Vec<_>>(),
        [8]
    );

    let fx = &avatar.controllers[1];
    assert_eq!(fx.mode, MergeMode::Append);
    assert_eq!(fx.path_mode, PathMode::Relative);
    assert!(avatar.externals.needs_relative_root);

    for controller in &avatar.controllers {
        assert!(controller.controller.parameters.contains(&AnimatorParameter::create_bool("Hat", None)));
        assert!(
            controller
                .controller
                .parameters
                .contains(&AnimatorParameter::create_int("GestureLeft", None).with_origin(VRCHAT))
        );
    }
    let paths: Vec<_> = avatar.externals.object_paths.entries().iter().map(|entry| entry.value.clone()).collect();
    assert_eq!(paths, ["Hat"]);
}

const STATE_FEATURES: &str = r#"local da = require "declavatar"

local Body = da.renderer("Body", "UnityEngine.MeshRenderer")

return da.avatar({
    parameters = {
        da.provided("VRChat"),
        da.int("Emote"),
        da.float("Blend"),
        da.bool("Coin", { scope = "internal" }),
    },
    controllers = {
        da.controller("fx", {
            da.raw.layer("Raw", {}, {
                da.raw.state("Wave", {
                    motion = da.raw.keyed_clip({ length = 2, loop_time = true }, {
                        da.raw.keyframe(0, { Body:shape("smile", 0), Body:enabled(false) }),
                        da.raw.keyframe(1, { interpolation = da.raw.bezier(0.25, 0, 0.75, 1) }, { Body:shape("smile", 1) }),
                        da.raw.keyframe(0.5, { Body:enabled(true), Body:serialized("m_Quality", 2) }),
                    }),
                    behaviors = {
                        da.drive_add("Emote", 1),
                        da.drive_random_bool("Coin"),
                        da.drive_copy("GestureLeftWeight", "Blend", { from_range = { 0, 1 }, to_range = { -1, 1 } }),
                        da.behavior("Example.CustomStateBehaviour", { goalWeight = 1, layer = 3, blendableLayers = { true, false } }),
                    },
                }, {
                    da.raw.transition("Wave", {}),
                }),
            }),
            da.blend_layer("Face", {
                da.puppet_layer("Brow", { driven_by = "Blend" }, { da.keyframe(0, { Body:shape("brow", 0) }), da.keyframe(1, { Body:shape("brow", 1) }) }),
            }),
        }),
    },
    menu = {
        da.radial("Blend", "Blend"),
        da.four_axis("Move", { up = da.axis("Blend", { positive = "Up" }), down = "Blend", left = "Blend", right = "Blend" }),
    },
})
"#;

#[rstest]
fn state_features_compile_through_to_the_avatar() {
    let avatar = compile(STATE_FEATURES, "avatar.lua", &EvaluateOptions::new()).expect("the script should compile");
    let controller = &avatar.controllers[0].controller;

    let origin = |name: &str| controller.parameters.iter().find(|parameter| parameter.name == name).unwrap().origin;
    assert_eq!(origin("Emote"), AnimatorParameterOrigin::Declared);
    assert_eq!(origin("Coin"), AnimatorParameterOrigin::Declared);
    assert_eq!(origin("IsLocal"), VRCHAT);
    assert_eq!(origin("Face/Brow"), AnimatorParameterOrigin::Generated);

    let state = &controller.layers[0].states[0];
    let Some(Motion::Clip(Clip::Inline(InlineAnimation::Keyed(keyed)))) = &state.motion else {
        panic!("expected a keyed clip, got {:?}", state.motion);
    };
    assert_eq!(
        keyed.attributes,
        ClipAttributes {
            length: 2.0,
            loop_time: true,
            ..ClipAttributes::default()
        }
    );
    let curves: Vec<_> = keyed
        .curves
        .entries()
        .map(|(key, entry)| {
            let AnimatedTarget::Renderer(renderer) = key else {
                panic!("expected a renderer target");
            };
            let segments: Vec<_> = entry
                .curve
                .rest
                .iter()
                .map(|(interpolation, keyframe)| (*interpolation, keyframe.time))
                .collect();
            (renderer.property.clone(), entry.curve.first.time, segments)
        })
        .collect();
    assert_eq!(
        curves,
        vec![
            (AnimatedRendererProperty::Enabled, 0.0, vec![(Interpolation::Constant, 0.5)]),
            (
                AnimatedRendererProperty::BlendShape { name: "smile".into() },
                0.0,
                vec![(
                    Interpolation::Bezier {
                        x1: 0.25,
                        y1: 0.0,
                        x2: 0.75,
                        y2: 1.0
                    },
                    1.0
                )]
            ),
            (AnimatedRendererProperty::Serialized { name: "m_Quality".into() }, 0.5, vec![]),
        ]
    );

    let drives: Vec<_> = state
        .behaviors
        .iter()
        .filter_map(|behavior| match behavior {
            Behavior::ParameterDrive(ParameterDrive { target }) => Some(target.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        drives,
        vec![
            ParameterDriveTarget::Add {
                parameter: Resolved::new("Emote".into(), AnimatedValueType::Int),
                value: AnimatedValue::Int(1),
            },
            ParameterDriveTarget::RandomBool {
                parameter: Resolved::new("Coin".into(), AnimatedValueType::Bool),
                chance: 0.5,
            },
            ParameterDriveTarget::RangedCopy {
                from: Resolved::new("GestureLeftWeight".into(), AnimatedValueType::Float),
                from_range: [0.0, 1.0],
                to: Resolved::new("Blend".into(), AnimatedValueType::Float),
                to_range: [-1.0, 1.0],
            },
        ]
    );

    let Some(Behavior::Generic(generic)) = state.behaviors.last() else {
        panic!("expected a generic behavior last");
    };
    let type_entry = avatar.externals.component_types.get(generic.type_name);
    assert_eq!(type_entry.value, "Example.CustomStateBehaviour");
    assert_eq!(type_entry.referenced_at.iter().map(|at| at.line).collect::<Vec<_>>(), [25]);
    assert_eq!(
        generic.fields,
        BTreeMap::from([
            (
                "blendableLayers".to_owned(),
                GenericValue::List(vec![GenericValue::Bool(true), GenericValue::Bool(false)])
            ),
            ("goalWeight".to_owned(), GenericValue::Int(1)),
            ("layer".to_owned(), GenericValue::Int(3)),
        ])
    );

    let blend = Resolved::new("Blend".to_owned(), AnimatedValueType::Float);
    assert_eq!(
        avatar.menu[0],
        MenuItem::Radial {
            name: "Blend".into(),
            parameter: blend.clone(),
        }
    );
    let MenuItem::FourAxis { up, down, .. } = &avatar.menu[1] else {
        panic!("expected a four-axis control");
    };
    assert_eq!(
        (up, down),
        (
            &MenuDirection {
                parameter: blend.clone(),
                label: Some("Up".into()),
            },
            &MenuDirection { parameter: blend, label: None },
        )
    );
}

#[rstest]
fn state_drives_are_checked_against_their_parameters() {
    let errors = errors_of(
        r#"local da = require "declavatar"
return da.avatar({
    parameters = { da.bool("Hat"), da.int("Emote") },
    controllers = {
        da.controller("fx", {
            da.raw.layer("Raw", {}, {
                da.raw.state("Only", { behaviors = {
                    da.drive_add("Hat", 1),
                    da.drive_random_float("Emote", 0, 1),
                    da.behavior("Missing.Behaviour"),
                } }),
            }),
        }),
    },
})
"#,
    );

    assert_eq!(errors, vec!["avatar.lua:8: `add` cannot be applied to parameter `Hat` of type Bool"]);
}

const LAYER_CONTROLS: &str = r#"local da = require "declavatar"

local Body = da.renderer("Body")

return da.avatar({
    parameters = { da.bool("Hat"), da.float("Blend"), da.int("Voice") },
    controllers = {
        da.controller("fx", {
            da.switch_layer("Hat", {}, { da.object("Hat"):active() }),
            da.raw.layer("Control", {}, {
                da.raw.state("Idle", { behaviors = {
                    da.layer_control("Hat", { goal_weight = 0, blend_duration = 0.5 }),
                    da.layer_control("Face"),
                    da.layer_control("Control"),
                    da.locomotion(false),
                    da.pose_space("enter", { delay = 0.5, fixed_delay = false }),
                    da.playable_control("action", { goal_weight = 0 }),
                    da.play_audio("Speaker", { parameter = "Voice", volume = { 0.5, 1 } }, { "Hello", "Bye" }),
                    da.play_audio("", {}),
                } }),
            }),
            da.blend_layer("Face", {
                da.puppet_layer("Brow", { driven_by = "Blend" }, { da.keyframe(0, { Body:shape("brow", 0) }), da.keyframe(1, { Body:shape("brow") }) }),
            }),
        }),
        da.controller("gesture", {
            da.switch_layer("Wave", { driven_by = "Hat" }, { da.object("Wave"):active() }),
        }),
        da.controller("fx", { priority = 10 }, {
            da.raw.layer("Late", {}, {
                da.raw.state("Only", { behaviors = { da.layer_control("Late") } }),
            }),
        }),
    },
})
"#;

fn behaviors_of(avatar: &Avatar, controller: usize, layer: usize) -> &[Behavior] {
    &avatar.controllers[controller].controller.layers[layer].states[0].behaviors
}

fn layer_control(controller: usize, layer: usize, goal_weight: f64, blend_duration: f64) -> Behavior {
    Behavior::LayerControl(LayerControl {
        layer: LayerRef { controller, layer },
        goal_weight,
        blend_duration,
    })
}

#[rstest]
fn a_layer_control_reaches_a_layer_of_its_own_controller_by_name() {
    let avatar = compile(LAYER_CONTROLS, "avatar.lua", &EvaluateOptions::new()).expect("the script should compile");

    assert_eq!(
        behaviors_of(&avatar, 0, 1)[..3],
        [layer_control(0, 0, 0.0, 0.5), layer_control(0, 2, 1.0, 0.0), layer_control(0, 1, 1.0, 0.0)]
    );
    assert_eq!(behaviors_of(&avatar, 2, 0), [layer_control(2, 0, 1.0, 0.0)]);

    let blob = declavatar2::interop::encode_avatar(&avatar).expect("the avatar should encode");
    assert_eq!(declavatar2::interop::decode_avatar(&blob).expect("the blob should decode"), avatar);
}

#[rstest]
fn the_other_vrchat_behaviors_compile_through_to_the_avatar() {
    let avatar = compile(LAYER_CONTROLS, "avatar.lua", &EvaluateOptions::new()).expect("the script should compile");
    let behaviors = &behaviors_of(&avatar, 0, 1)[3..];

    let speaker = avatar
        .externals
        .object_paths
        .entries()
        .iter()
        .position(|entry| entry.value == "Speaker")
        .expect("the source should be interned");
    let clips: Vec<_> = ["Hello", "Bye"]
        .map(|name| {
            let locator = AssetLocator::Named {
                asset_type: "UnityEngine.AudioClip".into(),
                name: name.into(),
            };
            let index = avatar
                .externals
                .assets
                .entries()
                .iter()
                .position(|entry| entry.value == locator)
                .expect("the clip should be interned");
            assert_eq!(
                avatar.externals.assets.entries()[index]
                    .referenced_at
                    .iter()
                    .map(|at| at.line)
                    .collect::<Vec<_>>(),
                [18]
            );
            index as u32
        })
        .into();

    let [
        Behavior::LocomotionControl(locomotion),
        Behavior::TemporaryPoseSpace(pose_space),
        Behavior::PlayableLayerControl(playable),
        Behavior::PlayAudio(audio),
        Behavior::PlayAudio(root_audio),
    ] = behaviors
    else {
        panic!("expected the behaviors in the written order, got {behaviors:?}");
    };
    assert_eq!(*locomotion, LocomotionControl { disable_locomotion: true });
    assert_eq!(
        *pose_space,
        TemporaryPoseSpace {
            enter: true,
            fixed_delay: false,
            delay: 0.5,
        }
    );
    assert_eq!(
        *playable,
        PlayableLayerControl {
            playable: BlendablePlayable::Action,
            goal_weight: 0.0,
            blend_duration: 0.0,
        }
    );

    assert_eq!(audio.source.map(|source| source.index() as usize), Some(speaker));
    assert_eq!(audio.order, PlaybackOrder::Parameter(Resolved::new("Voice".into(), AnimatedValueType::Int)));
    assert_eq!(audio.clips.value.iter().map(|clip| clip.index()).collect::<Vec<_>>(), clips);
    assert_eq!(
        audio.volume,
        AudioSetting {
            value: [0.5, 1.0],
            apply: ApplySettings::IfStopped,
        }
    );

    let PlayAudio { source, order, clips, .. } = root_audio;
    assert_eq!((source, order, clips.value.len()), (&None, &PlaybackOrder::Random, 0));
}

#[rstest]
fn a_layer_control_that_cannot_be_resolved_is_reported_where_it_was_written() {
    let errors = errors_of(
        r#"local da = require "declavatar"

local function control(name, behavior)
    return da.raw.layer(name, {}, { da.raw.state("Only", { behaviors = { behavior } }) })
end

return da.avatar({
    parameters = { da.bool("Hat"), da.float("Blend") },
    controllers = {
        da.controller("fx", {
            da.switch_layer("Hat", {}, { da.object("Hat"):active() }),
            da.blend_layer("Face", {
                da.puppet_layer("Brow", { driven_by = "Blend" }, { da.keyframe(0, { da.renderer("Body"):shape("brow") }) }),
            }),
            control("Missing", da.layer_control("Nowhere")),
            control("Crossing", da.layer_control("Wave")),
            control("Merged", da.layer_control("Brow")),
            control("ByIndex", da.behavior("VRC.SDK3.Avatars.Components.VRCAnimatorLayerControl", { layer = 1, goalWeight = 1 })),
            control("ByShortIndex", da.behavior("VRCAnimatorLayerControl", { layer = 1 })),
            control("Voice", da.play_audio("Speaker", { parameter = "Blend" }, {})),
            control("Elsewhere", da.layer_control("Late")),
        }),
        da.controller("gesture", {
            da.switch_layer("Wave", { driven_by = "Hat" }, { da.object("Wave"):active() }),
        }),
        da.controller("base", {
            control("Grounded", da.layer_control("Hat")),
        }),
        da.controller("sitting", {
            control("Seated", da.layer_control("Nowhere")),
        }),
        da.controller("fx", { priority = 10 }, {
            control("Late", da.layer_control("Hat")),
        }),
    },
})
"#,
    );

    assert_eq!(
        errors,
        vec![
            "avatar.lua:15: layer `Nowhere` is not declared",
            "avatar.lua:16: layer `Wave` is in a Gesture controller, but a layer control can only reach layers of its own Fx playable layer",
            "avatar.lua:17: layer `Brow` is merged into blend layer `Face`, so its weight cannot be controlled on its own",
            "avatar.lua:18: `VRC.SDK3.Avatars.Components.VRCAnimatorLayerControl` takes a layer index that the script cannot know; write `da.layer_control(layer, ...)` with the layer name instead",
            "avatar.lua:19: `VRCAnimatorLayerControl` takes a layer index that the script cannot know; write `da.layer_control(layer, ...)` with the layer name instead",
            "avatar.lua:20: parameter `Blend` is Float, but Int is needed here",
            "avatar.lua:21: layer `Late` is in another Fx controller (priority 10), but a layer control can only reach layers of the controller that holds its state",
            "avatar.lua:27: a layer control cannot be used in a Base controller; only Action, Fx, Gesture and Additive layers can be controlled",
            "avatar.lua:30: a layer control cannot be used in a Sitting controller; only Action, Fx, Gesture and Additive layers can be controlled",
            "avatar.lua:33: layer `Hat` is in another Fx controller (priority 0), but a layer control can only reach layers of the controller that holds its state",
        ]
    );
}

#[rstest]
#[case::same_controller(
    r#"local da = require "declavatar"
return da.avatar({
    parameters = { da.bool("Hat") },
    controllers = {
        da.controller("fx", {
            da.switch_layer("Hat", {}, { da.object("Hat"):active() }),
            da.raw.layer("Control", {}, { da.raw.state("Only", { behaviors = { da.layer_control("Hat") } }) }),
            da.switch_layer("Hat", {}, { da.object("Hat"):active() }),
        }),
    },
})
"#,
    "avatar.lua:8: layer `Hat` is declared more than once"
)]
#[case::another_controller(
    r#"local da = require "declavatar"
return da.avatar({
    parameters = { da.bool("Hat") },
    controllers = {
        da.controller("fx", {
            da.switch_layer("Hat", {}, { da.object("Hat"):active() }),
            da.raw.layer("Control", {}, { da.raw.state("Only", { behaviors = { da.layer_control("Hat") } }) }),
        }),
        da.controller("fx", { priority = 10 }, {
            da.switch_layer("Hat", {}, { da.object("Hat"):active() }),
        }),
    },
})
"#,
    "avatar.lua:10: layer `Hat` is declared more than once"
)]
fn a_layer_name_written_twice_is_rejected_before_a_layer_control_could_pick_one(#[case] source: &str, #[case] expected: &str) {
    assert_eq!(errors_of(source), vec![expected]);
}

#[rstest]
fn a_script_error_and_a_transform_error_are_told_apart() {
    let script = compile("return 1", "avatar.lua", &EvaluateOptions::new()).unwrap_err();
    assert!(matches!(script, CompileError::Script(_)));

    let transform = compile(
        "local da = require 'declavatar'\nreturn da.avatar({ menu = { da.toggle('X', da.drive_bool('Nope', true)) } })",
        "avatar.lua",
        &EvaluateOptions::new(),
    )
    .unwrap_err();
    assert!(matches!(transform, CompileError::Transform(_)));
    assert_eq!(transform.to_string(), "avatar.lua:2: parameter `Nope` is not declared");
}
