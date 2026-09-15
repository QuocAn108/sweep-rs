use crate::core::traits::{ArtifactTarget, ProjectDetector, ProjectType};
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone)]
pub struct ElixirDetector;

impl ElixirDetector {
    pub fn new() -> Self {
        Self
    }
}

impl ProjectDetector for ElixirDetector {
    fn name(&self) -> ProjectType {
        ProjectType::Elixir
    }

    fn detect(&self, dir: &Path) -> bool {
        dir.join("mix.exs").is_file()
    }

    fn get_artifacts(&self, _project_root: &Path) -> Vec<ArtifactTarget> {
        vec![
            ArtifactTarget {
                name: "_build",
                rel_path: PathBuf::from("_build"),
                is_reconstructible: true,
            },
            ArtifactTarget {
                name: "deps",
                rel_path: PathBuf::from("deps"),
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
    fn test_detect_elixir_project() {
        let temp = tempdir().unwrap();
        let mix_exs = temp.path().join("mix.exs");
        std::fs::write(&mix_exs, "defmodule MyApp.MixProject do\nend").unwrap();

        let detector = ElixirDetector::new();
        assert!(detector.detect(temp.path()));
        assert_eq!(detector.name(), ProjectType::Elixir);

        let artifacts = detector.get_artifacts(temp.path());
        assert_eq!(artifacts.len(), 2);
        assert_eq!(artifacts[0].name, "_build");
    }

    #[test]
    fn test_detect_non_elixir_project() {
        let temp = tempdir().unwrap();
        let detector = ElixirDetector::new();
        assert!(!detector.detect(temp.path()));
    }
}
