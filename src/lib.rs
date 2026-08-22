// Wire protocol version. Mirrors the `// v 1.x` header comment in
// proto/vynkor_protocol.proto — bump both together on any protocol change.
pub const PROTOCOL_VERSION: &str = "1.6";

pub mod error;
pub mod framing;
pub mod mac;
pub mod socket;

// opt-in plugin-manifest module (V-01): shared by kernel + vynm, off by
// default so SDK consumers compile neither serde_json nor semver.
#[cfg(feature = "manifest")]
pub mod manifest;
pub mod proto {
    #![allow(clippy::enum_variant_names)]
    pub mod vynkor {
        include!(concat!(env!("OUT_DIR"), "/vynkor.rs"));
    }
}

pub use error::WireError;

#[cfg(test)]
mod tests {
    use super::proto::vynkor::{
        ActionRisk, ActionStatus, CommandStatus, DeviceOs, DeviceState, PluginRegister,
    };

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

    // D-01 value lock: the new v1.6 enums keep *_UNSPECIFIED = 0 so proto3's
    // omitted-zero-field default reads back as "unspecified", never a real
    // value — same trap M9 closed for the status enums.
    #[test]
    fn v16_enums_have_unspecified_as_zero() {
        assert_eq!(DeviceOs::Unspecified as i32, 0);
        assert_eq!(DeviceOs::Linux as i32, 1);
        assert_eq!(DeviceState::Unspecified as i32, 0);
        assert_eq!(DeviceState::Online as i32, 1);
        assert_eq!(ActionRisk::Unknown as i32, 0);
        assert_eq!(ActionRisk::Low as i32, 1);
    }

    // D-01: the new PluginRegister identity fields default empty — a v1.5
    // plugin registering against a v1.6 kernel must parse cleanly (additive).
    #[test]
    fn v16_plugin_register_fields_default_empty() {
        let reg = PluginRegister::default();
        assert!(reg.device_id.is_empty());
        assert_eq!(reg.os, DeviceOs::Unspecified as i32);
        assert!(reg.capabilities.is_empty());
        assert!(reg.protocol_version.is_empty());
        assert!(reg.user_id.is_empty());
    }
}
