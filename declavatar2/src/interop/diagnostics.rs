use super::macros::{wire_enum, wire_struct};
use crate::{CompileError, core::resolution::SourceLocation};

/// Everything a failed compile reports to the client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostics {
    pub stage: DiagnosticStage,
    pub items: Vec<Diagnostic>,
}

/// The pipeline stage that failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticStage {
    Script,
    Transform,
}

/// One reported failure and where in the script it was written, if known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub at: Option<SourceLocation>,
    pub message: String,
}

impl From<&CompileError> for Diagnostics {
    fn from(error: &CompileError) -> Self {
        match error {
            CompileError::Script(error) => Diagnostics {
                stage: DiagnosticStage::Script,
                items: vec![Diagnostic {
                    at: None,
                    message: error.to_string(),
                }],
            },
            CompileError::Transform(errors) => Diagnostics {
                stage: DiagnosticStage::Transform,
                items: errors
                    .0
                    .iter()
                    .map(|error| Diagnostic {
                        at: error.at.clone(),
                        message: error.kind.to_string(),
                    })
                    .collect(),
            },
        }
    }
}

wire_struct! {
    Diagnostics { stage, items }
    Diagnostic { at, message }
}

wire_enum! {
    DiagnosticStage {
        0 Script,
        1 Transform,
    }
}
