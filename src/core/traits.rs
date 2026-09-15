use std::path::{Path, PathBuf};

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProjectType {
    Rust,
    Node,
    Dotnet,
    Python,
    Java,
    Go,
    Php,
    Ruby,
    Cpp,
    Flutter,
    Swift,
    Elixir,
    Custom(String),
}

impl std::fmt::Display for ProjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectType::Rust => write!(f, "Rust"),
            ProjectType::Node => write!(f, "Node"),
            ProjectType::Dotnet => write!(f, ".NET"),
            ProjectType::Python => write!(f, "Python"),
            ProjectType::Java => write!(f, "Java"),
            ProjectType::Go => write!(f, "Go"),
            ProjectType::Php => write!(f, "PHP"),
            ProjectType::Ruby => write!(f, "Ruby"),
            ProjectType::Cpp => write!(f, "C/C++"),
            ProjectType::Flutter => write!(f, "Flutter"),
            ProjectType::Swift => write!(f, "Swift"),
            ProjectType::Elixir => write!(f, "Elixir"),
            ProjectType::Custom(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitStatus {
    Stale,
    Active,
    Moderate,
    Unknown,
}

impl std::fmt::Display for GitStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GitStatus::Stale => write!(f, "Stale"),
            GitStatus::Active => write!(f, "Active"),
            GitStatus::Moderate => write!(f, "Moderate"),
            GitStatus::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitInfo {
    pub last_commit_days: Option<u64>,
    pub is_dirty: bool,
    pub status: GitStatus,
}

#[allow(dead_code)]
impl GitInfo {
    pub fn display_status(&self) -> String {
        if self.is_dirty {
            match self.status {
                GitStatus::Unknown => "Dirty".to_string(),
                other => format!("{} (Dirty)", other),
            }
        } else {
            self.status.to_string()
        }
    }

    pub fn display_last_commit(&self) -> String {
        match self.last_commit_days {
            Some(0) => "Today".to_string(),
            Some(1) => "1 day ago".to_string(),
            Some(days) => format!("{} days ago", days),
            None => "N/A".to_string(),
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

#[derive(Debug, Clone)]
pub struct DiscoveredProject {
    pub root: PathBuf,
    pub project_type: ProjectType,
    pub artifacts: Vec<DiscoveredArtifact>,
    pub git_info: Option<GitInfo>,
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
