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
        ]
    }
}
