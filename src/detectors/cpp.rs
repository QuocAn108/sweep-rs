use crate::core::traits::{ArtifactTarget, ProjectDetector, ProjectType};
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone)]
pub struct CppDetector;

impl CppDetector {
    pub fn new() -> Self {
        Self
    }
}

impl ProjectDetector for CppDetector {
    fn name(&self) -> ProjectType {
        ProjectType::Cpp
    }

    fn detect(&self, dir: &Path) -> bool {
        dir.join("CMakeLists.txt").is_file() || dir.join("meson.build").is_file()
    }

    fn get_artifacts(&self, _project_root: &Path) -> Vec<ArtifactTarget> {
        vec![
            ArtifactTarget {
                name: "build",
                rel_path: PathBuf::from("build"),
                is_reconstructible: true,
            },
            ArtifactTarget {
                name: "cmake-build-debug",
                rel_path: PathBuf::from("cmake-build-debug"),
                is_reconstructible: true,
            },
            ArtifactTarget {
                name: "cmake-build-release",
                rel_path: PathBuf::from("cmake-build-release"),
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
    fn test_detect_cpp_project() {
        let temp = tempdir().unwrap();
        let cmake = temp.path().join("CMakeLists.txt");
        std::fs::write(&cmake, "cmake_minimum_required(VERSION 3.10)").unwrap();

        let detector = CppDetector::new();
        assert!(detector.detect(temp.path()));
        assert_eq!(detector.name(), ProjectType::Cpp);

        let artifacts = detector.get_artifacts(temp.path());
        assert_eq!(artifacts.len(), 3);
        assert_eq!(artifacts[0].name, "build");
    }

    #[test]
    fn test_detect_non_cpp_project() {
        let temp = tempdir().unwrap();
        let detector = CppDetector::new();
        assert!(!detector.detect(temp.path()));
    }
}
