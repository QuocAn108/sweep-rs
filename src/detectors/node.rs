use crate::core::traits::{ArtifactTarget, ProjectDetector, ProjectType};
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone)]
pub struct NodeDetector;

impl NodeDetector {
    pub fn new() -> Self {
        Self
    }
}

impl ProjectDetector for NodeDetector {
    fn name(&self) -> ProjectType {
        ProjectType::Node
    }

    fn detect(&self, dir: &Path) -> bool {
        dir.join("package.json").is_file()
    }

    fn get_artifacts(&self, _project_root: &Path) -> Vec<ArtifactTarget> {
        vec![ArtifactTarget {
            name: "node_modules",
            rel_path: PathBuf::from("node_modules"),
            is_reconstructible: true,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_detect_node_project() {
        let temp = tempdir().unwrap();
        let pkg_json = temp.path().join("package.json");
        std::fs::write(&pkg_json, "{\"name\": \"test\"}").unwrap();

        let detector = NodeDetector::new();
        assert!(detector.detect(temp.path()));
        assert_eq!(detector.name(), ProjectType::Node);

        let artifacts = detector.get_artifacts(temp.path());
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].name, "node_modules");
        assert_eq!(artifacts[0].rel_path, PathBuf::from("node_modules"));
    }

    #[test]
    fn test_detect_non_node_project() {
        let temp = tempdir().unwrap();
        let detector = NodeDetector::new();
        assert!(!detector.detect(temp.path()));
    }
}
