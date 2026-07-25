use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GitError {
    #[error("Failed to clone repository {url}: {msg}")]
    CloneFailed { url: String, msg: String },
    #[error("Failed to fetch repository at {path}: {msg}")]
    FetchFailed { path: PathBuf, msg: String },
    #[error("Failed to checkout {target} in {path}: {msg}")]
    CheckoutFailed {
        path: PathBuf,
        target: String,
        msg: String,
    },
    #[error("Failed to get current commit hash in {path}: {msg}")]
    HashFailed { path: PathBuf, msg: String },
    #[error("Bad git source `{arg}`: {reason}")]
    BadSource { arg: String, reason: String },
}

/// A Go-style git source (plugin-interface v2, PLG-22 / D9): a bare
/// `owner/repo` resolves to GitHub; a full git URL (`https://…`, `git@…`)
/// is used verbatim. Parsed once at `piperine add` time — the resolver
/// proper only ever sees the resolved URL.
#[derive(Debug)]
pub struct GitSource {
    url: String,
    name: String,
}

impl GitSource {
    /// Parse an `add` argument into the git URL to clone and the package
    /// name to declare. Anything malformed fails loud — a guessed URL is
    /// worse than no URL.
    pub fn parse(arg: &str) -> Result<Self, GitError> {
        let bad = |reason: &str| GitError::BadSource { arg: arg.to_string(), reason: reason.to_string() };
        let arg = arg.trim();
        if arg.is_empty() {
            return Err(bad("empty source"));
        }
        let url = if arg.contains("://") || arg.starts_with("git@") {
            arg.to_string()
        } else {
            let segments: Vec<&str> = arg.split('/').collect();
            let [owner, repo] = segments.as_slice() else {
                return Err(bad("a bare source must be exactly `owner/repo` (a full git URL is used verbatim)"));
            };
            if owner.is_empty() || repo.is_empty() {
                return Err(bad("`owner/repo` segments must be non-empty"));
            }
            format!("https://github.com/{owner}/{repo}")
        };
        let name = url
            .trim_end_matches('/')
            .rsplit(['/', ':'])
            .next()
            .unwrap_or_default()
            .trim_end_matches(".git")
            .to_string();
        if name.is_empty() {
            return Err(bad("cannot derive a package name from the URL"));
        }
        Ok(Self { url, name })
    }

    /// The git URL to clone (bare forms already resolved to GitHub).
    pub fn url(&self) -> &str {
        &self.url
    }

    /// The package name derived from the URL's last segment.
    pub fn name(&self) -> &str {
        &self.name
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_owner_repo_resolves_to_github() {
        let src = GitSource::parse("acme/bjt-models").unwrap();
        assert_eq!(src.url(), "https://github.com/acme/bjt-models");
        assert_eq!(src.name(), "bjt-models");
    }

    #[test]
    fn full_https_url_is_verbatim() {
        let src = GitSource::parse("https://github.com/acme/bjt-models").unwrap();
        assert_eq!(src.url(), "https://github.com/acme/bjt-models");
        assert_eq!(src.name(), "bjt-models");
    }

    #[test]
    fn full_url_with_dot_git_suffix_is_verbatim() {
        let src = GitSource::parse("https://github.com/acme/bjt-models.git").unwrap();
        assert_eq!(src.url(), "https://github.com/acme/bjt-models.git");
        assert_eq!(src.name(), "bjt-models");
    }

    #[test]
    fn scp_style_git_url_is_verbatim() {
        let src = GitSource::parse("git@github.com:acme/bjt-models.git").unwrap();
        assert_eq!(src.url(), "git@github.com:acme/bjt-models.git");
        assert_eq!(src.name(), "bjt-models");
    }

    #[test]
    fn non_github_full_url_is_verbatim() {
        let src = GitSource::parse("https://gitlab.com/x/y").unwrap();
        assert_eq!(src.url(), "https://gitlab.com/x/y");
        assert_eq!(src.name(), "y");
    }

    #[test]
    fn malformed_bare_sources_fail_loud() {
        for arg in ["", "acme", "acme/", "/bjt", "a/b/c"] {
            let err = GitSource::parse(arg).unwrap_err();
            assert!(matches!(err, GitError::BadSource { .. }), "{arg}: {err}");
        }
    }
}
