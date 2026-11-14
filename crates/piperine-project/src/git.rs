//! Git checkouts of dependency packages: clone or fetch a repository and check a target out.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::manifest::GitRef;

/// Why a repository could not be fetched or checked out; each carries git's own message.
#[derive(Debug)]
pub enum GitError {
    CloneFailed { url: String, msg: String },
    FetchFailed { path: PathBuf, msg: String },
    CheckoutFailed { path: PathBuf, target: String, msg: String },
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GitError::CloneFailed { url, msg } => write!(f, "Failed to clone repository {url}: {msg}"),
            GitError::FetchFailed { path, msg } => write!(f, "Failed to fetch repository at {}: {msg}", path.display()),
            GitError::CheckoutFailed { path, target, msg } => write!(f, "Failed to checkout {target} in {}: {msg}", path.display()),
        }
    }
}

impl std::error::Error for GitError {}

/// A repository URL as identity: without a trailing `/`, and for a remote URL without a trailing `.git`, so that one
/// repository spelled two ways is one.
pub fn normalize_url(url: &str) -> String {
    let url = url.trim_end_matches('/');
    if !is_remote(url) {
        return url.to_string();
    }
    url.strip_suffix(".git").unwrap_or(url).trim_end_matches('/').to_string()
}

/// A remote URL: scheme `https://`, `http://`, `ssh://`, `git://`, or scp-like `user@host:path`.
/// Only there does a trailing `.git` name the same repository; a local path (or `file://`) with
/// and without it are two repositories.
fn is_remote(url: &str) -> bool {
    if ["https://", "http://", "ssh://", "git://"].iter().any(|scheme| url.starts_with(scheme)) {
        return true;
    }
    // scp-like: `user@host:path`, with no `/` before the `:` and no scheme.
    match (url.find('@'), url.find(':'), url.find('/')) {
        (Some(at), Some(colon), slash) => at > 0 && at < colon && slash.is_none_or(|slash| colon < slash) && !url.contains("://"),
        _ => false,
    }
}

/// The text of a dependency's source: `git+<url>`, then `?branch=<b>`, `?tag=<t>` or `?rev=<r>`
/// when one was given.
pub fn source_text(url: &str, reference: &GitRef) -> String {
    match reference {
        GitRef::DefaultBranch => format!("git+{url}"),
        GitRef::Branch(b) => format!("git+{url}?branch={b}"),
        GitRef::Tag(t) => format!("git+{url}?tag={t}"),
        GitRef::Rev(r) => format!("git+{url}?rev={r}"),
    }
}

/// Clones or fetches a repository, then checks out the commit `reference` names.
/// If `dest` does not exist, it clones `url` to `dest`; the heads and the tags are then fetched.
/// A branch is `refs/remotes/origin/<b>`, a tag `refs/tags/<t>`, a revision as given, and none
/// the head of the repository's default branch.
/// Returns the commit hash checked out.
pub fn sync_and_checkout(url: &str, dest: &Path, reference: &GitRef) -> Result<String, GitError> {
    use git2::Repository;
    use git2::build::RepoBuilder;

    let target = match reference {
        GitRef::DefaultBranch => "the default branch".to_string(),
        GitRef::Branch(b) => format!("branch {b}"),
        GitRef::Tag(t) => format!("tag {t}"),
        GitRef::Rev(r) => format!("revision {r}"),
    };
    let checkout_failed = |msg: String| GitError::CheckoutFailed { path: dest.to_path_buf(), target: target.clone(), msg };
    let fetch_failed = |msg: String| GitError::FetchFailed { path: dest.to_path_buf(), msg };

    let repo = if !dest.exists() {
        RepoBuilder::new().clone(url, dest).map_err(|e| {
            // A failed clone may leave a directory that would read as a repository next time.
            let _ = std::fs::remove_dir_all(dest);
            GitError::CloneFailed { url: url.to_string(), msg: e.to_string() }
        })?
    } else {
        Repository::open(dest).map_err(|e| fetch_failed(e.to_string()))?
    };

    {
        let mut remote = repo.find_remote("origin").or_else(|_| repo.remote("origin", url)).map_err(|e| fetch_failed(e.to_string()))?;
        remote
            .fetch(&["+refs/heads/*:refs/remotes/origin/*", "+refs/tags/*:refs/tags/*"], None, None)
            .map_err(|e| fetch_failed(e.to_string()))?;
    }

    let object = match reference {
        GitRef::DefaultBranch => repo.revparse_single("refs/remotes/origin/HEAD").or_else(|_| repo.revparse_single("HEAD")),
        GitRef::Branch(b) => repo.revparse_single(&format!("refs/remotes/origin/{b}")),
        GitRef::Tag(t) => repo.revparse_single(&format!("refs/tags/{t}")),
        GitRef::Rev(r) => repo.revparse_single(r),
    }
    .map_err(|e| checkout_failed(e.to_string()))?;
    let commit = object.peel_to_commit().map_err(|e| checkout_failed(e.to_string()))?;

    let mut checkout_builder = git2::build::CheckoutBuilder::new();
    checkout_builder.force();
    repo.checkout_tree(commit.as_object(), Some(&mut checkout_builder)).map_err(|e| checkout_failed(e.to_string()))?;
    repo.set_head_detached(commit.id()).map_err(|e| checkout_failed(e.to_string()))?;
    Ok(commit.id().to_string())
}
