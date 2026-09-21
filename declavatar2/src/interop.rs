mod avatar;
mod diagnostics;
mod header;
mod wire;

#[cfg(test)]
mod tests;

pub use diagnostics::{Diagnostic, DiagnosticStage, Diagnostics};
pub use header::{AVATAR_DATA_VERSION, AVATAR_MAGIC, BlobKind, DIAGNOSTICS_DATA_VERSION, DIAGNOSTICS_MAGIC, HEADER_LEN, SCHEMA_VERSION};
pub use wire::{Decode, DecodeError, Encode, EncodeError, Reader, Writer};

use crate::avatar::Avatar;

/// Encodes a compiled avatar into a complete `DA2a` blob, header included.
pub fn encode_avatar(avatar: &Avatar) -> Result<Vec<u8>, EncodeError> {
    encode_blob(BlobKind::Avatar, avatar)
}

/// Decodes a complete `DA2a` blob, validating the header and every byte of the payload.
pub fn decode_avatar(bytes: &[u8]) -> Result<Avatar, DecodeError> {
    decode_blob(BlobKind::Avatar, bytes)
}

/// Encodes diagnostics into a complete `DA2d` blob, header included.
pub fn encode_diagnostics(diagnostics: &Diagnostics) -> Result<Vec<u8>, EncodeError> {
    encode_blob(BlobKind::Diagnostics, diagnostics)
}

/// Decodes a complete `DA2d` blob, validating the header and every byte of the payload.
pub fn decode_diagnostics(bytes: &[u8]) -> Result<Diagnostics, DecodeError> {
    decode_blob(BlobKind::Diagnostics, bytes)
}

fn encode_blob<T: Encode>(kind: BlobKind, value: &T) -> Result<Vec<u8>, EncodeError> {
    let mut writer = Writer::new();
    value.encode(&mut writer)?;
    header::write_blob(kind, &writer.into_bytes())
}

fn decode_blob<T: Decode>(kind: BlobKind, bytes: &[u8]) -> Result<T, DecodeError> {
    let payload = header::read_blob(kind, bytes)?;
    let mut reader = Reader::new(payload);
    let value = T::decode(&mut reader)?;
    reader.finish()?;
    Ok(value)
}
