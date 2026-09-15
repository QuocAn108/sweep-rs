use crate::core::traits::{ArtifactTarget, ProjectDetector, ProjectType};
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone)]
pub struct JavaDetector;

impl JavaDetector {
    pub fn new() -> Self {
        Self
    }
}

impl ProjectDetector for JavaDetector {
    fn name(&self) -> ProjectType {
        ProjectType::Java
    }

    fn detect(&self, dir: &Path) -> bool {
        dir.join("pom.xml").is_file()
            || dir.join("build.gradle").is_file()
            || dir.join("build.gradle.kts").is_file()
            || dir.join("settings.gradle").is_file()
            || dir.join("settings.gradle.kts").is_file()
    }

    fn get_artifacts(&self, _project_root: &Path) -> Vec<ArtifactTarget> {
        vec![
            ArtifactTarget {
                name: "target",
                rel_path: PathBuf::from("target"),
                is_reconstructible: true,
            },
            ArtifactTarget {
                name: "build",
                rel_path: PathBuf::from("build"),
                is_reconstructible: true,
            },
            ArtifactTarget {
                name: ".gradle",
                rel_path: PathBuf::from(".gradle"),
                is_reconstructible: true,
            },
            ArtifactTarget {
                name: "out",
                rel_path: PathBuf::from("out"),
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
    fn test_detect_maven_project() {
        let temp = tempdir().unwrap();
        let pom_xml = temp.path().join("pom.xml");
        std::fs::write(&pom_xml, "<project></project>").unwrap();

        let detector = JavaDetector::new();
        assert!(detector.detect(temp.path()));
        assert_eq!(detector.name(), ProjectType::Java);

        let artifacts = detector.get_artifacts(temp.path());
        assert_eq!(artifacts.len(), 4);
    }

    #[test]
    fn test_detect_gradle_project() {
        let temp = tempdir().unwrap();
        let build_gradle = temp.path().join("build.gradle");
        std::fs::write(&build_gradle, "plugins {}").unwrap();

        let detector = JavaDetector::new();
        assert!(detector.detect(temp.path()));
        assert_eq!(detector.name(), ProjectType::Java);
    }

    #[test]
    fn test_detect_non_java_project() {
        let temp = tempdir().unwrap();
        let detector = JavaDetector::new();
        assert!(!detector.detect(temp.path()));
    }
}
