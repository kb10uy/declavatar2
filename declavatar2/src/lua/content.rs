use mlua::{Error as LuaError, FromLua, Lua, Result as LuaResult, Table, Value};

use crate::{
    core::phase::Declared,
    decl::behavior::{Animation, Behavior, Content},
    lua::{list, node},
    unity::animation::FixedAnimationEntry,
};

/// One entry of a list that holds state behaviors, written as a drive or as a behavior.
pub struct BehaviorItem(pub Behavior);

impl FromLua for BehaviorItem {
    fn from_lua(value: Value, lua: &Lua) -> LuaResult<Self> {
        match behavior_from(&value, lua)? {
            Some(behavior) => Ok(Self(behavior)),
            None => Err(LuaError::runtime(format!("expected drive or behavior, got {}", node::describe(&value)))),
        }
    }
}

/// One entry of a content list, which mixes animated targets with state behaviors.
pub enum ContentItem {
    Target(FixedAnimationEntry<Declared>),
    Behavior(Behavior),
}

impl FromLua for ContentItem {
    fn from_lua(value: Value, lua: &Lua) -> LuaResult<Self> {
        if let Value::UserData(userdata) = &value
            && userdata.is::<node::Target>()
        {
            return Ok(Self::Target(node::Target::from_lua(value.clone(), lua)?.0));
        }
        match behavior_from(&value, lua)? {
            Some(behavior) => Ok(Self::Behavior(behavior)),
            None => Err(LuaError::runtime(format!("expected target, drive or behavior, got {}", node::describe(&value)))),
        }
    }
}

fn behavior_from(value: &Value, lua: &Lua) -> LuaResult<Option<Behavior>> {
    let Value::UserData(userdata) = value else {
        return Ok(None);
    };
    if userdata.is::<node::Drive>() {
        return Ok(Some(Behavior::Drive(node::Drive::from_lua(value.clone(), lua)?.0)));
    }
    if userdata.is::<node::Behavior>() {
        return Ok(Some(node::Behavior::from_lua(value.clone(), lua)?.0));
    }
    Ok(None)
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

/// Reads a list that holds state behaviors only, such as the `behaviors` of a raw state.
pub fn behaviors(lua: &Lua, owner: &'static str, written: &Table) -> LuaResult<Vec<Behavior>> {
    Ok(list::collect::<BehaviorItem>(lua, owner, written)?.into_iter().map(|item| item.0).collect())
}

/// Reads a list that holds animated targets only, such as a puppet keyframe.
pub fn animation(lua: &Lua, owner: &'static str, written: &Table) -> LuaResult<Animation> {
    let mut animation = Animation::new();
    for target in list::collect::<node::Target>(lua, owner, written)? {
        animation.insert(target.0);
    }
    Ok(animation)
}
