use crate::core::traits::{ArtifactTarget, ProjectDetector, ProjectType};
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone)]
pub struct RubyDetector;

impl RubyDetector {
    pub fn new() -> Self {
        Self
    }
}

impl ProjectDetector for RubyDetector {
    fn name(&self) -> ProjectType {
        ProjectType::Ruby
    }

    fn detect(&self, dir: &Path) -> bool {
        dir.join("Gemfile").is_file()
    }

    fn get_artifacts(&self, _project_root: &Path) -> Vec<ArtifactTarget> {
        vec![
            ArtifactTarget {
                name: "vendor",
                rel_path: PathBuf::from("vendor").join("bundle"),
                is_reconstructible: true,
            },
            ArtifactTarget {
                name: ".bundle",
                rel_path: PathBuf::from(".bundle"),
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
    fn test_detect_ruby_project() {
        let temp = tempdir().unwrap();
        let gemfile = temp.path().join("Gemfile");
        std::fs::write(&gemfile, "source 'https://rubygems.org'\ngem 'rails'").unwrap();

        let detector = RubyDetector::new();
        assert!(detector.detect(temp.path()));
        assert_eq!(detector.name(), ProjectType::Ruby);

        let artifacts = detector.get_artifacts(temp.path());
        assert_eq!(artifacts.len(), 2);
    }

    #[test]
    fn test_detect_non_ruby_project() {
        let temp = tempdir().unwrap();
        let detector = RubyDetector::new();
        assert!(!detector.detect(temp.path()));
    }
}
