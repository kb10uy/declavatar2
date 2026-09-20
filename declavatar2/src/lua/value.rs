use mlua::{Error as LuaError, FromLua, Lua, Result as LuaResult, Table, Value};
use nalgebra::{Vector2, Vector3, Vector4};

use crate::{lua::node, unity::value::AnimatedValue};

pub(crate) struct StrictBoolean(pub bool);

impl FromLua for StrictBoolean {
    fn from_lua(value: Value, _: &Lua) -> LuaResult<Self> {
        match value {
            Value::Boolean(value) => Ok(Self(value)),
            other => Err(LuaError::runtime(format!("expected a boolean, got {}", node::describe(&other)))),
        }
    }
}

/// Vector written with `da.vec2`, `da.vec3` or `da.vec4`, or as a plain table of numbers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VectorValue {
    Two(Vector2<f64>),
    Three(Vector3<f64>),
    Four(Vector4<f64>),
}

impl VectorValue {
    pub fn from_components(components: &[f64]) -> Option<Self> {
        match components {
            [x, y] => Some(Self::Two(Vector2::new(*x, *y))),
            [x, y, z] => Some(Self::Three(Vector3::new(*x, *y, *z))),
            [x, y, z, w] => Some(Self::Four(Vector4::new(*x, *y, *z, *w))),
            _ => None,
        }
    }

    pub fn into_animated_value<R>(self) -> AnimatedValue<R> {
        match self {
            Self::Two(vector) => AnimatedValue::Vector2(vector),
            Self::Three(vector) => AnimatedValue::Vector3(vector),
            Self::Four(vector) => AnimatedValue::Vector4(vector),
        }
    }
}

/// Reads the value written for an animated property.
///
/// Object references do not come through here; they are written with `:material` and `:reference`.
pub fn animated_value<R>(owner: &'static str, value: &Value) -> LuaResult<AnimatedValue<R>> {
    match value {
        Value::Boolean(written) => Ok(AnimatedValue::Bool(*written)),
        Value::Integer(written) => Ok(AnimatedValue::Int(*written)),
        Value::Number(written) => Ok(AnimatedValue::Float(*written)),
        Value::UserData(userdata) if userdata.is::<node::Vector>() => Ok(userdata.borrow::<node::Vector>()?.0.into_animated_value()),
        Value::UserData(userdata) if userdata.is::<node::Color>() => Ok(AnimatedValue::Color(userdata.borrow::<node::Color>()?.0)),
        Value::UserData(userdata) if userdata.is::<node::Quaternion>() => Ok(AnimatedValue::Quaternion(userdata.borrow::<node::Quaternion>()?.0)),
        Value::Table(table) => vector_from_table(table)?
            .map(VectorValue::into_animated_value)
            .ok_or_else(|| LuaError::runtime(format!("{owner}: a table written as a value must hold two to four numbers"))),
        other => Err(LuaError::runtime(format!("{owner}: a {} cannot be animated", node::describe(other)))),
    }
}

fn vector_from_table(table: &Table) -> LuaResult<Option<VectorValue>> {
    let mut components = Vec::new();
    for value in table.sequence_values::<Value>() {
        match value? {
            Value::Integer(written) => components.push(written as f64),
            Value::Number(written) => components.push(written),
            _ => return Ok(None),
        }
    }
    Ok(VectorValue::from_components(&components))
}

#[cfg(test)]
mod tests {
    use nalgebra::{Quaternion, UnitQuaternion};
    use rstest::*;

    use super::*;
    use crate::lua::testing::{eval, eval_error};

    fn read(expression: &str) -> AnimatedValue<()> {
        let (_lua, value) = eval(expression);
        animated_value("da.test", &value).expect("value should be accepted")
    }

    fn reject(expression: &str) -> String {
        let (_lua, value) = eval(expression);
        animated_value::<()>("da.test", &value).expect_err("value should be rejected").to_string()
    }

    #[rstest]
    #[case::boolean("true", AnimatedValue::Bool(true))]
    #[case::integer("3", AnimatedValue::Int(3))]
    #[case::float("1.5", AnimatedValue::Float(1.5))]
    fn primitives_keep_the_type_written_in_the_script(#[case] expression: &str, #[case] expected: AnimatedValue<()>) {
        assert_eq!(read(expression), expected);
    }

    #[rstest]
    #[case::vec2("da.vec2(1, 2)", AnimatedValue::Vector2(Vector2::new(1.0, 2.0)))]
    #[case::vec3("da.vec3(1, 2, 3)", AnimatedValue::Vector3(Vector3::new(1.0, 2.0, 3.0)))]
    #[case::vec4("da.vec4(1, 2, 3, 4)", AnimatedValue::Vector4(Vector4::new(1.0, 2.0, 3.0, 4.0)))]
    #[case::table2("{ 1, 2 }", AnimatedValue::Vector2(Vector2::new(1.0, 2.0)))]
    #[case::table3("{ 1, 2.5, 3 }", AnimatedValue::Vector3(Vector3::new(1.0, 2.5, 3.0)))]
    #[case::table4("{ 1, 2, 3, 4 }", AnimatedValue::Vector4(Vector4::new(1.0, 2.0, 3.0, 4.0)))]
    fn vectors_come_from_a_constructor_or_a_plain_table(#[case] expression: &str, #[case] expected: AnimatedValue<()>) {
        assert_eq!(read(expression), expected);
    }

    #[rstest]
    fn a_color_defaults_to_an_opaque_alpha() {
        assert_eq!(read("da.color(0.1, 0.2, 0.3)"), AnimatedValue::Color(Vector4::new(0.1, 0.2, 0.3, 1.0)));
        assert_eq!(read("da.color(0.1, 0.2, 0.3, 0.4)"), AnimatedValue::Color(Vector4::new(0.1, 0.2, 0.3, 0.4)));
    }

    #[rstest]
    fn a_quaternion_is_normalized_and_takes_w_last() {
        let expected = UnitQuaternion::from_quaternion(Quaternion::new(4.0, 1.0, 2.0, 3.0));
        assert_eq!(read("da.quat(1, 2, 3, 4)"), AnimatedValue::Quaternion(expected));
        assert_eq!(read("da.quat(0, 0, 0, 2)"), AnimatedValue::Quaternion(UnitQuaternion::identity()));
    }

    #[rstest]
    fn a_table_is_never_read_as_a_color_or_a_rotation() {
        assert_eq!(read("{ 1, 2, 3, 4 }"), AnimatedValue::Vector4(Vector4::new(1.0, 2.0, 3.0, 4.0)));
    }

    #[rstest]
    #[case::too_few("{ 1 }")]
    #[case::too_many("{ 1, 2, 3, 4, 5 }")]
    #[case::not_numbers("{ 1, 'two' }")]
    #[case::empty("{}")]
    fn a_table_that_is_not_a_vector_is_rejected(#[case] expression: &str) {
        let message = reject(expression);
        assert!(message.contains("must hold two to four numbers"), "{message}");
    }

    #[rstest]
    fn a_value_that_cannot_be_animated_is_named_in_the_message() {
        let message = reject("'hello'");
        assert!(message.contains("da.test: a string cannot be animated"), "{message}");

        let message = reject("da.avatar()");
        assert!(message.contains("da.test: a avatar cannot be animated"), "{message}");
    }

    #[rstest]
    fn vector_constructors_require_their_components() {
        let message = eval_error("da.vec3(1, 2)");
        assert!(message.contains("bad argument #3"), "{message}");
        assert!(message.contains("test.lua:2"), "{message}");
    }
}
