use std::path::Path;

pub const CLUSTER_SIZE: u64 = 4096;

pub fn allocated_size(len: u64) -> u64 {
    if len == 0 {
        0
    } else {
        len.div_ceil(CLUSTER_SIZE) * CLUSTER_SIZE
    }
}

pub fn compute_dir_size(path: &Path) -> u64 {
    jwalk::WalkDir::new(path)
        .skip_hidden(false)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type.is_file())
        .map(|e| {
            if let Ok(meta) = e.metadata() {
                allocated_size(meta.len())
            } else {
                0
            }
        })
        .sum()
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    const TB: f64 = GB * 1024.0;

    let b = bytes as f64;
    if b < KB {
        format!("{} B", bytes)
    } else if b < MB {
        format!("{:.1} KB", b / KB)
    } else if b < GB {
        format!("{:.1} MB", b / MB)
    } else if b < TB {
        format!("{:.2} GB", b / GB)
    } else {
        format!("{:.2} TB", b / TB)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_allocated_size() {
        assert_eq!(allocated_size(0), 0);
        assert_eq!(allocated_size(1), 4096);
        assert_eq!(allocated_size(4096), 4096);
        assert_eq!(allocated_size(4097), 8192);
        assert_eq!(allocated_size(8192), 8192);
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1024 * 1024 * 10), "10.0 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024 * 2), "2.00 GB");
    }

    #[test]
    fn test_compute_dir_size_with_blocks() {
        let temp = tempdir().unwrap();
        let sub = temp.path().join("sub");
        std::fs::create_dir(&sub).unwrap();

        let file1 = temp.path().join("f1.txt");
        let file2 = sub.join("f2.txt");

        std::fs::write(&file1, "12345").unwrap();
        std::fs::write(&file2, "1234567890").unwrap();

        assert_eq!(compute_dir_size(temp.path()), 4096 * 2);
    }
}
