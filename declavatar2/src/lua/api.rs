pub mod behavior;
pub mod controller;
pub mod layer;
pub mod menu;
pub mod parameter;
pub mod raw;
pub mod target;

use std::{collections::BTreeSet, rc::Rc};

use mlua::{Function, Lua, Result as LuaResult, Table, Value, Variadic};
use nalgebra::{Quaternion, UnitQuaternion, Vector4};

use crate::{
    decl,
    lua::{list, node, options::Options, value::VectorValue},
};

/// Builds the function that `require "declavatar"` evaluates to.
pub(crate) fn declavatar_module(lua: &Lua, symbols: &BTreeSet<String>) -> LuaResult<Function> {
    let symbols = Rc::new(symbols.clone());
    lua.create_function(move |lua, ()| {
        let da = lua.create_table()?;
        register_root(lua, &da, &symbols)?;
        Ok(da)
    })
}

fn register_root(lua: &Lua, da: &Table, symbols: &Rc<BTreeSet<String>>) -> LuaResult<()> {
    let known = Rc::clone(symbols);
    da.set("symbol", lua.create_function(move |_, name: String| Ok(known.contains(&name)))?)?;

    da.set("avatar", lua.create_function(avatar)?)?;
    da.set("flatten", lua.create_function(flatten)?)?;
    da.set("map", lua.create_function(map)?)?;

    da.set(
        "vec2",
        lua.create_function(|_, (x, y): (f64, f64)| Ok(node::Vector(VectorValue::from_components(&[x, y]).expect("two components"))))?,
    )?;
    da.set(
        "vec3",
        lua.create_function(|_, (x, y, z): (f64, f64, f64)| Ok(node::Vector(VectorValue::from_components(&[x, y, z]).expect("three components"))))?,
    )?;
    da.set(
        "vec4",
        lua.create_function(|_, (x, y, z, w): (f64, f64, f64, f64)| Ok(node::Vector(VectorValue::from_components(&[x, y, z, w]).expect("four components"))))?,
    )?;
    da.set("color", lua.create_function(color)?)?;
    da.set("quat", lua.create_function(quat)?)?;

    behavior::register(lua, da)?;
    controller::register(lua, da)?;
    layer::register(lua, da)?;
    menu::register(lua, da)?;
    parameter::register(lua, da)?;
    raw::register(lua, da)?;
    target::register(lua, da)?;

    Ok(())
}

fn avatar(lua: &Lua, blocks: Option<Table>) -> LuaResult<node::Avatar> {
    const OWNER: &str = "da.avatar";

    let mut options = Options::new(OWNER, blocks);
    let parameters = options.take::<Table>("parameters")?;
    let controllers = options.take::<Table>("controllers")?;
    let menu = options.take::<Table>("menu")?;
    let exports = options.take::<Table>("exports")?;
    options.finish()?;

    let parameters = block::<node::Parameter, _>(lua, "da.avatar: parameters", parameters)?;
    let controllers = block::<node::Controller, _>(lua, "da.avatar: controllers", controllers)?;
    let menu = block::<node::MenuItem, _>(lua, "da.avatar: menu", menu)?;
    let exports = block::<node::Export, _>(lua, "da.avatar: exports", exports)?;

    Ok(node::Avatar(decl::Avatar {
        parameters,
        controllers,
        menu,
        exports,
    }))
}

/// Reads one block of `da.avatar`, which is a child list that may be left out entirely.
fn block<T: mlua::FromLua + Into<U>, U>(lua: &Lua, owner: &'static str, written: Option<Table>) -> LuaResult<Vec<U>> {
    let Some(written) = written else {
        return Ok(Vec::new());
    };
    Ok(list::collect::<T>(lua, owner, &written)?.into_iter().map(Into::into).collect())
}

fn flatten(lua: &Lua, values: Variadic<Value>) -> LuaResult<Table> {
    list::flatten(lua, values)
}

fn map(lua: &Lua, (list, mapping): (Table, Function)) -> LuaResult<Table> {
    let mapped = lua.create_table()?;
    for (offset, value) in list.sequence_values::<Value>().enumerate() {
        let result: Value = mapping.call((value?, offset + 1))?;
        mapped.raw_push(result)?;
    }
    Ok(mapped)
}

fn color(_: &Lua, (r, g, b, a): (f64, f64, f64, Option<f64>)) -> LuaResult<node::Color> {
    Ok(node::Color(Vector4::new(r, g, b, a.unwrap_or(1.0))))
}

fn quat(_: &Lua, (x, y, z, w): (f64, f64, f64, f64)) -> LuaResult<node::Quaternion> {
    Ok(node::Quaternion(UnitQuaternion::from_quaternion(Quaternion::new(w, x, y, z))))
}

/// Guards the Lua type definitions against the builders they describe.
///
/// The definitions under `lua/types` are what an editor completes from, and nothing at
/// runtime reads them, so only a test keeps them honest.
#[cfg(test)]
mod definition_tests {
    use std::collections::BTreeSet;

    use mlua::Value;
    use rstest::*;

    use super::*;
    use crate::lua::{
        EvaluateOptions,
        api::{
            behavior::{TRACKING_MODES, TRACKING_TARGETS},
            controller::{MERGE_MODES, PATH_MODES, PLAYABLE_LAYERS},
            parameter::{PROVIDED_GROUPS, SCOPES},
            raw::{DIRECT_TREE_TYPE, PARAMETRIC_TREE_TYPES},
        },
        runtime::create_runtime,
    };

    const DECLAVATAR: &str = include_str!("../../lua/types/declavatar.lua");
    const EXTENSION: &str = include_str!("../../lua/types/declavatar/ext.lua");

    fn state() -> Lua {
        create_runtime(&EvaluateOptions::new()).expect("runtime should be prepared")
    }

    fn module(lua: &Lua, name: &str) -> Table {
        lua.load(format!("return require '{name}'"))
            .call(())
            .unwrap_or_else(|error| panic!("`{name}` should load: {error}"))
    }

    /// Every function the module holds, named by the path a script writes to reach it.
    fn registered(table: &Table, prefix: &str) -> BTreeSet<String> {
        let mut paths = BTreeSet::new();
        for pair in table.pairs::<String, Value>() {
            let (key, value) = pair.expect("the module should have string keys");
            let path = if prefix.is_empty() { key } else { format!("{prefix}.{key}") };
            match value {
                Value::Function(_) => {
                    paths.insert(path);
                }
                Value::Table(inner) => paths.extend(registered(&inner, &path)),
                other => panic!("`{path}` is a {}, which the definitions do not describe", other.type_name()),
            }
        }
        paths
    }

    /// Every function the definitions declare on the given table.
    fn documented(source: &str, receiver: &str) -> BTreeSet<String> {
        let prefix = format!("function {receiver}.");
        source
            .lines()
            .filter_map(|line| line.strip_prefix(&prefix))
            .filter_map(|rest| rest.split('(').next())
            .map(str::to_owned)
            .collect()
    }

    /// Every method the definitions declare on the given class.
    fn documented_methods(source: &str, class: &str) -> BTreeSet<String> {
        let prefix = format!("function {class}:");
        source
            .lines()
            .filter_map(|line| line.strip_prefix(&prefix))
            .filter_map(|rest| rest.split('(').next())
            .map(str::to_owned)
            .collect()
    }

    /// The members of an `---@alias` written as a union of string literals.
    fn documented_alias(source: &str, name: &str) -> BTreeSet<String> {
        let prefix = format!("---@alias {name} ");
        let line = source
            .lines()
            .find(|line| line.starts_with(&prefix))
            .unwrap_or_else(|| panic!("the definitions should declare `{name}`"));
        line[prefix.len()..]
            .split('|')
            .map(|member| member.trim().trim_matches('"').to_owned())
            .collect()
    }

    fn accepted<T>(choices: &[(&str, T)]) -> BTreeSet<String> {
        choices.iter().map(|(name, _)| (*name).to_owned()).collect()
    }

    #[rstest]
    fn every_builder_has_a_type_definition() {
        let lua = state();
        let registered = registered(&module(&lua, "declavatar"), "");
        let documented = documented(DECLAVATAR, "da");

        let undocumented: Vec<_> = registered.difference(&documented).collect();
        assert!(undocumented.is_empty(), "builders without a type definition: {undocumented:?}");

        let invented: Vec<_> = documented.difference(&registered).collect();
        assert!(invented.is_empty(), "type definitions without a builder: {invented:?}");
    }

    #[rstest]
    fn every_extension_helper_has_a_type_definition() {
        let lua = state();
        let registered = registered(&module(&lua, "declavatar.ext"), "");
        let documented = documented(EXTENSION, "ext");

        assert_eq!(registered, documented);
    }

    /// Methods live behind a protected metatable, so this only catches a definition that
    /// describes a method the runtime does not have, not the other way round.
    #[rstest]
    #[case::renderer("da.Renderer", "da.renderer('Body')")]
    #[case::object("da.Object", "da.object('Hat')")]
    #[case::component("da.Component", "da.component('Root', 'UnityEngine.Light')")]
    fn every_documented_method_exists_on_its_bound_object(#[case] class: &str, #[case] constructor: &str) {
        let class = class.rsplit('.').next().expect("a class name");
        let methods = documented_methods(DECLAVATAR, class);
        assert!(!methods.is_empty(), "`{class}` should document some methods");

        let lua = state();
        for method in methods {
            let kind: String = lua
                .load(format!("local da = require 'declavatar'\nreturn type(({constructor}).{method})"))
                .call(())
                .unwrap_or_else(|error| panic!("`{class}:{method}` should be reachable: {error}"));
            assert_eq!(kind, "function", "`{class}:{method}` is documented but the runtime has no such method");
        }
    }

    #[rstest]
    fn the_documented_enumerations_match_what_the_builders_accept() {
        assert_eq!(documented_alias(DECLAVATAR, "da.Scope"), accepted(SCOPES));
        assert_eq!(documented_alias(DECLAVATAR, "da.ProvidedGroup"), accepted(PROVIDED_GROUPS));
        assert_eq!(documented_alias(DECLAVATAR, "da.TrackingMode"), accepted(TRACKING_MODES));
        assert_eq!(documented_alias(DECLAVATAR, "da.TrackingTarget"), accepted(TRACKING_TARGETS));
        assert_eq!(documented_alias(DECLAVATAR, "da.PlayableLayer"), accepted(PLAYABLE_LAYERS));
        assert_eq!(documented_alias(DECLAVATAR, "da.MergeMode"), accepted(MERGE_MODES));
        assert_eq!(documented_alias(DECLAVATAR, "da.PathMode"), accepted(PATH_MODES));

        let mut tree_types = accepted(PARAMETRIC_TREE_TYPES);
        tree_types.insert(DIRECT_TREE_TYPE.to_owned());
        assert_eq!(documented_alias(DECLAVATAR, "da.BlendTreeType"), tree_types);
    }

    #[rstest]
    fn the_definitions_parse_as_lua() {
        let lua = state();
        for (name, source) in [("declavatar", DECLAVATAR), ("declavatar.ext", EXTENSION)] {
            lua.load(source)
                .set_name(format!("@{name}"))
                .into_function()
                .unwrap_or_else(|error| panic!("the definitions of `{name}` should be valid Lua: {error}"));
        }
    }

    #[rstest]
    fn the_definitions_are_declarations_only() {
        for (name, source) in [("declavatar", DECLAVATAR), ("declavatar.ext", EXTENSION)] {
            assert!(
                source.starts_with(&format!("---@meta {name}\n")),
                "`{name}` should open with its meta annotation so that `require` resolves to it",
            );
        }
    }
}
