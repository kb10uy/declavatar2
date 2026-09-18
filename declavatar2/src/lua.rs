pub mod api;
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
