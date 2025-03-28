use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

// Note: Add this to Cargo.toml:
// [build-dependencies]
// toml = "0.8"
use toml::Value;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    
    // Paths
    let home = match env::var("HOME") {
        Ok(h) => h,
        Err(_) => {
            // If HOME isn't available, we just build without embedded files
            println!("cargo:warning=HOME environment variable not set, building without embedded files");
            return;
        }
    };
    
    let dotfiles_dir = format!("{}/repos/dotfiles", home);
    let distribution_path = format!("{}/distribution.toml", dotfiles_dir);
    let dotignore_path = format!("{}/.dotignore", dotfiles_dir);
    
    // Check if distribution file exists
    if !Path::new(&distribution_path).exists() {
        println!("cargo:warning=distribution.toml not found at {}, building without embedded files", distribution_path);
        return;
    }
    
    println!("cargo:rerun-if-changed={}", distribution_path);
    
    // Read distribution.toml
    let distribution_content = match fs::read_to_string(&distribution_path) {
        Ok(content) => content,
        Err(e) => {
            println!("cargo:warning=Failed to read distribution.toml: {}", e);
            return;
        }
    };
    
    // Parse TOML
    let distribution: Value = match toml::from_str(&distribution_content) {
        Ok(parsed) => parsed,
        Err(e) => {
            println!("cargo:warning=Failed to parse distribution.toml: {}", e);
            return;
        }
    };

    // Create output directory if needed
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    
    // Create output file for file mappings
    let mut file_map = match File::create(format!("{}/embedded_files.rs", out_dir)) {
        Ok(file) => file,
        Err(e) => {
            println!("cargo:warning=Failed to create output file: {}", e);
            return;
        }
    };
    
    // Write header
    writeln!(file_map, "// Auto-generated file mapping for embedded dotfiles").unwrap();
    writeln!(file_map, "use once_cell::sync::Lazy;").unwrap();
    writeln!(file_map, "").unwrap();
    
    // Define function to check if we have embedded files
    writeln!(file_map, "pub fn has_embedded_files() -> bool {{").unwrap();
    writeln!(file_map, "    !EMBEDDED_FILES.is_empty()").unwrap();
    writeln!(file_map, "}}").unwrap();
    writeln!(file_map, "").unwrap();
    
    // Embed distribution.toml itself
    writeln!(file_map, "pub const DISTRIBUTION_TOML: &[u8] = include_bytes!(\"{}\");", distribution_path).unwrap();
    
    // Embed dotignore if it exists
    if Path::new(&dotignore_path).exists() {
        println!("cargo:rerun-if-changed={}", dotignore_path);
        writeln!(file_map, "pub const DOTIGNORE: &[u8] = include_bytes!(\"{}\");", dotignore_path).unwrap();
        writeln!(file_map, "pub const HAS_DOTIGNORE: bool = true;").unwrap();
    } else {
        writeln!(file_map, "pub const DOTIGNORE: &[u8] = &[];").unwrap();
        writeln!(file_map, "pub const HAS_DOTIGNORE: bool = false;").unwrap();
    }
    writeln!(file_map, "").unwrap();
    
    // Start files map
    writeln!(file_map, "pub static EMBEDDED_FILES: Lazy<HashMap<String, &'static [u8]>> = Lazy::new(|| {{").unwrap();
    writeln!(file_map, "    let mut map = HashMap::new();").unwrap();
    
    // Add distribution.toml to the map
    writeln!(file_map, "    map.insert(\"distribution.toml\".to_string(), DISTRIBUTION_TOML);").unwrap();
    
    // Add dotignore to the map if it exists
    if Path::new(&dotignore_path).exists() {
        writeln!(file_map, "    map.insert(\".dotignore\".to_string(), DOTIGNORE);").unwrap();
    }
    
    // Process each section in distribution.toml
    let mut embedded_count = 0;
    
    // Clone distribution for embedding files
    let distribution_clone = distribution.clone();
    
    if let Value::Table(sections) = distribution_clone {
        for (section_name, section_data) in sections {
            // Skip sections that start with underscore (convention for metadata)
            if section_name.starts_with('_') {
                continue;
            }
            
            if let Value::Table(table) = section_data {
                if let Some(Value::Array(files)) = table.get("files") {
                    for file_value in files {
                        if let Value::String(file) = file_value {
                            let file_path = format!("{}/config/{}/{}", dotfiles_dir, section_name, file);
                            let map_key = format!("config/{}/{}", section_name, file);
                            
                            // Check if file exists
                            if Path::new(&file_path).exists() {
                                println!("cargo:rerun-if-changed={}", file_path);
                                
                                // Create a safe constant name by removing all problematic characters
                                // Using uppercase for constants to follow Rust conventions
                                let const_name = format!("FILE_{}", 
                                    map_key.replace(|c: char| !c.is_alphanumeric() && c != '_', "_").to_uppercase());
                                
                                // Include the file and add to map
                                writeln!(file_map, "    const {}: &[u8] = include_bytes!(\"{}\");", 
                                    const_name, file_path).unwrap();
                                writeln!(file_map, "    map.insert(\"{}\".to_string(), {});", 
                                    map_key, const_name).unwrap();
                                
                                embedded_count += 1;
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
    
    println!("cargo:warning=Embedded {} files from distribution.toml into the binary", embedded_count);
    
    // Find the newest file timestamp to use as build identity
    let mut newest_timestamp = 0;
    let mut newest_file = String::new();
    
    // Check timestamp of distribution.toml itself
    if let Ok(metadata) = std::fs::metadata(&distribution_path) {
        if let Ok(modified_time) = metadata.modified() {
            if let Ok(system_time) = modified_time.duration_since(std::time::SystemTime::UNIX_EPOCH) {
                let timestamp = system_time.as_secs();
                if timestamp > newest_timestamp {
                    newest_timestamp = timestamp;
                    newest_file = distribution_path.clone();
                }
            }
        }
    }
    
    // Process all embedded files - use a fresh parse to avoid ownership issues
    let distribution_for_timestamp = match fs::read_to_string(&distribution_path) {
        Ok(content) => match toml::from_str::<Value>(&content) {
            Ok(parsed) => parsed,
            Err(_) => Value::Table(toml::map::Map::new()),
        },
        Err(_) => Value::Table(toml::map::Map::new()),
    };
    
    if let Value::Table(sections) = distribution_for_timestamp {
        for (section_name, section_data) in sections {
            if section_name.starts_with('_') {
                continue;
            }
            
            if let Value::Table(table) = section_data {
                if let Some(Value::Array(files)) = table.get("files") {
                    for file_value in files {
                        if let Value::String(file) = file_value {
                            let file_path = format!("{}/config/{}/{}", dotfiles_dir, section_name, file);
                            
                            if let Ok(metadata) = std::fs::metadata(&file_path) {
                                if let Ok(modified_time) = metadata.modified() {
                                    if let Ok(system_time) = modified_time.duration_since(std::time::SystemTime::UNIX_EPOCH) {
                                        let timestamp = system_time.as_secs();
                                        if timestamp > newest_timestamp {
                                            newest_timestamp = timestamp;
                                            newest_file = file_path.clone();
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Convert timestamp to formatted datetime string
    if newest_timestamp > 0 {
        use std::time::{UNIX_EPOCH, Duration};
        use chrono::{DateTime, Utc, Datelike, Timelike};
        
        let dt = DateTime::<Utc>::from(UNIX_EPOCH + Duration::from_secs(newest_timestamp));
        let week_num = dt.iso_week().week();
        
        let build_identity = format!("{:04}{:02}{:02}-{:02}-{:02}{:02}{:02}",
            dt.year(), dt.month(), dt.day(),
            week_num,
            dt.hour(), dt.minute(), dt.second());
            
        println!("cargo:rustc-env=BUILD_IDENTITY={}", build_identity);
        println!("cargo:rustc-env=NEWEST_FILE={}", newest_file);
    } else {
        println!("cargo:rustc-env=BUILD_IDENTITY=00000000-00-000000");
        println!("cargo:rustc-env=NEWEST_FILE=unknown");
    }
}