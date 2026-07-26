//! Trust-on-first-use (SPEC Part VI §3.2): a plugin loads only after its
//! artifact hash is approved, and the approval is persisted to
//! `Piperine.lock` keyed by that hash — a changed artifact re-prompts.

use std::io::IsTerminal;
use std::path::Path;

use piperine_project::lockfile::{EntryKind, LockEntry, PiperineLock};

use crate::error::{PluginError, PluginResult};
use crate::manifest::Manifest;

/// How trust decisions are made this run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustMode {
    /// Prompt on stdin when it is a terminal; reject otherwise.
    Interactive,
    /// Accept and record every plugin (tests, `--trust-all` CI).
    AcceptAll,
    /// Reject anything not already trusted (`--no-trust` CI).
    RejectUntrusted,
}

/// sha256 of an artifact's bytes, as `sha256:<hex>`.
pub fn artifact_hash(path: &Path) -> PluginResult<String> {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path).map_err(|e| PluginError::Other {
        plugin: path.display().to_string(),
        message: format!("reading artifact: {e}"),
    })?;
    let digest = Sha256::digest(&bytes);
    Ok(format!("sha256:{digest:x}"))
}

/// Ensure `manifest`'s artifact (already hashed) is trusted, prompting or
/// recording per `mode`. `source` is the human-readable origin shown in the
/// prompt and stored in the lockfile.
pub fn ensure_trusted(
    project_root: &Path,
    manifest: &Manifest,
    source: &str,
    content_hash: &str,
    mode: TrustMode,
) -> PluginResult<()> {
    let lock_path = project_root.join("Piperine.lock");
    let mut lock = PiperineLock::load(&lock_path)
        .map_err(|e| PluginError::Other { plugin: manifest.name.clone(), message: e.to_string() })?
        .unwrap_or_default();

    if let Some(entry) = lock.plugin_entry(&manifest.name) {
        return match entry.content_hash.as_deref() {
            Some(trusted) if trusted == content_hash => Ok(()),
            // Known plugin, different artifact bytes: a silent binary swap
            // is exactly what the hash exists to catch.
            Some(_) => Err(PluginError::HashMismatch { plugin: manifest.name.clone() }),
            None => Err(PluginError::Untrusted(manifest.name.clone())),
        };
    }

    if !decide(mode, manifest, source, content_hash) {
        return Err(PluginError::Untrusted(manifest.name.clone()));
    }
    pin(&mut lock, &lock_path, manifest, source, content_hash, None)
}

/// TOFU for a fetched release asset (plugin-interface v2, PLG-17/18,
/// design §4) — distinct from [`ensure_trusted`] in two ways:
///
/// - `verify` is checked **up front**: a mismatch is a hard fail with no
///   prompt; a match is trusted (and pinned) without a TOFU prompt.
/// - a **changed** asset (a pinned hash that differs from the fetched
///   one — a mutable release tag) re-prompts per `mode` instead of
///   hard-failing, re-pinning on accept.
///
/// The approved pin records `(release-url, triple, content-hash)` — the
/// triple lands in the entry's `abi` field.
pub fn ensure_release_trusted(
    project_root: &Path,
    manifest: &Manifest,
    release: &str,
    content_hash: &str,
    triple: &str,
    verify: Option<&str>,
    mode: TrustMode,
) -> PluginResult<()> {
    let lock_path = project_root.join("Piperine.lock");
    let mut lock = PiperineLock::load(&lock_path)
        .map_err(|e| PluginError::Other { plugin: manifest.name.clone(), message: e.to_string() })?
        .unwrap_or_default();

    if let Some(expected) = verify {
        if expected != content_hash {
            return Err(PluginError::VerifyMismatch {
                plugin: manifest.name.clone(),
                release: release.to_string(),
            });
        }
        // `verify` IS the explicit consent — pin without prompting.
        return pin(&mut lock, &lock_path, manifest, release, content_hash, Some(triple));
    }

    if let Some(entry) = lock.plugin_entry(&manifest.name) {
        if entry.content_hash.as_deref() == Some(content_hash) {
            // Already approved — reproducible from the lockfile, no prompt.
            return Ok(());
        }
        // A changed asset re-prompts (PLG-17); reject aborts.
        if !decide(mode, manifest, release, content_hash) {
            return Err(PluginError::Untrusted(manifest.name.clone()));
        }
        return pin(&mut lock, &lock_path, manifest, release, content_hash, Some(triple));
    }

    if !decide(mode, manifest, release, content_hash) {
        return Err(PluginError::Untrusted(manifest.name.clone()));
    }
    pin(&mut lock, &lock_path, manifest, release, content_hash, Some(triple))
}

/// The TOFU decision for one artifact: deterministic in the non-
/// interactive modes, the prompt otherwise.
fn decide(mode: TrustMode, manifest: &Manifest, source: &str, content_hash: &str) -> bool {
    match mode {
        TrustMode::AcceptAll => true,
        TrustMode::RejectUntrusted => false,
        TrustMode::Interactive => prompt(manifest, source, content_hash),
    }
}

/// Permissions consent at `add` (plugin-interface v2, PLG-23 / D11) —
/// the FIRST of the two trust gates, distinct from the artifact-hash
/// TOFU above: the declared `[plugin.permissions]` are printed and
/// require an explicit accept/deny; a deny aborts the install
/// ([`PluginError::PermissionsDenied`]). A manifest declaring no
/// permissions needs no consent. The deterministic modes bypass:
/// `AcceptAll` accepts, `RejectUntrusted` denies.
pub fn ensure_permissions_consented(manifest: &Manifest, mode: TrustMode) -> PluginResult<()> {
    if manifest.permissions.is_default() {
        return Ok(());
    }
    print_permissions(manifest);
    let approved = match mode {
        TrustMode::AcceptAll => true,
        TrustMode::RejectUntrusted => false,
        TrustMode::Interactive => {
            if !std::io::stdin().is_terminal() {
                // Non-tty stdin denies — CI must opt in explicitly.
                false
            } else {
                eprint!("  Grant these permissions? [y/N] ");
                let mut line = String::new();
                if std::io::stdin().read_line(&mut line).is_err() {
                    return Err(PluginError::PermissionsDenied(manifest.name.clone()));
                }
                matches!(line.trim(), "y" | "Y" | "yes")
            }
        }
    };
    if !approved {
        return Err(PluginError::PermissionsDenied(manifest.name.clone()));
    }
    Ok(())
}

/// Print the declared permissions — always, tty or not: the gate must
/// show what it is asking about (PLG-23).
fn print_permissions(manifest: &Manifest) {
    eprintln!();
    eprintln!("  Plugin '{}' declares permissions:", manifest.name);
    if !manifest.permissions.filesystem.is_empty() {
        eprintln!("    filesystem    : {}", manifest.permissions.filesystem.join(", "));
    }
    if manifest.permissions.network {
        eprintln!("    network       : true");
    }
    if !manifest.permissions.process_spawn.is_empty() {
        eprintln!("    process_spawn : {}", manifest.permissions.process_spawn.join(", "));
    }
}

/// Record (or replace) the approved pin and persist the lockfile.
fn pin(
    lock: &mut PiperineLock,
    lock_path: &Path,
    manifest: &Manifest,
    source: &str,
    content_hash: &str,
    triple: Option<&str>,
) -> PluginResult<()> {
    lock.record_plugin(LockEntry {
        name: manifest.name.clone(),
        source: source.to_string(),
        hash: content_hash.to_string(),
        kind: EntryKind::Plugin,
        content_hash: Some(content_hash.to_string()),
        // The device binary's target triple for a release fetch.
        abi: triple.map(str::to_string),
        trusted_at: Some(now_rfc3339()),
    });
    lock.save(lock_path)
        .map_err(|e| PluginError::Other { plugin: manifest.name.clone(), message: e.to_string() })
}

/// Interactive TOFU prompt. Non-tty stdin rejects — CI must opt in
/// explicitly, never by hanging on a prompt.
fn prompt(manifest: &Manifest, source: &str, content_hash: &str) -> bool {
    if !std::io::stdin().is_terminal() {
        return false;
    }
    eprintln!();
    eprintln!("  Plugin '{}' ({}) loaded from:", manifest.name, manifest.shape().as_str());
    eprintln!("    {source}");
    if !manifest.permissions.filesystem.is_empty() {
        eprintln!("    filesystem    : {}", manifest.permissions.filesystem.join(", "));
    }
    if manifest.permissions.network {
        eprintln!("    network       : true");
    }
    if !manifest.permissions.process_spawn.is_empty() {
        eprintln!("    process_spawn : {}", manifest.permissions.process_spawn.join(", "));
    }
    eprintln!("  Artifact hash: {content_hash}");
    eprint!("  Trust and save to Piperine.lock? [y/N] ");
    let mut line = String::new();
    if std::io::stdin().read_line(&mut line).is_err() {
        return false;
    }
    matches!(line.trim(), "y" | "Y" | "yes")
}

/// RFC3339 UTC timestamp without pulling a chrono dependency.
fn now_rfc3339() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Days-to-date conversion (civil-from-days, Howard Hinnant's algorithm).
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}
