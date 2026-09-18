use mlua::{AnyUserData, Value};

use crate::{decl, lua::value::VectorValue};

/// Declares the userdata wrappers that builders hand back to scripts.
///
/// Each wrapper carries the name a script sees in error messages, so that a value used in the
/// wrong place is reported by its kind rather than as bare userdata.
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

            impl From<$node> for $inner {
                fn from(node: $node) -> Self {
                    node.0
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

            impl mlua::FromLua for $node {
                fn from_lua(value: mlua::Value, _: &mlua::Lua) -> mlua::Result<Self> {
                    match &value {
                        mlua::Value::UserData(userdata) if userdata.is::<$node>() => Ok(userdata.borrow::<$node>()?.clone()),
                        other => Err(mlua::Error::runtime(format!("expected {}, got {}", $name, $crate::lua::node::describe(other)))),
                    }
                }
            }
        )*
    };
}

declare_nodes! {
    /// Avatar that `da.avatar` returns and the script gives back to the host.
    "avatar" => Avatar(decl::Avatar),

    /// Entry of the `parameters` block.
    "parameter" => Parameter(decl::parameter::Parameter),

    /// Animated target and the value written for it.
    "target" => Target(crate::unity::animation::FixedAnimationEntry<crate::core::phase::Declared>),

    /// Entry of the `fx_controller` block.
    "layer" => Layer(decl::layer::Layer),

    /// Default state of a group layer, written with `da.default`.
    "default" => GroupDefault(decl::behavior::Content),

    /// One option of a group layer, written with `da.option`.
    "option" => GroupOption(decl::layer::GroupOption),

    /// One keyframe of a puppet layer, written with `da.keyframe`.
    "keyframe" => Keyframe(decl::layer::PuppetKeyframe),

    /// Entry of the `menu` block.
    "menu item" => MenuItem(decl::menu::MenuItem),

    /// One axis of a puppet menu item, written with `da.axis`.
    "axis" => Axis(decl::menu::Axis),

    /// State of a raw layer, with the transitions written inside it.
    "state" => RawState(crate::lua::api::raw::PendingState),

    /// Transition of a raw layer, whose source may still be implied by a state.
    "transition" => RawTransition(crate::lua::api::raw::PendingTransition),

    /// What a raw state plays.
    "motion" => Motion(decl::raw::Motion),

    /// Field of a blend tree placed on its axes.
    "field" => BlendTreeField(decl::raw::BlendTreeField),

    /// Field of a direct blend tree, weighted by its own parameter.
    "weighted field" => DirectBlendTreeField(decl::raw::DirectBlendTreeField),

    /// Condition of a raw transition.
    "condition" => Condition(decl::raw::Condition),

    /// Entry of the `exports` block.
    "export" => Export(decl::avatar::Export),

    /// Parameter drive written with `da.drive_*`.
    "drive" => Drive(decl::behavior::Drive),

    /// State behavior other than a drive, such as `da.tracking`.
    "behavior" => Behavior(decl::behavior::Behavior),

    /// Explicit asset locator written with `da.asset.*`.
    "asset" => Asset(crate::unity::external::AssetLocator),

    /// Vector written with `da.vec2`, `da.vec3` or `da.vec4`.
    "vector" => Vector(VectorValue),

    /// Color written with `da.color`.
    "color" => Color(nalgebra::Vector4<f64>),

    /// Rotation written with `da.quat`.
    "quaternion" => Quaternion(nalgebra::UnitQuaternion<f64>),
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
