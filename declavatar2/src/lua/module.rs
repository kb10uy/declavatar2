use std::{fs, path::PathBuf, rc::Rc};

use mlua::{Error as LuaError, Lua, Result as LuaResult, Table, Value};

/// Source of a module that `require` resolved to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleSource {
    /// Name that appears in tracebacks and in `SourceLocation`.
    pub chunk_name: String,
    pub source: String,
}

/// Supplies module sources to `require`, in place of the filesystem search Lua would do on its own.
pub trait ModuleLoader {
    /// Returns the source of `module`, `None` when this loader does not provide it,
    /// and `Err` when it should have provided it but could not.
    fn load(&self, module: &str) -> Result<Option<ModuleSource>, String>;
}

/// Loader that reads `<dir>/a/b.lua` and `<dir>/a/b/init.lua` for `require "a.b"`.
#[derive(Debug, Clone, Default)]
pub struct DirectoryLoader {
    directories: Vec<PathBuf>,
}

impl DirectoryLoader {
    pub fn new(directories: impl IntoIterator<Item = impl Into<PathBuf>>) -> Self {
        Self {
            directories: directories.into_iter().map(Into::into).collect(),
        }
    }

    fn candidates(&self, module: &str) -> Vec<PathBuf> {
        let relative = module.replace('.', "/");
        self.directories
            .iter()
            .flat_map(|directory| [directory.join(format!("{relative}.lua")), directory.join(&relative).join("init.lua")])
            .collect()
    }
}

impl ModuleLoader for DirectoryLoader {
    fn load(&self, module: &str) -> Result<Option<ModuleSource>, String> {
        if !is_safe_module_name(module) {
            return Ok(None);
        }

        for candidate in self.candidates(module) {
            if !candidate.is_file() {
                continue;
            }
            let source = fs::read_to_string(&candidate).map_err(|error| format!("cannot read {}: {error}", candidate.display()))?;
            return Ok(Some(ModuleSource {
                chunk_name: candidate.display().to_string(),
                source,
            }));
        }
        Ok(None)
    }
}

/// Whether a module name can be turned into a relative path without escaping the library directory.
fn is_safe_module_name(module: &str) -> bool {
    !module.is_empty()
        && module
            .split('.')
            .all(|segment| !segment.is_empty() && segment != ".." && segment.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'))
}

/// Turns a name into a Lua chunk name that reports itself verbatim in tracebacks.
pub(crate) fn chunk_name(name: &str) -> String {
    format!("@{name}")
}

/// Replaces the searchers that reach the filesystem with the given loaders, keeping `package.preload`.
pub(crate) fn install_searchers(lua: &Lua, loaders: &[Rc<dyn ModuleLoader>]) -> LuaResult<()> {
    let package: Table = lua.globals().get("package")?;
    package.set("loadlib", Value::Nil)?;
    package.set("path", "")?;
    package.set("cpath", "")?;

    let searchers: Table = package.get("searchers")?;
    let preload: Value = searchers.get(1)?;
    searchers.clear()?;
    searchers.set(1, preload)?;

    for (offset, loader) in loaders.iter().enumerate() {
        let loader = Rc::clone(loader);
        let searcher = lua.create_function(move |lua, module: String| match loader.load(&module) {
            Ok(Some(found)) => {
                let function = lua.load(found.source).set_name(chunk_name(&found.chunk_name)).into_function()?;
                Ok(Value::Function(function))
            }
            Ok(None) => Ok(Value::String(lua.create_string(format!("\n\tno module `{module}` in the declavatar loaders"))?)),
            Err(reason) => Err(LuaError::runtime(format!("cannot load module `{module}`: {reason}"))),
        })?;
        searchers.set(offset + 2, searcher)?;
    }

    Ok(())
}

/// Removes the standard functions that would let a script read the host filesystem on its own.
pub(crate) fn remove_filesystem_globals(lua: &Lua) -> LuaResult<()> {
    let globals = lua.globals();
    for name in ["dofile", "loadfile"] {
        globals.set(name, Value::Nil)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rstest::*;

    use super::*;
    use crate::lua::testing::TempTree;

    #[rstest]
    fn a_module_is_read_from_a_plain_file() {
        let tree = TempTree::new();
        tree.write("greeting.lua", "return 1");
        let loader = DirectoryLoader::new([tree.path()]);

        let found = loader.load("greeting").expect("lookup should succeed").expect("module should be found");
        assert_eq!(found.source, "return 1");
        assert!(found.chunk_name.ends_with("greeting.lua"), "{}", found.chunk_name);
    }

    #[rstest]
    fn a_dotted_module_becomes_a_nested_path() {
        let tree = TempTree::new();
        tree.write("pack/inner.lua", "return 2");
        let loader = DirectoryLoader::new([tree.path()]);

        let found = loader.load("pack.inner").expect("lookup should succeed").expect("module should be found");
        assert_eq!(found.source, "return 2");
    }

    #[rstest]
    fn a_directory_module_falls_back_to_init() {
        let tree = TempTree::new();
        tree.write("pack/init.lua", "return 3");
        let loader = DirectoryLoader::new([tree.path()]);

        let found = loader.load("pack").expect("lookup should succeed").expect("module should be found");
        assert_eq!(found.source, "return 3");
    }

    #[rstest]
    fn directories_are_searched_in_order() {
        let first = TempTree::new();
        let second = TempTree::new();
        first.write("shared.lua", "return 'first'");
        second.write("shared.lua", "return 'second'");
        let loader = DirectoryLoader::new([first.path(), second.path()]);

        let found = loader.load("shared").expect("lookup should succeed").expect("module should be found");
        assert_eq!(found.source, "return 'first'");
    }

    #[rstest]
    fn an_absent_module_is_not_an_error() {
        let tree = TempTree::new();
        let loader = DirectoryLoader::new([tree.path()]);

        assert_eq!(loader.load("missing"), Ok(None));
    }

    #[rstest]
    #[case::parent("..")]
    #[case::nested_parent("pack...inner")]
    #[case::separator("pack/inner")]
    #[case::backslash(r"pack\inner")]
    #[case::absolute("/etc/passwd")]
    #[case::empty("")]
    fn a_module_name_cannot_escape_the_library_directories(#[case] module: &str) {
        let tree = TempTree::new();
        let loader = DirectoryLoader::new([tree.path()]);

        assert_eq!(loader.load(module), Ok(None));
        assert!(!is_safe_module_name(module));
    }
}

#[cfg(test)]
mod script_tests {
    use rstest::*;

    use crate::lua::{EvaluateOptions, ScriptError, evaluate, testing::TempTree};

    #[rstest]
    fn a_script_requires_a_module_from_the_library_directories() {
        let tree = TempTree::new();
        tree.write("helper.lua", "return { greet = function() return 'hi' end }");
        let options = EvaluateOptions::new().library_paths([tree.path()]);

        evaluate(
            "local da = require 'declavatar'
local helper = require 'helper'
assert(helper.greet() == 'hi', 'the module should be the written one')
return da.avatar()
",
            "avatar.lua",
            &options,
        )
        .expect("script should evaluate");
    }

    #[rstest]
    fn an_error_inside_a_module_names_that_module() {
        let tree = TempTree::new();
        tree.write(
            "broken.lua",
            "local x = 1
error('inner boom')
",
        );
        let options = EvaluateOptions::new().library_paths([tree.path()]);

        let error = evaluate(
            "require 'broken'
",
            "avatar.lua",
            &options,
        )
        .expect_err("script should fail");
        let ScriptError::Script(error) = error else {
            panic!("the module error should surface as a script error");
        };
        let message = error.to_string();

        assert!(message.contains("broken.lua:2: inner boom"), "{message}");
    }

    #[rstest]
    fn the_declavatar_module_is_not_shadowed_by_a_library_directory() {
        let tree = TempTree::new();
        tree.write("declavatar.lua", "error('this must never load')");
        let options = EvaluateOptions::new().library_paths([tree.path()]);

        evaluate(
            "local da = require 'declavatar'
return da.avatar()
",
            "avatar.lua",
            &options,
        )
        .expect("the preloaded module should win");
    }
}
