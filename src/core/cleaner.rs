use std::path::Path;
use anyhow::Result;

#[allow(dead_code)]
pub struct Cleaner;

#[allow(dead_code)]
impl Cleaner {
    pub fn new() -> Self {
        Self
    }

    pub fn delete_artifact(&self, _path: &Path) -> Result<()> {
        todo!("Cleaner will be implemented in Sprint 3");
    }
}
