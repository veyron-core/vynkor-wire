# V-01 implementation notes — wire manifest module

Task: **V-01** from `veyron/docs/VYNM_ROADMAP.md` (vynm marketplace extraction,
stage 1). Branch `feat/manifest-module`, base `main`. Implemented 2026-08-21.

## What changed

| File | Change |
|---|---|
| `src/manifest.rs` | NEW — plugin-manifest parsing/validation behind the `manifest` feature. Ported verbatim from the kernel: `InstallManifest`, `KernelCompatRange`, `ActionSpec`/`ActionSpecV2` (untagged V1-string/V2-object enum), `known_permissions()` probe, `check_kernel_compatibility()`, `validate_manifest()`. |
| `src/lib.rs` | `#[cfg(feature = "manifest")] pub mod manifest;` — nothing else touched. |
| `Cargo.toml` | version `0.2.3 → 0.2.4`; optional deps `serde`(derive)/`serde_json`/`semver`; `[features] manifest = [...]` off by default; dev-dep `tempfile`. |
| `tests/manifest.rs` | NEW — 17 tests: manifest cases + R8-02 drift test moved from kernel `tests/unit/test_installer.rs`, compat tests adapted to the new tuple signature, plus new tests for wildcard-max, invalid versions, and D1 resolver injection. |

## Design decisions (and where they differ from the roadmap text)

1. **Third optional dep: `serde` itself.** The roadmap listed only
   `serde_json` + `semver`. First `--features manifest` build failed with
   E0277/E0432 — this crate never depended on `serde` before (the kernel has
   its own), and `#[derive(Deserialize)]` needs it directly. Added as
   `serde = { version = "1", features = ["derive"], optional = true }` and
   wired into the feature.
2. **Error variant: reused `WireError::Internal(String)`** instead of adding a
   new one. The roadmap allowed either ("new variant or existing message
   variant"). A new variant would break the kernel's exhaustive
   `From<WireError> for VeyronError` match on day one of adoption; reusing
   `Internal` keeps the patch bump honestly additive and preserves today's
   fail-loud semantics. All message texts are byte-identical to the kernel's
   (`"Invalid plugin.json: ..."`, `"Plugin '{}' declares unknown permission ..."`),
   so downstream `contains()` assertions survive the extraction unchanged.
3. **`check_kernel_compatibility(slug, min_kernel_version, max_kernel_version,
   running_kernel)`** — tuple form chosen over an `&InstallManifest` overload:
   it serves both callers (manifest fields here, registry-entry fields in vynm
   later) without inventing a shared entry type. The throwaway-fake-
   `RegistryEntry` hack from the kernel's `validate_manifest` did not move.
4. **`known_permissions()` is `pub`** (private in the kernel) — vynm will need
   the same accepted-permission list; no reason to force a re-probe there.
5. **D1 seam:** `validate_manifest(path, kernel_ver, resolver:
   fn(&str) -> Option<PermissionType>)` + `default_resolver` implementing the
   kernel's `resolve_permission` normalization verbatim (strip `PERMISSION_`
   prefix → uppercase → `from_str_name` → reject UNKNOWN). Kernel policy can
   diverge later by injecting its own fn — no wire change (F5-ready).
6. **Tests live in `tests/manifest.rs`** with `#![cfg(feature = "manifest")]`
   at the top — plain `cargo test` compiles the file to nothing, so the
   no-feature gate stays green while `cargo test --features manifest` runs all
   17. `tempfile` is a dev-dependency (never compiled by SDK consumers).

## Gates (all green)

| Gate | Result |
|---|---|
| `cargo build` (no features) | ✓ — serde/semver not compiled (SDK weight guard) |
| `cargo test --features manifest` | ✓ 17 passed incl. R8-02 drift test |
| `cargo test` (no features) | ✓ (existing lib tests only) |
| `cargo clippy --all-targets -- -D warnings` (both configs) | ✓ clean |
| `cargo fmt --check` | ✓ (after one `cargo fmt` pass over the new tests) |
| `PROTOCOL_VERSION` / proto header | untouched — no proto change, per task |

## Gotchas worth remembering

- **prost 0.13 has no `values()`** — the enum probe walks integer codes and
  stops after 4 consecutive misses (reserved gap 7). Preserved exactly; the
  drift test locks it: every proto `PermissionType` must pass validation in
  both name forms.
- **`Cargo.lock` is gitignored** in this repo (library crate) — lockfile
  updates from adding optional deps stay local, nothing to commit.
- **This repo has no CI** (no `.github/`). The roadmap's "CI guard" was
  verified locally for this PR; a small workflow asserting the default-feature
  build + MSRV would make that permanent (follow-up suggestion).
- `cargo fmt` reformats long `validate_manifest(...)` call chains differently
  than the kernel's style — ran it once over the new test file, don't fight it.

## Follow-ups

1. ~~Publish `0.2.4` to crates.io~~ — **DONE 2026-08-21**: `cargo publish`
   (dry-run first), verified via `cargo search`. No proto re-sync needed:
   header and `PROTOCOL_VERSION` are untouched.
2. **V-02 (kernel lane)**: switch `src/plugins/loader.rs` import to
   `veyron_wire::manifest::{validate_manifest, InstallManifest}`, pass
   `crate::auth::permissions::resolve_permission` as the resolver, add
   `features = ["manifest"]` to the wire dep. Since 0.2.4 is already on
   crates.io, the planned `[patch.crates-io]` git override is NOT needed —
   just bump the version requirement.
