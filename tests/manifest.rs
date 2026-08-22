// Manifest-module tests, ported from the kernel's tests/unit/test_installer.rs
// (V-01: the manifest implementation moved here, so its tests move too).
// Whole file inert without the feature — plain `cargo test` stays green.
#![cfg(feature = "manifest")]

use std::fs;

use semver::Version;
use tempfile::tempdir;
use veyron_wire::manifest::{
    check_kernel_compatibility, default_resolver, validate_manifest, ActionSpec,
};
use veyron_wire::proto::veyron::PermissionType;

// prost 0.13+ derives inherent as_str_name/try_from (no Enumeration::values);
// probe codes, stopping after a run of misses so a reserved gap (7) is fine.
fn all_permission_types() -> Vec<PermissionType> {
    let mut out = Vec::new();
    let mut misses = 0;
    for i in 0i32.. {
        match PermissionType::try_from(i) {
            Ok(pt) => {
                out.push(pt);
                misses = 0;
            }
            Err(_) => {
                misses += 1;
                if misses >= 4 {
                    break;
                }
            }
        }
    }
    out
}

fn write_manifest(dir: &std::path::Path, json: &str) {
    fs::write(dir.join("plugin.json"), json).unwrap();
}

// kernel below min → error
#[test]
fn compat_below_min_rejected() {
    let kernel = Version::parse("0.2.0").unwrap();
    let err = check_kernel_compatibility("stt-whisper", "0.3.0", "1.0.0", &kernel).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("requires Veyron kernel >= 0.3.0") && msg.contains("you are running 0.2.0"),
        "unexpected: {msg}"
    );
}

// kernel above max → error
#[test]
fn compat_above_max_rejected() {
    let kernel = Version::parse("2.0.0").unwrap();
    let err = check_kernel_compatibility("stt-whisper", "0.3.0", "1.0.0", &kernel).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("requires Veyron kernel <= 1.0.0") && msg.contains("you are running 2.0.0"),
        "unexpected: {msg}"
    );
}

// "*" max = unbounded
#[test]
fn compat_wildcard_max_accepts_any() {
    let kernel = Version::parse("99.0.0").unwrap();
    assert!(check_kernel_compatibility("foo", "0.1.0", "*", &kernel).is_ok());
}

// unparseable bounds fail loud, never pass silently
#[test]
fn compat_invalid_versions_rejected() {
    let kernel = Version::parse("0.1.0").unwrap();
    assert!(check_kernel_compatibility("foo", "not-a-version", "*", &kernel).is_err());
    assert!(check_kernel_compatibility("foo", "0.1.0", "also-bad", &kernel).is_err());
}

#[test]
fn manifest_missing_compat_range_errors() {
    let tmp = tempdir().unwrap();
    write_manifest(
        tmp.path(),
        r#"{"plugin_id":"foo","version":"1.0.0","permissions":[],"binary":"foo"}"#,
    );
    let kernel = Version::parse("0.1.0").unwrap();
    let err =
        validate_manifest(&tmp.path().join("plugin.json"), &kernel, default_resolver).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("Invalid plugin.json"),
        "expected parse error, got: {msg}"
    );
}

// unknown permission → validation error
#[test]
fn manifest_unknown_permission_errors() {
    let tmp = tempdir().unwrap();
    write_manifest(
        tmp.path(),
        r#"{
            "plugin_id": "foo",
            "version": "1.0.0",
            "permissions": ["teleport"],
            "binary": "foo",
            "kernel_compatibility_range": {"min": "0.1.0", "max": "*"}
        }"#,
    );
    let kernel = Version::parse("0.1.0").unwrap();
    let err =
        validate_manifest(&tmp.path().join("plugin.json"), &kernel, default_resolver).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("unknown permission") && msg.contains("teleport"),
        "unexpected: {msg}"
    );
}

// R8-02 drift guard: every proto permission (both name forms) must pass
// validate_manifest — a future enum value that breaks here means the probe
// logic or resolver drifted from the proto.
#[test]
fn manifest_accepts_every_proto_permission() {
    let kernel = Version::parse("0.1.0").unwrap();
    for pt in all_permission_types() {
        let proto_name = pt.as_str_name();
        if proto_name == "PERMISSION_UNKNOWN" {
            continue;
        }
        let lower = proto_name
            .strip_prefix("PERMISSION_")
            .unwrap_or(proto_name)
            .to_ascii_lowercase();
        for perm in [proto_name, lower.as_str()] {
            let tmp = tempdir().unwrap();
            write_manifest(
                tmp.path(),
                &format!(
                    r#"{{
                        "plugin_id": "perm-check",
                        "version": "1.0.0",
                        "permissions": ["{perm}"],
                        "binary": "perm-check",
                        "kernel_compatibility_range": {{"min": "0.1.0", "max": "*"}}
                    }}"#
                ),
            );
            let res = validate_manifest(&tmp.path().join("plugin.json"), &kernel, default_resolver);
            assert!(
                res.is_ok(),
                "permission {perm} (proto {proto_name}) should be accepted, got: {res:?}"
            );
        }
    }
}

// kernel incompatible in plugin.json → validation error
#[test]
fn manifest_kernel_compat_enforced() {
    let tmp = tempdir().unwrap();
    write_manifest(
        tmp.path(),
        r#"{
            "plugin_id": "foo",
            "version": "1.0.0",
            "permissions": [],
            "binary": "foo",
            "kernel_compatibility_range": {"min": "99.0.0", "max": "*"}
        }"#,
    );
    let kernel = Version::parse("0.1.0").unwrap();
    let err =
        validate_manifest(&tmp.path().join("plugin.json"), &kernel, default_resolver).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("requires Veyron kernel >= 99.0.0"),
        "unexpected: {msg}"
    );
}

#[test]
fn manifest_valid_ok() {
    let tmp = tempdir().unwrap();
    write_manifest(
        tmp.path(),
        r#"{
            "plugin_id": "stt-whisper",
            "version": "1.2.0",
            "permissions": ["audio_stream", "network"],
            "binary": "stt-whisper",
            "kernel_compatibility_range": {"min": "0.1.0", "max": "*"},
            "events": ["system.ready"],
            "actions": ["transcribe_audio"]
        }"#,
    );
    let kernel = Version::parse("0.1.0").unwrap();
    let manifest =
        validate_manifest(&tmp.path().join("plugin.json"), &kernel, default_resolver).unwrap();
    assert_eq!(manifest.plugin_id, "stt-whisper");
    assert_eq!(manifest.version, "1.2.0");
}

#[test]
fn manifest_not_found_errors() {
    let tmp = tempdir().unwrap();
    let kernel = Version::parse("0.1.0").unwrap();
    let err =
        validate_manifest(&tmp.path().join("plugin.json"), &kernel, default_resolver).unwrap_err();
    assert!(err.to_string().contains("Invalid plugin.json"));
}

// legacy string-form actions still parse.
#[test]
fn manifest_legacy_actions_parse() {
    let tmp = tempdir().unwrap();
    write_manifest(
        tmp.path(),
        r#"{
            "plugin_id": "legacy",
            "version": "1.0.0",
            "permissions": [],
            "binary": "legacy",
            "kernel_compatibility_range": {"min": "0.1.0", "max": "*"},
            "actions": ["transcribe_audio", "get_weather"]
        }"#,
    );
    let kernel = Version::parse("0.1.0").unwrap();
    let manifest =
        validate_manifest(&tmp.path().join("plugin.json"), &kernel, default_resolver).unwrap();
    let actions = manifest.actions.unwrap();
    assert_eq!(actions.len(), 2);
    for spec in &actions {
        assert!(matches!(spec, ActionSpec::Legacy(_)));
        assert!(spec.permission().is_none());
    }
    assert_eq!(actions[0].name(), "transcribe_audio");
    assert_eq!(actions[1].name(), "get_weather");
}

// object-form actions parse, with and without optional fields.
#[test]
fn manifest_v2_actions_parse() {
    let tmp = tempdir().unwrap();
    write_manifest(
        tmp.path(),
        r#"{
            "plugin_id": "db",
            "version": "1.0.0",
            "permissions": ["storage"],
            "binary": "db",
            "kernel_compatibility_range": {"min": "0.1.0", "max": "*"},
            "actions": [
                {"name": "db_get", "permission": "storage", "input": {"type": "object"}, "output": {"type": "object"}},
                {"name": "db_health"}
            ]
        }"#,
    );
    let kernel = Version::parse("0.1.0").unwrap();
    let manifest =
        validate_manifest(&tmp.path().join("plugin.json"), &kernel, default_resolver).unwrap();
    let actions = manifest.actions.unwrap();
    assert_eq!(actions.len(), 2);
    assert!(matches!(&actions[0], ActionSpec::V2(_)));
    assert_eq!(actions[0].name(), "db_get");
    assert_eq!(actions[0].permission(), Some("storage"));
    assert!(matches!(&actions[1], ActionSpec::V2(_)));
    assert_eq!(actions[1].name(), "db_health");
    assert_eq!(actions[1].permission(), None);
}

// unknown per-action permission refuses the plugin (fail-closed).
#[test]
fn manifest_unknown_action_permission_errors() {
    let tmp = tempdir().unwrap();
    write_manifest(
        tmp.path(),
        r#"{
            "plugin_id": "bad-action",
            "version": "1.0.0",
            "permissions": [],
            "binary": "bad-action",
            "kernel_compatibility_range": {"min": "0.1.0", "max": "*"},
            "actions": [{"name": "db_get", "permission": "teleport"}]
        }"#,
    );
    let kernel = Version::parse("0.1.0").unwrap();
    let err =
        validate_manifest(&tmp.path().join("plugin.json"), &kernel, default_resolver).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("unknown action permission")
            && msg.contains("teleport")
            && msg.contains("db_get"),
        "unexpected: {msg}"
    );
}

// per-action permission accepts both the lowercase and proto forms.
#[test]
fn manifest_action_permission_resolves_both_forms() {
    let kernel = Version::parse("0.1.0").unwrap();
    for perm in ["storage", "PERMISSION_STORAGE"] {
        let tmp = tempdir().unwrap();
        write_manifest(
            tmp.path(),
            &format!(
                r#"{{
                    "plugin_id": "db",
                    "version": "1.0.0",
                    "permissions": ["storage"],
                    "binary": "db",
                    "kernel_compatibility_range": {{"min": "0.1.0", "max": "*"}},
                    "actions": [{{"name": "db_get", "permission": "{perm}"}}]
                }}"#
            ),
        );
        assert!(
            validate_manifest(&tmp.path().join("plugin.json"), &kernel, default_resolver).is_ok(),
            "permission {perm} should be accepted"
        );
    }
}

// ── D3: optional sandbox hint ───────────────────────────────────────────────

#[test]
fn sandbox_hint_parses_all_three_states() {
    let kernel = Version::parse("0.1.0").unwrap();
    let base = |sandbox: &str| {
        format!(
            r#"{{
                "plugin_id": "db",
                "version": "1.0.0",
                "permissions": [],
                "binary": "db",
                "kernel_compatibility_range": {{"min": "0.1.0", "max": "*"}}{sandbox}
            }}"#
        )
    };
    for (json, expected) in [
        (base(""), None),
        (base(r#", "sandbox": true"#), Some(true)),
        (base(r#", "sandbox": false"#), Some(false)),
    ] {
        let tmp = tempdir().unwrap();
        write_manifest(tmp.path(), &json);
        let manifest =
            validate_manifest(&tmp.path().join("plugin.json"), &kernel, default_resolver).unwrap();
        assert_eq!(manifest.sandbox, expected);
    }
}

// ── D1 seam: the injected resolver ──────────────────────────────────────────
#[test]
fn default_resolver_accepts_lowercase_and_proto_forms() {
    assert_eq!(
        default_resolver("network"),
        Some(PermissionType::PermissionNetwork)
    );
    assert_eq!(
        default_resolver("PERMISSION_NETWORK"),
        Some(PermissionType::PermissionNetwork)
    );
    assert_eq!(
        default_resolver("storage"),
        Some(PermissionType::PermissionStorage)
    );
}

#[test]
fn default_resolver_rejects_garbage() {
    assert_eq!(default_resolver("teleport"), None);
    assert_eq!(default_resolver(""), None);
    assert_eq!(default_resolver("PERMISSION_UNKNOWN"), None);
}

// D1: policy is injectable — a stricter resolver can refuse permissions the
// default one accepts (F5 changes kernel policy without a wire change).
#[test]
fn validate_manifest_honors_injected_resolver() {
    fn deny_all(_s: &str) -> Option<PermissionType> {
        None
    }

    let manifest_json = r#"{
        "plugin_id": "db",
        "version": "1.0.0",
        "permissions": ["storage"],
        "binary": "db",
        "kernel_compatibility_range": {"min": "0.1.0", "max": "*"},
        "actions": [{"name": "db_get", "permission": "storage"}]
    }"#;

    let tmp = tempdir().unwrap();
    write_manifest(tmp.path(), manifest_json);
    let kernel = Version::parse("0.1.0").unwrap();

    let err = validate_manifest(&tmp.path().join("plugin.json"), &kernel, deny_all).unwrap_err();
    assert!(err.to_string().contains("unknown action permission"));

    // same manifest passes under the default policy
    assert!(validate_manifest(&tmp.path().join("plugin.json"), &kernel, default_resolver).is_ok());
}
