use std::collections::BTreeMap;

use thiserror::Error;

use super::header::{BlobKind, HEADER_LEN};
use crate::unity::value::AnimatedValueType;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EncodeError {
    #[error("{what} {value} does not fit in its wire type")]
    Overflow { what: &'static str, value: usize },

    #[error("{0} has no wire representation")]
    Unrepresentable(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DecodeError {
    #[error("the blob is {found} bytes long, shorter than the {HEADER_LEN}-byte header")]
    TruncatedHeader { found: usize },

    #[error("magic {0:?} does not belong to a declavatar blob")]
    UnknownMagic([u8; 4]),

    #[error("expected a {expected} blob but found a {found} blob")]
    UnexpectedKind { expected: BlobKind, found: BlobKind },

    #[error("schema version {found} does not match the supported version {expected}")]
    SchemaVersionMismatch { expected: u16, found: u16 },

    #[error("{kind} data version {found} does not match the supported version {expected}")]
    DataVersionMismatch { kind: BlobKind, expected: u16, found: u16 },

    #[error("the reserved header field is {0:#x}, not zero")]
    ReservedNotZero(u32),

    #[error("the header declares {declared} payload bytes but {actual} follow it")]
    LengthMismatch { declared: u32, actual: usize },

    #[error("{needed} bytes are needed at offset {offset}, but {remaining} remain")]
    UnexpectedEnd { offset: usize, needed: usize, remaining: usize },

    #[error("{0} bytes remain after the root value")]
    TrailingBytes(usize),

    #[error("{what} {value} does not fit in this platform's size type")]
    LengthOverflow { what: &'static str, value: u32 },

    #[error("{0:#x} is not a bool")]
    InvalidBool(u8),

    #[error("{0:#x} is not an option tag")]
    InvalidOptionTag(u8),

    #[error("{value} is not a discriminator of {type_name}")]
    InvalidDiscriminator { type_name: &'static str, value: u8 },

    #[error("a string at offset {offset} is not valid UTF-8")]
    InvalidUtf8 { offset: usize },

    #[error("{kind} index {index} is out of range for a table of {len} entries")]
    ExternOutOfRange { kind: &'static str, index: u32, len: usize },

    #[error("state index {index} is out of range for a layer of {len} states")]
    StateOutOfRange { index: u32, len: usize },

    #[error("parameter `{0}` is not in the animator parameter list")]
    UnknownParameter(String),

    #[error("parameter `{name}` is declared as both {first:?} and {second:?}")]
    ConflictingParameter {
        name: String,
        first: AnimatedValueType,
        second: AnimatedValueType,
    },

    #[error("{what} {key} appears more than once")]
    Duplicate { what: &'static str, key: String },
}

pub trait Encode {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError>;
}

pub trait Decode: Sized {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError>;
}

#[derive(Debug, Default)]
pub struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn i32(&mut self, value: i32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn i64(&mut self, value: i64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn f32(&mut self, value: f32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn f64(&mut self, value: f64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    pub fn bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    pub fn length(&mut self, what: &'static str, value: usize) -> Result<(), EncodeError> {
        let value = u32::try_from(value).map_err(|_| EncodeError::Overflow { what, value })?;
        self.u32(value);
        Ok(())
    }

    pub fn string(&mut self, value: &str) -> Result<(), EncodeError> {
        self.length("string length", value.len())?;
        self.bytes(value.as_bytes());
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Context {
    pub extern_lens: [usize; 3],
    pub parameters: BTreeMap<String, AnimatedValueType>,
    pub states: Option<usize>,
}

#[derive(Debug)]
pub struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
    context: Context,
}

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            position: 0,
            context: Context::default(),
        }
    }

    pub fn position(&self) -> usize {
        self.position
    }

    pub fn remaining(&self) -> usize {
        self.bytes.len() - self.position
    }

    pub fn finish(self) -> Result<(), DecodeError> {
        match self.remaining() {
            0 => Ok(()),
            remaining => Err(DecodeError::TrailingBytes(remaining)),
        }
    }

    pub(crate) fn context(&self) -> &Context {
        &self.context
    }

    pub(crate) fn context_mut(&mut self) -> &mut Context {
        &mut self.context
    }

    pub fn decode<T: Decode>(&mut self) -> Result<T, DecodeError> {
        T::decode(self)
    }

    fn take(&mut self, needed: usize) -> Result<&'a [u8], DecodeError> {
        let remaining = self.remaining();
        if needed > remaining {
            return Err(DecodeError::UnexpectedEnd {
                offset: self.position,
                needed,
                remaining,
            });
        }
        let bytes = &self.bytes[self.position..self.position + needed];
        self.position += needed;
        Ok(bytes)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        let mut array = [0; N];
        array.copy_from_slice(self.take(N)?);
        Ok(array)
    }

    pub fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take(1)?[0])
    }

    pub fn u16(&mut self) -> Result<u16, DecodeError> {
        self.array().map(u16::from_le_bytes)
    }

    pub fn u32(&mut self) -> Result<u32, DecodeError> {
        self.array().map(u32::from_le_bytes)
    }

    pub fn i32(&mut self) -> Result<i32, DecodeError> {
        self.array().map(i32::from_le_bytes)
    }

    pub fn i64(&mut self) -> Result<i64, DecodeError> {
        self.array().map(i64::from_le_bytes)
    }

    pub fn f32(&mut self) -> Result<f32, DecodeError> {
        self.array().map(f32::from_le_bytes)
    }

    pub fn f64(&mut self) -> Result<f64, DecodeError> {
        self.array().map(f64::from_le_bytes)
    }

    pub fn bool(&mut self) -> Result<bool, DecodeError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(DecodeError::InvalidBool(other)),
        }
    }

    pub fn length(&mut self, what: &'static str) -> Result<usize, DecodeError> {
        let value = self.u32()?;
        usize::try_from(value).map_err(|_| DecodeError::LengthOverflow { what, value })
    }

    pub fn string(&mut self) -> Result<String, DecodeError> {
        let length = self.length("string length")?;
        let offset = self.position;
        let bytes = self.take(length)?;
        str::from_utf8(bytes).map(str::to_owned).map_err(|_| DecodeError::InvalidUtf8 { offset })
    }
}

impl Encode for bool {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        writer.bool(*self);
        Ok(())
    }
}

impl Decode for bool {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        reader.bool()
    }
}

macro_rules! numeric_wire {
    ($($type:ident),*) => {
        $(
            impl Encode for $type {
                fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
                    writer.$type(*self);
                    Ok(())
                }
            }

            impl Decode for $type {
                fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
                    reader.$type()
                }
            }
        )*
    };
}

numeric_wire!(u32, i32, i64, f32, f64);

impl Encode for String {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        writer.string(self)
    }
}

impl Decode for String {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        reader.string()
    }
}

impl<T: Encode> Encode for Option<T> {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        match self {
            None => writer.u8(0),
            Some(value) => {
                writer.u8(1);
                value.encode(writer)?;
            }
        }
        Ok(())
    }
}

impl<T: Decode> Decode for Option<T> {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        match reader.u8()? {
            0 => Ok(None),
            1 => T::decode(reader).map(Some),
            other => Err(DecodeError::InvalidOptionTag(other)),
        }
    }
}

impl<T: Encode> Encode for Vec<T> {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        self.as_slice().encode(writer)
    }
}

impl<T: Encode> Encode for [T] {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        writer.length("list length", self.len())?;
        for item in self {
            item.encode(writer)?;
        }
        Ok(())
    }
}

impl<T: Decode> Decode for Vec<T> {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let count = reader.length("list length")?;
        let mut items = Vec::new();
        for _ in 0..count {
            items.push(T::decode(reader)?);
        }
        Ok(items)
    }
}

impl<V: Encode> Encode for BTreeMap<String, V> {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        writer.length("map length", self.len())?;
        for (key, value) in self {
            writer.string(key)?;
            value.encode(writer)?;
        }
        Ok(())
    }
}

impl<V: Decode> Decode for BTreeMap<String, V> {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let count = reader.length("map length")?;
        let mut map = BTreeMap::new();
        for _ in 0..count {
            let key = reader.string()?;
            let value = V::decode(reader)?;
            if map.insert(key.clone(), value).is_some() {
                return Err(DecodeError::Duplicate {
                    what: "map key",
                    key: format!("`{key}`"),
                });
            }
        }
        Ok(map)
    }
}

#[cfg(test)]
mod tests {
    use rstest::*;

    use super::*;

    fn decode_all<T: Decode>(bytes: &[u8]) -> Result<T, DecodeError> {
        let mut reader = Reader::new(bytes);
        let value = T::decode(&mut reader)?;
        reader.finish()?;
        Ok(value)
    }

    #[rstest]
    fn primitives_are_little_endian_and_fixed_width() {
        let mut writer = Writer::new();
        writer.u8(0x01);
        writer.u16(0x0302);
        writer.u32(0x07060504);
        writer.i32(-1);
        writer.i64(i64::MIN);
        writer.f32(1.0);
        writer.f64(-2.0);
        writer.bool(true);
        assert_eq!(
            writer.into_bytes(),
            [
                0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x80, 0x3f, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xc0, 0x01,
            ]
        );
    }

    #[rstest]
    fn strings_carry_a_byte_length_and_utf8() {
        let mut writer = Writer::new();
        writer.string("Hips").unwrap();
        writer.string("日本").unwrap();
        writer.string("").unwrap();
        let bytes = writer.into_bytes();
        assert_eq!(
            bytes,
            [4, 0, 0, 0, b'H', b'i', b'p', b's', 6, 0, 0, 0, 0xe6, 0x97, 0xa5, 0xe6, 0x9c, 0xac, 0, 0, 0, 0]
        );

        let mut reader = Reader::new(&bytes);
        assert_eq!(reader.string().unwrap(), "Hips");
        assert_eq!(reader.string().unwrap(), "日本");
        assert_eq!(reader.string().unwrap(), "");
        reader.finish().unwrap();
    }

    #[rstest]
    fn composite_values_round_trip() {
        let mut writer = Writer::new();
        Some(vec![1u32, 2, 3]).encode(&mut writer).unwrap();
        None::<String>.encode(&mut writer).unwrap();
        BTreeMap::from([("a".to_string(), 1i64), ("b".to_string(), -1)]).encode(&mut writer).unwrap();
        let bytes = writer.into_bytes();
        assert_eq!(&bytes[..17], [1, 3, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0]);
        assert_eq!(bytes[17], 0);

        let mut reader = Reader::new(&bytes);
        assert_eq!(reader.decode::<Option<Vec<u32>>>().unwrap(), Some(vec![1, 2, 3]));
        assert_eq!(reader.decode::<Option<String>>().unwrap(), None);
        assert_eq!(
            reader.decode::<BTreeMap<String, i64>>().unwrap(),
            BTreeMap::from([("a".to_string(), 1), ("b".to_string(), -1)])
        );
        reader.finish().unwrap();
    }

    #[rstest]
    #[case::bool(&[2], decode_all::<bool>, DecodeError::InvalidBool(2))]
    #[case::option_tag(&[2], decode_all::<Option<bool>>, DecodeError::InvalidOptionTag(2))]
    #[case::truncated_u32(&[1, 2, 3], decode_all::<u32>, DecodeError::UnexpectedEnd { offset: 0, needed: 4, remaining: 3 })]
    #[case::truncated_string(&[5, 0, 0, 0, b'a'], decode_all::<String>, DecodeError::UnexpectedEnd { offset: 4, needed: 5, remaining: 1 })]
    #[case::invalid_utf8(&[1, 0, 0, 0, 0xff], decode_all::<String>, DecodeError::InvalidUtf8 { offset: 4 })]
    #[case::trailing(&[1, 0], decode_all::<bool>, DecodeError::TrailingBytes(1))]
    #[case::short_list(&[2, 0, 0, 0, 1], decode_all::<Vec<bool>>, DecodeError::UnexpectedEnd { offset: 5, needed: 1, remaining: 0 })]
    #[case::duplicate_key(
        &[2, 0, 0, 0, 1, 0, 0, 0, b'a', 1, 1, 0, 0, 0, b'a', 0],
        decode_all::<BTreeMap<String, bool>>,
        DecodeError::Duplicate { what: "map key", key: "`a`".into() }
    )]
    fn malformed_primitives_are_rejected<T: Decode + std::fmt::Debug>(
        #[case] bytes: &[u8],
        #[case] decode: fn(&[u8]) -> Result<T, DecodeError>,
        #[case] expected: DecodeError,
    ) {
        assert_eq!(decode(bytes).unwrap_err(), expected);
    }

    #[rstest]
    fn oversized_lengths_are_encode_errors() {
        let mut writer = Writer::new();
        assert_eq!(
            writer.length("list length", u32::MAX as usize + 1).unwrap_err(),
            EncodeError::Overflow {
                what: "list length",
                value: u32::MAX as usize + 1
            }
        );
    }
}
