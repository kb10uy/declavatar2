pub mod api;
pub mod content;
pub mod error;
pub mod list;
pub mod location;
pub mod module;
pub mod node;
pub mod options;
pub mod runtime;
pub mod value;

pub use error::ScriptError;
pub use module::{DirectoryLoader, ModuleLoader, ModuleSource};
pub use runtime::{EvaluateOptions, evaluate};

#[cfg(test)]
pub(crate) mod testing {
    use mlua::{Lua, Value};

    use crate::lua::{EvaluateOptions, runtime::create_runtime};

    /// Directory tree of Lua modules, removed when the test ends.
    pub(crate) struct TempTree(std::path::PathBuf);

    impl TempTree {
        pub(crate) fn new() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};

            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!("declavatar2-modules-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&root).expect("temporary tree should be created");
            Self(root)
        }

        pub(crate) fn write(&self, relative: &str, contents: &str) -> std::path::PathBuf {
            let path = self.0.join(relative);
            std::fs::create_dir_all(path.parent().expect("a file has a parent")).expect("directory should be created");
            std::fs::write(&path, contents).expect("file should be written");
            path
        }

        pub(crate) fn path(&self) -> &std::path::PathBuf {
            &self.0
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn state() -> Lua {
        create_runtime(&EvaluateOptions::new()).expect("runtime should be prepared")
    }

    /// Evaluates one expression with `da` in scope.
    ///
    /// The state is returned alongside the value because a Lua value is only usable while its state lives.
    pub(crate) fn eval(expression: &str) -> (Lua, Value) {
        let lua = state();
        let value = lua
            .load(format!("local da = require 'declavatar'\nreturn {expression}"))
            .set_name("@test.lua")
            .call(())
            .expect("expression should evaluate");
        (lua, value)
    }

    /// Evaluates a whole script and takes the avatar it returns.
    pub(crate) fn avatar(source: &str) -> crate::decl::Avatar {
        crate::lua::evaluate(source, "avatar.lua", &EvaluateOptions::new()).expect("script should evaluate")
    }

    /// Evaluates a whole script that is expected to fail, and returns the message.
    pub(crate) fn avatar_error(source: &str) -> String {
        crate::lua::evaluate(source, "avatar.lua", &EvaluateOptions::new())
            .expect_err("script should be rejected")
            .to_string()
    }

    /// Evaluates one expression that is expected to fail, and returns the message.
    pub(crate) fn eval_error(expression: &str) -> String {
        let lua = state();
        lua.load(format!("local da = require 'declavatar'\nreturn {expression}"))
            .set_name("@test.lua")
            .call::<Value>(())
            .expect_err("expression should be rejected")
            .to_string()
    }
}
