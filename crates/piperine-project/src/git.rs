//! Git checkouts of dependency packages: clone or fetch a repository and check a target out.

use std::fmt;
use std::path::{Path, PathBuf};

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

/// Clones or fetches a repository, then checks out the required target.
/// If `dest` does not exist, it clones `url` to `dest`.
/// If `dest` exists, it fetches updates.
/// Finally, it checks out `target` (a branch, tag, or commit hash).
/// Returns the actual commit hash checked out.
pub fn sync_and_checkout(url: &str, dest: &Path, target: &str) -> Result<String, GitError> {
    use git2::Repository;
    use git2::build::RepoBuilder;

    let repo = if !dest.exists() {
        RepoBuilder::new()
            .clone(url, dest)
            .map_err(|e| GitError::CloneFailed {
                url: url.to_string(),
                msg: e.to_string(),
            })?
    } else {
        let repo = Repository::open(dest).map_err(|e| GitError::FetchFailed {
            path: dest.to_path_buf(),
            msg: e.to_string(),
        })?;

        {
            let mut remote = repo
                .find_remote("origin")
                .or_else(|_| repo.remote("origin", url))
                .map_err(|e| GitError::FetchFailed {
                    path: dest.to_path_buf(),
                    msg: e.to_string(),
                })?;

            remote
                .fetch(&["+refs/heads/*:refs/remotes/origin/*"], None, None)
                .map_err(|e| GitError::FetchFailed {
                    path: dest.to_path_buf(),
                    msg: e.to_string(),
                })?;
        }

        repo
    };

    // Parse target revision (branch, tag, or commit)
    let rev = repo
        .revparse_single(target)
        .or_else(|_| repo.revparse_single(&format!("origin/{}", target)))
        .map_err(|e| GitError::CheckoutFailed {
            path: dest.to_path_buf(),
            target: target.to_string(),
            msg: e.to_string(),
        })?;

    let commit = rev.peel_to_commit().map_err(|e| GitError::CheckoutFailed {
        path: dest.to_path_buf(),
        target: target.to_string(),
        msg: e.to_string(),
    })?;

    // Checkout the tree
    let mut checkout_builder = git2::build::CheckoutBuilder::new();
    checkout_builder.force();

    repo.checkout_tree(commit.as_object(), Some(&mut checkout_builder))
        .map_err(|e| GitError::CheckoutFailed {
            path: dest.to_path_buf(),
            target: target.to_string(),
            msg: e.to_string(),
        })?;

    // Move HEAD
    repo.set_head_detached(commit.id())
        .map_err(|e| GitError::CheckoutFailed {
            path: dest.to_path_buf(),
            target: target.to_string(),
            msg: e.to_string(),
        })?;

    Ok(commit.id().to_string())
}
