use std::collections::{BTreeMap, HashMap};

use mlua::{Error as LuaError, Lua, Result as LuaResult, Table, Value};

use crate::{
    core::{phase::Declared, resolution::Unresolved},
    decl::behavior::{Behavior, Drive},
    lua::{
        list,
        location::caller_location,
        node,
        options::{Options, one_of},
        value::{StrictBoolean, animated_value},
    },
    unity::{
        state::{GenericStateBehavior, GenericValue},
        value::AnimatedValue,
    },
    vrchat::state_behaviour::{ParameterDriveTarget, TrackingControl, TrackingControlMode, TrackingControlTarget},
};

pub(crate) const TRACKING_MODES: &[(&str, TrackingControlMode)] = &[("tracking", TrackingControlMode::Tracking), ("animation", TrackingControlMode::Animation)];

pub(crate) const TRACKING_TARGETS: &[(&str, TrackingControlTarget)] = &[
    ("head", TrackingControlTarget::Head),
    ("left_hand", TrackingControlTarget::LeftHand),
    ("right_hand", TrackingControlTarget::RightHand),
    ("hip", TrackingControlTarget::Hip),
    ("left_foot", TrackingControlTarget::LeftFoot),
    ("right_foot", TrackingControlTarget::RightFoot),
    ("left_fingers", TrackingControlTarget::LeftFingers),
    ("right_fingers", TrackingControlTarget::RightFingers),
    ("eyes", TrackingControlTarget::Eyes),
    ("mouth", TrackingControlTarget::Mouth),
];

pub(crate) fn register(lua: &Lua, da: &Table) -> LuaResult<()> {
    da.set("drive_group", lua.create_function(drive_group)?)?;
    da.set("drive_switch", lua.create_function(drive_switch)?)?;
    da.set("drive_puppet", lua.create_function(drive_puppet)?)?;
    da.set("drive_bool", lua.create_function(drive_bool)?)?;
    da.set("drive_int", lua.create_function(drive_int)?)?;
    da.set("drive_float", lua.create_function(drive_float)?)?;
    da.set("drive_add", lua.create_function(drive_add)?)?;
    da.set("drive_random_int", lua.create_function(drive_random_int)?)?;
    da.set("drive_random_bool", lua.create_function(drive_random_bool)?)?;
    da.set("drive_random_float", lua.create_function(drive_random_float)?)?;
    da.set("drive_copy", lua.create_function(drive_copy)?)?;
    da.set("tracking", lua.create_function(tracking)?)?;
    da.set("behavior", lua.create_function(behavior)?)?;
    Ok(())
}

fn drive_group(lua: &Lua, (layer, option): (String, String)) -> LuaResult<node::Drive> {
    Ok(node::Drive(Drive::Group {
        layer: located(lua, layer),
        option,
    }))
}

fn drive_switch(lua: &Lua, (layer, value): (String, Option<StrictBoolean>)) -> LuaResult<node::Drive> {
    Ok(node::Drive(Drive::Switch {
        layer: located(lua, layer),
        value: value.map(|value| value.0),
    }))
}

fn drive_puppet(lua: &Lua, (layer, value): (String, Option<f64>)) -> LuaResult<node::Drive> {
    Ok(node::Drive(Drive::Puppet {
        layer: located(lua, layer),
        value,
    }))
}

fn drive_bool(lua: &Lua, (parameter, value): (String, Value)) -> LuaResult<node::Drive> {
    let Value::Boolean(value) = value else {
        return Err(wrong_value("da.drive_bool", "a boolean", &value));
    };
    Ok(parameter_drive(lua, parameter, AnimatedValue::Bool(value)))
}

fn drive_int(lua: &Lua, (parameter, value): (String, Value)) -> LuaResult<node::Drive> {
    let Value::Integer(value) = value else {
        return Err(wrong_value("da.drive_int", "an integer", &value));
    };
    Ok(parameter_drive(lua, parameter, AnimatedValue::Int(value)))
}

fn drive_float(lua: &Lua, (parameter, value): (String, Value)) -> LuaResult<node::Drive> {
    let value = match value {
        Value::Integer(value) => value as f64,
        Value::Number(value) => value,
        other => return Err(wrong_value("da.drive_float", "a number", &other)),
    };
    Ok(parameter_drive(lua, parameter, AnimatedValue::Float(value)))
}

fn drive_add(lua: &Lua, (parameter, value): (String, Value)) -> LuaResult<node::Behavior> {
    let value = match value {
        Value::Integer(value) => AnimatedValue::Int(value),
        Value::Number(value) => AnimatedValue::Float(value),
        other => return Err(wrong_value("da.drive_add", "a number", &other)),
    };
    Ok(state_drive(ParameterDriveTarget::Add {
        parameter: located(lua, parameter),
        value,
    }))
}

fn drive_random_int(lua: &Lua, (parameter, min, max): (String, Value, Value)) -> LuaResult<node::Behavior> {
    const OWNER: &str = "da.drive_random_int";

    let integer = |value: Value| match value {
        Value::Integer(value) => Ok(value),
        other => Err(wrong_value(OWNER, "an integer", &other)),
    };
    let range = ordered(OWNER, [integer(min)?, integer(max)?])?;
    Ok(state_drive(ParameterDriveTarget::RandomInt {
        parameter: located(lua, parameter),
        range,
    }))
}

fn drive_random_bool(lua: &Lua, (parameter, chance): (String, Option<f64>)) -> LuaResult<node::Behavior> {
    let chance = chance.unwrap_or(0.5);
    if !(0.0..=1.0).contains(&chance) {
        return Err(LuaError::runtime(format!(
            "da.drive_random_bool: a chance is between 0 and 1, but {chance} was written"
        )));
    }
    Ok(state_drive(ParameterDriveTarget::RandomBool {
        parameter: located(lua, parameter),
        chance,
    }))
}

fn drive_random_float(lua: &Lua, (parameter, min, max): (String, f64, f64)) -> LuaResult<node::Behavior> {
    let range = ordered("da.drive_random_float", [min, max])?;
    Ok(state_drive(ParameterDriveTarget::RandomFloat {
        parameter: located(lua, parameter),
        range,
    }))
}

fn drive_copy(lua: &Lua, (from, to, table): (String, String, Option<Table>)) -> LuaResult<node::Behavior> {
    const OWNER: &str = "da.drive_copy";

    let mut options = Options::new(OWNER, table);
    let from_range = options
        .take::<Value>("from_range")?
        .map(|value| range(OWNER, "from_range", &value))
        .transpose()?;
    let to_range = options.take::<Value>("to_range")?.map(|value| range(OWNER, "to_range", &value)).transpose()?;
    options.finish()?;

    let from = located(lua, from);
    let to = located(lua, to);
    let target = match (from_range, to_range) {
        (None, None) => ParameterDriveTarget::Copy { from, to },
        (Some(from_range), Some(to_range)) => ParameterDriveTarget::RangedCopy {
            from,
            from_range,
            to,
            to_range,
        },
        _ => {
            return Err(LuaError::runtime(format!(
                "{OWNER}: a ranged copy maps one range onto another, so `from_range` and `to_range` are written together"
            )));
        }
    };
    Ok(state_drive(target))
}

fn tracking(lua: &Lua, (mode, targets): (String, Table)) -> LuaResult<node::Behavior> {
    const OWNER: &str = "da.tracking";

    let mode = one_of(OWNER, "mode", &mode, TRACKING_MODES)?;
    let written = list::collect::<String>(lua, OWNER, &targets)?;
    if written.is_empty() {
        return Err(LuaError::runtime(format!("{OWNER}: at least one target is needed")));
    }

    let mut values = HashMap::new();
    for target in written {
        values.insert(one_of(OWNER, "target", &target, TRACKING_TARGETS)?, mode);
    }

    Ok(node::Behavior(Behavior::TrackingControl(TrackingControl { values })))
}

fn behavior(lua: &Lua, (type_name, fields): (String, Option<Table>)) -> LuaResult<node::Behavior> {
    const OWNER: &str = "da.behavior";

    let fields = match fields {
        Some(table) => match generic_table(OWNER, "", &table)? {
            GenericValue::Map(fields) => fields,
            GenericValue::List(values) if values.is_empty() => BTreeMap::new(),
            _ => {
                return Err(LuaError::runtime(format!(
                    "{OWNER}: fields are named, so they are written as a table with string keys"
                )));
            }
        },
        None => BTreeMap::new(),
    };

    Ok(node::Behavior(Behavior::Generic(GenericStateBehavior {
        type_name: located(lua, type_name),
        fields,
    })))
}

/// Reads one value of a generic behavior, where `path` names it in error messages.
fn generic_value(owner: &'static str, path: &str, value: &Value) -> LuaResult<GenericValue> {
    match value {
        Value::Boolean(value) => Ok(GenericValue::Bool(*value)),
        Value::Integer(value) => Ok(GenericValue::Int(*value)),
        Value::Number(value) => Ok(GenericValue::Float(*value)),
        Value::String(value) => Ok(GenericValue::String(value.to_str()?.to_owned())),
        Value::Table(table) => generic_table(owner, path, table),
        other => Err(LuaError::runtime(format!(
            "{owner}: field `{path}` is {}, which a behavior cannot hold",
            node::describe(other)
        ))),
    }
}

/// A table with string keys is a map and a sequence is a list. An empty table is an empty list.
fn generic_table(owner: &'static str, path: &str, table: &Table) -> LuaResult<GenericValue> {
    let mut named = BTreeMap::new();
    let mut numbered = 0;
    for pair in table.pairs::<Value, Value>() {
        let (key, value) = pair?;
        match key {
            Value::String(name) => {
                let name = name.to_str()?.to_owned();
                let inner = if path.is_empty() { name.clone() } else { format!("{path}.{name}") };
                named.insert(name, generic_value(owner, &inner, &value)?);
            }
            Value::Integer(_) => numbered += 1,
            other => {
                return Err(LuaError::runtime(format!(
                    "{owner}: field `{path}` has a {} key, but only names and list positions are accepted",
                    node::describe(&other)
                )));
            }
        }
    }

    match (named.is_empty(), numbered) {
        (false, 0) => Ok(GenericValue::Map(named)),
        (true, _) if numbered == table.raw_len() => {
            let mut values = Vec::with_capacity(numbered);
            for (offset, value) in table.sequence_values::<Value>().enumerate() {
                values.push(generic_value(owner, &format!("{path}[{}]", offset + 1), &value?)?);
            }
            Ok(GenericValue::List(values))
        }
        (true, _) => Err(LuaError::runtime(format!("{owner}: field `{path}` is a list with gaps"))),
        (false, _) => Err(LuaError::runtime(format!(
            "{owner}: field `{path}` mixes names with list positions, so it is neither a map nor a list"
        ))),
    }
}

fn state_drive(target: ParameterDriveTarget<Declared>) -> node::Behavior {
    node::Behavior(Behavior::ParameterDrive(target))
}

fn ordered<T: PartialOrd + std::fmt::Display + Copy>(owner: &'static str, range: [T; 2]) -> LuaResult<[T; 2]> {
    if range[0] > range[1] {
        return Err(LuaError::runtime(format!(
            "{owner}: a range runs from its minimum to its maximum, but {} is greater than {}",
            range[0], range[1]
        )));
    }
    Ok(range)
}

fn range(owner: &'static str, key: &str, written: &Value) -> LuaResult<[f64; 2]> {
    match animated_value::<()>(owner, written) {
        Ok(AnimatedValue::Vector2(range)) => Ok([range.x, range.y]),
        _ => Err(LuaError::runtime(format!("{owner}: option `{key}` is a range written as two numbers"))),
    }
}

fn parameter_drive(lua: &Lua, parameter: String, value: AnimatedValue<()>) -> node::Drive {
    node::Drive(Drive::Parameter {
        parameter: located(lua, parameter),
        value,
    })
}

fn located<T>(lua: &Lua, value: T) -> Unresolved<T> {
    match caller_location(lua) {
        Some(at) => Unresolved::located(value, at),
        None => Unresolved::new(value),
    }
}

fn wrong_value(owner: &'static str, expected: &str, value: &Value) -> LuaError {
    LuaError::runtime(format!("{owner}: the value must be {expected}, but {} was written", node::describe(value)))
}

#[cfg(test)]
mod tests {
    use mlua::FromLua;
    use rstest::*;

    use super::*;
    use crate::{
        core::resolution::SourceLocation,
        lua::testing::{eval, eval_error},
    };

    fn drive_of(expression: &str) -> Drive {
        let (lua, value) = eval(expression);
        node::Drive::from_lua(value, &lua).expect("a drive should be built").0
    }

    fn behavior_of(expression: &str) -> Behavior {
        let (lua, value) = eval(expression);
        node::Behavior::from_lua(value, &lua).expect("a behavior should be built").0
    }

    fn tracking_values(expression: &str) -> HashMap<TrackingControlTarget, TrackingControlMode> {
        match behavior_of(expression) {
            Behavior::TrackingControl(control) => control.values,
            other => panic!("expected a tracking control, got {other:?}"),
        }
    }

    #[rstest]
    fn a_layer_drive_names_the_layer_it_points_at() {
        assert_eq!(
            drive_of("da.drive_group('Expressions', 'smile')"),
            Drive::Group {
                layer: Unresolved::new("Expressions".into()),
                option: "smile".into(),
            },
        );
        assert_eq!(
            drive_of("da.drive_switch('Hat')"),
            Drive::Switch {
                layer: Unresolved::new("Hat".into()),
                value: None,
            },
        );
        assert_eq!(
            drive_of("da.drive_switch('Hat', false)"),
            Drive::Switch {
                layer: Unresolved::new("Hat".into()),
                value: Some(false),
            },
        );
        assert_eq!(
            drive_of("da.drive_puppet('Wink')"),
            Drive::Puppet {
                layer: Unresolved::new("Wink".into()),
                value: None,
            },
        );
        assert_eq!(
            drive_of("da.drive_puppet('Wink', 0.5)"),
            Drive::Puppet {
                layer: Unresolved::new("Wink".into()),
                value: Some(0.5),
            },
        );
    }

    #[rstest]
    #[case::boolean("da.drive_bool('Hat', true)", AnimatedValue::Bool(true))]
    #[case::integer("da.drive_int('Emote', 3)", AnimatedValue::Int(3))]
    #[case::float("da.drive_float('Blend', 0.25)", AnimatedValue::Float(0.25))]
    #[case::float_from_integer("da.drive_float('Blend', 1)", AnimatedValue::Float(1.0))]
    fn a_parameter_drive_keeps_the_type_of_its_builder(#[case] expression: &str, #[case] expected: AnimatedValue<()>) {
        let Drive::Parameter { value, .. } = drive_of(expression) else {
            panic!("expected a parameter drive");
        };
        assert_eq!(value, expected);
    }

    #[rstest]
    #[case::bool_given_number("da.drive_bool('Hat', 1)", "must be a boolean, but number was written")]
    #[case::int_given_float("da.drive_int('Emote', 1.5)", "must be an integer, but number was written")]
    #[case::float_given_string("da.drive_float('Blend', 'x')", "must be a number, but string was written")]
    fn a_parameter_drive_rejects_a_value_of_another_type(#[case] expression: &str, #[case] expected: &str) {
        let message = eval_error(expression);
        assert!(message.contains(expected), "{message}");
    }

    #[rstest]
    fn a_drive_records_where_it_was_written() {
        let (lua, value) = eval("(function()\n  return da.drive_switch('Hat')\nend)()");
        let Drive::Switch { layer, .. } = node::Drive::from_lua(value, &lua).expect("a drive should be built").0 else {
            panic!("expected a switch drive");
        };

        assert_eq!(
            layer.at,
            Some(SourceLocation {
                chunk: "test.lua".into(),
                line: 3,
            })
        );
    }

    #[rstest]
    fn tracking_applies_one_mode_to_every_target() {
        assert_eq!(
            tracking_values("da.tracking('animation', { 'head', 'eyes' })"),
            HashMap::from([
                (TrackingControlTarget::Head, TrackingControlMode::Animation),
                (TrackingControlTarget::Eyes, TrackingControlMode::Animation),
            ]),
        );
        assert_eq!(
            tracking_values("da.tracking('tracking', { 'left_hand' })"),
            HashMap::from([(TrackingControlTarget::LeftHand, TrackingControlMode::Tracking)]),
        );
    }

    #[rstest]
    fn every_tracking_target_is_written_in_snake_case() {
        let written: Vec<_> = TRACKING_TARGETS.iter().map(|(name, _)| format!("'{name}'")).collect();
        let values = tracking_values(&format!("da.tracking('animation', {{ {} }})", written.join(", ")));

        assert_eq!(values.len(), TRACKING_TARGETS.len());
    }

    #[rstest]
    fn an_unknown_tracking_mode_or_target_lists_the_accepted_ones() {
        let message = eval_error("da.tracking('none', { 'head' })");
        assert!(message.contains("da.tracking: mode `none` is not known"), "{message}");
        assert!(message.contains("`tracking`, `animation`"), "{message}");

        let message = eval_error("da.tracking('animation', { 'leftHand' })");
        assert!(message.contains("da.tracking: target `leftHand` is not known"), "{message}");
        assert!(message.contains("`left_hand`"), "{message}");
    }

    #[rstest]
    fn tracking_needs_at_least_one_target() {
        let message = eval_error("da.tracking('animation', {})");
        assert!(message.contains("da.tracking: at least one target is needed"), "{message}");
    }

    fn state_drive_of(expression: &str) -> ParameterDriveTarget<Declared> {
        match behavior_of(expression) {
            Behavior::ParameterDrive(target) => target,
            other => panic!("expected a state drive, got {other:?}"),
        }
    }

    fn named(name: &str) -> Unresolved<String> {
        Unresolved::new(name.into())
    }

    #[rstest]
    #[case::add_int("da.drive_add('Emote', 2)", ParameterDriveTarget::Add { parameter: named("Emote"), value: AnimatedValue::Int(2) })]
    #[case::add_float("da.drive_add('Blend', -0.5)", ParameterDriveTarget::Add { parameter: named("Blend"), value: AnimatedValue::Float(-0.5) })]
    #[case::random_int("da.drive_random_int('Emote', 1, 4)", ParameterDriveTarget::RandomInt { parameter: named("Emote"), range: [1, 4] })]
    #[case::random_bool("da.drive_random_bool('Coin', 0.25)", ParameterDriveTarget::RandomBool { parameter: named("Coin"), chance: 0.25 })]
    #[case::random_bool_even("da.drive_random_bool('Coin')", ParameterDriveTarget::RandomBool { parameter: named("Coin"), chance: 0.5 })]
    #[case::random_float("da.drive_random_float('Blend', -1, 1)", ParameterDriveTarget::RandomFloat { parameter: named("Blend"), range: [-1.0, 1.0] })]
    #[case::copy("da.drive_copy('A', 'B')", ParameterDriveTarget::Copy { from: named("A"), to: named("B") })]
    #[case::ranged_copy(
        "da.drive_copy('A', 'B', { from_range = { 0, 1 }, to_range = da.vec2(-1, 1) })",
        ParameterDriveTarget::RangedCopy { from: named("A"), from_range: [0.0, 1.0], to: named("B"), to_range: [-1.0, 1.0] },
    )]
    fn a_state_drive_is_a_behavior(#[case] expression: &str, #[case] expected: ParameterDriveTarget<Declared>) {
        assert_eq!(state_drive_of(expression), expected);
    }

    #[rstest]
    #[case::add_string("da.drive_add('Emote', 'x')", "da.drive_add: the value must be a number")]
    #[case::random_int_float("da.drive_random_int('Emote', 0, 1.5)", "da.drive_random_int: the value must be an integer")]
    #[case::random_int_backwards("da.drive_random_int('Emote', 4, 1)", "4 is greater than 1")]
    #[case::random_float_backwards("da.drive_random_float('Blend', 1, -1)", "1 is greater than -1")]
    #[case::chance("da.drive_random_bool('Coin', 1.5)", "a chance is between 0 and 1")]
    #[case::one_range("da.drive_copy('A', 'B', { from_range = { 0, 1 } })", "`from_range` and `to_range` are written together")]
    #[case::bad_range("da.drive_copy('A', 'B', { from_range = { 0, 1, 2 }, to_range = { 0, 1 } })", "option `from_range` is a range")]
    fn a_bad_state_drive_is_rejected_where_it_is_written(#[case] expression: &str, #[case] expected: &str) {
        let message = eval_error(expression);
        assert!(message.contains(expected), "{message}");
        assert!(message.contains("test.lua:2:"), "{message}");
    }

    #[rstest]
    fn a_state_drive_cannot_trigger_a_menu_item() {
        let message = eval_error("da.toggle('Coin', da.drive_random_bool('Coin'))");
        assert!(message.contains("expected drive, got behavior"), "{message}");
    }

    #[rstest]
    fn a_generic_behavior_holds_plain_data() {
        let Behavior::Generic(generic) = behavior_of(
            "da.behavior('VRCAnimatorLayerControl', { goalWeight = 0.5, layer = 3, debugString = 'x', off = false, list = { 1, false }, nested = { x = {} } })",
        ) else {
            panic!("expected a generic behavior");
        };

        assert_eq!(generic.type_name, named("VRCAnimatorLayerControl"));
        assert_eq!(
            generic.type_name.at,
            Some(SourceLocation {
                chunk: "test.lua".into(),
                line: 2,
            })
        );
        assert_eq!(
            generic.fields,
            BTreeMap::from([
                ("debugString".to_owned(), GenericValue::String("x".into())),
                ("goalWeight".to_owned(), GenericValue::Float(0.5)),
                ("layer".to_owned(), GenericValue::Int(3)),
                ("list".to_owned(), GenericValue::List(vec![GenericValue::Int(1), GenericValue::Bool(false)])),
                (
                    "nested".to_owned(),
                    GenericValue::Map(BTreeMap::from([("x".to_owned(), GenericValue::List(vec![]))]))
                ),
                ("off".to_owned(), GenericValue::Bool(false)),
            ])
        );
    }

    #[rstest]
    #[case::no_fields("da.behavior('Custom')")]
    #[case::empty_fields("da.behavior('Custom', {})")]
    fn a_generic_behavior_may_have_no_fields(#[case] expression: &str) {
        let Behavior::Generic(generic) = behavior_of(expression) else {
            panic!("expected a generic behavior");
        };
        assert!(generic.fields.is_empty());
    }

    #[rstest]
    #[case::positional("da.behavior('Custom', { 1, 2 })", "fields are named")]
    #[case::mixed("da.behavior('Custom', { a = { 1, x = 2 } })", "field `a` mixes names with list positions")]
    #[case::gap("da.behavior('Custom', { a = { [1] = 1, [3] = 3 } })", "field `a` is a list with gaps")]
    #[case::vector("da.behavior('Custom', { a = { b = da.vec2(1, 2) } })", "field `a.b` is vector")]
    #[case::function("da.behavior('Custom', { a = { print } })", "field `a[1]` is function")]
    fn a_generic_behavior_rejects_what_it_cannot_carry(#[case] expression: &str, #[case] expected: &str) {
        let message = eval_error(expression);
        assert!(message.contains(expected), "{message}");
    }

    #[rstest]
    fn a_false_entry_in_the_target_list_is_dropped() {
        let values = tracking_values("da.tracking('animation', { 'head', (1 == 2) and 'eyes' })");
        assert_eq!(values, HashMap::from([(TrackingControlTarget::Head, TrackingControlMode::Animation)]));
    }
}
