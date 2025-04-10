// No top-level imports needed

mod filepaths {
    use anyhow::Result;
    use dirs::home_dir;
    use std::path::PathBuf;
    use crate::DotfilesError;

    pub struct FilePaths {
        pub repo_dir: PathBuf,
        pub config_dir: PathBuf,
        pub distribution_file: PathBuf,
        pub dotignore_file: PathBuf,
    }
    
    impl FilePaths {
        pub fn new() -> Result<Self> {
            let home = home_dir().ok_or_else(|| DotfilesError::RepoNotFound("Home directory not found".to_string()))?;
            
            let repo_dir = home.join("repos").join("dotfiles");
            let config_dir = home.join(".config");
            let distribution_file = repo_dir.join("distribution.toml");
            let dotignore_file = repo_dir.join(".dotignore");
            
            Ok(Self {
                repo_dir,
                config_dir,
                distribution_file,
                dotignore_file,
            })
        }
        
        pub fn repo_config_dir(&self, section: &str) -> PathBuf {
            self.repo_dir.join("config").join(section)
        }
        
        pub fn config_section_dir(&self, section: &str) -> PathBuf {
            self.config_dir.join(section)
        }
        
        pub fn repo_file_path(&self, section: &str, file: &str) -> PathBuf {
            self.repo_config_dir(section).join(file)
        }
        
        pub fn config_file_path(&self, section: &str, file: &str) -> PathBuf {
            self.config_section_dir(section).join(file)
        }
    }
}

mod distribution {
    use anyhow::{Context, Result};
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use crate::DotfilesError;
    use crate::DotfilesArchive;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Distribution {
        #[serde(flatten)]
        pub sections: HashMap<String, Section>,
    }
    
    #[derive(Debug, Serialize, Deserialize)]
    pub struct Section {
        #[serde(default)]
        pub files: Vec<String>,
    }
    
    pub enum DistributionSource {
        File(PathBuf),
        Embedded,
    }
    
    pub struct DistributionParser {
        pub source: DistributionSource,
    }
    
    impl DistributionParser {
        pub fn new(path: PathBuf) -> Self {
            Self { source: DistributionSource::File(path) }
        }
        
        pub fn from_embedded() -> Self {
            Self { source: DistributionSource::Embedded }
        }
        
        pub fn read_distribution(&self) -> Result<Distribution> {
            let content = match &self.source {
                DistributionSource::File(path) => fs::read_to_string(path)
                    .context("Failed to read distribution file")?,
                DistributionSource::Embedded => DotfilesArchive::get_distribution()?,
            };
            
            let distribution: Distribution = toml::from_str(&content)
                .map_err(|e| DotfilesError::DistributionParseError(e.to_string()))?;
            
            Ok(distribution)
        }
        
        pub fn get_tools(&self) -> Result<Vec<String>> {
            let distribution = self.read_distribution()?;
            Ok(distribution.sections.keys().cloned().collect())
        }
        
        pub fn get_files(&self, tool: &str) -> Result<Vec<String>> {
            let distribution = self.read_distribution()?;
            
            match distribution.sections.get(tool) {
                Some(section_data) => Ok(section_data.files.clone()),
                None => Ok(Vec::new()),
            }
        }
        
        pub fn add_file(&self, tool: &str, file: &str) -> Result<()> {
            let mut distribution = self.read_distribution().unwrap_or_else(|_| Distribution {
                sections: HashMap::new(),
            });
            
            // Create tool section if it doesn't exist
            let section_entry = distribution.sections.entry(tool.to_string())
                .or_insert_with(|| Section { files: Vec::new() });
            
            // Add file if it doesn't already exist
            if !section_entry.files.contains(&file.to_string()) {
                section_entry.files.push(file.to_string());
            }
            
            // Write back to file
            let toml_content = toml::to_string(&distribution)
                .map_err(|e| DotfilesError::DistributionParseError(format!("Failed to serialize: {}", e)))?;
            
            match &self.source {
                DistributionSource::File(path) => fs::write(path, toml_content)?,
                DistributionSource::Embedded => return Err(DotfilesError::InvalidCommand(
                    "Cannot modify distribution file in embedded mode".to_string()).into()),
            }
            
            Ok(())
        }
        
        pub fn remove_file(&self, tool: &str, file: &str) -> Result<()> {
            let mut distribution = self.read_distribution()?;
            
            // Check if tool section exists
            if let Some(section_data) = distribution.sections.get_mut(tool) {
                // Remove file if it exists
                section_data.files.retain(|f| f != file);
                
                // Write back to file
                let toml_content = toml::to_string(&distribution)
                    .map_err(|e| DotfilesError::DistributionParseError(format!("Failed to serialize: {}", e)))?;
                
                match &self.source {
                    DistributionSource::File(path) => fs::write(path, toml_content)?,
                    DistributionSource::Embedded => return Err(DotfilesError::InvalidCommand(
                        "Cannot modify distribution file in embedded mode".to_string()).into()),
                }
                
                Ok(())
            } else {
                Err(DotfilesError::InvalidCommand(format!("Tool '{}' not found", tool)).into())
            }
        }
    }
}

mod ignore {
    use anyhow::Result;
    use glob::Pattern;
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use crate::DotfilesArchive;
    
    pub enum DotIgnoreSource {
        File(PathBuf),
        Embedded,
    }
    
    pub struct DotIgnore {
        pub patterns: Vec<Pattern>,
    }
    
    impl DotIgnore {
        pub fn new(path: &Path) -> Result<Self> {
            Self::from_source(DotIgnoreSource::File(path.to_path_buf()))
        }
        
        pub fn from_embedded() -> Result<Self> {
            Self::from_source(DotIgnoreSource::Embedded)
        }
        
        pub fn from_source(source: DotIgnoreSource) -> Result<Self> {
            let mut patterns = Vec::new();
            
            let content = match source {
                DotIgnoreSource::File(path) => {
                    if path.exists() {
                        fs::read_to_string(&path)?
                    } else {
                        Self::default_content().to_string()
                    }
                },
                DotIgnoreSource::Embedded => {
                    DotfilesArchive::get_dotignore().unwrap_or_else(|_| Self::default_content().to_string())
                }
            };
            
            for line in content.lines() {
                let line = line.trim();
                if !line.is_empty() && !line.starts_with('#') {
                    patterns.push(Pattern::new(line)?);
                }
            }
            
            Ok(Self { patterns })
        }
        
        pub fn default_content() -> &'static str {
            r#"# Add files to ignore when syncing
# Each line is a glob pattern matched against the basename of files
*history
*_history
*id_rsa*
*authorized_keys*
*known_hosts*
*htop
*netrc
*oauth*
*robrc
*token*
*.cert
*.key
*.pem
*.crt
*credentials*
*client_secret*
"#
        }
        
        pub fn create_default(path: &Path) -> Result<()> {
            if !path.exists() {
                let mut file = File::create(path)?;
                file.write_all(Self::default_content().as_bytes())?;
            }
            
            Ok(())
        }
        
        pub fn is_ignored(&self, filename: &str) -> bool {
            let basename = Path::new(filename).file_name()
                .and_then(|os_str| os_str.to_str())
                .unwrap_or("");
                
            self.patterns.iter().any(|pattern| pattern.matches(basename))
        }
    }
}

// Re-exports for use in main.rs
pub use filepaths::FilePaths;
pub use distribution::{Distribution, DistributionParser};
pub use ignore::DotIgnore;