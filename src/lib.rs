// Wire protocol version. Mirrors the `// v 1.x` header comment in
// proto/veyron_protocol.proto — bump both together on any protocol change.
pub const PROTOCOL_VERSION: &str = "1.4";

pub mod error;
pub mod framing;
pub mod mac;
pub mod socket;
pub mod proto {
    #![allow(clippy::enum_variant_names)]
    pub mod veyron {
        include!(concat!(env!("OUT_DIR"), "/veyron.rs"));
    }
}

pub use error::WireError;
