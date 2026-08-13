// Wire protocol version. Mirrors the `// v 1.x` header comment in
// proto/veyron_protocol.proto — bump both together on any protocol change.
pub const PROTOCOL_VERSION: &str = "1.5";

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

#[cfg(test)]
mod tests {
    use super::proto::veyron::{ActionStatus, CommandStatus};

    // M9 value lock: the renumber is a wire-breaking semantic only the
    // compiler can't catch — a missed set_status() must read back as
    // *_UNKNOWN (0), never OK.
    #[test]
    fn status_enums_have_unknown_as_zero() {
        assert_eq!(ActionStatus::ActionUnknown as i32, 0);
        assert_eq!(ActionStatus::ActionOk as i32, 1);
        assert_eq!(ActionStatus::ActionError as i32, 2);
        assert_eq!(CommandStatus::CommandUnknown as i32, 0);
        assert_eq!(CommandStatus::CommandOk as i32, 1);
        assert_eq!(CommandStatus::CommandError as i32, 2);
    }
}
