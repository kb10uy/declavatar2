use mlua::Lua;

use crate::core::resolution::SourceLocation;

/// Where in the script the builder that is currently running was called from.
///
/// Level `0` is the builder itself, so the Lua frame that called it is level `1`.
pub fn caller_location(lua: &Lua) -> Option<SourceLocation> {
    lua.inspect_stack(1, |debug| {
        let line = debug.current_line()?;
        let chunk = debug.source().short_src?.into_owned();
        Some(SourceLocation { chunk, line: line as u32 })
    })
    .flatten()
}

#[cfg(test)]
mod tests {
    use mlua::Lua;
    use rstest::*;

    use super::*;

    fn probing_state() -> Lua {
        let lua = Lua::new();
        let probe = lua
            .create_function(|lua, ()| {
                let location = caller_location(lua).expect("the caller should be a Lua frame");
                Ok((location.chunk, location.line))
            })
            .expect("probe should be created");
        lua.globals().set("probe", probe).expect("probe should be installed");
        lua
    }

    #[rstest]
    fn location_names_the_chunk_and_the_calling_line() {
        let lua = probing_state();
        let (chunk, line): (String, u32) = lua
            .load("local a = 1\nlocal b = 2\nreturn probe()\n")
            .set_name("@avatar.lua")
            .call(())
            .expect("chunk should run");

        assert_eq!(chunk, "avatar.lua");
        assert_eq!(line, 3);
    }

    #[rstest]
    fn location_follows_the_immediate_caller_through_helpers() {
        let lua = probing_state();
        let (chunk, line): (String, u32) = lua
            .load("local function helper()\n  return probe()\nend\nreturn helper()\n")
            .set_name("@library.lua")
            .call(())
            .expect("chunk should run");

        assert_eq!(chunk, "library.lua");
        assert_eq!(line, 2);
    }
}
