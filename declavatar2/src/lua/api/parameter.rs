use mlua::{Error as LuaError, Lua, Result as LuaResult, Table, Value};

use crate::{
    decl::parameter::{Parameter, ParameterRepr, ParameterScope, PrimitiveParameter, PrimitiveParameterValue, ProvidedParameterGroup, Signedness},
    lua::{
        location::caller_location,
        node,
        options::{Options, one_of},
        value::StrictBoolean,
    },
    vrchat::expr_parameter::ExpressionParameterWidth,
};

pub(crate) const SCOPES: &[(&str, ParameterScope)] = &[
    ("synced", ParameterScope::Synced),
    ("local", ParameterScope::Local),
    ("internal", ParameterScope::Internal),
];

pub(crate) const PROVIDED_GROUPS: &[(&str, ProvidedParameterGroup)] = &[("VRChat", ProvidedParameterGroup::Vrchat)];

pub(crate) fn register(lua: &Lua, da: &Table) -> LuaResult<()> {
    da.set("bool", lua.create_function(bool_parameter)?)?;
    da.set("int", lua.create_function(int_parameter)?)?;
    da.set("float", lua.create_function(float_parameter)?)?;
    da.set("provided", lua.create_function(provided)?)?;
    Ok(())
}

fn bool_parameter(lua: &Lua, (name, table): (String, Option<Table>)) -> LuaResult<node::Parameter> {
    const OWNER: &str = "da.bool";

    let mut options = Options::new(OWNER, table);
    let default = options.take::<Value>("default")?.map(|value| boolean(OWNER, "default", &value)).transpose()?;
    let common = common_options(OWNER, &mut options)?;
    options.finish()?;

    Ok(node::Parameter(Parameter::Primitive(primitive(
        lua,
        name,
        PrimitiveParameterValue::Bool { default },
        common,
    ))))
}

fn int_parameter(lua: &Lua, (name, table): (String, Option<Table>)) -> LuaResult<node::Parameter> {
    const OWNER: &str = "da.int";

    let mut options = Options::new(OWNER, table);
    let default = options.take::<Value>("default")?.map(|value| integer(OWNER, "default", &value)).transpose()?;
    let repr = repr(OWNER, &mut options)?;
    let common = common_options(OWNER, &mut options)?;
    options.finish()?;

    let parameter = primitive(lua, name, PrimitiveParameterValue::Int { default, repr }, common);
    check_numeric(OWNER, &parameter)?;
    Ok(node::Parameter(Parameter::Primitive(parameter)))
}

fn float_parameter(lua: &Lua, (name, table): (String, Option<Table>)) -> LuaResult<node::Parameter> {
    const OWNER: &str = "da.float";

    let mut options = Options::new(OWNER, table);
    let default = options.take::<Value>("default")?.map(|value| number(OWNER, "default", &value)).transpose()?;
    let repr = repr(OWNER, &mut options)?;
    let common = common_options(OWNER, &mut options)?;
    options.finish()?;

    let parameter = primitive(lua, name, PrimitiveParameterValue::Float { default, repr }, common);
    check_numeric(OWNER, &parameter)?;
    Ok(node::Parameter(Parameter::Primitive(parameter)))
}

fn provided(_: &Lua, group: String) -> LuaResult<node::Parameter> {
    let group = one_of("da.provided", "parameter group", &group, PROVIDED_GROUPS)?;
    Ok(node::Parameter(Parameter::Provided(group)))
}

struct CommonOptions {
    scope: Option<ParameterScope>,
    save: Option<bool>,
}

fn common_options(owner: &'static str, options: &mut Options) -> LuaResult<CommonOptions> {
    let scope = options
        .take::<String>("scope")?
        .map(|written| one_of(owner, "scope", &written, SCOPES))
        .transpose()?;
    let save = options.take::<StrictBoolean>("save")?.map(|value| value.0);
    Ok(CommonOptions { scope, save })
}

fn primitive(lua: &Lua, name: String, value: PrimitiveParameterValue, common: CommonOptions) -> PrimitiveParameter {
    PrimitiveParameter {
        name,
        value,
        scope: common.scope,
        save: common.save,
        at: caller_location(lua),
    }
}

fn repr(owner: &'static str, options: &mut Options) -> LuaResult<Option<ParameterRepr>> {
    let Some(written) = options.take::<String>("repr")? else {
        return Ok(None);
    };
    let mut chars = written.chars();
    let signedness = match chars.next() {
        Some('u') => Some(Signedness::Unsigned),
        Some('s') => Some(Signedness::Signed),
        _ => None,
    };
    let width = match chars.as_str() {
        "" => Some(None),
        bits => match bits.as_bytes() {
            [digit @ b'0'..=b'9'] => Some(digit - b'0')
                .filter(|width| (ExpressionParameterWidth::MIN_BIT_WIDTH..=ExpressionParameterWidth::MAX_BIT_WIDTH).contains(width))
                .map(Some),
            _ => None,
        },
    };
    match (signedness, width) {
        (Some(signedness), Some(width)) => Ok(Some(ParameterRepr { signedness, width })),
        _ => Err(LuaError::runtime(format!(
            "{owner}: option `repr` must be `u` or `s` followed by an optional bit width from 2 to 8, such as `u4` or `s`, but `{written}` was written"
        ))),
    }
}

fn check_numeric(owner: &'static str, parameter: &PrimitiveParameter) -> LuaResult<()> {
    let written_repr = matches!(
        parameter.value,
        PrimitiveParameterValue::Int { repr: Some(_), .. } | PrimitiveParameterValue::Float { repr: Some(_), .. }
    );
    if written_repr && parameter.scope == Some(ParameterScope::Internal) {
        return Err(LuaError::runtime(format!(
            "{owner}: option `repr` cannot be written on an internal parameter, which is not an expression parameter"
        )));
    }
    match parameter.default_out_of_range() {
        Some((written, range)) => Err(LuaError::runtime(format!(
            "{owner}: option `default` is {written}, but the parameter holds {range}"
        ))),
        None => Ok(()),
    }
}

fn boolean(owner: &'static str, label: &str, value: &Value) -> LuaResult<bool> {
    match value {
        Value::Boolean(written) => Ok(*written),
        other => Err(wrong_kind(owner, label, "a boolean", other)),
    }
}

fn integer(owner: &'static str, label: &str, value: &Value) -> LuaResult<i64> {
    match value {
        Value::Integer(written) => Ok(*written),
        Value::Number(written) => Err(LuaError::runtime(format!(
            "{owner}: option `{label}` must be an integer, but {written} was written"
        ))),
        other => Err(wrong_kind(owner, label, "an integer", other)),
    }
}

fn number(owner: &'static str, label: &str, value: &Value) -> LuaResult<f64> {
    match value {
        Value::Integer(written) => Ok(*written as f64),
        Value::Number(written) => Ok(*written),
        other => Err(wrong_kind(owner, label, "a number", other)),
    }
}

fn wrong_kind(owner: &'static str, label: &str, expected: &str, value: &Value) -> LuaError {
    LuaError::runtime(format!(
        "{owner}: option `{label}` must be {expected}, but {} was written",
        node::describe(value)
    ))
}

#[cfg(test)]
mod tests {
    use rstest::*;

    use super::*;
    use crate::{
        core::resolution::SourceLocation,
        lua::testing::{avatar, avatar_error},
    };

    fn script(expression: &str) -> String {
        format!("local da = require 'declavatar'\nreturn da.avatar({{ parameters = {{ {expression} }} }})\n")
    }

    fn declared(expression: &str) -> Parameter {
        let mut parameters = avatar(&script(expression)).parameters;
        assert_eq!(parameters.len(), 1, "exactly one parameter should be declared");
        parameters.remove(0)
    }

    fn rejected(expression: &str) -> String {
        avatar_error(&script(expression))
    }

    fn primitive_of(expression: &str) -> PrimitiveParameter {
        match declared(expression) {
            Parameter::Primitive(primitive) => primitive,
            other => panic!("expected a primitive parameter, got {other:?}"),
        }
    }

    #[rstest]
    #[case::bare("da.bool('Hat')", PrimitiveParameterValue::Bool { default: None })]
    #[case::default_true("da.bool('Hat', { default = true })", PrimitiveParameterValue::Bool { default: Some(true) })]
    #[case::default_false("da.bool('Hat', { default = false })", PrimitiveParameterValue::Bool { default: Some(false) })]
    fn a_bool_parameter_takes_only_a_boolean_default(#[case] expression: &str, #[case] expected: PrimitiveParameterValue) {
        let parameter = primitive_of(expression);
        assert_eq!(parameter.name, "Hat");
        assert_eq!(parameter.value, expected);
    }

    fn repr(signedness: Signedness, width: Option<u8>) -> Option<ParameterRepr> {
        Some(ParameterRepr { signedness, width })
    }

    #[rstest]
    #[case::bare("da.int('Emote')", PrimitiveParameterValue::Int { default: None, repr: None })]
    #[case::default("da.int('Emote', { default = 42 })", PrimitiveParameterValue::Int { default: Some(42), repr: None })]
    #[case::repr("da.int('Emote', { repr = 'u4' })", PrimitiveParameterValue::Int { default: None, repr: repr(Signedness::Unsigned, Some(4)) })]
    #[case::signed_default("da.int('Emote', { repr = 's', default = -1 })", PrimitiveParameterValue::Int { default: Some(-1), repr: repr(Signedness::Signed, None) })]
    fn an_int_parameter_carries_its_default_and_repr(#[case] expression: &str, #[case] expected: PrimitiveParameterValue) {
        assert_eq!(primitive_of(expression).value, expected);
    }

    #[rstest]
    #[case::bare("da.float('Blend')", PrimitiveParameterValue::Float { default: None, repr: None })]
    #[case::fractional("da.float('Blend', { default = 0.25 })", PrimitiveParameterValue::Float { default: Some(0.25), repr: None })]
    #[case::integer_written("da.float('Blend', { default = 1 })", PrimitiveParameterValue::Float { default: Some(1.0), repr: None })]
    #[case::repr("da.float('Blend', { repr = 'u2' })", PrimitiveParameterValue::Float { default: None, repr: repr(Signedness::Unsigned, Some(2)) })]
    fn a_float_parameter_accepts_an_integer_default(#[case] expression: &str, #[case] expected: PrimitiveParameterValue) {
        assert_eq!(primitive_of(expression).value, expected);
    }

    #[rstest]
    #[case::unsigned("u", Signedness::Unsigned, None)]
    #[case::signed("s", Signedness::Signed, None)]
    #[case::narrowest("u2", Signedness::Unsigned, Some(2))]
    #[case::widest("s8", Signedness::Signed, Some(8))]
    fn a_repr_is_a_signedness_and_an_optional_width(#[case] written: &str, #[case] signedness: Signedness, #[case] width: Option<u8>) {
        let parameter = primitive_of(&format!("da.int('Emote', {{ repr = '{written}' }})"));
        assert_eq!(
            parameter.value,
            PrimitiveParameterValue::Int {
                default: None,
                repr: repr(signedness, width)
            }
        );
    }

    #[rstest]
    #[case::empty("")]
    #[case::width_alone("4")]
    #[case::rust_style("i4")]
    #[case::uppercase("U4")]
    #[case::too_narrow("u1")]
    #[case::too_wide("s9")]
    #[case::trailing("u4 ")]
    #[case::plus_sign("u+4")]
    #[case::leading_zero("u04")]
    fn a_malformed_repr_is_rejected(#[case] written: &str) {
        let message = rejected(&format!("da.int('Emote', {{ repr = '{written}' }})"));
        assert!(
            message.contains(&format!(
                "da.int: option `repr` must be `u` or `s` followed by an optional bit width from 2 to 8, such as `u4` or `s`, but `{written}` was written"
            )),
            "{message}"
        );
    }

    #[rstest]
    #[case::unsigned_default("da.int('Emote', { default = 255 })")]
    #[case::unsigned_narrow("da.int('Emote', { repr = 'u4', default = 15 })")]
    #[case::signed_low("da.int('Emote', { repr = 's4', default = -8 })")]
    #[case::signed_high("da.int('Emote', { repr = 's4', default = 7 })")]
    #[case::signed_float("da.float('Blend', { default = -1 })")]
    #[case::unsigned_float("da.float('Blend', { repr = 'u', default = 1 })")]
    #[case::internal_int("da.int('Emote', { scope = 'internal', default = -1000 })")]
    #[case::internal_float("da.float('Blend', { scope = 'internal', default = 5 })")]
    fn a_default_inside_the_range_is_accepted(#[case] expression: &str) {
        primitive_of(expression);
    }

    #[rstest]
    #[case::negative_int("da.int('Emote', { default = -1 })", "da.int: option `default` is -1, but the parameter holds 0..=255")]
    #[case::unsigned_narrow(
        "da.int('Emote', { repr = 'u4', default = 16 })",
        "da.int: option `default` is 16, but the parameter holds 0..=15"
    )]
    #[case::signed_low(
        "da.int('Emote', { repr = 's4', default = -9 })",
        "da.int: option `default` is -9, but the parameter holds -8..=7"
    )]
    #[case::signed_high(
        "da.int('Emote', { repr = 's', default = 128 })",
        "da.int: option `default` is 128, but the parameter holds -128..=127"
    )]
    #[case::signed_float("da.float('Blend', { default = 1.5 })", "da.float: option `default` is 1.5, but the parameter holds -1.0..=1.0")]
    #[case::unsigned_float(
        "da.float('Blend', { repr = 'u8', default = -0.5 })",
        "da.float: option `default` is -0.5, but the parameter holds 0.0..=1.0"
    )]
    fn a_default_outside_the_range_is_rejected(#[case] expression: &str, #[case] expected: &str) {
        let message = rejected(expression);
        assert!(message.contains(expected), "{message}");
    }

    #[rstest]
    fn an_internal_parameter_has_no_repr() {
        let message = rejected("da.int('Emote', { scope = 'internal', repr = 'u4' })");
        assert!(
            message.contains("da.int: option `repr` cannot be written on an internal parameter, which is not an expression parameter"),
            "{message}"
        );
    }

    #[rstest]
    fn an_int_default_must_be_written_as_an_integer() {
        let message = rejected("da.int('Emote', { default = 1.5 })");
        assert!(
            message.contains("da.int: option `default` must be an integer, but 1.5 was written"),
            "{message}"
        );
    }

    #[rstest]
    #[case::synced("synced", ParameterScope::Synced)]
    #[case::local("local", ParameterScope::Local)]
    #[case::internal("internal", ParameterScope::Internal)]
    fn a_scope_is_written_by_name(#[case] written: &str, #[case] expected: ParameterScope) {
        let parameter = primitive_of(&format!("da.bool('Hat', {{ scope = '{written}' }})"));
        assert_eq!(parameter.scope, Some(expected));
    }

    #[rstest]
    fn an_unknown_scope_lists_the_accepted_ones() {
        let message = rejected("da.bool('Hat', { scope = 'private' })");
        assert!(message.contains("da.bool: scope `private` is not known"), "{message}");
        assert!(message.contains("`synced`, `local`, `internal`"), "{message}");
    }

    #[rstest]
    fn save_is_taken_as_written() {
        assert_eq!(primitive_of("da.bool('Hat', { save = true })").save, Some(true));
        assert_eq!(primitive_of("da.bool('Hat', { save = false })").save, Some(false));
        assert_eq!(primitive_of("da.bool('Hat')").save, None);
    }

    #[rstest]
    #[case::repr("repr")]
    #[case::width("width")]
    fn a_bool_parameter_has_no_repr(#[case] option: &str) {
        let message = rejected(&format!("da.bool('Hat', {{ {option} = 'u4' }})"));
        assert!(message.contains(&format!("da.bool: unknown option `{option}`")), "{message}");
    }

    #[rstest]
    fn width_is_not_an_option() {
        let message = rejected("da.int('Emote', { width = 4 })");
        assert!(message.contains("da.int: unknown option `width`"), "{message}");
    }

    #[rstest]
    fn provided_groups_match_exactly() {
        assert_eq!(declared("da.provided('VRChat')"), Parameter::Provided(ProvidedParameterGroup::Vrchat));

        let message = rejected("da.provided('vrchat')");
        assert!(message.contains("da.provided: parameter group `vrchat` is not known"), "{message}");
        assert!(message.contains("`VRChat`"), "{message}");
    }

    #[rstest]
    fn a_parameter_records_where_it_was_written() {
        let declared = avatar("local da = require 'declavatar'\nreturn da.avatar({\n  parameters = {\n    da.int('Emote'),\n  },\n})\n");
        let Parameter::Primitive(parameter) = &declared.parameters[0] else {
            panic!("expected a primitive parameter");
        };

        assert_eq!(
            parameter.at,
            Some(SourceLocation {
                chunk: "avatar.lua".into(),
                line: 4,
            })
        );
    }

    #[rstest]
    fn the_parameters_block_keeps_order_and_drops_false() {
        let declared = avatar(&script("da.provided('VRChat'), false, da.int('Emote'), da.bool('Hat')"));

        let names: Vec<_> = declared
            .parameters
            .iter()
            .map(|parameter| match parameter {
                Parameter::Primitive(primitive) => primitive.name.clone(),
                Parameter::Provided(group) => format!("{group:?}"),
            })
            .collect();
        assert_eq!(names, ["Vrchat", "Emote", "Hat"]);
    }

    #[rstest]
    fn a_value_is_not_a_parameter() {
        let message = rejected("da.vec3(1, 2, 3)");
        assert!(message.contains("da.avatar: parameters: entry 1: expected parameter, got vector"), "{message}");
    }
}
