use declavatar2::{
    core::resolution::SourceLocation,
    decl::{
        Avatar,
        controller::Controller,
        parameter::{Parameter, ParameterRepr, ParameterScope, PrimitiveParameter, PrimitiveParameterValue, ProvidedParameterGroup, Signedness},
    },
    transform::{TransformErrorKind, transform},
    unity::animator::{AnimatorParameter, AnimatorParameterOrigin, AnimatorParameterTypeDefault},
    vrchat::{
        expr_parameter::{ExpressionParameterSignedness, ExpressionParameterTypeDefault, ExpressionParameterWidth},
        playable_layer::PlayableLayer,
    },
};
use rstest::rstest;

fn declaration(value: PrimitiveParameterValue, scope: ParameterScope) -> Avatar {
    Avatar {
        parameters: vec![Parameter::Primitive(PrimitiveParameter {
            name: "Test".into(),
            value,
            scope: Some(scope),
            save: None,
            at: Some(SourceLocation {
                chunk: "parameters.lua".into(),
                line: 7,
            }),
        })],
        controllers: vec![Controller::new(PlayableLayer::Fx, vec![])],
        ..Avatar::default()
    }
}

#[rstest]
#[case(None)]
#[case(Some(0))]
#[case(Some(16_777_217))]
#[case(Some(-16_777_217))]
#[case(Some(i32::MIN))]
#[case(Some(i32::MAX))]
fn internal_integer_defaults_are_preserved(#[case] default: Option<i32>) {
    let avatar = transform(&declaration(
        PrimitiveParameterValue::Int {
            default: default.map(i64::from),
            repr: None,
        },
        ParameterScope::Internal,
    ))
    .unwrap();

    assert_eq!(
        avatar.controllers[0].controller.parameters,
        vec![AnimatorParameter::create_int("Test", default)]
    );
    assert!(avatar.expression_parameters.is_empty());
}

#[rstest]
#[case(None)]
#[case(Some(-128))]
#[case(Some(0))]
#[case(Some(127))]
fn expression_integer_defaults_are_preserved(#[case] default: Option<i32>, #[values(ParameterScope::Synced, ParameterScope::Local)] scope: ParameterScope) {
    let avatar = transform(&declaration(
        PrimitiveParameterValue::Int {
            default: default.map(i64::from),
            repr: Some(ParameterRepr {
                signedness: Signedness::Signed,
                width: Some(8),
            }),
        },
        scope,
    ))
    .unwrap();

    assert_eq!(
        avatar.controllers[0].controller.parameters[0].type_default,
        AnimatorParameterTypeDefault::Int(default)
    );
    assert_eq!(
        avatar.expression_parameters[0].type_default,
        ExpressionParameterTypeDefault::Int {
            width: ExpressionParameterWidth::Specified(8),
            signedness: ExpressionParameterSignedness::Signed,
            default,
        }
    );
}

#[rstest]
#[case::unsigned_int(PrimitiveParameterValue::Int { default: Some(-1), repr: None }, "-1", "0..=255")]
#[case::narrow_int(
    PrimitiveParameterValue::Int { default: Some(4), repr: Some(ParameterRepr { signedness: Signedness::Signed, width: Some(3) }) },
    "4",
    "-4..=3"
)]
#[case::signed_float(PrimitiveParameterValue::Float { default: Some(-1.5), repr: None }, "-1.5", "-1.0..=1.0")]
#[case::unsigned_float(
    PrimitiveParameterValue::Float { default: Some(-0.25), repr: Some(ParameterRepr { signedness: Signedness::Unsigned, width: None }) },
    "-0.25",
    "0.0..=1.0"
)]
fn expression_defaults_outside_their_range_are_located_errors(
    #[case] value: PrimitiveParameterValue,
    #[case] written: &str,
    #[case] range: &str,
    #[values(ParameterScope::Synced, ParameterScope::Local)] scope: ParameterScope,
) {
    let errors = transform(&declaration(value, scope)).unwrap_err();
    assert_eq!(errors.0.len(), 1);
    assert_eq!(
        errors.0[0].to_string(),
        format!("parameters.lua:7: parameter `Test` holds {range}, but {written} is written")
    );
}

#[rstest]
#[case(i64::from(i32::MIN) - 1)]
#[case(i64::from(i32::MAX) + 1)]
#[case(i64::MIN)]
#[case(i64::MAX)]
fn out_of_range_integer_defaults_are_located_errors(#[case] value: i64, #[values(ParameterScope::Synced, ParameterScope::Internal)] scope: ParameterScope) {
    let errors = transform(&declaration(
        PrimitiveParameterValue::Int {
            default: Some(value),
            repr: None,
        },
        scope,
    ))
    .unwrap_err();
    assert_eq!(
        errors.0,
        vec![
            TransformErrorKind::ParameterDefaultOutOfRange { name: "Test".into(), value }.at(Some(SourceLocation {
                chunk: "parameters.lua".into(),
                line: 7
            }))
        ]
    );
}

#[rstest]
#[case(PrimitiveParameterValue::Bool { default: None }, AnimatorParameterTypeDefault::Bool(None))]
#[case(PrimitiveParameterValue::Bool { default: Some(false) }, AnimatorParameterTypeDefault::Bool(Some(false)))]
#[case(PrimitiveParameterValue::Bool { default: Some(true) }, AnimatorParameterTypeDefault::Bool(Some(true)))]
#[case(PrimitiveParameterValue::Float { default: None, repr: None }, AnimatorParameterTypeDefault::Float(None))]
#[case(PrimitiveParameterValue::Float { default: Some(0.25), repr: None }, AnimatorParameterTypeDefault::Float(Some(0.25)))]
fn defaults_keep_their_parameter_type(#[case] value: PrimitiveParameterValue, #[case] expected: AnimatorParameterTypeDefault) {
    let avatar = transform(&declaration(value, ParameterScope::Synced)).unwrap();
    assert_eq!(avatar.controllers[0].controller.parameters[0].type_default, expected);
}

#[rstest]
fn provided_parameters_have_typed_absent_defaults_and_name_their_group() {
    let avatar = transform(&Avatar {
        parameters: vec![Parameter::Provided(ProvidedParameterGroup::Vrchat)],
        controllers: vec![Controller::new(PlayableLayer::Fx, vec![])],
        ..Avatar::default()
    })
    .unwrap();
    let parameters = &avatar.controllers[0].controller.parameters;
    let provided = AnimatorParameterOrigin::Provided(ProvidedParameterGroup::Vrchat);
    for expected in [
        AnimatorParameter::create_bool("AFK", None),
        AnimatorParameter::create_int("GestureLeft", None),
        AnimatorParameter::create_float("GestureLeftWeight", None),
    ] {
        assert!(parameters.contains(&expected.with_origin(provided)));
    }
    assert!(parameters.iter().all(|parameter| parameter.origin == provided));
}
