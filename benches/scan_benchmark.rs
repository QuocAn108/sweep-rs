use criterion::{Criterion, black_box, criterion_group, criterion_main};
use sweep_rs::core::engine::ScanEngine;
use sweep_rs::detectors::DetectorRegistry;
use tempfile::tempdir;

fn benchmark_directory_scan(c: &mut Criterion) {
    let temp = tempdir().unwrap();
    let root = temp.path();

    // Generate synthetic projects with artifacts
    for i in 0..100 {
        let proj_dir = root.join(format!("proj_{}", i));
        std::fs::create_dir_all(&proj_dir).unwrap();

        if i % 2 == 0 {
            std::fs::write(proj_dir.join("Cargo.toml"), "[package]\nname=\"t\"").unwrap();
            let target = proj_dir.join("target");
            std::fs::create_dir_all(&target).unwrap();
            std::fs::write(target.join("output.bin"), vec![0u8; 512]).unwrap();
        } else {
            std::fs::write(proj_dir.join("package.json"), "{\"name\":\"t\"}").unwrap();
            let nm = proj_dir.join("node_modules").join("pkg");
            std::fs::create_dir_all(&nm).unwrap();
            std::fs::write(nm.join("index.js"), vec![0u8; 512]).unwrap();
        }
    }

    let registry = DetectorRegistry::default_all();
    let engine = ScanEngine::new(registry);

    c.bench_function("scan_100_projects", |b| {
        b.iter(|| {
            let (projects, stats) = engine.scan::<fn(usize)>(black_box(root), None);
            black_box((projects, stats));
        })
    });
}

criterion_group!(benches, benchmark_directory_scan);
criterion_main!(benches);
