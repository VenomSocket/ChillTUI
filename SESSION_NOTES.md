# PutIO-TUI Development Session Notes

## Project Overview
A Rust-based terminal UI for searching torrents via Chill.institute API and sending them to Put.io for download.

**Location**: `/home/vixus/claude/putio-tui-dist`
**Language**: Rust
**Key Dependencies**: crossterm (TUI), ureq (HTTP), serde (JSON)

## Current Status (2025-09-30)

### ✅ Completed Features

1. **API Integration**
   - Chill.institute v3 API search (requires both `Authorization` header with API key AND `X-Putio-Token` header)
   - Put.io OAuth token authentication (users create their own OAuth apps)
   - NSFW content filtering (filterNastyResults parameter)
   - Multiple indexer support: 1337x, eztv, nyaasi, rutracker, thepiratebay, therarbg, uindex, yts

2. **Core Functionality**
   - Search torrents by keyword
   - Filter by indexers (individual or "all")
   - Sort by: Seeders (default), Size, Name
   - Min seeds filter: 0, 5, 10 (default), 100
   - NSFW filter: Filter NSFW (default) / Allow NSFW
   - Select multiple torrents with Space
   - Send torrents to Put.io with Enter (works on single highlighted item or multiple selected)

3. **UI Layout** (Recently Fixed)
   - Single-line borders (┌─┐└┘│)
   - Left panel (27 chars): Filter boxes with consistent alignment
   - Results panel: Dynamic width, fills to terminal edge
   - Column headers: Sel │ Title │ Size │ Seeds
   - Search bar with cursor indicator (_)
   - Status bar at bottom
   - Scroll indicators (^^ / vv)

4. **Navigation**
   - Arrow keys: Up/Down to navigate within panels, Left/Right between panels
   - Tab/Shift+Tab: Switch between panels
   - Up from top of results: Jump to search
   - Left from results: Go to filters
   - Right from filters: Go to results
   - Space: Select/deselect items
   - Enter: Search (from search/filters) or send to Put.io (from results)
   - ESC: Quit

### 🐛 Known Issues

1. **Seeders option in SORT BY might not be visible** - May be cut off or hidden when selected (cursor with inverted background)
2. **Border alignment may need fine-tuning** - Results panel borders (top, bottom, right) were just fixed but need verification

### 📁 Key Files

- `src/main.rs` - Entry point
- `src/ui/mod.rs` - Main TUI implementation (~600 lines)
- `src/ui/setup.rs` - First-run setup wizard
- `src/api/chill.rs` - Chill.institute API client
- `src/api/putio.rs` - Put.io API client
- `src/models/mod.rs` - Data structures with field mappings
- `src/config.rs` - Configuration management
- `Cargo.toml` - Project dependencies

### 🔧 Recent Major Changes (Session 2025-09-30)

1. **Fixed all UI alignment issues**:
   - Left panel boxes now exactly 27 chars wide with consistent formatting
   - Results panel content width calculated as: `width - x - 7`
   - Title column dynamically sized: `content_width - 28` (for fixed columns)
   - All borders now align vertically using terminal width correctly
   - Padding calculation: `width - x - 7 - header_line.len()`

2. **Layout Calculation Logic**:
   ```rust
   // Left panel: x=0 to x=27, separator at x=28
   // Results panel: starts at x=29
   // Terminal width passed to results panel for proper edge alignment
   // Content width = terminal_width - x_position - 7 (for borders/spacing)
   // Padding = terminal_width - x_position - 7 - content.len()
   ```

3. **Format Strings**:
   - Header: `"{:<3} │ {:<width$} │ {:>10} │ {:>6}"`
   - Rows: `"{} │ {} │ {:>10} │ {:>6}"` (checkbox, title, size, seeds)
   - Both print: "   " (3 spaces for scroll indicator alignment) before content

### 🚀 Build & Run

```bash
cd /home/vixus/claude/putio-tui-dist

# Development (fast, debug build)
cargo run

# Production (optimized, slower build)
cargo build --release
cp target/release/putio-tui ./putio-tui
./putio-tui
```

### 📝 Configuration

Location: `~/.config/putio-tui/config.json`

Required:
- `chill_api_key` - Get by emailing chill-institute@proton.me or via x.com/chill_institute
- `putio_oauth_token` - Create OAuth app at https://app.put.io/oauth
- `putio_folder_name` - Folder name on Put.io (default: "PutTUI")
- `putio_folder_id` - Auto-populated after first run

### 🔍 API Details

**Chill.institute API**:
- Base URL: `https://chill.institute/api/v3`
- Endpoint: `/search?keyword={query}&indexer={csv}&filterNastyResults={bool}`
- Headers:
  - `Authorization: {api_key}` (NOT "Bearer {key}")
  - `X-Putio-Token: {putio_token}`
- Returns: Array of TorrentResult objects

**Field Mappings**:
- API `source` → Rust `indexer`
- API `peers` → Rust `leechers`
- API `link` → Rust `magnet`
- API `size` → Rust `size` (u64, not String)

### 🎯 Next Session TODO

1. **Verify all border alignments** - Test with different terminal sizes
2. **Fix Seeders visibility** - Check if it's hidden due to background color issue
3. **Test resizing** - Ensure dynamic width calculations work correctly
4. **Consider adding**:
   - Help screen (show keybindings)
   - Download progress tracking
   - Search history
   - Configurable keybindings

### 💡 Important Development Notes

- **Always work in**: `/home/vixus/claude/putio-tui-dist`
- **Test with**: `./putio-tui` (binary in working directory)
- **Backup exists at**: `/home/vixus/claude/putio-tui-dist-backup`
- **For faster iteration**: Use `cargo run` instead of release builds
- **Border calculations**: Always account for x position + all printed characters
- **Width parameter**: Results panel now receives full terminal width, not panel width

### 🎨 UI Layout ASCII Reference

```
┌─────────────────────────────────────────────────────────────────┐
│ Search: query_                                                  │
├─────────────────────────────────────────────────────────────────┤
│┌─ SORT BY ────────┐│┌─ RESULTS ───────────────────────────────┐│
││ ● Seeders        ││││ Sel │ Title            │ Size │ Seeds  ││
││ ○ Size           ││││ [ ] │ Movie Title      │ 1GB  │ 100    ││
││ ○ Name           ││││ [✓] │ Another Movie    │ 2GB  │ 200    ││
│└──────────────────┘││└────────────────────────────────────────┘│
│┌─ INDEXERS ───────┐││                                           │
││ [✓] all          │││                                           │
││ [ ] 1337x        │││                                           │
│└──────────────────┘││                                           │
├────────────────────┴┴───────────────────────────────────────────┤
│ ✓ Found 32 results                                              │
└─────────────────────────────────────────────────────────────────┘
```

### 📞 Contact & Support

If issues persist:
- GitHub: https://github.com/anthropics/claude-code/issues
- Check logs with `--debug` flag
- Review config at `~/.config/putio-tui/config.json`

---
*Last Updated: 2025-09-30 01:06 PDT*
*Session Duration: ~3 hours*
*Builds Completed: 50+*