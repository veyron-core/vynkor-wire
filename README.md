# vynkor-wire

Shared wire-protocol crate for [Vynkor](https://github.com/vynkor-core/vynkor):
frame framing, frame authentication, socket-path defaults, and the generated
Protobuf types. This is the protocol surface the Vynkor kernel and its SDKs
both build on, so a plugin author can depend on `vynkor-wire` alone instead
of pulling in the whole kernel crate.

## What's inside

- `framing` — frame read/write over an async stream (`write_frame`,
  `write_frame_raw`, `read_frame`, `read_frame_with_timeout`), fragmentation
  (`FragmentHeader`, `parse_frag_header`), and the wire constants
  (`MAX_PAYLOAD_SIZE`, `COMPRESS_THRESHOLD`, `FLAG_MAC_PRESENT`,
  `FLAG_COMPRESSED`, `FLAG_RAW_BINARY`, `FLAG_FRAGMENTED`).
- `mac` — HMAC-SHA256 frame authentication (`derive_session_key`,
  `compute_tag`, `verify_tag`).
- `socket` — default Unix socket path resolution (`default_socket_path`,
  `default_private_dir`), matching the kernel's `$XDG_RUNTIME_DIR` →
  `/run/user/<uid>` → `~/.local/state/vyn/run` fallback order.
- `proto::vynkor` — protobuf types generated from `proto/vynkor_protocol.proto`
  at build time via `prost-build`.
- `manifest` *(opt-in)* — plugin-manifest parsing/validation (`InstallManifest`,
  `validate_manifest`, `check_kernel_compatibility`), the single implementation
  shared by the kernel and vynm. Enable with `features = ["manifest"]`; off by
  default so SDK consumers compile neither serde nor serde_json nor semver.
- `WireError` — the protocol-level error type returned by `framing` and
  `mac` functions.

## Who uses this

- The `vynkor` kernel (path dependency, re-exports/wraps this crate's API
  so kernel call sites see no behavior change).
- `vynkor-sdk` (Rust), via the published crates.io version.

C++ and Python SDKs can't depend on a Cargo crate directly — they vendor a
copy of `proto/vynkor_protocol.proto` instead; see
[`vynkor-sdk-cpp`](https://github.com/vynkor-core/vynkor-sdk-cpp) and
[`vynkor-sdk-python`](https://github.com/vynkor-core/vynkor-sdk-python).

## Protocol versioning

The wire protocol version lives in two places that must stay in sync:

- the `// v 1.x` header comment in `proto/vynkor_protocol.proto`, and
- `vynkor_wire::PROTOCOL_VERSION` (a `&str` exported from `src/lib.rs`).

**v1.6** (D-01, 2026-08-14) adds device identity + versioning + `user_id` +
the AI tool schema, one additive bump: `PluginRegister` gains `device_id`,
`os` (`DeviceOs`), `arch`, `os_version`, `capabilities[]`, `protocol_version`
(semver), `user_id`; `PluginManifest` gains `platforms[]` and `action_specs[]`
(`ActionSpec { name, description, params_schema, risk, requires_confirmation }`
+ `ActionRisk`); new `DeviceInfo { device_id, os, arch, os_version,
capabilities, last_seen, state }` + `DeviceState` for registry/discovery.
Shipped as **0.2.3** (patch — additive per the rule above), **published to
crates.io 2026-08-14**. The kernel continues to consume published 0.2.2 until
D-03 wires the fields up (then bumps the requirement to 0.2.3 — no
`[patch.crates-io]` override needed); vendored SDK copies re-synced
byte-identical.

**v1.4** adds five `PermissionType` values — 15 through 19:
`PERMISSION_SECRETS`, `PERMISSION_CLIPBOARD`, `PERMISSION_LAUNCH`,
`PERMISSION_SCREEN`, `PERMISSION_HOME` — covering the planned
`secrets`/`clipboard`/`launcher`/`screenshot`/`home` plugins. The values
stay contiguous because the kernel's `known_permissions()` probe
(`vynkor/src/marketplace/installer.rs`) walks enum codes and stops after 4
consecutive misses — a gap ≥4 silently rejects installs of any plugin
declaring a later value.

**0.2.4** (V-01, 2026-08-21) ships the opt-in `manifest` module — no proto
change, header and `PROTOCOL_VERSION` untouched, **published to crates.io
2026-08-21**. The kernel keeps consuming 0.2.3 until V-02 switches
`loader.rs` to `vynkor_wire::manifest` (then bumps the requirement to 0.2.4 —
no `[patch.crates-io]` override needed).

Crate version tracks the wire protocol. Additive changes (new enum values,
new optional fields) bump the **patch** (`0.2.0 → 0.2.1`); breaking wire
changes bump the **minor** (`0.2.x → 0.3.0`). Both the proto header and
`PROTOCOL_VERSION` are bumped in the same commit as `Cargo.toml`.

### Releasing a new version

1. Bump `version` in `Cargo.toml`, `PROTOCOL_VERSION` in `src/lib.rs`, and
   the `// v 1.x` header in `proto/vynkor_protocol.proto` — one commit.
2. Re-sync the vendored proto copies in `vynkor-sdk-python/proto/` and
   `vynkor-sdk-cpp/proto/` (byte-identical), and regenerate the Python
   binding via the kernel's `scripts/gen_proto_python.py`. The kernel's
   `tests/unit/test_proto_sync.rs` (R8-05) guards all three copies and the
   `pb2` marker check.
3. `cargo publish` from this directory (requires a crates.io token in
   `~/.cargo/credentials.toml`).
4. Update dependents: `vynkor-sdk` (Rust) bumps its `vynkor-wire`
   requirement, and the `vynkor` kernel drops its `[patch.crates-io]`
   override once the new version is on crates.io.

## MSRV

Rust 1.85.0.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
