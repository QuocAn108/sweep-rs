use crate::core::traits::{ArtifactTarget, ProjectDetector, ProjectType};
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone)]
pub struct PhpDetector;

impl PhpDetector {
    pub fn new() -> Self {
        Self
    }
}

impl ProjectDetector for PhpDetector {
    fn name(&self) -> ProjectType {
        ProjectType::Php
    }

    fn detect(&self, dir: &Path) -> bool {
        dir.join("composer.json").is_file()
    }

    fn get_artifacts(&self, _project_root: &Path) -> Vec<ArtifactTarget> {
        vec![ArtifactTarget {
            name: "vendor",
            rel_path: PathBuf::from("vendor"),
            is_reconstructible: true,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_detect_php_project() {
        let temp = tempdir().unwrap();
        let composer = temp.path().join("composer.json");
        std::fs::write(&composer, "{\"name\": \"laravel/laravel\"}").unwrap();

        let detector = PhpDetector::new();
        assert!(detector.detect(temp.path()));
        assert_eq!(detector.name(), ProjectType::Php);

        let artifacts = detector.get_artifacts(temp.path());
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].name, "vendor");
    }

    #[test]
    fn test_detect_non_php_project() {
        let temp = tempdir().unwrap();
        let detector = PhpDetector::new();
        assert!(!detector.detect(temp.path()));
    }
}
