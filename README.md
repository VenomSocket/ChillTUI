# ChillTUI

Fast terminal UI for torrent search via chill.institute and Put.io integration.

## Quick Start

```bash
# Run the binary
./chilltui

# First run will launch setup wizard
```

## Build from Source

### Dependencies

**Arch Linux**
```bash
sudo pacman -S rust
```

**Ubuntu/Debian**
```bash
sudo apt update
sudo apt install curl build-essential
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

**Fedora**
```bash
sudo dnf install rust cargo
```

### Build

```bash
cargo build --release
sudo cp target/release/chilltui /usr/local/bin/
```

## Usage

```bash
./chilltui          # Run application
./chilltui --setup  # Run setup wizard
./chilltui --debug  # Enable debug logging
./chilltui --help   # Show help
```

## Controls

- **Type** - Search torrents
- **Enter** - Execute search / Send to Put.io
- **↑↓** - Navigate results
- **Space** - Select/deselect
- **Tab** - Switch focus
- **ESC** - Clear search
- **ESC×2** - Quit

## Requirements

- **Chill.institute API key** - Request by emailing chill-institute@proton.me or via x.com/chill_institute
- **Put.io account** - Sign up at https://put.io

## Config

`~/.config/chilltui/config.json`