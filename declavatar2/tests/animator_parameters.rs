use declavatar2::{
    core::resolution::SourceLocation,
    decl::{
        Avatar,
        controller::Controller,
        parameter::{Parameter, ParameterScope, PrimitiveParameter, PrimitiveParameterValue, ProvidedParameterGroup},
    },
    transform::{TransformErrorKind, transform},
    unity::animator::{AnimatorParameter, AnimatorParameterTypeDefault},
    vrchat::{
        expr_parameter::{ExpressionParameterTypeDefault, ExpressionParameterWidth},
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
fn integer_defaults_are_preserved(
    #[case] default: Option<i32>,
    #[values(ParameterScope::Synced, ParameterScope::Local, ParameterScope::Internal)] scope: ParameterScope,
) {
    let avatar = transform(&declaration(
        PrimitiveParameterValue::Int {
            default: default.map(i64::from),
            width: Some(8),
        },
        scope,
    ))
    .unwrap();

    assert_eq!(
        avatar.controllers[0].controller.parameters,
        vec![AnimatorParameter::create_int("Test", default)]
    );
    assert_eq!(
        avatar.controllers[0].controller.parameters[0].type_default,
        AnimatorParameterTypeDefault::Int(default)
    );
    if scope == ParameterScope::Internal {
        assert!(avatar.expression_parameters.is_empty());
    } else {
        assert_eq!(
            avatar.expression_parameters[0].type_default,
            ExpressionParameterTypeDefault::Int {
                width: ExpressionParameterWidth::Specified(8),
                default,
            }
        );
    }
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
            width: None,
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
#[case(PrimitiveParameterValue::Float { default: None, width: None }, AnimatorParameterTypeDefault::Float(None))]
#[case(PrimitiveParameterValue::Float { default: Some(0.25), width: None }, AnimatorParameterTypeDefault::Float(Some(0.25)))]
fn defaults_keep_their_parameter_type(#[case] value: PrimitiveParameterValue, #[case] expected: AnimatorParameterTypeDefault) {
    let avatar = transform(&declaration(value, ParameterScope::Synced)).unwrap();
    assert_eq!(avatar.controllers[0].controller.parameters[0].type_default, expected);
}

#[rstest]
fn provided_parameters_have_typed_absent_defaults() {
    let avatar = transform(&Avatar {
        parameters: vec![Parameter::Provided(ProvidedParameterGroup::Vrchat)],
        controllers: vec![Controller::new(PlayableLayer::Fx, vec![])],
        ..Avatar::default()
    })
    .unwrap();
    let parameters = &avatar.controllers[0].controller.parameters;
    for expected in [
        AnimatorParameter::create_bool("AFK", None),
        AnimatorParameter::create_int("GestureLeft", None),
        AnimatorParameter::create_float("GestureLeftWeight", None),
    ] {
        assert!(parameters.contains(&expected));
    }
}
