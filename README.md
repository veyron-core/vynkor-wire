# veyron-wire

Shared wire-protocol crate for [Veyron](https://github.com/veyron-core/veyron):
frame framing, frame authentication, socket-path defaults, and the generated
Protobuf types. This is the protocol surface the Veyron kernel and its SDKs
both build on, so a plugin author can depend on `veyron-wire` alone instead
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
  `/run/user/<uid>` → `~/.veyron/run` fallback order.
- `proto::veyron` — protobuf types generated from `proto/veyron_protocol.proto`
  at build time via `prost-build`.
- `WireError` — the protocol-level error type returned by `framing` and
  `mac` functions.

## Who uses this

- The `veyron` kernel (path dependency, re-exports/wraps this crate's API
  so kernel call sites see no behavior change).
- `veyron-sdk` (Rust), via the published crates.io version.

C++ and Python SDKs can't depend on a Cargo crate directly — they vendor a
copy of `proto/veyron_protocol.proto` instead; see
[`veyron-sdk-cpp`](https://github.com/veyron-core/veyron-sdk-cpp) and
[`veyron-sdk-python`](https://github.com/veyron-core/veyron-sdk-python).

## Protocol versioning

The wire protocol version lives in two places that must stay in sync:

- the `// v 1.x` header comment in `proto/veyron_protocol.proto`, and
- `veyron_wire::PROTOCOL_VERSION` (a `&str` exported from `src/lib.rs`).

**v1.4** adds five `PermissionType` values — 15 through 19:
`PERMISSION_SECRETS`, `PERMISSION_CLIPBOARD`, `PERMISSION_LAUNCH`,
`PERMISSION_SCREEN`, `PERMISSION_HOME` — covering the planned
`secrets`/`clipboard`/`launcher`/`screenshot`/`home` plugins. The values
stay contiguous because the kernel's `known_permissions()` probe
(`veyron/src/marketplace/installer.rs`) walks enum codes and stops after 4
consecutive misses — a gap ≥4 silently rejects installs of any plugin
declaring a later value.

Crate version tracks the wire protocol. Additive changes (new enum values,
new optional fields) bump the **patch** (`0.2.0 → 0.2.1`); breaking wire
changes bump the **minor** (`0.2.x → 0.3.0`). Both the proto header and
`PROTOCOL_VERSION` are bumped in the same commit as `Cargo.toml`.

### Releasing a new version

1. Bump `version` in `Cargo.toml`, `PROTOCOL_VERSION` in `src/lib.rs`, and
   the `// v 1.x` header in `proto/veyron_protocol.proto` — one commit.
2. Re-sync the vendored proto copies in `veyron-sdk-python/proto/` and
   `veyron-sdk-cpp/proto/` (byte-identical), and regenerate the Python
   binding via the kernel's `scripts/gen_proto_python.py`. The kernel's
   `tests/unit/test_proto_sync.rs` (R8-05) guards all three copies and the
   `pb2` marker check.
3. `cargo publish` from this directory (requires a crates.io token in
   `~/.cargo/credentials.toml`).
4. Update dependents: `veyron-sdk` (Rust) bumps its `veyron-wire`
   requirement, and the `veyron` kernel drops its `[patch.crates-io]`
   override once the new version is on crates.io.

## MSRV

Rust 1.85.0.

## License

MIT
