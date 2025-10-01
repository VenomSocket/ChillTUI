# ChillTUI

Fast terminal UI for torrent search via chill.institute and Put.io integration.

## Quick Start

```bash
# Run the binary
./chilltui

# First run will launch setup wizard
```

## Build from Source

```bash
cargo build --release
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