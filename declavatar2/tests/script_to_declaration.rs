use declavatar2::{
    core::{
        phase::Declared,
        resolution::{SourceLocation, Unresolved},
    },
    decl::{
        Avatar,
        behavior::{Animation, Content, Drive},
        layer::{GroupLayer, GroupOption, Layer, SwitchContent, SwitchLayer, SwitchSource},
        menu::MenuItem,
        parameter::{Parameter, ParameterScope, PrimitiveParameter, PrimitiveParameterValue, ProvidedParameterGroup},
    },
    lua::{EvaluateOptions, ScriptError, evaluate},
    unity::{
        animation::FixedAnimationEntry,
        animator::{AnimatedGameObjectProperty, AnimatedGameObjectTarget, AnimatedRendererProperty, AnimatedRendererTarget, AnimatedTarget},
        value::AnimatedValue,
    },
};
use rstest::*;

/// The script from the Lua API design section of AGENTS.md, kept verbatim so that the
/// documented example and the implementation cannot drift apart.
const SCRIPT: &str = r#"local da = require "declavatar"

local Face = da.renderer("Face")
local hat = da.symbol("ENABLE_HAT")

return da.avatar({
    parameters = {
        da.provided("VRChat"),
        da.int("Emote", { default = 42 }),
        hat and da.bool("Hat", { scope = "local" }),
    },
    fx_controller = {
        da.group_layer("Expressions", { driven_by = "Emote" }, {
            da.default { Face:shape("eyelid_L", 0.3) },
            da.option("smile", { Face:shape("smile"), Face:shape("eye_joy", 0.5) }),
        }),
        hat and da.switch_layer("Hat", { driven_by = "Hat" }, { da.object("Hat"):active() }),
    },
    menu = {
        hat and da.toggle("Hat", da.drive_switch("Hat")),
    },
})
"#;

fn at(line: u32) -> Option<SourceLocation> {
    Some(SourceLocation {
        chunk: "avatar.lua".into(),
        line,
    })
}

fn shape(name: &str, value: f64) -> FixedAnimationEntry<Declared> {
    FixedAnimationEntry {
        key: AnimatedTarget::Renderer(AnimatedRendererTarget {
            path: Unresolved::new("Face".into()),
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

fn animation_only(entries: impl IntoIterator<Item = FixedAnimationEntry<Declared>>) -> Content {
    Content {
        animation: Animation::from(entries),
        behaviors: vec![],
    }
}

fn expressions_layer() -> Layer {
    Layer::Group(GroupLayer {
        name: "Expressions".into(),
        driven_by: Some(Unresolved::new("Emote".into())),
        symmetric: None,
        default: Some(animation_only([shape("eyelid_L", 0.3)])),
        options: vec![GroupOption {
            name: "smile".into(),
            content: animation_only([shape("smile", 1.0), shape("eye_joy", 0.5)]),
            at: at(15),
        }],
        at: at(13),
    })
}

fn hat_layer() -> Layer {
    Layer::Switch(SwitchLayer {
        name: "Hat".into(),
        source: Some(SwitchSource::Parameter(Unresolved::new("Hat".into()))),
        content: SwitchContent::Toggle(animation_only([active("Hat", true)])),
        at: at(17),
    })
}

fn run(symbols: &[&str]) -> Avatar {
    let options = EvaluateOptions::new().symbols(symbols.iter().copied());
    evaluate(SCRIPT, "avatar.lua", &options).expect("the documented example should evaluate")
}

#[rstest]
fn the_documented_example_declares_the_avatar_it_describes() {
    let declared = run(&["ENABLE_HAT"]);

    assert_eq!(
        declared,
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
                    at: at(9),
                }),
                Parameter::Primitive(PrimitiveParameter {
                    name: "Hat".into(),
                    value: PrimitiveParameterValue::Bool { default: None },
                    scope: Some(ParameterScope::Local),
                    save: None,
                    at: at(10),
                }),
            ],
            fx_controller: vec![expressions_layer(), hat_layer()],
            menu: vec![MenuItem::Toggle {
                name: "Hat".into(),
                drive: Drive::Switch {
                    layer: Unresolved::new("Hat".into()),
                    value: None,
                },
            }],
            exports: vec![],
        }
    );
}

#[rstest]
fn a_symbol_the_host_withholds_drops_everything_guarded_by_it() {
    let declared = run(&[]);

    assert_eq!(
        declared.parameters,
        vec![
            Parameter::Provided(ProvidedParameterGroup::Vrchat),
            Parameter::Primitive(PrimitiveParameter {
                name: "Emote".into(),
                value: PrimitiveParameterValue::Int {
                    default: Some(42),
                    width: None,
                },
                scope: None,
                save: None,
                at: at(9),
            }),
        ],
    );
    assert_eq!(declared.fx_controller, vec![expressions_layer()]);
    assert!(declared.menu.is_empty());
}

#[rstest]
fn evaluating_the_same_script_twice_gives_the_same_declaration() {
    assert_eq!(run(&["ENABLE_HAT"]), run(&["ENABLE_HAT"]));
}

#[rstest]
#[case::nested_list("da.avatar({ parameters = { { da.int('Emote') } } })", "entry 1 is a list")]
#[case::unknown_option("da.avatar({ parameters = { da.int('Emote', { defualt = 1 }) } })", "da.int: unknown option `defualt`")]
#[case::wrong_kind("da.avatar({ menu = { da.int('Emote') } })", "expected menu item, got parameter")]
#[case::wrong_value("da.avatar({ parameters = { da.int('Emote', { default = 1.5 }) } })", "must be an integer")]
fn a_mistake_is_reported_with_the_line_that_made_it(#[case] expression: &str, #[case] expected: &str) {
    let source = format!("local da = require \"declavatar\"\n\nreturn {expression}\n");
    let error = evaluate(&source, "avatar.lua", &EvaluateOptions::new()).expect_err("the script should be rejected");

    let ScriptError::Script(error) = error else {
        panic!("a builder mistake should surface as a script error, got {error}");
    };
    let message = error.to_string();

    assert!(message.contains(expected), "{message}");
    assert!(message.contains("avatar.lua:3:"), "{message}");
}
