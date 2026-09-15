use crate::core::traits::{ArtifactTarget, ProjectDetector, ProjectType};
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone)]
pub struct SwiftDetector;

impl SwiftDetector {
    pub fn new() -> Self {
        Self
    }
}

impl ProjectDetector for SwiftDetector {
    fn name(&self) -> ProjectType {
        ProjectType::Swift
    }

    fn detect(&self, dir: &Path) -> bool {
        if dir.join("Package.swift").is_file() {
            return true;
        }
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let s = name.to_string_lossy();
                if s.ends_with(".xcodeproj") || s.ends_with(".xcworkspace") {
                    return true;
                }
            }
        }
        false
    }

    fn get_artifacts(&self, _project_root: &Path) -> Vec<ArtifactTarget> {
        vec![
            ArtifactTarget {
                name: ".build",
                rel_path: PathBuf::from(".build"),
                is_reconstructible: true,
            },
            ArtifactTarget {
                name: "DerivedData",
                rel_path: PathBuf::from("DerivedData"),
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
    fn test_detect_swift_project() {
        let temp = tempdir().unwrap();
        let pkg_swift = temp.path().join("Package.swift");
        std::fs::write(&pkg_swift, "// swift-tools-version:5.5").unwrap();

        let detector = SwiftDetector::new();
        assert!(detector.detect(temp.path()));
        assert_eq!(detector.name(), ProjectType::Swift);

        let artifacts = detector.get_artifacts(temp.path());
        assert_eq!(artifacts.len(), 2);
        assert_eq!(artifacts[0].name, ".build");
    }

    #[test]
    fn test_detect_non_swift_project() {
        let temp = tempdir().unwrap();
        let detector = SwiftDetector::new();
        assert!(!detector.detect(temp.path()));
    }
}
