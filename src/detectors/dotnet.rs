use crate::core::traits::{ArtifactTarget, ProjectDetector, ProjectType};
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone)]
pub struct DotnetDetector;

impl DotnetDetector {
    pub fn new() -> Self {
        Self
    }
}

impl ProjectDetector for DotnetDetector {
    fn name(&self) -> ProjectType {
        ProjectType::Dotnet
    }

    fn detect(&self, dir: &Path) -> bool {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension()
                    && (ext == "csproj" || ext == "sln" || ext == "fsproj")
                {
                    return true;
                }
            }
        }
        false
    }

    fn get_artifacts(&self, _project_root: &Path) -> Vec<ArtifactTarget> {
        vec![
            ArtifactTarget {
                name: "bin",
                rel_path: PathBuf::from("bin"),
                is_reconstructible: true,
            },
            ArtifactTarget {
                name: "obj",
                rel_path: PathBuf::from("obj"),
                is_reconstructible: true,
            },
        ]
    }
}
