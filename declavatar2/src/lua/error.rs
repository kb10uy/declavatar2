use thiserror::Error;

/// Failure of evaluating a declaration script.
#[derive(Debug, Error)]
pub enum ScriptError {
    /// The interpreter could not be prepared. This is a fault of the host, not of the script.
    #[error("failed to prepare the Lua runtime: {0}")]
    Runtime(#[source] mlua::Error),

    /// The script raised an error. Its message carries the Lua traceback.
    #[error("{0}")]
    Script(#[source] mlua::Error),

    /// The script finished without returning an avatar.
    #[error("the script must end with `return da.avatar(...)`, but it returned {found}")]
    NotAnAvatar { found: String },
}

/// The message of a Lua error without the `runtime error:` prefix, for composing into another one.
pub(crate) fn plain_message(error: &mlua::Error) -> String {
    match error {
        mlua::Error::RuntimeError(message) => message.clone(),
        other => other.to_string(),
    }
}
