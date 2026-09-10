use std::path::{Path, PathBuf};

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectType {
    Rust,
    Node,
    Dotnet,
    Python,
    Custom(String),
}

impl std::fmt::Display for ProjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectType::Rust => write!(f, "Rust"),
            ProjectType::Node => write!(f, "Node"),
            ProjectType::Dotnet => write!(f, ".NET"),
            ProjectType::Python => write!(f, "Python"),
            ProjectType::Custom(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactTarget {
    pub name: &'static str,
    pub rel_path: PathBuf,
    pub is_reconstructible: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DiscoveredArtifact {
    pub target: ArtifactTarget,
    pub abs_path: PathBuf,
    pub size_bytes: u64,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DiscoveredProject {
    pub root: PathBuf,
    pub project_type: ProjectType,
    pub artifacts: Vec<DiscoveredArtifact>,
}

#[allow(dead_code)]
impl DiscoveredProject {
    pub fn total_reclaimable_bytes(&self) -> u64 {
        self.artifacts.iter().map(|a| a.size_bytes).sum()
    }
}

pub trait ProjectDetector: Send + Sync {
    fn name(&self) -> ProjectType;
    fn detect(&self, dir: &Path) -> bool;
    fn get_artifacts(&self, project_root: &Path) -> Vec<ArtifactTarget>;
}
