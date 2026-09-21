pub mod avatar;
pub mod core;
pub mod decl;
pub mod lua;
pub mod transform;
pub mod unity;
pub mod vrchat;

#[cfg(test)]
mod test_support;

use thiserror::Error;

pub use lua::EvaluateOptions;

/// Failure of turning a script into an avatar, at either stage of the pipeline.
#[derive(Debug, Error)]
pub enum CompileError {
    #[error(transparent)]
    Script(lua::ScriptError),

    #[error(transparent)]
    Transform(transform::TransformErrors),
}

/// Runs the whole pipeline: evaluates the script and transforms the declaration it returns.
pub fn compile(source: &str, chunk_name: &str, options: &EvaluateOptions) -> Result<avatar::Avatar, CompileError> {
    let declaration = lua::evaluate(source, chunk_name, options).map_err(CompileError::Script)?;
    transform::transform(&declaration).map_err(CompileError::Transform)
}
