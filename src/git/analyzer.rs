use crate::core::traits::{GitInfo, GitStatus};
use git2::{Repository, StatusOptions};
use std::path::Path;

#[allow(dead_code)]
pub struct GitAnalyzer;

#[allow(dead_code)]
impl GitAnalyzer {
    pub fn analyze(project_root: &Path) -> Option<GitInfo> {
        let repo = Repository::discover(project_root).ok()?;

        let mut status_opts = StatusOptions::new();
        status_opts
            .include_untracked(true)
            .recurse_untracked_dirs(false);

        let is_dirty = repo
            .statuses(Some(&mut status_opts))
            .map(|statuses| !statuses.is_empty())
            .unwrap_or(false);

        let head = match repo.head() {
            Ok(h) => h,
            Err(_) => {
                return Some(GitInfo {
                    last_commit_days: None,
                    is_dirty,
                    status: GitStatus::Unknown,
                });
            }
        };

        let commit = match head.peel_to_commit() {
            Ok(c) => c,
            Err(_) => {
                return Some(GitInfo {
                    last_commit_days: None,
                    is_dirty,
                    status: GitStatus::Unknown,
                });
            }
        };

        let commit_time = commit.time().seconds();
        let now = chrono::Utc::now().timestamp();
        let days_ago = if now > commit_time {
            ((now - commit_time) / 86400) as u64
        } else {
            0
        };

        let status = if days_ago > 30 {
            GitStatus::Stale
        } else if days_ago <= 7 {
            GitStatus::Active
        } else {
            GitStatus::Moderate
        };

        Some(GitInfo {
            last_commit_days: Some(days_ago),
            is_dirty,
            status,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_non_git_directory() {
        let temp = tempdir().unwrap();
        assert!(GitAnalyzer::analyze(temp.path()).is_none());
    }

    #[test]
    fn test_git_repo_lifecycle() {
        let temp = tempdir().unwrap();
        let repo = Repository::init(temp.path()).unwrap();

        let info = GitAnalyzer::analyze(temp.path()).unwrap();
        assert_eq!(info.last_commit_days, None);
        assert!(!info.is_dirty);
        assert_eq!(info.status, GitStatus::Unknown);

        let file_path = temp.path().join("README.md");
        std::fs::write(&file_path, "# Test Repo").unwrap();

        let info_dirty = GitAnalyzer::analyze(temp.path()).unwrap();
        assert!(info_dirty.is_dirty);

        let mut index = repo.index().unwrap();
        index.add_path(Path::new("README.md")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();

        let sig = git2::Signature::now("Test Dev", "test@example.com").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
            .unwrap();

        let info_clean = GitAnalyzer::analyze(temp.path()).unwrap();
        assert_eq!(info_clean.last_commit_days, Some(0));
        assert!(!info_clean.is_dirty);
        assert_eq!(info_clean.status, GitStatus::Active);
    }
}
