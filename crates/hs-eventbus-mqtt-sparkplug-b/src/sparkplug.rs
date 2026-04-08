//! Sparkplug B protobuf types generated from proto/sparkplug_b.proto via prost-build.
//! Source: https://github.com/Cirrus-Link2/Sparkplug/blob/main/sparkplug_b/sparkplug_b.proto

include!(concat!(
    env!("OUT_DIR"),
    "/com.cirruslink.sparkplug.protobuf.rs"
));

/// Sparkplug B DataType numeric identifiers per official specification.
#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum DataType {
    Int8 = 1,
    Int16 = 2,
    Int32 = 3,
    Int64 = 4,
    UInt8 = 5,
    UInt16 = 6,
    UInt32 = 7,
    UInt64 = 8,
    Float = 9,
    Double = 10,
    Boolean = 11,
    String = 12,
    DateTime = 13,
    Text = 14,
    Uuid = 15,
    DataSet = 16,
    Bytes = 17,
    File = 18,
    Template = 19,
}

impl DataType {
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            1 => Some(Self::Int8),
            2 => Some(Self::Int16),
            3 => Some(Self::Int32),
            4 => Some(Self::Int64),
            5 => Some(Self::UInt8),
            6 => Some(Self::UInt16),
            7 => Some(Self::UInt32),
            8 => Some(Self::UInt64),
            9 => Some(Self::Float),
            10 => Some(Self::Double),
            11 => Some(Self::Boolean),
            12 => Some(Self::String),
            13 => Some(Self::DateTime),
            14 => Some(Self::Text),
            15 => Some(Self::Uuid),
            16 => Some(Self::DataSet),
            17 => Some(Self::Bytes),
            18 => Some(Self::File),
            19 => Some(Self::Template),
            _ => None,
        }
    }
}
