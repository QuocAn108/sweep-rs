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
            || dir.join("pnpm-workspace.yaml").is_file()
            || dir.join("bun.lockb").is_file()
            || dir.join("bun.lock").is_file()
            || dir.join("deno.json").is_file()
            || dir.join("deno.jsonc").is_file()
    }

    fn get_artifacts(&self, _project_root: &Path) -> Vec<ArtifactTarget> {
        vec![
            ArtifactTarget {
                name: "node_modules",
                rel_path: PathBuf::from("node_modules"),
                is_reconstructible: true,
            },
            ArtifactTarget {
                name: ".next",
                rel_path: PathBuf::from(".next"),
                is_reconstructible: true,
            },
            ArtifactTarget {
                name: ".nuxt",
                rel_path: PathBuf::from(".nuxt"),
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
            ArtifactTarget {
                name: ".turbo",
                rel_path: PathBuf::from(".turbo"),
                is_reconstructible: true,
            },
            ArtifactTarget {
                name: ".svelte-kit",
                rel_path: PathBuf::from(".svelte-kit"),
                is_reconstructible: true,
            },
            ArtifactTarget {
                name: ".astro",
                rel_path: PathBuf::from(".astro"),
                is_reconstructible: true,
            },
            ArtifactTarget {
                name: ".output",
                rel_path: PathBuf::from(".output"),
                is_reconstructible: true,
            },
            ArtifactTarget {
                name: ".parcel-cache",
                rel_path: PathBuf::from(".parcel-cache"),
                is_reconstructible: true,
            },
            ArtifactTarget {
                name: ".angular",
                rel_path: PathBuf::from(".angular"),
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
    fn test_detect_node_project() {
        let temp = tempdir().unwrap();
        let pkg_json = temp.path().join("package.json");
        std::fs::write(&pkg_json, "{\"name\": \"test\"}").unwrap();

        let detector = NodeDetector::new();
        assert!(detector.detect(temp.path()));
        assert_eq!(detector.name(), ProjectType::Node);

        let artifacts = detector.get_artifacts(temp.path());
        assert_eq!(artifacts.len(), 11);
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
