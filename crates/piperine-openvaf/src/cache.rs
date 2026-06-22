use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Returns the cached `.osdi` path for `source_path` under `cache_dir`,
/// or `None` if the cache is stale or missing.
///
/// Cache key: `<stem>-<mtime_secs>.osdi`.  Cheap and avoids the md5 dep.
/// For a more robust hash use the openvaf built-in cache via
/// `CompilationDestination::Cache`.
pub fn lookup(source_path: &Path, cache_dir: &Path) -> Option<PathBuf> {
    let mtime = mtime_secs(source_path)?;
    let stem = source_path.file_stem()?.to_str()?;
    let candidate = cache_dir.join(format!("{stem}-{mtime}.osdi"));
    if candidate.exists() { Some(candidate) } else { None }
}

/// Returns the output path to write into, creating the cache directory.
pub fn output_path(source_path: &Path, cache_dir: &Path) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(cache_dir)?;
    let mtime = mtime_secs(source_path).unwrap_or(0);
    let stem = source_path.file_stem().and_then(|s| s.to_str()).unwrap_or("module");
    Ok(cache_dir.join(format!("{stem}-{mtime}.osdi")))
}

fn mtime_secs(path: &Path) -> Option<u64> {
    path.metadata()
        .ok()?
        .modified()
        .ok()?
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}
