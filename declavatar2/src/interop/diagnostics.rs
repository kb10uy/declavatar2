use super::wire::{Decode, DecodeError, Encode, EncodeError, Reader, Writer};
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

impl Encode for Diagnostics {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        self.stage.encode(writer)?;
        self.items.encode(writer)
    }
}

impl Decode for Diagnostics {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            stage: reader.decode()?,
            items: reader.decode()?,
        })
    }
}

impl Encode for DiagnosticStage {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        writer.u8(match self {
            DiagnosticStage::Script => 0,
            DiagnosticStage::Transform => 1,
        });
        Ok(())
    }
}

impl Decode for DiagnosticStage {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        match reader.u8()? {
            0 => Ok(DiagnosticStage::Script),
            1 => Ok(DiagnosticStage::Transform),
            value => Err(DecodeError::InvalidDiscriminator {
                type_name: "DiagnosticStage",
                value,
            }),
        }
    }
}

impl Encode for Diagnostic {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        self.at.encode(writer)?;
        self.message.encode(writer)
    }
}

impl Decode for Diagnostic {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            at: reader.decode()?,
            message: reader.decode()?,
        })
    }
}
