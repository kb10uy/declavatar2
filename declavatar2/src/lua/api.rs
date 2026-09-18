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

    Ok(())
}

fn avatar(_: &Lua, blocks: Option<Table>) -> LuaResult<node::Avatar> {
    let mut options = Options::new("da.avatar", blocks);
    let _parameters = options.take::<Table>("parameters")?;
    let _fx_controller = options.take::<Table>("fx_controller")?;
    let _menu = options.take::<Table>("menu")?;
    let _exports = options.take::<Table>("exports")?;
    options.finish()?;

    Ok(node::Avatar(decl::Avatar::default()))
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
