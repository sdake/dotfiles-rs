# dotfiles-rs

A Rust implementation of a dotfiles manager that synchronizes configuration files between your home
directory and a Git repository.

## Features

- Synchronize files from `$HOME/.config` to repository
- Install files from repository to `$HOME/.config`
- Track status of configuration files
- Add or remove files from tracking
- Built-in ignore patterns for sensitive files
- Simple TOML-based configuration format
- Support for creating a self-contained binary with embedded dotfiles

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/yourusername/dotfiles-rs.git
cd dotfiles-rs

# Build the project
cargo build --release

# Install the binary to your PATH
cargo install --path .
```

### Building a Static Binary

For maximum portability, you can build a completely static binary that has no dependencies on system
libraries.

#### On Linux with musl:

```bash
# Install musl target
rustup target add x86_64-unknown-linux-musl

# Build static binary
cargo build --release --target=x86_64-unknown-linux-musl

# The static binary will be in target/x86_64-unknown-linux-musl/release/dotfiles-rs
```

#### On macOS (using cargo-zigbuild):

Since macOS doesn't support fully static binaries natively, you can use the Zig compiler:

```bash
# Install cargo-zigbuild
cargo install cargo-zigbuild

# Build for macOS
cargo zigbuild --release --target=x86_64-apple-darwin
# Or for Apple Silicon
cargo zigbuild --release --target=aarch64-apple-darwin

# The binary will be in target/{target}/release/dotfiles-rs
```

#### Cross-compilation:

```bash
# Build for different platforms using cargo-zigbuild
cargo install cargo-zigbuild

# For Linux
cargo zigbuild --release --target=x86_64-unknown-linux-gnu
# For Windows
cargo zigbuild --release --target=x86_64-pc-windows-gnu
```

### Prerequisites

- Rust 1.70.0 or later
- Cargo package manager

## Usage

```
dotfiles-rs [COMMAND]
```

### Commands

- `sync` - Sync files from $HOME/.config to repository
- `status` - Show status of files in distribution.toml
- `install` - Install files from repository to $HOME/.config
- `add <tool> <file>` - Add a file to distribution.toml and copy to repo
- `remove <tool> <file>` - Remove a file from distribution.toml
- `precheck` - Check that distribution.toml exists and has valid syntax
- `usage` - Show usage information
- `help` - Print help message

### Options

- `--embedded, -e` - Use files from the embedded archive instead of filesystem

### Examples

```bash
# Install dotfiles from repository to your .config directory
dotfiles-rs install

# Check status of all tracked files
dotfiles-rs status

# Add a new file to tracking
dotfiles-rs add nvim init.lua  # where 'nvim' is the tool and 'init.lua' is the file

# Sync all files back to repository (after making local changes)
dotfiles-rs sync

# Verify your distribution.toml file is valid
dotfiles-rs precheck

# Use the embedded dotfiles (if built with embedded files)
dotfiles-rs --embedded install
```

## Getting Started: Adding Your Dotfiles

Follow this step-by-step guide to manage your dotfiles with dotfiles-rs:

### 1. Set Up Your Repository

```bash
# Create your dotfiles repository
mkdir -p ~/repos/dotfiles/config

# Create an initial distribution.toml file
touch ~/repos/dotfiles/distribution.toml
```

### 2. Add Your First Configuration Files

Identify configuration files you want to track. For example, to add your Neovim configuration:

```bash
# Create the tool directory in the repo if it doesn't exist
mkdir -p ~/repos/dotfiles/config/nvim

# Add a file to tracking (this copies from ~/.config/nvim/init.lua to the repo)
dotfiles-rs add nvim init.lua

# Add another file from the same tool
dotfiles-rs add nvim lua/plugins.lua
```

The `add` command does three things:

1. Adds the file entry to distribution.toml
2. Creates necessary directories in your repo
3. Copies the file from ~/.config to your repo

### 3. Organize By Tools

Group your configurations by the tools they belong to:

```bash
# Shell configurations
dotfiles-rs add fish config.fish
dotfiles-rs add fish functions/myfunction.fish

# Terminal configurations
dotfiles-rs add alacritty alacritty.yml
dotfiles-rs add tmux tmux.conf

# Git configurations
dotfiles-rs add git config
```

### 4. Check Your Tracked Files

```bash
# Verify distribution.toml syntax
dotfiles-rs precheck

# See status of all tracked files
dotfiles-rs status
```

### 5. Setting Up On a New Machine

When setting up a new machine, clone your repository and install your dotfiles:

```bash
# Clone your dotfiles repository
git clone https://github.com/yourusername/dotfiles.git ~/repos/dotfiles

# Install all tracked files to their proper locations
dotfiles-rs install
```

### 6. Keep Everything in Sync

After making changes to your configuration files:

```bash
# Sync changes from ~/.config to the repository
dotfiles-rs sync

# Commit and push changes
cd ~/repos/dotfiles
git add .
git commit -m "Update configurations"
git push
```

## Configuration

The program uses a TOML configuration file called `distribution.toml` located in your dotfiles
repository.

Example configuration:

```toml
[nvim]
files = [
  "init.lua",
  "lua/plugins.lua",
  "lua/keymaps.lua"
]

[zsh]
files = [
  ".zshrc",
  ".zshenv"
]

[alacritty]
files = ["alacritty.yml"]
```

Each tool (section in the TOML file) corresponds to a directory under `.config`, and the files array
contains the files to track within that directory.

## Ignoring Files

Create a `.dotignore` file in your repository to specify patterns for files that should be ignored
during sync operations. This is useful for sensitive files like credentials and tokens.

Example `.dotignore`:

```
*history
*_history
*id_rsa*
*authorized_keys*
*known_hosts*
*token*
*.cert
*.key
*.pem
```

## Embedded Dotfiles

You can create a self-contained binary that includes all your dotfiles embedded within it. This is useful for:

- Deploying your dotfiles to a new machine without needing to clone a repository
- Keeping your dotfiles private while still being able to use them on multiple machines
- Simplifying the installation process with a single executable file

### Building with Embedded Dotfiles

The project includes a helper script to build a binary with your dotfiles embedded:

```bash
# Make the script executable if needed
chmod +x build_with_dotfiles.sh

# Run the script to build a binary with embedded dotfiles
./build_with_dotfiles.sh
```

The script will:
1. Copy your dotfiles from `$HOME/repos/dotfiles` to the `embedded_dotfiles` directory
2. Build a release binary with these embedded files
3. The resulting binary will be at `target/release/dotfiles-rs`

### Using the Embedded Binary

To use the embedded dotfiles, run the binary with the `--embedded` flag:

```bash
# Install dotfiles from the embedded archive
./target/release/dotfiles-rs --embedded install

# Check status of embedded dotfiles
./target/release/dotfiles-rs --embedded status
```

### Customizing the Embedded Files

If you want to manually control which files are embedded:

1. Copy only the files you want to the `embedded_dotfiles` directory
2. Ensure you have a valid `distribution.toml` and `.dotignore` file
3. Run `cargo build --release` to build the binary

## Development

```bash
# Run tests
cargo test

# Check for linting issues
cargo clippy

# Format code
cargo fmt
```

