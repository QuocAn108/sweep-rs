use sweep_rs::core::cleaner::Cleaner;
use sweep_rs::core::engine::ScanEngine;
use sweep_rs::core::traits::ProjectType;
use sweep_rs::detectors::DetectorRegistry;
use tempfile::tempdir;

#[test]
fn test_monorepo_multi_ecosystem_detection() {
    let temp = tempdir().unwrap();
    let root = temp.path();

    // 1. Rust workspace & sub-crate
    let rust_root = root.join("rust_service");
    std::fs::create_dir_all(&rust_root).unwrap();
    std::fs::write(rust_root.join("Cargo.toml"), "[package]\nname = \"svc\"").unwrap();
    let rust_target = rust_root.join("target");
    std::fs::create_dir_all(&rust_target).unwrap();
    std::fs::write(rust_target.join("output.bin"), vec![0u8; 1024]).unwrap();

    // 2. Node.js web app
    let node_root = root.join("web_frontend");
    std::fs::create_dir_all(&node_root).unwrap();
    std::fs::write(node_root.join("package.json"), "{\"name\": \"web\"}").unwrap();
    let node_modules = node_root.join("node_modules").join("pkg");
    std::fs::create_dir_all(&node_modules).unwrap();
    std::fs::write(node_modules.join("bundle.js"), vec![0u8; 2048]).unwrap();

    // 3. Python backend
    let py_root = root.join("ml_service");
    std::fs::create_dir_all(&py_root).unwrap();
    std::fs::write(py_root.join("pyproject.toml"), "[project]\nname=\"ml\"").unwrap();
    let venv = py_root.join(".venv");
    std::fs::create_dir_all(&venv).unwrap();
    std::fs::write(venv.join("pyvenv.cfg"), "home = /usr/bin").unwrap();

    // 4. .NET tool
    let dotnet_root = root.join("dotnet_tool");
    std::fs::create_dir_all(&dotnet_root).unwrap();
    std::fs::write(dotnet_root.join("tool.csproj"), "<Project />").unwrap();
    let bin_dir = dotnet_root.join("bin");
    std::fs::create_dir_all(&bin_dir).unwrap();
    std::fs::write(bin_dir.join("tool.dll"), vec![0u8; 512]).unwrap();

    let registry = DetectorRegistry::default_all();
    let engine = ScanEngine::new(registry);

    let (projects, stats) = engine.scan::<fn(usize)>(root, None);

    assert_eq!(projects.len(), 4);
    assert!(stats.dirs_inspected >= 4);

    let types: Vec<ProjectType> = projects.iter().map(|p| p.project_type.clone()).collect();
    assert!(types.contains(&ProjectType::Rust));
    assert!(types.contains(&ProjectType::Node));
    assert!(types.contains(&ProjectType::Python));
    assert!(types.contains(&ProjectType::Dotnet));
}

#[test]
fn test_ecosystem_type_filtering() {
    let temp = tempdir().unwrap();
    let root = temp.path();

    let rust_dir = root.join("rust_app");
    std::fs::create_dir_all(&rust_dir).unwrap();
    std::fs::write(rust_dir.join("Cargo.toml"), "[package]\nname = \"r\"").unwrap();
    std::fs::create_dir_all(rust_dir.join("target")).unwrap();
    std::fs::write(rust_dir.join("target").join("a.out"), b"123").unwrap();

    let node_dir = root.join("node_app");
    std::fs::create_dir_all(&node_dir).unwrap();
    std::fs::write(node_dir.join("package.json"), "{\"name\": \"n\"}").unwrap();
    std::fs::create_dir_all(node_dir.join("node_modules")).unwrap();
    std::fs::write(node_dir.join("node_modules").join("x.js"), b"123").unwrap();

    // Filter type = rust
    let rust_registry = DetectorRegistry::for_type("rust").unwrap();
    let engine = ScanEngine::new(rust_registry);
    let (projects, _) = engine.scan::<fn(usize)>(root, None);

    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].project_type, ProjectType::Rust);

    // Filter type = node
    let node_registry = DetectorRegistry::for_type("node").unwrap();
    let engine_node = ScanEngine::new(node_registry);
    let (node_projects, _) = engine_node.scan::<fn(usize)>(root, None);

    assert_eq!(node_projects.len(), 1);
    assert_eq!(node_projects[0].project_type, ProjectType::Node);
}

#[test]
fn test_stale_filter_filtering() {
    let temp = tempdir().unwrap();
    let repo_dir = temp.path().join("my_repo");
    std::fs::create_dir_all(&repo_dir).unwrap();

    let repo = git2::Repository::init(&repo_dir).unwrap();
    std::fs::write(repo_dir.join("Cargo.toml"), "[package]\nname = \"repo\"").unwrap();
    std::fs::create_dir_all(repo_dir.join("target")).unwrap();
    std::fs::write(repo_dir.join("target").join("out.bin"), b"data").unwrap();

    // Commit file
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("Cargo.toml")).unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let sig = git2::Signature::now("Dev", "dev@example.com").unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
        .unwrap();

    let registry = DetectorRegistry::default_all();

    // Stale 30: repo was just committed today -> must be filtered out
    let engine_stale30 = ScanEngine::new(registry).with_stale_filter(Some(30));
    let (projects_stale, _) = engine_stale30.scan::<fn(usize)>(&repo_dir, None);
    assert_eq!(projects_stale.len(), 0);

    // Stale 0: repo commit age >= 0 -> must be included
    let registry2 = DetectorRegistry::default_all();
    let engine_stale0 = ScanEngine::new(registry2).with_stale_filter(Some(0));
    let (projects_all, _) = engine_stale0.scan::<fn(usize)>(&repo_dir, None);
    assert_eq!(projects_all.len(), 1);
}

#[test]
fn test_safe_cleaner_e2e() {
    let temp = tempdir().unwrap();
    let app_dir = temp.path().join("app");
    let target_dir = app_dir.join("target");
    std::fs::create_dir_all(&target_dir).unwrap();
    std::fs::write(target_dir.join("app.exe"), vec![0u8; 4096]).unwrap();

    let cleaner = Cleaner::new();
    let clean_item = cleaner.rename_to_trash(&target_dir, 4096).unwrap();

    // Original target path is immediately gone
    assert!(!target_dir.exists());

    // Trash path exists
    assert!(clean_item.trash_path.exists());

    // Purge in background
    let handle = cleaner.purge_items_in_background(vec![clean_item.clone()]);
    let report = handle.join().unwrap();

    assert_eq!(report.items_cleaned, 1);
    assert_eq!(report.total_bytes_reclaimed, 4096);
    assert!(!clean_item.trash_path.exists());
}
