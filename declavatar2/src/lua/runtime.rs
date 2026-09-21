use std::{collections::BTreeSet, path::PathBuf, rc::Rc};

use mlua::{Lua, LuaOptions, Result as LuaResult, StdLib, Value};

use crate::{
    decl,
    lua::{
        api,
        error::ScriptError,
        module::{self, DirectoryLoader, ModuleLoader},
        node,
    },
};

/// What the host gives the interpreter besides the script itself.
#[derive(Default, Clone)]
pub struct EvaluateOptions {
    symbols: BTreeSet<String>,
    loaders: Vec<Rc<dyn ModuleLoader>>,
}

impl EvaluateOptions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Names that `da.symbol` reports as supplied.
    pub fn symbols(mut self, names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.symbols.extend(names.into_iter().map(Into::into));
        self
    }

    /// Appends a loader that `require` consults, after the ones already added.
    pub fn loader(mut self, loader: impl ModuleLoader + 'static) -> Self {
        self.loaders.push(Rc::new(loader));
        self
    }

    /// Appends a loader that reads modules from the given directories.
    pub fn library_paths(self, directories: impl IntoIterator<Item = impl Into<PathBuf>>) -> Self {
        self.loader(DirectoryLoader::new(directories))
    }
}

/// Evaluates one declaration script and takes the avatar it returns.
pub fn evaluate(source: &str, chunk_name: &str, options: &EvaluateOptions) -> Result<decl::Avatar, ScriptError> {
    let lua = create_runtime(options).map_err(ScriptError::Runtime)?;
    let returned: Value = lua
        .load(source.to_owned())
        .set_name(module::chunk_name(chunk_name))
        .call(())
        .map_err(ScriptError::Script)?;

    match &returned {
        Value::UserData(userdata) if userdata.is::<node::Avatar>() => {
            let avatar = userdata.borrow::<node::Avatar>().map_err(ScriptError::Script)?;
            Ok(avatar.0.clone())
        }
        other => Err(ScriptError::NotAnAvatar { found: node::describe(other) }),
    }
}

/// Loads the extension helpers, which are plain Lua shipped inside the binary rather than host features.
fn extension_module(lua: &Lua) -> LuaResult<mlua::Function> {
    lua.load(include_str!("../../lua/runtime/ext.lua"))
        .set_name(module::chunk_name("declavatar.ext"))
        .into_function()
}

/// Creates an interpreter that cannot reach the host except through the configured loaders.
pub(crate) fn create_runtime(options: &EvaluateOptions) -> LuaResult<Lua> {
    let libraries = StdLib::COROUTINE | StdLib::TABLE | StdLib::STRING | StdLib::UTF8 | StdLib::MATH | StdLib::PACKAGE;
    let lua = Lua::new_with(libraries, LuaOptions::default())?;

    module::remove_filesystem_globals(&lua)?;
    module::install_searchers(&lua, &options.loaders)?;

    lua.preload_module("declavatar", api::declavatar_module(&lua, &options.symbols)?)?;
    lua.preload_module("declavatar.ext", extension_module(&lua)?)?;

    Ok(lua)
}

#[cfg(test)]
mod tests {
    use rstest::*;

    use super::*;

    fn run(source: &str) -> Result<decl::Avatar, ScriptError> {
        evaluate(source, "avatar.lua", &EvaluateOptions::new())
    }

    fn message(error: ScriptError) -> String {
        error.to_string()
    }

    #[rstest]
    fn a_script_gives_back_the_avatar_it_builds() {
        let avatar = run(r#"
            local da = require "declavatar"
            return da.avatar({})
        "#)
        .expect("script should evaluate");

        assert_eq!(avatar, decl::Avatar::default());
    }

    #[rstest]
    fn the_blocks_of_an_avatar_are_optional() {
        let avatar = run(r#"
            local da = require "declavatar"
            return da.avatar()
        "#)
        .expect("script should evaluate");

        assert_eq!(avatar, decl::Avatar::default());
    }

    #[rstest]
    #[case::number("return 42", "number")]
    #[case::nothing("return", "nothing")]
    #[case::nothing_at_all("local x = 1", "nothing")]
    #[case::table("return {}", "table")]
    fn script_must_return_an_avatar(#[case] source: &str, #[case] expected: &str) {
        let error = run(source).expect_err("script should be rejected");
        assert!(matches!(&error, ScriptError::NotAnAvatar { found } if found == expected), "{error}");
    }

    #[rstest]
    fn script_error_points_at_the_chunk_and_line() {
        let error = run("local da = require \"declavatar\"\nerror(\"boom\")\n").expect_err("script should fail");
        let message = message(error);

        assert!(message.contains("avatar.lua:2: boom"), "{message}");
        assert!(message.contains("stack traceback:"), "{message}");
    }

    #[rstest]
    fn builder_error_points_at_the_call_site() {
        let error = run("local da = require \"declavatar\"\nreturn da.avatar({ fx_controller = {} })\n").expect_err("script should fail");
        let message = message(error);

        assert!(message.contains("da.avatar: unknown option `fx_controller`"), "{message}");
        assert!(message.contains("known options are parameters, controllers, menu"), "{message}");
        assert!(message.contains("avatar.lua:2:"), "{message}");
    }

    #[rstest]
    fn symbols_come_from_the_host() {
        const SCRIPT: &str = r#"
            local da = require "declavatar"
            if not da.symbol("ENABLE_HAT") then
                error("the symbol should have been supplied")
            end
            return da.avatar()
        "#;

        evaluate(SCRIPT, "avatar.lua", &EvaluateOptions::new().symbols(["ENABLE_HAT"])).expect("script should evaluate");

        let message = message(run(SCRIPT).expect_err("the symbol should be absent"));
        assert!(message.contains("the symbol should have been supplied"), "{message}");
    }

    #[rstest]
    #[case::io("io")]
    #[case::os("os")]
    #[case::debug("debug")]
    #[case::dofile("dofile")]
    #[case::loadfile("loadfile")]
    fn host_reaching_globals_are_absent(#[case] name: &str) {
        let source = format!("return {name}");
        let error = run(&source).expect_err("the global should be absent");
        assert!(matches!(&error, ScriptError::NotAnAvatar { found } if found == "nothing"), "{error}");
    }

    #[rstest]
    fn package_cannot_load_native_libraries() {
        let error = run("return package.loadlib").expect_err("loadlib should be absent");
        assert!(matches!(&error, ScriptError::NotAnAvatar { found } if found == "nothing"), "{error}");
    }

    #[rstest]
    fn requiring_an_unknown_module_fails() {
        let error = run("local x = require \"nonexistent\"\nreturn x").expect_err("require should fail");
        let message = message(error);

        assert!(message.contains("nonexistent"), "{message}");
    }

    #[rstest]
    fn standard_libraries_scripts_rely_on_are_present() {
        run(r#"
            local da = require "declavatar"
            local parts = {}
            for i = 1, 3 do
                table.insert(parts, tostring(math.floor(i * 1.5)))
            end
            assert(table.concat(parts, "-") == "1-3-4", "the standard libraries should work")
            return da.avatar()
        "#)
        .expect("script should evaluate");
    }
}

#[cfg(test)]
mod extension_tests {
    use rstest::*;

    use super::*;
    use crate::lua::testing::TempTree;

    /// Runs a script that fails unless `expression` builds exactly the list written in `expected`.
    fn assert_range(expression: &str, expected: &str) {
        let script = format!(
            "local da = require 'declavatar'\n\
             local ext = require 'declavatar.ext'\n\
             local joined = table.concat({expression}, ',')\n\
             assert(joined == '{expected}', 'got ' .. joined)\n\
             return da.avatar()\n"
        );
        evaluate(&script, "avatar.lua", &EvaluateOptions::new()).expect("script should evaluate");
    }

    #[rstest]
    fn the_extension_module_ships_inside_the_binary() {
        assert_range("ext.range(1, 4)", "1,2,3,4");
    }

    #[rstest]
    fn a_range_counts_down_when_its_step_is_negative() {
        assert_range("ext.range(3, 1, -1)", "3,2,1");
    }

    #[rstest]
    fn a_range_that_never_reaches_its_end_is_empty() {
        assert_range("ext.range(3, 1)", "");
    }

    #[rstest]
    fn a_range_step_must_not_be_zero() {
        let error = evaluate(
            "local da = require 'declavatar'\nlocal ext = require 'declavatar.ext'\nreturn da.avatar(#ext.range(1, 2, 0))\n",
            "avatar.lua",
            &EvaluateOptions::new(),
        )
        .expect_err("the step should be rejected");

        assert!(error.to_string().contains("the step must not be zero"), "{error}");
    }

    #[rstest]
    fn the_extension_module_cannot_be_shadowed_by_a_library_directory() {
        let tree = TempTree::new();
        tree.write("declavatar/ext.lua", "error('this must never load')");
        let options = EvaluateOptions::new().library_paths([tree.path()]);

        evaluate(
            "local da = require 'declavatar'\n\
             local ext = require 'declavatar.ext'\n\
             assert(#ext.range(1, 3) == 3, 'the preloaded module should win')\n\
             return da.avatar()\n",
            "avatar.lua",
            &options,
        )
        .expect("the preloaded module should win");
    }
}
