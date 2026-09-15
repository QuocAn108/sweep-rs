use crate::core::traits::{ArtifactTarget, ProjectDetector, ProjectType};
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone)]
pub struct GoDetector;

impl GoDetector {
    pub fn new() -> Self {
        Self
    }
}

impl ProjectDetector for GoDetector {
    fn name(&self) -> ProjectType {
        ProjectType::Go
    }

    fn detect(&self, dir: &Path) -> bool {
        dir.join("go.mod").is_file() || dir.join("go.work").is_file()
    }

    fn get_artifacts(&self, _project_root: &Path) -> Vec<ArtifactTarget> {
        vec![
            ArtifactTarget {
                name: "bin",
                rel_path: PathBuf::from("bin"),
                is_reconstructible: true,
            },
            ArtifactTarget {
                name: "dist",
                rel_path: PathBuf::from("dist"),
                is_reconstructible: true,
            },
            ArtifactTarget {
                name: "vendor",
                rel_path: PathBuf::from("vendor"),
                is_reconstructible: true,
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_detect_go_project() {
        let temp = tempdir().unwrap();
        let go_mod = temp.path().join("go.mod");
        std::fs::write(&go_mod, "module example.com/myapp\n\ngo 1.22").unwrap();

        let detector = GoDetector::new();
        assert!(detector.detect(temp.path()));
        assert_eq!(detector.name(), ProjectType::Go);

        let artifacts = detector.get_artifacts(temp.path());
        assert_eq!(artifacts.len(), 3);
        assert_eq!(artifacts[0].name, "bin");
    }

    #[test]
    fn test_detect_non_go_project() {
        let temp = tempdir().unwrap();
        let detector = GoDetector::new();
        assert!(!detector.detect(temp.path()));
    }
}
