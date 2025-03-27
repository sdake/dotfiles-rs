use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use dirs::home_dir;
use glob::Pattern;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, create_dir_all, File};
use std::io::{Write};
use std::path::{Path, PathBuf};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};
use thiserror::Error;

// Custom error types
#[derive(Error, Debug)]
enum DotfilesError {
    #[error("Repository not found: {0}")]
    RepoNotFound(String),
    
    #[error("Distribution file not found: {0}")]
    DistributionNotFound(String),
    
    #[error("Failed to parse distribution file: {0}")]
    DistributionParseError(String),
    
    #[error("File not found: {0}")]
    FileNotFound(String),
    
    #[error("Invalid command format: {0}")]
    InvalidCommand(String),
}

// Status symbols
const CHECK_MARK: &str = "✓";
const CROSS_MARK: &str = "✗";
const WARNING_MARK: &str = "⚠";
const INFO_MARK: &str = "ℹ";
const ARROW_MARK: &str = "→";

// Command line arguments
#[derive(Parser)]
#[clap(
    name = "dotfiles-rs",
    about = "Manages dotfiles between system configuration directories and git repository",
    version = "0.1.0"
)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Sync files from $HOME/.config to repository
    Sync,
    
    /// Show status of files in distribution.toml
    Status,
    
    /// Install files from repository to $HOME/.config
    Install,
    
    /// Add a file to distribution.toml and copy to repo
    Add {
        /// The tool name (directory under .config)
        tool: String,
        
        /// The file name to add
        file: String,
    },
    
    /// Remove a file from distribution.toml
    Remove {
        /// The tool name (directory under .config)
        tool: String,
        
        /// The file name to remove
        file: String,
    },
    
    /// Check that distribution.toml exists and has valid syntax
    Precheck,
    
    /// Show usage information
    Usage,
}

// Output formatter helper
struct Formatter {
    stdout: StandardStream,
}

impl Formatter {
    fn new() -> Self {
        Self {
            stdout: StandardStream::stdout(ColorChoice::Auto),
        }
    }
    
    fn print(&mut self, message: &str, color: Option<Color>, bold: bool) -> Result<()> {
        let mut color_spec = ColorSpec::new();
        if let Some(c) = color {
            color_spec.set_fg(Some(c));
        }
        color_spec.set_bold(bold);
        
        self.stdout.set_color(&color_spec)?;
        write!(self.stdout, "{}", message)?;
        self.stdout.reset()?;
        Ok(())
    }
    
    
    fn success(&mut self, message: &str) -> Result<()> {
        self.print(&format!("{} ", CHECK_MARK), Some(Color::Green), false)?;
        self.print(message, None, false)?;
        writeln!(self.stdout)?;
        Ok(())
    }
    
    fn warning(&mut self, message: &str) -> Result<()> {
        self.print(&format!("{} ", WARNING_MARK), Some(Color::Yellow), false)?;
        self.print(message, None, false)?;
        writeln!(self.stdout)?;
        Ok(())
    }
    
    fn error(&mut self, message: &str) -> Result<()> {
        self.print(&format!("{} ", CROSS_MARK), Some(Color::Red), false)?;
        self.print(message, None, false)?;
        writeln!(self.stdout)?;
        Ok(())
    }
    
    fn info(&mut self, message: &str) -> Result<()> {
        self.print(&format!("{} ", INFO_MARK), Some(Color::Blue), false)?;
        self.print(message, None, false)?;
        writeln!(self.stdout)?;
        Ok(())
    }
    
    fn modified(&mut self, message: &str) -> Result<()> {
        self.print(&format!("{} ", ARROW_MARK), Some(Color::Magenta), false)?;
        self.print(message, None, false)?;
        writeln!(self.stdout)?;
        Ok(())
    }
    
    fn header(&mut self, message: &str) -> Result<()> {
        self.print(message, None, true)?;
        writeln!(self.stdout)?;
        Ok(())
    }
}

// Paths helper
struct Paths {
    repo_dir: PathBuf,
    config_dir: PathBuf,
    distribution_file: PathBuf,
    dotignore_file: PathBuf,
}

impl Paths {
    fn new() -> Result<Self> {
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
    
    fn repo_config_dir(&self, section: &str) -> PathBuf {
        self.repo_dir.join("config").join(section)
    }
    
    fn config_section_dir(&self, section: &str) -> PathBuf {
        self.config_dir.join(section)
    }
    
    fn repo_file_path(&self, section: &str, file: &str) -> PathBuf {
        self.repo_config_dir(section).join(file)
    }
    
    fn config_file_path(&self, section: &str, file: &str) -> PathBuf {
        self.config_section_dir(section).join(file)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Distribution {
    #[serde(flatten)]
    sections: HashMap<String, Section>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Section {
    #[serde(default)]
    files: Vec<String>,
}

// DistributionParser
struct DistributionParser {
    path: PathBuf,
}

impl DistributionParser {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
    
    fn read_distribution(&self) -> Result<Distribution> {
        let content = fs::read_to_string(&self.path)
            .context("Failed to read distribution file")?;
        
        let distribution: Distribution = toml::from_str(&content)
            .map_err(|e| DotfilesError::DistributionParseError(e.to_string()))?;
        
        Ok(distribution)
    }
    
    fn get_tools(&self) -> Result<Vec<String>> {
        let distribution = self.read_distribution()?;
        Ok(distribution.sections.keys().cloned().collect())
    }
    
    fn get_files(&self, tool: &str) -> Result<Vec<String>> {
        let distribution = self.read_distribution()?;
        
        match distribution.sections.get(tool) {
            Some(section_data) => Ok(section_data.files.clone()),
            None => Ok(Vec::new()),
        }
    }
    
    fn add_file(&self, tool: &str, file: &str) -> Result<()> {
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
        
        fs::write(&self.path, toml_content)?;
        Ok(())
    }
    
    fn remove_file(&self, tool: &str, file: &str) -> Result<()> {
        let mut distribution = self.read_distribution()?;
        
        // Check if tool section exists
        if let Some(section_data) = distribution.sections.get_mut(tool) {
            // Remove file if it exists
            section_data.files.retain(|f| f != file);
            
            // Write back to file
            let toml_content = toml::to_string(&distribution)
                .map_err(|e| DotfilesError::DistributionParseError(format!("Failed to serialize: {}", e)))?;
            
            fs::write(&self.path, toml_content)?;
            Ok(())
        } else {
            Err(DotfilesError::InvalidCommand(format!("Tool '{}' not found", tool)).into())
        }
    }
}

// DotIgnore parser
struct DotIgnore {
    patterns: Vec<Pattern>,
}

impl DotIgnore {
    fn new(path: &Path) -> Result<Self> {
        let mut patterns = Vec::new();
        
        if path.exists() {
            let content = fs::read_to_string(path)?;
            for line in content.lines() {
                let line = line.trim();
                if !line.is_empty() && !line.starts_with('#') {
                    patterns.push(Pattern::new(line)?);
                }
            }
        }
        
        Ok(Self { patterns })
    }
    
    fn create_default(path: &Path) -> Result<()> {
        if !path.exists() {
            let default_content = r#"# Add files to ignore when syncing
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
"#;
            let mut file = File::create(path)?;
            file.write_all(default_content.as_bytes())?;
        }
        
        Ok(())
    }
    
    fn is_ignored(&self, filename: &str) -> bool {
        let basename = Path::new(filename).file_name()
            .and_then(|os_str| os_str.to_str())
            .unwrap_or("");
            
        self.patterns.iter().any(|pattern| pattern.matches(basename))
    }
}

// FileManager handles file operations
struct FileManager<'a> {
    paths: &'a Paths,
    formatter: &'a mut Formatter,
    dotignore: &'a DotIgnore,
}

impl<'a> FileManager<'a> {
    fn new(paths: &'a Paths, formatter: &'a mut Formatter, dotignore: &'a DotIgnore) -> Self {
        Self {
            paths,
            formatter,
            dotignore,
        }
    }
    
    fn install_file(&mut self, section: &str, file: &str) -> Result<()> {
        let repo_file = self.paths.repo_file_path(section, file);
        let config_file = self.paths.config_file_path(section, file);
        let display_path = format!("{}/{}", section, file);
        
        if self.dotignore.is_ignored(file) {
            self.formatter.warning(&format!("Ignored by .dotignore: {}", display_path))?;
            return Ok(());
        }
        
        if repo_file.exists() {
            if let Some(parent) = config_file.parent() {
                create_dir_all(parent)?;
            }
            
            fs::copy(&repo_file, &config_file)?;
            self.formatter.success(&format!("Installed to local: {}", display_path))?;
        } else {
            self.formatter.warning(&format!("Repo file not found: {}", display_path))?;
        }
        
        Ok(())
    }
    
    fn sync_file(&mut self, section: &str, file: &str) -> Result<()> {
        let repo_file = self.paths.repo_file_path(section, file);
        let config_file = self.paths.config_file_path(section, file);
        let display_path = format!("{}/{}", section, file);
        
        if self.dotignore.is_ignored(file) {
            self.formatter.warning(&format!("Ignored by .dotignore: {}", display_path))?;
            return Ok(());
        }
        
        if config_file.exists() {
            if let Some(parent) = repo_file.parent() {
                create_dir_all(parent)?;
            }
            
            fs::copy(&config_file, &repo_file)?;
            self.formatter.success(&format!("Synced to repo: {}", display_path))?;
        } else {
            self.formatter.warning(&format!("Local file not found: {}", display_path))?;
        }
        
        Ok(())
    }
    
    fn check_status(&mut self, section: &str, file: &str) -> Result<()> {
        let repo_file = self.paths.repo_file_path(section, file);
        let config_file = self.paths.config_file_path(section, file);
        let display_path = format!("{}/{}", section, file);
        
        if self.dotignore.is_ignored(file) {
            self.formatter.warning(&format!("Ignored by .dotignore: {}", display_path))?;
            return Ok(());
        }
        
        if !repo_file.exists() {
            self.formatter.error(&format!("Missing in repo: {}", display_path))?;
            return Ok(());
        }
        
        if !config_file.exists() {
            self.formatter.warning(&format!("Not installed: {}", display_path))?;
            return Ok(());
        }
        
        // Compare files
        let repo_content = fs::read(&repo_file)?;
        let config_content = fs::read(&config_file)?;
        
        if repo_content == config_content {
            self.formatter.success(&format!("Identical: {}", display_path))?;
        } else {
            self.formatter.modified(&format!("Modified locally: {}", display_path))?;
        }
        
        Ok(())
    }
    
    fn add_file(&mut self, section: &str, file: &str) -> Result<()> {
        let source_dir = self.paths.config_section_dir(section);
        let dest_dir = self.paths.repo_config_dir(section);
        let source_file = source_dir.join(file);
        let dest_file = dest_dir.join(file);
        let display_path = format!("{}/{}", section, file);
        
        if !source_file.exists() {
            return Err(DotfilesError::FileNotFound(source_file.to_string_lossy().to_string()).into());
        }
        
        // Create destination directory if needed
        if let Some(parent) = dest_file.parent() {
            create_dir_all(parent)?;
        }
        
        // Add file to distribution.toml
        let parser = DistributionParser::new(self.paths.distribution_file.clone());
        parser.add_file(section, file)?;
        
        // Copy file to repo
        fs::copy(&source_file, &dest_file)?;
        self.formatter.success(&format!("Added to tracking: {}", display_path))?;
        
        Ok(())
    }
    
    fn remove_file(&mut self, section: &str, file: &str) -> Result<()> {
        let repo_file = self.paths.repo_file_path(section, file);
        let display_path = format!("{}/{}", section, file);
        
        // Remove file from distribution.toml
        let parser = DistributionParser::new(self.paths.distribution_file.clone());
        parser.remove_file(section, file)?;
        
        self.formatter.info(&format!("Removed from distribution file: {}", display_path))?;
        
        // Inform user to remove the file manually
        if repo_file.exists() {
            self.formatter.warning(&format!(
                "To complete removal, manually delete the file: {}",
                repo_file.display()
            ))?;
            self.formatter.print("   ", Some(Color::Cyan), false)?;
            self.formatter.print(
                &format!("rm {}", repo_file.display()),
                Some(Color::Cyan),
                false,
            )?;
            writeln!(self.formatter.stdout)?;
        }
        
        Ok(())
    }
}

// App is the main application
struct App {
    paths: Paths,
    formatter: Formatter,
    distribution_parser: DistributionParser,
    dotignore: DotIgnore,
}

impl App {
    fn new() -> Result<Self> {
        let paths = Paths::new()?;
        let formatter = Formatter::new();
        let distribution_parser = DistributionParser::new(paths.distribution_file.clone());
        let dotignore = DotIgnore::new(&paths.dotignore_file)?;
        
        Ok(Self {
            paths,
            formatter,
            distribution_parser,
            dotignore,
        })
    }
    
    fn check_paths(&mut self) -> Result<()> {
        // Check repository directory
        if !self.paths.repo_dir.exists() {
            return Err(DotfilesError::RepoNotFound(
                self.paths.repo_dir.to_string_lossy().to_string(),
            )
            .into());
        }
        
        // Check distribution file
        if !self.paths.distribution_file.exists() {
            return Err(DotfilesError::DistributionNotFound(
                self.paths.distribution_file.to_string_lossy().to_string(),
            )
            .into());
        }
        
        // Create config directory if it doesn't exist
        if !self.paths.config_dir.exists() {
            self.formatter.warning(&format!(
                "Config directory not found, creating: {}",
                self.paths.config_dir.display()
            ))?;
            create_dir_all(&self.paths.config_dir)?;
        }
        
        Ok(())
    }
    
    fn create_dotignore(&self) -> Result<()> {
        DotIgnore::create_default(&self.paths.dotignore_file)?;
        Ok(())
    }
    
    fn process_section(&mut self, tool: &str, action: &str) -> Result<()> {
        let files = self.distribution_parser.get_files(tool)?;
        
        self.formatter
            .info(&format!("Processing tool: {}", tool))?;
        
        let dest_dir = self.paths.config_section_dir(tool);
        if !dest_dir.exists() {
            self.formatter
                .warning(&format!("Creating directory: {}", dest_dir.display()))?;
            create_dir_all(&dest_dir)?;
        }
        
        let mut file_manager = FileManager::new(&self.paths, &mut self.formatter, &self.dotignore);
        
        for file in files {
            match action {
                "install" => file_manager.install_file(tool, &file)?,
                "sync" => file_manager.sync_file(tool, &file)?,
                "status" => file_manager.check_status(tool, &file)?,
                _ => {
                    return Err(DotfilesError::InvalidCommand(format!(
                        "Invalid action: {}",
                        action
                    )).into())
                }
            }
        }
        
        Ok(())
    }
    
    fn run_sync(&mut self) -> Result<()> {
        self.formatter.header("Syncing dotfiles...")?;
        
        let tools = self.distribution_parser.get_tools()?;
        for tool in tools {
            self.process_section(&tool, "sync")?;
        }
        
        Ok(())
    }
    
    fn run_status(&mut self) -> Result<()> {
        self.formatter.header("Checking dotfiles status...")?;
        
        let tools = self.distribution_parser.get_tools()?;
        for tool in tools {
            self.process_section(&tool, "status")?;
        }
        
        Ok(())
    }
    
    fn run_install(&mut self) -> Result<()> {
        self.formatter.header("Installing dotfiles...")?;
        
        let tools = self.distribution_parser.get_tools()?;
        for tool in tools {
            self.process_section(&tool, "install")?;
        }
        
        Ok(())
    }
    
    fn run_add(&mut self, tool: &str, file: &str) -> Result<()> {
        let mut file_manager = FileManager::new(&self.paths, &mut self.formatter, &self.dotignore);
        file_manager.add_file(tool, file)?;
        Ok(())
    }
    
    fn run_remove(&mut self, tool: &str, file: &str) -> Result<()> {
        let mut file_manager = FileManager::new(&self.paths, &mut self.formatter, &self.dotignore);
        file_manager.remove_file(tool, file)?;
        Ok(())
    }
    
    fn run_precheck(&mut self) -> Result<()> {
        self.formatter.header("Checking distribution file...")?;
        
        // Check if distribution file exists
        self.formatter.print("Distribution file: ", Some(Color::Cyan), false)?;
        self.formatter.print(&self.paths.distribution_file.to_string_lossy(), None, false)?;
        writeln!(self.formatter.stdout)?;
        
        if !self.paths.distribution_file.exists() {
            self.formatter.error("Distribution file not found")?;
            return Err(DotfilesError::DistributionNotFound(
                self.paths.distribution_file.to_string_lossy().to_string()).into());
        }
        
        self.formatter.success("Distribution file exists")?;
        
        // Check if it's valid TOML
        self.formatter.print("Checking TOML syntax... ", Some(Color::Cyan), false)?;
        
        let content = fs::read_to_string(&self.paths.distribution_file)?;
        
        // Try to parse the TOML content
        match toml::from_str::<Distribution>(&content) {
            Ok(_) => {
                self.formatter.success("Valid TOML syntax")?;
                
                // Show basic info
                let line_count = content.lines().count();
                self.formatter.print("Line count: ", Some(Color::Cyan), false)?;
                self.formatter.print(&format!("{} lines", line_count), None, false)?;
                writeln!(self.formatter.stdout)?;
                
                let tools = self.distribution_parser.get_tools()?;
                self.formatter.print("Total tools: ", Some(Color::Cyan), false)?;
                self.formatter.print(&format!("{}", tools.len()), None, false)?;
                writeln!(self.formatter.stdout)?;
                
                writeln!(self.formatter.stdout)?;
                self.formatter.success("Precheck passed successfully")?;
            },
            Err(e) => {
                self.formatter.error(&format!("Invalid TOML syntax: {}", e))?;
                return Err(DotfilesError::DistributionParseError(e.to_string()).into());
            }
        }
        
        Ok(())
    }
    
    fn run(&mut self, command: &Commands) -> Result<()> {
        // Check required paths
        self.check_paths()?;
        
        // Create dotignore if it doesn't exist
        self.create_dotignore()?;
        
        match command {
            Commands::Sync => self.run_sync()?,
            Commands::Status => self.run_status()?,
            Commands::Install => self.run_install()?,
            Commands::Add { tool, file } => self.run_add(tool, file)?,
            Commands::Remove { tool, file } => self.run_remove(tool, file)?,
            Commands::Precheck => self.run_precheck()?,
            Commands::Usage => {
                // Print help information
                println!("dotfiles-rs - Manages dotfiles between system configuration and git repository");
                println!();
                println!("Commands:");
                println!("  sync          - Sync files from $HOME/.config to $HOME/repos/dotfiles/config");
                println!("  status        - Show status of files in distribution.toml");
                println!("  install       - Install files from $HOME/repos/dotfiles/config to $HOME/.config");
                println!("  add <tool> <file> - Add a file to distribution.toml and copy to repo");
                println!("  remove <tool> <file> - Remove a file from distribution.toml");
                println!("  precheck      - Check that distribution.toml exists and has valid syntax");
                println!("  usage         - Show this help message");
                println!();
                println!("Files matching patterns in $HOME/repos/dotfiles/.dotignore will be skipped");
            }
        }
        
        Ok(())
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    
    let mut app = App::new()?;
    app.run(&cli.command)?;
    
    Ok(())
}