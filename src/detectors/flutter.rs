use crate::core::traits::{ArtifactTarget, ProjectDetector, ProjectType};
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone)]
pub struct FlutterDetector;

impl FlutterDetector {
    pub fn new() -> Self {
        Self
    }
}

impl ProjectDetector for FlutterDetector {
    fn name(&self) -> ProjectType {
        ProjectType::Flutter
    }

    fn detect(&self, dir: &Path) -> bool {
        dir.join("pubspec.yaml").is_file()
    }

    fn get_artifacts(&self, _project_root: &Path) -> Vec<ArtifactTarget> {
        vec![
            ArtifactTarget {
                name: ".dart_tool",
                rel_path: PathBuf::from(".dart_tool"),
                is_reconstructible: true,
            },
            ArtifactTarget {
                name: "build",
                rel_path: PathBuf::from("build"),
                is_reconstructible: true,
            },
            ArtifactTarget {
                name: ".flutter-plugins",
                rel_path: PathBuf::from(".flutter-plugins"),
                is_reconstructible: true,
            },
            ArtifactTarget {
                name: ".flutter-plugins-dependencies",
                rel_path: PathBuf::from(".flutter-plugins-dependencies"),
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
    fn test_detect_flutter_project() {
        let temp = tempdir().unwrap();
        let pubspec = temp.path().join("pubspec.yaml");
        std::fs::write(
            &pubspec,
            "name: my_app\ndescription: A new Flutter project.",
        )
        .unwrap();

        let detector = FlutterDetector::new();
        assert!(detector.detect(temp.path()));
        assert_eq!(detector.name(), ProjectType::Flutter);

        let artifacts = detector.get_artifacts(temp.path());
        assert_eq!(artifacts.len(), 4);
        assert_eq!(artifacts[0].name, ".dart_tool");
    }

    #[test]
    fn test_detect_non_flutter_project() {
        let temp = tempdir().unwrap();
        let detector = FlutterDetector::new();
        assert!(!detector.detect(temp.path()));
    }
}
