use std::path::PathBuf;
use anyhow::Result;
use inquire::MultiSelect;

use crate::core::size::format_bytes;
use crate::core::traits::{DiscoveredProject, GitInfo, GitStatus, ProjectType};

#[derive(Debug, Clone)]
pub struct SelectableArtifact {
    pub project_root: PathBuf,
    pub project_type: ProjectType,
    pub artifact_path: PathBuf,
    pub artifact_name: &'static str,
    pub size_bytes: u64,
    pub git_info: Option<GitInfo>,
}

impl std::fmt::Display for SelectableArtifact {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let path_str = self.project_root.display().to_string();
        let display_path = if path_str.len() > 30 {
            format!("...{}", &path_str[path_str.len() - 27..])
        } else {
            path_str
        };

        let status_str = match &self.git_info {
            Some(info) => {
                if info.is_dirty {
                    format!("{} (Dirty)", info.status)
                } else {
                    info.status.to_string()
                }
            }
            None => "No Git".to_string(),
        };

        write!(
            f,
            "{} ({}: {}) - {} [{}]",
            display_path,
            self.project_type,
            self.artifact_name,
            format_bytes(self.size_bytes),
            status_str
        )
    }
}

pub fn prompt_selection(projects: &[DiscoveredProject]) -> Result<Vec<SelectableArtifact>> {
    let mut items = Vec::new();
    let mut default_indices = Vec::new();

    for project in projects {
        for artifact in &project.artifacts {
            let is_stale = project
                .git_info
                .as_ref()
                .map(|info| info.status == GitStatus::Stale && !info.is_dirty)
                .unwrap_or(false);

            if is_stale {
                default_indices.push(items.len());
            }

            items.push(SelectableArtifact {
                project_root: project.root.clone(),
                project_type: project.project_type.clone(),
                artifact_path: artifact.abs_path.clone(),
                artifact_name: artifact.target.name,
                size_bytes: artifact.size_bytes,
                git_info: project.git_info.clone(),
            });
        }
    }

    if items.is_empty() {
        return Ok(Vec::new());
    }

    let prompt = MultiSelect::new("Select the build artifacts to clean:", items)
        .with_default(&default_indices)
        .with_help_message(
            "Space: toggle item, 'a': toggle all, Enter: confirm selection, Esc: cancel",
        );

    match prompt.prompt() {
        Ok(selected) => Ok(selected),
        Err(inquire::InquireError::OperationCanceled) => Ok(Vec::new()),
        Err(e) => Err(anyhow::anyhow!("Interactive selection failed: {}", e)),
    }
}
