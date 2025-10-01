pub mod setup;

use crossterm::{
    cursor, event::{self, Event, KeyCode, KeyEvent},
    execute, queue, style::{Color, Print, SetBackgroundColor, SetForegroundColor},
    terminal::{self, ClearType},
};
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::api::{ChillClient, PutioClient};
use crate::config::Config;
use crate::models::TorrentResult;

// Layout constants
struct Layout;

impl Layout {
    const MARGIN_X: u16 = 1;  // Horizontal margin (left/right)
    const MARGIN_Y: u16 = 1;  // Vertical margin (top/bottom)
    const LEFT_PANEL_WIDTH: u16 = 22;
    const LEFT_PANEL_CONTENT_WIDTH: usize = 21;
    const RESULTS_X_OFFSET: u16 = 23;
    const FILTER_BOX_CONTENT_WIDTH: usize = 17;
    const STATUS_BAR_LINES: u16 = 3;
    const HEADER_HEIGHT: u16 = 3;
}

// Dracula theme colors
struct DraculaTheme;

impl DraculaTheme {
    const BG: Color = Color::Rgb { r: 40, g: 42, b: 54 };
    const BG_LIGHTER: Color = Color::Rgb { r: 68, g: 71, b: 90 };
    const FG: Color = Color::Rgb { r: 248, g: 248, b: 242 };
    const FG_DIM: Color = Color::Rgb { r: 189, g: 191, b: 186 };
    const COMMENT: Color = Color::Rgb { r: 98, g: 114, b: 164 };
    const CYAN: Color = Color::Rgb { r: 139, g: 233, b: 253 };
    const GREEN: Color = Color::Rgb { r: 80, g: 250, b: 123 };
    const ORANGE: Color = Color::Rgb { r: 255, g: 184, b: 108 };
    const PINK: Color = Color::Rgb { r: 255, g: 121, b: 198 };
    const PURPLE: Color = Color::Rgb { r: 189, g: 147, b: 249 };
    const RED: Color = Color::Rgb { r: 255, g: 85, b: 85 };
    const YELLOW: Color = Color::Rgb { r: 241, g: 250, b: 140 };
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Panel {
    Search,
    Filters,
    Results,
}

#[derive(Clone, Copy, PartialEq)]
enum SortMode {
    Seeders,
    Size,
    Name,
}

// Animation and rendering constants
struct AnimationConfig;

impl AnimationConfig {
    const FRAME_INTERVAL_MS: u64 = 50;
    const TITLE_SCROLL_SPEED: u8 = 3;  // Frames between scroll updates
    const TITLE_SCROLL_PAUSE: usize = 20;  // Frames to pause at ends
    const SEND_SUCCESS_DURATION_SECS: u64 = 2;
}

// Layout cache for column positions (recalculated on resize)
#[derive(Clone)]
struct LayoutCache {
    title_width: usize,
    size_column: u16,
    seeds_column: u16,
    source_column: u16,
    separator_column: u16,
    right_border_column: usize,
    terminal_width: u16,
    terminal_height: u16,
}

impl LayoutCache {
    fn new(term_width: u16, term_height: u16, results_x: u16) -> Self {
        let right_border_col = (term_width as usize).saturating_sub(Layout::MARGIN_X as usize + 1);

        // Position columns from right to left
        let source_end = right_border_col;
        let source_start = source_end.saturating_sub(10);
        let seeds_end = source_start.saturating_sub(3);
        let seeds_start = seeds_end.saturating_sub(5);
        let size_end = seeds_start.saturating_sub(3);
        let size_start = size_end.saturating_sub(12);
        let sep_pos = size_start.saturating_sub(3);

        let title_width = sep_pos.saturating_sub((results_x as usize) + 2 + 3 + 3 + 3);

        Self {
            title_width,
            size_column: size_start as u16,
            seeds_column: seeds_start as u16,
            source_column: source_start as u16,
            separator_column: sep_pos as u16,
            right_border_column: right_border_col,
            terminal_width: term_width,
            terminal_height: term_height,
        }
    }

    fn needs_update(&self, term_width: u16, term_height: u16) -> bool {
        self.terminal_width != term_width || self.terminal_height != term_height
    }
}

// Precomputed marquee for scrolling status bar
struct MarqueeCache {
    chars: Vec<char>,
    offset: usize,
}

impl MarqueeCache {
    fn new(text: &str) -> Self {
        Self {
            chars: text.chars().collect(),
            offset: 0,
        }
    }

    fn advance(&mut self) {
        self.offset = (self.offset + 1) % self.chars.len();
    }

    fn render(&self, width: usize) -> String {
        if self.chars.is_empty() {
            return String::new();
        }

        self.chars.iter()
            .cycle()
            .skip(self.offset)
            .take(width)
            .collect()
    }
}

pub struct App {
    config: Config,
    chill_client: Option<ChillClient>,
    putio_client: Option<PutioClient>,
    query: String,
    results: Vec<TorrentResult>,
    selected_index: usize,
    scroll_offset: usize,
    active_panel: Panel,
    available_indexers: Vec<String>,
    selected_indexers: Vec<String>,
    indexer_cursor: usize,
    sort_by: SortMode,
    sort_cursor: usize,
    min_seeds: u32,
    filter_nsfw: bool,
    searching: bool,
    status_message: String,
    debug: bool,
    sending_to_putio: bool,
    sending_complete: bool,
    sent_file_name: String,
    title_scroll_offset: usize,
    title_scroll_direction: i8,  // 1 = forward, -1 = backward
    frame_counter: u8,
    marquee_scroll_offset: usize,
    should_animate: bool,
    cached_width: u16,
    cached_height: u16,
    spinner_frame: u8,
    search_results: Arc<Mutex<Option<Result<Vec<TorrentResult>, String>>>>,
    send_complete: Arc<Mutex<bool>>,
    layout_cache: Option<LayoutCache>,
    marquee_cache: MarqueeCache,
}

impl App {
    pub fn new(config: Config, debug: bool) -> Self {
        let chill_client = Self::create_chill_client(&config);
        let putio_client = Self::create_putio_client(&config);

        Self {
            config,
            chill_client,
            putio_client,
            query: String::new(),
            results: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            active_panel: Panel::Search,
            available_indexers: vec![
                "all".to_string(),
                "1337x".to_string(),
                "EZTV".to_string(),
                "nyaa.si".to_string(),
                "RUtracker".to_string(),
                "TPB".to_string(),
                "RARBG".to_string(),
                "Uindex".to_string(),
                "YTS".to_string(),
            ],
            selected_indexers: vec!["all".to_string()],
            indexer_cursor: 0,
            sort_by: SortMode::Seeders,
            sort_cursor: 0,
            min_seeds: 10,
            filter_nsfw: true,
            searching: false,
            status_message: "Ready".to_string(),
            debug,
            sending_to_putio: false,
            sending_complete: false,
            sent_file_name: String::new(),
            title_scroll_offset: 0,
            title_scroll_direction: 1,
            frame_counter: 0,
            marquee_scroll_offset: 0,
            should_animate: true,
            cached_width: 0,
            cached_height: 0,
            spinner_frame: 0,
            search_results: Arc::new(Mutex::new(None)),
            send_complete: Arc::new(Mutex::new(false)),
            layout_cache: None,
            marquee_cache: MarqueeCache::new("+++ ChillTUI - chill.institute but from the terminal! Search for content and press enter to send results to Put.io +++    +++"),
        }
    }

    fn create_chill_client(config: &Config) -> Option<ChillClient> {
        config
            .chill_api_key
            .as_ref()
            .map(|key| ChillClient::new(key.clone(), config.putio_oauth_token.clone()))
    }

    fn create_putio_client(config: &Config) -> Option<PutioClient> {
        config
            .putio_oauth_token
            .as_ref()
            .map(|token| PutioClient::new(token.clone()))
    }

    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;

        let result = self.main_loop();

        // Cleanup
        execute!(
            stdout,
            terminal::LeaveAlternateScreen,
            cursor::Show
        )?;
        terminal::disable_raw_mode()?;

        result
    }

    fn main_loop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            // Check for completed search results
            if self.searching {
                if let Ok(mut guard) = self.search_results.try_lock() {
                    if let Some(result) = guard.take() {
                        match result {
                            Ok(results) => {
                                self.results = results;
                                self.selected_index = 0;
                                self.scroll_offset = 0;
                                self.active_panel = Panel::Results;
                                self.status_message = format!("✓ Found {} results", self.results.len());
                                self.should_animate = true;

                                if self.debug {
                                    eprintln!("[DEBUG] Search completed: {} results", self.results.len());
                                }
                            }
                            Err(e) => {
                                self.status_message = format!("✗ Search error: {}", e);
                                if self.debug {
                                    eprintln!("[DEBUG] Search error: {}", e);
                                }
                            }
                        }
                        self.searching = false;
                    }
                }
            }

            // Check for completed send to Put.io
            if self.sending_to_putio && !self.sending_complete {
                if let Ok(guard) = self.send_complete.try_lock() {
                    if *guard {
                        self.sending_complete = true;

                        // Update message to show completion
                        let msg = &self.sent_file_name.clone();
                        if msg.starts_with("Sending '") {
                            // Single file: extract title
                            if let Some(title) = msg.strip_prefix("Sending '").and_then(|s| s.strip_suffix("' to Put.io")) {
                                self.sent_file_name = format!("Sent '{}' to Put.io!", title);
                            }
                        } else if let Some(num_str) = msg.split("Sending ").nth(1).and_then(|s| s.split(" files").next()) {
                            self.sent_file_name = format!("Sent {} files to Put.io!", num_str);
                        }

                        // Schedule close after 2 seconds
                        let send_complete_clone = Arc::clone(&self.send_complete);
                        thread::spawn(move || {
                            std::thread::sleep(std::time::Duration::from_secs(2));
                            if let Ok(mut g) = send_complete_clone.lock() {
                                *g = false;
                            }
                        });
                    }
                }
            }

            // Check if we should close the sending dialog
            if self.sending_to_putio && self.sending_complete {
                if let Ok(guard) = self.send_complete.try_lock() {
                    if !*guard {
                        self.sending_to_putio = false;
                        self.sending_complete = false;
                        self.active_panel = Panel::Results;
                    }
                }
            }

            self.draw()?;

            // Only update animations when needed
            if self.should_animate {
                self.frame_counter = self.frame_counter.wrapping_add(1);
                if self.frame_counter % 3 == 0 {
                    self.update_title_scroll();
                }
            }

            if event::poll(std::time::Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    if !self.handle_key(key)? {
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    fn update_title_scroll(&mut self) {
        // Update marquee scroll (always scroll) - using cache
        self.marquee_cache.advance();

        // Update spinner when searching or sending
        if self.searching || self.sending_to_putio {
            self.spinner_frame = self.spinner_frame.wrapping_add(1);
        }

        // Update title scroll only if there are results
        if self.results.is_empty() {
            return;
        }

        // Scroll based on direction
        if self.title_scroll_direction == 1 {
            self.title_scroll_offset += 1;
            // Reverse when we've scrolled enough (arbitrary max scroll)
            if self.title_scroll_offset >= 20 {
                self.title_scroll_direction = -1;
            }
        } else {
            if self.title_scroll_offset > 0 {
                self.title_scroll_offset -= 1;
            } else {
                self.title_scroll_direction = 1;
            }
        }
    }

    fn draw(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut stdout = io::stdout();

        // Check if terminal size changed
        let (term_width, term_height) = terminal::size()?;
        let size_changed = term_width != self.cached_width || term_height != self.cached_height;

        if size_changed {
            self.cached_width = term_width;
            self.cached_height = term_height;
        }

        // Update layout cache if needed
        if self.layout_cache.is_none() || size_changed {
            let results_x = Layout::RESULTS_X_OFFSET + Layout::MARGIN_X;
            self.layout_cache = Some(LayoutCache::new(term_width, term_height, results_x));
        }

        // Calculate content area with margins
        let x_offset = Layout::MARGIN_X;
        let y_offset = Layout::MARGIN_Y;
        let content_width = term_width.saturating_sub(Layout::MARGIN_X * 2);
        let content_height = term_height.saturating_sub(Layout::MARGIN_Y * 2);

        queue!(
            stdout,
            terminal::Clear(ClearType::All),
            SetBackgroundColor(DraculaTheme::BG),
            SetForegroundColor(DraculaTheme::FG)
        )?;

        // Draw search bar at top (with y offset)
        self.draw_search_bar(&mut stdout, x_offset, content_width, y_offset)?;

        // Draw horizontal separator
        self.draw_horizontal_line(&mut stdout, x_offset, 2 + y_offset, content_width)?;

        // Draw left panel (filters & sort)
        self.draw_left_panel(&mut stdout, x_offset, Layout::LEFT_PANEL_WIDTH, content_height, y_offset)?;

        // Draw vertical separator between panels
        self.draw_vertical_line(&mut stdout, Layout::LEFT_PANEL_WIDTH + x_offset, Layout::HEADER_HEIGHT + y_offset, content_height - Layout::STATUS_BAR_LINES - 2)?;

        // Draw results panel
        self.draw_results_panel(&mut stdout, Layout::RESULTS_X_OFFSET + x_offset, term_width, content_height, y_offset)?;

        // Draw status bars (navigation help + status message)
        self.draw_status_bars(&mut stdout, x_offset, content_width, content_height, y_offset)?;

        stdout.flush()?;
        Ok(())
    }

    fn draw_search_bar(&self, stdout: &mut io::Stdout, x: u16, width: u16, y: u16) -> Result<(), Box<dyn std::error::Error>> {
        let active = matches!(self.active_panel, Panel::Search);
        let border_color = if active { DraculaTheme::CYAN } else { DraculaTheme::FG_DIM };

        queue!(
            stdout,
            cursor::MoveTo(x, y),
            SetForegroundColor(border_color),
            Print("┌"),
            Print("─".repeat((width - 2) as usize)),
            Print("┐"),
            cursor::MoveTo(x, y + 1),
            Print("│ "),
            SetForegroundColor(DraculaTheme::FG),
        )?;

        if active {
            queue!(
                stdout,
                SetForegroundColor(DraculaTheme::PINK),
                Print("▶ "),
            )?;
        } else {
            queue!(stdout, Print("  "))?;
        }

        queue!(
            stdout,
            SetForegroundColor(DraculaTheme::FG),
            Print("Search: "),
            SetForegroundColor(DraculaTheme::CYAN),
            Print(&self.query),
        )?;

        // Add cursor indicator when active
        if active {
            queue!(stdout, SetForegroundColor(DraculaTheme::YELLOW), Print("_"))?;
        }

        // Fill rest of line to align with frame
        // Printed so far at position x: "│ " (2) + arrow/space (2) + "Search: " (8) + query + maybe "_" (1)
        // Right border should be at position (x + width - 1)
        // Current position = x + 2 + 2 + 8 + query + cursor
        // Padding needed = (x + width - 1) - current_position
        let used = 2 + 2 + 8 + self.query.chars().count() + (if active { 1 } else { 0 });
        let right_border_pos = (x as usize) + (width as usize) - 1;
        let current_pos = (x as usize) + used;
        let remaining = right_border_pos.saturating_sub(current_pos);
        queue!(
            stdout,
            SetForegroundColor(DraculaTheme::FG),
            Print(" ".repeat(remaining)),
            SetForegroundColor(border_color),
            Print("│"),
        )?;

        Ok(())
    }

    fn draw_horizontal_line(&self, stdout: &mut io::Stdout, x: u16, y: u16, width: u16) -> Result<(), Box<dyn std::error::Error>> {
        queue!(
            stdout,
            cursor::MoveTo(x, y),
            SetForegroundColor(DraculaTheme::FG_DIM),
            Print("├"),
            Print("─".repeat((width - 2) as usize)),
            Print("┤"),
        )?;
        Ok(())
    }

    fn draw_vertical_line(&self, stdout: &mut io::Stdout, x: u16, start_y: u16, height: u16) -> Result<(), Box<dyn std::error::Error>> {
        for y in start_y..(start_y + height) {
            queue!(
                stdout,
                cursor::MoveTo(x, y),
                SetForegroundColor(DraculaTheme::FG_DIM),
                Print("│"),
            )?;
        }
        Ok(())
    }

    fn draw_left_panel(&self, stdout: &mut io::Stdout, x: u16, _width: u16, height: u16, y_offset: u16) -> Result<(), Box<dyn std::error::Error>> {
        let active = matches!(self.active_panel, Panel::Filters);
        let mut y = Layout::HEADER_HEIGHT + y_offset;

        // Draw left border for entire left panel
        let panel_height = height + y_offset - Layout::STATUS_BAR_LINES - y;
        for i in 0..panel_height {
            queue!(
                stdout,
                cursor::MoveTo(x, y + i),
                SetForegroundColor(DraculaTheme::FG_DIM),
                Print("│"),
            )?;
        }

        // Sort section
        // Format: "┌─ SORT BY {:─<N}┐" where N makes total = 21
        // "┌─ SORT BY " = 11 chars, "┐" = 1 char, so N = 21 - 12 = 9
        queue!(
            stdout,
            cursor::MoveTo(x + 1, y),
            SetForegroundColor(if active { DraculaTheme::CYAN } else { DraculaTheme::PURPLE }),
            Print(format!("┌─ SORT BY {:─<9}┐", "")),
        )?;
        y += 1;

        let sorts = vec![
            ("Seeders", SortMode::Seeders),
            ("Size", SortMode::Size),
            ("Name", SortMode::Name),
        ];

        for (i, (name, mode)) in sorts.iter().enumerate() {
            let selected = self.sort_by == *mode;
            let cursor = active && self.sort_cursor == i;

            let (fg, bg, marker) = if cursor {
                (DraculaTheme::BG, DraculaTheme::PINK, "●")
            } else if selected {
                (DraculaTheme::GREEN, DraculaTheme::BG, "●")
            } else {
                (DraculaTheme::FG_DIM, DraculaTheme::BG, "○")
            };

            // Content rows: "│ " + content + " │"
            let content = format!("{} {}", marker, name);
            queue!(
                stdout,
                cursor::MoveTo(x + 1, y),
                SetForegroundColor(DraculaTheme::FG_DIM),
                Print("│ "),
                SetBackgroundColor(bg),
                SetForegroundColor(fg),
                Print(format!("{:<width$}", content, width = Layout::FILTER_BOX_CONTENT_WIDTH)),
                SetBackgroundColor(DraculaTheme::BG),
                SetForegroundColor(DraculaTheme::FG_DIM),
                Print(" │"),
            )?;
            y += 1;
        }

        // Footer: "└{:─<19}┘" = 21 total
        queue!(
            stdout,
            cursor::MoveTo(x + 1, y),
            SetForegroundColor(if active { DraculaTheme::CYAN } else { DraculaTheme::FG_DIM }),
            Print(format!("└{:─<19}┘", "")),
        )?;
        y += 2;

        // Indexers section
        // "┌─ INDEXERS " = 12 chars, "┐" = 1 char, so N = 21 - 13 = 8
        queue!(
            stdout,
            cursor::MoveTo(x + 1, y),
            SetForegroundColor(if active { DraculaTheme::CYAN } else { DraculaTheme::PURPLE }),
            Print(format!("┌─ INDEXERS {:─<8}┐", "")),
        )?;
        y += 1;

        let visible_indexers = ((height as usize).saturating_sub(y as usize + 8)).min(self.available_indexers.len());
        for (i, indexer) in self.available_indexers.iter().take(visible_indexers).enumerate() {
            let selected = self.selected_indexers.contains(indexer);
            let cursor = active && self.sort_cursor == i + 3;

            let (fg, bg, marker) = if cursor {
                (DraculaTheme::BG, DraculaTheme::PINK, if selected { "[✓]" } else { "[ ]" })
            } else if selected {
                (DraculaTheme::GREEN, DraculaTheme::BG, "[✓]")
            } else {
                (DraculaTheme::FG_DIM, DraculaTheme::BG, "[ ]")
            };

            // Content rows: "│ " + content + " │"
            let content = format!("{} {}", marker, indexer);
            queue!(
                stdout,
                cursor::MoveTo(x + 1, y),
                SetForegroundColor(DraculaTheme::FG_DIM),
                Print("│ "),
                SetBackgroundColor(bg),
                SetForegroundColor(fg),
                Print(format!("{:<width$}", content, width = Layout::FILTER_BOX_CONTENT_WIDTH)),
                SetBackgroundColor(DraculaTheme::BG),
                SetForegroundColor(DraculaTheme::FG_DIM),
                Print(" │"),
            )?;
            y += 1;
        }

        queue!(
            stdout,
            cursor::MoveTo(x + 1, y),
            SetForegroundColor(if active { DraculaTheme::CYAN } else { DraculaTheme::FG_DIM }),
            Print(format!("└{:─<19}┘", "")),
        )?;
        y += 2;

        // Min seeds section
        // "┌─ MIN SEEDS " = 13 chars, "┐" = 1 char, so N = 21 - 14 = 7
        queue!(
            stdout,
            cursor::MoveTo(x + 1, y),
            SetForegroundColor(if active { DraculaTheme::CYAN } else { DraculaTheme::PURPLE }),
            Print(format!("┌─ MIN SEEDS {:─<7}┐", "")),
        )?;
        y += 1;

        let min_seed_options = vec![0, 5, 10, 100];
        let total_items = 3 + self.available_indexers.len();

        for (i, &seeds) in min_seed_options.iter().enumerate() {
            let selected = self.min_seeds == seeds;
            let cursor = active && self.sort_cursor == total_items + i;

            let (fg, bg, marker) = if cursor {
                (DraculaTheme::BG, DraculaTheme::PINK, "●")
            } else if selected {
                (DraculaTheme::GREEN, DraculaTheme::BG, "●")
            } else {
                (DraculaTheme::FG_DIM, DraculaTheme::BG, "○")
            };

            // Content rows: "│ " + content + " │"
            let content = format!("{} {} seeds", marker, seeds);
            queue!(
                stdout,
                cursor::MoveTo(x + 1, y),
                SetForegroundColor(DraculaTheme::FG_DIM),
                Print("│ "),
                SetBackgroundColor(bg),
                SetForegroundColor(fg),
                Print(format!("{:<width$}", content, width = Layout::FILTER_BOX_CONTENT_WIDTH)),
                SetBackgroundColor(DraculaTheme::BG),
                SetForegroundColor(DraculaTheme::FG_DIM),
                Print(" │"),
            )?;
            y += 1;
        }

        queue!(
            stdout,
            cursor::MoveTo(x + 1, y),
            SetForegroundColor(if active { DraculaTheme::CYAN } else { DraculaTheme::FG_DIM }),
            Print(format!("└{:─<19}┘", "")),
        )?;
        y += 2;

        // NSFW Filter section
        // "┌─ NSFW " = 8 chars, "┐" = 1 char, so N = 21 - 9 = 12
        queue!(
            stdout,
            cursor::MoveTo(x + 1, y),
            SetForegroundColor(if active { DraculaTheme::CYAN } else { DraculaTheme::PURPLE }),
            Print(format!("┌─ NSFW {:─<12}┐", "")),
        )?;
        y += 1;

        let nsfw_options = vec![("Filter NSFW", true), ("Allow NSFW", false)];
        let nsfw_base = total_items + 4; // After sort, indexers, and min_seeds

        for (i, (label, value)) in nsfw_options.iter().enumerate() {
            let selected = self.filter_nsfw == *value;
            let cursor = active && self.sort_cursor == nsfw_base + i;

            let (fg, bg, marker) = if cursor {
                (DraculaTheme::BG, DraculaTheme::PINK, "●")
            } else if selected {
                (DraculaTheme::GREEN, DraculaTheme::BG, "●")
            } else {
                (DraculaTheme::FG_DIM, DraculaTheme::BG, "○")
            };

            // Content rows: "│ " + content + " │"
            let content = format!("{} {}", marker, label);
            queue!(
                stdout,
                cursor::MoveTo(x + 1, y),
                SetForegroundColor(DraculaTheme::FG_DIM),
                Print("│ "),
                SetBackgroundColor(bg),
                SetForegroundColor(fg),
                Print(format!("{:<width$}", content, width = Layout::FILTER_BOX_CONTENT_WIDTH)),
                SetBackgroundColor(DraculaTheme::BG),
                SetForegroundColor(DraculaTheme::FG_DIM),
                Print(" │"),
            )?;
            y += 1;
        }

        // NSFW section bottom border
        queue!(
            stdout,
            cursor::MoveTo(x + 1, y),
            SetForegroundColor(if active { DraculaTheme::CYAN } else { DraculaTheme::FG_DIM }),
            Print(format!("└{:─<19}┘", "")),
        )?;
        y += 1;

        // Draw outer bottom border (for the entire left panel)
        let bottom_y = height + y_offset - Layout::STATUS_BAR_LINES;
        queue!(
            stdout,
            cursor::MoveTo(x, bottom_y),
            SetForegroundColor(DraculaTheme::FG_DIM),
            Print("└"),
            Print("─".repeat(Layout::LEFT_PANEL_WIDTH as usize - 1)),
            Print("┘"),
        )?;

        Ok(())
    }

    fn draw_results_panel(&self, stdout: &mut io::Stdout, x: u16, width: u16, height: u16, y_offset: u16) -> Result<(), Box<dyn std::error::Error>> {
        let active = matches!(self.active_panel, Panel::Results);
        let y = Layout::HEADER_HEIGHT + y_offset;

        // Calculate scroll state upfront
        let has_more_above = self.scroll_offset > 0;
        let results_height = (height as usize).saturating_sub(y as usize + Layout::STATUS_BAR_LINES as usize + 2);
        let visible_end = self.scroll_offset + results_height.min(self.results.len() - self.scroll_offset);
        let has_more_below = visible_end < self.results.len();

        // Results header - spans from x to right margin
        // Right edge is at (width - MARGIN_X - 1), so header_width = right_edge - x - 11 ("┌─ RESULTS ")
        let right_edge = (width as usize).saturating_sub(Layout::MARGIN_X as usize + 1);
        let header_width = right_edge.saturating_sub(x as usize + 11);
        queue!(
            stdout,
            cursor::MoveTo(x, y),
            SetForegroundColor(if active { DraculaTheme::CYAN } else { DraculaTheme::PURPLE }),
            Print("┌─ RESULTS "),
            SetForegroundColor(DraculaTheme::FG_DIM),
            Print("─".repeat(header_width)),
            Print("┐"),
        )?;

        // Calculate content dimensions
        let right_border_col = (width as usize).saturating_sub(Layout::MARGIN_X as usize + 1);

        if self.searching || self.sending_to_putio {
            // Draw outer panel borders
            for row_y in (y + 1)..(height + y_offset - Layout::STATUS_BAR_LINES) {
                queue!(
                    stdout,
                    cursor::MoveTo(x, row_y),
                    SetForegroundColor(DraculaTheme::FG_DIM),
                    Print("│"),
                    cursor::MoveTo(width - Layout::MARGIN_X - 1, row_y),
                    Print("│"),
                )?;
            }

            // Draw a nice centered box
            if self.searching {
                // Spinner animation for searching
                let spinner_chars = ['|', '/', '-', '\\'];
                let spinner = spinner_chars[(self.spinner_frame / 1) as usize % 4];
                let message = format!("Fetching {}", spinner);

                let box_width = message.len() + 4; // 2 chars padding on each side
                let panel_width = (width as usize).saturating_sub(x as usize + 1);
                let box_x = x + ((panel_width.saturating_sub(box_width)) / 2) as u16;
                let box_y = y + ((height.saturating_sub(y + Layout::STATUS_BAR_LINES + 5)) / 2);

                // Top border
                queue!(
                    stdout,
                    cursor::MoveTo(box_x, box_y),
                    SetForegroundColor(DraculaTheme::CYAN),
                    Print("┌"),
                    Print("─".repeat(box_width - 2)),
                    Print("┐"),
                )?;

                // Content
                let padding = (box_width - 2).saturating_sub(message.len()) / 2;
                queue!(
                    stdout,
                    cursor::MoveTo(box_x, box_y + 1),
                    SetForegroundColor(DraculaTheme::CYAN),
                    Print("│"),
                    SetForegroundColor(DraculaTheme::FG),
                    Print(" ".repeat(padding)),
                    SetForegroundColor(DraculaTheme::CYAN),
                    Print(&message),
                    SetForegroundColor(DraculaTheme::FG),
                    Print(" ".repeat((box_width - 2).saturating_sub(message.len() + padding))),
                    SetForegroundColor(DraculaTheme::CYAN),
                    Print("│"),
                )?;

                // Bottom border
                queue!(
                    stdout,
                    cursor::MoveTo(box_x, box_y + 2),
                    SetForegroundColor(DraculaTheme::CYAN),
                    Print("└"),
                    Print("─".repeat(box_width - 2)),
                    Print("┘"),
                )?;
            } else if self.sending_to_putio {
                // Sending confirmation with spinner or checkmark
                let icon = if self.sending_complete {
                    "✓"
                } else {
                    let spinner_chars = ['|', '/', '-', '\\'];
                    let ch = spinner_chars[(self.spinner_frame / 1) as usize % 4];
                    &format!("{}", ch)[..]
                };

                let message = format!("{} {}", icon, self.sent_file_name);

                // Calculate box width based on message length, ensuring it's wide enough
                // Add extra space to account for icon width variations
                let content_width = message.chars().count();
                let box_width = content_width + 4; // 2 chars padding on each side
                let panel_width = (width as usize).saturating_sub(x as usize + 1);
                let box_x = x + ((panel_width.saturating_sub(box_width)) / 2) as u16;
                let box_y = y + ((height.saturating_sub(y + Layout::STATUS_BAR_LINES + 5)) / 2);

                // Top border
                queue!(
                    stdout,
                    cursor::MoveTo(box_x, box_y),
                    SetForegroundColor(DraculaTheme::CYAN),
                    Print("┌"),
                    Print("─".repeat(box_width - 2)),
                    Print("┐"),
                )?;

                // Content
                let msg_len = message.chars().count();
                let padding = (box_width - 2).saturating_sub(msg_len) / 2;
                let right_padding = (box_width - 2).saturating_sub(msg_len + padding);
                queue!(
                    stdout,
                    cursor::MoveTo(box_x, box_y + 1),
                    SetForegroundColor(DraculaTheme::CYAN),
                    Print("│"),
                    SetForegroundColor(DraculaTheme::FG),
                    Print(" ".repeat(padding)),
                    SetForegroundColor(if self.sending_complete { DraculaTheme::GREEN } else { DraculaTheme::CYAN }),
                    Print(&message),
                    SetForegroundColor(DraculaTheme::FG),
                    Print(" ".repeat(right_padding)),
                    SetForegroundColor(DraculaTheme::CYAN),
                    Print("│"),
                )?;

                // Bottom border
                queue!(
                    stdout,
                    cursor::MoveTo(box_x, box_y + 2),
                    SetForegroundColor(DraculaTheme::CYAN),
                    Print("└"),
                    Print("─".repeat(box_width - 2)),
                    Print("┘"),
                )?;
            }
        } else if self.results.is_empty() {
            let message = "No results. Press Enter to search.";

            // Draw y+1 row with borders only
            queue!(
                stdout,
                cursor::MoveTo(x, y + 1),
                SetForegroundColor(DraculaTheme::FG_DIM),
                Print("│"),
                cursor::MoveTo(width - Layout::MARGIN_X - 1, y + 1),
                Print("│"),
            )?;

            // Draw message row at y+2 - left-aligned with right border
            queue!(
                stdout,
                cursor::MoveTo(x, y + 2),
                SetForegroundColor(DraculaTheme::FG_DIM),
                Print("│   "),
                Print(message),
                cursor::MoveTo(width - Layout::MARGIN_X - 1, y + 2),
                SetForegroundColor(DraculaTheme::FG_DIM),
                Print("│"),
            )?;

            // Fill empty rows with borders (starting from y+3)
            for row_y in (y + 3)..(height + y_offset - Layout::STATUS_BAR_LINES) {
                queue!(
                    stdout,
                    cursor::MoveTo(x, row_y),
                    SetForegroundColor(DraculaTheme::FG_DIM),
                    Print("│"),
                    cursor::MoveTo(width - Layout::MARGIN_X - 1, row_y),
                    Print("│"),
                )?;
            }
        } else {
            // Content dimensions already calculated above

            // Draw left and right borders for all rows when showing results
            for row_y in (y + 1)..(height + y_offset - Layout::STATUS_BAR_LINES) {
                queue!(
                    stdout,
                    cursor::MoveTo(x, row_y),
                    SetForegroundColor(DraculaTheme::FG_DIM),
                    Print("│"),
                    cursor::MoveTo(width - Layout::MARGIN_X - 1, row_y),
                    Print("│"),
                )?;
            }

            // Column headers with scroll indicator
            queue!(
                stdout,
                cursor::MoveTo(x, y + 1),
                SetForegroundColor(DraculaTheme::FG_DIM),
                Print("│ "),
            )?;

            if has_more_above {
                queue!(
                    stdout,
                    SetForegroundColor(DraculaTheme::YELLOW),
                    Print("^^ "),
                )?;
            } else {
                queue!(stdout, SetForegroundColor(DraculaTheme::FG), Print("   "))?;
            }

            // Use cached layout positions (recalculated only on resize)
            let cache = self.layout_cache.as_ref().unwrap();
            let size_start = cache.size_column;
            let seeds_start = cache.seeds_column;
            let source_start = cache.source_column;
            let sep_pos = cache.separator_column;
            let title_width = cache.title_width;

            // Print left side (Sel and Title)
            queue!(
                stdout,
                SetForegroundColor(DraculaTheme::CYAN),
                Print("Sel │ Title"),
            )?;

            // Add separator before Size column
            queue!(
                stdout,
                cursor::MoveTo(sep_pos as u16, y + 1),
                SetForegroundColor(DraculaTheme::CYAN),
                Print(" │ "),
            )?;

            // Position and print Size column
            queue!(
                stdout,
                cursor::MoveTo(size_start as u16, y + 1),
                SetForegroundColor(DraculaTheme::CYAN),
                Print(format!("{:^12} │ ", "Size")),
            )?;

            // Position and print Seeds column
            queue!(
                stdout,
                cursor::MoveTo(seeds_start as u16, y + 1),
                SetForegroundColor(DraculaTheme::CYAN),
                Print(format!("{:^5} │ ", "Seeds")),
            )?;

            // Position and print Source column
            queue!(
                stdout,
                cursor::MoveTo(source_start as u16, y + 1),
                SetForegroundColor(DraculaTheme::CYAN),
                Print(format!("{:^10}", "Source")),
            )?;

            // Results list
            for (i, result) in self.results[self.scroll_offset..visible_end].iter().enumerate() {
                let actual_index = self.scroll_offset + i;
                let is_selected = actual_index == self.selected_index;
                let is_marked = result.selected;

                let (fg, bg) = if is_selected && active {
                    (DraculaTheme::BG, DraculaTheme::PINK)
                } else if is_marked {
                    (DraculaTheme::GREEN, DraculaTheme::BG)
                } else {
                    (DraculaTheme::FG, DraculaTheme::BG)
                };

                let checkbox = if is_marked { "[✓]" } else { "[ ]" };

                // Scrolling title logic for long titles - only scroll when highlighted
                let title = if result.title.chars().count() > title_width {
                    if is_selected && active {
                        // OPTIMIZED: Precompute chars for O(1) access instead of O(n)
                        let extended_title = format!("{}    ", result.title);
                        let title_chars: Vec<char> = extended_title.chars().collect();
                        let scroll_pos = self.title_scroll_offset % title_chars.len();

                        // Create circular scrolling effect with direct indexing
                        title_chars.iter()
                            .cycle()
                            .skip(scroll_pos)
                            .take(title_width)
                            .collect()
                    } else {
                        // Not selected: just truncate with ellipsis (char-safe)
                        let truncated: String = result.title.chars().take(title_width.saturating_sub(3)).collect();
                        format!("{}...", truncated)
                    }
                } else {
                    format!("{:<width$}", result.title, width = title_width)
                };

                // Map indexer name and truncate if needed
                let indexer_lower = result.indexer.to_lowercase();
                let indexer_display = if indexer_lower.contains("rutracker") {
                    "RUtracker"
                } else {
                    match result.indexer.as_str() {
                        "thepiratebay" | "The Pirate Bay" => "TPB",
                        "eztv" => "EZTV",
                        "therarbg" => "RARBG",
                        "yts" => "YTS",
                        _ => &result.indexer,
                    }
                };

                let indexer = if indexer_display.chars().count() > 10 {
                    let truncated: String = indexer_display.chars().take(7).collect();
                    format!("{}...", truncated)
                } else {
                    indexer_display.to_string()
                };

                // Row format: Print checkbox and title on left, then position Size/Seeds/Source at absolute positions
                let row_y = y + 2 + i as u16;

                // Print left side (checkbox and title with scroll indicator space)
                queue!(
                    stdout,
                    cursor::MoveTo(x, row_y),
                    SetForegroundColor(DraculaTheme::FG_DIM),
                    Print("│ "),
                    SetForegroundColor(DraculaTheme::FG),
                    Print("   "),  // Space for scroll indicator alignment
                    SetBackgroundColor(bg),
                    SetForegroundColor(fg),
                    Print(&checkbox),
                    Print(" │ "),
                    Print(&title),
                    SetBackgroundColor(DraculaTheme::BG),
                )?;

                // Add separator before Size column
                queue!(
                    stdout,
                    cursor::MoveTo(sep_pos as u16, row_y),
                    SetForegroundColor(DraculaTheme::FG),
                    Print(" │ "),
                )?;

                // Position and print Size column at absolute position
                queue!(
                    stdout,
                    cursor::MoveTo(size_start as u16, row_y),
                    SetBackgroundColor(bg),
                    SetForegroundColor(fg),
                    Print(format!("{:>12} │ ", result.size_str())),
                    SetBackgroundColor(DraculaTheme::BG),
                )?;

                // Position and print Seeds column at absolute position
                queue!(
                    stdout,
                    cursor::MoveTo(seeds_start as u16, row_y),
                    SetBackgroundColor(bg),
                    SetForegroundColor(fg),
                    Print(format!("{:^5} │ ", result.seeders)),
                    SetBackgroundColor(DraculaTheme::BG),
                )?;

                // Position and print Source column at absolute position
                queue!(
                    stdout,
                    cursor::MoveTo(source_start as u16, row_y),
                    SetBackgroundColor(bg),
                    SetForegroundColor(fg),
                    Print(format!("{:<10}", indexer)),
                    SetBackgroundColor(DraculaTheme::BG),
                )?;
            }

            // Empty rows after results already have borders from the fill loop above
        }

        // Bottom border - spans from x to right border position
        // Right border is at (width - Layout::MARGIN_X - 1), so dashes fill the gap
        let right_border_pos = width - Layout::MARGIN_X - 1;
        let border_width = (right_border_pos as usize).saturating_sub(x as usize + 1);
        queue!(
            stdout,
            cursor::MoveTo(x, height + y_offset - Layout::STATUS_BAR_LINES),
            SetForegroundColor(if active { DraculaTheme::CYAN } else { DraculaTheme::FG_DIM }),
            Print("└"),
            Print("─".repeat(border_width)),
            Print("┘"),
        )?;

        // Show vv indicator inside the frame if there's more below
        if !self.results.is_empty() && has_more_below {
            queue!(
                stdout,
                cursor::MoveTo(x + 2, height - 4),
                SetForegroundColor(DraculaTheme::YELLOW),
                Print("vv"),
            )?;
        }

        Ok(())
    }

    fn draw_status_bars(&self, stdout: &mut io::Stdout, x: u16, width: u16, height: u16, y_offset: u16) -> Result<(), Box<dyn std::error::Error>> {
        // Line 1: Navigation help with result count on the right
        let help_text = "Tab/←→: panels | ↑↓: navigate | Space: toggle | Enter: search/send | ESC: quit";
        let result_count = if !self.results.is_empty() {
            format!("{} results", self.results.len())
        } else {
            String::new()
        };

        // Make sure the total width matches exactly
        let total_text_len = help_text.len() + result_count.len();
        let padding_width = if total_text_len < width as usize {
            (width as usize) - total_text_len
        } else {
            0
        };

        queue!(
            stdout,
            cursor::MoveTo(x, height + y_offset - 2),
            SetBackgroundColor(DraculaTheme::BG),
            SetForegroundColor(DraculaTheme::CYAN),
            Print(help_text),
            Print(" ".repeat(padding_width)),
            SetForegroundColor(DraculaTheme::GREEN),
            Print(&result_count),
        )?;

        // Line 2: Scrolling marquee (using cached precomputed text)
        let visible_marquee = self.marquee_cache.render(width as usize);

        queue!(
            stdout,
            cursor::MoveTo(x, height + y_offset - 1),
            SetBackgroundColor(DraculaTheme::PINK),
            SetForegroundColor(DraculaTheme::BG),
            Print(&visible_marquee),
        )?;

        // Reset colors
        queue!(
            stdout,
            SetBackgroundColor(DraculaTheme::BG),
            SetForegroundColor(DraculaTheme::FG),
        )?;
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<bool, Box<dyn std::error::Error>> {
        match key.code {
            KeyCode::Esc => {
                return Ok(false); // Quit
            }
            KeyCode::Tab => {
                self.active_panel = match self.active_panel {
                    Panel::Search => Panel::Filters,
                    Panel::Filters => Panel::Results,
                    Panel::Results => Panel::Search,
                };
                self.status_message = format!("Switched to {:?} panel", self.active_panel);
            }
            KeyCode::BackTab => {
                self.active_panel = match self.active_panel {
                    Panel::Search => Panel::Results,
                    Panel::Filters => Panel::Search,
                    Panel::Results => Panel::Filters,
                };
                self.status_message = format!("Switched to {:?} panel", self.active_panel);
            }
            KeyCode::Enter => {
                match self.active_panel {
                    Panel::Search | Panel::Filters => self.perform_search()?,
                    Panel::Results => self.send_to_putio()?,
                }
            }
            _ => {
                match self.active_panel {
                    Panel::Search => self.handle_search_key(key)?,
                    Panel::Filters => self.handle_filter_key(key)?,
                    Panel::Results => self.handle_results_key(key)?,
                }
            }
        }

        Ok(true)
    }

    fn handle_search_key(&mut self, key: KeyEvent) -> Result<(), Box<dyn std::error::Error>> {
        match key.code {
            KeyCode::Char(c) => {
                self.query.push(c);
            }
            KeyCode::Backspace => {
                self.query.pop();
            }
            KeyCode::Down => {
                if !self.results.is_empty() {
                    self.active_panel = Panel::Results;
                } else {
                    self.active_panel = Panel::Filters;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_filter_key(&mut self, key: KeyEvent) -> Result<(), Box<dyn std::error::Error>> {
        let total_items = 3 + self.available_indexers.len() + 4 + 2; // 3 sort + indexers + 4 min_seed + 2 nsfw options

        match key.code {
            KeyCode::Up if self.sort_cursor > 0 => {
                self.sort_cursor -= 1;
            }
            KeyCode::Down if self.sort_cursor < total_items - 1 => {
                self.sort_cursor += 1;
            }
            KeyCode::Right => {
                self.active_panel = Panel::Results;
            }
            KeyCode::Char(' ') | KeyCode::Enter => {
                if self.sort_cursor < 3 {
                    // Sort mode selection
                    self.sort_by = match self.sort_cursor {
                        0 => SortMode::Seeders,
                        1 => SortMode::Size,
                        2 => SortMode::Name,
                        _ => SortMode::Seeders,
                    };
                } else if self.sort_cursor < 3 + self.available_indexers.len() {
                    // Indexer selection
                    let idx = self.sort_cursor - 3;
                    if let Some(indexer) = self.available_indexers.get(idx) {
                        if indexer == "all" {
                            self.selected_indexers = vec!["all".to_string()];
                        } else {
                            if self.selected_indexers.contains(&"all".to_string()) {
                                self.selected_indexers.clear();
                            }
                            if self.selected_indexers.contains(indexer) {
                                self.selected_indexers.retain(|x| x != indexer);
                                if self.selected_indexers.is_empty() {
                                    self.selected_indexers.push("all".to_string());
                                }
                            } else {
                                self.selected_indexers.push(indexer.clone());
                            }
                        }
                        // Re-run search if we have results to filter
                        if !self.results.is_empty() && !self.query.is_empty() {
                            self.perform_search()?;
                        }
                    }
                } else if self.sort_cursor < 3 + self.available_indexers.len() + 4 {
                    // Min seeds selection
                    let min_seed_options = vec![0, 5, 10, 100];
                    let idx = self.sort_cursor - 3 - self.available_indexers.len();
                    if let Some(&seeds) = min_seed_options.get(idx) {
                        self.min_seeds = seeds;
                    }
                } else {
                    // NSFW filter selection
                    let idx = self.sort_cursor - 3 - self.available_indexers.len() - 4;
                    self.filter_nsfw = match idx {
                        0 => true,  // Filter NSFW
                        1 => false, // Allow NSFW
                        _ => true,
                    };
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_results_key(&mut self, key: KeyEvent) -> Result<(), Box<dyn std::error::Error>> {
        match key.code {
            KeyCode::Up => {
                if self.results.is_empty() {
                    // Go to search
                    self.active_panel = Panel::Search;
                } else if self.selected_index > 0 {
                    self.selected_index -= 1;
                    if self.selected_index < self.scroll_offset {
                        self.scroll_offset = self.selected_index;
                    }
                    // Reset scroll animation when changing selection
                    self.title_scroll_offset = 0;
                    self.title_scroll_direction = 1;
                } else {
                    // At top of results, go to search
                    self.active_panel = Panel::Search;
                }
            }
            KeyCode::Down if !self.results.is_empty() && self.selected_index < self.results.len().saturating_sub(1) => {
                self.selected_index += 1;
                let results_height = (self.cached_height as usize).saturating_sub(7);
                if self.selected_index >= self.scroll_offset + results_height {
                    self.scroll_offset = self.selected_index - results_height + 1;
                }
                // Reset scroll animation when changing selection
                self.title_scroll_offset = 0;
                self.title_scroll_direction = 1;
            }
            KeyCode::Left => {
                self.active_panel = Panel::Filters;
            }
            KeyCode::Char(' ') if !self.results.is_empty() => {
                if let Some(result) = self.results.get_mut(self.selected_index) {
                    result.selected = !result.selected;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn perform_search(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.query.is_empty() {
            return Ok(());
        }

        // Clear results and show spinner
        self.results.clear();
        self.searching = true;
        self.spinner_frame = 0;
        self.should_animate = true;
        self.status_message = "Fetching results...".to_string();

        if let Some(ref client) = self.chill_client {
            // Clone data needed for background thread
            let client = client.clone();
            let query = self.query.clone();
            let min_seeds = self.min_seeds;
            let sort_by = self.sort_by;
            let filter_nsfw = self.filter_nsfw;
            let debug = self.debug;

            // Map display names to API names
            let map_indexer = |name: &str| -> String {
                match name {
                    "TPB" => "thepiratebay".to_string(),
                    "EZTV" => "eztv".to_string(),
                    "RUtracker" => "rutracker".to_string(),
                    "RARBG" => "therarbg".to_string(),
                    "YTS" => "yts".to_string(),
                    _ => name.to_string(),
                }
            };

            let all_indexers: Vec<String> = self.available_indexers.iter()
                .filter(|i| *i != "all")
                .map(|i| map_indexer(i))
                .collect();

            let selected_mapped: Vec<String> = self.selected_indexers.iter()
                .map(|i| map_indexer(i))
                .collect();

            let indexers = if self.selected_indexers.contains(&"all".to_string()) {
                all_indexers
            } else {
                selected_mapped
            };

            let results_arc = Arc::clone(&self.search_results);

            // Spawn background thread for search
            thread::spawn(move || {
                if debug {
                    eprintln!("[DEBUG] Starting background search for: {}", query);
                }

                let search_result = client.search(&query, Some(&indexers), filter_nsfw);

                let processed_result = search_result.map(|mut results| {
                    // Filter by min seeds
                    if min_seeds > 0 {
                        results.retain(|r| r.seeders >= min_seeds);
                    }

                    // Sort results
                    match sort_by {
                        SortMode::Seeders => results.sort_by(|a, b| b.seeders.cmp(&a.seeders)),
                        SortMode::Name => results.sort_by(|a, b| a.title.cmp(&b.title)),
                        SortMode::Size => results.sort_by(|a, b| b.size.cmp(&a.size)),
                    }

                    results
                }).map_err(|e| e.to_string());

                // Store result in shared state
                if let Ok(mut guard) = results_arc.lock() {
                    *guard = Some(processed_result);
                }

                if debug {
                    eprintln!("[DEBUG] Background search completed");
                }
            });
        } else {
            self.status_message = "✗ Chill API key not configured".to_string();
            self.searching = false;
        }

        Ok(())
    }

    fn send_to_putio(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let selected_results: Vec<_> = self
            .results
            .iter()
            .filter(|r| r.selected)
            .collect();

        // If nothing is explicitly selected, use the currently highlighted item
        let items_to_send: Vec<_> = if selected_results.is_empty() {
            if let Some(current) = self.results.get(self.selected_index) {
                vec![current]
            } else {
                self.status_message = "✗ No results available".to_string();
                return Ok(());
            }
        } else {
            selected_results
        };

        // Store info for display message
        let file_count = items_to_send.len();
        let first_title = items_to_send.first().map(|r| r.title.clone()).unwrap_or_default();

        // Show sending message with spinner
        self.sending_to_putio = true;
        self.sending_complete = false;
        self.should_animate = true;
        self.sent_file_name = if file_count == 1 {
            format!("Sending '{}' to Put.io", first_title)
        } else {
            format!("Sending {} files to Put.io", file_count)
        };
        self.query.clear();
        self.active_panel = Panel::Search;

        if let Some(ref client) = self.putio_client {
            // Clone data for background thread
            let client = client.clone();
            let folder_name = self.config.putio_folder_name.clone();
            let folder_id = self.config.putio_folder_id;
            let debug = self.debug;
            let magnets: Vec<String> = items_to_send.iter().map(|r| r.magnet.clone()).collect();
            let send_complete = Arc::clone(&self.send_complete);

            // Clear selections immediately
            for result in &mut self.results {
                result.selected = false;
            }

            // Spawn background thread
            thread::spawn(move || {
                if debug {
                    eprintln!("[DEBUG] Starting Put.io transfer");
                }

                // Ensure folder exists
                let folder_id = match folder_id {
                    Some(id) => id,
                    None => {
                        match client.find_or_create_folder(&folder_name) {
                            Ok(id) => id,
                            Err(e) => {
                                if debug {
                                    eprintln!("[DEBUG] Failed to create folder: {}", e);
                                }
                                return;
                            }
                        }
                    }
                };

                // Send transfers
                for magnet in magnets {
                    if let Err(e) = client.add_transfer(&magnet, folder_id) {
                        if debug {
                            eprintln!("[DEBUG] Failed to add transfer: {}", e);
                        }
                    }
                }

                // Signal completion
                if let Ok(mut guard) = send_complete.lock() {
                    *guard = true;
                }

                if debug {
                    eprintln!("[DEBUG] Put.io transfer completed");
                }
            });
        } else {
            self.status_message = "✗ Put.io not configured".to_string();
        }

        Ok(())
    }
}