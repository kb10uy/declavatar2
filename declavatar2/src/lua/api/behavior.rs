use std::collections::HashMap;

use mlua::{Error as LuaError, Lua, Result as LuaResult, Table, Value};

use crate::{
    core::resolution::Unresolved,
    decl::behavior::{Behavior, Drive},
    lua::{list, location::caller_location, node, options::one_of},
    unity::value::AnimatedValue,
    vrchat::state_behaviour::{TrackingControl, TrackingControlMode, TrackingControlTarget},
};

const TRACKING_MODES: &[(&str, TrackingControlMode)] = &[("tracking", TrackingControlMode::Tracking), ("animation", TrackingControlMode::Animation)];

const TRACKING_TARGETS: &[(&str, TrackingControlTarget)] = &[
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
    da.set("tracking", lua.create_function(tracking)?)?;
    Ok(())
}

fn drive_group(lua: &Lua, (layer, option): (String, String)) -> LuaResult<node::Drive> {
    Ok(node::Drive(Drive::Group {
        layer: located(lua, layer),
        option,
    }))
}

fn drive_switch(lua: &Lua, (layer, value): (String, Option<bool>)) -> LuaResult<node::Drive> {
    Ok(node::Drive(Drive::Switch {
        layer: located(lua, layer),
        value,
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

    #[rstest]
    fn a_false_entry_in_the_target_list_is_dropped() {
        let values = tracking_values("da.tracking('animation', { 'head', (1 == 2) and 'eyes' })");
        assert_eq!(values, HashMap::from([(TrackingControlTarget::Head, TrackingControlMode::Animation)]));
    }
}
