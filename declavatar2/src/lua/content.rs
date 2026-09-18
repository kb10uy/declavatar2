use mlua::{Error as LuaError, FromLua, Lua, Result as LuaResult, Table, Value};

use crate::{
    core::phase::Declared,
    decl::behavior::{Animation, Behavior, Content},
    lua::{list, node},
    unity::animation::FixedAnimationEntry,
};

/// One entry of a content list, which mixes animated targets with state behaviors.
pub enum ContentItem {
    Target(FixedAnimationEntry<Declared>),
    Behavior(Behavior),
}

impl FromLua for ContentItem {
    fn from_lua(value: Value, lua: &Lua) -> LuaResult<Self> {
        match &value {
            Value::UserData(userdata) if userdata.is::<node::Target>() => Ok(Self::Target(node::Target::from_lua(value.clone(), lua)?.0)),
            Value::UserData(userdata) if userdata.is::<node::Drive>() => Ok(Self::Behavior(Behavior::Drive(node::Drive::from_lua(value.clone(), lua)?.0))),
            Value::UserData(userdata) if userdata.is::<node::Behavior>() => Ok(Self::Behavior(node::Behavior::from_lua(value.clone(), lua)?.0)),
            other => Err(LuaError::runtime(format!("expected target, drive or behavior, got {}", node::describe(other)))),
        }
    }
}

/// Reads a list that holds animated targets and state behaviors in any order.
///
/// Two entries animating the same target are not an error; the later one wins.
pub fn content(lua: &Lua, owner: &'static str, written: &Table) -> LuaResult<Content> {
    let mut animation = Animation::new();
    let mut behaviors = Vec::new();

    for item in list::collect::<ContentItem>(lua, owner, written)? {
        match item {
            ContentItem::Target(entry) => {
                animation.insert(entry);
            }
            ContentItem::Behavior(behavior) => behaviors.push(behavior),
        }
    }

    Ok(Content { animation, behaviors })
}

/// Reads a list that holds animated targets only, such as a puppet keyframe.
pub fn animation(lua: &Lua, owner: &'static str, written: &Table) -> LuaResult<Animation> {
    let mut animation = Animation::new();
    for target in list::collect::<node::Target>(lua, owner, written)? {
        animation.insert(target.0);
    }
    Ok(animation)
}
