// plugin-manifest parsing/validation, shared by the kernel and vynm so the
// two can't drift (V-01 / coupling point C1). Entire module behind the
// `manifest` feature — SDK consumers compile neither serde_json nor semver.
//
// Ported verbatim from the kernel marketplace (`src/marketplace/installer.rs`
// + `registry.rs::check_kernel_compatibility`) with two deliberate seams:
// - D1: the per-action permission policy is injected as a `fn(&str)`
//   resolver instead of calling kernel auth code; `default_resolver`
//   implements today's normalization so callers need no kernel.
// - check_kernel_compatibility takes a plain (slug, min, max) tuple — the
//   old RegistryEntry-shaped overload forced callers to build throwaway
//   fake entries; that hack did not move here.

use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::error::WireError;
use crate::proto::veyron::PermissionType;

// permission strings accepted in plugin.json: every proto enum variant in both
// the documented lowercase form (storage) and the PERMISSION_-prefixed proto
// name (PERMISSION_STORAGE), so a future proto permission can't silently break
// installs. UNKNOWN stays excluded. prost 0.13+ has no values(); probe codes,
// stopping after a run of misses so the reserved gap (7) is fine.
pub fn known_permissions() -> &'static [String] {
    static KNOWN: OnceLock<Vec<String>> = OnceLock::new();
    KNOWN.get_or_init(|| {
        let mut out = Vec::new();
        let mut misses = 0;
        for i in 0i32.. {
            match PermissionType::try_from(i) {
                Ok(pt) => {
                    let name = pt.as_str_name();
                    if name != "PERMISSION_UNKNOWN" {
                        out.push(name.to_string());
                        out.push(
                            name.strip_prefix("PERMISSION_")
                                .unwrap_or(name)
                                .to_ascii_lowercase(),
                        );
                    }
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
    })
}

#[derive(Debug, Deserialize)]
pub struct InstallManifest {
    pub plugin_id: String,
    pub version: String,
    pub permissions: Vec<String>,
    pub binary: String,
    pub kernel_compatibility_range: KernelCompatRange,
    pub events: Option<Vec<String>>,
    pub actions: Option<Vec<ActionSpec>>,
    /// Plugin IDs that must be loaded and registered before this plugin starts.
    #[serde(default)]
    pub requires: Vec<String>,
    /// Manifest v2 extraction allowlist: the archive entries to extract into
    /// the plugin dir. Empty = not declared (legacy extract-everything).
    #[serde(default)]
    pub files: Vec<String>,
}

/// A single entry in a v2 manifest's `actions` array. Legacy manifests declare
/// actions as plain strings; v2 manifests declare objects with a per-action
/// `permission` (the permission a *caller* must hold to invoke the action,
/// T-19 anti-laundering) plus optional JSON-Schema `input`/`output`. Both
/// forms are accepted — the untagged enum parses whichever is present.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ActionSpec {
    Legacy(String),
    V2(ActionSpecV2),
}

#[derive(Debug, Deserialize)]
pub struct ActionSpecV2 {
    pub name: String,
    #[serde(default)]
    pub permission: Option<String>,
    #[serde(default)]
    pub input: Option<serde_json::Value>,
    #[serde(default)]
    pub output: Option<serde_json::Value>,
}

impl ActionSpec {
    pub fn name(&self) -> &str {
        match self {
            ActionSpec::Legacy(s) => s,
            ActionSpec::V2(spec) => &spec.name,
        }
    }

    pub fn permission(&self) -> Option<&str> {
        match self {
            ActionSpec::Legacy(_) => None,
            ActionSpec::V2(spec) => spec.permission.as_deref(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct KernelCompatRange {
    pub min: String,
    pub max: String,
}

/// D1 default for the injected resolver: today's kernel normalization —
/// strip the `PERMISSION_` prefix if present, uppercase, `from_str_name`,
/// exclude UNKNOWN (value 0 is not a usable requirement). Kernel policy may
/// later diverge by passing its own resolver — no wire change needed.
pub fn default_resolver(s: &str) -> Option<PermissionType> {
    let name = if let Some(rest) = s.strip_prefix("PERMISSION_") {
        format!("PERMISSION_{}", rest.to_ascii_uppercase())
    } else {
        format!("PERMISSION_{}", s.to_ascii_uppercase())
    };
    // from_str_name happily resolves the literal "PERMISSION_UNKNOWN" to the
    // UNKNOWN variant (value 0) — treat it as unresolvable, matching
    // known_permissions() which excludes it.
    let pt = PermissionType::from_str_name(&name)?;
    if pt == PermissionType::PermissionUnknown {
        None
    } else {
        Some(pt)
    }
}

/// Check whether `running_kernel` falls within the stated compatibility
/// range. Takes loose strings so both manifest and registry-domain callers
/// fit without a shared entry type (`max == "*"` = unbounded).
pub fn check_kernel_compatibility(
    slug: &str,
    min_kernel_version: &str,
    max_kernel_version: &str,
    running_kernel: &semver::Version,
) -> Result<(), WireError> {
    let min = semver::Version::parse(min_kernel_version).map_err(|e| {
        WireError::Internal(format!(
            "Plugin '{slug}' has invalid min_kernel_version '{min_kernel_version}': {e}"
        ))
    })?;

    if *running_kernel < min {
        return Err(WireError::Internal(format!(
            "Plugin '{slug}' requires Veyron kernel >= {min}, you are running {running_kernel}"
        )));
    }

    if max_kernel_version != "*" {
        let max = semver::Version::parse(max_kernel_version).map_err(|e| {
            WireError::Internal(format!(
                "Plugin '{slug}' has invalid max_kernel_version '{max_kernel_version}': {e}"
            ))
        })?;

        if *running_kernel > max {
            return Err(WireError::Internal(format!(
                "Plugin '{slug}' requires Veyron kernel <= {max}, you are running {running_kernel}"
            )));
        }
    }

    Ok(())
}

/// Validate the `plugin.json` at `path`: parse, kernel compat range, known
/// permissions, and (v2) per-action permissions via the injected `resolver`.
/// Fail-loud: any problem refuses the whole manifest. Errors carry the same
/// message text the kernel produced pre-extraction, so downstream matching
/// on wording keeps working.
pub fn validate_manifest(
    path: &Path,
    kernel_ver: &semver::Version,
    resolver: fn(&str) -> Option<PermissionType>,
) -> Result<InstallManifest, WireError> {
    let data = fs::read_to_string(path).map_err(|_| {
        WireError::Internal("Invalid plugin.json: file not found or unreadable.".into())
    })?;

    let manifest: InstallManifest = serde_json::from_str(&data)
        .map_err(|e| WireError::Internal(format!("Invalid plugin.json: {e}")))?;

    check_kernel_compatibility(
        &manifest.plugin_id,
        &manifest.kernel_compatibility_range.min,
        &manifest.kernel_compatibility_range.max,
        kernel_ver,
    )?;

    let known = known_permissions();
    for perm in &manifest.permissions {
        if !known.contains(perm) {
            return Err(WireError::Internal(format!(
                "Plugin '{}' declares unknown permission '{perm}'.",
                manifest.plugin_id
            )));
        }
    }

    // Manifest v2: every per-action `permission` must resolve via the
    // injected resolver. Fail-closed — an unknown per-action permission
    // refuses the whole plugin, so a typo can't silently downgrade the
    // action to unrestricted.
    if let Some(actions) = &manifest.actions {
        for spec in actions {
            if let ActionSpec::V2(spec) = spec {
                if let Some(p) = &spec.permission {
                    if resolver(p).is_none() {
                        return Err(WireError::Internal(format!(
                            "Plugin '{}' declares unknown action permission '{}' for action '{}'.",
                            manifest.plugin_id, p, spec.name
                        )));
                    }
                }
            }
        }
    }

    Ok(manifest)
}
