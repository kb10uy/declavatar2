use std::collections::BTreeSet;
use std::rc::Rc;

use mlua::{Function, Lua, Result as LuaResult, Table};

use crate::{
    decl,
    lua::{node, options::Options},
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
