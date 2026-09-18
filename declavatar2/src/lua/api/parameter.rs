use mlua::{Error as LuaError, Lua, Result as LuaResult, Table, Value};

use crate::{
    decl::parameter::{Parameter, ParameterScope, PrimitiveParameter, PrimitiveParameterValue, ProvidedParameterGroup},
    lua::{
        location::caller_location,
        node,
        options::{Options, one_of},
    },
};

const SCOPES: &[(&str, ParameterScope)] = &[
    ("synced", ParameterScope::Synced),
    ("local", ParameterScope::Local),
    ("internal", ParameterScope::Internal),
];

const PROVIDED_GROUPS: &[(&str, ProvidedParameterGroup)] = &[("VRChat", ProvidedParameterGroup::Vrchat)];

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

    Ok(primitive(lua, name, PrimitiveParameterValue::Bool { default }, common))
}

fn int_parameter(lua: &Lua, (name, table): (String, Option<Table>)) -> LuaResult<node::Parameter> {
    const OWNER: &str = "da.int";

    let mut options = Options::new(OWNER, table);
    let default = options.take::<Value>("default")?.map(|value| integer(OWNER, "default", &value)).transpose()?;
    let width = width(OWNER, &mut options)?;
    let common = common_options(OWNER, &mut options)?;
    options.finish()?;

    Ok(primitive(lua, name, PrimitiveParameterValue::Int { default, width }, common))
}

fn float_parameter(lua: &Lua, (name, table): (String, Option<Table>)) -> LuaResult<node::Parameter> {
    const OWNER: &str = "da.float";

    let mut options = Options::new(OWNER, table);
    let default = options.take::<Value>("default")?.map(|value| number(OWNER, "default", &value)).transpose()?;
    let width = width(OWNER, &mut options)?;
    let common = common_options(OWNER, &mut options)?;
    options.finish()?;

    Ok(primitive(lua, name, PrimitiveParameterValue::Float { default, width }, common))
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
    let save = options.take::<bool>("save")?;
    Ok(CommonOptions { scope, save })
}

fn primitive(lua: &Lua, name: String, value: PrimitiveParameterValue, common: CommonOptions) -> node::Parameter {
    node::Parameter(Parameter::Primitive(PrimitiveParameter {
        name,
        value,
        scope: common.scope,
        save: common.save,
        at: caller_location(lua),
    }))
}

fn width(owner: &'static str, options: &mut Options) -> LuaResult<Option<u8>> {
    let Some(value) = options.take::<Value>("width")? else {
        return Ok(None);
    };
    let written = integer(owner, "width", &value)?;
    u8::try_from(written)
        .ok()
        .filter(|width| *width > 0)
        .map(Some)
        .ok_or_else(|| LuaError::runtime(format!("{owner}: option `width` must be a positive bit count, but {written} was written")))
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

    #[rstest]
    #[case::bare("da.int('Emote')", PrimitiveParameterValue::Int { default: None, width: None })]
    #[case::default("da.int('Emote', { default = 42 })", PrimitiveParameterValue::Int { default: Some(42), width: None })]
    #[case::width("da.int('Emote', { width = 4 })", PrimitiveParameterValue::Int { default: None, width: Some(4) })]
    #[case::negative("da.int('Emote', { default = -1 })", PrimitiveParameterValue::Int { default: Some(-1), width: None })]
    fn an_int_parameter_carries_its_default_and_width(#[case] expression: &str, #[case] expected: PrimitiveParameterValue) {
        assert_eq!(primitive_of(expression).value, expected);
    }

    #[rstest]
    #[case::bare("da.float('Blend')", PrimitiveParameterValue::Float { default: None, width: None })]
    #[case::fractional("da.float('Blend', { default = 0.25 })", PrimitiveParameterValue::Float { default: Some(0.25), width: None })]
    #[case::integer_written("da.float('Blend', { default = 1 })", PrimitiveParameterValue::Float { default: Some(1.0), width: None })]
    fn a_float_parameter_accepts_an_integer_default(#[case] expression: &str, #[case] expected: PrimitiveParameterValue) {
        assert_eq!(primitive_of(expression).value, expected);
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
    fn a_bool_parameter_has_no_width() {
        let message = rejected("da.bool('Hat', { width = 4 })");
        assert!(message.contains("da.bool: unknown option `width`"), "{message}");
    }

    #[rstest]
    #[case::zero("0")]
    #[case::negative("-1")]
    #[case::too_wide("300")]
    fn a_width_must_be_a_positive_bit_count(#[case] written: &str) {
        let message = rejected(&format!("da.int('Emote', {{ width = {written} }})"));
        assert!(message.contains("must be a positive bit count"), "{message}");
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
