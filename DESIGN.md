# Dotfiles-rs Design Document

## Core Concept: Self-Contained Dotfiles Binary

Dotfiles-rs is a utility that manages configuration files between a repository and the user's system. Its unique feature is the ability to create a self-contained binary that includes both the management code and the actual dotfiles themselves.

## Implementation Strategy

### 1. Build-time File Embedding with build.rs

The core of this design uses Rust's build script system (`build.rs`) to handle embedding dotfiles at compile time:

```rust
// build.rs
use std::env;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use toml::Value;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    
    // Paths
    let home = env::var("HOME").expect("HOME environment variable not set");
    let dotfiles_dir = format!("{}/repos/dotfiles", home);
    let distribution_path = format!("{}/distribution.toml", dotfiles_dir);
    
    // Read distribution.toml
    println!("cargo:rerun-if-changed={}", distribution_path);
    let distribution_content = fs::read_to_string(&distribution_path)
        .expect("Failed to read distribution.toml");
    
    // Parse TOML
    let distribution: Value = toml::from_str(&distribution_content)
        .expect("Failed to parse distribution.toml");

    // Create output file for file mappings
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let mut file_map = File::create(format!("{}/embedded_files.rs", out_dir))
        .expect("Failed to create output file");
    
    // Write header
    writeln!(file_map, "// Auto-generated file mapping for embedded dotfiles").unwrap();
    writeln!(file_map, "use std::collections::HashMap;").unwrap();
    writeln!(file_map, "use once_cell::sync::Lazy;").unwrap();
    writeln!(file_map, "").unwrap();
    
    // Embed distribution.toml itself
    writeln!(file_map, "pub const DISTRIBUTION_TOML: &[u8] = include_bytes!(\"{}\");", distribution_path).unwrap();
    
    // Start files map
    writeln!(file_map, "pub static EMBEDDED_FILES: Lazy<HashMap<String, &'static [u8]>> = Lazy::new(|| {{").unwrap();
    writeln!(file_map, "    let mut map = HashMap::new();").unwrap();
    
    // Add distribution.toml to the map
    writeln!(file_map, "    map.insert(\"distribution.toml\".to_string(), DISTRIBUTION_TOML);").unwrap();
    
    // Process each section in distribution.toml
    if let Value::Table(sections) = distribution {
        for (section_name, section_data) in sections {
            if let Value::Table(table) = section_data {
                if let Some(Value::Array(files)) = table.get("files") {
                    for file_value in files {
                        if let Value::String(file) = file_value {
                            let file_path = format!("{}/config/{}/{}", dotfiles_dir, section_name, file);
                            let map_key = format!("config/{}/{}", section_name, file);
                            
                            // Check if file exists
                            if Path::new(&file_path).exists() {
                                println!("cargo:rerun-if-changed={}", file_path);
                                
                                // Include the file and add to map
                                let const_name = format!("FILE_{}", map_key.replace("/", "_").replace(".", "_"));
                                writeln!(file_map, "    const {}: &[u8] = include_bytes!(\"{}\");", const_name, file_path).unwrap();
                                writeln!(file_map, "    map.insert(\"{}\".to_string(), {});", map_key, const_name).unwrap();
                            } else {
                                println!("cargo:warning=File not found: {}", file_path);
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Close the map
    writeln!(file_map, "    map").unwrap();
    writeln!(file_map, "}});").unwrap();
}
```

### 2. Auto-detection of Embedded Files

The binary automatically detects whether it has embedded files and uses them without requiring any special flags:

```rust
// In main.rs, detecting embedded files
fn has_embedded_files() -> bool {
    // Check if EMBEDDED_FILES contains any entries
    !EMBEDDED_FILES.is_empty()
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    
    let mut app = if has_embedded_files() {
        App::from_embedded()?
    } else {
        App::new()?
    };
    
    app.run(&cli.command)?;
    
    Ok(())
}
```

### 3. File Access Strategy

The system uses a unified interface for file access that transparently selects between embedded and filesystem sources:

```rust
enum FileSource {
    Filesystem(PathBuf),
    Embedded(String),
}

struct FileManager {
    source: FileSource,
}

impl FileManager {
    fn read_file(&self, path: &str) -> Result<Vec<u8>> {
        match &self.source {
            FileSource::Filesystem(base_path) => {
                let full_path = base_path.join(path);
                fs::read(&full_path).map_err(|e| anyhow!("Failed to read {}: {}", full_path.display(), e))
            },
            FileSource::Embedded(prefix) => {
                let full_path = format!("{}/{}", prefix, path);
                EMBEDDED_FILES.get(full_path.as_str())
                    .map(|bytes| bytes.to_vec())
                    .ok_or_else(|| anyhow!("Embedded file not found: {}", full_path))
            }
        }
    }
    
    // Similar implementations for exists(), write_file(), etc.
}
```

## Key Design Decisions

### 1. Compile-time File Inclusion

Files are embedded at compile time rather than runtime, making the binary completely self-contained:

- No need for external files or extraction
- Files are directly accessible from memory
- Minimal dependencies required

### 2. Automatic Mode Selection

The program automatically selects between embedded and filesystem modes:

- No user flags or configuration required
- Same commands work regardless of how the binary was built
- Fallback to filesystem if embedded files aren't available

### 3. Command Behavior in Embedded Mode

| Command | Behavior in Embedded Mode |
|---------|---------------------------|
| `install` | Extracts files from the binary to the filesystem |
| `status` | Compares embedded files with filesystem |
| `sync` | Not supported (embedded files are immutable) |

### 4. Handling distribution.toml

The distribution.toml file serves multiple purposes:

1. During compilation: Determines which files to embed in the binary
2. During runtime: Dictates which files to install/check and where they belong
3. In embedded mode: Is itself embedded and read from memory

## Usage Workflow

1. **Development Workflow**:
   - Manage dotfiles in a repository with the normal binary
   - Use `sync`, `status`, etc. freely with the filesystem

2. **Deployment Workflow**:
   - Build the self-contained binary once
   - Distribute only this binary to target systems
   - Run `dotfiles install` to deploy all configurations

3. **Update Workflow**:
   - Make changes to the source repository
   - Rebuild the binary to incorporate changes
   - Redeploy the updated binary

## Benefits of This Design

1. **Simplicity**: Users run the same commands regardless of mode
2. **Portability**: Single binary contains everything needed
3. **Privacy**: Configuration files are compiled into the binary, not easily extractable
4. **Reliability**: No dependencies on external repositories or connections
5. **Flexibility**: Can function with or without embedded files

## Implementation Notes

1. The `build.rs` script needs careful error handling for missing files
2. Command implementations need to handle the limitation of embedded mode appropriately
3. Clear error messages should be provided when sync is attempted in embedded mode
4. File path handling must be consistent between embedded and filesystem modes