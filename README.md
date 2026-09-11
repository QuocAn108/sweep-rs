```text
███████╗██╗    ██╗███████╗███████╗██████╗       ██████╗ ███████╗
██╔════╝██║    ██║██╔════╝██╔════╝██╔══██╗      ██╔══██╗██╔════╝
███████╗██║ █╗ ██║█████╗  █████╗  ██████╔╝█████╗██████╔╝███████╗
╚════██║██║███╗██║██╔══╝  ██╔══╝  ██╔═══╝ ╚════╝██╔══██╗╚════██║
███████║╚███╔███╔╝███████╗███████╗██║           ██║  ██║███████║
╚══════╝ ╚══╝╚══╝ ╚══════╝╚══════╝╚═╝           ╚═╝  ╚═╝╚══════╝
```

# sweep-rs 🧹

[![CI](https://github.com/QuocAn108/sweep-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/QuocAn108/sweep-rs/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/QuocAn108/sweep-rs)](https://github.com/QuocAn108/sweep-rs/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust Edition](https://img.shields.io/badge/Rust-2024%20Edition-orange.svg)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20Linux%20%7C%20macOS-blue.svg)](https://github.com/QuocAn108/sweep-rs)

> A blazing fast, Git-aware workspace cleanup tool written in Rust. Reclaim gigabytes of disk space from build artifacts (`target`, `node_modules`, `bin/obj`, `.venv`) with zero risk.

---

## ✨ Key Features

* ⚡ **Ultra-Fast Traversal**: Traverses directories in parallel with `jwalk` and `rayon`. Employs aggressive **Early Pruning** to skip deep inspection of known artifact and VCS directories (`.git`, `node_modules`, `target`), achieving $\ge 50,000$ items/sec throughput.
* 🧠 **Git Lifecycle Introspection**: Reads conjoined Git repository metadata (`HEAD` commit age and worktree dirty index) directly using `libgit2` bindings without spawning expensive child processes.
* 🛡️ **Zero False Positives & Multi-Layer Safety**:
  * **Strict Whitelisting**: Only recognizes registered build artifacts.
  * **Risk Classification**: Automatically tags projects as `Stale` (> 30 days, green), `Active` (< 7 days, yellow), or `(Dirty)` (uncommitted changes, red).
  * **Fast Atomic Rename ($O(1)$)**: Renames target folders to `.sweep-trash-<UUID>` in milliseconds on the same filesystem before physical background purging.
* 📊 **Real Block-Size Calculation**: Computes actual disk allocation (4KB cluster multiples) rather than just logical file length.
* 🖥️ **Dual UI Interfaces**:
  * **Fast CLI Mode**: Beautiful ANSI summary tables and interactive `inquire` multiselect checkbox prompts.
  * **Fullscreen TUI Dashboard (`--tui`)**: Rich split-panel terminal interface powered by `ratatui` with live project inspector, storage usage gauges, and modal confirmation popups.

---

## 🥊 Feature Comparison

| Feature | `sweep-rs` | `cargo-sweep` | `kondo` | `npkill` |
| :--- | :---: | :---: | :---: | :---: |
| **Language Ecosystems** | **Rust, Node, Python, .NET** | Rust only | Rust, Node, Python, JVM | Node only |
| **Git Awareness (Commit Age & Dirty State)** | ✅ **Native (`libgit2`)** | ⚠️ Limited | ❌ No | ❌ No |
| **Traversal Speed** | 🚀 **Parallel (`jwalk`)** | Standard | Standard | Node.js Single-thread |
| **Fast Deletion Mechanism** | ⚡ **$O(1)$ Atomic Rename** | Direct deletion | Direct deletion | Direct deletion |
| **Fullscreen TUI Dashboard** | ✅ **Yes (`ratatui`)** | ❌ No | ❌ No | ⚠️ Basic list |
| **Early Subtree Pruning** | ✅ **Yes** | ❌ No | ⚠️ Limited | ❌ No |

---

## 🚀 Installation

### Using Cargo
```bash
cargo install --path .
```

### Pre-built Binaries
Download pre-compiled standalone executables for Windows, Linux, and macOS directly from [GitHub Releases](https://github.com/QuocAn108/sweep-rs/releases).

---

## 📖 Usage & Examples

### 1. Interactive Fullscreen TUI Dashboard
Launch the full terminal user interface with live inspection and gauge metrics:
```bash
sweep-rs --tui
```
Or scan a specific development directory:
```bash
sweep-rs --path ~/workspace --tui
```

#### TUI Hotkeys
* `↑` / `k`: Move up
* `↓` / `j`: Move down
* `Space`: Toggle selection
* `a`: Toggle all
* `d`: Prompt delete confirmation modal
* `y` / `Enter`: Confirm deletion
* `q` / `Esc`: Exit dashboard

---

### 2. Standard CLI Mode
Scan current directory and select artifacts with interactive checkbox prompt:
```bash
sweep-rs
```

### 3. Safe Dry-Run
Simulate scan and check reclaimable disk space without touching any files:
```bash
sweep-rs --dry-run
```

### 4. Clean Stale Projects Only
Filter projects that haven't been committed to in the last 30 days:
```bash
sweep-rs --stale 30
```

### 5. Filter by Ecosystem
Limit scanning to specific language project types (`rust`, `node`, `python`, `dotnet`):
```bash
sweep-rs --type rust
sweep-rs --type node
```

### 6. Non-Interactive / CI/CD Mode
Bypass confirmation prompts and clean immediately:
```bash
sweep-rs --path /ci/workspace --force
```

---

## 🛠️ CLI Options Reference

| Flag / Option | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `-p, --path <PATH>` | `PathBuf` | `.` | Root directory to begin recursive scanning |
| `-d, --dry-run` | `bool` | `false` | Run full scan pipeline and report sizes without deleting files |
| `-s, --stale [<DAYS>]`| `u64` | `30` | Filter and display projects with no commits in the last $N$ days |
| `-t, --type <TYPE>` | `String` | `all` | Limit scan to project type (`all`, `rust`, `node`, `python`, `dotnet`) |
| `-f, --force` | `bool` | `false` | Bypass confirmation prompts and clean immediately |
| `--tui` | `bool` | `false` | Launch fullscreen interactive Terminal User Interface (TUI) |
| `-v, --verbose` | `bool` | `false` | Display detailed inspection logs and errors |

---

## 🏗️ Architecture & Extensibility

`sweep-rs` is built around a decoupled, plugin-based detector architecture:

```
src/
├── main.rs                   # Entrypoint & CLI execution
├── lib.rs                    # Reusable library interface
├── cli/                      # Command-line argument specifications (Clap)
├── core/
│   ├── traits.rs             # ProjectDetector trait & domain models
│   ├── engine.rs             # Parallel jwalk traversal & Early Pruning
│   ├── size.rs               # 4KB block-size cluster allocation calculator
│   └── cleaner.rs            # O(1) Atomic Rename & async background purge
├── detectors/                # Ecosystem detection plugins
│   ├── rust.rs               # Rust (Cargo.toml -> target)
│   ├── node.rs               # Node.js (package.json -> node_modules)
│   ├── python.rs             # Python (pyproject.toml -> .venv, __pycache__)
│   └── dotnet.rs             # .NET (*.csproj -> bin/, obj/)
├── git/                      # Git metadata introspection (libgit2)
└── ui/                       # ANSI table formatter & Ratatui TUI dashboard
```

### Adding a Custom Project Detector
To add support for another ecosystem (e.g. Go, Java, Elixir), implement the `ProjectDetector` trait:

```rust
use std::path::{Path, PathBuf};
use sweep_rs::core::traits::{ArtifactTarget, ProjectDetector, ProjectType};

pub struct GoDetector;

impl ProjectDetector for GoDetector {
    fn name(&self) -> ProjectType {
        ProjectType::Custom("Go".to_string())
    }

    fn detect(&self, dir: &Path) -> bool {
        dir.join("go.mod").is_file()
    }

    fn get_artifacts(&self, _project_root: &Path) -> Vec<ArtifactTarget> {
        vec![ArtifactTarget {
            name: "bin",
            rel_path: PathBuf::from("bin"),
            is_reconstructible: true,
        }]
    }
}
```

Register your detector in `DetectorRegistry` and it will automatically participate in parallel early-pruned scans!

---

## 📄 License

This project is licensed under the [MIT License](LICENSE).
