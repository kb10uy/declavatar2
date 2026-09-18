use mlua::{Error as LuaError, FromLua, Lua, Result as LuaResult, Table, Value};

use crate::lua::error::plain_message;

/// Reads a child list of one kind.
///
/// `false` entries are dropped so that `cond and da.bool("X")` reads as a conditional element,
/// and a nested list is an error because declavatar2 does not flatten on its own.
pub fn collect<T: FromLua>(lua: &Lua, owner: &'static str, list: &Table) -> LuaResult<Vec<T>> {
    let mut items = Vec::new();
    for (offset, value) in list.sequence_values::<Value>().enumerate() {
        let position = offset + 1;
        match value? {
            Value::Boolean(false) => continue,
            Value::Table(_) => {
                return Err(LuaError::runtime(format!(
                    "{owner}: entry {position} is a list; declavatar2 does not flatten lists, so use `da.flatten` or `table.insert`"
                )));
            }
            value => {
                let item = T::from_lua(value, lua).map_err(|error| LuaError::runtime(format!("{owner}: entry {position}: {}", plain_message(&error))))?;
                items.push(item);
            }
        }
    }
    Ok(items)
}

/// Expands one level of lists and drops `false`, so that groups of nodes can be spliced together.
pub fn flatten(lua: &Lua, values: impl IntoIterator<Item = Value>) -> LuaResult<Table> {
    let flattened = lua.create_table()?;
    for (offset, value) in values.into_iter().enumerate() {
        let argument = offset + 1;
        match value {
            Value::Boolean(false) => continue,
            Value::Table(inner) => {
                for (offset, value) in inner.sequence_values::<Value>().enumerate() {
                    match value? {
                        Value::Boolean(false) => continue,
                        Value::Table(_) => {
                            return Err(LuaError::runtime(format!(
                                "da.flatten: entry {} of argument {argument} is a list; `da.flatten` expands one level only",
                                offset + 1
                            )));
                        }
                        value => flattened.raw_push(value)?,
                    }
                }
            }
            value => flattened.raw_push(value)?,
        }
    }
    Ok(flattened)
}

#[cfg(test)]
mod tests {
    use rstest::*;

    use super::*;
    use crate::lua::{node, testing::eval, value::VectorValue};

    /// Collects vectors and reports the first component of each, which marks the entry it came from.
    fn marks(expression: &str) -> Vec<f64> {
        let (lua, value) = eval(expression);
        let Value::Table(list) = value else {
            panic!("the expression should build a list");
        };
        collect::<node::Vector>(&lua, "da.test", &list)
            .expect("list should be accepted")
            .into_iter()
            .map(|vector| match vector.0 {
                VectorValue::Two(components) => components.x,
                other => panic!("expected a two component vector, got {other:?}"),
            })
            .collect()
    }

    fn reject(expression: &str) -> String {
        let (lua, value) = eval(expression);
        let Value::Table(list) = value else {
            panic!("the expression should build a list");
        };
        collect::<node::Vector>(&lua, "da.test", &list)
            .expect_err("list should be rejected")
            .to_string()
    }

    #[rstest]
    fn nodes_are_collected_in_order() {
        assert_eq!(marks("{ da.vec2(1, 0), da.vec2(2, 0) }"), [1.0, 2.0]);
    }

    #[rstest]
    fn a_false_entry_is_dropped_so_that_a_condition_reads_naturally() {
        assert_eq!(marks("{ da.vec2(1, 0), false, da.vec2(2, 0) }"), [1.0, 2.0]);
        assert_eq!(marks("{ (1 == 2) and da.vec2(1, 0), da.vec2(2, 0) }"), [2.0]);
        assert_eq!(marks("{}"), Vec::<f64>::new());
    }

    #[rstest]
    fn a_nested_list_is_rejected_with_a_way_out() {
        let message = reject("{ da.vec2(1, 0), { da.vec2(2, 0) } }");
        assert!(message.contains("da.test: entry 2 is a list"), "{message}");
        assert!(message.contains("da.flatten"), "{message}");
    }

    #[rstest]
    fn a_wrong_kind_names_both_what_was_wanted_and_what_was_written() {
        let message = reject("{ da.vec2(1, 0), da.avatar() }");
        assert_eq!(message, "runtime error: da.test: entry 2: expected vector, got avatar");

        let message = reject("{ 'not a node' }");
        assert!(message.contains("da.test: entry 1: expected vector, got string"), "{message}");

        let message = reject("{ true }");
        assert!(message.contains("da.test: entry 1: expected vector, got boolean"), "{message}");
    }

    #[rstest]
    fn flatten_expands_one_level_and_drops_false() {
        assert_eq!(
            marks("da.flatten(da.vec2(1, 0), { da.vec2(2, 0), false, da.vec2(3, 0) }, false, da.vec2(4, 0))"),
            [1.0, 2.0, 3.0, 4.0],
        );
    }

    #[rstest]
    fn flatten_expands_one_level_only() {
        let message = crate::lua::testing::eval_error("da.flatten(da.vec2(1, 0), { { da.vec2(2, 0) } })");
        assert!(message.contains("da.flatten: entry 1 of argument 2 is a list"), "{message}");
        assert!(message.contains("expands one level only"), "{message}");
    }

    #[rstest]
    fn map_applies_the_function_with_the_index() {
        let (_lua, value) = eval("da.map({ 2, 3, 4 }, function(v, i) return v * i end)");
        let Value::Table(list) = value else {
            panic!("the expression should build a list");
        };
        let mapped: Vec<i64> = list.sequence_values::<i64>().map(|v| v.expect("number")).collect();
        assert_eq!(mapped, [2, 6, 12]);
    }
}
