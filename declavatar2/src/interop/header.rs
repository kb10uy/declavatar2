use std::fmt::{self, Display};

use super::wire::{DecodeError, EncodeError};

pub const HEADER_LEN: usize = 16;
pub const SCHEMA_VERSION: u16 = 1;
pub const AVATAR_MAGIC: [u8; 4] = *b"DA2a";
pub const AVATAR_DATA_VERSION: u16 = 1;
pub const DIAGNOSTICS_MAGIC: [u8; 4] = *b"DA2d";
pub const DIAGNOSTICS_DATA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlobKind {
    Avatar,
    Diagnostics,
}

impl BlobKind {
    pub fn magic(self) -> [u8; 4] {
        match self {
            BlobKind::Avatar => AVATAR_MAGIC,
            BlobKind::Diagnostics => DIAGNOSTICS_MAGIC,
        }
    }

    pub fn data_version(self) -> u16 {
        match self {
            BlobKind::Avatar => AVATAR_DATA_VERSION,
            BlobKind::Diagnostics => DIAGNOSTICS_DATA_VERSION,
        }
    }

    pub fn of_magic(magic: [u8; 4]) -> Option<Self> {
        match magic {
            AVATAR_MAGIC => Some(BlobKind::Avatar),
            DIAGNOSTICS_MAGIC => Some(BlobKind::Diagnostics),
            _ => None,
        }
    }
}

impl Display for BlobKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            BlobKind::Avatar => "avatar",
            BlobKind::Diagnostics => "diagnostics",
        })
    }
}

pub fn write_blob(kind: BlobKind, payload: &[u8]) -> Result<Vec<u8>, EncodeError> {
    let payload_len = u32::try_from(payload.len()).map_err(|_| EncodeError::Overflow {
        what: "payload length",
        value: payload.len(),
    })?;
    let mut bytes = Vec::with_capacity(HEADER_LEN + payload.len());
    bytes.extend_from_slice(&kind.magic());
    bytes.extend_from_slice(&SCHEMA_VERSION.to_le_bytes());
    bytes.extend_from_slice(&kind.data_version().to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&payload_len.to_le_bytes());
    bytes.extend_from_slice(payload);
    Ok(bytes)
}

pub fn read_blob(expected: BlobKind, bytes: &[u8]) -> Result<&[u8], DecodeError> {
    let Some(header) = bytes.get(..HEADER_LEN) else {
        return Err(DecodeError::TruncatedHeader { found: bytes.len() });
    };
    let magic: [u8; 4] = header[0..4].try_into().expect("the header slice has four magic bytes");
    let found = BlobKind::of_magic(magic).ok_or(DecodeError::UnknownMagic(magic))?;
    if found != expected {
        return Err(DecodeError::UnexpectedKind { expected, found });
    }
    let schema_version = u16::from_le_bytes([header[4], header[5]]);
    if schema_version != SCHEMA_VERSION {
        return Err(DecodeError::SchemaVersionMismatch {
            expected: SCHEMA_VERSION,
            found: schema_version,
        });
    }
    let data_version = u16::from_le_bytes([header[6], header[7]]);
    if data_version != found.data_version() {
        return Err(DecodeError::DataVersionMismatch {
            kind: found,
            expected: found.data_version(),
            found: data_version,
        });
    }
    let reserved = u32::from_le_bytes([header[8], header[9], header[10], header[11]]);
    if reserved != 0 {
        return Err(DecodeError::ReservedNotZero(reserved));
    }
    let payload_len = u32::from_le_bytes([header[12], header[13], header[14], header[15]]);
    let payload = &bytes[HEADER_LEN..];
    if usize::try_from(payload_len).ok() != Some(payload.len()) {
        return Err(DecodeError::LengthMismatch {
            declared: payload_len,
            actual: payload.len(),
        });
    }
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use rstest::*;

    use super::*;

    #[rstest]
    fn a_written_header_has_the_documented_layout() {
        let bytes = write_blob(BlobKind::Avatar, &[0xaa, 0xbb, 0xcc]).unwrap();
        assert_eq!(bytes, [b'D', b'A', b'2', b'a', 1, 0, 1, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0xaa, 0xbb, 0xcc]);
        assert_eq!(read_blob(BlobKind::Avatar, &bytes).unwrap(), [0xaa, 0xbb, 0xcc]);

        let bytes = write_blob(BlobKind::Diagnostics, &[]).unwrap();
        assert_eq!(bytes, [b'D', b'A', b'2', b'd', 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(read_blob(BlobKind::Diagnostics, &bytes).unwrap(), []);
    }

    fn header(magic: &[u8; 4], schema: u16, data: u16, reserved: u32, payload_len: u32) -> Vec<u8> {
        let mut bytes = magic.to_vec();
        bytes.extend_from_slice(&schema.to_le_bytes());
        bytes.extend_from_slice(&data.to_le_bytes());
        bytes.extend_from_slice(&reserved.to_le_bytes());
        bytes.extend_from_slice(&payload_len.to_le_bytes());
        bytes
    }

    #[rstest]
    #[case::truncated(header(b"DA2a", 1, 1, 0, 0)[..15].to_vec(), DecodeError::TruncatedHeader { found: 15 })]
    #[case::empty(vec![], DecodeError::TruncatedHeader { found: 0 })]
    #[case::unknown_magic(header(b"DA2x", 1, 1, 0, 0), DecodeError::UnknownMagic(*b"DA2x"))]
    #[case::other_kind(header(b"DA2d", 1, 1, 0, 0), DecodeError::UnexpectedKind { expected: BlobKind::Avatar, found: BlobKind::Diagnostics })]
    #[case::schema(header(b"DA2a", 2, 1, 0, 0), DecodeError::SchemaVersionMismatch { expected: 1, found: 2 })]
    #[case::data(header(b"DA2a", 1, 9, 0, 0), DecodeError::DataVersionMismatch { kind: BlobKind::Avatar, expected: 1, found: 9 })]
    #[case::reserved(header(b"DA2a", 1, 1, 0x100, 0), DecodeError::ReservedNotZero(0x100))]
    #[case::short_payload(header(b"DA2a", 1, 1, 0, 2), DecodeError::LengthMismatch { declared: 2, actual: 0 })]
    #[case::long_payload([header(b"DA2a", 1, 1, 0, 0), vec![0]].concat(), DecodeError::LengthMismatch { declared: 0, actual: 1 })]
    #[case::huge_payload(header(b"DA2a", 1, 1, 0, u32::MAX), DecodeError::LengthMismatch { declared: u32::MAX, actual: 0 })]
    fn header_validation_reports_the_first_failure(#[case] bytes: Vec<u8>, #[case] expected: DecodeError) {
        assert_eq!(read_blob(BlobKind::Avatar, &bytes).unwrap_err(), expected);
    }

    #[rstest]
    fn kind_checks_precede_version_checks() {
        let bytes = header(b"DA2d", 2, 9, 1, 0);
        assert_eq!(
            read_blob(BlobKind::Avatar, &bytes).unwrap_err(),
            DecodeError::UnexpectedKind {
                expected: BlobKind::Avatar,
                found: BlobKind::Diagnostics
            }
        );
        assert_eq!(
            read_blob(BlobKind::Diagnostics, &bytes).unwrap_err(),
            DecodeError::SchemaVersionMismatch { expected: 1, found: 2 }
        );
    }
}
