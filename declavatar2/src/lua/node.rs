use mlua::{AnyUserData, Value};

use crate::decl;

/// Declares the userdata wrappers that builders hand back to scripts.
///
/// Each wrapper carries the name a script sees in error messages, so that a value
/// used in the wrong place is reported by its kind rather than as bare userdata.
macro_rules! declare_nodes {
    ($($(#[$meta:meta])* $name:literal => $node:ident($inner:ty)),* $(,)?) => {
        $(
            $(#[$meta])*
            #[derive(Debug, Clone, PartialEq)]
            pub struct $node(pub $inner);

            impl $node {
                pub const KIND: &'static str = $name;

                pub fn into_inner(self) -> $inner {
                    self.0
                }
            }

            impl From<$inner> for $node {
                fn from(value: $inner) -> Self {
                    Self(value)
                }
            }

            impl mlua::UserData for $node {
                fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
                    fields.add_meta_field("__name", $name);
                }

                fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
                    methods.add_meta_method("__tostring", |_, _, ()| Ok($name));
                }
            }
        )*
    };
}

declare_nodes! {
    /// Avatar that `da.avatar` returns and the script gives back to the host.
    "avatar" => Avatar(decl::Avatar),
}

/// How a value should be named when a builder rejects it.
///
/// Names follow what `type()` reports in a script, so an integer and a float are both a number.
pub fn describe(value: &Value) -> String {
    match value {
        Value::Nil => "nothing".into(),
        Value::Integer(_) | Value::Number(_) => "number".into(),
        Value::UserData(userdata) => describe_userdata(userdata),
        other => other.type_name().into(),
    }
}

fn describe_userdata(userdata: &AnyUserData) -> String {
    userdata.type_name().map(|name| name.to_string_lossy()).unwrap_or_else(|_| "userdata".into())
}
