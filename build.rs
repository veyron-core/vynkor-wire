fn main() {
    let mut config = prost_build::Config::new();
    // Envelope::Payload::PluginRegister dwarfs the other variants (device
    // identity fields, D-01); boxing would reshape a published protocol type,
    // so silence the lint on the generated enum instead
    config.type_attribute(
        "vynkor.Envelope.payload",
        "#[allow(clippy::large_enum_variant)]",
    );
    config
        .compile_protos(&["proto/vynkor_protocol.proto"], &["proto/"])
        .unwrap_or_else(|e| panic!("proto codegen failed: {}", e));
}
