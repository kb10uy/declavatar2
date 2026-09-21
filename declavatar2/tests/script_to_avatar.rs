use declavatar2::{
    CompileError, EvaluateOptions,
    avatar::{
        Avatar,
        controller::{AnimatorCondition, Clip, Motion, TransitionSource, TransitionTarget},
        menu::MenuItem,
    },
    compile,
    core::resolution::Resolved,
    unity::{
        animation::InlineAnimation,
        animator::{AnimatedRendererProperty, AnimatedRendererTarget, AnimatedTarget, AnimatorParameter, MergeMode, PathMode},
        external::AssetLocator,
        value::{AnimatedValue, AnimatedValueType},
    },
    vrchat::{
        expr_parameter::{ExpressionParameter, ExpressionParameterTypeDefault, ExpressionParameterWidth},
        playable_layer::PlayableLayer,
    },
};
use rstest::*;

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
    assert!(parameters.contains(&AnimatorParameter::create_int("GestureLeft", None)));
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
    assert_eq!(expressions.transitions[0].from, TransitionSource::Entry);
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
        assert!(controller.controller.parameters.contains(&AnimatorParameter::create_int("GestureLeft", None)));
    }
    let paths: Vec<_> = avatar.externals.object_paths.entries().iter().map(|entry| entry.value.clone()).collect();
    assert_eq!(paths, ["Hat"]);
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
