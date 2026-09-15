use anyhow::{Context, Result, anyhow};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const WHITELIST_ARTIFACT_NAMES: &[&str] = &[
    "target",
    "node_modules",
    "bin",
    "obj",
    ".venv",
    "venv",
    "__pycache__",
    ".cache",
    "build",
    ".gradle",
    "out",
    ".next",
    ".nuxt",
    "dist",
    ".turbo",
    ".svelte-kit",
    ".astro",
    ".output",
    ".parcel-cache",
    ".angular",
    ".dart_tool",
    ".flutter-plugins",
    ".flutter-plugins-dependencies",
    "vendor",
    ".bundle",
    "_build",
    "deps",
    "cmake-build-debug",
    "cmake-build-release",
    ".build",
    "DerivedData",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
];

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct CleanItem {
    pub original_path: PathBuf,
    pub trash_path: PathBuf,
    pub size_bytes: u64,
}

#[derive(Debug, Default, Clone)]
pub struct CleanReport {
    pub items_cleaned: usize,
    pub total_bytes_reclaimed: u64,
    pub errors: Vec<String>,
}

#[derive(Debug, Default, Clone)]
pub struct Cleaner;

impl Cleaner {
    pub fn new() -> Self {
        Self
    }

    pub fn rename_to_trash(&self, artifact_path: &Path, size_bytes: u64) -> Result<CleanItem> {
        let file_name = artifact_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow!("Invalid artifact path: {}", artifact_path.display()))?;

        if !WHITELIST_ARTIFACT_NAMES.contains(&file_name) {
            return Err(anyhow!(
                "Security Error: '{}' is not in the recognized build artifact whitelist. Aborting deletion.",
                file_name
            ));
        }

        let canonical_path = std::fs::canonicalize(artifact_path).with_context(|| {
            format!(
                "Failed to canonicalize artifact path '{}'",
                artifact_path.display()
            )
        })?;

        let parent = canonical_path
            .parent()
            .ok_or_else(|| anyhow!("Cannot determine parent directory for artifact"))?;

        let trash_name = format!(".sweep-trash-{}", Uuid::new_v4());
        let trash_path = parent.join(trash_name);

        std::fs::rename(&canonical_path, &trash_path).with_context(|| {
            format!(
                "Failed to atomically rename '{}' to '{}'",
                canonical_path.display(),
                trash_path.display()
            )
        })?;

        Ok(CleanItem {
            original_path: canonical_path,
            trash_path,
            size_bytes,
        })
    }

    pub fn purge_items_in_background(
        &self,
        items: Vec<CleanItem>,
    ) -> std::thread::JoinHandle<CleanReport> {
        std::thread::spawn(move || {
            let mut report = CleanReport::default();

            for item in items {
                match remove_dir_all_safe(&item.trash_path) {
                    Ok(_) => {
                        report.items_cleaned += 1;
                        report.total_bytes_reclaimed += item.size_bytes;
                    }
                    Err(e) => {
                        eprintln!(
                            "Failed to purge trash directory '{}': {}",
                            item.trash_path.display(),
                            e
                        );
                        report
                            .errors
                            .push(format!("{}: {}", item.trash_path.display(), e));
                    }
                }
            }

            report
        })
    }
}

fn remove_dir_all_safe(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_dir_all(path) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            set_writable_recursive(path)?;
            std::fs::remove_dir_all(path)
        }
        Err(e) => Err(e),
    }
}

fn set_writable_recursive(path: &Path) -> std::io::Result<()> {
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            set_writable_recursive(&entry.path())?;
        }
    }
    if let Ok(metadata) = std::fs::metadata(path) {
        let mut permissions = metadata.permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
        let _ = std::fs::set_permissions(path, permissions);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_atomic_rename_and_purge() {
        let temp = tempdir().unwrap();
        let target_dir = temp.path().join("target");
        let sub_dir = target_dir.join("debug");
        std::fs::create_dir_all(&sub_dir).unwrap();
        std::fs::write(sub_dir.join("app.bin"), b"mock binary data").unwrap();

        let cleaner = Cleaner::new();
        let item = cleaner.rename_to_trash(&target_dir, 1024).unwrap();

        assert!(!target_dir.exists());
        assert!(item.trash_path.exists());

        let handle = cleaner.purge_items_in_background(vec![item.clone()]);
        let report = handle.join().unwrap();

        assert_eq!(report.items_cleaned, 1);
        assert_eq!(report.total_bytes_reclaimed, 1024);
        assert!(!item.trash_path.exists());
    }

    #[test]
    fn test_security_whitelist_rejection() {
        let temp = tempdir().unwrap();
        let src_dir = temp.path().join("src");
        std::fs::create_dir(&src_dir).unwrap();

        let cleaner = Cleaner::new();
        let result = cleaner.rename_to_trash(&src_dir, 1024);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Security Error"));
        assert!(src_dir.exists());
    }
}
