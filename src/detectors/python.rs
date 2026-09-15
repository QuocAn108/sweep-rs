use crate::core::traits::{ArtifactTarget, ProjectDetector, ProjectType};
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone)]
pub struct PythonDetector;

impl PythonDetector {
    pub fn new() -> Self {
        Self
    }
}

impl ProjectDetector for PythonDetector {
    fn name(&self) -> ProjectType {
        ProjectType::Python
    }

    fn detect(&self, dir: &Path) -> bool {
        dir.join("pyproject.toml").is_file()
            || dir.join("requirements.txt").is_file()
            || dir.join("Pipfile").is_file()
            || dir.join("setup.py").is_file()
    }

    fn get_artifacts(&self, _project_root: &Path) -> Vec<ArtifactTarget> {
        vec![
            ArtifactTarget {
                name: ".venv",
                rel_path: PathBuf::from(".venv"),
                is_reconstructible: true,
            },
            ArtifactTarget {
                name: "venv",
                rel_path: PathBuf::from("venv"),
                is_reconstructible: true,
            },
            ArtifactTarget {
                name: "__pycache__",
                rel_path: PathBuf::from("__pycache__"),
                is_reconstructible: true,
            },
            ArtifactTarget {
                name: ".pytest_cache",
                rel_path: PathBuf::from(".pytest_cache"),
                is_reconstructible: true,
            },
            ArtifactTarget {
                name: ".mypy_cache",
                rel_path: PathBuf::from(".mypy_cache"),
                is_reconstructible: true,
            },
            ArtifactTarget {
                name: ".ruff_cache",
                rel_path: PathBuf::from(".ruff_cache"),
                is_reconstructible: true,
            },
            ArtifactTarget {
                name: "dist",
                rel_path: PathBuf::from("dist"),
                is_reconstructible: true,
            },
            ArtifactTarget {
                name: "build",
                rel_path: PathBuf::from("build"),
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
    fn test_detect_python_project() {
        let temp = tempdir().unwrap();
        let pyproject = temp.path().join("pyproject.toml");
        std::fs::write(&pyproject, "[project]\nname = \"test\"").unwrap();

        let detector = PythonDetector::new();
        assert!(detector.detect(temp.path()));
        assert_eq!(detector.name(), ProjectType::Python);

        let artifacts = detector.get_artifacts(temp.path());
        assert_eq!(artifacts.len(), 8);
        assert_eq!(artifacts[0].name, ".venv");
    }

    #[test]
    fn test_detect_non_python_project() {
        let temp = tempdir().unwrap();
        let detector = PythonDetector::new();
        assert!(!detector.detect(temp.path()));
    }
}
