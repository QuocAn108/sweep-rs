pub mod cpp;
pub mod dotnet;
pub mod elixir;
pub mod flutter;
pub mod go;
pub mod java;
pub mod node;
pub mod php;
pub mod python;
pub mod ruby;
pub mod rust;
pub mod swift;

use self::cpp::CppDetector;
use self::dotnet::DotnetDetector;
use self::elixir::ElixirDetector;
use self::flutter::FlutterDetector;
use self::go::GoDetector;
use self::java::JavaDetector;
use self::node::NodeDetector;
use self::php::PhpDetector;
use self::python::PythonDetector;
use self::ruby::RubyDetector;
use self::rust::RustDetector;
use self::swift::SwiftDetector;
use crate::core::traits::ProjectDetector;
use std::path::Path;

pub struct DetectorRegistry {
    detectors: Vec<Box<dyn ProjectDetector>>,
}

impl DetectorRegistry {
    pub fn new() -> Self {
        Self {
            detectors: Vec::new(),
        }
    }

    pub fn register(mut self, detector: Box<dyn ProjectDetector>) -> Self {
        self.detectors.push(detector);
        self
    }

    pub fn default_all() -> Self {
        Self::new()
            .register(Box::new(RustDetector::new()))
            .register(Box::new(NodeDetector::new()))
            .register(Box::new(PythonDetector::new()))
            .register(Box::new(DotnetDetector::new()))
            .register(Box::new(JavaDetector::new()))
            .register(Box::new(GoDetector::new()))
            .register(Box::new(FlutterDetector::new()))
            .register(Box::new(PhpDetector::new()))
            .register(Box::new(RubyDetector::new()))
            .register(Box::new(CppDetector::new()))
            .register(Box::new(SwiftDetector::new()))
            .register(Box::new(ElixirDetector::new()))
    }

    pub fn for_type(filter: &str) -> Result<Self, String> {
        match filter.to_lowercase().as_str() {
            "all" => Ok(Self::default_all()),
            "rust" => Ok(Self::new().register(Box::new(RustDetector::new()))),
            "node" | "nodejs" | "javascript" | "js" | "typescript" | "ts" => {
                Ok(Self::new().register(Box::new(NodeDetector::new())))
            }
            "python" | "py" => Ok(Self::new().register(Box::new(PythonDetector::new()))),
            "dotnet" | ".net" | "csharp" | "cs" => {
                Ok(Self::new().register(Box::new(DotnetDetector::new())))
            }
            "java" | "jvm" => Ok(Self::new().register(Box::new(JavaDetector::new()))),
            "go" | "golang" => Ok(Self::new().register(Box::new(GoDetector::new()))),
            "flutter" | "dart" => Ok(Self::new().register(Box::new(FlutterDetector::new()))),
            "php" | "composer" => Ok(Self::new().register(Box::new(PhpDetector::new()))),
            "ruby" | "rb" | "rails" => Ok(Self::new().register(Box::new(RubyDetector::new()))),
            "cpp" | "c" | "c++" | "cmake" => Ok(Self::new().register(Box::new(CppDetector::new()))),
            "swift" | "apple" | "xcode" => Ok(Self::new().register(Box::new(SwiftDetector::new()))),
            "elixir" | "ex" | "mix" => Ok(Self::new().register(Box::new(ElixirDetector::new()))),
            other => Err(format!(
                "Unknown project type '{}'. Available types: all, rust, node, python, dotnet, java, go, flutter, php, ruby, cpp, swift, elixir",
                other
            )),
        }
    }

    #[allow(dead_code)]
    pub fn detectors(&self) -> &[Box<dyn ProjectDetector>] {
        &self.detectors
    }

    pub fn detect_all<'a>(&'a self, dir: &Path) -> Vec<&'a dyn ProjectDetector> {
        self.detectors
            .iter()
            .map(|d| d.as_ref())
            .filter(|d| d.detect(dir))
            .collect()
    }
}

impl Default for DetectorRegistry {
    fn default() -> Self {
        Self::default_all()
    }
}
