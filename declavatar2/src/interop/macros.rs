macro_rules! wire_struct {
    ($($type:ident $(<$($generic:ty),+>)? { $($field:ident),+ $(,)? })+) => {
        $(
            impl $crate::interop::wire::Encode for $type $(<$($generic),+>)? {
                fn encode(&self, writer: &mut $crate::interop::wire::Writer) -> Result<(), $crate::interop::wire::EncodeError> {
                    $(self.$field.encode(writer)?;)+
                    Ok(())
                }
            }

            impl $crate::interop::wire::Decode for $type $(<$($generic),+>)? {
                fn decode(reader: &mut $crate::interop::wire::Reader<'_>) -> Result<Self, $crate::interop::wire::DecodeError> {
                    Ok(Self {
                        $($field: reader.decode()?,)+
                    })
                }
            }
        )+
    };
}

macro_rules! decode_field {
    ($reader:ident, $_field:ident) => {
        $reader.decode()?
    };
}

macro_rules! wire_enum {
    ($($type:ident $(<$($generic:ty),+>)? {
        $($tag:literal $variant:ident $({ $($field:ident),+ $(,)? })? $(( $($tuple:ident),+ $(,)? ))?),+ $(,)?
    })+) => {
        $(
            impl $crate::interop::wire::Encode for $type $(<$($generic),+>)? {
                fn encode(&self, writer: &mut $crate::interop::wire::Writer) -> Result<(), $crate::interop::wire::EncodeError> {
                    match self {
                        $(
                            Self::$variant $({ $($field),+ })? $(( $($tuple),+ ))? => {
                                writer.u8($tag);
                                $($($field.encode(writer)?;)+)?
                                $($($tuple.encode(writer)?;)+)?
                            }
                        )+
                    }
                    Ok(())
                }
            }

            impl $crate::interop::wire::Decode for $type $(<$($generic),+>)? {
                fn decode(reader: &mut $crate::interop::wire::Reader<'_>) -> Result<Self, $crate::interop::wire::DecodeError> {
                    match reader.u8()? {
                        $(
                            $tag => Ok(Self::$variant $({ $($field: reader.decode()?),+ })? $(( $($crate::interop::macros::decode_field!(reader, $tuple)),+ ))?),
                        )+
                        value => Err($crate::interop::wire::DecodeError::InvalidDiscriminator {
                            type_name: stringify!($type),
                            value,
                        }),
                    }
                }
            }
        )+
    };
}

pub(crate) use {decode_field, wire_enum, wire_struct};
