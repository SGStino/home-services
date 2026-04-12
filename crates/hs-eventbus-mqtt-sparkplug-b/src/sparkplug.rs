//! Sparkplug B protobuf types generated from proto/sparkplug_b.proto via prost-build.
//! Source: https://github.com/Cirrus-Link2/Sparkplug/blob/main/sparkplug_b/sparkplug_b.proto

include!(concat!(
    env!("OUT_DIR"),
    "/org.eclipse.tahu.protobuf.rs"
));

pub fn datatype_from_u32(value: u32) -> Option<DataType> {
    DataType::try_from(value as i32).ok()
}
