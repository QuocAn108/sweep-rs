use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use crate::core::size::compute_dir_size;
use crate::core::traits::{DiscoveredArtifact, DiscoveredProject};
use crate::detectors::DetectorRegistry;
use crate::git::analyzer::GitAnalyzer;

pub const PRUNE_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "bin",
    "obj",
    ".venv",
    "venv",
    "__pycache__",
    ".cache",
    ".idea",
    ".vscode",
    ".sweep-trash",
];

#[derive(Debug, Clone)]
pub struct ScanStats {
    pub dirs_inspected: usize,
    pub duration: Duration,
}

pub struct ScanEngine {
    registry: DetectorRegistry,
    stale_days_filter: Option<u64>,
}

impl ScanEngine {
    pub fn new(registry: DetectorRegistry) -> Self {
        Self {
            registry,
            stale_days_filter: None,
        }
    }

    pub fn with_stale_filter(mut self, stale_days: Option<u64>) -> Self {
        self.stale_days_filter = stale_days;
        self
    }

    pub fn scan<F>(
        &self,
        root: &Path,
        on_dir_inspected: Option<F>,
    ) -> (Vec<DiscoveredProject>, ScanStats)
    where
        F: Fn(usize) + Send + Sync + 'static,
    {
        let start = Instant::now();
        let dirs_count = Arc::new(AtomicUsize::new(0));
        let dirs_count_clone = dirs_count.clone();

        let mut projects = Vec::new();

        let walker = jwalk::WalkDir::new(root)
            .follow_links(false)
            .skip_hidden(false)
            .process_read_dir(move |_depth, _path, _read_dir_state, children| {
                for entry in children.iter_mut().flatten() {
                    if entry.file_type.is_dir()
                        && let Some(name) = entry.file_name.to_str()
                        && PRUNE_DIRS.contains(&name)
                    {
                        entry.read_children_path = None;
                    }
                }
            });

        for entry_res in walker {
            let entry = match entry_res {
                Ok(e) => e,
                Err(_) => continue,
            };

            if entry.file_type.is_dir() {
                let current_dirs = dirs_count_clone.fetch_add(1, Ordering::Relaxed) + 1;
                if let Some(ref cb) = on_dir_inspected
                    && current_dirs.is_multiple_of(50)
                {
                    cb(current_dirs);
                }

                let dir_path = entry.path();

                let matched_detectors = self.registry.detect_all(&dir_path);
                for detector in matched_detectors {
                    let targets = detector.get_artifacts(&dir_path);
                    let mut discovered_artifacts = Vec::new();

                    for target in targets {
                        let artifact_path = dir_path.join(&target.rel_path);
                        if artifact_path.exists() {
                            let size = compute_dir_size(&artifact_path);
                            discovered_artifacts.push(DiscoveredArtifact {
                                target,
                                abs_path: artifact_path,
                                size_bytes: size,
                            });
                        }
                    }

                    if !discovered_artifacts.is_empty() {
                        let git_info = GitAnalyzer::analyze(&dir_path);

                        if let Some(min_days) = self.stale_days_filter {
                            let is_stale = match &git_info {
                                Some(info) => match info.last_commit_days {
                                    Some(days) => days >= min_days,
                                    None => false,
                                },
                                None => false,
                            };
                            if !is_stale {
                                continue;
                            }
                        }

                        projects.push(DiscoveredProject {
                            root: dir_path.clone(),
                            project_type: detector.name(),
                            artifacts: discovered_artifacts,
                            git_info,
                        });
                    }
                }
            }
        }

        let elapsed = start.elapsed();
        let final_count = dirs_count.load(Ordering::Relaxed);

        (
            projects,
            ScanStats {
                dirs_inspected: final_count,
                duration: elapsed,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::ProjectType;
    use tempfile::tempdir;

    #[test]
    fn test_scan_and_prune_mock_projects() {
        let root = tempdir().unwrap();

        let rust_dir = root.path().join("my-rust-app");
        std::fs::create_dir_all(&rust_dir).unwrap();
        std::fs::write(rust_dir.join("Cargo.toml"), "[package]\nname = \"app\"").unwrap();
        let rust_target = rust_dir.join("target");
        std::fs::create_dir_all(&rust_target).unwrap();
        std::fs::write(rust_target.join("output.bin"), vec![0u8; 1024]).unwrap();

        let node_dir = root.path().join("my-node-app");
        std::fs::create_dir_all(&node_dir).unwrap();
        std::fs::write(node_dir.join("package.json"), "{\"name\": \"app\"}").unwrap();
        let node_modules = node_dir.join("node_modules").join("dummy-pkg");
        std::fs::create_dir_all(&node_modules).unwrap();
        std::fs::write(node_modules.join("index.js"), vec![0u8; 2048]).unwrap();
        std::fs::write(node_modules.join("package.json"), "{\"name\": \"nested\"}").unwrap();

        let registry = DetectorRegistry::default_all();
        let engine = ScanEngine::new(registry);

        let (projects, stats) = engine.scan::<fn(usize)>(root.path(), None);

        assert_eq!(projects.len(), 2);
        assert!(stats.dirs_inspected >= 2);

        let rust_proj = projects
            .iter()
            .find(|p| p.project_type == ProjectType::Rust)
            .unwrap();
        assert_eq!(rust_proj.artifacts.len(), 1);
        assert_eq!(rust_proj.artifacts[0].target.name, "target");
        assert_eq!(rust_proj.artifacts[0].size_bytes, 4096);

        let node_proj = projects
            .iter()
            .find(|p| p.project_type == ProjectType::Node)
            .unwrap();
        assert_eq!(node_proj.artifacts.len(), 1);
        assert_eq!(node_proj.artifacts[0].target.name, "node_modules");
        assert_eq!(node_proj.artifacts[0].size_bytes, 4096 * 2);
    }
}
