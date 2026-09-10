use crate::core::traits::{ArtifactTarget, ProjectDetector, ProjectType};
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone)]
pub struct RustDetector;

impl RustDetector {
    pub fn new() -> Self {
        Self
    }
}

impl ProjectDetector for RustDetector {
    fn name(&self) -> ProjectType {
        ProjectType::Rust
    }

    fn detect(&self, dir: &Path) -> bool {
        dir.join("Cargo.toml").is_file()
    }

    fn get_artifacts(&self, _project_root: &Path) -> Vec<ArtifactTarget> {
        vec![ArtifactTarget {
            name: "target",
            rel_path: PathBuf::from("target"),
            is_reconstructible: true,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_detect_rust_project() {
        let temp = tempdir().unwrap();
        let cargo_toml = temp.path().join("Cargo.toml");
        std::fs::write(&cargo_toml, "[package]\nname = \"test\"").unwrap();

        let detector = RustDetector::new();
        assert!(detector.detect(temp.path()));
        assert_eq!(detector.name(), ProjectType::Rust);

        let artifacts = detector.get_artifacts(temp.path());
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].name, "target");
        assert_eq!(artifacts[0].rel_path, PathBuf::from("target"));
    }

    #[test]
    fn test_detect_non_rust_project() {
        let temp = tempdir().unwrap();
        let detector = RustDetector::new();
        assert!(!detector.detect(temp.path()));
    }
}
